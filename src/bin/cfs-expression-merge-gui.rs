use cfs_expression_merger::cohort::{
    build_comparison, build_mapping, load_metadata, run_bcr_comparison, run_bcr_isotype_comparison,
    run_gene_comparison, run_transcript_comparison, run_tcr_comparison, run_immune_infiltrate_comparison,
    scan_project, write_bcr_isotype_values, write_bcr_sample_summaries, write_feature_tests,
    write_immune_values, write_mapping_audit, write_mapping_audit_csv, write_tcr_sample_summaries,
    BcrComparisonResult, BcrIsotypeComparisonResult, CohortComparison, CohortMapping, ComparisonPreset,
    FeatureTestResult, GeneComparisonResult, ImmuneComparisonResult, MappingStatus, MetadataTable,
    ProjectScan, CohortModality, TcrComparisonResult,
};
use cfs_expression_merger::report::{generate_cohort_html_report, generate_corrected_html_report, CohortReportInputs, CorrectedReportInputs};
use cfs_expression_merger::corrected::{
    run_corrected_expression, run_corrected_immune, run_corrected_isotype, run_corrected_repertoire, run_pathway_ora,
    write_adjusted_tests, write_corrected_repertoire_samples, write_dtu_gene_tests, write_ordination, write_pathway_ora, AdjustedFeatureResult,
    AmbiguousCallPolicy, CorrectedCompositionResult, CorrectedExpressionResult, CorrectedImmuneResult,
    CorrectedIsotypeResult, CorrectedModelSpec, CorrectedPathwayResult, CorrectedRepertoireResult, OrdinationResult,
};
use cfs_expression_merger::{
    all_metrics, collaborator_default_metrics, inspect_kind, inspect_merged_tsv, load_entity_catalog,
    load_entity_catalog_from_merged, load_entity_profile, load_entity_profile_from_merged,
    merge_wide, sample_name, summarize_merged_distributions, summarize_sample_distributions,
    ColumnOrder, DataKind, EntityCatalogEntry, EntityProfilePoint, MergeConfig, MergeSummary,
    MergedDatasetInfo, Metric, SampleDistributionSummary,
};
use eframe::egui;
use egui_plot::{Bar, BarChart, BoxElem, BoxPlot, BoxSpread, HoverPosition, Line, Plot, Points};
use rfd::FileDialog;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

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
    Cohort,
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
struct CorrectedAnalysisBundle {
    comparison: CohortComparison,
    spec: CorrectedModelSpec,
    gene: Option<CorrectedExpressionResult>,
    gene_error: Option<String>,
    transcript: Option<CorrectedExpressionResult>,
    transcript_error: Option<String>,
    bcr: Option<CorrectedRepertoireResult>,
    bcr_error: Option<String>,
    bcr_isotype: Option<CorrectedIsotypeResult>,
    bcr_isotype_error: Option<String>,
    tcr_alpha: Option<CorrectedRepertoireResult>,
    tcr_alpha_error: Option<String>,
    tcr_beta: Option<CorrectedRepertoireResult>,
    tcr_beta_error: Option<String>,
    immune: Option<CorrectedImmuneResult>,
    immune_error: Option<String>,
    pathway: Option<CorrectedPathwayResult>,
    pathway_error: Option<String>,
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
}

struct MergeApp {
    active_tab: AppTab,
    files: Vec<PathBuf>,
    sample_filter: String,
    input_path_text: String,
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

