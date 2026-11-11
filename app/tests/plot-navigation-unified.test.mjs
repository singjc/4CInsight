import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {
  normalizePlotBox,boxZoomDomain,boxZoomView,bindPlotNavigation,plotNavigationToolbar,
} from '../plot-navigation.js'
import {completeWindow,zoomHeatmapWindow,panHeatmapWindow} from '../joint-heatmap-viewer.js'

const source=f=>readFileSync(new URL(`../${f}`,import.meta.url),'utf8')

test('box selection normalizes reverse drags and rejects tiny or invalid boxes',()=>{
  assert.deepEqual(normalizePlotBox({x:.8,y:.9},{x:.2,y:.1}),{x0:.2,x1:.8,y0:.1,y1:.9})
  assert.equal(normalizePlotBox({x:.8,y:.9},{x:.8,y:.9}),null)
  assert.equal(normalizePlotBox({x:NaN,y:0},{x:1,y:1}),null)
  assert.deepEqual(normalizePlotBox({x:-1,y:1.4},{x:2,y:-1}),{x0:0,x1:1,y0:0,y1:1})
})
test('box zoom maps screen Y down onto a data Y axis up, without changing input arrays',()=>{
  const base={minX:-10,maxX:10,minY:0,maxY:20}
  const selection={x0:.25,x1:.75,y0:.1,y1:.6}
  assert.deepEqual(boxZoomDomain(base,selection),{minX:-5,maxX:5,minY:8,maxY:18})
  assert.deepEqual(base,{minX:-10,maxX:10,minY:0,maxY:20})
  assert.deepEqual(boxZoomView({x:0,y:0,width:200,height:100},selection),{x:50,y:10,width:100,height:50})
})
test('distance heatmap wheel zoom preserves row and column ordering, minimum two cells and boundaries',()=>{
  const full=completeWindow(237)
  const zoom=zoomHeatmapWindow(full,.82,.16,.5,237)
  assert.equal(zoom.columnEnd-zoom.columnStart,119)
  assert.equal(zoom.rowEnd-zoom.rowStart,119)
  assert.ok(zoom.columnStart>zoom.rowStart)
  const moved=panHeatmapWindow(zoom,40,-30,300,300,237)
  assert.equal(moved.columnEnd-moved.columnStart,119)
  assert.equal(moved.rowEnd-moved.rowStart,119)
  assert.ok(moved.columnStart<zoom.columnStart)
  assert.ok(moved.rowStart>zoom.rowStart)
  let tiny=zoom
  for(let i=0;i<20;i++)tiny=zoomHeatmapWindow(tiny,.5,.5,.1,237)
  assert.equal(tiny.columnEnd-tiny.columnStart,2)
  assert.equal(tiny.rowEnd-tiny.rowStart,2)
  assert.deepEqual(zoomHeatmapWindow(full,.5,.5,3,237),full)
})

test('all figure toolbars expose wheel zoom, pan, box zoom and reset actions',()=>{
  const html=plotNavigationToolbar()
  for(const token of ['data-plot-nav-action="in"','data-plot-nav-action="out"','data-plot-nav-action="fit"',
    'data-plot-nav-mode="pan"','data-plot-nav-mode="box"','Scroll over plot','double-click'])assert.ok(html.includes(token),token)
  assert.match(source('integrative-ux.js'),/bindPlotNavigation\(svg/)
  assert.match(source('main.js'),/boxZoomDomain\(current,b\)/)
  assert.match(source('main.js'),/attachStaticSvgNavigation\(main\)/)
  assert.match(source('results-interactions.js'),/axisSetView\(svg,axis,next\)/)
  assert.match(source('joint-heatmap-viewer.js'),/matrix\.addEventListener\('wheel'/)
})

test('wheel acts only inside plot, box selects, and pan reports pixel motion',()=>{
  const oldDocument=globalThis.document
  const handlers=new Map()
  const stub=()=>({attrs:new Map(),dataset:{},classList:{add(){},toggle(){}},
    setAttribute(k,v){this.attrs.set(k,String(v))},getAttribute(k){return this.attrs.get(k)||null},
    addEventListener(k,fn){if(!handlers.has(k))handlers.set(k,[]);handlers.get(k).push(fn)},
    getBoundingClientRect(){return {left:100,top:200,width:500,height:400}},
    appendChild(){},setPointerCapture(){},closest(){return {querySelectorAll(){return []}}}})
  globalThis.document={createElementNS(){return stub()}}
  try{
    const svg=stub()
    svg.setAttribute('viewBox','0 0 500 400')
    const calls={zoom:[],pan:[],box:[],reset:0}
    const controller=bindPlotNavigation(svg,{plot:{left:50,right:450,top:40,bottom:360},
      zoom:(...a)=>calls.zoom.push(a),pan:(...a)=>calls.pan.push(a),
      box:b=>calls.box.push(b),reset:()=>calls.reset++})
    const run=(type,props)=>{
      let prevented=false
      const event={button:0,pointerId:1,pointerType:'mouse',clientX:350,clientY:400,
        preventDefault(){prevented=true},stopImmediatePropagation(){},...props,type}
      for(const fn of handlers.get(type)||[])fn(event)
      return prevented
    }
    assert.equal(run('wheel',{clientX:101,deltaY:-1}),false)
    assert.equal(calls.zoom.length,0)
    assert.equal(run('wheel',{deltaY:-1}),true)
    assert.equal(calls.zoom.length,1)
    assert.ok(calls.zoom[0][0]<1)
    controller.setMode('box')
    run('pointerdown',{clientX:250,clientY:320})
    run('pointermove',{clientX:450,clientY:480})
    run('pointerup',{clientX:450,clientY:480})
    assert.equal(calls.box.length,1)
    assert.ok(calls.box[0].x0<calls.box[0].x1)
    controller.setMode('pan')
    run('pointerdown',{clientX:250,clientY:320})
    run('pointermove',{clientX:280,clientY:343})
    run('pointerup',{clientX:280,clientY:343})
    assert.ok(calls.pan.length>0)
    run('dblclick',{})
    assert.equal(calls.reset,1)
  }finally{globalThis.document=oldDocument}
})

test('navigation imports use the current application cache contract without changing science schema',()=>{
  const main=source('main.js')
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.ok(version)
  for(const path of ['main.js','integrative-ux.js','results-interactions.js']){
    assert.ok(source(path).includes(`./plot-navigation.js?v=${version}`),`${path} navigation import/version mismatch`)
  }
  assert.match(main,/const APP_STATE_SCHEMA = 1/)
})
test('heatmap table shares zoom mode and keeps measured cells in the DOM',()=>{
  const plot=source('plot-navigation.js')
  assert.match(plot,/attachTableHeatmapNavigation/)
  assert.match(plot,/scroll\.addEventListener\('wheel'/)
  assert.match(plot,/scale\*factor/)
  assert.match(plot,/table\.style\.setProperty\('--plot-heatmap-cell'/)
  assert.doesNotMatch(plot,/\.innerHTML\s*=\s*.*result|fetch\(|XMLHttpRequest|WebSocket/)
})
