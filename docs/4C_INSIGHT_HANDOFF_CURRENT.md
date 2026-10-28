# 4C Insight — Rolling Handoff

Date: 2026-10-28
Scientific engine: v0.21.0 (unchanged)
Most recent browser milestone: UX3 resource safeguards (source overlay; 29/29 tests)
Deployment: NOT VERIFIED — do not claim Pages is updated
Remote: `singjc/4CInsight`
Pages: `https://singjc.github.io/4CInsight/`

## UX3: Browser resource safeguards — 2026-10-28

Status: **IMPLEMENTED IN SOURCE OVERLAY, AUTOMATED TESTS PASS, USER DEPLOYMENT NOT VERIFIED**.
Scientific engine: unchanged **v0.21.0**. No Rust/WASM science changes.
Browser shell: `0.21.0-ux3-resources`.

### Source-grounded diagnosis

- A portable application state (`Load state`) is metadata/config only. It does not read the cohort files or run science. Do not assert state loading itself caused the high CPU load without a trace.
- The deployed browser science currently runs on **one** Web Worker, not Rayon. Existing native Rust uses bounded `std::thread` and wasm32 code branches use 1 worker.
- Restoring large browser History snapshots can load large in-memory result objects. UX2 volcano SVG drew one circle, title, and text label for every test feature; large results could make the DOM very large. `paddedExtent()` also used `Math.min(...values)` and `Math.max(...values)`, which can exceed JS argument limits on large results.
- Browser JavaScript cannot reliably read true Linux RAM or current free physical memory. `navigator.deviceMemory` and `performance.memory` are optional and differ across browsers. Report only the source of each figure.

### UX3 overlay changes

- Global Settings page, also reachable from the system chip, with locally persisted settings (`localStorage`). It is deliberately independent of saved project states so importing a state cannot override safety settings.
- CPU target default **85%**, thread ceiling default **8**, one core reserved when possible. The page displays the **future PX1 planning ceiling**. This is **not an active Rayon or operating-system CPU limit** in the current single-worker browser runtime.
- Maximum per-file input default **256 MiB**. Input working-budget default **1.5 GiB**. Checks run **before** each `File.arrayBuffer()` operation in the science worker. The estimated input-copy factor is 4. These are input safeguards, **not a hard WASM heap/physical-memory cap**.
- Optional manual physical RAM value (default 0 = not supplied). If entered, chip labels it **manual**. Reported browser device memory / JS heap limit is not current free RAM.
- `paddedExtent()` now scans numeric bounds without spreading a large array into function arguments. Volcano extent calculation no longer creates two large intermediate coordinate arrays. Opening History no longer retains the full unused `historySelectedSnapshot` reference.
- Maximum points drawn in a volcano plot default **2,000**, adjustable 200–6,000. The sampling is deterministic and spans the original row order. All tests are kept in the scientific result, table search/sort, and exports.
- Completed science operations release the Worker/WASM memory by terminating the Worker; next request starts a new worker. This can reduce retained WASM high-water memory between analyses.
- When an operation is pending, the sidebar offers **Stop operation**. It terminates the worker; original files remain unchanged.
- History snapshots do not use an extra `JSON.stringify`/`JSON.parse` deep clone before IndexedDB structured cloning. This reduces one large in-memory copy when writing history.
- `app/index.html` and isolation-worker registration use the UX3 cache version. Pages workflow syntax-checks the new resource module.

### Files in UX3 overlay

- `app/main.js`
- `app/analysis-worker.js`
- `app/resource-controls.js` **new**
- `app/styles.css`
- `app/index.html`
- `app/tests/ux3-resource-controls.test.mjs` **new**
- `.github/workflows/pages.yml`
- `docs/4C_INSIGHT_HANDOFF_CURRENT.md` (this file)
- `docs/4C_INSIGHT_CONTINUATION_PROMPT.md`

### Validation

