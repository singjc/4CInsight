/**
 * I1: Exact per-observation, seven-modality source alignment.
 * Metadata and file identities only. No statistical model, normalization,
 * feature selection, distance fusion, or missing-data imputation is performed.
 */
export const I1_MODALITIES = Object.freeze([
  Object.freeze({ key: 'gene', label: 'Gene expression', kind: 'gene-expression', mergedKind: 'merged-gene-expression', alias: 'gene_expression_id' }),
  Object.freeze({ key: 'transcript', label: 'Transcript expression', kind: 'transcript-expression', mergedKind: 'merged-transcript-expression', alias: 'transcript_expression_id' }),
  Object.freeze({ key: 'bcr', label: 'BCR repertoire', kind: 'bcr', alias: 'bcr_id' }),
  Object.freeze({ key: 'tcrAlpha', label: 'TCR alpha', kind: 'tcr-alpha', alias: 'tcr_alpha_id' }),
  Object.freeze({ key: 'tcrBeta', label: 'TCR beta', kind: 'tcr-beta', alias: 'tcr_beta_id' }),
  Object.freeze({ key: 'isotype', label: 'BCR isotypes', kind: 'bcr-isotype', alias: 'bcr_isotype_id' }),
  Object.freeze({ key: 'immune', label: 'Immune infiltration', kind: 'immune-infiltrate', alias: 'immune_infiltrate_id' }),
])

const GENE_METRICS = ['Percentile Rank', 'Is Expressed', 'Raw Count', 'FPKM', 'CPM', 'TPM']
const TRANSCRIPT_METRICS = ['Effective Transcript Length', 'Transcript Length', 'Percentile Rank', 'Total Reads', 'TPM']

function exactId(value) { return String(value ?? '').trim() }
function normalizeRuleValue(value) { return exactId(value).toLocaleLowerCase('en-US') }

function requiredColumn(columns, name, label) {
  const index = columns.indexOf(name)
  if (index < 0) throw new Error(`${label} column '${name}' was not found in metadata`)
  return index
}

function validColumnChoice(columns, name, label) {
  if (!name) return -1
  return requiredColumn(columns, name, label)
}

function sourceLabel(record) { return String(record.relativePath || record.name || record.id || '') }

function buildSourceIndex(records, kind) {
  const map = new Map()
  for (const record of records) {
    if (record?.kind !== kind || !exactId(record?.sampleId)) continue
    const sample = exactId(record.sampleId)
    if (!map.has(sample)) map.set(sample, [])
    map.get(sample).push(sourceLabel(record))
  }
  return map
}

/**
 * Extract the FIRST row of a TSV without reading the whole merged expression table.
 * Files can be arbitrarily large; no maximum file size or fixed header byte cap.
 * The caller can cancel via AbortSignal. A quoted line break is retained.
 */
export async function readFirstTsvRecord(file, signal = null) {
  if (typeof file?.stream !== 'function') throw new Error('A local merged-table File is required')
  const reader = file.stream().getReader()
  const decoder = new TextDecoder('utf-8', { fatal: true })
  let header = '', inQuotes = false, foundEnd = false
  try {
    while (true) {
      if (signal?.aborted) throw new Error('Header reading stopped')
      const { done, value } = await reader.read()
      if (done) break
      const text = decoder.decode(value, { stream: true })
      for (const ch of text) {
        // A doubled quote toggles twice, even across chunk boundaries.
        // Embedded newlines inside quoted fields do not end the header.
        if (ch === '"') inQuotes = !inQuotes
        if (!inQuotes && (ch === '\n' || ch === '\r')) { foundEnd = true; break }
        header += ch
      }
      if (foundEnd) break
    }
    if (!foundEnd && inQuotes) throw new Error('Unterminated quoted TSV header')
    if (!header) throw new Error('Merged TSV header is empty')
    return parseTsvHeader(header)
  } finally {
    try { await reader.cancel() } catch { /* already at EOF */ }
    try { reader.releaseLock() } catch { /* no active lock */ }
  }
}

