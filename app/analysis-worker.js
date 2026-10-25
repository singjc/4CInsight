import { classifyInputFilename } from './file-classifier.js'

let wasmModulePromise = null

async function loadWasm() {
  if (!wasmModulePromise) {
    wasmModulePromise = (async () => {
      try {
        const moduleUrl = new URL('./wasm/four_c_insight_wasm.js', self.location.href).href
        const module = await import(moduleUrl)
        await module.default()
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
}

async function wasmBytes(file) {
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
  if (!wasm) throw new Error('Rust/WASM is required to inspect metadata. Build or deploy app/wasm first.')
  const bytes = await wasmBytes(request.file)
  const result = JSON.parse(
    wasm.inspect_metadata_json(request.file.name, bytes, request.idColumn || ''),
  )
  self.postMessage({
    type: 'metadata-inspection-result',
    requestId: request.requestId,
    result,
  })
}

async function handleExpressionQc(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Rust/WASM is required for expression QC. Build or deploy app/wasm first.')

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
  if (!wasm) throw new Error('Rust/WASM is required to read metadata values. Build or deploy app/wasm first.')
  if (!request.file || !request.column) throw new Error('Metadata file and column are required')
  const bytes = await wasmBytes(request.file)
  const result = JSON.parse(
    wasm.metadata_column_values_json(request.file.name, bytes, request.column),
  )
  self.postMessage({
    type: 'metadata-column-values-result',
    requestId: request.requestId,
    result,
  })
}

async function handleExpressionComparison(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Rust/WASM is required for group comparison. Build or deploy app/wasm first.')
  if (!request.metadataFile) throw new Error('Select a metadata file before running a group comparison')

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

  result.completedAt = new Date().toISOString()
  self.postMessage({
    type: 'expression-comparison-result',
    requestId: request.requestId,
    result,
  })
}


async function handleLongitudinalExpression(request) {
  const wasm = await loadWasm()
  if (!wasm) throw new Error('Rust/WASM is required for longitudinal analysis. Build or deploy app/wasm first.')
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
  self.postMessage({
    type: 'longitudinal-expression-result',
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
    if (request.type === 'inspect-metadata') {
      await handleMetadataInspection(request)
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
    }
  } catch (error) {
    postError(request.requestId, error)
  }
})
