//! Browser-side transport-shaped state for the hosted 4C client.
//!
//! Phase 3D.2 connects private project lifecycle, bounded authenticated batch
//! upload, native-like automatic role/sample assignment, mapping, and frozen job
//! operations to `cfs-server`. Native QC and one representative corrected gene
//! lane are enabled while the rest of the scientific stack remains disabled.

#[derive(Debug, Clone)]
pub struct RemoteProjectSummary {
    pub project_id: Option<String>,
    pub display_name: String,
    pub uploaded_objects: usize,
    pub storage_bytes: u64,
    pub job_count: usize,
    pub job_snapshot_bytes: u64,
    pub job_artifact_bytes: u64,
    pub created_unix_seconds: Option<u64>,
    pub expires_unix_seconds: Option<u64>,
}

impl Default for RemoteProjectSummary {
    fn default() -> Self {
        Self {
            project_id: None,
            display_name: "No remote project loaded".to_string(),
            uploaded_objects: 0,
            storage_bytes: 0,
            job_count: 0,
            job_snapshot_bytes: 0,
            job_artifact_bytes: 0,
            created_unix_seconds: None,
            expires_unix_seconds: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RemoteBackendCapabilities {
    pub project_lifecycle: bool,
    pub upload: bool,
    pub mapping: bool,
    pub analysis: bool,
    pub persistent_jobs: bool,
}

impl RemoteBackendCapabilities {
    pub const PHASE3D: Self = Self {
        project_lifecycle: true,
        upload: true,
        mapping: true,
        analysis: true,
        persistent_jobs: true,
    };
}
