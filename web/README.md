# 4C Cohort Explorer browser client

This directory contains the WASM/`eframe` browser client for 4C Cohort Explorer.

## Current status: Phase 3D.2 native-like high-throughput ingest (v0.16.8)

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
- mapping-snapshot fingerprints that detect stale inputs;
- persistent frozen asynchronous analysis jobs;
- the existing native cross-modal outlier/QC path from the frozen job snapshot;
- one representative native corrected gene-expression lane when a `merged_gene_expression` TSV is assigned;
- persisted private mapping/QC/gene artifacts and an authenticated artifact endpoint;
- shared 4C visual theme and canonical Docs/search/toy plots.

The complete native transcript/BCR/TCR/isotype/immune/pathway corrected-analysis stack and unified HTML report/export workflow are **not** connected to remote jobs yet.

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

## Frozen Phase-3D jobs

After uploaded objects have explicit roles and the current mapping audit succeeds, the mapping result contains a fingerprint. Job submission must present that exact fingerprint. Uploading, deleting, or reassigning an input makes the old mapping stale and requires a new mapping audit before another job can be submitted.

A submitted job freezes the current assigned input bytes and logical manifest before scientific execution. Persisted states are:

```text
queued
preparing
running
complete / failed / cancelled
```

The browser polls the local server while a job is active and can list/open persisted jobs after page refresh or project recovery.

Phase 3D scientific scope is deliberately narrow:

- native mapping audit from the frozen snapshot;
- native cross-modal/sample outlier QC where usable modalities are present;
- optional representative corrected **gene expression** using the assigned merged gene-expression TSV, `Raw Count`, `Gene`, and `CorrectedModelSpec::default()` plus optional subject/covariates.

This is a parity bridge, not a second scientific implementation.

## Build checks

```bash
cargo fmt --manifest-path web/Cargo.toml --all
cargo check --manifest-path web/Cargo.toml --target wasm32-unknown-unknown
cargo build --manifest-path web/Cargo.toml --target wasm32-unknown-unknown --release
```

The web crate has its own `Cargo.lock` once generated locally. Keep/stage the updated lock file after a successful build.

See [`../docs/web_development.md`](../docs/web_development.md) for the complete API, storage, privacy, batch-upload, mapping, job, cancellation, and parity-validation notes.
