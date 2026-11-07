import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { renderResultsOverview } from '../results-overview.js'
import {
  parsePlotBounds, zoomPlotBounds, panPlotBounds, rowsToCsv, csvEscape,
  availableResultTables, tableColumns,
} from '../results-interactions.js'
const main = await readFile(new URL('../main.js',import.meta.url), 'utf8')
const styles = await readFile(new URL('../styles.css',import.meta.url), 'utf8')
const controls = await readFile(new URL('../results-interactions.js',import.meta.url), 'utf8')
const fixture = () => ({
  gene:{analysis:{tests:[{feature:'A',beta_group:2.3,q_value:.002},{feature:'B',beta_group:.1,q_value:.2}]}},
  transcript:{analysis:{tests:[{feature:'T1',beta_group:-2,q_value:.002}]}},
  bcr:{analysis:{tests:[],sample_summaries:[{sample_id:'S1',group:'Case',clonality:.4,observed_richness:150},{sample_id:'S2',group:'Control',clonality:.1,observed_richness:100}],compositions:[{family:'V',tests:[{feature:'IGHV1',beta_group:.2,q_value:.1}]}]}},
  tcrAlpha:{analysis:{sample_summaries:[{sample_id:'S1',group:'Case',clonality:.3}]}},
  tcrBeta:{analysis:{sample_summaries:[{sample_id:'S2',group:'Control',clonality:.5}]}},
  isotype:{analysis:{tests:[{feature:'IgG',beta_group:-.25,q_value:.01}]}},
  immune:{analysis:{tests:[{feature:'NK cells',beta_group:10,q_value:.03}]}},
  omicsErrors:{},
})
test('UX11: all seven overview cards have an Expand control and a chart export location', () => {
  const html=renderResultsOverview(fixture(), {fdr:.05,absLog2Fc:1})
  for (const key of ['gene','transcript','bcr','tcrAlpha','tcrBeta','isotype','immune']) {
    assert.match(html, new RegExp(`data-overview-modality="${key}"`))
    assert.match(html, new RegExp(`data-overview-expand="${key}"`))
  }
  assert.equal((html.match(/overview-card-tools/g)||[]).length,7)
  assert.equal((html.match(/results-export-error/g)||[]).length,7)
})
test('UX11: expansion can survive re-render and does not change the result numbers', () => {
  const small=renderResultsOverview(fixture(),{fdr:.05})
  const large=renderResultsOverview(fixture(),{fdr:.05,expandedModalities:['bcr','immune']})
  assert.match(large,/overview-expanded" data-overview-modality="bcr"/)
  assert.match(large,/overview-expanded" data-overview-modality="immune"/)
  assert.match(large,/data-overview-expand="bcr" aria-expanded="true">Reduce/)
  assert.match(small,/data-overview-expand="bcr" aria-expanded="false">Expand/)
  assert.match(large,/1 of 2 adjusted tests meet the selected threshold/)
})
test('UX11: zoom and pan clamp to the original SVG view and preserve aspect ratio', () => {
  const base=parsePlotBounds('0 0 660 230')
  let view=zoomPlotBounds(base,base,.5,.75,.25)
  assert.equal(view.width,330)
  assert.equal(view.height,115)
  assert.equal(view.x,247.5)
  assert.equal(view.y,28.75)
  view=panPlotBounds(view,base,1200,-1200,600,209)
  assert.equal(view.x,0)
  assert.equal(view.y,115)
  view=zoomPlotBounds(view,base,100)
  assert.deepEqual(view,base)
  let deep=base
  for(let i=0;i<20;i++)deep=zoomPlotBounds(deep,base,.5)
  assert.equal(deep.width,660/16)
  assert.equal(deep.height,230/16)
  assert.equal(parsePlotBounds('bad'),null)
})
test('UX11: CSV escapes fields and preserves all unioned columns with exact numeric values', () => {
  const data=[{sample_id:'S1',effect:-.875,feature:'A,"B"',comment:'first\nsecond'},
    {sample_id:'S2',effect:0,extra:'=1+2'}]
  assert.deepEqual(tableColumns(data),['sample_id','effect','feature','comment','extra'])
  const csv=rowsToCsv(data)
  assert.match(csv,/A,""B""/)
  assert.match(csv,/-0.875/)
  assert.match(csv,/0,,,'=1\+2/)
  assert.match(csv,/"first\nsecond"/)
  assert.equal(csvEscape('=SUM(A1)'), "'=SUM(A1)")
  assert.equal(csvEscape(-12), '-12')
})
test('UX11: exports all scientific rows, including composition and transcript usage tables', () => {
  const entry=fixture().bcr
  const tables=availableResultTables(entry)
  assert.deepEqual(tables.map(x=>x.kind),['sample-metrics','composition-V'])
  assert.equal(tables[0].rows.length,2)
  assert.equal(tables[1].rows[0].feature,'IGHV1')
  const tx={analysis:{tests:[{feature:'T1'}],dtu:{gene_tests:[{gene_id:'ENSG1',q_value:.04}]}}}
  assert.deepEqual(availableResultTables(tx).map(x=>x.kind),['adjusted-tests','transcript-usage'])
})
test('UX11: CSV export has no plot-thinning or visible-table row cap', () => {
  const rows=Array.from({length:10007},(_,i)=>({feature:`GENE_${i}`,q_value:i/10007}))
  const csv=rowsToCsv(rows)
  assert.equal(csv.trimEnd().split('\r\n').length,10008)
  assert.match(csv,/GENE_10006/)
})
test('UX11: interactive controls are browser-local and do not refit analysis', () => {
  assert.match(main,/bindOverviewInteractions\(document, state\.compareResult/)
  assert.match(main,/bindDetailedResultsExports\(document, state\.compareResult\)/)
  assert.match(main,/expandedModalities: state\.resultsExpanded/)
  assert.match(controls,/downloadPlotSvg/)
  assert.match(controls,/downloadPlotPng/)
  assert.match(controls,/querySelectorAll\('\.comparison-result'\)/)
  assert.match(controls,/plot\.querySelector\('svg'\)/)
  assert.doesNotMatch(controls,/\bfetch\s*\(|XMLHttpRequest|WebSocket|FileReader|\.readAsArrayBuffer/)
  assert.match(styles,/\.overview-expanded/)
  assert.match(styles,/\.results-plot-tools/)
})
