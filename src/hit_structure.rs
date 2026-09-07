//! Post-selection visualization of FDR-supported features.
//!
//! This module is intentionally descriptive. Features are selected using the
//! same Case/Control labels that are then displayed in the PCA/dendrogram, so
//! separation in these plots is not independent validation. The purpose is to
//! ask whether the significant hits form a coherent multivariate pattern, to
//! identify samples that do not follow that pattern, and to compare that
//! structure with the unbiased all-feature views and longitudinal patient
//! structure already produced elsewhere in the application.

use crate::analysis::CorrectedAnalysisBundle;
use crate::cohort::{
    CohortComparison, CohortModality, MetadataTable, ProjectScan, normalize_sample_id,
};
use crate::corrected::{
    AdjustedFeatureResult, CorrectedModelSpec, DendrogramResult, OrdinationResult,
    average_linkage_dendrogram_for_samples, euclidean_distance_from_feature_rows,
    parse_immune_file, parse_isotype_file, parse_repertoire_feature_maps,
    pca_from_feature_rows_for_samples, permanova_for_sample_distance, write_dendrogram,
    write_ordination,
};
use crate::metadata::row_matching_ids;
use anyhow::{Context, Result, anyhow, bail};
use csv::{ReaderBuilder, WriterBuilder};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SignificantHitStructureConfig {
    /// BH-FDR threshold used to define the post-selection feature set.
    pub q_threshold: f64,
    /// Optional absolute adjusted effect threshold applied after the q filter.
    pub min_abs_effect: f64,
    /// Safety cap for PCA/clustering. Hits are ordered by q then |effect|.
    pub max_analysis_features: usize,
    /// Number of strongest hits retained in the exported/display heatmap matrix.
    pub max_heatmap_features: usize,
}

