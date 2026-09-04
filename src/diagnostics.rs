//! PBMC/source-composition diagnostics and candidate robustness checks.
//!
//! This module is intentionally diagnostic rather than a fourth biomarker model.
//! It helps identify sample-source, cell-composition, low-count, and individual-
//! sample effects that can make an otherwise convincing cohort comparison unstable.

use crate::cohort::{
    CohortComparison, CohortModality, DetectedCohortFile, GeneComparisonResult, MetadataTable,
    normalize_sample_id, run_gene_comparison, write_feature_tests,
};
use crate::corrected::{
    CorrectedExpressionResult, CorrectedModelSpec, OrdinationPoint, OrdinationResult,
    parse_immune_file, run_corrected_expression, write_adjusted_tests, write_ordination,
};
use crate::metadata::row_matching_ids;
use anyhow::{Context, Result, bail};
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PbmcDiagnosticsConfig {
    pub subject_column: Option<String>,
    pub batch_column: Option<String>,
    pub family_column: Option<String>,
    pub sensitivity_column: Option<String>,
    pub known_non_pbmc_ids: HashSet<String>,
    pub suspect_ids: HashSet<String>,
    pub exclude_suspects_in_sensitivity: bool,
    pub auto_exclude_transcriptome_outliers: bool,
    pub auto_exclude_immune_outliers: bool,
    pub top_candidates: usize,
    pub pca_features: usize,
    pub outlier_z_threshold: f64,
    pub low_count_threshold: f64,
    pub low_count_min_fraction: f64,
    pub top2_contribution_threshold: f64,
}

