# Continue 4C Insight development — UX3 resource safeguards

You are continuing `singjc/4CInsight`, published at https://singjc.github.io/4CInsight/.
Use ASD-STE100. Keep `docs/4C_INSIGHT_HANDOFF_CURRENT.md` and `docs/4C_INSIGHT_CONTINUATION_PROMPT.md` updated after substantive changes. Always provide `git add` and `git commit` commands.

## Current state

Scientific engine v0.21.0, W1–W5. UX2 was developed and tested. The current UX3 overlay adds Settings and bounded browser operations. Source JS tests: 29/29 PASS. **Deployment and real-cohort browser testing are not verified.**

Read `docs/4C_INSIGHT_HANDOFF_CURRENT.md` for detailed architecture, progress, changes, limitations, and acceptance tasks.

## UX3 functionality

- Current browser science uses one Web Worker; it does NOT use Rayon. Native Rust uses bounded `std::thread`.
- CPU target: 85%, future PX1 ceiling 8 workers. Only planning for a threaded-WASM milestone; not yet an active CPU or Rayon limit.
- Local settings: per-file input cap 256 MiB; input working budget 1.5 GiB; manually entered RAM optional; 2,000 maximum volcano points; recycle WASM Worker after analysis; Stop operation.
- Limits run before loading local input into WASM and reduce DOM/retained memory. They are NOT hard physical RAM limits.
- Large scientific results remain complete; plot display is sampled but tables and exports retain all tested features.
- History avoids an unnecessary pre-IndexedDB JSON deep copy and removes a long-lived unused result reference. Large plot bounds are computed with loops instead of argument-spreading arrays.

## First tasks

1. Review and apply UX3 source overlay. Check intended Git repository and source version.
2. Validate JS and tests, inspect git diff, and do a browser smoke test (Settings, manual Linux memory entry, state load, History load, Stop, large volcano, second run).
3. Determine whether crash occurs on application state restore, IndexedDB History view, or actual WASM analysis. Do not assume it is a thread issue.
4. Record memory/timing on the user's real 322–323-sample cohort. If still failing, focus on XLSX uncompressed cells, WASM allocation growth, result serialization, and History snapshot size.
5. Only after validation, stage/commit/push the source overlay and confirm Pages deployment.
6. PX1 true threaded WASM is separate work. Add shared-memory worker pool with limits, fallback, parity, and benchmarks.

## Checks

```bash
git diff --check
node --check app/main.js
node --check app/analysis-worker.js
node --check app/resource-controls.js
node --check app/persistence.js
node --check app/docs-content.js
node --check app/coi-serviceworker.js
node --test app/tests/*.test.mjs
```

## Commit once validated

```bash
git add app/index.html app/main.js app/analysis-worker.js app/resource-controls.js app/styles.css app/tests/ux3-resource-controls.test.mjs .github/workflows/pages.yml docs/4C_INSIGHT_HANDOFF_CURRENT.md docs/4C_INSIGHT_CONTINUATION_PROMPT.md
git diff --cached --check
git commit -m "Add browser resource controls and bounded plot rendering"
git push origin main
```
