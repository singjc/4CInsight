import test from 'node:test'
import assert from 'node:assert/strict'
import { access, readFile } from 'node:fs/promises'
const js = new URL('../wasm/four_c_insight_wasm.js', import.meta.url)
const binary = new URL('../wasm/four_c_insight_wasm_bg.wasm', import.meta.url)
async function exists(url) { try { await access(url); return true } catch (error) { if (error?.code === 'ENOENT') return false; throw error } }
const present = await Promise.all([exists(js), exists(binary)])
const requireBuild = process.env.CFS_REQUIRE_WASM_ARTIFACTS === '1'
test('M1 built WASM merges gene workbooks into an analysis-compatible TSV', { skip: !requireBuild ? 'Built-WASM integration runs after wasm-pack in CI' : false }, async () => {
  assert.ok(present.every(Boolean), 'WASM build artifacts required')
  const { default: init, merge_begin_json, merge_sample_packed, merge_last_sample_json, merge_header_tsv, merge_render_rows_tsv, merge_reset } = await import(js)
  assert.equal(typeof merge_begin_json, 'function', 'Rebuild WASM with M1 merge exports')
  await init({ module_or_path: await readFile(binary) })
  const files = [
    'S1_tumor_rna_gene_expression_report.xlsx',
    'S2_tumor_rna_gene_expression_report.xlsx',
  ]
  const bytes = await Promise.all(files.map(name => readFile(new URL(`../../tests/fixtures/${name}`, import.meta.url))))
  const plan = JSON.parse(merge_begin_json(bytes[0], '"gene"', '["raw-count","tpm"]', '"grouped-by-measurement"', JSON.stringify(files)))
  assert.ok(plan.rows > 0)
  assert.equal(plan.samples, 2)
  assert.equal(plan.measurements, 2)
  try {
    const samples = bytes.map(source => {
      const packed = merge_sample_packed(source)
      assert.equal(packed.length, plan.bytesPerSample)
      assert.ok(Number.isInteger(JSON.parse(merge_last_sample_json()).missingRows))
      return packed
    })
    let result = merge_header_tsv()
    for (let at = 0; at < plan.rows; at += 31) {
      const count = Math.min(31, plan.rows - at)
      const stride = plan.measurements * 9
      const input = new Uint8Array(count * stride * 2)
      for (let sample = 0; sample < 2; sample++) input.set(samples[sample].subarray(at * stride, (at + count) * stride), sample * count * stride)
      result += merge_render_rows_tsv(at, count, input)
    }
    const lines = result.trimEnd().split('\n')
    assert.equal(lines.length, plan.rows + 1)
    const cols = lines[0].split('\t')
    assert.equal(cols.length, 6)
    assert.match(cols[2], /S1 Raw Count/)
    assert.match(cols[3], /S2 Raw Count/)
    assert.match(cols[4], /S1 TPM/)
    assert.match(cols[5], /S2 TPM/)
  } finally { merge_reset() }
})
