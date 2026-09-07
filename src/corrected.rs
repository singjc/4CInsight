//! Dependency-light corrected / standard cohort analyses.
//!
//! This module intentionally separates the original exploratory screens from a
//! second, more rigorous analysis layer.  It implements common building blocks
//! directly in Rust so the desktop application stays self contained:
//!
//! - count normalization by DESeq2-style positive-count median ratios;
//! - covariate-adjusted NB2 GLMs for RNA counts plus robust repeated-measure inference;
//! - subject-collapsed sensitivity tests for repeated sampling;
//! - matched-depth repertoire rarefaction;
//! - Morisita-Horn and Jaccard repertoire distances, PCoA, and PERMANOVA;
//! - subject-aware private / longitudinally-shared / public clonotype fractions;
//! - CLR/Aitchison analysis for V/D/J/VJ and isotype compositions;
//! - adjusted ssGSEA immune-infiltrate testing and PCA.
//!
//! The expression model is deliberately described as a dependency-light
//! native unshrunk-dispersion NB2 model, not as DESeq2/edgeR/limma-voom.  Those packages
//! include empirical-Bayes/dispersion machinery that is not reproduced here.

use crate::cohort::{
    CohortComparison, CohortModality, DetectedCohortFile, MetadataTable, normalize_sample_id,
};
use crate::metadata::row_matching_ids;
use anyhow::{Context, Result, anyhow, bail};
use calamine::{Data, Reader, Xlsx, open_workbook};
use csv::ReaderBuilder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguousCallPolicy {
    Exclude,
    Split,
    First,
    Family,
}

impl AmbiguousCallPolicy {
    pub const ALL: [Self; 4] = [Self::Exclude, Self::Split, Self::First, Self::Family];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Exclude => "Exclude ambiguous calls",
            Self::Split => "Split weight across tied hits",
            Self::First => "Use first reported hit",
            Self::Family => "Collapse to gene family",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Exclude => {
                "Rows with comma-separated tied V/D/J calls are omitted from that composition. This is the conservative default."
            }
            Self::Split => "A clone's weight is divided equally among all reported tied hits.",
            Self::First => {
                "Uses only the first reported hit, matching the legacy Python/R workflow but potentially privileging an arbitrary tie ordering."
            }
            Self::Family => {
                "Ambiguous gene calls are collapsed to their gene family (for example TRBV6-2/TRBV6-3 -> TRBV6) before counting."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CorrectedModelSpec {
    pub subject_column: Option<String>,
    pub covariates: Vec<String>,
    pub min_count: f64,
    pub min_fraction_samples: f64,
    pub clr_pseudocount: f64,
    pub composition_min_mean: f64,
    pub composition_min_prevalence: f64,
    pub permutations: usize,
    pub rarefaction_reps: usize,
    pub rarefaction_cap: usize,
    pub ambiguous_policy: AmbiguousCallPolicy,
    pub exclude_undetermined_isotype: bool,
}

impl Default for CorrectedModelSpec {
    fn default() -> Self {
        Self {
            subject_column: None,
            covariates: Vec::new(),
            min_count: 10.0,
            min_fraction_samples: 0.20,
            clr_pseudocount: 1e-6,
            composition_min_mean: 0.002,
            composition_min_prevalence: 0.05,
            permutations: 499,
            rarefaction_reps: 3,
            rarefaction_cap: 50_000,
            ambiguous_policy: AmbiguousCallPolicy::Exclude,
            exclude_undetermined_isotype: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdjustedFeatureResult {
    pub feature: String,
    pub secondary_id: String,
    pub n_samples: usize,
    pub n_subjects: usize,
    pub n_case: usize,
    pub n_control: usize,
    pub mean_case: f64,
    pub mean_control: f64,
    pub beta_group: f64,
    pub robust_se: f64,
    pub z_value: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub subject_effect: f64,
    pub subject_p_value: f64,
    pub subject_q_value: f64,
    pub method: String,
}

#[derive(Debug, Clone)]
pub struct OrdinationPoint {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct OrdinationResult {
    pub name: String,
    pub x_label: String,
    pub y_label: String,
    pub points: Vec<OrdinationPoint>,
    pub permanova_f: f64,
    pub permanova_p: f64,
    pub permutations: usize,
}

#[derive(Debug, Clone)]
pub struct DendrogramLeaf {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub position: f64,
}

#[derive(Debug, Clone)]
pub struct DendrogramSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone)]
pub struct DendrogramResult {
    pub name: String,
    pub distance_label: String,
    pub linkage: String,
    pub leaves: Vec<DendrogramLeaf>,
    pub segments: Vec<DendrogramSegment>,
}

#[derive(Debug, Clone)]
pub(crate) struct UnsupervisedDistanceMatrix {
    pub sample_ids: Vec<String>,
    pub distances: Vec<Vec<f64>>,
    pub distance_label: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DtuGeneResult {
    pub gene_id: String,
    pub n_transcripts: usize,
    pub p_value: f64,
    pub q_value: f64,
    pub max_abs_clr_effect: f64,
}

#[derive(Debug, Clone)]
pub struct CorrectedDtuResult {
    pub genes_tested: usize,
    pub transcripts_tested: usize,
    pub transcript_tests: Vec<AdjustedFeatureResult>,
    pub gene_tests: Vec<DtuGeneResult>,
    pub method_description: String,
}

#[derive(Debug, Clone)]
pub struct CorrectedExpressionResult {
    pub input: PathBuf,
    pub feature_label: String,
    pub source_metric: String,
    pub normalization: String,
    pub model_description: String,
    /// Canonical model sample order used for count normalization/model fitting.
    /// Kept so sensitivity refits with the exact same sample population can
    /// reuse the already-computed median-ratio size factors without another
    /// full expression-matrix pass.
    pub model_sample_ids: Vec<String>,
    /// Positive-count median-ratio size factors aligned to `model_sample_ids`.
    pub size_factors: Vec<f64>,
    pub n_input_samples: usize,
    pub n_model_samples: usize,
    pub n_subjects: usize,
    pub tested_features: usize,
    pub filtered_features: usize,
    pub tests: Vec<AdjustedFeatureResult>,
    pub pca: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
    pub dtu: Option<CorrectedDtuResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PathwayOraResult {
    pub pathway: String,
    pub description: String,
    pub measured_set_size: usize,
    pub overlap: usize,
    pub odds_ratio: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub overlap_genes: String,
}

#[derive(Debug, Clone)]
pub struct CorrectedPathwayResult {
    pub gmt_path: PathBuf,
    pub universe_size: usize,
    pub foreground_size: usize,
    pub q_threshold: f64,
    pub effect_threshold: f64,
    pub min_set_size: usize,
    pub max_set_size: usize,
    pub tests: Vec<PathwayOraResult>,
}

#[derive(Debug, Clone)]
pub struct CorrectedRepertoireSample {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub total_clone_count: f64,
    pub observed_richness: f64,
    pub shannon: f64,
    pub clonality: f64,
    pub rarefied_richness: f64,
    pub rarefied_shannon: f64,
    pub top_clone_fraction: f64,
    pub weighted_mean_cdr3_aa_length: f64,
    pub private_fraction: f64,
    pub longitudinal_shared_fraction: f64,
    pub public_fraction: f64,
    pub convergent_read_fraction: f64,
}

#[derive(Debug, Clone)]
pub struct CorrectedCompositionResult {
    pub family: String,
    pub retained_features: usize,
    /// Exact CLR denominator/basis used when the composition was modeled.
    pub retained_feature_names: Vec<String>,
    pub tests: Vec<AdjustedFeatureResult>,
    pub ordination: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
}

#[derive(Debug, Clone)]
pub struct CorrectedRepertoireResult {
    pub modality: String,
    pub files_used: usize,
    pub matched_depth: usize,
    pub rarefaction_reps: usize,
    pub ambiguous_policy: AmbiguousCallPolicy,
    pub sample_summaries: Vec<CorrectedRepertoireSample>,
    pub metric_tests: Vec<AdjustedFeatureResult>,
    pub compositions: Vec<CorrectedCompositionResult>,
    pub morisita_horn: Option<OrdinationResult>,
    pub jaccard: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CorrectedIsotypeResult {
    pub files_used: usize,
    pub determined_only: bool,
    /// Exact CLR denominator/basis used for the isotype composition.
    pub retained_feature_names: Vec<String>,
    pub tests: Vec<AdjustedFeatureResult>,
    pub ordination: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CorrectedImmuneResult {
    pub files_used: usize,
    pub tests: Vec<AdjustedFeatureResult>,
    pub pca: Option<OrdinationResult>,
    pub dendrogram: Option<DendrogramResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct DesignMatrix {
    sample_ids: Vec<String>,
    group_case: Vec<bool>,
    subjects: Vec<String>,
    x: Vec<Vec<f64>>,
    column_names: Vec<String>,
    group_col: usize,
    dropped_missing_covariates: usize,
}

impl DesignMatrix {
    fn n_subjects(&self) -> usize {
        self.subjects.iter().cloned().collect::<HashSet<_>>().len()
    }
}

#[derive(Debug, Clone)]
struct SelectedMetaRow {
    sample_id: String,
    row_idx: usize,
    is_case: bool,
    subject: String,
}

fn build_design(
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    sample_ids: &[String],
    spec: &CorrectedModelSpec,
) -> Result<DesignMatrix> {
    if metadata.column_index(&metadata.id_column).is_none() {
        bail!("Metadata ID column '{}' is missing", metadata.id_column);
    }
    let mut row_by_id: HashMap<String, Vec<usize>> = HashMap::new();
    for row_idx in 0..metadata.rows.len() {
        for id in row_matching_ids(metadata, row_idx) {
            let entry = row_by_id.entry(id).or_default();
            if !entry.contains(&row_idx) {
                entry.push(row_idx);
            }
        }
    }

    let subject_idx = spec
        .subject_column
        .as_ref()
        .and_then(|name| metadata.column_index(name));
    let mut selected = Vec::new();
    for sample in sample_ids {
        let key = normalize_sample_id(sample);
        let Some(rows) = row_by_id.get(&key) else {
            continue;
        };
        if rows.len() != 1 {
            continue;
        }
        let is_case = comparison.is_case_id(&key);
        let is_control = comparison.is_control_id(&key);
        if !is_case && !is_control {
            continue;
        }
        let row_idx = rows[0];
        let subject = subject_idx
            .and_then(|idx| metadata.rows[row_idx].get(idx))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(normalize_sample_id)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| key.clone());
        selected.push(SelectedMetaRow {
            sample_id: sample.clone(),
            row_idx,
            is_case,
            subject,
        });
    }
    if selected.len() < 4 {
        bail!(
            "Only {} samples have an unambiguous metadata row and belong to the selected comparison",
            selected.len()
        );
    }

    #[derive(Debug, Clone)]
    enum Encoding {
        Numeric {
            idx: usize,
            mean: f64,
            sd: f64,
            name: String,
        },
        Categorical {
            idx: usize,
            levels: Vec<String>,
            name: String,
        },
    }

    let mut encodings = Vec::new();
    for cov in &spec.covariates {
        if cov == &metadata.id_column || spec.subject_column.as_ref() == Some(cov) {
            continue;
        }
        let Some(idx) = metadata.column_index(cov) else {
            continue;
        };
        let vals = selected
            .iter()
            .filter_map(|r| metadata.rows[r.row_idx].get(idx))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        if vals.is_empty() {
            continue;
        }
        let numeric = vals
            .iter()
            .all(|s| s.parse::<f64>().ok().is_some_and(|v| v.is_finite()));
        if numeric {
            let nums = vals
                .iter()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect::<Vec<_>>();
            let m = mean(&nums);
            let var = if nums.len() > 1 {
                nums.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (nums.len() - 1) as f64
            } else {
                0.0
            };
            let sd = var.sqrt();
            if sd > 1e-12 {
                encodings.push(Encoding::Numeric {
                    idx,
                    mean: m,
                    sd,
                    name: cov.clone(),
                });
            }
        } else {
            let mut levels = vals
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if levels.len() > 1 {
                // Deterministic baseline = lexicographically first level.
                levels.sort();
                encodings.push(Encoding::Categorical {
                    idx,
                    levels,
                    name: cov.clone(),
                });
            }
        }
    }

    let mut column_names = vec!["Intercept".to_string(), "Group(case)".to_string()];
    for enc in &encodings {
        match enc {
            Encoding::Numeric { name, .. } => column_names.push(name.clone()),
            Encoding::Categorical { levels, name, .. } => {
                for level in levels.iter().skip(1) {
                    column_names.push(format!("{name}={level}"));
                }
            }
        }
    }

    let mut out_ids = Vec::new();
    let mut groups = Vec::new();
    let mut subjects = Vec::new();
    let mut x = Vec::new();
    let mut dropped = 0usize;
    for r in selected {
        let mut row_x = vec![1.0, if r.is_case { 1.0 } else { 0.0 }];
        let mut complete = true;
        for enc in &encodings {
            match enc {
                Encoding::Numeric { idx, mean, sd, .. } => {
                    let value = metadata.rows[r.row_idx]
                        .get(*idx)
                        .and_then(|s| s.trim().parse::<f64>().ok())
                        .filter(|v| v.is_finite());
                    if let Some(value) = value {
                        row_x.push((value - mean) / sd);
                    } else {
                        complete = false;
                        break;
                    }
                }
                Encoding::Categorical { idx, levels, .. } => {
                    let value = metadata.rows[r.row_idx]
                        .get(*idx)
                        .map(|s| s.trim())
                        .unwrap_or_default();
                    if value.is_empty() {
                        complete = false;
                        break;
                    }
                    for level in levels.iter().skip(1) {
                        row_x.push(if value == level { 1.0 } else { 0.0 });
                    }
                }
            }
        }
        if !complete {
            dropped += 1;
            continue;
        }
        out_ids.push(r.sample_id);
        groups.push(r.is_case);
        subjects.push(r.subject);
        x.push(row_x);
    }
    if x.len() <= column_names.len() + 2 {
        bail!(
            "Adjusted model has {} complete samples for {} coefficients; remove covariates or resolve missing metadata",
            x.len(),
            column_names.len()
        );
    }
    let mut subject_group: HashMap<String, bool> = HashMap::new();
    for (subject, is_case) in subjects.iter().zip(groups.iter().copied()) {
        if let Some(previous) = subject_group.insert(subject.clone(), is_case) {
            if previous != is_case {
                bail!(
                    "Subject/patient '{}' appears in both case and control groups; resolve the metadata before repeated-measure analysis",
                    subject
                );
            }
        }
    }
    Ok(DesignMatrix {
        sample_ids: out_ids,
        group_case: groups,
        subjects,
        x,
        column_names,
        group_col: 1,
        dropped_missing_covariates: dropped,
    })
}

fn fit_adjusted(y: &[f64], design: &DesignMatrix) -> Result<(f64, f64, f64, f64, usize, String)> {
    if y.len() != design.x.len() {
        bail!("Response/design length mismatch");
    }
    let keep = y
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_finite())
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let p = design.column_names.len();
    if keep.len() <= p + 1 {
        bail!("Too few finite observations for adjusted model");
    }
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for &i in &keep {
        let xi = &design.x[i];
        for a in 0..p {
            xty[a] += xi[a] * y[i];
            for b in 0..p {
                xtx[a][b] += xi[a] * xi[b];
            }
        }
    }
    let bread =
        invert_matrix(&xtx).context("Design matrix is singular; remove collinear covariates")?;
    let beta = mat_vec(&bread, &xty);
    let residuals = keep
        .iter()
        .map(|&i| y[i] - dot(&design.x[i], &beta))
        .collect::<Vec<_>>();

    let mut cluster_members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (k, &i) in keep.iter().enumerate() {
        cluster_members
            .entry(design.subjects[i].clone())
            .or_default()
            .push(k);
    }
    let n_clusters = cluster_members.len();
    let use_cluster = n_clusters >= 3 && n_clusters < keep.len();
    let mut meat = vec![vec![0.0; p]; p];
    let method;
    if use_cluster {
        for members in cluster_members.values() {
            let mut score = vec![0.0; p];
            for &k in members {
                let i = keep[k];
                let u = residuals[k];
                for a in 0..p {
                    score[a] += design.x[i][a] * u;
                }
            }
            outer_add(&mut meat, &score, 1.0);
        }
        let n = keep.len() as f64;
        let g = n_clusters as f64;
        let correction = if g > 1.0 && n > p as f64 {
            (g / (g - 1.0)) * ((n - 1.0) / (n - p as f64))
        } else {
            1.0
        };
        scale_matrix(&mut meat, correction);
        method = "Adjusted linear model + subject-clustered sandwich SE".to_string();
    } else {
        // HC3 heteroskedasticity-consistent covariance.
        for (k, &i) in keep.iter().enumerate() {
            let xi = &design.x[i];
            let h = quadratic_form(xi, &bread).clamp(0.0, 0.999_999);
            let u = residuals[k] / (1.0 - h);
            let mut score = vec![0.0; p];
            for a in 0..p {
                score[a] = xi[a] * u;
            }
            outer_add(&mut meat, &score, 1.0);
        }
        method = "Adjusted linear model + HC3 robust SE".to_string();
    }
    let covariance = sandwich(&bread, &meat);
    let idx = design.group_col;
    let se = covariance[idx][idx].max(0.0).sqrt();
    let b = beta[idx];
    let z = if se > 0.0 { b / se } else { f64::NAN };
    let pval = if z.is_finite() {
        2.0 * (1.0 - normal_cdf(z.abs()))
    } else {
        f64::NAN
    };
    Ok((b, se, z, pval.clamp(0.0, 1.0), n_clusters, method))
}

fn fit_nb_adjusted(
    counts: &[f64],
    size_factors: &[f64],
    design: &DesignMatrix,
) -> Result<(f64, f64, f64, f64, usize, f64, String)> {
    if counts.len() != design.x.len() || size_factors.len() != counts.len() {
        bail!("Count/offset/design length mismatch");
    }
    let keep = counts
        .iter()
        .enumerate()
        .filter(|(_, y)| y.is_finite() && **y >= 0.0)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let p = design.column_names.len();
    if keep.len() <= p + 1 {
        bail!("Too few finite observations for negative-binomial model");
    }

    // Stable initialization from normalized log counts.  The actual fitted
    // model below is NB2 with a log link and log(size factor) offset.
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for &i in &keep {
        let xi = &design.x[i];
        let target = (counts[i] / size_factors[i].max(1e-12) + 0.5).ln();
        for a in 0..p {
            xty[a] += xi[a] * target;
            for b in 0..p {
                xtx[a][b] += xi[a] * xi[b];
            }
        }
    }
    let init_bread =
        invert_matrix(&xtx).context("Design matrix is singular; remove collinear covariates")?;
    let mut beta = mat_vec(&init_bread, &xty);
    let mut alpha = 0.05_f64;

    // Alternate NB IRLS and a transparent method-of-moments NB2 dispersion
    // estimate.  Unlike DESeq2/edgeR this dispersion is not EB-shrunk across
    // genes, so the report labels this explicitly as an unshrunk native NB GLM.
    for _outer in 0..4 {
        for _iter in 0..40 {
            let mut xwx = vec![vec![0.0; p]; p];
            let mut xwz = vec![0.0; p];
            for &i in &keep {
                let xi = &design.x[i];
                let offset = size_factors[i].max(1e-12).ln();
                let linear = dot(xi, &beta).clamp(-25.0, 25.0);
                let mu = (offset + linear).exp().clamp(1e-10, 1e14);
                let weight = (mu / (1.0 + alpha * mu)).max(1e-12);
                let working = linear + (counts[i] - mu) / mu;
                for a in 0..p {
                    xwz[a] += weight * xi[a] * working;
                    for b in 0..p {
                        xwx[a][b] += weight * xi[a] * xi[b];
                    }
                }
            }
            let bread =
                invert_matrix(&xwx).context("Negative-binomial weighted design is singular")?;
            let next = mat_vec(&bread, &xwz);
            let delta = next
                .iter()
                .zip(&beta)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);
            beta = next;
            if delta < 1e-8 {
                break;
            }
        }
        let mut num = 0.0;
        let mut den = 0.0;
        for &i in &keep {
            let mu = (size_factors[i].max(1e-12).ln()
                + dot(&design.x[i], &beta).clamp(-25.0, 25.0))
            .exp()
            .clamp(1e-10, 1e14);
            num += (counts[i] - mu).powi(2) - mu;
            den += mu * mu;
        }
        let next_alpha = if den > 0.0 {
            (num / den).max(1e-8).min(100.0)
        } else {
            alpha
        };
        if (next_alpha - alpha).abs() < 1e-6 * (1.0 + alpha) {
            alpha = next_alpha;
            break;
        }
        alpha = 0.5 * alpha + 0.5 * next_alpha;
    }

    let mut xwx = vec![vec![0.0; p]; p];
    let mut scores = Vec::with_capacity(keep.len());
    for &i in &keep {
        let xi = &design.x[i];
        let mu = (size_factors[i].max(1e-12).ln() + dot(xi, &beta).clamp(-25.0, 25.0))
            .exp()
            .clamp(1e-10, 1e14);
        let weight = (mu / (1.0 + alpha * mu)).max(1e-12);
        for a in 0..p {
            for b in 0..p {
                xwx[a][b] += weight * xi[a] * xi[b];
            }
        }
        let scalar = (counts[i] - mu) / (1.0 + alpha * mu);
        scores.push(xi.iter().map(|x| x * scalar).collect::<Vec<_>>());
    }
    let bread =
        invert_matrix(&xwx).context("Negative-binomial final information matrix is singular")?;
    let mut cluster_members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (k, &i) in keep.iter().enumerate() {
        cluster_members
            .entry(design.subjects[i].clone())
            .or_default()
            .push(k);
    }
    let n_clusters = cluster_members.len();
    let repeated = n_clusters >= 3 && n_clusters < keep.len();
    let covariance;
    let method;
    if repeated {
        let mut meat = vec![vec![0.0; p]; p];
        for members in cluster_members.values() {
            let mut score = vec![0.0; p];
            for &k in members {
                for a in 0..p {
                    score[a] += scores[k][a];
                }
            }
            outer_add(&mut meat, &score, 1.0);
        }
        let n = keep.len() as f64;
        let g = n_clusters as f64;
        let correction = if g > 1.0 && n > p as f64 {
            (g / (g - 1.0)) * ((n - 1.0) / (n - p as f64))
        } else {
            1.0
        };
        scale_matrix(&mut meat, correction);
        covariance = sandwich(&bread, &meat);
        method = format!(
            "Native NB2 GLM (unshrunk dispersion α={alpha:.4}) + subject-clustered sandwich SE"
        );
    } else {
        covariance = bread;
        method = format!("Native NB2 GLM (unshrunk dispersion α={alpha:.4}) + model-based Wald SE");
    }
    let idx = design.group_col;
    let se_ln = covariance[idx][idx].max(0.0).sqrt();
    let beta_ln = beta[idx];
    let z = if se_ln > 0.0 {
        beta_ln / se_ln
    } else {
        f64::NAN
    };
    let pval = if z.is_finite() {
        2.0 * (1.0 - normal_cdf(z.abs()))
    } else {
        f64::NAN
    };
    let ln2 = std::f64::consts::LN_2;
    Ok((
        beta_ln / ln2,
        se_ln / ln2,
        z,
        pval.clamp(0.0, 1.0),
        n_clusters,
        alpha,
        method,
    ))
}

fn make_nb_adjusted_test(
    feature: String,
    secondary_id: String,
    counts: &[f64],
    normalized: &[f64],
    size_factors: &[f64],
    design: &DesignMatrix,
) -> Option<AdjustedFeatureResult> {
    let (beta, se, z, p, n_subjects, _alpha, method) =
        fit_nb_adjusted(counts, size_factors, design).ok()?;
    let sensitivity_y = normalized
        .iter()
        .map(|x| (x + 0.5).log2())
        .collect::<Vec<_>>();
    let (subject_effect, subject_p, _) = subject_collapsed_test(&sensitivity_y, design);
    let mut case = Vec::new();
    let mut ctrl = Vec::new();
    for i in 0..normalized.len() {
        if !normalized[i].is_finite() {
            continue;
        }
        if design.group_case[i] {
            case.push(normalized[i]);
        } else {
            ctrl.push(normalized[i]);
        }
    }
    Some(AdjustedFeatureResult {
        feature,
        secondary_id,
        n_samples: case.len() + ctrl.len(),
        n_subjects,
        n_case: case.len(),
        n_control: ctrl.len(),
        mean_case: mean(&case),
        mean_control: mean(&ctrl),
        beta_group: beta,
        robust_se: se,
        z_value: z,
        p_value: p,
        q_value: f64::NAN,
        subject_effect,
        subject_p_value: subject_p,
        subject_q_value: f64::NAN,
        method,
    })
}

fn subject_collapsed_test(y: &[f64], design: &DesignMatrix) -> (f64, f64, usize) {
    let mut by_subject: BTreeMap<String, (bool, Vec<f64>)> = BTreeMap::new();
    for i in 0..y.len() {
        if !y[i].is_finite() {
            continue;
        }
        by_subject
            .entry(design.subjects[i].clone())
            .or_insert_with(|| (design.group_case[i], Vec::new()))
            .1
            .push(y[i]);
    }
    let mut case = Vec::new();
    let mut ctrl = Vec::new();
    for (_subject, (is_case, vals)) in by_subject {
        if vals.is_empty() {
            continue;
        }
        if is_case {
            case.push(mean(&vals));
        } else {
            ctrl.push(mean(&vals));
        }
    }
    let effect = mean(&case) - mean(&ctrl);
    let p = if case.len() >= 2 && ctrl.len() >= 2 {
        mann_whitney_p(&case, &ctrl)
    } else {
        f64::NAN
    };
    (effect, p, case.len() + ctrl.len())
}

fn make_adjusted_test(
    feature: String,
    secondary_id: String,
    y_model: &[f64],
    y_display: &[f64],
    design: &DesignMatrix,
) -> Option<AdjustedFeatureResult> {
    let (beta, se, z, p, n_subjects, method) = fit_adjusted(y_model, design).ok()?;
    let (subject_effect, subject_p, _) = subject_collapsed_test(y_model, design);
    let mut case = Vec::new();
    let mut ctrl = Vec::new();
    for i in 0..y_display.len() {
        if !y_display[i].is_finite() {
            continue;
        }
        if design.group_case[i] {
            case.push(y_display[i]);
        } else {
            ctrl.push(y_display[i]);
        }
    }
    Some(AdjustedFeatureResult {
        feature,
        secondary_id,
        n_samples: case.len() + ctrl.len(),
        n_subjects,
        n_case: case.len(),
        n_control: ctrl.len(),
        mean_case: mean(&case),
        mean_control: mean(&ctrl),
        beta_group: beta,
        robust_se: se,
        z_value: z,
        p_value: p,
        q_value: f64::NAN,
        subject_effect,
        subject_p_value: subject_p,
        subject_q_value: f64::NAN,
        method,
    })
}

fn apply_bh_adjusted(results: &mut [AdjustedFeatureResult]) {
    let q = bh_values(results.iter().map(|r| r.p_value));
    let sq = bh_values(results.iter().map(|r| r.subject_p_value));
    for (i, result) in results.iter_mut().enumerate() {
        result.q_value = q[i];
        result.subject_q_value = sq[i];
    }
    results.sort_by(|a, b| {
        finite_or_two(a.q_value)
            .total_cmp(&finite_or_two(b.q_value))
            .then_with(|| b.beta_group.abs().total_cmp(&a.beta_group.abs()))
            .then_with(|| a.feature.cmp(&b.feature))
    });
}

fn bh_values<I: IntoIterator<Item = f64>>(values: I) -> Vec<f64> {
    let vals = values.into_iter().collect::<Vec<_>>();
    let mut indexed = vals
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_finite())
        .map(|(i, p)| (i, *p))
        .collect::<Vec<_>>();
    indexed.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut out = vec![f64::NAN; vals.len()];
    let m = indexed.len();
    let mut running = 1.0;
    for rev in (0..m).rev() {
        let (idx, p) = indexed[rev];
        let rank = rev + 1;
        let q = (p * m as f64 / rank as f64).min(running).clamp(0.0, 1.0);
        out[idx] = q;
        running = q;
    }
    out
}

fn finite_or_two(x: f64) -> f64 {
    if x.is_finite() { x } else { 2.0 }
}

#[derive(Clone)]
struct VariableRow {
    variance: f64,
    values: Vec<f64>,
}

#[derive(Debug)]
struct ExpressionModelWorkRow {
    feature: String,
    secondary: String,
    counts: Vec<f64>,
}

fn trim_variable_rows(rows: &mut Vec<VariableRow>, keep: usize) {
    if rows.len() <= keep {
        return;
    }
    rows.sort_by(|a, b| b.variance.total_cmp(&a.variance));
    rows.truncate(keep);
}

fn model_expression_slice(
    rows: &[ExpressionModelWorkRow],
    size_factors: &[f64],
    design: &DesignMatrix,
    min_count: f64,
    min_present: usize,
) -> (Vec<AdjustedFeatureResult>, usize, Vec<VariableRow>) {
    let mut tests = Vec::with_capacity(rows.len());
    let mut filtered = 0usize;
    let mut variable = Vec::new();
    for row in rows {
        let normalized = row
            .counts
            .iter()
            .zip(size_factors.iter())
            .map(|(count, sf)| count / sf.max(1e-12))
            .collect::<Vec<_>>();
        if normalized
            .iter()
            .filter(|value| **value >= min_count)
            .count()
            < min_present
        {
            filtered += 1;
            continue;
        }
        let y = normalized
            .iter()
            .map(|value| (value + 0.5).log2())
            .collect::<Vec<_>>();
        if let Some(test) = make_nb_adjusted_test(
            row.feature.clone(),
            row.secondary.clone(),
            &row.counts,
            &normalized,
            size_factors,
            design,
        ) {
            tests.push(test);
        }
        let variance = sample_variance(&y);
        if variance.is_finite() {
            variable.push(VariableRow {
                variance,
                values: y,
            });
        }
    }
    // The final ordination only needs the global top 300 rows. Retaining the
    // best 350 per worker is sufficient to preserve that global top set while
    // bounding merge memory for transcript-scale matrices.
    trim_variable_rows(&mut variable, 350);
    (tests, filtered, variable)
}

fn expression_model_worker_count(row_count: usize) -> usize {
    if row_count == 0 {
        return 1;
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism()
            .map(|value| value.get().min(8).min(row_count).max(1))
            .unwrap_or(1)
    }
}

fn model_expression_batch_parallel(
    rows: &[ExpressionModelWorkRow],
    size_factors: &[f64],
    design: &DesignMatrix,
    min_count: f64,
    min_present: usize,
) -> Result<(Vec<AdjustedFeatureResult>, usize, Vec<VariableRow>)> {
    if rows.is_empty() {
        return Ok((Vec::new(), 0, Vec::new()));
    }
    #[cfg(target_arch = "wasm32")]
    {
        return Ok(model_expression_slice(
            rows,
            size_factors,
            design,
            min_count,
            min_present,
        ));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let worker_count = expression_model_worker_count(rows.len());
        if worker_count == 1 || rows.len() < 256 {
            return Ok(model_expression_slice(
                rows,
                size_factors,
                design,
                min_count,
                min_present,
            ));
        }
        let chunk_size = rows.len().div_ceil(worker_count);
        return std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in rows.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    model_expression_slice(chunk, size_factors, design, min_count, min_present)
                }));
            }
            let mut tests = Vec::new();
            let mut filtered = 0usize;
            let mut variable = Vec::new();
            for handle in handles {
                let (mut local_tests, local_filtered, mut local_variable) = handle
                    .join()
                    .map_err(|_| anyhow!("parallel corrected-expression worker panicked"))?;
                tests.append(&mut local_tests);
                filtered += local_filtered;
                variable.append(&mut local_variable);
            }
            trim_variable_rows(&mut variable, 350);
            Ok((tests, filtered, variable))
        });
    }
    #[allow(unreachable_code)]
    Ok(model_expression_slice(
        rows,
        size_factors,
        design,
        min_count,
        min_present,
    ))
}

