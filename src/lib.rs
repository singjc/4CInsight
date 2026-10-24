pub mod analysis;
pub mod analysis_docs;
pub mod cohort;
pub mod corrected;
pub mod diagnostics;
pub mod focused;
pub mod gmt_catalog;
pub mod hit_structure;
pub mod input_conventions;
pub mod longitudinal;
pub mod metadata;
pub mod outliers;
pub mod portable_io;
pub mod report;

use anyhow::{Context, Result, anyhow, bail};
use calamine::{Data, Range, Reader, Xlsx, open_workbook};
use csv::{ReaderBuilder, WriterBuilder};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const CELL_BYTES: usize = 9;
const TARGET_CHUNK_BYTES: usize = 64 * 1024 * 1024;
const MAX_SAMPLES_PER_CHUNK: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    Auto,
    Gene,
    Transcript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnOrder {
    AdjacentBySample,
    GroupedByMeasurement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Metric {
    RawCount,
    Fpkm,
    Cpm,
    Tpm,
    PercentileRank,
    IsExpressed,
    TranscriptLength,
    EffectiveTranscriptLength,
    TotalReads,
}

impl Metric {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::RawCount => "Raw Count",
            Self::Fpkm => "FPKM",
            Self::Cpm => "CPM",
            Self::Tpm => "TPM",
            Self::PercentileRank => "Percentile Rank",
            Self::IsExpressed => "Is Expressed",
            Self::TranscriptLength => "Transcript Length",
            Self::EffectiveTranscriptLength => "Effective Transcript Length",
            Self::TotalReads => "Total Reads",
        }
    }

    fn input_header(self) -> &'static str {
        match self {
            Self::RawCount => "RNA-Seq Raw Counts",
            Self::Fpkm => "FPKM",
            Self::Cpm => "CPM",
            Self::Tpm => "TPM",
            Self::PercentileRank => "Percentile Rank",
            Self::IsExpressed => "Is Expressed",
            Self::TranscriptLength => "Transcript Length",
            Self::EffectiveTranscriptLength => "Effective Transcript Length",
            Self::TotalReads => "Total Reads",
        }
    }

    pub fn valid_for(self, kind: DataKind) -> bool {
        match kind {
            DataKind::Gene => matches!(
                self,
                Self::RawCount
                    | Self::Fpkm
                    | Self::Cpm
                    | Self::Tpm
                    | Self::PercentileRank
                    | Self::IsExpressed
            ),
            DataKind::Transcript => matches!(
                self,
                Self::TranscriptLength
                    | Self::EffectiveTranscriptLength
                    | Self::TotalReads
                    | Self::Tpm
                    | Self::PercentileRank
                    | Self::IsExpressed
            ),
            DataKind::Auto => true,
        }
    }

    /// Whether this measurement can be represented as a continuous numeric value
    /// in the built-in plotting/analysis views.
    pub fn is_numeric(self) -> bool {
        self != Self::IsExpressed
    }
}

pub fn collaborator_default_metrics(kind: DataKind) -> Vec<Metric> {
    match kind {
        DataKind::Gene => vec![Metric::RawCount, Metric::Tpm],
        DataKind::Transcript => vec![Metric::TotalReads, Metric::Tpm],
        DataKind::Auto => vec![Metric::RawCount, Metric::TotalReads, Metric::Tpm],
    }
}

pub fn all_metrics(kind: DataKind) -> Vec<Metric> {
    match kind {
        DataKind::Gene => vec![
            Metric::RawCount,
            Metric::Fpkm,
            Metric::Cpm,
            Metric::Tpm,
            Metric::PercentileRank,
            Metric::IsExpressed,
        ],
        DataKind::Transcript => vec![
            Metric::TranscriptLength,
            Metric::EffectiveTranscriptLength,
            Metric::TotalReads,
            Metric::Tpm,
            Metric::PercentileRank,
            Metric::IsExpressed,
        ],
        DataKind::Auto => Vec::new(),
    }
}

#[derive(Debug, Clone)]
pub struct MergeConfig {
    pub files: Vec<PathBuf>,
    pub output: PathBuf,
    pub kind: DataKind,
    pub metrics: Vec<Metric>,
    pub column_order: ColumnOrder,
}

#[derive(Debug, Clone)]
pub struct MergeSummary {
    pub kind: DataKind,
    pub samples: usize,
    pub rows: usize,
    pub metrics: Vec<Metric>,
    pub output: PathBuf,
    pub missing_rows: usize,
    pub unknown_rows: usize,
}

/// Searchable identifier metadata from the first workbook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCatalogEntry {
    /// Gene symbol for gene reports, transcript accession/name for transcript reports.
    pub primary: String,
    pub gene_id: String,
    pub gene_symbol: String,
}

