/** Unified browser-only plot navigation. Display state only: no source data changes. */
const SVG_NS='http://www.w3.org/2000/svg'
const clamp=(v,a,b)=>Math.max(a,Math.min(b,v))
const finite=v=>Number.isFinite(v)
export const PLOT_ZOOM_MAX=24

/** Fractions of a visible plot, left-to-right and TOP-to-bottom. */
export function normalizePlotBox(a,b,minFraction=0.015){
  if(!a||!b||![a.x,a.y,b.x,b.y].every(finite))return null
  const x0=clamp(Math.min(a.x,b.x),0,1),x1=clamp(Math.max(a.x,b.x),0,1)
  const y0=clamp(Math.min(a.y,b.y),0,1),y1=clamp(Math.max(a.y,b.y),0,1)
  if(x1-x0<minFraction||y1-y0<minFraction)return null
  return {x0,y0,x1,y1}
}
export function boxZoomDomain(domain,box){
  if(!domain||!box)return null
  const {minX,maxX,minY,maxY}=domain
  if(![minX,maxX,minY,maxY].every(finite)||maxX<=minX||maxY<=minY)return null
  const b=normalizePlotBox({x:box.x0,y:box.y0},{x:box.x1,y:box.y1},0.00001)
  if(!b)return null
  return {minX:minX+(maxX-minX)*b.x0,maxX:minX+(maxX-minX)*b.x1,
    minY:maxY-(maxY-minY)*b.y1,maxY:maxY-(maxY-minY)*b.y0}
}
export function boxZoomView(view,box){
  if(!view||!box||![view.x,view.y,view.width,view.height].every(finite)||view.width<=0||view.height<=0)return null
  const b=normalizePlotBox({x:box.x0,y:box.y0},{x:box.x1,y:box.y1},0.00001)
  if(!b)return null
  return {x:view.x+b.x0*view.width,y:view.y+b.y0*view.height,
    width:(b.x1-b.x0)*view.width,height:(b.y1-b.y0)*view.height}
}

function boundsFraction(svg,plot){
  const r=svg.getBoundingClientRect()
  if(!(r.width>0&&r.height>0))return null
  // plot margins in logical SVG units, not CSS px.
  const b=String(svg.getAttribute('viewBox')||'').trim().split(/[\s,]+/).map(Number)
  if(b.length!==4||b.some(n=>!finite(n))||b[2]<=0||b[3]<=0)return null
  const plotBounds=typeof plot==='function'?plot():plot
  const x0=plotBounds?clamp((plotBounds.left-b[0])/b[2],0,1):0
  const x1=plotBounds?clamp((plotBounds.right-b[0])/b[2],0,1):1
  const y0=plotBounds?clamp((plotBounds.top-b[1])/b[3],0,1):0
  const y1=plotBounds?clamp((plotBounds.bottom-b[1])/b[3],0,1):1
  if(x1<=x0||y1<=y0)return null
  return {r,b,x0,x1,y0,y1}
}
function pointerFraction(event,g){
  return {x:clamp(((event.clientX-g.r.left)/g.r.width-g.x0)/(g.x1-g.x0),0,1),
    y:clamp(((event.clientY-g.r.top)/g.r.height-g.y0)/(g.y1-g.y0),0,1)}
}
function isInside(event,g){
  const x=(event.clientX-g.r.left)/g.r.width,y=(event.clientY-g.r.top)/g.r.height
  return x>=g.x0&&x<=g.x1&&y>=g.y0&&y<=g.y1
}
function selectionRect(svg){
  const n=document.createElementNS(SVG_NS,'rect')
  n.setAttribute('class','plot-navigation-selection')
  n.setAttribute('pointer-events','none')
  n.setAttribute('visibility','hidden')
  svg.appendChild(n)
  return n
}
/**
 * Shared gesture contract for SVG charts. Native wheel zoom works only while
 * the pointer is inside the plot. Buttons, pan, box zoom and reset use the
 * same callbacks. Pan is the default; box mode has an explicit pressed button.
 * Event coordinates are fractions of the CURRENT visible plot rectangle.
 */
