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

const geneMetrics = [
  ['tpm', 'TPM'],
  ['raw-count', 'Raw Count'],
  ['fpkm', 'FPKM'],
  ['cpm', 'CPM'],
  ['percentile-rank', 'Percentile Rank'],
]

const transcriptMetrics = [
  ['tpm', 'TPM'],
  ['total-reads', 'Total Reads'],
  ['transcript-length', 'Transcript Length'],
  ['effective-transcript-length', 'Effective Transcript Length'],
  ['percentile-rank', 'Percentile Rank'],
]

const state = {
  tab: 'project',
  files: new Map(),
  records: [],
  busy: false,
  error: null,
  engine: 'Waiting for local inputs',
  wasmReady: false,
  metadataInspectBusy: false,
  metadataInspection: null,
  metadataInspectionFileId: null,
  metadataValueCache: new Map(),
  qcProgress: null,
  qcResult: null,
  compareProgress: null,
  compareResult: null,
  compareConfig: {
    groupColumn: '',
    caseValues: [],
    controlValues: [],
    subjectColumn: '',
    covariates: [],
    qcExclusionEnabled: false,
    qcExclusionColumn: '',
    qcExclusionValues: [],
    customExclusionEnabled: false,
    customExclusionColumn: '',
    customExclusionValues: [],
    geneEnabled: true,
    geneMergedId: '',
    transcriptEnabled: true,
    transcriptMergedId: '',
    minCount: 10,
    minFractionSamples: 0.20,
  },
  qcConfig: {
    metadataId: '',
    metadataIdColumn: '',
    geneEnabled: true,
    geneMetric: 'tpm',
    transcriptEnabled: true,
    transcriptMetric: 'tpm',
    ignoreZeros: false,
  },
}