pub fn run_corrected_expression<F>(
    merged_tsv: &Path,
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    source_metric: &str,
    feature_label: &str,
    spec: &CorrectedModelSpec,
    progress: F,
) -> Result<CorrectedExpressionResult>
where
    F: FnMut(usize, usize, &str),
{
    run_corrected_expression_with_reference(
        merged_tsv,
        metadata,
        comparison,
        source_metric,
        feature_label,
        spec,
        None,
        progress,
    )
}

/// Run the corrected expression model while optionally reusing count
/// normalization from a prior corrected run that used the exact same model
/// sample population and source metric. The statistical model is unchanged;
/// only the redundant median-ratio normalization pass is skipped when safe.
pub fn run_corrected_expression_with_reference<F>(
    merged_tsv: &Path,
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    source_metric: &str,
    feature_label: &str,
    spec: &CorrectedModelSpec,
    normalization_reference: Option<&CorrectedExpressionResult>,
    mut progress: F,
) -> Result<CorrectedExpressionResult>
where
    F: FnMut(usize, usize, &str),
{
    let mut header_reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)
        .with_context(|| format!("Could not open {}", merged_tsv.display()))?;
    let headers = header_reader.headers()?.clone();
    let suffix = format!(" {source_metric}");
    let mut header_cols: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (idx, header) in headers.iter().enumerate().skip(2) {
        if let Some(sample) = header.strip_suffix(&suffix) {
            let key = normalize_sample_id(sample);
            if comparison.is_case_id(&key) || comparison.is_control_id(&key) {
                header_cols.entry(key).or_insert((sample.to_string(), idx));
            }
        }
    }
    if header_cols.len() < 4 {
        bail!(
            "Merged {feature_label} table has too few '{source_metric}' sample columns for this comparison"
        );
    }
    let candidate_ids = header_cols
        .values()
        .map(|(s, _)| s.clone())
        .collect::<Vec<_>>();
    let design = build_design(metadata, comparison, &candidate_ids, spec)?;
    let mut design_cols = Vec::new();
    for sample in &design.sample_ids {
        let key = normalize_sample_id(sample);
        let Some((_, idx)) = header_cols.get(&key) else {
            continue;
        };
        design_cols.push(*idx);
    }
    if design_cols.len() != design.sample_ids.len() {
        bail!("Could not align adjusted metadata design with expression columns");
    }

    // Use byte-position progress instead of pre-parsing the complete merged
    // matrix solely to count rows. The old row-count pass was an extra full
    // CSV/TSV parse before normalization/modeling and was particularly costly
    // for transcript-scale matrices with hundreds of sample columns.
    let file_bytes = fs::metadata(merged_tsv)
        .with_context(|| format!("Could not stat {}", merged_tsv.display()))?
        .len()
        .max(1);
    let file_units = usize::try_from(file_bytes).unwrap_or(usize::MAX / 4).max(1);
    let is_transcript = feature_label.eq_ignore_ascii_case("Transcript");
    // Gene: normalization + feature models. Transcript: those two passes plus
    // the two DTU matrix passes. Saturating arithmetic keeps progress robust
    // even for unusually large files on narrower usize targets.
    let total_work = if is_transcript {
        file_units.saturating_mul(4)
    } else {
        file_units.saturating_mul(2)
    }
    .max(1);
    let reusable_normalization = normalization_reference.filter(|reference| {
        reference.source_metric == source_metric
            && reference.model_sample_ids.as_slice() == design.sample_ids.as_slice()
            && reference.size_factors.len() == design.sample_ids.len()
            && reference
                .size_factors
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
    });
    let size_factors = if let Some(reference) = reusable_normalization {
        progress(
            file_units.min(total_work),
            total_work,
            &format!("{feature_label}: reusing precomputed count size factors"),
        );
        reference.size_factors.clone()
    } else {
        // Pass 1: DESeq2-style `poscounts` median-ratio size factors. P² keeps the
        // median estimate bounded-memory even for transcript-scale matrices.
        let mut medians = vec![P2Median::new(); design_cols.len()];
        let mut pass1 = ReaderBuilder::new()
            .delimiter(b'\t')
            .flexible(true)
            .from_path(merged_tsv)?;
        let mut record = csv::StringRecord::new();
        let mut row_idx = 0usize;
        while pass1.read_record(&mut record)? {
            let counts = values_at_columns_with_zero(&record, &design_cols);
            let mut positive_count = 0usize;
            let mut positive_log_sum = 0.0_f64;
            for count in &counts {
                if *count > 0.0 {
                    positive_count += 1;
                    positive_log_sum += count.ln();
                }
            }
            if positive_count >= 2 {
                let gmean = (positive_log_sum / positive_count as f64).exp();
                if gmean > 0.0 && gmean.is_finite() {
                    for (j, count) in counts.iter().enumerate() {
                        if *count > 0.0 {
                            medians[j].push(*count / gmean);
                        }
                    }
                }
            }
            row_idx += 1;
            if row_idx % 1000 == 0 {
                let done = usize::try_from(pass1.position().byte())
                    .unwrap_or(file_units)
                    .min(file_units);
                progress(
                    done,
                    total_work,
                    &format!("{feature_label}: estimating count size factors"),
                );
            }
        }
        progress(
            file_units.min(total_work),
            total_work,
            &format!("{feature_label}: count size factors complete ({row_idx} rows)"),
        );
        let mut factors = medians
            .iter()
            .map(|m| {
                m.value()
                    .filter(|v| *v > 0.0 && v.is_finite())
                    .unwrap_or(1.0)
            })
            .collect::<Vec<_>>();
        let sf_gmean = (factors.iter().map(|x| x.ln()).sum::<f64>() / factors.len() as f64).exp();
        if sf_gmean.is_finite() && sf_gmean > 0.0 {
            for sf in &mut factors {
                *sf /= sf_gmean;
            }
        }
        factors
    };

    let min_present = ((design.x.len() as f64 * spec.min_fraction_samples).ceil() as usize).max(3);
    let mut tests = Vec::new();
    let mut filtered = 0usize;
    let mut top_variable: Vec<VariableRow> = Vec::new();
    let mut pass2 = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    const MODEL_BATCH_ROWS: usize = 4096;
    let mut work_batch = Vec::with_capacity(MODEL_BATCH_ROWS);
    let mut modeled_rows = 0usize;

    let mut finish_batch =
        |batch: &mut Vec<ExpressionModelWorkRow>, pass2_done: usize| -> Result<()> {
            if batch.is_empty() {
                return Ok(());
            }
            let batch_rows = batch.len();
            let (mut batch_tests, batch_filtered, mut batch_variable) =
                model_expression_batch_parallel(
                    batch,
                    &size_factors,
                    &design,
                    spec.min_count,
                    min_present,
                )?;
            tests.append(&mut batch_tests);
            filtered += batch_filtered;
            top_variable.append(&mut batch_variable);
            trim_variable_rows(&mut top_variable, 350);
            modeled_rows += batch_rows;
            progress(
                file_units.saturating_add(pass2_done.min(file_units)),
                total_work,
                &format!(
                    "{feature_label}: adjusted feature models ({} worker(s); {modeled_rows} rows)",
                    expression_model_worker_count(batch_rows)
                ),
            );
            batch.clear();
            Ok(())
        };

    let mut record = csv::StringRecord::new();
    while pass2.read_record(&mut record)? {
        work_batch.push(ExpressionModelWorkRow {
            feature: record.get(0).unwrap_or_default().to_string(),
            secondary: record.get(1).unwrap_or_default().to_string(),
            counts: values_at_columns_with_zero(&record, &design_cols),
        });
        if work_batch.len() >= MODEL_BATCH_ROWS {
            let done = usize::try_from(pass2.position().byte())
                .unwrap_or(file_units)
                .min(file_units);
            finish_batch(&mut work_batch, done)?;
        }
    }
    let pass2_done = usize::try_from(pass2.position().byte())
        .unwrap_or(file_units)
        .min(file_units);
    finish_batch(&mut work_batch, pass2_done)?;
    apply_bh_adjusted(&mut tests);
    top_variable.sort_by(|a, b| b.variance.total_cmp(&a.variance));
    top_variable.truncate(300);
    let ordination_rows = top_variable
        .iter()
        .map(|r| r.values.clone())
        .collect::<Vec<_>>();
    let pca = pca_from_rows(
        &ordination_rows,
        &design,
        &format!("{feature_label} PCA — top variable normalized features"),
        0,
    );
    let dendrogram = if ordination_rows.is_empty() {
        None
    } else {
        let distances = euclidean_distance_from_rows(&ordination_rows);
        average_linkage_dendrogram(
            &distances,
            &design,
            &format!("{feature_label} hierarchical clustering — top variable normalized features"),
            "Euclidean distance",
        )
    };
    let dtu = if is_transcript {
        // DTU is two more full transcript-matrix passes. Give those passes
        // their own byte-position progress so a long DTU screen cannot look
        // like the previous feature-model stage has stalled.
        progress(
            file_units.saturating_mul(2).min(total_work),
            total_work,
            "Transcript: DTU gene-total pass",
        );
        match run_dtu_screen(
            merged_tsv,
            &design_cols,
            &design,
            spec,
            |done, total, msg| {
                let denom = total.max(1);
                let dtu_span = file_units.saturating_mul(2);
                let scaled = done.saturating_mul(dtu_span) / denom;
                progress(
                    file_units
                        .saturating_mul(2)
                        .saturating_add(scaled)
                        .min(total_work),
                    total_work,
                    msg,
                );
            },
        ) {
            Ok(result) => Some(result),
            Err(_) => None,
        }
    } else {
        None
    };
    let mut warnings = Vec::new();
    if feature_label.eq_ignore_ascii_case("Transcript") && dtu.is_none() {
        warnings.push("Differential transcript-usage screen could not be constructed; differential transcript abundance results remain available.".to_string());
    }
    if design.dropped_missing_covariates > 0 {
        warnings.push(format!(
            "{} sample(s) were excluded because one or more selected covariates were missing",
            design.dropped_missing_covariates
        ));
    }
    if spec.subject_column.is_none() {
        warnings.push("No subject/repeated-measure column was selected; count models use ordinary NB Wald SEs and the subject-collapsed sensitivity result is equivalent to one observation per sample.".to_string());
    }
    progress(
        total_work,
        total_work,
        &format!("{feature_label}: corrected analysis complete"),
    );
    Ok(CorrectedExpressionResult {
        input: merged_tsv.to_path_buf(),
        feature_label: feature_label.to_string(),
        source_metric: source_metric.to_string(),
        normalization: "Positive-count median-ratio size factors; count models include a log(size-factor) offset".to_string(),
        model_description: if spec.subject_column.is_some() {
            format!("Negative-binomial log-link model: group + {} covariate(s), per-feature dispersion, subject-clustered sandwich SE; subject-collapsed log2-normalized Mann-Whitney sensitivity", spec.covariates.len())
        } else {
            format!("Negative-binomial log-link model: group + {} covariate(s), per-feature dispersion, model-based Wald SE; sample-level log2-normalized sensitivity", spec.covariates.len())
        },
        model_sample_ids: design.sample_ids.clone(),
        size_factors,
        n_input_samples: candidate_ids.len(),
        n_model_samples: design.x.len(),
        n_subjects: design.n_subjects(),
        tested_features: tests.len(),
        filtered_features: filtered,
        tests,
        pca,
        dendrogram,
        dtu,
        warnings,
    })
}

