//! Same-origin Phase-3D client for the local `cfs-server` API.

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
    pub batch_upload_enabled: bool,
    pub storage_backend: String,
    pub max_file_bytes: u64,
    pub max_project_bytes: u64,
    pub max_objects: usize,
    pub max_jobs_per_project: usize,
    pub max_concurrent_jobs: usize,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobSummaryResponse {
    pub job_id: String,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
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
    pub warnings: Vec<String>,
    pub artifacts: Vec<JobArtifactSummary>,
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
