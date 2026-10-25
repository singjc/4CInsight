//! Longitudinal patient-signature clustering across the full QC-eligible cohort.
//!
//! This analysis is deliberately unsupervised and independent of the selected
//! Case/Control comparison.  It asks whether repeated observations from the
//! same configured person/patient remain more similar to one another than to
//! observations from other people, and compares that signal across modalities.

#[cfg(not(target_arch = "wasm32"))]
use crate::analysis::CohortModalitySelection;
#[cfg(not(target_arch = "wasm32"))]
use crate::cohort::{CohortModality, ProjectScan};
use crate::cohort::{MetadataTable, normalize_sample_id};
#[cfg(not(target_arch = "wasm32"))]
use crate::corrected::{
    CorrectedModelSpec, run_unsupervised_expression_distance, run_unsupervised_immune_distance,
    run_unsupervised_isotype_distance, run_unsupervised_repertoire_distance, write_dendrogram,
};
use crate::corrected::{
    DendrogramResult, UnsupervisedDistanceMatrix, average_linkage_dendrogram_for_samples,
};
use crate::metadata::MODALITY_ALIAS_COLUMNS;
use anyhow::{Result, anyhow, bail};
#[cfg(not(target_arch = "wasm32"))]
use csv::WriterBuilder;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongitudinalClusteringConfig {
    /// Metadata column containing the biological observation/sample identifier
    /// used by filenames or merged expression columns.
    pub observation_id_column: String,
    /// Metadata column containing the stable person/patient identifier shared
    /// by repeated longitudinal observations.
    pub patient_id_column: String,
    /// Optional collection date / timepoint / visit column used for annotation.
    pub time_column: Option<String>,
    /// Optional metadata review/QC column used for pre-clustering exclusions.
    pub qc_status_column: Option<String>,
    /// Exact (case-insensitive) values in `qc_status_column` to exclude.
    pub excluded_qc_values: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalExcludedRow {
    pub observation_id: String,
    pub patient_id: String,
    pub qc_value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalSampleAnnotation {
    pub sample_id: String,
    pub observation_id: String,
    pub patient_id: String,
    pub time_value: String,
    pub qc_value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalPairDistance {
    pub sample_a: String,
    pub sample_b: String,
    pub observation_a: String,
    pub observation_b: String,
    pub patient_a: String,
    pub patient_b: String,
    pub time_a: String,
    pub time_b: String,
    pub same_patient: bool,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalModalityResult {
    pub modality: String,
    pub distance_label: String,
    pub samples: usize,
    pub patients: usize,
    pub longitudinal_patients: usize,
    pub longitudinal_samples: usize,
    pub within_patient_pairs: usize,
    pub between_patient_pairs: usize,
    /// Probability that a randomly selected same-patient pair is closer than a
    /// randomly selected different-patient pair (0.5 = no ranking signal).
    pub identity_auc: f64,
    /// Among samples belonging to patients with >=2 observations, fraction for
    /// which the nearest other sample belongs to the same patient.
    pub nearest_neighbor_accuracy: f64,
    pub median_within_patient_distance: f64,
    pub median_between_patient_distance: f64,
    pub within_between_ratio: f64,
    pub annotations: Vec<LongitudinalSampleAnnotation>,
    pub pairwise_distances: Vec<LongitudinalPairDistance>,
    pub dendrogram: Option<DendrogramResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalModalityError {
    pub modality: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongitudinalAnalysisResult {
    pub config: LongitudinalClusteringConfig,
    pub metadata_rows: usize,
    pub eligible_metadata_rows: usize,
    pub qc_excluded_rows: usize,
    pub missing_identifier_rows: usize,
    pub ambiguous_observation_keys: usize,
    pub excluded_rows: Vec<LongitudinalExcludedRow>,
    pub modalities: Vec<LongitudinalModalityResult>,
    pub modality_errors: Vec<LongitudinalModalityError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct MetadataObservation {
    observation_id: String,
    patient_id: String,
    patient_key: String,
    time_value: String,
    qc_value: String,
    row_idx: usize,
}

#[derive(Debug)]
pub(crate) struct LongitudinalMetadataIndex {
    pub(crate) eligible_ids: HashSet<String>,
    by_key: HashMap<String, MetadataObservation>,
    pub(crate) eligible_rows: usize,
    pub(crate) qc_excluded_rows: usize,
    pub(crate) missing_identifier_rows: usize,
    pub(crate) ambiguous_keys: usize,
    pub(crate) excluded_rows: Vec<LongitudinalExcludedRow>,
    pub(crate) warnings: Vec<String>,
}

pub fn suggested_observation_id_column(metadata: &MetadataTable) -> Option<String> {
    // The currently configured metadata ID column is the best default because
    // it already matches the project mapping.  Do not assume a literal field
    // name: collaborator sheets may intentionally reverse sample_id/subject_id.
    metadata
        .columns
        .contains(&metadata.id_column)
        .then(|| metadata.id_column.clone())
        .or_else(|| metadata.columns.first().cloned())
}

pub fn suggested_patient_id_column(metadata: &MetadataTable, observation: &str) -> Option<String> {
    for preferred in ["patient_id", "sample_id", "subject_id", "participant_id"] {
        if preferred != observation && metadata.columns.iter().any(|column| column == preferred) {
            return Some(preferred.to_string());
        }
    }
    metadata
        .columns
        .iter()
        .find(|column| column.as_str() != observation)
        .cloned()
}

pub fn suggested_time_column(metadata: &MetadataTable) -> Option<String> {
    let preferred = [
        "collection_date",
        "Collection_Date_(mm-dd-yy)",
        "Collection_Date",
        "timepoint",
        "visit",
        "visit_date",
    ];
    for name in preferred {
        if metadata.columns.iter().any(|column| column == name) {
            return Some(name.to_string());
        }
    }
    metadata.columns.iter().find_map(|column| {
        let lower = column.to_ascii_lowercase();
        (lower.contains("date") || lower.contains("timepoint") || lower.contains("visit"))
            .then(|| column.clone())
    })
}

pub fn suggested_qc_status_column(metadata: &MetadataTable) -> Option<String> {
    for preferred in ["qc_review_status", "qc_status", "review_status", "status"] {
        if metadata.columns.iter().any(|column| column == preferred) {
            return Some(preferred.to_string());
        }
    }
    metadata.columns.iter().find_map(|column| {
        let lower = column.to_ascii_lowercase();
        ((lower.contains("qc") || lower.contains("review")) && lower.contains("status"))
            .then(|| column.clone())
    })
}

pub fn distinct_metadata_values(metadata: &MetadataTable, column: &str) -> Vec<String> {
    let Some(idx) = metadata.column_index(column) else {
        return Vec::new();
    };
    metadata
        .rows
        .iter()
        .filter_map(|row| row.get(idx))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalized_exclusion_values(config: &LongitudinalClusteringConfig) -> HashSet<String> {
    config
        .excluded_qc_values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

pub(crate) fn build_metadata_index(
    metadata: &MetadataTable,
    config: &LongitudinalClusteringConfig,
) -> Result<LongitudinalMetadataIndex> {
    let observation_idx = metadata
        .column_index(&config.observation_id_column)
        .ok_or_else(|| {
            anyhow!(
                "Observation/sample ID column '{}' is missing",
                config.observation_id_column
            )
        })?;
    let patient_idx = metadata
        .column_index(&config.patient_id_column)
        .ok_or_else(|| {
            anyhow!(
                "Patient/person ID column '{}' is missing",
                config.patient_id_column
            )
        })?;
    if observation_idx == patient_idx {
        bail!("Observation/sample ID and patient/person ID must use different metadata columns");
    }
    let time_idx = config
        .time_column
        .as_ref()
        .and_then(|column| metadata.column_index(column));
    let qc_idx = config
        .qc_status_column
        .as_ref()
        .and_then(|column| metadata.column_index(column));
    let excluded_values = normalized_exclusion_values(config);

    let alias_indices = MODALITY_ALIAS_COLUMNS
        .iter()
        .filter_map(|name| metadata.column_index(name))
        .filter(|idx| *idx != patient_idx && *idx != observation_idx)
        .collect::<Vec<_>>();

    let mut candidates: HashMap<String, Vec<MetadataObservation>> = HashMap::new();
    let mut eligible_rows = 0usize;
    let mut qc_excluded_rows = 0usize;
    let mut missing_identifier_rows = 0usize;
    let mut excluded_rows = Vec::new();

    for (row_idx, row) in metadata.rows.iter().enumerate() {
        let observation_id = row
            .get(observation_idx)
            .map(|value| value.trim())
            .unwrap_or_default();
        let patient_id = row
            .get(patient_idx)
            .map(|value| value.trim())
            .unwrap_or_default();
        let qc_value = qc_idx
            .and_then(|idx| row.get(idx))
            .map(|value| value.trim())
            .unwrap_or_default();
        if observation_id.is_empty() || patient_id.is_empty() {
            missing_identifier_rows += 1;
            continue;
        }
        if !excluded_values.is_empty() && excluded_values.contains(&qc_value.to_ascii_lowercase()) {
            qc_excluded_rows += 1;
            excluded_rows.push(LongitudinalExcludedRow {
                observation_id: observation_id.to_string(),
                patient_id: patient_id.to_string(),
                qc_value: qc_value.to_string(),
            });
            continue;
        }
        let patient_key = normalize_sample_id(patient_id);
        if patient_key.is_empty() {
            missing_identifier_rows += 1;
            continue;
        }
        eligible_rows += 1;
        let observation = MetadataObservation {
            observation_id: observation_id.to_string(),
            patient_id: patient_id.to_string(),
            patient_key,
            time_value: time_idx
                .and_then(|idx| row.get(idx))
                .map(|value| value.trim().to_string())
                .unwrap_or_default(),
            qc_value: qc_value.to_string(),
            row_idx,
        };
        let mut keys = BTreeSet::new();
        let primary_key = normalize_sample_id(observation_id);
        if !primary_key.is_empty() {
            keys.insert(primary_key);
        }
        for idx in &alias_indices {
            let alias = row.get(*idx).map(|value| value.trim()).unwrap_or_default();
            let key = normalize_sample_id(alias);
            if !key.is_empty() {
                keys.insert(key);
            }
        }
        for key in keys {
            candidates.entry(key).or_default().push(observation.clone());
        }
    }

    let mut by_key = HashMap::new();
    let mut ambiguous_keys = 0usize;
    let mut warnings = Vec::new();
    for (key, mut rows) in candidates {
        rows.sort_by_key(|row| row.row_idx);
        rows.dedup_by_key(|row| row.row_idx);
        if rows.len() == 1 {
            by_key.insert(key, rows.remove(0));
        } else {
            ambiguous_keys += 1;
        }
    }
    if ambiguous_keys > 0 {
        warnings.push(format!(
            "{ambiguous_keys} normalized observation/alias key(s) matched more than one metadata row and were excluded from longitudinal file matching"
        ));
    }
    if missing_identifier_rows > 0 {
        warnings.push(format!(
            "{missing_identifier_rows} metadata row(s) were missing the configured observation or patient identifier and were not eligible"
        ));
    }
    let eligible_ids = by_key.keys().cloned().collect();
    Ok(LongitudinalMetadataIndex {
        eligible_ids,
        by_key,
        eligible_rows,
        qc_excluded_rows,
        missing_identifier_rows,
        ambiguous_keys,
        excluded_rows,
        warnings,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_longitudinal_analysis<F>(
    scan: &ProjectScan,
    metadata: &MetadataTable,
    selection: &CohortModalitySelection,
    corrected: &CorrectedModelSpec,
    config: LongitudinalClusteringConfig,
    mut progress: F,
) -> Result<LongitudinalAnalysisResult>
where
    F: FnMut(usize, usize, &str),
{
    let index = build_metadata_index(metadata, &config)?;
    if index.eligible_rows < 2 || index.eligible_ids.len() < 2 {
        bail!("Fewer than two metadata observations remain eligible for longitudinal clustering");
    }
    let selected_stages = [
        selection.gene,
        selection.transcript,
        selection.bcr,
        selection.bcr_isotype,
        selection.tcr_alpha,
        selection.tcr_beta,
        selection.immune,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count()
    .max(1);
    let mut stage = 0usize;
    let mut modalities = Vec::new();
    let mut modality_errors = Vec::new();

    macro_rules! run_stage {
        ($enabled:expr, $label:expr, $expr:expr) => {
            if $enabled {
                let label = $label.to_string();
                let base = stage;
                let raw = $expr;
                match raw {
                    Ok(matrix) => match summarize_modality(&label, matrix, &index, &config) {
                        Ok(result) => modalities.push(result),
                        Err(error) => modality_errors.push(LongitudinalModalityError {
                            modality: label.clone(),
                            error: format!("{error:#}"),
                        }),
                    },
                    Err(error) => modality_errors.push(LongitudinalModalityError {
                        modality: label.clone(),
                        error: format!("{error:#}"),
                    }),
                }
                stage += 1;
                progress(
                    stage,
                    selected_stages,
                    &format!("Longitudinal clustering — {label} complete"),
                );
                let _ = base;
            }
        };
    }

    run_stage!(selection.gene, "Gene expression", {
        if let Some(path) = scan.merged_gene_tsv.as_deref() {
            run_unsupervised_expression_distance(
                path,
                &index.eligible_ids,
                "Raw Count",
                "Gene",
                corrected,
                |done, total, message| {
                    let denom = total.max(1);
                    let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                    progress(
                        scaled,
                        selected_stages * 1000,
                        &format!("Longitudinal — {message}"),
                    );
                },
            )
        } else {
            Err(anyhow!("No merged gene-expression TSV detected"))
        }
    });

    run_stage!(selection.transcript, "Transcript / isoform expression", {
        if let Some(path) = scan.merged_transcript_tsv.as_deref() {
            run_unsupervised_expression_distance(
                path,
                &index.eligible_ids,
                "Total Reads",
                "Transcript",
                corrected,
                |done, total, message| {
                    let denom = total.max(1);
                    let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                    progress(
                        scaled,
                        selected_stages * 1000,
                        &format!("Longitudinal — {message}"),
                    );
                },
            )
        } else {
            Err(anyhow!("No merged transcript-expression TSV detected"))
        }
    });

    run_stage!(
        selection.bcr,
        "BCR",
        run_unsupervised_repertoire_distance(
            &scan.files,
            &index.eligible_ids,
            CohortModality::Bcr,
            corrected.ambiguous_policy,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    scaled,
                    selected_stages * 1000,
                    &format!("Longitudinal — {message}"),
                );
            },
        )
    );

    run_stage!(
        selection.bcr_isotype,
        "BCR isotype",
        run_unsupervised_isotype_distance(
            &scan.files,
            &index.eligible_ids,
            corrected,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    scaled,
                    selected_stages * 1000,
                    &format!("Longitudinal — {message}"),
                );
            },
        )
    );

    run_stage!(
        selection.tcr_alpha,
        "TCR alpha",
        run_unsupervised_repertoire_distance(
            &scan.files,
            &index.eligible_ids,
            CohortModality::TcrAlpha,
            corrected.ambiguous_policy,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    scaled,
                    selected_stages * 1000,
                    &format!("Longitudinal — {message}"),
                );
            },
        )
    );

    run_stage!(
        selection.tcr_beta,
        "TCR beta",
        run_unsupervised_repertoire_distance(
            &scan.files,
            &index.eligible_ids,
            CohortModality::TcrBeta,
            corrected.ambiguous_policy,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    scaled,
                    selected_stages * 1000,
                    &format!("Longitudinal — {message}"),
                );
            },
        )
    );

    run_stage!(
        selection.immune,
        "Immune infiltration",
        run_unsupervised_immune_distance(
            &scan.files,
            &index.eligible_ids,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = stage * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    scaled,
                    selected_stages * 1000,
                    &format!("Longitudinal — {message}"),
                );
            },
        )
    );

    let mut warnings = index.warnings;
    if modalities.is_empty() {
        warnings.push(
            "No selected modality produced a usable longitudinal distance matrix".to_string(),
        );
    }
    progress(
        selected_stages,
        selected_stages,
        "Longitudinal patient-signature analysis complete",
    );
    Ok(LongitudinalAnalysisResult {
        config,
        metadata_rows: metadata.rows.len(),
        eligible_metadata_rows: index.eligible_rows,
        qc_excluded_rows: index.qc_excluded_rows,
        missing_identifier_rows: index.missing_identifier_rows,
        ambiguous_observation_keys: index.ambiguous_keys,
        excluded_rows: index.excluded_rows,
        modalities,
        modality_errors,
        warnings,
    })
}

pub(crate) fn summarize_modality(
    modality: &str,
    raw: UnsupervisedDistanceMatrix,
    metadata_index: &LongitudinalMetadataIndex,
    config: &LongitudinalClusteringConfig,
) -> Result<LongitudinalModalityResult> {
    let metadata_by_key = &metadata_index.by_key;
    if raw.sample_ids.len() != raw.distances.len()
        || raw
            .distances
            .iter()
            .any(|row| row.len() != raw.sample_ids.len())
    {
        bail!("{modality} distance matrix/sample alignment is invalid");
    }
    let mut warnings = raw.warnings;
    let mut retained = Vec::<(usize, MetadataObservation)>::new();
    let mut seen_rows = HashSet::new();
    for (idx, sample_id) in raw.sample_ids.iter().enumerate() {
        let key = normalize_sample_id(sample_id);
        let Some(annotation) = metadata_by_key.get(&key).cloned() else {
            warnings.push(format!(
                "{sample_id}: distance row could not be mapped back to one unambiguous eligible metadata row"
            ));
            continue;
        };
        if !seen_rows.insert(annotation.row_idx) {
            warnings.push(format!(
                "{sample_id}: a second modality sample mapped to metadata observation '{}'; only the first was retained",
                annotation.observation_id
            ));
            continue;
        }
        retained.push((idx, annotation));
    }
    if retained.len() < 2 {
        bail!("{modality} has fewer than two uniquely mapped eligible samples");
    }
    let n = retained.len();
    let sample_ids = retained
        .iter()
        .map(|(idx, _)| raw.sample_ids[*idx].clone())
        .collect::<Vec<_>>();
    let annotations = retained
        .iter()
        .map(|(idx, meta)| LongitudinalSampleAnnotation {
            sample_id: raw.sample_ids[*idx].clone(),
            observation_id: meta.observation_id.clone(),
            patient_id: meta.patient_id.clone(),
            time_value: meta.time_value.clone(),
            qc_value: meta.qc_value.clone(),
        })
        .collect::<Vec<_>>();
    let patients = retained
        .iter()
        .map(|(_, meta)| meta.patient_key.clone())
        .collect::<Vec<_>>();
    let patient_display = retained
        .iter()
        .map(|(_, meta)| meta.patient_id.clone())
        .collect::<Vec<_>>();
    let groups = retained
        .iter()
        .map(|(_, meta)| {
            if meta.time_value.is_empty() {
                "Eligible".to_string()
            } else {
                meta.time_value.clone()
            }
        })
        .collect::<Vec<_>>();
    let mut distances = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let value = raw.distances[retained[i].0][retained[j].0];
            if !value.is_finite() {
                bail!("{modality} contains a non-finite pairwise distance");
            }
            distances[i][j] = value;
            distances[j][i] = value;
        }
    }

    let patient_counts =
        patients
            .iter()
            .fold(HashMap::<String, usize>::new(), |mut acc, patient| {
                *acc.entry(patient.clone()).or_default() += 1;
                acc
            });
    let longitudinal_patients = patient_counts.values().filter(|count| **count >= 2).count();
    let longitudinal_samples = patients
        .iter()
        .filter(|patient| patient_counts.get(*patient).copied().unwrap_or(0) >= 2)
        .count();
    let mut within = Vec::new();
    let mut between = Vec::new();
    let mut pairwise_distances = Vec::with_capacity(n.saturating_mul(n.saturating_sub(1)) / 2);
    for i in 0..n {
        for j in (i + 1)..n {
            let same_patient = patients[i] == patients[j];
            let distance = distances[i][j];
            if same_patient {
                within.push(distance);
            } else {
                between.push(distance);
            }
            pairwise_distances.push(LongitudinalPairDistance {
                sample_a: sample_ids[i].clone(),
                sample_b: sample_ids[j].clone(),
                observation_a: annotations[i].observation_id.clone(),
                observation_b: annotations[j].observation_id.clone(),
                patient_a: annotations[i].patient_id.clone(),
                patient_b: annotations[j].patient_id.clone(),
                time_a: annotations[i].time_value.clone(),
                time_b: annotations[j].time_value.clone(),
                same_patient,
                distance,
            });
        }
    }
    let median_within = median(&within);
    let median_between = median(&between);
    let identity_auc = patient_identity_auc(&distances, &patients);
    let nearest_neighbor_accuracy =
        nearest_neighbor_accuracy(&distances, &patients, &patient_counts);
    let ratio = if median_within.is_finite() && median_between.is_finite() && median_between > 0.0 {
        median_within / median_between
    } else {
        f64::NAN
    };
    if longitudinal_patients == 0 {
        warnings.push(format!(
            "No patient/person in {modality} has two or more eligible observations; the dendrogram is available but patient-stability metrics are undefined"
        ));
    } else if within.is_empty() {
        warnings.push(format!(
            "No same-patient pairwise distances were available for {modality}"
        ));
    }
    let title = format!("{modality} — longitudinal patient-signature hierarchical clustering");
    let dendrogram = average_linkage_dendrogram_for_samples(
        &distances,
        &sample_ids,
        &groups,
        &patient_display,
        &title,
        &raw.distance_label,
    );
    if config.time_column.is_none() {
        warnings.push("No collection/time column was selected; leaves can still be assessed by patient identity but longitudinal visit/date labels are unavailable".to_string());
    }
    Ok(LongitudinalModalityResult {
        modality: modality.to_string(),
        distance_label: raw.distance_label,
        samples: n,
        patients: patient_counts.len(),
        longitudinal_patients,
        longitudinal_samples,
        within_patient_pairs: within.len(),
        between_patient_pairs: between.len(),
        identity_auc,
        nearest_neighbor_accuracy,
        median_within_patient_distance: median_within,
        median_between_patient_distance: median_between,
        within_between_ratio: ratio,
        annotations,
        pairwise_distances,
        dendrogram,
        warnings,
    })
}

fn median(values: &[f64]) -> f64 {
    let mut values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) * 0.5
    } else {
        values[mid]
    }
}

