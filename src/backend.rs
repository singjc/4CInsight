//! Execution-backend boundary for the cohort-analysis UI.
//!
//! The desktop application currently uses [`LocalBackend`], which delegates to
//! the existing filesystem and scientific-engine functions without changing
//! their behavior.  [`RemoteBackend`] is intentionally a typed stub in this
//! phase: it defines the seam that the future WASM client / HTTP service will
//! use without introducing networking into the native application yet.

use crate::analysis::{
    CohortAnalysisSettingsSnapshot, CohortDefinitionSnapshot, UnifiedCohortAnalysisRun,
    run_unified_cohort_analysis,
};
use crate::cohort::{
    CohortComparison, CohortMapping, CohortModality, MetadataTable, ProjectScan,
    build_mapping as build_local_mapping, load_metadata as load_local_metadata,
    scan_project as scan_local_project,
};
use crate::outliers::{
    CrossModalOutlierResult, OutlierConfig, OutlierFilterSnapshot, run_cross_modal_outlier_qc,
};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// The execution environment selected by the application shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Direct filesystem access and in-process Rust analysis.
    Local,
    /// Future HTTP/job-service execution path.
    Remote,
}

/// Backend-neutral reference to a cohort project.
///
/// The native GUI currently creates `LocalDirectory` values.  A future WASM
/// client will receive a `RemoteProject` handle from `cfs-server` instead of a
/// browser-inaccessible local path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectLocator {
    LocalDirectory(PathBuf),
    RemoteProject {
        project_id: String,
        display_name: String,
    },
}

impl ProjectLocator {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::LocalDirectory(path.into())
    }

    pub fn local_path(&self) -> Option<&Path> {
        match self {
            Self::LocalDirectory(path) => Some(path.as_path()),
            Self::RemoteProject { .. } => None,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::LocalDirectory(path) => path.display().to_string(),
            Self::RemoteProject { display_name, .. } => display_name.clone(),
        }
    }
}

/// Backend-neutral reference to metadata used by Cohort Analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataLocator {
    LocalFile(PathBuf),
    RemoteObject {
        object_id: String,
        display_name: String,
    },
}

impl MetadataLocator {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::LocalFile(path.into())
    }

    pub fn local_path(&self) -> Option<&Path> {
        match self {
            Self::LocalFile(path) => Some(path.as_path()),
            Self::RemoteObject { .. } => None,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::LocalFile(path) => path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string()),
            Self::RemoteObject { display_name, .. } => display_name.clone(),
        }
    }
}

/// Inputs required by the existing native outlier-QC engine.
#[derive(Debug, Clone)]
pub struct LocalOutlierRequest {
    pub scan: ProjectScan,
    pub comparison: CohortComparison,
    pub selected_modalities: Vec<CohortModality>,
    pub config: OutlierConfig,
}

/// Placeholder identifying a future server-side outlier-QC request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteOutlierRequest {
    pub project_id: String,
    pub analysis_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum OutlierAnalysisRequest {
    Local(LocalOutlierRequest),
    Remote(RemoteOutlierRequest),
}

/// Inputs required by one existing native unified cohort-analysis run.
#[derive(Debug, Clone)]
pub struct LocalUnifiedAnalysisRequest {
    pub scan: ProjectScan,
    pub metadata: MetadataTable,
    pub cohort: CohortDefinitionSnapshot,
    pub settings: CohortAnalysisSettingsSnapshot,
    pub qc_snapshot: Option<OutlierFilterSnapshot>,
}

/// Placeholder identifying a future queued server-side analysis request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUnifiedAnalysisRequest {
    pub project_id: String,
    pub metadata_object_id: String,
}

#[derive(Debug, Clone)]
pub enum UnifiedAnalysisRequest {
    Local(LocalUnifiedAnalysisRequest),
    Remote(RemoteUnifiedAnalysisRequest),
}

/// Execution boundary used by reusable Cohort Analysis UI orchestration.
///
/// Phase 1 deliberately keeps this trait synchronous because the native
/// backend already runs inside a worker thread.  The future remote backend can
/// translate these calls into job submission/status operations without
/// changing the UI-facing concepts.
pub trait AnalysisBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    fn display_name(&self) -> &str;

    fn scan_project(&self, source: &ProjectLocator) -> Result<ProjectScan>;

    fn load_metadata(&self, source: &MetadataLocator) -> Result<MetadataTable>;

    fn build_mapping(&self, scan: &ProjectScan, metadata: &MetadataTable) -> Result<CohortMapping>;

    fn run_outlier_qc(
        &self,
        request: OutlierAnalysisRequest,
        progress: &mut dyn FnMut(usize, usize, &str),
    ) -> Result<CrossModalOutlierResult>;

    fn run_unified_analysis(
        &self,
        request: UnifiedAnalysisRequest,
        progress: &mut dyn FnMut(usize, usize, &str),
    ) -> Result<UnifiedCohortAnalysisRun>;
}

/// Native backend: preserves the v0.15.4 direct-filesystem behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalBackend;

