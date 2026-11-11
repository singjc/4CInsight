import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {pathwaySelectionKey,pathwayEvidenceBanner,pathwayScoreChartHtml,
  inspectPathwayGenes,pathwayInspectorHtml,pathwayInspectorTsv} from '../pathway-inspector.js'
const rows=[{gene:'A',score:0.8,q:null},{gene:'B',score:0.4,q:null},{gene:'C',score:-0.3,q:null},{gene:'D',score:-0.6,q:null}]
const input={rows}
const sets={pathways:[{name:'Immune signaling',description:'R-HSA-1',genes:['A','B','C','NOT_MEASURED']}]}
const ranked={source:'exploratory',mode:'ranked',method:'Weighted KS, gene-label permutations',
  geneSetFile:'Reactome.gmt',organism:'',geneSetRelease:'',options:{permutations:199},
  results:[{pathway:'Immune signaling',description:'R-HSA-1',overlap:3,matchedSetSize:3,
    normalizedScore:1.7,enrichmentScore:.62,p_value:.005,q_value:.018,leadingGenes:['A','B']},
    {pathway:'Second',description:'R-HSA-2',normalizedScore:-1.2,p_value:.005,q_value:.018,leadingGenes:[]}]}
const compare={...ranked,mode:'ora',source:'compare',method:'Hypergeometric right tail',
  results:[{...ranked.results[0],foldEnrichment:2.1,leadingGenes:['B']}]}

test('exploratory warning explains tied empirical q values and exactly states resolution',()=>{
 const html=pathwayEvidenceBanner(ranked)
 assert.match(html,/0\.005/)
 assert.match(html,/199 permutations/)
 assert.match(html,/not establish disease pathways/)
 assert.match(html,/Axis orientation/)
 assert.match(pathwayEvidenceBanner(compare),/Predefined comparison/)
 assert.doesNotMatch(pathwayEvidenceBanner(compare),/199 permutations/)
})
test('signed NES chart separates positive and negative effects, displays exact BH q values',()=>{
 const html=pathwayScoreChartHtml(ranked)
 assert.match(html,/normalized enrichment score/)
 assert.match(html,/pathway-score-bar positive/)
 assert.match(html,/pathway-score-bar negative/)
 assert.match(html,/q 0\.018/)
 assert.match(html,/data-pathway-index="1"/)
 assert.doesNotMatch(html,/−log10\(BH q\)/)
 assert.match(pathwayScoreChartHtml(compare),/Fold enrichment/)
})
test('inspection selects only original GMT genes with valid scored results',()=>{
 const got=inspectPathwayGenes(ranked,ranked.results[0],input,sets)
 assert.equal(got.available,true)
 assert.deepEqual(got.rows.map(r=>r.gene),['A','B','C'])
 assert.deepEqual(got.rows.map(r=>r.selected),[true,true,false])
 assert.equal(got.matched,3)
 assert.equal(got.selectedCount,2)
 assert.equal(got.rows[0].rank,1)
 assert.equal(got.rows[2].score,-0.3)
 assert.equal(got.rows[1].q,null)
 assert.match(pathwayInspectorHtml(ranked,ranked.results[0],got),/not necessarily differentially expressed/)
})
test('ORA selection is the original reported set; gene q and effects are preserved',()=>{
 const inp={rows:[{gene:'A',score:1.5,q:.03},{gene:'B',score:-2,q:.002},{gene:'C',score:0.1,q:.9}]}
 const x=inspectPathwayGenes(compare,compare.results[0],inp,sets)
 assert.deepEqual(x.rows.map(g=>g.selected),[true,false,false])
 assert.deepEqual(x.rows.map(g=>g.gene),['B','A','C'])
 const html=pathwayInspectorHtml(compare,compare.results[0],x)
 assert.match(html,/Group log2 effect/)
 assert.match(html,/Corrected gene q-value/)
 assert.match(html,/0\.002/)
 const tsv=pathwayInspectorTsv(compare,compare.results[0],x)
 assert.match(tsv,/B\t2\t-2\t0\.002\tyes/)
 assert.equal(tsv.trim().split('\n').length,4)
})
test('reject absent, ambiguous and unmatched gene-set membership rather than guessing',()=>{
 const row=ranked.results[0]
 assert.equal(inspectPathwayGenes(ranked,row,input,null).available,false)
 assert.equal(inspectPathwayGenes(ranked,row,input,{pathways:[...sets.pathways,...sets.pathways]}).available,false)
 assert.equal(inspectPathwayGenes(ranked,{...row,description:'different'},input,sets).available,false)
 assert.throws(()=>pathwayInspectorTsv(ranked,row,{available:false}),/Select a pathway/)
 assert.equal(pathwaySelectionKey({pathway:'A',description:'X'}),pathwaySelectionKey({pathway:'A',description:'X'}))
 assert.notEqual(pathwaySelectionKey({pathway:'A',description:'X'}),pathwaySelectionKey({pathway:'A',description:'Y'}))
})
test('user-controlled names are escaped and TSV formula injection is neutralized',()=>{
 const row={...ranked.results[0],pathway:'<img src=x onerror=alert(1)>'}
 const gmt={pathways:[{name:row.pathway,description:row.description,genes:['=HYPERLINK','B']}]}
 const data={rows:[{gene:'=HYPERLINK',score:.9,q:null},{gene:'B',score:.2,q:null}]}
 const inspected=inspectPathwayGenes(ranked,row,data,gmt)
 const html=pathwayInspectorHtml(ranked,row,inspected)
 assert.match(html,/&lt;img/)
 assert.doesNotMatch(html,/<img src=/)
 const tsv=pathwayInspectorTsv(ranked,row,inspected)
 assert.match(tsv,/'=HYPERLINK/)
 assert.match(tsv,/<img src=x onerror=alert\(1\)>/)
})
test('P3 is presentation only, view tab is exclusive and original statistical engine is not changed',()=>{
 const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
 const view=readFileSync(new URL('../pathway-view.js',import.meta.url),'utf8')
 const inspector=readFileSync(new URL('../pathway-inspector.js',import.meta.url),'utf8')
 assert.match(main,/const overview=state\.integrativeView==='overview'/)
 assert.match(main,/aria-controls="integrative-overview-panel" aria-selected="\$\{overview\}"/)
 assert.match(main,/pathwaySelectedKey: state\.pathwaySelectedKey/)
 assert.match(main,/event\.key==='ArrowRight'/)
 assert.match(main,/event\.key==='Home'/)
 assert.match(main,/typeof saved\.pathwaySelectedKey === 'string'/)
 assert.match(view,/pathwayEvidenceBanner\(report\)/)
 assert.match(view,/pathwayScoreChartHtml\(report\)/)
 assert.match(view,/inspectPathwayGenes\(report,row,prepared,workspace\.pathwayGeneSets\)/)
 assert.match(view,/id="pathway-gene-inspector"/)
 assert.match(view,/pathwayInspectorTsv\(report,row,inspection\)/)
 assert.doesNotMatch(inspector,/\bfetch\s*\(|XMLHttpRequest|runPathwayEnrichment|hypergeometricTail|bhAdjust|permutations\s*\(/)
})
