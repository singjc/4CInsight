# 4C Cohort Explorer web development

## Current status: Phase 3D.2 native-like high-throughput ingest (v0.16.8)

The browser client now runs through the native Rust `cfs-server` and supports private temporary projects, authenticated opaque-object storage, native-like folder scanning, automatic standard role/sample-ID assignment, bounded concurrent uploads, the existing native mapping audit, frozen persistent jobs, native cross-modal QC, and one representative corrected gene-expression lane.

Phase 3D.2 is primarily a **large-cohort ingest usability/performance iteration**. It addresses the realistic case where one cohort contains hundreds of gene-expression workbooks plus BCR/TCR/isotype/immune files: users should not have to upload them serially or manually assign a role to every standard 4C filename.

The full desktop scientific stack is **not** web-connected yet. Phase 3D remains a parity bridge: browser/server ingest ultimately feeds the same native mapping/QC/scientific functions rather than creating a second browser-side implementation.

```text
Desktop
    egui desktop UI
        ↓
    LocalBackend
        ↓
    existing Rust filesystem + complete scientific analysis

Browser (Phase 3D.2)
    egui / eframe WebRunner (WASM)
        ↓ same-origin HTTP
    cfs-server (Axum, native Rust)
        ├── /       → web/dist static assets
        └── /api/*
              ├── project capability/lifecycle API
              ├── bounded concurrent opaque-object uploads
              ├── shared native filename conventions
              ├── validated automatic role/sample-ID assignment
              ├── native mapping-audit API
              └── frozen persistent job API
                        ↓
               private local filesystem storage
                        ↓
               existing native mapping / QC /
               representative gene-corrected engine
```

The browser crate remains separate under `web/`; workbook parsing, report writers, and the scientific engine continue to compile and run natively rather than inside WASM. `src/input_conventions.rs` is pure shared classification logic imported by native scanning, the server, and WASM, while the browser continues to reuse `src/analysis_docs.rs` and `src/ui_theme.rs` for single-source docs/theme behavior.

## Run the local web application

From the repository root:

```bash
cd /home/singjc/Documents/github/cfs-expression-merger
./scripts/run_web_local.sh
```

Open:

```text
http://127.0.0.1:8080
```

`run_web_local.sh` uses Trunk only to build `web/dist/`; the Rust `cfs-server` process serves both the static WASM application and `/api` from the same origin. Python and `trunk serve` are not runtime dependencies.

For a server-only iteration when `web/dist` is already current:

```bash
CFS_SKIP_WEB_BUILD=1 ./scripts/run_web_local.sh
```

### Native/server validation

```bash
cargo fmt --all
git diff --check
cargo check
cargo test --release
cargo build --release
```

### WASM validation

```bash
cargo fmt --manifest-path web/Cargo.toml --all
git diff --check
cargo check --manifest-path web/Cargo.toml --target wasm32-unknown-unknown
cargo build --manifest-path web/Cargo.toml --target wasm32-unknown-unknown --release
```

The root and web crates have locally generated lock files. Keep the updated `Cargo.lock` and `web/Cargo.lock` after a successful local build.

---

## Security model: project ID is not the secret

Every private project has two independent random values:

```text
project UUID
    → identifies the project

recovery token (256-bit random secret)
    → authorizes access to project/object/mapping/job/artifact endpoints
```

Only:

```text
SHA-256(recovery token)
```

is persisted server-side. The recovery token is not derived from the uploaded data, filename, project UUID, or content hash. A file SHA-256 is integrity/provenance metadata and is **not** an authorization secret.

Every private object, mapping, job, and artifact operation uses:

```http
Authorization: Bearer <recovery-token>
```

The Phase-3D browser keeps the recovery token in memory rather than persistent browser storage. Treat a copied recovery token or future recovery file as a password.

The current server binds to `127.0.0.1` by default. Do not expose the development server to a LAN/internet endpoint containing identifiable patient data until an HTTPS/reverse-proxy deployment and the corresponding security review are implemented.

---

## Local storage layout

Default storage root:

```text
~/.local/share/4c-cohort-explorer-web/
```

