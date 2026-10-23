import { saveTextFile } from './save.js'

const tabs = [
  ['project', 'Project', '1'],
  ['qc', 'Explore & QC', '2'],
  ['compare', 'Compare Groups', '3'],
  ['longitudinal', 'Longitudinal', '4'],
  ['hits', 'Significant Hits', '5'],
  ['results', 'Results', '6'],
  ['docs', 'Docs', '?'],
]

const state = {
  tab: 'project',
  files: new Map(),
  records: [],
  busy: false,
  error: null,
  engine: 'Waiting for local inputs',
  wasmReady: false,
}

const pending = new Map()
const worker = new Worker(new URL('./analysis-worker.js', import.meta.url), { type: 'module' })
worker.addEventListener('message', (event) => {
  const request = pending.get(event.data.requestId)
  if (!request) return
  pending.delete(event.data.requestId)
  event.data.type === 'worker-error'
    ? request.reject(new Error(event.data.message))
    : request.resolve(event.data)
})
worker.addEventListener('error', (event) => {
  const error = new Error(event.message || '4C Insight analysis worker failed')
  for (const request of pending.values()) request.reject(error)
  pending.clear()
})

function preflightFiles(files) {
  const requestId = crypto.randomUUID()
  return new Promise((resolve, reject) => {
    pending.set(requestId, { resolve, reject })
    worker.postMessage({ type: 'preflight', requestId, files })
  })
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;')
}

function humanBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`
  const units = ['KiB', 'MiB', 'GiB', 'TiB']
  let value = bytes / 1024
  let index = 0
  while (value >= 1024 && index < units.length - 1) { value /= 1024; index += 1 }
  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${units[index]}`
}

function fileKey(file) {
  return JSON.stringify([file.webkitRelativePath || file.name, file.size, file.lastModified])
}

function fileMetadata(file) {
  return {
    id: fileKey(file),
    name: file.name,
    relativePath: file.webkitRelativePath || file.name,
    size: file.size,
    lastModified: file.lastModified,
    mimeType: file.type,
  }
}

async function refreshFiles(nextFiles) {
  state.files = nextFiles
  state.busy = true
  state.error = null
  render()
  try {
    const response = await preflightFiles([...nextFiles.values()].map(fileMetadata))
    state.records = response.records
    state.wasmReady = response.wasmReady
    state.engine = response.wasmReady
      ? `Rust/WASM ${response.engineVersion || ''}`.trim()
      : 'JavaScript preflight fallback'
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    render()
  }
}

function addFiles(fileList) {
  if (!fileList) return
  const next = new Map(state.files)
  for (const file of Array.from(fileList)) next.set(fileKey(file), file)
  void refreshFiles(next)
}

function removeFile(id) {
  const next = new Map(state.files)
  next.delete(id)
  void refreshFiles(next)
}

function clearProject() {
  state.files = new Map()
  state.records = []
  state.error = null
  state.engine = 'Waiting for local inputs'
  state.wasmReady = false
  render()
}

async function exportManifest() {
  const manifest = {
    product: '4C Insight',
    schemaVersion: 1,
    createdAt: new Date().toISOString(),
    executionModel: 'local-browser',
    files: state.records.map(({ id: _id, ...record }) => record),
  }
  await saveTextFile('4c-insight-project-manifest.json', `${JSON.stringify(manifest, null, 2)}\n`)
}

function modalityCounts() {
  const counts = new Map()
  for (const record of state.records.filter((record) => record.recognized)) {
    const label = record.label || 'Other'
    counts.set(label, (counts.get(label) || 0) + 1)
  }
  return [...counts.entries()].sort((a, b) => a[0].localeCompare(b[0]))
}

function recognizedCount() { return state.records.filter((record) => record.recognized).length }
function totalBytes() { return state.records.reduce((sum, record) => sum + record.size, 0) }

function renderNav() {
  document.querySelector('#workflow-nav').innerHTML = tabs.map(([id, label, short]) => `
    <button class="nav-item ${state.tab === id ? 'active' : ''}" data-tab="${id}" ${state.tab === id ? 'aria-current="page"' : ''}>
      <span class="nav-index">${short}</span><span>${label}</span>
    </button>`).join('')
  document.querySelectorAll('[data-tab]').forEach((button) => {
    button.addEventListener('click', () => { state.tab = button.dataset.tab; render() })
  })

  document.querySelector('#sidebar-status').innerHTML = `
    <span class="status-dot ${state.wasmReady ? 'ok' : ''}"></span>
    <div><strong>${escapeHtml(state.engine)}</strong><span>${crossOriginIsolated ? 'Cross-origin isolated' : 'Single-worker compatible'}</span></div>`
}

