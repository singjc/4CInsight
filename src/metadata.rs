//! Standardized multi-omics sample metadata schema and helpers.
//!
//! The schema is deliberately tabular: one row per biological sample, one
//! canonical sample identifier, optional subject/covariate fields, and optional
//! per-modality aliases when upstream files use different sample names.

use crate::cohort::{MetadataTable, normalize_sample_id};
use anyhow::{Context, Result};
use csv::WriterBuilder;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataFieldLevel {
    Required,
    Recommended,
    Optional,
    Legacy,
}

impl MetadataFieldLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Recommended => "recommended",
            Self::Optional => "optional",
            Self::Legacy => "legacy compatibility",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StandardMetadataField {
    pub name: &'static str,
    pub level: MetadataFieldLevel,
    pub section: &'static str,
    pub description: &'static str,
}

pub const MODALITY_ALIAS_COLUMNS: [&str; 7] = [
    "gene_expression_id",
    "transcript_expression_id",
    "bcr_id",
    "bcr_isotype_id",
    "tcr_alpha_id",
    "tcr_beta_id",
    "immune_infiltrate_id",
];

pub const STANDARD_METADATA_FIELDS: &[StandardMetadataField] = &[
    StandardMetadataField {
        name: "sample_id",
        level: MetadataFieldLevel::Required,
        section: "Identity",
        description: "Canonical unique sample identifier used across the project.",
    },
    StandardMetadataField {
        name: "subject_id",
        level: MetadataFieldLevel::Recommended,
        section: "Identity",
        description: "Participant/patient identifier for repeated-measure and subject-clustered analyses.",
    },
    StandardMetadataField {
        name: "analysis_group",
        level: MetadataFieldLevel::Recommended,
        section: "Cohort",
        description: "Preferred standardized categorical grouping field for custom comparisons.",
    },
    StandardMetadataField {
        name: "cohort",
        level: MetadataFieldLevel::Optional,
        section: "Cohort",
        description: "Study cohort or recruitment stratum.",
    },
    StandardMetadataField {
        name: "diagnosis",
        level: MetadataFieldLevel::Optional,
        section: "Cohort",
        description: "Primary diagnosis or disease label.",
    },
    StandardMetadataField {
        name: "disease_subtype",
        level: MetadataFieldLevel::Optional,
        section: "Cohort",
        description: "Optional disease subtype / phenotype subclassification.",
    },
    StandardMetadataField {
        name: "Phenotype",
        level: MetadataFieldLevel::Legacy,
        section: "Cohort",
        description: "Legacy 4C phenotype field retained for the built-in HYP comparisons.",
    },
    StandardMetadataField {
        name: "HYP",
        level: MetadataFieldLevel::Legacy,
        section: "Cohort",
        description: "Legacy 4C yes/no HYP field retained for the built-in HYP comparisons.",
    },
    StandardMetadataField {
        name: "timepoint",
        level: MetadataFieldLevel::Recommended,
        section: "Collection",
        description: "Longitudinal timepoint or visit label.",
    },
    StandardMetadataField {
        name: "collection_date",
        level: MetadataFieldLevel::Optional,
        section: "Collection",
        description: "Collection date, preferably ISO YYYY-MM-DD.",
    },
    StandardMetadataField {
        name: "sample_source",
        level: MetadataFieldLevel::Recommended,
        section: "Specimen",
        description: "Source material, e.g. PBMC, whole blood, tumor tissue.",
    },
    StandardMetadataField {
        name: "tissue",
        level: MetadataFieldLevel::Optional,
        section: "Specimen",
        description: "Tissue/anatomical source when relevant.",
    },
    StandardMetadataField {
        name: "specimen_type",
        level: MetadataFieldLevel::Optional,
        section: "Specimen",
        description: "Specimen preparation/type, e.g. cryopreserved PBMC, plasma.",
    },
    StandardMetadataField {
        name: "sex",
        level: MetadataFieldLevel::Optional,
        section: "Covariates",
        description: "Sex/gender field if appropriate for the scientific analysis.",
    },
    StandardMetadataField {
        name: "age_years",
        level: MetadataFieldLevel::Optional,
        section: "Covariates",
        description: "Age at collection in years.",
    },
    StandardMetadataField {
        name: "family_id",
        level: MetadataFieldLevel::Optional,
        section: "Covariates",
        description: "Family/household identifier when relatedness may matter.",
    },
    StandardMetadataField {
        name: "site",
        level: MetadataFieldLevel::Optional,
        section: "Provenance",
        description: "Collection/recruitment site.",
    },
    StandardMetadataField {
        name: "study_id",
        level: MetadataFieldLevel::Optional,
        section: "Provenance",
        description: "Study/project identifier for combined cohorts.",
    },
    StandardMetadataField {
        name: "collection_batch",
        level: MetadataFieldLevel::Optional,
        section: "Batch",
        description: "Collection wave/batch.",
    },
    StandardMetadataField {
        name: "processing_batch",
        level: MetadataFieldLevel::Recommended,
        section: "Batch",
        description: "Sample-processing/extraction batch.",
    },
    StandardMetadataField {
        name: "library_batch",
        level: MetadataFieldLevel::Optional,
        section: "Batch",
        description: "Library-preparation batch.",
    },
    StandardMetadataField {
        name: "sequencing_batch",
        level: MetadataFieldLevel::Optional,
        section: "Batch",
        description: "Sequencing/run batch.",
    },
    StandardMetadataField {
        name: "plate",
        level: MetadataFieldLevel::Optional,
        section: "Batch",
        description: "Plate identifier.",
    },
    StandardMetadataField {
        name: "well",
        level: MetadataFieldLevel::Optional,
        section: "Batch",
        description: "Plate well.",
    },
    StandardMetadataField {
        name: "technical_replicate",
        level: MetadataFieldLevel::Optional,
        section: "Batch",
        description: "Technical replicate identifier/number.",
    },
    StandardMetadataField {
        name: "gene_expression_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by gene-expression files/merged columns when different from sample_id.",
    },
    StandardMetadataField {
        name: "transcript_expression_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by transcript-expression files/merged columns when different from sample_id.",
    },
    StandardMetadataField {
        name: "bcr_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by BCR files when different from sample_id.",
    },
    StandardMetadataField {
        name: "bcr_isotype_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by BCR-isotype files when different from sample_id.",
    },
    StandardMetadataField {
        name: "tcr_alpha_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by TCR-alpha files when different from sample_id.",
    },
    StandardMetadataField {
        name: "tcr_beta_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by TCR-beta files when different from sample_id.",
    },
    StandardMetadataField {
        name: "immune_infiltrate_id",
        level: MetadataFieldLevel::Optional,
        section: "Modality mapping",
        description: "Alias used by immune-infiltrate files when different from sample_id.",
    },
    StandardMetadataField {
        name: "analysis_include",
        level: MetadataFieldLevel::Optional,
        section: "Review",
        description: "Curator eligibility note (yes/no). It is not a substitute for frozen per-run QC review.",
    },
    StandardMetadataField {
        name: "known_wrong_source",
        level: MetadataFieldLevel::Optional,
        section: "Review",
        description: "Curator flag for a known wrong tissue/source sample (yes/no).",
    },
    StandardMetadataField {
        name: "qc_review_status",
        level: MetadataFieldLevel::Optional,
        section: "Review",
        description: "Curator status such as include, review, or exclude.",
    },
    StandardMetadataField {
        name: "qc_exclusion_reason",
        level: MetadataFieldLevel::Optional,
        section: "Review",
        description: "Human-readable curator reason; run-specific automatic/manual QC remains frozen separately.",
    },
    StandardMetadataField {
        name: "notes",
        level: MetadataFieldLevel::Optional,
        section: "Review",
        description: "Free-text provenance or curation notes.",
    },
];