/** Strict TSV field parser for the first row; escaped quotes stay exact. */
export function parseTsvHeader(line) {
  const out = []
  let value = '', quoted = false, start = true, closed = false
  const text = String(line).replace(/^\uFEFF/, '')
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') { value += '"'; i++ }
      else if (c === '"') { quoted = false; closed = true }
      else value += c
    } else if (c === '\t') { out.push(value); value = ''; closed = false; start = true }
    else if (c === '"' && start) { quoted = true; start = false }
    else if (closed) { if (c !== ' ') throw new Error('Malformed quotation in TSV header') }
    else { if (c === '"') throw new Error('Unexpected quote in TSV header'); value += c; start = false }
  }
  if (quoted) throw new Error('Unterminated quoted TSV header')
  out.push(value)
  return out
}

/**
 * Interpret only known canonical wide-format merged header suffixes.
 * This audits presence, not numeric expression values or analysis eligibility.
 */
export function mergedSampleIds(header, modality) {
  const fields = Array.isArray(header) ? header : parseTsvHeader(header)
  const first = modality === 'gene' ? 'Gene Symbol' : 'Transcript Name'
  if (fields[0] !== first || fields[1] !== 'NCBI Gene ID') {
    throw new Error(`Merged ${modality} table has an unrecognized 4C header`)
  }
  const suffixes = modality === 'gene' ? GENE_METRICS : modality === 'transcript' ? TRANSCRIPT_METRICS : null
  if (!suffixes) throw new Error(`Unsupported merged expression modality: ${modality}`)
  const counts = new Map()
  for (const col of fields.slice(2)) {
    const suffix = suffixes.find(metric => col.endsWith(` ${metric}`))
    if (!suffix) throw new Error(`Unknown merged ${modality} measurement: ${col}`)
    const sample = col.slice(0, -suffix.length - 1)
    if (!sample || sample !== sample.trim()) throw new Error(`Invalid merged ${modality} sample identifier`)
    const columns = counts.get(sample) || new Set()
    if (columns.has(suffix)) throw new Error(`Repeated ${modality} column '${col}'`)
    columns.add(suffix)
    counts.set(sample, columns)
  }
  if (!counts.size) throw new Error(`Merged ${modality} table has no sample columns`)
  return [...counts.keys()]
}

function exclusionConfiguration(table, rules) {
  const configs = []
  for (const rule of rules || []) {
    if (!rule?.enabled) continue
    const column = exactId(rule.column)
    const values = (rule.values || []).map(normalizeRuleValue).filter(Boolean)
    if (!column || !values.length) throw new Error(`Exclusion '${rule.label || 'rule'}' is incomplete`)
    configs.push({ label: String(rule.label || column), index: requiredColumn(table.columns, column, 'Exclusion'), values: new Set(values) })
  }
  return configs
}

function mergedIndexEntry(source) {
  if (!source) return { ids: new Set(), error: null, source: '' }
  const ids = Array.isArray(source.sampleIds) ? source.sampleIds : []
  return { ids: new Set(ids), error: source.error ? String(source.error) : null, source: String(source.source || '') }
}

/**
 * Strict mapping rules:
 * 1. nonblank modality alias takes precedence; it is NEVER replaced on failure;
 * 2. otherwise the selected observation ID is the exact source identity;
 * 3. ambiguous source duplicates and aliases across rows are not resolved;
 * 4. missing and excluded rows remain in the exported ledger.
 */
