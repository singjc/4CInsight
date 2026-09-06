# 4C Cohort Explorer

A Rust application for memory-bounded RNA expression merging plus metadata-driven multi-modal cohort exploration across gene/transcript expression, BCR, BCR isotypes, TCR alpha/beta, and immune-infiltrate data.

The project contains a command-line interface, an `egui` desktop interface, and a browser/WASM client served by a local Rust `cfs-server`. The native Rust application remains the scientific reference implementation. The browser path is a thin same-origin client over authenticated opaque projects/objects/jobs and now drives the existing native frozen unified cohort-analysis workflow rather than maintaining a separate web statistics engine.

## What v0.16.16 adds

- Adds **Phase 3E.8 shared-core repertoire working-set stabilization** after real-cohort timing proved that TCR-beta scientific substages finished in about a minute while the function then spent far longer silently destroying the raw parsed repertoire payload.
- Replaces three duplicate clonotype-keyed structures (`clonotype_aa`, `aa_to_nt`, `aa_read_counts`) with one compact clonotype map that retains exactly the downstream information required: read count, one representative nucleotide sequence, and whether a second distinct nucleotide sequence was observed. This preserves convergence/private/public/distance semantics while substantially reducing allocation count and teardown cost for very large TCR-beta cohorts.
- Makes repertoire cleanup an explicit measured stage and performs bounded parallel working-set destruction before emitting `corrected repertoire analysis complete`, so the browser cannot report a modality complete while Rust is still reclaiming millions of allocations.
- Publishes the primary-results checkpoint regardless of whether PBMC diagnostics are enabled; optional TPM/PBMC enrichment is described according to the actual requested layers.
- Keeps the native desktop and server on the same `run_corrected_repertoire()` implementation; this is a shared scientific-core performance fix, not a web-only alternative engine.

## What v0.16.15 adds

- Adds **Phase 3E.7 end-to-end transcript/runtime stabilization**. The primary corrected-result checkpoint now fires immediately after corrected modalities/pathway results, before optional gene/transcript TPM direction-concordance and PBMC/confounding enrichment, so a large supporting TPM scan can no longer block access to the main scientific result.
- Removes redundant whole-file row-count pre-scans from corrected gene/transcript modeling and exploratory TPM direction checks. Progress is derived from CSV byte position instead, eliminating an otherwise complete extra parse of potentially multi-GiB merged matrices.
- Speeds exact exploratory TPM direction tests without changing their statistics: rows are processed in bounded native batches (up to 8 workers), Mann-Whitney p-values and Cliff's delta share one rank pass, and detailed byte/row progress is forwarded to persisted job telemetry.
- Makes transcript DTU explicitly observable as separate gene-total and transcript-model passes while retaining the existing DTU method and result semantics.
- Fixes encrypted `.4ckey` saving in browsers that drop user activation across asynchronous WebCrypto work. Encryption is prepared first, then a second explicit **Save prepared .4ckey…** click invokes a browser save picker where supported or a normal trusted download in Firefox. Browser download-location prompting still follows the user's browser settings.
- Keeps `/api/health` at API v12 because no server contract/schema change is required; existing v0.16.14 jobs/credentials remain compatible.

## What v0.16.14 adds

- Adds **Phase 3E.6 end-to-end runtime telemetry**. Persisted jobs now record job start time, current stage, current-stage start time, last progress update, and completed stage durations so a long calculation can be diagnosed from the browser without inferring timing from a percentage label.
- Stabilizes corrected BCR/TCR execution for large cohorts: parsed repertoire payloads are moved into model order instead of cloned, matched-depth rarefaction/sample summaries use bounded deterministic native workers, independent composition/ordination tasks run concurrently, Morisita-Horn distance is reused instead of recomputed for clustering, and PERMANOVA reuses precomputed squared-distance pairs across permutations. Statistical definitions, deterministic seeds, BH/FDR, and final ordering are unchanged.
- Adds an optional **encrypted `.4ckey` project-access file**. UUID + recovery token are encrypted entirely in the browser using PBKDF2-HMAC-SHA-256 (310,000 iterations) and AES-256-GCM; the passphrase is never sent to `cfs-server`. Loading a key restores the credential fields after decrypting locally and verifies the saved same-origin binding before the user authenticates.
- Advances `/api/health` to API v12. Existing jobs remain readable; new timing fields are backward-compatible optional fields in stored job records.

## What v0.16.13 adds

- Adds **Phase 3E.5: primary-results checkpoint before optional sensitivity refits**. Once corrected gene/transcript, repertoire/isotype, immune, pathway, supporting direction checks, reviewed QC, and the initial candidate ranking are complete, the browser receives a persisted `primary_results_ready` result immediately instead of waiting for PBMC/confounding sensitivity refits.
- Keeps PBMC/confounding diagnostics in the same frozen unified job. When those diagnostics finish, the same job advances to `results_ready` and candidate robustness annotations are enriched; the local worker slot remains held through optional export finalization so a new CPU-heavy scientific job cannot contend with a previous job's report/table export on the same workstation.
- Prevents the expensive immune-composition-adjusted gene sensitivity model from looking like a frozen job: its internal normalization/model progress is now forwarded to the browser.
- Reuses already-computed positive-count median-ratio size factors when a sensitivity refit has the exact same model sample population/source metric, avoiding another full count-normalization pass.
- Parallelizes corrected gene/transcript per-feature NB2 model fitting in bounded native batches (up to 8 workers on native targets; WASM stays serial). Statistics, BH/FDR, feature filtering, and final deterministic rankings are unchanged.
- Advances `/api/health` to API v11. Existing `results_ready`/terminal jobs remain readable; the new intermediate API state serializes as `primary_results_ready`.

## What v0.16.12 adds

- Adds **Phase 3E.4: scientific-result readiness decoupled from export finalization**. The complete native unified result is persisted and shown before browser visualization enrichment, HTML reports, and exhaustive technical artifacts finish finalizing for that same frozen run.
- Adds interactive browser plots for corrected gene/transcript volcano views, repertoire/isotype/immune corrected-effect panels, native PCA/PCoA ordinations with PERMANOVA context, native hierarchical dendrograms, pathway enrichment, and modality result coverage. Full-resolution rows remain in the authenticated native tables/reports.
- Adds authenticated **Download all as ZIP** for a persisted job and keeps the recovery token in the bearer header rather than a download URL.
- Advances `/api/health` to API v10 with the persisted `results_ready` job state.

## What v0.16.10 adds

- Adds **Phase 3E.2: metadata-driven comparison configuration parity**. Once exactly one uploaded object is assigned as primary metadata, the authenticated server parses that private file with the existing native metadata loader and exposes only a typed metadata profile to the WASM client.
- Replaces free-text Case/Control entry with a metadata field dropdown plus **multi-select Case and Control value dropdowns** showing per-value row counts. Selecting a value for one side removes it from the other, preserving disjoint cohorts.
- Mirrors the native comparison-builder defaults: prefer `analysis_group` when it is usable, otherwise suggest the first low-cardinality categorical field; exact `Case` / `Control` values are preselected only when those labels actually exist in the chosen metadata column. No disease/control orientation is guessed from arbitrary labels.
- Makes the comparison semantics explicit: the selected metadata column and selected Case/Control value sets are included in the mapping fingerprint and then frozen into every submitted unified job. They therefore define the cohorts for corrected gene/transcript, repertoire, immune, pathway/diagnostic, robustness, candidate-prioritization, and report layers. Metadata rows selected for neither side remain outside that comparison.
- Automatically reloads metadata comparison choices when a project containing primary metadata is recovered or when an object is newly assigned as primary metadata. Changing the grouping field/value sets invalidates the prior mapping snapshot, so a job cannot silently run with stale cohort definitions.
- Adds authenticated `GET /api/projects/{project_id}/metadata-profile` and advances `/api/health` to API v8 with `metadata_profile_enabled=true`. The endpoint keeps filesystem paths private, validates/materializes the primary metadata in server scratch space, limits exposed distinct values per field, and does not change the native scientific engine.

