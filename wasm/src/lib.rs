use serde::Serialize;
use wasm_bindgen::prelude::*;

#[path = "../../src/input_conventions.rs"]
mod input_conventions;
#[allow(dead_code)]
#[path = "../../src/cohort.rs"]
mod cohort;
#[allow(dead_code)]
#[path = "../../src/metadata.rs"]
mod metadata;
#[allow(dead_code)]
#[path = "../../src/corrected.rs"]
mod corrected;
#[allow(dead_code)]
#[path = "../../src/portable_io.rs"]
mod portable_io;
#[path = "../../src/portable_compare.rs"]
mod portable_compare;

use input_conventions::{FilenameInputKind, classify_input_filename};
use portable_compare::{
    PortableExpressionComparisonConfig, run_portable_expression_comparison,
};
use portable_io::{
    PortableExpressionKind, PortableExpressionMetric, inspect_metadata_bytes,
    metadata_column_values_bytes, summarize_expression_bytes,
};

#[derive(Serialize)]
struct EngineInfo<'a> {
    product: &'a str,
    package: &'a str,
    version: &'a str,
    execution: &'a str,
    capabilities: [&'a str; 5],
}

#[derive(Serialize)]
struct FilenameClassification<'a> {
    recognized: bool,
    kind: Option<&'a str>,
    label: Option<&'a str>,
    #[serde(rename = "sampleId")]
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

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[wasm_bindgen]
pub fn engine_info_json() -> String {
    serde_json::to_string(&EngineInfo {
        product: "4C Insight",
        package: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        execution: "local-browser-wasm",
        capabilities: [
            "filename-classification",
            "metadata",
            "expression-qc",
            "metadata-grouping",
            "corrected-expression-comparison",
        ],
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

#[wasm_bindgen]
pub fn inspect_metadata_json(
    file_name: &str,
    bytes: &[u8],
    id_column: &str,
) -> Result<String, JsValue> {
    let selected_id = (!id_column.trim().is_empty()).then_some(id_column);
    let inspection = inspect_metadata_bytes(file_name, bytes, selected_id).map_err(js_error)?;
    serde_json::to_string(&inspection).map_err(js_error)
}

#[wasm_bindgen]
pub fn summarize_expression_json(
    file_name: &str,
    bytes: &[u8],
    requested_kind: &str,
    metric: &str,
    ignore_zeros: bool,
) -> Result<String, JsValue> {
    let requested_kind = PortableExpressionKind::parse(requested_kind).map_err(js_error)?;
    let metric = PortableExpressionMetric::parse(metric).map_err(js_error)?;
    let summary = summarize_expression_bytes(
        file_name,
        bytes,
        requested_kind,
        metric,
        ignore_zeros,
    )
    .map_err(js_error)?;
    serde_json::to_string(&summary).map_err(js_error)
}


#[wasm_bindgen]
pub fn metadata_column_values_json(
    file_name: &str,
    bytes: &[u8],
    column: &str,
) -> Result<String, JsValue> {
    let values = metadata_column_values_bytes(file_name, bytes, column).map_err(js_error)?;
    serde_json::to_string(&values).map_err(js_error)
}

fn run_expression_comparison_json_core(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config_json: &str,
) -> Result<String, String> {
    let config: PortableExpressionComparisonConfig =
        serde_json::from_str(config_json).map_err(|error| error.to_string())?;
    let result = run_portable_expression_comparison(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        &config,
        |_done, _total, _message| {},
    )
    .map_err(|error| error.to_string())?;
    serde_json::to_string(&result).map_err(|error| error.to_string())
}

#[wasm_bindgen]
pub fn run_expression_comparison_json(
    metadata_file_name: &str,
    metadata_bytes: &[u8],
    merged_file_name: &str,
    merged_bytes: &[u8],
    config_json: &str,
) -> Result<String, JsValue> {
    run_expression_comparison_json_core(
        metadata_file_name,
        metadata_bytes,
        merged_file_name,
        merged_bytes,
        config_json,
    )
    .map_err(js_error)
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

    #[test]
    fn metadata_fixture_is_parsed_by_wasm_bridge() {
        let bytes = include_bytes!("../../tests/fixtures/w2_metadata.tsv");
        let json = inspect_metadata_json("w2_metadata.tsv", bytes, "sample_id").unwrap();
        assert!(json.contains("\"rows\":3"));
        assert!(json.contains("\"id_column\":\"sample_id\""));
    }

    #[test]
    fn gene_expression_fixture_is_summarized_by_wasm_bridge() {
        let bytes = include_bytes!("../../tests/fixtures/S1_tumor_rna_gene_expression_report.xlsx");
        let json = summarize_expression_json(
            "S1_tumor_rna_gene_expression_report.xlsx",
            bytes,
            "gene-expression",
            "tpm",
            false,
        )
        .unwrap();
        assert!(json.contains("\"sample\":\"S1\""));
        assert!(json.contains("\"median\":3.0"));
    }


    #[test]
    fn w3_gene_comparison_runs_through_wasm_bridge() {
        let metadata = include_bytes!("../../tests/fixtures/w3_metadata.tsv");
        let merged = include_bytes!("../../tests/fixtures/w3_merged_gene_expression.tsv");
        let config = serde_json::json!({
            "kind": "gene-expression",
            "metadataIdColumn": "sample_id",
            "groupColumn": "analysis_group",
            "caseValues": ["Case"],
            "controlValues": ["Control"],
            "comparisonName": "Case vs Control",
            "caseLabel": "Case",
            "controlLabel": "Control",
            "subjectColumn": "patient_id",
            "covariates": ["batch"],
            "exclusions": [{
                "enabled": true,
                "label": "Reviewed QC",
                "column": "qc_review_status",
                "values": ["Failed"]
            }],
            "minCount": 10.0,
            "minFractionSamples": 0.20
        });
        let json = run_expression_comparison_json_core(
            "w3_metadata.tsv",
            metadata,
            "w3_merged_gene_expression.tsv",
            merged,
            &config.to_string(),
        )
        .unwrap();
        assert!(json.contains("\"excluded_rows\":1"));
        assert!(json.contains("\"n_model_samples\":7"));
        assert!(json.contains("\"feature_label\":\"Gene\""));
    }
}