A project evolves approximately as:

```text
projects/<project-uuid>/
├── project.json
├── inputs/
│   └── <object-uuid>.bin
├── objects/
│   └── <object-uuid>.json
├── manifests/
│   ├── inputs.json
│   └── mapping_audit.json
├── jobs/
│   └── <job-uuid>/
│       ├── job.json
│       ├── snapshot.json
│       ├── snapshot/
│       │   ├── inputs/
│       │   │   └── <object-uuid>.bin
│       │   └── scratch/
│       └── artifacts/
│           ├── <artifact-uuid>.bin
│           └── <artifact-uuid>.json
├── results/
├── reports/
└── scratch/
```

The browser/API works with opaque project/object/job/artifact identifiers. Browser DTOs do not contain server filesystem paths. This is the boundary that allows a later `S3Storage` implementation to replace `LocalFsStorage` without changing the logical browser contract.

On Unix-like systems the server attempts to use private permissions (`0700` directories and `0600` sensitive metadata/data files).

### TTL/deletion

Projects have fixed expiry metadata. The development default is 24 hours and the maximum configurable project retention is currently 7 days. Expired project directories are removed on server startup and by the hourly cleanup task.

Permanent project deletion removes the complete project directory, including:

- uploads;
- role/mapping manifests;
- frozen job snapshots;
- private result artifacts;
- scratch files.

Abandoned `.part` upload files are cleaned immediately on handled upload failure, on startup, and by the periodic stale-scratch cleanup.

---

## Uploads: native-like folder scans and bounded concurrency

The server retains one authenticated object-upload primitive:

```text
POST /api/projects/{project_id}/objects?filename=...
```

The browser provides:

- **Choose files…** — arbitrary browser multi-file selection, including intentionally custom-named inputs;
- **Choose folder…** — directory selection using browser `webkitdirectory`/`directory` semantics where supported.

For a folder selection, Phase 3D.2 first applies the **same shared filename conventions used by the native cohort scanner**. Recognized 4C sample workbooks, merged expression tables, and metadata candidates are queued. Auxiliary/unrecognized files such as GMT libraries, logs, unrelated reports, and other files in the same directory are skipped by default. This avoids uploading an entire analysis directory merely because relevant 4C workbooks live inside it.

A batch is still **not** collapsed into one giant request. Each file remains independently authenticated and independently addressable, but the WASM client now runs a bounded number of those requests concurrently:

```text
browser selection: N recognized files
        ↓
        ├─ file 1 ─┐
        ├─ file 2 ─┤  up to configured concurrency
        ├─ file 3 ─┤  (local default 6)
        └─ ...    ─┘
              ↓
       individual authenticated streams
              ↓
       opaque object UUID per file
       SHA-256 per file
       quota check per file
```

Default upload concurrency:

```text
local/self-hosted profile        6
hosted/conservative profile      2
```

Override it deliberately with:

```text
CFS_MAX_CONCURRENT_UPLOADS
```

This setting is independent of `CFS_MAX_CONCURRENT_JOBS`, which controls CPU-heavy scientific workers rather than HTTP upload streams.

### Server-side upload locking

The local server no longer holds its global project mutation lock while receiving an entire request body. Each upload now uses:

```text
short authorization/quota preflight under lock
        ↓
release lock
        ↓
stream + SHA-256 into unique private .part file
        ↓
defensive inferred-role validation where applicable
        ↓
reacquire lock
        ↓
exact object-count/storage recheck + atomic finalization
```

This preserves authoritative serialized quota accounting while allowing several file bodies to stream/hash concurrently. Phase 3D.2 also avoids rebuilding the full `manifests/inputs.json` after every single upload; assignment/mapping operations regenerate it once from authoritative object records. Together these changes remove the biggest avoidable serial/O(n²)-like costs seen with hundreds of files.

If a concurrent upload fails, the server removes the partial scratch object and the browser retains that failed `File` in the pending selection so it can be retried without re-selecting the whole batch.

Directory-relative browser paths are **never** used as server storage keys. Original filenames remain classification/provenance metadata only. A future S3 backend can preserve the same opaque-object contract.

