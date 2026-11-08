import { classifyInputFilename } from './file-classifier.js'
import { validateLocalInputFile } from './resource-controls.js'
import { measureWasmAllocation } from './wasm-memory.js'
import { runPortableMerge } from './merge-worker.js'
import { analyzeJointStructure } from './joint-structure.js?v=0.22.3'
import { runPatientStabilityAsync } from './joint-stability.js?v=0.22.3'

let wasmModulePromise = null
let wasmMemory = null
let wasmPeakAllocated = 0
let lastWasmMemorySent = 0
let lastWasmMemoryAt = 0

function postWasmMemory(requestId, force = false) {
  const sample = measureWasmAllocation(wasmMemory, wasmPeakAllocated)
  if (!sample) return
  wasmPeakAllocated = sample.peakAllocatedBytes
  const now = performance.now()
  if (!force && sample.allocatedBytes === lastWasmMemorySent && now - lastWasmMemoryAt < 3000) return
  lastWasmMemorySent = sample.allocatedBytes
  lastWasmMemoryAt = now
  self.postMessage({ type: 'wasm-memory', requestId, ...sample })
}

async function loadWasm() {
  if (!wasmModulePromise) {
    wasmModulePromise = (async () => {
      try {
        const moduleUrl = new URL('./wasm/four_c_insight_wasm.js', self.location.href).href
        const module = await import(moduleUrl)
        const exports = await module.default()
        // wasm-bindgen returns the instance exports, including its Memory.
        wasmMemory = exports?.memory instanceof WebAssembly.Memory ? exports.memory : null
        return module
      } catch {
        return null
      }
    })()
  }
  return wasmModulePromise
}

function postError(requestId, error) {
  self.postMessage({
    type: 'worker-error',
    requestId,
    message: error instanceof Error ? error.message : String(error),
  })
}

function postProgress(requestId, completed, total, message) {
  self.postMessage({
    type: 'analysis-progress',
    requestId,
    completed,
    total,
    message,
  })
  postWasmMemory(requestId)
}

async function wasmBytes(file) {
  validateLocalInputFile(file)
  const buffer = await file.arrayBuffer()
  return new Uint8Array(buffer)
}

async function handlePreflight(request) {
  const wasm = await loadWasm()
  const records = request.files.map((file) => {
    const classification = wasm
      ? JSON.parse(wasm.classify_filename_json(file.name))
      : classifyInputFilename(file.name)
    return {
      ...file,
      ...classification,
      status: classification.recognized ? 'ready' : 'unrecognized',
    }
  })

  let engineVersion = null
  if (wasm) {
    try {
      engineVersion = JSON.parse(wasm.engine_info_json()).version ?? null
    } catch {
      engineVersion = null
    }
  }

  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'preflight-result',
    requestId: request.requestId,
    records,
    engine: wasm ? 'Rust/WASM' : 'JavaScript preflight fallback',
    wasmReady: Boolean(wasm),
    engineVersion,
  })
}

async function handleMetadataInspection(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Analysis tools are unavailable. Refresh the page and try again.')
  const bytes = await wasmBytes(request.file)
  const result = JSON.parse(
    wasm.inspect_metadata_json(request.file.name, bytes, request.idColumn || ''),
  )
  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'metadata-inspection-result',
    requestId: request.requestId,
    result,
  })
}

async function handleMetadataTable(request) {
  const wasm = await loadWasm()
  if (!wasm?.read_metadata_table_json) {
    throw new Error('Rebuild the 4C Insight WASM module to enable the full metadata editor')
  }
  const bytes = await wasmBytes(request.file)
  const table = JSON.parse(wasm.read_metadata_table_json(request.file.name, bytes))
  postWasmMemory(request.requestId, true)
  self.postMessage({ type: 'metadata-table-result', requestId: request.requestId, table })
}

