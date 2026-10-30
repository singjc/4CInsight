# Continue 4C Insight development — UX4 SVG branding

Project: `singjc/4CInsight` at https://singjc.github.io/4CInsight/.
Use ASD-STE100. Always update `docs/4C_INSIGHT_HANDOFF_CURRENT.md` and this continuation prompt after substantial work. Always supply complete `git add`, `git commit`, and `git push` commands.

## Current state

- Scientific engine v0.21.0, W1–W5 complete. UX2 has scientific plots/tables/docs.
- User confirmed UX3 commit `a26ff40` pushed to `main` on 2026-10-28. UX3 resource settings are in that commit; Pages status is not independently verified.
- UX4 source overlay moves the original `4C_Insight_logo_practical_trace_balanced_tight_margin.svg` to `app/assets/4c-insight-logo.svg`, retaining exact bytes. The header uses the image instead of the old 4C mark, text title, and subtitle.
- Header CSS: 96 px desktop, 80 px logo; 64 px logo below 720 px. Relative SVG path for Pages, accessible `h1`/`alt`. System information chip retained.
- No Rust/WASM rebuild needed for local UX4 editing; GitHub Actions rebuilds WASM on Pages deployment.
- UX4 validation: JS 32/32 PASS, JS syntax PASS, clean-tree installer PASS (Git reports the SVG rename as 100%). Interactive visual check and deployment are pending. UX4 is not committed/deployed until the user validates and pushes it.

## First tasks

1. Apply UX4 package to a clean UX3 `main` checkout. Confirm root SVG and verify moved asset matches source bytes.
2. Run `git diff --check`, all JS syntax checks, and `node --test app/tests/*.test.mjs`.
3. Visually check desktop/mobile header, image display, contrast, aspect ratio, and system chip, including Firefox on Linux.
4. Stage deleted root SVG and new SVG path, plus HTML/CSS/test/docs. Commit and push. Confirm Pages asset is accessible.
5. Then return to resource/memory real-cohort validation, and PX1 threaded WASM with parity and benchmarks.

## Validation commands

```bash
git diff --check
node --check app/main.js
node --check app/analysis-worker.js
node --check app/resource-controls.js
node --check app/docs-content.js
node --check app/persistence.js
node --check app/coi-serviceworker.js
node --test app/tests/*.test.mjs
```

## Commit after visual validation

```bash
git add -- app/assets/4c-insight-logo.svg app/index.html app/styles.css app/tests/ux4-branding.test.mjs docs/4C_INSIGHT_HANDOFF_CURRENT.md docs/4C_INSIGHT_CONTINUATION_PROMPT.md
git diff --cached --check
git diff --cached --stat
git commit -m "Use project SVG logo in application header"
git push origin main
```

## UX4 recovery state
The SVG was committed in 99c382b at app/assets/4c-insight-logo.svg.
Do not rerun the original UX4 installer.
The remaining UX4 changes use the existing logo.
Verify the browser header before deployment.

## UX5: Remove hard local-file thresholds and correct RAM reporting (2026-10-28)

User confirmed UX4 SVG branding and showed UX3 blocking a large transcript TSV because estimated copies exceed the default 1.5 GiB budget. Do not enforce fixed file-size or heuristic input-copy limits: large local cohort inputs are expected.

Prepared UX5 patch (`0.21.0-ux5-unlimited-inputs`) removes both `maxFileMiB` and `workingBudgetGiB` from resource normalization, UI and WASM-worker preflight. Retains basic File metadata validation, CPU settings (still future PX1, not Rayon), optional manual total RAM, plot-point cap, and worker lifecycle cleanup. Existing old Settings are migrated by dropping retired keys.

Added a button to measure app memory only where APIs support it. Distinguish app estimate, JS heap usage, approximate browser device-memory class, manual total RAM and system RAM unavailable. Web pages cannot call Linux sysinfo for physical/free RAM. Firefox may not expose memory APIs. No native companion added.

