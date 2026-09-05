use crate::analysis::UnifiedCohortAnalysisRun;
use crate::analysis_docs::{AnalysisDocToyChart, AnalysisDocToyExample, analysis_doc_sections};
use crate::cohort::{
    BcrComparisonResult, BcrIsotypeComparisonResult, BcrSampleSummary, CohortComparison,
    CohortComparisonDefinition, CohortMapping, CohortModality, FeatureTestResult,
    GeneComparisonResult, ImmuneComparisonResult, MappingStatus, TcrComparisonResult,
    TcrSampleSummary,
};
use crate::corrected::{
    AdjustedFeatureResult, CorrectedExpressionResult, CorrectedImmuneResult,
    CorrectedIsotypeResult, CorrectedModelSpec, CorrectedPathwayResult, CorrectedRepertoireResult,
    DendrogramResult, DtuGeneResult, OrdinationResult,
};
use crate::diagnostics::PbmcDiagnosticsResult;
use crate::focused::{
    FocusedCandidateSummary, FocusedGeneCandidate, FocusedOtherCandidate, FocusedPathwayCandidate,
};
use crate::outliers::{OutlierConsensusRule, OutlierFilterSnapshot};
use anyhow::{Context, Result};
use maud::{Markup, PreEscaped, html};
use plotly::common::{HoverInfo, Mode, Title};
use plotly::layout::Axis;
use plotly::{Layout, Plot, Scatter};
use report_builder::plots::plot_boxplot;
use report_builder::{Report, ReportSection};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

const REPORT_MAX_SCATTER_POINTS: usize = 4_000;

pub struct CohortReportInputs<'a> {
    pub comparison: &'a CohortComparison,
    pub mapping: Option<&'a CohortMapping>,
    pub gene: Option<&'a GeneComparisonResult>,
    pub gene_error: Option<&'a str>,
    pub transcript: Option<&'a GeneComparisonResult>,
    pub transcript_error: Option<&'a str>,
    pub bcr: Option<&'a BcrComparisonResult>,
    pub bcr_error: Option<&'a str>,
    pub bcr_isotype: Option<&'a BcrIsotypeComparisonResult>,
    pub bcr_isotype_error: Option<&'a str>,
    pub tcr_alpha: Option<&'a TcrComparisonResult>,
    pub tcr_alpha_error: Option<&'a str>,
    pub tcr_beta: Option<&'a TcrComparisonResult>,
    pub tcr_beta_error: Option<&'a str>,
    pub immune: Option<&'a ImmuneComparisonResult>,
    pub immune_error: Option<&'a str>,
    pub outlier_qc: Option<&'a OutlierFilterSnapshot>,
}

pub fn generate_cohort_html_report(path: &Path, inputs: CohortReportInputs<'_>) -> Result<()> {
    let title = format!("4C cohort analysis — {}", inputs.comparison.name);
    let mut report = Report::new(
        "4C Cohort Explorer",
        env!("CARGO_PKG_VERSION"),
        None,
        &title,
    );

    report.add_section(build_overview_section(&inputs));
    if let Some(mapping) = inputs.mapping {
        report.add_section(build_mapping_section(mapping));
    }
    if let Some(qc) = inputs.outlier_qc {
        report.add_section(build_outlier_qc_section(
            qc,
            "Sample outlier QC (exploratory audit)",
        ));
    }
    if inputs.gene.is_some() || inputs.gene_error.is_some() {
        report.add_section(build_gene_section(&inputs));
    }
    if inputs.transcript.is_some() || inputs.transcript_error.is_some() {
        report.add_section(build_transcript_section(&inputs));
    }
    if inputs.bcr.is_some() || inputs.bcr_error.is_some() {
        report.add_section(build_bcr_section(&inputs));
    }
    if inputs.bcr_isotype.is_some() || inputs.bcr_isotype_error.is_some() {
        report.add_section(build_bcr_isotype_section(&inputs));
    }
    if inputs.tcr_alpha.is_some() || inputs.tcr_alpha_error.is_some() {
        report.add_section(build_tcr_section(
            "TCR alpha",
            inputs.tcr_alpha,
            inputs.tcr_alpha_error,
            inputs.comparison,
        ));
    }
    if inputs.tcr_beta.is_some() || inputs.tcr_beta_error.is_some() {
        report.add_section(build_tcr_section(
            "TCR beta",
            inputs.tcr_beta,
            inputs.tcr_beta_error,
            inputs.comparison,
        ));
    }
    if inputs.immune.is_some() || inputs.immune_error.is_some() {
        report.add_section(build_immune_section(&inputs));
    }

    let output = path.to_string_lossy().to_string();
    report
        .save_to_file(&output)
        .with_context(|| format!("Could not write HTML report to {}", path.display()))?;
    make_report_self_contained(path)?;
    Ok(())
}

fn build_outlier_qc_section(snapshot: &OutlierFilterSnapshot, title: &str) -> ReportSection {
    let mut section = ReportSection::new(title);
    let cfg = &snapshot.audit.config;
    let consensus = snapshot
        .audit
        .consensus_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let excluded = snapshot
        .excluded_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let manual_excluded = excluded.difference(&consensus).count();
    let consensus_kept = consensus.difference(&excluded).count();
    let rule_detail = if cfg.consensus_rule == OutlierConsensusRule::AtLeastN {
        format!(
            "{} (N={})",
            cfg.consensus_rule.display_name(),
            cfg.at_least_n
        )
    } else {
        cfg.consensus_rule.display_name().to_string()
    };
    let excluded_evidence_rows = snapshot
        .audit
        .samples
        .iter()
        .filter(|row| excluded.contains(&row.sample_id))
        .map(|row| {
            let mut evidence = Vec::new();
            for modality in &snapshot.audit.modalities {
                if let Some(point) = modality
                    .points
                    .iter()
                    .find(|p| crate::cohort::normalize_sample_id(&p.sample_id) == row.sample_id)
                {
                    evidence.push(format!(
                        "{}: distance {:.2}{}",
                        modality.modality,
                        point.robust_distance,
                        if point.flagged { " (flagged)" } else { "" }
                    ));
                }
            }
            (
                row.sample_id.clone(),
                row.group.clone(),
                row.flagged_names.join("; "),
                evidence.join(" | "),
            )
        })
        .collect::<Vec<_>>();
    let decision_rows = snapshot
        .audit
        .samples
        .iter()
        .map(|row| {
            let is_consensus = consensus.contains(&row.sample_id);
            let is_excluded = excluded.contains(&row.sample_id);
            let decision = match (is_consensus, is_excluded) {
                (true, true) => "excluded — consensus flag",
                (true, false) => "kept — manual override",
                (false, true) => "excluded — manual override",
                (false, false) => "kept",
            }
            .to_string();
            (
                row.sample_id.clone(),
                row.group.clone(),
                row.available_modalities,
                row.flagged_modalities,
                is_consensus,
                is_excluded,
                decision,
                if row.flagged_names.is_empty() {
                    "—".to_string()
                } else {
                    row.flagged_names.join("; ")
                },
            )
        })
        .collect::<Vec<_>>();
    section.add_content(html! {
        h3 { "QC filter provenance" }
        p {
            strong { "Important: " }
            "The table below records the sample-outlier audit and reviewed exclusion decisions associated with this result. "
            "Outlier flags are unsupervised QC signals and are not, by themselves, proof that a sample is biologically invalid. "
            @if snapshot.applied_to_analysis {
                "The reviewed exclusion list shown here is the exact set removed before this analysis was run."
            } @else {
                "The QC audit was frozen with this run, but no reviewed sample exclusion was applied to the primary analysis."
            }
        }
        ul {
            li { strong { "QC comparison: " } (snapshot.audit.comparison_name) }
            li { strong { "Detection scope: " } (cfg.scope.display_name()) }
            li { strong { "Robust-distance threshold: " } (format!("{:.3}", cfg.robust_distance_threshold)) }
            li { strong { "Cross-modal consensus rule: " } (rule_detail) }
            li { strong { "Minimum available modalities: " } (cfg.min_available_modalities) }
            li { strong { "Consensus-flagged samples: " } (snapshot.audit.consensus_ids.len()) }
            li { strong { "Reviewed exclusions selected: " } (snapshot.excluded_ids.len()) }
            li { strong { "Applied to this analysis: " } (if snapshot.applied_to_analysis { "yes" } else { "no" }) }
            li { strong { "Manual exclusions outside consensus: " } (manual_excluded) }
            li { strong { "Consensus flags manually retained: " } (consensus_kept) }
            li { strong { "Case samples: " } (snapshot.original_case_n) " before reviewed exclusions → " (snapshot.filtered_case_n) " after reviewed exclusions" }
            li { strong { "Control samples: " } (snapshot.original_control_n) " before reviewed exclusions → " (snapshot.filtered_control_n) " after reviewed exclusions" }
        }
        @if !snapshot.excluded_ids.is_empty() {
            p { strong { "Excluded sample IDs: " } (snapshot.excluded_ids.join(", ")) }
        }
        @if !snapshot.audit.warnings.is_empty() {
            h4 { "QC warnings" }
            ul { @for warning in &snapshot.audit.warnings { li { (warning) } } }
        }
        @if !excluded_evidence_rows.is_empty() {
            h3 { "Why the excluded samples were flagged" }
            p { "The evidence below shows the modality-specific robust distances that were available when the reviewed exclusion decision was made. A modality is marked '(flagged)' when its robust distance exceeded the configured threshold under the selected detection scope." }
            div class="table-container" {
                table {
                    thead { tr { th { "Sample" } th { "Group" } th { "Automatically flagged in" } th { "Per-modality evidence" } } }
                    tbody {
                        @for (sample_id,group,flagged_in,evidence) in &excluded_evidence_rows {
                            tr { td { (sample_id) } td { (group) } td { (if flagged_in.is_empty(){"—".to_string()}else{flagged_in.clone()}) } td { (evidence) } }
                        }
                    }
                }
            }
        }
        h3 { "Per-modality QC summary" }
        div class="table-container" {
            table {
                thead { tr { th { "Modality" } th { "Representation" } th { "Samples" } th { "Flagged" } th { "Axes" } } }
                tbody {
                    @for modality in &snapshot.audit.modalities {
                        tr {
                            td { (modality.modality) }
                            td { (modality.representation) }
                            td { (modality.points.len()) }
                            td { (modality.points.iter().filter(|p| p.flagged).count()) }
                            td { (modality.x_label) " / " (modality.y_label) }
                        }
                    }
                }
            }
        }
        h3 { "Sample-level QC decisions" }
        p { "Decision labels distinguish automatic consensus flags from the reviewed set that was actually excluded. This makes manual overrides explicit." }
        div class="table-container" {
            table {
                thead { tr { th { "Sample" } th { "Group" } th { "Available modalities" } th { "Flagged modalities" } th { "Consensus" } th { "Reviewed exclusion" } th { "Decision" } th { "Flagged in" } } }
                tbody {
                    @for (sample_id,group,available,flagged,is_consensus,is_excluded,decision,flagged_in) in &decision_rows {
                        tr {
                            td { (sample_id) }
                            td { (group) }
                            td { (available) }
                            td { (flagged) }
                            td { (if *is_consensus { "yes" } else { "no" }) }
                            td { (if *is_excluded { "yes" } else { "no" }) }
                            td { (decision) }
                            td { (flagged_in) }
                        }
                    }
                }
            }
        }
    });
    for modality in &snapshot.audit.modalities {
        if let Some(plot) = outlier_modality_plot(modality, &excluded, snapshot.applied_to_analysis)
        {
            section.add_plot(plot);
        }
    }
    section
}

fn outlier_modality_plot(
    modality: &crate::outliers::ModalityOutlierResult,
    excluded: &BTreeSet<String>,
    applied_to_analysis: bool,
) -> Option<Plot> {
    let mut ordinary_x = Vec::new();
    let mut ordinary_y = Vec::new();
    let mut ordinary_hover = Vec::new();
    let mut flagged_x = Vec::new();
    let mut flagged_y = Vec::new();
    let mut flagged_hover = Vec::new();
    let mut excluded_x = Vec::new();
    let mut excluded_y = Vec::new();
    let mut excluded_hover = Vec::new();
    for point in &modality.points {
        let id = crate::cohort::normalize_sample_id(&point.sample_id);
        let hover = format!(
            "<b>{}</b><br>Group: {}<br>Robust distance: {:.3}<br>Modality flag: {}<br>Reviewed exclusion: {}",
            escape_hover_text(&point.sample_id),
            escape_hover_text(&point.group),
            point.robust_distance,
            if point.flagged { "yes" } else { "no" },
            if excluded.contains(&id) { "yes" } else { "no" },
        );
        if excluded.contains(&id) {
            excluded_x.push(point.x);
            excluded_y.push(point.y);
            excluded_hover.push(hover);
        } else if point.flagged {
            flagged_x.push(point.x);
            flagged_y.push(point.y);
            flagged_hover.push(hover);
        } else {
            ordinary_x.push(point.x);
            ordinary_y.push(point.y);
            ordinary_hover.push(hover);
        }
    }
    labeled_scatter_plot(
        vec![
            (
                "Ordinary".to_string(),
                ordinary_x,
                ordinary_y,
                ordinary_hover,
            ),
            (
                "Flagged / retained".to_string(),
                flagged_x,
                flagged_y,
                flagged_hover,
            ),
            (
                (if applied_to_analysis {
                    "Excluded from analysis"
                } else {
                    "Selected for downstream exclusion"
                })
                .to_string(),
                excluded_x,
                excluded_y,
                excluded_hover,
            ),
        ],
        &format!("{} sample outlier QC", modality.modality),
        &modality.x_label,
        &modality.y_label,
    )
}

fn embedded_plotly_script() -> Result<&'static str> {
    static EMBEDDED_PLOTLY: OnceLock<Result<String, String>> = OnceLock::new();
    let cached = EMBEDDED_PLOTLY.get_or_init(|| {
        let plot_shell = plotly::Plot::new().to_html();
        let script_start = plot_shell
            .find("<script")
            .ok_or_else(|| "Could not locate embedded Plotly JavaScript".to_string())?;
        let plot_div = plot_shell
            .find(r#"<div id="plotly-html-element""#)
            .ok_or_else(|| "Could not locate Plotly placeholder in standalone HTML".to_string())?;
        if script_start >= plot_div {
            return Err("Unexpected Plotly standalone HTML layout".to_string());
        }
        Ok(plot_shell[script_start..plot_div].to_string())
    });
    match cached {
        Ok(script) => Ok(script.as_str()),
        Err(error) => anyhow::bail!("{error}"),
    }
}

