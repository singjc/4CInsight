import {bindPlotNavigation, bindPlotToolbar, plotNavigationToolbar, boxZoomView, PLOT_ZOOM_MAX} from './plot-navigation.js?v=0.22.8'
// UX11: interactive Results plots and local exports. No network or scientific computations.
const SVG_NS = 'http://www.w3.org/2000/svg'
const RESULT_NAMES = {
  gene: 'Gene expression', transcript: 'Transcript expression',
  bcr: 'BCR repertoire', tcrAlpha: 'TCR alpha', tcrBeta: 'TCR beta',
  isotype: 'BCR isotypes', immune: 'Immune infiltration',
}
const PLOT_STYLES = `
  text { font-family: system-ui, -apple-system, sans-serif; }
  .overview-svg-name { font: 600 13px system-ui,sans-serif; fill: #334155; }
  .overview-svg-small { font: 11px system-ui,sans-serif; fill: #64748b; }
  .overview-svg-tick { font: 10px system-ui,sans-serif; fill: #64748b; }
  .plot-label, .plot-tick { fill: #475569; }
`
const clamp = (v, a, b) => Math.max(a, Math.min(b, v))
const finite = (v) => typeof v === 'number' && Number.isFinite(v)
const safeName = (value) => String(value ?? '')
  .normalize('NFKD').replace(/[^a-zA-Z0-9._-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 90) || 'results'

export function parsePlotBounds(viewBox) {
  const coords = String(viewBox || '').trim().split(/[\s,]+/).map(Number)
  if (coords.length !== 4 || coords.some((x) => !Number.isFinite(x)) || coords[2] <= 0 || coords[3] <= 0) return null
  return { x: coords[0], y: coords[1], width: coords[2], height: coords[3] }
}

// Pure geometry: keep aspect ratio, bound the view to the original plot, max zoom 16x.
export function zoomPlotBounds(view, base, factor, anchorX = 0.5, anchorY = 0.5) {
  if (!view || !base || !finite(factor) || factor <= 0) return view
  const minWidth = base.width / 16
  const width = clamp(view.width * factor, minWidth, base.width)
  const height = base.height * width / base.width
  const ax = clamp(anchorX, 0, 1), ay = clamp(anchorY, 0, 1)
  return {
    x: clamp(view.x + ax * (view.width - width), base.x, base.x + base.width - width),
    y: clamp(view.y + ay * (view.height - height), base.y, base.y + base.height - height),
    width, height,
  }
}
export function panPlotBounds(view, base, dxPixels, dyPixels, renderedWidth, renderedHeight) {
  if (!view || !base || !(renderedWidth > 0) || !(renderedHeight > 0)) return view
  return {
    ...view,
    x: clamp(view.x - dxPixels * view.width / renderedWidth, base.x, base.x + base.width - view.width),
    y: clamp(view.y - dyPixels * view.height / renderedHeight, base.y, base.y + base.height - view.height),
  }
}
const asViewBox = (b) => `${b.x} ${b.y} ${b.width} ${b.height}`
// UX12: fixed-frame coordinate zoom. Data points move; axes and captions keep their size.
// The original scientific arrays are never changed. No network or worker calls.
const axisPlotState = new WeakMap()
const AXIS_SVG_NS = 'http://www.w3.org/2000/svg'
const axisClamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v))
const axisNumber = (v) => Number.isFinite(Number(v)) ? Number(v) : NaN
const axisShown = (v,lo,hi) => Number.isFinite(v) && v>=lo-1e-9 && v<=hi+1e-9

