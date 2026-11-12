import {bindPlotNavigation, bindPlotToolbar, plotNavigationToolbar, boxZoomDomain, boxZoomView, attachStaticSvgNavigation, attachTableHeatmapNavigation, PLOT_ZOOM_MAX} from './plot-navigation.js?v=0.22.8'
import { readIntegrativeMetadataColumns, validIntegrativeIdentityRoles } from './integrative-metadata-columns.js?v=0.22.8'
import { readLocalSession, saveLocalSession, saveLocalResult, readLocalResults, readStoredSources, listStoredSourceIds, saveStoredSource, removeStoredSources, clearLocalSession, recoverSourceFile, validLocalSession } from './local-session.js?v=0.22.8'
import { runWorkflowSteps, integratedOverviewMetrics } from './integrative-workflow.js?v=0.22.8'
import { integrativeSummaryHtml, integrativeInfluenceHtml, integrativeScatterMarkup, bindIntegrativeScatter, integrativeClusterCautionHtml } from './integrative-ux.js?v=0.22.8'
import { buildIntegrativeCoverage, integrativeCoverageHtml, integrativeOmittedTsv, bindIntegrativeCoverage } from './integrative-coverage.js?v=0.22.8'
import { biologicalCandidates, biologicalCandidatesTsv, exploratoryGeneRankingTsv, biologicalNextStepsHtml, bindBiologicalFeatureExplorer } from './biological-next-steps.js?v=0.22.8'
import {mountPathwayModule, defaultPathwayConfig} from './pathway-view.js?v=0.22.8'
import { rankDirectSourceFeatures, inspectJointObservation, directFeatureTsv } from './source-biological-interpretation.js?v=0.22.8'
import { modalityOverviewHtml, combinedFeatureRows, featureLollipopHtml, featuredProfiles, chosenProfile, combinedFeatureTsv } from './biological-dashboard.js?v=0.22.8'
import { buildBiologicalFindings, findingsTsv } from './biological-findings.js?v=0.22.8'
import { selectFeatureSamples, expressionAssociationsTsv } from './biological-feature-ranking.js?v=0.22.8'
import { runBoundedSourceTasks, sourceProgressSummary, validParallelWorkers } from './source-distance-queue.js?v=0.22.8'
import { heatmapViewerMarkup, mountJointHeatmapViewer, jointDistanceReviewMarkup } from './joint-heatmap-viewer.js?v=0.22.8'
import { compareFixedCohortAblation } from './joint-structure.js?v=0.22.8'
import { extractFullDistanceViews, prepareJointDistanceInput, jointDistancesTsv } from './integrative-distance-input.js?v=0.22.8'
import { prepareI2BRequests } from './i2b-source-selection.js?v=0.22.8'
import { I1_MODALITIES, alignObservations, alignmentTsv, mergedSampleIds, readFirstTsvRecord } from './multiomics-alignment.js?v=0.22.8'
import { summarizeIntegrativeEvidence, integrativeSummaryTotals, collectIntegrativeSelectedRows } from './integrative-evidence.js?v=0.22.8'
import { collectAdjustedTests, selectAdjustedTests, adjustedTestsCsv } from './extended-hit-review.js?v=0.22.8'
import { renderProjectInputGuide } from './project-input-guide.js?v=0.22.8'
import { renderResultsOverview } from './results-overview.js?v=0.22.8'
import { bindOverviewInteractions, bindDetailedResultsExports } from './results-interactions.js?v=0.22.8'
import {
  CORE_COLUMNS, OPTIONAL_COLUMNS, MAPPING_KINDS, createBlankMetadata,
  draftFromSamples, validateMetadata, addColumn, renameColumn, removeColumn,
  addRow, setCell, removeRow, autoMapFiles, mappingSummary, mappingManifest,
  matchingFiles, filterRows, serializeDelimited, applyMappingsToAliases,
} from './sample-metadata.js'
import { saveTextFile } from './save.js'
import { mergeMetrics, defaultMergeMetrics, sortedMergeFiles, validateMergeConfig } from './merge-helpers.js'
import { matchSavedFiles, matchSavedReference } from './state-reconnect.js'
import { aiAssistantView, mountAiAssistant } from './ai-assistant.js?v=0.22.8'
import { methodDocs } from './docs-content.js?v=0.22.8'
import { exampleSearchText, renderToyExamples, bindToyVisualControls } from './docs-toy-examples.js?v=0.22.8'
import { validateWasmMemoryReport } from './wasm-memory.js'
import { isolatedScienceRequest } from './isolated-science.js'
import { tableWindow, tableRowRange, TABLE_ROW_HEIGHT } from './table-viewport.js'
import { DEFAULT_RESOURCES, normalizeResources, suggestedScienceThreads, thinPlotPoints, measureAppMemory, detectBrowserResources } from './resource-controls.js'
import {
  clearHistoryEntries,
  deleteHistoryEntry,
  listHistoryEntries,
  loadHistorySnapshot,
  saveHistoryEntry,
  storageEstimate,
} from './persistence.js'

const APP_VERSION = '0.22.8'
const APP_STATE_SCHEMA = 1
const dynamicTableSources = new Map()

const tabs = [
  ['project', 'Project', '1'],
  ['metadata', 'Sample Metadata', 'M'],
  ['qc', 'Explore & QC', '2'],
  ['compare', 'Compare Groups', '3'],
  ['results', 'Results', 'R'],
  ['longitudinal', 'Longitudinal', '4'],
  ['hits', 'Integrative Analysis', '5'],
  ['ai', 'AI Assistant', 'A'],
  ['history', 'History', '6'],
  ['docs', 'Docs', '?'],
  ['settings', 'Settings', '⚙'],
]

