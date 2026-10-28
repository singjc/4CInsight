import { saveTextFile } from './save.js'
import { matchSavedFiles, matchSavedReference } from './state-reconnect.js'
import { createSystemMonitor } from './system-monitor.js'
import { methodDocs } from './docs-content.js'
import { DEFAULT_RESOURCES, normalizeResources, suggestedScienceThreads, thinPlotPoints, measureAppMemory } from './resource-controls.js'
import {
  clearHistoryEntries,
  deleteHistoryEntry,
  listHistoryEntries,
  loadHistorySnapshot,
  saveHistoryEntry,
  storageEstimate,
} from './persistence.js'

const APP_VERSION = '0.21.0-ux7-native-monitor'
const APP_STATE_SCHEMA = 1
const dynamicTableSources = new Map()

const tabs = [
  ['project', 'Project', '1'],
  ['qc', 'Explore & QC', '2'],
  ['compare', 'Compare Groups', '3'],
  ['longitudinal', 'Longitudinal', '4'],
  ['hits', 'Significant Hits', '5'],
  ['history', 'History', '6'],
  ['docs', 'Docs', '?'],
  ['settings', 'Settings', '⚙'],
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
  projectMessage: '',
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
  longitudinalProgress: null,
  longitudinalResult: null,
  hitProgress: null,
  hitResult: null,
  historyEntries: [],
  historyLoaded: false,
  historyBusy: false,
  historyMessage: '',
  storageEstimate: null,
  loadedStateManifest: null,
  pendingStateConfig: null,
  docsQuery: '',
  globalThresholds: { fdr: 0.05, absLog2Fc: 1.0 },
  systemInfo: null,
  appMemorySample: null,
  appMemoryBusy: false,
  monitorBusy: false,
  resources: loadResourceSettings(),
  hitConfig: {
    maxAnalysisFeatures: 500,
    maxHeatmapFeatures: 80,
    geneEnabled: true,
    transcriptEnabled: true,
  },
  longitudinalConfig: {
    observationIdColumn: '',
    patientIdColumn: '',
    timeColumn: '',
    exclusionEnabled: false,
    exclusionColumn: '',
    exclusionValues: [],
    geneEnabled: true,
    geneMergedId: '',
    transcriptEnabled: true,
    transcriptMergedId: '',
    minCount: 10,
    minFractionSamples: 0.20,
  },
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

// Native monitoring is optional. No network access takes place before Connect.
const nativeMonitor = createSystemMonitor()
let monitorTimer = null
let monitorRefreshing = false
const pending = new Map()
let worker = null
function attachScienceWorker() {
  const current = new Worker(new URL('./analysis-worker.js', import.meta.url), { type: 'module' })
  current.addEventListener('message', (event) => {
    if (current !== worker) return
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
    if (request.type.startsWith('run-') && state.resources.recycleWorker && !pending.size) {
      // Termination releases the WASM linear-memory high-water mark after a run.
      replaceScienceWorker()
    }
  })
  current.addEventListener('error', (event) => {
    if (current !== worker) return
    const error = new Error(event.message || '4C Insight analysis worker failed')
    for (const request of pending.values()) request.reject(error)
    pending.clear()
    current.terminate()
    worker = null // Recreate lazily on the next user request; avoid a crash/restart loop.
    state.error = error.message
    render()
  })
  return current
}
function replaceScienceWorker() {
  worker?.terminate()
  worker = null // Keep no worker alive when idle. The next request creates one.
}
function stopScienceWork() {
  if (!pending.size) return
  const error = new Error('Operation stopped. The science worker and its WASM memory were released.')
  for (const request of pending.values()) request.reject(error)
  pending.clear()
  replaceScienceWorker()
  state.projectMessage = 'The scientific operation was stopped. Selected local files are unchanged.'
  render()
}
worker = attachScienceWorker()

function workerRequest(type, payload = {}, onProgress = null) {
  const requestId = crypto.randomUUID()
  if (!worker) worker = attachScienceWorker()
  return new Promise((resolve, reject) => {
    pending.set(requestId, { resolve, reject, onProgress, type })
    worker.postMessage({ type, requestId, resourceSettings: state.resources, ...payload })
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

function loadResourceSettings() {
  try { return normalizeResources(JSON.parse(localStorage.getItem('4c-insight-resources') || '{}')) }
  catch { return { ...DEFAULT_RESOURCES } }
}
function persistResourceSettings() {
  try { localStorage.setItem('4c-insight-resources', JSON.stringify(state.resources)) }
  catch { state.error = 'This browser cannot save resource settings. They apply only until the page is closed.' }
}

function detectSystemInfo() {
  const platform = navigator.userAgentData?.platform || navigator.platform || 'Unknown OS'
  const threads = Number(navigator.hardwareConcurrency || 0)
  const deviceMemory = Number(navigator.deviceMemory || 0)
  const heapLimit = Number(globalThis.performance?.memory?.jsHeapSizeLimit || 0)
  let memory = 'System RAM not exposed by browser'
  if (deviceMemory > 0) memory = `≈${formatNumber(deviceMemory, 1)} GiB device memory class (approximate)`
  else if (heapLimit > 0) memory = `${humanBytes(heapLimit)} JS heap limit (not RAM)`
  return {
    platform,
    threads,
    memory,
    crossOriginIsolated: Boolean(globalThis.crossOriginIsolated),
  }
}

function renderSystemInfo() {
  state.systemInfo ||= detectSystemInfo()
  const chip = document.querySelector('#system-chip')
  if (!chip) return
  const info = state.systemInfo
  const threadText = info.threads ? `${info.threads} logical threads` : 'threads unavailable'
  const monitor = nativeMonitor.getState()
  const native = monitor.status === 'connected' ? monitor.last : null
  const memoryText = native
    ? `${humanBytes(native.available_memory_bytes)} RAM available / ${humanBytes(native.total_memory_bytes)} total (native)`
    : state.resources.physicalRamGiB ? `${state.resources.physicalRamGiB} GiB RAM (manual)` : info.memory
  chip.innerHTML = `<span class="privacy-dot"></span><span><strong>${escapeHtml(info.platform)}</strong><small>${escapeHtml(memoryText)} · ${escapeHtml(threadText)} · WASM science: 1 worker · Settings</small></span>`
  chip.title = `Open Settings · ${native ? 'Native sysinfo connected, read-only host metrics' : 'No native system monitor connected'} · ${info.crossOriginIsolated ? 'cross-origin isolated' : 'single-worker fallback'}`
}

function monitorSummaryHtml() {
  const monitor = nativeMonitor.getState()
  const snapshot = monitor.status === 'connected' ? monitor.last : null
  if (snapshot) {
    const percent = (100 * snapshot.available_memory_bytes / snapshot.total_memory_bytes).toFixed(1)
    return `<strong>Native system monitor connected</strong><span>Host RAM: ${humanBytes(snapshot.total_memory_bytes)} total · ${humanBytes(snapshot.available_memory_bytes)} available (${percent}%) · ${humanBytes(snapshot.used_memory_bytes)} used</span><span>CPU: ${formatNumber(snapshot.cpu_usage_percent, 1)}% total usage across ${snapshot.logical_cpus} logical CPUs · swap: ${humanBytes(snapshot.used_swap_bytes)} used of ${humanBytes(snapshot.total_swap_bytes)}</span><small>Host metrics from local sysinfo, not browser-process or WASM memory. Updated when the monitor responds.</small>`
  }
  return `<strong>Native system monitor: ${escapeHtml(monitor.status)}</strong><span>${escapeHtml(monitor.error || 'Optional monitor is not connected. Browser-only operation remains available.')}</span>`
}

function updateMonitorDisplay() {
  renderSystemInfo()
  const status = document.querySelector('#native-monitor-status')
  if (status) status.innerHTML = monitorSummaryHtml()
}

function stopMonitorPolling() {
  if (monitorTimer !== null) { clearInterval(monitorTimer); monitorTimer = null }
}

async function pollNativeMonitor() {
  if (monitorRefreshing || nativeMonitor.getState().status !== 'connected') return
  monitorRefreshing = true
  try { await nativeMonitor.refresh() }
  finally {
    monitorRefreshing = false
    if (nativeMonitor.getState().status !== 'connected') stopMonitorPolling()
    updateMonitorDisplay()
  }
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
  state.longitudinalResult = null
  state.longitudinalProgress = null
  state.hitResult = null
  state.hitProgress = null

  const geneMerged = recordsOfKind('merged-gene-expression')
  const transcriptMerged = recordsOfKind('merged-transcript-expression')
  if (!geneMerged.some((record) => record.id === state.compareConfig.geneMergedId)) {
    state.compareConfig.geneMergedId = geneMerged[0]?.id || ''
  }
  if (!transcriptMerged.some((record) => record.id === state.compareConfig.transcriptMergedId)) {
    state.compareConfig.transcriptMergedId = transcriptMerged[0]?.id || ''
  }
  if (!geneMerged.some((record) => record.id === state.longitudinalConfig.geneMergedId)) {
    state.longitudinalConfig.geneMergedId = geneMerged[0]?.id || ''
  }
  if (!transcriptMerged.some((record) => record.id === state.longitudinalConfig.transcriptMergedId)) {
    state.longitudinalConfig.transcriptMergedId = transcriptMerged[0]?.id || ''
  }
  state.longitudinalConfig.observationIdColumn = ''
  state.longitudinalConfig.patientIdColumn = ''
  state.longitudinalConfig.timeColumn = ''
  state.longitudinalConfig.exclusionEnabled = false
  state.longitudinalConfig.exclusionColumn = ''
  state.longitudinalConfig.exclusionValues = []
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
    applyPendingStateConfig()
    if (state.loadedStateManifest) state.projectMessage = reconnectMessage()
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
  state.projectMessage = ''
  state.loadedStateManifest = null
  state.pendingStateConfig = null
  state.engine = 'Waiting for local inputs'
  state.wasmReady = false
  state.metadataInspection = null
  state.metadataInspectionFileId = null
  state.metadataValueCache.clear()
  state.qcResult = null
  state.qcProgress = null
  state.compareResult = null
  state.compareProgress = null
  state.longitudinalResult = null
  state.longitudinalProgress = null
  state.hitResult = null
  state.hitProgress = null
  state.qcConfig.metadataId = ''
  state.qcConfig.metadataIdColumn = ''
  state.compareConfig.groupColumn = ''
  state.compareConfig.caseValues = []
  state.compareConfig.controlValues = []
  state.compareConfig.subjectColumn = ''
  state.compareConfig.covariates = []
  state.compareConfig.geneMergedId = ''
  state.compareConfig.transcriptMergedId = ''
  state.longitudinalConfig.observationIdColumn = ''
  state.longitudinalConfig.patientIdColumn = ''
  state.longitudinalConfig.timeColumn = ''
  state.longitudinalConfig.exclusionEnabled = false
  state.longitudinalConfig.exclusionColumn = ''
  state.longitudinalConfig.exclusionValues = []
  state.longitudinalConfig.geneMergedId = ''
  state.longitudinalConfig.transcriptMergedId = ''
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


async function exportLongitudinalResult() {
  if (!state.longitudinalResult) return
  const payload = {
    product: '4C Insight',
    schemaVersion: 1,
    workflow: 'longitudinal-patient-signature-expression',
    config: state.longitudinalConfig,
    result: state.longitudinalResult,
  }
  await saveTextFile('4c-insight-longitudinal-patient-signature.json', `${JSON.stringify(payload, null, 2)}\n`)
}

async function exportHitResult() {
  if (!state.hitResult) return
  const payload = {
    product: '4C Insight',
    schemaVersion: 1,
    workflow: 'significant-hit-structure-expression',
    comparisonConfig: {
      metadataIdColumn: state.qcConfig.metadataIdColumn,
      ...state.compareConfig,
    },
    hitConfig: state.hitConfig,
    result: state.hitResult,
  }
  await saveTextFile('4c-insight-significant-hit-structure.json', `${JSON.stringify(payload, null, 2)}\n`)
}


function cloneJson(value) {
  return JSON.parse(JSON.stringify(value))
}

function projectFileRef(id) {
  const record = state.records.find((candidate) => candidate.id === id)
  if (!record) return null
  return {
    name: record.name,
    relativePath: record.relativePath || record.name,
    size: record.size,
    lastModified: record.lastModified,
    kind: record.kind,
    label: record.label,
  }
}

function savedProjectManifest() {
  return state.records.map((record) => ({
    name: record.name,
    relativePath: record.relativePath || record.name,
    size: record.size,
    lastModified: record.lastModified,
    kind: record.kind,
    label: record.label,
    recognized: Boolean(record.recognized),
  }))
}

function configSnapshot() {
  const qc = cloneJson(state.qcConfig)
  const compare = cloneJson(state.compareConfig)
  const longitudinal = cloneJson(state.longitudinalConfig)
  const hit = cloneJson(state.hitConfig)
  const pendingRefs = state.pendingStateConfig?.fileRefs || {}
  const fileRefs = {
    metadata: projectFileRef(qc.metadataId) || pendingRefs.metadata || null,
    compareGene: projectFileRef(compare.geneMergedId) || pendingRefs.compareGene || null,
    compareTranscript: projectFileRef(compare.transcriptMergedId) || pendingRefs.compareTranscript || null,
    longitudinalGene: projectFileRef(longitudinal.geneMergedId) || pendingRefs.longitudinalGene || null,
    longitudinalTranscript: projectFileRef(longitudinal.transcriptMergedId) || pendingRefs.longitudinalTranscript || null,
  }
  delete qc.metadataId
  delete compare.geneMergedId
  delete compare.transcriptMergedId
  delete longitudinal.geneMergedId
  delete longitudinal.transcriptMergedId
  return { qc, compare, longitudinal, hit, thresholds: cloneJson(state.globalThresholds), fileRefs }
}

function applicationStatePayload() {
  return {
    product: '4C Insight',
    kind: 'application-state',
    schemaVersion: APP_STATE_SCHEMA,
    appVersion: APP_VERSION,
    createdAt: new Date().toISOString(),
    activeTab: state.tab,
    project: {
      files: savedProjectManifest(),
      note: 'File metadata only. Raw cohort bytes are not embedded in this state.',
    },
    configs: configSnapshot(),
  }
}

function historyStatePayload(workflow) {
  const results = {}
  if (workflow === 'explore-qc-expression') results.qc = state.qcResult
  if (workflow === 'compare-groups-expression') results.compare = state.compareResult
  if (workflow === 'longitudinal-patient-signature-expression') results.longitudinal = state.longitudinalResult
  if (workflow === 'significant-hit-structure-expression') {
    results.compare = state.compareResult
    results.hits = state.hitResult
  }
  return {
    product: '4C Insight',
    kind: 'history-snapshot',
    schemaVersion: APP_STATE_SCHEMA,
    appVersion: APP_VERSION,
    createdAt: new Date().toISOString(),
    workflow,
    project: {
      files: savedProjectManifest(),
      note: 'File metadata only. Raw cohort bytes are not stored in History.',
    },
    configs: configSnapshot(),
    results,
  }
}

function resolveSavedFileRef(reference) {
  return matchSavedReference(reference, state.records)?.id || ''
}

function stateReconnectStatus() {
  return matchSavedFiles(state.loadedStateManifest, state.records)
}

function reconnectMessage() {
  const report = stateReconnectStatus()
  if (!report.expected) return 'Loaded analysis settings. This state does not reference any cohort files.'
  if (!report.matched) return `Loaded ${report.expected.toLocaleString()} file references. Select the original project folder once to reconnect local files. The state JSON does not contain raw data.`
  const gaps = report.missing.length + report.ambiguous.length
  return `${report.matched.toLocaleString()} of ${report.expected.toLocaleString()} saved files reconnected. ${gaps ? `${report.missing.length.toLocaleString()} missing, ${report.ambiguous.length.toLocaleString()} ambiguous.` : 'All saved file references are connected.'}`
}

function applyConfigSnapshot(configs) {
  if (!configs || typeof configs !== 'object') return
  const refs = configs.fileRefs || {}
  state.qcConfig = { ...state.qcConfig, ...(configs.qc || {}), metadataId: resolveSavedFileRef(refs.metadata) }
  state.compareConfig = {
    ...state.compareConfig,
    ...(configs.compare || {}),
    geneMergedId: resolveSavedFileRef(refs.compareGene),
    transcriptMergedId: resolveSavedFileRef(refs.compareTranscript),
  }
  state.longitudinalConfig = {
    ...state.longitudinalConfig,
    ...(configs.longitudinal || {}),
    geneMergedId: resolveSavedFileRef(refs.longitudinalGene),
    transcriptMergedId: resolveSavedFileRef(refs.longitudinalTranscript),
  }
  state.hitConfig = { ...state.hitConfig, ...(configs.hit || {}) }
  state.globalThresholds = { ...state.globalThresholds, ...(configs.thresholds || {}) }
  if (configs.hit?.qThreshold !== undefined && configs.thresholds?.fdr === undefined) state.globalThresholds.fdr = Number(configs.hit.qThreshold)
  if (configs.hit?.minAbsEffect !== undefined && configs.thresholds?.absLog2Fc === undefined) state.globalThresholds.absLog2Fc = Number(configs.hit.minAbsEffect)

  const required = Object.values(refs).filter(Boolean)
  const resolved = required.filter((reference) => resolveSavedFileRef(reference)).length
  if (required.length && resolved < required.length) state.pendingStateConfig = cloneJson(configs)
  else state.pendingStateConfig = null
}

function applyPendingStateConfig() {
  if (!state.pendingStateConfig) return
  const pending = cloneJson(state.pendingStateConfig)
  applyConfigSnapshot(pending)
}

async function saveApplicationState() {
  const payload = applicationStatePayload()
  await saveTextFile('4c-insight-state.4cstate.json', `${JSON.stringify(payload, null, 2)}\n`)
  state.projectMessage = `Application state saved. It contains settings and ${payload.project.files.length.toLocaleString()} file references, not raw cohort data.`
  render()
}

async function loadApplicationState(file) {
  if (!file) return
  state.error = null
  try {
    if (file.size > 10 * 1024 * 1024) throw new Error('State file exceeds 10 MiB.')
    const payload = JSON.parse(await file.text())
    if (payload?.product !== '4C Insight' || payload?.kind !== 'application-state' || payload?.schemaVersion !== APP_STATE_SCHEMA) {
      throw new Error('This is not a compatible 4C Insight application state.')
    }
    state.loadedStateManifest = Array.isArray(payload.project?.files) ? payload.project.files : []
    state.pendingStateConfig = cloneJson(payload.configs || {})
    applyPendingStateConfig()
    state.qcResult = null
    state.compareResult = null
    state.longitudinalResult = null
    state.hitResult = null
    state.projectMessage = reconnectMessage()
    state.tab = 'project'
  } catch (error) {
    state.error = `Load state: ${error instanceof Error ? error.message : String(error)}`
  }
  render()
}

function historySummary(workflow) {
  if (workflow === 'explore-qc-expression') {
    return `${state.qcResult?.gene?.length || 0} gene and ${state.qcResult?.transcript?.length || 0} transcript samples`
  }
  if (workflow === 'compare-groups-expression') {
    return state.compareResult?.comparisonName || 'Corrected group comparison'
  }
  if (workflow === 'longitudinal-patient-signature-expression') {
    const count = Number(Boolean(state.longitudinalResult?.gene)) + Number(Boolean(state.longitudinalResult?.transcript))
    return `${count} longitudinal expression data type${count === 1 ? '' : 's'}`
  }
  if (workflow === 'significant-hit-structure-expression') {
    const count = Number(Boolean(state.hitResult?.gene)) + Number(Boolean(state.hitResult?.transcript))
    return `${count} significant-hit structure panel${count === 1 ? '' : 's'}`
  }
  return '4C Insight analysis result'
}

async function recordHistory(workflow, label) {
  try {
    const id = crypto.randomUUID()
    const snapshot = historyStatePayload(workflow)
    const entry = {
      id,
      workflow,
      label,
      summary: historySummary(workflow),
      createdAt: snapshot.createdAt,
      appVersion: APP_VERSION,
      engine: state.engine,
      projectFileCount: state.records.length,
      localDataBytes: totalBytes(),
    }
    await saveHistoryEntry(entry, snapshot)
    state.historyLoaded = false
  } catch (error) {
    console.warn('4C Insight could not save this result to browser History.', error)
  }
}

async function refreshHistory() {
  if (state.historyBusy) return
  state.historyBusy = true
  try {
    state.historyEntries = await listHistoryEntries()
    state.storageEstimate = await storageEstimate()
    state.historyLoaded = true
  } catch (error) {
    state.historyMessage = `History could not be loaded: ${error instanceof Error ? error.message : String(error)}`
  } finally {
    state.historyBusy = false
    render()
  }
}

function restoreHistorySnapshot(snapshot) {
  if (!snapshot || snapshot.product !== '4C Insight' || snapshot.kind !== 'history-snapshot') return
  applyConfigSnapshot(snapshot.configs || {})
  state.qcResult = snapshot.results?.qc || null
  state.compareResult = snapshot.results?.compare || null
  state.longitudinalResult = snapshot.results?.longitudinal || null
  state.hitResult = snapshot.results?.hits || null
  const tabForWorkflow = {
    'explore-qc-expression': 'qc',
    'compare-groups-expression': 'compare',
    'longitudinal-patient-signature-expression': 'longitudinal',
    'significant-hit-structure-expression': 'hits',
  }
  state.tab = tabForWorkflow[snapshot.workflow] || 'history'
}

async function openHistoryEntry(id) {
  state.historyBusy = true
  render()
  try {
    const snapshot = await loadHistorySnapshot(id)
    if (!snapshot) throw new Error('History snapshot is no longer available.')
    restoreHistorySnapshot(snapshot)
  } catch (error) {
    state.historyMessage = `History entry could not be opened: ${error instanceof Error ? error.message : String(error)}`
  } finally {
    state.historyBusy = false
    render()
  }
}

async function exportHistorySnapshot(id) {
  const snapshot = await loadHistorySnapshot(id)
  if (!snapshot) return
  const safeWorkflow = String(snapshot.workflow || 'result').replaceAll(/[^a-z0-9-]+/gi, '-').toLowerCase()
  await saveTextFile(`4c-insight-history-${safeWorkflow}.json`, `${JSON.stringify(snapshot, null, 2)}\n`)
}

async function removeHistorySnapshot(id) {
  if (!window.confirm('Delete this History snapshot? Original cohort files are not touched.')) return
  state.historyBusy = true
  try {
    await deleteHistoryEntry(id)
    state.historyMessage = 'History snapshot deleted. Original cohort files were not changed.'
    state.historyEntries = await listHistoryEntries()
    state.storageEstimate = await storageEstimate()
  } catch (error) {
    state.historyMessage = `History snapshot could not be deleted: ${error instanceof Error ? error.message : String(error)}`
  } finally {
    state.historyBusy = false
    render()
  }
}

async function removeAllHistory() {
  if (!window.confirm('Delete all 4C Insight History snapshots stored by this browser? Original cohort files are not touched.')) return
  state.historyBusy = true
  try {
    await clearHistoryEntries()
    state.historyEntries = []
    state.storageEstimate = await storageEstimate()
    state.historyMessage = 'Browser History cleared. Original cohort files were not changed.'
  } catch (error) {
    state.historyMessage = `History could not be cleared: ${error instanceof Error ? error.message : String(error)}`
  } finally {
    state.historyBusy = false
    render()
  }
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
    <div><strong>${escapeHtml(state.engine)}</strong><span>${crossOriginIsolated ? 'Cross-origin isolated' : 'Single-worker compatible'}</span>${pending.size ? '<button id="stop-science-work" class="danger-text-button">Stop operation</button>' : ''}</div>`
}

function summaryCard(label, value, detail) {
  return `<div class="summary-card"><span>${label}</span><strong>${value}</strong><small>${detail}</small></div>`
}

function projectView() {
  const recognized = recognizedCount()
  const counts = modalityCounts()
  const expected = state.loadedStateManifest?.length || 0
  const reconnect = expected ? stateReconnectStatus() : null
  const remaining = reconnect ? reconnect.missing.length + reconnect.ambiguous.length : 0
  const status = state.records.length === 0
    ? 'No files selected'
    : recognized === state.records.length
      ? `${recognized} recognized input${recognized === 1 ? '' : 's'}`
      : `${recognized} recognized · ${state.records.length - recognized} need review`

  return `
    <section class="page-heading">
      <div><span class="eyebrow">Project · Step 1</span><h2>Select the cohort data on this computer</h2>
      <p>4C Insight references selected files in place. There is no upload step and drag/drop does not duplicate the source files.</p></div>
      <div class="project-heading-actions"><div class="local-card"><strong>Private by design</strong><span>Files remain on this device. You choose when results or state are saved.</span></div><div class="button-row compact state-actions"><button id="load-state" class="secondary-button">Load state</button><button id="save-state" class="secondary-button">Save state</button><input id="state-input" class="visually-hidden" type="file" accept=".json,.4cstate" /></div></div>
    </section>
    <section class="action-panel" aria-labelledby="select-data-heading">
      <div class="action-copy"><h3 id="select-data-heading">Add project data</h3><p>Select files, select a complete project folder, or drop one or more files into the Input inventory. Duplicate selections are ignored.</p></div>
      <div class="button-row">
        <button id="select-files" class="primary-button">Select files</button>
        <button id="select-folder" class="secondary-button">Select folder</button>
        <input id="file-input" class="visually-hidden" type="file" multiple />
        <input id="folder-input" class="visually-hidden" type="file" multiple webkitdirectory directory />
      </div>
    </section>
    ${state.projectMessage ? `<div class="notice-banner" role="status"><strong>Project state</strong><span>${escapeHtml(state.projectMessage)}</span></div>` : ''}
    ${reconnect ? `<section class="state-reconnect-panel" aria-label="Saved project files">
      <div><strong>Saved file references: ${reconnect.expected.toLocaleString()}</strong>
        <span>Reconnected: ${reconnect.matched.toLocaleString()} · Missing: ${reconnect.missing.length.toLocaleString()} · Ambiguous: ${reconnect.ambiguous.length.toLocaleString()}</span>
        <small>For security, a portable state cannot open local paths. Select the project folder to grant read access. Files remain on this device.</small></div>
      ${remaining ? '<button id="reconnect-state-folder" class="primary-button" type="button">Reconnect project folder</button>' : '<span class="tag success">All state files matched</span>'}
      ${reconnect.ambiguous.length ? `<small class="state-reconnect-warning">Duplicate names and sizes need unique relative paths. Select a parent folder that includes the original subfolders.</small>` : ''}
    </section>` : ''}
    ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
    <section class="summary-grid" aria-label="Project summary">
      ${summaryCard('Selected files', String(state.records.length), status)}
      ${summaryCard('Local data', humanBytes(totalBytes()), 'Referenced from this device')}
      ${summaryCard('Detected types', String(counts.length), 'Using 4C filename conventions')}
      ${summaryCard('Processing', state.busy ? 'Checking…' : 'Ready', reconnect ? `${reconnect.matched.toLocaleString()}/${expected.toLocaleString()} state files matched` : 'Preflight runs in a Web Worker')}
    </section>
    ${counts.length ? `<section class="card"><div class="card-heading"><div><h3>Detected data</h3><p>Review the automatic classification before you configure an analysis.</p></div></div><div class="modality-list">${counts.map(([label,count]) => `<div class="modality-pill"><span>${escapeHtml(label)}</span><strong>${count}</strong></div>`).join('')}</div></section>` : ''}
    <section id="input-drop-zone" class="card table-card input-drop-zone" aria-label="Input inventory drop zone">
      <div class="card-heading"><div><h3>Input inventory</h3><p>Drop files here or use the system picker above. Files are referenced in place; nothing is uploaded or copied into the application.</p></div>
      ${state.records.length ? '<div class="button-row compact"><button id="save-manifest" class="text-button">Save manifest</button><button id="clear-project" class="danger-text-button">Clear all</button></div>' : ''}</div>
      <div class="drop-hint" aria-hidden="true"><span class="drop-hint-icon">↓</span><div><strong>Drop one or more cohort files here</strong><span>For a complete directory tree, use Select folder.</span></div></div>
      ${state.records.length === 0 ? `<div class="empty-state"><div class="empty-icon" aria-hidden="true">＋</div><strong>No project data selected</strong><span>Start with sample metadata and any available expression, repertoire, or immune files.</span></div>` : `
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
  let status = 'Optional'
  if (enabled) status = column && selectedValues.length ? 'Active' : 'Needs input'
  return `<div class="config-block stacked">
    <div class="config-heading"><label class="check-label"><input id="${prefix}-enabled" type="checkbox" ${enabled ? 'checked' : ''}/><span><strong>${title}</strong><small>${help}</small></span></label><span class="tag ${enabled ? 'warning' : 'neutral'}">${status}</span></div>
    <label><span>Metadata column</span><select id="${prefix}-column" ${enabled ? '' : 'disabled'}>${columnOptions(columns, column, true)}</select></label>
    <label><span>Exclude rows with these values</span><select id="${prefix}-values" class="multi-select" multiple size="${Math.min(6, Math.max(3, values.length || 3))}" ${enabled && column ? '' : 'disabled'}>${values.length ? metadataValueOptions(values, selectedValues) : '<option disabled>Choose a column to load values</option>'}</select></label>
  </div>`
}

function plotControls(plotId) {
  return `<div class="plot-toolbar" role="toolbar" aria-label="Plot zoom controls">
    <button type="button" class="plot-tool-button" data-plot-id="${plotId}" data-plot-action="out" aria-label="Zoom out">−</button>
    <button type="button" class="plot-tool-button" data-plot-id="${plotId}" data-plot-action="fit">Fit</button>
    <button type="button" class="plot-tool-button" data-plot-id="${plotId}" data-plot-action="in" aria-label="Zoom in">+</button>
    <span class="plot-toolbar-hint">Ctrl/⌘ + wheel zooms plot coordinates · drag to pan</span>
  </div>`
}

function plotLegend(plotId, caseLabel, controlLabel) {
  return `<div class="plot-legend" aria-label="Group visibility">
    <button type="button" class="plot-legend-button" data-plot-id="${plotId}" data-plot-group-toggle="case" aria-pressed="true"><i class="legend-dot case-point"></i><span>Case · ${escapeHtml(caseLabel || 'Case')}</span></button>
    <button type="button" class="plot-legend-button" data-plot-id="${plotId}" data-plot-group-toggle="control" aria-pressed="true"><i class="legend-dot control-point"></i><span>Control · ${escapeHtml(controlLabel || 'Control')}</span></button>
  </div>`
}

function paddedBounds(min, max) {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [-1, 1]
  if (min === max) { min -= 1; max += 1 }
  const padding = (max - min) * 0.08
  return [min - padding, max + padding]
}
function paddedExtent(values) {
  // No Math.min(...largeArray). Huge restored results can exceed call limits.
  let min = Infinity
  let max = -Infinity
  for (const value of values) {
    if (Number.isFinite(value)) {
      if (value < min) min = value
      if (value > max) max = value
    }
  }
  return paddedBounds(min, max)
}

function tickValues(min, max, count = 5) {
  if (!Number.isFinite(min) || !Number.isFinite(max) || max <= min) return []
  return Array.from({ length: count }, (_, index) => min + ((max - min) * index) / (count - 1))
}

function pcaSvg(pca, plotId, caseLabel, controlLabel) {
  const points = pca?.points || []
  if (!points.length) return '<div class="plot-empty">PCA is not available for this result.</div>'
  const width = 760
  const height = 360
  const pad = 54
  const xs = points.map((point) => Number(point.x)).filter(Number.isFinite)
  const ys = points.map((point) => Number(point.y)).filter(Number.isFinite)
  if (!xs.length || !ys.length) return '<div class="plot-empty">PCA coordinates are not finite.</div>'
  const [minX, maxX] = paddedExtent(xs)
  const [minY, maxY] = paddedExtent(ys)
  const sx = (x) => pad + ((x - minX) / (maxX - minX)) * (width - pad * 2)
  const sy = (y) => height - pad - ((y - minY) / (maxY - minY)) * (height - pad * 2)
  const circles = points.map((point, index) => {
    const group = String(point.group).toLowerCase() === 'case' ? 'case' : 'control'
    const klass = group === 'case' ? 'case-point' : 'control-point'
    const x = sx(Number(point.x)).toFixed(2)
    const y = sy(Number(point.y)).toFixed(2)
    return `<circle class="${klass}" data-plot-group="${group}" data-point-index="${index}" data-x="${Number(point.x)}" data-y="${Number(point.y)}" data-sample-id="${escapeHtml(point.sample_id)}" cx="${x}" cy="${y}" r="4.2"><title>${escapeHtml(point.sample_id)} · ${escapeHtml(point.group)} · subject ${escapeHtml(point.subject)} · ${escapeHtml(pca.x_label)} ${formatNumber(point.x)} · ${escapeHtml(pca.y_label)} ${formatNumber(point.y)}</title></circle><text class="scatter-point-label" data-plot-group="${group}" data-point-label="${index}" x="${Number(x)+7}" y="${Number(y)-6}" style="display:none">${escapeHtml(point.sample_id)}</text>`
  }).join('')
  const clipId = `${plotId}-clip`
  return `<div class="plot-shell" data-interactive-plot="${plotId}" data-plot-kind="pca" data-width="${width}" data-height="${height}" data-pad="${pad}" data-min-x="${minX}" data-max-x="${maxX}" data-min-y="${minY}" data-max-y="${maxY}">
    ${plotControls(plotId)}
    <div class="plot-scroll interactive-plot-scroll coordinate-plot"><svg class="science-plot" viewBox="0 0 ${width} ${height}" role="img" aria-label="${escapeHtml(pca.name)}">
      <defs><clipPath id="${clipId}"><rect x="${pad}" y="${pad}" width="${width-pad*2}" height="${height-pad*2}"/></clipPath></defs>
      <g class="pca-grid"></g><g class="pca-axes"></g><g class="pca-points" clip-path="url(#${clipId})">${circles}</g>
      <text class="plot-label" x="${width/2}" y="${height-10}" text-anchor="middle">${escapeHtml(pca.x_label)}</text>
      <text class="plot-label" x="16" y="${height/2}" text-anchor="middle" transform="rotate(-90 16 ${height/2})">${escapeHtml(pca.y_label)}</text>
    </svg></div>
    ${plotLegend(plotId, caseLabel, controlLabel)}
  </div>`
}

function volcanoSvg(analysis, plotId) {
  const rows = (analysis?.tests || []).filter((row) => Number.isFinite(Number(row.beta_group)) && Number.isFinite(Number(row.q_value)) && Number(row.q_value) >= 0)
  if (!rows.length) return '<div class="plot-empty">Volcano plot is not available for this result.</div>'
  const width = 760
  const height = 360
  const pad = 54
  const fdr = Math.min(1, Math.max(1e-12, Number(state.globalThresholds.fdr) || 0.05))
  const fc = Math.max(0, Number(state.globalThresholds.absLog2Fc) || 0)
  const points = rows.map((row) => ({
    row,
    x: Number(row.beta_group),
    y: -Math.log10(Math.max(Number(row.q_value), 1e-300)),
  }))
  let lowX = -fc
  let highX = fc
  let highY = -Math.log10(fdr)
  for (const point of points) {
    if (point.x < lowX) lowX = point.x
    if (point.x > highX) highX = point.x
    if (point.y > highY) highY = point.y
  }
  const [rawMinX, rawMaxX] = paddedBounds(lowX, highX)
  const minX = Math.min(rawMinX, -Math.max(0.5, fc * 1.15))
  const maxX = Math.max(rawMaxX, Math.max(0.5, fc * 1.15))
  const minY = 0
  const maxY = Math.max(1, paddedBounds(0, highY)[1])
  const sx = (x) => pad + ((x - minX) / (maxX - minX)) * (width - pad * 2)
  const sy = (y) => height - pad - ((y - minY) / (maxY - minY)) * (height - pad * 2)
  const displayedPoints = thinPlotPoints(points, state.resources.maxPlotPoints)
  const marks = displayedPoints.map((point, index) => {
    const significant = point.row.q_value <= fdr && Math.abs(point.row.beta_group) >= fc
    const regulation = significant ? (point.row.beta_group >= 0 ? 'up' : 'down') : 'neutral'
    const klass = regulation === 'up' ? 'volcano-up' : regulation === 'down' ? 'volcano-down' : 'volcano-neutral'
    const x = sx(point.x).toFixed(2)
    const y = sy(point.y).toFixed(2)
    return `<circle class="${klass}" data-point-index="${index}" data-x="${point.x}" data-y="${point.y}" data-sample-id="${escapeHtml(point.row.feature)}" cx="${x}" cy="${y}" r="3.2"><title>${escapeHtml(point.row.feature)} · log2FC ${formatNumber(point.x)} · q ${formatNumber(point.row.q_value)}</title></circle><text class="scatter-point-label" data-point-label="${index}" x="${Number(x)+7}" y="${Number(y)-6}" style="display:none">${escapeHtml(point.row.feature)}</text>`
  }).join('')
  const thresholdY = -Math.log10(fdr)
  const clipId = `${plotId}-clip`
  return `<div class="plot-shell" data-interactive-plot="${plotId}" data-plot-kind="pca" data-width="${width}" data-height="${height}" data-pad="${pad}" data-min-x="${minX}" data-max-x="${maxX}" data-min-y="${minY}" data-max-y="${maxY}" data-volcano-fdr-y="${thresholdY}" data-volcano-fc="${fc}">
    ${plotControls(plotId)}
    <div class="plot-scroll interactive-plot-scroll coordinate-plot"><svg class="science-plot" viewBox="0 0 ${width} ${height}" role="img" aria-label="Differential expression volcano plot">
      <defs><clipPath id="${clipId}"><rect x="${pad}" y="${pad}" width="${width-pad*2}" height="${height-pad*2}"/></clipPath></defs>
      <g class="pca-grid"></g><g class="pca-axes"></g><g class="volcano-thresholds" clip-path="url(#${clipId})"></g><g class="pca-points" clip-path="url(#${clipId})">${marks}</g>
      <text class="plot-label" x="${width/2}" y="${height-10}" text-anchor="middle">Model log2 fold change</text>
      <text class="plot-label" x="16" y="${height/2}" text-anchor="middle" transform="rotate(-90 16 ${height/2})">−log10(BH q-value)</text>
    </svg></div>
    ${points.length > displayedPoints.length ? `<p class="table-note">Showing ${displayedPoints.length.toLocaleString()} of ${points.length.toLocaleString()} points to limit browser memory. All tested features remain in the searchable table and export.</p>` : ''}
    <div class="plot-legend" aria-label="Volcano classification"><span><i class="legend-dot volcano-up"></i>Up-regulated</span><span><i class="legend-dot volcano-down"></i>Down-regulated</span><span><i class="legend-dot volcano-neutral"></i>Below threshold</span></div>
  </div>`
}

function globalThresholdControls() {
  return `<section class="card threshold-card"><div class="card-heading"><div><h3>Differential result thresholds</h3><p>Global display/post-selection thresholds. Changing these values does not refit the count model.</p></div><span class="tag neutral">Global</span></div><div class="form-grid threshold-grid"><label><span>FDR (BH q-value)</span><input id="global-fdr-threshold" type="number" min="0.000001" max="1" step="0.001" value="${state.globalThresholds.fdr}"/></label><label><span>Minimum |log2 fold change|</span><input id="global-log2fc-threshold" type="number" min="0" step="0.1" value="${state.globalThresholds.absLog2Fc}"/></label></div></section>`
}

function dendrogramSvg(tree, plotId, caseLabel, controlLabel) {
  const leaves = tree?.leaves || []
  const segments = tree?.segments || []
  if (!leaves.length || !segments.length) return '<div class="plot-empty">Hierarchical clustering is not available for this result.</div>'
  const width = 860
  const height = Math.max(320, Math.min(2400, 90 + leaves.length * 6))
  const left = 118; const right = 28; const top = 28; const bottom = 28
  const maxDistance = Math.max(1e-12, ...segments.flatMap((segment) => [Number(segment.x1), Number(segment.x2)]).filter(Number.isFinite))
  const maxPosition = Math.max(1, ...leaves.map((leaf) => Number(leaf.position)).filter(Number.isFinite))
  const sx = (x) => left + (Number(x) / maxDistance) * (width - left - right)
  const sy = (y) => top + (Number(y) / maxPosition) * (height - top - bottom)
  const lines = segments.map((segment) => `<line class="dendro-line" x1="${sx(segment.x1).toFixed(2)}" y1="${sy(segment.y1).toFixed(2)}" x2="${sx(segment.x2).toFixed(2)}" y2="${sy(segment.y2).toFixed(2)}"/>`).join('')
  const leafMarks = leaves.map((leaf) => {
    const group = String(leaf.group).toLowerCase() === 'case' ? 'case' : 'control'
    const klass = group === 'case' ? 'case-point' : 'control-point'
    const y = sy(leaf.position)
    return `<text class="dendro-label" data-plot-group="${group}" x="${left-10}" y="${(y+3).toFixed(2)}" text-anchor="end">${escapeHtml(leaf.sample_id)}</text><circle class="${klass}" data-plot-group="${group}" cx="${left}" cy="${y.toFixed(2)}" r="2.7"><title>${escapeHtml(leaf.sample_id)} · ${escapeHtml(leaf.group)} · subject ${escapeHtml(leaf.subject)}</title></circle>`
  }).join('')
  return `<div class="plot-shell" data-interactive-plot="${plotId}" data-plot-kind="dendrogram" data-width="${width}" data-height="${height}">
    ${plotControls(plotId)}
    <div class="plot-scroll dendro-scroll interactive-plot-scroll coordinate-plot"><svg class="science-plot dendrogram" viewBox="0 0 ${width} ${height}" role="img" aria-label="${escapeHtml(tree.name)}">${lines}${leafMarks}<text class="plot-label" x="${(left+width-right)/2}" y="${height-6}" text-anchor="middle">${escapeHtml(tree.distance_label)} · ${escapeHtml(tree.linkage)}</text></svg></div>
    ${plotLegend(plotId, caseLabel, controlLabel)}
  </div>`
}

function pcaInitialDomain(shell) {
  return {
    minX: Number(shell.dataset.minX), maxX: Number(shell.dataset.maxX),
    minY: Number(shell.dataset.minY), maxY: Number(shell.dataset.maxY),
  }
}

function renderPcaDomain(shell, domain) {
  const svg = shell.querySelector('.science-plot')
  if (!svg) return
  const width = Number(shell.dataset.width)
  const height = Number(shell.dataset.height)
  const pad = Number(shell.dataset.pad)
  const plotWidth = width - pad * 2
  const plotHeight = height - pad * 2
  const sx = (x) => pad + ((x - domain.minX) / (domain.maxX - domain.minX)) * plotWidth
  const sy = (y) => height - pad - ((y - domain.minY) / (domain.maxY - domain.minY)) * plotHeight
  const xTicks = tickValues(domain.minX, domain.maxX)
  const yTicks = tickValues(domain.minY, domain.maxY)
  const grid = svg.querySelector('.pca-grid')
  const axes = svg.querySelector('.pca-axes')
  if (grid) grid.innerHTML = [
    ...xTicks.map((tick) => `<line class="plot-grid-line" x1="${sx(tick).toFixed(2)}" y1="${pad}" x2="${sx(tick).toFixed(2)}" y2="${height-pad}"/>`),
    ...yTicks.map((tick) => `<line class="plot-grid-line" x1="${pad}" y1="${sy(tick).toFixed(2)}" x2="${width-pad}" y2="${sy(tick).toFixed(2)}"/>`),
  ].join('')
  if (axes) axes.innerHTML = `
    <line class="plot-axis" x1="${pad}" y1="${height-pad}" x2="${width-pad}" y2="${height-pad}"/>
    <line class="plot-axis" x1="${pad}" y1="${pad}" x2="${pad}" y2="${height-pad}"/>
    ${xTicks.map((tick) => `<text class="plot-tick" x="${sx(tick).toFixed(2)}" y="${height-pad+17}" text-anchor="middle">${escapeHtml(formatNumber(tick, 2))}</text>`).join('')}
    ${yTicks.map((tick) => `<text class="plot-tick" x="${pad-8}" y="${(sy(tick)+3).toFixed(2)}" text-anchor="end">${escapeHtml(formatNumber(tick, 2))}</text>`).join('')}`
  const visiblePoints = []
  svg.querySelectorAll('.pca-points circle').forEach((point) => {
    const x = Number(point.dataset.x)
    const y = Number(point.dataset.y)
    const px = sx(x)
    const py = sy(y)
    point.setAttribute('cx', px.toFixed(2))
    point.setAttribute('cy', py.toFixed(2))
    const inside = px >= pad && px <= width - pad && py >= pad && py <= height - pad
    if (inside && point.style.display !== 'none') visiblePoints.push({ point, px, py })
  })
  const initial = pcaInitialDomain(shell)
  const zoomX = (initial.maxX - initial.minX) / (domain.maxX - domain.minX)
  const zoomY = (initial.maxY - initial.minY) / (domain.maxY - domain.minY)
  const labelZoom = Math.max(zoomX, zoomY)
  const labelCandidates = []
  if (labelZoom >= 1.45) {
    for (const candidate of visiblePoints) {
      let nearest = Infinity
      for (const other of visiblePoints) {
        if (other === candidate) continue
        nearest = Math.min(nearest, Math.hypot(candidate.px - other.px, candidate.py - other.py))
      }
      if (visiblePoints.length <= 18 || nearest >= 28) labelCandidates.push(candidate)
    }
  }
  const accepted = []
  for (const candidate of labelCandidates.slice(0, 80)) {
    if (accepted.every((other) => Math.hypot(candidate.px - other.px, candidate.py - other.py) >= 34)) accepted.push(candidate)
  }
  svg.querySelectorAll('.scatter-point-label').forEach((label) => { label.style.display = 'none' })
  for (const candidate of accepted) {
    const label = svg.querySelector(`[data-point-label="${candidate.point.dataset.pointIndex}"]`)
    const toggle = shell.querySelector(`[data-plot-group-toggle="${candidate.point.dataset.plotGroup}"]`)
    if (!label || toggle?.getAttribute('aria-pressed') === 'false') continue
    label.setAttribute('x', (candidate.px + 7).toFixed(2))
    label.setAttribute('y', (candidate.py - 6).toFixed(2))
    label.style.display = ''
  }
  const thresholdLayer = svg.querySelector('.volcano-thresholds')
  if (thresholdLayer) {
    const fdrY = Number(shell.dataset.volcanoFdrY)
    const fc = Number(shell.dataset.volcanoFc)
    const lines = []
    if (Number.isFinite(fdrY) && fdrY >= domain.minY && fdrY <= domain.maxY) lines.push(`<line class="volcano-cutoff-line" x1="${pad}" y1="${sy(fdrY).toFixed(2)}" x2="${width-pad}" y2="${sy(fdrY).toFixed(2)}"/>`)
    for (const x of [-fc, fc]) if (Number.isFinite(x) && x >= domain.minX && x <= domain.maxX) lines.push(`<line class="volcano-cutoff-line" x1="${sx(x).toFixed(2)}" y1="${pad}" x2="${sx(x).toFixed(2)}" y2="${height-pad}"/>`)
    thresholdLayer.innerHTML = lines.join('')
  }
  shell._plotDomain = domain
  const initialWidth = initial.maxX - initial.minX
  const zoom = initialWidth / (domain.maxX - domain.minX)
  shell.querySelectorAll('[data-plot-action="fit"]').forEach((button) => { button.textContent = Math.abs(zoom - 1) < 0.02 ? 'Fit' : `${Math.round(zoom * 100)}%` })
}

function pcaZoom(shell, factor, anchorX = 0.5, anchorY = 0.5) {
  const domain = shell._plotDomain || pcaInitialDomain(shell)
  const initial = pcaInitialDomain(shell)
  const minRangeX = (initial.maxX - initial.minX) / 50
  const minRangeY = (initial.maxY - initial.minY) / 50
  const maxRangeX = (initial.maxX - initial.minX) * 4
  const maxRangeY = (initial.maxY - initial.minY) * 4
  const rangeX = Math.min(maxRangeX, Math.max(minRangeX, (domain.maxX - domain.minX) * factor))
  const rangeY = Math.min(maxRangeY, Math.max(minRangeY, (domain.maxY - domain.minY) * factor))
  const anchorDataX = domain.minX + (domain.maxX - domain.minX) * anchorX
  const anchorDataY = domain.maxY - (domain.maxY - domain.minY) * anchorY
  renderPcaDomain(shell, {
    minX: anchorDataX - rangeX * anchorX,
    maxX: anchorDataX + rangeX * (1 - anchorX),
    minY: anchorDataY - rangeY * (1 - anchorY),
    maxY: anchorDataY + rangeY * anchorY,
  })
}

function pcaPan(shell, dx, dy) {
  const domain = shell._plotDomain || pcaInitialDomain(shell)
  const width = Number(shell.dataset.width) - Number(shell.dataset.pad) * 2
  const height = Number(shell.dataset.height) - Number(shell.dataset.pad) * 2
  const shiftX = -(dx / width) * (domain.maxX - domain.minX)
  const shiftY = (dy / height) * (domain.maxY - domain.minY)
  renderPcaDomain(shell, {
    minX: domain.minX + shiftX, maxX: domain.maxX + shiftX,
    minY: domain.minY + shiftY, maxY: domain.maxY + shiftY,
  })
}

function dendrogramInitialViewBox(shell) {
  return { x: 0, y: 0, width: Number(shell.dataset.width), height: Number(shell.dataset.height) }
}

function renderDendrogramViewBox(shell, viewBox) {
  const svg = shell.querySelector('.science-plot')
  if (!svg) return
  svg.setAttribute('viewBox', `${viewBox.x} ${viewBox.y} ${viewBox.width} ${viewBox.height}`)
  shell._plotViewBox = viewBox
  const initial = dendrogramInitialViewBox(shell)
  const zoom = initial.width / viewBox.width
  shell.querySelectorAll('[data-plot-action="fit"]').forEach((button) => { button.textContent = Math.abs(zoom - 1) < 0.02 ? 'Fit' : `${Math.round(zoom * 100)}%` })
}

function dendrogramZoom(shell, factor, anchorX = 0.5, anchorY = 0.5) {
  const current = shell._plotViewBox || dendrogramInitialViewBox(shell)
  const initial = dendrogramInitialViewBox(shell)
  const width = Math.min(initial.width * 2, Math.max(initial.width / 20, current.width * factor))
  const height = Math.min(initial.height * 2, Math.max(initial.height / 20, current.height * factor))
  const anchorSvgX = current.x + current.width * anchorX
  const anchorSvgY = current.y + current.height * anchorY
  renderDendrogramViewBox(shell, {
    x: anchorSvgX - width * anchorX,
    y: anchorSvgY - height * anchorY,
    width,
    height,
  })
}

function dendrogramPan(shell, dx, dy, viewport) {
  const current = shell._plotViewBox || dendrogramInitialViewBox(shell)
  const shiftX = -(dx / Math.max(1, viewport.clientWidth)) * current.width
  const shiftY = -(dy / Math.max(1, viewport.clientHeight)) * current.height
  renderDendrogramViewBox(shell, { ...current, x: current.x + shiftX, y: current.y + shiftY })
}

function fitPlot(shell) {
  if (shell.dataset.plotKind === 'pca') renderPcaDomain(shell, pcaInitialDomain(shell))
  else renderDendrogramViewBox(shell, dendrogramInitialViewBox(shell))
}

function bindInteractivePlots() {
  document.querySelectorAll('[data-interactive-plot]').forEach((shell) => {
    const viewport = shell.querySelector('.interactive-plot-scroll')
    const svg = shell.querySelector('.science-plot')
    if (!viewport || !svg) return
    fitPlot(shell)

    shell.querySelectorAll('[data-plot-action]').forEach((button) => {
      button.addEventListener('click', () => {
        const action = button.dataset.plotAction
        if (action === 'fit') fitPlot(shell)
        else if (shell.dataset.plotKind === 'pca') pcaZoom(shell, action === 'in' ? 0.8 : 1.25)
        else dendrogramZoom(shell, action === 'in' ? 0.8 : 1.25)
      })
    })

    shell.querySelectorAll('[data-plot-group-toggle]').forEach((button) => {
      button.addEventListener('click', () => {
        const group = button.dataset.plotGroupToggle
        const visible = button.getAttribute('aria-pressed') !== 'false'
        button.setAttribute('aria-pressed', visible ? 'false' : 'true')
        button.classList.toggle('is-hidden', visible)
        shell.querySelectorAll(`[data-plot-group="${group}"]`).forEach((mark) => { mark.style.display = visible ? 'none' : '' })
      })
    })

    viewport.addEventListener('wheel', (event) => {
      if (!event.ctrlKey && !event.metaKey) return
      event.preventDefault()
      const rect = svg.getBoundingClientRect()
      const anchorX = Math.min(1, Math.max(0, (event.clientX - rect.left) / Math.max(1, rect.width)))
      const anchorY = Math.min(1, Math.max(0, (event.clientY - rect.top) / Math.max(1, rect.height)))
      const factor = event.deltaY < 0 ? 0.88 : 1.14
      if (shell.dataset.plotKind === 'pca') pcaZoom(shell, factor, anchorX, anchorY)
      else dendrogramZoom(shell, factor, anchorX, anchorY)
    }, { passive: false })

    let drag = null
    viewport.addEventListener('pointerdown', (event) => {
      if (event.button !== 0) return
      drag = { x: event.clientX, y: event.clientY }
      viewport.setPointerCapture?.(event.pointerId)
      viewport.classList.add('is-dragging')
    })
    viewport.addEventListener('pointermove', (event) => {
      if (!drag) return
      const dx = event.clientX - drag.x
      const dy = event.clientY - drag.y
      drag = { x: event.clientX, y: event.clientY }
      if (shell.dataset.plotKind === 'pca') pcaPan(shell, dx, dy)
      else dendrogramPan(shell, dx, dy, viewport)
    })
    const stopDrag = () => { drag = null; viewport.classList.remove('is-dragging') }
    viewport.addEventListener('pointerup', stopDrag)
    viewport.addEventListener('pointercancel', stopDrag)
  })
}

function differentialRowCells(row) {
  return [
    row.feature,
    row.secondary_id || '',
    row.mean_case,
    row.mean_control,
    row.beta_group,
    row.robust_se,
    row.q_value,
    row.subject_effect,
    row.subject_q_value,
  ]
}

function differentialRowHtml(row) {
  return `<tr><td><strong>${escapeHtml(row.feature)}</strong></td><td>${escapeHtml(row.secondary_id || '')}</td><td>${formatNumber(row.mean_case)}</td><td>${formatNumber(row.mean_control)}</td><td>${formatNumber(row.beta_group)}</td><td>${formatNumber(row.robust_se)}</td><td>${formatNumber(row.q_value)}</td><td>${formatNumber(row.subject_effect)}</td><td>${formatNumber(row.subject_q_value)}</td></tr>`
}

function differentialTable(analysis, tableId) {
  const rows = analysis?.tests || []
  if (!rows.length) return '<div class="plot-empty">No tested features are available.</div>'
  dynamicTableSources.set(tableId, { rows, cells: differentialRowCells, rowHtml: differentialRowHtml, limit: 500 })
  const initial = rows.slice(0, 500)
  return `<div class="table-scroll differential-table" data-dynamic-table="${escapeHtml(tableId)}"><table><thead><tr><th>Feature</th><th>Secondary ID</th><th>Case mean</th><th>Control mean</th><th>log2 group effect</th><th>SE</th><th>q-value</th><th>Subject effect</th><th>Subject q</th></tr></thead><tbody>${initial.map(differentialRowHtml).join('')}</tbody></table></div><p class="table-note">Search and sort operate across all ${rows.length.toLocaleString()} tested features. Up to 500 matching rows are drawn at once; the JSON export contains all rows.</p>`
}

function dtuGeneRowCells(row) {
  return [row.gene_id, row.n_transcripts, row.q_value, row.max_abs_clr_effect]
}

function dtuGeneRowHtml(row) {
  return `<tr><td><strong>${escapeHtml(row.gene_id)}</strong></td><td>${row.n_transcripts}</td><td>${formatNumber(row.q_value)}</td><td>${formatNumber(row.max_abs_clr_effect)}</td></tr>`
}

function dtuGeneTable(dtu, tableId) {
  const rows = dtu?.gene_tests || []
  if (!rows.length) return '<div class="plot-empty">No transcript-usage gene tests are available.</div>'
  dynamicTableSources.set(tableId, { rows, cells: dtuGeneRowCells, rowHtml: dtuGeneRowHtml, limit: 500 })
  return `<div class="table-scroll" data-dynamic-table="${escapeHtml(tableId)}"><table><thead><tr><th>Gene</th><th>Transcripts</th><th>q-value</th><th>Max |CLR effect|</th></tr></thead><tbody>${rows.slice(0, 500).map(dtuGeneRowHtml).join('')}</tbody></table></div><p class="table-note">Search and sort operate across all ${rows.length.toLocaleString()} tested genes. Up to 500 matching rows are drawn at once.</p>`
}

function comparisonResultSection(title, result) {
  if (!result?.analysis) return ''
  const analysis = result.analysis
  const plotPrefix = title.toLowerCase().startsWith('gene') ? 'gene' : 'transcript'
  const caseLabel = result.comparison?.case_label || 'Case'
  const controlLabel = result.comparison?.control_label || 'Control'
  const significant = (analysis.tests || []).filter((row) => Number.isFinite(Number(row.q_value)) && Number(row.q_value) <= state.globalThresholds.fdr && Math.abs(Number(row.beta_group)) >= state.globalThresholds.absLog2Fc).length
  return `<section class="card comparison-result">
    <div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>${escapeHtml(result.source_metric)} · corrected native/WASM-parity expression model</p></div><span class="tag success">Rust/WASM</span></div>
    <div class="summary-grid compact-grid">
      ${summaryCard('Model samples', analysis.n_model_samples, `${analysis.n_subjects} subject/patient IDs`)}
      ${summaryCard('Tested features', analysis.tested_features.toLocaleString(), `${analysis.filtered_features.toLocaleString()} filtered before modeling`)}
      ${summaryCard(`q ≤ ${formatNumber(state.globalThresholds.fdr)} & |log2FC| ≥ ${formatNumber(state.globalThresholds.absLog2Fc)}`, significant.toLocaleString(), 'Global differential-result thresholds')}
      ${summaryCard('Excluded rows', result.comparison.excluded_rows.toLocaleString(), `${result.comparison.metadata_rows_after_exclusions} metadata rows retained`)}
    </div>
    ${analysis.warnings?.length ? `<div class="analysis-warnings">${analysis.warnings.map((warning) => `<div>${escapeHtml(warning)}</div>`).join('')}</div>` : ''}
    <article class="plot-card volcano-card"><div class="plot-heading"><strong>Differential-expression volcano plot</strong><span>Model log2 fold change versus −log10(BH q-value). Dashed lines show the global FDR and |log2FC| thresholds.</span></div>${volcanoSvg(analysis, `${plotPrefix}-volcano`)}</article>
    <div class="plot-grid"><article class="plot-card"><div class="plot-heading"><strong>Global expression PCA</strong><span>Top variable normalized features; groups do not select the PCA features.</span></div>${pcaSvg(analysis.pca, `${plotPrefix}-pca`, caseLabel, controlLabel)}</article><article class="plot-card"><div class="plot-heading"><strong>Hierarchical clustering</strong><span>Average linkage on Euclidean distance across the same normalized feature space.</span></div>${dendrogramSvg(analysis.dendrogram, `${plotPrefix}-dendrogram`, caseLabel, controlLabel)}</article></div>
    <details class="details-block result-details" open><summary>Differential feature table</summary>${differentialTable(analysis, `${plotPrefix}-differential-table`)}</details>
    ${analysis.dtu ? `<details class="details-block result-details"><summary>Transcript-usage screen (${analysis.dtu.genes_tested.toLocaleString()} genes)</summary><p class="method-note">${escapeHtml(analysis.dtu.method_description)}</p>${dtuGeneTable(analysis.dtu, `${plotPrefix}-dtu-gene-table`)}</details>` : ''}
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
  const incompleteExclusion = cfg.qcExclusionEnabled && !cfg.qcExclusionColumn
    ? 'Reviewed QC exclusion is enabled. Select a metadata column and at least one value, or turn it off.'
    : cfg.qcExclusionEnabled && !cfg.qcExclusionValues.length
      ? 'Reviewed QC exclusion is enabled. Select at least one value to exclude, or turn it off.'
      : cfg.customExclusionEnabled && !cfg.customExclusionColumn
        ? 'Custom metadata exclusion is enabled. Select a metadata column and at least one value, or turn it off.'
        : cfg.customExclusionEnabled && !cfg.customExclusionValues.length
          ? 'Custom metadata exclusion is enabled. Select at least one value to exclude, or turn it off.'
          : ''
  const runnable = state.wasmReady && !state.busy && metadataReady && Boolean(cfg.groupColumn) && cfg.caseValues.length > 0 && cfg.controlValues.length > 0 && !overlap && modalityReady && exclusionReady

  return `<section class="page-heading"><div><span class="eyebrow">Compare Groups · W3</span><h2>Define a cohort comparison from metadata</h2><p>Select the metadata roles and group values that match this cohort. 4C Insight runs the established corrected count model, PCA, and hierarchical clustering inside the local Rust/WASM worker.</p></div><div class="local-card"><strong>Same science, local bytes</strong><span>The native and byte/WASM paths share the corrected-expression implementation.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  ${!metadataRecords.length ? '<div class="notice-banner"><strong>Metadata required.</strong><span>Select a metadata file in the Project tab before defining groups.</span></div>' : ''}
  ${metadata && (!metadataReady) ? `<div class="notice-banner"><strong>Resolve metadata IDs first.</strong><span>The selected ID column has ${metadata.blank_id_rows} blank row(s) and ${metadata.duplicate_id_values.length} duplicate value(s).</span></div>` : ''}
  ${overlap ? '<div class="notice-banner"><strong>Case and control selections overlap.</strong><span>Remove the same value from one group before running the comparison.</span></div>' : ''}
  ${incompleteExclusion ? `<div class="notice-banner"><strong>Finish or disable the optional exclusion.</strong><span>${escapeHtml(incompleteExclusion)}</span></div>` : ''}
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
  ${state.compareResult ? `${globalThresholdControls()}<div class="result-heading"><div><span class="eyebrow">Corrected expression result</span><h3>${escapeHtml(state.compareResult.comparisonName)}</h3><p>Generated entirely on this device. PCA and clustering use global top-variable normalized features, not significant-hit selection.</p></div><button id="export-comparison" class="secondary-button">Save comparison JSON</button></div>${comparisonResultSection('Gene expression', state.compareResult.gene)}${comparisonResultSection('Transcript expression', state.compareResult.transcript)}` : ''}`
}


function ensureLongitudinalDefaults() {
  const metadata = state.metadataInspection
  const cfg = state.longitudinalConfig
  const columns = metadata?.columns || []
  if (!columns.length) return

  if (!cfg.observationIdColumn || !columns.includes(cfg.observationIdColumn)) {
    cfg.observationIdColumn = columns.includes(state.qcConfig.metadataIdColumn)
      ? state.qcConfig.metadataIdColumn
      : (metadata.id_column || columns[0] || '')
  }
  if (!cfg.patientIdColumn || !columns.includes(cfg.patientIdColumn) || cfg.patientIdColumn === cfg.observationIdColumn) {
    cfg.patientIdColumn = ['patient_id', 'sample_id', 'subject_id', 'participant_id']
      .find((column) => column !== cfg.observationIdColumn && columns.includes(column))
      || columns.find((column) => column !== cfg.observationIdColumn)
      || ''
  }
  if (cfg.timeColumn && (!columns.includes(cfg.timeColumn) || cfg.timeColumn === cfg.observationIdColumn || cfg.timeColumn === cfg.patientIdColumn)) cfg.timeColumn = ''
  if (!cfg.timeColumn) {
    cfg.timeColumn = ['Collection_Date_(mm-dd-yy)', 'Collection_Date', 'collection_date', 'timepoint', 'visit', 'visit_date']
      .find((column) => columns.includes(column) && column !== cfg.observationIdColumn && column !== cfg.patientIdColumn)
      || columns.find((column) => column !== cfg.observationIdColumn && column !== cfg.patientIdColumn && /date|timepoint|visit/i.test(column))
      || ''
  }
  if (cfg.exclusionColumn && !columns.includes(cfg.exclusionColumn)) {
    cfg.exclusionColumn = ''
    cfg.exclusionValues = []
  }
  if (!cfg.exclusionColumn) {
    cfg.exclusionColumn = ['qc_review_status', 'qc_status', 'review_status', 'status']
      .find((column) => columns.includes(column)) || ''
  }

  const geneMerged = recordsOfKind('merged-gene-expression')
  const transcriptMerged = recordsOfKind('merged-transcript-expression')
  if (!geneMerged.some((record) => record.id === cfg.geneMergedId)) cfg.geneMergedId = geneMerged[0]?.id || ''
  if (!transcriptMerged.some((record) => record.id === cfg.transcriptMergedId)) cfg.transcriptMergedId = transcriptMerged[0]?.id || ''
}

function patientColor(patient) {
  let hash = 0
  for (const char of String(patient || '')) hash = ((hash * 31) + char.charCodeAt(0)) >>> 0
  return `hsl(${hash % 360} 58% 44%)`
}

function longitudinalDendrogramSvg(tree, annotations) {
  const leaves = tree?.leaves || []
  const segments = tree?.segments || []
  if (!leaves.length || !segments.length) return '<div class="plot-empty">Hierarchical clustering is not available for this result.</div>'
  const annotationMap = new Map((annotations || []).map((row) => [String(row.sample_id), row]))
  const width = 1080
  const height = Math.max(360, 70 + leaves.length * 18)
  const left = 300; const right = 30; const top = 28; const bottom = 30
  const maxDistance = Math.max(1e-12, ...segments.flatMap((segment) => [Number(segment.x1), Number(segment.x2)]).filter(Number.isFinite))
  const maxPosition = Math.max(1, ...leaves.map((leaf) => Number(leaf.position)).filter(Number.isFinite))
  const sx = (x) => left + (Number(x) / maxDistance) * (width - left - right)
  const sy = (y) => top + (Number(y) / maxPosition) * (height - top - bottom)
  const lines = segments.map((segment) => `<line class="dendro-line" x1="${sx(segment.x1).toFixed(2)}" y1="${sy(segment.y1).toFixed(2)}" x2="${sx(segment.x2).toFixed(2)}" y2="${sy(segment.y2).toFixed(2)}"/>`).join('')
  const leafMarks = leaves.map((leaf) => {
    const annotation = annotationMap.get(String(leaf.sample_id)) || {}
    const patient = annotation.patient_id || leaf.subject || 'Unknown patient'
    const time = annotation.time_value || leaf.group || ''
    const label = [leaf.sample_id, patient, time].filter(Boolean).join(' · ')
    const y = sy(leaf.position)
    const color = patientColor(patient)
    return `<text class="dendro-label longitudinal-leaf-label" x="${left-14}" y="${(y+3).toFixed(2)}" text-anchor="end">${escapeHtml(label)}</text><circle cx="${left}" cy="${y.toFixed(2)}" r="4" style="fill:${color};stroke:#fff;stroke-width:1"><title>${escapeHtml(label)}</title></circle>`
  }).join('')
  return `<div class="longitudinal-tree-note">Colored dot = patient identity · leaf text = data sample · patient · time/visit</div><div class="plot-scroll longitudinal-dendrogram-scroll"><svg class="science-plot longitudinal-dendrogram" viewBox="0 0 ${width} ${height}" role="img" aria-label="${escapeHtml(tree.name)}">${lines}${leafMarks}<text class="plot-label" x="${(left+width-right)/2}" y="${height-7}" text-anchor="middle">${escapeHtml(tree.distance_label)} · ${escapeHtml(tree.linkage)}</text></svg></div>`
}

function longitudinalRankingTable(result) {
  const rows = [['Gene expression', result?.gene], ['Transcript expression', result?.transcript]]
    .filter(([, entry]) => entry?.modality)
    .map(([label, entry]) => ({ label, modality: entry.modality }))
    .sort((a, b) => Number(b.modality.identity_auc || -1) - Number(a.modality.identity_auc || -1))
  if (!rows.length) return ''
  return `<section class="card"><div class="card-heading"><div><h3>Cross-modality patient-signature ranking</h3><p>Higher identity AUC and nearest-neighbour accuracy, together with a lower within/between distance ratio, indicate stronger patient-specific stability.</p></div><span class="tag success">Unsupervised</span></div><div class="table-scroll"><table><thead><tr><th>Data type</th><th>Samples</th><th>Longitudinal patients</th><th>Identity AUC</th><th>NN patient accuracy</th><th>Within / between distance</th></tr></thead><tbody>${rows.map(({ label, modality }) => `<tr><td><strong>${escapeHtml(label)}</strong></td><td>${modality.samples}</td><td>${modality.longitudinal_patients}</td><td>${formatNumber(modality.identity_auc)}</td><td>${Number.isFinite(Number(modality.nearest_neighbor_accuracy)) ? `${(Number(modality.nearest_neighbor_accuracy) * 100).toFixed(1)}%` : '—'}</td><td>${formatNumber(modality.within_between_ratio)}</td></tr>`).join('')}</tbody></table></div></section>`
}

function longitudinalResultSection(title, result) {
  if (!result?.modality) return ''
  const modality = result.modality
  const warnings = [...new Set([...(result.warnings || []), ...(modality.warnings || [])])]
  return `<section class="card comparison-result longitudinal-result"><div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>${escapeHtml(modality.distance_label)} · patient identity is not used to construct the distance matrix.</p></div><span class="tag success">Rust/WASM</span></div>
    <div class="summary-grid compact-grid">
      ${summaryCard('Samples', modality.samples, `${modality.patients} patient/person IDs`)}
      ${summaryCard('Longitudinal patients', modality.longitudinal_patients, `${modality.longitudinal_samples} samples from patients with ≥2 observations`)}
      ${summaryCard('Patient identity AUC', formatNumber(modality.identity_auc), '0.5 ≈ no ranking signal; 1.0 = same-patient pairs always closer')}
      ${summaryCard('Nearest-neighbour accuracy', Number.isFinite(Number(modality.nearest_neighbor_accuracy)) ? `${(Number(modality.nearest_neighbor_accuracy) * 100).toFixed(1)}%` : '—', 'Nearest other sample belongs to the same patient')}
      ${summaryCard('Median within-patient', formatNumber(modality.median_within_patient_distance), `${modality.within_patient_pairs} same-patient pairs`)}
      ${summaryCard('Median between-patient', formatNumber(modality.median_between_patient_distance), `${modality.between_patient_pairs} different-patient pairs`)}
      ${summaryCard('Within / between ratio', formatNumber(modality.within_between_ratio), 'Lower than 1 means repeated patient samples are closer')}
      ${summaryCard('QC-excluded rows', result.qc_excluded_rows, `${result.eligible_metadata_rows} metadata observations retained`)}
    </div>
    ${warnings.length ? `<div class="analysis-warnings">${warnings.map((warning) => `<div>${escapeHtml(warning)}</div>`).join('')}</div>` : ''}
    <article class="plot-card longitudinal-tree-card"><div class="plot-heading"><strong>Patient-specific hierarchical clustering</strong><span>Average linkage. Same-patient observations should appear close together when this modality preserves a stable patient signature.</span></div>${longitudinalDendrogramSvg(modality.dendrogram, modality.annotations)}</article>
    <details class="details-block result-details"><summary>Sample annotations (${modality.annotations.length})</summary><div class="table-scroll longitudinal-table"><table><thead><tr><th>Data sample</th><th>Metadata observation</th><th>Patient/person</th><th>Time / visit</th><th>QC value</th></tr></thead><tbody>${modality.annotations.slice(0, 500).map((row) => `<tr><td>${escapeHtml(row.sample_id)}</td><td>${escapeHtml(row.observation_id)}</td><td><strong>${escapeHtml(row.patient_id)}</strong></td><td>${escapeHtml(row.time_value || '—')}</td><td>${escapeHtml(row.qc_value || '—')}</td></tr>`).join('')}</tbody></table></div></details>
  </section>`
}

function longitudinalView() {
  ensureLongitudinalDefaults()
  const metadataRecords = recordsOfKind('metadata-candidate')
  const geneMerged = recordsOfKind('merged-gene-expression')
  const transcriptMerged = recordsOfKind('merged-transcript-expression')
  const metadata = state.metadataInspection
  const columns = metadata?.columns || []
  const cfg = state.longitudinalConfig
  const exclusionValues = cachedMetadataValues(cfg.exclusionColumn)
  const rolesReady = Boolean(metadata && cfg.observationIdColumn && cfg.patientIdColumn && cfg.observationIdColumn !== cfg.patientIdColumn)
  const modalityReady = (cfg.geneEnabled && cfg.geneMergedId) || (cfg.transcriptEnabled && cfg.transcriptMergedId)
  const exclusionReady = !cfg.exclusionEnabled || (Boolean(cfg.exclusionColumn) && cfg.exclusionValues.length > 0)
  const runnable = state.wasmReady && !state.busy && rolesReady && modalityReady && exclusionReady
  const incompleteExclusion = cfg.exclusionEnabled && !cfg.exclusionColumn
    ? 'The longitudinal exclusion is enabled. Select a metadata column and at least one value, or turn it off.'
    : cfg.exclusionEnabled && !cfg.exclusionValues.length
      ? 'The longitudinal exclusion is enabled. Select at least one value to exclude, or turn it off.'
      : ''

  return `<section class="page-heading"><div><span class="eyebrow">Longitudinal · W4</span><h2>Which data type preserves patient identity over time?</h2><p>Map the observation and patient identifiers explicitly. 4C Insight then clusters all eligible observations without using Case/Control labels and measures whether repeated samples from the same patient remain closer than the rest of the cohort.</p></div><div class="local-card"><strong>Unsupervised and local</strong><span>Patient identity is used only to score the finished distance matrix, not to construct it.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  ${!metadataRecords.length ? '<div class="notice-banner"><strong>Metadata required.</strong><span>Select the cohort files/folder in Project first.</span></div>' : ''}
  ${cfg.observationIdColumn && cfg.observationIdColumn === cfg.patientIdColumn ? '<div class="notice-banner"><strong>Choose two different identifier roles.</strong><span>The observation/sample ID identifies each longitudinal specimen. The patient/person ID must be the stable identifier shared across visits.</span></div>' : ''}
  ${incompleteExclusion ? `<div class="notice-banner"><strong>Finish or disable the optional exclusion.</strong><span>${escapeHtml(incompleteExclusion)}</span></div>` : ''}
  <section class="card"><div class="card-heading"><div><h3>1. Map longitudinal metadata roles</h3><p>Column names are not assumed. This supports sheets where, for example, subject_id is the longitudinal sample and sample_id is the patient.</p></div><span class="tag neutral">W4</span></div><div class="config-grid longitudinal-role-grid">
    <div class="config-block stacked"><label><span>Metadata file</span><select id="longitudinal-metadata-file">${metadataRecords.map((record) => `<option value="${escapeHtml(record.id)}" ${state.qcConfig.metadataId === record.id ? 'selected' : ''}>${escapeHtml(record.name)}</option>`).join('')}</select></label><label><span>Observation / longitudinal sample ID</span><select id="longitudinal-observation-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns, cfg.observationIdColumn) : '<option>Read metadata first</option>'}</select></label></div>
    <div class="config-block stacked"><label><span>Patient / person ID</span><select id="longitudinal-patient-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns.filter((column) => column !== cfg.observationIdColumn), cfg.patientIdColumn) : '<option>Read metadata first</option>'}</select></label><label><span>Collection date / timepoint / visit</span><select id="longitudinal-time-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns.filter((column) => column !== cfg.observationIdColumn && column !== cfg.patientIdColumn), cfg.timeColumn, true) : '<option>Read metadata first</option>'}</select></label></div>
  </div></section>
  <section class="card"><div class="card-heading"><div><h3>2. Optional metadata exclusion</h3><p>Use any metadata column/value rule, for example qc_review_status = Failed. Exclusion occurs before clustering.</p></div></div><div class="config-grid exclusion-grid"><div class="config-block stacked"><div class="config-heading"><label class="check-label"><input id="longitudinal-exclusion-enabled" type="checkbox" ${cfg.exclusionEnabled ? 'checked' : ''}/><span><strong>Exclude metadata rows</strong><small>Independent of automatic outlier analysis.</small></span></label><span class="tag ${cfg.exclusionEnabled ? 'warning' : 'neutral'}">${cfg.exclusionEnabled ? (exclusionReady ? 'Active' : 'Needs input') : 'Optional'}</span></div><label><span>Metadata column</span><select id="longitudinal-exclusion-column" ${cfg.exclusionEnabled ? '' : 'disabled'}>${columnOptions(columns, cfg.exclusionColumn, true)}</select></label><label><span>Exclude these values</span><select id="longitudinal-exclusion-values" class="multi-select" multiple size="5" ${cfg.exclusionEnabled && cfg.exclusionColumn ? '' : 'disabled'}>${exclusionValues.length ? metadataValueOptions(exclusionValues, cfg.exclusionValues) : '<option disabled>Choose a column to load values</option>'}</select></label></div></div></section>
  <section class="card"><div class="card-heading"><div><h3>3. Expression data types</h3><p>W4 uses the same merged count tables and normalization geometry already validated in W3.</p></div></div><div class="config-grid expression-source-grid">
    <div class="config-block stacked ${geneMerged.length ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="longitudinal-gene-enabled" type="checkbox" ${cfg.geneEnabled && geneMerged.length ? 'checked' : ''} ${geneMerged.length ? '' : 'disabled'}/><span><strong>Gene expression</strong><small>Raw Count · normalized log2 top-variable feature distance</small></span></label><span class="tag ${geneMerged.length ? 'success' : 'neutral'}">${geneMerged.length ? 'Available' : 'Unavailable'}</span></div><label><span>Merged gene table</span><select id="longitudinal-gene-file" ${cfg.geneEnabled && geneMerged.length ? '' : 'disabled'}>${mergedFileOptions(geneMerged, cfg.geneMergedId)}</select></label></div>
    <div class="config-block stacked ${transcriptMerged.length ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="longitudinal-transcript-enabled" type="checkbox" ${cfg.transcriptEnabled && transcriptMerged.length ? 'checked' : ''} ${transcriptMerged.length ? '' : 'disabled'}/><span><strong>Transcript / isoform expression</strong><small>Total Reads · normalized log2 top-variable feature distance</small></span></label><span class="tag ${transcriptMerged.length ? 'success' : 'neutral'}">${transcriptMerged.length ? 'Available' : 'Unavailable'}</span></div><label><span>Merged transcript table</span><select id="longitudinal-transcript-file" ${cfg.transcriptEnabled && transcriptMerged.length ? '' : 'disabled'}>${mergedFileOptions(transcriptMerged, cfg.transcriptMergedId)}</select></label></div>
  </div><details class="advanced-settings"><summary>Advanced feature-support settings</summary><div class="form-grid advanced-grid"><label><span>Minimum count</span><input id="longitudinal-min-count" type="number" min="0" step="1" value="${cfg.minCount}"/></label><label><span>Minimum fraction of samples</span><input id="longitudinal-min-fraction" type="number" min="0" max="1" step="0.01" value="${cfg.minFractionSamples}"/></label></div><p>Defaults match the established expression clustering support filter.</p></details>
  <div class="run-row"><div><strong>${cfg.observationIdColumn && cfg.patientIdColumn ? `${escapeHtml(cfg.observationIdColumn)} observations → ${escapeHtml(cfg.patientIdColumn)} patients` : 'Select observation and patient roles'}</strong><span>The same patient labels are used only after the unsupervised distance matrix is complete.</span></div><button id="run-longitudinal" class="primary-button" ${runnable ? '' : 'disabled'}>${state.busy ? 'Running…' : 'Run longitudinal analysis'}</button></div></section>
  ${state.longitudinalProgress ? `<div class="progress-card" role="status"><div class="progress-copy"><strong>${escapeHtml(state.longitudinalProgress.message)}</strong><span>${state.longitudinalProgress.completed} of ${state.longitudinalProgress.total} modality steps complete</span></div><progress max="${Math.max(1, state.longitudinalProgress.total)}" value="${state.longitudinalProgress.completed}"></progress></div>` : ''}
  ${state.longitudinalResult ? `<div class="result-heading"><div><span class="eyebrow">Longitudinal patient-signature result</span><h3>Patient identity over repeated observations</h3><p>Generated entirely on this device. Metrics compare same-patient against different-patient distances after unsupervised clustering.</p></div><button id="export-longitudinal" class="secondary-button">Save longitudinal JSON</button></div>${longitudinalRankingTable(state.longitudinalResult)}${longitudinalResultSection('Gene expression', state.longitudinalResult.gene)}${longitudinalResultSection('Transcript / isoform expression', state.longitudinalResult.transcript)}` : ''}`
}

async function runLongitudinalAnalysis() {
  const cfg = state.longitudinalConfig
  const metadataFile = selectedFile(state.qcConfig.metadataId)
  const geneFile = selectedFile(cfg.geneMergedId)
  const transcriptFile = selectedFile(cfg.transcriptMergedId)
  if (!metadataFile) {
    state.error = 'Select a metadata file before running longitudinal analysis.'
    render()
    return
  }
  const portableConfig = {
    observationIdColumn: cfg.observationIdColumn,
    patientIdColumn: cfg.patientIdColumn,
    timeColumn: cfg.timeColumn || null,
    exclusionEnabled: cfg.exclusionEnabled,
    exclusionColumn: cfg.exclusionEnabled ? (cfg.exclusionColumn || null) : null,
    exclusionValues: cfg.exclusionEnabled ? cfg.exclusionValues : [],
    minCount: Number(cfg.minCount),
    minFractionSamples: Number(cfg.minFractionSamples),
  }
  state.busy = true
  state.error = null
  state.longitudinalResult = null
  state.longitudinalProgress = { completed: 0, total: Number(Boolean(cfg.geneEnabled && geneFile)) + Number(Boolean(cfg.transcriptEnabled && transcriptFile)), message: 'Starting longitudinal analysis' }
  render()
  try {
    const response = await workerRequest('run-longitudinal-expression', {
      metadataFile,
      config: portableConfig,
      gene: { enabled: Boolean(cfg.geneEnabled && geneFile), file: cfg.geneEnabled ? geneFile : null },
      transcript: { enabled: Boolean(cfg.transcriptEnabled && transcriptFile), file: cfg.transcriptEnabled ? transcriptFile : null },
    }, (progress) => {
      state.longitudinalProgress = progress
      render()
    })
    state.longitudinalResult = response.result
    await recordHistory('longitudinal-patient-signature-expression', 'Longitudinal patient signature')
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    state.longitudinalProgress = null
    render()
  }
}

function bindLongitudinalEvents() {
  const cfg = state.longitudinalConfig
  const rerender = () => render()
  document.querySelector('#longitudinal-metadata-file')?.addEventListener('change', (event) => {
    state.qcConfig.metadataId = event.currentTarget.value
    state.qcConfig.metadataIdColumn = ''
    state.metadataInspection = null
    state.metadataInspectionFileId = null
    state.metadataValueCache.clear()
    cfg.observationIdColumn = ''
    cfg.patientIdColumn = ''
    cfg.timeColumn = ''
    cfg.exclusionColumn = ''
    cfg.exclusionValues = []
    state.longitudinalResult = null
    rerender()
  })
  document.querySelector('#longitudinal-observation-column')?.addEventListener('change', (event) => {
    cfg.observationIdColumn = event.currentTarget.value
    if (cfg.patientIdColumn === cfg.observationIdColumn) cfg.patientIdColumn = ''
    state.longitudinalResult = null
    rerender()
  })
  document.querySelector('#longitudinal-patient-column')?.addEventListener('change', (event) => {
    cfg.patientIdColumn = event.currentTarget.value
    state.longitudinalResult = null
    rerender()
  })
  document.querySelector('#longitudinal-time-column')?.addEventListener('change', (event) => {
    cfg.timeColumn = event.currentTarget.value
    state.longitudinalResult = null
  })
  document.querySelector('#longitudinal-exclusion-enabled')?.addEventListener('change', (event) => {
    cfg.exclusionEnabled = event.currentTarget.checked
    state.longitudinalResult = null
    rerender()
  })
  document.querySelector('#longitudinal-exclusion-column')?.addEventListener('change', (event) => {
    cfg.exclusionColumn = event.currentTarget.value
    cfg.exclusionValues = []
    state.longitudinalResult = null
    if (cfg.exclusionColumn) void loadMetadataValues(cfg.exclusionColumn).finally(rerender)
    else rerender()
  })
  document.querySelector('#longitudinal-exclusion-values')?.addEventListener('change', (event) => {
    cfg.exclusionValues = selectedValues(event.currentTarget)
    state.longitudinalResult = null
    rerender()
  })
  document.querySelector('#longitudinal-gene-enabled')?.addEventListener('change', (event) => { cfg.geneEnabled = event.currentTarget.checked; state.longitudinalResult = null; rerender() })
  document.querySelector('#longitudinal-gene-file')?.addEventListener('change', (event) => { cfg.geneMergedId = event.currentTarget.value; state.longitudinalResult = null })
  document.querySelector('#longitudinal-transcript-enabled')?.addEventListener('change', (event) => { cfg.transcriptEnabled = event.currentTarget.checked; state.longitudinalResult = null; rerender() })
  document.querySelector('#longitudinal-transcript-file')?.addEventListener('change', (event) => { cfg.transcriptMergedId = event.currentTarget.value; state.longitudinalResult = null })
  document.querySelector('#longitudinal-min-count')?.addEventListener('change', (event) => { cfg.minCount = Number(event.currentTarget.value); state.longitudinalResult = null })
  document.querySelector('#longitudinal-min-fraction')?.addEventListener('change', (event) => { cfg.minFractionSamples = Number(event.currentTarget.value); state.longitudinalResult = null })
  document.querySelector('#run-longitudinal')?.addEventListener('click', () => void runLongitudinalAnalysis())
  document.querySelector('#export-longitudinal')?.addEventListener('click', () => void exportLongitudinalResult())

  if (state.qcConfig.metadataId && state.wasmReady && !state.metadataInspectBusy && state.metadataInspectionFileId !== state.qcConfig.metadataId) {
    void inspectSelectedMetadata()
    return
  }
  if (cfg.exclusionEnabled && cfg.exclusionColumn && !state.metadataValueCache.has(metadataValueKey(cfg.exclusionColumn))) {
    void loadMetadataValues(cfg.exclusionColumn).then(rerender).catch((error) => {
      state.error = error instanceof Error ? error.message : String(error)
      rerender()
    })
  }
}

function significantSampleOrder(panel) {
  const samples = panel?.sample_ids || []
  const leaves = panel?.dendrogram?.leaves || []
  if (!samples.length || !leaves.length) return samples.map((_, index) => index)
  const bySample = new Map(samples.map((sample, index) => [String(sample), index]))
  const order = leaves.map((leaf) => bySample.get(String(leaf.sample_id))).filter((index) => index !== undefined)
  return order.length === samples.length ? order : samples.map((_, index) => index)
}

function heatmapCellColor(value) {
  const z = Math.max(-3, Math.min(3, Number(value) || 0))
  const amount = Math.abs(z) / 3
  if (z >= 0) return `rgba(196, 60, 60, ${0.12 + amount * 0.76})`
  return `rgba(49, 89, 217, ${0.12 + amount * 0.76})`
}

function significantHeatmap(panel) {
  const features = panel?.heatmap_features || []
  const rows = panel?.heatmap_zscores || []
  const samples = panel?.sample_ids || []
  if (!features.length || !rows.length || !samples.length) return '<div class="plot-empty">No heatmap matrix is available.</div>'
  const order = significantSampleOrder(panel)
  return `<div class="significant-heatmap-scroll"><table class="significant-heatmap"><thead><tr><th class="heatmap-feature-head">Feature</th>${order.map((index) => `<th class="heatmap-sample-head" title="${escapeHtml(samples[index])}"><span>${escapeHtml(samples[index])}</span></th>`).join('')}</tr><tr><th class="heatmap-feature-head heatmap-annotation-label">Group</th>${order.map((index) => `<th class="heatmap-group-cell ${String(panel.groups[index]).toLowerCase() === 'case' ? 'case-heat' : 'control-heat'}" title="${escapeHtml(samples[index])} · ${escapeHtml(panel.groups[index])}"></th>`).join('')}</tr></thead><tbody>${features.map((feature, rowIndex) => `<tr><th class="heatmap-feature-label" title="${escapeHtml(feature.feature)} · q ${formatNumber(feature.q_value)} · effect ${formatNumber(feature.adjusted_effect)}"><strong>${escapeHtml(feature.feature)}</strong><span>${escapeHtml(feature.family || '')}</span></th>${order.map((sampleIndex) => { const value = Number(rows[rowIndex]?.[sampleIndex] ?? 0); return `<td class="heatmap-cell" style="background:${heatmapCellColor(value)}" title="${escapeHtml(feature.feature)} · ${escapeHtml(samples[sampleIndex])} · z=${formatNumber(value)}"><span class="visually-hidden">${formatNumber(value)}</span></td>` }).join('')}</tr>`).join('')}</tbody></table></div><div class="heatmap-scale" aria-label="Heatmap z-score scale"><span>−3</span><i class="heatmap-gradient"></i><span>+3</span><small>Feature-wise z-score</small></div>`
}

function significantFeatureTable(panel) {
  const rows = panel?.analysis_features || []
  if (!rows.length) return '<div class="plot-empty">No selected features are available.</div>'
  return `<div class="table-scroll differential-table"><table><thead><tr><th>Feature</th><th>Family / secondary ID</th><th>Adjusted effect</th><th>q-value</th></tr></thead><tbody>${rows.map((row) => `<tr><td><strong>${escapeHtml(row.feature)}</strong></td><td>${escapeHtml(row.family || '')}</td><td>${formatNumber(row.adjusted_effect)}</td><td>${formatNumber(row.q_value)}</td></tr>`).join('')}</tbody></table></div><p class="table-note">Showing all ${rows.length.toLocaleString()} features used for the post-selection structure analysis.</p>`
}

function significantPanelSection(title, result) {
  if (!result) return ''
  const panel = result.panel
  const warnings = [...new Set([...(result.warnings || []), ...(panel?.warnings || [])])]
  const caseLabel = result.comparison?.case_label || 'Case'
  const controlLabel = result.comparison?.control_label || 'Control'
  if (!panel) {
    return `<section class="card significant-result"><div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>No features passed the configured post-selection threshold.</p></div><span class="tag neutral">No hits</span></div>${warnings.length ? `<div class="analysis-warnings">${warnings.map((warning) => `<div>${escapeHtml(warning)}</div>`).join('')}</div>` : ''}</section>`
  }
  const pca = panel.pca
  const permanova = pca ? `F ${formatNumber(pca.permanova_f)} · p ${formatNumber(pca.permanova_p)}` : 'Not available'
  const prefix = title.toLowerCase().startsWith('gene') ? 'hit-gene' : 'hit-transcript'
  return `<section class="card significant-result"><div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>${escapeHtml(panel.distance_label)} · ${panel.sample_ids.length} modeled samples</p></div><span class="tag warning">Post-selection</span></div>
    <div class="summary-grid compact-grid">
      ${summaryCard('Significant hits', panel.significant_features_total, `BH q ≤ ${formatNumber(state.globalThresholds.fdr)} · |log2FC| ≥ ${formatNumber(state.globalThresholds.absLog2Fc)}`)}
      ${summaryCard('Features analyzed', panel.analysis_features.length, `Maximum ${state.hitConfig.maxAnalysisFeatures}`)}
      ${summaryCard('Silhouette', formatNumber(panel.silhouette), 'Case/Control coherence on the selected-hit distance matrix')}
      ${summaryCard('NN group accuracy', Number.isFinite(Number(panel.nearest_neighbor_group_accuracy)) ? `${(Number(panel.nearest_neighbor_group_accuracy) * 100).toFixed(1)}%` : '—', 'Nearest other sample has the same Case/Control label')}
      ${summaryCard('PERMANOVA', permanova, `${pca?.permutations || 0} subject-blocked permutations when repeated measures are present`)}
      ${summaryCard('Heatmap rows', panel.heatmap_features.length, `Maximum ${state.hitConfig.maxHeatmapFeatures}`)}
    </div>
    ${warnings.length ? `<div class="analysis-warnings">${warnings.map((warning) => `<div>${escapeHtml(warning)}</div>`).join('')}</div>` : ''}
    <div class="plot-grid"><article class="plot-card"><div class="plot-heading"><strong>Significant-hit PCA</strong><span>Descriptive structure after feature selection using the same group labels.</span></div>${pcaSvg(panel.pca, `${prefix}-pca`, caseLabel, controlLabel)}</article><article class="plot-card"><div class="plot-heading"><strong>Significant-hit hierarchical clustering</strong><span>Average linkage using only the selected-hit matrix.</span></div>${dendrogramSvg(panel.dendrogram, `${prefix}-dendrogram`, caseLabel, controlLabel)}</article></div>
    <article class="plot-card hit-heatmap-card"><div class="plot-heading"><strong>Clustered significant-hit heatmap</strong><span>Rows are selected features; columns follow the significant-hit dendrogram leaf order.</span></div>${significantHeatmap(panel)}</article>
    <details class="details-block result-details"><summary>Selected features (${panel.analysis_features.length})</summary>${significantFeatureTable(panel)}</details>
  </section>`
}

function significantCoherenceSummary(result) {
  const rows = [['Gene expression', result?.gene], ['Transcript / isoform expression', result?.transcript]]
    .filter(([, entry]) => entry?.panel)
    .map(([label, entry]) => ({ label, entry, panel: entry.panel }))
    .sort((a, b) => Number(b.panel.silhouette || -1) - Number(a.panel.silhouette || -1))
  if (!rows.length) return ''
  return `<section class="card"><div class="card-heading"><div><h3>Cross-modality selected-hit coherence</h3><p>This is a descriptive comparison of the post-selection structures. It is not a predictive ranking or independent validation.</p></div><span class="tag warning">Descriptive</span></div><div class="table-scroll"><table><thead><tr><th>Data type</th><th>Significant hits</th><th>Features analyzed</th><th>Silhouette</th><th>NN group accuracy</th><th>PERMANOVA p</th></tr></thead><tbody>${rows.map(({ label, panel }) => `<tr><td><strong>${escapeHtml(label)}</strong></td><td>${panel.significant_features_total}</td><td>${panel.analysis_features.length}</td><td>${formatNumber(panel.silhouette)}</td><td>${Number.isFinite(Number(panel.nearest_neighbor_group_accuracy)) ? `${(Number(panel.nearest_neighbor_group_accuracy) * 100).toFixed(1)}%` : '—'}</td><td>${formatNumber(panel.pca?.permanova_p)}</td></tr>`).join('')}</tbody></table></div></section>`
}

function significantHitsView() {
  const cfg = state.hitConfig
  const compare = state.compareResult
  const geneAvailable = Boolean(compare?.gene && selectedFile(state.compareConfig.geneMergedId))
  const transcriptAvailable = Boolean(compare?.transcript && selectedFile(state.compareConfig.transcriptMergedId))
  const enabledCount = Number(cfg.geneEnabled && geneAvailable) + Number(cfg.transcriptEnabled && transcriptAvailable)
  const runnable = Boolean(compare && state.wasmReady && !state.busy && selectedFile(state.qcConfig.metadataId) && enabledCount > 0)

  if (!compare) {
    return `<section class="page-heading"><div><span class="eyebrow">Significant Hits · W5</span><h2>Inspect the structure of differential hits</h2><p>This workflow starts from a completed Compare Groups analysis so the cohort definition, exclusions, covariates, and Case/Control labels cannot drift between feature selection and visualization.</p></div></section>
      ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
      <section class="card migration-card"><span class="tag warning">Comparison required</span><h3>Run Compare Groups first.</h3><p>Significant-hit PCA, clustering, and heatmaps are post-selection views of the same corrected comparison. Complete the group comparison before opening this workflow.</p><div class="button-row"><button id="hits-open-compare" class="primary-button">Go to Compare Groups</button></div></section>`
  }

  const comparisonName = compare.comparisonName || 'Current corrected comparison'
  return `<section class="page-heading"><div><span class="eyebrow">Significant Hits · W5</span><h2>Post-selection PCA, clustering, and heatmaps</h2><p>Use the FDR-supported features from <strong>${escapeHtml(comparisonName)}</strong> to ask whether the discovered signal is coherent across samples.</p></div><div class="local-card"><strong>Descriptive, not validation</strong><span>The same Case/Control labels select the features and appear in these plots.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  <div class="notice-banner" role="note"><strong>Post-selection interpretation:</strong><span>Clean Case/Control separation here is expected to some degree because these features were selected using the same labels. Use these plots to inspect consistency, outliers, and multivariate structure—not as held-out predictive performance.</span></div>
  <section class="card"><div class="card-heading"><div><h3>1. Frozen comparison</h3><p>W5 reuses the current W3 metadata roles, groups, covariates, exclusions, and merged tables. Change them in Compare Groups, then rerun that workflow if needed.</p></div><span class="tag success">${escapeHtml(comparisonName)}</span></div><div class="summary-grid compact-grid">${summaryCard('Case', compare.gene?.comparison?.case_label || compare.transcript?.comparison?.case_label || 'Case', `${state.compareConfig.caseValues.length} selected metadata value${state.compareConfig.caseValues.length === 1 ? '' : 's'}`)}${summaryCard('Control', compare.gene?.comparison?.control_label || compare.transcript?.comparison?.control_label || 'Control', `${state.compareConfig.controlValues.length} selected metadata value${state.compareConfig.controlValues.length === 1 ? '' : 's'}`)}${summaryCard('Subject / patient', state.compareConfig.subjectColumn || 'Not selected', state.compareConfig.subjectColumn ? 'Used for repeated-measure robust inference' : 'Samples treated as independent subjects')}${summaryCard('Covariates', state.compareConfig.covariates.length, state.compareConfig.covariates.length ? state.compareConfig.covariates.join(', ') : 'No adjustment covariates')}</div></section>
  ${globalThresholdControls()}
  <section class="card"><div class="card-heading"><div><h3>2. Post-selection display limits</h3><p>The global FDR/log2FC thresholds define the hit set. These limits only cap how many selected features are drawn in dense views.</p></div></div><div class="form-grid hit-settings-grid"><label><span>Maximum PCA/clustering features</span><input id="hit-max-analysis" type="number" min="2" step="1" value="${cfg.maxAnalysisFeatures}"/></label><label><span>Maximum heatmap features</span><input id="hit-max-heatmap" type="number" min="1" step="1" value="${cfg.maxHeatmapFeatures}"/></label></div></section>
  <section class="card"><div class="card-heading"><div><h3>3. Data types</h3><p>Only expression modalities completed in the current Compare Groups result can be used.</p></div></div><div class="config-grid expression-source-grid"><div class="config-block stacked ${geneAvailable ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="hit-gene-enabled" type="checkbox" ${cfg.geneEnabled && geneAvailable ? 'checked' : ''} ${geneAvailable ? '' : 'disabled'}/><span><strong>Gene expression</strong><small>Selected corrected gene hits</small></span></label><span class="tag ${geneAvailable ? 'success' : 'neutral'}">${geneAvailable ? 'Available' : 'Run W3 first'}</span></div></div><div class="config-block stacked ${transcriptAvailable ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="hit-transcript-enabled" type="checkbox" ${cfg.transcriptEnabled && transcriptAvailable ? 'checked' : ''} ${transcriptAvailable ? '' : 'disabled'}/><span><strong>Transcript / isoform expression</strong><small>Selected corrected transcript hits</small></span></label><span class="tag ${transcriptAvailable ? 'success' : 'neutral'}">${transcriptAvailable ? 'Available' : 'Run W3 first'}</span></div></div></div>
  <div class="run-row"><div><strong>${enabledCount} expression data type${enabledCount === 1 ? '' : 's'} enabled</strong><span>The corrected W3 model is rerun locally with the frozen settings, then only the selected-hit structure is retained.</span></div><button id="run-significant-hits" class="primary-button" ${runnable ? '' : 'disabled'}>${state.busy ? 'Running…' : 'Run significant-hit structure'}</button></div></section>
  ${state.hitProgress ? `<div class="progress-card" role="status"><div class="progress-copy"><strong>${escapeHtml(state.hitProgress.message)}</strong><span>${state.hitProgress.completed} of ${state.hitProgress.total} modality steps complete</span></div><progress max="${Math.max(1, state.hitProgress.total)}" value="${state.hitProgress.completed}"></progress></div>` : ''}
  ${state.hitResult ? `<div class="result-heading"><div><span class="eyebrow">Post-selection result</span><h3>${escapeHtml(state.hitResult.comparisonName)}</h3><p>Generated entirely on this device from the same corrected comparison settings.</p></div><button id="export-significant-hits" class="secondary-button">Save significant-hit JSON</button></div>${significantCoherenceSummary(state.hitResult)}${significantPanelSection('Gene expression', state.hitResult.gene)}${significantPanelSection('Transcript / isoform expression', state.hitResult.transcript)}` : ''}`
}

async function runSignificantHits() {
  const compare = state.compareResult
  if (!compare) {
    state.error = 'Run Compare Groups before significant-hit structure analysis.'
    render()
    return
  }
  const metadataFile = selectedFile(state.qcConfig.metadataId)
  const geneFile = selectedFile(state.compareConfig.geneMergedId)
  const transcriptFile = selectedFile(state.compareConfig.transcriptMergedId)
  if (!metadataFile) {
    state.error = 'The local metadata file is no longer available. Select the project files again.'
    render()
    return
  }
  const { comparisonName, portableConfig } = currentPortableComparisonConfig()
  const hitConfig = {
    qThreshold: Number(state.globalThresholds.fdr),
    minAbsEffect: Number(state.globalThresholds.absLog2Fc),
    maxAnalysisFeatures: Math.max(2, Math.trunc(Number(state.hitConfig.maxAnalysisFeatures) || 500)),
    maxHeatmapFeatures: Math.max(1, Math.trunc(Number(state.hitConfig.maxHeatmapFeatures) || 80)),
  }
  const geneEnabled = Boolean(state.hitConfig.geneEnabled && compare.gene && geneFile)
  const transcriptEnabled = Boolean(state.hitConfig.transcriptEnabled && compare.transcript && transcriptFile)
  state.busy = true
  state.error = null
  state.hitResult = null
  state.hitProgress = { completed: 0, total: Number(geneEnabled) + Number(transcriptEnabled), message: 'Starting significant-hit analysis' }
  render()
  try {
    const response = await workerRequest('run-significant-hits', {
      metadataFile,
      comparisonName,
      comparisonConfig: portableConfig,
      hitConfig,
      gene: { enabled: geneEnabled, file: geneEnabled ? geneFile : null },
      transcript: { enabled: transcriptEnabled, file: transcriptEnabled ? transcriptFile : null },
    }, (progress) => {
      state.hitProgress = progress
      render()
    })
    state.hitResult = response.result
    await recordHistory('significant-hit-structure-expression', response.result.comparisonName || 'Significant Hits')
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    state.hitProgress = null
    render()
  }
}

function bindSignificantHitEvents() {
  const cfg = state.hitConfig
  document.querySelector('#hits-open-compare')?.addEventListener('click', () => { state.tab = 'compare'; render() })
  bindGlobalThresholdEvents()
  document.querySelector('#hit-max-analysis')?.addEventListener('change', (event) => { cfg.maxAnalysisFeatures = Math.max(2, Math.trunc(Number(event.currentTarget.value) || 500)); state.hitResult = null })
  document.querySelector('#hit-max-heatmap')?.addEventListener('change', (event) => { cfg.maxHeatmapFeatures = Math.max(1, Math.trunc(Number(event.currentTarget.value) || 80)); state.hitResult = null })
  document.querySelector('#hit-gene-enabled')?.addEventListener('change', (event) => { cfg.geneEnabled = event.currentTarget.checked; state.hitResult = null; render() })
  document.querySelector('#hit-transcript-enabled')?.addEventListener('change', (event) => { cfg.transcriptEnabled = event.currentTarget.checked; state.hitResult = null; render() })
  document.querySelector('#run-significant-hits')?.addEventListener('click', () => void runSignificantHits())
  document.querySelector('#export-significant-hits')?.addEventListener('click', () => void exportHitResult())
  bindInteractivePlots()
}


function historyWorkflowTitle(workflow) {
  return ({
    'explore-qc-expression': 'Explore & QC',
    'compare-groups-expression': 'Compare Groups',
    'longitudinal-patient-signature-expression': 'Longitudinal',
    'significant-hit-structure-expression': 'Significant Hits',
  })[workflow] || workflow || 'Analysis result'
}

function historyView() {
  const estimate = state.storageEstimate
  const storageDetail = estimate?.quota
    ? `${humanBytes(estimate.usage)} used of approximately ${humanBytes(estimate.quota)} browser storage`
    : 'Stored in this browser profile with IndexedDB'
  const entries = state.historyEntries
  return `<section class="page-heading"><div><span class="eyebrow">History</span><h2>Previous local analysis results</h2><p>Completed runs are saved as browser-local snapshots so you can inspect them again after a page reload. Raw cohort files are never stored in History.</p></div><div class="local-card"><strong>Browser-local audit</strong><span>${escapeHtml(storageDetail)}</span></div></section>
    ${state.historyMessage ? `<div class="notice-banner" role="status"><strong>History</strong><span>${escapeHtml(state.historyMessage)}</span></div>` : ''}
    <section class="card history-card"><div class="card-heading"><div><h3>Saved result snapshots</h3><p>Open a snapshot to restore its result and settings. Delete removes only this browser snapshot and never the original cohort files.</p></div>${entries.length ? '<button id="clear-history" class="danger-text-button">Clear History</button>' : ''}</div>
      ${state.historyBusy && !state.historyLoaded ? '<div class="inline-status"><span class="spinner" aria-hidden="true"></span><div><strong>Reading browser History</strong><span>No network request is used.</span></div></div>' : ''}
      ${!state.historyBusy && state.historyLoaded && !entries.length ? '<div class="empty-state"><div class="empty-icon" aria-hidden="true">↺</div><strong>No saved results yet</strong><span>Each completed QC, comparison, longitudinal, or significant-hit run will be added here automatically.</span></div>' : ''}
      ${entries.length ? `<div class="history-list">${entries.map((entry) => `<article class="history-entry"><div class="history-entry-main"><div class="history-entry-title"><span class="tag success">${escapeHtml(historyWorkflowTitle(entry.workflow))}</span><strong>${escapeHtml(entry.label || historyWorkflowTitle(entry.workflow))}</strong></div><span>${new Date(entry.createdAt).toLocaleString()} · ${escapeHtml(entry.summary || '')}</span><small>${Number(entry.projectFileCount || 0).toLocaleString()} referenced files · ${humanBytes(Number(entry.localDataBytes || 0))} · ${escapeHtml(entry.engine || '')}</small></div><div class="history-entry-actions"><button class="secondary-button history-open" data-history-id="${escapeHtml(entry.id)}">View</button><button class="text-button history-export" data-history-id="${escapeHtml(entry.id)}">Save JSON</button><button class="danger-text-button history-delete" data-history-id="${escapeHtml(entry.id)}">Delete</button></div></article>`).join('')}</div>` : ''}
    </section>`
}

function settingsView() {
  const r = state.resources
  const info = state.systemInfo || detectSystemInfo()
  const suggested = info.threads ? suggestedScienceThreads(info.threads, r) : 1
  const sample = state.appMemorySample
  const sampleText = sample?.available
    ? `${humanBytes(sample.bytes)} · ${sample.source}`
    : sample?.source || 'Not measured. Browser support varies.'
  return `<section class="page-heading"><div><span class="eyebrow">Global settings</span><h2>System resource controls</h2><p>Settings apply to this browser profile, not to saved cohort states. Local scientific input files have no fixed size cap.</p></div><div class="local-card"><strong>Science execution</strong><span>Current browser WASM: one worker, not Rayon</span></div></section>
    ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
    <section class="card"><div class="card-heading"><div><h3>CPU and worker use</h3><p>Reserve part of the available logical CPU capacity for other work.</p></div></div>
      <div class="form-grid resource-grid">
        <label><span>CPU use target (%)</span><input id="resource-cpu" type="number" min="10" max="90" step="5" value="${r.cpuPercent}" /></label>
        <label><span>Maximum science threads (future PX1)</span><input id="resource-threads" type="number" min="1" max="32" step="1" value="${r.maxThreads}" /></label>
      </div>
      <p class="method-note">${info.threads ? `${info.threads} browser-reported logical threads; planned PX1 ceiling: ${suggested} threads.` : 'Logical CPU count is unavailable.'} Current single-worker WASM cannot use this thread budget. A separate threaded-WASM build is required. These controls do not set a system CPU quota.</p>
    </section>
    <section class="card"><div class="card-heading"><div><h3>Memory information and display limits</h3><p>Local input files are never rejected because of their size or an estimated copy budget. Large inputs can still exhaust the browser's WASM memory.</p></div></div>
      <div class="form-grid resource-grid">
        <label><span>Physical RAM (GiB, optional manual entry)</span><input id="resource-ram" type="number" min="0" max="2048" step="0.5" value="${r.physicalRamGiB}" /></label>
        <label><span>Maximum volcano points drawn</span><input id="resource-points" type="number" min="200" max="6000" step="200" value="${r.maxPlotPoints}" /></label>
      </div>
      <p class="method-note">Browser system RAM report: ${escapeHtml(info.memory)}. ${r.physicalRamGiB ? `Manual RAM fallback: ${r.physicalRamGiB} GiB (not verified).` : 'Manual RAM is optional.'} An optional native Rust monitor can read actual system metrics; Firefox cannot access sysinfo directly.</p>
      <div class="native-monitor-panel">
        <div class="card-heading"><div><h3>Optional native system monitor</h3><p>Run the local Rust helper, then paste its session token. Connection is opt-in. Metrics stay on this computer; source files are not read.</p></div></div>
        <div id="native-monitor-status" class="resource-memory-status" role="status" aria-live="polite">${monitorSummaryHtml()}</div>
        <label class="native-monitor-token"><span>Local monitor session token (not saved)</span><input id="native-monitor-token" type="password" placeholder="Paste the 32-character token from the helper terminal" autocomplete="off" spellcheck="false" /></label>
        <div class="button-row resource-actions"><button id="connect-native-monitor" class="secondary-button" ${state.monitorBusy ? 'disabled' : ''}>${state.monitorBusy ? 'Connecting…' : 'Connect native monitor'}</button><button id="disconnect-native-monitor" class="secondary-button" ${nativeMonitor.getState().status === 'disconnected' ? 'disabled' : ''}>Disconnect</button></div>
        <p class="method-note">Start from the repository: <code>cargo run --release --manifest-path native/system-monitor/Cargo.toml</code>. The helper listens only on 127.0.0.1:47831 and accepts requests from the approved site with a session token. Your browser may request loopback-network permission. Stop the helper with Ctrl+C.</p>
      </div>
      <div class="resource-memory-status" role="status"><strong>Browser application memory</strong><span>${escapeHtml(sampleText)}</span></div>
      <p class="method-note">When supported, the measurement estimates application memory or the main JavaScript heap only. It is not physical RAM or currently free memory. Firefox may not expose either measurement.</p>
      <div class="button-row"><button id="measure-app-memory" class="secondary-button" ${state.appMemoryBusy ? 'disabled' : ''}>${state.appMemoryBusy ? 'Measuring…' : 'Measure app memory'}</button></div>
      <label class="resource-check"><input id="resource-recycle" type="checkbox" ${r.recycleWorker ? 'checked' : ''}/> <span>Release the science worker and WASM memory after each completed analysis (recommended).</span></label>
      <div class="button-row resource-actions"><button id="save-resources" class="primary-button">Apply settings</button><button id="reset-resources" class="secondary-button">Reset defaults</button></div>
    </section>`
}
function bindSettingsEvents() {
  document.querySelector('#connect-native-monitor')?.addEventListener('click', async () => {
    if (state.monitorBusy) return
    const tokenInput = document.querySelector('#native-monitor-token')
    const token = tokenInput?.value || ''
    state.monitorBusy = true
    if (tokenInput) tokenInput.value = ''
    // Call connect from this user gesture. Some browsers ask for loopback access.
    const connectRequest = nativeMonitor.connect(token)
    const connectButton = document.querySelector('#connect-native-monitor')
    if (connectButton) { connectButton.disabled = true; connectButton.textContent = 'Connecting…' }
    await connectRequest
    state.monitorBusy = false
    stopMonitorPolling()
    if (nativeMonitor.getState().status === 'connected') {
      monitorTimer = setInterval(() => void pollNativeMonitor(), 3000)
    }
    if (state.tab === 'settings') render()
    else updateMonitorDisplay()
  })
  document.querySelector('#disconnect-native-monitor')?.addEventListener('click', () => {
    stopMonitorPolling()
    nativeMonitor.disconnect()
    render()
  })
  document.querySelector('#save-resources')?.addEventListener('click', () => {
    state.resources = normalizeResources({
      cpuPercent: document.querySelector('#resource-cpu')?.value,
      maxThreads: document.querySelector('#resource-threads')?.value,
      physicalRamGiB: document.querySelector('#resource-ram')?.value,
      maxPlotPoints: document.querySelector('#resource-points')?.value,
      recycleWorker: Boolean(document.querySelector('#resource-recycle')?.checked),
    })
    persistResourceSettings()
    state.error = null
    render()
  })
  document.querySelector('#reset-resources')?.addEventListener('click', () => {
    state.resources = { ...DEFAULT_RESOURCES }
    persistResourceSettings()
    render()
  })
  document.querySelector('#measure-app-memory')?.addEventListener('click', async () => {
    if (state.appMemoryBusy) return
    state.appMemoryBusy = true
    render()
    try {
      state.appMemorySample = await measureAppMemory(globalThis.performance, Boolean(globalThis.crossOriginIsolated))
    } finally {
      state.appMemoryBusy = false
      if (state.tab === 'settings') render()
    }
  })
}

function docsView() {
  const references = methodDocs.reduce((total, section) => total + (section.references || []).length, 0)
  return `<section class="page-heading"><div><span class="eyebrow">Docs</span><h2>Methods, assumptions, and references</h2><p>Search the methods that are implemented in 4C Insight. Each section states what the application does, what it does not claim, and the literature that supports the method.</p></div><div class="local-card"><strong>Literature-backed methods</strong><span>${methodDocs.length} sections · ${references} linked references</span></div></section>
    <section class="card docs-search-card"><label for="docs-search"><strong>Search documentation</strong><span>Search methods, terms, limitations, or references.</span></label><input id="docs-search" type="search" value="${escapeHtml(state.docsQuery)}" placeholder="e.g. NB2, PERMANOVA, longitudinal, FDR, CLR" autocomplete="off"/><small id="docs-search-status">${methodDocs.length} sections shown</small></section>
    <nav class="docs-toc" aria-label="Documentation sections">${methodDocs.map((section) => `<a href="#doc-${escapeHtml(section.id)}">${escapeHtml(section.title)}</a>`).join('')}</nav>
    <div class="docs-methods">${methodDocs.map((section) => {
      const searchable = [section.title, section.keywords, section.summary, ...(section.body || []), ...(section.references || []).map((reference) => reference.label)].join(' ').toLowerCase()
      return `<article id="doc-${escapeHtml(section.id)}" class="card docs-section" data-doc-section data-doc-search="${escapeHtml(searchable)}"><div class="card-heading"><div><h3>${escapeHtml(section.title)}</h3><p>${escapeHtml(section.summary)}</p></div><a class="text-button docs-anchor" href="#doc-${escapeHtml(section.id)}" aria-label="Link to ${escapeHtml(section.title)}">#</a></div>${(section.body || []).map((paragraph) => `<p>${escapeHtml(paragraph)}</p>`).join('')}${section.references?.length ? `<div class="docs-references"><strong>References</strong><ol>${section.references.map((reference) => `<li><a href="${escapeHtml(reference.href)}" target="_blank" rel="noreferrer">${escapeHtml(reference.label)}</a></li>`).join('')}</ol></div>` : `<div class="docs-references"><strong>References</strong><p>No external reference is required for this application-specific provenance behavior.</p></div>`}</article>`
    }).join('')}</div>`
}

function bindDocsEvents() {
  const input = document.querySelector('#docs-search')
  const status = document.querySelector('#docs-search-status')
  const sections = [...document.querySelectorAll('[data-doc-section]')]
  const apply = () => {
    const query = String(input?.value || '').trim().toLowerCase()
    state.docsQuery = query
    let visible = 0
    for (const section of sections) {
      const match = !query || String(section.dataset.docSearch || '').includes(query)
      section.hidden = !match
      if (match) visible += 1
    }
    if (status) status.textContent = query ? `${visible} of ${sections.length} sections match` : `${sections.length} sections shown`
  }
  input?.addEventListener('input', apply)
  apply()
}

function sortableTableValue(value) {
  if (value === null || value === undefined) return { numeric: false, value: '' }
  if (typeof value === 'number' && Number.isFinite(value)) return { numeric: true, value }
  const text = String(value).trim()
  const numericText = text.replace(/,/g, '').replace(/%$/, '')
  if (numericText && /^[-+]?\d*\.?\d+(?:e[-+]?\d+)?$/i.test(numericText)) return { numeric: true, value: Number(numericText) }
  return { numeric: false, value: text.toLocaleLowerCase() }
}

function compareSortableValues(a, b) {
  const left = sortableTableValue(a)
  const right = sortableTableValue(b)
  if (left.numeric && right.numeric) return left.value - right.value
  return String(left.value).localeCompare(String(right.value), undefined, { numeric: true, sensitivity: 'base' })
}

function enhanceTables() {
  document.querySelectorAll('.table-scroll').forEach((scroll, tableIndex) => {
    const table = scroll.querySelector('table')
    if (!table || table.classList.contains('significant-heatmap') || table.dataset.enhanced === 'true') return
    table.dataset.enhanced = 'true'
    const headers = [...table.querySelectorAll('thead tr:first-child th')]
    const tbody = table.querySelector('tbody')
    if (!headers.length || !tbody) return
    const sourceId = scroll.dataset.dynamicTable || ''
    const source = sourceId ? dynamicTableSources.get(sourceId) : null
    const staticRows = source ? [] : [...tbody.querySelectorAll('tr')]
    let query = ''
    let sortIndex = -1
    let sortDirection = 1
    const toolbar = document.createElement('div')
    toolbar.className = 'table-tools'
    toolbar.innerHTML = `<label><span class="visually-hidden">Search table</span><input type="search" class="table-search" placeholder="Search table…" autocomplete="off"/></label><span class="table-match-count" aria-live="polite"></span>`
    scroll.before(toolbar)
    const status = toolbar.querySelector('.table-match-count')

    const updateHeaderState = () => {
      headers.forEach((header, index) => {
        header.classList.add('sortable-table-header')
        header.tabIndex = 0
        header.setAttribute('role', 'button')
        header.setAttribute('aria-sort', index === sortIndex ? (sortDirection > 0 ? 'ascending' : 'descending') : 'none')
        header.dataset.sortDirection = index === sortIndex ? (sortDirection > 0 ? 'asc' : 'desc') : ''
      })
    }

    const apply = () => {
      if (source) {
        let rows = source.rows.filter((row) => !query || source.cells(row).some((value) => String(value ?? '').toLowerCase().includes(query)))
        if (sortIndex >= 0) rows = [...rows].sort((a, b) => compareSortableValues(source.cells(a)[sortIndex], source.cells(b)[sortIndex]) * sortDirection)
        const limit = source.limit || 500
        tbody.innerHTML = rows.slice(0, limit).map(source.rowHtml).join('')
        status.textContent = `Showing ${Math.min(rows.length, limit).toLocaleString()} of ${rows.length.toLocaleString()} matching · ${source.rows.length.toLocaleString()} total`
      } else {
        const matches = staticRows.filter((row) => !query || row.textContent.toLowerCase().includes(query))
        if (sortIndex >= 0) matches.sort((a, b) => compareSortableValues(a.cells[sortIndex]?.textContent, b.cells[sortIndex]?.textContent) * sortDirection)
        const matchSet = new Set(matches)
        for (const row of staticRows) row.hidden = !matchSet.has(row)
        for (const row of matches) tbody.append(row)
        status.textContent = `${matches.length.toLocaleString()} of ${staticRows.length.toLocaleString()} rows`
      }
      updateHeaderState()
    }

    toolbar.querySelector('.table-search')?.addEventListener('input', (event) => {
      query = event.currentTarget.value.trim().toLowerCase()
      apply()
    })
    headers.forEach((header, index) => {
      const sort = () => {
        if (sortIndex === index) sortDirection *= -1
        else { sortIndex = index; sortDirection = 1 }
        apply()
      }
      header.addEventListener('click', sort)
      header.addEventListener('keydown', (event) => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); sort() }
      })
    })
    updateHeaderState()
    apply()
  })
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
    await recordHistory('explore-qc-expression', 'Explore & QC')
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


function currentPortableComparisonConfig() {
  const cfg = state.compareConfig
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
  return {
    comparisonName,
    portableConfig: {
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
    },
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

  const { comparisonName, portableConfig } = currentPortableComparisonConfig()

  state.busy = true
  state.error = null
  state.compareResult = null
  state.hitResult = null
  state.hitProgress = null
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
    await recordHistory('compare-groups-expression', response.result.comparisonName || comparisonName || 'Compare Groups')
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    state.compareProgress = null
    render()
  }
}

function bindGlobalThresholdEvents() {
  const apply = () => {
    const fdrInput = document.querySelector('#global-fdr-threshold')
    const fcInput = document.querySelector('#global-log2fc-threshold')
    if (fdrInput) state.globalThresholds.fdr = Math.min(1, Math.max(0.000001, Number(fdrInput.value) || 0.05))
    if (fcInput) state.globalThresholds.absLog2Fc = Math.max(0, Number(fcInput.value) || 0)
    state.hitResult = null
    render()
  }
  document.querySelector('#global-fdr-threshold')?.addEventListener('change', apply)
  document.querySelector('#global-log2fc-threshold')?.addEventListener('change', apply)
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
  bindGlobalThresholdEvents()
  bindInteractivePlots()

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
  const stateInput = document.querySelector('#state-input')
  const dropZone = document.querySelector('#input-drop-zone')
  document.querySelector('#select-files')?.addEventListener('click', () => fileInput.click())
  document.querySelector('#select-folder')?.addEventListener('click', () => folderInput.click())
  document.querySelector('#reconnect-state-folder')?.addEventListener('click', () => folderInput.click())
  document.querySelector('#load-state')?.addEventListener('click', () => stateInput.click())
  document.querySelector('#save-state')?.addEventListener('click', () => void saveApplicationState())
  fileInput?.addEventListener('change', (event) => addFiles(event.currentTarget.files))
  folderInput?.addEventListener('change', (event) => addFiles(event.currentTarget.files))
  stateInput?.addEventListener('change', (event) => void loadApplicationState(event.currentTarget.files?.[0]))
  if (dropZone) {
    const activate = (event) => { event.preventDefault(); dropZone.classList.add('is-dragover') }
    const deactivate = (event) => {
      event.preventDefault()
      if (!event.relatedTarget || !dropZone.contains(event.relatedTarget)) dropZone.classList.remove('is-dragover')
    }
    dropZone.addEventListener('dragenter', activate)
    dropZone.addEventListener('dragover', activate)
    dropZone.addEventListener('dragleave', deactivate)
    dropZone.addEventListener('drop', (event) => {
      event.preventDefault()
      dropZone.classList.remove('is-dragover')
      const files = event.dataTransfer?.files
      if (files?.length) addFiles(files)
    })
  }
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

function bindHistoryEvents() {
  document.querySelectorAll('.history-open').forEach((button) => button.addEventListener('click', () => void openHistoryEntry(button.dataset.historyId)))
  document.querySelectorAll('.history-export').forEach((button) => button.addEventListener('click', () => void exportHistorySnapshot(button.dataset.historyId)))
  document.querySelectorAll('.history-delete').forEach((button) => button.addEventListener('click', () => void removeHistorySnapshot(button.dataset.historyId)))
  document.querySelector('#clear-history')?.addEventListener('click', () => void removeAllHistory())
}

function render() {
  renderNav()
  dynamicTableSources.clear()
  const main = document.querySelector('#main-content')
  const viewFactories = {
    project: projectView,
    qc: qcView,
    compare: compareView,
    longitudinal: longitudinalView,
    hits: significantHitsView,
    history: historyView,
    docs: docsView,
    settings: settingsView,
  }
  main.innerHTML = (viewFactories[state.tab] || projectView)()
  if (state.tab === 'project') bindProjectEvents()
  if (state.tab === 'qc') bindQcEvents()
  if (state.tab === 'compare') bindCompareEvents()
  if (state.tab === 'longitudinal') bindLongitudinalEvents()
  if (state.tab === 'hits') bindSignificantHitEvents()
  if (state.tab === 'history') {
    bindHistoryEvents()
    if (!state.historyLoaded && !state.historyBusy) void refreshHistory()
  }
  if (state.tab === 'docs') bindDocsEvents()
  if (state.tab === 'settings') bindSettingsEvents()
  enhanceTables()
  renderSystemInfo()
  document.querySelector('#system-chip')?.addEventListener('click', () => { state.tab = 'settings'; render() })
  document.querySelector('#stop-science-work')?.addEventListener('click', stopScienceWork)
}

async function registerIsolationServiceWorker() {
  if (!('serviceWorker' in navigator) || !window.isSecureContext) return
  try {
    const workerUrl = new URL('./coi-serviceworker.js', window.location.href)
    workerUrl.searchParams.set('v', APP_VERSION)
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
