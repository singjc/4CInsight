import test from 'node:test'
import assert from 'node:assert/strict'
import {networkState,buildPathwayNetwork,layoutPathwayNetwork} from '../pathway-network.js'

const report={mode:'ranked',results:[
  {pathway:'PATH_A',description:'first',q_value:.01,p_value:.01,normalizedScore:2,matchedSetSize:3,leadingGenes:['A','B']},
  {pathway:'PATH_B',description:'second',q_value:.02,p_value:.02,normalizedScore:-2,matchedSetSize:3,leadingGenes:['B','C']},
]}
const sets={pathways:[{name:'PATH_A',description:'first',genes:['A','B','D']},{name:'PATH_B',description:'second',genes:['B','C','D']}]}
const input={rows:['A','B','C','D'].map((gene,i)=>({gene,score:1-i*.1}))}
const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,{view:'bipartite',minCommon:1,minSimilarity:0,genes:'matched',hideSingleGenes:false,maxGenes:10},input))
graph.reportMode='ranked'

function surface(){
  const el=()=>({innerHTML:'',events:{},dataset:{},addEventListener(type,cb){this.events[type]=cb},dispatch(type,data){this.events[type]?.({...data,type})},querySelectorAll(){return []},querySelector(){return null}})
  const canvas=el(),inspection=el(),status=el()
  canvas.contains=()=>true
  canvas.getBoundingClientRect=()=>({left:0,top:0,width:graph.width,height:graph.height})
  canvas.setPointerCapture=id=>{canvas.capture=id}
  canvas.hasPointerCapture=id=>canvas.capture===id
  canvas.releasePointerCapture=id=>{if(canvas.capture===id)canvas.capture=null}
  const container={dataset:{},querySelector(sel){return ({'.pathway-network-canvas':canvas,'.pathway-network-inspection':inspection,'.pathway-network-focus-status':status})[sel]||null},querySelectorAll(){return []}}
  const picked=[]
  const result=networkState(container,graph,index=>picked.push(index))
  return {canvas,inspection,container,picked,result}
}
function target(node){return {closest:()=>node?{dataset:{networkNode:node.id}}:null}}
function pointer(s,node,{move=false,upTarget=null,start={x:300,y:200},end={x:320,y:220},time=100}={}){
  const opts={button:0,pointerId:1,clientX:start.x,clientY:start.y,timeStamp:time,target:target(node)}
  s.canvas.dispatch('pointerdown',opts)
  if(move)s.canvas.dispatch('pointermove',{...opts,clientX:end.x,clientY:end.y,timeStamp:time+1,target:target(null)})
  s.canvas.dispatch('pointerup',{...opts,clientX:move?end.x:start.x,clientY:move?end.y:start.y,timeStamp:time+2,target:target(upTarget)})
  // Native click often targets the canvas after pointer capture.
  s.canvas.dispatch('click',{detail:1,timeStamp:time+3,target:target(null)})
}

test('P5.4: captured pointerup selects the original pathway once when click is retargeted to canvas',()=>{
  const s=surface(),node=graph.nodes.find(x=>x.type==='pathway')
  pointer(s,node)
  assert.deepEqual(s.picked,[node.reportIndex])
  assert.match(s.inspection.innerHTML,/Selected pathway/)
  assert.match(s.inspection.innerHTML,/Open full pathway gene table/)
  assert.equal(s.container.dataset.networkSelectedPathway,node.id)
  assert.equal(s.canvas.capture,null)
})

test('P5.4: captured pointerup selects a gene and lists connected pathways',()=>{
  const s=surface(),gene=graph.nodes.find(x=>x.type==='gene'&&x.name==='B')
  pointer(s,gene)
  assert.equal(s.picked.length,0)
  assert.match(s.inspection.innerHTML,/Selected measured gene/)
  assert.match(s.inspection.innerHTML,/PATH_A/)
  assert.match(s.inspection.innerHTML,/PATH_B/)
})

test('P5.4: pan drag does not inspect a node; later tap can still inspect',()=>{
  const s=surface(),node=graph.nodes.find(x=>x.type==='pathway')
  pointer(s,node,{move:true,time:100})
  assert.deepEqual(s.picked,[])
  pointer(s,node,{time:900})
  assert.deepEqual(s.picked,[node.reportIndex])
})

test('P5.4: keyboard and scripted SVG node clicks still inspect the selected pathway',()=>{
  const s=surface(),node=graph.nodes.find(x=>x.type==='pathway'),e={target:target(node),timeStamp:700}
  s.canvas.dispatch('click',{...e,detail:0})
  assert.deepEqual(s.picked,[node.reportIndex])
  let prevented=false
  s.canvas.dispatch('keydown',{...e,key:'Enter',preventDefault(){prevented=true}})
  assert.deepEqual(s.picked,[node.reportIndex,node.reportIndex])
  assert.equal(prevented,true)
})

test('P5.4: clicking empty canvas does not select anything',()=>{
  const s=surface()
  pointer(s,null)
  assert.deepEqual(s.picked,[])
  assert.match(s.inspection.innerHTML,/Inspect a network node/)
})
