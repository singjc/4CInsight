/** Local, header-only metadata identity column discovery. No file upload. */
import { parseDelimited } from './sample-metadata.js'

/**
 * Read only the first complete delimited record, including quoted line breaks.
 * An arbitrarily long header is supported. Source data are not retained.
 * This function does not validate sample uniqueness or make identity assignments.
 */
export async function readIntegrativeMetadataColumns(file, signal = null) {
  if (/\.xlsx?$/i.test(String(file?.name||''))) {
    throw new Error('Excel metadata columns must be read through the local WASM metadata parser')
  }
  if (!file || typeof file.stream !== 'function') {
    throw new Error('Connect a local metadata file before selecting its columns')
  }
  const reader = file.stream().getReader()
  const decoder = new TextDecoder('utf-8', { fatal: true })
  let record = ''
  let quoted = false
  let complete = false
  try {
    while (!complete) {
      if (signal?.aborted) throw new Error('Metadata header reading stopped')
      const { done, value } = await reader.read()
      if (done) {
        const end = decoder.decode()
        if (end) record += end
        break
      }
      for (const char of decoder.decode(value, { stream: true })) {
        if (char === '"') quoted = !quoted
        if ((char === '\r' || char === '\n') && !quoted) {
          complete = true
          break
        }
        record += char
      }
    }
    if (signal?.aborted) throw new Error('Metadata header reading stopped')
    if (quoted) throw new Error('Metadata header has an unclosed quoted field')
    if (!record.trim()) throw new Error('Metadata file header is empty')
    const delimiter = /\.csv$/i.test(String(file.name || '')) ? ',' :
      record.includes('\t') ? '\t' : ','
    const { columns } = parseDelimited(record, delimiter)
    if (columns.some(c => /[\r\n]/.test(c))) throw new Error('Metadata column name must not contain line breaks')
    return columns
  } finally {
    try { await reader.cancel() } catch { /* Reader may already be closed. */ }
    try { reader.releaseLock() } catch { /* Lock may already be released. */ }
  }
}

/** Select only an existing column; never infer a patient from an arbitrary name. */
export function validIntegrativeIdentityRoles(columns, roles = {}, preferredObservation = '') {
  const found = new Set(columns)
  const observation = found.has(roles.observation) ? roles.observation :
    [preferredObservation, 'sample_id', '4C ID', '4C_ID', 'Sample ID', 'sample'].find(c => found.has(c)) || columns[0] || ''
  const subject = found.has(roles.subject) ? roles.subject : ''
  const time = found.has(roles.time) && roles.time !== observation && roles.time !== subject ? roles.time : ''
  return { observation, subject, time }
}
