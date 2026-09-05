//! Unified cohort-analysis run model.
//!
//! This module does not implement a second statistical engine.  It owns the
//! configuration, provenance, corrected results, supporting diagnostics, and
//! candidate-prioritization output produced by one completed Cohort Analysis
//! run so every presentation/export can describe the exact same analysis.

use crate::cohort::{
    CohortComparison, CohortComparisonDefinition, CohortMapping, GeneComparisonResult,
};
use crate::corrected::{
    CorrectedExpressionResult, CorrectedImmuneResult, CorrectedIsotypeResult, CorrectedModelSpec,
    CorrectedPathwayResult, CorrectedRepertoireResult,
};
use crate::diagnostics::{PbmcDiagnosticsConfig, PbmcDiagnosticsResult};
use crate::focused::{FocusedCandidateSettings, FocusedCandidateSummary};
use crate::outliers::OutlierFilterSnapshot;
use std::path::PathBuf;

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
    pub candidates: FocusedCandidateSummary,
    pub warnings: Vec<String>,
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
    mut progress: F,
) -> Result<UnifiedCohortAnalysisRun>
where
    F: FnMut(usize, usize, &str),
{
    let selection = settings.modalities.clone();
    let comparison = cohort.comparison.clone();
    let spec = settings.corrected.clone();
    let diagnostics_requested = settings.pbmc_diagnostics.is_some();
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

    let mut exploratory_support = ExploratorySupportBundle::default();
    if settings.exploratory_direction_check && (selection.gene || selection.pathway) {
        progress(
            stage * 1000,
            stage_total * 1000,
            "Gene TPM direction-concordance check",
        );
        if let Some(path) = scan.merged_gene_tsv.as_deref() {
            match run_gene_comparison(path, &comparison, "TPM", |_, _, _| {}) {
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
            match run_transcript_comparison(path, &comparison, "TPM", |_, _, _| {}) {
                Ok(result) => exploratory_support.transcript = Some(result),
                Err(error) => exploratory_support.transcript_error = Some(format!("{error:#}")),
            }
        } else {
            exploratory_support.transcript_error =
                Some("No merged transcript-expression TSV detected.".to_string());
        }
        stage += 1;
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

    let mut candidates = build_focused_candidate_summary(
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
    if let Some(result) = diagnostics.as_ref() {
        annotate_focused_candidate_robustness(&mut candidates, result);
    }

    let mut warnings = primary.warnings();
    if let Some(error) = exploratory_support.gene_error.as_ref() {
        warnings.push(format!("Exploratory gene support: {error}"));
    }
    if let Some(error) = exploratory_support.transcript_error.as_ref() {
        warnings.push(format!("Exploratory transcript support: {error}"));
    }
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
    if let Some(qc) = qc_snapshot.as_ref() {
        warnings.extend(
            qc.audit
                .warnings
                .iter()
                .map(|warning| format!("Sample outlier QC: {warning}")),
        );
        for modality in &qc.audit.modalities {
            warnings.extend(
                modality
                    .warnings
                    .iter()
                    .map(|warning| format!("Sample outlier QC / {}: {warning}", modality.modality)),
            );
        }
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
use anyhow::Context;
use std::fs;
use std::path::Path;

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
    const STAGES: usize = 12;
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

    write_mapping_audit(
        &outdir.join("sample_mapping_audit.tsv"),
        &run.cohort.mapping,
    )?;
    write_mapping_audit_csv(
        &outdir.join("sample_mapping_audit.csv"),
        &run.cohort.mapping,
    )?;
    progress(1, STAGES, "Technical tables: sample mapping audit");

    if let Some(result) = run.primary.gene.as_ref() {
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
    progress(2, STAGES, "Technical tables: corrected gene expression");
    if let Some(result) = run.primary.transcript.as_ref() {
        write_adjusted_tests(&outdir.join("transcript_adjusted_tests.tsv"), &result.tests)?;
        if let Some(ordination) = result.pca.as_ref() {
            write_ordination(&outdir.join("transcript_pca.tsv"), ordination)?;
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            write_dendrogram(&outdir.join("transcript_dendrogram_leaf_order.tsv"), tree)?;
        }
        if let Some(dtu) = result.dtu.as_ref() {
            write_adjusted_tests(
                &outdir.join("transcript_dtu_transcript_tests.tsv"),
                &dtu.transcript_tests,
            )?;
            write_dtu_gene_tests(
                &outdir.join("transcript_dtu_gene_tests.tsv"),
                &dtu.gene_tests,
            )?;
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
        "Technical tables: corrected transcript expression and DTU",
    );

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
    progress(8, STAGES, "Technical tables: immune and pathway results");
    if let Some(result) = run.exploratory_support.gene.as_ref() {
        write_feature_tests(
            &outdir.join("supporting_gene_tpm_direction_tests.tsv"),
            &result.tests,
        )?;
    }
    if let Some(result) = run.exploratory_support.transcript.as_ref() {
        write_feature_tests(
            &outdir.join("supporting_transcript_tpm_direction_tests.tsv"),
            &result.tests,
        )?;
    }
    if let Some(qc) = run.qc_snapshot.as_ref() {
        write_outlier_filter_snapshot_csv(&outdir.join("sample_outlier_qc.csv"), qc)?;
        write_outlier_filter_modalities_tsv(&outdir.join("sample_outlier_qc_modalities.tsv"), qc)?;
        write_outlier_filter_summary(&outdir.join("sample_outlier_qc_summary.txt"), qc)?;
    }
    progress(
        9,
        STAGES,
        "Technical tables: supporting expression and QC provenance",
    );

    let candidate_dir = outdir.join("key_findings");
    write_key_findings_tables(&candidate_dir, &run.candidates)?;
    progress(10, STAGES, "Technical tables: candidate prioritization");

    if let Some(diagnostics) = run.diagnostics.as_ref() {
        write_pbmc_diagnostics_tables(&outdir.join("pbmc_confounding_diagnostics"), diagnostics)?;
    }
    progress(11, STAGES, "Technical tables: PBMC/confounding diagnostics");

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
    if !run.warnings.is_empty() {
        summary.push_str("\nWarnings:\n");
        for warning in &run.warnings {
            summary.push_str("- ");
            summary.push_str(warning);
            summary.push('\n');
        }
    }
    fs::write(outdir.join("analysis_summary.txt"), summary)?;
    progress(STAGES, STAGES, "Technical table materialization complete");
    Ok(outdir)
}
