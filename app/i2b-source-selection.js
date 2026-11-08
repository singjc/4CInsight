/** I2B: local, identity-only source selection for unsupervised distances.
 * Never uses phenotype groups, adjusted hits, approximate aliases or imputation.
 */
import { I1_MODALITIES } from './multiomics-alignment.js?v=0.22.2'
const SOURCE_KEYS = new Set(I1_MODALITIES.map(mod => mod.key))
const exact = x => typeof x === 'string' ? x.trim() : ''

/** Returns independent per-view requests. File handles stay on the current device.
 * Only unexcluded, unambiguous I1 rows may contribute. A duplicate workbook is
 * an error for its view, not a reason to select an arbitrary source file.
 */
export function prepareI2BRequests(alignment, records, resolveFile, selectedMerged = {}, enabled = {}) {
  if (alignment?.schema !== '4c-insight-observation-alignment-v1' || !Array.isArray(alignment.rows)) {
    throw new Error('Check sample alignment before source-distance calculation')
  }
  if (!Array.isArray(records) || typeof resolveFile !== 'function') throw new Error('A local source inventory is required')
  const requests = [], errors = []
  for (const mod of I1_MODALITIES) {
    if (!SOURCE_KEYS.has(mod.key) || enabled?.[mod.key] === false) continue
    try {
      const included = alignment.rows.filter(row => !row.excluded && row.modalities?.[mod.key]?.status === 'present')
      const aliases = new Set()
      for (const row of included) {
        const alias = exact(row.modalities[mod.key].alias)
        if (!alias || aliases.has(alias)) throw new Error(`Duplicate or empty exact source alias '${alias}'`)
        aliases.add(alias)
      }
      if (included.length < 3) {
        errors.push({modality: mod.key, message: `Only ${included.length} aligned source observations (need at least three)`})
        continue
      }
      if (mod.mergedKind) {
        const selected = records.filter(r => r.kind === mod.mergedKind && r.id === selectedMerged[mod.key])
        if (selected.length !== 1) throw new Error('Select one merged expression table in Compare Groups')
        const file = resolveFile(selected[0].id)
        if (!file) throw new Error('Selected merged expression file is disconnected')
        requests.push({modality: mod.key, kind: mod.mergedKind,
          files: [{file, name: selected[0].name, sampleId: ''}],
          sampleIds: included.map(row => exact(row.modalities[mod.key].alias))})
        continue
      }
      const byAlias = new Map()
      for (const record of records) {
        if (record.kind !== mod.kind || !aliases.has(exact(record.sampleId))) continue
        const id = exact(record.sampleId)
        const existing = byAlias.get(id) || []
        existing.push(record)
        byAlias.set(id, existing)
      }
      const files = []
      for (const alias of aliases) {
        const matches = byAlias.get(alias) || []
        if (matches.length !== 1) throw new Error(`${alias}: expected one source file, found ${matches.length}`)
        const file = resolveFile(matches[0].id)
        if (!file) throw new Error(`${alias}: source file is disconnected`)
        files.push({file, name: matches[0].name, sampleId: alias})
      }
      requests.push({modality: mod.key, kind: mod.kind, files})
    } catch (error) {
      errors.push({modality: mod.key, message: error instanceof Error ? error.message : String(error)})
    }
  }
  return {requests, errors}
}
