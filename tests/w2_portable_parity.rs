use four_c_insight::{
    DataKind, Metric,
    cohort::load_metadata,
    portable_io::{
        PortableExpressionKind, PortableExpressionMetric, parse_metadata_bytes,
        summarize_expression_bytes,
    },
    sample_name, summarize_sample_distributions,
};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn assert_option_close(left: Option<f64>, right: Option<f64>) {
    match (left, right) {
        (None, None) => {}
        (Some(left), Some(right)) => assert!((left - right).abs() < 1e-12, "{left} != {right}"),
        pair => panic!("optional values differ: {pair:?}"),
    }
}

#[test]
fn metadata_tsv_bytes_match_native_loader() {
    let path = fixture("w2_metadata.tsv");
    let native = load_metadata(&path).unwrap();
    let bytes = fs::read(&path).unwrap();
    let portable = parse_metadata_bytes("w2_metadata.tsv", &bytes).unwrap();

    assert_eq!(native.columns, portable.columns);
    assert_eq!(native.rows, portable.rows);
    assert_eq!(native.id_column, portable.id_column);
}

#[test]
fn metadata_xlsx_bytes_match_native_loader() {
    let path = fixture("w2_metadata.xlsx");
    let native = load_metadata(&path).unwrap();
    let bytes = fs::read(&path).unwrap();
    let portable = parse_metadata_bytes("w2_metadata.xlsx", &bytes).unwrap();

    assert_eq!(native.columns, portable.columns);
    assert_eq!(native.rows, portable.rows);
    assert_eq!(native.id_column, portable.id_column);
}

#[test]
fn gene_tpm_distribution_matches_native_path_reader() {
    let path = fixture("S1_tumor_rna_gene_expression_report.xlsx");
    let native = summarize_sample_distributions(
        std::slice::from_ref(&path),
        DataKind::Gene,
        Metric::Tpm,
        false,
        |_, _, _| {},
    )
    .unwrap()
    .remove(0);
    let bytes = fs::read(&path).unwrap();
    let portable = summarize_expression_bytes(
        path.file_name().unwrap().to_str().unwrap(),
        &bytes,
        PortableExpressionKind::Gene,
        PortableExpressionMetric::Tpm,
        false,
    )
    .unwrap();

    assert_eq!(native.sample, portable.sample);
    assert_eq!(sample_name(&path), portable.sample);
    assert_eq!(native.n_total, portable.n_total);
    assert_eq!(native.n_values, portable.n_values);
    assert_eq!(native.n_missing, portable.n_missing);
    assert_eq!(native.n_zero, portable.n_zero);
    assert_option_close(native.mean, portable.mean);
    assert_option_close(native.lower_whisker, portable.lower_whisker);
    assert_option_close(native.q1, portable.q1);
    assert_option_close(native.median, portable.median);
    assert_option_close(native.q3, portable.q3);
    assert_option_close(native.upper_whisker, portable.upper_whisker);
}

#[test]
fn transcript_total_reads_distribution_matches_native_path_reader() {
    let path = fixture("S1_tumor_rna_transcript_expression_report.xlsx");
    let native = summarize_sample_distributions(
        std::slice::from_ref(&path),
        DataKind::Transcript,
        Metric::TotalReads,
        true,
        |_, _, _| {},
    )
    .unwrap()
    .remove(0);
    let bytes = fs::read(&path).unwrap();
    let portable = summarize_expression_bytes(
        path.file_name().unwrap().to_str().unwrap(),
        &bytes,
        PortableExpressionKind::Transcript,
        PortableExpressionMetric::TotalReads,
        true,
    )
    .unwrap();

    assert_eq!(native.sample, portable.sample);
    assert_eq!(native.n_total, portable.n_total);
    assert_eq!(native.n_values, portable.n_values);
    assert_eq!(native.n_missing, portable.n_missing);
    assert_eq!(native.n_zero, portable.n_zero);
    assert_option_close(native.mean, portable.mean);
    assert_option_close(native.lower_whisker, portable.lower_whisker);
    assert_option_close(native.q1, portable.q1);
    assert_option_close(native.median, portable.median);
    assert_option_close(native.q3, portable.q3);
    assert_option_close(native.upper_whisker, portable.upper_whisker);
}
