# 4C Insight web/WASM development

## W1 local-first foundation

The old server/upload client is retired. `app/` is a static application and `wasm/` is the browser bridge.

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

Selecting a file does not upload it. Browser `File` objects stay in the page/worker context. W1 uses filename metadata only. Future parsers will transfer or stream bytes to the worker as needed.

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