/// Per-sample summary used by interactive QC distribution plots.
#[derive(Debug, Clone)]
pub struct SampleDistributionSummary {
    pub sample: String,
    pub n_total: usize,
    pub n_values: usize,
    pub n_missing: usize,
    pub n_zero: usize,
    pub mean: Option<f64>,
    pub lower_whisker: Option<f64>,
    pub q1: Option<f64>,
    pub median: Option<f64>,
    pub q3: Option<f64>,
    pub upper_whisker: Option<f64>,
}

impl SampleDistributionSummary {
    pub fn nonzero_fraction(&self) -> Option<f64> {
        let observed = self.n_total.saturating_sub(self.n_missing);
        if observed == 0 {
            None
        } else {
            Some((observed.saturating_sub(self.n_zero)) as f64 / observed as f64)
        }
    }
}

/// A single value for one selected gene/transcript in one sample.
#[derive(Debug, Clone)]
pub struct EntityProfilePoint {
    pub sample: String,
    pub value: Option<f64>,
}

/// Header-level metadata for a previously merged TSV file.
#[derive(Debug, Clone)]
pub struct MergedDatasetInfo {
    pub kind: DataKind,
    pub samples: Vec<String>,
    pub metrics: Vec<Metric>,
}

#[derive(Debug, Clone)]
struct MergedValueColumn {
    index: usize,
    sample: String,
    metric: Metric,
}

#[derive(Debug, Clone)]
struct MergedSchema {
    info: MergedDatasetInfo,
    value_columns: Vec<MergedValueColumn>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct EntityKey {
    primary: String,
    gene_id: String,
}

#[derive(Debug, Clone)]
struct EntityMeta {
    key: EntityKey,
}

#[derive(Debug, Clone)]
struct Schema {
    kind: DataKind,
    columns: HashMap<String, usize>,
}

#[derive(Clone, Copy, Debug)]
struct StoredCell {
    tag: u8,
    value: f64,
}

impl StoredCell {
    const MISSING: Self = Self { tag: 0, value: 0.0 };

    fn numeric(value: f64) -> Self {
        Self { tag: 1, value }
    }

    fn yes() -> Self {
        Self { tag: 2, value: 0.0 }
    }

    fn no() -> Self {
        Self { tag: 3, value: 0.0 }
    }

    fn write_to<W: Write>(self, mut w: W) -> Result<()> {
        w.write_all(&[self.tag])?;
        w.write_all(&self.value.to_le_bytes())?;
        Ok(())
    }

    fn read_from<R: Read>(mut r: R) -> Result<Self> {
        let mut tag = [0u8; 1];
        let mut bytes = [0u8; 8];
        r.read_exact(&mut tag)?;
        r.read_exact(&mut bytes)?;
        Ok(Self {
            tag: tag[0],
            value: f64::from_le_bytes(bytes),
        })
    }

