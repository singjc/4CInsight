// M1: Input planning only. XLSX interpretation and TSV formatting stay in Rust.
export const mergeMetrics = Object.freeze({
  gene: [
    ['raw-count', 'Raw Count'], ['fpkm', 'FPKM'], ['cpm', 'CPM'],
    ['tpm', 'TPM'], ['percentile-rank', 'Percentile Rank'], ['is-expressed', 'Is Expressed'],
  ],
  transcript: [
    ['transcript-length', 'Transcript Length'], ['effective-transcript-length', 'Effective Transcript Length'],
    ['total-reads', 'Total Reads'], ['tpm', 'TPM'],
    ['percentile-rank', 'Percentile Rank'], ['is-expressed', 'Is Expressed'],
  ],
})
export const defaultMergeMetrics = Object.freeze({ gene: ['raw-count', 'tpm'], transcript: ['total-reads', 'tpm'] })
export function sortedMergeFiles(records, files, kind) {
  if (kind !== 'gene' && kind !== 'transcript') throw new Error('Unsupported expression modality')
  const inputKind = kind === 'gene' ? 'gene-expression' : 'transcript-expression'
  return records.filter(record => record.kind === inputKind)
    .sort((a, b) => {
      const aPath = a.relativePath || a.name
      const bPath = b.relativePath || b.name
      return aPath < bPath ? -1 : aPath > bPath ? 1 : 0
    })
    .map(record => {
      const file = files.get(record.id)
      if (!file) throw new Error(`File must be reconnected: ${record.name}`)
      return file
    })
}
export function validateMergeConfig(kind, metrics, order) {
  const options = mergeMetrics[kind]
  if (!options) throw new Error('Select gene or transcript expression')
  if (!Array.isArray(metrics) || !metrics.length || new Set(metrics).size !== metrics.length ||
      metrics.some(metric => !options.some(([id]) => id === metric))) throw new Error('Select one or more valid measurements')
  if (!['adjacent-by-sample', 'grouped-by-measurement'].includes(order)) throw new Error('Unknown column order')
}
export function mergeBatchRows(rows, samples, measurements, targetBytes = 2 * 1024 * 1024) {
  const perRow = samples * measurements * 9
  if (!Number.isSafeInteger(rows) || rows < 0 || !Number.isSafeInteger(perRow) || perRow < 1) {
    throw new Error('Invalid merge dimensions')
  }
  return Math.min(rows || 1, Math.max(1, Math.min(256, Math.floor(targetBytes / perRow))))
}
