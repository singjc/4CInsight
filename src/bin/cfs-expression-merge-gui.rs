use cfs_expression_merger::{
    all_metrics, collaborator_default_metrics, inspect_kind, inspect_merged_tsv, load_entity_catalog,
    load_entity_catalog_from_merged, load_entity_profile, load_entity_profile_from_merged,
    merge_wide, sample_name, summarize_merged_distributions, summarize_sample_distributions,
    ColumnOrder, DataKind, EntityCatalogEntry, EntityProfilePoint, MergeConfig, MergeSummary,
    MergedDatasetInfo, Metric, SampleDistributionSummary,
};
use eframe::egui;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, HoverPosition, Line, Plot, Points};
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
        "CFS Expression Merger",
        options,
        Box::new(|cc| Ok(Box::new(MergeApp::new(cc)))),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppTab {
    Merge,
    Explore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisSource {
    Workbooks,
    MergedTsv,
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
        }
    }
}

impl eframe::App for MergeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_merge_worker();
        self.poll_analysis_worker();
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
                });
        });

        if self.running || self.analysis_running {
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
                            egui::RichText::new("CFS Expression Merger")
                                .size(24.0)
                                .strong()
                                .color(accent),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Memory-bounded RNA expression merging, preview, QC, and exploration",
                            )
                            .weak(),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let kind = if self.active_tab == AppTab::Explore {
                            self.analysis_kind()
                        } else {
                            self.effective_kind()
                        };
                        let kind_text = match kind {
                            DataKind::Gene => "Gene data",
                            DataKind::Transcript => "Transcript data",
                            DataKind::Auto => "Report type unknown",
                        };
                        let sample_count = if self.active_tab == AppTab::Explore {
                            self.analysis_sample_count()
                        } else {
                            self.files.len()
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
        self.render_distribution_card(ui);
        ui.add_space(10.0);
        self.render_entity_profile_card(ui);
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
                    egui::ScrollArea::vertical()
                        .id_salt("entity_browser_scroll_v03")
                        .max_height(390.0)
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

