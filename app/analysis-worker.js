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

self.addEventListener('message', async (event) => {
  const request = event.data
  if (!request || request.type !== 'preflight') return

  try {
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
  } catch (error) {
    self.postMessage({
      type: 'worker-error',
      requestId: request.requestId,
      message: error instanceof Error ? error.message : String(error),
    })
  }
})
