import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { mergeMetrics, defaultMergeMetrics, sortedMergeFiles, mergeBatchRows, validateMergeConfig } from '../merge-helpers.js'
const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const worker = await readFile(new URL('../analysis-worker.js', import.meta.url), 'utf8')
const merger = await readFile(new URL('../merge-worker.js', import.meta.url), 'utf8')
const bridge = await readFile(new URL('../../wasm/src/lib.rs', import.meta.url), 'utf8')

test('merger defaults reproduce the original native presets', () => {
  assert.deepEqual(defaultMergeMetrics.gene, ['raw-count', 'tpm'])
  assert.deepEqual(defaultMergeMetrics.transcript, ['total-reads', 'tpm'])
  assert.equal(mergeMetrics.gene.length, 6)
  assert.equal(mergeMetrics.transcript.length, 6)
})
test('source workbooks are selected by modality and sorted deterministically', () => {
  const records = [
    { id:'2', kind:'gene-expression', name:'b.xlsx', relativePath:'samples/b.xlsx' },
    { id:'1', kind:'gene-expression', name:'a.xlsx', relativePath:'samples/a.xlsx' },
    { id:'3', kind:'transcript-expression', name:'c.xlsx' },
  ]
  assert.deepEqual(sortedMergeFiles(records, new Map([['1',{name:'a.xlsx'}],['2',{name:'b.xlsx'}]]), 'gene').map(f=>f.name), ['a.xlsx','b.xlsx'])
  assert.throws(() => sortedMergeFiles(records, new Map(), 'gene'), /reconnected/)
})
test('invalid metrics and column order are rejected before any XLSX read', () => {
  assert.throws(() => validateMergeConfig('gene', [], 'adjacent-by-sample'))
  assert.throws(() => validateMergeConfig('gene', ['total-reads'], 'adjacent-by-sample'))
  assert.throws(() => validateMergeConfig('gene', ['tpm','tpm'], 'adjacent-by-sample'))
  assert.throws(() => validateMergeConfig('gene', ['tpm'], 'bad'))
  assert.doesNotThrow(() => validateMergeConfig('transcript', ['total-reads','tpm'], 'grouped-by-measurement'))
})
test('row processing is bounded by memory target even at 300 samples', () => {
  assert.equal(mergeBatchRows(50000, 300, 2) <= 256, true)
  assert.ok(mergeBatchRows(50000, 300, 2) * 300 * 2 * 9 <= 2 * 1024 * 1024)
  assert.equal(mergeBatchRows(0, 300, 2), 1)
})
test('Project UI exposes no-upload merge, restore, cancel, and download', () => {
  for (const expected of ['projectMergeSection()', 'runExpressionMerge(', 'restoreMergedTables(', "workerRequest('run-merge-expression'", 'downloadMergedTable(', 'stopScienceWork']) {
    assert.ok(main.includes(expected), expected)
  }
  assert.match(merger, /navigator\.storage\.getDirectory/)
  assert.match(merger, /createWritable/)
  assert.match(merger, /file\.arrayBuffer\(\)/)
  assert.match(merger, /wasm\.merge_render_rows_tsv\(/)
  assert.match(merger, /removeEntry\(runId, \{ recursive: true \}\)/)
  assert.match(merger, /navigator\.locks\?\.request/)
  assert.match(merger, /scratchRoot\.removeEntry\(name, \{ recursive: true \}\)/)
  assert.match(worker, /type: 'merge-expression-result'/)
})
test('Rust handles canonical feature index, sample mapping, and TSV rendering', () => {
  for (const name of ['merge_begin_json', 'merge_sample_packed', 'merge_last_sample_json', 'merge_render_rows_tsv', 'merge_reset']) {
    assert.ok(bridge.includes(`fn ${name}`))
  }
  assert.doesNotMatch(merger, /split\(['"]\\t/)
})
