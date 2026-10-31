# 4C Insight — Rolling Handoff

Date: 2026-10-28
Scientific engine: v0.21.0 (unchanged)
Most recent browser milestone: UX4 logo branding (source overlay; 32/32 JS tests pass)
Deployment: UX3 pushed to main at a26ff40; UX4 not deployed. Live Pages status not independently checked
Remote: `singjc/4CInsight`
Pages: `https://singjc.github.io/4CInsight/`

## UX4: Brand SVG integration — 2026-10-28

Status: **IMPLEMENTED IN LOCAL SOURCE OVERLAY; 32/32 JS TESTS PASS; NOT YET COMMITTED OR DEPLOYED**.
Baseline: user's confirmed `main` commit `a26ff40` (UX3). Scientific engine remains v0.21.0.

- Move the exact existing root SVG `4C_Insight_logo_practical_trace_balanced_tight_margin.svg` into the published static app as `app/assets/4c-insight-logo.svg`. Preserve the SVG bytes; Git recognizes a rename after staging.
- The existing SVG contains the red 4, multi-colour C/human graphic, and the text Insight. Replace both the temporary 4C square and the visible 4C Insight title/subtitle with the full graphic.
- Use a semantic `h1` containing an image with `alt="4C Insight"`. This provides the accessible page title without duplicate visible text.
- Preserve the system information chip. Set desktop header height to 96 px with an 80 px logo; mobile logo height is 64 px. Update the workspace minimum height to match the changed desktop header.
- Use a relative SVG URL for GitHub Pages subpath routing. Change the CSS/JS cache query to `0.21.0-ux4-branding`.
- No Rust/WASM changes. Normal Pages deployment will still run the existing WASM build pipeline.

Validation: `node --test app/tests/*.test.mjs` **32/32 PASS**; JavaScript syntax checks PASS; verified 100% Git SVG rename with clean UX3 dry-run installer. Interactive browser visual review and Pages deployment remain **NOT VERIFIED**. Test file: `app/tests/ux4-branding.test.mjs`.

Next steps:
1. Apply UX4 only to the UX3 checkout at commit `a26ff40` or an equivalent clean tree. Verify the old SVG exists and the new path does not before the move.
2. Check image dimensions, clipping, contrast, layout at desktop and <=720 px widths, and system chip alignment in the browser.
3. Check that `https://singjc.github.io/4CInsight/assets/4c-insight-logo.svg` loads without a 404 after Pages deployment.
4. Stage both the deleted original and new SVG to record the rename; run `git diff --cached --check`; commit and push to `main`.
5. Continue PX1 threaded WASM only after browser resource behaviour is stable; make no claims of Rayon in UX4.

## UX3: Browser resource safeguards — 2026-10-28

Status: **UX3 committed and pushed by user at a26ff40. Live Pages deployment not independently checked.**
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

## UX4 installation recovery — 2026-10-28
- The user moved the SVG into app/assets manually.
- Commit 99c382b already contains the SVG.
- The original UX4 installer did not support this state.
- Remaining UX4 source files were installed without moving the SVG again.
- Browser smoke test and Pages deployment still require verification.

## UX5: Unlimited scientific input size and truthful browser memory — 2026-10-28

Status: **UX5 source patch prepared; browser smoke test and real-cohort run pending.**
Scientific engine: **v0.21.0 unchanged**. Browser shell: `0.21.0-ux5-unlimited-inputs`.
User screenshots verified UX4 branding, UX3 Settings, and a transcript `20260820_merged_transcript_expression.tsv` rejected by the 1.5 GiB estimated-copy budget. That rejection is an avoidable UX3 policy limit; it is NOT proof the file cannot run in the browser.

### Code changes