    fn render(self) -> String {
        match self.tag {
            0 => String::new(),
            1 => format_number(self.value),
            2 => "yes".to_string(),
            3 => "no".to_string(),
            _ => String::new(),
        }
    }
}

pub fn discover_xlsx_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder)
        .with_context(|| format!("cannot read input folder {}", folder.display()))?
    {
        let path = entry?.path();
        if path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.eq_ignore_ascii_case("xlsx"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

pub fn inspect_kind(path: &Path) -> Result<DataKind> {
    let (schema, _, _) = scan_first(path, DataKind::Auto)?;
    Ok(schema.kind)
}

pub fn merge_wide<F>(mut config: MergeConfig, mut progress: F) -> Result<MergeSummary>
where
    F: FnMut(usize, usize, &str),
{
    if config.files.is_empty() {
        bail!("no input files were selected");
    }
    config.files.sort();

    progress(0, config.files.len(), "Inspecting first workbook");
    let (schema, entities, index) = scan_first(&config.files[0], config.kind)?;
    let kind = schema.kind;

    if config.metrics.is_empty() {
        config.metrics = collaborator_default_metrics(kind);
    }
    config.metrics.retain(|m| m.valid_for(kind));
    if config.metrics.is_empty() {
        bail!("no valid measurements were selected for {kind:?} data");
    }

    validate_required_columns(&schema, &config.metrics)?;

    let n_rows = entities.len();
    let n_metrics = config.metrics.len();
    let bytes_per_sample = n_rows
        .saturating_mul(n_metrics)
        .saturating_mul(CELL_BYTES)
        .max(1);
    let samples_per_chunk = (TARGET_CHUNK_BYTES / bytes_per_sample).clamp(1, MAX_SAMPLES_PER_CHUNK);

    let tempdir = TempDir::new().context("cannot create temporary working directory")?;
    let mut chunk_paths = Vec::new();
    let mut missing_rows_total = 0usize;
    let mut unknown_rows_total = 0usize;

    for (chunk_index, group) in config.files.chunks(samples_per_chunk).enumerate() {
        let mut sample_cells = Vec::with_capacity(group.len());
        let chunk_start = chunk_index * samples_per_chunk;
        for (local_idx, path) in group.iter().enumerate() {
            let sample_no = chunk_start + local_idx + 1;
            progress(
                sample_no,
                config.files.len(),
                &format!(
                    "Reading {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            );
            let (cells, missing, unknown) =
                read_sample(path, kind, &config.metrics, &index, n_rows)?;
            missing_rows_total += missing;
            unknown_rows_total += unknown;
            sample_cells.push(cells);
        }

        let chunk_path = tempdir.path().join(format!("chunk_{chunk_index:05}.bin"));
        let mut writer = BufWriter::new(File::create(&chunk_path)?);
        for row_idx in 0..n_rows {
            let base = row_idx * n_metrics;
            for sample in &sample_cells {
                for metric_idx in 0..n_metrics {
                    sample[base + metric_idx].write_to(&mut writer)?;
                }
            }
        }
        writer.flush()?;
        chunk_paths.push((chunk_path, group.len()));
    }

    progress(
        config.files.len(),
        config.files.len(),
        "Writing merged table",
    );
    write_wide_output(
        &config.output,
        kind,
        &entities,
        &config.files,
        &config.metrics,
        config.column_order,
        &chunk_paths,
    )?;

    Ok(MergeSummary {
        kind,
        samples: config.files.len(),
        rows: n_rows,
        metrics: config.metrics,
        output: config.output,
        missing_rows: missing_rows_total,
        unknown_rows: unknown_rows_total,
    })
}

fn scan_first(
    path: &Path,
    requested_kind: DataKind,
) -> Result<(Schema, Vec<EntityMeta>, HashMap<EntityKey, usize>)> {
    let (headers, range) = read_sheet(path)?;
    let schema = schema_from_headers(&headers, requested_kind)?;
    let row_capacity = range.height().saturating_sub(1);
    let mut entities = Vec::with_capacity(row_capacity);
    let mut index = HashMap::with_capacity(row_capacity);

    for row in range.rows().skip(1) {
        let meta = entity_from_row(&schema, row)?;
        let idx = entities.len();
        if index.insert(meta.key.clone(), idx).is_some() {
            bail!(
                "duplicate entity key in {}: {} / {}",
                path.display(),
                meta.key.primary,
                meta.key.gene_id
            );
        }
        entities.push(meta);
    }

    Ok((schema, entities, index))
}

fn read_sample(
    path: &Path,
    expected_kind: DataKind,
    metrics: &[Metric],
    index: &HashMap<EntityKey, usize>,
    n_rows: usize,
) -> Result<(Vec<StoredCell>, usize, usize)> {
    let (headers, range) = read_sheet(path)?;
    let schema = schema_from_headers(&headers, expected_kind)?;
    validate_required_columns(&schema, metrics)?;

    let n_metrics = metrics.len();
    let mut cells = vec![StoredCell::MISSING; n_rows * n_metrics];
    let mut seen = vec![false; n_rows];
    let mut unknown = 0usize;

    for row in range.rows().skip(1) {
        let meta = entity_from_row(&schema, row)?;
        let Some(&row_idx) = index.get(&meta.key) else {
            unknown += 1;
            continue;
        };
        seen[row_idx] = true;
        for (metric_idx, metric) in metrics.iter().enumerate() {
            let col = *schema
                .columns
                .get(metric.input_header())
                .ok_or_else(|| anyhow!("missing column {}", metric.input_header()))?;
            let value = row.get(col).unwrap_or(&Data::Empty);
            cells[row_idx * n_metrics + metric_idx] = parse_measurement(value, *metric)
                .with_context(|| {
                    format!(
                        "invalid {} value in {} for {}",
                        metric.display_name(),
                        path.display(),
                        meta.key.primary
                    )
                })?;
        }
    }

    let missing = seen.iter().filter(|&&x| !x).count();
    Ok((cells, missing, unknown))
}

fn read_sheet(path: &Path) -> Result<(Vec<String>, Range<Data>)> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("cannot open workbook {}", path.display()))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("workbook {} has no worksheets", path.display()))?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("cannot read worksheet {sheet_name} in {}", path.display()))?;
    let header_row = range
        .rows()
        .next()
        .ok_or_else(|| anyhow!("worksheet {} in {} is empty", sheet_name, path.display()))?;
    let headers: Vec<String> = header_row.iter().map(cell_text).collect();
    Ok((headers, range))
}

