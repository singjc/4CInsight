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
export function changePlotZoom(svg, factor, ax = 0.5, ay = 0.5) {
  const s = viewState(svg)
  if (s) setView(svg, zoomPlotBounds(s.view, s.base, factor, ax, ay))
}
export function resetPlotZoom(svg) {
  const s = viewState(svg)
  if (s) setView(svg, { ...s.base })
}
function attachPanAndZoom(svg) {
  if (!svg || svg.dataset.interactiveOverview === 'true') return
  if (!viewState(svg)) return
  svg.dataset.interactiveOverview = 'true'
  svg.setAttribute('tabindex', '0')
  svg.setAttribute('aria-keyshortcuts', '+ - 0')
  svg.addEventListener('wheel', (event) => {
    // Ctrl/Cmd prevents long-page scrolling from unexpectedly zooming a plot.
    if (!event.ctrlKey && !event.metaKey) return
    event.preventDefault()
    const rect = svg.getBoundingClientRect()
    const x = clamp((event.clientX - rect.left) / Math.max(1, rect.width), 0, 1)
    const y = clamp((event.clientY - rect.top) / Math.max(1, rect.height), 0, 1)
    changePlotZoom(svg, event.deltaY < 0 ? 0.8 : 1.25, x, y)
  }, { passive: false })
  let drag = null
  svg.addEventListener('pointerdown', (event) => {
    if (event.button !== 0 || event.pointerType === 'touch') return
    const s = viewState(svg)
    if (!s || s.view.width >= s.base.width) return
    drag = { x: event.clientX, y: event.clientY }
    svg.setPointerCapture?.(event.pointerId)
    svg.classList.add('overview-panning')
  })
  svg.addEventListener('pointermove', (event) => {
    if (!drag) return
    const rect = svg.getBoundingClientRect()
    const s = viewState(svg)
    setView(svg, panPlotBounds(s.view, s.base, event.clientX-drag.x, event.clientY-drag.y, rect.width, rect.height))
    drag = { x: event.clientX, y: event.clientY }
  })
  const finish = () => { drag = null; svg.classList.remove('overview-panning') }
  svg.addEventListener('pointerup', finish)
  svg.addEventListener('pointercancel', finish)
  svg.addEventListener('dblclick', () => resetPlotZoom(svg))
  svg.addEventListener('keydown', (event) => {
    if (event.key === '+' || event.key === '=') { event.preventDefault(); changePlotZoom(svg, 0.8) }
    else if (event.key === '-') { event.preventDefault(); changePlotZoom(svg, 1.25) }
    else if (event.key === '0') { event.preventDefault(); resetPlotZoom(svg) }
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
  if (zoom) {
    const controls = [
      ['Zoom +','Zoom into the chart',() => changePlotZoom(svg, 0.75)],
      ['Zoom −','Zoom out of the chart',() => changePlotZoom(svg, 1.35)],
      ['Reset','Show the whole chart',() => resetPlotZoom(svg)],
    ]
    controls.forEach(([label,title,action]) => actions.appendChild(makeButton(label,title,action)))
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
      attachPanAndZoom(svg)
      if (tools && !tools.querySelector('.results-plot-tools')) tools.appendChild(plotExportControls(svg,key,{zoom:true}))
    }
    const expand = card.querySelector('[data-overview-expand]')
    if (expand) expand.addEventListener('click',() => {
      const expanded = !card.classList.contains('overview-expanded')
      card.classList.toggle('overview-expanded', expanded)
      expand.setAttribute('aria-expanded', String(expanded))
      expand.textContent = expanded ? 'Reduce' : 'Expand'
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
