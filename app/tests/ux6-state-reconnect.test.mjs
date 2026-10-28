import test from 'node:test'
import assert from 'node:assert/strict'
import { matchSavedFiles, matchSavedReference } from '../state-reconnect.js'

const ref = (name, relativePath, size = 1) => ({ name, relativePath, size })

test('portable state reconnects with the original selected folder', () => {
  const manifest = [ref('a.xlsx', 'cohort/group/a.xlsx', 20), ref('b.tsv', 'cohort/b.tsv', 500)]
  const files = [ref('a.xlsx', 'cohort/group/a.xlsx', 20), ref('b.tsv', 'cohort/b.tsv', 500)]
  const result = matchSavedFiles(manifest, files)
  assert.equal(result.matched, 2)
  assert.equal(result.missing.length, 0)
})

test('portable state reconnects when the selected folder root changes', () => {
  const oldFile = ref('counts.tsv', 'root/experiment/counts.tsv', 1000)
  const newFile = ref('counts.tsv', 'experiment/counts.tsv', 1000)
  assert.equal(matchSavedReference(oldFile, [newFile]), newFile)
})

test('does not silently choose ambiguous same-name same-size files', () => {
  const old = ref('counts.tsv', 'old/counts.tsv', 1000)
  const candidates = [ref('counts.tsv', 'new1/counts.tsv', 1000), ref('counts.tsv', 'new2/counts.tsv', 1000)]
  const result = matchSavedFiles([old], candidates)
  assert.equal(result.matched, 0)
  assert.equal(result.ambiguous.length, 1)
})

test('prefers exact path and size over filename alone', () => {
  const files = [ref('sample.tsv', 'b/sample.tsv', 100), ref('sample.tsv', 'a/sample.tsv', 100)]
  assert.equal(matchSavedReference(ref('sample.tsv', 'a/sample.tsv', 100), files), files[1])
  assert.equal(matchSavedReference(ref('sample.tsv', 'a/sample.tsv', 999), files), null)
})

test('reports missing and extra files with no early file-size cap', () => {
  const result = matchSavedFiles([ref('large.tsv', 'large.tsv', 12_000_000_000), ref('lost.tsv', 'lost.tsv', 12)], [ref('large.tsv', 'large.tsv', 12_000_000_000), ref('new.tsv', 'new.tsv', 2)])
  assert.equal(result.matched, 1)
  assert.equal(result.missing.length, 1)
  assert.equal(result.additional, 1)
})

test('handles thousands of files without source byte reads', () => {
  const records = Array.from({ length: 2983 }, (_, i) => ref(`S${i}.xlsx`, `cohort/S${i}.xlsx`, i + 1024))
  const result = matchSavedFiles(records, records)
  assert.equal(result.matched, 2983)
  assert.equal(result.additional, 0)
})

test('matching retains subgroup path when the cohort root changes', () => {
  const files = [ref('sample.xlsx', 'updated/gene/sample.xlsx', 500), ref('sample.xlsx', 'updated/tcr/sample.xlsx', 500)]
  const saved = ref('sample.xlsx', 'old/gene/sample.xlsx', 500)
  assert.equal(matchSavedReference(saved, files), files[0])
})