## What v0.16.9 adds

- Starts **Phase 3E.1: full frozen unified native analysis jobs**. New browser job submissions reconstruct the frozen native metadata, mapping, comparison, modality selection, corrected model, reviewed QC exclusions, and optional diagnostic settings on `cfs-server`, then call the existing `run_unified_cohort_analysis()` implementation.
- Reuses the native corrected gene/transcript, BCR/TCR, BCR-isotype, immune, pathway, exploratory direction-check, PBMC/confounding, robustness, and candidate-prioritization layers. Modality failures remain fault-tolerant through the existing unified native run instead of becoming a second browser-specific implementation.
- Preserves QC as a **review workflow**: automatic cross-modal flags are shown to the user but are not silently excluded. Explicit reviewed sample exclusions are frozen into the job and preserved in the native `OutlierFilterSnapshot` provenance.
- Generates both `simplified_report.html` and `full_report.html` from the **same completed `UnifiedCohortAnalysisRun`**, and persists the native technical table export alongside the unified summary under authorization-protected opaque artifact UUIDs.
- Adds browser **authenticated artifact/report downloads**. The recovery token remains in the bearer header used by `fetch`; the browser converts the authorized response into a temporary Blob URL, so capability tokens are never put into report URLs.
- Adds terminal persisted-job deletion through a dedicated authenticated endpoint. Deleting one completed/failed/cancelled job removes its frozen snapshot, artifacts, and scratch data without deleting the project or uploaded source objects, then reconciles retained-storage accounting.
- Adds an explicit singleton `pathway_gmt` role. Folder ingest still skips GMT files by default so auxiliary gene-set libraries are never silently promoted; users deliberately add/assign the GMT used by a frozen pathway run.
- Improves persisted-job UX: **Open results** now displays the selected job directly below the job history, and terminal polling refreshes project job/snapshot/artifact accounting as well as the job list.
- Advances `/api/health` to API v7 with explicit unified-analysis, report-download, and stored-job-deletion capabilities. Existing v0.16.8/Phase-3D job records remain readable; only newly submitted browser jobs request the full unified lane.

## What v0.16.8 adds

- Adds **Phase 3D.2 native-like high-throughput ingest** for cohorts with hundreds or thousands of files. The local browser no longer sends a large selected batch strictly one file at a time: it uses bounded concurrent uploads while preserving one authenticated request, opaque object UUID, streaming SHA-256, and independent quota check per file.
- Defaults to **6 concurrent uploads** for the local/self-hosted quota profile and **2** for the hosted/conservative profile. Override deliberately with `CFS_MAX_CONCURRENT_UPLOADS`. The scientific worker limit remains separate (`CFS_MAX_CONCURRENT_JOBS=1` by default).
- Refactors server upload locking so the global project mutation lock is held only for short authorization/quota preflight and atomic finalization. Request bodies stream/hash to private scratch storage outside that lock, allowing bounded uploads to make real progress in parallel. Final project quota/object checks are still serialized and authoritative.
- Avoids rebuilding the complete `manifests/inputs.json` after every file in a large upload batch. The logical manifest is refreshed once by assignment/mapping operations from the authoritative object records, removing an avoidable O(n²)-like cost for large cohorts.
- Adds a shared `src/input_conventions.rs` filename classifier used by native project scanning, the server, and WASM. Exact standard 4C filenames can seed the same modality/sample identity as the native desktop scanner instead of requiring hundreds of manual role selections.
- **Choose folder…** now mirrors the native project scan: recognized 4C workbooks/merged tables/metadata candidates are queued, while auxiliary files such as GMT libraries, logs, and unrelated files are skipped by default. **Choose files…** remains available for intentionally custom-named inputs.
- Standard sample workbooks are automatically assigned a role and sample ID only after the uploaded bytes pass the same defensive role validator used by manual assignment. Custom/ambiguous names remain `unassigned`; metadata candidates remain manual so the server never silently chooses which metadata table is primary.
- Adds **Auto-detect native roles & sample IDs** for projects uploaded under older versions. Existing v0.16.7 projects can therefore be upgraded in place without re-uploading hundreds of files.
- Large object lists now default to showing only files that still need attention, with a role summary and a toggle to inspect all uploaded objects. Failed concurrent uploads stay selected for one-click retry.
- Advances `/api/health` to API v6 and reports `max_concurrent_uploads`. Mapping, frozen-job, native QC, and representative corrected-gene semantics are unchanged.

### Native filename conventions used for automatic assignment

The standard suffixes currently shared with the native cohort scanner are:

```text
*_tumor_rna_gene_expression_report.xlsx
*_tumor_rna_transcript_expression_report.xlsx
*_bcr_heavy_chain_report.xlsx
*_bcr_isotype_composition.xlsx
*_tcr_alpha_clone_report.xlsx
*_tcr_beta_clone_report.xlsx
*_immune_infiltrate.xlsx
```

The prefix before the suffix becomes the sample ID. Explorer/Finder-style copies such as `...report (1).xlsx` are normalized before classification. Merged gene/transcript TSVs and native-style metadata-candidate names are recognized for folder filtering; metadata candidates are deliberately not auto-promoted to `primary_metadata`. Filenames are only classification/provenance hints and are never authorization credentials or server storage paths.

## What v0.16.7 adds

- Adds explicit **quota profiles** for browser/server storage instead of using the original Phase-3B smoke-test limits for every deployment. The default `local` / self-hosted profile is sized for real cohort work: **2 GiB per file, 20 GiB retained project storage, and 4,096 uploaded objects**.
- Retains a `hosted` / conservative profile with the previous **64 MiB per file, 512 MiB per project, and 64 objects** defaults for future constrained/public deployments. Select it with `CFS_QUOTA_PROFILE=hosted`.
- Keeps `CFS_MAX_FILE_BYTES`, `CFS_MAX_PROJECT_BYTES`, and `CFS_MAX_OBJECTS` as explicit overrides on top of either profile. `CFS_MAX_PROJECT_BYTES` still covers logical retained bytes: uploads + frozen job snapshots + persisted job artifacts.
- Exposes the active quota profile through `/api/health` (API v5), server startup logging, and the browser UI.
- Improves batch-upload preflight messages. The browser now shows projected object count and retained storage after the selected batch, and quota failures report current/projected usage rather than a generic object-count error.
- Keeps the Phase-3D scientific/job behavior unchanged: one local worker by default, frozen snapshots, native mapping/QC, and the representative corrected gene-expression lane.

### Local/self-hosted defaults

```text
CFS_QUOTA_PROFILE=local       # default
per file                      2 GiB
retained project storage     20 GiB
uploaded objects             4096
persisted jobs                 16
concurrent scientific jobs      1
```

The 20 GiB project budget deliberately includes frozen job snapshots. For example, ~350 MiB of uploaded expression files plus a ~350 MiB frozen job snapshot consumes roughly ~700 MiB of logical retained storage even if hard links reduce physical disk use.

For a deliberately constrained deployment:

```bash
CFS_QUOTA_PROFILE=hosted ./scripts/run_web_local.sh
```

All quota values remain environment-overridable when the backend has appropriate private storage capacity.

## What v0.16.6 adds

