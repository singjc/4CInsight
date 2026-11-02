// Sample metadata editing rules. Pure functions are shared by the UI and tests.
// The metadata TSV is an ordinary 4C metadata input, not a replacement science model.
export const CORE_COLUMNS = ['sample_id', 'subject_id', 'analysis_group', 'timepoint', 'qc_review_status', 'notes']
export const MAPPING_KINDS = {
  'gene-expression': 'gene_expression_id',
  'transcript-expression': 'transcript_expression_id',
  bcr: 'bcr_id',
  'bcr-isotype': 'bcr_isotype_id',
  'tcr-alpha': 'tcr_alpha_id',
  'tcr-beta': 'tcr_beta_id',
  'immune-infiltrate': 'immune_infiltrate_id',
}
export const OPTIONAL_COLUMNS = [
  'cohort', 'diagnosis', 'disease_subtype', 'Phenotype', 'HYP', 'collection_date',
  'sample_source', 'tissue', 'specimen_type', 'sex', 'age_years', 'site',
  'study_id', 'collection_batch', 'processing_batch', 'sequencing_batch',
  'analysis_include', 'qc_exclusion_reason', ...Object.values(MAPPING_KINDS),
]
const controlChars = /[\u0000-\u0008\u000b\u000c\u000e-\u001f]/

export function validateColumnName(value, columns = []) {
  const name = String(value ?? '').trim()
  if (!name || controlChars.test(name) || /[\t\r\n]/.test(name)) throw new Error('Enter a column name without tabs or line breaks')
  if (columns.includes(name)) throw new Error(`The column '${name}' already exists`)
  return name
}

export function parseDelimited(text, delimiter = '\t') {
  if (!['\t', ','].includes(delimiter)) throw new Error('Unsupported delimiter')
  const source = String(text ?? '').replace(/^\uFEFF/, '')
  const lines = []
  let row = [], field = '', quoted = false, afterQuote = false
  for (let index = 0; index < source.length; index++) {
    const char = source[index]
    if (quoted) {
      if (char === '"' && source[index + 1] === '"') { field += '"'; index++ }
      else if (char === '"') { quoted = false; afterQuote = true }
      else field += char
    } else if (char === '"' && field === '' && !afterQuote) quoted = true
    else if (char === delimiter) { row.push(field); field = ''; afterQuote = false }
    else if (char === '\n' || char === '\r') {
      row.push(field)
      if (row.some(value => value !== '')) lines.push(row)
      row = []; field = ''; afterQuote = false
      if (char === '\r' && source[index + 1] === '\n') index++
    } else {
      if (afterQuote && char !== ' ') throw new Error('Unexpected character after quoted field')
      if (!afterQuote) field += char
    }
  }
  if (quoted) throw new Error('Metadata file has an unclosed quoted field')
  row.push(field)
  if (row.some(value => value !== '')) lines.push(row)
  if (!lines.length) throw new Error('Metadata file is empty')
  const columns = lines.shift().map(value => value.trim())
  if (!columns.length || columns.some(value => !value) || new Set(columns).size !== columns.length) throw new Error('Metadata headers must be nonempty and unique')
  const rows = lines.map(line => columns.map((_, index) => line[index] ?? ''))
  if (lines.some(line => line.length > columns.length)) throw new Error('Metadata row has more cells than the header')
  return { columns, rows }
}