fn make_report_self_contained(path: &Path) -> Result<()> {
    let mut rendered = fs::read_to_string(path)
        .with_context(|| format!("Could not reopen generated HTML report {}", path.display()))?;

    // report-builder 0.1.1 embeds Plotly figures as inline fragments but currently
    // references Plotly and table-helper libraries from public CDNs. Cache Plotly's
    // bundled JavaScript once per process rather than rebuilding the multi-megabyte
    // bundle for both simplified and detailed reports.
    let embedded_plotly = embedded_plotly_script()?;
    const REPORT_BUILDER_PLOTLY_CDN: &str =
        r#"<script src="https://cdn.plot.ly/plotly-latest.min.js"></script>"#;
    if let Some(marker_pos) = rendered.find(REPORT_BUILDER_PLOTLY_CDN) {
        rendered.replace_range(
            marker_pos..marker_pos + REPORT_BUILDER_PLOTLY_CDN.len(),
            embedded_plotly,
        );
    } else {
        anyhow::bail!(
            "report-builder Plotly CDN marker was not found; its template may have changed"
        );
    }

    // Our report uses ordinary HTML tables, so DataTables/FileSaver are optional.
    // Strip their remote assets and the jQuery-dependent initializer so the final
    // file has no network dependencies and can be opened offline.
    for marker in [
        "https://cdnjs.cloudflare.com/ajax/libs/jquery/",
        "https://cdn.datatables.net/1.13.4/",
        "https://cdn.datatables.net/colresize/",
        "https://cdn.datatables.net/colResize/",
        "https://cdnjs.cloudflare.com/ajax/libs/FileSaver.js/",
    ] {
        rendered = strip_html_tag_containing(rendered, marker);
    }
    rendered = strip_script_block_containing(rendered, "$(document).ready(function()");

    fs::write(path, rendered).with_context(|| {
        format!(
            "Could not finalize standalone HTML report {}",
            path.display()
        )
    })?;
    Ok(())
}

fn strip_html_tag_containing(mut html: String, marker: &str) -> String {
    while let Some(marker_pos) = html.find(marker) {
        let Some(tag_start) = html[..marker_pos].rfind('<') else {
            break;
        };
        let Some(open_end_rel) = html[marker_pos..].find('>') else {
            break;
        };
        let open_end = marker_pos + open_end_rel + 1;
        if html[tag_start..].starts_with("<script") {
            if let Some(close_rel) = html[open_end..].find("</script>") {
                let close_end = open_end + close_rel + "</script>".len();
                html.replace_range(tag_start..close_end, "");
            } else {
                html.replace_range(tag_start..open_end, "");
            }
        } else {
            html.replace_range(tag_start..open_end, "");
        }
    }
    html
}

fn strip_script_block_containing(mut html: String, marker: &str) -> String {
    while let Some(marker_pos) = html.find(marker) {
        let Some(script_start) = html[..marker_pos].rfind("<script") else {
            break;
        };
        let Some(close_rel) = html[marker_pos..].find("</script>") else {
            break;
        };
        let close_end = marker_pos + close_rel + "</script>".len();
        html.replace_range(script_start..close_end, "");
    }
    html
}

fn build_overview_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    let mut section = ReportSection::new("Overview");
    let comp = inputs.comparison;
    let mapping_summary = inputs.mapping.map(|mapping| {
        let matched = mapping
            .rows
            .iter()
            .filter(|r| r.mapping_status == MappingStatus::Matched)
            .count();
        let unmatched = mapping
            .rows
            .iter()
            .filter(|r| r.mapping_status == MappingStatus::Unmatched)
            .count();
        let ambiguous = mapping
            .rows
            .iter()
            .filter(|r| r.mapping_status == MappingStatus::Ambiguous)
            .count();
        let metadata_only = mapping
            .rows
            .iter()
            .filter(|r| r.mapping_status == MappingStatus::MetadataOnly)
            .count();
        (matched, unmatched, ambiguous, metadata_only)
    });

    section.add_content(html! {
        h3 { (comp.name) }
        p { strong { "Case: " } (comp.case_label) " — " (comp.case_metadata_rows) " metadata rows" }
        p { strong { "Control: " } (comp.control_label) " — " (comp.control_metadata_rows) " metadata rows" }
        p {
            strong { "Interpretation: exploratory discovery analysis. " }
            "All currently mapped samples are treated as observations, including longitudinal samples. "
            "Age, sex, ethnicity, family structure, repeated sampling and other covariates are not adjusted in this pass."
        }
        @if let Some((matched, unmatched, ambiguous, metadata_only)) = mapping_summary {
            h4 { "Sample mapping summary" }
            ul {
                li { "Matched sample IDs: " (matched) }
                li { "Unmatched sample IDs: " (unmatched) }
                li { "Ambiguous sample IDs: " (ambiguous) }
                li { "Metadata-only sample IDs: " (metadata_only) }
            }
        }
        h4 { "Analysis availability" }
        ul {
            li { "Gene expression: " (availability(inputs.gene.is_some(), inputs.gene_error)) }
            li { "Transcript expression: " (availability(inputs.transcript.is_some(), inputs.transcript_error)) }
            li { "BCR: " (availability(inputs.bcr.is_some(), inputs.bcr_error)) }
            li { "BCR isotype: " (availability(inputs.bcr_isotype.is_some(), inputs.bcr_isotype_error)) }
            li { "TCR alpha: " (availability(inputs.tcr_alpha.is_some(), inputs.tcr_alpha_error)) }
            li { "TCR beta: " (availability(inputs.tcr_beta.is_some(), inputs.tcr_beta_error)) }
            li { "Immune infiltrate: " (availability(inputs.immune.is_some(), inputs.immune_error)) }
        }
        p {
            em {
                "This report is generated as one self-contained HTML file. Plotly JavaScript is embedded so interactive plots work offline; report-builder's optional CDN table helpers are removed during finalization."
            }
        }
    });
    section
}

fn build_mapping_section(mapping: &CohortMapping) -> ReportSection {
    let mut section = ReportSection::new("Sample mapping");
    section.add_content(html! {
        p { "Cross-modality mapping and coverage audit. Rows that are unmatched or ambiguous are retained rather than causing the analysis to fail." }
        div class="table-container" {
            table {
                thead {
                    tr {
                        th { "Sample ID" }
                        th { "Mapping" }
                        th { "Metadata row" }
                        th { "Phenotype" }
                        th { "HYP" }
                        th { "Gene" }
                        th { "Transcript" }
                        th { "BCR" }
                        th { "BCR isotype" }
                        th { "TCR α" }
                        th { "TCR β" }
                        th { "Immune infiltrate" }
                    }
                }
                tbody {
                    @for row in &mapping.rows {
                        tr {
                            td { (row.sample_id) }
                            td { (mapping_status_text(row.mapping_status)) }
                            td { (row.metadata_row.map(|x| (x + 2).to_string()).unwrap_or_default()) }
                            td { (row.phenotype) }
                            td { (row.hyp) }
                            td { (modality_count(row, CohortModality::GeneExpression)) }
                            td { (modality_count(row, CohortModality::TranscriptExpression)) }
                            td { (modality_count(row, CohortModality::Bcr)) }
                            td { (modality_count(row, CohortModality::BcrIsotype)) }
                            td { (modality_count(row, CohortModality::TcrAlpha)) }
                            td { (modality_count(row, CohortModality::TcrBeta)) }
                            td { (modality_count(row, CohortModality::ImmuneInfiltrate)) }
                        }
                    }
                }
            }
        }
    });
    section
}

fn build_gene_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    build_expression_section(
        "Gene expression",
        "Gene",
        "genes",
        inputs.gene,
        inputs.gene_error,
    )
}

fn build_transcript_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    build_expression_section(
        "Transcript expression",
        "Transcript",
        "transcripts",
        inputs.transcript,
        inputs.transcript_error,
    )
}

fn build_expression_section(
    section_name: &str,
    feature_name: &str,
    feature_name_plural: &str,
    result: Option<&GeneComparisonResult>,
    error: Option<&str>,
) -> ReportSection {
    let mut section = ReportSection::new(section_name);
    let Some(result) = result else {
        section.add_content(
            html! { p { (error.unwrap_or("Expression analysis was not available.")) } },
        );
        return section;
    };

    let n_sig = result
        .tests
        .iter()
        .filter(|r| r.q_value.is_finite() && r.q_value <= 0.05)
        .count();
    section.add_content(html! {
        p {
            "Metric: " strong { (result.metric) } ". "
            "Case columns: " (result.n_case_columns) ", control columns: " (result.n_control_columns) ", tested " (feature_name_plural) ": " (result.tests.len()) ", BH q≤0.05: " (n_sig) "."
        }
        p { "Mann–Whitney U is applied to log2(value + 1). Effect is log2((mean case + 0.5)/(mean control + 0.5)); BH adjustment is across tested features." }
    });

    if let Some(plot) = expression_volcano_plot(
        result,
        &format!("{section_name} discovery screen"),
        feature_name,
    ) {
        section.add_plot(plot);
    }
    if let Some(plot) = expression_detection_plot(
        result,
        &format!("{section_name} detection shift"),
        feature_name,
    ) {
        section.add_plot(plot);
    }
    section.add_content(feature_table(
        &format!("Top differential {feature_name_plural}"),
        &result.tests,
        100,
    ));
    section
}

fn build_bcr_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    let mut section = ReportSection::new("BCR");
    let Some(result) = inputs.bcr else {
        section.add_content(
            html! { p { (inputs.bcr_error.unwrap_or("BCR analysis was not available.")) } },
        );
        return section;
    };
    section.add_content(html! {
        p { "BCR files used: " (result.files_used) "; samples summarized: " (result.sample_summaries.len()) "." }
        @if !result.file_errors.is_empty() {
            details {
                summary { "File warnings (" (result.file_errors.len()) ")" }
                ul { @for warning in &result.file_errors { li { (warning) } } }
            }
        }
    });

    for (metric_name, getter) in report_bcr_metric_specs() {
        let case = result
            .sample_summaries
            .iter()
            .filter(|r| r.group == inputs.comparison.case_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let ctrl = result
            .sample_summaries
            .iter()
            .filter(|r| r.group == inputs.comparison.control_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if case.len() >= 2 && ctrl.len() >= 2 {
            let scores = vec![ctrl, case];
            let labels = vec![
                inputs.comparison.control_label.clone(),
                inputs.comparison.case_label.clone(),
            ];
            if let Ok(plot) = plot_boxplot(
                &scores,
                labels,
                &format!("BCR: {metric_name}"),
                "Group",
                metric_name,
            ) {
                section.add_plot(plot);
            }
        }
    }

    section.add_content(feature_table(
        "Sample-level BCR metrics",
        &result.metric_tests,
        50,
    ));
    section.add_content(feature_table(
        "Heavy-chain isotype usage",
        &result.isotype_tests,
        50,
    ));
    if let Some(plot) =
        feature_effect_significance_plot("BCR V-gene usage", &result.v_gene_tests, "V gene")
    {
        section.add_plot(plot);
    }
    section.add_content(feature_table("V-gene usage", &result.v_gene_tests, 75));
    if let Some(plot) =
        feature_effect_significance_plot("BCR D-gene usage", &result.d_gene_tests, "D gene")
    {
        section.add_plot(plot);
    }
    section.add_content(feature_table("D-gene usage", &result.d_gene_tests, 75));
    if let Some(plot) =
        feature_effect_significance_plot("BCR J-gene usage", &result.j_gene_tests, "J gene")
    {
        section.add_plot(plot);
    }
    section.add_content(feature_table("J-gene usage", &result.j_gene_tests, 75));
    section
}

fn build_bcr_isotype_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    let mut section = ReportSection::new("BCR isotype");
    let Some(result) = inputs.bcr_isotype else {
        section.add_content(html! { p { (inputs.bcr_isotype_error.unwrap_or("BCR-isotype analysis was not available.")) } });
        return section;
    };
    section.add_content(html! {
        p { "Dedicated BCR-isotype files used: " (result.files_used) "." }
    });

    let mut features = BTreeSet::new();
    for row in &result.values {
        features.insert(row.isotype.clone());
    }
    let mut scores = Vec::new();
    let mut labels = Vec::new();
    for feature in features {
        let ctrl = result
            .values
            .iter()
            .filter(|r| r.isotype == feature && r.group == inputs.comparison.control_label)
            .map(|r| r.fraction)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let case = result
            .values
            .iter()
            .filter(|r| r.isotype == feature && r.group == inputs.comparison.case_label)
            .map(|r| r.fraction)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if !ctrl.is_empty() {
            labels.push(format!("{} — {}", feature, inputs.comparison.control_label));
            scores.push(ctrl);
        }
        if !case.is_empty() {
            labels.push(format!("{} — {}", feature, inputs.comparison.case_label));
            scores.push(case);
        }
    }
    if !scores.is_empty() {
        if let Ok(plot) = plot_boxplot(
            &scores,
            labels,
            "BCR isotype composition by group",
            "Isotype / group",
            "Fraction",
        ) {
            section.add_plot(plot);
        }
    }
    if let Some(plot) = feature_effect_significance_plot(
        "BCR isotype fraction shifts",
        &result.tests,
        "BCR isotype",
    ) {
        section.add_plot(plot);
    }
    section.add_content(feature_table(
        "BCR isotype fraction tests",
        &result.tests,
        50,
    ));
    section.add_content(group_mean_isotype_table(result, inputs.comparison));
    section
}