fn run_dtu_screen<F>(
    merged_tsv: &Path,
    design_cols: &[usize],
    design: &DesignMatrix,
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<CorrectedDtuResult>
where
    F: FnMut(usize, usize, &str),
{
    let n = design_cols.len();
    let pseudo = 0.5_f64;
    let min_present = ((n as f64 * spec.min_fraction_samples).ceil() as usize).max(3);
    let file_bytes = fs::metadata(merged_tsv)
        .with_context(|| format!("Could not stat {}", merged_tsv.display()))?
        .len()
        .max(1);
    let file_units = usize::try_from(file_bytes).unwrap_or(usize::MAX / 2).max(1);
    let total_work = file_units.saturating_mul(2).max(1);
    let mut transcript_counts: HashMap<String, usize> = HashMap::new();
    let mut gene_totals: HashMap<String, Vec<f64>> = HashMap::new();
    let mut gene_log_sums: HashMap<String, Vec<f64>> = HashMap::new();

    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    let mut record = csv::StringRecord::new();
    let mut rows_seen = 0usize;
    while reader.read_record(&mut record)? {
        rows_seen += 1;
        let gene = record.get(1).unwrap_or_default().trim();
        if !gene.is_empty() {
            let counts = values_at_columns_with_zero(&record, design_cols);
            *transcript_counts.entry(gene.to_string()).or_insert(0) += 1;
            let totals = gene_totals
                .entry(gene.to_string())
                .or_insert_with(|| vec![0.0; n]);
            let logs = gene_log_sums
                .entry(gene.to_string())
                .or_insert_with(|| vec![0.0; n]);
            for j in 0..n {
                totals[j] += counts[j];
                logs[j] += (counts[j] + pseudo).ln();
            }
        }
        if rows_seen % 1000 == 0 {
            let done = usize::try_from(reader.position().byte())
                .unwrap_or(file_units)
                .min(file_units);
            progress(done, total_work, "Transcript: DTU gene-total pass");
        }
    }
    progress(
        file_units,
        total_work,
        &format!("Transcript: DTU gene-total pass complete ({rows_seen} rows)"),
    );

    let eligible = transcript_counts
        .iter()
        .filter_map(|(gene, &n_tx)| {
            if n_tx < 2 {
                return None;
            }
            let totals = gene_totals.get(gene)?;
            if totals.iter().filter(|v| **v >= spec.min_count).count() >= min_present {
                Some(gene.clone())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    let mut transcript_tests = Vec::new();
    let mut by_gene_p: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    let mut record = csv::StringRecord::new();
    let mut rows_seen_second = 0usize;
    while reader.read_record(&mut record)? {
        rows_seen_second += 1;
        let transcript = record.get(0).unwrap_or_default().trim();
        let gene = record.get(1).unwrap_or_default().trim();
        if !transcript.is_empty() && eligible.contains(gene) {
            let n_tx = *transcript_counts.get(gene).unwrap_or(&1) as f64;
            let counts = values_at_columns_with_zero(&record, design_cols);
            if let (Some(log_sums), Some(totals)) = (gene_log_sums.get(gene), gene_totals.get(gene))
            {
                let clr = (0..n)
                    .map(|j| (counts[j] + pseudo).ln() - log_sums[j] / n_tx)
                    .collect::<Vec<_>>();
                let usage = (0..n)
                    .map(|j| {
                        if totals[j] > 0.0 {
                            counts[j] / totals[j]
                        } else {
                            0.0
                        }
                    })
                    .collect::<Vec<_>>();
                // Suppress essentially absent transcripts before the multiple-testing burden.
                if !(mean(&usage) < 0.001
                    && usage.iter().filter(|v| **v > 0.0).count() < min_present)
                {
                    if let Some(test) = make_adjusted_test(
                        transcript.to_string(),
                        gene.to_string(),
                        &clr,
                        &usage,
                        design,
                    ) {
                        by_gene_p
                            .entry(gene.to_string())
                            .or_default()
                            .push((test.p_value, test.beta_group.abs()));
                        transcript_tests.push(test);
                    }
                }
            }
        }
        if rows_seen_second % 500 == 0 {
            let done = usize::try_from(reader.position().byte())
                .unwrap_or(file_units)
                .min(file_units);
            progress(
                file_units.saturating_add(done),
                total_work,
                "Transcript: DTU transcript-model pass",
            );
        }
    }
    progress(
        total_work,
        total_work,
        &format!("Transcript: DTU transcript-model pass complete ({rows_seen_second} rows)"),
    );
    apply_bh_adjusted(&mut transcript_tests);

    let mut gene_tests = by_gene_p
        .into_iter()
        .map(|(gene, mut entries)| {
            entries.retain(|(p, _)| p.is_finite());
            entries.sort_by(|a, b| a.0.total_cmp(&b.0));
            let m = entries.len();
            let simes = if m == 0 {
                f64::NAN
            } else {
                entries
                    .iter()
                    .enumerate()
                    .map(|(i, (p, _))| p * m as f64 / (i + 1) as f64)
                    .fold(1.0_f64, f64::min)
                    .clamp(0.0, 1.0)
            };
            let max_effect = entries.iter().map(|(_, e)| *e).fold(0.0_f64, f64::max);
            DtuGeneResult {
                gene_id: gene,
                n_transcripts: m,
                p_value: simes,
                q_value: f64::NAN,
                max_abs_clr_effect: max_effect,
            }
        })
        .collect::<Vec<_>>();
    let q = bh_values(gene_tests.iter().map(|r| r.p_value));
    for (r, qv) in gene_tests.iter_mut().zip(q) {
        r.q_value = qv;
    }
    gene_tests.sort_by(|a, b| {
        finite_or_two(a.q_value)
            .total_cmp(&finite_or_two(b.q_value))
            .then_with(|| b.max_abs_clr_effect.total_cmp(&a.max_abs_clr_effect))
    });
    Ok(CorrectedDtuResult {
        genes_tested: gene_tests.len(),
        transcripts_tested: transcript_tests.len(),
        transcript_tests,
        gene_tests,
        method_description: "Dependency-light DTU screen: within each multi-transcript gene, transcript counts are CLR-transformed with a 0.5 pseudocount and modeled with the same covariates/subject-robust framework. Gene-level p-values are Simes combinations across transcript tests followed by BH. This is not DEXSeq/DRIMSeq Dirichlet-multinomial inference.".to_string(),
    })
}

pub fn run_pathway_ora(
    gmt_path: &Path,
    gene_result: &CorrectedExpressionResult,
    q_threshold: f64,
    effect_threshold: f64,
) -> Result<CorrectedPathwayResult> {
    let q_threshold = q_threshold.clamp(0.0, 1.0);
    let effect_threshold = effect_threshold.max(0.0);
    let universe = gene_result
        .tests
        .iter()
        .map(|r| r.feature.trim().to_ascii_uppercase())
        .filter(|g| !g.is_empty())
        .collect::<HashSet<_>>();
    let foreground = gene_result
        .tests
        .iter()
        .filter(|r| {
            r.q_value.is_finite()
                && r.q_value <= q_threshold
                && r.beta_group.abs() >= effect_threshold
        })
        .map(|r| r.feature.trim().to_ascii_uppercase())
        .filter(|g| !g.is_empty())
        .collect::<HashSet<_>>();
    if universe.len() < 20 {
        bail!("Too few tested gene symbols are available for pathway analysis");
    }
    if foreground.is_empty() {
        bail!(
            "No genes satisfy BH q <= {q_threshold:.3} and |adjusted log2FC| >= {effect_threshold:.3}"
        );
    }
    let min_set_size = 10usize;
    let max_set_size = 500usize;
    let text = std::fs::read_to_string(gmt_path)
        .with_context(|| format!("Could not read GMT {}", gmt_path.display()))?;
    let mut tests = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 3 {
            continue;
        }
        let pathway = fields[0].trim();
        if pathway.is_empty() {
            continue;
        }
        let description = fields[1].trim();
        let genes = fields[2..]
            .iter()
            .map(|g| g.trim().to_ascii_uppercase())
            .filter(|g| !g.is_empty())
            .collect::<HashSet<_>>();
        let measured = genes
            .intersection(&universe)
            .cloned()
            .collect::<HashSet<_>>();
        let n = measured.len();
        if n < min_set_size || n > max_set_size {
            continue;
        }
        let mut overlap = measured
            .intersection(&foreground)
            .cloned()
            .collect::<Vec<_>>();
        overlap.sort();
        let k = overlap.len();
        let p = hypergeom_upper_tail(universe.len(), foreground.len(), n, k);
        let neither = universe
            .len()
            .saturating_sub(foreground.len() + n)
            .saturating_add(k);
        let odds = ((k as f64 + 0.5) * (neither as f64 + 0.5))
            / (((n - k) as f64 + 0.5) * ((foreground.len() - k) as f64 + 0.5));
        tests.push(PathwayOraResult {
            pathway: pathway.to_string(),
            description: description.to_string(),
            measured_set_size: n,
            overlap: k,
            odds_ratio: odds,
            p_value: p,
            q_value: f64::NAN,
            overlap_genes: overlap.join(", "),
        });
    }
    if tests.is_empty() {
        bail!(
            "No GMT gene sets overlapped the measured universe with {min_set_size}-{max_set_size} genes"
        );
    }
    let q = bh_values(tests.iter().map(|r| r.p_value));
    for (r, qv) in tests.iter_mut().zip(q) {
        r.q_value = qv;
    }
    tests.sort_by(|a, b| {
        finite_or_two(a.q_value)
            .total_cmp(&finite_or_two(b.q_value))
            .then_with(|| b.odds_ratio.total_cmp(&a.odds_ratio))
            .then_with(|| a.pathway.cmp(&b.pathway))
    });
    Ok(CorrectedPathwayResult {
        gmt_path: gmt_path.to_path_buf(),
        universe_size: universe.len(),
        foreground_size: foreground.len(),
        q_threshold,
        effect_threshold,
        min_set_size,
        max_set_size,
        tests,
    })
}

fn hypergeom_upper_tail(population: usize, successes: usize, draws: usize, observed: usize) -> f64 {
    if population == 0 || successes > population || draws > population {
        return f64::NAN;
    }
    let max_k = successes.min(draws);
    if observed == 0 {
        return 1.0;
    }
    if observed > max_k {
        return 0.0;
    }
    let min_k = draws.saturating_sub(population - successes);
    let start = observed.max(min_k);
    let logs = (start..=max_k)
        .map(|k| {
            ln_choose(successes, k) + ln_choose(population - successes, draws - k)
                - ln_choose(population, draws)
        })
        .collect::<Vec<_>>();
    let max_log = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !max_log.is_finite() {
        return f64::NAN;
    }
    (max_log + logs.iter().map(|v| (v - max_log).exp()).sum::<f64>().ln())
        .exp()
        .clamp(0.0, 1.0)
}
fn ln_choose(n: usize, k: usize) -> f64 {
    if k > n {
        return f64::NEG_INFINITY;
    }
    let k = k.min(n - k);
    (1..=k)
        .map(|i| ((n - k + i) as f64).ln() - (i as f64).ln())
        .sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ByteSpan {
    start: u32,
    len: u32,
}

impl ByteSpan {
    fn new(buffer_len: usize, len: usize) -> Result<Self> {
        let start = u32::try_from(buffer_len)
            .map_err(|_| anyhow!("repertoire identity arena exceeds 4 GiB"))?;
        let len = u32::try_from(len)
            .map_err(|_| anyhow!("individual repertoire identity exceeds 4 GiB"))?;
        Ok(Self { start, len })
    }

    fn slice<'a>(self, buffer: &'a [u8]) -> &'a [u8] {
        let start = self.start as usize;
        let end = start + self.len as usize;
        &buffer[start..end]
    }
}

const UNRESOLVED_CLONOTYPE_ID: u32 = u32::MAX;

#[derive(Debug)]
struct ParsedClonotype {
    count: f64,
    /// Stable FNV-1a acceleration hash for the exact `V|J|CDR3` bytes.
    ///
    /// This is never scientific identity by itself. During the one-time
    /// identity compaction pass, equal hashes are resolved by exact byte
    /// comparison before a dense numeric clonotype ID is assigned.
    hash: u64,
    key: ByteSpan,
    clonotype_id: u32,
    convergent: bool,
}

#[derive(Debug, Default)]
struct ParsedClonotypeStore {
    /// Exact `V|J|CDR3` identities are packed back-to-back in one byte arena
    /// per sample while files are parsed. After all model samples are aligned,
    /// a one-time exact interning pass assigns dense numeric clonotype IDs and
    /// releases these arenas before sharing/distance calculations begin.
    key_bytes: Vec<u8>,
    entries: Vec<ParsedClonotype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClonotypeKeyRef<'a>(&'a [u8]);

impl ParsedClonotypeStore {
    fn len(&self) -> usize {
        self.entries.len()
    }

    fn iter(&self) -> impl Iterator<Item = (ClonotypeKeyRef<'_>, &ParsedClonotype)> {
        self.entries
            .iter()
            .map(|entry| (ClonotypeKeyRef(entry.key.slice(&self.key_bytes)), entry))
    }
}

#[derive(Debug)]
struct BuildingClonotype {
    key: ByteSpan,
    count: f64,
    hash: u64,
    first_nt: Option<ByteSpan>,
    convergent: bool,
    next_same_hash: Option<u32>,
}

#[derive(Debug, Default)]
struct ParsedClonotypeScratch {
    /// Per-sample convergence nucleotide evidence. The allocation is retained
    /// and reused across every sample in one repertoire modality so parsing a
    /// few hundred files does not repeatedly allocate/free a large byte arena.
    nt_bytes: Vec<u8>,
    /// Exact-key acceleration index used only while one sample is parsed.
    /// Entries are primitive `(u64 -> u32)` values, so `clear()` is cheap and
    /// keeps the bucket allocation for the next sample.
    hash_heads: HashMap<u64, u32>,
}

impl ParsedClonotypeScratch {
    fn reset(&mut self) {
        self.nt_bytes.clear();
        self.hash_heads.clear();
    }
}

#[derive(Debug)]
struct ParsedClonotypeBuilder<'a> {
    key_bytes: Vec<u8>,
    entries: Vec<BuildingClonotype>,
    scratch: &'a mut ParsedClonotypeScratch,
}

impl<'a> ParsedClonotypeBuilder<'a> {
    fn with_capacity(row_count: usize, scratch: &'a mut ParsedClonotypeScratch) -> Self {
        // Avoid one very large allocator request at the exact boundary that
        // became intermittently pathological late in TCR-beta parsing. The
        // retained sample payload grows amortized as needed; the much more
        // churn-prone hash/NT scratch allocations are reused across samples.
        const INITIAL_REPERTOIRE_ROWS: usize = 16_384;
        let initial_rows = row_count.min(INITIAL_REPERTOIRE_ROWS);
        Self {
            key_bytes: Vec::with_capacity(initial_rows.saturating_mul(48)),
            entries: Vec::with_capacity(initial_rows),
            scratch,
        }
    }

    fn record(
        &mut self,
        v_gene: &str,
        j_gene: &str,
        cdr3_aa: &str,
        count: f64,
        nucleotide: Option<&str>,
    ) -> Result<()> {
        self.record_hashed(
            clonotype_key_hash(v_gene, j_gene, cdr3_aa),
            v_gene,
            j_gene,
            cdr3_aa,
            count,
            nucleotide,
        )
    }

    fn record_hashed(
        &mut self,
        hash: u64,
        v_gene: &str,
        j_gene: &str,
        cdr3_aa: &str,
        count: f64,
        nucleotide: Option<&str>,
    ) -> Result<()> {
        let mut cursor = self.scratch.hash_heads.get(&hash).copied();
        let mut matched = None;
        while let Some(index) = cursor {
            let entry = &self.entries[index as usize];
            if clonotype_key_matches(entry.key.slice(&self.key_bytes), v_gene, j_gene, cdr3_aa) {
                matched = Some(index as usize);
                break;
            }
            cursor = entry.next_same_hash;
        }

        if let Some(index) = matched {
            self.entries[index].count += count;
            let nucleotide = nucleotide.filter(|value| !value.is_empty());
            let (convergent, first_nt) = {
                let entry = &self.entries[index];
                (entry.convergent, entry.first_nt)
            };
            if let Some(nucleotide) = nucleotide {
                if !convergent {
                    match first_nt {
                        None => {
                            let span = append_arena_bytes(
                                &mut self.scratch.nt_bytes,
                                nucleotide.as_bytes(),
                            )?;
                            self.entries[index].first_nt = Some(span);
                        }
                        Some(span)
                            if span.slice(&self.scratch.nt_bytes) != nucleotide.as_bytes() =>
                        {
                            self.entries[index].convergent = true;
                            self.entries[index].first_nt = None;
                        }
                        Some(_) => {}
                    }
                }
            }
            return Ok(());
        }

        let key_start = self.key_bytes.len();
        self.key_bytes.extend_from_slice(v_gene.as_bytes());
        self.key_bytes.push(b'|');
        self.key_bytes.extend_from_slice(j_gene.as_bytes());
        self.key_bytes.push(b'|');
        self.key_bytes.extend_from_slice(cdr3_aa.as_bytes());
        let key = ByteSpan::new(key_start, self.key_bytes.len() - key_start)?;
        let first_nt = nucleotide
            .filter(|value| !value.is_empty())
            .map(|value| append_arena_bytes(&mut self.scratch.nt_bytes, value.as_bytes()))
            .transpose()?;
        let index = u32::try_from(self.entries.len())
            .map_err(|_| anyhow!("sample contains more than 4 billion clonotypes"))?;
        let previous = self.scratch.hash_heads.insert(hash, index);
        self.entries.push(BuildingClonotype {
            key,
            count,
            hash,
            first_nt,
            convergent: false,
            next_same_hash: previous,
        });
        Ok(())
    }

    fn finish(self) -> ParsedClonotypeStore {
        ParsedClonotypeStore {
            key_bytes: self.key_bytes,
            entries: self
                .entries
                .into_iter()
                .map(|entry| ParsedClonotype {
                    count: entry.count,
                    hash: entry.hash,
                    key: entry.key,
                    clonotype_id: UNRESOLVED_CLONOTYPE_ID,
                    convergent: entry.convergent,
                })
                .collect(),
        }
        // The hash index and NT convergence arena live in reusable parser
        // scratch owned by the modality loop; finishing a sample therefore
        // does not free/reallocate those large scratch buffers.
    }
}

fn append_arena_bytes(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<ByteSpan> {
    let span = ByteSpan::new(buffer.len(), bytes.len())?;
    buffer.extend_from_slice(bytes);
    Ok(span)
}

fn clonotype_key_hash(v_gene: &str, j_gene: &str, cdr3_aa: &str) -> u64 {
    // Stable FNV-1a is sufficient as an acceleration hash because collisions
    // are always resolved with exact arena-backed byte comparison below.
    let mut hash = 0xcbf29ce484222325_u64;
    for bytes in [
        v_gene.as_bytes(),
        &b"|"[..],
        j_gene.as_bytes(),
        &b"|"[..],
        cdr3_aa.as_bytes(),
    ] {
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn clonotype_key_matches(stored: &[u8], v_gene: &str, j_gene: &str, cdr3_aa: &str) -> bool {
    let expected_len = v_gene.len() + 1 + j_gene.len() + 1 + cdr3_aa.len();
    if stored.len() != expected_len {
        return false;
    }
    let v_end = v_gene.len();
    let j_start = v_end + 1;
    let j_end = j_start + j_gene.len();
    let cdr3_start = j_end + 1;
    &stored[..v_end] == v_gene.as_bytes()
        && stored[v_end] == b'|'
        && &stored[j_start..j_end] == j_gene.as_bytes()
        && stored[j_end] == b'|'
        && &stored[cdr3_start..] == cdr3_aa.as_bytes()
}

#[derive(Debug)]
struct ParsedRepertoire {
    sample_id: String,
    group: String,
    counts: Vec<f64>,
    clonotypes: ParsedClonotypeStore,
    feature_maps: BTreeMap<String, HashMap<String, f64>>,
    top_clone_fraction: f64,
    weighted_mean_cdr3_aa_length: f64,
}

/// Collapse exact clonotype identities across model samples into dense numeric IDs.
///
/// v0.16.17 used `HashMap<ClonotypeKeyRef, ...>` for sharing and distance
/// calculations. At TCR-beta scale that re-hashed millions of full variable-
/// length byte strings and retained a second multi-million-entry hash table,
/// moving the performance regression from teardown into clonotype sharing.
///
/// This pass instead hashes only the precomputed u64 acceleration hash. Equal
/// hashes are resolved against the original arena bytes with exact comparison,
/// so hash collisions can never define scientific identity. Once every local
/// entry has a dense ID, all per-sample identity arenas are released and every
/// downstream sharing/distance operation uses integer indexing only.
fn assign_exact_clonotype_ids(samples: &mut [ParsedRepertoire]) -> Result<usize> {
    #[derive(Clone, Copy)]
    struct IdentityNode {
        sample_idx: u32,
        entry_idx: u32,
        next_same_hash: u32,
    }

    const NO_NODE: u32 = u32::MAX;

    let total_entries = samples
        .iter()
        .map(|sample| sample.clonotypes.entries.len())
        .sum::<usize>();
    if total_entries >= NO_NODE as usize {
        bail!("repertoire contains too many clonotype entries for compact u32 identity IDs");
    }

    let mut hash_heads: HashMap<u64, u32> = HashMap::with_capacity(total_entries);
    let mut identities = Vec::<IdentityNode>::with_capacity(total_entries);
    let mut ids_by_sample = Vec::<Vec<u32>>::with_capacity(samples.len());

    for sample_idx in 0..samples.len() {
        let sample_idx_u32 = u32::try_from(sample_idx)
            .map_err(|_| anyhow!("repertoire contains more than 4 billion samples"))?;
        let store = &samples[sample_idx].clonotypes;
        let mut sample_ids = Vec::with_capacity(store.entries.len());

        for entry_idx in 0..store.entries.len() {
            let entry = &store.entries[entry_idx];
            let key = entry.key.slice(&store.key_bytes);
            let mut cursor = hash_heads.get(&entry.hash).copied().unwrap_or(NO_NODE);
            let mut matched = None;

            while cursor != NO_NODE {
                let node = identities[cursor as usize];
                let representative_store = &samples[node.sample_idx as usize].clonotypes;
                let representative_entry = &representative_store.entries[node.entry_idx as usize];
                if representative_entry
                    .key
                    .slice(&representative_store.key_bytes)
                    == key
                {
                    matched = Some(cursor);
                    break;
                }
                cursor = node.next_same_hash;
            }

            let clonotype_id = if let Some(id) = matched {
                id
            } else {
                let id = u32::try_from(identities.len()).map_err(|_| {
                    anyhow!("repertoire contains more than 4 billion exact clonotypes")
                })?;
                let entry_idx_u32 = u32::try_from(entry_idx)
                    .map_err(|_| anyhow!("sample contains more than 4 billion clonotypes"))?;
                let previous = hash_heads.insert(entry.hash, id).unwrap_or(NO_NODE);
                identities.push(IdentityNode {
                    sample_idx: sample_idx_u32,
                    entry_idx: entry_idx_u32,
                    next_same_hash: previous,
                });
                id
            };
            sample_ids.push(clonotype_id);
        }
        ids_by_sample.push(sample_ids);
    }

    for (sample, sample_ids) in samples.iter_mut().zip(ids_by_sample) {
        for (entry, clonotype_id) in sample.clonotypes.entries.iter_mut().zip(sample_ids) {
            entry.clonotype_id = clonotype_id;
        }
    }

    // Numeric IDs now encode equivalence classes that were established using
    // exact bytes. The original identity arenas are no longer needed by any
    // scientific consumer and are released before sharing/distance work.
    for sample in samples.iter_mut() {
        drop(std::mem::take(&mut sample.clonotypes.key_bytes));
    }

    Ok(identities.len())
}

fn repertoire_sample_summary(
    sample: &ParsedRepertoire,
    subject: &str,
    sharing: (f64, f64, f64),
    matched_depth: usize,
    rarefaction_reps: usize,
) -> CorrectedRepertoireSample {
    let (rr, rs) = rarefy_metrics(
        &sample.counts,
        matched_depth,
        rarefaction_reps,
        stable_seed(&sample.sample_id),
    );
    let total_count = sample.counts.iter().sum::<f64>();
    let shannon = shannon_entropy(&sample.counts);
    let clonality = if sample.counts.len() > 1 {
        1.0 - shannon / (sample.counts.len() as f64).ln()
    } else {
        f64::NAN
    };
    let (private, longitudinal, public) = sharing;
    let convergent_reads = sample
        .clonotypes
        .entries
        .iter()
        .filter(|clonotype| clonotype.convergent)
        .map(|clonotype| clonotype.count)
        .sum::<f64>();
    CorrectedRepertoireSample {
        sample_id: sample.sample_id.clone(),
        group: sample.group.clone(),
        subject: subject.to_string(),
        total_clone_count: total_count,
        observed_richness: sample.counts.len() as f64,
        shannon,
        clonality,
        rarefied_richness: rr,
        rarefied_shannon: rs,
        top_clone_fraction: sample.top_clone_fraction,
        weighted_mean_cdr3_aa_length: sample.weighted_mean_cdr3_aa_length,
        private_fraction: private,
        longitudinal_shared_fraction: longitudinal,
        public_fraction: public,
        convergent_read_fraction: if total_count > 0.0 {
            convergent_reads / total_count
        } else {
            f64::NAN
        },
    }
}

fn repertoire_summary_worker_count(sample_count: usize) -> usize {
    if sample_count == 0 {
        return 1;
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism()
            .map(|value| value.get().min(8).min(sample_count).max(1))
            .unwrap_or(1)
    }
}

/// Release the parsed repertoire after all science consumers have finished.
///
/// Exact identity byte arenas are already released by the numeric-ID
/// compaction pass. The remaining clonotype state is a contiguous entry vector
/// per sample plus small composition maps, so serial destruction releases
/// O(samples) large buffers instead of millions of independently owned
/// Strings. Keeping cleanup serial also avoids the allocator contention seen
/// with the v0.16.16 eight-way destruction experiment.
fn release_repertoire_working_set(samples: Vec<ParsedRepertoire>) {
    drop(samples);
}

pub(crate) fn native_process_memory_summary() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            let rss = proc_status_kib(&status, "VmRSS:");
            let size = proc_status_kib(&status, "VmSize:");
            let hwm = proc_status_kib(&status, "VmHWM:");
            let peak = proc_status_kib(&status, "VmPeak:");
            let swap = proc_status_kib(&status, "VmSwap:");
            let (available, swap_free) = fs::read_to_string("/proc/meminfo")
                .ok()
                .map(|meminfo| {
                    (
                        proc_status_kib(&meminfo, "MemAvailable:"),
                        proc_status_kib(&meminfo, "SwapFree:"),
                    )
                })
                .unwrap_or((None, None));
            return format!(
                "RSS={} VmSize={} VmHWM={} VmPeak={} VmSwap={} MemAvailable={} SwapFree={}",
                format_kib(rss),
                format_kib(size),
                format_kib(hwm),
                format_kib(peak),
                format_kib(swap),
                format_kib(available),
                format_kib(swap_free)
            );
        }
    }
    "RSS/VmSize/swap unavailable on this platform".to_string()
}

pub(crate) fn native_runtime_context_summary() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let executable = std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "unknown".to_string());
        return format!(
            "exe={executable} version={} pid={} thread={:?}",
            env!("CARGO_PKG_VERSION"),
            std::process::id(),
            std::thread::current().id()
        );
    }
    #[cfg(target_arch = "wasm32")]
    {
        format!("exe=wasm version={}", env!("CARGO_PKG_VERSION"))
    }
}

#[cfg(target_os = "linux")]
fn proc_status_kib(status: &str, key: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        let value = line.strip_prefix(key)?.trim();
        value.split_whitespace().next()?.parse::<u64>().ok()
    })
}

#[cfg(target_os = "linux")]
fn format_kib(value: Option<u64>) -> String {
    match value {
        Some(kib) if kib >= 1024 => format!("{:.1} MiB", kib as f64 / 1024.0),
        Some(kib) => format!("{kib} KiB"),
        None => "n/a".to_string(),
    }
}

fn build_repertoire_sample_summaries<F>(
    samples: &[ParsedRepertoire],
    subjects: &[String],
    sharing: &[(f64, f64, f64)],
    matched_depth: usize,
    rarefaction_reps: usize,
    label: &str,
    progress: &mut F,
) -> Result<Vec<CorrectedRepertoireSample>>
where
    F: FnMut(usize, usize, &str),
{
    if samples.len() != subjects.len() || samples.len() != sharing.len() {
        bail!("internal repertoire sample/subject alignment mismatch");
    }
    let n = samples.len();
    let workers = repertoire_summary_worker_count(n);
    if workers == 1 || n < 8 {
        let mut out = Vec::with_capacity(n);
        for (i, sample) in samples.iter().enumerate() {
            let summary = repertoire_sample_summary(
                sample,
                &subjects[i],
                sharing[i],
                matched_depth,
                rarefaction_reps,
            );
            progress(
                350 + ((i + 1) * 300 / n.max(1)),
                1000,
                &format!(
                    "{label}: matched-depth diversity {} ({}/{n})",
                    sample.sample_id,
                    i + 1
                ),
            );
            out.push(summary);
        }
        return Ok(out);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::mpsc;
        let chunk_size = n.div_ceil(workers);
        let (tx, rx) = mpsc::channel::<(usize, CorrectedRepertoireSample)>();
        return std::thread::scope(|scope| -> Result<Vec<CorrectedRepertoireSample>> {
            for start in (0..n).step_by(chunk_size) {
                let end = (start + chunk_size).min(n);
                let tx = tx.clone();
                let sample_chunk = &samples[start..end];
                let subject_chunk = &subjects[start..end];
                let sharing_chunk = &sharing[start..end];
                scope.spawn(move || {
                    for local in 0..sample_chunk.len() {
                        let index = start + local;
                        let summary = repertoire_sample_summary(
                            &sample_chunk[local],
                            &subject_chunk[local],
                            sharing_chunk[local],
                            matched_depth,
                            rarefaction_reps,
                        );
                        if tx.send((index, summary)).is_err() {
                            return;
                        }
                    }
                });
            }
            drop(tx);
            let mut slots = std::iter::repeat_with(|| None)
                .take(n)
                .collect::<Vec<Option<CorrectedRepertoireSample>>>();
            for completed in 0..n {
                let (index, summary) = rx.recv().map_err(|_| {
                    anyhow!("parallel repertoire summary worker stopped unexpectedly")
                })?;
                let sample_id = summary.sample_id.clone();
                slots[index] = Some(summary);
                progress(
                    350 + ((completed + 1) * 300 / n.max(1)),
                    1000,
                    &format!(
                        "{label}: matched-depth diversity {sample_id} ({}/{n}; {workers} workers)",
                        completed + 1
                    ),
                );
            }
            slots
                .into_iter()
                .map(|value| {
                    value.ok_or_else(|| anyhow!("missing parallel repertoire summary row"))
                })
                .collect()
        });
    }

    #[allow(unreachable_code)]
    Ok(Vec::new())
}

