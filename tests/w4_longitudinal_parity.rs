use four_c_insight::analysis::CohortModalitySelection;
use four_c_insight::cohort::{MetadataTable, ProjectScan};
use four_c_insight::corrected::CorrectedModelSpec;
use four_c_insight::longitudinal::{LongitudinalClusteringConfig, run_longitudinal_analysis};
use four_c_insight::portable_io::PortableExpressionKind;
use four_c_insight::portable_longitudinal::{
    PortableLongitudinalConfig, run_portable_expression_longitudinal,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn metadata_table() -> MetadataTable {
    let path = fixture("w4_metadata.tsv");
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_path(&path)
        .unwrap();
    let columns = reader
        .headers()
        .unwrap()
        .iter()
        .map(str::to_string)
        .collect();
    let rows = reader
        .records()
        .map(|row| row.unwrap().iter().map(str::to_string).collect())
        .collect();
    MetadataTable {
        path,
        columns,
        rows,
        id_column: "sample_id".into(),
    }
}

fn native_config() -> LongitudinalClusteringConfig {
    LongitudinalClusteringConfig {
        observation_id_column: "sample_id".into(),
        patient_id_column: "patient_id".into(),
        time_column: Some("visit".into()),
        qc_status_column: Some("qc_review_status".into()),
        excluded_qc_values: BTreeSet::from(["Failed".into()]),
    }
}

fn portable_config(kind: PortableExpressionKind) -> PortableLongitudinalConfig {
    PortableLongitudinalConfig {
        kind,
        observation_id_column: "sample_id".into(),
        patient_id_column: "patient_id".into(),
        time_column: Some("visit".into()),
        exclusion_enabled: true,
        exclusion_column: Some("qc_review_status".into()),
        exclusion_values: vec!["Failed".into()],
        min_count: 10.0,
        min_fraction_samples: 0.20,
    }
}

fn selection(gene: bool, transcript: bool) -> CohortModalitySelection {
    CohortModalitySelection {
        gene,
        transcript,
        bcr: false,
        bcr_isotype: false,
        tcr_alpha: false,
        tcr_beta: false,
        immune: false,
        pathway: false,
    }
}

fn assert_modality_parity(
    native: &four_c_insight::longitudinal::LongitudinalModalityResult,
    portable: &four_c_insight::longitudinal::LongitudinalModalityResult,
) {
    assert_eq!(native.samples, portable.samples);
    assert_eq!(native.patients, portable.patients);
    assert_eq!(native.longitudinal_patients, portable.longitudinal_patients);
    assert_eq!(native.longitudinal_samples, portable.longitudinal_samples);
    assert_eq!(native.within_patient_pairs, portable.within_patient_pairs);
    assert_eq!(native.between_patient_pairs, portable.between_patient_pairs);
    assert!((native.identity_auc - portable.identity_auc).abs() < 1e-12);
    assert!((native.nearest_neighbor_accuracy - portable.nearest_neighbor_accuracy).abs() < 1e-12);
    assert!(
        (native.median_within_patient_distance - portable.median_within_patient_distance).abs()
            < 1e-12
    );
    assert!(
        (native.median_between_patient_distance - portable.median_between_patient_distance).abs()
            < 1e-12
    );
    assert!((native.within_between_ratio - portable.within_between_ratio).abs() < 1e-12);
    assert_eq!(native.annotations.len(), portable.annotations.len());
    assert_eq!(
        native.pairwise_distances.len(),
        portable.pairwise_distances.len()
    );
    for (left, right) in native
        .pairwise_distances
        .iter()
        .zip(portable.pairwise_distances.iter())
    {
        assert_eq!(left.sample_a, right.sample_a);
        assert_eq!(left.sample_b, right.sample_b);
        assert_eq!(left.same_patient, right.same_patient);
        assert!((left.distance - right.distance).abs() < 1e-12);
    }
    let left_tree = native.dendrogram.as_ref().unwrap();
    let right_tree = portable.dendrogram.as_ref().unwrap();
    assert_eq!(left_tree.leaves.len(), right_tree.leaves.len());
    assert_eq!(left_tree.segments.len(), right_tree.segments.len());
    for (left, right) in left_tree.leaves.iter().zip(right_tree.leaves.iter()) {
        assert_eq!(left.sample_id, right.sample_id);
        assert_eq!(left.group, right.group);
        assert_eq!(left.subject, right.subject);
        assert!((left.position - right.position).abs() < 1e-12);
    }
}

#[test]
fn gene_longitudinal_bytes_match_native_path() {
    let metadata = metadata_table();
    let merged = fixture("w3_merged_gene_expression.tsv");
    let scan = ProjectScan {
        root: fixture("."),
        files: Vec::new(),
        merged_gene_tsv: Some(merged.clone()),
        merged_transcript_tsv: None,
        metadata_candidates: Vec::new(),
    };
    let native = run_longitudinal_analysis(
        &scan,
        &metadata,
        &selection(true, false),
        &CorrectedModelSpec::default(),
        native_config(),
        |_done, _total, _message| {},
    )
    .unwrap();

    let metadata_bytes = std::fs::read(fixture("w4_metadata.tsv")).unwrap();
    let merged_bytes = std::fs::read(&merged).unwrap();
    let portable = run_portable_expression_longitudinal(
        "w4_metadata.tsv",
        &metadata_bytes,
        "w3_merged_gene_expression.tsv",
        &merged_bytes,
        &portable_config(PortableExpressionKind::Gene),
        |_done, _total, _message| {},
    )
    .unwrap();

    assert_eq!(native.qc_excluded_rows, 1);
    assert_eq!(portable.qc_excluded_rows, 1);
    assert_eq!(portable.modality.samples, 7);
    assert_eq!(portable.modality.longitudinal_patients, 3);
    assert_modality_parity(&native.modalities[0], &portable.modality);
}

#[test]
fn transcript_longitudinal_bytes_match_native_path() {
    let metadata = metadata_table();
    let merged = fixture("w3_merged_transcript_expression.tsv");
    let scan = ProjectScan {
        root: fixture("."),
        files: Vec::new(),
        merged_gene_tsv: None,
        merged_transcript_tsv: Some(merged.clone()),
        metadata_candidates: Vec::new(),
    };
    let native = run_longitudinal_analysis(
        &scan,
        &metadata,
        &selection(false, true),
        &CorrectedModelSpec::default(),
        native_config(),
        |_done, _total, _message| {},
    )
    .unwrap();

    let metadata_bytes = std::fs::read(fixture("w4_metadata.tsv")).unwrap();
    let merged_bytes = std::fs::read(&merged).unwrap();
    let portable = run_portable_expression_longitudinal(
        "w4_metadata.tsv",
        &metadata_bytes,
        "w3_merged_transcript_expression.tsv",
        &merged_bytes,
        &portable_config(PortableExpressionKind::Transcript),
        |_done, _total, _message| {},
    )
    .unwrap();

    assert_eq!(portable.modality.samples, 7);
    assert_eq!(portable.modality.longitudinal_patients, 3);
    assert_modality_parity(&native.modalities[0], &portable.modality);
}