impl AnalysisBackend for LocalBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Local
    }

    fn display_name(&self) -> &str {
        "Local native"
    }

    fn scan_project(&self, source: &ProjectLocator) -> Result<ProjectScan> {
        let ProjectLocator::LocalDirectory(root) = source else {
            bail!("LocalBackend cannot scan a remote project handle");
        };
        scan_local_project(root)
    }

    fn load_metadata(&self, source: &MetadataLocator) -> Result<MetadataTable> {
        let MetadataLocator::LocalFile(path) = source else {
            bail!("LocalBackend cannot load a remote metadata object");
        };
        load_local_metadata(path)
    }

    fn build_mapping(&self, scan: &ProjectScan, metadata: &MetadataTable) -> Result<CohortMapping> {
        Ok(build_local_mapping(scan, metadata))
    }

    fn run_outlier_qc(
        &self,
        request: OutlierAnalysisRequest,
        progress: &mut dyn FnMut(usize, usize, &str),
    ) -> Result<CrossModalOutlierResult> {
        let OutlierAnalysisRequest::Local(request) = request else {
            bail!("LocalBackend cannot execute a remote outlier-QC request");
        };
        let LocalOutlierRequest {
            scan,
            comparison,
            selected_modalities,
            config,
        } = request;
        run_cross_modal_outlier_qc(
            scan.merged_gene_tsv.as_deref(),
            scan.merged_transcript_tsv.as_deref(),
            &scan.files,
            &comparison,
            &selected_modalities,
            &config,
            |done, total, message| progress(done, total, message),
        )
    }

    fn run_unified_analysis(
        &self,
        request: UnifiedAnalysisRequest,
        progress: &mut dyn FnMut(usize, usize, &str),
    ) -> Result<UnifiedCohortAnalysisRun> {
        let UnifiedAnalysisRequest::Local(request) = request else {
            bail!("LocalBackend cannot execute a remote unified-analysis request");
        };
        let LocalUnifiedAnalysisRequest {
            scan,
            metadata,
            cohort,
            settings,
            qc_snapshot,
        } = request;
        run_unified_cohort_analysis(
            &scan,
            &metadata,
            cohort,
            settings,
            qc_snapshot,
            |done, total, message| progress(done, total, message),
        )
    }
}

/// Future HTTP-backed implementation.
///
/// Phase 1 intentionally does not make network requests.  Keeping a concrete
/// stub in the library lets the WASM/server phases build against a stable
/// backend concept while making accidental use fail loudly.
#[derive(Debug, Clone)]
pub struct RemoteBackend {
    endpoint: String,
}

impl RemoteBackend {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn not_implemented<T>(&self, operation: &str) -> Result<T> {
        bail!(
            "RemoteBackend operation '{operation}' is reserved for the web/API phase; no HTTP client is implemented yet (endpoint: {})",
            self.endpoint
        )
    }
}

impl AnalysisBackend for RemoteBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Remote
    }

    fn display_name(&self) -> &str {
        "Remote (not connected)"
    }

    fn scan_project(&self, _source: &ProjectLocator) -> Result<ProjectScan> {
        self.not_implemented("scan_project")
    }

    fn load_metadata(&self, _source: &MetadataLocator) -> Result<MetadataTable> {
        self.not_implemented("load_metadata")
    }

    fn build_mapping(
        &self,
        _scan: &ProjectScan,
        _metadata: &MetadataTable,
    ) -> Result<CohortMapping> {
        self.not_implemented("build_mapping")
    }

    fn run_outlier_qc(
        &self,
        _request: OutlierAnalysisRequest,
        _progress: &mut dyn FnMut(usize, usize, &str),
    ) -> Result<CrossModalOutlierResult> {
        self.not_implemented("run_outlier_qc")
    }

    fn run_unified_analysis(
        &self,
        _request: UnifiedAnalysisRequest,
        _progress: &mut dyn FnMut(usize, usize, &str),
    ) -> Result<UnifiedCohortAnalysisRun> {
        self.not_implemented("run_unified_analysis")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cohort::MappingStatus;

    #[test]
    fn local_backend_build_mapping_matches_existing_mapping_semantics() {
        let backend = LocalBackend;
        let scan = ProjectScan {
            root: std::path::PathBuf::from("/tmp/project"),
            files: Vec::new(),
            merged_gene_tsv: None,
            merged_transcript_tsv: None,
            metadata_candidates: Vec::new(),
        };
        let metadata = MetadataTable {
            path: std::path::PathBuf::from("/tmp/metadata.tsv"),
            columns: vec!["sample_id".to_string()],
            rows: vec![vec!["S1".to_string()]],
            id_column: "sample_id".to_string(),
        };
        let mapping = backend.build_mapping(&scan, &metadata).unwrap();
        assert_eq!(mapping.rows.len(), 1);
        assert_eq!(mapping.rows[0].sample_id, "S1");
        assert_eq!(mapping.rows[0].mapping_status, MappingStatus::MetadataOnly);
        assert_eq!(mapping.metadata_only_samples, 1);
    }

    #[test]
    fn remote_backend_fails_loudly_until_http_phase_exists() {
        let backend = RemoteBackend::new("http://127.0.0.1:8080/api");
        let error = backend
            .scan_project(&ProjectLocator::RemoteProject {
                project_id: "job-1".to_string(),
                display_name: "test".to_string(),
            })
            .unwrap_err()
            .to_string();
        assert!(error.contains("no HTTP client is implemented yet"));
    }

    #[test]
    fn locators_do_not_confuse_remote_handles_with_local_paths() {
        let local = ProjectLocator::local("/tmp/example");
        assert_eq!(
            local.local_path(),
            Some(std::path::Path::new("/tmp/example"))
        );
        let remote = ProjectLocator::RemoteProject {
            project_id: "p-123".to_string(),
            display_name: "example".to_string(),
        };
        assert!(remote.local_path().is_none());

        let local_meta = MetadataLocator::local("/tmp/meta.tsv");
        assert_eq!(
            local_meta.local_path(),
            Some(std::path::Path::new("/tmp/meta.tsv"))
        );
        let remote_meta = MetadataLocator::RemoteObject {
            object_id: "o-123".to_string(),
            display_name: "meta.tsv".to_string(),
        };
        assert!(remote_meta.local_path().is_none());
    }
}
