import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {
  buildPathwayNetwork,layoutPathwayNetwork,networkState,pathwayNetworkSelectionHtml,
  planPathwayNetworkLabels,pathwayNetworkComponents,
} from '../pathway-network.js'

const source=readFileSync(new URL('../pathway-network.js',import.meta.url),'utf8')
const report={mode:'ranked',results:[
  {pathway:'Pathway A',description:'a',q_value:0.01,normalizedScore:2.4,matchedSetSize:3,leadingGenes:['X','Y'],p_value:.01},
  {pathway:'Pathway B',description:'b',q_value:0.02,normalizedScore:-1.5,matchedSetSize:3,leadingGenes:['Y','Z'],p_value:.02},
  {pathway:'Pathway C',description:'c',q_value:0.04,normalizedScore:1.1,matchedSetSize:2,leadingGenes:['U'],p_value:.04},
]}
const sets={pathways:[
  {name:'Pathway A',description:'a',genes:['X','Y','V']},
  {name:'Pathway B',description:'b',genes:['Y','Z','V']},
  {name:'Pathway C',description:'c',genes:['U','V']},
]}
const input={rows:['X','Y','Z','U','V'].map((gene,i)=>({gene,score:1-i*.1}))}
const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,{view:'bipartite',minCommon:1,minSimilarity:0,genes:'matched',hideSingleGenes:false,maxGenes:10},input))
graph.reportMode='ranked'

function fakeSurface(){
  function element(){return {innerHTML:'',handlers:new Map(),addEventListener(name,fn){this.handlers.set(name,fn)},fire(name,event){this.handlers.get(name)?.(event)},querySelector:()=>null}}
  const canvas=element(),inspection=element(),focus=element(),inspector={scrolled:0,scrollIntoView(){this.scrolled++},querySelector:()=>null}
  const container={dataset:{},querySelector(s){return ({'.pathway-network-canvas':canvas,'.pathway-network-inspection':inspection,'.pathway-network-focus-status':focus})[s]||null},querySelectorAll:()=>[],closest:()=>({querySelector:()=>inspector}),ownerDocument:{querySelector:()=>inspector}}
  canvas.contains=()=>true
  return {canvas,inspection,container,inspector}
}
function select(s,id){s.canvas.fire('click',{target:{closest:()=>({dataset:{networkNode:id}})}})}

test('P5.3: selecting a pathway immediately opens visible evidence above the graph and links to full genes',()=>{
  const s=fakeSurface(),picked=[]
  networkState(s.container,graph,index=>picked.push(index))
  const node=graph.nodes.find(n=>n.type==='pathway')
  select(s,node.id)
  assert.deepEqual(picked,[node.reportIndex])
  assert.match(s.inspection.innerHTML,/Selected pathway/)
  assert.match(s.inspection.innerHTML,/Normalized enrichment score/)
  assert.match(s.inspection.innerHTML,/Open full pathway gene table/)
  assert.equal(s.container.dataset.networkSelectedPathway,node.id)
  s.inspection.fire('click',{target:{closest:()=>({dataset:{networkOpenPathway:node.id}})}})
  assert.equal(s.inspector.scrolled,1)
  assert.deepEqual(picked,[node.reportIndex,node.reportIndex])
})

test('P5.3: selecting a gene opens its original rank and clickable related pathways',()=>{
  const s=fakeSurface(),picked=[]
  networkState(s.container,graph,index=>picked.push(index))
  const gene=graph.nodes.find(n=>n.type==='gene'&&n.name==='Y')
  assert.ok(gene)
  select(s,gene.id)
  assert.match(s.inspection.innerHTML,/Selected measured gene/)
  assert.match(s.inspection.innerHTML,/Original gene rank/)
  assert.match(s.inspection.innerHTML,/Pathway A/)
  assert.match(s.inspection.innerHTML,/Pathway B/)
  assert.equal(picked.length,0)
  const a=graph.nodes.find(n=>n.name==='Pathway A')
  s.inspection.fire('click',{target:{closest:()=>({dataset:{networkOpenPathway:a.id}})}})
  assert.deepEqual(picked,[a.reportIndex])
  assert.equal(s.inspector.scrolled,1)
})

test('P5.3: selection content escapes HTML and never invents biological validation',()=>{
  const rendered=pathwayNetworkSelectionHtml({reportMode:'ranked',nodes:[],edges:[]},{id:'p1',type:'pathway',name:'<img onerror=alert(1)>',genes:['<B>'],q_value:.04,normalizedScore:2})
  assert.doesNotMatch(rendered,/<img|<B>/)
  assert.match(rendered,/&lt;img/)
  assert.match(rendered,/&lt;B&gt;/)
  assert.match(rendered,/not a physical interaction/)
  assert.doesNotMatch(source,/\bfetch\s*\(|XMLHttpRequest|Math\.random\(/)
})

test('P5.3: central labels are capped until zoomed, and selected node stays eligible',()=>{
  const dense={width:1000,height:570,nodes:Array.from({length:35},(_,i)=>({id:'p'+i,type:'pathway',name:'Long pathway '+i,x:485+(i%7)*9,y:250+Math.floor(i/7)*8,q_value:.001+i*.001})),edges:[]}
  const before=JSON.stringify(dense)
  const base=planPathwayNetworkLabels(dense,{selectedId:'p13'})
  assert.ok(base.length<=4)
  assert.equal(JSON.stringify(dense),before)
})

test('P5.3: group navigation, popup evidence and selection are prominent without changing graph membership',()=>{
  const original=JSON.stringify(graph)
  assert.equal(source.split('id="pathway-network-group-select"').length-1,1)
  assert.match(source,/pathway-network-focus-status/)
  assert.match(source,/id="pathway-network-show-all"/)
  assert.match(source,/class="pathway-network-inspection" role="region"/)
  assert.match(source,/slot\.closest\('\.pathway-results'\)\?\.querySelector\('#pathway-gene-inspector'\)/)
  assert.match(source,/Select a pathway first/)
  assert.equal(JSON.stringify(graph),original)
  assert.ok(pathwayNetworkComponents(graph).length>0)
})