export function axisZoomWindow(view, factor, anchorX=0.5, anchorY=0.5, zoomY=true) {
  if (!view || !Number.isFinite(factor) || factor<=0) return view
  const nextW=axisClamp(view.width*factor,1/16,1)
  const nextH=zoomY?axisClamp(view.height*factor,1/16,1):1
  return {x:axisClamp(view.x+axisClamp(anchorX,0,1)*(view.width-nextW),0,1-nextW),
    y:zoomY?axisClamp(view.y+axisClamp(anchorY,0,1)*(view.height-nextH),0,1-nextH):0,
    width:nextW,height:nextH}
}
export function axisPanWindow(view, dx, dy, plotWidth, plotHeight, zoomY=true) {
  if (!view || !(plotWidth>0) || !(plotHeight>0)) return view
  return {...view,
    x:axisClamp(view.x-dx/plotWidth*view.width,0,1-view.width),
    y:zoomY?axisClamp(view.y+dy/plotHeight*view.height,0,1-view.height):0}
}
export function axisProjection(value, view, from, to, reverse=false) {
  if (!Number.isFinite(value) || !view || !(view.width>0)) return NaN
  const fraction=(value-view.x)/view.width
  return reverse?to-fraction*(to-from):from+fraction*(to-from)
}
function axisText(value){
  if(!Number.isFinite(value)) return '—'
  if(Math.abs(value)>=100000 || (Math.abs(value)>0&&Math.abs(value)<0.01)) return value.toExponential(2)
  return Number(value.toPrecision(4)).toLocaleString(undefined,{maximumFractionDigits:3})
}
function axisCreateText(group,x,y,text,anchor='middle'){
  const node=document.createElementNS(AXIS_SVG_NS,'text')
  node.setAttribute('x',String(x));node.setAttribute('y',String(y))
  node.setAttribute('text-anchor',anchor)
  node.setAttribute('fill','#475569')
  node.setAttribute('font-size','10')
  node.setAttribute('font-family','system-ui,sans-serif')
  node.textContent=text
  group.appendChild(node)
  return node
}
function axisStateFor(svg){
  if(!svg?.dataset?.axisKind) return null
  let state=axisPlotState.get(svg)
  if (state) return state
  const original=parsePlotBounds(svg.getAttribute('viewBox'))
  const p={left:axisNumber(svg.dataset.axisLeft),right:axisNumber(svg.dataset.axisRight),
    top:axisNumber(svg.dataset.axisTop),bottom:axisNumber(svg.dataset.axisBottom)}
  if(!original || Object.values(p).some(x=>!Number.isFinite(x))||p.right<=p.left||p.bottom<=p.top)return null
  const kind=svg.dataset.axisKind
  if(!['samples','volcano','effects'].includes(kind))return null
  const viewport={x:0,y:0,width:1,height:1}
  const xMin=axisNumber(svg.dataset.axisXmin),xMax=axisNumber(svg.dataset.axisXmax)
  const yMin=axisNumber(svg.dataset.axisYmin),yMax=axisNumber(svg.dataset.axisYmax)
  state={kind,plot:p,original,view:viewport,xMin,xMax,yMin,yMax,
    points:[...svg.querySelectorAll('[data-axis-point]')],
    medians:[...svg.querySelectorAll('[data-axis-median]')],
    bars:[...svg.querySelectorAll('[data-axis-bar]')],
    barLabels:[...svg.querySelectorAll('[data-axis-bar-label]')],
    refsX:[...svg.querySelectorAll('[data-axis-ref-x]')],
    refsY:[...svg.querySelectorAll('[data-axis-ref-y]')],
    metrics:[...svg.querySelectorAll('[data-axis-metric]')]}
  const ticks=document.createElementNS(AXIS_SVG_NS,'g')
  ticks.setAttribute('data-axis-current-ticks','true')
  const annotations=document.createElementNS(AXIS_SVG_NS,'g')
  annotations.setAttribute('data-axis-annotations','true')
  annotations.setAttribute('aria-hidden','true')
  svg.appendChild(ticks);svg.appendChild(annotations)
  state.ticks=ticks;state.annotations=annotations
  axisPlotState.set(svg,state)
  return state
}
function axisScreenScale(svg,state){
  const r=svg.getBoundingClientRect?.()
  return (r?.width>0) ? r.width/state.original.width : 1
}
function axisRedrawLabels(svg,state,visible){
  state.annotations.replaceChildren()
  const zoom=1/state.view.width
  if (zoom<(svg.closest?.('.overview-expanded')?1.5:2.2)) return
  const scale=axisScreenScale(svg,state)
  const radiusX=20/scale, radiusY=13/scale
  const cells=new Map()
  const cell=(x,y)=>`${Math.floor(x/radiusX)},${Math.floor(y/radiusY)}`
  const coords=[]
  for(const p of visible){
    const x=axisNumber(p.getAttribute('cx')),y=axisNumber(p.getAttribute('cy'))
    if(!Number.isFinite(x)||!Number.isFinite(y))continue
    coords.push({node:p,x,y})
    const key=cell(x,y);if(!cells.has(key))cells.set(key,[])
    cells.get(key).push({node:p,x,y})
  }
  const occupied=[]
  const maxLabels=svg.closest?.('.overview-expanded')?40:22
  for(const p of coords){
    if(occupied.length>=maxLabels)break
    const label=p.node.dataset.pointLabel
    if(!label)continue
    // Keep labels only when nearby points do not make identification ambiguous.
    const cx=Math.floor(p.x/radiusX),cy=Math.floor(p.y/radiusY)
    let near=false
    for(let a=-1;a<=1&&!near;a++)for(let b=-1;b<=1&&!near;b++)for(const pt of cells.get(`${cx+a},${cy+b}`)||[]){
      if(pt.node===p.node)continue
      if(Math.abs(pt.x-p.x)<radiusX&&Math.abs(pt.y-p.y)<radiusY){near=true;break}
    }
    if(near)continue
    const approxWidth=Math.min(130,Math.max(28,label.length*6.1))/scale
    let tx=p.x+8/scale,anchor='start'
    if(tx+approxWidth>state.plot.right){tx=p.x-8/scale;anchor='end'}
    if(anchor==='end'&&tx-approxWidth<state.plot.left)continue
    const xl=anchor==='start'?tx:tx-approxWidth,xr=xl+approxWidth
    const y=p.y-6/scale
    if(y<state.plot.top+5/scale||y>state.plot.bottom)continue
    if(occupied.some(q=>xl<q.xr+4/scale&&xr>q.xl-4/scale&&Math.abs(y-q.y)<14/scale))continue
    const labelNode=axisCreateText(state.annotations,tx,y,label,anchor)
    labelNode.setAttribute('font-size',String(10/Math.max(0.75,scale)))
    labelNode.setAttribute('fill','#334155')
    labelNode.setAttribute('paint-order','stroke')
    labelNode.setAttribute('stroke','#ffffff')
    labelNode.setAttribute('stroke-width','2.3')
    labelNode.setAttribute('stroke-linejoin','round')
    occupied.push({xl,xr,y})
  }
}
function redrawAxisPlot(svg,state){
  const {view:v,plot:p,kind}=state
  const x=nx=>p.left+(nx-v.x)/v.width*(p.right-p.left)
  const y=ny=>p.bottom-(ny-v.y)/v.height*(p.bottom-p.top)
  const insideX=nx=>axisShown(nx,v.x,v.x+v.width)
  const insideY=ny=>axisShown(ny,v.y,v.y+v.height)
  const visible=[]
  for(const node of state.points){
    const nx=axisNumber(node.dataset.nx),ny=axisNumber(node.dataset.ny)
    const show=insideX(nx)&&(kind!=='volcano'||insideY(ny))
    node.setAttribute('display',show?'inline':'none')
    if(!show)continue
    node.setAttribute('cx',x(nx).toFixed(3))
    if(kind==='volcano')node.setAttribute('cy',y(ny).toFixed(3))
    else node.setAttribute('cy',node.dataset.fixedY)
    visible.push(node)
  }
  for(const node of state.medians){
    const nx=axisNumber(node.dataset.nx),show=insideX(nx)
    node.setAttribute('display',show?'inline':'none')
    if(show)node.setAttribute('d',`M${x(nx).toFixed(3)} ${node.dataset.fixedY} v17`)
  }
  for(const node of state.refsX){
    const nx=axisNumber(node.dataset.axisRefX),show=insideX(nx)
    node.setAttribute('display',show?'inline':'none')
    if(show){node.setAttribute('x1',x(nx).toFixed(3));node.setAttribute('x2',x(nx).toFixed(3))}
  }
  for(const node of state.refsY){
    const ny=axisNumber(node.dataset.axisRefY),show=insideY(ny)
    node.setAttribute('display',show?'inline':'none')
    if(show){node.setAttribute('y1',y(ny).toFixed(3));node.setAttribute('y2',y(ny).toFixed(3))}
  }
  for(const bar of state.bars){
    const nx=axisNumber(bar.dataset.nx)
    const l=Math.max(Math.min(nx,0.5),v.x),r=Math.min(Math.max(nx,0.5),v.x+v.width)
    const show=r>l&&insideX(axisClamp(nx,v.x,v.x+v.width))
    bar.setAttribute('display',show?'inline':'none')
    if(show){bar.setAttribute('x',x(l).toFixed(3));bar.setAttribute('width',Math.max(0.3,x(r)-x(l)).toFixed(3))}
  }
  for(const node of state.barLabels){
    const nx=axisNumber(node.dataset.nx),show=insideX(nx)
    node.setAttribute('display',show?'inline':'none')
    if(show){
      const endpoint=x(nx)
      const positive=nx>=0.5
      const xx=positive?endpoint+5:endpoint-5
      node.setAttribute('x',String(axisClamp(xx,p.left+5,p.right-5)))
      node.setAttribute('text-anchor',positive?'start':'end')
    }
  }
  for(const metric of state.metrics){
    const lo=axisNumber(metric.dataset.axisMin),hi=axisNumber(metric.dataset.axisMax)
    metric.querySelector('[data-axis-tick="low"]')?.replaceChildren(document.createTextNode(axisText(lo+v.x*(hi-lo))))
    metric.querySelector('[data-axis-tick="high"]')?.replaceChildren(document.createTextNode(axisText(lo+(v.x+v.width)*(hi-lo))))
  }
  state.ticks.replaceChildren()
  if(kind!=='samples'&&Number.isFinite(state.xMin)&&Number.isFinite(state.xMax)){
    for(let i=0;i<=4;i++){
      const t=i/4
      axisCreateText(state.ticks,p.left+t*(p.right-p.left),kind==='effects'?p.bottom+3:p.bottom+13,
        axisText(state.xMin+(v.x+t*v.width)*(state.xMax-state.xMin)))
    }
  }
  if(kind==='volcano'&&Number.isFinite(state.yMin)&&Number.isFinite(state.yMax)){
    for(let i=0;i<=3;i++){
      const t=i/3
      axisCreateText(state.ticks,p.left-6,p.bottom-t*(p.bottom-p.top)+3,
        axisText(state.yMin+(v.y+t*v.height)*(state.yMax-state.yMin)),'end')
    }
  }
  axisRedrawLabels(svg,state,visible)
  svg.dataset.zoom=(1/v.width).toFixed(2)
  // The SVG viewBox stays unchanged; exported SVG/PNG keeps the current coordinates.
}
function axisSetView(svg,state,view){state.view=view;redrawAxisPlot(svg,state)}
function axisEventAnchor(svg,event,state){
  const rect=svg.getBoundingClientRect()
  const px=(event.clientX-rect.left)/Math.max(1,rect.width)*state.original.width+state.original.x
  const py=(event.clientY-rect.top)/Math.max(1,rect.height)*state.original.height+state.original.y
  return {x:axisClamp((px-state.plot.left)/(state.plot.right-state.plot.left),0,1),
    y:axisClamp((state.plot.bottom-py)/(state.plot.bottom-state.plot.top),0,1)}
}

