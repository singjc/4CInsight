#![cfg(target_arch = "wasm32")]

#[path = "../../src/analysis_docs.rs"]
mod analysis_docs;
mod api;
mod contract;
#[path = "../../src/input_conventions.rs"]
mod input_conventions;
#[path = "../../src/ui_theme.rs"]
mod ui_theme;

use analysis_docs::{
    AnalysisDocToyChart, AnalysisDocToyExample, analysis_doc_example_matches,
    analysis_doc_section_header_matches, analysis_doc_section_matches, analysis_doc_sections,
    analysis_doc_subsection_matches,
};
use api::{HealthResponse, MappingAuditResponse, ObjectSummaryResponse, ProjectSummaryResponse};
use contract::{RemoteBackendCapabilities, RemoteProjectSummary};
use eframe::egui;
use egui_plot::{Bar, BarChart, Legend, Line, Plot, Points};
use futures_util::{StreamExt, stream};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("browser window is unavailable");
        let document = window.document().expect("browser document is unavailable");
        let canvas = document
            .get_element_by_id("cfs_canvas")
            .expect("missing #cfs_canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#cfs_canvas is not a canvas element");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(WebShellApp::new(cc)))),
            )
            .await;

        result.expect("failed to start 4C web shell");
        if let Some(loading) = document.get_element_by_id("loading") {
            let _ = loading.set_attribute("style", "display:none");
        }
    });

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebTab {
    Overview,
    Merge,
    Explore,
    Metadata,
    Cohort,
    Docs,
}

impl WebTab {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Web status",
            Self::Merge => "Merge & Export",
            Self::Explore => "Explore & QC",
            Self::Metadata => "Metadata Builder",
            Self::Cohort => "Cohort Analysis",
            Self::Docs => "Docs",
        }
    }
}

#[derive(Debug, Clone)]
struct MetadataDraftRow {
    sample_id: String,
    subject_id: String,
    analysis_group: String,
    phenotype: String,
    timepoint: String,
    notes: String,
}