function summaryCard(label, value, detail) {
  return `<div class="summary-card"><span>${label}</span><strong>${value}</strong><small>${detail}</small></div>`
}

function projectView() {
  const recognized = recognizedCount()
  const counts = modalityCounts()
  const status = state.records.length === 0
    ? 'No files selected'
    : recognized === state.records.length
      ? `${recognized} recognized input${recognized === 1 ? '' : 's'}`
      : `${recognized} recognized · ${state.records.length - recognized} need review`

  return `
    <section class="page-heading">
      <div><span class="eyebrow">Project · Step 1</span><h2>Select the cohort data on this computer</h2>
      <p>4C Insight reads selected files locally. There is no upload step and no analysis server.</p></div>
      <div class="local-card"><strong>Private by design</strong><span>Files remain on this device. You choose when results are saved.</span></div>
    </section>
    <section class="action-panel" aria-labelledby="select-data-heading">
      <div class="action-copy"><h3 id="select-data-heading">Add project data</h3><p>Select individual files or a complete project folder. Duplicate selections are ignored.</p></div>
      <div class="button-row">
        <button id="select-files" class="primary-button">Select files</button>
        <button id="select-folder" class="secondary-button">Select folder</button>
        <input id="file-input" class="visually-hidden" type="file" multiple />
        <input id="folder-input" class="visually-hidden" type="file" multiple webkitdirectory directory />
      </div>
    </section>
    ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
    <section class="summary-grid" aria-label="Project summary">
      ${summaryCard('Selected files', String(state.records.length), status)}
      ${summaryCard('Local data', humanBytes(totalBytes()), 'Referenced from this device')}
      ${summaryCard('Detected types', String(counts.length), 'Using 4C filename conventions')}
      ${summaryCard('Processing', state.busy ? 'Checking…' : 'Ready', 'Preflight runs in a Web Worker')}
    </section>
    ${counts.length ? `<section class="card"><div class="card-heading"><div><h3>Detected data</h3><p>Review the automatic classification before you configure an analysis.</p></div></div><div class="modality-list">${counts.map(([label,count]) => `<div class="modality-pill"><span>${escapeHtml(label)}</span><strong>${count}</strong></div>`).join('')}</div></section>` : ''}
    <section class="card table-card">
      <div class="card-heading"><div><h3>Input inventory</h3><p>Nothing in this table has been uploaded. Remove any file you do not want in the project.</p></div>
      ${state.records.length ? '<div class="button-row compact"><button id="save-manifest" class="text-button">Save manifest</button><button id="clear-project" class="danger-text-button">Clear all</button></div>' : ''}</div>
      ${state.records.length === 0 ? `<div class="empty-state"><div class="empty-icon" aria-hidden="true">＋</div><strong>No project data selected</strong><span>Start with the sample metadata and any available expression, repertoire, or immune files.</span></div>` : `
      <div class="table-scroll"><table><thead><tr><th>File</th><th>Detected type</th><th>Sample</th><th>Size</th><th>Status</th><th><span class="visually-hidden">Actions</span></th></tr></thead><tbody>
      ${state.records.map((record) => `<tr><td><span class="file-name" title="${escapeHtml(record.relativePath)}">${escapeHtml(record.relativePath)}</span></td><td>${escapeHtml(record.label || 'Not classified')}</td><td>${escapeHtml(record.sampleId || '—')}</td><td>${humanBytes(record.size)}</td><td><span class="tag ${record.recognized ? 'success' : 'warning'}">${record.recognized ? 'Ready' : 'Review'}</span></td><td><button class="icon-button remove-file" data-file-id="${escapeHtml(record.id)}" aria-label="Remove ${escapeHtml(record.name)}">×</button></td></tr>`).join('')}
      </tbody></table></div>`}
    </section>
    <div class="page-actions"><div><strong>${recognized > 0 ? 'Inputs are ready for workflow configuration.' : 'Select recognized cohort inputs to continue.'}</strong><span>Unrecognized files can be resolved in a later input-mapping step.</span></div><button id="continue-qc" class="primary-button" ${recognized === 0 || state.busy ? 'disabled' : ''}>Continue to Explore & QC</button></div>`
}

