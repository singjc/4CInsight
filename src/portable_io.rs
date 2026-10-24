//! Browser-safe metadata and expression readers shared by native parity tests and WASM.
//!
//! This module deliberately accepts byte slices rather than filesystem paths.  It contains no
//! browser APIs and no native filesystem calls, so the same Rust implementation can be compiled
//! for `wasm32-unknown-unknown` and exercised by native tests.

use anyhow::{Context, Result, anyhow, bail};
use calamine::{Data, Reader, Xlsx, open_workbook_auto_from_rs};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableMetadataTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub id_column: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataColumnSummary {
    pub name: String,
    pub non_empty: usize,
    pub unique: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataInspection {
    pub file_name: String,
    pub rows: usize,
    pub columns: Vec<String>,
    pub id_column: String,
    pub blank_id_rows: usize,
    pub duplicate_id_values: Vec<String>,
    pub column_summaries: Vec<MetadataColumnSummary>,
    pub preview_rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortableExpressionKind {
    Gene,
    Transcript,
}

impl PortableExpressionKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Gene => "gene-expression",
            Self::Transcript => "transcript-expression",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Gene => "Gene expression",
            Self::Transcript => "Transcript expression",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gene" | "gene-expression" | "gene_expression" => Ok(Self::Gene),
            "transcript" | "transcript-expression" | "transcript_expression" => {
                Ok(Self::Transcript)
            }
            other => bail!("unsupported expression kind '{other}'"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PortableExpressionMetric {
    RawCount,
    Fpkm,
    Cpm,
    Tpm,
    PercentileRank,
    TranscriptLength,
    EffectiveTranscriptLength,
    TotalReads,
}

impl PortableExpressionMetric {
    pub fn slug(self) -> &'static str {
        match self {
            Self::RawCount => "raw-count",
            Self::Fpkm => "fpkm",
            Self::Cpm => "cpm",
            Self::Tpm => "tpm",
            Self::PercentileRank => "percentile-rank",
            Self::TranscriptLength => "transcript-length",
            Self::EffectiveTranscriptLength => "effective-transcript-length",
            Self::TotalReads => "total-reads",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::RawCount => "Raw Count",
            Self::Fpkm => "FPKM",
            Self::Cpm => "CPM",
            Self::Tpm => "TPM",
            Self::PercentileRank => "Percentile Rank",
            Self::TranscriptLength => "Transcript Length",
            Self::EffectiveTranscriptLength => "Effective Transcript Length",
            Self::TotalReads => "Total Reads",
        }
    }

    pub fn input_header(self) -> &'static str {
        match self {
            Self::RawCount => "RNA-Seq Raw Counts",
            Self::Fpkm => "FPKM",
            Self::Cpm => "CPM",
            Self::Tpm => "TPM",
            Self::PercentileRank => "Percentile Rank",
            Self::TranscriptLength => "Transcript Length",
            Self::EffectiveTranscriptLength => "Effective Transcript Length",
            Self::TotalReads => "Total Reads",
        }
    }

    pub fn valid_for(self, kind: PortableExpressionKind) -> bool {
        match kind {
            PortableExpressionKind::Gene => matches!(
                self,
                Self::RawCount | Self::Fpkm | Self::Cpm | Self::Tpm | Self::PercentileRank
            ),
            PortableExpressionKind::Transcript => matches!(
                self,
                Self::TranscriptLength
                    | Self::EffectiveTranscriptLength
                    | Self::TotalReads
                    | Self::Tpm
                    | Self::PercentileRank
            ),
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "raw-count" | "raw_count" | "raw count" | "rna-seq raw counts" => Ok(Self::RawCount),
            "fpkm" => Ok(Self::Fpkm),
            "cpm" => Ok(Self::Cpm),
            "tpm" => Ok(Self::Tpm),
            "percentile-rank" | "percentile_rank" | "percentile rank" => Ok(Self::PercentileRank),
            "transcript-length" | "transcript_length" | "transcript length" => {
                Ok(Self::TranscriptLength)
            }
            "effective-transcript-length"
            | "effective_transcript_length"
            | "effective transcript length" => Ok(Self::EffectiveTranscriptLength),
            "total-reads" | "total_reads" | "total reads" => Ok(Self::TotalReads),
            other => bail!("unsupported expression metric '{other}'"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpressionDistributionSummary {
    pub file_name: String,
    pub sample: String,
    pub kind: PortableExpressionKind,
    pub metric: PortableExpressionMetric,
    pub rows: usize,
    pub available_metrics: Vec<PortableExpressionMetric>,
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

#[derive(Debug, Clone)]
struct ExpressionSchema {
    kind: PortableExpressionKind,
    columns: HashMap<String, usize>,
}

pub fn parse_metadata_bytes(file_name: &str, bytes: &[u8]) -> Result<PortableMetadataTable> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let (columns, rows) = match extension.as_str() {
        "xlsx" | "xlsm" | "xls" => read_metadata_workbook_bytes(bytes)?,
        "csv" => read_metadata_delimited_bytes(bytes, b',')?,
        "tsv" | "txt" => read_metadata_delimited_bytes(bytes, b'\t')?,
        _ => bail!(
            "unsupported metadata file type for '{file_name}'; expected .xlsx, .xls, .tsv, .txt, or .csv"
        ),
    };

    if columns.is_empty() {
        bail!("metadata file has no header row");
    }

    let id_column = detect_default_id_column(&columns);
    Ok(PortableMetadataTable {
        columns,
        rows,
        id_column,
    })
}

pub fn inspect_metadata_bytes(
    file_name: &str,
    bytes: &[u8],
    selected_id_column: Option<&str>,
) -> Result<MetadataInspection> {
    let mut table = parse_metadata_bytes(file_name, bytes)?;
    if let Some(column) = selected_id_column
        .map(str::trim)
        .filter(|column| !column.is_empty())
    {
        if !table.columns.iter().any(|existing| existing == column) {
            bail!("metadata ID column '{column}' does not exist");
        }
        table.id_column = column.to_string();
    }

    let id_index = table
        .columns
        .iter()
        .position(|column| column == &table.id_column)
        .ok_or_else(|| anyhow!("metadata ID column '{}' is missing", table.id_column))?;

    let mut blank_id_rows = 0usize;
    let mut id_counts = BTreeMap::<String, usize>::new();
    for row in &table.rows {
        let id = row
            .get(id_index)
            .map(String::as_str)
            .unwrap_or_default()
            .trim();
        if id.is_empty() {
            blank_id_rows += 1;
        } else {
            *id_counts.entry(id.to_string()).or_default() += 1;
        }
    }
    let duplicate_id_values = id_counts
        .into_iter()
        .filter_map(|(value, count)| (count > 1).then_some(value))
        .collect::<Vec<_>>();

    let mut column_summaries = Vec::with_capacity(table.columns.len());
    for (column_index, name) in table.columns.iter().enumerate() {
        let mut non_empty = 0usize;
        let mut unique = BTreeSet::<String>::new();
        for row in &table.rows {
            let value = row
                .get(column_index)
                .map(String::as_str)
                .unwrap_or_default()
                .trim();
            if !value.is_empty() {
                non_empty += 1;
                unique.insert(value.to_string());
            }
        }
        column_summaries.push(MetadataColumnSummary {
            name: name.clone(),
            non_empty,
            unique: unique.len(),
        });
    }

    Ok(MetadataInspection {
        file_name: file_name.to_string(),
        rows: table.rows.len(),
        columns: table.columns.clone(),
        id_column: table.id_column,
        blank_id_rows,
        duplicate_id_values,
        column_summaries,
        preview_rows: table.rows.into_iter().take(8).collect(),
    })
}

pub fn summarize_expression_bytes(
    file_name: &str,
    bytes: &[u8],
    requested_kind: PortableExpressionKind,
    metric: PortableExpressionMetric,
    ignore_zeros: bool,
) -> Result<ExpressionDistributionSummary> {
    let mut workbook: Xlsx<_> = Xlsx::new(Cursor::new(bytes))
        .with_context(|| format!("cannot open expression workbook {file_name}"))?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("workbook {file_name} has no worksheets"))?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("cannot read worksheet '{sheet_name}' in {file_name}"))?;
    let header_row = range
        .rows()
        .next()
        .ok_or_else(|| anyhow!("worksheet '{sheet_name}' in {file_name} is empty"))?;
    let headers = header_row.iter().map(cell_text).collect::<Vec<_>>();
    let schema = expression_schema_from_headers(&headers, Some(requested_kind))?;

    if !metric.valid_for(schema.kind) {
        bail!(
            "measurement '{}' is not valid for {} data",
            metric.display_name(),
            schema.kind.display_name()
        );
    }
    let metric_column = *schema
        .columns
        .get(metric.input_header())
        .ok_or_else(|| anyhow!("required column '{}' is missing", metric.input_header()))?;

    let available_metrics = expression_metrics(schema.kind)
        .into_iter()
        .filter(|candidate| schema.columns.contains_key(candidate.input_header()))
        .collect::<Vec<_>>();

    validate_identifier_columns(&schema)?;

    let mut values = Vec::<f64>::with_capacity(range.height().saturating_sub(1));
    let mut n_total = 0usize;
    let mut n_missing = 0usize;
    let mut n_zero = 0usize;
    let mut sum = 0.0f64;
    let mut n_for_mean = 0usize;
    for row in range.rows().skip(1) {
        n_total += 1;
        let cell = row.get(metric_column).unwrap_or(&Data::Empty);
        match numeric_measurement(cell)? {
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

    Ok(ExpressionDistributionSummary {
        file_name: file_name.to_string(),
        sample: sample_name_from_filename(file_name),
        kind: schema.kind,
        metric,
        rows: n_total,
        available_metrics,
        n_total,
        n_values: values.len(),
        n_missing,
        n_zero,
        mean: (n_for_mean > 0).then_some(sum / n_for_mean as f64),
        lower_whisker: stats.map(|value| value.0),
        q1: stats.map(|value| value.1),
        median: stats.map(|value| value.2),
        q3: stats.map(|value| value.3),
        upper_whisker: stats.map(|value| value.4),
    })
}

pub fn sample_name_from_filename(file_name: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
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

fn read_metadata_workbook_bytes(bytes: &[u8]) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
        .context("could not open metadata workbook")?;
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("metadata workbook has no worksheets"))?;
    let range = workbook
        .worksheet_range(&sheet_name)
        .with_context(|| format!("could not read metadata worksheet '{sheet_name}'"))?;
    let mut rows = range.rows();
    let header = rows
        .next()
        .ok_or_else(|| anyhow!("metadata worksheet is empty"))?;
    let columns = header.iter().map(cell_text).collect::<Vec<_>>();
    let rows = rows
        .map(|row| {
            (0..columns.len())
                .map(|index| row.get(index).map(cell_text).unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|value| !value.trim().is_empty()))
        .collect::<Vec<_>>();
    Ok((columns, rows))
}

fn read_metadata_delimited_bytes(
    bytes: &[u8],
    delimiter: u8,
) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut reader = ReaderBuilder::new().delimiter(delimiter).from_reader(bytes);
    let columns = reader
        .headers()?
        .iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let rows = reader
        .records()
        .map(|record| record.map(|record| record.iter().map(str::to_string).collect::<Vec<_>>()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((columns, rows))
}

fn detect_default_id_column(columns: &[String]) -> String {
    ["sample_id", "4C_ID", "Sample_ID", "Sample", "sample"]
        .iter()
        .find(|candidate| columns.iter().any(|column| column == **candidate))
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| columns.first().cloned().unwrap_or_default())
}

fn expression_schema_from_headers(
    headers: &[String],
    requested_kind: Option<PortableExpressionKind>,
) -> Result<ExpressionSchema> {
    let mut columns = HashMap::new();
    for (index, header) in headers.iter().enumerate() {
        columns.insert(header.trim().to_string(), index);
    }

    let detected = if columns.contains_key("Transcript Name") && columns.contains_key("Total Reads")
    {
        PortableExpressionKind::Transcript
    } else if columns.contains_key("RNA-Seq Raw Counts") {
        PortableExpressionKind::Gene
    } else {
        bail!(
            "could not detect expression report type: expected either 'RNA-Seq Raw Counts' or 'Transcript Name' + 'Total Reads'"
        );
    };

    if let Some(requested) = requested_kind {
        if requested != detected {
            bail!(
                "requested {} input but detected {} report",
                requested.display_name(),
                detected.display_name()
            );
        }
    }

    Ok(ExpressionSchema {
        kind: detected,
        columns,
    })
}

fn validate_identifier_columns(schema: &ExpressionSchema) -> Result<()> {
    for required in ["Gene Symbol", "NCBI Gene ID"] {
        if !schema.columns.contains_key(required) {
            bail!("required column '{required}' is missing");
        }
    }
    if schema.kind == PortableExpressionKind::Transcript
        && !schema.columns.contains_key("Transcript Name")
    {
        bail!("required column 'Transcript Name' is missing");
    }
    Ok(())
}

fn expression_metrics(kind: PortableExpressionKind) -> Vec<PortableExpressionMetric> {
    match kind {
        PortableExpressionKind::Gene => vec![
            PortableExpressionMetric::RawCount,
            PortableExpressionMetric::Fpkm,
            PortableExpressionMetric::Cpm,
            PortableExpressionMetric::Tpm,
            PortableExpressionMetric::PercentileRank,
        ],
        PortableExpressionKind::Transcript => vec![
            PortableExpressionMetric::TranscriptLength,
            PortableExpressionMetric::EffectiveTranscriptLength,
            PortableExpressionMetric::TotalReads,
            PortableExpressionMetric::Tpm,
            PortableExpressionMetric::PercentileRank,
        ],
    }
}

fn numeric_measurement(cell: &Data) -> Result<Option<f64>> {
    if matches!(cell, Data::Empty) {
        return Ok(None);
    }
    let value = match cell {
        Data::Int(value) => *value as f64,
        Data::Float(value) => *value,
        Data::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                return Ok(None);
            }
            value.parse::<f64>()?
        }
        Data::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        other => {
            let text = cell_text(other);
            let text = text.trim();
            if text.is_empty() {
                return Ok(None);
            }
            text.parse::<f64>()?
        }
    };
    Ok(Some(value))
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
        .find(|value| *value >= lower_fence)
        .unwrap_or(values[0]);
    let upper_whisker = values
        .iter()
        .rev()
        .copied()
        .find(|value| *value <= upper_fence)
        .unwrap_or(*values.last().unwrap());
    Some((lower_whisker, q1, median, q3, upper_whisker))
}

fn quantile_sorted(values: &[f64], probability: f64) -> f64 {
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

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => format_number(*value),
        Data::Bool(value) => value.to_string(),
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
