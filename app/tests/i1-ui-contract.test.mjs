import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const app = new URL('../', import.meta.url)
const read = name => readFileSync(new URL(name, app), 'utf8')

test('I1 loads a pure alignment module and exposes an explicit local preview', () => {
  const main = read('main.js')
  assert.match(main, /function i1AlignmentPanel\(/)
  assert.match(main, /function i1RunAlignment\(/)
  assert.match(main, /bindI1AlignmentEvents\(\)/)
  assert.match(main, /\$\{i1AlignmentPanel\(\)\}/)
  assert.match(main, /load-metadata-table/)
  assert.match(main, /readFirstTsvRecord\(file\)/)
  assert.match(main, /i1-export-json/)
  assert.match(main, /i1-export-tsv/)
  assert.match(main, /data-dynamic-table="i1-alignment-table"/)
  assert.match(main, /'active'\s*,?\s*\/\//m.test(main) ? /alignmentSource/ : /alignmentSource/)
})

test('I1 identity audit remains independent of I2A scientific fusion and does not upload data', () => {
  const main = read('main.js')
  const align = read('multiomics-alignment.js')
  assert.match(main, /I1 checks identities and source availability only/)
  assert.match(main, /The I2A panel below can combine full unsupervised sample distances/)
  assert.doesNotMatch(align, /\bfetch\s*\(|XMLHttpRequest|\/api\/|localhost/i)
  assert.doesNotMatch(align, /Math\.random|new\s+WebSocket/)
})

test('I1 assets and documentation remain cache-coherent and local-only', () => {
  const main = read('main.js'), html = read('index.html'), css = read('styles.css')
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version, '0.21.0-i2-distance-fusion')
  assert.match(main, /multiomics-alignment\.js\?v=0\.21\.0-i2-distance-fusion/)
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.match(css, /\.i1-alignment-scroll/)
  assert.doesNotMatch(main, /docs\/INTEGRATIVE_ANALYSIS_SCIENCE_PLAN/)
})
