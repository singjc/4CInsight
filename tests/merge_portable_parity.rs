//! Compare the browser-neutral merger with the original filesystem merger.
//! Run in CI with: cargo test --locked --test merge_portable_parity
use four_c_insight::portable_merge::{
    PortableMergeKind, PortableMergeMetric, PortableMergeOrder, PortableMerger,
};
use four_c_insight::{ColumnOrder, DataKind, MergeConfig, Metric, merge_wide};
use std::fs;

fn parity(
    kind: DataKind,
    source_names: &[&str],
    metrics: &[Metric],
    portable_metrics: &[PortableMergeMetric],
    order: ColumnOrder,
    portable_order: PortableMergeOrder,
) {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for (idx, name) in source_names.iter().enumerate() {
        let new_name = format!(
            "S{}_tumor_rna_{}_expression_report.xlsx",
            idx + 1,
            if kind == DataKind::Gene {
                "gene"
            } else {
                "transcript"
            }
        );
        let path = dir.path().join(new_name);
        fs::copy(format!("tests/fixtures/{name}"), &path).unwrap();
        paths.push(path);
    }
    let native_output = dir.path().join("native.tsv");
    merge_wide(
        MergeConfig {
            files: paths.clone(),
            output: native_output.clone(),
            kind,
            metrics: metrics.to_vec(),
            column_order: order,
        },
        |_, _, _| {},
    )
    .unwrap();
    paths.sort();
    let bytes: Vec<Vec<u8>> = paths.iter().map(|path| fs::read(path).unwrap()).collect();
    let names = paths
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let merger = PortableMerger::begin(
        &bytes[0],
        if kind == DataKind::Gene {
            PortableMergeKind::Gene
        } else {
            PortableMergeKind::Transcript
        },
        portable_metrics.to_vec(),
        portable_order,
        &names,
    )
    .unwrap();
    let plan = merger.plan();
    let packed = bytes
        .iter()
        .map(|source| merger.sample(source).unwrap().0)
        .collect::<Vec<_>>();
    let mut browser_output = merger.header();
    for first in (0..plan.rows).step_by(7) {
        let count = (plan.rows - first).min(7);
        let mut buffer = Vec::new();
        for sample in &packed {
            let stride = plan.measurements * 9;
            buffer.extend_from_slice(&sample[first * stride..(first + count) * stride]);
        }
        browser_output.push_str(&merger.rows_tsv(first, count, &buffer).unwrap());
    }
    assert_eq!(
        fs::read(native_output).unwrap(),
        browser_output.as_bytes(),
        "portable merger changed native TSV output"
    );
}

#[test]
fn gene_default_both_orders() {
    for (native_order, portable_order) in [
        (
            ColumnOrder::AdjacentBySample,
            PortableMergeOrder::AdjacentBySample,
        ),
        (
            ColumnOrder::GroupedByMeasurement,
            PortableMergeOrder::GroupedByMeasurement,
        ),
    ] {
        parity(
            DataKind::Gene,
            &[
                "S1_tumor_rna_gene_expression_report.xlsx",
                "S2_tumor_rna_gene_expression_report.xlsx",
            ],
            &[Metric::RawCount, Metric::Tpm],
            &[PortableMergeMetric::RawCount, PortableMergeMetric::Tpm],
            native_order,
            portable_order,
        );
    }
}
#[test]
fn gene_custom_metrics() {
    parity(
        DataKind::Gene,
        &[
            "S1_tumor_rna_gene_expression_report.xlsx",
            "S2_tumor_rna_gene_expression_report.xlsx",
        ],
        &[Metric::RawCount],
        &[PortableMergeMetric::RawCount],
        ColumnOrder::GroupedByMeasurement,
        PortableMergeOrder::GroupedByMeasurement,
    );
}
#[test]
fn transcript_default() {
    parity(
        DataKind::Transcript,
        &["S1_tumor_rna_transcript_expression_report.xlsx"],
        &[Metric::TotalReads, Metric::Tpm],
        &[PortableMergeMetric::TotalReads, PortableMergeMetric::Tpm],
        ColumnOrder::AdjacentBySample,
        PortableMergeOrder::AdjacentBySample,
    );
}
#[test]
fn duplicate_normalized_names_are_rejected() {
    let bytes = fs::read("tests/fixtures/S1_tumor_rna_gene_expression_report.xlsx").unwrap();
    let names = vec![
        "same_tumor_rna_gene_expression_report.xlsx".to_string(),
        "same_tumor_rna_gene_expression_report.xlsx".to_string(),
    ];
    assert!(
        PortableMerger::begin(
            &bytes,
            PortableMergeKind::Gene,
            vec![PortableMergeMetric::Tpm],
            PortableMergeOrder::AdjacentBySample,
            &names
        )
        .is_err()
    );
}
#[test]
fn mixed_modality_is_rejected() {
    let bytes = fs::read("tests/fixtures/S1_tumor_rna_transcript_expression_report.xlsx").unwrap();
    let names = vec!["S1_tumor_rna_transcript_expression_report.xlsx".to_string()];
    assert!(
        PortableMerger::begin(
            &bytes,
            PortableMergeKind::Gene,
            vec![PortableMergeMetric::RawCount],
            PortableMergeOrder::AdjacentBySample,
            &names
        )
        .is_err()
    );
}