### Upload limits and quota profiles

The server now has two named default profiles. The **local/self-hosted** profile is the default for `cfs-server` and is intended for real cohort work on a machine whose operator controls the private storage:

```text
CFS_QUOTA_PROFILE=local       (default)
maximum individual file       2 GiB
maximum retained project     20 GiB
maximum uploaded objects      4096
maximum persisted jobs          16
concurrent local uploads         6
concurrent local jobs            1
maximum one job artifact      128 MiB
```

The **hosted/conservative** profile retains the original Phase-3B smoke-test limits:

```text
CFS_QUOTA_PROFILE=hosted
maximum individual file      64 MiB
maximum retained project    512 MiB
maximum uploaded objects       64
concurrent uploads               2
```

Configuration/overrides:

```text
CFS_QUOTA_PROFILE            local (default) or hosted
CFS_MAX_FILE_BYTES           overrides profile per-file default
CFS_MAX_PROJECT_BYTES        overrides profile project default
CFS_MAX_OBJECTS              overrides profile object default
CFS_MAX_CONCURRENT_UPLOADS   local default 6; hosted default 2
CFS_MAX_JOBS                 default 16
CFS_MAX_CONCURRENT_JOBS      default 1
```

`CFS_MAX_PROJECT_BYTES` is a **logical retained-storage budget**, not merely raw uploads. It includes uploaded object bytes + frozen job snapshot bytes + persisted job artifacts. This is intentionally conservative even when local hard links mean snapshot bytes consume little additional physical disk space.

The browser preflights a selected batch using both projected object count and projected retained bytes, while the server remains authoritative and rechecks every individual upload. Large remote/internet deployments should later use resumable/multipart object storage behind the same opaque object interface rather than simply raising HTTP upload limits without bounds.

---

## Input-role contract and automatic assignment

Supported persisted roles remain:

```text
unassigned
primary_metadata
filtered_metadata
gene_expression
transcript_expression
bcr
bcr_isotype
tcr_alpha
tcr_beta
immune_infiltrate
merged_gene_expression
merged_transcript_expression
```

Phase 3D.2 changes **how standard inputs arrive at those roles**. A shared pure-string classifier in `src/input_conventions.rs` is used by native project scanning, `cfs-server`, and WASM. It recognizes these exact native sample-workbook suffixes:

```text
*_tumor_rna_gene_expression_report.xlsx
*_tumor_rna_transcript_expression_report.xlsx
*_bcr_heavy_chain_report.xlsx
*_bcr_isotype_composition.xlsx
*_tcr_alpha_clone_report.xlsx
*_tcr_beta_clone_report.xlsx
*_immune_infiltrate.xlsx
```

The prefix before the suffix becomes the proposed sample ID. Explorer/Finder-style copy suffixes such as `...report (1).xlsx` are normalized before classification. Native merged gene/transcript TSV naming and workbook metadata-candidate naming are also recognized for folder filtering.

Automatic assignment is deliberately conservative:

1. the filename must exactly match one of the standard conventions;
2. the corresponding uploaded bytes must pass the same defensive role validator used by manual assignment;
3. singleton-role conflicts are not silently overwritten;
4. metadata **candidates are not automatically made `primary_metadata`**, because choosing which metadata table defines the cohort is a scientific/user decision;
5. custom or ambiguous filenames remain `unassigned`.

Therefore filenames are a convenience/classification hint, not an authoritative mapping decision. The native mapping audit remains the scientific validation gate.

For an existing project uploaded with v0.16.7 or earlier, use **Auto-detect native roles & sample IDs**. The server validates unassigned recognized objects in a bounded batch, applies only safe assignments, writes the logical manifest once, and returns unresolved/validation-failed counts. No re-upload is required.

The browser defaults the large object table to **files needing attention only**. A role summary shows the automatically/manually assigned population, and **Show all uploaded objects** remains available for inspection/correction. Manual per-object role/sample edits are still supported.

Additional rules:

- exactly one `primary_metadata` object is required for mapping/jobs;
- singleton merged/metadata roles cannot be silently duplicated;
- sample-level modalities require a non-empty sample ID after assignment;
- original filenames remain display/provenance metadata only and are never storage paths or authorization credentials;
- web metadata accepts `.xlsx`, `.tsv`, `.csv`, and `.txt`;
- legacy `.xls` is currently rejected with an instruction to convert it;
- sample-level modality roles currently require `.xlsx`;
- merged expression roles currently require `.tsv`/`.txt`.

Assignments are persisted in object metadata and the logical input manifest:

```text
manifests/inputs.json
```

The manifest records opaque object ID, role, sample ID, SHA-256, and byte size, not server paths.

### Defensive XLSX checks

Before an XLSX object is accepted for a scientific role, Phase 3D retains the defensive ZIP-container checks:

```text
maximum ZIP entries             4,096
maximum one expanded entry     128 MiB
maximum expanded workbook      512 MiB
compression-ratio guard        ~500x
required [Content_Types].xml
required xl/workbook.xml
```

The check examines the ZIP metadata/container; it does not extract the workbook into a user-controlled path.

---

## Native mapping audit and snapshot fingerprint

Mapping continues to reuse the existing native implementation:

```text
opaque assigned objects
        ↓
primary metadata materialized privately
        ↓
existing load_metadata()
        ↓
native ProjectScan from explicit roles/sample IDs
        ↓
existing build_mapping()
        ↓
transport-safe MappingAuditResponse
```

The mapping response now includes:

```text
mapping_snapshot_sha256
```

This fingerprint covers the current logical input manifest plus the mapping/comparison request. The full mapping response is persisted in:

```text
manifests/mapping_audit.json
```

A Phase-3D job must present that same mapping fingerprint. The server recomputes the current fingerprint and rejects the job if any upload, deletion, or role/sample assignment made the mapping stale. The browser also clears the displayed mapping result when those project inputs change.

---

## Phase-3D persistent frozen jobs

### API

```text
GET    /api/projects/{project_id}/jobs
POST   /api/projects/{project_id}/jobs

GET    /api/projects/{project_id}/jobs/{job_id}
DELETE /api/projects/{project_id}/jobs/{job_id}

GET    /api/projects/{project_id}/jobs/{job_id}/artifacts/{artifact_id}
```

All require the recovery bearer token.

### Job submission

The browser submits:

- current `mapping_snapshot_sha256`;
- comparison column;
- Case value(s);
- Control value(s);
- optional subject column;
- optional comma-separated covariates;
- whether to run QC;
- whether to run the representative corrected gene lane.

The server rejects a stale mapping fingerprint, then creates a frozen snapshot before returning the queued job.

### Frozen bytes

For local-filesystem storage, the server attempts a filesystem hard link from the current private object blob into the job's frozen snapshot. If hard linking is unavailable it copies the bytes instead.

For quota/accounting purposes, each frozen snapshot is counted as retained **logical** project storage even when the underlying filesystem can represent it with hard links. Before a job is created, the server checks that current retained project storage plus the assigned snapshot bytes fits within `CFS_MAX_PROJECT_BYTES`. This is intentionally conservative: it prevents persistent jobs from bypassing the project storage budget and retaining many generations of private input data indefinitely.

The scientific job therefore consumes its own frozen snapshot location rather than the mutable project object location. Deleting or reassigning an uploaded object after job submission cannot silently change the already-created job snapshot.

A future S3 implementation should provide equivalent immutable/versioned snapshot semantics through object references or copies rather than depending on POSIX hard links.

### Lifecycle

Persisted states:

```text
queued
  ↓
preparing
  ↓
running
  ↓
complete | failed | cancelled
```

The browser polls active jobs about once per second and can list/reopen persisted job records after a browser reload or project recovery. The local server uses a worker semaphore and defaults to **one concurrent scientific job** so a batch of submissions cannot unexpectedly saturate a laptop; additional jobs remain queued until a slot is available. `CFS_MAX_CONCURRENT_JOBS` can raise that limit deliberately.

