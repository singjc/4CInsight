//! Unified cohort-analysis run model.
//!
//! This module does not implement a second statistical engine.  It owns the
//! configuration, provenance, corrected results, supporting diagnostics, and
//! candidate-prioritization output produced by one completed Cohort Analysis
//! run so every presentation/export can describe the exact same analysis.

use crate::cohort::{
    CohortComparison, CohortComparisonDefinition, CohortMapping, GeneComparisonResult,
    MetadataTable, normalize_sample_id,
};
use crate::corrected::{
    CorrectedExpressionResult, CorrectedImmuneResult, CorrectedIsotypeResult, CorrectedModelSpec,
    CorrectedPathwayResult, CorrectedRepertoireResult,
};
use crate::diagnostics::{PbmcDiagnosticsConfig, PbmcDiagnosticsResult};
use crate::focused::{FocusedCandidateSettings, FocusedCandidateSummary};
use crate::hit_structure::{SignificantHitStructureConfig, SignificantHitStructureResult};
use crate::longitudinal::{LongitudinalAnalysisResult, LongitudinalClusteringConfig};
use crate::outliers::OutlierFilterSnapshot;
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

/// Frozen provenance for a user-defined sample exclusion rule taken directly
/// from the loaded sample metadata.  The UI exposes this independently of the
/// cross-modal outlier-QC workflow so collaborators can mark samples using a
/// column/value convention of their own (for example `manual_outlier=Yes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataSampleExclusionSnapshot {
    /// Metadata column whose values were inspected.
    pub column: String,
    /// Exact observed values selected for exclusion (matching is
    /// case-insensitive after trimming whitespace).
    pub excluded_values: Vec<String>,
    /// Metadata IDs of all rows matching the configured rule, including rows
    /// that are outside the active Case/Control comparison.
    pub matching_metadata_ids: Vec<String>,
    /// Canonical IDs actually removed from the active Case/Control comparison.
    pub excluded_comparison_ids: Vec<String>,
    pub original_case_n: usize,
    pub original_control_n: usize,
    pub filtered_case_n: usize,
    pub filtered_control_n: usize,
}

impl MetadataSampleExclusionSnapshot {
    pub fn applied_count(&self) -> usize {
        self.excluded_comparison_ids.len()
    }
}

/// Return metadata IDs matching a selected column/value exclusion rule.
/// Empty values are ignored; selected values are matched case-insensitively
/// after trimming so the frozen rule remains robust to inconsequential
/// capitalization or surrounding whitespace.
pub fn metadata_exclusion_ids(
    metadata: &MetadataTable,
    column: &str,
    excluded_values: &HashSet<String>,
) -> Vec<String> {
    let Some(column_idx) = metadata.column_index(column) else {
        return Vec::new();
    };
    let normalized_values = excluded_values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>();
    if normalized_values.is_empty() {
        return Vec::new();
    }

    let mut ids = BTreeSet::new();
    for (row_idx, row) in metadata.rows.iter().enumerate() {
        let Some(value) = row.get(column_idx) else {
            continue;
        };
        if !normalized_values.contains(&value.trim().to_ascii_lowercase()) {
            continue;
        }
        if let Some(id) = metadata.id_value(row_idx) {
            let id = id.trim();
            if !id.is_empty() {
                ids.insert(id.to_string());
            }
        }
    }
    ids.into_iter().collect()
}

/// Filter an existing comparison by a set of sample IDs while retaining the
/// same Case/Control labels.  This is deliberately generic rather than tied to
/// the automatic outlier-QC terminology because the exclusion source may be a
/// collaborator-authored metadata flag.
pub fn filter_comparison_excluding_samples(
    comparison: &CohortComparison,
    excluded_ids: &HashSet<String>,
) -> CohortComparison {
    if excluded_ids.is_empty() {
        return comparison.clone();
    }
    let canonical_excluded = excluded_ids
        .iter()
        .map(|id| comparison.canonical_id(id))
        .collect::<HashSet<_>>();
    let mut case_ids = comparison.case_ids.clone();
    let mut control_ids = comparison.control_ids.clone();
    case_ids.retain(|id| !canonical_excluded.contains(id));
    control_ids.retain(|id| !canonical_excluded.contains(id));
    if case_ids.len() == comparison.case_ids.len()
        && control_ids.len() == comparison.control_ids.len()
    {
        return comparison.clone();
    }
    let mut id_to_canonical = comparison.id_to_canonical.clone();
    id_to_canonical.retain(|_, canonical| !canonical_excluded.contains(canonical));
    CohortComparison {
        name: format!("{} — sample-filtered", comparison.name),
        case_label: comparison.case_label.clone(),
        control_label: comparison.control_label.clone(),
        case_metadata_rows: case_ids.len(),
        control_metadata_rows: control_ids.len(),
        case_ids,
        control_ids,
        id_to_canonical,
    }
}