fn build_tcr_section(
    title: &str,
    result: Option<&TcrComparisonResult>,
    error: Option<&str>,
    comparison: &CohortComparison,
) -> ReportSection {
    let mut section = ReportSection::new(title);
    let Some(result) = result else {
        section.add_content(html! { p { (error.unwrap_or("TCR analysis was not available.")) } });
        return section;
    };
    section.add_content(html! {
        p { (result.chain) " files used: " (result.files_used) "; samples summarized: " (result.sample_summaries.len()) "." }
        p { "This exploratory TCR section reports depth-dependent repertoire summaries plus V-gene, J-gene and V–J-pair composition. The corrected/standard section should be used for matched-depth diversity, repertoire distances, clonotype sharing, convergence, and composition-aware inference." }
        @if !result.file_errors.is_empty() {
            details {
                summary { "File warnings (" (result.file_errors.len()) ")" }
                ul { @for warning in &result.file_errors { li { (warning) } } }
            }
        }
    });

    for (metric_name, getter) in report_tcr_metric_specs() {
        let case = result
            .sample_summaries
            .iter()
            .filter(|r| r.group == comparison.case_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let ctrl = result
            .sample_summaries
            .iter()
            .filter(|r| r.group == comparison.control_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if case.len() >= 2 && ctrl.len() >= 2 {
            let scores = vec![ctrl, case];
            let labels = vec![
                comparison.control_label.clone(),
                comparison.case_label.clone(),
            ];
            if let Ok(plot) = plot_boxplot(
                &scores,
                labels,
                &format!("{}: {metric_name}", result.chain),
                "Group",
                metric_name,
            ) {
                section.add_plot(plot);
            }
        }
    }
    section.add_content(feature_table(
        &format!("{} sample-level repertoire metrics", result.chain),
        &result.metric_tests,
        50,
    ));
    if let Some(plot) = feature_effect_significance_plot(
        &format!("{} V-gene usage", result.chain),
        &result.v_gene_tests,
        "V gene",
    ) {
        section.add_plot(plot);
    }
    section.add_content(feature_table("V-gene usage", &result.v_gene_tests, 75));
    if let Some(plot) = feature_effect_significance_plot(
        &format!("{} J-gene usage", result.chain),
        &result.j_gene_tests,
        "J gene",
    ) {
        section.add_plot(plot);
    }
    section.add_content(feature_table("J-gene usage", &result.j_gene_tests, 75));
    if let Some(plot) = feature_effect_significance_plot(
        &format!("{} V–J pair usage", result.chain),
        &result.vj_pair_tests,
        "V-J pair",
    ) {
        section.add_plot(plot);
    }
    section.add_content(feature_table("V–J pair usage", &result.vj_pair_tests, 75));
    section
}

fn build_immune_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    let mut section = ReportSection::new("Immune infiltrate");
    let Some(result) = inputs.immune else {
        section.add_content(html! { p { (inputs.immune_error.unwrap_or("Immune-infiltrate analysis was not available.")) } });
        return section;
    };
    section.add_content(html! {
        p { "Immune-infiltrate files used: " (result.files_used) ". ssGSEA scores are compared independently for each immune-cell type using Mann–Whitney U, BH correction and Cliff's delta." }
        @if !result.file_errors.is_empty() {
            details {
                summary { "File warnings (" (result.file_errors.len()) ")" }
                ul { @for warning in &result.file_errors { li { (warning) } } }
            }
        }
    });
    if let Some(plot) =
        feature_effect_significance_plot("Immune-cell ssGSEA shifts", &result.tests, "Immune cell")
    {
        section.add_plot(plot);
    }

    let mut features = BTreeSet::new();
    for row in &result.values {
        features.insert(row.cell_type.clone());
    }
    for feature in features {
        let ctrl = result
            .values
            .iter()
            .filter(|r| r.cell_type == feature && r.group == inputs.comparison.control_label)
            .map(|r| r.score)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let case = result
            .values
            .iter()
            .filter(|r| r.cell_type == feature && r.group == inputs.comparison.case_label)
            .map(|r| r.score)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if ctrl.len() >= 2 && case.len() >= 2 {
            if let Ok(plot) = plot_boxplot(
                &vec![ctrl, case],
                vec![
                    inputs.comparison.control_label.clone(),
                    inputs.comparison.case_label.clone(),
                ],
                &format!("Immune infiltrate: {feature}"),
                "Group",
                "ssGSEA score",
            ) {
                section.add_plot(plot);
            }
        }
    }
    section.add_content(feature_table("Immune-cell ssGSEA tests", &result.tests, 50));
    section
}

fn labeled_scatter_plot(
    traces: Vec<(String, Vec<f64>, Vec<f64>, Vec<String>)>,
    title: &str,
    x_label: &str,
    y_label: &str,
) -> Option<Plot> {
    if traces.iter().all(|(_, x, _, _)| x.is_empty()) {
        return None;
    }
    let mut plot = Plot::new();
    for (name, x, y, hover) in traces {
        if x.is_empty() {
            continue;
        }
        let trace = Scatter::new(x, y)
            .mode(Mode::Markers)
            .name(name)
            .hover_text_array(hover)
            .hover_template("%{hovertext}<extra>%{fullData.name}</extra>");
        plot.add_trace(trace);
    }
    plot.set_layout(
        Layout::new()
            .title(Title::with_text(title))
            .x_axis(Axis::new().title(Title::with_text(x_label)))
            .y_axis(Axis::new().title(Title::with_text(y_label))),
    );
    Some(plot)
}

fn escape_hover_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn feature_hover(r: &FeatureTestResult, feature_label: &str) -> String {
    let feature = escape_hover_text(&r.feature);
    let secondary_value = escape_hover_text(&r.secondary_id);
    let secondary_label = match feature_label {
        "Gene" | "Transcript" => "NCBI Gene ID",
        "Immune cell" => "Lineage",
        _ => "Secondary ID",
    };
    let secondary = if r.secondary_id.trim().is_empty() {
        String::new()
    } else {
        format!("<br>{secondary_label}: {secondary_value}")
    };
    let detection = match (r.detection_case, r.detection_control) {
        (Some(case), Some(ctrl)) => format!(
            "<br>Detection case/control: {:.1}% / {:.1}%",
            case * 100.0,
            ctrl * 100.0
        ),
        _ => String::new(),
    };
    format!(
        "<b>{}: {}</b>{secondary}<br>Effect: {:.4}<br>Cliff's δ: {:.4}<br>p: {:.3e}<br>BH q: {:.3e}<br>Median case/control: {:.4} / {:.4}<br>Mean case/control: {:.4} / {:.4}<br>n case/control: {} / {}{detection}",
        feature_label,
        feature,
        r.effect,
        r.cliffs_delta,
        r.p_value,
        r.q_value,
        r.median_case,
        r.median_control,
        r.mean_case,
        r.mean_control,
        r.n_case,
        r.n_control,
    )
}

fn sampled_feature_tests_for_report<'a>(
    tests: &'a [FeatureTestResult],
) -> Vec<&'a FeatureTestResult> {
    let valid = tests
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row.effect.is_finite()
                && row.p_value.is_finite()
                && row.q_value.is_finite()
                && row.q_value > 0.0
        })
        .collect::<Vec<_>>();
    if valid.len() <= REPORT_MAX_SCATTER_POINTS {
        return valid.into_iter().map(|(_, row)| row).collect();
    }

    let mut ranked = valid.clone();
    ranked.sort_by(|(_, a), (_, b)| {
        a.q_value
            .total_cmp(&b.q_value)
            .then_with(|| a.p_value.total_cmp(&b.p_value))
            .then_with(|| b.effect.abs().total_cmp(&a.effect.abs()))
    });

    let signal_budget = REPORT_MAX_SCATTER_POINTS / 2;
    let mut selected = BTreeSet::new();
    for (idx, _) in ranked.iter().take(signal_budget) {
        selected.insert(*idx);
    }
    let remaining = REPORT_MAX_SCATTER_POINTS.saturating_sub(selected.len());
    if remaining > 0 {
        let background = valid
            .iter()
            .map(|(idx, _)| *idx)
            .filter(|idx| !selected.contains(idx))
            .collect::<Vec<_>>();
        let take = remaining.min(background.len());
        for sample_idx in 0..take {
            let position = sample_idx * background.len() / take;
            selected.insert(background[position]);
        }
    }

    selected
        .into_iter()
        .filter_map(|idx| tests.get(idx))
        .collect()
}

fn sampled_adjusted_tests_for_report<'a>(
    tests: &'a [AdjustedFeatureResult],
) -> Vec<&'a AdjustedFeatureResult> {
    let valid = tests
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row.beta_group.is_finite()
                && row.p_value.is_finite()
                && row.q_value.is_finite()
                && row.q_value > 0.0
        })
        .collect::<Vec<_>>();
    if valid.len() <= REPORT_MAX_SCATTER_POINTS {
        return valid.into_iter().map(|(_, row)| row).collect();
    }

    let mut ranked = valid.clone();
    ranked.sort_by(|(_, a), (_, b)| {
        a.q_value
            .total_cmp(&b.q_value)
            .then_with(|| a.p_value.total_cmp(&b.p_value))
            .then_with(|| b.beta_group.abs().total_cmp(&a.beta_group.abs()))
    });

    let signal_budget = REPORT_MAX_SCATTER_POINTS / 2;
    let mut selected = BTreeSet::new();
    for (idx, _) in ranked.iter().take(signal_budget) {
        selected.insert(*idx);
    }
    let remaining = REPORT_MAX_SCATTER_POINTS.saturating_sub(selected.len());
    if remaining > 0 {
        let background = valid
            .iter()
            .map(|(idx, _)| *idx)
            .filter(|idx| !selected.contains(idx))
            .collect::<Vec<_>>();
        let take = remaining.min(background.len());
        for sample_idx in 0..take {
            let position = sample_idx * background.len() / take;
            selected.insert(background[position]);
        }
    }

    selected
        .into_iter()
        .filter_map(|idx| tests.get(idx))
        .collect()
}

fn expression_volcano_plot(
    result: &GeneComparisonResult,
    title: &str,
    feature_label: &str,
) -> Option<Plot> {
    let mut x_other = Vec::new();
    let mut y_other = Vec::new();
    let mut h_other = Vec::new();
    let mut x_sig = Vec::new();
    let mut y_sig = Vec::new();
    let mut h_sig = Vec::new();
    for r in sampled_feature_tests_for_report(&result.tests) {
        let y = -r.q_value.max(1e-300).log10();
        let hover = feature_hover(r, feature_label);
        if r.q_value <= 0.05 {
            x_sig.push(r.effect);
            y_sig.push(y);
            h_sig.push(hover);
        } else {
            x_other.push(r.effect);
            y_other.push(y);
            h_other.push(hover);
        }
    }
    labeled_scatter_plot(
        vec![
            (
                format!(
                    "Other {}",
                    if feature_label == "Gene" {
                        "genes"
                    } else {
                        "transcripts"
                    }
                ),
                x_other,
                y_other,
                h_other,
            ),
            ("BH q ≤ 0.05".to_string(), x_sig, y_sig, h_sig),
        ],
        title,
        "log2 mean fold change (case / control)",
        "-log10(BH q)",
    )
}

fn expression_detection_plot(
    result: &GeneComparisonResult,
    title: &str,
    feature_label: &str,
) -> Option<Plot> {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut hover = Vec::new();
    for r in sampled_feature_tests_for_report(&result.tests) {
        let (Some(case), Some(ctrl)) = (r.detection_case, r.detection_control) else {
            continue;
        };
        if case.is_finite() && ctrl.is_finite() {
            x.push(ctrl * 100.0);
            y.push(case * 100.0);
            let secondary_label = if feature_label == "Gene" || feature_label == "Transcript" {
                "NCBI Gene ID"
            } else {
                "Secondary ID"
            };
            let secondary = if r.secondary_id.trim().is_empty() {
                String::new()
            } else {
                format!(
                    "<br>{}: {}",
                    secondary_label,
                    escape_hover_text(&r.secondary_id)
                )
            };
            hover.push(format!(
                "<b>{}: {}</b>{secondary}<br>Control detection: {:.1}%<br>Case detection: {:.1}%<br>Δ detection: {:+.1} percentage points<br>Effect: {:.4}<br>BH q: {:.3e}",
                feature_label,
                escape_hover_text(&r.feature),
                ctrl * 100.0,
                case * 100.0,
                (case - ctrl) * 100.0,
                r.effect,
                r.q_value,
            ));
        }
    }
    labeled_scatter_plot(
        vec![(format!("{feature_label}s"), x, y, hover)],
        title,
        "Control detection (%)",
        "Case detection (%)",
    )
}

fn feature_effect_significance_plot(
    title: &str,
    tests: &[FeatureTestResult],
    feature_label: &str,
) -> Option<Plot> {
    let mut x_other = Vec::new();
    let mut y_other = Vec::new();
    let mut h_other = Vec::new();
    let mut x_sig = Vec::new();
    let mut y_sig = Vec::new();
    let mut h_sig = Vec::new();
    for r in sampled_feature_tests_for_report(tests) {
        let y = -r.q_value.max(1e-300).log10();
        let hover = feature_hover(r, feature_label);
        if r.q_value <= 0.05 {
            x_sig.push(r.effect);
            y_sig.push(y);
            h_sig.push(hover);
        } else {
            x_other.push(r.effect);
            y_other.push(y);
            h_other.push(hover);
        }
    }
    labeled_scatter_plot(
        vec![
            ("Other".to_string(), x_other, y_other, h_other),
            ("BH q ≤ 0.05".to_string(), x_sig, y_sig, h_sig),
        ],
        title,
        "Effect (case - control)",
        "-log10(BH q)",
    )
}

fn feature_table(title: &str, tests: &[FeatureTestResult], limit: usize) -> Markup {
    html! {
        h3 { (title) " (" (tests.len()) " rows; showing up to " (limit.min(tests.len())) ")" }
        div class="table-container" {
            table {
                thead { tr {
                    th { "Feature" } th { "Secondary ID" } th { "Effect" } th { "Cliff δ" }
                    th { "p" } th { "BH q" } th { "Median case" } th { "Median control" }
                    th { "n case" } th { "n control" }
                } }
                tbody {
                    @for r in tests.iter().take(limit) {
                        tr {
                            td { (r.feature) }
                            td { (r.secondary_id) }
                            td { (fmt(r.effect)) }
                            td { (fmt(r.cliffs_delta)) }
                            td { (fmt_sci(r.p_value)) }
                            td { (fmt_sci(r.q_value)) }
                            td { (fmt(r.median_case)) }
                            td { (fmt(r.median_control)) }
                            td { (r.n_case) }
                            td { (r.n_control) }
                        }
                    }
                }
            }
        }
    }
}

fn group_mean_isotype_table(
    result: &BcrIsotypeComparisonResult,
    comp: &CohortComparison,
) -> Markup {
    let mut by_feature: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for row in &result.values {
        let entry = by_feature.entry(row.isotype.clone()).or_default();
        if row.group == comp.case_label {
            entry.0.push(row.fraction);
        } else if row.group == comp.control_label {
            entry.1.push(row.fraction);
        }
    }
    html! {
        h3 { "Group mean isotype composition" }
        div class="table-container" { table {
            thead { tr { th { "Isotype" } th { (comp.case_label) } th { (comp.control_label) } } }
            tbody {
                @for (feature, (case, ctrl)) in by_feature {
                    tr {
                        td { (feature) }
                        td { (fmt(mean(&case))) }
                        td { (fmt(mean(&ctrl))) }
                    }
                }
            }
        } }
    }
}

