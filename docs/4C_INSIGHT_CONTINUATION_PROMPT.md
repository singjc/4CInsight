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
