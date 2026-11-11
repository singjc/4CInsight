import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {pathwayNetworkFitView,mountExpandedNetwork,buildPathwayNetwork,layoutPathwayNetwork,pathwayNetworkSvg} from '../pathway-network.js'

const src=readFileSync(new URL('../pathway-network.js',import.meta.url),'utf8')

function mockPopup({supported=true}={}) {
  class FakeElement {
    constructor(name){this.name=name;this.nodes=[];this.parent=null;this.attrs=new Map();this.listeners=new Map();this.focused=false}
    addEventListener(type,fn){let all=this.listeners.get(type)||[];all.push(fn);this.listeners.set(type,all)}
    fire(type){for(const fn of this.listeners.get(type)||[])fn({currentTarget:this})}
    append(node){if(node.parent)node.parent.nodes.splice(node.parent.nodes.indexOf(node),1);this.nodes.push(node);node.parent=this}
    setAttribute(key,value){this.attrs.set(key,value)}
    getAttribute(key){return this.attrs.get(key)}
    focus(){this.focused=true}
    querySelector(selector){return this.queryMap?.[selector]||this.nodes.find(n=>n.name===selector)||null}
  }
  const slot=new FakeElement('slot'),home=new FakeElement('home'),stage=new FakeElement('stage')
  const modal=new FakeElement('dialog'),expand=new FakeElement('expand'),close=new FakeElement('close'),inspect=new FakeElement('inspect')
  const toolbar=new FakeElement('.pathway-network-tools'),canvas=new FakeElement('.pathway-network-canvas'),text=new FakeElement('.pathway-network-inspection'),help=new FakeElement('.pathway-network-help'),exports=new FakeElement('.pathway-network-export')
  for(const el of [toolbar,canvas,text,help,exports])home.append(el)
  slot.queryMap={'#pathway-network-expand':expand,'#pathway-network-dialog':modal,'.pathway-network-home':home,'.pathway-network-inspection':text}
  modal.queryMap={'.pathway-network-modal-stage':stage,'#pathway-network-close':close,'#pathway-network-show-genes':inspect}
  modal.open=false
  if(supported)modal.showModal=function(){this.open=true}
  modal.close=function(){this.open=false;this.fire('close')}
  mountExpandedNetwork(slot)
  return {slot,home,stage,modal,expand,close,inspect,toolbar,canvas,text,help,exports}
}

test('P5.1: expand moves the *same* bound plot and toolbar to a native dialog and restores them',()=>{
  const x=mockPopup()
  let toolbarClicks=0
  x.toolbar.addEventListener('click',()=>toolbarClicks++)
  x.expand.fire('click')
  assert.equal(x.modal.open,true)
  assert.equal(x.expand.getAttribute('aria-expanded'),'true')
  assert.deepEqual(x.stage.nodes,[x.toolbar,x.canvas,x.text,x.help,x.exports])
  assert.equal(x.home.nodes.length,0)
  x.toolbar.fire('click')
  assert.equal(toolbarClicks,1)
  x.close.fire('click')
  assert.equal(x.modal.open,false)
  assert.equal(x.expand.getAttribute('aria-expanded'),'false')
  assert.deepEqual(x.home.nodes,[x.toolbar,x.canvas,x.text,x.help,x.exports])
  assert.equal(x.expand.focused,true)
  x.toolbar.fire('click')
  assert.equal(toolbarClicks,2)
})

test('P5.1: Escape/dialog close event restores original plot; open again remains idempotent',()=>{
  const x=mockPopup()
  x.expand.fire('click');x.expand.fire('click')
  assert.equal(x.stage.nodes.length,5)
  x.modal.close()
  assert.equal(x.home.nodes.length,5)
  x.expand.fire('click')
  assert.equal(x.stage.nodes.length,5)
  x.modal.close()
})

test('P5.1: unsupported browser leaves plot intact',()=>{
  const x=mockPopup({supported:false})
  x.expand.fire('click')
  assert.equal(x.home.nodes.length,5)
  assert.equal(x.stage.nodes.length,0)
  assert.match(x.text.textContent,/native dialog support/)
})

test('P5.1: fit nodes centers plotted data and keeps fixed marker scale',()=>{
  const graph={width:1000,height:570,nodes:[{x:700,y:330},{x:900,y:430},{x:800,y:380}]}
  const fit=pathwayNetworkFitView(graph)
  assert.equal(fit.cx,800)
  assert.equal(fit.cy,380)
  assert.ok(fit.k>1 && fit.k<=24)
  assert.deepEqual(graph.nodes,[{x:700,y:330},{x:900,y:430},{x:800,y:380}])
  assert.deepEqual(pathwayNetworkFitView({width:1000,height:570,nodes:[]}),{cx:500,cy:285,k:1})
})

test('P5.1: network labels and source values are still escaped and original graph is read-only',()=>{
  const gs={pathways:[{name:'<gene-pathway>',description:'ref',genes:['A','B','C']}]}
  const input={rows:['A','B','C'].map(gene=>({gene,score:.2}))}
  const report={mode:'ranked',results:[{pathway:'<gene-pathway>',description:'ref',q_value:.01,leadingGenes:['A','B'],matchedSetSize:3,p_value:.01}]}
  const graph=layoutPathwayNetwork(buildPathwayNetwork(report,gs,{minCommon:1,maxPathways:2},input))
  const svg=pathwayNetworkSvg(graph,{view:{cx:500,cy:285,k:2}})
  assert.ok(svg.includes('&lt;gene-pathway&gt;'))
  assert.equal(report.results[0].q_value,.01)
  assert.doesNotMatch(src,/\bfetch\s*\(|XMLHttpRequest|Math\.random\(/)
})

test('P5.1: popup is accessible, exposes SVG plot once, and retains all original controls',()=>{
  assert.equal((src.match(/class="pathway-network-canvas"/g)||[]).length,1)
  assert.match(src,/id="pathway-network-expand"[^>]*aria-haspopup="dialog"[^>]*aria-expanded="false"/)
  assert.match(src,/<dialog class="pathway-network-dialog"[^>]*aria-labelledby="pathway-network-modal-title"/)
  assert.match(src,/data-network-tool="fit"/)
  assert.match(src,/dialog\.addEventListener\('close',restore\)/)
  assert.match(src,/for\(const element of moved\)stage\.append\(element\)/)
  assert.match(src,/for\(const element of moved\)home\.append\(element\)/)
  for(const control of ['"in"','"out"','"pan"','"box"']){
    // Existing pointer and mouse navigation remain in the source.
    assert.ok(src.includes('pointerdown') && src.includes('wheel'))
  }
})
