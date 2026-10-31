import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'
import { measureWasmAllocation, validateWasmMemoryReport } from '../wasm-memory.js'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')

test('measures WASM linear-memory allocation without host RAM APIs', () => {
  const mem = new WebAssembly.Memory({ initial: 3 })
  assert.deepEqual(measureWasmAllocation(mem), { allocatedBytes: 3 * 65536, peakAllocatedBytes: 3 * 65536 })
  mem.grow(2)
  assert.deepEqual(measureWasmAllocation(mem, 3 * 65536), { allocatedBytes: 5 * 65536, peakAllocatedBytes: 5 * 65536 })
  assert.equal(measureWasmAllocation(null), null)
  assert.equal(measureWasmAllocation({ buffer: { byteLength: NaN } }), null)
  assert.deepEqual(measureWasmAllocation(mem, 10 * 65536), { allocatedBytes: 5 * 65536, peakAllocatedBytes: 10 * 65536 })
})

test('validates WASM memory reports before display', () => {
  assert.deepEqual(validateWasmMemoryReport({ allocatedBytes: 262144, peakAllocatedBytes: 524288 }), { allocatedBytes: 262144, peakAllocatedBytes: 524288 })
  for (const input of [null, {}, { allocatedBytes: -1, peakAllocatedBytes: 0 }, { allocatedBytes: 500, peakAllocatedBytes: 100 }, { allocatedBytes: 1.5, peakAllocatedBytes: 2 }, { allocatedBytes: Infinity, peakAllocatedBytes: Infinity }]) {
    assert.equal(validateWasmMemoryReport(input), null)
  }
})

// A fresh Git checkout contains source only. wasm-pack creates both files during
// the WASM build. Run this test after the build in CI and on GitHub Pages.
const generatedJsUrl = new URL('../wasm/four_c_insight_wasm.js', import.meta.url)
const generatedWasmUrl = new URL('../wasm/four_c_insight_wasm_bg.wasm', import.meta.url)
async function exists(url) {
  try { await access(url); return true } catch (error) {
    if (error?.code === 'ENOENT') return false
    throw error
  }
}
const [hasGeneratedJs, hasGeneratedWasm] = await Promise.all([
  exists(generatedJsUrl), exists(generatedWasmUrl)
])
const requireGeneratedWasm = process.env.CFS_REQUIRE_WASM_ARTIFACTS === '1'
const sourceOnlyCheckout = !hasGeneratedJs && !hasGeneratedWasm

test('deployed wasm-bindgen init exposes the actual WASM memory buffer', {
  skip: sourceOnlyCheckout && !requireGeneratedWasm
    ? 'WASM output is generated during the build (not checked into Git)'
    : false
}, async () => {
  assert.ok(hasGeneratedJs && hasGeneratedWasm,
    'Generated WASM artifacts are missing. Run wasm-pack build before this test.')
  const { default: init } = await import(generatedJsUrl)
  const binary = await readFile(generatedWasmUrl)
  const exports = await init({ module_or_path: binary })
  assert.ok(exports.memory instanceof WebAssembly.Memory)
  const snapshot = measureWasmAllocation(exports.memory)
  assert.ok(snapshot.allocatedBytes >= 65536)
})

test('science worker publishes memory samples before final responses', () => {
  assert.match(worker, /const exports = await module\.default\(\)/)
  assert.match(worker, /wasmMemory = exports\?\.memory instanceof WebAssembly\.Memory/)
  assert.match(worker, /type: 'wasm-memory'/)
  for (const type of ['preflight-result', 'metadata-inspection-result', 'expression-qc-result', 'expression-comparison-result', 'longitudinal-expression-result', 'significant-hit-result']) {
    assert.match(worker, new RegExp(`postWasmMemory\\(request\\.requestId, true\\)\\s+self\\.postMessage\\(\\{\\s+type: '${type}'`))
  }
  assert.match(main, /if \(event\.data\.type === 'wasm-memory'\)/)
  assert.match(main, /validateWasmMemoryReport\(event\.data\)/)
  assert.match(main, /wasmMemorySample = \{ \.\.\.state\.wasmMemorySample, active: false \}/)
})

test('Firefox display prioritizes engine memory rather than an unavailable-RAM warning', () => {
  assert.match(main, /id="wasm-memory-status"/)
  assert.match(main, /Scientific WASM memory/)
  assert.match(main, /info\.deviceMemoryGiB !== null \?/)
  assert.doesNotMatch(main, /\$\{escapeHtml\(info\.memory\)\}.*WASM science: 1 worker/)
  assert.match(main, /WASM memory is measured from the science engine, not total\/free host RAM/)
  assert.match(html, /0\.21\.0-m1-merge/)
  assert.doesNotMatch(main, /nativeMonitor|connect-native-monitor|resource-ram/)
})