fn schema_from_headers(headers: &[String], requested_kind: DataKind) -> Result<Schema> {
    let mut columns = HashMap::new();
    for (i, h) in headers.iter().enumerate() {
        columns.insert(h.trim().to_string(), i);
    }

    let detected = if columns.contains_key("Transcript Name") && columns.contains_key("Total Reads")
    {
        DataKind::Transcript
    } else if columns.contains_key("RNA-Seq Raw Counts") {
        DataKind::Gene
    } else {
        bail!(
            "could not detect report type: expected either 'RNA-Seq Raw Counts' or 'Transcript Name' + 'Total Reads'"
        );
    };

    let kind = match requested_kind {
        DataKind::Auto => detected,
        explicit if explicit == detected => explicit,
        explicit => bail!("requested {explicit:?} input but detected {detected:?} report"),
    };

    for required in ["Gene Symbol", "NCBI Gene ID"] {
        if !columns.contains_key(required) {
            bail!("required column '{required}' is missing");
        }
    }
    if kind == DataKind::Transcript && !columns.contains_key("Transcript Name") {
        bail!("required column 'Transcript Name' is missing");
    }

    Ok(Schema { kind, columns })
}

fn validate_required_columns(schema: &Schema, metrics: &[Metric]) -> Result<()> {
    for metric in metrics {
        if !metric.valid_for(schema.kind) {
            bail!(
                "measurement '{}' is not valid for {:?} data",
                metric.display_name(),
                schema.kind
            );
        }
        if !schema.columns.contains_key(metric.input_header()) {
            bail!("required column '{}' is missing", metric.input_header());
        }
    }
    Ok(())
}

fn entity_from_row(schema: &Schema, row: &[Data]) -> Result<EntityMeta> {
    let symbol_col = *schema.columns.get("Gene Symbol").unwrap();
    let gene_id_col = *schema.columns.get("NCBI Gene ID").unwrap();
    let gene_symbol = row.get(symbol_col).map(cell_text).unwrap_or_default();
    let gene_id = row.get(gene_id_col).map(normalize_id).unwrap_or_default();

    if gene_id.is_empty() {
        bail!("encountered a row with an empty NCBI Gene ID");
    }

    let primary = match schema.kind {
        DataKind::Gene => gene_symbol.clone(),
        DataKind::Transcript => {
            let col = *schema.columns.get("Transcript Name").unwrap();
            row.get(col).map(cell_text).unwrap_or_default()
        }
        DataKind::Auto => unreachable!(),
    };

    if primary.is_empty() {
        bail!("encountered a row with an empty primary identifier");
    }

    Ok(EntityMeta {
        key: EntityKey { primary, gene_id },
    })
}

fn parse_measurement(cell: &Data, metric: Metric) -> Result<StoredCell> {
    if matches!(cell, Data::Empty) {
        return Ok(StoredCell::MISSING);
    }

    if metric == Metric::IsExpressed {
        let text = cell_text(cell).trim().to_ascii_lowercase();
        return match text.as_str() {
            "yes" | "true" | "1" => Ok(StoredCell::yes()),
            "no" | "false" | "0" => Ok(StoredCell::no()),
            "" => Ok(StoredCell::MISSING),
            _ => bail!("expected yes/no, got '{text}'"),
        };
    }

    let value = match cell {
        Data::Int(v) => *v as f64,
        Data::Float(v) => *v,
        Data::String(s) => s.trim().parse::<f64>()?,
        Data::Bool(v) => {
            if *v {
                1.0
            } else {
                0.0
            }
        }
        _ => cell_text(cell).trim().parse::<f64>()?,
    };
    Ok(StoredCell::numeric(value))
}