impl Default for PbmcDiagnosticsConfig {
    fn default() -> Self {
        Self {
            subject_column: None,
            batch_column: None,
            family_column: None,
            sensitivity_column: None,
            known_non_pbmc_ids: HashSet::new(),
            suspect_ids: HashSet::new(),
            exclude_suspects_in_sensitivity: true,
            auto_exclude_transcriptome_outliers: false,
            auto_exclude_immune_outliers: false,
            top_candidates: 50,
            pca_features: 300,
            outlier_z_threshold: 4.0,
            low_count_threshold: 10.0,
            low_count_min_fraction: 0.20,
            top2_contribution_threshold: 0.50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PbmcSampleQcRow {
    pub sample_id: String,
    pub comparison_group: String,
    pub phenotype: String,
    pub hyp: String,
    pub subject: String,
    pub batch: String,
    pub family: String,
    pub library_raw_count: f64,
    pub detected_genes: usize,
    pub genes_ge5: usize,
    pub genes_ge10: usize,
    pub pca1: f64,
    pub pca2: f64,
    pub transcriptome_outlier_score: f64,
    pub immune_pc1: f64,
    pub immune_pc2: f64,
    pub immune_outlier_score: f64,
    pub monocyte_lymphoid_signature: f64,
    pub known_non_pbmc: bool,
    pub suspect: bool,
    pub auto_expression_outlier: bool,
    pub auto_immune_outlier: bool,
    pub sensitivity_excluded: bool,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CandidateSamplePoint {
    pub sample_id: String,
    pub group: String,
    pub raw_count: f64,
    pub tpm: f64,
    pub suspect: bool,
    pub sensitivity_excluded: bool,
    pub subject: String,
    pub batch: String,
    pub family: String,
    pub sensitivity_level: String,
}

#[derive(Debug, Clone)]
pub struct CandidateStabilityRow {
    pub gene: String,
    pub gene_id: String,
    pub baseline_log2fc: f64,
    pub baseline_q: f64,
    pub regularized_log2fc: f64,
    pub case_median_raw: f64,
    pub control_median_raw: f64,
    pub case_median_tpm: f64,
    pub control_median_tpm: f64,
    pub detected_case: f64,
    pub detected_control: f64,
    pub count_ge5_case: f64,
    pub count_ge5_control: f64,
    pub count_ge10_case: f64,
    pub count_ge10_control: f64,
    pub top2_contribution_case: f64,
    pub top2_contribution_control: f64,
    pub loo_log2fc_min: f64,
    pub loo_log2fc_max: f64,
    pub leave_level_log2fc_min: f64,
    pub leave_level_log2fc_max: f64,
    pub qc_restricted_log2fc: f64,
    pub qc_restricted_q: f64,
    pub immune_adjusted_log2fc: f64,
    pub immune_adjusted_q: f64,
    pub source_effect_log2fc: f64,
    pub source_effect_q: f64,
    pub classification: String,
    pub flags: Vec<String>,
    pub sample_points: Vec<CandidateSamplePoint>,
}

#[derive(Debug, Clone)]
pub struct LongitudinalQcRow {
    pub subject: String,
    pub n_samples: usize,
    pub max_pca_distance: f64,
    pub samples: String,
    pub batches: String,
    pub flagged_samples: String,
}

#[derive(Debug, Clone)]
pub struct PbmcDiagnosticsResult {
    pub merged_gene_tsv: PathBuf,
    pub comparison: CohortComparison,
    pub config: PbmcDiagnosticsConfig,
    pub pca: Option<OrdinationResult>,
    pub sample_qc: Vec<PbmcSampleQcRow>,
    pub longitudinal_qc: Vec<LongitudinalQcRow>,
    pub source_effect: Option<GeneComparisonResult>,
    pub baseline_gene: Option<CorrectedExpressionResult>,
    pub qc_restricted_gene: Option<CorrectedExpressionResult>,
    pub immune_adjusted_gene: Option<CorrectedExpressionResult>,
    pub candidate_stability: Vec<CandidateStabilityRow>,
    pub sensitivity_excluded_ids: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct SampleColumns {
    sample_id: String,
    raw_idx: usize,
    tpm_idx: usize,
}

#[derive(Debug, Clone)]
struct ExpressionScan {
    columns: Vec<SampleColumns>,
    library_counts: Vec<f64>,
    detected: Vec<usize>,
    ge5: Vec<usize>,
    ge10: Vec<usize>,
    top_variable_rows: Vec<Vec<f64>>,
}

#[derive(Debug, Clone)]
struct CandidateValues {
    gene_id: String,
    raw: Vec<f64>,
    tpm: Vec<f64>,
}

#[derive(Debug, Clone)]
struct ImmuneDiagnostics {
    pc_by_sample: HashMap<String, (f64, f64)>,
    outlier_by_sample: HashMap<String, f64>,
    mono_lymph_by_sample: HashMap<String, f64>,
    pc_covariates: HashMap<String, (f64, f64)>,
    warnings: Vec<String>,
}

pub fn run_pbmc_diagnostics<F>(
    merged_gene_tsv: &Path,
    files: &[DetectedCohortFile],
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    corrected_spec: &CorrectedModelSpec,
    config: &PbmcDiagnosticsConfig,
    mut progress: F,
) -> Result<PbmcDiagnosticsResult>
where
    F: FnMut(usize, usize, &str),
{
    let mut warnings = Vec::new();
    progress(0, 8, "Scanning merged gene matrix for sample-level QC");
    let scan = scan_expression_qc(merged_gene_tsv, config.pca_features)?;

    progress(1, 8, "Building whole-transcriptome PCA");
    let pca = pca_from_rows(
        &scan.top_variable_rows,
        &scan
            .columns
            .iter()
            .map(|c| c.sample_id.clone())
            .collect::<Vec<_>>(),
        metadata,
        comparison,
        config.subject_column.as_deref(),
        "Whole-transcriptome QC PCA — top variable log2(TPM+1) genes",
    );

    progress(2, 8, "Reading immune-infiltrate signatures");
    let immune = build_immune_diagnostics(files, &scan.columns, config.outlier_z_threshold);
    warnings.extend(immune.warnings.clone());

    let metadata_index = unique_metadata_rows(metadata);
    let pca_scores = ordination_outlier_scores(pca.as_ref(), config.outlier_z_threshold);
    let mut sample_qc = Vec::with_capacity(scan.columns.len());
    let mut sensitivity_excluded = config.known_non_pbmc_ids.clone();
    if config.exclude_suspects_in_sensitivity {
        sensitivity_excluded.extend(config.suspect_ids.iter().cloned());
    }

    for (i, col) in scan.columns.iter().enumerate() {
        let key = normalize_sample_id(&col.sample_id);
        let meta_row = metadata_index.get(&key).copied();
        let phenotype = metadata_value(metadata, meta_row, "Phenotype");
        let hyp = metadata_value(metadata, meta_row, "HYP");
        let subject = config
            .subject_column
            .as_deref()
            .map(|c| metadata_value(metadata, meta_row, c))
            .unwrap_or_default();
        let batch = config
            .batch_column
            .as_deref()
            .map(|c| metadata_value(metadata, meta_row, c))
            .unwrap_or_default();
        let family = config
            .family_column
            .as_deref()
            .map(|c| metadata_value(metadata, meta_row, c))
            .unwrap_or_default();
        let group = if comparison.is_case_id(&key) {
            comparison.case_label.clone()
        } else if comparison.is_control_id(&key) {
            comparison.control_label.clone()
        } else {
            "Outside comparison".to_string()
        };
        let (pc1, pc2) = pca
            .as_ref()
            .and_then(|o| {
                o.points
                    .iter()
                    .find(|p| normalize_sample_id(&p.sample_id) == key)
            })
            .map(|p| (p.x, p.y))
            .unwrap_or((f64::NAN, f64::NAN));
        let transcriptome_score = pca_scores.get(&key).copied().unwrap_or(f64::NAN);
        let immune_pc = immune
            .pc_by_sample
            .get(&key)
            .copied()
            .unwrap_or((f64::NAN, f64::NAN));
        let immune_score = immune
            .outlier_by_sample
            .get(&key)
            .copied()
            .unwrap_or(f64::NAN);
        let mono_lymph = immune
            .mono_lymph_by_sample
            .get(&key)
            .copied()
            .unwrap_or(f64::NAN);
        let known_non_pbmc = config.known_non_pbmc_ids.contains(&key);
        let suspect = config.suspect_ids.contains(&key);
        let auto_expression_outlier =
            transcriptome_score.is_finite() && transcriptome_score >= config.outlier_z_threshold;
        let auto_immune_outlier =
            immune_score.is_finite() && immune_score >= config.outlier_z_threshold;
        if config.auto_exclude_transcriptome_outliers && auto_expression_outlier {
            sensitivity_excluded.insert(key.clone());
        }
        if config.auto_exclude_immune_outliers && auto_immune_outlier {
            sensitivity_excluded.insert(key.clone());
        }
        let mut flags = Vec::new();
        if known_non_pbmc {
            flags.push("known non-PBMC / wrong source".to_string());
        }
        if suspect {
            flags.push("user-marked suspect".to_string());
        }
        if auto_expression_outlier {
            flags.push("whole-transcriptome PCA outlier".to_string());
        }
        if auto_immune_outlier {
            flags.push("immune-signature PCA outlier".to_string());
        }
        if mono_lymph.is_finite() && mono_lymph > 2.5 {
            flags.push("myeloid-high / lymphoid-low signature pattern".to_string());
        }
        sample_qc.push(PbmcSampleQcRow {
            sample_id: col.sample_id.clone(),
            comparison_group: group,
            phenotype,
            hyp,
            subject,
            batch,
            family,
            library_raw_count: scan.library_counts[i],
            detected_genes: scan.detected[i],
            genes_ge5: scan.ge5[i],
            genes_ge10: scan.ge10[i],
            pca1: pc1,
            pca2: pc2,
            transcriptome_outlier_score: transcriptome_score,
            immune_pc1: immune_pc.0,
            immune_pc2: immune_pc.1,
            immune_outlier_score: immune_score,
            monocyte_lymphoid_signature: mono_lymph,
            known_non_pbmc,
            suspect,
            auto_expression_outlier,
            auto_immune_outlier,
            sensitivity_excluded: false,
            flags,
        });
    }
    for row in &mut sample_qc {
        row.sensitivity_excluded =
            sensitivity_excluded.contains(&normalize_sample_id(&row.sample_id));
    }

    progress(3, 8, "Running baseline corrected gene analysis");
    let baseline_gene = match run_corrected_expression(
        merged_gene_tsv,
        metadata,
        comparison,
        "Raw Count",
        "Gene",
        corrected_spec,
        |_done, _total, _msg| {},
    ) {
        Ok(r) => Some(r),
        Err(e) => {
            warnings.push(format!("Baseline corrected gene analysis failed: {e:#}"));
            None
        }
    };

    progress(4, 8, "Running QC-restricted sensitivity analysis");
    let filtered_metadata = filter_metadata(metadata, &sensitivity_excluded)?;
    let qc_restricted_gene = match run_corrected_expression(
        merged_gene_tsv,
        &filtered_metadata,
        comparison,
        "Raw Count",
        "Gene",
        corrected_spec,
        |_done, _total, _msg| {},
    ) {
        Ok(r) => Some(r),
        Err(e) => {
            warnings.push(format!(
                "QC-restricted gene analysis could not be completed: {e:#}"
            ));
            None
        }
    };

    progress(
        5,
        8,
        "Running immune-composition-adjusted sensitivity analysis",
    );
    let immune_adjusted_gene = if immune.pc_covariates.len() >= 6 {
        match augment_metadata_with_immune_pcs(&filtered_metadata, &immune.pc_covariates) {
            Ok(augmented) => {
                let mut spec = corrected_spec.clone();
                if !spec.covariates.iter().any(|c| c == "QC_Immune_PC1") {
                    spec.covariates.push("QC_Immune_PC1".to_string());
                }
                if !spec.covariates.iter().any(|c| c == "QC_Immune_PC2") {
                    spec.covariates.push("QC_Immune_PC2".to_string());
                }
                match run_corrected_expression(
                    merged_gene_tsv,
                    &augmented,
                    comparison,
                    "Raw Count",
                    "Gene",
                    &spec,
                    |_done, _total, _msg| {},
                ) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        warnings.push(format!(
                            "Immune-PC-adjusted gene analysis could not be completed: {e:#}"
                        ));
                        None
                    }
                }
            }
            Err(e) => {
                warnings.push(format!("Could not add immune PC covariates: {e:#}"));
                None
            }
        }
    } else {
        warnings.push("Too few immune-infiltrate samples were available to run immune-PC-adjusted gene models.".to_string());
        None
    };

    progress(
        6,
        8,
        "Testing user-marked unusual samples against ordinary samples",
    );
    let source_effect = build_source_effect_screen(
        merged_gene_tsv,
        &scan.columns,
        &config.suspect_ids,
        &config.known_non_pbmc_ids,
    )
    .unwrap_or_else(|e| {
        warnings.push(format!("Source-effect screen was skipped: {e:#}"));
        None
    });

    progress(7, 8, "Building candidate robustness diagnostics");
    let candidate_stability = if let Some(base) = &baseline_gene {
        build_candidate_stability(
            merged_gene_tsv,
            &scan.columns,
            metadata,
            comparison,
            config,
            base,
            qc_restricted_gene.as_ref(),
            immune_adjusted_gene.as_ref(),
            source_effect.as_ref(),
            &sample_qc,
        )?
    } else {
        Vec::new()
    };
    let longitudinal_qc = build_longitudinal_qc(&sample_qc);
    let mut excluded_ids = sensitivity_excluded.into_iter().collect::<Vec<_>>();
    excluded_ids.sort();
    progress(8, 8, "PBMC/confounding diagnostics complete");

    Ok(PbmcDiagnosticsResult {
        merged_gene_tsv: merged_gene_tsv.to_path_buf(),
        comparison: comparison.clone(),
        config: config.clone(),
        pca,
        sample_qc,
        longitudinal_qc,
        source_effect,
        baseline_gene,
        qc_restricted_gene,
        immune_adjusted_gene,
        candidate_stability,
        sensitivity_excluded_ids: excluded_ids,
        warnings,
    })
}

fn scan_expression_qc(path: &Path, pca_features: usize) -> Result<ExpressionScan> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(path)
        .with_context(|| format!("Could not open merged expression table {}", path.display()))?;
    let headers = reader.headers()?.clone();
    let columns = expression_columns(&headers)?;
    if columns.len() < 3 {
        bail!("Merged gene table has fewer than three samples with both Raw Count and TPM");
    }
    let n = columns.len();
    let mut library_counts = vec![0.0; n];
    let mut detected = vec![0usize; n];
    let mut ge5 = vec![0usize; n];
    let mut ge10 = vec![0usize; n];
    let mut variable: Vec<(f64, Vec<f64>)> = Vec::new();
    let keep = pca_features.max(50);
    for record in reader.records() {
        let record = record?;
        let mut log_tpm = Vec::with_capacity(n);
        for (i, col) in columns.iter().enumerate() {
            let raw = record
                .get(col.raw_idx)
                .and_then(parse_f64)
                .unwrap_or(0.0)
                .max(0.0);
            let tpm = record
                .get(col.tpm_idx)
                .and_then(parse_f64)
                .unwrap_or(0.0)
                .max(0.0);
            library_counts[i] += raw;
            if raw > 0.0 {
                detected[i] += 1;
            }
            if raw >= 5.0 {
                ge5[i] += 1;
            }
            if raw >= 10.0 {
                ge10[i] += 1;
            }
            log_tpm.push((tpm + 1.0).log2());
        }
        let var = sample_variance(&log_tpm);
        if var.is_finite() {
            variable.push((var, log_tpm));
            if variable.len() > keep * 3 {
                variable.sort_by(|a, b| b.0.total_cmp(&a.0));
                variable.truncate(keep * 2);
            }
        }
    }
    variable.sort_by(|a, b| b.0.total_cmp(&a.0));
    variable.truncate(keep);
    Ok(ExpressionScan {
        columns,
        library_counts,
        detected,
        ge5,
        ge10,
        top_variable_rows: variable.into_iter().map(|(_, row)| row).collect(),
    })
}

fn expression_columns(headers: &StringRecord) -> Result<Vec<SampleColumns>> {
    let mut map: BTreeMap<String, (String, Option<usize>, Option<usize>)> = BTreeMap::new();
    for (idx, header) in headers.iter().enumerate().skip(2) {
        if let Some(sample) = header.strip_suffix(" Raw Count") {
            let key = normalize_sample_id(sample);
            let entry = map
                .entry(key)
                .or_insert_with(|| (sample.to_string(), None, None));
            entry.1 = Some(idx);
        } else if let Some(sample) = header.strip_suffix(" TPM") {
            let key = normalize_sample_id(sample);
            let entry = map
                .entry(key)
                .or_insert_with(|| (sample.to_string(), None, None));
            entry.2 = Some(idx);
        }
    }
    let columns = map
        .into_values()
        .filter_map(|(sample_id, raw, tpm)| {
            Some(SampleColumns {
                sample_id,
                raw_idx: raw?,
                tpm_idx: tpm?,
            })
        })
        .collect::<Vec<_>>();
    if columns.is_empty() {
        bail!("No paired '<sample> Raw Count' and '<sample> TPM' columns were found");
    }
    Ok(columns)
}

fn pca_from_rows(
    rows: &[Vec<f64>],
    sample_ids: &[String],
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    subject_column: Option<&str>,
    title: &str,
) -> Option<OrdinationResult> {
    if rows.len() < 2 || sample_ids.len() < 3 {
        return None;
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
    let (l1, v1, l2, v2) = top_two_eigen(&gram)?;
    let trace = (0..n).map(|i| gram[i][i]).sum::<f64>().max(1e-12);
    let meta_index = unique_metadata_rows(metadata);
    let points = sample_ids
        .iter()
        .enumerate()
        .map(|(i, sample)| {
            let key = normalize_sample_id(sample);
            let group = if comparison.is_case_id(&key) {
                "Case"
            } else if comparison.is_control_id(&key) {
                "Control"
            } else {
                "Outside comparison"
            };
            let subject = meta_index
                .get(&key)
                .copied()
                .and_then(|row| subject_column.map(|c| metadata_value(metadata, Some(row), c)))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| sample.clone());
            OrdinationPoint {
                sample_id: sample.clone(),
                group: group.to_string(),
                subject,
                x: v1[i] * l1.max(0.0).sqrt(),
                y: v2[i] * l2.max(0.0).sqrt(),
            }
        })
        .collect::<Vec<_>>();
    Some(OrdinationResult {
        name: title.to_string(),
        x_label: format!("PC1 ({:.1}%)", 100.0 * l1.max(0.0) / trace),
        y_label: format!("PC2 ({:.1}%)", 100.0 * l2.max(0.0) / trace),
        points,
        permanova_f: f64::NAN,
        permanova_p: f64::NAN,
        permutations: 0,
    })
}

fn ordination_outlier_scores(
    ord: Option<&OrdinationResult>,
    threshold: f64,
) -> HashMap<String, f64> {
    let Some(ord) = ord else {
        return HashMap::new();
    };
    let xs = ord.points.iter().map(|p| p.x).collect::<Vec<_>>();
    let ys = ord.points.iter().map(|p| p.y).collect::<Vec<_>>();
    let mx = median(&xs);
    let my = median(&ys);
    let sx = robust_scale(&xs, mx);
    let sy = robust_scale(&ys, my);
    let mut out = HashMap::new();
    for p in &ord.points {
        let zx = if sx > 1e-12 { (p.x - mx) / sx } else { 0.0 };
        let zy = if sy > 1e-12 { (p.y - my) / sy } else { 0.0 };
        let score = (zx * zx + zy * zy).sqrt();
        let _ = threshold;
        out.insert(normalize_sample_id(&p.sample_id), score);
    }
    out
}

fn build_immune_diagnostics(
    files: &[DetectedCohortFile],
    expression_columns: &[SampleColumns],
    outlier_threshold: f64,
) -> ImmuneDiagnostics {
    let expression_ids = expression_columns
        .iter()
        .map(|c| normalize_sample_id(&c.sample_id))
        .collect::<HashSet<_>>();
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files
        .iter()
        .filter(|f| f.modality == CohortModality::ImmuneInfiltrate)
    {
        let key = normalize_sample_id(&file.sample_id);
        if !expression_ids.contains(&key) {
            continue;
        }
        if chosen.contains_key(&key) {
            warnings.push(format!("Duplicate immune-infiltrate file for '{}'; the first file is used for diagnostics.", file.sample_id));
        } else {
            chosen.insert(key, file);
        }
    }
    let mut maps = BTreeMap::new();
    for (key, file) in chosen {
        match parse_immune_file(&file.path) {
            Ok(map) => {
                maps.insert(key, map);
            }
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if maps.len() < 3 {
        return ImmuneDiagnostics {
            pc_by_sample: HashMap::new(),
            outlier_by_sample: HashMap::new(),
            mono_lymph_by_sample: HashMap::new(),
            pc_covariates: HashMap::new(),
            warnings,
        };
    }
    let sample_ids = maps.keys().cloned().collect::<Vec<_>>();
    let features = maps
        .values()
        .flat_map(|m| m.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut zrows = Vec::new();
    let mut labels = Vec::new();
    for feature in features {
        let vals = sample_ids
            .iter()
            .map(|id| {
                maps.get(id)
                    .and_then(|m| m.get(&feature))
                    .copied()
                    .unwrap_or(f64::NAN)
            })
            .collect::<Vec<_>>();
        let finite = vals
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if finite.len() < 3 {
            continue;
        }
        let m = mean(&finite);
        let sd = sample_variance(&finite).sqrt();
        if !sd.is_finite() || sd <= 1e-12 {
            continue;
        }
        zrows.push(
            vals.iter()
                .map(|v| if v.is_finite() { (v - m) / sd } else { 0.0 })
                .collect::<Vec<_>>(),
        );
        labels.push(feature);
    }
    if zrows.len() < 2 {
        return ImmuneDiagnostics {
            pc_by_sample: HashMap::new(),
            outlier_by_sample: HashMap::new(),
            mono_lymph_by_sample: HashMap::new(),
            pc_covariates: HashMap::new(),
            warnings,
        };
    }
    let mut gram = vec![vec![0.0; sample_ids.len()]; sample_ids.len()];
    for row in &zrows {
        let m = mean(row);
        for i in 0..sample_ids.len() {
            let ci = row[i] - m;
            for j in i..sample_ids.len() {
                let v = ci * (row[j] - m);
                gram[i][j] += v;
                if i != j {
                    gram[j][i] += v;
                }
            }
        }
    }
    let mut pc_by_sample = HashMap::new();
    if let Some((l1, v1, l2, v2)) = top_two_eigen(&gram) {
        for (i, id) in sample_ids.iter().enumerate() {
            pc_by_sample.insert(
                id.clone(),
                (v1[i] * l1.max(0.0).sqrt(), v2[i] * l2.max(0.0).sqrt()),
            );
        }
    }
    let xvals = pc_by_sample.values().map(|p| p.0).collect::<Vec<_>>();
    let yvals = pc_by_sample.values().map(|p| p.1).collect::<Vec<_>>();
    let mx = median(&xvals);
    let my = median(&yvals);
    let sx = robust_scale(&xvals, mx);
    let sy = robust_scale(&yvals, my);
    let mut outlier_by_sample = HashMap::new();
    for (id, (x, y)) in &pc_by_sample {
        let zx = if sx > 1e-12 { (x - mx) / sx } else { 0.0 };
        let zy = if sy > 1e-12 { (y - my) / sy } else { 0.0 };
        outlier_by_sample.insert(id.clone(), (zx * zx + zy * zy).sqrt());
    }
    let myeloid_rows = labels
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let x = l.to_ascii_lowercase();
            x.contains("monocyte") || x.contains("macrophage") || x.contains("myeloid")
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let lymphoid_rows = labels
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let x = l.to_ascii_lowercase();
            x.contains("b-cell")
                || x.contains("b cell")
                || x.contains("t-cell")
                || x.contains("t cell")
                || x.contains("nk")
                || x.contains("lymph")
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let mut mono_lymph_by_sample = HashMap::new();
    if !myeloid_rows.is_empty() && !lymphoid_rows.is_empty() {
        for (j, id) in sample_ids.iter().enumerate() {
            let myeloid =
                myeloid_rows.iter().map(|&i| zrows[i][j]).sum::<f64>() / myeloid_rows.len() as f64;
            let lymphoid = lymphoid_rows.iter().map(|&i| zrows[i][j]).sum::<f64>()
                / lymphoid_rows.len() as f64;
            mono_lymph_by_sample.insert(id.clone(), myeloid - lymphoid);
        }
    }
    let _ = outlier_threshold;
    ImmuneDiagnostics {
        pc_covariates: pc_by_sample.clone(),
        pc_by_sample,
        outlier_by_sample,
        mono_lymph_by_sample,
        warnings,
    }
}

fn filter_metadata(metadata: &MetadataTable, exclude: &HashSet<String>) -> Result<MetadataTable> {
    if metadata.column_index(&metadata.id_column).is_none() {
        bail!("Metadata ID column is missing");
    }
    let rows = metadata
        .rows
        .iter()
        .enumerate()
        .filter(|(row_idx, _)| {
            !row_matching_ids(metadata, *row_idx)
                .iter()
                .any(|id| exclude.contains(id))
        })
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    Ok(MetadataTable {
        path: metadata.path.clone(),
        columns: metadata.columns.clone(),
        rows,
        id_column: metadata.id_column.clone(),
    })
}

fn augment_metadata_with_immune_pcs(
    metadata: &MetadataTable,
    pcs: &HashMap<String, (f64, f64)>,
) -> Result<MetadataTable> {
    if metadata.column_index(&metadata.id_column).is_none() {
        bail!("Metadata ID column is missing");
    }
    let mut columns = metadata.columns.clone();
    columns.push("QC_Immune_PC1".to_string());
    columns.push("QC_Immune_PC2".to_string());
    let rows = metadata
        .rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let mut out = row.clone();
            let matched = row_matching_ids(metadata, row_idx)
                .iter()
                .find_map(|id| pcs.get(id));
            if let Some((pc1, pc2)) = matched {
                out.push(pc1.to_string());
                out.push(pc2.to_string());
            } else {
                out.push(String::new());
                out.push(String::new());
            }
            out
        })
        .collect::<Vec<_>>();
    Ok(MetadataTable {
        path: metadata.path.clone(),
        columns,
        rows,
        id_column: metadata.id_column.clone(),
    })
}

fn build_source_effect_screen(
    merged_gene_tsv: &Path,
    columns: &[SampleColumns],
    suspect_ids: &HashSet<String>,
    known_non_pbmc_ids: &HashSet<String>,
) -> Result<Option<GeneComparisonResult>> {
    let all = columns
        .iter()
        .map(|c| normalize_sample_id(&c.sample_id))
        .collect::<HashSet<_>>();
    let case_ids = suspect_ids
        .intersection(&all)
        .cloned()
        .collect::<HashSet<_>>();
    let control_ids = all
        .iter()
        .filter(|id| !case_ids.contains(*id) && !known_non_pbmc_ids.contains(*id))
        .cloned()
        .collect::<HashSet<_>>();
    if case_ids.len() < 2 || control_ids.len() < 2 {
        return Ok(None);
    }
    let comparison = CohortComparison {
        name: "User-marked unusual samples vs ordinary samples".to_string(),
        case_label: "Unusual/suspect".to_string(),
        control_label: "Ordinary".to_string(),
        case_metadata_rows: case_ids.len(),
        control_metadata_rows: control_ids.len(),
        case_ids,
        control_ids,
        id_to_canonical: HashMap::new(),
    };
    Ok(Some(run_gene_comparison(
        merged_gene_tsv,
        &comparison,
        "TPM",
        |_d, _t, _m| {},
    )?))
}

fn build_candidate_stability(
    merged_gene_tsv: &Path,
    columns: &[SampleColumns],
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    config: &PbmcDiagnosticsConfig,
    baseline: &CorrectedExpressionResult,
    restricted: Option<&CorrectedExpressionResult>,
    immune_adjusted: Option<&CorrectedExpressionResult>,
    source_effect: Option<&GeneComparisonResult>,
    sample_qc: &[PbmcSampleQcRow],
) -> Result<Vec<CandidateStabilityRow>> {
    let candidate_tests = baseline
        .tests
        .iter()
        .take(config.top_candidates.max(1))
        .collect::<Vec<_>>();
    let wanted = candidate_tests
        .iter()
        .map(|t| t.feature.clone())
        .collect::<HashSet<_>>();
    let values = read_candidate_values(merged_gene_tsv, columns, &wanted)?;
    let restricted_map = restricted
        .map(|r| {
            r.tests
                .iter()
                .map(|t| (t.feature.as_str(), t))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let immune_map = immune_adjusted
        .map(|r| {
            r.tests
                .iter()
                .map(|t| (t.feature.as_str(), t))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let source_map = source_effect
        .map(|r| {
            r.tests
                .iter()
                .map(|t| (t.feature.as_str(), t))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let qc_map = sample_qc
        .iter()
        .map(|r| (normalize_sample_id(&r.sample_id), r))
        .collect::<HashMap<_, _>>();
    let meta_index = unique_metadata_rows(metadata);
    let tau2 = estimate_effect_prior_variance(&baseline.tests);
    let mut out = Vec::new();
    for test in candidate_tests {
        let Some(row) = values.get(&test.feature) else {
            continue;
        };
        let mut case_points = Vec::new();
        let mut ctrl_points = Vec::new();
        let mut points = Vec::new();
        for (i, col) in columns.iter().enumerate() {
            let key = normalize_sample_id(&col.sample_id);
            let group = if comparison.is_case_id(&key) {
                Some("Case")
            } else if comparison.is_control_id(&key) {
                Some("Control")
            } else {
                None
            };
            let Some(group) = group else {
                continue;
            };
            let qc = qc_map.get(&key).copied();
            let meta_row = meta_index.get(&key).copied();
            let sensitivity_level = config
                .sensitivity_column
                .as_deref()
                .map(|c| metadata_value(metadata, meta_row, c))
                .unwrap_or_default();
            let point = CandidateSamplePoint {
                sample_id: col.sample_id.clone(),
                group: group.to_string(),
                raw_count: row.raw[i],
                tpm: row.tpm[i],
                suspect: config.suspect_ids.contains(&key),
                sensitivity_excluded: qc.is_some_and(|x| x.sensitivity_excluded),
                subject: qc.map(|x| x.subject.clone()).unwrap_or_default(),
                batch: qc.map(|x| x.batch.clone()).unwrap_or_default(),
                family: qc.map(|x| x.family.clone()).unwrap_or_default(),
                sensitivity_level,
            };
            if group == "Case" {
                case_points.push(point.clone());
            } else {
                ctrl_points.push(point.clone());
            }
            points.push(point);
        }
        if case_points.len() < 2 || ctrl_points.len() < 2 {
            continue;
        }
        let case_raw = case_points.iter().map(|p| p.raw_count).collect::<Vec<_>>();
        let ctrl_raw = ctrl_points.iter().map(|p| p.raw_count).collect::<Vec<_>>();
        let case_tpm = case_points.iter().map(|p| p.tpm).collect::<Vec<_>>();
        let ctrl_tpm = ctrl_points.iter().map(|p| p.tpm).collect::<Vec<_>>();
        let (loo_min, loo_max) = leave_one_out_effect_range(&case_tpm, &ctrl_tpm);
        let (leave_min, leave_max) = leave_level_effect_range(&points);
        let r = restricted_map.get(test.feature.as_str()).copied();
        let ia = immune_map.get(test.feature.as_str()).copied();
        let source = source_map.get(test.feature.as_str()).copied();
        let shrink = if test.robust_se.is_finite() {
            test.beta_group * tau2 / (tau2 + test.robust_se * test.robust_se)
        } else {
            test.beta_group
        };
        let total_n = case_raw.len() + ctrl_raw.len();
        let ge10_fraction = (case_raw
            .iter()
            .chain(ctrl_raw.iter())
            .filter(|x| **x >= config.low_count_threshold)
            .count() as f64)
            / total_n as f64;
        let top2_case = top_k_contribution(&case_tpm, 2);
        let top2_ctrl = top_k_contribution(&ctrl_tpm, 2);
        let mut flags = Vec::new();
        if ge10_fraction < config.low_count_min_fraction {
            flags.push("low-count / sparse".to_string());
        }
        if top2_case.max(top2_ctrl) >= config.top2_contribution_threshold {
            flags.push("signal concentrated in one/two samples".to_string());
        }
        if loo_min.is_finite() && loo_max.is_finite() && loo_min.signum() != loo_max.signum() {
            flags.push("leave-one-sample-out sign unstable".to_string());
        }
        if let Some(r) = r {
            if r.beta_group.signum() != test.beta_group.signum() {
                flags.push("QC-restricted sign reversal".to_string());
            } else if test.beta_group.abs() > 0.25
                && r.beta_group.abs() < 0.5 * test.beta_group.abs()
            {
                flags.push("QC-restricted effect attenuated".to_string());
            }
        }
        if let Some(a) = ia {
            if a.beta_group.signum() != test.beta_group.signum() {
                flags.push("immune-adjusted sign reversal".to_string());
            } else if test.beta_group.abs() > 0.25
                && a.beta_group.abs() < 0.5 * test.beta_group.abs()
            {
                flags.push("immune-adjusted effect attenuated".to_string());
            }
        }
        if leave_min.is_finite()
            && leave_max.is_finite()
            && leave_min.signum() != leave_max.signum()
        {
            flags.push("leave-batch/family-out sign unstable".to_string());
        }
        let classification = if flags
            .iter()
            .any(|f| f.contains("sign reversal") || f.contains("sign unstable"))
        {
            "confounding-sensitive".to_string()
        } else if flags
            .iter()
            .any(|f| f.contains("low-count") || f.contains("concentrated"))
        {
            "outlier / low-count sensitive".to_string()
        } else if test.q_value <= 0.05
            && r.is_some_and(|x| {
                x.q_value <= 0.10 && x.beta_group.signum() == test.beta_group.signum()
            })
        {
            "robust preliminary candidate".to_string()
        } else {
            "needs review".to_string()
        };
        out.push(CandidateStabilityRow {
            gene: test.feature.clone(),
            gene_id: row.gene_id.clone(),
            baseline_log2fc: test.beta_group,
            baseline_q: test.q_value,
            regularized_log2fc: shrink,
            case_median_raw: median(&case_raw),
            control_median_raw: median(&ctrl_raw),
            case_median_tpm: median(&case_tpm),
            control_median_tpm: median(&ctrl_tpm),
            detected_case: fraction(&case_raw, |v| v > 0.0),
            detected_control: fraction(&ctrl_raw, |v| v > 0.0),
            count_ge5_case: fraction(&case_raw, |v| v >= 5.0),
            count_ge5_control: fraction(&ctrl_raw, |v| v >= 5.0),
            count_ge10_case: fraction(&case_raw, |v| v >= 10.0),
            count_ge10_control: fraction(&ctrl_raw, |v| v >= 10.0),
            top2_contribution_case: top2_case,
            top2_contribution_control: top2_ctrl,
            loo_log2fc_min: loo_min,
            loo_log2fc_max: loo_max,
            leave_level_log2fc_min: leave_min,
            leave_level_log2fc_max: leave_max,
            qc_restricted_log2fc: r.map(|x| x.beta_group).unwrap_or(f64::NAN),
            qc_restricted_q: r.map(|x| x.q_value).unwrap_or(f64::NAN),
            immune_adjusted_log2fc: ia.map(|x| x.beta_group).unwrap_or(f64::NAN),
            immune_adjusted_q: ia.map(|x| x.q_value).unwrap_or(f64::NAN),
            source_effect_log2fc: source.map(|x| x.effect).unwrap_or(f64::NAN),
            source_effect_q: source.map(|x| x.q_value).unwrap_or(f64::NAN),
            classification,
            flags,
            sample_points: points,
        });
    }
    Ok(out)
}

fn read_candidate_values(
    path: &Path,
    columns: &[SampleColumns],
    wanted: &HashSet<String>,
) -> Result<HashMap<String, CandidateValues>> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(path)?;
    let mut out = HashMap::new();
    for record in reader.records() {
        let record = record?;
        let gene = record.get(0).unwrap_or_default();
        if !wanted.contains(gene) {
            continue;
        }
        let gene_id = record.get(1).unwrap_or_default().to_string();
        let raw = columns
            .iter()
            .map(|c| {
                record
                    .get(c.raw_idx)
                    .and_then(parse_f64)
                    .unwrap_or(0.0)
                    .max(0.0)
            })
            .collect::<Vec<_>>();
        let tpm = columns
            .iter()
            .map(|c| {
                record
                    .get(c.tpm_idx)
                    .and_then(parse_f64)
                    .unwrap_or(0.0)
                    .max(0.0)
            })
            .collect::<Vec<_>>();
        out.insert(gene.to_string(), CandidateValues { gene_id, raw, tpm });
        if out.len() == wanted.len() {
            break;
        }
    }
    Ok(out)
}

fn build_longitudinal_qc(sample_qc: &[PbmcSampleQcRow]) -> Vec<LongitudinalQcRow> {
    let mut by_subject: BTreeMap<String, Vec<&PbmcSampleQcRow>> = BTreeMap::new();
    for row in sample_qc {
        if row.subject.trim().is_empty() {
            continue;
        }
        by_subject.entry(row.subject.clone()).or_default().push(row);
    }
    let mut out = Vec::new();
    for (subject, rows) in by_subject {
        if rows.len() < 2 {
            continue;
        }
        let mut max_dist = 0.0;
        for i in 0..rows.len() {
            for j in (i + 1)..rows.len() {
                if rows[i].pca1.is_finite()
                    && rows[i].pca2.is_finite()
                    && rows[j].pca1.is_finite()
                    && rows[j].pca2.is_finite()
                {
                    let d = ((rows[i].pca1 - rows[j].pca1).powi(2)
                        + (rows[i].pca2 - rows[j].pca2).powi(2))
                    .sqrt();
                    if d > max_dist {
                        max_dist = d;
                    }
                }
            }
        }
        let samples = rows
            .iter()
            .map(|r| r.sample_id.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let batches = rows
            .iter()
            .map(|r| r.batch.clone())
            .filter(|x| !x.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        let flagged = rows
            .iter()
            .filter(|r| !r.flags.is_empty())
            .map(|r| r.sample_id.clone())
            .collect::<Vec<_>>()
            .join(", ");
        out.push(LongitudinalQcRow {
            subject,
            n_samples: rows.len(),
            max_pca_distance: max_dist,
            samples,
            batches,
            flagged_samples: flagged,
        });
    }
    out.sort_by(|a, b| b.max_pca_distance.total_cmp(&a.max_pca_distance));
    out
}

pub fn write_pbmc_diagnostics_tables(
    output_dir: &Path,
    result: &PbmcDiagnosticsResult,
) -> Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    {
        let mut w = WriterBuilder::new().from_path(output_dir.join("pbmc_sample_qc.csv"))?;
        w.write_record([
            "sample_id",
            "comparison_group",
            "phenotype",
            "hyp",
            "subject",
            "batch",
            "family",
            "library_raw_count",
            "detected_genes",
            "genes_ge5",
            "genes_ge10",
            "pca1",
            "pca2",
            "transcriptome_outlier_score",
            "immune_pc1",
            "immune_pc2",
            "immune_outlier_score",
            "monocyte_lymphoid_signature",
            "known_non_pbmc",
            "suspect",
            "auto_expression_outlier",
            "auto_immune_outlier",
            "sensitivity_excluded",
            "flags",
        ])?;
        for r in &result.sample_qc {
            w.write_record([
                r.sample_id.clone(),
                r.comparison_group.clone(),
                r.phenotype.clone(),
                r.hyp.clone(),
                r.subject.clone(),
                r.batch.clone(),
                r.family.clone(),
                r.library_raw_count.to_string(),
                r.detected_genes.to_string(),
                r.genes_ge5.to_string(),
                r.genes_ge10.to_string(),
                r.pca1.to_string(),
                r.pca2.to_string(),
                r.transcriptome_outlier_score.to_string(),
                r.immune_pc1.to_string(),
                r.immune_pc2.to_string(),
                r.immune_outlier_score.to_string(),
                r.monocyte_lymphoid_signature.to_string(),
                r.known_non_pbmc.to_string(),
                r.suspect.to_string(),
                r.auto_expression_outlier.to_string(),
                r.auto_immune_outlier.to_string(),
                r.sensitivity_excluded.to_string(),
                r.flags.join("; "),
            ])?;
        }
        w.flush()?;
    }
    {
        let mut w = WriterBuilder::new().from_path(output_dir.join("candidate_robustness.csv"))?;
        w.write_record([
            "gene",
            "gene_id",
            "baseline_log2fc",
            "baseline_q",
            "regularized_log2fc",
            "case_median_raw",
            "control_median_raw",
            "case_median_tpm",
            "control_median_tpm",
            "detected_case",
            "detected_control",
            "count_ge5_case",
            "count_ge5_control",
            "count_ge10_case",
            "count_ge10_control",
            "top2_contribution_case",
            "top2_contribution_control",
            "loo_log2fc_min",
            "loo_log2fc_max",
            "leave_level_log2fc_min",
            "leave_level_log2fc_max",
            "qc_restricted_log2fc",
            "qc_restricted_q",
            "immune_adjusted_log2fc",
            "immune_adjusted_q",
            "source_effect_log2fc",
            "source_effect_q",
            "classification",
            "flags",
        ])?;
        for r in &result.candidate_stability {
            w.write_record([
                r.gene.clone(),
                r.gene_id.clone(),
                r.baseline_log2fc.to_string(),
                r.baseline_q.to_string(),
                r.regularized_log2fc.to_string(),
                r.case_median_raw.to_string(),
                r.control_median_raw.to_string(),
                r.case_median_tpm.to_string(),
                r.control_median_tpm.to_string(),
                r.detected_case.to_string(),
                r.detected_control.to_string(),
                r.count_ge5_case.to_string(),
                r.count_ge5_control.to_string(),
                r.count_ge10_case.to_string(),
                r.count_ge10_control.to_string(),
                r.top2_contribution_case.to_string(),
                r.top2_contribution_control.to_string(),
                r.loo_log2fc_min.to_string(),
                r.loo_log2fc_max.to_string(),
                r.leave_level_log2fc_min.to_string(),
                r.leave_level_log2fc_max.to_string(),
                r.qc_restricted_log2fc.to_string(),
                r.qc_restricted_q.to_string(),
                r.immune_adjusted_log2fc.to_string(),
                r.immune_adjusted_q.to_string(),
                r.source_effect_log2fc.to_string(),
                r.source_effect_q.to_string(),
                r.classification.clone(),
                r.flags.join("; "),
            ])?;
        }
        w.flush()?;
    }
    {
        let mut w =
            WriterBuilder::new().from_path(output_dir.join("candidate_sample_values.csv"))?;
        w.write_record([
            "gene",
            "sample_id",
            "group",
            "raw_count",
            "tpm",
            "suspect",
            "sensitivity_excluded",
            "subject",
            "batch",
            "family",
            "sensitivity_level",
        ])?;
        for r in &result.candidate_stability {
            for p in &r.sample_points {
                w.write_record([
                    r.gene.clone(),
                    p.sample_id.clone(),
                    p.group.clone(),
                    p.raw_count.to_string(),
                    p.tpm.to_string(),
                    p.suspect.to_string(),
                    p.sensitivity_excluded.to_string(),
                    p.subject.clone(),
                    p.batch.clone(),
                    p.family.clone(),
                    p.sensitivity_level.clone(),
                ])?;
            }
        }
        w.flush()?;
    }
    {
        let mut w = WriterBuilder::new().from_path(output_dir.join("longitudinal_qc.csv"))?;
        w.write_record([
            "subject",
            "n_samples",
            "max_pca_distance",
            "samples",
            "batches",
            "flagged_samples",
        ])?;
        for r in &result.longitudinal_qc {
            w.write_record([
                r.subject.clone(),
                r.n_samples.to_string(),
                r.max_pca_distance.to_string(),
                r.samples.clone(),
                r.batches.clone(),
                r.flagged_samples.clone(),
            ])?;
        }
        w.flush()?;
    }
    if let Some(pca) = &result.pca {
        write_ordination(&output_dir.join("pbmc_whole_transcriptome_pca.tsv"), pca)?;
    }
    if let Some(r) = &result.source_effect {
        write_feature_tests(
            &output_dir.join("suspect_vs_ordinary_expression.tsv"),
            &r.tests,
        )?;
    }
    if let Some(r) = &result.baseline_gene {
        write_adjusted_tests(
            &output_dir.join("baseline_corrected_gene_tests.tsv"),
            &r.tests,
        )?;
    }
    if let Some(r) = &result.qc_restricted_gene {
        write_adjusted_tests(&output_dir.join("qc_restricted_gene_tests.tsv"), &r.tests)?;
    }
    if let Some(r) = &result.immune_adjusted_gene {
        write_adjusted_tests(
            &output_dir.join("immune_pc_adjusted_gene_tests.tsv"),
            &r.tests,
        )?;
    }
    fs::write(
        output_dir.join("sensitivity_excluded_samples.txt"),
        result.sensitivity_excluded_ids.join("\n"),
    )?;
    if !result.warnings.is_empty() {
        fs::write(
            output_dir.join("diagnostic_warnings.txt"),
            result.warnings.join("\n"),
        )?;
    }
    let notes = format!(
        "4C Cohort Explorer PBMC/confounding diagnostics\nComparison: {}\n\nThese outputs are diagnostic. Automatic PCA or immune-signature outlier flags are not proof that a sample is invalid. Review provenance, sample type, processing history, longitudinal consistency, and the raw expression distributions before making final exclusion decisions.\n",
        result.comparison.name
    );
    fs::write(output_dir.join("README.txt"), notes)?;
    Ok(output_dir.to_path_buf())
}

fn unique_metadata_rows(metadata: &MetadataTable) -> HashMap<String, usize> {
    if metadata.column_index(&metadata.id_column).is_none() {
        return HashMap::new();
    }
    let mut temp: HashMap<String, Vec<usize>> = HashMap::new();
    for i in 0..metadata.rows.len() {
        for id in row_matching_ids(metadata, i) {
            let entry = temp.entry(id).or_default();
            if !entry.contains(&i) {
                entry.push(i);
            }
        }
    }
    temp.into_iter()
        .filter_map(|(id, rows)| {
            if rows.len() == 1 {
                Some((id, rows[0]))
            } else {
                None
            }
        })
        .collect()
}

fn metadata_value(metadata: &MetadataTable, row: Option<usize>, column: &str) -> String {
    row.and_then(|r| metadata.value(r, column))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn estimate_effect_prior_variance(tests: &[crate::corrected::AdjustedFeatureResult]) -> f64 {
    let betas = tests
        .iter()
        .map(|t| t.beta_group)
        .filter(|v| v.is_finite())
        .collect::<Vec<_>>();
    if betas.len() < 10 {
        return 0.25;
    }
    let m = median(&betas);
    let mad = median(&betas.iter().map(|v| (v - m).abs()).collect::<Vec<_>>());
    (1.4826 * mad).powi(2).max(0.05)
}

fn leave_one_out_effect_range(case: &[f64], ctrl: &[f64]) -> (f64, f64) {
    if case.len() < 3 || ctrl.len() < 3 {
        return (f64::NAN, f64::NAN);
    }
    let mut effects = Vec::with_capacity(case.len() + ctrl.len());
    for i in 0..case.len() {
        let x = case
            .iter()
            .enumerate()
            .filter_map(|(j, v)| if i != j { Some(*v) } else { None })
            .collect::<Vec<_>>();
        effects.push(log2_mean_effect(&x, ctrl));
    }
    for i in 0..ctrl.len() {
        let y = ctrl
            .iter()
            .enumerate()
            .filter_map(|(j, v)| if i != j { Some(*v) } else { None })
            .collect::<Vec<_>>();
        effects.push(log2_mean_effect(case, &y));
    }
    range_finite(&effects)
}

fn leave_level_effect_range(points: &[CandidateSamplePoint]) -> (f64, f64) {
    let levels = points
        .iter()
        .map(|p| p.sensitivity_level.trim())
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>();
    if levels.len() < 2 {
        return (f64::NAN, f64::NAN);
    }
    let mut effects = Vec::new();
    for level in levels {
        let case = points
            .iter()
            .filter(|p| p.group == "Case" && p.sensitivity_level.trim() != level)
            .map(|p| p.tpm)
            .collect::<Vec<_>>();
        let ctrl = points
            .iter()
            .filter(|p| p.group == "Control" && p.sensitivity_level.trim() != level)
            .map(|p| p.tpm)
            .collect::<Vec<_>>();
        if case.len() >= 2 && ctrl.len() >= 2 {
            effects.push(log2_mean_effect(&case, &ctrl));
        }
    }
    range_finite(&effects)
}

fn log2_mean_effect(case: &[f64], ctrl: &[f64]) -> f64 {
    ((mean(case) + 0.5) / (mean(ctrl) + 0.5)).log2()
}

fn top_k_contribution(values: &[f64], k: usize) -> f64 {
    let total = values
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .sum::<f64>();
    if total <= 0.0 {
        return 0.0;
    }
    let mut sorted = values
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| b.total_cmp(a));
    sorted.iter().take(k).sum::<f64>() / total
}

fn fraction<F: Fn(f64) -> bool>(values: &[f64], predicate: F) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().copied().filter(|v| predicate(*v)).count() as f64 / values.len() as f64
}

fn parse_f64(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
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
        1.0
    }
}

fn sample_variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return f64::NAN;
    }
    let m = mean(values);
    values
        .iter()
        .filter(|v| v.is_finite())
        .map(|v| (v - m).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64
}

fn range_finite(values: &[f64]) -> (f64, f64) {
    let finite = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect::<Vec<_>>();
    if finite.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let min = finite.iter().copied().fold(f64::INFINITY, f64::min);
    let max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (min, max)
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
    fn top_two_contribution_identifies_concentrated_signal() {
        let values = vec![100.0, 80.0, 1.0, 1.0, 1.0];
        assert!(top_k_contribution(&values, 2) > 0.95);
    }

    #[test]
    fn leave_one_out_range_detects_single_sample_instability() {
        let case = vec![100.0, 0.0, 0.0, 0.0];
        let ctrl = vec![0.0, 0.0, 0.0, 0.0];
        let (min, max) = leave_one_out_effect_range(&case, &ctrl);
        assert!(min <= 0.01);
        assert!(max > 1.0);
    }

    #[test]
    fn metadata_filter_removes_only_selected_ids() {
        let metadata = MetadataTable {
            path: PathBuf::from("meta.tsv"),
            columns: vec!["ID".into(), "Phenotype".into()],
            rows: vec![
                vec!["A".into(), "case".into()],
                vec!["B".into(), "control".into()],
            ],
            id_column: "ID".into(),
        };
        let exclude = HashSet::from([normalize_sample_id("A")]);
        let filtered = filter_metadata(&metadata, &exclude).unwrap();
        assert_eq!(filtered.rows.len(), 1);
        assert_eq!(filtered.rows[0][0], "B");
    }
}