fn report_bcr_metric_specs() -> [(&'static str, fn(&BcrSampleSummary) -> f64); 4] {
    [
        ("Shannon diversity", |r| r.shannon),
        ("Clonality", |r| r.clonality),
        ("Unique clones", |r| r.unique_clones),
        ("Top clone fraction", |r| r.top_clone_fraction),
    ]
}

fn report_tcr_metric_specs() -> [(&'static str, fn(&TcrSampleSummary) -> f64); 4] {
    [
        ("Shannon diversity", |r| r.shannon),
        ("Clonality", |r| r.clonality),
        ("Unique clones", |r| r.unique_clones),
        ("Top clone fraction", |r| r.top_clone_fraction),
    ]
}

fn availability(has_result: bool, error: Option<&str>) -> String {
    if has_result {
        "available".to_string()
    } else if let Some(err) = error {
        format!("not available — {err}")
    } else {
        "not requested".to_string()
    }
}

fn mapping_status_text(status: MappingStatus) -> &'static str {
    match status {
        MappingStatus::Matched => "matched",
        MappingStatus::Unmatched => "unmatched",
        MappingStatus::Ambiguous => "ambiguous",
        MappingStatus::MetadataOnly => "metadata only",
    }
}

fn modality_count(row: &crate::cohort::SampleCoverageRow, modality: CohortModality) -> usize {
    row.counts.get(&modality).copied().unwrap_or(0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        f64::NAN
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn fmt(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.4}")
    } else {
        "NA".to_string()
    }
}

fn fmt_sci(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3e}")
    } else {
        "NA".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cohort::{CohortComparison, FeatureTestResult, GeneComparisonResult};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn writes_cohort_html_report_with_gene_plot() {
        let comparison = CohortComparison {
            name: "HYP vs True_HC".to_string(),
            case_label: "HYP".to_string(),
            control_label: "True_HC".to_string(),
            case_ids: HashSet::new(),
            control_ids: HashSet::new(),
            id_to_canonical: HashMap::new(),
            case_metadata_rows: 3,
            control_metadata_rows: 3,
        };
        let gene = GeneComparisonResult {
            input: "merged.tsv".into(),
            metric: "TPM".to_string(),
            n_case_columns: 3,
            n_control_columns: 3,
            unmatched_columns: 0,
            tests: vec![FeatureTestResult {
                feature: "TEST1".to_string(),
                secondary_id: "1".to_string(),
                n_case: 3,
                n_control: 3,
                mean_case: 4.0,
                mean_control: 2.0,
                median_case: 4.0,
                median_control: 2.0,
                effect: 0.85,
                cliffs_delta: 0.5,
                p_value: 0.01,
                q_value: 0.02,
                detection_case: Some(1.0),
                detection_control: Some(0.67),
            }],
        };
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("report.html");
        generate_cohort_html_report(
            &path,
            CohortReportInputs {
                comparison: &comparison,
                mapping: None,
                gene: Some(&gene),
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
                outlier_qc: None,
            },
        )
        .unwrap();
        let html = std::fs::read_to_string(path).unwrap();
        assert!(html.contains("4C Cohort Explorer"));
        assert!(html.contains("Gene expression"));
        assert!(html.contains("TEST1"));
        assert!(html.contains("Gene: TEST1"));
        assert!(html.contains("NCBI Gene ID: 1"));
        assert!(
            !html.contains(r#"<script src="https://cdn.plot.ly/plotly-latest.min.js"></script>"#)
        );
        assert!(!html.contains("https://cdn.datatables.net/"));
        assert!(!html.contains("https://cdnjs.cloudflare.com/ajax/libs/jquery/"));
        assert!(html.contains("Plotly"));
    }

    #[test]
    fn report_scatter_sampling_is_bounded_and_retains_top_signal() {
        let tests = (0..5_500)
            .map(|i| FeatureTestResult {
                feature: format!("F{i}"),
                secondary_id: i.to_string(),
                n_case: 4,
                n_control: 4,
                mean_case: 2.0,
                mean_control: 1.0,
                median_case: 2.0,
                median_control: 1.0,
                effect: i as f64 / 1000.0,
                cliffs_delta: 0.25,
                p_value: if i == 5_499 { 1e-12 } else { 0.5 },
                q_value: if i == 5_499 { 1e-10 } else { 0.8 },
                detection_case: Some(1.0),
                detection_control: Some(0.75),
            })
            .collect::<Vec<_>>();
        let sampled = sampled_feature_tests_for_report(&tests);
        assert_eq!(sampled.len(), REPORT_MAX_SCATTER_POINTS);
        assert!(sampled.iter().any(|row| row.feature == "F5499"));
    }

    #[test]
    fn analysis_doc_toy_example_renders_inline_svg() {
        let example = &analysis_doc_sections()[0].examples[0];
        let rendered = build_analysis_doc_toy_example_markup(example).into_string();
        assert!(rendered.contains("Toy example"));
        assert!(rendered.contains("Illustrative values only"));
        assert!(rendered.contains("<svg"));
        assert!(rendered.contains(example.title));
    }
}

pub struct CorrectedReportInputs<'a> {
    pub comparison: &'a CohortComparison,
    pub mapping: Option<&'a CohortMapping>,
    pub spec: &'a CorrectedModelSpec,
    pub gene: Option<&'a CorrectedExpressionResult>,
    pub gene_error: Option<&'a str>,
    pub transcript: Option<&'a CorrectedExpressionResult>,
    pub transcript_error: Option<&'a str>,
    pub bcr: Option<&'a CorrectedRepertoireResult>,
    pub bcr_error: Option<&'a str>,
    pub bcr_isotype: Option<&'a CorrectedIsotypeResult>,
    pub bcr_isotype_error: Option<&'a str>,
    pub tcr_alpha: Option<&'a CorrectedRepertoireResult>,
    pub tcr_alpha_error: Option<&'a str>,
    pub tcr_beta: Option<&'a CorrectedRepertoireResult>,
    pub tcr_beta_error: Option<&'a str>,
    pub immune: Option<&'a CorrectedImmuneResult>,
    pub immune_error: Option<&'a str>,
    pub pathway: Option<&'a CorrectedPathwayResult>,
    pub pathway_error: Option<&'a str>,
    pub outlier_qc: Option<&'a OutlierFilterSnapshot>,
}

pub fn generate_corrected_html_report(
    path: &Path,
    inputs: CorrectedReportInputs<'_>,
) -> Result<()> {
    let title = format!("4C corrected cohort analysis — {}", inputs.comparison.name);
    let mut report = Report::new(
        "4C Cohort Explorer",
        env!("CARGO_PKG_VERSION"),
        None,
        &title,
    );
    let subject = inputs.spec.subject_column.as_deref().unwrap_or("none");
    let covariates = if inputs.spec.covariates.is_empty() {
        "none".to_string()
    } else {
        inputs.spec.covariates.join(", ")
    };
    let mut overview = ReportSection::new("Overview");
    overview.add_content(html! {
        h3 { (inputs.comparison.name) " — corrected / standard layer" }
        p { strong { "Case: " } (inputs.comparison.case_label) " | " strong { "Control: " } (inputs.comparison.control_label) }
        p { strong { "Subject / repeated-measure column: " } (subject) }
        p { strong { "Covariates: " } (covariates) }
        @if let Some(qc)=inputs.outlier_qc {
            p { strong { "QC-filtered analysis: " } (qc.excluded_ids.len()) " reviewed sample exclusion(s) were applied before modelling (case " (qc.original_case_n) " → " (qc.filtered_case_n) ", control " (qc.original_control_n) " → " (qc.filtered_control_n) ")." }
        }
        p { "Expression uses positive-count median-ratio normalization with a negative-binomial log-link count model, optional covariates, subject-clustered standard errors when repeated samples are configured, and a subject-collapsed sensitivity analysis." }
        p { "Repertoire analyses use matched-depth rarefaction, subject-aware clonotype sharing, Morisita-Horn and Jaccard distances with PCoA/PERMANOVA, average-linkage hierarchical clustering, and CLR/Aitchison composition analysis. Immune-infiltrate tests use original ssGSEA scores with adjusted models and standardized scores for unsupervised sample structure." }
    });
    report.add_section(overview);
    if let Some(mapping) = inputs.mapping {
        report.add_section(build_mapping_section(mapping));
    }
    if let Some(qc) = inputs.outlier_qc {
        report.add_section(build_outlier_qc_section(
            qc,
            "Sample QC and reviewed exclusions",
        ));
    }
    if let Some(r) = inputs.gene {
        report.add_section(build_corrected_expression_section("Gene counts", r));
    } else if let Some(e) = inputs.gene_error {
        report.add_section(corrected_error_section("Gene counts", e));
    }
    if let Some(r) = inputs.transcript {
        report.add_section(build_corrected_expression_section("Transcript counts", r));
    } else if let Some(e) = inputs.transcript_error {
        report.add_section(corrected_error_section("Transcript counts", e));
    }
    if let Some(r) = inputs.bcr {
        report.add_section(build_corrected_repertoire_section("BCR", r));
    } else if let Some(e) = inputs.bcr_error {
        report.add_section(corrected_error_section("BCR", e));
    }
    if let Some(r) = inputs.bcr_isotype {
        report.add_section(build_corrected_isotype_section(r));
    } else if let Some(e) = inputs.bcr_isotype_error {
        report.add_section(corrected_error_section("BCR isotype", e));
    }
    if let Some(r) = inputs.tcr_alpha {
        report.add_section(build_corrected_repertoire_section("TCR alpha", r));
    } else if let Some(e) = inputs.tcr_alpha_error {
        report.add_section(corrected_error_section("TCR alpha", e));
    }
    if let Some(r) = inputs.tcr_beta {
        report.add_section(build_corrected_repertoire_section("TCR beta", r));
    } else if let Some(e) = inputs.tcr_beta_error {
        report.add_section(corrected_error_section("TCR beta", e));
    }
    if let Some(r) = inputs.immune {
        report.add_section(build_corrected_immune_section(r));
    } else if let Some(e) = inputs.immune_error {
        report.add_section(corrected_error_section("Immune infiltrate", e));
    }
    if let Some(r) = inputs.pathway {
        report.add_section(build_corrected_pathway_section(r));
    } else if let Some(e) = inputs.pathway_error {
        report.add_section(corrected_error_section("Pathways", e));
    }
    let output = path.to_string_lossy().to_string();
    report.save_to_file(&output).with_context(|| {
        format!(
            "Could not write corrected HTML report to {}",
            path.display()
        )
    })?;
    make_report_self_contained(path)?;
    Ok(())
}

fn corrected_error_section(title: &str, error: &str) -> ReportSection {
    let mut s = ReportSection::new(title);
    s.add_content(html! {p{(error)}});
    s
}

fn build_corrected_expression_section(title: &str, r: &CorrectedExpressionResult) -> ReportSection {
    let mut s = ReportSection::new(title);
    s.add_content(html!{
        p { (r.normalization) }
        p { (r.model_description) }
        p { "Model samples: " (r.n_model_samples) ", subjects: " (r.n_subjects) ", tested features: " (r.tested_features) ", low-count filtered: " (r.filtered_features) "." }
        @if r.tests.len() > REPORT_MAX_SCATTER_POINTS {
            p { small { "Interactive feature plots show up to " (REPORT_MAX_SCATTER_POINTS) " points, prioritizing the strongest corrected signals plus a deterministic background sample. Complete feature-level results remain in the exported technical tables." } }
        }
        h3 { "How to read this analysis" }
        p { "The adjusted case/control coefficient tests whether each feature differs after the selected covariates are included. Positive coefficients indicate higher modeled abundance in cases; negative coefficients indicate lower modeled abundance. BH q-values control the false-discovery rate across this feature family." }
        p { "PCA and hierarchical clustering are unsupervised sample-level views. Separation by case/control can support a broad phenotype signal, while other clustering may reflect subject, batch, timepoint, or other metadata. These plots are descriptive and should be read alongside the feature-level tests." }
        @for w in &r.warnings { p { em { "Warning: " (w) } } }
    });
    if let Some(p) = adjusted_volcano_plot(&r.tests, &format!("{title}: adjusted feature screen")) {
        s.add_plot(p);
    }
    if let Some(ord) = &r.pca {
        if let Some(p) = corrected_ordination_plot(ord) {
            s.add_plot(p);
        }
    }
    if let Some(tree) = &r.dendrogram {
        if let Some(p) = corrected_dendrogram_plot(tree) {
            s.add_plot(p);
        }
    }
    s.add_content(adjusted_table("Adjusted feature tests", &r.tests, 150));
    if let Some(dtu) = &r.dtu {
        s.add_content(html!{
            h3 { "Differential transcript usage" }
            p { (dtu.method_description) }
            p { "Genes tested: " (dtu.genes_tested) ", transcripts tested: " (dtu.transcripts_tested) "." }
        });
        s.add_content(dtu_gene_table(&dtu.gene_tests, 120));
        s.add_content(adjusted_table(
            "Transcript-level CLR usage tests",
            &dtu.transcript_tests,
            150,
        ));
    }
    s
}

fn build_corrected_repertoire_section(title: &str, r: &CorrectedRepertoireResult) -> ReportSection {
    let mut s = ReportSection::new(title);
    s.add_content(html!{
        p { "Samples: " (r.files_used) ", matched depth: " (r.matched_depth) ", rarefaction repeats: " (r.rarefaction_reps) "." }
        p { "Ambiguous V/D/J calls: " (r.ambiguous_policy.display_name()) }
        h3 { "How to read this analysis" }
        p { "Matched-depth richness and Shannon diversity compare repertoires at the same read depth. Morisita-Horn emphasizes abundance-weighted clonotype similarity, whereas Jaccard uses clonotype presence/absence. PCoA visualizes those distances and PERMANOVA tests whether the two groups differ globally." }
        p { "The dendrogram is an unsupervised average-linkage clustering of Morisita-Horn distance. CLR/Aitchison analyses address V/D/J/V-J or isotype compositions as relative compositions rather than independent percentages." }
        @for w in &r.warnings { p { em { "Warning: " (w) } } }
    });
    s.add_content(adjusted_table(
        "Adjusted matched-depth / repertoire metrics",
        &r.metric_tests,
        100,
    ));
    if let Some(ord) = &r.morisita_horn {
        if let Some(p) = corrected_ordination_plot(ord) {
            s.add_plot(p);
        }
    }
    if let Some(tree) = &r.dendrogram {
        if let Some(p) = corrected_dendrogram_plot(tree) {
            s.add_plot(p);
        }
    }
    if let Some(ord) = &r.jaccard {
        if let Some(p) = corrected_ordination_plot(ord) {
            s.add_plot(p);
        }
    }
    for comp in &r.compositions {
        s.add_content(html!{h3{(comp.family) " — CLR/Aitchison"} p{"Retained features: "(comp.retained_features)}});
        if let Some(ord) = &comp.ordination {
            if let Some(p) = corrected_ordination_plot(ord) {
                s.add_plot(p);
            }
        }
        if let Some(tree) = &comp.dendrogram {
            if let Some(p) = corrected_dendrogram_plot(tree) {
                s.add_plot(p);
            }
        }
        s.add_content(adjusted_table(
            &format!("{} adjusted CLR tests", comp.family),
            &comp.tests,
            80,
        ));
    }
    s
}