fn patient_identity_auc(distances: &[Vec<f64>], patients: &[String]) -> f64 {
    let mut ranked = Vec::<(f64, bool)>::new();
    let mut positives = 0usize;
    let mut negatives = 0usize;
    for i in 0..patients.len() {
        for j in (i + 1)..patients.len() {
            let same = patients[i] == patients[j];
            if same {
                positives += 1;
            } else {
                negatives += 1;
            }
            ranked.push((distances[i][j], same));
        }
    }
    if positives == 0 || negatives == 0 {
        return f64::NAN;
    }
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut positive_rank_sum = 0.0;
    let mut index = 0usize;
    while index < ranked.len() {
        let mut end = index + 1;
        while end < ranked.len() && ranked[end].0.total_cmp(&ranked[index].0).is_eq() {
            end += 1;
        }
        let average_rank = ((index + 1 + end) as f64) * 0.5;
        let positive_in_tie = ranked[index..end].iter().filter(|(_, same)| *same).count();
        positive_rank_sum += average_rank * positive_in_tie as f64;
        index = end;
    }
    let u_distance_high = positive_rank_sum - (positives * (positives + 1) / 2) as f64;
    let auc_high_distance = u_distance_high / (positives as f64 * negatives as f64);
    (1.0 - auc_high_distance).clamp(0.0, 1.0)
}

