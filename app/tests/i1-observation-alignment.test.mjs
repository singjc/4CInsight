import test from 'node:test'
import assert from 'node:assert/strict'
import {
  I1_MODALITIES, alignObservations, alignmentTsv, mergedSampleIds,
  parseTsvHeader, readFirstTsvRecord,
} from '../multiomics-alignment.js'

const names = I1_MODALITIES.map(m => m.key)
const columns = ['sample_id', 'subject_id', 'timepoint', 'qc_review_status', 'gene_expression_id', 'tcr_alpha_id']
function table(rows = [
  ['S1', 'P1', 'T0', 'Pass', 'Gene-A', 'TCRA1'],
  ['S2', 'P1', 'T1', 'Pass', '', 'TCRA2'],
  ['S3', 'P2', 'T0', 'Failed', 'Gene-C', 'TCRA3'],
]) { return { columns, rows } }
function file(kind, sampleId, name = '') { return { kind, sampleId, name: name || `${sampleId}.xlsx`, relativePath: `data/${kind}/${name || sampleId + '.xlsx'}` } }
function input(rest = {}) {
  return {
    table: table(),
    observationColumn: 'sample_id', subjectColumn: 'subject_id', timeColumn: 'timepoint',
    exclusions: [{ enabled: true, label: 'QC exclusion', column: 'qc_review_status', values: ['failed'] }],
    records: [file('gene-expression', 'S1'), file('bcr', 'S1'), file('tcr-alpha', 'TCRA1'), file('tcr-alpha', 'TCRA2'), file('tcr-alpha', 'TCRA3')],
    merged: { gene: { sampleIds: ['Gene-A', 'S2', 'Gene-C'], source: 'merged_gene.tsv' } },
    ...rest,
  }
}

test('seven modalities have explicit, stable and distinct alias columns', () => {
  assert.equal(I1_MODALITIES.length, 7)
  assert.equal(new Set(names).size, 7)
  assert.equal(new Set(I1_MODALITIES.map(v => v.alias)).size, 7)
})

test('I1 aligns source IDs exactly, without substituting a non-matching explicit alias', () => {
  const ledger = alignObservations(input())
  assert.equal(ledger.rows.length, 3)
  assert.equal(ledger.rows[0].modalities.gene.alias, 'Gene-A')
  assert.equal(ledger.rows[0].modalities.gene.status, 'present')
  assert.equal(ledger.rows[1].modalities.gene.alias, 'S2')
  assert.equal(ledger.rows[1].modalities.gene.status, 'present')
  assert.equal(ledger.rows[0].modalities.tcrAlpha.status, 'present')
  assert.equal(ledger.rows[0].modalities.tcrBeta.status, 'missing')
  assert.equal(ledger.rows[0].subject, 'P1')
  assert.equal(ledger.rows[1].timepoint, 'T1')
  assert.equal(ledger.summary.excluded, 1)
  assert.equal(ledger.summary.eligible, 2)
  assert.equal(ledger.summary.pairedTwo, 2)
  assert.equal(ledger.summary.complete, 0)
  const missing = alignObservations(input({ merged: {}, records: [file('gene-expression', 'S1')] }))
  assert.equal(missing.rows[0].modalities.gene.alias, 'Gene-A')
  assert.equal(missing.rows[0].modalities.gene.status, 'missing')
})

test('explicit aliases are case sensitive; multiple files and shared source aliases are ambiguous', () => {
  const records = [file('bcr', 'S1', 'one.xlsx'), file('bcr', 'S1', 'two.xlsx'), file('tcr-alpha', 'TCRA1')]
  const result = alignObservations(input({ records, merged: {} }))
  assert.equal(result.rows[0].modalities.bcr.status, 'ambiguous')
  assert.equal(result.rows[0].modalities.tcrAlpha.status, 'present')
  const aliasRows = [
    ['S1', 'P1', 'T0', 'Pass', '', 'shared'],
    ['S2', 'P2', 'T1', 'Pass', '', 'shared'],
  ]
  const conflict = alignObservations(input({ table: table(aliasRows), records: [file('tcr-alpha', 'shared')], merged: {} }))
  assert.equal(conflict.rows[0].modalities.tcrAlpha.status, 'ambiguous')
  assert.equal(conflict.rows[1].modalities.tcrAlpha.status, 'ambiguous')
  assert.match(conflict.warnings.join(' '), /assigned to 2 observations/)
  const missingAliasConflict = alignObservations(input({ table: table(aliasRows), records: [], merged: {} }))
  assert.equal(missingAliasConflict.rows[0].modalities.tcrAlpha.status, 'ambiguous')
  assert.equal(missingAliasConflict.rows[1].modalities.tcrAlpha.status, 'ambiguous')
})

test('duplicate observations and invalid exclusion columns are rejected, not deduplicated', () => {
  assert.throws(() => alignObservations(input({ table: table([['S1'], ['S1']]) })), /Duplicate observation/)
  assert.throws(() => alignObservations(input({ exclusions: [{ enabled: true, column: 'unknown', values: ['Failed'] }] })), /column 'unknown'/)
  assert.throws(() => alignObservations(input({ exclusions: [{ enabled: true, label: 'QC', column: 'qc_review_status', values: [] }] })), /incomplete/)
})

