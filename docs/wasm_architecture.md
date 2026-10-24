# 4C Insight WASM-first architecture

## Goal

4C Insight is a local-first scientific web application. The primary deployment target is a static GitHub Pages site. Users select data from their own computer. Scientific processing runs on that computer in browser Web Workers and Rust/WebAssembly. Results are saved locally.

## Non-goals

The primary web app does not upload patient data to a 4C server, does not require project recovery credentials, and does not depend on a hosted analysis worker. A future native desktop wrapper can serve the same web application and WASM engine locally when browser limitations justify it.

## Layers

### 1. DOM web UI

`app/` owns navigation, accessible controls, file selection, progress, result presentation, and local export. It must not implement scientific statistics that already exist in Rust.

### 2. Web Worker boundary

Long-running work runs outside the UI thread. Worker messages use explicit versioned request/result types. Progress is coarse and meaningful. A task can be cancelled when the underlying algorithm supports cancellation.

### 3. Rust/WASM bridge

`wasm/` exposes browser-safe functions. W1 added input preflight classification. W2 adds metadata parsing and gene/transcript expression distribution QC from local bytes. Later phases add expression comparison models, repertoire analyses, longitudinal analysis, significant-hit structure, and report generation.

### 4. Shared Rust science

`src/` remains the scientific source of truth. Browser migration must move native-only I/O out of mathematical kernels rather than copy the mathematics into TypeScript.

Preferred boundary:

```text
native Path / browser File
        ↓
small I/O adapter
        ↓
bytes / reader / parsed matrix
        ↓
shared scientific function
```

## Memory rules

Browser code must not load an entire cohort of large workbooks at once. Process files in bounded batches, retain only parsed structures needed by downstream science, and release source buffers as soon as possible. Large BCR/TCR cohorts require explicit memory instrumentation before they are enabled in the browser.

## Threading

The first implementation uses one Web Worker. Scientific parallelism can later use WASM threads when `crossOriginIsolated` is available. GitHub Pages does not provide COOP/COEP response headers itself, so the app includes a service-worker path that can add the required headers to same-origin responses after the first controlled reload. A single-worker fallback must remain available.

## UI/UX rules

The web application follows seven practical interaction principles derived from Norman's design guidance:

1. Simplify the task structure: show one workflow at a time.
2. Make state visible: always show selected inputs, current stage, exclusions, and result readiness.
3. Make mappings natural: labels and controls use scientific concepts, not implementation terms.
4. Use constraints: disable impossible actions and explain what is missing.
5. Design for error: destructive actions require clear confirmation; invalid mappings are recoverable.
6. Keep knowledge visible: methods, thresholds, provenance, and examples are available in context.
7. Standardize: navigation, buttons, tables, filters, status indicators, and exports behave consistently.

## Workflow tabs

- **Project**: local inputs and metadata selection.
- **Explore & QC**: data inventory, QC, outliers, and exclusions.
- **Compare Groups**: flexible metadata-defined comparisons.
- **Longitudinal**: patient-specific stability and clustering.
- **Significant Hits**: post-selection PCA, dendrograms, and heatmaps.
- **Results**: completed analyses and local exports.
- **Docs**: methods, toy examples, and interpretation guidance.

## Scientific parity rule

A workflow is not complete because it runs in WASM. It is complete only when fixed-fixture native and WASM outputs agree within documented numerical tolerances and the browser path preserves the same scientific definitions.

## W3 corrected expression comparison boundary

W3 extends the shared Rust/WASM boundary from input QC to cohort comparison. `src/portable_compare.rs` owns browser-safe metadata-role and exclusion configuration. It constructs the same Rust cohort comparison used by the native core and delegates statistics to `src/corrected.rs`.

The corrected expression engine now accepts either a native filesystem path or an in-memory byte source. Both sources enter the same normalization, model, PCA, dendrogram, and transcript-DTU implementation. On `wasm32`, existing single-worker fallbacks are used where native builds use scoped CPU workers.

The intended data flow is:

```text
local metadata File + selected merged expression File
                    ↓
               Web Worker
                    ↓
             Uint8Array bytes
                    ↓
       portable metadata comparison
                    ↓
       shared corrected Rust engine
                    ↓
 differential tests + PCA + dendrogram + DTU
                    ↓
       browser result state / local JSON
```

The browser must not construct its own statistical model or send selected data to a server.