fn build_corrected_isotype_section(r: &CorrectedIsotypeResult) -> ReportSection {
    let mut s = ReportSection::new("BCR isotype");
    s.add_content(html!{p{"Files/samples: "(r.files_used) ". " @if r.determined_only {"Undetermined was excluded and the determined isotypes were renormalized."} @else {"Undetermined was retained in the composition."}} h3{"How to read this analysis"} p{"Isotype fractions are compositional: individual components are interpreted relative to the remaining isotype mixture. PCoA/PERMANOVA and hierarchical clustering summarize whole-composition differences, while the CLR tests identify components contributing to those differences."} @for w in &r.warnings{p{em{"Warning: "(w)}}}});
    if let Some(ord) = &r.ordination {
        if let Some(p) = corrected_ordination_plot(ord) {
            s.add_plot(p);
        }
    }
    if let Some(tree) = &r.dendrogram {
        if let Some(p) = corrected_dendrogram_plot(tree) {
            s.add_plot(p);
        }
    }
    s.add_content(adjusted_table("Isotype CLR tests", &r.tests, 50));
    s
}

fn build_corrected_immune_section(r: &CorrectedImmuneResult) -> ReportSection {
    let mut s = ReportSection::new("Immune infiltrate");
    s.add_content(html!{p{"Samples: "(r.files_used) ". Original ssGSEA scores are used for adjusted tests; PCA and clustering use cell-type z-scores."} h3{"How to read this analysis"} p{"Each immune-cell signature is compared across groups on its original ssGSEA scale after adjustment. ssGSEA values are enrichment scores rather than literal cell percentages. PCA and hierarchical clustering use standardized cell-type scores to summarize coordinated immune-profile structure."} @for w in &r.warnings{p{em{"Warning: "(w)}}}});
    if let Some(ord) = &r.pca {
        if let Some(p) = corrected_ordination_plot(ord) {
            s.add_plot(p);
        }
    }
    if let Some(tree) = &r.dendrogram {
        if let Some(p) = corrected_dendrogram_plot(tree) {
            s.add_plot(p);
        }
    }
    s.add_content(adjusted_table("Adjusted immune-cell tests", &r.tests, 80));
    s
}

fn build_corrected_pathway_section(r: &CorrectedPathwayResult) -> ReportSection {
    let mut s = ReportSection::new("Pathways");
    s.add_content(html!{
        p { "GMT: " code { (r.gmt_path.to_string_lossy()) } }
        p { "Tested-gene universe: " (r.universe_size) "; foreground: " (r.foreground_size) "; foreground definition: BH q ≤ " (fmt(r.q_threshold)) " and |adjusted log2FC| ≥ " (fmt(r.effect_threshold)) "." }
        p { "Over-representation uses the one-sided hypergeometric upper tail and BH correction across retained gene sets. Gene sets with " (r.min_set_size) "–" (r.max_set_size) " measured genes are tested." }
        h3 { "How to read this analysis" }
        p { "Odds ratio > 1 indicates that a pathway contains more foreground genes than expected from the measured-gene universe. BH q-values control the false-discovery rate across the retained pathways. Closely related gene sets can share many genes, so clusters of significant pathways are often best interpreted as a broader biological theme." }
        p { "Plot hover labels show a compact subset of overlap genes to remain readable; the complete overlap list is preserved in the table and exported result files." }
    });
    let rows = r
        .tests
        .iter()
        .filter(|x| x.q_value.is_finite() && x.q_value > 0.0)
        .take(120)
        .collect::<Vec<_>>();
    if !rows.is_empty() {
        let x = rows.iter().map(|x| x.odds_ratio.max(1e-12).ln()).collect();
        let y = rows
            .iter()
            .map(|x| -x.q_value.max(1e-300).log10())
            .collect();
        let h=rows.iter().map(|x|format!("<b>{}</b><br>Overlap: {}/{}<br>Odds ratio: {:.3}<br>p: {:.3e}<br>BH q: {:.3e}<br>Overlap genes: {}",escape_hover_text(&x.pathway),x.overlap,x.measured_set_size,x.odds_ratio,x.p_value,x.q_value,compact_hover_gene_list(&x.overlap_genes,6,110))).collect();
        if let Some(p) = labeled_scatter_plot(
            vec![("Pathways".to_string(), x, y, h)],
            "Pathway over-representation",
            "ln odds ratio",
            "-log10(BH q)",
        ) {
            s.add_plot(p);
        }
    }
    s.add_content(pathway_table(r, 150));
    s
}

fn pathway_table(r: &CorrectedPathwayResult, limit: usize) -> Markup {
    html! {h3{"Pathway ORA results ("(r.tests.len())" rows; showing up to "(limit.min(r.tests.len()))")"}div class="table-container"{table{thead{tr{th{"Pathway"}th{"Description"}th{"Set size"}th{"Overlap"}th{"Odds ratio"}th{"p"}th{"BH q"}th{"Overlap genes"}}}tbody{@for x in r.tests.iter().take(limit){tr{td{(x.pathway)}td{(x.description)}td{(x.measured_set_size)}td{(x.overlap)}td{(fmt(x.odds_ratio))}td{(fmt_sci(x.p_value))}td{(fmt_sci(x.q_value))}td{(x.overlap_genes)}}}}}}}
}

fn adjusted_hover(r: &AdjustedFeatureResult) -> String {
    format!(
        "<b>{}</b>{}<br>Adjusted β: {:.4}<br>SE: {:.4}<br>z: {:.3}<br>p: {:.3e}<br>BH q: {:.3e}<br>Subject-collapsed effect: {:.4}<br>Subject BH q: {:.3e}<br>Case/control mean: {:.4} / {:.4}<br>n samples/subjects: {} / {}",
        escape_hover_text(&r.feature),
        if r.secondary_id.is_empty() {
            String::new()
        } else {
            format!("<br>{}", escape_hover_text(&r.secondary_id))
        },
        r.beta_group,
        r.robust_se,
        r.z_value,
        r.p_value,
        r.q_value,
        r.subject_effect,
        r.subject_q_value,
        r.mean_case,
        r.mean_control,
        r.n_samples,
        r.n_subjects
    )
}

fn adjusted_volcano_plot(tests: &[AdjustedFeatureResult], title: &str) -> Option<Plot> {
    let rows = sampled_adjusted_tests_for_report(tests);
    if rows.is_empty() {
        return None;
    }
    let x = rows.iter().map(|r| r.beta_group).collect();
    let y = rows
        .iter()
        .map(|r| -r.q_value.max(1e-300).log10())
        .collect();
    let h = rows.iter().map(|r| adjusted_hover(r)).collect();
    labeled_scatter_plot(
        vec![("Adjusted tests".to_string(), x, y, h)],
        title,
        "Adjusted group coefficient",
        "-log10(BH q)",
    )
}

fn corrected_ordination_plot(ord: &OrdinationResult) -> Option<Plot> {
    let mut traces = Vec::new();
    for group in ["Control", "Case"] {
        let pts = ord
            .points
            .iter()
            .filter(|p| p.group == group)
            .collect::<Vec<_>>();
        let x = pts.iter().map(|p| p.x).collect();
        let y = pts.iter().map(|p| p.y).collect();
        let hover = pts
            .iter()
            .map(|p| {
                format!(
                    "<b>{}</b><br>Group: {}<br>Subject: {}<br>{}: {:.4}<br>{}: {:.4}",
                    escape_hover_text(&p.sample_id),
                    p.group,
                    escape_hover_text(&p.subject),
                    ord.x_label,
                    p.x,
                    ord.y_label,
                    p.y
                )
            })
            .collect();
        traces.push((group.to_string(), x, y, hover));
    }
    let title = if ord.permanova_p.is_finite() {
        format!(
            "{} — PERMANOVA F={:.3}, p={:.3e}",
            ord.name, ord.permanova_f, ord.permanova_p
        )
    } else {
        ord.name.clone()
    };
    labeled_scatter_plot(traces, &title, &ord.x_label, &ord.y_label)
}

fn corrected_dendrogram_plot(tree: &DendrogramResult) -> Option<Plot> {
    if tree.leaves.is_empty() || tree.segments.is_empty() {
        return None;
    }
    let mut plot = Plot::new();
    let mut xs: Vec<Option<f64>> = Vec::with_capacity(tree.segments.len() * 3);
    let mut ys: Vec<Option<f64>> = Vec::with_capacity(tree.segments.len() * 3);
    for seg in &tree.segments {
        xs.push(Some(seg.x1));
        ys.push(Some(seg.y1));
        xs.push(Some(seg.x2));
        ys.push(Some(seg.y2));
        xs.push(None);
        ys.push(None);
    }
    plot.add_trace(
        Scatter::new(xs, ys)
            .mode(Mode::Lines)
            .name("Average-linkage tree")
            .hover_info(HoverInfo::Skip),
    );
    for group in ["Control", "Case"] {
        let leaves = tree
            .leaves
            .iter()
            .filter(|l| l.group == group)
            .collect::<Vec<_>>();
        if leaves.is_empty() {
            continue;
        }
        let x = vec![0.0; leaves.len()];
        let y = leaves.iter().map(|l| l.position).collect::<Vec<_>>();
        let h = leaves
            .iter()
            .map(|l| {
                format!(
                    "<b>{}</b><br>Group: {}<br>Subject: {}<br>Cluster order: {}",
                    escape_hover_text(&l.sample_id),
                    l.group,
                    escape_hover_text(&l.subject),
                    l.position as usize + 1
                )
            })
            .collect::<Vec<_>>();
        plot.add_trace(
            Scatter::new(x, y)
                .mode(Mode::Markers)
                .name(group)
                .hover_text_array(h)
                .hover_template("%{hovertext}<extra>%{fullData.name}</extra>"),
        );
    }
    plot.set_layout(
        Layout::new()
            .title(Title::with_text(&tree.name))
            .x_axis(Axis::new().title(Title::with_text(&tree.distance_label)))
            .y_axis(Axis::new().title(Title::with_text("Cluster leaf order"))),
    );
    Some(plot)
}

fn compact_hover_gene_list(value: &str, max_items: usize, max_chars: usize) -> String {
    let items = value
        .split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();
    if items.is_empty() {
        return "none".to_string();
    }
    let mut shown = Vec::new();
    let mut chars = 0usize;
    for item in items.iter().take(max_items) {
        let extra = if shown.is_empty() {
            item.len()
        } else {
            item.len() + 2
        };
        if !shown.is_empty() && chars + extra > max_chars {
            break;
        }
        shown.push(*item);
        chars += extra;
    }
    let remaining = items.len().saturating_sub(shown.len());
    let display = if remaining > 0 {
        format!("{} … (+{} more)", shown.join(", "), remaining)
    } else {
        shown.join(", ")
    };
    escape_hover_text(&display)
}

fn dtu_gene_table(tests: &[DtuGeneResult], limit: usize) -> Markup {
    html! {h4{"Gene-level DTU screen ("(tests.len())" rows; showing up to "(limit.min(tests.len()))")"} div class="table-container"{table{thead{tr{th{"NCBI Gene ID"}th{"Transcripts"}th{"Simes p"}th{"BH q"}th{"Max |CLR effect|"}}}tbody{@for r in tests.iter().take(limit){tr{td{(r.gene_id)}td{(r.n_transcripts)}td{(fmt_sci(r.p_value))}td{(fmt_sci(r.q_value))}td{(fmt(r.max_abs_clr_effect))}}}}}}}
}

fn adjusted_table(title: &str, tests: &[AdjustedFeatureResult], limit: usize) -> Markup {
    html! {h3{(title)" ("(tests.len())" rows; showing up to "(limit.min(tests.len()))")"} div class="table-container"{table{thead{tr{th{"Feature"}th{"ID/family"}th{"β group"}th{"SE"}th{"p"}th{"BH q"}th{"Subject effect"}th{"Subject BH q"}th{"n"}th{"subjects"}th{"Case mean"}th{"Control mean"}}} tbody{@for r in tests.iter().take(limit){tr{td{(r.feature)}td{(r.secondary_id)}td{(fmt(r.beta_group))}td{(fmt(r.robust_se))}td{(fmt_sci(r.p_value))}td{(fmt_sci(r.q_value))}td{(fmt(r.subject_effect))}td{(fmt_sci(r.subject_q_value))}td{(r.n_samples)}td{(r.n_subjects)}td{(fmt(r.mean_case))}td{(fmt(r.mean_control))}}}}}}}
}

pub struct FocusedReportInputs<'a> {
    pub summary: &'a FocusedCandidateSummary,
    pub outlier_qc: Option<&'a OutlierFilterSnapshot>,
}