const plotState = new WeakMap()
function viewState(svg) {
  if (!svg) return null
  let state = plotState.get(svg)
  if (!state) {
    const base = parsePlotBounds(svg.getAttribute('viewBox'))
    if (!base) return null
    state = { base, view: { ...base } }
    plotState.set(svg, state)
  }
  return state
}
function setView(svg, next) {
  const state = viewState(svg)
  if (!state || !next) return
  state.view = next
  svg.setAttribute('viewBox', asViewBox(next))
  svg.setAttribute('data-zoom', (state.base.width / next.width).toFixed(2))
}
export function changePlotZoom(svg, factor, ax = 0.5, ay = 0.18) {
  const axis=axisStateFor(svg)
  if(axis){axisSetView(svg,axis,axisZoomWindow(axis.view,factor,ax,ay,axis.kind==='volcano'));return}
  const s=viewState(svg)
  if(s)setView(svg,zoomPlotBounds(s.view,s.base,factor,ax,ay))
}
export function resetPlotZoom(svg) {
  const axis=axisStateFor(svg)
  if(axis){axisSetView(svg,axis,{x:0,y:0,width:1,height:1});return}
  const s=viewState(svg)
  if(s)setView(svg,{...s.base})
}
function attachPanAndZoom(svg) {
  if (!svg) return null
  const axis=axisStateFor(svg)
  const generic=axis?null:viewState(svg)
  if(!axis&&!generic)return null
  if(axis)redrawAxisPlot(svg,axis)
  svg.dataset.interactiveOverview='true'
  const plot=axis?axis.plot:null
  return bindPlotNavigation(svg,{
    plot,
    zoom:(factor,ax,ay)=>changePlotZoom(svg,factor,ax,axis?1-ay:ay),
    reset:()=>resetPlotZoom(svg),
    pan:(dx,dy)=>{
      const rect=svg.getBoundingClientRect()
      if(axis){
        const w=rect.width*(axis.plot.right-axis.plot.left)/axis.original.width
        const h=rect.height*(axis.plot.bottom-axis.plot.top)/axis.original.height
        axisSetView(svg,axis,axisPanWindow(axis.view,dx,dy,w,h,axis.kind==='volcano'))
      }else{
        const state=viewState(svg)
        setView(svg,panPlotBounds(state.view,state.base,dx,dy,rect.width,rect.height))
      }
    },
    box:b=>{
      if(axis){
        const v=axis.view
        const next={x:v.x+b.x0*v.width,y:v.y+(1-b.y1)*v.height,
          width:(b.x1-b.x0)*v.width,height:axis.kind==='volcano'?(b.y1-b.y0)*v.height:1}
        if(next.width>=1/16&&next.height>=1/16)axisSetView(svg,axis,next)
      }else{
        const state=viewState(svg),next=boxZoomView(state.view,b)
        if(next&&next.width>=state.base.width/PLOT_ZOOM_MAX)setView(svg,next)
      }
    }
  })
}