pub fn build_metadata_sample_exclusion_snapshot(
    metadata: &MetadataTable,
    column: &str,
    excluded_values: &HashSet<String>,
    original: &CohortComparison,
) -> MetadataSampleExclusionSnapshot {
    let matching_metadata_ids = metadata_exclusion_ids(metadata, column, excluded_values);
    let matching_set = matching_metadata_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let filtered = filter_comparison_excluding_samples(original, &matching_set);
    let filtered_ids = filtered
        .case_ids
        .iter()
        .chain(filtered.control_ids.iter())
        .cloned()
        .collect::<HashSet<_>>();
    let mut excluded_comparison_ids = matching_metadata_ids
        .iter()
        .map(|id| original.canonical_id(id))
        .filter(|id| {
            (original.case_ids.contains(id) || original.control_ids.contains(id))
                && !filtered_ids.contains(id)
        })
        .collect::<Vec<_>>();
    excluded_comparison_ids.sort();
    excluded_comparison_ids.dedup();
    let mut excluded_values = excluded_values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    excluded_values.sort_by_key(|value| value.to_ascii_lowercase());
    excluded_values.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    MetadataSampleExclusionSnapshot {
        column: column.to_string(),
        excluded_values,
        matching_metadata_ids,
        excluded_comparison_ids,
        original_case_n: original.case_ids.len(),
        original_control_n: original.control_ids.len(),
        filtered_case_n: filtered.case_ids.len(),
        filtered_control_n: filtered.control_ids.len(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct CohortModalitySelection {
    pub gene: bool,
    pub transcript: bool,
    pub bcr: bool,
    pub bcr_isotype: bool,
    pub tcr_alpha: bool,
    pub tcr_beta: bool,
    pub immune: bool,
    pub pathway: bool,
}

impl CohortModalitySelection {
    pub fn any_primary(&self) -> bool {
        self.gene
            || self.transcript
            || self.bcr
            || self.bcr_isotype
            || self.tcr_alpha
            || self.tcr_beta
            || self.immune
            || self.pathway
    }
}

#[derive(Debug, Clone)]
pub struct CohortDefinitionSnapshot {
    pub project_root: PathBuf,
    pub metadata_path: PathBuf,
    pub metadata_id_column: String,
    pub uses_filtered_metadata_subset: bool,
    pub comparison_definition: CohortComparisonDefinition,
    pub comparison: CohortComparison,
    pub mapping: CohortMapping,
}

#[derive(Debug, Clone)]
pub struct CohortAnalysisSettingsSnapshot {
    pub modalities: CohortModalitySelection,
    pub corrected: CorrectedModelSpec,
    pub pathway_gmt_path: Option<PathBuf>,
    pub pathway_q_threshold: f64,
    pub pathway_effect_threshold: f64,
    pub exploratory_direction_check: bool,
    pub candidate: FocusedCandidateSettings,
    pub pbmc_diagnostics: Option<PbmcDiagnosticsConfig>,
    /// Optional user-defined metadata rule used to exclude samples from the
    /// primary Case/Control analysis independently of automatic outlier QC.
    pub metadata_sample_exclusion: Option<MetadataSampleExclusionSnapshot>,
    /// Optional unsupervised all-eligible-sample analysis of longitudinal
    /// patient/person identity, independent of the Case/Control comparison.
    pub longitudinal: Option<LongitudinalClusteringConfig>,
    /// Optional descriptive PCA/dendrogram analysis restricted to features
    /// that are significant in the same corrected Case/Control comparison.
    pub significant_hit_structure: Option<SignificantHitStructureConfig>,
}

#[derive(Debug, Clone)]
pub struct CorrectedAnalysisBundle {
    pub comparison: CohortComparison,
    pub spec: CorrectedModelSpec,
    pub gene: Option<CorrectedExpressionResult>,
    pub gene_error: Option<String>,
    pub transcript: Option<CorrectedExpressionResult>,
    pub transcript_error: Option<String>,
    pub bcr: Option<CorrectedRepertoireResult>,
    pub bcr_error: Option<String>,
    pub bcr_isotype: Option<CorrectedIsotypeResult>,
    pub bcr_isotype_error: Option<String>,
    pub tcr_alpha: Option<CorrectedRepertoireResult>,
    pub tcr_alpha_error: Option<String>,
    pub tcr_beta: Option<CorrectedRepertoireResult>,
    pub tcr_beta_error: Option<String>,
    pub immune: Option<CorrectedImmuneResult>,
    pub immune_error: Option<String>,
    pub pathway: Option<CorrectedPathwayResult>,
    pub pathway_error: Option<String>,
}

impl CorrectedAnalysisBundle {
    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (label, error) in [
            ("Gene expression", self.gene_error.as_deref()),
            ("Transcript expression", self.transcript_error.as_deref()),
            ("BCR", self.bcr_error.as_deref()),
            ("BCR isotype", self.bcr_isotype_error.as_deref()),
            ("TCR alpha", self.tcr_alpha_error.as_deref()),
            ("TCR beta", self.tcr_beta_error.as_deref()),
            ("Immune infiltrate", self.immune_error.as_deref()),
            ("Pathway ORA", self.pathway_error.as_deref()),
        ] {
            if let Some(error) = error {
                out.push(format!("{label}: {error}"));
            }
        }
        if let Some(r) = &self.gene {
            out.extend(r.warnings.iter().map(|w| format!("Gene expression: {w}")));
        }
        if let Some(r) = &self.transcript {
            out.extend(
                r.warnings
                    .iter()
                    .map(|w| format!("Transcript expression: {w}")),
            );
        }
        if let Some(r) = &self.bcr {
            out.extend(r.warnings.iter().map(|w| format!("BCR: {w}")));
        }
        if let Some(r) = &self.bcr_isotype {
            out.extend(r.warnings.iter().map(|w| format!("BCR isotype: {w}")));
        }
        if let Some(r) = &self.tcr_alpha {
            out.extend(r.warnings.iter().map(|w| format!("TCR alpha: {w}")));
        }
        if let Some(r) = &self.tcr_beta {
            out.extend(r.warnings.iter().map(|w| format!("TCR beta: {w}")));
        }
        if let Some(r) = &self.immune {
            out.extend(r.warnings.iter().map(|w| format!("Immune infiltrate: {w}")));
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExploratorySupportBundle {
    pub gene: Option<GeneComparisonResult>,
    pub gene_error: Option<String>,
    pub transcript: Option<GeneComparisonResult>,
    pub transcript_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UnifiedCohortAnalysisRun {
    pub cohort: CohortDefinitionSnapshot,
    pub settings: CohortAnalysisSettingsSnapshot,
    pub qc_snapshot: Option<OutlierFilterSnapshot>,
    pub primary: CorrectedAnalysisBundle,
    pub exploratory_support: ExploratorySupportBundle,
    pub diagnostics: Option<PbmcDiagnosticsResult>,
    pub diagnostics_error: Option<String>,
    pub longitudinal: Option<LongitudinalAnalysisResult>,
    pub longitudinal_error: Option<String>,
    pub significant_hit_structure: Option<SignificantHitStructureResult>,
    pub significant_hit_structure_error: Option<String>,
    pub candidates: FocusedCandidateSummary,
    pub warnings: Vec<String>,
}

/// Borrowed view of the completed primary corrected/modal analysis before
/// optional PBMC/confounding sensitivity diagnostics run. This checkpoint is
/// presentation-only: it lets the web server expose already-final primary
/// cohort results without duplicating or cloning the large native result.
pub struct UnifiedPrimaryCheckpoint<'a> {
    pub cohort: &'a CohortDefinitionSnapshot,
    pub settings: &'a CohortAnalysisSettingsSnapshot,
    pub qc_snapshot: Option<&'a OutlierFilterSnapshot>,
    pub primary: &'a CorrectedAnalysisBundle,
    pub exploratory_support: &'a ExploratorySupportBundle,
    pub candidates: &'a FocusedCandidateSummary,
    pub warnings: &'a [String],
}

impl UnifiedCohortAnalysisRun {
    pub fn has_qc_filter(&self) -> bool {
        self.qc_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.applied_to_analysis && !snapshot.excluded_ids.is_empty()
        })
    }

    pub fn case_n(&self) -> usize {
        self.primary.comparison.case_ids.len()
    }

    pub fn control_n(&self) -> usize {
        self.primary.comparison.control_ids.len()
    }
}

use crate::cohort::{
    CohortModality, MetadataTable, ProjectScan, run_gene_comparison, run_transcript_comparison,
};
use crate::corrected::{
    run_corrected_expression, run_corrected_immune, run_corrected_isotype,
    run_corrected_repertoire, run_pathway_ora,
};
use crate::diagnostics::run_pbmc_diagnostics_with_baseline;
use crate::focused::{annotate_focused_candidate_robustness, build_focused_candidate_summary};
use crate::hit_structure::{run_significant_hit_structure, write_significant_hit_structure_tables};
use crate::longitudinal::{run_longitudinal_analysis, write_longitudinal_analysis_tables};
use anyhow::Result;

/// Execute one complete cohort analysis using the existing statistical engines.
///
/// Modality failures are captured in the returned run so one unavailable data
/// type does not discard successful results from the other selected modalities.
pub fn run_unified_cohort_analysis<F>(
    scan: &ProjectScan,
    metadata: &MetadataTable,
    cohort: CohortDefinitionSnapshot,
    settings: CohortAnalysisSettingsSnapshot,
    qc_snapshot: Option<OutlierFilterSnapshot>,
    progress: F,
) -> Result<UnifiedCohortAnalysisRun>
where
    F: FnMut(usize, usize, &str),
{
    run_unified_cohort_analysis_with_primary_checkpoint(
        scan,
        metadata,
        cohort,
        settings,
        qc_snapshot,
        progress,
        |_| {},
    )
}

/// Execute the same unified native analysis while publishing a borrowed
/// checkpoint as soon as the primary corrected modalities/pathway result is
/// final, before optional exploratory TPM direction checks and PBMC/confounding
/// sensitivity refits.
/// The ordinary native/desktop entry point above remains behaviorally
/// identical and simply ignores this checkpoint.
pub fn run_unified_cohort_analysis_with_primary_checkpoint<F, G>(
    scan: &ProjectScan,
    metadata: &MetadataTable,
    cohort: CohortDefinitionSnapshot,
    settings: CohortAnalysisSettingsSnapshot,
    qc_snapshot: Option<OutlierFilterSnapshot>,
    mut progress: F,
    mut primary_ready: G,
) -> Result<UnifiedCohortAnalysisRun>
where
    F: FnMut(usize, usize, &str),
    G: for<'a> FnMut(UnifiedPrimaryCheckpoint<'a>),
{
    let selection = settings.modalities.clone();
    let comparison = cohort.comparison.clone();
    let spec = settings.corrected.clone();
    let diagnostics_requested = settings.pbmc_diagnostics.is_some();
    let longitudinal_requested = settings.longitudinal.is_some();
    let significant_hit_structure_requested = settings.significant_hit_structure.is_some();
    let stage_total = [
        selection.gene || selection.pathway,
        selection.transcript,
        selection.bcr,
        selection.bcr_isotype,
        selection.tcr_alpha,
        selection.tcr_beta,
        selection.immune,
        selection.pathway,
        settings.exploratory_direction_check && (selection.gene || selection.pathway),
        settings.exploratory_direction_check && selection.transcript,
        longitudinal_requested,
        significant_hit_structure_requested,
        diagnostics_requested,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count()
    .max(1);
    let mut stage = 0usize;

    let mut primary = CorrectedAnalysisBundle {
        comparison: comparison.clone(),
        spec: spec.clone(),
        gene: None,
        gene_error: None,
        transcript: None,
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
    };

    if selection.gene || selection.pathway {
        let base = stage;
        if let Some(path) = scan.merged_gene_tsv.as_deref() {
            match run_corrected_expression(
                path,
                metadata,
                &comparison,
                "Raw Count",
                "Gene",
                &spec,
                |done, total, msg| {
                    let denom = total.max(1);
                    let overall = base * 1000 + (done * 1000 / denom);
                    progress(
                        overall,
                        stage_total * 1000,
                        &format!("Gene corrected — {msg}"),
                    );
                },
            ) {
                Ok(result) => primary.gene = Some(result),
                Err(error) => primary.gene_error = Some(format!("{error:#}")),
            }
        } else {
            primary.gene_error = Some("No merged gene-expression TSV detected.".to_string());
        }
        stage += 1;
    }

    if selection.transcript {
        let base = stage;
        if let Some(path) = scan.merged_transcript_tsv.as_deref() {
            match run_corrected_expression(
                path,
                metadata,
                &comparison,
                "Total Reads",
                "Transcript",
                &spec,
                |done, total, msg| {
                    let denom = total.max(1);
                    let overall = base * 1000 + (done * 1000 / denom);
                    progress(
                        overall,
                        stage_total * 1000,
                        &format!("Transcript corrected — {msg}"),
                    );
                },
            ) {
                Ok(result) => primary.transcript = Some(result),
                Err(error) => primary.transcript_error = Some(format!("{error:#}")),
            }
        } else {
            primary.transcript_error =
                Some("No merged transcript-expression TSV detected.".to_string());
        }
        stage += 1;
    }

    macro_rules! repertoire_stage {
        ($enabled:expr, $modality:expr, $field:ident, $error_field:ident, $label:expr) => {
            if $enabled {
                let base = stage;
                match run_corrected_repertoire(
                    &scan.files,
                    metadata,
                    &comparison,
                    $modality,
                    &spec,
                    |done, total, msg| {
                        let denom = total.max(1);
                        let overall = base * 1000 + (done * 1000 / denom);
                        progress(
                            overall,
                            stage_total * 1000,
                            &format!("{} — {}", $label, msg),
                        );
                    },
                ) {
                    Ok(result) => primary.$field = Some(result),
                    Err(error) => primary.$error_field = Some(format!("{error:#}")),
                }
                stage += 1;
            }
        };
    }

    repertoire_stage!(
        selection.bcr,
        CohortModality::Bcr,
        bcr,
        bcr_error,
        "BCR corrected"
    );

    if selection.bcr_isotype {
        let base = stage;
        match run_corrected_isotype(
            &scan.files,
            metadata,
            &comparison,
            &spec,
            |done, total, msg| {
                let denom = total.max(1);
                let overall = base * 1000 + (done * 1000 / denom);
                progress(
                    overall,
                    stage_total * 1000,
                    &format!("BCR isotype corrected — {msg}"),
                );
            },
        ) {
            Ok(result) => primary.bcr_isotype = Some(result),
            Err(error) => primary.bcr_isotype_error = Some(format!("{error:#}")),
        }
        stage += 1;
    }

    repertoire_stage!(
        selection.tcr_alpha,
        CohortModality::TcrAlpha,
        tcr_alpha,
        tcr_alpha_error,
        "TCR alpha corrected"
    );
    repertoire_stage!(
        selection.tcr_beta,
        CohortModality::TcrBeta,
        tcr_beta,
        tcr_beta_error,
        "TCR beta corrected"
    );

    if selection.immune {
        let base = stage;
        match run_corrected_immune(
            &scan.files,
            metadata,
            &comparison,
            &spec,
            |done, total, msg| {
                let denom = total.max(1);
                let overall = base * 1000 + (done * 1000 / denom);
                progress(
                    overall,
                    stage_total * 1000,
                    &format!("Immune corrected — {msg}"),
                );
            },
        ) {
            Ok(result) => primary.immune = Some(result),
            Err(error) => primary.immune_error = Some(format!("{error:#}")),
        }
        stage += 1;
    }

    if selection.pathway {
        progress(stage * 1000, stage_total * 1000, "Pathway ORA");
        match (primary.gene.as_ref(), settings.pathway_gmt_path.as_deref()) {
            (Some(gene), Some(gmt)) => match run_pathway_ora(
                gmt,
                gene,
                settings.pathway_q_threshold,
                settings.pathway_effect_threshold,
            ) {
                Ok(result) => primary.pathway = Some(result),
                Err(error) => primary.pathway_error = Some(format!("{error:#}")),
            },
            (None, _) => {
                primary.pathway_error =
                    Some("Pathway ORA requires a successful gene-count analysis.".to_string())
            }
            (_, None) => {
                primary.pathway_error =
                    Some("Choose a GMT gene-set file before running pathway ORA.".to_string())
            }
        }
        stage += 1;
    }

    // Publish the primary corrected result before optional supporting TPM
    // direction checks. These exploratory checks are explicitly non-primary
    // evidence and can require another full scan of very large merged matrices;
    // they must not hold the main corrected result hostage.
    let mut exploratory_support = ExploratorySupportBundle::default();
    let mut candidates = build_focused_candidate_summary(
        &cohort.metadata_path,
        &comparison.case_label,
        &comparison.control_label,
        comparison.case_ids.len(),
        comparison.control_ids.len(),
        settings.candidate.clone(),
        primary.gene.as_ref(),
        None,
        primary.transcript.as_ref(),
        None,
        primary.bcr.as_ref(),
        primary.bcr_isotype.as_ref(),
        primary.tcr_alpha.as_ref(),
        primary.tcr_beta.as_ref(),
        primary.immune.as_ref(),
        primary.pathway.as_ref(),
    );

    let mut checkpoint_warnings = primary.warnings();
    if let Some(qc) = qc_snapshot.as_ref() {
        checkpoint_warnings.extend(
            qc.audit
                .warnings
                .iter()
                .map(|warning| format!("Sample outlier QC: {warning}")),
        );
        for modality in &qc.audit.modalities {
            checkpoint_warnings.extend(
                modality
                    .warnings
                    .iter()
                    .map(|warning| format!("Sample outlier QC / {}: {warning}", modality.modality)),
            );
        }
    }

    // Primary corrected modalities and pathway analysis are final here.
    // Exploratory TPM direction-concordance and PBMC/confounding sensitivity
    // are supporting/robustness enrichment layers, so publish the borrowed
    // checkpoint now. The later final result rebuilds the candidate summary
    // after those optional enrichments complete.
    primary_ready(UnifiedPrimaryCheckpoint {
        cohort: &cohort,
        settings: &settings,
        qc_snapshot: qc_snapshot.as_ref(),
        primary: &primary,
        exploratory_support: &exploratory_support,
        candidates: &candidates,
        warnings: &checkpoint_warnings,
    });

    // Longitudinal patient-signature clustering is intentionally independent
    // of the Case/Control definition and uses all metadata rows that remain
    // eligible under its own configured QC-status filter. Run it after the
    // primary checkpoint so expensive all-cohort repertoire parsing cannot
    // delay access to already-final corrected results.
    let (mut longitudinal, mut longitudinal_error) = (None, None);
    if let Some(config) = settings.longitudinal.as_ref() {
        let base = stage;
        match run_longitudinal_analysis(
            scan,
            metadata,
            &selection,
            &spec,
            config.clone(),
            |done, total, message| {
                let denom = total.max(1);
                let overall = base * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    overall,
                    stage_total * 1000,
                    &format!("Longitudinal patient signature — {message}"),
                );
            },
        ) {
            Ok(result) => longitudinal = Some(result),
            Err(error) => longitudinal_error = Some(format!("{error:#}")),
        }
        stage += 1;
    }

    let (mut significant_hit_structure, mut significant_hit_structure_error) = (None, None);
    if let Some(config) = settings.significant_hit_structure.as_ref() {
        let base = stage;
        match run_significant_hit_structure(
            scan,
            metadata,
            &comparison,
            &spec,
            &primary,
            config.clone(),
            |done, total, message| {
                let denom = total.max(1);
                let overall = base * 1000 + done.min(denom) * 1000 / denom;
                progress(
                    overall,
                    stage_total * 1000,
                    &format!("Significant-hit structure — {message}"),
                );
            },
        ) {
            Ok(result) => significant_hit_structure = Some(result),
            Err(error) => significant_hit_structure_error = Some(format!("{error:#}")),
        }
        stage += 1;
    }

    if settings.exploratory_direction_check && (selection.gene || selection.pathway) {
        progress(
            stage * 1000,
            stage_total * 1000,
            "Gene TPM direction-concordance check",
        );
        if let Some(path) = scan.merged_gene_tsv.as_deref() {
            let base = stage;
            match run_gene_comparison(path, &comparison, "TPM", |done, total, msg| {
                let denom = total.max(1);
                let overall = base * 1000 + (done * 1000 / denom);
                progress(
                    overall,
                    stage_total * 1000,
                    &format!("Gene TPM direction-concordance check — {msg}"),
                );
            }) {
                Ok(result) => exploratory_support.gene = Some(result),
                Err(error) => exploratory_support.gene_error = Some(format!("{error:#}")),
            }
        } else {
            exploratory_support.gene_error =
                Some("No merged gene-expression TSV detected.".to_string());
        }
        stage += 1;
    }
    if settings.exploratory_direction_check && selection.transcript {
        progress(
            stage * 1000,
            stage_total * 1000,
            "Transcript TPM direction-concordance check",
        );
        if let Some(path) = scan.merged_transcript_tsv.as_deref() {
            let base = stage;
            match run_transcript_comparison(path, &comparison, "TPM", |done, total, msg| {
                let denom = total.max(1);
                let overall = base * 1000 + (done * 1000 / denom);
                progress(
                    overall,
                    stage_total * 1000,
                    &format!("Transcript TPM direction-concordance check — {msg}"),
                );
            }) {
                Ok(result) => exploratory_support.transcript = Some(result),
                Err(error) => exploratory_support.transcript_error = Some(format!("{error:#}")),
            }
        } else {
            exploratory_support.transcript_error =
                Some("No merged transcript-expression TSV detected.".to_string());
        }
        stage += 1;
    }

    // Enrich the same candidate ranking with exploratory direction support
    // after the optional TPM checks finish. Corrected statistics remain the
    // primary evidence and their ranks/effects are unchanged.
    candidates = build_focused_candidate_summary(
        &cohort.metadata_path,
        &comparison.case_label,
        &comparison.control_label,
        comparison.case_ids.len(),
        comparison.control_ids.len(),
        settings.candidate.clone(),
        primary.gene.as_ref(),
        exploratory_support.gene.as_ref(),
        primary.transcript.as_ref(),
        exploratory_support.transcript.as_ref(),
        primary.bcr.as_ref(),
        primary.bcr_isotype.as_ref(),
        primary.tcr_alpha.as_ref(),
        primary.tcr_beta.as_ref(),
        primary.immune.as_ref(),
        primary.pathway.as_ref(),
    );
    if let Some(error) = exploratory_support.gene_error.as_ref() {
        checkpoint_warnings.push(format!("Exploratory gene support: {error}"));
    }
    if let Some(error) = exploratory_support.transcript_error.as_ref() {
        checkpoint_warnings.push(format!("Exploratory transcript support: {error}"));
    }

    let (mut diagnostics, mut diagnostics_error) = (None, None);
    if let Some(config) = settings.pbmc_diagnostics.as_ref() {
        progress(
            stage * 1000,
            stage_total * 1000,
            "PBMC / confounding sensitivity diagnostics",
        );
        if let Some(path) = scan.merged_gene_tsv.as_deref() {
            match run_pbmc_diagnostics_with_baseline(
                path,
                &scan.files,
                metadata,
                &comparison,
                &spec,
                config,
                primary.gene.as_ref(),
                |done, total, msg| {
                    let denom = total.max(1);
                    let overall = stage * 1000 + (done * 1000 / denom);
                    progress(overall, stage_total * 1000, msg);
                },
            ) {
                Ok(result) => diagnostics = Some(result),
                Err(error) => diagnostics_error = Some(format!("{error:#}")),
            }
        } else {
            diagnostics_error = Some(
                "PBMC/confounding diagnostics require a merged gene-expression TSV.".to_string(),
            );
        }
    }

    if let Some(result) = diagnostics.as_ref() {
        annotate_focused_candidate_robustness(&mut candidates, result);
    }

    let mut warnings = checkpoint_warnings;
    if let Some(error) = diagnostics_error.as_ref() {
        warnings.push(format!("PBMC/confounding diagnostics: {error}"));
    }
    if let Some(result) = diagnostics.as_ref() {
        warnings.extend(
            result
                .warnings
                .iter()
                .map(|warning| format!("PBMC/confounding diagnostics: {warning}")),
        );
    }
    if let Some(error) = longitudinal_error.as_ref() {
        warnings.push(format!("Longitudinal patient signature: {error}"));
    }
    if let Some(result) = longitudinal.as_ref() {
        warnings.extend(
            result
                .warnings
                .iter()
                .map(|warning| format!("Longitudinal patient signature: {warning}")),
        );
        warnings.extend(result.modality_errors.iter().map(|error| {
            format!(
                "Longitudinal patient signature / {}: {}",
                error.modality, error.error
            )
        }));
    }
    if let Some(error) = significant_hit_structure_error.as_ref() {
        warnings.push(format!("Significant-hit structure: {error}"));
    }
    if let Some(result) = significant_hit_structure.as_ref() {
        warnings.extend(
            result
                .warnings
                .iter()
                .map(|warning| format!("Significant-hit structure: {warning}")),
        );
        warnings.extend(result.errors.iter().map(|error| {
            format!(
                "Significant-hit structure / {} / {}: {}",
                error.modality, error.feature_set, error.error
            )
        }));
    }

    progress(stage_total * 1000, stage_total * 1000, "Analysis complete");
    Ok(UnifiedCohortAnalysisRun {
        cohort,
        settings,
        qc_snapshot,
        primary,
        exploratory_support,
        diagnostics,
        diagnostics_error,
        longitudinal,
        longitudinal_error,
        significant_hit_structure,
        significant_hit_structure_error,
        candidates,
        warnings,
    })
}

use crate::cohort::{write_feature_tests, write_mapping_audit, write_mapping_audit_csv};
use crate::corrected::{
    write_adjusted_tests, write_corrected_repertoire_samples, write_dendrogram,
    write_dtu_gene_tests, write_ordination, write_pathway_ora,
};
use crate::diagnostics::write_pbmc_diagnostics_tables;
use crate::focused::write_key_findings_tables;
use crate::outliers::{
    write_outlier_filter_modalities_tsv, write_outlier_filter_snapshot_csv,
    write_outlier_filter_summary,
};
use anyhow::{Context, anyhow};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};

pub const TECHNICAL_EXPORT_MANIFEST_NAME: &str = "technical_export_manifest.json";

fn write_metadata_sample_exclusion_tables(
    outdir: &Path,
    snapshot: &MetadataSampleExclusionSnapshot,
) -> Result<()> {
    let excluded = snapshot
        .excluded_comparison_ids
        .iter()
        .map(|id| normalize_sample_id(id))
        .collect::<HashSet<_>>();
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(outdir.join("sample_metadata_exclusion.tsv"))
        .with_context(|| "Could not create sample_metadata_exclusion.tsv")?;
    writer.write_record(["metadata_id", "excluded_from_active_comparison"])?;
    for id in &snapshot.matching_metadata_ids {
        writer.write_record([
            id.as_str(),
            if excluded.contains(&normalize_sample_id(id)) {
                "true"
            } else {
                "false"
            },
        ])?;
    }
    writer.flush()?;

    fs::write(
        outdir.join("sample_metadata_exclusion_summary.txt"),
        format!(
            "Metadata sample exclusion\nColumn: {}\nExcluded values: {}\nMatching metadata IDs: {}\nExcluded active-comparison IDs: {}\nCase: {} -> {}\nControl: {} -> {}\nMatching is case-insensitive after trimming whitespace.\n",
            snapshot.column,
            if snapshot.excluded_values.is_empty() {
                "none".to_string()
            } else {
                snapshot.excluded_values.join(", ")
            },
            snapshot.matching_metadata_ids.len(),
            snapshot.excluded_comparison_ids.len(),
            snapshot.original_case_n,
            snapshot.filtered_case_n,
            snapshot.original_control_n,
            snapshot.filtered_control_n,
        ),
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TechnicalExportManifestEntry {
    pub relative_path: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct TechnicalExportManifest {
    pub schema_version: u32,
    pub files: Vec<TechnicalExportManifestEntry>,
}

#[derive(Debug)]
struct TechnicalExportHashResult {
    path: PathBuf,
    byte_size: u64,
    sha256: String,
}

/// Hash completed technical exports while later table families are still being
/// serialized. This removes the previous second full read of every table from
/// the synchronous artifact-retention tail while preserving independent SHA-256
/// integrity metadata for every opaque artifact.
struct TechnicalExportHashPool {
    root: PathBuf,
    seen: HashSet<PathBuf>,
    task_tx: Option<mpsc::Sender<PathBuf>>,
    result_rx: mpsc::Receiver<anyhow::Result<TechnicalExportHashResult>>,
    workers: Vec<std::thread::JoinHandle<()>>,
    submitted: usize,
}

impl TechnicalExportHashPool {
    fn new(root: &Path) -> Self {
        let (task_tx, task_rx) = mpsc::channel::<PathBuf>();
        let task_rx = Arc::new(Mutex::new(task_rx));
        let (result_tx, result_rx) = mpsc::channel();
        let available = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1);
        let worker_count = available.min(4).max(1);
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let task_rx = Arc::clone(&task_rx);
            let result_tx = result_tx.clone();
            workers.push(std::thread::spawn(move || {
                loop {
                    let path = {
                        let receiver = match task_rx.lock() {
                            Ok(receiver) => receiver,
                            Err(_) => break,
                        };
                        match receiver.recv() {
                            Ok(path) => path,
                            Err(_) => break,
                        }
                    };
                    let result = hash_completed_export(&path);
                    if result_tx.send(result).is_err() {
                        break;
                    }
                }
            }));
        }
        drop(result_tx);
        Self {
            root: root.to_path_buf(),
            seen: HashSet::new(),
            task_tx: Some(task_tx),
            result_rx,
            workers,
            submitted: 0,
        }
    }

    fn discover(&mut self) -> Result<()> {
        let mut files = Vec::new();
        collect_export_files_for_hashing(&self.root, &mut files)?;
        files.sort();
        for path in files {
            if path.file_name().and_then(|value| value.to_str())
                == Some(TECHNICAL_EXPORT_MANIFEST_NAME)
            {
                continue;
            }
            if self.seen.insert(path.clone()) {
                if let Some(tx) = self.task_tx.as_ref() {
                    tx.send(path).map_err(|_| {
                        anyhow!("technical export hash workers stopped unexpectedly")
                    })?;
                    self.submitted += 1;
                }
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<TechnicalExportManifest> {
        self.discover()?;
        self.task_tx.take();
        let mut files = Vec::with_capacity(self.submitted);
        for _ in 0..self.submitted {
            files.push(
                self.result_rx
                    .recv()
                    .context("technical export hash worker stopped unexpectedly")??,
            );
        }
        for worker in self.workers.drain(..) {
            worker
                .join()
                .map_err(|_| anyhow!("technical export hash worker panicked"))?;
        }
        let root = self.root;
        let mut entries = files
            .into_iter()
            .map(|result| {
                let relative = result.path.strip_prefix(&root).unwrap_or(&result.path);
                TechnicalExportManifestEntry {
                    relative_path: relative.to_string_lossy().replace('\\', "/"),
                    byte_size: result.byte_size,
                    sha256: result.sha256,
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(TechnicalExportManifest {
            schema_version: 1,
            files: entries,
        })
    }
}

fn collect_export_files_for_hashing(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_export_files_for_hashing(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn hash_completed_export(path: &Path) -> Result<TechnicalExportHashResult> {
    let mut file = fs::File::open(path).with_context(|| {
        format!(
            "Could not open completed export {} for hashing",
            path.display()
        )
    })?;
    let byte_size = file.metadata()?.len();
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8 * 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(TechnicalExportHashResult {
        path: path.to_path_buf(),
        byte_size,
        sha256: hex::encode(hasher.finalize()),
    })
}

/// Export the complete technical table set from a frozen unified run.
pub fn write_unified_analysis_tables(
    parent: &Path,
    run: &UnifiedCohortAnalysisRun,
) -> Result<PathBuf> {
    write_unified_analysis_tables_with_progress(parent, run, |_, _, _| {})
}

/// Export the complete technical table set from a frozen unified run while
/// reporting coarse-grained materialization progress.  The progress callback
/// is intentionally presentation-only: it does not alter which tables are
/// generated or any scientific result.
pub fn write_unified_analysis_tables_with_progress<F>(
    parent: &Path,
    run: &UnifiedCohortAnalysisRun,
    mut progress: F,
) -> Result<PathBuf>
where
    F: FnMut(usize, usize, &str),
{
    const STAGES: usize = 14;
    progress(0, STAGES, "Preparing technical table export");
    let safe_name: String = run
        .primary
        .comparison
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let outdir = parent.join(format!("4c_cohort_analysis_{safe_name}"));
    if outdir.exists() {
        fs::remove_dir_all(&outdir)
            .with_context(|| format!("Could not replace existing export {}", outdir.display()))?;
    }
    fs::create_dir_all(&outdir)
        .with_context(|| format!("Could not create {}", outdir.display()))?;
    let mut hash_pool = TechnicalExportHashPool::new(&outdir);

    write_mapping_audit(
        &outdir.join("sample_mapping_audit.tsv"),
        &run.cohort.mapping,
    )?;
    write_mapping_audit_csv(
        &outdir.join("sample_mapping_audit.csv"),
        &run.cohort.mapping,
    )?;
    progress(1, STAGES, "Technical tables: sample mapping audit");
    hash_pool.discover()?;

    if let Some(result) = run.primary.gene.as_ref() {
        progress(
            1,
            STAGES,
            "Technical tables: writing corrected gene expression",
        );
        write_adjusted_tests(&outdir.join("gene_adjusted_tests.tsv"), &result.tests)?;
        if let Some(ordination) = result.pca.as_ref() {
            write_ordination(&outdir.join("gene_pca.tsv"), ordination)?;
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            write_dendrogram(&outdir.join("gene_dendrogram_leaf_order.tsv"), tree)?;
        }
        if !result.warnings.is_empty() {
            fs::write(outdir.join("gene_warnings.txt"), result.warnings.join("\n"))?;
        }
    }
    progress(
        2,
        STAGES,
        "Technical tables: corrected gene expression complete",
    );
    hash_pool.discover()?;
    if let Some(result) = run.primary.transcript.as_ref() {
        let abundance_path = outdir.join("transcript_adjusted_tests.tsv");
        if let Some(dtu) = result.dtu.as_ref() {
            progress(
                2,
                STAGES,
                &format!(
                    "Technical tables: writing transcript abundance ({}) + DTU transcript ({}) rows in parallel",
                    result.tests.len(),
                    dtu.transcript_tests.len()
                ),
            );
            let dtu_transcript_path = outdir.join("transcript_dtu_transcript_tests.tsv");
            let dtu_gene_path = outdir.join("transcript_dtu_gene_tests.tsv");
            let (abundance_result, dtu_transcript_result, dtu_gene_result) =
                std::thread::scope(|scope| {
                    let abundance =
                        scope.spawn(|| write_adjusted_tests(&abundance_path, &result.tests));
                    let dtu_transcript = scope.spawn(|| {
                        write_adjusted_tests(&dtu_transcript_path, &dtu.transcript_tests)
                    });
                    let dtu_gene =
                        scope.spawn(|| write_dtu_gene_tests(&dtu_gene_path, &dtu.gene_tests));
                    (abundance.join(), dtu_transcript.join(), dtu_gene.join())
                });
            abundance_result
                .map_err(|_| anyhow!("transcript abundance export worker panicked"))??;
            dtu_transcript_result
                .map_err(|_| anyhow!("transcript DTU transcript export worker panicked"))??;
            dtu_gene_result
                .map_err(|_| anyhow!("transcript DTU gene export worker panicked"))??;
        } else {
            progress(
                2,
                STAGES,
                &format!(
                    "Technical tables: writing corrected transcript abundance ({} rows)",
                    result.tests.len()
                ),
            );
            write_adjusted_tests(&abundance_path, &result.tests)?;
        }
        progress(
            2,
            STAGES,
            "Technical tables: transcript abundance/DTU rows written",
        );
        if let Some(ordination) = result.pca.as_ref() {
            write_ordination(&outdir.join("transcript_pca.tsv"), ordination)?;
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            write_dendrogram(&outdir.join("transcript_dendrogram_leaf_order.tsv"), tree)?;
        }
        if !result.warnings.is_empty() {
            fs::write(
                outdir.join("transcript_warnings.txt"),
                result.warnings.join("\n"),
            )?;
        }
    }
    progress(
        3,
        STAGES,
        "Technical tables: corrected transcript expression and DTU complete",
    );
    hash_pool.discover()?;

    for (stage, prefix, label, result) in [
        (4, "bcr", "BCR repertoire", run.primary.bcr.as_ref()),
        (
            5,
            "tcr_alpha",
            "TCR alpha repertoire",
            run.primary.tcr_alpha.as_ref(),
        ),
        (
            6,
            "tcr_beta",
            "TCR beta repertoire",
            run.primary.tcr_beta.as_ref(),
        ),
    ] {
        if let Some(result) = result {
            write_corrected_repertoire_samples(
                &outdir.join(format!("{prefix}_matched_depth_sample_metrics.tsv")),
                &result.sample_summaries,
            )?;
            write_adjusted_tests(
                &outdir.join(format!("{prefix}_adjusted_metric_tests.tsv")),
                &result.metric_tests,
            )?;
            if let Some(ordination) = result.morisita_horn.as_ref() {
                write_ordination(
                    &outdir.join(format!("{prefix}_morisita_horn_pcoa.tsv")),
                    ordination,
                )?;
            }
            if let Some(ordination) = result.jaccard.as_ref() {
                write_ordination(
                    &outdir.join(format!("{prefix}_jaccard_pcoa.tsv")),
                    ordination,
                )?;
            }
            if let Some(tree) = result.dendrogram.as_ref() {
                write_dendrogram(
                    &outdir.join(format!("{prefix}_morisita_horn_dendrogram_leaf_order.tsv")),
                    tree,
                )?;
            }
            for composition in &result.compositions {
                let family: String = composition
                    .family
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() {
                            c.to_ascii_lowercase()
                        } else {
                            '_'
                        }
                    })
                    .collect();
                write_adjusted_tests(
                    &outdir.join(format!("{prefix}_{family}_clr_tests.tsv")),
                    &composition.tests,
                )?;
                if let Some(ordination) = composition.ordination.as_ref() {
                    write_ordination(
                        &outdir.join(format!("{prefix}_{family}_aitchison_pcoa.tsv")),
                        ordination,
                    )?;
                }
                if let Some(tree) = composition.dendrogram.as_ref() {
                    write_dendrogram(
                        &outdir.join(format!(
                            "{prefix}_{family}_aitchison_dendrogram_leaf_order.tsv"
                        )),
                        tree,
                    )?;
                }
            }
            if !result.warnings.is_empty() {
                fs::write(
                    outdir.join(format!("{prefix}_warnings.txt")),
                    result.warnings.join("\n"),
                )?;
            }
        }
        progress(stage, STAGES, &format!("Technical tables: {label}"));
        hash_pool.discover()?;
    }

    if let Some(result) = run.primary.bcr_isotype.as_ref() {
        write_adjusted_tests(&outdir.join("bcr_isotype_clr_tests.tsv"), &result.tests)?;
        if let Some(ordination) = result.ordination.as_ref() {
            write_ordination(&outdir.join("bcr_isotype_aitchison_pcoa.tsv"), ordination)?;
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            write_dendrogram(
                &outdir.join("bcr_isotype_aitchison_dendrogram_leaf_order.tsv"),
                tree,
            )?;
        }
        if !result.warnings.is_empty() {
            fs::write(
                outdir.join("bcr_isotype_warnings.txt"),
                result.warnings.join("\n"),
            )?;
        }
    }
    progress(7, STAGES, "Technical tables: BCR isotype composition");
    hash_pool.discover()?;
    progress(
        7,
        STAGES,
        "Technical tables: writing immune and pathway results",
    );
    if let Some(result) = run.primary.immune.as_ref() {
        write_adjusted_tests(&outdir.join("immune_adjusted_tests.tsv"), &result.tests)?;
        if let Some(ordination) = result.pca.as_ref() {
            write_ordination(&outdir.join("immune_pca.tsv"), ordination)?;
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            write_dendrogram(&outdir.join("immune_dendrogram_leaf_order.tsv"), tree)?;
        }
        if !result.warnings.is_empty() {
            fs::write(
                outdir.join("immune_warnings.txt"),
                result.warnings.join("\n"),
            )?;
        }
    }
    if let Some(result) = run.primary.pathway.as_ref() {
        write_pathway_ora(&outdir.join("pathway_ora.tsv"), result)?;
    }
    progress(
        8,
        STAGES,
        "Technical tables: immune and pathway results complete",
    );
    hash_pool.discover()?;

    let supporting_gene = run.exploratory_support.gene.as_ref();
    let supporting_transcript = run.exploratory_support.transcript.as_ref();
    if supporting_gene.is_some() || supporting_transcript.is_some() {
        progress(
            8,
            STAGES,
            &format!(
                "Technical tables: writing supporting direction checks (gene {} rows; transcript {} rows)",
                supporting_gene.map_or(0, |result| result.tests.len()),
                supporting_transcript.map_or(0, |result| result.tests.len())
            ),
        );
        let gene_path = outdir.join("supporting_gene_tpm_direction_tests.tsv");
        let transcript_path = outdir.join("supporting_transcript_tpm_direction_tests.tsv");
        let (gene_export, transcript_export) = std::thread::scope(|scope| {
            let gene = supporting_gene
                .map(|result| scope.spawn(|| write_feature_tests(&gene_path, &result.tests)));
            let transcript = supporting_transcript
                .map(|result| scope.spawn(|| write_feature_tests(&transcript_path, &result.tests)));
            (
                gene.map(|worker| worker.join()),
                transcript.map(|worker| worker.join()),
            )
        });
        if let Some(result) = gene_export {
            result.map_err(|_| anyhow!("supporting gene direction export worker panicked"))??;
        }
        if let Some(result) = transcript_export {
            result
                .map_err(|_| anyhow!("supporting transcript direction export worker panicked"))??;
        }
    }
    progress(
        9,
        STAGES,
        "Technical tables: supporting expression direction checks complete",
    );
    hash_pool.discover()?;

    if let Some(qc) = run.qc_snapshot.as_ref() {
        progress(9, STAGES, "Technical tables: writing QC provenance");
        write_outlier_filter_snapshot_csv(&outdir.join("sample_outlier_qc.csv"), qc)?;
        write_outlier_filter_modalities_tsv(&outdir.join("sample_outlier_qc_modalities.tsv"), qc)?;
        write_outlier_filter_summary(&outdir.join("sample_outlier_qc_summary.txt"), qc)?;
    }
    if let Some(metadata_exclusion) = run.settings.metadata_sample_exclusion.as_ref() {
        progress(
            9,
            STAGES,
            "Technical tables: writing metadata sample-exclusion provenance",
        );
        write_metadata_sample_exclusion_tables(&outdir, metadata_exclusion)?;
    }
    progress(10, STAGES, "Technical tables: QC provenance complete");
    hash_pool.discover()?;

    if let Some(longitudinal) = run.longitudinal.as_ref() {
        progress(
            10,
            STAGES,
            "Technical tables: writing longitudinal patient-signature clustering",
        );
        write_longitudinal_analysis_tables(
            &outdir.join("longitudinal_patient_signature"),
            longitudinal,
        )?;
        hash_pool.discover()?;
    }
    if let Some(hit_structure) = run.significant_hit_structure.as_ref() {
        progress(
            10,
            STAGES,
            "Technical tables: writing significant-hit PCA/dendrogram outputs",
        );
        write_significant_hit_structure_tables(
            &outdir.join("significant_hit_structure"),
            hit_structure,
        )?;
        hash_pool.discover()?;
    }

    let candidate_dir = outdir.join("key_findings");
    write_key_findings_tables(&candidate_dir, &run.candidates)?;
    progress(11, STAGES, "Technical tables: candidate prioritization");
    hash_pool.discover()?;

    if let Some(diagnostics) = run.diagnostics.as_ref() {
        progress(
            11,
            STAGES,
            "Technical tables: writing PBMC/confounding diagnostics",
        );
        write_pbmc_diagnostics_tables(&outdir.join("pbmc_confounding_diagnostics"), diagnostics)?;
    }
    progress(
        12,
        STAGES,
        "Technical tables: PBMC/confounding diagnostics complete",
    );
    hash_pool.discover()?;

    let comparison_definition = match &run.cohort.comparison_definition {
        CohortComparisonDefinition::Preset(preset) => {
            format!("Built-in preset: {}", preset.display_name())
        }
        CohortComparisonDefinition::MetadataValues(spec) => format!(
            "Custom metadata column '{}' | case values: {} | control values: {}",
            spec.column,
            spec.case_values
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            spec.control_values
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let mut summary = format!(
        "4C Cohort Explorer unified analysis\nComparison: {}\nComparison definition: {}\nCase: {}\nControl: {}\nMetadata: {}\nFiltered metadata subset: {}\nSubject column: {}\nCovariates: {}\n",
        run.primary.comparison.name,
        comparison_definition,
        run.primary.comparison.case_label,
        run.primary.comparison.control_label,
        run.cohort.metadata_path.display(),
        if run.cohort.uses_filtered_metadata_subset {
            "yes"
        } else {
            "no"
        },
        run.settings
            .corrected
            .subject_column
            .as_deref()
            .unwrap_or("none"),
        if run.settings.corrected.covariates.is_empty() {
            "none".to_string()
        } else {
            run.settings.corrected.covariates.join(", ")
        },
    );
    if let Some(qc) = run.qc_snapshot.as_ref() {
        summary.push_str(&format!(
            "QC exclusions applied: {}\nExcluded samples: {}\n",
            qc.excluded_ids.len(),
            if qc.excluded_ids.is_empty() {
                "none".to_string()
            } else {
                qc.excluded_ids.join(", ")
            }
        ));
    } else {
        summary.push_str("QC exclusions applied: 0\n");
    }
    if let Some(metadata_exclusion) = run.settings.metadata_sample_exclusion.as_ref() {
        summary.push_str(&format!(
            "Metadata sample exclusion: enabled\nMetadata exclusion column: {}\nMetadata exclusion values: {}\nMetadata rows/IDs matching rule: {}\nActive-comparison samples excluded by metadata rule: {}\n",
            metadata_exclusion.column,
            if metadata_exclusion.excluded_values.is_empty() {
                "none".to_string()
            } else {
                metadata_exclusion.excluded_values.join(", ")
            },
            metadata_exclusion.matching_metadata_ids.len(),
            metadata_exclusion.excluded_comparison_ids.len(),
        ));
    } else {
        summary.push_str("Metadata sample exclusion: disabled\n");
    }
    if let Some(longitudinal) = run.longitudinal.as_ref() {
        summary.push_str(&format!(
            "Longitudinal clustering: enabled\nLongitudinal observation/sample column: {}\nLongitudinal patient/person column: {}\nLongitudinal time/date column: {}\nLongitudinal QC/review column: {}\nLongitudinal QC values excluded: {}\nLongitudinal modalities completed: {}\n",
            longitudinal.config.observation_id_column,
            longitudinal.config.patient_id_column,
            longitudinal.config.time_column.as_deref().unwrap_or("none"),
            longitudinal.config.qc_status_column.as_deref().unwrap_or("none"),
            if longitudinal.config.excluded_qc_values.is_empty() {
                "none".to_string()
            } else {
                longitudinal
                    .config
                    .excluded_qc_values
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            longitudinal.modalities.len(),
        ));
    } else if let Some(error) = run.longitudinal_error.as_deref() {
        summary.push_str(&format!(
            "Longitudinal clustering: failed\nLongitudinal error: {error}\n"
        ));
    } else {
        summary.push_str("Longitudinal clustering: disabled\n");
    }
    if let Some(hit_structure) = run.significant_hit_structure.as_ref() {
        summary.push_str(&format!(
            "Significant-hit structure: enabled\nSignificant-hit BH q threshold: {}\nSignificant-hit minimum |adjusted effect|: {}\nSignificant-hit max analysis features: {}\nSignificant-hit panels completed: {}\n",
            hit_structure.config.q_threshold,
            hit_structure.config.min_abs_effect,
            hit_structure.config.max_analysis_features,
            hit_structure.panels.len(),
        ));
    } else if let Some(error) = run.significant_hit_structure_error.as_deref() {
        summary.push_str(&format!(
            "Significant-hit structure: failed\nSignificant-hit error: {error}\n"
        ));
    } else {
        summary.push_str("Significant-hit structure: disabled\n");
    }
    if !run.warnings.is_empty() {
        summary.push_str("\nWarnings:\n");
        for warning in &run.warnings {
            summary.push_str("- ");
            summary.push_str(warning);
            summary.push('\n');
        }
    }
    fs::write(outdir.join("analysis_summary.txt"), summary)?;
    progress(13, STAGES, "Technical tables: analysis summary written");
    hash_pool.discover()?;
    progress(
        13,
        STAGES,
        "Technical tables: finalizing precomputed integrity manifest",
    );
    let manifest = hash_pool.finish()?;
    fs::write(
        outdir.join(TECHNICAL_EXPORT_MANIFEST_NAME),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    progress(
        STAGES,
        STAGES,
        "Technical table materialization complete (all table SHA-256 values precomputed)",
    );
    Ok(outdir)
}

#[cfg(test)]
mod metadata_exclusion_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    fn test_metadata() -> MetadataTable {
        MetadataTable {
            path: PathBuf::from("metadata.tsv"),
            columns: vec!["sample_id".to_string(), "qc_review_status".to_string()],
            rows: vec![
                vec!["S1".to_string(), "Pass".to_string()],
                vec!["S2".to_string(), " Failed ".to_string()],
                vec!["S3".to_string(), "FAILED".to_string()],
                vec!["S4".to_string(), "Pass".to_string()],
            ],
            id_column: "sample_id".to_string(),
        }
    }

    fn test_comparison() -> CohortComparison {
        CohortComparison {
            name: "Case vs Control".to_string(),
            case_label: "Case".to_string(),
            control_label: "Control".to_string(),
            case_ids: ["S1".to_string(), "S2".to_string()].into_iter().collect(),
            control_ids: ["S3".to_string(), "S4".to_string()].into_iter().collect(),
            id_to_canonical: HashMap::new(),
            case_metadata_rows: 2,
            control_metadata_rows: 2,
        }
    }

    #[test]
    fn metadata_exclusion_matches_selected_values_case_insensitively() {
        let selected = ["failed".to_string()].into_iter().collect::<HashSet<_>>();
        let ids = metadata_exclusion_ids(&test_metadata(), "qc_review_status", &selected);
        assert_eq!(ids, vec!["S2".to_string(), "S3".to_string()]);
    }

    #[test]
    fn metadata_exclusion_snapshot_freezes_ids_and_group_counts() {
        let selected = ["Failed".to_string()].into_iter().collect::<HashSet<_>>();
        let snapshot = build_metadata_sample_exclusion_snapshot(
            &test_metadata(),
            "qc_review_status",
            &selected,
            &test_comparison(),
        );
        assert_eq!(
            snapshot.excluded_comparison_ids,
            vec!["S2".to_string(), "S3".to_string()]
        );
        assert_eq!(snapshot.original_case_n, 2);
        assert_eq!(snapshot.original_control_n, 2);
        assert_eq!(snapshot.filtered_case_n, 1);
        assert_eq!(snapshot.filtered_control_n, 1);
    }
}
