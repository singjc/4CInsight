import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { renderProjectInputGuide } from '../project-input-guide.js'
import { classifyInputFilename } from '../file-classifier.js'

const app = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const guide = renderProjectInputGuide()
const main = readFileSync(path.join(app, 'main.js'), 'utf8')
const index = readFileSync(path.join(app, 'index.html'), 'utf8')
const css = readFileSync(path.join(app, 'styles.css'), 'utf8')

test('bulk modality-first folder layout is the first example', () => {
  assert.match(guide, /Common layout: folders by data type/)
  assert.ok(guide.indexOf('Common layout: folders by data type') < guide.indexOf('Alternative: folders by sample'))
  for (const folder of [
    'Expression_Gene_Files/', 'Expression_Transcript_Files/', 'BCR/',
    'BCR_Isotypes/', 'TCR/', 'Immune_Infiltrate/',
  ]) assert.ok(guide.includes(folder), folder)
  assert.match(guide, /Keep the bulk folders you receive from collaborators/)
  assert.match(guide, /You do not have to move hundreds of workbooks/)
})

test('sample-first folder layout remains optional and collapsed', () => {
  assert.match(guide, /<details class="project-guide-alternative">\s*<summary>Alternative: folders by sample \(optional\)<\/summary>/)
  assert.doesNotMatch(guide, /<details class="project-guide-alternative"[^>]*\sopen(?:\s|=|>)/)
  assert.match(guide, /You do not need to create it/)
})

test('bulk examples use the exact recognized workbook suffixes', () => {
  for (const name of [
    '4C_100_tumor_rna_gene_expression_report.xlsx',
    '4C_100_tumor_rna_transcript_expression_report.xlsx',
    '4C_100_bcr_heavy_chain_report.xlsx',
    '4C_100_bcr_isotype_composition.xlsx',
    '4C_100_tcr_alpha_clone_report.xlsx',
    '4C_101_tcr_beta_clone_report.xlsx',
    '4C_100_immune_infiltrate.xlsx',
  ]) {
    assert.ok(guide.includes(name), name)
    assert.equal(classifyInputFilename(name).recognized, true, name)
  }
  for (const name of ['20260820_merged_gene_expression.tsv', '20260820_merged_transcript_expression.tsv']) {
    assert.ok(guide.includes(name), name)
    assert.equal(classifyInputFilename(name).recognized, true, name)
  }
})

test('guide does not require directory names or sample folder reorganization', () => {
  assert.match(guide, /No specific folder names are required/)
  assert.match(guide, /Folder names are examples, not rules/)
  assert.match(guide, /Select the parent folder to include its subfolders/)
  assert.match(guide, /If a collaborator uses another filename convention/)
  assert.match(guide, /review the unrecognized files before analysis/)
})

test('cache revision and local-only behavior remain correct', () => {
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version, '0.22.1')
  assert.ok(main.includes(`project-input-guide.js?v=${version}`))
  assert.ok(index.includes(`main.js?v=${version}`))
  assert.ok(index.includes(`styles.css?v=${version}`))
  assert.match(css, /\.project-guide-alternative/)
  assert.doesNotMatch(guide, /<script|<iframe|<form|https?:\/\//i)
  const code = readFileSync(path.join(app,'project-input-guide.js'),'utf8')
  assert.doesNotMatch(code, /\bfetch\s*\(|XMLHttpRequest|FileReader|\.arrayBuffer\(/)
})
