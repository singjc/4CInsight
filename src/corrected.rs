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
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

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

pub fn run_corrected_expression<F>(
    merged_tsv: &Path,
    metadata: &MetadataTable,
    comparison: &CohortComparison,
    source_metric: &str,
    feature_label: &str,
    spec: &CorrectedModelSpec,
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

    let total_rows = count_delimited_rows(merged_tsv, b'\t')?.saturating_sub(1);
    // Pass 1: DESeq2-style `poscounts` median-ratio size factors. P² keeps the
    // median estimate bounded-memory even for transcript-scale matrices.
    let mut medians = vec![P2Median::new(); design_cols.len()];
    let mut pass1 = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    for (row_idx, record) in pass1.records().enumerate() {
        let record = record?;
        let counts = values_at_columns_with_zero(&record, &design_cols);
        let positive = counts
            .iter()
            .copied()
            .filter(|x| *x > 0.0)
            .collect::<Vec<_>>();
        if positive.len() >= 2 {
            let gmean =
                (positive.iter().map(|x| x.ln()).sum::<f64>() / positive.len() as f64).exp();
            if gmean > 0.0 && gmean.is_finite() {
                for (j, count) in counts.iter().enumerate() {
                    if *count > 0.0 {
                        medians[j].push(*count / gmean);
                    }
                }
            }
        }
        if row_idx % 1000 == 0 {
            progress(
                row_idx + 1,
                (total_rows * 2).max(1),
                &format!("{feature_label}: estimating count size factors"),
            );
        }
    }
    let mut size_factors = medians
        .iter()
        .map(|m| {
            m.value()
                .filter(|v| *v > 0.0 && v.is_finite())
                .unwrap_or(1.0)
        })
        .collect::<Vec<_>>();
    let sf_gmean =
        (size_factors.iter().map(|x| x.ln()).sum::<f64>() / size_factors.len() as f64).exp();
    if sf_gmean.is_finite() && sf_gmean > 0.0 {
        for sf in &mut size_factors {
            *sf /= sf_gmean;
        }
    }

    let min_present = ((design.x.len() as f64 * spec.min_fraction_samples).ceil() as usize).max(3);
    let mut tests = Vec::new();
    let mut filtered = 0usize;
    let mut top_variable: Vec<VariableRow> = Vec::new();
    let mut pass2 = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    for (row_idx, record) in pass2.records().enumerate() {
        let record = record?;
        let feature = record.get(0).unwrap_or_default().to_string();
        let secondary = record.get(1).unwrap_or_default().to_string();
        let counts = values_at_columns_with_zero(&record, &design_cols);
        let normalized = counts
            .iter()
            .zip(size_factors.iter())
            .map(|(c, sf)| c / sf.max(1e-12))
            .collect::<Vec<_>>();
        if normalized.iter().filter(|v| **v >= spec.min_count).count() < min_present {
            filtered += 1;
            continue;
        }
        let y = normalized
            .iter()
            .map(|x| (x + 0.5).log2())
            .collect::<Vec<_>>();
        if let Some(test) = make_nb_adjusted_test(
            feature,
            secondary,
            &counts,
            &normalized,
            &size_factors,
            &design,
        ) {
            tests.push(test);
        }
        let v = sample_variance(&y);
        if v.is_finite() {
            top_variable.push(VariableRow {
                variance: v,
                values: y,
            });
            if top_variable.len() > 700 {
                top_variable.sort_by(|a, b| b.variance.total_cmp(&a.variance));
                top_variable.truncate(350);
            }
        }
        if row_idx % 500 == 0 {
            progress(
                total_rows + row_idx + 1,
                (total_rows * 2).max(1),
                &format!("{feature_label}: adjusted feature models"),
            );
        }
    }
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
    let dtu = if feature_label.eq_ignore_ascii_case("Transcript") {
        match run_dtu_screen(merged_tsv, &design_cols, &design, spec) {
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
        total_rows * 2,
        total_rows * 2,
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

fn run_dtu_screen(
    merged_tsv: &Path,
    design_cols: &[usize],
    design: &DesignMatrix,
    spec: &CorrectedModelSpec,
) -> Result<CorrectedDtuResult> {
    let n = design_cols.len();
    let pseudo = 0.5_f64;
    let min_present = ((n as f64 * spec.min_fraction_samples).ceil() as usize).max(3);
    let mut transcript_counts: HashMap<String, usize> = HashMap::new();
    let mut gene_totals: HashMap<String, Vec<f64>> = HashMap::new();
    let mut gene_log_sums: HashMap<String, Vec<f64>> = HashMap::new();

    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .from_path(merged_tsv)?;
    for record in reader.records() {
        let record = record?;
        let gene = record.get(1).unwrap_or_default().trim();
        if gene.is_empty() {
            continue;
        }
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
    for record in reader.records() {
        let record = record?;
        let transcript = record.get(0).unwrap_or_default().trim();
        let gene = record.get(1).unwrap_or_default().trim();
        if transcript.is_empty() || !eligible.contains(gene) {
            continue;
        }
        let n_tx = *transcript_counts.get(gene).unwrap_or(&1) as f64;
        let counts = values_at_columns_with_zero(&record, design_cols);
        let Some(log_sums) = gene_log_sums.get(gene) else {
            continue;
        };
        let Some(totals) = gene_totals.get(gene) else {
            continue;
        };
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
        if mean(&usage) < 0.001 && usage.iter().filter(|v| **v > 0.0).count() < min_present {
            continue;
        }
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

#[derive(Debug, Clone)]
struct ParsedRepertoire {
    sample_id: String,
    group: String,
    counts: Vec<f64>,
    clonotype_aa: HashMap<String, f64>,
    aa_to_nt: HashMap<String, HashSet<String>>,
    aa_read_counts: HashMap<String, f64>,
    feature_maps: BTreeMap<String, HashMap<String, f64>>,
    top_clone_fraction: f64,
    weighted_mean_cdr3_aa_length: f64,
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
            &format!("{label}: parsing {}", file.sample_id),
        );
        match parse_repertoire_file(
            &file.path,
            &file.sample_id,
            &group,
            modality,
            spec.ambiguous_policy,
        ) {
            Ok(p) => parsed.push(p),
            Err(e) => warnings.push(format!("{}: {e:#}", file.path.display())),
        }
    }
    if parsed.len() < 4 {
        bail!(
            "Only {} usable {label} samples belong to this comparison",
            parsed.len()
        );
    }
    let sample_ids = parsed
        .iter()
        .map(|p| p.sample_id.clone())
        .collect::<Vec<_>>();
    let design = build_design(metadata, comparison, &sample_ids, spec)?;
    // Keep only complete-case model samples, preserving design order.
    let by_id = parsed
        .into_iter()
        .map(|p| (normalize_sample_id(&p.sample_id), p))
        .collect::<HashMap<_, _>>();
    let mut model_samples = Vec::new();
    for id in &design.sample_ids {
        if let Some(p) = by_id.get(&normalize_sample_id(id)) {
            model_samples.push(p.clone());
        }
    }
    if model_samples.len() != design.sample_ids.len() {
        bail!("Could not align repertoire samples to adjusted metadata design");
    }

    let min_total = model_samples
        .iter()
        .map(|p| p.counts.iter().sum::<f64>().round().max(1.0) as usize)
        .min()
        .unwrap_or(1);
    let matched_depth = min_total.min(spec.rarefaction_cap.max(100));
    let subjects = design.subjects.clone();
    let sharing = sharing_fractions(&model_samples, &subjects, spec.subject_column.is_some());
    let mut summaries = Vec::new();
    for (i, p) in model_samples.iter().enumerate() {
        progress(
            i + 1,
            model_samples.len(),
            &format!("{label}: matched-depth diversity {}", p.sample_id),
        );
        let (rr, rs) = rarefy_metrics(
            &p.counts,
            matched_depth,
            spec.rarefaction_reps,
            stable_seed(&p.sample_id),
        );
        let total_count = p.counts.iter().sum::<f64>();
        let shannon = shannon_entropy(&p.counts);
        let clonality = if p.counts.len() > 1 {
            1.0 - shannon / (p.counts.len() as f64).ln()
        } else {
            f64::NAN
        };
        let (private, longitudinal, public) =
            sharing
                .get(i)
                .copied()
                .unwrap_or((f64::NAN, f64::NAN, f64::NAN));
        let convergent_reads = p
            .aa_to_nt
            .iter()
            .filter(|(_, nts)| nts.len() > 1)
            .map(|(aa, _)| p.aa_read_counts.get(aa).copied().unwrap_or(0.0))
            .sum::<f64>();
        summaries.push(CorrectedRepertoireSample {
            sample_id: p.sample_id.clone(),
            group: p.group.clone(),
            subject: subjects[i].clone(),
            total_clone_count: total_count,
            observed_richness: p.counts.len() as f64,
            shannon,
            clonality,
            rarefied_richness: rr,
            rarefied_shannon: rs,
            top_clone_fraction: p.top_clone_fraction,
            weighted_mean_cdr3_aa_length: p.weighted_mean_cdr3_aa_length,
            private_fraction: private,
            longitudinal_shared_fraction: longitudinal,
            public_fraction: public,
            convergent_read_fraction: if total_count > 0.0 {
                convergent_reads / total_count
            } else {
                f64::NAN
            },
        });
    }
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

    let mut compositions = Vec::new();
    let families: Vec<&str> = match modality {
        CohortModality::Bcr => vec!["Isotype", "V gene", "D gene", "J gene"],
        _ => vec!["V gene", "J gene", "V-J pair"],
    };
    for family in families {
        if let Some(comp) = composition_analysis(family, &model_samples, &design, spec) {
            compositions.push(comp);
        }
    }

    let morisita = repertoire_ordination(
        &model_samples,
        &design,
        SimilarityMetric::MorisitaHorn,
        spec.permutations,
        &format!("{label} Morisita-Horn PCoA"),
    );
    let jaccard = repertoire_ordination(
        &model_samples,
        &design,
        SimilarityMetric::Jaccard,
        spec.permutations,
        &format!("{label} Jaccard PCoA"),
    );
    let dendrogram = {
        let distances = repertoire_distance_matrix(&model_samples, SimilarityMetric::MorisitaHorn);
        average_linkage_dendrogram(
            &distances,
            &design,
            &format!("{label} hierarchical clustering — Morisita-Horn distance"),
            "1 - Morisita-Horn similarity",
        )
    };
    if spec.subject_column.is_none() {
        warnings.push("No subject ID column was selected: public/private sharing is sample-based and PERMANOVA permutations are unrestricted. Select a patient/subject column for longitudinally aware results.".to_string());
    }
    Ok(CorrectedRepertoireResult {
        modality: label.to_string(),
        files_used: model_samples.len(),
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
    })
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
            clonotype_aa: HashMap::new(),
            aa_to_nt: HashMap::new(),
            aa_read_counts: HashMap::new(),
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

fn composition_analysis(
    family: &str,
    samples: &[ParsedRepertoire],
    design: &DesignMatrix,
    spec: &CorrectedModelSpec,
) -> Option<CorrectedCompositionResult> {
    let maps = samples
        .iter()
        .map(|p| p.feature_maps.get(family).cloned().unwrap_or_default())
        .collect::<Vec<_>>();
    let all_features = maps
        .iter()
        .flat_map(|m| m.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut retained = Vec::new();
    for feature in all_features {
        let vals = maps
            .iter()
            .map(|m| *m.get(&feature).unwrap_or(&0.0))
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
            .map(|f| (map.get(f).copied().unwrap_or(0.0) + pseudo).ln())
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
            .map(|m| *m.get(feature).unwrap_or(&0.0))
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
        tests,
        ordination: ord,
        dendrogram,
    })
}

#[derive(Debug, Clone, Copy)]
enum SimilarityMetric {
    MorisitaHorn,
    Jaccard,
}

fn repertoire_distance_matrix(
    samples: &[ParsedRepertoire],
    metric: SimilarityMetric,
) -> Vec<Vec<f64>> {
    let n = samples.len();
    if n == 0 {
        return Vec::new();
    }
    let mut inverted: HashMap<String, Vec<(usize, f64)>> = HashMap::new();
    let mut sums_sq = vec![0.0; n];
    let mut richness = vec![0usize; n];
    for (i, sample) in samples.iter().enumerate() {
        let total = sample.clonotype_aa.values().sum::<f64>();
        if total <= 0.0 {
            continue;
        }
        richness[i] = sample.clonotype_aa.len();
        for (clono, count) in &sample.clonotype_aa {
            let p = count / total;
            sums_sq[i] += p * p;
            inverted.entry(clono.clone()).or_default().push((i, p));
        }
    }
    let mut dotm = vec![vec![0.0; n]; n];
    let mut shared = vec![vec![0usize; n]; n];
    for entries in inverted.values() {
        for a in 0..entries.len() {
            let (i, pi) = entries[a];
            for b in a..entries.len() {
                let (j, pj) = entries[b];
                dotm[i][j] += pi * pj;
                if i != j {
                    dotm[j][i] += pi * pj;
                }
                shared[i][j] += 1;
                if i != j {
                    shared[j][i] += 1;
                }
            }
        }
    }
    let mut dist = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..n {
            let sim = match metric {
                SimilarityMetric::MorisitaHorn => {
                    let den = sums_sq[i] + sums_sq[j];
                    if den > 0.0 {
                        (2.0 * dotm[i][j] / den).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                }
                SimilarityMetric::Jaccard => {
                    let union = richness[i] + richness[j] - shared[i][j];
                    if union > 0 {
                        shared[i][j] as f64 / union as f64
                    } else {
                        0.0
                    }
                }
            };
            dist[i][j] = (1.0 - sim).max(0.0);
        }
    }
    dist
}

fn repertoire_ordination(
    samples: &[ParsedRepertoire],
    design: &DesignMatrix,
    metric: SimilarityMetric,
    permutations: usize,
    title: &str,
) -> Option<OrdinationResult> {
    let dist = repertoire_distance_matrix(samples, metric);
    if dist.len() < 3 {
        return None;
    }
    pcoa_from_distance(&dist, design, title, permutations, stable_seed(title))
}

fn sharing_fractions(
    samples: &[ParsedRepertoire],
    subjects: &[String],
    subject_aware: bool,
) -> Vec<(f64, f64, f64)> {
    let mut prevalence: HashMap<String, (HashSet<String>, usize)> = HashMap::new();
    for (i, sample) in samples.iter().enumerate() {
        for clono in sample.clonotype_aa.keys() {
            let entry = prevalence
                .entry(clono.clone())
                .or_insert_with(|| (HashSet::new(), 0));
            entry.0.insert(subjects[i].clone());
            entry.1 += 1;
        }
    }
    samples
        .iter()
        .enumerate()
        .map(|(i, sample)| {
            let total = sample.clonotype_aa.values().sum::<f64>();
            if total <= 0.0 {
                return (f64::NAN, f64::NAN, f64::NAN);
            }
            let mut private = 0.0;
            let mut longitudinal = 0.0;
            let mut public = 0.0;
            for (clono, count) in &sample.clonotype_aa {
                let (subj, n_samples) = prevalence.get(clono).cloned().unwrap_or_default();
                if subj.len() >= 2 {
                    public += count;
                } else if subject_aware && n_samples >= 2 && subj.contains(&subjects[i]) {
                    longitudinal += count;
                } else {
                    private += count;
                }
            }
            (private / total, longitudinal / total, public / total)
        })
        .collect()
}

fn parse_repertoire_file(
    path: &Path,
    sample_id: &str,
    group: &str,
    modality: CohortModality,
    policy: AmbiguousCallPolicy,
) -> Result<ParsedRepertoire> {
    let (headers, rows) = read_first_sheet(path)?;
    let columns = header_map(&headers);
    let count_idx =
        find_header(&columns, &["Clone Count"]).ok_or_else(|| anyhow!("Missing Clone Count"))?;
    let v_idx = find_header(&columns, &["Top V Hits"]);
    let d_idx = find_header(&columns, &["Top D Hits"]);
    let j_idx = find_header(&columns, &["Top J Hits"]);
    let iso_idx = find_header(&columns, &["Isotype"]);
    let cdr3_idx = find_header(&columns, &["CDR3 Amino Acid Sequence"]);
    let nt_idx = find_header(&columns, &["Clonal Sequence"]);
    let mut counts = Vec::new();
    let mut feature_maps: BTreeMap<String, HashMap<String, f64>> = BTreeMap::new();
    for family in match modality {
        CohortModality::Bcr => vec!["Isotype", "V gene", "D gene", "J gene"],
        _ => vec!["V gene", "J gene", "V-J pair"],
    } {
        feature_maps.insert(family.to_string(), HashMap::new());
    }
    let mut clonotype_aa = HashMap::new();
    let mut aa_to_nt: HashMap<String, HashSet<String>> = HashMap::new();
    let mut aa_read_counts = HashMap::new();
    let mut top = 0.0_f64;
    let mut total = 0.0;
    let mut weighted_len = 0.0;
    let mut weighted_len_den = 0.0;
    for row in rows {
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
            let aa_key = format!("{}|{}|{}", v_calls[0].0, j_calls[0].0, cdr3);
            *clonotype_aa.entry(aa_key.clone()).or_insert(0.0) += count;
            *aa_read_counts.entry(aa_key.clone()).or_insert(0.0) += count;
            if let Some(idx) = nt_idx {
                let nt = row.get(idx).map(cell_text).unwrap_or_default();
                if !nt.is_empty() {
                    aa_to_nt.entry(aa_key).or_default().insert(nt);
                }
            }
        }
    }
    if total <= 0.0 {
        bail!("No positive clone counts");
    }
    for map in feature_maps.values_mut() {
        let denom = map.values().sum::<f64>();
        if denom > 0.0 {
            for v in map.values_mut() {
                *v /= denom;
            }
        }
    }
    Ok(ParsedRepertoire {
        sample_id: sample_id.to_string(),
        group: group.to_string(),
        counts,
        clonotype_aa,
        aa_to_nt,
        aa_read_counts,
        feature_maps,
        top_clone_fraction: top / total,
        weighted_mean_cdr3_aa_length: if weighted_len_den > 0.0 {
            weighted_len / weighted_len_den
        } else {
            f64::NAN
        },
    })
}

pub(crate) fn parse_repertoire_qc_metrics(
    path: &Path,
    sample_id: &str,
    modality: CohortModality,
) -> Result<Vec<(String, f64)>> {
    let parsed = parse_repertoire_file(
        path,
        sample_id,
        "QC",
        modality,
        AmbiguousCallPolicy::Exclude,
    )?;
    let total = parsed.counts.iter().copied().sum::<f64>();
    if total <= 0.0 {
        bail!("No positive clone counts");
    }
    let probs = parsed
        .counts
        .iter()
        .copied()
        .filter(|v| *v > 0.0)
        .map(|v| v / total)
        .collect::<Vec<_>>();
    let richness = probs.len() as f64;
    let shannon = -probs
        .iter()
        .copied()
        .filter(|p| *p > 0.0)
        .map(|p| p * p.ln())
        .sum::<f64>();
    let simpson = 1.0 - probs.iter().map(|p| p * p).sum::<f64>();
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
        ("Top clone fraction".to_string(), parsed.top_clone_fraction),
        (
            "Weighted mean CDR3 AA length".to_string(),
            parsed.weighted_mean_cdr3_aa_length,
        ),
    ])
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
    let integer_counts = counts
        .iter()
        .map(|v| {
            if v.is_finite() && *v > 0.0 {
                v.round().max(0.0) as u64
            } else {
                0
            }
        })
        .collect::<Vec<_>>();
    let total = integer_counts.iter().sum::<u64>();
    if total == 0 {
        return (f64::NAN, f64::NAN);
    }
    let k = (depth as u64).min(total) as usize;
    let mut rng = XorShift64::new(seed);
    let mut richness_sum = 0.0;
    let mut shannon_sum = 0.0;
    let mut sampled_per_clone = vec![0u32; integer_counts.len()];

    // Exact rarefaction without replacement. Floyd's algorithm selects k
    // distinct read positions uniformly from 0..total without expanding a
    // potentially huge read-level vector. Sorted positions are then mapped
    // back to clonotypes through cumulative clone counts.
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
        let richness = sampled_per_clone.iter().filter(|v| **v > 0).count() as f64;
        let d = k as f64;
        let shannon = sampled_per_clone
            .iter()
            .filter(|v| **v > 0)
            .map(|v| {
                let p = *v as f64 / d;
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
    let observed = permanova_f(dist, groups);
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
        let f = permanova_f(dist, &perm);
        if f.is_finite() && f >= observed - 1e-12 {
            extreme += 1;
        }
    }
    (
        observed,
        (extreme as f64 + 1.0) / (permutations as f64 + 1.0),
    )
}

fn permanova_f(dist: &[Vec<f64>], groups: &[bool]) -> f64 {
    let n = groups.len();
    let n_case = groups.iter().filter(|x| **x).count();
    let n_ctrl = n - n_case;
    if n_case < 2 || n_ctrl < 2 {
        return f64::NAN;
    }
    let mut total = 0.0;
    for i in 0..n {
        for j in (i + 1)..n {
            total += dist[i][j].powi(2);
        }
    }
    total /= n as f64;
    let mut within_case = 0.0;
    let mut within_ctrl = 0.0;
    for i in 0..n {
        for j in (i + 1)..n {
            if groups[i] == groups[j] {
                if groups[i] {
                    within_case += dist[i][j].powi(2)
                } else {
                    within_ctrl += dist[i][j].powi(2)
                }
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
    let n = dist.len();
    if n < 2 || design.sample_ids.len() != n || dist.iter().any(|row| row.len() != n) {
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
            sample_id: design.sample_ids[i].clone(),
            group: if design.group_case[i] {
                "Case"
            } else {
                "Control"
            }
            .to_string(),
            subject: design.subjects[i].clone(),
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
fn count_delimited_rows(path: &Path, delimiter: u8) -> Result<usize> {
    let mut reader = ReaderBuilder::new().delimiter(delimiter).from_path(path)?;
    let mut n = 1usize;
    for row in reader.records() {
        row?;
        n += 1;
    }
    Ok(n)
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
    fn clr_rows_sum_to_zero() {
        let vals = [0.1_f64, 0.2, 0.7];
        let logs = vals.iter().map(|v| (v + 1e-6).ln()).collect::<Vec<_>>();
        let m = mean(&logs);
        let clr = logs.iter().map(|v| v - m).collect::<Vec<_>>();
        assert!(clr.iter().sum::<f64>().abs() < 1e-10);
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

pub fn write_adjusted_tests(path: &Path, tests: &[AdjustedFeatureResult]) -> Result<()> {
    let mut w = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    w.write_record([
        "feature",
        "secondary_id",
        "n_samples",
        "n_subjects",
        "n_case",
        "n_control",
        "mean_case",
        "mean_control",
        "beta_group",
        "robust_se",
        "z_value",
        "p_value",
        "q_value",
        "subject_effect",
        "subject_p_value",
        "subject_q_value",
        "method",
    ])?;
    for r in tests {
        w.write_record(vec![
            r.feature.clone(),
            r.secondary_id.clone(),
            r.n_samples.to_string(),
            r.n_subjects.to_string(),
            r.n_case.to_string(),
            r.n_control.to_string(),
            r.mean_case.to_string(),
            r.mean_control.to_string(),
            r.beta_group.to_string(),
            r.robust_se.to_string(),
            r.z_value.to_string(),
            r.p_value.to_string(),
            r.q_value.to_string(),
            r.subject_effect.to_string(),
            r.subject_p_value.to_string(),
            r.subject_q_value.to_string(),
            r.method.clone(),
        ])?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_pathway_ora(path: &Path, result: &CorrectedPathwayResult) -> Result<()> {
    let mut w = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
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
        w.write_record([
            r.pathway.clone(),
            r.description.clone(),
            r.measured_set_size.to_string(),
            r.overlap.to_string(),
            r.odds_ratio.to_string(),
            r.p_value.to_string(),
            r.q_value.to_string(),
            r.overlap_genes.clone(),
        ])?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_dtu_gene_tests(path: &Path, tests: &[DtuGeneResult]) -> Result<()> {
    let mut w = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    w.write_record([
        "gene_id",
        "n_transcripts",
        "p_value_simes",
        "q_value_bh",
        "max_abs_clr_effect",
    ])?;
    for r in tests {
        w.write_record([
            r.gene_id.clone(),
            r.n_transcripts.to_string(),
            r.p_value.to_string(),
            r.q_value.to_string(),
            r.max_abs_clr_effect.to_string(),
        ])?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_ordination(path: &Path, ord: &OrdinationResult) -> Result<()> {
    let mut w = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    w.write_record(["sample_id", "group", "subject", "axis1", "axis2"])?;
    for p in &ord.points {
        w.write_record(vec![
            p.sample_id.clone(),
            p.group.clone(),
            p.subject.clone(),
            p.x.to_string(),
            p.y.to_string(),
        ])?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_dendrogram(path: &Path, tree: &DendrogramResult) -> Result<()> {
    let mut w = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
    w.write_record([
        "leaf_order",
        "sample_id",
        "group",
        "subject",
        "linkage",
        "distance_label",
    ])?;
    for (i, leaf) in tree.leaves.iter().enumerate() {
        w.write_record([
            (i + 1).to_string(),
            leaf.sample_id.clone(),
            leaf.group.clone(),
            leaf.subject.clone(),
            tree.linkage.clone(),
            tree.distance_label.clone(),
        ])?;
    }
    w.flush()?;
    Ok(())
}

pub fn write_corrected_repertoire_samples(
    path: &Path,
    rows: &[CorrectedRepertoireSample],
) -> Result<()> {
    let mut w = csv::WriterBuilder::new().delimiter(b'\t').from_path(path)?;
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
        w.write_record(vec![
            r.sample_id.clone(),
            r.group.clone(),
            r.subject.clone(),
            r.total_clone_count.to_string(),
            r.observed_richness.to_string(),
            r.shannon.to_string(),
            r.clonality.to_string(),
            r.rarefied_richness.to_string(),
            r.rarefied_shannon.to_string(),
            r.top_clone_fraction.to_string(),
            r.weighted_mean_cdr3_aa_length.to_string(),
            r.private_fraction.to_string(),
            r.longitudinal_shared_fraction.to_string(),
            r.public_fraction.to_string(),
            r.convergent_read_fraction.to_string(),
        ])?;
    }
    w.flush()?;
    Ok(())
}
