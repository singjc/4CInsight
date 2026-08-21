use anyhow::{anyhow, bail, Context, Result};
use calamine::{open_workbook, Data, Reader, Xlsx};
use csv::ReaderBuilder;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CohortModality {
    GeneExpression,
    TranscriptExpression,
    Bcr,
    BcrIsotype,
    TcrAlpha,
    TcrBeta,
    ImmuneInfiltrate,
}

impl CohortModality {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::GeneExpression => "Gene expression",
            Self::TranscriptExpression => "Transcript expression",
            Self::Bcr => "BCR",
            Self::BcrIsotype => "BCR isotype",
            Self::TcrAlpha => "TCR alpha",
            Self::TcrBeta => "TCR beta",
            Self::ImmuneInfiltrate => "Immune infiltrate",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DetectedCohortFile {
    pub path: PathBuf,
    pub modality: CohortModality,
    pub sample_id: String,
}

#[derive(Debug, Clone)]
pub struct ProjectScan {
    pub root: PathBuf,
    pub files: Vec<DetectedCohortFile>,
    pub merged_gene_tsv: Option<PathBuf>,
    pub merged_transcript_tsv: Option<PathBuf>,
    pub metadata_candidates: Vec<PathBuf>,
}

impl ProjectScan {
    pub fn count_modality(&self, modality: CohortModality) -> usize {
        self.files.iter().filter(|f| f.modality == modality).count()
    }

    pub fn unique_samples(&self) -> usize {
        self.files
            .iter()
            .map(|f| normalize_sample_id(&f.sample_id))
            .collect::<HashSet<_>>()
            .len()
    }
}

#[derive(Debug, Clone)]
pub struct MetadataTable {
    pub path: PathBuf,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub id_column: String,
}

impl MetadataTable {
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }

    pub fn value(&self, row: usize, column: &str) -> Option<&str> {
        let col = self.column_index(column)?;
        self.rows.get(row)?.get(col).map(String::as_str)
    }

    pub fn id_value(&self, row: usize) -> Option<&str> {
        self.value(row, &self.id_column)
    }

