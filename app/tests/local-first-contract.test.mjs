import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const clientFiles = [
  new URL('../main.js', import.meta.url),
  new URL('../analysis-worker.js', import.meta.url),
  new URL('../save.js', import.meta.url),
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