- Adds **Phase 3D frozen persistent analysis jobs**. A job can be submitted only from a current validated mapping snapshot; the server fingerprints the logical input manifest plus comparison and rejects submission if uploads or role assignments made that mapping stale.
- Freezes assigned input bytes into `jobs/<job-id>/snapshot/` before execution. On the local filesystem the server prefers hard links and falls back to copying, so later object deletion or assignment changes cannot silently mutate the already-submitted job.
- Adds an asynchronous persisted lifecycle: `queued → preparing → running → complete/failed/cancelled`. Browser polling shows progress, jobs can be reopened after browser refresh/project recovery, and server restart marks interrupted in-progress jobs as failed rather than silently resuming them.
- Reuses the **existing native cross-modal outlier/QC engine** from the frozen snapshot. There is no web-specific QC calculation.
- Adds one deliberately narrow representative corrected-analysis lane: when a `merged_gene_expression` TSV is assigned, the worker runs the existing native `run_corrected_expression()` path with `Raw Count`, `Gene`, `CorrectedModelSpec::default()`, plus the optional selected subject column/covariates. The browser shows the top corrected gene results and the server persists the complete result table as a private artifact.
- Persists mapping/QC/gene artifacts under opaque job artifact UUIDs and exposes an authenticated artifact-download endpoint. Full unified HTML reports and polished browser-side artifact download controls remain later work.
- Adds **multi-file upload selection** and **directory selection** in the browser. A selected batch is intentionally sent through the existing per-object upload API one file at a time: each file gets its own opaque UUID, streaming SHA-256, authorization check, and quota enforcement. Directory-relative paths are never used as server storage keys.
- Keeps the existing per-file/per-project/object quotas, adds a default limit of **16 persisted jobs per project** (`CFS_MAX_JOBS`), and defaults to **one concurrent local scientific worker** (`CFS_MAX_CONCURRENT_JOBS=1`) so queued browser jobs cannot unexpectedly saturate a laptop. Frozen job snapshots are counted as retained logical project storage, so submitting a new job is rejected when its frozen inputs would exceed `CFS_MAX_PROJECT_BYTES`. Individual persisted job artifacts are capped at **128 MiB**. Project TTL/deletion removes the jobs, snapshots, and artifacts with the rest of the private project directory.
- Advances the local API to **version 4** with `analysis_enabled=true`, `persistent_jobs=true`, `representative_gene_enabled=true`, and `batch_upload_enabled=true`. This does **not** mean the complete native multi-omics analysis stack is web-connected yet.
- Keeps local-filesystem storage behind opaque project/object/job identifiers so the later S3/MinIO storage implementation can preserve the same browser contract.

### Run and test Phase 3D

```bash
cd /home/singjc/Documents/github/cfs-expression-merger
./scripts/run_web_local.sh
```

Open `http://127.0.0.1:8080`. Create/recover a temporary project, use **Choose files…** for multi-select or **Choose folder…** for a directory, assign the uploaded objects, run a mapping audit, and then submit a frozen job from the current mapping fingerprint. Use synthetic/non-patient data while validating this development phase. See `docs/web_development.md` for the storage layout, API/job contract, parity tests, restart/cancellation semantics, and representative gene-lane requirements.

## What v0.16.5 adds

- Adds **Phase 3C uploaded-object interpretation and mapping audit**. Uploaded objects remain opaque UUID-backed blobs, but each object can now be assigned an explicit 4C role: primary metadata, filtered/subset metadata, gene expression, transcript expression, BCR, BCR isotype, TCR alpha, TCR beta, immune infiltrate, merged gene-expression TSV, or merged transcript-expression TSV.
- Sample-level modality roles require an explicit sample ID. The server does **not** infer modality or sample identity from filenames. Browser DTOs still expose object IDs/metadata only and never server filesystem paths.
- Persists role/sample-ID assignments both in each private object record and in `manifests/inputs.json`, preserving the same object-ID boundary needed for a future S3/MinIO storage backend.
- Adds defensive validation before scientific parsing. Web `.xlsx` inputs must be real XLSX/ZIP containers with required workbook entries and are bounded by ZIP entry-count, per-entry expanded-size, total expanded-size, and compression-ratio checks. Delimited text inputs are checked for UTF-8/NUL sanity. Legacy `.xls` is intentionally rejected by the Phase-3C web validator with a conversion message.
- Reuses the **existing native scientific mapping path** rather than implementing a browser-specific mapper: primary metadata is materialized in private scratch space and loaded with the existing `load_metadata()`, a native `ProjectScan` is constructed from explicit object roles/sample IDs, and the existing `build_mapping()` produces the audit.
- Adds authenticated `PUT /api/projects/{project_id}/objects/{object_id}/assignment` and `POST /api/projects/{project_id}/mapping-audit`. API health is now version 3 and reports `mapping_enabled=true` while `analysis_enabled=false`.
- Adds browser controls for assigning roles/sample IDs, running mapping validation, previewing optional Case/Control counts from a metadata-defined comparison, reviewing matched/unmatched/ambiguous/metadata-only counts, per-modality coverage, warnings, and sample-level mapping rows.
- Keeps corrected statistics, cross-modal QC, persistent analysis jobs, report generation, and S3/MinIO disabled until mapping parity is locally validated.

### Run the Phase-3C local web app

```bash
cd /home/singjc/Documents/github/cfs-expression-merger
./scripts/run_web_local.sh
```

Open `http://127.0.0.1:8080`, create/recover a project, upload harmless synthetic metadata/modality files, assign each object explicitly, and use **Cohort Analysis → Server-side mapping audit**. See `docs/web_development.md` for the role contract, API examples, validation limits, and native-parity checks.

## What v0.16.4 adds

- Adds **Phase 3B authenticated object upload** to the local web server. Every upload/list/delete request requires the project UUID plus the private recovery token established in Phase 3A.
- Upload request bodies are **streamed by Axum/Tokio to a private scratch file** instead of being accumulated into one server-memory buffer. SHA-256 is calculated incrementally over the same byte stream before the object is finalized.
- Stores each upload under an **opaque UUID object key** such as `inputs/<object-uuid>.bin`; the browser-supplied/original filename is preserved only as metadata and is never used as a filesystem path.
- Adds object metadata records containing the opaque object ID, original filename, byte size, SHA-256, upload timestamp, and optional content type.
- Adds configurable upload safety limits: per-file bytes, total project bytes, and object count. Phase 3B originally introduced conservative smoke-test defaults of **64 MiB per file**, **512 MiB per project**, and **64 objects**; v0.16.7 replaces those as the default local profile while retaining them as the optional hosted/conservative profile. The server rejects over-limit uploads and removes partial scratch files.
- Adds authenticated `GET /api/projects/{project_id}/objects`, `POST /api/projects/{project_id}/objects?filename=...`, and `DELETE /api/projects/{project_id}/objects/{object_id}` endpoints.
- Adds browser file selection/upload, uploaded-object listing with SHA-256/object IDs, and individual object deletion. Project deletion and TTL expiry continue to remove the complete project directory including all uploaded bytes.
- Keeps **scientific parsing, mapping/QC, corrected analysis, job execution, result/report persistence, and S3/MinIO disabled**. For Phase-3B validation, use harmless non-patient test files first.
- Keeps the storage/API contract S3-friendly: browser code only sees project/object identifiers and metadata, never server filesystem paths. The current implementation remains local-filesystem backed for laptop testing.

### Run the Phase-3B local web app

```bash
cd /home/singjc/Documents/github/cfs-expression-merger
./scripts/run_web_local.sh
```

Open `http://127.0.0.1:8080`, create/recover a private project, then use **Cohort Analysis → Private uploaded objects** to choose and upload a harmless test file. See `docs/web_development.md` for curl tests, quota configuration, storage layout, and deletion verification.


## What v0.16.3 adds

- Adds a **native/web visual-parity pass** for the browser client. The native and WASM frontends now share the same `src/ui_theme.rs` visual constants instead of configuring appearance independently.
- Applies the native dark palette, spacing, button sizing, accent color, hyperlink color, panel fills, and tooltip width to the WASM client.
- Rebuilds the browser header to match the native application: the same 4C title/subtitle card, accent-colored product name, and the same primary navigation ordering. Browser/server status is kept as a compact right-side status control rather than a separate first-class navigation tab.
- Uses the same dark page background in `web/index.html`, avoiding the flat gray/light bootstrap appearance seen in the first Phase-3A browser preview.
- Keeps all Phase-3A project/recovery-token/storage behavior unchanged. This release is presentation-only plus shared-theme refactoring; no scientific or server lifecycle semantics change.