impl Default for MetadataDraftRow {
    fn default() -> Self {
        Self {
            sample_id: String::new(),
            subject_id: String::new(),
            analysis_group: String::new(),
            phenotype: String::new(),
            timepoint: String::new(),
            notes: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct RemoteApiState {
    health: Option<HealthResponse>,
    busy: bool,
    status_message: String,
    last_error: Option<String>,
    project: RemoteProjectSummary,
    project_name: String,
    retention_hours: u64,
    recovery_project_id: String,
    recovery_token: String,
    recovery_token_warning: String,
    confirm_delete: bool,
    objects: Vec<ObjectSummaryResponse>,
    assignment_roles: BTreeMap<String, String>,
    assignment_sample_ids: BTreeMap<String, String>,
    mapping_audit: Option<MappingAuditResponse>,
    selected_upload_count: usize,
    selected_upload_total_size: u64,
    selected_upload_skipped_count: usize,
    selected_upload_preview: String,
    upload_progress_total: usize,
    upload_progress_completed: usize,
    upload_progress_failed: usize,
    show_all_uploaded_objects: bool,
    analysis_subject_column: String,
    analysis_covariates: String,
    jobs: Vec<api::JobSummaryResponse>,
    active_job: Option<api::JobDetailResponse>,
}

impl Default for RemoteApiState {
    fn default() -> Self {
        Self {
            health: None,
            busy: false,
            status_message: "Checking local cfs-server…".to_string(),
            last_error: None,
            project: RemoteProjectSummary::default(),
            project_name: "Temporary 4C project".to_string(),
            retention_hours: 24,
            recovery_project_id: String::new(),
            recovery_token: String::new(),
            recovery_token_warning: String::new(),
            confirm_delete: false,
            objects: Vec::new(),
            assignment_roles: BTreeMap::new(),
            assignment_sample_ids: BTreeMap::new(),
            mapping_audit: None,
            selected_upload_count: 0,
            selected_upload_total_size: 0,
            selected_upload_skipped_count: 0,
            selected_upload_preview: String::new(),
            upload_progress_total: 0,
            upload_progress_completed: 0,
            upload_progress_failed: 0,
            show_all_uploaded_objects: false,
            analysis_subject_column: String::new(),
            analysis_covariates: String::new(),
            jobs: Vec::new(),
            active_job: None,
        }
    }
}

struct WebShellApp {
    tab: WebTab,
    docs_query: String,
    metadata_rows: Vec<MetadataDraftRow>,
    comparison_field: String,
    case_values: String,
    control_values: String,
    remote: Rc<RefCell<RemoteApiState>>,
    pending_uploads: Rc<RefCell<Vec<web_sys::File>>>,
    capabilities: RemoteBackendCapabilities,
}

impl WebShellApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ui_theme::apply_4c_theme(&cc.egui_ctx);
        let remote = Rc::new(RefCell::new(RemoteApiState::default()));
        let app = Self {
            tab: WebTab::Overview,
            docs_query: String::new(),
            metadata_rows: vec![
                MetadataDraftRow {
                    sample_id: "S001".to_string(),
                    subject_id: "P001".to_string(),
                    analysis_group: "Control".to_string(),
                    phenotype: "True_HC".to_string(),
                    timepoint: "Baseline".to_string(),
                    notes: "Illustrative browser-only draft row".to_string(),
                },
                MetadataDraftRow {
                    sample_id: "S002".to_string(),
                    subject_id: "P002".to_string(),
                    analysis_group: "Case".to_string(),
                    phenotype: "HYP".to_string(),
                    timepoint: "Baseline".to_string(),
                    notes: "Illustrative browser-only draft row".to_string(),
                },
            ],
            comparison_field: "analysis_group".to_string(),
            case_values: "Case".to_string(),
            control_values: "Control".to_string(),
            remote,
            pending_uploads: Rc::new(RefCell::new(Vec::new())),
            capabilities: RemoteBackendCapabilities::PHASE3D,
        };
        app.refresh_health(cc.egui_ctx.clone());
        app
    }

    fn refresh_health(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        {
            let mut state = remote.borrow_mut();
            state.busy = true;
            state.status_message = "Checking local cfs-server…".to_string();
            state.last_error = None;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::health().await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(health) => {
                    state.status_message = format!(
                        "Connected to cfs-server v{} · API v{}",
                        health.app_version, health.api_version
                    );
                    if state.retention_hours > health.max_ttl_hours {
                        state.retention_hours = health.max_ttl_hours;
                    }
                    state.health = Some(health);
                    state.last_error = None;
                }
                Err(error) => {
                    state.status_message = "Local cfs-server is not reachable".to_string();
                    state.last_error = Some(error);
                    state.health = None;
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn create_remote_project(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (display_name, retention_hours) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = "Creating private temporary project…".to_string();
            (state.project_name.clone(), state.retention_hours)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::create_project(display_name, retention_hours).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(created) => {
                    state.project = remote_project_from_response(&created.project);
                    state.recovery_project_id = created.project.project_id.clone();
                    state.recovery_token = created.recovery_token;
                    state.recovery_token_warning = created.recovery_token_warning;
                    state.confirm_delete = false;
                    state.objects.clear();
                    state.assignment_roles.clear();
                    state.assignment_sample_ids.clear();
                    state.mapping_audit = None;
                    state.jobs.clear();
                    state.active_job = None;
                    state.selected_upload_count = 0;
                    state.selected_upload_total_size = 0;
                    state.selected_upload_skipped_count = 0;
                    state.selected_upload_preview.clear();
                    state.upload_progress_total = 0;
                    state.upload_progress_completed = 0;
                    state.upload_progress_failed = 0;
                    state.status_message = "Private project created".to_string();
                    state.last_error = None;
                }
                Err(error) => {
                    state.status_message = "Project creation failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn recover_remote_project(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error =
                    Some("Enter both a project UUID and recovery token.".to_string());
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = "Recovering project…".to_string();
            (project_id, token)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let project_result = api::get_project(&project_id, &token).await;
            let objects_result = match &project_result {
                Ok(_) => Some(api::list_objects(&project_id, &token).await),
                Err(_) => None,
            };
            let jobs_result = match &project_result {
                Ok(_) => Some(api::list_jobs(&project_id, &token).await),
                Err(_) => None,
            };
            let mut state = remote.borrow_mut();
            state.busy = false;
            match project_result {
                Ok(project) => {
                    state.project = remote_project_from_response(&project);
                    state.status_message = "Project recovered".to_string();
                    state.last_error = None;
                    state.confirm_delete = false;
                    match objects_result {
                        Some(Ok(objects)) => {
                            state.objects = objects;
                            sync_assignment_drafts(&mut state);
                            state.mapping_audit = None;
                        }
                        Some(Err(error)) => state.last_error = Some(error),
                        None => {}
                    }
                    match jobs_result {
                        Some(Ok(jobs)) => state.jobs = jobs,
                        Some(Err(error)) => state.last_error = Some(error),
                        None => {}
                    }
                    state.active_job = None;
                }
                Err(error) => {
                    state.status_message = "Project recovery failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn delete_remote_project(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let pending = self.pending_uploads.clone();
        let (project_id, token) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error =
                    Some("Enter the project UUID and recovery token before deleting.".to_string());
                return;
            }
            if !state.confirm_delete {
                state.last_error =
                    Some("Enable the deletion confirmation checkbox first.".to_string());
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = "Deleting project and local storage…".to_string();
            (project_id, token)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::delete_project(&project_id, &token).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(deleted) if deleted.deleted => {
                    state.project = RemoteProjectSummary::default();
                    state.recovery_project_id.clear();
                    state.recovery_token.clear();
                    state.recovery_token_warning.clear();
                    state.confirm_delete = false;
                    state.objects.clear();
                    state.assignment_roles.clear();
                    state.assignment_sample_ids.clear();
                    state.mapping_audit = None;
                    state.jobs.clear();
                    state.active_job = None;
                    state.selected_upload_count = 0;
                    state.selected_upload_total_size = 0;
                    state.selected_upload_skipped_count = 0;
                    state.selected_upload_preview.clear();
                    state.upload_progress_total = 0;
                    state.upload_progress_completed = 0;
                    state.upload_progress_failed = 0;
                    pending.borrow_mut().clear();
                    state.status_message = format!("Project {} deleted", deleted.project_id);
                    state.last_error = None;
                }
                Ok(_) => {
                    state.status_message = "Project was already absent".to_string();
                }
                Err(error) => {
                    state.status_message = "Project deletion failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn choose_upload_files(&self, ctx: egui::Context, directory: bool) {
        let Some(window) = web_sys::window() else {
            self.remote.borrow_mut().last_error = Some("Browser window is unavailable".to_string());
            return;
        };
        let Some(document) = window.document() else {
            self.remote.borrow_mut().last_error =
                Some("Browser document is unavailable".to_string());
            return;
        };
        let Ok(element) = document.create_element("input") else {
            self.remote.borrow_mut().last_error =
                Some("Unable to create browser file selector".to_string());
            return;
        };
        let Ok(input) = element.dyn_into::<web_sys::HtmlInputElement>() else {
            self.remote.borrow_mut().last_error =
                Some("Unable to initialize browser file selector".to_string());
            return;
        };
        input.set_type("file");
        input.set_multiple(true);
        if directory {
            let _ = input.set_attribute("webkitdirectory", "");
            let _ = input.set_attribute("directory", "");
        }
        let remote = self.remote.clone();
        let pending = self.pending_uploads.clone();
        let input_for_change = input.clone();
        let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| {
            let Some(files) = input_for_change.files() else {
                return;
            };
            let mut selected = Vec::new();
            let mut total = 0u64;
            let mut skipped = 0usize;
            let mut preview = Vec::new();
            let mut role_counts = BTreeMap::<String, usize>::new();
            for idx in 0..files.length() {
                let Some(file) = files.get(idx) else {
                    continue;
                };
                let classification = input_conventions::classify_input_filename(&file.name());
                if directory && classification.is_none() {
                    skipped = skipped.saturating_add(1);
                    continue;
                }
                if let Some(classification) = classification {
                    *role_counts
                        .entry(classification.kind.display_name().to_string())
                        .or_default() += 1;
                } else {
                    *role_counts
                        .entry("Custom / unresolved".to_string())
                        .or_default() += 1;
                }
                total = total.saturating_add(file.size() as u64);
                if preview.len() < 8 {
                    preview.push(format!(
                        "{} ({})",
                        file.name(),
                        format_bytes(file.size() as u64)
                    ));
                }
                selected.push(file);
            }
            let mut state = remote.borrow_mut();
            state.selected_upload_skipped_count = skipped;
            if selected.is_empty() {
                pending.borrow_mut().clear();
                state.selected_upload_count = 0;
                state.selected_upload_total_size = 0;
                state.selected_upload_preview.clear();
                state.last_error = Some(if directory && skipped > 0 {
                    format!(
                        "The selected directory contained {skipped} file(s), but none matched the native 4C input filename conventions. Use Choose files… for intentionally custom-named inputs."
                    )
                } else {
                    "No files were selected.".to_string()
                });
                drop(state);
                ctx.request_repaint();
                return;
            }
            let count = selected.len();
            *pending.borrow_mut() = selected;
            state.selected_upload_count = count;
            state.selected_upload_total_size = total;
            state.selected_upload_preview = preview.join("  •  ");
            if count > 8 {
                state
                    .selected_upload_preview
                    .push_str(&format!("  •  … and {} more", count - 8));
            }
            state.upload_progress_total = 0;
            state.upload_progress_completed = 0;
            state.upload_progress_failed = 0;
            state.last_error = None;
            let role_summary = role_counts
                .into_iter()
                .map(|(role, count)| format!("{role}: {count}"))
                .collect::<Vec<_>>()
                .join(" · ");
            state.status_message = if directory {
                if skipped > 0 {
                    format!(
                        "Native-like folder scan selected {count} recognized 4C input file(s) and skipped {skipped} auxiliary/unrecognized file(s) · {role_summary}"
                    )
                } else {
                    format!(
                        "Native-like folder scan selected {count} recognized 4C input file(s) · {role_summary}"
                    )
                }
            } else {
                format!("Selected {count} file(s) · {role_summary}")
            };
            drop(state);
            ctx.request_repaint();
        });
        input.set_onchange(Some(closure.as_ref().unchecked_ref()));
        input.click();
        closure.forget();
    }

    fn upload_selected_files(&self, ctx: egui::Context) {
        let files = self.pending_uploads.borrow().clone();
        if files.is_empty() {
            self.remote.borrow_mut().last_error =
                Some("Choose one or more files first.".to_string());
            return;
        }
        let remote = self.remote.clone();
        let pending = self.pending_uploads.clone();
        let (project_id, token, concurrency) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error =
                    Some("Recover or create a private project before uploading.".to_string());
                return;
            }
            let (max_file_bytes, max_project_bytes, max_objects, max_concurrent_uploads) = state
                .health
                .as_ref()
                .map(|h| {
                    (
                        h.max_file_bytes,
                        h.max_project_bytes,
                        h.max_objects,
                        h.max_concurrent_uploads,
                    )
                })
                .unwrap_or((0, 0, 0, 1));
            if max_file_bytes > 0 {
                if let Some(file) = files
                    .iter()
                    .find(|file| file.size() as u64 > max_file_bytes)
                {
                    state.last_error = Some(format!(
                        "{} is {}, above the server per-file limit of {}. Raise CFS_MAX_FILE_BYTES only if this local project requires it.",
                        file.name(),
                        format_bytes(file.size() as u64),
                        format_bytes(max_file_bytes)
                    ));
                    return;
                }
            }
            let projected_bytes = state
                .project
                .storage_bytes
                .saturating_add(state.selected_upload_total_size);
            if max_project_bytes > 0 && projected_bytes > max_project_bytes {
                state.last_error = Some(format!(
                    "Selected batch {} + current retained storage {} = {}, above the {} project quota. The quota also counts frozen job snapshots/artifacts. Raise CFS_MAX_PROJECT_BYTES only if the backend has sufficient private storage.",
                    format_bytes(state.selected_upload_total_size),
                    format_bytes(state.project.storage_bytes),
                    format_bytes(projected_bytes),
                    format_bytes(max_project_bytes)
                ));
                return;
            }
            let projected_objects = state.project.uploaded_objects.saturating_add(files.len());
            if max_objects > 0 && projected_objects > max_objects {
                state.last_error = Some(format!(
                    "Selected batch has {} files; the project currently has {} uploaded objects, so this upload would require {} of the server's {} object slots. Reduce the batch or raise CFS_MAX_OBJECTS for an appropriately sized local backend.",
                    files.len(),
                    state.project.uploaded_objects,
                    projected_objects,
                    max_objects
                ));
                return;
            }
            let concurrency = max_concurrent_uploads.max(1).min(files.len().max(1));
            state.busy = true;
            state.last_error = None;
            state.upload_progress_total = files.len();
            state.upload_progress_completed = 0;
            state.upload_progress_failed = 0;
            state.status_message = format!(
                "Uploading {} file(s) with up to {} concurrent local stream(s)…",
                files.len(),
                concurrency
            );
            (project_id, token, concurrency)
        };

        wasm_bindgen_futures::spawn_local(async move {
            let uploads = stream::iter(files.into_iter().enumerate().map(|(idx, file)| {
                let project_id = project_id.clone();
                let token = token.clone();
                let retry_file = file.clone();
                async move {
                    (
                        idx,
                        retry_file,
                        api::upload_object(&project_id, &token, file).await,
                    )
                }
            }))
            .buffer_unordered(concurrency);
            futures_util::pin_mut!(uploads);

            let mut errors = Vec::new();
            let mut failed_files = Vec::new();
            while let Some((idx, retry_file, result)) = uploads.next().await {
                let mut state = remote.borrow_mut();
                state.upload_progress_completed = state.upload_progress_completed.saturating_add(1);
                if let Err(error) = result {
                    state.upload_progress_failed = state.upload_progress_failed.saturating_add(1);
                    errors.push(format!("file {}: {error}", idx + 1));
                    failed_files.push(retry_file);
                }
                state.status_message = format!(
                    "Uploaded {}/{} file(s) · {} failed · concurrency {}",
                    state.upload_progress_completed,
                    state.upload_progress_total,
                    state.upload_progress_failed,
                    concurrency
                );
                drop(state);
                ctx.request_repaint();
            }

            let project_result = api::get_project(&project_id, &token).await;
            let objects_result = api::list_objects(&project_id, &token).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            if let Ok(project) = project_result {
                state.project = remote_project_from_response(&project);
            }
            match objects_result {
                Ok(objects) => {
                    state.objects = objects;
                    sync_assignment_drafts(&mut state);
                }
                Err(error) => errors.push(format!("refresh after upload: {error}")),
            }
            state.mapping_audit = None;
            state.active_job = None;
            if failed_files.is_empty() {
                state.selected_upload_count = 0;
                state.selected_upload_total_size = 0;
                state.selected_upload_skipped_count = 0;
                state.selected_upload_preview.clear();
                pending.borrow_mut().clear();
            } else {
                let failed_total = failed_files
                    .iter()
                    .map(|file| file.size() as u64)
                    .sum::<u64>();
                state.selected_upload_count = failed_files.len();
                state.selected_upload_total_size = failed_total;
                state.selected_upload_skipped_count = 0;
                state.selected_upload_preview = format!(
                    "{} failed file(s) retained for one-click retry",
                    failed_files.len()
                );
                *pending.borrow_mut() = failed_files;
            }

            let auto_assigned = state
                .objects
                .iter()
                .filter(|object| object.role != "unassigned")
                .count();
            let unresolved = state
                .objects
                .iter()
                .filter(|object| object.role == "unassigned")
                .count();
            if errors.is_empty() {
                state.status_message = format!(
                    "Batch upload complete · {} object(s) assigned by native filename conventions · {} object(s) need attention",
                    auto_assigned, unresolved
                );
                state.last_error = None;
            } else {
                let hidden = errors.len().saturating_sub(6);
                let mut message = errors.into_iter().take(6).collect::<Vec<_>>().join(" | ");
                if hidden > 0 {
                    message.push_str(&format!(" | … and {hidden} more failure(s)"));
                }
                state.status_message = "Batch upload completed with one or more failures; failed files remain selected for retry".to_string();
                state.last_error = Some(message);
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn delete_remote_object(&self, object_id: String, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error = Some(
                    "Project credentials are required to delete an uploaded object.".to_string(),
                );
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = format!("Deleting uploaded object {object_id}…");
            (project_id, token)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::delete_object(&project_id, &token, &object_id).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(deleted) if deleted.deleted => {
                    state.project = remote_project_from_response(&deleted.project);
                    state
                        .objects
                        .retain(|item| item.object_id != deleted.object_id);
                    state.assignment_roles.remove(&deleted.object_id);
                    state.assignment_sample_ids.remove(&deleted.object_id);
                    state.mapping_audit = None;
                    state.status_message = "Uploaded object deleted".to_string();
                    state.last_error = None;
                }
                Ok(_) => {
                    state.status_message = "Uploaded object was already absent".to_string();
                }
                Err(error) => {
                    state.status_message = "Object deletion failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn save_object_assignment(&self, object_id: String, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token, role, sample_id) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error = Some(
                    "Project credentials are required to assign uploaded objects.".to_string(),
                );
                return;
            }
            let role = state
                .assignment_roles
                .get(&object_id)
                .cloned()
                .unwrap_or_else(|| "unassigned".to_string());
            let sample_id = state
                .assignment_sample_ids
                .get(&object_id)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            state.busy = true;
            state.last_error = None;
            state.status_message = "Saving uploaded-object role assignment…".to_string();
            (project_id, token, role, sample_id)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result =
                api::assign_object(&project_id, &token, &object_id, &role, sample_id).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(assigned) => {
                    if let Some(existing) = state
                        .objects
                        .iter_mut()
                        .find(|item| item.object_id == object_id)
                    {
                        *existing = assigned.object;
                    }
                    sync_assignment_drafts(&mut state);
                    state.mapping_audit = None;
                    state.status_message = "Input role assignment saved".to_string();
                    state.last_error = None;
                }
                Err(error) => {
                    state.status_message = "Input role assignment failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn auto_assign_existing_objects(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error = Some(
                    "Project credentials are required to auto-detect uploaded input roles."
                        .to_string(),
                );
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message =
                "Applying native filename conventions to unassigned uploaded objects…".to_string();
            (project_id, token)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::auto_assign_objects(&project_id, &token).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(response) => {
                    state.objects = response.objects;
                    sync_assignment_drafts(&mut state);
                    state.mapping_audit = None;
                    state.active_job = None;
                    state.status_message = format!(
                        "Native auto-detection assigned {} object(s) · {} already assigned · {} failed validation · {} unresolved",
                        response.assigned,
                        response.already_assigned,
                        response.validation_failed,
                        response.unresolved
                    );
                    if response.warnings.is_empty() {
                        state.last_error = None;
                    } else {
                        state.last_error = Some(response.warnings.join(" | "));
                    }
                }
                Err(error) => {
                    state.status_message = "Automatic role detection failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn run_remote_mapping_audit(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let comparison_column = self.comparison_field.trim().to_string();
        let case_values = split_comparison_values(&self.case_values);
        let control_values = split_comparison_values(&self.control_values);
        let (project_id, token) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            if project_id.is_empty() || token.is_empty() {
                state.last_error =
                    Some("Project credentials are required to run the mapping audit.".to_string());
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = "Running native server-side sample mapping audit…".to_string();
            (project_id, token)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::run_mapping_audit(
                &project_id,
                &token,
                if comparison_column.is_empty() {
                    None
                } else {
                    Some(comparison_column)
                },
                case_values,
                control_values,
            )
            .await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(mapping) => {
                    state.mapping_audit = Some(mapping);
                    state.status_message = "Server-side mapping audit complete".to_string();
                    state.last_error = None;
                }
                Err(error) => {
                    state.mapping_audit = None;
                    state.status_message = "Mapping audit failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn submit_remote_job(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let comparison_column = self.comparison_field.trim().to_string();
        let case_values = split_comparison_values(&self.case_values);
        let control_values = split_comparison_values(&self.control_values);
        let (project_id, token, mapping_snapshot_sha256, subject_column, covariates) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            let mapping_snapshot_sha256 = match state.mapping_audit.as_ref() {
                Some(mapping) => mapping.mapping_snapshot_sha256.clone(),
                None => {
                    state.last_error = Some(
                        "Run and review the current mapping audit before submitting a job."
                            .to_string(),
                    );
                    return;
                }
            };
            if project_id.is_empty() || token.is_empty() {
                state.last_error =
                    Some("Project credentials are required to submit a job.".to_string());
                return;
            }
            if comparison_column.is_empty() || case_values.is_empty() || control_values.is_empty() {
                state.last_error = Some(
                    "A comparison column plus Case and Control values is required.".to_string(),
                );
                return;
            }
            let subject_column = state.analysis_subject_column.trim().to_string();
            let covariates = state
                .analysis_covariates
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            state.busy = true;
            state.last_error = None;
            state.status_message =
                "Freezing the validated project snapshot and submitting a local analysis job…"
                    .to_string();
            (
                project_id,
                token,
                mapping_snapshot_sha256,
                if subject_column.is_empty() {
                    None
                } else {
                    Some(subject_column)
                },
                covariates,
            )
        };
        wasm_bindgen_futures::spawn_local(async move {
            let request = api::SubmitJobRequest {
                mapping_snapshot_sha256,
                comparison_column,
                case_values,
                control_values,
                subject_column,
                covariates,
                run_qc: true,
                run_gene_corrected: true,
            };
            let result = api::submit_job(&project_id, &token, request).await;
            let mut poll_job_id = None;
            {
                let mut state = remote.borrow_mut();
                state.busy = false;
                match result {
                    Ok(job) => {
                        poll_job_id = Some(job.summary.job_id.clone());
                        state.active_job = Some(job);
                        state.status_message =
                            "Phase-3D job submitted; polling local worker status…".to_string();
                        state.last_error = None;
                    }
                    Err(error) => {
                        state.status_message = "Job submission failed".to_string();
                        state.last_error = Some(error);
                    }
                }
            }
            ctx.request_repaint();
            if let Some(job_id) = poll_job_id {
                poll_remote_job(remote, ctx, project_id, token, job_id).await;
            }
        });
    }

    fn refresh_remote_jobs(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token) = {
            let state = remote.borrow();
            (
                state.recovery_project_id.trim().to_string(),
                state.recovery_token.trim().to_string(),
            )
        };
        if project_id.is_empty() || token.is_empty() {
            self.remote.borrow_mut().last_error =
                Some("Project credentials are required to list jobs.".to_string());
            return;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::list_jobs(&project_id, &token).await;
            let mut state = remote.borrow_mut();
            match result {
                Ok(jobs) => {
                    state.jobs = jobs;
                    state.last_error = None;
                }
                Err(error) => state.last_error = Some(error),
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn open_remote_job(&self, job_id: String, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token) = {
            let state = remote.borrow();
            (
                state.recovery_project_id.trim().to_string(),
                state.recovery_token.trim().to_string(),
            )
        };
        if project_id.is_empty() || token.is_empty() {
            self.remote.borrow_mut().last_error =
                Some("Project credentials are required to open a persisted job.".to_string());
            return;
        }
        wasm_bindgen_futures::spawn_local(async move {
            match api::get_job(&project_id, &token, &job_id).await {
                Ok(job) => {
                    let terminal = matches!(
                        job.summary.status.as_str(),
                        "complete" | "failed" | "cancelled"
                    );
                    remote.borrow_mut().active_job = Some(job);
                    ctx.request_repaint();
                    if !terminal {
                        poll_remote_job(remote, ctx, project_id, token, job_id).await;
                    }
                }
                Err(error) => {
                    remote.borrow_mut().last_error = Some(error);
                    ctx.request_repaint();
                }
            }
        });
    }

    fn cancel_active_remote_job(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token, job_id) = {
            let state = remote.borrow();
            let Some(job) = state.active_job.as_ref() else {
                return;
            };
            (
                state.recovery_project_id.trim().to_string(),
                state.recovery_token.trim().to_string(),
                job.summary.job_id.clone(),
            )
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::cancel_job(&project_id, &token, &job_id).await;
            let mut state = remote.borrow_mut();
            match result {
                Ok(cancelled) => {
                    if let Some(job) = state.active_job.as_mut() {
                        job.summary = cancelled.job;
                    }
                    state.status_message = "Cancellation request sent".to_string();
                    state.last_error = None;
                }
                Err(error) => state.last_error = Some(error),
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.remote.borrow().clone();
        let connected = snapshot.health.is_some();
        let phase_label = "Web · Phase 3D.2";
        let server_label = if connected {
            "Local server connected"
        } else {
            "Server not connected"
        };

        ui_theme::header_frame(ui, |ui| {
            let accent = ui.visuals().selection.bg_fill;
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
                    if ui
                        .selectable_label(self.tab == WebTab::Overview, "Web status")
                        .on_hover_text(&snapshot.status_message)
                        .clicked()
                    {
                        self.tab = WebTab::Overview;
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(server_label).strong());
                    ui.separator();
                    ui.label(egui::RichText::new(phase_label).weak());
                });
            });
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for tab in [
                WebTab::Merge,
                WebTab::Explore,
                WebTab::Metadata,
                WebTab::Cohort,
                WebTab::Docs,
            ] {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
    }

    fn overview(&mut self, ui: &mut egui::Ui) {
        ui.heading("Browser client + local Rust server");
        ui.label(
            "Phase 3D.2 adds native-like high-throughput ingest on top of the same-origin cfs-server: folder scans skip auxiliary files, standard 4C filenames auto-assign roles/sample IDs, and bounded concurrent uploads feed the existing native mapping/QC/job path.",
        );
        ui.add_space(10.0);

        let snapshot = self.remote.borrow().clone();
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("Local server status").strong());
            ui.horizontal_wrapped(|ui| {
                ui.label("API root:");
                ui.monospace(api::API_ROOT);
                ui.separator();
                ui.label(&snapshot.status_message);
                if snapshot.busy {
                    ui.spinner();
                }
            });
            if let Some(health) = &snapshot.health {
                ui.label(format!(
                    "Server status {} · v{} · API v{} · default retention {} h · maximum retention {} h",
                    health.status,
                    health.app_version,
                    health.api_version,
                    health.default_ttl_hours,
                    health.max_ttl_hours
                ));
                ui.label(format!(
                    "Uploads enabled: {} · mapping enabled: {} · analysis enabled: {}",
                    health.upload_enabled, health.mapping_enabled, health.analysis_enabled
                ));
                ui.label(format!(
                    "Batch/folder selection: {} · persistent jobs: {} · local worker slots: {}",
                    health.batch_upload_enabled, health.persistent_jobs, health.max_concurrent_jobs
                ));
                ui.label(format!(
                    "Quota profile: {} · per file {} · project {} · {} objects",
                    health.quota_profile,
                    format_bytes(health.max_file_bytes),
                    format_bytes(health.max_project_bytes),
                    health.max_objects
                ));
            }
            if let Some(error) = &snapshot.last_error {
                ui.label(egui::RichText::new(error).color(egui::Color32::LIGHT_RED));
            }
            if ui.button("Refresh server status").clicked() {
                self.refresh_health(ui.ctx().clone());
            }
        });

        ui.add_space(10.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("What works in Phase 3D.2").strong());
            for item in [
                "One Rust cfs-server serves both web/dist and /api from 127.0.0.1:8080 by default.",
                "A private temporary project can be created with an opaque UUID and independent recovery token.",
                "Only the SHA-256 hash of the recovery token is stored by the server.",
                "A project can be recovered or permanently deleted by presenting the UUID plus recovery token.",
                "Browser upload supports multiple selected files and directory selection; each file is still streamed as its own authenticated opaque object with independent SHA-256 and quota enforcement.",
                "Uploaded objects can be listed and individually deleted, with strict per-file/per-project/object-count quotas; folder paths never become server storage keys.",
                "Every uploaded object can be assigned an explicit 4C input role; sample-level modalities also require an explicit sample ID.",
                "Primary metadata is parsed server-side with the existing native loader and the existing native sample mapping audit can be run from the browser.",
                "A current validated mapping fingerprint can be frozen into a persistent asynchronous job; later upload/assignment changes cannot silently alter that job snapshot.",
                "Phase 3D jobs reuse the native cross-modal QC engine and can run one representative corrected gene-expression lane from an explicitly assigned merged gene-expression TSV.",
                "XLSX role assignment performs defensive ZIP/container checks before scientific parsing, including entry-count and expanded-size limits.",
                "Persisted jobs survive browser reload/recovery; interrupted in-progress jobs are marked failed after a server restart rather than silently resumed.",
                "Expired project directories are cleaned on startup and by an hourly cleanup task.",
                "Shared 4C Docs, toy plots, and the browser Metadata Builder preview continue to work.",
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label("•");
                    ui.label(item);
                });
            }
        });
        ui.add_space(10.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("Still intentionally disabled").strong());
            for item in [
                "Running the full unified corrected transcript/BCR/TCR/isotype/immune/pathway stack through remote jobs; Phase 3D exposes only QC plus the representative corrected gene lane.",
                "Full unified HTML report generation and polished browser artifact download controls (the authenticated artifact endpoint itself is available).",
                "S3/MinIO storage or any user/account system.",
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label("•");
                    ui.label(item);
                });
            }
        });
    }

