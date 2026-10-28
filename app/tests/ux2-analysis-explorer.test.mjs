import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const styles = await readFile(new URL('../styles.css', import.meta.url), 'utf8')
const docs = await readFile(new URL('../docs-content.js', import.meta.url), 'utf8')
const { methodDocs } = await import(new URL('../docs-content.js', import.meta.url))


test('UX2 removes redundant Results tab and keeps History', () => {
  assert.doesNotMatch(main, /\['results',\s*'Results'/)
  assert.match(main, /\['history', 'History', '6'\]/)
  assert.doesNotMatch(main, /function resultsView\(/)
})

test('differential expression has global FDR/log2FC volcano controls', () => {
  assert.match(main, /globalThresholds: \{ fdr: 0\.05, absLog2Fc: 1\.0 \}/)
  assert.match(main, /Differential-expression volcano plot/)
  assert.match(main, /id="global-fdr-threshold"/)
  assert.match(main, /id="global-log2fc-threshold"/)
  assert.match(main, /volcano-cutoff-line/)
  assert.match(styles, /\.volcano-up \{ fill: #d64545/)
  assert.match(styles, /\.volcano-down \{ fill: #3159d9/)
})

test('zoomed scatter plots can expose direct point labels', () => {
  assert.match(main, /scatter-point-label/)
  assert.match(main, /labelZoom >= 1\.45/)
  assert.match(main, /nearest >= 28/)
  assert.match(main, /data-sample-id=/)
})

test('data tables are searchable sortable and sticky', () => {
  assert.match(main, /function enhanceTables\(\)/)
  assert.match(main, /placeholder="Search table…"/)
  assert.match(main, /aria-sort/)
  assert.match(main, /dynamicTableSources/)
  assert.match(styles, /th \{ position: sticky; top: 0;/)
  assert.match(styles, /\.sortable-table-header/)
})

test('documentation is searchable and literature-backed', () => {
  assert.match(main, /id="docs-search"/)
  assert.match(main, /methodDocs/)
  assert.match(docs, /Benjamini & Hochberg/)
  assert.match(docs, /Jolliffe & Cadima/)
  assert.match(docs, /Anderson \(2001\)/)
  assert.match(docs, /Ambroise & McLachlan/)
  assert.match(docs, /wasm-bindgen-rayon/)
  assert.equal(methodDocs.every((section) => Array.isArray(section.references) && section.references.length > 0), true, 'every documentation section must include references')
})

test('sidebar is fixed during desktop page scrolling and system info is browser-reported', () => {
  assert.match(styles, /@media \(min-width: 721px\)[\s\S]*\.sidebar \{ position: sticky; top: 0; height: 100vh;/)
  assert.match(main, /navigator\.hardwareConcurrency/)
  assert.match(main, /navigator\.deviceMemory/)
  assert.match(main, /Memory not exposed/)
  assert.doesNotMatch(main, /Runs on this device/)
})

test('render only constructs the active workflow view', () => {
  assert.match(main, /const viewFactories = \{/)
  assert.match(main, /\(viewFactories\[state\.tab\] \|\| projectView\)\(\)/)
  assert.doesNotMatch(main, /project:\s*projectView\(\)/)
})
