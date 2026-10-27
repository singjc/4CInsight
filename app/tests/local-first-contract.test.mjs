import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const clientFiles = [
  new URL('../main.js', import.meta.url),
  new URL('../analysis-worker.js', import.meta.url),
  new URL('../save.js', import.meta.url),
  new URL('../persistence.js', import.meta.url),
]

test('primary browser client has no analysis API endpoint', async () => {
  for (const url of clientFiles) {
    const source = await readFile(url, 'utf8')
    assert.equal(source.includes('/api/'), false, `${url.pathname} contains an /api/ dependency`)
    assert.equal(source.includes('recovery_token'), false, `${url.pathname} contains legacy project credentials`)
    assert.equal(source.includes('cfs-server'), false, `${url.pathname} contains the retired server name`)
  }
})

test('static entry point is branded 4C Insight', async () => {
  const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')
  assert.match(html, /<title>4C Insight<\/title>/)
  assert.doesNotMatch(html, /Cohort Explorer|cfs-expression-merger/i)
})



test('browser shell cannot silently reuse a stale workflow bundle', async () => {
  const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  const worker = await readFile(new URL('../coi-serviceworker.js', import.meta.url), 'utf8')
  assert.match(html, /styles\.css\?v=/)
  assert.match(html, /main\.js\?v=/)
  assert.match(main, /coi-serviceworker\.js/)
  assert.match(main, /searchParams\.set\('v',/)
  assert.match(worker, /cache:\s*['"]no-store['"]/)
})

test('W2 expression QC stays inside the worker and Rust/WASM bridge', async () => {
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
  assert.match(main, /run-expression-qc/)
  assert.match(worker, /summarize_expression_json/)
  assert.match(worker, /inspect_metadata_json/)
  assert.match(worker, /await file\.arrayBuffer\(\)/)
  assert.equal(worker.includes('fetch('), false, 'analysis worker must not send selected data over the network')
})

test('W3 corrected group comparison stays inside the worker and shared Rust/WASM engine', async () => {
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
  assert.match(main, /run-expression-comparison/)
  assert.match(main, /metadata-column-values/)
  assert.match(worker, /run_expression_comparison_json/)
  assert.match(worker, /metadata_column_values_json/)
  assert.match(main, /Custom metadata exclusion/)
  assert.match(main, /Patient\/subject column for repeated measures/)
  assert.match(main, /Finish or disable the optional exclusion/)
  assert.match(main, /Needs input/)
  assert.equal(worker.includes('fetch('), false, 'W3 analysis worker must not send selected data over the network')
})

test('W4 longitudinal expression signature stays inside the worker and Rust/WASM engine', async () => {
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
  assert.match(main, /run-longitudinal-expression/)
  assert.match(main, /Observation \/ longitudinal sample ID/)
  assert.match(main, /Patient \/ person ID/)
  assert.match(main, /Patient identity AUC/)
  assert.match(main, /Nearest-neighbour accuracy/)
  assert.match(worker, /run_expression_longitudinal_json/)
  assert.equal(worker.includes('fetch('), false, 'W4 analysis worker must not send selected data over the network')
})

test('W5 significant-hit structure stays inside the worker and shared Rust/WASM engine', async () => {
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
  assert.match(main, /run-significant-hits/)
  assert.match(main, /Post-selection interpretation/)
  assert.match(main, /Significant-hit PCA/)
  assert.match(main, /Clustered significant-hit heatmap/)
  assert.match(main, /Descriptive, not validation/)
  assert.match(worker, /run_expression_hit_structure_json/)
  assert.equal(worker.includes('fetch('), false, 'W5 analysis worker must not send selected data over the network')
})