    fn placeholder(&self, ui: &mut egui::Ui, title: &str, next_step: &str) {
        ui.heading(title);
        ui.add_space(6.0);
        ui.label(
            "The browser now supports private objects, native mapping, and a narrow frozen-job scientific lane. Merge/export remains disabled until the full unified analysis/report contract is connected.",
        );
        ui.add_space(10.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Next server-backed step").strong());
                ui.label(next_step);
            });
    }

    fn metadata_builder(&mut self, ui: &mut egui::Ui) {
        ui.heading("Metadata Builder — browser shell");
        ui.label(
            "Rows edited here live only in browser memory for this preview. A later server phase will connect the same UI concepts to uploaded metadata objects and server-side validation.",
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Add sample row").clicked() {
                self.metadata_rows.push(MetadataDraftRow::default());
            }
            if ui.button("Remove last row").clicked() && !self.metadata_rows.is_empty() {
                self.metadata_rows.pop();
            }
            ui.label(format!("{} draft rows", self.metadata_rows.len()));
        });
        ui.add_space(8.0);

        egui::ScrollArea::horizontal()
            .id_salt("web_metadata_horizontal")
            .show(ui, |ui| {
                egui::Grid::new("web_metadata_grid")
                    .striped(true)
                    .min_col_width(120.0)
                    .show(ui, |ui| {
                        for header in [
                            "sample_id",
                            "subject_id",
                            "analysis_group",
                            "phenotype",
                            "timepoint",
                            "notes",
                        ] {
                            ui.label(egui::RichText::new(header).strong());
                        }
                        ui.end_row();

                        for row in &mut self.metadata_rows {
                            ui.add_sized(
                                [125.0, 22.0],
                                egui::TextEdit::singleline(&mut row.sample_id),
                            );
                            ui.add_sized(
                                [125.0, 22.0],
                                egui::TextEdit::singleline(&mut row.subject_id),
                            );
                            ui.add_sized(
                                [135.0, 22.0],
                                egui::TextEdit::singleline(&mut row.analysis_group),
                            );
                            ui.add_sized(
                                [125.0, 22.0],
                                egui::TextEdit::singleline(&mut row.phenotype),
                            );
                            ui.add_sized(
                                [125.0, 22.0],
                                egui::TextEdit::singleline(&mut row.timepoint),
                            );
                            ui.add_sized([260.0, 22.0], egui::TextEdit::singleline(&mut row.notes));
                            ui.end_row();
                        }
                    });
            });

        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(
                "The production Metadata Builder remains in the native app for now; this preview is deliberately transport-free.",
            )
            .weak(),
        );
    }

    fn cohort_shell(&mut self, ui: &mut egui::Ui) {
        ui.heading("Cohort Analysis — private remote-project shell");
        ui.label(
            "Phase 3D.2 keeps explicit/manual role correction available but automatically recognizes the same standard 4C filenames as the native desktop scanner. Folder upload filters auxiliary files, bounded concurrency speeds large cohorts, and the existing native mapping/QC/frozen-job path remains the scientific validation gate.",
        );
        ui.add_space(8.0);

        let snapshot = self.remote.borrow().clone();
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Execution backend").strong());
                ui.label(if snapshot.health.is_some() {
                    "Remote browser client — local cfs-server connected"
                } else {
                    "Remote browser client — cfs-server not connected"
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Same-origin API:");
                    ui.monospace(api::API_ROOT);
                    if snapshot.busy {
                        ui.spinner();
                    }
                });
                if let Some(error) = &snapshot.last_error {
                    ui.label(egui::RichText::new(error).color(egui::Color32::LIGHT_RED));
                }
            });
        ui.add_space(8.0);

        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("1. Create a private temporary project").strong());
            ui.label(
                "The recovery token is generated independently from the project UUID. The server stores only its SHA-256 hash. Keep the original token private; it behaves like a password for this project.",
            );
            ui.add_space(5.0);
            let max_ttl = snapshot
                .health
                .as_ref()
                .map(|health| health.max_ttl_hours)
                .unwrap_or(168);
            {
                let mut state = self.remote.borrow_mut();
                egui::Grid::new("web_phase3b_create_project_grid").show(ui, |ui| {
                    ui.label("Display name");
                    ui.text_edit_singleline(&mut state.project_name);
                    ui.end_row();
                    ui.label("Retention (hours)");
                    ui.add(
                        egui::DragValue::new(&mut state.retention_hours)
                            .range(1..=max_ttl)
                            .speed(1.0),
                    );
                    ui.end_row();
                });
            }
            let can_create = snapshot.health.is_some() && !snapshot.busy;
            if ui
                .add_enabled(can_create, egui::Button::new("Create private project"))
                .clicked()
            {
                self.create_remote_project(ui.ctx().clone());
            }
        });

        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("2. Recovery credentials").strong());
            ui.label(
                "Save both values somewhere private. The recovery token is intentionally not derivable from the project data and is not recoverable from the server-side hash.",
            );
            ui.add_space(5.0);
            {
                let mut state = self.remote.borrow_mut();
                egui::Grid::new("web_phase3b_recovery_grid").show(ui, |ui| {
                    ui.label("Project UUID");
                    ui.add_sized(
                        [470.0, 24.0],
                        egui::TextEdit::singleline(&mut state.recovery_project_id),
                    );
                    ui.end_row();
                    ui.label("Recovery token");
                    ui.add_sized(
                        [470.0, 24.0],
                        egui::TextEdit::singleline(&mut state.recovery_token),
                    );
                    ui.end_row();
                });
                if !state.recovery_token_warning.is_empty() {
                    ui.label(
                        egui::RichText::new(&state.recovery_token_warning)
                            .small()
                            .strong(),
                    );
                }
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!snapshot.busy, egui::Button::new("Recover / refresh project"))
                    .clicked()
                {
                    self.recover_remote_project(ui.ctx().clone());
                }
                if ui.button("Clear credentials from this browser view").clicked() {
                    let mut state = self.remote.borrow_mut();
                    state.recovery_project_id.clear();
                    state.recovery_token.clear();
                    state.recovery_token_warning.clear();
                    state.project = RemoteProjectSummary::default();
                    state.objects.clear();
                    state.assignment_roles.clear();
                    state.assignment_sample_ids.clear();
                    state.mapping_audit = None;
                    state.jobs.clear();
                    state.active_job = None;
                    state.selected_upload_count = 0;
                    state.selected_upload_total_size = 0;
                    state.selected_upload_skipped_count = 0;
                    state.selected_upload_preview.clear();
                    state.upload_progress_total = 0;
                    state.upload_progress_completed = 0;
                    state.upload_progress_failed = 0;
                    self.pending_uploads.borrow_mut().clear();
                    state.status_message = "Recovery credentials cleared from browser memory; the server project remains until deletion or expiry".to_string();
                }
            });
        });

        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("3. Private uploaded objects").strong());
            ui.label(
                "Phase 3D.2 stores uploaded bytes under opaque object UUIDs. Standard native 4C filenames can automatically seed role/sample-ID assignments after content validation; custom or ambiguous inputs remain unassigned for review. Filenames remain metadata only and never become storage keys or authorization credentials.",
            );
            if let Some(health) = &snapshot.health {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Storage backend:");
                    ui.monospace(&health.storage_backend);
                    ui.separator();
                    ui.label("quota profile");
                    ui.monospace(&health.quota_profile);
                    ui.separator();
                    ui.label(format!("per-file limit {}", format_bytes(health.max_file_bytes)));
                    ui.separator();
                    ui.label(format!("project quota {}", format_bytes(health.max_project_bytes)));
                    ui.separator();
                    ui.label(format!("max {} objects", health.max_objects));
                    ui.separator();
                    ui.label(format!("max {} persisted jobs", health.max_jobs_per_project));
                    ui.separator();
                    ui.label(format!("{} concurrent uploads", health.max_concurrent_uploads));
                });
            }
            ui.add_space(5.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!snapshot.busy, egui::Button::new("Choose files…"))
                    .clicked()
                {
                    self.choose_upload_files(ui.ctx().clone(), false);
                }
                if ui
                    .add_enabled(!snapshot.busy, egui::Button::new("Choose folder…"))
                    .clicked()
                {
                    self.choose_upload_files(ui.ctx().clone(), true);
                }
                if snapshot.selected_upload_count == 0 {
                    ui.label(egui::RichText::new("No files selected").weak());
                } else {
                    ui.label(format!(
                        "{} file(s), {} total",
                        snapshot.selected_upload_count,
                        format_bytes(snapshot.selected_upload_total_size)
                    ));
                }
                let has_credentials = !snapshot.recovery_project_id.trim().is_empty()
                    && !snapshot.recovery_token.trim().is_empty()
                    && snapshot.project.project_id.is_some();
                let upload_enabled = snapshot
                    .health
                    .as_ref()
                    .map(|health| health.upload_enabled)
                    .unwrap_or(false);
                let can_upload = !snapshot.busy
                    && has_credentials
                    && upload_enabled
                    && !self.pending_uploads.borrow().is_empty();
                if ui
                    .add_enabled(
                        can_upload,
                        egui::Button::new(format!("Upload {} selected file(s)", snapshot.selected_upload_count)),
                    )
                    .clicked()
                {
                    self.upload_selected_files(ui.ctx().clone());
                }
                if snapshot.selected_upload_count > 0
                    && ui.add_enabled(!snapshot.busy, egui::Button::new("Clear selection")).clicked()
                {
                    self.pending_uploads.borrow_mut().clear();
                    let mut state = self.remote.borrow_mut();
                    state.selected_upload_count = 0;
                    state.selected_upload_total_size = 0;
                    state.selected_upload_skipped_count = 0;
                    state.selected_upload_preview.clear();
                    state.upload_progress_total = 0;
                    state.upload_progress_completed = 0;
                    state.upload_progress_failed = 0;
                }
            });
            if !snapshot.selected_upload_preview.is_empty() {
                ui.label(egui::RichText::new(&snapshot.selected_upload_preview).small().weak());
            }
            if snapshot.selected_upload_skipped_count > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "Folder scan skipped {} auxiliary/unrecognized file(s), matching native desktop scan behavior.",
                        snapshot.selected_upload_skipped_count
                    ))
                    .small()
                    .weak(),
                );
            }
            if snapshot.upload_progress_total > 0 && snapshot.busy {
                let fraction = snapshot.upload_progress_completed as f32
                    / snapshot.upload_progress_total.max(1) as f32;
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .show_percentage()
                        .text(format!(
                            "{}/{} uploaded · {} failed",
                            snapshot.upload_progress_completed,
                            snapshot.upload_progress_total,
                            snapshot.upload_progress_failed
                        )),
                );
            }
            if snapshot.selected_upload_count > 0 {
                if let Some(health) = &snapshot.health {
                    let projected_objects = snapshot
                        .project
                        .uploaded_objects
                        .saturating_add(snapshot.selected_upload_count);
                    let projected_bytes = snapshot
                        .project
                        .storage_bytes
                        .saturating_add(snapshot.selected_upload_total_size);
                    let fits_objects = projected_objects <= health.max_objects;
                    let fits_bytes = projected_bytes <= health.max_project_bytes;
                    let color = if fits_objects && fits_bytes {
                        ui.visuals().weak_text_color()
                    } else {
                        egui::Color32::LIGHT_RED
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "Projected after upload: {}/{} objects · {} / {} retained project storage",
                            projected_objects,
                            health.max_objects,
                            format_bytes(projected_bytes),
                            format_bytes(health.max_project_bytes)
                        ))
                        .small()
                        .color(color),
                    );
                }
            }
            ui.label(
                egui::RichText::new(
                    "Choose folder… now mirrors native project scanning: only recognized 4C inputs are queued by default, while auxiliary files such as GMT libraries/logs are skipped. Uploads use bounded concurrency, but each file remains its own authenticated opaque object with independent SHA-256/quota enforcement. Directory paths are never server storage keys.",
                )
                .small()
                .weak(),
            );
            ui.label(
                egui::RichText::new(
                    "For Phase-3D validation, use harmless synthetic inputs first. Mapping, native QC, and the representative corrected gene-expression job lane are enabled; the rest of the corrected multi-omics stack remains disabled.",
                )
                .small()
                .weak(),
            );
            ui.add_space(7.0);
            let objects = snapshot.objects.clone();
            if objects.is_empty() {
                ui.label(egui::RichText::new("No uploaded objects in this project.").weak());
            } else {
                let unresolved = objects.iter().filter(|object| object.role == "unassigned").count();
                let assigned = objects.len().saturating_sub(unresolved);
                let mut role_counts = BTreeMap::<String, usize>::new();
                for object in &objects {
                    *role_counts.entry(input_role_label(&object.role).to_string()).or_default() += 1;
                }
                let role_summary = role_counts
                    .iter()
                    .map(|(role, count)| format!("{role}: {count}"))
                    .collect::<Vec<_>>()
                    .join(" · ");
                ui.label(
                    egui::RichText::new(format!(
                        "{} assigned automatically/manually · {} need attention · {}",
                        assigned, unresolved, role_summary
                    ))
                    .small(),
                );
                ui.label(
                    egui::RichText::new(
                        "Exact native desktop filename conventions automatically assign standard sample workbooks and sample IDs after defensive validation. Use Auto-detect for projects uploaded with an older version; only unresolved/custom files require manual edits. The mapping audit remains the authoritative validation gate.",
                    )
                    .small()
                    .weak(),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !snapshot.busy && unresolved > 0,
                            egui::Button::new("Auto-detect native roles & sample IDs"),
                        )
                        .clicked()
                    {
                        self.auto_assign_existing_objects(ui.ctx().clone());
                    }
                    let mut show_all = snapshot.show_all_uploaded_objects;
                    let label = if show_all {
                        "Show only files needing attention"
                    } else {
                        "Show all uploaded objects"
                    };
                    if ui.selectable_label(show_all, label).clicked() {
                        show_all = !show_all;
                        self.remote.borrow_mut().show_all_uploaded_objects = show_all;
                    }
                });
                ui.add_space(5.0);
                egui::Grid::new("web_phase3c_objects_grid")
                    .striped(true)
                    .min_col_width(80.0)
                    .show(ui, |ui| {
                        for heading in ["Original filename", "4C input role", "Sample ID", "Size", "SHA-256", "Object ID", ""] {
                            ui.label(egui::RichText::new(heading).strong());
                        }
                        ui.end_row();
                        let visible_objects = objects
                            .into_iter()
                            .filter(|object| snapshot.show_all_uploaded_objects || object.role == "unassigned")
                            .collect::<Vec<_>>();
                        if visible_objects.is_empty() {
                            ui.label(egui::RichText::new("All uploaded objects are assigned. Use ‘Show all uploaded objects’ to inspect or correct them.").weak());
                            for _ in 1..7 {
                                ui.label("");
                            }
                            ui.end_row();
                        }
                        for object in visible_objects {
                            ui.label(&object.original_name).on_hover_text(format!(
                                "Uploaded at Unix second {}{}",
                                object.uploaded_unix_seconds,
                                object
                                    .content_type
                                    .as_deref()
                                    .map(|value| format!(" · content type {value}"))
                                    .unwrap_or_default()
                            ));

                            let mut draft_role = snapshot
                                .assignment_roles
                                .get(&object.object_id)
                                .cloned()
                                .unwrap_or_else(|| object.role.clone());
                            egui::ComboBox::from_id_salt(format!("role_{}", object.object_id))
                                .selected_text(input_role_label(&draft_role))
                                .show_ui(ui, |ui| {
                                    for (value, label) in INPUT_ROLE_OPTIONS {
                                        ui.selectable_value(&mut draft_role, (*value).to_string(), *label);
                                    }
                                });
                            self.remote
                                .borrow_mut()
                                .assignment_roles
                                .insert(object.object_id.clone(), draft_role.clone());

                            let requires_sample = role_requires_sample_id(&draft_role);
                            let mut sample_id = snapshot
                                .assignment_sample_ids
                                .get(&object.object_id)
                                .cloned()
                                .or_else(|| object.sample_id.clone())
                                .unwrap_or_default();
                            ui.add_enabled_ui(requires_sample, |ui| {
                                ui.add_sized([150.0, 24.0], egui::TextEdit::singleline(&mut sample_id));
                            });
                            self.remote
                                .borrow_mut()
                                .assignment_sample_ids
                                .insert(object.object_id.clone(), sample_id);

                            ui.label(format_bytes(object.byte_size));
                            ui.monospace(short_hash(&object.sha256));
                            ui.monospace(&object.object_id);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(!snapshot.busy, egui::Button::new("Save role"))
                                    .clicked()
                                {
                                    self.save_object_assignment(object.object_id.clone(), ui.ctx().clone());
                                }
                                if ui
                                    .add_enabled(!snapshot.busy, egui::Button::new("Delete"))
                                    .clicked()
                                {
                                    self.delete_remote_object(object.object_id.clone(), ui.ctx().clone());
                                }
                            });
                            ui.end_row();
                        }
                    });
            }
        });

        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("4. Server-side mapping audit").strong());
                ui.label(
                    "The server materializes the assigned primary metadata in private scratch space, parses it with the existing native metadata loader, constructs the native ProjectScan representation from explicit object roles/sample IDs, and runs the existing build_mapping() audit. A successful current audit becomes the gate for a frozen Phase-3D job.",
                );
                ui.add_space(5.0);
                ui.label(egui::RichText::new("Optional Case/Control count preview").strong());
                egui::Grid::new("web_phase3c_mapping_comparison").show(ui, |ui| {
                    ui.label("Metadata field");
                    ui.text_edit_singleline(&mut self.comparison_field);
                    ui.end_row();
                    ui.label("Case value(s)");
                    ui.text_edit_singleline(&mut self.case_values);
                    ui.end_row();
                    ui.label("Control value(s)");
                    ui.text_edit_singleline(&mut self.control_values);
                    ui.end_row();
                });
                ui.label(
                    egui::RichText::new(
                        "Separate multiple values with commas. These values only preview cohort counts during mapping; they do not submit an analysis job.",
                    )
                    .small()
                    .weak(),
                );
                let mapping_enabled = snapshot
                    .health
                    .as_ref()
                    .map(|health| health.mapping_enabled)
                    .unwrap_or(false);
                let has_credentials = !snapshot.recovery_project_id.trim().is_empty()
                    && !snapshot.recovery_token.trim().is_empty();
                if ui
                    .add_enabled(
                        !snapshot.busy && mapping_enabled && has_credentials,
                        egui::Button::new("Validate inputs & run mapping audit"),
                    )
                    .clicked()
                {
                    self.run_remote_mapping_audit(ui.ctx().clone());
                }

                if let Some(mapping) = snapshot.mapping_audit.as_ref() {
                    ui.add_space(8.0);
                    egui::Grid::new("web_phase3c_mapping_summary")
                        .striped(true)
                        .show(ui, |ui| {
                            for (label, value) in [
                                ("Metadata rows", mapping.metadata_rows.to_string()),
                                ("Canonical samples", mapping.canonical_samples.to_string()),
                                ("Metadata ID column", mapping.metadata_id_column.clone()),
                                ("Matched files", mapping.matched_files.to_string()),
                                ("Unmatched files", mapping.unmatched_files.to_string()),
                                ("Ambiguous files", mapping.ambiguous_files.to_string()),
                                ("Metadata-only samples", mapping.metadata_only_samples.to_string()),
                            ] {
                                ui.label(label);
                                ui.label(value);
                                ui.end_row();
                            }
                            if let Some(name) = &mapping.comparison_name {
                                ui.label("Comparison preview");
                                ui.label(name);
                                ui.end_row();
                                ui.label("Case / Control samples");
                                ui.label(format!(
                                    "{} / {}",
                                    mapping.case_samples.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
                                    mapping.control_samples.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string())
                                ));
                                ui.end_row();
                            }
                        });

                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Modality coverage").strong());
                    egui::Grid::new("web_phase3c_modality_coverage")
                        .striped(true)
                        .show(ui, |ui| {
                            for heading in ["Modality", "Assigned files", "Matched files", "Samples with files"] {
                                ui.label(egui::RichText::new(heading).strong());
                            }
                            ui.end_row();
                            for modality in &mapping.modality_coverage {
                                ui.label(&modality.modality);
                                ui.label(modality.assigned_files.to_string());
                                ui.label(modality.matched_files.to_string());
                                ui.label(modality.samples_with_files.to_string());
                                ui.end_row();
                            }
                        });

                    if !mapping.warnings.is_empty() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Mapping warnings").strong());
                        for warning in &mapping.warnings {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("•");
                                ui.label(warning);
                            });
                        }
                    }

                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(format!("Sample mapping detail ({} rows)", mapping.rows.len()))
                        .show(ui, |ui| {
                            egui::Grid::new("web_phase3c_mapping_rows")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Sample ID").strong());
                                    ui.label(egui::RichText::new("Status").strong());
                                    ui.label(egui::RichText::new("Modality counts").strong());
                                    ui.end_row();
                                    for row in &mapping.rows {
                                        ui.monospace(&row.sample_id);
                                        ui.label(&row.status);
                                        let counts = if row.counts.is_empty() {
                                            "—".to_string()
                                        } else {
                                            row.counts
                                                .iter()
                                                .map(|(name, count)| format!("{name}: {count}"))
                                                .collect::<Vec<_>>()
                                                .join(" · ")
                                        };
                                        ui.label(counts);
                                        ui.end_row();
                                    }
                                });
                        });
                } else {
                    ui.label(
                        egui::RichText::new(
                            "No mapping audit has been run for the current assignments.",
                        )
                        .small()
                        .weak(),
                    );
                }
            });

        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("5. Frozen Phase-3D analysis job").strong());
                ui.label(
                    "A job freezes the currently validated object-role/sample-ID manifest before scientific execution. Later uploads, assignment changes, or object deletion do not silently change that frozen job snapshot. Phase 3D runs the native mapping/QC path and one representative corrected gene-expression lane only.",
                );
                ui.add_space(5.0);
                {
                    let mut state = self.remote.borrow_mut();
                    egui::Grid::new("web_phase3d_job_settings").show(ui, |ui| {
                        ui.label("Subject column (optional)");
                        ui.add_sized(
                            [260.0, 22.0],
                            egui::TextEdit::singleline(&mut state.analysis_subject_column)
                                .hint_text("subject_id"),
                        );
                        ui.end_row();
                        ui.label("Covariates (comma separated)");
                        ui.add_sized(
                            [420.0, 22.0],
                            egui::TextEdit::singleline(&mut state.analysis_covariates)
                                .hint_text("age, sex, batch"),
                        );
                        ui.end_row();
                    });
                }
                ui.label(
                    egui::RichText::new(
                        "QC uses every frozen assigned modality that can produce an existing native QC representation. The corrected gene lane requires an object assigned as Merged gene-expression TSV; otherwise the job still completes with mapping/QC and records a warning.",
                    )
                    .small()
                    .weak(),
                );
                let job_enabled = snapshot
                    .health
                    .as_ref()
                    .map(|health| health.analysis_enabled && health.persistent_jobs)
                    .unwrap_or(false);
                let has_credentials = !snapshot.recovery_project_id.trim().is_empty()
                    && !snapshot.recovery_token.trim().is_empty();
                let current_mapping = snapshot.mapping_audit.is_some();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !snapshot.busy && job_enabled && has_credentials && current_mapping,
                            egui::Button::new("Freeze snapshot & submit QC/gene job"),
                        )
                        .clicked()
                    {
                        self.submit_remote_job(ui.ctx().clone());
                    }
                    if ui
                        .add_enabled(has_credentials, egui::Button::new("Refresh job list"))
                        .clicked()
                    {
                        self.refresh_remote_jobs(ui.ctx().clone());
                    }
                    let can_cancel = snapshot
                        .active_job
                        .as_ref()
                        .is_some_and(|job| !matches!(job.summary.status.as_str(), "complete" | "failed" | "cancelled"));
                    if ui
                        .add_enabled(can_cancel, egui::Button::new("Request cancellation"))
                        .clicked()
                    {
                        self.cancel_active_remote_job(ui.ctx().clone());
                    }
                });

                if let Some(job) = snapshot.active_job.as_ref() {
                    ui.add_space(8.0);
                    egui::Grid::new("web_phase3d_active_job").striped(true).show(ui, |ui| {
                        for (label, value) in [
                            ("Job ID", job.summary.job_id.clone()),
                            ("Status", job.summary.status.clone()),
                            ("Progress", job.summary.progress_message.clone()),
                            ("Snapshot SHA-256", job.summary.snapshot_sha256.clone()),
                        ] {
                            ui.label(label);
                            if label.contains("ID") || label.contains("SHA") {
                                ui.monospace(value);
                            } else {
                                ui.label(value);
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(error) = job.summary.error.as_ref() {
                        ui.label(egui::RichText::new(error).color(egui::Color32::LIGHT_RED));
                    }
                    if let Some(result) = job.result.as_ref() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Frozen-job results").strong());
                        ui.label(format!(
                            "Mapping: {} matched · {} unmatched · {} ambiguous · {} metadata-only",
                            result.mapping.matched_files,
                            result.mapping.unmatched_files,
                            result.mapping.ambiguous_files,
                            result.mapping.metadata_only_samples
                        ));
                        if let Some(qc) = result.qc.as_ref() {
                            ui.label(format!(
                                "QC: {} sample(s), {} modality representation(s), {} consensus outlier flag(s)",
                                qc.samples,
                                qc.modalities.len(),
                                qc.consensus_ids.len()
                            ));
                            if !qc.consensus_ids.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Consensus flagged sample IDs: {}",
                                        qc.consensus_ids.join(", ")
                                    ))
                                    .small(),
                                );
                            }
                        } else {
                            ui.label(egui::RichText::new("QC result not available for this snapshot").weak());
                        }
                        if let Some(gene) = result.gene.as_ref() {
                            ui.label(format!(
                                "Corrected gene lane: {} tested features · {} filtered · {} modeled samples · {} subjects",
                                gene.tested_features,
                                gene.filtered_features,
                                gene.n_model_samples,
                                gene.n_subjects
                            ));
                            egui::CollapsingHeader::new(format!(
                                "Top corrected gene results (showing {})",
                                gene.top_features.len()
                            ))
                            .show(ui, |ui| {
                                egui::Grid::new("web_phase3d_gene_top").striped(true).show(ui, |ui| {
                                    for heading in ["Feature", "Adjusted log2FC", "SE", "p", "q"] {
                                        ui.label(egui::RichText::new(heading).strong());
                                    }
                                    ui.end_row();
                                    for feature in &gene.top_features {
                                        ui.label(&feature.feature);
                                        ui.label(format!("{:.4}", feature.beta_group));
                                        ui.label(format!("{:.4}", feature.robust_se));
                                        ui.label(format!("{:.3e}", feature.p_value));
                                        ui.label(format!("{:.3e}", feature.q_value));
                                        ui.end_row();
                                    }
                                });
                            });
                        } else {
                            ui.label(
                                egui::RichText::new(
                                    "Representative corrected gene result not available; assign a valid Merged gene-expression TSV to exercise that parity lane.",
                                )
                                .weak(),
                            );
                        }
                        if !result.warnings.is_empty() {
                            ui.label(egui::RichText::new("Job warnings").strong());
                            for warning in &result.warnings {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("•");
                                    ui.label(warning);
                                });
                            }
                        }
                        if !result.artifacts.is_empty() {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Private result artifacts").strong());
                            for artifact in &result.artifacts {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(&artifact.name);
                                    ui.label(format_bytes(artifact.byte_size));
                                    ui.monospace(format!("sha256 {}", artifact.sha256));
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "artifact UUID {}",
                                            artifact.artifact_id
                                        ))
                                        .small()
                                        .weak(),
                                    );
                                });
                            }
                            ui.label(
                                egui::RichText::new(
                                    "Artifacts are authorization-protected. Phase 3D exposes the authenticated download endpoint on the server; a polished browser download control can be added with the full results/export phase.",
                                )
                                .small()
                                .weak(),
                            );
                        }
                    }
                } else {
                    ui.label(
                        egui::RichText::new(
                            "No active job in this browser view. A current mapping audit is required before submission.",
                        )
                        .small()
                        .weak(),
                    );
                }
                if !snapshot.jobs.is_empty() {
                    ui.add_space(6.0);
                    egui::CollapsingHeader::new(format!("Recent persisted jobs ({})", snapshot.jobs.len()))
                        .show(ui, |ui| {
                            egui::Grid::new("web_phase3d_job_history").striped(true).show(ui, |ui| {
                                ui.label(egui::RichText::new("Job ID").strong());
                                ui.label(egui::RichText::new("Status").strong());
                                ui.label(egui::RichText::new("Progress").strong());
                                ui.label(egui::RichText::new("Open").strong());
                                ui.end_row();
                                for job in &snapshot.jobs {
                                    ui.monospace(&job.job_id);
                                    ui.label(&job.status);
                                    ui.label(&job.progress_message);
                                    if ui.small_button("Open").clicked() {
                                        self.open_remote_job(job.job_id.clone(), ui.ctx().clone());
                                    }
                                    ui.end_row();
                                }
                            });
                        });
                }
            });

        ui.add_space(8.0);
        let current = self.remote.borrow().project.clone();
        let current_job = self.remote.borrow().active_job.clone();
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("6. Remote project summary").strong());
                egui::Grid::new("web_phase3d_project_summary").show(ui, |ui| {
                    ui.label("Project");
                    ui.label(&current.display_name);
                    ui.end_row();
                    ui.label("Project ID");
                    ui.monospace(current.project_id.as_deref().unwrap_or("not loaded"));
                    ui.end_row();
                    ui.label("Uploaded objects");
                    ui.label(current.uploaded_objects.to_string());
                    ui.end_row();
                    ui.label("Total retained storage");
                    ui.label(format_bytes(current.storage_bytes));
                    ui.end_row();
                    ui.label("Persisted jobs");
                    ui.label(current.job_count.to_string());
                    ui.end_row();
                    ui.label("Frozen job snapshot bytes");
                    ui.label(format_bytes(current.job_snapshot_bytes));
                    ui.end_row();
                    ui.label("Job artifact bytes");
                    ui.label(format_bytes(current.job_artifact_bytes));
                    ui.end_row();
                    ui.label("Created (Unix seconds)");
                    ui.label(
                        current
                            .created_unix_seconds
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    );
                    ui.end_row();
                    ui.label("Expires (Unix seconds)");
                    ui.label(
                        current
                            .expires_unix_seconds
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "—".to_string()),
                    );
                    ui.end_row();
                    ui.label("Current job");
                    ui.label(
                        current_job
                            .as_ref()
                            .map(|job| job.summary.status.as_str())
                            .unwrap_or("not submitted"),
                    );
                    ui.end_row();
                });
            });

        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
            ui.label(egui::RichText::new("7. Permanent deletion test").strong());
            ui.label(
                "Deletion removes the entire local project directory, including uploaded objects, frozen job snapshots, private artifacts, metadata records, and scratch files. The recovery token is required for this operation.",
            );
            {
                let mut state = self.remote.borrow_mut();
                ui.checkbox(
                    &mut state.confirm_delete,
                    "I understand this permanently deletes this temporary project",
                );
            }
            let can_delete = {
                let state = self.remote.borrow();
                !state.busy
                    && state.confirm_delete
                    && !state.recovery_project_id.trim().is_empty()
                    && !state.recovery_token.trim().is_empty()
            };
            if ui
                .add_enabled(can_delete, egui::Button::new("Delete project permanently"))
                .clicked()
            {
                self.delete_remote_project(ui.ctx().clone());
            }
        });

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Phase-3D capabilities:").strong());
            ui.label(format!(
                "project_lifecycle={} · upload={} · mapping={} · analysis={} · persistent_jobs={}",
                self.capabilities.project_lifecycle,
                self.capabilities.upload,
                self.capabilities.mapping,
                self.capabilities.analysis,
                self.capabilities.persistent_jobs
            ));
        });
    }

    fn docs(&mut self, ui: &mut egui::Ui) {
        ui.heading("Analysis documentation");
        ui.label(
            "This is the same canonical analysis-guide content used by the native GUI and HTML reports.",
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Search");
            ui.add_sized(
                [360.0, 24.0],
                egui::TextEdit::singleline(&mut self.docs_query)
                    .hint_text("clonality, PERMANOVA, q value, Aitchison, PCA…"),
            );
            if ui.button("Clear").clicked() {
                self.docs_query.clear();
            }
        });
        ui.add_space(6.0);

        let query = self.docs_query.trim();
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
                            self.render_toy_example(ui, section_idx, example_idx, example);
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
            ui.add_space(10.0);
            ui.label(egui::RichText::new("No documentation matched the search.").italics());
        }
    }

    fn render_toy_example(
        &self,
        ui: &mut egui::Ui,
        section_idx: usize,
        example_idx: usize,
        example: &AnalysisDocToyExample,
    ) {
        egui::Frame::group(ui.style())
            .fill(ui.visuals().faint_bg_color)
            .inner_margin(12)
            .corner_radius(8)
            .show(ui, |ui| {
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
                self.render_toy_plot(ui, section_idx, example_idx, example);
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

    fn render_toy_plot(
        &self,
        ui: &mut egui::Ui,
        section_idx: usize,
        example_idx: usize,
        example: &AnalysisDocToyExample,
    ) {
        let plot_id = format!("web_analysis_docs_toy_{section_idx}_{example_idx}");
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
                    .x_axis_formatter(move |mark, _range| doc_tick_label(mark.value, &labels))
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
                    .x_axis_formatter(move |mark, _range| doc_tick_label(mark.value, &labels))
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
                            "{}={} ({:.2}, {:.2})",
                            point.label, point.group, point.x, point.y
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("  •  ");
                ui.label(egui::RichText::new(labels).small().weak());
            }
        }
    }
}