const otherOmics = [
  ['bcr', 'BCR repertoire', 'bcrEnabled', 'bcr', 'Clone diversity, clonality, V/D/J and composition'],
  ['tcr-alpha', 'TCR alpha', 'tcrAlphaEnabled', 'tcrAlpha', 'Clone diversity, V/J, V-J pairing and sharing'],
  ['tcr-beta', 'TCR beta', 'tcrBetaEnabled', 'tcrBeta', 'Clone diversity, V/J, V-J pairing and sharing'],
  ['bcr-isotype', 'BCR isotypes', 'isotypeEnabled', 'isotype', 'CLR composition, Aitchison distance and adjusted comparisons'],
  ['immune-infiltrate', 'Immune infiltration', 'immuneEnabled', 'immune', 'ssGSEA signatures, corrected group tests and PCA'],
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
  mergeBusy: false,
  mergeProgress: '',
  mergeReports: { gene: null, transcript: null },
  mergeSettings: { gene: ['raw-count', 'tpm'], transcript: ['total-reads', 'tpm'], order: 'adjacent-by-sample' },
  metadataEditor: {
    table: null, sourceName: '', sourceId: '', generatedId: '', mappings: {},
    idColumn: 'sample_id', dirty: false, status: '', filter: '', filterColumn: '',
    filterValue: '', sortColumn: '', descending: false,
    metadataScrollTop: 0, mapFilter: '', mapUnmatched: false, mappingScrollTop: 0,
  },
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
  resultsMode: 'overview',
  resultsExpanded: [],
  longitudinalProgress: null,
  longitudinalResult: null,
  hitProgress: null,
  hitResult: null,
  integrativeView: 'overview',
  pathwayBusy: false,
  integrativeRunBusy: false,
  integrativeRunError: '',
  integrativeRunSteps: [],
  alignmentSource: 'active',
  alignmentRoles: { observation: '', subject: '', time: '' },
  alignmentColumns: [],
  integrativeColumnBusy: false,
  integrativeColumnError: '',
  alignmentPreview: null,
  alignmentTable: null,
  bioMetadataColumn: '',
  bioClusterCount: 3,
  bioResult: null,
  bioError: '',
  bioFeatureKind: 'gene',
  bioFeatureResult: null,
  pathwayConfig: defaultPathwayConfig(),
  pathwayGeneSets: null,
  pathwayResult: null,
  pathwaySelectedKey: '',
  pathwayNetworkConfig: {},
  pathwayLeadingOnly: false,
  pathwayStatus: '',
  bioFeatureAxis: 'pcoa1_spearman',
  bioSelectedProfile: '',
  bioSelectedSourceFeature: '',
  bioInspectObservation: '',
  bioFeatureBusy: false,
  bioFeatureError: '',
  bioFeatureBytes: 0,
  bioFeatureTotal: 1,
  bioFeatureProgress: '',
  alignmentBusy: false,
  alignmentError: '',
  i2ImportedViews: [],
  i2bDirectViews: [],
  i2bBusy: false,
  i2bErrors: [],
  i2bProgress: '',
  i2bProgressRows: [],
  i2bParallelWorkers: 2,
  i2Result: null,
  i2Busy: false,
  i2Error: '',
  i2MinViews: 2,
  i2StructureResult: null,
  jointStabilityResult: null,
  jointStabilityBusy: false,
  jointStabilityError: '',
  jointStabilityProgress: '',
  jointStabilityConfig: { clusters: 3, repeats: 40, fraction: 0.8, seed: 20261108 },
  i2StructureBusy: false,
  i2StructureError: '',
  i2StructureSelection: {},
  i2StructureOnePerPatient: false,
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
  wasmMemorySample: null,
  appMemoryBusy: false,
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
    bcrEnabled: false,
    tcrAlphaEnabled: false,
    tcrBetaEnabled: false,
    isotypeEnabled: false,
    immuneEnabled: false,
    ambiguousPolicy: 'exclude',
    excludeUndeterminedIsotype: false,
    repertoireRarefactionReps: 3,
    repertoireRarefactionCap: 50000,
    repertoirePermutations: 499,
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
let activeCompareController = null
let activeI2bController = null
let worker = null
function attachScienceWorker() {
  const current = new Worker(new URL(`./analysis-worker.js?v=${APP_VERSION}`, import.meta.url), { type: 'module' })
  current.addEventListener('message', (event) => {
    if (current !== worker) return
    const request = pending.get(event.data.requestId)
    if (!request) return
    if (event.data.type === 'wasm-memory') {
      const report = validateWasmMemoryReport(event.data)
      if (report) {
        state.wasmMemorySample = { ...report, active: true }
        renderSystemInfo()
        const display = document.querySelector('#wasm-memory-status')
        if (display) display.textContent = wasmMemoryDescription(state.wasmMemorySample)
      }
      return
    }
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
    const error = new Error(event.message || 'Analysis stopped unexpectedly')
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
  if (state.wasmMemorySample) state.wasmMemorySample = { ...state.wasmMemorySample, active: false }
}
function stopScienceWork() {
  if (activeIntegrativeRunController) activeIntegrativeRunController.abort()
  if (activeI2bController) { activeI2bController.abort(); state.i2bProgress = 'Stopping active source calculations; completed results will be kept.'; return }
  if (activeCompareController) {
    activeCompareController.abort()
    state.projectMessage = 'Comparison stopped. Completed modality results are retained.'
    return
  }
  if (!pending.size) return
  const error = new Error('Operation stopped. Allocated analysis memory was released.')
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
  return detectBrowserResources(navigator, globalThis.performance, Boolean(globalThis.crossOriginIsolated))
}

function renderSystemInfo() {
  state.systemInfo ||= detectSystemInfo()
  const chip = document.querySelector('#system-chip')
  if (!chip) return
  const info = state.systemInfo
  const threads = info.threads ? `${info.threads} logical CPU threads reported` : 'CPU information unavailable'
  const wasmSample = state.wasmMemorySample
  const wasmText = wasmSample ? ` · ${humanBytes(wasmSample.peakAllocatedBytes)} analysis memory ${wasmSample.active ? 'allocated' : 'last run peak'}` : ''
  const ramText = info.deviceMemoryGiB !== null ? `${info.memory} · ` : ''
  chip.innerHTML = `<span class="privacy-dot"></span><span><strong>${escapeHtml(info.platform)}</strong><small>${escapeHtml(ramText)}${escapeHtml(threads)} · On-device analysis${escapeHtml(wasmText)} · Settings</small></span>`
  chip.title = `Open Settings · CPU information is reported by your browser. Analysis memory is not total or free device RAM. RAM is reported only when the browser provides it.`
}

// Measure application memory automatically without adding host access or a service.
// These values are an application estimate or JS heap size, never free system RAM.
let memorySampleBusy = false
async function refreshAutomaticAppMemory() {
  if (memorySampleBusy || document.visibilityState === 'hidden') return
  memorySampleBusy = true
  try {
    state.appMemorySample = await measureAppMemory(globalThis.performance, Boolean(globalThis.crossOriginIsolated))
    renderSystemInfo()
    const display = document.querySelector('#automatic-memory-status')
    if (display) display.textContent = appMemoryDescription(state.appMemorySample)
  } finally {
    memorySampleBusy = false
  }
}

function appMemoryDescription(sample) {
  if (sample?.available) return `${humanBytes(sample.bytes)} · ${sample.source}`
  return sample?.source || 'Checking browser support…'
}

function wasmMemoryDescription(sample) {
  if (!sample) return 'Shown after an analysis starts.'
  const amount = humanBytes(sample.peakAllocatedBytes)
  return sample.active
    ? `${amount} allocated during analysis (highest observed)`
    : `${amount} highest observed in last analysis (released)`
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
  state.pathwayResult = null
  state.compareProgress = null
  state.longitudinalResult = null
  state.longitudinalProgress = null
  state.hitResult = null
  state.alignmentPreview = null
  state.alignmentTable = null
  state.bioResult = null
  state.bioFeatureResult = null
  state.i2Result = null
  state.i2StructureResult = null
  state.i2ImportedViews = []
  state.i2bDirectViews = []
  state.i2bErrors = []
  state.i2bProgressRows = []
  state.i2bProgress = ''
  state.alignmentError = ''
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

async function refreshFiles(nextFiles, preserveConfig = false) {
  const previousConfig = preserveConfig ? configSnapshot() : null
  state.files = nextFiles
  state.busy = true
  state.error = null
  render()
  try {
    const response = await preflightFiles([...nextFiles.values()].map(fileMetadata))
    state.records = response.records
    if (state.metadataEditor.table) {
      state.metadataEditor.mappings = autoMapFiles(
        state.records, state.metadataEditor.table, state.metadataEditor.idColumn, state.metadataEditor.mappings,
      )
    }
    state.wasmReady = response.wasmReady
    state.engine = response.wasmReady
      ? `4C Insight ${response.engineVersion || ''}`.trim()
      : 'Basic file check'
    resetQcStateAfterInputsChange()
    if (previousConfig) applyConfigSnapshot(previousConfig)
    applyPendingStateConfig()
    if (state.loadedStateManifest) state.projectMessage = reconnectMessage()
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.busy = false
    if (localSessionReady && !localSessionRestoring) queueLocalSourceCache()
    render()
  }
}

function addFiles(fileList) {
  if (!fileList || state.mergeBusy) return
  const next = new Map(state.files)
  for (const file of Array.from(fileList)) next.set(fileKey(file), file)
  void refreshFiles(next)
}


async function runExpressionMerge(kind) {
  if (state.mergeBusy || state.busy) return
  const settings = state.mergeSettings
  try {
    validateMergeConfig(kind, settings[kind], settings.order)
    const files = sortedMergeFiles(state.records, state.files, kind)
    if (!files.length) throw new Error(`No ${kind} workbooks are selected`)
    if (!state.wasmReady) throw new Error('The Completed science engine must be available to merge workbooks')
    state.mergeBusy = true
    state.busy = true
    state.mergeProgress = `Starting ${kind} merge (${files.length} workbooks)`
    state.error = null
    render()
    const response = await workerRequest('run-merge-expression', {
      kind, files, metrics: settings[kind], order: settings.order,
    }, event => {
      state.mergeProgress = event.message
      const status = document.querySelector('#merge-progress')
      if (status) status.textContent = state.mergeProgress
    })
    if (!response.file || !response.summary) throw new Error('The merge worker did not return a generated TSV')
    const newFiles = new Map(state.files)
    newFiles.set(fileKey(response.file), response.file)
    // The worker returns an OPFS-backed File. No download or second picker is
    // needed for Compare Groups, QC, Longitudinal, or Significant Hits.
    await refreshFiles(newFiles, true)
    if (kind === 'gene') {
      state.compareConfig.geneMergedId = fileKey(response.file)
      state.longitudinalConfig.geneMergedId = fileKey(response.file)
    } else {
      state.compareConfig.transcriptMergedId = fileKey(response.file)
      state.longitudinalConfig.transcriptMergedId = fileKey(response.file)
    }
    state.mergeReports[kind] = { ...response.summary, fileId: fileKey(response.file) }
    state.projectMessage = `Merged ${response.summary.samples} ${kind} workbooks into ${response.summary.name}. Canonical features: ${response.summary.rows}. Missing: ${response.summary.missingRows}; unknown: ${response.summary.unknownRows}.`
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  } finally {
    state.mergeBusy = false
    state.busy = false
    state.mergeProgress = ''
    render()
  }
}

async function downloadMergedTable(kind) {
  const report = state.mergeReports[kind]
  const file = report && selectedFile(report.fileId)
  if (!file) { state.error = 'The generated table is not available. Restore generated tables in Project.'; render(); return }
  const url = URL.createObjectURL(file)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = file.name
  document.body.appendChild(anchor)
  anchor.click()
  anchor.remove()
  // File-backed blob URL. Do not revoke before the browser starts its download.
  setTimeout(() => URL.revokeObjectURL(url), 60_000)
}

async function restoreMergedTables() {
  try {
    if (!navigator.storage?.getDirectory) throw new Error('Browser-local storage is unavailable')
    const root = await navigator.storage.getDirectory()
    const directory = await root.getDirectoryHandle('4c-insight-merged-tables')
    const next = new Map(state.files)
    let restored = 0
    for await (const [name, handle] of directory.entries()) {
      if (handle.kind !== 'file' || !/_merged_(gene|transcript)_expression\.tsv$/i.test(name)) continue
      const file = await handle.getFile()
      if (!next.has(fileKey(file))) { next.set(fileKey(file), file); restored++ }
    }
    await refreshFiles(next)
    state.projectMessage = `Restored ${restored} browser-generated merged tables. Raw XLSX inputs still require folder permission if they were saved in a prior state.`
  } catch (error) {
    state.error = error instanceof Error ? error.message : String(error)
  }
  render()
}

function removeFile(id) {
  if (state.mergeBusy) return
  const next = new Map(state.files)
  next.delete(id)
  void refreshFiles(next)
}

function clearProject() {
  if (state.mergeBusy) return
  state.mergeReports = { gene: null, transcript: null }
  state.metadataEditor = { ...state.metadataEditor, table: null, sourceName: '', sourceId: '', generatedId: '', mappings: {}, dirty: false, status: '' }
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
  state.pathwayResult = null
  state.compareProgress = null
  state.longitudinalResult = null
  state.longitudinalProgress = null
  state.hitResult = null
  state.alignmentPreview = null
  state.alignmentTable = null
  state.bioResult = null
  state.bioFeatureResult = null
  state.i2Result = null
  state.i2StructureResult = null
  state.i2ImportedViews = []
  state.i2bDirectViews = []
  state.i2bErrors = []
  state.i2bProgressRows = []
  state.i2bProgress = ''
  state.alignmentError = ''
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
  if (localSessionReady && !localSessionRestoring) queueLocalSourceCache()
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
  return { qc, compare, longitudinal, hit, merge: cloneJson(state.mergeSettings), thresholds: cloneJson(state.globalThresholds), fileRefs, pathway: cloneJson(state.pathwayConfig), biological: { annotationColumn: state.bioMetadataColumn, clusters: state.bioClusterCount, featureKind: state.bioFeatureKind } }
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
  if (workflow === 'compare-groups-expression' || workflow === 'compare-groups-multiomics') results.compare = state.compareResult
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
  if (configs.biological && typeof configs.biological === 'object') {
    if (typeof configs.biological.annotationColumn === 'string') state.bioMetadataColumn = configs.biological.annotationColumn
    const k = Number(configs.biological.clusters)
    if (Number.isInteger(k) && k >= 2 && k <= 12) state.bioClusterCount = k
    if (['gene','transcript'].includes(configs.biological.featureKind)) state.bioFeatureKind = configs.biological.featureKind
  }
  if (configs.pathway && typeof configs.pathway === 'object') state.pathwayConfig = { ...state.pathwayConfig, ...configs.pathway }
  if (configs.merge && typeof configs.merge === 'object') {
    for (const kind of ['gene', 'transcript']) {
      const metrics = configs.merge[kind]
      if (Array.isArray(metrics)) {
        try { validateMergeConfig(kind, metrics, configs.merge.order || state.mergeSettings.order); state.mergeSettings[kind] = [...metrics] } catch { /* do not restore invalid options */ }
      }
    }
    if (['adjacent-by-sample', 'grouped-by-measurement'].includes(configs.merge.order)) state.mergeSettings.order = configs.merge.order
  }
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

// Active browser-local session: distinct from portable JSON state and History.
// Large scientific results are cached by identity instead of copied on every render.
const ACTIVE_RESULT_FIELDS = [
  'qcResult', 'compareResult', 'longitudinalResult', 'hitResult',
  'alignmentPreview', 'alignmentTable', 'i2ImportedViews', 'i2bDirectViews',
  'i2Result', 'i2StructureResult', 'jointStabilityResult',
  'bioResult', 'bioFeatureResult', 'pathwayGeneSets', 'pathwayResult', 'mergeReports', 'metadataInspection',
]
let localSessionReady = false
let localSessionRestoring = false
let localSessionDisabled = false
let localSessionTimer = null
let localSessionWrite = Promise.resolve()
let localSourceWrite = Promise.resolve()
let localSourceRevision = 0
let localResultTokenNumber = 0
let localSessionMutationVersion = 0
const localResultReferences = new Map()
const localResultTokens = new Map()
const localResultFailures = new Map()
let localStorageNotice = 'Starting local session…'
let localStorageSeverity = 'info'

function updateLocalSessionNotice(message, severity = 'info') {
  localStorageNotice = String(message)
  localStorageSeverity = severity
  const display = document.querySelector('#local-session-status')
  if (display) {
    display.textContent = localStorageNotice
    display.title = localStorageNotice
    display.dataset.status = localStorageSeverity
  }
}
function activeSessionWorkRunning() {
  return state.busy || state.mergeBusy || state.alignmentBusy || state.i2bBusy ||
    state.i2Busy || state.i2StructureBusy || state.jointStabilityBusy ||
    state.bioFeatureBusy || state.integrativeRunBusy || state.pathwayBusy
}
function scheduleLocalSessionSave() {
  if (!localSessionReady || localSessionRestoring || localSessionDisabled || activeSessionWorkRunning()) return
  ++localSessionMutationVersion
  clearTimeout(localSessionTimer)
  localSessionTimer = setTimeout(() => {
    localSessionWrite = localSessionWrite.catch(() => {}).then(() => saveCurrentLocalSession())
  }, 1500)
}
function localSessionUiSnapshot() {
  return {
    tab: state.tab, integrativeView: state.integrativeView,
    resultsMode: state.resultsMode, alignmentSource: state.alignmentSource,
    alignmentRoles: { ...state.alignmentRoles }, alignmentColumns: [...state.alignmentColumns],
    bioMetadataColumn: state.bioMetadataColumn, bioClusterCount: state.bioClusterCount,
    pathwayConfig: { ...state.pathwayConfig },
    pathwaySelectedKey: state.pathwaySelectedKey,
    pathwayNetworkConfig: { ...state.pathwayNetworkConfig },
    pathwayLeadingOnly: state.pathwayLeadingOnly,
    bioFeatureAxis: state.bioFeatureAxis, bioSelectedProfile: state.bioSelectedProfile,
    bioSelectedSourceFeature: state.bioSelectedSourceFeature,
    bioInspectObservation: state.bioInspectObservation,
    i2bParallelWorkers: state.i2bParallelWorkers, i2MinViews: state.i2MinViews,
    i2StructureSelection: { ...state.i2StructureSelection },
    i2StructureOnePerPatient: state.i2StructureOnePerPatient,
    jointStabilityConfig: { ...state.jointStabilityConfig },
    integrativeRunSteps: (state.integrativeRunSteps || []).filter(step => step.status !== 'running'),
    metadataEditor: state.metadataEditor,
  }
}
async function saveCurrentLocalSession() {
  const writeRevision = localSessionMutationVersion
  if (!localSessionReady || localSessionRestoring || localSessionDisabled || activeSessionWorkRunning()) return
  let unavailable = 0
  for (const key of ACTIVE_RESULT_FIELDS) {
    const value = state[key] ?? null
    if (value == null) { localResultReferences.delete(key); localResultTokens.delete(key); localResultFailures.delete(key); continue }
    if (localResultReferences.get(key) === value) { if (localResultFailures.has(key)) unavailable++; continue }
    const token = `${Date.now()}-${++localResultTokenNumber}`
    try {
      await saveLocalResult(key, token, value)
      localResultReferences.set(key, value)
      localResultTokens.set(key, token)
      localResultFailures.delete(key)
    } catch (error) {
      localResultReferences.set(key, value)
      localResultTokens.delete(key) // Never restore stale results after a failed write.
      localResultFailures.set(key, String(error?.message || error))
      unavailable++
    }
  }
  if (writeRevision !== localSessionMutationVersion) { scheduleLocalSessionSave(); return }
  try {
    await saveLocalSession({
      format: '4c-insight-active-workspace-v1',
      payload: applicationStatePayload(),
      ui: localSessionUiSnapshot(),
      resultTokens: Object.fromEntries(localResultTokens),
      savedAt: new Date().toISOString(),
    })
    if (unavailable) updateLocalSessionNotice(`Settings saved locally; ${unavailable} result group(s) exceed browser storage or cannot be saved`, 'warning')
    else if (localStorageSeverity !== 'warning') updateLocalSessionNotice('Session saved on this device', 'ok')
  } catch (error) { updateLocalSessionNotice(`Automatic session save failed: ${error?.message || error}`, 'warning') }
}
function queueLocalSourceCache() {
  if (!localSessionReady || localSessionRestoring || localSessionDisabled) return
  const revision = ++localSourceRevision
  const files = new Map(state.files)
  localSourceWrite = localSourceWrite.catch(() => {}).then(async () => {
    if (localSessionDisabled || revision !== localSourceRevision) return
    let existing
    try { existing = new Set(await listStoredSourceIds()) }
    catch (error) { updateLocalSessionNotice(`Cannot cache source files: ${error?.message || error}`, 'warning'); return }
    const missing = [...files.entries()].filter(([id]) => !existing.has(id))
    let cached = 0
    let failed = null
    for (const [id, file] of missing) {
      if (localSessionDisabled || revision !== localSourceRevision) return
      try {
        await saveStoredSource(id, file, file.webkitRelativePath || file.name)
        cached++
        if (cached % 10 === 0 || cached === missing.length) updateLocalSessionNotice(`Saving local files ${cached}/${missing.length}; no upload`, 'info')
      } catch (error) { failed = String(error?.message || error); break }
    }
    if (localSessionDisabled || revision !== localSourceRevision) return
    const obsolete = [...existing].filter(id => !files.has(id))
    try { await removeStoredSources(obsolete) }
    catch (error) { failed = String(error?.message || error) }
    if (failed) updateLocalSessionNotice(`Some files cannot be cached (${failed}). Results may restore, but source files may need reconnection.`, 'warning')
    else if (missing.length && localStorageSeverity !== 'warning') updateLocalSessionNotice(`${files.size} local source files cached for refresh`, 'ok')
  })
}
function applyLocalSessionUi(saved) {
  if (!saved || typeof saved !== 'object') return
  if (['project', 'metadata', 'qc', 'compare', 'results', 'longitudinal', 'hits', 'ai', 'history', 'docs', 'settings'].includes(saved.tab)) state.tab = saved.tab
  if (['overview', 'details', 'pathways'].includes(saved.integrativeView)) state.integrativeView = saved.integrativeView
  if (['overview', 'details'].includes(saved.resultsMode)) state.resultsMode = saved.resultsMode
  if (['active', 'draft'].includes(saved.alignmentSource)) state.alignmentSource = saved.alignmentSource
  if (saved.alignmentRoles && typeof saved.alignmentRoles === 'object') {
    for (const role of ['observation', 'subject', 'time']) if (typeof saved.alignmentRoles[role] === 'string') state.alignmentRoles[role] = saved.alignmentRoles[role]
  }
  if (Array.isArray(saved.alignmentColumns)) state.alignmentColumns = saved.alignmentColumns.filter(value => typeof value === 'string')
  for (const key of ['bioMetadataColumn', 'bioFeatureAxis', 'bioSelectedProfile', 'bioSelectedSourceFeature', 'bioInspectObservation'])
    if (typeof saved[key] === 'string') state[key] = saved[key]
  const integer = (key, min, max) => { const n = Number(saved[key]); if (Number.isInteger(n) && n >= min && n <= max) state[key] = n }
  if (saved.pathwayConfig && typeof saved.pathwayConfig === 'object') state.pathwayConfig = {...state.pathwayConfig,...saved.pathwayConfig}
  if (typeof saved.pathwaySelectedKey === 'string' && saved.pathwaySelectedKey.length <= 5000) state.pathwaySelectedKey = saved.pathwaySelectedKey
  if (saved.pathwayNetworkConfig && typeof saved.pathwayNetworkConfig === 'object' && !Array.isArray(saved.pathwayNetworkConfig)) state.pathwayNetworkConfig = {...saved.pathwayNetworkConfig}
  if (typeof saved.pathwayLeadingOnly === 'boolean') state.pathwayLeadingOnly = saved.pathwayLeadingOnly
  integer('bioClusterCount', 2, 12); integer('i2MinViews', 2, 7); integer('i2bParallelWorkers', 1, 3)
  if (saved.i2StructureSelection && typeof saved.i2StructureSelection === 'object') state.i2StructureSelection = { ...saved.i2StructureSelection }
  if (typeof saved.i2StructureOnePerPatient === 'boolean') state.i2StructureOnePerPatient = saved.i2StructureOnePerPatient
  if (saved.jointStabilityConfig && typeof saved.jointStabilityConfig === 'object') {
    state.jointStabilityConfig = { ...state.jointStabilityConfig, ...saved.jointStabilityConfig }
  }
  if (Array.isArray(saved.integrativeRunSteps)) state.integrativeRunSteps = saved.integrativeRunSteps.map(step => ({ ...step, status: step.status === 'running' ? 'stopped' : step.status }))
  if (saved.metadataEditor && typeof saved.metadataEditor === 'object') state.metadataEditor = { ...state.metadataEditor, ...saved.metadataEditor }
}
async function restoreActiveLocalSession() {
  localSessionRestoring = true
  try {
    const saved = await readLocalSession()
    if (!saved) { updateLocalSessionNotice('Local auto-save ready', 'info'); return }
    if (!validLocalSession(saved)) throw new Error('Saved session has an unknown format; no data were changed')
    const entries = await readStoredSources()
    const manifest = saved.payload.project.files
    const expected = new Set(manifest.map(row => JSON.stringify([row.relativePath || row.name, row.size, row.lastModified])))
    const files = new Map()
    for (const entry of entries) {
      if (!expected.has(entry.id)) continue // Never attach a source from another project.
      const recovered = recoverSourceFile(entry, fileKey)
      if (recovered) files.set(...recovered)
    }
    state.loadedStateManifest = manifest
    state.pendingStateConfig = cloneJson(saved.payload.configs || {})
    if (files.size) await refreshFiles(files)
    applyPendingStateConfig()
    applyLocalSessionUi(saved.ui)
    const compatible = saved.payload.appVersion === APP_VERSION
    if (compatible) {
      const results = await readLocalResults(saved.resultTokens)
      for (const key of ACTIVE_RESULT_FIELDS) {
        if (!Object.hasOwn(results, key)) continue
        state[key] = results[key]
        localResultReferences.set(key, state[key])
        localResultTokens.set(key, saved.resultTokens[key])
      }
    }
    const missing = manifest.length - files.size
    state.projectMessage = !compatible
      ? 'Restored saved settings and available source files. Results were produced by another application revision and must be recalculated.'
      : missing > 0
        ? `Restored local session and available results. ${files.size} of ${manifest.length} source files are cached; reconnect ${missing} missing file(s) to repeat the analysis.`
        : `Restored local session and ${files.size} browser-cached source files.`
    updateLocalSessionNotice(!compatible ? 'Previous settings restored; rerun the updated science engine' : missing > 0 ? `${missing} source file(s) need reconnection; saved results restored` : 'Previous local session restored', !compatible || missing > 0 ? 'warning' : 'ok')
  } catch (error) {
    updateLocalSessionNotice(`Session restore unavailable: ${error?.message || error}`, 'warning')
  } finally {
    localSessionRestoring = false
    localSessionReady = true
    render()
  }
}
async function clearSavedActiveSession() {
  if (!window.confirm('Clear the saved 4C Insight session, browser-cached source files and active results? Your original files and separate History entries will not be deleted.')) return
  localSessionDisabled = true
  ++localSourceRevision
  clearTimeout(localSessionTimer)
  updateLocalSessionNotice('Clearing saved session…', 'info')
  try {
    await Promise.allSettled([localSessionWrite, localSourceWrite])
    await clearLocalSession()
    window.location.reload()
  } catch (error) {
    localSessionDisabled = false
    updateLocalSessionNotice(`Could not clear saved state: ${error?.message || error}`, 'warning')
  }
}

async function saveApplicationState() {
  const payload = applicationStatePayload()
  await saveTextFile('4c-insight-state.4cstate.json', `${JSON.stringify(payload, null, 2)}\n`)
  state.projectMessage = `Application state saved. It contains settings and ${payload.project.files.length.toLocaleString()} file references, not raw cohort data.`
  if (state.tab === 'project') render()
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
    state.pathwayResult = null
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
  if (workflow === 'compare-groups-expression' || workflow === 'compare-groups-multiomics') {
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
  state.resultsMode = 'overview'
  const tabForWorkflow = {
    'explore-qc-expression': 'qc',
    'compare-groups-expression': 'results',
    'compare-groups-multiomics': 'results',
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
    <div><strong>${escapeHtml(state.engine)}</strong><span>Engine status · App ${escapeHtml(APP_VERSION)}</span><span>Ready for analysis</span>${(pending.size || activeCompareController || activeI2bController) ? '<button id="stop-science-work" class="danger-text-button">Stop operation</button>' : ''}</div>`
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
      <div class="project-heading-actions"><div class="local-card"><strong>Private by design</strong><span>Files remain on this device. You choose when results or state are saved.</span></div></div>
    </section>
    ${renderProjectInputGuide()}
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
      ${summaryCard('Processing', state.busy ? 'Checking…' : 'Ready', reconnect ? `${reconnect.matched.toLocaleString()}/${expected.toLocaleString()} state files matched` : 'Checks selected files on this device')}
    </section>
    ${counts.length ? `<section class="card"><div class="card-heading"><div><h3>Detected data</h3><p>Review the automatic classification before you configure an analysis.</p></div></div><div class="modality-list">${counts.map(([label,count]) => `<div class="modality-pill"><span>${escapeHtml(label)}</span><strong>${count}</strong></div>`).join('')}</div></section>` : ''}
    ${projectMetadataSection()}
    ${projectMergeSection()}
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

function projectMetadataSection() {
  const metadataCount = recordsOfKind('metadata-candidate').length
  const discovered = matchingFiles(state.records).length
  return `<section class="action-panel" aria-label="Sample metadata workspace">
    <div class="action-copy"><h3>Sample Metadata</h3><p>View or create a sample sheet, edit cohort fields, and map ${discovered.toLocaleString()} individual report files to biological samples. ${metadataCount} metadata file(s) detected.</p></div>
    <div class="button-row"><button id="open-sample-metadata" class="secondary-button" type="button">Open metadata workspace</button></div>
  </section>`
}

function metadataEditorOrNull() { return state.metadataEditor.table }
function metadataSelectOptions(columns, chosen, includeBlank = false) {
  return `${includeBlank ? '<option value="">All columns</option>' : ''}${columns.map(column => `<option value="${escapeHtml(column)}" ${column === chosen ? 'selected' : ''}>${escapeHtml(column)}</option>`).join('')}`
}
function metadataFilteredRows() {
  const editor = state.metadataEditor
  if (!editor.table) return []
  return filterRows(editor.table, editor.filter, editor.filterColumn, editor.filterValue, editor.sortColumn, editor.descending)
}
function metadataFilterValues(editor) {
  const index = editor.table?.columns.indexOf(editor.filterColumn) ?? -1
  if (index < 0) return []
  return [...new Set(editor.table.rows.map(row => row[index]).filter(Boolean))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).slice(0, 250)
}
function virtualSpacer(colspan, height) {
  return height > 0 ? `<tr class="virtual-spacer" aria-hidden="true"><td colspan="${colspan}" style="height:${height}px"></td></tr>` : ''
}
function metadataTableRows() {
  const editor = state.metadataEditor
  const width = 62 + 188 * editor.table.columns.length + 108
  return `<div class="metadata-list-meta"><span class="metadata-viewport-status" role="status">Continuous sample table</span><span>Scroll to see all matching rows</span></div>
    <div class="table-scroll metadata-grid-scroll" data-virtual-kind="metadata" role="region" aria-label="Editable sample metadata" tabindex="0">
    <table class="metadata-editor-table" style="width:${width}px;table-layout:fixed"><thead><tr><th style="width:62px">Row</th>${editor.table.columns.map(column => `<th style="width:188px">${escapeHtml(column)} <button class="metadata-sort" data-column="${escapeHtml(column)}" type="button" aria-label="Sort by ${escapeHtml(column)}">${editor.sortColumn === column ? (editor.descending ? '↓' : '↑') : '↕'}</button></th>`).join('')}<th style="width:108px">Actions</th></tr></thead><tbody></tbody></table></div>`
}
function metadataMappingFilteredRows() {
  const editor = state.metadataEditor
  const filter = editor.mapFilter.toLocaleLowerCase()
  return matchingFiles(state.records).filter(record => {
    const matches = (record.relativePath || record.name).toLocaleLowerCase().includes(filter) || String(record.sampleId || '').toLocaleLowerCase().includes(filter)
    return matches && (!editor.mapUnmatched || !editor.mappings[record.id])
  })
}
function metadataMappingRows() {
  return `<div class="metadata-list-meta"><span class="metadata-viewport-status" role="status">Continuous file mapping table</span><span>Scroll to see all matching files</span></div>
  <div class="table-scroll metadata-grid-scroll" data-virtual-kind="mapping" role="region" aria-label="File to sample mapping" tabindex="0"><table class="metadata-editor-table metadata-file-map-table" style="width:1120px;table-layout:fixed"><thead><tr><th style="width:360px">File</th><th style="width:170px">Modality</th><th style="width:180px">Detected sample</th><th style="width:255px">Mapped biological sample</th><th style="width:155px">Status</th></tr></thead><tbody></tbody></table></div>`
}
function renderMetadataVisibleRow({ row, index }, columns) {
  return `<tr><td class="metadata-row-index">${index + 1}</td>${columns.map((column, col) => `<td><input class="metadata-cell" data-row="${index}" data-col="${col}" aria-label="${escapeHtml(column)} row ${index + 1}" value="${escapeHtml(row[col] ?? '')}" autocomplete="off" /></td>`).join('')}<td><button class="danger-text-button metadata-remove-row" data-row="${index}" aria-label="Remove row ${index + 1}" type="button">Remove</button></td></tr>`
}
function renderMetadataVisibleMapping(record) {
  const mappings = state.metadataEditor.mappings
  return `<tr><td title="${escapeHtml(record.relativePath || record.name)}">${escapeHtml(record.name)}</td><td>${escapeHtml(record.label || record.kind)}</td><td>${escapeHtml(record.sampleId || '—')}</td><td><input class="mapping-sample" data-file-id="${escapeHtml(record.id)}" list="sample-mapping-ids" value="${escapeHtml(mappings[record.id] || '')}" placeholder="Select or enter sample ID" aria-label="Sample for ${escapeHtml(record.name)}" /></td><td><span class="tag ${mappings[record.id] ? 'success' : 'warning'}">${mappings[record.id] ? 'Matched' : 'Unmapped'}</span></td></tr>`
}

/** Attach virtual scrolling to a stable holder. Filters and sort run once per
 * new view, not on every scroll event. Source indices survive sorting. */
function initializeMetadataViewport(holder, kind) {
  const scroller = holder.querySelector('.metadata-grid-scroll')
  const tbody = scroller?.querySelector('tbody')
  const status = holder.querySelector('.metadata-viewport-status')
  if (!scroller || !tbody || !status) return
  const rows = kind === 'metadata' ? metadataFilteredRows() : metadataMappingFilteredRows()
  const scrollProperty = kind === 'metadata' ? 'metadataScrollTop' : 'mappingScrollTop'
  scroller.scrollTop = state.metadataEditor[scrollProperty] || 0
  const cols = kind === 'metadata' ? state.metadataEditor.table.columns.length + 2 : 5
  let previous = null
  const renderWindow = () => {
    const window = tableWindow(rows.length, scroller.scrollTop, scroller.clientHeight || 520)
    if (previous && window.first === previous.first && window.end === previous.end) return
    // Do not discard uncommitted input when a focused row leaves the viewport.
    const active = document.activeElement
    if (active && scroller.contains(active) && active.matches('input')) active.blur()
    tbody.innerHTML = virtualSpacer(cols, window.topPadding)
      + rows.slice(window.first, window.end).map(item => kind === 'metadata'
        ? renderMetadataVisibleRow(item, state.metadataEditor.table.columns)
        : renderMetadataVisibleMapping(item)).join('')
      + virtualSpacer(cols, window.bottomPadding)
    status.textContent = `${tableRowRange(window)} · ${kind === 'metadata' ? state.metadataEditor.table.rows.length.toLocaleString() + ' total samples' : 'matching files'}`
    previous = window
  }
  scroller.addEventListener('scroll', () => {
    state.metadataEditor[scrollProperty] = scroller.scrollTop
    renderWindow()
  }, { passive: true })
  renderWindow()
}
function metadataView() {
  const editor = state.metadataEditor
  const table = metadataEditorOrNull()
  const candidates = recordsOfKind('metadata-candidate')
  const sampleReports = matchingFiles(state.records)
  const assessment = table ? validateMetadata(table, editor.idColumn) : null
  const summary = table ? mappingSummary(state.records, editor.mappings, table, editor.idColumn) : null
  const values = table ? metadataFilterValues(editor) : []
  return `<section class="page-heading"><div><span class="eyebrow">Project · Sample Metadata</span><h2>View, create, and edit sample metadata</h2><p>Review sample identity and group annotations, edit columns, and link individual files to samples. All changes stay on this device.</p></div><div class="local-card"><strong>One row per sample</strong><span>Export a TSV to keep your edits. No data is uploaded.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  <section class="card"><div class="card-heading"><div><h3>1. Start a metadata sheet</h3><p>Open an existing TSV, CSV, or Excel sample sheet, or build rows from detected expression and immune reports.</p></div>${table ? `<span class="tag ${editor.dirty ? 'warning' : 'success'}">${editor.dirty ? 'Unsaved draft' : 'Draft ready'}</span>` : ''}</div>
    <div class="form-grid metadata-start-grid"><label><span>Existing metadata file</span><select id="metadata-editor-source"><option value="">Select a file…</option>${candidates.map(record => `<option value="${escapeHtml(record.id)}" ${editor.sourceId === record.id ? 'selected' : ''}>${escapeHtml(record.name)}</option>`).join('')}</select></label>
    <div class="button-row metadata-start-buttons"><button id="metadata-load" class="secondary-button" ${!candidates.length || state.busy ? 'disabled' : ''}>Open selected metadata</button><button id="metadata-create" class="primary-button" ${state.busy ? 'disabled' : ''}>Create from ${sampleReports.length} files</button><button id="metadata-blank" class="secondary-button" ${state.busy ? 'disabled' : ''}>New empty sheet</button></div></div>
    <p class="method-note">Import uses the shared Completed metadata reader. Loading a new sheet replaces the current unsaved draft. Download edited metadata before you replace it.</p>
    ${editor.status ? `<div class="notice-banner" role="status"><strong>Metadata</strong><span>${escapeHtml(editor.status)}</span></div>` : ''}
  </section>
  ${table ? `<section class="summary-grid metadata-summary-grid" aria-label="Metadata checks">
    ${summaryCard('Samples', table.rows.length.toLocaleString(), 'Metadata rows')}
    ${summaryCard('Columns', table.columns.length, 'Editable fields')}
    ${summaryCard('ID problems', assessment.blankIds.length + assessment.duplicates.length, `${assessment.blankIds.length} blank · ${assessment.duplicates.length} duplicate`)}
    ${summaryCard('Files mapped', `${summary.matched}/${summary.matched + summary.missing}`, 'Exact sample assignment')}
  </section>
  ${assessment.errors.length || assessment.blankIds.length || assessment.duplicates.length ? `<div class="notice-banner" role="alert"><strong>Review sample IDs</strong><span>${escapeHtml([...assessment.errors, assessment.blankIds.length ? `${assessment.blankIds.length} blank ID row(s)` : '', assessment.duplicates.length ? `Duplicate: ${assessment.duplicates.slice(0, 5).join(', ')}` : ''].filter(Boolean).join(' · '))}</span></div>` : ''}
  <section class="card"><div class="card-heading"><div><h3>2. Edit sample metadata</h3><p>Search all rows, filter by a field, and edit cells directly. Scroll through all matching rows. Only visible rows are drawn to keep browser memory use low.</p></div><div class="button-row"><button id="metadata-add-row" class="secondary-button">Add sample</button><button id="metadata-download" class="primary-button" ${assessment.errors.length || assessment.blankIds.length || assessment.duplicates.length ? 'disabled' : ''}>Download metadata TSV</button><button id="metadata-use" class="secondary-button" ${assessment.errors.length || assessment.blankIds.length || assessment.duplicates.length || state.busy ? 'disabled' : ''}>Use in analyses</button></div></div>
    <div class="form-grid metadata-filter-grid"><label><span>Search all columns</span><input id="metadata-search" type="search" value="${escapeHtml(editor.filter)}" placeholder="Sample ID, group, batch…" /></label><label><span>Filter column</span><select id="metadata-filter-column">${metadataSelectOptions(table.columns, editor.filterColumn, true)}</select></label><label><span>Filter value</span><select id="metadata-filter-value"><option value="">All values</option>${values.map(value => `<option value="${escapeHtml(value)}" ${value === editor.filterValue ? 'selected' : ''}>${escapeHtml(value)}</option>`).join('')}</select></label></div>
    <p id="metadata-check-issues" class="method-note" role="status">${assessment.blankIds.length} blank sample IDs · ${assessment.duplicates.length} duplicate sample IDs</p>
    <div id="metadata-row-holder">${metadataTableRows()}</div>
    <details class="details-block metadata-columns"><summary>Manage columns and apply a value to multiple samples</summary><div class="form-grid metadata-action-grid"><label><span>Add a common field</span><select id="metadata-common-column"><option value="">Choose optional field…</option>${OPTIONAL_COLUMNS.filter(name => !table.columns.includes(name)).map(name => `<option value="${escapeHtml(name)}">${escapeHtml(name)}</option>`).join('')}</select></label><label><span>Custom column name</span><input id="metadata-new-column" placeholder="e.g. batch_id" /></label><button id="metadata-add-column" class="secondary-button" type="button">Add column</button></div>
    <div class="form-grid metadata-action-grid"><label><span>Existing column</span><select id="metadata-manage-column">${metadataSelectOptions(table.columns, editor.idColumn)}</select></label><label><span>New column name</span><input id="metadata-rename-input" placeholder="Enter new name" /></label><div class="button-row"><button id="metadata-rename-column" class="secondary-button">Rename</button><button id="metadata-remove-column" class="danger-text-button">Remove column</button></div></div>
    <div class="form-grid metadata-action-grid"><label><span>Bulk edit column</span><select id="metadata-bulk-column">${metadataSelectOptions(table.columns, 'analysis_group')}</select></label><label><span>Value for matching rows</span><input id="metadata-bulk-value" placeholder="e.g. Case" /></label><button id="metadata-bulk-apply" class="secondary-button">Apply to filtered rows</button></div>
    <p class="method-note">Bulk edits affect all filtered rows, including rows outside the visible viewport. Confirm the count before applying. Blank values clear the selected field.</p></details>
  </section>
  <section class="card"><div class="card-heading"><div><h3>3. Match files to samples</h3><p>Detected file IDs are matched to the selected metadata sample-ID column. Correct assignments manually when sample names differ.</p></div><div class="button-row"><button id="mapping-auto" class="secondary-button">Match by sample ID</button><button id="mapping-aliases" class="secondary-button">Add aliases to metadata</button><button id="mapping-download" class="secondary-button">Download file map TSV</button></div></div>
    <div class="form-grid metadata-filter-grid"><label><span>Sample identifier column</span><select id="metadata-id-column-editor">${metadataSelectOptions(table.columns, editor.idColumn)}</select></label><label><span>Find a file or detected ID</span><input id="mapping-search" type="search" value="${escapeHtml(editor.mapFilter)}" placeholder="Search ${sampleReports.length} reports…" /></label><label class="check-label metadata-check"><input id="mapping-unmatched" type="checkbox" ${editor.mapUnmatched ? 'checked' : ''} /> Only unmapped</label></div>
    <datalist id="sample-mapping-ids">${table.rows.map(row => String(row[table.columns.indexOf(editor.idColumn)] || '').trim()).filter(Boolean).map(id => `<option value="${escapeHtml(id)}"></option>`).join('')}</datalist>
    <div id="metadata-mapping-holder">${metadataMappingRows()}</div>
    <p class="method-note">File mappings are saved as a separate manifest for review and provenance. Existing analysis code still matches expression-table sample columns to the selected metadata ID. Exporting a file map does not rename sources or change fitted models.</p>
  </section>
  <div class="page-actions"><div><strong>Keep a permanent copy</strong><span>Download the metadata TSV and file map. Saved application state contains file references, not unsaved metadata cells.</span></div><button id="metadata-return-project" class="secondary-button">Back to Project</button></div>` : `<section class="card"><div class="empty-state"><strong>No metadata sheet open</strong><span>Start from detected files or select an existing metadata file.</span></div></section>`}`
}

function projectMergeSection() {
  const entries = ['gene', 'transcript'].map(kind => {
    const count = recordsOfKind(kind === 'gene' ? 'gene-expression' : 'transcript-expression').length
    const selected = state.mergeSettings[kind]
    const report = state.mergeReports[kind]
    return `<div class="config-block stacked">
      <div class="config-heading"><div><strong>${kind === 'gene' ? 'Merge gene expression' : 'Merge transcript expression'}</strong><span>${count} detected sample workbooks · first sorted workbook defines the feature universe</span></div><span class="tag ${count ? 'success' : 'neutral'}">${count ? 'Ready' : 'No workbooks'}</span></div>
      <div class="merge-metric-list">${mergeMetrics[kind].map(([value, label]) => `<label class="check-label"><input type="checkbox" class="merge-metric" data-kind="${kind}" value="${value}" ${selected.includes(value) ? 'checked' : ''} ${state.mergeBusy ? 'disabled' : ''} />${escapeHtml(label)}</label>`).join('')}</div>
      <div class="button-row"><button class="primary-button run-merge" data-kind="${kind}" ${count && state.wasmReady && !state.busy && !state.mergeBusy ? '' : 'disabled'}>Merge ${kind} workbooks</button>${report && selectedFile(report.fileId) ? `<button class="secondary-button download-merge" data-kind="${kind}">Download TSV</button>` : ''}</div>
      ${report ? `<small>${report.samples} samples · ${report.rows} features · missing ${report.missingRows} · unknown ${report.unknownRows} · ${humanBytes(report.bytes)}</small>` : ''}
    </div>`
  }).join('')
  return `<section class="card"><div class="card-heading"><div><h3>Merge individual expression workbooks</h3><p>Local Completed merger. One XLSX workbook at a time, with browser-local temporary files. Generated TSV tables are available immediately to downstream workflows.</p></div><span class="tag success">No upload</span></div>
    <div class="form-grid merge-settings-grid"><label><span>Output column order</span><select id="merge-column-order" ${state.mergeBusy ? 'disabled' : ''}>
      <option value="adjacent-by-sample" ${state.mergeSettings.order === 'adjacent-by-sample' ? 'selected' : ''}>Measurements adjacent by sample</option>
      <option value="grouped-by-measurement" ${state.mergeSettings.order === 'grouped-by-measurement' ? 'selected' : ''}>Samples grouped by measurement</option>
    </select></label></div>
    <div class="form-grid">${entries}</div>
    <div id="merge-progress" role="status" aria-live="polite">${escapeHtml(state.mergeProgress)}</div>
    ${state.mergeBusy ? '<button id="stop-merge" class="danger-text-button">Stop merging</button>' : ''}
    <div class="button-row"><button id="restore-merged" class="secondary-button" ${state.mergeBusy ? 'disabled' : ''}>Restore browser-generated tables</button></div>
    <p class="muted-note">First workbook determines canonical feature rows. Features absent from later workbooks are empty. Unknown later features are counted, not added. Temporary tables use browser origin storage; do not clear site data before downloading important results. Stop does not modify source files.</p>
  </section>`
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
    return `<section class="card"><div class="inline-status"><span class="spinner" aria-hidden="true"></span><div><strong>Reading metadata locally</strong><span>Reading the selected metadata file.</span></div></div></section>`
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
    <div class="card-heading"><div><h3>${title}</h3><p>${rows.length} samples · ${escapeHtml(rows[0].metric.replaceAll('-', ' '))} · sample workbooks processed one at a time</p></div><span class="tag success">Completed</span></div>
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

  return `<section class="page-heading"><div><span class="eyebrow">Explore & QC · W2</span><h2>Inspect metadata and expression quality</h2><p>Review the quality of each expression sample before you compare groups. Files are processed on your device, one at a time.</p></div><div class="local-card"><strong>No upload</strong><span>Your selected files remain on your device.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  ${!state.wasmReady ? `<div class="notice-banner" role="status"><strong>Analysis tools are not ready.</strong><span>Refresh this page. If the issue continues, check that the application is fully deployed.</span></div>` : ''}
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
  return `<div class="plot-toolbar" data-plot-navigation="${plotId}">${plotNavigationToolbar({compact:false})}</div>`
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
  return `<div class="plot-shell" data-interactive-plot="${plotId}" data-plot-nav-root="true" data-plot-kind="pca" data-width="${width}" data-height="${height}" data-pad="${pad}" data-min-x="${minX}" data-max-x="${maxX}" data-min-y="${minY}" data-max-y="${maxY}">
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
  return `<div class="plot-shell" data-interactive-plot="${plotId}" data-plot-nav-root="true" data-plot-kind="pca" data-width="${width}" data-height="${height}" data-pad="${pad}" data-min-x="${minX}" data-max-x="${maxX}" data-min-y="${minY}" data-max-y="${maxY}" data-volcano-fdr-y="${thresholdY}" data-volcano-fc="${fc}">
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
  return `<div class="plot-shell" data-interactive-plot="${plotId}" data-plot-nav-root="true" data-plot-kind="dendrogram" data-width="${width}" data-height="${height}">
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
    if(shell.dataset.plotNavigationMounted==='true')return
    const svg = shell.querySelector('.science-plot')
    if (!svg) return
    fitPlot(shell)
    shell.querySelectorAll('[data-plot-group-toggle]').forEach(button=>{
      button.addEventListener('click',()=>{
        const group=button.dataset.plotGroupToggle
        const hidden=button.getAttribute('aria-pressed')!=='false'
        button.setAttribute('aria-pressed',hidden?'false':'true')
        button.classList.toggle('is-hidden',hidden)
        shell.querySelectorAll(`[data-plot-group="${group}"]`).forEach(mark=>{mark.style.display=hidden?'none':''})
        if(shell.dataset.plotKind==='pca')renderPcaDomain(shell,shell._plotDomain||pcaInitialDomain(shell))
      })
    })
    const isPca=shell.dataset.plotKind==='pca'
    const pad=Number(shell.dataset.pad),width=Number(shell.dataset.width),height=Number(shell.dataset.height)
    const zoom=(factor,ax=.5,ay=.5)=>isPca?pcaZoom(shell,factor,ax,ay):dendrogramZoom(shell,factor,ax,ay)
    const pan=(dx,dy)=>{
      if(!isPca)return dendrogramPan(shell,dx,dy,svg)
      const bounds=svg.getBoundingClientRect()
      return pcaPan(shell,dx*width/Math.max(1,bounds.width),dy*height/Math.max(1,bounds.height))
    }
    const box=b=>{
      if(isPca){
        const current=shell._plotDomain||pcaInitialDomain(shell)
        const target=boxZoomDomain(current,b)
        if(!target)return
        // The maximum zoom is relative to the whole fitted coordinate domain.
        const original=pcaInitialDomain(shell)
        if((original.maxX-original.minX)/(target.maxX-target.minX)>50 ||
           (original.maxY-original.minY)/(target.maxY-target.minY)>50)return
        renderPcaDomain(shell,target)
      }else{
        const view=shell._plotViewBox||dendrogramInitialViewBox(shell)
        const next=boxZoomView(view,b)
        if(next&&next.width>=width/PLOT_ZOOM_MAX&&next.height>=height/PLOT_ZOOM_MAX)renderDendrogramViewBox(shell,next)
      }
    }
    const controller=bindPlotNavigation(svg,{
      plot:isPca?{left:pad,right:width-pad,top:pad,bottom:height-pad}:null,
      zoom,pan,box,reset:()=>fitPlot(shell)
    })
    bindPlotToolbar(shell,controller)
    shell.dataset.plotNavigationMounted='true'
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
  dynamicTableSources.set(tableId, { rows, cells: differentialRowCells, rowHtml: differentialRowHtml })
  return `<div class="table-scroll differential-table" data-dynamic-table="${escapeHtml(tableId)}"><table><thead><tr><th>Feature</th><th>Secondary ID</th><th>Case mean</th><th>Control mean</th><th>log2 group effect</th><th>SE</th><th>q-value</th><th>Subject effect</th><th>Subject q</th></tr></thead><tbody></tbody></table></div><p class="table-note">Search and sort operate across all ${rows.length.toLocaleString()} tested features. Scroll through all matching rows; only the visible rows are drawn. The JSON export contains all rows.</p>`
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
  dynamicTableSources.set(tableId, { rows, cells: dtuGeneRowCells, rowHtml: dtuGeneRowHtml })
  return `<div class="table-scroll" data-dynamic-table="${escapeHtml(tableId)}"><table><thead><tr><th>Gene</th><th>Transcripts</th><th>q-value</th><th>Max |CLR effect|</th></tr></thead><tbody></tbody></table></div><p class="table-note">Search and sort operate across all ${rows.length.toLocaleString()} tested genes. Scroll through all matching rows; only the visible rows are drawn.</p>`
}

function comparisonResultSection(title, result) {
  if (!result?.analysis) return ''
  const analysis = result.analysis
  const plotPrefix = title.toLowerCase().startsWith('gene') ? 'gene' : 'transcript'
  const caseLabel = result.comparison?.case_label || 'Case'
  const controlLabel = result.comparison?.control_label || 'Control'
  const significant = (analysis.tests || []).filter((row) => Number.isFinite(Number(row.q_value)) && Number(row.q_value) <= state.globalThresholds.fdr && Math.abs(Number(row.beta_group)) >= state.globalThresholds.absLog2Fc).length
  return `<section class="card comparison-result">
    <div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>${escapeHtml(result.source_metric)} · corrected count-based analysis</p></div><span class="tag success">Completed</span></div>
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

function otherOmicsControls() {
  const cfg = state.compareConfig
  return `<section class="card"><div class="card-heading"><div><h3>4. Repertoire, isotype, and immune analyses</h3><p>Select the receptor and immune analyses to include. If one data type is unavailable, the others can still run.</p></div><span class="tag neutral">Select analyses</span></div>
    <div class="config-grid expression-source-grid">${otherOmics.map(([kind, title, enabledKey, , description]) => {
      const files = recordsOfKind(kind)
      return `<div class="config-block ${files.length ? '' : 'muted-block'}"><div class="config-heading"><label class="check-label"><input id="compare-${kind}-enabled" type="checkbox" ${cfg[enabledKey] && files.length ? 'checked' : ''} ${files.length ? '' : 'disabled'} /><span><strong>${escapeHtml(title)}</strong><small>${escapeHtml(description)}</small></span></label><span class="tag ${files.length ? 'success' : 'neutral'}">${files.length} files</span></div></div>`
    }).join('')}</div>
    <details class="advanced-settings"><summary>Repertoire and composition settings</summary>
      <div class="form-grid advanced-grid"><label><span>V/D/J ambiguity handling</span><select id="compare-repertoire-policy">${[['exclude','Exclude ambiguous calls'],['split','Split weight'],['first','First reported hit'],['family','Collapse to gene family']].map(([value,label]) => `<option value="${value}" ${cfg.ambiguousPolicy === value ? 'selected' : ''}>${label}</option>`).join('')}</select></label>
      <label><span>Matched-depth rarefaction replicates</span><input id="compare-repertoire-reps" type="number" min="1" max="32" value="${cfg.repertoireRarefactionReps || 3}" /></label>
      <label><span>Maximum matched depth</span><input id="compare-repertoire-cap" type="number" min="100" max="1000000" value="${cfg.repertoireRarefactionCap || 50000}" /></label>
      <label><span>PERMANOVA permutations</span><input id="compare-repertoire-permutations" type="number" min="1" max="9999" value="${cfg.repertoirePermutations || 499}" /></label></div>
      <label class="check-label"><input id="compare-isotype-determined" type="checkbox" ${cfg.excludeUndeterminedIsotype ? 'checked' : ''} /><span>Exclude Undetermined from isotype composition and renormalize</span></label>
      <p>Change these settings only when they fit your study design. They affect receptor and composition comparisons.</p>
    </details>
  </section>`
}

function multiomicsResultSection(title, result, plotPrefix) {
  if (!result?.analysis) return ''
  const analysis = result.analysis
  const isRepertoire = Array.isArray(analysis.sample_summaries)
  const isImmune = Array.isArray(analysis.tests) && !!analysis.pca
  const tests = isRepertoire ? (analysis.metric_tests || []) : (analysis.tests || [])
  const ord = isRepertoire ? analysis.morisita_horn : (isImmune ? analysis.pca : analysis.ordination)
  const linkage = analysis.dendrogram
  const comps = isRepertoire ? (analysis.compositions || []) : []
  const compare = result.comparison || {}
  return `<section class="card comparison-result"><div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>Corrected analysis · ${isRepertoire ? 'matched-depth repertoire analysis' : isImmune ? 'ssGSEA signatures' : 'CLR isotype composition'}</p></div><span class="tag success">Completed</span></div>
    <div class="summary-grid compact-grid">${summaryCard('Usable samples', analysis.files_used || 0, 'Complete-case scientific inputs')}${summaryCard('Adjusted tests', tests.length, 'Adjusted group tests')}${isRepertoire ? summaryCard('Matched depth', analysis.matched_depth, `${analysis.rarefaction_reps} repetitions`) : ''}${summaryCard('Excluded metadata rows', compare.excluded_rows || 0, 'Applied before sample matching')}</div>
    ${(result.skippedFilesOutsideGroups || 0) ? `<p class="method-note">${result.skippedFilesOutsideGroups} source files were not in the selected case/control groups.</p>` : ''}
    ${analysis.warnings?.length ? `<div class="analysis-warnings">${analysis.warnings.map(w => `<div>${escapeHtml(w)}</div>`).join('')}</div>` : ''}
    <div class="plot-grid"><article class="plot-card"><div class="plot-heading"><strong>${isRepertoire ? 'Morisita–Horn ordination' : isImmune ? 'Immune signature PCA' : 'Isotype Aitchison ordination'}</strong><span>Sample structure; descriptive group comparisons.</span></div>${pcaSvg(ord, `${plotPrefix}-ordination`, compare.case_label || 'Case', compare.control_label || 'Control')}</article>
    <article class="plot-card"><div class="plot-heading"><strong>Hierarchical clustering</strong><span>Original modality-specific sample distances.</span></div>${dendrogramSvg(linkage, `${plotPrefix}-dendro`, compare.case_label || 'Case', compare.control_label || 'Control')}</article></div>
    <details class="details-block result-details" open><summary>Adjusted feature tests</summary>${differentialTable({tests}, `${plotPrefix}-tests`)}</details>
    ${isRepertoire ? `<details class="details-block result-details"><summary>Repertoire sample metrics</summary><div class="table-scroll"><table><thead><tr><th>Sample</th><th>Group</th><th>Richness</th><th>Shannon</th><th>Clonality</th><th>Private fraction</th><th>Public fraction</th></tr></thead><tbody>${analysis.sample_summaries.map(v => `<tr><td>${escapeHtml(v.sample_id)}</td><td>${escapeHtml(v.group)}</td><td>${formatNumber(v.observed_richness)}</td><td>${formatNumber(v.shannon)}</td><td>${formatNumber(v.clonality)}</td><td>${formatNumber(v.private_fraction)}</td><td>${formatNumber(v.public_fraction)}</td></tr>`).join('')}</tbody></table></div></details>${comps.map(c => `<details class="details-block result-details"><summary>${escapeHtml(c.family)} composition (${c.tests.length} tests)</summary>${differentialTable({tests:c.tests}, `${plotPrefix}-comp-${c.family.replace(/[^A-Za-z0-9]/g,'-')}`)}</details>`).join('')}` : ''}
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
  const modalityReady = (cfg.geneEnabled && cfg.geneMergedId) || (cfg.transcriptEnabled && cfg.transcriptMergedId) || otherOmics.some(([kind, , enabledKey]) => cfg[enabledKey] && recordsOfKind(kind).length > 0)
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

  return `<section class="page-heading"><div><span class="eyebrow">Compare Groups · W3</span><h2>Define a cohort comparison from metadata</h2><p>Select the metadata roles and group values that match this cohort. Select your groups and choose the analyses to run. Review adjusted differences, sample patterns, and clustering.</p></div><div class="local-card"><strong>Data stays on your device</strong><span>Your selected source files are not uploaded.</span></div></section>
  ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
  ${!metadataRecords.length ? '<div class="notice-banner"><strong>Metadata required.</strong><span>Select a metadata file in the Project tab before defining groups.</span></div>' : ''}
  ${metadata && (!metadataReady) ? `<div class="notice-banner"><strong>Resolve metadata IDs first.</strong><span>The selected ID column has ${metadata.blank_id_rows} blank row(s) and ${metadata.duplicate_id_values.length} duplicate value(s).</span></div>` : ''}
  ${overlap ? '<div class="notice-banner"><strong>Case and control selections overlap.</strong><span>Remove the same value from one group before running the comparison.</span></div>' : ''}
  ${incompleteExclusion ? `<div class="notice-banner"><strong>Finish or disable the optional exclusion.</strong><span>${escapeHtml(incompleteExclusion)}</span></div>` : ''}
  <section class="card"><div class="card-heading"><div><h3>1. Metadata and groups</h3><p>The sample-ID column maps expression data. The optional patient ID can use the same column when each patient has one sample.</p></div><span class="tag neutral">W3</span></div>
    <div class="config-grid compare-grid">
      <div class="config-block stacked"><div class="config-heading"><div><strong>Metadata roles</strong><span>Choose columns by meaning, not by hardcoded names.</span></div><span class="tag success">Flexible</span></div>
        <label><span>Metadata file</span><select id="compare-metadata-file">${metadataRecords.map((record) => `<option value="${escapeHtml(record.id)}" ${state.qcConfig.metadataId === record.id ? 'selected' : ''}>${escapeHtml(record.name)}</option>`).join('')}</select></label>
        <label><span>Expression/sample ID column</span><select id="compare-id-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns, state.qcConfig.metadataIdColumn) : '<option>Read metadata first</option>'}</select></label>
        <label><span>Patient/subject column for repeated measures</span><select id="compare-subject-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns, cfg.subjectColumn, true) : '<option>Read metadata first</option>'}</select></label>
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
  </div></section>
  ${otherOmicsControls()}
  <section class="card"><div class="card-heading"><div><h3>5. Run corrected comparison</h3><p>Enabled modalities are evaluated independently using the selected metadata groups.</p></div></div>
  <details class="advanced-settings"><summary>Advanced model settings</summary><div class="form-grid advanced-grid"><label><span>Minimum count</span><input id="compare-min-count" type="number" min="0" step="1" value="${cfg.minCount}"/></label><label><span>Minimum fraction of samples</span><input id="compare-min-fraction" type="number" min="0" max="1" step="0.01" value="${cfg.minFractionSamples}"/></label></div><p>Defaults match the established corrected-expression analysis. Change them only when the analysis plan requires it.</p></details>
  <div class="run-row"><div><strong>${cfg.caseValues.length} case value${cfg.caseValues.length === 1 ? '' : 's'} · ${cfg.controlValues.length} control value${cfg.controlValues.length === 1 ? '' : 's'}</strong><span>Group membership, exclusions, subject mapping, and model settings are frozen into the result.</span></div><button id="run-group-comparison" class="primary-button" ${runnable ? '' : 'disabled'}>${state.busy ? 'Running…' : 'Run local comparison'}</button></div></section>
  ${state.compareProgress ? `<div class="progress-card" role="status"><div class="progress-copy"><strong>${escapeHtml(state.compareProgress.message)}</strong><span>${state.compareProgress.completed} of ${state.compareProgress.total} modality steps complete</span></div><progress max="${Math.max(1, state.compareProgress.total)}" value="${state.compareProgress.completed}"></progress></div>` : ''}
  ${state.compareResult ? `<section class="card"><div class="card-heading"><div><h3>Comparison results are ready</h3><p>Review the main findings first, or open the full tables and statistics.</p></div><span class="tag success">Completed</span></div><button type="button" class="primary-button" data-open-results="overview">Open results</button></section>` : ''}`
}



function detailedComparisonHtml() {
  return `  ${state.compareResult ? `${globalThresholdControls()}<div class="result-heading"><div><span class="eyebrow">Corrected multi-omics result</span><h3>${escapeHtml(state.compareResult.comparisonName)}</h3><p>Generated locally. Expression PCA uses top-variable normalized features; other modalities retain their own scientific distances.</p></div><button id="export-comparison" class="secondary-button">Save comparison JSON</button></div>${comparisonResultSection('Gene expression', state.compareResult.gene)}${comparisonResultSection('Transcript expression', state.compareResult.transcript)}${otherOmics.map(([,title,,key]) => multiomicsResultSection(title, state.compareResult[key], key)).join('')}${Object.entries(state.compareResult.omicsErrors || {}).map(([name, message])=> `<div class="analysis-warnings"><strong>${escapeHtml(name)}</strong>: ${escapeHtml(message)}</div>`).join('')}` : ''}`
}

function resultsView() {
  const view = state.resultsMode || 'overview'
  const result = state.compareResult
  return `<section class="page-heading"><div><span class="eyebrow">Results</span><h2>${escapeHtml(result?.comparisonName || 'Cohort comparison')}</h2><p>Start with the main findings, or open all plots, statistics, and tables.</p></div><div class="local-card"><strong>Data stays on your device</strong><span>Analysis results remain in this browser until you export or save them.</span></div></section>
  <div class="results-view-switch" role="group" aria-label="Choose how to view results">
    <button type="button" data-results-view="overview" aria-pressed="${view === 'overview'}">Overview</button>
    <button type="button" data-results-view="detailed" aria-pressed="${view === 'detailed'}">Detailed Results</button>
    <button type="button" class="secondary-button" data-return-compare="true">Compare Groups</button>
  </div>
  ${!result ? '<section class="card"><h3>No comparison results yet</h3><p>Open Compare Groups to run an analysis, or open a previous comparison from History.</p></section>'
    : view === 'overview'
      ? `${globalThresholdControls()}${renderResultsOverview(result, {...state.globalThresholds, expandedModalities: state.resultsExpanded})}`
      : detailedComparisonHtml()}`
}
function bindResultsEvents() {
  document.querySelectorAll('[data-results-view]').forEach(button => button.addEventListener('click', () => {
    state.resultsMode = button.dataset.resultsView
    render()
  }))
  document.querySelector('[data-return-compare]')?.addEventListener('click', () => { state.tab = 'compare'; render() })
  document.querySelectorAll('[data-results-detail]').forEach(button => button.addEventListener('click', () => {
    const name = {gene:'Gene expression',transcript:'Transcript expression',bcr:'BCR repertoire',tcrAlpha:'TCR alpha',tcrBeta:'TCR beta',isotype:'BCR isotypes',immune:'Immune infiltration'}[button.dataset.resultsDetail]
    state.resultsMode='detailed'
    render()
    if (name) {
      const target=[...document.querySelectorAll('.comparison-result')].find(el => el.querySelector('h3')?.textContent?.trim() === name)
      target?.scrollIntoView?.({behavior:'smooth',block:'start'})
    }
  }))
  document.querySelector('#export-comparison')?.addEventListener('click', () => void exportCompareResult())
  bindGlobalThresholdEvents()
  bindInteractivePlots()
  attachStaticSvgNavigation(document)
  if (state.resultsMode === 'overview') {
    bindOverviewInteractions(document, state.compareResult, (key, expanded) => {
      const current = new Set(state.resultsExpanded)
      if (expanded) current.add(key)
      else current.delete(key)
      state.resultsExpanded = [...current]
    })
  } else {
    bindDetailedResultsExports(document, state.compareResult)
  }
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
  if (!cfg.patientIdColumn || !columns.includes(cfg.patientIdColumn)) {
    cfg.patientIdColumn = ['patient_id', 'sample_id', 'subject_id', 'participant_id']
      .find((column) => column !== cfg.observationIdColumn && columns.includes(column))
      || cfg.observationIdColumn
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

function longitudinalAnnotationCells(row) {
  return [row.sample_id, row.observation_id, row.patient_id, row.time_value, row.qc_value]
}
function longitudinalAnnotationRow(row) {
  return `<tr><td>${escapeHtml(row.sample_id)}</td><td>${escapeHtml(row.observation_id)}</td><td><strong>${escapeHtml(row.patient_id)}</strong></td><td>${escapeHtml(row.time_value || '—')}</td><td>${escapeHtml(row.qc_value || '—')}</td></tr>`
}

function longitudinalResultSection(title, result) {
  if (!result?.modality) return ''
  const modality = result.modality
  const warnings = [...new Set([...(result.warnings || []), ...(modality.warnings || [])])]
  const annotationId = title.toLowerCase().startsWith('gene') ? 'gene-annotations' : 'transcript-annotations'
  dynamicTableSources.set(annotationId, { rows: modality.annotations || [], cells: longitudinalAnnotationCells, rowHtml: longitudinalAnnotationRow })
  return `<section class="card comparison-result longitudinal-result"><div class="card-heading"><div><h3>${escapeHtml(title)}</h3><p>${escapeHtml(modality.distance_label)} · patient identity is not used to construct the distance matrix.</p></div><span class="tag success">Completed</span></div>
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
    <details class="details-block result-details"><summary>Sample annotations (${modality.annotations.length})</summary><div class="table-scroll longitudinal-table" data-dynamic-table="${annotationId}"><table><thead><tr><th>Data sample</th><th>Metadata observation</th><th>Patient/person</th><th>Time / visit</th><th>QC value</th></tr></thead><tbody></tbody></table></div></details>
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
  const rolesReady = Boolean(metadata && cfg.observationIdColumn && cfg.patientIdColumn)
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
  ${cfg.observationIdColumn && cfg.observationIdColumn === cfg.patientIdColumn ? '<p class="method-note">One column identifies both the sample and the patient. This is valid for one observation per patient. Repeated-patient identity scores need two or more distinct observations per patient.</p>' : ''}
  ${incompleteExclusion ? `<div class="notice-banner"><strong>Finish or disable the optional exclusion.</strong><span>${escapeHtml(incompleteExclusion)}</span></div>` : ''}
  <section class="card"><div class="card-heading"><div><h3>1. Map longitudinal metadata roles</h3><p>Column names are not assumed. This supports sheets where, for example, subject_id is the longitudinal sample and sample_id is the patient.</p></div><span class="tag neutral">W4</span></div><div class="config-grid longitudinal-role-grid">
    <div class="config-block stacked"><label><span>Metadata file</span><select id="longitudinal-metadata-file">${metadataRecords.map((record) => `<option value="${escapeHtml(record.id)}" ${state.qcConfig.metadataId === record.id ? 'selected' : ''}>${escapeHtml(record.name)}</option>`).join('')}</select></label><label><span>Observation / longitudinal sample ID</span><select id="longitudinal-observation-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns, cfg.observationIdColumn) : '<option>Read metadata first</option>'}</select></label></div>
    <div class="config-block stacked"><label><span>Patient / person ID</span><select id="longitudinal-patient-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns, cfg.patientIdColumn) : '<option>Read metadata first</option>'}</select></label><label><span>Collection date / timepoint / visit</span><select id="longitudinal-time-column" ${metadata ? '' : 'disabled'}>${metadata ? columnOptions(columns.filter((column) => column !== cfg.observationIdColumn && column !== cfg.patientIdColumn), cfg.timeColumn, true) : '<option>Read metadata first</option>'}</select></label></div>
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
    state.i2Result = null
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
  return `<section class="card"><div class="card-heading"><div><h3>Gene and transcript selected-hit structure comparison</h3><p>This is a descriptive comparison of the post-selection structures. It is not a predictive ranking or independent validation.</p></div><span class="tag warning">Descriptive</span></div><div class="table-scroll"><table><thead><tr><th>Data type</th><th>Significant hits</th><th>Features analyzed</th><th>Silhouette</th><th>NN group accuracy</th><th>PERMANOVA p</th></tr></thead><tbody>${rows.map(({ label, panel }) => `<tr><td><strong>${escapeHtml(label)}</strong></td><td>${panel.significant_features_total}</td><td>${panel.analysis_features.length}</td><td>${formatNumber(panel.silhouette)}</td><td>${Number.isFinite(Number(panel.nearest_neighbor_group_accuracy)) ? `${(Number(panel.nearest_neighbor_group_accuracy) * 100).toFixed(1)}%` : '—'}</td><td>${formatNumber(panel.pca?.permanova_p)}</td></tr>`).join('')}</tbody></table></div></section>`
}

function renderIntegrativeEvidenceOverview() {
  const cfg = state.hitConfig
  const rows = summarizeIntegrativeEvidence(state.compareResult, {
    ...state.globalThresholds,
    omicsMinAdjustedEffect: cfg.omicsMinAdjustedEffect ?? 0,
  }, cfg.integrativeModalities || {})
  const totals = integrativeSummaryTotals(rows)
  const controls = rows.map(row => `<label><input type="checkbox" data-integrative-modality="${row.key}" ${row.included ? 'checked' : ''} ${row.available ? '' : 'disabled'}/><span>${escapeHtml(row.label)}${row.available ? '' : ' (no result)'}</span></label>`).join('')
  const body = rows.filter(row => row.included).map(row => `<tr><td><strong>${escapeHtml(row.label)}</strong></td><td>${row.available ? 'Available' : 'Not available'}</td><td>${row.samples === null ? '—' : row.samples.toLocaleString()}</td><td>${row.available ? row.tested.toLocaleString() : '—'}</td><td>${row.available ? row.selected.toLocaleString() : '—'}</td><td>${escapeHtml(row.effectType)}</td></tr>`).join('')
  return `<section class="card integrative-evidence" id="integrative-evidence"><div class="card-heading"><div><h3>Corrected evidence across data types</h3><p>Compare previously calculated findings across all available data types. Choose which results to include below.</p></div><span class="tag neutral">Evidence summary</span></div>
    <div class="summary-grid compact-grid">${summaryCard('Selected data types', totals.included, `${totals.available} with completed results`)}${summaryCard('Adjusted tests available', totals.adjustedTests.toLocaleString(), 'Across selected data types')}${summaryCard('Selected adjusted findings', totals.selectedTests.toLocaleString(), 'Using each data type’s thresholds')}</div>
    <div class="integrative-selector" role="group" aria-label="Data types to review">${controls}</div>
    ${totals.included ? `<div class="table-scroll"><table class="integrative-evidence-table"><thead><tr><th>Data type</th><th>Result</th><th>Samples</th><th>Adjusted tests</th><th>Selected findings</th><th>Effect interpretation</th></tr></thead><tbody>${body}</tbody></table></div>` : '<p class="integrative-message">Select one or more available data types to review.</p>'}
    <div class="run-row"><div><strong>Selected findings from all included data types</strong><span>Save a complete table with modality names, feature families, original q-values, and effects.</span></div><button id="integrative-export-csv" class="secondary-button" type="button" ${totals.selectedTests ? '' : 'disabled'}>Save combined evidence CSV</button></div>
    <p class="integrative-message">These are results from separate adjusted analyses, not one joint statistical model. BH q-values are corrected within their original test families. Effect sizes use different scales and must not be compared directly across data types.</p>
    <p class="integrative-message"><strong>Joint sample-level distance fusion is available below after exact source alignment. Direct source distances are available for all seven data types. A separate feature-level latent-factor model is not fitted; sample alignment checks identities and source availability only.</strong> It needs sample alignment, missing-data checks, and validated combined-distance or latent-factor calculations.</p>
  </section>`
}

function renderAdjustedOmicsReview() {
  const cfg = state.hitConfig
  const raw = Number(cfg.omicsMinAdjustedEffect ?? 0)
  const minEffect = Number.isFinite(raw) && raw >= 0 ? raw : 0
  const groups = collectAdjustedTests(state.compareResult).filter(group => cfg.integrativeModalities?.[group.key] !== false)
  const available = groups.filter(group => group.available).length
  const selectedRows = []
  const sections = groups.map(group => {
    const selected = selectAdjustedTests(group.rows, state.globalThresholds.fdr, minEffect)
    selectedRows.push(...selected)
    const tag = !group.available ? 'Run Compare Groups first' : !group.rows.length ? 'No adjusted tests' : `${selected.length} of ${group.rows.length} selected`
    let detail = ''
    if (selected.length) {
      const tableId = `h0-adjusted-${group.key}`
      const cells = row => [row.family, row.feature, row.secondary_id, row.adjusted_effect, row.q_value, row.p_value]
      const rowHtml = row => `<tr><td>${escapeHtml(row.family)}</td><td><strong>${escapeHtml(row.feature)}</strong></td><td>${escapeHtml(row.secondary_id)}</td><td>${formatNumber(row.adjusted_effect)}</td><td>${formatNumber(row.q_value)}</td><td>${formatNumber(row.p_value)}</td></tr>`
      dynamicTableSources.set(tableId, { rows: selected, cells, rowHtml })
      detail = `<details class="details-block result-details"><summary>View ${selected.length} selected features</summary><div class="table-scroll differential-table h0-hits-scroll" data-dynamic-table="${tableId}"><table><thead><tr><th>Feature family</th><th>Feature</th><th>Secondary ID</th><th>Adjusted group effect</th><th>BH q-value</th><th>p-value</th></tr></thead><tbody></tbody></table></div><p class="table-note">Search and sort apply to all selected features. CSV export includes the full selection.</p></details>`
    }
    return `<div class="config-block h0-modality-card ${group.available ? '' : 'muted-block'}"><div class="config-heading"><strong>${escapeHtml(group.label)}</strong><span class="tag ${group.available ? 'neutral' : 'warning'}">${escapeHtml(tag)}</span></div>${!group.available ? '<small>No corrected result is available for this data type.</small>' : !group.rows.length ? '<small>The corrected model did not produce adjusted feature tests for this data type.</small>' : !selected.length ? '<small>No adjusted features pass the current FDR and adjusted-effect thresholds.</small>' : ''}${detail}</div>`
  }).join('')
  return `<section class="card"><div class="card-heading"><div><h3>4. Receptor and immune adjusted findings</h3><p>Review adjusted tests already calculated in Compare Groups. This table does not run a second model.</p></div><span class="tag neutral">${available}/5 completed</span></div>
    <div class="form-grid hit-settings-grid"><label><span>Minimum |adjusted effect| for receptor, isotype and immune tests</span><input id="hit-omics-min-effect" type="number" min="0" step="0.1" value="${minEffect}"/></label><div><strong>FDR threshold</strong><p class="method-note">The global BH q-value limit is ${formatNumber(state.globalThresholds.fdr)}. These adjusted effects are not all log2 fold changes.</p></div></div>
    <p class="method-note">Full selected-hit PCA and clustering for these modalities are not yet available. Do not interpret the global ordination from Compare Groups as a selected-hit analysis.</p>
    <div class="h0-modality-grid">${sections}</div>
    <div class="run-row"><div><strong>${selectedRows.length} selected adjusted tests across ${available} completed data types</strong><span>Selection uses finite BH q-values and adjusted group effects, without a new model fit.</span></div><button id="export-omics-adjusted-hits" class="secondary-button" type="button" ${selectedRows.length ? '' : 'disabled'}>Save selected hits CSV</button></div>
  </section>`
}

// I1 aligns observations across source inventories. It does not fit a joint model.
function i1ExclusionRules() {
  const cfg = state.compareConfig
  return [
    { enabled: cfg.qcExclusionEnabled, label: 'Reviewed QC exclusion', column: cfg.qcExclusionColumn, values: cfg.qcExclusionValues || [] },
    { enabled: cfg.customExclusionEnabled, label: 'Custom metadata exclusion', column: cfg.customExclusionColumn, values: cfg.customExclusionValues || [] },
  ]
}
function i1SelectionSummary() {
  const roles=state.alignmentRoles
  const metadataName=state.alignmentSource==='draft' ? 'Open local metadata draft' :
    (selectedFile(state.qcConfig.metadataId)?.name || 'No metadata file connected')
  const selected=I1_MODALITIES.filter(m=>state.hitConfig.integrativeModalities?.[m.key]!==false)
  return `<div class="method-note i1-selection-summary" aria-label="Current sample alignment settings">
    <strong>Alignment settings from Overview</strong>
    <p>Metadata: ${escapeHtml(metadataName)} · Observation: ${escapeHtml(roles.observation||'Not selected')} · Patient: ${escapeHtml(roles.subject||'Not selected')} · Timepoint: ${escapeHtml(roles.time||'Not selected')}</p>
    <p>${selected.length} data types selected. Change the identity columns or data types in Main analysis settings above.</p>
  </div>`
}
function i1AlignmentPanel() {
  const preview = state.alignmentPreview
  const activeFile = selectedFile(state.qcConfig.metadataId)
  const hasDraft = Boolean(state.metadataEditor.table)
  let results = ''
  if (preview) {
    const counts = preview.summary.counts.filter(c => c.selected)
    const heads = I1_MODALITIES.map(m => `<th>${escapeHtml(m.label)}<small>Source status</small></th>`).join('')
    dynamicTableSources.set('i1-alignment-table', {
      rows: preview.rows,
      cells: row => [row.observation, row.subject, row.timepoint, row.excluded ? 'Excluded' : 'Included', ...I1_MODALITIES.map(m => row.modalities[m.key].status)],
      rowHtml: row => `<tr><td><strong>${escapeHtml(row.observation)}</strong></td><td>${escapeHtml(row.subject || '—')}</td><td>${escapeHtml(row.timepoint || '—')}</td><td>${row.excluded ? `<span class="tag warning" title="${escapeHtml(row.exclusions.join(' | '))}">Excluded</span>` : 'Included'}</td>${I1_MODALITIES.map(m => { const v = row.modalities[m.key]; return `<td title="${escapeHtml(v.source || v.alias)}"><span class="i1-source-status i1-status-${escapeHtml(v.status)}">${escapeHtml(v.status)}</span><small>${escapeHtml(v.alias)}</small></td>` }).join('')}</tr>`,
    })
    results = `<div class="summary-grid compact-grid">
      ${summaryCard('Metadata observations', preview.summary.total.toLocaleString(), `${preview.summary.excluded} excluded by current rules`)}
      ${summaryCard('Eligible observations', preview.summary.eligible.toLocaleString(), 'After metadata exclusions')}
      ${summaryCard('At least two source types', preview.summary.pairedTwo.toLocaleString(), 'Exact links; not paired biological measures')}
      ${summaryCard('Complete selected coverage', preview.summary.complete.toLocaleString(), 'Present in every selected source type')}
    </div>
    <div class="table-scroll i1-coverage-scroll"><table><thead><tr><th>Data type</th><th>Matched</th><th>Missing</th><th>Ambiguous</th><th>Source error</th></tr></thead><tbody>${counts.map(c => `<tr><td>${escapeHtml(c.label)}</td><td>${c.present}</td><td>${c.missing}</td><td>${c.ambiguous}</td><td>${c['source-error']}</td></tr>`).join('')}</tbody></table></div>
    ${preview.warnings.length ? `<details class="details-block"><summary>Review ${preview.warnings.length.toLocaleString()} alignment warning(s)</summary><div class="analysis-warnings">${preview.warnings.map(message => `<div>${escapeHtml(message)}</div>`).join('')}</div></details>` : ''}
    <div class="table-scroll i1-alignment-scroll" data-dynamic-table="i1-alignment-table" role="region" aria-label="All aligned observations" tabindex="0"><table><thead><tr><th>Observation</th><th>Patient</th><th>Visit</th><th>Eligibility</th>${heads}</tr></thead><tbody></tbody></table></div>
    <div class="button-row"><button type="button" class="secondary-button" id="i1-export-json">Save exact alignment JSON</button><button type="button" class="secondary-button" id="i1-export-tsv">Save review TSV</button></div>
    <p class="method-note">The JSON keeps exact source IDs for later joint modeling. The TSV protects spreadsheet cells that look like formulas. Source presence does not prove that a sample passed quality control or was used in a fitted model.</p>`
  }
  return `<section class="card i1-alignment" id="i1-observation-alignment"><div class="card-heading"><div><h3>Sample alignment across data types</h3><p>Match exact sample IDs, patient IDs, visits and source aliases. Review missing data and excluded observations before any joint analysis.</p></div><span class="tag neutral">Sample identity</span></div>
    ${i1SelectionSummary()}
    ${!activeFile && state.alignmentSource !== 'draft' ? `<p class="notice-banner">Select an analysis metadata file in Project before alignment.</p>` : ''}
    <p class="method-note">One column may identify both the observation and patient when sample IDs are unique. For repeated visits, select unique observation IDs and a shared patient ID column. A timepoint must use a separate column.</p>
    <p class="method-note">Uses the two current Compare Groups exclusion rules, when enabled. Matches are case-sensitive and exact after trimming surrounding spaces. An explicit alias is never replaced by a guessed ID. A source conflict is not resolved automatically.</p>
    <div class="run-row"><div><strong>Build an observation-level alignment manifest</strong><span>Reads metadata and only the header of each selected merged expression table; raw sample workbooks are not opened.</span></div><button id="i1-run-alignment" class="primary-button" type="button" ${state.alignmentBusy || state.busy || state.mergeBusy || (state.alignmentSource !== 'draft' && !activeFile) ? 'disabled' : ''}>${state.alignmentBusy ? 'Checking sources…' : 'Check sample alignment'}</button></div>
    ${state.alignmentError ? `<div class="analysis-warnings" role="alert">${escapeHtml(state.alignmentError)}</div>` : ''}
    ${results}
    <p class="integrative-message"><strong>Scientific limit:</strong> Sample alignment checks identities and source availability only. It does not read quantitative feature matrices, normalize joint distances, impute missing modalities or fit a multi-omics model. Joint sample analysis below can combine full unsupervised sample distances; native/browser parity and source-level validation are required.</p>
  </section>`
}
function i1ResetPreview() { state.alignmentPreview = null; state.alignmentTable = null; state.bioResult = null; state.bioFeatureResult = null; state.alignmentError = ''; state.i2Result = null; state.i2bDirectViews = []; state.i2bErrors = []; state.i2bProgressRows = []; state.i2bProgress = ''; state.i2StructureResult = null }
async function i1RunAlignment() {
  if (state.alignmentBusy || state.busy || state.mergeBusy) return
  const draft = state.alignmentSource === 'draft'
  const metadataFile = selectedFile(state.qcConfig.metadataId)
  if (!draft && !metadataFile) { state.alignmentError = 'Select the active metadata file first'; render(); return }
  state.alignmentBusy = true
  i1ResetPreview()
  render()
  try {
    let table
    if (draft) {
      if (!state.metadataEditor.table) throw new Error('Open a metadata draft first')
      table = state.metadataEditor.table
    } else {
      const response = await workerRequest('load-metadata-table', { file: metadataFile })
      table = response.table
    }
    const merged = {}
    for (const [key, kind] of [['gene', 'merged-gene-expression'], ['transcript', 'merged-transcript-expression']]) {
      const candidates = recordsOfKind(kind)
      const chosenId = key === 'gene' ? state.compareConfig.geneMergedId : state.compareConfig.transcriptMergedId
      const record = candidates.find(item => item.id === chosenId) || (candidates.length === 1 ? candidates[0] : null)
      if (!record && candidates.length > 1) { merged[key] = { error: 'Several merged tables were found. Select one in Compare Groups.' }; continue }
      if (!record) continue
      const file = selectedFile(record.id)
      if (!file) { merged[key] = { error: 'Selected merged file is not connected' }; continue }
      try {
        const header = await readFirstTsvRecord(file)
        merged[key] = { sampleIds: mergedSampleIds(header, key), source: record.relativePath || record.name }
      } catch (error) {
        merged[key] = { error: error instanceof Error ? error.message : String(error), source: record.name }
      }
    }
    const columns = table.columns || []
    state.alignmentColumns = [...columns]
    if (!state.alignmentRoles.observation) {
      const fromQc = state.qcConfig.metadataIdColumn || 'sample_id'
      if (columns.includes(fromQc)) state.alignmentRoles.observation = fromQc
    }
    if (!state.alignmentRoles.subject && columns.includes(state.compareConfig.subjectColumn)) {
      state.alignmentRoles.subject = state.compareConfig.subjectColumn
    }
    if (!state.alignmentRoles.time && columns.includes(state.longitudinalConfig.timeColumn)) {
      state.alignmentRoles.time = state.longitudinalConfig.timeColumn
    }
    const observationColumn = state.alignmentRoles.observation
    if (!observationColumn) throw new Error('Choose an observation ID column in the alignment settings')
    const preview = alignObservations({
      table, records: state.records, merged,
      observationColumn,
      subjectColumn: state.alignmentRoles.subject,
      timeColumn: state.alignmentRoles.time,
      exclusions: i1ExclusionRules(),
      enabled: state.hitConfig.integrativeModalities || {},
    })
    preview.metadataSource = draft ? `Open local metadata draft: ${state.metadataEditor.sourceName || 'untitled'}` : `Active analysis metadata: ${metadataFile.name}`
    preview.mergedSources = Object.fromEntries(Object.entries(merged).map(([key, v]) => [key, { source: v.source || '', error: v.error || null }]))
    state.alignmentPreview = preview
    state.alignmentTable = table
  } catch (error) {
    state.alignmentError = `Sample alignment: ${error instanceof Error ? error.message : String(error)}`
  } finally { state.alignmentBusy = false; render() }
}
function bindI1AlignmentEvents() {
  document.querySelector('#i1-run-alignment')?.addEventListener('click', () => void i1RunAlignment())
  document.querySelector('#i1-export-json')?.addEventListener('click', () => {
    if (!state.alignmentPreview) return
    void saveTextFile('4c-insight-observation-alignment.json', JSON.stringify(state.alignmentPreview, null, 2) + '\n')
      .catch(error => { state.alignmentError = error.message; render() })
  })
  document.querySelector('#i1-export-tsv')?.addEventListener('click', () => {
    if (!state.alignmentPreview) return
    void saveTextFile('4c-insight-observation-alignment-review.tsv', alignmentTsv(state.alignmentPreview))
      .catch(error => { state.alignmentError = error.message; render() })
  })
}


// I2A uses full, unsupervised W4 sample distances. I1 maps exact observation IDs.
function i2AvailableViews() {
  const local = state.longitudinalResult ? extractFullDistanceViews(state.longitudinalResult) : []
  const merged = new Map(local.map(view => [view.modality, view]))
  for (const view of state.i2ImportedViews) merged.set(view.modality, view)
  for (const view of state.i2bDirectViews) merged.set(view.modality, view)
  return [...merged.values()]
}
function i2HeatmapSvg(result) {
  const observations = result.observations.slice(0, 28)
  if (observations.length < 2) return '<p>No eligible samples for a heatmap.</p>'
  const pos = new Map(observations.map((id, i) => [id, i]))
  const points = new Map()
  let max = 0
  for (const pair of result.pairs) {
    if (!pos.has(pair.a) || !pos.has(pair.b)) continue
    const k = `${pos.get(pair.a)}:${pos.get(pair.b)}`
    points.set(k, pair.distance)
    points.set(`${pos.get(pair.b)}:${pos.get(pair.a)}`, pair.distance)
    max = Math.max(max, pair.distance)
  }
  const cell = 13, left = 136, top = 118, length = observations.length * cell
  const width = left + length + 18, height = top + length + 25
  const labels = observations.map((id, i) => `<text x="${left - 7}" y="${top + i * cell + 9}" text-anchor="end" font-size="9">${escapeHtml(id.slice(0, 24))}</text><text transform="translate(${left + i*cell + 8},${top - 6}) rotate(-65)" text-anchor="start" font-size="9">${escapeHtml(id.slice(0, 24))}</text>`).join('')
  const rectangles = []
  for (let i = 0; i < observations.length; i++) for (let j = 0; j < observations.length; j++) {
    const d = i === j ? 0 : points.get(`${i}:${j}`)
    const exists = Number.isFinite(d)
    const t = exists && max ? Math.min(1, Math.max(0, d / max)) : 0
    const shade = Math.round(252 - 180 * t)
    const color = !exists ? '#eceff3' : i === j ? '#ffffff' : `rgb(${shade},${shade},252)`
    rectangles.push(`<rect x="${left+i*cell}" y="${top+j*cell}" width="${cell}" height="${cell}" fill="${color}" stroke="#d8dde7" stroke-width=".5"><title>${escapeHtml(observations[i])} / ${escapeHtml(observations[j])}: ${exists ? formatNumber(d, 4) : 'Unavailable'}</title></rect>`)
  }
  return `<div class="plot-scroll i2-heatmap-scroll"><svg class="science-plot i2-fusion-heatmap" viewBox="0 0 ${width} ${height}" role="img" aria-label="Fused distance heatmap, first ${observations.length} aligned observations">${rectangles.join('')}${labels}</svg></div><p class="method-note">First ${observations.length} aligned observations only, in metadata order. Grey = insufficient measured view coverage. The complete fused distance list is in the export.</p>`
}
function i2bProgressMarkup() {
  const rows=state.i2bProgressRows
  if(!rows?.length)return state.i2bProgress?`<p id="i2b-progress" class="i2b-progress-highlight" role="status">${escapeHtml(state.i2bProgress)}</p>`:''
  const summary=sourceProgressSummary(rows)
  return `<section class="i2b-progress-panel" aria-label="Source-distance calculation progress">
    <div class="i2b-progress-heading"><strong>Local distance calculation</strong><span id="i2b-progress-headline">${summary.finished} of ${summary.views} data types finished · ${summary.percent}% of reported steps</span></div>
    <progress id="i2b-overall-progress" max="${Math.max(1,summary.total)}" value="${summary.completed}" aria-label="All source-distance processing steps"></progress>
    <div class="i2b-progress-grid">${rows.map(row=>`<div class="i2b-progress-row" data-i2b-progress-row="${row.index}" data-status="${escapeHtml(row.status)}">
      <div class="i2b-progress-row-head"><strong>${escapeHtml(I1_MODALITIES.find(m=>m.key===row.modality)?.label||row.modality)}</strong><span data-i2b-state>${row.status==='running'?'Processing':row.status==='complete'?'Complete':row.status==='failed'?'Needs review':row.status==='stopped'?'Stopped':'Queued'}</span></div>
      <progress max="${Math.max(1,row.total)}" value="${row.completed}" aria-label="${escapeHtml(row.modality)} processing steps"></progress>
      <small data-i2b-stage>${escapeHtml(row.message)}</small>
    </div>`).join('')}</div>
    <p class="method-note">Steps count parsed files and final distance calculations. They are not a time estimate. Each data type uses a separate on-device worker; no source data is uploaded.</p>
    ${state.i2bProgress?`<p id="i2b-progress" class="i2b-progress-highlight">${escapeHtml(state.i2bProgress)}</p>`:''}
  </section>`
}
function i2DistancePanel() {
  const result = state.i2Result
  const alignment = state.alignmentPreview
  let viewNames = []
  try { viewNames = i2AvailableViews().map(view => I1_MODALITIES.find(m => m.key === view.modality)?.label || view.modality) }
  catch (error) { viewNames = [`Invalid distance source: ${error.message}`] }
  let content = ''
  if (result) {
    const ablationRows = result.ablation.map(row => `<tr><td>${escapeHtml(row.omitted)}</td><td>${row.overlapping_pairs.toLocaleString()}</td><td>${row.pearson_distance_r === null ? 'Unavailable' : formatNumber(row.pearson_distance_r, 4)}</td></tr>`).join('')
    dynamicTableSources.set('i2-nearest-neighbors', {
      rows: result.nearest_neighbors,
      cells: row => [row.observation, row.neighbor || '', row.distance, row.contributing_views],
      rowHtml: row => `<tr><td>${escapeHtml(row.observation)}</td><td>${escapeHtml(row.neighbor || 'Not available')}</td><td>${row.distance === null ? '—' : formatNumber(row.distance, 5)}</td><td>${row.contributing_views}</td></tr>`,
    })
    content = `<div class="summary-grid compact-grid">
      ${summaryCard('Aligned observations', result.observations.length.toLocaleString(), 'No group labels used')}
      ${summaryCard('Distance views', result.views.length, 'Equal median-scaled contributions')}
      ${summaryCard('Fused sample pairs', result.fused_pairs.toLocaleString(), `${result.min_views} or more matched views`)}
      ${summaryCard('Unavailable pairs', result.unavailable_pairs.toLocaleString(), 'Not imputed')}
    </div><div class="i2-result-grid"><article class="plot-card"><div class="plot-heading"><strong>Joint sample-distance heatmap</strong><span>Median-scaled distances, not a PCA or supervised prediction.</span></div>${i2HeatmapSvg(result)}</article><div><h4>Leave-one-modality-out sensitivity</h4><p class="method-note">Correlation with complete-fusion distances on shared measured sample pairs. A low value means the omitted view changes distances.</p><div class="table-scroll"><table><thead><tr><th>Removed data type</th><th>Shared pairs</th><th>Distance correlation</th></tr></thead><tbody>${ablationRows}</tbody></table></div></div></div>
    <h4>Nearest neighbors in the fused distances</h4><div class="table-scroll i2-neighbors" data-dynamic-table="i2-nearest-neighbors"><table><thead><tr><th>Observation</th><th>Nearest observation</th><th>Distance</th><th>Measured views</th></tr></thead><tbody></tbody></table></div>
    <div class="button-row"><button type="button" id="i2-export-json" class="secondary-button">Save full joint result JSON</button><button type="button" id="i2-export-tsv" class="secondary-button">Save all fused pairs TSV</button></div>
    ${result.warnings.length ? `<div class="analysis-warnings">${result.warnings.map(item => `<div>${escapeHtml(item)}</div>`).join('')}</div>` : ''}
    <p class="method-note">A nearest neighbor is a descriptive relationship between samples. No group model, adjusted-hit selection, outcome prediction or joint significance test was fitted.</p>`
  }
  return `<section class="card i2-distance-fusion" id="i2-distance-fusion"><div class="card-heading"><div><h3>Joint sample similarity</h3><p>Combine full, unsupervised distances from at least two data types after exact sample alignment. Each view receives a scale adjustment. Missing pairs stay missing.</p></div><span class="tag neutral">Combined distances</span></div>
    <p class="method-note">Input: full sample distances from the Longitudinal analysis. Gene and transcript results from the current browser session are used when available. Use Calculate local source distances to process the five other data types, or select native Longitudinal distance JSON files as an alternative. Corrected Compare Groups results and 2D plots are not valid inputs.</p>
    <div class="form-grid i2b-execution-controls"><label><span>Parallel data types</span><select id="i2b-parallel-workers" ${state.i2bBusy?'disabled':''}>${[1,2,3].map(n=>`<option value="${n}" ${state.i2bParallelWorkers===n?'selected':''}>${n} ${n===1?'worker':'workers'}</option>`).join('')}</select></label><p class="method-note">Start with 2 workers. Choose 1 if memory use is high. More workers can increase peak memory, and may not be faster on all devices.</p></div>
    <div class="run-row i2b-source-actions"><div><strong>Calculate distances directly from the aligned local data</strong><span>Uses all selected and available data types. One source workbook is read at a time. BCR/TCR use Morisita–Horn; isotypes use Aitchison; immune data uses standardized ssGSEA distances; expression uses the existing W4 distance method. No group labels or adjusted hits are used.</span></div><button type="button" id="i2b-run-direct" class="primary-button" ${!alignment || !state.wasmReady || state.i2bBusy || state.i2Busy || state.busy || state.alignmentBusy ? 'disabled' : ''}>${state.i2bBusy ? 'Calculating direct distances…' : 'Calculate local source distances'}</button></div>
    ${i2bProgressMarkup()}
    ${state.i2bDirectViews.length ? `<div class="run-row"><p class="method-note">Direct distances ready: ${escapeHtml(state.i2bDirectViews.map(v=>v.modality).join(', '))}. These take priority over imported distances for the same data type.</p><button type="button" class="secondary-button" id="i2b-export-sources">Save full source-distance JSON</button></div>` : ''}
    ${state.i2bErrors.length ? `<details class="details-block" open><summary>${state.i2bErrors.length} data type(s) need review</summary><div class="analysis-warnings">${state.i2bErrors.map(e=>`<div>${escapeHtml(e.modality)}: ${escapeHtml(e.message)}</div>`).join('')}</div></details>` : ''}
    <p class="method-note">After direct distances are ready, run Joint distance fusion below. Missing or failed views are not imputed. Large source tables can still exceed browser memory.</p>
    <div class="form-grid"><label><span>Local full-distance JSON files (optional, multiple allowed)</span><input id="i2-distance-files" type="file" accept=".json,application/json" multiple/></label><label><span>Minimum measured data types per pair</span><input id="i2-min-views" type="number" min="2" max="7" step="1" value="${state.i2MinViews}"/></label></div>
    <p class="method-note">Loaded views: ${viewNames.length ? escapeHtml(viewNames.join(' · ')) : 'None'}. Imported files stay in this browser session only. A local import replaces the same data type from the current session.</p>
    <div class="run-row"><div><strong>Fit a joint unsupervised distance model</strong><span>${alignment ? `${alignment.summary.eligible} observations passed metadata exclusions` : 'Check sample alignment first'}. Source IDs must match exact sample aliases.</span></div><button id="i2-run-fusion" class="primary-button" type="button" ${!alignment || state.i2Busy || state.busy || state.alignmentBusy || viewNames.length < 2 ? 'disabled' : ''}>${state.i2Busy ? 'Combining measured distances…' : 'Run joint distance fusion'}</button></div>
    ${state.i2Error ? `<div class="analysis-warnings" role="alert">${escapeHtml(state.i2Error)}</div>` : ''}
    ${content}
    <p class="integrative-message"><strong>Scientific limits:</strong> This is a real multi-view sample-distance analysis, but it does not fit latent factors, a joint ordination, or a predictive model. It uses full W4 distances; Direct local source-distance calculation supports all seven available data types, subject to native/WASM parity and real-cohort validation. Distance correlation is sensitivity analysis, not independent biological validation.</p>
  </section>`
}
// I2B runs each source modality in a separate short-lived worker. One failed
// parser cannot remove complete, previously calculated views.
async function i2bRunDirect() {
  if (state.i2bBusy || state.busy || state.i2Busy || state.alignmentBusy || !state.alignmentPreview) return
  const controller = new AbortController()
  activeI2bController = controller
  state.i2bBusy = true
  state.busy = true
  state.i2bErrors = []
  state.i2bDirectViews = []
  state.i2bProgressRows = []
  state.i2Result = null
  state.i2StructureResult = null
  const selectedMerged = {gene: state.compareConfig.geneMergedId, transcript: state.compareConfig.transcriptMergedId}
  try {
    const {requests, errors} = prepareI2BRequests(state.alignmentPreview, state.records, selectedFile,
      selectedMerged, state.hitConfig.integrativeModalities || {})
    state.i2bErrors = errors
    state.i2bProgressRows = requests.map((entry,index)=>({
      index, modality:entry.modality, status:'queued', message:'Queued', completed:0,
      total:Math.max(1,entry.files.length)+(entry.sampleIds?0:1),
    }))
    state.i2bProgress = `Preparing ${requests.length} data types with up to ${state.i2bParallelWorkers} local workers`
    render()
    const {outcomes} = await runBoundedSourceTasks(requests, {
      parallel:state.i2bParallelWorkers,
      signal:controller.signal,
      run: async(entry,index,onProgress) => {
        const response=await isolatedScienceRequest({
        createWorker: () => new Worker(new URL(`./analysis-worker.js?v=${APP_VERSION}`,import.meta.url),{type:'module'}),
        type:'run-i2b-source-distance',
        payload:{modality:entry.modality, files:entry.files, sampleIds:entry.sampleIds,
          options:{ambiguousPolicy:state.compareConfig.ambiguousPolicy,
            determinedOnly:state.compareConfig.excludeUndeterminedIsotype,
            minCount:state.longitudinalConfig.minCount,
            minFractionSamples:state.longitudinalConfig.minFractionSamples}},
        resourceSettings:state.resources,
        signal:controller.signal,
        onProgress,
        onMemory:report=>{
          const memory=validateWasmMemoryReport(report)
          if(memory){state.wasmMemorySample={...memory,active:true};renderSystemInfo()}
        },
        })
        if(response.type!=='i2b-source-distance-result'||response.view?.modality!==entry.modality)
          throw new Error('Source-distance worker returned the wrong result')
        return response
      },
      onUpdate:(row,summary,allRows)=>{
        state.i2bProgressRows=allRows
        state.i2bProgress=`${summary.finished} of ${summary.views} data types finished; ${summary.percent}% of reported processing steps`
        updateI2bProgressDom(row,summary)
        const live=document.querySelector('#integrative-live-source-status')
        if(live)live.textContent=`${summary.finished} of ${summary.views} data types finished · ${summary.percent}% of reported steps`
        const bar=document.querySelector('#integrative-live-source-bar')
        if(bar){bar.max=Math.max(1,summary.total);bar.value=summary.completed}
      },
    })
    for(const outcome of outcomes){
      if(outcome?.status==='complete'){
        const entry=outcome.entry,response=outcome.value
        if(response.type!=='i2b-source-distance-result'||response.view?.modality!==entry.modality){
          state.i2bErrors.push({modality:entry.modality,message:'Source-distance worker returned the wrong result'})
          continue
        }
        state.i2bDirectViews.push({
          ...response.view,
          input_files:entry.files.map(item=>({name:item.name,sample_id:item.sampleId||null})),
          input_options:{ambiguousPolicy:state.compareConfig.ambiguousPolicy,
            determinedOnly:state.compareConfig.excludeUndeterminedIsotype,
            minCount:state.longitudinalConfig.minCount,
            minFractionSamples:state.longitudinalConfig.minFractionSamples},
        })
      }else if(outcome?.status==='failed'){
        state.i2bErrors.push({modality:outcome.entry.modality,message:outcome.error.message})
      }
    }
  } catch(error) {
    state.i2bErrors.push({modality:'alignment',message:error instanceof Error?error.message:String(error)})
  } finally {
    activeI2bController=null
    state.busy=false
    state.i2bBusy=false
    state.i2bProgress=controller.signal.aborted
      ? 'Stopped; completed data types remain available.'
      : `Local distance calculation finished; ${state.i2bDirectViews.length} data types ready.`
    if(state.wasmMemorySample)state.wasmMemorySample={...state.wasmMemorySample,active:false}
    render()
  }
}

function updateI2bProgressDom(row,summary) {
  const overall=document.querySelector('#i2b-overall-progress')
  if(overall){overall.max=Math.max(1,summary.total);overall.value=summary.completed}
  const headline=document.querySelector('#i2b-progress-headline')
  if(headline)headline.textContent=`${summary.finished} of ${summary.views} data types finished · ${summary.percent}% of reported steps`
  const current=document.querySelector(`[data-i2b-progress-row="${row.index}"]`)
  if(!current)return
  current.dataset.status=row.status
  const bar=current.querySelector('progress')
  if(bar){bar.max=Math.max(1,row.total);bar.value=row.completed}
  const stage=current.querySelector('[data-i2b-stage]')
  if(stage)stage.textContent=row.message
  const stateLabel=current.querySelector('[data-i2b-state]')
  if(stateLabel)stateLabel.textContent=row.status==='running'?'Processing':row.status==='complete'?'Complete':row.status==='failed'?'Needs review':row.status==='stopped'?'Stopped':'Queued'
}

async function i2ReadLocalFiles(fileList) {
  state.i2Busy = true
  state.i2Error = ''
  state.i2Result = null
  state.i2StructureResult = null
  render()
  try {
    const merged = new Map(state.i2ImportedViews.map(v => [v.modality, v]))
    for (const file of fileList) {
      const json = JSON.parse(await file.text())
      for (const view of extractFullDistanceViews(json)) merged.set(view.modality, view)
    }
    state.i2ImportedViews = [...merged.values()]
  } catch (error) {
    state.i2Error = `Local distance import: ${error.message}`
  } finally { state.i2Busy = false; render() }
}
async function i2RunFusion() {
  state.i2Error = ''
  state.i2Result = null
  state.i2StructureResult = null
  if (!state.alignmentPreview) { state.i2Error = 'Check sample alignment first'; render(); return }
  state.i2Busy = true
  render()
  try {
    const prepared = prepareJointDistanceInput(state.alignmentPreview, i2AvailableViews(), { minViews: state.i2MinViews })
    const response = await workerRequest('run-integrative-distance', { input: prepared.input })
    state.i2Result = response.result
    const dropped = prepared.diagnostics.filter(item => item.excludedOrUnmatchedPairs)
    if (dropped.length) state.i2Error = `Not used: ${dropped.map(item => `${item.modality} ${item.excludedOrUnmatchedPairs} unaligned/excluded source pairs`).join('; ')}. Review the sample alignment manifest.`
  } catch (error) { state.i2Error = error instanceof Error ? error.message : String(error) }
  finally { state.i2Busy = false; render() }
}
function bindI2DistanceEvents() {
  document.querySelector('#i2b-parallel-workers')?.addEventListener('change',event=>{
    if(state.i2bBusy)return
    state.i2bParallelWorkers=validParallelWorkers(event.currentTarget.value)
    render()
  })
  document.querySelector('#i2b-run-direct')?.addEventListener('click', () => void i2bRunDirect())
  document.querySelector('#i2b-export-sources')?.addEventListener('click', () => {
    if (state.i2bDirectViews.length) void saveTextFile('4c-insight-i2b-source-distances.json',
      JSON.stringify({schema:'4c-insight-i2b-source-distances-v1', views:state.i2bDirectViews,
        alignment_roles:state.alignmentPreview?.roles, alignment_exclusions:state.alignmentPreview?.exclusions},null,2)+'\n')
      .catch(error=>{state.i2bErrors.push({modality:'export',message:error.message});render()})
  })
  document.querySelector('#i2-distance-files')?.addEventListener('change', event => {
    const files = [...event.currentTarget.files || []]
    if (files.length) void i2ReadLocalFiles(files)
  })
  document.querySelector('#i2-min-views')?.addEventListener('change', event => {
    const n = Number(event.currentTarget.value)
    state.i2MinViews = Number.isInteger(n) && n >= 2 && n <= 7 ? n : 2
    state.i2Result = null; render()
  })
  document.querySelector('#i2-run-fusion')?.addEventListener('click', () => void i2RunFusion())
  document.querySelector('#i2-export-json')?.addEventListener('click', () => {
    if (state.i2Result) void saveTextFile('4c-insight-joint-distance-fusion.json', JSON.stringify(state.i2Result, null, 2) + '\n')
      .catch(error => { state.i2Error = error.message; render() })
  })
  document.querySelector('#i2-export-tsv')?.addEventListener('click', () => {
    if (state.i2Result) void saveTextFile('4c-insight-joint-distances.tsv', jointDistancesTsv(state.i2Result))
      .catch(error => { state.i2Error = error.message; render() })
  })
}

// Complete-distance joint ordination and clustering. Only selected source
// modalities enter the Rust fusion. Labels remain post-fit annotations.
function jointStructurePointSvg(result) {
  const points = result.pcoa.points
  if (!points.length) return '<p>No joint ordination is available.</p>'
  const width=760, height=385, pad=58
  const xs=points.map(p=>p.x), ys=points.map(p=>p.y)
  const [minX,maxX]=paddedExtent(xs),[minY,maxY]=paddedExtent(ys)
  const sx=x=>pad+(x-minX)/(maxX-minX)*(width-2*pad)
  const sy=y=>height-pad-(y-minY)/(maxY-minY)*(height-2*pad)
  const axes=result.pcoa.axis_variance_explained
  const xLabel=`PCo1 (${(100*axes[0]).toFixed(1)}% of positive inertia)`
  const yLabel=`PCo2 (${(100*axes[1]).toFixed(1)}% of positive inertia)`
  const circles=points.map((point,i)=>{
    const row=state.alignmentPreview?.rows?.find(r=>r.observation===point.observation)
    const x=sx(point.x),y=sy(point.y)
    return `<circle class="joint-point" data-point-index="${i}" data-sample-id="${escapeHtml(point.observation)}" data-x="${point.x}" data-y="${point.y}" cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" r="4.5"><title>${escapeHtml(point.observation)} · Patient: ${escapeHtml(row?.subject||'Unknown')} · Timepoint: ${escapeHtml(row?.timepoint||'Not selected')} · ${xLabel} ${formatNumber(point.x,4)} · ${yLabel} ${formatNumber(point.y,4)}</title></circle><text class="scatter-point-label" data-point-label="${i}" x="${(x+7).toFixed(2)}" y="${(y-6).toFixed(2)}" style="display:none">${escapeHtml(point.observation)}</text>`
  }).join('')
  const id='joint-ordination'
  return `<div class="plot-shell" data-interactive-plot="${id}" data-plot-nav-root="true" data-plot-kind="pca" data-width="${width}" data-height="${height}" data-pad="${pad}" data-min-x="${minX}" data-max-x="${maxX}" data-min-y="${minY}" data-max-y="${maxY}">
    ${plotControls(id)}
    <div class="plot-scroll interactive-plot-scroll coordinate-plot"><svg class="science-plot" viewBox="0 0 ${width} ${height}" role="img" aria-label="Joint unsupervised principal coordinates">
      <defs><clipPath id="${id}-clip"><rect x="${pad}" y="${pad}" width="${width-pad*2}" height="${height-pad*2}"/></clipPath></defs>
      <g class="pca-grid"></g><g class="pca-axes"></g><g class="pca-points" clip-path="url(#${id}-clip">${circles}</g>
      <text class="plot-label" x="${width/2}" y="${height-9}" text-anchor="middle">${xLabel}</text>
      <text class="plot-label" x="16" y="${height/2}" transform="rotate(-90 16 ${height/2})" text-anchor="middle">${yLabel}</text>
    </svg></div></div>`
}
function jointStructurePanel() {
  const result=state.i2StructureResult
  let views=[]
  try { views=i2AvailableViews() } catch { views=[] }
  const enabled=views.map(v=>`<label><input type="checkbox" data-joint-structure-view="${escapeHtml(v.modality)}" ${state.i2StructureSelection[v.modality]!==false?'checked':''}/><span>${escapeHtml(I1_MODALITIES.find(m=>m.key===v.modality)?.label||v.modality)}</span></label>`).join('')
  const chosen=views.filter(v=>state.i2StructureSelection[v.modality]!==false)
  let content=''
  if(result){
    const report=result.coverage
    const coveragePercent=(report.pair_coverage*100).toFixed(1)
    const n=result.observations.length
    content=`<div class="summary-grid compact-grid">
      ${summaryCard('Samples in joint structure',n.toLocaleString(),`${report.removed_for_missing_pairs} removed for incomplete pairs`)}
      ${summaryCard('Fixed-view pair coverage',`${coveragePercent}%`,`${report.available_fixed_view_pairs} of ${report.eligible_pairs} pairs`)}
      ${summaryCard('PCoA positive axes',result.pcoa.positive_axes,`${result.pcoa.negative_axes} negative eigenvalues`)}
      ${summaryCard('Negative inertia',`${(100*result.pcoa.negative_inertia_ratio).toFixed(1)}%`,'Non-Euclidean distance diagnostic')}
    </div>
    <div class="plot-grid joint-structure-plots"><article class="plot-card"><div class="plot-heading"><strong>Joint sample ordination</strong><span>Principal coordinates from the same complete, fixed-view sample matrix. Hover to identify observations. Group labels were not used for fitting.</span></div>${jointStructurePointSvg(result)}</article></div>
    <article class="plot-card joint-heatmap-card"><div class="plot-heading"><strong>Hierarchical clustering and full joint heatmap</strong><span>All ${n} retained observations in their original cluster order. Drag a rectangle to zoom, or expand the view. Cropping does not re-fit the hierarchy.</span></div>
      ${heatmapViewerMarkup()}
      ${jointDistanceReviewMarkup(result)}
    </article>
    <h4>Leave-one-data-type-out sensitivity on the same observations</h4>
    ${result.fixed_ablation?.length?`<div class="table-scroll"><table><thead><tr><th>Removed data type</th><th>Pairwise correlation</th><th>Nearest-neighbor agreement</th><th>Status</th></tr></thead><tbody>${result.fixed_ablation.map(row=>`<tr><td>${escapeHtml(I1_MODALITIES.find(m=>m.key===row.omitted)?.label||row.omitted)}</td><td>${row.available&&row.distance_correlation!==null?formatNumber(row.distance_correlation,4):'—'}</td><td>${row.available?`${(100*row.nearest_neighbor_retention).toFixed(1)}%`:'—'}</td><td>${row.available?'Same complete cohort':escapeHtml(row.reason||'Unavailable')}</td></tr>`).join('')}</tbody></table></div>`:'<p class="method-note">At least three data types are required for a leave-one-out comparison with two remaining views.</p>'}
    <div class="button-row"><button class="secondary-button" id="joint-structure-json" type="button">Save full joint structure JSON</button><button class="secondary-button" id="joint-structure-coordinates" type="button">Save PCoA coordinates TSV</button></div>
    ${result.warnings.map(w=>`<p class="method-note">${escapeHtml(w)}</p>`).join('')}
    <p class="method-note">The complete-distance cohort is chosen by deterministic greedy removal. It is not guaranteed to contain the maximum possible number of samples. Sample selection may change biological composition. Review excluded IDs in the JSON export.</p>`
  }
  return `<section class="card joint-structure" id="joint-structure"><div class="card-heading"><div><h3>Joint ordination and clustering</h3><p>Use a fixed set of data types for every sample pair. Calculate principal coordinates and hierarchical clusters only for observations with complete pairwise distances.</p></div><span class="tag neutral">Unsupervised analysis</span></div>
    <div class="integrative-selector" role="group" aria-label="Data types for joint ordination">${enabled||'Calculate source distances first'}</div>
    <label class="check-label"><input type="checkbox" id="joint-one-per-patient" ${state.i2StructureOnePerPatient?'checked':''}/><span>Keep only the first eligible observation per patient (sensitivity check; requires patient IDs)</span></label>
    <p class="method-note">This calculation refits the Rust joint distance model with exactly the selected modalities. It does not reuse mixed-coverage distances or replace missing pairs with zero.</p>
    <div class="run-row"><div><strong>Explore shared sample structure</strong><span>Runs PCoA and average-linkage clustering in a browser worker. No case/control labels, selected hits or corrected group effects enter the fit.</span></div><button id="joint-run-structure" class="primary-button" type="button" ${!state.alignmentPreview||state.i2StructureBusy||state.i2Busy||state.busy||chosen.length<2?'disabled':''}>${state.i2StructureBusy?'Analyzing joint structure…':'Analyze joint structure'}</button></div>
    ${state.i2StructureError?`<div class="analysis-warnings" role="alert">${escapeHtml(state.i2StructureError)}</div>`:''}
    ${content}
  </section>`
}
async function runJointStructure() {
  if(state.i2StructureBusy||state.busy||state.i2Busy||!state.alignmentPreview)return
  const views=i2AvailableViews().filter(v=>state.i2StructureSelection[v.modality]!==false)
  if(views.length<2){state.i2StructureError='Select at least two measured data types';render();return}
  state.i2StructureResult=null
  state.jointStabilityResult=null
  state.bioResult=null
  state.bioFeatureResult=null
  state.i2StructureError=''
  state.i2StructureBusy=true
  state.busy=true
  render()
  try {
    const input=prepareJointDistanceInput(state.alignmentPreview,views,{minViews:views.length}).input
    const fused=await workerRequest('run-integrative-distance',{input})
    const patientByObservation=Object.fromEntries(state.alignmentPreview.rows.filter(row=>!row.excluded).map(row=>[row.observation,row.subject||'']))
    const answer=await workerRequest('run-joint-structure',{
      result:fused.result,
      options:{selectedModalities:views.map(v=>v.modality),patientByObservation,onePerPatient:state.i2StructureOnePerPatient},
    })
    if(answer.type!=='joint-structure-result'||answer.result?.schema!=='4c-insight-joint-structure-v1')throw new Error('The joint model returned an unexpected result')
    const ablation=[]
    if(views.length>=3){
      for(const omitted of views){
        const retained=views.filter(v=>v.modality!==omitted.modality)
        try{
          const subset=prepareJointDistanceInput(state.alignmentPreview,retained,{minViews:retained.length}).input
          const response=await workerRequest('run-integrative-distance',{input:subset})
          ablation.push(compareFixedCohortAblation(answer.result,response.result,omitted.modality))
        }catch(error){
          if(/operation stopped|operation cancelled/i.test(String(error?.message||error))) throw error
          ablation.push({omitted:omitted.modality,available:false,reason:error instanceof Error?error.message:String(error)})
        }
      }
    }
    answer.result.fixed_ablation=ablation
    state.i2StructureResult=answer.result
  }catch(error){state.i2StructureError=error instanceof Error?error.message:String(error)}
  finally{state.i2StructureBusy=false;state.busy=false;render()}
}
function bindJointStructureEvents() {
  document.querySelectorAll('[data-joint-structure-view]').forEach(el=>el.addEventListener('change',event=>{
    state.i2StructureSelection[event.currentTarget.dataset.jointStructureView]=event.currentTarget.checked
    state.i2StructureResult=null;state.i2StructureError='';render()
  }))
  document.querySelector('#joint-one-per-patient')?.addEventListener('change',event=>{
    state.i2StructureOnePerPatient=event.currentTarget.checked
    state.i2StructureResult=null;state.i2StructureError='';render()
  })
  document.querySelector('#joint-run-structure')?.addEventListener('click',()=>void runJointStructure())
  document.querySelector('#joint-structure-json')?.addEventListener('click',()=>{
    if(state.i2StructureResult)void saveTextFile('4c-insight-joint-ordination-clustering.json',JSON.stringify(state.i2StructureResult,null,2)+'\n')
      .catch(error=>{state.i2StructureError=error.message;render()})
  })
  document.querySelector('#joint-structure-coordinates')?.addEventListener('click',()=>{
    const output=state.i2StructureResult
    if(!output)return
    const safe=text=>{const value=String(text??'');const guarded=/^[=+@\-\t\r]/.test(value)?`'${value}`:value;return /["\t\n\r]/.test(guarded)?`"${guarded.replaceAll('"','""')}"`:guarded}
    const lines=['observation\tpcoa_1\tpcoa_2\tpatient\ttimepoint']
    for(const p of output.pcoa.points){const row=state.alignmentPreview.rows.find(r=>r.observation===p.observation);lines.push([p.observation,p.x,p.y,row?.subject||'',row?.timepoint||''].map(safe).join('\t'))}
    void saveTextFile('4c-insight-joint-pcoa-coordinates.tsv',lines.join('\n')+'\n')
      .catch(error=>{state.i2StructureError=error.message;render()})
  })
  mountJointHeatmapViewer(state.i2StructureResult)
}

// Patient-level cluster sensitivity uses the unchanged complete joint distance
// matrix and does not read case/control assignments or selected hits.
function jointStabilityPanel() {
  const structure=state.i2StructureResult
  if(!structure)return ''
  const cfg=state.jointStabilityConfig
  const outcome=state.jointStabilityResult
  const disabled=state.jointStabilityBusy||state.busy||!state.alignmentPreview
  const filled=structure.observations.every(id=>{
    const row=state.alignmentPreview?.rows?.find(r=>r.observation===id)
    return typeof row?.subject==='string'&&row.subject.trim().length>0
  })
  let content=''
  if(outcome){
    const ari=outcome.adjusted_rand, pair=outcome.pair_support
    content=`<div class="summary-grid compact-grid">
      ${summaryCard('Mean cluster agreement',formatNumber(ari.mean,3),'Adjusted Rand index across repeats')}
      ${summaryCard('Median cluster agreement',formatNumber(ari.median,3),`${outcome.repeats} patient-level repeats`)}
      ${summaryCard('Within-cluster agreement',pair.within_cluster_coassignment===null?'—':`${(100*pair.within_cluster_coassignment).toFixed(1)}%`,'Co-assigned pairs, when both patients are selected')}
      ${summaryCard('Between-cluster separation',pair.between_cluster_separation===null?'—':`${(100*pair.between_cluster_separation).toFixed(1)}%`,'Pairs from different original clusters that remain separate')}
    </div>
    <div class="table-scroll"><table><thead><tr><th>Original cluster</th><th>Observations</th><th>Patients</th><th>Measured pairs</th><th>Co-assignment</th></tr></thead><tbody>${outcome.clusters_summary.map(row=>`<tr><td>${row.cluster}</td><td>${row.observations}</td><td>${row.patients}</td><td>${row.informative_pairs}</td><td>${row.within_cluster_coassignment===null?'Unavailable':`${(100*row.within_cluster_coassignment).toFixed(1)}%`}</td></tr>`).join('')}</tbody></table></div>
    <p class="method-note">${outcome.patients} unique patients; ${outcome.sampled_patients_per_repeat} selected per repeat. ${outcome.patients_with_repeated_visits} patients have repeated observations; ${outcome.patients_crossing_original_clusters} have visits in different original clusters.</p>
    <details class="details-block"><summary>How to interpret cluster stability</summary>
      <p class="method-note">Cluster agreement measures how often the original groupings persist when patients are resampled. It does not identify biological pathways or validate a clinical subtype.</p>
      <p class="method-note">Next: compare two, three and four clusters; change the random seed; inspect distant observations and source QC; then compare phenotypes and molecular features using patient-aware tests. Do not use these same labels to claim independent prediction.</p>
    </details>
    ${outcome.warnings.map(item=>`<p class="method-note">${escapeHtml(item)}</p>`).join('')}
    <div class="button-row"><button id="joint-stability-export" class="secondary-button" type="button">Save patient stability JSON</button></div>`
  }
  return `<section class="card joint-stability" id="joint-stability"><div class="card-heading"><div><h3>Patient-level cluster stability</h3><p>Check whether the joint clusters remain similar when distinct patients are subsampled. Each repeat uses one eligible visit per patient. Distances and source features are not recalculated.</p></div><span class="tag neutral">Sensitivity analysis</span></div>
    <div class="form-grid joint-stability-settings">
      <label><span>Number of clusters to compare</span><input id="joint-stability-k" type="number" min="2" max="12" step="1" value="${cfg.clusters}"/></label>
      <label><span>Independent patient subsets</span><input id="joint-stability-repeats" type="number" min="10" max="500" step="1" value="${cfg.repeats}"/></label>
      <label><span>Fraction of patients per subset</span><select id="joint-stability-fraction"><option value="0.7" ${cfg.fraction===0.7?'selected':''}>70%</option><option value="0.8" ${cfg.fraction===0.8?'selected':''}>80%</option><option value="0.9" ${cfg.fraction===0.9?'selected':''}>90%</option></select></label>
      <label><span>Reproducible random seed</span><input id="joint-stability-seed" type="number" min="0" max="4294967295" step="1" value="${cfg.seed}"/></label>
    </div>
    ${!filled?'<p class="notice-banner">Patient IDs are required for every fitted observation. Select an exact patient ID column in Sample alignment and run alignment again.</p>':''}
    <div class="run-row"><div><strong>Compare clusters across patient subsets</strong><span>Reports adjusted Rand agreement and pair co-assignment. This tests sensitivity to patient composition, not clinical validity or out-of-sample prediction.</span></div><button type="button" id="joint-stability-run" class="primary-button" ${disabled||!filled?'disabled':''}>${state.jointStabilityBusy?'Analyzing patient subsets…':'Analyze cluster stability'}</button></div>
    ${state.jointStabilityProgress?`<p class="method-note" role="status">${escapeHtml(state.jointStabilityProgress)}</p>`:''}
    ${state.jointStabilityError?`<div class="analysis-warnings" role="alert">${escapeHtml(state.jointStabilityError)}</div>`:''}
    ${content}
    <p class="method-note">Choose the number of clusters before the analysis. Cluster assignments are descriptive and depend on the fixed-view cohort. No phenotype labels enter the fit. Low agreement calls for further source-level and quality-control review.</p>
  </section>`
}
async function runJointStability() {
  if(!state.i2StructureResult||!state.alignmentPreview||state.jointStabilityBusy||state.busy)return
  const cfg={...state.jointStabilityConfig}
  const patients=Object.fromEntries(state.alignmentPreview.rows.filter(row=>!row.excluded).map(row=>[row.observation,row.subject||'']))
  state.jointStabilityBusy=true
  state.busy=true
  state.jointStabilityError=''
  state.jointStabilityResult=null
  state.jointStabilityProgress='Preparing patient subsets'
  render()
  try {
    const response=await workerRequest('run-joint-stability',{
      structure:state.i2StructureResult, patients, options:cfg,
    },progress=>{
      state.jointStabilityProgress=progress.message
      const status=document.querySelector('#joint-stability [role="status"]')
      if(status)status.textContent=progress.message
    })
    if(response.type!=='joint-stability-result'||response.result?.schema!=='4c-insight-patient-stability-v1')throw new Error('Unexpected patient stability result')
    state.jointStabilityResult=response.result
  }catch(error){state.jointStabilityError=error instanceof Error?error.message:String(error)}
  finally{state.jointStabilityBusy=false;state.busy=false;state.jointStabilityProgress='';render()}
}
function bindJointStabilityEvents() {
  const controls={
    'joint-stability-k':'clusters',
    'joint-stability-repeats':'repeats',
    'joint-stability-fraction':'fraction',
    'joint-stability-seed':'seed',
  }
  for(const [id,key] of Object.entries(controls))document.querySelector(`#${id}`)?.addEventListener('change',event=>{
    const value=Number(event.currentTarget.value)
    if(!Number.isFinite(value))return
    state.jointStabilityConfig[key]=value
    state.jointStabilityResult=null;state.jointStabilityError='';render()
  })
  document.querySelector('#joint-stability-run')?.addEventListener('click',()=>void runJointStability())
  document.querySelector('#joint-stability-export')?.addEventListener('click',()=>{
    if(state.jointStabilityResult)void saveTextFile('4c-insight-patient-cluster-stability.json',JSON.stringify(state.jointStabilityResult,null,2)+'\n')
      .catch(error=>{state.jointStabilityError=error.message;render()})
  })
}

// Biological interpretation is post-fit and never feeds labels or ranked features
// back into the unsupervised distance model.
function biologicalScatterSvg(report, structure=state.i2StructureResult) {
  return integrativeScatterMarkup(report, structure?.pcoa?.axis_variance_explained || [] , escapeHtml,formatNumber)
}
// Post-fit feature color overlay. This does not refit distances or PCoA.
function biologicalFeatureScatterSvg(report,profile) {
  if(!report||!profile)return ''
  const measures=new Map((profile.values||[]).map(row=>[row.observation,row.log2_tpm]))
  const valid=[...measures.values()].filter(Number.isFinite)
  if(!valid.length)return '<p class="method-note">No measured values are available for this feature.</p>'
  const lo=Math.min(...valid),hi=Math.max(...valid),span=hi-lo
  const points=report.points, width=650,height=350,pad=49
  const [xmin,xmax]=paddedExtent(points.map(p=>p.x)),[ymin,ymax]=paddedExtent(points.map(p=>p.y))
  const sx=x=>pad+(x-xmin)/(xmax-xmin)*(width-2*pad)
  const sy=y=>height-pad-(y-ymin)/(ymax-ymin)*(height-2*pad)
  const circles=points.map(p=>{
    const value=measures.get(p.observation),known=Number.isFinite(value)
    const u=known&&span>0?(value-lo)/span:known?0.5:0
    const fill=known?`rgb(${Math.round(235-199*u)},${Math.round(239-160*u)},${Math.round(250-27*u)})`:'#aeb8c9'
    return `<circle data-x="${p.x}" data-y="${p.y}" cx="${sx(p.x).toFixed(2)}" cy="${sy(p.y).toFixed(2)}" r="4.3" fill="${fill}" stroke="#fff" stroke-width="0.7"><title>${escapeHtml(p.observation)} · ${escapeHtml(profile.feature)}: ${known?formatNumber(value,3):'Missing'} log2(TPM + 1) · PCo1 ${formatNumber(p.x,3)} · PCo2 ${formatNumber(p.y,3)}</title></circle>`
  }).join('')
  return `<div class="bio-scatter bio-feature-plot"><svg viewBox="0 0 ${width} ${height}" data-coordinate-min-x="${xmin}" data-coordinate-max-x="${xmax}" data-coordinate-min-y="${ymin}" data-coordinate-max-y="${ymax}" role="img" aria-label="Joint ordination colored by ${escapeHtml(profile.feature)} log2 TPM measurements"><rect x="${pad}" y="${pad}" width="${width-2*pad}" height="${height-2*pad}" fill="#fff" stroke="#e3e8f1"/>${circles}<text x="${width/2}" y="${height-9}" text-anchor="middle">Joint PCo1</text><text x="15" y="${height/2}" transform="rotate(-90 15 ${height/2})" text-anchor="middle">Joint PCo2</text></svg>
    <div class="bio-feature-legend"><span>Low ${formatNumber(lo,2)}</span><i></i><span>High ${formatNumber(hi,2)}</span><span>Gray: missing</span></div></div>`
}
function biologicalFindingsPanel() {
  ensureBiologicalFindings()
  const structure=state.i2StructureResult
  const report=structure?state.bioResult:null
  const feature=structure?state.bioFeatureResult:null
  const cols=state.alignmentTable?.columns||[]
  const options=cols.filter(c=>c!==state.alignmentRoles.observation)
  let reportMarkup=''
  if(report){
    const m=report.metadata
    const annotation=m?.kind==='numeric'?`<p class="method-note">${escapeHtml(report.column)}: ${m.observed} measured values. Spearman with PCo1: ${m.pcoa_1_rho===null?'—':formatNumber(m.pcoa_1_rho,3)}; PCo2: ${m.pcoa_2_rho===null?'—':formatNumber(m.pcoa_2_rho,3)}. Descriptive only.</p>`:
      m?.kind==='categorical'?`<p class="method-note">${escapeHtml(report.column)}: ${m.observed} annotated, ${m.missing} missing, ${m.unique} categories.</p>${m.too_many_categories?'<p class="notice-banner">Many categories were found. Confirm that this field is not a unique patient or sample ID.</p>':`<details class="details-block"><summary>Metadata counts for each cluster</summary><div class="table-scroll"><table><thead><tr><th>Value</th><th>Total</th>${report.membership.map(c=>`<th>Cluster ${c.cluster}</th>`).join('')}</tr></thead><tbody>${m.categories.map(c=>`<tr><td>${escapeHtml(c.value)}</td><td>${c.n}</td>${c.clusters.map(x=>`<td>${x.count}</td>`).join('')}</tr>`).join('')}</tbody></table></div></details>`}`:
      '<p class="method-note">Select a clinical or QC column above to color the PCoA. Cluster colors are used by default.</p>'
    reportMarkup=`<div class="summary-grid compact-grid bio-findings-summary">
      ${summaryCard('Retained observations',report.observations,'Fixed joint-distance cohort')}
      ${summaryCard('Descriptive clusters',report.clusters,'Post-fit partition')}
      ${summaryCard('Metadata coverage',m?`${m.observed}/${report.observations}`:'Not selected','No missing-value imputation')}
      ${summaryCard('Data types fitted',report.fixed_modalities.length,'Seven types shown below')}
    </div>
    <div class="bio-dashboard-top"><article class="plot-card"><div class="plot-heading"><strong>Joint ordination and clinical context</strong><span>Color annotations are applied only after the unsupervised model.</span></div>${biologicalScatterSvg(report)}${annotation}</article>
      <article class="plot-card bio-all-modality-card"><div class="plot-heading"><strong>All seven data types</strong><span>Observed pair coverage, distance agreement and between/within separation. A missing type is shown as unavailable.</span></div>${modalityOverviewHtml(report)}
        <p class="method-note">Agreement is a Pearson correlation of modality distances with fused distances. Between/within ratios above 1 show larger average distances between the fitted clusters. Both are in-sample and can be circular. Bar lengths are displays, not biological effect weights.</p></article></div>
    <details class="details-block"><summary>Sample coverage and exact IDs</summary><div class="table-scroll"><table><thead><tr><th>Observation</th><th>Cluster</th><th>Patient</th><th>Visit</th><th>Measured data types</th></tr></thead><tbody>${report.coverage.map(r=>`<tr><td>${escapeHtml(r.observation)}</td><td>${r.cluster}</td><td>${escapeHtml(r.patient||'—')}</td><td>${escapeHtml(r.timepoint||'—')}</td><td>${r.measured}</td></tr>`).join('')}</tbody></table></div></details>
    ${report.warnings.map(w=>`<p class="method-note">${escapeHtml(w)}</p>`).join('')}`
  }
  let featureMarkup=''
  if(feature){
    const combined=combinedFeatureRows(feature)
    const label=key=>I1_MODALITIES.find(m=>m.key===key)?.label||key
    dynamicTableSources.set('bio-feature-table',{
      rows:combined,
      cells:r=>[r.modality_label,r.feature,r.secondary_id,r.n_used,r.pcoa1_spearman,r.pcoa2_spearman,r.cluster1_log2_tpm_difference],
      rowHtml:r=>`<tr><td>${escapeHtml(r.modality_label)}</td><td><strong>${escapeHtml(r.feature)}</strong></td><td>${escapeHtml(r.secondary_id)}</td><td>${r.n_used}</td><td>${r.pcoa1_spearman===null?'—':formatNumber(r.pcoa1_spearman,4)}</td><td>${r.pcoa2_spearman===null?'—':formatNumber(r.pcoa2_spearman,4)}</td><td>${r.cluster1_log2_tpm_difference===null?'—':formatNumber(r.cluster1_log2_tpm_difference,4)}</td></tr>`,
    })
    const profiles=featuredProfiles(feature)
    const profile=chosenProfile(feature,state.bioSelectedProfile)||profiles[0]||null
    const warnings=feature.warnings||[]
    featureMarkup=`<div class="bio-feature-section"><div class="bio-feature-section-title"><strong>Top associated RNA features</strong><label>PCoA axis <select id="bio-feature-axis"><option value="pcoa1_spearman" ${state.bioFeatureAxis==='pcoa1_spearman'?'selected':''}>PCo1</option><option value="pcoa2_spearman" ${state.bioFeatureAxis==='pcoa2_spearman'?'selected':''}>PCo2</option></select></label></div>
      ${featureLollipopHtml(feature,state.bioFeatureAxis,8)}
      <div class="bio-feature-selection"><label>Inspect a measured RNA feature <select id="bio-feature-inspect">${profiles.map(p=>`<option value="${p.key}" ${p.key===profile?.key?'selected':''}>${escapeHtml(label(p.modality))} · ${escapeHtml(p.feature)}</option>`).join('')}</select></label></div>
      ${profile?`<article class="plot-card"><div class="plot-heading"><strong>${escapeHtml(profile.feature)}</strong><span>${escapeHtml(label(profile.modality))} · sample values on fixed PCoA coordinates · ${profile.values.length} measurements</span></div>${biologicalFeatureScatterSvg(report,profile)}</article>`:''}
      <details class="details-block bio-full-table"><summary>Search and sort all ${combined.length.toLocaleString()} RNA feature scores</summary><div class="table-scroll differential-table" data-dynamic-table="bio-feature-table"><table><thead><tr><th>Data type</th><th>Feature</th><th>Secondary ID</th><th>Measured observations</th><th>Spearman PCo1</th><th>Spearman PCo2</th><th>Cluster 1 log2(TPM+1) contrast</th></tr></thead><tbody></tbody></table></div></details>
      ${warnings.map(w=>`<p class="method-note">${escapeHtml(w)}</p>`).join('')}
    </div>`
  }
  return `<section class="card biological-findings" id="biological-findings"><div class="card-heading"><div><h3>Biological Findings</h3><p>One post-fit dashboard for all seven data types. Interpret the sample structure, review metadata, then inspect measured features.</p></div><span class="tag neutral">Exploratory interpretation</span></div>
    ${!structure?'<p class="method-note">Run Joint ordination and clustering first. The dashboard will then calculate its descriptive summaries automatically.</p>':''}
    <div class="form-grid bio-options">
      <label><span>Clinical or QC annotation</span><select id="bio-metadata-column" ${!structure?'disabled':''}><option value="">Color by joint cluster</option>${options.map(c=>`<option value="${escapeHtml(c)}" ${state.bioMetadataColumn===c?'selected':''}>${escapeHtml(c)}</option>`).join('')}</select></label>
      <label><span>Number of descriptive clusters</span><input id="bio-cluster-count" type="number" min="2" max="12" step="1" value="${state.bioClusterCount}" ${!structure?'disabled':''}/></label>
    </div>
    ${state.bioError?`<div class="analysis-warnings" role="alert">${escapeHtml(state.bioError)}</div>`:''}
    ${reportMarkup}
    <div class="bio-feature-heading"><div><h4>Associated molecular signals</h4><p>RNA rankings use gene and transcript TPM. Repertoire, isotype and immune feature summaries from the native source parsers are shown below after the direct source distances are recalculated.</p></div>
      <button id="bio-feature-run" type="button" class="primary-button" ${!report||state.busy||state.bioFeatureBusy?'disabled':''}>${state.bioFeatureBusy?'Reading local RNA…':'Rank associated features from both RNA types'}</button></div>
    ${state.bioFeatureBusy?`<progress class="bio-feature-progress" max="${Math.max(1,state.bioFeatureTotal)}" value="${state.bioFeatureBytes}" aria-label="Current RNA table bytes processed"></progress><p role="status" class="i2b-progress-highlight">${escapeHtml(state.bioFeatureProgress||'Reading local RNA data')}</p>`:''}
    ${state.bioFeatureError?`<div class="analysis-warnings" role="alert">${escapeHtml(state.bioFeatureError)}</div>`:''}
    ${featureMarkup||'<p class="method-note">Select the button once to rank available gene and transcript measurements together. The source files stay on this device. Missing data types remain marked as unavailable.</p>'}
    ${sourceBiologicalPanel()}
    <details class="details-block bio-exports"><summary>Download results and provenance</summary><div class="button-row">
      <button id="bio-export-json" type="button" class="secondary-button" ${!report?'disabled':''}>Joint interpretation JSON</button>
      <button id="bio-export-tsv" type="button" class="secondary-button" ${!report?'disabled':''}>Sample annotations TSV</button>
      <button id="bio-features-tsv" type="button" class="secondary-button" ${!feature?'disabled':''}>All RNA feature scores TSV</button>
      <button id="bio-features-json" type="button" class="secondary-button" ${!feature?'disabled':''}>All RNA feature scores JSON</button>
    </div></details>
    <p class="method-note"><strong>Scientific limit:</strong> This panel is post-fit and exploratory. The same molecular inputs helped define the joint clusters, so in-sample correlations do not establish independent biological findings. No post-hoc p-values, q-values or disease biomarker claims are calculated. Use patient-aware validation, source QC and replication.</p>
  </section>`
}
function runBiologicalFindings() {
  state.bioResult=null;state.bioError='';render()
}
async function runBiologicalFeatureRanking() {
  if(!state.bioResult||state.busy||state.bioFeatureBusy)return
  state.bioFeatureBusy=true;state.busy=true;state.bioFeatureError='';state.bioFeatureResult=null
  state.bioFeatureTotal=1;state.bioFeatureBytes=0;state.bioFeatureProgress='Starting local RNA feature ranking';render()
  const byModality={},warnings=[]
  try {
    const clusters=Object.fromEntries(state.bioResult.points.map(p=>[p.observation,p.cluster]))
    for(const kind of ['gene','transcript']) {
      const selected=kind==='gene'?state.compareConfig.geneMergedId:state.compareConfig.transcriptMergedId
      const candidates=recordsOfKind(kind==='gene'?'merged-gene-expression':'merged-transcript-expression')
      const record=candidates.find(r=>r.id===selected)||(candidates.length===1?candidates[0]:null)
      const file=record?selectedFile(record.id):null
      if(!file){warnings.push(`${kind}: no uniquely selected, connected merged TPM table`);continue}
      try {
        const samples=selectFeatureSamples(state.i2StructureResult,state.alignmentPreview,kind)
        state.bioFeatureTotal=file.size||1;state.bioFeatureBytes=0;state.bioFeatureProgress=`${kind}: reading local merged TPM`;render()
        const answer=await workerRequest('run-biological-feature-ranking',{file,modality:kind,samples,clusters},progress=>{
          state.bioFeatureBytes=Number(progress.completed)||0
          state.bioFeatureTotal=Math.max(1,Number(progress.total)||file.size||1)
          state.bioFeatureProgress=progress.message
          const bar=document.querySelector('.bio-feature-progress');if(bar){bar.max=state.bioFeatureTotal;bar.value=state.bioFeatureBytes}
          const status=document.querySelector('#biological-findings [role="status"]');if(status)status.textContent=state.bioFeatureProgress
        })
        if(answer.type!=='biological-feature-result'||answer.result?.schema!=='4c-insight-exploratory-expression-associations-v1')throw new Error('Unexpected feature ranking result')
        byModality[kind]=answer.result
      } catch(error) {
        if(/operation stopped|operation cancelled|aborted/i.test(String(error?.message||error)))throw error
        warnings.push(`${kind}: ${error instanceof Error?error.message:String(error)}`)
      }
    }
    if(!Object.keys(byModality).length)throw new Error(`No RNA source was ranked. ${warnings.join('; ')}`)
    state.bioFeatureResult={schema:'4c-insight-exploratory-combined-rna-v1',byModality,warnings}
    state.bioSelectedProfile=''
  }catch(error){state.bioFeatureError=error instanceof Error?error.message:String(error)}
  finally{state.bioFeatureBusy=false;state.busy=false;state.bioFeatureProgress='';render()}
}
function bindBiologicalFindingsEvents() {
  bindSourceBiologicalEvents()
  document.querySelector('#bio-metadata-column')?.addEventListener('change',event=>{
    state.bioMetadataColumn=event.currentTarget.value;state.bioResult=null;state.bioError='';render()
  })
  document.querySelector('#bio-cluster-count')?.addEventListener('change',event=>{
    const n=Number(event.currentTarget.value)
    if(Number.isInteger(n)&&n>=2&&n<=12)state.bioClusterCount=n
    state.bioResult=null;state.bioFeatureResult=null;state.bioSelectedProfile='';state.bioError='';render()
  })
  document.querySelector('#bio-feature-run')?.addEventListener('click',()=>void runBiologicalFeatureRanking())
  document.querySelector('#bio-feature-axis')?.addEventListener('change',event=>{state.bioFeatureAxis=event.currentTarget.value;render()})
  document.querySelector('#bio-feature-inspect')?.addEventListener('change',event=>{state.bioSelectedProfile=event.currentTarget.value;render()})
  document.querySelectorAll('[data-bio-profile]').forEach(button=>button.addEventListener('click',event=>{
    state.bioSelectedProfile=event.currentTarget.dataset.bioProfile;render()
  }))
  const save=(name,text,isFeature=false)=>void saveTextFile(name,text).catch(error=>{
    if(isFeature)state.bioFeatureError=error instanceof Error?error.message:String(error)
    else state.bioError=error instanceof Error?error.message:String(error)
    render()
  })
  document.querySelector('#bio-export-json')?.addEventListener('click',()=>state.bioResult&&save('4c-insight-biological-findings.json',JSON.stringify(state.bioResult,null,2)+'\n'))
  document.querySelector('#bio-export-tsv')?.addEventListener('click',()=>state.bioResult&&save('4c-insight-biological-findings.tsv',findingsTsv(state.bioResult)))
  document.querySelector('#bio-features-tsv')?.addEventListener('click',()=>state.bioFeatureResult&&save('4c-insight-all-rna-feature-associations.tsv',combinedFeatureTsv(state.bioFeatureResult),true))
  document.querySelector('#bio-features-json')?.addEventListener('click',()=>state.bioFeatureResult&&save('4c-insight-all-rna-feature-associations.json',JSON.stringify(state.bioFeatureResult,null,2)+'\n',true))
}

// Cache one post-fit analysis per fixed structure, alignment, cluster cut and
// direct source object identity. Do not recompute thousands of rank correlations
// on every UI render or progress message.
let sourceBiologicalCache = null
function cachedSourceBiological(structure,alignment,views,clusters,clusterCount){
  const prev=sourceBiologicalCache
  if(prev && prev.structure===structure && prev.alignment===alignment && prev.clusterCount===clusterCount &&
    prev.views.length===views.length && prev.views.every((view,i)=>view===views[i]))return prev.result
  const result=rankDirectSourceFeatures(structure,alignment,views,clusters)
  sourceBiologicalCache={structure,alignment,views,clusterCount,result}
  return result
}
// Post-fit measured features and sample inspection, without further file parsing.
function sourceBiologicalPanel(){
  const structure=state.i2StructureResult, alignment=state.alignmentPreview
  if(!structure||!alignment)return ''
  let found,views
  try{
    views=i2AvailableViews()
    const clusters=Object.fromEntries((state.bioResult?.points||[]).map(p=>[p.observation,p.cluster]))
    found=cachedSourceBiological(structure,alignment,views,clusters,state.bioClusterCount)
  }catch(error){return `<p class="analysis-warnings" role="alert">Source feature interpretation: ${escapeHtml(error.message)}</p>`}
  const labels=Object.fromEntries(I1_MODALITIES.map(m=>[m.key,m.label]))
  const ids=structure.observations
  const selected=ids.includes(state.bioInspectObservation)?state.bioInspectObservation:ids[0]
  let inspect
  try{ inspect=inspectJointObservation(structure,alignment,views,selected) }
  catch(error){ return `<p class="analysis-warnings" role="alert">Sample inspection: ${escapeHtml(error.message)}</p>` }
  const outliers=ids.map((id,i)=>{
    const n=ids.length, ds=ids.map((_,j)=>structure.distances[i*n+j]).filter((v,j)=>j!==i&&Number.isFinite(v))
    ds.sort((a,b)=>a-b)
    return {id,median:ds.length?ds[Math.floor(ds.length/2)]:null}
  }).filter(x=>x.median!==null).sort((a,b)=>b.median-a.median).slice(0,8)
  const reportRows=Object.values(found.byModality).flatMap(m=>m.rows)
  const profiles=Object.values(found.byModality).flatMap(m=>m.profiles)
  const selectedProfile=profiles.find(p=>`${p.modality}|${p.family}|${p.feature}`===state.bioSelectedSourceFeature)||profiles[0]
  const summary=I1_MODALITIES.filter(m=>!['gene','transcript'].includes(m.key)).map(m=>{
    const data=found.byModality[m.key]
    return `<article class="bio-native-modality"><strong>${escapeHtml(m.label)}</strong><span>${data?`${data.features_ranked.toLocaleString()} associated signals · ${data.sample_count} measured samples`:'Unavailable — recalculate local source distances'}</span>
      ${data?.rows?.length?data.rows.slice(0,5).map(r=>{
        const rho=r.pcoa1_spearman??0
        const w=(Math.min(1,Math.abs(rho))*50).toFixed(2),left=rho>=0?50:50-Number(w)
        return `<div class="bio-native-rank"><span title="${escapeHtml(r.family)}">${escapeHtml(r.feature)}</span><div class="bio-rank-track"><i class="bio-rank-zero"></i><b style="left:${left}%;width:${w}%;background:${rho<0?'#bd4656':'#3159d9'}"></b></div><small>${formatNumber(rho,3)}</small></div>`
      }).join(''):'<p class="method-note">No source-level feature scores are available.</p>'}</article>`
  }).join('')
  const exactRows=reportRows.map(r=>({ ...r, modality_label:labels[r.modality]||r.modality }))
  dynamicTableSources.set('bio-native-features',{
    rows:exactRows,
    cells:r=>[r.modality_label,r.family,r.feature,r.unit,r.n_used,r.pcoa1_spearman,r.pcoa2_spearman,r.cluster1_mean_difference],
    rowHtml:r=>`<tr><td>${escapeHtml(r.modality_label)}</td><td>${escapeHtml(r.family)}</td><td>${escapeHtml(r.feature)}</td><td>${escapeHtml(r.unit)}</td><td>${r.n_used}</td><td>${r.pcoa1_spearman===null?'—':formatNumber(r.pcoa1_spearman,4)}</td><td>${r.pcoa2_spearman===null?'—':formatNumber(r.pcoa2_spearman,4)}</td><td>${r.cluster1_mean_difference===null?'—':formatNumber(r.cluster1_mean_difference,4)}</td></tr>`,
  })
  let profilePlot=''
  if(selectedProfile&&state.bioResult){
    const numbers=new Map(selectedProfile.values.map(v=>[v.observation,v.value]))
    const vals=[...numbers.values()].filter(Number.isFinite)
    if(vals.length){
      const lo=Math.min(...vals),hi=Math.max(...vals)
      const pts=state.bioResult.points
      const [xmin,xmax]=paddedExtent(pts.map(p=>p.x)),[ymin,ymax]=paddedExtent(pts.map(p=>p.y))
      const w=650,h=310,pad=48
      const circles=pts.map(p=>{
        const value=numbers.get(p.observation),valid=Number.isFinite(value)
        const ratio=valid?(hi>lo?(value-lo)/(hi-lo):0.5):0
        const color=valid?`rgb(${Math.round(237-196*ratio)},${Math.round(240-163*ratio)},${Math.round(250-35*ratio)})`:'#bbc4d3'
        return `<circle data-x="${p.x}" data-y="${p.y}" cx="${(pad+(p.x-xmin)/(xmax-xmin)*(w-pad*2)).toFixed(2)}" cy="${(h-pad-(p.y-ymin)/(ymax-ymin)*(h-pad*2)).toFixed(2)}" r="4.3" fill="${color}"><title>${escapeHtml(p.observation)} · ${escapeHtml(selectedProfile.feature)}: ${valid?formatNumber(value,3):'Missing'} ${escapeHtml(selectedProfile.unit)}</title></circle>`
      }).join('')
      profilePlot=`<figure class="bio-native-plot"><svg viewBox="0 0 ${w} ${h}" data-coordinate-min-x="${xmin}" data-coordinate-max-x="${xmax}" data-coordinate-min-y="${ymin}" data-coordinate-max-y="${ymax}" aria-label="Fixed joint coordinates colored by measured ${escapeHtml(selectedProfile.feature)}"><rect x="${pad}" y="${pad}" width="${w-2*pad}" height="${h-2*pad}" fill="white" stroke="#e3e8f1"/>${circles}</svg><figcaption>${escapeHtml(labels[selectedProfile.modality])} · ${escapeHtml(selectedProfile.family)} · ${escapeHtml(selectedProfile.unit)} · range ${formatNumber(lo,3)}–${formatNumber(hi,3)} · gray: missing. Fixed PCoA coordinates.</figcaption></figure>`
    }
  }
  const qcTable=state.alignmentTable
  const qcCells=qcTable?.rows?.[inspect.metadata_row-2]||[]
  const qcFields=(qcTable?.columns||[]).map((name,i)=>({name,value:qcCells[i]}))
    .filter(x=>/qc|quality|batch|status|failure|processing|site|lane/i.test(x.name))
  const modalities=inspect.modalities.map(row=>`<tr><td>${escapeHtml(labels[row.modality]||row.modality)}</td><td>${escapeHtml(row.status)}</td><td>${row.matched_pairs}/${row.expected_pairs}</td><td>${row.median_distance===null?'—':formatNumber(row.median_distance,4)}</td><td>${escapeHtml(row.nearest_neighbor||'—')}</td><td>${row.nearest_distance===null?'—':formatNumber(row.nearest_distance,4)}</td></tr>`).join('')
  const summaryFeatures=inspect.modalities.flatMap(row=>row.features.filter(f=>f.family==='Repertoire metric'||f.family==='Immune signature score'||f.family==='Isotype fraction').slice(0,8).map(f=>({modality:row.modality,...f})))
  return `<div class="bio-native-section"><h4>Measured repertoire, isotype and immune features</h4>
    <p class="method-note">Five direct-workbook data types, scored on the fixed joint sample coordinates. Receptor features include native diversity/QC metrics and V/D/J fractions; isotypes use per-sample proportions; immune values use the reported signature scores. Zero is used only for an absent feature within a complete measured composition. Missing sources remain missing.</p>
    <div class="bio-native-grid">${summary}</div>
    ${found.warnings.map(w=>`<p class="method-note">${escapeHtml(w)}</p>`).join('')}
    ${profiles.length?`<label>Inspect associated signal <select id="bio-native-profile">${profiles.map(p=>{const id=`${p.modality}|${p.family}|${p.feature}`;return `<option value="${escapeHtml(id)}" ${p===selectedProfile?'selected':''}>${escapeHtml(labels[p.modality])}: ${escapeHtml(p.feature)}</option>`}).join('')}</select></label>${profilePlot}`:''}
    <details class="details-block"><summary>Search all ${reportRows.length.toLocaleString()} measured non-RNA feature associations</summary><div class="table-scroll differential-table" data-dynamic-table="bio-native-features"><table><thead><tr><th>Data type</th><th>Family</th><th>Feature</th><th>Unit</th><th>Samples</th><th>Spearman PCo1</th><th>Spearman PCo2</th><th>Cluster 1 mean difference</th></tr></thead><tbody></tbody></table></div></details>
    <details class="details-block"><summary>Export non-RNA features</summary><button id="bio-native-tsv" type="button" class="secondary-button">All measured features TSV</button><button id="bio-native-json" type="button" class="secondary-button">Measured feature associations JSON</button></details>
    <h4>Inspect an individual observation</h4><p class="method-note">Choose any retained observation to review source-specific distances and available measured features. This is a QC and biological hypothesis review, not an automated outlier test.</p>
    <label>Fitted observation <select id="bio-inspect-observation"><optgroup label="Largest median joint distances — QC review">${outliers.map(x=>`<option value="${escapeHtml(x.id)}" ${x.id===selected?'selected':''}>${escapeHtml(x.id)} · ${formatNumber(x.median,3)}</option>`).join('')}</optgroup><optgroup label="All retained observations">${ids.map(id=>`<option value="${escapeHtml(id)}" ${id===selected&&!outliers.some(x=>x.id===id)?'selected':''}>${escapeHtml(id)}</option>`).join('')}</optgroup></select></label>
    <p class="method-note">Patient ${escapeHtml(inspect.patient||'Not assigned')} · Visit ${escapeHtml(inspect.timepoint||'Not selected')} · Median joint distance ${inspect.joint_median_distance===null?'—':formatNumber(inspect.joint_median_distance,4)}</p>
    ${qcFields.length?`<p class="method-note"><strong>Recorded QC and batch metadata:</strong> ${qcFields.map(x=>`${escapeHtml(x.name)}: ${escapeHtml(x.value||'Missing')}`).join(' · ')}</p>`:'<p class="method-note">No QC or batch columns identified in the active metadata.</p>'}
    <p class="method-note"><strong>Nearest in joint distances:</strong> ${inspect.joint_nearest.map(x=>`${escapeHtml(x.observation)} (${formatNumber(x.distance,4)})`).join(' · ')}</p>
    <div class="table-scroll"><table><thead><tr><th>Data type</th><th>Source status</th><th>Measured pairs</th><th>Median distance</th><th>Nearest observation</th><th>Nearest distance</th></tr></thead><tbody>${modalities}</tbody></table></div>
    <details class="details-block"><summary>Measured values for this observation (${summaryFeatures.length} shown)</summary><div class="table-scroll"><table><thead><tr><th>Data type</th><th>Measurement</th><th>Value</th><th>Unit</th></tr></thead><tbody>${summaryFeatures.map(f=>`<tr><td>${escapeHtml(labels[f.modality])}</td><td>${escapeHtml(f.name)}</td><td>${formatNumber(f.value,4)}</td><td>${escapeHtml(f.unit)}</td></tr>`).join('')}</tbody></table></div></details>
    <button id="bio-inspect-json" type="button" class="secondary-button">Save observation review JSON</button>
    <p class="method-note">All associations are post-fit and descriptive. No new p-values, FDR or biomarker claims are calculated. Confirm signals using patient-aware inference and external samples.</p>
  </div>`
}
function bindSourceBiologicalEvents(){
  document.querySelector('#bio-native-profile')?.addEventListener('change',event=>{state.bioSelectedSourceFeature=event.currentTarget.value;render()})
  document.querySelector('#bio-inspect-observation')?.addEventListener('change',event=>{state.bioInspectObservation=event.currentTarget.value;render()})
  const getResults=()=>{
    const clusters=Object.fromEntries((state.bioResult?.points||[]).map(p=>[p.observation,p.cluster]))
    return cachedSourceBiological(state.i2StructureResult,state.alignmentPreview,i2AvailableViews(),clusters,state.bioClusterCount)
  }
  document.querySelector('#bio-native-tsv')?.addEventListener('click',()=>void saveTextFile('4c-insight-non-rna-source-features.tsv',directFeatureTsv(getResults())))
  document.querySelector('#bio-native-json')?.addEventListener('click',()=>void saveTextFile('4c-insight-non-rna-source-features.json',JSON.stringify(getResults(),null,2)+'\n'))
  document.querySelector('#bio-inspect-json')?.addEventListener('click',()=>{
    const id=state.bioInspectObservation||state.i2StructureResult?.observations?.[0]
    if(!id)return
    void saveTextFile('4c-insight-observation-review.json',JSON.stringify(inspectJointObservation(state.i2StructureResult,state.alignmentPreview,i2AvailableViews(),id),null,2)+'\n')
  })
}

// The full analysis reuses the existing scientific functions. It does not change
// their formulas. Required failures stop later stages; unsupported optional
// interpretation stages remain visible as warnings rather than false passes.
let activeIntegrativeRunController = null
// The original alignment and file identities are pinned for this run. Do not
// restore it if any source, selected role, exclusion, or input file changed.
function integrativeInputFingerprint() {
  const records=(state.records||[]).map(r=>[r.id,r.name,r.relativePath,r.kind]).sort((a,b)=>String(a[0]).localeCompare(String(b[0])))
  const files=[...state.files.entries()].map(([id,f])=>[id,f?.name,f?.size,f?.lastModified]).sort((a,b)=>String(a[0]).localeCompare(String(b[0])))
  return JSON.stringify({records,files,metadata:state.qcConfig.metadataId,
    gene:state.compareConfig.geneMergedId,transcript:state.compareConfig.transcriptMergedId,
    source:state.alignmentSource,roles:state.alignmentRoles,
    exclusions:i1ExclusionRules(),modalities:state.hitConfig.integrativeModalities||{}})
}
function integrativeRestoreCheckedAlignment(pinned) {
  if(state.alignmentPreview && state.alignmentTable)return {recovered:false}
  if(!pinned?.preview || !pinned.table || pinned.fingerprint!==integrativeInputFingerprint()) {
    throw new Error('Sample alignment became unavailable after processing sources. Input files or alignment settings changed. Recheck sample alignment before continuing; saved distances were not treated as aligned.')
  }
  if(pinned.preview.schema!=='4c-insight-observation-alignment-v1')
    throw new Error('The previous alignment had an unexpected schema; recheck sample alignment')
  state.alignmentPreview=pinned.preview
  state.alignmentTable=pinned.table
  return {recovered:true}
}
function ensureBiologicalFindings() {
  const structure=state.i2StructureResult
  if(structure && state.alignmentPreview && state.alignmentTable && !state.bioResult && !state.bioError) {
    try {
      state.bioResult = buildBiologicalFindings(state.i2StructureResult,state.alignmentPreview,state.alignmentTable,i2AvailableViews(),{
        column:state.bioMetadataColumn,clusters:Number(state.bioClusterCount),
      })
    } catch(error) { state.bioError = error instanceof Error ? error.message : String(error) }
  }
  return state.bioResult
}
let currentBiologicalNextRows=[]
let cachedIntegrativeCoverage=null
function currentIntegrativeCoverage(){
  const {alignmentPreview:alignment,i2StructureResult:structure,i2Result:distances}=state
  if(cachedIntegrativeCoverage&&cachedIntegrativeCoverage.alignment===alignment&&
    cachedIntegrativeCoverage.structure===structure&&cachedIntegrativeCoverage.distances===distances)return cachedIntegrativeCoverage.result
  const result=buildIntegrativeCoverage(alignment,structure,distances)
  cachedIntegrativeCoverage={alignment,structure,distances,result}
  return result
}
function showIntegrativeSampleMetadata(observation){
  const target=document.querySelector('[data-scatter-metadata]')
  if(!target)return
  const table=state.alignmentTable
  const observationColumn=state.alignmentRoles.observation
  const idIndex=table?.columns?.indexOf(observationColumn)??-1
  const row=idIndex>=0?table.rows?.find(values=>String(values[idIndex]??'').trim()===observation):null
  target.replaceChildren()
  target.hidden=false
  if(!row){target.textContent='Full metadata for this observation is not available. Review the Sample Metadata page.';return}
  const heading=document.createElement('strong')
  heading.textContent=`Local sample metadata: ${observation}`
  target.append(heading)
  const list=document.createElement('dl')
  for(let i=0;i<table.columns.length;i++){
    const term=document.createElement('dt'),value=document.createElement('dd')
    term.textContent=table.columns[i]
    value.textContent=String(row[i]??'')||'Not recorded'
    list.append(term,value)
  }
  target.append(list)
}
function integrativeOverviewPanel() {
  const structure=state.i2StructureResult
  if(!structure) {currentBiologicalNextRows=[];return `<section class="card integrative-overview" role="tabpanel" id="integrative-overview-panel" aria-labelledby="integrative-overview-tab">
    ${state.integrativeRunError?`<div class="analysis-warnings" role="alert"><strong>Analysis stopped:</strong> ${escapeHtml(state.integrativeRunError)} <button id="integrative-review-error" type="button" class="secondary-button">Review error details</button></div>`:''}
    <h3>Your results at a glance</h3><p class="method-note">Select <strong>Run complete analysis</strong>. The application will align samples, calculate source distances, fit the joint sample structure, and prepare available biological findings. Results will appear here automatically. Review methods and exports in Details.</p>
    <p class="method-note">The analysis uses local files. It does not require Compare Groups or disease labels.</p>
  </section>`}
  const report=ensureBiologicalFindings()
  let source=null,sourceError=''
  try {
    const clusters=Object.fromEntries((report?.points||[]).map(p=>[p.observation,p.cluster]))
    source=cachedSourceBiological(structure,state.alignmentPreview,i2AvailableViews(),clusters,state.bioClusterCount)
  }catch(error){sourceError=error instanceof Error?error.message:String(error)}
  const m=integratedOverviewMetrics({alignment:state.alignmentPreview,structure,stability:state.jointStabilityResult,features:state.bioFeatureResult,sourceFeatures:source})
  const counts=source?.byModality||{}
  const nonRnaCards=I1_MODALITIES.filter(x=>!['gene','transcript'].includes(x.key)).map(x=>{
    const record=counts[x.key]
    const signal=record?.rows?.[0]
    return `<article class="integrative-insight-card"><strong>${escapeHtml(x.label)}</strong><span>${record?`${record.features_ranked.toLocaleString()} ranked signals · ${record.sample_count} measured observations`:'No saved sample-level features'}</span>
      ${signal?`<small>Leading association: ${escapeHtml(signal.feature)} (${signal.pcoa1_spearman===null?'—':formatNumber(signal.pcoa1_spearman,3)} with PCo1)</small>`:'<small>Review source coverage and feature provenance in Details.</small>'}</article>`
  }).join('')
  const kept=m.retained===null?'—':m.retained.toLocaleString()
  const eligible=m.eligible===null?'—':m.eligible.toLocaleString()
  const metadataColumns=(state.alignmentTable?.columns||[]).filter(c=>c!==state.alignmentRoles.observation)
  const coverageReport=currentIntegrativeCoverage()
  currentBiologicalNextRows=biologicalCandidates(state.bioFeatureResult,source)
  return `<section class="card integrative-overview" role="tabpanel" id="integrative-overview-panel" aria-labelledby="integrative-overview-tab">
    <div class="card-heading"><div><h3>Main findings</h3><p>Sample coverage, joint patterns and feature associations.</p></div><span class="tag neutral">Exploratory</span></div>
    ${integrativeSummaryHtml({eligible:m.eligible,retained:m.retained,modalities:m.modalities,clusters:report?.membership?.length??null,ari:m.meanAri,escape:escapeHtml})}
    ${report?integrativeClusterCautionHtml(report,escapeHtml):''}
    <div class="integrative-overview-section integrative-overview-coverage"><h4>Who was included in the joint model?</h4>
      ${integrativeCoverageHtml(coverageReport)}
    </div>
    <div class="integrative-overview-grid">
      ${report?`<article class="plot-card"><div class="plot-heading"><strong>Joint sample patterns</strong><span>Unsupervised PCoA · cluster sizes describe observations, not confirmed biological groups.</span></div>
        <div class="integrative-plot-metadata"><label>Color points by <select id="integrative-overview-annotation"><option value="">Joint clusters</option>${metadataColumns.map(c=>`<option value="${escapeHtml(c)}" ${c===state.bioMetadataColumn?'selected':''}>${escapeHtml(c)}</option>`).join('')}</select></label></div>
        ${report.metadata?.too_many_categories?'<p class="method-note">This metadata column has too many categories for separate colours. Cluster colours are shown. Choose a grouping column with fewer categories.</p>':''}
        ${biologicalScatterSvg(report)}</article>`:'<article class="plot-card"><h4>Joint sample patterns</h4><p class="method-note">The metadata interpretation is not available. See Details for the reason.</p></article>'}
      ${report?`<article class="plot-card"><div class="plot-heading"><strong>Data type coverage and agreement</strong><span>Pair coverage is measured within retained observations. Agreement with the joint distances is descriptive, not independent feature importance.</span></div>${modalityOverviewHtml(report)}</article>`:'<article class="plot-card"><p class="method-note">Modality interpretation needs a valid joint result.</p></article>'}
    </div>
    <div class="integrative-overview-section"><h4>How much does each data type affect the joint structure?</h4>
      ${integrativeInfluenceHtml(structure,escapeHtml,formatNumber)}
    </div>
    <div class="integrative-overview-section"><h4>Features associated with joint sample patterns</h4>
      <p class="method-note">These features are ranked by correlation with the fitted joint coordinates. They are not statistically significant findings, independent differential-expression results, or evidence of clinical subtypes.</p>
      ${state.bioFeatureResult?featureLollipopHtml(state.bioFeatureResult,'pcoa1_spearman',5):'<p class="method-note">Gene and transcript features have not been ranked. Check the workflow steps above, then inspect source availability in Details.</p>'}
      <div class="integrative-insight-grid">${nonRnaCards}</div>
      <button id="integrative-open-source-details" type="button" class="secondary-button">Inspect measured receptor, isotype and immune features in Details</button>
      ${sourceError?`<p class="analysis-warnings">Source-feature review: ${escapeHtml(sourceError)}</p>`:''}
      <p class="method-note">Feature rankings are post-fit associations, not independent differential-expression results. No p-values, FDR, pathway claims or clinical subgroup labels are inferred.</p>
    </div>
    <div class="integrative-overview-section"><h4>What to check before biological interpretation</h4>
      <p class="method-note">Inspect distant samples and their metadata. Check the observations excluded from clustering, review sensitivity when a data type is removed, and examine patient-level stability. Cluster agreement does not confirm biological groups or clinical subtypes.</p>
      <button id="integrative-open-details" type="button" class="secondary-button">Open detailed results</button>
    </div>
    ${biologicalNextStepsHtml({report,coverage:coverageReport,rna:state.bioFeatureResult,source,compare:state.compareResult,
      thresholds:{...state.globalThresholds,omicsMinAdjustedEffect:state.hitConfig.omicsMinAdjustedEffect??0},
      enabled:state.hitConfig.integrativeModalities||{},observationRole:state.alignmentRoles.observation,
      patientRole:state.alignmentRoles.subject})}
  </section>`
}
/** Pathways never depends on a prior Overview render or re-runs the fitted joint model. */
function pathwaysSavedGeneRows() {
  return biologicalCandidates(state.bioFeatureResult, null)
}
function integrativePathwaysPanel() {
  const corrected=Boolean(state.compareResult?.gene?.analysis?.tests?.length)
  const exploratory=Boolean(pathwaysSavedGeneRows().some(row=>row.kind==='gene'))
  return `<section class="card integrative-pathways-view" role="tabpanel" id="integrative-pathways-panel" aria-labelledby="integrative-view-pathways">
    <div class="card-heading"><div><h3>Pathway analysis</h3><p>Choose a biological question, select gene sets, then calculate and inspect pathway enrichment on this device.</p></div><span class="tag neutral">Local analysis</span></div>
    ${!corrected?'<div class="pathway-guided-notice"><strong>No corrected gene comparison is available.</strong> Run Compare Groups for a predefined case/control analysis. <button id="pathway-open-compare" type="button" class="secondary-button">Open Compare Groups</button></div>':''}
    ${!exploratory?'<p class="method-note">Exploratory PCoA gene rankings are not available. Run complete Integrative Analysis to prepare them.</p>':''}
    <div id="pathway-analysis-widget"></div>
  </section>`
}
function integrativeDetailsPanel() {
  return `<section role="tabpanel" id="integrative-details-panel" aria-labelledby="integrative-details-tab" class="integrative-detail-layout">
    ${integrativeRunDiagnostics()}
    ${state.compareResult?`<details class="details-block"><summary>Previously calculated corrected evidence (optional)</summary>${renderIntegrativeEvidenceOverview()}</details>`:''}
    ${i1AlignmentPanel()}
    ${i2DistancePanel()}
    ${jointStructurePanel()}
    ${jointStabilityPanel()}
    ${biologicalFindingsPanel()}
  </section>`
}
async function runCompleteIntegrativeAnalysis() {
  if(state.integrativeRunBusy||state.busy||state.i2bBusy||state.i2Busy||state.alignmentBusy||state.i2StructureBusy||state.jointStabilityBusy||state.bioFeatureBusy)return
  const controller=new AbortController()
  activeIntegrativeRunController=controller
  state.integrativeRunBusy=true
  state.integrativeRunError=''
  state.integrativeView='overview'
  state.integrativeRunSteps=[]
  // Clear prior fitted results before a new analysis. Keep reusable aligned
  // source distances but never show stale clusters after a failed new run.
  state.i2Result=null
  state.i2StructureResult=null
  state.jointStabilityResult=null
  state.bioResult=null
  state.bioFeatureResult=null
  state.bioError=''
  state.bioFeatureError=''
  let pinnedAlignment=null
  const stages=[
    {id:'alignment',label:'Match sample and patient IDs',run:async()=>{
      const reused=Boolean(state.alignmentPreview && state.alignmentTable)
      if(!reused)await i1RunAlignment()
      if(!state.alignmentPreview||!state.alignmentTable)throw new Error(state.alignmentError||'Sample alignment was not available')
      pinnedAlignment={preview:state.alignmentPreview,table:state.alignmentTable,fingerprint:integrativeInputFingerprint()}
      return {status:reused?'skipped':'complete',message:`${reused?'Reused ':''}${state.alignmentPreview.summary.eligible} eligible observations`}
    }},
    {id:'sources',label:'Calculate measured source distances',run:async()=>{
      const chosen=I1_MODALITIES.filter(x=>state.hitConfig.integrativeModalities?.[x.key]!==false).map(x=>x.key)
      const ready=i2AvailableViews()
      if(ready.length>=2 && chosen.every(k=>ready.some(v=>v.modality===k)))return{status:'skipped',message:`Reused ${ready.length} available data types`}
      if(!state.wasmReady && ready.length<2)throw new Error('The local WASM engine is not ready')
      if(state.wasmReady)await i2bRunDirect()
      const views=i2AvailableViews()
      if(views.length<2)throw new Error('Fewer than two usable distance data types. Review local files and source errors in Details.')
      const restored=integrativeRestoreCheckedAlignment(pinnedAlignment)
      const warnings=state.i2bErrors.length+(restored.recovered?1:0)
      return {status:warnings?'warning':'complete',message:`${views.length} distance data types ready${state.i2bErrors.length?`; ${state.i2bErrors.length} source warning(s)`:''}${restored.recovered?'; verified alignment restored after processing':''}`}
    }},
    {id:'fusion',label:'Combine measured distances',run:async()=>{
      integrativeRestoreCheckedAlignment(pinnedAlignment)
      await i2RunFusion()
      if(!state.i2Result)throw new Error(state.i2Error||'No fused distances were produced')
      return{message:`${state.i2Result.fused_pairs} measured sample pairs`}
    }},
    {id:'structure',label:'Fit joint ordination and clustering',run:async()=>{
      await runJointStructure()
      if(!state.i2StructureResult)throw new Error(state.i2StructureError||'No joint structure was produced')
      return{message:`${state.i2StructureResult.observations.length} retained observations`}
    }},
    {id:'stability',label:'Check patient-level cluster stability',optional:true,run:async()=>{
      const rows=state.alignmentPreview?.rows||[]
      const linked=new Map(rows.filter(r=>!r.excluded).map(r=>[r.observation,r.subject||'']))
      if(!state.i2StructureResult.observations.every(id=>linked.get(id)))return{status:'skipped',message:'Patient IDs are unavailable; stability was not calculated'}
      await runJointStability()
      if(!state.jointStabilityResult)throw new Error(state.jointStabilityError||'Stability was not calculated')
      return{message:`${state.jointStabilityResult.repeats} patient-subset repeats`}
    }},
    {id:'interpretation',label:'Explain joint biological patterns',run:async()=>{
      if(!ensureBiologicalFindings())throw new Error(state.bioError||'Metadata interpretation is unavailable')
      return{message:'Metadata and seven-modality overview ready'}
    }},
    {id:'rna',label:'Rank available gene and transcript features',optional:true,run:async()=>{
      if(!recordsOfKind('merged-gene-expression').length&&!recordsOfKind('merged-transcript-expression').length)
        return{status:'skipped',message:'No merged expression table is connected'}
      await runBiologicalFeatureRanking()
      if(!state.bioFeatureResult)throw new Error(state.bioFeatureError||'RNA feature rankings were not produced')
      return{status:state.bioFeatureResult.warnings?.length?'warning':'complete',message:'RNA feature rankings available'}
    }},
  ]
  try {
    await runWorkflowSteps(stages,{signal:controller.signal,onUpdate:rows=>{state.integrativeRunSteps=rows;render()}})
  }catch(error){state.integrativeRunError=error instanceof Error?error.message:String(error)}
  finally{state.integrativeRunBusy=false;activeIntegrativeRunController=null;render()}
}
// Main controls are visible without opening the detailed scientific panels.
// All options reuse the same validated state and source calculations.
// Read the selected local metadata header before the first analysis run.
// Column names do not require a completed alignment or a second copy of source data.
let integrativeMetadataFile=null
let integrativeMetadataFailedFile=null
let integrativeMetadataPendingFile=null
let integrativeMetadataRequest=0
function integrativeSettingColumns() {
  return state.alignmentSource==='draft'
    ? (state.metadataEditor.table?.columns||[])
    : (state.alignmentColumns||[])
}
async function ensureIntegrativeMetadataColumns() {
  if(state.tab!=='hits')return
  if(state.alignmentSource==='draft') {
    const cols=integrativeSettingColumns()
    const changed=cols.join('\u0000')!==(state.alignmentColumns||[]).join('\u0000')
    if(changed) {
      state.alignmentColumns=[...cols]
      state.alignmentRoles=validIntegrativeIdentityRoles(cols,state.alignmentRoles,state.qcConfig.metadataIdColumn)
      state.integrativeColumnError=''
      render()
    }
    return
  }
  const file=selectedFile(state.qcConfig.metadataId)
  if(!file)return
  if(integrativeMetadataFile===file && state.alignmentColumns.length)return
  if(integrativeMetadataFailedFile===file)return
  if(integrativeMetadataPendingFile===file)return
  const request=++integrativeMetadataRequest
  integrativeMetadataPendingFile=file
  state.integrativeColumnBusy=true
  state.integrativeColumnError=''
  try {
    const columns=/\.xlsx?$/i.test(file.name)
      ? (await workerRequest('load-metadata-table',{file})).table?.columns
      : await readIntegrativeMetadataColumns(file)
    if(!Array.isArray(columns)||!columns.length)throw new Error('Metadata has no usable columns')
    if(request!==integrativeMetadataRequest || state.alignmentSource!=='active' || selectedFile(state.qcConfig.metadataId)!==file)return
    state.alignmentColumns=[...columns]
    state.alignmentRoles=validIntegrativeIdentityRoles(columns,state.alignmentRoles,state.qcConfig.metadataIdColumn)
    integrativeMetadataFile=file
    integrativeMetadataFailedFile=null
  }catch(error){
    if(request===integrativeMetadataRequest && state.alignmentSource==='active' && selectedFile(state.qcConfig.metadataId)===file){
      integrativeMetadataFailedFile=file
      state.integrativeColumnError=`Metadata columns: ${error instanceof Error?error.message:String(error)}`
    }
  }finally{
    if(integrativeMetadataPendingFile===file)integrativeMetadataPendingFile=null
    if(request===integrativeMetadataRequest){
      state.integrativeColumnBusy=false
      if(state.tab==='hits')render()
    }
  }
}
function integrativeMainSettings() {
  const busy=state.integrativeRunBusy||state.busy||state.i2bBusy||state.alignmentBusy
  const locked=busy?'disabled':''
  const cols=integrativeSettingColumns()
  const candidates=recordsOfKind('metadata-candidate')
  const choices=(selected,includeEmpty=false)=>`${includeEmpty?'<option value="">Not selected</option>':''}${cols.map(c=>`<option value="${escapeHtml(c)}" ${selected===c?'selected':''}>${escapeHtml(c)}</option>`).join('')}`
  const chosen=state.hitConfig.integrativeModalities||{}
  return `<section class="integrative-main-settings" aria-label="Main multi-omics settings">
    <div class="integrative-settings-heading"><strong>Main analysis settings</strong><span>Choose the sample and patient IDs and data types. Open Advanced settings for other controls.</span></div>
    <div class="integrative-settings-grid">
      <label>Metadata source<select id="integrative-setting-source" ${locked}><option value="active" ${state.alignmentSource==='active'?'selected':''}>Selected metadata file</option><option value="draft" ${state.alignmentSource==='draft'?'selected':''}>Open metadata draft</option></select></label>
      <label>Metadata file<select id="integrative-setting-metadata" ${locked} ${state.alignmentSource==='draft'?'disabled':''}>${candidates.map(r=>`<option value="${escapeHtml(r.id)}" ${r.id===state.qcConfig.metadataId?'selected':''}>${escapeHtml(r.name)}</option>`).join('')}</select></label>
      <label>Observation ID<select id="integrative-setting-observation" ${locked||(!cols.length?'disabled':'')}>${!cols.length?'<option value="">Reading metadata columns…</option>':choices(state.alignmentRoles.observation)}</select></label>
      <label>Patient ID<select id="integrative-setting-patient" ${locked||(!cols.length?'disabled':'')}>${!cols.length?'<option value="">Reading metadata columns…</option>':choices(state.alignmentRoles.subject,true)}</select></label>
      <label>Visit / timepoint<select id="integrative-setting-visit" ${locked||(!cols.length?'disabled':'')}>${!cols.length?'<option value="">Reading metadata columns…</option>':choices(state.alignmentRoles.time,true)}</select></label>
    </div>
    <details class="integrative-advanced-settings"><summary>Advanced settings · computation and sensitivity</summary><p class="method-note">Minimum data types per pair is used for broad pairwise distance fusion. The final joint ordination requires complete distances across all selected data types for every retained sample pair. These are different coverage requirements.</p><div class="integrative-settings-grid">
      <label>Parallel data types<select id="integrative-setting-workers" ${locked}>${[1,2,3].map(n=>`<option value="${n}" ${state.i2bParallelWorkers===n?'selected':''}>${n} ${n===1?'worker':'workers'}</option>`).join('')}</select></label>
      <label>Minimum data types per pair (fusion)<input id="integrative-setting-min-views" type="number" min="2" max="7" value="${state.i2MinViews}" ${locked}/></label>
      <label>Descriptive clusters (post-fit)<input id="integrative-setting-clusters" type="number" min="2" max="12" value="${state.bioClusterCount}" ${locked}/></label>
      <label>Patient stability repeats<input id="integrative-setting-repeats" type="number" min="10" max="500" value="${state.jointStabilityConfig.repeats}" ${locked}/></label>
    </div></details>
    <div class="integrative-settings-modalities" role="group" aria-label="Data types in complete analysis">${I1_MODALITIES.map(m=>`<label class="check-label"><input type="checkbox" data-integrative-main-modality="${m.key}" ${chosen[m.key]===false?'':'checked'} ${locked}/><span>${escapeHtml(m.label)}</span></label>`).join('')}</div>
    <label class="check-label integrative-one-patient"><input type="checkbox" id="integrative-setting-one-patient" ${state.i2StructureOnePerPatient?'checked':''} ${locked}/><span>One observation per patient in the joint structure</span></label>
    ${state.alignmentRoles.observation&&state.alignmentRoles.subject===state.alignmentRoles.observation?`<p class="notice-banner" role="status">Observation ID and Patient ID use the same column. This is correct only when each patient has one observation. Choose a separate Patient ID column for repeated visits.</p>`:''}
    ${state.integrativeColumnError?`<p class="analysis-warnings" role="alert">${escapeHtml(state.integrativeColumnError)}</p>`:''}
    ${!cols.length && !state.integrativeColumnError?`<p class="method-note" role="status">${state.alignmentSource==='draft'?'Open a local metadata draft in Sample Metadata before selecting its columns.':'Reading column names from the selected local metadata file. The analysis does not start until the columns are available.'}</p>`:''}
    <p class="method-note">A single metadata column can identify both the observation and the patient when each patient has one sample. Changing source IDs or data types requires fresh alignment. No disease labels enter model fitting.</p>
  </section>`
}
function integrativeRunDiagnostics() {
  const stages=state.integrativeRunSteps||[]
  const errors=[]
  const add=(label,value)=>{if(value)errors.push({label,message:String(value)})}
  add('Complete workflow',state.integrativeRunError)
  add('Sample alignment',state.alignmentError)
  add('Metadata column discovery',state.integrativeColumnError)
  for(const item of state.i2bErrors||[])add(`Source: ${item.modality}`,item.message)
  add('Distance fusion',state.i2Error)
  add('Joint structure',state.i2StructureError)
  add('Patient-level stability',state.jointStabilityError)
  add('Biological interpretation',state.bioError)
  add('RNA feature ranking',state.bioFeatureError)
  const failures=stages.filter(s=>s.status==='failed'||s.status==='stopped'||s.status==='warning')
  return `<section id="integrative-run-diagnostics" class="card integrative-diagnostics" aria-label="Workflow errors and validation">
    <div class="card-heading"><div><h3>Run diagnostics and source checks</h3><p>Problems are listed here, including errors that prevent later stages from starting.</p></div><span class="tag neutral">${errors.length||failures.length?'Review required':'No recorded errors'}</span></div>
    ${errors.length?`<div class="analysis-warnings" role="alert">${errors.map(e=>`<p><strong>${escapeHtml(e.label)}:</strong> ${escapeHtml(e.message)}</p>`).join('')}</div>`:'<p class="method-note">There are no recorded workflow errors. If an analysis is missing, inspect the stage status below.</p>'}
    ${stages.length?`<div class="table-scroll"><table><thead><tr><th>Stage</th><th>Status</th><th>Result or failure reason</th></tr></thead><tbody>${stages.map(s=>`<tr><td>${escapeHtml(s.label)}</td><td>${escapeHtml(s.status)}</td><td>${escapeHtml(s.message||'—')}</td></tr>`).join('')}</tbody></table></div>`:''}
    <p class="method-note">Current state: ${state.alignmentPreview?.summary?.eligible??'no'} eligible aligned observations; ${state.i2bDirectViews.length} direct source views; ${state.i2Result?'joint distances ready':'joint distances not fitted'}; ${state.i2StructureResult?'joint structure ready':'joint structure not fitted'}.</p>
    <p class="method-note">Source files remain on this device. Review diagnostic text before sharing it, because it may contain file or sample names.</p>
  </section>`
}
function invalidateIntegrativeFit() {
  state.i2Result=null;state.i2StructureResult=null;state.jointStabilityResult=null
  state.bioResult=null;state.bioFeatureResult=null;state.integrativeRunSteps=[];state.integrativeRunError=''
}
function bindIntegrativePrimarySettings() {
  const change=(id,fn)=>document.querySelector(`#${id}`)?.addEventListener('change',e=>{fn(e.currentTarget);render()})
  change('integrative-setting-source',el=>{state.alignmentSource=el.value;state.alignmentColumns=[];state.alignmentRoles={observation:'',subject:'',time:''};state.integrativeColumnError='';integrativeMetadataFile=null;integrativeMetadataFailedFile=null;integrativeMetadataPendingFile=null;state.integrativeColumnBusy=false;integrativeMetadataRequest++;i1ResetPreview();invalidateIntegrativeFit()})
  change('integrative-setting-metadata',el=>{state.qcConfig.metadataId=el.value;state.alignmentColumns=[];state.alignmentRoles={observation:'',subject:'',time:''};state.integrativeColumnError='';integrativeMetadataFile=null;integrativeMetadataFailedFile=null;integrativeMetadataPendingFile=null;state.integrativeColumnBusy=false;integrativeMetadataRequest++;i1ResetPreview();invalidateIntegrativeFit()})
  for(const [id,role] of [['integrative-setting-observation','observation'],['integrative-setting-patient','subject'],['integrative-setting-visit','time']])
    change(id,el=>{state.alignmentRoles[role]=el.value;i1ResetPreview();invalidateIntegrativeFit()})
  change('integrative-setting-workers',el=>{state.i2bParallelWorkers=validParallelWorkers(el.value)})
  change('integrative-setting-min-views',el=>{const n=Number(el.value);if(Number.isInteger(n)&&n>=2&&n<=7){state.i2MinViews=n;invalidateIntegrativeFit()}})
  change('integrative-setting-clusters',el=>{const n=Number(el.value);if(Number.isInteger(n)&&n>=2&&n<=12){state.bioClusterCount=n;state.jointStabilityConfig.clusters=n;state.bioResult=null;state.bioFeatureResult=null;state.jointStabilityResult=null;state.integrativeRunSteps=[]}})
  change('integrative-setting-repeats',el=>{const n=Number(el.value);if(Number.isInteger(n)&&n>=10&&n<=500){state.jointStabilityConfig.repeats=n;state.jointStabilityResult=null;state.integrativeRunSteps=[]}})
  change('integrative-setting-one-patient',el=>{state.i2StructureOnePerPatient=el.checked;invalidateIntegrativeFit()})
  document.querySelectorAll('[data-integrative-main-modality]').forEach(el=>el.addEventListener('change',event=>{
    const m=event.currentTarget.dataset.integrativeMainModality
    state.hitConfig.integrativeModalities||={}
    state.hitConfig.integrativeModalities[m]=event.currentTarget.checked
    state.i2StructureSelection[m]=event.currentTarget.checked
    i1ResetPreview();invalidateIntegrativeFit();render()
  }))
}
function bindIntegrativeRunControls(){
  bindIntegrativePrimarySettings()
  document.querySelector('#integrative-run-complete')?.addEventListener('click',()=>void runCompleteIntegrativeAnalysis())
  document.querySelector('#integrative-stop-run')?.addEventListener('click',stopScienceWork)
  document.querySelector('#integrative-view-overview')?.addEventListener('click',()=>{if(state.pathwayBusy)return;state.integrativeView='overview';render()})
  document.querySelector('#integrative-view-details')?.addEventListener('click',()=>{if(state.pathwayBusy)return;state.integrativeView='details';render()})
  document.querySelector('#integrative-view-pathways')?.addEventListener('click',()=>{if(state.pathwayBusy)return;state.integrativeView='pathways';render()})
  document.querySelector('#pathway-open-compare')?.addEventListener('click',()=>{if(state.pathwayBusy)return;state.tab='compare';render()})
  document.querySelector('.integrative-tab-controls[role="tablist"]')?.addEventListener('keydown',event=>{
    if(state.pathwayBusy)return
    const ids=['integrative-view-overview','integrative-view-details','integrative-view-pathways']
    const old=ids.indexOf(event.target?.id)
    if(old<0)return
    const next=event.key==='ArrowRight'?(old+1)%ids.length:event.key==='ArrowLeft'?(old+ids.length-1)%ids.length:event.key==='Home'?0:event.key==='End'?ids.length-1:-1
    if(next<0)return
    event.preventDefault()
    document.getElementById(ids[next])?.click()
    document.getElementById(ids[next])?.focus()
  })
  document.querySelector('#integrative-open-details')?.addEventListener('click',()=>{state.integrativeView='details';render()})
  document.querySelector('#integrative-overview-annotation')?.addEventListener('change',event=>{state.bioMetadataColumn=event.currentTarget.value;state.bioResult=null;state.bioError='';render()})
  bindIntegrativeScatter(document,showIntegrativeSampleMetadata)
  bindIntegrativeCoverage(document)
  bindBiologicalFeatureExplorer(document,currentBiologicalNextRows)
  mountPathwayModule(document,{workspace:state,compare:state.compareResult,exploratoryRows:pathwaysSavedGeneRows(),
    onChanged:scheduleLocalSessionSave,
    exportFile:(filename,content)=>void saveTextFile(filename,content,filename.endsWith('.tsv')?'text/tab-separated-values':filename.endsWith('.svg')?'image/svg+xml':'application/json')
      .catch(error=>{state.pathwayStatus=`Pathway export failed: ${error.message}`})})
  bindBiologicalNextStepActions()
  document.querySelector('#integrative-coverage-export')?.addEventListener('click',()=>{
    const report=currentIntegrativeCoverage()
    if(report)void saveTextFile('4c-insight-joint-cohort-omissions.tsv',integrativeOmittedTsv(report))
      .catch(error=>{state.integrativeRunError=`Omitted-observation export: ${error.message}`;render()})
  })
  document.querySelector('#integrative-open-source-details')?.addEventListener('click',()=>{state.integrativeView='details';render();document.querySelector('#biological-findings')?.scrollIntoView({block:'start'})})
  document.querySelector('#integrative-review-error')?.addEventListener('click',()=>{state.integrativeView='details';render()})
}

function bindBiologicalNextStepActions() {
  const status=document.querySelector('#bio-next-export-status')
  const message=text=>{if(status){status.textContent=text;status.hidden=false}}
  function localExport(filename,content) {
    const mime=filename.endsWith('.tsv')?'text/tab-separated-values':'application/json'
    void saveTextFile(filename,content,mime).then(()=>message(`Export requested: ${filename}`))
      .catch(error=>message(`Could not save: ${error instanceof Error?error.message:String(error)}`))
  }
  document.querySelectorAll('[data-bio-next]').forEach(button=>button.addEventListener('click',()=>{
    const action=button.dataset.bioNext
    if(action==='coverage')document.querySelector('.integrative-overview-coverage')?.scrollIntoView({block:'start'})
    if(action==='metadata'){
      document.querySelector('.integrative-overview-grid')?.scrollIntoView({block:'start'})
      document.querySelector('#integrative-overview-annotation')?.focus()
    }
    if(action==='compare'){
      // Group and control values must be defined by the user, not inferred from a fitted cluster.
      state.tab='compare';render()
    }
    if(action==='results'&&state.compareResult){state.tab='results';state.resultsMode='overview';render()}
    if(action==='features'){
      const explorer=document.querySelector('#bio-next-feature-explorer')
      if(explorer)explorer.open=true
      explorer?.scrollIntoView({block:'start'})
      document.querySelector('#bio-next-query')?.focus()
    }
    if(action==='pathways'){
      state.integrativeView='pathways';render()
    }
    if(action==='details'){
      state.integrativeView='details';render()
      document.querySelector('#biological-findings')?.scrollIntoView({block:'start'})
    }
    if(action==='export-features')localExport('4c-insight-all-measured-postfit-associations.tsv',biologicalCandidatesTsv(currentBiologicalNextRows))
    if(action==='export-genes'){
      const axis=document.querySelector('#bio-next-axis')?.value||'pcoa1_spearman'
      try{
        const result=exploratoryGeneRankingTsv(currentBiologicalNextRows,axis)
        if(result.total===0){message('No gene ranks are available. Check that the gene feature analysis completed.');return}
        localExport(`4c-insight-exploratory-gene-ranks-${axis}.tsv`,result.tsv)
        if(result.excludedDuplicateIds)message(`Export excludes ${result.excludedDuplicateIds} rows with duplicate gene identifiers. Check identifiers before enrichment.`)
      }catch(error){message(error.message)}
    }
    if(action==='export-audit'){
      const coverage=currentIntegrativeCoverage(),structure=state.i2StructureResult
      const audit={schema:'4c-insight-biological-interpretation-audit-v1',app_version:APP_VERSION,
        generated_at:new Date().toISOString(),status:'exploratory_not_independently_validated',
        validation:'No held-out or independent validation was performed by this workflow',
        fitted_cohort:{eligible:coverage?.eligible??null,retained:coverage?.retained??null,excluded:coverage?.excluded??null,
          exclusion_reasons:coverage?.byReason??null,cluster_sizes:ensureBiologicalFindings()?.membership??[],
          fitted_modalities:structure?.fixed_modalities??[],patient_id_column:state.alignmentRoles.subject||null,
          observation_id_column:state.alignmentRoles.observation||null},
        pcoa_ranking:{method:'Post-fit Spearman association; not FDR-controlled or causal',
          feature_rows:currentBiologicalNextRows.length},
        previous_comparison:state.compareResult?{name:state.compareResult.comparisonName||null,
          thresholds_display_only:{...state.globalThresholds,omicsMinAdjustedEffect:state.hitConfig.omicsMinAdjustedEffect??0},
          cohort_overlap_with_joint:'not_verified',independent_validation:false}:null,
        pathway_enrichment:state.pathwayResult?{status:'calculated',method:state.pathwayResult.method,
          source:state.pathwayResult.source,reference:state.pathwayResult.geneSetFile,
          tested_sets:state.pathwayResult.matchedSetCount,significant_BH_q005:state.pathwayResult.results.filter(r=>r.q_value<=0.05).length,
          independent_validation:false}: {status:'not_calculated',p_value:null,q_value:null},
        caution:'Do not treat post-fit cluster contrasts or Spearman ranks as confirmatory differential expression.'}
      localExport('4c-insight-interpretation-audit.json',JSON.stringify(audit,null,2)+'\n')
    }
  }))
}

function significantHitsView() {
  const busy=state.integrativeRunBusy
  const steps=state.integrativeRunSteps||[]
  const messages={queued:'○',running:'●',complete:'✓',warning:'!',failed:'×',skipped:'–',stopped:'■'}
  const ready=state.alignmentSource==='draft'?Boolean(state.metadataEditor.table):Boolean(selectedFile(state.qcConfig.metadataId))
  const disabled=busy||state.busy||state.i2bBusy||state.i2Busy||state.alignmentBusy||state.mergeBusy||!ready||!integrativeSettingColumns().length||state.integrativeColumnBusy||Boolean(state.integrativeColumnError)
  const stepMarkup=steps.map(row=>`<div class="integrative-step" data-status="${escapeHtml(row.status)}"><span class="integrative-step-marker" aria-hidden="true">${messages[row.status]||'○'}</span><p><strong>${escapeHtml(row.label)}</strong><small>${escapeHtml(row.message||row.status)}</small></p></div>`).join('')
  const overview=state.integrativeView==='overview'
  const detail=state.integrativeView==='details'
  const pathways=state.integrativeView==='pathways'
  const stepDone=steps.length>0 && steps.every(row=>['complete','skipped','warning'].includes(row.status)) && !busy
  const problems=steps.some(row=>['warning','failed','stopped'].includes(row.status))
  const completed=steps.filter(row=>row.status==='complete').length
  return `<section class="page-heading"><div><span class="eyebrow">Integrative Analysis</span><h2>Explore shared biological patterns across data types</h2><p>Run the full local analysis once, review the main findings, or open the detailed scientific results.</p></div><div class="local-card"><strong>On-device multi-omics</strong><span>Seven available data types · Exact sample alignment · No source upload</span></div></section>
    ${state.error?`<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>`:''}
    <section class="card integrative-entry"><div class="integrative-main-action">
      <div class="integrative-run-description"><h3>Complete multi-omics analysis</h3><p class="method-note">Match samples, calculate available source distances, fit the joint structure, check cluster stability, and summarize measured biological signals. Existing valid source results may be reused. Source data remains on this device.</p></div>
      <div class="integrative-top-controls"><button id="integrative-run-complete" type="button" class="primary-button" ${disabled?'disabled':''}>${busy?'Analysis running…':'Run complete analysis'}</button>${busy?'<button id="integrative-stop-run" type="button" class="secondary-button">Stop analysis</button>':''}</div>
    </div>
    ${integrativeMainSettings()}
    ${!ready?'<p class="notice-banner">Connect the local metadata file in Project or open a metadata draft before starting.</p>':''}
    ${steps.length?(stepDone?`<details class="integrative-step-disclosure" ${problems?"open":""}><summary>${completed} of ${steps.length} analysis stages complete${problems?" · review stage warnings":""}</summary><div class="integrative-run-steps" aria-label="Complete analysis stages">${stepMarkup}</div></details>`:`<div class="integrative-run-steps" aria-label="Complete analysis progress" role="status" aria-live="polite">${stepMarkup}</div>`):''}
    ${busy && steps.some(x=>x.id==='sources'&&x.status==='running')?`<div class="integrative-live-progress"><strong>Processing local source files</strong><progress id="integrative-live-source-bar" max="${Math.max(1,sourceProgressSummary(state.i2bProgressRows).total)}" value="${sourceProgressSummary(state.i2bProgressRows).completed}" aria-label="Source file processing progress"></progress><p id="integrative-live-source-status" role="status">${escapeHtml(state.i2bProgress||'Starting source workers')}</p></div>`:''}
    ${state.integrativeRunError?`<div class="analysis-warnings" role="alert"><strong>Analysis stopped:</strong> ${escapeHtml(state.integrativeRunError)}. Open Details to see the stage report and source errors.</div>`:''}
    <div class="integrative-tab-controls" role="tablist" aria-label="Integrative analysis results">
      <button id="integrative-view-overview" type="button" role="tab" aria-controls="integrative-overview-panel" aria-selected="${overview}" ${overview?'tabindex="0"':'tabindex="-1"'}>Overview</button>
      <button id="integrative-view-details" type="button" role="tab" aria-controls="integrative-details-panel" aria-selected="${detail}" ${detail?'tabindex="0"':'tabindex="-1"'}>Details</button>
      <button id="integrative-view-pathways" type="button" role="tab" aria-controls="integrative-pathways-panel" aria-selected="${pathways}" ${pathways?'tabindex="0"':'tabindex="-1"'}>Pathways</button>
    </div></section>
    ${pathways?integrativePathwaysPanel():detail?integrativeDetailsPanel():integrativeOverviewPanel()}`
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
  bindIntegrativeRunControls()
  const cfg = state.hitConfig
  bindJointStructureEvents()
  bindJointStabilityEvents()
  bindBiologicalFindingsEvents()
  bindI1AlignmentEvents()
  bindI2DistanceEvents()
  document.querySelectorAll('[data-integrative-modality]').forEach(input => {
    input.addEventListener('change', event => {
      if (!cfg.integrativeModalities || typeof cfg.integrativeModalities !== 'object') cfg.integrativeModalities = {}
      cfg.integrativeModalities[event.currentTarget.dataset.integrativeModality] = event.currentTarget.checked
      render()
    })
  })
  document.querySelector('#integrative-export-csv')?.addEventListener('click', () => {
    const rows = collectIntegrativeSelectedRows(state.compareResult, {
      ...state.globalThresholds, omicsMinAdjustedEffect: cfg.omicsMinAdjustedEffect ?? 0,
    }, cfg.integrativeModalities || {})
    void saveTextFile('4c-insight-integrative-selected-evidence.csv', adjustedTestsCsv(rows))
      .catch(error => { state.error = error instanceof Error ? error.message : String(error); render() })
  })
  document.querySelector('#hits-open-compare')?.addEventListener('click', () => { state.tab = 'compare'; render() })
  bindGlobalThresholdEvents()
  document.querySelector('#hit-max-analysis')?.addEventListener('change', (event) => { cfg.maxAnalysisFeatures = Math.max(2, Math.trunc(Number(event.currentTarget.value) || 500)); state.hitResult = null })
  document.querySelector('#hit-max-heatmap')?.addEventListener('change', (event) => { cfg.maxHeatmapFeatures = Math.max(1, Math.trunc(Number(event.currentTarget.value) || 80)); state.hitResult = null })
  document.querySelector('#hit-gene-enabled')?.addEventListener('change', (event) => { cfg.geneEnabled = event.currentTarget.checked; state.hitResult = null; render() })
  document.querySelector('#hit-transcript-enabled')?.addEventListener('change', (event) => { cfg.transcriptEnabled = event.currentTarget.checked; state.hitResult = null; render() })
  document.querySelector('#run-significant-hits')?.addEventListener('click', () => void runSignificantHits())
  document.querySelector('#export-significant-hits')?.addEventListener('click', () => void exportHitResult())
  document.querySelector('#hit-omics-min-effect')?.addEventListener('change', event => {
    const value = Number(event.currentTarget.value)
    cfg.omicsMinAdjustedEffect = Number.isFinite(value) && value >= 0 ? value : 0
    render()
  })
  document.querySelector('#export-omics-adjusted-hits')?.addEventListener('click', () => {
    const minimum = Number(state.hitConfig.omicsMinAdjustedEffect ?? 0)
    const rows = collectAdjustedTests(state.compareResult)
      .filter(group => state.hitConfig.integrativeModalities?.[group.key] !== false)
      .flatMap(group => selectAdjustedTests(group.rows, state.globalThresholds.fdr, minimum))
    void saveTextFile('4c-insight-adjusted-omics-hits.csv', adjustedTestsCsv(rows))
      .catch(error => { state.error = error instanceof Error ? error.message : String(error); render() })
  })
  bindInteractivePlots()
  attachStaticSvgNavigation(document)
}


function historyWorkflowTitle(workflow) {
  return ({
    'explore-qc-expression': 'Explore & QC',
    'compare-groups-expression': 'Compare Groups',
    'compare-groups-multiomics': 'Compare Groups',
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
  const sampleText = appMemoryDescription(state.appMemorySample)
  return `<section class="page-heading"><div><span class="eyebrow">Global settings</span><h2>Performance settings</h2><p>Settings apply to this browser profile, not to saved cohort states. Local scientific input files have no fixed size cap.</p></div><div class="local-card"><strong>Analysis</strong><span>Runs analyses on this device</span></div></section>
    ${state.error ? `<div class="error-banner" role="alert">${escapeHtml(state.error)}</div>` : ''}
    <section class="card"><div class="card-heading"><div><h3>CPU information</h3><p>Reserve part of the available logical CPU capacity for other work.</p></div></div>
      <div class="form-grid resource-grid">
        <label><span>CPU preference (%) (planned)</span><input id="resource-cpu" type="number" min="10" max="90" step="5" value="${r.cpuPercent}" /></label>
        <label><span>Preferred CPU threads (planned)</span><input id="resource-threads" type="number" min="1" max="32" step="1" value="${r.maxThreads}" /></label>
      </div>
      <p class="method-note">${info.threads ? `${info.threads} logical CPU threads reported; configured preference: ${suggested} threads.` : 'Logical CPU count is unavailable.'} Current analyses run one task at a time. This preference does not set a system-wide CPU limit.</p>
    </section>
    <section class="card"><div class="card-heading"><div><h3>Memory information and display limits</h3><p>Local input files are never rejected because of their size or an estimated copy budget. Very large inputs can still exceed the memory available to this page.</p></div></div>
      <div class="form-grid resource-grid">
        <label><span>Maximum volcano points drawn</span><input id="resource-points" type="number" min="200" max="6000" step="200" value="${r.maxPlotPoints}" /></label>
      </div>
      <div class="resource-memory-status" role="status" aria-live="polite"><strong>Analysis memory</strong><span id="wasm-memory-status">${escapeHtml(wasmMemoryDescription(state.wasmMemorySample))}</span><small>Memory reserved for calculations. This does not include all browser or device memory. Values are sampled between analysis steps.</small></div>
      ${info.deviceMemoryGiB !== null ? `<div class="resource-memory-status"><strong>Device RAM class (browser estimate)</strong><span>${escapeHtml(info.memory)}</span><small>${escapeHtml(info.memoryNote)}</small></div>` : ''}
      ${state.appMemorySample?.available ? `<div class="resource-memory-status" role="status" aria-live="polite"><strong>Browser app memory estimate</strong><span id="automatic-memory-status">${escapeHtml(sampleText)}</span><small>Not total or free system RAM.</small></div>` : ''}
      <p class="method-note">Some browsers do not report total or free device RAM. No additional software is required.</p>
      ${state.appMemorySample?.available ? `<div class="button-row"><button id="measure-app-memory" class="secondary-button" ${state.appMemoryBusy ? 'disabled' : ''}>${state.appMemoryBusy ? 'Measuring…' : 'Refresh app memory'}</button></div>` : ''}
      <label class="resource-check"><input id="resource-recycle" type="checkbox" ${r.recycleWorker ? 'checked' : ''}/> <span>Release analysis memory after each completed run (recommended).</span></label>
      <div class="button-row resource-actions"><button id="save-resources" class="primary-button">Apply settings</button><button id="reset-resources" class="secondary-button">Reset defaults</button></div>
    </section>`
}
function bindSettingsEvents() {
  document.querySelector('#save-resources')?.addEventListener('click', () => {
    state.resources = normalizeResources({
      cpuPercent: document.querySelector('#resource-cpu')?.value,
      maxThreads: document.querySelector('#resource-threads')?.value,
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
      await refreshAutomaticAppMemory()
    } finally {
      state.appMemoryBusy = false
      if (state.tab === 'settings') render()
    }
  })
}

function docsView() {
  const references = methodDocs.reduce((total, section) => total + (section.references || []).length, 0)
  return `<section class="page-heading"><div><span class="eyebrow">Docs</span><h2>Methods, assumptions, and references</h2><p>Explore each method with a labeled diagram or plot. Expand the toy numbers and calculation when you want more detail. These are learning examples, not cohort results.</p></div><div class="local-card"><strong>Literature-backed methods</strong><span>${methodDocs.length} sections · ${references} linked references</span></div></section>
    <section class="card docs-search-card"><label for="docs-search"><strong>Search documentation</strong><span>Search methods, terms, limitations, or references.</span></label><input id="docs-search" type="search" value="${escapeHtml(state.docsQuery)}" placeholder="e.g. NB2, PERMANOVA, longitudinal, FDR, CLR" autocomplete="off"/><small id="docs-search-status">${methodDocs.length} sections shown</small></section>
    <nav class="docs-toc" aria-label="Documentation sections">${methodDocs.map((section) => `<a href="#doc-${escapeHtml(section.id)}">${escapeHtml(section.title)}</a>`).join('')}</nav>
    <div class="docs-methods">${methodDocs.map((section) => {
      const searchable = [section.title, section.keywords, section.summary, ...(section.body || []), exampleSearchText(section.id), ...(section.references || []).map((reference) => reference.label)].join(' ').toLowerCase()
      return `<article id="doc-${escapeHtml(section.id)}" class="card docs-section" data-doc-section data-doc-search="${escapeHtml(searchable)}"><div class="card-heading"><div><h3>${escapeHtml(section.title)}</h3><p>${escapeHtml(section.summary)}</p></div><a class="text-button docs-anchor" href="#doc-${escapeHtml(section.id)}" aria-label="Link to ${escapeHtml(section.title)}">#</a></div>${(section.body || []).map((paragraph) => `<p>${escapeHtml(paragraph)}</p>`).join('')}${renderToyExamples(section.id)}${section.references?.length ? `<div class="docs-references"><strong>References</strong><ol>${section.references.map((reference) => `<li><a href="${escapeHtml(reference.href)}" target="_blank" rel="noreferrer">${escapeHtml(reference.label)}</a></li>`).join('')}</ol></div>` : `<div class="docs-references"><strong>References</strong><p>No external reference is required for this application-specific provenance behavior.</p></div>`}</article>`
    }).join('')}</div>`
}

function bindDocsEvents() {
  bindToyVisualControls(document)
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
    if (!table || table.classList.contains('metadata-editor-table') || table.classList.contains('significant-heatmap') || table.dataset.enhanced === 'true') return
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

    let dynamicRows = []
    let previousWindow = null
    const drawDynamicWindow = () => {
      if (!source) return
      const window = tableWindow(dynamicRows.length, scroll.scrollTop, scroll.clientHeight || 520, 40)
      if (previousWindow && previousWindow.first === window.first && previousWindow.end === window.end) return
      tbody.innerHTML = virtualSpacer(headers.length, window.topPadding)
        + dynamicRows.slice(window.first, window.end).map(source.rowHtml).join('')
        + virtualSpacer(headers.length, window.bottomPadding)
      status.textContent = `${tableRowRange(window)} matching · ${source.rows.length.toLocaleString()} total`
      previousWindow = window
    }
    if (source) scroll.addEventListener('scroll', drawDynamicWindow, { passive: true })
    const apply = () => {
      if (source) {
        dynamicRows = source.rows.filter((row) => !query || source.cells(row).some((value) => String(value ?? '').toLowerCase().includes(query)))
        if (sortIndex >= 0) dynamicRows.sort((a, b) => compareSortableValues(source.cells(a)[sortIndex], source.cells(b)[sortIndex]) * sortDirection)
        scroll.scrollTop = 0
        previousWindow = null
        drawDynamicWindow()
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

// Update the progress fields without rebuilding the whole Results page for each
// workbook. Rebuilding hundreds of plots during a long repertoire parse is costly.
function updateCompareProgress(completed, total, message) {
  state.compareProgress = { completed, total, message }
  const card = document.querySelector('.progress-card')
  if (!card) {
    if (state.tab === 'compare') render()
    return
  }
  const label = card.querySelector('.progress-copy strong')
  if (label) label.textContent = message
  const count = card.querySelector('.progress-copy span')
  if (count) count.textContent = `${completed} of ${total} modality steps complete`
  const bar = card.querySelector('progress')
  if (bar) { bar.max = Math.max(1, total); bar.value = completed }
}

async function runGroupComparison() {
  if (state.busy) return
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
  const shared = {
    metadataFile,
    comparisonName,
    config: portableConfig,
    omicsOptions: {
      ambiguousPolicy: cfg.ambiguousPolicy,
      determinedOnly: cfg.excludeUndeterminedIsotype,
      rarefactionReps: Number(cfg.repertoireRarefactionReps),
      rarefactionCap: Number(cfg.repertoireRarefactionCap),
      permutations: Number(cfg.repertoirePermutations),
    },
  }
  const steps = []
  if (cfg.geneEnabled && geneFile) steps.push({
    kind: 'gene', key: 'gene', title: 'Gene expression',
    gene: { enabled: true, file: geneFile },
  })
  if (cfg.transcriptEnabled && transcriptFile) steps.push({
    kind: 'transcript', key: 'transcript', title: 'Transcript expression',
    transcript: { enabled: true, file: transcriptFile },
  })
  for (const [kind, title, enabledKey, key] of otherOmics) {
    if (!cfg[enabledKey]) continue
    const files = recordsOfKind(kind)
      .slice().sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id))
      .map(record => ({ name: record.name, sampleId: record.sampleId, file: selectedFile(record.id) }))
      .filter(record => record.file)
    if (files.length) steps.push({ kind, key, title, otherOmics: [{ kind, key, title, files }] })
  }
  if (!steps.length) {
    state.error = 'Select at least one available data modality.'
    render()
    return
  }

  const controller = new AbortController()
  activeCompareController = controller
  state.busy = true
  state.error = null
  state.compareResult = null
  state.hitResult = null
  state.hitProgress = null
  const result = {
    comparisonName, gene: null, transcript: null, bcr: null, tcrAlpha: null,
    tcrBeta: null, isotype: null, immune: null, omicsErrors: {},
    completedAt: null, execution: 'local-browser-wasm',
  }
  updateCompareProgress(0, steps.length, 'Starting isolated modality analysis')
  render()
  let completed = 0
  try {
    for (const step of steps) {
      if (controller.signal.aborted) break
      updateCompareProgress(completed, steps.length, `${step.title}: initializing isolated analysis task`)
      try {
        const response = await isolatedScienceRequest({
          createWorker: () => new Worker(new URL(`./analysis-worker.js?v=${APP_VERSION}`, import.meta.url), { type: 'module' }),
          type: 'run-expression-comparison',
          resourceSettings: state.resources,
          signal: controller.signal,
          payload: {
            ...shared,
            gene: step.gene || { enabled: false, file: null },
            transcript: step.transcript || { enabled: false, file: null },
            otherOmics: step.otherOmics || [],
          },
          onProgress: progress => updateCompareProgress(completed, steps.length, progress.message || step.title),
          onMemory: data => {
            const report = validateWasmMemoryReport(data)
            if (report) {
              state.wasmMemorySample = { ...report, active: true }
              renderSystemInfo()
            }
          },
        })
        if (!response.result?.[step.key]) {
          const reason = response.result?.omicsErrors?.[step.kind] || 'No result returned for the selected modality'
          throw new Error(reason)
        }
        result[step.key] = response.result[step.key]
      } catch (error) {
        if (controller.signal.aborted) break
        const message = error instanceof Error ? error.message : String(error)
        result.omicsErrors[step.kind] = message
        console.error(`4C Insight ${step.title} failed:`, message)
      } finally {
        completed += 1
        if (state.wasmMemorySample) state.wasmMemorySample = { ...state.wasmMemorySample, active: false }
        state.compareResult = result
        updateCompareProgress(completed, steps.length, `${step.title}: ${result[step.key] ? 'complete' : 'failed or stopped'}`)
        render()
      }
    }
    result.completedAt = new Date().toISOString()
    state.compareResult = result
    const succeeded = steps.filter(step => result[step.key]).length
    if (controller.signal.aborted) {
      state.error = `Analysis stopped. ${succeeded} of ${steps.length} modalities completed; existing results remain available.`
    } else if (!succeeded) {
      state.error = 'Every selected modality failed. Review the named errors below and the browser console.'
    }
    if (succeeded) {
      state.resultsMode = 'overview'
      state.tab = 'results'
      try {
        await recordHistory(steps.some(step => step.otherOmics?.length) ? 'compare-groups-multiomics' : 'compare-groups-expression', comparisonName || 'Compare Groups')
      } catch (error) {
        // A History storage error must not discard completed scientific results.
        state.error = `Results completed but could not be saved to History: ${error instanceof Error ? error.message : String(error)}`
      }
    }
  } catch (error) {
    // Retain any completed modalities even after an unexpected browser error.
    state.compareResult = result
    state.error = `Comparison stopped unexpectedly: ${error instanceof Error ? error.message : String(error)}`
  } finally {
    activeCompareController = null
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
  for (const [kind, , enabledKey] of otherOmics) {
    document.querySelector(`#compare-${kind}-enabled`)?.addEventListener('change', (event) => {
      cfg[enabledKey] = event.currentTarget.checked
      state.compareResult = null
      rerender()
    })
  }
  for (const [id, field, asNumber] of [
    ['compare-repertoire-policy', 'ambiguousPolicy', false],
    ['compare-repertoire-reps', 'repertoireRarefactionReps', true],
    ['compare-repertoire-cap', 'repertoireRarefactionCap', true],
    ['compare-repertoire-permutations', 'repertoirePermutations', true],
  ]) {
    document.querySelector(`#${id}`)?.addEventListener('change', (event) => {
      cfg[field] = asNumber ? Number(event.currentTarget.value) : event.currentTarget.value
      state.compareResult = null
    })
  }
  document.querySelector('#compare-isotype-determined')?.addEventListener('change', (event) => {
    cfg.excludeUndeterminedIsotype = event.currentTarget.checked
    state.compareResult = null
  })
  document.querySelector('#run-group-comparison')?.addEventListener('click', () => void runGroupComparison())
  document.querySelector('#export-comparison')?.addEventListener('click', () => void exportCompareResult())
  bindGlobalThresholdEvents()
  bindInteractivePlots()
  attachStaticSvgNavigation(document)

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

function metadataReplaceDraft(table, sourceName = '', sourceId = '') {
  if (!Array.isArray(table?.columns) || !Array.isArray(table?.rows) || !table.columns.length) {
    throw new Error('The metadata table has no columns')
  }
  if (new Set(table.columns).size !== table.columns.length || table.columns.some(name => !name)) {
    throw new Error('Metadata column names must be unique and nonempty')
  }
  const editor = state.metadataEditor
  editor.table = { columns: [...table.columns], rows: table.rows.map(row => table.columns.map((_, i) => String(row[i] ?? ''))) }
  editor.sourceName = sourceName
  editor.sourceId = sourceId
  editor.idColumn = table.columns.includes(table.id_column) ? table.id_column
    : table.columns.includes('sample_id') ? 'sample_id'
      : table.columns.find(name => /^(4c_id|sample.?id|observation.?id|subject.?id)$/i.test(name)) || table.columns[0]
  editor.mappings = autoMapFiles(state.records, editor.table, editor.idColumn, {})
  editor.metadataScrollTop = 0
  editor.mappingScrollTop = 0
  editor.filter = ''
  editor.filterColumn = ''
  editor.filterValue = ''
  editor.mapFilter = ''
  editor.mapUnmatched = false
  editor.sortColumn = ''
  editor.descending = false
  editor.dirty = Boolean(!sourceId)
  editor.status = sourceName ? `Opened ${sourceName}: ${editor.table.rows.length.toLocaleString()} rows. Edits are local until you download the TSV.`
    : `Created ${editor.table.rows.length.toLocaleString()} sample rows from local files.`
  render()
}

function metadataConfirmReplace() {
  return !state.metadataEditor.table || !state.metadataEditor.dirty || window.confirm('Replace the unsaved metadata draft? Download your edited TSV first if you need it.')
}

async function metadataOpenSelected(fileId) {
  const file = selectedFile(fileId)
  if (!file || !metadataConfirmReplace()) return
  state.busy = true
  state.error = null
  render()
  try {
    const response = await workerRequest('load-metadata-table', { file })
    metadataReplaceDraft(response.table, file.name, fileId)
  } catch (error) {
    state.error = `Open metadata: ${error instanceof Error ? error.message : String(error)}`
  } finally {
    state.busy = false
    render()
  }
}

function metadataEdit(nextTable, status = 'Metadata draft updated. Download the TSV to save your changes.') {
  state.metadataEditor.table = nextTable
  state.metadataEditor.dirty = true
  state.metadataEditor.status = status
  // Preserve existing exact file assignments when a non-ID field changes.
  state.metadataEditor.mappings = autoMapFiles(state.records, nextTable, state.metadataEditor.idColumn, state.metadataEditor.mappings)
  render()
}

function metadataDownload(filename, text, mime = 'text/tab-separated-values') {
  const url = URL.createObjectURL(new Blob([text], { type: `${mime};charset=utf-8` }))
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.append(link)
  link.click()
  link.remove()
  setTimeout(() => URL.revokeObjectURL(url), 60000)
}

function metadataExportTsv() {
  const editor = state.metadataEditor
  if (!editor.table) return
  const check = validateMetadata(editor.table, editor.idColumn)
  if (check.errors.length || check.blankIds.length || check.duplicates.length) {
    state.error = 'Correct blank or duplicate sample IDs before exporting the metadata TSV.'
    render()
    return
  }
  metadataDownload('4c_sample_metadata.tsv', serializeDelimited(editor.table))
  editor.status = 'Metadata TSV download requested. The file can also be added to Project to use in analyses.'
  editor.dirty = false
  render()
}

function metadataExportMap() {
  const editor = state.metadataEditor
  if (!editor.table) return
  metadataDownload('4c_sample_file_mapping.tsv', serializeDelimited(mappingManifest(state.records, editor.mappings)))
  editor.status = 'File-mapping TSV download requested. Unmapped files have blank sample IDs.'
  render()
}

async function metadataUseInAnalyses() {
  const editor = state.metadataEditor
  if (!editor.table || state.busy) return
  const check = validateMetadata(editor.table, editor.idColumn)
  if (check.errors.length || check.blankIds.length || check.duplicates.length) {
    state.error = 'Correct sample IDs before you use this sheet for analysis.'
    render()
    return
  }
  try {
    const file = new File([serializeDelimited(editor.table)], '4c_sample_metadata.tsv', {
      type: 'text/tab-separated-values', lastModified: Date.now(),
    })
    const updated = new Map(state.files)
    if (editor.generatedId) updated.delete(editor.generatedId)
    updated.set(fileKey(file), file)
    await refreshFiles(updated, true)
    if (state.error) throw new Error(state.error)
    editor.generatedId = fileKey(file)
    state.qcConfig.metadataId = editor.generatedId
    state.qcConfig.metadataIdColumn = editor.idColumn
    editor.sourceName = file.name
    editor.sourceId = editor.generatedId
    editor.status = 'Edited metadata is selected for QC, Compare Groups, and Longitudinal in this browser session. Download the TSV to keep a copy.'
    // Do not mark saved: an in-memory File does not survive a browser restart.
    editor.dirty = true
    await inspectSelectedMetadata()
    state.tab = 'metadata'
    render()
  } catch (error) {
    state.error = `Use metadata: ${error instanceof Error ? error.message : String(error)}`
    render()
  }
}

function bindMetadataEvents() {
  const editor = state.metadataEditor
  const table = editor.table
  const displayError = error => { state.error = error instanceof Error ? error.message : String(error); render() }
  document.querySelector('#metadata-load')?.addEventListener('click', () => void metadataOpenSelected(document.querySelector('#metadata-editor-source')?.value))
  document.querySelector('#metadata-create')?.addEventListener('click', () => {
    if (metadataConfirmReplace()) metadataReplaceDraft(draftFromSamples(state.records))
  })
  document.querySelector('#metadata-blank')?.addEventListener('click', () => {
    if (metadataConfirmReplace()) metadataReplaceDraft(createBlankMetadata())
  })
  if (!table) return
  const rerender = () => { state.error = null; render() }
  document.querySelector('#metadata-search')?.addEventListener('input', event => {
    editor.filter = event.currentTarget.value
    editor.metadataScrollTop = 0
    renderMetadataTableOnly()
  })
  document.querySelector('#metadata-filter-column')?.addEventListener('change', event => {
    editor.filterColumn = event.currentTarget.value; editor.filterValue = ''; editor.metadataScrollTop = 0; rerender()
  })
  document.querySelector('#metadata-filter-value')?.addEventListener('change', event => {
    editor.filterValue = event.currentTarget.value; editor.metadataScrollTop = 0; renderMetadataTableOnly()
  })
  document.querySelector('#metadata-add-row')?.addEventListener('click', () => metadataEdit(addRow(editor.table), 'Added a blank sample. Enter its sample ID.'))
  document.querySelector('#metadata-download')?.addEventListener('click', metadataExportTsv)
  document.querySelector('#metadata-use')?.addEventListener('click', () => void metadataUseInAnalyses())
  document.querySelector('#metadata-add-column')?.addEventListener('click', () => {
    try {
      const name = document.querySelector('#metadata-new-column')?.value.trim() || document.querySelector('#metadata-common-column')?.value
      metadataEdit(addColumn(editor.table, name), `Added column: ${name}`)
    } catch (error) { displayError(error) }
  })
  document.querySelector('#metadata-rename-column')?.addEventListener('click', () => {
    try {
      const name = document.querySelector('#metadata-manage-column')?.value
      const index = table.columns.indexOf(name)
      const replacement = document.querySelector('#metadata-rename-input')?.value
      const renamed = renameColumn(editor.table, index, replacement)
      if (editor.idColumn === name) editor.idColumn = replacement.trim()
      metadataEdit(renamed, `Renamed ${name} to ${replacement}`)
    } catch (error) { displayError(error) }
  })
  document.querySelector('#metadata-remove-column')?.addEventListener('click', () => {
    try {
      const name = document.querySelector('#metadata-manage-column')?.value
      if (name === editor.idColumn) throw new Error('Select a different sample ID column before removing this one')
      if (!window.confirm(`Remove column '${name}' and all of its values?`)) return
      metadataEdit(removeColumn(editor.table, table.columns.indexOf(name)), `Removed column ${name}`)
    } catch (error) { displayError(error) }
  })
  document.querySelector('#metadata-bulk-apply')?.addEventListener('click', () => {
    const name = document.querySelector('#metadata-bulk-column')?.value
    const index = table.columns.indexOf(name)
    const value = document.querySelector('#metadata-bulk-value')?.value ?? ''
    const matches = metadataFilteredRows()
    if (index < 0 || !matches.length) return
    if (!window.confirm(`Replace '${name}' for ${matches.length} filtered sample rows?`)) return
    const indices = new Set(matches.map(match => match.index))
    const updated = { columns: [...editor.table.columns], rows: editor.table.rows.map((row, i) => indices.has(i) ? row.map((cell, j) => j === index ? value : cell) : row) }
    metadataEdit(updated, `Updated ${matches.length} rows in ${name}`)
  })
  document.querySelector('#metadata-id-column-editor')?.addEventListener('change', event => {
    editor.idColumn = event.currentTarget.value
    editor.mappings = autoMapFiles(state.records, table, editor.idColumn, editor.mappings)
    rerender()
  })
  document.querySelector('#mapping-auto')?.addEventListener('click', () => {
    editor.mappings = autoMapFiles(state.records, table, editor.idColumn, {})
    editor.status = 'Matched file names to sample IDs where an exact ID exists. Review unmatched files.'
    rerender()
  })
  document.querySelector('#mapping-aliases')?.addEventListener('click', () => {
    try {
      const result = applyMappingsToAliases(editor.table, state.records, editor.mappings, editor.idColumn)
      metadataEdit(result.table, `Added ${result.aliasesWritten} mapped source IDs to modality alias columns. Download the edited metadata TSV to keep them.`)
    } catch (error) { displayError(error) }
  })
  document.querySelector('#mapping-download')?.addEventListener('click', metadataExportMap)
  document.querySelector('#mapping-search')?.addEventListener('input', event => { editor.mapFilter = event.currentTarget.value; editor.mappingScrollTop = 0; renderMetadataMappingsOnly() })
  document.querySelector('#mapping-unmatched')?.addEventListener('change', event => { editor.mapUnmatched = event.currentTarget.checked; editor.mappingScrollTop = 0; renderMetadataMappingsOnly() })
  document.querySelector('#metadata-return-project')?.addEventListener('click', () => { state.tab = 'project'; render() })
  bindMetadataGridEvents()
  bindMetadataMappingEvents()
}

function refreshMetadataValidationDisplay() {
  const editor = state.metadataEditor
  if (!editor.table) return
  const health = validateMetadata(editor.table, editor.idColumn)
  const blocked = Boolean(health.errors.length || health.blankIds.length || health.duplicates.length)
  const validation = document.querySelector('#metadata-check-issues')
  if (validation) validation.textContent = `${health.blankIds.length} blank sample IDs · ${health.duplicates.length} duplicate sample IDs${health.errors.length ? ` · ${health.errors.join('; ')}` : ''}`
  const download = document.querySelector('#metadata-download')
  if (download) download.disabled = blocked
  const use = document.querySelector('#metadata-use')
  if (use) use.disabled = blocked || state.busy
}
function renderMetadataTableOnly() {
  const holder = document.querySelector('#metadata-row-holder')
  if (!holder) return
  holder.innerHTML = metadataTableRows()
  bindMetadataGridEvents()
}
function bindMetadataGridEvents() {
  const holder = document.querySelector('#metadata-row-holder')
  if (!holder) return
  const editor = state.metadataEditor
  if (!holder.dataset.eventsBound) {
    holder.dataset.eventsBound = 'true'
    // Delegation survives virtual-row replacement on scroll.
    holder.addEventListener('change', event => {
      const input = event.target.closest('.metadata-cell')
      if (!input) return
      const { row, col } = input.dataset
      const index = Number(row), column = Number(col)
      if (editor.table.rows[index]?.[column] !== input.value) {
        editor.table = setCell(editor.table, index, column, input.value)
        editor.dirty = true
        editor.status = 'Unsaved changes: download the metadata TSV to keep a copy.'
        if (editor.table.columns[column] === editor.idColumn) {
          editor.mappings = autoMapFiles(state.records, editor.table, editor.idColumn, editor.mappings)
        }
        const badge = document.querySelector('.metadata-summary-grid')
        if (badge) badge.dataset.edited = 'true'
        refreshMetadataValidationDisplay()
      }
    })
    holder.addEventListener('click', event => {
      const remove = event.target.closest('.metadata-remove-row')
      if (remove) {
        const index = Number(remove.dataset.row)
        if (window.confirm(`Remove metadata row ${index + 1}?`)) metadataEdit(removeRow(editor.table, index), 'Removed sample row')
        return
      }
      const sort = event.target.closest('.metadata-sort')
      if (sort) {
        const col = sort.dataset.column
        editor.descending = editor.sortColumn === col ? !editor.descending : false
        editor.sortColumn = col
        editor.metadataScrollTop = 0
        renderMetadataTableOnly()
      }
    })
  }
  initializeMetadataViewport(holder, 'metadata')
}
function renderMetadataMappingsOnly() {
  const holder = document.querySelector('#metadata-mapping-holder')
  if (!holder) return
  holder.innerHTML = metadataMappingRows()
  bindMetadataMappingEvents()
}
function bindMetadataMappingEvents() {
  const holder = document.querySelector('#metadata-mapping-holder')
  if (!holder) return
  const editor = state.metadataEditor
  if (!holder.dataset.eventsBound) {
    holder.dataset.eventsBound = 'true'
    holder.addEventListener('change', event => {
      const input = event.target.closest('.mapping-sample')
      if (!input) return
      const id = input.dataset.fileId
      const value = input.value.trim()
      const validIds = new Set(editor.table.rows.map(row => String(row[editor.table.columns.indexOf(editor.idColumn)] || '').trim()).filter(Boolean))
      if (value && !validIds.has(value)) {
        input.setCustomValidity('Choose an existing sample ID')
        input.reportValidity()
        return
      }
      input.setCustomValidity('')
      editor.mappings[id] = value
      editor.status = 'Updated local file-to-sample assignments. Download the mapping manifest to keep them.'
      const cell = input.closest('tr')?.querySelector('td:last-child .tag')
      if (cell) { cell.textContent = value ? 'Matched' : 'Unmapped'; cell.className = `tag ${value ? 'success' : 'warning'}` }
    })
  }
  initializeMetadataViewport(holder, 'mapping')
}

function bindGlobalStateActions() {
  // The header remains mounted when the user moves between workflows.
  // Attach one set of handlers for the lifetime of the page.
  const stateInput = document.querySelector('#state-input')
  const status = document.querySelector('#global-state-status')
  document.querySelector('#load-state')?.addEventListener('click', () => stateInput?.click())
  document.querySelector('#save-state')?.addEventListener('click', () => {
    void saveApplicationState()
      .then(() => { if (status) status.textContent = 'Project state export requested.' })
      .catch((error) => {
        state.error = `Save state: ${error instanceof Error ? error.message : String(error)}`
        if (status) status.textContent = 'Could not save project state.'
        render()
      })
  })
  document.querySelector('#clear-state')?.addEventListener('click', () => void clearSavedActiveSession())
  stateInput?.addEventListener('change', (event) => {
    const file = event.currentTarget.files?.[0]
    event.currentTarget.value = '' // Allow selecting the same state file again.
    if (!file) return
    void loadApplicationState(file).then(() => {
      if (status) status.textContent = state.error ? 'Could not load project state.' : 'Project state loaded.'
    })
  })
}

function bindProjectEvents() {
  const fileInput = document.querySelector('#file-input')
  const folderInput = document.querySelector('#folder-input')
  const dropZone = document.querySelector('#input-drop-zone')
  document.querySelector('#select-files')?.addEventListener('click', () => fileInput.click())
  document.querySelector('#select-folder')?.addEventListener('click', () => folderInput.click())
  document.querySelector('#reconnect-state-folder')?.addEventListener('click', () => folderInput.click())
  fileInput?.addEventListener('change', (event) => addFiles(event.currentTarget.files))
  folderInput?.addEventListener('change', (event) => addFiles(event.currentTarget.files))
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
  document.querySelector('#open-sample-metadata')?.addEventListener('click', () => { state.tab = 'metadata'; render() })
  document.querySelector('#save-manifest')?.addEventListener('click', () => void exportManifest())
  document.querySelector('#clear-project')?.addEventListener('click', clearProject)
  document.querySelector('#merge-column-order')?.addEventListener('change', event => { state.mergeSettings.order = event.currentTarget.value })
  document.querySelectorAll('.merge-metric').forEach(input => input.addEventListener('change', event => {
    const { kind } = event.currentTarget.dataset
    const selected = new Set(state.mergeSettings[kind])
    if (event.currentTarget.checked) selected.add(event.currentTarget.value)
    else selected.delete(event.currentTarget.value)
    state.mergeSettings[kind] = mergeMetrics[kind].map(([value]) => value).filter(value => selected.has(value))
  }))
  document.querySelectorAll('.run-merge').forEach(button => button.addEventListener('click', () => void runExpressionMerge(button.dataset.kind)))
  document.querySelectorAll('.download-merge').forEach(button => button.addEventListener('click', () => void downloadMergedTable(button.dataset.kind)))
  document.querySelector('#restore-merged')?.addEventListener('click', () => void restoreMergedTables())
  document.querySelector('#stop-merge')?.addEventListener('click', stopScienceWork)
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
    metadata: metadataView,
    qc: qcView,
    compare: compareView,
    results: resultsView,
    longitudinal: longitudinalView,
    hits: significantHitsView,
    ai: aiAssistantView,
    history: historyView,
    docs: docsView,
    settings: settingsView,
  }
  main.innerHTML = (viewFactories[state.tab] || projectView)()
  if (state.tab === 'project') bindProjectEvents()
  if (state.tab === 'metadata') bindMetadataEvents()
  if (state.tab === 'qc') bindQcEvents()
  if (state.tab === 'compare') bindCompareEvents()
  if (state.tab === 'results') bindResultsEvents()
  if (state.tab === 'longitudinal') bindLongitudinalEvents()
  if (state.tab === 'hits') { bindSignificantHitEvents(); void ensureIntegrativeMetadataColumns() }
  if (state.tab === 'ai') mountAiAssistant(main,{summary:{
    integrative: Boolean(state.i2StructureResult),
    compare: Boolean(state.compareResult),
    pathways: Boolean(state.pathwayResult),
    pathwayCount: state.pathwayResult?.results?.length || 0,
    pathwayEvidence: state.pathwayResult?.source === 'exploratory' ? 'exploratory' : 'unverified',
  }})
  document.querySelectorAll('[data-open-results]').forEach(button => button.addEventListener('click', () => { state.resultsMode = 'overview'; state.tab = 'results'; render() }))
  if (state.tab === 'history') {
    bindHistoryEvents()
    if (!state.historyLoaded && !state.historyBusy) void refreshHistory()
  }
  if (state.tab === 'docs') bindDocsEvents()
  if (state.tab === 'settings') bindSettingsEvents()
  enhanceTables()
  attachStaticSvgNavigation(main)
  attachTableHeatmapNavigation(main)
  renderSystemInfo()
  scheduleLocalSessionSave()
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
bindGlobalStateActions()
render()
void restoreActiveLocalSession()
// One automatic sample shortly after startup and a low-frequency refresh while visible.
// Unavailable APIs return immediately; repeated checks cannot reveal physical RAM.
setTimeout(() => { void refreshAutomaticAppMemory() }, 2500)
setInterval(() => { void refreshAutomaticAppMemory() }, 120000)
