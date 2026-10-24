import test from 'node:test'
import assert from 'node:assert/strict'
import { classifyInputFilename } from '../file-classifier.js'

test('classifies established 4C sample workbook names', () => {
  const gene = classifyInputFilename('4C_465_tumor_rna_gene_expression_report (1).xlsx')
  assert.equal(gene.kind, 'gene-expression')
  assert.equal(gene.sampleId, '4C_465')

  const tcr = classifyInputFilename('P001_tcr_beta_clone_report.xlsx')
  assert.equal(tcr.kind, 'tcr-beta')
  assert.equal(tcr.sampleId, 'P001')
})

test('classifies merged tables and metadata candidates', () => {
  assert.equal(classifyInputFilename('merged_gene_expression.tsv').kind, 'merged-gene-expression')
  assert.equal(classifyInputFilename('cohort_sample_metadata.xlsx').kind, 'metadata-candidate')
  assert.equal(classifyInputFilename('cohort_sample_metadata.tsv').kind, 'metadata-candidate')
})

test('leaves unrelated files unresolved for user review', () => {
  assert.equal(classifyInputFilename('notes.pdf').recognized, false)
})