fn write_wide_output(
    output: &Path,
    kind: DataKind,
    entities: &[EntityMeta],
    files: &[PathBuf],
    metrics: &[Metric],
    order: ColumnOrder,
    chunk_paths: &[(PathBuf, usize)],
) -> Result<()> {
    let file = File::create(output)
        .with_context(|| format!("cannot create output {}", output.display()))?;
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(BufWriter::new(file));

    let sample_names: Vec<String> = files.iter().map(|p| sample_name(p)).collect();
    let mut seen_names = HashSet::with_capacity(sample_names.len());
    for name in &sample_names {
        if !seen_names.insert(name.clone()) {
            bail!("duplicate sample name after filename normalization: '{name}'");
        }
    }

    let mut header = match kind {
        DataKind::Gene => vec!["Gene Symbol".to_string(), "NCBI Gene ID".to_string()],
        DataKind::Transcript => vec!["Transcript Name".to_string(), "NCBI Gene ID".to_string()],
        DataKind::Auto => unreachable!(),
    };

    match order {
        ColumnOrder::AdjacentBySample => {
            for sample in &sample_names {
                for metric in metrics {
                    header.push(format!("{sample} {}", metric.display_name()));
                }
            }
        }
        ColumnOrder::GroupedByMeasurement => {
            for metric in metrics {
                for sample in &sample_names {
                    header.push(format!("{sample} {}", metric.display_name()));
                }
            }
        }
    }
    writer.write_record(&header)?;

    let mut readers: Vec<(BufReader<File>, usize)> = chunk_paths
        .iter()
        .map(|(p, n)| Ok((BufReader::new(File::open(p)?), *n)))
        .collect::<Result<Vec<_>>>()?;

    let n_samples = files.len();
    let n_metrics = metrics.len();
    let mut values = vec![StoredCell::MISSING; n_samples * n_metrics];

    for entity in entities {
        let mut sample_offset = 0usize;
        for (reader, chunk_samples) in &mut readers {
            for local_sample in 0..*chunk_samples {
                for metric_idx in 0..n_metrics {
                    values[(sample_offset + local_sample) * n_metrics + metric_idx] =
                        StoredCell::read_from(&mut *reader)?;
                }
            }
            sample_offset += *chunk_samples;
        }

        let mut record = match kind {
            DataKind::Gene => vec![entity.key.primary.clone(), entity.key.gene_id.clone()],
            DataKind::Transcript => vec![entity.key.primary.clone(), entity.key.gene_id.clone()],
            DataKind::Auto => unreachable!(),
        };

        match order {
            ColumnOrder::AdjacentBySample => {
                for sample_idx in 0..n_samples {
                    for metric_idx in 0..n_metrics {
                        record.push(values[sample_idx * n_metrics + metric_idx].render());
                    }
                }
            }
            ColumnOrder::GroupedByMeasurement => {
                for metric_idx in 0..n_metrics {
                    for sample_idx in 0..n_samples {
                        record.push(values[sample_idx * n_metrics + metric_idx].render());
                    }
                }
            }
        }
        writer.write_record(&record)?;
    }

    writer.flush()?;
    Ok(())
}

/// Read the first workbook into a compact searchable identifier catalogue.
/// This does not retain measurement matrices in memory.
pub fn load_entity_catalog(
    path: &Path,
    requested_kind: DataKind,
) -> Result<Vec<EntityCatalogEntry>> {
    let (headers, range) = read_sheet(path)?;
    let schema = schema_from_headers(&headers, requested_kind)?;
    let symbol_col = *schema
        .columns
        .get("Gene Symbol")
        .ok_or_else(|| anyhow!("required column 'Gene Symbol' is missing"))?;
    let gene_id_col = *schema
        .columns
        .get("NCBI Gene ID")
        .ok_or_else(|| anyhow!("required column 'NCBI Gene ID' is missing"))?;
    let transcript_col = schema.columns.get("Transcript Name").copied();

    let mut out = Vec::with_capacity(range.height().saturating_sub(1));
    for row in range.rows().skip(1) {
        let gene_symbol = row.get(symbol_col).map(cell_text).unwrap_or_default();
        let gene_id = row.get(gene_id_col).map(normalize_id).unwrap_or_default();
        if gene_id.is_empty() {
            continue;
        }
        let primary = match schema.kind {
            DataKind::Gene => gene_symbol.clone(),
            DataKind::Transcript => transcript_col
                .and_then(|col| row.get(col))
                .map(cell_text)
                .unwrap_or_default(),
            DataKind::Auto => unreachable!(),
        };
        if primary.is_empty() {
            continue;
        }
        out.push(EntityCatalogEntry {
            primary,
            gene_id,
            gene_symbol,
        });
    }
    Ok(out)
}

