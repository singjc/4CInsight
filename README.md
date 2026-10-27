# 4C Insight

**4C Insight** is a local-first multi-omics cohort analysis application. The primary application is a static web app that is designed to run from GitHub Pages. Users select files that remain on their own computer, analysis runs in browser workers with Rust/WebAssembly, and reports are saved back to the user's computer.

## Direction from v0.17.0

The old upload/server architecture is retired from the active source tree. Normal browser use must not require a 4C server, temporary project credentials, remote object storage, or file upload.

```text
local files/folder
      ↓
4C Insight web UI
      ↓
Web Worker
      ↓
Rust/WASM analysis engine
      ↓
local browser result state
      ↓
HTML / TSV / ZIP / JSON saved locally
```

The existing Rust scientific modules remain the reference implementation while they are migrated to browser-safe byte/reader APIs. Each migrated workflow must pass native/WASM parity tests before the UI enables it as production analysis.

## Current W5 status

W1 established the static local-first browser shell, W2 added metadata/expression QC, W3 added corrected gene/transcript group comparison, and W4 added longitudinal patient-signature analysis. W5 bumps the source version to **v0.21.0** and enables the **Significant Hits** workflow for gene and transcript expression in local Rust/WASM. W5 reuses the exact completed W3 comparison definition and adds post-selection PCA, hierarchical clustering, a clustered heatmap, silhouette, nearest-neighbour group coherence, and subject-blocked PERMANOVA.

- W2 metadata and per-sample gene/transcript expression QC remain available;
- the sample/expression ID, patient/subject ID, grouping column, and adjustment covariates are selected by role rather than hardcoded names;
- Case and Control can each contain one or more observed metadata values;
- reviewed-QC exclusions and a separate arbitrary metadata column/value exclusion rule are applied before cohort construction;
- gene comparison uses Raw Count columns and the established corrected NB2 expression model;
- transcript comparison uses Total Reads columns and also runs the established transcript-usage screen;
- repeated measures can use the selected patient/subject column for subject-clustered sandwich standard errors;
- global PCA and average-linkage hierarchical clustering use top-variable normalized features and do not select features from Case/Control significance;
- merged gene/transcript tables are read from local bytes inside the Web Worker;
- comparison results can be saved locally as JSON;
- fixed W3 fixtures compare the native filesystem path with the byte/WASM path for differential tests, normalization, PCA, dendrograms, exclusions, and transcript DTU.

W4 adds:

- flexible observation/sample ID and stable patient/person ID metadata roles;
- optional collection-date/timepoint/visit annotation;
- optional metadata column/value exclusion before clustering;
- gene Raw Count and transcript Total Reads longitudinal distances using the same normalized top-variable expression geometry as the native implementation;
- patient-identity AUC, nearest-neighbour patient accuracy, within-versus-between patient distance summaries, and average-linkage dendrograms;
- cross-modality ranking of gene versus transcript patient-specific stability;
- native filesystem versus byte/WASM parity fixtures for the longitudinal workflow.

W5 currently enables significant-hit structure for **gene and transcript/isoform expression**. BCR/TCR and immune-infiltration significant-hit panels, repertoire/immune longitudinal analysis, and full browser report generation remain gated until their own byte/WASM parity work is complete. Significant-hit plots are explicitly post-selection descriptive views; they are not independent validation or predictive performance.

## Repository layout

```text
app/                  standards-based HTML/CSS/ES-module web application
wasm/                 Rust/WASM browser bridge
src/                  retained Rust scientific core
src/bin/               optional command-line utility
docs/                  methods and architecture documentation
templates/             metadata templates
.github/workflows/     CI and GitHub Pages deployment
```

The legacy analysis server, server API/backend abstraction, canvas browser shell, and old native GUI target are intentionally absent. Their history remains available in Git.

## Web development

Requirements:

- Rust stable with `wasm32-unknown-unknown`;
- `wasm-pack` for the Rust/WASM package.

Build the current WASM engine:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
wasm-pack build wasm \
  --target web \
  --out-dir ../app/wasm \
  --out-name four_c_insight_wasm \
  --release
```

Then serve the static `app/` directory from any local HTTP server. The app can still show filename preflight without the generated WASM package. W2-W5 scientific workflows require the generated WASM package and remain disabled when it is absent. Production Pages builds compile the WASM package first and publish the static `app/` directory directly.

## Validation

```bash
cargo fmt --all
cargo check --locked
cargo test --locked

cargo check --manifest-path wasm/Cargo.toml
cargo test --manifest-path wasm/Cargo.toml

node --check app/main.js
node --check app/analysis-worker.js
node --check app/file-classifier.js
node --check app/save.js
node --check app/coi-serviceworker.js
```

## Privacy model

4C Insight has no analysis upload endpoint in the web application. File picker handles refer to files on the user's device. The browser worker receives only the selected local inputs needed for the active task. The application must not add telemetry or remote analysis calls without an explicit product decision and visible user consent.

See [`docs/wasm_architecture.md`](docs/wasm_architecture.md) and [`docs/wasm_development.md`](docs/wasm_development.md).