pub fn run_corrected_repertoire<F>(
    files: &[DetectedCohortFile],
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    modality: CohortModality,
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<CorrectedRepertoireResult>
where
    F: FnMut(usize, usize, &str),
{
    let label = match modality {
        CohortModality::Bcr => "BCR",
        CohortModality::TcrAlpha => "TCR alpha",
        CohortModality::TcrBeta => "TCR beta",
        _ => bail!("Corrected repertoire analysis supports BCR/TCR only"),
    };
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files.iter().filter(|f| f.modality == modality) {
        let key = normalize_sample_id(&file.sample_id);
        if let Some(first) = chosen.get(&key) {
            warnings.push(format!(
                "Duplicate {label} file for '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    if chosen.is_empty() {
        bail!("No {label} files detected");
    }
    let mut parsed = Vec::new();
    let mut parser_scratch = ParsedClonotypeScratch::default();
    let total = chosen.len();
    progress(
        1,
        1000,
        &format!(
            "{label}: parsing start ({total} candidate sample(s); runtime {}; memory {})",
            native_runtime_context_summary(),
            native_process_memory_summary()
        ),
    );
    for (i, (key, file)) in chosen.iter().enumerate() {
        let group = if comparison.is_case_id(key) {
            comparison.case_label.clone()
        } else if comparison.is_control_id(key) {
            comparison.control_label.clone()
        } else {
            continue;
        };
        let sample_progress = ((i + 1) * 300 / total.max(1)).max(1);
        let sample_started = Instant::now();
        progress(
            sample_progress,
            1000,
            &format!(
                "{label}: parsing {} ({}/{total}) — sample start; retained_samples={}; retained_clonotypes={}; memory {}",
                file.sample_id,
                i + 1,
                parsed.len(),
                parsed
                    .iter()
                    .map(|sample: &ParsedRepertoire| sample.clonotypes.len())
                    .sum::<usize>(),
                native_process_memory_summary()
            ),
        );
        let parsed_sample = parse_repertoire_file_with_audit(
            &file.path,
            &file.sample_id,
            &group,
            modality,
            spec.ambiguous_policy,
            &mut parser_scratch,
            |detail| {
                progress(
                    sample_progress,
                    1000,
                    &format!(
                        "{label}: parsing {} ({}/{total}) — {detail}",
                        file.sample_id,
                        i + 1
                    ),
                );
            },
        );
        match parsed_sample {
            Ok(p) => {
                let sample_clonotypes = p.clonotypes.len();
                let sample_key_bytes = p.clonotypes.key_bytes.len();
                parsed.push(p);
                let retained_clonotypes = parsed
                    .iter()
                    .map(|sample| sample.clonotypes.len())
                    .sum::<usize>();
                let retained_key_bytes = parsed
                    .iter()
                    .map(|sample| sample.clonotypes.key_bytes.len())
                    .sum::<usize>();
                progress(
                    sample_progress,
                    1000,
                    &format!(
                        "{label}: parsing {} ({}/{total}) — sample complete in {:.3}s; sample_clonotypes={sample_clonotypes}; sample_key_bytes={sample_key_bytes}; retained_samples={}; retained_clonotypes={retained_clonotypes}; retained_key_bytes={retained_key_bytes}; memory {}",
                        file.sample_id,
                        i + 1,
                        sample_started.elapsed().as_secs_f64(),
                        parsed.len(),
                        native_process_memory_summary()
                    ),
                );
            }
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    progress(
        295,
        1000,
        &format!(
            "{label}: parser scratch cleanup start (hash_capacity={}; nt_capacity_bytes={}; memory {})",
            parser_scratch.hash_heads.capacity(),
            parser_scratch.nt_bytes.capacity(),
            native_process_memory_summary()
        ),
    );
    let parser_scratch_drop_started = Instant::now();
    drop(parser_scratch);
    progress(
        299,
        1000,
        &format!(
            "{label}: parser scratch cleanup complete in {:.3}s (memory {})",
            parser_scratch_drop_started.elapsed().as_secs_f64(),
            native_process_memory_summary()
        ),
    );
    if parsed.len() < 4 {
        bail!(
            "Only {} usable {label} samples belong to this comparison",
            parsed.len()
        );
    }
    let parsed_clonotypes = parsed
        .iter()
        .map(|sample| sample.clonotypes.len())
        .sum::<usize>();
    progress(
        300,
        1000,
        &format!(
            "{label}: parsing complete ({} usable sample(s); {parsed_clonotypes} clonotypes; memory {})",
            parsed.len(),
            native_process_memory_summary()
        ),
    );
    let sample_ids = parsed
        .iter()
        .map(|p| p.sample_id.clone())
        .collect::<Vec<_>>();
    let design = build_design(metadata, comparison, &sample_ids, spec)?;
    // Keep only complete-case model samples, preserving design order.
    // Move the parsed repertoire payloads into model order rather than cloning
    // their potentially very large clonotype/count maps.  This materially
    // reduces peak memory and allocator pressure for 300+ sample cohorts.
    let mut by_id = parsed
        .into_iter()
        .map(|p| (normalize_sample_id(&p.sample_id), p))
        .collect::<HashMap<_, _>>();
    let mut model_samples = Vec::with_capacity(design.sample_ids.len());
    for id in &design.sample_ids {
        if let Some(p) = by_id.remove(&normalize_sample_id(id)) {
            model_samples.push(p);
        }
    }
    if model_samples.len() != design.sample_ids.len() {
        bail!("Could not align repertoire samples to adjusted metadata design");
    }
    // Any parsed samples excluded from the complete-case model are no longer
    // needed. Release them before constructing the global numeric identity
    // space so they do not contribute to peak repertoire memory.
    drop(by_id);
    drop(sample_ids);

    let total_model_clonotypes = model_samples
        .iter()
        .map(|sample| sample.clonotypes.len())
        .sum::<usize>();
    progress(
        305,
        1000,
        &format!(
            "{label}: assigning exact clonotype IDs across {total_model_clonotypes} model entries (memory {})",
            native_process_memory_summary()
        ),
    );
    let exact_clonotype_count = assign_exact_clonotype_ids(&mut model_samples)?;
    progress(
        320,
        1000,
        &format!(
            "{label}: exact clonotype IDs ready ({exact_clonotype_count} unique exact identities; local identity arenas released; memory {})",
            native_process_memory_summary()
        ),
    );

    let min_total = model_samples
        .iter()
        .map(|p| p.counts.iter().sum::<f64>().round().max(1.0) as usize)
        .min()
        .unwrap_or(1);
    let matched_depth = min_total.min(spec.rarefaction_cap.max(100));
    let subjects = design.subjects.clone();
    progress(
        330,
        1000,
        &format!(
            "{label}: computing clonotype sharing across {} model sample(s) using dense numeric IDs",
            model_samples.len()
        ),
    );
    let sharing = sharing_fractions(
        &model_samples,
        &subjects,
        spec.subject_column.is_some(),
        exact_clonotype_count,
    );
    progress(
        350,
        1000,
        &format!(
            "{label}: matched-depth diversity at depth {matched_depth} ({} replicate(s); {} worker(s))",
            spec.rarefaction_reps,
            repertoire_summary_worker_count(model_samples.len())
        ),
    );
    let summaries = build_repertoire_sample_summaries(
        &model_samples,
        &subjects,
        &sharing,
        matched_depth,
        spec.rarefaction_reps,
        label,
        &mut progress,
    )?;
    let metric_specs: [(&str, fn(&CorrectedRepertoireSample) -> f64); 8] = [
        ("Matched-depth richness", |r| r.rarefied_richness),
        ("Matched-depth Shannon diversity", |r| r.rarefied_shannon),
        ("Clonality", |r| r.clonality),
        ("Top clone fraction", |r| r.top_clone_fraction),
        ("Weighted mean CDR3 AA length", |r| {
            r.weighted_mean_cdr3_aa_length
        }),
        ("Private clonotype read fraction", |r| r.private_fraction),
        ("Longitudinally shared clonotype read fraction", |r| {
            r.longitudinal_shared_fraction
        }),
        ("Public clonotype read fraction", |r| r.public_fraction),
    ];
    let mut metric_tests = Vec::new();
    for (name, f) in metric_specs {
        let y = summaries.iter().map(f).collect::<Vec<_>>();
        if let Some(test) = make_adjusted_test(name.to_string(), String::new(), &y, &y, &design) {
            metric_tests.push(test);
        }
    }
    if modality != CohortModality::Bcr {
        let y = summaries
            .iter()
            .map(|r| r.convergent_read_fraction)
            .collect::<Vec<_>>();
        if let Some(test) = make_adjusted_test(
            "AA convergence read fraction".to_string(),
            String::new(),
            &y,
            &y,
            &design,
        ) {
            metric_tests.push(test);
        }
    }
    apply_bh_adjusted(&mut metric_tests);

    progress(
        670,
        1000,
        &format!("{label}: adjusted diversity metrics complete"),
    );

    let families: Vec<&'static str> = match modality {
        CohortModality::Bcr => vec!["Isotype", "V gene", "D gene", "J gene"],
        _ => vec!["V gene", "J gene", "V-J pair"],
    };
    progress(
        700,
        1000,
        &format!(
            "{label}: building Morisita-Horn + Jaccard distance matrices ({} samples)",
            model_samples.len()
        ),
    );
    // Build the shared clonotype inverted index once for both distance metrics.
    // The older path rebuilt the same large index independently for Morisita
    // and Jaccard, and recomputed Morisita once more for clustering.
    let (morisita_dist, jaccard_dist) =
        repertoire_distance_matrices(&model_samples, exact_clonotype_count);
    progress(
        780,
        1000,
        &format!(
            "{label}: composition + ordination/PERMANOVA tasks ({} permutation(s))",
            spec.permutations
        ),
    );

    let mor_title = format!("{label} Morisita-Horn PCoA");
    let jac_title = format!("{label} Jaccard PCoA");
    let dendro_title = format!("{label} hierarchical clustering — Morisita-Horn distance");

    #[cfg(not(target_arch = "wasm32"))]
    let (compositions, morisita, jaccard, dendrogram) = std::thread::scope(|scope| -> Result<_> {
        let samples_ref = &model_samples;
        let design_ref = &design;
        let spec_ref = spec;
        let mut composition_handles = Vec::with_capacity(families.len());
        for &family in &families {
            composition_handles
                .push(scope.spawn(move || {
                    composition_analysis(family, samples_ref, design_ref, spec_ref)
                }));
        }
        let mor_handle = scope.spawn(|| {
            pcoa_from_distance(
                &morisita_dist,
                &design,
                &mor_title,
                spec.permutations,
                stable_seed(&mor_title),
            )
        });
        let jac_handle = scope.spawn(|| {
            pcoa_from_distance(
                &jaccard_dist,
                &design,
                &jac_title,
                spec.permutations,
                stable_seed(&jac_title),
            )
        });
        let dendro_handle = scope.spawn(|| {
            average_linkage_dendrogram(
                &morisita_dist,
                &design,
                &dendro_title,
                "1 - Morisita-Horn similarity",
            )
        });

        let mut compositions = Vec::new();
        for handle in composition_handles {
            if let Some(result) = handle
                .join()
                .map_err(|_| anyhow!("parallel repertoire composition worker panicked"))?
            {
                compositions.push(result);
            }
        }
        let morisita = mor_handle
            .join()
            .map_err(|_| anyhow!("parallel Morisita-Horn ordination worker panicked"))?;
        let jaccard = jac_handle
            .join()
            .map_err(|_| anyhow!("parallel Jaccard ordination worker panicked"))?;
        let dendrogram = dendro_handle
            .join()
            .map_err(|_| anyhow!("parallel repertoire dendrogram worker panicked"))?;
        Ok((compositions, morisita, jaccard, dendrogram))
    })?;

    #[cfg(target_arch = "wasm32")]
    let (compositions, morisita, jaccard, dendrogram) = {
        let mut compositions = Vec::new();
        for family in families {
            if let Some(comp) = composition_analysis(family, &model_samples, &design, spec) {
                compositions.push(comp);
            }
        }
        let morisita = pcoa_from_distance(
            &morisita_dist,
            &design,
            &mor_title,
            spec.permutations,
            stable_seed(&mor_title),
        );
        let jaccard = pcoa_from_distance(
            &jaccard_dist,
            &design,
            &jac_title,
            spec.permutations,
            stable_seed(&jac_title),
        );
        let dendrogram = average_linkage_dendrogram(
            &morisita_dist,
            &design,
            &dendro_title,
            "1 - Morisita-Horn similarity",
        );
        (compositions, morisita, jaccard, dendrogram)
    };
    if spec.subject_column.is_none() {
        warnings.push("No subject ID column was selected: public/private sharing is sample-based and PERMANOVA permutations are unrestricted. Select a patient/subject column for longitudinally aware results.".to_string());
    }
    let files_used = model_samples.len();
    let raw_clonotypes = model_samples
        .iter()
        .map(|sample| sample.clonotypes.len())
        .sum::<usize>();
    let raw_feature_entries = model_samples
        .iter()
        .flat_map(|sample| sample.feature_maps.values())
        .map(|map| map.len())
        .sum::<usize>();
    let result = CorrectedRepertoireResult {
        modality: label.to_string(),
        files_used,
        matched_depth,
        rarefaction_reps: spec.rarefaction_reps,
        ambiguous_policy: spec.ambiguous_policy,
        sample_summaries: summaries,
        metric_tests,
        compositions,
        morisita_horn: morisita,
        jaccard,
        dendrogram,
        warnings,
    };

    // Do not report completion until the raw parsed working set has actually
    // been released.  Previously the final callback happened before implicit
    // local destruction, so a very large TCR-beta cleanup could look like a
    // multi-hour post-completion hang.
    progress(
        960,
        1000,
        &format!(
            "{label}: releasing parsed repertoire working set ({files_used} samples; {raw_clonotypes} clonotypes; {raw_feature_entries} composition entries; dense numeric IDs; identity arenas already released; serial cleanup; memory {})",
            native_process_memory_summary()
        ),
    );
    release_repertoire_working_set(model_samples);
    progress(
        985,
        1000,
        &format!(
            "{label}: releasing parsed repertoire working set complete (memory {})",
            native_process_memory_summary()
        ),
    );
    progress(
        1000,
        1000,
        &format!("{label}: corrected repertoire analysis complete"),
    );
    Ok(result)
}

pub fn run_corrected_isotype<F>(
    files: &[DetectedCohortFile],
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<CorrectedIsotypeResult>
where
    F: FnMut(usize, usize, &str),
{
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files
        .iter()
        .filter(|f| f.modality == CohortModality::BcrIsotype)
    {
        let key = normalize_sample_id(&file.sample_id);
        if let Some(first) = chosen.get(&key) {
            warnings.push(format!(
                "Duplicate BCR-isotype file for '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let mut maps = Vec::new();
    let total = chosen.len();
    for (i, (key, file)) in chosen.iter().enumerate() {
        let group = if comparison.is_case_id(key) {
            comparison.case_label.clone()
        } else if comparison.is_control_id(key) {
            comparison.control_label.clone()
        } else {
            continue;
        };
        progress(
            i + 1,
            total.max(1),
            &format!("BCR isotype: {}", file.sample_id),
        );
        match parse_isotype_file(&file.path, spec.exclude_undetermined_isotype) {
            Ok(map) => maps.push((file.sample_id.clone(), group, map)),
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if maps.len() < 4 {
        bail!("Too few BCR-isotype samples for corrected analysis");
    }
    let ids = maps.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    let design = build_design(metadata, comparison, &ids, spec)?;
    let map_by_id = maps
        .into_iter()
        .map(|x| (normalize_sample_id(&x.0), x))
        .collect::<HashMap<_, _>>();
    let aligned = design
        .sample_ids
        .iter()
        .filter_map(|id| map_by_id.get(&normalize_sample_id(id)).cloned())
        .collect::<Vec<_>>();
    if aligned.len() != design.sample_ids.len() {
        bail!("Could not align isotype samples to adjusted design");
    }
    let samples = aligned
        .into_iter()
        .map(|(sample_id, group, feature_map)| ParsedRepertoire {
            sample_id,
            group,
            counts: Vec::new(),
            clonotypes: ParsedClonotypeStore::default(),
            feature_maps: BTreeMap::from([("Isotype".to_string(), feature_map)]),
            top_clone_fraction: f64::NAN,
            weighted_mean_cdr3_aa_length: f64::NAN,
        })
        .collect::<Vec<_>>();
    let comp = composition_analysis("Isotype", &samples, &design, spec)
        .ok_or_else(|| anyhow!("No isotype features passed the composition filter"))?;
    Ok(CorrectedIsotypeResult {
        files_used: samples.len(),
        determined_only: spec.exclude_undetermined_isotype,
        retained_feature_names: comp.retained_feature_names,
        tests: comp.tests,
        ordination: comp.ordination,
        dendrogram: comp.dendrogram,
        warnings,
    })
}

pub fn run_corrected_immune<F>(
    files: &[DetectedCohortFile],
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<CorrectedImmuneResult>
where
    F: FnMut(usize, usize, &str),
{
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files
        .iter()
        .filter(|f| f.modality == CohortModality::ImmuneInfiltrate)
    {
        let key = normalize_sample_id(&file.sample_id);
        if let Some(first) = chosen.get(&key) {
            warnings.push(format!(
                "Duplicate immune file for '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let total = chosen.len();
    let mut maps = Vec::new();
    for (i, (key, file)) in chosen.iter().enumerate() {
        let group = if comparison.is_case_id(key) {
            comparison.case_label.clone()
        } else if comparison.is_control_id(key) {
            comparison.control_label.clone()
        } else {
            continue;
        };
        progress(
            i + 1,
            total.max(1),
            &format!("Immune infiltrate: {}", file.sample_id),
        );
        match parse_immune_file(&file.path) {
            Ok(map) => maps.push((file.sample_id.clone(), group, map)),
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if maps.len() < 4 {
        bail!("Too few immune-infiltrate samples for corrected analysis");
    }
    let ids = maps.iter().map(|x| x.0.clone()).collect::<Vec<_>>();
    let design = build_design(metadata, comparison, &ids, spec)?;
    let map_by_id = maps
        .into_iter()
        .map(|x| (normalize_sample_id(&x.0), x))
        .collect::<HashMap<_, _>>();
    let aligned = design
        .sample_ids
        .iter()
        .filter_map(|id| map_by_id.get(&normalize_sample_id(id)).cloned())
        .collect::<Vec<_>>();
    if aligned.len() != design.sample_ids.len() {
        bail!("Could not align immune samples to adjusted design");
    }
    let features = aligned
        .iter()
        .flat_map(|(_, _, m)| m.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut tests = Vec::new();
    let mut rows_for_pca = Vec::new();
    for feature in &features {
        let y = aligned
            .iter()
            .map(|(_, _, m)| m.get(feature).copied().unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        if let Some(test) = make_adjusted_test(feature.clone(), String::new(), &y, &y, &design) {
            tests.push(test);
        }
        let m = mean_finite(&y);
        let sd = sd_finite(&y, m);
        if sd > 0.0 {
            rows_for_pca.push(
                y.iter()
                    .map(|v| if v.is_finite() { (v - m) / sd } else { 0.0 })
                    .collect::<Vec<_>>(),
            );
        }
    }
    apply_bh_adjusted(&mut tests);
    let distances = euclidean_distance_from_rows(&rows_for_pca);
    let mut pca = pca_from_rows(
        &rows_for_pca,
        &design,
        "Immune-infiltrate PCA (cell-type z-scores)",
        spec.permutations,
    );
    if let Some(ord) = &mut pca {
        let (f, p) = permanova(
            &distances,
            &design.group_case,
            &design.subjects,
            spec.permutations,
            0xBADC0FFE,
        );
        ord.permanova_f = f;
        ord.permanova_p = p;
        ord.permutations = spec.permutations;
    }
    let dendrogram = average_linkage_dendrogram(
        &distances,
        &design,
        "Immune-infiltrate hierarchical clustering — cell-type z-scores",
        "Euclidean distance",
    );
    Ok(CorrectedImmuneResult {
        files_used: aligned.len(),
        tests,
        pca,
        dendrogram,
        warnings,
    })
}

/// Build the same normalized-expression sample distance used by the corrected
/// PCA/dendrogram layer, but over an arbitrary QC-eligible sample set rather
/// than a Case/Control comparison. This powers the dedicated longitudinal
/// patient-signature analysis without inventing a synthetic phenotype group.
pub(crate) fn run_unsupervised_expression_distance<F>(
    merged_tsv: &Path,
    eligible_ids: &HashSet<String>,
    source_metric: &str,
    feature_label: &str,
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<UnsupervisedDistanceMatrix>
where
    F: FnMut(usize, usize, &str),
{
    let mut header_reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)
        .with_context(|| format!("Could not open {}", merged_tsv.display()))?;
    let headers = header_reader.headers()?.clone();
    let suffix = format!(" {source_metric}");
    let mut header_cols: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let mut warnings = Vec::new();
    for (idx, header) in headers.iter().enumerate().skip(2) {
        let Some(sample) = header.strip_suffix(&suffix) else {
            continue;
        };
        let key = normalize_sample_id(sample);
        if !eligible_ids.contains(&key) {
            continue;
        }
        if let Some((first, _)) = header_cols.get(&key) {
            warnings.push(format!(
                "Duplicate normalized {feature_label} sample column '{sample}' matches '{first}'; using the first column"
            ));
        } else {
            header_cols.insert(key, (sample.to_string(), idx));
        }
    }
    if header_cols.len() < 2 {
        bail!(
            "Merged {feature_label} table has fewer than two QC-eligible '{source_metric}' sample columns"
        );
    }
    let sample_ids = header_cols
        .values()
        .map(|(sample, _)| sample.clone())
        .collect::<Vec<_>>();
    let columns = header_cols
        .values()
        .map(|(_, idx)| *idx)
        .collect::<Vec<_>>();
    let file_bytes = fs::metadata(merged_tsv)
        .with_context(|| format!("Could not stat {}", merged_tsv.display()))?
        .len()
        .max(1);
    let units = usize::try_from(file_bytes).unwrap_or(usize::MAX / 4).max(1);
    let total_work = units.saturating_mul(2).max(1);

    progress(
        0,
        total_work,
        &format!("{feature_label}: estimating count size factors"),
    );
    let mut medians = vec![P2Median::new(); columns.len()];
    let mut pass1 = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    let mut record = csv::StringRecord::new();
    let mut rows_seen = 0usize;
    while pass1.read_record(&mut record)? {
        let counts = values_at_columns_with_zero(&record, &columns);
        let mut positive_count = 0usize;
        let mut positive_log_sum = 0.0;
        for count in &counts {
            if *count > 0.0 {
                positive_count += 1;
                positive_log_sum += count.ln();
            }
        }
        if positive_count >= 2 {
            let gmean = (positive_log_sum / positive_count as f64).exp();
            if gmean > 0.0 && gmean.is_finite() {
                for (j, count) in counts.iter().enumerate() {
                    if *count > 0.0 {
                        medians[j].push(*count / gmean);
                    }
                }
            }
        }
        rows_seen += 1;
        if rows_seen % 2000 == 0 {
            let done = usize::try_from(pass1.position().byte())
                .unwrap_or(units)
                .min(units);
            progress(
                done,
                total_work,
                &format!("{feature_label}: estimating count size factors ({rows_seen} rows)"),
            );
        }
    }
    let mut size_factors = medians
        .iter()
        .map(|median| {
            median
                .value()
                .filter(|value| *value > 0.0 && value.is_finite())
                .unwrap_or(1.0)
        })
        .collect::<Vec<_>>();
    let sf_gmean = (size_factors.iter().map(|value| value.ln()).sum::<f64>()
        / size_factors.len() as f64)
        .exp();
    if sf_gmean.is_finite() && sf_gmean > 0.0 {
        for size_factor in &mut size_factors {
            *size_factor /= sf_gmean;
        }
    }
    progress(
        units,
        total_work,
        &format!("{feature_label}: count size factors complete"),
    );

    let min_present = ((sample_ids.len() as f64 * spec.min_fraction_samples).ceil() as usize)
        .max(3)
        .min(sample_ids.len());
    let mut top_variable = Vec::<VariableRow>::new();
    let mut pass2 = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    let mut retained_rows = 0usize;
    let mut filtered_rows = 0usize;
    let mut record = csv::StringRecord::new();
    while pass2.read_record(&mut record)? {
        let counts = values_at_columns_with_zero(&record, &columns);
        let normalized = counts
            .iter()
            .zip(size_factors.iter())
            .map(|(count, size_factor)| count / size_factor.max(1e-12))
            .collect::<Vec<_>>();
        if normalized
            .iter()
            .filter(|value| **value >= spec.min_count)
            .count()
            < min_present
        {
            filtered_rows += 1;
            continue;
        }
        let values = normalized
            .iter()
            .map(|value| (value + 0.5).log2())
            .collect::<Vec<_>>();
        let variance = sample_variance(&values);
        if variance.is_finite() {
            top_variable.push(VariableRow { variance, values });
            retained_rows += 1;
            if top_variable.len() > 700 {
                trim_variable_rows(&mut top_variable, 350);
            }
        }
        if (retained_rows + filtered_rows) % 2000 == 0 {
            let done = usize::try_from(pass2.position().byte())
                .unwrap_or(units)
                .min(units);
            progress(
                units.saturating_add(done),
                total_work,
                &format!("{feature_label}: selecting top variable normalized features"),
            );
        }
    }
    top_variable.sort_by(|a, b| b.variance.total_cmp(&a.variance));
    top_variable.truncate(300);
    if top_variable.is_empty() {
        bail!("No {feature_label} features passed the clustering support filter");
    }
    if top_variable.len() < 50 {
        warnings.push(format!(
            "Only {} {feature_label} features passed the clustering support filter",
            top_variable.len()
        ));
    }
    let rows = top_variable
        .into_iter()
        .map(|row| row.values)
        .collect::<Vec<_>>();
    let distances = euclidean_distance_from_rows(&rows);
    progress(
        total_work,
        total_work,
        &format!(
            "{feature_label}: longitudinal clustering distance complete ({} samples; {} top variable features)",
            sample_ids.len(),
            rows.len()
        ),
    );
    Ok(UnsupervisedDistanceMatrix {
        sample_ids,
        distances,
        distance_label: "Euclidean distance on top variable normalized log2 counts".to_string(),
        warnings,
    })
}

/// Build abundance-weighted repertoire distances for all QC-eligible samples.
/// Parsing, exact clonotype identity assignment, and Morisita-Horn semantics are
/// identical to the corrected repertoire engine; only phenotype modeling and
/// rarefaction summaries are skipped.
pub(crate) fn run_unsupervised_repertoire_distance<F>(
    files: &[DetectedCohortFile],
    eligible_ids: &HashSet<String>,
    modality: CohortModality,
    policy: AmbiguousCallPolicy,
    mut progress: F,
) -> Result<UnsupervisedDistanceMatrix>
where
    F: FnMut(usize, usize, &str),
{
    let label = match modality {
        CohortModality::Bcr => "BCR",
        CohortModality::TcrAlpha => "TCR alpha",
        CohortModality::TcrBeta => "TCR beta",
        _ => bail!("Longitudinal repertoire clustering supports BCR/TCR only"),
    };
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files.iter().filter(|file| file.modality == modality) {
        let key = normalize_sample_id(&file.sample_id);
        if !eligible_ids.contains(&key) {
            continue;
        }
        if let Some(first) = chosen.get(&key) {
            warnings.push(format!(
                "Duplicate {label} file for '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    if chosen.len() < 2 {
        bail!("Fewer than two QC-eligible {label} files were detected");
    }
    let total = chosen.len();
    let mut parsed = Vec::with_capacity(total);
    let mut scratch = ParsedClonotypeScratch::default();
    for (i, file) in chosen.values().enumerate() {
        progress(
            i,
            total.saturating_add(2),
            &format!(
                "{label}: longitudinal parsing {} ({}/{total})",
                file.sample_id,
                i + 1
            ),
        );
        match parse_repertoire_file_with_audit(
            &file.path,
            &file.sample_id,
            "Eligible",
            modality,
            policy,
            &mut scratch,
            |_| {},
        ) {
            Ok(sample) => parsed.push(sample),
            Err(error) => warnings.push(format!("{}: {error:#}", file.path.display())),
        }
    }
    drop(scratch);
    if parsed.len() < 2 {
        bail!("Fewer than two usable QC-eligible {label} repertoires remained after parsing");
    }
    progress(
        total,
        total.saturating_add(2),
        &format!("{label}: assigning exact clonotype identities"),
    );
    let exact_clonotypes = assign_exact_clonotype_ids(&mut parsed)?;
    progress(
        total.saturating_add(1),
        total.saturating_add(2),
        &format!("{label}: building Morisita-Horn distances"),
    );
    let (distances, _) = repertoire_distance_matrices(&parsed, exact_clonotypes);
    let sample_ids = parsed
        .iter()
        .map(|sample| sample.sample_id.clone())
        .collect::<Vec<_>>();
    release_repertoire_working_set(parsed);
    progress(
        total.saturating_add(2),
        total.saturating_add(2),
        &format!("{label}: longitudinal Morisita-Horn distance complete"),
    );
    Ok(UnsupervisedDistanceMatrix {
        sample_ids,
        distances,
        distance_label: "1 - Morisita-Horn similarity".to_string(),
        warnings,
    })
}

pub(crate) fn run_unsupervised_isotype_distance<F>(
    files: &[DetectedCohortFile],
    eligible_ids: &HashSet<String>,
    spec: &CorrectedModelSpec,
    mut progress: F,
) -> Result<UnsupervisedDistanceMatrix>
where
    F: FnMut(usize, usize, &str),
{
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files
        .iter()
        .filter(|file| file.modality == CohortModality::BcrIsotype)
    {
        let key = normalize_sample_id(&file.sample_id);
        if !eligible_ids.contains(&key) {
            continue;
        }
        if let Some(first) = chosen.get(&key) {
            warnings.push(format!(
                "Duplicate BCR-isotype file for '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let total = chosen.len();
    let mut maps = Vec::new();
    for (i, file) in chosen.values().enumerate() {
        progress(
            i + 1,
            total.max(1),
            &format!("BCR isotype: {}", file.sample_id),
        );
        match parse_isotype_file(&file.path, spec.exclude_undetermined_isotype) {
            Ok(map) => maps.push((file.sample_id.clone(), map)),
            Err(error) => warnings.push(format!("{}: {error:#}", file.path.display())),
        }
    }
    if maps.len() < 2 {
        bail!("Fewer than two usable QC-eligible BCR-isotype samples were detected");
    }
    let all_features = maps
        .iter()
        .flat_map(|(_, map)| map.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut retained = Vec::new();
    for feature in all_features {
        let values = maps
            .iter()
            .map(|(_, map)| map.get(&feature).copied().unwrap_or(0.0))
            .collect::<Vec<_>>();
        let avg = mean(&values);
        let prevalence =
            values.iter().filter(|value| **value > 0.0).count() as f64 / values.len().max(1) as f64;
        if avg >= spec.composition_min_mean && prevalence >= spec.composition_min_prevalence {
            retained.push(feature);
        }
    }
    if retained.len() < 2 {
        bail!("Fewer than two BCR-isotype features passed the composition filter");
    }
    let pseudo = spec.clr_pseudocount.max(1e-12);
    let mut clr_by_sample = Vec::with_capacity(maps.len());
    for (_, map) in &maps {
        let logs = retained
            .iter()
            .map(|feature| (map.get(feature).copied().unwrap_or(0.0) + pseudo).ln())
            .collect::<Vec<_>>();
        let center = mean(&logs);
        clr_by_sample.push(logs.into_iter().map(|value| value - center).collect());
    }
    Ok(UnsupervisedDistanceMatrix {
        sample_ids: maps.into_iter().map(|(sample, _)| sample).collect(),
        distances: euclidean_distance_from_sample_rows(&clr_by_sample),
        distance_label: "Aitchison distance".to_string(),
        warnings,
    })
}

pub(crate) fn run_unsupervised_immune_distance<F>(
    files: &[DetectedCohortFile],
    eligible_ids: &HashSet<String>,
    mut progress: F,
) -> Result<UnsupervisedDistanceMatrix>
where
    F: FnMut(usize, usize, &str),
{
    let mut chosen: BTreeMap<String, &DetectedCohortFile> = BTreeMap::new();
    let mut warnings = Vec::new();
    for file in files
        .iter()
        .filter(|file| file.modality == CohortModality::ImmuneInfiltrate)
    {
        let key = normalize_sample_id(&file.sample_id);
        if !eligible_ids.contains(&key) {
            continue;
        }
        if let Some(first) = chosen.get(&key) {
            warnings.push(format!(
                "Duplicate immune file for '{}': using {} and ignoring {}",
                file.sample_id,
                first.path.display(),
                file.path.display()
            ));
        } else {
            chosen.insert(key, file);
        }
    }
    let total = chosen.len();
    let mut maps = Vec::new();
    for (i, file) in chosen.values().enumerate() {
        progress(
            i + 1,
            total.max(1),
            &format!("Immune infiltrate: {}", file.sample_id),
        );
        match parse_immune_file(&file.path) {
            Ok(map) => maps.push((file.sample_id.clone(), map)),
            Err(error) => warnings.push(format!("{}: {error:#}", file.path.display())),
        }
    }
    if maps.len() < 2 {
        bail!("Fewer than two usable QC-eligible immune-infiltrate samples were detected");
    }
    let features = maps
        .iter()
        .flat_map(|(_, map)| map.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for feature in features {
        let values = maps
            .iter()
            .map(|(_, map)| map.get(&feature).copied().unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        let center = mean_finite(&values);
        let sd = sd_finite(&values, center);
        if sd > 0.0 {
            rows.push(
                values
                    .iter()
                    .map(|value| {
                        if value.is_finite() {
                            (value - center) / sd
                        } else {
                            0.0
                        }
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
    if rows.is_empty() {
        bail!("No variable immune-infiltrate signatures were available for clustering");
    }
    Ok(UnsupervisedDistanceMatrix {
        sample_ids: maps.into_iter().map(|(sample, _)| sample).collect(),
        distances: euclidean_distance_from_rows(&rows),
        distance_label: "Euclidean distance on standardized immune-signature scores".to_string(),
        warnings,
    })
}

fn composition_analysis(
    family: &str,
    samples: &[ParsedRepertoire],
    design: &DesignMatrix,
    spec: &CorrectedModelSpec,
) -> Option<CorrectedCompositionResult> {
    // Borrow the per-sample composition maps rather than cloning every V/D/J/
    // isotype feature map for each analysis family. These maps can be sizeable
    // across a few hundred samples, and the analysis only reads them.
    let maps = samples
        .iter()
        .map(|p| p.feature_maps.get(family))
        .collect::<Vec<_>>();
    let all_features = maps
        .iter()
        .filter_map(|m| *m)
        .flat_map(|m| m.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut retained = Vec::new();
    for feature in all_features {
        let vals = maps
            .iter()
            .map(|m| {
                (*m).and_then(|map| map.get(&feature))
                    .copied()
                    .unwrap_or(0.0)
            })
            .collect::<Vec<_>>();
        let avg = mean(&vals);
        let prevalence =
            vals.iter().filter(|v| **v > 0.0).count() as f64 / vals.len().max(1) as f64;
        if avg >= spec.composition_min_mean && prevalence >= spec.composition_min_prevalence {
            retained.push(feature);
        }
    }
    if retained.len() < 2 {
        return None;
    }
    let pseudo = spec.clr_pseudocount.max(1e-12);
    let mut clr_by_sample = vec![vec![0.0; retained.len()]; maps.len()];
    for (i, map) in maps.iter().enumerate() {
        let logs = retained
            .iter()
            .map(|f| {
                ((*map)
                    .and_then(|values| values.get(f))
                    .copied()
                    .unwrap_or(0.0)
                    + pseudo)
                    .ln()
            })
            .collect::<Vec<_>>();
        let center = mean(&logs);
        for j in 0..retained.len() {
            clr_by_sample[i][j] = logs[j] - center;
        }
    }
    let mut tests = Vec::new();
    for (j, feature) in retained.iter().enumerate() {
        let y = clr_by_sample.iter().map(|row| row[j]).collect::<Vec<_>>();
        let display = maps
            .iter()
            .map(|m| {
                (*m).and_then(|map| map.get(feature))
                    .copied()
                    .unwrap_or(0.0)
            })
            .collect::<Vec<_>>();
        if let Some(test) =
            make_adjusted_test(feature.clone(), family.to_string(), &y, &display, design)
        {
            tests.push(test);
        }
    }
    apply_bh_adjusted(&mut tests);
    let distances = euclidean_distance_from_sample_rows(&clr_by_sample);
    let ord = pcoa_from_distance(
        &distances,
        design,
        &format!("{family} Aitchison PCoA"),
        spec.permutations,
        stable_seed(family),
    );
    let dendrogram = average_linkage_dendrogram(
        &distances,
        design,
        &format!("{family} hierarchical clustering — Aitchison distance"),
        "Aitchison distance",
    );
    Some(CorrectedCompositionResult {
        family: family.to_string(),
        retained_features: retained.len(),
        retained_feature_names: retained,
        tests,
        ordination: ord,
        dendrogram,
    })
}

fn repertoire_distance_matrices(
    samples: &[ParsedRepertoire],
    clonotype_universe: usize,
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let n = samples.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }

    #[derive(Clone, Copy)]
    struct OccurrenceNode {
        sample_idx: u32,
        next: u32,
        p: f64,
    }
    const NO_NODE: u32 = u32::MAX;

    let total_clonotypes = samples.iter().map(|sample| sample.clonotypes.len()).sum();
    // Dense numeric IDs let the distance pass use one contiguous head array.
    // This avoids hashing millions of variable-length clonotype byte slices a
    // second time after exact identity has already been established.
    let mut heads = vec![NO_NODE; clonotype_universe];
    let mut occurrences = Vec::<OccurrenceNode>::with_capacity(total_clonotypes);
    let mut sums_sq = vec![0.0; n];
    let mut richness = vec![0usize; n];
    let mut dotm = vec![vec![0.0; n]; n];
    let mut shared = vec![vec![0usize; n]; n];

    for (i, sample) in samples.iter().enumerate() {
        let total = sample
            .clonotypes
            .entries
            .iter()
            .map(|clonotype| clonotype.count)
            .sum::<f64>();
        if total <= 0.0 {
            continue;
        }
        richness[i] = sample.clonotypes.len();
        for clonotype in &sample.clonotypes.entries {
            let clonotype_id = clonotype.clonotype_id as usize;
            assert!(
                clonotype.clonotype_id != UNRESOLVED_CLONOTYPE_ID
                    && clonotype_id < clonotype_universe,
                "repertoire distance matrix received an unresolved clonotype ID"
            );
            let p = clonotype.count / total;
            sums_sq[i] += p * p;
            dotm[i][i] += p * p;
            shared[i][i] += 1;

            let mut cursor = heads[clonotype_id];
            while cursor != NO_NODE {
                let node = occurrences[cursor as usize];
                let j = node.sample_idx as usize;
                let dot = p * node.p;
                dotm[i][j] += dot;
                dotm[j][i] += dot;
                shared[i][j] += 1;
                shared[j][i] += 1;
                cursor = node.next;
            }

            let index =
                u32::try_from(occurrences.len()).expect("repertoire occurrence index exceeds u32");
            let previous = heads[clonotype_id];
            heads[clonotype_id] = index;
            occurrences.push(OccurrenceNode {
                sample_idx: u32::try_from(i).expect("repertoire sample index exceeds u32"),
                next: previous,
                p,
            });
        }
    }

    let mut morisita = vec![vec![0.0; n]; n];
    let mut jaccard = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in i..n {
            let den = sums_sq[i] + sums_sq[j];
            let morisita_sim = if den > 0.0 {
                (2.0 * dotm[i][j] / den).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let union = richness[i] + richness[j] - shared[i][j];
            let jaccard_sim = if union > 0 {
                shared[i][j] as f64 / union as f64
            } else {
                0.0
            };
            let m = (1.0 - morisita_sim).max(0.0);
            let jv = (1.0 - jaccard_sim).max(0.0);
            morisita[i][j] = m;
            morisita[j][i] = m;
            jaccard[i][j] = jv;
            jaccard[j][i] = jv;
        }
    }
    (morisita, jaccard)
}

fn sharing_fractions(
    samples: &[ParsedRepertoire],
    subjects: &[String],
    subject_aware: bool,
    clonotype_universe: usize,
) -> Vec<(f64, f64, f64)> {
    #[derive(Clone, Copy)]
    struct Prevalence {
        first_sample: u32,
        n_samples: u32,
        multiple_subjects: bool,
    }

    const NO_SAMPLE: u32 = u32::MAX;
    let mut prevalence = vec![
        Prevalence {
            first_sample: NO_SAMPLE,
            n_samples: 0,
            multiple_subjects: false,
        };
        clonotype_universe
    ];

    // Exact equality was already resolved once in assign_exact_clonotype_ids.
    // Sharing is therefore a pair of linear integer-indexed scans with no
    // string hashing, string allocation, or multi-million-entry HashMap.
    for (i, sample) in samples.iter().enumerate() {
        for clonotype in &sample.clonotypes.entries {
            let clonotype_id = clonotype.clonotype_id as usize;
            assert!(
                clonotype.clonotype_id != UNRESOLVED_CLONOTYPE_ID
                    && clonotype_id < clonotype_universe,
                "repertoire sharing received an unresolved clonotype ID"
            );
            let entry = &mut prevalence[clonotype_id];
            if entry.n_samples == 0 {
                entry.first_sample = u32::try_from(i).expect("repertoire sample index exceeds u32");
                entry.n_samples = 1;
            } else {
                entry.n_samples += 1;
                let first_sample = entry.first_sample as usize;
                if subjects[first_sample] != subjects[i] {
                    entry.multiple_subjects = true;
                }
            }
        }
    }

    samples
        .iter()
        .enumerate()
        .map(|(i, sample)| {
            let total = sample
                .clonotypes
                .entries
                .iter()
                .map(|clonotype| clonotype.count)
                .sum::<f64>();
            if total <= 0.0 {
                return (f64::NAN, f64::NAN, f64::NAN);
            }
            let mut private = 0.0;
            let mut longitudinal = 0.0;
            let mut public = 0.0;
            for clonotype in &sample.clonotypes.entries {
                let count = clonotype.count;
                let entry = &prevalence[clonotype.clonotype_id as usize];
                if entry.multiple_subjects {
                    public += count;
                } else if subject_aware
                    && entry.n_samples >= 2
                    && subjects[entry.first_sample as usize] == subjects[i]
                {
                    longitudinal += count;
                } else {
                    private += count;
                }
            }
            (private / total, longitudinal / total, public / total)
        })
        .collect()
}

pub(crate) fn parse_repertoire_feature_maps(
    path: &Path,
    modality: CohortModality,
    policy: AmbiguousCallPolicy,
) -> Result<BTreeMap<String, HashMap<String, f64>>> {
    if !matches!(
        modality,
        CohortModality::Bcr | CohortModality::TcrAlpha | CohortModality::TcrBeta
    ) {
        bail!("Repertoire feature-map parsing supports BCR/TCR only");
    }

    // This helper is used only when a repertoire composition family has
    // significant hits.  Do not call `read_first_sheet`: cloning every XLSX
    // worksheet cell into a second Vec<Vec<Data>> would recreate the large
    // temporary/destructor pressure that the native repertoire cleanup work
    // deliberately removed.  Consume Calamine's materialized Range in place,
    // accumulate only the small composition maps, then release the Range before
    // the workbook/shared-string container just like the main native parser.
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("Could not open {}", path.display()))?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("Workbook has no worksheets"))?;
    let range = workbook.worksheet_range(&sheet)?;
    let mut rows = range.rows();
    let header = rows.next().ok_or_else(|| anyhow!("Worksheet is empty"))?;
    let headers = header.iter().map(cell_text).collect::<Vec<_>>();
    let columns = header_map(&headers);
    let count_idx =
        find_header(&columns, &["Clone Count"]).ok_or_else(|| anyhow!("Missing Clone Count"))?;
    let v_idx = find_header(&columns, &["Top V Hits"]);
    let d_idx = find_header(&columns, &["Top D Hits"]);
    let j_idx = find_header(&columns, &["Top J Hits"]);
    let iso_idx = find_header(&columns, &["Isotype"]);
    let mut feature_maps: BTreeMap<String, HashMap<String, f64>> = BTreeMap::new();
    for family in match modality {
        CohortModality::Bcr => vec!["Isotype", "V gene", "D gene", "J gene"],
        _ => vec!["V gene", "J gene", "V-J pair"],
    } {
        feature_maps.insert(family.to_string(), HashMap::new());
    }
    for row in rows.by_ref() {
        let count = row.get(count_idx).and_then(cell_f64).unwrap_or(0.0);
        if count <= 0.0 {
            continue;
        }
        if modality == CohortModality::Bcr {
            if let Some(idx) = iso_idx {
                let iso = row.get(idx).map(cell_text).unwrap_or_default();
                if !iso.is_empty() {
                    *feature_maps
                        .get_mut("Isotype")
                        .unwrap()
                        .entry(iso)
                        .or_insert(0.0) += count;
                }
            }
        }
        let v_raw = v_idx
            .and_then(|idx| row.get(idx))
            .map(cell_text)
            .unwrap_or_default();
        let d_raw = d_idx
            .and_then(|idx| row.get(idx))
            .map(cell_text)
            .unwrap_or_default();
        let j_raw = j_idx
            .and_then(|idx| row.get(idx))
            .map(cell_text)
            .unwrap_or_default();
        let v_calls = resolve_calls(&v_raw, policy);
        let d_calls = resolve_calls(&d_raw, policy);
        let j_calls = resolve_calls(&j_raw, policy);
        add_resolved(feature_maps.get_mut("V gene").unwrap(), &v_calls, count);
        if modality == CohortModality::Bcr {
            add_resolved(feature_maps.get_mut("D gene").unwrap(), &d_calls, count);
        }
        add_resolved(feature_maps.get_mut("J gene").unwrap(), &j_calls, count);
        if modality != CohortModality::Bcr {
            let pair_map = feature_maps.get_mut("V-J pair").unwrap();
            for (v, vw) in &v_calls {
                for (j, jw) in &j_calls {
                    *pair_map.entry(format!("{v} | {j}")).or_insert(0.0) += count * vw * jw;
                }
            }
        }
    }
    drop(rows);
    drop(range);
    drop(workbook);

    for map in feature_maps.values_mut() {
        let denom = map.values().sum::<f64>();
        if denom > 0.0 {
            for value in map.values_mut() {
                *value /= denom;
            }
        }
    }
    Ok(feature_maps)
}

fn parse_repertoire_file_with_audit<F>(
    path: &Path,
    sample_id: &str,
    group: &str,
    modality: CohortModality,
    policy: AmbiguousCallPolicy,
    scratch: &mut ParsedClonotypeScratch,
    mut audit: F,
) -> Result<ParsedRepertoire>
where
    F: FnMut(&str),
{
    // The server and desktop both enter this exact parser. The audit callbacks
    // deliberately bracket XLSX materialization, row scanning, worksheet
    // destruction, and compact-store finalization so a stale "parsing sample"
    // message cannot hide allocator work at one of those boundaries.
    let parse_started = Instant::now();
    let scratch_reset_started = Instant::now();
    scratch.reset();
    audit(&format!(
        "parser scratch reset in {:.3}s (hash_capacity={}; nt_capacity_bytes={}; memory {})",
        scratch_reset_started.elapsed().as_secs_f64(),
        scratch.hash_heads.capacity(),
        scratch.nt_bytes.capacity(),
        native_process_memory_summary()
    ));
    let builder_started = Instant::now();
    let mut clonotypes = ParsedClonotypeBuilder::with_capacity(16_384, scratch);
    audit(&format!(
        "compact parser buffers initialized in {:.3}s before workbook open (entry_capacity={}; key_capacity_bytes={}; scratch_hash_capacity={}; scratch_nt_capacity_bytes={}; memory {})",
        builder_started.elapsed().as_secs_f64(),
        clonotypes.entries.capacity(),
        clonotypes.key_bytes.capacity(),
        clonotypes.scratch.hash_heads.capacity(),
        clonotypes.scratch.nt_bytes.capacity(),
        native_process_memory_summary()
    ));
    let input_bytes = fs::metadata(path).ok().map(|meta| meta.len()).unwrap_or(0);
    audit(&format!(
        "open workbook start (input_bytes={input_bytes}; memory {})",
        native_process_memory_summary()
    ));

    let open_started = Instant::now();
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("Could not open {}", path.display()))?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("Workbook has no worksheets"))?;
    audit(&format!(
        "workbook opened in {:.3}s; materializing first worksheet",
        open_started.elapsed().as_secs_f64()
    ));

    let range_started = Instant::now();
    let range = workbook.worksheet_range(&sheet)?;
    let mut rows = range.rows();
    let header = rows.next().ok_or_else(|| anyhow!("Worksheet is empty"))?;
    let row_capacity = rows.size_hint().0;
    audit(&format!(
        "worksheet materialized in {:.3}s (estimated_data_rows={row_capacity}; memory {})",
        range_started.elapsed().as_secs_f64(),
        native_process_memory_summary()
    ));

    // Keep the Calamine workbook container alive while its materialized
    // Range<Data> is being consumed. Before v0.16.19 Rust's ordinary reverse
    // local-drop order released `range` before `workbook`; v0.16.19 explicitly
    // reversed that order in an attempt to lower peak memory. Real-cohort
    // telemetry localized multi-minute stalls exactly inside that early
    // `drop(workbook)` even with ~44 GiB MemAvailable and zero VmSwap. Preserve
    // the historically fast/native destruction order instead: scan first, drop
    // the worksheet cells, then drop the workbook/shared-string container.
    audit("workbook retained until worksheet cells are released; scanning worksheet rows");

    let headers = header.iter().map(cell_text).collect::<Vec<_>>();
    let columns = header_map(&headers);
    let count_idx =
        find_header(&columns, &["Clone Count"]).ok_or_else(|| anyhow!("Missing Clone Count"))?;
    let v_idx = find_header(&columns, &["Top V Hits"]);
    let d_idx = find_header(&columns, &["Top D Hits"]);
    let j_idx = find_header(&columns, &["Top J Hits"]);
    let iso_idx = find_header(&columns, &["Isotype"]);
    let cdr3_idx = find_header(&columns, &["CDR3 Amino Acid Sequence"]);
    let nt_idx = find_header(&columns, &["Clonal Sequence"]);
    let mut counts = Vec::with_capacity(row_capacity.min(16_384));
    let mut feature_maps: BTreeMap<String, HashMap<String, f64>> = BTreeMap::new();
    for family in match modality {
        CohortModality::Bcr => vec!["Isotype", "V gene", "D gene", "J gene"],
        _ => vec!["V gene", "J gene", "V-J pair"],
    } {
        feature_maps.insert(family.to_string(), HashMap::new());
    }
    let mut top = 0.0_f64;
    let mut total = 0.0;
    let mut weighted_len = 0.0;
    let mut weighted_len_den = 0.0;
    let scan_started = Instant::now();
    let mut heartbeat_started = Instant::now();
    let mut rows_seen = 0usize;
    let mut positive_rows = 0usize;

    for row in rows.by_ref() {
        rows_seen += 1;
        let count = row.get(count_idx).and_then(cell_f64).unwrap_or(0.0);
        if count > 0.0 {
            positive_rows += 1;
            counts.push(count);
            total += count;
            top = top.max(count);
            let cdr3 = cdr3_idx
                .and_then(|idx| row.get(idx))
                .map(cell_text)
                .unwrap_or_default();
            if !cdr3.is_empty() {
                weighted_len += count * cdr3.chars().count() as f64;
                weighted_len_den += count;
            }
            if modality == CohortModality::Bcr {
                if let Some(idx) = iso_idx {
                    let iso = row.get(idx).map(cell_text).unwrap_or_default();
                    if !iso.is_empty() {
                        *feature_maps
                            .get_mut("Isotype")
                            .unwrap()
                            .entry(iso)
                            .or_insert(0.0) += count;
                    }
                }
            }
            let v_raw = v_idx
                .and_then(|idx| row.get(idx))
                .map(cell_text)
                .unwrap_or_default();
            let d_raw = d_idx
                .and_then(|idx| row.get(idx))
                .map(cell_text)
                .unwrap_or_default();
            let j_raw = j_idx
                .and_then(|idx| row.get(idx))
                .map(cell_text)
                .unwrap_or_default();
            let v_calls = resolve_calls(&v_raw, policy);
            let d_calls = resolve_calls(&d_raw, policy);
            let j_calls = resolve_calls(&j_raw, policy);
            add_resolved(feature_maps.get_mut("V gene").unwrap(), &v_calls, count);
            if modality == CohortModality::Bcr {
                add_resolved(feature_maps.get_mut("D gene").unwrap(), &d_calls, count);
            }
            add_resolved(feature_maps.get_mut("J gene").unwrap(), &j_calls, count);
            if modality != CohortModality::Bcr {
                let pair_map = feature_maps.get_mut("V-J pair").unwrap();
                for (v, vw) in &v_calls {
                    for (j, jw) in &j_calls {
                        *pair_map.entry(format!("{v} | {j}")).or_insert(0.0) += count * vw * jw;
                    }
                }
            }
            if !cdr3.is_empty() && v_calls.len() == 1 && j_calls.len() == 1 {
                let nt = nt_idx
                    .and_then(|idx| row.get(idx))
                    .map(cell_text)
                    .filter(|value| !value.is_empty());
                clonotypes.record(&v_calls[0].0, &j_calls[0].0, &cdr3, count, nt.as_deref())?;
            }
        }

        if heartbeat_started.elapsed() >= Duration::from_secs(5) {
            audit(&format!(
                "row scan heartbeat (rows_seen={rows_seen}; positive_rows={positive_rows}; local_clonotypes={}; key_arena_bytes={}; nt_arena_bytes={}; hash_entries={}; elapsed={:.1}s; memory {})",
                clonotypes.entries.len(),
                clonotypes.key_bytes.len(),
                clonotypes.scratch.nt_bytes.len(),
                clonotypes.scratch.hash_heads.len(),
                scan_started.elapsed().as_secs_f64(),
                native_process_memory_summary()
            ));
            heartbeat_started = Instant::now();
        }
    }
    drop(rows);

    if total <= 0.0 {
        bail!("No positive clone counts");
    }
    audit(&format!(
        "row scan complete (rows_seen={rows_seen}; positive_rows={positive_rows}; local_clonotypes={}; elapsed={:.3}s); releasing worksheet cells; memory {}",
        clonotypes.entries.len(),
        scan_started.elapsed().as_secs_f64(),
        native_process_memory_summary()
    ));

    // Range<Data> can itself own a large number of cell Strings. Make that
    // destructor a measured parser boundary rather than letting it execute
    // invisibly at function return.
    let range_drop_started = Instant::now();
    drop(range);
    audit(&format!(
        "worksheet cells released in {:.3}s; memory {}; releasing workbook container next",
        range_drop_started.elapsed().as_secs_f64(),
        native_process_memory_summary()
    ));

    let workbook_drop_started = Instant::now();
    drop(workbook);
    audit(&format!(
        "workbook container released after worksheet cells in {:.3}s; memory {}",
        workbook_drop_started.elapsed().as_secs_f64(),
        native_process_memory_summary()
    ));

    for map in feature_maps.values_mut() {
        let denom = map.values().sum::<f64>();
        if denom > 0.0 {
            for value in map.values_mut() {
                *value /= denom;
            }
        }
    }

    let final_clonotype_count = clonotypes.entries.len();
    let final_key_bytes = clonotypes.key_bytes.len();
    audit(&format!(
        "compact clonotype finalize start (clonotypes={final_clonotype_count}; key_arena_bytes={final_key_bytes}; memory {})",
        native_process_memory_summary()
    ));
    let finalize_started = Instant::now();
    let clonotypes = clonotypes.finish();
    audit(&format!(
        "compact clonotype finalize complete in {:.3}s; parser total {:.3}s; memory {}",
        finalize_started.elapsed().as_secs_f64(),
        parse_started.elapsed().as_secs_f64(),
        native_process_memory_summary()
    ));

    Ok(ParsedRepertoire {
        sample_id: sample_id.to_string(),
        group: group.to_string(),
        counts,
        clonotypes,
        feature_maps,
        top_clone_fraction: top / total,
        weighted_mean_cdr3_aa_length: if weighted_len_den > 0.0 {
            weighted_len / weighted_len_den
        } else {
            f64::NAN
        },
    })
}

fn repertoire_qc_metrics_from_counts(
    counts: &[f64],
    top_clone_fraction: f64,
    weighted_mean_cdr3_aa_length: f64,
) -> Result<Vec<(String, f64)>> {
    let total = counts.iter().copied().sum::<f64>();
    if total <= 0.0 {
        bail!("No positive clone counts");
    }
    let probs = counts
        .iter()
        .copied()
        .filter(|value| *value > 0.0)
        .map(|value| value / total)
        .collect::<Vec<_>>();
    let richness = probs.len() as f64;
    let shannon = -probs
        .iter()
        .copied()
        .filter(|probability| *probability > 0.0)
        .map(|probability| probability * probability.ln())
        .sum::<f64>();
    let simpson = 1.0
        - probs
            .iter()
            .map(|probability| probability * probability)
            .sum::<f64>();
    let clonality = if richness > 1.0 {
        1.0 - shannon / richness.ln()
    } else {
        0.0
    };
    Ok(vec![
        ("Total clone count".to_string(), total),
        ("Observed richness".to_string(), richness),
        ("Shannon diversity".to_string(), shannon),
        ("Simpson diversity".to_string(), simpson),
        ("Clonality".to_string(), clonality),
        ("Top clone fraction".to_string(), top_clone_fraction),
        (
            "Weighted mean CDR3 AA length".to_string(),
            weighted_mean_cdr3_aa_length,
        ),
    ])
}

pub(crate) fn parse_repertoire_qc_metrics(
    path: &Path,
    _sample_id: &str,
    _modality: CohortModality,
) -> Result<Vec<(String, f64)>> {
    // Cross-modal QC needs only seven depth/diversity summary scalars. The
    // previous implementation used the full corrected parser, allocating and
    // destroying exact clonotype arenas, V/J composition maps, nucleotide
    // convergence state, and hash tables for every repertoire file immediately
    // before the server reparsed the same inputs for corrected analysis. That
    // is unnecessary allocator churn and is a concrete server/native lifecycle
    // difference because frozen web jobs run QC immediately before unified
    // science. Keep this preflight parser intentionally minimal.
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("Could not open {}", path.display()))?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("Workbook has no worksheets"))?;
    let range = workbook.worksheet_range(&sheet)?;
    // Match the full/native parser lifetime ordering: worksheet cells are
    // released before the Calamine workbook/shared-string container.
    let mut rows = range.rows();
    let header = rows.next().ok_or_else(|| anyhow!("Worksheet is empty"))?;
    let row_capacity = rows.size_hint().0;
    let headers = header.iter().map(cell_text).collect::<Vec<_>>();
    let columns = header_map(&headers);
    let count_idx =
        find_header(&columns, &["Clone Count"]).ok_or_else(|| anyhow!("Missing Clone Count"))?;
    let cdr3_idx = find_header(&columns, &["CDR3 Amino Acid Sequence"]);
    let mut counts = Vec::with_capacity(row_capacity);
    let mut top = 0.0_f64;
    let mut total = 0.0_f64;
    let mut weighted_len = 0.0_f64;
    let mut weighted_len_den = 0.0_f64;
    for row in rows.by_ref() {
        let count = row.get(count_idx).and_then(cell_f64).unwrap_or(0.0);
        if count <= 0.0 {
            continue;
        }
        counts.push(count);
        total += count;
        top = top.max(count);
        let cdr3 = cdr3_idx
            .and_then(|idx| row.get(idx))
            .map(cell_text)
            .unwrap_or_default();
        if !cdr3.is_empty() {
            weighted_len += count * cdr3.chars().count() as f64;
            weighted_len_den += count;
        }
    }
    drop(rows);
    drop(range);
    drop(workbook);
    if total <= 0.0 {
        bail!("No positive clone counts");
    }
    repertoire_qc_metrics_from_counts(
        &counts,
        top / total,
        if weighted_len_den > 0.0 {
            weighted_len / weighted_len_den
        } else {
            f64::NAN
        },
    )
}

fn resolve_calls(raw: &str, policy: AmbiguousCallPolicy) -> Vec<(String, f64)> {
    let mut hits = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if hits.is_empty() {
        return Vec::new();
    }
    match policy {
        AmbiguousCallPolicy::Exclude => {
            if hits.len() == 1 {
                vec![(hits.remove(0), 1.0)]
            } else {
                Vec::new()
            }
        }
        AmbiguousCallPolicy::First => vec![(hits.remove(0), 1.0)],
        AmbiguousCallPolicy::Split => {
            let w = 1.0 / hits.len() as f64;
            hits.into_iter().map(|h| (h, w)).collect()
        }
        AmbiguousCallPolicy::Family => {
            let mut families = hits
                .into_iter()
                .map(|h| gene_family(&h))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if families.is_empty() {
                Vec::new()
            } else {
                let w = 1.0 / families.len() as f64;
                families.drain(..).map(|h| (h, w)).collect()
            }
        }
    }
}

fn gene_family(value: &str) -> String {
    value.split('-').next().unwrap_or(value).trim().to_string()
}

fn add_resolved(map: &mut HashMap<String, f64>, calls: &[(String, f64)], count: f64) {
    for (feature, weight) in calls {
        *map.entry(feature.clone()).or_insert(0.0) += count * weight;
    }
}

pub(crate) fn parse_isotype_file(
    path: &Path,
    determined_only: bool,
) -> Result<HashMap<String, f64>> {
    let (headers, rows) = read_first_sheet(path)?;
    let columns = header_map(&headers);
    let fraction_idx = find_header(&columns, &["Isotype Read Fraction"])
        .ok_or_else(|| anyhow!("Missing Isotype Read Fraction"))?;
    let iso_idx = find_header(&columns, &["IGH Isotype", "Isotype"])
        .ok_or_else(|| anyhow!("Missing IGH Isotype"))?;
    let mut map = HashMap::new();
    for row in rows {
        let iso = row.get(iso_idx).map(cell_text).unwrap_or_default();
        let value = row.get(fraction_idx).and_then(cell_f64).unwrap_or(f64::NAN);
        if iso.is_empty() || !value.is_finite() {
            continue;
        }
        if determined_only && iso.eq_ignore_ascii_case("Undetermined") {
            continue;
        }
        *map.entry(iso).or_insert(0.0) += value.max(0.0);
    }
    let total = map.values().sum::<f64>();
    if total <= 0.0 {
        bail!("No usable isotype fractions");
    }
    for value in map.values_mut() {
        *value /= total;
    }
    Ok(map)
}

pub(crate) fn parse_immune_file(path: &Path) -> Result<HashMap<String, f64>> {
    let (headers, rows) = read_first_sheet(path)?;
    let columns = header_map(&headers);
    let type_idx = find_header(&columns, &["Immune Cell Type"])
        .ok_or_else(|| anyhow!("Missing Immune Cell Type"))?;
    let score_idx =
        find_header(&columns, &["ssGSEA Score"]).ok_or_else(|| anyhow!("Missing ssGSEA Score"))?;
    let mut map = HashMap::new();
    for row in rows {
        let cell = row.get(type_idx).map(cell_text).unwrap_or_default();
        let score = row.get(score_idx).and_then(cell_f64).unwrap_or(f64::NAN);
        if !cell.is_empty() && score.is_finite() {
            map.insert(cell, score);
        }
    }
    if map.is_empty() {
        bail!("No usable immune ssGSEA scores");
    }
    Ok(map)
}

fn rarefy_metrics(counts: &[f64], depth: usize, reps: usize, seed: u64) -> (f64, f64) {
    if counts.is_empty() || depth == 0 || reps == 0 {
        return (f64::NAN, f64::NAN);
    }

    // The dense clonotype-ID representation can contain millions of clonotypes.
    // Matched-depth rarefaction usually samples a relatively small number of
    // reads. Do not allocate/clear a million-entry sampled_per_clone vector for
    // every replicate. Instead, keep only non-zero clonotypes and use cumulative
    // counts plus binary search to map sampled read positions back to clonotypes.
    let mut cumulative = Vec::<u64>::new();
    let mut total = 0u64;
    for count in counts {
        let integer = if count.is_finite() && *count > 0.0 {
            count.round().max(0.0) as u64
        } else {
            0
        };
        if integer > 0 {
            total = total.saturating_add(integer);
            cumulative.push(total);
        }
    }

    if total == 0 {
        return (f64::NAN, f64::NAN);
    }

    let k = (depth as u64).min(total) as usize;
    let mut rng = XorShift64::new(seed);
    let mut richness_sum = 0.0;
    let mut shannon_sum = 0.0;

    for _ in 0..reps {
        let mut selected = HashSet::with_capacity(k.saturating_mul(2).max(16));
        let start = total - k as u64;
        for j in start..total {
            let t = rng.next_bounded(j + 1);
            if !selected.insert(t) {
                selected.insert(j);
            }
        }

        let mut sampled = HashMap::<usize, u32>::with_capacity(k.max(1));
        for position in selected {
            let clone_index = cumulative.partition_point(|value| *value <= position);
            *sampled.entry(clone_index).or_insert(0) += 1;
        }

        let richness = sampled.len() as f64;
        let d = k as f64;
        // Legacy code accumulated Shannon terms in clonotype-index order.
        // Preserve that exact deterministic floating-point order even though
        // the sparse implementation uses a tiny HashMap internally.
        let mut sampled_counts = sampled.into_iter().collect::<Vec<_>>();
        sampled_counts.sort_unstable_by_key(|(clone_index, _)| *clone_index);
        let shannon = sampled_counts
            .iter()
            .map(|(_, value)| {
                let p = *value as f64 / d;
                -p * p.ln()
            })
            .sum::<f64>();

        richness_sum += richness;
        shannon_sum += shannon;
    }

    (richness_sum / reps as f64, shannon_sum / reps as f64)
}

#[derive(Clone)]
struct XorShift64 {
    state: u64,
}
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
    fn next_bounded(&mut self, upper: u64) -> u64 {
        if upper <= 1 {
            return 0;
        }
        // Rejection sampling avoids modulo bias.
        let zone = u64::MAX - (u64::MAX % upper);
        loop {
            let x = self.next_u64();
            if x < zone {
                return x % upper;
            }
        }
    }
    fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = (self.next_u64() as usize) % (i + 1);
            values.swap(i, j);
        }
    }
}

fn stable_seed(text: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub(crate) fn pca_from_feature_rows_for_samples(
    rows: &[Vec<f64>],
    sample_ids: &[String],
    group_case: &[bool],
    subjects: &[String],
    title: &str,
    permutations: usize,
) -> Option<OrdinationResult> {
    if sample_ids.len() != group_case.len() || sample_ids.len() != subjects.len() {
        return None;
    }
    let design = DesignMatrix {
        sample_ids: sample_ids.to_vec(),
        group_case: group_case.to_vec(),
        subjects: subjects.to_vec(),
        x: vec![vec![1.0]; sample_ids.len()],
        column_names: vec!["Intercept".to_string()],
        group_col: 0,
        dropped_missing_covariates: 0,
    };
    pca_from_rows(rows, &design, title, permutations)
}

pub(crate) fn euclidean_distance_from_feature_rows(rows: &[Vec<f64>]) -> Vec<Vec<f64>> {
    euclidean_distance_from_rows(rows)
}

pub(crate) fn permanova_for_sample_distance(
    dist: &[Vec<f64>],
    group_case: &[bool],
    subjects: &[String],
    permutations: usize,
    seed: u64,
) -> (f64, f64) {
    permanova(dist, group_case, subjects, permutations, seed)
}

fn pca_from_rows(
    rows: &[Vec<f64>],
    design: &DesignMatrix,
    title: &str,
    permutations: usize,
) -> Option<OrdinationResult> {
    if rows.len() < 2 || design.x.len() < 3 {
        return None;
    }
    let n = design.x.len();
    let mut gram = vec![vec![0.0; n]; n];
    for row in rows {
        if row.len() != n {
            continue;
        }
        let m = mean(row);
        let centered = row.iter().map(|v| v - m).collect::<Vec<_>>();
        for i in 0..n {
            for j in i..n {
                let v = centered[i] * centered[j];
                gram[i][j] += v;
                if i != j {
                    gram[j][i] += v;
                }
            }
        }
    }
    let (l1, v1, l2, v2) = top_two_eigen(&gram)?;
    let trace = (0..n).map(|i| gram[i][i]).sum::<f64>().max(1e-12);
    let points = (0..n)
        .map(|i| OrdinationPoint {
            sample_id: design.sample_ids[i].clone(),
            group: if design.group_case[i] {
                "Case"
            } else {
                "Control"
            }
            .to_string(),
            subject: design.subjects[i].clone(),
            x: v1[i] * l1.max(0.0).sqrt(),
            y: v2[i] * l2.max(0.0).sqrt(),
        })
        .collect();
    Some(OrdinationResult {
        name: title.to_string(),
        x_label: format!("PC1 ({:.1}%)", 100.0 * l1.max(0.0) / trace),
        y_label: format!("PC2 ({:.1}%)", 100.0 * l2.max(0.0) / trace),
        points,
        permanova_f: f64::NAN,
        permanova_p: f64::NAN,
        permutations,
    })
}

fn pcoa_from_distance(
    dist: &[Vec<f64>],
    design: &DesignMatrix,
    title: &str,
    permutations: usize,
    seed: u64,
) -> Option<OrdinationResult> {
    let n = dist.len();
    if n < 3 || design.x.len() != n {
        return None;
    }
    let mut d2 = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            d2[i][j] = dist[i][j] * dist[i][j];
        }
    }
    let row_mean = (0..n).map(|i| mean(&d2[i])).collect::<Vec<_>>();
    let grand = mean(&row_mean);
    let mut b = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            b[i][j] = -0.5 * (d2[i][j] - row_mean[i] - row_mean[j] + grand);
        }
    }
    let (l1, v1, l2, v2) = top_two_eigen(&b)?;
    let points = (0..n)
        .map(|i| OrdinationPoint {
            sample_id: design.sample_ids[i].clone(),
            group: if design.group_case[i] {
                "Case"
            } else {
                "Control"
            }
            .to_string(),
            subject: design.subjects[i].clone(),
            x: v1[i] * l1.max(0.0).sqrt(),
            y: v2[i] * l2.max(0.0).sqrt(),
        })
        .collect();
    let (f, p) = permanova(
        dist,
        &design.group_case,
        &design.subjects,
        permutations,
        seed,
    );
    // For non-Euclidean ecological/repertoire distances, negative PCoA
    // eigenvalues can occur.  Without computing the full eigenspectrum, a
    // percent-variance label would be misleading, so report axes plainly.
    Some(OrdinationResult {
        name: title.to_string(),
        x_label: "PCo1".to_string(),
        y_label: "PCo2".to_string(),
        points,
        permanova_f: f,
        permanova_p: p,
        permutations,
    })
}

fn top_two_eigen(a: &[Vec<f64>]) -> Option<(f64, Vec<f64>, f64, Vec<f64>)> {
    let n = a.len();
    if n == 0 {
        return None;
    }
    let mut v1 = (0..n)
        .map(|i| ((i + 1) as f64 * 1.61803398875).sin() + 0.1)
        .collect::<Vec<_>>();
    normalize_vec(&mut v1);
    for _ in 0..160 {
        let mut w = mat_vec_dense(a, &v1);
        normalize_vec(&mut w);
        if vec_distance(&w, &v1) < 1e-10 {
            v1 = w;
            break;
        }
        v1 = w;
    }
    let l1 = dot(&v1, &mat_vec_dense(a, &v1));
    let mut v2 = (0..n)
        .map(|i| ((i + 3) as f64 * 0.754877666).cos() + 0.2)
        .collect::<Vec<_>>();
    orthogonalize(&mut v2, &v1);
    normalize_vec(&mut v2);
    for _ in 0..180 {
        let av = mat_vec_dense(a, &v2);
        let proj = dot(&v1, &av);
        let mut w = av
            .iter()
            .zip(v1.iter())
            .map(|(x, u)| x - proj * u)
            .collect::<Vec<_>>();
        normalize_vec(&mut w);
        if vec_distance(&w, &v2) < 1e-10 {
            v2 = w;
            break;
        }
        v2 = w;
    }
    let l2 = dot(&v2, &mat_vec_dense(a, &v2));
    Some((l1, v1, l2, v2))
}

fn permanova(
    dist: &[Vec<f64>],
    groups: &[bool],
    subjects: &[String],
    permutations: usize,
    seed: u64,
) -> (f64, f64) {
    if dist.len() != groups.len() || groups.len() < 4 {
        return (f64::NAN, f64::NAN);
    }
    // The distance matrix is invariant across permutations.  Precompute the
    // squared upper triangle and the group-independent total once rather than
    // recomputing both for every shuffled label vector.  This preserves the
    // exact permutation sequence and pair iteration order while removing a
    // large amount of redundant work for repertoire-scale cohorts.
    let n = groups.len();
    let mut pairs = Vec::with_capacity(n.saturating_mul(n.saturating_sub(1)) / 2);
    let mut total_sum = 0.0;
    for i in 0..n {
        for j in (i + 1)..n {
            let d2 = dist[i][j] * dist[i][j];
            total_sum += d2;
            pairs.push((i, j, d2));
        }
    }
    let total = total_sum / n as f64;
    let observed = permanova_f_from_pairs(&pairs, groups, total);
    if !observed.is_finite() || permutations == 0 {
        return (observed, f64::NAN);
    }
    let mut blocks: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, s) in subjects.iter().enumerate() {
        blocks.entry(s.clone()).or_default().push(i);
    }
    let block_vec = blocks.into_values().collect::<Vec<_>>();
    let mut labels = block_vec
        .iter()
        .map(|idx| groups[idx[0]])
        .collect::<Vec<_>>();
    let mut rng = XorShift64::new(seed);
    let mut extreme = 0usize;
    for _ in 0..permutations {
        rng.shuffle(&mut labels);
        let mut perm = groups.to_vec();
        for (b, idxs) in block_vec.iter().enumerate() {
            for &i in idxs {
                perm[i] = labels[b];
            }
        }
        let f = permanova_f_from_pairs(&pairs, &perm, total);
        if f.is_finite() && f >= observed - 1e-12 {
            extreme += 1;
        }
    }
    (
        observed,
        (extreme as f64 + 1.0) / (permutations as f64 + 1.0),
    )
}

fn permanova_f_from_pairs(pairs: &[(usize, usize, f64)], groups: &[bool], total: f64) -> f64 {
    let n = groups.len();
    let n_case = groups.iter().filter(|x| **x).count();
    let n_ctrl = n - n_case;
    if n_case < 2 || n_ctrl < 2 {
        return f64::NAN;
    }
    let mut within_case = 0.0;
    let mut within_ctrl = 0.0;
    for &(i, j, d2) in pairs {
        if groups[i] == groups[j] {
            if groups[i] {
                within_case += d2;
            } else {
                within_ctrl += d2;
            }
        }
    }
    let within = within_case / n_case as f64 + within_ctrl / n_ctrl as f64;
    let between = (total - within).max(0.0);
    if within <= 0.0 {
        return f64::INFINITY;
    }
    (between / 1.0) / (within / (n as f64 - 2.0))
}

fn permanova_f(dist: &[Vec<f64>], groups: &[bool]) -> f64 {
    if dist.len() != groups.len() || groups.is_empty() {
        return f64::NAN;
    }
    let n = groups.len();
    let mut pairs = Vec::with_capacity(n.saturating_mul(n.saturating_sub(1)) / 2);
    let mut total_sum = 0.0;
    for i in 0..n {
        for j in (i + 1)..n {
            let d2 = dist[i][j] * dist[i][j];
            total_sum += d2;
            pairs.push((i, j, d2));
        }
    }
    permanova_f_from_pairs(&pairs, groups, total_sum / n as f64)
}

fn euclidean_distance_from_rows(feature_rows: &[Vec<f64>]) -> Vec<Vec<f64>> {
    if feature_rows.is_empty() {
        return Vec::new();
    }
    let n = feature_rows[0].len();
    let mut d = vec![vec![0.0; n]; n];
    for row in feature_rows {
        for i in 0..n {
            for j in (i + 1)..n {
                let delta = row[i] - row[j];
                d[i][j] += delta * delta;
                d[j][i] = d[i][j];
            }
        }
    }
    for i in 0..n {
        for j in 0..n {
            d[i][j] = d[i][j].sqrt();
        }
    }
    d
}
fn euclidean_distance_from_sample_rows(rows: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = rows.len();
    let mut d = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let s = rows[i]
                .iter()
                .zip(rows[j].iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            d[i][j] = s;
            d[j][i] = s;
        }
    }
    d
}

fn average_linkage_dendrogram(
    dist: &[Vec<f64>],
    design: &DesignMatrix,
    title: &str,
    distance_label: &str,
) -> Option<DendrogramResult> {
    let groups = design
        .group_case
        .iter()
        .map(|is_case| if *is_case { "Case" } else { "Control" }.to_string())
        .collect::<Vec<_>>();
    average_linkage_dendrogram_for_samples(
        dist,
        &design.sample_ids,
        &groups,
        &design.subjects,
        title,
        distance_label,
    )
}

/// Average-linkage clustering over an already-computed sample distance matrix.
///
/// This shared entry point is also used by the longitudinal patient-signature
/// analysis, where `groups` carries a descriptive collection/time label and
/// `subjects` carries the configured patient/person identifier.  The clustering
/// itself never uses either annotation.
pub(crate) fn average_linkage_dendrogram_for_samples(
    dist: &[Vec<f64>],
    sample_ids: &[String],
    groups: &[String],
    subjects: &[String],
    title: &str,
    distance_label: &str,
) -> Option<DendrogramResult> {
    let n = dist.len();
    if n < 2
        || sample_ids.len() != n
        || groups.len() != n
        || subjects.len() != n
        || dist.iter().any(|row| row.len() != n)
    {
        return None;
    }
    let max_nodes = n * 2 - 1;
    let mut matrix = vec![vec![f64::INFINITY; max_nodes]; max_nodes];
    for i in 0..n {
        matrix[i][i] = 0.0;
        for j in (i + 1)..n {
            let d = dist[i][j];
            if !d.is_finite() {
                return None;
            }
            matrix[i][j] = d;
            matrix[j][i] = d;
        }
    }
    let mut active = vec![false; max_nodes];
    let mut sizes = vec![0usize; max_nodes];
    let mut heights = vec![0.0; max_nodes];
    let mut children: Vec<Option<(usize, usize)>> = vec![None; max_nodes];
    for i in 0..n {
        active[i] = true;
        sizes[i] = 1;
    }

    let mut active_count = n;
    let mut next = n;
    while active_count > 1 {
        let mut best = None::<(usize, usize, f64)>;
        for i in 0..next {
            if !active[i] {
                continue;
            }
            for j in (i + 1)..next {
                if !active[j] {
                    continue;
                }
                let d = matrix[i][j];
                if !d.is_finite() {
                    continue;
                }
                if best.as_ref().is_none_or(|(_, _, bd)| d < *bd) {
                    best = Some((i, j, d));
                }
            }
        }
        let (a, b, height) = best?;
        let new_id = next;
        next += 1;
        children[new_id] = Some((a, b));
        heights[new_id] = height.max(heights[a]).max(heights[b]);
        sizes[new_id] = sizes[a] + sizes[b];
        for k in 0..new_id {
            if !active[k] || k == a || k == b {
                continue;
            }
            let d = (sizes[a] as f64 * matrix[a][k] + sizes[b] as f64 * matrix[b][k])
                / sizes[new_id] as f64;
            matrix[new_id][k] = d;
            matrix[k][new_id] = d;
        }
        active[a] = false;
        active[b] = false;
        active[new_id] = true;
        active_count -= 1;
    }
    let root = (0..next).find(|&i| active[i])?;

    fn leaf_order(
        node: usize,
        n: usize,
        children: &[Option<(usize, usize)>],
        out: &mut Vec<usize>,
    ) {
        if node < n {
            out.push(node);
            return;
        }
        if let Some((a, b)) = children[node] {
            leaf_order(a, n, children, out);
            leaf_order(b, n, children, out);
        }
    }
    let mut order = Vec::with_capacity(n);
    leaf_order(root, n, &children, &mut order);
    if order.len() != n {
        return None;
    }
    let mut y = vec![0.0; next];
    for (pos, leaf) in order.iter().copied().enumerate() {
        y[leaf] = pos as f64;
    }
    let mut segments = Vec::with_capacity((n - 1) * 3);
    fn layout_node(
        node: usize,
        n: usize,
        children: &[Option<(usize, usize)>],
        heights: &[f64],
        y: &mut [f64],
        segments: &mut Vec<DendrogramSegment>,
    ) -> f64 {
        if node < n {
            return y[node];
        }
        let Some((a, b)) = children[node] else {
            return y[node];
        };
        let ya = layout_node(a, n, children, heights, y, segments);
        let yb = layout_node(b, n, children, heights, y, segments);
        let yp = (ya + yb) * 0.5;
        y[node] = yp;
        let xp = heights[node];
        segments.push(DendrogramSegment {
            x1: heights[a],
            y1: ya,
            x2: xp,
            y2: ya,
        });
        segments.push(DendrogramSegment {
            x1: heights[b],
            y1: yb,
            x2: xp,
            y2: yb,
        });
        segments.push(DendrogramSegment {
            x1: xp,
            y1: ya,
            x2: xp,
            y2: yb,
        });
        yp
    }
    layout_node(root, n, &children, &heights, &mut y, &mut segments);
    let leaves = order
        .into_iter()
        .enumerate()
        .map(|(pos, i)| DendrogramLeaf {
            sample_id: sample_ids[i].clone(),
            group: groups[i].clone(),
            subject: subjects[i].clone(),
            position: pos as f64,
        })
        .collect::<Vec<_>>();
    Some(DendrogramResult {
        name: title.to_string(),
        distance_label: distance_label.to_string(),
        linkage: "Average linkage".to_string(),
        leaves,
        segments,
    })
}

#[derive(Clone)]
struct P2Median {
    initial: Vec<f64>,
    q: [f64; 5],
    n: [f64; 5],
    np: [f64; 5],
    ready: bool,
}
impl P2Median {
    fn new() -> Self {
        Self {
            initial: Vec::new(),
            q: [0.0; 5],
            n: [0.0; 5],
            np: [0.0; 5],
            ready: false,
        }
    }
    fn push(&mut self, x: f64) {
        if !x.is_finite() {
            return;
        }
        if !self.ready {
            self.initial.push(x);
            if self.initial.len() == 5 {
                self.initial.sort_by(|a, b| a.total_cmp(b));
                for i in 0..5 {
                    self.q[i] = self.initial[i];
                    self.n[i] = (i + 1) as f64;
                }
                self.np = [1.0, 2.0, 3.0, 4.0, 5.0];
                self.ready = true;
            }
            return;
        }
        let k = if x < self.q[0] {
            self.q[0] = x;
            0
        } else if x < self.q[1] {
            0
        } else if x < self.q[2] {
            1
        } else if x < self.q[3] {
            2
        } else if x <= self.q[4] {
            3
        } else {
            self.q[4] = x;
            3
        };
        for i in (k + 1)..5 {
            self.n[i] += 1.0;
        }
        let dn = [0.0, 0.25, 0.5, 0.75, 1.0];
        for i in 0..5 {
            self.np[i] += dn[i];
        }
        for i in 1..4 {
            let d = self.np[i] - self.n[i];
            if (d >= 1.0 && self.n[i + 1] - self.n[i] > 1.0)
                || (d <= -1.0 && self.n[i - 1] - self.n[i] < -1.0)
            {
                let ds = d.signum();
                let ni = self.n[i];
                let qp = self.q[i]
                    + ds / (self.n[i + 1] - self.n[i - 1])
                        * ((ni - self.n[i - 1] + ds) * (self.q[i + 1] - self.q[i])
                            / (self.n[i + 1] - ni)
                            + (self.n[i + 1] - ni - ds) * (self.q[i] - self.q[i - 1])
                                / (ni - self.n[i - 1]));
                if self.q[i - 1] < qp && qp < self.q[i + 1] {
                    self.q[i] = qp;
                } else {
                    let j = if ds > 0.0 { i + 1 } else { i - 1 };
                    self.q[i] += ds * (self.q[j] - self.q[i]) / (self.n[j] - self.n[i]);
                }
                self.n[i] += ds;
            }
        }
    }
    fn value(&self) -> Option<f64> {
        if self.ready {
            Some(self.q[2])
        } else if self.initial.is_empty() {
            None
        } else {
            let mut v = self.initial.clone();
            v.sort_by(|a, b| a.total_cmp(b));
            Some(v[v.len() / 2])
        }
    }
}

fn read_first_sheet(path: &Path) -> Result<(Vec<String>, Vec<Vec<Data>>)> {
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("Could not open {}", path.display()))?;
    let sheet = workbook
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("Workbook has no worksheets"))?;
    let range = workbook.worksheet_range(&sheet)?;
    let mut iter = range.rows();
    let header = iter.next().ok_or_else(|| anyhow!("Worksheet is empty"))?;
    let headers = header.iter().map(cell_text).collect();
    let rows = iter.map(|r| r.to_vec()).collect();
    Ok((headers, rows))
}
fn header_map(headers: &[String]) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_ascii_lowercase(), i))
        .collect()
}
fn find_header(map: &HashMap<String, usize>, names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|n| map.get(&n.to_ascii_lowercase()).copied())
}
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(v) => {
            if v.fract() == 0.0 {
                format!("{v:.0}")
            } else {
                v.to_string()
            }
        }
        Data::Int(v) => v.to_string(),
        Data::Bool(v) => v.to_string(),
        other => other.to_string(),
    }
}
fn cell_f64(cell: &Data) -> Option<f64> {
    match cell {
        Data::Float(v) => Some(*v),
        Data::Int(v) => Some(*v as f64),
        Data::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}
fn values_at_columns_with_zero(record: &csv::StringRecord, cols: &[usize]) -> Vec<f64> {
    cols.iter()
        .map(|idx| {
            record
                .get(*idx)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
        })
        .collect()
}
fn invert_matrix(a: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
    let n = a.len();
    if n == 0 || a.iter().any(|r| r.len() != n) {
        bail!("Matrix is not square");
    }
    let mut aug = vec![vec![0.0; 2 * n]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = a[i][j];
        }
        aug[i][n + i] = 1.0;
    }
    for col in 0..n {
        let mut pivot = col;
        for r in (col + 1)..n {
            if aug[r][col].abs() > aug[pivot][col].abs() {
                pivot = r;
            }
        }
        if aug[pivot][col].abs() < 1e-10 {
            bail!("Singular matrix");
        }
        aug.swap(col, pivot);
        let div = aug[col][col];
        for j in 0..2 * n {
            aug[col][j] /= div;
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let factor = aug[r][col];
            if factor == 0.0 {
                continue;
            }
            for j in 0..2 * n {
                aug[r][j] -= factor * aug[col][j];
            }
        }
    }
    Ok((0..n).map(|i| aug[i][n..].to_vec()).collect())
}
fn mat_vec(a: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    a.iter().map(|r| dot(r, v)).collect()
}
fn mat_vec_dense(a: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
    mat_vec(a, v)
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
fn quadratic_form(x: &[f64], a: &[Vec<f64>]) -> f64 {
    dot(x, &mat_vec(a, x))
}
fn outer_add(a: &mut [Vec<f64>], v: &[f64], scale: f64) {
    for i in 0..v.len() {
        for j in 0..v.len() {
            a[i][j] += scale * v[i] * v[j];
        }
    }
}
fn scale_matrix(a: &mut [Vec<f64>], scale: f64) {
    for row in a {
        for v in row {
            *v *= scale;
        }
    }
}
fn sandwich(bread: &[Vec<f64>], meat: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = bread.len();
    let mut temp = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                temp[i][j] += bread[i][k] * meat[k][j];
            }
        }
    }
    let mut out = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                out[i][j] += temp[i][k] * bread[j][k];
            }
        }
    }
    out
}
fn normalize_vec(v: &mut [f64]) {
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for x in v {
            *x /= norm;
        }
    }
}
fn orthogonalize(v: &mut [f64], basis: &[f64]) {
    let p = dot(v, basis);
    for (i, x) in v.iter_mut().enumerate() {
        *x -= p * basis[i];
    }
}
fn vec_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
        .min(
            a.iter()
                .zip(b)
                .map(|(x, y)| (x + y).powi(2))
                .sum::<f64>()
                .sqrt(),
        )
}
fn sample_variance(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64
}
fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}
fn mean_finite(v: &[f64]) -> f64 {
    let f = v
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .collect::<Vec<_>>();
    mean(&f)
}
fn sd_finite(v: &[f64], m: f64) -> f64 {
    let f = v
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .collect::<Vec<_>>();
    if f.len() < 2 {
        return 0.0;
    }
    (f.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (f.len() - 1) as f64).sqrt()
}
fn shannon_entropy(counts: &[f64]) -> f64 {
    let total = counts.iter().copied().filter(|x| *x > 0.0).sum::<f64>();
    if total <= 0.0 {
        return f64::NAN;
    }
    counts
        .iter()
        .copied()
        .filter(|x| *x > 0.0)
        .map(|x| {
            let p = x / total;
            -p * p.ln()
        })
        .sum()
}
fn mann_whitney_p(x: &[f64], y: &[f64]) -> f64 {
    if x.len() < 2 || y.len() < 2 {
        return f64::NAN;
    }
    let mut c = Vec::with_capacity(x.len() + y.len());
    c.extend(x.iter().copied().map(|v| (v, true)));
    c.extend(y.iter().copied().map(|v| (v, false)));
    c.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut rankx = 0.0;
    let mut ties = 0.0;
    let mut i = 0;
    while i < c.len() {
        let mut j = i + 1;
        while j < c.len() && c[j].0 == c[i].0 {
            j += 1;
        }
        let rank = (i + 1 + j) as f64 / 2.0;
        for item in &c[i..j] {
            if item.1 {
                rankx += rank;
            }
        }
        let t = (j - i) as f64;
        if t > 1.0 {
            ties += t * t * t - t;
        }
        i = j;
    }
    let n1 = x.len() as f64;
    let n2 = y.len() as f64;
    let n = n1 + n2;
    let u = rankx - n1 * (n1 + 1.0) / 2.0;
    let mu = n1 * n2 / 2.0;
    let var = n1 * n2 / 12.0 * ((n + 1.0) - ties / (n * (n - 1.0)));
    if var <= 0.0 {
        return 1.0;
    }
    let z = (u - mu).abs() / var.sqrt();
    (2.0 * (1.0 - normal_cdf(z))).clamp(0.0, 1.0)
}
fn normal_cdf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let z = x.abs() / 2.0_f64.sqrt();
    let t = 1.0 / (1.0 + 0.3275911 * z);
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let erf = 1.0 - (((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t * (-z * z).exp());
    0.5 * (1.0 + sign * erf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robust_linear_model_recovers_group_shift() {
        let design = DesignMatrix {
            sample_ids: (0..10).map(|i| format!("S{i}")).collect(),
            group_case: (0..10).map(|i| i >= 5).collect(),
            subjects: (0..10).map(|i| format!("P{i}")).collect(),
            x: (0..10)
                .map(|i| vec![1.0, if i >= 5 { 1.0 } else { 0.0 }])
                .collect(),
            column_names: vec!["Intercept".into(), "Group".into()],
            group_col: 1,
            dropped_missing_covariates: 0,
        };
        let y = vec![0.0, 0.1, -0.1, 0.05, 0.0, 2.0, 2.1, 1.9, 2.05, 2.0];
        let (b, se, _, p, _, _) = fit_adjusted(&y, &design).unwrap();
        assert!(b > 1.8 && b < 2.2);
        assert!(se.is_finite());
        assert!(p < 0.01);
    }

    #[test]
    fn native_nb_model_recovers_count_fold_change() {
        let design = DesignMatrix {
            sample_ids: (0..10).map(|i| format!("S{i}")).collect(),
            group_case: (0..10).map(|i| i >= 5).collect(),
            subjects: (0..10).map(|i| format!("P{i}")).collect(),
            x: (0..10)
                .map(|i| vec![1.0, if i >= 5 { 1.0 } else { 0.0 }])
                .collect(),
            column_names: vec!["Intercept".into(), "Group".into()],
            group_col: 1,
            dropped_missing_covariates: 0,
        };
        let counts = vec![9.0, 11.0, 10.0, 8.0, 12.0, 38.0, 42.0, 40.0, 36.0, 44.0];
        let sf = vec![1.0; 10];
        let (b, se, _, p, _, _, _) = fit_nb_adjusted(&counts, &sf, &design).unwrap();
        assert!(b > 1.5 && b < 2.5);
        assert!(se.is_finite() && se > 0.0);
        assert!(p < 0.05);
    }

    #[test]
    fn parallel_expression_batch_matches_serial_modeling() {
        let design = DesignMatrix {
            sample_ids: (0..10).map(|i| format!("S{i}")).collect(),
            group_case: (0..10).map(|i| i >= 5).collect(),
            subjects: (0..10).map(|i| format!("P{i}")).collect(),
            x: (0..10)
                .map(|i| vec![1.0, if i >= 5 { 1.0 } else { 0.0 }])
                .collect(),
            column_names: vec!["Intercept".into(), "Group".into()],
            group_col: 1,
            dropped_missing_covariates: 0,
        };
        let rows = (0..300)
            .map(|feature_idx| ExpressionModelWorkRow {
                feature: format!("F{feature_idx}"),
                secondary: String::new(),
                counts: (0..10)
                    .map(|sample_idx| {
                        let base = 8.0 + (feature_idx % 7) as f64;
                        if sample_idx >= 5 { base * 2.0 } else { base }
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        let size_factors = vec![1.0; 10];
        let serial = model_expression_slice(&rows, &size_factors, &design, 1.0, 3);
        let parallel =
            model_expression_batch_parallel(&rows, &size_factors, &design, 1.0, 3).unwrap();
        assert_eq!(serial.1, parallel.1);
        assert_eq!(serial.0.len(), parallel.0.len());
        for (left, right) in serial.0.iter().zip(parallel.0.iter()) {
            assert_eq!(left.feature, right.feature);
            assert!((left.beta_group - right.beta_group).abs() < 1e-12);
            assert!((left.p_value - right.p_value).abs() < 1e-12);
        }
    }

    #[test]
    fn clr_rows_sum_to_zero() {
        let vals = [0.1_f64, 0.2, 0.7];
        let logs = vals.iter().map(|v| (v + 1e-6).ln()).collect::<Vec<_>>();
        let m = mean(&logs);
        let clr = logs.iter().map(|v| v - m).collect::<Vec<_>>();
        assert!(clr.iter().sum::<f64>().abs() < 1e-10);
    }

    #[test]
    fn permanova_pair_precomputation_matches_naive_f_statistic() {
        let dist = vec![
            vec![0.0, 1.0, 2.0, 2.5],
            vec![1.0, 0.0, 1.5, 2.0],
            vec![2.0, 1.5, 0.0, 0.8],
            vec![2.5, 2.0, 0.8, 0.0],
        ];
        let groups = vec![false, false, true, true];
        let naive = {
            let n = groups.len();
            let n_case = groups.iter().filter(|value| **value).count();
            let n_ctrl = n - n_case;
            let mut total = 0.0;
            let mut within_case = 0.0;
            let mut within_ctrl = 0.0;
            for i in 0..n {
                for j in (i + 1)..n {
                    let d2 = dist[i][j] * dist[i][j];
                    total += d2;
                    if groups[i] == groups[j] {
                        if groups[i] {
                            within_case += d2;
                        } else {
                            within_ctrl += d2;
                        }
                    }
                }
            }
            total /= n as f64;
            let within = within_case / n_case as f64 + within_ctrl / n_ctrl as f64;
            ((total - within).max(0.0) / 1.0) / (within / (n as f64 - 2.0))
        };
        let optimized = permanova_f(&dist, &groups);
        assert!((optimized - naive).abs() < 1e-12);
    }

    #[test]
    fn hypergeometric_tail_is_bounded() {
        let p = hypergeom_upper_tail(100, 10, 20, 5);
        assert!(p > 0.0 && p < 1.0);
        assert_eq!(hypergeom_upper_tail(100, 10, 20, 0), 1.0);
    }

    #[test]
    fn rarefaction_is_bounded() {
        let counts = vec![100.0, 50.0, 25.0, 10.0];
        let (rich, sh) = rarefy_metrics(&counts, 50, 3, 1);
        assert!(rich >= 1.0 && rich <= 4.0);
        assert!(sh >= 0.0);
    }

    #[test]
    fn dense_rarefaction_matches_legacy_reference_exactly() {
        fn legacy(counts: &[f64], depth: usize, reps: usize, seed: u64) -> (f64, f64) {
            let integer_counts = counts
                .iter()
                .map(|value| {
                    if value.is_finite() && *value > 0.0 {
                        value.round().max(0.0) as u64
                    } else {
                        0
                    }
                })
                .collect::<Vec<_>>();
            let total = integer_counts.iter().sum::<u64>();
            let k = (depth as u64).min(total) as usize;
            let mut rng = XorShift64::new(seed);
            let mut richness_sum = 0.0;
            let mut shannon_sum = 0.0;
            let mut sampled_per_clone = vec![0u32; integer_counts.len()];
            for _ in 0..reps {
                let mut selected = HashSet::with_capacity(k.saturating_mul(2).max(16));
                let start = total - k as u64;
                for j in start..total {
                    let t = rng.next_bounded(j + 1);
                    if !selected.insert(t) {
                        selected.insert(j);
                    }
                }
                let mut positions = selected.into_iter().collect::<Vec<_>>();
                positions.sort_unstable();
                sampled_per_clone.fill(0);
                let mut pos_i = 0usize;
                let mut cumulative = 0u64;
                for (clone_i, count) in integer_counts.iter().enumerate() {
                    if *count == 0 {
                        continue;
                    }
                    cumulative += *count;
                    while pos_i < positions.len() && positions[pos_i] < cumulative {
                        sampled_per_clone[clone_i] += 1;
                        pos_i += 1;
                    }
                    if pos_i == positions.len() {
                        break;
                    }
                }
                let richness = sampled_per_clone.iter().filter(|value| **value > 0).count() as f64;
                let d = k as f64;
                let shannon = sampled_per_clone
                    .iter()
                    .filter(|value| **value > 0)
                    .map(|value| {
                        let p = *value as f64 / d;
                        -p * p.ln()
                    })
                    .sum::<f64>();
                richness_sum += richness;
                shannon_sum += shannon;
            }
            (richness_sum / reps as f64, shannon_sum / reps as f64)
        }

        let counts = vec![0.0, 100.0, 50.0, f64::NAN, 25.0, 10.0, 1.0];
        for seed in [1, 7, 0xDEADBEEF] {
            let expected = legacy(&counts, 83, 5, seed);
            let observed = rarefy_metrics(&counts, 83, 5, seed);
            assert_eq!(observed, expected);
        }
    }

    fn compact_store(records: &[(&str, &str, &str, f64, Option<&str>)]) -> ParsedClonotypeStore {
        let mut scratch = ParsedClonotypeScratch::default();
        let mut builder = ParsedClonotypeBuilder::with_capacity(records.len(), &mut scratch);
        for (v, j, cdr3, count, nt) in records {
            builder.record(v, j, cdr3, *count, *nt).unwrap();
        }
        builder.finish()
    }

    fn compact_sample(
        sample_id: &str,
        group: &str,
        counts: Vec<f64>,
        records: &[(&str, &str, &str, f64, Option<&str>)],
    ) -> ParsedRepertoire {
        let total = counts.iter().sum::<f64>();
        let top = counts.iter().copied().fold(0.0_f64, f64::max);
        ParsedRepertoire {
            sample_id: sample_id.to_string(),
            group: group.to_string(),
            counts,
            clonotypes: compact_store(records),
            feature_maps: BTreeMap::new(),
            top_clone_fraction: if total > 0.0 { top / total } else { f64::NAN },
            weighted_mean_cdr3_aa_length: 12.0,
        }
    }

    #[test]
    fn arena_clonotype_tracking_preserves_exact_identity_and_convergence() {
        let mut scratch = ParsedClonotypeScratch::default();
        let mut builder = ParsedClonotypeBuilder::with_capacity(4, &mut scratch);
        builder
            .record("TRBV1", "TRBJ1", "CASS", 10.0, Some("AAA"))
            .unwrap();
        builder
            .record("TRBV1", "TRBJ1", "CASS", 5.0, Some("AAA"))
            .unwrap();
        builder
            .record("TRBV1", "TRBJ1", "CASS", 2.0, Some("BBB"))
            .unwrap();
        builder.record("TRBV2", "TRBJ2", "CASR", 3.0, None).unwrap();
        let store = builder.finish();

        assert_eq!(store.len(), 2);
        let mut seen = store
            .iter()
            .map(|(key, entry)| {
                (
                    String::from_utf8(key.0.to_vec()).unwrap(),
                    entry.count,
                    entry.convergent,
                )
            })
            .collect::<Vec<_>>();
        seen.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(seen[0], ("TRBV1|TRBJ1|CASS".to_string(), 17.0, true));
        assert_eq!(seen[1], ("TRBV2|TRBJ2|CASR".to_string(), 3.0, false));
        assert!(store.key_bytes.capacity() >= store.key_bytes.len());
    }

    #[test]
    fn hash_collision_does_not_merge_exact_clonotype_identity() {
        let mut scratch = ParsedClonotypeScratch::default();
        let mut builder = ParsedClonotypeBuilder::with_capacity(3, &mut scratch);
        builder
            .record_hashed(7, "TRBV1", "TRBJ1", "CASSA", 10.0, Some("AAA"))
            .unwrap();
        builder
            .record_hashed(7, "TRBV2", "TRBJ2", "CASSB", 20.0, Some("BBB"))
            .unwrap();
        builder
            .record_hashed(7, "TRBV1", "TRBJ1", "CASSA", 5.0, Some("CCC"))
            .unwrap();
        let store = builder.finish();
        let mut seen = store
            .iter()
            .map(|(key, entry)| {
                (
                    String::from_utf8(key.0.to_vec()).unwrap(),
                    entry.count,
                    entry.convergent,
                )
            })
            .collect::<Vec<_>>();
        seen.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], ("TRBV1|TRBJ1|CASSA".to_string(), 15.0, true));
        assert_eq!(seen[1], ("TRBV2|TRBJ2|CASSB".to_string(), 20.0, false));
    }

    #[test]
    fn exact_numeric_ids_survive_forced_hash_collisions() {
        let mut left_scratch = ParsedClonotypeScratch::default();
        let mut left_builder = ParsedClonotypeBuilder::with_capacity(2, &mut left_scratch);
        left_builder
            .record_hashed(7, "TRBV1", "TRBJ1", "CASSA", 10.0, Some("AAA"))
            .unwrap();
        left_builder
            .record_hashed(7, "TRBV2", "TRBJ2", "CASSB", 20.0, Some("BBB"))
            .unwrap();

        let mut right_scratch = ParsedClonotypeScratch::default();
        let mut right_builder = ParsedClonotypeBuilder::with_capacity(2, &mut right_scratch);
        right_builder
            .record_hashed(7, "TRBV1", "TRBJ1", "CASSA", 30.0, Some("CCC"))
            .unwrap();
        right_builder
            .record_hashed(7, "TRBV3", "TRBJ3", "CASSC", 40.0, Some("DDD"))
            .unwrap();

        let mut samples = vec![
            ParsedRepertoire {
                sample_id: "S1".into(),
                group: "Control".into(),
                counts: vec![10.0, 20.0],
                clonotypes: left_builder.finish(),
                feature_maps: BTreeMap::new(),
                top_clone_fraction: 2.0 / 3.0,
                weighted_mean_cdr3_aa_length: 5.0,
            },
            ParsedRepertoire {
                sample_id: "S2".into(),
                group: "Case".into(),
                counts: vec![30.0, 40.0],
                clonotypes: right_builder.finish(),
                feature_maps: BTreeMap::new(),
                top_clone_fraction: 4.0 / 7.0,
                weighted_mean_cdr3_aa_length: 5.0,
            },
        ];

        let universe = assign_exact_clonotype_ids(&mut samples).unwrap();
        assert_eq!(universe, 3);
        assert!(
            samples
                .iter()
                .all(|sample| sample.clonotypes.key_bytes.is_empty())
        );

        let left_a = samples[0].clonotypes.entries[0].clonotype_id;
        let left_b = samples[0].clonotypes.entries[1].clonotype_id;
        let right_a = samples[1].clonotypes.entries[0].clonotype_id;
        let right_c = samples[1].clonotypes.entries[1].clonotype_id;
        assert_eq!(left_a, right_a);
        assert_ne!(left_a, left_b);
        assert_ne!(left_a, right_c);
        assert_ne!(left_b, right_c);
    }

    #[test]
    fn sharing_fractions_preserve_private_longitudinal_and_public_semantics() {
        let mut samples = vec![
            compact_sample(
                "S1",
                "Control",
                vec![60.0, 20.0, 20.0],
                &[
                    ("V", "J", "PUBLIC", 60.0, Some("N1")),
                    ("V", "J", "LONG", 20.0, Some("N2")),
                    ("V", "J", "PRIVATE1", 20.0, Some("N3")),
                ],
            ),
            compact_sample(
                "S2",
                "Control",
                vec![50.0, 30.0, 20.0],
                &[
                    ("V", "J", "PUBLIC", 50.0, Some("N4")),
                    ("V", "J", "LONG", 30.0, Some("N5")),
                    ("V", "J", "PRIVATE2", 20.0, Some("N6")),
                ],
            ),
            compact_sample(
                "S3",
                "Case",
                vec![25.0, 75.0],
                &[
                    ("V", "J", "PUBLIC", 25.0, Some("N7")),
                    ("V", "J", "PRIVATE3", 75.0, Some("N8")),
                ],
            ),
        ];
        let subjects = vec!["P1".to_string(), "P1".to_string(), "P2".to_string()];
        let clonotype_universe = assign_exact_clonotype_ids(&mut samples).unwrap();
        let sharing = sharing_fractions(&samples, &subjects, true, clonotype_universe);
        let expected = [(0.20, 0.20, 0.60), (0.20, 0.30, 0.50), (0.75, 0.0, 0.25)];
        for (actual, expected) in sharing.iter().zip(expected) {
            assert!((actual.0 - expected.0).abs() < 1e-12);
            assert!((actual.1 - expected.1).abs() < 1e-12);
            assert!((actual.2 - expected.2).abs() < 1e-12);
        }

        let sample_based = sharing_fractions(&samples, &subjects, false, clonotype_universe);
        assert!((sample_based[0].0 - 0.40).abs() < 1e-12);
        assert_eq!(sample_based[0].1, 0.0);
        assert!((sample_based[0].2 - 0.60).abs() < 1e-12);
    }

    #[test]
    fn compact_distance_matrices_match_legacy_morisita_and_jaccard_values() {
        let mut samples = vec![
            compact_sample(
                "S1",
                "Control",
                vec![2.0, 1.0],
                &[
                    ("V", "J", "A", 2.0, Some("N1")),
                    ("V", "J", "B", 1.0, Some("N2")),
                ],
            ),
            compact_sample(
                "S2",
                "Case",
                vec![1.0, 1.0],
                &[
                    ("V", "J", "A", 1.0, Some("N3")),
                    ("V", "J", "C", 1.0, Some("N4")),
                ],
            ),
        ];
        let clonotype_universe = assign_exact_clonotype_ids(&mut samples).unwrap();
        let (morisita, jaccard) = repertoire_distance_matrices(&samples, clonotype_universe);
        assert!(morisita[0][0].abs() < 1e-12);
        assert!(morisita[1][1].abs() < 1e-12);
        assert!((morisita[0][1] - 7.0 / 19.0).abs() < 1e-12);
        assert!((morisita[1][0] - 7.0 / 19.0).abs() < 1e-12);
        assert!((jaccard[0][1] - 2.0 / 3.0).abs() < 1e-12);
        assert!((jaccard[1][0] - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn repertoire_sample_summary_preserves_diversity_and_convergence_content() {
        let sample = compact_sample(
            "S1",
            "Case",
            vec![100.0, 50.0, 25.0, 10.0],
            &[
                ("V1", "J1", "A", 100.0, Some("N1")),
                ("V1", "J1", "A", 25.0, Some("N2")),
                ("V2", "J2", "B", 60.0, Some("N3")),
            ],
        );
        let sharing = (0.25, 0.50, 0.25);
        let summary = repertoire_sample_summary(&sample, "P1", sharing, 50, 3);
        let (expected_richness, expected_shannon) =
            rarefy_metrics(&sample.counts, 50, 3, stable_seed("S1"));
        assert_eq!(summary.sample_id, "S1");
        assert_eq!(summary.group, "Case");
        assert_eq!(summary.subject, "P1");
        assert_eq!(summary.total_clone_count, 185.0);
        assert_eq!(summary.observed_richness, 4.0);
        assert!((summary.rarefied_richness - expected_richness).abs() < 1e-12);
        assert!((summary.rarefied_shannon - expected_shannon).abs() < 1e-12);
        assert_eq!(summary.private_fraction, 0.25);
        assert_eq!(summary.longitudinal_shared_fraction, 0.50);
        assert_eq!(summary.public_fraction, 0.25);
        assert!((summary.convergent_read_fraction - 125.0 / 185.0).abs() < 1e-12);
    }

    #[test]
    fn repertoire_parser_scratch_reuses_capacity_without_cross_sample_state() {
        let mut scratch = ParsedClonotypeScratch::default();
        let first = {
            let mut builder = ParsedClonotypeBuilder::with_capacity(4, &mut scratch);
            builder
                .record("TRBV1", "TRBJ1", "CASSA", 10.0, Some("AAA"))
                .unwrap();
            builder
                .record("TRBV1", "TRBJ1", "CASSA", 5.0, Some("BBB"))
                .unwrap();
            builder.finish()
        };
        let hash_capacity = scratch.hash_heads.capacity();
        let nt_capacity = scratch.nt_bytes.capacity();
        assert!(hash_capacity > 0);
        assert!(nt_capacity > 0);

        scratch.reset();
        assert_eq!(scratch.hash_heads.len(), 0);
        assert_eq!(scratch.nt_bytes.len(), 0);
        assert_eq!(scratch.hash_heads.capacity(), hash_capacity);
        assert_eq!(scratch.nt_bytes.capacity(), nt_capacity);

        let second = {
            let mut builder = ParsedClonotypeBuilder::with_capacity(2, &mut scratch);
            builder
                .record("TRBV2", "TRBJ2", "CASSB", 7.0, Some("CCC"))
                .unwrap();
            builder.finish()
        };
        assert_eq!(first.len(), 1);
        assert!(first.entries[0].convergent);
        assert_eq!(second.len(), 1);
        assert!(!second.entries[0].convergent);
        assert_eq!(second.entries[0].count, 7.0);
    }

    #[test]
    fn repertoire_working_set_cleanup_accepts_bulk_owned_samples() {
        let samples = (0..16)
            .map(|i| {
                let cdr3 = format!("CASS{i}");
                let nt = format!("NT{i}");
                let mut scratch = ParsedClonotypeScratch::default();
                let mut builder = ParsedClonotypeBuilder::with_capacity(1, &mut scratch);
                builder.record("V", "J", &cdr3, 1.0, Some(&nt)).unwrap();
                ParsedRepertoire {
                    sample_id: format!("S{i}"),
                    group: "Case".to_string(),
                    counts: vec![1.0],
                    clonotypes: builder.finish(),
                    feature_maps: BTreeMap::new(),
                    top_clone_fraction: 1.0,
                    weighted_mean_cdr3_aa_length: 5.0,
                }
            })
            .collect::<Vec<_>>();
        release_repertoire_working_set(samples);
    }

    #[test]
    fn parallel_repertoire_summary_preserves_final_sample_order_and_content() {
        let mut samples = Vec::new();
        let mut subjects = Vec::new();
        for i in 0..16 {
            let cdr3_a = format!("CASS{i}A");
            let cdr3_b = format!("CASS{i}B");
            let nt_a1 = format!("NT{i}A1");
            let nt_a2 = format!("NT{i}A2");
            let nt_b = format!("NT{i}B1");
            let mut scratch = ParsedClonotypeScratch::default();
            let mut builder = ParsedClonotypeBuilder::with_capacity(3, &mut scratch);
            builder
                .record("V1", "J1", &cdr3_a, 75.0, Some(&nt_a1))
                .unwrap();
            builder
                .record("V1", "J1", &cdr3_a, 25.0, Some(&nt_a2))
                .unwrap();
            builder
                .record("V2", "J2", &cdr3_b, 50.0, Some(&nt_b))
                .unwrap();
            samples.push(ParsedRepertoire {
                sample_id: format!("S{i}"),
                group: if i >= 8 {
                    "Case".into()
                } else {
                    "Control".into()
                },
                counts: vec![100.0, 50.0, 25.0, 10.0],
                clonotypes: builder.finish(),
                feature_maps: BTreeMap::new(),
                top_clone_fraction: 100.0 / 185.0,
                weighted_mean_cdr3_aa_length: 12.0,
            });
            subjects.push(format!("P{i}"));
        }
        let clonotype_universe = assign_exact_clonotype_ids(&mut samples).unwrap();
        let sharing = sharing_fractions(&samples, &subjects, true, clonotype_universe);
        let expected = samples
            .iter()
            .enumerate()
            .map(|(i, sample)| repertoire_sample_summary(sample, &subjects[i], sharing[i], 50, 3))
            .collect::<Vec<_>>();
        let mut progress_calls = 0usize;
        let actual = build_repertoire_sample_summaries(
            &samples,
            &subjects,
            &sharing,
            50,
            3,
            "TCR beta",
            &mut |_, _, _| progress_calls += 1,
        )
        .unwrap();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(progress_calls, samples.len());
        for (index, (left, right)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(left.sample_id, format!("S{index}"));
            assert_eq!(left.sample_id, right.sample_id);
            assert_eq!(left.group, right.group);
            assert!((left.rarefied_richness - right.rarefied_richness).abs() < 1e-12);
            assert!((left.rarefied_shannon - right.rarefied_shannon).abs() < 1e-12);
            assert!((left.convergent_read_fraction - right.convergent_read_fraction).abs() < 1e-12);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_status_memory_parser_extracts_kib_values() {
        let status = "Name:\tcfs-server\nVmPeak:\t  123456 kB\nVmSize:\t  120000 kB\nVmRSS:\t   64000 kB\nVmSwap:\t    8192 kB\nMemAvailable:\t 2048000 kB\nSwapFree:\t 1024000 kB\n";
        assert_eq!(proc_status_kib(status, "VmRSS:"), Some(64_000));
        assert_eq!(proc_status_kib(status, "VmSize:"), Some(120_000));
        assert_eq!(proc_status_kib(status, "VmPeak:"), Some(123_456));
        assert_eq!(proc_status_kib(status, "VmSwap:"), Some(8_192));
        assert_eq!(proc_status_kib(status, "MemAvailable:"), Some(2_048_000));
        assert_eq!(proc_status_kib(status, "SwapFree:"), Some(1_024_000));
    }

    #[test]
    fn lightweight_repertoire_qc_summary_math_matches_legacy_formulas() {
        let counts = vec![10.0, 5.0, 1.0, 4.0];
        let metrics = repertoire_qc_metrics_from_counts(&counts, 0.5, 13.25).unwrap();
        let by_name = metrics.into_iter().collect::<HashMap<_, _>>();

        let total = counts.iter().copied().sum::<f64>();
        let probs = counts.iter().map(|count| count / total).collect::<Vec<_>>();
        let richness = probs.len() as f64;
        let shannon = -probs.iter().map(|p| p * p.ln()).sum::<f64>();
        let simpson = 1.0 - probs.iter().map(|p| p * p).sum::<f64>();
        let clonality = 1.0 - shannon / richness.ln();

        assert_eq!(by_name["Total clone count"], total);
        assert_eq!(by_name["Observed richness"], richness);
        assert!((by_name["Shannon diversity"] - shannon).abs() < 1e-12);
        assert!((by_name["Simpson diversity"] - simpson).abs() < 1e-12);
        assert!((by_name["Clonality"] - clonality).abs() < 1e-12);
        assert_eq!(by_name["Top clone fraction"], 0.5);
        assert_eq!(by_name["Weighted mean CDR3 AA length"], 13.25);
    }

    #[test]
    fn average_linkage_dendrogram_contains_every_sample() {
        let design = DesignMatrix {
            sample_ids: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            group_case: vec![false, false, true, true],
            subjects: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            x: vec![
                vec![1.0, 0.0],
                vec![1.0, 0.0],
                vec![1.0, 1.0],
                vec![1.0, 1.0],
            ],
            column_names: vec!["Intercept".into(), "Group".into()],
            group_col: 1,
            dropped_missing_covariates: 0,
        };
        let dist = vec![
            vec![0.0, 0.1, 0.9, 1.0],
            vec![0.1, 0.0, 0.8, 0.9],
            vec![0.9, 0.8, 0.0, 0.2],
            vec![1.0, 0.9, 0.2, 0.0],
        ];
        let tree = average_linkage_dendrogram(&dist, &design, "test", "distance").unwrap();
        assert_eq!(tree.leaves.len(), 4);
        assert_eq!(tree.segments.len(), 9);
        let ids = tree
            .leaves
            .iter()
            .map(|l| l.sample_id.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 4);
    }
}

#[derive(Serialize)]
struct AdjustedFeatureExportRow<'a> {
    feature: &'a str,
    secondary_id: &'a str,
    n_samples: usize,
    n_subjects: usize,
    n_case: usize,
    n_control: usize,
    mean_case: f64,
    mean_control: f64,
    beta_group: f64,
    robust_se: f64,
    z_value: f64,
    p_value: f64,
    q_value: f64,
    subject_effect: f64,
    subject_p_value: f64,
    subject_q_value: f64,
    method: &'a str,
}

pub fn write_adjusted_tests(path: &Path, tests: &[AdjustedFeatureResult]) -> Result<()> {
    // This writer is used by the largest technical exports (especially
    // transcript abundance and DTU).  Serializing borrowed rows avoids the
    // previous 17 String allocations/clones per feature while preserving
    // csv's escaping and numeric formatting semantics.
    let mut w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .buffer_capacity(1024 * 1024)
        .from_path(path)?;
    for r in tests {
        w.serialize(AdjustedFeatureExportRow {
            feature: &r.feature,
            secondary_id: &r.secondary_id,
            n_samples: r.n_samples,
            n_subjects: r.n_subjects,
            n_case: r.n_case,
            n_control: r.n_control,
            mean_case: r.mean_case,
            mean_control: r.mean_control,
            beta_group: r.beta_group,
            robust_se: r.robust_se,
            z_value: r.z_value,
            p_value: r.p_value,
            q_value: r.q_value,
            subject_effect: r.subject_effect,
            subject_p_value: r.subject_p_value,
            subject_q_value: r.subject_q_value,
            method: &r.method,
        })?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_pathway_ora(path: &Path, result: &CorrectedPathwayResult) -> Result<()> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(path)?;
    w.write_record([
        "pathway",
        "description",
        "measured_set_size",
        "overlap",
        "odds_ratio",
        "p_value",
        "q_value",
        "overlap_genes",
    ])?;
    for r in &result.tests {
        w.serialize((
            &r.pathway,
            &r.description,
            r.measured_set_size,
            r.overlap,
            r.odds_ratio,
            r.p_value,
            r.q_value,
            &r.overlap_genes,
        ))?;
    }
    w.flush()?;
    Ok(())
}

#[derive(Serialize)]
struct DtuGeneExportRow<'a> {
    gene_id: &'a str,
    n_transcripts: usize,
    p_value_simes: f64,
    q_value_bh: f64,
    max_abs_clr_effect: f64,
}

pub fn write_dtu_gene_tests(path: &Path, tests: &[DtuGeneResult]) -> Result<()> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .buffer_capacity(1024 * 1024)
        .from_path(path)?;
    for r in tests {
        w.serialize(DtuGeneExportRow {
            gene_id: &r.gene_id,
            n_transcripts: r.n_transcripts,
            p_value_simes: r.p_value,
            q_value_bh: r.q_value,
            max_abs_clr_effect: r.max_abs_clr_effect,
        })?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_ordination(path: &Path, ord: &OrdinationResult) -> Result<()> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(path)?;
    w.write_record(["sample_id", "group", "subject", "axis1", "axis2"])?;
    for p in &ord.points {
        w.serialize((&p.sample_id, &p.group, &p.subject, p.x, p.y))?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_dendrogram(path: &Path, tree: &DendrogramResult) -> Result<()> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(path)?;
    w.write_record([
        "leaf_order",
        "sample_id",
        "group",
        "subject",
        "linkage",
        "distance_label",
    ])?;
    for (i, leaf) in tree.leaves.iter().enumerate() {
        w.serialize((
            i + 1,
            &leaf.sample_id,
            &leaf.group,
            &leaf.subject,
            &tree.linkage,
            &tree.distance_label,
        ))?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_corrected_repertoire_samples(
    path: &Path,
    rows: &[CorrectedRepertoireSample],
) -> Result<()> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(path)?;
    w.write_record([
        "sample_id",
        "group",
        "subject",
        "total_clone_count",
        "observed_richness",
        "shannon",
        "clonality",
        "rarefied_richness",
        "rarefied_shannon",
        "top_clone_fraction",
        "weighted_mean_cdr3_aa_length",
        "private_fraction",
        "longitudinal_shared_fraction",
        "public_fraction",
        "convergent_read_fraction",
    ])?;
    for r in rows {
        w.serialize((
            &r.sample_id,
            &r.group,
            &r.subject,
            r.total_clone_count,
            r.observed_richness,
            r.shannon,
            r.clonality,
            r.rarefied_richness,
            r.rarefied_shannon,
            r.top_clone_fraction,
            r.weighted_mean_cdr3_aa_length,
            r.private_fraction,
            r.longitudinal_shared_fraction,
            r.public_fraction,
            r.convergent_read_fraction,
        ))?;
    }
    w.flush()?;
    Ok(())
}
