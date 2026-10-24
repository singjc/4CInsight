import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const styles = await readFile(new URL('../styles.css', import.meta.url), 'utf8')

test('W3 PCA and dendrogram expose end-user plot interactions', () => {
  assert.match(main, /function bindInteractivePlots\(\)/)
  assert.match(main, /data-plot-action="in"/)
  assert.match(main, /data-plot-action="fit"/)
  assert.match(main, /data-plot-group-toggle="case"/)
  assert.match(main, /data-plot-group-toggle="control"/)
  assert.match(main, /event\.ctrlKey/)
  assert.match(main, /event\.metaKey/)
  assert.match(main, /pointerdown/)
  assert.match(main, /data-plot-group="\$\{group\}"/)
  assert.equal(main.includes('labelEvery'), false, 'every dendrogram leaf label should be rendered for zoom inspection')
  assert.match(styles, /\.legend-dot\.case-point \{ background:/)
  assert.match(styles, /\.legend-dot\.control-point \{ background:/)
})