## What v0.16.2 adds

- Adds **Phase 3A** of the hosted architecture: a native Rust **`cfs-server`** binary built with Axum. It binds to `127.0.0.1:8080` by default and serves both the compiled `web/dist/` WASM application and the same-origin `/api` endpoints, so Python or `trunk serve` is no longer required at runtime.
- Adds `GET /api/health`, `POST /api/projects`, `GET /api/projects/{project_id}`, and `DELETE /api/projects/{project_id}`. Upload and scientific-analysis endpoints remain deliberately disabled until the next phase.
- Adds passwordless **capability-based project recovery**: project creation returns an opaque UUID plus an independent random 256-bit recovery token. Only the token's SHA-256 hash is persisted server-side; patient-data/content hashes are not used as authorization secrets.
- Adds local temporary project persistence under a configurable storage root. The default is `~/.local/share/4c-cohort-explorer-web/`; each project receives isolated `inputs/`, `results/`, `reports/`, `manifests/`, and `scratch/` directories plus a small `project.json` lifecycle record.
- Adds privacy-oriented TTL handling. Projects default to 24 hours of retention (configurable up to 7 days in this development phase), expired projects are removed on server startup and by an hourly cleanup task, and explicit deletion removes the whole project directory.
- Connects the browser Cohort Analysis shell to `/api` using same-origin HTTP. Users can create, recover/refresh, and permanently delete a temporary project from the browser. Recovery credentials remain browser-memory-only in this phase and are not persisted to localStorage.
- Adds `scripts/run_web_local.sh`, which builds the WASM frontend with Trunk and launches the Rust server. This is now the recommended local browser workflow; `trunk serve` is not required.
- Keeps patient/sequencing file upload, mapping/QC, analysis jobs, result/report persistence, and S3/MinIO disabled until the storage lifecycle is validated independently.

### Run the Phase-3A local web app

One-time prerequisites:

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

Then from the repository root:

```bash
cd /home/singjc/Documents/github/cfs-expression-merger
./scripts/run_web_local.sh
```

Open:

```text
http://127.0.0.1:8080
```

The helper performs `trunk build --release` under `web/` and then runs `cargo run --release --bin cfs-server`. No Python HTTP server is required.

For separate build/server steps:

```bash
cd /home/singjc/Documents/github/cfs-expression-merger/web
trunk build --release

cd /home/singjc/Documents/github/cfs-expression-merger
cargo run --release --bin cfs-server
```

Useful local server configuration:

```bash
CFS_BIND=127.0.0.1:8080
CFS_STORAGE_ROOT=/path/to/private/4c-storage
CFS_WEB_DIST=/path/to/repo/web/dist
CFS_DEFAULT_TTL_HOURS=24
```

See `docs/web_development.md` for API tests, privacy/storage details, recovery-token handling, and the Phase-3B upload boundary.

## What v0.16.1 adds

- Adds a standalone **WASM/browser shell** under `web/` using `eframe::WebRunner`, while leaving the native scientific package unchanged and fully local.
- Reuses the canonical `src/analysis_docs.rs` directly from the browser crate, so the searchable analysis documentation and educational plots stay single-source across native GUI, HTML reports, and the web preview.
- Adds browser navigation for **Merge & Export**, **Explore & QC**, **Metadata Builder**, **Cohort Analysis**, and **Docs**. Native-data operations are intentionally disabled until the local Axum/job-service phase; the browser does not pretend it can open arbitrary server/local `PathBuf` values.
- Adds an in-memory Metadata Builder preview and a remote Cohort Analysis shell so browser editing/state interactions can be tested before uploads, job persistence, and storage are introduced.
- Keeps the planned remote API endpoint visible but display-only. There is **no upload, patient-data persistence, recovery-token, S3, or analysis HTTP implementation in this phase**.
- Adds `docs/web_development.md` and `web/README.md` with exact local build/run instructions using `wasm32-unknown-unknown` and Trunk.

### Historical Phase-2 note

v0.16.1 established the WASM browser shell. The current recommended local workflow is the v0.16.2 `cfs-server` flow above rather than `trunk serve`.

## What v0.16.0 adds

- Introduces a library-level **`AnalysisBackend` execution boundary** for the Cohort Analysis workflow. The current desktop application uses `LocalBackend`, which delegates to the existing filesystem, mapping, outlier-QC, and unified-analysis functions so the scientific behavior remains unchanged.
- Adds backend-neutral **project and metadata locator types** plus local/remote analysis request types. Native mode uses local directory/file locators today; future WASM/server work can use opaque remote project/object handles instead of pretending browser data are `PathBuf`s.
- Routes the user-facing unified Cohort Analysis project scan, metadata loading, mapping audit construction, cross-modal outlier QC, and unified analysis execution through the backend interface.
- Adds a concrete **`RemoteBackend` typed stub** that intentionally fails loudly because no HTTP client/server exists yet. This establishes the seam for the next WASM/API phases without adding networking or storage dependencies to the native application.
- Keeps the hidden legacy analysis paths available for regression comparison; this phase is an architectural boundary only and does not change the corrected statistics, QC algorithms, reports, metadata schema, or file formats.
- Displays the active execution backend in Cohort Analysis (`Local native` for the current desktop build).

## What v0.15.4 adds

- Keeps the **Docs search bar** and expands the educational plots with more realistic sample-level toy data, clearer category labeling, and dedicated statistics explainers for concepts such as **Aitchison/CLR** and **PERMANOVA**. Matching sections automatically open, so terms such as `clonality`, `q value`, `PCA`, `PERMANOVA`, `Aitchison`, `outlier`, `pathway`, or `covariate` jump directly to the relevant explanation.
- Keeps only the **Start here** section expanded by default; the larger method sections stay collapsed until opened or matched by search, reducing the initial wall of text.
- Provides **one or more synthetic worked examples + visuals** in every major analysis-guide section. Examples are explicitly labelled illustrative toy data and walk through what to notice before giving the interpretation takeaway.
- Uses the same example definitions in the GUI and both unified HTML report styles. The GUI renders lightweight `egui_plot` charts; reports render self-contained inline SVG plots inside the existing expandable documentation appendix. The toy examples now emphasize per-sample values and realistic multivariate/compositional interpretations rather than only coarse two-bar summaries. Dedicated explainers cover **Shannon/clonality, rarefaction, Morisita-Horn vs Jaccard, Aitchison/CLR, PERMANOVA, BH/FDR, hypergeometric ORA, PCA variance explained, and PCA loadings**.
- Keeps toy examples completely separate from cohort results: they are fixed educational values compiled into the documentation module and never enter the statistical run/result objects.

## What v0.15.2 adds

- Replaces the short GUI analysis notes with a **shared, detailed lay-person analysis reference** covering cohort definition, mapping, cross-modal QC, corrected gene/transcript models, DTU, BCR/TCR repertoire analysis, BCR isotype composition, immune ssGSEA, pathway ORA, PBMC/confounding diagnostics, candidate prioritization, ordinations, PERMANOVA, and statistical terminology.
- Explains the concrete values used by the Rust implementation rather than giving only generic definitions: expression count source/normalization/filtering, NB2 effect scale, repeated-subject handling, CLR pseudocount/filtering, rarefaction depth/repeats, repertoire-sharing definitions, outlier robust distance, pathway foreground/background, and robustness thresholds.
- Clarifies repertoire terminology in particular: **clonality is calculated once per sample across the complete clonotype abundance distribution**, then those per-sample clonality values are compared between Case and Control. It is not a per-V-gene or per-clone-type comparison.
- Adds a shared statistical glossary explaining effect/beta, adjusted log2 fold change, standard error, p value, BH q/FDR, dispersion, richness, Shannon diversity, clonality, CLR/Aitchison distance, Morisita-Horn, Jaccard, rarefaction, PCA/PCoA, and PERMANOVA.
- Embeds the **same canonical documentation** in both unified HTML report styles. Each report also records the exact settings used by that completed run before the expandable method/interpretation guide, so collaborators can understand both the method and the concrete thresholds that produced their results.