pub fn generate_focused_candidate_html_report(
    path: &Path,
    inputs: FocusedReportInputs<'_>,
) -> Result<()> {
    let summary = inputs.summary;
    let title = format!(
        "4C focused candidate signals — {} vs {}",
        summary.case_label, summary.control_label
    );
    let mut report = Report::new(
        "4C Cohort Explorer",
        env!("CARGO_PKG_VERSION"),
        None,
        &title,
    );

    let mut overview = ReportSection::new("Overview");
    overview.add_content(html!{
        h2{"Focused candidate-signal summary"}
        p{strong{"Comparison: "}(summary.case_label)" vs "(summary.control_label)}
        p{strong{"Focused metadata: "}code{(summary.metadata_path.to_string_lossy())}}
        p{strong{"Samples: "}(summary.n_case)" case / "(summary.n_control)" control"}
        @if let Some(qc)=inputs.outlier_qc {
            p{strong{"QC-filtered focused run: "}(qc.excluded_ids.len())" reviewed sample exclusion(s) were applied before candidate prioritization (case "(qc.original_case_n)" → "(qc.filtered_case_n)", control "(qc.original_control_n)" → "(qc.filtered_control_n)")."}
        }
        p{strong{"Purpose: "}"Prioritize a small number of preliminary genes, pathways, and other molecular signals for follow-up. These are candidate signals, not validated biomarkers."}
        p{"Corrected/standard statistics are the primary evidence. If an exploratory expression screen was also run, it is used only to show whether the direction of change agrees across two analysis views of the same samples; it is not independent replication."}
        p{"The focused sample list is expected to contain one sample per individual. This removes the repeated-sampling issue from the direct comparison, although related family members can still introduce dependence that should be addressed in later validation."}
    });
    report.add_section(overview);
    if let Some(qc) = inputs.outlier_qc {
        report.add_section(build_outlier_qc_section(
            qc,
            "Sample QC and reviewed exclusions",
        ));
    }

    let mut genes = ReportSection::new("Candidate genes");
    genes.add_content(html!{
        p{"Genes are ranked primarily by corrected BH q-value, then nominal p-value and absolute adjusted log2 fold change. The shortlist requires |adjusted log2FC| ≥ "(fmt(summary.settings.min_gene_abs_log2fc))". Pathway support counts how many q≤0.10 enriched pathways contain the gene."}
        (focused_gene_table(&summary.genes))
    });
    if let Some(plot) = focused_gene_plot(&summary.genes) {
        genes.add_plot(plot);
    }
    report.add_section(genes);

    if !summary.pathways.is_empty() {
        let mut pathways = ReportSection::new("Candidate pathways");
        pathways.add_content(html!{
            p{"Pathways are ranked by BH q-value and enrichment strength. Odds ratio >1 indicates over-representation among the corrected gene foreground. Related pathways often share genes, so multiple significant sets can reflect one broader biological theme."}
            (focused_pathway_table(&summary.pathways))
        });
        if let Some(plot) = focused_pathway_plot(&summary.pathways) {
            pathways.add_plot(plot);
        }
        report.add_section(pathways);
    }

    if !summary.other.is_empty() {
        let mut other = ReportSection::new("Other candidate signals");
        other.add_content(html!{
            p{"This table collects the strongest corrected signals from BCR/TCR repertoire metrics and compositions, BCR isotypes, and immune-infiltrate scores. Effects are on each model's analysis scale and should be interpreted within the named modality/family rather than compared numerically across modalities."}
            (focused_other_table(&summary.other))
        });
        report.add_section(other);
    }

    if !summary.transcripts.is_empty() {
        let mut transcripts = ReportSection::new("Candidate transcripts");
        transcripts.add_content(html!{
            p{"Transcript-level abundance signals are shown separately from gene-level candidates. They can motivate isoform follow-up but should not be interpreted as differential transcript usage unless supported by the dedicated DTU analysis."}
            (focused_gene_table(&summary.transcripts))
        });
        report.add_section(transcripts);
    }

    let mut interpretation = ReportSection::new("Interpretation & next steps");
    interpretation.add_content(html!{
        h3{"How to present these results"}
        ul{
            li{"Use terms such as “preliminary signal”, “candidate gene”, or “candidate pathway”, rather than “validated biomarker”."}
            li{"Prioritize signals that combine a low corrected q-value, a meaningful effect size, plausible pathway context, and consistent direction across analysis views."}
            li{"A non-significant but highly ranked result can still be useful for follow-up in a small focused cohort, but its uncertainty should be stated explicitly."}
            li{"The strongest next validation is replication in an independent cohort or a pre-specified follow-up analysis, not another re-ranking of the same samples."}
        }
        h3{"Analysis notes"}
        @for note in &summary.notes{p{(note)}}
    });
    report.add_section(interpretation);

    let output = path.to_string_lossy().to_string();
    report
        .save_to_file(&output)
        .with_context(|| format!("Could not write focused HTML report to {}", path.display()))?;
    make_report_self_contained(path)?;
    Ok(())
}

fn focused_gene_plot(rows: &[FocusedGeneCandidate]) -> Option<Plot> {
    let rows = rows
        .iter()
        .filter(|r| r.q_value.is_finite() && r.q_value > 0.0 && r.adjusted_log2fc.is_finite())
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    let x = rows.iter().map(|r| r.adjusted_log2fc).collect();
    let y = rows
        .iter()
        .map(|r| -r.q_value.max(1e-300).log10())
        .collect();
    let hover=rows.iter().map(|r|{
        let concordance=match r.direction_concordant{Some(true)=>"same direction",Some(false)=>"opposite direction",None=>"not available"};
        format!("<b>{}</b><br>Gene ID: {}<br>Adjusted log2FC: {:.3}<br>p: {:.3e}<br>BH q: {:.3e}<br>Case/control mean: {:.3} / {:.3}<br>Exploratory concordance: {}<br>Pathway support: {}<br>{}",escape_hover_text(&r.gene),escape_hover_text(&r.gene_id),r.adjusted_log2fc,r.p_value,r.q_value,r.case_mean,r.control_mean,concordance,r.pathway_support_count,escape_hover_text(&r.evidence))
    }).collect();
    labeled_scatter_plot(
        vec![("Candidate genes".to_string(), x, y, hover)],
        "Focused candidate genes",
        "Adjusted log2 fold change",
        "-log10(BH q)",
    )
}