/// Summarize the distribution of one numeric measurement independently for
/// each sample. Workbooks are processed one at a time, so memory use is bounded
/// by one workbook plus one numeric vector.
pub fn summarize_sample_distributions<F>(
    files: &[PathBuf],
    requested_kind: DataKind,
    metric: Metric,
    ignore_zeros: bool,
    mut progress: F,
) -> Result<Vec<SampleDistributionSummary>>
where
    F: FnMut(usize, usize, &str),
{
    if files.is_empty() {
        bail!("no input files were selected");
    }
    if !metric.is_numeric() {
        bail!(
            "'{}' is categorical and cannot be plotted as a numeric distribution",
            metric.display_name()
        );
    }

    let mut summaries = Vec::with_capacity(files.len());
    for (idx, path) in files.iter().enumerate() {
        progress(
            idx + 1,
            files.len(),
            &format!(
                "Summarizing {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
        );

        let (headers, range) = read_sheet(path)?;
        let schema = schema_from_headers(&headers, requested_kind)?;
        validate_required_columns(&schema, &[metric])?;
        let col = *schema
            .columns
            .get(metric.input_header())
            .ok_or_else(|| anyhow!("missing column {}", metric.input_header()))?;

        let mut values = Vec::with_capacity(range.height().saturating_sub(1));
        let mut n_total = 0usize;
        let mut n_missing = 0usize;
        let mut n_zero = 0usize;
        let mut sum = 0.0f64;
        let mut n_for_mean = 0usize;

        for row in range.rows().skip(1) {
            n_total += 1;
            let cell = row.get(col).unwrap_or(&Data::Empty);
            match numeric_measurement(cell, metric)? {
                None => n_missing += 1,
                Some(value) if !value.is_finite() => n_missing += 1,
                Some(value) => {
                    if value == 0.0 {
                        n_zero += 1;
                    }
                    if !ignore_zeros || value != 0.0 {
                        values.push(value);
                        sum += value;
                        n_for_mean += 1;
                    }
                }
            }
        }

        values.sort_by(f64::total_cmp);
        let stats = distribution_stats(&values);
        summaries.push(SampleDistributionSummary {
            sample: sample_name(path),
            n_total,
            n_values: values.len(),
            n_missing,
            n_zero,
            mean: if n_for_mean == 0 {
                None
            } else {
                Some(sum / n_for_mean as f64)
            },
            lower_whisker: stats.map(|x| x.0),
            q1: stats.map(|x| x.1),
            median: stats.map(|x| x.2),
            q3: stats.map(|x| x.3),
            upper_whisker: stats.map(|x| x.4),
        });
    }
    Ok(summaries)
}

/// Extract one selected gene/transcript across all sample workbooks for a
/// numeric measurement. Each workbook is opened and discarded independently.
pub fn load_entity_profile<F>(
    files: &[PathBuf],
    requested_kind: DataKind,
    metric: Metric,
    primary: &str,
    gene_id: &str,
    mut progress: F,
) -> Result<Vec<EntityProfilePoint>>
where
    F: FnMut(usize, usize, &str),
{
    if files.is_empty() {
        bail!("no input files were selected");
    }
    if !metric.is_numeric() {
        bail!(
            "'{}' is categorical and cannot be plotted as a numeric profile",
            metric.display_name()
        );
    }

    let wanted = EntityKey {
        primary: primary.to_string(),
        gene_id: gene_id.to_string(),
    };
    let mut out = Vec::with_capacity(files.len());

    for (idx, path) in files.iter().enumerate() {
        progress(
            idx + 1,
            files.len(),
            &format!(
                "Reading {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
        );

        let (headers, range) = read_sheet(path)?;
        let schema = schema_from_headers(&headers, requested_kind)?;
        validate_required_columns(&schema, &[metric])?;
        let metric_col = *schema
            .columns
            .get(metric.input_header())
            .ok_or_else(|| anyhow!("missing column {}", metric.input_header()))?;

        let mut found = None;
        for row in range.rows().skip(1) {
            let meta = entity_from_row(&schema, row)?;
            if meta.key == wanted {
                found = numeric_measurement(row.get(metric_col).unwrap_or(&Data::Empty), metric)?;
                break;
            }
        }

        out.push(EntityProfilePoint {
            sample: sample_name(path),
            value: found.filter(|v| v.is_finite()),
        });
    }

    Ok(out)
}

/// Inspect a merged TSV produced by this tool without loading its data matrix.
pub fn inspect_merged_tsv(path: &Path) -> Result<MergedDatasetInfo> {
    Ok(read_merged_schema(path)?.info)
}

/// Load the searchable identifier catalogue from an existing merged TSV.
/// This is much faster than reopening hundreds of Excel workbooks.
pub fn load_entity_catalog_from_merged(path: &Path) -> Result<Vec<EntityCatalogEntry>> {
    let schema = read_merged_schema(path)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("cannot open merged TSV {}", path.display()))?;

    let mut out = Vec::new();
    for record in reader.records() {
        let record = record?;
        let primary = record.get(0).unwrap_or("").trim().to_string();
        let gene_id = record.get(1).unwrap_or("").trim().to_string();
        if primary.is_empty() || gene_id.is_empty() {
            continue;
        }
        let gene_symbol = if schema.info.kind == DataKind::Gene {
            primary.clone()
        } else {
            String::new()
        };
        out.push(EntityCatalogEntry {
            primary,
            gene_id,
            gene_symbol,
        });
    }
    Ok(out)
}

/// Summarize one metric from a previously merged TSV. The TSV is processed in
/// sample chunks sized to keep the numeric working set near TARGET_CHUNK_BYTES,
/// so QC remains bounded-memory for large transcript matrices.
pub fn summarize_merged_distributions<F>(
    path: &Path,
    metric: Metric,
    ignore_zeros: bool,
    mut progress: F,
) -> Result<Vec<SampleDistributionSummary>>
where
    F: FnMut(usize, usize, &str),
{
    if !metric.is_numeric() {
        bail!(
            "'{}' is categorical and cannot be plotted as a numeric distribution",
            metric.display_name()
        );
    }

    let schema = read_merged_schema(path)?;
    let columns: Vec<MergedValueColumn> = schema
        .value_columns
        .iter()
        .filter(|column| column.metric == metric)
        .cloned()
        .collect();
    if columns.is_empty() {
        bail!(
            "merged TSV does not contain '{}' columns",
            metric.display_name()
        );
    }

    let file =
        File::open(path).with_context(|| format!("cannot open merged TSV {}", path.display()))?;
    let n_rows = BufReader::new(file).lines().skip(1).count().max(1);
    let bytes_per_sample = n_rows.saturating_mul(std::mem::size_of::<f64>()).max(1);
    let samples_per_chunk = (TARGET_CHUNK_BYTES / bytes_per_sample).clamp(1, 128);

    let mut summaries = Vec::with_capacity(columns.len());
    let total_samples = columns.len();
    for chunk in columns.chunks(samples_per_chunk) {
        let mut values: Vec<Vec<f64>> = chunk.iter().map(|_| Vec::with_capacity(n_rows)).collect();
        let mut n_total = vec![0usize; chunk.len()];
        let mut n_missing = vec![0usize; chunk.len()];
        let mut n_zero = vec![0usize; chunk.len()];
        let mut sums = vec![0.0f64; chunk.len()];
        let mut n_for_mean = vec![0usize; chunk.len()];

        let mut reader = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(true)
            .from_path(path)
            .with_context(|| format!("cannot open merged TSV {}", path.display()))?;

        for record in reader.records() {
            let record = record?;
            for (local_idx, column) in chunk.iter().enumerate() {
                n_total[local_idx] += 1;
                let text = record.get(column.index).unwrap_or("").trim();
                if text.is_empty() {
                    n_missing[local_idx] += 1;
                    continue;
                }
                let value = match text.parse::<f64>() {
                    Ok(value) if value.is_finite() => value,
                    Ok(_) => {
                        n_missing[local_idx] += 1;
                        continue;
                    }
                    Err(_) => {
                        n_missing[local_idx] += 1;
                        continue;
                    }
                };
                if value == 0.0 {
                    n_zero[local_idx] += 1;
                }
                if !ignore_zeros || value != 0.0 {
                    values[local_idx].push(value);
                    sums[local_idx] += value;
                    n_for_mean[local_idx] += 1;
                }
            }
        }

        for (local_idx, column) in chunk.iter().enumerate() {
            values[local_idx].sort_by(f64::total_cmp);
            let stats = distribution_stats(&values[local_idx]);
            summaries.push(SampleDistributionSummary {
                sample: column.sample.clone(),
                n_total: n_total[local_idx],
                n_values: values[local_idx].len(),
                n_missing: n_missing[local_idx],
                n_zero: n_zero[local_idx],
                mean: if n_for_mean[local_idx] == 0 {
                    None
                } else {
                    Some(sums[local_idx] / n_for_mean[local_idx] as f64)
                },
                lower_whisker: stats.map(|x| x.0),
                q1: stats.map(|x| x.1),
                median: stats.map(|x| x.2),
                q3: stats.map(|x| x.3),
                upper_whisker: stats.map(|x| x.4),
            });
            progress(
                summaries.len(),
                total_samples,
                &format!("Summarized {}", column.sample),
            );
        }
    }

    Ok(summaries)
}

/// Extract one gene/transcript profile directly from an existing merged TSV.
/// Only a single TSV pass is required.
pub fn load_entity_profile_from_merged(
    path: &Path,
    metric: Metric,
    primary: &str,
    gene_id: &str,
) -> Result<Vec<EntityProfilePoint>> {
    if !metric.is_numeric() {
        bail!(
            "'{}' is categorical and cannot be plotted as a numeric profile",
            metric.display_name()
        );
    }

    let schema = read_merged_schema(path)?;
    let columns: Vec<MergedValueColumn> = schema
        .value_columns
        .iter()
        .filter(|column| column.metric == metric)
        .cloned()
        .collect();
    if columns.is_empty() {
        bail!(
            "merged TSV does not contain '{}' columns",
            metric.display_name()
        );
    }

    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("cannot open merged TSV {}", path.display()))?;

    for record in reader.records() {
        let record = record?;
        if record.get(0).unwrap_or("").trim() != primary
            || record.get(1).unwrap_or("").trim() != gene_id
        {
            continue;
        }

        let mut out = Vec::with_capacity(columns.len());
        for column in columns {
            let value = record
                .get(column.index)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .and_then(|text| text.parse::<f64>().ok())
                .filter(|value| value.is_finite());
            out.push(EntityProfilePoint {
                sample: column.sample,
                value,
            });
        }
        return Ok(out);
    }

    bail!("identifier '{primary}' / Gene ID '{gene_id}' was not found in merged TSV")
}