- `node --check` changed JavaScript: PASS.
- `node --test app/tests/*.test.mjs`: **29/29 PASS** (22 existing + 7 UX3 tests).
- No Rust build required for the UX3 source overlay; GitHub Actions Pages workflow itself runs its normal WASM rebuild on deployment.
- A full interactive browser smoke test and real 322–323 sample memory/timing benchmark have **NOT** been completed. Do not claim the reported browser crash is fixed until confirmed on the affected browser and cohort.

### Acceptance tasks / next steps

1. Apply overlay to the actual `singjc/4CInsight` checkout. Confirm the repository and inspect local modifications before extraction.
2. Run JS syntax checks, 28 browser tests, and `git diff --check`.
3. Confirm Settings opens from sidebar and top system chip. Test 85% CPU planned ceiling, manual RAM label, persistence after reload, safe input error, stop button, and worker reuse after stop.
4. Test a restored application-state file **separately** from a History snapshot. Record which action increases CPU/RAM. Run with browser Task Manager and DevTools; record browser name/version, file sizes, and console errors.
5. Test a real large transcript comparison and large saved History result. Confirm volcano max-point control reduces DOM nodes, all feature counts/exports remain unchanged, and a second run does not retain the previous WASM memory peak.
6. If a single analysis still exceeds the browser's limit, investigate Rust/WASM allocation: XLSX expansion, result serialization, large feature tables, and WASM growth. The input checks alone cannot solve this.
7. Implement **PX1 threaded WASM** only as a distinct milestone. Use bounded shared-memory worker pool, COOP/COEP, single worker fallback, true thread-count controls, native-vs-WASM parity and measured timing/peak memory.
8. After smoke test, stage and commit the overlay, then push `main` to trigger Pages.

### Versioned validation commands

```bash
node --check app/main.js
node --check app/analysis-worker.js
node --check app/resource-controls.js
node --check app/persistence.js
node --check app/docs-content.js
node --check app/coi-serviceworker.js
node --test app/tests/*.test.mjs
git diff --check
```

### Versioned commit commands

```bash
git add app/index.html app/main.js app/analysis-worker.js app/resource-controls.js app/styles.css app/tests/ux3-resource-controls.test.mjs .github/workflows/pages.yml docs/4C_INSIGHT_HANDOFF_CURRENT.md docs/4C_INSIGHT_CONTINUATION_PROMPT.md
git diff --cached --check
git status --short
git commit -m "Add browser resource controls and bounded plot rendering"
git push origin main
```

### Constraints and risks

- The product remains local-first and does not upload cohort data.
- Setting CPU target to 85% **does not set** a real CPU quota in single-worker WASM. Do not present it as Rayon tuning.
- The configurable input budget is an **early check** and not a guaranteed browser RAM limit. Workbook decompression and modeled feature allocations can exceed it.
- History open still reads the selected IndexedDB result snapshot into memory. Large saved snapshots can still be heavy. More work may be necessary for streaming results/partial history loads.
- Large-cohort validation is a hard gate before declaring crash resolved.

---

## Previous UX2 handoff (retained)

## Product rules

4C Insight is a local-first, WASM-first multi-omics cohort analysis application.

- Normal cohort analysis does not upload data.
- Local File objects remain on the user's machine.
- Rust/WASM is the authoritative scientific implementation.
- JavaScript manages workflow, rendering, persistence, and export.
- Web Worker isolates scientific execution from the UI.
- Native/WASM parity is a hard gate for scientific workflows.
- Keep browser memory bounded.
- Respond and document in ASD-STE100 style.

## Scientific workflow status

Implemented:
- W1 local-first WASM architecture
- W2 metadata and expression QC
- W3 corrected group comparison
- W4 longitudinal patient-signature analysis
- W5 Significant Hits post-selection structure

W5 commit:
`309f94c Add W5 local WASM significant-hit structure`

UX1 added:
- local drag/drop
- browser-local IndexedDB History
- portable Save/Load State
- true PCA domain zoom
- dendrogram coordinate zoom

