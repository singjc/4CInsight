import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { renderResultsOverview } from '../results-overview.js'
const main=await readFile(new URL('../main.js',import.meta.url),'utf8')
const css=await readFile(new URL('../styles.css',import.meta.url),'utf8')
const example = () => ({
  gene:{analysis:{n_model_samples:8,tests:[{feature:'GENE1',beta_group:1.4,q_value:0.01},{feature:'GENE2',beta_group:0.2,q_value:0.02}]}},
  transcript:{analysis:{n_model_samples:8,tests:[{feature:'TX1',beta_group:-1.7,q_value:0.04}]}},
  bcr:{analysis:{sample_summaries:[{sample_id:'A<1',group:'Case',observed_richness:100,clonality:0.3,public_fraction:0.02},{sample_id:'B2',group:'Control',observed_richness:80,clonality:0.2,public_fraction:0.03}],tests:[{feature:'clonality',beta_group:0.1,q_value:0.01}]},comparison:{case_label:'Case',control_label:'Control'}},
  tcrAlpha:{analysis:{sample_summaries:[{sample_id:'A',group:'Case',clonality:0.4},{sample_id:'B',group:'Control',clonality:0.2}],tests:[]}},
  tcrBeta:{analysis:{sample_summaries:[{sample_id:'A',group:'Case',clonality:0.1},{sample_id:'B',group:'Control',clonality:0.3}],tests:[]}},
  isotype:{analysis:{tests:[{feature:'IgG',beta_group:-0.21,q_value:0.02},{feature:'IgM',beta_group:0.1,q_value:0.8}]}},
  immune:{analysis:{tests:[{feature:'T cells',beta_group:0.42,q_value:0.03},{feature:'NK cells',beta_group:-0.15,q_value:0.2}]}},
  omicsErrors:{},
})
test('overview presents all seven data types and their detailed links',()=>{
  const html=renderResultsOverview(example(),{fdr:0.05,absLog2Fc:1})
  for (const key of ['gene','transcript','bcr','tcrAlpha','tcrBeta','isotype','immune']) {
    assert.match(html,new RegExp(`data-overview-modality="${key}"`))
    assert.match(html,new RegExp(`data-results-detail="${key}"`))
  }
})
test('overview shows exact FDR selection counts without refitting model',()=>{
  const html=renderResultsOverview(example(),{fdr:0.05,absLog2Fc:1})
  assert.match(html,/1 of 2 adjusted tests meet the selected threshold/)
  assert.match(html,/GENE1/)
  assert.doesNotMatch(html,/2 of 2 adjusted tests meet the selected threshold/)
})
test('sample labels are escaped and charts have accessible text',()=>{
  const html=renderResultsOverview(example(),{})
  assert.match(html,/A&lt;1/)
  assert.doesNotMatch(html,/A<1/)
  assert.match(html,/aria-label="Receptor sample distributions/)
})
test('isotypes and immune signatures use signed effects with correct cautions',()=>{
  const html=renderResultsOverview(example(),{})
  assert.match(html,/relative composition effects, not antibody concentrations/)
  assert.match(html,/not estimates of immune-cell percentages/)
  assert.match(html,/Lower in case/)
})
test('missing modalities and errors do not hide successful results',()=>{
  const result=example();delete result.tcrBeta; result.omicsErrors['tcr-beta']='Unusable <file>'
  const html=renderResultsOverview(result,{})
  assert.match(html,/6 of 7 data types produced results/)
  assert.match(html,/Unusable &lt;file&gt;/)
  assert.doesNotMatch(html,/data-overview-modality="tcrBeta"/)
})
test('source keeps the old detail renderer and history',()=>{
  assert.match(main,/function detailedComparisonHtml\(/)
  assert.match(main,/function resultsView\(/)
  assert.match(main,/function comparisonResultSection\(/)
  assert.match(main,/function multiomicsResultSection\(/)
  assert.match(main,/\['history', 'History', '6'\]/)
  assert.match(main,/\['results', 'Results', 'R'\]/)
})
test('navigation, status, styles and error view exist',async()=>{
  assert.match(main,/state\.resultsMode = 'overview'/)
  assert.match(main,/data-results-view="detailed"/)
  const overviewModule=await readFile(new URL('../results-overview.js',import.meta.url),'utf8')
  assert.match(overviewModule,/omicsErrors/)
  assert.match(css,/\.overview-modality-grid/)
  assert.match(css,/\.results-view-switch/)
})
