use four_c_insight::cohort::{MetadataComparisonSpec, build_custom_comparison, load_metadata};
use four_c_insight::corrected::{
    CorrectedExpressionResult, CorrectedModelSpec, run_corrected_expression,
    run_corrected_expression_bytes,
};
use four_c_insight::portable_compare::{
    PortableExclusionRule, PortableExpressionComparisonConfig, run_portable_expression_comparison,
};
use four_c_insight::portable_io::PortableExpressionKind;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn comparison_and_spec() -> (
    four_c_insight::cohort::MetadataTable,
    four_c_insight::cohort::CohortComparison,
    CorrectedModelSpec,
) {
    let metadata = load_metadata(&fixture("w3_metadata.tsv")).unwrap();
    let comparison = build_custom_comparison(
        &metadata,
        &MetadataComparisonSpec {
            column: "analysis_group".to_string(),
            case_values: BTreeSet::from(["Case".to_string()]),
            control_values: BTreeSet::from(["Control".to_string()]),
            name: "Case vs Control".to_string(),
            case_label: "Case".to_string(),
            control_label: "Control".to_string(),
        },
    )
    .unwrap();
    let spec = CorrectedModelSpec {
        subject_column: Some("patient_id".to_string()),
        covariates: vec!["batch".to_string()],
        permutations: 19,
        ..CorrectedModelSpec::default()
    };
    (metadata, comparison, spec)
}

fn assert_close(a: f64, b: f64, tolerance: f64) {
    if a.is_nan() && b.is_nan() {
        return;
    }
    assert!(
        (a - b).abs() <= tolerance,
        "values differ: {a:?} vs {b:?} (tol {tolerance})"
    );
}

fn assert_expression_parity(native: &CorrectedExpressionResult, bytes: &CorrectedExpressionResult) {
    assert_eq!(native.feature_label, bytes.feature_label);
    assert_eq!(native.source_metric, bytes.source_metric);
    assert_eq!(native.model_sample_ids, bytes.model_sample_ids);
    assert_eq!(native.n_input_samples, bytes.n_input_samples);
    assert_eq!(native.n_model_samples, bytes.n_model_samples);
    assert_eq!(native.n_subjects, bytes.n_subjects);
    assert_eq!(native.tested_features, bytes.tested_features);
    assert_eq!(native.filtered_features, bytes.filtered_features);
    assert_eq!(native.tests.len(), bytes.tests.len());
    assert_eq!(native.size_factors.len(), bytes.size_factors.len());
    for (a, b) in native.size_factors.iter().zip(bytes.size_factors.iter()) {
        assert_close(*a, *b, 1e-12);
    }
    for (a, b) in native.tests.iter().zip(bytes.tests.iter()) {
        assert_eq!(a.feature, b.feature);
        assert_eq!(a.secondary_id, b.secondary_id);
        assert_eq!(a.n_samples, b.n_samples);
        assert_eq!(a.n_subjects, b.n_subjects);
        assert_eq!(a.n_case, b.n_case);
        assert_eq!(a.n_control, b.n_control);
        assert_close(a.mean_case, b.mean_case, 1e-12);
        assert_close(a.mean_control, b.mean_control, 1e-12);
        assert_close(a.beta_group, b.beta_group, 1e-12);
        assert_close(a.robust_se, b.robust_se, 1e-12);
        assert_close(a.z_value, b.z_value, 1e-12);
        assert_close(a.p_value, b.p_value, 1e-12);
        assert_close(a.q_value, b.q_value, 1e-12);
        assert_close(a.subject_effect, b.subject_effect, 1e-12);
        assert_close(a.subject_p_value, b.subject_p_value, 1e-12);
        assert_close(a.subject_q_value, b.subject_q_value, 1e-12);
    }

    match (&native.pca, &bytes.pca) {
        (Some(a), Some(b)) => {
            assert_eq!(a.x_label, b.x_label);
            assert_eq!(a.y_label, b.y_label);
            assert_eq!(a.points.len(), b.points.len());
            for (pa, pb) in a.points.iter().zip(b.points.iter()) {
                assert_eq!(pa.sample_id, pb.sample_id);
                assert_eq!(pa.group, pb.group);
                assert_eq!(pa.subject, pb.subject);
                assert_close(pa.x, pb.x, 1e-12);
                assert_close(pa.y, pb.y, 1e-12);
            }
        }
        (None, None) => {}
        _ => panic!("PCA presence differs between native and byte paths"),
    }

    match (&native.dendrogram, &bytes.dendrogram) {
        (Some(a), Some(b)) => {
            assert_eq!(a.distance_label, b.distance_label);
            assert_eq!(a.linkage, b.linkage);
            assert_eq!(a.leaves.len(), b.leaves.len());
            assert_eq!(a.segments.len(), b.segments.len());
            for (la, lb) in a.leaves.iter().zip(b.leaves.iter()) {
                assert_eq!(la.sample_id, lb.sample_id);
                assert_eq!(la.group, lb.group);
                assert_eq!(la.subject, lb.subject);
                assert_close(la.position, lb.position, 1e-12);
            }
            for (sa, sb) in a.segments.iter().zip(b.segments.iter()) {
                assert_close(sa.x1, sb.x1, 1e-12);
                assert_close(sa.y1, sb.y1, 1e-12);
                assert_close(sa.x2, sb.x2, 1e-12);
                assert_close(sa.y2, sb.y2, 1e-12);
            }
        }
        (None, None) => {}
        _ => panic!("dendrogram presence differs between native and byte paths"),
    }
}