## UX2 features

### 1. Zoomed PCA sample labels

PCA points now carry sample IDs in `data-sample`.

When the PCA view is zoomed to at least 2.2x, persistent sample labels are added next to sufficiently separated visible points.

The label algorithm:
- uses screen-space coordinates after the current PCA data domain is applied;
- greedily avoids labels whose point positions are closer than about 34 px;
- limits visible labels to 60;
- retains hover titles.

This applies to the shared PCA renderer used by global Compare Groups and Significant Hits PCA panels.

### 2. Volcano plots and global thresholds

Compare Groups now includes a volcano plot for each enabled expression modality.

Axes:
- x: adjusted log2 group effect (`beta_group`)
- y: `-log10(BH q-value)`

Dashed cutoff lines:
- global FDR threshold
- positive and negative minimum absolute log2-effect thresholds

Colors:
- up-regulated: red
- down-regulated: blue
- below threshold: neutral gray

Global numeric controls:
- FDR (BH q), default 0.05
- minimum absolute log2 effect, default 1.0

These thresholds:
- change volcano classification;
- define Significant Hits feature selection;
- do not refit the corrected differential model.

The global thresholds are persisted in the portable application state.

### 3. Searchable/sortable tables

All ordinary rendered tables are enhanced after each page render.

Features:
- search box
- clickable sortable headers
- keyboard-accessible sorting with Enter/Space
- ascending/descending sort indicators
- sticky table headers

Differential-feature tables use a managed implementation that searches and sorts the complete differential result, while rendering at most 250 matching rows to keep DOM size bounded.

Other ordinary tables are enhanced from their current rendered row set.

The significant-hit heatmap is excluded because it is a matrix visualization, not a standard result table.

### 4. Results tab removed

The left navigation no longer contains Results.

History is now the single result archive.

Compatibility:
- old saved state or code paths that refer to `results` are redirected to History;
- History snapshots restore directly to the workflow that produced them.

Dead `resultsView()` and `bindResultsEvents()` code were removed.

### 5. Searchable literature-backed Docs

Added:
`app/docs-content.js`

The Docs page now has a live search box and method sections with references local to each section.

Current sections:
- Local-first execution and privacy
- Expression QC
- Count normalization and corrected differential expression
- Repeated measurements and subject-aware inference
- Multiple testing, FDR, and volcano plots
- PCA
- Hierarchical clustering
- Differential transcript usage
- Longitudinal patient signature
- Significant Hits post-selection structure
- Scientific parity, limitations, and interpretation

Important scientific wording:
- 4C Insight uses a dependency-light native NB2 model with positive-count median-ratio normalization and unshrunk per-feature dispersion.
- It is NOT DESeq2, edgeR, or limma-voom.
- DESeq2 and edgeR are cited only as established context for negative-binomial RNA-seq modeling and normalization.
- Repeated-measure covariance is subject-clustered sandwich covariance when a subject role is selected.
- BH controls the reported multiple-testing q-values.
- DTU is a dependency-light CLR/Simes/BH screen and is NOT DEXSeq/DRIMSeq.
- Significant Hits plots and coherence metrics are post-selection descriptive results, not independent validation.

References currently included:
- Haas et al. 2017 WebAssembly
- W3C WebAssembly specification
- Conesa et al. 2016 RNA-seq best practices
- Love, Huber & Anders 2014 DESeq2
- Robinson, McCarthy & Smyth 2010 edgeR
- Liang & Zeger 1986 longitudinal GLM
- Benjamini & Hochberg 1995 FDR
- Jolliffe & Cadima 2016 PCA
- Sokal & Michener 1958 hierarchical clustering
- Aitchison 1986 compositional data / CLR
- Anders, Reyes & Huber 2012 DEXSeq context
- Hanley & McNeil 1982 AUC interpretation
- Cover & Hart 1967 nearest neighbor
- Rousseeuw 1987 silhouette
- Anderson 2001 PERMANOVA

