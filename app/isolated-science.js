// Run one scientific modality in its own disposable Web Worker.
// A Rust/WASM trap must not poison later modalities or discard earlier results.
export function isolatedScienceRequest({
  createWorker, type, payload = {}, resourceSettings = {}, signal,
  onProgress = () => {}, onMemory = () => {},
  requestId = globalThis.crypto?.randomUUID?.() || `isolated-${Date.now()}`,
}) {
  if (signal?.aborted) return Promise.reject(new Error('Analysis stopped'))

  return new Promise((resolve, reject) => {
    let worker
    let settled = false
    let lastStage = 'starting science worker'
    let lastFile = ''

    function finish(error, value) {
      if (settled) return
      settled = true
      signal?.removeEventListener('abort', abort)
      worker?.terminate()
      if (error) reject(error)
      else resolve(value)
    }

    function makeError(detail) {
      const note = lastFile ? `; last file: ${lastFile}` : ''
      return new Error(`${lastStage}${note}: ${detail}`)
    }

    function abort() {
      finish(makeError('Operation stopped by user'))
    }

    try {
      worker = createWorker()
      worker.addEventListener('message', event => {
        if (settled || event.data?.requestId !== requestId) return
        const data = event.data
        if (data.type === 'analysis-progress') {
          lastStage = data.message || lastStage
          const candidate = lastStage.match(/parsing\s+\d+\/\d+:\s*(.+)$/i)
          if (candidate) lastFile = candidate[1]
          onProgress(data)
          return
        }
        if (data.type === 'wasm-memory') {
          onMemory(data)
          return
        }
        if (data.type === 'worker-error') {
          finish(makeError(data.message || 'Science worker failed'))
        } else {
          finish(null, data)
        }
      })
      worker.addEventListener('error', event => {
        event.preventDefault?.()
        finish(makeError(event.message || 'Science worker failed'))
      })
      worker.addEventListener('messageerror', () => finish(makeError('Science worker response could not be decoded')))
      signal?.addEventListener('abort', abort, { once: true })
      if (signal?.aborted) { abort(); return }
      worker.postMessage({ type, requestId, resourceSettings, ...payload })
    } catch (error) {
      finish(error instanceof Error ? error : new Error(String(error)))
    }
  })
}