#[test]
fn gene_corrected_expression_bytes_match_native_path() {
    let (metadata, comparison, spec) = comparison_and_spec();
    let path = fixture("w3_merged_gene_expression.tsv");
    let native = run_corrected_expression(
        &path,
        &metadata,
        &comparison,
        "Raw Count",
        "Gene",
        &spec,
        |_done, _total, _message| {},
    )
    .unwrap();
    let input = fs::read(&path).unwrap();
    let bytes = run_corrected_expression_bytes(
        "w3_merged_gene_expression.tsv",
        &input,
        &metadata,
        &comparison,
        "Raw Count",
        "Gene",
        &spec,
        |_done, _total, _message| {},
    )
    .unwrap();
    assert_expression_parity(&native, &bytes);
}

#[test]
fn transcript_corrected_expression_and_dtu_bytes_match_native_path() {
    let (metadata, comparison, spec) = comparison_and_spec();
    let path = fixture("w3_merged_transcript_expression.tsv");
    let native = run_corrected_expression(
        &path,
        &metadata,
        &comparison,
        "Total Reads",
        "Transcript",
        &spec,
        |_done, _total, _message| {},
    )
    .unwrap();
    let input = fs::read(&path).unwrap();
    let bytes = run_corrected_expression_bytes(
        "w3_merged_transcript_expression.tsv",
        &input,
        &metadata,
        &comparison,
        "Total Reads",
        "Transcript",
        &spec,
        |_done, _total, _message| {},
    )
    .unwrap();
    assert_expression_parity(&native, &bytes);
    match (&native.dtu, &bytes.dtu) {
        (Some(a), Some(b)) => {
            assert_eq!(a.genes_tested, b.genes_tested);
            assert_eq!(a.transcripts_tested, b.transcripts_tested);
            assert_eq!(a.gene_tests.len(), b.gene_tests.len());
            assert_eq!(a.transcript_tests.len(), b.transcript_tests.len());
            for (ga, gb) in a.gene_tests.iter().zip(b.gene_tests.iter()) {
                assert_eq!(ga.gene_id, gb.gene_id);
                assert_eq!(ga.n_transcripts, gb.n_transcripts);
                assert_close(ga.p_value, gb.p_value, 1e-12);
                assert_close(ga.q_value, gb.q_value, 1e-12);
                assert_close(ga.max_abs_clr_effect, gb.max_abs_clr_effect, 1e-12);
            }
        }
        (None, None) => {}
        _ => panic!("DTU presence differs between native and byte paths"),
    }
}

#[test]
fn portable_comparison_applies_metadata_exclusion_before_modeling() {
    let metadata_path = fixture("w3_metadata.tsv");
    let merged_path = fixture("w3_merged_gene_expression.tsv");
    let metadata = fs::read(&metadata_path).unwrap();
    let merged = fs::read(&merged_path).unwrap();
    let config = PortableExpressionComparisonConfig {
        kind: PortableExpressionKind::Gene,
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
        permutations: 19,
    };
    let result = run_portable_expression_comparison(
        "w3_metadata.tsv",
        &metadata,
        "w3_merged_gene_expression.tsv",
        &merged,
        &config,
        |_done, _total, _message| {},
    )
    .unwrap();

    assert_eq!(result.comparison.metadata_rows_before_exclusions, 8);
    assert_eq!(result.comparison.metadata_rows_after_exclusions, 7);
    assert_eq!(result.comparison.excluded_rows, 1);
    assert_eq!(result.comparison.case_metadata_rows, 3);
    assert_eq!(result.comparison.control_metadata_rows, 4);
    assert_eq!(result.exclusions.len(), 1);
    assert_eq!(result.exclusions[0].matching_rows, 1);
    assert_eq!(result.exclusions[0].excluded_sample_ids, vec!["C2_T1"]);
    assert_eq!(result.analysis.n_model_samples, 7);
}
