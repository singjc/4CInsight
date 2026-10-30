// Browser resource preferences. File size does not block local cohort inputs.
// Browser RAM information is deliberately labelled by source and capability.
export const DEFAULT_RESOURCES = Object.freeze({
  cpuPercent: 85,
  maxThreads: 8,
  maxPlotPoints: 2000,
  recycleWorker: true,
})

export function normalizeResources(value = {}) {
  const source = value && typeof value === 'object' ? value : {}
  const clamp = (key, min, max, integer = false) => {
    const input = source[key]
    const numeric = input === '' || input === null || input === undefined ? NaN : Number(input)
    const fallback = DEFAULT_RESOURCES[key]
    const n = Number.isFinite(numeric) ? Math.min(max, Math.max(min, numeric)) : fallback
    return integer ? Math.floor(n) : n
  }
  return {
    cpuPercent: clamp('cpuPercent', 10, 90, true),
    maxThreads: clamp('maxThreads', 1, 32, true),
    maxPlotPoints: clamp('maxPlotPoints', 200, 6000, true),
    recycleWorker: typeof source.recycleWorker === 'boolean' ? source.recycleWorker : true,
  }
}

export function suggestedScienceThreads(logicalThreads, resources = DEFAULT_RESOURCES) {
  const n = Math.max(1, Math.trunc(Number(logicalThreads) || 1))
  const config = normalizeResources(resources)
  // Leave a logical core for the OS and UI. PX1 must implement the actual pool.
  return Math.max(1, Math.min(n - 1 || 1, Math.floor(n * config.cpuPercent / 100) || 1, config.maxThreads))
}

export function validateLocalInputFile(file) {
  // No size or estimated-copy-pressure threshold. Large cohort files are valid.
  // This validates metadata only; memory safety requires streaming science inputs.
  if (!file || !Number.isSafeInteger(file.size) || file.size < 0) {
    throw new Error('Invalid local input file metadata.')
  }
}

// Application memory is not physical RAM. These samples are browser-dependent.
// Chromium may expose either of these APIs. Firefox often exposes neither.
export async function measureAppMemory(performanceObject, isolated = false) {
  if (isolated && typeof performanceObject?.measureUserAgentSpecificMemory === 'function') {
    try {
      const sample = await performanceObject.measureUserAgentSpecificMemory()
      if (Number.isFinite(sample?.bytes) && sample.bytes >= 0) {
        return { available: true, bytes: sample.bytes, source: 'App estimate (document and workers)', kind: 'app' }
      }
    } catch {
      // Browser restrictions or a transient sampling failure; try JS heap only.
    }
  }
  const heapBytes = Number(performanceObject?.memory?.usedJSHeapSize)
  if (Number.isFinite(heapBytes) && heapBytes >= 0 && performanceObject?.memory) {
    return { available: true, bytes: heapBytes, source: 'JS heap only (excludes worker/WASM memory)', kind: 'heap' }
  }
  return { available: false, bytes: null, source: 'App memory measurement is not available in this browser', kind: 'unavailable' }
}

export function thinPlotPoints(points, maximum = 2000) {
  const cap = Math.max(1, Math.floor(Number(maximum) || 2000))
  if (points.length <= cap) return points
  // Fixed-step selection spans the full original result, without changing science.
  return Array.from({ length: cap }, (_, i) => points[Math.floor(i * (points.length - 1) / (cap - 1 || 1))])
}

// Strict browser capability report. Hardware concurrency is not a CPU-load meter.
// Device Memory returns an approximate, privacy-rounded class when supported;
// Firefox currently does not expose this API. Do not substitute disk quota,
// WASM memory pages, or the JS heap limit for physical/free RAM.
export function detectBrowserResources(nav = {}, perf = {}, isolated = false) {
  const platform = String(nav.userAgentData?.platform || nav.platform || 'Unknown OS')
  const rawThreads = Number(nav.hardwareConcurrency)
  const threads = Number.isSafeInteger(rawThreads) && rawThreads > 0 ? rawThreads : 0
  const rawMemory = Number(nav.deviceMemory)
  const deviceMemoryGiB = Number.isFinite(rawMemory) && rawMemory > 0 ? rawMemory : null
  const memory = deviceMemoryGiB === null
    ? 'System RAM unavailable (browser restriction)'
    : `≈${deviceMemoryGiB} GiB RAM class (approximate)`
  const memoryNote = deviceMemoryGiB === null
    ? 'This browser does not expose physical RAM or available system memory to websites.'
    : 'Browser-reported RAM is rounded and may be capped. Free/available system RAM is not exposed.'
  return {
    platform, threads, deviceMemoryGiB, memory, memoryNote,
    crossOriginIsolated: Boolean(isolated),
  }
}
