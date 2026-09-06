# 4C Cohort Explorer browser client

This directory contains the WASM/`eframe` browser client for 4C Cohort Explorer.

## Current status: Phase 3E.8 shared-core repertoire teardown stabilization (v0.16.16)

The browser now renders bounded interactive presentation views directly from the completed native unified result and can download every retained job artifact as one authenticated ZIP. No statistical calculation is duplicated in WASM; full-resolution tables and both HTML reports remain the authoritative persisted outputs.

The browser is served by the native Rust `cfs-server` and now supports:

- private temporary project creation/recovery/deletion;
- capability-token authorization (the server stores only the token SHA-256);
- authenticated opaque-object upload/list/delete;
- **multi-file selection and native-like directory scanning**;
- **bounded concurrent uploads** (local default 6, hosted default 2);
- shared native filename classification and automatic role/sample-ID assignment for standard 4C workbooks after defensive validation;
- one-click auto-detection for existing older projects, so hundreds of uploaded objects do not need manual reassignment;
- an attention-first object list that hides already-assigned files by default;
- explicit manual role/sample-ID correction for custom/ambiguous inputs;
- defensive workbook/text validation;
- the existing native metadata loader and sample mapping audit;
- authenticated primary-metadata profiling with metadata-backed grouping-field selection;
- multi-select Case/Control value controls with row counts and stale-mapping invalidation;
- mapping-snapshot fingerprints that detect stale inputs;
- persistent frozen asynchronous analysis jobs;
- the existing native cross-modal outlier/QC path from the frozen job snapshot;
- full frozen native unified cohort analysis across every available corrected modality;
- reviewed QC exclusions with frozen provenance (automatic flags are never silently removed);
- native pathway/PBMC/confounding/robustness/candidate-prioritization layers where their inputs are available;
- simplified and full HTML reports generated from the same completed native unified result;
- interactive browser result plots derived from the completed native unified result;
- authenticated individual artifact/report downloads plus one-click bulk ZIP export;
- terminal persisted-job deletion;
- shared 4C visual theme and canonical Docs/search/toy plots.

The browser now executes that unified scientific stack on the native Rust server. S3/MinIO and production public-host deployment hardening remain later phases; local filesystem + localhost stay the development defaults.

## Run locally

From the repository root:

```bash
./scripts/run_web_local.sh
```

This performs the Trunk build and then starts `cfs-server`. Open:

```text
http://127.0.0.1:8080
```

Trunk is a build tool here; it is not required as the runtime web server. `cfs-server` serves both `web/dist/` and the relative `/api` endpoints from the same origin.

To skip rebuilding the WASM client while testing a server-only change:

```bash
CFS_SKIP_WEB_BUILD=1 ./scripts/run_web_local.sh
```

## Quota profiles

The local Rust server now uses an explicit quota profile. The default is appropriate for a local/self-hosted scientific workstation:

```text
profile                 local-self-hosted
maximum individual file        2 GiB
retained project storage      20 GiB
maximum uploaded objects       4096
concurrent uploads                6
```

`retained project storage` includes uploaded objects plus frozen job snapshots and persisted job artifacts. The browser shows projected storage/object usage before a selected batch is uploaded.

For a deliberately constrained future hosted deployment:

```bash
CFS_QUOTA_PROFILE=hosted ./scripts/run_web_local.sh
```

which restores the conservative 64 MiB / 512 MiB / 64-object defaults and uses 2 concurrent uploads. `CFS_MAX_FILE_BYTES`, `CFS_MAX_PROJECT_BYTES`, `CFS_MAX_OBJECTS`, and `CFS_MAX_CONCURRENT_UPLOADS` override the relevant profile defaults.

## Browser upload behavior

**Choose files…** accepts an arbitrary multi-file selection. **Choose folder…** asks the browser for a directory and filters the returned files using the same shared native 4C filename conventions used by desktop cohort scanning. Standard 4C workbooks/merged tables/metadata candidates are queued, while GMT libraries, logs, and unrelated files are skipped by default.

The client does not create one giant folder payload. It launches a bounded number of independent authenticated object requests:

```text
recognized browser files
        ↓
up to 6 concurrent requests by default on local/self-hosted
        ↓
opaque object UUID + SHA-256 + authoritative per-file/project quota checks
```

Local/self-hosted defaults to 6 concurrent uploads; hosted/conservative defaults to 2. Override with `CFS_MAX_CONCURRENT_UPLOADS`. The server streams request bodies to private scratch files outside the short project mutation lock, then serializes final quota/accounting updates. This is separate from `CFS_MAX_CONCURRENT_JOBS`, which controls CPU-heavy scientific jobs.

