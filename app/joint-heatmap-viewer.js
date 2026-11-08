/**
 * Interactive, strictly descriptive joint-distance heatmap.
 * The original clustering is never re-fit when the view is cropped.
 * Input distances come from the complete fixed-modality scientific model.
 * Labels and patient information are shown only after fitting.
 */
const clamp = (value, lo, hi) => Math.min(hi, Math.max(lo, value))
const finite = value => typeof value === 'number' && Number.isFinite(value)
const DEFAULT_SCALE = 'p95'
const LEGEND = 'Normalized joint distance (unitless; each source divided by its median positive pair distance)'

export function completeWindow(n) {
  if (!Number.isInteger(n) || n < 2) throw new Error('A heatmap requires at least two observations')
  return { rowStart: 0, rowEnd: n, columnStart: 0, columnEnd: n }
}

export function validateHeatmapWindow(window, n) {
  if (!window || !Number.isInteger(n) || n < 2) throw new Error('Invalid distance heatmap')
  for (const value of Object.values(window)) {
    if (!Number.isInteger(value)) throw new Error('Heatmap window indices must be integers')
  }
  if (window.rowStart < 0 || window.columnStart < 0 ||
      window.rowEnd > n || window.columnEnd > n ||
      window.rowEnd - window.rowStart < 2 || window.columnEnd - window.columnStart < 2) {
    throw new Error('The selected region must contain at least two rows and two columns')
  }
  return { ...window }
}

/**
 * Map a dragged rectangle in actual canvas CSS pixel coordinates to inclusive /
 * exclusive original dendrogram-leaf positions. Both axes can be different.
 */
export function boxToWindow(window, bounds, width, height, n) {
  validateHeatmapWindow(window, n)
  if (![width, height, bounds?.x0, bounds?.x1, bounds?.y0, bounds?.y1].every(finite) ||
      width <= 0 || height <= 0) throw new Error('Invalid heatmap pointer coordinates')
  const cols = window.columnEnd - window.columnStart
  const rows = window.rowEnd - window.rowStart
  const loX = clamp(Math.min(bounds.x0, bounds.x1), 0, width)
  const hiX = clamp(Math.max(bounds.x0, bounds.x1), 0, width)
  const loY = clamp(Math.min(bounds.y0, bounds.y1), 0, height)
  const hiY = clamp(Math.max(bounds.y0, bounds.y1), 0, height)
  if (hiX - loX < 8 || hiY - loY < 8) return null
  const next = {
    columnStart: window.columnStart + Math.floor(loX * cols / width),
    columnEnd: window.columnStart + Math.ceil(hiX * cols / width),
    rowStart: window.rowStart + Math.floor(loY * rows / height),
    rowEnd: window.rowStart + Math.ceil(hiY * rows / height),
  }
  try { validateHeatmapWindow(next, n) } catch { return null }
  if (next.rowStart === window.rowStart && next.rowEnd === window.rowEnd &&
      next.columnStart === window.columnStart && next.columnEnd === window.columnEnd) return null
  return next
}

export function heatmapDisplayScale(distances, n, mode = DEFAULT_SCALE) {
  if ((!Array.isArray(distances) && !(distances instanceof Float64Array)) || distances.length !== n * n) {
    throw new Error('Distance matrix must be a complete square')
  }
  if (!['p95', 'max'].includes(mode)) throw new Error('Unknown heatmap display scale')
  for(let i=0;i<n;i++) {
    const diagonal=distances[i*n+i]
    if(!finite(diagonal) || Math.abs(diagonal)>1e-9) throw new Error('Distance matrix diagonal must be zero')
  }
  const positive = []
  for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) {
    const d = distances[i*n+j]
    if (!finite(d) || !finite(distances[j*n+i]) || d < 0 || Math.abs(distances[j*n+i] - d) > 1e-8) {
      throw new Error('Heatmap requires finite, nonnegative, symmetric distances')
    }
    if (d > 0) positive.push(d)
  }
  positive.sort((a, b) => a - b)
  const maximum = positive.at(-1) || 0
  const index = Math.max(0, Math.ceil(positive.length * 0.95) - 1)
  const p95 = positive[index] || maximum
  return { maximum, cap: mode === 'max' ? maximum : p95, mode, unit: LEGEND, positivePairs: positive.length }
}