export function csvEscape(value) {
  if (value == null) return ''
  const isText = typeof value === 'string'
  let str = typeof value === 'object' ? JSON.stringify(value) : String(value)
  // Do not modify numerical measurements. Guard text imported into spreadsheet programs.
  if (isText && /^[\t\r\n\s]*[=+@-]/.test(str)) str = `'${str}`
  return /[",\r\n]/.test(str) ? `"${str.replaceAll('"', '""')}"` : str
}
export function tableColumns(rows) {
  const columns = new Set()
  for (const row of rows || []) {
    if (row && typeof row === 'object' && !Array.isArray(row)) {
      for (const key of Object.keys(row)) columns.add(key)
    }
  }
  return [...columns]
}
export function rowsToCsv(rows) {
  if (!Array.isArray(rows) || !rows.length) return ''
  const columns = tableColumns(rows)
  return [columns.map(csvEscape).join(','), ...rows.map(row => columns.map(c => csvEscape(row?.[c])).join(','))].join('\r\n') + '\r\n'
}
export function availableResultTables(entry) {
  const analysis = entry?.analysis
  if (!analysis) return []
  const tables = []
  const add = (kind, label, rows) => {
    if (Array.isArray(rows) && rows.length && rows.some(v => v && typeof v === 'object' && !Array.isArray(v))) {
      tables.push({ kind, label, rows })
    }
  }
  add('adjusted-tests', 'Adjusted tests CSV', analysis.tests)
  add('sample-metrics', 'Sample metrics CSV', analysis.sample_summaries)
  if (Array.isArray(analysis.compositions)) {
    analysis.compositions.forEach((c, i) => add(`composition-${safeName(c.family || i)}`, `${c.family || 'Composition'} tests CSV`, c.tests))
  }
  add('transcript-usage', 'Transcript usage CSV', analysis.dtu?.gene_tests)
  // Preserve other scientific row tables without silently truncating them to displayed rows.
  for (const [key, rows] of Object.entries(analysis)) {
    if (key === 'tests' || key === 'sample_summaries' || key === 'compositions') continue
    if (tables.some(t => t.kind === safeName(key))) continue
    add(safeName(key), `${key.replaceAll('_',' ')} CSV`, rows)
  }
  return tables
}
function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.hidden = true
  document.body.appendChild(link)
  link.click()
  link.remove()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
function downloadTable(table, name) {
  if (!Array.isArray(table.rows) || !table.rows.length) throw new Error('This result table has no rows to export.')
  const columns = tableColumns(table.rows)
  const chunks = ['\ufeff', columns.map(csvEscape).join(',') + '\r\n']
  let buffer = ''
  for (const row of table.rows) {
    buffer += columns.map(c => csvEscape(row?.[c])).join(',') + '\r\n'
    if (buffer.length >= 262144) { chunks.push(buffer); buffer = '' }
  }
  if (buffer) chunks.push(buffer)
  downloadBlob(new Blob(chunks, { type: 'text/csv;charset=utf-8' }), `${safeName(name)}-${safeName(table.kind)}.csv`)
}
function formatSvgForExport(svg) {
  const clone = svg.cloneNode(true)
  clone.setAttribute('xmlns', SVG_NS)
  clone.removeAttribute('tabindex')
  clone.removeAttribute('aria-keyshortcuts')
  clone.removeAttribute('data-zoom')
  clone.removeAttribute('data-interactive-overview')
  clone.classList?.remove('overview-panning')
  // In exported files, source-page CSS is unavailable. Copy computed plot styles
  // into the SVG so the downloaded image preserves the visible colors and fonts.
  if (typeof getComputedStyle === 'function') {
    const sourceNodes = [svg, ...svg.querySelectorAll('*')]
    const cloneNodes = [clone, ...clone.querySelectorAll('*')]
    for (let i = 0; i < sourceNodes.length; i++) {
      const computed = getComputedStyle(sourceNodes[i])
      const inline = ['fill','fill-opacity','stroke','stroke-width','stroke-dasharray','stroke-opacity',
        'opacity','font-family','font-size','font-weight','font-style','text-anchor','display']
        .map(name => `${name}:${computed.getPropertyValue(name)}`).join(';')
      cloneNodes[i].setAttribute('style', `${cloneNodes[i].getAttribute('style') || ''};${inline}`)
    }
  }
  const bounds = parsePlotBounds(svg.getAttribute('viewBox'))
  if (!bounds) throw new Error('This plot has no valid SVG viewBox.')
  clone.setAttribute('width', String(Math.round(bounds.width)))
  clone.setAttribute('height', String(Math.round(bounds.height)))
  const style = document.createElementNS(SVG_NS, 'style')
  style.textContent = PLOT_STYLES
  clone.insertBefore(style, clone.firstChild)
  const bg = document.createElementNS(SVG_NS,'rect')
  bg.setAttribute('x', String(bounds.x))
  bg.setAttribute('y', String(bounds.y))
  bg.setAttribute('width', String(bounds.width))
  bg.setAttribute('height', String(bounds.height))
  bg.setAttribute('fill', '#ffffff')
  clone.insertBefore(bg, style.nextSibling)
  return { markup: new XMLSerializer().serializeToString(clone), bounds }
}
export function downloadPlotSvg(svg, name) {
  const { markup } = formatSvgForExport(svg)
  downloadBlob(new Blob([markup],{type:'image/svg+xml;charset=utf-8'}),`${safeName(name)}.svg`)
}
export async function downloadPlotPng(svg, name) {
  const { markup, bounds } = formatSvgForExport(svg)
  const blob = new Blob([markup], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  try {
    const img = new Image()
    img.src = url
    if (typeof img.decode === 'function') await img.decode()
    else await new Promise((resolve, reject) => { img.onload = resolve; img.onerror = reject })
    const factor = 2
    const width = Math.round(bounds.width * factor), height = Math.round(bounds.height * factor)
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('This browser cannot create a PNG canvas.')
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0,0,width,height)
    ctx.drawImage(img,0,0,width,height)
    const png = await new Promise((resolve, reject) => canvas.toBlob(b => b ? resolve(b) : reject(new Error('PNG conversion failed.')), 'image/png'))
    downloadBlob(png, `${safeName(name)}.png`)
  } finally { URL.revokeObjectURL(url) }
}
function makeButton(text, title, handler) {
  const button = document.createElement('button')
  button.type = 'button'
  button.className = 'secondary-button results-export-button'
  button.textContent = text
  button.title = title
  button.addEventListener('click', async () => {
    button.disabled = true
    try { await handler() }
    catch (error) {
      const message = button.closest('.plot-card')?.querySelector('.plot-heading > .results-export-error')
        || button.closest('.overview-modality')?.querySelector(':scope > .results-export-error')
        || button.closest('.comparison-result')?.querySelector('.card-heading > .results-export-error')
      if (message) { message.hidden = false; message.textContent = `Export failed: ${error.message}` }
      else console.error('4C Insight export failed:', error)
    } finally { button.disabled = false }
  })
  return button
}
function plotExportControls(svg, key, { zoom = false } = {}) {
  const actions = document.createElement('div')
  actions.className = 'results-plot-tools'
  actions.setAttribute('role','group')
  actions.setAttribute('aria-label',`${RESULT_NAMES[key] || key} plot controls`)
  if(zoom){
    const layout=document.createElement('div')
    layout.innerHTML=plotNavigationToolbar({compact:true})
    actions.appendChild(layout.firstElementChild)
  }
  actions.appendChild(makeButton('SVG','Save the currently viewed plot as a vector image',() => downloadPlotSvg(svg,`4C-Insight-${key}-plot`)))
  actions.appendChild(makeButton('PNG','Save the currently viewed plot as a high-resolution image',() => downloadPlotPng(svg,`4C-Insight-${key}-plot`)))
  return actions
}
function makeTableControls(entry, key) {
  const tables = availableResultTables(entry)
  const group = document.createElement('div')
  group.className = 'results-table-tools'
  group.setAttribute('role','group')
  group.setAttribute('aria-label',`${RESULT_NAMES[key] || key} data exports`)
  for (const table of tables) group.appendChild(makeButton(table.label,`Save every row of ${table.label}; not only displayed points`,() => downloadTable(table,`4C-Insight-${key}`)))
  return group
}
export function bindOverviewInteractions(root, result, onExpand = null) {
  root.querySelectorAll('[data-overview-modality]').forEach(card => {
    const key = card.dataset.overviewModality
    const svg = card.querySelector('svg.overview-svg')
    const tools = card.querySelector('.overview-card-tools')
    if (svg) {
      card.dataset.plotNavRoot='true'
      const navigation=attachPanAndZoom(svg)
      if (tools && !tools.querySelector('.results-plot-tools')) {
        tools.appendChild(plotExportControls(svg,key,{zoom:true}))
        bindPlotToolbar(card,navigation)
      }
    }
    const expand = card.querySelector('[data-overview-expand]')
    if (expand) expand.addEventListener('click',() => {
      const expanded = !card.classList.contains('overview-expanded')
      card.classList.toggle('overview-expanded', expanded)
      expand.setAttribute('aria-expanded', String(expanded))
      expand.textContent = expanded ? 'Reduce' : 'Expand'
      if(svg){const axis=axisStateFor(svg);if(axis)redrawAxisPlot(svg,axis)}
      onExpand?.(key, expanded)
    })
    if (tools && result?.[key] && !tools.querySelector('.results-table-tools')) tools.appendChild(makeTableControls(result[key],key))
  })
}
export function bindDetailedResultsExports(root, result) {
  root.querySelectorAll('.comparison-result').forEach(section => {
    const title = section.querySelector('h3')?.textContent?.trim()
    const key = Object.keys(RESULT_NAMES).find(k => RESULT_NAMES[k] === title)
    if (!key || !result?.[key]) return
    const heading = section.querySelector('.card-heading')
    if (heading && !heading.querySelector('.results-table-tools')) heading.appendChild(makeTableControls(result[key],key))
    section.querySelectorAll('.plot-card').forEach((plot, i) => {
      const svg = plot.querySelector('svg')
      if (!svg || plot.querySelector('.results-plot-tools')) return
      const header = plot.querySelector('.plot-heading') || plot
      const controls = plotExportControls(svg, `${key}-${i}`)
      header.appendChild(controls)
      const error = document.createElement('p'); error.className = 'results-export-error'; error.hidden = true
      header.appendChild(error)
    })
    if (heading && !heading.querySelector(':scope > .results-export-error')) {
      const error = document.createElement('p'); error.className = 'results-export-error'; error.hidden = true
      error.setAttribute('role','alert')
      heading.appendChild(error)
    }
  })
}
