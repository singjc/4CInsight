# 4C Insight web/WASM development

## W2 local-first expression foundation

The old server/upload client is retired. `app/` is a static application and `wasm/` is the browser bridge. W2 adds metadata and gene/transcript expression QC from local file bytes.

### Build WASM

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
wasm-pack build wasm \
  --target web \
  --out-dir ../app/wasm \
  --out-name four_c_insight_wasm \
  --release
```

### Run locally

Serve the repository `app/` directory with a local HTTP server. No JavaScript package installation is required.

The worker attempts to load `/wasm/four_c_insight_wasm.js`. If it is not present during frontend-only development, it uses the matching JavaScript filename classifier and clearly reports that fallback in the UI.

### Production build

There is no JavaScript bundling step. The GitHub Pages workflow builds the Rust/WASM package into `app/wasm/` and uploads the static `app/` directory.

## Data boundary

Selecting a file does not upload it. Browser `File` objects stay in the page/worker context. W2 sends one selected metadata or expression file at a time to the worker, where `arrayBuffer()` supplies bytes to the Rust/WASM parser. The worker does not use `fetch` for selected data.

Do not add:

- `/api` analysis calls;
- project/recovery tokens;
- server-side object IDs;
- temporary remote storage;
- hidden telemetry.

## WASM migration checklist for each scientific workflow

1. Separate native filesystem access from the calculation.
2. Add a byte/reader or parsed-data entry point.
3. Keep one Rust mathematical implementation.
4. Add deterministic fixture tests.
5. Run native and WASM outputs on the same fixture.
6. Document numerical tolerance and any browser-specific limitation.
7. Add progress messages that correspond to real scientific stages.
8. Add memory measurements for large inputs.
9. Enable the corresponding UI action only after parity passes.

## GitHub Pages

The site is deployed directly from `app/` through GitHub Actions. All application URLs are relative so project Pages, user Pages, and custom domains can use the same static tree.

## W2 parity validation

After applying W2, regenerate both Cargo lockfiles because the package version changes and the WASM crate gains `anyhow`, `calamine`, and `csv`:

```bash
cargo generate-lockfile --offline
( cd wasm && cargo generate-lockfile --offline )
```

Then run:

```bash
cargo fmt --all
git diff --check
cargo check --locked
cargo test --locked
cargo check --manifest-path wasm/Cargo.toml --locked
cargo test --manifest-path wasm/Cargo.toml --locked
node --test app/tests/*.test.mjs
```

The root test suite includes `tests/w2_portable_parity.rs`. It compares metadata parsing and expression distribution statistics from the byte-oriented parser against the established native path reader on fixed fixtures.

## W3 metadata-defined expression comparison

W3 enables the **Compare Groups** workflow in the local browser application. The browser UI only manages file handles, metadata role selection, progress, rendering, and export. The scientific calculations remain in shared Rust.

The browser sends the selected metadata file and one selected merged expression table at a time to the Web Worker. The worker converts the local `File` to bytes and calls the Rust/WASM bridge. No selected cohort data are sent with `fetch` or an analysis API.

The user selects these metadata roles independently:

- expression/sample ID column;
- grouping column and one or more Case values;
- one or more Control values;
- optional patient/subject column for repeated measures;
- optional model covariates.

Two independent metadata exclusion rules are available. One is labelled for reviewed QC status and the other is a general custom metadata exclusion. Each rule uses a user-selected column and one or more observed values. Matching rows are removed before cohort construction and model fitting.

Gene comparison uses `Raw Count` columns. Transcript comparison uses `Total Reads` columns. Both use the established corrected-expression engine: positive-count median-ratio normalization, the corrected negative-binomial group model, BH adjustment, global PCA, and average-linkage hierarchical clustering. Transcript comparison also runs the existing transcript-usage screen.

W3 adds a byte-oriented entry point to `src/corrected.rs`; it is a different input source for the same model code, not a second statistical implementation.

### W3 parity validation

After applying W3, regenerate the root and WASM lockfiles because the package version changes:

```bash
cargo generate-lockfile --offline
(
  cd wasm
  cargo generate-lockfile --offline
)
```

Then validate:

```bash
cargo fmt --all
git diff --check
cargo check --locked
cargo test --locked
cargo test --test w3_expression_compare_parity --locked
cargo check --manifest-path wasm/Cargo.toml --locked
cargo test --manifest-path wasm/Cargo.toml --locked
cargo check --manifest-path wasm/Cargo.toml --locked --target wasm32-unknown-unknown
node --test app/tests/*.test.mjs
```

`tests/w3_expression_compare_parity.rs` compares native path input and byte input for gene and transcript corrected analyses. It covers size factors, adjusted feature tests, PCA coordinates, dendrogram structure, transcript DTU output, and metadata-defined exclusion before model fitting.


## W4 longitudinal parity

W4 adds `src/portable_longitudinal.rs` and a byte-based expression-distance entry point in `src/corrected.rs`. `tests/w4_longitudinal_parity.rs` runs the same fixed metadata and merged gene/transcript tables through the native filesystem longitudinal workflow and the byte/WASM orchestration, then compares patient counts, identity metrics, pairwise distances, and dendrogram structure.

The browser worker reads the selected metadata file once, then one selected merged expression table per enabled modality. It does not load the original per-sample expression workbooks for longitudinal analysis.


## W5 significant-hit parity

W5 adds `src/hit_structure_core.rs` and `src/portable_hit_structure.rs`. The native significant-hit workflow now uses the same reader-agnostic core that the WASM path uses. `tests/w5_hit_structure_parity.rs` runs fixed gene and transcript fixtures through the native filesystem workflow and the browser-safe byte workflow, then compares selected features, heatmap z scores, PCA coordinates, PERMANOVA values, silhouette, nearest-neighbour coherence, and dendrogram structure. The test also covers transcript **Total Reads** reconstruction so the browser and native transcript hit matrices use the same source metric as W3.
