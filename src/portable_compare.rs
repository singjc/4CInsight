//! Browser-safe metadata-driven expression comparison orchestration.
//!
//! This module owns the small amount of configuration/mapping logic needed by
//! the local web application. The statistical work is delegated to the same
//! corrected-expression engine used by the native Rust analysis. The only
//! difference is that the merged expression table is read from bytes.

use crate::cohort::{MetadataComparisonSpec, MetadataTable, build_custom_comparison};
use crate::corrected::{
    CorrectedExpressionResult, CorrectedModelSpec, run_corrected_expression_bytes,
};
use crate::portable_io::{PortableExpressionKind, parse_metadata_bytes};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableExclusionRule {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub column: String,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableExpressionComparisonConfig {
    pub kind: PortableExpressionKind,
    pub metadata_id_column: String,
    pub group_column: String,
    pub case_values: Vec<String>,
    pub control_values: Vec<String>,
    #[serde(default)]
    pub comparison_name: String,
    #[serde(default)]
    pub case_label: String,
    #[serde(default)]
    pub control_label: String,
    #[serde(default)]
    pub subject_column: Option<String>,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default)]
    pub exclusions: Vec<PortableExclusionRule>,
    #[serde(default = "default_min_count")]
    pub min_count: f64,
    #[serde(default = "default_min_fraction_samples")]
    pub min_fraction_samples: f64,
    #[serde(default = "default_permutations")]
    pub permutations: usize,
}

fn default_min_count() -> f64 {
    10.0
}

fn default_min_fraction_samples() -> f64 {
    0.20
}

fn default_permutations() -> usize {
    499
}

