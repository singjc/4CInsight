import assert from 'node:assert/strict';
import { access, readFile } from 'node:fs/promises';
import { test } from 'node:test';

const rustPath = new URL('../../src/corrected.rs', import.meta.url);
const wasmPath = new URL('../../wasm/src/lib.rs', import.meta.url);

test('repertoire parser uses native clock on host and browser monotonic clock on WASM', async () => {
  const rust = await readFile(rustPath, 'utf8');
  assert.match(rust, /#\[cfg\(not\(target_arch = "wasm32"\)\)\]\s*use std::time::Instant/);
  assert.match(rust, /#\[cfg\(target_arch = "wasm32"\)\]\s*use wasm_monotonic_clock::Instant/);
  assert.match(rust, /js_namespace = performance, js_name = now/);
  assert.match(rust, /Duration::from_secs_f64\(ms \/ 1000\.0\)/);
  assert.doesNotMatch(rust, /use std::time::\{Duration, Instant\}/);
  assert.ok((rust.match(/Instant::now\(\)/g) ?? []).length >= 10);
});

test('multiomics releases RefCell borrow before XLSX parsing and restores after recoverable errors', async () => {
  const wasm = await readFile(wasmPath, 'utf8');
  const fn = wasm.match(/pub fn multiomics_add_sample_json\s*\([\s\S]*?(?=#\[wasm_bindgen\]\s*pub fn multiomics_finish_json)/)?.[0];
  assert.ok(fn, 'missing multiomics_add_sample_json entrypoint');
  assert.match(fn, /let mut session = MULTIOMICS_SESSION\.with/);
  assert.match(fn, /\.borrow_mut\(\)\s*\.take\(\)/);
  assert.match(fn, /repertoire\s*\.\s*add_bytes\s*\(/);
  assert.match(fn, /let outcome = \(\|\| -> Result<String, JsValue>/);
  assert.match(fn, /\*slot = Some\(session\)/);
  assert.match(fn, /\n\s*outcome\s*\n/);
  assert.doesNotMatch(fn, /let mut borrow = cell\.borrow_mut\(\)/);
});

const mustBuild = process.env.CFS_REQUIRE_WASM_ARTIFACTS === '1';
test('compiled WASM parses BCR, TCR alpha, and TCR beta without unsupported clock', {
  skip: mustBuild ? false : 'Requires a fresh wasm-pack build and CFS_REQUIRE_WASM_ARTIFACTS=1',
}, async () => {
  const js = new URL('../wasm/four_c_insight_wasm.js', import.meta.url);
  const bg = new URL('../wasm/four_c_insight_wasm_bg.wasm', import.meta.url);
  await access(js);
  await access(bg);
  const wasm = await import(js.href);
  await wasm.default({ module_or_path: await readFile(bg) });
  const labels = Array.from({length: 8}, (_, index) => {
    const i = index + 1;
    return `S${i}\t${[1, 2, 5, 6].includes(i) ? 'Case' : 'Control'}\tP${i}`;
  });
  const metadata = new TextEncoder().encode(['sample_id\tPhenotype\tsubject_id', ...labels, ''].join('\n'));
  const config = JSON.stringify({
    kind: 'gene-expression', metadataIdColumn: 'sample_id',
    groupColumn: 'Phenotype', caseValues: ['Case'], controlValues: ['Control'],
    comparisonName: 'Case vs Control', subjectColumn: 'subject_id',
    covariates: [], exclusions: [], minCount: 10,
    minFractionSamples: 0.2, permutations: 11,
  });
  for (const [kind, suffix] of [
    ['bcr', 'bcr_heavy_chain_report'],
    ['tcr-alpha', 'tcr_alpha_clone_report'],
    ['tcr-beta', 'tcr_beta_clone_report'],
  ]) {
    assert.equal(typeof wasm.multiomics_begin_json, 'function');
    wasm.multiomics_begin_json('synthetic.tsv', metadata, config, kind,
      JSON.stringify({ permutations: 11, rarefactionReps: 1, rarefactionCap: 100 }));
    try {
      if (kind === 'bcr') {
        assert.throws(() => wasm.multiomics_add_sample_json('bad.xlsx', 'S1', Uint8Array.of(1, 2, 3)));
      }
      for (let i = 1; i <= 8; i++) {
        const file = `S${i}_${suffix}.xlsx`;
        const bytes = await readFile(new URL(`../../tests/fixtures/m2_multiomics/${file}`, import.meta.url));
        const response = JSON.parse(wasm.multiomics_add_sample_json(file, `S${i}`, bytes));
        assert.equal(response.status, 'added', file);
      }
      const result = JSON.parse(wasm.multiomics_finish_json());
      assert.equal(result.analysis.files_used, 8, kind);
      assert.equal(result.analysis.sample_summaries.length, 8, kind);
    } finally {
      wasm.multiomics_abort();
    }
  }
});