fn read_merged_schema(path: &Path) -> Result<MergedSchema> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("cannot open merged TSV {}", path.display()))?;
    let headers = reader.headers()?.clone();
    if headers.len() < 3 {
        bail!("merged TSV must contain at least two identifier columns and one value column");
    }

    let first = headers.get(0).unwrap_or("").trim();
    let second = headers.get(1).unwrap_or("").trim();
    if second != "NCBI Gene ID" {
        bail!("expected second merged TSV column to be 'NCBI Gene ID', found '{second}'");
    }
    let kind = match first {
        "Gene Symbol" => DataKind::Gene,
        "Transcript Name" => DataKind::Transcript,
        other => bail!(
            "could not detect merged TSV type from first column '{other}'; expected 'Gene Symbol' or 'Transcript Name'"
        ),
    };

    let mut value_columns = Vec::new();
    let mut samples = Vec::new();
    let mut seen_samples = HashSet::new();
    let mut seen_sample_metrics = HashSet::new();
    let mut seen_metrics = HashSet::new();

    for (index, header) in headers.iter().enumerate().skip(2) {
        let header = header.trim();
        let Some((sample, metric)) = parse_merged_value_header(kind, header) else {
            continue;
        };
        if !seen_sample_metrics.insert((sample.clone(), metric)) {
            bail!(
                "duplicate merged TSV column for sample '{sample}' and '{}'",
                metric.display_name()
            );
        }
        if seen_samples.insert(sample.clone()) {
            samples.push(sample.clone());
        }
        seen_metrics.insert(metric);
        value_columns.push(MergedValueColumn {
            index,
            sample,
            metric,
        });
    }

    if value_columns.is_empty() {
        bail!("no recognized expression measurement columns were found in merged TSV");
    }

    let metrics = all_metrics(kind)
        .into_iter()
        .filter(|metric| seen_metrics.contains(metric))
        .collect();
    Ok(MergedSchema {
        info: MergedDatasetInfo {
            kind,
            samples,
            metrics,
        },
        value_columns,
    })
}

