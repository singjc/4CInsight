import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { renderProjectInputGuide, sampleWorkbookSuffixes } from '../project-input-guide.js'
import { classifyInputFilename } from '../file-classifier.js'

const app = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const main = readFileSync(path.join(app, 'main.js'), 'utf8')
const html = readFileSync(path.join(app, 'index.html'), 'utf8')
const css = readFileSync(path.join(app, 'styles.css'), 'utf8')
const guide = renderProjectInputGuide()

test('Project starts with an accessible collapsed input file guide', () => {
  assert.match(guide, /<details class="project-guide-details" id="project-input-guide">/)
  assert.doesNotMatch(guide, /<details[^>]*\bopen(?:=|\s|>)/)
  assert.match(guide, /<summary[^>]*>/)
  assert.match(guide, /How to prepare your project files/)
  const view = main.slice(main.indexOf('function projectView() {'), main.indexOf('function ', main.indexOf('function projectView() {') + 22))
  assert.equal(view.split('${renderProjectInputGuide()}').length - 1, 1)
  assert.ok(view.indexOf('${renderProjectInputGuide()}') < view.indexOf('id="select-data-heading"'))
})

test('seven supported workbook suffixes are identical to the real classifier', () => {
  assert.equal(sampleWorkbookSuffixes.length, 7)
  for (const [label, suffix] of sampleWorkbookSuffixes) {
    assert.match(guide, new RegExp(suffix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')))
    const classified = classifyInputFilename(`4C_100${suffix}`)
    assert.equal(classified.recognized, true, label)
    assert.equal(classified.sampleId, '4C_100', label)
  }
})

test('merged filenames and metadata example follow actual detection rules', () => {
  assert.equal(classifyInputFilename('merged_gene_expression.tsv').kind, 'merged-gene-expression')
  assert.equal(classifyInputFilename('merged_transcript_expression.tsv').kind, 'merged-transcript-expression')
  assert.equal(classifyInputFilename('cohort_sample_metadata.tsv').kind, 'metadata-candidate')
  for (const name of ['merged_gene_expression.tsv', 'merged_transcript_expression.tsv', 'cohort_sample_metadata.tsv']) {
    assert.ok(guide.includes(name), name)
  }
  assert.match(guide, /No specific folder names are required/)
})

test('guide distinguishes essential columns and optional metadata roles', () => {
  for (const field of ['sample_id', 'subject_id', 'analysis_group', 'Clone Count', 'IGH Isotype', 'Isotype Read Fraction', 'Immune Cell Type', 'ssGSEA Score']) {
    assert.ok(guide.includes(field), field)
  }
  assert.match(guide, /one row per biological sample/i)
  assert.match(guide, /not required/i)
  assert.match(guide, /Sample Metadata workspace/)
})

test('guide contains no network links, upload requests or file-reading behavior', () => {
  assert.doesNotMatch(guide, /<script|<iframe|<form|https?:\/\/|<a /i)
  assert.doesNotMatch(readFileSync(path.join(app, 'project-input-guide.js'), 'utf8'), /\bfetch\s*\(|XMLHttpRequest|FileReader|\.arrayBuffer\(/)
})

test('visible project help has its own responsive style and one cache revision', () => {
  assert.match(css, /\.project-guide-details/)
  assert.match(css, /@media \(max-width: 880px\)/)
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.ok(version?.includes('ux14-input-guide'))
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(main.includes(`project-input-guide.js?v=${version}`))
})