    pub fn with_id_column(mut self, id_column: impl Into<String>) -> Result<Self> {
        let id_column = id_column.into();
        if self.column_index(&id_column).is_none() {
            bail!("Metadata column '{id_column}' does not exist");
        }
        self.id_column = id_column;
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingStatus {
    Matched,
    Unmatched,
    Ambiguous,
    MetadataOnly,
}

#[derive(Debug, Clone)]
pub struct SampleCoverageRow {
    pub sample_id: String,
    pub metadata_row: Option<usize>,
    pub mapping_status: MappingStatus,
    pub phenotype: String,
    pub hyp: String,
    pub counts: BTreeMap<CohortModality, usize>,
}

#[derive(Debug, Clone)]
pub struct CohortMapping {
    pub rows: Vec<SampleCoverageRow>,
    pub matched_files: usize,
    pub unmatched_files: usize,
    pub ambiguous_files: usize,
    pub metadata_only_samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonPreset {
    HypVsTrueHc,
    HypMecfsVsMecfsOnly,
}

impl ComparisonPreset {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::HypVsTrueHc => "HYP vs True_HC",
            Self::HypMecfsVsMecfsOnly => "HYP+ME/CFS vs ME/CFS-only",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::HypVsTrueHc => {
                "Discovery comparison: all metadata rows marked HYP=yes (hEDS/HSD, with or without ME/CFS) versus Phenotype=True_HC."
            }
            Self::HypMecfsVsMecfsOnly => {
                "Specificity comparison: Phenotype=HYP+ME/CFS versus Phenotype=ME/CFS with no HYP flag."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CohortComparison {
    pub name: String,
    pub case_label: String,
    pub control_label: String,
    pub case_ids: HashSet<String>,
    pub control_ids: HashSet<String>,
    pub case_metadata_rows: usize,
    pub control_metadata_rows: usize,
}

#[derive(Debug, Clone)]
pub struct FeatureTestResult {
    pub feature: String,
    pub secondary_id: String,
    pub n_case: usize,
    pub n_control: usize,
    pub mean_case: f64,
    pub mean_control: f64,
    pub median_case: f64,
    pub median_control: f64,
    pub effect: f64,
    pub cliffs_delta: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub detection_case: Option<f64>,
    pub detection_control: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct GeneComparisonResult {
    pub input: PathBuf,
    pub metric: String,
    pub n_case_columns: usize,
    pub n_control_columns: usize,
    pub unmatched_columns: usize,
    pub tests: Vec<FeatureTestResult>,
}

#[derive(Debug, Clone)]
pub struct BcrSampleSummary {
    pub sample_id: String,
    pub group: String,
    pub total_clone_count: f64,
    pub unique_clones: f64,
    pub shannon: f64,
    pub simpson: f64,
    pub clonality: f64,
    pub effective_n: f64,
    pub top_clone_fraction: f64,
    pub weighted_mean_cdr3_aa_length: f64,
}

#[derive(Debug, Clone)]
pub struct BcrComparisonResult {
    pub files_used: usize,
    pub files_skipped_unmatched: usize,
    pub files_skipped_not_in_groups: usize,
    pub file_errors: Vec<String>,
    pub sample_summaries: Vec<BcrSampleSummary>,
    pub metric_tests: Vec<FeatureTestResult>,
    pub isotype_tests: Vec<FeatureTestResult>,
    pub v_gene_tests: Vec<FeatureTestResult>,
    pub d_gene_tests: Vec<FeatureTestResult>,
    pub j_gene_tests: Vec<FeatureTestResult>,
}

#[derive(Debug, Clone)]
pub struct BcrIsotypeSampleValue {
    pub sample_id: String,
    pub group: String,
    pub isotype: String,
    pub fraction: f64,
}

#[derive(Debug, Clone)]
pub struct BcrIsotypeComparisonResult {
    pub files_used: usize,
    pub files_skipped_unmatched: usize,
    pub files_skipped_not_in_groups: usize,
    pub file_errors: Vec<String>,
    pub values: Vec<BcrIsotypeSampleValue>,
    pub tests: Vec<FeatureTestResult>,
}

pub fn scan_project(root: &Path) -> Result<ProjectScan> {
    if !root.is_dir() {
        bail!("Project root is not a directory: {}", root.display());
    }
    let mut paths = Vec::new();
    collect_files_recursive(root, &mut paths)?;
    paths.sort();

    let mut files = Vec::new();
    let mut merged_gene_tsv = None;
    let mut merged_transcript_tsv = None;
    let mut metadata_candidates = Vec::new();

    for path in paths {
        let Some(name) = path.file_name().and_then(|x| x.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".xlsx") || lower.ends_with(".xls") {
            if let Some((modality, sample_id)) = classify_workbook_name(name) {
                files.push(DetectedCohortFile {
                    path,
                    modality,
                    sample_id,
                });
            } else if lower.contains("sample") && (lower.contains("submission") || lower.contains("metadata")) {
                metadata_candidates.push(path);
            }
            continue;
        }
        if lower.ends_with(".tsv") {
            if lower.contains("merged_gene_expression") {
                merged_gene_tsv.get_or_insert(path);
            } else if lower.contains("merged_transcript_expression") {
                merged_transcript_tsv.get_or_insert(path);
            }
        }
    }

    Ok(ProjectScan {
        root: root.to_path_buf(),
        files,
        merged_gene_tsv,
        merged_transcript_tsv,
        metadata_candidates,
    })
}

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Could not read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn classify_workbook_name(name: &str) -> Option<(CohortModality, String)> {
    let canonical_name = strip_copy_suffix(name);
    let lower = canonical_name.to_ascii_lowercase();
    let suffixes = [
        ("_tumor_rna_gene_expression_report.xlsx", CohortModality::GeneExpression),
        ("_tumor_rna_transcript_expression_report.xlsx", CohortModality::TranscriptExpression),
        ("_bcr_heavy_chain_report.xlsx", CohortModality::Bcr),
        ("_bcr_isotype_composition.xlsx", CohortModality::BcrIsotype),
        ("_tcr_alpha_clone_report.xlsx", CohortModality::TcrAlpha),
        ("_tcr_beta_clone_report.xlsx", CohortModality::TcrBeta),
        ("_immune_infiltrate.xlsx", CohortModality::ImmuneInfiltrate),
    ];
    for (suffix, modality) in suffixes {
        if lower.ends_with(suffix) {
            let prefix_len = canonical_name.len().saturating_sub(suffix.len());
            return Some((modality, canonical_name[..prefix_len].to_string()));
        }
    }
    None
}

fn strip_copy_suffix(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let Some(ext_pos) = lower.rfind(".xlsx") else {
        return name.to_string();
    };
    let stem = &name[..ext_pos];
    if let Some(open) = stem.rfind(" (") {
        if stem.ends_with(')') {
            let digits = &stem[open + 2..stem.len() - 1];
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                return format!("{}.xlsx", &stem[..open]);
            }
        }
    }
    name.to_string()
}

pub fn load_metadata(path: &Path) -> Result<MetadataTable> {
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (columns, rows) = match ext.as_str() {
        "xlsx" | "xls" => read_metadata_xlsx(path)?,
        "csv" => read_metadata_delimited(path, b',')?,
        "tsv" | "txt" => read_metadata_delimited(path, b'\t')?,
        _ => bail!("Unsupported metadata file type: {}", path.display()),
    };
    if columns.is_empty() {
        bail!("Metadata file has no header row");
    }
    let id_column = ["4C_ID", "sample_id", "Sample_ID", "Sample", "sample"]
        .iter()
        .find(|candidate| columns.iter().any(|col| col == **candidate))
        .map(|x| (*x).to_string())
        .unwrap_or_else(|| columns[0].clone());
    Ok(MetadataTable {
        path: path.to_path_buf(),
        columns,
        rows,
        id_column,
    })
}

fn read_metadata_xlsx(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Could not open metadata workbook {}", path.display()))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("Metadata workbook has no worksheets"))?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("Could not read worksheet '{sheet_name}'"))?;
    let mut iter = range.rows();
    let header = iter.next().ok_or_else(|| anyhow!("Metadata worksheet is empty"))?;
    let columns: Vec<String> = header.iter().map(cell_text).collect();
    let rows = iter
        .map(|row| {
            (0..columns.len())
                .map(|i| row.get(i).map(cell_text).unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|v| !v.trim().is_empty()))
        .collect();
    Ok((columns, rows))
}

fn read_metadata_delimited(path: &Path, delimiter: u8) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .from_path(path)
        .with_context(|| format!("Could not open metadata table {}", path.display()))?;
    let columns = reader.headers()?.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        rows.push(record.iter().map(|x| x.to_string()).collect());
    }
    Ok((columns, rows))
}

pub fn build_mapping(scan: &ProjectScan, metadata: &MetadataTable) -> CohortMapping {
    let id_col = metadata.column_index(&metadata.id_column).unwrap_or(0);
    let mut meta_index: HashMap<String, Vec<usize>> = HashMap::new();
    for (row_idx, row) in metadata.rows.iter().enumerate() {
        let key = normalize_sample_id(row.get(id_col).map(String::as_str).unwrap_or_default());
        if !key.is_empty() {
            meta_index.entry(key).or_default().push(row_idx);
        }
    }

    let mut grouped: BTreeMap<String, (String, BTreeMap<CohortModality, usize>)> = BTreeMap::new();
    for file in &scan.files {
        let key = normalize_sample_id(&file.sample_id);
        let entry = grouped
            .entry(key)
            .or_insert_with(|| (file.sample_id.clone(), BTreeMap::new()));
        *entry.1.entry(file.modality).or_insert(0) += 1;
    }

    let mut rows = Vec::new();
    let mut matched_files = 0usize;
    let mut unmatched_files = 0usize;
    let mut ambiguous_files = 0usize;
    for (key, (sample_id, counts)) in grouped {
        let matches = meta_index.get(&key);
        let (status, metadata_row) = match matches {
            Some(v) if v.len() == 1 => (MappingStatus::Matched, Some(v[0])),
            Some(_) => (MappingStatus::Ambiguous, None),
            None => (MappingStatus::Unmatched, None),
        };
        let n_files: usize = counts.values().sum();
        match status {
            MappingStatus::Matched => matched_files += n_files,
            MappingStatus::Unmatched => unmatched_files += n_files,
            MappingStatus::Ambiguous => ambiguous_files += n_files,
            MappingStatus::MetadataOnly => {}
        }
        let phenotype = metadata_row
            .and_then(|r| metadata.value(r, "Phenotype"))
            .unwrap_or_default()
            .to_string();
        let hyp = metadata_row
            .and_then(|r| metadata.value(r, "HYP"))
            .unwrap_or_default()
            .to_string();
        rows.push(SampleCoverageRow {
            sample_id,
            metadata_row,
            mapping_status: status,
            phenotype,
            hyp,
            counts,
        });
    }

    let file_keys: HashSet<String> = rows
        .iter()
        .map(|row| normalize_sample_id(&row.sample_id))
        .collect();
    let mut metadata_only_samples = 0usize;
    for (key, matches) in &meta_index {
        if file_keys.contains(key) || matches.len() != 1 {
            continue;
        }
        let row_idx = matches[0];
        let sample_id = metadata
            .rows
            .get(row_idx)
            .and_then(|row| row.get(id_col))
            .cloned()
            .unwrap_or_default();
        let phenotype = metadata.value(row_idx, "Phenotype").unwrap_or_default().to_string();
        let hyp = metadata.value(row_idx, "HYP").unwrap_or_default().to_string();
        rows.push(SampleCoverageRow {
            sample_id,
            metadata_row: Some(row_idx),
            mapping_status: MappingStatus::MetadataOnly,
            phenotype,
            hyp,
            counts: BTreeMap::new(),
        });
        metadata_only_samples += 1;
    }
    rows.sort_by(|a, b| normalize_sample_id(&a.sample_id).cmp(&normalize_sample_id(&b.sample_id)));

    CohortMapping {
        rows,
        matched_files,
        unmatched_files,
        ambiguous_files,
        metadata_only_samples,
    }
}

pub fn build_comparison(metadata: &MetadataTable, preset: ComparisonPreset) -> Result<CohortComparison> {
    let id_col = metadata
        .column_index(&metadata.id_column)
        .ok_or_else(|| anyhow!("Metadata ID column '{}' is missing", metadata.id_column))?;
    let phenotype_col = metadata
        .column_index("Phenotype")
        .ok_or_else(|| anyhow!("Metadata must contain a Phenotype column for the built-in comparisons"))?;
    let hyp_col = metadata.column_index("HYP");

    let mut id_counts: HashMap<String, usize> = HashMap::new();
    for row in &metadata.rows {
        let id = normalize_sample_id(row.get(id_col).map(String::as_str).unwrap_or_default());
        if !id.is_empty() {
            *id_counts.entry(id).or_insert(0) += 1;
        }
    }

    let mut case_ids = HashSet::new();
    let mut control_ids = HashSet::new();
    let mut case_rows = 0usize;
    let mut control_rows = 0usize;

    for row in &metadata.rows {
        let id = normalize_sample_id(row.get(id_col).map(String::as_str).unwrap_or_default());
        if id.is_empty() || id_counts.get(&id).copied().unwrap_or(0) != 1 {
            continue;
        }
        let phenotype = row
            .get(phenotype_col)
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        let hyp = hyp_col
            .and_then(|idx| row.get(idx))
            .map(String::as_str)
            .unwrap_or_default();

        let (is_case, is_control) = match preset {
            ComparisonPreset::HypVsTrueHc => (is_yes(hyp), phenotype.eq_ignore_ascii_case("True_HC")),
            ComparisonPreset::HypMecfsVsMecfsOnly => (
                phenotype.eq_ignore_ascii_case("HYP+ME/CFS"),
                phenotype.eq_ignore_ascii_case("ME/CFS") && !is_yes(hyp),
            ),
        };
        if is_case && !is_control {
            case_ids.insert(id);
            case_rows += 1;
        } else if is_control && !is_case {
            control_ids.insert(id);
            control_rows += 1;
        }
    }

    let (case_label, control_label) = match preset {
        ComparisonPreset::HypVsTrueHc => ("HYP", "True_HC"),
        ComparisonPreset::HypMecfsVsMecfsOnly => ("HYP+ME/CFS", "ME/CFS-only"),
    };
    if case_ids.is_empty() || control_ids.is_empty() {
        bail!(
            "Comparison '{}' produced {} case and {} control metadata rows",
            preset.display_name(),
            case_rows,
            control_rows
        );
    }
    Ok(CohortComparison {
        name: preset.display_name().to_string(),
        case_label: case_label.to_string(),
        control_label: control_label.to_string(),
        case_ids,
        control_ids,
        case_metadata_rows: case_rows,
        control_metadata_rows: control_rows,
    })
}

pub fn run_gene_comparison<F>(
    merged_tsv: &Path,
    comparison: &CohortComparison,
    metric: &str,
    mut progress: F,
) -> Result<GeneComparisonResult>
where
    F: FnMut(usize, usize, &str),
{
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)
        .with_context(|| format!("Could not open {}", merged_tsv.display()))?;
    let headers = reader.headers()?.clone();
    let metric_suffix = format!(" {metric}");
    let mut case_cols = Vec::new();
    let mut ctrl_cols = Vec::new();
    let mut unmatched_columns = 0usize;
    for (idx, header) in headers.iter().enumerate().skip(2) {
        if let Some(sample) = header.strip_suffix(&metric_suffix) {
            let key = normalize_sample_id(sample);
            if comparison.case_ids.contains(&key) {
                case_cols.push(idx);
            } else if comparison.control_ids.contains(&key) {
                ctrl_cols.push(idx);
            } else {
                unmatched_columns += 1;
            }
        }
    }
    if case_cols.len() < 2 || ctrl_cols.len() < 2 {
        bail!(
            "Merged gene table has only {} case and {} control '{metric}' columns after metadata matching",
            case_cols.len(),
            ctrl_cols.len()
        );
    }

    let total_rows = count_delimited_rows(merged_tsv, b'\t')?.saturating_sub(1);
    let mut tests = Vec::with_capacity(total_rows.min(30_000));
    for (row_idx, record) in reader.records().enumerate() {
        let record = record?;
        let feature = record.get(0).unwrap_or_default().to_string();
        let secondary_id = record.get(1).unwrap_or_default().to_string();
        let case_raw = values_at(&record, &case_cols);
        let ctrl_raw = values_at(&record, &ctrl_cols);
        if case_raw.len() < 2 || ctrl_raw.len() < 2 {
            continue;
        }
        let case_log: Vec<f64> = case_raw.iter().map(|x| (x.max(0.0) + 1.0).log2()).collect();
        let ctrl_log: Vec<f64> = ctrl_raw.iter().map(|x| (x.max(0.0) + 1.0).log2()).collect();
        let p_value = mann_whitney_p(&case_log, &ctrl_log);
        let mean_case = mean(&case_raw);
        let mean_control = mean(&ctrl_raw);
        let effect = ((mean_case + 0.5) / (mean_control + 0.5)).log2();
        tests.push(FeatureTestResult {
            feature,
            secondary_id,
            n_case: case_raw.len(),
            n_control: ctrl_raw.len(),
            mean_case,
            mean_control,
            median_case: median(&case_raw),
            median_control: median(&ctrl_raw),
            effect,
            cliffs_delta: cliffs_delta(&case_log, &ctrl_log),
            p_value,
            q_value: f64::NAN,
            detection_case: Some(case_raw.iter().filter(|x| **x > 0.0).count() as f64 / case_raw.len() as f64),
            detection_control: Some(ctrl_raw.iter().filter(|x| **x > 0.0).count() as f64 / ctrl_raw.len() as f64),
        });
        if row_idx % 250 == 0 {
            progress(row_idx + 1, total_rows.max(1), &format!("Testing {metric} expression"));
        }
    }
    apply_bh(&mut tests);
    tests.sort_by(feature_result_order);
    progress(total_rows.max(1), total_rows.max(1), "Gene comparison complete");
    Ok(GeneComparisonResult {
        input: merged_tsv.to_path_buf(),
        metric: metric.to_string(),
        n_case_columns: case_cols.len(),
        n_control_columns: ctrl_cols.len(),
        unmatched_columns,
        tests,
    })
}

pub fn run_bcr_comparison<F>(
    files: &[DetectedCohortFile],
    comparison: &CohortComparison,
    mut progress: F,
) -> Result<BcrComparisonResult>
where
    F: FnMut(usize, usize, &str),
{
    let all_bcr_files: Vec<_> = files.iter().filter(|f| f.modality == CohortModality::Bcr).collect();
    if all_bcr_files.is_empty() {
        bail!("No BCR heavy-chain report files were detected");
    }
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut file_errors = Vec::new();
    for file in all_bcr_files {
        let key = normalize_sample_id(&file.sample_id);
        if let Some(first) = chosen.get(&key) {
            file_errors.push(format!(
                "Duplicate BCR sample '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let bcr_files: Vec<_> = chosen.into_values().collect();
    let total = bcr_files.len();
    let mut files_used = 0usize;
    let mut skipped_not_in_groups = 0usize;
    let mut summaries = Vec::new();
    let mut feature_maps: HashMap<&'static str, Vec<(String, String, HashMap<String, f64>)>> = HashMap::new();
    feature_maps.insert("isotype", Vec::new());
    feature_maps.insert("v", Vec::new());
    feature_maps.insert("d", Vec::new());
    feature_maps.insert("j", Vec::new());

    for (i, file) in bcr_files.iter().enumerate() {
        let key = normalize_sample_id(&file.sample_id);
        let group = if comparison.case_ids.contains(&key) {
            comparison.case_label.clone()
        } else if comparison.control_ids.contains(&key) {
            comparison.control_label.clone()
        } else {
            skipped_not_in_groups += 1;
            continue;
        };
        progress(i + 1, total.max(1), &format!("BCR: {}", file.sample_id));
        let parsed = match parse_bcr_file(&file.path)
            .with_context(|| format!("While reading {}", file.path.display()))
        {
            Ok(parsed) => parsed,
            Err(e) => {
                file_errors.push(format!("{}: {e:#}", file.path.display()));
                continue;
            }
        };
        files_used += 1;
        summaries.push(BcrSampleSummary {
            sample_id: file.sample_id.clone(),
            group: group.clone(),
            total_clone_count: parsed.total_clone_count,
            unique_clones: parsed.unique_clones as f64,
            shannon: parsed.shannon,
            simpson: parsed.simpson,
            clonality: parsed.clonality,
            effective_n: parsed.effective_n,
            top_clone_fraction: parsed.top_clone_fraction,
            weighted_mean_cdr3_aa_length: parsed.weighted_mean_cdr3_aa_length,
        });
        for (name, map) in [
            ("isotype", parsed.isotype_fraction),
            ("v", parsed.v_fraction),
            ("d", parsed.d_fraction),
            ("j", parsed.j_fraction),
        ] {
            feature_maps
                .get_mut(name)
                .unwrap()
                .push((file.sample_id.clone(), group.clone(), map));
        }
    }

    let metric_tests = compare_bcr_metrics(&summaries, comparison);
    let isotype_tests = compare_feature_maps(feature_maps.remove("isotype").unwrap(), comparison);
    let v_gene_tests = compare_feature_maps(feature_maps.remove("v").unwrap(), comparison);
    let d_gene_tests = compare_feature_maps(feature_maps.remove("d").unwrap(), comparison);
    let j_gene_tests = compare_feature_maps(feature_maps.remove("j").unwrap(), comparison);

    Ok(BcrComparisonResult {
        files_used,
        files_skipped_unmatched: 0,
        files_skipped_not_in_groups: skipped_not_in_groups,
        file_errors,
        sample_summaries: summaries,
        metric_tests,
        isotype_tests,
        v_gene_tests,
        d_gene_tests,
        j_gene_tests,
    })
}

pub fn run_bcr_isotype_comparison<F>(
    files: &[DetectedCohortFile],
    comparison: &CohortComparison,
    mut progress: F,
) -> Result<BcrIsotypeComparisonResult>
where
    F: FnMut(usize, usize, &str),
{
    let all_iso_files: Vec<_> = files.iter().filter(|f| f.modality == CohortModality::BcrIsotype).collect();
    if all_iso_files.is_empty() {
        bail!("No BCR isotype-composition files were detected");
    }
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut file_errors = Vec::new();
    for file in all_iso_files {
        let key = normalize_sample_id(&file.sample_id);
        if let Some(first) = chosen.get(&key) {
            file_errors.push(format!(
                "Duplicate BCR-isotype sample '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let iso_files: Vec<_> = chosen.into_values().collect();
    let total = iso_files.len();
    let mut files_used = 0usize;
    let mut skipped_not_in_groups = 0usize;
    let mut values = Vec::new();
    let mut maps = Vec::new();
    for (i, file) in iso_files.iter().enumerate() {
        let key = normalize_sample_id(&file.sample_id);
        let group = if comparison.case_ids.contains(&key) {
            comparison.case_label.clone()
        } else if comparison.control_ids.contains(&key) {
            comparison.control_label.clone()
        } else {
            skipped_not_in_groups += 1;
            continue;
        };
        progress(i + 1, total.max(1), &format!("BCR isotype: {}", file.sample_id));
        let map = match parse_bcr_isotype_file(&file.path) {
            Ok(map) => map,
            Err(e) => {
                file_errors.push(format!("{}: {e:#}", file.path.display()));
                continue;
            }
        };
        for (isotype, fraction) in &map {
            values.push(BcrIsotypeSampleValue {
                sample_id: file.sample_id.clone(),
                group: group.clone(),
                isotype: isotype.clone(),
                fraction: *fraction,
            });
        }
        maps.push((file.sample_id.clone(), group, map));
        files_used += 1;
    }
    let tests = compare_feature_maps(maps, comparison);
    Ok(BcrIsotypeComparisonResult {
        files_used,
        files_skipped_unmatched: 0,
        files_skipped_not_in_groups: skipped_not_in_groups,
        file_errors,
        values,
        tests,
    })
}

#[derive(Debug)]
struct ParsedBcr {
    total_clone_count: f64,
    unique_clones: usize,
    shannon: f64,
    simpson: f64,
    clonality: f64,
    effective_n: f64,
    top_clone_fraction: f64,
    weighted_mean_cdr3_aa_length: f64,
    isotype_fraction: HashMap<String, f64>,
    v_fraction: HashMap<String, f64>,
    d_fraction: HashMap<String, f64>,
    j_fraction: HashMap<String, f64>,
}

fn parse_bcr_file(path: &Path) -> Result<ParsedBcr> {
    let (headers, rows) = read_first_sheet(path)?;
    let columns = header_map(&headers);
    let count_idx = find_header(&columns, &["Clone Count"])
        .ok_or_else(|| anyhow!("Missing 'Clone Count' column"))?;
    let isotype_idx = find_header(&columns, &["Isotype"]);
    let v_idx = find_header(&columns, &["Top V Hits"]);
    let d_idx = find_header(&columns, &["Top D Hits"]);
    let j_idx = find_header(&columns, &["Top J Hits"]);
    let cdr3_idx = find_header(&columns, &["CDR3 Amino Acid Sequence"]);

    let mut counts = Vec::new();
    let mut total = 0.0;
    let mut top = 0.0_f64;
    let mut weighted_len_sum = 0.0;
    let mut weighted_len_den = 0.0;
    let mut iso = HashMap::<String, f64>::new();
    let mut v = HashMap::<String, f64>::new();
    let mut d = HashMap::<String, f64>::new();
    let mut j = HashMap::<String, f64>::new();
    for row in rows {
        let count = row.get(count_idx).and_then(cell_f64).unwrap_or(0.0);
        if count <= 0.0 {
            continue;
        }
        counts.push(count);
        total += count;
        top = top.max(count);
        if let Some(idx) = cdr3_idx {
            let text = row.get(idx).map(cell_text).unwrap_or_default();
            if !text.is_empty() {
                weighted_len_sum += count * text.chars().count() as f64;
                weighted_len_den += count;
            }
        }
        accumulate_feature(&mut iso, row.get(isotype_idx.unwrap_or(usize::MAX)), count, false);
        accumulate_feature(&mut v, row.get(v_idx.unwrap_or(usize::MAX)), count, true);
        accumulate_feature(&mut d, row.get(d_idx.unwrap_or(usize::MAX)), count, true);
        accumulate_feature(&mut j, row.get(j_idx.unwrap_or(usize::MAX)), count, true);
    }
    if total <= 0.0 {
        bail!("No positive clone counts");
    }
    normalize_feature_map(&mut iso, total);
    normalize_feature_map(&mut v, total);
    normalize_feature_map(&mut d, total);
    normalize_feature_map(&mut j, total);
    let shannon = shannon_entropy(&counts);
    let simpson = simpson_diversity(&counts);
    let clonality = if counts.len() > 1 {
        1.0 - shannon / (counts.len() as f64).ln()
    } else {
        f64::NAN
    };
    Ok(ParsedBcr {
        total_clone_count: total,
        unique_clones: counts.len(),
        shannon,
        simpson,
        clonality,
        effective_n: shannon.exp(),
        top_clone_fraction: top / total,
        weighted_mean_cdr3_aa_length: if weighted_len_den > 0.0 {
            weighted_len_sum / weighted_len_den
        } else {
            f64::NAN
        },
        isotype_fraction: iso,
        v_fraction: v,
        d_fraction: d,
        j_fraction: j,
    })
}

fn parse_bcr_isotype_file(path: &Path) -> Result<HashMap<String, f64>> {
    let (headers, rows) = read_first_sheet(path)?;
    let columns = header_map(&headers);
    let value_idx = find_header(&columns, &["Isotype Read Fraction"])
        .ok_or_else(|| anyhow!("Missing 'Isotype Read Fraction' column"))?;
    let feature_idx = find_header(&columns, &["IGH Isotype", "Isotype"])
        .ok_or_else(|| anyhow!("Missing 'IGH Isotype' column"))?;
    let mut out = HashMap::new();
    for row in rows {
        let feature = row.get(feature_idx).map(cell_text).unwrap_or_default();
        let value = row.get(value_idx).and_then(cell_f64).unwrap_or(f64::NAN);
        if !feature.is_empty() && value.is_finite() {
            *out.entry(feature).or_insert(0.0) += value;
        }
    }
    Ok(out)
}

fn compare_bcr_metrics(rows: &[BcrSampleSummary], comparison: &CohortComparison) -> Vec<FeatureTestResult> {
    let specs: [(&str, fn(&BcrSampleSummary) -> f64); 8] = [
        ("Total clone count", |r| r.total_clone_count),
        ("Unique clones", |r| r.unique_clones),
        ("Shannon diversity", |r| r.shannon),
        ("Simpson diversity", |r| r.simpson),
        ("Clonality", |r| r.clonality),
        ("Effective clonotypes", |r| r.effective_n),
        ("Top clone fraction", |r| r.top_clone_fraction),
        ("Weighted mean CDR3 AA length", |r| r.weighted_mean_cdr3_aa_length),
    ];
    let mut out = Vec::new();
    for (name, getter) in specs {
        let case = rows
            .iter()
            .filter(|r| r.group == comparison.case_label)
            .map(getter)
            .filter(|x| x.is_finite())
            .collect::<Vec<_>>();
        let ctrl = rows
            .iter()
            .filter(|r| r.group == comparison.control_label)
            .map(getter)
            .filter(|x| x.is_finite())
            .collect::<Vec<_>>();
        if case.len() < 2 || ctrl.len() < 2 {
            continue;
        }
        out.push(make_feature_test(name.to_string(), String::new(), &case, &ctrl, false));
    }
    apply_bh(&mut out);
    out.sort_by(feature_result_order);
    out
}

fn compare_feature_maps(
    sample_maps: Vec<(String, String, HashMap<String, f64>)>,
    comparison: &CohortComparison,
) -> Vec<FeatureTestResult> {
    let features: BTreeSet<String> = sample_maps
        .iter()
        .flat_map(|(_, _, map)| map.keys().cloned())
        .collect();
    let mut out = Vec::new();
    for feature in features {
        let case = sample_maps
            .iter()
            .filter(|(_, group, _)| group == &comparison.case_label)
            .map(|(_, _, map)| *map.get(&feature).unwrap_or(&0.0))
            .collect::<Vec<_>>();
        let ctrl = sample_maps
            .iter()
            .filter(|(_, group, _)| group == &comparison.control_label)
            .map(|(_, _, map)| *map.get(&feature).unwrap_or(&0.0))
            .collect::<Vec<_>>();
        if case.len() < 2 || ctrl.len() < 2 {
            continue;
        }
        out.push(make_feature_test(feature, String::new(), &case, &ctrl, true));
    }
    apply_bh(&mut out);
    out.sort_by(feature_result_order);
    out
}

fn make_feature_test(
    feature: String,
    secondary_id: String,
    case: &[f64],
    control: &[f64],
    fractions: bool,
) -> FeatureTestResult {
    FeatureTestResult {
        feature,
        secondary_id,
        n_case: case.len(),
        n_control: control.len(),
        mean_case: mean(case),
        mean_control: mean(control),
        median_case: median(case),
        median_control: median(control),
        effect: median(case) - median(control),
        cliffs_delta: cliffs_delta(case, control),
        p_value: mann_whitney_p(case, control),
        q_value: f64::NAN,
        detection_case: if fractions {
            Some(case.iter().filter(|x| **x > 0.0).count() as f64 / case.len() as f64)
        } else {
            None
        },
        detection_control: if fractions {
            Some(control.iter().filter(|x| **x > 0.0).count() as f64 / control.len() as f64)
        } else {
            None
        },
    }
}

fn read_first_sheet(path: &Path) -> Result<(Vec<String>, Vec<Vec<Data>>)> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("Could not open workbook {}", path.display()))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("Workbook has no worksheets"))?;
    let range = workbook.worksheet_range(&sheet_name)?;
    let mut iter = range.rows();
    let header = iter.next().ok_or_else(|| anyhow!("Worksheet is empty"))?;
    let headers = header.iter().map(cell_text).collect();
    let rows = iter.map(|row| row.to_vec()).collect();
    Ok((headers, rows))
}

fn header_map(headers: &[String]) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_ascii_lowercase(), i))
        .collect()
}

fn find_header(columns: &HashMap<String, usize>, candidates: &[&str]) -> Option<usize> {
    candidates
        .iter()
        .find_map(|name| columns.get(&name.to_ascii_lowercase()).copied())
}

fn accumulate_feature(map: &mut HashMap<String, f64>, cell: Option<&Data>, count: f64, first_only: bool) {
    let Some(cell) = cell else { return; };
    let mut feature = cell_text(cell);
    if first_only {
        feature = feature.split(',').next().unwrap_or_default().trim().to_string();
    }
    if feature.is_empty() {
        return;
    }
    *map.entry(feature).or_insert(0.0) += count;
}

fn normalize_feature_map(map: &mut HashMap<String, f64>, total: f64) {
    if total > 0.0 {
        for value in map.values_mut() {
            *value /= total;
        }
    }
}

fn values_at(record: &csv::StringRecord, cols: &[usize]) -> Vec<f64> {
    cols.iter()
        .filter_map(|idx| record.get(*idx))
        .filter_map(|text| text.trim().parse::<f64>().ok())
        .filter(|x| x.is_finite())
        .collect()
}

fn count_delimited_rows(path: &Path, delimiter: u8) -> Result<usize> {
    let mut reader = ReaderBuilder::new().delimiter(delimiter).from_path(path)?;
    let mut n = 1usize;
    for row in reader.records() {
        row?;
        n += 1;
    }
    Ok(n)
}

pub fn normalize_sample_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn is_yes(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "y" | "true" | "1"
    )
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(v) => {
            if v.fract() == 0.0 {
                format!("{v:.0}")
            } else {
                v.to_string()
            }
        }
        Data::Int(v) => v.to_string(),
        Data::Bool(v) => v.to_string(),
        other => other.to_string(),
    }
}

fn cell_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(v) => Some(*v),
        Data::Int(v) => Some(*v as f64),
        Data::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn shannon_entropy(counts: &[f64]) -> f64 {
    let total: f64 = counts.iter().copied().filter(|x| *x > 0.0).sum();
    if total <= 0.0 {
        return f64::NAN;
    }
    counts
        .iter()
        .copied()
        .filter(|x| *x > 0.0)
        .map(|x| {
            let p = x / total;
            -p * p.ln()
        })
        .sum()
}

fn simpson_diversity(counts: &[f64]) -> f64 {
    let total: f64 = counts.iter().copied().filter(|x| *x > 0.0).sum();
    if total <= 0.0 {
        return f64::NAN;
    }
    1.0 - counts
        .iter()
        .copied()
        .filter(|x| *x > 0.0)
        .map(|x| (x / total).powi(2))
        .sum::<f64>()
}

fn cliffs_delta(x: &[f64], y: &[f64]) -> f64 {
    if x.is_empty() || y.is_empty() {
        return f64::NAN;
    }
    let mut y_sorted = y.to_vec();
    y_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let mut gt = 0usize;
    let mut lt = 0usize;
    for xi in x {
        let less = y_sorted.partition_point(|v| v < xi);
        let leq = y_sorted.partition_point(|v| v <= xi);
        gt += less;
        lt += y_sorted.len() - leq;
    }
    (gt as f64 - lt as f64) / (x.len() * y.len()) as f64
}

fn mann_whitney_p(x: &[f64], y: &[f64]) -> f64 {
    if x.len() < 2 || y.len() < 2 {
        return f64::NAN;
    }
    let mut combined = Vec::with_capacity(x.len() + y.len());
    combined.extend(x.iter().copied().map(|v| (v, true)));
    combined.extend(y.iter().copied().map(|v| (v, false)));
    combined.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

    let mut rank_sum_x = 0.0;
    let mut tie_sum = 0.0;
    let mut i = 0usize;
    while i < combined.len() {
        let mut j = i + 1;
        while j < combined.len() && combined[j].0 == combined[i].0 {
            j += 1;
        }
        let avg_rank = ((i + 1 + j) as f64) / 2.0;
        for item in &combined[i..j] {
            if item.1 {
                rank_sum_x += avg_rank;
            }
        }
        let t = (j - i) as f64;
        if t > 1.0 {
            tie_sum += t * t * t - t;
        }
        i = j;
    }
    let n1 = x.len() as f64;
    let n2 = y.len() as f64;
    let n = n1 + n2;
    let u1 = rank_sum_x - n1 * (n1 + 1.0) / 2.0;
    let mean_u = n1 * n2 / 2.0;
    let variance = n1 * n2 / 12.0 * ((n + 1.0) - tie_sum / (n * (n - 1.0)));
    if variance <= 0.0 || !variance.is_finite() {
        return 1.0;
    }
    let z = (u1 - mean_u).abs() / variance.sqrt();
    (2.0 * (1.0 - normal_cdf(z))).clamp(0.0, 1.0)
}

fn normal_cdf(x: f64) -> f64 {
    // Abramowitz-Stegun 7.1.26 approximation.
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let z = x.abs() / 2.0_f64.sqrt();
    let t = 1.0 / (1.0 + 0.3275911 * z);
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let erf = 1.0
        - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-z * z).exp();
    0.5 * (1.0 + sign * erf)
}

fn apply_bh(results: &mut [FeatureTestResult]) {
    let mut indexed = results
        .iter()
        .enumerate()
        .filter(|(_, r)| r.p_value.is_finite())
        .map(|(i, r)| (i, r.p_value))
        .collect::<Vec<_>>();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    let m = indexed.len();
    let mut running = 1.0_f64;
    for rank_rev in (0..m).rev() {
        let (idx, p) = indexed[rank_rev];
        let rank = rank_rev + 1;
        let q = (p * m as f64 / rank as f64).min(running).clamp(0.0, 1.0);
        results[idx].q_value = q;
        running = q;
    }
}

fn feature_result_order(a: &FeatureTestResult, b: &FeatureTestResult) -> Ordering {
    let aq = if a.q_value.is_finite() { a.q_value } else { 2.0 };
    let bq = if b.q_value.is_finite() { b.q_value } else { 2.0 };
    aq.partial_cmp(&bq)
        .unwrap_or(Ordering::Equal)
        .then_with(|| {
            b.effect
                .abs()
                .partial_cmp(&a.effect.abs())
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| a.feature.cmp(&b.feature))
}

pub fn write_feature_tests(path: &Path, tests: &[FeatureTestResult]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    writer.write_record([
        "feature",
        "secondary_id",
        "n_case",
        "n_control",
        "mean_case",
        "mean_control",
        "median_case",
        "median_control",
        "effect",
        "cliffs_delta",
        "p_value",
        "q_value",
        "detection_case",
        "detection_control",
    ])?;
    for r in tests {
        writer.write_record([
            r.feature.clone(),
            r.secondary_id.clone(),
            r.n_case.to_string(),
            r.n_control.to_string(),
            r.mean_case.to_string(),
            r.mean_control.to_string(),
            r.median_case.to_string(),
            r.median_control.to_string(),
            r.effect.to_string(),
            r.cliffs_delta.to_string(),
            r.p_value.to_string(),
            r.q_value.to_string(),
            r.detection_case.map(|x| x.to_string()).unwrap_or_default(),
            r.detection_control.map(|x| x.to_string()).unwrap_or_default(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

pub fn write_bcr_sample_summaries(path: &Path, rows: &[BcrSampleSummary]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    writer.write_record([
        "sample_id",
        "group",
        "total_clone_count",
        "unique_clones",
        "shannon",
        "simpson",
        "clonality",
        "effective_n",
        "top_clone_fraction",
        "weighted_mean_cdr3_aa_length",
    ])?;
    for r in rows {
        writer.write_record([
            r.sample_id.clone(),
            r.group.clone(),
            r.total_clone_count.to_string(),
            r.unique_clones.to_string(),
            r.shannon.to_string(),
            r.simpson.to_string(),
            r.clonality.to_string(),
            r.effective_n.to_string(),
            r.top_clone_fraction.to_string(),
            r.weighted_mean_cdr3_aa_length.to_string(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

pub fn write_bcr_isotype_values(path: &Path, rows: &[BcrIsotypeSampleValue]) -> Result<()> {
    let mut writer = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    writer.write_record(["sample_id", "group", "isotype", "fraction"])?;
    for r in rows {
        writer.write_record([
            r.sample_id.clone(),
            r.group.clone(),
            r.isotype.clone(),
            r.fraction.to_string(),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

pub fn write_mapping_audit(path: &Path, mapping: &CohortMapping) -> Result<()> {
    let mut writer = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    writer.write_record([
        "sample_id",
        "mapping_status",
        "phenotype",
        "hyp",
        "gene_files",
        "transcript_files",
        "bcr_files",
        "bcr_isotype_files",
        "tcr_alpha_files",
        "tcr_beta_files",
        "immune_infiltrate_files",
    ])?;
    for r in &mapping.rows {
        let status = match r.mapping_status {
            MappingStatus::Matched => "matched",
            MappingStatus::Unmatched => "unmatched",
            MappingStatus::Ambiguous => "ambiguous",
            MappingStatus::MetadataOnly => "metadata_only",
        };
        let count = |m| r.counts.get(&m).copied().unwrap_or(0).to_string();
        writer.write_record([
            r.sample_id.clone(),
            status.to_string(),
            r.phenotype.clone(),
            r.hyp.clone(),
            count(CohortModality::GeneExpression),
            count(CohortModality::TranscriptExpression),
            count(CohortModality::Bcr),
            count(CohortModality::BcrIsotype),
            count(CohortModality::TcrAlpha),
            count(CohortModality::TcrBeta),
            count(CohortModality::ImmuneInfiltrate),
        ])?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_workbook_names_and_copy_suffixes() {
        let (kind, id) = classify_workbook_name("4C_101_bcr_heavy_chain_report.xlsx").unwrap();
        assert_eq!(kind, CohortModality::Bcr);
        assert_eq!(id, "4C_101");

        let (kind, id) = classify_workbook_name(
            "4C_465_tumor_rna_gene_expression_report (1).xlsx",
        )
        .unwrap();
        assert_eq!(kind, CohortModality::GeneExpression);
        assert_eq!(id, "4C_465");
    }

    #[test]
    fn normalizes_sample_ids_without_punctuation() {
        assert_eq!(normalize_sample_id("4C_CHBA-2-25-18"), "4CCHBA22518");
        assert_eq!(normalize_sample_id(" 4c_101 "), "4C101");
    }

    #[test]
    fn builds_requested_comparison_presets() {
        let metadata = MetadataTable {
            path: PathBuf::from("metadata.xlsx"),
            columns: vec![
                "4C_ID".to_string(),
                "Phenotype".to_string(),
                "HYP".to_string(),
            ],
            rows: vec![
                vec!["4C_100".into(), "True_HC".into(), "N".into()],
                vec!["4C_104".into(), "ME/CFS".into(), "N".into()],
                vec!["4C_106".into(), "HYP+ME/CFS".into(), "Yes".into()],
                vec!["4C_107".into(), "HYP".into(), "Yes".into()],
            ],
            id_column: "4C_ID".to_string(),
        };

        let discovery = build_comparison(&metadata, ComparisonPreset::HypVsTrueHc).unwrap();
        assert_eq!(discovery.case_ids.len(), 2);
        assert_eq!(discovery.control_ids.len(), 1);

        let specificity =
            build_comparison(&metadata, ComparisonPreset::HypMecfsVsMecfsOnly).unwrap();
        assert_eq!(specificity.case_ids.len(), 1);
        assert_eq!(specificity.control_ids.len(), 1);
    }

    #[test]
    fn bh_adjustment_is_monotone_in_ranked_results() {
        let mut rows = vec![
            make_feature_test("a".into(), String::new(), &[1.0, 2.0], &[8.0, 9.0], false),
            make_feature_test("b".into(), String::new(), &[1.0, 2.0], &[1.0, 2.0], false),
            make_feature_test("c".into(), String::new(), &[1.0, 2.0], &[4.0, 5.0], false),
        ];
        apply_bh(&mut rows);
        assert!(rows.iter().all(|r| r.q_value.is_finite()));
        assert!(rows.iter().all(|r| r.q_value >= r.p_value - 1e-12));
    }
}
