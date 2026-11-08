import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
const read=path=>readFileSync(new URL(path,import.meta.url),'utf8')
test('integrative page contains only align, fuse, joint structure and optional previous evidence',()=>{
  const main=read('../main.js')
  const page=main.split('function significantHitsView() {')[1].split('async function runSignificantHits() {')[0]
  assert.match(page,/i1AlignmentPanel\(\)/)
  assert.match(page,/i2DistancePanel\(\)/)
  assert.match(page,/jointStructurePanel\(\)/)
  assert.doesNotMatch(page,/Expression-only selected-hit structure|Receptor and immune adjusted findings|run-significant-hits|Comparison required|I[012][ABC]?\s*·/)
  assert.doesNotMatch(page,/No joint model is fitted|Run Compare Groups first/)
  assert.match(main,/function runSignificantHits\(\)/,'Original expression hit engine retained for later Significant Findings view')
  assert.match(main,/function renderAdjustedOmicsReview\(\)/,'Previous adjusted-result rendering retained')
})
test('scientific fixed-view integration runs Rust fusion first, then strict structure in a worker',()=>{
  const main=read('../main.js'),worker=read('../analysis-worker.js'),science=read('../joint-structure.js')
  assert.match(main,/prepareJointDistanceInput\(state\.alignmentPreview,views,\{minViews:views\.length\}\)/)
  assert.match(main,/workerRequest\('run-integrative-distance',\{input\}\)/)
  assert.match(main,/workerRequest\('run-joint-structure'/)
  assert.match(worker,/analyzeJointStructure\(request\.result,\s*request\.options/)
  assert.match(science,/non_fixed_pairs/)
  assert.doesNotMatch(science,/\b(?:beta_group|q_value|caseValues|controlValues|significant_hits)\b/)
  assert.match(science,/No phenotype\/diagnosis\/case-control labels enter this function/,
    'The kernel must document why group labels cannot enter the fit')
})
test('user interface uses descriptive names instead of developer phase labels',()=>{
  const main=read('../main.js')
  const page=main.slice(main.indexOf('function jointStructurePanel()'), main.indexOf('function significantHitsView()'))
  assert.match(page,/Joint ordination and clustering/)
  assert.match(page,/fixed set of data types/)
  assert.doesNotMatch(page,/<span class="tag neutral">I[012]/)
  assert.match(main,/const APP_VERSION = '0\.22\.0'/)
  assert.match(read('../index.html'),/main\.js\?v=0\.22\.0/)
})
