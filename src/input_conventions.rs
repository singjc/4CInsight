//! Shared 4C input filename conventions used by local input adapters and the
//! WASM browser preflight.
//!
//! These helpers only classify names. They never authorize access, choose a
//! storage path, or replace content validation/mapping.

/// Filename-level 4C input kind recognized by the native project conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FilenameInputKind {
    GeneExpression,
    TranscriptExpression,
    Bcr,
    BcrIsotype,
    TcrAlpha,
    TcrBeta,
    ImmuneInfiltrate,
    MergedGeneExpression,
    MergedTranscriptExpression,
    MetadataCandidate,
}

impl FilenameInputKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::GeneExpression => "Gene expression",
            Self::TranscriptExpression => "Transcript expression",
            Self::Bcr => "BCR",
            Self::BcrIsotype => "BCR isotype",
            Self::TcrAlpha => "TCR alpha",
            Self::TcrBeta => "TCR beta",
            Self::ImmuneInfiltrate => "Immune infiltrate",
            Self::MergedGeneExpression => "Merged gene expression",
            Self::MergedTranscriptExpression => "Merged transcript expression",
            Self::MetadataCandidate => "Metadata candidate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilenameInputClassification {
    pub kind: FilenameInputKind,
    pub sample_id: Option<String>,
}

/// Classify a filename using the same exact sample-workbook suffix conventions
/// used by the established 4C sample naming rules, plus the merged-TSV and
/// metadata-candidate conventions.
pub fn classify_input_filename(name: &str) -> Option<FilenameInputClassification> {
    let canonical_name = strip_xlsx_copy_suffix(name);
    let lower = canonical_name.to_ascii_lowercase();
    let workbook_suffixes = [
        (
            "_tumor_rna_gene_expression_report.xlsx",
            FilenameInputKind::GeneExpression,
        ),
        (
            "_tumor_rna_transcript_expression_report.xlsx",
            FilenameInputKind::TranscriptExpression,
        ),
        ("_bcr_heavy_chain_report.xlsx", FilenameInputKind::Bcr),
        (
            "_bcr_isotype_composition.xlsx",
            FilenameInputKind::BcrIsotype,
        ),
        ("_tcr_alpha_clone_report.xlsx", FilenameInputKind::TcrAlpha),
        ("_tcr_beta_clone_report.xlsx", FilenameInputKind::TcrBeta),
        (
            "_immune_infiltrate.xlsx",
            FilenameInputKind::ImmuneInfiltrate,
        ),
    ];
    for (suffix, kind) in workbook_suffixes {
        if lower.ends_with(suffix) {
            let prefix_len = canonical_name.len().saturating_sub(suffix.len());
            let sample_id = canonical_name[..prefix_len].trim().to_string();
            if !sample_id.is_empty() {
                return Some(FilenameInputClassification {
                    kind,
                    sample_id: Some(sample_id),
                });
            }
        }
    }

    let raw_lower = name.to_ascii_lowercase();
    if raw_lower.ends_with(".tsv") {
        if raw_lower.contains("merged_gene_expression") {
            return Some(FilenameInputClassification {
                kind: FilenameInputKind::MergedGeneExpression,
                sample_id: None,
            });
        }
        if raw_lower.contains("merged_transcript_expression") {
            return Some(FilenameInputClassification {
                kind: FilenameInputKind::MergedTranscriptExpression,
                sample_id: None,
            });
        }
    }

    let metadata_extension = [".xlsx", ".xls", ".tsv", ".csv", ".txt"]
        .iter()
        .any(|extension| raw_lower.ends_with(extension));
    if metadata_extension
        && (raw_lower.contains("sample") || raw_lower.contains("cohort"))
        && (raw_lower.contains("submission") || raw_lower.contains("metadata"))
    {
        return Some(FilenameInputClassification {
            kind: FilenameInputKind::MetadataCandidate,
            sample_id: None,
        });
    }

    None
}

fn strip_xlsx_copy_suffix(name: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_native_sample_workbooks_and_copy_suffixes() {
        let gene =
            classify_input_filename("4C_465_tumor_rna_gene_expression_report (1).xlsx").unwrap();
        assert_eq!(gene.kind, FilenameInputKind::GeneExpression);
        assert_eq!(gene.sample_id.as_deref(), Some("4C_465"));

        let bcr = classify_input_filename("Normal_FJ_RNA_13_bcr_heavy_chain_report.xlsx").unwrap();
        assert_eq!(bcr.kind, FilenameInputKind::Bcr);
        assert_eq!(bcr.sample_id.as_deref(), Some("Normal_FJ_RNA_13"));
    }

    #[test]
    fn classifies_merged_tables_and_metadata_candidates() {
        assert_eq!(
            classify_input_filename("merged_gene_expression.tsv")
                .unwrap()
                .kind,
            FilenameInputKind::MergedGeneExpression
        );
        assert_eq!(
            classify_input_filename("cohort_sample_metadata.xlsx")
                .unwrap()
                .kind,
            FilenameInputKind::MetadataCandidate
        );
        assert_eq!(
            classify_input_filename("cohort_sample_metadata.tsv")
                .unwrap()
                .kind,
            FilenameInputKind::MetadataCandidate
        );
        assert!(classify_input_filename("h.all.v2026.1.Hs.symbols.gmt").is_none());
    }
}