fn nearest_neighbor_accuracy(
    distances: &[Vec<f64>],
    patients: &[String],
    counts: &HashMap<String, usize>,
) -> f64 {
    let mut eligible = 0usize;
    let mut correct_credit = 0.0;
    for i in 0..patients.len() {
        if counts.get(&patients[i]).copied().unwrap_or(0) < 2 {
            continue;
        }
        let best_distance = (0..patients.len())
            .filter(|j| *j != i)
            .map(|j| distances[i][j])
            .filter(|distance| distance.is_finite())
            .min_by(f64::total_cmp);
        let Some(best_distance) = best_distance else {
            continue;
        };
        eligible += 1;
        let tolerance = 1e-12 * best_distance.abs().max(1.0);
        let mut tied = 0usize;
        let mut same_patient_tied = 0usize;
        for j in 0..patients.len() {
            if i == j {
                continue;
            }
            let distance = distances[i][j];
            if distance.is_finite() && (distance - best_distance).abs() <= tolerance {
                tied += 1;
                if patients[i] == patients[j] {
                    same_patient_tied += 1;
                }
            }
        }
        if tied > 0 {
            // Fractional credit makes the metric deterministic when several
            // samples are exactly tied for nearest distance.
            correct_credit += same_patient_tied as f64 / tied as f64;
        }
    }
    if eligible == 0 {
        f64::NAN
    } else {
        correct_credit / eligible as f64
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_longitudinal_analysis_tables(
    parent: &Path,
    result: &LongitudinalAnalysisResult,
) -> Result<()> {
    fs::create_dir_all(parent)?;
    let mut summary = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(parent.join("longitudinal_patient_signature_summary.tsv"))?;
    summary.write_record([
        "modality",
        "samples",
        "patients",
        "longitudinal_patients",
        "longitudinal_samples",
        "within_patient_pairs",
        "between_patient_pairs",
        "patient_identity_auc",
        "nearest_neighbor_accuracy",
        "median_within_patient_distance",
        "median_between_patient_distance",
        "within_between_ratio",
        "distance_definition",
    ])?;
    for modality in &result.modalities {
        summary.serialize((
            &modality.modality,
            modality.samples,
            modality.patients,
            modality.longitudinal_patients,
            modality.longitudinal_samples,
            modality.within_patient_pairs,
            modality.between_patient_pairs,
            modality.identity_auc,
            modality.nearest_neighbor_accuracy,
            modality.median_within_patient_distance,
            modality.median_between_patient_distance,
            modality.within_between_ratio,
            &modality.distance_label,
        ))?;
    }
    summary.flush()?;

    let mut excluded = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(parent.join("longitudinal_metadata_exclusions.tsv"))?;
    excluded.write_record(["observation_id", "patient_id", "qc_value"])?;
    for row in &result.excluded_rows {
        excluded.serialize((&row.observation_id, &row.patient_id, &row.qc_value))?;
    }
    excluded.flush()?;

    for modality in &result.modalities {
        let safe: String = modality
            .modality
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let mut annotations = WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(parent.join(format!("{safe}_sample_annotations.tsv")))?;
        annotations.write_record([
            "data_sample_id",
            "metadata_observation_id",
            "patient_id",
            "time_or_collection",
            "qc_value",
        ])?;
        for row in &modality.annotations {
            annotations.serialize((
                &row.sample_id,
                &row.observation_id,
                &row.patient_id,
                &row.time_value,
                &row.qc_value,
            ))?;
        }
        annotations.flush()?;

        let mut pairs = WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(parent.join(format!("{safe}_pairwise_distances.tsv")))?;
        pairs.write_record([
            "sample_a",
            "sample_b",
            "observation_a",
            "observation_b",
            "patient_a",
            "patient_b",
            "time_a",
            "time_b",
            "same_patient",
            "distance",
        ])?;
        for pair in &modality.pairwise_distances {
            pairs.serialize((
                &pair.sample_a,
                &pair.sample_b,
                &pair.observation_a,
                &pair.observation_b,
                &pair.patient_a,
                &pair.patient_b,
                &pair.time_a,
                &pair.time_b,
                pair.same_patient,
                pair.distance,
            ))?;
        }
        pairs.flush()?;
        if let Some(tree) = modality.dendrogram.as_ref() {
            write_dendrogram(
                &parent.join(format!("{safe}_patient_dendrogram_leaf_order.tsv")),
                tree,
            )?;
        }
        if !modality.warnings.is_empty() {
            fs::write(
                parent.join(format!("{safe}_warnings.txt")),
                modality.warnings.join("\n"),
            )?;
        }
    }
    if !result.modality_errors.is_empty() {
        let mut errors = WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(parent.join("longitudinal_modality_errors.tsv"))?;
        errors.write_record(["modality", "error"])?;
        for error in &result.modality_errors {
            errors.serialize((&error.modality, &error.error))?;
        }
        errors.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_auc_is_one_when_every_same_patient_pair_is_closer() {
        let distances = vec![
            vec![0.0, 0.1, 1.0, 1.1],
            vec![0.1, 0.0, 1.2, 1.0],
            vec![1.0, 1.2, 0.0, 0.2],
            vec![1.1, 1.0, 0.2, 0.0],
        ];
        let patients = vec!["A".into(), "A".into(), "B".into(), "B".into()];
        assert!((patient_identity_auc(&distances, &patients) - 1.0).abs() < 1e-12);
        let counts = HashMap::from([("A".to_string(), 2), ("B".to_string(), 2)]);
        assert!((nearest_neighbor_accuracy(&distances, &patients, &counts) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn collaborator_style_reversed_id_names_are_supported_by_role_selection() {
        let metadata = MetadataTable {
            path: "meta.tsv".into(),
            columns: vec![
                "subject_id".into(),
                "sample_id".into(),
                "Collection_Date_(mm-dd-yy)".into(),
                "qc_review_status".into(),
            ],
            rows: vec![
                vec!["OBS1".into(), "PAT1".into(), "01-01-2020".into(), "".into()],
                vec![
                    "OBS2".into(),
                    "PAT1".into(),
                    "01-01-2021".into(),
                    "Failed".into(),
                ],
            ],
            id_column: "subject_id".into(),
        };
        assert_eq!(
            suggested_observation_id_column(&metadata).as_deref(),
            Some("subject_id")
        );
        assert_eq!(
            suggested_patient_id_column(&metadata, "subject_id").as_deref(),
            Some("sample_id")
        );
        assert_eq!(
            suggested_time_column(&metadata).as_deref(),
            Some("Collection_Date_(mm-dd-yy)")
        );
        assert_eq!(
            suggested_qc_status_column(&metadata).as_deref(),
            Some("qc_review_status")
        );
        assert!(
            distinct_metadata_values(&metadata, "qc_review_status")
                .iter()
                .any(|value| value == "Failed")
        );
    }

    #[test]
    fn failed_rows_are_excluded_using_configured_columns_not_fixed_names() {
        let metadata = MetadataTable {
            path: "meta.tsv".into(),
            columns: vec![
                "subject_id".into(),
                "sample_id".into(),
                "review".into(),
                "visit".into(),
            ],
            rows: vec![
                vec!["OBS1".into(), "PAT1".into(), "".into(), "T0".into()],
                vec!["OBS2".into(), "PAT1".into(), "Failed".into(), "T1".into()],
                vec!["OBS3".into(), "PAT2".into(), "Include".into(), "T0".into()],
            ],
            id_column: "subject_id".into(),
        };
        let config = LongitudinalClusteringConfig {
            observation_id_column: "subject_id".into(),
            patient_id_column: "sample_id".into(),
            time_column: Some("visit".into()),
            qc_status_column: Some("review".into()),
            excluded_qc_values: BTreeSet::from(["Failed".into()]),
        };
        let index = build_metadata_index(&metadata, &config).unwrap();
        assert_eq!(index.eligible_rows, 2);
        assert_eq!(index.qc_excluded_rows, 1);
        assert!(index.eligible_ids.contains(&normalize_sample_id("OBS1")));
        assert!(!index.eligible_ids.contains(&normalize_sample_id("OBS2")));
    }
}