## What v0.15.1 adds

- Adds a dedicated **Metadata Builder** desktop tab for creating, curating, validating, and exporting a standardized one-row-per-sample multi-omics metadata TSV.
- Defines a 37-field standard schema covering canonical sample/subject identity, a recommended `analysis_group`, disease/cohort labels, longitudinal collection fields, specimen/source provenance, common covariates, technical batches, QC-review notes, and optional per-modality sample-ID aliases.
- Adds `templates/4C_multiomics_sample_metadata_template.tsv` plus `docs/sample_metadata_schema.md` as the repository-level metadata contract.
- Adds **custom metadata comparisons** to the unified Cohort Analysis workflow. Select any metadata column, assign one or more values to Case and Control, optionally provide presentation labels/name, then run the exact same corrected/statistical engine used by the built-in comparisons.
- Keeps the historical **HYP vs True_HC** and **HYP+ME/CFS vs ME/CFS-only** definitions as convenient built-in presets rather than hard-coded requirements.
- Adds canonical sample alias resolution for `gene_expression_id`, `transcript_expression_id`, `bcr_id`, `bcr_isotype_id`, `tcr_alpha_id`, `tcr_beta_id`, and `immune_infiltrate_id`. Modality aliases map back to one canonical `sample_id`, so they do not inflate case/control sample counts.
- Preserves custom/extra metadata columns and keeps them available for corrected-model covariates and ordination coloring.
- Records the exact built-in or custom comparison definition in the frozen unified run/full report. Rows not assigned to either custom group are simply outside that comparison.

## What v0.15.0 adds

- Replaces the user-facing Exploratory / Corrected / Focused / Diagnostics mode selector with **one coherent Cohort Analysis workflow**: cohort definition → sample mapping/QC → analysis settings → one **Run analysis** action → unified results/export.
- Adds a library-level `UnifiedCohortAnalysisRun` that freezes the cohort definition, mapping audit, corrected-model settings, reviewed cross-modal QC snapshot, corrected modality results, optional supporting descriptive checks, PBMC/confounding diagnostics, candidate prioritization, and warnings in one self-contained result.
- Keeps the existing **corrected / standard methods as the primary inferential engine**. The refactor orchestrates the existing corrected functions rather than introducing a second statistical implementation.
- Treats collaborator-supplied filtered metadata as an **optional cohort/subset definition**. It no longer implies a different analysis methodology.
- Moves cross-modal outlier QC into the unified sample-QC stage. Fresh automatic outlier calls are **flags only** and no longer preselect samples for exclusion; reviewed exclusions are explicitly applied in Analysis settings and frozen into the completed run.
- Integrates optional PBMC/source/composition diagnostics and candidate robustness labels into the same run. Focused-style candidate ranking is now a **presentation/prioritization layer derived from corrected results**.
- Adds two self-contained HTML report presets from the exact same completed run: **Simplified key-findings report** and **Full detailed report**. Choosing a report style never reruns statistics or changes sample inclusion.
- Adds a unified technical table export containing the frozen mapping/QC provenance, selected corrected modality results, pathways, diagnostics, key findings, warnings, and run summary.
- Keeps the legacy mode-specific implementation paths temporarily in source for parity/regression comparison; they are no longer the primary user-facing Cohort Analysis navigation and can be removed only after local v0.14.1 parity validation.

## What v0.14.1 adds

- Makes **QC filtering fully auditable in exports and HTML reports**. When reviewed outlier exclusions are applied to Corrected / standard or Focused candidate analyses, the app snapshots the exact outlier audit and reviewed exclusion list used for that run.
- Corrected and focused exports now include `sample_outlier_qc.csv`, `sample_outlier_qc_modalities.tsv`, and `sample_outlier_qc_summary.txt` with the detection scope, robust-distance threshold, cross-modal rule, consensus flags, manual overrides, per-modality coordinates/distances, and the exact sample IDs removed.
- Corrected and focused HTML reports now include a dedicated **Sample QC and reviewed exclusions** section with before/after case/control counts, excluded IDs, consensus-vs-manual decisions, per-modality QC summaries, and interactive outlier scatter plots.
- Exploratory exports/reports also include the current outlier audit when one has been run, but explicitly state that those reviewed exclusions were **not applied** to the exploratory result.
- Run-specific QC snapshots are preserved even if the user later changes the outlier settings, preventing a report from accidentally describing a different exclusion configuration than the one used to generate the statistics.

## What v0.14 adds

- Adds a **cross-modal sample outlier QC** section to the Exploratory workflow. Outlier detection is unsupervised and is based on sample-level representations for the currently enabled modalities rather than on the significance of individual biomarkers.
- Gene/transcript QC uses PCA of top-variable expression features; BCR/TCR use robust-standardized repertoire summary metrics; BCR isotype uses CLR composition; immune infiltrate uses robust-standardized ssGSEA signatures.
- Adds a configurable **within-group vs across-all** detection scope. Within case/control groups is the recommended default so a genuine disease-wide shift is less likely to be mistaken for a technical outlier.
- Adds configurable cross-modal rules: **any modality**, **at least N modalities**, **majority**, or **all available selected modalities**, with a minimum-modality requirement for consensus rules.
- Adds an editable sample audit table and per-modality PCA view. Automatic flags can be accepted, overridden, or cleared; the app does not treat an automatic flag as proof that a sample is biologically invalid.
- Adds optional application of the reviewed exclusion list to **Corrected / standard** and/or **Focused candidate** analyses. Exploratory results remain inclusive so filtered analyses can be treated as sensitivity checks rather than silently replacing the original cohort.
- Adds CSV export of the sample-level consensus flags and per-modality robust outlier scores.
- Adds unit tests for the cross-modal consensus rules and robust-distance detection.

## What v0.13 adds

- Adds an optional **sample-metadata filter directly to Merge & Export**. Load XLSX/CSV/TSV/TXT metadata without changing the selected workbook set.
- Lets the user choose the **metadata sample-ID column** used to match expression-report filenames, using the same punctuation-insensitive sample-ID normalization used by Cohort Analysis.
- The metadata table can act directly as an **inclusion list**: enable metadata filtering and every uniquely matched workbook represented by a metadata row is eligible. This is useful for collaborator-supplied pre-filtered metadata tables.
- Adds an optional **value-filter column** (for example `Phenotype`, `Healthy`, `HYP`, cohort, batch, or any other metadata field) plus checkboxes when an additional subset is desired.
- Filtering is **off by default**. When enabled, only uniquely matched workbooks represented in the metadata are merged; if an optional value-filter column is selected, the workbook must also have a checked value. Unmatched and ambiguous files are reported and excluded rather than aborting the merge.
- Expands the sample preview with **metadata match status, filter value, and include/exclude status**, and updates the output-column preview and merge button to reflect the filtered sample set.
- Metadata-only rows never manufacture missing expression files: the filter only selects among workbooks that actually exist.

## What v0.12 adds