export function bindPlotNavigation(svg,{plot=null,zoom,pan,box,reset,mode='pan',onMode=null}={}){
  if(!svg||typeof zoom!=='function'||typeof pan!=='function'||typeof box!=='function')return null
  let currentMode=mode,drag=null,skipClick=false
  const outline=selectionRect(svg)
  svg.classList.add('plot-navigation-enabled')
  svg.setAttribute('tabindex',svg.getAttribute('tabindex')||'0')
  const modeButtons=()=>{
    const toolbar=svg.closest('[data-plot-nav-root]')
    toolbar?.querySelectorAll('[data-plot-nav-mode]').forEach(button=>{
      const active=button.dataset.plotNavMode===currentMode
      button.setAttribute('aria-pressed',String(active))
      button.classList.toggle('is-active',active)
    })
  }
  const setMode=next=>{if(next!=='pan'&&next!=='box')return;currentMode=next;drag=null;outline.setAttribute('visibility','hidden');svg.dataset.plotMode=currentMode;modeButtons();onMode?.(next)}
  function renderBox(a,b,g){
    const box=normalizePlotBox(a,b,0)
    if(!box)return
    const X=v=>g.b[0]+(g.x0+(g.x1-g.x0)*v)*g.b[2]
    const Y=v=>g.b[1]+(g.y0+(g.y1-g.y0)*v)*g.b[3]
    outline.setAttribute('x',String(X(box.x0)))
    outline.setAttribute('y',String(Y(box.y0)))
    outline.setAttribute('width',String(X(box.x1)-X(box.x0)))
    outline.setAttribute('height',String(Y(box.y1)-Y(box.y0)))
    outline.setAttribute('visibility','visible')
  }
  svg.addEventListener('wheel',event=>{
    if(event.altKey)return
    const g=boundsFraction(svg,plot)
    if(!g||!isInside(event,g))return
    event.preventDefault()
    const at=pointerFraction(event,g)
    zoom(event.deltaY<0?0.82:1.22,at.x,at.y)
  },{passive:false})
  svg.addEventListener('pointerdown',event=>{
    if(event.button!==0||event.pointerType==='touch'||event.target===outline)return
    const g=boundsFraction(svg,plot)
    if(!g||!isInside(event,g))return
    drag={id:event.pointerId,x:event.clientX,y:event.clientY,start:pointerFraction(event,g),last:pointerFraction(event,g),moved:false}
    svg.setPointerCapture?.(event.pointerId)
    if(currentMode==='box'){event.preventDefault();renderBox(drag.start,drag.start,g)}
  })
  svg.addEventListener('pointermove',event=>{
    if(!drag||event.pointerId!==drag.id)return
    const g=boundsFraction(svg,plot)
    if(!g)return
    const dx=event.clientX-drag.x,dy=event.clientY-drag.y
    drag.moved ||= Math.hypot(dx,dy)>3 || Math.hypot(event.clientX-drag.lastX||0,event.clientY-drag.lastY||0)>3
    drag.lastX=event.clientX;drag.lastY=event.clientY
    if(currentMode==='box')renderBox(drag.start,pointerFraction(event,g),g)
    else if(drag.moved){pan(dx,dy);drag.x=event.clientX;drag.y=event.clientY}
  })
  const finish=event=>{
    if(!drag||event.pointerId!==drag.id)return
    const g=boundsFraction(svg,plot)
    if(currentMode==='box'&&g&&event.type==='pointerup'){
      const end=pointerFraction(event,g)
      const minX=8/(g.r.width*(g.x1-g.x0)),minY=8/(g.r.height*(g.y1-g.y0))
      const b=normalizePlotBox(drag.start,end,0)
      if(b&&b.x1-b.x0>=minX&&b.y1-b.y0>=minY){box(b);skipClick=true;setTimeout(()=>{skipClick=false},0)}
    }
    if(currentMode==='pan'&&drag.moved){skipClick=true;setTimeout(()=>{skipClick=false},0)}
    drag=null;outline.setAttribute('visibility','hidden')
  }
  svg.addEventListener('click',event=>{if(skipClick){event.preventDefault();event.stopImmediatePropagation()}},true)
  svg.addEventListener('pointerup',finish)
  svg.addEventListener('pointercancel',finish)
  svg.addEventListener('dblclick',event=>{if(event.target?.closest?.('circle'))return;reset?.()})
  svg.addEventListener('keydown',event=>{
    if(event.target!==svg)return
    if(event.key==='+'||event.key==='='){event.preventDefault();zoom(0.82,.5,.5)}
    if(event.key==='-'){event.preventDefault();zoom(1.22,.5,.5)}
    if(event.key==='0'){event.preventDefault();reset?.()}
    if(event.key.toLowerCase()==='b'){event.preventDefault();setMode('box')}
    if(event.key.toLowerCase()==='p'){event.preventDefault();setMode('pan')}
  })
  setMode(mode)
  return {setMode,get mode(){return currentMode},zoom,reset}
}

