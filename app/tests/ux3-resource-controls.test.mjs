import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import {
  DEFAULT_RESOURCES,
  normalizeResources,
  suggestedScienceThreads,
  validateLocalInputFile,
  measureAppMemory,
  thinPlotPoints,
} from '../resource-controls.js'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')

test('CPU budget is for future threaded-WASM, one browser science worker today', () => {
  assert.equal(DEFAULT_RESOURCES.cpuPercent, 85)
  assert.equal(suggestedScienceThreads(20), 8)
  assert.equal(suggestedScienceThreads(8), 6)
  assert.equal(suggestedScienceThreads(2), 1)
  assert.equal(suggestedScienceThreads(1), 1)
  assert.match(main, /Current single-worker WASM cannot use this thread budget/)
  assert.match(main, /WASM science: 1 worker/)
})

test('settings discard old hard input caps and recover invalid resource values', () => {
  const r = normalizeResources({ cpuPercent: 800, maxThreads: 1e8, maxFileMiB: 16, workingBudgetGiB: 0.25, physicalRamGiB: -4, maxPlotPoints: 1e9 })
  assert.deepEqual([r.cpuPercent, r.maxThreads, r.maxPlotPoints], [90, 32, 6000])
  assert.equal(Object.hasOwn(r, 'maxFileMiB'), false)
  assert.equal(Object.hasOwn(r, 'workingBudgetGiB'), false)
  assert.deepEqual(normalizeResources(null), { ...DEFAULT_RESOURCES })
  assert.equal(Object.hasOwn(normalizeResources({ physicalRamGiB: '32' }), 'physicalRamGiB'), false)
})

test('large local inputs do not fail a size/estimated-copy-pressure threshold', () => {
  validateLocalInputFile({ name: 'small.tsv', size: 4 * 1024 * 1024 })
  validateLocalInputFile({ name: 'large.tsv', size: 20 * 1024 * 1024 * 1024 })
  assert.throws(() => validateLocalInputFile({ name: 'invalid.tsv', size: -2 }), /Invalid local input/)
  assert.throws(() => validateLocalInputFile({ name: 'invalid.tsv', size: Number.POSITIVE_INFINITY }), /Invalid local input/)
  assert.match(worker, /validateLocalInputFile\(file\)[\s\S]*file\.arrayBuffer\(\)/)
  assert.doesNotMatch(worker, /checkInputBudget|workingBudgetGiB|maxFileMiB/)
  assert.doesNotMatch(main, /resource-file|resource-budget/)
  assert.match(main, /Local scientific input files have no fixed size cap/)
})

test('volcano drawing remains bounded without changing full scientific results', () => {
  const points = Array.from({ length: 30001 }, (_, x) => ({ x }))
  const sample = thinPlotPoints(points, 2000)
  assert.equal(sample.length, 2000)
  assert.equal(sample[0].x, 0)
  assert.equal(sample.at(-1).x, 30000)
  assert.deepEqual(thinPlotPoints(points, 2000), sample)
  assert.equal(thinPlotPoints(points.slice(0, 10), 2000).length, 10)
  assert.match(main, /thinPlotPoints\(points, state\.resources\.maxPlotPoints\)/)
  assert.match(main, /All tested features remain in the searchable table and export/)
})

test('resource preferences persist without a native monitor or manual RAM requirement', () => {
  assert.match(main, /localStorage\.setItem\('4c-insight-resources'/)
  assert.doesNotMatch(main, /nativeMonitor|resource-ram|physicalRamGiB/)
  assert.match(main, /System RAM \(browser-reported\)/)
  assert.match(main, /RAM is reported only when the browser provides it/)
  assert.match(main, /id="save-resources"/)
  assert.match(main, /id="stop-science-work"/)
  assert.match(html, /styles\.css\?v=0\.21\.0-ux8-browser-metrics/)
})

test('completed science runs release WASM memory and History avoids duplicate JSON clones', () => {
  assert.match(main, /request\.type\.startsWith\('run-'\) && state\.resources\.recycleWorker/)
  assert.match(main, /worker\?\.terminate\(\)/)
  assert.match(main, /results\.compare = state\.compareResult/)
  assert.doesNotMatch(main, /results\.compare = cloneJson\(state\.compareResult\)/)
})

test('very large History volcano extents are scanned, not spread into Math.min', () => {
  assert.match(main, /function paddedBounds\(min, max\)/)
  assert.match(main, /for \(const value of values\)/)
  assert.doesNotMatch(main, /Math\.min\(\.\.\.values\)/)
  assert.doesNotMatch(main, /Math\.max\(\.\.\.values\)/)
  assert.doesNotMatch(main, /historySelectedSnapshot = snapshot/)
})

test('optional memory sampler reports app/worker estimate when supported', async () => {
  const result = await measureAppMemory({ measureUserAgentSpecificMemory: async () => ({ bytes: 2_000_000 }) }, true)
  assert.deepEqual(result, { available: true, bytes: 2_000_000, source: 'App estimate (document and workers)', kind: 'app' })
})

test('memory sampler uses labelled Chromium heap fallback and reports Firefox unavailability', async () => {
  const heap = { memory: { usedJSHeapSize: 32_000_000 } }
  assert.equal((await measureAppMemory(heap, false)).kind, 'heap')
  const failed = { ...heap, measureUserAgentSpecificMemory: async () => { throw new Error('NotAllowedError') } }
  assert.equal((await measureAppMemory(failed, true)).kind, 'heap')
  assert.deepEqual(await measureAppMemory({}, false), {
    available: false, bytes: null, source: 'App memory measurement is not available in this browser', kind: 'unavailable',
  })
})
