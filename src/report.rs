use crate::cohort::{
    BcrComparisonResult, BcrIsotypeComparisonResult, BcrSampleSummary, CohortComparison,
    CohortMapping, CohortModality, FeatureTestResult, GeneComparisonResult, ImmuneComparisonResult,
    MappingStatus, TcrComparisonResult, TcrSampleSummary,
};
use anyhow::{Context, Result};
use maud::{html, Markup};
use report_builder::plots::plot_boxplot;
use report_builder::{Report, ReportSection};
use plotly::common::{Mode, Title};
use plotly::layout::Axis;
use plotly::{Layout, Plot, Scatter};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
}

pub fn generate_cohort_html_report(path: &Path, inputs: CohortReportInputs<'_>) -> Result<()> {
    let title = format!("4C cohort analysis — {}", inputs.comparison.name);
    let mut report = Report::new("4C Cohort Explorer", env!("CARGO_PKG_VERSION"), None, &title);

    report.add_section(build_overview_section(&inputs));
    if let Some(mapping) = inputs.mapping {
        report.add_section(build_mapping_section(mapping));
    }
    if inputs.gene.is_some() || inputs.gene_error.is_some() { report.add_section(build_gene_section(&inputs)); }
    if inputs.transcript.is_some() || inputs.transcript_error.is_some() { report.add_section(build_transcript_section(&inputs)); }
    if inputs.bcr.is_some() || inputs.bcr_error.is_some() { report.add_section(build_bcr_section(&inputs)); }
    if inputs.bcr_isotype.is_some() || inputs.bcr_isotype_error.is_some() { report.add_section(build_bcr_isotype_section(&inputs)); }
    if inputs.tcr_alpha.is_some() || inputs.tcr_alpha_error.is_some() {
        report.add_section(build_tcr_section("TCR alpha", inputs.tcr_alpha, inputs.tcr_alpha_error, inputs.comparison));
    }
    if inputs.tcr_beta.is_some() || inputs.tcr_beta_error.is_some() {
        report.add_section(build_tcr_section("TCR beta", inputs.tcr_beta, inputs.tcr_beta_error, inputs.comparison));
    }
    if inputs.immune.is_some() || inputs.immune_error.is_some() { report.add_section(build_immune_section(&inputs)); }

    let output = path.to_string_lossy().to_string();
    report
        .save_to_file(&output)
        .with_context(|| format!("Could not write HTML report to {}", path.display()))?;
    make_report_self_contained(path)?;
    Ok(())
}