function migrationView(title, step) {
  const records = recognizedCount()
  return `<section class="page-heading"><div><span class="eyebrow">${title}</span><h2>${title}</h2><p>This workflow is being moved to the shared Rust/WASM engine.</p></div></section>
  <section class="card migration-card"><span class="tag neutral">Migration ${step}</span><h3>The workflow is visible, but scientific execution is not enabled yet.</h3><p>The previous Rust implementation remains the scientific reference. 4C Insight enables this action only after the browser-safe API and native/WASM parity tests pass.</p><div class="migration-checks"><div><span class="check done">✓</span><span>Local file selection and worker boundary</span></div><div><span class="check done">✓</span><span>${records} recognized local input${records === 1 ? '' : 's'} available</span></div><div><span class="check">→</span><span>Reader/byte API extraction from native filesystem code</span></div><div><span class="check">→</span><span>Fixed-fixture native/WASM numerical parity</span></div></div></section>`
}

function resultsView() {
  return `<section class="page-heading"><div><span class="eyebrow">Results</span><h2>Results stay with the project on this computer</h2><p>Completed workflows will appear here with local HTML, table, and archive exports.</p></div></section><section class="card empty-results"><div class="empty-icon" aria-hidden="true">↧</div><h3>No WASM analysis has completed yet</h3><p>${state.records.length ? 'Your local inputs are selected. Scientific workflows will be enabled as their parity ports complete.' : 'Select project inputs first.'}</p></section>`
}

function docsView() {
  return `<section class="page-heading"><div><span class="eyebrow">Docs</span><h2>Methods and interpretation</h2><p>4C Insight keeps analysis assumptions and limitations visible next to the workflows that use them.</p></div></section><div class="docs-grid"><article class="card"><h3>Local-first execution</h3><p>Selected files remain on the user's computer. Browser workers and Rust/WebAssembly perform analysis locally. Results are saved by the user.</p></article><article class="card"><h3>Scientific parity</h3><p>The existing Rust implementation remains authoritative during migration. A browser workflow is enabled only after fixed-fixture native/WASM output parity is demonstrated.</p></article><article class="card"><h3>Longitudinal analysis</h3><p>Repeated samples can be grouped by a user-selected patient column and ordered by a user-selected observation or time column. Patient-specific clustering is distinct from Case/Control structure.</p></article><article class="card"><h3>Significant-hit structure</h3><p>PCA, dendrograms, and heatmaps restricted to significant hits are post-selection descriptions. They do not provide independent evidence of predictive performance.</p></article></div>`
}

function bindProjectEvents() {
  const fileInput = document.querySelector('#file-input')
  const folderInput = document.querySelector('#folder-input')
  document.querySelector('#select-files')?.addEventListener('click', () => fileInput.click())
  document.querySelector('#select-folder')?.addEventListener('click', () => folderInput.click())
  fileInput?.addEventListener('change', (event) => addFiles(event.currentTarget.files))
  folderInput?.addEventListener('change', (event) => addFiles(event.currentTarget.files))
  document.querySelector('#save-manifest')?.addEventListener('click', () => void exportManifest())
  document.querySelector('#clear-project')?.addEventListener('click', clearProject)
  document.querySelector('#continue-qc')?.addEventListener('click', () => { state.tab = 'qc'; render() })
  document.querySelectorAll('.remove-file').forEach((button) => button.addEventListener('click', () => removeFile(button.dataset.fileId)))
}

function render() {
  renderNav()
  const main = document.querySelector('#main-content')
  const views = {
    project: projectView(),
    qc: migrationView('Explore & QC', 'W2'),
    compare: migrationView('Compare Groups', 'W2–W3'),
    longitudinal: migrationView('Longitudinal', 'W3'),
    hits: migrationView('Significant Hits', 'W3'),
    results: resultsView(),
    docs: docsView(),
  }
  main.innerHTML = views[state.tab]
  if (state.tab === 'project') bindProjectEvents()
}

async function registerIsolationServiceWorker() {
  if (!('serviceWorker' in navigator) || !window.isSecureContext) return
  try {
    const workerUrl = new URL('./coi-serviceworker.js', window.location.href)
    await navigator.serviceWorker.register(workerUrl.href, { scope: new URL('./', workerUrl).pathname })
    await navigator.serviceWorker.ready
    if (!navigator.serviceWorker.controller && sessionStorage.getItem('4c-insight-coi-reload') !== '1') {
      sessionStorage.setItem('4c-insight-coi-reload', '1')
      window.location.reload()
    }
  } catch (error) {
    console.warn('4C Insight could not enable cross-origin isolation; single-worker mode remains available.', error)
  }
}

void registerIsolationServiceWorker()
render()
