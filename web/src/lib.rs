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
use api::{
    HealthResponse, MappingAuditResponse, MetadataProfileResponse, ObjectSummaryResponse,
    ProjectSummaryResponse,
};
use contract::{RemoteBackendCapabilities, RemoteProjectSummary};
use eframe::egui;
use egui_plot::{Bar, BarChart, HoverPosition, Legend, Line, Plot, Points};
use futures_util::{StreamExt, stream};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
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
    metadata_profile: Option<MetadataProfileResponse>,
    comparison_field: String,
    case_values: BTreeSet<String>,
    control_values: BTreeSet<String>,
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
    analysis_batch_column: String,
    analysis_family_column: String,
    analysis_sensitivity_column: String,
    analysis_run_exploratory: bool,
    analysis_run_pbmc_diagnostics: bool,
    analysis_pathway_q_threshold: f64,
    analysis_pathway_effect_threshold: f64,
    reviewed_exclusions: BTreeSet<String>,
    job_delete_armed: Option<String>,
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
            metadata_profile: None,
            comparison_field: String::new(),
            case_values: BTreeSet::new(),
            control_values: BTreeSet::new(),
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
            analysis_batch_column: String::new(),
            analysis_family_column: String::new(),
            analysis_sensitivity_column: String::new(),
            analysis_run_exploratory: true,
            analysis_run_pbmc_diagnostics: true,
            analysis_pathway_q_threshold: 0.05,
            analysis_pathway_effect_threshold: 0.5,
            reviewed_exclusions: BTreeSet::new(),
            job_delete_armed: None,
            jobs: Vec::new(),
            active_job: None,
        }
    }
}

