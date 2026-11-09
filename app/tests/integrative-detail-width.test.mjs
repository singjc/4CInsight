import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const read = file => readFileSync(new URL(file, import.meta.url), 'utf8')

test('Details is a single shrinkable grid column, not a max-content track', () => {
  const css = read('../integrative-detail-width.css')
  assert.match(css, /#integrative-details-panel\.integrative-detail-layout\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)/s)
  assert.match(css, /#integrative-details-panel\.integrative-detail-layout\s*>\s*\*\s*\{[^}]*min-width:\s*0/s)
  assert.match(css, /#integrative-details-panel\.integrative-detail-layout\s*>\s*\*\s*\{[^}]*max-width:\s*100%/s)
})

test('Wide alignment tables, heatmaps and plots retain their own horizontal scroll', () => {
  const css = read('../integrative-detail-width.css')
  assert.match(css, /\.i1-alignment-scroll/)
  assert.match(css, /\.joint-heatmap-scroll/)
  assert.match(css, /\.table-scroll/)
  assert.match(css, /\.plot-scroll/)
  assert.match(css, /overflow-x:\s*auto/)
  assert.doesNotMatch(css, /\b(?:canvas|table|svg)\s*\{|\.joint-heatmap-grid\s*\{/,
    'Never resize the underlying scientific matrix, canvas, or coordinate system')
})

test('The additional stylesheet loads after the versioned primary CSS without changing JS version', () => {
  const html = read('../index.html')
  const main = read('../main.js')
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version, '0.22.8')
  const original = `styles.css?v=${version}`
  const extra = 'integrative-detail-width.css?v=20261109-r1'
  assert.ok(html.includes(original))
  assert.ok(html.includes(extra))
  assert.ok(html.indexOf(extra) > html.indexOf(original))
  assert.ok(html.includes(`main.js?v=${version}`))
})

test('The detailed result panels still exist and no scientific source is replaced', () => {
  const main = read('../main.js')
  assert.match(main, /id="integrative-details-panel"[^\n]*class="integrative-detail-layout"/)
  for (const name of ['i1AlignmentPanel()', 'i2DistancePanel()', 'jointStructurePanel()', 'jointStabilityPanel()', 'biologicalFindingsPanel()']) {
    assert.ok(main.includes('${' + name + '}'), `Detailed view must retain ${name}`)
  }
})
