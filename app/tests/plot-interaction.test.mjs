import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const styles = await readFile(new URL('../styles.css', import.meta.url), 'utf8')

test('PCA zoom changes data coordinates and redraws axes instead of scaling a static SVG', () => {
  assert.match(main, /function renderPcaDomain\(shell, domain\)/)
  assert.match(main, /function pcaZoom\(shell, factor/)
  assert.match(main, /function pcaPan\(shell, dx, dy\)/)
  assert.match(main, /data-plot-kind="pca"/)
  assert.match(main, /data-x="\$\{Number\(point\.x\)\}"/)
  assert.match(main, /data-y="\$\{Number\(point\.y\)\}"/)
  assert.match(main, /tickValues\(domain\.minX, domain\.maxX\)/)
  assert.match(main, /plot-grid-line/)
  assert.match(main, /plot-tick/)
  assert.equal(main.includes('svg.style.width ='), false, 'plot zoom must not resize the static SVG element')
})

test('dendrogram zoom changes SVG coordinates and group legend remains interactive', () => {
  assert.match(main, /function renderDendrogramViewBox/)
  assert.match(main, /function dendrogramZoom/)
  assert.match(main, /function dendrogramPan/)
  assert.match(main, /data-plot-kind="dendrogram"/)
  assert.match(main, /data-plot-group-toggle="case"/)
  assert.match(main, /data-plot-group-toggle="control"/)
  assert.match(main, /event\.ctrlKey/)
  assert.match(main, /event\.metaKey/)
  assert.match(main, /pointerdown/)
  assert.equal(main.includes('labelEvery'), false, 'every dendrogram leaf label should remain available')
  assert.match(styles, /\.legend-dot\.case-point \{ background:/)
  assert.match(styles, /\.legend-dot\.control-point \{ background:/)
})
