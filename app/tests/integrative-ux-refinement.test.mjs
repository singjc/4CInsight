import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {integrativeSummaryHtml,integrativeInfluenceHtml,integrativeScatterMarkup,bindIntegrativeScatter} from '../integrative-ux.js'
import {modalityOverviewHtml} from '../biological-dashboard.js'
const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
const style=readFileSync(new URL('../styles.css',import.meta.url),'utf8')

test('summary distinguishes eligible, complete-case and excluded observations without inventing results',()=>{
 const html=integrativeSummaryHtml({eligible:322,retained:237,modalities:7,clusters:3,ari:.961})
 for(const text of ['322','237','85','7 / 7','3 descriptive clusters','0.961','does not validate biological'])assert.ok(html.includes(text),`Missing ${text}`)
 const unknown=integrativeSummaryHtml({eligible:null,retained:null,modalities:0,clusters:null,ari:null})
 assert.match(unknown,/Not available|not available|—/)
 assert.doesNotMatch(unknown,/NaN|undefined|null/i)
})
test('sensitivity cards use only fixed-cohort scientific ablation values and show unavailable reasons',()=>{
 const structure={fixed_ablation:[{omitted:'gene',available:true,distance_correlation:.904,nearest_neighbor_retention:.876},
  {omitted:'bcr',available:false,reason:'Too few shared pairs'}]}
 const html=integrativeInfluenceHtml(structure)
 for(const text of ['Gene expression','0.904','87.6%','BCR repertoire','Too few shared pairs','same-cohort'])assert.ok(html.includes(text),text)
 assert.doesNotMatch(html,/NaN|width:undefined/)
 assert.match(integrativeInfluenceHtml({fixed_ablation:[]}),/At least three fitted/)
})
test('joint scatter shows sample counts, positive-inertia labels, patient inspection, selectable metadata',()=>{
 const report={column:'QC group',metadata:{kind:'categorical',too_many_categories:false,categories:[{value:'A',n:1}]},
  membership:[{cluster:1,observations:1},{cluster:2,observations:2}],
  points:[{observation:'sample<1>',x:0,y:1,cluster:1,annotation:'A'},
   {observation:'sample2',x:3,y:4,cluster:2,annotation:''}],
  coverage:[{observation:'sample<1>',cluster:1,patient:'person<1>',timepoint:'V1'}]}
 const html=integrativeScatterMarkup(report,[.412,.213])
 for(const text of ['41.2% of positive inertia','21.3% of positive inertia','Cluster 2: 2 observations','data-integrative-point','data-scatter-inspector','data-scatter-action="in"','Inspect distant plotted observations','distinct patients','Patient: person&lt;1&gt;'])assert.ok(html.includes(text),text)
 assert.ok(html.includes('sample&lt;1&gt;'))
 assert.doesNotMatch(html,/<1>|person<1>/)
 assert.equal(typeof bindIntegrativeScatter,'function')
})
test('numeric annotation renders measured gradient instead of misleading cluster colours',()=>{
 const report={column:'Age',metadata:{kind:'numeric',range:[10,90]},membership:[],
   points:[{observation:'a',x:0,y:0,cluster:1,annotation:'60'},{observation:'b',x:1,y:1,cluster:1,annotation:''}]}
 const html=integrativeScatterMarkup(report,[.5,.2])
 assert.match(html,/rgb\(/)
 assert.match(html,/grey = missing/)
})
test('all modality ratio tiles use shared data-derived scale, not old capped ratio of 2',()=>{
 const html=modalityOverviewHtml({fixed_modalities:['gene','bcr'],modality:[
  {modality:'gene',pairs:3,expected_pairs:3,between_within_ratio:9.316,joint_distance_correlation:.5},
  {modality:'bcr',pairs:3,expected_pairs:3,between_within_ratio:5.034,joint_distance_correlation:.2},
 ]})
 assert.match(html,/shared visual range 0 to 9.316/)
 assert.match(html,/width:54.0%/) // formerly both bars saturated at 100%
 assert.match(html,/width:100.0%/)
})
test('main UI binds real controls, keeps science unchanged and does not save overlays in public Docs',()=>{
 for(const token of ['integrative-overview-annotation','integrativeInfluenceHtml(structure','bindIntegrativeScatter(document,showIntegrativeSampleMetadata)',
  'integrative-step-disclosure','integrative-advanced-settings','integrative-setting-one-patient',
  'The final joint ordination requires complete distances','Feature rankings are post-fit associations',
  'state.bioResult=null;state.bioError=\'\';render()'])assert.ok(main.includes(token),token)
 assert.match(style,/integrative-interactive-scatter/)
 assert.match(main,/id="integrative-run-complete"/)
 assert.match(main,/await runJointStructure\(\)/)
 assert.doesNotMatch(readFileSync(new URL('../integrative-ux.js',import.meta.url),'utf8'),/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
})