async function handleExpressionQc(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Analysis tools are unavailable. Refresh the page and try again.')

  const geneFiles = request.gene?.enabled ? request.gene.files || [] : []
  const transcriptFiles = request.transcript?.enabled ? request.transcript.files || [] : []
  const total = (request.metadataFile ? 1 : 0) + geneFiles.length + transcriptFiles.length
  let completed = 0
  let metadata = null
  const gene = []
  const transcript = []

  if (request.metadataFile) {
    postProgress(request.requestId, completed, total, `Reading metadata: ${request.metadataFile.name}`)
    const bytes = await wasmBytes(request.metadataFile)
    metadata = JSON.parse(
      wasm.inspect_metadata_json(
        request.metadataFile.name,
        bytes,
        request.metadataIdColumn || '',
      ),
    )
    completed += 1
    postProgress(request.requestId, completed, total, `Metadata ready: ${metadata.rows} rows`)
  }

  for (const file of geneFiles) {
    postProgress(request.requestId, completed, total, `Gene expression: ${file.name}`)
    const bytes = await wasmBytes(file)
    gene.push(JSON.parse(
      wasm.summarize_expression_json(
        file.name,
        bytes,
        'gene-expression',
        request.gene.metric,
        Boolean(request.ignoreZeros),
      ),
    ))
    completed += 1
    postProgress(request.requestId, completed, total, `Gene expression ${completed}/${total}`)
  }

  for (const file of transcriptFiles) {
    postProgress(request.requestId, completed, total, `Transcript expression: ${file.name}`)
    const bytes = await wasmBytes(file)
    transcript.push(JSON.parse(
      wasm.summarize_expression_json(
        file.name,
        bytes,
        'transcript-expression',
        request.transcript.metric,
        Boolean(request.ignoreZeros),
      ),
    ))
    completed += 1
    postProgress(request.requestId, completed, total, `Transcript expression ${completed}/${total}`)
  }

  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'expression-qc-result',
    requestId: request.requestId,
    result: {
      metadata,
      gene,
      transcript,
      ignoreZeros: Boolean(request.ignoreZeros),
      completedAt: new Date().toISOString(),
      execution: 'local-browser-wasm',
    },
  })
}


async function handleMetadataColumnValues(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Analysis tools are unavailable. Refresh the page and try again.')
  if (!request.file || !request.column) throw new Error('Metadata file and column are required')
  const bytes = await wasmBytes(request.file)
  const result = JSON.parse(
    wasm.metadata_column_values_json(request.file.name, bytes, request.column),
  )
  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'metadata-column-values-result',
    requestId: request.requestId,
    result,
  })
}

async function handleExpressionComparison(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Analysis tools are unavailable. Refresh the page and try again.')
  if (!request.metadataFile) throw new Error('Select a metadata file before running a group comparison')

  const analyses = [
    ['gene', request.gene],
    ['transcript', request.transcript],
  ].filter(([, entry]) => entry?.enabled && entry?.file)
  const otherOmics = (request.otherOmics || []).filter(entry => entry.files?.length)
  if (!analyses.length && !otherOmics.length) throw new Error('Select at least one available omics modality')

  const total = analyses.length + otherOmics.length
  let completed = 0
  postProgress(request.requestId, completed, total, `Reading metadata: ${request.metadataFile.name}`)
  const metadataBytes = await wasmBytes(request.metadataFile)
  const result = {
    comparisonName: request.comparisonName || 'Metadata-defined comparison',
    gene: null,
    transcript: null,
    bcr: null, tcrAlpha: null, tcrBeta: null, isotype: null, immune: null, omicsErrors: {},
    completedAt: null,
    execution: 'local-browser-wasm',
  }

  for (const [kind, entry] of analyses) {
    const label = kind === 'gene' ? 'Gene expression' : 'Transcript expression'
    postProgress(request.requestId, completed, total, `${label}: reading ${entry.file.name}`)
    const mergedBytes = await wasmBytes(entry.file)
    const config = {
      ...request.config,
      kind: kind === 'gene' ? 'gene-expression' : 'transcript-expression',
    }
    result[kind] = JSON.parse(
      wasm.run_expression_comparison_json(
        request.metadataFile.name,
        metadataBytes,
        entry.file.name,
        mergedBytes,
        JSON.stringify(config),
      ),
    )
    completed += 1
    postProgress(request.requestId, completed, total, `${label}: comparison complete`)
  }

  for (const entry of otherOmics) {
    if (!wasm.multiomics_begin_json || !wasm.multiomics_add_sample_json || !wasm.multiomics_finish_json || !wasm.multiomics_abort || !wasm.multiomics_sample_role_json) {
      throw new Error('This analysis is unavailable in this application version. Refresh the page and try again.')
    }
    postProgress(request.requestId, completed, total, `${entry.title}: starting ${entry.files.length} local files`)
    let begun = false
    try {
      wasm.multiomics_begin_json(
        request.metadataFile.name, metadataBytes, JSON.stringify({ ...request.config, kind: 'gene-expression' }),
        entry.kind, JSON.stringify(request.omicsOptions || {}),
      )
      begun = true
      const records = entry.files.slice().sort((a, b) => a.sampleId.localeCompare(b.sampleId) || a.name.localeCompare(b.name))
      for (let i = 0; i < records.length; i++) {
        const record = records[i]
        postProgress(request.requestId, completed, total, `${entry.title}: parsing ${i+1}/${records.length}: ${record.name}`)
        // Skip source byte reads when a sample is outside the selected cohorts.
        const role = JSON.parse(wasm.multiomics_sample_role_json(record.sampleId)).role
        if (role === 'outside') {
          wasm.multiomics_add_sample_json(record.name, record.sampleId, new Uint8Array(0))
          continue
        }
        const bytes = await wasmBytes(record.file)
        // Rust parses one workbook and retains only the scientific representation.
        wasm.multiomics_add_sample_json(record.name, record.sampleId, bytes)
        postWasmMemory(request.requestId)
      }
      postProgress(request.requestId, completed, total, `${entry.title}: corrected statistics, ordination and clustering`)
      result[entry.key] = JSON.parse(wasm.multiomics_finish_json())
      begun = false
    } catch (error) {
      result.omicsErrors[entry.kind] = error instanceof Error ? error.message : String(error)
    } finally {
      if (begun) wasm.multiomics_abort()
      completed += 1
      postProgress(request.requestId, completed, total, `${entry.title}: ${result[entry.key] ? 'complete' : 'failed (see warnings)'}`)
    }
  }

  result.completedAt = new Date().toISOString()
  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'expression-comparison-result',
    requestId: request.requestId,
    result,
  })
}