export function distanceFill(distance, scale, diagonal = false) {
  if (diagonal) return '#ffffff'
  if (!finite(distance) || distance < 0) return '#e5e8ed'
  const t = scale.cap > 0 ? clamp(distance / scale.cap, 0, 1) : 0
  const shade = Math.round(253 - 181*t)
  return `rgb(${shade},${shade},252)`
}

export function heatmapCellSize(window, expanded = false) {
  const maxVisible = Math.max(window.rowEnd - window.rowStart, window.columnEnd - window.columnStart)
  // The source matrix is not thinned. Only the on-screen cell width changes.
  if (maxVisible > 750) return 3
  if (maxVisible > 400) return 5
  if (maxVisible > 260) return expanded ? 10 : 8
  if (maxVisible > 120) return expanded ? 13 : 10
  if (maxVisible > 65) return expanded ? 18 : 14
  if (maxVisible > 35) return expanded ? 24 : 19
  return expanded ? 36 : 30
}

export function clusteredSampleIds(result, window) {
  const order = result?.clustering?.leaf_order
  const names = result?.observations
  if (!Array.isArray(names) || !Array.isArray(order) || order.length !== names.length) {
    throw new Error('The original clustered leaf order is missing')
  }
  validateHeatmapWindow(window, names.length)
  const perm = new Set(order)
  if (perm.size !== names.length || order.some(v=>!Number.isInteger(v)||v<0||v>=names.length)) {
    throw new Error('Invalid clustered sample order')
  }
  return {
    rows: order.slice(window.rowStart, window.rowEnd).map(i=>names[i]),
    columns: order.slice(window.columnStart, window.columnEnd).map(i=>names[i]),
  }
}

const escapeText = text => String(text).replaceAll('&','&amp;').replaceAll('<','&lt;')
  .replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#039;')

/** QC review only: highest median distance is not a validated biological outlier. */
export function rankTypicalJointDistances(result, limit = 10) {
  const names = result?.observations
  const matrix = result?.distances
  const n = names?.length
  if (!Array.isArray(names) || (!Array.isArray(matrix) && !(matrix instanceof Float64Array)) ||
      matrix.length !== n*n || !Number.isInteger(limit) || limit < 1) {
    throw new Error('A complete joint matrix is required for sample-distance review')
  }
  return names.map((name,i)=>{
    const values=[]
    for(let j=0;j<n;j++) if(i!==j) {
      const x=matrix[i*n+j]
      if(!finite(x)||x<0) throw new Error('Invalid sample distance in QC review')
      values.push(x)
    }
    values.sort((a,b)=>a-b)
    const k=Math.floor(values.length/2)
    const median=values.length%2?values[k]:(values[k-1]+values[k])/2
    return {observation:name,median_distance:median,measured_pairs:values.length}
  }).sort((a,b)=>b.median_distance-a.median_distance||a.observation.localeCompare(b.observation)).slice(0,limit)
}

export function jointDistanceReviewMarkup(result) {
  const rows=rankTypicalJointDistances(result)
  return `<details class="details-block joint-distance-review"><summary>Review samples with large typical joint distances</summary>
    <p class="method-note">High median distance can indicate an isolated observation, a source-data difference, or a QC issue. This is a descriptive review list, <strong>not an outlier test</strong>. Check each source modality before excluding any observation.</p>
    <div class="table-scroll"><table><thead><tr><th>Observation</th><th>Median distance to other retained observations</th><th>Measured pairs</th></tr></thead>
    <tbody>${rows.map(row=>`<tr><td>${escapeText(row.observation)}</td><td>${row.median_distance.toPrecision(5)}</td><td>${row.measured_pairs}</td></tr>`).join('')}</tbody></table></div>
  </details>`
}