    cohort_root: Option<PathBuf>,
    cohort_scan: Option<ProjectScan>,
    cohort_metadata_path: Option<PathBuf>,
    cohort_metadata: Option<MetadataTable>,
    cohort_mapping: Option<CohortMapping>,
    cohort_preset: ComparisonPreset,
    cohort_status: String,
    cohort_progress: f32,
    cohort_running: bool,
    cohort_receiver: Option<Receiver<CohortWorkerMessage>>,
    cohort_results: Option<CohortAnalysisBundle>,
    cohort_result_view: CohortResultView,
    cohort_analysis_mode: CohortAnalysisMode,
    corrected_results: Option<CorrectedAnalysisBundle>,
    corrected_result_view: CorrectedResultView,
    corrected_subject_column: Option<String>,
    corrected_covariates: HashSet<String>,
    corrected_ambiguous_policy: AmbiguousCallPolicy,
    corrected_exclude_undetermined: bool,
    corrected_permutations: usize,
    corrected_rarefaction_reps: usize,
    corrected_rarefaction_cap: usize,
    corrected_run_pathway: bool,
    corrected_gmt_path: Option<PathBuf>,
    corrected_pathway_q: f64,
    corrected_pathway_effect: f64,
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

impl MergeApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.style_mut_of(egui::Theme::Dark, |style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(11.0, 6.0);
            style.spacing.interact_size.y = 28.0;
            style.spacing.tooltip_width = 360.0;
        });

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(22, 24, 29);
        visuals.faint_bg_color = egui::Color32::from_rgb(29, 32, 39);
        visuals.extreme_bg_color = egui::Color32::from_rgb(17, 19, 23);
        visuals.selection.bg_fill = egui::Color32::from_rgb(45, 125, 190);
        visuals.hyperlink_color = egui::Color32::from_rgb(105, 180, 235);
        cc.egui_ctx.set_visuals(visuals);

        Self {
            active_tab: AppTab::Merge,
            files: Vec::new(),
            sample_filter: String::new(),
            input_path_text: String::new(),
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
            analysis_status: "Load workbooks or open a previously merged TSV to enable QC and plotting.".to_string(),
            analysis_progress: 0.0,
            analysis_running: false,
            analysis_receiver: None,
            entity_catalog: Vec::new(),
            entity_visible: Vec::new(),
            entity_search: String::new(),
            selected_entity: None,
            distributions: Vec::new(),
            profile: Vec::new(),

            cohort_root: None,
            cohort_scan: None,
            cohort_metadata_path: None,
            cohort_metadata: None,
            cohort_mapping: None,
            cohort_preset: ComparisonPreset::HypVsTrueHc,
            cohort_status: "Choose the dataset root to scan all supported data types.".to_string(),
            cohort_progress: 0.0,
            cohort_running: false,
            cohort_receiver: None,
            cohort_results: None,
            cohort_result_view: CohortResultView::Gene,
            cohort_analysis_mode: CohortAnalysisMode::Exploratory,
            corrected_results: None,
            corrected_result_view: CorrectedResultView::Gene,
            corrected_subject_column: None,
            corrected_covariates: HashSet::new(),
            corrected_ambiguous_policy: AmbiguousCallPolicy::Exclude,
            corrected_exclude_undetermined: false,
            corrected_permutations: 499,
            corrected_rarefaction_reps: 3,
            corrected_rarefaction_cap: 50_000,
            corrected_run_pathway: false,
            corrected_gmt_path: None,
            corrected_pathway_q: 0.05,
            corrected_pathway_effect: 0.5,
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
                    AppTab::Cohort => self.render_cohort_tab(ui),
                });
        });

        if self.running || self.analysis_running || self.cohort_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl MergeApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let accent = ui.visuals().selection.bg_fill;
        egui::Frame::new()
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(egui::Margin::symmetric(16, 12))
            .corner_radius(10)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("4C Cohort Explorer")
                                .size(24.0)
                                .strong()
                                .color(accent),
                        );
                        ui.label(
                            egui::RichText::new(
                                "RNA merging plus metadata-driven multi-modal cohort analysis",
                            )
                            .weak(),
                        );
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
                            AppTab::Cohort => (
                                "Cohort project",
                                self.cohort_scan.as_ref().map(|scan| scan.unique_samples()).unwrap_or(0),
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
            let cohort = ui
                .selectable_label(self.active_tab == AppTab::Cohort, "Cohort Analysis")
                .on_hover_text("Scan a complete cohort dataset, join sample metadata, audit unmatched samples, and run exploratory gene/BCR/BCR-isotype comparisons.");
            if cohort.clicked() {
                self.active_tab = AppTab::Cohort;
            }
        });
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

            ui.add_space(6.0);
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
            let matching: Vec<(usize, &PathBuf)> = this
                .files
                .iter()
                .enumerate()
                .filter(|(_, path)| {
                    if filter.is_empty() {
                        true
                    } else {
                        sample_name(path).to_ascii_lowercase().contains(&filter)
                            || path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_ascii_lowercase()
                                .contains(&filter)
                    }
                })
                .take(250)
                .collect();

            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("Sample preview ({})", this.files.len())).strong(),
            );
            egui::ScrollArea::vertical()
                .max_height(245.0)
                .show(ui, |ui| {
                    egui::Grid::new("sample_preview_grid_v03")
                        .striped(true)
                        .num_columns(3)
                        .min_col_width(50.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("#").strong());
                            ui.label(egui::RichText::new("Sample ID").strong());
                            ui.label(egui::RichText::new("Source workbook").strong());
                            ui.end_row();

                            for (idx, path) in matching {
                                ui.label((idx + 1).to_string());
                                ui.monospace(sample_name(path));
                                ui.label(path.file_name().unwrap_or_default().to_string_lossy());
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
            let ready = !this.running
                && !this.files.is_empty()
                && this.output.is_some()
                && !this.selected_metrics().is_empty();
            let merge = ui
                .add_enabled(
                    ready,
                    egui::Button::new(egui::RichText::new("Merge datasets").strong())
                        .min_size(egui::vec2(150.0, 34.0)),
                )
                .on_hover_text("Start the bounded-memory merge. Workbooks are processed in chunks and the final TSV is streamed to disk.")
                .on_disabled_hover_text("Select input files, at least one measurement, and an output path before merging.");
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
            egui::ScrollArea::vertical()
                .max_height(210.0)
                .show(ui, |ui| {
                    egui::Grid::new("distribution_summary_grid_v03")
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
            ui.label(
                egui::RichText::new("● Mean").color(egui::Color32::from_rgb(255, 190, 70)),
            );
            ui.label(
                egui::RichText::new("The contrasting markers make the center of each box easier to compare.")
                    .small()
                    .weak(),
            );
        });
    }

    fn render_detection_qc(&self, ui: &mut egui::Ui) {
        let sample_names: Vec<String> = self.distributions.iter().map(|s| s.sample.clone()).collect();
        let line_points: Vec<[f64; 2]> = self
            .distributions
            .iter()
            .enumerate()
            .filter_map(|(idx, summary)| {
                summary
                    .nonzero_fraction()
                    .map(|v| [idx as f64, 100.0 * v])
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
                    let Some(frac) = summary.nonzero_fraction() else { continue; };
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
                    let (Some(frac), Some(median)) = (summary.nonzero_fraction(), summary.median) else {
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
            plot.x_axis_label("Sample").x_axis_formatter(move |mark, _range| {
                sample_tick_label(mark.value, &axis_samples)
            })
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
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    egui::Grid::new("profile_value_grid_v03")
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
        self.card(ui, "1. Cohort project & metadata", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "Scan the dataset root once. The app detects gene/transcript expression, BCR, BCR isotype, TCR alpha/beta, immune-infiltrate files, merged expression tables, and metadata candidates.",
                )
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
                if let Some(root) = this.cohort_root.clone() {
                    ui.monospace(root.display().to_string());
                    let root_clone = root.clone();
                    let rescan = ui
                        .add_enabled(!this.cohort_running, egui::Button::new("Rescan"))
                        .on_hover_text("Rescan the same project root after new files or merged outputs have been added.");
                    if rescan.clicked() {
                        this.load_cohort_project(root_clone);
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
                    if let Some(path) = &this.cohort_metadata_path {
                        ui.monospace(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        );
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
                        this.rebuild_cohort_mapping();
                    }
                }
            } else {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("No cohort project loaded.").weak());
            }
        });

        ui.add_space(10.0);
        self.render_cohort_mapping_card(ui);
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Analysis layer:").strong());
            ui.selectable_value(
                &mut self.cohort_analysis_mode,
                CohortAnalysisMode::Exploratory,
                "Exploratory discovery",
            )
            .on_hover_text("Original first-pass screens: all mapped samples are observations; non-parametric tests and effect sizes are used without repeated-measure/covariate adjustment.");
            ui.selectable_value(
                &mut self.cohort_analysis_mode,
                CohortAnalysisMode::Corrected,
                "Corrected / standard",
            )
            .on_hover_text("Subject-aware and covariate-adjusted analyses, matched-depth repertoire diversity, compositional CLR/Aitchison analyses, ordination, and restricted-permutation PERMANOVA.");
        });
        ui.add_space(8.0);
        match self.cohort_analysis_mode {
            CohortAnalysisMode::Exploratory => {
                self.render_cohort_comparison_card(ui);
                if self.cohort_results.is_some() {
                    ui.add_space(10.0);
                    self.render_cohort_results_card(ui);
                }
            }
            CohortAnalysisMode::Corrected => {
                self.render_corrected_analysis_card(ui);
                if self.corrected_results.is_some() {
                    ui.add_space(10.0);
                    self.render_corrected_results_card(ui);
                }
            }
        }
    }

    fn render_cohort_mapping_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "2. Sample mapping & coverage audit", |this, ui| {
            let Some(mapping) = this.cohort_mapping.clone() else {
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
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show_rows(ui, 24.0, mapping.rows.len(), |ui, range| {
                    egui::Grid::new("cohort_mapping_grid")
                        .striped(true)
                        .num_columns(12)
                        .show(ui, |ui| {
                            if range.start == 0 {
                                for heading in [
                                    "Sample",
                                    "Map",
                                    "Phenotype",
                                    "HYP",
                                    "Gene",
                                    "Tx",
                                    "BCR",
                                    "Iso",
                                    "TCRα",
                                    "TCRβ",
                                    "Immune",
                                    "Notes",
                                ] {
                                    ui.label(egui::RichText::new(heading).strong());
                                }
                                ui.end_row();
                            }
                            for row_idx in range {
                                let row = &mapping.rows[row_idx];
                                ui.monospace(&row.sample_id);
                                match row.mapping_status {
                                    MappingStatus::Matched => {
                                        ui.colored_label(egui::Color32::from_rgb(105, 190, 130), "matched");
                                    }
                                    MappingStatus::Unmatched => {
                                        ui.colored_label(egui::Color32::from_rgb(230, 190, 90), "unmatched");
                                    }
                                    MappingStatus::Ambiguous => {
                                        ui.colored_label(egui::Color32::from_rgb(225, 120, 110), "ambiguous");
                                    }
                                    MappingStatus::MetadataOnly => {
                                        ui.colored_label(egui::Color32::from_rgb(130, 165, 220), "metadata only");
                                    }
                                }
                                ui.label(&row.phenotype);
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
                                    if n > 0 {
                                        ui.label(n.to_string());
                                    } else {
                                        ui.label("·");
                                    }
                                }
                                let duplicates: Vec<_> = row
                                    .counts
                                    .iter()
                                    .filter(|(_, n)| **n > 1)
                                    .map(|(m, n)| format!("{}×{}", m.display_name(), n))
                                    .collect();
                                ui.label(if duplicates.is_empty() {
                                    String::new()
                                } else {
                                    format!("duplicate: {}", duplicates.join(", "))
                                });
                                ui.end_row();
                            }
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

    fn render_corrected_analysis_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "3. Corrected / standard analysis", |this, ui| {
            ui.label(
                egui::RichText::new(
                    "This layer is designed for the more rigorous pass: repeated samples can be clustered by subject, selected metadata covariates are included in the model, repertoire diversity is compared at a common depth, and repertoire/isotype compositions are analyzed in CLR/Aitchison space.",
                )
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Comparison:");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypVsTrueHc, "HYP vs True_HC")
                    .on_hover_text("All HYP=yes samples versus true healthy controls.");
                ui.selectable_value(&mut this.cohort_preset, ComparisonPreset::HypMecfsVsMecfsOnly, "HYP+ME/CFS vs ME/CFS-only")
                    .on_hover_text("HYP+ME/CFS versus ME/CFS samples without HYP.");
            });

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
                    .on_hover_text("Uses RNA-Seq Raw Count columns, positive-count median-ratio normalization, a native NB2 GLM with log(size-factor) offset, covariates, subject-clustered sandwich SEs when repeated samples are configured, BH FDR, subject-collapsed sensitivity tests, and PCA of variable genes.");
                ui.checkbox(&mut this.cohort_run_transcript, "Transcript counts")
                    .on_hover_text("Uses Total Reads with the same NB count model and also runs a dependency-light within-gene CLR differential-transcript-usage screen with Simes/BH gene-level summaries.");
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
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut this.corrected_run_pathway, "Pathway ORA")
                    .on_hover_text("Optional gene-set over-representation analysis using a user-supplied versioned GMT collection. It uses the corrected gene-count results, the tested-gene universe, a hypergeometric upper-tail test, odds ratio, and BH FDR.");
                if ui.button("Choose GMT…").on_hover_text("Select a GMT pathway/gene-set collection. Keeping the GMT external makes the pathway database/version explicit and reproducible.").clicked() {
                    let mut dialog=FileDialog::new().add_filter("GMT gene sets", &["gmt"]);
                    if let Some(root)=&this.cohort_root {dialog=dialog.set_directory(root);}
                    if let Some(path)=dialog.pick_file(){this.corrected_gmt_path=Some(path);}
                }
                if let Some(path)=&this.corrected_gmt_path {ui.monospace(path.file_name().and_then(|x|x.to_str()).unwrap_or("GMT"));}
            });
            if this.corrected_run_pathway {
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
                    ui.label(egui::RichText::new("Repertoire matched depth is the minimum usable library depth, capped by the value above to keep the native exact-without-replacement Monte-Carlo rarefaction responsive. The same depth is used for every included sample.").small().weak());
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
            ui.label(egui::RichText::new("Expression note: the native model deliberately does not claim to reproduce DESeq2/edgeR/limma empirical-Bayes machinery. It uses positive-count median-ratio normalization and a transparent NB2 GLM with per-feature unshrunk dispersion, optional covariates, subject-clustered robust inference, and a subject-collapsed sensitivity result.").small().weak());
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
                        };
                        match generate_corrected_html_report(&path,inputs) {
                            Ok(())=>this.cohort_status=format!("Generated corrected HTML report: {}",path.display()),
                            Err(e)=>this.cohort_status=format!("Could not generate corrected report: {e:#}"),
                        }
                    }
                }
            });
            ui.label(egui::RichText::new(format!(
                "Subject column: {} | covariates: {} | ambiguous repertoire calls: {}",
                bundle.spec.subject_column.as_deref().unwrap_or("none"),
                if bundle.spec.covariates.is_empty() { "none".to_string() } else { bundle.spec.covariates.join(", ") },
                bundle.spec.ambiguous_policy.display_name(),
            )).small().weak());
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

    fn render_corrected_expression(&self, ui: &mut egui::Ui, result: &CorrectedExpressionResult, id: &str) {
        ui.heading(format!("{} — corrected count analysis", result.feature_label));
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Model samples: {}",result.n_model_samples)); ui.separator();
            ui.label(format!("Subjects: {}",result.n_subjects)); ui.separator();
            ui.label(format!("Tested: {}",result.tested_features)); ui.separator();
            ui.label(format!("Filtered low-count: {}",result.filtered_features));
        });
        ui.label(egui::RichText::new(&result.normalization).small().weak());
        ui.label(egui::RichText::new(&result.model_description).small().weak());
        for warning in &result.warnings { ui.colored_label(egui::Color32::from_rgb(230,170,70),warning); }
        if let Some(ord)=&result.pca { self.render_corrected_ordination(ui,ord,&format!("{id}_pca")); }
        self.render_adjusted_test_table(ui,"Adjusted feature tests",&result.tests,560.0);
        if let Some(dtu)=&result.dtu {
            egui::CollapsingHeader::new(format!("Differential transcript usage ({} genes / {} transcripts)",dtu.genes_tested,dtu.transcripts_tested))
                .default_open(true)
                .show(ui,|ui|{
                    ui.label(egui::RichText::new(&dtu.method_description).small().weak());
                    ui.label(egui::RichText::new(format!("Gene-level DTU screen ({} rows)",dtu.gene_tests.len())).strong());
                    egui::ScrollArea::both().max_height(360.0).show(ui,|ui|{
                        egui::Grid::new(format!("{id}_dtu_genes")).striped(true).show(ui,|ui|{
                            for h in ["NCBI Gene ID","Transcripts","Simes p","BH q","Max |CLR β|"] {ui.label(egui::RichText::new(h).strong());} ui.end_row();
                            for r in &dtu.gene_tests {ui.monospace(&r.gene_id);ui.label(r.n_transcripts.to_string());ui.label(format!("{:.3e}",r.p_value));ui.label(format!("{:.3e}",r.q_value));ui.label(format!("{:.4}",r.max_abs_clr_effect));ui.end_row();}
                        });
                    });
                    self.render_adjusted_test_table(ui,"Transcript-level CLR usage tests",&dtu.transcript_tests,460.0);
                });
        }
    }

    fn render_corrected_repertoire(&self, ui: &mut egui::Ui, result: &CorrectedRepertoireResult, id: &str) {
        ui.heading(format!("{} — depth/composition corrected",result.modality));
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Samples: {}",result.files_used)); ui.separator();
            ui.label(format!("Matched depth: {} reads",result.matched_depth)); ui.separator();
            ui.label(format!("Rarefaction repeats: {}",result.rarefaction_reps)); ui.separator();
            ui.label(format!("Ambiguous calls: {}",result.ambiguous_policy.display_name()));
        });
        for warning in &result.warnings { ui.colored_label(egui::Color32::from_rgb(230,170,70),warning); }
        self.render_adjusted_test_table(ui,"Adjusted repertoire metrics",&result.metric_tests,420.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(ord)=&result.morisita_horn { ui.label(format!("Morisita-Horn PERMANOVA: F={:.3}, p={:.3e}",ord.permanova_f,ord.permanova_p)); }
            if let Some(ord)=&result.jaccard { ui.label(format!("Jaccard PERMANOVA: F={:.3}, p={:.3e}",ord.permanova_f,ord.permanova_p)); }
        });
        if let Some(ord)=&result.morisita_horn { self.render_corrected_ordination(ui,ord,&format!("{id}_mh")); }
        if let Some(ord)=&result.jaccard {
            egui::CollapsingHeader::new("Jaccard presence/absence PCoA").show(ui,|ui|self.render_corrected_ordination(ui,ord,&format!("{id}_jaccard")));
        }
        for (idx,comp) in result.compositions.iter().enumerate() {
            egui::CollapsingHeader::new(format!("{} CLR/Aitchison analysis ({} retained features)",comp.family,comp.retained_features))
                .default_open(idx==0)
                .show(ui,|ui|self.render_corrected_composition(ui,comp,&format!("{id}_comp_{idx}")));
        }
        egui::CollapsingHeader::new("Matched-depth & sharing sample table").show(ui,|ui|{
            egui::ScrollArea::both().max_height(430.0).show(ui,|ui|{
                egui::Grid::new(format!("{id}_samples")).striped(true).show(ui,|ui|{
                    for h in ["Sample","Group","Subject","Observed richness","Rarefied richness","Rarefied Shannon","Private","Longitudinal","Public"] { ui.label(egui::RichText::new(h).strong()); } ui.end_row();
                    for r in &result.sample_summaries { ui.monospace(&r.sample_id);ui.label(&r.group);ui.monospace(&r.subject);ui.label(format!("{:.1}",r.observed_richness));ui.label(format!("{:.1}",r.rarefied_richness));ui.label(format!("{:.3}",r.rarefied_shannon));ui.label(format!("{:.1}%",100.0*r.private_fraction));ui.label(format!("{:.1}%",100.0*r.longitudinal_shared_fraction));ui.label(format!("{:.1}%",100.0*r.public_fraction));ui.end_row(); }
                });
            });
        });
    }

    fn render_corrected_composition(&self, ui:&mut egui::Ui, result:&CorrectedCompositionResult, id:&str) {
        if let Some(ord)=&result.ordination {
            ui.label(format!("Global Aitchison PERMANOVA: F={:.3}, p={:.3e} ({} permutations)",ord.permanova_f,ord.permanova_p,ord.permutations));
            self.render_corrected_ordination(ui,ord,&format!("{id}_ord"));
        }
        self.render_adjusted_test_table(ui,&format!("{} adjusted CLR tests",result.family),&result.tests,500.0);
    }

    fn render_corrected_isotype(&self, ui:&mut egui::Ui, result:&CorrectedIsotypeResult, id:&str) {
        ui.heading("BCR isotype — compositional corrected analysis");
        ui.label(format!("Samples: {} | {}",result.files_used,if result.determined_only{"Undetermined excluded and composition renormalized"}else{"Original composition including Undetermined"}));
        for warning in &result.warnings { ui.colored_label(egui::Color32::from_rgb(230,170,70),warning); }
        if let Some(ord)=&result.ordination { self.render_corrected_ordination(ui,ord,&format!("{id}_ord")); }
        self.render_adjusted_test_table(ui,"Isotype CLR tests",&result.tests,480.0);
    }

    fn render_corrected_immune(&self, ui:&mut egui::Ui, result:&CorrectedImmuneResult, id:&str) {
        ui.heading("Immune infiltrate — adjusted ssGSEA analysis");
        ui.label(format!("Samples: {}. Tests use original ssGSEA scores; PCA uses per-cell-type z-scores.",result.files_used));
        for warning in &result.warnings { ui.colored_label(egui::Color32::from_rgb(230,170,70),warning); }
        if let Some(ord)=&result.pca { self.render_corrected_ordination(ui,ord,&format!("{id}_pca")); }
        self.render_adjusted_test_table(ui,"Adjusted immune-cell tests",&result.tests,520.0);
    }

    fn render_corrected_pathway(&self, ui:&mut egui::Ui, result:&CorrectedPathwayResult, id:&str) {
        ui.heading("Pathway over-representation analysis");
        ui.label(format!("GMT: {} | tested-gene universe: {} | foreground: {} | criteria: q ≤ {:.3}, |adjusted log2FC| ≥ {:.3}",result.gmt_path.display(),result.universe_size,result.foreground_size,result.q_threshold,result.effect_threshold));
        ui.label(egui::RichText::new(format!("Hypergeometric upper-tail test with BH correction; gene sets retained at {}-{} measured genes.",result.min_set_size,result.max_set_size)).small().weak());
        let has_points=result.tests.iter().any(|r|r.q_value.is_finite()&&r.q_value>0.0);
        if has_points{
            Plot::new(format!("{id}_plot")).height(340.0).x_axis_label("ln odds ratio").y_axis_label("-log10(BH q)").show(ui,|plot_ui|{
                for r in result.tests.iter().filter(|r|r.q_value.is_finite()&&r.q_value>0.0).take(100){
                    let label=format!("{}\nOverlap: {}/{}\nOR: {:.3}\np: {:.3e}\nq: {:.3e}\nGenes: {}",r.pathway,r.overlap,r.measured_set_size,r.odds_ratio,r.p_value,r.q_value,r.overlap_genes);
                    plot_ui.points(Points::new(label,vec![[r.odds_ratio.ln(),-r.q_value.max(1e-300).log10()]]).radius(4.0));
                }
            });
        }
        egui::ScrollArea::both().max_height(560.0).show(ui,|ui|{
            egui::Grid::new(format!("{id}_table")).striped(true).show(ui,|ui|{
                for h in ["Pathway","Set size","Overlap","Odds ratio","p","BH q","Overlap genes"]{ui.label(egui::RichText::new(h).strong());}ui.end_row();
                for r in &result.tests{ui.monospace(&r.pathway);ui.label(r.measured_set_size.to_string());ui.label(r.overlap.to_string());ui.label(format!("{:.3}",r.odds_ratio));ui.label(format!("{:.3e}",r.p_value));ui.label(format!("{:.3e}",r.q_value));ui.label(&r.overlap_genes);ui.end_row();}
            });
        });
    }

    fn render_corrected_ordination(&self, ui:&mut egui::Ui, ord:&OrdinationResult, id:&str) {
        ui.label(egui::RichText::new(&ord.name).strong());
        if ord.permanova_p.is_finite() { ui.label(format!("PERMANOVA: pseudo-F {:.3}, p {:.3e}, {} permutations",ord.permanova_f,ord.permanova_p,ord.permutations)); }
        let case_points=ord.points.iter().filter(|p|p.group=="Case").map(|p|[p.x,p.y]).collect::<Vec<_>>();
        let ctrl_points=ord.points.iter().filter(|p|p.group=="Control").map(|p|[p.x,p.y]).collect::<Vec<_>>();
        let accent=ui.visuals().selection.bg_fill;
        Plot::new(id).height(390.0).x_axis_label(&ord.x_label).y_axis_label(&ord.y_label)
            .label_formatter(|hover|match hover{HoverPosition::NearDataPoint{plot_name,..}if !plot_name.is_empty()=>Some(plot_name.to_string()),_=>None})
            .show(ui,|plot_ui|{
                if !ctrl_points.is_empty(){plot_ui.points(Points::new("Control",ctrl_points).color(egui::Color32::from_rgb(120,170,230)).radius(4.0));}
                if !case_points.is_empty(){plot_ui.points(Points::new("Case",case_points).color(accent).radius(4.0));}
                for p in ord.points.iter().take(500) {
                    let label=format!("{}\n{}\nSubject: {}\n{}: {:.4}\n{}: {:.4}",p.sample_id,p.group,p.subject,ord.x_label,p.x,ord.y_label,p.y);
                    plot_ui.points(Points::new(label,vec![[p.x,p.y]]).radius(2.0).allow_hover(true));
                }
            });
    }

    fn render_adjusted_test_table(&self, ui:&mut egui::Ui, title:&str, tests:&[AdjustedFeatureResult], height:f32) {
        ui.label(egui::RichText::new(format!("{title} ({} rows)",tests.len())).strong());
        ui.label(egui::RichText::new("β is the adjusted case-vs-control coefficient on the modeled scale. q is BH-adjusted for the robust model. Subject p/q comes from averaging repeated samples within each subject and applying a non-parametric sensitivity test.").small().weak());
        egui::ScrollArea::both().max_height(height).show(ui,|ui|{
            egui::Grid::new(format!("adjusted_table_{}", title.replace(' ', "_"))).striped(true).show(ui,|ui|{
                for h in ["Feature","ID/family","β group","SE","p","BH q","Subject effect","Subject q","n","subjects","case mean","ctrl mean"] { ui.label(egui::RichText::new(h).strong()); } ui.end_row();
                for r in tests {
                    ui.monospace(&r.feature);ui.monospace(&r.secondary_id);ui.label(format!("{:.4}",r.beta_group));ui.label(format!("{:.4}",r.robust_se));ui.label(format!("{:.3e}",r.p_value));ui.label(format!("{:.3e}",r.q_value));ui.label(format!("{:.4}",r.subject_effect));ui.label(format!("{:.3e}",r.subject_q_value));ui.label(r.n_samples.to_string());ui.label(r.n_subjects.to_string());ui.label(format!("{:.4}",r.mean_case));ui.label(format!("{:.4}",r.mean_control));ui.end_row();
                }
            });
        });
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
            ui.label(format!("Other/unmatched columns: {}", result.unmatched_columns));
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

        let all_points: Vec<[f64; 2]> = result
            .tests
            .iter()
            .filter(|r| r.effect.is_finite() && r.q_value.is_finite() && r.q_value > 0.0)
            .map(|r| [r.effect, -r.q_value.max(1e-300).log10()])
            .collect();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new(volcano_id)
            .height(360.0)
            .x_axis_label("log2 mean fold change (case / control)")
            .y_axis_label("-log10(BH q)")
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                if !all_points.is_empty() {
                    plot_ui.points(
                        Points::new(format!("All tested {feature_plural}"), all_points)
                            .color(egui::Color32::from_gray(100))
                            .radius(2.0),
                    );
                }
                for r in result.tests.iter().take(120) {
                    if !r.effect.is_finite() || !r.q_value.is_finite() || r.q_value <= 0.0 {
                        continue;
                    }
                    let label = format!(
                        "{}{}\nlog2FC: {:.3}\nCliff's δ: {:.3}\np: {:.3e}\nq: {:.3e}\nmedian case/control: {:.3} / {:.3}",
                        r.feature,
                        if r.secondary_id.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", r.secondary_id)
                        },
                        r.effect,
                        r.cliffs_delta,
                        r.p_value,
                        r.q_value,
                        r.median_case,
                        r.median_control
                    );
                    let color = if r.q_value <= 0.05 {
                        egui::Color32::from_rgb(230, 170, 70)
                    } else {
                        accent
                    };
                    plot_ui.points(
                        Points::new(label, vec![[r.effect, -r.q_value.max(1e-300).log10()]])
                            .color(color)
                            .radius(if r.q_value <= 0.05 { 4.0 } else { 3.0 }),
                    );
                }
            });
        ui.label(
            egui::RichText::new(
                format!("Hover the highlighted top-ranked {feature_plural} for effect size and significance. These are discovery candidates, not covariate-adjusted differential-expression results."),
            )
            .small()
            .weak(),
        );
        ui.collapsing("Detection shift (case vs control)", |ui| {
            self.render_expression_detection_shift(ui, result, feature_name, feature_plural, detection_id);
        });
        self.render_feature_test_table(ui, &format!("Top differential {feature_plural}"), &result.tests, 520.0);
        if feature_name == "Gene" {
            ui.label(
                egui::RichText::new(
                    "Pathway enrichment is not yet run in this pass. A future gene-analysis extension can accept a versioned GMT library for hit-list ORA and ranked enrichment.",
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
            ui.label(format!("Samples summarized: {}", result.sample_summaries.len()));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(format!("File warnings ({})", result.file_errors.len()), |ui| {
                for warning in &result.file_errors {
                    ui.label(warning);
                }
            });
        }
        ui.collapsing("Group distribution for a BCR metric", |ui| {
            self.render_bcr_metric_distribution(ui, result);
        });
        self.render_feature_effect_plot(ui, "cohort_bcr_metric_effects_v05", "BCR sample metrics", &result.metric_tests);
        self.render_feature_test_table(ui, "Sample-level BCR metrics", &result.metric_tests, 360.0);
        ui.collapsing("BCR heavy-chain isotype usage", |ui| {
            self.render_feature_test_table(ui, "Isotype fractions from BCR heavy-chain reports", &result.isotype_tests, 420.0);
        });
        ui.collapsing("V-gene usage", |ui| {
            self.render_feature_test_table(ui, "V-gene fraction tests", &result.v_gene_tests, 540.0);
        });
        ui.collapsing("D-gene usage", |ui| {
            self.render_feature_test_table(ui, "D-gene fraction tests", &result.d_gene_tests, 540.0);
        });
        ui.collapsing("J-gene usage", |ui| {
            self.render_feature_test_table(ui, "J-gene fraction tests", &result.j_gene_tests, 540.0);
        });
        ui.label(
            egui::RichText::new(
                "This first Rust BCR pass mirrors the simple comparison layer: Mann-Whitney U, BH adjustment within each feature family, and Cliff's delta. Public/private clonotypes, convergence, matched-depth richness, PCoA and dendrograms are intentionally left for the next repertoire-analysis iteration.",
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
            ui.collapsing(format!("File warnings ({})", result.file_errors.len()), |ui| {
                for warning in &result.file_errors {
                    ui.label(warning);
                }
            });
        }
        self.render_bcr_isotype_group_means_plot(ui, result);
        self.render_feature_effect_plot(
            ui,
            "cohort_bcr_isotype_effects_v05",
            "BCR isotype median-fraction shifts",
            &result.tests,
        );
        self.render_feature_test_table(ui, "BCR isotype fraction tests", &result.tests, 420.0);

        let mut case_means: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
        let mut ctrl_means: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
        let case_label = self
            .cohort_results
            .as_ref()
            .map(|b| b.comparison.case_label.as_str())
            .unwrap_or("case");
        for value in &result.values {
            if value.group == case_label {
                case_means.entry(value.isotype.clone()).or_default().push(value.fraction);
            } else {
                ctrl_means.entry(value.isotype.clone()).or_default().push(value.fraction);
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
            ui.label(format!("{} files used: {}", result.chain, result.files_used));
            ui.separator();
            ui.label(format!(
                "Files outside comparison/unmatched: {}",
                result.files_skipped_not_in_groups
            ));
            ui.separator();
            ui.label(format!("Samples summarized: {}", result.sample_summaries.len()));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(format!("File warnings ({})", result.file_errors.len()), |ui| {
                for warning in &result.file_errors {
                    ui.label(warning);
                }
            });
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
            self.render_feature_test_table(ui, "V-gene fraction tests", &result.v_gene_tests, 540.0);
        });
        ui.collapsing("J-gene usage", |ui| {
            self.render_feature_effect_plot(
                ui,
                &format!("{id_prefix}_j_gene_effects_v06"),
                "J-gene fraction shifts",
                &result.j_gene_tests,
            );
            self.render_feature_test_table(ui, "J-gene fraction tests", &result.j_gene_tests, 540.0);
        });
        ui.collapsing("V-J pair usage", |ui| {
            self.render_feature_effect_plot(
                ui,
                &format!("{id_prefix}_vj_pair_effects_v06"),
                "V-J pair fraction shifts",
                &result.vj_pair_tests,
            );
            self.render_feature_test_table(ui, "V-J pair fraction tests", &result.vj_pair_tests, 540.0);
        });
        ui.label(
            egui::RichText::new(
                "This initial TCR pass is an exploratory, depth-dependent comparison of repertoire summaries and V/J composition. Matched-depth rarefaction, Morisita-Horn similarity/PCoA, public/private clonotypes and amino-acid convergence remain separate advanced repertoire analyses for a later iteration.",
            )
            .small()
            .weak(),
        );
    }

    fn render_immune_comparison_result(&self, ui: &mut egui::Ui, result: &ImmuneComparisonResult) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Immune-infiltrate files used: {}", result.files_used));
            ui.separator();
            ui.label(format!(
                "Files outside comparison/unmatched: {}",
                result.files_skipped_not_in_groups
            ));
            ui.separator();
            ui.label(format!("Cell types tested: {}", result.tests.len()));
        });
        if !result.file_errors.is_empty() {
            ui.collapsing(format!("File warnings ({})", result.file_errors.len()), |ui| {
                for warning in &result.file_errors {
                    ui.label(warning);
                }
            });
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
        let Some(bundle) = &self.cohort_results else { return; };
        let case_label = bundle.comparison.case_label.clone();
        let control_label = bundle.comparison.control_label.clone();
        let mut case_values: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
        let mut ctrl_values: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
        for value in &result.values {
            if value.group == case_label {
                case_values.entry(value.cell_type.clone()).or_default().push(value.score);
            } else if value.group == control_label {
                ctrl_values.entry(value.cell_type.clone()).or_default().push(value.score);
            }
        }
        let features: Vec<String> = case_values
            .keys()
            .chain(ctrl_values.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if features.is_empty() { return; }
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
                        .name(format!("{} — {}: {}", feature, control_label, format_value(ctrl_mean)))
                        .width(0.32),
                );
            }
            if case_mean.is_finite() {
                case_bars.push(
                    Bar::new(idx as f64 + 0.18, case_mean)
                        .name(format!("{} — {}: {}", feature, case_label, format_value(case_mean)))
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
        let points: Vec<_> = result
            .tests
            .iter()
            .filter_map(|r| Some((r, r.detection_case?, r.detection_control?)))
            .filter(|(_, case, ctrl)| case.is_finite() && ctrl.is_finite())
            .collect();
        if points.is_empty() {
            ui.label(format!("Detection fractions are not available for this {} result.", feature_name.to_ascii_lowercase()));
            return;
        }
        let all_points: Vec<[f64; 2]> = points
            .iter()
            .map(|(_, case, ctrl)| [100.0 * *ctrl, 100.0 * *case])
            .collect();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new(plot_id)
            .height(330.0)
            .x_axis_label("Control detection (%)")
            .y_axis_label("Case detection (%)")
            .label_formatter(|hover| match hover {
                HoverPosition::NearDataPoint { plot_name, .. } if !plot_name.is_empty() => {
                    Some(plot_name.to_string())
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("Equal detection", vec![[0.0, 0.0], [100.0, 100.0]])
                        .color(egui::Color32::from_gray(120)),
                );
                plot_ui.points(
                    Points::new(format!("All {feature_plural}"), all_points)
                        .color(egui::Color32::from_gray(95))
                        .radius(2.0),
                );
                for (r, case, ctrl) in points.iter().take(120) {
                    let label = format!(
                        "{}\ncase detection: {:.1}%\ncontrol detection: {:.1}%\nΔ detection: {:+.1} pp\nq: {:.3e}",
                        r.feature,
                        100.0 * *case,
                        100.0 * *ctrl,
                        100.0 * (*case - *ctrl),
                        r.q_value,
                    );
                    let color = if r.q_value.is_finite() && r.q_value <= 0.05 {
                        egui::Color32::from_rgb(230, 170, 70)
                    } else {
                        accent
                    };
                    plot_ui.points(
                        Points::new(label, vec![[100.0 * *ctrl, 100.0 * *case]])
                            .color(color)
                            .radius(if r.q_value <= 0.05 { 4.0 } else { 3.0 }),
                    );
                }
            });
        ui.label(
            egui::RichText::new(
                format!("{feature_name}s above the diagonal are detected in a larger fraction of case samples; {feature_plural} below it are detected more often in controls."),
            )
            .small()
            .weak(),
        );
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
        let case_fill = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 90);
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
                                format!("{}\n{}: {}", sample, metric.display_name(), format_value(*value)),
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
        let Some(bundle) = &self.cohort_results else { return; };
        let case_label = bundle.comparison.case_label.clone();
        let control_label = bundle.comparison.control_label.clone();
        let mut case_values: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
        let mut ctrl_values: std::collections::BTreeMap<String, Vec<f64>> = std::collections::BTreeMap::new();
        for value in &result.values {
            if value.group == case_label {
                case_values.entry(value.isotype.clone()).or_default().push(value.fraction);
            } else if value.group == control_label {
                ctrl_values.entry(value.isotype.clone()).or_default().push(value.fraction);
            }
        }
        let features: Vec<String> = case_values
            .keys()
            .chain(ctrl_values.keys())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if features.is_empty() { return; }
        let mut case_bars = Vec::new();
        let mut ctrl_bars = Vec::new();
        for (idx, feature) in features.iter().enumerate() {
            let cm = case_values.get(feature).map(|v| v.iter().sum::<f64>() / v.len() as f64).unwrap_or(0.0);
            let hm = ctrl_values.get(feature).map(|v| v.iter().sum::<f64>() / v.len() as f64).unwrap_or(0.0);
            case_bars.push(Bar::new(idx as f64 + 0.18, cm).name(format!("{} — {}: {:.4}", feature, case_label, cm)).width(0.32));
            ctrl_bars.push(Bar::new(idx as f64 - 0.18, hm).name(format!("{} — {}: {:.4}", feature, control_label, hm)).width(0.32));
        }
        let axis_features = features.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("cohort_bcr_isotype_group_means_v05")
            .height(320.0)
            .x_axis_label("Isotype")
            .y_axis_label("Mean isotype fraction")
            .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_features))
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new(control_label.clone(), ctrl_bars).color(egui::Color32::from_gray(145)));
                plot_ui.bar_chart(BarChart::new(case_label.clone(), case_bars).color(accent));
            });
        ui.label(egui::RichText::new("Group means from the dedicated BCR isotype-composition files.").small().weak());
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
            ui.label(egui::RichText::new(format!("({} rows)", tests.len())).small().weak());
        });

        if tests.is_empty() {
            ui.label(egui::RichText::new("No rows available for this feature family.").weak());
            return;
        }

        // Reserve a real viewport for larger result families. Without this, nested
        // scroll areas can collapse to only a few visible rows near the bottom of
        // the outer Cohort Analysis page even when the table contains many tests.
        let row_height = 24.0_f32;
        let content_height = row_height * (tests.len().saturating_add(1) as f32) + 10.0;
        let desired_height = if tests.len() > 8 {
            max_height
        } else {
            content_height.min(max_height)
        };

        ui.allocate_ui(egui::vec2(ui.available_width(), desired_height), |ui| {
            egui::ScrollArea::vertical()
                .id_salt(format!("feature_test_scroll_{title}"))
                .max_height(desired_height)
                .show_rows(ui, row_height, tests.len(), |ui, range| {
                    egui::Grid::new(format!("feature_test_grid_{title}"))
                        .striped(true)
                        .num_columns(9)
                        .show(ui, |ui| {
                            if range.start == 0 {
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
                            }
                            for idx in range {
                                let r = &tests[idx];
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
                });
        });
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
            write_feature_tests(&outdir.join("transcript_expression_tests.tsv"), &transcript.tests)?;
        }
        if let Some(bcr) = &bundle.bcr {
            write_bcr_sample_summaries(
                &outdir.join("bcr_sample_summaries.tsv"),
                &bcr.sample_summaries,
            )?;
            write_feature_tests(&outdir.join("bcr_sample_metric_tests.tsv"), &bcr.metric_tests)?;
            write_feature_tests(
                &outdir.join("bcr_heavy_isotype_fraction_tests.tsv"),
                &bcr.isotype_tests,
            )?;
            write_feature_tests(&outdir.join("bcr_v_gene_fraction_tests.tsv"), &bcr.v_gene_tests)?;
            write_feature_tests(&outdir.join("bcr_d_gene_fraction_tests.tsv"), &bcr.d_gene_tests)?;
            write_feature_tests(&outdir.join("bcr_j_gene_fraction_tests.tsv"), &bcr.j_gene_tests)?;
            if !bcr.file_errors.is_empty() {
                fs::write(outdir.join("bcr_file_warnings.txt"), bcr.file_errors.join("\n"))?;
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
            write_tcr_sample_summaries(&outdir.join("tcr_alpha_sample_summaries.tsv"), &tcr.sample_summaries)?;
            write_feature_tests(&outdir.join("tcr_alpha_sample_metric_tests.tsv"), &tcr.metric_tests)?;
            write_feature_tests(&outdir.join("tcr_alpha_v_gene_fraction_tests.tsv"), &tcr.v_gene_tests)?;
            write_feature_tests(&outdir.join("tcr_alpha_j_gene_fraction_tests.tsv"), &tcr.j_gene_tests)?;
            write_feature_tests(&outdir.join("tcr_alpha_vj_pair_fraction_tests.tsv"), &tcr.vj_pair_tests)?;
            if !tcr.file_errors.is_empty() {
                fs::write(outdir.join("tcr_alpha_file_warnings.txt"), tcr.file_errors.join("\n"))?;
            }
        }
        if let Some(tcr) = &bundle.tcr_beta {
            write_tcr_sample_summaries(&outdir.join("tcr_beta_sample_summaries.tsv"), &tcr.sample_summaries)?;
            write_feature_tests(&outdir.join("tcr_beta_sample_metric_tests.tsv"), &tcr.metric_tests)?;
            write_feature_tests(&outdir.join("tcr_beta_v_gene_fraction_tests.tsv"), &tcr.v_gene_tests)?;
            write_feature_tests(&outdir.join("tcr_beta_j_gene_fraction_tests.tsv"), &tcr.j_gene_tests)?;
            write_feature_tests(&outdir.join("tcr_beta_vj_pair_fraction_tests.tsv"), &tcr.vj_pair_tests)?;
            if !tcr.file_errors.is_empty() {
                fs::write(outdir.join("tcr_beta_file_warnings.txt"), tcr.file_errors.join("\n"))?;
            }
        }
        if let Some(immune) = &bundle.immune {
            write_immune_values(&outdir.join("immune_infiltrate_sample_values.tsv"), &immune.values)?;
            write_feature_tests(&outdir.join("immune_infiltrate_tests.tsv"), &immune.tests)?;
            if !immune.file_errors.is_empty() {
                fs::write(
                    outdir.join("immune_infiltrate_file_warnings.txt"),
                    immune.file_errors.join("\n"),
                )?;
            }
        }
        let notes = format!(
            "Exploratory comparison: {}\nCase: {}\nControl: {}\n\nThis first-pass analysis intentionally uses all available mapped samples, including longitudinal samples, without repeated-measures or covariate adjustment.\n",
            bundle.comparison.name,
            bundle.comparison.case_label,
            bundle.comparison.control_label,
        );
        fs::write(outdir.join("README.txt"), notes)?;
        Ok(outdir)
    }

    fn save_corrected_result_tables(&self, parent: &Path) -> anyhow::Result<PathBuf> {
        let bundle = self.corrected_results.as_ref().ok_or_else(|| anyhow::anyhow!("No corrected results are available"))?;
        let safe_name: String = bundle.comparison.name.chars().map(|c| if c.is_ascii_alphanumeric(){c}else{'_'}).collect();
        let outdir = parent.join(format!("4c_corrected_analysis_{safe_name}"));
        fs::create_dir_all(&outdir)?;
        if let Some(mapping)=&self.cohort_mapping { write_mapping_audit(&outdir.join("sample_mapping_audit.tsv"),mapping)?; write_mapping_audit_csv(&outdir.join("sample_mapping_audit.csv"),mapping)?; }
        if let Some(r)=&bundle.gene { write_adjusted_tests(&outdir.join("gene_adjusted_tests.tsv"),&r.tests)?; if let Some(o)=&r.pca{write_ordination(&outdir.join("gene_pca.tsv"),o)?;} }
        if let Some(r)=&bundle.transcript {
            write_adjusted_tests(&outdir.join("transcript_adjusted_tests.tsv"),&r.tests)?;
            if let Some(o)=&r.pca{write_ordination(&outdir.join("transcript_pca.tsv"),o)?;}
            if let Some(dtu)=&r.dtu {
                write_adjusted_tests(&outdir.join("transcript_dtu_transcript_tests.tsv"),&dtu.transcript_tests)?;
                write_dtu_gene_tests(&outdir.join("transcript_dtu_gene_tests.tsv"),&dtu.gene_tests)?;
            }
        }
        for (prefix,r) in [("bcr",bundle.bcr.as_ref()),("tcr_alpha",bundle.tcr_alpha.as_ref()),("tcr_beta",bundle.tcr_beta.as_ref())] {
            if let Some(r)=r {
                write_corrected_repertoire_samples(&outdir.join(format!("{prefix}_matched_depth_sample_metrics.tsv")),&r.sample_summaries)?;
                write_adjusted_tests(&outdir.join(format!("{prefix}_adjusted_metric_tests.tsv")),&r.metric_tests)?;
                if let Some(o)=&r.morisita_horn{write_ordination(&outdir.join(format!("{prefix}_morisita_horn_pcoa.tsv")),o)?;}
                if let Some(o)=&r.jaccard{write_ordination(&outdir.join(format!("{prefix}_jaccard_pcoa.tsv")),o)?;}
                for comp in &r.compositions {
                    let fam:String=comp.family.chars().map(|c|if c.is_ascii_alphanumeric(){c.to_ascii_lowercase()}else{'_'}).collect();
                    write_adjusted_tests(&outdir.join(format!("{prefix}_{fam}_clr_tests.tsv")),&comp.tests)?;
                    if let Some(o)=&comp.ordination{write_ordination(&outdir.join(format!("{prefix}_{fam}_aitchison_pcoa.tsv")),o)?;}
                }
                if !r.warnings.is_empty(){fs::write(outdir.join(format!("{prefix}_warnings.txt")),r.warnings.join("\n"))?;}
            }
        }
        if let Some(r)=&bundle.bcr_isotype { write_adjusted_tests(&outdir.join("bcr_isotype_clr_tests.tsv"),&r.tests)?; if let Some(o)=&r.ordination{write_ordination(&outdir.join("bcr_isotype_aitchison_pcoa.tsv"),o)?;} if !r.warnings.is_empty(){fs::write(outdir.join("bcr_isotype_warnings.txt"),r.warnings.join("\n"))?;} }
        if let Some(r)=&bundle.immune { write_adjusted_tests(&outdir.join("immune_adjusted_tests.tsv"),&r.tests)?; if let Some(o)=&r.pca{write_ordination(&outdir.join("immune_pca.tsv"),o)?;} if !r.warnings.is_empty(){fs::write(outdir.join("immune_warnings.txt"),r.warnings.join("\n"))?;} }
        if let Some(r)=&bundle.pathway { write_pathway_ora(&outdir.join("pathway_ora.tsv"),r)?; }
        let notes=format!("4C Cohort Explorer corrected/standard analysis\nComparison: {}\nCase: {}\nControl: {}\nSubject column: {}\nCovariates: {}\n\nExpression uses positive-count median-ratio normalization followed by a native NB2 GLM with a log(size-factor) offset and per-feature unshrunk dispersion. Subject-clustered sandwich SEs are used when a subject column is configured, and a subject-collapsed non-parametric sensitivity result is also reported. This is intentionally not labeled DESeq2/edgeR/limma-voom because their cross-feature empirical-Bayes/dispersion-shrinkage machinery is not reproduced.\n\nRepertoire analyses use matched-depth exact-without-replacement Monte-Carlo rarefaction, subject-aware public/private sharing, Morisita-Horn and Jaccard distances, PCoA/PERMANOVA, and CLR/Aitchison composition analysis.\n",
            bundle.comparison.name,bundle.comparison.case_label,bundle.comparison.control_label,bundle.spec.subject_column.as_deref().unwrap_or("none"),if bundle.spec.covariates.is_empty(){"none".to_string()}else{bundle.spec.covariates.join(", ")});
        fs::write(outdir.join("README.txt"),notes)?;
        Ok(outdir)
    }

    fn configure_corrected_metadata_defaults(&mut self) {
        let Some(meta)=&self.cohort_metadata else {return;};
        let columns=meta.columns.clone();
        if self.corrected_subject_column.as_ref().is_none_or(|c|!columns.contains(c)) {
            self.corrected_subject_column=columns.iter().find(|c|{
                let l=c.to_ascii_lowercase();
                l=="subject"||l=="subject_id"||l=="patient"||l=="patient_id"||l=="participant"||l=="participant_id"||l.contains("patient id")||l.contains("subject id")
            }).cloned();
        }
        self.corrected_covariates.retain(|c|columns.contains(c));
        if self.corrected_covariates.is_empty() {
            for c in &columns {
                let l=c.to_ascii_lowercase();
                if l=="age"||l.contains("age_at")||l=="sex"||l=="gender"||l.contains("ethnic")||l=="race"||l.contains("batch") {
                    if Some(c)!=self.corrected_subject_column.as_ref() && c!=&meta.id_column { self.corrected_covariates.insert(c.clone()); }
                }
            }
        }
    }

    fn start_corrected_analysis(&mut self) {
        let (Some(scan),Some(metadata))=(self.cohort_scan.clone(),self.cohort_metadata.clone()) else {self.cohort_status="Load a project and metadata first.".to_string();return;};
        let comparison=match build_comparison(&metadata,self.cohort_preset){Ok(c)=>c,Err(e)=>{self.cohort_status=format!("Could not build comparison: {e:#}");return;}};
        let mut covariates=self.corrected_covariates.iter().cloned().collect::<Vec<_>>();covariates.sort();
        let spec=CorrectedModelSpec{subject_column:self.corrected_subject_column.clone(),covariates,permutations:self.corrected_permutations,rarefaction_reps:self.corrected_rarefaction_reps,rarefaction_cap:self.corrected_rarefaction_cap,ambiguous_policy:self.corrected_ambiguous_policy,exclude_undetermined_isotype:self.corrected_exclude_undetermined,..CorrectedModelSpec::default()};
        let run_pathway=self.corrected_run_pathway;
        let run_gene=self.cohort_run_gene || run_pathway;let run_transcript=self.cohort_run_transcript;let run_bcr=self.cohort_run_bcr;let run_iso=self.cohort_run_bcr_isotype;let run_tcra=self.cohort_run_tcr_alpha;let run_tcrb=self.cohort_run_tcr_beta;let run_immune=self.cohort_run_immune;
        let gmt_path=self.corrected_gmt_path.clone();let pathway_q=self.corrected_pathway_q;let pathway_effect=self.corrected_pathway_effect;
        let stage_count=[run_gene,run_transcript,run_bcr,run_iso,run_tcra,run_tcrb,run_immune,run_pathway].into_iter().filter(|x|*x).count().max(1)as f32;
        let(tx,rx)=mpsc::channel();self.cohort_receiver=Some(rx);self.cohort_running=true;self.cohort_progress=0.0;self.cohort_status=format!("Running corrected {}…",comparison.name);self.corrected_results=None;
        std::thread::spawn(move||{
            let mut stage=0usize;let weight=1.0/stage_count;
            let(mut gene,mut gene_error,mut transcript,mut transcript_error,mut bcr,mut bcr_error,mut iso,mut iso_error,mut tcra,mut tcra_error,mut tcrb,mut tcrb_error,mut immune,mut immune_error,mut pathway,mut pathway_error)=(None,None,None,None,None,None,None,None,None,None,None,None,None,None,None,None);
            macro_rules! prog {($base:expr,$prefix:expr)=>{|done,total,msg|{let frac=if total==0{0.0}else{done as f32/total as f32};let _=tx.send(CohortWorkerMessage::Progress($base+weight*frac,format!("{} — {}",$prefix,msg)));}}}
            if run_gene {let base=stage as f32*weight;if let Some(path)=scan.merged_gene_tsv.clone(){match run_corrected_expression(&path,&metadata,&comparison,"Raw Count","Gene",&spec,prog!(base,"Gene corrected")){Ok(r)=>gene=Some(r),Err(e)=>gene_error=Some(format!("{e:#}"))}}else{gene_error=Some("No merged gene-expression TSV detected.".to_string());}stage+=1;}
            if run_transcript {let base=stage as f32*weight;if let Some(path)=scan.merged_transcript_tsv.clone(){match run_corrected_expression(&path,&metadata,&comparison,"Total Reads","Transcript",&spec,prog!(base,"Transcript corrected")){Ok(r)=>transcript=Some(r),Err(e)=>transcript_error=Some(format!("{e:#}"))}}else{transcript_error=Some("No merged transcript-expression TSV detected.".to_string());}stage+=1;}
            if run_bcr {let base=stage as f32*weight;match run_corrected_repertoire(&scan.files,&metadata,&comparison,CohortModality::Bcr,&spec,prog!(base,"BCR corrected")){Ok(r)=>bcr=Some(r),Err(e)=>bcr_error=Some(format!("{e:#}"))}stage+=1;}
            if run_iso {let base=stage as f32*weight;match run_corrected_isotype(&scan.files,&metadata,&comparison,&spec,prog!(base,"BCR isotype corrected")){Ok(r)=>iso=Some(r),Err(e)=>iso_error=Some(format!("{e:#}"))}stage+=1;}
            if run_tcra {let base=stage as f32*weight;match run_corrected_repertoire(&scan.files,&metadata,&comparison,CohortModality::TcrAlpha,&spec,prog!(base,"TCR alpha corrected")){Ok(r)=>tcra=Some(r),Err(e)=>tcra_error=Some(format!("{e:#}"))}stage+=1;}
            if run_tcrb {let base=stage as f32*weight;match run_corrected_repertoire(&scan.files,&metadata,&comparison,CohortModality::TcrBeta,&spec,prog!(base,"TCR beta corrected")){Ok(r)=>tcrb=Some(r),Err(e)=>tcrb_error=Some(format!("{e:#}"))}stage+=1;}
            if run_immune {let base=stage as f32*weight;match run_corrected_immune(&scan.files,&metadata,&comparison,&spec,prog!(base,"Immune corrected")){Ok(r)=>immune=Some(r),Err(e)=>immune_error=Some(format!("{e:#}"))}stage+=1;}
            if run_pathway {
                let _=tx.send(CohortWorkerMessage::Progress(stage as f32*weight,"Pathway ORA".to_string()));
                match (gene.as_ref(),gmt_path.as_ref()) {
                    (Some(g),Some(path))=>match run_pathway_ora(path,g,pathway_q,pathway_effect){Ok(r)=>pathway=Some(r),Err(e)=>pathway_error=Some(format!("{e:#}"))},
                    (None,_)=>pathway_error=Some("Pathway ORA requires a successful corrected gene-count analysis.".to_string()),
                    (_,None)=>pathway_error=Some("Choose a GMT gene-set file before running pathway ORA.".to_string()),
                }
            }
            let _=tx.send(CohortWorkerMessage::CorrectedDone(CorrectedAnalysisBundle{comparison,spec,gene,gene_error,transcript,transcript_error,bcr,bcr_error,bcr_isotype:iso,bcr_isotype_error:iso_error,tcr_alpha:tcra,tcr_alpha_error:tcra_error,tcr_beta:tcrb,tcr_beta_error:tcrb_error,immune,immune_error,pathway,pathway_error}));
        });
    }

    fn load_cohort_project(&mut self, root: PathBuf) {
        match scan_project(&root) {
            Ok(scan) => {
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
                if let Some(path) = auto_meta {
                    self.load_cohort_metadata_path(path);
                } else {
                    self.cohort_metadata = None;
                    self.cohort_metadata_path = None;
                    self.cohort_mapping = None;
                }
            }
            Err(e) => {
                self.cohort_status = format!("Project scan failed: {e:#}");
            }
        }
    }

    fn load_cohort_metadata_path(&mut self, path: PathBuf) {
        match load_metadata(&path) {
            Ok(meta) => {
                let rows = meta.rows.len();
                let id_column = meta.id_column.clone();
                self.cohort_metadata_path = Some(path);
                self.cohort_metadata = Some(meta);
                self.cohort_results = None;
                self.corrected_results = None;
                self.configure_corrected_metadata_defaults();
                self.rebuild_cohort_mapping();
                self.cohort_status = format!(
                    "Metadata loaded: {rows} rows; matching on '{id_column}'."
                );
            }
            Err(e) => {
                self.cohort_status = format!("Metadata load failed: {e:#}");
            }
        }
    }

    fn rebuild_cohort_mapping(&mut self) {
        self.cohort_mapping = match (&self.cohort_scan, &self.cohort_metadata) {
            (Some(scan), Some(meta)) => Some(build_mapping(scan, meta)),
            _ => None,
        };
    }

    fn start_cohort_analysis(&mut self) {
        let (Some(scan), Some(metadata)) = (self.cohort_scan.clone(), self.cohort_metadata.clone()) else {
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
                    match run_gene_comparison(&path, &comparison, &gene_metric, |done, total, msg| {
                        let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                        let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, format!("Gene expression — {msg}")));
                    }) {
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
                    match run_transcript_comparison(&path, &comparison, &transcript_metric, |done, total, msg| {
                        let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                        let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, format!("Transcript expression — {msg}")));
                    }) {
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
                    let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, msg.to_string()));
                }) {
                    Ok(result) => bcr = Some(result),
                    Err(e) => bcr_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_iso {
                let base = stage_index as f32 * stage_weight;
                match run_bcr_isotype_comparison(&scan.files, &comparison, |done, total, msg| {
                    let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, msg.to_string()));
                }) {
                    Ok(result) => bcr_isotype = Some(result),
                    Err(e) => bcr_isotype_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_tcr_alpha {
                let base = stage_index as f32 * stage_weight;
                match run_tcr_comparison(&scan.files, &comparison, CohortModality::TcrAlpha, |done, total, msg| {
                    let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, msg.to_string()));
                }) {
                    Ok(result) => tcr_alpha = Some(result),
                    Err(e) => tcr_alpha_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_tcr_beta {
                let base = stage_index as f32 * stage_weight;
                match run_tcr_comparison(&scan.files, &comparison, CohortModality::TcrBeta, |done, total, msg| {
                    let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, msg.to_string()));
                }) {
                    Ok(result) => tcr_beta = Some(result),
                    Err(e) => tcr_beta_error = Some(format!("{e:#}")),
                }
                stage_index += 1;
            }

            if run_immune {
                let base = stage_index as f32 * stage_weight;
                match run_immune_infiltrate_comparison(&scan.files, &comparison, |done, total, msg| {
                    let frac = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    let _ = tx.send(CohortWorkerMessage::Progress(base + stage_weight * frac, msg.to_string()));
                }) {
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
        if let Some(rx) = &self.cohort_receiver {
            while let Ok(message) = rx.try_recv() {
                match message {
                    CohortWorkerMessage::Progress(frac, text) => {
                        self.cohort_progress = frac.clamp(0.0, 1.0);
                        self.cohort_status = text;
                    }
                    CohortWorkerMessage::Done(bundle) => exploratory_done = Some(bundle),
                    CohortWorkerMessage::CorrectedDone(bundle) => corrected_done = Some(bundle),
                }
            }
        }
        if let Some(bundle) = exploratory_done {
            let mut messages = Vec::new();
            if let Some(err) = &bundle.gene_error { messages.push(format!("Gene: {err}")); }
            if let Some(err) = &bundle.transcript_error { messages.push(format!("Transcript: {err}")); }
            if let Some(err) = &bundle.bcr_error { messages.push(format!("BCR: {err}")); }
            if let Some(err) = &bundle.bcr_isotype_error { messages.push(format!("BCR isotype: {err}")); }
            if let Some(err) = &bundle.tcr_alpha_error { messages.push(format!("TCR alpha: {err}")); }
            if let Some(err) = &bundle.tcr_beta_error { messages.push(format!("TCR beta: {err}")); }
            if let Some(err) = &bundle.immune_error { messages.push(format!("Immune infiltrate: {err}")); }
            let current_available = match self.cohort_result_view {
                CohortResultView::Gene => bundle.gene.is_some(), CohortResultView::Transcript => bundle.transcript.is_some(),
                CohortResultView::Bcr => bundle.bcr.is_some(), CohortResultView::BcrIsotype => bundle.bcr_isotype.is_some(),
                CohortResultView::TcrAlpha => bundle.tcr_alpha.is_some(), CohortResultView::TcrBeta => bundle.tcr_beta.is_some(),
                CohortResultView::Immune => bundle.immune.is_some(),
            };
            if !current_available {
                self.cohort_result_view = if bundle.gene.is_some(){CohortResultView::Gene}else if bundle.transcript.is_some(){CohortResultView::Transcript}else if bundle.bcr.is_some(){CohortResultView::Bcr}else if bundle.bcr_isotype.is_some(){CohortResultView::BcrIsotype}else if bundle.tcr_alpha.is_some(){CohortResultView::TcrAlpha}else if bundle.tcr_beta.is_some(){CohortResultView::TcrBeta}else{CohortResultView::Immune};
            }
            self.cohort_results=Some(bundle); self.cohort_running=false; self.cohort_progress=1.0; self.cohort_receiver=None;
            self.cohort_status=if messages.is_empty(){"Exploratory comparison complete.".to_string()}else{format!("Comparison completed with {} modality warning(s): {}",messages.len(),messages.join(" | "))};
        }
        if let Some(bundle) = corrected_done {
            let mut messages=Vec::new();
            if let Some(err)=&bundle.gene_error{messages.push(format!("Gene: {err}"));}
            if let Some(err)=&bundle.transcript_error{messages.push(format!("Transcript: {err}"));}
            if let Some(err)=&bundle.bcr_error{messages.push(format!("BCR: {err}"));}
            if let Some(err)=&bundle.bcr_isotype_error{messages.push(format!("BCR isotype: {err}"));}
            if let Some(err)=&bundle.tcr_alpha_error{messages.push(format!("TCR alpha: {err}"));}
            if let Some(err)=&bundle.tcr_beta_error{messages.push(format!("TCR beta: {err}"));}
            if let Some(err)=&bundle.immune_error{messages.push(format!("Immune: {err}"));}
            if let Some(err)=&bundle.pathway_error{messages.push(format!("Pathway: {err}"));}
            let current=match self.corrected_result_view{CorrectedResultView::Gene=>bundle.gene.is_some(),CorrectedResultView::Transcript=>bundle.transcript.is_some(),CorrectedResultView::Bcr=>bundle.bcr.is_some(),CorrectedResultView::BcrIsotype=>bundle.bcr_isotype.is_some(),CorrectedResultView::TcrAlpha=>bundle.tcr_alpha.is_some(),CorrectedResultView::TcrBeta=>bundle.tcr_beta.is_some(),CorrectedResultView::Immune=>bundle.immune.is_some(),CorrectedResultView::Pathway=>bundle.pathway.is_some()};
            if !current {self.corrected_result_view=if bundle.gene.is_some(){CorrectedResultView::Gene}else if bundle.transcript.is_some(){CorrectedResultView::Transcript}else if bundle.bcr.is_some(){CorrectedResultView::Bcr}else if bundle.bcr_isotype.is_some(){CorrectedResultView::BcrIsotype}else if bundle.tcr_alpha.is_some(){CorrectedResultView::TcrAlpha}else if bundle.tcr_beta.is_some(){CorrectedResultView::TcrBeta}else if bundle.immune.is_some(){CorrectedResultView::Immune}else{CorrectedResultView::Pathway};}
            self.corrected_results=Some(bundle);self.cohort_running=false;self.cohort_progress=1.0;self.cohort_receiver=None;
            self.cohort_status=if messages.is_empty(){"Corrected / standard analysis complete.".to_string()}else{format!("Corrected analysis completed with {} modality warning(s): {}",messages.len(),messages.join(" | "))};
        }
    }

    fn card(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        add_contents: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(title).size(17.0).strong());
                ui.add_space(4.0);
                add_contents(self, ui);
            });
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
                self.analysis_status = format!(
                    "Merged TSV ready for QC: {sample_count} samples ({kind:?})."
                );
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
                self.status = "Supported inputs are .xlsx workbooks, folders, and merged .tsv files.".to_string();
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
        if self.files.is_empty() || metrics.is_empty() {
            return "Select at least one input workbook and measurement.".to_string();
        }

        let mut columns: Vec<String> = match kind {
            DataKind::Gene => vec!["Gene Symbol".to_string(), "NCBI Gene ID".to_string()],
            DataKind::Transcript => {
                vec!["Transcript Name".to_string(), "NCBI Gene ID".to_string()]
            }
            DataKind::Auto => vec!["Identifier".to_string(), "NCBI Gene ID".to_string()],
        };

        let sample_names: Vec<String> = self.files.iter().map(|p| sample_name(p)).collect();
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
        let config = MergeConfig {
            files: self.files.clone(),
            output,
            kind: self.kind,
            metrics: self.selected_metrics(),
            column_order: self.order,
        };

        let (tx, rx) = mpsc::channel();
        self.merge_receiver = Some(rx);
        self.running = true;
        self.progress = 0.0;
        self.status = "Starting merge…".to_string();

        std::thread::spawn(move || {
            let tx_progress = tx.clone();
            let result = merge_wide(config, move |done, total, msg| {
                let _ = tx_progress.send(MergeWorkerMessage::Progress(
                    done,
                    total,
                    msg.to_string(),
                ));
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
                    let result = load_entity_catalog_from_merged(&path)
                        .map_err(|e| format!("{e:#}"));
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
                            self.analysis_status = format!("Search index ready: {n} identifiers from {source}.");
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
                            self.analysis_status = format!("Profile ready: value found in {n} samples.");
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

fn metric_help(metric: Metric) -> &'static str {
    match metric {
        Metric::RawCount => "Raw RNA-seq gene counts from the source report.",
        Metric::Fpkm => "Fragments Per Kilobase of transcript per Million mapped reads.",
        Metric::Cpm => "Counts Per Million mapped reads.",
        Metric::Tpm => "Transcripts Per Million; commonly useful for comparing expression abundance within a sample.",
        Metric::PercentileRank => "Expression percentile rank supplied by the source report.",
        Metric::IsExpressed => "Categorical yes/no expression call supplied by the source report.",
        Metric::TranscriptLength => "Reported transcript length.",
        Metric::EffectiveTranscriptLength => "Effective transcript length used for abundance estimation.",
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
    Some(BoxSpread::new(
        lower_whisker,
        q1,
        median,
        q3,
        upper_whisker,
    ))
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
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|base| {
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
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|base| {
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

