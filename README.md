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

## Current W1 status

W1 provides the new web-first foundation:

- product rename to **4C Insight**;
- standards-based static web application suitable for GitHub Pages;
- direct local file and folder selection with no upload step;
- browser-local input inventory and modality classification;
- Web Worker boundary so file processing does not block the UI;
- a Rust/WASM preflight module that reuses the shared Rust filename conventions;
- a JavaScript preflight fallback for local frontend development before the WASM package is built;
- local project-manifest export using the File System Access API where supported, with a download fallback;
- cross-origin-isolation service worker groundwork for future threaded WASM;
- task-based tabs for Project, Explore & QC, Compare Groups, Longitudinal, Significant Hits, Results, and Docs;
- GitHub Pages deployment workflow.

W1 does **not** claim that the full scientific stack is already WASM-enabled. The analysis tabs state their migration status. W2 starts the scientific port with metadata and expression workflows.

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

Build the WASM preflight bridge:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
wasm-pack build wasm \
  --target web \
  --out-dir ../app/wasm \
  --out-name four_c_insight_wasm \
  --release
```

Then serve the static `app/` directory from any local HTTP server. The app also starts without the generated WASM package. In that development-only state the worker reports that it is using the JavaScript preflight fallback. Production Pages builds compile the WASM package first and publish the static `app/` directory directly.

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

4C Insight has no analysis upload endpoint in the W1 web application. File picker handles refer to files on the user's device. The browser worker receives only the selected local inputs needed for the active task. The application must not add telemetry or remote analysis calls without an explicit product decision and visible user consent.

See [`docs/wasm_architecture.md`](docs/wasm_architecture.md) and [`docs/wasm_development.md`](docs/wasm_development.md).
