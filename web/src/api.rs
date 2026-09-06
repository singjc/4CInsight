//! Same-origin Phase-3E.6 client for the local `cfs-server` API.

use gloo_net::http::{Request, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const API_ROOT: &str = "/api";

#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub app_version: String,
    pub api_version: u32,
    pub default_ttl_hours: u64,
    pub max_ttl_hours: u64,
    pub upload_enabled: bool,
    pub mapping_enabled: bool,
    pub analysis_enabled: bool,
    pub persistent_jobs: bool,
    pub representative_gene_enabled: bool,
    #[serde(default)]
    pub unified_analysis_enabled: bool,
    #[serde(default)]
    pub metadata_profile_enabled: bool,
    #[serde(default)]
    pub report_download_enabled: bool,
    #[serde(default)]
    pub bulk_artifact_download_enabled: bool,
    #[serde(default)]
    pub result_visualizations_enabled: bool,
    #[serde(default)]
    pub job_deletion_enabled: bool,
    pub batch_upload_enabled: bool,
    pub storage_backend: String,
    #[serde(default)]
    pub quota_profile: String,
    pub max_file_bytes: u64,
    pub max_project_bytes: u64,
    pub max_objects: usize,
    pub max_jobs_per_project: usize,
    pub max_concurrent_jobs: usize,
    pub max_concurrent_uploads: usize,
    pub max_job_artifact_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateProjectRequest {
    pub display_name: Option<String>,
    pub retention_hours: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSummaryResponse {
    pub project_id: String,
    pub display_name: String,
    pub created_unix_seconds: u64,
    pub expires_unix_seconds: u64,
    pub uploaded_objects: usize,
    pub storage_bytes: u64,
    pub job_count: usize,
    #[serde(default)]
    pub job_snapshot_bytes: u64,
    pub job_artifact_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectResponse {
    pub project: ProjectSummaryResponse,
    pub recovery_token: String,
    pub recovery_token_warning: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteProjectResponse {
    pub project_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObjectSummaryResponse {
    pub object_id: String,
    pub original_name: String,
    pub byte_size: u64,
    pub sha256: String,
    pub uploaded_unix_seconds: u64,
    pub content_type: Option<String>,
    pub role: String,
    pub sample_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadObjectResponse {
    pub object: ObjectSummaryResponse,
    pub project: ProjectSummaryResponse,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssignObjectRequest {
    pub role: String,
    pub sample_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssignObjectResponse {
    pub object: ObjectSummaryResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AutoAssignObjectsResponse {
    pub assigned: usize,
    pub already_assigned: usize,
    pub unresolved: usize,
    pub validation_failed: usize,
    pub objects: Vec<ObjectSummaryResponse>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataValueCount {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataColumnProfile {
    pub name: String,
    pub distinct_nonempty_values: usize,
    pub comparison_ready: bool,
    pub values_truncated: bool,
    pub values: Vec<MetadataValueCount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetadataProfileResponse {
    pub metadata_object_id: String,
    pub metadata_sha256: String,
    pub metadata_rows: usize,
    pub metadata_id_column: String,
    pub suggested_comparison_column: Option<String>,
    pub columns: Vec<MetadataColumnProfile>,
    pub validation_errors: Vec<String>,
    pub validation_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingAuditRequest {
    pub comparison_column: Option<String>,
    pub case_values: Vec<String>,
    pub control_values: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MappingAuditResponse {
    pub mapping_snapshot_sha256: String,
    pub metadata_rows: usize,
    pub canonical_samples: usize,
    pub metadata_id_column: String,
    pub comparison_name: Option<String>,
    pub case_samples: Option<usize>,
    pub control_samples: Option<usize>,
    pub matched_files: usize,
    pub unmatched_files: usize,
    pub ambiguous_files: usize,
    pub metadata_only_samples: usize,
    pub modality_coverage: Vec<ModalityCoverageSummary>,
    pub rows: Vec<MappingRowSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModalityCoverageSummary {
    pub modality: String,
    pub assigned_files: usize,
    pub matched_files: usize,
    pub samples_with_files: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MappingRowSummary {
    pub sample_id: String,
    pub status: String,
    pub counts: std::collections::BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitJobRequest {
    pub mapping_snapshot_sha256: String,
    pub comparison_column: String,
    pub case_values: Vec<String>,
    pub control_values: Vec<String>,
    pub subject_column: Option<String>,
    pub covariates: Vec<String>,
    pub run_qc: bool,
    pub run_gene_corrected: bool,
    pub run_full_unified: bool,
    pub excluded_sample_ids: Vec<String>,
    pub run_exploratory_direction_check: bool,
    pub run_pbmc_diagnostics: bool,
    pub pbmc_batch_column: Option<String>,
    pub pbmc_family_column: Option<String>,
    pub pbmc_sensitivity_column: Option<String>,
    pub pathway_q_threshold: f64,
    pub pathway_effect_threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobStageTiming {
    pub stage: String,
    pub started_unix_seconds: u64,
    pub ended_unix_seconds: u64,
    pub elapsed_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobSummaryResponse {
    pub job_id: String,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
    #[serde(default)]
    pub started_unix_seconds: Option<u64>,
    #[serde(default)]
    pub current_stage: Option<String>,
    #[serde(default)]
    pub current_stage_started_unix_seconds: Option<u64>,
    #[serde(default)]
    pub stage_timings: Vec<JobStageTiming>,
    pub status: String,
    pub progress_message: String,
    pub snapshot_sha256: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobDetailResponse {
    pub summary: JobSummaryResponse,
    pub result: Option<JobResultSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobResultSummary {
    pub mapping: MappingAuditResponse,
    pub qc: Option<QcResultSummary>,
    pub gene: Option<GeneCorrectedSummary>,
    #[serde(default)]
    pub unified: Option<UnifiedAnalysisSummary>,
    pub warnings: Vec<String>,
    pub artifacts: Vec<JobArtifactSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnifiedAnalysisSummary {
    pub comparison_name: String,
    pub case_samples: usize,
    pub control_samples: usize,
    pub excluded_sample_ids: Vec<String>,
    pub modalities: Vec<UnifiedModalitySummary>,
    pub exploratory_gene_available: bool,
    pub exploratory_transcript_available: bool,
    #[serde(default)]
    pub pbmc_diagnostics_requested: bool,
    pub pbmc_diagnostics_available: bool,
    pub candidates: Vec<UnifiedCandidateSummary>,
    #[serde(default)]
    pub visualizations: UnifiedVisualizationSummary,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UnifiedVisualizationSummary {
    #[serde(default)]
    pub expression_panels: Vec<ExpressionVisualizationSummary>,
    #[serde(default)]
    pub effect_panels: Vec<EffectVisualizationSummary>,
    #[serde(default)]
    pub ordinations: Vec<OrdinationVisualizationSummary>,
    #[serde(default)]
    pub dendrograms: Vec<DendrogramVisualizationSummary>,
    #[serde(default)]
    pub pathway_points: Vec<PathwayVisualizationPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpressionVisualizationSummary {
    pub modality: String,
    pub tested_features: usize,
    pub filtered_features: usize,
    pub significant_q05: usize,
    pub plotted_features: usize,
    pub points: Vec<FeatureVisualizationPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EffectVisualizationSummary {
    pub modality: String,
    pub family: String,
    pub total_tests: usize,
    pub points: Vec<FeatureVisualizationPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeatureVisualizationPoint {
    pub feature: String,
    pub secondary_id: String,
    pub effect: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub mean_case: f64,
    pub mean_control: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrdinationVisualizationSummary {
    pub modality: String,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub permanova_f: f64,
    pub permanova_p: f64,
    pub permutations: usize,
    pub points: Vec<OrdinationVisualizationPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrdinationVisualizationPoint {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DendrogramVisualizationSummary {
    pub modality: String,
    pub title: String,
    pub distance_label: String,
    pub leaves: Vec<DendrogramVisualizationLeaf>,
    pub segments: Vec<DendrogramVisualizationSegment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DendrogramVisualizationLeaf {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub position: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DendrogramVisualizationSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathwayVisualizationPoint {
    pub pathway: String,
    pub description: String,
    pub measured_set_size: usize,
    pub overlap: usize,
    pub odds_ratio: f64,
    pub p_value: f64,
    pub q_value: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnifiedModalitySummary {
    pub modality: String,
    pub status: String,
    pub result_items: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnifiedCandidateSummary {
    pub rank: usize,
    pub modality: String,
    pub feature: String,
    pub effect: f64,
    pub q_value: f64,
    pub evidence: String,
    pub robustness: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QcResultSummary {
    pub comparison_name: String,
    pub modalities: Vec<String>,
    pub samples: usize,
    pub consensus_ids: Vec<String>,
    pub flagged_samples: Vec<QcSampleSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QcSampleSummary {
    pub sample_id: String,
    pub group: String,
    pub available_modalities: usize,
    pub flagged_modalities: usize,
    pub flagged_names: Vec<String>,
    pub consensus_flag: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneCorrectedSummary {
    pub n_input_samples: usize,
    pub n_model_samples: usize,
    pub n_subjects: usize,
    pub tested_features: usize,
    pub filtered_features: usize,
    pub model_description: String,
    pub top_features: Vec<GeneFeatureSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneFeatureSummary {
    pub feature: String,
    pub secondary_id: String,
    pub n_samples: usize,
    pub mean_case: f64,
    pub mean_control: f64,
    pub beta_group: f64,
    pub robust_se: f64,
    pub p_value: f64,
    pub q_value: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobArtifactSummary {
    pub artifact_id: String,
    pub name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CancelJobResponse {
    pub job: JobSummaryResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteJobResponse {
    pub job_id: String,
    pub deleted: bool,
    pub project: ProjectSummaryResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteObjectResponse {
    pub object_id: String,
    pub deleted: bool,
    pub project: ProjectSummaryResponse,
}

pub async fn health() -> Result<HealthResponse, String> {
    let response = Request::get(&format!("{API_ROOT}/health"))
        .send()
        .await
        .map_err(|error| format!("API connection failed: {error}"))?;
    decode(response).await
}

pub async fn create_project(
    display_name: String,
    retention_hours: u64,
) -> Result<CreateProjectResponse, String> {
    let request = CreateProjectRequest {
        display_name: Some(display_name),
        retention_hours: Some(retention_hours),
    };
    let builder = Request::post(&format!("{API_ROOT}/projects"))
        .json(&request)
        .map_err(|error| format!("failed to encode project request: {error}"))?;
    let response = builder
        .send()
        .await
        .map_err(|error| format!("project request failed: {error}"))?;
    decode(response).await
}

pub async fn get_project(
    project_id: &str,
    recovery_token: &str,
) -> Result<ProjectSummaryResponse, String> {
    let response = Request::get(&format!("{API_ROOT}/projects/{project_id}"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .send()
        .await
        .map_err(|error| format!("project recovery request failed: {error}"))?;
    decode(response).await
}

pub async fn delete_project(
    project_id: &str,
    recovery_token: &str,
) -> Result<DeleteProjectResponse, String> {
    let response = Request::delete(&format!("{API_ROOT}/projects/{project_id}"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .send()
        .await
        .map_err(|error| format!("project deletion request failed: {error}"))?;
    decode(response).await
}

pub async fn list_objects(
    project_id: &str,
    recovery_token: &str,
) -> Result<Vec<ObjectSummaryResponse>, String> {
    let response = Request::get(&format!("{API_ROOT}/projects/{project_id}/objects"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .send()
        .await
        .map_err(|error| format!("object listing request failed: {error}"))?;
    decode(response).await
}

pub async fn upload_object(
    project_id: &str,
    recovery_token: &str,
    file: web_sys::File,
) -> Result<UploadObjectResponse, String> {
    let filename = file.name();
    let mime = file.type_();
    let mut builder = Request::post(&format!("{API_ROOT}/projects/{project_id}/objects"))
        .query([("filename", filename.as_str())])
        .header("Authorization", &format!("Bearer {recovery_token}"));
    if !mime.trim().is_empty() {
        builder = builder.header("Content-Type", &mime);
    }
    let request = builder
        .body(file)
        .map_err(|error| format!("failed to create upload request: {error}"))?;
    let response = request
        .send()
        .await
        .map_err(|error| format!("upload request failed: {error}"))?;
    decode(response).await
}

pub async fn delete_object(
    project_id: &str,
    recovery_token: &str,
    object_id: &str,
) -> Result<DeleteObjectResponse, String> {
    let response = Request::delete(&format!(
        "{API_ROOT}/projects/{project_id}/objects/{object_id}"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .send()
    .await
    .map_err(|error| format!("object deletion request failed: {error}"))?;
    decode(response).await
}

pub async fn assign_object(
    project_id: &str,
    recovery_token: &str,
    object_id: &str,
    role: &str,
    sample_id: Option<String>,
) -> Result<AssignObjectResponse, String> {
    let request = AssignObjectRequest {
        role: role.to_string(),
        sample_id,
    };
    let builder = Request::put(&format!(
        "{API_ROOT}/projects/{project_id}/objects/{object_id}/assignment"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .json(&request)
    .map_err(|error| format!("failed to encode assignment request: {error}"))?;
    let response = builder
        .send()
        .await
        .map_err(|error| format!("assignment request failed: {error}"))?;
    decode(response).await
}

pub async fn auto_assign_objects(
    project_id: &str,
    recovery_token: &str,
) -> Result<AutoAssignObjectsResponse, String> {
    let response = Request::post(&format!(
        "{API_ROOT}/projects/{project_id}/objects/auto-assign"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .send()
    .await
    .map_err(|error| format!("automatic assignment request failed: {error}"))?;
    decode(response).await
}

pub async fn metadata_profile(
    project_id: &str,
    recovery_token: &str,
) -> Result<MetadataProfileResponse, String> {
    let response = Request::get(&format!(
        "{API_ROOT}/projects/{project_id}/metadata-profile"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .send()
    .await
    .map_err(|error| format!("metadata-profile request failed: {error}"))?;
    decode(response).await
}

pub async fn run_mapping_audit(
    project_id: &str,
    recovery_token: &str,
    comparison_column: Option<String>,
    case_values: Vec<String>,
    control_values: Vec<String>,
) -> Result<MappingAuditResponse, String> {
    let request = MappingAuditRequest {
        comparison_column,
        case_values,
        control_values,
    };
    let builder = Request::post(&format!("{API_ROOT}/projects/{project_id}/mapping-audit"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .json(&request)
        .map_err(|error| format!("failed to encode mapping request: {error}"))?;
    let response = builder
        .send()
        .await
        .map_err(|error| format!("mapping-audit request failed: {error}"))?;
    decode(response).await
}

pub async fn list_jobs(
    project_id: &str,
    recovery_token: &str,
) -> Result<Vec<JobSummaryResponse>, String> {
    let response = Request::get(&format!("{API_ROOT}/projects/{project_id}/jobs"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .send()
        .await
        .map_err(|error| format!("job listing request failed: {error}"))?;
    decode(response).await
}

pub async fn submit_job(
    project_id: &str,
    recovery_token: &str,
    request: SubmitJobRequest,
) -> Result<JobDetailResponse, String> {
    let builder = Request::post(&format!("{API_ROOT}/projects/{project_id}/jobs"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .json(&request)
        .map_err(|error| format!("failed to encode job request: {error}"))?;
    let response = builder
        .send()
        .await
        .map_err(|error| format!("job submission request failed: {error}"))?;
    decode(response).await
}

pub async fn get_job(
    project_id: &str,
    recovery_token: &str,
    job_id: &str,
) -> Result<JobDetailResponse, String> {
    let response = Request::get(&format!("{API_ROOT}/projects/{project_id}/jobs/{job_id}"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .send()
        .await
        .map_err(|error| format!("job status request failed: {error}"))?;
    decode(response).await
}

pub async fn cancel_job(
    project_id: &str,
    recovery_token: &str,
    job_id: &str,
) -> Result<CancelJobResponse, String> {
    let response = Request::delete(&format!("{API_ROOT}/projects/{project_id}/jobs/{job_id}"))
        .header("Authorization", &format!("Bearer {recovery_token}"))
        .send()
        .await
        .map_err(|error| format!("job cancellation request failed: {error}"))?;
    decode(response).await
}

pub async fn delete_stored_job(
    project_id: &str,
    recovery_token: &str,
    job_id: &str,
) -> Result<DeleteJobResponse, String> {
    let response = Request::delete(&format!(
        "{API_ROOT}/projects/{project_id}/jobs/{job_id}/stored"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .send()
    .await
    .map_err(|error| format!("stored job deletion request failed: {error}"))?;
    decode(response).await
}

pub async fn download_all_job_artifacts(
    project_id: &str,
    recovery_token: &str,
    job_id: &str,
) -> Result<Vec<u8>, String> {
    let response = Request::get(&format!(
        "{API_ROOT}/projects/{project_id}/jobs/{job_id}/artifacts.zip"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .send()
    .await
    .map_err(|error| format!("bulk artifact download request failed: {error}"))?;
    let status = response.status();
    if !(200..300).contains(&status) {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "server returned no readable error body".to_string());
        return Err(format!("server returned HTTP {status}: {body}"));
    }
    response
        .binary()
        .await
        .map_err(|error| format!("failed to read ZIP bundle bytes: {error}"))
}

pub async fn download_job_artifact(
    project_id: &str,
    recovery_token: &str,
    job_id: &str,
    artifact_id: &str,
) -> Result<Vec<u8>, String> {
    let response = Request::get(&format!(
        "{API_ROOT}/projects/{project_id}/jobs/{job_id}/artifacts/{artifact_id}"
    ))
    .header("Authorization", &format!("Bearer {recovery_token}"))
    .send()
    .await
    .map_err(|error| format!("artifact download request failed: {error}"))?;
    let status = response.status();
    if !(200..300).contains(&status) {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "server returned no readable error body".to_string());
        return Err(format!("server returned HTTP {status}: {body}"));
    }
    response
        .binary()
        .await
        .map_err(|error| format!("failed to read artifact bytes: {error}"))
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T, String> {
    let status = response.status();
    if !(200..300).contains(&status) {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "server returned no readable error body".to_string());
        return Err(format!("server returned HTTP {status}: {body}"));
    }
    response
        .json::<T>()
        .await
        .map_err(|error| format!("failed to decode server response: {error}"))
}