export function heatmapViewerMarkup() {
  return `<div class="joint-heatmap-viewer" id="joint-heatmap-viewer" aria-label="Interactive clustered joint distances">
    <div class="joint-heatmap-toolbar" role="group" aria-label="Heatmap view controls">
      <strong>Clustered distance matrix</strong>
      <span class="joint-heatmap-toolbar-actions">
        <button type="button" id="joint-heatmap-back" class="secondary-button" disabled>Previous zoom</button>
        <button type="button" id="joint-heatmap-reset" class="secondary-button">Show all</button>
        <button type="button" id="joint-heatmap-expand" class="secondary-button" aria-expanded="false">Expand view</button>
      </span>
      <label for="joint-heatmap-scale">Color range
        <select id="joint-heatmap-scale"><option value="p95">95th percentile (more detail)</option><option value="max">Maximum distance</option></select>
      </label>
    </div>
    <div class="joint-heatmap-color-key">
      <span id="joint-heatmap-key-title">Normalized joint distance (unitless)</span>
      <div class="joint-heatmap-gradient" role="img" aria-label="White: near zero distance; dark blue: large distance"></div>
      <div class="joint-heatmap-key-numbers"><span>0 · very similar</span><span id="joint-heatmap-cap">Scale not available</span></div>
      <small id="joint-heatmap-range-note">Colors are descriptive distances, not significance or expression changes.</small>
    </div>
    <p class="method-note joint-heatmap-instructions">Drag a box across the matrix to zoom into the selected rows and columns. The top and left dendrograms show the matching portion of the <strong>original clustering</strong>; the model is not recalculated. Zoom in to display more sample IDs.</p>
    <div class="joint-heatmap-scroll" id="joint-heatmap-scroll" role="region" tabindex="0" aria-label="Scrollable clustered distance heatmap, with top and left dendrograms">
      <div class="joint-heatmap-grid" id="joint-heatmap-grid">
        <canvas id="joint-top-tree" role="img" aria-label="Original dendrogram, columns"></canvas>
        <canvas id="joint-column-labels" role="img" aria-label="Column sample IDs"></canvas>
        <canvas id="joint-left-tree" role="img" aria-label="Original dendrogram, rows"></canvas>
        <canvas id="joint-row-labels" role="img" aria-label="Row sample IDs"></canvas>
        <div class="joint-heatmap-matrix-wrap" id="joint-heatmap-matrix-wrap">
          <canvas id="joint-heatmap-matrix" aria-label="Clustered joint sample distances; drag a box to zoom" role="img"></canvas>
          <div id="joint-heatmap-selection" class="joint-heatmap-selection" hidden></div>
        </div>
      </div>
    </div>
    <p class="joint-heatmap-status" id="joint-heatmap-status" role="status" aria-live="polite">Move the pointer over a cell to show the original sample IDs and distance.</p>
    <p class="method-note" id="joint-heatmap-window-description"></p>
  </div>`
}

let active = null
let lastResult = null
let viewer = null
let abortEvents = null
function getState(result) {
  if (lastResult !== result || !viewer) {
    active = {window:completeWindow(result.observations.length),history:[],expanded:false,mode:DEFAULT_SCALE}
    lastResult = result
  }
  return active
}

function sizeCanvas(canvas, width, height) {
  canvas.width = Math.max(1, Math.round(width))
  canvas.height = Math.max(1, Math.round(height))
  canvas.style.width = `${width}px`
  canvas.style.height = `${height}px`
  const ctx=canvas.getContext('2d')
  if (!ctx) throw new Error('This browser cannot draw the joint heatmap')
  return ctx
}

function treeLayout(result, cell) {
  const n=result.observations.length
  const order=result.clustering.leaf_order
  const merges=result.clustering.merges
  const nodes=new Map(order.map((leaf,i)=>[leaf,{p:(i+.5)*cell,y:0,count:1}]))
  const maximum=Math.max(...merges.map(m=>m.height),1e-12)
  const links=[]
  for (const merge of merges) {
    const a=nodes.get(merge.left),b=nodes.get(merge.right)
    if (!a||!b) throw new Error('Joint dendrogram has an invalid merge')
    const y=clamp(merge.height/maximum,0,1)
    const p=(a.p*a.count+b.p*b.count)/(a.count+b.count)
    links.push({a,b,p,y})
    nodes.set(merge.id,{p,y,count:a.count+b.count})
  }
  return links
}

function drawTree(ctx, result, cell, start, end, orientation) {
  const height=114
  const links=treeLayout(result, cell)
  const leaf = position => position-start*cell
  ctx.save()
  ctx.strokeStyle='#526885'
  ctx.lineWidth=1
  ctx.beginPath()
  for (const link of links) {
    const a=leaf(link.a.p),b=leaf(link.b.p)
    // Keep the global clustering geometry: clip branches at crop boundaries.
    if (orientation==='top') {
      const y=height-6-link.y*(height-16)
      const ya=height-6-link.a.y*(height-16),yb=height-6-link.b.y*(height-16)
      ctx.moveTo(a,ya);ctx.lineTo(a,y);ctx.lineTo(b,y);ctx.lineTo(b,yb)
    } else {
      const x=height-6-link.y*(height-16)
      const xa=height-6-link.a.y*(height-16),xb=height-6-link.b.y*(height-16)
      ctx.moveTo(xa,a);ctx.lineTo(x,a);ctx.lineTo(x,b);ctx.lineTo(xb,b)
    }
  }
  ctx.stroke()
  ctx.restore()
}