fn focused_pathway_plot(rows: &[FocusedPathwayCandidate]) -> Option<Plot> {
    let rows = rows
        .iter()
        .filter(|r| {
            r.q_value.is_finite()
                && r.q_value > 0.0
                && r.odds_ratio.is_finite()
                && r.odds_ratio > 0.0
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    let x = rows.iter().map(|r| r.odds_ratio.ln()).collect();
    let y = rows
        .iter()
        .map(|r| -r.q_value.max(1e-300).log10())
        .collect();
    let hover=rows.iter().map(|r|format!("<b>{}</b><br>Overlap: {}/{}<br>Odds ratio: {:.3}<br>p: {:.3e}<br>BH q: {:.3e}<br>Overlap genes: {}",escape_hover_text(&r.pathway),r.overlap,r.measured_set_size,r.odds_ratio,r.p_value,r.q_value,compact_hover_gene_list(&r.overlap_genes,6,110))).collect();
    labeled_scatter_plot(
        vec![("Candidate pathways".to_string(), x, y, hover)],
        "Focused candidate pathways",
        "ln odds ratio",
        "-log10(BH q)",
    )
}

fn focused_gene_table(rows: &[FocusedGeneCandidate]) -> Markup {
    html! {div class="table-container"{table{thead{tr{th{"Rank"}th{"Gene / transcript"}th{"ID"}th{"Adjusted log2FC"}th{"p"}th{"BH q"}th{"Case mean"}th{"Control mean"}th{"Exploratory direction"}th{"Pathway support"}th{"Robustness"}th{"Evidence"}}}tbody{@for r in rows{tr{td{(r.rank)}td{(r.gene)}td{(r.gene_id)}td{(fmt(r.adjusted_log2fc))}td{(fmt_sci(r.p_value))}td{(fmt_sci(r.q_value))}td{(fmt(r.case_mean))}td{(fmt(r.control_mean))}td{(match r.direction_concordant{Some(true)=>"same",Some(false)=>"opposite",None=>"—"})}td{(r.pathway_support_count) @if !r.pathway_support.is_empty(){br; small{(r.pathway_support)}}}td{@if let Some(label)=&r.robustness{(label) @if !r.robustness_flags.is_empty(){br; small{(r.robustness_flags.join("; "))}}} @else {"—"}}td{(r.evidence)}}}}}}}
}

fn focused_pathway_table(rows: &[FocusedPathwayCandidate]) -> Markup {
    html! {div class="table-container"{table{thead{tr{th{"Rank"}th{"Pathway"}th{"Description"}th{"Odds ratio"}th{"Overlap"}th{"Set size"}th{"p"}th{"BH q"}th{"Evidence"}}}tbody{@for r in rows{tr{td{(r.rank)}td{(r.pathway)}td{(r.description)}td{(fmt(r.odds_ratio))}td{(r.overlap)}td{(r.measured_set_size)}td{(fmt_sci(r.p_value))}td{(fmt_sci(r.q_value))}td{(r.evidence)}}}}}}}
}

fn focused_other_table(rows: &[FocusedOtherCandidate]) -> Markup {
    html! {div class="table-container"{table{thead{tr{th{"Rank"}th{"Modality"}th{"Family"}th{"Feature"}th{"ID"}th{"Effect"}th{"p"}th{"BH q"}th{"Case mean"}th{"Control mean"}th{"Evidence"}}}tbody{@for r in rows{tr{td{(r.rank)}td{(r.modality)}td{(r.family)}td{(r.feature)}td{(r.secondary_id)}td{(fmt(r.effect))}td{(fmt_sci(r.p_value))}td{(fmt_sci(r.q_value))}td{(fmt(r.case_mean))}td{(fmt(r.control_mean))}td{(r.evidence)}}}}}}}
}

/// Render the technical/audit report for one completed unified analysis run.
/// This consumes the frozen run object and never reruns statistics.
pub fn generate_unified_detailed_html_report(
    path: &Path,
    run: &UnifiedCohortAnalysisRun,
) -> Result<()> {
    let title = format!(
        "4C cohort analysis — {} — full report",
        run.primary.comparison.name
    );
    let mut report = Report::new(
        "4C Cohort Explorer",
        env!("CARGO_PKG_VERSION"),
        None,
        &title,
    );
    let subject = run
        .settings
        .corrected
        .subject_column
        .as_deref()
        .unwrap_or("none");
    let covariates = if run.settings.corrected.covariates.is_empty() {
        "none".to_string()
    } else {
        run.settings.corrected.covariates.join(", ")
    };
    let comparison_definition = match &run.cohort.comparison_definition {
        CohortComparisonDefinition::Preset(preset) => {
            format!("Built-in preset: {}", preset.display_name())
        }
        CohortComparisonDefinition::MetadataValues(spec) => format!(
            "Custom metadata comparison using '{}' (case values: {}; control values: {})",
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
    let mut overview = ReportSection::new("Run overview and provenance");
    overview.add_content(html! {
        h2 { "Unified Cohort Analysis" }
        p { strong { "Comparison: " } (run.primary.comparison.case_label) " vs " (run.primary.comparison.control_label) }
        p { strong { "Comparison definition: " } (comparison_definition) }
        p { strong { "Completed analysis samples: " } (run.case_n()) " case / " (run.control_n()) " control" }
        p { strong { "Project root: " } code { (run.cohort.project_root.to_string_lossy()) } }
        p { strong { "Metadata: " } code { (run.cohort.metadata_path.to_string_lossy()) } " (ID column: " (run.cohort.metadata_id_column) ")" }
        @if run.cohort.uses_filtered_metadata_subset {
            p { strong { "Cohort definition: " } "A collaborator/user-supplied filtered metadata table defined the analyzed subset. The statistical methodology is unchanged." }
        } @else {
            p { strong { "Cohort definition: " } "Primary project metadata." }
        }
        p { strong { "Subject / repeated-measure column: " } (subject) }
        p { strong { "Covariates: " } (covariates) }
        @if let Some(qc)=run.qc_snapshot.as_ref() {
            p { strong { "Reviewed QC exclusions: " } (qc.excluded_ids.len()) " applied exclusion(s); case " (qc.original_case_n) " → " (qc.filtered_case_n) ", control " (qc.original_control_n) " → " (qc.filtered_control_n) "." }
        } @else {
            p { strong { "Reviewed QC exclusions: " } "none applied to this run." }
        }
        p { "Corrected/standard results are the primary inferential evidence. Exploratory expression calculations, when enabled, are retained only as descriptive direction-concordance support. Candidate prioritization and both report styles are derived from this completed run." }
    });
    report.add_section(overview);

    let selected_modalities = [
        (run.settings.modalities.gene, "Gene expression"),
        (run.settings.modalities.transcript, "Transcript expression"),
        (run.settings.modalities.bcr, "BCR"),
        (run.settings.modalities.bcr_isotype, "BCR isotype"),
        (run.settings.modalities.tcr_alpha, "TCR alpha"),
        (run.settings.modalities.tcr_beta, "TCR beta"),
        (run.settings.modalities.immune, "Immune infiltrate"),
        (run.settings.modalities.pathway, "Pathway ORA"),
    ]
    .into_iter()
    .filter_map(|(selected, label)| selected.then_some(label))
    .collect::<Vec<_>>()
    .join(", ");
    let pathway_source = run
        .settings
        .pathway_gmt_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "not selected".to_string());
    let mut configuration = ReportSection::new("Analysis configuration");
    configuration.add_content(html! {
        ul {
            li { strong { "Selected modalities: " } (selected_modalities) }
            li { strong { "Expression filtering: " } "minimum count " (fmt(run.settings.corrected.min_count)) "; minimum sample fraction " (fmt(run.settings.corrected.min_fraction_samples)) }
            li { strong { "Subject-blocked permutation count: " } (run.settings.corrected.permutations) }
            li { strong { "Repertoire rarefaction: " } (run.settings.corrected.rarefaction_reps) " replicate(s); depth cap " (run.settings.corrected.rarefaction_cap) }
            li { strong { "Ambiguous repertoire-call policy: " } (run.settings.corrected.ambiguous_policy.display_name()) }
            li { strong { "BCR-isotype Undetermined sensitivity policy: " } (if run.settings.corrected.exclude_undetermined_isotype { "exclude and renormalize" } else { "retain" }) }
            li { strong { "Exploratory TPM direction support: " } (if run.settings.exploratory_direction_check { "enabled" } else { "disabled" }) }
            li { strong { "PBMC / confounding diagnostics: " } (if run.settings.pbmc_diagnostics.is_some() { "enabled" } else { "disabled" }) }
            @if run.settings.modalities.pathway {
                li { strong { "Pathway GMT: " } code { (pathway_source) } }
                li { strong { "Pathway foreground thresholds: " } "gene BH q ≤ " (fmt(run.settings.pathway_q_threshold)) "; |adjusted effect| ≥ " (fmt(run.settings.pathway_effect_threshold)) }
            }
        }
    });
    report.add_section(configuration);

    report.add_section(build_mapping_section(&run.cohort.mapping));
    if let Some(qc) = run.qc_snapshot.as_ref() {
        report.add_section(build_outlier_qc_section(
            qc,
            "Sample QC and reviewed exclusions",
        ));
    }

    let primary = &run.primary;
    if let Some(result) = primary.gene.as_ref() {
        report.add_section(build_corrected_expression_section("Gene counts", result));
    } else if let Some(error) = primary.gene_error.as_deref() {
        report.add_section(corrected_error_section("Gene counts", error));
    }
    if let Some(result) = primary.transcript.as_ref() {
        report.add_section(build_corrected_expression_section(
            "Transcript counts",
            result,
        ));
    } else if let Some(error) = primary.transcript_error.as_deref() {
        report.add_section(corrected_error_section("Transcript counts", error));
    }
    if let Some(result) = primary.bcr.as_ref() {
        report.add_section(build_corrected_repertoire_section("BCR", result));
    } else if let Some(error) = primary.bcr_error.as_deref() {
        report.add_section(corrected_error_section("BCR", error));
    }
    if let Some(result) = primary.bcr_isotype.as_ref() {
        report.add_section(build_corrected_isotype_section(result));
    } else if let Some(error) = primary.bcr_isotype_error.as_deref() {
        report.add_section(corrected_error_section("BCR isotype", error));
    }
    if let Some(result) = primary.tcr_alpha.as_ref() {
        report.add_section(build_corrected_repertoire_section("TCR alpha", result));
    } else if let Some(error) = primary.tcr_alpha_error.as_deref() {
        report.add_section(corrected_error_section("TCR alpha", error));
    }
    if let Some(result) = primary.tcr_beta.as_ref() {
        report.add_section(build_corrected_repertoire_section("TCR beta", result));
    } else if let Some(error) = primary.tcr_beta_error.as_deref() {
        report.add_section(corrected_error_section("TCR beta", error));
    }
    if let Some(result) = primary.immune.as_ref() {
        report.add_section(build_corrected_immune_section(result));
    } else if let Some(error) = primary.immune_error.as_deref() {
        report.add_section(corrected_error_section("Immune infiltrate", error));
    }
    if let Some(result) = primary.pathway.as_ref() {
        report.add_section(build_corrected_pathway_section(result));
    } else if let Some(error) = primary.pathway_error.as_deref() {
        report.add_section(corrected_error_section("Pathways", error));
    }

    if run.settings.exploratory_direction_check {
        if run.exploratory_support.gene.is_some() || run.exploratory_support.gene_error.is_some() {
            report.add_section(build_expression_section(
                "Supporting descriptive gene TPM check",
                "Gene",
                "genes",
                run.exploratory_support.gene.as_ref(),
                run.exploratory_support.gene_error.as_deref(),
            ));
        }
        if run.exploratory_support.transcript.is_some()
            || run.exploratory_support.transcript_error.is_some()
        {
            report.add_section(build_expression_section(
                "Supporting descriptive transcript TPM check",
                "Transcript",
                "transcripts",
                run.exploratory_support.transcript.as_ref(),
                run.exploratory_support.transcript_error.as_deref(),
            ));
        }
    }

    if let Some(diagnostics) = run.diagnostics.as_ref() {
        report.add_section(build_unified_diagnostics_overview(diagnostics));
        if let Some(result) = diagnostics.qc_restricted_gene.as_ref() {
            report.add_section(build_corrected_expression_section(
                "QC-restricted gene sensitivity analysis",
                result,
            ));
        }
        if let Some(result) = diagnostics.immune_adjusted_gene.as_ref() {
            report.add_section(build_corrected_expression_section(
                "Immune-composition-adjusted gene sensitivity analysis",
                result,
            ));
        }
        report.add_section(build_unified_candidate_robustness(diagnostics));
    } else if let Some(error) = run.diagnostics_error.as_deref() {
        report.add_section(corrected_error_section(
            "PBMC / confounding sensitivity diagnostics",
            error,
        ));
    }

    let mut candidate = ReportSection::new("Candidate prioritization");
    candidate.add_content(html! {
        p { "This section is a presentation/prioritization layer derived from the corrected results above. It is not an independent analysis and does not add statistical evidence." }
        h3 { "Candidate genes" }
        (focused_gene_table(&run.candidates.genes))
        @if !run.candidates.pathways.is_empty() {
            h3 { "Candidate pathways" }
            (focused_pathway_table(&run.candidates.pathways))
        }
        @if !run.candidates.other.is_empty() {
            h3 { "Other modality signals" }
            (focused_other_table(&run.candidates.other))
        }
    });
    report.add_section(candidate);

    report.add_section(build_unified_analysis_documentation(run));

    let mut limitations = ReportSection::new("Warnings, assumptions, and limitations");
    limitations.add_content(html! {
        p { "4C Cohort Explorer supports discovery, robustness assessment, and candidate prioritization. Results are preliminary and must not be described as validated clinical biomarkers without independent replication/validation." }
        p { "The native expression model does not reproduce the full empirical-Bayes/dispersion-shrinkage machinery of DESeq2, edgeR, or limma-voom. Interpret model-specific effects and uncertainty accordingly." }
        @if run.warnings.is_empty() {
            p { "No additional run warnings were recorded." }
        } @else {
            h3 { "Run warnings" }
            ul { @for warning in &run.warnings { li { (warning) } } }
        }
    });
    report.add_section(limitations);

    let output = path.to_string_lossy().to_string();
    report.save_to_file(&output).with_context(|| {
        format!(
            "Could not write unified detailed HTML report to {}",
            path.display()
        )
    })?;
    make_report_self_contained(path)?;
    Ok(())
}

/// Render the presentation-friendly report from the same frozen run used by
/// the detailed report. No statistics are recalculated here.
pub fn generate_unified_simplified_html_report(
    path: &Path,
    run: &UnifiedCohortAnalysisRun,
) -> Result<()> {
    let title = format!(
        "4C key findings — {} vs {}",
        run.primary.comparison.case_label, run.primary.comparison.control_label
    );
    let mut report = Report::new(
        "4C Cohort Explorer",
        env!("CARGO_PKG_VERSION"),
        None,
        &title,
    );

    let top_gene = run.candidates.genes.first();
    let mut overview = ReportSection::new("Comparison and main takeaway");
    overview.add_content(html! {
        h2 { "Key findings" }
        p { strong { "Comparison: " } (run.primary.comparison.case_label) " vs " (run.primary.comparison.control_label) }
        p { strong { "Analyzed samples: " } (run.case_n()) " case / " (run.control_n()) " control" }
        @if let Some(qc)=run.qc_snapshot.as_ref() {
            p { strong { "QC review: " } (qc.audit.consensus_ids.len()) " sample(s) met the frozen cross-modal consensus flagging rule; " (qc.excluded_ids.len()) " reviewed exclusion(s) were applied to the completed run. Case " (qc.original_case_n) " → " (qc.filtered_case_n) ", control " (qc.original_control_n) " → " (qc.filtered_control_n) "." }
        } @else {
            p { strong { "QC review: " } "no frozen cross-modal outlier audit was attached to this run, and no reviewed sample exclusions were applied." }
        }
        @if let Some(gene)=top_gene {
            p { strong { "Main takeaway: " } "The strongest ranked gene-level candidate is " (gene.gene) " (adjusted log2FC " (fmt(gene.adjusted_log2fc)) ", BH q " (fmt_sci(gene.q_value)) "). This is a preliminary candidate signal, not a validated biomarker." }
        } @else {
            p { strong { "Main takeaway: " } "No gene met the configured candidate-shortlist criteria. The completed analysis should be interpreted through the modality-specific results and limitations rather than forcing a biomarker claim." }
        }
    });
    report.add_section(overview);

    let mut genes = ReportSection::new("Strongest candidate genes");
    let gene_limit = run.candidates.genes.len().min(10);
    genes.add_content(html! {
        p { "Candidates are ranked from the primary corrected analysis. Robustness labels, when available, come from the run's PBMC/confounding sensitivity diagnostics." }
        (focused_gene_table(&run.candidates.genes[..gene_limit]))
    });
    if let Some(plot) = focused_gene_plot(&run.candidates.genes[..gene_limit]) {
        genes.add_plot(plot);
    }
    report.add_section(genes);

    if !run.candidates.pathways.is_empty() {
        let mut pathways = ReportSection::new("Key pathways and biological themes");
        let limit = run.candidates.pathways.len().min(8);
        pathways.add_content(focused_pathway_table(&run.candidates.pathways[..limit]));
        if let Some(plot) = focused_pathway_plot(&run.candidates.pathways[..limit]) {
            pathways.add_plot(plot);
        }
        report.add_section(pathways);
    }

    if !run.candidates.other.is_empty() {
        let mut other = ReportSection::new("Key multi-modal findings");
        let limit = run.candidates.other.len().min(10);
        other.add_content(html! {
            p { "Only the highest-ranked corrected signals from the selected non-expression modalities are shown here." }
            (focused_other_table(&run.candidates.other[..limit]))
        });
        report.add_section(other);
    }

    if let Some(pca) = run.primary.gene.as_ref().and_then(|gene| gene.pca.as_ref()) {
        let mut structure = ReportSection::new("Cohort structure");
        structure.add_content(html! {
            p { "This unsupervised PCA is included as context for sample structure; it is not a separate source of inferential evidence." }
        });
        if let Some(plot) = corrected_ordination_plot(pca) {
            structure.add_plot(plot);
        }
        report.add_section(structure);
    }

    report.add_section(build_unified_analysis_documentation(run));

    let mut limitations = ReportSection::new("Confidence and limitations");
    limitations.add_content(html! {
        ul {
            li { "These findings are preliminary candidates for follow-up, not validated biomarkers." }
            li { "Interpret the effect estimates together with sample support, repeated/family/batch structure, and QC sensitivity." }
            li { "Related pathways can share many genes and should be interpreted as broader themes rather than independent discoveries." }
            li { "Independent replication or a pre-specified follow-up cohort is the strongest next validation step." }
        }
        @if let Some(qc)=run.qc_snapshot.as_ref() {
            p { strong { "Reviewed exclusions used by this run: " } @if qc.excluded_ids.is_empty() { "none" } @else { (qc.excluded_ids.join(", ")) } }
        }
        @if !run.warnings.is_empty() {
            p { strong { "Run note: " } (run.warnings.len()) " warning(s) were recorded. See the full detailed report for the complete technical audit." }
        }
    });
    report.add_section(limitations);

    let output = path.to_string_lossy().to_string();
    report.save_to_file(&output).with_context(|| {
        format!(
            "Could not write unified simplified HTML report to {}",
            path.display()
        )
    })?;
    make_report_self_contained(path)?;
    Ok(())
}

fn build_unified_analysis_documentation(run: &UnifiedCohortAnalysisRun) -> ReportSection {
    let mut section = ReportSection::new("Analysis guide: what the methods and values mean");
    let subject = run
        .settings
        .corrected
        .subject_column
        .as_deref()
        .unwrap_or("none selected");
    let covariates = if run.settings.corrected.covariates.is_empty() {
        "none selected".to_string()
    } else {
        run.settings.corrected.covariates.join(", ")
    };
    let qc_description = run.qc_snapshot.as_ref().map(|qc| {
        let cfg = &qc.audit.config;
        let rule = if cfg.consensus_rule == OutlierConsensusRule::AtLeastN {
            format!("{} (N={})", cfg.consensus_rule.display_name(), cfg.at_least_n)
        } else {
            cfg.consensus_rule.display_name().to_string()
        };
        format!(
            "scope {}; robust-distance threshold {:.3}; consensus {}; minimum available modalities {}; reviewed exclusions {}; applied to primary analysis {}",
            cfg.scope.display_name(),
            cfg.robust_distance_threshold,
            rule,
            cfg.min_available_modalities,
            qc.excluded_ids.len(),
            if qc.applied_to_analysis { "yes" } else { "no" }
        )
    });
    let diagnostics_description = run.settings.pbmc_diagnostics.as_ref().map(|cfg| {
        format!(
            "enabled; top variable PCA genes {}; robust outlier threshold {:.3}; low-count threshold {:.3}; minimum supported fraction {:.3}; top-two contribution warning threshold {:.3}; top candidates {}",
            cfg.pca_features,
            cfg.outlier_z_threshold,
            cfg.low_count_threshold,
            cfg.low_count_min_fraction,
            cfg.top2_contribution_threshold,
            cfg.top_candidates
        )
    });
    section.add_content(html! {
        p {
            "This appendix is intentionally written for readers who are not expected to know the statistical terminology in advance. "
            "The GUI Docs workspace and both unified HTML report styles use the same canonical explanations and toy examples below. "
            "The desktop guide has a built-in search box; in this HTML report, use the browser's Find command and open the relevant expandable section. "
            "The first block records the values used for this completed run; the expandable sections then explain what those values and outputs mean with small illustrative plots."
        }
        h3 { "Settings used in this completed run" }
        ul {
            li { strong { "Subject / repeated-measure column: " } (subject) }
            li { strong { "Covariates: " } (covariates) }
            li { strong { "Expression support filter: " } "normalized count >= " (fmt(run.settings.corrected.min_count)) " in at least " (format!("{:.1}%", 100.0 * run.settings.corrected.min_fraction_samples)) " of modeled samples (with the implementation also requiring at least 3 samples)." }
            li { strong { "Repertoire composition filter: " } "mean fraction >= " (fmt(run.settings.corrected.composition_min_mean)) "; prevalence >= " (format!("{:.1}%", 100.0 * run.settings.corrected.composition_min_prevalence)) "; CLR pseudocount " (fmt_sci(run.settings.corrected.clr_pseudocount)) "." }
            li { strong { "PERMANOVA permutations: " } (run.settings.corrected.permutations) }
            li { strong { "Repertoire rarefaction: " } (run.settings.corrected.rarefaction_reps) " replicate(s), depth cap " (run.settings.corrected.rarefaction_cap) "." }
            li { strong { "Ambiguous V/D/J call policy: " } (run.settings.corrected.ambiguous_policy.display_name()) }
            li { strong { "Dedicated BCR-isotype Undetermined handling: " } (if run.settings.corrected.exclude_undetermined_isotype { "exclude and renormalize" } else { "retain in composition" }) }
            li { strong { "Candidate display limits: " } (run.settings.candidate.top_genes) " genes, " (run.settings.candidate.top_transcripts) " transcripts, " (run.settings.candidate.top_pathways) " pathways, " (run.settings.candidate.top_other) " other signals; minimum candidate gene |log2FC| " (fmt(run.settings.candidate.min_gene_abs_log2fc)) "." }
            @if run.settings.modalities.pathway {
                li { strong { "Pathway foreground: " } "gene BH q <= " (fmt(run.settings.pathway_q_threshold)) " and |adjusted log2FC| >= " (fmt(run.settings.pathway_effect_threshold)) "." }
            }
            li { strong { "Exploratory TPM direction annotation: " } (if run.settings.exploratory_direction_check { "enabled" } else { "disabled" }) }
            @if let Some(text) = qc_description.as_ref() {
                li { strong { "Cross-modal QC snapshot: " } (text) }
            } @else {
                li { strong { "Cross-modal QC snapshot: " } "none attached to this completed run." }
            }
            @if let Some(text) = diagnostics_description.as_ref() {
                li { strong { "PBMC/source/composition diagnostics: " } (text) }
            } @else {
                li { strong { "PBMC/source/composition diagnostics: " } "disabled for this run." }
            }
        }
        p {
            em { "Reading tip: " }
            "A displayed Case mean and Control mean are descriptive summaries. The fitted group coefficient is the model-based Case-versus-Control effect after selected covariates, and it can therefore differ from the raw difference between those means."
        }
        @for doc in analysis_doc_sections() {
            details {
                summary { strong { (doc.title) } " — " (doc.summary) }
                p { em { (doc.summary) } }
                @for example in doc.examples {
                    (build_analysis_doc_toy_example_markup(example))
                }
                @for subsection in doc.subsections {
                    h4 { (subsection.title) }
                    @for paragraph in subsection.paragraphs {
                        p { (paragraph) }
                    }
                    @if !subsection.bullets.is_empty() {
                        ul {
                            @for bullet in subsection.bullets {
                                li { (bullet) }
                            }
                        }
                    }
                }
            }
        }
    });
    section
}

fn build_analysis_doc_toy_example_markup(example: &AnalysisDocToyExample) -> Markup {
    html! {
        div style="border:1px solid #d5dae0;border-radius:8px;padding:12px 14px;margin:12px 0 16px;background:#fafbfc;" {
            h4 style="margin:0 0 4px;" { "Toy example — " (example.title) }
            p style="margin:0 0 8px;color:#59636e;font-size:0.92em;" { strong { "Illustrative values only." } " This is not a result from the current cohort." }
            p { (example.setup) }
            (build_analysis_doc_toy_chart_markup(example))
            ol {
                @for step in example.walkthrough {
                    li { (step) }
                }
            }
            p { strong { "Takeaway: " } (example.takeaway) }
        }
    }
}

fn build_analysis_doc_toy_chart_markup(example: &AnalysisDocToyExample) -> Markup {
    let svg = match example.chart {
        AnalysisDocToyChart::Bars {
            categories,
            series,
            y_label,
            stacked,
        } => analysis_doc_bar_svg(categories, series, y_label, stacked),
        AnalysisDocToyChart::Line {
            categories,
            series,
            y_label,
        } => analysis_doc_line_svg(categories, series, y_label),
        AnalysisDocToyChart::Scatter {
            points,
            x_label,
            y_label,
        } => analysis_doc_scatter_svg(points, x_label, y_label),
    };
    html! {
        div style="margin:10px 0 12px;overflow-x:auto;" {
            (PreEscaped(svg))
        }
    }
}

fn analysis_doc_svg_shell(body: &str, aria_label: &str, legend: &str) -> String {
    format!(
        r#"<svg viewBox="0 0 720 285" role="img" aria-label="{}" style="width:100%;max-width:760px;min-width:520px;height:auto;background:white;border:1px solid #e1e5e9;border-radius:6px;">{}</svg><div style="font-size:0.86em;color:#59636e;margin-top:4px;">{}</div>"#,
        aria_label, body, legend
    )
}

fn analysis_doc_bar_svg(
    categories: &[&str],
    series: &[crate::analysis_docs::AnalysisDocToySeries],
    y_label: &str,
    stacked: bool,
) -> String {
    if categories.is_empty() || series.is_empty() {
        return String::new();
    }
    let max_y = if stacked {
        (0..categories.len())
            .map(|idx| {
                series
                    .iter()
                    .map(|s| s.values.get(idx).copied().unwrap_or(0.0).max(0.0))
                    .sum::<f64>()
            })
            .fold(0.0, f64::max)
            .max(1.0)
    } else {
        series
            .iter()
            .flat_map(|s| s.values.iter().copied())
            .fold(0.0, f64::max)
            .max(1.0)
    };
    let colors = [
        "#4c78a8", "#f58518", "#54a24b", "#e45756", "#b279a2", "#72b7b2",
    ];
    let plot_left = 62.0;
    let plot_top = 22.0;
    let plot_height = 196.0;
    let plot_width = 618.0;
    let category_width = plot_width / categories.len() as f64;
    let mut body = format!(
        r##"<line x1="62" y1="218" x2="680" y2="218" stroke="#88929c" stroke-width="1"/><line x1="62" y1="22" x2="62" y2="218" stroke="#88929c" stroke-width="1"/>"##
    );
    for (cat_idx, category) in categories.iter().enumerate() {
        let center = plot_left + (cat_idx as f64 + 0.5) * category_width;
        if stacked {
            let mut base = 0.0;
            for (series_idx, toy_series) in series.iter().enumerate() {
                let value = toy_series
                    .values
                    .get(cat_idx)
                    .copied()
                    .unwrap_or(0.0)
                    .max(0.0);
                let h = value / max_y * plot_height;
                let y = plot_top + plot_height - (base + value) / max_y * plot_height;
                body.push_str(&format!(
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" opacity="0.9"/>"#,
                    center - category_width * 0.28,
                    y,
                    category_width * 0.56,
                    h,
                    colors[series_idx % colors.len()]
                ));
                base += value;
            }
        } else {
            let n = series.len() as f64;
            let bar_width = (category_width * 0.68 / n).max(4.0);
            for (series_idx, toy_series) in series.iter().enumerate() {
                let value = toy_series
                    .values
                    .get(cat_idx)
                    .copied()
                    .unwrap_or(0.0)
                    .max(0.0);
                let h = value / max_y * plot_height;
                let x = center - (n * bar_width) / 2.0 + series_idx as f64 * bar_width;
                let y = plot_top + plot_height - h;
                body.push_str(&format!(
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" opacity="0.9"/>"#,
                    x,
                    y,
                    bar_width * 0.88,
                    h,
                    colors[series_idx % colors.len()]
                ));
            }
        }
        body.push_str(&format!(
            r##"<text x="{:.2}" y="242" text-anchor="middle" font-size="11" fill="#39434d">{}</text>"##,
            center, category
        ));
    }
    body.push_str(&format!(
        r##"<text x="16" y="122" transform="rotate(-90 16 122)" text-anchor="middle" font-size="12" fill="#39434d">{}</text>"##,
        y_label
    ));
    let legend = series
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            format!(
                r#"<span style="display:inline-block;margin-right:14px;color:{};">■ {}</span>"#,
                colors[idx % colors.len()],
                s.label
            )
        })
        .collect::<String>();
    analysis_doc_svg_shell(&body, &format!("Toy bar plot: {y_label}"), &legend)
}

fn analysis_doc_line_svg(
    categories: &[&str],
    series: &[crate::analysis_docs::AnalysisDocToySeries],
    y_label: &str,
) -> String {
    if categories.is_empty() || series.is_empty() {
        return String::new();
    }
    let values = series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .collect::<Vec<_>>();
    let min_y = values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
        .min(0.0);
    let max_y = values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(1.0);
    let span = (max_y - min_y).abs().max(1.0);
    let colors = ["#4c78a8", "#f58518", "#54a24b", "#e45756"];
    let mut body = String::from(
        r##"<line x1="62" y1="220" x2="680" y2="220" stroke="#88929c" stroke-width="1"/><line x1="62" y1="24" x2="62" y2="220" stroke="#88929c" stroke-width="1"/>"##,
    );
    for (series_idx, toy_series) in series.iter().enumerate() {
        let coords = toy_series
            .values
            .iter()
            .enumerate()
            .map(|(idx, value)| {
                let x = if categories.len() <= 1 {
                    371.0
                } else {
                    62.0 + idx as f64 / (categories.len() - 1) as f64 * 618.0
                };
                let y = 220.0 - ((*value - min_y) / span) * 196.0;
                (x, y)
            })
            .collect::<Vec<_>>();
        let points = coords
            .iter()
            .map(|(x, y)| format!("{x:.2},{y:.2}"))
            .collect::<Vec<_>>()
            .join(" ");
        body.push_str(&format!(
            r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="3"/>"#,
            points,
            colors[series_idx % colors.len()]
        ));
        for (x, y) in coords {
            body.push_str(&format!(
                r#"<circle cx="{x:.2}" cy="{y:.2}" r="4.5" fill="{}"/>"#,
                colors[series_idx % colors.len()]
            ));
        }
    }
    for (idx, category) in categories.iter().enumerate() {
        let x = if categories.len() <= 1 {
            371.0
        } else {
            62.0 + idx as f64 / (categories.len() - 1) as f64 * 618.0
        };
        body.push_str(&format!(
            r##"<text x="{x:.2}" y="242" text-anchor="middle" font-size="11" fill="#39434d">{}</text>"##,
            category
        ));
    }
    body.push_str(&format!(
        r##"<text x="16" y="122" transform="rotate(-90 16 122)" text-anchor="middle" font-size="12" fill="#39434d">{}</text>"##,
        y_label
    ));
    let legend = series
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            format!(
                r#"<span style="display:inline-block;margin-right:14px;color:{};">● {}</span>"#,
                colors[idx % colors.len()],
                s.label
            )
        })
        .collect::<String>();
    analysis_doc_svg_shell(&body, &format!("Toy line plot: {y_label}"), &legend)
}