If `cfs-server` restarts while a job was `queued`, `preparing`, or `running`, Phase 3D marks that job `failed` with an interruption message. It does not pretend that an interrupted native stage resumed safely.

### Cancellation

Cancellation is currently best effort:

- queued/preparing jobs can be cancelled before native execution;
- running jobs receive `cancel_requested`;
- the first Phase-3D worker cannot preempt an arbitrary blocking native scientific function midway through that function;
- it checks cancellation between stages and discards later completion when cancellation was requested.

This limitation is explicit rather than pretending a native CPU-bound call was interruptible.

---

## Scientific execution scope in Phase 3D

### Mapping

Every job rebuilds mapping from the frozen snapshot using the same native mapping code and persists a private `mapping_audit.json` artifact.

### Cross-modal/sample QC

When requested, the worker selects the usable frozen QC representations and calls the existing:

```rust
run_cross_modal_outlier_qc(...)
```

with `OutlierConfig::default()`.

Available representations can include merged gene/transcript tables and assigned BCR/BCR-isotype/TCR/immune workbooks. If the frozen project cannot form a usable QC representation, the job records a warning rather than fabricating a QC result.

A `qc_summary.json` artifact is persisted when QC succeeds.

### Representative corrected gene-expression lane

This is the only corrected inferential lane connected in Phase 3D.

Requirements:

- an object explicitly assigned `merged_gene_expression`;
- a current valid mapping/comparison;
- the merged TSV uses the same schema expected by native mode.

The worker calls the existing native:

```rust
run_corrected_expression(
    ...,
    "Raw Count",
    "Gene",
    &CorrectedModelSpec::default(),
    ...,
)
```

with the optional requested subject column/covariates added to the model specification.

The browser summary shows up to the top 50 results ordered by q value and then absolute group effect. The server persists **all** tested gene rows as:

```text
gene_corrected_results.tsv
```

under an opaque private artifact ID.

No new web-only count model, normalization, p-value, q-value, or QC algorithm was introduced.

### Still intentionally not connected

Phase 3D does not yet expose the complete unified remote analysis stack:

- corrected transcript abundance / DTU;
- complete BCR/TCR corrected repertoire output;
- BCR-isotype corrected composition output;
- immune corrected output;
- pathway ORA;
- PBMC/confounding diagnostics;
- candidate prioritization/full unified result object;
- simplified/full HTML report generation;
- polished in-browser artifact download controls;
- S3/MinIO;
- user account/login system.

The private authenticated artifact endpoint exists, but the browser currently presents artifact IDs/hashes rather than a finished download UX.

---

## Phase-3D API health

```bash
curl -s http://127.0.0.1:8080/api/health
```

Expected capabilities include:

```json
{
  "status": "ok",
  "api_version": 6,
  "upload_enabled": true,
  "mapping_enabled": true,
  "analysis_enabled": true,
  "persistent_jobs": true,
  "representative_gene_enabled": true,
  "batch_upload_enabled": true,
  "storage_backend": "local-filesystem",
  "max_concurrent_jobs": 1,
  "max_concurrent_uploads": 6
}
```

`analysis_enabled=true` means the **narrow Phase-3D server job lane** is available; it must not be interpreted as complete desktop/web analysis parity.

---

## Recommended Phase-3D validation sequence

Use harmless synthetic/non-patient data first.

### 1. Large native-like folder ingest

Create a temporary project and test a realistic directory containing many standard 4C workbooks plus auxiliary files. For the user's current validation cohort, the 323 gene-expression workbooks are a useful real-world stress test after synthetic testing.

Verify:

- **Choose folder…** queues recognized 4C inputs and skips GMT/log/unrelated files;
- the selected-batch status reports role counts and skipped-file count;
- upload progress reports bounded concurrency (local default 6);
- every file still becomes one opaque object with its own SHA-256;
- standard native-named workbooks arrive already assigned to the expected role/sample ID;
- only failed uploads remain selected for retry;
- no selected folder hierarchy appears as a physical server storage path.

For intentionally custom filenames, use **Choose files…**; those inputs are allowed but may remain unassigned for manual review.

