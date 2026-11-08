import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
const read=name=>readFileSync(new URL('../'+name,import.meta.url),'utf8')

test('seven source views use a bounded independent-worker queue',()=>{
  const main=read('main.js')
  const section=main.slice(main.indexOf('async function i2bRunDirect() {'),main.indexOf('async function i2ReadLocalFiles('))
  assert.match(section,/runBoundedSourceTasks\(requests/)
  assert.match(section,/parallel:state\.i2bParallelWorkers/)
  assert.match(section,/createWorker:.*new Worker/)
  assert.match(section,/signal:controller\.signal/)
  assert.match(section,/outcomes/)
  assert.doesNotMatch(section,/for \(let i = 0; i < requests\.length; i\+\+\)/)
  assert.doesNotMatch(section,/fetch\(|XMLHttpRequest|\/api\//)
})

test('source progress shows separate blue processing states and an accessible bar',()=>{
  const main=read('main.js'),css=read('styles.css')
  assert.match(main,/function i2bProgressMarkup\(\)/)
  assert.match(main,/function updateI2bProgressDom\(row,summary\)/)
  assert.match(main,/id="i2b-overall-progress"/)
  assert.match(main,/data-i2b-progress-row/)
  assert.match(main,/id="i2b-parallel-workers"/)
  assert.match(css,/\.i2b-progress-panel/)
  assert.match(css,/\.i2b-progress-row\[data-status="running"\]/)
  assert.match(css,/#3159d9/)
})

test('cache-bust version is coherent after release and stability interpretation is clear',()=>{
  const main=read('main.js'),html=read('index.html')
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version,'0.22.6')
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.ok(main.includes(`source-distance-queue.js?v=${version}`))
  assert.match(main,/How to interpret cluster stability/)
  assert.match(main,/do not identify biological pathways|does not identify biological pathways/)
})
