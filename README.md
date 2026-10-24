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

## Current W2 status

W1 established the static, local-first browser shell. W2 adds the first real browser scientific workflow and bumps the source version to **v0.18.0**:

- metadata tables (`.xlsx`, `.xls`, `.tsv`, `.txt`, `.csv`) are parsed from local bytes in Rust;
- the sample-ID column is selectable from the detected metadata columns;
- gene and transcript workbooks are processed locally in a Web Worker;
- the Rust/WASM bridge calculates per-sample expression distribution QC for user-selected metrics;
- source workbooks are read sequentially so browser memory is bounded to one active workbook plus WASM working memory;
- Explore & QC shows sample-level mean, median, quartiles, zero fraction, and missing fraction;
- QC results can be saved as a local JSON file;
- fixed fixtures compare native path-based expression summaries with the byte-oriented parser used by WASM.

W2 does **not** yet enable Case/Control testing, outlier detection, longitudinal analysis, significant-hit structure, repertoire analysis, or full report generation in the browser. Those workflows remain visible but disabled until their Rust/WASM parity work is complete.

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

Build the W2 WASM engine:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
wasm-pack build wasm \
  --target web \
  --out-dir ../app/wasm \
  --out-name four_c_insight_wasm \
  --release
```

Then serve the static `app/` directory from any local HTTP server. The app can still show filename preflight without the generated WASM package. W2 metadata and expression QC require the generated WASM package and remain disabled when it is absent. Production Pages builds compile the WASM package first and publish the static `app/` directory directly.

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