function quoteField(value, delimiter) {
  const text = String(value ?? '')
  return /["\r\n]/.test(text) || text.includes(delimiter) ? `"${text.replaceAll('"', '""')}"` : text
}
export function serializeDelimited(table, delimiter = '\t') {
  return [table.columns, ...table.rows].map(row => row.map(value => quoteField(value, delimiter)).join(delimiter)).join('\n') + '\n'
}
export function createBlankMetadata() {
  return { columns: [...CORE_COLUMNS], rows: [] }
}
export function draftFromSamples(records) {
  const ids = [...new Set(records.filter(record => MAPPING_KINDS[record.kind] && record.sampleId).map(record => record.sampleId.trim()))]
    .filter(Boolean).sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
  return { columns: [...CORE_COLUMNS], rows: ids.map(id => [id, '', '', '', '', '']) }
}
export function validateMetadata(table, idColumn = 'sample_id') {
  const errors = []
  if (!Array.isArray(table?.columns) || !Array.isArray(table?.rows)) return { errors: ['Invalid metadata table'], blankIds: [], duplicates: [], total: 0 }
  if (!table.columns.includes(idColumn)) errors.push(`Select a sample ID column (missing '${idColumn}')`)
  if (table.columns.some(name => !name || controlChars.test(name)) || new Set(table.columns).size !== table.columns.length) errors.push('Column headers must be nonempty and unique')
  const index = table.columns.indexOf(idColumn)
  const counts = new Map(), blankIds = []
  if (index >= 0) table.rows.forEach((row, rowIndex) => {
    const value = String(row[index] ?? '').trim()
    if (!value) blankIds.push(rowIndex)
    else counts.set(value, (counts.get(value) || 0) + 1)
  })
  const duplicates = [...counts].filter(([, count]) => count > 1).map(([value]) => value)
  return { errors, blankIds, duplicates, total: table.rows.length }
}
export function addColumn(table, name) {
  const valid = validateColumnName(name, table.columns)
  return { columns: [...table.columns, valid], rows: table.rows.map(row => [...row, '']) }
}
export function renameColumn(table, index, name) {
  if (!Number.isInteger(index) || index < 0 || index >= table.columns.length) throw new Error('Invalid column index')
  if (table.columns[index] === 'sample_id') throw new Error('The canonical sample_id column cannot be renamed')
  const valid = validateColumnName(name, table.columns.filter((_, i) => i !== index))
  return { columns: table.columns.map((col, i) => i === index ? valid : col), rows: table.rows.map(row => [...row]) }
}
export function removeColumn(table, index) {
  if (!Number.isInteger(index) || index < 0 || index >= table.columns.length) throw new Error('Invalid column index')
  if (table.columns[index] === 'sample_id') throw new Error('The canonical sample_id column cannot be removed')
  return { columns: table.columns.filter((_, i) => i !== index), rows: table.rows.map(row => row.filter((_, i) => i !== index)) }
}
export function addRow(table, sampleId = '') {
  const row = table.columns.map(column => column === 'sample_id' ? String(sampleId) : '')
  return { columns: [...table.columns], rows: [...table.rows, row] }
}
export function setCell(table, rowIndex, columnIndex, value) {
  if (!table.rows[rowIndex] || !table.columns[columnIndex]) throw new Error('Invalid table cell')
  const rows = [...table.rows], row = [...rows[rowIndex]]
  row[columnIndex] = String(value ?? '')
  rows[rowIndex] = row
  return { columns: [...table.columns], rows }
}
export function removeRow(table, rowIndex) {
  return { columns: [...table.columns], rows: table.rows.filter((_, index) => index !== rowIndex) }
}
export function matchingFiles(records) {
  return records.filter(record => Object.hasOwn(MAPPING_KINDS, record.kind))
}
export function autoMapFiles(records, table, idColumn = 'sample_id', previous = {}) {
  const idIndex = table.columns.indexOf(idColumn)
  const ids = new Set(idIndex < 0 ? [] : table.rows.map(row => String(row[idIndex] || '').trim()).filter(Boolean))
  const mappings = {}
  for (const record of matchingFiles(records)) {
    const saved = previous[record.id]
    mappings[record.id] = ids.has(saved) ? saved : ids.has(record.sampleId) ? record.sampleId : ''
  }
  return mappings
}
export function mappingSummary(records, mappings, table, idColumn = 'sample_id') {
  const validation = validateMetadata(table, idColumn)
  const ids = new Set(table.rows.map(row => String(row[table.columns.indexOf(idColumn)] ?? '').trim()).filter(Boolean))
  let matched = 0, missing = 0
  for (const record of matchingFiles(records)) {
    if (ids.has(mappings[record.id])) matched++
    else missing++
  }
  return { matched, missing, duplicates: validation.duplicates.length, blank: validation.blankIds.length }
}
export function mappingManifest(records, mappings) {
  return { columns: ['relative_path', 'file_name', 'modality', 'sample_id'],
    rows: matchingFiles(records).map(record => [record.relativePath || record.name, record.name, record.kind, mappings[record.id] || '']) }
}
export function filterRows(table, query = '', column = '', value = '', sortColumn = '', descending = false) {
  const needle = String(query).toLocaleLowerCase().trim()
  const filterIndex = table.columns.indexOf(column), sortIndex = table.columns.indexOf(sortColumn)
  const rows = table.rows.map((row, index) => ({ row, index })).filter(({ row }) => {
    if (filterIndex >= 0 && value && String(row[filterIndex] ?? '') !== value) return false
    return !needle || row.some(cell => String(cell).toLocaleLowerCase().includes(needle))
  })
  if (sortIndex >= 0) rows.sort((a, b) => {
    const result = String(a.row[sortIndex] ?? '').localeCompare(String(b.row[sortIndex] ?? ''), undefined, { numeric: true, sensitivity: 'base' })
    return (descending ? -result : result) || a.index - b.index
  })
  return rows
}

// Write file-derived IDs to the standard modality alias columns. Reject
// conflicts: two differently named files in the same modality may not silently
// overwrite one sample's alias. Use a separate manifest for full file paths.
export function applyMappingsToAliases(table, records, mappings, idColumn = 'sample_id') {
  const idIndex = table.columns.indexOf(idColumn)
  if (idIndex < 0) throw new Error(`Missing sample identifier column: ${idColumn}`)
  const idRows = new Map()
  table.rows.forEach((row, i) => {
    const id = String(row[idIndex] || '').trim()
    if (idRows.has(id)) throw new Error(`Duplicate sample ID: ${id}`)
    if (id) idRows.set(id, i)
  })
  let updated = { columns: [...table.columns], rows: table.rows.map(row => [...row]) }
  const seen = new Map()
  for (const record of matchingFiles(records)) {
    const sample = mappings[record.id]
    if (!sample) continue
    const rowIndex = idRows.get(sample)
    if (rowIndex === undefined) throw new Error(`Mapped sample '${sample}' does not exist`)
    const column = MAPPING_KINDS[record.kind]
    const alias = String(record.sampleId || '').trim()
    if (!alias) continue
    const key = `${rowIndex}:${column}`
    if (seen.has(key) && seen.get(key) !== alias) {
      throw new Error(`Conflicting ${column} values for sample '${sample}': ${seen.get(key)} and ${alias}`)
    }
    seen.set(key, alias)
    if (!updated.columns.includes(column)) updated = addColumn(updated, column)
    const columnIndex = updated.columns.indexOf(column)
    const current = String(updated.rows[rowIndex][columnIndex] || '').trim()
    if (current && current !== alias) {
      throw new Error(`Existing ${column} value for sample '${sample}' conflicts with '${alias}'`)
    }
    updated.rows[rowIndex][columnIndex] = alias
  }
  return { table: updated, aliasesWritten: seen.size }
}
