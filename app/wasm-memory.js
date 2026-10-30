// The WASM memory object measures the allocated linear-memory buffer only.
// It is not a browser process RSS, host RAM, or bytes in active Rust objects.
export function measureWasmAllocation(memory, previousPeak = 0) {
  const bytes = memory?.buffer?.byteLength
  if (!Number.isSafeInteger(bytes) || bytes < 0) return null
  const prior = Number.isSafeInteger(previousPeak) && previousPeak >= 0 ? previousPeak : 0
  return { allocatedBytes: bytes, peakAllocatedBytes: Math.max(bytes, prior) }
}

export function validateWasmMemoryReport(report) {
  if (!report || typeof report !== 'object') return null
  const { allocatedBytes, peakAllocatedBytes } = report
  if (!Number.isSafeInteger(allocatedBytes) || allocatedBytes < 0 ||
      !Number.isSafeInteger(peakAllocatedBytes) || peakAllocatedBytes < allocatedBytes) return null
  return { allocatedBytes, peakAllocatedBytes }
}