fn make_report_self_contained(path: &Path) -> Result<()> {
    let mut rendered = fs::read_to_string(path)
        .with_context(|| format!("Could not reopen generated HTML report {}", path.display()))?;

    // report-builder 0.1.1 embeds Plotly figures as inline fragments but currently
    // references Plotly and table-helper libraries from public CDNs. Plotly 0.12.1
    // can bundle its JavaScript when compiled with `plotly_embed_js`; obtain that
    // bundle from a tiny standalone Plot and inject it once into the report.
    let plot_shell = plotly::Plot::new().to_html();
    let script_start = plot_shell
        .find("<script")
        .context("Could not locate embedded Plotly JavaScript")?;
    let plot_div = plot_shell
        .find(r#"<div id="plotly-html-element""#)
        .context("Could not locate Plotly placeholder in standalone HTML")?;
    if script_start >= plot_div {
        anyhow::bail!("Unexpected Plotly standalone HTML layout");
    }
    let embedded_plotly = &plot_shell[script_start..plot_div];
    const REPORT_BUILDER_PLOTLY_CDN: &str =
        r#"<script src="https://cdn.plot.ly/plotly-latest.min.js"></script>"#;
    if rendered.contains(REPORT_BUILDER_PLOTLY_CDN) {
        rendered = rendered.replace(REPORT_BUILDER_PLOTLY_CDN, embedded_plotly);
    } else {
        anyhow::bail!("report-builder Plotly CDN marker was not found; its template may have changed");
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

    fs::write(path, rendered)
        .with_context(|| format!("Could not finalize standalone HTML report {}", path.display()))?;
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
        section.add_content(html! { p { (error.unwrap_or("Expression analysis was not available.")) } });
        return section;
    };

    let n_sig = result.tests.iter().filter(|r| r.q_value.is_finite() && r.q_value <= 0.05).count();
    section.add_content(html! {
        p {
            "Metric: " strong { (result.metric) } ". "
            "Case columns: " (result.n_case_columns) ", control columns: " (result.n_control_columns) ", tested " (feature_name_plural) ": " (result.tests.len()) ", BH q≤0.05: " (n_sig) "."
        }
        p { "Mann–Whitney U is applied to log2(value + 1). Effect is log2((mean case + 0.5)/(mean control + 0.5)); BH adjustment is across tested features." }
    });

    if let Some(plot) = expression_volcano_plot(result, &format!("{section_name} discovery screen"), feature_name) {
        section.add_plot(plot);
    }
    if let Some(plot) = expression_detection_plot(result, &format!("{section_name} detection shift"), feature_name) {
        section.add_plot(plot);
    }
    section.add_content(feature_table(&format!("Top differential {feature_name_plural}"), &result.tests, 100));
    section
}

fn build_bcr_section(inputs: &CohortReportInputs<'_>) -> ReportSection {
    let mut section = ReportSection::new("BCR");
    let Some(result) = inputs.bcr else {
        section.add_content(html! { p { (inputs.bcr_error.unwrap_or("BCR analysis was not available.")) } });
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
        let case = result.sample_summaries.iter()
            .filter(|r| r.group == inputs.comparison.case_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let ctrl = result.sample_summaries.iter()
            .filter(|r| r.group == inputs.comparison.control_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if case.len() >= 2 && ctrl.len() >= 2 {
            let scores = vec![ctrl, case];
            let labels = vec![inputs.comparison.control_label.clone(), inputs.comparison.case_label.clone()];
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

    section.add_content(feature_table("Sample-level BCR metrics", &result.metric_tests, 50));
    section.add_content(feature_table("Heavy-chain isotype usage", &result.isotype_tests, 50));
    if let Some(plot) = feature_effect_significance_plot("BCR V-gene usage", &result.v_gene_tests, "V gene") {
        section.add_plot(plot);
    }
    section.add_content(feature_table("V-gene usage", &result.v_gene_tests, 75));
    if let Some(plot) = feature_effect_significance_plot("BCR D-gene usage", &result.d_gene_tests, "D gene") {
        section.add_plot(plot);
    }
    section.add_content(feature_table("D-gene usage", &result.d_gene_tests, 75));
    if let Some(plot) = feature_effect_significance_plot("BCR J-gene usage", &result.j_gene_tests, "J gene") {
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
        let ctrl = result.values.iter()
            .filter(|r| r.isotype == feature && r.group == inputs.comparison.control_label)
            .map(|r| r.fraction)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let case = result.values.iter()
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
        if let Ok(plot) = plot_boxplot(&scores, labels, "BCR isotype composition by group", "Isotype / group", "Fraction") {
            section.add_plot(plot);
        }
    }
    if let Some(plot) = feature_effect_significance_plot("BCR isotype fraction shifts", &result.tests, "BCR isotype") {
        section.add_plot(plot);
    }
    section.add_content(feature_table("BCR isotype fraction tests", &result.tests, 50));
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
        p { "This initial TCR comparison reports depth-dependent repertoire summaries plus V-gene, J-gene and V–J-pair composition. Matched-depth rarefaction, Morisita–Horn/PCoA, public/private clonotypes and convergence are planned repertoire extensions." }
        @if !result.file_errors.is_empty() {
            details {
                summary { "File warnings (" (result.file_errors.len()) ")" }
                ul { @for warning in &result.file_errors { li { (warning) } } }
            }
        }
    });

    for (metric_name, getter) in report_tcr_metric_specs() {
        let case = result.sample_summaries.iter()
            .filter(|r| r.group == comparison.case_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        let ctrl = result.sample_summaries.iter()
            .filter(|r| r.group == comparison.control_label)
            .map(getter)
            .filter(|v| v.is_finite())
            .collect::<Vec<_>>();
        if case.len() >= 2 && ctrl.len() >= 2 {
            let scores = vec![ctrl, case];
            let labels = vec![comparison.control_label.clone(), comparison.case_label.clone()];
            if let Ok(plot) = plot_boxplot(&scores, labels, &format!("{}: {metric_name}", result.chain), "Group", metric_name) {
                section.add_plot(plot);
            }
        }
    }
    section.add_content(feature_table(&format!("{} sample-level repertoire metrics", result.chain), &result.metric_tests, 50));
    if let Some(plot) = feature_effect_significance_plot(&format!("{} V-gene usage", result.chain), &result.v_gene_tests, "V gene") { section.add_plot(plot); }
    section.add_content(feature_table("V-gene usage", &result.v_gene_tests, 75));
    if let Some(plot) = feature_effect_significance_plot(&format!("{} J-gene usage", result.chain), &result.j_gene_tests, "J gene") { section.add_plot(plot); }
    section.add_content(feature_table("J-gene usage", &result.j_gene_tests, 75));
    if let Some(plot) = feature_effect_significance_plot(&format!("{} V–J pair usage", result.chain), &result.vj_pair_tests, "V-J pair") { section.add_plot(plot); }
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
    if let Some(plot) = feature_effect_significance_plot("Immune-cell ssGSEA shifts", &result.tests, "Immune cell") {
        section.add_plot(plot);
    }

    let mut features = BTreeSet::new();
    for row in &result.values { features.insert(row.cell_type.clone()); }
    for feature in features {
        let ctrl = result.values.iter()
            .filter(|r| r.cell_type == feature && r.group == inputs.comparison.control_label)
            .map(|r| r.score).filter(|v| v.is_finite()).collect::<Vec<_>>();
        let case = result.values.iter()
            .filter(|r| r.cell_type == feature && r.group == inputs.comparison.case_label)
            .map(|r| r.score).filter(|v| v.is_finite()).collect::<Vec<_>>();
        if ctrl.len() >= 2 && case.len() >= 2 {
            if let Ok(plot) = plot_boxplot(
                &vec![ctrl, case],
                vec![inputs.comparison.control_label.clone(), inputs.comparison.case_label.clone()],
                &format!("Immune infiltrate: {feature}"),
                "Group",
                "ssGSEA score",
            ) { section.add_plot(plot); }
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
        if x.is_empty() { continue; }
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

fn expression_volcano_plot(result: &GeneComparisonResult, title: &str, feature_label: &str) -> Option<Plot> {
    let mut x_other = Vec::new();
    let mut y_other = Vec::new();
    let mut h_other = Vec::new();
    let mut x_sig = Vec::new();
    let mut y_sig = Vec::new();
    let mut h_sig = Vec::new();
    for r in &result.tests {
        if !r.effect.is_finite() || !r.q_value.is_finite() || r.q_value <= 0.0 { continue; }
        let y = -r.q_value.max(1e-300).log10();
        let hover = feature_hover(r, feature_label);
        if r.q_value <= 0.05 {
            x_sig.push(r.effect); y_sig.push(y); h_sig.push(hover);
        } else {
            x_other.push(r.effect); y_other.push(y); h_other.push(hover);
        }
    }
    labeled_scatter_plot(
        vec![
            (format!("Other {}", if feature_label == "Gene" { "genes" } else { "transcripts" }), x_other, y_other, h_other),
            ("BH q ≤ 0.05".to_string(), x_sig, y_sig, h_sig),
        ],
        title,
        "log2 mean fold change (case / control)",
        "-log10(BH q)",
    )
}

fn expression_detection_plot(result: &GeneComparisonResult, title: &str, feature_label: &str) -> Option<Plot> {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut hover = Vec::new();
    for r in &result.tests {
        let (Some(case), Some(ctrl)) = (r.detection_case, r.detection_control) else { continue; };
        if case.is_finite() && ctrl.is_finite() {
            x.push(ctrl * 100.0);
            y.push(case * 100.0);
            let secondary_label = if feature_label == "Gene" || feature_label == "Transcript" { "NCBI Gene ID" } else { "Secondary ID" };
            let secondary = if r.secondary_id.trim().is_empty() { String::new() } else { format!("<br>{}: {}", secondary_label, escape_hover_text(&r.secondary_id)) };
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

fn feature_effect_significance_plot(title: &str, tests: &[FeatureTestResult], feature_label: &str) -> Option<Plot> {
    let mut x_other = Vec::new();
    let mut y_other = Vec::new();
    let mut h_other = Vec::new();
    let mut x_sig = Vec::new();
    let mut y_sig = Vec::new();
    let mut h_sig = Vec::new();
    for r in tests {
        if !r.effect.is_finite() || !r.q_value.is_finite() || r.q_value <= 0.0 { continue; }
        let y = -r.q_value.max(1e-300).log10();
        let hover = feature_hover(r, feature_label);
        if r.q_value <= 0.05 {
            x_sig.push(r.effect); y_sig.push(y); h_sig.push(hover);
        } else {
            x_other.push(r.effect); y_other.push(y); h_other.push(hover);
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

fn group_mean_isotype_table(result: &BcrIsotypeComparisonResult, comp: &CohortComparison) -> Markup {
    let mut by_feature: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for row in &result.values {
        let entry = by_feature.entry(row.isotype.clone()).or_default();
        if row.group == comp.case_label { entry.0.push(row.fraction); }
        else if row.group == comp.control_label { entry.1.push(row.fraction); }
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
    if has_result { "available".to_string() }
    else if let Some(err) = error { format!("not available — {err}") }
    else { "not requested".to_string() }
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
    if values.is_empty() { f64::NAN } else { values.iter().sum::<f64>() / values.len() as f64 }
}

fn fmt(value: f64) -> String {
    if value.is_finite() { format!("{value:.4}") } else { "NA".to_string() }
}

fn fmt_sci(value: f64) -> String {
    if value.is_finite() { format!("{value:.3e}") } else { "NA".to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cohort::{CohortComparison, FeatureTestResult, GeneComparisonResult};
    use std::collections::HashSet;

    #[test]
    fn writes_cohort_html_report_with_gene_plot() {
        let comparison = CohortComparison {
            name: "HYP vs True_HC".to_string(),
            case_label: "HYP".to_string(),
            control_label: "True_HC".to_string(),
            case_ids: HashSet::new(),
            control_ids: HashSet::new(),
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
            },
        )
        .unwrap();
        let html = std::fs::read_to_string(path).unwrap();
        assert!(html.contains("4C Cohort Explorer"));
        assert!(html.contains("Gene expression"));
        assert!(html.contains("TEST1"));
        assert!(html.contains("Gene: TEST1"));
        assert!(html.contains("NCBI Gene ID: 1"));
        assert!(!html.contains(r#"<script src="https://cdn.plot.ly/plotly-latest.min.js"></script>"#));
        assert!(!html.contains("https://cdn.datatables.net/"));
        assert!(!html.contains("https://cdnjs.cloudflare.com/ajax/libs/jquery/"));
        assert!(html.contains("Plotly"));
    }
}