async function handleLongitudinalExpression(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Analysis tools are unavailable. Refresh the page and try again.')
  if (!request.metadataFile) throw new Error('Select a metadata file before running longitudinal analysis')

  const analyses = [
    ['gene', request.gene],
    ['transcript', request.transcript],
  ].filter(([, entry]) => entry?.enabled && entry?.file)
  if (!analyses.length) throw new Error('Enable gene and/or transcript expression and select a merged table')

  const total = analyses.length
  let completed = 0
  postProgress(request.requestId, completed, total, `Reading metadata: ${request.metadataFile.name}`)
  const metadataBytes = await wasmBytes(request.metadataFile)
  const result = {
    gene: null,
    transcript: null,
    completedAt: null,
    execution: 'local-browser-wasm',
  }

  for (const [kind, entry] of analyses) {
    const label = kind === 'gene' ? 'Gene expression' : 'Transcript expression'
    postProgress(request.requestId, completed, total, `${label}: reading ${entry.file.name}`)
    const mergedBytes = await wasmBytes(entry.file)
    const config = {
      ...request.config,
      kind: kind === 'gene' ? 'gene-expression' : 'transcript-expression',
    }
    result[kind] = JSON.parse(
      wasm.run_expression_longitudinal_json(
        request.metadataFile.name,
        metadataBytes,
        entry.file.name,
        mergedBytes,
        JSON.stringify(config),
      ),
    )
    completed += 1
    postProgress(request.requestId, completed, total, `${label}: longitudinal signature complete`)
  }

  result.completedAt = new Date().toISOString()
  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'longitudinal-expression-result',
    requestId: request.requestId,
    result,
  })
}


async function handleSignificantHits(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Analysis tools are unavailable. Refresh the page and try again.')
  if (!request.metadataFile) throw new Error('Select a metadata file before running significant-hit analysis')

  const analyses = [
    ['gene', request.gene],
    ['transcript', request.transcript],
  ].filter(([, entry]) => entry?.enabled && entry?.file)
  if (!analyses.length) throw new Error('Enable gene and/or transcript expression and select a merged table')

  const total = analyses.length
  let completed = 0
  postProgress(request.requestId, completed, total, `Reading metadata: ${request.metadataFile.name}`)
  const metadataBytes = await wasmBytes(request.metadataFile)
  const result = {
    comparisonName: request.comparisonName || 'Metadata-defined comparison',
    gene: null,
    transcript: null,
    completedAt: null,
    execution: 'local-browser-wasm',
  }

  for (const [kind, entry] of analyses) {
    const label = kind === 'gene' ? 'Gene expression' : 'Transcript expression'
    postProgress(request.requestId, completed, total, `${label}: re-running corrected comparison and selecting hits`)
    const mergedBytes = await wasmBytes(entry.file)
    const comparisonConfig = {
      ...request.comparisonConfig,
      kind: kind === 'gene' ? 'gene-expression' : 'transcript-expression',
    }
    result[kind] = JSON.parse(
      wasm.run_expression_hit_structure_json(
        request.metadataFile.name,
        metadataBytes,
        entry.file.name,
        mergedBytes,
        JSON.stringify(comparisonConfig),
        JSON.stringify(request.hitConfig),
      ),
    )
    completed += 1
    postProgress(request.requestId, completed, total, `${label}: significant-hit structure complete`)
  }

  result.completedAt = new Date().toISOString()
  postWasmMemory(request.requestId, true)
  self.postMessage({
    type: 'significant-hit-result',
    requestId: request.requestId,
    result,
  })
}

