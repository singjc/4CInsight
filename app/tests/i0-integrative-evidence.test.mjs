import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'
import { INTEGRATIVE_MODALITIES, summarizeIntegrativeEvidence, integrativeSummaryTotals, collectIntegrativeSelectedRows } from '../integrative-evidence.js'

const app = fileURLToPath(new URL('../', import.meta.url))
const read = path => readFileSync(join(app, path), 'utf8')
const testRow = (feature, q, effect) => ({feature, q_value:q, beta_group:effect})

test('I0 summarizes all seven modalities without pretending to fit a joint model', () => {
  assert.equal(INTEGRATIVE_MODALITIES.length, 7)
  const comparison = {
    gene: {analysis: {n_model_samples: 10, tests: [testRow('G',0.01,1.2),testRow('G2',0.2,2)]}},
    transcript: {analysis: {n_model_samples: 9, tests:[testRow('T',0.01,0.5)]}},
    bcr: {analysis: {files_used: 8, metric_tests:[testRow('clonality',0.02,0.2)],compositions:[]}},
    tcrAlpha: {analysis: {files_used:7,metric_tests:[], compositions:[{family:'V', tests:[testRow('TRAV1',0.01,0.5)]}]}},
    tcrBeta: {analysis: {files_used:7,metric_tests:[],compositions:[]}},
    isotype: {analysis: {files_used:8,tests:[testRow('IgG',0.02,0.1)]}},
    immune: {analysis: {files_used:9, tests:[testRow('NK',0.001,0.3)]}},
  }
  const rows = summarizeIntegrativeEvidence(comparison, {fdr:0.05,absLog2Fc:1,omicsMinAdjustedEffect:0.15})
  assert.deepEqual(rows.map(r=>r.key), ['gene','transcript','bcr','tcrAlpha','tcrBeta','isotype','immune'])
  assert.deepEqual(rows.map(r=>r.selected), [1,0,1,1,0,0,1])
  assert.equal(rows[0].samples,10)
  assert.deepEqual(integrativeSummaryTotals(rows), {included:7,available:7,adjustedTests:7,selectedTests:4})
  assert.equal(rows[2].effectType,'Modality-specific adjusted effect')
})

test('I0 reports absent modalities and respects explicit user selection', () => {
  const rows = summarizeIntegrativeEvidence({gene:{analysis:{tests:[testRow('A',0.02,1)]}}}, {fdr:0.05,absLog2Fc:0}, {gene:false,bcr:true})
  assert.equal(rows[0].included,false)
  assert.equal(rows[1].available,false)
  assert.equal(rows[2].available,false)
  assert.deepEqual(integrativeSummaryTotals(rows), {included:0,available:0,adjustedTests:0,selectedTests:0})
})

test('I0 ignores missing, nonfinite and zero-denominator scientific tests', () => {
  const rows = summarizeIntegrativeEvidence({gene:{analysis:{tests:[
    testRow('null',null,2),testRow('NaN','NaN',3),testRow('missing',0.01,null),testRow('good',0.01,1),
  ]}}}, {fdr:0.05,absLog2Fc:0.5})
  assert.equal(rows[0].selected,1)
})

test('I0 keeps expression feature effects separate from modality-specific adjusted effects', () => {
  const rows = summarizeIntegrativeEvidence({
    gene:{analysis:{tests:[testRow('G',0.04,0.1)]}},
    isotype:{analysis:{tests:[testRow('IgA',0.04,0.1)]}},
  }, {fdr:0.05,absLog2Fc:1,omicsMinAdjustedEffect:0})
  assert.equal(rows[0].selected,0)
  assert.equal(rows[5].selected,1)
})

test('I0 full-width corrected tables have scroll containers, sticky headers and no grid stretch', () => {
  const main = read('main.js')
  const css = read('styles.css')
  assert.match(main, /h0-modality-grid/)
  assert.match(main, /h0-modality-card/)
  assert.match(main, /h0-hits-scroll/)
  assert.doesNotMatch(main.match(/function renderAdjustedOmicsReview\(\)[\s\S]*?function significantHitsView\(\)/)?.[0] || '', /<div class="config-grid expression-source-grid">\$\{sections\}/)
  assert.match(css, /\.h0-modality-grid\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1fr\)/)
  assert.match(css, /\.h0-hits-scroll\s+table\s*\{[^}]*min-width:/)
  assert.match(css, /\.h0-hits-scroll\s+thead\s+th\s*\{[^}]*position:\s*sticky/)
})

test('I0 navigation is clear about current evidence versus future true joint analysis', () => {
  const main=read('main.js')
  assert.match(main,/\['hits', 'Integrative Analysis', '5'\]/)
  assert.match(main,/Corrected evidence across data types/)
  assert.match(main,/function i2DistancePanel\(\)/)
  assert.match(main,/function significantPanelSection\(/)
  assert.match(main,/function runSignificantHits\(/)
  assert.match(main,/function runSignificantHits\(\)/)
  assert.match(main,/integrative-evidence\.js\?v=0\.22\.0/)
  const index=read('index.html')
  const rev=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.ok(rev)
  assert.ok(index.includes(`main.js?v=${rev}`))
  assert.ok(index.includes(`styles.css?v=${rev}`))
})

test('I0 exports selected evidence from all seven data types without combining effect scales', () => {
  const compare = {gene:{analysis:{tests:[testRow('G',0.001,1.5)]}},
    transcript:{analysis:{tests:[testRow('T',0.02,1.2)]}},
    bcr:{analysis:{metric_tests:[testRow('Clonality',0.04,0.3)]}},
    isotype:{analysis:{tests:[testRow('IgA',0.04,0.3)]}},
    immune:{analysis:{tests:[testRow('NK',0.01,0.2)]}}}
  const rows=collectIntegrativeSelectedRows(compare,{fdr:0.05,absLog2Fc:1,omicsMinAdjustedEffect:0.1})
  assert.deepEqual(rows.map(row=>row.modality),
    ['Gene expression','Transcript expression','BCR repertoire','BCR isotypes','Immune infiltration'])
  assert.match(rows[0].family,/log2 effect/)
  assert.match(rows[3].family,/isotype CLR/)
  assert.equal(rows[0].q_value,0.001)
  assert.equal(rows.length, summarizeIntegrativeEvidence(compare,{fdr:0.05,absLog2Fc:1,omicsMinAdjustedEffect:0.1}).reduce((sum,group)=>sum+group.selected,0))
  const chosen=collectIntegrativeSelectedRows(compare,{fdr:0.05,absLog2Fc:1,omicsMinAdjustedEffect:0.1},{isotype:false})
  assert.equal(chosen.some(row=>row.modality==='BCR isotypes'),false)
  assert.equal(chosen.length,4)
})
