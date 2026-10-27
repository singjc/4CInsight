//! Shared post-selection significant-hit structure calculations.
//!
//! This module contains only reader-agnostic statistical and matrix helpers so
//! the native application and browser/WASM path use the same implementation.

use crate::cohort::normalize_sample_id;
use crate::corrected::{
    AdjustedFeatureResult, CorrectedExpressionResult, DendrogramResult, OrdinationResult,
    average_linkage_dendrogram_for_samples, euclidean_distance_from_feature_rows,
    pca_from_feature_rows_for_samples, permanova_for_sample_distance,
};
use anyhow::{Result, anyhow, bail};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignificantHitStructureConfig {
    /// BH-FDR threshold used to define the post-selection feature set.
    pub q_threshold: f64,
    /// Optional absolute adjusted effect threshold applied after the q filter.
    pub min_abs_effect: f64,
    /// Safety cap for PCA/clustering. Hits are ordered by q then |effect|.
    pub max_analysis_features: usize,
    /// Number of strongest hits retained in the exported/display heatmap matrix.
    pub max_heatmap_features: usize,
}

impl Default for SignificantHitStructureConfig {
    fn default() -> Self {
        Self {
            q_threshold: 0.05,
            min_abs_effect: 0.0,
            max_analysis_features: 500,
            max_heatmap_features: 80,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SignificantHitFeature {
    pub feature: String,
    pub family: String,
    pub adjusted_effect: f64,
    pub q_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignificantHitPanel {
    pub modality: String,
    pub feature_set: String,
    pub sample_ids: Vec<String>,
    pub groups: Vec<String>,
    pub subjects: Vec<String>,
    pub significant_features_total: usize,
    pub analysis_features: Vec<SignificantHitFeature>,
    pub heatmap_features: Vec<SignificantHitFeature>,
    /// Feature-major z scores for display/export only. PCA/clustering use the
    /// values described by `distance_label`.
    pub heatmap_zscores: Vec<Vec<f64>>,
    pub pca: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
    pub silhouette: f64,
    pub nearest_neighbor_group_accuracy: f64,
    pub distance_label: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignificantHitPanelError {
    pub modality: String,
    pub feature_set: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignificantHitStructureResult {
    pub config: SignificantHitStructureConfig,
    pub panels: Vec<SignificantHitPanel>,
    pub errors: Vec<SignificantHitPanelError>,
    pub warnings: Vec<String>,
}

fn finite_or_inf(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        f64::INFINITY
    }
}

pub(crate) fn selected_features(
    tests: &[AdjustedFeatureResult],
    family: &str,
    config: &SignificantHitStructureConfig,
) -> (usize, Vec<SignificantHitFeature>) {
    let mut selected = tests
        .iter()
        .filter(|row| {
            row.q_value.is_finite()
                && row.q_value <= config.q_threshold
                && row.beta_group.is_finite()
                && row.beta_group.abs() >= config.min_abs_effect
        })
        .map(|row| SignificantHitFeature {
            feature: row.feature.clone(),
            family: if family.is_empty() {
                row.secondary_id.clone()
            } else {
                family.to_string()
            },
            adjusted_effect: row.beta_group,
            q_value: row.q_value,
        })
        .collect::<Vec<_>>();
    selected.sort_by(|a, b| {
        finite_or_inf(a.q_value)
            .total_cmp(&finite_or_inf(b.q_value))
            .then_with(|| b.adjusted_effect.abs().total_cmp(&a.adjusted_effect.abs()))
            .then_with(|| a.feature.cmp(&b.feature))
    });
    let total = selected.len();
    selected.truncate(config.max_analysis_features.max(1));
    (total, selected)
}

pub(crate) fn zscore_rows(rows: &[Vec<f64>], keep: usize) -> Vec<Vec<f64>> {
    rows.iter()
        .take(keep)
        .map(|row| {
            let finite = row
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect::<Vec<_>>();
            if finite.is_empty() {
                return vec![0.0; row.len()];
            }
            let mean = finite.iter().sum::<f64>() / finite.len() as f64;
            let var = if finite.len() > 1 {
                finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (finite.len() - 1) as f64
            } else {
                0.0
            };
            let sd = var.sqrt();
            row.iter()
                .map(|value| {
                    if value.is_finite() && sd > 1e-12 {
                        (value - mean) / sd
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

pub(crate) fn silhouette_two_groups(dist: &[Vec<f64>], groups: &[bool]) -> f64 {
    if dist.len() != groups.len() || groups.len() < 4 {
        return f64::NAN;
    }
    let mut values = Vec::new();
    for i in 0..groups.len() {
        let mut same_sum = 0.0;
        let mut same_n = 0usize;
        let mut other_sum = 0.0;
        let mut other_n = 0usize;
        for j in 0..groups.len() {
            if i == j || !dist[i][j].is_finite() {
                continue;
            }
            if groups[i] == groups[j] {
                same_sum += dist[i][j];
                same_n += 1;
            } else {
                other_sum += dist[i][j];
                other_n += 1;
            }
        }
        if same_n == 0 || other_n == 0 {
            continue;
        }
        let a = same_sum / same_n as f64;
        let b = other_sum / other_n as f64;
        let denom = a.max(b);
        if denom > 0.0 {
            values.push((b - a) / denom);
        }
    }
    if values.is_empty() {
        f64::NAN
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

pub(crate) fn nearest_neighbor_accuracy(dist: &[Vec<f64>], groups: &[bool]) -> f64 {
    if dist.len() != groups.len() || groups.len() < 2 {
        return f64::NAN;
    }
    let mut correct = 0usize;
    let mut tested = 0usize;
    for i in 0..groups.len() {
        let mut best = None::<(usize, f64)>;
        for j in 0..groups.len() {
            if i == j || !dist[i][j].is_finite() {
                continue;
            }
            if best.as_ref().is_none_or(|(_, d)| dist[i][j] < *d) {
                best = Some((j, dist[i][j]));
            }
        }
        if let Some((j, _)) = best {
            tested += 1;
            if groups[i] == groups[j] {
                correct += 1;
            }
        }
    }
    if tested == 0 {
        f64::NAN
    } else {
        correct as f64 / tested as f64
    }
}

fn stable_seed(text: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_panel(
    modality: &str,
    feature_set: &str,
    sample_ids: Vec<String>,
    group_case: Vec<bool>,
    groups: Vec<String>,
    subjects: Vec<String>,
    significant_features_total: usize,
    features: Vec<SignificantHitFeature>,
    rows: Vec<Vec<f64>>,
    distance_label: &str,
    config: &SignificantHitStructureConfig,
    permutations: usize,
    mut warnings: Vec<String>,
) -> Result<SignificantHitPanel> {
    if sample_ids.len() < 4 {
        bail!("Fewer than four modeled Case/Control samples are available");
    }
    if features.len() != rows.len() {
        bail!("Internal selected-feature/value alignment mismatch");
    }
    if rows.iter().any(|row| row.len() != sample_ids.len()) {
        bail!("Selected-feature matrix does not align to modeled samples");
    }
    if significant_features_total > features.len() {
        warnings.push(format!(
            "{} hits passed the configured threshold; PCA/clustering use the strongest {} because max_analysis_features={}",
            significant_features_total,
            features.len(),
            config.max_analysis_features
        ));
    }
    if features.len() < 2 {
        warnings.push(
            "Fewer than two significant features are available; PCA and hierarchical clustering are omitted and the hit should be inspected individually."
                .to_string(),
        );
        let keep = features.len().min(config.max_heatmap_features);
        return Ok(SignificantHitPanel {
            modality: modality.to_string(),
            feature_set: feature_set.to_string(),
            sample_ids,
            groups,
            subjects,
            significant_features_total,
            analysis_features: features.clone(),
            heatmap_features: features.into_iter().take(keep).collect(),
            heatmap_zscores: zscore_rows(&rows, keep),
            pca: None,
            dendrogram: None,
            silhouette: f64::NAN,
            nearest_neighbor_group_accuracy: f64::NAN,
            distance_label: distance_label.to_string(),
            warnings,
        });
    }
    let dist = euclidean_distance_from_feature_rows(&rows);
    let title = format!("{modality} — significant-hit PCA ({feature_set})");
    let mut pca = pca_from_feature_rows_for_samples(
        &rows,
        &sample_ids,
        &group_case,
        &subjects,
        &title,
        permutations,
    );
    let (f, p) = permanova_for_sample_distance(
        &dist,
        &group_case,
        &subjects,
        permutations,
        stable_seed(&format!("{modality}|{feature_set}")),
    );
    if let Some(ord) = pca.as_mut() {
        ord.permanova_f = f;
        ord.permanova_p = p;
        ord.permutations = permutations;
    }
    let dendrogram = average_linkage_dendrogram_for_samples(
        &dist,
        &sample_ids,
        &groups,
        &subjects,
        &format!("{modality} — significant-hit hierarchical clustering ({feature_set})"),
        distance_label,
    );
    let keep = features.len().min(config.max_heatmap_features.max(1));
    Ok(SignificantHitPanel {
        modality: modality.to_string(),
        feature_set: feature_set.to_string(),
        sample_ids,
        groups,
        subjects,
        significant_features_total,
        heatmap_features: features.iter().take(keep).cloned().collect(),
        heatmap_zscores: zscore_rows(&rows, keep),
        analysis_features: features,
        pca,
        dendrogram,
        silhouette: silhouette_two_groups(&dist, &group_case),
        nearest_neighbor_group_accuracy: nearest_neighbor_accuracy(&dist, &group_case),
        distance_label: distance_label.to_string(),
        warnings,
    })
}

pub(crate) fn expression_rows_from_reader<R, F>(
    reader: R,
    source_units: usize,
    result: &CorrectedExpressionResult,
    features: &[SignificantHitFeature],
    mut progress: F,
) -> Result<Vec<Vec<f64>>>
where
    R: Read,
    F: FnMut(usize, usize, &str),
{
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_reader(reader);
    let headers = reader.headers()?.clone();
    let suffix = format!(" {}", result.source_metric.trim());
    let mut by_sample = HashMap::<String, usize>::new();
    for (idx, header) in headers.iter().enumerate().skip(2) {
        if let Some(sample) = header.strip_suffix(&suffix) {
            by_sample.entry(normalize_sample_id(sample)).or_insert(idx);
        }
    }
    let columns = result
        .model_sample_ids
        .iter()
        .map(|sample| {
            by_sample
                .get(&normalize_sample_id(sample))
                .copied()
                .ok_or_else(|| {
                    anyhow!(
                        "Missing {} column for modeled sample '{sample}'",
                        result.source_metric
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    if result.size_factors.len() != columns.len() {
        bail!("Expression size factors do not align to modeled samples");
    }
    let wanted = features
        .iter()
        .enumerate()
        .map(|(idx, feature)| ((feature.feature.clone(), feature.family.clone()), idx))
        .collect::<HashMap<_, _>>();
    let total_units = source_units.max(1);
    let mut rows = vec![None::<Vec<f64>>; features.len()];
    let mut record = csv::StringRecord::new();
    let mut rows_seen = 0usize;
    while reader.read_record(&mut record)? {
        rows_seen += 1;
        let key = (
            record.get(0).unwrap_or_default().to_string(),
            record.get(1).unwrap_or_default().to_string(),
        );
        if let Some(&wanted_idx) = wanted
            .get(&key)
            .or_else(|| wanted.get(&(key.0.clone(), String::new())))
        {
            let values = columns
                .iter()
                .zip(result.size_factors.iter())
                .map(|(column, sf)| {
                    let count = record
                        .get(*column)
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .unwrap_or(0.0);
                    (count / sf.max(1e-12) + 0.5).log2()
                })
                .collect::<Vec<_>>();
            rows[wanted_idx] = Some(values);
            if rows.iter().all(Option::is_some) {
                break;
            }
        }
        if rows_seen % 5_000 == 0 {
            let done = usize::try_from(reader.position().byte())
                .unwrap_or(total_units)
                .min(total_units);
            progress(
                done,
                total_units,
                &format!(
                    "scanning merged matrix for selected hits ({rows_seen} rows; {}/{} hits found)",
                    rows.iter().filter(|row| row.is_some()).count(),
                    rows.len()
                ),
            );
        }
    }
    progress(
        total_units,
        total_units,
        &format!(
            "selected-hit matrix reconstruction complete ({}/{} hits found)",
            rows.iter().filter(|row| row.is_some()).count(),
            rows.len()
        ),
    );
    let mut out = Vec::with_capacity(rows.len());
    for (idx, row) in rows.into_iter().enumerate() {
        out.push(row.ok_or_else(|| {
            anyhow!(
                "Selected feature '{}' was not found in the merged matrix",
                features[idx].feature
            )
        })?);
    }
    Ok(out)
}

/// Sample indices in hierarchical leaf order when a dendrogram is available.
/// Falls back to modeled sample order.
pub fn clustered_sample_order(panel: &SignificantHitPanel) -> Vec<usize> {
    if let Some(tree) = panel.dendrogram.as_ref() {
        let order = tree
            .leaves
            .iter()
            .filter_map(|leaf| {
                panel.sample_ids.iter().position(|sample| {
                    normalize_sample_id(sample) == normalize_sample_id(&leaf.sample_id)
                })
            })
            .collect::<Vec<_>>();
        if order.len() == panel.sample_ids.len() {
            return order;
        }
    }
    (0..panel.sample_ids.len()).collect()
}