const INPUT_ROLE_OPTIONS: &[(&str, &str)] = &[
    ("unassigned", "Unassigned"),
    ("primary_metadata", "Primary metadata"),
    ("filtered_metadata", "Filtered/subset metadata"),
    ("gene_expression", "Gene expression workbook"),
    ("transcript_expression", "Transcript expression workbook"),
    ("bcr", "BCR workbook"),
    ("bcr_isotype", "BCR isotype workbook"),
    ("tcr_alpha", "TCR alpha workbook"),
    ("tcr_beta", "TCR beta workbook"),
    ("immune_infiltrate", "Immune infiltrate workbook"),
    ("merged_gene_expression", "Merged gene-expression TSV"),
    (
        "merged_transcript_expression",
        "Merged transcript-expression TSV",
    ),
];

fn input_role_label(value: &str) -> &'static str {
    INPUT_ROLE_OPTIONS
        .iter()
        .find(|(role, _)| *role == value)
        .map(|(_, label)| *label)
        .unwrap_or("Unassigned")
}

fn role_requires_sample_id(value: &str) -> bool {
    matches!(
        value,
        "gene_expression"
            | "transcript_expression"
            | "bcr"
            | "bcr_isotype"
            | "tcr_alpha"
            | "tcr_beta"
            | "immune_infiltrate"
    )
}

