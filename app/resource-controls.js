// Browser resource policy. These limits are safeguards, not OS memory quotas.
export const DEFAULT_RESOURCES = Object.freeze({
  cpuPercent: 85,
  maxThreads: 8,
  maxFileMiB: 256,
  workingBudgetGiB: 1.5,
  physicalRamGiB: 0,
  maxPlotPoints: 2000,
  recycleWorker: true,
})

export function normalizeResources(value = {}) {
  const source = value && typeof value === 'object' ? value : {}
  const clamp = (key, min, max, integer = false) => {
    const input = Number(source[key])
    const fallback = DEFAULT_RESOURCES[key]
    const n = Number.isFinite(input) ? Math.min(max, Math.max(min, input)) : fallback
    return integer ? Math.floor(n) : n
  }
  return {
    cpuPercent: clamp('cpuPercent', 10, 90, true),
    maxThreads: clamp('maxThreads', 1, 32, true),
    maxFileMiB: clamp('maxFileMiB', 16, 4096, true),
    workingBudgetGiB: clamp('workingBudgetGiB', 0.25, 16),
    physicalRamGiB: clamp('physicalRamGiB', 0, 2048),
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

export function checkInputBudget(file, resources = DEFAULT_RESOURCES) {
  const config = normalizeResources(resources)
  if (!file || !Number.isFinite(file.size) || file.size < 0) throw new Error('Invalid local input file.')
  const mib = 1024 * 1024
  if (file.size > config.maxFileMiB * mib) {
    throw new Error(`${file.name}: ${Math.ceil(file.size / mib)} MiB exceeds the configured ${config.maxFileMiB} MiB per-file limit. Open Settings to review the limit.`)
  }
  // This factor is only an early warning for a copy plus WASM input overhead.
  // Uncompressed workbook cells and the science model can use much more RAM.
  if (file.size * 4 > config.workingBudgetGiB * 1024 * mib) {
    throw new Error(`${file.name}: estimated input-copy pressure exceeds the ${config.workingBudgetGiB} GiB working budget. Open Settings to review the limits.`)
  }
}

export function thinPlotPoints(points, maximum = 2000) {
  const cap = Math.max(1, Math.floor(Number(maximum) || 2000))
  if (points.length <= cap) return points
  // Fixed-step selection spans the entire original result order, including
  // upper/lower q-value ranks, without using randomness or changing the data.
  return Array.from({ length: cap }, (_, i) => points[Math.floor(i * (points.length - 1) / (cap - 1 || 1))])
}