- Adds a dedicated **PBMC / confounding diagnostics** analysis layer for investigating sample-source, composition, historical batch/family, low-count, and single-sample-driven expression signals before promoting them as disease-associated candidates.
- Adds **whole-transcriptome QC PCA** from top-variable `log2(TPM+1)` genes across the merged gene matrix, with robust sample-outlier scores and metadata-aware coloring.
- Adds per-sample **library/detection summaries** (total raw counts, detected genes, genes with counts ≥5/≥10) and integrates immune-infiltrate signatures when available.
- Adds an immune-signature PCA plus a descriptive **myeloid-high / lymphoid-low signature imbalance** score. These remain enrichment-score diagnostics rather than cell-percentage estimates.
- Adds user-editable lists for **known non-PBMC/wrong-source samples** and **suspect/unusual samples**. Suspect samples can be compared directly against ordinary samples without using disease status, making it easier to identify a source/processing expression program.
- Adds a **QC-restricted corrected gene sensitivity model** after sensitivity-only exclusions, plus an **immune-PC-adjusted corrected model** using the first two immune-signature PCs as covariates when enough immune profiles are available.
- Adds candidate-gene robustness diagnostics: raw-count and TPM medians, detection rates, count ≥5/≥10 fractions, top-two-sample contribution, leave-one-sample-out effect range, optional leave-batch/family/year-out effect range, a simple regularized-effect diagnostic, and comparison with the QC-restricted, immune-adjusted, and suspect-vs-ordinary effects.
- Adds a per-candidate **sample-level TPM plot** that highlights suspect or sensitivity-excluded samples, with raw-count/TPM/subject/batch/family information on hover.
- Adds **within-subject longitudinal QC** using maximum pairwise whole-transcriptome PCA distance across repeated samples.
- Adds one-click export of the sample QC audit, candidate robustness table, per-sample candidate values, longitudinal QC, suspect-vs-ordinary expression screen, and corrected sensitivity-model tables.

## What v0.11 adds

- Adds a dedicated **Docs** workspace to the desktop app, plus a **Docs** analysis layer inside Cohort Analysis. These pages explain the exploratory, corrected / standard, and focused workflows in plain language.
- Adds a lay-friendly **analysis guide** covering what each workflow is for, how to read the main plots, what conclusions are reasonable, and what important limitations to keep in mind.
- Adds simple **workflow schematics** in the app so non-technical collaborators can understand how the pieces fit together: merge & map → exploratory → corrected / standard → focused candidates.
- Adds a new repository document at **`docs/analysis_guide.md`** so the same explanation can be shared outside the app or quoted in methods/supporting notes.
- Adds **key literature links** for false-discovery-rate control, count-based expression modelling, compositional analysis, PERMANOVA, pathway analysis, Reactome, Hallmark/MSigDB, and ssGSEA.

## What v0.10 adds

- Adds a **built-in public GMT collection browser** to both the Corrected / standard and Focused candidate workflows. The catalog includes Reactome, Hallmark, GO Biological Process, ImmuneSigDB, HPO, Cell Type Signatures (C8), and WikiPathways from the human MSigDB v2026.1.Hs release.
- Adds **Download & use** for public collections. Downloads are cached under `<project>/gene_sets/msigdb_v2026.1.Hs/` and reused on later runs; selecting an already cached collection activates it immediately.
- Keeps **Choose custom GMT…** as a first-class option for local/private/custom pathway libraries.
- Shows collection-specific guidance in the GUI (for example, Reactome as the recommended mechanistic collection, Hallmark as the compact overview, and HPO as phenotype relevance rather than a literal pathway ontology), together with source/version/license links for provenance.
- Uses the operating system HTTPS client (`curl`, then `wget`, with PowerShell fallback on Windows) so the feature adds **no new Cargo dependency**. Downloaded files are validated as GMT before they are selected.
- Tightens the focused pathway shortlist so gene sets with **zero foreground overlap or p=1** cannot be promoted as candidate pathways.

## What v0.8 adds

- Adds **average-linkage hierarchical clustering / dendrograms** throughout the corrected layer: gene/transcript expression on top-variable normalized features, BCR/TCR on Morisita-Horn distance, V/D/J/V-J and isotype compositions on Aitchison distance, and immune infiltrate on standardized cell-type profiles.
- Adds a **PCA / PCoA / dendrogram color selector** in the app. Samples can be colored by analysis group, subject/patient, or any loaded metadata column to inspect phenotype, batch, sex, timepoint, or other structure.
- Fixes **pathway point hover in the desktop app** and keeps pathway hover concise in HTML reports. Full overlap-gene lists remain available in the result tables and exports.
- Improves **scatter responsiveness** by drawing ordinations/pathways as grouped point series and resolving hover labels from the nearest point. Large exploratory gene/transcript scatter plots use a bounded representative display while preserving every result in tables/exports.
- Expands **table viewports** and enables horizontal as well as vertical scrolling for large cohort/QC tables so wide or long results do not collapse to only a few visible rows.
- Adds modality-specific **analysis guides and interpretation notes** to both the corrected desktop results and corrected HTML reports. These explain what is being tested, what the unsupervised plots mean, and important cautions such as compositionality, sequencing depth, ssGSEA scale, and pathway redundancy.
- Removes implementation-oriented wording from the user-facing analysis descriptions so reports focus on methods, assumptions, interpretation, and limitations.
- Corrected result exports now include **dendrogram leaf-order TSVs** for reproducible sample-clustering review.

## What v0.6 adds

- Adds **optional transcript-expression analysis** to Cohort Analysis. The merged transcript table can be screened with TPM or Total Reads using the same exploratory Mann–Whitney/BH/Cliff's-delta framework as gene expression.
- Adds first-pass **TCR alpha and TCR beta comparison modules**. Each chain is analyzed independently for clone depth, richness, Shannon/Simpson diversity, clonality, effective clonotypes, top-clone fraction, weighted CDR3 amino-acid length, V-gene usage, J-gene usage, and V–J-pair usage.
- Adds an optional **immune-infiltrate module** for the per-sample ssGSEA workbooks, including per-cell-type group tests, effect plots, group-mean visualization, and exported sample values.
- Makes all newly enabled modalities **fault tolerant**: absent files, samples outside the comparison, duplicates, and malformed individual workbooks are reported without aborting the other selected analyses.
- Extends **Save result tables…** and the standalone HTML report to transcript, TCR alpha, TCR beta, and immune-infiltrate outputs.
- Reworks all report scatter plots to use explicit Plotly hover text. Hovering a point now reports the biological feature name (gene/transcript/V gene/J gene/V–J pair/BCR isotype/immune-cell type), secondary annotation where available, effect, Cliff's delta, p value, BH q value, group medians/means, sample counts, and detection fractions when applicable.
- Adds a dedicated BCR-isotype effect/significance scatter to the HTML report.

The TCR module in v0.6 is intentionally the first comparison layer. The existing Python workflow's matched-depth rarefaction, Morisita–Horn similarity/PCoA, public/private clonotypes, amino-acid convergence, and related repertoire-wide analyses remain candidates for the next advanced TCR iteration.

## What v0.5 adds

- Adds a **Generate HTML report…** action to Cohort Analysis using the `report-builder` Rust crate. The report combines the comparison definition, mapping audit, interactive plots, ranked tables, analysis caveats, and modality warnings into one HTML file.
- Adds a **gene detection-shift plot** comparing the fraction of case versus control samples in which each gene is detected.
- Adds selectable **case/control BCR distribution box plots** for Shannon diversity, clonality, unique clones, top-clone fraction, effective clonotypes, and weighted mean CDR3 AA length, including individual sample points.
- Adds a **group-mean BCR-isotype composition plot** alongside the existing effect/test table.
- Adds an automated report-generation unit test in addition to the existing cohort tests.

## What v0.4.1 adds

- Renames the desktop application to **4C Cohort Explorer**.
- Adds a one-click **Export mapping CSV…** action in the sample mapping & coverage audit so unmatched/ambiguous/metadata-only samples and cross-modality coverage can be sent back for metadata correction.
- Makes large cohort result tables substantially taller and reserves a fixed scrollable viewport so V/D/J-gene and differential-feature tables no longer collapse to only a few visible rows inside the outer page.
- Increases the on-screen sample mapping audit height for easier review.

## What v0.4 adds

v0.4 introduces a new **Cohort Analysis** workspace while preserving the existing Merge & Export and Explore & QC workflows.