self.addEventListener('message', async (event) => {
  const request = event.data
  if (!request?.type || !request.requestId) return

  try {
    if (request.type === 'preflight') {
      await handlePreflight(request)
      return
    }
    if (request.type === 'load-metadata-table') {
      await handleMetadataTable(request)
      return
    }
    if (request.type === 'inspect-metadata') {
      await handleMetadataInspection(request)
      return
    }
    if (request.type === 'run-merge-expression') {
      const wasm = await loadWasm()
      if (!wasm?.merge_begin_json || !wasm?.merge_sample_packed) throw new Error('Rebuild the M1 Rust/WASM merger before merging XLSX workbooks')
      const { file, summary } = await runPortableMerge({
        wasm, request,
        postProgress: (completed, total, message) => postProgress(request.requestId, completed, total, message),
        postWasmMemory: force => postWasmMemory(request.requestId, force),
      })
      postWasmMemory(request.requestId, true)
      self.postMessage({ type: 'merge-expression-result', requestId: request.requestId, file, summary })
      return
    }
    if (request.type === 'run-expression-qc') {
      await handleExpressionQc(request)
      return
    }
    if (request.type === 'metadata-column-values') {
      await handleMetadataColumnValues(request)
      return
    }
    if (request.type === 'run-expression-comparison') {
      await handleExpressionComparison(request)
      return
    }
    if (request.type === 'run-longitudinal-expression') {
      await handleLongitudinalExpression(request)
      return
    }
    if (request.type === 'run-i2b-source-distance') {
      const wasm = await loadWasm()
      if (!wasm?.i2b_begin_json || !wasm?.i2b_add_sample_json || !wasm?.i2b_finish_json ||
          !wasm?.i2b_abort || !wasm?.i2b_expression_distance_json) {
        throw new Error('Rebuild the browser WASM source-distance engine before running direct integration')
      }
      const kind = request.modality
      const files = request.files || []
      if (!files.length) throw new Error(`${kind}: no exactly aligned source files`)
      let view
      if (kind === 'gene' || kind === 'transcript') {
        if (files.length !== 1 || !Array.isArray(request.sampleIds)) throw new Error('Select one merged expression table')
        postProgress(request.requestId, 0, 1, `${kind}: reading local merged expression table`)
        const bytes = await wasmBytes(files[0].file)
        view = JSON.parse(wasm.i2b_expression_distance_json(kind, bytes,
          JSON.stringify(request.sampleIds), JSON.stringify(request.options || {})))
      } else {
        wasm.i2b_begin_json(kind, JSON.stringify(request.options || {}))
        let active = true
        try {
          for (let i = 0; i < files.length; i++) {
            const item = files[i]
            postProgress(request.requestId, i, files.length + 1,
              `${kind}: parsing ${i + 1}/${files.length}: ${item.name}`)
            const bytes = await wasmBytes(item.file)
            wasm.i2b_add_sample_json(item.name, item.sampleId, bytes)
          }
          postProgress(request.requestId, files.length, files.length + 1,
            `${kind}: calculating shared native unsupervised sample distances`)
          view = JSON.parse(wasm.i2b_finish_json())
          active = false
        } finally { if (active) wasm.i2b_abort() }
      }
      postWasmMemory(request.requestId, true)
      self.postMessage({ type: 'i2b-source-distance-result', requestId: request.requestId, view })
      return
    }
    if (request.type === 'run-joint-stability') {
      const result = await runPatientStabilityAsync(request.structure, request.patients,
        request.options || {}, (completed,total,message) => postProgress(request.requestId,completed,total,message))
      self.postMessage({ type: 'joint-stability-result', requestId: request.requestId, result })
      return
    }
    if (request.type === 'run-joint-structure') {
      const result = analyzeJointStructure(request.result, request.options || {})
      self.postMessage({ type: 'joint-structure-result', requestId: request.requestId, result })
      return
    }
    if (request.type === 'run-integrative-distance') {
      const wasm = await loadWasm()
      if (!wasm?.fuse_integrative_distances_json) throw new Error('Rebuild the Rust/WASM distance engine before running this analysis')
      postProgress(request.requestId, 0, 1, 'Combining aligned full-distance views')
      const result = JSON.parse(wasm.fuse_integrative_distances_json(JSON.stringify(request.input)))
      postWasmMemory(request.requestId, true)
      self.postMessage({ type: 'integrative-distance-result', requestId: request.requestId, result })
      return
    }
    if (request.type === 'run-significant-hits') {
      await handleSignificantHits(request)
    }
  } catch (error) {
    postError(request.requestId, error)
  }
})