- Delete the UX3 `maxFileMiB` maximum input-file setting and the `workingBudgetGiB` early-rejection setting. A file is no longer blocked because of byte size or an inferred 4x copy factor.
- `validateLocalInputFile()` checks only valid `File.size` metadata; it has **no size cutoff**. The Web Worker then reads the file normally with `File.arrayBuffer()`. This does NOT make the read streaming or prevent WASM out-of-memory failures.
- Previously saved browser Settings with `maxFileMiB` and `workingBudgetGiB` are normalized and those retired keys are silently discarded. The global CPU planning budget, optional manual physical RAM, maximum volcano points, and post-run worker termination are retained.
- System chip distinguishes browser-reported **approximate device memory class**, **JavaScript heap limit**, **manual total RAM**, and **system RAM unavailable**. None is free Linux RAM.
- Settings includes a user-triggered **Measure app memory** button. If supported and cross-origin isolated, `performance.measureUserAgentSpecificMemory()` estimates page and worker memory. Otherwise it may report `performance.memory.usedJSHeapSize` as *JS heap only*. When unavailable (e.g. Firefox), it reports that limitation without inventing a reading. This is not native RAM monitoring.
- `sysinfo` cannot provide native Linux system information to the current sandboxed GitHub Pages browser process; it would require a distinct native desktop runtime or an explicitly installed local companion. Do not claim an OS probe exists in this browser build.
- Retain bounded volcano *rendering* (default 2,000 points) to avoid DOM overload. It does not drop features from the model, searchable differential tables, or exports.
- Portable **state JSON** has an existing separate 10 MiB loader check. This is unrelated to scientific expression input and is not changed in UX5.

### Verification

- JS syntax checked for modified modules.
- `node --test app/tests/*.test.mjs`: **34/34 PASS** in the reconstructed UX4 tree.
- A real Linux/Firefox large-input run and in-browser memory measurement are **NOT** verified. No memory-safe streaming parser or threaded-WASM pool is implemented.

### Next steps

1. Apply UX5 patch to the existing UX4 checkout, without moving the already committed SVG.
2. Confirm the previous transcript input-warning banner no longer appears after loading/reselecting files; run a gene/transcript comparison with the intended large cohort.
3. In Firefox Settings, expect physical RAM to remain browser-unavailable; manual 32 GiB appears as manual and the app-memory button may report unavailable. Test the memory sampler in Chromium only where available.
4. Use browser process monitoring to record peak RSS, worker lifetime, execution time and failure stage. No fixed file limit means allocation pressure can still crash browser processes.
5. Investigate streaming/chunked Rust/WASM input and transient copies when a real-cohort run is too large; keep scientific model and parity unchanged.
6. Implement PX1 threaded-WASM only after independent single-worker performance and parity testing.

### References

- MDN `Navigator.deviceMemory`: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/deviceMemory
- MDN `Performance.memory`: https://developer.mozilla.org/en-US/docs/Web/API/Performance/memory
- MDN `Performance.measureUserAgentSpecificMemory`: https://developer.mozilla.org/en-US/docs/Web/API/Performance/measureUserAgentSpecificMemory
- Rust sysinfo: https://docs.rs/sysinfo/latest/sysinfo/

## UX6 — Reconnect saved cohort and fixed sidebar (2026-10-28)

Scientific engine remains v0.21.0. Browser shell v0.21.0-ux6-state-reconnect.

- Root cause of empty Project after Load State: portable state JSON stores the source-file manifest and analysis settings, not source bytes or browser file access grants. Loading 2,983 references must not claim that 2,983 files have been opened.
- Project shows a **Reconnect project folder** control after Load State. A single explicit folder picker selection reconnects files; there is no browser bypass to reopen arbitrary local paths.
- Source matches use filename, exact size, and preferred relative-path match; root-folder differences can be reconciled using path suffixes; ambiguous candidates are not silently selected. The interface shows matched, missing and ambiguous counts. Existing pending comparison/QC/longitudinal configurations reapply after file preflight.
- Desktop topbar remains sticky; sidebar uses viewport-fixed layout under header, sidebar navigation scrolls separately when necessary, and Rust/WASM status stays visible at the bottom. Mobile keeps its horizontal navigation.
- Browser memory limits: native sysinfo cannot run with OS access from GitHub Pages WASM; Firefox does not expose exact total/available RAM via a standard web API. Manual system RAM entry remains truthful, and UX5 browser application-memory measurement is retained where supported. Real automatic RAM reporting requires an optional explicit native companion/desktop runtime and is not implemented by UX6.
- No input-size cap was restored. No scientific Rust or WASM algorithm was changed.
- Automated source tests: **41/41 PASS**, including 7 dedicated matching tests and a 2,983-reference test. Patch against reconstructed UX5 source: PASS. Actual user browser smoke test: PENDING. Headless browser test was blocked by local page access in the tool environment.
- For user acceptance: load prior `.4cstate.json`; confirm expected reference count; click Reconnect project folder; select full cohort root; verify matched count, missing/ambiguous count, and restored metadata/group/selected expression tables; compare and Significant Hits must not run unless sources are available; scroll a long results or Docs page and confirm sidebar status stays visible; check mobile width; confirm RAM still shows its source (manual/browser) rather than falsely claiming auto-detection.
- Recommended next milestone: optionally store browser-granted directory handles (where supported and with permission checks), add a clearly opt-in native RAM companion for Linux, and investigate streaming/chunked large-file processing. These are not included in this update.


