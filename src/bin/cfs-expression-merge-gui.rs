use cfs_expression_merger::analysis::{
    CohortAnalysisSettingsSnapshot, CohortDefinitionSnapshot, CohortModalitySelection,
    CorrectedAnalysisBundle, UnifiedCohortAnalysisRun, write_unified_analysis_tables,
};
use cfs_expression_merger::analysis_docs::{
    AnalysisDocToyChart, AnalysisDocToyExample, analysis_doc_example_matches,
    analysis_doc_section_header_matches, analysis_doc_section_matches, analysis_doc_sections,
    analysis_doc_subsection_matches,
};
use cfs_expression_merger::backend::{
    AnalysisBackend, LocalBackend, LocalOutlierRequest, LocalUnifiedAnalysisRequest,
    MetadataLocator, OutlierAnalysisRequest, ProjectLocator, UnifiedAnalysisRequest,
};
use cfs_expression_merger::cohort::{
    BcrComparisonResult, BcrIsotypeComparisonResult, CohortComparison, CohortComparisonDefinition,
    CohortMapping, CohortModality, ComparisonPreset, FeatureTestResult, GeneComparisonResult,
    ImmuneComparisonResult, MappingStatus, MetadataComparisonSpec, MetadataTable, ProjectScan,
    TcrComparisonResult, build_comparison, build_comparison_definition, load_metadata,
    normalize_sample_id, run_bcr_comparison, run_bcr_isotype_comparison, run_gene_comparison,
    run_immune_infiltrate_comparison, run_tcr_comparison, run_transcript_comparison,
    write_bcr_isotype_values, write_bcr_sample_summaries, write_feature_tests, write_immune_values,
    write_mapping_audit, write_mapping_audit_csv, write_tcr_sample_summaries,
};
use cfs_expression_merger::corrected::{
    AdjustedFeatureResult, AmbiguousCallPolicy, CorrectedCompositionResult,
    CorrectedExpressionResult, CorrectedImmuneResult, CorrectedIsotypeResult, CorrectedModelSpec,
    CorrectedPathwayResult, CorrectedRepertoireResult, DendrogramResult, OrdinationResult,
    run_corrected_expression, run_corrected_immune, run_corrected_isotype,
    run_corrected_repertoire, run_pathway_ora, write_adjusted_tests,
    write_corrected_repertoire_samples, write_dendrogram, write_dtu_gene_tests, write_ordination,
    write_pathway_ora,
};
use cfs_expression_merger::diagnostics::{
    CandidateStabilityRow, PbmcDiagnosticsConfig, PbmcDiagnosticsResult, PbmcSampleQcRow,
    run_pbmc_diagnostics, write_pbmc_diagnostics_tables,
};
use cfs_expression_merger::focused::{
    FocusedCandidateSettings, FocusedCandidateSummary, build_focused_candidate_summary,
    write_focused_candidate_tables,
};
use cfs_expression_merger::gmt_catalog::{
    GMT_CATALOG, MSIGDB_LICENSE_URL, MSIGDB_RELEASE_URL, MSIGDB_VERSION,
    cache_path as gmt_cache_path, download_public_gmt,
};
use cfs_expression_merger::metadata::{
    STANDARD_METADATA_FIELDS, add_missing_standard_columns, canonical_sample_id,
    new_standard_metadata, row_matching_ids, unique_values as metadata_unique_values,
    validate_standard_metadata, write_metadata_tsv,
};
use cfs_expression_merger::outliers::{
    CrossModalOutlierResult, OutlierConfig, OutlierConsensusRule, OutlierFilterSnapshot,
    OutlierScope, build_outlier_filter_snapshot, filter_comparison_excluding,
    write_outlier_audit_csv, write_outlier_filter_modalities_tsv,
    write_outlier_filter_snapshot_csv, write_outlier_filter_summary,
};
use cfs_expression_merger::report::{
    CohortReportInputs, CorrectedReportInputs, FocusedReportInputs, generate_cohort_html_report,
    generate_corrected_html_report, generate_focused_candidate_html_report,
    generate_unified_detailed_html_report, generate_unified_simplified_html_report,
};
use cfs_expression_merger::ui_theme;
use cfs_expression_merger::{
    ColumnOrder, DataKind, EntityCatalogEntry, EntityProfilePoint, MergeConfig, MergeSummary,
    MergedDatasetInfo, Metric, SampleDistributionSummary, all_metrics,
    collaborator_default_metrics, inspect_kind, inspect_merged_tsv, load_entity_catalog,
    load_entity_catalog_from_merged, load_entity_profile, load_entity_profile_from_merged,
    merge_wide, sample_name, summarize_merged_distributions, summarize_sample_distributions,
};
use eframe::egui;
use egui_plot::{
    Bar, BarChart, BoxElem, BoxPlot, BoxSpread, HoverPosition, Legend, Line, Plot, Points,
};
use rfd::FileDialog;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    mpsc::{self, Receiver},
};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([960.0, 640.0]),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "4C Cohort Explorer",
        options,
        Box::new(|cc| Ok(Box::new(MergeApp::new(cc)))),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppTab {
    Merge,
    Explore,
    Metadata,
    Cohort,
    Docs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnifiedComparisonMode {
    BuiltIn,
    CustomMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisSource {
    Workbooks,
    MergedTsv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExplorePanel {
    SampleQc,
    EntityExplorer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SampleQcView {
    BoxPlots,
    DetectionRate,
    MedianVsDetection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileView {
    SampleOrder,
    Ranked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CohortResultView {
    Gene,
    Transcript,
    Bcr,
    BcrIsotype,
    TcrAlpha,
    TcrBeta,
    Immune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CohortAnalysisMode {
    Exploratory,
    Corrected,
    Focused,
    Diagnostics,
    Docs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CorrectedResultView {
    Gene,
    Transcript,
    Bcr,
    BcrIsotype,
    TcrAlpha,
    TcrBeta,
    Immune,
    Pathway,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnifiedResultView {
    Overview,
    Gene,
    Transcript,
    Bcr,
    BcrIsotype,
    TcrAlpha,
    TcrBeta,
    Immune,
    Pathway,
    QcRobustness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedResultView {
    Overview,
    Genes,
    Pathways,
    Other,
    Transcripts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagnosticsResultView {
    Overview,
    SampleQc,
    CandidateRobustness,
    SourceEffect,
    Sensitivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BcrMetricView {
    Shannon,
    Clonality,
    UniqueClones,
    TopCloneFraction,
    EffectiveClonotypes,
    WeightedCdr3Length,
}

impl BcrMetricView {
    fn display_name(self) -> &'static str {
        match self {
            Self::Shannon => "Shannon diversity",
            Self::Clonality => "Clonality",
            Self::UniqueClones => "Unique clones",
            Self::TopCloneFraction => "Top clone fraction",
            Self::EffectiveClonotypes => "Effective clonotypes",
            Self::WeightedCdr3Length => "Weighted mean CDR3 AA length",
        }
    }

    fn value(self, row: &cfs_expression_merger::cohort::BcrSampleSummary) -> f64 {
        match self {
            Self::Shannon => row.shannon,
            Self::Clonality => row.clonality,
            Self::UniqueClones => row.unique_clones,
            Self::TopCloneFraction => row.top_clone_fraction,
            Self::EffectiveClonotypes => row.effective_n,
            Self::WeightedCdr3Length => row.weighted_mean_cdr3_aa_length,
        }
    }

    const ALL: [Self; 6] = [
        Self::Shannon,
        Self::Clonality,
        Self::UniqueClones,
        Self::TopCloneFraction,
        Self::EffectiveClonotypes,
        Self::WeightedCdr3Length,
    ];
}

#[derive(Debug, Clone)]
struct CohortAnalysisBundle {
    comparison: CohortComparison,
    gene: Option<GeneComparisonResult>,
    gene_error: Option<String>,
    transcript: Option<GeneComparisonResult>,
    transcript_error: Option<String>,
    bcr: Option<BcrComparisonResult>,
    bcr_error: Option<String>,
    bcr_isotype: Option<BcrIsotypeComparisonResult>,
    bcr_isotype_error: Option<String>,
    tcr_alpha: Option<TcrComparisonResult>,
    tcr_alpha_error: Option<String>,
    tcr_beta: Option<TcrComparisonResult>,
    tcr_beta_error: Option<String>,
    immune: Option<ImmuneComparisonResult>,
    immune_error: Option<String>,
}

#[derive(Debug, Clone)]
struct FocusedAnalysisBundle {
    corrected: CorrectedAnalysisBundle,
    candidates: FocusedCandidateSummary,
}

enum MergeWorkerMessage {
    Progress(usize, usize, String),
    Done(Result<MergeSummary, String>),
}

enum AnalysisWorkerMessage {
    Progress(usize, usize, String),
    CatalogDone(Result<Vec<EntityCatalogEntry>, String>),
    DistributionDone(Result<Vec<SampleDistributionSummary>, String>),
    ProfileDone(Result<Vec<EntityProfilePoint>, String>),
}

enum CohortWorkerMessage {
    Progress(f32, String),
    Done(CohortAnalysisBundle),
    CorrectedDone(CorrectedAnalysisBundle),
    FocusedDone(FocusedAnalysisBundle),
    DiagnosticsDone(PbmcDiagnosticsResult),
    UnifiedDone(Result<UnifiedCohortAnalysisRun, String>),
    OutlierDone(Result<CrossModalOutlierResult, String>),
}

enum GmtDownloadMessage {
    Done(Result<PathBuf, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeMetadataMatchStatus {
    Matched,
    Unmatched,
    Ambiguous,
}

#[derive(Debug, Clone)]
struct MergeMetadataFileStatus {
    path: PathBuf,
    sample_id: String,
    status: MergeMetadataMatchStatus,
    metadata_row: Option<usize>,
    filter_value: String,
    included: bool,
}

struct MergeApp {
    active_tab: AppTab,
    analysis_backend: Arc<dyn AnalysisBackend>,
    files: Vec<PathBuf>,
    sample_filter: String,
    input_path_text: String,
    merge_metadata_path: Option<PathBuf>,
    merge_metadata: Option<MetadataTable>,
    merge_metadata_filter_enabled: bool,
    merge_metadata_filter_column: Option<String>,
    merge_metadata_filter_values: HashSet<String>,
    last_directory: Option<PathBuf>,
    detected_kind: DataKind,
    kind: DataKind,
    metrics: Vec<Metric>,
    order: ColumnOrder,
    output: Option<PathBuf>,

    status: String,
    progress: f32,
    running: bool,
    merge_receiver: Option<Receiver<MergeWorkerMessage>>,

    analysis_source: AnalysisSource,
    explore_panel: ExplorePanel,
    merged_analysis_path: Option<PathBuf>,
    merged_info: Option<MergedDatasetInfo>,
    analysis_metric: Metric,
    analysis_ignore_zeros: bool,
    analysis_log1p: bool,
    sample_qc_view: SampleQcView,
    profile_view: ProfileView,
    analysis_status: String,
    analysis_progress: f32,
    analysis_running: bool,
    analysis_receiver: Option<Receiver<AnalysisWorkerMessage>>,
    entity_catalog: Vec<EntityCatalogEntry>,
    entity_visible: Vec<usize>,
    entity_search: String,
    selected_entity: Option<usize>,
    distributions: Vec<SampleDistributionSummary>,
    profile: Vec<EntityProfilePoint>,
    docs_search: String,

    cohort_root: Option<PathBuf>,
    cohort_project_source: Option<ProjectLocator>,
    cohort_scan: Option<ProjectScan>,
    cohort_metadata_path: Option<PathBuf>,
    cohort_metadata_source: Option<MetadataLocator>,
    cohort_metadata: Option<MetadataTable>,
    cohort_mapping: Option<CohortMapping>,
    cohort_preset: ComparisonPreset,
    unified_comparison_mode: UnifiedComparisonMode,
    custom_comparison_column: Option<String>,
    custom_case_values: HashSet<String>,
    custom_control_values: HashSet<String>,
    custom_comparison_name: String,
    custom_case_label: String,
    custom_control_label: String,
    metadata_builder_path: Option<PathBuf>,
    metadata_builder: Option<MetadataTable>,
    metadata_builder_selected_row: usize,
    metadata_builder_status: String,
    cohort_status: String,
    cohort_progress: f32,
    cohort_running: bool,
    cohort_receiver: Option<Receiver<CohortWorkerMessage>>,
    cohort_results: Option<CohortAnalysisBundle>,
    cohort_result_view: CohortResultView,
    cohort_analysis_mode: CohortAnalysisMode,
    corrected_results: Option<CorrectedAnalysisBundle>,
    corrected_result_view: CorrectedResultView,
    unified_results: Option<UnifiedCohortAnalysisRun>,
    unified_result_view: UnifiedResultView,
    cohort_use_filtered_metadata: bool,
    unified_run_diagnostics: bool,
    corrected_subject_column: Option<String>,
    corrected_covariates: HashSet<String>,
    corrected_ambiguous_policy: AmbiguousCallPolicy,
    corrected_exclude_undetermined: bool,
    corrected_permutations: usize,
    corrected_rarefaction_reps: usize,
    corrected_rarefaction_cap: usize,
    corrected_run_pathway: bool,
    corrected_gmt_path: Option<PathBuf>,
    gmt_catalog_index: usize,
    gmt_download_status: String,
    gmt_download_receiver: Option<Receiver<GmtDownloadMessage>>,
    corrected_pathway_q: f64,
    corrected_pathway_effect: f64,
    corrected_ordination_color_by: String,
    focused_metadata_path: Option<PathBuf>,
    focused_metadata_source: Option<MetadataLocator>,
    focused_metadata: Option<MetadataTable>,
    focused_mapping: Option<CohortMapping>,
    focused_run_exploratory_concordance: bool,
    focused_settings: FocusedCandidateSettings,
    focused_results: Option<FocusedCandidateSummary>,
    focused_result_view: FocusedResultView,
    diagnostics_results: Option<PbmcDiagnosticsResult>,
    diagnostics_result_view: DiagnosticsResultView,
    diagnostics_known_non_pbmc_text: String,
    diagnostics_suspect_ids_text: String,
    diagnostics_batch_column: Option<String>,
    diagnostics_family_column: Option<String>,
    diagnostics_sensitivity_column: Option<String>,
    diagnostics_auto_exclude_expression_outliers: bool,
    diagnostics_auto_exclude_immune_outliers: bool,
    diagnostics_exclude_suspects: bool,
    diagnostics_outlier_z: f64,
    diagnostics_top_candidates: usize,
    diagnostics_selected_gene: Option<String>,
    outlier_results: Option<CrossModalOutlierResult>,
    corrected_outlier_snapshot: Option<OutlierFilterSnapshot>,
    focused_outlier_snapshot: Option<OutlierFilterSnapshot>,
    outlier_config: OutlierConfig,
    outlier_excluded_ids: HashSet<String>,
    outlier_apply_to_corrected: bool,
    outlier_apply_to_focused: bool,
    outlier_plot_modality: usize,
    cohort_run_gene: bool,
    cohort_gene_metric: Metric,
    cohort_run_transcript: bool,
    cohort_transcript_metric: Metric,
    cohort_run_bcr: bool,
    cohort_run_bcr_isotype: bool,
    cohort_run_tcr_alpha: bool,
    cohort_run_tcr_beta: bool,
    cohort_run_immune: bool,
    cohort_bcr_metric_view: BcrMetricView,
}

fn diagnostics_column_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    columns: &[String],
    value: &mut Option<String>,
) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.clone().unwrap_or_else(|| "(none)".to_string()))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "(none)");
            for col in columns {
                ui.selectable_value(value, Some(col.clone()), col);
            }
        });
}

fn parse_sample_id_text(text: &str) -> HashSet<String> {
    text.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_sample_id)
        .filter(|s| !s.is_empty())
        .collect()
}

fn merge_metadata_sample_name(path: &Path) -> String {
    let mut stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("sample")
        .to_string();

    // Explorer/Finder copies are often named `...report (1).xlsx`. Strip the
    // copy suffix before applying the normal expression-report suffix rules so
    // metadata matching behaves like the Cohort Analysis sample matcher.
    if let Some(open) = stem.rfind(" (") {
        if stem.ends_with(')') {
            let digits = &stem[open + 2..stem.len() - 1];
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                stem.truncate(open);
            }
        }
    }

    const SUFFIXES: &[&str] = &[
        "_tumor_rna_gene_expression_report",
        "_tumor_rna_transcript_expression_report",
        "_rna_gene_expression_report",
        "_rna_transcript_expression_report",
        "_gene_expression_report",
        "_transcript_expression_report",
    ];
    for suffix in SUFFIXES {
        if let Some(trimmed) = stem.strip_suffix(suffix) {
            return trimmed.to_string();
        }
    }
    stem
}

fn format_float_or_dash(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.3}")
    } else {
        "—".to_string()
    }
}

fn format_sci_or_dash(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.2e}")
    } else {
        "—".to_string()
    }
}

fn format_range(min: f64, max: f64) -> String {
    if min.is_finite() && max.is_finite() {
        format!("{min:.2} … {max:.2}")
    } else {
        "—".to_string()
    }
}

fn deterministic_jitter(text: &str) -> f64 {
    let mut hash = 0xcbf29ce484222325u64;
    for b in text.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let unit = (hash % 10_000) as f64 / 9_999.0;
    unit - 0.5
}

fn fixed_table_viewport<F>(ui: &mut egui::Ui, id: &str, height: f32, min_width: f32, add: F)
where
    F: FnOnce(&mut egui::Ui),
{
    let width = ui.available_width().max(320.0);
    ui.allocate_ui(egui::vec2(width, height), |ui| {
        egui::ScrollArea::both()
            .id_salt(id)
            .max_height(height)
            .show(ui, |ui| {
                ui.set_min_width(min_width.max(width - 20.0));
                add(ui);
            });
    });
}

fn nearest_hover_label(x: f64, y: f64, points: &[(f64, f64, String)]) -> Option<String> {
    points
        .iter()
        .min_by(|a, b| {
            let da = (a.0 - x).powi(2) + (a.1 - y).powi(2);
            let db = (b.0 - x).powi(2) + (b.1 - y).powi(2);
            da.total_cmp(&db)
        })
        .map(|p| p.2.clone())
}

fn compact_comma_list(text: &str, max_items: usize, max_chars: usize) -> String {
    let items = text
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
    if remaining > 0 {
        format!("{} … (+{} more)", shown.join(", "), remaining)
    } else {
        shown.join(", ")
    }
}

fn bounded_plot_indices(n: usize, max_total: usize, keep_head: usize) -> Vec<usize> {
    if n <= max_total {
        return (0..n).collect();
    }
    let head = keep_head.min(max_total).min(n);
    let mut out = (0..head).collect::<Vec<_>>();
    let remaining = n - head;
    let slots = max_total.saturating_sub(head).min(remaining);
    if slots == 0 {
        return out;
    }
    for k in 0..slots {
        out.push(head + (k * remaining) / slots);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn category_color(
    category: &str,
    index: usize,
    color_by: &str,
    accent: egui::Color32,
) -> egui::Color32 {
    if color_by == "Analysis group" {
        return match category {
            "Case" => accent,
            "Control" => egui::Color32::from_rgb(105, 165, 225),
            _ => egui::Color32::from_gray(160),
        };
    }
    const PALETTE: [(u8, u8, u8); 12] = [
        (70, 145, 210),
        (225, 125, 65),
        (90, 180, 120),
        (195, 95, 155),
        (175, 145, 70),
        (120, 105, 200),
        (70, 175, 175),
        (220, 90, 95),
        (150, 165, 80),
        (120, 175, 230),
        (210, 135, 190),
        (165, 165, 165),
    ];
    let (r, g, b) = PALETTE[index % PALETTE.len()];
    egui::Color32::from_rgb(r, g, b)
}

impl MergeApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ui_theme::apply_4c_theme(&cc.egui_ctx);

        Self {
            active_tab: AppTab::Merge,
            analysis_backend: Arc::new(LocalBackend),
            files: Vec::new(),
            sample_filter: String::new(),
            input_path_text: String::new(),
            merge_metadata_path: None,
            merge_metadata: None,
            merge_metadata_filter_enabled: false,
            merge_metadata_filter_column: None,
            merge_metadata_filter_values: HashSet::new(),
            last_directory: load_last_directory(),
            detected_kind: DataKind::Auto,
            kind: DataKind::Auto,
            metrics: Vec::new(),
            order: ColumnOrder::AdjacentBySample,
            output: None,
            status: "Choose, paste, or drop a folder or a set of Excel files.".to_string(),
            progress: 0.0,
            running: false,
            merge_receiver: None,
            analysis_source: AnalysisSource::Workbooks,
            explore_panel: ExplorePanel::SampleQc,
            merged_analysis_path: None,
            merged_info: None,
            analysis_metric: Metric::Tpm,
            analysis_ignore_zeros: false,
            analysis_log1p: true,
            sample_qc_view: SampleQcView::BoxPlots,
            profile_view: ProfileView::SampleOrder,
            analysis_status:
                "Load workbooks or open a previously merged TSV to enable QC and plotting."
                    .to_string(),
            analysis_progress: 0.0,
            analysis_running: false,
            analysis_receiver: None,
            entity_catalog: Vec::new(),
            entity_visible: Vec::new(),
            entity_search: String::new(),
            selected_entity: None,
            distributions: Vec::new(),
            profile: Vec::new(),
            docs_search: String::new(),

            cohort_root: None,
            cohort_project_source: None,
            cohort_scan: None,
            cohort_metadata_path: None,
            cohort_metadata_source: None,
            cohort_metadata: None,
            cohort_mapping: None,
            cohort_preset: ComparisonPreset::HypVsTrueHc,
            unified_comparison_mode: UnifiedComparisonMode::CustomMetadata,
            custom_comparison_column: None,
            custom_case_values: HashSet::new(),
            custom_control_values: HashSet::new(),
            custom_comparison_name: String::new(),
            custom_case_label: String::new(),
            custom_control_label: String::new(),
            metadata_builder_path: None,
            metadata_builder: None,
            metadata_builder_selected_row: 0,
            metadata_builder_status: "Create a standardized metadata table from the cohort scan or load an existing table to curate it.".to_string(),
            cohort_status: "Choose the dataset root to scan all supported data types.".to_string(),
            cohort_progress: 0.0,
            cohort_running: false,
            cohort_receiver: None,
            cohort_results: None,
            cohort_result_view: CohortResultView::Gene,
            cohort_analysis_mode: CohortAnalysisMode::Exploratory,
            corrected_results: None,
            corrected_result_view: CorrectedResultView::Gene,
            unified_results: None,
            unified_result_view: UnifiedResultView::Overview,
            cohort_use_filtered_metadata: false,
            unified_run_diagnostics: true,
            corrected_subject_column: None,
            corrected_covariates: HashSet::new(),
            corrected_ambiguous_policy: AmbiguousCallPolicy::Exclude,
            corrected_exclude_undetermined: false,
            corrected_permutations: 499,
            corrected_rarefaction_reps: 3,
            corrected_rarefaction_cap: 50_000,
            corrected_run_pathway: false,
            corrected_gmt_path: None,
            gmt_catalog_index: 0,
            gmt_download_status:
                "Select a public collection to download, or choose a custom GMT file.".to_string(),
            gmt_download_receiver: None,
            corrected_pathway_q: 0.05,
            corrected_pathway_effect: 0.5,
            corrected_ordination_color_by: "Analysis group".to_string(),
            focused_metadata_path: None,
            focused_metadata_source: None,
            focused_metadata: None,
            focused_mapping: None,
            focused_run_exploratory_concordance: true,
            focused_settings: FocusedCandidateSettings::default(),
            focused_results: None,
            focused_result_view: FocusedResultView::Overview,
            diagnostics_results: None,
            diagnostics_result_view: DiagnosticsResultView::Overview,
            diagnostics_known_non_pbmc_text: String::new(),
            diagnostics_suspect_ids_text: String::new(),
            diagnostics_batch_column: None,
            diagnostics_family_column: None,
            diagnostics_sensitivity_column: None,
            diagnostics_auto_exclude_expression_outliers: false,
            diagnostics_auto_exclude_immune_outliers: false,
            diagnostics_exclude_suspects: true,
            diagnostics_outlier_z: 4.0,
            diagnostics_top_candidates: 50,
            diagnostics_selected_gene: None,
            outlier_results: None,
            corrected_outlier_snapshot: None,
            focused_outlier_snapshot: None,
            outlier_config: OutlierConfig::default(),
            outlier_excluded_ids: HashSet::new(),
            outlier_apply_to_corrected: false,
            outlier_apply_to_focused: false,
            outlier_plot_modality: 0,
            cohort_run_gene: true,
            cohort_gene_metric: Metric::Tpm,
            cohort_run_transcript: false,
            cohort_transcript_metric: Metric::Tpm,
            cohort_run_bcr: true,
            cohort_run_bcr_isotype: true,
            cohort_run_tcr_alpha: false,
            cohort_run_tcr_beta: false,
            cohort_run_immune: false,
            cohort_bcr_metric_view: BcrMetricView::Shannon,
        }
    }
}

impl eframe::App for MergeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_merge_worker();
        self.poll_analysis_worker();
        self.poll_cohort_worker();
        self.poll_gmt_download_worker();
        let ctx = ui.ctx().clone();
        self.handle_dropped_paths(&ctx);

        egui::CentralPanel::default().show(ui, |ui| {
            self.render_header(ui);
            ui.add_space(10.0);
            self.render_navigation(ui);
            ui.add_space(10.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.active_tab {
                    AppTab::Merge => self.render_merge_tab(ui),
                    AppTab::Explore => self.render_explore_tab(ui),
                    AppTab::Metadata => self.render_metadata_tab(ui),
                    AppTab::Cohort => self.render_cohort_tab(ui),
                    AppTab::Docs => self.render_docs_tab(ui),
                });
        });

        if self.running
            || self.analysis_running
            || self.cohort_running
            || self.gmt_download_receiver.is_some()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl MergeApp {
    fn start_gmt_download(&mut self) {
        if self.gmt_download_receiver.is_some() {
            return;
        }
        let Some(root) = self.cohort_root.clone() else {
            self.gmt_download_status =
                "Choose the cohort project root before downloading a pathway collection."
                    .to_string();
            return;
        };
        let entry = GMT_CATALOG[self
            .gmt_catalog_index
            .min(GMT_CATALOG.len().saturating_sub(1))];
        let destination = gmt_cache_path(&root, entry);
        if destination.exists() {
            self.corrected_gmt_path = Some(destination.clone());
            self.gmt_download_status = format!(
                "Using cached {} collection: {}",
                entry.short_label,
                destination.display()
            );
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.gmt_download_receiver = Some(rx);
        self.gmt_download_status = format!(
            "Downloading {} (MSigDB v{})…",
            entry.short_label, MSIGDB_VERSION
        );
        std::thread::spawn(move || {
            let result = download_public_gmt(&root, entry).map_err(|e| format!("{e:#}"));
            let _ = tx.send(GmtDownloadMessage::Done(result));
        });
    }

    fn poll_gmt_download_worker(&mut self) {
        let mut done = None;
        if let Some(rx) = &self.gmt_download_receiver {
            while let Ok(message) = rx.try_recv() {
                match message {
                    GmtDownloadMessage::Done(result) => done = Some(result),
                }
            }
        }
        if let Some(result) = done {
            self.gmt_download_receiver = None;
            match result {
                Ok(path) => {
                    self.corrected_gmt_path = Some(path.clone());
                    self.gmt_download_status =
                        format!("Downloaded and selected: {}", path.display());
                }
                Err(err) => {
                    self.gmt_download_status = format!("Automatic download failed: {err}");
                }
            }
        }
    }

    fn render_gmt_source_selector(&mut self, ui: &mut egui::Ui, id: &str) {
        let old_index = self.gmt_catalog_index;
        let selected = GMT_CATALOG[self
            .gmt_catalog_index
            .min(GMT_CATALOG.len().saturating_sub(1))];
        ui.horizontal_wrapped(|ui|{
            ui.label("Public gene-set library:");
            egui::ComboBox::from_id_salt(format!("gmt_catalog_{id}"))
                .selected_text(selected.label)
                .width(390.0)
                .show_ui(ui,|ui|{
                    for (idx,entry) in GMT_CATALOG.iter().enumerate(){
                        ui.selectable_value(&mut self.gmt_catalog_index,idx,entry.label)
                            .on_hover_text(format!("{}\n\n{}",entry.description,entry.relevance));
                    }
                });
            let downloading=self.gmt_download_receiver.is_some();
            if ui.add_enabled(!downloading,egui::Button::new(if downloading{"Downloading…"}else{"Download & use"}))
                .on_hover_text("Download the selected public MSigDB gene-symbol GMT into this project's gene_sets cache and select it for pathway analysis. Cached copies are reused.")
                .clicked(){self.start_gmt_download();}
            if ui.button("Choose custom GMT…")
                .on_hover_text("Use any local GMT file instead of the built-in public collection catalog.")
                .clicked(){
                    let mut dialog=FileDialog::new().add_filter("GMT gene sets", &["gmt"]);
                    if let Some(root)=&self.cohort_root {dialog=dialog.set_directory(root);}
                    if let Some(path)=dialog.pick_file(){
                        self.corrected_gmt_path=Some(path.clone());
                        self.gmt_download_status=format!("Using custom GMT: {}",path.display());
                    }
                }
            if ui.button("Clear")
                .on_hover_text("Clear the currently selected pathway collection without deleting any cached files.")
                .clicked(){self.corrected_gmt_path=None;self.gmt_download_status="No GMT selected.".to_string();}
        });
        if self.gmt_catalog_index != old_index {
            let entry = GMT_CATALOG[self.gmt_catalog_index];
            if let Some(root) = &self.cohort_root {
                let cached = gmt_cache_path(root, entry);
                if cached.exists() {
                    self.corrected_gmt_path = Some(cached.clone());
                    self.gmt_download_status = format!(
                        "Selected cached {} collection: {}",
                        entry.short_label,
                        cached.display()
                    );
                } else {
                    self.gmt_download_status = format!(
                        "{} selected. Click Download & use to retrieve it.",
                        entry.short_label
                    );
                }
            }
        }
        let entry = GMT_CATALOG[self
            .gmt_catalog_index
            .min(GMT_CATALOG.len().saturating_sub(1))];
        ui.label(
            egui::RichText::new(format!("{} {}", entry.description, entry.relevance))
                .small()
                .weak(),
        );
        ui.horizontal_wrapped(|ui|{
            ui.label(egui::RichText::new(format!("Source: MSigDB v{} • human gene symbols • downloaded from the public Broad release directory.",MSIGDB_VERSION)).small().weak());
            ui.hyperlink_to("Browse release",MSIGDB_RELEASE_URL);
            ui.hyperlink_to("License / attribution",MSIGDB_LICENSE_URL);
        });
        ui.label(egui::RichText::new("MSigDB v2026.1.Hs is distributed under CC BY 4.0, with collection-specific terms where applicable. Keep the collection name/version in exported reports for provenance.").small().weak());
        if let Some(path) = &self.corrected_gmt_path {
            ui.horizontal_wrapped(|ui| {
                ui.label("Selected GMT:");
                ui.monospace(path.display().to_string());
            });
        }
        if !self.gmt_download_status.is_empty() {
            ui.label(
                egui::RichText::new(&self.gmt_download_status)
                    .small()
                    .weak(),
            );
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        let accent = ui.visuals().selection.bg_fill;
        ui_theme::header_frame(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(ui_theme::APP_TITLE)
                            .size(24.0)
                            .strong()
                            .color(accent),
                    );
                    ui.label(egui::RichText::new(ui_theme::APP_SUBTITLE).weak());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (kind_text, sample_count) = match self.active_tab {
                        AppTab::Explore => {
                            let text = match self.analysis_kind() {
                                DataKind::Gene => "Gene data",
                                DataKind::Transcript => "Transcript data",
                                DataKind::Auto => "Report type unknown",
                            };
                            (text, self.analysis_sample_count())
                        }
                        AppTab::Merge => {
                            let text = match self.effective_kind() {
                                DataKind::Gene => "Gene data",
                                DataKind::Transcript => "Transcript data",
                                DataKind::Auto => "Report type unknown",
                            };
                            (text, self.files.len())
                        }
                        AppTab::Metadata => (
                            "Sample metadata",
                            self.metadata_builder
                                .as_ref()
                                .map(|meta| meta.rows.len())
                                .unwrap_or(0),
                        ),
                        AppTab::Cohort => (
                            "Cohort project",
                            self.cohort_scan
                                .as_ref()
                                .map(|scan| scan.unique_samples())
                                .unwrap_or(0),
                        ),
                        AppTab::Docs => (
                            "Analysis guide",
                            self.cohort_scan
                                .as_ref()
                                .map(|scan| scan.unique_samples())
                                .unwrap_or(0),
                        ),
                    };
                    ui.label(egui::RichText::new(kind_text).strong());
                    ui.separator();
                    ui.label(format!("{sample_count} samples"));
                });
            });
        });
    }

    fn render_navigation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let merge = ui
                .selectable_label(self.active_tab == AppTab::Merge, "Merge & Export")
                .on_hover_text("Configure input workbooks, output measurements, column order, and write a merged TSV.");
            if merge.clicked() {
                self.active_tab = AppTab::Merge;
            }
            let explore = ui
                .selectable_label(self.active_tab == AppTab::Explore, "Explore & QC")
                .on_hover_text("Explore a merged TSV when available, or fall back to the selected Excel workbooks. Includes sample QC and searchable gene/transcript profiles.");
            if explore.clicked() {
                self.active_tab = AppTab::Explore;
                if self.analysis_source_available() && self.entity_catalog.is_empty() && !self.analysis_running {
                    self.start_catalog_load();
                }
            }
            let metadata = ui
                .selectable_label(self.active_tab == AppTab::Metadata, "Metadata Builder")
                .on_hover_text("Create, curate, validate, and export a standardized multi-omics sample metadata TSV with cohort/covariate fields and modality aliases.");
            if metadata.clicked() {
                self.active_tab = AppTab::Metadata;
            }
            let cohort = ui
                .selectable_label(self.active_tab == AppTab::Cohort, "Cohort Analysis")
                .on_hover_text("Scan a complete cohort dataset, join sample metadata, define built-in or custom metadata comparisons, review QC, and run the unified multi-modal analysis.");
            if cohort.clicked() {
                self.active_tab = AppTab::Cohort;
            }
            let docs = ui
                .selectable_label(self.active_tab == AppTab::Docs, "Docs")
                .on_hover_text("Plain-language documentation of the workflows, methods, interpretation, and key references.");
            if docs.clicked() {
                self.active_tab = AppTab::Docs;
            }
        });
    }

    fn render_docs_tab(&mut self, ui: &mut egui::Ui) {
        self.render_analysis_docs_card(ui);
    }

    fn render_metadata_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Metadata Builder / Curator");
        ui.label(
            egui::RichText::new(
                "Create or curate one standardized sample table for cohort definition, repeated-measure metadata, covariates, batch/source provenance, and cross-modality sample aliases. The saved TSV can be loaded directly into Cohort Analysis.",
            )
            .weak(),
        );
        ui.add_space(8.0);

        self.card(ui, "1. Create or load metadata", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("New blank standard table").clicked() {
                    this.metadata_builder = Some(new_standard_metadata(&[]));
                    this.metadata_builder_path = None;
                    this.metadata_builder_selected_row = 0;
                    this.metadata_builder_status = "Created a blank standard metadata table.".to_string();
                }
                if ui.button("Scan project folder & create table…").clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(root) = dialog.pick_folder() {
                        match this.analysis_backend.scan_project(&ProjectLocator::local(root.clone())) {
                            Ok(scan) => {
                                let mut seen = BTreeMap::<String, String>::new();
                                for file in &scan.files {
                                    seen.entry(normalize_sample_id(&file.sample_id))
                                        .or_insert_with(|| file.sample_id.clone());
                                }
                                let samples = seen.into_values().collect::<Vec<_>>();
                                this.cohort_root = Some(root.clone());
                                this.cohort_project_source = Some(ProjectLocator::local(root.clone()));
                                this.cohort_scan = Some(scan.clone());
                                this.cohort_mapping = this.cohort_metadata.as_ref().and_then(|meta| {
                                    this.analysis_backend.build_mapping(&scan, meta).ok()
                                });
                                this.last_directory = Some(root);
                                this.metadata_builder = Some(new_standard_metadata(&samples));
                                this.metadata_builder_path = None;
                                this.metadata_builder_selected_row = 0;
                                this.metadata_builder_status = format!(
                                    "Scanned the project and created standard metadata rows for {} samples.",
                                    samples.len()
                                );
                            }
                            Err(error) => {
                                this.metadata_builder_status = format!("Could not scan project: {error:#}");
                            }
                        }
                    }
                }
                if ui.button("Create from current cohort scan").clicked() {
                    if let Some(scan) = &this.cohort_scan {
                        let mut seen = BTreeMap::<String, String>::new();
                        for file in &scan.files {
                            seen.entry(normalize_sample_id(&file.sample_id))
                                .or_insert_with(|| file.sample_id.clone());
                        }
                        let samples = seen.into_values().collect::<Vec<_>>();
                        this.metadata_builder = Some(new_standard_metadata(&samples));
                        this.metadata_builder_path = None;
                        this.metadata_builder_selected_row = 0;
                        this.metadata_builder_status = format!(
                            "Created standard metadata rows for {} scanned samples.",
                            samples.len()
                        );
                    } else {
                        this.metadata_builder_status = "No cohort project is scanned yet. Open Cohort Analysis and scan a project first, or load an existing metadata table here.".to_string();
                    }
                }
                if ui.button("Load metadata…").clicked() {
                    let mut dialog = FileDialog::new().add_filter(
                        "Sample metadata",
                        &["xlsx", "xls", "csv", "tsv", "txt"],
                    );
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(path) = dialog.pick_file() {
                        match this.analysis_backend.load_metadata(&MetadataLocator::local(path.clone())) {
                            Ok(metadata) => {
                                this.last_directory = path.parent().map(Path::to_path_buf);
                                this.metadata_builder_path = Some(path.clone());
                                this.metadata_builder_selected_row = 0;
                                this.metadata_builder_status = format!(
                                    "Loaded {} rows from {}.",
                                    metadata.rows.len(),
                                    path.display()
                                );
                                this.metadata_builder = Some(metadata);
                            }
                            Err(error) => {
                                this.metadata_builder_status = format!("Could not load metadata: {error:#}");
                            }
                        }
                    }
                }
            });
            ui.label(egui::RichText::new(&this.metadata_builder_status).small().weak());
            if let Some(path) = &this.metadata_builder_path {
                ui.label(egui::RichText::new(format!("Source: {}", path.display())).small().weak());
            }
        });

        if self.metadata_builder.is_none() {
            return;
        }

        self.card(ui, "2. Validate schema", |this, ui| {
            let (columns, current_id) = this.metadata_builder.as_ref()
                .map(|m| (m.columns.clone(), m.id_column.clone()))
                .unwrap_or_default();
            let mut selected_id = current_id.clone();
            ui.horizontal_wrapped(|ui| {
                ui.label("Current sample ID column:");
                egui::ComboBox::from_id_salt("metadata_builder_id_column")
                    .selected_text(&selected_id)
                    .show_ui(ui, |ui| {
                        for column in &columns {
                            ui.selectable_value(&mut selected_id, column.clone(), column);
                        }
                    });
            });
            if selected_id != current_id {
                if let Some(metadata) = &mut this.metadata_builder {
                    metadata.id_column = selected_id;
                }
            }
            let report = validate_standard_metadata(this.metadata_builder.as_ref().unwrap());
            let row_count = this.metadata_builder.as_ref().map(|m| m.rows.len()).unwrap_or(0);
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{row_count} sample rows"));
                if report.is_valid() {
                    ui.colored_label(egui::Color32::from_rgb(105, 190, 125), "No blocking ID errors");
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(225, 120, 110),
                        format!("{} blocking issue(s)", report.errors.len()),
                    );
                }
                ui.label(format!("{} warning(s)", report.warnings.len()));
                ui.label(format!("{} comparison-ready categorical columns", report.comparison_columns.len()));
            });
            if ui.button("Add any missing standard columns").clicked() {
                if let Some(metadata) = &mut this.metadata_builder {
                    let added = add_missing_standard_columns(metadata);
                    this.metadata_builder_status = format!("Added {added} missing standard column(s); existing columns and values were preserved.");
                }
            }
            for error in report.errors.iter().take(8) {
                ui.colored_label(egui::Color32::from_rgb(225, 120, 110), error);
            }
            for warning in report.warnings.iter().take(8) {
                ui.label(egui::RichText::new(warning).small().weak());
            }
            if report.errors.len() > 8 || report.warnings.len() > 8 {
                ui.label(egui::RichText::new("Additional validation messages are omitted from this compact view.").small().weak());
            }
        });

        self.card(ui, "3. Curate samples", |this, ui| {
            let row_count = this
                .metadata_builder
                .as_ref()
                .map(|m| m.rows.len())
                .unwrap_or(0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Add sample row").clicked() {
                    if let Some(metadata) = &mut this.metadata_builder {
                        metadata
                            .rows
                            .push(vec![String::new(); metadata.columns.len()]);
                        this.metadata_builder_selected_row = metadata.rows.len().saturating_sub(1);
                    }
                }
                if ui
                    .add_enabled(row_count > 0, egui::Button::new("Delete selected row"))
                    .clicked()
                {
                    if let Some(metadata) = &mut this.metadata_builder {
                        if this.metadata_builder_selected_row < metadata.rows.len() {
                            metadata.rows.remove(this.metadata_builder_selected_row);
                            this.metadata_builder_selected_row = this
                                .metadata_builder_selected_row
                                .min(metadata.rows.len().saturating_sub(1));
                        }
                    }
                }
            });

            let row_labels = this
                .metadata_builder
                .as_ref()
                .map(|metadata| {
                    (0..metadata.rows.len())
                        .map(|row| {
                            let id = canonical_sample_id(metadata, row)
                                .unwrap_or_default()
                                .trim();
                            if id.is_empty() {
                                format!("Row {}", row + 2)
                            } else {
                                id.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !row_labels.is_empty() {
                this.metadata_builder_selected_row =
                    this.metadata_builder_selected_row.min(row_labels.len() - 1);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Selected sample:");
                    egui::ComboBox::from_id_salt("metadata_builder_selected_row")
                        .selected_text(&row_labels[this.metadata_builder_selected_row])
                        .show_ui(ui, |ui| {
                            for (idx, label) in row_labels.iter().enumerate() {
                                ui.selectable_value(
                                    &mut this.metadata_builder_selected_row,
                                    idx,
                                    label,
                                );
                            }
                        });
                });

                let sections = [
                    "Identity",
                    "Cohort",
                    "Collection",
                    "Specimen",
                    "Covariates",
                    "Provenance",
                    "Batch",
                    "Modality mapping",
                    "Review",
                ];
                for section in sections {
                    egui::CollapsingHeader::new(section)
                        .default_open(matches!(
                            section,
                            "Identity" | "Cohort" | "Modality mapping"
                        ))
                        .show(ui, |ui| {
                            let fields = STANDARD_METADATA_FIELDS
                                .iter()
                                .filter(|field| field.section == section)
                                .copied()
                                .collect::<Vec<_>>();
                            for field in fields {
                                let idx = this
                                    .metadata_builder
                                    .as_ref()
                                    .and_then(|m| m.column_index(field.name));
                                let Some(idx) = idx else {
                                    continue;
                                };
                                let columns_len = this
                                    .metadata_builder
                                    .as_ref()
                                    .map(|m| m.columns.len())
                                    .unwrap_or(0);
                                if let Some(metadata) = &mut this.metadata_builder {
                                    if this.metadata_builder_selected_row >= metadata.rows.len() {
                                        continue;
                                    }
                                    metadata.rows[this.metadata_builder_selected_row]
                                        .resize(columns_len, String::new());
                                    ui.horizontal(|ui| {
                                        ui.label(format!("{}:", field.name)).on_hover_text(
                                            format!(
                                                "{} — {}",
                                                field.level.label(),
                                                field.description
                                            ),
                                        );
                                        ui.add_sized(
                                            [430.0, 24.0],
                                            egui::TextEdit::singleline(
                                                &mut metadata.rows
                                                    [this.metadata_builder_selected_row][idx],
                                            ),
                                        );
                                    });
                                }
                            }
                        });
                }

                let standard = STANDARD_METADATA_FIELDS
                    .iter()
                    .map(|f| f.name)
                    .collect::<HashSet<_>>();
                let extra = this
                    .metadata_builder
                    .as_ref()
                    .map(|m| {
                        m.columns
                            .iter()
                            .filter(|c| !standard.contains(c.as_str()))
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !extra.is_empty() {
                    egui::CollapsingHeader::new("Additional loaded columns").show(ui, |ui| {
                        for column in extra {
                            let idx = this
                                .metadata_builder
                                .as_ref()
                                .and_then(|m| m.column_index(&column));
                            let Some(idx) = idx else {
                                continue;
                            };
                            let columns_len = this
                                .metadata_builder
                                .as_ref()
                                .map(|m| m.columns.len())
                                .unwrap_or(0);
                            if let Some(metadata) = &mut this.metadata_builder {
                                metadata.rows[this.metadata_builder_selected_row]
                                    .resize(columns_len, String::new());
                                ui.horizontal(|ui| {
                                    ui.label(format!("{column}:"));
                                    ui.add_sized(
                                        [430.0, 24.0],
                                        egui::TextEdit::singleline(
                                            &mut metadata.rows[this.metadata_builder_selected_row]
                                                [idx],
                                        ),
                                    );
                                });
                            }
                        }
                    });
                }
            }
        });

        self.card(ui, "4. Save / use metadata", |this, ui| {
            ui.label("Saving writes a plain UTF-8 TSV. The metadata is cohort definition/provenance; automatic cross-modal QC decisions remain frozen separately for each analysis run.");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save standardized TSV…").clicked() {
                    this.save_metadata_builder(false);
                }
                if ui.button("Save TSV & use in Cohort Analysis").clicked() {
                    this.save_metadata_builder(true);
                }
            });
        });
    }

    fn save_metadata_builder(&mut self, use_in_cohort: bool) {
        let Some(metadata) = self.metadata_builder.clone() else {
            return;
        };
        let report = validate_standard_metadata(&metadata);
        if !report.is_valid() {
            self.metadata_builder_status = "Fix blocking sample-ID validation errors before exporting the standardized metadata TSV.".to_string();
            return;
        }
        let mut dialog = FileDialog::new()
            .add_filter("TSV", &["tsv"])
            .set_file_name("4C_multiomics_sample_metadata.tsv");
        if let Some(dir) = &self.last_directory {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        match write_metadata_tsv(&metadata, &path) {
            Ok(()) => {
                self.last_directory = path.parent().map(Path::to_path_buf);
                self.metadata_builder_path = Some(path.clone());
                if let Some(builder) = &mut self.metadata_builder {
                    builder.path = path.clone();
                }
                self.metadata_builder_status =
                    format!("Saved standardized metadata to {}.", path.display());
                if use_in_cohort {
                    let mut active = metadata.clone();
                    active.path = path.clone();
                    self.cohort_metadata_source = Some(MetadataLocator::local(path.clone()));
                    self.cohort_metadata_path = Some(path);
                    self.cohort_metadata = Some(active.clone());
                    self.cohort_mapping = self
                        .cohort_scan
                        .as_ref()
                        .and_then(|scan| self.analysis_backend.build_mapping(scan, &active).ok());
                    self.cohort_use_filtered_metadata = false;
                    self.unified_results = None;
                    self.outlier_results = None;
                    self.outlier_excluded_ids.clear();
                    self.active_tab = AppTab::Cohort;
                    self.cohort_status = "Standardized metadata saved and loaded as the primary Cohort Analysis metadata table.".to_string();
                }
            }
            Err(error) => {
                self.metadata_builder_status = format!("Could not save metadata TSV: {error:#}");
            }
        }
    }

    fn render_merge_tab(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            self.render_input_card(&mut columns[0]);
            self.render_output_card(&mut columns[1]);
        });

        ui.add_space(10.0);
        self.render_merge_status_card(ui);
    }

    fn render_input_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "1. Input samples", |this, ui| {
            ui.label(egui::RichText::new("Select the per-sample Excel expression reports.").weak());
            ui.horizontal_wrapped(|ui| {
                let choose_files = ui
                    .add_enabled(!this.running, egui::Button::new("Choose files…"))
                    .on_hover_text("Select one or more .xlsx expression reports. The dialog reopens in the last directory you used.");
                if choose_files.clicked() {
                    let mut dialog = FileDialog::new().add_filter("Excel workbook", &["xlsx"]);
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(mut files) = dialog.pick_files() {
                        files.sort();
                        if let Some(first) = files.first() {
                            this.remember_directory_for(first);
                        }
                        this.set_files(files);
                    }
                }

                let choose_folder = ui
                    .add_enabled(!this.running, egui::Button::new("Choose folder…"))
                    .on_hover_text("Select a folder containing .xlsx reports. The last successfully used directory is remembered across app launches.");
                if choose_folder.clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(folder) = dialog.pick_folder() {
                        this.remember_directory_for(&folder);
                        match cfs_expression_merger::discover_xlsx_files(&folder) {
                            Ok(files) => this.set_files(files),
                            Err(e) => this.status = format!("Error: {e:#}"),
                        }
                    }
                }

                let clear = ui
                    .add_enabled(!this.running && !this.files.is_empty(), egui::Button::new("Clear"))
                    .on_hover_text("Remove all selected workbooks. A separately loaded merged TSV remains available in Explore & QC.");
                if clear.clicked() {
                    this.set_files(Vec::new());
                }
            });

            ui.add_space(8.0);
            let hovering_files = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
            let drop_fill = if hovering_files {
                egui::Color32::from_rgb(31, 65, 88)
            } else {
                ui.visuals().extreme_bg_color
            };
            egui::Frame::group(ui.style())
                .fill(drop_fill)
                .inner_margin(12)
                .corner_radius(7)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Drop files or a folder here").strong());
                        ui.label(
                            egui::RichText::new(
                                "Drop multiple .xlsx files, a folder containing them, or a merged .tsv for QC.",
                            )
                            .small()
                            .weak(),
                        );
                    });
                });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Path:")
                    .on_hover_text("Paste a path to one .xlsx file, a folder of workbooks, or a previously merged .tsv file.");
                let path_edit = ui.add(
                    egui::TextEdit::singleline(&mut this.input_path_text)
                        .hint_text("/path/to/folder or merged_expression.tsv")
                        .desired_width(360.0),
                );
                path_edit.on_hover_text("Paste a file/folder path here, then choose Add path. Quotation marks around a copied path are accepted.");
                let add_path = ui
                    .add_enabled(!this.running && !this.input_path_text.trim().is_empty(), egui::Button::new("Add path"))
                    .on_hover_text("Add the pasted XLSX/folder to the input set, or open a TSV directly in Explore & QC.");
                if add_path.clicked() {
                    let text = this.input_path_text.trim().trim_matches(|c| c == '\'' || c == '"').to_string();
                    this.input_path_text.clear();
                    this.handle_input_path(PathBuf::from(text));
                }
            });

            if let Some(first) = this.files.first() {
                ui.add_space(4.0);
                ui.label(format!(
                    "First workbook: {}",
                    first.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            if let Some(dir) = &this.last_directory {
                ui.label(
                    egui::RichText::new(format!("Last directory: {}", dir.display()))
                        .small()
                        .weak(),
                )
                .on_hover_text("This location is persisted and used as the starting folder for future file/folder dialogs.");
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Optional sample metadata filter").strong())
                .on_hover_text("Load a sample metadata table, match workbook sample IDs to a chosen metadata ID column, then optionally include only selected values from another metadata column in the merge.");
            ui.label(
                egui::RichText::new(
                    "This is merge-specific: loading metadata does not change Explore & QC. Filtering is off by default, so simply loading a metadata file never removes samples.",
                )
                .small()
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                let choose_metadata = ui
                    .add_enabled(!this.running, egui::Button::new("Choose sample metadata…"))
                    .on_hover_text("Load XLSX, CSV, TSV, or TXT sample metadata. The app will suggest 4C_ID/sample_id as the matching column when available.");
                if choose_metadata.clicked() {
                    let mut dialog = FileDialog::new().add_filter(
                        "Sample metadata",
                        &["xlsx", "xls", "csv", "tsv", "txt"],
                    );
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(path) = dialog.pick_file() {
                        this.load_merge_metadata_path(path);
                    }
                }
                let clear_metadata = ui
                    .add_enabled(!this.running && this.merge_metadata.is_some(), egui::Button::new("Clear metadata"))
                    .on_hover_text("Remove the metadata/filter configuration. The selected expression workbooks themselves are not changed.");
                if clear_metadata.clicked() {
                    this.clear_merge_metadata();
                }
            });

            if let Some(path) = &this.merge_metadata_path {
                ui.label(
                    egui::RichText::new(format!("Metadata: {}", path.display()))
                        .small()
                        .weak(),
                );
            }

            if this.merge_metadata.is_some() {
                let (columns, current_id) = {
                    let meta = this.merge_metadata.as_ref().unwrap();
                    (meta.columns.clone(), meta.id_column.clone())
                };
                let mut selected_id = current_id.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Sample ID column:")
                        .on_hover_text("This column is normalized and matched to the sample prefix parsed from each expression-report filename.");
                    egui::ComboBox::from_id_salt("merge_metadata_id_column_v013")
                        .selected_text(selected_id.clone())
                        .show_ui(ui, |ui| {
                            for column in &columns {
                                ui.selectable_value(&mut selected_id, column.clone(), column);
                            }
                        });
                });
                if selected_id != current_id {
                    if let Some(meta) = &mut this.merge_metadata {
                        meta.id_column = selected_id.clone();
                    }
                    if this.merge_metadata_filter_column.as_deref() == Some(selected_id.as_str()) {
                        let replacement = columns
                            .iter()
                            .find(|column| column.as_str() != selected_id.as_str())
                            .cloned();
                        this.set_merge_filter_column(replacement);
                    }
                }

                let current_id = this
                    .merge_metadata
                    .as_ref()
                    .map(|meta| meta.id_column.clone())
                    .unwrap_or_default();
                let filter_columns = columns
                    .iter()
                    .filter(|column| column.as_str() != current_id.as_str())
                    .cloned()
                    .collect::<Vec<_>>();
                let mut selected_filter = this.merge_metadata_filter_column.clone();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Optional value filter:")
                        .on_hover_text("Leave this unset to use the metadata rows themselves as the inclusion list. Or choose a metadata field such as Phenotype/cohort/status and retain only selected values.");
                    egui::ComboBox::from_id_salt("merge_metadata_filter_column_v013")
                        .selected_text(selected_filter.clone().unwrap_or_else(|| "(none — all matched metadata rows)".to_string()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut selected_filter, None, "(none — all matched metadata rows)");
                            for column in &filter_columns {
                                ui.selectable_value(&mut selected_filter, Some(column.clone()), column);
                            }
                        });
                });
                if selected_filter != this.merge_metadata_filter_column {
                    this.set_merge_filter_column(selected_filter);
                }

                ui.checkbox(
                    &mut this.merge_metadata_filter_enabled,
                    "Use this metadata to filter workbooks included in the merge",
                )
                .on_hover_text("When enabled, only workbooks with one unique metadata match are eligible. If no value-filter column is selected, every matched metadata row is included. This means a pre-filtered metadata file can be used directly as the sample inclusion list. Unmatched or ambiguous files are reported and excluded rather than causing the merge to fail.");

                if this.merge_metadata_filter_column.is_none() {
                    ui.label(
                        egui::RichText::new(
                            "No value filter selected: enabling metadata filtering will merge every uniquely matched workbook represented in this metadata file.",
                        )
                        .small()
                        .weak(),
                    );
                }

                if this.merge_metadata_filter_column.is_some() {
                    let value_counts = this.merge_filter_value_counts();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Include values:").strong());
                        let all = ui.button("All").on_hover_text("Include every value in the selected metadata column.");
                        if all.clicked() {
                            this.merge_metadata_filter_values = value_counts.keys().cloned().collect();
                        }
                        let clear = ui.button("Clear").on_hover_text("Deselect every metadata value. If filtering is enabled, no workbooks will be eligible until at least one value is checked.");
                        if clear.clicked() {
                            this.merge_metadata_filter_values.clear();
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("merge_metadata_filter_values_v013")
                        .max_height(135.0)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (value, count) in &value_counts {
                                    let mut enabled = this.merge_metadata_filter_values.contains(value);
                                    let label = if value.trim().is_empty() {
                                        format!("(blank) ({count})")
                                    } else {
                                        format!("{value} ({count})")
                                    };
                                    if ui.checkbox(&mut enabled, label).changed() {
                                        if enabled {
                                            this.merge_metadata_filter_values.insert(value.clone());
                                        } else {
                                            this.merge_metadata_filter_values.remove(value);
                                        }
                                    }
                                }
                            });
                        });
                }

                let statuses = this.merge_metadata_file_statuses();
                let matched = statuses
                    .iter()
                    .filter(|row| row.status == MergeMetadataMatchStatus::Matched)
                    .count();
                let unmatched = statuses
                    .iter()
                    .filter(|row| row.status == MergeMetadataMatchStatus::Unmatched)
                    .count();
                let ambiguous = statuses
                    .iter()
                    .filter(|row| row.status == MergeMetadataMatchStatus::Ambiguous)
                    .count();
                let included = statuses.iter().filter(|row| row.included).count();
                let metadata_only = this.merge_metadata_only_count();
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Matched: {matched}"));
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 190, 90),
                        format!("Unmatched: {unmatched}"),
                    );
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::from_rgb(225, 120, 105),
                        format!("Ambiguous: {ambiguous}"),
                    );
                    ui.separator();
                    ui.label(format!("Metadata-only: {metadata_only}"))
                        .on_hover_text("Metadata rows with one unique sample ID but no selected expression workbook. They are reported for audit purposes and do not create missing columns in the merged output.");
                    if this.merge_metadata_filter_enabled {
                        ui.separator();
                        ui.label(egui::RichText::new(format!("Will merge: {included} / {}", this.files.len())).strong());
                    }
                });
                if this.merge_metadata_filter_enabled && included == 0 && !this.files.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 190, 90),
                        "Metadata filtering currently excludes every selected workbook. Check the ID match, filter column, and included values.",
                    );
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Filter preview:")
                    .on_hover_text("Filter the sample preview by sample prefix or source workbook name.");
                ui.add(
                    egui::TextEdit::singleline(&mut this.sample_filter)
                        .hint_text("e.g. 4C_101")
                        .desired_width(180.0),
                )
                .on_hover_text("Type part of a sample prefix or workbook filename to filter the preview table.");
            });

            if this.files.is_empty() {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("No workbooks selected yet.").weak());
                return;
            }

            let filter = this.sample_filter.trim().to_ascii_lowercase();
            let metadata_loaded = this.merge_metadata.is_some();
            let filter_column_label = this
                .merge_metadata_filter_column
                .clone()
                .unwrap_or_else(|| "Filter value".to_string());
            let statuses = this.merge_metadata_file_statuses();
            let included_total = statuses.iter().filter(|row| row.included).count();
            let matching: Vec<(usize, MergeMetadataFileStatus)> = statuses
                .into_iter()
                .enumerate()
                .filter(|(_, row)| {
                    if filter.is_empty() {
                        true
                    } else {
                        row.sample_id.to_ascii_lowercase().contains(&filter)
                            || row
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_ascii_lowercase()
                                .contains(&filter)
                            || row.filter_value.to_ascii_lowercase().contains(&filter)
                    }
                })
                .take(250)
                .collect();

            ui.add_space(6.0);
            let preview_title = if this.merge_metadata_filter_enabled && metadata_loaded {
                format!(
                    "Sample preview ({} selected; {included_total} included in merge)",
                    this.files.len()
                )
            } else {
                format!("Sample preview ({})", this.files.len())
            };
            ui.label(egui::RichText::new(preview_title).strong());
            let table_width = if metadata_loaded { 1220.0 } else { 820.0 };
            fixed_table_viewport(ui, "sample_preview_scroll_v013", 360.0, table_width, |ui| {
                egui::Grid::new("sample_preview_grid_v013")
                    .striped(true)
                    .num_columns(if metadata_loaded { 6 } else { 3 })
                    .min_col_width(50.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("#").strong());
                        ui.label(egui::RichText::new("Sample ID").strong());
                        ui.label(egui::RichText::new("Source workbook").strong());
                        if metadata_loaded {
                            ui.label(egui::RichText::new("Metadata match").strong());
                            ui.label(egui::RichText::new(&filter_column_label).strong());
                            ui.label(egui::RichText::new("Merge").strong());
                        }
                        ui.end_row();

                        for (idx, row) in matching {
                            ui.label((idx + 1).to_string());
                            ui.monospace(&row.sample_id);
                            ui.label(row.path.file_name().unwrap_or_default().to_string_lossy());
                            if metadata_loaded {
                                match row.status {
                                    MergeMetadataMatchStatus::Matched => {
                                        let row_label = row
                                            .metadata_row
                                            .map(|r| format!("matched (row {})", r + 2))
                                            .unwrap_or_else(|| "matched".to_string());
                                        ui.colored_label(
                                            egui::Color32::from_rgb(120, 200, 145),
                                            row_label,
                                        );
                                    }
                                    MergeMetadataMatchStatus::Unmatched => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(230, 190, 90),
                                            "unmatched",
                                        );
                                    }
                                    MergeMetadataMatchStatus::Ambiguous => {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(225, 120, 105),
                                            "ambiguous",
                                        );
                                    }
                                }
                                let filter_display = if row.filter_value.trim().is_empty() {
                                    "—".to_string()
                                } else {
                                    row.filter_value.clone()
                                };
                                ui.label(filter_display);
                                if row.included {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(120, 200, 145),
                                        "include",
                                    );
                                } else {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(225, 145, 100),
                                        "exclude",
                                    );
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        });
    }

    fn render_output_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "2. Output configuration", |this, ui| {
            ui.horizontal(|ui| {
                ui.label("Report type:")
                    .on_hover_text("Auto detects gene vs transcript reports from the first workbook. Override only if you want strict type validation.");
                let kind_before = this.kind;
                let combo = egui::ComboBox::from_id_salt("kind_v03")
                    .selected_text(format!("{:?}", this.kind))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut this.kind, DataKind::Auto, "Auto");
                        ui.selectable_value(&mut this.kind, DataKind::Gene, "Gene");
                        ui.selectable_value(&mut this.kind, DataKind::Transcript, "Transcript");
                    });
                combo.response.on_hover_text(
                    "Auto is recommended. Gene reports contain RNA-Seq Raw Counts; transcript reports contain Transcript Name and Total Reads.",
                );
                if kind_before != this.kind {
                    this.metrics = collaborator_default_metrics(this.effective_kind());
                    this.ensure_metrics_valid();
                    if this.analysis_source == AnalysisSource::Workbooks {
                        this.reset_analysis_results();
                        this.ensure_analysis_metric_valid();
                        if this.active_tab == AppTab::Explore
                            && !this.files.is_empty()
                            && !this.analysis_running
                        {
                            this.start_catalog_load();
                        }
                    }
                }
                if this.detected_kind != DataKind::Auto {
                    ui.label(format!("Detected: {:?}", this.detected_kind));
                }
            });

            this.ensure_metrics_valid();
            let effective_kind = this.effective_kind();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Measurements / value suffixes").strong())
                    .on_hover_text("Each checked measurement becomes one output column per sample.");
                let default_button = ui
                    .button("Default")
                    .on_hover_text("Recommended collaborator layout: Raw Count + TPM for gene reports, or Total Reads + TPM for transcript reports.");
                if default_button.clicked() {
                    this.metrics = collaborator_default_metrics(effective_kind);
                }
                let all_button = ui
                    .button("All")
                    .on_hover_text("Select every measurement available for this report type.");
                if all_button.clicked() {
                    this.metrics = all_metrics(effective_kind);
                }
                let clear_button = ui
                    .button("Clear")
                    .on_hover_text("Deselect all measurement suffixes. At least one must be checked before merging.");
                if clear_button.clicked() {
                    this.metrics.clear();
                }
            });

            egui::Frame::group(ui.style())
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for metric in all_metrics(effective_kind) {
                            let mut enabled = this.metrics.contains(&metric);
                            let response = ui
                                .checkbox(&mut enabled, metric.display_name())
                                .on_hover_text(metric_help(metric));
                            if response.changed() {
                                if enabled {
                                    if !this.metrics.contains(&metric) {
                                        this.metrics.push(metric);
                                    }
                                } else {
                                    this.metrics.retain(|m| *m != metric);
                                }
                                this.sort_metrics();
                            }
                        }
                    });
                });

            ui.label(format!("{} measurement(s) selected", this.selected_metrics().len()));
            ui.label(
                egui::RichText::new("Tip: hover controls for a short explanation of what they do.")
                    .small()
                    .weak(),
            );

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Column order").strong());
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(
                    &mut this.order,
                    ColumnOrder::AdjacentBySample,
                    "By sample prefix",
                )
                .on_hover_text("Keep all selected measurements adjacent for each sample: Sample A count, Sample A TPM, Sample B count, Sample B TPM, …");
                ui.radio_value(
                    &mut this.order,
                    ColumnOrder::GroupedByMeasurement,
                    "By value suffix",
                )
                .on_hover_text("Group the same measurement across samples: all count columns first, then all TPM columns, etc.");
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Output column preview").strong());
            egui::ScrollArea::horizontal()
                .max_height(58.0)
                .show(ui, |ui| {
                    ui.monospace(this.output_column_preview(22));
                });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                let choose_output = ui
                    .add_enabled(!this.running, egui::Button::new("Choose output…"))
                    .on_hover_text("Choose the destination TSV file. TSV is preferred for very wide expression matrices and is streamed to disk.");
                if choose_output.clicked() {
                    let mut dialog = FileDialog::new()
                        .add_filter("TSV", &["tsv"])
                        .set_file_name(this.default_output_name());
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(path) = dialog.save_file() {
                        this.remember_directory_for(&path);
                        this.output = Some(path);
                    }
                }
                if let Some(path) = &this.output {
                    ui.label(path.display().to_string());
                } else {
                    ui.label(egui::RichText::new("No output selected").weak());
                }
            });

            ui.add_space(8.0);
            let merge_input_count = this.merge_input_files().len();
            let ready = !this.running
                && merge_input_count > 0
                && this.output.is_some()
                && !this.selected_metrics().is_empty();
            let merge = ui
                .add_enabled(
                    ready,
                    egui::Button::new(egui::RichText::new("Merge datasets").strong())
                        .min_size(egui::vec2(150.0, 34.0)),
                )
                .on_hover_text(format!("Start the bounded-memory merge using {merge_input_count} included sample workbook(s). Workbooks are processed in chunks and the final TSV is streamed to disk."))
                .on_disabled_hover_text("Select input files, at least one measurement, an output path, and make sure the optional metadata filter includes at least one matched workbook.");
            if merge.clicked() {
                this.start_merge();
            }
        });
    }

    fn render_merge_status_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. Merge status", |this, ui| {
            if this.running {
                ui.add(
                    egui::ProgressBar::new(this.progress)
                        .show_percentage()
                        .animate(true),
                );
            }
            ui.label(&this.status);
        });
    }

    fn render_explore_tab(&mut self, ui: &mut egui::Ui) {
        self.render_analysis_source_card(ui);
        ui.add_space(10.0);

        if !self.analysis_source_available() {
            self.card(ui, "Explore & QC", |_this, ui| {
                ui.heading("No analysis data loaded");
                ui.label("Select Excel workbooks on Merge & Export, or open a previously merged TSV above.");
            });
            return;
        }

        self.ensure_analysis_metric_valid();
        self.render_analysis_controls(ui);
        ui.add_space(10.0);

        // Keep sample-level QC and identifier exploration in separate workspaces.
        // Both can be data-heavy/tall, and stacking them caused the identifier
        // browser to be pushed below the viewport after QC plots were built.
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(8)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Workspace:").strong());
                    ui.selectable_value(
                        &mut self.explore_panel,
                        ExplorePanel::SampleQc,
                        "Sample QC",
                    )
                    .on_hover_text(
                        "Inspect sample-wide distributions, detection rates, and abundance-vs-detection QC plots.",
                    );
                    ui.selectable_value(
                        &mut self.explore_panel,
                        ExplorePanel::EntityExplorer,
                        "Gene / transcript explorer",
                    )
                    .on_hover_text(
                        "Browse genes/transcripts and plot one selected identifier across all samples. This view keeps the tall browser and profile side by side.",
                    );
                });
            });

        ui.add_space(10.0);
        match self.explore_panel {
            ExplorePanel::SampleQc => self.render_distribution_card(ui),
            ExplorePanel::EntityExplorer => self.render_entity_profile_card(ui),
        }
    }

    fn render_analysis_source_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Analysis data source", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Source:").strong());
                let workbook_enabled = !this.files.is_empty() && !this.analysis_running;
                let workbook_response = ui
                    .add_enabled_ui(workbook_enabled, |ui| {
                        ui.selectable_label(
                            this.analysis_source == AnalysisSource::Workbooks,
                            format!("Selected workbooks ({})", this.files.len()),
                        )
                    })
                    .inner
                    .on_hover_text("Use the original Excel reports for QC. This is the fallback when no merged TSV is available, but repeated analyses must reopen the XLSX files.")
                    .on_disabled_hover_text("No workbooks are selected, or an analysis task is currently running.");
                if workbook_response.clicked() && this.analysis_source != AnalysisSource::Workbooks {
                    this.analysis_source = AnalysisSource::Workbooks;
                    this.reset_analysis_results();
                    this.ensure_analysis_metric_valid();
                    this.start_catalog_load();
                }

                let merged_sample_count = this.merged_info.as_ref().map(|info| info.samples.len());
                if let Some(sample_count) = merged_sample_count {
                    let merged_response = ui
                        .add_enabled_ui(!this.analysis_running, |ui| {
                            ui.selectable_label(
                                this.analysis_source == AnalysisSource::MergedTsv,
                                format!("Merged TSV ({sample_count} samples)"),
                            )
                        })
                        .inner
                        .on_hover_text("Use the already merged TSV for QC and profiles. This avoids reopening every Excel workbook and is recommended whenever the merged file exists.")
                        .on_disabled_hover_text("Wait for the current analysis task to finish before switching data sources.");
                    if merged_response.clicked() && this.analysis_source != AnalysisSource::MergedTsv {
                        this.analysis_source = AnalysisSource::MergedTsv;
                        this.reset_analysis_results();
                        this.ensure_analysis_metric_valid();
                        this.start_catalog_load();
                    }
                }

                let open_merged = ui
                    .add_enabled(!this.analysis_running, egui::Button::new("Open merged TSV…"))
                    .on_hover_text("Open a merged gene/transcript TSV directly. You can use Explore & QC without selecting or merging the original Excel reports.");
                if open_merged.clicked() {
                    let mut dialog = FileDialog::new().add_filter("Merged TSV", &["tsv"]);
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(path) = dialog.pick_file() {
                        this.remember_directory_for(&path);
                        this.set_merged_analysis_file(path, true);
                    }
                }
            });

            ui.label(
                egui::RichText::new("Tip: you can also drop a merged .tsv anywhere on the window to open it here.")
                    .small()
                    .weak(),
            );
            ui.add_space(4.0);
            match this.analysis_source {
                AnalysisSource::Workbooks => {
                    if this.files.is_empty() {
                        ui.label(egui::RichText::new("No workbooks are currently selected.").weak());
                    } else {
                        ui.label(format!(
                            "Using {} Excel workbooks. QC operations read each workbook independently.",
                            this.files.len()
                        ));
                    }
                }
                AnalysisSource::MergedTsv => {
                    if let (Some(path), Some(info)) = (&this.merged_analysis_path, &this.merged_info) {
                        let metrics = info
                            .metrics
                            .iter()
                            .map(|m| m.display_name())
                            .collect::<Vec<_>>()
                            .join(", ");
                        ui.label(format!("Using merged file: {}", path.display()));
                        ui.label(
                            egui::RichText::new(format!(
                                "Detected {:?} data • {} samples • available values: {}",
                                info.kind,
                                info.samples.len(),
                                metrics
                            ))
                            .small()
                            .weak(),
                        );
                    }
                }
            }
        });
    }

    fn render_analysis_controls(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Analysis controls", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Measurement:")
                    .on_hover_text("Choose the numeric value suffix used for QC distributions and gene/transcript profiles. When using a merged TSV, only measurements present in that file are offered.");
                let before = this.analysis_metric;
                let available = this.analysis_available_metrics();
                let combo = egui::ComboBox::from_id_salt("analysis_metric_v03")
                    .selected_text(this.analysis_metric.display_name())
                    .show_ui(ui, |ui| {
                        for metric in available {
                            ui.selectable_value(
                                &mut this.analysis_metric,
                                metric,
                                metric.display_name(),
                            );
                        }
                    });
                combo.response.on_hover_text(
                    "Only numeric measurements available from the current analysis source are shown.",
                );
                if before != this.analysis_metric {
                    this.distributions.clear();
                    this.profile.clear();
                }

                let zero_response = ui
                    .checkbox(&mut this.analysis_ignore_zeros, "Ignore zeros for box plots")
                    .on_hover_text("Exclude exact zeros when calculating quartiles/whiskers and means. Detection-rate statistics still use all observed values.");
                if zero_response.changed() {
                    this.distributions.clear();
                }
                ui.checkbox(&mut this.analysis_log1p, "Plot log10(value + 1)")
                    .on_hover_text("Apply log10(value + 1) for display only. Useful for raw counts, TPM, FPKM, and CPM with long right tails.");
            });

            if this.analysis_running {
                ui.add(
                    egui::ProgressBar::new(this.analysis_progress)
                        .show_percentage()
                        .animate(true),
                );
            }
            ui.label(&this.analysis_status);
        });
    }

    fn render_distribution_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Sample QC", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                let build_label = if this.analysis_source == AnalysisSource::MergedTsv {
                    "Build / refresh QC"
                } else {
                    "Build sample QC"
                };
                let build = ui
                    .add_enabled(!this.analysis_running, egui::Button::new(build_label))
                    .on_hover_text(if this.analysis_source == AnalysisSource::MergedTsv {
                        "Calculate sample QC directly from the merged TSV. The file is processed in bounded-memory sample chunks instead of reopening the original XLSX files."
                    } else {
                        "Read the selected measurement from each workbook, one sample at a time, and calculate sample QC statistics."
                    });
                if build.clicked() {
                    this.start_distribution_analysis();
                }

                ui.separator();
                ui.label("View:")
                    .on_hover_text("Switch between complementary sample-level QC visualizations without recalculating the data.");
                ui.selectable_value(&mut this.sample_qc_view, SampleQcView::BoxPlots, "Distributions")
                    .on_hover_text("Box plots of the selected measurement across genes/transcripts within each sample.");
                ui.selectable_value(&mut this.sample_qc_view, SampleQcView::DetectionRate, "Detection rate")
                    .on_hover_text("Percentage of observed values that are non-zero for each sample. Useful for spotting low-complexity or low-depth samples.");
                ui.selectable_value(&mut this.sample_qc_view, SampleQcView::MedianVsDetection, "Median vs detection")
                    .on_hover_text("Scatter plot of each sample's median abundance versus non-zero detection rate. Outlying samples are often visually obvious here.");
            });

            if this.distributions.is_empty() {
                ui.add_space(12.0);
                ui.label("Build sample QC once, then switch among the available QC views.");
                return;
            }

            match this.sample_qc_view {
                SampleQcView::BoxPlots => this.render_boxplot_qc(ui),
                SampleQcView::DetectionRate => this.render_detection_qc(ui),
                SampleQcView::MedianVsDetection => this.render_median_detection_qc(ui),
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("QC summary").strong());
            fixed_table_viewport(ui, "distribution_summary_scroll_v08", 360.0, 760.0, |ui| {
                    egui::Grid::new("distribution_summary_grid_v08")
                        .striped(true)
                        .num_columns(6)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Sample").strong());
                            ui.label(egui::RichText::new("N plotted").strong());
                            ui.label(egui::RichText::new("Median").strong());
                            ui.label(egui::RichText::new("Mean").strong());
                            ui.label(egui::RichText::new("Non-zero").strong());
                            ui.label(egui::RichText::new("Missing").strong());
                            ui.end_row();
                            for summary in &this.distributions {
                                ui.monospace(&summary.sample);
                                ui.label(summary.n_values.to_string());
                                ui.label(format_optional(summary.median));
                                ui.label(format_optional(summary.mean));
                                ui.label(
                                    summary
                                        .nonzero_fraction()
                                        .map(|v| format!("{:.1}%", 100.0 * v))
                                        .unwrap_or_else(|| "NA".to_string()),
                                );
                                ui.label(summary.n_missing.to_string());
                                ui.end_row();
                            }
                        });
                });
        });
    }

    fn render_boxplot_qc(&self, ui: &mut egui::Ui) {
        let metric_name = self.analysis_metric.display_name().to_string();
        let log1p = self.analysis_log1p;
        let sample_names: Vec<String> = self
            .distributions
            .iter()
            .map(|summary| summary.sample.clone())
            .collect();
        let accent = ui.visuals().selection.bg_fill;
        let fill = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 72);
        let outline = egui::Color32::from_gray(175);
        let boxes: Vec<BoxElem> = self
            .distributions
            .iter()
            .enumerate()
            .filter_map(|(idx, summary)| {
                let (Some(low), Some(q1), Some(median), Some(q3), Some(high)) = (
                    summary.lower_whisker,
                    summary.q1,
                    summary.median,
                    summary.q3,
                    summary.upper_whisker,
                ) else {
                    return None;
                };
                Some(
                    BoxElem::new(
                        idx as f64,
                        BoxSpread::new(
                            plot_transform(low, log1p),
                            plot_transform(q1, log1p),
                            plot_transform(median, log1p),
                            plot_transform(q3, log1p),
                            plot_transform(high, log1p),
                        ),
                    )
                    .name(summary.sample.clone())
                    .box_width(0.65)
                    .fill(fill)
                    .stroke(egui::Stroke::new(1.3, outline)),
                )
            })
            .collect();

        let axis_samples = sample_names.clone();
        let hover_metric = metric_name.clone();
        let box_plot = BoxPlot::new("sample distributions", boxes).element_formatter(Box::new(
            move |element, _plot| {
                format!(
                    "{}\n{}\nQ1: {:.4}\nMedian: {:.4}\nQ3: {:.4}\nWhiskers: {:.4} – {:.4}",
                    element.name,
                    hover_metric,
                    element.spread.quartile1,
                    element.spread.median,
                    element.spread.quartile3,
                    element.spread.lower_whisker,
                    element.spread.upper_whisker,
                )
            },
        ));

        Plot::new("sample_distribution_plot_v03")
            .height(350.0)
            .x_axis_label("Sample")
            .y_axis_label(self.plot_axis_label())
            .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_samples))
            .show(ui, |plot_ui| {
                plot_ui.box_plot(box_plot);
                for (idx, summary) in self.distributions.iter().enumerate() {
                    if let Some(median) = summary.median {
                        plot_ui.points(
                            Points::new(
                                format!(
                                    "{}\nMedian {}: {}",
                                    summary.sample,
                                    self.analysis_metric.display_name(),
                                    format_value(median)
                                ),
                                vec![[idx as f64, plot_transform(median, log1p)]],
                            )
                            .color(egui::Color32::WHITE)
                            .radius(3.2),
                        );
                    }
                    if let Some(mean) = summary.mean {
                        plot_ui.points(
                            Points::new(
                                format!(
                                    "{}\nMean {}: {}",
                                    summary.sample,
                                    self.analysis_metric.display_name(),
                                    format_value(mean)
                                ),
                                vec![[idx as f64, plot_transform(mean, log1p)]],
                            )
                            .color(egui::Color32::from_rgb(255, 190, 70))
                            .radius(2.8),
                        );
                    }
                }
            });
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("● Median").color(egui::Color32::WHITE));
            ui.label(egui::RichText::new("● Mean").color(egui::Color32::from_rgb(255, 190, 70)));
            ui.label(
                egui::RichText::new(
                    "The contrasting markers make the center of each box easier to compare.",
                )
                .small()
                .weak(),
            );
        });
    }

    fn render_detection_qc(&self, ui: &mut egui::Ui) {
        let sample_names: Vec<String> = self
            .distributions
            .iter()
            .map(|s| s.sample.clone())
            .collect();
        let line_points: Vec<[f64; 2]> = self
            .distributions
            .iter()
            .enumerate()
            .filter_map(|(idx, summary)| {
                summary.nonzero_fraction().map(|v| [idx as f64, 100.0 * v])
            })
            .collect();
        let axis_samples = sample_names.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("sample_detection_plot_v03")
            .height(320.0)
            .x_axis_label("Sample")
            .y_axis_label("Non-zero values (%)")
            .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_samples))
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                if line_points.len() > 1 {
                    plot_ui.line(
                        Line::new("Detection trend", line_points)
                            .color(accent)
                            .width(1.3)
                            .allow_hover(false),
                    );
                }
                for (idx, summary) in self.distributions.iter().enumerate() {
                    let Some(frac) = summary.nonzero_fraction() else {
                        continue;
                    };
                    let label = format!(
                        "{}\nNon-zero: {:.2}%\nMissing: {}\nMedian: {}",
                        summary.sample,
                        100.0 * frac,
                        summary.n_missing,
                        format_optional(summary.median),
                    );
                    plot_ui.points(
                        Points::new(label, vec![[idx as f64, 100.0 * frac]])
                            .color(accent)
                            .radius(4.0),
                    );
                }
            });
    }

    fn render_median_detection_qc(&self, ui: &mut egui::Ui) {
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("median_detection_plot_v03")
            .height(330.0)
            .x_axis_label("Non-zero values (%)")
            .y_axis_label(self.plot_axis_label())
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                for summary in &self.distributions {
                    let (Some(frac), Some(median)) = (summary.nonzero_fraction(), summary.median)
                    else {
                        continue;
                    };
                    let label = format!(
                        "{}\nNon-zero: {:.2}%\nMedian {}: {}\nMissing: {}",
                        summary.sample,
                        100.0 * frac,
                        self.analysis_metric.display_name(),
                        format_value(median),
                        summary.n_missing,
                    );
                    plot_ui.points(
                        Points::new(
                            label,
                            vec![[100.0 * frac, plot_transform(median, self.analysis_log1p)]],
                        )
                        .color(accent)
                        .radius(4.5),
                    );
                }
            });
        ui.label(
            egui::RichText::new(
                "Samples separated from the main cloud can indicate lower detection, shifted abundance, or other QC differences worth inspecting.",
            )
            .small()
            .weak(),
        );
    }

    fn render_entity_profile_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Gene / transcript browser & profile", |this, ui| {
            ui.columns(2, |columns| {
                let (left_columns, right_columns) = columns.split_at_mut(1);
                let left = &mut left_columns[0];
                left.label(egui::RichText::new("Identifier browser").strong());
                left.horizontal_wrapped(|ui| {
                    ui.label("Search:")
                        .on_hover_text("Filter the table below by gene symbol, NCBI Gene ID, or transcript accession/name when available.");
                    let search_hint = match (this.analysis_kind(), this.analysis_source) {
                        (DataKind::Transcript, AnalysisSource::Workbooks) => {
                            "transcript ID, gene symbol, or gene ID"
                        }
                        (DataKind::Transcript, AnalysisSource::MergedTsv) => {
                            "transcript ID or gene ID"
                        }
                        _ => "gene symbol or NCBI Gene ID",
                    };
                    let response = ui
                        .add(
                            egui::TextEdit::singleline(&mut this.entity_search)
                                .hint_text(search_hint)
                                .desired_width(260.0),
                        )
                        .on_hover_text("The table is always available; search simply narrows it. Search is case-insensitive.");
                    if response.changed() {
                        this.rebuild_entity_filter();
                    }
                    let reload = ui
                        .add_enabled(!this.analysis_running, egui::Button::new("Reload index"))
                        .on_hover_text("Reload identifiers from the current analysis source.");
                    if reload.clicked() {
                        this.start_catalog_load();
                    }
                });

                if this.entity_catalog.is_empty() {
                    left.add_space(8.0);
                    left.label("The identifier index is loading or unavailable.");
                } else {
                    left.label(
                        egui::RichText::new(format!(
                            "Showing {} of {} indexed rows",
                            this.entity_visible.len(),
                            this.entity_catalog.len()
                        ))
                        .small()
                        .weak(),
                    );
                    left.separator();

                    let browser_kind = this.analysis_kind();
                    let browser_columns = if browser_kind == DataKind::Transcript { 4 } else { 3 };
                    egui::Grid::new("entity_browser_header_v03")
                        .num_columns(browser_columns)
                        .min_col_width(72.0)
                        .show(left, |ui| {
                            if browser_kind == DataKind::Transcript {
                                ui.label(egui::RichText::new("Transcript ID").strong());
                                ui.label(egui::RichText::new("Gene Symbol").strong());
                            } else {
                                ui.label(egui::RichText::new("Gene Symbol").strong());
                            }
                            ui.label(egui::RichText::new("NCBI Gene ID").strong());
                            ui.label(egui::RichText::new("Action").strong());
                            ui.end_row();
                        });

                    let row_height = left.text_style_height(&egui::TextStyle::Body) + 8.0;
                    let mut selection_changed = false;
                    let mut plot_requested = false;
                    let total_rows = this.entity_visible.len();
                    let catalog = &this.entity_catalog;
                    let visible = &this.entity_visible;
                    let selected = &mut this.selected_entity;
                    // Keep the browser beside the profile, but give it substantially more
                    // vertical room so users can browse many identifiers without constant scrolling.
                    egui::ScrollArea::vertical()
                        .id_salt("entity_browser_scroll_v032")
                        .max_height(650.0)
                        .auto_shrink([false, false])
                        .show_rows(left, row_height, total_rows, |ui, row_range| {
                            egui::Grid::new("entity_browser_rows_v03")
                                .striped(true)
                                .num_columns(browser_columns)
                                .min_col_width(72.0)
                                .show(ui, |ui| {
                                    for visible_row in row_range {
                                        let idx = visible[visible_row];
                                        let entry = &catalog[idx];
                                        let is_selected = *selected == Some(idx);
                                        let select = ui
                                            .selectable_label(is_selected, &entry.primary)
                                            .on_hover_text("Select this row. Then use Plot profile, or the Plot button at right.");
                                        if select.clicked() {
                                            *selected = Some(idx);
                                            selection_changed = true;
                                        }
                                        if browser_kind == DataKind::Transcript {
                                            if entry.gene_symbol.is_empty() {
                                                ui.label(egui::RichText::new("—").weak())
                                                    .on_hover_text("Gene Symbol is not stored in collaborator-style merged transcript TSVs; it remains available when the original workbooks are the analysis source.");
                                            } else {
                                                ui.label(&entry.gene_symbol);
                                            }
                                        }
                                        ui.monospace(&entry.gene_id);
                                        let plot = ui
                                            .small_button("Plot")
                                            .on_hover_text("Select this identifier and immediately load its profile across samples.");
                                        if plot.clicked() {
                                            *selected = Some(idx);
                                            selection_changed = true;
                                            plot_requested = true;
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                    if selection_changed {
                        this.profile.clear();
                    }
                    if plot_requested && !this.analysis_running {
                        this.start_profile_analysis();
                    }
                }

                let right = &mut right_columns[0];
                right.label(egui::RichText::new("Selected identifier profile").strong());
                right.separator();
                let selected_entry = this
                    .selected_entity
                    .and_then(|idx| this.entity_catalog.get(idx))
                    .cloned();

                if let Some(entry) = selected_entry {
                    let selected_label = if this.analysis_kind() == DataKind::Transcript {
                        if entry.gene_symbol.is_empty() {
                            format!("{} (Gene ID {})", entry.primary, entry.gene_id)
                        } else {
                            format!("{} — {} (Gene ID {})", entry.primary, entry.gene_symbol, entry.gene_id)
                        }
                    } else {
                        format!("{} (Gene ID {})", entry.primary, entry.gene_id)
                    };
                    right.monospace(selected_label);
                    right.add_space(5.0);
                    right.horizontal_wrapped(|ui| {
                        let plot = ui
                            .add_enabled(
                                !this.analysis_running,
                                egui::Button::new(egui::RichText::new("Plot profile").strong()),
                            )
                            .on_hover_text(if this.analysis_source == AnalysisSource::MergedTsv {
                                "Read this identifier directly from the merged TSV. This is usually fast because only one row is needed."
                            } else {
                                "Extract this identifier from every selected Excel workbook. This can take longer because each workbook must be opened."
                            });
                        if plot.clicked() {
                            this.start_profile_analysis();
                        }
                        ui.label("View:");
                        ui.selectable_value(&mut this.profile_view, ProfileView::SampleOrder, "Sample order")
                            .on_hover_text("Plot values in the original sample order.");
                        ui.selectable_value(&mut this.profile_view, ProfileView::Ranked, "Ranked")
                            .on_hover_text("Sort samples by expression value. This often makes the expression range and high/low samples easier to see.");
                    });

                    if this.analysis_running {
                        right.spinner();
                    }

                    if this.profile.is_empty() {
                        right.add_space(18.0);
                        right.label(
                            egui::RichText::new("Choose Plot profile to load this gene/transcript across samples.")
                                .weak(),
                        );
                    } else {
                        this.render_profile_plot(right, &entry);
                        right.add_space(6.0);
                        this.render_profile_values_table(right);
                    }
                } else {
                    right.add_space(12.0);
                    right.label(
                        egui::RichText::new(
                            "Select any row in the browser. The table is scrollable even when the search box is empty.",
                        )
                        .weak(),
                    );
                }
            });
        });
    }

    fn render_profile_plot(&self, ui: &mut egui::Ui, entry: &EntityCatalogEntry) {
        let mut plotted: Vec<(String, f64)> = self
            .profile
            .iter()
            .filter_map(|point| point.value.map(|value| (point.sample.clone(), value)))
            .collect();
        if self.profile_view == ProfileView::Ranked {
            plotted.sort_by(|a, b| a.1.total_cmp(&b.1));
        }
        if plotted.is_empty() {
            ui.label("No numeric values were found for this identifier.");
            return;
        }

        let sample_names: Vec<String> = plotted.iter().map(|(sample, _)| sample.clone()).collect();
        let line_points: Vec<[f64; 2]> = plotted
            .iter()
            .enumerate()
            .map(|(idx, (_, value))| [idx as f64, plot_transform(*value, self.analysis_log1p)])
            .collect();
        let accent = ui.visuals().selection.bg_fill;
        let metric_name = self.analysis_metric.display_name().to_string();
        let entity_label = entry.primary.clone();
        let axis_samples = sample_names.clone();
        let profile_view = self.profile_view;

        let mut plot = Plot::new("entity_profile_plot_v03")
            .height(315.0)
            .y_axis_label(self.plot_axis_label())
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            });
        plot = if profile_view == ProfileView::SampleOrder {
            plot.x_axis_label("Sample")
                .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_samples))
        } else {
            plot.x_axis_label("Expression rank (low → high)")
        };

        plot.show(ui, |plot_ui| {
            if line_points.len() > 1 {
                plot_ui.line(
                    Line::new("Profile trend", line_points)
                        .color(accent)
                        .width(1.4)
                        .allow_hover(false),
                );
            }
            for (idx, (sample, raw)) in plotted.iter().enumerate() {
                let label = format!(
                    "{}\nSample: {}\n{}: {}{}",
                    entity_label,
                    sample,
                    metric_name,
                    format_value(*raw),
                    if profile_view == ProfileView::Ranked {
                        format!("\nRank: {}/{}", idx + 1, plotted.len())
                    } else {
                        String::new()
                    }
                );
                plot_ui.points(
                    Points::new(
                        label,
                        vec![[idx as f64, plot_transform(*raw, self.analysis_log1p)]],
                    )
                    .color(accent)
                    .radius(4.2),
                );
            }
        });

        let observed: Vec<f64> = self.profile.iter().filter_map(|p| p.value).collect();
        let missing = self.profile.len().saturating_sub(observed.len());
        if !observed.is_empty() {
            let mean = observed.iter().sum::<f64>() / observed.len() as f64;
            let mut sorted = observed.clone();
            sorted.sort_by(f64::total_cmp);
            let median = if sorted.len() % 2 == 1 {
                sorted[sorted.len() / 2]
            } else {
                (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
            };
            let min = sorted[0];
            let max = *sorted.last().unwrap();
            ui.label(format!(
                "Observed in {}/{} samples • missing {} • median {} • mean {} • range {} – {}",
                observed.len(),
                self.profile.len(),
                missing,
                format_value(median),
                format_value(mean),
                format_value(min),
                format_value(max)
            ));
        }
    }

    fn render_profile_values_table(&self, ui: &mut egui::Ui) {
        ui.collapsing("Sample values", |ui| {
            fixed_table_viewport(ui, "profile_value_scroll_v08", 340.0, 520.0, |ui| {
                egui::Grid::new("profile_value_grid_v08")
                    .striped(true)
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Sample").strong());
                        ui.label(egui::RichText::new(self.analysis_metric.display_name()).strong());
                        ui.end_row();
                        for point in &self.profile {
                            ui.monospace(&point.sample);
                            ui.label(format_optional(point.value));
                            ui.end_row();
                        }
                    });
            });
        });
    }

    fn render_cohort_tab(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "1. Cohort & metadata", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "Scan the dataset root once. The app detects gene/transcript expression, BCR, BCR isotype, TCR alpha/beta, immune-infiltrate files, merged expression tables, and metadata candidates.",
                )
                .weak(),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Execution backend: {}",
                    this.analysis_backend.display_name()
                ))
                .small()
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                let choose = ui
                    .add_enabled(!this.cohort_running, egui::Button::new("Choose project folder…"))
                    .on_hover_text("Choose the top-level cohort directory. Subdirectories are scanned recursively; unsupported files are ignored.");
                if choose.clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(dir) = &this.last_directory {
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(root) = dialog.pick_folder() {
                        this.remember_directory_for(&root);
                        this.load_cohort_project(root);
                    }
                }
                if let Some(source) = this.cohort_project_source.clone() {
                    ui.monospace(source.display_name());
                    if let Some(root) = source.local_path().map(Path::to_path_buf) {
                        let rescan = ui
                            .add_enabled(!this.cohort_running, egui::Button::new("Rescan"))
                            .on_hover_text("Rescan the same local project root after new files or merged outputs have been added.");
                        if rescan.clicked() {
                            this.load_cohort_project(root);
                        }
                    }
                }
            });

            if let Some(scan) = this.cohort_scan.clone() {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for modality in [
                        CohortModality::GeneExpression,
                        CohortModality::TranscriptExpression,
                        CohortModality::Bcr,
                        CohortModality::BcrIsotype,
                        CohortModality::TcrAlpha,
                        CohortModality::TcrBeta,
                        CohortModality::ImmuneInfiltrate,
                    ] {
                        ui.label(format!(
                            "{}: {}",
                            modality.display_name(),
                            scan.count_modality(modality)
                        ));
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Merged gene TSV: {}",
                        scan.merged_gene_tsv
                            .as_ref()
                            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                            .unwrap_or_else(|| "not found".to_string())
                    ));
                    ui.separator();
                    ui.label(format!(
                        "Merged transcript TSV: {}",
                        scan.merged_transcript_tsv
                            .as_ref()
                            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                            .unwrap_or_else(|| "not found".to_string())
                    ));
                });

                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    let choose_meta = ui
                        .add_enabled(!this.cohort_running, egui::Button::new("Choose metadata…"))
                        .on_hover_text("Load cohort metadata from .xlsx, .csv, .tsv, or .txt. 4C_ID is used automatically when present, and can be changed below.");
                    if choose_meta.clicked() {
                        let mut dialog = FileDialog::new()
                            .add_filter("Metadata", &["xlsx", "csv", "tsv", "txt"]);
                        if let Some(dir) = &this.last_directory {
                            dialog = dialog.set_directory(dir);
                        }
                        if let Some(path) = dialog.pick_file() {
                            this.remember_directory_for(&path);
                            this.load_cohort_metadata_path(path);
                        }
                    }
                    if let Some(source) = &this.cohort_metadata_source {
                        ui.monospace(source.display_name());
                    } else if !scan.metadata_candidates.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} metadata candidate(s) detected",
                                scan.metadata_candidates.len()
                            ))
                            .weak(),
                        );
                    }
                });

                if this.cohort_metadata.is_some() {
                    let mut selected_id = this
                        .cohort_metadata
                        .as_ref()
                        .map(|m| m.id_column.clone())
                        .unwrap_or_default();
                    let columns = this
                        .cohort_metadata
                        .as_ref()
                        .map(|m| m.columns.clone())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("Sample ID column:")
                            .on_hover_text("This metadata column is matched to sample IDs parsed from filenames. Matching is case-insensitive and ignores punctuation such as '-' and '_'.");
                        egui::ComboBox::from_id_salt("cohort_metadata_id_column")
                            .selected_text(&selected_id)
                            .show_ui(ui, |ui| {
                                for col in &columns {
                                    ui.selectable_value(&mut selected_id, col.clone(), col);
                                }
                            });
                    });
                    let current_id = this
                        .cohort_metadata
                        .as_ref()
                        .map(|m| m.id_column.clone())
                        .unwrap_or_default();
                    if selected_id != current_id {
                        if let Some(meta) = &mut this.cohort_metadata {
                            meta.id_column = selected_id;
                        }
                        this.corrected_results = None;
                        this.unified_results = None;
                        this.outlier_results = None;
                        this.outlier_excluded_ids.clear();
                        this.rebuild_cohort_mapping();
                    }
                }
            } else {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("No cohort project loaded.").weak());
            }
        });

        ui.add_space(10.0);
        self.render_cohort_subset_card(ui);
        ui.add_space(10.0);
        self.render_cohort_mapping_card(ui);
        ui.add_space(10.0);
        self.render_outlier_qc_card(ui);
        ui.add_space(10.0);
        self.render_unified_analysis_settings_card(ui);
        if self.unified_results.is_some() {
            ui.add_space(10.0);
            self.render_unified_results_card(ui);
        }
    }

    fn active_cohort_inputs(&self) -> Option<(MetadataTable, PathBuf, CohortMapping, bool)> {
        if self.cohort_use_filtered_metadata {
            let metadata = self.focused_metadata.clone()?;
            let path = self.focused_metadata_path.clone()?;
            let mapping = self.focused_mapping.clone().or_else(|| {
                self.cohort_scan
                    .as_ref()
                    .and_then(|scan| self.analysis_backend.build_mapping(scan, &metadata).ok())
            })?;
            Some((metadata, path, mapping, true))
        } else {
            let metadata = self.cohort_metadata.clone()?;
            let path = self.cohort_metadata_path.clone()?;
            let mapping = self.cohort_mapping.clone().or_else(|| {
                self.cohort_scan
                    .as_ref()
                    .and_then(|scan| self.analysis_backend.build_mapping(scan, &metadata).ok())
            })?;
            Some((metadata, path, mapping, false))
        }
    }

    fn current_comparison_definition(&self) -> Result<CohortComparisonDefinition, String> {
        match self.unified_comparison_mode {
            UnifiedComparisonMode::BuiltIn => {
                Ok(CohortComparisonDefinition::Preset(self.cohort_preset))
            }
            UnifiedComparisonMode::CustomMetadata => {
                let column = self.custom_comparison_column.clone().ok_or_else(|| {
                    "Choose a metadata column for the custom comparison.".to_string()
                })?;
                if self.custom_case_values.is_empty() || self.custom_control_values.is_empty() {
                    return Err("Choose at least one case value and one control value.".to_string());
                }
                Ok(CohortComparisonDefinition::MetadataValues(
                    MetadataComparisonSpec {
                        column,
                        case_values: self
                            .custom_case_values
                            .iter()
                            .cloned()
                            .collect::<BTreeSet<_>>(),
                        control_values: self
                            .custom_control_values
                            .iter()
                            .cloned()
                            .collect::<BTreeSet<_>>(),
                        name: self.custom_comparison_name.clone(),
                        case_label: self.custom_case_label.clone(),
                        control_label: self.custom_control_label.clone(),
                    },
                ))
            }
        }
    }

    fn build_active_comparison(
        &self,
        metadata: &MetadataTable,
    ) -> Result<(CohortComparisonDefinition, CohortComparison), String> {
        let definition = self.current_comparison_definition()?;
        let comparison = build_comparison_definition(metadata, &definition)
            .map_err(|error| format!("{error:#}"))?;
        Ok((definition, comparison))
    }

    fn render_cohort_subset_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "1b. Optional cohort subset", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "A collaborator-supplied filtered metadata table can define which samples belong to this run. It changes the cohort/subset only; it does not switch statistical methodology.",
                )
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                let mut use_subset = this.cohort_use_filtered_metadata;
                if ui
                    .checkbox(&mut use_subset, "Use filtered metadata subset for this analysis")
                    .changed()
                {
                    this.cohort_use_filtered_metadata = use_subset;
                    this.unified_results = None;
                    this.outlier_results = None;
                    this.outlier_excluded_ids.clear();
                }
                if ui.button("Choose filtered metadata…").clicked() {
                    let mut dialog = FileDialog::new()
                        .add_filter("Metadata", &["xlsx", "xls", "csv", "tsv", "txt"]);
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(path) = dialog.pick_file() {
                        this.load_focused_metadata_path(path);
                        this.cohort_use_filtered_metadata = this.focused_metadata.is_some();
                        this.unified_results = None;
                        this.outlier_results = None;
                        this.outlier_excluded_ids.clear();
                    }
                }
                if this.focused_metadata.is_some() && ui.button("Clear filtered subset").clicked() {
                    this.focused_metadata = None;
                    this.focused_metadata_path = None;
                    this.focused_metadata_source = None;
                    this.focused_mapping = None;
                    this.cohort_use_filtered_metadata = false;
                    this.unified_results = None;
                    this.outlier_results = None;
                    this.outlier_excluded_ids.clear();
                }
            });
            if let Some(source) = &this.focused_metadata_source {
                ui.label(format!("Filtered metadata: {}", source.display_name()));
            }
            if this.focused_metadata.is_some() {
                let (mut selected_id, columns, current_id) = this
                    .focused_metadata
                    .as_ref()
                    .map(|m| (m.id_column.clone(), m.columns.clone(), m.id_column.clone()))
                    .unwrap_or_default();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Filtered metadata sample ID column:");
                    egui::ComboBox::from_id_salt("unified_filtered_metadata_id_column")
                        .selected_text(&selected_id)
                        .show_ui(ui, |ui| {
                            for column in &columns {
                                ui.selectable_value(&mut selected_id, column.clone(), column);
                            }
                        });
                });
                if selected_id != current_id {
                    if let Some(metadata) = &mut this.focused_metadata {
                        metadata.id_column = selected_id;
                    }
                    this.rebuild_focused_mapping();
                    this.unified_results = None;
                    this.outlier_results = None;
                    this.outlier_excluded_ids.clear();
                }
                if let Some(mapping) = &this.focused_mapping {
                    ui.label(format!(
                        "Filtered subset mapping: {} matched files, {} unmatched files, {} metadata-only samples.",
                        mapping.matched_files, mapping.unmatched_files, mapping.metadata_only_samples
                    ));
                }
            }
            if this.cohort_use_filtered_metadata && this.focused_metadata.is_none() {
                ui.colored_label(
                    egui::Color32::from_rgb(225, 120, 110),
                    "Filtered-subset mode is selected, but no filtered metadata table is loaded.",
                );
            }
        });
    }

    fn render_unified_analysis_settings_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. Analysis settings", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "The corrected / standard analysis is the primary inferential engine. Supporting descriptive checks, candidate prioritization, and optional PBMC/confounding sensitivity diagnostics are attached to the same completed run.",
                )
                .weak(),
            );

            let active = this.active_cohort_inputs();
            if let Some((metadata, path, _, uses_subset)) = active.as_ref() {
                ui.label(format!(
                    "Cohort source: {}{} | {} metadata rows",
                    path.file_name().and_then(|x| x.to_str()).unwrap_or("metadata"),
                    if *uses_subset { " (filtered subset)" } else { "" },
                    metadata.rows.len()
                ));
            }

            let mut comparison_changed = false;
            ui.separator();
            ui.label(egui::RichText::new("Comparison from metadata").strong());
            ui.label(
                egui::RichText::new(
                    "Choose a grouping field, then select one or more observed metadata values for Case and Control. Values are populated from the loaded metadata, so nothing has to be typed or memorized.",
                )
                .small()
                .weak(),
            );

            if let Some((metadata, _, _, _)) = active.as_ref() {
                let candidates = native_metadata_comparison_candidates(metadata);
                let suggested = suggested_native_comparison_column(metadata);
                let current_valid = this
                    .custom_comparison_column
                    .as_ref()
                    .is_some_and(|column| candidates.iter().any(|(name, _, _)| name == column));
                if !current_valid {
                    this.custom_comparison_column = suggested.clone();
                    this.custom_case_values.clear();
                    this.custom_control_values.clear();
                    this.custom_comparison_name.clear();
                    this.custom_case_label.clear();
                    this.custom_control_label.clear();
                    comparison_changed = true;
                }

                if this.unified_comparison_mode == UnifiedComparisonMode::BuiltIn {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(
                            egui::Color32::from_rgb(230, 170, 80),
                            format!(
                                "Legacy preset active: {}",
                                this.cohort_preset.display_name()
                            ),
                        );
                        if ui.small_button("Use metadata grouping instead").clicked() {
                            this.unified_comparison_mode = UnifiedComparisonMode::CustomMetadata;
                            comparison_changed = true;
                        }
                    });
                }

                let metadata_mode = this.unified_comparison_mode == UnifiedComparisonMode::CustomMetadata;
                ui.add_enabled_ui(metadata_mode, |ui| {
                    let mut selected_column = this.custom_comparison_column.clone();
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Grouping field");
                        egui::ComboBox::from_id_salt("native_metadata_comparison_column")
                            .selected_text(
                                selected_column
                                    .clone()
                                    .unwrap_or_else(|| "(choose metadata field)".to_string()),
                            )
                            .width(300.0)
                            .show_ui(ui, |ui| {
                                for (name, distinct, comparison_ready) in &candidates {
                                    let label = if *comparison_ready {
                                        format!("{name} ({distinct} values)")
                                    } else {
                                        format!("{name} ({distinct} values; high-cardinality)")
                                    };
                                    ui.selectable_value(
                                        &mut selected_column,
                                        Some(name.clone()),
                                        label,
                                    );
                                }
                            });
                        if let Some(suggested) = suggested.as_deref() {
                            ui.label(
                                egui::RichText::new(format!("Suggested: {suggested}"))
                                    .small()
                                    .weak(),
                            );
                        }
                    });

                    if selected_column != this.custom_comparison_column {
                        this.custom_comparison_column = selected_column.clone();
                        this.custom_case_values.clear();
                        this.custom_control_values.clear();
                        this.custom_comparison_name.clear();
                        this.custom_case_label.clear();
                        this.custom_control_label.clear();
                        comparison_changed = true;
                    }

                    if let Some(column) = selected_column {
                        let values = metadata_unique_values(metadata, &column);
                        if values.len() > 100 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "This field has {} distinct non-empty values; only the first 100 are exposed. Prefer a lower-cardinality categorical field for cohort comparison.",
                                    values.len()
                                ))
                                .color(egui::Color32::LIGHT_RED),
                            );
                        }

                        let mut case_values = this.custom_case_values.clone();
                        let mut control_values = this.custom_control_values.clone();
                        let mut selection_changed = false;
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Case values");
                            egui::ComboBox::from_id_salt("native_metadata_case_values")
                                .selected_text(native_selection_label(&case_values))
                                .width(280.0)
                                .show_ui(ui, |ui| {
                                    for (value, count) in values.iter().take(100) {
                                        let mut selected = case_values.contains(value);
                                        if ui
                                            .checkbox(
                                                &mut selected,
                                                format!("{value}  (n={count})"),
                                            )
                                            .changed()
                                        {
                                            selection_changed = true;
                                            if selected {
                                                case_values.insert(value.clone());
                                                control_values.remove(value);
                                            } else {
                                                case_values.remove(value);
                                            }
                                        }
                                    }
                                });
                            ui.label("Control values");
                            egui::ComboBox::from_id_salt("native_metadata_control_values")
                                .selected_text(native_selection_label(&control_values))
                                .width(280.0)
                                .show_ui(ui, |ui| {
                                    for (value, count) in values.iter().take(100) {
                                        let mut selected = control_values.contains(value);
                                        if ui
                                            .checkbox(
                                                &mut selected,
                                                format!("{value}  (n={count})"),
                                            )
                                            .changed()
                                        {
                                            selection_changed = true;
                                            if selected {
                                                control_values.insert(value.clone());
                                                case_values.remove(value);
                                            } else {
                                                control_values.remove(value);
                                            }
                                        }
                                    }
                                });
                            if ui.small_button("Clear groups").clicked() {
                                case_values.clear();
                                control_values.clear();
                                selection_changed = true;
                            }
                        });

                        if selection_changed {
                            this.custom_case_values = case_values.clone();
                            this.custom_control_values = control_values.clone();
                            comparison_changed = true;
                        }

                        let case_rows = native_selected_metadata_rows(&values, &case_values);
                        let control_rows = native_selected_metadata_rows(&values, &control_values);
                        ui.label(
                            egui::RichText::new(format!(
                                "Selected metadata rows: {case_rows} Case / {control_rows} Control · {} value(s) / {} value(s)",
                                case_values.len(),
                                control_values.len()
                            ))
                            .small(),
                        );
                        if case_values.is_empty() || control_values.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "Select at least one value for each group before running QC or analysis.",
                                )
                                .color(egui::Color32::LIGHT_RED),
                            );
                        }

                        egui::CollapsingHeader::new("Optional comparison labels")
                            .default_open(false)
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Case label:");
                                    comparison_changed |= ui
                                        .text_edit_singleline(&mut this.custom_case_label)
                                        .changed();
                                    ui.label("Control label:");
                                    comparison_changed |= ui
                                        .text_edit_singleline(&mut this.custom_control_label)
                                        .changed();
                                });
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Comparison name:");
                                    comparison_changed |= ui
                                        .add_sized(
                                            [420.0, 24.0],
                                            egui::TextEdit::singleline(
                                                &mut this.custom_comparison_name,
                                            ),
                                        )
                                        .changed();
                                });
                            });
                    } else {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            "Choose a metadata grouping field before defining Case and Control.",
                        );
                    }
                });

                egui::CollapsingHeader::new("Legacy 4C preset comparisons (optional)")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(
                                "These presets are retained only for reproducing earlier 4C analyses. New analyses should normally use the metadata grouping controls above.",
                            )
                            .small()
                            .weak(),
                        );
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Use HYP vs True_HC").clicked() {
                                this.cohort_preset = ComparisonPreset::HypVsTrueHc;
                                this.unified_comparison_mode = UnifiedComparisonMode::BuiltIn;
                                comparison_changed = true;
                            }
                            if ui.button("Use HYP+ME/CFS vs ME/CFS-only").clicked() {
                                this.cohort_preset = ComparisonPreset::HypMecfsVsMecfsOnly;
                                this.unified_comparison_mode = UnifiedComparisonMode::BuiltIn;
                                comparison_changed = true;
                            }
                        });
                    });
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(225, 120, 110),
                    "Load metadata before defining the Case / Control comparison.",
                );
            }

            if comparison_changed {
                this.unified_results = None;
                this.outlier_results = None;
                this.outlier_excluded_ids.clear();
            }
            if let Some((metadata, _, _, _)) = active.as_ref() {
                match this.build_active_comparison(metadata) {
                    Ok((definition, comparison)) => {
                        let source = match definition {
                            CohortComparisonDefinition::Preset(_) => "built-in preset".to_string(),
                            CohortComparisonDefinition::MetadataValues(spec) => {
                                format!("metadata column '{}'", spec.column)
                            }
                        };
                        ui.label(format!(
                            "Defined cohort: {} case / {} control metadata rows ({source})",
                            comparison.case_metadata_rows, comparison.control_metadata_rows
                        ));
                    }
                    Err(error) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(225, 120, 110),
                            format!("Comparison cannot be built: {error}"),
                        );
                    }
                }
            }

            ui.separator();
            ui.label(egui::RichText::new("Modalities").strong());
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut this.cohort_run_gene, "Gene expression");
                ui.checkbox(&mut this.cohort_run_transcript, "Transcript expression");
                ui.checkbox(&mut this.cohort_run_bcr, "BCR");
                ui.checkbox(&mut this.cohort_run_bcr_isotype, "BCR isotype");
                ui.checkbox(&mut this.cohort_run_tcr_alpha, "TCR alpha");
                ui.checkbox(&mut this.cohort_run_tcr_beta, "TCR beta");
                ui.checkbox(&mut this.cohort_run_immune, "Immune infiltrate");
                ui.checkbox(&mut this.corrected_run_pathway, "Pathway ORA");
            });
            if this.corrected_run_pathway {
                this.render_gmt_source_selector(ui, "unified");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Pathway foreground: BH q ≤");
                    ui.add(
                        egui::DragValue::new(&mut this.corrected_pathway_q)
                            .range(0.001..=0.25)
                            .speed(0.005),
                    );
                    ui.label("and |adjusted log2FC| ≥");
                    ui.add(
                        egui::DragValue::new(&mut this.corrected_pathway_effect)
                            .range(0.0..=5.0)
                            .speed(0.1),
                    );
                });
            }

            let columns = active
                .as_ref()
                .map(|(metadata, _, _, _)| metadata.columns.clone())
                .unwrap_or_default();
            if !columns.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Repeated measures & covariates").strong());
                ui.horizontal_wrapped(|ui| {
                    ui.label("Subject / patient ID:");
                    let selected = this
                        .corrected_subject_column
                        .clone()
                        .unwrap_or_else(|| "(none)".to_string());
                    egui::ComboBox::from_id_salt("unified_subject_column")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut this.corrected_subject_column, None, "(none)");
                            for column in &columns {
                                if active
                                    .as_ref()
                                    .is_some_and(|(metadata, _, _, _)| column == &metadata.id_column)
                                {
                                    continue;
                                }
                                ui.selectable_value(
                                    &mut this.corrected_subject_column,
                                    Some(column.clone()),
                                    column,
                                );
                            }
                        });
                });
                egui::CollapsingHeader::new(format!(
                    "Covariates ({})",
                    this.corrected_covariates.len()
                ))
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for column in &columns {
                            if active
                                .as_ref()
                                .is_some_and(|(metadata, _, _, _)| column == &metadata.id_column)
                                || this.corrected_subject_column.as_ref() == Some(column)
                            {
                                continue;
                            }
                            let mut selected = this.corrected_covariates.contains(column);
                            if ui.checkbox(&mut selected, column).changed() {
                                if selected {
                                    this.corrected_covariates.insert(column.clone());
                                } else {
                                    this.corrected_covariates.remove(column);
                                }
                            }
                        }
                    });
                });
            }

            egui::CollapsingHeader::new("Repertoire / model details")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Ambiguous V/D/J calls:");
                        egui::ComboBox::from_id_salt("unified_ambiguous_policy")
                            .selected_text(this.corrected_ambiguous_policy.display_name())
                            .show_ui(ui, |ui| {
                                for policy in AmbiguousCallPolicy::ALL {
                                    ui.selectable_value(
                                        &mut this.corrected_ambiguous_policy,
                                        policy,
                                        policy.display_name(),
                                    );
                                }
                            });
                    });
                    ui.checkbox(
                        &mut this.corrected_exclude_undetermined,
                        "Renormalize BCR-isotype analysis after excluding Undetermined",
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.label("PERMANOVA permutations:");
                        ui.add(
                            egui::DragValue::new(&mut this.corrected_permutations)
                                .range(99..=4999)
                                .speed(100),
                        );
                        ui.label("Rarefaction replicates:");
                        ui.add(
                            egui::DragValue::new(&mut this.corrected_rarefaction_reps)
                                .range(1..=20),
                        );
                        ui.label("Rarefaction cap:");
                        ui.add(
                            egui::DragValue::new(&mut this.corrected_rarefaction_cap)
                                .range(1_000..=500_000)
                                .speed(5_000),
                        );
                    });
                });

            ui.separator();
            ui.label(egui::RichText::new("QC, sensitivity & candidate summary").strong());
            ui.checkbox(
                &mut this.outlier_apply_to_corrected,
                "Apply reviewed sample exclusions to this run",
            )
            .on_hover_text("Automatic outlier flags remain review prompts. Only samples checked in the QC audit are excluded, and the exact frozen audit/exclusion state is attached to the completed run.");
            if this.outlier_apply_to_corrected {
                ui.label(format!(
                    "{} reviewed sample(s) currently selected for exclusion.",
                    this.outlier_excluded_ids.len()
                ));
            }
            ui.checkbox(
                &mut this.focused_run_exploratory_concordance,
                "Add exploratory TPM direction-concordance support",
            )
            .on_hover_text("This descriptive check is attached to candidate prioritization only; it is not independent validation and does not change the corrected p/q values.");
            ui.checkbox(
                &mut this.unified_run_diagnostics,
                "Run PBMC / confounding sensitivity diagnostics",
            )
            .on_hover_text("Adds source/composition QC, candidate robustness, QC-restricted and immune-adjusted sensitivity evidence to the same completed run when gene expression is available.");

            if this.unified_run_diagnostics {
                egui::CollapsingHeader::new("PBMC / confounding diagnostic settings")
                    .default_open(false)
                    .show(ui, |ui| {
                        if !columns.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                diagnostics_column_combo(
                                    ui,
                                    "unified_diag_batch",
                                    "Batch / collection period:",
                                    &columns,
                                    &mut this.diagnostics_batch_column,
                                );
                                diagnostics_column_combo(
                                    ui,
                                    "unified_diag_family",
                                    "Family:",
                                    &columns,
                                    &mut this.diagnostics_family_column,
                                );
                                diagnostics_column_combo(
                                    ui,
                                    "unified_diag_sensitivity",
                                    "Leave-level-out column:",
                                    &columns,
                                    &mut this.diagnostics_sensitivity_column,
                                );
                            });
                        }
                        ui.label("Known non-PBMC / wrong-source sample IDs:");
                        ui.text_edit_multiline(&mut this.diagnostics_known_non_pbmc_text);
                        ui.label("Other suspect / unusual sample IDs:");
                        ui.text_edit_multiline(&mut this.diagnostics_suspect_ids_text);
                        ui.checkbox(
                            &mut this.diagnostics_exclude_suspects,
                            "Exclude user-entered suspect IDs in QC sensitivity only",
                        );
                        ui.label(
                            egui::RichText::new(
                                "Automatic transcriptome/immune outlier calls remain flags in the unified workflow and are not auto-excluded. Use the reviewed cross-modal QC list above when an outlier should be removed from the primary run.",
                            )
                            .small()
                            .weak(),
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Robust outlier threshold:");
                            ui.add(
                                egui::DragValue::new(&mut this.diagnostics_outlier_z)
                                    .range(2.5..=10.0)
                                    .speed(0.25),
                            );
                            ui.label("Top candidate genes to diagnose:");
                            ui.add(
                                egui::DragValue::new(&mut this.diagnostics_top_candidates)
                                    .range(10..=250)
                                    .speed(5),
                            );
                        });
                    });
            }

            egui::CollapsingHeader::new("Key-findings shortlist settings")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Top genes:");
                        ui.add(egui::DragValue::new(&mut this.focused_settings.top_genes).range(1..=100));
                        ui.label("Top transcripts:");
                        ui.add(egui::DragValue::new(&mut this.focused_settings.top_transcripts).range(0..=100));
                        ui.label("Top pathways:");
                        ui.add(egui::DragValue::new(&mut this.focused_settings.top_pathways).range(0..=100));
                        ui.label("Top other signals:");
                        ui.add(egui::DragValue::new(&mut this.focused_settings.top_other).range(0..=100));
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Minimum candidate gene |adjusted log2FC|:");
                        ui.add(
                            egui::DragValue::new(&mut this.focused_settings.min_gene_abs_log2fc)
                                .range(0.0..=5.0)
                                .speed(0.1),
                        );
                    });
                    ui.label(
                        egui::RichText::new(
                            "These settings change only the presentation shortlist. They do not change the primary corrected tests.",
                        )
                        .small()
                        .weak(),
                    );
                });

            ui.add_space(8.0);
            let has_modality = this.cohort_run_gene
                || this.cohort_run_transcript
                || this.cohort_run_bcr
                || this.cohort_run_bcr_isotype
                || this.cohort_run_tcr_alpha
                || this.cohort_run_tcr_beta
                || this.cohort_run_immune
                || this.corrected_run_pathway;
            let ready = active.is_some() && has_modality && !this.cohort_running;
            if ui
                .add_enabled(ready, egui::Button::new("Run analysis"))
                .on_hover_text("Runs the selected corrected analyses and attaches the frozen QC state, optional supporting direction checks, sensitivity diagnostics, and candidate summary to one completed result.")
                .clicked()
            {
                this.start_unified_analysis();
            }
            if this.cohort_running {
                ui.add(
                    egui::ProgressBar::new(this.cohort_progress)
                        .show_percentage()
                        .animate(true),
                );
            }
            if !this.cohort_status.is_empty() {
                ui.label(egui::RichText::new(&this.cohort_status).small().weak());
            }
        });
    }

    fn render_unified_results_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "5. Unified results & export", |this, ui| {
            let Some(run) = this.unified_results.clone() else {
                return;
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} — {} case / {} control",
                        run.primary.comparison.name,
                        run.case_n(),
                        run.control_n()
                    ))
                    .strong(),
                );
                if ui
                    .button("Export simplified key-findings report…")
                    .on_hover_text("Presentation-friendly HTML generated from this exact completed run; no statistics are rerun.")
                    .clicked()
                {
                    let safe = run
                        .primary
                        .comparison
                        .name
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect::<String>();
                    let mut dialog = FileDialog::new()
                        .add_filter("HTML report", &["html"])
                        .set_file_name(format!("4c_key_findings_{safe}.html"));
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(path) = dialog.save_file() {
                        match generate_unified_simplified_html_report(&path, &run) {
                            Ok(()) => {
                                this.cohort_status = format!(
                                    "Generated simplified key-findings report: {}",
                                    path.display()
                                )
                            }
                            Err(error) => {
                                this.cohort_status =
                                    format!("Could not generate simplified report: {error:#}")
                            }
                        }
                    }
                }
                if ui
                    .button("Export full detailed report…")
                    .on_hover_text("Technical/audit HTML with mapping, frozen QC provenance, all selected modality results, sensitivities, pathway provenance, warnings, methods, and limitations from this exact run.")
                    .clicked()
                {
                    let safe = run
                        .primary
                        .comparison
                        .name
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect::<String>();
                    let mut dialog = FileDialog::new()
                        .add_filter("HTML report", &["html"])
                        .set_file_name(format!("4c_full_report_{safe}.html"));
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(path) = dialog.save_file() {
                        match generate_unified_detailed_html_report(&path, &run) {
                            Ok(()) => {
                                this.cohort_status =
                                    format!("Generated full detailed report: {}", path.display())
                            }
                            Err(error) => {
                                this.cohort_status =
                                    format!("Could not generate full report: {error:#}")
                            }
                        }
                    }
                }
                if ui.button("Export all result tables…").clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(folder) = dialog.pick_folder() {
                        match write_unified_analysis_tables(&folder, &run) {
                            Ok(path) => {
                                this.cohort_status =
                                    format!("Saved unified result tables to {}", path.display())
                            }
                            Err(error) => {
                                this.cohort_status =
                                    format!("Could not save unified result tables: {error:#}")
                            }
                        }
                    }
                }
            });

            if let Some(qc) = run.qc_snapshot.as_ref() {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 170, 80),
                    if qc.applied_to_analysis {
                        format!(
                            "Frozen QC provenance: {} reviewed exclusion(s) applied (case {} → {}, control {} → {}).",
                            qc.excluded_ids.len(),
                            qc.original_case_n,
                            qc.filtered_case_n,
                            qc.original_control_n,
                            qc.filtered_control_n
                        )
                    } else {
                        "Frozen QC audit attached; no reviewed exclusions were applied to the primary analysis."
                            .to_string()
                    },
                );
            }
            ui.colored_label(
                egui::Color32::from_rgb(230, 190, 90),
                "Candidate rankings are preliminary signals for follow-up, not validated biomarkers.",
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("PCA / PCoA / dendrogram color:")
                    .on_hover_text("Color unsupervised sample-level views by analysis group, subject, or a metadata field from the cohort definition used for this run.");
                egui::ComboBox::from_id_salt("unified_ordination_color_by")
                    .selected_text(&this.corrected_ordination_color_by)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut this.corrected_ordination_color_by,
                            "Analysis group".to_string(),
                            "Analysis group",
                        );
                        ui.selectable_value(
                            &mut this.corrected_ordination_color_by,
                            "Subject / patient".to_string(),
                            "Subject / patient",
                        );
                        let metadata = if this.cohort_use_filtered_metadata {
                            this.focused_metadata.as_ref()
                        } else {
                            this.cohort_metadata.as_ref()
                        };
                        if let Some(metadata) = metadata {
                            for column in &metadata.columns {
                                ui.selectable_value(
                                    &mut this.corrected_ordination_color_by,
                                    column.clone(),
                                    column,
                                );
                            }
                        }
                    });
            });

            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::Overview, "Overview");
                if run.primary.gene.is_some() || run.primary.gene_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::Gene, "Gene");
                }
                if run.primary.transcript.is_some() || run.primary.transcript_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::Transcript, "Transcript");
                }
                if run.primary.bcr.is_some() || run.primary.bcr_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::Bcr, "BCR");
                }
                if run.primary.bcr_isotype.is_some() || run.primary.bcr_isotype_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::BcrIsotype, "BCR isotype");
                }
                if run.primary.tcr_alpha.is_some() || run.primary.tcr_alpha_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::TcrAlpha, "TCR alpha");
                }
                if run.primary.tcr_beta.is_some() || run.primary.tcr_beta_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::TcrBeta, "TCR beta");
                }
                if run.primary.immune.is_some() || run.primary.immune_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::Immune, "Immune");
                }
                if run.primary.pathway.is_some() || run.primary.pathway_error.is_some() {
                    ui.selectable_value(&mut this.unified_result_view, UnifiedResultView::Pathway, "Pathways");
                }
                ui.selectable_value(
                    &mut this.unified_result_view,
                    UnifiedResultView::QcRobustness,
                    "QC / robustness",
                );
            });
            ui.separator();

            match this.unified_result_view {
                UnifiedResultView::Overview => {
                    let fdr_genes = run
                        .candidates
                        .genes
                        .iter()
                        .filter(|row| row.q_value.is_finite() && row.q_value <= 0.05)
                        .count();
                    ui.heading("Key findings from the completed run");
                    ui.label(format!(
                        "{} shortlisted genes ({} FDR-supported), {} pathways, and {} other modality signals.",
                        run.candidates.genes.len(),
                        fdr_genes,
                        run.candidates.pathways.len(),
                        run.candidates.other.len()
                    ));
                    if let Some(top) = run.candidates.genes.first() {
                        ui.label(format!(
                            "Top ranked gene candidate: {} — adjusted log2FC {:.2}, BH q {:.2e}{}.",
                            top.gene,
                            top.adjusted_log2fc,
                            top.q_value,
                            top.robustness
                                .as_ref()
                                .map(|label| format!(", robustness: {label}"))
                                .unwrap_or_default()
                        ));
                    }
                    if let Some(top) = run.candidates.pathways.first() {
                        ui.label(format!(
                            "Top ranked pathway: {} — odds ratio {:.2}, BH q {:.2e}.",
                            top.pathway, top.odds_ratio, top.q_value
                        ));
                    }
                    if !run.warnings.is_empty() {
                        ui.add_space(6.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(230, 170, 80),
                            format!("{} run warning(s) recorded; review QC / robustness or the full report.", run.warnings.len()),
                        );
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Top candidate genes").strong());
                    fixed_table_viewport(ui, "unified_candidate_genes", 360.0, 1450.0, |ui| {
                        egui::Grid::new("unified_candidate_genes_grid").striped(true).show(ui, |ui| {
                            for heading in ["Rank", "Gene", "Adj log2FC", "BH q", "TPM direction", "Pathway support", "Robustness", "Evidence"] {
                                ui.label(egui::RichText::new(heading).strong());
                            }
                            ui.end_row();
                            for row in run.candidates.genes.iter().take(15) {
                                ui.label(row.rank.to_string());
                                ui.monospace(&row.gene);
                                ui.label(format!("{:.3}", row.adjusted_log2fc));
                                ui.label(format!("{:.2e}", row.q_value));
                                ui.label(match row.direction_concordant { Some(true) => "same", Some(false) => "opposite", None => "—" });
                                ui.label(row.pathway_support_count.to_string());
                                ui.label(row.robustness.as_deref().unwrap_or("—"));
                                ui.label(&row.evidence);
                                ui.end_row();
                            }
                        });
                    });
                }
                UnifiedResultView::Gene => {
                    if let Some(result) = run.primary.gene.as_ref() {
                        this.render_corrected_expression(ui, result, "unified_gene");
                    } else {
                        ui.label(run.primary.gene_error.as_deref().unwrap_or("Gene analysis not requested."));
                    }
                }
                UnifiedResultView::Transcript => {
                    if let Some(result) = run.primary.transcript.as_ref() {
                        this.render_corrected_expression(ui, result, "unified_transcript");
                    } else {
                        ui.label(run.primary.transcript_error.as_deref().unwrap_or("Transcript analysis not requested."));
                    }
                }
                UnifiedResultView::Bcr => {
                    if let Some(result) = run.primary.bcr.as_ref() {
                        this.render_corrected_repertoire(ui, result, "unified_bcr");
                    } else {
                        ui.label(run.primary.bcr_error.as_deref().unwrap_or("BCR analysis not requested."));
                    }
                }
                UnifiedResultView::BcrIsotype => {
                    if let Some(result) = run.primary.bcr_isotype.as_ref() {
                        this.render_corrected_isotype(ui, result, "unified_bcr_isotype");
                    } else {
                        ui.label(run.primary.bcr_isotype_error.as_deref().unwrap_or("BCR-isotype analysis not requested."));
                    }
                }
                UnifiedResultView::TcrAlpha => {
                    if let Some(result) = run.primary.tcr_alpha.as_ref() {
                        this.render_corrected_repertoire(ui, result, "unified_tcr_alpha");
                    } else {
                        ui.label(run.primary.tcr_alpha_error.as_deref().unwrap_or("TCR-alpha analysis not requested."));
                    }
                }
                UnifiedResultView::TcrBeta => {
                    if let Some(result) = run.primary.tcr_beta.as_ref() {
                        this.render_corrected_repertoire(ui, result, "unified_tcr_beta");
                    } else {
                        ui.label(run.primary.tcr_beta_error.as_deref().unwrap_or("TCR-beta analysis not requested."));
                    }
                }
                UnifiedResultView::Immune => {
                    if let Some(result) = run.primary.immune.as_ref() {
                        this.render_corrected_immune(ui, result, "unified_immune");
                    } else {
                        ui.label(run.primary.immune_error.as_deref().unwrap_or("Immune analysis not requested."));
                    }
                }
                UnifiedResultView::Pathway => {
                    if let Some(result) = run.primary.pathway.as_ref() {
                        this.render_corrected_pathway(ui, result, "unified_pathway");
                    } else {
                        ui.label(run.primary.pathway_error.as_deref().unwrap_or("Pathway analysis not requested."));
                    }
                }
                UnifiedResultView::QcRobustness => {
                    if let Some(qc) = run.qc_snapshot.as_ref() {
                        ui.heading("Frozen sample QC provenance");
                        ui.label(format!(
                            "Outlier scope: {} | threshold {:.2} | rule: {} | exclusions applied: {}",
                            qc.audit.config.scope.display_name(),
                            qc.audit.config.robust_distance_threshold,
                            qc.audit.config.consensus_rule.display_name(),
                            qc.excluded_ids.len()
                        ));
                        if !qc.excluded_ids.is_empty() {
                            ui.label(format!("Excluded samples: {}", qc.excluded_ids.join(", ")));
                        }
                    } else {
                        ui.label("No frozen cross-modal outlier audit was attached to this run.");
                    }
                    if let Some(result) = run.diagnostics.as_ref() {
                        ui.separator();
                        ui.heading("PBMC / confounding sensitivity diagnostics");
                        this.render_diagnostics_overview(ui, result);
                        this.render_diagnostics_candidates(ui, result);
                    } else if let Some(error) = run.diagnostics_error.as_deref() {
                        ui.colored_label(egui::Color32::from_rgb(225, 120, 110), error);
                    }
                    if !run.warnings.is_empty() {
                        ui.separator();
                        egui::CollapsingHeader::new(format!("Run warnings ({})", run.warnings.len()))
                            .default_open(true)
                            .show(ui, |ui| {
                                for warning in &run.warnings {
                                    ui.label(format!("• {warning}"));
                                }
                            });
                    }
                }
            }
        });
    }

    fn render_cohort_mapping_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "2. Sample mapping & QC", |this, ui| {
            let mapping = if this.cohort_use_filtered_metadata { this.focused_mapping.clone() } else { this.cohort_mapping.clone() };
            let Some(mapping) = mapping else {
                ui.label(
                    egui::RichText::new(
                        "Load both a project and metadata to audit cross-modality sample mapping.",
                    )
                    .weak(),
                );
                return;
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Matched files: {}", mapping.matched_files));
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(230, 190, 90),
                    format!("Unmatched files: {}", mapping.unmatched_files),
                );
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(225, 120, 110),
                    format!("Ambiguous files: {}", mapping.ambiguous_files),
                );
                ui.separator();
                ui.label(format!("Metadata-only samples: {}", mapping.metadata_only_samples));
                ui.separator();
                ui.label(format!("Audit rows: {}", mapping.rows.len()));
            });
            ui.label(
                egui::RichText::new(
                    "Unmatched/ambiguous samples are reported and skipped only for metadata-defined comparisons; they do not abort the project scan or other usable analyses.",
                )
                .small()
                .weak(),
            );

            ui.horizontal_wrapped(|ui| {
                let export = ui
                    .button("Export mapping CSV…")
                    .on_hover_text("Export the complete sample mapping and modality-coverage audit as a CSV file. This is useful for sending unmatched, ambiguous, metadata-only, or duplicate entries back for metadata correction.");
                if export.clicked() {
                    let mut dialog = FileDialog::new()
                        .add_filter("CSV", &["csv"])
                        .set_file_name("sample_mapping_and_coverage.csv");
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(path) = dialog.save_file() {
                        match write_mapping_audit_csv(&path, &mapping) {
                            Ok(()) => {
                                this.cohort_status = format!(
                                    "Saved sample mapping and coverage audit to {}",
                                    path.display()
                                );
                            }
                            Err(e) => {
                                this.cohort_status = format!(
                                    "Could not export sample mapping CSV: {e:#}"
                                );
                            }
                        }
                    }
                }
            });

            ui.add_space(6.0);
            let active_metadata = if this.cohort_use_filtered_metadata {
                this.focused_metadata.clone()
            } else {
                this.cohort_metadata.clone()
            };
            let mapping_group_column = match this.unified_comparison_mode {
                UnifiedComparisonMode::CustomMetadata => this.custom_comparison_column.clone(),
                UnifiedComparisonMode::BuiltIn => Some("Phenotype".to_string()),
            };
            let mapping_group_heading = mapping_group_column
                .clone()
                .unwrap_or_else(|| "Group field".to_string());
            let mapping_view_width = ui.available_width().max(320.0);
            ui.allocate_ui(egui::vec2(mapping_view_width, 470.0), |ui| {
                egui::ScrollArea::both()
                    .id_salt("cohort_mapping_scroll_v08")
                    .max_height(470.0)
                    .auto_shrink([false, false])
                    .show_rows(ui, 24.0, mapping.rows.len(), |ui, range| {
                        ui.set_min_width(1280.0);
                        egui::Grid::new("cohort_mapping_grid_v08")
                            .striped(true)
                            .num_columns(12)
                            .show(ui, |ui| {
                                if range.start == 0 {
                                    for heading in [
                                        "Sample", "Map", mapping_group_heading.as_str(), "HYP", "Gene", "Tx", "BCR", "Iso",
                                        "TCRα", "TCRβ", "Immune", "Notes",
                                    ] {
                                        ui.label(egui::RichText::new(heading).strong());
                                    }
                                    ui.end_row();
                                }
                                for row_idx in range {
                                    let row = &mapping.rows[row_idx];
                                    ui.monospace(&row.sample_id);
                                    match row.mapping_status {
                                        MappingStatus::Matched => { ui.colored_label(egui::Color32::from_rgb(105, 190, 130), "matched"); }
                                        MappingStatus::Unmatched => { ui.colored_label(egui::Color32::from_rgb(230, 190, 90), "unmatched"); }
                                        MappingStatus::Ambiguous => { ui.colored_label(egui::Color32::from_rgb(225, 120, 110), "ambiguous"); }
                                        MappingStatus::MetadataOnly => { ui.colored_label(egui::Color32::from_rgb(130, 165, 220), "metadata only"); }
                                    }
                                    let group_value = row.metadata_row
                                        .and_then(|metadata_row| active_metadata.as_ref().and_then(|metadata| {
                                            mapping_group_column.as_deref().and_then(|column| metadata.value(metadata_row, column))
                                        }))
                                        .unwrap_or(row.phenotype.as_str());
                                    ui.label(group_value);
                                    ui.label(&row.hyp);
                                    for modality in [
                                        CohortModality::GeneExpression,
                                        CohortModality::TranscriptExpression,
                                        CohortModality::Bcr,
                                        CohortModality::BcrIsotype,
                                        CohortModality::TcrAlpha,
                                        CohortModality::TcrBeta,
                                        CohortModality::ImmuneInfiltrate,
                                    ] {
                                        let n = row.counts.get(&modality).copied().unwrap_or(0);
                                        if n > 0 { ui.label(n.to_string()); } else { ui.label("·"); }
                                    }
                                    let duplicates: Vec<_> = row.counts.iter()
                                        .filter(|(_, n)| **n > 1)
                                        .map(|(m, n)| format!("{}×{}", m.display_name(), n))
                                        .collect();
                                    ui.label(if duplicates.is_empty() { String::new() } else { format!("duplicate: {}", duplicates.join(", ")) });
                                    ui.end_row();
                                }
                            });
                    });
            });
        });
    }

    fn render_cohort_comparison_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. Exploratory biomarker comparison", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "This first-pass analysis intentionally uses all available mapped samples, including longitudinal samples, without covariate or repeated-measures adjustment.",
                )
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Comparison:");
                let old = this.cohort_preset;
                ui.selectable_value(
                    &mut this.cohort_preset,
                    ComparisonPreset::HypVsTrueHc,
                    "HYP vs True_HC",
                )
                .on_hover_text("All HYP=yes metadata rows (hEDS/HSD with or without ME/CFS) versus Phenotype=True_HC.");
                ui.selectable_value(
                    &mut this.cohort_preset,
                    ComparisonPreset::HypMecfsVsMecfsOnly,
                    "HYP+ME/CFS vs ME/CFS-only",
                )
                .on_hover_text("Phenotype=HYP+ME/CFS versus Phenotype=ME/CFS and HYP not marked yes.");
                if old != this.cohort_preset {
                    this.cohort_results = None;
                    this.outlier_results = None;
                    this.outlier_excluded_ids.clear();
                }
            });
            ui.label(
                egui::RichText::new(this.cohort_preset.description())
                    .small()
                    .weak(),
            );

            if let Some(meta) = &this.cohort_metadata {
                match build_comparison(meta, this.cohort_preset) {
                    Ok(comp) => {
                        ui.label(format!(
                            "Metadata definition: {} case rows / {} control rows",
                            comp.case_metadata_rows, comp.control_metadata_rows
                        ));
                    }
                    Err(e) => {
                        ui.colored_label(
                            egui::Color32::from_rgb(225, 120, 110),
                            format!("Comparison cannot be built: {e:#}"),
                        );
                    }
                }
            }

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Optional analyses").strong());
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut this.cohort_run_gene, "Gene expression")
                    .on_hover_text("Tests every gene from the merged gene-expression table using the selected value type. Reports Mann-Whitney U, BH FDR, Cliff's delta, fold change and detection shifts.");
                ui.checkbox(&mut this.cohort_run_transcript, "Transcript expression")
                    .on_hover_text("Tests transcript-level abundance from the merged transcript-expression table. TPM is recommended for the exploratory screen; Total Reads is also available.");
                ui.checkbox(&mut this.cohort_run_bcr, "BCR")
                    .on_hover_text("Summarizes clone count, diversity/clonality, top-clone fraction, weighted CDR3 length, and BCR isotype/V/D/J usage from heavy-chain reports.");
                ui.checkbox(&mut this.cohort_run_bcr_isotype, "BCR isotype")
                    .on_hover_text("Compares the dedicated IGH isotype-composition fractions across groups.");
                ui.checkbox(&mut this.cohort_run_tcr_alpha, "TCR alpha")
                    .on_hover_text("Summarizes alpha-chain clone depth/diversity/clonality, CDR3 length, V-gene, J-gene and V-J pair usage.");
                ui.checkbox(&mut this.cohort_run_tcr_beta, "TCR beta")
                    .on_hover_text("Summarizes beta-chain clone depth/diversity/clonality, CDR3 length, V-gene, J-gene and V-J pair usage.");
                ui.checkbox(&mut this.cohort_run_immune, "Immune infiltrate")
                    .on_hover_text("Compares ssGSEA immune-cell infiltration scores for each reported immune-cell type.");
            });
            if this.cohort_run_gene {
                ui.horizontal(|ui| {
                    ui.label("Gene value:");
                    egui::ComboBox::from_id_salt("cohort_gene_metric")
                        .selected_text(this.cohort_gene_metric.display_name())
                        .show_ui(ui, |ui| {
                            for metric in [Metric::Tpm, Metric::Fpkm, Metric::Cpm, Metric::RawCount] {
                                ui.selectable_value(
                                    &mut this.cohort_gene_metric,
                                    metric,
                                    metric.display_name(),
                                );
                            }
                        });
                });
                ui.label(
                    egui::RichText::new(
                        "TPM is the default for this simple exploratory screen. Raw-count modeling with DESeq2/edgeR-style dispersion estimation is not being claimed here.",
                    )
                    .small()
                    .weak(),
                );
            }
            if this.cohort_run_transcript {
                ui.horizontal(|ui| {
                    ui.label("Transcript value:");
                    egui::ComboBox::from_id_salt("cohort_transcript_metric")
                        .selected_text(this.cohort_transcript_metric.display_name())
                        .show_ui(ui, |ui| {
                            for metric in [Metric::Tpm, Metric::TotalReads] {
                                ui.selectable_value(
                                    &mut this.cohort_transcript_metric,
                                    metric,
                                    metric.display_name(),
                                );
                            }
                        });
                });
                ui.label(
                    egui::RichText::new(
                        "Transcript analysis uses the same exploratory non-parametric screen as gene expression; transcript length fields are not treated as biological abundance measurements.",
                    )
                    .small()
                    .weak(),
                );
            }
            let ready = this.cohort_scan.is_some()
                && this.cohort_metadata.is_some()
                && !this.cohort_running
                && (this.cohort_run_gene
                    || this.cohort_run_transcript
                    || this.cohort_run_bcr
                    || this.cohort_run_bcr_isotype
                    || this.cohort_run_tcr_alpha
                    || this.cohort_run_tcr_beta
                    || this.cohort_run_immune);
            let run = ui
                .add_enabled(ready, egui::Button::new("Run exploratory comparison"))
                .on_hover_text("Run each selected modality independently. If one input type is absent or cannot be analyzed, the remaining modalities continue and the failure is reported.");
            if run.clicked() {
                this.start_cohort_analysis();
            }

            if this.cohort_running {
                ui.add(
                    egui::ProgressBar::new(this.cohort_progress)
                        .show_percentage()
                        .animate(true),
                );
            }
            ui.label(&this.cohort_status);
        });
    }

    fn render_outlier_qc_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "2b. Cross-modal sample QC", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "Outlier screening is an unsupervised QC step. It flags samples that are unusually distant from comparable samples within one or more data types. A flag is evidence to review a sample, not proof that the biology is invalid.",
                )
                .weak(),
            );
            ui.colored_label(
                egui::Color32::from_rgb(230, 190, 90),
                "Recommended: detect outliers within case/control groups and inspect the flagged samples. Automatic flags do not exclude samples; reviewed exclusions are applied explicitly in Analysis settings and frozen into the completed run.",
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Detection scope:");
                egui::ComboBox::from_id_salt("outlier_scope")
                    .selected_text(this.outlier_config.scope.display_name())
                    .show_ui(ui, |ui| {
                        for scope in OutlierScope::ALL {
                            ui.selectable_value(&mut this.outlier_config.scope, scope, scope.display_name());
                        }
                    });
                ui.label("Robust distance threshold:");
                ui.add(egui::DragValue::new(&mut this.outlier_config.robust_distance_threshold).range(2.0..=10.0).speed(0.1));
            });
            ui.label(
                egui::RichText::new(
                    "Within-group detection is safer when the disease itself may create a real global expression/repertoire shift. The score is a robust distance in the first two unsupervised components of each modality.",
                )
                .small()
                .weak(),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Cross-modal consensus flagging rule:");
                egui::ComboBox::from_id_salt("outlier_consensus_rule")
                    .selected_text(this.outlier_config.consensus_rule.display_name())
                    .show_ui(ui, |ui| {
                        for rule in OutlierConsensusRule::ALL {
                            ui.selectable_value(&mut this.outlier_config.consensus_rule, rule, rule.display_name());
                        }
                    });
                if this.outlier_config.consensus_rule == OutlierConsensusRule::AtLeastN {
                    ui.label("N:");
                    ui.add(egui::DragValue::new(&mut this.outlier_config.at_least_n).range(1..=7));
                }
                if this.outlier_config.consensus_rule != OutlierConsensusRule::Any {
                    ui.label("Minimum available modalities:");
                    ui.add(egui::DragValue::new(&mut this.outlier_config.min_available_modalities).range(1..=7));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Modalities included:").strong());
                if this.cohort_run_gene { ui.label("Gene"); }
                if this.cohort_run_transcript { ui.label("Transcript"); }
                if this.cohort_run_bcr { ui.label("BCR"); }
                if this.cohort_run_bcr_isotype { ui.label("BCR isotype"); }
                if this.cohort_run_tcr_alpha { ui.label("TCR alpha"); }
                if this.cohort_run_tcr_beta { ui.label("TCR beta"); }
                if this.cohort_run_immune { ui.label("Immune"); }
            });
            let ready = this.cohort_scan.is_some() && this.active_cohort_inputs().is_some() && !this.cohort_running;
            if ui.add_enabled(ready, egui::Button::new("Run sample outlier QC"))
                .on_hover_text("Build sample-level unsupervised representations for the currently enabled modalities, flag modality-specific outliers, and combine those flags using the selected consensus rule.")
                .clicked()
            {
                this.start_outlier_qc();
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} sample(s) currently marked by the reviewer for possible exclusion. Whether these reviewed exclusions are applied is controlled in Analysis settings.",
                    this.outlier_excluded_ids.len()
                ))
                .small()
                .weak(),
            );

            let Some(result) = this.outlier_results.clone() else { return; };
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Audit comparison: {}", result.comparison_name));
                ui.separator();
                ui.label(format!("{} modality result(s)", result.modalities.len()));
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(230, 170, 70),
                    format!("{} consensus outlier(s)", result.consensus_ids.len()),
                );
                if ui.button("Select consensus outliers").on_hover_text("Replace the manual exclusion list with the samples meeting the current audit's cross-modal consensus rule.").clicked() {
                    this.outlier_excluded_ids = result.consensus_ids.iter().cloned().collect();
                }
                if ui.button("Clear exclusions").clicked() {
                    this.outlier_excluded_ids.clear();
                }
                if ui.button("Export audit CSV…")
                    .on_hover_text("Save the sample-level consensus flags plus per-modality robust outlier scores/flags for review outside the app.")
                    .clicked()
                {
                    let mut dialog = FileDialog::new().add_filter("CSV", &["csv"]);
                    if let Some(root) = &this.cohort_root { dialog = dialog.set_directory(root); }
                    if let Some(path) = dialog.set_file_name("sample_outlier_audit.csv").save_file() {
                        match write_outlier_audit_csv(&path, &result) {
                            Ok(()) => this.cohort_status = format!("Saved sample outlier audit to {}", path.display()),
                            Err(e) => this.cohort_status = format!("Could not save sample outlier audit: {e:#}"),
                        }
                    }
                }
            });
            ui.label(
                egui::RichText::new(
                    "The exclusion checkbox is deliberately editable. A sample can be reviewed and kept even if it crosses the automatic threshold, or manually excluded if the QC/provenance evidence supports doing so.",
                )
                .small()
                .weak(),
            );

            if !result.modalities.is_empty() {
                this.outlier_plot_modality = this.outlier_plot_modality.min(result.modalities.len() - 1);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Outlier view:");
                    egui::ComboBox::from_id_salt("outlier_plot_modality")
                        .selected_text(&result.modalities[this.outlier_plot_modality].modality)
                        .show_ui(ui, |ui| {
                            for (idx, modality) in result.modalities.iter().enumerate() {
                                ui.selectable_value(&mut this.outlier_plot_modality, idx, &modality.modality);
                            }
                        });
                });
                let modality = &result.modalities[this.outlier_plot_modality];
                ui.label(egui::RichText::new(&modality.representation).small().weak());
                let mut ordinary = Vec::new();
                let mut flagged = Vec::new();
                let mut hover_points = Vec::new();
                for point in &modality.points {
                    if point.flagged { flagged.push([point.x, point.y]); } else { ordinary.push([point.x, point.y]); }
                    hover_points.push((point.x, point.y, format!("{}\nGroup: {}\nRobust distance: {:.2}\nModality flag: {}", point.sample_id, point.group, point.robust_distance, if point.flagged { "yes" } else { "no" })));
                }
                let formatter_points = hover_points.clone();
                Plot::new("outlier_qc_plot")
                    .height(360.0)
                    .x_axis_label(&modality.x_label)
                    .y_axis_label(&modality.y_label)
                    .legend(Legend::default())
                    .label_formatter(move |hover| match hover {
                        HoverPosition::NearDataPoint { position, .. } => nearest_hover_label(position.x, position.y, &formatter_points),
                        _ => None,
                    })
                    .show(ui, |plot_ui| {
                        if !ordinary.is_empty() { plot_ui.points(Points::new("Not flagged", ordinary).color(egui::Color32::from_rgb(100, 165, 225)).radius(3.5)); }
                        if !flagged.is_empty() { plot_ui.points(Points::new("Modality outlier", flagged).color(egui::Color32::from_rgb(235, 145, 70)).radius(5.0)); }
                    });
            }

            ui.add_space(6.0);
            ui.label(egui::RichText::new(format!("Sample outlier audit ({} samples)", result.samples.len())).strong());
            fixed_table_viewport(ui, "outlier_audit_table", 420.0, 900.0, |ui| {
                egui::Grid::new("outlier_audit_grid").striped(true).show(ui, |ui| {
                    for h in ["Exclude", "Sample", "Group", "Available modalities", "Flagged modalities", "Flagged in"] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for row in &result.samples {
                        let mut excluded = this.outlier_excluded_ids.contains(&row.sample_id);
                        if ui.checkbox(&mut excluded, "").changed() {
                            if excluded { this.outlier_excluded_ids.insert(row.sample_id.clone()); }
                            else { this.outlier_excluded_ids.remove(&row.sample_id); }
                        }
                        ui.monospace(&row.sample_id);
                        ui.label(&row.group);
                        ui.label(row.available_modalities.to_string());
                        if row.consensus_flag {
                            ui.colored_label(egui::Color32::from_rgb(235, 145, 70), row.flagged_modalities.to_string());
                        } else {
                            ui.label(row.flagged_modalities.to_string());
                        }
                        ui.label(if row.flagged_names.is_empty() { "—".to_string() } else { row.flagged_names.join(", ") });
                        ui.end_row();
                    }
                });
            });
            if !result.warnings.is_empty() || result.modalities.iter().any(|m| !m.warnings.is_empty()) {
                egui::CollapsingHeader::new("Outlier QC warnings").show(ui, |ui| {
                    for warning in &result.warnings { ui.label(format!("• {warning}")); }
                    for modality in &result.modalities {
                        for warning in &modality.warnings { ui.label(format!("• {}: {}", modality.modality, warning)); }
                    }
                });
            }
        });
    }

    fn render_focused_analysis_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. Focused candidate-signal workflow", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "Use a filtered metadata file containing the direct comparison you want to present (ideally one sample per individual). The corrected/standard analysis is the primary inference; an optional exploratory expression screen is used only to show whether the direction of change agrees across two analysis views of the same samples.",
                )
                .weak(),
            );
            ui.colored_label(
                egui::Color32::from_rgb(230, 190, 90),
                "Outputs are preliminary candidate signals for follow-up, not validated biomarkers.",
            );
            if this.outlier_apply_to_focused && !this.outlier_excluded_ids.is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 170, 70),
                    format!("Outlier sensitivity filter is ON: up to {} reviewed sample(s) will be excluded if present in the focused metadata.", this.outlier_excluded_ids.len()),
                );
            }

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Focused sample list / metadata").strong());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Choose filtered metadata…")
                    .on_hover_text("Load the collaborator-curated sample list. Rows absent from this file are excluded from the focused comparison without changing the full project metadata used by the other analysis layers.")
                    .clicked()
                {
                    let mut dialog = FileDialog::new().add_filter("Metadata", &["xlsx", "xls", "csv", "tsv", "txt"]);
                    if let Some(root) = &this.cohort_root { dialog = dialog.set_directory(root); }
                    if let Some(path) = dialog.pick_file() { this.load_focused_metadata_path(path); }
                }
                if ui.button("Use current project metadata")
                    .on_hover_text("Copy the currently loaded project metadata into the focused workflow. This is useful for testing the candidate-ranking layer before a collaborator-supplied filtered list is available.")
                    .clicked()
                {
                    if let (Some(path), Some(meta)) = (this.cohort_metadata_path.clone(), this.cohort_metadata.clone()) {
                        this.focused_metadata_source = Some(MetadataLocator::local(path.clone()));
                        this.focused_metadata_path = Some(path);
                        this.focused_metadata = Some(meta);
                        this.focused_results = None;
                        this.rebuild_focused_mapping();
                    }
                }
                if let Some(path) = &this.focused_metadata_path {
                    ui.monospace(path.file_name().and_then(|x| x.to_str()).unwrap_or("focused metadata"));
                }
            });

            if this.focused_metadata.is_some() {
                let (mut selected_id, columns, row_count, current_id) = this.focused_metadata.as_ref()
                    .map(|m|(m.id_column.clone(),m.columns.clone(),m.rows.len(),m.id_column.clone()))
                    .unwrap_or_default();
                ui.horizontal_wrapped(|ui| {
                    ui.label("Sample ID column:");
                    egui::ComboBox::from_id_salt("focused_metadata_id_column")
                        .selected_text(&selected_id)
                        .show_ui(ui, |ui| {
                            for col in &columns { ui.selectable_value(&mut selected_id, col.clone(), col); }
                        });
                });
                if selected_id != current_id {
                    if let Some(meta) = &mut this.focused_metadata { meta.id_column = selected_id; }
                    this.focused_results = None;
                    this.rebuild_focused_mapping();
                }
                if let Some(mapping) = &this.focused_mapping {
                    ui.label(format!(
                        "Focused metadata rows: {} | matched files: {} | unmatched files: {} | metadata-only samples: {}",
                        row_count, mapping.matched_files, mapping.unmatched_files, mapping.metadata_only_samples
                    ));
                }
            }

            ui.separator();
            let old_preset = this.cohort_preset;
            ui.horizontal_wrapped(|ui| {
                ui.label("Comparison:");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypVsTrueHc, "HYP vs True_HC")
                    .on_hover_text("All HYP=yes samples in the focused metadata versus Phenotype=True_HC.");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypMecfsVsMecfsOnly, "HYP+ME/CFS vs ME/CFS-only")
                    .on_hover_text("Phenotype=HYP+ME/CFS versus Phenotype=ME/CFS without HYP, restricted to the focused metadata rows.");
            });
            if old_preset != this.cohort_preset {
                this.outlier_results = None;
                this.outlier_excluded_ids.clear();
            }
            if let Some(meta) = &this.focused_metadata {
                match build_comparison(meta, this.cohort_preset) {
                    Ok(comp) => ui.label(format!("Direct comparison: {} case samples / {} control samples", comp.case_metadata_rows, comp.control_metadata_rows)),
                    Err(e) => ui.colored_label(egui::Color32::from_rgb(225,120,110), format!("Comparison cannot be built: {e:#}")),
                };
            }

            ui.separator();
            ui.label(egui::RichText::new("Analyses to include").strong());
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut this.cohort_run_gene, "Gene counts").on_hover_text("Primary source for candidate genes and pathway analysis.");
                ui.checkbox(&mut this.cohort_run_transcript, "Transcript counts").on_hover_text("Adds candidate transcript abundance signals and the standard corrected transcript analysis.");
                ui.checkbox(&mut this.cohort_run_bcr, "BCR").on_hover_text("Adds repertoire and composition signals to the focused shortlist.");
                ui.checkbox(&mut this.cohort_run_bcr_isotype, "BCR isotype").on_hover_text("Adds corrected isotype-composition signals.");
                ui.checkbox(&mut this.cohort_run_tcr_alpha, "TCR alpha").on_hover_text("Adds corrected alpha-chain repertoire signals.");
                ui.checkbox(&mut this.cohort_run_tcr_beta, "TCR beta").on_hover_text("Adds corrected beta-chain repertoire signals.");
                ui.checkbox(&mut this.cohort_run_immune, "Immune infiltrate").on_hover_text("Adds corrected immune-cell ssGSEA signals.");
            });
            ui.checkbox(&mut this.focused_run_exploratory_concordance, "Run exploratory expression direction check")
                .on_hover_text("Runs TPM-based exploratory gene/transcript screens on the same focused samples. These results do not increase statistical evidence; they only flag whether the direction agrees with the corrected count analysis.");

            ui.checkbox(&mut this.corrected_run_pathway, "Pathway ORA")
                .on_hover_text("Use the corrected focused gene results with a selected public or custom GMT collection to prioritize pathway-level signals.");
            if this.corrected_run_pathway {
                this.render_gmt_source_selector(ui,"focused");
            }

            egui::CollapsingHeader::new("Candidate shortlist settings")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Top genes:"); ui.add(egui::DragValue::new(&mut this.focused_settings.top_genes).range(3..=100));
                        ui.label("Top transcripts:"); ui.add(egui::DragValue::new(&mut this.focused_settings.top_transcripts).range(3..=100));
                        ui.label("Top pathways:"); ui.add(egui::DragValue::new(&mut this.focused_settings.top_pathways).range(3..=100));
                        ui.label("Top other signals:"); ui.add(egui::DragValue::new(&mut this.focused_settings.top_other).range(5..=100));
                        ui.label("Minimum |adjusted log2FC|:"); ui.add(egui::DragValue::new(&mut this.focused_settings.min_gene_abs_log2fc).range(0.0..=5.0).speed(0.1));
                    });
                    ui.label(egui::RichText::new("Ranking is transparent: corrected BH q-value first, then nominal p-value and effect magnitude. FDR-supported results are labeled separately from promising/suggestive ranked signals so a small cohort does not force an all-or-nothing interpretation.").small().weak());
                });

            let ready = this.cohort_scan.is_some() && this.focused_metadata.is_some() && !this.cohort_running
                && (this.cohort_run_gene || this.cohort_run_transcript || this.cohort_run_bcr || this.cohort_run_bcr_isotype || this.cohort_run_tcr_alpha || this.cohort_run_tcr_beta || this.cohort_run_immune || this.corrected_run_pathway);
            if ui.add_enabled(ready, egui::Button::new("Run focused candidate workflow"))
                .on_hover_text("Run the selected corrected analyses on the filtered metadata, optionally run exploratory expression direction checks, then build a concise candidate-gene/pathway/other-signal shortlist.")
                .clicked()
            {
                this.start_focused_analysis();
            }
            if this.cohort_running { ui.add(egui::ProgressBar::new(this.cohort_progress).show_percentage().animate(true)); }
            ui.label(&this.cohort_status);
        });
    }

    fn render_focused_results_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "4. Focused candidate signals", |this, ui| {
            let Some(summary)=this.focused_results.clone() else { return; };
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} vs {} — preliminary shortlist",summary.case_label,summary.control_label)).strong());
                if ui.button("Export candidate CSVs…").on_hover_text("Export concise CSV files for candidate genes, pathways, transcripts, and other modality signals, plus interpretation notes.").clicked() {
                    let mut dialog=FileDialog::new(); if let Some(root)=&this.cohort_root{dialog=dialog.set_directory(root);}
                    if let Some(folder)=dialog.pick_folder(){
                        match this.save_focused_result_tables(&folder,&summary){
                            Ok(path)=>this.cohort_status=format!("Saved focused candidate tables to {}",path.display()),
                            Err(e)=>this.cohort_status=format!("Could not save focused candidate tables: {e:#}")
                        };
                    }
                }
                if ui.button("Generate focused HTML report…").on_hover_text("Generate a concise standalone report intended for a less technical audience. It emphasizes preliminary candidate genes/pathways, clear interpretation, and limitations rather than exhaustive analysis tables.").clicked() {
                    let safe=format!("{}_vs_{}",summary.case_label,summary.control_label).chars().map(|c|if c.is_ascii_alphanumeric(){c}else{'_'}).collect::<String>();
                    let mut dialog=FileDialog::new().add_filter("HTML report", &["html"]).set_file_name(format!("4c_focused_candidates_{safe}.html")); if let Some(root)=&this.cohort_root{dialog=dialog.set_directory(root);}
                    if let Some(path)=dialog.save_file(){
                        let inputs=FocusedReportInputs{summary:&summary,outlier_qc:this.focused_outlier_snapshot.as_ref()};
                        match generate_focused_candidate_html_report(&path,inputs){Ok(())=>this.cohort_status=format!("Generated focused candidate report: {}",path.display()),Err(e)=>this.cohort_status=format!("Could not generate focused report: {e:#}")};
                    }
                }
            });
            ui.label(format!("Focused metadata: {} | {} case / {} control",summary.metadata_path.file_name().and_then(|x|x.to_str()).unwrap_or("metadata"),summary.n_case,summary.n_control));
            if let Some(qc)=&this.focused_outlier_snapshot {
                ui.colored_label(egui::Color32::from_rgb(230,170,80),format!("QC-filtered focused run: {} reviewed sample exclusion(s) applied (case {} → {}, control {} → {}). The exact audit is included in exports and the HTML report.",qc.excluded_ids.len(),qc.original_case_n,qc.filtered_case_n,qc.original_control_n,qc.filtered_control_n));
            }
            ui.colored_label(egui::Color32::from_rgb(230,190,90),"These are preliminary signals for follow-up, not validated biomarkers.");

            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut this.focused_result_view,FocusedResultView::Overview,"Overview");
                ui.selectable_value(&mut this.focused_result_view,FocusedResultView::Genes,format!("Genes ({})",summary.genes.len()));
                if !summary.pathways.is_empty(){ui.selectable_value(&mut this.focused_result_view,FocusedResultView::Pathways,format!("Pathways ({})",summary.pathways.len()));}
                if !summary.other.is_empty(){ui.selectable_value(&mut this.focused_result_view,FocusedResultView::Other,format!("Other signals ({})",summary.other.len()));}
                if !summary.transcripts.is_empty(){ui.selectable_value(&mut this.focused_result_view,FocusedResultView::Transcripts,format!("Transcripts ({})",summary.transcripts.len()));}
            });
            ui.separator();

            match this.focused_result_view {
                FocusedResultView::Overview => {
                    let fdr_genes=summary.genes.iter().filter(|r|r.q_value.is_finite()&&r.q_value<=0.05).count();
                    let fdr_pathways=summary.pathways.iter().filter(|r|r.q_value.is_finite()&&r.q_value<=0.05).count();
                    ui.heading("Presentation-ready summary");
                    ui.label(format!("{} shortlisted genes ({} FDR-supported) and {} shortlisted pathways ({} FDR-supported).",summary.genes.len(),fdr_genes,summary.pathways.len(),fdr_pathways));
                    if let Some(top)=summary.genes.first(){ui.label(format!("Top ranked gene: {} — adjusted log2FC {:.2}, BH q {:.2e}.",top.gene,top.adjusted_log2fc,top.q_value));}
                    if let Some(top)=summary.pathways.first(){ui.label(format!("Top ranked pathway: {} — odds ratio {:.2}, BH q {:.2e}.",top.pathway,top.odds_ratio,top.q_value));}
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Interpretation").strong());
                    for note in &summary.notes { ui.label(format!("• {note}")); }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Suggested wording for a nontechnical audience").strong());
                    ui.label("“Using a simplified one-sample-per-individual comparison, we identified a small set of genes and biological pathways that show the strongest preliminary differences between the groups. These signals are not yet validated biomarkers, but they provide specific candidates for follow-up and independent replication.”");
                }
                FocusedResultView::Genes => {
                    ui.label("Corrected count-model results drive the ranking. Exploratory TPM direction, when available, is shown only as method concordance. Pathway support counts q≤0.10 enriched pathways containing the gene.");
                    fixed_table_viewport(ui,"focused_gene_candidates_v09",620.0,1500.0,|ui|{
                        egui::Grid::new("focused_gene_candidates_grid_v09").striped(true).show(ui,|ui|{
                            for h in ["Rank","Gene","Gene ID","Adj log2FC","p","BH q","Case mean","Control mean","TPM direction","Pathway support","Evidence"]{ui.label(egui::RichText::new(h).strong());}ui.end_row();
                            for r in &summary.genes{
                                ui.label(r.rank.to_string());ui.monospace(&r.gene);ui.monospace(&r.gene_id);ui.label(format!("{:.3}",r.adjusted_log2fc));ui.label(format!("{:.2e}",r.p_value));ui.label(format!("{:.2e}",r.q_value));ui.label(format!("{:.3}",r.case_mean));ui.label(format!("{:.3}",r.control_mean));
                                ui.label(match r.direction_concordant{Some(true)=>"same",Some(false)=>"opposite",None=>"—"});
                                ui.label(if r.pathway_support_count==0{"—".to_string()}else{format!("{}",r.pathway_support_count)}).on_hover_text(if r.pathway_support.is_empty(){"No q≤0.10 pathway support recorded".to_string()}else{r.pathway_support.clone()});
                                ui.label(&r.evidence);ui.end_row();
                            }
                        });
                    });
                }
                FocusedResultView::Pathways => {
                    ui.label("Pathways are ranked by corrected over-representation BH q-value. Closely related gene sets can share many genes, so interpret clusters of pathways as broader biological themes.");
                    fixed_table_viewport(ui,"focused_pathway_candidates_v09",620.0,1500.0,|ui|{
                        egui::Grid::new("focused_pathway_candidates_grid_v09").striped(true).show(ui,|ui|{
                            for h in ["Rank","Pathway","Odds ratio","Overlap","Set size","p","BH q","Evidence","Overlap genes"]{ui.label(egui::RichText::new(h).strong());}ui.end_row();
                            for r in &summary.pathways{ui.label(r.rank.to_string());ui.monospace(&r.pathway);ui.label(format!("{:.3}",r.odds_ratio));ui.label(r.overlap.to_string());ui.label(r.measured_set_size.to_string());ui.label(format!("{:.2e}",r.p_value));ui.label(format!("{:.2e}",r.q_value));ui.label(&r.evidence);ui.label(compact_comma_list(&r.overlap_genes,8,120)).on_hover_text(&r.overlap_genes);ui.end_row();}
                        });
                    });
                }
                FocusedResultView::Other => {
                    ui.label("Effects are on each modality's corrected analysis scale. Compare significance and direction within a modality/family rather than comparing raw effect magnitudes between, for example, TCR CLR effects and immune ssGSEA coefficients.");
                    fixed_table_viewport(ui,"focused_other_candidates_v09",620.0,1500.0,|ui|{
                        egui::Grid::new("focused_other_candidates_grid_v09").striped(true).show(ui,|ui|{
                            for h in ["Rank","Modality","Family","Feature","ID","Effect","p","BH q","Case mean","Control mean","Evidence"]{ui.label(egui::RichText::new(h).strong());}ui.end_row();
                            for r in &summary.other{ui.label(r.rank.to_string());ui.label(&r.modality);ui.label(&r.family);ui.monospace(&r.feature);ui.monospace(&r.secondary_id);ui.label(format!("{:.3}",r.effect));ui.label(format!("{:.2e}",r.p_value));ui.label(format!("{:.2e}",r.q_value));ui.label(format!("{:.3}",r.case_mean));ui.label(format!("{:.3}",r.control_mean));ui.label(&r.evidence);ui.end_row();}
                        });
                    });
                }
                FocusedResultView::Transcripts => {
                    ui.label("Transcript abundance candidates are shown separately from gene candidates. Treat these as isoform follow-up leads rather than evidence of differential transcript usage unless the dedicated DTU analysis also supports the gene.");
                    fixed_table_viewport(ui,"focused_transcript_candidates_v09",620.0,1450.0,|ui|{
                        egui::Grid::new("focused_transcript_candidates_grid_v09").striped(true).show(ui,|ui|{
                            for h in ["Rank","Transcript","Gene ID","Adjusted effect","p","BH q","Case mean","Control mean","TPM direction","Evidence"]{ui.label(egui::RichText::new(h).strong());}ui.end_row();
                            for r in &summary.transcripts{ui.label(r.rank.to_string());ui.monospace(&r.gene);ui.monospace(&r.gene_id);ui.label(format!("{:.3}",r.adjusted_log2fc));ui.label(format!("{:.2e}",r.p_value));ui.label(format!("{:.2e}",r.q_value));ui.label(format!("{:.3}",r.case_mean));ui.label(format!("{:.3}",r.control_mean));ui.label(match r.direction_concordant{Some(true)=>"same",Some(false)=>"opposite",None=>"—"});ui.label(&r.evidence);ui.end_row();}
                        });
                    });
                }
            }
        });
    }

    fn render_diagnostics_analysis_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. PBMC / confounding diagnostics", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "Use this layer when a disease signal may be driven by sample source, unusual PBMC composition, a historical batch/family, a few extreme samples, or low-count genes. The diagnostics are deliberately separate from the disease model: they help decide which samples/signals deserve sensitivity testing rather than automatically declaring a sample invalid.",
                )
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Disease comparison:");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypVsTrueHc, "HYP vs True_HC");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypMecfsVsMecfsOnly, "HYP+ME/CFS vs ME/CFS-only");
            });
            let columns = this.cohort_metadata.as_ref().map(|m| m.columns.clone()).unwrap_or_default();
            if !columns.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Provenance / repeated-measure columns").strong());
                ui.horizontal_wrapped(|ui| {
                    diagnostics_column_combo(ui, "diag_subject", "Subject:", &columns, &mut this.corrected_subject_column);
                    diagnostics_column_combo(ui, "diag_batch", "Batch / collection period:", &columns, &mut this.diagnostics_batch_column);
                    diagnostics_column_combo(ui, "diag_family", "Family:", &columns, &mut this.diagnostics_family_column);
                    diagnostics_column_combo(ui, "diag_sensitivity", "Leave-level-out column:", &columns, &mut this.diagnostics_sensitivity_column);
                });
                ui.label(egui::RichText::new("The leave-level-out diagnostic recomputes the TPM effect after removing each level of the selected batch/family/year column in turn. It is a robustness diagnostic, not a replacement for the corrected count model.").small().weak());
            }
            ui.separator();
            ui.columns(2, |cols| {
                cols[0].label(egui::RichText::new("Known non-PBMC / wrong-source sample IDs").strong());
                cols[0].add(egui::TextEdit::multiline(&mut this.diagnostics_known_non_pbmc_text).desired_rows(4).hint_text("One per line, or comma/space separated\nExample: RNA_NGC-..."));
                cols[0].label(egui::RichText::new("These are excluded from the QC-restricted sensitivity model and ordinary-sample control pool. Use this for samples whose source/tissue is already known to be inappropriate for the PBMC comparison.").small().weak());

                cols[1].label(egui::RichText::new("Suspect / unusual sample IDs").strong());
                cols[1].add(egui::TextEdit::multiline(&mut this.diagnostics_suspect_ids_text).desired_rows(4).hint_text("Paste samples with the unusual ECM/composition pattern"));
                cols[1].label(egui::RichText::new("These define the optional 'unusual vs ordinary' expression screen. They can also be excluded in the QC-restricted sensitivity model without deleting them from the project.").small().weak());
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut this.diagnostics_exclude_suspects, "Exclude suspect IDs in QC sensitivity")
                    .on_hover_text("Keeps the original disease analysis intact but reruns a sensitivity model after removing the user-marked suspect samples.");
                ui.checkbox(&mut this.diagnostics_auto_exclude_expression_outliers, "Also exclude strong transcriptome PCA outliers")
                    .on_hover_text("Automatically adds samples with a robust PC1/PC2 outlier score above the configured threshold to the sensitivity-only exclusion set. Review these flags rather than treating them as automatic QC failures.");
                ui.checkbox(&mut this.diagnostics_auto_exclude_immune_outliers, "Also exclude strong immune-signature PCA outliers")
                    .on_hover_text("Off by default because immune composition can be biologically meaningful. Enable only as a sensitivity analysis.");
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Robust outlier threshold:");
                ui.add(egui::DragValue::new(&mut this.diagnostics_outlier_z).range(2.5..=10.0).speed(0.25));
                ui.label("Top candidate genes to diagnose:");
                ui.add(egui::DragValue::new(&mut this.diagnostics_top_candidates).range(10..=250).speed(5));
            });
            ui.label(egui::RichText::new("The run performs: whole-transcriptome PCA on top-variable log2(TPM+1) genes; sample-level library/detection summaries; immune-signature PCA and a myeloid-vs-lymphoid imbalance diagnostic when immune files exist; suspect-vs-ordinary expression screening; baseline corrected gene modelling; QC-restricted and immune-PC-adjusted sensitivity models; low-count, top-two-sample contribution, leave-one-sample-out, and leave-batch/family-out candidate diagnostics; and longitudinal within-subject PCA-distance summaries.").small().weak());
            if ui.add_enabled(!this.cohort_running, egui::Button::new("Run PBMC / confounding diagnostics"))
                .on_hover_text("Runs the diagnostics against the merged gene matrix and current cohort metadata. The original files/results are not modified.")
                .clicked() {
                this.start_diagnostics_analysis();
            }
            if this.cohort_running {
                ui.add(egui::ProgressBar::new(this.cohort_progress).show_percentage());
            }
            if !this.cohort_status.is_empty() { ui.label(egui::RichText::new(&this.cohort_status).small().weak()); }
        });
    }

    fn start_diagnostics_analysis(&mut self) {
        let (Some(scan), Some(metadata)) = (self.cohort_scan.clone(), self.cohort_metadata.clone())
        else {
            self.cohort_status = "Load a project and metadata first.".to_string();
            return;
        };
        let Some(merged_gene_tsv) = scan.merged_gene_tsv.clone() else {
            self.cohort_status = "PBMC diagnostics require a merged gene-expression TSV containing Raw Count and TPM columns.".to_string();
            return;
        };
        let comparison = match build_comparison(&metadata, self.cohort_preset) {
            Ok(c) => c,
            Err(e) => {
                self.cohort_status = format!("Could not build comparison: {e:#}");
                return;
            }
        };
        let mut covariates = self
            .corrected_covariates
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        covariates.sort();
        let spec = CorrectedModelSpec {
            subject_column: self.corrected_subject_column.clone(),
            covariates,
            permutations: self.corrected_permutations,
            rarefaction_reps: self.corrected_rarefaction_reps,
            rarefaction_cap: self.corrected_rarefaction_cap,
            ambiguous_policy: self.corrected_ambiguous_policy,
            exclude_undetermined_isotype: self.corrected_exclude_undetermined,
            ..CorrectedModelSpec::default()
        };
        let config = PbmcDiagnosticsConfig {
            subject_column: self.corrected_subject_column.clone(),
            batch_column: self.diagnostics_batch_column.clone(),
            family_column: self.diagnostics_family_column.clone(),
            sensitivity_column: self.diagnostics_sensitivity_column.clone(),
            known_non_pbmc_ids: parse_sample_id_text(&self.diagnostics_known_non_pbmc_text),
            suspect_ids: parse_sample_id_text(&self.diagnostics_suspect_ids_text),
            exclude_suspects_in_sensitivity: self.diagnostics_exclude_suspects,
            auto_exclude_transcriptome_outliers: self.diagnostics_auto_exclude_expression_outliers,
            auto_exclude_immune_outliers: self.diagnostics_auto_exclude_immune_outliers,
            top_candidates: self.diagnostics_top_candidates,
            outlier_z_threshold: self.diagnostics_outlier_z,
            ..PbmcDiagnosticsConfig::default()
        };
        let files = scan.files.clone();
        let (tx, rx) = mpsc::channel();
        self.cohort_receiver = Some(rx);
        self.cohort_running = true;
        self.cohort_progress = 0.0;
        self.cohort_status = "Running PBMC/source-composition diagnostics…".to_string();
        self.diagnostics_results = None;
        std::thread::spawn(move || {
            match run_pbmc_diagnostics(
                &merged_gene_tsv,
                &files,
                &metadata,
                &comparison,
                &spec,
                &config,
                |done, total, msg| {
                    let frac = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    let _ = tx.send(CohortWorkerMessage::Progress(frac, msg.to_string()));
                },
            ) {
                Ok(result) => {
                    let _ = tx.send(CohortWorkerMessage::DiagnosticsDone(result));
                }
                Err(e) => {
                    let _ = tx.send(CohortWorkerMessage::Progress(
                        1.0,
                        format!("PBMC diagnostics failed: {e:#}"),
                    ));
                    let fallback = PbmcDiagnosticsResult {
                        merged_gene_tsv,
                        comparison,
                        config,
                        pca: None,
                        sample_qc: Vec::new(),
                        longitudinal_qc: Vec::new(),
                        source_effect: None,
                        baseline_gene: None,
                        qc_restricted_gene: None,
                        immune_adjusted_gene: None,
                        candidate_stability: Vec::new(),
                        sensitivity_excluded_ids: Vec::new(),
                        warnings: vec![format!("{e:#}")],
                    };
                    let _ = tx.send(CohortWorkerMessage::DiagnosticsDone(fallback));
                }
            }
        });
    }

    fn render_diagnostics_results_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "4. PBMC / confounding diagnostic results", |this, ui| {
            let Some(result) = this.diagnostics_results.clone() else { return; };
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} — diagnostics", result.comparison.name)).strong());
                if ui.button("Export diagnostic tables…")
                    .on_hover_text("Exports the sample QC audit, candidate robustness table, per-sample candidate values, longitudinal QC, suspect-vs-ordinary screen, and sensitivity-model gene tables.")
                    .clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(root) = &this.cohort_root { dialog = dialog.set_directory(root); }
                    if let Some(folder) = dialog.pick_folder() {
                        let out = folder.join("pbmc_confounding_diagnostics");
                        match write_pbmc_diagnostics_tables(&out, &result) {
                            Ok(path) => this.cohort_status = format!("Saved PBMC/confounding diagnostic tables to {}", path.display()),
                            Err(e) => this.cohort_status = format!("Could not save diagnostics: {e:#}"),
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut this.diagnostics_result_view, DiagnosticsResultView::Overview, "Overview");
                ui.selectable_value(&mut this.diagnostics_result_view, DiagnosticsResultView::SampleQc, "Sample QC / PCA");
                ui.selectable_value(&mut this.diagnostics_result_view, DiagnosticsResultView::CandidateRobustness, "Candidate robustness");
                ui.selectable_value(&mut this.diagnostics_result_view, DiagnosticsResultView::SourceEffect, "Suspect vs ordinary");
                ui.selectable_value(&mut this.diagnostics_result_view, DiagnosticsResultView::Sensitivity, "Sensitivity models");
            });
            ui.separator();
            match this.diagnostics_result_view {
                DiagnosticsResultView::Overview => this.render_diagnostics_overview(ui, &result),
                DiagnosticsResultView::SampleQc => this.render_diagnostics_sample_qc(ui, &result),
                DiagnosticsResultView::CandidateRobustness => this.render_diagnostics_candidates(ui, &result),
                DiagnosticsResultView::SourceEffect => this.render_diagnostics_source_effect(ui, &result),
                DiagnosticsResultView::Sensitivity => this.render_diagnostics_sensitivity(ui, &result),
            }
        });
    }

    fn render_diagnostics_overview(&self, ui: &mut egui::Ui, result: &PbmcDiagnosticsResult) {
        let flagged = result
            .sample_qc
            .iter()
            .filter(|r| !r.flags.is_empty())
            .count();
        let transcriptome = result
            .sample_qc
            .iter()
            .filter(|r| r.auto_expression_outlier)
            .count();
        let immune = result
            .sample_qc
            .iter()
            .filter(|r| r.auto_immune_outlier)
            .count();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Expression samples audited: {}",
                result.sample_qc.len()
            ));
            ui.separator();
            ui.label(format!("Flagged for review: {flagged}"));
            ui.separator();
            ui.label(format!("Transcriptome PCA outliers: {transcriptome}"));
            ui.separator();
            ui.label(format!("Immune-signature PCA outliers: {immune}"));
            ui.separator();
            ui.label(format!(
                "Sensitivity-only exclusions: {}",
                result.sensitivity_excluded_ids.len()
            ));
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("How to use this page").strong());
        ui.label("A flag is a prompt for review, not an automatic statement that a sample is bad. The strongest evidence for a source/processing problem is agreement between independent clues: known tissue provenance, a whole-transcriptome outlier position, unusual immune-signature structure, historical batch/family concentration, and within-person inconsistency across timepoints.");
        ui.add_space(6.0);
        if !result.warnings.is_empty() {
            egui::CollapsingHeader::new(format!("Warnings ({})", result.warnings.len()))
                .default_open(true)
                .show(ui, |ui| {
                    for warning in &result.warnings {
                        ui.label(format!("• {warning}"));
                    }
                });
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Samples currently flagged for review").strong());
        fixed_table_viewport(ui, "diagnostics_overview_flagged", 460.0, 1500.0, |ui| {
            egui::Grid::new("diagnostics_overview_flagged_grid")
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "Sample",
                        "Group",
                        "Phenotype",
                        "Subject",
                        "Batch",
                        "Family",
                        "PCA outlier",
                        "Immune outlier",
                        "Myeloid-lymphoid",
                        "Sensitivity excluded",
                        "Flags",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in result.sample_qc.iter().filter(|r| !r.flags.is_empty()) {
                        ui.monospace(&r.sample_id);
                        ui.label(&r.comparison_group);
                        ui.label(&r.phenotype);
                        ui.monospace(&r.subject);
                        ui.label(&r.batch);
                        ui.label(&r.family);
                        ui.label(format_float_or_dash(r.transcriptome_outlier_score));
                        ui.label(format_float_or_dash(r.immune_outlier_score));
                        ui.label(format_float_or_dash(r.monocyte_lymphoid_signature));
                        ui.label(if r.sensitivity_excluded { "yes" } else { "no" });
                        ui.label(r.flags.join("; "));
                        ui.end_row();
                    }
                });
        });
    }

    fn render_diagnostics_sample_qc(&mut self, ui: &mut egui::Ui, result: &PbmcDiagnosticsResult) {
        ui.label(egui::RichText::new("Whole-transcriptome sample structure").strong());
        ui.label(egui::RichText::new("PCA is calculated from top-variable log2(TPM+1) genes across the merged expression samples. Extreme separation can indicate tissue/source, cell-composition, batch, or other global structure; it is not automatically a disease effect.").small().weak());
        if let Some(pca) = &result.pca {
            self.render_corrected_ordination(ui, pca, "diagnostics_whole_transcriptome_pca");
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Sample audit").strong());
        fixed_table_viewport(ui, "diagnostics_sample_qc_table", 620.0, 2200.0, |ui| {
            egui::Grid::new("diagnostics_sample_qc_grid")
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "Sample",
                        "Group",
                        "Phenotype",
                        "Subject",
                        "Batch",
                        "Family",
                        "Library raw counts",
                        "Detected genes",
                        "Genes ≥5",
                        "Genes ≥10",
                        "PC1",
                        "PC2",
                        "PCA outlier",
                        "Immune PC1",
                        "Immune PC2",
                        "Immune outlier",
                        "Myeloid-lymphoid",
                        "Excluded sensitivity",
                        "Flags",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in &result.sample_qc {
                        ui.monospace(&r.sample_id);
                        ui.label(&r.comparison_group);
                        ui.label(&r.phenotype);
                        ui.monospace(&r.subject);
                        ui.label(&r.batch);
                        ui.label(&r.family);
                        ui.label(format!("{:.0}", r.library_raw_count));
                        ui.label(r.detected_genes.to_string());
                        ui.label(r.genes_ge5.to_string());
                        ui.label(r.genes_ge10.to_string());
                        ui.label(format_float_or_dash(r.pca1));
                        ui.label(format_float_or_dash(r.pca2));
                        ui.label(format_float_or_dash(r.transcriptome_outlier_score));
                        ui.label(format_float_or_dash(r.immune_pc1));
                        ui.label(format_float_or_dash(r.immune_pc2));
                        ui.label(format_float_or_dash(r.immune_outlier_score));
                        ui.label(format_float_or_dash(r.monocyte_lymphoid_signature));
                        ui.label(if r.sensitivity_excluded { "yes" } else { "no" });
                        ui.label(r.flags.join("; "));
                        ui.end_row();
                    }
                });
        });
        if !result.longitudinal_qc.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Within-subject longitudinal consistency").strong());
            ui.label(egui::RichText::new("Large within-subject PCA distance means that different timepoints from the same person occupy very different whole-transcriptome positions. That can be biologically real, but it is particularly informative when an early historical sample is extreme and later samples return to the main PBMC cluster.").small().weak());
            fixed_table_viewport(ui, "diagnostics_longitudinal_table", 360.0, 1200.0, |ui| {
                egui::Grid::new("diagnostics_longitudinal_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        for h in [
                            "Subject",
                            "N samples",
                            "Max PCA distance",
                            "Samples",
                            "Batches",
                            "Flagged samples",
                        ] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for r in &result.longitudinal_qc {
                            ui.monospace(&r.subject);
                            ui.label(r.n_samples.to_string());
                            ui.label(format!("{:.3}", r.max_pca_distance));
                            ui.label(&r.samples);
                            ui.label(&r.batches);
                            ui.label(&r.flagged_samples);
                            ui.end_row();
                        }
                    });
            });
        }
    }

    fn render_diagnostics_candidates(&mut self, ui: &mut egui::Ui, result: &PbmcDiagnosticsResult) {
        ui.label(egui::RichText::new("Candidate robustness diagnostics").strong());
        ui.label("This table asks whether leading corrected gene signals remain credible after looking directly at count sparsity, sample concentration, leave-one-sample-out behavior, QC restriction, immune-composition adjustment, and optional leave-batch/family-out sensitivity. The regularized effect is a simple normal-prior diagnostic shrinkage estimate; it is not DESeq2/apeglm shrinkage.");
        ui.add_space(6.0);
        fixed_table_viewport(ui, "diagnostics_candidates_table", 520.0, 2600.0, |ui| {
            egui::Grid::new("diagnostics_candidates_grid")
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "Gene",
                        "Plot",
                        "Baseline log2FC",
                        "BH q",
                        "Regularized log2FC",
                        "Median raw case",
                        "Median raw ctrl",
                        "Detected case",
                        "Detected ctrl",
                        "≥10 case",
                        "≥10 ctrl",
                        "Top2 share case",
                        "Top2 share ctrl",
                        "LOO effect range",
                        "Leave-level range",
                        "QC-restricted log2FC",
                        "QC q",
                        "Immune-adjusted log2FC",
                        "Immune q",
                        "Source-effect log2FC",
                        "Source q",
                        "Classification",
                        "Flags",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in &result.candidate_stability {
                        ui.monospace(&r.gene);
                        if ui.small_button("Plot").clicked() {
                            self.diagnostics_selected_gene = Some(r.gene.clone());
                        }
                        ui.label(format!("{:.3}", r.baseline_log2fc));
                        ui.label(format!("{:.2e}", r.baseline_q));
                        ui.label(format!("{:.3}", r.regularized_log2fc));
                        ui.label(format!("{:.1}", r.case_median_raw));
                        ui.label(format!("{:.1}", r.control_median_raw));
                        ui.label(format!("{:.0}%", 100.0 * r.detected_case));
                        ui.label(format!("{:.0}%", 100.0 * r.detected_control));
                        ui.label(format!("{:.0}%", 100.0 * r.count_ge10_case));
                        ui.label(format!("{:.0}%", 100.0 * r.count_ge10_control));
                        ui.label(format!("{:.0}%", 100.0 * r.top2_contribution_case));
                        ui.label(format!("{:.0}%", 100.0 * r.top2_contribution_control));
                        ui.label(format_range(r.loo_log2fc_min, r.loo_log2fc_max));
                        ui.label(format_range(
                            r.leave_level_log2fc_min,
                            r.leave_level_log2fc_max,
                        ));
                        ui.label(format_float_or_dash(r.qc_restricted_log2fc));
                        ui.label(format_sci_or_dash(r.qc_restricted_q));
                        ui.label(format_float_or_dash(r.immune_adjusted_log2fc));
                        ui.label(format_sci_or_dash(r.immune_adjusted_q));
                        ui.label(format_float_or_dash(r.source_effect_log2fc));
                        ui.label(format_sci_or_dash(r.source_effect_q));
                        ui.label(&r.classification);
                        ui.label(r.flags.join("; "));
                        ui.end_row();
                    }
                });
        });
        let selected = self
            .diagnostics_selected_gene
            .clone()
            .or_else(|| result.candidate_stability.first().map(|r| r.gene.clone()));
        if let Some(gene) = selected {
            if let Some(row) = result.candidate_stability.iter().find(|r| r.gene == gene) {
                ui.add_space(8.0);
                self.render_diagnostic_candidate_plot(ui, row);
            }
        }
    }

    fn render_diagnostic_candidate_plot(&self, ui: &mut egui::Ui, row: &CandidateStabilityRow) {
        ui.label(egui::RichText::new(format!("{} — per-sample log2(TPM+1)", row.gene)).strong());
        let mut control = Vec::new();
        let mut case = Vec::new();
        let mut flagged = Vec::new();
        let mut hover = Vec::new();
        for p in &row.sample_points {
            let base_x = if p.group == "Case" { 1.0 } else { 0.0 };
            let jitter = deterministic_jitter(&p.sample_id) * 0.22;
            let x = base_x + jitter;
            let y = (p.tpm.max(0.0) + 1.0).log2();
            let label = format!(
                "{}\n{}\nRaw count: {:.0}\nTPM: {:.4}\nlog2(TPM+1): {:.3}\nSubject: {}\nBatch: {}\nFamily: {}\nSuspect: {}\nSensitivity excluded: {}",
                p.sample_id,
                p.group,
                p.raw_count,
                p.tpm,
                y,
                p.subject,
                p.batch,
                p.family,
                p.suspect,
                p.sensitivity_excluded
            );
            hover.push((x, y, label));
            if p.suspect || p.sensitivity_excluded {
                flagged.push([x, y]);
            } else if p.group == "Case" {
                case.push([x, y]);
            } else {
                control.push([x, y]);
            }
        }
        let formatter = hover.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("diagnostic_candidate_profile")
            .height(380.0)
            .x_axis_label("Control ← group → Case")
            .y_axis_label("log2(TPM+1)")
            .label_formatter(move |h| match h {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_hover_label(position.x, position.y, &formatter)
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                if !control.is_empty() {
                    plot_ui.points(
                        Points::new("Control", control)
                            .color(egui::Color32::from_rgb(105, 165, 225))
                            .radius(4.0),
                    );
                }
                if !case.is_empty() {
                    plot_ui.points(Points::new("Case", case).color(accent).radius(4.0));
                }
                if !flagged.is_empty() {
                    plot_ui.points(
                        Points::new("Suspect / sensitivity excluded", flagged)
                            .color(egui::Color32::from_rgb(230, 150, 70))
                            .radius(5.5),
                    );
                }
            });
        ui.label(egui::RichText::new(format!("Baseline adjusted log2FC {:.3} (q {:.2e}); QC-restricted {:.3} (q {}); immune-adjusted {:.3} (q {}). Top-two sample contribution: case {:.0}%, control {:.0}%.",row.baseline_log2fc,row.baseline_q,row.qc_restricted_log2fc,format_sci_or_dash(row.qc_restricted_q),row.immune_adjusted_log2fc,format_sci_or_dash(row.immune_adjusted_q),100.0*row.top2_contribution_case,100.0*row.top2_contribution_control)).small().weak());
    }

    fn render_diagnostics_source_effect(&self, ui: &mut egui::Ui, result: &PbmcDiagnosticsResult) {
        ui.label(egui::RichText::new("User-marked unusual samples vs ordinary samples").strong());
        ui.label("This comparison intentionally ignores disease status. Its purpose is to characterize the transcriptomic program that distinguishes the suspicious/source-shifted samples themselves. If the same genes dominate both this screen and the disease comparison, that is strong evidence that the disease result is entangled with sample source/composition.");
        let Some(source) = &result.source_effect else {
            ui.colored_label(egui::Color32::from_rgb(230,170,70),"No source-effect screen was run. Provide at least two suspect sample IDs and at least two ordinary samples.");
            return;
        };
        fixed_table_viewport(ui, "diagnostics_source_effect_table", 620.0, 1700.0, |ui| {
            egui::Grid::new("diagnostics_source_effect_grid")
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "Gene",
                        "Gene ID",
                        "log2 mean FC",
                        "Cliff δ",
                        "p",
                        "BH q",
                        "Median unusual",
                        "Median ordinary",
                        "Detected unusual",
                        "Detected ordinary",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in source.tests.iter().take(250) {
                        ui.monospace(&r.feature);
                        ui.monospace(&r.secondary_id);
                        ui.label(format!("{:.3}", r.effect));
                        ui.label(format!("{:.3}", r.cliffs_delta));
                        ui.label(format!("{:.2e}", r.p_value));
                        ui.label(format!("{:.2e}", r.q_value));
                        ui.label(format!("{:.4}", r.median_case));
                        ui.label(format!("{:.4}", r.median_control));
                        ui.label(format!("{:.0}%", 100.0 * r.detection_case.unwrap_or(0.0)));
                        ui.label(format!(
                            "{:.0}%",
                            100.0 * r.detection_control.unwrap_or(0.0)
                        ));
                        ui.end_row();
                    }
                });
        });
    }

    fn render_diagnostics_sensitivity(&self, ui: &mut egui::Ui, result: &PbmcDiagnosticsResult) {
        ui.label(egui::RichText::new("Disease-model sensitivity checks").strong());
        ui.label("The same disease comparison is fit in three ways when data permit: the baseline corrected model; a QC-restricted model after removing the sensitivity-only exclusion set; and an immune-PC-adjusted model that adds the first two immune-signature PCs as covariates. A convincing disease signal should ideally retain direction and a comparable effect across these views.");
        ui.add_space(6.0);
        ui.label(format!(
            "Sensitivity-only exclusions ({}): {}",
            result.sensitivity_excluded_ids.len(),
            if result.sensitivity_excluded_ids.is_empty() {
                "none".to_string()
            } else {
                result.sensitivity_excluded_ids.join(", ")
            }
        ));
        ui.add_space(6.0);
        fixed_table_viewport(ui, "diagnostics_sensitivity_table", 620.0, 1900.0, |ui| {
            egui::Grid::new("diagnostics_sensitivity_grid")
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "Gene",
                        "Baseline log2FC",
                        "Baseline q",
                        "QC-restricted log2FC",
                        "QC q",
                        "Immune-adjusted log2FC",
                        "Immune q",
                        "Regularized log2FC",
                        "LOO range",
                        "Leave-level range",
                        "Classification",
                        "Flags",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in &result.candidate_stability {
                        ui.monospace(&r.gene);
                        ui.label(format!("{:.3}", r.baseline_log2fc));
                        ui.label(format!("{:.2e}", r.baseline_q));
                        ui.label(format_float_or_dash(r.qc_restricted_log2fc));
                        ui.label(format_sci_or_dash(r.qc_restricted_q));
                        ui.label(format_float_or_dash(r.immune_adjusted_log2fc));
                        ui.label(format_sci_or_dash(r.immune_adjusted_q));
                        ui.label(format!("{:.3}", r.regularized_log2fc));
                        ui.label(format_range(r.loo_log2fc_min, r.loo_log2fc_max));
                        ui.label(format_range(
                            r.leave_level_log2fc_min,
                            r.leave_level_log2fc_max,
                        ));
                        ui.label(&r.classification);
                        ui.label(r.flags.join("; "));
                        ui.end_row();
                    }
                });
        });
    }

    fn render_corrected_analysis_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. Corrected / standard analysis", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "This layer is designed for the more rigorous pass: repeated samples can be clustered by subject, selected metadata covariates are included in the model, repertoire diversity is compared at a common depth, and repertoire/isotype compositions are analyzed in CLR/Aitchison space.",
                )
                .weak(),
            );
            if this.outlier_apply_to_corrected && !this.outlier_excluded_ids.is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 170, 70),
                    format!("Outlier sensitivity filter is ON: {} reviewed sample(s) will be excluded from this corrected run. Keep an inclusive run for comparison.", this.outlier_excluded_ids.len()),
                );
            }
            let old_preset = this.cohort_preset;
            ui.horizontal_wrapped(|ui| {
                ui.label("Comparison:");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypVsTrueHc, "HYP vs True_HC")
                    .on_hover_text("All HYP=yes samples versus true healthy controls.");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypMecfsVsMecfsOnly, "HYP+ME/CFS vs ME/CFS-only")
                    .on_hover_text("HYP+ME/CFS versus ME/CFS samples without HYP.");
            });
            if old_preset != this.cohort_preset {
                this.outlier_results = None;
                this.outlier_excluded_ids.clear();
            }

            let columns = this.cohort_metadata.as_ref().map(|m| m.columns.clone()).unwrap_or_default();
            if !columns.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Repeated measures & covariates").strong());
                ui.horizontal_wrapped(|ui| {
                    ui.label("Subject / patient ID:")
                        .on_hover_text("Repeated samples with the same value are treated as one subject cluster for sandwich standard errors, subject-collapsed sensitivity tests, restricted PERMANOVA permutations, and public-vs-longitudinal clonotype classification.");
                    let selected = this.corrected_subject_column.clone().unwrap_or_else(|| "(none)".to_string());
                    egui::ComboBox::from_id_salt("corrected_subject_column")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut this.corrected_subject_column, None, "(none)");
                            for col in &columns {
                                if this.cohort_metadata.as_ref().is_some_and(|m| col == &m.id_column) { continue; }
                                ui.selectable_value(&mut this.corrected_subject_column, Some(col.clone()), col);
                            }
                        });
                });
                if this.corrected_subject_column.is_none() {
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 170, 70),
                        "No subject column selected: repeated-measure correction cannot be applied. Count models use ordinary NB Wald SEs; continuous/compositional models use HC3 robust SEs; PERMANOVA permutations are unrestricted.",
                    );
                }

                egui::CollapsingHeader::new(format!("Covariates ({})", this.corrected_covariates.len()))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Select adjustment variables such as age, sex, ethnicity, family/batch variables. Numeric columns are standardized; categorical columns are dummy encoded with one reference level.").small().weak());
                        egui::ScrollArea::vertical().max_height(190.0).show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for col in &columns {
                                    let lower = col.to_ascii_lowercase();
                                    let excluded = this.cohort_metadata.as_ref().is_some_and(|m| col == &m.id_column)
                                        || this.corrected_subject_column.as_ref() == Some(col)
                                        || matches!(lower.as_str(), "phenotype" | "hyp" | "healthy" | "hyp_type");
                                    if excluded { continue; }
                                    let mut selected = this.corrected_covariates.contains(col);
                                    if ui.checkbox(&mut selected, col).changed() {
                                        if selected { this.corrected_covariates.insert(col.clone()); }
                                        else { this.corrected_covariates.remove(col); }
                                    }
                                }
                            });
                        });
                    });
            }

            ui.separator();
            ui.label(egui::RichText::new("Analyses to run").strong());
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut this.cohort_run_gene, "Gene counts")
                    .on_hover_text("Uses RNA-Seq Raw Count columns, positive-count median-ratio normalization, a negative-binomial log-link count model with a size-factor offset, optional covariates, subject-clustered standard errors when repeated samples are configured, BH FDR, a subject-collapsed sensitivity analysis, PCA, and hierarchical clustering.");
                ui.checkbox(&mut this.cohort_run_transcript, "Transcript counts")
                    .on_hover_text("Uses Total Reads with the same count model and also runs a within-gene CLR differential-transcript-usage screen with Simes/BH gene-level summaries.");
                ui.checkbox(&mut this.cohort_run_bcr, "BCR")
                    .on_hover_text("Matched-depth richness/Shannon, subject-aware sharing, Morisita-Horn and Jaccard PCoA/PERMANOVA, and CLR/Aitchison isotype/V/D/J composition analyses.");
                ui.checkbox(&mut this.cohort_run_bcr_isotype, "BCR isotype")
                    .on_hover_text("CLR-transformed isotype composition with adjusted per-isotype models and Aitchison PCoA/PERMANOVA.");
                ui.checkbox(&mut this.cohort_run_tcr_alpha, "TCR alpha")
                    .on_hover_text("Matched-depth diversity, subject-aware sharing, AA convergence, Morisita-Horn/Jaccard ordination, and CLR V/J/V-J composition analysis.");
                ui.checkbox(&mut this.cohort_run_tcr_beta, "TCR beta")
                    .on_hover_text("Matched-depth diversity, subject-aware sharing, AA convergence, Morisita-Horn/Jaccard ordination, and CLR V/J/V-J composition analysis.");
                ui.checkbox(&mut this.cohort_run_immune, "Immune infiltrate")
                    .on_hover_text("Adjusted linear models of each ssGSEA score plus PCA of cell-type z-scores; ssGSEA values are treated as enrichment scores, not cell percentages.");
            });
            ui.checkbox(&mut this.corrected_run_pathway, "Pathway ORA")
                .on_hover_text("Optional gene-set over-representation analysis using either a built-in public collection or a custom GMT. It uses the corrected gene-count results, the tested-gene universe, a hypergeometric upper-tail test, odds ratio, and BH FDR.");
            if this.corrected_run_pathway {
                this.render_gmt_source_selector(ui,"corrected");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Pathway foreground:");
                    ui.label("BH q ≤"); ui.add(egui::DragValue::new(&mut this.corrected_pathway_q).range(0.001..=0.25).speed(0.005));
                    ui.label("and |adjusted log2FC| ≥"); ui.add(egui::DragValue::new(&mut this.corrected_pathway_effect).range(0.0..=5.0).speed(0.1));
                });
            }

            egui::CollapsingHeader::new("Advanced standard-analysis settings")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Ambiguous V/D/J calls:");
                        egui::ComboBox::from_id_salt("corrected_ambiguous_policy")
                            .selected_text(this.corrected_ambiguous_policy.display_name())
                            .show_ui(ui, |ui| {
                                for policy in AmbiguousCallPolicy::ALL {
                                    ui.selectable_value(&mut this.corrected_ambiguous_policy, policy, policy.display_name())
                                        .on_hover_text(policy.description());
                                }
                            });
                    });
                    ui.checkbox(&mut this.corrected_exclude_undetermined, "Renormalize BCR-isotype analysis after excluding Undetermined")
                        .on_hover_text("The original composition remains preferable for QC. Enable this sensitivity view when the biological question is class-switching among determined IgA/IgD/IgG/IgM assignments.");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("PERMANOVA permutations:");
                        ui.add(egui::DragValue::new(&mut this.corrected_permutations).range(99..=4999).speed(100));
                        ui.label("Rarefaction repeats:");
                        ui.add(egui::DragValue::new(&mut this.corrected_rarefaction_reps).range(1..=20));
                        ui.label("Rarefaction depth cap:");
                        ui.add(egui::DragValue::new(&mut this.corrected_rarefaction_cap).range(1_000..=500_000).speed(5_000));
                    });
                    ui.label(egui::RichText::new("Repertoire matched depth is the minimum usable library depth, capped by the value above to keep exact-without-replacement Monte-Carlo rarefaction responsive. The same depth is used for every included sample.").small().weak());
                });

            let ready = this.cohort_scan.is_some() && this.cohort_metadata.is_some() && !this.cohort_running
                && (this.cohort_run_gene || this.cohort_run_transcript || this.cohort_run_bcr || this.cohort_run_bcr_isotype || this.cohort_run_tcr_alpha || this.cohort_run_tcr_beta || this.cohort_run_immune || this.corrected_run_pathway);
            let run = ui.add_enabled(ready, egui::Button::new("Run corrected analyses"))
                .on_hover_text("Run the selected corrected/standard analysis modules independently. A failure in one modality is reported without discarding successful modalities.");
            if run.clicked() { this.start_corrected_analysis(); }
            if this.cohort_running {
                ui.add(egui::ProgressBar::new(this.cohort_progress).show_percentage().animate(true));
            }
            ui.label(&this.cohort_status);
            ui.label(egui::RichText::new("Expression note: count models use positive-count median-ratio normalization, a negative-binomial log-link model with per-feature dispersion, optional covariates, subject-clustered inference when repeated samples are configured, and a subject-collapsed sensitivity result. Results should be interpreted as this model's estimates rather than as output from a different external package.").small().weak());
        });
    }

    fn render_corrected_results_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "4. Corrected / standard results", |this, ui| {
            let Some(bundle) = this.corrected_results.clone() else { return; };
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{} — corrected layer", bundle.comparison.name)).strong());
                if ui.button("Save corrected tables…").on_hover_text("Export all available adjusted tests, ordination coordinates, repertoire sample summaries, and method notes as TSV files.").clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(root) = &this.cohort_root { dialog = dialog.set_directory(root); }
                    if let Some(folder) = dialog.pick_folder() {
                        match this.save_corrected_result_tables(&folder) {
                            Ok(path) => this.cohort_status = format!("Saved corrected analysis tables to {}", path.display()),
                            Err(e) => this.cohort_status = format!("Could not save corrected tables: {e:#}"),
                        }
                    }
                }
                if ui.button("Generate corrected HTML report…").on_hover_text("Generate a standalone offline HTML report for the corrected/standard analysis layer, including adjusted statistics, PCoA/PCA plots, PERMANOVA summaries, and ranked tables.").clicked() {
                    let safe_name: String = bundle.comparison.name.chars().map(|c| if c.is_ascii_alphanumeric(){c}else{'_'}).collect();
                    let mut dialog = FileDialog::new().add_filter("HTML report", &["html"]).set_file_name(format!("4c_corrected_report_{safe_name}.html"));
                    if let Some(root)=&this.cohort_root { dialog=dialog.set_directory(root); }
                    if let Some(path)=dialog.save_file() {
                        let inputs=CorrectedReportInputs {
                            comparison:&bundle.comparison, mapping:this.cohort_mapping.as_ref(), spec:&bundle.spec,
                            gene:bundle.gene.as_ref(), gene_error:bundle.gene_error.as_deref(),
                            transcript:bundle.transcript.as_ref(), transcript_error:bundle.transcript_error.as_deref(),
                            bcr:bundle.bcr.as_ref(), bcr_error:bundle.bcr_error.as_deref(),
                            bcr_isotype:bundle.bcr_isotype.as_ref(), bcr_isotype_error:bundle.bcr_isotype_error.as_deref(),
                            tcr_alpha:bundle.tcr_alpha.as_ref(), tcr_alpha_error:bundle.tcr_alpha_error.as_deref(),
                            tcr_beta:bundle.tcr_beta.as_ref(), tcr_beta_error:bundle.tcr_beta_error.as_deref(),
                            immune:bundle.immune.as_ref(), immune_error:bundle.immune_error.as_deref(),
                            pathway:bundle.pathway.as_ref(), pathway_error:bundle.pathway_error.as_deref(),
                            outlier_qc:this.corrected_outlier_snapshot.as_ref(),
                        };
                        match generate_corrected_html_report(&path,inputs) {
                            Ok(())=>this.cohort_status=format!("Generated corrected HTML report: {}",path.display()),
                            Err(e)=>this.cohort_status=format!("Could not generate corrected report: {e:#}"),
                        }
                    }
                }
            });
            if let Some(qc)=&this.corrected_outlier_snapshot {
                ui.colored_label(egui::Color32::from_rgb(230,170,80),format!("QC-filtered corrected run: {} reviewed sample exclusion(s) applied (case {} → {}, control {} → {}). The export/report preserves the exact rule, modality flags, distances, and manual overrides.",qc.excluded_ids.len(),qc.original_case_n,qc.filtered_case_n,qc.original_control_n,qc.filtered_control_n));
            }
            ui.label(egui::RichText::new(format!(
                "Subject column: {} | covariates: {} | ambiguous repertoire calls: {}",
                bundle.spec.subject_column.as_deref().unwrap_or("none"),
                if bundle.spec.covariates.is_empty() { "none".to_string() } else { bundle.spec.covariates.join(", ") },
                bundle.spec.ambiguous_policy.display_name(),
            )).small().weak());
            ui.horizontal_wrapped(|ui| {
                ui.label("PCA / PCoA / dendrogram color:")
                    .on_hover_text("Choose how samples are colored in unsupervised sample-level plots. Analysis group shows the selected case/control comparison; metadata columns can reveal batch, phenotype, sex, timepoint, or other structure.");
                egui::ComboBox::from_id_salt("corrected_ordination_color_by")
                    .selected_text(&this.corrected_ordination_color_by)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut this.corrected_ordination_color_by, "Analysis group".to_string(), "Analysis group");
                        ui.selectable_value(&mut this.corrected_ordination_color_by, "Subject / patient".to_string(), "Subject / patient");
                        let metadata = if this.cohort_use_filtered_metadata {
                            this.focused_metadata.as_ref()
                        } else {
                            this.cohort_metadata.as_ref()
                        };
                        if let Some(meta) = metadata {
                            for column in &meta.columns {
                                ui.selectable_value(&mut this.corrected_ordination_color_by, column.clone(), column);
                            }
                        }
                    });
            });
            egui::CollapsingHeader::new("How to interpret the corrected / standard layer")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("Adjusted feature tests ask whether the case/control contrast remains after the selected covariates are included. BH q-values control the false-discovery rate within each reported feature family.");
                    ui.label("PCA, PCoA, and dendrograms are unsupervised views: visible case/control separation is supportive when present, but lack of separation does not rule out individual biomarkers. Conversely, clustering can reflect batch, subject, timepoint, or other metadata rather than disease status.");
                    ui.label("PERMANOVA tests whether multivariate distances differ between the two comparison groups. With a subject column selected, permutations are restricted at the subject level.");
                    ui.label("Pathway over-representation asks whether the foreground gene list contains more members of a gene set than expected from the measured-gene universe; it should be interpreted alongside effect direction, overlap size, and redundancy among gene sets.");
                });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if bundle.gene.is_some() || bundle.gene_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::Gene, "Gene counts"); }
                if bundle.transcript.is_some() || bundle.transcript_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::Transcript, "Transcript counts"); }
                if bundle.bcr.is_some() || bundle.bcr_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::Bcr, "BCR"); }
                if bundle.bcr_isotype.is_some() || bundle.bcr_isotype_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::BcrIsotype, "BCR isotype"); }
                if bundle.tcr_alpha.is_some() || bundle.tcr_alpha_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::TcrAlpha, "TCR alpha"); }
                if bundle.tcr_beta.is_some() || bundle.tcr_beta_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::TcrBeta, "TCR beta"); }
                if bundle.immune.is_some() || bundle.immune_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::Immune, "Immune infiltrate"); }
                if bundle.pathway.is_some() || bundle.pathway_error.is_some() { ui.selectable_value(&mut this.corrected_result_view, CorrectedResultView::Pathway, "Pathways"); }
            });
            ui.separator();
            match this.corrected_result_view {
                CorrectedResultView::Gene => if let Some(r)=&bundle.gene { this.render_corrected_expression(ui,r,"corrected_gene"); } else { ui.label(bundle.gene_error.as_deref().unwrap_or("Gene corrected analysis not requested.")); },
                CorrectedResultView::Transcript => if let Some(r)=&bundle.transcript { this.render_corrected_expression(ui,r,"corrected_transcript"); } else { ui.label(bundle.transcript_error.as_deref().unwrap_or("Transcript corrected analysis not requested.")); },
                CorrectedResultView::Bcr => if let Some(r)=&bundle.bcr { this.render_corrected_repertoire(ui,r,"corrected_bcr"); } else { ui.label(bundle.bcr_error.as_deref().unwrap_or("BCR corrected analysis not requested.")); },
                CorrectedResultView::BcrIsotype => if let Some(r)=&bundle.bcr_isotype { this.render_corrected_isotype(ui,r,"corrected_isotype"); } else { ui.label(bundle.bcr_isotype_error.as_deref().unwrap_or("BCR-isotype corrected analysis not requested.")); },
                CorrectedResultView::TcrAlpha => if let Some(r)=&bundle.tcr_alpha { this.render_corrected_repertoire(ui,r,"corrected_tcra"); } else { ui.label(bundle.tcr_alpha_error.as_deref().unwrap_or("TCR-alpha corrected analysis not requested.")); },
                CorrectedResultView::TcrBeta => if let Some(r)=&bundle.tcr_beta { this.render_corrected_repertoire(ui,r,"corrected_tcrb"); } else { ui.label(bundle.tcr_beta_error.as_deref().unwrap_or("TCR-beta corrected analysis not requested.")); },
                CorrectedResultView::Immune => if let Some(r)=&bundle.immune { this.render_corrected_immune(ui,r,"corrected_immune"); } else { ui.label(bundle.immune_error.as_deref().unwrap_or("Immune corrected analysis not requested.")); },
                CorrectedResultView::Pathway => if let Some(r)=&bundle.pathway { this.render_corrected_pathway(ui,r,"corrected_pathway"); } else { ui.label(bundle.pathway_error.as_deref().unwrap_or("Pathway analysis not requested.")); },
            }
        });
    }

    fn render_corrected_expression(
        &mut self,
        ui: &mut egui::Ui,
        result: &CorrectedExpressionResult,
        id: &str,
    ) {
        ui.heading(format!(
            "{} — corrected count analysis",
            result.feature_label
        ));
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Model samples: {}", result.n_model_samples));
            ui.separator();
            ui.label(format!("Subjects: {}", result.n_subjects));
            ui.separator();
            ui.label(format!("Tested: {}", result.tested_features));
            ui.separator();
            ui.label(format!("Filtered low-count: {}", result.filtered_features));
        });
        ui.label(egui::RichText::new(&result.normalization).small().weak());
        ui.label(
            egui::RichText::new(&result.model_description)
                .small()
                .weak(),
        );
        egui::CollapsingHeader::new("Analysis guide").default_open(false).show(ui, |ui| {
            ui.label("The count model tests the case/control coefficient after the selected covariates are included. Positive coefficients indicate higher modeled counts in cases; negative coefficients indicate lower modeled counts.");
            ui.label("BH q-values control the false-discovery rate across the tested genes or transcripts. The subject-collapsed result is a sensitivity analysis that gives each subject one averaged observation when repeated samples are configured.");
            ui.label("PCA and hierarchical clustering use the most variable normalized features and are unsupervised. They are useful for detecting broad phenotype structure, batch effects, outlying samples, or other metadata-driven clustering, but are not significance tests for individual genes.");
        });
        for warning in &result.warnings {
            ui.colored_label(egui::Color32::from_rgb(230, 170, 70), warning);
        }
        if let Some(ord) = &result.pca {
            self.render_corrected_ordination(ui, ord, &format!("{id}_pca"));
        }
        if let Some(tree) = &result.dendrogram {
            egui::CollapsingHeader::new("Hierarchical clustering")
                .default_open(false)
                .show(ui, |ui| {
                    self.render_corrected_dendrogram(ui, tree, &format!("{id}_dendrogram"));
                });
        }
        self.render_adjusted_test_table(ui, "Adjusted feature tests", &result.tests, 600.0);
        if let Some(dtu) = &result.dtu {
            egui::CollapsingHeader::new(format!("Differential transcript usage ({} genes / {} transcripts)",dtu.genes_tested,dtu.transcripts_tested))
                .default_open(true)
                .show(ui,|ui|{
                    ui.label(egui::RichText::new(&dtu.method_description).small().weak());
                    ui.label("This screen asks whether the relative contribution of transcripts from the same gene changes between groups, rather than whether total gene expression changes.");
                    ui.label(egui::RichText::new(format!("Gene-level DTU screen ({} rows)",dtu.gene_tests.len())).strong());
                    fixed_table_viewport(ui, &format!("{id}_dtu_genes_scroll"), 430.0, 820.0, |ui| {
                        egui::Grid::new(format!("{id}_dtu_genes")).striped(true).show(ui,|ui|{
                            for h in ["NCBI Gene ID","Transcripts","Simes p","BH q","Max |CLR β|"] {ui.label(egui::RichText::new(h).strong());} ui.end_row();
                            for r in &dtu.gene_tests {ui.monospace(&r.gene_id);ui.label(r.n_transcripts.to_string());ui.label(format!("{:.3e}",r.p_value));ui.label(format!("{:.3e}",r.q_value));ui.label(format!("{:.4}",r.max_abs_clr_effect));ui.end_row();}
                        });
                    });
                    self.render_adjusted_test_table(ui,"Transcript-level CLR usage tests",&dtu.transcript_tests,520.0);
                });
        }
    }

    fn render_corrected_repertoire(
        &mut self,
        ui: &mut egui::Ui,
        result: &CorrectedRepertoireResult,
        id: &str,
    ) {
        ui.heading(format!("{} — depth/composition corrected", result.modality));
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Samples: {}", result.files_used));
            ui.separator();
            ui.label(format!("Matched depth: {} reads", result.matched_depth));
            ui.separator();
            ui.label(format!("Rarefaction repeats: {}", result.rarefaction_reps));
            ui.separator();
            ui.label(format!(
                "Ambiguous calls: {}",
                result.ambiguous_policy.display_name()
            ));
        });
        egui::CollapsingHeader::new("Analysis guide").default_open(false).show(ui, |ui| {
            ui.label("Richness and Shannon diversity are compared after rarefying every repertoire to a common read depth, reducing the effect of unequal library depth. Clonality, CDR3 length, sharing, and convergence describe complementary repertoire properties.");
            ui.label("Morisita-Horn emphasizes abundance-weighted clonotype similarity; Jaccard uses presence/absence. PCoA visualizes these pairwise distances, while PERMANOVA tests whether the case/control groups differ globally in multivariate repertoire space.");
            ui.label("The dendrogram uses average-linkage hierarchical clustering on Morisita-Horn distance. Branches show similarity structure without using phenotype labels; leaf colors can be changed using the metadata color selector above.");
            ui.label("V/D/J/V-J and isotype families are compositional. CLR/Aitchison analysis evaluates relative composition, so individual positive or negative coefficients should be interpreted as changes relative to the rest of the composition.");
        });
        for warning in &result.warnings {
            ui.colored_label(egui::Color32::from_rgb(230, 170, 70), warning);
        }
        self.render_adjusted_test_table(
            ui,
            "Adjusted repertoire metrics",
            &result.metric_tests,
            480.0,
        );
        ui.horizontal_wrapped(|ui| {
            if let Some(ord) = &result.morisita_horn {
                ui.label(format!(
                    "Morisita-Horn PERMANOVA: F={:.3}, p={:.3e}",
                    ord.permanova_f, ord.permanova_p
                ));
            }
            if let Some(ord) = &result.jaccard {
                ui.label(format!(
                    "Jaccard PERMANOVA: F={:.3}, p={:.3e}",
                    ord.permanova_f, ord.permanova_p
                ));
            }
        });
        if let Some(ord) = &result.morisita_horn {
            self.render_corrected_ordination(ui, ord, &format!("{id}_mh"));
        }
        if let Some(tree) = &result.dendrogram {
            egui::CollapsingHeader::new("Morisita-Horn hierarchical clustering")
                .default_open(false)
                .show(ui, |ui| {
                    self.render_corrected_dendrogram(ui, tree, &format!("{id}_mh_tree"));
                });
        }
        if let Some(ord) = &result.jaccard {
            egui::CollapsingHeader::new("Jaccard presence/absence PCoA").show(ui, |ui| {
                self.render_corrected_ordination(ui, ord, &format!("{id}_jaccard"))
            });
        }
        for (idx, comp) in result.compositions.iter().enumerate() {
            egui::CollapsingHeader::new(format!(
                "{} CLR/Aitchison analysis ({} retained features)",
                comp.family, comp.retained_features
            ))
            .default_open(idx == 0)
            .show(ui, |ui| {
                self.render_corrected_composition(ui, comp, &format!("{id}_comp_{idx}"))
            });
        }
        egui::CollapsingHeader::new("Matched-depth & sharing sample table").show(ui, |ui| {
            fixed_table_viewport(ui, &format!("{id}_samples_scroll"), 520.0, 1180.0, |ui| {
                egui::Grid::new(format!("{id}_samples"))
                    .striped(true)
                    .show(ui, |ui| {
                        for h in [
                            "Sample",
                            "Group",
                            "Subject",
                            "Observed richness",
                            "Rarefied richness",
                            "Rarefied Shannon",
                            "Private",
                            "Longitudinal",
                            "Public",
                        ] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for r in &result.sample_summaries {
                            ui.monospace(&r.sample_id);
                            ui.label(&r.group);
                            ui.monospace(&r.subject);
                            ui.label(format!("{:.1}", r.observed_richness));
                            ui.label(format!("{:.1}", r.rarefied_richness));
                            ui.label(format!("{:.3}", r.rarefied_shannon));
                            ui.label(format!("{:.1}%", 100.0 * r.private_fraction));
                            ui.label(format!("{:.1}%", 100.0 * r.longitudinal_shared_fraction));
                            ui.label(format!("{:.1}%", 100.0 * r.public_fraction));
                            ui.end_row();
                        }
                    });
            });
        });
    }

    fn render_corrected_composition(
        &mut self,
        ui: &mut egui::Ui,
        result: &CorrectedCompositionResult,
        id: &str,
    ) {
        ui.label("Aitchison distance compares complete compositions after CLR transformation; the global PERMANOVA asks whether composition differs between groups before individual features are interpreted.");
        if let Some(ord) = &result.ordination {
            ui.label(format!(
                "Global Aitchison PERMANOVA: F={:.3}, p={:.3e} ({} permutations)",
                ord.permanova_f, ord.permanova_p, ord.permutations
            ));
            self.render_corrected_ordination(ui, ord, &format!("{id}_ord"));
        }
        if let Some(tree) = &result.dendrogram {
            egui::CollapsingHeader::new("Aitchison hierarchical clustering")
                .default_open(false)
                .show(ui, |ui| {
                    self.render_corrected_dendrogram(ui, tree, &format!("{id}_tree"));
                });
        }
        self.render_adjusted_test_table(
            ui,
            &format!("{} adjusted CLR tests", result.family),
            &result.tests,
            560.0,
        );
    }

    fn render_corrected_isotype(
        &mut self,
        ui: &mut egui::Ui,
        result: &CorrectedIsotypeResult,
        id: &str,
    ) {
        ui.heading("BCR isotype — compositional corrected analysis");
        ui.label(format!(
            "Samples: {} | {}",
            result.files_used,
            if result.determined_only {
                "Undetermined excluded and composition renormalized"
            } else {
                "Original composition including Undetermined"
            }
        ));
        egui::CollapsingHeader::new("Analysis guide").default_open(false).show(ui, |ui| {
            ui.label("Isotype fractions form a composition: increasing one component necessarily changes the relative share of others. CLR/Aitchison methods therefore test the composition jointly and individual coefficients are relative rather than independent percentage changes.");
            ui.label("The PCoA and dendrogram provide unsupervised views of isotype-composition similarity. If Undetermined is excluded, results describe the determined isotype mixture after renormalization.");
        });
        for warning in &result.warnings {
            ui.colored_label(egui::Color32::from_rgb(230, 170, 70), warning);
        }
        if let Some(ord) = &result.ordination {
            self.render_corrected_ordination(ui, ord, &format!("{id}_ord"));
        }
        if let Some(tree) = &result.dendrogram {
            egui::CollapsingHeader::new("Isotype hierarchical clustering")
                .default_open(false)
                .show(ui, |ui| {
                    self.render_corrected_dendrogram(ui, tree, &format!("{id}_tree"));
                });
        }
        self.render_adjusted_test_table(ui, "Isotype CLR tests", &result.tests, 540.0);
    }

    fn render_corrected_immune(
        &mut self,
        ui: &mut egui::Ui,
        result: &CorrectedImmuneResult,
        id: &str,
    ) {
        ui.heading("Immune infiltrate — adjusted ssGSEA analysis");
        ui.label(format!("Samples: {}. Tests use original ssGSEA scores; PCA and clustering use per-cell-type z-scores.",result.files_used));
        egui::CollapsingHeader::new("Analysis guide").default_open(false).show(ui, |ui| {
            ui.label("Each cell type is tested using its original ssGSEA enrichment score after covariate adjustment. Scores are enrichment measures, not literal cell percentages, so comparisons are made for the same cell type across groups.");
            ui.label("PCA and hierarchical clustering use standardized cell-type scores so one high-scale signature cannot dominate solely because of its numerical range. These plots are useful for identifying coordinated immune patterns and sample outliers.");
        });
        for warning in &result.warnings {
            ui.colored_label(egui::Color32::from_rgb(230, 170, 70), warning);
        }
        if let Some(ord) = &result.pca {
            self.render_corrected_ordination(ui, ord, &format!("{id}_pca"));
        }
        if let Some(tree) = &result.dendrogram {
            egui::CollapsingHeader::new("Immune-profile hierarchical clustering")
                .default_open(false)
                .show(ui, |ui| {
                    self.render_corrected_dendrogram(ui, tree, &format!("{id}_tree"));
                });
        }
        self.render_adjusted_test_table(ui, "Adjusted immune-cell tests", &result.tests, 580.0);
    }

    fn render_corrected_pathway(
        &mut self,
        ui: &mut egui::Ui,
        result: &CorrectedPathwayResult,
        id: &str,
    ) {
        ui.heading("Pathway over-representation analysis");
        ui.label(format!("GMT: {} | tested-gene universe: {} | foreground: {} | criteria: q ≤ {:.3}, |adjusted log2FC| ≥ {:.3}",result.gmt_path.display(),result.universe_size,result.foreground_size,result.q_threshold,result.effect_threshold));
        ui.label(egui::RichText::new(format!("Hypergeometric upper-tail test with BH correction; gene sets retained at {}-{} measured genes.",result.min_set_size,result.max_set_size)).small().weak());
        egui::CollapsingHeader::new("Analysis guide").default_open(false).show(ui, |ui| {
            ui.label("The foreground contains genes passing the configured corrected-expression q-value and effect thresholds. ORA asks whether a pathway contains more foreground genes than expected from all measured/tested genes.");
            ui.label("Odds ratio > 1 indicates enrichment. Strong significance with a very large overlap may reflect overlapping or redundant gene sets, so related pathways should be interpreted as biological themes rather than independent discoveries.");
            ui.label("Hover text intentionally shows only a compact subset of overlap genes; the complete overlap list remains in the table and exported results.");
        });
        let rows = result
            .tests
            .iter()
            .filter(|r| r.q_value.is_finite() && r.q_value > 0.0)
            .take(250)
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            let hover_points=rows.iter().map(|r|{
                let x=r.odds_ratio.max(1e-12).ln();
                let y=-r.q_value.max(1e-300).log10();
                let genes=compact_comma_list(&r.overlap_genes,8,120);
                let label=format!("{}\nOverlap: {}/{}\nOdds ratio: {:.3}\np: {:.3e}\nBH q: {:.3e}\nOverlap genes: {}",r.pathway,r.overlap,r.measured_set_size,r.odds_ratio,r.p_value,r.q_value,genes);
                (x,y,label)
            }).collect::<Vec<_>>();
            let coords = hover_points
                .iter()
                .map(|(x, y, _)| [*x, *y])
                .collect::<Vec<_>>();
            let formatter_points = hover_points.clone();
            Plot::new(format!("{id}_plot"))
                .height(390.0)
                .x_axis_label("ln odds ratio")
                .y_axis_label("-log10(BH q)")
                .label_formatter(move |hover| match hover {
                    HoverPosition::NearDataPoint { position, .. } => {
                        nearest_hover_label(position.x, position.y, &formatter_points)
                    }
                    _ => None,
                })
                .show(ui, |plot_ui| {
                    plot_ui.points(Points::new("Pathways", coords).radius(4.0));
                });
        }
        fixed_table_viewport(ui, &format!("{id}_table_scroll"), 640.0, 1320.0, |ui| {
            egui::Grid::new(format!("{id}_table"))
                .striped(true)
                .show(ui, |ui| {
                    for h in [
                        "Pathway",
                        "Set size",
                        "Overlap",
                        "Odds ratio",
                        "p",
                        "BH q",
                        "Overlap genes",
                    ] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for r in &result.tests {
                        ui.monospace(&r.pathway);
                        ui.label(r.measured_set_size.to_string());
                        ui.label(r.overlap.to_string());
                        ui.label(format!("{:.3}", r.odds_ratio));
                        ui.label(format!("{:.3e}", r.p_value));
                        ui.label(format!("{:.3e}", r.q_value));
                        ui.label(&r.overlap_genes);
                        ui.end_row();
                    }
                });
        });
    }

    fn render_corrected_ordination(&mut self, ui: &mut egui::Ui, ord: &OrdinationResult, id: &str) {
        ui.label(egui::RichText::new(&ord.name).strong());
        if ord.permanova_p.is_finite() {
            ui.label(format!(
                "PERMANOVA: pseudo-F {:.3}, p {:.3e}, {} permutations",
                ord.permanova_f, ord.permanova_p, ord.permutations
            ));
        }
        let meta_map = self.selected_metadata_color_map();
        let mut grouped: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();
        let mut hover_points = Vec::with_capacity(ord.points.len());
        for p in &ord.points {
            let category = self.sample_color_value(&p.sample_id, &p.group, &p.subject, &meta_map);
            grouped
                .entry(category.clone())
                .or_default()
                .push([p.x, p.y]);
            hover_points.push((
                p.x,
                p.y,
                format!(
                    "{}\nAnalysis group: {}\nColor ({}): {}\nSubject: {}\n{}: {:.4}\n{}: {:.4}",
                    p.sample_id,
                    p.group,
                    self.corrected_ordination_color_by,
                    category,
                    p.subject,
                    ord.x_label,
                    p.x,
                    ord.y_label,
                    p.y
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let mut plot = Plot::new(id)
            .height(430.0)
            .x_axis_label(&ord.x_label)
            .y_axis_label(&ord.y_label)
            .label_formatter(move |hover| match hover {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_hover_label(position.x, position.y, &formatter_points)
                }
                _ => None,
            });
        if grouped.len() <= 12 {
            plot = plot.legend(Legend::default());
        }
        let accent = ui.visuals().selection.bg_fill;
        plot.show(ui, |plot_ui| {
            for (idx, (category, points)) in grouped.into_iter().enumerate() {
                let color =
                    category_color(&category, idx, &self.corrected_ordination_color_by, accent);
                plot_ui.points(Points::new(category, points).color(color).radius(4.5));
            }
        });
        if ord.points.len() > 180 {
            ui.label(egui::RichText::new(format!("{} samples are drawn in grouped point series for responsive interaction; hover labels are resolved from the nearest plotted point.",ord.points.len())).small().weak());
        }
    }

    fn render_corrected_dendrogram(
        &mut self,
        ui: &mut egui::Ui,
        tree: &DendrogramResult,
        id: &str,
    ) {
        ui.label(egui::RichText::new(&tree.name).strong());
        ui.label(
            egui::RichText::new(format!(
                "{} using {}. Shorter merge distances indicate more similar samples.",
                tree.linkage, tree.distance_label
            ))
            .small()
            .weak(),
        );
        let meta_map = self.selected_metadata_color_map();
        let mut grouped: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();
        let mut hover_points = Vec::with_capacity(tree.leaves.len());
        for leaf in &tree.leaves {
            let category =
                self.sample_color_value(&leaf.sample_id, &leaf.group, &leaf.subject, &meta_map);
            grouped
                .entry(category.clone())
                .or_default()
                .push([0.0, leaf.position]);
            hover_points.push((
                0.0,
                leaf.position,
                format!(
                    "{}\nAnalysis group: {}\nColor ({}): {}\nSubject: {}\nCluster order: {}",
                    leaf.sample_id,
                    leaf.group,
                    self.corrected_ordination_color_by,
                    category,
                    leaf.subject,
                    leaf.position as usize + 1
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let mut plot = Plot::new(id)
            .height(520.0)
            .x_axis_label(&tree.distance_label)
            .y_axis_label("Cluster leaf order")
            .label_formatter(move |hover| match hover {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_hover_label(position.x, position.y, &formatter_points)
                }
                _ => None,
            });
        if grouped.len() <= 12 {
            plot = plot.legend(Legend::default());
        }
        let accent = ui.visuals().selection.bg_fill;
        plot.show(ui, |plot_ui| {
            for (idx, seg) in tree.segments.iter().enumerate() {
                plot_ui.line(
                    Line::new(
                        format!("branch_{idx}"),
                        vec![[seg.x1, seg.y1], [seg.x2, seg.y2]],
                    )
                    .color(egui::Color32::from_gray(120))
                    .width(1.0)
                    .allow_hover(false),
                );
            }
            for (idx, (category, points)) in grouped.into_iter().enumerate() {
                let color =
                    category_color(&category, idx, &self.corrected_ordination_color_by, accent);
                plot_ui.points(Points::new(category, points).color(color).radius(4.0));
            }
        });
        egui::CollapsingHeader::new(format!(
            "Cluster leaf order ({} samples)",
            tree.leaves.len()
        ))
        .show(ui, |ui| {
            fixed_table_viewport(ui, &format!("{id}_leaves_scroll"), 360.0, 760.0, |ui| {
                egui::Grid::new(format!("{id}_leaves"))
                    .striped(true)
                    .show(ui, |ui| {
                        for h in ["Order", "Sample", "Group", "Subject"] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (i, leaf) in tree.leaves.iter().enumerate() {
                            ui.label((i + 1).to_string());
                            ui.monospace(&leaf.sample_id);
                            ui.label(&leaf.group);
                            ui.monospace(&leaf.subject);
                            ui.end_row();
                        }
                    });
            });
        });
    }

    fn selected_metadata_color_map(&self) -> HashMap<String, String> {
        if self.corrected_ordination_color_by == "Analysis group"
            || self.corrected_ordination_color_by == "Subject / patient"
        {
            return HashMap::new();
        }
        let metadata = if self.cohort_use_filtered_metadata {
            self.focused_metadata.as_ref()
        } else {
            self.cohort_metadata.as_ref()
        };
        let Some(meta) = metadata else {
            return HashMap::new();
        };
        if meta.column_index(&meta.id_column).is_none() {
            return HashMap::new();
        }
        let Some(value_idx) = meta.column_index(&self.corrected_ordination_color_by) else {
            return HashMap::new();
        };
        let mut tmp: HashMap<String, Vec<String>> = HashMap::new();
        for (row_idx, row) in meta.rows.iter().enumerate() {
            let value = row.get(value_idx).map(|s| s.trim()).unwrap_or_default();
            if value.is_empty() {
                continue;
            }
            for key in row_matching_ids(meta, row_idx) {
                tmp.entry(key).or_default().push(value.to_string());
            }
        }
        tmp.into_iter()
            .filter_map(|(k, v)| {
                let mut unique = v;
                unique.sort();
                unique.dedup();
                if unique.len() == 1 {
                    Some((k, unique.remove(0)))
                } else {
                    None
                }
            })
            .collect()
    }

    fn sample_color_value(
        &self,
        sample_id: &str,
        group: &str,
        subject: &str,
        meta_map: &HashMap<String, String>,
    ) -> String {
        match self.corrected_ordination_color_by.as_str() {
            "Analysis group" => group.to_string(),
            "Subject / patient" => subject.to_string(),
            _ => meta_map
                .get(&normalize_sample_id(sample_id))
                .cloned()
                .unwrap_or_else(|| "(missing)".to_string()),
        }
    }

    fn render_adjusted_test_table(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        tests: &[AdjustedFeatureResult],
        height: f32,
    ) {
        ui.label(egui::RichText::new(format!("{title} ({} rows)", tests.len())).strong());
        ui.label(egui::RichText::new("β is the adjusted case-vs-control coefficient on the modeled scale. q is BH-adjusted within this feature family. Subject p/q comes from averaging repeated samples within each subject and applying a non-parametric sensitivity test.").small().weak());
        fixed_table_viewport(
            ui,
            &format!("adjusted_table_scroll_{}", title.replace(' ', "_")),
            height.max(360.0),
            1220.0,
            |ui| {
                egui::Grid::new(format!("adjusted_table_{}", title.replace(' ', "_")))
                    .striped(true)
                    .show(ui, |ui| {
                        for h in [
                            "Feature",
                            "ID/family",
                            "β group",
                            "SE",
                            "p",
                            "BH q",
                            "Subject effect",
                            "Subject q",
                            "n",
                            "subjects",
                            "case mean",
                            "ctrl mean",
                        ] {
                            ui.label(egui::RichText::new(h).strong());
                        }
                        ui.end_row();
                        for r in tests {
                            ui.monospace(&r.feature);
                            ui.monospace(&r.secondary_id);
                            ui.label(format!("{:.4}", r.beta_group));
                            ui.label(format!("{:.4}", r.robust_se));
                            ui.label(format!("{:.3e}", r.p_value));
                            ui.label(format!("{:.3e}", r.q_value));
                            ui.label(format!("{:.4}", r.subject_effect));
                            ui.label(format!("{:.3e}", r.subject_q_value));
                            ui.label(r.n_samples.to_string());
                            ui.label(r.n_subjects.to_string());
                            ui.label(format!("{:.4}", r.mean_case));
                            ui.label(format!("{:.4}", r.mean_control));
                            ui.end_row();
                        }
                    });
            },
        );
    }

    fn render_cohort_results_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "4. Comparison results", |this, ui| {
            let Some(bundle) = this.cohort_results.clone() else { return; };
            ui.label(
                egui::RichText::new(format!(
                    "{}: {} vs {}",
                    bundle.comparison.name,
                    bundle.comparison.case_label,
                    bundle.comparison.control_label
                ))
                .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                let save = ui
                    .button("Save result tables…")
                    .on_hover_text("Export the mapping audit and all available comparison tables as TSV files. Plots remain interactive in the app.");
                if save.clicked() {
                    let mut dialog = FileDialog::new();
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(folder) = dialog.pick_folder() {
                        match this.save_cohort_result_tables(&folder) {
                            Ok(path) => {
                                this.cohort_status = format!("Saved comparison tables to {}", path.display());
                            }
                            Err(e) => {
                                this.cohort_status = format!("Could not save comparison tables: {e:#}");
                            }
                        }
                    }
                }

                let report_button = ui
                    .button("Generate HTML report…")
                    .on_hover_text("Generate one self-contained HTML file containing the study overview, mapping audit, interactive plots and ranked result tables using report-builder. Plotly JavaScript is embedded so the report can be opened offline.");
                if report_button.clicked() {
                    let safe_name: String = bundle
                        .comparison
                        .name
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect();
                    let mut dialog = FileDialog::new()
                        .add_filter("HTML report", &["html"])
                        .set_file_name(format!("4c_cohort_report_{safe_name}.html"));
                    if let Some(root) = &this.cohort_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(path) = dialog.save_file() {
                        this.remember_directory_for(&path);
                        let outlier_snapshot=this.current_outlier_snapshot_for_comparison(&bundle.comparison,false);
                        let inputs = CohortReportInputs {
                            comparison: &bundle.comparison,
                            mapping: this.cohort_mapping.as_ref(),
                            gene: bundle.gene.as_ref(),
                            gene_error: bundle.gene_error.as_deref(),
                            transcript: bundle.transcript.as_ref(),
                            transcript_error: bundle.transcript_error.as_deref(),
                            bcr: bundle.bcr.as_ref(),
                            bcr_error: bundle.bcr_error.as_deref(),
                            bcr_isotype: bundle.bcr_isotype.as_ref(),
                            bcr_isotype_error: bundle.bcr_isotype_error.as_deref(),
                            tcr_alpha: bundle.tcr_alpha.as_ref(),
                            tcr_alpha_error: bundle.tcr_alpha_error.as_deref(),
                            tcr_beta: bundle.tcr_beta.as_ref(),
                            tcr_beta_error: bundle.tcr_beta_error.as_deref(),
                            immune: bundle.immune.as_ref(),
                            immune_error: bundle.immune_error.as_deref(),
                            outlier_qc: outlier_snapshot.as_ref(),
                        };
                        match generate_cohort_html_report(&path, inputs) {
                            Ok(()) => {
                                this.cohort_status = format!("Generated HTML report: {}", path.display());
                            }
                            Err(e) => {
                                this.cohort_status = format!("Could not generate HTML report: {e:#}");
                            }
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if bundle.gene.is_some() || bundle.gene_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::Gene, "Gene expression");
                }
                if bundle.transcript.is_some() || bundle.transcript_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::Transcript, "Transcript expression");
                }
                if bundle.bcr.is_some() || bundle.bcr_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::Bcr, "BCR");
                }
                if bundle.bcr_isotype.is_some() || bundle.bcr_isotype_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::BcrIsotype, "BCR isotype");
                }
                if bundle.tcr_alpha.is_some() || bundle.tcr_alpha_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::TcrAlpha, "TCR alpha");
                }
                if bundle.tcr_beta.is_some() || bundle.tcr_beta_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::TcrBeta, "TCR beta");
                }
                if bundle.immune.is_some() || bundle.immune_error.is_some() {
                    ui.selectable_value(&mut this.cohort_result_view, CohortResultView::Immune, "Immune infiltrate");
                }
            });
            ui.separator();

            match this.cohort_result_view {
                CohortResultView::Gene => {
                    if let Some(result) = &bundle.gene {
                        this.render_expression_comparison_result(ui, result, "Gene", "genes", "cohort_gene_volcano_v06", "cohort_gene_detection_v06");
                    } else {
                        ui.label(bundle.gene_error.as_deref().unwrap_or("Gene analysis was not requested."));
                    }
                }
                CohortResultView::Transcript => {
                    if let Some(result) = &bundle.transcript {
                        this.render_expression_comparison_result(ui, result, "Transcript", "transcripts", "cohort_transcript_volcano_v06", "cohort_transcript_detection_v06");
                    } else {
                        ui.label(bundle.transcript_error.as_deref().unwrap_or("Transcript analysis was not requested."));
                    }
                }
                CohortResultView::Bcr => {
                    if let Some(result) = &bundle.bcr { this.render_bcr_comparison_result(ui, result); }
                    else { ui.label(bundle.bcr_error.as_deref().unwrap_or("BCR analysis was not requested.")); }
                }
                CohortResultView::BcrIsotype => {
                    if let Some(result) = &bundle.bcr_isotype { this.render_bcr_isotype_result(ui, result); }
                    else { ui.label(bundle.bcr_isotype_error.as_deref().unwrap_or("BCR-isotype analysis was not requested.")); }
                }
                CohortResultView::TcrAlpha => {
                    if let Some(result) = &bundle.tcr_alpha { this.render_tcr_comparison_result(ui, result); }
                    else { ui.label(bundle.tcr_alpha_error.as_deref().unwrap_or("TCR-alpha analysis was not requested.")); }
                }
                CohortResultView::TcrBeta => {
                    if let Some(result) = &bundle.tcr_beta { this.render_tcr_comparison_result(ui, result); }
                    else { ui.label(bundle.tcr_beta_error.as_deref().unwrap_or("TCR-beta analysis was not requested.")); }
                }
                CohortResultView::Immune => {
                    if let Some(result) = &bundle.immune { this.render_immune_comparison_result(ui, result); }
                    else { ui.label(bundle.immune_error.as_deref().unwrap_or("Immune-infiltrate analysis was not requested.")); }
                }
            }
        });
    }

    fn render_expression_comparison_result(
        &self,
        ui: &mut egui::Ui,
        result: &GeneComparisonResult,
        feature_name: &str,
        feature_plural: &str,
        volcano_id: &'static str,
        detection_id: &'static str,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Case columns: {}", result.n_case_columns));
            ui.separator();
            ui.label(format!("Control columns: {}", result.n_control_columns));
            ui.separator();
            ui.label(format!(
                "Other/unmatched columns: {}",
                result.unmatched_columns
            ));
            ui.separator();
            ui.label(format!("{} tested: {}", feature_plural, result.tests.len()));
        });
        ui.label(
            egui::RichText::new(format!(
                "Exploratory test: Mann-Whitney U on log2({}+1); effect is log2((mean case + 0.5)/(mean control + 0.5)); q is BH-adjusted across tested {feature_plural}.",
                result.metric
            ))
            .small()
            .weak(),
        );

        let valid = result
            .tests
            .iter()
            .filter(|r| r.effect.is_finite() && r.q_value.is_finite() && r.q_value > 0.0)
            .collect::<Vec<_>>();
        let selected = bounded_plot_indices(valid.len(), 6000, 800);
        let mut significant = Vec::new();
        let mut other = Vec::new();
        let mut hover_points = Vec::with_capacity(selected.len());
        for idx in selected {
            let r = valid[idx];
            let xy = [r.effect, -r.q_value.max(1e-300).log10()];
            if r.q_value <= 0.05 {
                significant.push(xy);
            } else {
                other.push(xy);
            }
            hover_points.push((xy[0], xy[1], format!(
                "{}{}\nlog2FC: {:.3}\nCliff's δ: {:.3}\np: {:.3e}\nBH q: {:.3e}\nmedian case/control: {:.3} / {:.3}\nmean case/control: {:.3} / {:.3}\nn case/control: {} / {}",
                r.feature,
                if r.secondary_id.is_empty() { String::new() } else { format!(" ({})", r.secondary_id) },
                r.effect, r.cliffs_delta, r.p_value, r.q_value, r.median_case, r.median_control,
                r.mean_case, r.mean_control, r.n_case, r.n_control,
            )));
        }
        let formatter_points = hover_points.clone();
        let mut plot = Plot::new(volcano_id)
            .height(360.0)
            .x_axis_label("log2 mean fold change (case / control)")
            .y_axis_label("-log10(BH q)")
            .label_formatter(move |hover| match hover {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_hover_label(position.x, position.y, &formatter_points)
                }
                _ => None,
            });
        if !significant.is_empty() && !other.is_empty() {
            plot = plot.legend(Legend::default());
        }
        plot.show(ui, |plot_ui| {
            if !other.is_empty() {
                plot_ui.points(
                    Points::new("Other tested features", other)
                        .color(egui::Color32::from_gray(100))
                        .radius(2.0),
                );
            }
            if !significant.is_empty() {
                plot_ui.points(
                    Points::new("BH q ≤ 0.05", significant)
                        .color(egui::Color32::from_rgb(230, 170, 70))
                        .radius(3.5),
                );
            }
        });
        if valid.len() > hover_points.len() {
            ui.label(egui::RichText::new(format!(
                "For responsive interaction, the plot displays {} representative points from {} tested features; all features remain in the ranked table and exports.",
                hover_points.len(), valid.len()
            )).small().weak());
        } else {
            ui.label(egui::RichText::new(format!(
                "Hover points for {feature_name} identity, effect size, significance, and group summaries. These are discovery candidates, not covariate-adjusted differential-expression results."
            )).small().weak());
        }
        ui.collapsing("Detection shift (case vs control)", |ui| {
            self.render_expression_detection_shift(
                ui,
                result,
                feature_name,
                feature_plural,
                detection_id,
            );
        });
        self.render_feature_test_table(
            ui,
            &format!("Top differential {feature_plural}"),
            &result.tests,
            560.0,
        );
        if feature_name == "Gene" {
            ui.label(
                egui::RichText::new(
                    "Use the Corrected / standard layer for count-based adjusted models and optional GMT pathway analysis."
                )
                .small()
                .weak(),
            );
        }
    }

    fn render_bcr_comparison_result(&mut self, ui: &mut egui::Ui, result: &BcrComparisonResult) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("BCR files used: {}", result.files_used));
            ui.separator();
            ui.label(format!(
                "Files outside comparison/unmatched: {}",
                result.files_skipped_not_in_groups + result.files_skipped_unmatched
            ));
            ui.separator();
            ui.label(format!(
                "Samples summarized: {}",
                result.sample_summaries.len()
            ));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(
                format!("File warnings ({})", result.file_errors.len()),
                |ui| {
                    for warning in &result.file_errors {
                        ui.label(warning);
                    }
                },
            );
        }
        ui.collapsing("Group distribution for a BCR metric", |ui| {
            self.render_bcr_metric_distribution(ui, result);
        });
        self.render_feature_effect_plot(
            ui,
            "cohort_bcr_metric_effects_v05",
            "BCR sample metrics",
            &result.metric_tests,
        );
        self.render_feature_test_table(ui, "Sample-level BCR metrics", &result.metric_tests, 360.0);
        ui.collapsing("BCR heavy-chain isotype usage", |ui| {
            self.render_feature_test_table(
                ui,
                "Isotype fractions from BCR heavy-chain reports",
                &result.isotype_tests,
                420.0,
            );
        });
        ui.collapsing("V-gene usage", |ui| {
            self.render_feature_test_table(
                ui,
                "V-gene fraction tests",
                &result.v_gene_tests,
                540.0,
            );
        });
        ui.collapsing("D-gene usage", |ui| {
            self.render_feature_test_table(
                ui,
                "D-gene fraction tests",
                &result.d_gene_tests,
                540.0,
            );
        });
        ui.collapsing("J-gene usage", |ui| {
            self.render_feature_test_table(
                ui,
                "J-gene fraction tests",
                &result.j_gene_tests,
                540.0,
            );
        });
        ui.label(
            egui::RichText::new(
                "This exploratory BCR view uses Mann-Whitney U, BH adjustment within each feature family, and Cliff's delta. Use the Corrected / standard layer for matched-depth diversity, clonotype sharing, multivariate repertoire distances, compositional analysis, PCoA/PERMANOVA, and hierarchical clustering.",
            )
            .small()
            .weak(),
        );
    }

    fn render_bcr_isotype_result(&self, ui: &mut egui::Ui, result: &BcrIsotypeComparisonResult) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Isotype files used: {}", result.files_used));
            ui.separator();
            ui.label(format!(
                "Files outside comparison/unmatched: {}",
                result.files_skipped_not_in_groups + result.files_skipped_unmatched
            ));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(
                format!("File warnings ({})", result.file_errors.len()),
                |ui| {
                    for warning in &result.file_errors {
                        ui.label(warning);
                    }
                },
            );
        }
        self.render_bcr_isotype_group_means_plot(ui, result);
        self.render_feature_effect_plot(
            ui,
            "cohort_bcr_isotype_effects_v05",
            "BCR isotype median-fraction shifts",
            &result.tests,
        );
        self.render_feature_test_table(ui, "BCR isotype fraction tests", &result.tests, 420.0);

        let mut case_means: std::collections::BTreeMap<String, Vec<f64>> =
            std::collections::BTreeMap::new();
        let mut ctrl_means: std::collections::BTreeMap<String, Vec<f64>> =
            std::collections::BTreeMap::new();
        let case_label = self
            .cohort_results
            .as_ref()
            .map(|b| b.comparison.case_label.as_str())
            .unwrap_or("case");
        for value in &result.values {
            if value.group == case_label {
                case_means
                    .entry(value.isotype.clone())
                    .or_default()
                    .push(value.fraction);
            } else {
                ctrl_means
                    .entry(value.isotype.clone())
                    .or_default()
                    .push(value.fraction);
            }
        }
        ui.collapsing("Group mean isotype composition", |ui| {
            egui::Grid::new("cohort_isotype_means_grid")
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Isotype").strong());
                    ui.label(egui::RichText::new("Case mean").strong());
                    ui.label(egui::RichText::new("Control mean").strong());
                    ui.end_row();
                    let features: std::collections::BTreeSet<_> = case_means
                        .keys()
                        .chain(ctrl_means.keys())
                        .cloned()
                        .collect();
                    for feature in features {
                        let cm = case_means
                            .get(&feature)
                            .map(|v| v.iter().sum::<f64>() / v.len().max(1) as f64)
                            .unwrap_or(0.0);
                        let hm = ctrl_means
                            .get(&feature)
                            .map(|v| v.iter().sum::<f64>() / v.len().max(1) as f64)
                            .unwrap_or(0.0);
                        ui.label(feature);
                        ui.label(format!("{cm:.4}"));
                        ui.label(format!("{hm:.4}"));
                        ui.end_row();
                    }
                });
        });
    }

    fn render_tcr_comparison_result(&self, ui: &mut egui::Ui, result: &TcrComparisonResult) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "{} files used: {}",
                result.chain, result.files_used
            ));
            ui.separator();
            ui.label(format!(
                "Files outside comparison/unmatched: {}",
                result.files_skipped_not_in_groups
            ));
            ui.separator();
            ui.label(format!(
                "Samples summarized: {}",
                result.sample_summaries.len()
            ));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(
                format!("File warnings ({})", result.file_errors.len()),
                |ui| {
                    for warning in &result.file_errors {
                        ui.label(warning);
                    }
                },
            );
        }

        let id_prefix = if result.chain.to_ascii_lowercase().contains("alpha") {
            "cohort_tcr_alpha"
        } else {
            "cohort_tcr_beta"
        };
        self.render_feature_effect_plot(
            ui,
            &format!("{id_prefix}_metric_effects_v06"),
            &format!("{} sample-level repertoire metrics", result.chain),
            &result.metric_tests,
        );
        self.render_feature_test_table(
            ui,
            &format!("{} sample-level repertoire metrics", result.chain),
            &result.metric_tests,
            360.0,
        );
        ui.collapsing("V-gene usage", |ui| {
            self.render_feature_effect_plot(
                ui,
                &format!("{id_prefix}_v_gene_effects_v06"),
                "V-gene fraction shifts",
                &result.v_gene_tests,
            );
            self.render_feature_test_table(
                ui,
                "V-gene fraction tests",
                &result.v_gene_tests,
                540.0,
            );
        });
        ui.collapsing("J-gene usage", |ui| {
            self.render_feature_effect_plot(
                ui,
                &format!("{id_prefix}_j_gene_effects_v06"),
                "J-gene fraction shifts",
                &result.j_gene_tests,
            );
            self.render_feature_test_table(
                ui,
                "J-gene fraction tests",
                &result.j_gene_tests,
                540.0,
            );
        });
        ui.collapsing("V-J pair usage", |ui| {
            self.render_feature_effect_plot(
                ui,
                &format!("{id_prefix}_vj_pair_effects_v06"),
                "V-J pair fraction shifts",
                &result.vj_pair_tests,
            );
            self.render_feature_test_table(
                ui,
                "V-J pair fraction tests",
                &result.vj_pair_tests,
                540.0,
            );
        });
        ui.label(
            egui::RichText::new(
                "This exploratory TCR view summarizes depth-dependent repertoire metrics and V/J composition. Use the Corrected / standard layer for matched-depth rarefaction, Morisita-Horn and Jaccard distances, PCoA/PERMANOVA, clonotype sharing, convergence, compositional testing, and hierarchical clustering.",
            )
            .small()
            .weak(),
        );
    }

    fn render_immune_comparison_result(&self, ui: &mut egui::Ui, result: &ImmuneComparisonResult) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Immune-infiltrate files used: {}",
                result.files_used
            ));
            ui.separator();
            ui.label(format!(
                "Files outside comparison/unmatched: {}",
                result.files_skipped_not_in_groups
            ));
            ui.separator();
            ui.label(format!("Cell types tested: {}", result.tests.len()));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(
                format!("File warnings ({})", result.file_errors.len()),
                |ui| {
                    for warning in &result.file_errors {
                        ui.label(warning);
                    }
                },
            );
        }
        self.render_immune_group_means_plot(ui, result);
        self.render_feature_effect_plot(
            ui,
            "cohort_immune_effects_v06",
            "Immune-cell ssGSEA shifts",
            &result.tests,
        );
        self.render_feature_test_table(ui, "Immune-cell ssGSEA tests", &result.tests, 520.0);
    }

    fn render_immune_group_means_plot(&self, ui: &mut egui::Ui, result: &ImmuneComparisonResult) {
        let Some(bundle) = &self.cohort_results else {
            return;
        };
        let case_label = bundle.comparison.case_label.clone();
        let control_label = bundle.comparison.control_label.clone();
        let mut case_values: std::collections::BTreeMap<String, Vec<f64>> =
            std::collections::BTreeMap::new();
        let mut ctrl_values: std::collections::BTreeMap<String, Vec<f64>> =
            std::collections::BTreeMap::new();
        for value in &result.values {
            if value.group == case_label {
                case_values
                    .entry(value.cell_type.clone())
                    .or_default()
                    .push(value.score);
            } else if value.group == control_label {
                ctrl_values
                    .entry(value.cell_type.clone())
                    .or_default()
                    .push(value.score);
            }
        }
        let features: Vec<String> = case_values
            .keys()
            .chain(ctrl_values.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if features.is_empty() {
            return;
        }
        let mut case_bars = Vec::new();
        let mut ctrl_bars = Vec::new();
        for (idx, feature) in features.iter().enumerate() {
            let case_mean = case_values
                .get(feature)
                .filter(|v| !v.is_empty())
                .map(|v| v.iter().sum::<f64>() / v.len() as f64)
                .unwrap_or(f64::NAN);
            let ctrl_mean = ctrl_values
                .get(feature)
                .filter(|v| !v.is_empty())
                .map(|v| v.iter().sum::<f64>() / v.len() as f64)
                .unwrap_or(f64::NAN);
            if ctrl_mean.is_finite() {
                ctrl_bars.push(
                    Bar::new(idx as f64 - 0.18, ctrl_mean)
                        .name(format!(
                            "{} — {}: {}",
                            feature,
                            control_label,
                            format_value(ctrl_mean)
                        ))
                        .width(0.32),
                );
            }
            if case_mean.is_finite() {
                case_bars.push(
                    Bar::new(idx as f64 + 0.18, case_mean)
                        .name(format!(
                            "{} — {}: {}",
                            feature,
                            case_label,
                            format_value(case_mean)
                        ))
                        .width(0.32),
                );
            }
        }
        let axis_features = features.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("cohort_immune_group_means_v06")
            .height(340.0)
            .x_axis_label("Immune cell type")
            .y_axis_label("Mean ssGSEA score")
            .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_features))
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(
                    BarChart::new(control_label.clone(), ctrl_bars)
                        .color(egui::Color32::from_gray(145)),
                );
                plot_ui.bar_chart(BarChart::new(case_label.clone(), case_bars).color(accent));
            });
        ui.label(
            egui::RichText::new("Group means for each immune-cell ssGSEA score; use the ranked test table below for effect size and significance.")
                .small()
                .weak(),
        );
    }

    fn render_expression_detection_shift(
        &self,
        ui: &mut egui::Ui,
        result: &GeneComparisonResult,
        feature_name: &str,
        feature_plural: &str,
        plot_id: &'static str,
    ) {
        let valid = result
            .tests
            .iter()
            .filter_map(|r| Some((r, r.detection_case?, r.detection_control?)))
            .filter(|(_, case, ctrl)| case.is_finite() && ctrl.is_finite())
            .collect::<Vec<_>>();
        if valid.is_empty() {
            ui.label(format!(
                "Detection fractions are not available for this {} result.",
                feature_name.to_ascii_lowercase()
            ));
            return;
        }
        let selected = bounded_plot_indices(valid.len(), 6000, 800);
        let mut significant = Vec::new();
        let mut other = Vec::new();
        let mut hover_points = Vec::with_capacity(selected.len());
        for idx in selected {
            let (r, case, ctrl) = valid[idx];
            let xy = [100.0 * ctrl, 100.0 * case];
            if r.q_value.is_finite() && r.q_value <= 0.05 {
                significant.push(xy);
            } else {
                other.push(xy);
            }
            hover_points.push((xy[0], xy[1], format!(
                "{}{}\ncase detection: {:.1}%\ncontrol detection: {:.1}%\nΔ detection: {:+.1} pp\np: {:.3e}\nBH q: {:.3e}",
                r.feature,
                if r.secondary_id.is_empty() { String::new() } else { format!(" ({})", r.secondary_id) },
                100.0 * case, 100.0 * ctrl, 100.0 * (case - ctrl), r.p_value, r.q_value,
            )));
        }
        let formatter_points = hover_points.clone();
        let mut plot = Plot::new(plot_id)
            .height(350.0)
            .x_axis_label("Control detection (%)")
            .y_axis_label("Case detection (%)")
            .label_formatter(move |hover| match hover {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_hover_label(position.x, position.y, &formatter_points)
                }
                _ => None,
            });
        if !significant.is_empty() && !other.is_empty() {
            plot = plot.legend(Legend::default());
        }
        plot.show(ui, |plot_ui| {
            plot_ui.line(
                Line::new("Equal detection", vec![[0.0, 0.0], [100.0, 100.0]])
                    .color(egui::Color32::from_gray(120))
                    .allow_hover(false),
            );
            if !other.is_empty() {
                plot_ui.points(
                    Points::new(format!("Other {feature_plural}"), other)
                        .color(egui::Color32::from_gray(95))
                        .radius(2.0),
                );
            }
            if !significant.is_empty() {
                plot_ui.points(
                    Points::new("BH q ≤ 0.05", significant)
                        .color(egui::Color32::from_rgb(230, 170, 70))
                        .radius(3.5),
                );
            }
        });
        if valid.len() > hover_points.len() {
            ui.label(egui::RichText::new(format!(
                "Displaying {} representative points from {} features for responsive interaction; the full result table is unchanged.",
                hover_points.len(), valid.len()
            )).small().weak());
        }
    }

    fn render_bcr_metric_distribution(&mut self, ui: &mut egui::Ui, result: &BcrComparisonResult) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Metric:");
            egui::ComboBox::from_id_salt("cohort_bcr_metric_distribution_combo")
                .selected_text(self.cohort_bcr_metric_view.display_name())
                .show_ui(ui, |ui| {
                    for metric in BcrMetricView::ALL {
                        ui.selectable_value(
                            &mut self.cohort_bcr_metric_view,
                            metric,
                            metric.display_name(),
                        );
                    }
                });
        });
        let Some(bundle) = &self.cohort_results else {
            return;
        };
        let case_label = bundle.comparison.case_label.clone();
        let control_label = bundle.comparison.control_label.clone();
        let metric = self.cohort_bcr_metric_view;
        let mut case_values = result
            .sample_summaries
            .iter()
            .filter(|r| r.group == case_label)
            .map(|r| (r.sample_id.clone(), metric.value(r)))
            .filter(|(_, value)| value.is_finite())
            .collect::<Vec<_>>();
        let mut ctrl_values = result
            .sample_summaries
            .iter()
            .filter(|r| r.group == control_label)
            .map(|r| (r.sample_id.clone(), metric.value(r)))
            .filter(|(_, value)| value.is_finite())
            .collect::<Vec<_>>();
        if case_values.len() < 2 || ctrl_values.len() < 2 {
            ui.label("Not enough mapped samples to draw a case/control distribution.");
            return;
        }
        let case_numeric = case_values.iter().map(|(_, v)| *v).collect::<Vec<_>>();
        let ctrl_numeric = ctrl_values.iter().map(|(_, v)| *v).collect::<Vec<_>>();
        let (Some(case_spread), Some(ctrl_spread)) = (
            box_spread_from_values(&case_numeric),
            box_spread_from_values(&ctrl_numeric),
        ) else {
            ui.label("Could not calculate quartiles for this metric.");
            return;
        };
        let accent = ui.visuals().selection.bg_fill;
        let case_fill =
            egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 90);
        let ctrl_fill = egui::Color32::from_rgba_unmultiplied(155, 155, 155, 75);
        let boxes = vec![
            BoxElem::new(0.0, ctrl_spread)
                .name(control_label.clone())
                .box_width(0.55)
                .fill(ctrl_fill),
            BoxElem::new(1.0, case_spread)
                .name(case_label.clone())
                .box_width(0.55)
                .fill(case_fill),
        ];
        let labels = vec![control_label.clone(), case_label.clone()];
        let box_plot = BoxPlot::new("BCR group distributions", boxes);
        Plot::new("cohort_bcr_group_distribution_v05")
            .height(330.0)
            .x_axis_label("Group")
            .y_axis_label(metric.display_name())
            .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &labels))
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                plot_ui.box_plot(box_plot);
                for (group_x, values) in [(0.0, &mut ctrl_values), (1.0, &mut case_values)] {
                    for (idx, (sample, value)) in values.iter().enumerate() {
                        let jitter = (((idx * 37) % 101) as f64 / 100.0 - 0.5) * 0.24;
                        plot_ui.points(
                            Points::new(
                                format!(
                                    "{}\n{}: {}",
                                    sample,
                                    metric.display_name(),
                                    format_value(*value)
                                ),
                                vec![[group_x + jitter, *value]],
                            )
                            .radius(2.7),
                        );
                    }
                }
            });
        ui.label(
            egui::RichText::new(format!(
                "{}: n={}  |  {}: n={}",
                control_label,
                ctrl_numeric.len(),
                case_label,
                case_numeric.len(),
            ))
            .small()
            .weak(),
        );
    }

    fn render_bcr_isotype_group_means_plot(
        &self,
        ui: &mut egui::Ui,
        result: &BcrIsotypeComparisonResult,
    ) {
        let Some(bundle) = &self.cohort_results else {
            return;
        };
        let case_label = bundle.comparison.case_label.clone();
        let control_label = bundle.comparison.control_label.clone();
        let mut case_values: std::collections::BTreeMap<String, Vec<f64>> =
            std::collections::BTreeMap::new();
        let mut ctrl_values: std::collections::BTreeMap<String, Vec<f64>> =
            std::collections::BTreeMap::new();
        for value in &result.values {
            if value.group == case_label {
                case_values
                    .entry(value.isotype.clone())
                    .or_default()
                    .push(value.fraction);
            } else if value.group == control_label {
                ctrl_values
                    .entry(value.isotype.clone())
                    .or_default()
                    .push(value.fraction);
            }
        }
        let features: Vec<String> = case_values
            .keys()
            .chain(ctrl_values.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if features.is_empty() {
            return;
        }
        let mut case_bars = Vec::new();
        let mut ctrl_bars = Vec::new();
        for (idx, feature) in features.iter().enumerate() {
            let cm = case_values
                .get(feature)
                .map(|v| v.iter().sum::<f64>() / v.len() as f64)
                .unwrap_or(0.0);
            let hm = ctrl_values
                .get(feature)
                .map(|v| v.iter().sum::<f64>() / v.len() as f64)
                .unwrap_or(0.0);
            case_bars.push(
                Bar::new(idx as f64 + 0.18, cm)
                    .name(format!("{} — {}: {:.4}", feature, case_label, cm))
                    .width(0.32),
            );
            ctrl_bars.push(
                Bar::new(idx as f64 - 0.18, hm)
                    .name(format!("{} — {}: {:.4}", feature, control_label, hm))
                    .width(0.32),
            );
        }
        let axis_features = features.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("cohort_bcr_isotype_group_means_v05")
            .height(320.0)
            .x_axis_label("Isotype")
            .y_axis_label("Mean isotype fraction")
            .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_features))
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(
                    BarChart::new(control_label.clone(), ctrl_bars)
                        .color(egui::Color32::from_gray(145)),
                );
                plot_ui.bar_chart(BarChart::new(case_label.clone(), case_bars).color(accent));
            });
        ui.label(
            egui::RichText::new("Group means from the dedicated BCR isotype-composition files.")
                .small()
                .weak(),
        );
    }

    fn render_feature_effect_plot(
        &self,
        ui: &mut egui::Ui,
        plot_id: &str,
        title: &str,
        tests: &[FeatureTestResult],
    ) {
        let shown: Vec<_> = tests
            .iter()
            .filter(|r| r.effect.is_finite())
            .take(25)
            .collect();
        if shown.is_empty() {
            ui.label("No usable feature effects to plot.");
            return;
        }
        let labels: Vec<String> = shown.iter().map(|r| r.feature.clone()).collect();
        let axis_labels = labels.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new(plot_id)
            .height((140.0 + 22.0 * shown.len() as f32).min(420.0))
            .x_axis_label("Effect (case - control)")
            .y_axis_formatter(move |mark, _range| {
                let idx = mark.value.round() as isize;
                if idx >= 0 && (idx as usize) < axis_labels.len() {
                    axis_labels[idx as usize].clone()
                } else {
                    String::new()
                }
            })
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                for (idx, result) in shown.iter().enumerate() {
                    let label = format!(
                        "{}\nEffect: {:.4}\nCliff's δ: {:.3}\np: {:.3e}\nq: {:.3e}",
                        result.feature,
                        result.effect,
                        result.cliffs_delta,
                        result.p_value,
                        result.q_value
                    );
                    let color = if result.q_value.is_finite() && result.q_value <= 0.05 {
                        egui::Color32::from_rgb(230, 170, 70)
                    } else {
                        accent
                    };
                    plot_ui.points(
                        Points::new(label, vec![[result.effect, idx as f64]])
                            .color(color)
                            .radius(5.0),
                    );
                }
            });
        ui.label(egui::RichText::new(title).small().weak());
    }

    fn render_feature_test_table(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        tests: &[FeatureTestResult],
        max_height: f32,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(title).strong());
            ui.label(
                egui::RichText::new(format!("({} rows)", tests.len()))
                    .small()
                    .weak(),
            );
        });

        if tests.is_empty() {
            ui.label(egui::RichText::new("No rows available for this feature family.").weak());
            return;
        }

        let row_height = 24.0_f32;
        let content_height = row_height * (tests.len().saturating_add(1) as f32) + 10.0;
        let desired_height = if tests.len() > 8 {
            max_height.max(360.0)
        } else {
            content_height.max(120.0).min(max_height.max(360.0))
        };
        fixed_table_viewport(
            ui,
            &format!("feature_test_scroll_{title}"),
            desired_height,
            1040.0,
            |ui| {
                egui::Grid::new(format!("feature_test_grid_{title}"))
                    .striped(true)
                    .num_columns(9)
                    .show(ui, |ui| {
                        for heading in [
                            "Feature",
                            "Effect",
                            "Cliff δ",
                            "p",
                            "BH q",
                            "Median case",
                            "Median ctrl",
                            "n case",
                            "n ctrl",
                        ] {
                            ui.label(egui::RichText::new(heading).strong());
                        }
                        ui.end_row();
                        for r in tests {
                            ui.monospace(&r.feature);
                            ui.label(format!("{:.4}", r.effect));
                            ui.label(format!("{:.3}", r.cliffs_delta));
                            ui.label(format!("{:.3e}", r.p_value));
                            ui.label(if r.q_value.is_finite() {
                                format!("{:.3e}", r.q_value)
                            } else {
                                "NA".to_string()
                            });
                            ui.label(format!("{:.4}", r.median_case));
                            ui.label(format!("{:.4}", r.median_control));
                            ui.label(r.n_case.to_string());
                            ui.label(r.n_control.to_string());
                            ui.end_row();
                        }
                    });
            },
        );
    }

    fn save_cohort_result_tables(&self, parent: &Path) -> anyhow::Result<PathBuf> {
        let bundle = self
            .cohort_results
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No cohort results are available"))?;
        let safe_name: String = bundle
            .comparison
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let outdir = parent.join(format!("4c_analysis_{safe_name}"));
        fs::create_dir_all(&outdir)?;
        if let Some(mapping) = &self.cohort_mapping {
            write_mapping_audit(&outdir.join("sample_mapping_audit.tsv"), mapping)?;
            write_mapping_audit_csv(&outdir.join("sample_mapping_audit.csv"), mapping)?;
        }
        if let Some(gene) = &bundle.gene {
            write_feature_tests(&outdir.join("gene_expression_tests.tsv"), &gene.tests)?;
        }
        if let Some(transcript) = &bundle.transcript {
            write_feature_tests(
                &outdir.join("transcript_expression_tests.tsv"),
                &transcript.tests,
            )?;
        }
        if let Some(bcr) = &bundle.bcr {
            write_bcr_sample_summaries(
                &outdir.join("bcr_sample_summaries.tsv"),
                &bcr.sample_summaries,
            )?;
            write_feature_tests(
                &outdir.join("bcr_sample_metric_tests.tsv"),
                &bcr.metric_tests,
            )?;
            write_feature_tests(
                &outdir.join("bcr_heavy_isotype_fraction_tests.tsv"),
                &bcr.isotype_tests,
            )?;
            write_feature_tests(
                &outdir.join("bcr_v_gene_fraction_tests.tsv"),
                &bcr.v_gene_tests,
            )?;
            write_feature_tests(
                &outdir.join("bcr_d_gene_fraction_tests.tsv"),
                &bcr.d_gene_tests,
            )?;
            write_feature_tests(
                &outdir.join("bcr_j_gene_fraction_tests.tsv"),
                &bcr.j_gene_tests,
            )?;
            if !bcr.file_errors.is_empty() {
                fs::write(
                    outdir.join("bcr_file_warnings.txt"),
                    bcr.file_errors.join("\n"),
                )?;
            }
        }
        if let Some(iso) = &bundle.bcr_isotype {
            write_bcr_isotype_values(&outdir.join("bcr_isotype_sample_values.tsv"), &iso.values)?;
            write_feature_tests(&outdir.join("bcr_isotype_fraction_tests.tsv"), &iso.tests)?;
            if !iso.file_errors.is_empty() {
                fs::write(
                    outdir.join("bcr_isotype_file_warnings.txt"),
                    iso.file_errors.join("\n"),
                )?;
            }
        }
        if let Some(tcr) = &bundle.tcr_alpha {
            write_tcr_sample_summaries(
                &outdir.join("tcr_alpha_sample_summaries.tsv"),
                &tcr.sample_summaries,
            )?;
            write_feature_tests(
                &outdir.join("tcr_alpha_sample_metric_tests.tsv"),
                &tcr.metric_tests,
            )?;
            write_feature_tests(
                &outdir.join("tcr_alpha_v_gene_fraction_tests.tsv"),
                &tcr.v_gene_tests,
            )?;
            write_feature_tests(
                &outdir.join("tcr_alpha_j_gene_fraction_tests.tsv"),
                &tcr.j_gene_tests,
            )?;
            write_feature_tests(
                &outdir.join("tcr_alpha_vj_pair_fraction_tests.tsv"),
                &tcr.vj_pair_tests,
            )?;
            if !tcr.file_errors.is_empty() {
                fs::write(
                    outdir.join("tcr_alpha_file_warnings.txt"),
                    tcr.file_errors.join("\n"),
                )?;
            }
        }
        if let Some(tcr) = &bundle.tcr_beta {
            write_tcr_sample_summaries(
                &outdir.join("tcr_beta_sample_summaries.tsv"),
                &tcr.sample_summaries,
            )?;
            write_feature_tests(
                &outdir.join("tcr_beta_sample_metric_tests.tsv"),
                &tcr.metric_tests,
            )?;
            write_feature_tests(
                &outdir.join("tcr_beta_v_gene_fraction_tests.tsv"),
                &tcr.v_gene_tests,
            )?;
            write_feature_tests(
                &outdir.join("tcr_beta_j_gene_fraction_tests.tsv"),
                &tcr.j_gene_tests,
            )?;
            write_feature_tests(
                &outdir.join("tcr_beta_vj_pair_fraction_tests.tsv"),
                &tcr.vj_pair_tests,
            )?;
            if !tcr.file_errors.is_empty() {
                fs::write(
                    outdir.join("tcr_beta_file_warnings.txt"),
                    tcr.file_errors.join("\n"),
                )?;
            }
        }
        if let Some(immune) = &bundle.immune {
            write_immune_values(
                &outdir.join("immune_infiltrate_sample_values.tsv"),
                &immune.values,
            )?;
            write_feature_tests(&outdir.join("immune_infiltrate_tests.tsv"), &immune.tests)?;
            if !immune.file_errors.is_empty() {
                fs::write(
                    outdir.join("immune_infiltrate_file_warnings.txt"),
                    immune.file_errors.join("\n"),
                )?;
            }
        }
        if let Some(qc) = self.current_outlier_snapshot_for_comparison(&bundle.comparison, false) {
            write_outlier_filter_snapshot_csv(&outdir.join("sample_outlier_qc.csv"), &qc)?;
            write_outlier_filter_modalities_tsv(
                &outdir.join("sample_outlier_qc_modalities.tsv"),
                &qc,
            )?;
            write_outlier_filter_summary(&outdir.join("sample_outlier_qc_summary.txt"), &qc)?;
        }
        let notes = format!(
            "Exploratory comparison: {}\nCase: {}\nControl: {}\n\nThis first-pass analysis intentionally uses all available mapped samples, including longitudinal samples, without repeated-measures or covariate adjustment. If a sample outlier QC audit was run, its reviewed decisions are exported separately; those exclusions were not applied to this exploratory result.\n",
            bundle.comparison.name, bundle.comparison.case_label, bundle.comparison.control_label,
        );
        fs::write(outdir.join("README.txt"), notes)?;
        Ok(outdir)
    }

    fn save_corrected_result_tables(&self, parent: &Path) -> anyhow::Result<PathBuf> {
        let bundle = self
            .corrected_results
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No corrected results are available"))?;
        let safe_name: String = bundle
            .comparison
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let outdir = parent.join(format!("4c_corrected_analysis_{safe_name}"));
        fs::create_dir_all(&outdir)?;
        if let Some(mapping) = &self.cohort_mapping {
            write_mapping_audit(&outdir.join("sample_mapping_audit.tsv"), mapping)?;
            write_mapping_audit_csv(&outdir.join("sample_mapping_audit.csv"), mapping)?;
        }
        if let Some(r) = &bundle.gene {
            write_adjusted_tests(&outdir.join("gene_adjusted_tests.tsv"), &r.tests)?;
            if let Some(o) = &r.pca {
                write_ordination(&outdir.join("gene_pca.tsv"), o)?;
            }
            if let Some(t) = &r.dendrogram {
                write_dendrogram(&outdir.join("gene_dendrogram_leaf_order.tsv"), t)?;
            }
        }
        if let Some(r) = &bundle.transcript {
            write_adjusted_tests(&outdir.join("transcript_adjusted_tests.tsv"), &r.tests)?;
            if let Some(o) = &r.pca {
                write_ordination(&outdir.join("transcript_pca.tsv"), o)?;
            }
            if let Some(t) = &r.dendrogram {
                write_dendrogram(&outdir.join("transcript_dendrogram_leaf_order.tsv"), t)?;
            }
            if let Some(dtu) = &r.dtu {
                write_adjusted_tests(
                    &outdir.join("transcript_dtu_transcript_tests.tsv"),
                    &dtu.transcript_tests,
                )?;
                write_dtu_gene_tests(
                    &outdir.join("transcript_dtu_gene_tests.tsv"),
                    &dtu.gene_tests,
                )?;
            }
        }
        for (prefix, r) in [
            ("bcr", bundle.bcr.as_ref()),
            ("tcr_alpha", bundle.tcr_alpha.as_ref()),
            ("tcr_beta", bundle.tcr_beta.as_ref()),
        ] {
            if let Some(r) = r {
                write_corrected_repertoire_samples(
                    &outdir.join(format!("{prefix}_matched_depth_sample_metrics.tsv")),
                    &r.sample_summaries,
                )?;
                write_adjusted_tests(
                    &outdir.join(format!("{prefix}_adjusted_metric_tests.tsv")),
                    &r.metric_tests,
                )?;
                if let Some(o) = &r.morisita_horn {
                    write_ordination(&outdir.join(format!("{prefix}_morisita_horn_pcoa.tsv")), o)?;
                }
                if let Some(o) = &r.jaccard {
                    write_ordination(&outdir.join(format!("{prefix}_jaccard_pcoa.tsv")), o)?;
                }
                if let Some(t) = &r.dendrogram {
                    write_dendrogram(
                        &outdir.join(format!("{prefix}_morisita_horn_dendrogram_leaf_order.tsv")),
                        t,
                    )?;
                }
                for comp in &r.compositions {
                    let fam: String = comp
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
                        &outdir.join(format!("{prefix}_{fam}_clr_tests.tsv")),
                        &comp.tests,
                    )?;
                    if let Some(o) = &comp.ordination {
                        write_ordination(
                            &outdir.join(format!("{prefix}_{fam}_aitchison_pcoa.tsv")),
                            o,
                        )?;
                    }
                    if let Some(t) = &comp.dendrogram {
                        write_dendrogram(
                            &outdir.join(format!(
                                "{prefix}_{fam}_aitchison_dendrogram_leaf_order.tsv"
                            )),
                            t,
                        )?;
                    }
                }
                if !r.warnings.is_empty() {
                    fs::write(
                        outdir.join(format!("{prefix}_warnings.txt")),
                        r.warnings.join("\n"),
                    )?;
                }
            }
        }
        if let Some(r) = &bundle.bcr_isotype {
            write_adjusted_tests(&outdir.join("bcr_isotype_clr_tests.tsv"), &r.tests)?;
            if let Some(o) = &r.ordination {
                write_ordination(&outdir.join("bcr_isotype_aitchison_pcoa.tsv"), o)?;
            }
            if let Some(t) = &r.dendrogram {
                write_dendrogram(
                    &outdir.join("bcr_isotype_aitchison_dendrogram_leaf_order.tsv"),
                    t,
                )?;
            }
            if !r.warnings.is_empty() {
                fs::write(
                    outdir.join("bcr_isotype_warnings.txt"),
                    r.warnings.join("\n"),
                )?;
            }
        }
        if let Some(r) = &bundle.immune {
            write_adjusted_tests(&outdir.join("immune_adjusted_tests.tsv"), &r.tests)?;
            if let Some(o) = &r.pca {
                write_ordination(&outdir.join("immune_pca.tsv"), o)?;
            }
            if let Some(t) = &r.dendrogram {
                write_dendrogram(&outdir.join("immune_dendrogram_leaf_order.tsv"), t)?;
            }
            if !r.warnings.is_empty() {
                fs::write(outdir.join("immune_warnings.txt"), r.warnings.join("\n"))?;
            }
        }
        if let Some(r) = &bundle.pathway {
            write_pathway_ora(&outdir.join("pathway_ora.tsv"), r)?;
        }
        if let Some(qc) = &self.corrected_outlier_snapshot {
            write_outlier_filter_snapshot_csv(&outdir.join("sample_outlier_qc.csv"), qc)?;
            write_outlier_filter_modalities_tsv(
                &outdir.join("sample_outlier_qc_modalities.tsv"),
                qc,
            )?;
            write_outlier_filter_summary(&outdir.join("sample_outlier_qc_summary.txt"), qc)?;
        }
        let qc_note = if let Some(qc) = &self.corrected_outlier_snapshot {
            format!(
                "\nSample outlier QC: {} reviewed exclusion(s) were applied before this analysis (case {} -> {}, control {} -> {}). See sample_outlier_qc.csv and sample_outlier_qc_summary.txt for the exact samples, automatic flags, manual overrides, modality distances, and rule used.\n",
                qc.excluded_ids.len(),
                qc.original_case_n,
                qc.filtered_case_n,
                qc.original_control_n,
                qc.filtered_control_n
            )
        } else {
            "\nSample outlier QC: no reviewed outlier exclusions were applied to this analysis.\n"
                .to_string()
        };
        let notes = format!(
            "4C Cohort Explorer corrected/standard analysis\nComparison: {}\nCase: {}\nControl: {}\nSubject column: {}\nCovariates: {}\n{}\nExpression uses positive-count median-ratio normalization with a negative-binomial log-link count model, optional covariates, subject-clustered inference when repeated samples are configured, and a subject-collapsed non-parametric sensitivity analysis.\n\nRepertoire analyses use matched-depth exact-without-replacement Monte-Carlo rarefaction, subject-aware public/private sharing, Morisita-Horn and Jaccard distances, PCoA/PERMANOVA, average-linkage hierarchical clustering, and CLR/Aitchison composition analysis.\n",
            bundle.comparison.name,
            bundle.comparison.case_label,
            bundle.comparison.control_label,
            bundle.spec.subject_column.as_deref().unwrap_or("none"),
            if bundle.spec.covariates.is_empty() {
                "none".to_string()
            } else {
                bundle.spec.covariates.join(", ")
            },
            qc_note
        );
        fs::write(outdir.join("README.txt"), notes)?;
        Ok(outdir)
    }

    fn current_outlier_snapshot_for_comparison(
        &self,
        comparison: &CohortComparison,
        applied_to_analysis: bool,
    ) -> Option<OutlierFilterSnapshot> {
        let audit = self.outlier_results.as_ref()?;
        if audit.comparison_name != comparison.name
            && !comparison.name.starts_with(&audit.comparison_name)
        {
            return None;
        }
        let filtered = filter_comparison_excluding(comparison, &self.outlier_excluded_ids);
        Some(build_outlier_filter_snapshot(
            audit,
            &self.outlier_excluded_ids,
            comparison,
            &filtered,
            applied_to_analysis,
        ))
    }

    fn save_focused_result_tables(
        &self,
        folder: &Path,
        summary: &FocusedCandidateSummary,
    ) -> anyhow::Result<PathBuf> {
        let path = write_focused_candidate_tables(folder, summary)?;
        if let Some(qc) = &self.focused_outlier_snapshot {
            write_outlier_filter_snapshot_csv(&path.join("sample_outlier_qc.csv"), qc)?;
            write_outlier_filter_modalities_tsv(
                &path.join("sample_outlier_qc_modalities.tsv"),
                qc,
            )?;
            write_outlier_filter_summary(&path.join("sample_outlier_qc_summary.txt"), qc)?;
        }
        Ok(path)
    }

    fn configure_corrected_metadata_defaults(&mut self) {
        let Some(meta) = &self.cohort_metadata else {
            return;
        };
        let columns = meta.columns.clone();
        if self
            .corrected_subject_column
            .as_ref()
            .is_none_or(|c| !columns.contains(c))
        {
            self.corrected_subject_column = columns
                .iter()
                .find(|c| {
                    let l = c.to_ascii_lowercase();
                    l == "subject"
                        || l == "subject_id"
                        || l == "patient"
                        || l == "patient_id"
                        || l == "participant"
                        || l == "participant_id"
                        || l.contains("patient id")
                        || l.contains("subject id")
                })
                .cloned();
        }
        self.corrected_covariates.retain(|c| columns.contains(c));
        if self.corrected_covariates.is_empty() {
            for c in &columns {
                let l = c.to_ascii_lowercase();
                if l == "age"
                    || l.contains("age_at")
                    || l == "sex"
                    || l == "gender"
                    || l.contains("ethnic")
                    || l == "race"
                    || l.contains("batch")
                {
                    if Some(c) != self.corrected_subject_column.as_ref() && c != &meta.id_column {
                        self.corrected_covariates.insert(c.clone());
                    }
                }
            }
        }
    }

    fn start_outlier_qc(&mut self) {
        let Some(scan) = self.cohort_scan.clone() else {
            self.cohort_status = "Load a project first.".to_string();
            return;
        };
        let Some((metadata, _, _, _)) = self.active_cohort_inputs() else {
            self.cohort_status =
                "Load metadata for the selected cohort definition first.".to_string();
            return;
        };
        let (_, comparison) = match self.build_active_comparison(&metadata) {
            Ok(value) => value,
            Err(error) => {
                self.cohort_status = format!("Could not build comparison for outlier QC: {error}");
                return;
            }
        };
        let mut modalities = Vec::new();
        if self.cohort_run_gene {
            modalities.push(CohortModality::GeneExpression);
        }
        if self.cohort_run_transcript {
            modalities.push(CohortModality::TranscriptExpression);
        }
        if self.cohort_run_bcr {
            modalities.push(CohortModality::Bcr);
        }
        if self.cohort_run_bcr_isotype {
            modalities.push(CohortModality::BcrIsotype);
        }
        if self.cohort_run_tcr_alpha {
            modalities.push(CohortModality::TcrAlpha);
        }
        if self.cohort_run_tcr_beta {
            modalities.push(CohortModality::TcrBeta);
        }
        if self.cohort_run_immune {
            modalities.push(CohortModality::ImmuneInfiltrate);
        }
        if modalities.is_empty() {
            self.cohort_status =
                "Enable at least one modality before running outlier QC.".to_string();
            return;
        }
        let config = self.outlier_config.clone();
        let backend = Arc::clone(&self.analysis_backend);
        let request = OutlierAnalysisRequest::Local(LocalOutlierRequest {
            scan,
            comparison,
            selected_modalities: modalities,
            config,
        });
        let (tx, rx) = mpsc::channel();
        self.cohort_receiver = Some(rx);
        self.cohort_running = true;
        self.cohort_progress = 0.0;
        self.cohort_status = format!("Running sample outlier QC via {}…", backend.display_name());
        self.outlier_results = None;
        std::thread::spawn(move || {
            let mut progress = |done: usize, total: usize, msg: &str| {
                let frac = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                let _ = tx.send(CohortWorkerMessage::Progress(frac, msg.to_string()));
            };
            let result = backend
                .run_outlier_qc(request, &mut progress)
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(CohortWorkerMessage::OutlierDone(result));
        });
    }

    fn start_unified_analysis(&mut self) {
        let Some(scan) = self.cohort_scan.clone() else {
            self.cohort_status = "Load a cohort project first.".to_string();
            return;
        };
        let Some((metadata, metadata_path, mapping, uses_subset)) = self.active_cohort_inputs()
        else {
            self.cohort_status =
                "Load metadata for the selected cohort definition first.".to_string();
            return;
        };
        let (comparison_definition, original_comparison) =
            match self.build_active_comparison(&metadata) {
                Ok(value) => value,
                Err(error) => {
                    self.cohort_status = format!("Could not build comparison: {error}");
                    return;
                }
            };

        let mut comparison = original_comparison.clone();
        let mut qc_snapshot = None;
        if let Some(audit) = self.outlier_results.as_ref() {
            if audit.comparison_name == original_comparison.name {
                if self.outlier_apply_to_corrected && !self.outlier_excluded_ids.is_empty() {
                    comparison = filter_comparison_excluding(
                        &original_comparison,
                        &self.outlier_excluded_ids,
                    );
                    if comparison.case_ids.len() < 2 || comparison.control_ids.len() < 2 {
                        self.cohort_status = "Reviewed QC exclusions leave fewer than two samples in one comparison group; revise the exclusion list before running the analysis.".to_string();
                        return;
                    }
                    qc_snapshot = Some(build_outlier_filter_snapshot(
                        audit,
                        &self.outlier_excluded_ids,
                        &original_comparison,
                        &comparison,
                        true,
                    ));
                } else {
                    qc_snapshot = Some(build_outlier_filter_snapshot(
                        audit,
                        &HashSet::new(),
                        &original_comparison,
                        &original_comparison,
                        false,
                    ));
                }
            }
        } else if self.outlier_apply_to_corrected && !self.outlier_excluded_ids.is_empty() {
            self.cohort_status = "Reviewed exclusions are selected but the cross-modal QC audit is unavailable. Rerun sample outlier QC so the exclusion provenance can be frozen into this analysis.".to_string();
            return;
        }

        let subject_column = self
            .corrected_subject_column
            .clone()
            .filter(|column| metadata.columns.contains(column));
        let mut covariates = self
            .corrected_covariates
            .iter()
            .filter(|column| {
                metadata.columns.contains(*column)
                    && Some(*column) != subject_column.as_ref()
                    && *column != &metadata.id_column
            })
            .cloned()
            .collect::<Vec<_>>();
        covariates.sort();
        let spec = CorrectedModelSpec {
            subject_column: subject_column.clone(),
            covariates,
            permutations: self.corrected_permutations,
            rarefaction_reps: self.corrected_rarefaction_reps,
            rarefaction_cap: self.corrected_rarefaction_cap,
            ambiguous_policy: self.corrected_ambiguous_policy,
            exclude_undetermined_isotype: self.corrected_exclude_undetermined,
            ..CorrectedModelSpec::default()
        };
        let diagnostics = if self.unified_run_diagnostics {
            Some(PbmcDiagnosticsConfig {
                subject_column: subject_column.clone(),
                batch_column: self
                    .diagnostics_batch_column
                    .clone()
                    .filter(|column| metadata.columns.contains(column)),
                family_column: self
                    .diagnostics_family_column
                    .clone()
                    .filter(|column| metadata.columns.contains(column)),
                sensitivity_column: self
                    .diagnostics_sensitivity_column
                    .clone()
                    .filter(|column| metadata.columns.contains(column)),
                known_non_pbmc_ids: parse_sample_id_text(&self.diagnostics_known_non_pbmc_text),
                suspect_ids: parse_sample_id_text(&self.diagnostics_suspect_ids_text),
                exclude_suspects_in_sensitivity: self.diagnostics_exclude_suspects,
                auto_exclude_transcriptome_outliers: false,
                auto_exclude_immune_outliers: false,
                top_candidates: self.diagnostics_top_candidates,
                outlier_z_threshold: self.diagnostics_outlier_z,
                ..PbmcDiagnosticsConfig::default()
            })
        } else {
            None
        };
        let modalities = CohortModalitySelection {
            gene: self.cohort_run_gene,
            transcript: self.cohort_run_transcript,
            bcr: self.cohort_run_bcr,
            bcr_isotype: self.cohort_run_bcr_isotype,
            tcr_alpha: self.cohort_run_tcr_alpha,
            tcr_beta: self.cohort_run_tcr_beta,
            immune: self.cohort_run_immune,
            pathway: self.corrected_run_pathway,
        };
        if !modalities.any_primary() {
            self.cohort_status = "Enable at least one analysis modality.".to_string();
            return;
        }
        let cohort = CohortDefinitionSnapshot {
            project_root: scan.root.clone(),
            metadata_path,
            metadata_id_column: metadata.id_column.clone(),
            uses_filtered_metadata_subset: uses_subset,
            comparison_definition,
            comparison: comparison.clone(),
            mapping,
        };
        let settings = CohortAnalysisSettingsSnapshot {
            modalities,
            corrected: spec,
            pathway_gmt_path: self.corrected_gmt_path.clone(),
            pathway_q_threshold: self.corrected_pathway_q,
            pathway_effect_threshold: self.corrected_pathway_effect,
            exploratory_direction_check: self.focused_run_exploratory_concordance,
            candidate: self.focused_settings.clone(),
            pbmc_diagnostics: diagnostics,
        };

        let backend = Arc::clone(&self.analysis_backend);
        let request = UnifiedAnalysisRequest::Local(LocalUnifiedAnalysisRequest {
            scan,
            metadata,
            cohort,
            settings,
            qc_snapshot,
        });
        let (tx, rx) = mpsc::channel();
        self.cohort_receiver = Some(rx);
        self.cohort_running = true;
        self.cohort_progress = 0.0;
        self.cohort_status = format!(
            "Running unified {} via {}…",
            comparison.name,
            backend.display_name()
        );
        self.unified_results = None;
        self.corrected_results = None;
        self.focused_results = None;
        self.diagnostics_results = None;
        self.corrected_outlier_snapshot = None;
        std::thread::spawn(move || {
            let mut progress = |done: usize, total: usize, message: &str| {
                let fraction = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                let _ = tx.send(CohortWorkerMessage::Progress(fraction, message.to_string()));
            };
            let result = backend
                .run_unified_analysis(request, &mut progress)
                .map_err(|error| format!("{error:#}"));
            let _ = tx.send(CohortWorkerMessage::UnifiedDone(result));
        });
    }

    fn start_corrected_analysis(&mut self) {
        let (Some(scan), Some(metadata)) = (self.cohort_scan.clone(), self.cohort_metadata.clone())
        else {
            self.cohort_status = "Load a project and metadata first.".to_string();
            return;
        };
        let original_comparison = match build_comparison(&metadata, self.cohort_preset) {
            Ok(c) => c,
            Err(e) => {
                self.cohort_status = format!("Could not build comparison: {e:#}");
                return;
            }
        };
        let mut comparison = original_comparison.clone();
        self.corrected_outlier_snapshot = None;
        if self.outlier_apply_to_corrected && !self.outlier_excluded_ids.is_empty() {
            let Some(audit) = self.outlier_results.as_ref() else {
                self.cohort_status="Outlier exclusions are enabled but the QC audit is no longer available. Rerun Sample outlier QC before the corrected analysis so the exclusions are auditable.".to_string();
                return;
            };
            if audit.comparison_name != original_comparison.name {
                self.cohort_status = format!(
                    "The current outlier QC was generated for '{}', but the corrected comparison is '{}'. Rerun Sample outlier QC for the current comparison before applying exclusions.",
                    audit.comparison_name, original_comparison.name
                );
                return;
            }
            comparison =
                filter_comparison_excluding(&original_comparison, &self.outlier_excluded_ids);
            if comparison.case_ids.len() < 2 || comparison.control_ids.len() < 2 {
                self.cohort_status = "Outlier filtering leaves fewer than two samples in one comparison group; review the exclusion list.".to_string();
                return;
            }
            self.corrected_outlier_snapshot = Some(build_outlier_filter_snapshot(
                audit,
                &self.outlier_excluded_ids,
                &original_comparison,
                &comparison,
                true,
            ));
        }
        let mut covariates = self
            .corrected_covariates
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        covariates.sort();
        let spec = CorrectedModelSpec {
            subject_column: self.corrected_subject_column.clone(),
            covariates,
            permutations: self.corrected_permutations,
            rarefaction_reps: self.corrected_rarefaction_reps,
            rarefaction_cap: self.corrected_rarefaction_cap,
            ambiguous_policy: self.corrected_ambiguous_policy,
            exclude_undetermined_isotype: self.corrected_exclude_undetermined,
            ..CorrectedModelSpec::default()
        };
        let run_pathway = self.corrected_run_pathway;
        let run_gene = self.cohort_run_gene || run_pathway;
        let run_transcript = self.cohort_run_transcript;
        let run_bcr = self.cohort_run_bcr;
        let run_iso = self.cohort_run_bcr_isotype;
        let run_tcra = self.cohort_run_tcr_alpha;
        let run_tcrb = self.cohort_run_tcr_beta;
        let run_immune = self.cohort_run_immune;
        let gmt_path = self.corrected_gmt_path.clone();
        let pathway_q = self.corrected_pathway_q;
        let pathway_effect = self.corrected_pathway_effect;
        let stage_count = [
            run_gene,
            run_transcript,
            run_bcr,
            run_iso,
            run_tcra,
            run_tcrb,
            run_immune,
            run_pathway,
        ]
        .into_iter()
        .filter(|x| *x)
        .count()
        .max(1) as f32;
        let (tx, rx) = mpsc::channel();
        self.cohort_receiver = Some(rx);
        self.cohort_running = true;
        self.cohort_progress = 0.0;
        self.cohort_status = format!("Running corrected {}…", comparison.name);
        self.corrected_results = None;
        std::thread::spawn(move || {
            let mut stage = 0usize;
            let weight = 1.0 / stage_count;
            let (
                mut gene,
                mut gene_error,
                mut transcript,
                mut transcript_error,
                mut bcr,
                mut bcr_error,
                mut iso,
                mut iso_error,
                mut tcra,
                mut tcra_error,
                mut tcrb,
                mut tcrb_error,
                mut immune,
                mut immune_error,
                mut pathway,
                mut pathway_error,
            ) = (
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None,
            );
            macro_rules! prog {
                ($base:expr,$prefix:expr) => {
                    |done, total, msg| {
                        let frac = if total == 0 {
                            0.0
                        } else {
                            done as f32 / total as f32
                        };
                        let _ = tx.send(CohortWorkerMessage::Progress(
                            $base + weight * frac,
                            format!("{} — {}", $prefix, msg),
                        ));
                    }
                };
            }
            if run_gene {
                let base = stage as f32 * weight;
                if let Some(path) = scan.merged_gene_tsv.clone() {
                    match run_corrected_expression(
                        &path,
                        &metadata,
                        &comparison,
                        "Raw Count",
                        "Gene",
                        &spec,
                        prog!(base, "Gene corrected"),
                    ) {
                        Ok(r) => gene = Some(r),
                        Err(e) => gene_error = Some(format!("{e:#}")),
                    }
                } else {
                    gene_error = Some("No merged gene-expression TSV detected.".to_string());
                }
                stage += 1;
            }
            if run_transcript {
                let base = stage as f32 * weight;
                if let Some(path) = scan.merged_transcript_tsv.clone() {
                    match run_corrected_expression(
                        &path,
                        &metadata,
                        &comparison,
                        "Total Reads",
                        "Transcript",
                        &spec,
                        prog!(base, "Transcript corrected"),
                    ) {
                        Ok(r) => transcript = Some(r),
                        Err(e) => transcript_error = Some(format!("{e:#}")),
                    }
                } else {
                    transcript_error =
                        Some("No merged transcript-expression TSV detected.".to_string());
                }
                stage += 1;
            }
            if run_bcr {
                let base = stage as f32 * weight;
                match run_corrected_repertoire(
                    &scan.files,
                    &metadata,
                    &comparison,
                    CohortModality::Bcr,
                    &spec,
                    prog!(base, "BCR corrected"),
                ) {
                    Ok(r) => bcr = Some(r),
                    Err(e) => bcr_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_iso {
                let base = stage as f32 * weight;
                match run_corrected_isotype(
                    &scan.files,
                    &metadata,
                    &comparison,
                    &spec,
                    prog!(base, "BCR isotype corrected"),
                ) {
                    Ok(r) => iso = Some(r),
                    Err(e) => iso_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_tcra {
                let base = stage as f32 * weight;
                match run_corrected_repertoire(
                    &scan.files,
                    &metadata,
                    &comparison,
                    CohortModality::TcrAlpha,
                    &spec,
                    prog!(base, "TCR alpha corrected"),
                ) {
                    Ok(r) => tcra = Some(r),
                    Err(e) => tcra_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_tcrb {
                let base = stage as f32 * weight;
                match run_corrected_repertoire(
                    &scan.files,
                    &metadata,
                    &comparison,
                    CohortModality::TcrBeta,
                    &spec,
                    prog!(base, "TCR beta corrected"),
                ) {
                    Ok(r) => tcrb = Some(r),
                    Err(e) => tcrb_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_immune {
                let base = stage as f32 * weight;
                match run_corrected_immune(
                    &scan.files,
                    &metadata,
                    &comparison,
                    &spec,
                    prog!(base, "Immune corrected"),
                ) {
                    Ok(r) => immune = Some(r),
                    Err(e) => immune_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_pathway {
                let _ = tx.send(CohortWorkerMessage::Progress(
                    stage as f32 * weight,
                    "Pathway ORA".to_string(),
                ));
                match (gene.as_ref(), gmt_path.as_ref()) {
                    (Some(g), Some(path)) => {
                        match run_pathway_ora(path, g, pathway_q, pathway_effect) {
                            Ok(r) => pathway = Some(r),
                            Err(e) => pathway_error = Some(format!("{e:#}")),
                        }
                    }
                    (None, _) => {
                        pathway_error = Some(
                            "Pathway ORA requires a successful corrected gene-count analysis."
                                .to_string(),
                        )
                    }
                    (_, None) => {
                        pathway_error = Some(
                            "Choose a GMT gene-set file before running pathway ORA.".to_string(),
                        )
                    }
                }
            }
            let _ = tx.send(CohortWorkerMessage::CorrectedDone(
                CorrectedAnalysisBundle {
                    comparison,
                    spec,
                    gene,
                    gene_error,
                    transcript,
                    transcript_error,
                    bcr,
                    bcr_error,
                    bcr_isotype: iso,
                    bcr_isotype_error: iso_error,
                    tcr_alpha: tcra,
                    tcr_alpha_error: tcra_error,
                    tcr_beta: tcrb,
                    tcr_beta_error: tcrb_error,
                    immune,
                    immune_error,
                    pathway,
                    pathway_error,
                },
            ));
        });
    }

    fn load_cohort_project(&mut self, root: PathBuf) {
        match self
            .analysis_backend
            .scan_project(&ProjectLocator::local(root.clone()))
        {
            Ok(scan) => {
                self.cohort_project_source = Some(ProjectLocator::local(root.clone()));
                self.cohort_root = Some(root);
                self.cohort_status = format!(
                    "Project scanned: {} supported files across {} parsed sample IDs.",
                    scan.files.len(),
                    scan.unique_samples()
                );
                let auto_meta = scan.metadata_candidates.first().cloned();
                self.cohort_scan = Some(scan);
                self.cohort_results = None;
                self.corrected_results = None;
                self.focused_results = None;
                self.diagnostics_results = None;
                self.unified_results = None;
                self.outlier_results = None;
                self.corrected_outlier_snapshot = None;
                self.focused_outlier_snapshot = None;
                self.outlier_excluded_ids.clear();
                if let Some(path) = auto_meta {
                    self.load_cohort_metadata_path(path);
                } else {
                    self.cohort_metadata = None;
                    self.cohort_metadata_path = None;
                    self.cohort_metadata_source = None;
                    self.cohort_mapping = None;
                }
                self.rebuild_focused_mapping();
            }
            Err(e) => {
                self.cohort_status = format!("Project scan failed: {e:#}");
            }
        }
    }

    fn load_cohort_metadata_path(&mut self, path: PathBuf) {
        match self
            .analysis_backend
            .load_metadata(&MetadataLocator::local(path.clone()))
        {
            Ok(meta) => {
                let rows = meta.rows.len();
                let id_column = meta.id_column.clone();
                self.cohort_metadata_source = Some(MetadataLocator::local(path.clone()));
                self.cohort_metadata_path = Some(path);
                self.cohort_metadata = Some(meta);
                self.cohort_results = None;
                self.corrected_results = None;
                self.diagnostics_results = None;
                self.unified_results = None;
                self.outlier_results = None;
                self.corrected_outlier_snapshot = None;
                self.focused_outlier_snapshot = None;
                self.outlier_excluded_ids.clear();
                self.configure_corrected_metadata_defaults();
                self.rebuild_cohort_mapping();
                self.cohort_status =
                    format!("Metadata loaded: {rows} rows; matching on '{id_column}'.");
            }
            Err(e) => {
                self.cohort_status = format!("Metadata load failed: {e:#}");
            }
        }
    }

    fn rebuild_cohort_mapping(&mut self) {
        self.cohort_mapping = match (&self.cohort_scan, &self.cohort_metadata) {
            (Some(scan), Some(meta)) => match self.analysis_backend.build_mapping(scan, meta) {
                Ok(mapping) => Some(mapping),
                Err(error) => {
                    self.cohort_status = format!("Sample mapping failed: {error:#}");
                    None
                }
            },
            _ => None,
        };
    }

    fn load_focused_metadata_path(&mut self, path: PathBuf) {
        match self
            .analysis_backend
            .load_metadata(&MetadataLocator::local(path.clone()))
        {
            Ok(meta) => {
                let rows = meta.rows.len();
                let id_column = meta.id_column.clone();
                self.focused_metadata_source = Some(MetadataLocator::local(path.clone()));
                self.focused_metadata_path = Some(path);
                self.focused_metadata = Some(meta);
                self.focused_results = None;
                self.focused_outlier_snapshot = None;
                self.unified_results = None;
                self.outlier_results = None;
                self.outlier_excluded_ids.clear();
                self.rebuild_focused_mapping();
                self.cohort_status = format!(
                    "Filtered cohort metadata loaded: {rows} rows; matching on '{id_column}'."
                );
            }
            Err(e) => self.cohort_status = format!("Filtered cohort metadata load failed: {e:#}"),
        }
    }

    fn rebuild_focused_mapping(&mut self) {
        self.focused_mapping = match (&self.cohort_scan, &self.focused_metadata) {
            (Some(scan), Some(meta)) => match self.analysis_backend.build_mapping(scan, meta) {
                Ok(mapping) => Some(mapping),
                Err(error) => {
                    self.cohort_status = format!("Filtered cohort mapping failed: {error:#}");
                    None
                }
            },
            _ => None,
        };
    }

    fn start_focused_analysis(&mut self) {
        let (Some(scan), Some(metadata), Some(metadata_path)) = (
            self.cohort_scan.clone(),
            self.focused_metadata.clone(),
            self.focused_metadata_path.clone(),
        ) else {
            self.cohort_status = "Load a project and a focused metadata file first.".to_string();
            return;
        };
        let original_comparison = match build_comparison(&metadata, self.cohort_preset) {
            Ok(c) => c,
            Err(e) => {
                self.cohort_status = format!("Could not build focused comparison: {e:#}");
                return;
            }
        };
        let mut comparison = original_comparison.clone();
        self.focused_outlier_snapshot = None;
        if self.outlier_apply_to_focused && !self.outlier_excluded_ids.is_empty() {
            let Some(audit) = self.outlier_results.as_ref() else {
                self.cohort_status="Outlier exclusions are enabled but the QC audit is no longer available. Rerun Sample outlier QC before the focused analysis so the exclusions are auditable.".to_string();
                return;
            };
            if audit.comparison_name != original_comparison.name {
                self.cohort_status = format!(
                    "The current outlier QC was generated for '{}', but the focused comparison is '{}'. Rerun Sample outlier QC for the current comparison before applying exclusions.",
                    audit.comparison_name, original_comparison.name
                );
                return;
            }
            comparison =
                filter_comparison_excluding(&original_comparison, &self.outlier_excluded_ids);
            if comparison.case_ids.len() < 2 || comparison.control_ids.len() < 2 {
                self.cohort_status = "Outlier filtering leaves fewer than two samples in one focused comparison group; review the exclusion list.".to_string();
                return;
            }
            self.focused_outlier_snapshot = Some(build_outlier_filter_snapshot(
                audit,
                &self.outlier_excluded_ids,
                &original_comparison,
                &comparison,
                true,
            ));
        }
        let spec = CorrectedModelSpec {
            subject_column: None,
            covariates: Vec::new(),
            permutations: self.corrected_permutations,
            rarefaction_reps: self.corrected_rarefaction_reps,
            rarefaction_cap: self.corrected_rarefaction_cap,
            ambiguous_policy: self.corrected_ambiguous_policy,
            exclude_undetermined_isotype: self.corrected_exclude_undetermined,
            ..CorrectedModelSpec::default()
        };
        let run_pathway = self.corrected_run_pathway;
        let run_gene = self.cohort_run_gene || run_pathway;
        let run_transcript = self.cohort_run_transcript;
        let run_bcr = self.cohort_run_bcr;
        let run_iso = self.cohort_run_bcr_isotype;
        let run_tcra = self.cohort_run_tcr_alpha;
        let run_tcrb = self.cohort_run_tcr_beta;
        let run_immune = self.cohort_run_immune;
        let run_concordance = self.focused_run_exploratory_concordance;
        let gmt_path = self.corrected_gmt_path.clone();
        let pathway_q = self.corrected_pathway_q;
        let pathway_effect = self.corrected_pathway_effect;
        let settings = self.focused_settings.clone();
        let stage_count = [
            run_gene,
            run_transcript,
            run_bcr,
            run_iso,
            run_tcra,
            run_tcrb,
            run_immune,
            run_pathway,
            run_concordance && run_gene,
            run_concordance && run_transcript,
        ]
        .into_iter()
        .filter(|x| *x)
        .count()
        .max(1) as f32;
        let (tx, rx) = mpsc::channel();
        self.cohort_receiver = Some(rx);
        self.cohort_running = true;
        self.cohort_progress = 0.0;
        self.cohort_status = format!("Running focused {}…", comparison.name);
        self.focused_results = None;
        std::thread::spawn(move || {
            let mut stage = 0usize;
            let weight = 1.0 / stage_count;
            let (
                mut gene,
                mut gene_error,
                mut transcript,
                mut transcript_error,
                mut bcr,
                mut bcr_error,
                mut iso,
                mut iso_error,
                mut tcra,
                mut tcra_error,
                mut tcrb,
                mut tcrb_error,
                mut immune,
                mut immune_error,
                mut pathway,
                mut pathway_error,
            ) = (
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None,
            );
            let (mut exploratory_gene, mut exploratory_transcript) = (None, None);
            macro_rules! prog {
                ($base:expr,$prefix:expr) => {
                    |done, total, msg| {
                        let frac = if total == 0 {
                            0.0
                        } else {
                            done as f32 / total as f32
                        };
                        let _ = tx.send(CohortWorkerMessage::Progress(
                            $base + weight * frac,
                            format!("{} — {}", $prefix, msg),
                        ));
                    }
                };
            }
            if run_gene {
                let base = stage as f32 * weight;
                if let Some(path) = scan.merged_gene_tsv.clone() {
                    match run_corrected_expression(
                        &path,
                        &metadata,
                        &comparison,
                        "Raw Count",
                        "Gene",
                        &spec,
                        prog!(base, "Focused gene counts"),
                    ) {
                        Ok(r) => gene = Some(r),
                        Err(e) => gene_error = Some(format!("{e:#}")),
                    }
                } else {
                    gene_error = Some("No merged gene-expression TSV detected.".to_string());
                }
                stage += 1;
            }
            if run_transcript {
                let base = stage as f32 * weight;
                if let Some(path) = scan.merged_transcript_tsv.clone() {
                    match run_corrected_expression(
                        &path,
                        &metadata,
                        &comparison,
                        "Total Reads",
                        "Transcript",
                        &spec,
                        prog!(base, "Focused transcript counts"),
                    ) {
                        Ok(r) => transcript = Some(r),
                        Err(e) => transcript_error = Some(format!("{e:#}")),
                    }
                } else {
                    transcript_error =
                        Some("No merged transcript-expression TSV detected.".to_string());
                }
                stage += 1;
            }
            if run_bcr {
                let base = stage as f32 * weight;
                match run_corrected_repertoire(
                    &scan.files,
                    &metadata,
                    &comparison,
                    CohortModality::Bcr,
                    &spec,
                    prog!(base, "Focused BCR"),
                ) {
                    Ok(r) => bcr = Some(r),
                    Err(e) => bcr_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_iso {
                let base = stage as f32 * weight;
                match run_corrected_isotype(
                    &scan.files,
                    &metadata,
                    &comparison,
                    &spec,
                    prog!(base, "Focused BCR isotype"),
                ) {
                    Ok(r) => iso = Some(r),
                    Err(e) => iso_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_tcra {
                let base = stage as f32 * weight;
                match run_corrected_repertoire(
                    &scan.files,
                    &metadata,
                    &comparison,
                    CohortModality::TcrAlpha,
                    &spec,
                    prog!(base, "Focused TCR alpha"),
                ) {
                    Ok(r) => tcra = Some(r),
                    Err(e) => tcra_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_tcrb {
                let base = stage as f32 * weight;
                match run_corrected_repertoire(
                    &scan.files,
                    &metadata,
                    &comparison,
                    CohortModality::TcrBeta,
                    &spec,
                    prog!(base, "Focused TCR beta"),
                ) {
                    Ok(r) => tcrb = Some(r),
                    Err(e) => tcrb_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_immune {
                let base = stage as f32 * weight;
                match run_corrected_immune(
                    &scan.files,
                    &metadata,
                    &comparison,
                    &spec,
                    prog!(base, "Focused immune"),
                ) {
                    Ok(r) => immune = Some(r),
                    Err(e) => immune_error = Some(format!("{e:#}")),
                }
                stage += 1;
            }
            if run_pathway {
                let _ = tx.send(CohortWorkerMessage::Progress(
                    stage as f32 * weight,
                    "Focused pathway ORA".to_string(),
                ));
                match (gene.as_ref(), gmt_path.as_ref()) {
                    (Some(g), Some(path)) => {
                        match run_pathway_ora(path, g, pathway_q, pathway_effect) {
                            Ok(r) => pathway = Some(r),
                            Err(e) => pathway_error = Some(format!("{e:#}")),
                        }
                    }
                    (None, _) => {
                        pathway_error = Some(
                            "Focused pathway ORA requires a successful gene-count analysis."
                                .to_string(),
                        )
                    }
                    (_, None) => {
                        pathway_error = Some(
                            "Choose a GMT gene-set file before running focused pathway ORA."
                                .to_string(),
                        )
                    }
                }
                stage += 1;
            }
            if run_concordance && run_gene {
                let base = stage as f32 * weight;
                if let Some(path) = scan.merged_gene_tsv.clone() {
                    let _ = tx.send(CohortWorkerMessage::Progress(
                        base,
                        "Focused gene TPM direction check".to_string(),
                    ));
                    if let Ok(r) = run_gene_comparison(&path, &comparison, "TPM", |_, _, _| {}) {
                        exploratory_gene = Some(r);
                    }
                }
                stage += 1;
            }
            if run_concordance && run_transcript {
                let base = stage as f32 * weight;
                if let Some(path) = scan.merged_transcript_tsv.clone() {
                    let _ = tx.send(CohortWorkerMessage::Progress(
                        base,
                        "Focused transcript TPM direction check".to_string(),
                    ));
                    if let Ok(r) =
                        run_transcript_comparison(&path, &comparison, "TPM", |_, _, _| {})
                    {
                        exploratory_transcript = Some(r);
                    }
                }
            }
            let corrected = CorrectedAnalysisBundle {
                comparison: comparison.clone(),
                spec,
                gene,
                gene_error,
                transcript,
                transcript_error,
                bcr,
                bcr_error,
                bcr_isotype: iso,
                bcr_isotype_error: iso_error,
                tcr_alpha: tcra,
                tcr_alpha_error: tcra_error,
                tcr_beta: tcrb,
                tcr_beta_error: tcrb_error,
                immune,
                immune_error,
                pathway,
                pathway_error,
            };
            let candidates = build_focused_candidate_summary(
                &metadata_path,
                &comparison.case_label,
                &comparison.control_label,
                comparison.case_metadata_rows,
                comparison.control_metadata_rows,
                settings,
                corrected.gene.as_ref(),
                exploratory_gene.as_ref(),
                corrected.transcript.as_ref(),
                exploratory_transcript.as_ref(),
                corrected.bcr.as_ref(),
                corrected.bcr_isotype.as_ref(),
                corrected.tcr_alpha.as_ref(),
                corrected.tcr_beta.as_ref(),
                corrected.immune.as_ref(),
                corrected.pathway.as_ref(),
            );
            let _ = tx.send(CohortWorkerMessage::FocusedDone(FocusedAnalysisBundle {
                corrected,
                candidates,
            }));
        });
    }

    fn start_cohort_analysis(&mut self) {
        let (Some(scan), Some(metadata)) = (self.cohort_scan.clone(), self.cohort_metadata.clone())
        else {
            self.cohort_status = "Load a project and metadata first.".to_string();
            return;
        };
        let comparison = match build_comparison(&metadata, self.cohort_preset) {
            Ok(comp) => comp,
            Err(e) => {
                self.cohort_status = format!("Could not build comparison: {e:#}");
                return;
            }
        };
        let run_gene = self.cohort_run_gene;
        let gene_metric = self.cohort_gene_metric.display_name().to_string();
        let run_transcript = self.cohort_run_transcript;
        let transcript_metric = self.cohort_transcript_metric.display_name().to_string();
        let run_bcr = self.cohort_run_bcr;
        let run_iso = self.cohort_run_bcr_isotype;
        let run_tcr_alpha = self.cohort_run_tcr_alpha;
        let run_tcr_beta = self.cohort_run_tcr_beta;
        let run_immune = self.cohort_run_immune;
        let stage_count = [
            run_gene,
            run_transcript,
            run_bcr,
            run_iso,
            run_tcr_alpha,
            run_tcr_beta,
            run_immune,
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        .max(1) as f32;

        let (tx, rx) = mpsc::channel();
        self.cohort_receiver = Some(rx);
        self.cohort_running = true;
        self.cohort_progress = 0.0;
        self.cohort_status = format!("Running {}…", comparison.name);
        self.cohort_results = None;

        std::thread::spawn(move || {
            let mut stage_index = 0usize;
            let stage_weight = 1.0 / stage_count;
            let mut gene = None;
            let mut gene_error = None;
            let mut transcript = None;
            let mut transcript_error = None;
            let mut bcr = None;
            let mut bcr_error = None;
            let mut bcr_isotype = None;
            let mut bcr_isotype_error = None;
            let mut tcr_alpha = None;
            let mut tcr_alpha_error = None;
            let mut tcr_beta = None;
            let mut tcr_beta_error = None;
            let mut immune = None;
            let mut immune_error = None;

            if run_gene {
                let base = stage_index as f32 * stage_weight;
                if let Some(path) = scan.merged_gene_tsv.clone() {
                    match run_gene_comparison(
                        &path,
                        &comparison,
                        &gene_metric,
                        |done, total, msg| {
                            let frac = if total == 0 {
                                0.0
                            } else {
                                done as f32 / total as f32
                            };
                            let _ = tx.send(CohortWorkerMessage::Progress(
                                base + stage_weight * frac,
                                format!("Gene expression — {msg}"),
                            ));
                        },
                    ) {
                        Ok(result) => gene = Some(result),
                        Err(e) => gene_error = Some(format!("{e:#}")),
                    }
                } else {
                    gene_error = Some(
                        "No merged gene-expression TSV was detected in the project. Merge gene expression first or place the merged TSV under the project root.".to_string(),
                    );
                }
                stage_index += 1;
            }

            if run_transcript {
                let base = stage_index as f32 * stage_weight;
                if let Some(path) = scan.merged_transcript_tsv.clone() {
                    match run_transcript_comparison(
                        &path,
                        &comparison,
                        &transcript_metric,
                        |done, total, msg| {
                            let frac = if total == 0 {
                                0.0
                            } else {
                                done as f32 / total as f32
                            };
                            let _ = tx.send(CohortWorkerMessage::Progress(
                                base + stage_weight * frac,
                                format!("Transcript expression — {msg}"),
                            ));
                        },
                    ) {
                        Ok(result) => transcript = Some(result),
                        Err(e) => transcript_error = Some(format!("{e:#}")),
                    }
                } else {
                    transcript_error = Some(
                        "No merged transcript-expression TSV was detected in the project. Merge transcript expression first or place the merged TSV under the project root.".to_string(),
                    );
                }
                stage_index += 1;
            }

            if run_bcr {
                let base = stage_index as f32 * stage_weight;
                match run_bcr_comparison(&scan.files, &comparison, |done, total, msg| {
                    let frac = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    let _ = tx.send(CohortWorkerMessage::Progress(
                        base + stage_weight * frac,
                        msg.to_string(),
                    ));
                }) {
                    Ok(result) => bcr = Some(result),
                    Err(e) => bcr_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_iso {
                let base = stage_index as f32 * stage_weight;
                match run_bcr_isotype_comparison(&scan.files, &comparison, |done, total, msg| {
                    let frac = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    let _ = tx.send(CohortWorkerMessage::Progress(
                        base + stage_weight * frac,
                        msg.to_string(),
                    ));
                }) {
                    Ok(result) => bcr_isotype = Some(result),
                    Err(e) => bcr_isotype_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_tcr_alpha {
                let base = stage_index as f32 * stage_weight;
                match run_tcr_comparison(
                    &scan.files,
                    &comparison,
                    CohortModality::TcrAlpha,
                    |done, total, msg| {
                        let frac = if total == 0 {
                            0.0
                        } else {
                            done as f32 / total as f32
                        };
                        let _ = tx.send(CohortWorkerMessage::Progress(
                            base + stage_weight * frac,
                            msg.to_string(),
                        ));
                    },
                ) {
                    Ok(result) => tcr_alpha = Some(result),
                    Err(e) => tcr_alpha_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_tcr_beta {
                let base = stage_index as f32 * stage_weight;
                match run_tcr_comparison(
                    &scan.files,
                    &comparison,
                    CohortModality::TcrBeta,
                    |done, total, msg| {
                        let frac = if total == 0 {
                            0.0
                        } else {
                            done as f32 / total as f32
                        };
                        let _ = tx.send(CohortWorkerMessage::Progress(
                            base + stage_weight * frac,
                            msg.to_string(),
                        ));
                    },
                ) {
                    Ok(result) => tcr_beta = Some(result),
                    Err(e) => tcr_beta_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_immune {
                let base = stage_index as f32 * stage_weight;
                match run_immune_infiltrate_comparison(
                    &scan.files,
                    &comparison,
                    |done, total, msg| {
                        let frac = if total == 0 {
                            0.0
                        } else {
                            done as f32 / total as f32
                        };
                        let _ = tx.send(CohortWorkerMessage::Progress(
                            base + stage_weight * frac,
                            msg.to_string(),
                        ));
                    },
                ) {
                    Ok(result) => immune = Some(result),
                    Err(e) => immune_error = Some(format!("{e:#}")),
                }
            }

            let _ = tx.send(CohortWorkerMessage::Done(CohortAnalysisBundle {
                comparison,
                gene,
                gene_error,
                transcript,
                transcript_error,
                bcr,
                bcr_error,
                bcr_isotype,
                bcr_isotype_error,
                tcr_alpha,
                tcr_alpha_error,
                tcr_beta,
                tcr_beta_error,
                immune,
                immune_error,
            }));
        });
    }

    fn poll_cohort_worker(&mut self) {
        let mut exploratory_done = None;
        let mut corrected_done = None;
        let mut focused_done = None;
        let mut diagnostics_done = None;
        let mut unified_done = None;
        let mut outlier_done = None;
        if let Some(rx) = &self.cohort_receiver {
            while let Ok(message) = rx.try_recv() {
                match message {
                    CohortWorkerMessage::Progress(frac, text) => {
                        self.cohort_progress = frac.clamp(0.0, 1.0);
                        self.cohort_status = text;
                    }
                    CohortWorkerMessage::Done(bundle) => exploratory_done = Some(bundle),
                    CohortWorkerMessage::CorrectedDone(bundle) => corrected_done = Some(bundle),
                    CohortWorkerMessage::FocusedDone(bundle) => focused_done = Some(bundle),
                    CohortWorkerMessage::DiagnosticsDone(result) => diagnostics_done = Some(result),
                    CohortWorkerMessage::UnifiedDone(result) => unified_done = Some(result),
                    CohortWorkerMessage::OutlierDone(result) => outlier_done = Some(result),
                }
            }
        }
        if let Some(bundle) = exploratory_done {
            let mut messages = Vec::new();
            if let Some(err) = &bundle.gene_error {
                messages.push(format!("Gene: {err}"));
            }
            if let Some(err) = &bundle.transcript_error {
                messages.push(format!("Transcript: {err}"));
            }
            if let Some(err) = &bundle.bcr_error {
                messages.push(format!("BCR: {err}"));
            }
            if let Some(err) = &bundle.bcr_isotype_error {
                messages.push(format!("BCR isotype: {err}"));
            }
            if let Some(err) = &bundle.tcr_alpha_error {
                messages.push(format!("TCR alpha: {err}"));
            }
            if let Some(err) = &bundle.tcr_beta_error {
                messages.push(format!("TCR beta: {err}"));
            }
            if let Some(err) = &bundle.immune_error {
                messages.push(format!("Immune infiltrate: {err}"));
            }
            let current_available = match self.cohort_result_view {
                CohortResultView::Gene => bundle.gene.is_some(),
                CohortResultView::Transcript => bundle.transcript.is_some(),
                CohortResultView::Bcr => bundle.bcr.is_some(),
                CohortResultView::BcrIsotype => bundle.bcr_isotype.is_some(),
                CohortResultView::TcrAlpha => bundle.tcr_alpha.is_some(),
                CohortResultView::TcrBeta => bundle.tcr_beta.is_some(),
                CohortResultView::Immune => bundle.immune.is_some(),
            };
            if !current_available {
                self.cohort_result_view = if bundle.gene.is_some() {
                    CohortResultView::Gene
                } else if bundle.transcript.is_some() {
                    CohortResultView::Transcript
                } else if bundle.bcr.is_some() {
                    CohortResultView::Bcr
                } else if bundle.bcr_isotype.is_some() {
                    CohortResultView::BcrIsotype
                } else if bundle.tcr_alpha.is_some() {
                    CohortResultView::TcrAlpha
                } else if bundle.tcr_beta.is_some() {
                    CohortResultView::TcrBeta
                } else {
                    CohortResultView::Immune
                };
            }
            self.cohort_results = Some(bundle);
            self.cohort_running = false;
            self.cohort_progress = 1.0;
            self.cohort_receiver = None;
            self.cohort_status = if messages.is_empty() {
                "Exploratory comparison complete.".to_string()
            } else {
                format!(
                    "Comparison completed with {} modality warning(s): {}",
                    messages.len(),
                    messages.join(" | ")
                )
            };
        }
        if let Some(bundle) = corrected_done {
            let mut messages = Vec::new();
            if let Some(err) = &bundle.gene_error {
                messages.push(format!("Gene: {err}"));
            }
            if let Some(err) = &bundle.transcript_error {
                messages.push(format!("Transcript: {err}"));
            }
            if let Some(err) = &bundle.bcr_error {
                messages.push(format!("BCR: {err}"));
            }
            if let Some(err) = &bundle.bcr_isotype_error {
                messages.push(format!("BCR isotype: {err}"));
            }
            if let Some(err) = &bundle.tcr_alpha_error {
                messages.push(format!("TCR alpha: {err}"));
            }
            if let Some(err) = &bundle.tcr_beta_error {
                messages.push(format!("TCR beta: {err}"));
            }
            if let Some(err) = &bundle.immune_error {
                messages.push(format!("Immune: {err}"));
            }
            if let Some(err) = &bundle.pathway_error {
                messages.push(format!("Pathway: {err}"));
            }
            let current = match self.corrected_result_view {
                CorrectedResultView::Gene => bundle.gene.is_some(),
                CorrectedResultView::Transcript => bundle.transcript.is_some(),
                CorrectedResultView::Bcr => bundle.bcr.is_some(),
                CorrectedResultView::BcrIsotype => bundle.bcr_isotype.is_some(),
                CorrectedResultView::TcrAlpha => bundle.tcr_alpha.is_some(),
                CorrectedResultView::TcrBeta => bundle.tcr_beta.is_some(),
                CorrectedResultView::Immune => bundle.immune.is_some(),
                CorrectedResultView::Pathway => bundle.pathway.is_some(),
            };
            if !current {
                self.corrected_result_view = if bundle.gene.is_some() {
                    CorrectedResultView::Gene
                } else if bundle.transcript.is_some() {
                    CorrectedResultView::Transcript
                } else if bundle.bcr.is_some() {
                    CorrectedResultView::Bcr
                } else if bundle.bcr_isotype.is_some() {
                    CorrectedResultView::BcrIsotype
                } else if bundle.tcr_alpha.is_some() {
                    CorrectedResultView::TcrAlpha
                } else if bundle.tcr_beta.is_some() {
                    CorrectedResultView::TcrBeta
                } else if bundle.immune.is_some() {
                    CorrectedResultView::Immune
                } else {
                    CorrectedResultView::Pathway
                };
            }
            self.corrected_results = Some(bundle);
            self.cohort_running = false;
            self.cohort_progress = 1.0;
            self.cohort_receiver = None;
            self.cohort_status = if messages.is_empty() {
                "Corrected / standard analysis complete.".to_string()
            } else {
                format!(
                    "Corrected analysis completed with {} modality warning(s): {}",
                    messages.len(),
                    messages.join(" | ")
                )
            };
        }
        if let Some(bundle) = focused_done {
            let gene_count = bundle.candidates.genes.len();
            let pathway_count = bundle.candidates.pathways.len();
            self.corrected_results = Some(bundle.corrected);
            self.corrected_outlier_snapshot = self.focused_outlier_snapshot.clone();
            self.focused_results = Some(bundle.candidates);
            self.focused_result_view = FocusedResultView::Overview;
            self.cohort_running = false;
            self.cohort_progress = 1.0;
            self.cohort_receiver = None;
            self.cohort_status = format!(
                "Focused candidate workflow complete: {gene_count} gene candidate(s), {pathway_count} pathway candidate(s)."
            );
        }
        if let Some(result) = diagnostics_done {
            let flagged = result
                .sample_qc
                .iter()
                .filter(|r| !r.flags.is_empty())
                .count();
            let candidates = result.candidate_stability.len();
            self.diagnostics_results = Some(result);
            self.diagnostics_result_view = DiagnosticsResultView::Overview;
            self.diagnostics_selected_gene = None;
            self.cohort_running = false;
            self.cohort_progress = 1.0;
            self.cohort_receiver = None;
            self.cohort_status = format!(
                "PBMC/confounding diagnostics complete: {flagged} sample(s) flagged for review; {candidates} candidate gene robustness profile(s)."
            );
        }
        if let Some(result) = unified_done {
            self.cohort_running = false;
            self.cohort_progress = 1.0;
            self.cohort_receiver = None;
            match result {
                Ok(run) => {
                    let gene_count = run.candidates.genes.len();
                    let pathway_count = run.candidates.pathways.len();
                    let warning_count = run.warnings.len();
                    self.corrected_results = Some(run.primary.clone());
                    self.corrected_outlier_snapshot = run.qc_snapshot.clone();
                    self.focused_results = Some(run.candidates.clone());
                    self.focused_outlier_snapshot = run.qc_snapshot.clone();
                    self.diagnostics_results = run.diagnostics.clone();
                    self.diagnostics_selected_gene = None;
                    self.unified_result_view = UnifiedResultView::Overview;
                    self.unified_results = Some(run);
                    self.cohort_status = if warning_count == 0 {
                        format!(
                            "Analysis complete: {gene_count} candidate gene(s), {pathway_count} candidate pathway(s)."
                        )
                    } else {
                        format!(
                            "Analysis complete with {warning_count} warning(s): {gene_count} candidate gene(s), {pathway_count} candidate pathway(s)."
                        )
                    };
                }
                Err(error) => {
                    self.cohort_status = format!("Unified analysis failed: {error}");
                }
            }
        }
        if let Some(result) = outlier_done {
            self.cohort_running = false;
            self.cohort_progress = 1.0;
            self.cohort_receiver = None;
            match result {
                Ok(audit) => {
                    let n = audit.consensus_ids.len();
                    self.outlier_excluded_ids.clear();
                    self.outlier_plot_modality = 0;
                    self.outlier_results = Some(audit);
                    self.cohort_status = format!(
                        "Sample outlier QC complete: {n} sample(s) meet the selected cross-modal rule. They are flags only; no sample was selected for exclusion automatically."
                    );
                }
                Err(error) => {
                    self.cohort_status = format!("Sample outlier QC failed: {error}");
                }
            }
        }
    }

    fn render_analysis_docs_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Analysis guide", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "This guide explains, in plain language, what each analysis layer does, when to use it, how to interpret the results, and where to read more. It is meant for collaborators and readers who want the main ideas without needing to read the code or methods section first.",
                )
                .weak(),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Search docs").strong());
                let response = ui.add_sized(
                    [ui.available_width().min(520.0), 28.0],
                    egui::TextEdit::singleline(&mut this.docs_search)
                        .hint_text("Try: clonality, q value, PCA, outlier, pathway, covariate…"),
                );
                if !this.docs_search.is_empty() && ui.button("Clear").clicked() {
                    this.docs_search.clear();
                    response.request_focus();
                }
            });
            let query = this.docs_search.trim();
            if query.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "Search is case-insensitive and scans section titles, definitions, examples, chart labels, and interpretation notes.",
                    )
                    .small()
                    .weak(),
                );
            } else {
                let matches = analysis_doc_sections()
                    .iter()
                    .filter(|section| analysis_doc_section_matches(section, query))
                    .count();
                ui.label(
                    egui::RichText::new(format!(
                        "Showing {matches} of {} documentation section(s) matching ‘{query}’.",
                        analysis_doc_sections().len()
                    ))
                    .small()
                    .weak(),
                );
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Tip:").strong());
                ui.label("Each major topic now includes a small synthetic worked example and visual. Toy values are deliberately simple and are never drawn from, or mixed with, the current cohort results.");
            });
            ui.add_space(10.0);

            if query.is_empty() {
                this.render_docs_workflow_overview(ui);
                ui.add_space(10.0);
            }
            this.render_shared_analysis_docs(ui);
            if query.is_empty() {
                ui.add_space(8.0);
                this.docs_section_references(ui);
            }
        });
    }

    fn render_shared_analysis_docs(&self, ui: &mut egui::Ui) {
        let query = self.docs_search.trim();
        ui.label(
            egui::RichText::new(
                "Detailed method and interpretation reference. The same canonical explanations and toy examples are embedded in both unified HTML report styles so a report reader can look up exactly what a statistic means without returning to the application.",
            )
            .small()
            .weak(),
        );
        ui.add_space(6.0);

        let mut shown = 0usize;
        for (section_idx, section) in analysis_doc_sections().iter().enumerate() {
            if !analysis_doc_section_matches(section, query) {
                continue;
            }
            shown += 1;
            let header_match = analysis_doc_section_header_matches(section, query);
            egui::CollapsingHeader::new(section.title)
                .default_open(!query.is_empty() || section.default_open)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(section.summary).italics());
                    ui.add_space(8.0);

                    for (example_idx, example) in section.examples.iter().enumerate() {
                        if header_match || analysis_doc_example_matches(example, query) {
                            self.render_doc_toy_example(ui, section_idx, example_idx, example);
                            ui.add_space(10.0);
                        }
                    }

                    for subsection in section.subsections {
                        if !header_match && !analysis_doc_subsection_matches(subsection, query) {
                            continue;
                        }
                        ui.label(egui::RichText::new(subsection.title).strong());
                        ui.add_space(3.0);
                        for paragraph in subsection.paragraphs {
                            ui.label(*paragraph);
                            ui.add_space(4.0);
                        }
                        for bullet in subsection.bullets {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("•");
                                ui.label(*bullet);
                            });
                        }
                        ui.add_space(9.0);
                    }
                });
            ui.add_space(4.0);
        }

        if shown == 0 {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(format!(
                    "No documentation matched ‘{}’. Try a broader term such as ‘BCR’, ‘clonality’, ‘PCA’, ‘q value’, ‘outlier’, ‘covariate’, or ‘pathway’.",
                    query
                ))
                .italics(),
            );
        }
    }

    fn render_doc_toy_example(
        &self,
        ui: &mut egui::Ui,
        section_idx: usize,
        example_idx: usize,
        example: &AnalysisDocToyExample,
    ) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new(format!("Toy example — {}", example.title)).strong());
            ui.label(
                egui::RichText::new(
                    "Illustrative values only — this is not a result from your cohort.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);
            ui.label(example.setup);
            ui.add_space(6.0);
            self.render_doc_toy_plot(ui, section_idx, example_idx, example);
            ui.add_space(6.0);
            for (idx, step) in example.walkthrough.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{}.", idx + 1)).strong());
                    ui.label(*step);
                });
            }
            ui.add_space(5.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Takeaway:").strong());
                ui.label(example.takeaway);
            });
        });
    }

    fn render_doc_toy_plot(
        &self,
        ui: &mut egui::Ui,
        section_idx: usize,
        example_idx: usize,
        example: &AnalysisDocToyExample,
    ) {
        let plot_id = format!("analysis_docs_toy_{section_idx}_{example_idx}");
        match example.chart {
            AnalysisDocToyChart::Bars {
                categories,
                series,
                y_label,
                stacked,
            } => {
                let labels = categories
                    .iter()
                    .map(|x| (*x).to_string())
                    .collect::<Vec<_>>();
                Plot::new(plot_id)
                    .height(230.0)
                    .legend(Legend::default())
                    .y_axis_label(y_label)
                    .x_axis_formatter(move |mark, _range| {
                        analysis_doc_tick_label(mark.value, &labels)
                    })
                    .show(ui, |plot_ui| {
                        if stacked {
                            let mut bases = vec![0.0f64; categories.len()];
                            for toy_series in series {
                                let bars = toy_series
                                    .values
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, value)| {
                                        let base = bases[idx];
                                        bases[idx] += *value;
                                        Bar::new(idx as f64, *value).base_offset(base).width(0.62)
                                    })
                                    .collect::<Vec<_>>();
                                plot_ui.bar_chart(BarChart::new(toy_series.label, bars));
                            }
                        } else {
                            let count = series.len().max(1) as f64;
                            let width = (0.72 / count).min(0.42);
                            for (series_idx, toy_series) in series.iter().enumerate() {
                                let center = (series.len().saturating_sub(1)) as f64 / 2.0;
                                let offset = (series_idx as f64 - center) * width;
                                let bars = toy_series
                                    .values
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, value)| {
                                        Bar::new(idx as f64 + offset, *value).width(width * 0.88)
                                    })
                                    .collect::<Vec<_>>();
                                plot_ui.bar_chart(BarChart::new(toy_series.label, bars));
                            }
                        }
                    });
            }
            AnalysisDocToyChart::Line {
                categories,
                series,
                y_label,
            } => {
                let labels = categories
                    .iter()
                    .map(|x| (*x).to_string())
                    .collect::<Vec<_>>();
                Plot::new(plot_id)
                    .height(230.0)
                    .legend(Legend::default())
                    .y_axis_label(y_label)
                    .x_axis_formatter(move |mark, _range| {
                        analysis_doc_tick_label(mark.value, &labels)
                    })
                    .show(ui, |plot_ui| {
                        for toy_series in series {
                            let points = toy_series
                                .values
                                .iter()
                                .enumerate()
                                .map(|(idx, value)| [idx as f64, *value])
                                .collect::<Vec<_>>();
                            plot_ui.line(Line::new(toy_series.label, points.clone()));
                            plot_ui.points(Points::new(toy_series.label, points).radius(4.0));
                        }
                    });
            }
            AnalysisDocToyChart::Scatter {
                points,
                x_label,
                y_label,
            } => {
                let mut grouped = BTreeMap::<&str, Vec<[f64; 2]>>::new();
                for point in points {
                    grouped
                        .entry(point.group)
                        .or_default()
                        .push([point.x, point.y]);
                }
                Plot::new(plot_id)
                    .height(230.0)
                    .legend(Legend::default())
                    .x_axis_label(x_label)
                    .y_axis_label(y_label)
                    .show(ui, |plot_ui| {
                        for (group, coords) in grouped {
                            plot_ui.points(Points::new(group, coords).radius(4.2));
                        }
                    });
                let labels = points
                    .iter()
                    .map(|point| {
                        format!(
                            "{}={} ({})",
                            point.label,
                            point.group,
                            format!("{:.2}, {:.2}", point.x, point.y)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("  •  ");
                ui.label(egui::RichText::new(labels).small().weak());
            }
        }
    }

    fn render_docs_workflow_overview(&self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Workflow schematic").strong());
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "The workflow is shown as a simple vertical sequence so it stays readable on smaller windows and avoids clipped text.",
            )
            .small()
            .weak(),
        );
        ui.add_space(6.0);
        let steps = [
            (
                "1. Cohort & metadata",
                "Load the project and primary metadata, or use Metadata Builder to create/curate the standardized sample table. A collaborator-supplied filtered table can still define an optional subset.",
            ),
            (
                "2. Sample mapping & QC",
                "Audit sample-to-file coverage, run cross-modal outlier flagging, review any automatic flags, and record manual retain/exclude decisions without silently deleting samples.",
            ),
            (
                "3. Analysis settings",
                "Choose a built-in or custom metadata-defined case/control comparison, modalities, repeated-measure handling, covariates, repertoire settings, pathways, and optional PBMC/confounding sensitivity diagnostics.",
            ),
            (
                "4. Run analysis",
                "One action runs the corrected/standard inferential engine and attaches the frozen QC provenance, supporting descriptive checks, diagnostics, candidate summary, and warnings.",
            ),
            (
                "5. Results & export",
                "Inspect modality results and QC/robustness together, then export either a presentation-friendly key-findings report or a full technical report from the exact same completed run.",
            ),
        ];
        for (idx, (title, body)) in steps.iter().enumerate() {
            self.docs_flow_step(ui, title, body);
            if idx + 1 != steps.len() {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(egui::RichText::new("↓").size(18.0));
                    ui.label(egui::RichText::new("Next step").small().weak());
                });
                ui.add_space(4.0);
            }
        }
    }

    fn docs_flow_step(&self, ui: &mut egui::Ui, title: &str, body: &str) {
        egui::Frame::group(ui.style())
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(10)
            .corner_radius(6)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(title).strong());
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(body).small().weak());
                });
            });
    }

    fn docs_section_overview(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("How the unified Cohort Analysis workflow is organized")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Cohort Analysis now has one primary inferential workflow rather than several competing analysis modes.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Metadata contract and custom comparisons").strong());
                ui.label("The Metadata Builder can create a standardized one-row-per-sample TSV with canonical sample/subject IDs, cohort fields, covariates, batch/source provenance, and per-modality aliases. Cohort Analysis can use the historical 4C presets or construct Case and Control from values in any metadata column; the statistical engine is unchanged.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Primary evidence: corrected / standard analysis").strong());
                ui.label("The corrected methods remain the main statistical evidence for gene/transcript expression, BCR/TCR, BCR isotype, immune signatures, and pathways.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Supporting evidence: QC, descriptive checks, and sensitivity diagnostics").strong());
                ui.label("Cross-modal outlier screening, exploratory direction checks, and PBMC/confounding diagnostics help interpret robustness and sample quality. They do not become separate competing sources of evidence.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Presentation layer: key findings").strong());
                ui.label("Candidate genes, pathways, transcripts, and other signals are shortlisted from the completed corrected analysis. A filtered metadata table changes the cohort definition, not the statistical methodology.");
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Important principle").strong());
                ui.label("Both the simplified and full reports are rendered from the same frozen completed run. Neither report style reruns statistics, and neither should describe preliminary candidates as validated biomarkers.");
            });
    }

    fn docs_section_exploratory(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Supporting descriptive checks and cross-modal sample QC")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Plain-language summary: exploratory calculations are now supporting context rather than a separate Cohort Analysis mode.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Cross-modal sample outlier QC").strong());
                ui.label("Each enabled data type can produce an unsupervised sample representation and robust distance score. The audit can flag a sample when it is unusual in any modality, in at least N modalities, in a majority, or in all available selected modalities.");
                ui.label("Within-group detection is the recommended default so a real case/control-wide biological shift is less likely to be mislabeled as an outlier.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Review before exclusion").strong());
                ui.label("Automatic outliers are flags only. A new QC run does not automatically select samples for exclusion. Reviewers may explicitly retain consensus-flagged samples or select other samples for exclusion when provenance/QC evidence supports that decision.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Exploratory direction support").strong());
                ui.label("When enabled, simple TPM-based expression comparisons are used only to annotate whether candidate directions agree with a descriptive screen. They do not change corrected p values, q values, or effect estimates and are not independent validation.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("What a reader should conclude").strong());
                ui.label("Use these views to understand sample structure and robustness. The primary inferential interpretation still comes from the corrected/standard results.");
            });
    }

    fn docs_section_diagnostics(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("QC / robustness sensitivity diagnostics")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Plain-language summary: PBMC/source/composition diagnostics are an optional sensitivity stage attached to the same completed analysis run.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("What it checks").strong());
                ui.label("• Whole-transcriptome PCA and robust sample-outlier structure.");
                ui.label("• Library/detection summaries and immune-signature structure when immune-infiltrate files are available.");
                ui.label("• Known wrong-source/non-PBMC samples and reviewer-supplied suspect samples.");
                ui.label("• Low-count support, top-two-sample contribution, leave-one-sample-out, and optional leave-batch/family-level-out stability for leading genes.");
                ui.label("• QC-restricted and immune-PC-adjusted corrected sensitivity models.");
                ui.label("• Longitudinal within-person PCA consistency when a subject column is available.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("How to interpret it").strong());
                ui.label("A statistical outlier flag alone is not proof that a sample is invalid. Confidence is higher when provenance, global sample structure, immune composition, historical/family structure, and sensitivity analyses point in the same direction.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("What a robust candidate should look like").strong());
                ui.label("A promising disease-associated feature should usually remain directionally/effect-size stable across reasonable QC and composition sensitivity checks and should not depend on one or two samples.");
            });
    }

    fn docs_section_corrected(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Primary corrected / standard analysis")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Plain-language summary: this is the primary inferential engine used by the unified workflow.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Gene and transcript expression").strong());
                ui.label("Positive-count median-ratio normalization and negative-binomial log-link models provide adjusted case/control effects, optional metadata covariates, subject-clustered inference for repeated sampling, multiple-testing correction, subject-collapsed sensitivity analysis, PCA, and dendrograms. Transcript analysis also retains the within-gene CLR/Simes/BH DTU screen.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("BCR and TCR repertoires").strong());
                ui.label("Matched-depth exact-without-replacement rarefaction supports depth-fair diversity summaries, while CLR/Aitchison analyses, Morisita-Horn/Jaccard ordinations, subject-blocked PERMANOVA, and dendrograms address composition and multi-sample structure.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("BCR isotype and immune signatures").strong());
                ui.label("BCR-isotype composition uses CLR/Aitchison methods with the existing Undetermined sensitivity option. Immune signatures retain covariate-adjusted feature models, PCA, and clustering.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Pathways").strong());
                ui.label("Optional GMT-driven over-representation analysis is derived from the corrected gene analysis using the tested-gene universe, odds ratios, overlap genes, and BH FDR. Public MSigDB collections and custom GMT files remain supported.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Important model caveat").strong());
                ui.label("The native expression model does not claim to reproduce the exact empirical-Bayes/dispersion-shrinkage machinery of DESeq2, edgeR, or limma-voom. This application supports discovery and robustness assessment rather than final clinical biomarker validation.");
            });
    }

    fn docs_section_focused(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Key findings and report detail levels")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Plain-language summary: candidate prioritization is now a presentation layer derived from the primary corrected result, not a separate analysis mode.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Filtered collaborator metadata").strong());
                ui.label("A filtered metadata table can define a narrower cohort (for example one selected sample per person). The same corrected methodology is then run on that defined cohort.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Candidate shortlist").strong());
                ui.label("The app ranks a concise set of genes, pathways, transcripts, and other modality signals using the completed corrected results. Optional exploratory direction concordance and PBMC/confounding robustness labels are annotations, not extra statistical votes.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Simplified report").strong());
                ui.label("Shows the comparison, sample/QC summary, main takeaway, strongest candidate signals, a small number of useful plots, and concise limitations. It explicitly describes candidates as preliminary rather than validated biomarkers.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Full detailed report").strong());
                ui.label("Adds the mapping audit, frozen QC decisions and modality evidence, all selected corrected results, sensitivity analyses, pathway provenance, detailed plots/tables, warnings, assumptions, and limitations.");
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Same run, different presentation").strong());
                ui.label("Exporting a different report style never reruns the analysis or changes sample inclusion. The report is a view of the completed run object.");
            });
    }

    fn docs_section_plots(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("How to read the main plots")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Volcano / effect-significance scatter").strong());
                ui.label("Points farther to the left or right have larger negative or positive effects. Points higher on the plot have smaller q values. Features in the upper corners are often the most interesting because they combine effect size with statistical support.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Detection-shift plot").strong());
                ui.label("This shows whether a gene tends to be detected in a larger fraction of one group than the other. It can help identify group-specific or sparsely detected signals that a mean-based plot alone may hide.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("PCA / PCoA").strong());
                ui.label("These are low-dimensional maps of the samples. Samples plotted close together are more similar by the chosen distance or feature space. Coloring by status, subject, or another metadata column can reveal whether the main structure follows biology, time, family, or some possible confounder.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Dendrogram").strong());
                ui.label("A dendrogram shows hierarchical clustering. It answers a similar question to PCA/PCoA but as a tree rather than a map. If cases tend to branch together and controls tend to branch together, that supports group separation; if branches instead follow subject or batch, that may indicate another source of structure.");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Pathway ORA plot").strong());
                ui.label("Each point is a pathway. Stronger enrichment appears higher on the plot and, depending on the chart, may also have a larger odds ratio or overlap. Closely related pathways often share many genes, so several significant pathways can reflect one broader biological program.");
            });
    }

    fn docs_section_references(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Key references and further reading")
            .default_open(false)
            .show(ui, |ui| {
                ui.label("These references are included so collaborators can connect each analysis layer to the broader literature.");
                ui.add_space(6.0);
                self.docs_reference(ui, "False discovery rate", "Benjamini Y, Hochberg Y. Controlling the false discovery rate: a practical and powerful approach to multiple testing. Journal of the Royal Statistical Society Series B (1995).", "https://doi.org/10.1111/j.2517-6161.1995.tb02031.x");
                self.docs_reference(ui, "Count-based RNA-seq modelling", "Love MI, Huber W, Anders S. Moderated estimation of fold change and dispersion for RNA-seq data with DESeq2. Genome Biology (2014).", "https://doi.org/10.1186/s13059-014-0550-8");
                self.docs_reference(ui, "Alternative count-based RNA-seq framework", "Robinson MD, McCarthy DJ, Smyth GK. edgeR: a Bioconductor package for differential expression analysis of digital gene expression data. Bioinformatics (2010).", "https://doi.org/10.1093/bioinformatics/btp616");
                self.docs_reference(ui, "Compositional data analysis", "Aitchison J. The statistical analysis of compositional data. Journal of the Royal Statistical Society Series B (1982).", "https://doi.org/10.1111/j.2517-6161.1982.tb01195.x");
                self.docs_reference(ui, "PERMANOVA", "Anderson MJ. A new method for non-parametric multivariate analysis of variance. Austral Ecology (2001).", "https://onlinelibrary.wiley.com/doi/10.1111/j.1442-9993.2001.01070.pp.x");
                self.docs_reference(ui, "Gene set enrichment / pathways", "Subramanian A et al. Gene set enrichment analysis: a knowledge-based approach for interpreting genome-wide expression profiles. PNAS (2005).", "https://doi.org/10.1073/pnas.0506580102");
                self.docs_reference(ui, "MSigDB Hallmark collection", "Liberzon A et al. The Molecular Signatures Database Hallmark Gene Set Collection. Cell Systems (2015).", "https://doi.org/10.1016/j.cels.2015.12.004");
                self.docs_reference(ui, "Reactome", "Jassal B et al. The Reactome pathway knowledgebase. Nucleic Acids Research (2020).", "https://doi.org/10.1093/nar/gkz1031");
                self.docs_reference(ui, "ssGSEA", "Barbie DA et al. Systematic RNA interference reveals that oncogenic KRAS-driven cancers require TBK1. Nature (2009).", "https://doi.org/10.1038/nature08460");
            });
    }

    fn docs_reference(&self, ui: &mut egui::Ui, label: &str, citation: &str, url: &str) {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{label}: ")).strong());
            ui.label(citation);
            ui.hyperlink_to("Link", url);
        });
    }

    fn card(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        add_contents: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        ui_theme::card(ui, title, |ui| {
            add_contents(self, ui);
        });
    }

    fn load_merge_metadata_path(&mut self, path: PathBuf) {
        match load_metadata(&path) {
            Ok(meta) => {
                let rows = meta.rows.len();
                let id_column = meta.id_column.clone();
                self.merge_metadata_path = Some(path.clone());
                self.merge_metadata = Some(meta);
                self.merge_metadata_filter_enabled = false;

                // A collaborator-supplied metadata file may already be the filtered
                // inclusion list, so do not impose a second value filter by default.
                // The user can optionally choose a metadata column/value subset below.
                self.set_merge_filter_column(None);
                self.remember_directory_for(&path);
                self.status = format!(
                    "Sample metadata loaded: {rows} rows; matching selected workbooks on '{id_column}'. Metadata filtering is off until enabled."
                );
            }
            Err(e) => {
                self.status = format!("Could not load sample metadata: {e:#}");
            }
        }
    }

    fn clear_merge_metadata(&mut self) {
        self.merge_metadata_path = None;
        self.merge_metadata = None;
        self.merge_metadata_filter_enabled = false;
        self.merge_metadata_filter_column = None;
        self.merge_metadata_filter_values.clear();
        self.status =
            "Sample metadata filter cleared; all selected workbooks are eligible for merging."
                .to_string();
    }

    fn set_merge_filter_column(&mut self, column: Option<String>) {
        self.merge_metadata_filter_column = column.clone();
        self.merge_metadata_filter_values.clear();
        let Some(meta) = &self.merge_metadata else {
            return;
        };
        let Some(column) = column else {
            return;
        };
        let Some(col_idx) = meta.column_index(&column) else {
            return;
        };
        for row in &meta.rows {
            self.merge_metadata_filter_values
                .insert(row.get(col_idx).cloned().unwrap_or_default());
        }
    }

    fn merge_filter_value_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        let (Some(meta), Some(column)) = (&self.merge_metadata, &self.merge_metadata_filter_column)
        else {
            return counts;
        };
        let Some(col_idx) = meta.column_index(column) else {
            return counts;
        };
        for row in &meta.rows {
            let value = row.get(col_idx).cloned().unwrap_or_default();
            *counts.entry(value).or_insert(0) += 1;
        }
        counts
    }

    fn merge_metadata_file_statuses(&self) -> Vec<MergeMetadataFileStatus> {
        let Some(meta) = &self.merge_metadata else {
            return self
                .files
                .iter()
                .map(|path| MergeMetadataFileStatus {
                    path: path.clone(),
                    sample_id: merge_metadata_sample_name(path),
                    status: MergeMetadataMatchStatus::Unmatched,
                    metadata_row: None,
                    filter_value: String::new(),
                    included: true,
                })
                .collect();
        };

        let Some(id_col) = meta.column_index(&meta.id_column) else {
            return Vec::new();
        };
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        for (row_idx, row) in meta.rows.iter().enumerate() {
            let key = normalize_sample_id(row.get(id_col).map(String::as_str).unwrap_or_default());
            if !key.is_empty() {
                index.entry(key).or_default().push(row_idx);
            }
        }
        let filter_col = self
            .merge_metadata_filter_column
            .as_ref()
            .and_then(|column| meta.column_index(column));
        let filtering = self.merge_metadata_filter_enabled;

        self.files
            .iter()
            .map(|path| {
                let sample_id = merge_metadata_sample_name(path);
                let key = normalize_sample_id(&sample_id);
                let matches = index.get(&key);
                let (status, metadata_row) = match matches {
                    Some(rows) if rows.len() == 1 => {
                        (MergeMetadataMatchStatus::Matched, Some(rows[0]))
                    }
                    Some(_) => (MergeMetadataMatchStatus::Ambiguous, None),
                    None => (MergeMetadataMatchStatus::Unmatched, None),
                };
                let filter_value = metadata_row
                    .and_then(|row_idx| {
                        filter_col.and_then(|col_idx| meta.rows.get(row_idx)?.get(col_idx))
                    })
                    .cloned()
                    .unwrap_or_default();
                let included = if filtering {
                    status == MergeMetadataMatchStatus::Matched
                        && filter_col
                            .map(|_| self.merge_metadata_filter_values.contains(&filter_value))
                            .unwrap_or(true)
                } else {
                    true
                };
                MergeMetadataFileStatus {
                    path: path.clone(),
                    sample_id,
                    status,
                    metadata_row,
                    filter_value,
                    included,
                }
            })
            .collect()
    }

    fn merge_input_files(&self) -> Vec<PathBuf> {
        if !self.merge_metadata_filter_enabled || self.merge_metadata.is_none() {
            return self.files.clone();
        }
        self.merge_metadata_file_statuses()
            .into_iter()
            .filter(|row| row.included)
            .map(|row| row.path)
            .collect()
    }

    fn merge_metadata_only_count(&self) -> usize {
        let Some(meta) = &self.merge_metadata else {
            return 0;
        };
        let Some(id_col) = meta.column_index(&meta.id_column) else {
            return 0;
        };
        let file_ids = self
            .files
            .iter()
            .map(|path| normalize_sample_id(&merge_metadata_sample_name(path)))
            .collect::<HashSet<_>>();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for row in &meta.rows {
            let id = normalize_sample_id(row.get(id_col).map(String::as_str).unwrap_or_default());
            if !id.is_empty() {
                *counts.entry(id).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .filter(|(id, n)| *n == 1 && !file_ids.contains(id))
            .count()
    }

    fn set_files(&mut self, mut files: Vec<PathBuf>) {
        files.sort();
        files.dedup();
        self.files = files;
        self.detected_kind = self
            .files
            .first()
            .and_then(|p| inspect_kind(p).ok())
            .unwrap_or(DataKind::Auto);
        self.status = if self.files.is_empty() {
            "No .xlsx files selected.".to_string()
        } else {
            format!("Loaded {} files.", self.files.len())
        };
        self.metrics = collaborator_default_metrics(self.effective_kind());
        self.ensure_metrics_valid();

        self.analysis_source = if self.files.is_empty() && self.merged_info.is_some() {
            AnalysisSource::MergedTsv
        } else {
            AnalysisSource::Workbooks
        };
        self.reset_analysis_results();
        self.ensure_analysis_metric_valid();
        self.analysis_status = if self.analysis_source == AnalysisSource::MergedTsv {
            "Merged TSV remains available for Explore & QC.".to_string()
        } else if self.files.is_empty() {
            "Load workbooks or open a merged TSV to enable QC and plotting.".to_string()
        } else {
            "Workbooks ready. Open Explore & QC to build the identifier index.".to_string()
        };
        if self.active_tab == AppTab::Explore
            && self.analysis_source_available()
            && !self.analysis_running
        {
            self.start_catalog_load();
        }
    }

    fn add_files(&mut self, files: Vec<PathBuf>) {
        let mut combined = self.files.clone();
        combined.extend(files);
        let mut seen = HashSet::new();
        combined.retain(|path| seen.insert(path.clone()));
        self.set_files(combined);
    }

    fn effective_kind(&self) -> DataKind {
        match self.kind {
            DataKind::Auto => self.detected_kind,
            other => other,
        }
    }

    fn analysis_kind(&self) -> DataKind {
        match self.analysis_source {
            AnalysisSource::Workbooks => self.effective_kind(),
            AnalysisSource::MergedTsv => self
                .merged_info
                .as_ref()
                .map(|info| info.kind)
                .unwrap_or(DataKind::Auto),
        }
    }

    fn analysis_sample_count(&self) -> usize {
        match self.analysis_source {
            AnalysisSource::Workbooks => self.files.len(),
            AnalysisSource::MergedTsv => self
                .merged_info
                .as_ref()
                .map(|info| info.samples.len())
                .unwrap_or(0),
        }
    }

    fn analysis_source_available(&self) -> bool {
        match self.analysis_source {
            AnalysisSource::Workbooks => !self.files.is_empty(),
            AnalysisSource::MergedTsv => {
                self.merged_analysis_path.is_some() && self.merged_info.is_some()
            }
        }
    }

    fn analysis_available_metrics(&self) -> Vec<Metric> {
        match self.analysis_source {
            AnalysisSource::Workbooks => all_metrics(self.effective_kind())
                .into_iter()
                .filter(|metric| metric.is_numeric())
                .collect(),
            AnalysisSource::MergedTsv => self
                .merged_info
                .as_ref()
                .map(|info| {
                    info.metrics
                        .iter()
                        .copied()
                        .filter(|metric| metric.is_numeric())
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn ensure_metrics_valid(&mut self) {
        let kind = self.effective_kind();
        if kind == DataKind::Auto {
            return;
        }
        self.metrics.retain(|metric| metric.valid_for(kind));
        self.sort_metrics();
    }

    fn ensure_analysis_metric_valid(&mut self) {
        let available = self.analysis_available_metrics();
        if !available.contains(&self.analysis_metric) {
            self.analysis_metric = if available.contains(&Metric::Tpm) {
                Metric::Tpm
            } else {
                available.first().copied().unwrap_or(Metric::Tpm)
            };
            self.distributions.clear();
            self.profile.clear();
        }
    }

    fn reset_analysis_results(&mut self) {
        self.entity_catalog.clear();
        self.entity_visible.clear();
        self.entity_search.clear();
        self.selected_entity = None;
        self.distributions.clear();
        self.profile.clear();
        self.analysis_progress = 0.0;
    }

    fn rebuild_entity_filter(&mut self) {
        let query = self.entity_search.trim().to_ascii_lowercase();
        self.entity_visible = self
            .entity_catalog
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                query.is_empty()
                    || entry.primary.to_ascii_lowercase().contains(&query)
                    || entry.gene_id.to_ascii_lowercase().contains(&query)
                    || entry.gene_symbol.to_ascii_lowercase().contains(&query)
            })
            .map(|(idx, _)| idx)
            .collect();
        if let Some(selected) = self.selected_entity {
            if !self.entity_visible.contains(&selected) && !query.is_empty() {
                self.selected_entity = None;
                self.profile.clear();
            }
        }
    }

    fn set_merged_analysis_file(&mut self, path: PathBuf, load_catalog_now: bool) {
        match inspect_merged_tsv(&path) {
            Ok(info) => {
                let sample_count = info.samples.len();
                let kind = info.kind;
                self.merged_analysis_path = Some(path.clone());
                self.merged_info = Some(info);
                self.analysis_source = AnalysisSource::MergedTsv;
                self.reset_analysis_results();
                self.ensure_analysis_metric_valid();
                self.analysis_status =
                    format!("Merged TSV ready for QC: {sample_count} samples ({kind:?}).");
                if load_catalog_now && !self.analysis_running {
                    self.start_catalog_load();
                }
            }
            Err(e) => {
                self.analysis_status = format!("Could not open merged TSV: {e:#}");
            }
        }
    }

    fn remember_directory_for(&mut self, path: &Path) {
        let directory = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        if directory.as_os_str().is_empty() {
            return;
        }
        self.last_directory = Some(directory.clone());
        save_last_directory(&directory);
    }

    fn handle_input_path(&mut self, path: PathBuf) {
        if !path.exists() {
            self.status = format!("Path does not exist: {}", path.display());
            return;
        }
        self.remember_directory_for(&path);
        if path.is_dir() {
            match cfs_expression_merger::discover_xlsx_files(&path) {
                Ok(files) if files.is_empty() => {
                    self.status = format!("No .xlsx files found in {}", path.display());
                }
                Ok(files) => self.add_files(files),
                Err(e) => self.status = format!("Could not read {}: {e:#}", path.display()),
            }
            return;
        }

        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("xlsx") => self.add_files(vec![path]),
            Some("tsv") => {
                self.active_tab = AppTab::Explore;
                self.set_merged_analysis_file(path, true);
            }
            _ => {
                self.status =
                    "Supported inputs are .xlsx workbooks, folders, and merged .tsv files."
                        .to_string();
            }
        }
    }

    fn handle_dropped_paths(&mut self, ctx: &egui::Context) {
        if self.running || self.analysis_running {
            return;
        }
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }

        let mut xlsx_files = Vec::new();
        let mut merged_tsv = None;
        let mut unsupported = 0usize;
        for path in dropped {
            self.remember_directory_for(&path);
            if path.is_dir() {
                match cfs_expression_merger::discover_xlsx_files(&path) {
                    Ok(mut files) => xlsx_files.append(&mut files),
                    Err(e) => self.status = format!("Could not read dropped folder: {e:#}"),
                }
                continue;
            }
            match path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .as_deref()
            {
                Some("xlsx") => xlsx_files.push(path),
                Some("tsv") => merged_tsv = Some(path),
                _ => unsupported += 1,
            }
        }
        if !xlsx_files.is_empty() {
            self.add_files(xlsx_files);
        }
        if let Some(path) = merged_tsv {
            self.active_tab = AppTab::Explore;
            self.set_merged_analysis_file(path, true);
        }
        if unsupported > 0 {
            self.status = format!("Ignored {unsupported} unsupported dropped item(s).");
        }
    }

    fn sort_metrics(&mut self) {
        let canonical = all_metrics(self.effective_kind());
        self.metrics.sort_by_key(|metric| {
            canonical
                .iter()
                .position(|candidate| candidate == metric)
                .unwrap_or(usize::MAX)
        });
        self.metrics.dedup();
    }

    fn selected_metrics(&self) -> Vec<Metric> {
        let canonical = all_metrics(self.effective_kind());
        canonical
            .into_iter()
            .filter(|metric| self.metrics.contains(metric))
            .collect()
    }

    fn output_column_preview(&self, max_columns: usize) -> String {
        let kind = self.effective_kind();
        let metrics = self.selected_metrics();
        let merge_files = self.merge_input_files();
        if merge_files.is_empty() || metrics.is_empty() {
            if self.merge_metadata_filter_enabled && !self.files.is_empty() {
                return "The metadata filter currently includes no workbooks. Adjust the metadata match/filter values before merging.".to_string();
            }
            return "Select at least one input workbook and measurement.".to_string();
        }

        let mut columns: Vec<String> = match kind {
            DataKind::Gene => vec!["Gene Symbol".to_string(), "NCBI Gene ID".to_string()],
            DataKind::Transcript => {
                vec!["Transcript Name".to_string(), "NCBI Gene ID".to_string()]
            }
            DataKind::Auto => vec!["Identifier".to_string(), "NCBI Gene ID".to_string()],
        };

        let sample_names: Vec<String> = merge_files.iter().map(|p| sample_name(p)).collect();
        match self.order {
            ColumnOrder::AdjacentBySample => {
                'outer_sample: for sample in &sample_names {
                    for metric in &metrics {
                        columns.push(format!("{sample} {}", metric.display_name()));
                        if columns.len() >= max_columns {
                            break 'outer_sample;
                        }
                    }
                }
            }
            ColumnOrder::GroupedByMeasurement => {
                'outer_metric: for metric in &metrics {
                    for sample in &sample_names {
                        columns.push(format!("{sample} {}", metric.display_name()));
                        if columns.len() >= max_columns {
                            break 'outer_metric;
                        }
                    }
                }
            }
        }

        let total_columns = 2usize.saturating_add(sample_names.len().saturating_mul(metrics.len()));
        let mut preview = columns.join("  |  ");
        if total_columns > columns.len() {
            preview.push_str("  |  …");
        }
        preview
    }

    fn default_output_name(&self) -> &'static str {
        match self.effective_kind() {
            DataKind::Transcript => "merged_transcript_expression.tsv",
            _ => "merged_gene_expression.tsv",
        }
    }

    fn start_merge(&mut self) {
        let Some(output) = self.output.clone() else {
            return;
        };
        let merge_files = self.merge_input_files();
        if merge_files.is_empty() {
            self.status = if self.merge_metadata_filter_enabled {
                "Metadata filtering currently includes no workbooks. Check the sample ID column, filter values, and metadata matches.".to_string()
            } else {
                "No input workbooks are selected.".to_string()
            };
            return;
        }
        let merge_count = merge_files.len();
        let config = MergeConfig {
            files: merge_files,
            output,
            kind: self.kind,
            metrics: self.selected_metrics(),
            column_order: self.order,
        };

        let (tx, rx) = mpsc::channel();
        self.merge_receiver = Some(rx);
        self.running = true;
        self.progress = 0.0;
        self.status = format!("Starting merge of {merge_count} included sample workbook(s)…");

        std::thread::spawn(move || {
            let tx_progress = tx.clone();
            let result = merge_wide(config, move |done, total, msg| {
                let _ =
                    tx_progress.send(MergeWorkerMessage::Progress(done, total, msg.to_string()));
            });
            let _ = tx.send(MergeWorkerMessage::Done(
                result.map_err(|e| format!("{e:#}")),
            ));
        });
    }

    fn start_catalog_load(&mut self) {
        if self.analysis_running || !self.analysis_source_available() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.analysis_receiver = Some(rx);
        self.analysis_running = true;
        self.analysis_progress = 0.0;
        self.analysis_status = "Loading searchable identifier index…".to_string();

        match self.analysis_source {
            AnalysisSource::Workbooks => {
                let Some(first) = self.files.first().cloned() else {
                    self.analysis_running = false;
                    return;
                };
                let kind = self.kind;
                std::thread::spawn(move || {
                    let result = load_entity_catalog(&first, kind).map_err(|e| format!("{e:#}"));
                    let _ = tx.send(AnalysisWorkerMessage::CatalogDone(result));
                });
            }
            AnalysisSource::MergedTsv => {
                let Some(path) = self.merged_analysis_path.clone() else {
                    self.analysis_running = false;
                    return;
                };
                std::thread::spawn(move || {
                    let result =
                        load_entity_catalog_from_merged(&path).map_err(|e| format!("{e:#}"));
                    let _ = tx.send(AnalysisWorkerMessage::CatalogDone(result));
                });
            }
        }
    }

    fn start_distribution_analysis(&mut self) {
        if self.analysis_running || !self.analysis_source_available() {
            return;
        }
        let metric = self.analysis_metric;
        let ignore_zeros = self.analysis_ignore_zeros;
        let (tx, rx) = mpsc::channel();
        self.analysis_receiver = Some(rx);
        self.analysis_running = true;
        self.analysis_progress = 0.0;
        self.analysis_status = format!("Calculating {} sample QC…", metric.display_name());

        match self.analysis_source {
            AnalysisSource::Workbooks => {
                let files = self.files.clone();
                let kind = self.kind;
                std::thread::spawn(move || {
                    let progress_tx = tx.clone();
                    let result = summarize_sample_distributions(
                        &files,
                        kind,
                        metric,
                        ignore_zeros,
                        move |done, total, msg| {
                            let _ = progress_tx.send(AnalysisWorkerMessage::Progress(
                                done,
                                total,
                                msg.to_string(),
                            ));
                        },
                    )
                    .map_err(|e| format!("{e:#}"));
                    let _ = tx.send(AnalysisWorkerMessage::DistributionDone(result));
                });
            }
            AnalysisSource::MergedTsv => {
                let Some(path) = self.merged_analysis_path.clone() else {
                    self.analysis_running = false;
                    return;
                };
                std::thread::spawn(move || {
                    let progress_tx = tx.clone();
                    let result = summarize_merged_distributions(
                        &path,
                        metric,
                        ignore_zeros,
                        move |done, total, msg| {
                            let _ = progress_tx.send(AnalysisWorkerMessage::Progress(
                                done,
                                total,
                                msg.to_string(),
                            ));
                        },
                    )
                    .map_err(|e| format!("{e:#}"));
                    let _ = tx.send(AnalysisWorkerMessage::DistributionDone(result));
                });
            }
        }
    }

    fn start_profile_analysis(&mut self) {
        if self.analysis_running || !self.analysis_source_available() {
            return;
        }
        let Some(idx) = self.selected_entity else {
            self.analysis_status = "Select a gene/transcript row first.".to_string();
            return;
        };
        let Some(entry) = self.entity_catalog.get(idx).cloned() else {
            self.analysis_status = "The selected identifier is no longer available.".to_string();
            return;
        };
        let metric = self.analysis_metric;
        let (tx, rx) = mpsc::channel();
        self.analysis_receiver = Some(rx);
        self.analysis_running = true;
        self.analysis_progress = 0.0;
        self.analysis_status = format!("Loading {} across samples…", entry.primary);

        match self.analysis_source {
            AnalysisSource::Workbooks => {
                let files = self.files.clone();
                let kind = self.kind;
                std::thread::spawn(move || {
                    let progress_tx = tx.clone();
                    let result = load_entity_profile(
                        &files,
                        kind,
                        metric,
                        &entry.primary,
                        &entry.gene_id,
                        move |done, total, msg| {
                            let _ = progress_tx.send(AnalysisWorkerMessage::Progress(
                                done,
                                total,
                                msg.to_string(),
                            ));
                        },
                    )
                    .map_err(|e| format!("{e:#}"));
                    let _ = tx.send(AnalysisWorkerMessage::ProfileDone(result));
                });
            }
            AnalysisSource::MergedTsv => {
                let Some(path) = self.merged_analysis_path.clone() else {
                    self.analysis_running = false;
                    return;
                };
                std::thread::spawn(move || {
                    let result = load_entity_profile_from_merged(
                        &path,
                        metric,
                        &entry.primary,
                        &entry.gene_id,
                    )
                    .map_err(|e| format!("{e:#}"));
                    let _ = tx.send(AnalysisWorkerMessage::ProfileDone(result));
                });
            }
        }
    }

    fn poll_merge_worker(&mut self) {
        let Some(rx) = self.merge_receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        while let Ok(message) = rx.try_recv() {
            match message {
                MergeWorkerMessage::Progress(done, total, msg) => {
                    self.progress = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    self.status = msg;
                }
                MergeWorkerMessage::Done(result) => {
                    self.running = false;
                    self.progress = 1.0;
                    keep_receiver = false;
                    match result {
                        Ok(summary) => {
                            self.status = format!(
                                "Done: {} rows × {} samples. Missing rows: {}; unknown rows: {}. Saved to {}",
                                summary.rows,
                                summary.samples,
                                summary.missing_rows,
                                summary.unknown_rows,
                                summary.output.display()
                            );
                            self.set_merged_analysis_file(summary.output.clone(), false);
                            self.analysis_status = format!(
                                "Merge complete. Explore & QC will use the merged TSV automatically ({} samples).",
                                summary.samples
                            );
                        }
                        Err(e) => self.status = format!("Merge failed: {e}"),
                    }
                }
            }
        }
        if keep_receiver {
            self.merge_receiver = Some(rx);
        }
    }

    fn poll_analysis_worker(&mut self) {
        let Some(rx) = self.analysis_receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        while let Ok(message) = rx.try_recv() {
            match message {
                AnalysisWorkerMessage::Progress(done, total, msg) => {
                    self.analysis_progress = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    self.analysis_status = msg;
                }
                AnalysisWorkerMessage::CatalogDone(result) => {
                    self.analysis_running = false;
                    self.analysis_progress = 1.0;
                    keep_receiver = false;
                    match result {
                        Ok(catalog) => {
                            let n = catalog.len();
                            self.entity_catalog = catalog;
                            self.rebuild_entity_filter();
                            let source = if self.analysis_source == AnalysisSource::MergedTsv {
                                "merged TSV"
                            } else {
                                "first workbook"
                            };
                            self.analysis_status =
                                format!("Search index ready: {n} identifiers from {source}.");
                        }
                        Err(e) => {
                            self.entity_catalog.clear();
                            self.analysis_status = format!("Could not load search index: {e}");
                        }
                    }
                }
                AnalysisWorkerMessage::DistributionDone(result) => {
                    self.analysis_running = false;
                    self.analysis_progress = 1.0;
                    keep_receiver = false;
                    match result {
                        Ok(distributions) => {
                            let n = distributions.len();
                            self.distributions = distributions;
                            self.analysis_status = format!("Sample QC ready for {n} samples.");
                        }
                        Err(e) => {
                            self.distributions.clear();
                            self.analysis_status = format!("Sample QC failed: {e}");
                        }
                    }
                }
                AnalysisWorkerMessage::ProfileDone(result) => {
                    self.analysis_running = false;
                    self.analysis_progress = 1.0;
                    keep_receiver = false;
                    match result {
                        Ok(profile) => {
                            let n = profile.iter().filter(|p| p.value.is_some()).count();
                            self.profile = profile;
                            self.analysis_status =
                                format!("Profile ready: value found in {n} samples.");
                        }
                        Err(e) => {
                            self.profile.clear();
                            self.analysis_status = format!("Profile failed: {e}");
                        }
                    }
                }
            }
        }
        if keep_receiver {
            self.analysis_receiver = Some(rx);
        }
    }

    fn plot_axis_label(&self) -> String {
        if self.analysis_log1p {
            format!("log10({} + 1)", self.analysis_metric.display_name())
        } else {
            self.analysis_metric.display_name().to_string()
        }
    }
}

fn native_metadata_comparison_candidates(metadata: &MetadataTable) -> Vec<(String, usize, bool)> {
    metadata
        .columns
        .iter()
        .filter(|column| column.as_str() != metadata.id_column.as_str())
        .filter_map(|column| {
            let distinct = metadata_unique_values(metadata, column).len();
            (distinct >= 2).then(|| (column.clone(), distinct, (2..=20).contains(&distinct)))
        })
        .collect()
}

fn suggested_native_comparison_column(metadata: &MetadataTable) -> Option<String> {
    let candidates = native_metadata_comparison_candidates(metadata);
    candidates
        .iter()
        .find(|(name, distinct, _)| name == "analysis_group" && *distinct >= 2)
        .map(|(name, _, _)| name.clone())
        .or_else(|| {
            candidates
                .iter()
                .find(|(_, _, comparison_ready)| *comparison_ready)
                .map(|(name, _, _)| name.clone())
        })
}

fn native_selection_label(values: &HashSet<String>) -> String {
    match values.len() {
        0 => "(select one or more)".to_string(),
        1 => values.iter().next().cloned().unwrap_or_default(),
        n => format!("{n} values selected"),
    }
}

fn native_selected_metadata_rows(values: &[(String, usize)], selected: &HashSet<String>) -> usize {
    values
        .iter()
        .filter(|(value, _)| selected.contains(value))
        .map(|(_, count)| *count)
        .sum()
}

fn metric_help(metric: Metric) -> &'static str {
    match metric {
        Metric::RawCount => "Raw RNA-seq gene counts from the source report.",
        Metric::Fpkm => "Fragments Per Kilobase of transcript per Million mapped reads.",
        Metric::Cpm => "Counts Per Million mapped reads.",
        Metric::Tpm => {
            "Transcripts Per Million; commonly useful for comparing expression abundance within a sample."
        }
        Metric::PercentileRank => "Expression percentile rank supplied by the source report.",
        Metric::IsExpressed => "Categorical yes/no expression call supplied by the source report.",
        Metric::TranscriptLength => "Reported transcript length.",
        Metric::EffectiveTranscriptLength => {
            "Effective transcript length used for abundance estimation."
        }
        Metric::TotalReads => "Raw/estimated read count assigned to the transcript.",
    }
}

fn plot_transform(value: f64, log1p: bool) -> f64 {
    if log1p && value >= 0.0 {
        (value + 1.0).log10()
    } else {
        value
    }
}

fn sample_tick_label(value: f64, samples: &[String]) -> String {
    let rounded = value.round();
    if (value - rounded).abs() > 0.12 || rounded < 0.0 {
        return String::new();
    }
    let idx = rounded as usize;
    samples.get(idx).cloned().unwrap_or_default()
}

fn analysis_doc_tick_label(value: f64, labels: &[String]) -> String {
    let rounded = value.round();
    if (value - rounded).abs() > 0.04 || rounded < 0.0 {
        return String::new();
    }
    let idx = rounded as usize;
    labels.get(idx).cloned().unwrap_or_default()
}

fn box_spread_from_values(values: &[f64]) -> Option<BoxSpread> {
    let mut sorted = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let q1 = quantile_sorted(&sorted, 0.25);
    let median = quantile_sorted(&sorted, 0.50);
    let q3 = quantile_sorted(&sorted, 0.75);
    let iqr = q3 - q1;
    let lower_fence = q1 - 1.5 * iqr;
    let upper_fence = q3 + 1.5 * iqr;
    let lower_whisker = sorted
        .iter()
        .copied()
        .find(|value| *value >= lower_fence)
        .unwrap_or(sorted[0]);
    let upper_whisker = sorted
        .iter()
        .copied()
        .rev()
        .find(|value| *value <= upper_fence)
        .unwrap_or(*sorted.last().unwrap());
    Some(BoxSpread::new(lower_whisker, q1, median, q3, upper_whisker))
}

fn quantile_sorted(values: &[f64], probability: f64) -> f64 {
    debug_assert!(!values.is_empty());
    if values.len() == 1 {
        return values[0];
    }
    let position = probability.clamp(0.0, 1.0) * (values.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        values[lower]
    } else {
        let weight = position - lower as f64;
        values[lower] * (1.0 - weight) + values[upper] * weight
    }
}

fn format_optional(value: Option<f64>) -> String {
    value.map(format_value).unwrap_or_else(|| "NA".to_string())
}

fn format_value(value: f64) -> String {
    if value.abs() >= 100_000.0 || (value != 0.0 && value.abs() < 0.001) {
        format!("{value:.3e}")
    } else if value.abs() >= 100.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.4}")
    }
}

fn app_state_file() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|base| base.join("CFSExpressionMerger").join("last_directory.txt"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME").map(PathBuf::from).map(|base| {
            base.join("Library")
                .join("Application Support")
                .join("CFSExpressionMerger")
                .join("last_directory.txt")
        });
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return Some(
                PathBuf::from(xdg)
                    .join("cfs-expression-merger")
                    .join("last_directory.txt"),
            );
        }
        return std::env::var_os("HOME").map(PathBuf::from).map(|base| {
            base.join(".config")
                .join("cfs-expression-merger")
                .join("last_directory.txt")
        });
    }
    #[allow(unreachable_code)]
    None
}

fn load_last_directory() -> Option<PathBuf> {
    let state_file = app_state_file()?;
    let text = fs::read_to_string(state_file).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_dir().then_some(path)
}

fn save_last_directory(path: &Path) {
    let Some(state_file) = app_state_file() else {
        return;
    };
    if let Some(parent) = state_file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(state_file, path.to_string_lossy().as_bytes());
}

#[cfg(test)]
mod merge_metadata_gui_tests {
    use super::*;

    #[test]
    fn metadata_match_sample_name_handles_expression_suffix_and_copy_suffix() {
        assert_eq!(
            merge_metadata_sample_name(Path::new("4C_101_tumor_rna_gene_expression_report.xlsx")),
            "4C_101"
        );
        assert_eq!(
            merge_metadata_sample_name(Path::new(
                "4C_465_tumor_rna_gene_expression_report (1).xlsx"
            )),
            "4C_465"
        );
        assert_eq!(
            merge_metadata_sample_name(Path::new(
                "Normal_FJ_RNA_20_tumor_rna_transcript_expression_report.xlsx"
            )),
            "Normal_FJ_RNA_20"
        );
    }

    #[test]
    fn native_metadata_grouping_prefers_analysis_group_then_first_low_cardinality_field() {
        let metadata = MetadataTable {
            path: PathBuf::from("metadata.tsv"),
            columns: vec![
                "sample_id".to_string(),
                "batch".to_string(),
                "analysis_group".to_string(),
                "free_text".to_string(),
            ],
            rows: vec![
                vec!["S1", "A", "Case", "x"],
                vec!["S2", "A", "Control", "y"],
                vec!["S3", "B", "Case", "z"],
            ]
            .into_iter()
            .map(|row| row.into_iter().map(str::to_string).collect())
            .collect(),
            id_column: "sample_id".to_string(),
        };
        assert_eq!(
            suggested_native_comparison_column(&metadata).as_deref(),
            Some("analysis_group")
        );

        let mut without_analysis_group = metadata.clone();
        without_analysis_group.columns.remove(2);
        for row in &mut without_analysis_group.rows {
            row.remove(2);
        }
        assert_eq!(
            suggested_native_comparison_column(&without_analysis_group).as_deref(),
            Some("batch")
        );
    }

    #[test]
    fn native_metadata_group_selection_counts_rows_and_labels_without_typed_values() {
        let values = vec![
            ("HYP+ME/CFS".to_string(), 12),
            ("ME/CFS".to_string(), 14),
            ("True_HC".to_string(), 79),
        ];
        let cases = HashSet::from(["HYP+ME/CFS".to_string(), "ME/CFS".to_string()]);
        let controls = HashSet::from(["True_HC".to_string()]);
        assert_eq!(native_selected_metadata_rows(&values, &cases), 26);
        assert_eq!(native_selected_metadata_rows(&values, &controls), 79);
        assert_eq!(native_selection_label(&cases), "2 values selected");
        assert_eq!(native_selection_label(&controls), "True_HC");
    }
}
