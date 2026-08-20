use cfs_expression_merger::{
    all_metrics, collaborator_default_metrics, inspect_kind, merge_wide, ColumnOrder, DataKind,
    MergeConfig, Metric, MergeSummary,
};
use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "CFS Expression Merger",
        options,
        Box::new(|_cc| Ok(Box::<MergeApp>::default())),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preset {
    Collaborator,
    All,
    Custom,
}

enum WorkerMessage {
    Progress(usize, usize, String),
    Done(Result<MergeSummary, String>),
}

struct MergeApp {
    files: Vec<PathBuf>,
    detected_kind: DataKind,
    kind: DataKind,
    preset: Preset,
    metrics: Vec<Metric>,
    order: ColumnOrder,
    output: Option<PathBuf>,
    status: String,
    progress: f32,
    running: bool,
    receiver: Option<Receiver<WorkerMessage>>,
}

impl Default for MergeApp {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            detected_kind: DataKind::Auto,
            kind: DataKind::Auto,
            preset: Preset::Collaborator,
            metrics: Vec::new(),
            order: ColumnOrder::AdjacentBySample,
            output: None,
            status: "Choose a folder or a set of Excel files.".to_string(),
            progress: 0.0,
            running: false,
            receiver: None,
        }
    }
}

impl eframe::App for MergeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_worker();
        let ctx = ui.ctx().clone();

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("CFS Expression Merger");
            ui.label("Merge RNA gene/transcript expression reports without loading the full cohort into memory.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add_enabled(!self.running, egui::Button::new("Choose files…")).clicked() {
                    if let Some(mut files) = FileDialog::new()
                        .add_filter("Excel workbook", &["xlsx"])
                        .pick_files()
                    {
                        files.sort();
                        self.set_files(files);
                    }
                }
                if ui.add_enabled(!self.running, egui::Button::new("Choose folder…")).clicked() {
                    if let Some(folder) = FileDialog::new().pick_folder() {
                        match cfs_expression_merger::discover_xlsx_files(&folder) {
                            Ok(files) => self.set_files(files),
                            Err(e) => self.status = format!("Error: {e:#}"),
                        }
                    }
                }
                ui.label(format!("{} files", self.files.len()));
            });

            if let Some(first) = self.files.first() {
                ui.label(format!("First: {}", first.file_name().unwrap_or_default().to_string_lossy()));
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Report type:");
                egui::ComboBox::from_id_salt("kind")
                    .selected_text(format!("{:?}", self.kind))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.kind, DataKind::Auto, "Auto");
                        ui.selectable_value(&mut self.kind, DataKind::Gene, "Gene");
                        ui.selectable_value(&mut self.kind, DataKind::Transcript, "Transcript");
                    });
                if self.detected_kind != DataKind::Auto {
                    ui.label(format!("Detected: {:?}", self.detected_kind));
                }
            });

            ui.horizontal(|ui| {
                ui.label("Preset:");
                let before = self.preset;
                ui.selectable_value(&mut self.preset, Preset::Collaborator, "Collaborator default");
                ui.selectable_value(&mut self.preset, Preset::All, "All measurements");
                ui.selectable_value(&mut self.preset, Preset::Custom, "Custom");
                if before != self.preset {
                    self.reset_metrics();
                }
            });

            let effective_kind = self.effective_kind();
            if self.preset == Preset::Custom {
                ui.group(|ui| {
                    ui.label("Measurements");
                    for metric in all_metrics(effective_kind) {
                        let mut enabled = self.metrics.contains(&metric);
                        if ui.checkbox(&mut enabled, metric.display_name()).changed() {
                            if enabled {
                                self.metrics.push(metric);
                            } else {
                                self.metrics.retain(|m| *m != metric);
                            }
                        }
                    }
                });
            } else {
                let shown = self.selected_metrics();
                ui.label(format!(
                    "Measurements: {}",
                    shown.iter().map(|m| m.display_name()).collect::<Vec<_>>().join(", ")
                ));
            }

            ui.horizontal(|ui| {
                ui.label("Column order:");
                ui.selectable_value(
                    &mut self.order,
                    ColumnOrder::AdjacentBySample,
                    "Sample A count, Sample A TPM, Sample B count, …",
                );
                ui.selectable_value(
                    &mut self.order,
                    ColumnOrder::GroupedByMeasurement,
                    "All counts, then all TPMs, …",
                );
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.running, egui::Button::new("Choose output…")).clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("TSV", &["tsv"])
                        .set_file_name("merged_expression.tsv")
                        .save_file()
                    {
                        self.output = Some(path);
                    }
                }
                if let Some(path) = &self.output {
                    ui.label(path.display().to_string());
                } else {
                    ui.label("No output selected");
                }
            });

            let ready = !self.running && !self.files.is_empty() && self.output.is_some();
            if ui.add_enabled(ready, egui::Button::new("Merge datasets")).clicked() {
                self.start_merge();
            }

            if self.running {
                ui.add(egui::ProgressBar::new(self.progress).show_percentage());
            }
            ui.label(&self.status);
        });

        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl MergeApp {
    fn set_files(&mut self, files: Vec<PathBuf>) {
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
        self.reset_metrics();
    }

    fn effective_kind(&self) -> DataKind {
        match self.kind {
            DataKind::Auto => self.detected_kind,
            other => other,
        }
    }

    fn reset_metrics(&mut self) {
        let kind = self.effective_kind();
        self.metrics = match self.preset {
            Preset::Collaborator => collaborator_default_metrics(kind),
            Preset::All => all_metrics(kind),
            Preset::Custom => collaborator_default_metrics(kind),
        };
    }

    fn selected_metrics(&self) -> Vec<Metric> {
        let kind = self.effective_kind();
        match self.preset {
            Preset::Collaborator => collaborator_default_metrics(kind),
            Preset::All => all_metrics(kind),
            Preset::Custom => self.metrics.clone(),
        }
    }

    fn start_merge(&mut self) {
        let Some(output) = self.output.clone() else { return; };
        let config = MergeConfig {
            files: self.files.clone(),
            output,
            kind: self.kind,
            metrics: self.selected_metrics(),
            column_order: self.order,
        };

        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.running = true;
        self.progress = 0.0;
        self.status = "Starting merge…".to_string();

        std::thread::spawn(move || {
            let tx_progress = tx.clone();
            let result = merge_wide(config, move |done, total, msg| {
                let _ = tx_progress.send(WorkerMessage::Progress(done, total, msg.to_string()));
            });
            let _ = tx.send(WorkerMessage::Done(result.map_err(|e| format!("{e:#}"))));
        });
    }

    fn poll_worker(&mut self) {
        let Some(rx) = self.receiver.take() else { return; };
        let mut keep_receiver = true;
        while let Ok(message) = rx.try_recv() {
            match message {
                WorkerMessage::Progress(done, total, msg) => {
                    self.progress = if total == 0 { 0.0 } else { done as f32 / total as f32 };
                    self.status = msg;
                }
                WorkerMessage::Done(result) => {
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
            self.receiver = Some(rx);
        }
    }
}
