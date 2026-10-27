use four_c_insight::analysis::CorrectedAnalysisBundle;
use four_c_insight::cohort::{
    MetadataComparisonSpec, ProjectScan, build_custom_comparison, load_metadata,
};
use four_c_insight::corrected::{
    CorrectedExpressionResult, CorrectedModelSpec, run_corrected_expression,
};
use four_c_insight::hit_structure::{
    SignificantHitPanel, SignificantHitStructureConfig, run_significant_hit_structure,
};
use four_c_insight::portable_compare::{PortableExclusionRule, PortableExpressionComparisonConfig};
use four_c_insight::portable_hit_structure::run_portable_expression_hit_structure;
use four_c_insight::portable_io::PortableExpressionKind;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn filtered_metadata() -> four_c_insight::cohort::MetadataTable {
    let mut metadata = load_metadata(&fixture("w3_metadata.tsv")).unwrap();
    let qc_idx = metadata.column_index("qc_review_status").unwrap();
    metadata.rows.retain(|row| {
        !row.get(qc_idx)
            .map(String::as_str)
            .unwrap_or_default()
            .eq_ignore_ascii_case("Failed")
    });
    metadata
}

fn native_comparison(
    metadata: &four_c_insight::cohort::MetadataTable,
) -> four_c_insight::cohort::CohortComparison {
    build_custom_comparison(
        metadata,
        &MetadataComparisonSpec {
            column: "analysis_group".to_string(),
            case_values: BTreeSet::from(["Case".to_string()]),
            control_values: BTreeSet::from(["Control".to_string()]),
            name: "Case vs Control".to_string(),
            case_label: "Case".to_string(),
            control_label: "Control".to_string(),
        },
    )
    .unwrap()
}

fn model_spec() -> CorrectedModelSpec {
    let mut spec = CorrectedModelSpec::default();
    spec.subject_column = Some("patient_id".to_string());
    spec.covariates = vec!["batch".to_string()];
    spec.min_count = 10.0;
    spec.min_fraction_samples = 0.20;
    spec.permutations = 49;
    spec
}

fn portable_config(kind: PortableExpressionKind) -> PortableExpressionComparisonConfig {
    PortableExpressionComparisonConfig {
        kind,
        metadata_id_column: "sample_id".to_string(),
        group_column: "analysis_group".to_string(),
        case_values: vec!["Case".to_string()],
        control_values: vec!["Control".to_string()],
        comparison_name: "Case vs Control".to_string(),
        case_label: "Case".to_string(),
        control_label: "Control".to_string(),
        subject_column: Some("patient_id".to_string()),
        covariates: vec!["batch".to_string()],
        exclusions: vec![PortableExclusionRule {
            enabled: true,
            label: "Reviewed QC".to_string(),
            column: "qc_review_status".to_string(),
            values: vec!["Failed".to_string()],
        }],
        min_count: 10.0,
        min_fraction_samples: 0.20,
        permutations: 49,
    }
}

fn hit_config() -> SignificantHitStructureConfig {
    SignificantHitStructureConfig {
        q_threshold: 1.0,
        min_abs_effect: 0.0,
        max_analysis_features: 50,
        max_heatmap_features: 20,
    }
}

fn bundle(
    comparison: four_c_insight::cohort::CohortComparison,
    spec: CorrectedModelSpec,
    gene: Option<CorrectedExpressionResult>,
    transcript: Option<CorrectedExpressionResult>,
) -> CorrectedAnalysisBundle {
    CorrectedAnalysisBundle {
        comparison,
        spec,
        gene,
        gene_error: None,
        transcript,
        transcript_error: None,
        bcr: None,
        bcr_error: None,
        bcr_isotype: None,
        bcr_isotype_error: None,
        tcr_alpha: None,
        tcr_alpha_error: None,
        tcr_beta: None,
        tcr_beta_error: None,
        immune: None,
        immune_error: None,
        pathway: None,
        pathway_error: None,
    }
}

fn assert_close(a: f64, b: f64) {
    if a.is_nan() && b.is_nan() {
        return;
    }
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}