export function alignObservations({ table, records = [], merged = {}, observationColumn, subjectColumn = '', timeColumn = '', exclusions = [], enabled = {} }) {
  if (!Array.isArray(table?.columns) || !Array.isArray(table?.rows) || !table.columns.length) throw new Error('Select a valid metadata table')
  if (new Set(table.columns).size !== table.columns.length) throw new Error('Metadata headers must be unique')
  const obsIdx = requiredColumn(table.columns, observationColumn, 'Observation ID')
  const subjectIdx = validColumnChoice(table.columns, subjectColumn, 'Patient / subject')
  const timeIdx = validColumnChoice(table.columns, timeColumn, 'Timepoint')
  // A unique sample ID can also be its patient ID when each patient has one sample.
  // Keep the unique observation-ID check below; never merge repeated rows.
  if (timeIdx >= 0 && (timeIdx === obsIdx || timeIdx === subjectIdx)) throw new Error('Timepoint must use a separate metadata column')
  const exclusionConfigs = exclusionConfiguration(table, exclusions)
  const seenObs = new Set()
  const sources = Object.fromEntries(I1_MODALITIES.map(mod => [mod.key, buildSourceIndex(records, mod.kind)]))
  const mergedSources = Object.fromEntries(I1_MODALITIES.map(mod => [mod.key, mergedIndexEntry(merged?.[mod.key])]))
  const owners = Object.fromEntries(I1_MODALITIES.map(mod => [mod.key, new Map()]))
  const warnings = []
  for (const [key, info] of Object.entries(mergedSources)) if (info.error) warnings.push(`${key}: merged table header could not be read: ${info.error}`)
  const rows = table.rows.map((cells, i) => {
    const observation = exactId(cells[obsIdx])
    if (!observation) throw new Error(`Metadata row ${i + 2} has an empty observation ID`)
    if (seenObs.has(observation)) throw new Error(`Duplicate observation ID '${observation}' (row ${i + 2})`)
    seenObs.add(observation)
    const exclusionsMatched = exclusionConfigs.filter(rule => rule.values.has(normalizeRuleValue(cells[rule.index]))).map(rule => rule.label)
    const subject = subjectIdx < 0 ? '' : exactId(cells[subjectIdx])
    const timepoint = timeIdx < 0 ? '' : exactId(cells[timeIdx])
    const modalities = {}
    for (const mod of I1_MODALITIES) {
      const aliasIndex = table.columns.indexOf(mod.alias)
      const explicitAlias = aliasIndex < 0 ? '' : exactId(cells[aliasIndex])
      const alias = explicitAlias || observation
      const active = enabled?.[mod.key] !== false
      const individual = sources[mod.key].get(alias) || []
      const merge = mergedSources[mod.key]
      const linked = [...individual]
      if (merge.ids.has(alias)) linked.push(merge.source || 'Merged expression table')
      let status = !active ? 'not-selected' : linked.length === 0 ? (merge.error && mod.mergedKind ? 'source-error' : 'missing') : individual.length > 1 ? 'ambiguous' : 'present'
      modalities[mod.key] = { alias, explicitAlias: Boolean(explicitAlias), status, source: linked.join(' | '), files: linked.length }
      if (active) {
        const existing = owners[mod.key].get(alias) || []
        existing.push(i)
        owners[mod.key].set(alias, existing)
      }
    }
    return { observation, subject, timepoint, excluded: exclusionsMatched.length > 0, exclusions: exclusionsMatched, modalities, rowNumber: i + 2 }
  })
  // One modality source alias cannot identify two independent observations.
  for (const mod of I1_MODALITIES) {
    for (const [alias, indices] of owners[mod.key]) {
      if (indices.length < 2) continue
      for (const index of indices) rows[index].modalities[mod.key].status = 'ambiguous'
      warnings.push(`${mod.label}: source ID '${alias}' is assigned to ${indices.length} observations`)
    }
  }
  // Repeated patient/timepoint is a review warning, not automatic exclusion.
  const patientVisits = new Map()
  if (subjectIdx >= 0 && timeIdx >= 0) for (const row of rows) {
    if (!row.subject || !row.timepoint) continue
    const key = JSON.stringify([row.subject, row.timepoint])
    const seen = patientVisits.get(key) || []
    seen.push(row.observation)
    patientVisits.set(key, seen)
  }
  for (const [key, observations] of patientVisits) {
    if (observations.length > 1) {
      const [subject, time] = JSON.parse(key)
      warnings.push(`Patient '${subject}' has ${observations.length} observations at time '${time}'. Review replicates; nothing was merged.`)
    }
  }
  if (subjectIdx >= 0) {
    const nMissing = rows.filter(r => !r.subject).length
    if (nMissing) warnings.push(`${nMissing} observations have no patient/subject ID`)
  }
  if (timeIdx >= 0) {
    const nMissing = rows.filter(r => !r.timepoint).length
    if (nMissing) warnings.push(`${nMissing} observations have no timepoint`)
  }
  // An unlinked source is reported. It is not assigned to a nearby name.
  for (const mod of I1_MODALITIES) {
    if (enabled?.[mod.key] === false) continue
    const known = new Set(rows.map(r => r.modalities[mod.key].alias))
    const unlinked = [...sources[mod.key].keys()].filter(id => !known.has(id))
    const mergedUnlinked = [...mergedSources[mod.key].ids].filter(id => !known.has(id))
    if (unlinked.length || mergedUnlinked.length) {
      const examples = [...unlinked, ...mergedUnlinked].slice(0, 5)
      warnings.push(`${mod.label}: ${unlinked.length} per-sample source IDs and ${mergedUnlinked.length} merged-table source IDs have no exact metadata match. Examples: ${examples.join(', ')}`)
    }
  }
  const active = rows.filter(r => !r.excluded)
  const counts = I1_MODALITIES.map(mod => {
    const statuses = { present: 0, missing: 0, ambiguous: 0, 'source-error': 0, 'not-selected': 0 }
    for (const row of active) statuses[row.modalities[mod.key].status]++
    return { key: mod.key, label: mod.label, selected: enabled?.[mod.key] !== false, ...statuses }
  })
  const selected = I1_MODALITIES.filter(mod => enabled?.[mod.key] !== false)
  if (selected.length < 2) warnings.push('Select at least two modalities to prepare a joint analysis.')
  const complete = active.filter(row => selected.length >= 2 && selected.every(mod => row.modalities[mod.key].status === 'present')).length
  const pairedTwo = active.filter(row => selected.filter(mod => row.modalities[mod.key].status === 'present').length >= 2).length
  return {
    schema: '4c-insight-observation-alignment-v1',
    model: 'source-identity-audit-only',
    roles: { observationColumn, subjectColumn, timeColumn },
    exclusions: (exclusions || []).filter(r => r.enabled).map(r => ({ label: r.label, column: r.column, values: [...r.values] })),
    rows, warnings,
    summary: { total: rows.length, eligible: active.length, excluded: rows.length - active.length, complete, pairedTwo, counts },
  }
}

function safeTsvCell(value) {
  const text = String(value ?? '')
  // Spreadsheet-view safety; JSON retains exact source IDs for machine use.
  const guarded = /^[\s]*[=+@]/.test(text) || /^[\s]*-(?!\d)/.test(text) ? `'${text}` : text
  return /[\t\n\r"]/.test(guarded) ? `"${guarded.replaceAll('"', '""')}"` : guarded
}
export function alignmentTsv(ledger) {
  const header = ['observation_id', 'subject_id', 'timepoint', 'excluded', 'exclusion_rules', ...I1_MODALITIES.flatMap(mod => [`${mod.key}_alias`, `${mod.key}_status`, `${mod.key}_source`])]
  const rows = ledger.rows.map(r => [r.observation, r.subject, r.timepoint, String(r.excluded), r.exclusions.join(' | '), ...I1_MODALITIES.flatMap(mod => [r.modalities[mod.key].alias, r.modalities[mod.key].status, r.modalities[mod.key].source])])
  return [header, ...rows].map(row => row.map(safeTsvCell).join('\t')).join('\n') + '\n'
}