Code tests: 34/34 JS PASS on reconstructed UX4 tree; live full-cohort load/browser memory use NOT tested. The existing 10 MiB limit on separate portable state JSON remains; scientific expression files have no arbitrary threshold. `File.arrayBuffer()` still loads the full file, so large inputs can exhaust browser/WASM memory; investigate streaming when necessary.

Next: apply UX5 to the already-logo-integrated checkout (no SVG move). Run validation, commit and push. Confirm large transcript input no longer fails at UX3 guard; benchmark full cohort on Linux and test browser-specific memory diagnostics. Continue to PX1 only when single-worker baseline stable.

## Continuation after UX6 (2026-10-28)

Current scientific engine: v0.21.0. Latest browser milestone UX6. Prior patches UX3, UX4 and UX5 are prerequisites. Repo `~/Documents/github/cfs-expression-merger` remote `singjc/4CInsight`.

User findings: portable Save State references 2,983 files but cannot grant file access on load. UX6 adds explicit one-folder reconnect and conservative matching; previously missing status at bottom of sidebar fixed by desktop fixed-position sidebar plus internally scrollable nav. Exact RAM remains unavailable to Firefox/GitHub Pages; manual value and browser app-memory measurements are separate. Do not claim sysinfo inside WASM provides Linux total RAM. Do not add input file caps. Do not alter scientific models.

UX6 verification: 41/41 JS tests pass against reconstructed UX5 source. Browser visual and real 2,983-file restore remain to be tested by user. Package `4C_INSIGHT_UX6_RECONNECT_FIXED_SIDEBAR_20261028.tar.gz` contains patch, installer and docs addenda. Use verified package SHA and apply script. Run `git diff --check`, `node --check app/main.js`, `node --check app/state-reconnect.js`, `node --test app/tests/*.test.mjs`; include exact git add/commit commands for changed source and docs. Do not re-move SVG; it is already committed under `app/assets/4c-insight-logo.svg`.

Next: review user's real-cohort smoke, especially path matching and restored metadata selection. If user requires zero-click reloads, investigate permission-aware IndexedDB directory handles for Chromium, but not possible for arbitrary JSON paths and not assumed supported by Firefox. Consider opt-in local native companion for physical/available Linux RAM; confirm security and browser transport first. Then prioritize large-file streaming and PX1 threaded WASM when measured benefit and parity are established.


## Continue after UX7: native host monitor

UX7 added an optional native Rust sysinfo companion. Folder: `native/system-monitor` (standalone Rust crate). New JS module: `app/system-monitor.js`; current shell `0.21.0-ux7-native-monitor`. No science engine change.

47/47 JS tests passed on reconstructed source. Native compilation, rustfmt, and real browser↔localhost connection were NOT performed in this authoring environment because cargo/rustc were not available. Next: obtain user local `cargo fmt/check/test` output and Firefox/Chromium smoke test after launching helper, then fix issues and update rolling handoff. Do not claim automatic host memory is available when helper is not connected. No Rayon in WASM. Include full runnable shell validation and git add/commit commands for source overlays.

## Current priority — UX8 browser-only metrics replaces UX7 helper

The user rejected the UX7 native sysinfo service. UX8 removes it. Do not suggest installing or starting a local helper as a normal requirement. No browser technology can recover exact free physical RAM on Firefox 158 without native privileges; show unavailable rather than fabricate it. Chromium may expose a coarse RAM class, plus a distinct application-memory estimate.

Validate UX8 in Firefox and Chromium, especially automatic sampling and readable source labels. Check that the complete 2,983-file state reconnection, fixed sidebar, analysis workflow, and memory cleanup still work. Run `node --test app/tests/*.test.mjs`, `git diff --check`, `cargo test --locked`, and WASM parity tests. Continue streaming input work and separately PX1 true threaded WASM. Update handoff each milestone.

UX8 package smoke: 45/45 JS tests PASS; installer verifies UX7 source hashes and removes rejected native monitor code. Browser Firefox and Chromium smoke tests still required. There is no true physical/free-RAM reading in Firefox; never describe browser heap or RAM-class as available host RAM.