impl Default for SignificantHitStructureConfig {
    fn default() -> Self {
        Self {
            q_threshold: 0.05,
            min_abs_effect: 0.0,
            max_analysis_features: 500,
            max_heatmap_features: 80,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignificantHitFeature {
    pub feature: String,
    pub family: String,
    pub adjusted_effect: f64,
    pub q_value: f64,
}

#[derive(Debug, Clone)]
pub struct SignificantHitPanel {
    pub modality: String,
    pub feature_set: String,
    pub sample_ids: Vec<String>,
    pub groups: Vec<String>,
    pub subjects: Vec<String>,
    pub significant_features_total: usize,
    pub analysis_features: Vec<SignificantHitFeature>,
    pub heatmap_features: Vec<SignificantHitFeature>,
    /// Feature-major z scores for display/export only. PCA/clustering use the
    /// modality-appropriate values described by `distance_label`.
    pub heatmap_zscores: Vec<Vec<f64>>,
    pub pca: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
    pub silhouette: f64,
    pub nearest_neighbor_group_accuracy: f64,
    pub distance_label: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SignificantHitPanelError {
    pub modality: String,
    pub feature_set: String,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct SignificantHitStructureResult {
    pub config: SignificantHitStructureConfig,
    pub panels: Vec<SignificantHitPanel>,
    pub errors: Vec<SignificantHitPanelError>,
    pub warnings: Vec<String>,
}

fn finite_or_inf(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        f64::INFINITY
    }
}

fn selected_features(
    tests: &[AdjustedFeatureResult],
    family: &str,
    config: &SignificantHitStructureConfig,
) -> (usize, Vec<SignificantHitFeature>) {
    let mut selected = tests
        .iter()
        .filter(|row| {
            row.q_value.is_finite()
                && row.q_value <= config.q_threshold
                && row.beta_group.is_finite()
                && row.beta_group.abs() >= config.min_abs_effect
        })
        .map(|row| SignificantHitFeature {
            feature: row.feature.clone(),
            family: if family.is_empty() {
                row.secondary_id.clone()
            } else {
                family.to_string()
            },
            adjusted_effect: row.beta_group,
            q_value: row.q_value,
        })
        .collect::<Vec<_>>();
    selected.sort_by(|a, b| {
        finite_or_inf(a.q_value)
            .total_cmp(&finite_or_inf(b.q_value))
            .then_with(|| b.adjusted_effect.abs().total_cmp(&a.adjusted_effect.abs()))
            .then_with(|| a.feature.cmp(&b.feature))
    });
    let total = selected.len();
    selected.truncate(config.max_analysis_features.max(1));
    (total, selected)
}

fn sample_annotations(
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    spec: &CorrectedModelSpec,
    sample_ids: &[String],
) -> (Vec<bool>, Vec<String>, Vec<String>) {
    let mut row_by_id: HashMap<String, Vec<usize>> = HashMap::new();
    for row_idx in 0..metadata.rows.len() {
        for id in row_matching_ids(metadata, row_idx) {
            let rows = row_by_id.entry(id).or_default();
            if !rows.contains(&row_idx) {
                rows.push(row_idx);
            }
        }
    }
    let subject_idx = spec
        .subject_column
        .as_ref()
        .and_then(|name| metadata.column_index(name));
    let mut case = Vec::with_capacity(sample_ids.len());
    let mut groups = Vec::with_capacity(sample_ids.len());
    let mut subjects = Vec::with_capacity(sample_ids.len());
    for sample in sample_ids {
        let key = normalize_sample_id(sample);
        let is_case = comparison.is_case_id(&key);
        case.push(is_case);
        groups.push(if is_case { "Case" } else { "Control" }.to_string());
        let subject = row_by_id
            .get(&key)
            .filter(|rows| rows.len() == 1)
            .and_then(|rows| subject_idx.and_then(|idx| metadata.rows[rows[0]].get(idx)))
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(normalize_sample_id)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| key.clone());
        subjects.push(subject);
    }
    (case, groups, subjects)
}

fn zscore_rows(rows: &[Vec<f64>], keep: usize) -> Vec<Vec<f64>> {
    rows.iter()
        .take(keep)
        .map(|row| {
            let finite = row
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect::<Vec<_>>();
            if finite.is_empty() {
                return vec![0.0; row.len()];
            }
            let mean = finite.iter().sum::<f64>() / finite.len() as f64;
            let var = if finite.len() > 1 {
                finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (finite.len() - 1) as f64
            } else {
                0.0
            };
            let sd = var.sqrt();
            row.iter()
                .map(|value| {
                    if value.is_finite() && sd > 1e-12 {
                        (value - mean) / sd
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

fn silhouette_two_groups(dist: &[Vec<f64>], groups: &[bool]) -> f64 {
    if dist.len() != groups.len() || groups.len() < 4 {
        return f64::NAN;
    }
    let mut values = Vec::new();
    for i in 0..groups.len() {
        let mut same_sum = 0.0;
        let mut same_n = 0usize;
        let mut other_sum = 0.0;
        let mut other_n = 0usize;
        for j in 0..groups.len() {
            if i == j || !dist[i][j].is_finite() {
                continue;
            }
            if groups[i] == groups[j] {
                same_sum += dist[i][j];
                same_n += 1;
            } else {
                other_sum += dist[i][j];
                other_n += 1;
            }
        }
        if same_n == 0 || other_n == 0 {
            continue;
        }
        let a = same_sum / same_n as f64;
        let b = other_sum / other_n as f64;
        let denom = a.max(b);
        if denom > 0.0 {
            values.push((b - a) / denom);
        }
    }
    if values.is_empty() {
        f64::NAN
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn nearest_neighbor_accuracy(dist: &[Vec<f64>], groups: &[bool]) -> f64 {
    if dist.len() != groups.len() || groups.len() < 2 {
        return f64::NAN;
    }
    let mut correct = 0usize;
    let mut tested = 0usize;
    for i in 0..groups.len() {
        let mut best = None::<(usize, f64)>;
        for j in 0..groups.len() {
            if i == j || !dist[i][j].is_finite() {
                continue;
            }
            if best.as_ref().is_none_or(|(_, d)| dist[i][j] < *d) {
                best = Some((j, dist[i][j]));
            }
        }
        if let Some((j, _)) = best {
            tested += 1;
            if groups[i] == groups[j] {
                correct += 1;
            }
        }
    }
    if tested == 0 {
        f64::NAN
    } else {
        correct as f64 / tested as f64
    }
}

fn stable_seed(text: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn build_panel(
    modality: &str,
    feature_set: &str,
    sample_ids: Vec<String>,
    group_case: Vec<bool>,
    groups: Vec<String>,
    subjects: Vec<String>,
    significant_features_total: usize,
    features: Vec<SignificantHitFeature>,
    rows: Vec<Vec<f64>>,
    distance_label: &str,
    config: &SignificantHitStructureConfig,
    permutations: usize,
    mut warnings: Vec<String>,
) -> Result<SignificantHitPanel> {
    if sample_ids.len() < 4 {
        bail!("Fewer than four modeled Case/Control samples are available");
    }
    if features.len() != rows.len() {
        bail!("Internal selected-feature/value alignment mismatch");
    }
    if rows.iter().any(|row| row.len() != sample_ids.len()) {
        bail!("Selected-feature matrix does not align to modeled samples");
    }
    if significant_features_total > features.len() {
        warnings.push(format!(
            "{} hits passed the configured threshold; PCA/clustering use the strongest {} because max_analysis_features={}",
            significant_features_total,
            features.len(),
            config.max_analysis_features
        ));
    }
    if features.len() < 2 {
        warnings.push(
            "Fewer than two significant features are available; PCA and hierarchical clustering are omitted and the hit should be inspected individually."
                .to_string(),
        );
        let keep = features.len().min(config.max_heatmap_features);
        return Ok(SignificantHitPanel {
            modality: modality.to_string(),
            feature_set: feature_set.to_string(),
            sample_ids,
            groups,
            subjects,
            significant_features_total,
            analysis_features: features.clone(),
            heatmap_features: features.into_iter().take(keep).collect(),
            heatmap_zscores: zscore_rows(&rows, keep),
            pca: None,
            dendrogram: None,
            silhouette: f64::NAN,
            nearest_neighbor_group_accuracy: f64::NAN,
            distance_label: distance_label.to_string(),
            warnings,
        });
    }
    let dist = euclidean_distance_from_feature_rows(&rows);
    let title = format!("{modality} — significant-hit PCA ({feature_set})");
    let mut pca = pca_from_feature_rows_for_samples(
        &rows,
        &sample_ids,
        &group_case,
        &subjects,
        &title,
        permutations,
    );
    let (f, p) = permanova_for_sample_distance(
        &dist,
        &group_case,
        &subjects,
        permutations,
        stable_seed(&format!("{modality}|{feature_set}")),
    );
    if let Some(ord) = pca.as_mut() {
        ord.permanova_f = f;
        ord.permanova_p = p;
        ord.permutations = permutations;
    }
    let dendrogram = average_linkage_dendrogram_for_samples(
        &dist,
        &sample_ids,
        &groups,
        &subjects,
        &format!("{modality} — significant-hit hierarchical clustering ({feature_set})"),
        distance_label,
    );
    let keep = features.len().min(config.max_heatmap_features.max(1));
    Ok(SignificantHitPanel {
        modality: modality.to_string(),
        feature_set: feature_set.to_string(),
        sample_ids,
        groups,
        subjects,
        significant_features_total,
        heatmap_features: features.iter().take(keep).cloned().collect(),
        heatmap_zscores: zscore_rows(&rows, keep),
        analysis_features: features,
        pca,
        dendrogram,
        silhouette: silhouette_two_groups(&dist, &group_case),
        nearest_neighbor_group_accuracy: nearest_neighbor_accuracy(&dist, &group_case),
        distance_label: distance_label.to_string(),
        warnings,
    })
}

fn expression_rows<F>(
    merged_tsv: &Path,
    result: &crate::corrected::CorrectedExpressionResult,
    features: &[SignificantHitFeature],
    mut progress: F,
) -> Result<Vec<Vec<f64>>>
where
    F: FnMut(usize, usize, &str),
{
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)
        .with_context(|| format!("Could not open {}", merged_tsv.display()))?;
    let headers = reader.headers()?.clone();
    let suffix = " Raw Count";
    let mut by_sample = HashMap::<String, usize>::new();
    for (idx, header) in headers.iter().enumerate().skip(2) {
        if let Some(sample) = header.strip_suffix(suffix) {
            by_sample.entry(normalize_sample_id(sample)).or_insert(idx);
        }
    }
    let columns = result
        .model_sample_ids
        .iter()
        .map(|sample| {
            by_sample
                .get(&normalize_sample_id(sample))
                .copied()
                .ok_or_else(|| anyhow!("Missing Raw Count column for modeled sample '{sample}'"))
        })
        .collect::<Result<Vec<_>>>()?;
    if result.size_factors.len() != columns.len() {
        bail!("Expression size factors do not align to modeled samples");
    }
    let wanted = features
        .iter()
        .enumerate()
        .map(|(idx, feature)| ((feature.feature.clone(), feature.family.clone()), idx))
        .collect::<HashMap<_, _>>();
    let file_units = fs::metadata(merged_tsv)
        .ok()
        .and_then(|meta| usize::try_from(meta.len()).ok())
        .unwrap_or(1)
        .max(1);
    let mut rows = vec![None::<Vec<f64>>; features.len()];
    let mut record = csv::StringRecord::new();
    let mut rows_seen = 0usize;
    while reader.read_record(&mut record)? {
        rows_seen += 1;
        let key = (
            record.get(0).unwrap_or_default().to_string(),
            record.get(1).unwrap_or_default().to_string(),
        );
        if let Some(&wanted_idx) = wanted
            .get(&key)
            .or_else(|| wanted.get(&(key.0.clone(), String::new())))
        {
            let values = columns
                .iter()
                .zip(result.size_factors.iter())
                .map(|(column, sf)| {
                    let count = record
                        .get(*column)
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .unwrap_or(0.0);
                    (count / sf.max(1e-12) + 0.5).log2()
                })
                .collect::<Vec<_>>();
            rows[wanted_idx] = Some(values);
            if rows.iter().all(Option::is_some) {
                break;
            }
        }
        if rows_seen % 5_000 == 0 {
            let done = usize::try_from(reader.position().byte())
                .unwrap_or(file_units)
                .min(file_units);
            progress(
                done,
                file_units,
                &format!(
                    "scanning merged matrix for selected hits ({rows_seen} rows; {}/{} hits found)",
                    rows.iter().filter(|row| row.is_some()).count(),
                    rows.len()
                ),
            );
        }
    }
    progress(
        file_units,
        file_units,
        &format!(
            "selected-hit matrix reconstruction complete ({}/{} hits found)",
            rows.iter().filter(|row| row.is_some()).count(),
            rows.len()
        ),
    );
    let mut out = Vec::with_capacity(rows.len());
    for (idx, row) in rows.into_iter().enumerate() {
        out.push(row.ok_or_else(|| {
            anyhow!(
                "Selected feature '{}' was not found in the merged matrix",
                features[idx].feature
            )
        })?);
    }
    Ok(out)
}

fn repertoire_metric_value(
    sample: &crate::corrected::CorrectedRepertoireSample,
    feature: &str,
) -> Option<f64> {
    match feature {
        "Matched-depth richness" => Some(sample.rarefied_richness),
        "Matched-depth Shannon diversity" => Some(sample.rarefied_shannon),
        "Clonality" => Some(sample.clonality),
        "Top clone fraction" => Some(sample.top_clone_fraction),
        "Weighted mean CDR3 AA length" => Some(sample.weighted_mean_cdr3_aa_length),
        "Private clonotype read fraction" => Some(sample.private_fraction),
        "Longitudinally shared clonotype read fraction" => {
            Some(sample.longitudinal_shared_fraction)
        }
        "Public clonotype read fraction" => Some(sample.public_fraction),
        "AA convergence read fraction" => Some(sample.convergent_read_fraction),
        _ => None,
    }
}

fn standardized_metric_rows(
    result: &crate::corrected::CorrectedRepertoireResult,
    features: &[SignificantHitFeature],
) -> Result<Vec<Vec<f64>>> {
    let mut rows = Vec::new();
    for feature in features {
        let values = result
            .sample_summaries
            .iter()
            .map(|sample| repertoire_metric_value(sample, &feature.feature).unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        rows.push(
            zscore_rows(&[values], 1)
                .into_iter()
                .next()
                .unwrap_or_default(),
        );
    }
    Ok(rows)
}

fn chosen_files<'a>(
    scan: &'a ProjectScan,
    modality: CohortModality,
    sample_ids: &[String],
) -> BTreeMap<String, &'a crate::cohort::DetectedCohortFile> {
    let wanted = sample_ids
        .iter()
        .map(|sample| normalize_sample_id(sample))
        .collect::<HashSet<_>>();
    let mut chosen = BTreeMap::new();
    for file in scan.files.iter().filter(|file| file.modality == modality) {
        let key = normalize_sample_id(&file.sample_id);
        if wanted.contains(&key) {
            chosen.entry(key).or_insert(file);
        }
    }
    chosen
}

fn load_repertoire_composition_maps<F>(
    scan: &ProjectScan,
    modality: CohortModality,
    sample_ids: &[String],
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<Vec<BTreeMap<String, HashMap<String, f64>>>>
where
    F: FnMut(usize, usize, &str),
{
    let files = chosen_files(scan, modality, sample_ids);
    let mut parsed = Vec::with_capacity(sample_ids.len());
    let total = sample_ids.len().max(1);
    for (idx, sample) in sample_ids.iter().enumerate() {
        progress(
            idx,
            total,
            &format!("reconstructing selected composition coordinates for {sample}"),
        );
        let key = normalize_sample_id(sample);
        let file = files
            .get(&key)
            .ok_or_else(|| anyhow!("Missing {:?} file for modeled sample '{sample}'", modality))?;
        parsed.push(parse_repertoire_feature_maps(
            &file.path,
            modality,
            spec.ambiguous_policy,
        )?);
    }
    progress(total, total, "selected composition reconstruction complete");
    Ok(parsed)
}

fn repertoire_composition_rows(
    parsed_maps: &[BTreeMap<String, HashMap<String, f64>>],
    composition: &crate::corrected::CorrectedCompositionResult,
    features: &[SignificantHitFeature],
    spec: &CorrectedModelSpec,
) -> Result<Vec<Vec<f64>>> {
    let basis = composition.retained_feature_names.clone();
    let selected_index = features
        .iter()
        .map(|feature| {
            basis
                .iter()
                .position(|name| name == &feature.feature)
                .ok_or_else(|| {
                    anyhow!(
                        "Selected composition feature '{}' is not in the corrected CLR basis",
                        feature.feature
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let pseudo = spec.clr_pseudocount.max(1e-12);
    let mut rows = vec![Vec::with_capacity(parsed_maps.len()); features.len()];
    for maps in parsed_maps {
        let map = maps.get(&composition.family).ok_or_else(|| {
            anyhow!(
                "Missing '{}' composition in parsed repertoire",
                composition.family
            )
        })?;
        let logs = basis
            .iter()
            .map(|name| (map.get(name).copied().unwrap_or(0.0) + pseudo).ln())
            .collect::<Vec<_>>();
        let center = logs.iter().sum::<f64>() / logs.len().max(1) as f64;
        for (row_idx, basis_idx) in selected_index.iter().enumerate() {
            rows[row_idx].push(logs[*basis_idx] - center);
        }
    }
    Ok(rows)
}

fn isotype_rows(
    scan: &ProjectScan,
    result: &crate::corrected::CorrectedIsotypeResult,
    sample_ids: &[String],
    features: &[SignificantHitFeature],
    spec: &CorrectedModelSpec,
) -> Result<Vec<Vec<f64>>> {
    let files = chosen_files(scan, CohortModality::BcrIsotype, sample_ids);
    let basis = result.retained_feature_names.clone();
    let selected_index = features
        .iter()
        .map(|feature| {
            basis
                .iter()
                .position(|name| name == &feature.feature)
                .ok_or_else(|| {
                    anyhow!(
                        "Selected isotype '{}' is not in the corrected CLR basis",
                        feature.feature
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let pseudo = spec.clr_pseudocount.max(1e-12);
    let mut rows = vec![Vec::with_capacity(sample_ids.len()); features.len()];
    for sample in sample_ids {
        let file = files
            .get(&normalize_sample_id(sample))
            .ok_or_else(|| anyhow!("Missing BCR-isotype file for modeled sample '{sample}'"))?;
        let map = parse_isotype_file(&file.path, result.determined_only)?;
        let logs = basis
            .iter()
            .map(|name| (map.get(name).copied().unwrap_or(0.0) + pseudo).ln())
            .collect::<Vec<_>>();
        let center = logs.iter().sum::<f64>() / logs.len().max(1) as f64;
        for (row_idx, basis_idx) in selected_index.iter().enumerate() {
            rows[row_idx].push(logs[*basis_idx] - center);
        }
    }
    Ok(rows)
}

fn immune_rows(
    scan: &ProjectScan,
    sample_ids: &[String],
    features: &[SignificantHitFeature],
) -> Result<Vec<Vec<f64>>> {
    let files = chosen_files(scan, CohortModality::ImmuneInfiltrate, sample_ids);
    let mut rows = vec![Vec::with_capacity(sample_ids.len()); features.len()];
    for sample in sample_ids {
        let file = files.get(&normalize_sample_id(sample)).ok_or_else(|| {
            anyhow!("Missing immune-infiltrate file for modeled sample '{sample}'")
        })?;
        let map = parse_immune_file(&file.path)?;
        for (idx, feature) in features.iter().enumerate() {
            rows[idx].push(map.get(&feature.feature).copied().unwrap_or(f64::NAN));
        }
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            zscore_rows(&[row], 1)
                .into_iter()
                .next()
                .unwrap_or_default()
        })
        .collect())
}

fn sample_ids_from_ordination_or_tree(
    ordination: Option<&OrdinationResult>,
    tree: Option<&DendrogramResult>,
) -> Vec<String> {
    if let Some(ord) = ordination {
        return ord
            .points
            .iter()
            .map(|point| point.sample_id.clone())
            .collect();
    }
    tree.map(|tree| {
        tree.leaves
            .iter()
            .map(|leaf| leaf.sample_id.clone())
            .collect()
    })
    .unwrap_or_default()
}

fn push_panel_result(
    panels: &mut Vec<SignificantHitPanel>,
    errors: &mut Vec<SignificantHitPanelError>,
    modality: &str,
    feature_set: &str,
    result: Result<SignificantHitPanel>,
) {
    match result {
        Ok(panel) => panels.push(panel),
        Err(error) => errors.push(SignificantHitPanelError {
            modality: modality.to_string(),
            feature_set: feature_set.to_string(),
            error: format!("{error:#}"),
        }),
    }
}

pub fn run_significant_hit_structure<F>(
    scan: &ProjectScan,
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    spec: &CorrectedModelSpec,
    primary: &CorrectedAnalysisBundle,
    config: SignificantHitStructureConfig,
    mut progress: F,
) -> Result<SignificantHitStructureResult>
where
    F: FnMut(usize, usize, &str),
{
    if !(0.0..=1.0).contains(&config.q_threshold) {
        bail!("Significant-hit q threshold must be between 0 and 1");
    }
    if !config.min_abs_effect.is_finite() || config.min_abs_effect < 0.0 {
        bail!("Significant-hit minimum absolute effect must be finite and non-negative");
    }
    if config.max_analysis_features < 2 {
        bail!("Significant-hit max_analysis_features must be at least 2");
    }
    if config.max_heatmap_features == 0 {
        bail!("Significant-hit max_heatmap_features must be at least 1");
    }
    let mut panels = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = vec![
        "Post-selection plots reuse the same Case/Control labels that selected the significant features. Separation is descriptive evidence of coherence, not independent validation or predictive performance."
            .to_string(),
    ];
    let mut stages = 0usize;
    stages += if primary.gene.is_some() { 1 } else { 0 };
    stages += if primary.transcript.is_some() { 1 } else { 0 };
    stages += if primary.bcr.is_some() { 1 } else { 0 };
    stages += if primary.bcr_isotype.is_some() { 1 } else { 0 };
    stages += if primary.tcr_alpha.is_some() { 1 } else { 0 };
    stages += if primary.tcr_beta.is_some() { 1 } else { 0 };
    stages += if primary.immune.is_some() { 1 } else { 0 };
    let total = stages.max(1);
    let mut stage = 0usize;

    if let (Some(result), Some(path)) = (primary.gene.as_ref(), scan.merged_gene_tsv.as_deref()) {
        progress(stage, total, "Gene expression");
        let (hit_total, features) = selected_features(&result.tests, "", &config);
        if hit_total > 0 {
            let rows = expression_rows(path, result, &features, |done, inner_total, message| {
                progress(
                    stage,
                    total,
                    &format!("Gene expression — {message} [{done}/{inner_total}]"),
                );
            });
            let sample_ids = result.model_sample_ids.clone();
            let (case, groups, subjects) =
                sample_annotations(metadata, comparison, spec, &sample_ids);
            push_panel_result(
                &mut panels,
                &mut errors,
                "Gene expression",
                "significant adjusted abundance hits",
                rows.and_then(|rows| {
                    build_panel(
                        "Gene expression",
                        "significant adjusted abundance hits",
                        sample_ids,
                        case,
                        groups,
                        subjects,
                        hit_total,
                        features,
                        rows,
                        "Euclidean distance on significant normalized log2 counts",
                        &config,
                        spec.permutations,
                        Vec::new(),
                    )
                }),
            );
        }
        stage += 1;
    }
    if let (Some(result), Some(path)) = (
        primary.transcript.as_ref(),
        scan.merged_transcript_tsv.as_deref(),
    ) {
        progress(stage, total, "Transcript / isoform expression");
        let (hit_total, features) = selected_features(&result.tests, "", &config);
        if hit_total > 0 {
            let rows = expression_rows(path, result, &features, |done, inner_total, message| {
                progress(
                    stage,
                    total,
                    &format!("Transcript / isoform expression — {message} [{done}/{inner_total}]"),
                );
            });
            let sample_ids = result.model_sample_ids.clone();
            let (case, groups, subjects) =
                sample_annotations(metadata, comparison, spec, &sample_ids);
            push_panel_result(
                &mut panels,
                &mut errors,
                "Transcript / isoform expression",
                "significant adjusted abundance hits",
                rows.and_then(|rows| {
                    build_panel(
                        "Transcript / isoform expression",
                        "significant adjusted abundance hits",
                        sample_ids,
                        case,
                        groups,
                        subjects,
                        hit_total,
                        features,
                        rows,
                        "Euclidean distance on significant normalized log2 counts",
                        &config,
                        spec.permutations,
                        Vec::new(),
                    )
                }),
            );
        }
        stage += 1;
    }

    for (modality, label, result_opt) in [
        (CohortModality::Bcr, "BCR", primary.bcr.as_ref()),
        (
            CohortModality::TcrAlpha,
            "TCR alpha",
            primary.tcr_alpha.as_ref(),
        ),
        (
            CohortModality::TcrBeta,
            "TCR beta",
            primary.tcr_beta.as_ref(),
        ),
    ] {
        let Some(result) = result_opt else {
            continue;
        };
        progress(stage, total, &format!("{label}"));
        let sample_ids = result
            .sample_summaries
            .iter()
            .map(|row| row.sample_id.clone())
            .collect::<Vec<_>>();
        let group_case = result
            .sample_summaries
            .iter()
            .map(|row| row.group == comparison.case_label || row.group == "Case")
            .collect::<Vec<_>>();
        let groups = group_case
            .iter()
            .map(|case| if *case { "Case" } else { "Control" }.to_string())
            .collect::<Vec<_>>();
        let subjects = result
            .sample_summaries
            .iter()
            .map(|row| row.subject.clone())
            .collect::<Vec<_>>();
        let (hit_total, features) =
            selected_features(&result.metric_tests, "Repertoire metrics", &config);
        if hit_total > 0 {
            let rows = standardized_metric_rows(result, &features);
            push_panel_result(
                &mut panels,
                &mut errors,
                label,
                "significant repertoire metrics",
                rows.and_then(|rows| {
                    build_panel(
                        label,
                        "significant repertoire metrics",
                        sample_ids.clone(),
                        group_case.clone(),
                        groups.clone(),
                        subjects.clone(),
                        hit_total,
                        features,
                        rows,
                        "Euclidean distance on standardized significant repertoire metrics",
                        &config,
                        spec.permutations,
                        Vec::new(),
                    )
                }),
            );
        }
        let selected_compositions = result
            .compositions
            .iter()
            .filter_map(|composition| {
                let (hit_total, features) =
                    selected_features(&composition.tests, &composition.family, &config);
                (hit_total > 0).then_some((composition, hit_total, features))
            })
            .collect::<Vec<_>>();
        if !selected_compositions.is_empty() {
            match load_repertoire_composition_maps(
                scan,
                modality,
                &sample_ids,
                spec,
                |done, inner_total, message| {
                    progress(
                        stage,
                        total,
                        &format!("{label} — {message} [{done}/{inner_total}]"),
                    );
                },
            ) {
                Ok(parsed_maps) => {
                    for (composition, hit_total, features) in selected_compositions {
                        let feature_set =
                            format!("significant {} CLR composition hits", composition.family);
                        let rows =
                            repertoire_composition_rows(&parsed_maps, composition, &features, spec);
                        push_panel_result(
                            &mut panels,
                            &mut errors,
                            label,
                            &feature_set,
                            rows.and_then(|rows| {
                                build_panel(
                                    label,
                                    &feature_set,
                                    sample_ids.clone(),
                                    group_case.clone(),
                                    groups.clone(),
                                    subjects.clone(),
                                    hit_total,
                                    features,
                                    rows,
                                    "Euclidean distance on selected CLR composition coordinates",
                                    &config,
                                    spec.permutations,
                                    Vec::new(),
                                )
                            }),
                        );
                    }
                }
                Err(error) => {
                    let message = format!("{error:#}");
                    for (composition, _, _) in selected_compositions {
                        errors.push(SignificantHitPanelError {
                            modality: label.to_string(),
                            feature_set: format!(
                                "significant {} CLR composition hits",
                                composition.family
                            ),
                            error: message.clone(),
                        });
                    }
                }
            }
        }
        stage += 1;
    }

    if let Some(result) = primary.bcr_isotype.as_ref() {
        progress(stage, total, "BCR isotype");
        let (hit_total, features) = selected_features(&result.tests, "Isotype", &config);
        if hit_total > 0 {
            let sample_ids = sample_ids_from_ordination_or_tree(
                result.ordination.as_ref(),
                result.dendrogram.as_ref(),
            );
            let (case, groups, subjects) =
                sample_annotations(metadata, comparison, spec, &sample_ids);
            let rows = isotype_rows(scan, result, &sample_ids, &features, spec);
            push_panel_result(
                &mut panels,
                &mut errors,
                "BCR isotype",
                "significant CLR isotype hits",
                rows.and_then(|rows| {
                    build_panel(
                        "BCR isotype",
                        "significant CLR isotype hits",
                        sample_ids,
                        case,
                        groups,
                        subjects,
                        hit_total,
                        features,
                        rows,
                        "Euclidean distance on selected CLR isotype coordinates",
                        &config,
                        spec.permutations,
                        Vec::new(),
                    )
                }),
            );
        }
        stage += 1;
    }

    if let Some(result) = primary.immune.as_ref() {
        progress(stage, total, "Immune infiltration");
        let (hit_total, features) = selected_features(&result.tests, "ssGSEA score", &config);
        if hit_total > 0 {
            let sample_ids =
                sample_ids_from_ordination_or_tree(result.pca.as_ref(), result.dendrogram.as_ref());
            let (case, groups, subjects) =
                sample_annotations(metadata, comparison, spec, &sample_ids);
            let rows = immune_rows(scan, &sample_ids, &features);
            push_panel_result(
                &mut panels,
                &mut errors,
                "Immune infiltration",
                "significant adjusted ssGSEA hits",
                rows.and_then(|rows| {
                    build_panel(
                        "Immune infiltration",
                        "significant adjusted ssGSEA hits",
                        sample_ids,
                        case,
                        groups,
                        subjects,
                        hit_total,
                        features,
                        rows,
                        "Euclidean distance on standardized significant ssGSEA scores",
                        &config,
                        spec.permutations,
                        Vec::new(),
                    )
                }),
            );
        }
    }

    if panels.is_empty() && errors.is_empty() {
        warnings.push(format!(
            "No selected modality had features satisfying BH q <= {:.3} and |adjusted effect| >= {:.3}; no significant-hit PCA/dendrogram was generated.",
            config.q_threshold, config.min_abs_effect
        ));
    }
    progress(total, total, "Complete");
    Ok(SignificantHitStructureResult {
        config,
        panels,
        errors,
        warnings,
    })
}

/// Sample indices in hierarchical leaf order when a dendrogram is available.
/// Falls back to the modeled sample order.  The helper is shared by the native
/// view, HTML report, and technical export so every selected-hit heatmap uses
/// the exact same sample ordering.
pub fn clustered_sample_order(panel: &SignificantHitPanel) -> Vec<usize> {
    if let Some(tree) = panel.dendrogram.as_ref() {
        let order = tree
            .leaves
            .iter()
            .filter_map(|leaf| {
                panel.sample_ids.iter().position(|sample| {
                    normalize_sample_id(sample) == normalize_sample_id(&leaf.sample_id)
                })
            })
            .collect::<Vec<_>>();
        if order.len() == panel.sample_ids.len() {
            return order;
        }
    }
    (0..panel.sample_ids.len()).collect()
}

fn safe_component(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

pub fn write_significant_hit_structure_tables(
    outdir: &Path,
    result: &SignificantHitStructureResult,
) -> Result<()> {
    fs::create_dir_all(outdir)?;
    let mut summary = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(outdir.join("summary.tsv"))?;
    summary.write_record([
        "modality",
        "feature_set",
        "significant_hits_total",
        "analysis_features",
        "samples",
        "silhouette",
        "nearest_neighbor_group_accuracy",
        "permanova_f",
        "permanova_p",
        "distance",
    ])?;
    for panel in &result.panels {
        let (f, p) = panel
            .pca
            .as_ref()
            .map(|ord| (ord.permanova_f, ord.permanova_p))
            .unwrap_or((f64::NAN, f64::NAN));
        summary.write_record([
            panel.modality.as_str(),
            panel.feature_set.as_str(),
            &panel.significant_features_total.to_string(),
            &panel.analysis_features.len().to_string(),
            &panel.sample_ids.len().to_string(),
            &panel.silhouette.to_string(),
            &panel.nearest_neighbor_group_accuracy.to_string(),
            &f.to_string(),
            &p.to_string(),
            panel.distance_label.as_str(),
        ])?;
    }
    summary.flush()?;
    for (idx, panel) in result.panels.iter().enumerate() {
        let stem = format!(
            "{:02}_{}_{}",
            idx + 1,
            safe_component(&panel.modality),
            safe_component(&panel.feature_set)
        );
        let mut features = WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(outdir.join(format!("{stem}_features.tsv")))?;
        features.write_record(["feature", "family", "adjusted_effect", "q_value"])?;
        for feature in &panel.analysis_features {
            features.write_record([
                feature.feature.as_str(),
                feature.family.as_str(),
                &feature.adjusted_effect.to_string(),
                &feature.q_value.to_string(),
            ])?;
        }
        features.flush()?;
        if let Some(ord) = panel.pca.as_ref() {
            write_ordination(&outdir.join(format!("{stem}_pca.tsv")), ord)?;
        }
        if let Some(tree) = panel.dendrogram.as_ref() {
            write_dendrogram(
                &outdir.join(format!("{stem}_dendrogram_leaf_order.tsv")),
                tree,
            )?;
        }
        if !panel.heatmap_features.is_empty() {
            let order = clustered_sample_order(panel);
            let mut heatmap = WriterBuilder::new()
                .delimiter(b'\t')
                .from_path(outdir.join(format!("{stem}_clustered_heatmap_zscores.tsv")))?;
            let mut header = vec!["feature".to_string(), "family".to_string()];
            header.extend(order.iter().map(|idx| panel.sample_ids[*idx].clone()));
            heatmap.write_record(header)?;
            for (feature, values) in panel
                .heatmap_features
                .iter()
                .zip(panel.heatmap_zscores.iter())
            {
                let mut row = vec![feature.feature.clone(), feature.family.clone()];
                row.extend(order.iter().map(|idx| format!("{:.6}", values[*idx])));
                heatmap.write_record(row)?;
            }
            heatmap.flush()?;

            let mut annotations = WriterBuilder::new()
                .delimiter(b'\t')
                .from_path(outdir.join(format!("{stem}_clustered_heatmap_samples.tsv")))?;
            annotations.write_record(["cluster_order", "sample_id", "group", "subject"])?;
            for (rank, idx) in order.iter().enumerate() {
                annotations.write_record([
                    &(rank + 1).to_string(),
                    panel.sample_ids[*idx].as_str(),
                    panel.groups[*idx].as_str(),
                    panel.subjects[*idx].as_str(),
                ])?;
            }
            annotations.flush()?;
        }
    }
    if !result.errors.is_empty() {
        let mut errors = WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(outdir.join("errors.tsv"))?;
        errors.write_record(["modality", "feature_set", "error"])?;
        for error in &result.errors {
            errors.write_record([
                error.modality.as_str(),
                error.feature_set.as_str(),
                error.error.as_str(),
            ])?;
        }
        errors.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silhouette_is_positive_for_clean_separation() {
        let dist = vec![
            vec![0.0, 0.1, 2.0, 2.1],
            vec![0.1, 0.0, 2.1, 2.0],
            vec![2.0, 2.1, 0.0, 0.1],
            vec![2.1, 2.0, 0.1, 0.0],
        ];
        let groups = vec![false, false, true, true];
        assert!(silhouette_two_groups(&dist, &groups) > 0.9);
        assert_eq!(nearest_neighbor_accuracy(&dist, &groups), 1.0);
    }

    #[test]
    fn selected_hits_obey_q_and_effect_thresholds() {
        let mk = |feature: &str, effect: f64, q: f64| AdjustedFeatureResult {
            feature: feature.to_string(),
            secondary_id: String::new(),
            n_samples: 8,
            n_subjects: 8,
            n_case: 4,
            n_control: 4,
            mean_case: 1.0,
            mean_control: 0.0,
            beta_group: effect,
            robust_se: 0.1,
            z_value: 2.0,
            p_value: q / 2.0,
            q_value: q,
            subject_effect: effect,
            subject_p_value: q / 2.0,
            subject_q_value: q,
            method: "test".to_string(),
        };
        let cfg = SignificantHitStructureConfig {
            q_threshold: 0.05,
            min_abs_effect: 0.5,
            max_analysis_features: 10,
            max_heatmap_features: 5,
        };
        let (total, rows) = selected_features(
            &[mk("A", 1.0, 0.01), mk("B", 0.2, 0.01), mk("C", 2.0, 0.2)],
            "",
            &cfg,
        );
        assert_eq!(total, 1);
        assert_eq!(rows[0].feature, "A");
    }
}
