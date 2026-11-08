import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8')

test('joint heatmap has explicit original-distance labels, expand, independent row/column zoom and dendrograms',()=>{
  const main=read('../main.js')
  const viewer=read('../joint-heatmap-viewer.js')
  const css=read('../styles.css')
  const panel=main.slice(main.indexOf('function jointStructurePanel()'),main.indexOf('async function runJointStructure()'))
  assert.match(main,/heatmapViewerMarkup\(\)/)
  assert.match(main,/mountJointHeatmapViewer\(state\.i2StructureResult\)/)
  assert.match(panel,/joint-heatmap-card/)
  assert.match(panel,/jointDistanceReviewMarkup\(result\)/)
  assert.doesNotMatch(panel,/id="joint-structure-heatmap"|joint-cluster-scroll/)
  assert.match(viewer,/boxToWindow\(/)
  assert.match(viewer,/joint-top-tree/)
  assert.match(viewer,/joint-left-tree/)
  assert.match(viewer,/original clustering/)
  assert.match(viewer,/95th percentile/)
  assert.match(viewer,/normalized|Normalized/)
  assert.match(css,/\.joint-heatmap-viewer\.is-expanded/)
  assert.doesNotMatch(viewer,/fetch\(|XMLHttpRequest|new WebSocket|\/api\//)
})

test('all external app assets refer to the declared application cache revision',()=>{
  const main=read('../main.js'),html=read('../index.html')
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version,'0.22.6')
  for(const token of [`main.js?v=${version}`,`styles.css?v=${version}`])assert.ok(html.includes(token))
  assert.ok(main.includes(`joint-heatmap-viewer.js?v=${version}`))
  assert.ok(main.includes(`joint-structure.js?v=${version}`))
})