/** Toolbars share the same labels/behavior across result views. */
export function plotNavigationToolbar({compact=false}={}){
  return `<div class="plot-nav-toolbar" role="group" aria-label="Plot view tools">
    <button type="button" class="plot-nav-button" data-plot-nav-action="in">Zoom +</button>
    <button type="button" class="plot-nav-button" data-plot-nav-action="out">Zoom −</button>
    <button type="button" class="plot-nav-button" data-plot-nav-action="fit">Reset</button>
    <button type="button" class="plot-nav-button is-active" data-plot-nav-mode="pan" aria-pressed="true">Pan</button>
    <button type="button" class="plot-nav-button" data-plot-nav-mode="box" aria-pressed="false">Box zoom</button>
    ${compact?'':'<span class="plot-nav-hint">Scroll over plot to zoom · drag to pan or draw a box · double-click to reset</span>'}
  </div>`
}
export function bindPlotToolbar(root,controller){
  if(!root||!controller)return
  root.querySelectorAll('[data-plot-nav-mode]').forEach(button=>button.addEventListener('click',()=>controller.setMode(button.dataset.plotNavMode)))
  root.querySelectorAll('[data-plot-nav-action]').forEach(button=>button.addEventListener('click',()=>{
    const action=button.dataset.plotNavAction
    if(action==='fit')controller.reset?.()
    else controller.zoom?.(action==='in'?0.78:1.28,.5,.5)
  }))
}