fn parse_merged_value_header(kind: DataKind, header: &str) -> Option<(String, Metric)> {
    for metric in all_metrics(kind) {
        let suffix = format!(" {}", metric.display_name());
        if let Some(sample) = header.strip_suffix(&suffix) {
            let sample = sample.trim();
            if !sample.is_empty() {
                return Some((sample.to_string(), metric));
            }
        }
    }
    None
}

fn numeric_measurement(cell: &Data, metric: Metric) -> Result<Option<f64>> {
    let stored = parse_measurement(cell, metric)?;
    match stored.tag {
        0 => Ok(None),
        1 => Ok(Some(stored.value)),
        2 => Ok(Some(1.0)),
        3 => Ok(Some(0.0)),
        _ => Ok(None),
    }
}

fn distribution_stats(values: &[f64]) -> Option<(f64, f64, f64, f64, f64)> {
    if values.is_empty() {
        return None;
    }
    let q1 = quantile_sorted(values, 0.25);
    let median = quantile_sorted(values, 0.5);
    let q3 = quantile_sorted(values, 0.75);
    let iqr = q3 - q1;
    let lower_fence = q1 - 1.5 * iqr;
    let upper_fence = q3 + 1.5 * iqr;
    let lower_whisker = values
        .iter()
        .copied()
        .find(|v| *v >= lower_fence)
        .unwrap_or(values[0]);
    let upper_whisker = values
        .iter()
        .rev()
        .copied()
        .find(|v| *v <= upper_fence)
        .unwrap_or(*values.last().unwrap());
    Some((lower_whisker, q1, median, q3, upper_whisker))
}

fn quantile_sorted(values: &[f64], probability: f64) -> f64 {
    debug_assert!(!values.is_empty());
    if values.len() == 1 {
        return values[0];
    }
    let position = (values.len() - 1) as f64 * probability.clamp(0.0, 1.0);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        values[lower]
    } else {
        let weight = position - lower as f64;
        values[lower] * (1.0 - weight) + values[upper] * weight
    }
}

pub fn sample_name(path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("sample");
    portable_io::sample_name_from_filename(file_name)
}

fn normalize_id(cell: &Data) -> String {
    match cell {
        Data::Int(v) => v.to_string(),
        Data::Float(v) if v.fract() == 0.0 => format!("{:.0}", v),
        _ => cell_text(cell),
    }
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(v) => v.to_string(),
        Data::Float(v) => format_number(*v),
        Data::Bool(v) => v.to_string(),
        other => other.to_string(),
    }
}

fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_992.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}