## UX7 — Optional native sysinfo monitor (2026-10-28)

- Added a standalone native Rust helper `native/system-monitor` using `sysinfo 0.36`, `uuid` (ephemeral token), and `serde_json`. It runs on the user's machine and binds only IPv4 loopback `127.0.0.1:47831`. No raw cohort access, no uploads, no filesystem scanning.
- Browser client `app/system-monitor.js`: explicit Connect action, bearer token in memory only, strict response schema, 4-second request timeout, 3-second metric polling after connection, disconnect, stale-reading invalidation, and browser-only fallback.
- Settings displays host total/available/used RAM, CPU usage and swap when connected. Header gives source-labelled native available/total RAM; manual RAM remains fallback when not connected.
- Connection uses exact origin verification and HTTP CORS; the allowed production origin is `https://singjc.github.io`. Some browsers also gate public-HTTPS-to-loopback with a permission prompt or browser policy. This cannot be bypassed by WASM `sysinfo`.
- UX7 does NOT implement Rayon, impose hard memory/CPU limits, or change scientific results.
- UX7 tests: 47/47 JavaScript tests passed after reconstruction from uploaded base + UX3/UX4/UX5/UX6 overlays. Rust cargo/rustfmt unavailable in authoring environment, so native compile, formatting, and local browser integration remain UNVERIFIED. Native test job added to CI.
- First live validation: `cargo fmt --manifest-path native/system-monitor/Cargo.toml` then `cargo test --manifest-path native/system-monitor/Cargo.toml`, start helper, use browser Settings Connect, test Linux Firefox and Chromium HTTPS Pages, check CORS/loopback prompts, inspect 3-second updates and disconnect; verify browser-only mode after killing helper.

## UX8 — Browser-only automatic resource reporting (2026-10-28)

- The user rejected the UX7 native sysinfo helper because it required installation, token entry, and a continuously running program. UX8 **supersedes UX7**; do not deploy or require its native helper.
- Delete the UX7 `native/system-monitor` and `app/system-monitor.js` sources, UX7 tests, helper CI job, loopback polling, and token UI. No code attempts access to localhost.
- On page load and every two minutes while visible, automatically sample application memory with `performance.measureUserAgentSpecificMemory` where available under cross-origin isolation, or Chromium `performance.memory.usedJSHeapSize` as an explicitly labelled JS-heap-only fallback. Firefox may provide neither.
- Browser physical RAM is reported only when `navigator.deviceMemory` is available, as a **coarse/capped RAM class**, not exact physical or free RAM. In Firefox 158, physical RAM remains unavailable due to browser security/anti-fingerprinting restrictions. Do not invent RAM from CPU threads, WASM memory, JS heap, or disk quota.
- Remove optional manual physical RAM field, and ignore old saved `physicalRamGiB` preference. No installation, token, setup, file-size cap, or manual entry is required to run analyses.
- `navigator.hardwareConcurrency` gives logical processing capacity, not host CPU utilization; WASM science still uses one worker. Future PX1 will need a separate parallel WASM implementation.
- The existing 2,983-file folder reconnection and fixed sidebar remain unchanged.
- UX8 needs browser smoke tests in Firefox and Chromium, plus full cohort validation.