fn analysis_doc_scatter_svg(
    points: &[crate::analysis_docs::AnalysisDocToyPoint],
    x_label: &str,
    y_label: &str,
) -> String {
    if points.is_empty() {
        return String::new();
    }
    let min_x = points.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_x = points.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_y = points.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let x_span = (max_x - min_x).abs().max(1.0);
    let y_span = (max_y - min_y).abs().max(1.0);
    let colors = ["#4c78a8", "#f58518", "#54a24b", "#e45756", "#b279a2"];
    let mut groups = BTreeMap::<&str, usize>::new();
    for point in points {
        if !groups.contains_key(point.group) {
            let idx = groups.len();
            groups.insert(point.group, idx);
        }
    }
    let mut body = String::from(
        r##"<line x1="62" y1="220" x2="680" y2="220" stroke="#88929c" stroke-width="1"/><line x1="62" y1="24" x2="62" y2="220" stroke="#88929c" stroke-width="1"/>"##,
    );
    for point in points {
        let px = 62.0 + ((point.x - min_x) / x_span) * 618.0;
        let py = 220.0 - ((point.y - min_y) / y_span) * 196.0;
        let color = colors[*groups.get(point.group).unwrap_or(&0) % colors.len()];
        body.push_str(&format!(
            r##"<circle cx="{px:.2}" cy="{py:.2}" r="5" fill="{color}" opacity="0.9"/><text x="{:.2}" y="{:.2}" font-size="11" fill="#39434d">{}</text>"##,
            px + 7.0,
            py - 6.0,
            point.label
        ));
    }
    body.push_str(&format!(
        r##"<text x="371" y="255" text-anchor="middle" font-size="12" fill="#39434d">{}</text><text x="16" y="122" transform="rotate(-90 16 122)" text-anchor="middle" font-size="12" fill="#39434d">{}</text>"##,
        x_label, y_label
    ));
    let legend = groups
        .iter()
        .map(|(group, idx)| {
            format!(
                r#"<span style="display:inline-block;margin-right:14px;color:{};">● {}</span>"#,
                colors[*idx % colors.len()],
                group
            )
        })
        .collect::<String>();
    analysis_doc_svg_shell(
        &body,
        &format!("Toy scatter plot: {x_label} versus {y_label}"),
        &legend,
    )
}

fn build_unified_diagnostics_overview(result: &PbmcDiagnosticsResult) -> ReportSection {
    let mut section = ReportSection::new("PBMC / confounding diagnostics");
    section.add_content(html! {
        p { "These diagnostics are sensitivity/QC evidence attached to the same completed analysis, not a competing disease model." }
        p { strong { "Samples with QC sensitivity exclusions: " } (result.sensitivity_excluded_ids.len()) }
        @if !result.sensitivity_excluded_ids.is_empty() {
            p { (result.sensitivity_excluded_ids.join(", ")) }
        }
        h3 { "Sample QC summary" }
        div class="table-container" { table {
            thead { tr { th { "Sample" } th { "Group" } th { "Library raw count" } th { "Detected genes" } th { "Transcriptome outlier" } th { "Immune outlier" } th { "Myeloid/lymphoid score" } th { "Flags" } } }
            tbody { @for row in &result.sample_qc { tr {
                td { (row.sample_id) } td { (row.comparison_group) } td { (fmt(row.library_raw_count)) } td { (row.detected_genes) }
                td { (if row.auto_expression_outlier { "yes" } else { "no" }) }
                td { (if row.auto_immune_outlier { "yes" } else { "no" }) }
                td { (fmt(row.monocyte_lymphoid_signature)) } td { (row.flags.join("; ")) }
            } } }
        } }
        @if !result.longitudinal_qc.is_empty() {
            h3 { "Within-subject longitudinal structure" }
            div class="table-container" { table {
                thead { tr { th { "Subject" } th { "n" } th { "Max PCA distance" } th { "Samples" } th { "Batches" } th { "Flagged samples" } } }
                tbody { @for row in &result.longitudinal_qc { tr { td { (row.subject) } td { (row.n_samples) } td { (fmt(row.max_pca_distance)) } td { (row.samples) } td { (row.batches) } td { (row.flagged_samples) } } } }
            } }
        }
    });
    if let Some(pca) = result.pca.as_ref() {
        if let Some(plot) = corrected_ordination_plot(pca) {
            section.add_plot(plot);
        }
    }
    section
}

fn build_unified_candidate_robustness(result: &PbmcDiagnosticsResult) -> ReportSection {
    let mut section = ReportSection::new("Candidate robustness diagnostics");
    section.add_content(html! {
        p { "Candidate labels summarize low-count support, one/few-sample dominance, leave-one-sample/leave-level-out stability, QC-restricted sensitivity, immune-composition adjustment, and suspect-vs-ordinary source effects where available." }
        div class="table-container" { table {
            thead { tr { th { "Gene" } th { "Baseline log2FC" } th { "Baseline q" } th { "QC-restricted log2FC" } th { "QC q" } th { "Immune-adjusted log2FC" } th { "Immune q" } th { "LOO range" } th { "Classification" } th { "Flags" } } }
            tbody { @for row in &result.candidate_stability { tr {
                td { (row.gene) } td { (fmt(row.baseline_log2fc)) } td { (fmt_sci(row.baseline_q)) }
                td { (fmt(row.qc_restricted_log2fc)) } td { (fmt_sci(row.qc_restricted_q)) }
                td { (fmt(row.immune_adjusted_log2fc)) } td { (fmt_sci(row.immune_adjusted_q)) }
                td { (fmt(row.loo_log2fc_min)) " … " (fmt(row.loo_log2fc_max)) }
                td { (row.classification) } td { (row.flags.join("; ")) }
            } } }
        } }
    });
    section
}