struct WebShellApp {
    tab: WebTab,
    docs_query: String,
    metadata_rows: Vec<MetadataDraftRow>,
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
            remote,
            pending_uploads: Rc::new(RefCell::new(Vec::new())),
            capabilities: RemoteBackendCapabilities::PHASE3E,
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
                    clear_metadata_profile_state(&mut state);
                    state.mapping_audit = None;
                    state.jobs.clear();
                    state.active_job = None;
                    state.reviewed_exclusions.clear();
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
            let metadata_profile_result = match &objects_result {
                Some(Ok(objects))
                    if objects
                        .iter()
                        .any(|object| object.role == "primary_metadata") =>
                {
                    Some(api::metadata_profile(&project_id, &token).await)
                }
                _ => None,
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
                    match metadata_profile_result {
                        Some(Ok(profile)) => apply_metadata_profile(&mut state, profile),
                        Some(Err(error)) => {
                            clear_metadata_profile_state(&mut state);
                            state.last_error = Some(format!(
                                "Project recovered, but primary metadata fields could not be loaded: {error}"
                            ));
                        }
                        None => clear_metadata_profile_state(&mut state),
                    }
                    match jobs_result {
                        Some(Ok(jobs)) => state.jobs = jobs,
                        Some(Err(error)) => state.last_error = Some(error),
                        None => {}
                    }
                    state.active_job = None;
                    state.reviewed_exclusions.clear();
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
                    clear_metadata_profile_state(&mut state);
                    state.mapping_audit = None;
                    state.jobs.clear();
                    state.active_job = None;
                    state.reviewed_exclusions.clear();
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
            state.reviewed_exclusions.clear();
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
        let (project_id, token, was_primary_metadata) = {
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
            let was_primary_metadata = state
                .objects
                .iter()
                .any(|object| object.object_id == object_id && object.role == "primary_metadata");
            (project_id, token, was_primary_metadata)
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
                    if was_primary_metadata {
                        clear_metadata_profile_state(&mut state);
                    }
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
        let (project_id, token, role, sample_id, was_primary_metadata) = {
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
            let was_primary_metadata = state
                .objects
                .iter()
                .any(|object| object.object_id == object_id && object.role == "primary_metadata");
            (project_id, token, role, sample_id, was_primary_metadata)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result =
                api::assign_object(&project_id, &token, &object_id, &role, sample_id).await;
            let profile_result = if result.is_ok() && role == "primary_metadata" {
                Some(api::metadata_profile(&project_id, &token).await)
            } else {
                None
            };
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
                    if role == "primary_metadata" {
                        match profile_result {
                            Some(Ok(profile)) => apply_metadata_profile(&mut state, profile),
                            Some(Err(error)) => {
                                clear_metadata_profile_state(&mut state);
                                state.last_error = Some(format!(
                                    "Primary metadata was assigned, but its fields could not be loaded: {error}"
                                ));
                            }
                            None => {}
                        }
                    } else if was_primary_metadata {
                        clear_metadata_profile_state(&mut state);
                    }
                    state.mapping_audit = None;
                    state.status_message = "Input role assignment saved".to_string();
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
                    state.reviewed_exclusions.clear();
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

    fn refresh_remote_metadata_profile(&self, ctx: egui::Context) {
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
                    "Project credentials are required to load primary metadata fields.".to_string(),
                );
                return;
            }
            if !state
                .objects
                .iter()
                .any(|object| object.role == "primary_metadata")
            {
                state.last_error = Some(
                    "Assign exactly one uploaded object as Primary metadata first.".to_string(),
                );
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = "Loading categorical fields from primary metadata…".to_string();
            (project_id, token)
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::metadata_profile(&project_id, &token).await;
            let mut state = remote.borrow_mut();
            state.busy = false;
            match result {
                Ok(profile) => {
                    apply_metadata_profile(&mut state, profile);
                    state.mapping_audit = None;
                    state.status_message =
                        "Primary metadata fields loaded for comparison selection".to_string();
                    state.last_error = None;
                }
                Err(error) => {
                    clear_metadata_profile_state(&mut state);
                    state.mapping_audit = None;
                    state.status_message = "Primary metadata profiling failed".to_string();
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn run_remote_mapping_audit(&self, ctx: egui::Context) {
        let remote = self.remote.clone();
        let (project_id, token, comparison_column, case_values, control_values) = {
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
            let comparison_column = state.comparison_field.trim().to_string();
            let case_values = state.case_values.iter().cloned().collect::<Vec<_>>();
            let control_values = state.control_values.iter().cloned().collect::<Vec<_>>();
            if comparison_column.is_empty() || case_values.is_empty() || control_values.is_empty() {
                state.last_error = Some(
                    "Select a metadata grouping field plus at least one Case and one Control value before running the mapping audit."
                        .to_string(),
                );
                return;
            }
            state.busy = true;
            state.last_error = None;
            state.status_message = "Running native server-side sample mapping audit…".to_string();
            (
                project_id,
                token,
                comparison_column,
                case_values,
                control_values,
            )
        };
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::run_mapping_audit(
                &project_id,
                &token,
                Some(comparison_column),
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
        let (
            project_id,
            token,
            mapping_snapshot_sha256,
            comparison_column,
            case_values,
            control_values,
            subject_column,
            covariates,
            excluded_sample_ids,
            run_exploratory_direction_check,
            run_pbmc_diagnostics,
            pbmc_batch_column,
            pbmc_family_column,
            pbmc_sensitivity_column,
            pathway_q_threshold,
            pathway_effect_threshold,
        ) = {
            let mut state = remote.borrow_mut();
            if state.busy {
                return;
            }
            let project_id = state.recovery_project_id.trim().to_string();
            let token = state.recovery_token.trim().to_string();
            let comparison_column = state.comparison_field.trim().to_string();
            let case_values = state.case_values.iter().cloned().collect::<Vec<_>>();
            let control_values = state.control_values.iter().cloned().collect::<Vec<_>>();
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
            let subject_column = optional_trimmed(&state.analysis_subject_column);
            let covariates = state
                .analysis_covariates
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            let excluded_sample_ids = state
                .reviewed_exclusions
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            let run_exploratory_direction_check = state.analysis_run_exploratory;
            let run_pbmc_diagnostics = state.analysis_run_pbmc_diagnostics;
            let pbmc_batch_column = optional_trimmed(&state.analysis_batch_column);
            let pbmc_family_column = optional_trimmed(&state.analysis_family_column);
            let pbmc_sensitivity_column = optional_trimmed(&state.analysis_sensitivity_column);
            let pathway_q_threshold = state.analysis_pathway_q_threshold;
            let pathway_effect_threshold = state.analysis_pathway_effect_threshold;
            state.busy = true;
            state.last_error = None;
            state.status_message =
                "Freezing the validated project state and submitting the unified native analysis…"
                    .to_string();
            (
                project_id,
                token,
                mapping_snapshot_sha256,
                comparison_column,
                case_values,
                control_values,
                subject_column,
                covariates,
                excluded_sample_ids,
                run_exploratory_direction_check,
                run_pbmc_diagnostics,
                pbmc_batch_column,
                pbmc_family_column,
                pbmc_sensitivity_column,
                pathway_q_threshold,
                pathway_effect_threshold,
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
                run_full_unified: true,
                excluded_sample_ids,
                run_exploratory_direction_check,
                run_pbmc_diagnostics,
                pbmc_batch_column,
                pbmc_family_column,
                pbmc_sensitivity_column,
                pathway_q_threshold,
                pathway_effect_threshold,
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
                            "Phase-3E unified job submitted; polling the local native worker…"
                                .to_string();
                        state.last_error = None;
                    }
                    Err(error) => {
                        state.status_message = "Unified job submission failed".to_string();
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
                    {
                        let mut state = remote.borrow_mut();
                        state.status_message = format!(
                            "Opened persisted job {job_id}; its results are shown in the Frozen unified analysis section."
                        );
                        state.active_job = Some(job);
                        state.last_error = None;
                    }
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

    fn delete_stored_remote_job(&self, job_id: String, ctx: egui::Context) {
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
                Some("Project credentials are required to delete a persisted job.".to_string());
            return;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::delete_stored_job(&project_id, &token, &job_id).await;
            let mut state = remote.borrow_mut();
            match result {
                Ok(deleted) => {
                    state.project = remote_project_from_response(&deleted.project);
                    state.jobs.retain(|job| job.job_id != deleted.job_id);
                    if state
                        .active_job
                        .as_ref()
                        .is_some_and(|job| job.summary.job_id == deleted.job_id)
                    {
                        state.active_job = None;
                        state.reviewed_exclusions.clear();
                    }
                    state.job_delete_armed = None;
                    state.status_message = if deleted.deleted {
                        format!(
                            "Deleted persisted job {} and its frozen inputs/artifacts",
                            deleted.job_id
                        )
                    } else {
                        format!("Persisted job {} was already absent", deleted.job_id)
                    };
                    state.last_error = None;
                }
                Err(error) => {
                    state.job_delete_armed = None;
                    state.last_error = Some(error);
                }
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn download_remote_artifact(
        &self,
        job_id: String,
        artifact_id: String,
        filename: String,
        ctx: egui::Context,
    ) {
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
                Some("Project credentials are required to download job artifacts.".to_string());
            return;
        }
        {
            let mut state = remote.borrow_mut();
            state.status_message = format!("Downloading {filename}…");
            state.last_error = None;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let result =
                api::download_job_artifact(&project_id, &token, &job_id, &artifact_id).await;
            let mut state = remote.borrow_mut();
            match result {
                Ok(bytes) => match save_browser_download(&filename, &bytes) {
                    Ok(()) => {
                        state.status_message = format!("Downloaded {filename}");
                        state.last_error = None;
                    }
                    Err(error) => state.last_error = Some(error),
                },
                Err(error) => state.last_error = Some(error),
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn download_all_remote_artifacts(&self, job_id: String, ctx: egui::Context) {
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
                Some("Project credentials are required to download job artifacts.".to_string());
            return;
        }
        let filename = format!("4c_job_{job_id}_artifacts.zip");
        {
            let mut state = remote.borrow_mut();
            state.status_message = format!("Preparing authenticated ZIP bundle for job {job_id}…");
            state.last_error = None;
        }
        wasm_bindgen_futures::spawn_local(async move {
            let result = api::download_all_job_artifacts(&project_id, &token, &job_id).await;
            let mut state = remote.borrow_mut();
            match result {
                Ok(bytes) => match save_browser_download(&filename, &bytes) {
                    Ok(()) => {
                        state.status_message = format!(
                            "Downloaded all retained artifacts for job {job_id} as one ZIP"
                        );
                        state.last_error = None;
                    }
                    Err(error) => state.last_error = Some(error),
                },
                Err(error) => state.last_error = Some(error),
            }
            drop(state);
            ctx.request_repaint();
        });
    }

    fn render_unified_visualizations(
        &self,
        ui: &mut egui::Ui,
        unified: &api::UnifiedAnalysisSummary,
    ) {
        let visualizations = &unified.visualizations;
        let available = !visualizations.expression_panels.is_empty()
            || !visualizations.effect_panels.is_empty()
            || !visualizations.ordinations.is_empty()
            || !visualizations.dendrograms.is_empty()
            || !visualizations.pathway_points.is_empty();
        egui::CollapsingHeader::new("Interactive native-result visualizations")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(
                        "These plots are presentation views of the completed native result; no statistics are recomputed in WASM. Expression volcano payloads are deliberately bounded for browser responsiveness, while complete rows remain in the authenticated tables and reports.",
                    )
                    .small()
                    .weak(),
                );
                if !available {
                    ui.label(
                        egui::RichText::new(
                            "This persisted job predates browser visualization payloads. Download its reports/tables or submit a new unified job to populate the interactive plots.",
                        )
                        .weak(),
                    );
                    return;
                }

                if !unified.modalities.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Result coverage by modality").strong());
                    let bars = unified
                        .modalities
                        .iter()
                        .enumerate()
                        .map(|(idx, modality)| {
                            Bar::new(idx as f64, modality.result_items as f64).width(0.72)
                        })
                        .collect::<Vec<_>>();
                    Plot::new("web_phase3e_modality_result_counts")
                        .height(250.0)
                        .x_axis_label("Modality index (see summary table above)")
                        .y_axis_label("Result rows / items")
                        .show(ui, |plot_ui| {
                            plot_ui.bar_chart(BarChart::new("Completed native result items", bars));
                        });
                }

                for panel in &visualizations.expression_panels {
                    egui::CollapsingHeader::new(format!(
                        "{} volcano plot · {} tested · {} q≤0.05",
                        panel.modality, panel.tested_features, panel.significant_q05
                    ))
                    .default_open(panel.modality == "Gene expression")
                    .show(ui, |ui| self.render_expression_volcano(ui, panel));
                }

                for panel in &visualizations.effect_panels {
                    egui::CollapsingHeader::new(format!(
                        "{} · {}",
                        panel.modality, panel.family
                    ))
                    .default_open(false)
                    .show(ui, |ui| self.render_effect_panel(ui, panel));
                }

                for ordination in &visualizations.ordinations {
                    egui::CollapsingHeader::new(format!(
                        "{} · {}",
                        ordination.modality, ordination.title
                    ))
                    .default_open(ordination.modality == "Gene expression")
                    .show(ui, |ui| self.render_ordination_panel(ui, ordination));
                }

                for tree in &visualizations.dendrograms {
                    egui::CollapsingHeader::new(format!(
                        "{} · {}",
                        tree.modality, tree.title
                    ))
                    .default_open(false)
                    .show(ui, |ui| self.render_dendrogram_panel(ui, tree));
                }

                if !visualizations.pathway_points.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "Pathway enrichment plot ({})",
                        visualizations.pathway_points.len()
                    ))
                    .default_open(false)
                    .show(ui, |ui| {
                        self.render_pathway_panel(ui, &visualizations.pathway_points)
                    });
                }
            });
    }

    fn render_expression_volcano(
        &self,
        ui: &mut egui::Ui,
        panel: &api::ExpressionVisualizationSummary,
    ) {
        if panel.points.is_empty() {
            ui.label("No finite corrected expression rows are available for this volcano preview.");
            return;
        }
        let mut significant = Vec::new();
        let mut other = Vec::new();
        let mut hover_points = Vec::new();
        for point in &panel.points {
            let y = -point.q_value.max(1e-300).log10();
            let xy = [point.effect, y];
            if point.q_value <= 0.05 {
                significant.push(xy);
            } else {
                other.push(xy);
            }
            hover_points.push((
                point.effect,
                y,
                format!(
                    "{}{}\nβ: {:.4}\np: {:.3e}\nBH q: {:.3e}\nCase mean: {:.4}\nControl mean: {:.4}",
                    point.feature,
                    if point.secondary_id.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", point.secondary_id)
                    },
                    point.effect,
                    point.p_value,
                    point.q_value,
                    point.mean_case,
                    point.mean_control
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let threshold = -(0.05f64).log10();
        let x_min = hover_points
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
        let x_max = hover_points
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let y_max = hover_points
            .iter()
            .map(|point| point.1)
            .fold(threshold, f64::max);
        let x_pad = ((x_max - x_min).abs() * 0.04).max(0.1);
        let accent = ui.visuals().selection.bg_fill;
        Plot::new(format!("web_volcano_{}", safe_plot_id(&panel.modality)))
            .height(390.0)
            .x_axis_label("Adjusted case-vs-control coefficient β")
            .y_axis_label("-log10(BH q)")
            .legend(Legend::default())
            .label_formatter(move |hover| match hover {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_plot_label(position.x, position.y, &formatter_points)
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new(
                        "BH q = 0.05",
                        vec![[x_min - x_pad, threshold], [x_max + x_pad, threshold]],
                    )
                    .allow_hover(false),
                );
                plot_ui.line(
                    Line::new("β = 0", vec![[0.0, 0.0], [0.0, y_max * 1.04]]).allow_hover(false),
                );
                if !other.is_empty() {
                    plot_ui.points(Points::new("Other plotted features", other).radius(2.4));
                }
                if !significant.is_empty() {
                    plot_ui.points(
                        Points::new("BH q ≤ 0.05", significant)
                            .color(accent)
                            .radius(4.0),
                    );
                }
            });
        ui.label(
            egui::RichText::new(format!(
                "Showing {} bounded browser points from {} tested features; {} low-count features were filtered before testing.",
                panel.plotted_features, panel.tested_features, panel.filtered_features
            ))
            .small()
            .weak(),
        );
    }

    fn render_effect_panel(&self, ui: &mut egui::Ui, panel: &api::EffectVisualizationSummary) {
        if panel.points.is_empty() {
            ui.label("No finite corrected effect rows are available for this panel.");
            return;
        }
        let mut coords = Vec::new();
        let mut hover_points = Vec::new();
        for (idx, point) in panel.points.iter().enumerate() {
            let y = (panel.points.len() - idx) as f64;
            coords.push([point.effect, y]);
            hover_points.push((
                point.effect,
                y,
                format!(
                    "{}{}\nβ: {:.4}\np: {:.3e}\nBH q: {:.3e}\nCase mean: {:.4}\nControl mean: {:.4}",
                    point.feature,
                    if point.secondary_id.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", point.secondary_id)
                    },
                    point.effect,
                    point.p_value,
                    point.q_value,
                    point.mean_case,
                    point.mean_control
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new(format!(
            "web_effect_{}_{}",
            safe_plot_id(&panel.modality),
            safe_plot_id(&panel.family)
        ))
        .height(330.0)
        .x_axis_label("Adjusted case-vs-control coefficient β")
        .y_axis_label("Top tests ranked by BH q")
        .label_formatter(move |hover| match hover {
            HoverPosition::NearDataPoint { position, .. } => {
                nearest_plot_label(position.x, position.y, &formatter_points)
            }
            _ => None,
        })
        .show(ui, |plot_ui| {
            plot_ui.line(
                Line::new(
                    "β = 0",
                    vec![[0.0, 0.0], [0.0, panel.points.len() as f64 + 1.0]],
                )
                .allow_hover(false),
            );
            plot_ui.points(
                Points::new("Corrected effects", coords)
                    .color(accent)
                    .radius(4.0),
            );
        });
        ui.label(
            egui::RichText::new(format!(
                "Top {} of {} corrected tests are plotted; hover points for feature-level statistics.",
                panel.points.len(), panel.total_tests
            ))
            .small()
            .weak(),
        );
        egui::Grid::new(format!(
            "web_effect_table_{}_{}",
            safe_plot_id(&panel.modality),
            safe_plot_id(&panel.family)
        ))
        .striped(true)
        .show(ui, |ui| {
            for heading in ["Feature", "β", "BH q", "Case mean", "Control mean"] {
                ui.label(egui::RichText::new(heading).strong());
            }
            ui.end_row();
            for point in panel.points.iter().take(15) {
                ui.monospace(&point.feature);
                ui.label(format!("{:.4}", point.effect));
                ui.label(format!("{:.3e}", point.q_value));
                ui.label(format!("{:.4}", point.mean_case));
                ui.label(format!("{:.4}", point.mean_control));
                ui.end_row();
            }
        });
    }

    fn render_ordination_panel(
        &self,
        ui: &mut egui::Ui,
        ordination: &api::OrdinationVisualizationSummary,
    ) {
        if ordination.permanova_p.is_finite() {
            ui.label(format!(
                "PERMANOVA: pseudo-F {:.3}, p {:.3e}, {} permutations",
                ordination.permanova_f, ordination.permanova_p, ordination.permutations
            ));
        }
        let mut grouped = BTreeMap::<String, Vec<[f64; 2]>>::new();
        let mut hover_points = Vec::new();
        for point in &ordination.points {
            grouped
                .entry(point.group.clone())
                .or_default()
                .push([point.x, point.y]);
            hover_points.push((
                point.x,
                point.y,
                format!(
                    "{}\nGroup: {}\nSubject: {}\n{}: {:.4}\n{}: {:.4}",
                    point.sample_id,
                    point.group,
                    point.subject,
                    ordination.x_label,
                    point.x,
                    ordination.y_label,
                    point.y
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let mut plot = Plot::new(format!(
            "web_ordination_{}_{}",
            safe_plot_id(&ordination.modality),
            safe_plot_id(&ordination.title)
        ))
        .height(400.0)
        .x_axis_label(&ordination.x_label)
        .y_axis_label(&ordination.y_label)
        .label_formatter(move |hover| match hover {
            HoverPosition::NearDataPoint { position, .. } => {
                nearest_plot_label(position.x, position.y, &formatter_points)
            }
            _ => None,
        });
        if grouped.len() <= 12 {
            plot = plot.legend(Legend::default());
        }
        let accent = ui.visuals().selection.bg_fill;
        plot.show(ui, |plot_ui| {
            for (idx, (group, points)) in grouped.into_iter().enumerate() {
                plot_ui.points(
                    Points::new(group, points)
                        .color(web_series_color(idx, accent))
                        .radius(4.2),
                );
            }
        });
    }

    fn render_dendrogram_panel(
        &self,
        ui: &mut egui::Ui,
        tree: &api::DendrogramVisualizationSummary,
    ) {
        if tree.leaves.is_empty() || tree.segments.is_empty() {
            ui.label("No native dendrogram coordinates are available for this result.");
            return;
        }
        ui.label(
            egui::RichText::new(
                "This is the native average-linkage hierarchy already stored in the completed result; the browser only draws its leaf and segment coordinates.",
            )
            .small()
            .weak(),
        );
        let mut grouped = BTreeMap::<String, Vec<[f64; 2]>>::new();
        let mut hover_points = Vec::new();
        for leaf in &tree.leaves {
            grouped
                .entry(leaf.group.clone())
                .or_default()
                .push([0.0, leaf.position]);
            hover_points.push((
                0.0,
                leaf.position,
                format!(
                    "{}\nGroup: {}\nSubject: {}\nCluster order: {}",
                    leaf.sample_id,
                    leaf.group,
                    leaf.subject,
                    leaf.position as usize + 1
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let accent = ui.visuals().selection.bg_fill;
        let branch_color = ui.visuals().weak_text_color();
        let mut plot = Plot::new(format!(
            "web_dendrogram_{}_{}",
            safe_plot_id(&tree.modality),
            safe_plot_id(&tree.title)
        ))
        .height(430.0)
        .x_axis_label(&tree.distance_label)
        .y_axis_label("Cluster leaf order")
        .label_formatter(move |hover| match hover {
            HoverPosition::NearDataPoint { position, .. } => {
                nearest_plot_label(position.x, position.y, &formatter_points)
            }
            _ => None,
        });
        if grouped.len() <= 12 {
            plot = plot.legend(Legend::default());
        }
        plot.show(ui, |plot_ui| {
            for segment in &tree.segments {
                plot_ui.line(
                    Line::new("", vec![[segment.x1, segment.y1], [segment.x2, segment.y2]])
                        .color(branch_color)
                        .allow_hover(false),
                );
            }
            for (idx, (group, points)) in grouped.into_iter().enumerate() {
                plot_ui.points(
                    Points::new(group, points)
                        .color(web_series_color(idx, accent))
                        .radius(4.0),
                );
            }
        });
    }

    fn render_pathway_panel(&self, ui: &mut egui::Ui, points: &[api::PathwayVisualizationPoint]) {
        let mut significant = Vec::new();
        let mut other = Vec::new();
        let mut hover_points = Vec::new();
        for point in points {
            let x = point.odds_ratio.max(1e-300).ln();
            let y = -point.q_value.max(1e-300).log10();
            if point.q_value <= 0.05 {
                significant.push([x, y]);
            } else {
                other.push([x, y]);
            }
            hover_points.push((
                x,
                y,
                format!(
                    "{}\n{}\nOverlap: {}/{}\nOdds ratio: {:.3}\np: {:.3e}\nBH q: {:.3e}",
                    point.pathway,
                    point.description,
                    point.overlap,
                    point.measured_set_size,
                    point.odds_ratio,
                    point.p_value,
                    point.q_value
                ),
            ));
        }
        let formatter_points = hover_points.clone();
        let accent = ui.visuals().selection.bg_fill;
        Plot::new("web_pathway_enrichment")
            .height(390.0)
            .x_axis_label("ln odds ratio")
            .y_axis_label("-log10(BH q)")
            .legend(Legend::default())
            .label_formatter(move |hover| match hover {
                HoverPosition::NearDataPoint { position, .. } => {
                    nearest_plot_label(position.x, position.y, &formatter_points)
                }
                _ => None,
            })
            .show(ui, |plot_ui| {
                if !other.is_empty() {
                    plot_ui.points(Points::new("Other pathways", other).radius(3.0));
                }
                if !significant.is_empty() {
                    plot_ui.points(
                        Points::new("BH q ≤ 0.05", significant)
                            .color(accent)
                            .radius(4.2),
                    );
                }
            });
        egui::Grid::new("web_pathway_top_table")
            .striped(true)
            .show(ui, |ui| {
                for heading in ["Pathway", "Overlap", "Odds ratio", "BH q"] {
                    ui.label(egui::RichText::new(heading).strong());
                }
                ui.end_row();
                for point in points.iter().take(15) {
                    ui.monospace(&point.pathway);
                    ui.label(format!("{}/{}", point.overlap, point.measured_set_size));
                    ui.label(format!("{:.3}", point.odds_ratio));
                    ui.label(format!("{:.3e}", point.q_value));
                    ui.end_row();
                }
            });
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let snapshot = self.remote.borrow().clone();
        let connected = snapshot.health.is_some();
        let phase_label = "Web · Phase 3E.5";
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
            "Phase 3E.5 publishes the primary corrected cohort result before optional PBMC/confounding sensitivity refits, then advances the same frozen job to the full scientific result before interactive/report/table finalization.",
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
            ui.label(egui::RichText::new("What works in Phase 3E.5").strong());
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
                "Frozen Phase 3E jobs reuse the native cross-modal QC engine, preserve reviewed exclusions, and execute every available corrected native modality through the existing unified cohort-analysis function.",
                "The same completed unified result now renders interactive browser plots, authenticated simplified/full self-contained HTML reports, and the native technical table set without recomputing statistics in WASM.",
                "All retained artifacts from one job can be downloaded as a single authenticated ZIP assembled on demand; individual downloads remain available.",
                "Persisted terminal jobs can be deleted independently to remove their frozen inputs and private artifacts without deleting the whole project.",
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
                "A public/LAN production deployment; localhost remains the safe development default until HTTPS/reverse-proxy hardening exists.",
                "S3/MinIO storage or any user/account system.",
                "Automatic promotion of arbitrary metadata/GMT files; cohort-defining metadata and pathway gene sets remain deliberate assignments.",
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
            "The browser now supports private objects, native mapping, frozen unified native analysis, and authenticated result/report downloads. This tab remains a lightweight shell until its dedicated native workflow is connected.",
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
        ui.heading("Cohort Analysis — frozen native unified workflow");
        ui.label(
            "Phase 3E.5 keeps the validated high-throughput ingest/mapping and frozen unified native analysis, publishes primary corrected results before optional PBMC/confounding refits finish, and then enriches the same frozen job with sensitivity diagnostics, interactive views, reports, and technical exports.",
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
                    state.reviewed_exclusions.clear();
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
                "Uploaded bytes remain stored under opaque object UUIDs. Standard native 4C filenames can seed role/sample-ID assignments after content validation; custom or ambiguous inputs remain unassigned for review. Filenames remain metadata only and never become storage keys or authorization credentials.",
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
                    "For local validation, use harmless synthetic inputs first. Mapping, native QC review, the full frozen native unified analysis, report generation, authenticated downloads, and persisted-job cleanup are enabled. Public deployment hardening and S3/MinIO remain disabled.",
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
                    "The server materializes the assigned primary metadata in private scratch space, parses it with the existing native metadata loader, constructs the native ProjectScan representation from explicit object roles/sample IDs, and runs the existing build_mapping() audit. A successful current audit becomes the gate for a frozen native unified job.",
                );
                ui.add_space(5.0);
                ui.label(egui::RichText::new("Metadata-defined Case/Control comparison").strong());
                ui.label(
                    egui::RichText::new(
                        "The grouping field and its categorical values are read directly from the assigned primary metadata. Select one or more disjoint values for Case and Control. Those exact selections are frozen into the job and define the cohorts used by all downstream corrected modality comparisons, supporting diagnostics, candidate prioritization, and both reports. Metadata rows assigned to neither side remain outside this comparison.",
                    )
                    .small(),
                );

                let metadata_profile_enabled = snapshot
                    .health
                    .as_ref()
                    .map(|health| health.metadata_profile_enabled)
                    .unwrap_or(false);
                let has_credentials = !snapshot.recovery_project_id.trim().is_empty()
                    && !snapshot.recovery_token.trim().is_empty();
                let has_primary_metadata = snapshot
                    .objects
                    .iter()
                    .any(|object| object.role == "primary_metadata");
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !snapshot.busy
                                && metadata_profile_enabled
                                && has_credentials
                                && has_primary_metadata,
                            egui::Button::new("Reload primary metadata fields"),
                        )
                        .clicked()
                    {
                        self.refresh_remote_metadata_profile(ui.ctx().clone());
                    }
                    if let Some(profile) = snapshot.metadata_profile.as_ref() {
                        ui.label(format!(
                            "{} metadata rows · ID column {} · {} columns",
                            profile.metadata_rows,
                            profile.metadata_id_column,
                            profile.columns.len()
                        ));
                        ui.monospace(short_hash(&profile.metadata_sha256));
                    } else if has_primary_metadata {
                        ui.label(
                            egui::RichText::new(
                                "Primary metadata is assigned; load its fields to configure the comparison.",
                            )
                            .weak(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(
                                "Assign exactly one uploaded object as Primary metadata first.",
                            )
                            .weak(),
                        );
                    }
                });

                if let Some(profile) = snapshot.metadata_profile.as_ref() {
                    if !profile.validation_errors.is_empty() || !profile.validation_warnings.is_empty() {
                        egui::CollapsingHeader::new(format!(
                            "Metadata validation notes ({} error(s), {} warning(s))",
                            profile.validation_errors.len(),
                            profile.validation_warnings.len()
                        ))
                        .show(ui, |ui| {
                            for error in &profile.validation_errors {
                                ui.label(
                                    egui::RichText::new(format!("Error: {error}"))
                                        .color(egui::Color32::LIGHT_RED),
                                );
                            }
                            for warning in &profile.validation_warnings {
                                ui.label(format!("Warning: {warning}"));
                            }
                        });
                    }

                    let candidate_columns = profile
                        .columns
                        .iter()
                        .filter(|column| {
                            column.name.as_str() != profile.metadata_id_column.as_str()
                                && column.distinct_nonempty_values >= 2
                        })
                        .collect::<Vec<_>>();
                    let mut selected_column = snapshot.comparison_field.clone();
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Grouping field");
                        egui::ComboBox::from_id_salt("web_metadata_comparison_column")
                            .selected_text(if selected_column.is_empty() {
                                "(choose metadata field)".to_string()
                            } else {
                                selected_column.clone()
                            })
                            .width(300.0)
                            .show_ui(ui, |ui| {
                                for column in &candidate_columns {
                                    let label = if column.comparison_ready {
                                        format!(
                                            "{} ({} values)",
                                            column.name, column.distinct_nonempty_values
                                        )
                                    } else {
                                        format!(
                                            "{} ({} values; high-cardinality)",
                                            column.name, column.distinct_nonempty_values
                                        )
                                    };
                                    ui.selectable_value(
                                        &mut selected_column,
                                        column.name.clone(),
                                        label,
                                    );
                                }
                            });
                        if let Some(suggested) = profile.suggested_comparison_column.as_deref() {
                            ui.label(
                                egui::RichText::new(format!("Suggested: {suggested}"))
                                    .small()
                                    .weak(),
                            );
                        }
                    });
                    if selected_column != snapshot.comparison_field {
                        let mut state = self.remote.borrow_mut();
                        set_comparison_column(&mut state, selected_column.clone());
                    }

                    let (mut case_values, mut control_values) = {
                        let state = self.remote.borrow();
                        (state.case_values.clone(), state.control_values.clone())
                    };
                    if let Some(column) = profile
                        .columns
                        .iter()
                        .find(|column| column.name == selected_column)
                    {
                        if column.values_truncated {
                            ui.label(
                                egui::RichText::new(format!(
                                    "This field has {} distinct non-empty values; only the first {} are exposed. Prefer a lower-cardinality categorical field for cohort comparison.",
                                    column.distinct_nonempty_values,
                                    column.values.len()
                                ))
                                .color(egui::Color32::LIGHT_RED),
                            );
                        }
                        let mut selection_changed = false;
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Case values");
                            egui::ComboBox::from_id_salt("web_metadata_case_values")
                                .selected_text(selection_label(&case_values))
                                .width(280.0)
                                .show_ui(ui, |ui| {
                                    for item in &column.values {
                                        let mut selected = case_values.contains(&item.value);
                                        if ui
                                            .checkbox(
                                                &mut selected,
                                                format!("{}  (n={})", item.value, item.count),
                                            )
                                            .changed()
                                        {
                                            selection_changed = true;
                                            if selected {
                                                case_values.insert(item.value.clone());
                                                control_values.remove(&item.value);
                                            } else {
                                                case_values.remove(&item.value);
                                            }
                                        }
                                    }
                                });
                            ui.label("Control values");
                            egui::ComboBox::from_id_salt("web_metadata_control_values")
                                .selected_text(selection_label(&control_values))
                                .width(280.0)
                                .show_ui(ui, |ui| {
                                    for item in &column.values {
                                        let mut selected = control_values.contains(&item.value);
                                        if ui
                                            .checkbox(
                                                &mut selected,
                                                format!("{}  (n={})", item.value, item.count),
                                            )
                                            .changed()
                                        {
                                            selection_changed = true;
                                            if selected {
                                                control_values.insert(item.value.clone());
                                                case_values.remove(&item.value);
                                            } else {
                                                control_values.remove(&item.value);
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
                            let mut state = self.remote.borrow_mut();
                            state.case_values = case_values.clone();
                            state.control_values = control_values.clone();
                            state.mapping_audit = None;
                            state.status_message =
                                "Comparison selection changed; rerun the mapping audit before analysis."
                                    .to_string();
                        }

                        let case_rows = selected_metadata_rows(column, &case_values);
                        let control_rows = selected_metadata_rows(column, &control_values);
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
                                    "Select at least one value for each group before validating the mapping.",
                                )
                                .color(egui::Color32::LIGHT_RED),
                            );
                        }
                    }
                }

                let mapping_enabled = snapshot
                    .health
                    .as_ref()
                    .map(|health| health.mapping_enabled)
                    .unwrap_or(false);
                let comparison_ready = !snapshot.comparison_field.trim().is_empty()
                    && !snapshot.case_values.is_empty()
                    && !snapshot.control_values.is_empty();
                if ui
                    .add_enabled(
                        !snapshot.busy
                            && mapping_enabled
                            && has_credentials
                            && comparison_ready,
                        egui::Button::new("Validate comparison & run mapping audit"),
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
                ui.label(egui::RichText::new("5. Frozen unified analysis job").strong());
                ui.label(
                    "Phase 3E freezes the validated opaque-object manifest and analysis configuration, reruns native cross-modal QC, applies only reviewed sample exclusions, and executes the existing native unified cohort-analysis workflow. Simplified and full HTML reports are rendered from that exact completed native result; the browser does not implement a second statistical engine.",
                );
                ui.add_space(5.0);
                {
                    let mut state = self.remote.borrow_mut();
                    egui::Grid::new("web_phase3e_job_settings").show(ui, |ui| {
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
                        ui.label("PBMC batch column (optional)");
                        ui.add_sized(
                            [260.0, 22.0],
                            egui::TextEdit::singleline(&mut state.analysis_batch_column)
                                .hint_text("batch"),
                        );
                        ui.end_row();
                        ui.label("PBMC family column (optional)");
                        ui.add_sized(
                            [260.0, 22.0],
                            egui::TextEdit::singleline(&mut state.analysis_family_column)
                                .hint_text("family_id"),
                        );
                        ui.end_row();
                        ui.label("PBMC sensitivity column (optional)");
                        ui.add_sized(
                            [260.0, 22.0],
                            egui::TextEdit::singleline(&mut state.analysis_sensitivity_column)
                                .hint_text("sensitivity_group"),
                        );
                        ui.end_row();
                        ui.label("Pathway foreground");
                        ui.horizontal(|ui| {
                            ui.label("BH q ≤");
                            ui.add(
                                egui::DragValue::new(&mut state.analysis_pathway_q_threshold)
                                    .range(0.001..=0.25)
                                    .speed(0.005),
                            );
                            ui.label("|adjusted log2FC| ≥");
                            ui.add(
                                egui::DragValue::new(&mut state.analysis_pathway_effect_threshold)
                                    .range(0.0..=5.0)
                                    .speed(0.1),
                            );
                        });
                        ui.end_row();
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(
                            &mut state.analysis_run_exploratory,
                            "Supporting expression direction-concordance checks",
                        );
                        ui.checkbox(
                            &mut state.analysis_run_pbmc_diagnostics,
                            "PBMC / confounding diagnostics",
                        );
                    });
                }
                ui.label(
                    egui::RichText::new(
                        "All frozen corrected modalities that are actually available are selected automatically. Pathway ORA is enabled only when both a merged gene-expression table and a deliberately assigned Pathway GMT object are present. Standard folder ingest still skips GMT files; add one with Choose files… and assign its role explicitly.",
                    )
                    .small()
                    .weak(),
                );

                if let Some(qc) = snapshot
                    .active_job
                    .as_ref()
                    .and_then(|job| job.result.as_ref())
                    .and_then(|result| result.qc.as_ref())
                {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Reviewed QC exclusions for the next run").strong());
                    ui.label(
                        egui::RichText::new(
                            "QC flags are review prompts, not automatic deletions. Check only samples you want excluded from the next frozen unified run. The server reruns QC on the new frozen snapshot and preserves the audit plus these manual overrides in the completed result/report.",
                        )
                        .small(),
                    );
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Use consensus flags as reviewed exclusions").clicked() {
                            self.remote.borrow_mut().reviewed_exclusions =
                                qc.consensus_ids.iter().cloned().collect();
                        }
                        if ui.button("Clear reviewed exclusions").clicked() {
                            self.remote.borrow_mut().reviewed_exclusions.clear();
                        }
                        ui.label(format!(
                            "{} reviewed exclusion(s)",
                            snapshot.reviewed_exclusions.len()
                        ));
                    });
                    if !qc.flagged_samples.is_empty() {
                        egui::CollapsingHeader::new(format!(
                            "Review flagged samples ({})",
                            qc.flagged_samples.len()
                        ))
                        .show(ui, |ui| {
                            egui::Grid::new("web_phase3e_qc_review")
                                .striped(true)
                                .show(ui, |ui| {
                                    for heading in [
                                        "Exclude",
                                        "Sample",
                                        "Group",
                                        "Flagged modalities",
                                        "Consensus",
                                    ] {
                                        ui.label(egui::RichText::new(heading).strong());
                                    }
                                    ui.end_row();
                                    for sample in &qc.flagged_samples {
                                        let mut excluded = snapshot
                                            .reviewed_exclusions
                                            .contains(&sample.sample_id);
                                        if ui.checkbox(&mut excluded, "").changed() {
                                            let mut state = self.remote.borrow_mut();
                                            if excluded {
                                                state
                                                    .reviewed_exclusions
                                                    .insert(sample.sample_id.clone());
                                            } else {
                                                state.reviewed_exclusions.remove(&sample.sample_id);
                                            }
                                        }
                                        ui.monospace(&sample.sample_id);
                                        ui.label(&sample.group);
                                        ui.label(if sample.flagged_names.is_empty() {
                                            sample.flagged_modalities.to_string()
                                        } else {
                                            sample.flagged_names.join(", ")
                                        });
                                        ui.label(if sample.consensus_flag { "yes" } else { "no" });
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                } else if !snapshot.reviewed_exclusions.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "Reviewed exclusions are currently populated but no active QC result is visible. Open the QC job they came from and verify them before submitting another unified run.",
                        )
                        .color(egui::Color32::LIGHT_RED),
                    );
                }

                let job_enabled = snapshot
                    .health
                    .as_ref()
                    .map(|health| {
                        health.analysis_enabled
                            && health.persistent_jobs
                            && health.unified_analysis_enabled
                    })
                    .unwrap_or(false);
                let has_credentials = !snapshot.recovery_project_id.trim().is_empty()
                    && !snapshot.recovery_token.trim().is_empty();
                let current_mapping = snapshot.mapping_audit.is_some();
                ui.add_space(5.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !snapshot.busy && job_enabled && has_credentials && current_mapping,
                            egui::Button::new("Freeze snapshot & run unified analysis"),
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
                    let can_cancel = snapshot.active_job.as_ref().is_some_and(|job| {
                        matches!(
                            job.summary.status.as_str(),
                            "queued" | "preparing" | "running" | "primary_results_ready"
                        )
                    });
                    if ui
                        .add_enabled(can_cancel, egui::Button::new("Request cancellation"))
                        .clicked()
                    {
                        self.cancel_active_remote_job(ui.ctx().clone());
                    }
                });

                if !snapshot.jobs.is_empty() {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(format!(
                        "Recent persisted jobs ({})",
                        snapshot.jobs.len()
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::Grid::new("web_phase3e_job_history")
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Job ID").strong());
                                ui.label(egui::RichText::new("Status").strong());
                                ui.label(egui::RichText::new("Progress").strong());
                                ui.label(egui::RichText::new("Actions").strong());
                                ui.end_row();
                                for job in &snapshot.jobs {
                                    ui.monospace(&job.job_id);
                                    ui.label(&job.status);
                                    ui.label(&job.progress_message);
                                    ui.horizontal(|ui| {
                                        if ui.small_button("Open results").clicked() {
                                            self.open_remote_job(
                                                job.job_id.clone(),
                                                ui.ctx().clone(),
                                            );
                                        }
                                        let terminal = matches!(
                                            job.status.as_str(),
                                            "complete" | "failed" | "cancelled"
                                        );
                                        if terminal {
                                            let armed = snapshot
                                                .job_delete_armed
                                                .as_deref()
                                                == Some(job.job_id.as_str());
                                            let label = if armed {
                                                "Confirm delete"
                                            } else {
                                                "Delete stored job"
                                            };
                                            if ui.small_button(label).clicked() {
                                                if armed {
                                                    self.delete_stored_remote_job(
                                                        job.job_id.clone(),
                                                        ui.ctx().clone(),
                                                    );
                                                } else {
                                                    self.remote.borrow_mut().job_delete_armed =
                                                        Some(job.job_id.clone());
                                                }
                                            }
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
                }

                if let Some(job) = snapshot.active_job.as_ref() {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.label(egui::RichText::new("Opened job results").heading());
                    egui::Grid::new("web_phase3e_active_job")
                        .striped(true)
                        .show(ui, |ui| {
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
                    if job.summary.status == "primary_results_ready" {
                        ui.add_space(6.0);
                        egui::Frame::group(ui.style())
                            .fill(ui.visuals().faint_bg_color)
                            .inner_margin(10)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        "Primary corrected results ready — you can review the main cohort result now.",
                                    )
                                    .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "PBMC/confounding sensitivity diagnostics are still running. Primary modality statistics are final; robustness annotations and diagnostic outputs may be enriched when that optional sensitivity stage finishes.",
                                    )
                                    .small(),
                                );
                            });
                    } else if job.summary.status == "results_ready" {
                        ui.add_space(6.0);
                        egui::Frame::group(ui.style())
                            .fill(ui.visuals().faint_bg_color)
                            .inner_margin(10)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(
                                        "Scientific result ready — you can review the complete cohort result now.",
                                    )
                                    .strong(),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "Interactive visualization enrichment and optional HTML/technical export finalization are still running in the background. They do not change the frozen statistics below.",
                                    )
                                    .small(),
                                );
                            });
                    }
                    if let Some(error) = job.summary.error.as_ref() {
                        ui.label(egui::RichText::new(error).color(egui::Color32::LIGHT_RED));
                    }
                    if let Some(result) = job.result.as_ref() {
                        ui.add_space(6.0);
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
                        }

                        if let Some(unified) = result.unified.as_ref() {
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new("Unified native result").strong());
                            ui.label(format!(
                                "{} · {} case / {} control · {} reviewed QC exclusion(s)",
                                unified.comparison_name,
                                unified.case_samples,
                                unified.control_samples,
                                unified.excluded_sample_ids.len()
                            ));
                            egui::Grid::new("web_phase3e_modality_summary")
                                .striped(true)
                                .show(ui, |ui| {
                                    for heading in ["Modality", "Status", "Result rows/items"] {
                                        ui.label(egui::RichText::new(heading).strong());
                                    }
                                    ui.end_row();
                                    for modality in &unified.modalities {
                                        ui.label(&modality.modality);
                                        ui.label(&modality.status);
                                        ui.label(modality.result_items.to_string());
                                        ui.end_row();
                                    }
                                });
                            let pbmc_status = if unified.pbmc_diagnostics_available {
                                "ready"
                            } else if unified.pbmc_diagnostics_requested
                                && job.summary.status == "primary_results_ready"
                            {
                                "running"
                            } else if unified.pbmc_diagnostics_requested {
                                "unavailable"
                            } else {
                                "not requested"
                            };
                            ui.label(format!(
                                "Supporting direction checks: gene {} · transcript {} · PBMC/confounding diagnostics {}",
                                yes_no(unified.exploratory_gene_available),
                                yes_no(unified.exploratory_transcript_available),
                                pbmc_status,
                            ));
                            let visualizations_ready = !unified.visualizations.expression_panels.is_empty()
                                || !unified.visualizations.effect_panels.is_empty()
                                || !unified.visualizations.ordinations.is_empty()
                                || !unified.visualizations.dendrograms.is_empty()
                                || !unified.visualizations.pathway_points.is_empty();
                            if matches!(
                                job.summary.status.as_str(),
                                "primary_results_ready" | "results_ready"
                            ) && !visualizations_ready {
                                ui.label(
                                    egui::RichText::new(
                                        "Interactive plots are not available yet; modality summaries and prioritized signals above can already be reviewed while remaining sensitivity/export work continues.",
                                    )
                                    .small()
                                    .weak(),
                                );
                            } else {
                                self.render_unified_visualizations(ui, unified);
                            }
                            if !unified.candidates.is_empty() {
                                egui::CollapsingHeader::new(format!(
                                    "Prioritized signals ({})",
                                    unified.candidates.len()
                                ))
                                .show(ui, |ui| {
                                    egui::Grid::new("web_phase3e_candidate_summary")
                                        .striped(true)
                                        .show(ui, |ui| {
                                            for heading in [
                                                "Modality",
                                                "Feature",
                                                "Effect",
                                                "BH q",
                                                "Evidence",
                                                "Robustness",
                                            ] {
                                                ui.label(egui::RichText::new(heading).strong());
                                            }
                                            ui.end_row();
                                            for candidate in &unified.candidates {
                                                ui.label(&candidate.modality);
                                                ui.label(&candidate.feature);
                                                ui.label(format!("{:.4}", candidate.effect));
                                                ui.label(format!("{:.3e}", candidate.q_value));
                                                ui.label(&candidate.evidence);
                                                ui.label(
                                                    candidate
                                                        .robustness
                                                        .as_deref()
                                                        .unwrap_or("—"),
                                                );
                                                ui.end_row();
                                            }
                                        });
                                });
                            }
                        } else if let Some(gene) = result.gene.as_ref() {
                            ui.label(format!(
                                "Legacy representative corrected gene lane: {} tested features · {} filtered · {} modeled samples · {} subjects",
                                gene.tested_features,
                                gene.filtered_features,
                                gene.n_model_samples,
                                gene.n_subjects
                            ));
                        }

                        if !result.warnings.is_empty() {
                            egui::CollapsingHeader::new(format!(
                                "Job warnings ({})",
                                result.warnings.len()
                            ))
                            .show(ui, |ui| {
                                for warning in &result.warnings {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("•");
                                        ui.label(warning);
                                    });
                                }
                            });
                        }
                        if !result.artifacts.is_empty() {
                            ui.add_space(5.0);
                            egui::CollapsingHeader::new(format!(
                                "Authenticated result & report downloads ({})",
                                result.artifacts.len()
                            ))
                            .default_open(true)
                            .show(ui, |ui| {
                                let total_bytes = result
                                    .artifacts
                                    .iter()
                                    .map(|artifact| artifact.byte_size)
                                    .sum::<u64>();
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .button(format!(
                                            "Download all as ZIP · {} files · {}",
                                            result.artifacts.len(),
                                            format_bytes(total_bytes)
                                        ))
                                        .on_hover_text("Build one authenticated ZIP on demand from every retained artifact for this job. The archive is not persisted as a second copy on the server and includes artifact_manifest.json with names, sizes, SHA-256 values, and media types.")
                                        .clicked()
                                    {
                                        self.download_all_remote_artifacts(
                                            job.summary.job_id.clone(),
                                            ui.ctx().clone(),
                                        );
                                    }
                                    ui.label(
                                        egui::RichText::new(
                                            "The recovery token stays in the Authorization header; it is not placed in the download URL.",
                                        )
                                        .small()
                                        .weak(),
                                    );
                                });
                                ui.add_space(4.0);
                                egui::Grid::new("web_phase3e_artifacts")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        for heading in ["Artifact", "Size", "SHA-256", ""] {
                                            ui.label(egui::RichText::new(heading).strong());
                                        }
                                        ui.end_row();
                                        for artifact in &result.artifacts {
                                            ui.label(&artifact.name);
                                            ui.label(format_bytes(artifact.byte_size));
                                            ui.monospace(short_hash(&artifact.sha256));
                                            if ui.small_button("Download").clicked() {
                                                self.download_remote_artifact(
                                                    job.summary.job_id.clone(),
                                                    artifact.artifact_id.clone(),
                                                    artifact.name.clone(),
                                                    ui.ctx().clone(),
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                        }
                    } else if matches!(job.summary.status.as_str(), "complete") {
                        ui.label(
                            egui::RichText::new(
                                "This completed persisted job has no readable result payload.",
                            )
                            .weak(),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new(
                            "No persisted job is open. Submit a new unified job or use Open results in the job history above.",
                        )
                        .small()
                        .weak(),
                    );
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
            ui.label(egui::RichText::new("Phase-3E capabilities:").strong());
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
    ("pathway_gmt", "Pathway gene-set GMT"),
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

fn optional_trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn save_browser_download(filename: &str, bytes: &[u8]) -> Result<(), String> {
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(array.as_ref());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts)
        .map_err(|_| "browser could not construct the download blob".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "browser could not create a private download URL".to_string())?;
    let window = web_sys::window().ok_or_else(|| "browser window is unavailable".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "browser document is unavailable".to_string())?;
    let body = document
        .body()
        .ok_or_else(|| "browser document body is unavailable".to_string())?;
    let anchor = document
        .create_element("a")
        .map_err(|_| "browser could not create the download element".to_string())?;
    anchor
        .set_attribute("href", &url)
        .map_err(|_| "browser could not attach the download URL".to_string())?;
    anchor
        .set_attribute("download", filename)
        .map_err(|_| "browser could not set the download filename".to_string())?;
    anchor
        .set_attribute("style", "display:none")
        .map_err(|_| "browser could not hide the temporary download element".to_string())?;
    body.append_child(&anchor)
        .map_err(|_| "browser could not attach the download element".to_string())?;
    let html = anchor
        .clone()
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| "browser download element is not clickable".to_string())?;
    html.click();
    let _ = body.remove_child(&anchor);
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

fn clear_metadata_profile_state(state: &mut RemoteApiState) {
    state.metadata_profile = None;
    state.comparison_field.clear();
    state.case_values.clear();
    state.control_values.clear();
}

fn apply_metadata_profile(state: &mut RemoteApiState, profile: MetadataProfileResponse) {
    let same_source = state.metadata_profile.as_ref().is_some_and(|current| {
        current.metadata_object_id == profile.metadata_object_id
            && current.metadata_sha256 == profile.metadata_sha256
    });
    let current_column_valid = profile.columns.iter().any(|column| {
        column.name == state.comparison_field && column.distinct_nonempty_values >= 2
    });
    let selected_column = if same_source && current_column_valid {
        state.comparison_field.clone()
    } else {
        profile
            .suggested_comparison_column
            .clone()
            .unwrap_or_default()
    };

    state.metadata_profile = Some(profile);
    if !same_source || selected_column != state.comparison_field {
        state.comparison_field = selected_column;
        state.case_values.clear();
        state.control_values.clear();
    }
    reconcile_comparison_values(state);
}

fn set_comparison_column(state: &mut RemoteApiState, column: String) {
    if state.comparison_field == column {
        return;
    }
    state.comparison_field = column;
    state.case_values.clear();
    state.control_values.clear();
    reconcile_comparison_values(state);
    state.mapping_audit = None;
    state.status_message =
        "Comparison field changed; select groups and rerun the mapping audit before analysis."
            .to_string();
}

fn reconcile_comparison_values(state: &mut RemoteApiState) {
    let Some(profile) = state.metadata_profile.as_ref() else {
        state.case_values.clear();
        state.control_values.clear();
        return;
    };
    let Some(column) = profile
        .columns
        .iter()
        .find(|column| column.name == state.comparison_field)
    else {
        state.case_values.clear();
        state.control_values.clear();
        return;
    };
    let available = column
        .values
        .iter()
        .map(|item| item.value.clone())
        .collect::<BTreeSet<_>>();
    state.case_values.retain(|value| available.contains(value));
    state
        .control_values
        .retain(|value| available.contains(value));
    for value in state.case_values.clone() {
        state.control_values.remove(&value);
    }

    if state.case_values.is_empty() && state.control_values.is_empty() {
        let case = column
            .values
            .iter()
            .find(|item| item.value.eq_ignore_ascii_case("case"))
            .map(|item| item.value.clone());
        let control = column
            .values
            .iter()
            .find(|item| item.value.eq_ignore_ascii_case("control"))
            .map(|item| item.value.clone());
        if let (Some(case), Some(control)) = (case, control) {
            if !case.eq_ignore_ascii_case(&control) {
                state.case_values.insert(case);
                state.control_values.insert(control);
            }
        }
    }
}

fn selection_label(values: &BTreeSet<String>) -> String {
    match values.len() {
        0 => "(select one or more)".to_string(),
        1 => values.iter().next().cloned().unwrap_or_default(),
        n => format!("{n} values selected"),
    }
}

fn selected_metadata_rows(
    column: &api::MetadataColumnProfile,
    selected: &BTreeSet<String>,
) -> usize {
    column
        .values
        .iter()
        .filter(|item| selected.contains(&item.value))
        .map(|item| item.count)
        .sum()
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
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
    for _ in 0..14400usize {
        gloo_timers::future::TimeoutFuture::new(2000).await;
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
    let jobs = api::list_jobs(&project_id, &token).await;
    let project = api::get_project(&project_id, &token).await;
    {
        let mut state = remote.borrow_mut();
        if let Ok(jobs) = jobs {
            state.jobs = jobs;
        }
        if let Ok(project) = project {
            state.project = remote_project_from_response(&project);
        }
    }
    ctx.request_repaint();
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

fn safe_plot_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn nearest_plot_label(x: f64, y: f64, points: &[(f64, f64, String)]) -> Option<String> {
    points
        .iter()
        .min_by(|a, b| {
            let da = (a.0 - x).powi(2) + (a.1 - y).powi(2);
            let db = (b.0 - x).powi(2) + (b.1 - y).powi(2);
            da.total_cmp(&db)
        })
        .map(|(_, _, label)| label.clone())
}

fn web_series_color(index: usize, accent: egui::Color32) -> egui::Color32 {
    match index % 8 {
        0 => accent,
        1 => egui::Color32::from_rgb(98, 170, 230),
        2 => egui::Color32::from_rgb(230, 155, 82),
        3 => egui::Color32::from_rgb(130, 200, 145),
        4 => egui::Color32::from_rgb(205, 125, 190),
        5 => egui::Color32::from_rgb(220, 205, 95),
        6 => egui::Color32::from_rgb(125, 205, 205),
        _ => egui::Color32::from_rgb(190, 150, 225),
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
                        "Phase 3E exposes private uploaded-object handles, validated assignments, frozen unified results, technical tables, and authenticated report downloads. A dedicated browser-side merge/export workflow remains separate from the unified analysis job.",
                    ),
                    WebTab::Explore => self.placeholder(
                        ui,
                        "Explore & QC",
                        "Phase 3E runs cross-modal QC inside the same frozen native job and preserves the reviewed exclusions used downstream. A dedicated interactive Explore & QC workspace is still separate UI work; the scientific QC engine remains native Rust.",
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