fn assert_panels_equal(native: &SignificantHitPanel, portable: &SignificantHitPanel) {
    assert_eq!(native.modality, portable.modality);
    assert_eq!(native.feature_set, portable.feature_set);
    assert_eq!(native.sample_ids, portable.sample_ids);
    assert_eq!(native.groups, portable.groups);
    assert_eq!(native.subjects, portable.subjects);
    assert_eq!(
        native.significant_features_total,
        portable.significant_features_total
    );
    assert_eq!(
        native.analysis_features.len(),
        portable.analysis_features.len()
    );
    for (a, b) in native
        .analysis_features
        .iter()
        .zip(portable.analysis_features.iter())
    {
        assert_eq!(a.feature, b.feature);
        assert_eq!(a.family, b.family);
        assert_close(a.adjusted_effect, b.adjusted_effect);
        assert_close(a.q_value, b.q_value);
    }
    assert_eq!(
        native.heatmap_features.len(),
        portable.heatmap_features.len()
    );
    assert_eq!(native.heatmap_zscores.len(), portable.heatmap_zscores.len());
    for (a, b) in native
        .heatmap_zscores
        .iter()
        .zip(portable.heatmap_zscores.iter())
    {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_close(*x, *y);
        }
    }
    assert_close(native.silhouette, portable.silhouette);
    assert_close(
        native.nearest_neighbor_group_accuracy,
        portable.nearest_neighbor_group_accuracy,
    );
    assert_eq!(native.distance_label, portable.distance_label);

    let native_pca = native.pca.as_ref().unwrap();
    let portable_pca = portable.pca.as_ref().unwrap();
    assert_eq!(native_pca.points.len(), portable_pca.points.len());
    assert_eq!(native_pca.x_label, portable_pca.x_label);
    assert_eq!(native_pca.y_label, portable_pca.y_label);
    assert_close(native_pca.permanova_f, portable_pca.permanova_f);
    assert_close(native_pca.permanova_p, portable_pca.permanova_p);
    for (a, b) in native_pca.points.iter().zip(portable_pca.points.iter()) {
        assert_eq!(a.sample_id, b.sample_id);
        assert_eq!(a.group, b.group);
        assert_eq!(a.subject, b.subject);
        assert_close(a.x, b.x);
        assert_close(a.y, b.y);
    }

    let native_tree = native.dendrogram.as_ref().unwrap();
    let portable_tree = portable.dendrogram.as_ref().unwrap();
    assert_eq!(native_tree.leaves.len(), portable_tree.leaves.len());
    assert_eq!(native_tree.segments.len(), portable_tree.segments.len());
    for (a, b) in native_tree.leaves.iter().zip(portable_tree.leaves.iter()) {
        assert_eq!(a.sample_id, b.sample_id);
        assert_eq!(a.group, b.group);
        assert_eq!(a.subject, b.subject);
        assert_close(a.position, b.position);
    }
    for (a, b) in native_tree
        .segments
        .iter()
        .zip(portable_tree.segments.iter())
    {
        assert_close(a.x1, b.x1);
        assert_close(a.x2, b.x2);
        assert_close(a.y1, b.y1);
        assert_close(a.y2, b.y2);
    }
}

fn run_case(
    kind: PortableExpressionKind,
    fixture_name: &str,
    source_metric: &str,
    feature_label: &str,
) {
    let dir = tempdir().unwrap();
    let merged_name = match kind {
        PortableExpressionKind::Gene => "merged_gene_expression.tsv",
        PortableExpressionKind::Transcript => "merged_transcript_expression.tsv",
    };
    let merged_path = dir.path().join(merged_name);
    fs::copy(fixture(fixture_name), &merged_path).unwrap();

    let metadata = filtered_metadata();
    let comparison = native_comparison(&metadata);
    let spec = model_spec();
    let corrected = run_corrected_expression(
        &merged_path,
        &metadata,
        &comparison,
        source_metric,
        feature_label,
        &spec,
        |_done, _total, _message| {},
    )
    .unwrap();
    let scan = ProjectScan {
        root: dir.path().to_path_buf(),
        files: Vec::new(),
        merged_gene_tsv: matches!(kind, PortableExpressionKind::Gene)
            .then_some(merged_path.clone()),
        merged_transcript_tsv: matches!(kind, PortableExpressionKind::Transcript)
            .then_some(merged_path.clone()),
        metadata_candidates: Vec::new(),
    };
    let native_bundle = match kind {
        PortableExpressionKind::Gene => {
            bundle(comparison.clone(), spec.clone(), Some(corrected), None)
        }
        PortableExpressionKind::Transcript => {
            bundle(comparison.clone(), spec.clone(), None, Some(corrected))
        }
    };
    let native = run_significant_hit_structure(
        &scan,
        &metadata,
        &comparison,
        &spec,
        &native_bundle,
        hit_config(),
        |_done, _total, _message| {},
    )
    .unwrap();
    assert_eq!(native.errors.len(), 0);
    assert_eq!(native.panels.len(), 1);

    let metadata_bytes = fs::read(fixture("w3_metadata.tsv")).unwrap();
    let merged_bytes = fs::read(fixture(fixture_name)).unwrap();
    let portable = run_portable_expression_hit_structure(
        "w3_metadata.tsv",
        &metadata_bytes,
        fixture_name,
        &merged_bytes,
        &portable_config(kind),
        hit_config(),
        |_done, _total, _message| {},
    )
    .unwrap();
    let portable_panel = portable.panel.as_ref().unwrap();
    assert_panels_equal(&native.panels[0], portable_panel);
}

#[test]
fn gene_significant_hit_structure_matches_native_path() {
    run_case(
        PortableExpressionKind::Gene,
        "w3_merged_gene_expression.tsv",
        "Raw Count",
        "Gene",
    );
}

#[test]
fn transcript_significant_hit_structure_matches_native_path() {
    run_case(
        PortableExpressionKind::Transcript,
        "w3_merged_transcript_expression.tsv",
        "Total Reads",
        "Transcript",
    );
}
