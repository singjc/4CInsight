import { mergeBatchRows, validateMergeConfig } from './merge-helpers.js'

// OPFS is private to the site's browser origin. Only the selected XLSX File
// objects are read, one by one; scratch is never uploaded to any server.
export async function runPortableMerge(args) {
  // A Web Lock prevents concurrent tabs from clearing each other's scratch.
  // When a worker is terminated, the browser releases its lock. The next run
  // can remove abandoned scratch from that terminated operation safely.
  if (navigator.locks?.request) {
    return navigator.locks.request('4c-insight-opfs-merge', { mode: 'exclusive' }, () => performMerge(args, true))
  }
  return performMerge(args, false)
}

async function performMerge({ wasm, request, postProgress, postWasmMemory }, ownsScratchLock) {
  const files = request.files || []
  const { kind, metrics, order } = request
  validateMergeConfig(kind, metrics, order)
  if (!files.length) throw new Error('Select expression workbooks in the Project inventory first')
  if (!navigator.storage?.getDirectory) {
    throw new Error('Browser-local temporary file storage (OPFS) is not available. Try an up-to-date Firefox, Chrome, or Edge browser.')
  }
  const root = await navigator.storage.getDirectory()
  const scratchRoot = await root.getDirectoryHandle('4c-insight-merge-scratch', { create: true })
  if (ownsScratchLock) {
    for await (const [name, handle] of scratchRoot.entries()) {
      if (handle.kind === 'directory' && name.startsWith('run-')) {
        await scratchRoot.removeEntry(name, { recursive: true })
      }
    }
  }
  const runId = `run-${crypto.randomUUID()}`
  const scratchDir = await scratchRoot.getDirectoryHandle(runId, { create: true })
  let completed = 0
  const total = files.length
  const report = { missingRows: 0, unknownRows: 0 }
  const fileHandles = []
  let finished = false
  let outputFilename = ''
  try {
    postProgress(0, total, `Inspecting canonical ${kind} workbook: ${files[0].name}`)
    const plan = JSON.parse(wasm.merge_begin_json(
      new Uint8Array(await files[0].arrayBuffer()), JSON.stringify(kind), JSON.stringify(metrics), JSON.stringify(order),
      JSON.stringify(files.map(file => file.name)),
    ))
    if (plan.samples !== total || plan.measurements !== metrics.length || !Number.isSafeInteger(plan.rows)) {
      throw new Error('Invalid merge plan returned from Rust/WASM')
    }
    const stride = metrics.length * 9
    for (const file of files) {
      postProgress(completed, total, `Reading ${kind} workbook ${completed + 1}/${total}: ${file.name}`)
      // Only one XLSX workbook and one packed column buffer exist at a time.
      const packed = wasm.merge_sample_packed(new Uint8Array(await file.arrayBuffer()))
      if (packed.byteLength !== plan.rows * stride) throw new Error('WASM packed-column length mismatch')
      const sampleReport = JSON.parse(wasm.merge_last_sample_json())
      report.missingRows += sampleReport.missingRows
      report.unknownRows += sampleReport.unknownRows
      const handle = await scratchDir.getFileHandle(`sample-${String(completed).padStart(5, '0')}.bin`, { create: true })
      const writer = await handle.createWritable()
      try { await writer.write(packed) } finally { await writer.close() }
      fileHandles.push(handle)
      completed += 1
      postProgress(completed, total, `Parsed ${completed}/${total} ${kind} workbooks`)
      postWasmMemory?.(true)
    }
    const outputDir = await root.getDirectoryHandle('4c-insight-merged-tables', { create: true })
    const stamp = new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14)
    outputFilename = `${stamp}_${crypto.randomUUID().slice(0, 8)}_merged_${kind}_expression.tsv`
    const outHandle = await outputDir.getFileHandle(outputFilename, { create: true })
    const out = await outHandle.createWritable()
    try {
      await out.write(wasm.merge_header_tsv())
      const blockRows = mergeBatchRows(plan.rows, total, metrics.length)
      const inputFiles = []
      for (const handle of fileHandles) inputFiles.push(await handle.getFile())
      for (let start = 0; start < plan.rows; start += blockRows) {
        const count = Math.min(blockRows, plan.rows - start)
        const size = count * stride
        const window = new Uint8Array(total * size)
        for (let sample = 0; sample < total; sample += 1) {
          const bytes = await inputFiles[sample].slice(start * stride, start * stride + size).arrayBuffer()
          if (bytes.byteLength !== size) throw new Error('Incomplete local scratch file')
          window.set(new Uint8Array(bytes), sample * size)
        }
        await out.write(wasm.merge_render_rows_tsv(start, count, window))
        postProgress(total, total, `Writing merged ${kind} table: ${Math.min(start + count, plan.rows)}/${plan.rows} features`)
      }
      finished = true
    } finally { await out.close() }
    const resultFile = await outHandle.getFile()
    return { file: resultFile, summary: { ...plan, ...report, name: resultFile.name, bytes: resultFile.size, execution: 'local-rust-wasm-opfs' } }
  } catch (error) {
    if (outputFilename && !finished) {
      try { await (await root.getDirectoryHandle('4c-insight-merged-tables')).removeEntry(outputFilename) } catch { /* best-effort rollback */ }
    }
    throw error
  } finally {
    wasm.merge_reset()
    // Cancellation terminates the worker. If terminated during this block, a
    // scratch run can remain; it is not a raw upload and can be cleared locally.
    try { await scratchRoot.removeEntry(runId, { recursive: true }) } catch { /* best effort */ }
  }
}