test('repeated patient visits are retained with a warning and incomplete metadata is not fabricated', () => {
  const repeated = alignObservations(input({ table: table([
    ['S1', 'P1', 'T0', 'Pass', '', ''],
    ['S2', 'P1', 'T0', 'Pass', '', ''],
    ['S3', '', '', 'Pass', '', ''],
  ]), records: [], merged: {} }))
  assert.equal(repeated.rows.length, 3)
  assert.match(repeated.warnings.join(' '), /replicates/)
  assert.match(repeated.warnings.join(' '), /no patient\/subject ID/)
  assert.equal(repeated.rows[2].subject, '')
  assert.equal(repeated.rows[2].timepoint, '')
})

test('modality selection and missingness preserve all observations, including excluded rows', () => {
  const result = alignObservations(input({ enabled: { bcr: false, transcript: false } }))
  assert.equal(result.rows.length, 3)
  assert.equal(result.summary.counts.find(c => c.key === 'bcr').selected, false)
  assert.equal(result.rows[2].excluded, true)
  assert.equal(result.rows[0].modalities.bcr.status, 'not-selected')
  assert.equal(result.summary.counts.find(c => c.key === 'bcr').present, 0)
})

test('merged header parser supports both 4C column orders, quotes and prevents false sample matching', () => {
  assert.deepEqual(mergedSampleIds(['Gene Symbol', 'NCBI Gene ID', 'A Raw Count', 'A TPM', 'B Raw Count', 'B TPM'], 'gene'), ['A', 'B'])
  assert.deepEqual(mergedSampleIds(['Gene Symbol', 'NCBI Gene ID', 'A Is Expressed'], 'gene'), ['A'])
  assert.deepEqual(mergedSampleIds(['Transcript Name', 'NCBI Gene ID', 'A Total Reads', 'B Total Reads', 'A TPM', 'B TPM'], 'transcript'), ['A', 'B'])
  assert.deepEqual(parseTsvHeader('Gene Symbol\tNCBI Gene ID\t"A""X TPM"'), ['Gene Symbol', 'NCBI Gene ID', 'A"X TPM'])
  assert.throws(() => mergedSampleIds(['Gene Symbol', 'NCBI Gene ID', 'A TPM', 'A TPM'], 'gene'), /Repeated/)
  assert.throws(() => mergedSampleIds(['Gene Symbol', 'NCBI Gene ID', 'Other Value'], 'gene'), /Unknown/)
  assert.throws(() => mergedSampleIds(['Wrong', 'NCBI Gene ID', 'A TPM'], 'gene'), /unrecognized/)
})

test('streaming header reads only the header, not a large merged payload', async () => {
  const header = 'Gene Symbol\tNCBI Gene ID\t"A\tB TPM"\n'
  const giant = new Blob([header, new Uint8Array(4_000_000)])
  const result = await readFirstTsvRecord(giant)
  assert.deepEqual(result, ['Gene Symbol', 'NCBI Gene ID', 'A\tB TPM'])
  assert.deepEqual(mergedSampleIds(result, 'gene'), ['A\tB'])
})

test('source parser errors remain per modality, without pretending missing data are zero', () => {
  const result = alignObservations(input({ records: [file('tcr-alpha', 'TCRA1')], merged: { gene: { error: 'bad TSV' } } }))
  assert.equal(result.rows[0].modalities.gene.status, 'source-error')
  assert.equal(result.rows[0].modalities.tcrAlpha.status, 'present')
  assert.match(result.warnings.join(' '), /bad TSV/)
})

test('TSV export contains every observation, all modalities, provenance and spreadsheet guard', () => {
  const modified = table([['=SUM(1,2)', 'P', 'T0', 'Pass', '', '']])
  const result = alignObservations(input({ table: modified, records: [], merged: {} }))
  const tsv = alignmentTsv(result)
  assert.equal(tsv.split('\n').filter(Boolean).length, 2)
  assert.match(tsv, /'\=SUM\(1,2\)/)
  assert.match(tsv, /tcrBeta_status/)
  assert.equal(JSON.stringify(result).includes('=SUM(1,2)'), true, 'exact source ID is retained in JSON')
})

test('large cohorts are indexed without a fixed file count or observation cap', () => {
  const rows = Array.from({ length: 2983 }, (_, i) => [`S${i}`, `P${Math.floor(i / 3)}`, `V${i % 3}`, 'Pass', '', ''])
  const records = rows.map(row => file('tcr-beta', row[0]))
  const result = alignObservations(input({ table: table(rows), records, merged: {}, enabled: { gene: false } }))
  assert.equal(result.rows.length, 2983)
  assert.equal(result.summary.counts.find(c => c.key === 'tcrBeta').present, 2983)
  assert.equal(result.summary.eligible, 2983)
})