- **Recursive project scan** detects gene expression, transcript expression, BCR heavy-chain, BCR isotype, TCR alpha, TCR beta, immune-infiltrate, and merged expression files.
- **Generic sample metadata loading** from XLSX/CSV/TSV/TXT. `4C_ID` is selected automatically when present and the ID column can be changed in the GUI.
- **Cross-modality mapping audit** reports matched, unmatched, ambiguous and duplicate sample/file combinations without aborting the analysis.
- **Built-in exploratory comparison presets** implement the current study questions: all HYP versus `True_HC`, and `HYP+ME/CFS` versus ME/CFS-only.
- **Gene-expression biomarker screen** uses the merged TPM matrix, Mann–Whitney U tests on `log2(TPM+1)`, Benjamini–Hochberg FDR, Cliff's delta, detection fractions and a simple log2 mean fold-change effect. Results are shown as an interactive volcano-style plot and ranked table.
- **BCR first-pass comparison** calculates total clone count, unique clones, Shannon/Simpson diversity, normalized clonality, effective clonotype count, top-clone fraction, weighted mean CDR3 amino-acid length, and heavy-chain isotype/V/D/J usage. Group tests use Mann–Whitney U, BH correction and Cliff's delta.
- **Dedicated BCR-isotype comparison** tests the `Isotype Read Fraction` values from the composition workbooks.
- **Fault-tolerant modality execution** means a missing/unsupported modality or a malformed individual BCR workbook is reported while other available analyses continue.
- TCR and immune-infiltrate files are detected and included in the sample coverage audit; v0.6 adds their first-pass group-comparison modules.

The v0.4 cohort tests are deliberately an **exploratory first pass**. They do not yet adjust for repeated sampling, family structure, age, sex, ethnicity or other covariates.

## What v0.3 adds

The v0.3 desktop interface keeps the **Merge & Export** and **Explore & QC** workspaces, but makes them much more practical for repeated use:

- **Remembered file-dialog directory** — the last directory is persisted across application launches and reused by the native file/folder dialogs.
- **Drag and drop** — drop multiple `.xlsx` files, an entire folder, or a previously merged `.tsv` onto the application. A pasted path can also be added from the input card.
- **Merged TSV as the preferred QC source** — after a successful merge, Explore & QC automatically points at the generated TSV instead of reopening every Excel workbook. A prior merged TSV can also be opened directly, without loading the original workbooks at all.
- **Bounded-memory merged-file QC** — sample distribution summaries read the merged TSV in bounded sample chunks rather than building the full cohort matrix in memory.
- **Scrollable identifier browser** — all genes/transcripts are visible in a virtualized table even before a search is entered. Search filters that table instead of hiding the catalogue entirely.
- **Faster, clearer gene/transcript profiles** — a row-level **Plot** action and a prominent **Plot profile** button make selection explicit. With a merged TSV, a profile needs only one TSV scan instead of reopening every workbook.
- **Two profile views** — preserve sample order or rank samples from low to high expression, with hover annotations and a collapsible sample/value table.
- **Three sample-QC views** — distribution box plots, non-zero detection rate across samples, and median-abundance versus detection-rate scatter.
- **Improved box-plot readability** — translucent boxes plus high-contrast median and mean markers make sample centers easier to compare.

The output side still provides explicit value/suffix checkboxes, **Default / All / Clear** shortcuts, sample-prefix versus value-suffix column ordering, searchable sample preview, hover help, and an output-column preview.

## Supported report schemas

### Gene expression

Expected columns include:

- `Gene Symbol`
- `NCBI Gene ID`
- `RNA-Seq Raw Counts`
- `FPKM`
- `CPM`
- `TPM`
- `Percentile Rank`
- `Is Expressed`

The **Default** measurement selection is:

- Raw Count
- TPM

### Transcript expression

Expected columns include:

- `Gene Symbol`
- `NCBI Gene ID`
- `Transcript Name`
- `Transcript Length`
- `Effective Transcript Length`
- `Total Reads`
- `TPM`
- `Percentile Rank`
- `Is Expressed`

The **Default** measurement selection is:

- Total Reads
- TPM

The application auto-detects gene vs transcript reports from the workbook headers.

## Sample names

Standard report suffixes are removed automatically. For example:

```text
4C_101_tumor_rna_gene_expression_report.xlsx
```

becomes:

```text
4C_101
```

The sample preview lets you inspect these names before merging.

## Memory strategy

The first workbook establishes the canonical row index. Subsequent workbooks are processed in bounded groups. Only one workbook plus a bounded set of selected measurement arrays is retained in memory. Each group is transposed to a temporary row-major binary chunk, and the final TSV is streamed row-by-row from those chunks.

This avoids the previous R pattern of retaining all workbooks/data frames, a large long-format object, and a wide pivoted object in memory simultaneously.

Explore & QC now prefers a merged TSV whenever one is available. Selected-identifier profiles are extracted with a single TSV scan. Sample-distribution QC uses bounded sample chunks sized around the same working-memory target as the merger, so a large merged transcript table does not need to be loaded as one giant matrix. If no merged TSV exists, the application still falls back to processing the Excel workbooks one at a time.

## Row policy

The first workbook currently defines the canonical set and order of genes/transcripts.

- Missing rows in later workbooks are emitted as blank values and counted in the merge summary.
- Rows that occur only in later workbooks are skipped and counted as unknown rows.

This is a deliberate safety policy for the standardized reports. Future versions can add explicit `strict`, `union`, and `intersection` policies.

## Build

Rust 1.95 or newer is required by the current `eframe`/`egui_plot` stack.

```bash
cargo build --release
```

Binaries:

```text
target/release/cfs-expression-merge
target/release/cfs-expression-merge-gui
```

On Windows the binaries use the `.exe` extension.

Run the desktop application:

```bash
./target/release/cfs-expression-merge-gui
```

or during development:

```bash
cargo run --release --bin cfs-expression-merge-gui
```

## Desktop workflow

### Merge & Export

1. Choose individual XLSX files or a folder.
2. Check the sample-prefix preview.
3. Leave report type on **Auto** unless an override is needed.
4. Check the values/suffixes that should be written.
5. Use **Default** for Raw Count + TPM (gene) or Total Reads + TPM (transcript).
6. Choose **By sample prefix** or **By value suffix** column ordering.
7. Review the output-column preview.
8. Choose the output TSV.
9. Click **Merge datasets**.

### Explore & QC

1. Use the merged TSV created in the current session, click **Open merged TSV…** to load an older merged result, or explicitly fall back to the selected Excel workbooks. Direct TSV loading currently expects the collaborator-style headers generated by this tool (`Gene Symbol` or `Transcript Name`, `NCBI Gene ID`, then columns such as `4C_101 TPM`).
2. Choose a numeric measurement available from that source.
3. Optionally ignore zeros for distribution statistics and/or use the log10 display transform.
4. Click **Build / refresh QC**, then switch among **Distributions**, **Detection rate**, and **Median vs detection**.
5. Browse the full scrollable gene/transcript table or filter it with the search box.
6. Click a row, then **Plot profile**, or use the row's **Plot** button directly.
7. Switch the profile between **Sample order** and **Ranked** views and hover points for sample/value annotations.

A selected gene/transcript has one value per sample, so the profile view uses individual sample points rather than a misleading per-sample box plot. The sample-level distribution panel instead summarizes all genes/transcripts within each sample.

### Cohort Analysis

