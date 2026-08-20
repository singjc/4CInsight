use anyhow::{anyhow, bail, Context, Result};
use calamine::{open_workbook, Data, Range, Reader, Xlsx};
use csv::WriterBuilder;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
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
    let samples_per_chunk = (TARGET_CHUNK_BYTES / bytes_per_sample)
        .clamp(1, MAX_SAMPLES_PER_CHUNK);

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
                &format!("Reading {}", path.file_name().unwrap_or_default().to_string_lossy()),
            );
            let (cells, missing, unknown) = read_sample(
                path,
                kind,
                &config.metrics,
                &index,
                n_rows,
            )?;
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

    progress(config.files.len(), config.files.len(), "Writing merged table");
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
    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("cannot open workbook {}", path.display()))?;
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

    let detected = if columns.contains_key("Transcript Name") && columns.contains_key("Total Reads") {
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
            if *v { 1.0 } else { 0.0 }
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
        DataKind::Transcript => vec![
            "Transcript Name".to_string(),
            "NCBI Gene ID".to_string(),
        ],
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
            DataKind::Transcript => vec![
                entity.key.primary.clone(),
                entity.key.gene_id.clone(),
            ],
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

fn sample_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("sample")
        .to_string();

    const SUFFIXES: &[&str] = &[
        "_tumor_rna_gene_expression_report",
        "_tumor_rna_transcript_expression_report",
        "_rna_gene_expression_report",
        "_rna_transcript_expression_report",
        "_gene_expression_report",
        "_transcript_expression_report",
    ];

    for suffix in SUFFIXES {
        if let Some(trimmed) = stem.strip_suffix(suffix) {
            return trimmed.to_string();
        }
    }
    stem
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