#[derive(Debug, Clone, Serialize)]
pub struct PortableExclusionSummary {
    pub label: String,
    pub column: String,
    pub values: Vec<String>,
    pub matching_rows: usize,
    pub excluded_sample_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortableComparisonSummary {
    pub name: String,
    pub case_label: String,
    pub control_label: String,
    pub metadata_rows_before_exclusions: usize,
    pub metadata_rows_after_exclusions: usize,
    pub excluded_rows: usize,
    pub case_metadata_rows: usize,
    pub control_metadata_rows: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortableExpressionComparisonResult {
    pub kind: PortableExpressionKind,
    pub source_metric: String,
    pub metadata_file: String,
    pub merged_expression_file: String,
    pub comparison: PortableComparisonSummary,
    pub exclusions: Vec<PortableExclusionSummary>,
    pub analysis: CorrectedExpressionResult,
}

struct ActiveRule {
    label: String,
    column: String,
    column_index: usize,
    values: Vec<String>,
    matching_rows: usize,
    excluded_sample_ids: BTreeSet<String>,
}

pub fn run_portable_expression_comparison<F>(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config: &PortableExpressionComparisonConfig,
    progress: F,
) -> Result<PortableExpressionComparisonResult>
where
    F: FnMut(usize, usize, &str),
{
    if config.case_values.is_empty() || config.control_values.is_empty() {
        bail!("Select at least one case value and one control value");
    }
    if config.metadata_id_column.trim().is_empty() {
        bail!("Select the metadata column that identifies expression samples");
    }
    if config.group_column.trim().is_empty() {
        bail!("Select a metadata grouping column");
    }

    let overlap = config
        .case_values
        .iter()
        .filter(|case| {
            config
                .control_values
                .iter()
                .any(|control| control.eq_ignore_ascii_case(case))
        })
        .cloned()
        .collect::<Vec<_>>();
    if !overlap.is_empty() {
        bail!("Case and control values overlap: {}", overlap.join(", "));
    }

    let portable = parse_metadata_bytes(metadata_file_name, metadata_bytes)?;
    let id_index = portable
        .columns
        .iter()
        .position(|name| name == config.metadata_id_column.trim())
        .ok_or_else(|| {
            anyhow!(
                "metadata sample-ID column '{}' does not exist",
                config.metadata_id_column
            )
        })?;
    if !portable
        .columns
        .iter()
        .any(|name| name == config.group_column.trim())
    {
        bail!(
            "metadata grouping column '{}' does not exist",
            config.group_column
        );
    }

    let subject_column = config
        .subject_column
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(column) = subject_column.as_ref()
        && !portable.columns.iter().any(|name| name == column)
    {
        bail!("metadata subject/patient column '{column}' does not exist");
    }
    for covariate in &config.covariates {
        let covariate = covariate.trim();
        if !covariate.is_empty() && !portable.columns.iter().any(|name| name == covariate) {
            bail!("metadata covariate column '{covariate}' does not exist");
        }
    }

    let mut active_rules = Vec::<ActiveRule>::new();
    for (index, rule) in config.exclusions.iter().enumerate() {
        if !rule.enabled {
            continue;
        }
        let column = rule.column.trim();
        if column.is_empty() || rule.values.is_empty() {
            continue;
        }
        let column_index = portable
            .columns
            .iter()
            .position(|name| name == column)
            .ok_or_else(|| anyhow!("metadata exclusion column '{column}' does not exist"))?;
        let values = rule
            .values
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if values.is_empty() {
            continue;
        }
        active_rules.push(ActiveRule {
            label: if rule.label.trim().is_empty() {
                format!("Exclusion rule {}", index + 1)
            } else {
                rule.label.trim().to_string()
            },
            column: column.to_string(),
            column_index,
            values,
            matching_rows: 0,
            excluded_sample_ids: BTreeSet::new(),
        });
    }

    let rows_before = portable.rows.len();
    let mut rows = Vec::with_capacity(rows_before);
    let mut excluded_rows = 0usize;
    for row in portable.rows {
        let sample_id = row
            .get(id_index)
            .map(String::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let mut excluded = false;
        for rule in &mut active_rules {
            let observed = row
                .get(rule.column_index)
                .map(String::as_str)
                .unwrap_or_default()
                .trim();
            if rule
                .values
                .iter()
                .any(|value| value.eq_ignore_ascii_case(observed))
            {
                rule.matching_rows += 1;
                if !sample_id.is_empty() {
                    rule.excluded_sample_ids.insert(sample_id.clone());
                }
                excluded = true;
            }
        }
        if excluded {
            excluded_rows += 1;
        } else {
            rows.push(row);
        }
    }

    let metadata = MetadataTable {
        path: PathBuf::from(metadata_file_name),
        columns: portable.columns,
        rows,
        id_column: config.metadata_id_column.trim().to_string(),
    };

    let case_values = config
        .case_values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let control_values = config
        .control_values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if case_values.is_empty() || control_values.is_empty() {
        bail!("Case and control selections cannot be empty after trimming values");
    }

    let case_label = if config.case_label.trim().is_empty() {
        case_values.iter().cloned().collect::<Vec<_>>().join(" + ")
    } else {
        config.case_label.trim().to_string()
    };
    let control_label = if config.control_label.trim().is_empty() {
        control_values
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" + ")
    } else {
        config.control_label.trim().to_string()
    };
    let comparison_name = if config.comparison_name.trim().is_empty() {
        format!("{case_label} vs {control_label}")
    } else {
        config.comparison_name.trim().to_string()
    };

    let comparison = build_custom_comparison(
        &metadata,
        &MetadataComparisonSpec {
            column: config.group_column.trim().to_string(),
            case_values,
            control_values,
            name: comparison_name,
            case_label,
            control_label,
        },
    )?;

    let mut seen_covariates = HashSet::<String>::new();
    let covariates = config
        .covariates
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .filter(|value| seen_covariates.insert((*value).to_string()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut model_spec = CorrectedModelSpec::default();
    model_spec.subject_column = subject_column;
    model_spec.covariates = covariates;
    model_spec.min_count = if config.min_count.is_finite() {
        config.min_count.max(0.0)
    } else {
        default_min_count()
    };
    model_spec.min_fraction_samples = if config.min_fraction_samples.is_finite() {
        config.min_fraction_samples.clamp(0.0, 1.0)
    } else {
        default_min_fraction_samples()
    };
    model_spec.permutations = config.permutations;

    let (source_metric, feature_label) = match config.kind {
        PortableExpressionKind::Gene => ("Raw Count", "Gene"),
        PortableExpressionKind::Transcript => ("Total Reads", "Transcript"),
    };
    let analysis = run_corrected_expression_bytes(
        merged_file_name,
        merged_bytes,
        &metadata,
        &comparison,
        source_metric,
        feature_label,
        &model_spec,
        progress,
    )?;

    let exclusions = active_rules
        .into_iter()
        .map(|rule| PortableExclusionSummary {
            label: rule.label,
            column: rule.column,
            values: rule.values,
            matching_rows: rule.matching_rows,
            excluded_sample_ids: rule.excluded_sample_ids.into_iter().collect(),
        })
        .collect::<Vec<_>>();

    Ok(PortableExpressionComparisonResult {
        kind: config.kind,
        source_metric: source_metric.to_string(),
        metadata_file: metadata_file_name.to_string(),
        merged_expression_file: merged_file_name.to_string(),
        comparison: PortableComparisonSummary {
            name: comparison.name.clone(),
            case_label: comparison.case_label.clone(),
            control_label: comparison.control_label.clone(),
            metadata_rows_before_exclusions: rows_before,
            metadata_rows_after_exclusions: metadata.rows.len(),
            excluded_rows,
            case_metadata_rows: comparison.case_metadata_rows,
            control_metadata_rows: comparison.control_metadata_rows,
        },
        exclusions,
        analysis,
    })
}