### UX8 validation and distribution
- Browser-only replacement archive: `4C_INSIGHT_UX8_BROWSER_ONLY_METRICS_20261028.tar.gz`.
- Source overlay installer was tested on UX7 reconstruction; SHA preflight, cleanup, syntax, tests, and `git diff --check` passed.
- JavaScript test suite: **45/45 passed**, including Firefox unavailable, Chromium approximate-class, and no-native-helper contract tests.
- No Rust scientific source or wasm-pack build changes. Rust parity and full browser/manual cohort runs remain unverified in this environment.
- Source versions in `app/index.html` and `app/main.js`: `0.21.0-ux8-browser-metrics`.

## UX9 — WASM memory measurements (2026-10-30)

User observation: Firefox 158 still shows "System RAM unavailable" in UX8. This is a real browser restriction. The user requires zero install and no extra local process. Do not attempt browser `sysinfo`, memory guessing, probing RAM through allocations, or installing the rejected UX7 native companion.

UX9 changes:
- The top information chip no longer shows the unavailable-physical-RAM message in Firefox.
- Settings prioritizes scientific WASM allocated linear-memory buffer in all browsers. The app reads `WebAssembly.Memory.buffer.byteLength` from the existing wasm-bindgen module's initialization exports inside `analysis-worker.js`.
- Worker emits validated `wasm-memory` samples during scientific progress and before successful completion events. Main UI shows allocated bytes and highest observed WASM linear-memory allocation per science worker.
- When an analysis worker is terminated, the last allocation is labelled released. This is not process RSS or actual live Rust heap usage; samples do not include JS/File buffers, worker overhead, OS RAM, or true free host RAM. Samples between scientific stages do not guarantee the true maximum process memory.
- Chromium `deviceMemory` is still displayed as a coarse class when available. Optional browser-app estimates are displayed only when supported, with separate labels.
- Firefox host RAM continues to be unavailable through website APIs. A small explanatory note remains in Settings instead of a repeated status warning.
- No native helper, token, browser permission request, file caps, Rayon change, or scientific-engine change. UX8 browser-only approach remains.
- Browser shell: `0.21.0-ux9-wasm-memory`; Rust scientific engine remains v0.21.0.