### 6. Frozen desktop sidebar

For desktop widths >720 px:
- sidebar is sticky at top;
- height is 100vh;
- sidebar can scroll internally only if needed;
- main content scroll does not move the workflow navigation.

Mobile keeps the existing horizontal sticky workflow bar.

### 7. System information chip

The old top-right `Runs on this device` chip is replaced by browser-reported system information.

Shown:
- OS/platform when exposed by the browser
- memory class (`navigator.deviceMemory`) when available
- otherwise JS heap limit where available
- otherwise `RAM not exposed`
- logical thread count from `navigator.hardwareConcurrency`
- current scientific worker count: `1 science worker`

Important limitation:
Browsers do not reliably expose true currently available physical RAM. Do not claim the memory value is exact free memory.

### 8. Browser cache version

Browser shell version:
`0.21.0-ux2`

The cross-origin-isolation service worker still uses `cache: no-store`.

## Rayon / parallel execution audit

Do NOT claim browser Rayon is implemented in UX2.

Current state:
- native Rust heavy paths already use bounded OS-thread parallelism (`std::thread`, available parallelism, capped worker counts);
- `wasm32` branches deliberately use one scientific worker today;
- browser UI displays this truthfully.

True browser Rayon requires a separate threaded-WASM runtime:
- shared WebAssembly memory;
- `SharedArrayBuffer`;
- cross-origin isolation;
- a thread-pool initializer;
- worker bootstrap and fallback behavior.

The existing Pages/COOP/COEP groundwork is suitable, but this should be a separate performance milestone.

Recommended next milestone: **PX1 threaded WASM**.

PX1 acceptance:
1. choose `N = min(max(hardwareConcurrency - 1, 1), 8)` or a measured equivalent;
2. initialize a shared-memory WASM thread pool;
3. keep deterministic single-worker fallback;
4. run native-vs-single-WASM parity;
5. run single-WASM-vs-threaded-WASM parity;
6. benchmark real 322-sample gene/transcript workflows;
7. report timing and peak-memory changes;
8. make threaded mode default only if stable and beneficial.

## UX2 package

Archive:
`4c-insight-v0.21.0-ux2-scientific-ux-docs.tar.gz`

SHA-256:
`a6a372501644ddf3132fa6c86f07a330d48c62f68bd81ca57315fdb3cbdabc75`

Changed files:
- `app/index.html`
- `app/main.js`
- `app/styles.css`
- `app/coi-serviceworker.js`
- `app/docs-content.js` (new)
- `app/tests/ux2-usability.test.mjs` (new)

No Rust/WASM source changes.
No wasm-pack rebuild is required.

## Validation completed

Package was applied to reconstructed UX1 source.

PASS:
- installer
- exact payload comparison
- `node --check app/main.js`
- `node --check app/docs-content.js`
- `node --check app/persistence.js`
- `node --check app/coi-serviceworker.js`
- browser tests 22/22

## Local validation

```bash
git diff --check
node --check app/main.js
node --check app/docs-content.js
node --check app/persistence.js
node --check app/coi-serviceworker.js
node --test app/tests/*.test.mjs
```

Expected:
22 tests, 22 pass.

## Git after smoke test

```bash
git add   app/index.html   app/main.js   app/styles.css   app/coi-serviceworker.js   app/docs-content.js   app/tests/ux2-usability.test.mjs

git diff --cached --check
git status --short

git commit -m "Improve scientific plots tables and documentation"
git push origin main
```

GitHub Pages should redeploy automatically.

## Browser smoke test

Test:
1. top system chip
2. fixed desktop sidebar
3. Compare Groups global FDR/log2 effect controls
4. gene/transcript volcano plots
5. PCA zoom labels
6. table search and sort
7. sticky headers during scrolling
8. Significant Hits uses global thresholds
9. Results nav is absent
10. History remains functional
11. Docs search and reference links
12. stale-bundle protection after Pages redeploy