1. Choose the top-level cohort project folder. The app recursively detects supported modalities and existing merged expression tables.
2. Load the primary sample metadata table. Optionally load a collaborator-supplied **filtered metadata subset**; this changes which samples define the run, not the statistical methodology.
3. Review **Sample mapping & QC**. The mapping audit keeps unmatched, ambiguous, metadata-only, duplicate, and modality-coverage information visible without aborting other usable modalities.
4. Optionally run **cross-modal sample QC**. Automatic consensus outliers are review flags only. Explicitly select any samples that should be excluded after review; no sample is automatically selected for deletion.
5. In **Analysis settings**, choose the comparison, modalities, subject/repeated-measure column, covariates, repertoire settings, GMT/pathway options, whether reviewed QC exclusions should be applied, and optional PBMC/confounding sensitivity diagnostics.
6. Click the single **Run analysis** button. The corrected/standard methods are the primary inferential engine; modality failures remain fault tolerant. Optional exploratory TPM calculations are used only as descriptive direction-concordance support.
7. Inspect **Unified results** by modality plus the QC / robustness view. Candidate genes/pathways/other signals are a shortlist derived from the same corrected results, not a separate analysis.
8. Export either **Simplified key-findings report**, **Full detailed report**, or the complete technical table set. All three exports use the exact same frozen completed run and therefore cannot silently change sample inclusion or rerun a different statistical engine.

The application supports discovery, robustness assessment, and candidate prioritization. Preliminary candidates should not be described as validated clinical biomarkers without independent replication/validation.

### HTML reports

A completed unified run supports two report detail levels:

- **Simplified key-findings report** — presentation-friendly comparison/QC summary, concise main takeaway, strongest preliminary genes/pathways/other modality signals, a small number of main plots, and short limitations. Candidate wording explicitly avoids validated-biomarker claims.
- **Full detailed report** — cohort definition, mapping audit, exact reviewed QC exclusions and reasons, per-modality outlier evidence, all selected corrected modality results, ordinations/dendrograms, pathway provenance, PBMC/confounding sensitivity analyses, candidate robustness, warnings, methods, assumptions, and limitations.

Both reports are rendered from the same `UnifiedCohortAnalysisRun`; report generation does **not** rerun statistics. The full technical table export is generated from that same frozen object as well.

`report-builder` 0.1.1 normally references Plotly and optional table helpers from public CDNs. v0.5+ compiles Plotly with its `plotly_embed_js` feature, injects that bundled JavaScript into the report, and removes the unused DataTables/FileSaver CDN helpers during finalization. The resulting `.html` is therefore one self-contained file whose interactive plots can be opened offline. This post-processing is pinned to the current report-builder 0.1.1 template and is covered by a report-generation unit test.

## CLI examples

Gene expression, default Raw Count + TPM:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_merged.tsv \
  --kind gene \
  --preset collaborator
```

Transcript expression, default Total Reads + TPM:

```bash
cfs-expression-merge \
  --input ./Transcript_expression \
  --output ./transcript_expression_merged.tsv \
  --kind transcript \
  --preset collaborator
```

Export all available measurements:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_all.tsv \
  --kind auto \
  --preset all
```

Custom measurements:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_tpm_fpkm_cpm.tsv \
  --kind gene \
  --preset custom \
  --metrics tpm,fpkm,cpm
```

## Cross-platform packaging

The same source can be compiled into native releases for each operating system:

- Windows `.exe` / zip
- macOS binary or `.app` bundle
- Linux binary/AppImage

A single executable cannot run unchanged on all three operating systems because their binary formats and ABIs differ. The included GitHub Actions workflow is the starting point for creating per-OS release artifacts.

## Good next additions

For cohort analysis in particular, useful next modules are:

- Repertoire similarity heatmaps / dendrograms and richer public/longitudinal clonotype browsers on top of the corrected Morisita-Horn/Jaccard and matched-depth analyses.
- Immune-infiltrate cell-type heatmaps and cross-modal correlation with expression/repertoire features.
- Ranked pathway enrichment in addition to the corrected GMT-based ORA now available.
- A more specialized DTU model (e.g. Dirichlet-multinomial/DEXSeq-like); the current corrected layer provides a CLR/Simes DTU screen with its assumptions stated explicitly.
- Gene-expression PCA/correlation heatmaps and selected top-gene heatmaps.
- Cross-modal candidate-biomarker summaries that combine gene, immune-infiltrate, BCR, BCR-isotype, and later TCR evidence.
- Further validation/regression fixtures for the unified runner across representative longitudinal/covariate configurations.

General merge/QC additions:

- Preflight schema validation for every workbook, not only the first report.
- Duplicate sample-prefix detection and editable sample names.
- `strict`, `union`, and `intersection` row policies.
- Save/load named export and QC presets.
- Metadata/group import so plots can compare biological groups, diagnoses, treatments, or batches.
- Multi-gene selection with heatmaps and grouped box/violin plots.
- Sample-sample correlation heatmap and PCA using the merged TSV with a bounded/sampled representation.
- Highly-variable-gene ranking and top-variable-gene browser.
- Optional XLSX export for modest-sized matrices.
- Long-format export matching the legacy R workflow.
- A detailed QA report listing schema mismatches, missing/extra rows, and duplicate identifiers per sample.

## Unified Cohort Analysis architecture (v0.15.0)

The corrected / standard layer remains the statistical backbone, but the desktop UX now presents one workflow rather than asking users to choose between competing analysis modes. A completed `UnifiedCohortAnalysisRun` owns the cohort definition, mapping audit, frozen QC provenance, settings, corrected results, optional supporting diagnostics, candidate shortlist, and warnings.

The primary corrected methods are preserved:

- Gene raw counts and transcript Total Reads: positive-count median-ratio normalization, negative-binomial log-link count models with log(size-factor) offsets and per-feature dispersion, metadata covariates, subject-clustered sandwich SEs for repeated sampling, BH FDR, subject-collapsed log2-normalized sensitivity tests, PCA, and hierarchical clustering. Transcript data additionally retains the within-gene CLR/Simes/BH DTU screen.
- BCR/TCR: exact without-replacement matched-depth rarefaction, adjusted repertoire metrics, subject-aware private/longitudinal/public clonotype fractions, Morisita-Horn and Jaccard PCoA, subject-blocked permutation PERMANOVA, CLR/Aitchison composition analysis, and dendrograms.
- TCR: AA-convergence read fraction when nucleotide clonal sequences are available.
- BCR/TCR V/D/J/V-J ambiguity handling: exclude tied calls (default), split weight, legacy first-hit, or gene-family collapse.
- Dedicated BCR isotypes: CLR/Aitchison analysis, optionally excluding `Undetermined` and renormalizing as a sensitivity analysis.
- Immune infiltrate: covariate-adjusted ssGSEA models plus PCA and clustering of standardized signatures.
- Optional GMT pathway ORA from corrected gene-count results using the tested-gene universe, odds ratios, explicit overlap genes, and BH FDR. Public MSigDB collections and custom GMT files remain supported.

Cross-modal outlier QC and PBMC/confounding diagnostics are now QC/sensitivity evidence attached to the same run. Automatic cross-modal outliers are flags requiring review; when reviewed exclusions are applied, the exact audit, manual overrides, excluded IDs, and before/after group counts are frozen for exports/reports.

Candidate prioritization is likewise derived from the corrected result. A collaborator-filtered metadata table simply defines a narrower cohort, and the same corrected methodology is used. Optional TPM direction concordance and PBMC/confounding robustness classifications annotate the shortlist without becoming independent statistical evidence.

The expression count model does **not** claim to reproduce the full DESeq2, edgeR, or limma-voom empirical-Bayes/dispersion machinery. Final publication-level findings can still be cross-checked with specialized packages; 4C Cohort Explorer is intended for transparent cohort discovery, robustness assessment, and follow-up prioritization.


### Phase 3E.4 — scientific result readiness decoupled from export finalization

Frozen unified jobs now publish the native scientific result as soon as the native analysis completes. The browser can inspect cohort/modality summaries and prioritized signals while interactive visualization enrichment, HTML reports, and exhaustive technical-table artifacts continue finalizing for the same immutable job. Export latency therefore no longer hides an already-completed scientific result.

In v0.16.13 the browser still receives `results_ready` before optional export finalization, but the default local scientific semaphore remains held until that job finishes finalizing. This prevents a new CPU-heavy fit from competing with an older job's large report/table export on the same workstation.