#[derive(Debug, Clone, Default)]
pub struct MetadataValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub normalized_duplicate_ids: Vec<String>,
    pub comparison_columns: Vec<String>,
}

impl MetadataValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn standard_metadata_columns() -> Vec<String> {
    STANDARD_METADATA_FIELDS
        .iter()
        .map(|field| field.name.to_string())
        .collect()
}

pub fn new_standard_metadata(sample_ids: &[String]) -> MetadataTable {
    let columns = standard_metadata_columns();
    let sample_col = columns.iter().position(|c| c == "sample_id").unwrap_or(0);
    let rows = sample_ids
        .iter()
        .map(|sample| {
            let mut row = vec![String::new(); columns.len()];
            row[sample_col] = sample.clone();
            for alias in MODALITY_ALIAS_COLUMNS {
                if let Some(idx) = columns.iter().position(|c| c == alias) {
                    row[idx] = sample.clone();
                }
            }
            row
        })
        .collect();
    MetadataTable {
        path: PathBuf::new(),
        columns,
        rows,
        id_column: "sample_id".to_string(),
    }
}

pub fn add_missing_standard_columns(metadata: &mut MetadataTable) -> usize {
    let mut added = 0usize;
    for field in STANDARD_METADATA_FIELDS {
        if metadata.column_index(field.name).is_some() {
            continue;
        }
        let copy_from_id = field.name == "sample_id";
        let id_values = if copy_from_id {
            (0..metadata.rows.len())
                .map(|row| metadata.id_value(row).unwrap_or_default().to_string())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        metadata.columns.push(field.name.to_string());
        for (row_idx, row) in metadata.rows.iter_mut().enumerate() {
            row.push(if copy_from_id {
                id_values.get(row_idx).cloned().unwrap_or_default()
            } else {
                String::new()
            });
        }
        added += 1;
    }
    added
}

pub fn unique_values(metadata: &MetadataTable, column: &str) -> Vec<(String, usize)> {
    let Some(idx) = metadata.column_index(column) else {
        return Vec::new();
    };
    let mut counts = BTreeMap::<String, usize>::new();
    for row in &metadata.rows {
        let value = row.get(idx).map(String::as_str).unwrap_or_default().trim();
        if !value.is_empty() {
            *counts.entry(value.to_string()).or_default() += 1;
        }
    }
    counts.into_iter().collect()
}

pub fn canonical_sample_id(metadata: &MetadataTable, row_idx: usize) -> Option<&str> {
    metadata
        .value(row_idx, "sample_id")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            metadata
                .id_value(row_idx)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
}

pub fn row_matching_ids(metadata: &MetadataTable, row_idx: usize) -> Vec<String> {
    let Some(row) = metadata.rows.get(row_idx) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    let mut push_column = |name: &str| {
        let Some(idx) = metadata.column_index(name) else {
            return;
        };
        let raw = row.get(idx).map(String::as_str).unwrap_or_default();
        let key = normalize_sample_id(raw);
        if !key.is_empty() && !ids.contains(&key) {
            ids.push(key);
        }
    };
    push_column(&metadata.id_column);
    if metadata.id_column != "sample_id" {
        push_column("sample_id");
    }
    for alias in MODALITY_ALIAS_COLUMNS {
        push_column(alias);
    }
    ids
}

pub fn validate_standard_metadata(metadata: &MetadataTable) -> MetadataValidationReport {
    let mut report = MetadataValidationReport::default();
    if metadata.column_index(&metadata.id_column).is_none() {
        report.errors.push(format!(
            "Selected sample ID column '{}' is missing.",
            metadata.id_column
        ));
        return report;
    }
    if metadata.column_index("sample_id").is_none() {
        report.errors.push(
            "Required standard column 'sample_id' is missing; add the standard columns before exporting."
                .to_string(),
        );
    } else if let Some(sample_idx) = metadata.column_index("sample_id") {
        for (row_idx, row) in metadata.rows.iter().enumerate() {
            if row
                .get(sample_idx)
                .map(String::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                report.errors.push(format!(
                    "Row {} has an empty canonical sample_id.",
                    row_idx + 2
                ));
            }
        }
    }

    let mut seen = HashMap::<String, Vec<usize>>::new();
    for row_idx in 0..metadata.rows.len() {
        let raw = metadata.id_value(row_idx).unwrap_or_default().trim();
        let key = normalize_sample_id(raw);
        if key.is_empty() {
            report
                .errors
                .push(format!("Row {} has an empty sample ID.", row_idx + 2));
        } else {
            seen.entry(key).or_default().push(row_idx + 2);
        }
    }
    for (key, rows) in seen {
        if rows.len() > 1 {
            report.normalized_duplicate_ids.push(key.clone());
            report.errors.push(format!(
                "Normalized sample ID '{key}' occurs on metadata rows {}.",
                rows.iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    for column in [
        "subject_id",
        "analysis_group",
        "sample_source",
        "processing_batch",
    ] {
        if metadata.column_index(column).is_none() {
            report
                .warnings
                .push(format!("Recommended column '{column}' is missing."));
        }
    }

    for field in ["analysis_include", "known_wrong_source"] {
        let Some(idx) = metadata.column_index(field) else {
            continue;
        };
        for (row_idx, row) in metadata.rows.iter().enumerate() {
            let value = row.get(idx).map(String::as_str).unwrap_or_default().trim();
            if value.is_empty() {
                continue;
            }
            let ok = matches!(
                value.to_ascii_lowercase().as_str(),
                "yes" | "no" | "true" | "false" | "1" | "0"
            );
            if !ok {
                report.warnings.push(format!(
                    "Row {} column '{field}' has value '{value}'; prefer yes/no.",
                    row_idx + 2
                ));
            }
        }
    }

    let mut alias_rows = HashMap::<String, Vec<usize>>::new();
    for row_idx in 0..metadata.rows.len() {
        for key in row_matching_ids(metadata, row_idx) {
            let entry = alias_rows.entry(key).or_default();
            if !entry.contains(&(row_idx + 2)) {
                entry.push(row_idx + 2);
            }
        }
    }
    for (key, rows) in alias_rows {
        if rows.len() > 1 {
            report.errors.push(format!(
                "Sample/alias identifier '{key}' maps to multiple metadata rows: {}.",
                rows.iter()
                    .map(|r| r.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    for column in &metadata.columns {
        let values = unique_values(metadata, column);
        if values.len() >= 2 && values.len() <= 20 && column != &metadata.id_column {
            report.comparison_columns.push(column.clone());
        }
    }
    report
}

pub fn write_metadata_tsv(metadata: &MetadataTable, path: &Path) -> Result<()> {
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(path)
        .with_context(|| format!("Could not create {}", path.display()))?;
    writer.write_record(&metadata.columns)?;
    for row in &metadata.rows {
        let mut complete = row.clone();
        complete.resize(metadata.columns.len(), String::new());
        writer.write_record(complete)?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_template_has_required_identity_fields() {
        let metadata = new_standard_metadata(&["S1".to_string(), "S2".to_string()]);
        assert_eq!(metadata.id_column, "sample_id");
        assert!(metadata.column_index("subject_id").is_some());
        assert!(metadata.column_index("analysis_group").is_some());
        assert!(metadata.column_index("gene_expression_id").is_some());
        assert!(validate_standard_metadata(&metadata).is_valid());
    }

    #[test]
    fn validation_rejects_alias_collisions_between_samples() {
        let mut metadata = new_standard_metadata(&["S1".to_string(), "S2".to_string()]);
        let alias = metadata.column_index("gene_expression_id").unwrap();
        metadata.rows[0][alias] = "RNA1".to_string();
        metadata.rows[1][alias] = "RNA1".to_string();
        let report = validate_standard_metadata(&metadata);
        assert!(!report.is_valid());
        assert!(report.errors.iter().any(|message| message.contains("RNA1")));
    }
}