// Lightweight viewport for SVG displays that have no data axis engine.
// Keeps source SVG content untouched; annotations are generated only on zoom.
export function attachStaticSvgNavigation(root=document){
  const selectors=['.bio-feature-plot svg','.bio-native-plot svg','.longitudinal-dendrogram-scroll svg','.i2-heatmap-scroll svg']
  for(const svg of root.querySelectorAll(selectors.join(','))){
    if(svg.dataset.plotNavMounted==='true')continue
    const base=String(svg.getAttribute('viewBox')||'').split(/[\s,]+/).map(Number)
    if(base.length!==4||base.some(n=>!finite(n))||base[2]<=0||base[3]<=0)continue
    const container=svg.closest('.bio-feature-plot,.bio-native-plot,.longitudinal-dendrogram-scroll,.i2-heatmap-scroll')
    if(!container)continue
    container.dataset.plotNavRoot='true'
    const tools=document.createElement('div')
    tools.innerHTML=plotNavigationToolbar()
    container.insertBefore(tools.firstElementChild,svg)
    const original={x:base[0],y:base[1],width:base[2],height:base[3]}
    let view={...original}
    const labelLayer=document.createElementNS(SVG_NS,'g')
    labelLayer.setAttribute('class','plot-navigation-labels')
    labelLayer.setAttribute('pointer-events','none')
    svg.appendChild(labelLayer)
    const marks=[...svg.querySelectorAll('circle')].filter(c=>finite(Number(c.getAttribute('cx')))&&finite(Number(c.getAttribute('cy'))))
    const isScatter=container.classList.contains('bio-native-plot')||container.classList.contains('bio-feature-plot')
    function annotate(){
      labelLayer.replaceChildren()
      if(!isScatter||original.width/view.width<2||!marks.length)return
      const box=svg.getBoundingClientRect(),scale=box.width/view.width
      if(!(scale>0))return
      const candidates=marks.map(circle=>{
        const x=Number(circle.getAttribute('cx')),y=Number(circle.getAttribute('cy'))
        return {x,y,circle,sx:(x-view.x)*scale,sy:(y-view.y)*box.height/view.height,
          name:circle.dataset.sampleId||circle.querySelector('title')?.textContent?.split(' · ')[0]||''}
      }).filter(p=>p.name&&p.sx>15&&p.sx<box.width-25&&p.sy>8&&p.sy<box.height-10)
      const chosen=[]
      for(const p of candidates){
        if(chosen.length>=40)break
        if(candidates.some(q=>q!==p&&Math.abs(q.sx-p.sx)<25&&Math.abs(q.sy-p.sy)<15))continue
        const textWidth=Math.min(145,p.name.length*6.5)
        if(p.sx+textWidth+10>box.width)continue
        if(chosen.some(q=>Math.abs(q.sy-p.sy)<15&&Math.abs(q.sx-p.sx)<textWidth+20))continue
        chosen.push(p)
        const label=document.createElementNS(SVG_NS,'text')
        label.setAttribute('x',String(p.x+8/scale))
        label.setAttribute('y',String(p.y-6/scale))
        label.setAttribute('font-size',String(11/scale))
        label.setAttribute('stroke-width',String(2.5/scale))
        label.setAttribute('class','plot-navigation-point-label')
        label.textContent=p.name
        labelLayer.appendChild(label)
      }
    }
    function set(v){
      const minimum=original.width/PLOT_ZOOM_MAX
      if(!(v.width>=minimum&&v.height>0))return
      view=v;svg.setAttribute('viewBox',`${v.x} ${v.y} ${v.width} ${v.height}`);annotate()
    }
    const zoom=(factor,ax=.5,ay=.5)=>{
      const width=clamp(view.width*factor,original.width/PLOT_ZOOM_MAX,original.width)
      const height=clamp(view.height*factor,original.height/PLOT_ZOOM_MAX,original.height)
      const x=clamp(view.x+ax*(view.width-width),original.x,original.x+original.width-width)
      const y=clamp(view.y+ay*(view.height-height),original.y,original.y+original.height-height)
      set({x,y,width,height})
    }
    const reset=()=>set({...original})
    const controller=bindPlotNavigation(svg,{plot:null,zoom,
      pan:(dx,dy)=>set({ ...view,
        x:clamp(view.x-dx*view.width/Math.max(1,svg.getBoundingClientRect().width),original.x,original.x+original.width-view.width),
        y:clamp(view.y-dy*view.height/Math.max(1,svg.getBoundingClientRect().height),original.y,original.y+original.height-view.height)}),
      box:b=>set(boxZoomView(view,b)),reset})
    bindPlotToolbar(container,controller)
    svg.dataset.plotNavMounted='true'
  }
}