const pending = new Map()
const worker = new Worker(new URL('./analysis-worker.js', import.meta.url), { type: 'module' })
worker.addEventListener('message', (event) => {
  const request = pending.get(event.data.requestId)
  if (!request) return
  if (event.data.type === 'analysis-progress') {
    request.onProgress?.(event.data)
    return
  }
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

function workerRequest(type, payload = {}, onProgress = null) {
  const requestId = crypto.randomUUID()
  return new Promise((resolve, reject) => {
    pending.set(requestId, { resolve, reject, onProgress })
    worker.postMessage({ type, requestId, ...payload })
  })
}

function preflightFiles(files) {
  return workerRequest('preflight', { files })
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

function formatNumber(value, digits = 3) {
  if (value === null || value === undefined || Number.isNaN(Number(value))) return '—'
  const number = Number(value)
  if (!Number.isFinite(number)) return '—'
  if (number === 0) return '0'
  if (Math.abs(number) >= 10000 || Math.abs(number) < 0.001) return number.toExponential(2)
  return number.toLocaleString(undefined, { maximumFractionDigits: digits })
}

function percent(part, total) {
  if (!total) return '—'
  return `${((part / total) * 100).toFixed(1)}%`
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

function recordsOfKind(kind) {
  return state.records.filter((record) => record.kind === kind)
}

function selectedFile(id) {
  return id ? state.files.get(id) || null : null
}

function resetQcStateAfterInputsChange() {
  const metadata = recordsOfKind('metadata-candidate')
  if (!metadata.some((record) => record.id === state.qcConfig.metadataId)) {
    state.qcConfig.metadataId = metadata[0]?.id || ''
    state.qcConfig.metadataIdColumn = ''
  }
  state.metadataInspection = null
  state.metadataInspectionFileId = null
  state.metadataValueCache.clear()
  state.qcResult = null
  state.qcProgress = null
  state.compareResult = null
  state.compareProgress = null

  const geneMerged = recordsOfKind('merged-gene-expression')
  const transcriptMerged = recordsOfKind('merged-transcript-expression')
  if (!geneMerged.some((record) => record.id === state.compareConfig.geneMergedId)) {
    state.compareConfig.geneMergedId = geneMerged[0]?.id || ''
  }
  if (!transcriptMerged.some((record) => record.id === state.compareConfig.transcriptMergedId)) {
    state.compareConfig.transcriptMergedId = transcriptMerged[0]?.id || ''
  }
  state.compareConfig.groupColumn = ''
  state.compareConfig.caseValues = []
  state.compareConfig.controlValues = []
  state.compareConfig.subjectColumn = ''
  state.compareConfig.covariates = []
  state.compareConfig.qcExclusionEnabled = false
  state.compareConfig.qcExclusionColumn = ''
  state.compareConfig.qcExclusionValues = []
  state.compareConfig.customExclusionEnabled = false
  state.compareConfig.customExclusionColumn = ''
  state.compareConfig.customExclusionValues = []
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
    resetQcStateAfterInputsChange()
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
  state.metadataInspection = null
  state.metadataInspectionFileId = null
  state.metadataValueCache.clear()
  state.qcResult = null
  state.qcProgress = null
  state.compareResult = null
  state.compareProgress = null
  state.qcConfig.metadataId = ''
  state.qcConfig.metadataIdColumn = ''
  state.compareConfig.groupColumn = ''
  state.compareConfig.caseValues = []
  state.compareConfig.controlValues = []
  state.compareConfig.subjectColumn = ''
  state.compareConfig.covariates = []
  state.compareConfig.geneMergedId = ''
  state.compareConfig.transcriptMergedId = ''
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

async function exportQcResult() {
  if (!state.qcResult) return
  const payload = {
    product: '4C Insight',
    schemaVersion: 1,
    workflow: 'explore-qc-expression',
    config: state.qcConfig,
    result: state.qcResult,
  }
  await saveTextFile('4c-insight-explore-qc.json', `${JSON.stringify(payload, null, 2)}\n`)
}

async function exportCompareResult() {
  if (!state.compareResult) return
  const payload = {
    product: '4C Insight',
    schemaVersion: 1,
    workflow: 'compare-groups-expression',
    config: {
      metadataIdColumn: state.qcConfig.metadataIdColumn,
      ...state.compareConfig,
    },
    result: state.compareResult,
  }
  await saveTextFile('4c-insight-compare-groups-expression.json', `${JSON.stringify(payload, null, 2)}\n`)
}

function selectedValues(select) {
  return Array.from(select?.selectedOptions || []).map((option) => option.value)
}

function metadataValueKey(column) {
  return `${state.qcConfig.metadataId}::${column}`
}

function cachedMetadataValues(column) {
  if (!column) return []
  return state.metadataValueCache.get(metadataValueKey(column)) || []
}

async function loadMetadataValues(column) {
  const file = selectedFile(state.qcConfig.metadataId)
  if (!file || !column || !state.wasmReady) return []
  const key = metadataValueKey(column)
  if (state.metadataValueCache.has(key)) return state.metadataValueCache.get(key)
  const response = await workerRequest('metadata-column-values', { file, column })
  state.metadataValueCache.set(key, response.result || [])
  return response.result || []
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

function metricOptions(options, selected) {
  return options.map(([value, label]) => `<option value="${value}" ${selected === value ? 'selected' : ''}>${label}</option>`).join('')
}

function metadataConfiguration(metadataRecords) {
  if (!metadataRecords.length) {
    return `<div class="config-block"><div><strong>Sample metadata</strong><span>No metadata candidate was detected. Expression QC can still run, but cohort mapping is not available yet.</span></div><span class="tag warning">Optional for QC</span></div>`
  }
  const inspection = state.metadataInspection
  return `<div class="config-block stacked">
    <div class="config-heading"><div><strong>Sample metadata</strong><span>Select the table that defines sample and patient annotations.</span></div><span class="tag success">Local</span></div>
    <div class="form-grid">
      <label><span>Metadata file</span><select id="metadata-file">${metadataRecords.map((record) => `<option value="${escapeHtml(record.id)}" ${state.qcConfig.metadataId === record.id ? 'selected' : ''}>${escapeHtml(record.name)}</option>`).join('')}</select></label>
      <label><span>Sample ID column</span><select id="metadata-id-column" ${!inspection || state.metadataInspectBusy ? 'disabled' : ''}>${inspection ? inspection.columns.map((column) => `<option value="${escapeHtml(column)}" ${state.qcConfig.metadataIdColumn === column ? 'selected' : ''}>${escapeHtml(column)}</option>`).join('') : '<option>Reading metadata…</option>'}</select></label>
    </div>
  </div>`
}

function expressionConfiguration(kind, title, records, enabled, metric, options) {
  return `<div class="config-block stacked ${records.length ? '' : 'muted-block'}">
    <div class="config-heading"><label class="check-label"><input class="modality-toggle" data-kind="${kind}" type="checkbox" ${enabled && records.length ? 'checked' : ''} ${records.length ? '' : 'disabled'} /><span><strong>${title}</strong><small>${records.length} local workbook${records.length === 1 ? '' : 's'}</small></span></label><span class="tag ${records.length ? 'success' : 'neutral'}">${records.length ? 'Available' : 'Not selected'}</span></div>
    <label><span>QC measurement</span><select class="metric-select" data-kind="${kind}" ${enabled && records.length ? '' : 'disabled'}>${metricOptions(options, metric)}</select></label>
  </div>`
}

function metadataSummaryView() {
  if (state.metadataInspectBusy) {
    return `<section class="card"><div class="inline-status"><span class="spinner" aria-hidden="true"></span><div><strong>Reading metadata locally</strong><span>The selected file is being parsed in the Rust/WASM worker.</span></div></div></section>`
  }
  const metadata = state.metadataInspection
  if (!metadata) return ''
  const hasIssues = metadata.blank_id_rows > 0 || metadata.duplicate_id_values.length > 0
  return `<section class="card">
    <div class="card-heading"><div><h3>Metadata check</h3><p>${escapeHtml(metadata.file_name)} · ID column: <strong>${escapeHtml(metadata.id_column)}</strong></p></div><span class="tag ${hasIssues ? 'warning' : 'success'}">${hasIssues ? 'Review IDs' : 'IDs ready'}</span></div>
    <div class="summary-grid compact-grid">
      ${summaryCard('Rows', metadata.rows, 'Non-empty metadata records')}
      ${summaryCard('Columns', metadata.columns.length, 'Available for later workflows')}
      ${summaryCard('Blank IDs', metadata.blank_id_rows, 'Rows without an ID value')}
      ${summaryCard('Duplicate IDs', metadata.duplicate_id_values.length, metadata.duplicate_id_values.length ? metadata.duplicate_id_values.slice(0, 3).join(', ') : 'No duplicate values')}
    </div>
    <details class="details-block"><summary>Preview metadata</summary><div class="table-scroll metadata-preview"><table><thead><tr>${metadata.columns.map((column) => `<th>${escapeHtml(column)}</th>`).join('')}</tr></thead><tbody>${metadata.preview_rows.map((row) => `<tr>${metadata.columns.map((_, index) => `<td>${escapeHtml(row[index] || '')}</td>`).join('')}</tr>`).join('')}</tbody></table></div></details>
  </section>`
}

function expressionResultSection(title, rows) {
  if (!rows?.length) return ''
  const totalRows = rows.reduce((sum, row) => sum + row.n_total, 0)
  const totalZero = rows.reduce((sum, row) => sum + row.n_zero, 0)
  const totalMissing = rows.reduce((sum, row) => sum + row.n_missing, 0)
  const medians = rows.map((row) => row.median).filter((value) => value !== null && value !== undefined).sort((a, b) => a - b)
  const medianOfMedians = medians.length ? medians[Math.floor((medians.length - 1) / 2)] : null
  return `<section class="card table-card">
    <div class="card-heading"><div><h3>${title}</h3><p>${rows.length} samples · ${escapeHtml(rows[0].metric.replaceAll('-', ' '))} · sample workbooks processed one at a time</p></div><span class="tag success">Rust/WASM</span></div>
    <div class="summary-grid compact-grid">
      ${summaryCard('Samples', rows.length, 'Successfully parsed workbooks')}
      ${summaryCard('Typical median', formatNumber(medianOfMedians), 'Median of sample medians')}
      ${summaryCard('Zero values', percent(totalZero, totalRows), `${totalZero.toLocaleString()} of ${totalRows.toLocaleString()} rows`)}
      ${summaryCard('Missing values', percent(totalMissing, totalRows), `${totalMissing.toLocaleString()} of ${totalRows.toLocaleString()} rows`)}
    </div>
    <div class="table-scroll qc-table"><table><thead><tr><th>Sample</th><th>Rows</th><th>Mean</th><th>Median</th><th>Q1</th><th>Q3</th><th>Zeros</th><th>Missing</th></tr></thead><tbody>
      ${rows.slice().sort((a,b) => a.sample.localeCompare(b.sample)).map((row) => `<tr><td><strong>${escapeHtml(row.sample)}</strong></td><td>${row.n_total.toLocaleString()}</td><td>${formatNumber(row.mean)}</td><td>${formatNumber(row.median)}</td><td>${formatNumber(row.q1)}</td><td>${formatNumber(row.q3)}</td><td>${percent(row.n_zero, row.n_total)}</td><td>${percent(row.n_missing, row.n_total)}</td></tr>`).join('')}
    </tbody></table></div>
  </section>`
}

function qcView() {
  const metadataRecords = recordsOfKind('metadata-candidate')
  const geneRecords = recordsOfKind('gene-expression')
  const transcriptRecords = recordsOfKind('transcript-expression')
  const runnable = state.wasmReady && !state.busy && !state.metadataInspectBusy && (
    (state.qcConfig.geneEnabled && geneRecords.length > 0) ||
    (state.qcConfig.transcriptEnabled && transcriptRecords.length > 0) ||
    Boolean(state.qcConfig.metadataId)
  )

  return `<section class="page-heading"><div><span class="eyebrow">Explore & QC · W2</span><h2>Inspect metadata and expression quality</h2><p>4C Insight reads each selected workbook inside the local Web Worker, runs the parser and distribution calculations in Rust/WASM, and releases the source buffer before moving to the next sample.</p></div><div class="local-card"><strong>No upload</strong><span>Only local file handles and local WASM memory are used during this workflow.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  ${!state.wasmReady ? `<div class="notice-banner" role="status"><strong>Rust/WASM is not loaded.</strong><span>W2 scientific execution is disabled until the WASM package exists in <code>app/wasm/</code>. Filename preflight can still use the JavaScript fallback.</span></div>` : ''}
  <section class="card"><div class="card-heading"><div><h3>Configure local QC</h3><p>Use the metadata roles that match this cohort. Expression QC is independent of Case/Control grouping.</p></div><span class="tag neutral">W2</span></div>
    <div class="config-grid">
      ${metadataConfiguration(metadataRecords)}
      ${expressionConfiguration('gene', 'Gene expression', geneRecords, state.qcConfig.geneEnabled, state.qcConfig.geneMetric, geneMetrics)}
      ${expressionConfiguration('transcript', 'Transcript expression', transcriptRecords, state.qcConfig.transcriptEnabled, state.qcConfig.transcriptMetric, transcriptMetrics)}
    </div>
    <label class="check-label standalone-check"><input id="ignore-zeros" type="checkbox" ${state.qcConfig.ignoreZeros ? 'checked' : ''} /><span><strong>Ignore zero values in distribution statistics</strong><small>Zero counts are still reported. They are omitted only from mean, quartile, and whisker calculations.</small></span></label>
    <div class="run-row"><div><strong>${geneRecords.length + transcriptRecords.length} expression workbook${geneRecords.length + transcriptRecords.length === 1 ? '' : 's'} available</strong><span>Processing is sequential and bounded to one source workbook at a time.</span></div><button id="run-expression-qc" class="primary-button" ${runnable ? '' : 'disabled'}>${state.busy ? 'Running…' : 'Run local QC'}</button></div>
  </section>
  ${state.qcProgress ? `<div class="progress-card" role="status"><div class="progress-copy"><strong>${escapeHtml(state.qcProgress.message)}</strong><span>${state.qcProgress.completed} of ${state.qcProgress.total} steps complete</span></div><progress max="${Math.max(1, state.qcProgress.total)}" value="${state.qcProgress.completed}"></progress></div>` : ''}
  ${metadataSummaryView()}
  ${state.qcResult ? `<div class="result-heading"><div><span class="eyebrow">Local QC result</span><h3>Expression distributions</h3><p>These summaries are descriptive QC views. They do not perform group testing or outlier exclusion.</p></div><button id="export-qc" class="secondary-button">Save QC JSON</button></div>${expressionResultSection('Gene expression QC', state.qcResult.gene)}${expressionResultSection('Transcript expression QC', state.qcResult.transcript)}` : ''}`
}


function ensureCompareDefaults() {
  const columns = state.metadataInspection?.columns || []
  const cfg = state.compareConfig
  if (columns.length) {
    if (!columns.includes(cfg.groupColumn)) {
      cfg.groupColumn = ['analysis_group', 'Phenotype', 'diagnosis', 'cohort']
        .find((column) => columns.includes(column) && column !== state.qcConfig.metadataIdColumn)
        || columns.find((column) => column !== state.qcConfig.metadataIdColumn)
        || ''
      cfg.caseValues = []
      cfg.controlValues = []
    }
    if (cfg.subjectColumn && !columns.includes(cfg.subjectColumn)) cfg.subjectColumn = ''
    cfg.covariates = cfg.covariates.filter((column) => columns.includes(column))
    if (!columns.includes(cfg.qcExclusionColumn)) {
      cfg.qcExclusionColumn = ['qc_review_status', 'qc_status', 'review_status']
        .find((column) => columns.includes(column)) || ''
      cfg.qcExclusionValues = []
    }
    if (cfg.customExclusionColumn && !columns.includes(cfg.customExclusionColumn)) {
      cfg.customExclusionColumn = ''
      cfg.customExclusionValues = []
    }
  }

  const geneMerged = recordsOfKind('merged-gene-expression')
  const transcriptMerged = recordsOfKind('merged-transcript-expression')
  if (!geneMerged.some((record) => record.id === cfg.geneMergedId)) cfg.geneMergedId = geneMerged[0]?.id || ''
  if (!transcriptMerged.some((record) => record.id === cfg.transcriptMergedId)) cfg.transcriptMergedId = transcriptMerged[0]?.id || ''
}

function metadataValueOptions(values, selected) {
  const selectedSet = new Set(selected)
  return values.map((entry) => `<option value="${escapeHtml(entry.value)}" ${selectedSet.has(entry.value) ? 'selected' : ''}>${escapeHtml(entry.value)} (${entry.count})</option>`).join('')
}

function columnOptions(columns, selected, includeNone = false) {
  return `${includeNone ? '<option value="">None</option>' : ''}${columns.map((column) => `<option value="${escapeHtml(column)}" ${selected === column ? 'selected' : ''}>${escapeHtml(column)}</option>`).join('')}`
}

function mergedFileOptions(records, selected) {
  if (!records.length) return '<option value="">No merged table selected</option>'
  return records.map((record) => `<option value="${escapeHtml(record.id)}" ${selected === record.id ? 'selected' : ''}>${escapeHtml(record.relativePath || record.name)}</option>`).join('')
}

function exclusionRuleView(prefix, title, enabled, column, selectedValues, columns, values, help) {
  return `<div class="config-block stacked">
    <div class="config-heading"><label class="check-label"><input id="${prefix}-enabled" type="checkbox" ${enabled ? 'checked' : ''}/><span><strong>${title}</strong><small>${help}</small></span></label><span class="tag ${enabled ? 'warning' : 'neutral'}">${enabled ? 'Active' : 'Optional'}</span></div>
    <label><span>Metadata column</span><select id="${prefix}-column" ${enabled ? '' : 'disabled'}>${columnOptions(columns, column, true)}</select></label>
    <label><span>Exclude rows with these values</span><select id="${prefix}-values" class="multi-select" multiple size="${Math.min(6, Math.max(3, values.length || 3))}" ${enabled && column ? '' : 'disabled'}>${values.length ? metadataValueOptions(values, selectedValues) : '<option disabled>Choose a column to load values</option>'}</select></label>
  </div>`
}

function pcaSvg(pca) {
  const points = pca?.points || []
  if (!points.length) return '<div class="plot-empty">PCA is not available for this result.</div>'
  const width = 760
  const height = 360
  const pad = 46
  const xs = points.map((point) => Number(point.x)).filter(Number.isFinite)
  const ys = points.map((point) => Number(point.y)).filter(Number.isFinite)
  if (!xs.length || !ys.length) return '<div class="plot-empty">PCA coordinates are not finite.</div>'
  let minX = Math.min(...xs); let maxX = Math.max(...xs)
  let minY = Math.min(...ys); let maxY = Math.max(...ys)
  if (minX === maxX) { minX -= 1; maxX += 1 }
  if (minY === maxY) { minY -= 1; maxY += 1 }
  const xPad = (maxX - minX) * 0.08; const yPad = (maxY - minY) * 0.08
  minX -= xPad; maxX += xPad; minY -= yPad; maxY += yPad
  const sx = (x) => pad + ((x - minX) / (maxX - minX)) * (width - pad * 2)
  const sy = (y) => height - pad - ((y - minY) / (maxY - minY)) * (height - pad * 2)
  const xZero = minX <= 0 && maxX >= 0 ? sx(0) : pad
  const yZero = minY <= 0 && maxY >= 0 ? sy(0) : height - pad
  const circles = points.map((point) => {
    const klass = String(point.group).toLowerCase() === 'case' ? 'case-point' : 'control-point'
    return `<circle class="${klass}" cx="${sx(Number(point.x)).toFixed(2)}" cy="${sy(Number(point.y)).toFixed(2)}" r="4.2"><title>${escapeHtml(point.sample_id)} · ${escapeHtml(point.group)} · subject ${escapeHtml(point.subject)} · ${escapeHtml(pca.x_label)} ${formatNumber(point.x)} · ${escapeHtml(pca.y_label)} ${formatNumber(point.y)}</title></circle>`
  }).join('')
  return `<div class="plot-scroll"><svg class="science-plot" viewBox="0 0 ${width} ${height}" role="img" aria-label="${escapeHtml(pca.name)}">
    <line class="plot-axis" x1="${pad}" y1="${yZero}" x2="${width-pad}" y2="${yZero}"/><line class="plot-axis" x1="${xZero}" y1="${pad}" x2="${xZero}" y2="${height-pad}"/>
    ${circles}
    <text class="plot-label" x="${width/2}" y="${height-10}" text-anchor="middle">${escapeHtml(pca.x_label)}</text>
    <text class="plot-label" x="14" y="${height/2}" text-anchor="middle" transform="rotate(-90 14 ${height/2})">${escapeHtml(pca.y_label)}</text>
  </svg></div><div class="plot-legend"><span><i class="legend-dot case-point"></i>Case</span><span><i class="legend-dot control-point"></i>Control</span></div>`
}

function dendrogramSvg(tree) {
  const leaves = tree?.leaves || []
  const segments = tree?.segments || []
  if (!leaves.length || !segments.length) return '<div class="plot-empty">Hierarchical clustering is not available for this result.</div>'
  const width = 860
  const height = Math.max(300, Math.min(1400, 90 + leaves.length * 4.2))
  const left = 118; const right = 28; const top = 28; const bottom = 28
  const maxDistance = Math.max(1e-12, ...segments.flatMap((segment) => [Number(segment.x1), Number(segment.x2)]).filter(Number.isFinite))
  const maxPosition = Math.max(1, ...leaves.map((leaf) => Number(leaf.position)).filter(Number.isFinite))
  const sx = (x) => left + (Number(x) / maxDistance) * (width - left - right)
  const sy = (y) => top + (Number(y) / maxPosition) * (height - top - bottom)
  const lines = segments.map((segment) => `<line class="dendro-line" x1="${sx(segment.x1).toFixed(2)}" y1="${sy(segment.y1).toFixed(2)}" x2="${sx(segment.x2).toFixed(2)}" y2="${sy(segment.y2).toFixed(2)}"/>`).join('')
  const labelEvery = Math.max(1, Math.ceil(leaves.length / 55))
  const leafMarks = leaves.map((leaf, index) => {
    const klass = String(leaf.group).toLowerCase() === 'case' ? 'case-point' : 'control-point'
    const y = sy(leaf.position)
    const label = index % labelEvery === 0 ? `<text class="dendro-label" x="${left-10}" y="${(y+3).toFixed(2)}" text-anchor="end">${escapeHtml(leaf.sample_id)}</text>` : ''
    return `${label}<circle class="${klass}" cx="${left}" cy="${y.toFixed(2)}" r="2.7"><title>${escapeHtml(leaf.sample_id)} · ${escapeHtml(leaf.group)} · subject ${escapeHtml(leaf.subject)}</title></circle>`
  }).join('')
  return `<div class="plot-scroll dendro-scroll"><svg class="science-plot dendrogram" viewBox="0 0 ${width} ${height}" role="img" aria-label="${escapeHtml(tree.name)}">${lines}${leafMarks}<text class="plot-label" x="${(left+width-right)/2}" y="${height-6}" text-anchor="middle">${escapeHtml(tree.distance_label)} · ${escapeHtml(tree.linkage)}</text></svg></div>`
}

function differentialTable(analysis) {
  const rows = analysis?.tests || []
  if (!rows.length) return '<div class="plot-empty">No tested features are available.</div>'
  return `<div class="table-scroll differential-table"><table><thead><tr><th>Feature</th><th>Secondary ID</th><th>Case mean</th><th>Control mean</th><th>log2 group effect</th><th>SE</th><th>q-value</th><th>Subject effect</th><th>Subject q</th></tr></thead><tbody>${rows.slice(0, 250).map((row) => `<tr><td><strong>${escapeHtml(row.feature)}</strong></td><td>${escapeHtml(row.secondary_id || '')}</td><td>${formatNumber(row.mean_case)}</td><td>${formatNumber(row.mean_control)}</td><td>${formatNumber(row.beta_group)}</td><td>${formatNumber(row.robust_se)}</td><td>${formatNumber(row.q_value)}</td><td>${formatNumber(row.subject_effect)}</td><td>${formatNumber(row.subject_q_value)}</td></tr>`).join('')}</tbody></table></div><p class="table-note">Showing the first ${Math.min(250, rows.length).toLocaleString()} of ${rows.length.toLocaleString()} tested features, ordered by adjusted q-value and effect size. The JSON export contains all rows.</p>`
}

function comparisonResultSection(title, result) {
  if (!result?.analysis) return ''
  const analysis = result.analysis
  const significant = (analysis.tests || []).filter((row) => Number.isFinite(Number(row.q_value)) && Number(row.q_value) <= 0.05).length
  return `<section class="card comparison-result">
    <div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>${escapeHtml(result.source_metric)} · corrected native/WASM-parity expression model</p></div><span class="tag success">Rust/WASM</span></div>
    <div class="summary-grid compact-grid">
      ${summaryCard('Model samples', analysis.n_model_samples, `${analysis.n_subjects} subject/patient IDs`)}
      ${summaryCard('Tested features', analysis.tested_features.toLocaleString(), `${analysis.filtered_features.toLocaleString()} filtered before modeling`)}
      ${summaryCard('q ≤ 0.05', significant.toLocaleString(), 'BH-adjusted feature tests')}
      ${summaryCard('Excluded rows', result.comparison.excluded_rows.toLocaleString(), `${result.comparison.metadata_rows_after_exclusions} metadata rows retained`)}
    </div>
    ${analysis.warnings?.length ? `<div class="analysis-warnings">${analysis.warnings.map((warning) => `<div>${escapeHtml(warning)}</div>`).join('')}</div>` : ''}
    <div class="plot-grid"><article class="plot-card"><div class="plot-heading"><strong>Global expression PCA</strong><span>Top variable normalized features; groups do not select the PCA features.</span></div>${pcaSvg(analysis.pca)}</article><article class="plot-card"><div class="plot-heading"><strong>Hierarchical clustering</strong><span>Average linkage on Euclidean distance across the same normalized feature space.</span></div>${dendrogramSvg(analysis.dendrogram)}</article></div>
    <details class="details-block result-details" open><summary>Differential feature table</summary>${differentialTable(analysis)}</details>
    ${analysis.dtu ? `<details class="details-block result-details"><summary>Transcript-usage screen (${analysis.dtu.genes_tested.toLocaleString()} genes)</summary><p class="method-note">${escapeHtml(analysis.dtu.method_description)}</p><div class="table-scroll"><table><thead><tr><th>Gene</th><th>Transcripts</th><th>q-value</th><th>Max |CLR effect|</th></tr></thead><tbody>${analysis.dtu.gene_tests.slice(0, 100).map((row) => `<tr><td><strong>${escapeHtml(row.gene_id)}</strong></td><td>${row.n_transcripts}</td><td>${formatNumber(row.q_value)}</td><td>${formatNumber(row.max_abs_clr_effect)}</td></tr>`).join('')}</tbody></table></div></details>` : ''}
  </section>`
}

function compareView() {
  ensureCompareDefaults()
  const metadataRecords = recordsOfKind('metadata-candidate')
  const geneMerged = recordsOfKind('merged-gene-expression')
  const transcriptMerged = recordsOfKind('merged-transcript-expression')
  const metadata = state.metadataInspection
  const columns = metadata?.columns || []
  const cfg = state.compareConfig
  const groupValues = cachedMetadataValues(cfg.groupColumn)
  const qcValues = cachedMetadataValues(cfg.qcExclusionColumn)
  const customValues = cachedMetadataValues(cfg.customExclusionColumn)
  const covariateColumns = columns.filter((column) => column !== state.qcConfig.metadataIdColumn && column !== cfg.groupColumn && column !== cfg.subjectColumn)
  const overlap = cfg.caseValues.some((value) => cfg.controlValues.some((control) => control.toLowerCase() === value.toLowerCase()))
  const metadataReady = Boolean(metadata && !metadata.blank_id_rows && !metadata.duplicate_id_values.length)
  const modalityReady = (cfg.geneEnabled && cfg.geneMergedId) || (cfg.transcriptEnabled && cfg.transcriptMergedId)
  const exclusionReady = (!cfg.qcExclusionEnabled || (Boolean(cfg.qcExclusionColumn) && cfg.qcExclusionValues.length > 0)) && (!cfg.customExclusionEnabled || (Boolean(cfg.customExclusionColumn) && cfg.customExclusionValues.length > 0))
  const runnable = state.wasmReady && !state.busy && metadataReady && Boolean(cfg.groupColumn) && cfg.caseValues.length > 0 && cfg.controlValues.length > 0 && !overlap && modalityReady && exclusionReady

  return `<section class="page-heading"><div><span class="eyebrow">Compare Groups · W3</span><h2>Define a cohort comparison from metadata</h2><p>Select the metadata roles and group values that match this cohort. 4C Insight runs the established corrected count model, PCA, and hierarchical clustering inside the local Rust/WASM worker.</p></div><div class="local-card"><strong>Same science, local bytes</strong><span>The native and byte/WASM paths share the corrected-expression implementation.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  ${!metadataRecords.length ? '<div class="notice-banner"><strong>Metadata required.</strong><span>Select a metadata file in the Project tab before defining groups.</span></div>' : ''}
  ${metadata && (!metadataReady) ? `<div class="notice-banner"><strong>Resolve metadata IDs first.</strong><span>The selected ID column has ${metadata.blank_id_rows} blank row(s) and ${metadata.duplicate_id_values.length} duplicate value(s).</span></div>` : ''}
  ${overlap ? '<div class="notice-banner"><strong>Case and control selections overlap.</strong><span>Remove the same value from one group before running the comparison.</span></div>' : ''}
  <section class="card"><div class="card-heading"><div><h3>1. Metadata and groups</h3><p>The sample-ID column maps expression columns. The patient/subject column is independent and optional.</p></div><span class="tag neutral">W3</span></div>
    <div class="config-grid compare-grid">
      <div class="config-block stacked"><div class="config-heading"><div><strong>Metadata roles</strong><span>Choose columns by meaning, not by hardcoded names.</span></div><span class="tag success">Flexible</span></div>
        <label><span>Metadata file</span><select id="compare-metadata-file">${metadataRecords.map((record) => `<option value="${escapeHtml(record.id)}" ${state.qcConfig.metadataId === record.id ? 'selected' : ''}>${escapeHtml(record.name)}</option>`).join('')}</select></label>
        <label><span>Expression/sample ID column</span><select id="compare-id-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns, state.qcConfig.metadataIdColumn) : '<option>Read metadata first</option>'}</select></label>
        <label><span>Patient/subject column for repeated measures</span><select id="compare-subject-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns.filter((column) => column !== state.qcConfig.metadataIdColumn), cfg.subjectColumn, true) : '<option>Read metadata first</option>'}</select></label>
      </div>
      <div class="config-block stacked"><div class="config-heading"><div><strong>Case group</strong><span>Select one or more observed values.</span></div><span class="tag neutral">Outcome</span></div>
        <label><span>Grouping column</span><select id="compare-group-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns.filter((column) => column !== state.qcConfig.metadataIdColumn), cfg.groupColumn) : '<option>Read metadata first</option>'}</select></label>
        <label><span>Case values</span><select id="compare-case-values" class="multi-select" multiple size="${Math.min(8, Math.max(4, groupValues.length || 4))}" ${groupValues.length ? '' : 'disabled'}>${groupValues.length ? metadataValueOptions(groupValues, cfg.caseValues) : '<option disabled>Loading observed values…</option>'}</select></label>
      </div>
      <div class="config-block stacked"><div class="config-heading"><div><strong>Control group</strong><span>Case and control values must not overlap.</span></div><span class="tag neutral">Reference</span></div>
        <label><span>Control values</span><select id="compare-control-values" class="multi-select" multiple size="${Math.min(8, Math.max(4, groupValues.length || 4))}" ${groupValues.length ? '' : 'disabled'}>${groupValues.length ? metadataValueOptions(groupValues, cfg.controlValues) : '<option disabled>Loading observed values…</option>'}</select></label>
        <label><span>Adjustment covariates</span><select id="compare-covariates" class="multi-select" multiple size="4" ${metadata ? '' : 'disabled'}>${covariateColumns.map((column) => `<option value="${escapeHtml(column)}" ${cfg.covariates.includes(column) ? 'selected' : ''}>${escapeHtml(column)}</option>`).join('')}</select></label>
      </div>
    </div>
  </section>
  <section class="card"><div class="card-heading"><div><h3>2. Optional sample exclusions</h3><p>Exclusion happens before group construction and model fitting. Rules are recorded in the exported result.</p></div></div><div class="config-grid exclusion-grid">
    ${exclusionRuleView('compare-qc-exclusion', 'Reviewed QC exclusion', cfg.qcExclusionEnabled, cfg.qcExclusionColumn, cfg.qcExclusionValues, columns, qcValues, 'For example: qc_review_status = Failed. This is independent of automatic outlier analysis.')}
    ${exclusionRuleView('compare-custom-exclusion', 'Custom metadata exclusion', cfg.customExclusionEnabled, cfg.customExclusionColumn, cfg.customExclusionValues, columns, customValues, 'Use any metadata column/value rule for user-defined outliers or exclusions.')}
  </div></section>
  <section class="card"><div class="card-heading"><div><h3>3. Expression analyses</h3><p>Corrected analyses use raw counts for genes and Total Reads for transcripts. Merged tables are read locally from the selected files.</p></div></div><div class="config-grid expression-source-grid">
    <div class="config-block stacked ${geneMerged.length ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="compare-gene-enabled" type="checkbox" ${cfg.geneEnabled && geneMerged.length ? 'checked' : ''} ${geneMerged.length ? '' : 'disabled'}/><span><strong>Gene expression</strong><small>NB2 corrected differential analysis + PCA + dendrogram</small></span></label><span class="tag ${geneMerged.length ? 'success' : 'neutral'}">${geneMerged.length ? `${geneMerged.length} merged table${geneMerged.length === 1 ? '' : 's'}` : 'Unavailable'}</span></div><label><span>Merged gene table</span><select id="compare-gene-file" ${cfg.geneEnabled && geneMerged.length ? '' : 'disabled'}>${mergedFileOptions(geneMerged, cfg.geneMergedId)}</select></label></div>
    <div class="config-block stacked ${transcriptMerged.length ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="compare-transcript-enabled" type="checkbox" ${cfg.transcriptEnabled && transcriptMerged.length ? 'checked' : ''} ${transcriptMerged.length ? '' : 'disabled'}/><span><strong>Transcript expression</strong><small>NB2 corrected abundance + PCA + dendrogram + DTU screen</small></span></label><span class="tag ${transcriptMerged.length ? 'success' : 'neutral'}">${transcriptMerged.length ? `${transcriptMerged.length} merged table${transcriptMerged.length === 1 ? '' : 's'}` : 'Unavailable'}</span></div><label><span>Merged transcript table</span><select id="compare-transcript-file" ${cfg.transcriptEnabled && transcriptMerged.length ? '' : 'disabled'}>${mergedFileOptions(transcriptMerged, cfg.transcriptMergedId)}</select></label></div>
  </div>
  <details class="advanced-settings"><summary>Advanced model settings</summary><div class="form-grid advanced-grid"><label><span>Minimum count</span><input id="compare-min-count" type="number" min="0" step="1" value="${cfg.minCount}"/></label><label><span>Minimum fraction of samples</span><input id="compare-min-fraction" type="number" min="0" max="1" step="0.01" value="${cfg.minFractionSamples}"/></label></div><p>Defaults match the established corrected-expression analysis. Change them only when the analysis plan requires it.</p></details>
  <div class="run-row"><div><strong>${cfg.caseValues.length} case value${cfg.caseValues.length === 1 ? '' : 's'} · ${cfg.controlValues.length} control value${cfg.controlValues.length === 1 ? '' : 's'}</strong><span>Group membership, exclusions, subject mapping, and model settings are frozen into the result.</span></div><button id="run-group-comparison" class="primary-button" ${runnable ? '' : 'disabled'}>${state.busy ? 'Running…' : 'Run local comparison'}</button></div></section>
  ${state.compareProgress ? `<div class="progress-card" role="status"><div class="progress-copy"><strong>${escapeHtml(state.compareProgress.message)}</strong><span>${state.compareProgress.completed} of ${state.compareProgress.total} modality steps complete</span></div><progress max="${Math.max(1, state.compareProgress.total)}" value="${state.compareProgress.completed}"></progress></div>` : ''}
  ${state.compareResult ? `<div class="result-heading"><div><span class="eyebrow">Corrected expression result</span><h3>${escapeHtml(state.compareResult.comparisonName)}</h3><p>Generated entirely on this device. PCA and clustering use global top-variable normalized features, not significant-hit selection.</p></div><button id="export-comparison" class="secondary-button">Save comparison JSON</button></div>${comparisonResultSection('Gene expression', state.compareResult.gene)}${comparisonResultSection('Transcript expression', state.compareResult.transcript)}` : ''}`
}

function migrationView(title, step) {
  const records = recognizedCount()
  return `<section class="page-heading"><div><span class="eyebrow">${title}</span><h2>${title}</h2><p>This workflow is being moved to the shared Rust/WASM engine.</p></div></section>
  <section class="card migration-card"><span class="tag neutral">Migration ${step}</span><h3>The workflow is visible, but scientific execution is not enabled yet.</h3><p>The existing Rust implementation remains the scientific reference. 4C Insight enables this action only after the browser-safe API and native/WASM parity tests pass.</p><div class="migration-checks"><div><span class="check done">✓</span><span>Local file selection and worker boundary</span></div><div><span class="check done">✓</span><span>Metadata and expression byte readers</span></div><div><span class="check done">✓</span><span>${records} recognized local input${records === 1 ? '' : 's'} available</span></div><div><span class="check">→</span><span>Workflow-specific statistical parity</span></div></div></section>`
}

function resultsView() {
  const cards = []
  if (state.qcResult) {
    cards.push(`<section class="card result-card"><div><span class="tag success">Completed</span><h3>Explore & QC · expression distributions</h3><p>${state.qcResult.gene.length} gene-expression samples and ${state.qcResult.transcript.length} transcript-expression samples processed with Rust/WASM.</p></div><div class="button-row"><button id="results-open-qc" class="secondary-button">View QC</button><button id="results-export-qc" class="primary-button">Save QC JSON</button></div></section>`)
  }
  if (state.compareResult) {
    const geneSamples = state.compareResult.gene?.analysis?.n_model_samples || 0
    const transcriptSamples = state.compareResult.transcript?.analysis?.n_model_samples || 0
    cards.push(`<section class="card result-card"><div><span class="tag success">Completed</span><h3>Compare Groups · corrected expression</h3><p>${escapeHtml(state.compareResult.comparisonName)} · ${geneSamples ? `${geneSamples} gene model samples` : 'gene disabled'} · ${transcriptSamples ? `${transcriptSamples} transcript model samples` : 'transcript disabled'}.</p></div><div class="button-row"><button id="results-open-compare" class="secondary-button">View comparison</button><button id="results-export-compare" class="primary-button">Save comparison JSON</button></div></section>`)
  }
  if (!cards.length) {
    return `<section class="page-heading"><div><span class="eyebrow">Results</span><h2>Results stay with the project on this computer</h2><p>Completed workflows appear here with local exports.</p></div></section><section class="card empty-results"><div class="empty-icon" aria-hidden="true">↧</div><h3>No WASM analysis has completed yet</h3><p>${state.records.length ? 'Your local inputs are selected. Run Explore & QC or Compare Groups to create a browser result.' : 'Select project inputs first.'}</p></section>`
  }
  return `<section class="page-heading"><div><span class="eyebrow">Results</span><h2>Local analysis results</h2><p>No result data have left this device.</p></div></section>${cards.join('')}`
}

function docsView() {
  return `<section class="page-heading"><div><span class="eyebrow">Docs</span><h2>Methods and interpretation</h2><p>4C Insight keeps analysis assumptions and limitations visible next to the workflows that use them.</p></div></section><div class="docs-grid"><article class="card"><h3>Local-first execution</h3><p>Selected files remain on the user's computer. Browser workers and Rust/WebAssembly perform analysis locally. Results are saved only when the user chooses an export.</p></article><article class="card"><h3>Expression QC</h3><p>Metadata tables and gene/transcript workbooks are parsed from bytes in Rust. Expression workbooks are processed one at a time so the browser does not retain the complete cohort in source-file buffers.</p></article><article class="card"><h3>W3 corrected group comparison</h3><p>Gene analysis uses Raw Count columns. Transcript analysis uses Total Reads columns. Positive-count median-ratio size factors are followed by the established negative-binomial log-link group model. A selected patient/subject column enables subject-clustered sandwich standard errors for repeated measurements. Selected covariates are added to the same model.</p></article><article class="card"><h3>PCA and hierarchical clustering</h3><p>Global PCA and the dendrogram use top-variable normalized features. Case/Control labels do not select those features. The dendrogram uses average linkage with Euclidean distance. These plots describe cohort structure and are distinct from post-selection significant-hit plots.</p></article><article class="card"><h3>Sample exclusions</h3><p>Reviewed-QC and custom metadata rules are optional and independent. Each rule uses a user-selected metadata column and one or more exact observed values. Matching metadata rows are removed before Case/Control construction and model fitting, and the exact rule and excluded sample IDs are stored in the result.</p></article><article class="card"><h3>Transcript usage</h3><p>The transcript workflow also runs the existing differential transcript-usage screen. It aggregates transcript proportions within genes, applies the established CLR-based transcript-usage model, and reports BH-adjusted gene and transcript results.</p></article><article class="card"><h3>Scientific parity</h3><p>W3 fixtures run the same merged expression tables through native filesystem APIs and browser-safe byte APIs. The enabled workflow must match for size factors, differential tests, PCA, dendrograms, and transcript DTU outputs.</p></article><article class="card"><h3>Longitudinal and significant hits</h3><p>These remain separate workflows. Longitudinal analysis asks whether repeated samples preserve patient identity. Significant-hit PCA and clustering are post-selection descriptions and do not provide independent predictive validation.</p></article></div>`
}

async function inspectSelectedMetadata() {
  const id = state.qcConfig.metadataId
  const file = selectedFile(id)
  if (!file || !state.wasmReady || state.metadataInspectBusy) return
  state.metadataInspectBusy = true
  state.error = null
  render()
  try {
    const response = await workerRequest('inspect-metadata', {
      file,
      idColumn: state.qcConfig.metadataIdColumn,
    })
    state.metadataInspection = response.result
    state.metadataInspectionFileId = id
    if (!state.qcConfig.metadataIdColumn) state.qcConfig.metadataIdColumn = response.result.id_column
  } catch (error) {
    state.metadataInspection = null
    state.metadataInspectionFileId = null
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.metadataInspectBusy = false
    render()
  }
}

async function runExpressionQc() {
  const geneRecords = recordsOfKind('gene-expression')
  const transcriptRecords = recordsOfKind('transcript-expression')
  const metadataFile = selectedFile(state.qcConfig.metadataId)
  state.busy = true
  state.error = null
  state.qcResult = null
  state.qcProgress = { completed: 0, total: 1, message: 'Starting local QC' }
  render()
  try {
    const response = await workerRequest('run-expression-qc', {
      metadataFile,
      metadataIdColumn: state.qcConfig.metadataIdColumn,
      gene: {
        enabled: state.qcConfig.geneEnabled,
        metric: state.qcConfig.geneMetric,
        files: state.qcConfig.geneEnabled ? geneRecords.map((record) => selectedFile(record.id)).filter(Boolean) : [],
      },
      transcript: {
        enabled: state.qcConfig.transcriptEnabled,
        metric: state.qcConfig.transcriptMetric,
        files: state.qcConfig.transcriptEnabled ? transcriptRecords.map((record) => selectedFile(record.id)).filter(Boolean) : [],
      },
      ignoreZeros: state.qcConfig.ignoreZeros,
    }, (progress) => {
      state.qcProgress = progress
      render()
    })
    state.qcResult = response.result
    if (response.result.metadata) {
      state.metadataInspection = response.result.metadata
      state.metadataInspectionFileId = state.qcConfig.metadataId
    }
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    state.qcProgress = null
    render()
  }
}


async function runGroupComparison() {
  const cfg = state.compareConfig
  const metadataFile = selectedFile(state.qcConfig.metadataId)
  const geneFile = selectedFile(cfg.geneMergedId)
  const transcriptFile = selectedFile(cfg.transcriptMergedId)
  if (!metadataFile) {
    state.error = 'Select a metadata file before running the comparison.'
    render()
    return
  }

  const exclusions = [
    {
      enabled: cfg.qcExclusionEnabled,
      label: 'Reviewed QC exclusion',
      column: cfg.qcExclusionColumn,
      values: cfg.qcExclusionValues,
    },
    {
      enabled: cfg.customExclusionEnabled,
      label: 'Custom metadata exclusion',
      column: cfg.customExclusionColumn,
      values: cfg.customExclusionValues,
    },
  ]
  const caseLabel = cfg.caseValues.join(' + ')
  const controlLabel = cfg.controlValues.join(' + ')
  const comparisonName = `${caseLabel || 'Case'} vs ${controlLabel || 'Control'}`
  const portableConfig = {
    metadataIdColumn: state.qcConfig.metadataIdColumn,
    groupColumn: cfg.groupColumn,
    caseValues: cfg.caseValues,
    controlValues: cfg.controlValues,
    comparisonName,
    caseLabel: caseLabel || 'Case',
    controlLabel: controlLabel || 'Control',
    subjectColumn: cfg.subjectColumn || null,
    covariates: cfg.covariates,
    exclusions,
    minCount: Number(cfg.minCount),
    minFractionSamples: Number(cfg.minFractionSamples),
  }

  state.busy = true
  state.error = null
  state.compareResult = null
  state.compareProgress = { completed: 0, total: Number(cfg.geneEnabled) + Number(cfg.transcriptEnabled), message: 'Starting local comparison' }
  render()
  try {
    const response = await workerRequest('run-expression-comparison', {
      metadataFile,
      comparisonName,
      config: portableConfig,
      gene: {
        enabled: Boolean(cfg.geneEnabled && geneFile),
        file: cfg.geneEnabled ? geneFile : null,
      },
      transcript: {
        enabled: Boolean(cfg.transcriptEnabled && transcriptFile),
        file: cfg.transcriptEnabled ? transcriptFile : null,
      },
    }, (progress) => {
      state.compareProgress = progress
      render()
    })
    state.compareResult = response.result
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    state.compareProgress = null
    render()
  }
}

function bindCompareEvents() {
  const cfg = state.compareConfig
  const rerender = () => render()

  document.querySelector('#compare-metadata-file')?.addEventListener('change', (event) => {
    state.qcConfig.metadataId = event.currentTarget.value
    state.qcConfig.metadataIdColumn = ''
    state.metadataInspection = null
    state.metadataInspectionFileId = null
    state.metadataValueCache.clear()
    state.compareResult = null
    rerender()
  })
  document.querySelector('#compare-id-column')?.addEventListener('change', (event) => {
    state.qcConfig.metadataIdColumn = event.currentTarget.value
    state.metadataInspection = null
    state.metadataInspectionFileId = null
    state.metadataValueCache.clear()
    state.compareResult = null
    void inspectSelectedMetadata()
  })
  document.querySelector('#compare-subject-column')?.addEventListener('change', (event) => {
    cfg.subjectColumn = event.currentTarget.value
    cfg.covariates = cfg.covariates.filter((column) => column !== cfg.subjectColumn)
    state.compareResult = null
    rerender()
  })
  document.querySelector('#compare-group-column')?.addEventListener('change', (event) => {
    cfg.groupColumn = event.currentTarget.value
    cfg.caseValues = []
    cfg.controlValues = []
    state.compareResult = null
    void loadMetadataValues(cfg.groupColumn).finally(rerender)
  })
  document.querySelector('#compare-case-values')?.addEventListener('change', (event) => {
    cfg.caseValues = selectedValues(event.currentTarget)
    state.compareResult = null
    rerender()
  })
  document.querySelector('#compare-control-values')?.addEventListener('change', (event) => {
    cfg.controlValues = selectedValues(event.currentTarget)
    state.compareResult = null
    rerender()
  })
  document.querySelector('#compare-covariates')?.addEventListener('change', (event) => {
    cfg.covariates = selectedValues(event.currentTarget)
    state.compareResult = null
  })

  for (const [prefix, key] of [['compare-qc-exclusion', 'qc'], ['compare-custom-exclusion', 'custom']]) {
    const enabledKey = key === 'qc' ? 'qcExclusionEnabled' : 'customExclusionEnabled'
    const columnKey = key === 'qc' ? 'qcExclusionColumn' : 'customExclusionColumn'
    const valuesKey = key === 'qc' ? 'qcExclusionValues' : 'customExclusionValues'
    document.querySelector(`#${prefix}-enabled`)?.addEventListener('change', (event) => {
      cfg[enabledKey] = event.currentTarget.checked
      state.compareResult = null
      rerender()
    })
    document.querySelector(`#${prefix}-column`)?.addEventListener('change', (event) => {
      cfg[columnKey] = event.currentTarget.value
      cfg[valuesKey] = []
      state.compareResult = null
      if (cfg[columnKey]) void loadMetadataValues(cfg[columnKey]).finally(rerender)
      else rerender()
    })
    document.querySelector(`#${prefix}-values`)?.addEventListener('change', (event) => {
      cfg[valuesKey] = selectedValues(event.currentTarget)
      state.compareResult = null
      rerender()
    })
  }

  document.querySelector('#compare-gene-enabled')?.addEventListener('change', (event) => {
    cfg.geneEnabled = event.currentTarget.checked
    state.compareResult = null
    rerender()
  })
  document.querySelector('#compare-gene-file')?.addEventListener('change', (event) => {
    cfg.geneMergedId = event.currentTarget.value
    state.compareResult = null
  })
  document.querySelector('#compare-transcript-enabled')?.addEventListener('change', (event) => {
    cfg.transcriptEnabled = event.currentTarget.checked
    state.compareResult = null
    rerender()
  })
  document.querySelector('#compare-transcript-file')?.addEventListener('change', (event) => {
    cfg.transcriptMergedId = event.currentTarget.value
    state.compareResult = null
  })
  document.querySelector('#compare-min-count')?.addEventListener('change', (event) => {
    cfg.minCount = Number(event.currentTarget.value)
    state.compareResult = null
  })
  document.querySelector('#compare-min-fraction')?.addEventListener('change', (event) => {
    cfg.minFractionSamples = Number(event.currentTarget.value)
    state.compareResult = null
  })
  document.querySelector('#run-group-comparison')?.addEventListener('click', () => void runGroupComparison())
  document.querySelector('#export-comparison')?.addEventListener('click', () => void exportCompareResult())

  if (
    state.qcConfig.metadataId &&
    state.wasmReady &&
    !state.metadataInspectBusy &&
    state.metadataInspectionFileId !== state.qcConfig.metadataId
  ) {
    void inspectSelectedMetadata()
    return
  }
  for (const column of [cfg.groupColumn, cfg.qcExclusionEnabled ? cfg.qcExclusionColumn : '', cfg.customExclusionEnabled ? cfg.customExclusionColumn : '']) {
    if (column && !state.metadataValueCache.has(metadataValueKey(column))) {
      void loadMetadataValues(column).then(rerender).catch((error) => {
        state.error = error instanceof Error ? error.message : String(error)
        rerender()
      })
    }
  }
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

function bindQcEvents() {
  document.querySelector('#metadata-file')?.addEventListener('change', (event) => {
    state.qcConfig.metadataId = event.currentTarget.value
    state.qcConfig.metadataIdColumn = ''
    state.metadataInspection = null
    state.metadataInspectionFileId = null
    render()
  })
  document.querySelector('#metadata-id-column')?.addEventListener('change', (event) => {
    state.qcConfig.metadataIdColumn = event.currentTarget.value
    state.metadataInspection = null
    state.metadataInspectionFileId = null
    void inspectSelectedMetadata()
  })
  document.querySelectorAll('.modality-toggle').forEach((input) => input.addEventListener('change', () => {
    if (input.dataset.kind === 'gene') state.qcConfig.geneEnabled = input.checked
    if (input.dataset.kind === 'transcript') state.qcConfig.transcriptEnabled = input.checked
    render()
  }))
  document.querySelectorAll('.metric-select').forEach((select) => select.addEventListener('change', () => {
    if (select.dataset.kind === 'gene') state.qcConfig.geneMetric = select.value
    if (select.dataset.kind === 'transcript') state.qcConfig.transcriptMetric = select.value
  }))
  document.querySelector('#ignore-zeros')?.addEventListener('change', (event) => {
    state.qcConfig.ignoreZeros = event.currentTarget.checked
  })
  document.querySelector('#run-expression-qc')?.addEventListener('click', () => void runExpressionQc())
  document.querySelector('#export-qc')?.addEventListener('click', () => void exportQcResult())

  if (
    state.qcConfig.metadataId &&
    state.wasmReady &&
    !state.metadataInspectBusy &&
    state.metadataInspectionFileId !== state.qcConfig.metadataId
  ) {
    void inspectSelectedMetadata()
  }
}

function bindResultsEvents() {
  document.querySelector('#results-open-qc')?.addEventListener('click', () => { state.tab = 'qc'; render() })
  document.querySelector('#results-export-qc')?.addEventListener('click', () => void exportQcResult())
  document.querySelector('#results-open-compare')?.addEventListener('click', () => { state.tab = 'compare'; render() })
  document.querySelector('#results-export-compare')?.addEventListener('click', () => void exportCompareResult())
}

function render() {
  renderNav()
  const main = document.querySelector('#main-content')
  const views = {
    project: projectView(),
    qc: qcView(),
    compare: compareView(),
    longitudinal: migrationView('Longitudinal', 'W3'),
    hits: migrationView('Significant Hits', 'W3'),
    results: resultsView(),
    docs: docsView(),
  }
  main.innerHTML = views[state.tab]
  if (state.tab === 'project') bindProjectEvents()
  if (state.tab === 'qc') bindQcEvents()
  if (state.tab === 'compare') bindCompareEvents()
  if (state.tab === 'results') bindResultsEvents()
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
