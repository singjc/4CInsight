//! Local web server for the 4C Cohort Explorer browser client.
//!
//! Phase 3E.3 keeps authenticated metadata-backed comparison configuration and
//! adds presentation payloads plus on-demand bulk export for completed frozen
//! unified native jobs. The server reconstructs metadata/scientific state and
//! derives plot coordinates from native results; WASM does not recompute science.
//! Existing native multi-omics statistics and report generation remain the
//! scientific implementation, and authorization/storage semantics stay opaque.

use crate::{
    analysis::{
        CohortAnalysisSettingsSnapshot, CohortDefinitionSnapshot, CohortModalitySelection,
        UnifiedCohortAnalysisRun, run_unified_cohort_analysis,
        write_unified_analysis_tables_with_progress,
    },
    cohort::{
        CohortComparison, CohortComparisonDefinition, CohortModality, DetectedCohortFile,
        MappingStatus, MetadataComparisonSpec, MetadataTable, ProjectScan,
        build_custom_comparison, build_mapping, load_metadata,
    },
    corrected::{
        AdjustedFeatureResult, CorrectedExpressionResult, CorrectedModelSpec,
        CorrectedRepertoireResult, DendrogramResult, OrdinationResult, run_corrected_expression,
    },
    diagnostics::PbmcDiagnosticsConfig,
    input_conventions::{FilenameInputKind, classify_input_filename},
    metadata::{canonical_sample_id, unique_values as metadata_unique_values, validate_standard_metadata},
    outliers::{
        OutlierConfig, build_outlier_filter_snapshot, filter_comparison_excluding,
        run_cross_modal_outlier_qc,
    },
    report::{generate_unified_detailed_html_report, generate_unified_simplified_html_report},
};
use anyhow::{Context, Result, anyhow, bail};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::{StreamExt, stream};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    env,
    io::{Cursor, Read, Write},
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use tokio::{
    fs,
    io::AsyncWriteExt,
    sync::{Mutex, Semaphore},
};
use tower_http::services::ServeDir;
use uuid::Uuid;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const DEFAULT_BIND: &str = "127.0.0.1:8080";
const DEFAULT_TTL_HOURS: u64 = 24;
const MAX_TTL_HOURS: u64 = 24 * 7;
const CLEANUP_INTERVAL_SECONDS: u64 = 60 * 60;
const LOCAL_DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const LOCAL_DEFAULT_MAX_PROJECT_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const LOCAL_DEFAULT_MAX_OBJECTS: usize = 4_096;
const HOSTED_DEFAULT_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const HOSTED_DEFAULT_MAX_PROJECT_BYTES: u64 = 512 * 1024 * 1024;
const HOSTED_DEFAULT_MAX_OBJECTS: usize = 64;
const DEFAULT_MAX_JOBS_PER_PROJECT: usize = 16;
const DEFAULT_MAX_CONCURRENT_JOBS: usize = 1;
const LOCAL_DEFAULT_MAX_CONCURRENT_UPLOADS: usize = 6;
const HOSTED_DEFAULT_MAX_CONCURRENT_UPLOADS: usize = 2;
const AUTO_ASSIGN_VALIDATION_CONCURRENCY: usize = 6;
const MAX_JOB_ARTIFACT_BYTES: u64 = 128 * 1024 * 1024;
const UPLOAD_SCRATCH_MAX_AGE_SECONDS: u64 = 6 * 60 * 60;
const MAX_XLSX_ENTRIES: usize = 4_096;
const MAX_XLSX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_XLSX_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
const MAX_METADATA_PROFILE_VALUES: usize = 100;
const MAX_BULK_DOWNLOAD_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_VIS_EXPRESSION_POINTS: usize = 4_000;
const MAX_VIS_EFFECT_POINTS: usize = 30;
const MAX_VIS_PATHWAY_POINTS: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaProfile {
    LocalSelfHosted,
    HostedConservative,
}

impl QuotaProfile {
    fn from_env() -> Result<Self> {
        let value = env::var("CFS_QUOTA_PROFILE")
            .unwrap_or_else(|_| "local".to_string())
            .trim()
            .to_ascii_lowercase();
        match value.as_str() {
            "local" | "self-hosted" | "self_hosted" => Ok(Self::LocalSelfHosted),
            "hosted" | "conservative" => Ok(Self::HostedConservative),
            _ => {
                bail!("CFS_QUOTA_PROFILE must be one of: local, self-hosted, hosted, conservative")
            }
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LocalSelfHosted => "local-self-hosted",
            Self::HostedConservative => "hosted-conservative",
        }
    }