/** Sticky-header, table-based feature heatmaps use cell-size zoom, not SVG scaling. */
export function attachTableHeatmapNavigation(root=document){
  for(const scroll of root.querySelectorAll('.significant-heatmap-scroll')){
    if(scroll.dataset.plotNavMounted==='true')continue
    const table=scroll.querySelector('table.significant-heatmap')
    if(!table)continue
    let scale=1,mode='pan',start=null,selection=null
    const container=scroll.parentElement
    if(!container)continue
    const toolbar=document.createElement('div')
    toolbar.innerHTML=plotNavigationToolbar()
    const bar=toolbar.firstElementChild
    container.insertBefore(bar,scroll)
    const setScale=(next,clientX=null,clientY=null)=>{
      const value=clamp(next,1,6)
      const prior=scale
      if(prior===value)return
      const rect=scroll.getBoundingClientRect()
      const px=clientX===null?rect.width/2:clamp(clientX-rect.left,0,rect.width)
      const py=clientY===null?rect.height/2:clamp(clientY-rect.top,0,rect.height)
      scale=value
      table.style.setProperty('--plot-heatmap-cell',`${(15*scale).toFixed(1)}px`)
      table.style.setProperty('--plot-heatmap-header',`${(88*scale).toFixed(1)}px`)
      scroll.scrollLeft=(scroll.scrollLeft+px)*(scale/prior)-px
      scroll.scrollTop=(scroll.scrollTop+py)*(scale/prior)-py
    }
    const reset=()=>{setScale(1);scroll.scrollLeft=0;scroll.scrollTop=0}
    const changeMode=next=>{
      mode=next
      bar.querySelectorAll('[data-plot-nav-mode]').forEach(button=>{
        const active=button.dataset.plotNavMode===next
        button.setAttribute('aria-pressed',String(active))
        button.classList.toggle('is-active',active)
      })
    }
    bar.querySelectorAll('[data-plot-nav-mode]').forEach(button=>button.addEventListener('click',()=>changeMode(button.dataset.plotNavMode)))
    bar.querySelectorAll('[data-plot-nav-action]').forEach(button=>button.addEventListener('click',()=>{
      const action=button.dataset.plotNavAction
      if(action==='fit')reset()
      else setScale(scale*(action==='in'?1.3:1/1.3))
    }))
    scroll.addEventListener('wheel',event=>{
      if(!event.target.closest?.('.significant-heatmap'))return
      event.preventDefault()
      setScale(scale*(event.deltaY<0?1.16:1/1.16),event.clientX,event.clientY)
    },{passive:false})
    const hideSelection=()=>{selection?.remove();selection=null}
    scroll.addEventListener('pointerdown',event=>{
      if(event.button!==0||event.pointerType==='touch'||!event.target.closest?.('.significant-heatmap'))return
      start={id:event.pointerId,x:event.clientX,y:event.clientY,left:scroll.scrollLeft,top:scroll.scrollTop}
      scroll.setPointerCapture?.(event.pointerId)
      if(mode==='box'){
        selection=document.createElement('div')
        selection.className='plot-navigation-table-selection'
        document.body.appendChild(selection)
      }
    })
    scroll.addEventListener('pointermove',event=>{
      if(!start||event.pointerId!==start.id)return
      if(mode==='pan'){
        scroll.scrollLeft=start.left+start.x-event.clientX
        scroll.scrollTop=start.top+start.y-event.clientY
        return
      }
      if(selection){
        const r=scroll.getBoundingClientRect()
        const x0=clamp(start.x,r.left,r.right),x1=clamp(event.clientX,r.left,r.right)
        const y0=clamp(start.y,r.top,r.bottom),y1=clamp(event.clientY,r.top,r.bottom)
        selection.style.left=`${Math.min(x0,x1)}px`
        selection.style.top=`${Math.min(y0,y1)}px`
        selection.style.width=`${Math.abs(x1-x0)}px`
        selection.style.height=`${Math.abs(y1-y0)}px`
      }
    })
    const end=event=>{
      if(!start||event.pointerId!==start.id)return
      if(mode==='box'&&event.type==='pointerup'){
        const r=scroll.getBoundingClientRect()
        const x0=clamp(Math.min(start.x,event.clientX),r.left,r.right),x1=clamp(Math.max(start.x,event.clientX),r.left,r.right)
        const y0=clamp(Math.min(start.y,event.clientY),r.top,r.bottom),y1=clamp(Math.max(start.y,event.clientY),r.top,r.bottom)
        if(x1-x0>=10&&y1-y0>=10){
          const factor=Math.min(r.width/(x1-x0),r.height/(y1-y0))
          const before=scale,left=scroll.scrollLeft,top=scroll.scrollTop
          setScale(scale*factor,x0,y0)
          if(scale>before){
            scroll.scrollLeft=(left+x0-r.left)*scale/before
            scroll.scrollTop=(top+y0-r.top)*scale/before
          }
        }
      }
      start=null;hideSelection()
    }
    scroll.addEventListener('pointerup',end)
    scroll.addEventListener('pointercancel',end)
    scroll.dataset.plotNavMounted='true'
  }
}