Inspect:

```bash
find ~/.local/share/4c-cohort-explorer-web/projects/<PROJECT_UUID> -maxdepth 3 -type f -print
```

### 2. Automatic assignment + mapping parity

If validating an already-uploaded v0.16.7 project, first click **Auto-detect native roles & sample IDs** and verify the recognized sample workbooks are assigned without re-uploading. Manually choose the one primary metadata object and resolve only genuinely custom/ambiguous inputs.

Run the server mapping audit and compare the exact same synthetic cohort in native mode:

```text
metadata rows
canonical samples
matched files
unmatched files
ambiguous files
metadata-only samples
per-modality coverage
Case/Control counts
sample-level mapping status/counts
```

These should match because the server invokes the existing native mapping implementation.

### 3. Frozen job without merged gene table

Submit a Phase-3D job with QC enabled but no `merged_gene_expression` role. Mapping should complete; QC should run only if another usable representation exists; the representative gene lane should produce an explicit warning rather than inventing an analysis.

### 4. Representative gene parity

With a harmless valid merged gene-expression TSV assigned:

1. rerun mapping so the fingerprint is current;
2. submit the same comparison used in native mode;
3. use the same optional subject/covariates;
4. compare the complete `gene_corrected_results.tsv` artifact with the corresponding native corrected gene table.

The core effect estimates/p/q values should match because the same native function and defaults are used.

### 5. Snapshot immutability

After submitting a job, alter/delete an original uploaded object. The frozen job already created should remain based on the submitted snapshot; a **new** job must require a freshly validated mapping if project inputs changed.

### 6. Persistence/recovery

After a completed job:

1. refresh the browser or clear/re-enter recovery credentials;
2. refresh the persisted job list;
3. open the old job;
4. confirm its mapping/QC/gene summaries and artifact metadata are still available.

### 7. Restart behavior

If practical with synthetic data, stop `cfs-server` during a running job and restart it. The interrupted job should be marked failed rather than remaining indefinitely `running`.

### 8. Project deletion

Delete the project and confirm:

```bash
ls ~/.local/share/4c-cohort-explorer-web/projects/<PROJECT_UUID>
```

fails because the complete upload/snapshot/job/artifact tree was removed.

---

## Configuration reference

```text
CFS_BIND                  default 127.0.0.1:8080
CFS_STORAGE_ROOT          default ~/.local/share/4c-cohort-explorer-web
CFS_WEB_DIST              default web/dist
CFS_DEFAULT_TTL_HOURS     default 24, capped at 168
CFS_QUOTA_PROFILE         default local; alternative hosted
CFS_MAX_FILE_BYTES        local default 2 GiB; hosted default 64 MiB
CFS_MAX_PROJECT_BYTES     local default 20 GiB; hosted default 512 MiB
                         includes uploaded objects + frozen job snapshots + persisted artifacts (logical retained bytes)
CFS_MAX_OBJECTS           local default 4096; hosted default 64
CFS_MAX_CONCURRENT_UPLOADS local default 6; hosted default 2
CFS_MAX_JOBS              default 16
CFS_MAX_CONCURRENT_JOBS   default 1
```

The current job-artifact safety cap is compiled as 128 MiB per artifact.

---

## Storage-backend direction

Phase 3D remains local-filesystem-first because development is being validated on one laptop. The implementation deliberately maintains opaque IDs and logical manifests rather than leaking local paths to WASM.

The future storage progression remains:

```text
LocalFsStorage                 ← current
OperatorConfiguredS3Storage    ← future production option
possibly BYO/institutional S3  ← later advanced option
```

For private patient data, a self-hosted server using a secure local/NAS path remains a first-class deployment mode even after S3 support exists.

---

## Next web milestone

Do **not** immediately replace every native analysis path until Phase-3D parity has been validated locally.

The next logical milestone is Phase 3E: expand the frozen job to the complete unified cohort-analysis result/report contract while continuing to call the existing native scientific functions. That phase should also add polished authenticated artifact downloads and decide how/when raw uploads can be deleted after a completed privacy-first run.