fn sync_assignment_drafts(state: &mut RemoteApiState) {
    let valid_ids = state
        .objects
        .iter()
        .map(|object| object.object_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    state
        .assignment_roles
        .retain(|id, _| valid_ids.contains(id));
    state
        .assignment_sample_ids
        .retain(|id, _| valid_ids.contains(id));
    for object in &state.objects {
        state
            .assignment_roles
            .insert(object.object_id.clone(), object.role.clone());
        state.assignment_sample_ids.insert(
            object.object_id.clone(),
            object.sample_id.clone().unwrap_or_default(),
        );
    }
}

fn split_comparison_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn short_hash(value: &str) -> String {
    if value.len() <= 20 {
        value.to_string()
    } else {
        format!("{}…{}", &value[..12], &value[value.len() - 6..])
    }
}

async fn poll_remote_job(
    remote: Rc<RefCell<RemoteApiState>>,
    ctx: egui::Context,
    project_id: String,
    token: String,
    job_id: String,
) {
    for _ in 0..3600usize {
        gloo_timers::future::TimeoutFuture::new(1000).await;
        let result = api::get_job(&project_id, &token, &job_id).await;
        let terminal = {
            let mut state = remote.borrow_mut();
            match result {
                Ok(job) => {
                    let terminal = matches!(
                        job.summary.status.as_str(),
                        "complete" | "failed" | "cancelled"
                    );
                    state.status_message = job.summary.progress_message.clone();
                    state.active_job = Some(job);
                    state.last_error = None;
                    terminal
                }
                Err(error) => {
                    state.last_error = Some(error);
                    true
                }
            }
        };
        ctx.request_repaint();
        if terminal {
            break;
        }
    }
    if let Ok(jobs) = api::list_jobs(&project_id, &token).await {
        remote.borrow_mut().jobs = jobs;
        ctx.request_repaint();
    }
}

fn remote_project_from_response(value: &ProjectSummaryResponse) -> RemoteProjectSummary {
    RemoteProjectSummary {
        project_id: Some(value.project_id.clone()),
        display_name: value.display_name.clone(),
        uploaded_objects: value.uploaded_objects,
        storage_bytes: value.storage_bytes,
        job_count: value.job_count,
        job_snapshot_bytes: value.job_snapshot_bytes,
        job_artifact_bytes: value.job_artifact_bytes,
        created_unix_seconds: Some(value.created_unix_seconds),
        expires_unix_seconds: Some(value.expires_unix_seconds),
    }
}

impl eframe::App for WebShellApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.top_bar(ui);
            ui.add_space(10.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.tab {
                    WebTab::Overview => self.overview(ui),
                    WebTab::Merge => self.placeholder(
                        ui,
                        "Merge & Export",
                        "Phase 3D exposes private uploaded-object handles, validated role assignments, mapping summaries, and persisted frozen job/result summaries. Merge/export remains disabled until the full report/export lane is connected.",
                    ),
                    WebTab::Explore => self.placeholder(
                        ui,
                        "Explore & QC",
                        "Phase 3D now exposes uploaded-object summaries, mapping audit results, native QC, and a representative corrected gene result through persisted server jobs. Later phases will expand the same job contract to the complete scientific stack and reports without moving the scientific engine into WASM.",
                    ),
                    WebTab::Metadata => self.metadata_builder(ui),
                    WebTab::Cohort => self.cohort_shell(ui),
                    WebTab::Docs => self.docs(ui),
                });
        });
    }
}

fn doc_tick_label(value: f64, labels: &[String]) -> String {
    let rounded = value.round();
    if (value - rounded).abs() > 0.04 || rounded < 0.0 {
        return String::new();
    }
    labels.get(rounded as usize).cloned().unwrap_or_default()
}
