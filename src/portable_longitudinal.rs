//! Browser-safe longitudinal patient-signature analysis for expression data.
//!
//! Metadata and merged expression tables are read from byte slices. The
//! distance construction and patient-identity summary reuse the same Rust
//! implementations as the native longitudinal workflow.

use crate::cohort::MetadataTable;
use crate::corrected::{CorrectedModelSpec, run_unsupervised_expression_distance_bytes};
use crate::longitudinal::{
    LongitudinalClusteringConfig, LongitudinalExcludedRow, LongitudinalModalityResult,
    build_metadata_index, summarize_modality,
};
use crate::portable_io::{PortableExpressionKind, parse_metadata_bytes};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableLongitudinalConfig {
    pub kind: PortableExpressionKind,
    pub observation_id_column: String,
    pub patient_id_column: String,
    #[serde(default)]
    pub time_column: Option<String>,
    #[serde(default)]
    pub exclusion_enabled: bool,
    #[serde(default)]
    pub exclusion_column: Option<String>,
    #[serde(default)]
    pub exclusion_values: Vec<String>,
    #[serde(default = "default_min_count")]
    pub min_count: f64,
    #[serde(default = "default_min_fraction_samples")]
    pub min_fraction_samples: f64,
}

fn default_min_count() -> f64 {
    10.0
}

fn default_min_fraction_samples() -> f64 {
    0.20
}

#[derive(Debug, Clone, Serialize)]
pub struct PortableLongitudinalResult {
    pub kind: PortableExpressionKind,
    pub metadata_file: String,
    pub merged_expression_file: String,
    pub metadata_rows: usize,
    pub eligible_metadata_rows: usize,
    pub qc_excluded_rows: usize,
    pub missing_identifier_rows: usize,
    pub ambiguous_observation_keys: usize,
    pub excluded_rows: Vec<LongitudinalExcludedRow>,
    pub modality: LongitudinalModalityResult,
    pub warnings: Vec<String>,
}

pub fn run_portable_expression_longitudinal<F>(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config: &PortableLongitudinalConfig,
    progress: F,
) -> Result<PortableLongitudinalResult>
where
    F: FnMut(usize, usize, &str),
{
    let observation_column = config.observation_id_column.trim();
    let patient_column = config.patient_id_column.trim();
    if observation_column.is_empty() {
        bail!("Select the metadata column containing longitudinal observation/sample IDs");
    }
    if patient_column.is_empty() {
        bail!("Select the metadata column containing stable patient/person IDs");
    }
    if observation_column == patient_column {
        bail!("Observation/sample ID and patient/person ID must use different metadata columns");
    }

    let portable = parse_metadata_bytes(metadata_file_name, metadata_bytes)?;
    for column in [observation_column, patient_column] {
        if !portable.columns.iter().any(|existing| existing == column) {
            bail!("metadata column '{column}' does not exist");
        }
    }
    let time_column = config
        .time_column
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(column) = time_column.as_ref()
        && !portable.columns.iter().any(|existing| existing == column)
    {
        bail!("metadata time/visit column '{column}' does not exist");
    }

    let exclusion_column = if config.exclusion_enabled {
        config
            .exclusion_column
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    } else {
        None
    };
    if config.exclusion_enabled && exclusion_column.is_none() {
        bail!("Select a metadata column for the enabled longitudinal exclusion rule");
    }
    if let Some(column) = exclusion_column.as_ref()
        && !portable.columns.iter().any(|existing| existing == column)
    {
        bail!("metadata exclusion column '{column}' does not exist");
    }
    let excluded_values = if config.exclusion_enabled {
        config
            .exclusion_values
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    if config.exclusion_enabled && excluded_values.is_empty() {
        bail!("Select at least one value for the enabled longitudinal exclusion rule");
    }

    let metadata = MetadataTable {
        path: PathBuf::from(metadata_file_name),
        columns: portable.columns,
        rows: portable.rows,
        id_column: observation_column.to_string(),
    };
    let longitudinal_config = LongitudinalClusteringConfig {
        observation_id_column: observation_column.to_string(),
        patient_id_column: patient_column.to_string(),
        time_column,
        qc_status_column: exclusion_column,
        excluded_qc_values: excluded_values,
    };
    let index = build_metadata_index(&metadata, &longitudinal_config)?;
    if index.eligible_rows < 2 || index.eligible_ids.len() < 2 {
        bail!("Fewer than two metadata observations remain eligible for longitudinal clustering");
    }

    let mut spec = CorrectedModelSpec::default();
    spec.min_count = config.min_count.max(0.0);
    spec.min_fraction_samples = config.min_fraction_samples.clamp(0.0, 1.0);
    let (source_metric, feature_label, modality_label) = match config.kind {
        PortableExpressionKind::Gene => ("Raw Count", "Gene", "Gene expression"),
        PortableExpressionKind::Transcript => (
            "Total Reads",
            "Transcript",
            "Transcript / isoform expression",
        ),
    };
    let raw = run_unsupervised_expression_distance_bytes(
        merged_bytes,
        &index.eligible_ids,
        source_metric,
        feature_label,
        &spec,
        progress,
    )?;
    let modality = summarize_modality(modality_label, raw, &index, &longitudinal_config)?;
    let mut warnings = index.warnings;
    warnings.extend(modality.warnings.iter().cloned());

    Ok(PortableLongitudinalResult {
        kind: config.kind,
        metadata_file: metadata_file_name.to_string(),
        merged_expression_file: merged_file_name.to_string(),
        metadata_rows: metadata.rows.len(),
        eligible_metadata_rows: index.eligible_rows,
        qc_excluded_rows: index.qc_excluded_rows,
        missing_identifier_rows: index.missing_identifier_rows,
        ambiguous_observation_keys: index.ambiguous_keys,
        excluded_rows: index.excluded_rows,
        modality,
        warnings,
    })
}