function drawAxisLabels(ctx, result, window, cell, orientation) {
  const {observations,clustering:{leaf_order:order}}=result
  const start=orientation==='row'?window.rowStart:window.columnStart
  const end=orientation==='row'?window.rowEnd:window.columnEnd
  const stride=orientation==='row'?Math.max(1,Math.ceil(13/cell)):Math.max(1,Math.ceil(20/cell))
  ctx.fillStyle='#24324b'
  ctx.font='11px system-ui, sans-serif'
  for (let index=start;index<end;index+=stride) {
    const name=observations[order[index]]
    const p=(index-start+0.5)*cell
    if (orientation==='row') {
      ctx.textAlign='right';ctx.textBaseline='middle'
      ctx.fillText(name, 158, p, 151)
    } else {
      ctx.save();ctx.translate(p,127);ctx.rotate(-Math.PI*0.37)
      ctx.textAlign='left';ctx.textBaseline='middle'
      ctx.fillText(name,0,0,150);ctx.restore()
    }
  }
}

function scrollCenter(scroll) {
  scroll.scrollTop=0
  scroll.scrollLeft=0
}

function draw(result, root, state, resetScroll = false) {
  const n=result.observations.length
  const window=validateHeatmapWindow(state.window,n)
  const cell=heatmapCellSize(window,state.expanded)
  const cols=window.columnEnd-window.columnStart
  const rows=window.rowEnd-window.rowStart
  const width=cols*cell,height=rows*cell
  const scale=heatmapDisplayScale(result.distances,n,state.mode)
  const canvases={
    top:root.querySelector('#joint-top-tree'),
    topLabels:root.querySelector('#joint-column-labels'),
    left:root.querySelector('#joint-left-tree'),
    rowLabels:root.querySelector('#joint-row-labels'),
    matrix:root.querySelector('#joint-heatmap-matrix'),
  }
  const ctx=sizeCanvas(canvases.matrix,width,height)
  const order=result.clustering.leaf_order
  for(let row=0;row<rows;row++){
    const ri=order[row+window.rowStart]
    for(let col=0;col<cols;col++){
      const ci=order[col+window.columnStart]
      const distance=result.distances[ri*n+ci]
      ctx.fillStyle=distanceFill(distance,scale,ri===ci)
      ctx.fillRect(col*cell,row*cell,cell,cell)
    }
  }
  drawTree(sizeCanvas(canvases.top,width,114),result,cell,window.columnStart,window.columnEnd,'top')
  drawTree(sizeCanvas(canvases.left,114,height),result,cell,window.rowStart,window.rowEnd,'left')
  drawAxisLabels(sizeCanvas(canvases.topLabels,width,134),result,window,cell,'column')
  drawAxisLabels(sizeCanvas(canvases.rowLabels,160,height),result,window,cell,'row')
  const grid=root.querySelector('#joint-heatmap-grid')
  grid.style.gridTemplateColumns=`114px 160px ${width}px`
  grid.style.gridTemplateRows=`114px 134px ${height}px`
  root.querySelector('#joint-heatmap-cap').textContent=`${scale.cap.toPrecision(4)} · ${state.mode==='p95'?'95th percentile':'maximum'}`
  root.querySelector('#joint-heatmap-range-note').textContent=`${LEGEND}. Darker blue means greater distance. Values over ${scale.cap.toPrecision(4)} are saturated in this display; original values remain unchanged.`
  root.querySelector('#joint-heatmap-back').disabled=!state.history.length
  root.querySelector('#joint-heatmap-expand').textContent=state.expanded?'Close expanded view':'Expand view'
  root.querySelector('#joint-heatmap-expand').setAttribute('aria-expanded',String(state.expanded))
  root.querySelector('#joint-heatmap-scale').value=state.mode
  root.classList.toggle('is-expanded',state.expanded)
  const caption=`Showing ${rows} row samples (${window.rowStart+1}–${window.rowEnd}) and ${cols} column samples (${window.columnStart+1}–${window.columnEnd}) of ${n} clustered observations. Sample names appear when space permits. Box zoom does not re-fit the dendrogram.`
  root.querySelector('#joint-heatmap-window-description').textContent=caption
  const status=root.querySelector('#joint-heatmap-status')
  status.textContent=caption
  const matrix=canvases.matrix
  matrix.dataset.cell=String(cell)
  if(resetScroll) scrollCenter(root.querySelector('#joint-heatmap-scroll'))
  return {matrix,window,scale,cell,rows,cols}
}

