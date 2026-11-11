import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {inspectPathwayGenes,pathwayInspectorHtml,pathwayInspectorTsv} from '../pathway-inspector.js'

const ranked={mode:'ranked',source:'exploratory',method:'Ranked permutation',options:{permutations:199}}
const row={pathway:'T-cell process',description:'R-HSA-42',leadingGenes:['C','E'],p_value:0.02,q_value:0.06,normalizedScore:1.4}
const input={rows:[{gene:'A',score:.9,q:null},{gene:'B',score:.8,q:null},{gene:'C',score:.7,q:null},{gene:'D',score:.6,q:null},{gene:'E',score:.5,q:null}]}
const sets={pathways:[{name:row.pathway,description:row.description,genes:['D','A','E','B','C']}]}
const inspected=inspectPathwayGenes(ranked,row,input,sets)

test('P3.1: saved leading-edge genes come first; original ranks and exact scores are preserved',()=>{
  assert.equal(inspected.available,true)
  assert.deepEqual(inspected.rows.map(g=>g.gene),['C','E','A','B','D'])
  assert.deepEqual(inspected.rows.map(g=>g.rank),[3,5,1,2,4])
  assert.deepEqual(inspected.rows.map(g=>g.selected),[true,true,false,false,false])
  assert.deepEqual(inspected.rows.map(g=>g.score),[.7,.5,.9,.8,.6])
})
test('P3.1: leading-only filter changes visible rows, not saved inspection or exported genes',()=>{
  const html=pathwayInspectorHtml(ranked,row,inspected,{selectedOnly:true})
  assert.match(html,/id="pathway-inspector-selected-only" checked/)
  assert.match(html,/Showing 2 of 5 measured pathway genes/)
  assert.doesNotMatch(html,/<td>A<\/td>/)
  assert.match(html,/<td>C<\/td>/)
  assert.match(html,/Export all exact matched genes/)
  const full=pathwayInspectorTsv(ranked,row,inspected)
  assert.equal(full.trim().split('\n').length,6)
  assert.match(full,/\tA\t1\t0\.9/)
})
test('P3.1: ORA selection uses saved thresholds, not the leading-edge label',()=>{
  const ora={...ranked,mode:'ora',source:'compare'}
  const r={...row,leadingGenes:['B'],foldEnrichment:1.4}
  const x=inspectPathwayGenes(ora,r,input,sets)
  assert.equal(x.rows[0].gene,'B')
  assert.match(pathwayInspectorHtml(ora,r,x,{selectedOnly:true}),/Show ORA-selected genes only/)
  assert.match(pathwayInspectorHtml(ora,r,x,{selectedOnly:true}),/Showing 1 of 5/)
})
test('P3.1: zero leading-edge genes returns a clear empty view without changing the full export',()=>{
  const r={...row,leadingGenes:[]}
  const x=inspectPathwayGenes(ranked,r,input,sets)
  const html=pathwayInspectorHtml(ranked,r,x,{selectedOnly:true})
  assert.match(html,/No selected genes in this pathway/)
  assert.equal(pathwayInspectorTsv(ranked,r,x).trim().split('\n').length,6)
})
test('P3.1: inspector state restores locally; provenance warnings do not assert an organism or release',()=>{
  const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
  const view=readFileSync(new URL('../pathway-view.js',import.meta.url),'utf8')
  assert.match(main,/pathwayLeadingOnly: false/)
  assert.match(main,/pathwayLeadingOnly: state\.pathwayLeadingOnly/)
  assert.match(main,/typeof saved\.pathwayLeadingOnly === 'boolean'/)
  assert.match(view,/pathway-inspector-selected-only/)
  assert.match(view,/workspace\.pathwayLeadingOnly=event\.currentTarget\.checked/)
  assert.match(view,/Source metadata incomplete/)
  assert.match(view,/has not independently verified/)
})