Validation in authoring environment:
- Source reconstructed from the uploaded archive and UX3/UX4/UX5/UX6/UX7/UX8 patches.
- JS tests 50/50 PASS (includes real compiled WASM module's memory-export smoke test).
- `node --check` on changed JS files PASS.
- Browser Firefox/Chromium interactive memory observation and real 2,983-file cohort stress test NOT performed; user must smoke-test.

Next: run actual Firefox smoke test with preflight, Compare Groups and worker recycle; check Settings and header, ensure no stale values. Benchmark real 2,983-file cohort; then work on streaming parser and bounded memory. Respect no file-size cap and single-worker fallback. PX1 threaded WASM remains a separate milestone.

## CI1 — Generated WASM artifact ordering (2026-10-30)

User provided a GitHub Actions `web-static` failure: UX9 test 48 imported `app/wasm/four_c_insight_wasm.js` in a fresh checkout before WASM output had been generated. Result: Node `ERR_MODULE_NOT_FOUND`; 49 passing, 1 failing. The failure is in test setup, not evidence of a scientific engine regression.

CI1 fix:
- In `app/tests/ux9-wasm-memory.test.mjs`, verify both wasm-pack output paths exist. Skip only the generated-module test when **both** artifacts are absent and no build is required. If only one exists, fail rather than silently skipping an incomplete build.
- Support `CFS_REQUIRE_WASM_ARTIFACTS=1`, which forces a failing test if generated output is missing. Use this mode after wasm-pack builds.
- In `.github/workflows/ci.yml`, keep `web-static` source-only tests and add a real wasm-pack build and forced generated-module test in the `wasm-bridge` job. This maintains build-output checking on pull requests.
- In `.github/workflows/pages.yml`, make the already post-build Node tests require the generated artifacts explicitly.
- No Rust science, browser resource API, input file limit, or WASM runtime changes.

Local simulation results: built module available: **50 passed, 0 failed**; source-only checkout: **49 passed, 1 intentionally skipped, 0 failed**; forced mode without output: **expected failure**. These checks use an existing compiled WASM module; live GitHub CI and fresh wasm-pack compilation remain unverified until a push.

After applying: run tests, stage and commit `.github/workflows/ci.yml`, `.github/workflows/pages.yml`, `app/tests/ux9-wasm-memory.test.mjs`, and the rolling docs. Push and confirm both `web-static` and `wasm-bridge` jobs, then the Pages deployment build. A source-only skip is expected; a compiled WASM test skip is not permitted.

## M1 — Browser gene/transcript XLSX merger (2026-10-30)

Implementation source patch prepared against the post-UX9/CI1 application, based on the user's earlier `cfs-expression-merger.tar.gz` engine. The source-only patch has **not** been compiled in Rust in this environment. It must not be described as scientifically validated until cargo and generated-WASM tests pass.

### Added Rust shared portable merge core

- `src/portable_merge.rs`, used by native root crate and `wasm/src/lib.rs`.
- Portable XLSX parsing with `calamine::Xlsx<Cursor<&[u8]>>`, one input workbook per operation.
- Strict first-sorted-workbook feature universe, duplicate canonical entity check, normalized unique sample names.
- Same gene/transcript metrics and common output formatting as original `merge_wide()`.
- Packed 9-byte measurement cells and row-batch TSV renderer that avoids retaining every sample matrix in WASM.
- WASM exports `merge_begin_json`, `merge_sample_packed`, `merge_last_sample_json`, `merge_header_tsv`, `merge_render_rows_tsv`, `merge_reset`.
- `tests/merge_portable_parity.rs` compares native `merge_wide()` TSV output against the portable code using real fixture XLSX files. BOTH adjacent/grouped orders and gene/transcript presets are covered, plus malformed configuration checks.

### Browser workflow

- `app/merge-helpers.js`, `app/merge-worker.js`.
- Project page: merge gene or transcript workbooks, select available metrics, select column order, progress, Stop, Download TSV, Restore browser-generated tables.
- Browser worker reads one XLSX at a time and writes temporary compact sample matrices to origin-private file system (OPFS). Output TSV assembled in memory-bounded row slices (2 MiB packed input target), stored in OPFS and returned as a native browser `File`.
- Merged output is added to project inventory; Compare Groups and Longitudinal merged-table selections are updated automatically. Existing comparison selections and model configuration are preserved when adding a merged table.
- No upload, native helper, new input size cap, or Rayon introduced. Scientific engine stays v0.21.0; capability is M1.
- Full workbook contents still must fit in memory **one workbook at a time** during XLSX parsing. Downstream scientific analysis still uses `file.arrayBuffer()` on the full merged TSV. This milestone does **not** claim fully streaming end-to-end analysis.
- Temporary OPFS runs delete on success/error. On browsers that support Web Locks, the next merge runs under an exclusive lock and removes abandoned scratch folders from terminated merges. On browsers without Web Locks, forced cancellation can leave scratch folders; cleanup remains a future task.
- Generated merged output is saved in browser site storage, not the source folder. Download TSV for durable storage. A saved `.4cstate` contains only its reference; restore generated tables on the same browser profile, or use the file picker with the exported TSV.

### Validation performed in creation environment

- JS syntax checks: PASS.
- `node --test app/tests/*.test.mjs`: 48 pass, 2 skip (generated-WASM tests), 0 fail on clean source-only checkout.
- Patch application and `git diff --check`: PASS.
- Rust compilation / native merger parity / generated-WASM integration / Firefox OPFS / 300+ workbook peak memory: **NOT RUN** (Rust toolchain absent from creation environment).

### Required release gate

```
cargo fmt --all
cargo fmt --manifest-path wasm/Cargo.toml --all
cargo check --locked
cargo test --locked --test merge_portable_parity
cargo test --manifest-path wasm/Cargo.toml --locked
cargo check --manifest-path wasm/Cargo.toml --locked --target wasm32-unknown-unknown
wasm-pack build wasm --target web --out-dir ../app/wasm --out-name four_c_insight_wasm --release
CFS_REQUIRE_WASM_ARTIFACTS=1 node --test app/tests/*.test.mjs
```

Run local Firefox/Chrome UI with the 2 gene fixture XLSX files and one transcript XLSX fixture. Confirm merge output data and provenance, downstream comparison selection, downloadable TSV, and restore after reload. Test at least one large cohort (300+ workbooks), measure OPFS use, browser peak memory, and result parity before deploying broadly.