export function mountJointHeatmapViewer(result) {
  if(!result) return
  const root=document.querySelector('#joint-heatmap-viewer')
  if(!root) return
  if(abortEvents) abortEvents.abort()
  abortEvents=new AbortController()
  viewer=root
  const state=getState(result)
  const signal=abortEvents.signal
  const refresh=(resetScroll=false)=>draw(result,root,state,resetScroll)
  const initial=refresh()
  const matrix=root.querySelector('#joint-heatmap-matrix')
  const selection=root.querySelector('#joint-heatmap-selection')
  const hover=root.querySelector('#joint-heatmap-status')
  let start=null,latest=null
  function localPosition(event) {
    const rect=matrix.getBoundingClientRect()
    return {x:clamp(event.clientX-rect.left,0,rect.width),y:clamp(event.clientY-rect.top,0,rect.height),width:rect.width,height:rect.height}
  }
  function setSelection(a,b) {
    const l=Math.min(a.x,b.x),t=Math.min(a.y,b.y)
    selection.hidden=false
    selection.style.left=`${l}px`
    selection.style.top=`${t}px`
    selection.style.width=`${Math.abs(b.x-a.x)}px`
    selection.style.height=`${Math.abs(b.y-a.y)}px`
  }
  matrix.addEventListener('pointerdown',event=>{
    if(event.button!==0)return
    start=localPosition(event);latest=start
    matrix.setPointerCapture?.(event.pointerId)
    setSelection(start,latest)
    event.preventDefault()
  },{signal})
  matrix.addEventListener('pointermove',event=>{
    const p=localPosition(event)
    if(start){latest=p;setSelection(start,p);return}
    const win=state.window
    const x=win.columnStart+Math.floor(p.x*(win.columnEnd-win.columnStart)/Math.max(1,p.width))
    const y=win.rowStart+Math.floor(p.y*(win.rowEnd-win.rowStart)/Math.max(1,p.height))
    if(x>=win.columnEnd||y>=win.rowEnd)return
    const indices=result.clustering.leaf_order
    const row=indices[y],col=indices[x]
    const value=result.distances[row*result.observations.length+col]
    hover.textContent=`Row: ${result.observations[row]} · Column: ${result.observations[col]} · Joint distance: ${value.toPrecision(6)} (unitless)`
  },{signal})
  matrix.addEventListener('pointerup',event=>{
    if(!start)return
    latest=localPosition(event)
    const next=boxToWindow(state.window,{x0:start.x,y0:start.y,x1:latest.x,y1:latest.y},latest.width,latest.height,result.observations.length)
    start=null;latest=null;selection.hidden=true
    if(next){state.history.push({...state.window});state.window=next;refresh(true)}
  },{signal})
  matrix.addEventListener('pointercancel',()=>{start=null;latest=null;selection.hidden=true},{signal})
  root.querySelector('#joint-heatmap-back').addEventListener('click',()=>{
    if(!state.history.length)return
    state.window=state.history.pop();refresh(true)
  },{signal})
  root.querySelector('#joint-heatmap-reset').addEventListener('click',()=>{
    state.window=completeWindow(result.observations.length);state.history=[];refresh(true)
  },{signal})
  root.querySelector('#joint-heatmap-expand').addEventListener('click',()=>{
    state.expanded=!state.expanded;refresh()
    if(state.expanded)root.querySelector('#joint-heatmap-expand').focus()
  },{signal})
  root.querySelector('#joint-heatmap-scale').addEventListener('change',event=>{
    state.mode=event.currentTarget.value;refresh()
  },{signal})
  document.addEventListener('keydown',event=>{
    if(event.key==='Escape' && state.expanded){state.expanded=false;refresh();event.preventDefault()}
  },{signal})
  return initial
}
