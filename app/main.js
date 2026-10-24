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
  qcProgress: null,
  qcResult: null,
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
  state.qcResult = null
  state.qcProgress = null
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
  state.qcResult = null
  state.qcProgress = null
  state.qcConfig.metadataId = ''
  state.qcConfig.metadataIdColumn = ''
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

function migrationView(title, step) {
  const records = recognizedCount()
  return `<section class="page-heading"><div><span class="eyebrow">${title}</span><h2>${title}</h2><p>This workflow is being moved to the shared Rust/WASM engine.</p></div></section>
  <section class="card migration-card"><span class="tag neutral">Migration ${step}</span><h3>The workflow is visible, but scientific execution is not enabled yet.</h3><p>The existing Rust implementation remains the scientific reference. 4C Insight enables this action only after the browser-safe API and native/WASM parity tests pass.</p><div class="migration-checks"><div><span class="check done">✓</span><span>Local file selection and worker boundary</span></div><div><span class="check done">✓</span><span>Metadata and expression byte readers</span></div><div><span class="check done">✓</span><span>${records} recognized local input${records === 1 ? '' : 's'} available</span></div><div><span class="check">→</span><span>Workflow-specific statistical parity</span></div></div></section>`
}

function resultsView() {
  if (!state.qcResult) {
    return `<section class="page-heading"><div><span class="eyebrow">Results</span><h2>Results stay with the project on this computer</h2><p>Completed workflows appear here with local exports.</p></div></section><section class="card empty-results"><div class="empty-icon" aria-hidden="true">↧</div><h3>No WASM analysis has completed yet</h3><p>${state.records.length ? 'Your local inputs are selected. Run Explore & QC to create the first browser result.' : 'Select project inputs first.'}</p></section>`
  }
  return `<section class="page-heading"><div><span class="eyebrow">Results</span><h2>Local analysis results</h2><p>No result data have left this device.</p></div></section><section class="card result-card"><div><span class="tag success">Completed</span><h3>Explore & QC · expression distributions</h3><p>${state.qcResult.gene.length} gene-expression samples and ${state.qcResult.transcript.length} transcript-expression samples processed with Rust/WASM.</p></div><div class="button-row"><button id="results-open-qc" class="secondary-button">View QC</button><button id="results-export-qc" class="primary-button">Save QC JSON</button></div></section>`
}

function docsView() {
  return `<section class="page-heading"><div><span class="eyebrow">Docs</span><h2>Methods and interpretation</h2><p>4C Insight keeps analysis assumptions and limitations visible next to the workflows that use them.</p></div></section><div class="docs-grid"><article class="card"><h3>Local-first execution</h3><p>Selected files remain on the user's computer. Browser workers and Rust/WebAssembly perform analysis locally. Results are saved only when the user chooses an export.</p></article><article class="card"><h3>W2 expression QC</h3><p>Metadata tables and gene/transcript workbooks are parsed from bytes in Rust. Expression workbooks are processed one at a time. The QC table reports the same distribution statistics used by the native implementation.</p></article><article class="card"><h3>Scientific parity</h3><p>Fixed fixtures compare the native path reader with the browser-safe byte reader. Later workflows remain disabled until their statistical outputs pass equivalent parity tests.</p></article><article class="card"><h3>Longitudinal analysis</h3><p>Repeated samples can be grouped by a user-selected patient column and ordered by a user-selected observation or time column. Patient-specific clustering is distinct from Case/Control structure.</p></article><article class="card"><h3>Significant-hit structure</h3><p>PCA, dendrograms, and heatmaps restricted to significant hits are post-selection descriptions. They do not provide independent evidence of predictive performance.</p></article></div>`
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
}

function render() {
  renderNav()
  const main = document.querySelector('#main-content')
  const views = {
    project: projectView(),
    qc: qcView(),
    compare: migrationView('Compare Groups', 'W3'),
    longitudinal: migrationView('Longitudinal', 'W3'),
    hits: migrationView('Significant Hits', 'W3'),
    results: resultsView(),
    docs: docsView(),
  }
  main.innerHTML = views[state.tab]
  if (state.tab === 'project') bindProjectEvents()
  if (state.tab === 'qc') bindQcEvents()
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
