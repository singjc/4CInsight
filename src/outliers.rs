//! Cross-modal sample outlier screening for cohort QC.
//!
//! The detector is intentionally unsupervised with respect to the tested
//! feature effects.  Each modality is reduced to a sample-level representation
//! (expression PCA, repertoire-summary PCA, CLR isotype PCA, or immune-signature
//! PCA), then robust distances are calculated either within the case/control
//! groups or across the complete comparison.  These flags are QC signals, not
//! automatic proof that a sample is biologically invalid.

use crate::cohort::{CohortComparison, CohortModality, DetectedCohortFile, normalize_sample_id};
use crate::corrected::{parse_immune_file, parse_isotype_file, parse_repertoire_qc_metrics};
use anyhow::{Context, Result, anyhow, bail};
use csv::ReaderBuilder;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlierScope {
    WithinGroup,
    AcrossAll,
}

impl OutlierScope {
    pub const ALL: [Self; 2] = [Self::WithinGroup, Self::AcrossAll];
    pub fn display_name(self) -> &'static str {
        match self {
            Self::WithinGroup => "Within case/control group (recommended)",
            Self::AcrossAll => "Across all samples in the comparison",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlierConsensusRule {
    Any,
    AtLeastN,
    Majority,
    AllAvailable,
}

impl OutlierConsensusRule {
    pub const ALL: [Self; 4] = [
        Self::Any,
        Self::AtLeastN,
        Self::Majority,
        Self::AllAvailable,
    ];
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Any => "Outlier in any selected modality",
            Self::AtLeastN => "Outlier in at least N modalities",
            Self::Majority => "Outlier in a majority of available modalities",
            Self::AllAvailable => "Outlier in all available selected modalities",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutlierConfig {
    pub scope: OutlierScope,
    pub robust_distance_threshold: f64,
    pub consensus_rule: OutlierConsensusRule,
    pub at_least_n: usize,
    pub min_available_modalities: usize,
    pub expression_features: usize,
}

impl Default for OutlierConfig {
    fn default() -> Self {
        Self {
            scope: OutlierScope::WithinGroup,
            robust_distance_threshold: 4.0,
            consensus_rule: OutlierConsensusRule::AtLeastN,
            at_least_n: 2,
            min_available_modalities: 2,
            expression_features: 300,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModalityOutlierPoint {
    pub sample_id: String,
    pub group: String,
    pub x: f64,
    pub y: f64,
    pub robust_distance: f64,
    pub flagged: bool,
}

#[derive(Debug, Clone)]
pub struct ModalityOutlierResult {
    pub modality: String,
    pub x_label: String,
    pub y_label: String,
    pub representation: String,
    pub points: Vec<ModalityOutlierPoint>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SampleOutlierSummary {
    pub sample_id: String,
    pub group: String,
    pub available_modalities: usize,
    pub flagged_modalities: usize,
    pub flagged_names: Vec<String>,
    pub consensus_flag: bool,
}

#[derive(Debug, Clone)]
pub struct CrossModalOutlierResult {
    pub comparison_name: String,
    pub config: OutlierConfig,
    pub modalities: Vec<ModalityOutlierResult>,
    pub samples: Vec<SampleOutlierSummary>,
    pub consensus_ids: Vec<String>,
    pub warnings: Vec<String>,
}

/// Immutable record of the exact reviewed QC filter that was applied to a
/// downstream analysis.  Keeping this snapshot separate from the live UI
/// state makes exported tables/reports reproducible even if the user changes
/// the outlier settings after the analysis has finished.
#[derive(Debug, Clone)]
pub struct OutlierFilterSnapshot {
    pub audit: CrossModalOutlierResult,
    pub excluded_ids: Vec<String>,
    pub original_case_n: usize,
    pub original_control_n: usize,
    pub filtered_case_n: usize,
    pub filtered_control_n: usize,
    pub applied_to_analysis: bool,
}

pub fn build_outlier_filter_snapshot(
    audit: &CrossModalOutlierResult,
    reviewed_exclusions: &HashSet<String>,
    original: &CohortComparison,
    filtered: &CohortComparison,
    applied_to_analysis: bool,
) -> OutlierFilterSnapshot {
    let mut audit_subset = audit.clone();
    audit_subset
        .samples
        .retain(|row| original.contains_id(&row.sample_id));
    audit_subset
        .consensus_ids
        .retain(|id| original.contains_id(id));
    for modality in &mut audit_subset.modalities {
        modality
            .points
            .retain(|point| original.contains_id(&point.sample_id));
    }

    let reviewed_canonical = reviewed_exclusions
        .iter()
        .map(|id| original.canonical_id(id))
        .collect::<HashSet<_>>();
    let filtered_ids = filtered
        .case_ids
        .iter()
        .chain(filtered.control_ids.iter())
        .cloned()
        .collect::<HashSet<_>>();
    let mut excluded_ids = reviewed_canonical
        .into_iter()
        .filter(|id| {
            (original.case_ids.contains(id) || original.control_ids.contains(id))
                && !filtered_ids.contains(id)
        })
        .collect::<Vec<_>>();
    excluded_ids.sort();
    excluded_ids.dedup();
    OutlierFilterSnapshot {
        audit: audit_subset,
        excluded_ids,
        original_case_n: original.case_ids.len(),
        original_control_n: original.control_ids.len(),
        filtered_case_n: filtered.case_ids.len(),
        filtered_control_n: filtered.control_ids.len(),
        applied_to_analysis,
    }
}

pub fn write_outlier_filter_snapshot_csv(
    path: &Path,
    snapshot: &OutlierFilterSnapshot,
) -> Result<()> {
    let excluded = snapshot
        .excluded_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let consensus = snapshot
        .audit
        .consensus_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut writer = csv::WriterBuilder::new()
        .buffer_capacity(1024 * 1024)
        .from_path(path)
        .with_context(|| format!("Could not create {}", path.display()))?;
    let mut header = vec![
        "sample_id".to_string(),
        "group".to_string(),
        "available_modalities".to_string(),
        "flagged_modalities".to_string(),
        "consensus_flag".to_string(),
        "reviewed_excluded".to_string(),
        "applied_to_analysis".to_string(),
        "qc_decision".to_string(),
        "flagged_in".to_string(),
    ];
    for modality in &snapshot.audit.modalities {
        let stem = modality
            .modality
            .to_ascii_lowercase()
            .replace(' ', "_")
            .replace('/', "_")
            .replace('-', "_");
        header.push(format!("{stem}_pc1"));
        header.push(format!("{stem}_pc2"));
        header.push(format!("{stem}_robust_distance"));
        header.push(format!("{stem}_flag"));
    }
    writer.write_record(&header)?;
    for sample in &snapshot.audit.samples {
        let is_excluded = excluded.contains(&sample.sample_id);
        let is_consensus = consensus.contains(&sample.sample_id);
        let decision = match (is_consensus, is_excluded) {
            (true, true) => "excluded_consensus",
            (true, false) => "kept_manual_override",
            (false, true) => "excluded_manual_override",
            (false, false) => "kept",
        };
        let mut row = vec![
            sample.sample_id.clone(),
            sample.group.clone(),
            sample.available_modalities.to_string(),
            sample.flagged_modalities.to_string(),
            is_consensus.to_string(),
            is_excluded.to_string(),
            snapshot.applied_to_analysis.to_string(),
            decision.to_string(),
            sample.flagged_names.join("; "),
        ];
        for modality in &snapshot.audit.modalities {
            let point = modality
                .points
                .iter()
                .find(|p| normalize_sample_id(&p.sample_id) == sample.sample_id);
            row.push(point.map(|p| format!("{:.6}", p.x)).unwrap_or_default());
            row.push(point.map(|p| format!("{:.6}", p.y)).unwrap_or_default());
            row.push(
                point
                    .map(|p| format!("{:.6}", p.robust_distance))
                    .unwrap_or_default(),
            );
            row.push(point.map(|p| p.flagged.to_string()).unwrap_or_default());
        }
        writer.write_record(&row)?;
    }
    writer.flush()?;
    Ok(())
}

pub fn write_outlier_filter_modalities_tsv(
    path: &Path,
    snapshot: &OutlierFilterSnapshot,
) -> Result<()> {
    let excluded = snapshot
        .excluded_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(path)
        .with_context(|| format!("Could not create {}", path.display()))?;
    writer.write_record([
        "modality",
        "representation",
        "sample_id",
        "group",
        "component_1",
        "component_2",
        "robust_distance",
        "modality_flag",
        "reviewed_excluded",
    ])?;
    for modality in &snapshot.audit.modalities {
        for point in &modality.points {
            let sample_id = normalize_sample_id(&point.sample_id);
            writer.serialize((
                &modality.modality,
                &modality.representation,
                &sample_id,
                &point.group,
                point.x,
                point.y,
                point.robust_distance,
                point.flagged,
                excluded.contains(&sample_id),
            ))?;
        }
    }
    writer.flush()?;
    Ok(())
}

pub fn write_outlier_filter_summary(path: &Path, snapshot: &OutlierFilterSnapshot) -> Result<()> {
    let cfg = &snapshot.audit.config;
    let consensus = snapshot.audit.consensus_ids.len();
    let manual_excluded = snapshot
        .excluded_ids
        .iter()
        .filter(|id| !snapshot.audit.consensus_ids.contains(id))
        .count();
    let consensus_kept = snapshot
        .audit
        .consensus_ids
        .iter()
        .filter(|id| !snapshot.excluded_ids.contains(id))
        .count();
    let mut text = String::new();
    text.push_str(
        "Sample outlier QC / exclusion summary\n=====================================\n\n",
    );
    text.push_str(&format!(
        "Comparison used for QC: {}\n",
        snapshot.audit.comparison_name
    ));
    text.push_str(&format!("Detection scope: {}\n", cfg.scope.display_name()));
    text.push_str(&format!(
        "Robust distance threshold: {:.3}\n",
        cfg.robust_distance_threshold
    ));
    text.push_str(&format!(
        "Cross-modal rule: {}\n",
        cfg.consensus_rule.display_name()
    ));
    if cfg.consensus_rule == OutlierConsensusRule::AtLeastN {
        text.push_str(&format!(
            "Required flagged modalities: {}\n",
            cfg.at_least_n
        ));
    }
    text.push_str(&format!(
        "Minimum available modalities: {}\n",
        cfg.min_available_modalities
    ));
    text.push_str(&format!(
        "Expression features used for QC representation: {}\n",
        cfg.expression_features
    ));
    text.push_str(&format!("Consensus-flagged samples: {}\n", consensus));
    text.push_str(&format!(
        "Reviewed exclusions selected: {}\n",
        snapshot.excluded_ids.len()
    ));
    text.push_str(&format!(
        "QC exclusions applied to this analysis: {}\n",
        snapshot.applied_to_analysis
    ));
    text.push_str(&format!(
        "Manual exclusions outside consensus: {}\n",
        manual_excluded
    ));
    text.push_str(&format!(
        "Consensus flags manually retained: {}\n\n",
        consensus_kept
    ));
    let after_label = if snapshot.applied_to_analysis {
        "after applied QC exclusions"
    } else {
        "after reviewed exclusions if applied downstream"
    };
    text.push_str(&format!(
        "Case samples: {} before QC -> {} {}\n",
        snapshot.original_case_n, snapshot.filtered_case_n, after_label
    ));
    text.push_str(&format!(
        "Control samples: {} before QC -> {} {}\n\n",
        snapshot.original_control_n, snapshot.filtered_control_n, after_label
    ));
    text.push_str("Excluded sample IDs:\n");
    if snapshot.excluded_ids.is_empty() {
        text.push_str("  none\n");
    } else {
        for id in &snapshot.excluded_ids {
            text.push_str(&format!("  - {id}\n"));
        }
    }
    if !snapshot.audit.warnings.is_empty() {
        text.push_str("\nQC warnings:\n");
        for warning in &snapshot.audit.warnings {
            text.push_str(&format!("  - {warning}\n"));
        }
    }
    text.push_str("\nInterpretation:\n");
    if snapshot.applied_to_analysis {
        text.push_str("Outlier flags are unsupervised QC signals based on sample-level structure within the selected modality. They are not proof that a sample is biologically invalid. The reviewed exclusion list shown above is the exact set removed from this downstream analysis and may differ from the automatic consensus list because the user can manually retain or exclude samples after review.\n");
    } else {
        text.push_str("Outlier flags are unsupervised QC signals based on sample-level structure within the selected modality. They are not proof that a sample is biologically invalid. The reviewed exclusion list was recorded for downstream sensitivity analyses but was not removed from this exploratory result.\n");
    }
    std::fs::write(path, text).with_context(|| format!("Could not write {}", path.display()))?;
    Ok(())
}

pub fn write_outlier_audit_csv(path: &Path, result: &CrossModalOutlierResult) -> Result<()> {
    let mut writer = csv::WriterBuilder::new()
        .buffer_capacity(1024 * 1024)
        .from_path(path)
        .with_context(|| format!("Could not create {}", path.display()))?;
    let mut header = vec![
        "sample_id".to_string(),
        "group".to_string(),
        "available_modalities".to_string(),
        "flagged_modalities".to_string(),
        "consensus_flag".to_string(),
        "flagged_in".to_string(),
    ];
    for modality in &result.modalities {
        let stem = modality
            .modality
            .to_ascii_lowercase()
            .replace(' ', "_")
            .replace('/', "_")
            .replace('-', "_");
        header.push(format!("{stem}_robust_distance"));
        header.push(format!("{stem}_flag"));
    }
    writer.write_record(&header)?;
    for sample in &result.samples {
        let mut row = vec![
            sample.sample_id.clone(),
            sample.group.clone(),
            sample.available_modalities.to_string(),
            sample.flagged_modalities.to_string(),
            sample.consensus_flag.to_string(),
            sample.flagged_names.join("; "),
        ];
        for modality in &result.modalities {
            let point = modality
                .points
                .iter()
                .find(|p| normalize_sample_id(&p.sample_id) == sample.sample_id);
            row.push(
                point
                    .map(|p| format!("{:.6}", p.robust_distance))
                    .unwrap_or_default(),
            );
            row.push(point.map(|p| p.flagged.to_string()).unwrap_or_default());
        }
        writer.write_record(&row)?;
    }
    writer.flush()?;
    Ok(())
}

pub fn filter_comparison_excluding(
    comparison: &CohortComparison,
    excluded_ids: &HashSet<String>,
) -> CohortComparison {
    if excluded_ids.is_empty() {
        return comparison.clone();
    }
    let canonical_excluded = excluded_ids
        .iter()
        .map(|id| comparison.canonical_id(id))
        .collect::<HashSet<_>>();
    let mut case_ids = comparison.case_ids.clone();
    let mut control_ids = comparison.control_ids.clone();
    case_ids.retain(|id| !canonical_excluded.contains(id));
    control_ids.retain(|id| !canonical_excluded.contains(id));
    let mut id_to_canonical = comparison.id_to_canonical.clone();
    id_to_canonical.retain(|_, canonical| !canonical_excluded.contains(canonical));
    CohortComparison {
        name: format!("{} — outlier-filtered", comparison.name),
        case_label: comparison.case_label.clone(),
        control_label: comparison.control_label.clone(),
        case_metadata_rows: case_ids.len(),
        control_metadata_rows: control_ids.len(),
        case_ids,
        control_ids,
        id_to_canonical,
    }
}

pub fn run_cross_modal_outlier_qc<F>(
    merged_gene_tsv: Option<&Path>,
    merged_transcript_tsv: Option<&Path>,
    files: &[DetectedCohortFile],
    comparison: &CohortComparison,
    selected_modalities: &[CohortModality],
    config: &OutlierConfig,
    mut progress: F,
) -> Result<CrossModalOutlierResult>
where
    F: FnMut(usize, usize, &str),
{
    let total = selected_modalities.len().max(1);
    let mut modalities = Vec::new();
    let mut warnings = Vec::new();
    for (idx, modality) in selected_modalities.iter().copied().enumerate() {
        progress(
            idx,
            total,
            &format!("Outlier QC: {}", modality.display_name()),
        );
        let built = match modality {
            CohortModality::GeneExpression => merged_gene_tsv
                .ok_or_else(|| anyhow!("No merged gene-expression TSV detected"))
                .and_then(|p| {
                    expression_embedding(
                        p,
                        comparison,
                        "Gene expression",
                        &[" TPM", " Raw Count"],
                        config,
                    )
                }),
            CohortModality::TranscriptExpression => merged_transcript_tsv
                .ok_or_else(|| anyhow!("No merged transcript-expression TSV detected"))
                .and_then(|p| {
                    expression_embedding(
                        p,
                        comparison,
                        "Transcript expression",
                        &[" TPM", " Total Reads"],
                        config,
                    )
                }),
            CohortModality::Bcr | CohortModality::TcrAlpha | CohortModality::TcrBeta => {
                repertoire_embedding(files, comparison, modality, config)
            }
            CohortModality::BcrIsotype => isotype_embedding(files, comparison, config),
            CohortModality::ImmuneInfiltrate => immune_embedding(files, comparison, config),
        };
        match built {
            Ok(result) => modalities.push(result),
            Err(e) => warnings.push(format!("{}: {e:#}", modality.display_name())),
        }
    }
    progress(total, total, "Combining modality outlier flags");
    if modalities.is_empty() {
        bail!("No selected modality could produce an outlier representation");
    }
    // Canonicalize modality-specific aliases before cross-modal consensus so
    // one biological sample is counted once even when upstream assays use
    // different IDs.
    for modality in &mut modalities {
        for point in &mut modality.points {
            point.sample_id = comparison.canonical_id(&point.sample_id);
        }
    }
    let mut by_sample: BTreeMap<String, (String, usize, Vec<String>)> = BTreeMap::new();
    for modality in &modalities {
        for point in &modality.points {
            let key = comparison.canonical_id(&point.sample_id);
            let entry = by_sample
                .entry(key)
                .or_insert((point.group.clone(), 0, Vec::new()));
            entry.1 += 1;
            if point.flagged {
                entry.2.push(modality.modality.clone());
            }
        }
    }
    let mut samples = Vec::new();
    let mut consensus_ids = Vec::new();
    for (sample_id, (group, available, mut flagged_names)) in by_sample {
        flagged_names.sort();
        let flagged = flagged_names.len();
        let consensus = consensus_flag(flagged, available, config);
        if consensus {
            consensus_ids.push(sample_id.clone());
        }
        samples.push(SampleOutlierSummary {
            sample_id,
            group,
            available_modalities: available,
            flagged_modalities: flagged,
            flagged_names,
            consensus_flag: consensus,
        });
    }
    samples.sort_by(|a, b| {
        b.consensus_flag
            .cmp(&a.consensus_flag)
            .then_with(|| b.flagged_modalities.cmp(&a.flagged_modalities))
            .then_with(|| a.sample_id.cmp(&b.sample_id))
    });
    consensus_ids.sort();
    Ok(CrossModalOutlierResult {
        comparison_name: comparison.name.clone(),
        config: config.clone(),
        modalities,
        samples,
        consensus_ids,
        warnings,
    })
}

fn consensus_flag(flagged: usize, available: usize, config: &OutlierConfig) -> bool {
    if available == 0 {
        return false;
    }
    match config.consensus_rule {
        OutlierConsensusRule::Any => flagged >= 1,
        OutlierConsensusRule::AtLeastN => {
            available >= config.min_available_modalities && flagged >= config.at_least_n.max(1)
        }
        OutlierConsensusRule::Majority => {
            available >= config.min_available_modalities && flagged * 2 > available
        }
        OutlierConsensusRule::AllAvailable => {
            available >= config.min_available_modalities && flagged == available
        }
    }
}

fn group_for_sample(sample: &str, comparison: &CohortComparison) -> Option<String> {
    let key = normalize_sample_id(sample);
    if comparison.is_case_id(&key) {
        Some(comparison.case_label.clone())
    } else if comparison.is_control_id(&key) {
        Some(comparison.control_label.clone())
    } else {
        None
    }
}

fn expression_embedding(
    path: &Path,
    comparison: &CohortComparison,
    label: &str,
    metric_suffixes: &[&str],
    config: &OutlierConfig,
) -> Result<ModalityOutlierResult> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("Could not open {}", path.display()))?;
    let headers = reader.headers()?.clone();
    let mut chosen_suffix = None;
    let mut sample_cols = Vec::new();
    for suffix in metric_suffixes {
        let mut cols = Vec::new();
        for (idx, header) in headers.iter().enumerate().skip(2) {
            if let Some(sample) = header.strip_suffix(suffix) {
                if let Some(group) = group_for_sample(sample, comparison) {
                    cols.push((sample.to_string(), group, idx));
                }
            }
        }
        if cols.len() >= 4 {
            chosen_suffix = Some(*suffix);
            sample_cols = cols;
            break;
        }
    }
    let suffix =
        chosen_suffix.ok_or_else(|| anyhow!("No usable sample columns found for {label}"))?;
    sample_cols.sort_by(|a, b| a.0.cmp(&b.0));
    let mut top: Vec<(f64, Vec<f64>)> = Vec::new();
    for record in reader.records() {
        let record = record?;
        let values = sample_cols
            .iter()
            .map(|(_, _, idx)| {
                record
                    .get(*idx)
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
            })
            .map(|v| (v.max(0.0) + 1.0).log2())
            .collect::<Vec<_>>();
        let var = sample_variance(&values);
        if var.is_finite() && var > 0.0 {
            top.push((var, values));
            if top.len() > config.expression_features.saturating_mul(2).max(200) {
                top.sort_by(|a, b| b.0.total_cmp(&a.0));
                top.truncate(config.expression_features.max(50));
            }
        }
    }
    top.sort_by(|a, b| b.0.total_cmp(&a.0));
    top.truncate(config.expression_features.max(50));
    let rows = top.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
    let sample_ids = sample_cols.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    let groups = sample_cols.iter().map(|x| x.1.clone()).collect::<Vec<_>>();
    let mut result = pca_modality(
        label,
        &rows,
        &sample_ids,
        &groups,
        &format!("Top-variable log2({}+1) feature PCA", suffix.trim()),
        config,
    )?;
    if suffix != " TPM" {
        result.warnings.push(format!(
            "TPM was unavailable; outlier PCA used '{}' columns instead.",
            suffix.trim()
        ));
    }
    Ok(result)
}

fn repertoire_embedding(
    files: &[DetectedCohortFile],
    comparison: &CohortComparison,
    modality: CohortModality,
    config: &OutlierConfig,
) -> Result<ModalityOutlierResult> {
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files.iter().filter(|f| f.modality == modality) {
        if group_for_sample(&file.sample_id, comparison).is_none() {
            continue;
        }
        let key = normalize_sample_id(&file.sample_id);
        if chosen.contains_key(&key) {
            warnings.push(format!(
                "Duplicate {} file for {}; first file used.",
                modality.display_name(),
                file.sample_id
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let mut metric_maps = Vec::new();
    for file in chosen.into_values() {
        match parse_repertoire_qc_metrics(&file.path, &file.sample_id, modality) {
            Ok(values) => metric_maps.push((
                file.sample_id.clone(),
                values.into_iter().collect::<HashMap<_, _>>(),
            )),
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if metric_maps.len() < 4 {
        bail!("Too few {} samples for outlier QC", modality.display_name());
    }
    metric_maps.sort_by(|a, b| a.0.cmp(&b.0));
    let feature_names = metric_maps
        .iter()
        .flat_map(|(_, m)| m.keys().cloned())
        .collect::<BTreeSet<_>>();
    let sample_ids = metric_maps.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    let groups = sample_ids
        .iter()
        .map(|s| group_for_sample(s, comparison).unwrap_or_default())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for feature in feature_names {
        let values = metric_maps
            .iter()
            .map(|(_, m)| m.get(&feature).copied().unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        if let Some(z) = robust_standardized_row(&values) {
            rows.push(z);
        }
    }
    let mut result = pca_modality(
        modality.display_name(),
        &rows,
        &sample_ids,
        &groups,
        "PCA of robust-standardized repertoire summary metrics",
        config,
    )?;
    result.warnings.extend(warnings);
    Ok(result)
}

fn isotype_embedding(
    files: &[DetectedCohortFile],
    comparison: &CohortComparison,
    config: &OutlierConfig,
) -> Result<ModalityOutlierResult> {
    let mut maps = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    for file in files
        .iter()
        .filter(|f| f.modality == CohortModality::BcrIsotype)
    {
        if group_for_sample(&file.sample_id, comparison).is_none() {
            continue;
        }
        let key = normalize_sample_id(&file.sample_id);
        if !seen.insert(key) {
            warnings.push(format!(
                "Duplicate BCR-isotype file for {}; first file used.",
                file.sample_id
            ));
            continue;
        }
        match parse_isotype_file(&file.path, false) {
            Ok(map) => maps.push((file.sample_id.clone(), map)),
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if maps.len() < 4 {
        bail!("Too few BCR-isotype samples for outlier QC");
    }
    maps.sort_by(|a, b| a.0.cmp(&b.0));
    let features = maps
        .iter()
        .flat_map(|(_, m)| m.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let sample_ids = maps.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    let groups = sample_ids
        .iter()
        .map(|s| group_for_sample(s, comparison).unwrap_or_default())
        .collect::<Vec<_>>();
    let mut sample_clr = Vec::new();
    for (_, map) in &maps {
        let logs = features
            .iter()
            .map(|f| (map.get(f).copied().unwrap_or(0.0) + 1e-6).ln())
            .collect::<Vec<_>>();
        let center = mean(&logs);
        sample_clr.push(logs.into_iter().map(|v| v - center).collect::<Vec<_>>());
    }
    let rows = (0..features.len())
        .map(|i| sample_clr.iter().map(|s| s[i]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut result = pca_modality(
        "BCR isotype",
        &rows,
        &sample_ids,
        &groups,
        "CLR isotype-composition PCA",
        config,
    )?;
    result.warnings.extend(warnings);
    Ok(result)
}

fn immune_embedding(
    files: &[DetectedCohortFile],
    comparison: &CohortComparison,
    config: &OutlierConfig,
) -> Result<ModalityOutlierResult> {
    let mut maps = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    for file in files
        .iter()
        .filter(|f| f.modality == CohortModality::ImmuneInfiltrate)
    {
        if group_for_sample(&file.sample_id, comparison).is_none() {
            continue;
        }
        let key = normalize_sample_id(&file.sample_id);
        if !seen.insert(key) {
            warnings.push(format!(
                "Duplicate immune-infiltrate file for {}; first file used.",
                file.sample_id
            ));
            continue;
        }
        match parse_immune_file(&file.path) {
            Ok(map) => maps.push((file.sample_id.clone(), map)),
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if maps.len() < 4 {
        bail!("Too few immune-infiltrate samples for outlier QC");
    }
    maps.sort_by(|a, b| a.0.cmp(&b.0));
    let features = maps
        .iter()
        .flat_map(|(_, m)| m.keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let sample_ids = maps.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    let groups = sample_ids
        .iter()
        .map(|s| group_for_sample(s, comparison).unwrap_or_default())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for feature in features {
        let values = maps
            .iter()
            .map(|(_, m)| m.get(&feature).copied().unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        if let Some(z) = robust_standardized_row(&values) {
            rows.push(z);
        }
    }
    let mut result = pca_modality(
        "Immune infiltrate",
        &rows,
        &sample_ids,
        &groups,
        "PCA of robust-standardized immune ssGSEA signatures",
        config,
    )?;
    result.warnings.extend(warnings);
    Ok(result)
}

fn pca_modality(
    label: &str,
    rows: &[Vec<f64>],
    sample_ids: &[String],
    groups: &[String],
    representation: &str,
    config: &OutlierConfig,
) -> Result<ModalityOutlierResult> {
    if rows.len() < 2 || sample_ids.len() < 4 || sample_ids.len() != groups.len() {
        bail!("Insufficient sample/features for {label} outlier PCA");
    }
    let n = sample_ids.len();
    let mut gram = vec![vec![0.0; n]; n];
    for row in rows {
        if row.len() != n {
            continue;
        }
        let m = mean(row);
        for i in 0..n {
            let ci = row[i] - m;
            for j in i..n {
                let value = ci * (row[j] - m);
                gram[i][j] += value;
                if i != j {
                    gram[j][i] += value;
                }
            }
        }
    }
    let (l1, v1, l2, v2) =
        top_two_eigen(&gram).ok_or_else(|| anyhow!("Could not calculate {label} PCA"))?;
    let trace = (0..n).map(|i| gram[i][i]).sum::<f64>().max(1e-12);
    let coords = (0..n)
        .map(|i| (v1[i] * l1.max(0.0).sqrt(), v2[i] * l2.max(0.0).sqrt()))
        .collect::<Vec<_>>();
    let scores = robust_scores(&coords, groups, config.scope);
    let points = sample_ids
        .iter()
        .enumerate()
        .map(|(i, sample)| ModalityOutlierPoint {
            sample_id: normalize_sample_id(sample),
            group: groups[i].clone(),
            x: coords[i].0,
            y: coords[i].1,
            robust_distance: scores[i],
            flagged: scores[i].is_finite() && scores[i] >= config.robust_distance_threshold,
        })
        .collect::<Vec<_>>();
    Ok(ModalityOutlierResult {
        modality: label.to_string(),
        x_label: format!("PC1 ({:.1}%)", 100.0 * l1.max(0.0) / trace),
        y_label: format!("PC2 ({:.1}%)", 100.0 * l2.max(0.0) / trace),
        representation: representation.to_string(),
        points,
        warnings: Vec::new(),
    })
}

fn robust_scores(coords: &[(f64, f64)], groups: &[String], scope: OutlierScope) -> Vec<f64> {
    let mut scores = vec![f64::NAN; coords.len()];
    let strata = match scope {
        OutlierScope::AcrossAll => vec![("all".to_string(), (0..coords.len()).collect::<Vec<_>>())],
        OutlierScope::WithinGroup => {
            let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for (i, group) in groups.iter().enumerate() {
                map.entry(group.clone()).or_default().push(i);
            }
            map.into_iter().collect::<Vec<_>>()
        }
    };
    for (_, idxs) in strata {
        let use_idxs = if idxs.len() >= 5 {
            idxs
        } else {
            (0..coords.len()).collect::<Vec<_>>()
        };
        let xs = use_idxs.iter().map(|i| coords[*i].0).collect::<Vec<_>>();
        let ys = use_idxs.iter().map(|i| coords[*i].1).collect::<Vec<_>>();
        let mx = median(&xs);
        let my = median(&ys);
        let sx = robust_scale(&xs, mx);
        let sy = robust_scale(&ys, my);
        for i in use_idxs {
            let zx = if sx > 1e-12 {
                (coords[i].0 - mx) / sx
            } else {
                0.0
            };
            let zy = if sy > 1e-12 {
                (coords[i].1 - my) / sy
            } else {
                0.0
            };
            scores[i] = (zx * zx + zy * zy).sqrt();
        }
    }
    scores
}

fn robust_standardized_row(values: &[f64]) -> Option<Vec<f64>> {
    let m = median(values);
    if !m.is_finite() {
        return None;
    }
    let scale = robust_scale(values, m);
    if !scale.is_finite() || scale <= 1e-12 {
        return None;
    }
    Some(
        values
            .iter()
            .map(|v| if v.is_finite() { (v - m) / scale } else { 0.0 })
            .collect(),
    )
}

fn mean(values: &[f64]) -> f64 {
    let finite = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        f64::NAN
    } else {
        finite.iter().sum::<f64>() / finite.len() as f64
    }
}

fn median(values: &[f64]) -> f64 {
    let mut v = values
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .collect::<Vec<_>>();
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    if v.len() % 2 == 1 {
        v[v.len() / 2]
    } else {
        (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0
    }
}

fn robust_scale(values: &[f64], center: f64) -> f64 {
    let dev = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| (v - center).abs())
        .collect::<Vec<_>>();
    let mad = median(&dev);
    if mad.is_finite() && mad > 1e-12 {
        1.4826 * mad
    } else {
        let sd = sample_variance(values).sqrt();
        if sd.is_finite() && sd > 1e-12 {
            sd
        } else {
            1.0
        }
    }
}

fn sample_variance(values: &[f64]) -> f64 {
    let finite = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect::<Vec<_>>();
    if finite.len() < 2 {
        return f64::NAN;
    }
    let m = finite.iter().sum::<f64>() / finite.len() as f64;
    finite.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (finite.len() - 1) as f64
}

fn top_two_eigen(a: &[Vec<f64>]) -> Option<(f64, Vec<f64>, f64, Vec<f64>)> {
    let n = a.len();
    if n == 0 {
        return None;
    }
    let mut v1 = (0..n)
        .map(|i| ((i + 1) as f64 * 1.61803398875).sin() + 0.1)
        .collect::<Vec<_>>();
    normalize_vec(&mut v1);
    for _ in 0..160 {
        let mut w = mat_vec_dense(a, &v1);
        normalize_vec(&mut w);
        if vec_distance(&w, &v1) < 1e-10 {
            v1 = w;
            break;
        }
        v1 = w;
    }
    let l1 = dot(&v1, &mat_vec_dense(a, &v1));
    let mut v2 = (0..n)
        .map(|i| ((i + 3) as f64 * 0.754877666).cos() + 0.2)
        .collect::<Vec<_>>();
    orthogonalize(&mut v2, &v1);
    normalize_vec(&mut v2);
    for _ in 0..180 {
        let av = mat_vec_dense(a, &v2);
        let proj = dot(&v1, &av);
        let mut w = av
            .iter()
            .zip(v1.iter())
            .map(|(x, u)| x - proj * u)
            .collect::<Vec<_>>();
        normalize_vec(&mut w);
        if vec_distance(&w, &v2) < 1e-10 {
            v2 = w;
            break;
        }
        v2 = w;
    }
    let l2 = dot(&v2, &mat_vec_dense(a, &v2));
    Some((l1, v1, l2, v2))
}

fn mat_vec_dense(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    a.iter()
        .map(|row| row.iter().zip(x.iter()).map(|(v, u)| v * u).sum())
        .collect()
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
fn normalize_vec(v: &mut [f64]) {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for x in v {
            *x /= norm;
        }
    }
}
fn orthogonalize(v: &mut [f64], basis: &[f64]) {
    let p = dot(v, basis);
    for (i, x) in v.iter_mut().enumerate() {
        *x -= p * basis[i];
    }
}
fn vec_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
        .min(
            a.iter()
                .zip(b)
                .map(|(x, y)| (x + y).powi(2))
                .sum::<f64>()
                .sqrt(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consensus_rules_behave_as_expected() {
        let mut cfg = OutlierConfig::default();
        cfg.min_available_modalities = 2;
        cfg.consensus_rule = OutlierConsensusRule::Any;
        assert!(consensus_flag(1, 4, &cfg));
        cfg.consensus_rule = OutlierConsensusRule::AtLeastN;
        cfg.at_least_n = 2;
        assert!(!consensus_flag(1, 4, &cfg));
        assert!(consensus_flag(2, 4, &cfg));
        cfg.consensus_rule = OutlierConsensusRule::Majority;
        assert!(!consensus_flag(2, 4, &cfg));
        assert!(consensus_flag(3, 4, &cfg));
        cfg.consensus_rule = OutlierConsensusRule::AllAvailable;
        assert!(!consensus_flag(3, 4, &cfg));
        assert!(consensus_flag(4, 4, &cfg));
    }

    #[test]
    fn robust_distance_flags_extreme_point() {
        let mut coords = (0..12)
            .map(|i| (i as f64 * 0.02, -(i as f64) * 0.01))
            .collect::<Vec<_>>();
        coords.push((20.0, 20.0));
        let groups = vec!["Case".to_string(); coords.len()];
        let scores = robust_scores(&coords, &groups, OutlierScope::AcrossAll);
        assert!(scores.last().copied().unwrap_or(0.0) > 4.0);
    }

    #[test]
    fn alias_exclusion_removes_the_canonical_sample() {
        let original = CohortComparison {
            name: "Case vs Control".to_string(),
            case_label: "Case".to_string(),
            control_label: "Control".to_string(),
            case_ids: ["S1".to_string(), "S2".to_string()].into_iter().collect(),
            control_ids: ["S3".to_string(), "S4".to_string()].into_iter().collect(),
            id_to_canonical: [
                ("S1".to_string(), "S1".to_string()),
                ("RNA1".to_string(), "S1".to_string()),
                ("S2".to_string(), "S2".to_string()),
                ("S3".to_string(), "S3".to_string()),
                ("S4".to_string(), "S4".to_string()),
            ]
            .into_iter()
            .collect(),
            case_metadata_rows: 2,
            control_metadata_rows: 2,
        };
        let reviewed = ["RNA1".to_string()].into_iter().collect::<HashSet<_>>();
        let filtered = filter_comparison_excluding(&original, &reviewed);
        assert!(!filtered.case_ids.contains("S1"));
        assert_eq!(filtered.case_ids.len(), 1);
        assert!(!filtered.is_case_id("RNA1"));
    }

    #[test]
    fn filter_snapshot_records_exact_reviewed_exclusions() {
        let case_ids = ["A".to_string(), "B".to_string()].into_iter().collect();
        let control_ids = ["C".to_string(), "D".to_string()].into_iter().collect();
        let original = CohortComparison {
            name: "Case vs Control".to_string(),
            case_label: "Case".to_string(),
            control_label: "Control".to_string(),
            case_ids,
            control_ids,
            id_to_canonical: HashMap::new(),
            case_metadata_rows: 2,
            control_metadata_rows: 2,
        };
        let reviewed = ["B".to_string()].into_iter().collect::<HashSet<_>>();
        let filtered = filter_comparison_excluding(&original, &reviewed);
        let audit = CrossModalOutlierResult {
            comparison_name: original.name.clone(),
            config: OutlierConfig::default(),
            modalities: vec![ModalityOutlierResult {
                modality: "Gene expression".to_string(),
                x_label: "PC1".to_string(),
                y_label: "PC2".to_string(),
                representation: "PCA".to_string(),
                points: vec![ModalityOutlierPoint {
                    sample_id: "B".to_string(),
                    group: "Case".to_string(),
                    x: 4.0,
                    y: 0.0,
                    robust_distance: 5.0,
                    flagged: true,
                }],
                warnings: Vec::new(),
            }],
            samples: vec![SampleOutlierSummary {
                sample_id: "B".to_string(),
                group: "Case".to_string(),
                available_modalities: 1,
                flagged_modalities: 1,
                flagged_names: vec!["Gene expression".to_string()],
                consensus_flag: true,
            }],
            consensus_ids: vec!["B".to_string()],
            warnings: Vec::new(),
        };
        let snapshot = build_outlier_filter_snapshot(&audit, &reviewed, &original, &filtered, true);
        assert_eq!(snapshot.excluded_ids, vec!["B".to_string()]);
        assert_eq!(snapshot.original_case_n, 2);
        assert_eq!(snapshot.filtered_case_n, 1);
        assert!(snapshot.applied_to_analysis);
    }
}
