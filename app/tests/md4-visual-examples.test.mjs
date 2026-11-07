import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { methodDocs } from '../docs-content.js'
import { methodToyExamples, renderToyExamples, exampleSearchText } from '../docs-toy-examples.js'
import {
  visualDefinitionCount, visualCaption, renderToyVisual,
  classifyToyVolcano, renderToyVolcano, volcanoSummary, sanitizeToyThresholds,
  bindToyVisualControls,
} from '../docs-toy-visuals.js'

const read = (f) => readFile(new URL(`../${f}`, import.meta.url), 'utf8')

test('every one of the 22 worked examples has an accessible, matching visual', () => {
  assert.equal(visualDefinitionCount(), 22)
  let count = 0
  for (const section of methodDocs) {
    const examples = methodToyExamples[section.id]
    const html = renderToyExamples(section.id)
    const figures = html.match(/<figure class="docs-visual-figure"/g) || []
    assert.equal(figures.length, examples.length, `${section.id} figure count`)
    assert.match(html, /<details class="docs-toy-math">/)
    assert.match(html, /Illustrative values only/)
    assert.ok(html.indexOf('docs-visual-figure') < html.indexOf('docs-toy-table-wrap'), `${section.id} is not visual-first`)
    for (let i = 0; i < examples.length; i++) {
      const visual = renderToyVisual(section.id, i)
      assert.match(visual, /<svg[^>]+role="img"[^>]+aria-labelledby=/)
      assert.match(visual, /<title id="visual-title-/)
      assert.match(visual, /<desc id="visual-desc-/)
      assert.match(visual, /<figcaption>/)
      assert.match(visual, /viewBox="0 0 680/)
      assert.ok(visualCaption(section.id, i).length > 25)
      assert.ok(exampleSearchText(section.id).includes(visualCaption(section.id, i)))
      count += 1
    }
  }
  assert.equal(count, 22)
})

test('toy normalization, BH, CLR, clustering, AUC and PCA chart facts match teaching examples', () => {
  const normalize = renderToyVisual('normalization-nb2', 0)
  assert.match(normalize, /Before: raw counts/)
  assert.match(normalize, /After: median-ratio correction/)
  assert.match(normalize, /Paired bars now match/)
  const effect = renderToyVisual('normalization-nb2', 1)
  assert.match(effect, /log2FC = \+1/)
  const bh = renderToyVisual('fdr-volcano', 0)
  assert.match(bh, /rank 4/)
  assert.match(bh, /q ≤ 0.05/)
  const pca = renderToyVisual('pca-clustering', 0)
  assert.match(pca, /PC1: 100% toy variance/)
  const clustering = renderToyVisual('pca-clustering', 1)
  assert.match(clustering, /1.41/)
  assert.match(clustering, /8.49/)
  const clr = renderToyVisual('transcript-usage', 0)
  assert.match(clr, /CLR\(T1\) \+0.923/)
  assert.match(clr, /CLR\(T1\) −0.923/)
  const auc = renderToyVisual('longitudinal', 0)
  assert.match(auc, /AUC = 5\/8 = 0.625/)
})

test('volcano toy plots track FDR and log2 cutoff changes without refitting the model', () => {
  assert.deepEqual(classifyToyVolcano({ fdr: 0.05, effect: 1 }).map((p) => p.classification), ['up', 'neutral', 'neutral'])
  assert.deepEqual(classifyToyVolcano({ fdr: 0.10, effect: 0.5 }).map((p) => p.classification), ['up', 'down', 'down'])
  assert.deepEqual(classifyToyVolcano({ fdr: 0.01, effect: 2 }).map((p) => p.classification), ['neutral', 'neutral', 'neutral'])
  assert.equal(volcanoSummary({ fdr: 0.05, effect: 1 }), '1 of 3 pass both cutoffs (1 up, 0 down)')
  assert.equal(volcanoSummary({ fdr: 0.10, effect: 0.5 }), '3 of 3 pass both cutoffs (1 up, 2 down)')
  assert.deepEqual(sanitizeToyThresholds({ fdr: 999, effect: -3 }), { fdr: 0.2, effect: 0 })
  assert.deepEqual(sanitizeToyThresholds({ fdr: 'bad', effect: 'bad' }), { fdr: 0.05, effect: 1 })
  assert.match(renderToyVolcano({ fdr: 0.05, effect: 1 }), /−log10\(q\)/)
  assert.match(renderToyVisual('fdr-volcano', 1), /data-toy-fdr/)
  assert.match(renderToyVisual('fdr-volcano', 1), /data-toy-effect/)
})

test('volcano controls update only their chart slot, output numbers and selection summary', () => {
  const handlers = {}
  const slot = { innerHTML: '' }
  const outputs = {
    '[data-toy-fdr-output]': { textContent: '' },
    '[data-toy-effect-output]': { textContent: '' },
    '[data-toy-count]': { textContent: '' },
  }
  const fdr = { value: '0.10', addEventListener: (kind, callback) => { handlers[`fdr:${kind}`] = callback } }
  const effect = { value: '0.5', addEventListener: (kind, callback) => { handlers[`effect:${kind}`] = callback } }
  const container = { querySelector: (selector) => selector === '[data-toy-chart-slot]' ? slot : null }
  const group = {
    closest: () => container,
    querySelector: (selector) => ({ '[data-toy-fdr]': fdr, '[data-toy-effect]': effect, ...outputs })[selector],
  }
  bindToyVisualControls({ querySelectorAll: (selector) => selector === '[data-toy-volcano]' ? [group] : [] })
  handlers['fdr:input']()
  assert.match(slot.innerHTML, /<svg/)
  assert.equal(outputs['[data-toy-fdr-output]'].textContent, '0.10')
  assert.equal(outputs['[data-toy-effect-output]'].textContent, '0.50')
  assert.equal(outputs['[data-toy-count]'].textContent, '3 of 3 pass both cutoffs (1 up, 2 down)')
})

test('visual examples are client-only and scripts use one consistent cache revision', async () => {
  const [visuals, examples, main, html, styles] = await Promise.all([
    read('docs-toy-visuals.js'), read('docs-toy-examples.js'), read('main.js'), read('index.html'), read('styles.css'),
  ])
  assert.doesNotMatch(visuals, /\bfetch\s*\(|\bXMLHttpRequest\b|navigator\.permissions|localStorage|WebSocket/)
  assert.match(examples, /renderToyVisual\(sectionId, index\)/)
  assert.match(main, /bindToyVisualControls\(document\)/)
  assert.match(styles, /\.docs-visual-svg\s*\{/)
  assert.match(styles, /\.docs-toy-math\s*\{/)
  const v = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(v, '0.21.0-ux11-interactive-exports')
  assert.doesNotMatch(main, /from ['"]\.\/docs-toy-visuals/) // imported through docs-toy-examples
  assert.match(examples, /docs-toy-visuals\.js\?v=0\.21\.0-ux11-interactive-exports/)
  assert.match(html, /styles\.css\?v=0\.21\.0-ux11-interactive-exports/)
  assert.match(html, /main\.js\?v=0\.21\.0-ux11-interactive-exports/)
})
