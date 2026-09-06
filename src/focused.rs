//! Candidate prioritization derived from corrected cohort-analysis results.
//!
//! This module is intentionally a presentation-oriented layer on top of the
//! corrected analysis results. Corrected statistics remain the primary evidence.
//! Optional exploratory expression results are used only as a direction-
//! concordance check and never as an independent validation dataset.

use crate::cohort::{FeatureTestResult, GeneComparisonResult};
use crate::corrected::{
    AdjustedFeatureResult, CorrectedExpressionResult, CorrectedImmuneResult,
    CorrectedIsotypeResult, CorrectedPathwayResult, CorrectedRepertoireResult,
};
use crate::diagnostics::PbmcDiagnosticsResult;
use anyhow::{Context, Result};
use csv::WriterBuilder;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FocusedCandidateSettings {
    pub top_genes: usize,
    pub top_transcripts: usize,
    pub top_pathways: usize,
    pub top_other: usize,
    pub min_gene_abs_log2fc: f64,
}

impl Default for FocusedCandidateSettings {
    fn default() -> Self {
        Self {
            top_genes: 15,
            top_transcripts: 10,
            top_pathways: 12,
            top_other: 20,
            min_gene_abs_log2fc: 0.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FocusedGeneCandidate {
    pub rank: usize,
    pub gene: String,
    pub gene_id: String,
    pub adjusted_log2fc: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub case_mean: f64,
    pub control_mean: f64,
    pub exploratory_log2fc: Option<f64>,
    pub exploratory_q: Option<f64>,
    pub direction_concordant: Option<bool>,
    pub pathway_support_count: usize,
    pub pathway_support: String,
    pub evidence: String,
    pub robustness: Option<String>,
    pub robustness_flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FocusedPathwayCandidate {
    pub rank: usize,
    pub pathway: String,
    pub description: String,
    pub odds_ratio: f64,
    pub overlap: usize,
    pub measured_set_size: usize,
    pub p_value: f64,
    pub q_value: f64,
    pub overlap_genes: String,
    pub evidence: String,
}

#[derive(Debug, Clone)]
pub struct FocusedOtherCandidate {
    pub rank: usize,
    pub modality: String,
    pub family: String,
    pub feature: String,
    pub secondary_id: String,
    pub effect: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub case_mean: f64,
    pub control_mean: f64,
    pub evidence: String,
}

#[derive(Debug, Clone)]
pub struct FocusedCandidateSummary {
    pub metadata_path: PathBuf,
    pub case_label: String,
    pub control_label: String,
    pub n_case: usize,
    pub n_control: usize,
    pub settings: FocusedCandidateSettings,
    pub genes: Vec<FocusedGeneCandidate>,
    pub transcripts: Vec<FocusedGeneCandidate>,
    pub pathways: Vec<FocusedPathwayCandidate>,
    pub other: Vec<FocusedOtherCandidate>,
    pub notes: Vec<String>,
}

fn evidence_label(q: f64, p: f64) -> String {
    if q.is_finite() && q <= 0.05 {
        "FDR-supported preliminary signal".to_string()
    } else if q.is_finite() && q <= 0.10 {
        "Promising preliminary signal".to_string()
    } else if p.is_finite() && p <= 0.01 {
        "Suggestive preliminary signal".to_string()
    } else {
        "Ranked exploratory signal".to_string()
    }
}

fn finite_or_inf(x: f64) -> f64 {
    if x.is_finite() { x } else { f64::INFINITY }
}

fn exploratory_index(
    result: Option<&GeneComparisonResult>,
) -> HashMap<(String, String), &FeatureTestResult> {
    let mut out = HashMap::new();
    if let Some(result) = result {
        for row in &result.tests {
            out.insert(
                (
                    row.feature.to_ascii_uppercase(),
                    row.secondary_id.to_ascii_uppercase(),
                ),
                row,
            );
        }
    }
    out
}

fn pathway_support_index(pathway: Option<&CorrectedPathwayResult>) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let Some(pathway) = pathway else {
        return out;
    };
    for row in pathway
        .tests
        .iter()
        .filter(|r| r.q_value.is_finite() && r.q_value <= 0.10 && r.odds_ratio > 1.0)
    {
        for gene in row
            .overlap_genes
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
        {
            let entry = out.entry(gene.to_ascii_uppercase()).or_default();
            if !entry.iter().any(|p| p == &row.pathway) {
                entry.push(row.pathway.clone());
            }
        }
    }
    out
}

fn shortlist_expression(
    corrected: Option<&CorrectedExpressionResult>,
    exploratory: Option<&GeneComparisonResult>,
    pathway: Option<&CorrectedPathwayResult>,
    top_n: usize,
    min_abs_effect: f64,
) -> Vec<FocusedGeneCandidate> {
    let Some(corrected) = corrected else {
        return Vec::new();
    };
    let exploratory = exploratory_index(exploratory);
    let pathway_support = pathway_support_index(pathway);

    let mut rows = corrected
        .tests
        .iter()
        .filter(|r| {
            r.beta_group.is_finite()
                && r.beta_group.abs() >= min_abs_effect
                && r.p_value.is_finite()
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        finite_or_inf(a.q_value)
            .total_cmp(&finite_or_inf(b.q_value))
            .then_with(|| finite_or_inf(a.p_value).total_cmp(&finite_or_inf(b.p_value)))
            .then_with(|| b.beta_group.abs().total_cmp(&a.beta_group.abs()))
    });

    rows.into_iter()
        .take(top_n)
        .enumerate()
        .map(|(idx, r)| {
            let key = (
                r.feature.to_ascii_uppercase(),
                r.secondary_id.to_ascii_uppercase(),
            );
            let exp = exploratory.get(&key).copied();
            let exploratory_log2fc = exp.map(|x| x.effect);
            let exploratory_q = exp.map(|x| x.q_value);
            let direction_concordant = exp.and_then(|x| {
                if x.effect.is_finite()
                    && r.beta_group.is_finite()
                    && x.effect != 0.0
                    && r.beta_group != 0.0
                {
                    Some(x.effect.signum() == r.beta_group.signum())
                } else {
                    None
                }
            });
            let support = pathway_support
                .get(&r.feature.to_ascii_uppercase())
                .cloned()
                .unwrap_or_default();
            let support_display = support
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ");
            FocusedGeneCandidate {
                rank: idx + 1,
                gene: r.feature.clone(),
                gene_id: r.secondary_id.clone(),
                adjusted_log2fc: r.beta_group,
                p_value: r.p_value,
                q_value: r.q_value,
                case_mean: r.mean_case,
                control_mean: r.mean_control,
                exploratory_log2fc,
                exploratory_q,
                direction_concordant,
                pathway_support_count: support.len(),
                pathway_support: support_display,
                evidence: evidence_label(r.q_value, r.p_value),
                robustness: None,
                robustness_flags: Vec::new(),
            }
        })
        .collect()
}

fn shortlist_pathways(
    pathway: Option<&CorrectedPathwayResult>,
    top_n: usize,
) -> Vec<FocusedPathwayCandidate> {
    let Some(pathway) = pathway else {
        return Vec::new();
    };
    let mut rows = pathway
        .tests
        .iter()
        .filter(|r| {
            r.overlap > 0
                && r.p_value.is_finite()
                && r.p_value < 1.0
                && r.odds_ratio.is_finite()
                && r.odds_ratio > 1.0
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        finite_or_inf(a.q_value)
            .total_cmp(&finite_or_inf(b.q_value))
            .then_with(|| finite_or_inf(a.p_value).total_cmp(&finite_or_inf(b.p_value)))
            .then_with(|| b.odds_ratio.total_cmp(&a.odds_ratio))
    });
    rows.into_iter()
        .take(top_n)
        .enumerate()
        .map(|(idx, r)| FocusedPathwayCandidate {
            rank: idx + 1,
            pathway: r.pathway.clone(),
            description: r.description.clone(),
            odds_ratio: r.odds_ratio,
            overlap: r.overlap,
            measured_set_size: r.measured_set_size,
            p_value: r.p_value,
            q_value: r.q_value,
            overlap_genes: r.overlap_genes.clone(),
            evidence: evidence_label(r.q_value, r.p_value),
        })
        .collect()
}

fn append_adjusted_candidates(
    out: &mut Vec<FocusedOtherCandidate>,
    modality: &str,
    family: &str,
    tests: &[AdjustedFeatureResult],
) {
    for r in tests {
        if !r.p_value.is_finite() || !r.beta_group.is_finite() {
            continue;
        }
        out.push(FocusedOtherCandidate {
            rank: 0,
            modality: modality.to_string(),
            family: family.to_string(),
            feature: r.feature.clone(),
            secondary_id: r.secondary_id.clone(),
            effect: r.beta_group,
            p_value: r.p_value,
            q_value: r.q_value,
            case_mean: r.mean_case,
            control_mean: r.mean_control,
            evidence: evidence_label(r.q_value, r.p_value),
        });
    }
}

fn collect_other_candidates(
    bcr: Option<&CorrectedRepertoireResult>,
    bcr_isotype: Option<&CorrectedIsotypeResult>,
    tcr_alpha: Option<&CorrectedRepertoireResult>,
    tcr_beta: Option<&CorrectedRepertoireResult>,
    immune: Option<&CorrectedImmuneResult>,
    top_n: usize,
) -> Vec<FocusedOtherCandidate> {
    let mut out = Vec::new();
    if let Some(r) = bcr {
        append_adjusted_candidates(&mut out, "BCR", "Repertoire metrics", &r.metric_tests);
        for c in &r.compositions {
            append_adjusted_candidates(&mut out, "BCR", &c.family, &c.tests);
        }
    }
    if let Some(r) = bcr_isotype {
        append_adjusted_candidates(&mut out, "BCR isotype", "Isotype composition", &r.tests);
    }
    if let Some(r) = tcr_alpha {
        append_adjusted_candidates(&mut out, "TCR alpha", "Repertoire metrics", &r.metric_tests);
        for c in &r.compositions {
            append_adjusted_candidates(&mut out, "TCR alpha", &c.family, &c.tests);
        }
    }
    if let Some(r) = tcr_beta {
        append_adjusted_candidates(&mut out, "TCR beta", "Repertoire metrics", &r.metric_tests);
        for c in &r.compositions {
            append_adjusted_candidates(&mut out, "TCR beta", &c.family, &c.tests);
        }
    }
    if let Some(r) = immune {
        append_adjusted_candidates(&mut out, "Immune infiltrate", "ssGSEA score", &r.tests);
    }
    out.sort_by(|a, b| {
        finite_or_inf(a.q_value)
            .total_cmp(&finite_or_inf(b.q_value))
            .then_with(|| finite_or_inf(a.p_value).total_cmp(&finite_or_inf(b.p_value)))
            .then_with(|| b.effect.abs().total_cmp(&a.effect.abs()))
    });
    out.truncate(top_n);
    for (idx, row) in out.iter_mut().enumerate() {
        row.rank = idx + 1;
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn build_focused_candidate_summary(
    metadata_path: &Path,
    case_label: &str,
    control_label: &str,
    n_case: usize,
    n_control: usize,
    settings: FocusedCandidateSettings,
    corrected_gene: Option<&CorrectedExpressionResult>,
    exploratory_gene: Option<&GeneComparisonResult>,
    corrected_transcript: Option<&CorrectedExpressionResult>,
    exploratory_transcript: Option<&GeneComparisonResult>,
    bcr: Option<&CorrectedRepertoireResult>,
    bcr_isotype: Option<&CorrectedIsotypeResult>,
    tcr_alpha: Option<&CorrectedRepertoireResult>,
    tcr_beta: Option<&CorrectedRepertoireResult>,
    immune: Option<&CorrectedImmuneResult>,
    pathway: Option<&CorrectedPathwayResult>,
) -> FocusedCandidateSummary {
    let genes = shortlist_expression(
        corrected_gene,
        exploratory_gene,
        pathway,
        settings.top_genes,
        settings.min_gene_abs_log2fc,
    );
    let transcripts = shortlist_expression(
        corrected_transcript,
        exploratory_transcript,
        None,
        settings.top_transcripts,
        settings.min_gene_abs_log2fc,
    );
    let pathways = shortlist_pathways(pathway, settings.top_pathways);
    let other = collect_other_candidates(
        bcr,
        bcr_isotype,
        tcr_alpha,
        tcr_beta,
        immune,
        settings.top_other,
    );

    let mut notes = vec![
        "Corrected/standard results are the primary evidence used for prioritization. Exploratory expression results, when requested, are shown only as a direction-concordance check and are not independent validation.".to_string(),
        "Candidate labels are deliberately preliminary. A ranked signal can be useful for follow-up even when it does not meet FDR <= 0.05, especially in a small cohort, but it should not be presented as a validated biomarker.".to_string(),
        "Candidate prioritization does not alter the statistical model or sample-dependence assumptions. Repeated-measure, family, batch, and other dependence should be interpreted according to the corrected analysis settings used for the same run.".to_string(),
    ];
    if genes.is_empty() {
        notes.push("No gene passed the configured minimum absolute adjusted log2 fold-change for the configured shortlist.".to_string());
    }
    if pathway.is_none() {
        notes.push("No pathway collection was analyzed; pathway support is therefore unavailable for gene prioritization.".to_string());
    }

    FocusedCandidateSummary {
        metadata_path: metadata_path.to_path_buf(),
        case_label: case_label.to_string(),
        control_label: control_label.to_string(),
        n_case,
        n_control,
        settings,
        genes,
        transcripts,
        pathways,
        other,
        notes,
    }
}

pub fn annotate_focused_candidate_robustness(
    summary: &mut FocusedCandidateSummary,
    diagnostics: &PbmcDiagnosticsResult,
) {
    let by_gene = diagnostics
        .candidate_stability
        .iter()
        .map(|row| (row.gene.to_ascii_uppercase(), row))
        .collect::<HashMap<_, _>>();
    for candidate in &mut summary.genes {
        if let Some(row) = by_gene.get(&candidate.gene.to_ascii_uppercase()) {
            candidate.robustness = Some(row.classification.clone());
            candidate.robustness_flags = row.flags.clone();
        }
    }
}

pub fn write_focused_candidate_tables(
    folder: &Path,
    summary: &FocusedCandidateSummary,
) -> Result<PathBuf> {
    fs::create_dir_all(folder).with_context(|| format!("Could not create {}", folder.display()))?;

    let gene_path = folder.join("focused_candidate_genes.csv");
    let mut w = WriterBuilder::new()
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(&gene_path)?;
    w.write_record([
        "rank",
        "gene",
        "gene_id",
        "adjusted_log2fc",
        "p_value",
        "q_value",
        "case_mean",
        "control_mean",
        "exploratory_log2fc",
        "exploratory_q",
        "direction_concordant",
        "pathway_support_count",
        "pathway_support",
        "evidence",
        "robustness",
        "robustness_flags",
    ])?;
    for r in &summary.genes {
        let robustness_flags = r.robustness_flags.join("; ");
        w.serialize((
            r.rank,
            &r.gene,
            &r.gene_id,
            r.adjusted_log2fc,
            r.p_value,
            r.q_value,
            r.case_mean,
            r.control_mean,
            r.exploratory_log2fc,
            r.exploratory_q,
            r.direction_concordant,
            r.pathway_support_count,
            &r.pathway_support,
            &r.evidence,
            r.robustness.as_deref(),
            &robustness_flags,
        ))?;
    }
    w.flush()?;

    let pathway_path = folder.join("focused_candidate_pathways.csv");
    let mut w = WriterBuilder::new()
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(&pathway_path)?;
    w.write_record([
        "rank",
        "pathway",
        "description",
        "odds_ratio",
        "overlap",
        "measured_set_size",
        "p_value",
        "q_value",
        "overlap_genes",
        "evidence",
    ])?;
    for r in &summary.pathways {
        w.serialize((
            r.rank,
            &r.pathway,
            &r.description,
            r.odds_ratio,
            r.overlap,
            r.measured_set_size,
            r.p_value,
            r.q_value,
            &r.overlap_genes,
            &r.evidence,
        ))?;
    }
    w.flush()?;

    let other_path = folder.join("focused_candidate_other_signals.csv");
    let mut w = WriterBuilder::new()
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(&other_path)?;
    w.write_record([
        "rank",
        "modality",
        "family",
        "feature",
        "secondary_id",
        "effect",
        "p_value",
        "q_value",
        "case_mean",
        "control_mean",
        "evidence",
    ])?;
    for r in &summary.other {
        w.serialize((
            r.rank,
            &r.modality,
            &r.family,
            &r.feature,
            &r.secondary_id,
            r.effect,
            r.p_value,
            r.q_value,
            r.case_mean,
            r.control_mean,
            &r.evidence,
        ))?;
    }
    w.flush()?;

    let transcript_path = folder.join("focused_candidate_transcripts.csv");
    let mut w = WriterBuilder::new()
        .has_headers(false)
        .buffer_capacity(1024 * 1024)
        .from_path(&transcript_path)?;
    w.write_record([
        "rank",
        "transcript",
        "gene_id",
        "adjusted_effect",
        "p_value",
        "q_value",
        "case_mean",
        "control_mean",
        "exploratory_effect",
        "exploratory_q",
        "direction_concordant",
        "evidence",
    ])?;
    for r in &summary.transcripts {
        w.serialize((
            r.rank,
            &r.gene,
            &r.gene_id,
            r.adjusted_log2fc,
            r.p_value,
            r.q_value,
            r.case_mean,
            r.control_mean,
            r.exploratory_log2fc,
            r.exploratory_q,
            r.direction_concordant,
            &r.evidence,
        ))?;
    }
    w.flush()?;

    let note_path = folder.join("focused_candidate_notes.txt");
    let mut text = format!(
        "Candidate-prioritization summary\n==============================\n\nMetadata: {}\nComparison: {} vs {}\nCase samples: {}\nControl samples: {}\nMinimum gene |adjusted log2FC|: {:.3}\n\n",
        summary.metadata_path.display(),
        summary.case_label,
        summary.control_label,
        summary.n_case,
        summary.n_control,
        summary.settings.min_gene_abs_log2fc,
    );
    for note in &summary.notes {
        text.push_str("- ");
        text.push_str(note);
        text.push('\n');
    }
    fs::write(&note_path, text)?;

    Ok(folder.to_path_buf())
}

/// Write the candidate-prioritization layer using unified user-facing key-findings names.
/// The underlying rows are identical to the legacy focused export; only filenames/notes
/// are presentation-oriented for the unified Cohort Analysis workflow.
pub fn write_key_findings_tables(
    folder: &Path,
    summary: &FocusedCandidateSummary,
) -> Result<PathBuf> {
    let output = write_focused_candidate_tables(folder, summary)?;
    for (legacy, unified) in [
        ("focused_candidate_genes.csv", "key_candidate_genes.csv"),
        (
            "focused_candidate_pathways.csv",
            "key_candidate_pathways.csv",
        ),
        (
            "focused_candidate_transcripts.csv",
            "key_candidate_transcripts.csv",
        ),
        (
            "focused_candidate_other_signals.csv",
            "key_other_signals.csv",
        ),
        ("focused_candidate_notes.txt", "key_findings_notes.txt"),
    ] {
        let from = output.join(legacy);
        if from.exists() {
            let to = output.join(unified);
            if to.exists() {
                fs::remove_file(&to).with_context(|| {
                    format!(
                        "Could not replace existing unified key-findings file {}",
                        to.display()
                    )
                })?;
            }
            fs::rename(&from, &to).with_context(|| {
                format!(
                    "Could not rename {} for unified key-findings export",
                    from.display()
                )
            })?;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corrected::CorrectedExpressionResult;
    use std::path::PathBuf;

    fn adjusted(feature: &str, beta: f64, p: f64, q: f64) -> AdjustedFeatureResult {
        AdjustedFeatureResult {
            feature: feature.to_string(),
            secondary_id: "1".to_string(),
            n_samples: 20,
            n_subjects: 20,
            n_case: 10,
            n_control: 10,
            mean_case: 2.0,
            mean_control: 1.0,
            beta_group: beta,
            robust_se: 0.2,
            z_value: beta / 0.2,
            p_value: p,
            q_value: q,
            subject_effect: beta,
            subject_p_value: p,
            subject_q_value: q,
            method: "test".to_string(),
        }
    }

    #[test]
    fn focused_gene_ranking_prefers_q_then_effect() {
        let r = CorrectedExpressionResult {
            input: PathBuf::new(),
            feature_label: "Gene".to_string(),
            source_metric: "Raw Count".to_string(),
            normalization: String::new(),
            model_description: String::new(),
            model_sample_ids: Vec::new(),
            size_factors: Vec::new(),
            n_input_samples: 20,
            n_model_samples: 20,
            n_subjects: 20,
            tested_features: 3,
            filtered_features: 0,
            tests: vec![
                adjusted("B", 1.2, 0.001, 0.02),
                adjusted("A", 0.8, 0.0001, 0.01),
                adjusted("C", 2.0, 0.02, 0.2),
            ],
            pca: None,
            dendrogram: None,
            dtu: None,
            warnings: Vec::new(),
        };
        let out = shortlist_expression(Some(&r), None, None, 3, 0.5);
        assert_eq!(out[0].gene, "A");
        assert_eq!(out[1].gene, "B");
        assert_eq!(out[2].gene, "C");
    }
    #[test]
    fn focused_pathways_require_real_foreground_overlap() {
        let pathway = CorrectedPathwayResult {
            gmt_path: PathBuf::new(),
            universe_size: 1000,
            foreground_size: 25,
            q_threshold: 0.05,
            effect_threshold: 0.5,
            min_set_size: 10,
            max_set_size: 500,
            tests: vec![
                crate::corrected::PathwayOraResult {
                    pathway: "ZERO".to_string(),
                    description: String::new(),
                    measured_set_size: 20,
                    overlap: 0,
                    odds_ratio: 99.0,
                    p_value: 1.0,
                    q_value: 1.0,
                    overlap_genes: String::new(),
                },
                crate::corrected::PathwayOraResult {
                    pathway: "REAL".to_string(),
                    description: String::new(),
                    measured_set_size: 20,
                    overlap: 3,
                    odds_ratio: 2.0,
                    p_value: 0.01,
                    q_value: 0.04,
                    overlap_genes: "A,B,C".to_string(),
                },
            ],
        };
        let out = shortlist_pathways(Some(&pathway), 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pathway, "REAL");
    }
}
