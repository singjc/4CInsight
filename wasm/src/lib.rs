use serde::Serialize;
use wasm_bindgen::prelude::*;

#[path = "../../src/input_conventions.rs"]
mod input_conventions;

use input_conventions::{FilenameInputKind, classify_input_filename};

#[derive(Serialize)]
struct EngineInfo<'a> {
    product: &'a str,
    package: &'a str,
    version: &'a str,
    execution: &'a str,
}

#[derive(Serialize)]
struct FilenameClassification<'a> {
    recognized: bool,
    kind: Option<&'a str>,
    label: Option<&'a str>,
    sample_id: Option<String>,
}

fn kind_slug(kind: FilenameInputKind) -> &'static str {
    match kind {
        FilenameInputKind::GeneExpression => "gene-expression",
        FilenameInputKind::TranscriptExpression => "transcript-expression",
        FilenameInputKind::Bcr => "bcr",
        FilenameInputKind::BcrIsotype => "bcr-isotype",
        FilenameInputKind::TcrAlpha => "tcr-alpha",
        FilenameInputKind::TcrBeta => "tcr-beta",
        FilenameInputKind::ImmuneInfiltrate => "immune-infiltrate",
        FilenameInputKind::MergedGeneExpression => "merged-gene-expression",
        FilenameInputKind::MergedTranscriptExpression => "merged-transcript-expression",
        FilenameInputKind::MetadataCandidate => "metadata-candidate",
    }
}

#[wasm_bindgen]
pub fn engine_info_json() -> String {
    serde_json::to_string(&EngineInfo {
        product: "4C Insight",
        package: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        execution: "local-browser-wasm",
    })
    .expect("engine info serialization is infallible")
}

#[wasm_bindgen]
pub fn classify_filename_json(name: &str) -> String {
    let classification = match classify_input_filename(name) {
        Some(value) => FilenameClassification {
            recognized: true,
            kind: Some(kind_slug(value.kind)),
            label: Some(value.kind.display_name()),
            sample_id: value.sample_id,
        },
        None => FilenameClassification {
            recognized: false,
            kind: None,
            label: None,
            sample_id: None,
        },
    };

    serde_json::to_string(&classification).expect("filename classification serialization is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_bridge_reuses_shared_filename_conventions() {
        let json = classify_filename_json("4C_465_tumor_rna_gene_expression_report.xlsx");
        assert!(json.contains("gene-expression"));
        assert!(json.contains("4C_465"));
    }

    #[test]
    fn unrelated_files_remain_unrecognized() {
        let json = classify_filename_json("notes.pdf");
        assert!(json.contains("\"recognized\":false"));
    }
}
