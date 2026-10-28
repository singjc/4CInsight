import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import {
  DEFAULT_RESOURCES,
  normalizeResources,
  suggestedScienceThreads,
  checkInputBudget,
  thinPlotPoints,
} from '../resource-controls.js'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')

test('default CPU budget is 85%, retains OS capacity, and browser still reports one worker', () => {
  assert.equal(DEFAULT_RESOURCES.cpuPercent, 85)
  assert.equal(suggestedScienceThreads(20), 8)
  assert.equal(suggestedScienceThreads(8), 6)
  assert.equal(suggestedScienceThreads(2), 1)
  assert.equal(suggestedScienceThreads(1), 1)
  assert.match(main, /Current single-worker WASM cannot use this thread budget/)
  assert.match(main, /WASM science: 1 worker/)
})

test('resources clamp hostile inputs and recover from invalid persisted values', () => {
  const r = normalizeResources({ cpuPercent: 800, maxThreads: 1e8, maxFileMiB: -1, workingBudgetGiB: 0, physicalRamGiB: -4, maxPlotPoints: 1e9 })
  assert.deepEqual([r.cpuPercent, r.maxThreads, r.maxFileMiB, r.workingBudgetGiB, r.physicalRamGiB, r.maxPlotPoints], [90, 32, 16, 0.25, 0, 6000])
  assert.deepEqual(normalizeResources(null), { ...DEFAULT_RESOURCES })
})

test('large local files are blocked before arrayBuffer loads them into WASM', () => {
  checkInputBudget({ name: 'safe.tsv', size: 4 * 1024 * 1024 })
  assert.throws(() => checkInputBudget({ name: 'large.tsv', size: 300 * 1024 * 1024 }), /per-file limit/)
  assert.throws(() => checkInputBudget({ name: 'large.tsv', size: 150 * 1024 * 1024 }, { ...DEFAULT_RESOURCES, workingBudgetGiB: 0.25 }), /working budget/)
  assert.match(worker, /checkInputBudget\(file, activeResources\)[\s\S]*file\.arrayBuffer\(\)/)
})

test('volcano decimation is bounded deterministic and retains endpoints', () => {
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

test('resource page persists a separate browser-level policy without claiming OS RAM', () => {
  assert.match(main, /localStorage\.setItem\('4c-insight-resources'/)
  assert.match(main, /physicalRamGiB/)
  assert.match(main, /not a hard RAM limit/)
  assert.match(main, /Neither number measures currently free RAM/)
  assert.match(main, /id="save-resources"/)
  assert.match(main, /id="stop-science-work"/)
  assert.match(html, /styles\.css\?v=0\.21\.0-ux\d+-[a-z-]+/)
})

test('completed science runs can release WASM memory; history avoids duplicate JSON clones', () => {
  assert.match(main, /request\.type\.startsWith\('run-'\) && state\.resources\.recycleWorker/)
  assert.match(main, /worker\?\.terminate\(\)/)
  assert.match(main, /results\.compare = state\.compareResult/)
  assert.doesNotMatch(main, /results\.compare = cloneJson\(state\.compareResult\)/)
})

test('large History volcano extents never spread a large array onto Math.min/Math.max', () => {
  assert.match(main, /function paddedBounds\(min, max\)/)
  assert.match(main, /for \(const value of values\)/)
  assert.doesNotMatch(main, /Math\.min\(\.\.\.values\)/)
  assert.doesNotMatch(main, /Math\.max\(\.\.values\)/)
  assert.doesNotMatch(main, /historySelectedSnapshot = snapshot/)
})
