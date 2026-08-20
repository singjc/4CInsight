use cfs_expression_merger::{
    all_metrics, collaborator_default_metrics, inspect_kind, load_entity_catalog,
    load_entity_profile, merge_wide, sample_name, summarize_sample_distributions, ColumnOrder,
    DataKind, EntityCatalogEntry, EntityProfilePoint, MergeConfig, MergeSummary, Metric,
    SampleDistributionSummary,
};
use eframe::egui;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, HoverPosition, Line, Plot, Points};
use rfd::FileDialog;
use std::path::PathBuf;
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
    detected_kind: DataKind,
    kind: DataKind,
    metrics: Vec<Metric>,
    order: ColumnOrder,
    output: Option<PathBuf>,

    status: String,
    progress: f32,
    running: bool,
    merge_receiver: Option<Receiver<MergeWorkerMessage>>,

    analysis_metric: Metric,
    analysis_ignore_zeros: bool,
    analysis_log1p: bool,
    analysis_status: String,
    analysis_progress: f32,
    analysis_running: bool,
    analysis_receiver: Option<Receiver<AnalysisWorkerMessage>>,
    entity_catalog: Vec<EntityCatalogEntry>,
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
            detected_kind: DataKind::Auto,
            kind: DataKind::Auto,
            metrics: Vec::new(),
            order: ColumnOrder::AdjacentBySample,
            output: None,
            status: "Choose a folder or a set of Excel files.".to_string(),
            progress: 0.0,
            running: false,
            merge_receiver: None,
            analysis_metric: Metric::Tpm,
            analysis_ignore_zeros: false,
            analysis_log1p: true,
            analysis_status: "Load some expression workbooks to enable QC and plotting.".to_string(),
            analysis_progress: 0.0,
            analysis_running: false,
            analysis_receiver: None,
            entity_catalog: Vec::new(),
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
                        let kind_text = match self.effective_kind() {
                            DataKind::Gene => "Gene reports",
                            DataKind::Transcript => "Transcript reports",
                            DataKind::Auto => "Report type unknown",
                        };
                        ui.label(egui::RichText::new(kind_text).strong());
                        ui.separator();
                        ui.label(format!("{} samples", self.files.len()));
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
                .on_hover_text("Inspect per-sample distributions and search/plot individual genes or transcripts without creating another full in-memory matrix.");
            if explore.clicked() {
                self.active_tab = AppTab::Explore;
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
                    .on_hover_text("Select one or more .xlsx expression reports. Files are sorted by filename before merging.");
                if choose_files.clicked() {
                    if let Some(mut files) = FileDialog::new()
                        .add_filter("Excel workbook", &["xlsx"])
                        .pick_files()
                    {
                        files.sort();
                        this.set_files(files);
                    }
                }

                let choose_folder = ui
                    .add_enabled(!this.running, egui::Button::new("Choose folder…"))
                    .on_hover_text("Select a folder. All .xlsx files directly inside that folder will be included and sorted by filename.");
                if choose_folder.clicked() {
                    if let Some(folder) = FileDialog::new().pick_folder() {
                        match cfs_expression_merger::discover_xlsx_files(&folder) {
                            Ok(files) => this.set_files(files),
                            Err(e) => this.status = format!("Error: {e:#}"),
                        }
                    }
                }

                let clear = ui
                    .add_enabled(!this.running && !this.files.is_empty(), egui::Button::new("Clear"))
                    .on_hover_text("Remove all selected workbooks and reset previews/analysis state.");
                if clear.clicked() {
                    this.set_files(Vec::new());
                }
            });

            if let Some(first) = this.files.first() {
                ui.add_space(4.0);
                ui.label(format!(
                    "First workbook: {}",
                    first.file_name().unwrap_or_default().to_string_lossy()
                ));
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
                .max_height(310.0)
                .show(ui, |ui| {
                    egui::Grid::new("sample_preview_grid_v02")
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
                let combo = egui::ComboBox::from_id_salt("kind_v02")
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
                    this.ensure_analysis_metric_valid();
                    this.entity_catalog.clear();
                    this.entity_search.clear();
                    this.selected_entity = None;
                    this.distributions.clear();
                    this.profile.clear();
                    if !this.files.is_empty() && !this.analysis_running {
                        this.start_catalog_load();
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
                    if let Some(path) = FileDialog::new()
                        .add_filter("TSV", &["tsv"])
                        .set_file_name(this.default_output_name())
                        .save_file()
                    {
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
        if self.files.is_empty() {
            self.card(ui, "Explore & QC", |_this, ui| {
                ui.heading("No data selected");
                ui.label("Select expression workbooks on the Merge & Export tab first.");
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

    fn render_analysis_controls(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Analysis controls", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Measurement:")
                    .on_hover_text("Choose the numeric value suffix used for QC distributions and gene/transcript profiles.");
                let before = this.analysis_metric;
                let combo = egui::ComboBox::from_id_salt("analysis_metric")
                    .selected_text(this.analysis_metric.display_name())
                    .show_ui(ui, |ui| {
                        for metric in all_metrics(this.effective_kind())
                            .into_iter()
                            .filter(|m| m.is_numeric())
                        {
                            ui.selectable_value(
                                &mut this.analysis_metric,
                                metric,
                                metric.display_name(),
                            );
                        }
                    });
                combo.response.on_hover_text(
                    "Only numeric measurements are offered. 'Is Expressed' is categorical and is not plotted as a continuous value.",
                );
                if before != this.analysis_metric {
                    this.distributions.clear();
                    this.profile.clear();
                }

                let zero_response = ui
                    .checkbox(&mut this.analysis_ignore_zeros, "Ignore zeros for box plots")
                    .on_hover_text("Exclude exact zeros when calculating sample quartiles/whiskers. Detection-rate statistics still use all observed values.");
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
        self.card(ui, "Sample distributions & QC", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                let build = ui
                    .add_enabled(!this.analysis_running, egui::Button::new("Build sample QC"))
                    .on_hover_text("Read the selected measurement from each workbook, one sample at a time, and calculate box-plot statistics plus detection rates.");
                if build.clicked() {
                    this.start_distribution_analysis();
                }
                ui.label(
                    egui::RichText::new(
                        "Box plots summarize the distribution across genes/transcripts within each sample.",
                    )
                    .weak(),
                );
            });

            if this.distributions.is_empty() {
                ui.add_space(12.0);
                ui.label("Click ‘Build sample QC’ to calculate per-sample distributions.");
                return;
            }

            let metric_name = this.analysis_metric.display_name().to_string();
            let log1p = this.analysis_log1p;
            let sample_names: Vec<String> = this
                .distributions
                .iter()
                .map(|summary| summary.sample.clone())
                .collect();
            let boxes: Vec<BoxElem> = this
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
                        .box_width(0.65),
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

            let y_label = this.plot_axis_label();
            Plot::new("sample_distribution_plot")
                .height(330.0)
                .x_axis_label("Sample")
                .y_axis_label(y_label)
                .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_samples))
                .show(ui, |plot_ui| {
                    plot_ui.box_plot(box_plot);
                });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("QC summary").strong());
            egui::ScrollArea::vertical()
                .max_height(210.0)
                .show(ui, |ui| {
                    egui::Grid::new("distribution_summary_grid")
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

    fn render_entity_profile_card(&mut self, ui: &mut egui::Ui) {
        self.card(ui, "Gene / transcript search & profile", |this, ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Search:")
                    .on_hover_text("Search the first workbook by gene symbol, NCBI Gene ID, or transcript accession/name.");
                let search_hint = match this.effective_kind() {
                    DataKind::Transcript => "gene symbol, gene ID, or transcript ID",
                    _ => "gene symbol or NCBI Gene ID",
                };
                ui.add(
                    egui::TextEdit::singleline(&mut this.entity_search)
                        .hint_text(search_hint)
                        .desired_width(320.0),
                )
                .on_hover_text("Search is case-insensitive and uses the identifier catalogue loaded from the first sample workbook.");

                let reload = ui
                    .add_enabled(!this.analysis_running, egui::Button::new("Reload index"))
                    .on_hover_text("Re-read identifiers from the first workbook. This is useful if the selected file set changed or the catalogue failed to load.");
                if reload.clicked() {
                    this.start_catalog_load();
                }
                ui.label(format!("{} indexed rows", this.entity_catalog.len()));
            });

            if this.entity_catalog.is_empty() {
                ui.add_space(8.0);
                ui.label("The searchable identifier index is loading or unavailable.");
                return;
            }

            let matches = this.entity_matches(60);
            if !this.entity_search.trim().is_empty() {
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        egui::Grid::new("entity_search_results")
                            .striped(true)
                            .num_columns(3)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Identifier").strong());
                                ui.label(egui::RichText::new("Gene").strong());
                                ui.label(egui::RichText::new("NCBI Gene ID").strong());
                                ui.end_row();
                                for idx in matches {
                                    let entry = &this.entity_catalog[idx];
                                    let selected = this.selected_entity == Some(idx);
                                    if ui
                                        .selectable_label(selected, &entry.primary)
                                        .on_hover_text("Select this identifier for plotting across all sample workbooks.")
                                        .clicked()
                                    {
                                        this.selected_entity = Some(idx);
                                        this.profile.clear();
                                    }
                                    ui.label(&entry.gene_symbol);
                                    ui.monospace(&entry.gene_id);
                                    ui.end_row();
                                }
                            });
                    });
            }

            ui.add_space(8.0);
            if let Some(idx) = this.selected_entity {
                if let Some(entry) = this.entity_catalog.get(idx) {
                    let selected_label = if this.effective_kind() == DataKind::Transcript {
                        format!("{} — {} (Gene ID {})", entry.primary, entry.gene_symbol, entry.gene_id)
                    } else {
                        format!("{} (Gene ID {})", entry.primary, entry.gene_id)
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Selected:").strong());
                        ui.monospace(selected_label);
                        let plot = ui
                            .add_enabled(!this.analysis_running, egui::Button::new("Plot selected"))
                            .on_hover_text("Extract this identifier's selected measurement from every sample workbook and plot the per-sample profile.");
                        if plot.clicked() {
                            this.start_profile_analysis();
                        }
                    });
                }
            } else {
                ui.label(egui::RichText::new("Select a search result to plot it.").weak());
            }

            if this.profile.is_empty() {
                return;
            }

            let sample_names: Vec<String> = this.profile.iter().map(|p| p.sample.clone()).collect();
            let line_points: Vec<[f64; 2]> = this
                .profile
                .iter()
                .enumerate()
                .filter_map(|(idx, point)| {
                    point
                        .value
                        .map(|v| [idx as f64, plot_transform(v, this.analysis_log1p)])
                })
                .collect();
            let accent = ui.visuals().selection.bg_fill;
            let axis_samples = sample_names.clone();
            let metric_name = this.analysis_metric.display_name().to_string();
            let entity_label = this
                .selected_entity
                .and_then(|idx| this.entity_catalog.get(idx))
                .map(|entry| entry.primary.clone())
                .unwrap_or_else(|| "Selected entity".to_string());

            Plot::new("entity_profile_plot")
                .height(330.0)
                .x_axis_label("Sample")
                .y_axis_label(this.plot_axis_label())
                .x_axis_formatter(move |mark, _range| sample_tick_label(mark.value, &axis_samples))
                .label_formatter(|hover| match hover {
                    HoverPosition::NearDataPoint {
                        plot_name,
                        position: _,
                        ..
                    } if !plot_name.is_empty() => Some(plot_name.to_string()),
                    _ => None,
                })
                .show(ui, |plot_ui| {
                    if line_points.len() > 1 {
                        plot_ui.line(
                            Line::new("profile trend", line_points.clone())
                                .color(accent)
                                .width(1.5)
                                .allow_hover(false),
                        );
                    }
                    for (idx, point) in this.profile.iter().enumerate() {
                        let Some(raw) = point.value else { continue; };
                        let label = format!(
                            "{}\nSample: {}\n{}: {}",
                            entity_label,
                            point.sample,
                            metric_name,
                            format_value(raw)
                        );
                        plot_ui.points(
                            Points::new(
                                label,
                                vec![[idx as f64, plot_transform(raw, this.analysis_log1p)]],
                            )
                            .color(accent)
                            .radius(4.0),
                        );
                    }
                });

            let observed: Vec<f64> = this.profile.iter().filter_map(|p| p.value).collect();
            if !observed.is_empty() {
                let mean = observed.iter().sum::<f64>() / observed.len() as f64;
                let min = observed.iter().copied().fold(f64::INFINITY, f64::min);
                let max = observed.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                ui.label(format!(
                    "Observed in {}/{} samples • mean {} • range {} – {}",
                    observed.len(),
                    this.profile.len(),
                    format_value(mean),
                    format_value(min),
                    format_value(max)
                ));
            }
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
        self.ensure_analysis_metric_valid();
        self.entity_catalog.clear();
        self.entity_search.clear();
        self.selected_entity = None;
        self.distributions.clear();
        self.profile.clear();
        self.analysis_status = if self.files.is_empty() {
            "Load some expression workbooks to enable QC and plotting.".to_string()
        } else {
            "Loading searchable gene/transcript index from the first workbook…".to_string()
        };
        if !self.files.is_empty() && !self.analysis_running {
            self.start_catalog_load();
        }
    }

    fn effective_kind(&self) -> DataKind {
        match self.kind {
            DataKind::Auto => self.detected_kind,
            other => other,
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
        let available: Vec<Metric> = all_metrics(self.effective_kind())
            .into_iter()
            .filter(|m| m.is_numeric())
            .collect();
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
        let Some(first) = self.files.first().cloned() else {
            return;
        };
        if self.analysis_running {
            return;
        }
        let kind = self.kind;
        let (tx, rx) = mpsc::channel();
        self.analysis_receiver = Some(rx);
        self.analysis_running = true;
        self.analysis_progress = 0.0;
        self.analysis_status = "Loading searchable identifier index…".to_string();
        std::thread::spawn(move || {
            let result = load_entity_catalog(&first, kind).map_err(|e| format!("{e:#}"));
            let _ = tx.send(AnalysisWorkerMessage::CatalogDone(result));
        });
    }

    fn start_distribution_analysis(&mut self) {
        if self.analysis_running || self.files.is_empty() {
            return;
        }
        let files = self.files.clone();
        let kind = self.kind;
        let metric = self.analysis_metric;
        let ignore_zeros = self.analysis_ignore_zeros;
        let (tx, rx) = mpsc::channel();
        self.analysis_receiver = Some(rx);
        self.analysis_running = true;
        self.analysis_progress = 0.0;
        self.analysis_status = format!("Calculating {} distributions…", metric.display_name());
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

    fn start_profile_analysis(&mut self) {
        if self.analysis_running || self.files.is_empty() {
            return;
        }
        let Some(idx) = self.selected_entity else {
            return;
        };
        let Some(entry) = self.entity_catalog.get(idx).cloned() else {
            return;
        };
        let files = self.files.clone();
        let kind = self.kind;
        let metric = self.analysis_metric;
        let (tx, rx) = mpsc::channel();
        self.analysis_receiver = Some(rx);
        self.analysis_running = true;
        self.analysis_progress = 0.0;
        self.analysis_status = format!("Loading {} across samples…", entry.primary);
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
                    self.status = match result {
                        Ok(summary) => format!(
                            "Done: {} rows × {} samples. Missing rows: {}; unknown rows: {}. Saved to {}",
                            summary.rows,
                            summary.samples,
                            summary.missing_rows,
                            summary.unknown_rows,
                            summary.output.display()
                        ),
                        Err(e) => format!("Merge failed: {e}"),
                    };
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
                            self.analysis_status = format!("Search index ready: {n} identifiers.");
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

    fn entity_matches(&self, limit: usize) -> Vec<usize> {
        let query = self.entity_search.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        self.entity_catalog
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.primary.to_ascii_lowercase().contains(&query)
                    || entry.gene_id.to_ascii_lowercase().contains(&query)
                    || entry.gene_symbol.to_ascii_lowercase().contains(&query)
            })
            .map(|(idx, _)| idx)
            .take(limit)
            .collect()
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
