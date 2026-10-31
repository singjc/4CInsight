//! Browser-neutral implementation of the original 4C wide-table merger.
//! The first workbook determines row order and the canonical feature universe.
//! A caller supplies one XLSX byte buffer at a time and owns its scratch storage.
use anyhow::{Context, Result, anyhow, bail};
use calamine::{Data, Range, Reader, Xlsx};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

const CELL_BYTES: usize = 9;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PortableMergeKind {
    Gene,
    Transcript,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PortableMergeOrder {
    AdjacentBySample,
    GroupedByMeasurement,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PortableMergeMetric {
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

impl PortableMergeMetric {
    fn header(self) -> &'static str {
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
    fn display(self) -> &'static str {
        match self {
            Self::RawCount => "Raw Count",
            other => other.header(),
        }
    }
    fn valid(self, kind: PortableMergeKind) -> bool {
        match kind {
            PortableMergeKind::Gene => matches!(
                self,
                Self::RawCount
                    | Self::Fpkm
                    | Self::Cpm
                    | Self::Tpm
                    | Self::PercentileRank
                    | Self::IsExpressed
            ),
            PortableMergeKind::Transcript => matches!(
                self,
                Self::TranscriptLength
                    | Self::EffectiveTranscriptLength
                    | Self::TotalReads
                    | Self::Tpm
                    | Self::PercentileRank
                    | Self::IsExpressed
            ),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePlan {
    pub kind: PortableMergeKind,
    pub rows: usize,
    pub samples: usize,
    pub measurements: usize,
    pub bytes_per_sample: usize,
    pub first_workbook_policy: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeSampleReport {
    pub missing_rows: usize,
    pub unknown_rows: usize,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct Key {
    primary: String,
    gene_id: String,
}

pub struct PortableMerger {
    kind: PortableMergeKind,
    order: PortableMergeOrder,
    metrics: Vec<PortableMergeMetric>,
    samples: Vec<String>,
    entities: Vec<Key>,
    index: HashMap<Key, usize>,
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(v) => v.clone(),
        Data::Int(v) => v.to_string(),
        Data::Float(v) => format_number(*v),
        Data::Bool(v) => v.to_string(),
        v => v.to_string(),
    }
}
fn normalized_id(cell: &Data) -> String {
    match cell {
        Data::Int(v) => v.to_string(),
        Data::Float(v) if v.fract() == 0.0 => format!("{v:.0}"),
        v => cell_text(v),
    }
}
fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_992.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn read_sheet(bytes: &[u8]) -> Result<(HashMap<String, usize>, Range<Data>)> {
    let mut workbook: Xlsx<_> =
        Xlsx::new(Cursor::new(bytes)).context("cannot open XLSX workbook")?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("workbook has no worksheets"))?;
    let range = workbook
        .worksheet_range(&sheet)
        .with_context(|| format!("cannot read worksheet {sheet}"))?;
    let headers = range
        .rows()
        .next()
        .ok_or_else(|| anyhow!("empty worksheet"))?;
    let columns = headers
        .iter()
        .enumerate()
        .map(|(index, cell)| (cell_text(cell).trim().to_string(), index))
        .collect();
    Ok((columns, range))
}
fn detect_kind(columns: &HashMap<String, usize>) -> Result<PortableMergeKind> {
    if columns.contains_key("Transcript Name") && columns.contains_key("Total Reads") {
        Ok(PortableMergeKind::Transcript)
    } else if columns.contains_key("RNA-Seq Raw Counts") {
        Ok(PortableMergeKind::Gene)
    } else {
        bail!(
            "could not detect report type: expected 'RNA-Seq Raw Counts' or 'Transcript Name' + 'Total Reads'"
        )
    }
}
fn verify(
    columns: &HashMap<String, usize>,
    kind: PortableMergeKind,
    metrics: &[PortableMergeMetric],
) -> Result<()> {
    if detect_kind(columns)? != kind {
        bail!("workbook type does not match selected merge modality")
    }
    for field in ["Gene Symbol", "NCBI Gene ID"] {
        if !columns.contains_key(field) {
            bail!("required column '{field}' is missing")
        }
    }
    if kind == PortableMergeKind::Transcript && !columns.contains_key("Transcript Name") {
        bail!("required column 'Transcript Name' is missing")
    }
    for metric in metrics {
        if !metric.valid(kind) {
            bail!("invalid metric for selected modality: {}", metric.display())
        }
        if !columns.contains_key(metric.header()) {
            bail!("required column '{}' is missing", metric.header())
        }
    }
    Ok(())
}
fn key_from_row(
    columns: &HashMap<String, usize>,
    kind: PortableMergeKind,
    row: &[Data],
) -> Result<Key> {
    let symbol = row
        .get(columns["Gene Symbol"])
        .map(cell_text)
        .unwrap_or_default();
    let gene_id = row
        .get(columns["NCBI Gene ID"])
        .map(normalized_id)
        .unwrap_or_default();
    let primary = if kind == PortableMergeKind::Gene {
        symbol
    } else {
        row.get(columns["Transcript Name"])
            .map(cell_text)
            .unwrap_or_default()
    };
    if primary.is_empty() {
        bail!("encountered a row with an empty primary identifier")
    }
    if gene_id.is_empty() {
        bail!("encountered a row with an empty NCBI Gene ID")
    }
    Ok(Key { primary, gene_id })
}
fn cell_bytes(cell: &Data, metric: PortableMergeMetric) -> Result<[u8; CELL_BYTES]> {
    let (tag, number) = if matches!(cell, Data::Empty) {
        (0, 0.0)
    } else if metric == PortableMergeMetric::IsExpressed {
        let text = cell_text(cell).trim().to_ascii_lowercase();
        match text.as_str() {
            "yes" | "true" | "1" => (2, 0.0),
            "no" | "false" | "0" => (3, 0.0),
            "" => (0, 0.0),
            _ => bail!("expected yes/no, got '{text}'"),
        }
    } else {
        let value = match cell {
            Data::Int(v) => *v as f64,
            Data::Float(v) => *v,
            Data::String(v) => v.trim().parse::<f64>()?,
            Data::Bool(v) => {
                if *v {
                    1.0
                } else {
                    0.0
                }
            }
            other => cell_text(other).trim().parse::<f64>()?,
        };
        (1, value)
    };
    let mut bytes = [0u8; CELL_BYTES];
    bytes[0] = tag;
    bytes[1..].copy_from_slice(&number.to_le_bytes());
    Ok(bytes)
}
fn decode_cell(bytes: &[u8]) -> Result<String> {
    Ok(match bytes[0] {
        0 => String::new(),
        1 => format_number(f64::from_le_bytes(bytes[1..9].try_into()?)),
        2 => "yes".to_string(),
        3 => "no".to_string(),
        _ => bail!("invalid stored measurement tag"),
    })
}

impl PortableMerger {
    pub fn begin(
        first: &[u8],
        kind: PortableMergeKind,
        metrics: Vec<PortableMergeMetric>,
        order: PortableMergeOrder,
        file_names: &[String],
    ) -> Result<Self> {
        if file_names.is_empty() {
            bail!("select at least one sample workbook")
        }
        if metrics.is_empty() {
            bail!("select at least one measurement")
        }
        let mut unique = HashSet::new();
        for metric in &metrics {
            if !unique.insert(*metric) {
                bail!(
                    "duplicate metric '{}': select each measurement once",
                    metric.display()
                )
            }
        }
        let samples = file_names
            .iter()
            .map(|name| crate::portable_io::sample_name_from_filename(name))
            .collect::<Vec<_>>();
        let mut unique_names = HashSet::new();
        for sample in &samples {
            if !unique_names.insert(sample) {
                bail!("duplicate sample name after filename normalization: '{sample}'")
            }
        }
        let (columns, range) = read_sheet(first)?;
        verify(&columns, kind, &metrics)?;
        let mut entities = Vec::with_capacity(range.height().saturating_sub(1));
        let mut index = HashMap::with_capacity(range.height().saturating_sub(1));
        for row in range.rows().skip(1) {
            let key = key_from_row(&columns, kind, row)?;
            if index.insert(key.clone(), entities.len()).is_some() {
                bail!(
                    "duplicate entity key in first workbook: {} / {}",
                    key.primary,
                    key.gene_id
                )
            }
            entities.push(key);
        }
        Ok(Self {
            kind,
            order,
            metrics,
            samples,
            entities,
            index,
        })
    }
    pub fn plan(&self) -> MergePlan {
        MergePlan {
            kind: self.kind,
            rows: self.entities.len(),
            samples: self.samples.len(),
            measurements: self.metrics.len(),
            bytes_per_sample: self.entities.len() * self.metrics.len() * CELL_BYTES,
            first_workbook_policy: "first-sorted-workbook-canonical",
        }
    }
    pub fn header(&self) -> String {
        let mut row = vec![
            if self.kind == PortableMergeKind::Gene {
                "Gene Symbol"
            } else {
                "Transcript Name"
            }
            .to_string(),
            "NCBI Gene ID".to_string(),
        ];
        match self.order {
            PortableMergeOrder::AdjacentBySample => {
                for sample in &self.samples {
                    for metric in &self.metrics {
                        row.push(format!("{sample} {}", metric.display()));
                    }
                }
            }
            PortableMergeOrder::GroupedByMeasurement => {
                for metric in &self.metrics {
                    for sample in &self.samples {
                        row.push(format!("{sample} {}", metric.display()));
                    }
                }
            }
        }
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(vec![]);
        writer.write_record(&row).expect("in-memory TSV header");
        String::from_utf8(writer.into_inner().expect("in-memory TSV header")).expect("UTF-8 header")
    }
    pub fn sample(&self, bytes: &[u8]) -> Result<(Vec<u8>, MergeSampleReport)> {
        let (columns, range) = read_sheet(bytes)?;
        verify(&columns, self.kind, &self.metrics)?;
        let width = self.metrics.len() * CELL_BYTES;
        let mut values = vec![0u8; self.entities.len() * width];
        let mut seen = vec![false; self.entities.len()];
        let mut unknown = 0usize;
        for row in range.rows().skip(1) {
            let key = key_from_row(&columns, self.kind, row)?;
            let Some(&at) = self.index.get(&key) else {
                unknown += 1;
                continue;
            };
            seen[at] = true;
            for (metric_index, metric) in self.metrics.iter().enumerate() {
                let source = row.get(columns[metric.header()]).unwrap_or(&Data::Empty);
                let encoded = cell_bytes(source, *metric).with_context(|| {
                    format!("invalid {} value for {}", metric.display(), key.primary)
                })?;
                let offset = at * width + metric_index * CELL_BYTES;
                values[offset..offset + CELL_BYTES].copy_from_slice(&encoded);
            }
        }
        Ok((
            values,
            MergeSampleReport {
                missing_rows: seen.iter().filter(|&&seen| !seen).count(),
                unknown_rows: unknown,
            },
        ))
    }
    /// `buffer` contains one contiguous row range per sample, in sample order.
    /// This allows the browser to read only small slices of temporary files.
    pub fn rows_tsv(&self, start: usize, count: usize, buffer: &[u8]) -> Result<String> {
        if count == 0 || start > self.entities.len() || count > self.entities.len() - start {
            bail!("invalid row range")
        }
        let stride = self.metrics.len() * CELL_BYTES;
        let expected = self
            .samples
            .len()
            .checked_mul(count)
            .and_then(|x| x.checked_mul(stride))
            .ok_or_else(|| anyhow!("row batch is too large"))?;
        if buffer.len() != expected {
            bail!(
                "row batch has {} bytes, expected {}",
                buffer.len(),
                expected
            )
        }
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(vec![]);
        for i in 0..count {
            let entity = &self.entities[start + i];
            let mut row = vec![entity.primary.clone(), entity.gene_id.clone()];
            let mut append = |sample: usize, metric: usize| -> Result<()> {
                let from = sample * count * stride + i * stride + metric * CELL_BYTES;
                row.push(decode_cell(&buffer[from..from + CELL_BYTES])?);
                Ok(())
            };
            match self.order {
                PortableMergeOrder::AdjacentBySample => {
                    for sample in 0..self.samples.len() {
                        for metric in 0..self.metrics.len() {
                            append(sample, metric)?;
                        }
                    }
                }
                PortableMergeOrder::GroupedByMeasurement => {
                    for metric in 0..self.metrics.len() {
                        for sample in 0..self.samples.len() {
                            append(sample, metric)?;
                        }
                    }
                }
            }
            writer.write_record(&row)?;
        }
        let data = writer.into_inner()?;
        Ok(String::from_utf8(data)?)
    }
}
