import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  CORE_COLUMNS, createBlankMetadata, draftFromSamples, parseDelimited, serializeDelimited,
  validateMetadata, addColumn, renameColumn, removeColumn, addRow, setCell, removeRow,
  matchingFiles, autoMapFiles, mappingSummary, mappingManifest, filterRows,
} from '../sample-metadata.js'
import { classifyInputFilename } from '../file-classifier.js'

const base = new URL('../', import.meta.url)
const source = name => readFile(new URL(name, base), 'utf8')
const record = (sample, kind = 'gene-expression') => ({ id: `file:${sample}:${kind}`, kind, sampleId: sample, name: `${sample}.xlsx`, relativePath: `study/${sample}.xlsx` })

test('blank metadata uses the standard editable sample identity and annotation fields', () => {
  assert.deepEqual(createBlankMetadata().columns, CORE_COLUMNS)
  assert.deepEqual(createBlankMetadata().rows, [])
})
test('detected sample reports create one row per unique sample, never per modality', () => {
  const table = draftFromSamples([record('S2'), record('S1'), record('S1', 'transcript-expression'), record('S1', 'tcr-beta')])
  assert.deepEqual(table.rows.map(row => row[0]), ['S1', 'S2'])
})
test('TSV and CSV import/export preserve quoted tabs, commas, quotes, and newlines', () => {
  const table = { columns: ['sample_id', 'notes'], rows: [['S1', 'a\t"b"\nsecond line'], ['S2', 'comma,value']] }
  assert.deepEqual(parseDelimited(serializeDelimited(table)), table)
  assert.deepEqual(parseDelimited(serializeDelimited(table, ','), ','), table)
})
test('metadata editing supports adding rows and columns without dropping other cells', () => {
  let table = addRow(createBlankMetadata(), 'S1')
  table = addColumn(table, 'batch')
  table = setCell(table, 0, table.columns.indexOf('batch'), 'A')
  table = renameColumn(table, table.columns.indexOf('batch'), 'processing_batch')
  assert.equal(table.rows[0].at(-1), 'A')
  table = removeColumn(table, table.columns.indexOf('processing_batch'))
  assert.equal(table.columns.includes('processing_batch'), false)
  table = removeRow(table, 0)
  assert.equal(table.rows.length, 0)
  assert.throws(() => removeColumn(table, 0), /cannot be removed/)
  assert.throws(() => addColumn(table, 'sample_id'), /already exists/)
})
test('blank and duplicate biological sample IDs prevent activation', () => {
  const table = { columns: ['sample_id', 'group'], rows: [['S1', 'Case'], ['', 'Control'], ['S1', 'Control']] }
  const report = validateMetadata(table)
  assert.deepEqual(report.blankIds, [1])
  assert.deepEqual(report.duplicates, ['S1'])
})
test('file-to-sample assignments use exact names; never invent fuzzy matches', () => {
  const files = [record('S1'), record('S1', 'transcript-expression'), record('S3'), record('S2')]
  const table = { columns: ['sample_id'], rows: [['S1'], ['S2']] }
  const mappings = autoMapFiles(files, table)
  assert.equal(mappings['file:S3:gene-expression'], '')
  assert.equal(mappings['file:S2:gene-expression'], 'S2')
  assert.deepEqual(mappingSummary(files, mappings, table), { matched: 3, missing: 1, duplicates: 0, blank: 0 })
  assert.equal(mappingManifest(files, mappings).rows.length, 4)
  assert.equal(matchingFiles(files).length, 4)
})
test('metadata search/filter/sort operates on all rows, not only displayed page', () => {
  const table = { columns: ['sample_id', 'status'], rows: [['S10', 'Failed'], ['S2', 'Pass'], ['S1', 'Failed']] }
  assert.deepEqual(filterRows(table, '', 'status', 'Failed', 'sample_id').map(({ row }) => row[0]), ['S1', 'S10'])
  assert.equal(filterRows(table, 'S2').length, 1)
})
test('large metadata and mapping can be validated without fixed file-count cap', () => {
  const files = Array.from({ length: 2983 }, (_, i) => record(`S${i + 1}`))
  const table = draftFromSamples(files)
  assert.equal(validateMetadata(table).total, 2983)
  assert.equal(mappingSummary(files, autoMapFiles(files, table), table).matched, 2983)
})
test('generated sample metadata is recognized by existing preflight', () => {
  const result = classifyInputFilename('4c_sample_metadata.tsv')
  assert.equal(result.kind, 'metadata-candidate')
  assert.equal(result.recognized, true)
})
test('WASM exposes full metadata table and worker dispatch for local XLSX import', async () => {
  const wasm = await readFile(new URL('../../wasm/src/lib.rs', import.meta.url), 'utf8')
  const worker = await source('analysis-worker.js')
  assert.match(wasm, /pub fn read_metadata_table_json\(/)
  assert.match(worker, /wasm\.read_metadata_table_json/)
  assert.match(worker, /request\.type === 'load-metadata-table'/)
})
test('UI includes create/open/edit/mapping/export and activates metadata in science workflows', async () => {
  const main = await source('main.js')
  const html = await source('index.html')
  assert.match(main, /Sample Metadata/)
  assert.match(main, /function metadataView\(/)
  assert.match(main, /metadataUseInAnalyses/)
  assert.match(main, /state\.qcConfig\.metadataId = editor\.generatedId/)
  assert.match(main, /Download file map TSV/)
  assert.match(html, /styles\.css\?v=0\.21\.0-[a-z0-9-]+/)
})

test('file mappings can populate standard modality aliases without changing sample identities', async () => {
  const { applyMappingsToAliases } = await import('../sample-metadata.js')
  const files = [record('S1'), record('S1', 'transcript-expression')]
  const table = { columns: ['sample_id', 'analysis_group'], rows: [['S1', 'Case']] }
  const result = applyMappingsToAliases(table, files, autoMapFiles(files, table))
  assert.equal(result.aliasesWritten, 2)
  assert.equal(result.table.rows[0][result.table.columns.indexOf('gene_expression_id')], 'S1')
  assert.equal(result.table.rows[0][result.table.columns.indexOf('transcript_expression_id')], 'S1')
  assert.equal(table.columns.length, 2)
})
test('alias collisions are rejected instead of silently taking one file', async () => {
  const { applyMappingsToAliases } = await import('../sample-metadata.js')
  const files = [record('S1'), record('OTHER')]
  const table = { columns: ['sample_id'], rows: [['S1']] }
  assert.throws(() => applyMappingsToAliases(table, files, { [files[0].id]: 'S1', [files[1].id]: 'S1' }), /Conflicting/)
})


test('generated WASM exposes full metadata reader when artifact checks are required', async t => {
  if (process.env.CFS_REQUIRE_WASM_ARTIFACTS !== '1') {
    t.skip('Full metadata WASM binding verified after wasm-pack build in CI')
    return
  }
  const module = await import('../wasm/four_c_insight_wasm.js')
  assert.equal(typeof module.read_metadata_table_json, 'function')
})