Standard native sample filenames automatically seed role and sample ID after defensive content validation. Metadata candidates remain manual because the server should not guess which metadata table is the primary cohort definition. Custom/ambiguous filenames remain unassigned. For a project uploaded under v0.16.7 or earlier, click **Auto-detect native roles & sample IDs** instead of re-uploading.

The object table shows unresolved files by default; use **Show all uploaded objects** to inspect/correct the assigned population. Failed concurrent uploads remain selected for retry.

Every file still gets an independent opaque object ID, integrity hash, deletion operation, and quota check. Folder-relative paths and original filenames are not storage keys or authorization credentials.

## Frozen Phase-3E unified jobs

After uploaded objects have explicit roles and the current mapping audit succeeds, a new job freezes the exact assigned input bytes and logical manifest. The server reconstructs the native cohort state from that immutable snapshot and calls the existing unified Rust analysis workflow. Browser code does not reproduce corrected statistics.

Persisted unified-job states now include:

```text
queued
preparing
running
primary_results_ready   # primary corrected modalities/candidates are reviewable
results_ready           # PBMC/confounding sensitivity diagnostics have finished
complete / failed / cancelled
```

Before submission, an opened QC result can be reviewed sample-by-sample. Automatic flags are prompts only. Checked exclusions are sent as explicit overrides, rerun against the new frozen QC audit, and recorded in the native QC provenance used by reports.

Every corrected modality actually present in the frozen project is enabled automatically. Pathway ORA additionally requires a deliberately assigned `pathway_gmt` object. Optional subject/covariate and PBMC diagnostic columns are frozen in the request. Fault-tolerant modality errors are reported in the unified result rather than replacing the native behavior.

On completion, the job persists a unified summary, bounded browser-visualization payloads, native technical tables, and both simplified and detailed HTML reports generated from the same `UnifiedCohortAnalysisRun`. **Open results** loads a persisted result below the history table and renders volcano/effect/ordination/pathway views without rerunning statistics. **Download** performs an authenticated fetch and saves through a temporary browser Blob URL; **Download all as ZIP** assembles every retained artifact plus a manifest on demand. The recovery token is never embedded in either download URL. Terminal jobs can be deleted independently of their project/source uploads.

Older Phase-3D job records are still readable and display their representative gene result when no unified payload exists.

## Build checks

```bash
cargo fmt --manifest-path web/Cargo.toml --all
cargo check --manifest-path web/Cargo.toml --target wasm32-unknown-unknown
cargo build --manifest-path web/Cargo.toml --target wasm32-unknown-unknown --release
```

The web crate has its own `Cargo.lock` once generated locally. Keep/stage the updated lock file after a successful build.

See [`../docs/web_development.md`](../docs/web_development.md) for the complete API, storage, privacy, batch-upload, mapping, job, cancellation, and parity-validation notes.



### Phase 3E.6 runtime + recovery ergonomics

- Active jobs display start/current UTC time, total elapsed time, current-stage elapsed time, last-progress age, and persisted completed-stage timings.
- Standard BCR/TCR corrected execution is optimized without changing the scientific definitions, including bounded deterministic rarefaction workers and concurrent independent ordination/composition tasks.
- Project UUID + recovery token can be saved as an encrypted `.4ckey` file. Encryption is entirely client-side using PBKDF2-HMAC-SHA-256 + AES-256-GCM; the passphrase never goes to the server.

### Phase 3E.5 primary/result-ready behavior

Unified jobs publish `primary_results_ready` once the corrected gene/transcript, repertoire/isotype, immune/pathway, supporting-direction, reviewed-QC, and initial candidate results are complete. PBMC/confounding sensitivity refits can continue afterward without hiding those primary results. When sensitivity diagnostics finish, the same frozen job advances to `results_ready`; visualization/report/table finalization can continue until `complete`.

The local scientific worker semaphore remains held through finalization in v0.16.13. Results are visible early, but a new CPU-heavy fit is not allowed to contend with a previous job's large HTML/table export on the same workstation.


### Phase 3E.8 repertoire working-set stabilization

The browser timing table can now distinguish real BCR/TCR scientific work from raw parsed-repertoire cleanup. Large TCR-beta jobs use the shared native compact clonotype representation and report a dedicated `working-set cleanup` stage; the modality is not marked complete until that cleanup is actually finished. Primary corrected results are also published independently of whether PBMC diagnostics were selected.

### Phase 3E.7 transcript/runtime + reliable encrypted-key saving

The browser now receives `primary_results_ready` before optional TPM direction-concordance and PBMC/confounding enrichment. Job telemetry remains live during those optional layers. Exact TPM support is faster through single-pass byte-position progress and bounded native feature batches. Encrypted `.4ckey` export now deliberately separates encryption from the final user-gesture save click so Firefox/Chromium cannot silently block the download after asynchronous WebCrypto work.