    fn defaults(self) -> (u64, u64, usize) {
        match self {
            Self::LocalSelfHosted => (
                LOCAL_DEFAULT_MAX_FILE_BYTES,
                LOCAL_DEFAULT_MAX_PROJECT_BYTES,
                LOCAL_DEFAULT_MAX_OBJECTS,
            ),
            Self::HostedConservative => (
                HOSTED_DEFAULT_MAX_FILE_BYTES,
                HOSTED_DEFAULT_MAX_PROJECT_BYTES,
                HOSTED_DEFAULT_MAX_OBJECTS,
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebServerConfig {
    pub bind: SocketAddr,
    pub storage_root: PathBuf,
    pub web_dist: PathBuf,
    pub default_ttl_hours: u64,
    pub quota_profile: QuotaProfile,
    pub max_file_bytes: u64,
    pub max_project_bytes: u64,
    pub max_objects: usize,
    pub max_jobs_per_project: usize,
    pub max_concurrent_jobs: usize,
    pub max_concurrent_uploads: usize,
}

impl WebServerConfig {
    pub fn from_env() -> Result<Self> {
        let bind = env::var("CFS_BIND")
            .unwrap_or_else(|_| DEFAULT_BIND.to_string())
            .parse::<SocketAddr>()
            .with_context(|| "CFS_BIND must be a socket address such as 127.0.0.1:8080")?;

        let storage_root = match env::var_os("CFS_STORAGE_ROOT") {
            Some(path) => PathBuf::from(path),
            None => default_storage_root(),
        };
        let web_dist = env::var_os("CFS_WEB_DIST")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("web/dist"));
        let default_ttl_hours = env::var("CFS_DEFAULT_TTL_HOURS")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .with_context(|| "CFS_DEFAULT_TTL_HOURS must be an integer number of hours")
            })
            .transpose()?
            .unwrap_or(DEFAULT_TTL_HOURS)
            .clamp(1, MAX_TTL_HOURS);
        let quota_profile = QuotaProfile::from_env()?;
        let (default_max_file_bytes, default_max_project_bytes, default_max_objects) =
            quota_profile.defaults();
        let max_file_bytes = env_u64("CFS_MAX_FILE_BYTES", default_max_file_bytes)?;
        let max_project_bytes = env_u64("CFS_MAX_PROJECT_BYTES", default_max_project_bytes)?;
        let max_objects = env_usize("CFS_MAX_OBJECTS", default_max_objects)?;
        let max_jobs_per_project = env_usize("CFS_MAX_JOBS", DEFAULT_MAX_JOBS_PER_PROJECT)?;
        let max_concurrent_jobs =
            env_usize("CFS_MAX_CONCURRENT_JOBS", DEFAULT_MAX_CONCURRENT_JOBS)?;
        let default_upload_concurrency = match quota_profile {
            QuotaProfile::LocalSelfHosted => LOCAL_DEFAULT_MAX_CONCURRENT_UPLOADS,
            QuotaProfile::HostedConservative => HOSTED_DEFAULT_MAX_CONCURRENT_UPLOADS,
        };
        let max_concurrent_uploads =
            env_usize("CFS_MAX_CONCURRENT_UPLOADS", default_upload_concurrency)?;
        if max_file_bytes == 0
            || max_project_bytes == 0
            || max_objects == 0
            || max_jobs_per_project == 0
            || max_concurrent_jobs == 0
            || max_concurrent_uploads == 0
        {
            bail!("web limits must all be greater than zero");
        }
        if max_file_bytes > max_project_bytes {
            bail!("CFS_MAX_FILE_BYTES cannot exceed CFS_MAX_PROJECT_BYTES");
        }

        Ok(Self {
            bind,
            storage_root,
            web_dist,
            default_ttl_hours,
            quota_profile,
            max_file_bytes,
            max_project_bytes,
            max_objects,
            max_jobs_per_project,
            max_concurrent_jobs,
            max_concurrent_uploads,
        })
    }
}

fn env_u64(name: &str, default: u64) -> Result<u64> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<u64>()
                .with_context(|| format!("{name} must be an integer number of bytes"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn env_usize(name: &str, default: usize) -> Result<usize> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .with_context(|| format!("{name} must be a positive integer"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn default_storage_root() -> PathBuf {
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("4c-cohort-explorer-web");
    }
    PathBuf::from(".4c-cohort-explorer-web")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredProjectRecord {
    schema_version: u32,
    project_id: Uuid,
    display_name: String,
    created_unix_seconds: u64,
    expires_unix_seconds: u64,
    last_accessed_unix_seconds: u64,
    access_token_sha256: String,
    uploaded_objects: usize,
    storage_bytes: u64,
    #[serde(default)]
    job_count: usize,
    #[serde(default)]
    job_snapshot_bytes: u64,
    #[serde(default)]
    job_artifact_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub display_name: String,
    pub created_unix_seconds: u64,
    pub expires_unix_seconds: u64,
    pub uploaded_objects: usize,
    pub storage_bytes: u64,
    pub job_count: usize,
    pub job_snapshot_bytes: u64,
    pub job_artifact_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UploadedInputRole {
    #[default]
    Unassigned,
    PrimaryMetadata,
    FilteredMetadata,
    GeneExpression,
    TranscriptExpression,
    Bcr,
    BcrIsotype,
    TcrAlpha,
    TcrBeta,
    ImmuneInfiltrate,
    MergedGeneExpression,
    MergedTranscriptExpression,
    PathwayGmt,
}

impl UploadedInputRole {
    fn is_singleton(self) -> bool {
        matches!(
            self,
            Self::PrimaryMetadata
                | Self::FilteredMetadata
                | Self::MergedGeneExpression
                | Self::MergedTranscriptExpression
                | Self::PathwayGmt
        )
    }

    fn requires_sample_id(self) -> bool {
        matches!(
            self,
            Self::GeneExpression
                | Self::TranscriptExpression
                | Self::Bcr
                | Self::BcrIsotype
                | Self::TcrAlpha
                | Self::TcrBeta
                | Self::ImmuneInfiltrate
        )
    }

    fn from_filename_kind(kind: FilenameInputKind) -> Option<Self> {
        match kind {
            FilenameInputKind::GeneExpression => Some(Self::GeneExpression),
            FilenameInputKind::TranscriptExpression => Some(Self::TranscriptExpression),
            FilenameInputKind::Bcr => Some(Self::Bcr),
            FilenameInputKind::BcrIsotype => Some(Self::BcrIsotype),
            FilenameInputKind::TcrAlpha => Some(Self::TcrAlpha),
            FilenameInputKind::TcrBeta => Some(Self::TcrBeta),
            FilenameInputKind::ImmuneInfiltrate => Some(Self::ImmuneInfiltrate),
            FilenameInputKind::MergedGeneExpression => Some(Self::MergedGeneExpression),
            FilenameInputKind::MergedTranscriptExpression => Some(Self::MergedTranscriptExpression),
            FilenameInputKind::MetadataCandidate => None,
        }
    }

    fn cohort_modality(self) -> Option<CohortModality> {
        match self {
            Self::GeneExpression => Some(CohortModality::GeneExpression),
            Self::TranscriptExpression => Some(CohortModality::TranscriptExpression),
            Self::Bcr => Some(CohortModality::Bcr),
            Self::BcrIsotype => Some(CohortModality::BcrIsotype),
            Self::TcrAlpha => Some(CohortModality::TcrAlpha),
            Self::TcrBeta => Some(CohortModality::TcrBeta),
            Self::ImmuneInfiltrate => Some(CohortModality::ImmuneInfiltrate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredInputManifest {
    schema_version: u32,
    project_id: Uuid,
    objects: Vec<StoredInputManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredInputManifestEntry {
    object_id: Uuid,
    role: UploadedInputRole,
    sample_id: Option<String>,
    sha256: String,
    byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredObjectRecord {
    schema_version: u32,
    object_id: Uuid,
    original_name: String,
    byte_size: u64,
    sha256: String,
    uploaded_unix_seconds: u64,
    content_type: Option<String>,
    #[serde(default)]
    role: UploadedInputRole,
    #[serde(default)]
    sample_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSummary {
    pub object_id: String,
    pub original_name: String,
    pub byte_size: u64,
    pub sha256: String,
    pub uploaded_unix_seconds: u64,
    pub content_type: Option<String>,
    pub role: UploadedInputRole,
    pub sample_id: Option<String>,
}

impl From<&StoredObjectRecord> for ObjectSummary {
    fn from(value: &StoredObjectRecord) -> Self {
        Self {
            object_id: value.object_id.to_string(),
            original_name: value.original_name.clone(),
            byte_size: value.byte_size,
            sha256: value.sha256.clone(),
            uploaded_unix_seconds: value.uploaded_unix_seconds,
            content_type: value.content_type.clone(),
            role: value.role,
            sample_id: value.sample_id.clone(),
        }
    }
}

impl From<&StoredProjectRecord> for ProjectSummary {
    fn from(value: &StoredProjectRecord) -> Self {
        Self {
            project_id: value.project_id.to_string(),
            display_name: value.display_name.clone(),
            created_unix_seconds: value.created_unix_seconds,
            expires_unix_seconds: value.expires_unix_seconds,
            uploaded_objects: value.uploaded_objects,
            storage_bytes: value.storage_bytes,
            job_count: value.job_count,
            job_snapshot_bytes: value.job_snapshot_bytes,
            job_artifact_bytes: value.job_artifact_bytes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub app_version: &'static str,
    pub api_version: u32,
    pub default_ttl_hours: u64,
    pub max_ttl_hours: u64,
    pub upload_enabled: bool,
    pub mapping_enabled: bool,
    pub analysis_enabled: bool,
    pub persistent_jobs: bool,
    pub representative_gene_enabled: bool,
    pub unified_analysis_enabled: bool,
    pub metadata_profile_enabled: bool,
    pub report_download_enabled: bool,
    pub bulk_artifact_download_enabled: bool,
    pub result_visualizations_enabled: bool,
    pub job_deletion_enabled: bool,
    pub batch_upload_enabled: bool,
    pub storage_backend: &'static str,
    pub quota_profile: &'static str,
    pub max_file_bytes: u64,
    pub max_project_bytes: u64,
    pub max_objects: usize,
    pub max_jobs_per_project: usize,
    pub max_concurrent_jobs: usize,
    pub max_concurrent_uploads: usize,
    pub max_job_artifact_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub retention_hours: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateProjectResponse {
    pub project: ProjectSummary,
    pub recovery_token: String,
    pub recovery_token_warning: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteProjectResponse {
    pub project_id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadObjectQuery {
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadObjectResponse {
    pub object: ObjectSummary,
    pub project: ProjectSummary,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssignObjectRequest {
    pub role: UploadedInputRole,
    #[serde(default)]
    pub sample_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssignObjectResponse {
    pub object: ObjectSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoAssignObjectsResponse {
    pub assigned: usize,
    pub already_assigned: usize,
    pub unresolved: usize,
    pub validation_failed: usize,
    pub objects: Vec<ObjectSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataValueCount {
    pub value: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataColumnProfile {
    pub name: String,
    pub distinct_nonempty_values: usize,
    pub comparison_ready: bool,
    pub values_truncated: bool,
    pub values: Vec<MetadataValueCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingAuditRequest {
    #[serde(default)]
    pub comparison_column: Option<String>,
    #[serde(default)]
    pub case_values: Vec<String>,
    #[serde(default)]
    pub control_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalityCoverageSummary {
    pub modality: String,
    pub assigned_files: usize,
    pub matched_files: usize,
    pub samples_with_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRowSummary {
    pub sample_id: String,
    pub status: String,
    pub counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Preparing,
    Running,
    Complete,
    Failed,
    Cancelled,
}

impl JobStatus {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitJobRequest {
    pub mapping_snapshot_sha256: String,
    pub comparison_column: String,
    pub case_values: Vec<String>,
    pub control_values: Vec<String>,
    #[serde(default)]
    pub subject_column: Option<String>,
    #[serde(default)]
    pub covariates: Vec<String>,
    #[serde(default = "default_true")]
    pub run_qc: bool,
    #[serde(default = "default_true")]
    pub run_gene_corrected: bool,
    #[serde(default)]
    pub run_full_unified: bool,
    #[serde(default)]
    pub excluded_sample_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub run_exploratory_direction_check: bool,
    #[serde(default = "default_true")]
    pub run_pbmc_diagnostics: bool,
    #[serde(default)]
    pub pbmc_batch_column: Option<String>,
    #[serde(default)]
    pub pbmc_family_column: Option<String>,
    #[serde(default)]
    pub pbmc_sensitivity_column: Option<String>,
    #[serde(default = "default_pathway_q_threshold")]
    pub pathway_q_threshold: f64,
    #[serde(default = "default_pathway_effect_threshold")]
    pub pathway_effect_threshold: f64,
}

fn default_true() -> bool {
    true
}

fn default_pathway_q_threshold() -> f64 {
    0.05
}

fn default_pathway_effect_threshold() -> f64 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredJobSnapshot {
    schema_version: u32,
    project_id: Uuid,
    job_id: Uuid,
    created_unix_seconds: u64,
    input_manifest_sha256: String,
    mapping_snapshot_sha256: String,
    #[serde(default)]
    artifact_budget_bytes: u64,
    request: SubmitJobRequest,
    objects: Vec<StoredObjectRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredJobRecord {
    schema_version: u32,
    project_id: Uuid,
    job_id: Uuid,
    created_unix_seconds: u64,
    updated_unix_seconds: u64,
    status: JobStatus,
    progress_message: String,
    snapshot_sha256: String,
    cancel_requested: bool,
    result: Option<JobResultSummary>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct JobWorkerProgress {
    done: usize,
    total: usize,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummaryResponse {
    pub job_id: String,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
    pub status: JobStatus,
    pub progress_message: String,
    pub snapshot_sha256: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDetailResponse {
    pub summary: JobSummaryResponse,
    pub result: Option<JobResultSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResultSummary {
    pub mapping: MappingAuditResponse,
    pub qc: Option<QcResultSummary>,
    pub gene: Option<GeneCorrectedSummary>,
    #[serde(default)]
    pub unified: Option<UnifiedAnalysisSummary>,
    pub warnings: Vec<String>,
    pub artifacts: Vec<JobArtifactSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAnalysisSummary {
    pub comparison_name: String,
    pub case_samples: usize,
    pub control_samples: usize,
    pub excluded_sample_ids: Vec<String>,
    pub modalities: Vec<UnifiedModalitySummary>,
    pub exploratory_gene_available: bool,
    pub exploratory_transcript_available: bool,
    pub pbmc_diagnostics_available: bool,
    pub candidates: Vec<UnifiedCandidateSummary>,
    #[serde(default)]
    pub visualizations: UnifiedVisualizationSummary,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionVisualizationSummary {
    pub modality: String,
    pub tested_features: usize,
    pub filtered_features: usize,
    pub significant_q05: usize,
    pub plotted_features: usize,
    pub points: Vec<FeatureVisualizationPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectVisualizationSummary {
    pub modality: String,
    pub family: String,
    pub total_tests: usize,
    pub points: Vec<FeatureVisualizationPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVisualizationPoint {
    pub feature: String,
    pub secondary_id: String,
    pub effect: f64,
    pub p_value: f64,
    pub q_value: f64,
    pub mean_case: f64,
    pub mean_control: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdinationVisualizationPoint {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DendrogramVisualizationSummary {
    pub modality: String,
    pub title: String,
    pub distance_label: String,
    pub leaves: Vec<DendrogramVisualizationLeaf>,
    pub segments: Vec<DendrogramVisualizationSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DendrogramVisualizationLeaf {
    pub sample_id: String,
    pub group: String,
    pub subject: String,
    pub position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DendrogramVisualizationSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathwayVisualizationPoint {
    pub pathway: String,
    pub description: String,
    pub measured_set_size: usize,
    pub overlap: usize,
    pub odds_ratio: f64,
    pub p_value: f64,
    pub q_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedModalitySummary {
    pub modality: String,
    pub status: String,
    pub result_items: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCandidateSummary {
    pub rank: usize,
    pub modality: String,
    pub feature: String,
    pub effect: f64,
    pub q_value: f64,
    pub evidence: String,
    pub robustness: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QcResultSummary {
    pub comparison_name: String,
    pub modalities: Vec<String>,
    pub samples: usize,
    pub consensus_ids: Vec<String>,
    pub flagged_samples: Vec<QcSampleSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QcSampleSummary {
    pub sample_id: String,
    pub group: String,
    pub available_modalities: usize,
    pub flagged_modalities: usize,
    pub flagged_names: Vec<String>,
    pub consensus_flag: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobArtifactSummary {
    pub artifact_id: String,
    pub name: String,
    pub media_type: String,
    pub byte_size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredJobArtifactRecord {
    schema_version: u32,
    artifact_id: Uuid,
    name: String,
    media_type: String,
    byte_size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelJobResponse {
    pub job: JobSummaryResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteJobResponse {
    pub job_id: String,
    pub deleted: bool,
    pub project: ProjectSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMappingSnapshot {
    schema_version: u32,
    project_id: Uuid,
    created_unix_seconds: u64,
    input_manifest_sha256: String,
    mapping_snapshot_sha256: String,
    request: MappingAuditRequest,
    response: MappingAuditResponse,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteObjectResponse {
    pub object_id: String,
    pub deleted: bool,
    pub project: ProjectSummary,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Debug)]
pub struct LocalProjectStore {
    root: PathBuf,
    default_ttl_hours: u64,
    mutation_lock: Mutex<()>,
}

impl LocalProjectStore {
    pub async fn new(root: PathBuf, default_ttl_hours: u64) -> Result<Self> {
        let projects = root.join("projects");
        fs::create_dir_all(&projects)
            .await
            .with_context(|| format!("failed to create {}", projects.display()))?;
        set_private_dir_permissions(&root).await?;
        set_private_dir_permissions(&projects).await?;
        Ok(Self {
            root,
            default_ttl_hours: default_ttl_hours.clamp(1, MAX_TTL_HOURS),
            mutation_lock: Mutex::new(()),
        })
    }

    fn projects_root(&self) -> PathBuf {
        self.root.join("projects")
    }

    fn project_dir(&self, project_id: Uuid) -> PathBuf {
        self.projects_root().join(project_id.to_string())
    }

    fn project_record_path(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id).join("project.json")
    }

    fn inputs_dir(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id).join("inputs")
    }

    fn objects_dir(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id).join("objects")
    }

    fn scratch_dir(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id).join("scratch")
    }

    fn input_manifest_path(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id)
            .join("manifests")
            .join("inputs.json")
    }

    fn mapping_snapshot_path(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id)
            .join("manifests")
            .join("mapping_audit.json")
    }

    fn jobs_dir(&self, project_id: Uuid) -> PathBuf {
        self.project_dir(project_id).join("jobs")
    }

    fn job_dir(&self, project_id: Uuid, job_id: Uuid) -> PathBuf {
        self.jobs_dir(project_id).join(job_id.to_string())
    }

    fn job_record_path(&self, project_id: Uuid, job_id: Uuid) -> PathBuf {
        self.job_dir(project_id, job_id).join("job.json")
    }

    fn job_snapshot_path(&self, project_id: Uuid, job_id: Uuid) -> PathBuf {
        self.job_dir(project_id, job_id).join("snapshot.json")
    }

    fn job_snapshot_root(&self, project_id: Uuid, job_id: Uuid) -> PathBuf {
        self.job_dir(project_id, job_id).join("snapshot")
    }

    fn job_artifacts_dir(&self, project_id: Uuid, job_id: Uuid) -> PathBuf {
        self.job_dir(project_id, job_id).join("artifacts")
    }

    fn job_artifact_data_path(&self, project_id: Uuid, job_id: Uuid, artifact_id: Uuid) -> PathBuf {
        self.job_artifacts_dir(project_id, job_id)
            .join(format!("{artifact_id}.bin"))
    }

    fn job_artifact_record_path(
        &self,
        project_id: Uuid,
        job_id: Uuid,
        artifact_id: Uuid,
    ) -> PathBuf {
        self.job_artifacts_dir(project_id, job_id)
            .join(format!("{artifact_id}.json"))
    }

    fn object_data_path(&self, project_id: Uuid, object_id: Uuid) -> PathBuf {
        self.inputs_dir(project_id).join(format!("{object_id}.bin"))
    }

    fn object_record_path(&self, project_id: Uuid, object_id: Uuid) -> PathBuf {
        self.objects_dir(project_id)
            .join(format!("{object_id}.json"))
    }

    async fn create_project(
        &self,
        request: CreateProjectRequest,
    ) -> Result<(ProjectSummary, String)> {
        let now = unix_seconds()?;
        let retention_hours = request
            .retention_hours
            .unwrap_or(self.default_ttl_hours)
            .clamp(1, MAX_TTL_HOURS);
        let expires = now
            .checked_add(retention_hours.saturating_mul(60 * 60))
            .ok_or_else(|| anyhow!("project expiry overflow"))?;
        let project_id = Uuid::new_v4();
        let token = generate_recovery_token();
        let display_name = sanitize_display_name(request.display_name.as_deref());
        let record = StoredProjectRecord {
            schema_version: 1,
            project_id,
            display_name,
            created_unix_seconds: now,
            expires_unix_seconds: expires,
            last_accessed_unix_seconds: now,
            access_token_sha256: token_hash_hex(&token),
            uploaded_objects: 0,
            storage_bytes: 0,
            job_count: 0,
            job_snapshot_bytes: 0,
            job_artifact_bytes: 0,
        };

        let project_dir = self.project_dir(project_id);
        fs::create_dir_all(&project_dir).await.with_context(|| {
            format!(
                "failed to create project directory {}",
                project_dir.display()
            )
        })?;
        set_private_dir_permissions(&project_dir).await?;
        for subdir in [
            "inputs",
            "objects",
            "results",
            "reports",
            "manifests",
            "scratch",
            "jobs",
        ] {
            let dir = project_dir.join(subdir);
            fs::create_dir_all(&dir)
                .await
                .with_context(|| format!("failed to create project {subdir} directory"))?;
            set_private_dir_permissions(&dir).await?;
        }
        write_json_atomic(&self.project_record_path(project_id), &record).await?;
        Ok((ProjectSummary::from(&record), token))
    }

    async fn read_authorized(&self, project_id: Uuid, token: &str) -> Result<ProjectSummary> {
        let _guard = self.mutation_lock.lock().await;
        let mut record = self.authorize_record(project_id, token).await?;
        self.reconcile_record_storage(&mut record).await?;
        record.last_accessed_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.project_record_path(project_id), &record).await?;
        Ok(ProjectSummary::from(&record))
    }

    async fn delete_authorized(&self, project_id: Uuid, token: &str) -> Result<bool> {
        let _guard = self.mutation_lock.lock().await;
        let _ = self.authorize_record(project_id, token).await?;
        self.delete_unchecked(project_id).await
    }

    async fn read_record(&self, project_id: Uuid) -> Result<StoredProjectRecord> {
        let path = self.project_record_path(project_id);
        let bytes = fs::read(&path)
            .await
            .with_context(|| format!("project {} does not exist", project_id))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    async fn authorize_record(&self, project_id: Uuid, token: &str) -> Result<StoredProjectRecord> {
        let record = self.read_record(project_id).await?;
        let now = unix_seconds()?;
        if now >= record.expires_unix_seconds {
            let _ = self.delete_unchecked(project_id).await;
            bail!("project has expired");
        }
        if !token_matches(token, &record.access_token_sha256)? {
            bail!("invalid recovery token");
        }
        Ok(record)
    }

    async fn read_object_record(
        &self,
        project_id: Uuid,
        object_id: Uuid,
    ) -> Result<StoredObjectRecord> {
        let path = self.object_record_path(project_id, object_id);
        let bytes = fs::read(&path)
            .await
            .with_context(|| format!("object {} does not exist", object_id))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    async fn list_object_records(&self, project_id: Uuid) -> Result<Vec<StoredObjectRecord>> {
        let dir = self.objects_dir(project_id);
        if fs::metadata(&dir).await.is_err() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&dir)
            .await
            .with_context(|| format!("failed to scan {}", dir.display()))?;
        let mut objects = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).await?;
            let record: StoredObjectRecord = serde_json::from_slice(&bytes)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            objects.push(record);
        }
        objects.sort_by(|a, b| {
            a.uploaded_unix_seconds
                .cmp(&b.uploaded_unix_seconds)
                .then_with(|| a.object_id.cmp(&b.object_id))
        });
        Ok(objects)
    }

    async fn write_input_manifest_from_records(
        &self,
        project_id: Uuid,
        objects: &[StoredObjectRecord],
    ) -> Result<()> {
        let manifest = StoredInputManifest {
            schema_version: 1,
            project_id,
            objects: objects
                .iter()
                .map(|object| StoredInputManifestEntry {
                    object_id: object.object_id,
                    role: object.role,
                    sample_id: object.sample_id.clone(),
                    sha256: object.sha256.clone(),
                    byte_size: object.byte_size,
                })
                .collect(),
        };
        write_json_atomic(&self.input_manifest_path(project_id), &manifest).await
    }

    async fn refresh_input_manifest(&self, project_id: Uuid) -> Result<Vec<StoredObjectRecord>> {
        let objects = self.list_object_records(project_id).await?;
        self.write_input_manifest_from_records(project_id, &objects)
            .await?;
        Ok(objects)
    }

    fn input_manifest_sha256_for_records(
        &self,
        project_id: Uuid,
        objects: &[StoredObjectRecord],
    ) -> Result<String> {
        let manifest = StoredInputManifest {
            schema_version: 1,
            project_id,
            objects: objects
                .iter()
                .map(|object| StoredInputManifestEntry {
                    object_id: object.object_id,
                    role: object.role,
                    sample_id: object.sample_id.clone(),
                    sha256: object.sha256.clone(),
                    byte_size: object.byte_size,
                })
                .collect(),
        };
        Ok(hex::encode(Sha256::digest(serde_json::to_vec(&manifest)?)))
    }

    async fn reconcile_record_storage(&self, record: &mut StoredProjectRecord) -> Result<()> {
        let objects = self.list_object_records(record.project_id).await?;
        let input_bytes = objects.iter().map(|item| item.byte_size).sum::<u64>();
        let jobs = self
            .list_job_records(record.project_id)
            .await
            .unwrap_or_default();
        let job_count = jobs.len();
        let mut snapshot_bytes = 0u64;
        let mut artifact_bytes = 0u64;
        for job in &jobs {
            if let Ok(bytes) = fs::read(self.job_snapshot_path(record.project_id, job.job_id)).await
            {
                if let Ok(snapshot) = serde_json::from_slice::<StoredJobSnapshot>(&bytes) {
                    snapshot_bytes = snapshot_bytes.saturating_add(
                        snapshot
                            .objects
                            .iter()
                            .map(|item| item.byte_size)
                            .sum::<u64>(),
                    );
                }
            }
            let dir = self.job_artifacts_dir(record.project_id, job.job_id);
            let Ok(mut entries) = fs::read_dir(&dir).await else {
                continue;
            };
            while let Some(entry) = entries.next_entry().await? {
                if !entry.file_type().await?.is_file() {
                    continue;
                }
                let path = entry.path();
                if path.extension().and_then(|x| x.to_str()) != Some("json") {
                    continue;
                }
                let bytes = fs::read(&path).await?;
                if let Ok(artifact) = serde_json::from_slice::<StoredJobArtifactRecord>(&bytes) {
                    artifact_bytes = artifact_bytes.saturating_add(artifact.byte_size);
                }
            }
        }
        record.uploaded_objects = objects.len();
        record.job_count = job_count;
        record.job_snapshot_bytes = snapshot_bytes;
        record.job_artifact_bytes = artifact_bytes;
        record.storage_bytes = input_bytes
            .saturating_add(snapshot_bytes)
            .saturating_add(artifact_bytes);
        Ok(())
    }

    async fn refresh_project_storage_record(&self, project_id: Uuid) -> Result<()> {
        let _guard = self.mutation_lock.lock().await;
        let mut record = self.read_record(project_id).await?;
        self.reconcile_record_storage(&mut record).await?;
        write_json_atomic(&self.project_record_path(project_id), &record).await
    }

    async fn list_objects_authorized(
        &self,
        project_id: Uuid,
        token: &str,
    ) -> Result<Vec<ObjectSummary>> {
        let _guard = self.mutation_lock.lock().await;
        let mut record = self.authorize_record(project_id, token).await?;
        let objects = self.list_object_records(project_id).await?;
        // Preserve complete project accounting (inputs + persisted job artifacts)
        // rather than resetting storage_bytes to input objects only when the
        // browser refreshes the object list.
        self.reconcile_record_storage(&mut record).await?;
        record.last_accessed_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.project_record_path(project_id), &record).await?;
        Ok(objects.iter().map(ObjectSummary::from).collect())
    }

    async fn upload_object_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        filename: &str,
        content_type: Option<String>,
        content_length: Option<u64>,
        body: Body,
        max_file_bytes: u64,
        max_project_bytes: u64,
        max_objects: usize,
    ) -> Result<(ObjectSummary, ProjectSummary)> {
        let original_name = sanitize_upload_filename(filename)?;

        // Short authorization/quota preflight only. The global project mutation
        // lock is intentionally released before reading the request body so
        // bounded concurrent uploads can stream/hash in parallel. Quotas are
        // checked again atomically during finalization.
        {
            let _guard = self.mutation_lock.lock().await;
            let project = self.authorize_record(project_id, token).await?;
            if project.uploaded_objects >= max_objects {
                bail!("project has reached the maximum object count");
            }
            if let Some(length) = content_length {
                if length == 0 {
                    bail!("empty uploads are not allowed");
                }
                if length > max_file_bytes {
                    bail!("upload exceeds the maximum file size");
                }
                if project.storage_bytes.saturating_add(length) > max_project_bytes {
                    bail!("upload would exceed the project storage quota");
                }
            }
        }

        let object_id = Uuid::new_v4();
        let scratch_path = self
            .scratch_dir(project_id)
            .join(format!("upload-{object_id}.part"));
        let final_path = self.object_data_path(project_id, object_id);
        let metadata_path = self.object_record_path(project_id, object_id);
        let mut file = fs::File::create(&scratch_path)
            .await
            .with_context(|| format!("failed to create upload scratch object {object_id}"))?;
        set_private_file_permissions(&scratch_path).await?;

        let mut total = 0u64;
        let mut hasher = Sha256::new();
        let mut stream = body.into_data_stream();
        let stream_result: Result<()> = async {
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context("failed while reading upload body")?;
                let chunk_len = u64::try_from(chunk.len()).context("upload chunk is too large")?;
                total = total
                    .checked_add(chunk_len)
                    .ok_or_else(|| anyhow!("upload size overflow"))?;
                if total > max_file_bytes {
                    bail!("upload exceeds the maximum file size");
                }
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            Ok(())
        }
        .await;
        drop(file);
        if let Err(error) = stream_result {
            let _ = fs::remove_file(&scratch_path).await;
            return Err(error);
        }
        if total == 0 {
            let _ = fs::remove_file(&scratch_path).await;
            bail!("empty uploads are not allowed");
        }

        // Exact native filename conventions can safely prepare an automatic
        // role/sample-ID assignment. The bytes still pass the same defensive
        // role validator used by manual assignment. If validation fails, the
        // upload remains valid but is stored as unassigned for manual review.
        let sha256 = hex::encode(hasher.finalize());
        let mut inferred_assignment = infer_auto_assignment_from_filename(&original_name);
        if let Some((role, _)) = &inferred_assignment {
            let validation_path = scratch_path.clone();
            let validation_name = original_name.clone();
            let validation_role = *role;
            let valid = tokio::task::spawn_blocking(move || {
                validate_object_for_role(&validation_path, &validation_name, validation_role)
            })
            .await
            .context("automatic input validation task failed")?
            .is_ok();
            if !valid {
                inferred_assignment = None;
            }
        }

        let finalize_result: Result<(ObjectSummary, ProjectSummary)> = async {
            let _guard = self.mutation_lock.lock().await;
            let mut project = self.authorize_record(project_id, token).await?;
            if project.uploaded_objects >= max_objects {
                bail!("project has reached the maximum object count");
            }
            if project.storage_bytes.saturating_add(total) > max_project_bytes {
                bail!("upload would exceed the project storage quota");
            }

            let (mut role, mut sample_id) = inferred_assignment
                .clone()
                .unwrap_or((UploadedInputRole::Unassigned, None));
            if role.is_singleton() {
                let objects = self.list_object_records(project_id).await?;
                if objects.iter().any(|other| other.role == role) {
                    role = UploadedInputRole::Unassigned;
                    sample_id = None;
                }
            }

            fs::rename(&scratch_path, &final_path)
                .await
                .with_context(|| format!("failed to finalize uploaded object {object_id}"))?;
            set_private_file_permissions(&final_path).await?;
            let object_record = StoredObjectRecord {
                schema_version: 1,
                object_id,
                original_name: original_name.clone(),
                byte_size: total,
                sha256: sha256.clone(),
                uploaded_unix_seconds: unix_seconds()?,
                content_type: sanitize_content_type(content_type.as_deref()),
                role,
                sample_id,
            };
            if let Err(error) = write_json_atomic(&metadata_path, &object_record).await {
                let _ = fs::remove_file(&final_path).await;
                return Err(error);
            }

            project.uploaded_objects = project.uploaded_objects.saturating_add(1);
            project.storage_bytes = project.storage_bytes.saturating_add(total);
            project.last_accessed_unix_seconds = unix_seconds()?;
            if let Err(error) =
                write_json_atomic(&self.project_record_path(project_id), &project).await
            {
                let _ = fs::remove_file(&metadata_path).await;
                let _ = fs::remove_file(&final_path).await;
                return Err(error);
            }
            // Do not rebuild the full manifest after every file in a large batch.
            // Mapping/assignment operations refresh it once from the final object set.
            Ok((
                ObjectSummary::from(&object_record),
                ProjectSummary::from(&project),
            ))
        }
        .await;

        if finalize_result.is_err() {
            let _ = fs::remove_file(&scratch_path).await;
        }
        finalize_result
    }

    async fn assign_object_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        object_id: Uuid,
        request: AssignObjectRequest,
    ) -> Result<ObjectSummary> {
        let _guard = self.mutation_lock.lock().await;
        let mut project = self.authorize_record(project_id, token).await?;
        let mut object = self.read_object_record(project_id, object_id).await?;
        let sample_id = normalize_assigned_sample_id(request.role, request.sample_id.as_deref())?;

        let objects = self.list_object_records(project_id).await?;
        if matches!(
            request.role,
            UploadedInputRole::PrimaryMetadata
                | UploadedInputRole::FilteredMetadata
                | UploadedInputRole::MergedGeneExpression
                | UploadedInputRole::MergedTranscriptExpression
                | UploadedInputRole::PathwayGmt
        ) && objects
            .iter()
            .any(|other| other.object_id != object_id && other.role == request.role)
        {
            bail!("only one object can be assigned to this singleton input role");
        }

        let data_path = self.object_data_path(project_id, object_id);
        let original_name = object.original_name.clone();
        let role = request.role;
        tokio::task::spawn_blocking(move || {
            validate_object_for_role(&data_path, &original_name, role)
        })
        .await
        .context("input validation task failed")??;

        object.role = role;
        object.sample_id = sample_id;
        write_json_atomic(&self.object_record_path(project_id, object_id), &object).await?;
        self.refresh_input_manifest(project_id).await?;
        project.last_accessed_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.project_record_path(project_id), &project).await?;
        Ok(ObjectSummary::from(&object))
    }

    async fn auto_assign_objects_authorized(
        &self,
        project_id: Uuid,
        token: &str,
    ) -> Result<AutoAssignObjectsResponse> {
        let (objects, already_assigned) = {
            let _guard = self.mutation_lock.lock().await;
            let mut project = self.authorize_record(project_id, token).await?;
            let objects = self.list_object_records(project_id).await?;
            project.last_accessed_unix_seconds = unix_seconds()?;
            write_json_atomic(&self.project_record_path(project_id), &project).await?;
            let already_assigned = objects
                .iter()
                .filter(|object| object.role != UploadedInputRole::Unassigned)
                .count();
            (objects, already_assigned)
        };

        let candidates = objects
            .into_iter()
            .filter(|object| object.role == UploadedInputRole::Unassigned)
            .filter_map(|object| {
                let (role, sample_id) = infer_auto_assignment_from_filename(&object.original_name)?;
                let data_path = self.object_data_path(project_id, object.object_id);
                Some((object, data_path, role, sample_id))
            })
            .collect::<Vec<_>>();

        let validations = stream::iter(candidates.into_iter().map(
            |(object, data_path, role, sample_id)| async move {
                let original_name = object.original_name.clone();
                let validation = tokio::task::spawn_blocking(move || {
                    validate_object_for_role(&data_path, &original_name, role)
                })
                .await;
                let valid = matches!(validation, Ok(Ok(())));
                (object.object_id, role, sample_id, valid)
            },
        ))
        .buffer_unordered(AUTO_ASSIGN_VALIDATION_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

        let validation_failed = validations
            .iter()
            .filter(|(_, _, _, valid)| !*valid)
            .count();
        let mut warnings = Vec::new();
        if validation_failed > 0 {
            warnings.push(format!(
                "{validation_failed} recognized object(s) failed defensive content validation and remain unassigned"
            ));
        }

        let _guard = self.mutation_lock.lock().await;
        let mut project = self.authorize_record(project_id, token).await?;
        let mut current = self.list_object_records(project_id).await?;
        let mut assigned = 0usize;

        for (object_id, role, sample_id, valid) in validations {
            if !valid {
                continue;
            }
            let Some(index) = current
                .iter()
                .position(|object| object.object_id == object_id)
            else {
                continue;
            };
            if current[index].role != UploadedInputRole::Unassigned {
                continue;
            }
            if role.is_singleton()
                && current
                    .iter()
                    .any(|other| other.object_id != object_id && other.role == role)
            {
                continue;
            }
            current[index].role = role;
            current[index].sample_id = sample_id;
            write_json_atomic(
                &self.object_record_path(project_id, object_id),
                &current[index],
            )
            .await?;
            assigned = assigned.saturating_add(1);
        }

        self.write_input_manifest_from_records(project_id, &current)
            .await?;
        project.last_accessed_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.project_record_path(project_id), &project).await?;
        let unresolved = current
            .iter()
            .filter(|object| object.role == UploadedInputRole::Unassigned)
            .count();
        Ok(AutoAssignObjectsResponse {
            assigned,
            already_assigned,
            unresolved,
            validation_failed,
            objects: current.iter().map(ObjectSummary::from).collect(),
            warnings,
        })
    }

    async fn metadata_profile_authorized(
        &self,
        project_id: Uuid,
        token: &str,
    ) -> Result<MetadataProfileResponse> {
        let (project_dir, primary) = {
            let _guard = self.mutation_lock.lock().await;
            let mut project = self.authorize_record(project_id, token).await?;
            let objects = self.list_object_records(project_id).await?;
            let primary = objects
                .iter()
                .filter(|object| object.role == UploadedInputRole::PrimaryMetadata)
                .cloned()
                .collect::<Vec<_>>();
            if primary.len() != 1 {
                bail!(
                    "assign exactly one uploaded object as primary_metadata before loading metadata fields"
                );
            }
            project.last_accessed_unix_seconds = unix_seconds()?;
            write_json_atomic(&self.project_record_path(project_id), &project).await?;
            (self.project_dir(project_id), primary[0].clone())
        };

        tokio::task::spawn_blocking(move || build_server_metadata_profile(&project_dir, &primary))
            .await
            .context("metadata-profile task failed")?
    }

    async fn mapping_audit_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        request: MappingAuditRequest,
    ) -> Result<MappingAuditResponse> {
        let _guard = self.mutation_lock.lock().await;
        let mut project = self.authorize_record(project_id, token).await?;
        let objects = self.refresh_input_manifest(project_id).await?;
        let input_manifest_sha256 = self.input_manifest_sha256_for_records(project_id, &objects)?;
        let mapping_snapshot_sha256 =
            mapping_snapshot_fingerprint(&input_manifest_sha256, &request)?;
        let project_dir = self.project_dir(project_id);
        let request_for_worker = request.clone();
        let mapping_hash_for_worker = mapping_snapshot_sha256.clone();
        let response = tokio::task::spawn_blocking(move || {
            build_server_mapping_audit(
                project_id,
                &project_dir,
                &objects,
                request_for_worker,
                mapping_hash_for_worker,
            )
        })
        .await
        .context("mapping-audit task failed")??;
        let stored = StoredMappingSnapshot {
            schema_version: 1,
            project_id,
            created_unix_seconds: unix_seconds()?,
            input_manifest_sha256,
            mapping_snapshot_sha256,
            request,
            response: response.clone(),
        };
        write_json_atomic(&self.mapping_snapshot_path(project_id), &stored).await?;
        project.last_accessed_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.project_record_path(project_id), &project).await?;
        Ok(response)
    }

    async fn read_job_record(&self, project_id: Uuid, job_id: Uuid) -> Result<StoredJobRecord> {
        let path = self.job_record_path(project_id, job_id);
        let bytes = fs::read(&path)
            .await
            .with_context(|| format!("job {} does not exist", job_id))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    async fn list_job_records(&self, project_id: Uuid) -> Result<Vec<StoredJobRecord>> {
        let dir = self.jobs_dir(project_id);
        if fs::metadata(&dir).await.is_err() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&dir).await?;
        let mut jobs = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(job_id) = Uuid::parse_str(&name) else {
                continue;
            };
            if let Ok(record) = self.read_job_record(project_id, job_id).await {
                jobs.push(record);
            }
        }
        jobs.sort_by(|a, b| b.created_unix_seconds.cmp(&a.created_unix_seconds));
        Ok(jobs)
    }

    async fn list_jobs_authorized(
        &self,
        project_id: Uuid,
        token: &str,
    ) -> Result<Vec<JobSummaryResponse>> {
        let _guard = self.mutation_lock.lock().await;
        let _ = self.authorize_record(project_id, token).await?;
        Ok(self
            .list_job_records(project_id)
            .await?
            .iter()
            .map(job_summary_from_record)
            .collect())
    }

    async fn read_job_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        job_id: Uuid,
    ) -> Result<JobDetailResponse> {
        let _guard = self.mutation_lock.lock().await;
        let _ = self.authorize_record(project_id, token).await?;
        let record = self.read_job_record(project_id, job_id).await?;
        Ok(JobDetailResponse {
            summary: job_summary_from_record(&record),
            result: record.result.clone(),
        })
    }

    async fn create_job_snapshot_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        request: SubmitJobRequest,
        max_jobs_per_project: usize,
        max_project_bytes: u64,
    ) -> Result<JobDetailResponse> {
        let _guard = self.mutation_lock.lock().await;
        let mut project = self.authorize_record(project_id, token).await?;
        self.reconcile_record_storage(&mut project).await?;
        if project.job_count >= max_jobs_per_project {
            bail!("project has reached the maximum persistent job count");
        }
        let objects = self.refresh_input_manifest(project_id).await?;
        let current_input_manifest_sha256 =
            self.input_manifest_sha256_for_records(project_id, &objects)?;
        let assigned = objects
            .into_iter()
            .filter(|object| object.role != UploadedInputRole::Unassigned)
            .collect::<Vec<_>>();
        let logical_snapshot_bytes = assigned.iter().map(|object| object.byte_size).sum::<u64>();
        if project.storage_bytes.saturating_add(logical_snapshot_bytes) > max_project_bytes {
            bail!(
                "freezing this job would exceed the project storage quota; delete old projects/jobs or raise CFS_MAX_PROJECT_BYTES"
            );
        }
        let artifact_budget_bytes = max_project_bytes.saturating_sub(
            project.storage_bytes.saturating_add(logical_snapshot_bytes),
        );
        if assigned
            .iter()
            .filter(|object| object.role == UploadedInputRole::PrimaryMetadata)
            .count()
            != 1
        {
            bail!(
                "a current validated primary metadata assignment is required before submitting a job"
            );
        }
        let mapping_request = MappingAuditRequest {
            comparison_column: Some(request.comparison_column.trim().to_string()),
            case_values: request.case_values.clone(),
            control_values: request.control_values.clone(),
        };
        if mapping_request
            .comparison_column
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
            || mapping_request.case_values.is_empty()
            || mapping_request.control_values.is_empty()
        {
            bail!(
                "job submission requires a metadata comparison column plus Case and Control values"
            );
        }
        let expected_mapping_hash =
            mapping_snapshot_fingerprint(&current_input_manifest_sha256, &mapping_request)?;
        if !constant_time_string_eq(
            &expected_mapping_hash,
            request.mapping_snapshot_sha256.trim(),
        ) {
            bail!(
                "the validated mapping snapshot is stale; rerun the mapping audit after the latest input changes"
            );
        }
        let mapping_bytes = fs::read(self.mapping_snapshot_path(project_id))
            .await
            .context("no persisted mapping snapshot exists; rerun the mapping audit")?;
        let stored_mapping: StoredMappingSnapshot = serde_json::from_slice(&mapping_bytes)
            .context("persisted mapping snapshot is invalid")?;
        if !constant_time_string_eq(
            &stored_mapping.mapping_snapshot_sha256,
            request.mapping_snapshot_sha256.trim(),
        ) {
            bail!("the persisted mapping snapshot does not match this job request; rerun mapping");
        }

        let job_id = Uuid::new_v4();
        let now = unix_seconds()?;
        let job_dir = self.job_dir(project_id, job_id);
        let snapshot_root = self.job_snapshot_root(project_id, job_id);
        let snapshot_inputs = snapshot_root.join("inputs");
        let snapshot_scratch = snapshot_root.join("scratch");
        let artifacts = self.job_artifacts_dir(project_id, job_id);
        for dir in [
            &job_dir,
            &snapshot_root,
            &snapshot_inputs,
            &snapshot_scratch,
            &artifacts,
        ] {
            fs::create_dir_all(dir).await?;
            set_private_dir_permissions(dir).await?;
        }
        for object in &assigned {
            let source = self.object_data_path(project_id, object.object_id);
            let target = snapshot_inputs.join(format!("{}.bin", object.object_id));
            match fs::hard_link(&source, &target).await {
                Ok(()) => {}
                Err(_) => {
                    fs::copy(&source, &target).await.with_context(|| {
                        format!("failed to freeze uploaded object {}", object.object_id)
                    })?;
                }
            }
            set_private_file_permissions(&target).await?;
        }
        let snapshot = StoredJobSnapshot {
            schema_version: 1,
            project_id,
            job_id,
            created_unix_seconds: now,
            input_manifest_sha256: current_input_manifest_sha256,
            mapping_snapshot_sha256: request.mapping_snapshot_sha256.clone(),
            artifact_budget_bytes,
            request,
            objects: assigned,
        };
        let snapshot_bytes = serde_json::to_vec(&snapshot)?;
        let snapshot_sha256 = hex::encode(Sha256::digest(&snapshot_bytes));
        write_json_atomic(&self.job_snapshot_path(project_id, job_id), &snapshot).await?;
        let record = StoredJobRecord {
            schema_version: 1,
            project_id,
            job_id,
            created_unix_seconds: now,
            updated_unix_seconds: now,
            status: JobStatus::Queued,
            progress_message: "Frozen input snapshot created; queued for local worker".to_string(),
            snapshot_sha256,
            cancel_requested: false,
            result: None,
            error: None,
        };
        write_json_atomic(&self.job_record_path(project_id, job_id), &record).await?;
        project.last_accessed_unix_seconds = now;
        project.job_count = project.job_count.saturating_add(1);
        project.job_snapshot_bytes = project
            .job_snapshot_bytes
            .saturating_add(logical_snapshot_bytes);
        project.storage_bytes = project.storage_bytes.saturating_add(logical_snapshot_bytes);
        write_json_atomic(&self.project_record_path(project_id), &project).await?;
        Ok(JobDetailResponse {
            summary: job_summary_from_record(&record),
            result: None,
        })
    }

    async fn update_job_state(
        &self,
        project_id: Uuid,
        job_id: Uuid,
        status: JobStatus,
        progress_message: impl Into<String>,
        result: Option<JobResultSummary>,
        error: Option<String>,
    ) -> Result<StoredJobRecord> {
        let _guard = self.mutation_lock.lock().await;
        let mut record = self.read_job_record(project_id, job_id).await?;
        if record.status == JobStatus::Cancelled {
            return Ok(record);
        }
        record.status = status;
        record.progress_message = progress_message.into();
        record.updated_unix_seconds = unix_seconds()?;
        if result.is_some() {
            record.result = result;
        }
        if error.is_some() {
            record.error = error;
        }
        write_json_atomic(&self.job_record_path(project_id, job_id), &record).await?;
        Ok(record)
    }

    async fn update_job_progress(
        &self,
        project_id: Uuid,
        job_id: Uuid,
        progress: JobWorkerProgress,
    ) -> Result<()> {
        let _guard = self.mutation_lock.lock().await;
        let mut record = self.read_job_record(project_id, job_id).await?;
        if record.status != JobStatus::Running || record.cancel_requested {
            return Ok(());
        }
        let message = if progress.total > 0 {
            let percent = ((progress.done.min(progress.total) as f64 / progress.total as f64)
                * 100.0)
                .round() as usize;
            format!("{percent}% — {}", progress.message)
        } else {
            progress.message
        };
        if record.progress_message == message {
            return Ok(());
        }
        record.progress_message = message;
        record.updated_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.job_record_path(project_id, job_id), &record).await?;
        Ok(())
    }

    async fn cancel_job_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        job_id: Uuid,
    ) -> Result<JobSummaryResponse> {
        let _guard = self.mutation_lock.lock().await;
        let _ = self.authorize_record(project_id, token).await?;
        let mut record = self.read_job_record(project_id, job_id).await?;
        if !record.status.is_terminal() {
            record.cancel_requested = true;
            if record.status == JobStatus::Queued || record.status == JobStatus::Preparing {
                record.status = JobStatus::Cancelled;
                record.progress_message = "Cancelled before scientific execution".to_string();
            } else {
                record.progress_message =
                    "Cancellation requested; current native stage will finish before stopping"
                        .to_string();
            }
            record.updated_unix_seconds = unix_seconds()?;
            write_json_atomic(&self.job_record_path(project_id, job_id), &record).await?;
        }
        Ok(job_summary_from_record(&record))
    }

    async fn delete_job_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        job_id: Uuid,
    ) -> Result<(bool, ProjectSummary)> {
        let _guard = self.mutation_lock.lock().await;
        let mut project = self.authorize_record(project_id, token).await?;
        let record = self.read_job_record(project_id, job_id).await?;
        if !record.status.is_terminal() {
            bail!("only terminal persisted jobs can be deleted; cancel the job first");
        }
        let job_dir = self.job_dir(project_id, job_id);
        let deleted = match fs::remove_dir_all(&job_dir).await {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error).context("failed to delete persisted job directory"),
        };
        self.reconcile_record_storage(&mut project).await?;
        project.last_accessed_unix_seconds = unix_seconds()?;
        write_json_atomic(&self.project_record_path(project_id), &project).await?;
        Ok((deleted, ProjectSummary::from(&project)))
    }

    async fn is_job_cancel_requested(&self, project_id: Uuid, job_id: Uuid) -> Result<bool> {
        let _guard = self.mutation_lock.lock().await;
        let record = self.read_job_record(project_id, job_id).await?;
        Ok(record.cancel_requested || record.status == JobStatus::Cancelled)
    }

    async fn run_job(self: Arc<Self>, project_id: Uuid, job_id: Uuid) {
        let preparation = self
            .update_job_state(
                project_id,
                job_id,
                JobStatus::Preparing,
                "Preparing frozen local scientific snapshot",
                None,
                None,
            )
            .await;
        if let Err(error) = preparation {
            eprintln!("4C job preparation error: {error:#}");
            return;
        }
        if self
            .is_job_cancel_requested(project_id, job_id)
            .await
            .unwrap_or(true)
        {
            let _ = self
                .update_job_state(
                    project_id,
                    job_id,
                    JobStatus::Cancelled,
                    "Job cancelled",
                    None,
                    None,
                )
                .await;
            return;
        }
        let snapshot_path = self.job_snapshot_path(project_id, job_id);
        let snapshot_bytes = match fs::read(&snapshot_path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = self
                    .update_job_state(
                        project_id,
                        job_id,
                        JobStatus::Failed,
                        "Job snapshot could not be read",
                        None,
                        Some(error.to_string()),
                    )
                    .await;
                return;
            }
        };
        let snapshot: StoredJobSnapshot = match serde_json::from_slice(&snapshot_bytes) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = self
                    .update_job_state(
                        project_id,
                        job_id,
                        JobStatus::Failed,
                        "Job snapshot could not be parsed",
                        None,
                        Some(error.to_string()),
                    )
                    .await;
                return;
            }
        };
        let _ = self
            .update_job_state(
                project_id,
                job_id,
                JobStatus::Running,
                if snapshot.request.run_full_unified {
                    "Running frozen native unified multi-omics analysis"
                } else {
                    "Running native mapping/QC and representative corrected gene lane"
                },
                None,
                None,
            )
            .await;
        let job_dir = self.job_dir(project_id, job_id);
        let (progress_tx, mut progress_rx) =
            tokio::sync::mpsc::unbounded_channel::<JobWorkerProgress>();
        let mut worker = tokio::task::spawn_blocking(move || {
            execute_frozen_job(&job_dir, &snapshot, |done, total, message| {
                let _ = progress_tx.send(JobWorkerProgress {
                    done,
                    total,
                    message: message.to_string(),
                });
            })
        });
        let result = loop {
            tokio::select! {
                result = &mut worker => break result,
                Some(progress) = progress_rx.recv() => {
                    let _ = self.update_job_progress(project_id, job_id, progress).await;
                }
            }
        };
        let scientific = match result {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                let _ = self
                    .update_job_state(
                        project_id,
                        job_id,
                        JobStatus::Failed,
                        "Native scientific job failed",
                        None,
                        Some(error.to_string()),
                    )
                    .await;
                return;
            }
            Err(error) => {
                let _ = self
                    .update_job_state(
                        project_id,
                        job_id,
                        JobStatus::Failed,
                        "Native worker task failed",
                        None,
                        Some(error.to_string()),
                    )
                    .await;
                return;
            }
        };
        if self
            .is_job_cancel_requested(project_id, job_id)
            .await
            .unwrap_or(false)
        {
            let _ = fs::remove_dir_all(self.job_artifacts_dir(project_id, job_id)).await;
            let _ = self
                .update_job_state(
                    project_id,
                    job_id,
                    JobStatus::Cancelled,
                    "Job cancelled after the current native stage finished; generated artifacts were discarded",
                    None,
                    None,
                )
                .await;
            let _ = self.refresh_project_storage_record(project_id).await;
            return;
        }
        let _ = self
            .update_job_state(
                project_id,
                job_id,
                JobStatus::Complete,
                if scientific.unified.is_some() {
                    "Native Phase-3E unified job complete"
                } else {
                    "Native Phase-3D job complete"
                },
                Some(scientific),
                None,
            )
            .await;
        let _ = self.refresh_project_storage_record(project_id).await;
    }

    async fn read_job_artifact_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        job_id: Uuid,
        artifact_id: Uuid,
    ) -> Result<(StoredJobArtifactRecord, Vec<u8>)> {
        let _guard = self.mutation_lock.lock().await;
        let _ = self.authorize_record(project_id, token).await?;
        let _ = self.read_job_record(project_id, job_id).await?;
        let meta_path = self.job_artifact_record_path(project_id, job_id, artifact_id);
        let bytes = fs::read(&meta_path)
            .await
            .with_context(|| format!("artifact {} does not exist", artifact_id))?;
        let record: StoredJobArtifactRecord = serde_json::from_slice(&bytes)?;
        let data = fs::read(self.job_artifact_data_path(project_id, job_id, artifact_id))
            .await
            .with_context(|| format!("artifact {} data does not exist", artifact_id))?;
        Ok((record, data))
    }

    async fn list_job_artifact_records_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        job_id: Uuid,
    ) -> Result<Vec<StoredJobArtifactRecord>> {
        let _guard = self.mutation_lock.lock().await;
        let _ = self.authorize_record(project_id, token).await?;
        let _ = self.read_job_record(project_id, job_id).await?;
        let dir = self.job_artifacts_dir(project_id, job_id);
        if !fs::try_exists(&dir).await? {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        let mut entries = fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&path).await?;
            if let Ok(record) = serde_json::from_slice::<StoredJobArtifactRecord>(&bytes) {
                records.push(record);
            }
        }
        records.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.artifact_id.cmp(&b.artifact_id)));
        Ok(records)
    }

    pub async fn recover_interrupted_jobs(&self) -> Result<usize> {
        let _guard = self.mutation_lock.lock().await;
        let projects_root = self.projects_root();
        let mut projects = fs::read_dir(&projects_root).await?;
        let mut changed = 0usize;
        while let Some(project) = projects.next_entry().await? {
            if !project.file_type().await?.is_dir() {
                continue;
            }
            let Some(name) = project.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(project_id) = Uuid::parse_str(&name) else {
                continue;
            };
            for mut job in self.list_job_records(project_id).await.unwrap_or_default() {
                if matches!(
                    job.status,
                    JobStatus::Queued | JobStatus::Preparing | JobStatus::Running
                ) {
                    job.status = JobStatus::Failed;
                    job.progress_message = "Interrupted by server restart".to_string();
                    job.error = Some("The local server stopped before this job completed; submit a new job from the frozen current project state.".to_string());
                    job.updated_unix_seconds = unix_seconds()?;
                    write_json_atomic(&self.job_record_path(project_id, job.job_id), &job).await?;
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }

    async fn delete_object_authorized(
        &self,
        project_id: Uuid,
        token: &str,
        object_id: Uuid,
    ) -> Result<(bool, ProjectSummary)> {
        let _guard = self.mutation_lock.lock().await;
        let mut project = self.authorize_record(project_id, token).await?;
        let object = self.read_object_record(project_id, object_id).await?;
        let data_path = self.object_data_path(project_id, object_id);
        let metadata_path = self.object_record_path(project_id, object_id);
        match fs::remove_file(&data_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to remove {}", data_path.display()));
            }
        }
        match fs::remove_file(&metadata_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to remove {}", metadata_path.display()));
            }
        }
        project.uploaded_objects = project.uploaded_objects.saturating_sub(1);
        project.storage_bytes = project.storage_bytes.saturating_sub(object.byte_size);
        project.last_accessed_unix_seconds = unix_seconds()?;
        self.reconcile_record_storage(&mut project).await?;
        write_json_atomic(&self.project_record_path(project_id), &project).await?;
        self.refresh_input_manifest(project_id).await?;
        Ok((true, ProjectSummary::from(&project)))
    }

    async fn delete_unchecked(&self, project_id: Uuid) -> Result<bool> {
        let dir = self.project_dir(project_id);
        match fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => {
                Err(error).with_context(|| format!("failed to remove project {}", project_id))
            }
        }
    }

    pub async fn cleanup_abandoned_uploads(&self, min_age_seconds: u64) -> Result<usize> {
        let _guard = self.mutation_lock.lock().await;
        let projects_root = self.projects_root();
        let mut projects = fs::read_dir(&projects_root)
            .await
            .with_context(|| format!("failed to scan {}", projects_root.display()))?;
        let mut removed = 0usize;
        while let Some(project) = projects.next_entry().await? {
            if !project.file_type().await?.is_dir() {
                continue;
            }
            let scratch = project.path().join("scratch");
            let Ok(mut entries) = fs::read_dir(&scratch).await else {
                continue;
            };
            while let Some(entry) = entries.next_entry().await? {
                if !entry.file_type().await?.is_file() {
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with("upload-") || !name.ends_with(".part") {
                    continue;
                }
                let old_enough = if min_age_seconds == 0 {
                    true
                } else {
                    entry
                        .metadata()
                        .await
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .map(|age| age.as_secs() >= min_age_seconds)
                        .unwrap_or(false)
                };
                if old_enough {
                    match fs::remove_file(entry.path()).await {
                        Ok(()) => removed += 1,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => {
                            return Err(error)
                                .context("failed to remove abandoned upload scratch file");
                        }
                    }
                }
            }
        }
        Ok(removed)
    }

    pub async fn cleanup_expired(&self) -> Result<usize> {
        let _guard = self.mutation_lock.lock().await;
        let now = unix_seconds()?;
        let projects_root = self.projects_root();
        let mut entries = fs::read_dir(&projects_root)
            .await
            .with_context(|| format!("failed to scan {}", projects_root.display()))?;
        let mut removed = 0usize;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(project_id) = Uuid::parse_str(&name) else {
                continue;
            };
            let Ok(record) = self.read_record(project_id).await else {
                continue;
            };
            if now >= record.expires_unix_seconds && self.delete_unchecked(project_id).await? {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn infer_auto_assignment_from_filename(
    original_name: &str,
) -> Option<(UploadedInputRole, Option<String>)> {
    let classification = classify_input_filename(original_name)?;
    let role = UploadedInputRole::from_filename_kind(classification.kind)?;
    let sample_id = normalize_assigned_sample_id(role, classification.sample_id.as_deref()).ok()?;
    Some((role, sample_id))
}

fn normalize_assigned_sample_id(
    role: UploadedInputRole,
    value: Option<&str>,
) -> Result<Option<String>> {
    if !role.requires_sample_id() {
        return Ok(None);
    }
    let value = value.unwrap_or_default().trim();
    if value.is_empty() {
        bail!("this modality role requires an explicit sample ID");
    }
    if value.chars().count() > 200 {
        bail!("sample ID is too long");
    }
    if value
        .chars()
        .any(|ch| ch == '\0' || ch == '\n' || ch == '\r' || ch.is_control())
    {
        bail!("sample ID contains invalid control characters");
    }
    Ok(Some(value.to_string()))
}

fn validate_object_for_role(
    path: &FsPath,
    original_name: &str,
    role: UploadedInputRole,
) -> Result<()> {
    if role == UploadedInputRole::Unassigned {
        return Ok(());
    }
    let ext = FsPath::new(original_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match role {
        UploadedInputRole::PrimaryMetadata | UploadedInputRole::FilteredMetadata => {
            match ext.as_str() {
                "xlsx" => validate_xlsx_container(path),
                "csv" | "tsv" | "txt" => validate_text_container(path),
                "xls" => bail!(
                    "legacy .xls metadata is not accepted by the Phase-3C web validator; convert it to .xlsx, .tsv, or .csv first"
                ),
                _ => bail!("metadata role requires .xlsx, .tsv, .csv, or .txt"),
            }
        }
        UploadedInputRole::GeneExpression
        | UploadedInputRole::TranscriptExpression
        | UploadedInputRole::Bcr
        | UploadedInputRole::BcrIsotype
        | UploadedInputRole::TcrAlpha
        | UploadedInputRole::TcrBeta
        | UploadedInputRole::ImmuneInfiltrate => {
            if ext != "xlsx" {
                bail!("sample-level 4C modality roles currently require .xlsx workbooks");
            }
            validate_xlsx_container(path)
        }
        UploadedInputRole::MergedGeneExpression | UploadedInputRole::MergedTranscriptExpression => {
            if !matches!(ext.as_str(), "tsv" | "txt") {
                bail!("merged expression roles currently require .tsv or .txt tables");
            }
            validate_text_container(path)
        }
        UploadedInputRole::PathwayGmt => {
            if ext != "gmt" {
                bail!("pathway gene-set role requires a .gmt file");
            }
            validate_text_container(path)
        }
        UploadedInputRole::Unassigned => Ok(()),
    }
}

fn validate_text_container(path: &FsPath) -> Result<()> {
    use std::io::Read;
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open uploaded text object {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.contains(&0) {
        bail!("text input contains NUL bytes and does not look like a delimited text table");
    }
    if let Err(error) = std::str::from_utf8(&bytes) {
        if error.error_len().is_some() {
            bail!("text input is not valid UTF-8");
        }
        std::str::from_utf8(&bytes[..error.valid_up_to()])
            .context("text input is not valid UTF-8")?;
    }
    Ok(())
}

fn validate_xlsx_container(path: &FsPath) -> Result<()> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open uploaded workbook {}", path.display()))?;
    let mut archive =
        ZipArchive::new(file).context("uploaded .xlsx is not a readable ZIP/XLSX container")?;
    if archive.len() == 0 || archive.len() > MAX_XLSX_ENTRIES {
        bail!("uploaded .xlsx has an invalid or excessive ZIP entry count");
    }
    let mut total_uncompressed = 0u64;
    let mut has_content_types = false;
    let mut has_workbook = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .context("failed to inspect XLSX ZIP entry")?;
        let size = entry.size();
        if size > MAX_XLSX_ENTRY_BYTES {
            bail!("uploaded .xlsx contains an excessively large ZIP entry");
        }
        total_uncompressed = total_uncompressed
            .checked_add(size)
            .ok_or_else(|| anyhow!("XLSX uncompressed-size overflow"))?;
        if total_uncompressed > MAX_XLSX_UNCOMPRESSED_BYTES {
            bail!("uploaded .xlsx expands beyond the Phase-3C safety limit");
        }
        let compressed = entry.compressed_size();
        if compressed > 0 && size / compressed.max(1) > 500 {
            bail!("uploaded .xlsx contains a suspiciously high compression-ratio entry");
        }
        let name = entry.name();
        if name == "[Content_Types].xml" {
            has_content_types = true;
        } else if name == "xl/workbook.xml" {
            has_workbook = true;
        }
    }
    if !has_content_types || !has_workbook {
        bail!("uploaded .xlsx is missing required workbook container entries");
    }
    Ok(())
}

fn materialize_metadata_object(
    temp_root: &FsPath,
    data_path: &FsPath,
    original_name: &str,
) -> Result<PathBuf> {
    let ext = FsPath::new(original_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let target = temp_root.join(format!("primary_metadata.{ext}"));
    match std::fs::hard_link(data_path, &target) {
        Ok(()) => {}
        Err(_) => {
            std::fs::copy(data_path, &target).with_context(|| {
                format!(
                    "failed to materialize metadata object as {}",
                    target.display()
                )
            })?;
        }
    }
    Ok(target)
}

fn build_server_metadata_profile(
    project_dir: &FsPath,
    primary: &StoredObjectRecord,
) -> Result<MetadataProfileResponse> {
    if primary.role != UploadedInputRole::PrimaryMetadata {
        bail!("metadata profile requires an object assigned as primary_metadata");
    }
    let data_path = project_dir
        .join("inputs")
        .join(format!("{}.bin", primary.object_id));
    validate_object_for_role(&data_path, &primary.original_name, primary.role)?;

    let scratch = project_dir.join("scratch");
    std::fs::create_dir_all(&scratch)?;
    let temp = tempfile::Builder::new()
        .prefix("metadata-profile-")
        .tempdir_in(&scratch)
        .context("failed to create private metadata-profile scratch directory")?;
    let metadata_path =
        materialize_metadata_object(temp.path(), &data_path, &primary.original_name)?;
    let metadata = load_metadata(&metadata_path)?;
    let validation = validate_standard_metadata(&metadata);

    let mut columns = Vec::with_capacity(metadata.columns.len());
    for name in &metadata.columns {
        let counts = metadata_unique_values(&metadata, name);
        let distinct_nonempty_values = counts.len();
        let values_truncated = distinct_nonempty_values > MAX_METADATA_PROFILE_VALUES;
        let values = counts
            .into_iter()
            .take(MAX_METADATA_PROFILE_VALUES)
            .map(|(value, count)| MetadataValueCount { value, count })
            .collect::<Vec<_>>();
        columns.push(MetadataColumnProfile {
            name: name.clone(),
            distinct_nonempty_values,
            comparison_ready: name != &metadata.id_column
                && (2..=20).contains(&distinct_nonempty_values),
            values_truncated,
            values,
        });
    }

    let suggested_comparison_column = columns
        .iter()
        .find(|column| {
            column.name == "analysis_group" && column.distinct_nonempty_values >= 2
        })
        .map(|column| column.name.clone())
        .or_else(|| {
            columns
                .iter()
                .find(|column| column.comparison_ready)
                .map(|column| column.name.clone())
        });

    Ok(MetadataProfileResponse {
        metadata_object_id: primary.object_id.to_string(),
        metadata_sha256: primary.sha256.clone(),
        metadata_rows: metadata.rows.len(),
        metadata_id_column: metadata.id_column,
        suggested_comparison_column,
        columns,
        validation_errors: validation.errors,
        validation_warnings: validation.warnings,
    })
}

fn build_server_mapping_audit(
    project_id: Uuid,
    project_dir: &FsPath,
    objects: &[StoredObjectRecord],
    request: MappingAuditRequest,
    mapping_snapshot_sha256: String,
) -> Result<MappingAuditResponse> {
    let primary = objects
        .iter()
        .filter(|object| object.role == UploadedInputRole::PrimaryMetadata)
        .collect::<Vec<_>>();
    if primary.len() != 1 {
        bail!(
            "assign exactly one uploaded object as primary_metadata before running the mapping audit"
        );
    }
    for object in objects
        .iter()
        .filter(|object| object.role != UploadedInputRole::Unassigned)
    {
        validate_object_for_role(
            &project_dir
                .join("inputs")
                .join(format!("{}.bin", object.object_id)),
            &object.original_name,
            object.role,
        )?;
        if object.role.requires_sample_id() {
            normalize_assigned_sample_id(object.role, object.sample_id.as_deref())?;
        }
    }

    let scratch = project_dir.join("scratch");
    std::fs::create_dir_all(&scratch)?;
    let temp = tempfile::Builder::new()
        .prefix("mapping-")
        .tempdir_in(&scratch)
        .context("failed to create private mapping scratch directory")?;
    let primary_object = primary[0];
    let primary_data = project_dir
        .join("inputs")
        .join(format!("{}.bin", primary_object.object_id));
    let metadata_path =
        materialize_metadata_object(temp.path(), &primary_data, &primary_object.original_name)?;
    let metadata = load_metadata(&metadata_path)?;

    let mut scan = ProjectScan {
        root: temp.path().to_path_buf(),
        files: Vec::new(),
        merged_gene_tsv: None,
        merged_transcript_tsv: None,
        metadata_candidates: vec![metadata_path.clone()],
    };
    for object in objects {
        let data_path = project_dir
            .join("inputs")
            .join(format!("{}.bin", object.object_id));
        if let Some(modality) = object.role.cohort_modality() {
            scan.files.push(DetectedCohortFile {
                path: data_path,
                modality,
                sample_id: object.sample_id.clone().unwrap_or_default(),
            });
        } else if object.role == UploadedInputRole::MergedGeneExpression {
            scan.merged_gene_tsv = Some(data_path);
        } else if object.role == UploadedInputRole::MergedTranscriptExpression {
            scan.merged_transcript_tsv = Some(data_path);
        }
    }
    let mapping = build_mapping(&scan, &metadata);

    let canonical_samples = (0..metadata.rows.len())
        .filter_map(|row| canonical_sample_id(&metadata, row))
        .map(crate::cohort::normalize_sample_id)
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .len();

    let mut warnings = Vec::new();
    let unassigned = objects
        .iter()
        .filter(|object| object.role == UploadedInputRole::Unassigned)
        .count();
    if unassigned > 0 {
        warnings.push(format!(
            "{unassigned} uploaded object(s) remain unassigned and are ignored by mapping"
        ));
    }
    if objects
        .iter()
        .any(|object| object.role == UploadedInputRole::FilteredMetadata)
    {
        warnings.push("Filtered/subset metadata is recorded but the Phase-3C mapping audit uses primary metadata only".to_string());
    }
    if scan.merged_gene_tsv.is_some() || scan.merged_transcript_tsv.is_some() {
        warnings.push("Merged expression tables are recorded as project inputs but do not contribute sample-file rows to the mapping coverage audit".to_string());
    }
    if scan.files.is_empty() {
        warnings.push("No sample-level modality objects are assigned, so the mapping audit contains metadata-only samples".to_string());
    }
    if mapping.unmatched_files > 0 {
        warnings.push(format!(
            "{} assigned modality file(s) did not match primary metadata",
            mapping.unmatched_files
        ));
    }
    if mapping.ambiguous_files > 0 {
        warnings.push(format!(
            "{} assigned modality file(s) matched more than one metadata row",
            mapping.ambiguous_files
        ));
    }

    let mut comparison_name = None;
    let mut case_samples = None;
    let mut control_samples = None;
    if let Some(column) = request
        .comparison_column
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let case_values = request
            .case_values
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>();
        let control_values = request
            .control_values
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>();
        if !case_values.is_empty() && !control_values.is_empty() {
            let spec = MetadataComparisonSpec {
                column: column.to_string(),
                case_values,
                control_values,
                name: format!("Web mapping draft: {column}"),
                case_label: "Case".to_string(),
                control_label: "Control".to_string(),
            };
            match build_custom_comparison(&metadata, &spec) {
                Ok(comparison) => {
                    comparison_name = Some(comparison.name);
                    case_samples = Some(comparison.case_metadata_rows);
                    control_samples = Some(comparison.control_metadata_rows);
                }
                Err(error) => {
                    warnings.push(format!("Comparison draft could not be resolved: {error}"))
                }
            }
        }
    }

    let modality_coverage = [
        CohortModality::GeneExpression,
        CohortModality::TranscriptExpression,
        CohortModality::Bcr,
        CohortModality::BcrIsotype,
        CohortModality::TcrAlpha,
        CohortModality::TcrBeta,
        CohortModality::ImmuneInfiltrate,
    ]
    .into_iter()
    .map(|modality| {
        let assigned_files = scan
            .files
            .iter()
            .filter(|file| file.modality == modality)
            .count();
        let matched_files = mapping
            .rows
            .iter()
            .filter(|row| row.mapping_status == MappingStatus::Matched)
            .map(|row| row.counts.get(&modality).copied().unwrap_or(0))
            .sum();
        let samples_with_files = mapping
            .rows
            .iter()
            .filter(|row| row.counts.get(&modality).copied().unwrap_or(0) > 0)
            .count();
        ModalityCoverageSummary {
            modality: modality.display_name().to_string(),
            assigned_files,
            matched_files,
            samples_with_files,
        }
    })
    .collect::<Vec<_>>();

    let rows = mapping
        .rows
        .iter()
        .map(|row| MappingRowSummary {
            sample_id: row.sample_id.clone(),
            status: mapping_status_name(row.mapping_status).to_string(),
            counts: row
                .counts
                .iter()
                .map(|(modality, count)| (modality.display_name().to_string(), *count))
                .collect(),
        })
        .collect();

    let _ = project_id;
    Ok(MappingAuditResponse {
        mapping_snapshot_sha256,
        metadata_rows: metadata.rows.len(),
        canonical_samples,
        metadata_id_column: metadata.id_column,
        comparison_name,
        case_samples,
        control_samples,
        matched_files: mapping.matched_files,
        unmatched_files: mapping.unmatched_files,
        ambiguous_files: mapping.ambiguous_files,
        metadata_only_samples: mapping.metadata_only_samples,
        modality_coverage,
        rows,
        warnings,
    })
}

fn mapping_snapshot_fingerprint(
    input_manifest_sha256: &str,
    request: &MappingAuditRequest,
) -> Result<String> {
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        schema_version: u32,
        input_manifest_sha256: &'a str,
        request: &'a MappingAuditRequest,
    }
    let bytes = serde_json::to_vec(&Fingerprint {
        schema_version: 1,
        input_manifest_sha256,
        request,
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn constant_time_string_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.as_bytes().ct_eq(b.as_bytes()).unwrap_u8() == 1
}

fn job_summary_from_record(record: &StoredJobRecord) -> JobSummaryResponse {
    JobSummaryResponse {
        job_id: record.job_id.to_string(),
        created_unix_seconds: record.created_unix_seconds,
        updated_unix_seconds: record.updated_unix_seconds,
        status: record.status,
        progress_message: record.progress_message.clone(),
        snapshot_sha256: record.snapshot_sha256.clone(),
        error: record.error.clone(),
    }
}

fn build_snapshot_native_context(
    snapshot_root: &FsPath,
    objects: &[StoredObjectRecord],
    request: &SubmitJobRequest,
) -> Result<(MetadataTable, ProjectScan, CohortComparison)> {
    let primary = objects
        .iter()
        .filter(|object| object.role == UploadedInputRole::PrimaryMetadata)
        .collect::<Vec<_>>();
    if primary.len() != 1 {
        bail!("frozen job snapshot must contain exactly one primary metadata object");
    }
    let scratch = snapshot_root.join("scratch");
    std::fs::create_dir_all(&scratch)?;
    let primary_object = primary[0];
    let primary_data = snapshot_root
        .join("inputs")
        .join(format!("{}.bin", primary_object.object_id));
    let metadata_path =
        materialize_metadata_object(&scratch, &primary_data, &primary_object.original_name)?;
    let metadata = load_metadata(&metadata_path)?;
    let mut scan = ProjectScan {
        root: snapshot_root.to_path_buf(),
        files: Vec::new(),
        merged_gene_tsv: None,
        merged_transcript_tsv: None,
        metadata_candidates: vec![metadata_path],
    };
    for object in objects {
        let data_path = snapshot_root
            .join("inputs")
            .join(format!("{}.bin", object.object_id));
        if let Some(modality) = object.role.cohort_modality() {
            scan.files.push(DetectedCohortFile {
                path: data_path,
                modality,
                sample_id: object.sample_id.clone().unwrap_or_default(),
            });
        } else if object.role == UploadedInputRole::MergedGeneExpression {
            scan.merged_gene_tsv = Some(data_path);
        } else if object.role == UploadedInputRole::MergedTranscriptExpression {
            scan.merged_transcript_tsv = Some(data_path);
        }
    }
    let column = request.comparison_column.trim();
    if column.is_empty() {
        bail!("job comparison column is empty");
    }
    let case_values = request
        .case_values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    let control_values = request
        .control_values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if case_values.is_empty() || control_values.is_empty() {
        bail!("job comparison requires non-empty Case and Control values");
    }
    let comparison = build_custom_comparison(
        &metadata,
        &MetadataComparisonSpec {
            column: column.to_string(),
            case_values,
            control_values,
            name: format!("Web job: {column}"),
            case_label: "Case".to_string(),
            control_label: "Control".to_string(),
        },
    )?;
    Ok((metadata, scan, comparison))
}

fn selected_qc_modalities(scan: &ProjectScan) -> Vec<CohortModality> {
    let mut selected = Vec::new();
    if scan.merged_gene_tsv.is_some() {
        selected.push(CohortModality::GeneExpression);
    }
    if scan.merged_transcript_tsv.is_some() {
        selected.push(CohortModality::TranscriptExpression);
    }
    for modality in [
        CohortModality::Bcr,
        CohortModality::BcrIsotype,
        CohortModality::TcrAlpha,
        CohortModality::TcrBeta,
        CohortModality::ImmuneInfiltrate,
    ] {
        if scan.files.iter().any(|file| file.modality == modality) {
            selected.push(modality);
        }
    }
    selected
}

fn execute_frozen_job<F>(
    job_dir: &FsPath,
    snapshot: &StoredJobSnapshot,
    mut progress: F,
) -> Result<JobResultSummary>
where
    F: FnMut(usize, usize, &str),
{
    progress(0, 1000, "Validating frozen mapping and input snapshot");
    let snapshot_root = job_dir.join("snapshot");
    let mapping_request = MappingAuditRequest {
        comparison_column: Some(snapshot.request.comparison_column.clone()),
        case_values: snapshot.request.case_values.clone(),
        control_values: snapshot.request.control_values.clone(),
    };
    let mapping = build_server_mapping_audit(
        snapshot.project_id,
        &snapshot_root,
        &snapshot.objects,
        mapping_request,
        snapshot.mapping_snapshot_sha256.clone(),
    )?;
    let mut artifacts = Vec::new();
    let mut artifact_budget = snapshot.artifact_budget_bytes;
    artifacts.push(persist_job_artifact_budgeted(
        job_dir,
        "mapping_audit.json",
        "application/json",
        &serde_json::to_vec_pretty(&mapping)?,
        &mut artifact_budget,
    )?);
    progress(30, 1000, "Frozen mapping validated; reconstructing native inputs");

    let (metadata, scan, original_comparison) =
        build_snapshot_native_context(&snapshot_root, &snapshot.objects, &snapshot.request)?;
    progress(50, 1000, "Native frozen inputs reconstructed");
    let mut warnings = Vec::new();
    let mut qc_native = None;
    let qc = if snapshot.request.run_qc {
        let selected = selected_qc_modalities(&scan);
        if selected.is_empty() {
            warnings.push(
                "No frozen modality could produce cross-modal/sample QC; QC was skipped"
                    .to_string(),
            );
            None
        } else {
            match run_cross_modal_outlier_qc(
                scan.merged_gene_tsv.as_deref(),
                scan.merged_transcript_tsv.as_deref(),
                &scan.files,
                &original_comparison,
                &selected,
                &OutlierConfig::default(),
                |done, total, message| {
                    let denom = total.max(1);
                    let scaled = 50 + done.min(denom) * 100 / denom;
                    progress(scaled, 1000, &format!("Cross-modal QC — {message}"));
                },
            ) {
                Ok(result) => {
                    let summary = QcResultSummary {
                        comparison_name: result.comparison_name.clone(),
                        modalities: result
                            .modalities
                            .iter()
                            .map(|item| item.modality.clone())
                            .collect(),
                        samples: result.samples.len(),
                        consensus_ids: result.consensus_ids.clone(),
                        flagged_samples: result
                            .samples
                            .iter()
                            .filter(|sample| sample.flagged_modalities > 0 || sample.consensus_flag)
                            .map(|sample| QcSampleSummary {
                                sample_id: sample.sample_id.clone(),
                                group: sample.group.clone(),
                                available_modalities: sample.available_modalities,
                                flagged_modalities: sample.flagged_modalities,
                                flagged_names: sample.flagged_names.clone(),
                                consensus_flag: sample.consensus_flag,
                            })
                            .collect(),
                        warnings: result.warnings.clone(),
                    };
                    artifacts.push(persist_job_artifact_budgeted(
                        job_dir,
                        "qc_summary.json",
                        "application/json",
                        &serde_json::to_vec_pretty(&summary)?,
                        &mut artifact_budget,
                    )?);
                    qc_native = Some(result);
                    Some(summary)
                }
                Err(error) => {
                    warnings.push(format!("Cross-modal/sample QC skipped: {error}"));
                    None
                }
            }
        }
    } else {
        None
    };
    progress(150, 1000, "Frozen QC stage complete");

    if snapshot.request.run_full_unified {
        let reviewed_exclusions = snapshot
            .request
            .excluded_sample_ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<HashSet<_>>();
        let (comparison, qc_snapshot) = if let Some(audit) = qc_native.as_ref() {
            let filtered = filter_comparison_excluding(&original_comparison, &reviewed_exclusions);
            let qc_snapshot = build_outlier_filter_snapshot(
                audit,
                &reviewed_exclusions,
                &original_comparison,
                &filtered,
                !reviewed_exclusions.is_empty(),
            );
            (filtered, Some(qc_snapshot))
        } else {
            if !reviewed_exclusions.is_empty() {
                warnings.push(
                    "Reviewed sample exclusions were supplied but no frozen QC audit was available; exclusions were not applied"
                        .to_string(),
                );
            }
            (original_comparison.clone(), None)
        };

        if comparison.case_ids.is_empty() || comparison.control_ids.is_empty() {
            bail!(
                "reviewed QC exclusions leave an empty Case or Control cohort; revise the exclusions before submitting the unified job"
            );
        }

        let mut corrected = CorrectedModelSpec::default();
        corrected.subject_column = snapshot
            .request
            .subject_column
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        corrected.covariates = snapshot
            .request
            .covariates
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();

        let pathway_gmt_path = snapshot
            .objects
            .iter()
            .find(|object| object.role == UploadedInputRole::PathwayGmt)
            .map(|object| {
                snapshot_root
                    .join("inputs")
                    .join(format!("{}.bin", object.object_id))
            });
        let modalities = CohortModalitySelection {
            gene: scan.merged_gene_tsv.is_some(),
            transcript: scan.merged_transcript_tsv.is_some(),
            bcr: scan.files.iter().any(|file| file.modality == CohortModality::Bcr),
            bcr_isotype: scan
                .files
                .iter()
                .any(|file| file.modality == CohortModality::BcrIsotype),
            tcr_alpha: scan
                .files
                .iter()
                .any(|file| file.modality == CohortModality::TcrAlpha),
            tcr_beta: scan
                .files
                .iter()
                .any(|file| file.modality == CohortModality::TcrBeta),
            immune: scan
                .files
                .iter()
                .any(|file| file.modality == CohortModality::ImmuneInfiltrate),
            pathway: scan.merged_gene_tsv.is_some() && pathway_gmt_path.is_some(),
        };
        if !modalities.any_primary() {
            bail!("no frozen modality is available for the unified native analysis");
        }

        let comparison_definition = CohortComparisonDefinition::MetadataValues(
            MetadataComparisonSpec {
                column: snapshot.request.comparison_column.trim().to_string(),
                case_values: snapshot
                    .request
                    .case_values
                    .iter()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect(),
                control_values: snapshot
                    .request
                    .control_values
                    .iter()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect(),
                name: format!(
                    "Web unified job: {}",
                    snapshot.request.comparison_column.trim()
                ),
                case_label: "Case".to_string(),
                control_label: "Control".to_string(),
            },
        );
        let cohort = CohortDefinitionSnapshot {
            project_root: scan.root.clone(),
            metadata_path: metadata.path.clone(),
            metadata_id_column: metadata.id_column.clone(),
            uses_filtered_metadata_subset: snapshot
                .objects
                .iter()
                .any(|object| object.role == UploadedInputRole::FilteredMetadata),
            comparison_definition,
            comparison,
            mapping: build_mapping(&scan, &metadata),
        };

        let pbmc_diagnostics = if snapshot.request.run_pbmc_diagnostics
            && scan.merged_gene_tsv.is_some()
        {
            let mut config = PbmcDiagnosticsConfig::default();
            config.subject_column = corrected.subject_column.clone();
            config.batch_column = normalized_optional_string(snapshot.request.pbmc_batch_column.as_deref());
            config.family_column = normalized_optional_string(snapshot.request.pbmc_family_column.as_deref());
            config.sensitivity_column =
                normalized_optional_string(snapshot.request.pbmc_sensitivity_column.as_deref());
            Some(config)
        } else {
            None
        };
        let settings = CohortAnalysisSettingsSnapshot {
            modalities,
            corrected,
            pathway_gmt_path,
            pathway_q_threshold: snapshot.request.pathway_q_threshold,
            pathway_effect_threshold: snapshot.request.pathway_effect_threshold,
            exploratory_direction_check: snapshot.request.run_exploratory_direction_check,
            candidate: crate::focused::FocusedCandidateSettings::default(),
            pbmc_diagnostics,
        };

        progress(160, 1000, "Starting native unified multi-omics analysis");
        let run = run_unified_cohort_analysis(
            &scan,
            &metadata,
            cohort,
            settings,
            qc_snapshot,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = 160 + done.min(denom) * 650 / denom;
                progress(scaled, 1000, message);
            },
        )?;
        progress(815, 1000, "Native unified analysis complete; preparing browser result summary");
        warnings.extend(run.warnings.clone());
        let unified = summarize_unified_run(&run);
        let gene = run.primary.gene.as_ref().map(summarize_corrected_gene);

        progress(835, 1000, "Browser visualization payload prepared; persisting result summary");
        artifacts.push(persist_job_artifact_budgeted(
            job_dir,
            "unified_result_summary.json",
            "application/json",
            &serde_json::to_vec_pretty(&unified)?,
            &mut artifact_budget,
        )?);
        persist_unified_run_artifacts(
            job_dir,
            &run,
            &mut artifacts,
            &mut warnings,
            &mut artifact_budget,
            |done, total, message| {
                let denom = total.max(1);
                let scaled = 850 + done.min(denom) * 145 / denom;
                progress(scaled, 1000, message);
            },
        )?;
        progress(1000, 1000, "Frozen unified job complete");

        return Ok(JobResultSummary {
            mapping,
            qc,
            gene,
            unified: Some(unified),
            warnings,
            artifacts,
        });
    }

    let gene = if snapshot.request.run_gene_corrected {
        if let Some(path) = scan.merged_gene_tsv.as_deref() {
            let mut spec = CorrectedModelSpec::default();
            spec.subject_column = snapshot
                .request
                .subject_column
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            spec.covariates = snapshot
                .request
                .covariates
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            match run_corrected_expression(
                path,
                &metadata,
                &original_comparison,
                "Raw Count",
                "Gene",
                &spec,
                |done, total, message| {
                    let denom = total.max(1);
                    let scaled = 200 + done.min(denom) * 750 / denom;
                    progress(scaled, 1000, message);
                },
            ) {
                Ok(result) => {
                    let summary = summarize_corrected_gene(&result);
                    let tsv = corrected_gene_result_tsv(&result)?;
                    artifacts.push(persist_job_artifact_budgeted(
                        job_dir,
                        "gene_corrected_results.tsv",
                        "text/tab-separated-values",
                        &tsv,
                        &mut artifact_budget,
                    )?);
                    Some(summary)
                }
                Err(error) => {
                    warnings.push(format!(
                        "Representative corrected gene lane failed: {error}"
                    ));
                    None
                }
            }
        } else {
            warnings.push(
                "Representative corrected gene lane requested, but no merged_gene_expression object is assigned"
                    .to_string(),
            );
            None
        }
    } else {
        None
    };

    progress(1000, 1000, "Frozen representative job complete");
    Ok(JobResultSummary {
        mapping,
        qc,
        gene,
        unified: None,
        warnings,
        artifacts,
    })
}

fn normalized_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn summarize_corrected_gene(result: &CorrectedExpressionResult) -> GeneCorrectedSummary {
    let mut ranked = result.tests.clone();
    ranked.sort_by(|a, b| {
        a.q_value
            .total_cmp(&b.q_value)
            .then_with(|| b.beta_group.abs().total_cmp(&a.beta_group.abs()))
            .then_with(|| a.feature.cmp(&b.feature))
    });
    GeneCorrectedSummary {
        n_input_samples: result.n_input_samples,
        n_model_samples: result.n_model_samples,
        n_subjects: result.n_subjects,
        tested_features: result.tested_features,
        filtered_features: result.filtered_features,
        model_description: result.model_description.clone(),
        top_features: ranked
            .iter()
            .take(50)
            .map(|item| GeneFeatureSummary {
                feature: item.feature.clone(),
                secondary_id: item.secondary_id.clone(),
                n_samples: item.n_samples,
                mean_case: item.mean_case,
                mean_control: item.mean_control,
                beta_group: item.beta_group,
                robust_se: item.robust_se,
                p_value: item.p_value,
                q_value: item.q_value,
            })
            .collect(),
        warnings: result.warnings.clone(),
    }
}

fn summarize_unified_run(run: &UnifiedCohortAnalysisRun) -> UnifiedAnalysisSummary {
    let mut modalities = Vec::new();
    if run.settings.modalities.gene {
        modalities.push(unified_modality_summary(
            "Gene expression",
            run.primary.gene.as_ref().map(|result| result.tests.len()),
            run.primary.gene_error.as_deref(),
        ));
    }
    if run.settings.modalities.transcript {
        modalities.push(unified_modality_summary(
            "Transcript expression",
            run.primary.transcript.as_ref().map(|result| result.tests.len()),
            run.primary.transcript_error.as_deref(),
        ));
    }
    if run.settings.modalities.bcr {
        modalities.push(unified_modality_summary(
            "BCR",
            run.primary.bcr.as_ref().map(|result| {
                result.metric_tests.len()
                    + result
                        .compositions
                        .iter()
                        .map(|composition| composition.tests.len())
                        .sum::<usize>()
            }),
            run.primary.bcr_error.as_deref(),
        ));
    }
    if run.settings.modalities.bcr_isotype {
        modalities.push(unified_modality_summary(
            "BCR isotype",
            run.primary.bcr_isotype.as_ref().map(|result| result.tests.len()),
            run.primary.bcr_isotype_error.as_deref(),
        ));
    }
    if run.settings.modalities.tcr_alpha {
        modalities.push(unified_modality_summary(
            "TCR alpha",
            run.primary.tcr_alpha.as_ref().map(|result| {
                result.metric_tests.len()
                    + result
                        .compositions
                        .iter()
                        .map(|composition| composition.tests.len())
                        .sum::<usize>()
            }),
            run.primary.tcr_alpha_error.as_deref(),
        ));
    }
    if run.settings.modalities.tcr_beta {
        modalities.push(unified_modality_summary(
            "TCR beta",
            run.primary.tcr_beta.as_ref().map(|result| {
                result.metric_tests.len()
                    + result
                        .compositions
                        .iter()
                        .map(|composition| composition.tests.len())
                        .sum::<usize>()
            }),
            run.primary.tcr_beta_error.as_deref(),
        ));
    }
    if run.settings.modalities.immune {
        modalities.push(unified_modality_summary(
            "Immune infiltrate",
            run.primary.immune.as_ref().map(|result| result.tests.len()),
            run.primary.immune_error.as_deref(),
        ));
    }
    if run.settings.modalities.pathway {
        modalities.push(unified_modality_summary(
            "Pathway ORA",
            run.primary.pathway.as_ref().map(|result| result.tests.len()),
            run.primary.pathway_error.as_deref(),
        ));
    }

    let mut candidates = Vec::new();
    for row in run.candidates.genes.iter().take(15) {
        candidates.push(UnifiedCandidateSummary {
            rank: row.rank,
            modality: "Gene".to_string(),
            feature: row.gene.clone(),
            effect: row.adjusted_log2fc,
            q_value: row.q_value,
            evidence: row.evidence.clone(),
            robustness: row.robustness.clone(),
        });
    }
    for row in run.candidates.pathways.iter().take(12) {
        candidates.push(UnifiedCandidateSummary {
            rank: row.rank,
            modality: "Pathway".to_string(),
            feature: row.pathway.clone(),
            effect: row.odds_ratio,
            q_value: row.q_value,
            evidence: row.evidence.clone(),
            robustness: None,
        });
    }
    for row in run.candidates.other.iter().take(20) {
        candidates.push(UnifiedCandidateSummary {
            rank: row.rank,
            modality: row.modality.clone(),
            feature: row.feature.clone(),
            effect: row.effect,
            q_value: row.q_value,
            evidence: row.evidence.clone(),
            robustness: None,
        });
    }

    UnifiedAnalysisSummary {
        comparison_name: run.primary.comparison.name.clone(),
        case_samples: run.case_n(),
        control_samples: run.control_n(),
        excluded_sample_ids: run
            .qc_snapshot
            .as_ref()
            .map(|snapshot| snapshot.excluded_ids.clone())
            .unwrap_or_default(),
        modalities,
        exploratory_gene_available: run.exploratory_support.gene.is_some(),
        exploratory_transcript_available: run.exploratory_support.transcript.is_some(),
        pbmc_diagnostics_available: run.diagnostics.is_some(),
        candidates,
        visualizations: build_unified_visualizations(run),
    }
}

fn build_unified_visualizations(run: &UnifiedCohortAnalysisRun) -> UnifiedVisualizationSummary {
    let mut out = UnifiedVisualizationSummary::default();

    if let Some(result) = run.primary.gene.as_ref() {
        out.expression_panels
            .push(expression_visualization("Gene expression", result));
        if let Some(ord) = result.pca.as_ref() {
            out.ordinations
                .push(ordination_visualization("Gene expression", ord));
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            out.dendrograms
                .push(dendrogram_visualization("Gene expression", tree));
        }
    }
    if let Some(result) = run.primary.transcript.as_ref() {
        out.expression_panels
            .push(expression_visualization("Transcript expression", result));
        if let Some(ord) = result.pca.as_ref() {
            out.ordinations
                .push(ordination_visualization("Transcript expression", ord));
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            out.dendrograms
                .push(dendrogram_visualization("Transcript expression", tree));
        }
    }
    if let Some(result) = run.primary.bcr.as_ref() {
        push_repertoire_visualizations(&mut out, "BCR", result);
    }
    if let Some(result) = run.primary.bcr_isotype.as_ref() {
        out.effect_panels.push(effect_visualization(
            "BCR isotype",
            "CLR isotype effects",
            &result.tests,
        ));
        if let Some(ord) = result.ordination.as_ref() {
            out.ordinations
                .push(ordination_visualization("BCR isotype", ord));
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            out.dendrograms
                .push(dendrogram_visualization("BCR isotype", tree));
        }
    }
    if let Some(result) = run.primary.tcr_alpha.as_ref() {
        push_repertoire_visualizations(&mut out, "TCR alpha", result);
    }
    if let Some(result) = run.primary.tcr_beta.as_ref() {
        push_repertoire_visualizations(&mut out, "TCR beta", result);
    }
    if let Some(result) = run.primary.immune.as_ref() {
        out.effect_panels.push(effect_visualization(
            "Immune infiltrate",
            "Adjusted immune-cell effects",
            &result.tests,
        ));
        if let Some(ord) = result.pca.as_ref() {
            out.ordinations
                .push(ordination_visualization("Immune infiltrate", ord));
        }
        if let Some(tree) = result.dendrogram.as_ref() {
            out.dendrograms
                .push(dendrogram_visualization("Immune infiltrate", tree));
        }
    }
    if let Some(result) = run.primary.pathway.as_ref() {
        let mut rows = result
            .tests
            .iter()
            .filter(|row| {
                row.q_value.is_finite()
                    && row.q_value >= 0.0
                    && row.p_value.is_finite()
                    && row.odds_ratio.is_finite()
                    && row.odds_ratio > 0.0
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            a.q_value
                .total_cmp(&b.q_value)
                .then_with(|| a.p_value.total_cmp(&b.p_value))
                .then_with(|| b.odds_ratio.total_cmp(&a.odds_ratio))
        });
        out.pathway_points = rows
            .into_iter()
            .take(MAX_VIS_PATHWAY_POINTS)
            .map(|row| PathwayVisualizationPoint {
                pathway: row.pathway.clone(),
                description: row.description.clone(),
                measured_set_size: row.measured_set_size,
                overlap: row.overlap,
                odds_ratio: row.odds_ratio,
                p_value: row.p_value,
                q_value: row.q_value,
            })
            .collect();
    }

    out
}

fn expression_visualization(
    modality: &str,
    result: &CorrectedExpressionResult,
) -> ExpressionVisualizationSummary {
    let valid = result
        .tests
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row.beta_group.is_finite()
                && row.p_value.is_finite()
                && row.q_value.is_finite()
                && row.q_value >= 0.0
        })
        .collect::<Vec<_>>();
    let significant_q05 = valid
        .iter()
        .filter(|(_, row)| row.q_value <= 0.05)
        .count();

    let mut ranked = valid.clone();
    ranked.sort_by(|(_, a), (_, b)| {
        a.q_value
            .total_cmp(&b.q_value)
            .then_with(|| a.p_value.total_cmp(&b.p_value))
            .then_with(|| b.beta_group.abs().total_cmp(&a.beta_group.abs()))
    });

    let signal_budget = MAX_VIS_EXPRESSION_POINTS / 2;
    let mut selected = BTreeSet::new();
    for (idx, _) in ranked.iter().take(signal_budget) {
        selected.insert(*idx);
    }
    let remaining = MAX_VIS_EXPRESSION_POINTS.saturating_sub(selected.len());
    if remaining > 0 && !valid.is_empty() {
        let stride = (valid.len() as f64 / remaining as f64).max(1.0);
        let mut cursor = 0.0f64;
        while selected.len() < MAX_VIS_EXPRESSION_POINTS && (cursor as usize) < valid.len() {
            selected.insert(valid[cursor as usize].0);
            cursor += stride;
        }
    }

    let points = selected
        .into_iter()
        .filter_map(|idx| result.tests.get(idx))
        .map(feature_visualization_point)
        .collect::<Vec<_>>();

    ExpressionVisualizationSummary {
        modality: modality.to_string(),
        tested_features: result.tested_features,
        filtered_features: result.filtered_features,
        significant_q05,
        plotted_features: points.len(),
        points,
    }
}

fn effect_visualization(
    modality: &str,
    family: &str,
    tests: &[AdjustedFeatureResult],
) -> EffectVisualizationSummary {
    let mut rows = tests
        .iter()
        .filter(|row| {
            row.beta_group.is_finite()
                && row.p_value.is_finite()
                && row.q_value.is_finite()
                && row.q_value >= 0.0
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        a.q_value
            .total_cmp(&b.q_value)
            .then_with(|| a.p_value.total_cmp(&b.p_value))
            .then_with(|| b.beta_group.abs().total_cmp(&a.beta_group.abs()))
    });
    EffectVisualizationSummary {
        modality: modality.to_string(),
        family: family.to_string(),
        total_tests: tests.len(),
        points: rows
            .into_iter()
            .take(MAX_VIS_EFFECT_POINTS)
            .map(feature_visualization_point)
            .collect(),
    }
}

fn feature_visualization_point(row: &AdjustedFeatureResult) -> FeatureVisualizationPoint {
    FeatureVisualizationPoint {
        feature: row.feature.clone(),
        secondary_id: row.secondary_id.clone(),
        effect: row.beta_group,
        p_value: row.p_value,
        q_value: row.q_value,
        mean_case: row.mean_case,
        mean_control: row.mean_control,
    }
}

fn ordination_visualization(
    modality: &str,
    ord: &OrdinationResult,
) -> OrdinationVisualizationSummary {
    OrdinationVisualizationSummary {
        modality: modality.to_string(),
        title: ord.name.clone(),
        x_label: ord.x_label.clone(),
        y_label: ord.y_label.clone(),
        permanova_f: ord.permanova_f,
        permanova_p: ord.permanova_p,
        permutations: ord.permutations,
        points: ord
            .points
            .iter()
            .map(|point| OrdinationVisualizationPoint {
                sample_id: point.sample_id.clone(),
                group: point.group.clone(),
                subject: point.subject.clone(),
                x: point.x,
                y: point.y,
            })
            .collect(),
    }
}

fn dendrogram_visualization(
    modality: &str,
    tree: &DendrogramResult,
) -> DendrogramVisualizationSummary {
    DendrogramVisualizationSummary {
        modality: modality.to_string(),
        title: tree.name.clone(),
        distance_label: tree.distance_label.clone(),
        leaves: tree
            .leaves
            .iter()
            .map(|leaf| DendrogramVisualizationLeaf {
                sample_id: leaf.sample_id.clone(),
                group: leaf.group.clone(),
                subject: leaf.subject.clone(),
                position: leaf.position,
            })
            .collect(),
        segments: tree
            .segments
            .iter()
            .map(|segment| DendrogramVisualizationSegment {
                x1: segment.x1,
                y1: segment.y1,
                x2: segment.x2,
                y2: segment.y2,
            })
            .collect(),
    }
}

fn push_repertoire_visualizations(
    out: &mut UnifiedVisualizationSummary,
    modality: &str,
    result: &CorrectedRepertoireResult,
) {
    out.effect_panels.push(effect_visualization(
        modality,
        "Adjusted repertoire metrics",
        &result.metric_tests,
    ));
    if let Some(ord) = result.morisita_horn.as_ref() {
        out.ordinations
            .push(ordination_visualization(modality, ord));
    }
    if let Some(ord) = result.jaccard.as_ref() {
        out.ordinations
            .push(ordination_visualization(modality, ord));
    }
    if let Some(tree) = result.dendrogram.as_ref() {
        out.dendrograms
            .push(dendrogram_visualization(modality, tree));
    }
    for composition in &result.compositions {
        out.effect_panels.push(effect_visualization(
            modality,
            &format!("{} CLR effects", composition.family),
            &composition.tests,
        ));
        if let Some(ord) = composition.ordination.as_ref() {
            out.ordinations
                .push(ordination_visualization(modality, ord));
        }
        if let Some(tree) = composition.dendrogram.as_ref() {
            out.dendrograms
                .push(dendrogram_visualization(modality, tree));
        }
    }
}

fn unified_modality_summary(
    modality: &str,
    result_items: Option<usize>,
    error: Option<&str>,
) -> UnifiedModalitySummary {
    match (result_items, error) {
        (Some(count), _) => UnifiedModalitySummary {
            modality: modality.to_string(),
            status: "complete".to_string(),
            result_items: count,
        },
        (None, Some(error)) => UnifiedModalitySummary {
            modality: modality.to_string(),
            status: format!("failed: {error}"),
            result_items: 0,
        },
        (None, None) => UnifiedModalitySummary {
            modality: modality.to_string(),
            status: "not available".to_string(),
            result_items: 0,
        },
    }
}

fn persist_unified_run_artifacts<F>(
    job_dir: &FsPath,
    run: &UnifiedCohortAnalysisRun,
    artifacts: &mut Vec<JobArtifactSummary>,
    warnings: &mut Vec<String>,
    artifact_budget: &mut u64,
    mut progress: F,
) -> Result<()>
where
    F: FnMut(usize, usize, &str),
{
    let scratch = job_dir.join("snapshot").join("scratch");
    std::fs::create_dir_all(&scratch)?;
    let temp = tempfile::Builder::new()
        .prefix("phase3e-export-")
        .tempdir_in(&scratch)
        .context("failed to create temporary unified export directory")?;

    progress(0, 1000, "Preparing unified report exports");
    for (report_index, (name, generator)) in [
        (
            "simplified_report.html",
            generate_unified_simplified_html_report as fn(&FsPath, &UnifiedCohortAnalysisRun) -> Result<()>,
        ),
        (
            "full_report.html",
            generate_unified_detailed_html_report as fn(&FsPath, &UnifiedCohortAnalysisRun) -> Result<()>,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let report_start = if report_index == 0 { 100 } else { 400 };
        progress(report_start, 1000, &format!("Generating {name}"));
        let path = temp.path().join(name);
        match generator(&path, run) {
            Ok(()) => match std::fs::metadata(&path) {
                Ok(metadata)
                    if metadata.len() <= MAX_JOB_ARTIFACT_BYTES
                        && metadata.len() <= *artifact_budget =>
                {
                    artifacts.push(persist_job_artifact_file_budgeted(
                        job_dir,
                        name,
                        "text/html; charset=utf-8",
                        &path,
                        artifact_budget,
                    )?);
                }
                Ok(metadata) => warnings.push(format!(
                    "{name} was generated ({} bytes) but exceeds the available retained-artifact budget or per-artifact limit and was not retained",
                    metadata.len()
                )),
                Err(error) => warnings.push(format!("Could not inspect generated {name}: {error}")),
            },
            Err(error) => warnings.push(format!("Could not generate {name}: {error:#}")),
        }
    }

    progress(700, 1000, "Generating native technical result tables");
    match write_unified_analysis_tables_with_progress(
        temp.path(),
        run,
        |done, total, message| {
            let denom = total.max(1);
            let scaled = 700 + done.min(denom) * 180 / denom;
            progress(scaled, 1000, message);
        },
    ) {
        Ok(table_root) => {
            let mut files = Vec::new();
            collect_regular_files(&table_root, &mut files)?;
            files.sort();
            let mut pending = Vec::new();
            for path in files {
                let relative = path.strip_prefix(&table_root).unwrap_or(&path);
                let artifact_name = format!(
                    "tables__{}",
                    relative
                        .to_string_lossy()
                        .replace('/', "__").replace('\\', "__")
                );
                let metadata = std::fs::metadata(&path)?;
                if metadata.len() > MAX_JOB_ARTIFACT_BYTES || metadata.len() > *artifact_budget {
                    warnings.push(format!(
                        "Result table {} exceeds the available retained-artifact budget or per-artifact limit and was not retained",
                        relative.display()
                    ));
                    continue;
                }
                *artifact_budget = artifact_budget.saturating_sub(metadata.len());
                pending.push(PendingArtifactFile {
                    source_path: path.clone(),
                    name: artifact_name,
                    media_type: artifact_media_type(&path).to_string(),
                    byte_size: metadata.len(),
                });
            }

            let retained_count = pending.len();
            let retained_bytes = pending
                .iter()
                .map(|item| item.byte_size)
                .sum::<u64>();
            progress(
                880,
                1000,
                &format!(
                    "Retaining {retained_count} native technical table artifact(s) ({})",
                    format_binary_bytes(retained_bytes)
                ),
            );
            let retained = persist_job_artifact_files_parallel(
                job_dir,
                pending,
                |done, total, name, byte_size| {
                    let denom = total.max(1);
                    let scaled = 880 + done.min(denom) * 115 / denom;
                    progress(
                        scaled,
                        1000,
                        &format!(
                            "Retaining native technical tables ({done}/{total}) · {name} ({})",
                            format_binary_bytes(byte_size)
                        ),
                    );
                },
            )?;
            artifacts.extend(retained);
        }
        Err(error) => warnings.push(format!(
            "Could not materialize the complete unified technical table set: {error:#}"
        )),
    }
    progress(1000, 1000, "Unified reports and technical tables retained");
    Ok(())
}

fn collect_regular_files(root: &FsPath, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_regular_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn artifact_media_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "json" => "application/json",
        "csv" => "text/csv; charset=utf-8",
        "tsv" => "text/tab-separated-values; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn corrected_gene_result_tsv(
    result: &crate::corrected::CorrectedExpressionResult,
) -> Result<Vec<u8>> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .from_writer(Vec::new());
    writer.write_record([
        "feature",
        "secondary_id",
        "n_samples",
        "n_subjects",
        "n_case",
        "n_control",
        "mean_case",
        "mean_control",
        "beta_group",
        "robust_se",
        "z_value",
        "p_value",
        "q_value",
        "subject_effect",
        "subject_p_value",
        "subject_q_value",
        "method",
    ])?;
    for item in &result.tests {
        writer.write_record(vec![
            item.feature.clone(),
            item.secondary_id.clone(),
            item.n_samples.to_string(),
            item.n_subjects.to_string(),
            item.n_case.to_string(),
            item.n_control.to_string(),
            item.mean_case.to_string(),
            item.mean_control.to_string(),
            item.beta_group.to_string(),
            item.robust_se.to_string(),
            item.z_value.to_string(),
            item.p_value.to_string(),
            item.q_value.to_string(),
            item.subject_effect.to_string(),
            item.subject_p_value.to_string(),
            item.subject_q_value.to_string(),
            item.method.clone(),
        ])?;
    }
    writer.flush()?;
    Ok(writer.into_inner().map_err(|error| error.into_error())?)
}

fn persist_job_artifact_budgeted(
    job_dir: &FsPath,
    name: &str,
    media_type: &str,
    bytes: &[u8],
    remaining_budget: &mut u64,
) -> Result<JobArtifactSummary> {
    let byte_size = u64::try_from(bytes.len()).context("artifact is too large")?;
    if byte_size > *remaining_budget {
        bail!(
            "job artifact '{}' would exceed the retained project-storage budget reserved when the frozen snapshot was created",
            name
        );
    }
    let artifact = persist_job_artifact(job_dir, name, media_type, bytes)?;
    *remaining_budget = remaining_budget.saturating_sub(byte_size);
    Ok(artifact)
}


#[derive(Debug)]
struct PendingArtifactFile {
    source_path: PathBuf,
    name: String,
    media_type: String,
    byte_size: u64,
}

#[derive(Debug)]
struct MovedArtifactFile {
    artifact_id: Uuid,
    data_path: PathBuf,
    record_path: PathBuf,
    name: String,
    media_type: String,
    byte_size: u64,
}

#[derive(Debug)]
enum ArtifactFinalizeEvent {
    Started(usize),
    Finished(usize, Result<JobArtifactSummary>),
}

/// Move a batch of already-materialized technical tables into opaque artifact
/// storage and finalize their SHA-256 metadata with bounded parallelism.
///
/// The expensive part of local Phase-3E table retention is reading the table
/// bytes back once for their integrity hashes.  Doing that strictly serially
/// makes the entire job wait behind every large table in sequence.  The files
/// already live on the same private filesystem, so their move into artifact
/// storage is metadata-only in the normal case.  We then use a small bounded
/// worker pool for checksum/finalization.  This preserves independent opaque
/// artifact IDs and per-artifact SHA-256 values without creating a second copy
/// of each table.
fn persist_job_artifact_files_parallel<F>(
    job_dir: &FsPath,
    pending: Vec<PendingArtifactFile>,
    mut progress: F,
) -> Result<Vec<JobArtifactSummary>>
where
    F: FnMut(usize, usize, &str, u64),
{
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    let artifacts_dir = job_dir.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;

    let mut moved = Vec::with_capacity(pending.len());
    for item in pending {
        let artifact_id = Uuid::new_v4();
        let data_path = artifacts_dir.join(format!("{artifact_id}.bin"));
        let record_path = artifacts_dir.join(format!("{artifact_id}.json"));
        if let Err(rename_error) = std::fs::rename(&item.source_path, &data_path) {
            std::fs::copy(&item.source_path, &data_path).with_context(|| {
                format!(
                    "failed to retain generated artifact '{}' after atomic move failed: {rename_error}",
                    item.name
                )
            })?;
            std::fs::remove_file(&item.source_path).with_context(|| {
                format!(
                    "failed to remove scratch artifact '{}' after fallback copy",
                    item.name
                )
            })?;
        }
        set_private_file_permissions_sync(&data_path)?;
        moved.push(MovedArtifactFile {
            artifact_id,
            data_path,
            record_path,
            name: item.name,
            media_type: item.media_type,
            byte_size: item.byte_size,
        });
    }

    let total = moved.len();
    let available = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    let workers = total.min(available.min(4)).max(1);
    let mut work_order = (0..total).collect::<Vec<_>>();
    work_order.sort_by_key(|&index| std::cmp::Reverse(moved[index].byte_size));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::channel::<ArtifactFinalizeEvent>();
    let mut completed = 0usize;
    let mut summaries = vec![None; total];
    let mut active = std::collections::BTreeSet::new();

    std::thread::scope(|scope| -> Result<()> {
        for _ in 0..workers {
            let tx = tx.clone();
            let moved = &moved;
            let work_order = &work_order;
            let next = &next;
            scope.spawn(move || loop {
                use std::sync::atomic::Ordering;
                let work_position = next.fetch_add(1, Ordering::Relaxed);
                if work_position >= work_order.len() {
                    break;
                }
                let index = work_order[work_position];
                if tx.send(ArtifactFinalizeEvent::Started(index)).is_err() {
                    break;
                }
                let result = finalize_moved_artifact(&moved[index]);
                if tx
                    .send(ArtifactFinalizeEvent::Finished(index, result))
                    .is_err()
                {
                    break;
                }
            });
        }
        drop(tx);

        while completed < total {
            match rx
                .recv()
                .context("technical-artifact checksum worker stopped unexpectedly")?
            {
                ArtifactFinalizeEvent::Started(index) => {
                    active.insert(index);
                    let active_bytes = active
                        .iter()
                        .map(|&active_index| moved[active_index].byte_size)
                        .sum::<u64>();
                    progress(
                        completed,
                        total,
                        &format!(
                            "checksumming {} file(s), including {}",
                            active.len(),
                            moved[index].name
                        ),
                        active_bytes,
                    );
                }
                ArtifactFinalizeEvent::Finished(index, result) => {
                    active.remove(&index);
                    let summary = result?;
                    completed += 1;
                    progress(
                        completed,
                        total,
                        &format!("finalized {}", moved[index].name),
                        moved[index].byte_size,
                    );
                    summaries[index] = Some(summary);
                }
            }
        }
        Ok(())
    })?;

    summaries
        .into_iter()
        .enumerate()
        .map(|(index, summary)| {
            summary.ok_or_else(|| anyhow::anyhow!("technical artifact {index} was not finalized"))
        })
        .collect()
}

fn finalize_moved_artifact(item: &MovedArtifactFile) -> Result<JobArtifactSummary> {
    let sha256 = sha256_file_sync(&item.data_path)?;
    let record = StoredJobArtifactRecord {
        schema_version: 1,
        artifact_id: item.artifact_id,
        name: item.name.clone(),
        media_type: item.media_type.clone(),
        byte_size: item.byte_size,
        sha256: sha256.clone(),
    };
    std::fs::write(&item.record_path, serde_json::to_vec(&record)?)?;
    set_private_file_permissions_sync(&item.record_path)?;
    Ok(JobArtifactSummary {
        artifact_id: item.artifact_id.to_string(),
        name: record.name,
        media_type: record.media_type,
        byte_size: record.byte_size,
        sha256,
    })
}

fn format_binary_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.2} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.2} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Retain an already-materialized local artifact without reading the complete
/// file into memory and writing it a second time.  Phase 3E report/table
/// exporters materialize files under the job's private scratch directory, so
/// the source and destination live on the same filesystem and can normally be
/// atomically renamed into opaque artifact storage.  The content is hashed by
/// streaming from disk after the move, preserving the same SHA-256/integrity
/// semantics as byte-backed artifacts while avoiding a redundant full-file
/// copy for large HTML reports and technical TSVs.
fn persist_job_artifact_file_budgeted(
    job_dir: &FsPath,
    name: &str,
    media_type: &str,
    source_path: &FsPath,
    remaining_budget: &mut u64,
) -> Result<JobArtifactSummary> {
    let byte_size = std::fs::metadata(source_path)
        .with_context(|| format!("failed to inspect generated artifact '{}'", name))?
        .len();
    if byte_size > MAX_JOB_ARTIFACT_BYTES {
        bail!("job artifact exceeds the configured per-artifact safety limit");
    }
    if byte_size > *remaining_budget {
        bail!(
            "job artifact '{}' would exceed the retained project-storage budget reserved when the frozen snapshot was created",
            name
        );
    }

    let artifacts_dir = job_dir.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;
    let artifact_id = Uuid::new_v4();
    let data_path = artifacts_dir.join(format!("{artifact_id}.bin"));
    let record_path = artifacts_dir.join(format!("{artifact_id}.json"));

    if let Err(rename_error) = std::fs::rename(source_path, &data_path) {
        std::fs::copy(source_path, &data_path).with_context(|| {
            format!(
                "failed to retain generated artifact '{}' after atomic move failed: {rename_error}",
                name
            )
        })?;
        std::fs::remove_file(source_path).with_context(|| {
            format!(
                "failed to remove scratch artifact '{}' after fallback copy",
                name
            )
        })?;
    }
    set_private_file_permissions_sync(&data_path)?;
    let sha256 = sha256_file_sync(&data_path)?;
    let record = StoredJobArtifactRecord {
        schema_version: 1,
        artifact_id,
        name: name.to_string(),
        media_type: media_type.to_string(),
        byte_size,
        sha256: sha256.clone(),
    };
    std::fs::write(&record_path, serde_json::to_vec_pretty(&record)?)?;
    set_private_file_permissions_sync(&record_path)?;
    *remaining_budget = remaining_budget.saturating_sub(byte_size);

    Ok(JobArtifactSummary {
        artifact_id: artifact_id.to_string(),
        name: record.name,
        media_type: record.media_type,
        byte_size: record.byte_size,
        sha256,
    })
}

fn sha256_file_sync(path: &FsPath) -> Result<String> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to open artifact {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8 * 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed while hashing artifact {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn persist_job_artifact(
    job_dir: &FsPath,
    name: &str,
    media_type: &str,
    bytes: &[u8],
) -> Result<JobArtifactSummary> {
    let byte_size = u64::try_from(bytes.len()).context("artifact is too large")?;
    if byte_size > MAX_JOB_ARTIFACT_BYTES {
        bail!("job artifact exceeds the configured per-artifact safety limit");
    }
    let artifacts_dir = job_dir.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)?;
    let artifact_id = Uuid::new_v4();
    let data_path = artifacts_dir.join(format!("{artifact_id}.bin"));
    let record_path = artifacts_dir.join(format!("{artifact_id}.json"));
    std::fs::write(&data_path, bytes)?;
    set_private_file_permissions_sync(&data_path)?;
    let sha256 = hex::encode(Sha256::digest(bytes));
    let record = StoredJobArtifactRecord {
        schema_version: 1,
        artifact_id,
        name: name.to_string(),
        media_type: media_type.to_string(),
        byte_size,
        sha256: sha256.clone(),
    };
    std::fs::write(&record_path, serde_json::to_vec_pretty(&record)?)?;
    set_private_file_permissions_sync(&record_path)?;
    Ok(JobArtifactSummary {
        artifact_id: artifact_id.to_string(),
        name: record.name,
        media_type: record.media_type,
        byte_size: record.byte_size,
        sha256,
    })
}

#[cfg(unix)]
fn set_private_file_permissions_sync(path: &FsPath) -> Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions_sync(_path: &FsPath) -> Result<()> {
    Ok(())
}

fn mapping_status_name(status: MappingStatus) -> &'static str {
    match status {
        MappingStatus::Matched => "matched",
        MappingStatus::Unmatched => "unmatched",
        MappingStatus::Ambiguous => "ambiguous",
        MappingStatus::MetadataOnly => "metadata_only",
    }
}

#[derive(Clone)]
struct AppState {
    store: Arc<LocalProjectStore>,
    default_ttl_hours: u64,
    quota_profile: QuotaProfile,
    max_file_bytes: u64,
    max_project_bytes: u64,
    max_objects: usize,
    max_jobs_per_project: usize,
    max_concurrent_jobs: usize,
    max_concurrent_uploads: usize,
    worker_slots: Arc<Semaphore>,
    upload_slots: Arc<Semaphore>,
}

pub async fn run_local_web_server(config: WebServerConfig) -> Result<()> {
    ensure_web_dist_exists(&config.web_dist).await?;
    let store = Arc::new(
        LocalProjectStore::new(config.storage_root.clone(), config.default_ttl_hours).await?,
    );
    let removed = store.cleanup_expired().await?;
    if removed > 0 {
        eprintln!("4C cleanup removed {removed} expired local project(s)");
    }
    let scratch_removed = store.cleanup_abandoned_uploads(0).await?;
    if scratch_removed > 0 {
        eprintln!("4C cleanup removed {scratch_removed} abandoned upload scratch file(s)");
    }
    let interrupted = store.recover_interrupted_jobs().await?;
    if interrupted > 0 {
        eprintln!("4C marked {interrupted} interrupted local job(s) as failed after restart");
    }

    spawn_cleanup_loop(store.clone());

    let state = AppState {
        store,
        default_ttl_hours: config.default_ttl_hours,
        quota_profile: config.quota_profile,
        max_file_bytes: config.max_file_bytes,
        max_project_bytes: config.max_project_bytes,
        max_objects: config.max_objects,
        max_jobs_per_project: config.max_jobs_per_project,
        max_concurrent_jobs: config.max_concurrent_jobs,
        max_concurrent_uploads: config.max_concurrent_uploads,
        worker_slots: Arc::new(Semaphore::new(config.max_concurrent_jobs)),
        upload_slots: Arc::new(Semaphore::new(config.max_concurrent_uploads)),
    };
    let static_files =
        ServeDir::new(config.web_dist.clone()).append_index_html_on_directories(true);
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/projects", post(create_project))
        .route(
            "/api/projects/{project_id}",
            get(get_project).delete(delete_project),
        )
        .route(
            "/api/projects/{project_id}/objects",
            get(list_objects).post(upload_object),
        )
        .route(
            "/api/projects/{project_id}/objects/auto-assign",
            post(auto_assign_objects),
        )
        .route(
            "/api/projects/{project_id}/objects/{object_id}",
            axum::routing::delete(delete_object),
        )
        .route(
            "/api/projects/{project_id}/objects/{object_id}/assignment",
            axum::routing::put(assign_object),
        )
        .route(
            "/api/projects/{project_id}/metadata-profile",
            get(get_metadata_profile),
        )
        .route(
            "/api/projects/{project_id}/mapping-audit",
            post(run_mapping_audit),
        )
        .route(
            "/api/projects/{project_id}/jobs",
            get(list_jobs).post(submit_job),
        )
        .route(
            "/api/projects/{project_id}/jobs/{job_id}",
            get(get_job).delete(cancel_job),
        )
        .route(
            "/api/projects/{project_id}/jobs/{job_id}/stored",
            axum::routing::delete(delete_stored_job),
        )
        .route(
            "/api/projects/{project_id}/jobs/{job_id}/artifacts.zip",
            get(download_all_job_artifacts),
        )
        .route(
            "/api/projects/{project_id}/jobs/{job_id}/artifacts/{artifact_id}",
            get(download_job_artifact),
        )
        .fallback_service(static_files)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("failed to bind {}", config.bind))?;
    println!("4C Cohort Explorer web server: http://{}", config.bind);
    println!("API root: http://{}/api", config.bind);
    println!("Storage root: {}", config.storage_root.display());
    println!("Web assets: {}", config.web_dist.display());
    println!(
        "Default project retention: {} hours",
        config.default_ttl_hours
    );
    println!("Quota profile: {}", config.quota_profile.as_str());
    println!("Upload limit per file: {} bytes", config.max_file_bytes);
    println!(
        "Project storage quota: {} bytes across at most {} objects",
        config.max_project_bytes, config.max_objects
    );
    println!(
        "Persistent job limit: {} jobs per project",
        config.max_jobs_per_project
    );
    println!(
        "Concurrent local scientific jobs: {}",
        config.max_concurrent_jobs
    );
    println!(
        "Concurrent local uploads: {}",
        config.max_concurrent_uploads
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("4C web server stopped with an error")?;
    Ok(())
}

async fn ensure_web_dist_exists(web_dist: &FsPath) -> Result<()> {
    let index = web_dist.join("index.html");
    if fs::metadata(&index).await.is_err() {
        bail!(
            "web frontend not built: {} is missing. Run `cd web && trunk build --release` first, or set CFS_WEB_DIST",
            index.display()
        );
    }
    Ok(())
}

fn spawn_cleanup_loop(store: Arc<LocalProjectStore>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECONDS));
        interval.tick().await;
        loop {
            interval.tick().await;
            match store.cleanup_expired().await {
                Ok(removed) if removed > 0 => {
                    eprintln!("4C cleanup removed {removed} expired local project(s)");
                }
                Ok(_) => {}
                Err(error) => eprintln!("4C cleanup warning: {error:#}"),
            }
            match store
                .cleanup_abandoned_uploads(UPLOAD_SCRATCH_MAX_AGE_SECONDS)
                .await
            {
                Ok(removed) if removed > 0 => {
                    eprintln!("4C cleanup removed {removed} abandoned upload scratch file(s)");
                }
                Ok(_) => {}
                Err(error) => eprintln!("4C scratch cleanup warning: {error:#}"),
            }
        }
    });
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        app_version: env!("CARGO_PKG_VERSION"),
        api_version: 9,
        default_ttl_hours: state.default_ttl_hours,
        max_ttl_hours: MAX_TTL_HOURS,
        upload_enabled: true,
        mapping_enabled: true,
        analysis_enabled: true,
        persistent_jobs: true,
        representative_gene_enabled: true,
        unified_analysis_enabled: true,
        metadata_profile_enabled: true,
        report_download_enabled: true,
        bulk_artifact_download_enabled: true,
        result_visualizations_enabled: true,
        job_deletion_enabled: true,
        batch_upload_enabled: true,
        storage_backend: "local-filesystem",
        quota_profile: state.quota_profile.as_str(),
        max_file_bytes: state.max_file_bytes,
        max_project_bytes: state.max_project_bytes,
        max_objects: state.max_objects,
        max_jobs_per_project: state.max_jobs_per_project,
        max_concurrent_jobs: state.max_concurrent_jobs,
        max_concurrent_uploads: state.max_concurrent_uploads,
        max_job_artifact_bytes: MAX_JOB_ARTIFACT_BYTES,
    })
}

async fn create_project(
    State(state): State<AppState>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, ApiError> {
    let (project, recovery_token) = state
        .store
        .create_project(request)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(CreateProjectResponse {
        project,
        recovery_token,
        recovery_token_warning: "Keep this recovery token private. The server stores only its SHA-256 hash and cannot recover the original token.",
    }))
}

async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ProjectSummary>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state.store.read_authorized(project_id, token).await {
        Ok(summary) => Ok(Json(summary)),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn delete_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<DeleteProjectResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state.store.delete_authorized(project_id, token).await {
        Ok(deleted) => Ok(Json(DeleteProjectResponse {
            project_id: project_id.to_string(),
            deleted,
        })),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn list_objects(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<ObjectSummary>>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state.store.list_objects_authorized(project_id, token).await {
        Ok(objects) => Ok(Json(objects)),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn upload_object(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<UploadObjectQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<UploadObjectResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    let content_length = headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let _upload_permit = state
        .upload_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "upload worker is unavailable",
            )
        })?;
    match state
        .store
        .upload_object_authorized(
            project_id,
            token,
            &query.filename,
            content_type,
            content_length,
            body,
            state.max_file_bytes,
            state.max_project_bytes,
            state.max_objects,
        )
        .await
    {
        Ok((object, project)) => Ok(Json(UploadObjectResponse { object, project })),
        Err(error) => Err(map_upload_error(error)),
    }
}

async fn delete_object(
    State(state): State<AppState>,
    Path((project_id, object_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<DeleteObjectResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let object_id = Uuid::parse_str(&object_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid object UUID"))?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .delete_object_authorized(project_id, token, object_id)
        .await
    {
        Ok((deleted, project)) => Ok(Json(DeleteObjectResponse {
            object_id: object_id.to_string(),
            deleted,
            project,
        })),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn assign_object(
    State(state): State<AppState>,
    Path((project_id, object_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(request): Json<AssignObjectRequest>,
) -> Result<Json<AssignObjectResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let object_id = Uuid::parse_str(&object_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid object UUID"))?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .assign_object_authorized(project_id, token, object_id, request)
        .await
    {
        Ok(object) => Ok(Json(AssignObjectResponse { object })),
        Err(error) => Err(map_input_error(error)),
    }
}

async fn auto_assign_objects(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AutoAssignObjectsResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .auto_assign_objects_authorized(project_id, token)
        .await
    {
        Ok(response) => Ok(Json(response)),
        Err(error) => Err(map_input_error(error)),
    }
}

async fn get_metadata_profile(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<MetadataProfileResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .metadata_profile_authorized(project_id, token)
        .await
    {
        Ok(response) => Ok(Json(response)),
        Err(error) => Err(map_input_error(error)),
    }
}

async fn run_mapping_audit(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<MappingAuditRequest>,
) -> Result<Json<MappingAuditResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .mapping_audit_authorized(project_id, token, request)
        .await
    {
        Ok(response) => Ok(Json(response)),
        Err(error) => Err(map_input_error(error)),
    }
}

async fn list_jobs(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Vec<JobSummaryResponse>>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state.store.list_jobs_authorized(project_id, token).await {
        Ok(jobs) => Ok(Json(jobs)),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn submit_job(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<SubmitJobRequest>,
) -> Result<Json<JobDetailResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .create_job_snapshot_authorized(
            project_id,
            token,
            request,
            state.max_jobs_per_project,
            state.max_project_bytes,
        )
        .await
    {
        Ok(job) => {
            let job_id = Uuid::parse_str(&job.summary.job_id)
                .map_err(|_| ApiError::internal("newly-created job UUID is invalid"))?;
            let worker = state.store.clone();
            let worker_slots = state.worker_slots.clone();
            tokio::spawn(async move {
                match worker_slots.acquire_owned().await {
                    Ok(_permit) => worker.run_job(project_id, job_id).await,
                    Err(_) => {
                        let _ = worker
                            .update_job_state(
                                project_id,
                                job_id,
                                JobStatus::Failed,
                                "Local worker queue stopped",
                                None,
                                Some("The local worker semaphore closed before this job could start.".to_string()),
                            )
                            .await;
                    }
                }
            });
            Ok(Json(job))
        }
        Err(error) => Err(map_job_error(error)),
    }
}

async fn get_job(
    State(state): State<AppState>,
    Path((project_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<JobDetailResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid job UUID"))?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .read_job_authorized(project_id, token, job_id)
        .await
    {
        Ok(job) => Ok(Json(job)),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn cancel_job(
    State(state): State<AppState>,
    Path((project_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<CancelJobResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid job UUID"))?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .cancel_job_authorized(project_id, token, job_id)
        .await
    {
        Ok(job) => Ok(Json(CancelJobResponse { job })),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn delete_stored_job(
    State(state): State<AppState>,
    Path((project_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<DeleteJobResponse>, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid job UUID"))?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .delete_job_authorized(project_id, token, job_id)
        .await
    {
        Ok((deleted, project)) => Ok(Json(DeleteJobResponse {
            job_id: job_id.to_string(),
            deleted,
            project,
        })),
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

async fn download_all_job_artifacts(
    State(state): State<AppState>,
    Path((project_id, job_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid job UUID"))?;
    let token = bearer_token(&headers)?;
    let records = state
        .store
        .list_job_artifact_records_authorized(project_id, token, job_id)
        .await
        .map_err(map_authorization_or_not_found)?;
    if records.is_empty() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "this job has no retained artifacts to download",
        ));
    }
    let uncompressed_bytes = records
        .iter()
        .try_fold(0u64, |total, record| total.checked_add(record.byte_size))
        .ok_or_else(|| ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "artifact byte count overflow"))?;
    if uncompressed_bytes > MAX_BULK_DOWNLOAD_UNCOMPRESSED_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "bulk artifact download is limited to {} uncompressed bytes per request; this job has {} bytes. Download the largest artifacts individually.",
                MAX_BULK_DOWNLOAD_UNCOMPRESSED_BYTES, uncompressed_bytes
            ),
        ));
    }

    let entries = records
        .into_iter()
        .map(|record| {
            let path = state
                .store
                .job_artifact_data_path(project_id, job_id, record.artifact_id);
            (record, path)
        })
        .collect::<Vec<_>>();
    let archive = tokio::task::spawn_blocking(move || build_job_artifact_zip(entries))
        .await
        .map_err(|error| ApiError::internal(format!("bulk-download worker failed: {error}")))?
        .map_err(ApiError::internal)?;

    let filename = format!("4c_job_{job_id}_artifacts.zip");
    let mut response = Response::new(Body::from(archive));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response.headers_mut().insert(CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

async fn download_job_artifact(
    State(state): State<AppState>,
    Path((project_id, job_id, artifact_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let project_id = parse_project_id(&project_id)?;
    let job_id = Uuid::parse_str(&job_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid job UUID"))?;
    let artifact_id = Uuid::parse_str(&artifact_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid artifact UUID"))?;
    let token = bearer_token(&headers)?;
    match state
        .store
        .read_job_artifact_authorized(project_id, token, job_id, artifact_id)
        .await
    {
        Ok((record, data)) => {
            let mut response = Response::new(Body::from(data));
            *response.status_mut() = StatusCode::OK;
            if let Ok(value) = HeaderValue::from_str(&record.media_type) {
                response.headers_mut().insert(CONTENT_TYPE, value);
            }
            let safe_name = sanitize_download_name(&record.name);
            if let Ok(value) =
                HeaderValue::from_str(&format!("attachment; filename=\"{safe_name}\""))
            {
                response.headers_mut().insert(CONTENT_DISPOSITION, value);
            }
            Ok(response)
        }
        Err(error) => Err(map_authorization_or_not_found(error)),
    }
}

fn build_job_artifact_zip(
    entries: Vec<(StoredJobArtifactRecord, PathBuf)>,
) -> Result<Vec<u8>> {
    let manifest = entries
        .iter()
        .map(|(record, _)| record.clone())
        .collect::<Vec<_>>();
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        zip.start_file("artifact_manifest.json", options)?;
        zip.write_all(&manifest_bytes)?;

        let mut used_names = HashSet::new();
        used_names.insert("artifact_manifest.json".to_string());
        for (record, path) in entries {
            let data = std::fs::read(&path).with_context(|| {
                format!(
                    "failed to read retained artifact '{}' ({})",
                    record.name,
                    record.artifact_id
                )
            })?;
            if data.len() as u64 != record.byte_size {
                bail!(
                    "artifact '{}' size changed while preparing the download archive",
                    record.name
                );
            }
            let mut name = sanitize_download_name(&record.name);
            if name.is_empty() {
                name = format!("artifact_{}", record.artifact_id);
            }
            if !used_names.insert(name.clone()) {
                let suffix = record.artifact_id.to_string();
                name = format!("{}__{}", name, &suffix[..8]);
                used_names.insert(name.clone());
            }
            let file_options = options.large_file(record.byte_size > u32::MAX as u64);
            zip.start_file(name, file_options)?;
            zip.write_all(&data)?;
        }
        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

fn sanitize_download_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .take(160)
        .collect::<String>()
}

fn parse_project_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid project UUID"))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get(AUTHORIZATION)
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "missing Authorization header"))?
        .to_str()
        .map_err(|_| ApiError::new(StatusCode::UNAUTHORIZED, "invalid Authorization header"))?;
    value
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "expected Bearer recovery token"))
}

fn map_authorization_or_not_found(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if message.contains("invalid recovery token") {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid recovery token")
    } else if message.contains("has expired") {
        ApiError::new(StatusCode::GONE, "project has expired and was removed")
    } else if message.contains("does not exist") {
        ApiError::new(StatusCode::NOT_FOUND, "project or object does not exist")
    } else {
        ApiError::internal(error)
    }
}

fn map_upload_error(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if message.contains("invalid recovery token") {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid recovery token")
    } else if message.contains("has expired") {
        ApiError::new(StatusCode::GONE, "project has expired and was removed")
    } else if message.contains("does not exist") {
        ApiError::new(StatusCode::NOT_FOUND, "project or object does not exist")
    } else if message.contains("maximum file size")
        || message.contains("project storage quota")
        || message.contains("maximum object count")
    {
        ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, message)
    } else if message.contains("empty uploads") || message.contains("filename") {
        ApiError::new(StatusCode::BAD_REQUEST, message)
    } else {
        ApiError::internal(error)
    }
}

fn map_job_error(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if message.contains("invalid recovery token") {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid recovery token")
    } else if message.contains("has expired") {
        ApiError::new(StatusCode::GONE, "project has expired and was removed")
    } else if message.contains("maximum persistent job count") {
        ApiError::new(StatusCode::CONFLICT, message)
    } else if message.contains("stale")
        || message.contains("mapping snapshot")
        || message.contains("requires")
        || message.contains("comparison")
        || message.contains("primary metadata")
    {
        ApiError::new(StatusCode::BAD_REQUEST, message)
    } else {
        eprintln!("4C job submission error: {error:#}");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server job submission failed",
        )
    }
}

fn map_input_error(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if message.contains("invalid recovery token") {
        ApiError::new(StatusCode::UNAUTHORIZED, "invalid recovery token")
    } else if message.contains("has expired") {
        ApiError::new(StatusCode::GONE, "project has expired and was removed")
    } else if message.contains("does not exist") {
        ApiError::new(StatusCode::NOT_FOUND, "project or object does not exist")
    } else if message.contains("Metadata file")
        || message.contains("metadata workbook")
        || message.contains("metadata table")
        || message.contains("Could not open metadata")
        || message.contains("Could not read worksheet")
    {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "primary metadata could not be parsed; verify its assigned role, extension, and table/workbook content",
        )
    } else if message.contains("requires")
        || message.contains("assign exactly one")
        || message.contains("only one object")
        || message.contains("invalid")
        || message.contains("not accepted")
        || message.contains("not a readable")
        || message.contains("missing required")
        || message.contains("excessive")
        || message.contains("suspicious")
        || message.contains("could not be resolved")
        || message.contains("comparison")
    {
        ApiError::new(StatusCode::BAD_REQUEST, message)
    } else {
        eprintln!("4C input-processing error: {error:#}");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server input processing failed",
        )
    }
}

fn sanitize_display_name(value: Option<&str>) -> String {
    let trimmed = value.unwrap_or("Temporary 4C project").trim();
    let fallback = if trimmed.is_empty() {
        "Temporary 4C project"
    } else {
        trimmed
    };
    fallback.chars().take(120).collect()
}

fn sanitize_upload_filename(value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("upload filename is empty");
    }
    if trimmed.chars().count() > 240 {
        bail!("upload filename is too long");
    }
    if trimmed.chars().any(|ch| matches!(ch, '\0' | '\n' | '\r')) {
        bail!("upload filename contains invalid control characters");
    }
    let name = FsPath::new(trimmed)
        .file_name()
        .and_then(|part| part.to_str())
        .ok_or_else(|| anyhow!("upload filename is invalid"))?;
    if name == "." || name == ".." || name.is_empty() {
        bail!("upload filename is invalid");
    }
    Ok(name.to_string())
}

fn sanitize_content_type(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(200).collect())
}

fn generate_recovery_token() -> String {
    let mut bytes = [0u8; 32];
    let mut rng = OsRng;
    rng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn token_hash_hex(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn token_matches(token: &str, expected_hex: &str) -> Result<bool> {
    let expected = hex::decode(expected_hex).context("stored token hash is invalid")?;
    let actual = Sha256::digest(token.as_bytes());
    if expected.len() != actual.len() {
        return Ok(false);
    }
    Ok(actual.as_slice().ct_eq(expected.as_slice()).unwrap_u8() == 1)
}

async fn write_json_atomic<T: Serialize>(path: &FsPath, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)
        .await
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .await
        .with_context(|| format!("failed to replace {}", path.display()))?;
    set_private_file_permissions(path).await?;
    Ok(())
}

#[cfg(unix)]
async fn set_private_dir_permissions(path: &FsPath) -> Result<()> {
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .await
        .with_context(|| format!("failed to set private permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
async fn set_private_dir_permissions(_path: &FsPath) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
async fn set_private_file_permissions(path: &FsPath) -> Result<()> {
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .with_context(|| format!("failed to set private permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
async fn set_private_file_permissions(_path: &FsPath) -> Result<()> {
    Ok(())
}

fn unix_seconds() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("failed to install Ctrl-C handler: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn quota_profiles_have_expected_defaults() {
        assert_eq!(
            QuotaProfile::LocalSelfHosted.defaults(),
            (2 * 1024 * 1024 * 1024, 20 * 1024 * 1024 * 1024, 4_096,)
        );
        assert_eq!(
            QuotaProfile::HostedConservative.defaults(),
            (64 * 1024 * 1024, 512 * 1024 * 1024, 64)
        );
    }

    #[test]
    fn native_filename_auto_assignment_matches_shared_conventions() {
        let (role, sample_id) =
            infer_auto_assignment_from_filename("4C_39_tumor_rna_gene_expression_report.xlsx")
                .unwrap();
        assert_eq!(role, UploadedInputRole::GeneExpression);
        assert_eq!(sample_id.as_deref(), Some("4C_39"));

        let (role, sample_id) =
            infer_auto_assignment_from_filename("Normal_FJ_RNA_13_bcr_heavy_chain_report.xlsx")
                .unwrap();
        assert_eq!(role, UploadedInputRole::Bcr);
        assert_eq!(sample_id.as_deref(), Some("Normal_FJ_RNA_13"));

        assert!(infer_auto_assignment_from_filename("cohort_sample_metadata.xlsx").is_none());
        assert!(infer_auto_assignment_from_filename("h.all.v2026.1.Hs.symbols.gmt").is_none());
    }

    #[test]
    fn parallel_file_artifact_retention_preserves_hashes_and_names() {
        let temp = TempDir::new().unwrap();
        let job_dir = temp.path().join("job");
        let scratch = job_dir.join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();

        let mut pending = Vec::new();
        let mut expected = Vec::new();
        for (index, size) in [17usize, 8_193, 131_111, 1_048_777, 65_537]
            .into_iter()
            .enumerate()
        {
            let name = format!("tables__test_{index}.tsv");
            let path = scratch.join(format!("source_{index}.tsv"));
            let bytes = (0..size)
                .map(|offset| ((offset + index * 31) % 251) as u8)
                .collect::<Vec<_>>();
            std::fs::write(&path, &bytes).unwrap();
            expected.push((name.clone(), bytes));
            pending.push(PendingArtifactFile {
                source_path: path,
                name,
                media_type: "text/tab-separated-values; charset=utf-8".to_string(),
                byte_size: size as u64,
            });
        }

        let mut progress_events = 0usize;
        let retained = persist_job_artifact_files_parallel(
            &job_dir,
            pending,
            |_, _, _, _| progress_events += 1,
        )
        .unwrap();
        assert_eq!(retained.len(), expected.len());
        assert!(progress_events >= expected.len());

        for (summary, (expected_name, bytes)) in retained.iter().zip(expected.iter()) {
            assert_eq!(&summary.name, expected_name);
            assert_eq!(summary.byte_size, bytes.len() as u64);
            assert_eq!(summary.sha256, hex::encode(Sha256::digest(bytes)));
            let artifact_id = Uuid::parse_str(&summary.artifact_id).unwrap();
            let data_path = job_dir
                .join("artifacts")
                .join(format!("{artifact_id}.bin"));
            let record_path = job_dir
                .join("artifacts")
                .join(format!("{artifact_id}.json"));
            assert_eq!(
                std::fs::read(data_path).unwrap().as_slice(),
                bytes.as_slice()
            );
            let record: StoredJobArtifactRecord =
                serde_json::from_slice(&std::fs::read(record_path).unwrap()).unwrap();
            assert_eq!(record.name.as_str(), expected_name.as_str());
            assert_eq!(record.sha256, summary.sha256);
        }
    }

    #[test]
    fn recovery_tokens_are_random_and_hashable() {
        let first = generate_recovery_token();
        let second = generate_recovery_token();
        assert_ne!(first, second);
        assert!(first.len() >= 40);
        let hash = token_hash_hex(&first);
        assert!(token_matches(&first, &hash).unwrap());
        assert!(!token_matches(&second, &hash).unwrap());
    }

    #[tokio::test]
    async fn project_create_recover_delete_round_trip() {
        let temp = TempDir::new().unwrap();
        let store = LocalProjectStore::new(temp.path().to_path_buf(), 24)
            .await
            .unwrap();
        let (project, token) = store
            .create_project(CreateProjectRequest {
                display_name: Some("test cohort".to_string()),
                retention_hours: Some(2),
            })
            .await
            .unwrap();
        let project_id = Uuid::parse_str(&project.project_id).unwrap();
        let recovered = store.read_authorized(project_id, &token).await.unwrap();
        assert_eq!(recovered.display_name, "test cohort");
        assert!(store.delete_authorized(project_id, &token).await.unwrap());
        assert!(!store.project_dir(project_id).exists());
    }
    #[tokio::test]
    async fn upload_list_delete_round_trip_uses_opaque_storage_name() {
        let temp = TempDir::new().unwrap();
        let store = LocalProjectStore::new(temp.path().to_path_buf(), 24)
            .await
            .unwrap();
        let (project, token) = store
            .create_project(CreateProjectRequest {
                display_name: Some("upload test".to_string()),
                retention_hours: Some(2),
            })
            .await
            .unwrap();
        let project_id = Uuid::parse_str(&project.project_id).unwrap();
        let payload = b"sample_id\tgroup\nS1\tControl\n".to_vec();
        let (object, updated) = store
            .upload_object_authorized(
                project_id,
                &token,
                "metadata.tsv",
                Some("text/tab-separated-values".to_string()),
                Some(payload.len() as u64),
                Body::from(payload.clone()),
                1024 * 1024,
                4 * 1024 * 1024,
                8,
            )
            .await
            .unwrap();
        assert_eq!(object.original_name, "metadata.tsv");
        assert_eq!(object.byte_size, payload.len() as u64);
        assert_eq!(object.sha256, hex::encode(Sha256::digest(&payload)));
        assert_eq!(updated.uploaded_objects, 1);
        assert!(!store.inputs_dir(project_id).join("metadata.tsv").exists());
        let object_id = Uuid::parse_str(&object.object_id).unwrap();
        assert!(store.object_data_path(project_id, object_id).exists());

        let listed = store
            .list_objects_authorized(project_id, &token)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].object_id, object.object_id);

        let (deleted, updated) = store
            .delete_object_authorized(project_id, &token, object_id)
            .await
            .unwrap();
        assert!(deleted);
        assert_eq!(updated.uploaded_objects, 0);
        assert_eq!(updated.storage_bytes, 0);
        assert!(!store.object_data_path(project_id, object_id).exists());
    }

    #[tokio::test]
    async fn primary_metadata_assignment_runs_native_mapping_audit() {
        let temp = TempDir::new().unwrap();
        let store = LocalProjectStore::new(temp.path().to_path_buf(), 24)
            .await
            .unwrap();
        let (project, token) = store
            .create_project(CreateProjectRequest {
                display_name: Some("mapping test".to_string()),
                retention_hours: Some(2),
            })
            .await
            .unwrap();
        let project_id = Uuid::parse_str(&project.project_id).unwrap();
        let payload = b"sample_id\tanalysis_group\nS1\tControl\nS2\tCase\n".to_vec();
        let (object, _) = store
            .upload_object_authorized(
                project_id,
                &token,
                "metadata.tsv",
                Some("text/tab-separated-values".to_string()),
                Some(payload.len() as u64),
                Body::from(payload),
                1024 * 1024,
                4 * 1024 * 1024,
                8,
            )
            .await
            .unwrap();
        let object_id = Uuid::parse_str(&object.object_id).unwrap();
        let assigned = store
            .assign_object_authorized(
                project_id,
                &token,
                object_id,
                AssignObjectRequest {
                    role: UploadedInputRole::PrimaryMetadata,
                    sample_id: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(assigned.role, UploadedInputRole::PrimaryMetadata);
        let profile = store
            .metadata_profile_authorized(project_id, &token)
            .await
            .unwrap();
        assert_eq!(profile.metadata_rows, 2);
        assert_eq!(profile.metadata_id_column, "sample_id");
        assert_eq!(
            profile.suggested_comparison_column.as_deref(),
            Some("analysis_group")
        );
        let grouping = profile
            .columns
            .iter()
            .find(|column| column.name == "analysis_group")
            .unwrap();
        assert!(grouping.comparison_ready);
        assert_eq!(grouping.distinct_nonempty_values, 2);
        assert_eq!(
            grouping
                .values
                .iter()
                .map(|item| (item.value.as_str(), item.count))
                .collect::<Vec<_>>(),
            vec![("Case", 1), ("Control", 1)]
        );
        let audit = store
            .mapping_audit_authorized(
                project_id,
                &token,
                MappingAuditRequest {
                    comparison_column: Some("analysis_group".to_string()),
                    case_values: vec!["Case".to_string()],
                    control_values: vec!["Control".to_string()],
                },
            )
            .await
            .unwrap();
        assert_eq!(audit.metadata_rows, 2);
        assert_eq!(audit.canonical_samples, 2);
        assert_eq!(audit.case_samples, Some(1));
        assert_eq!(audit.control_samples, Some(1));
        assert_eq!(audit.metadata_only_samples, 2);
    }

    #[tokio::test]
    async fn frozen_job_snapshot_survives_source_object_deletion() {
        let temp = TempDir::new().unwrap();
        let store = Arc::new(
            LocalProjectStore::new(temp.path().to_path_buf(), 24)
                .await
                .unwrap(),
        );
        let (project, token) = store
            .create_project(CreateProjectRequest {
                display_name: Some("frozen job test".to_string()),
                retention_hours: Some(2),
            })
            .await
            .unwrap();
        let project_id = Uuid::parse_str(&project.project_id).unwrap();
        let payload = b"sample_id\tanalysis_group\nS1\tControl\nS2\tCase\n".to_vec();
        let (object, _) = store
            .upload_object_authorized(
                project_id,
                &token,
                "metadata.tsv",
                Some("text/tab-separated-values".to_string()),
                Some(payload.len() as u64),
                Body::from(payload),
                1024 * 1024,
                4 * 1024 * 1024,
                8,
            )
            .await
            .unwrap();
        let object_id = Uuid::parse_str(&object.object_id).unwrap();
        store
            .assign_object_authorized(
                project_id,
                &token,
                object_id,
                AssignObjectRequest {
                    role: UploadedInputRole::PrimaryMetadata,
                    sample_id: None,
                },
            )
            .await
            .unwrap();
        let mapping = store
            .mapping_audit_authorized(
                project_id,
                &token,
                MappingAuditRequest {
                    comparison_column: Some("analysis_group".to_string()),
                    case_values: vec!["Case".to_string()],
                    control_values: vec!["Control".to_string()],
                },
            )
            .await
            .unwrap();
        assert!(!mapping.mapping_snapshot_sha256.is_empty());
        let job = store
            .create_job_snapshot_authorized(
                project_id,
                &token,
                SubmitJobRequest {
                    mapping_snapshot_sha256: mapping.mapping_snapshot_sha256.clone(),
                    comparison_column: "analysis_group".to_string(),
                    case_values: vec!["Case".to_string()],
                    control_values: vec!["Control".to_string()],
                    subject_column: None,
                    covariates: Vec::new(),
                    run_qc: false,
                    run_gene_corrected: false,
                    run_full_unified: false,
                    excluded_sample_ids: Vec::new(),
                    run_exploratory_direction_check: false,
                    run_pbmc_diagnostics: false,
                    pbmc_batch_column: None,
                    pbmc_family_column: None,
                    pbmc_sensitivity_column: None,
                    pathway_q_threshold: default_pathway_q_threshold(),
                    pathway_effect_threshold: default_pathway_effect_threshold(),
                },
                16,
                512 * 1024 * 1024,
            )
            .await
            .unwrap();
        let job_id = Uuid::parse_str(&job.summary.job_id).unwrap();
        store
            .delete_object_authorized(project_id, &token, object_id)
            .await
            .unwrap();
        assert!(!store.object_data_path(project_id, object_id).exists());
        assert!(
            store
                .job_snapshot_root(project_id, job_id)
                .join("inputs")
                .join(format!("{object_id}.bin"))
                .exists()
        );
        store.clone().run_job(project_id, job_id).await;
        let completed = store
            .read_job_authorized(project_id, &token, job_id)
            .await
            .unwrap();
        assert_eq!(completed.summary.status, JobStatus::Complete);
        assert_eq!(completed.result.unwrap().mapping.metadata_rows, 2);

        let (deleted, project_after_delete) = store
            .delete_job_authorized(project_id, &token, job_id)
            .await
            .unwrap();
        assert!(deleted);
        assert_eq!(project_after_delete.job_count, 0);
        assert_eq!(project_after_delete.job_snapshot_bytes, 0);
        assert_eq!(project_after_delete.job_artifact_bytes, 0);
        assert!(!store.job_dir(project_id, job_id).exists());
    }

    #[test]
    fn bulk_artifact_zip_contains_manifest_and_all_files() {
        use std::io::Read;

        let temp = TempDir::new().unwrap();
        let first_path = temp.path().join("first.bin");
        let second_path = temp.path().join("second.bin");
        std::fs::write(&first_path, b"alpha").unwrap();
        std::fs::write(&second_path, b"beta").unwrap();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let bytes = build_job_artifact_zip(vec![
            (
                StoredJobArtifactRecord {
                    schema_version: 1,
                    artifact_id: first_id,
                    name: "report.html".to_string(),
                    media_type: "text/html".to_string(),
                    byte_size: 5,
                    sha256: "a".repeat(64),
                },
                first_path,
            ),
            (
                StoredJobArtifactRecord {
                    schema_version: 1,
                    artifact_id: second_id,
                    name: "table.tsv".to_string(),
                    media_type: "text/tab-separated-values".to_string(),
                    byte_size: 4,
                    sha256: "b".repeat(64),
                },
                second_path,
            ),
        ])
        .unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 3);
        let mut report = String::new();
        archive
            .by_name("report.html")
            .unwrap()
            .read_to_string(&mut report)
            .unwrap();
        assert_eq!(report, "alpha");
        let mut table = String::new();
        archive
            .by_name("table.tsv")
            .unwrap()
            .read_to_string(&mut table)
            .unwrap();
        assert_eq!(table, "beta");
        let mut manifest = String::new();
        archive
            .by_name("artifact_manifest.json")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        assert!(manifest.contains("report.html"));
        assert!(manifest.contains("table.tsv"));
    }

    #[tokio::test]
    async fn over_limit_upload_is_rejected_without_persisted_object() {
        let temp = TempDir::new().unwrap();
        let store = LocalProjectStore::new(temp.path().to_path_buf(), 24)
            .await
            .unwrap();
        let (project, token) = store
            .create_project(CreateProjectRequest {
                display_name: Some("quota test".to_string()),
                retention_hours: Some(2),
            })
            .await
            .unwrap();
        let project_id = Uuid::parse_str(&project.project_id).unwrap();
        let payload = vec![7u8; 128];
        let error = store
            .upload_object_authorized(
                project_id,
                &token,
                "too-large.bin",
                Some("application/octet-stream".to_string()),
                None,
                Body::from(payload),
                64,
                1024,
                8,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("maximum file size"));
        assert!(
            store
                .list_objects_authorized(project_id, &token)
                .await
                .unwrap()
                .is_empty()
        );
        let mut scratch = fs::read_dir(store.scratch_dir(project_id)).await.unwrap();
        assert!(scratch.next_entry().await.unwrap().is_none());
    }
}
