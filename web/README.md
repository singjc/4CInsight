# 4C Cohort Explorer browser client

This directory contains the WASM/`eframe` browser client for 4C Cohort Explorer.

## Current status: Phase 3D (v0.16.6)

The browser is served by the native Rust `cfs-server` and now supports:

- private temporary project creation/recovery/deletion;
- capability-token authorization (the server stores only the token SHA-256);
- authenticated opaque-object upload/list/delete;
- **multi-file selection** and **directory selection** from the browser;
- explicit 4C input-role/sample-ID assignment;
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

## Browser upload behavior

**Choose files…** uses the browser multi-file picker. **Choose folder…** asks the browser for a directory selection (`webkitdirectory`/directory picker semantics where supported) and places all returned files into the pending batch.

The batch picker is only a client convenience. There is deliberately no giant unauthenticated "folder blob" request:

```text
selected browser files
        ↓
for each file
        ↓
existing authenticated object-upload endpoint
        ↓
opaque object UUID + SHA-256 + quotas
```

Each file therefore gets an independent opaque object ID, independent quota enforcement, and an independent integrity hash. Directory-relative paths are not server storage keys; original filenames remain metadata only.

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
