/** I2A bridge: convert full unsupervised W4 distances to exact I1 observation IDs.
 * No use of corrected group coefficients, q-values, phenotype labels, or PCA axes.
 */
import { I1_MODALITIES } from './multiomics-alignment.js?v=0.22.1'

const NAMES = Object.freeze({
  'Gene expression': 'gene',
  'Transcript / isoform expression': 'transcript',
  'Transcript expression': 'transcript',
  BCR: 'bcr', 'BCR repertoire': 'bcr',
  'TCR alpha': 'tcrAlpha', 'TCR beta': 'tcrBeta',
  'BCR isotype': 'isotype', 'BCR isotypes': 'isotype',
  'Immune infiltration': 'immune',
})
const validKeys = new Set(I1_MODALITIES.map(item => item.key))
const trimmed = value => typeof value === 'string' ? value.trim() : ''

function normalizedView(key, source) {
  if (!validKeys.has(key)) throw new Error(`Unknown distance data type '${key}'`)
  if (!source || !Array.isArray(source.pairwise_distances)) throw new Error(`${key} has no full pairwise distance matrix`)
  const method = trimmed(source.distance_label)
  if (!method) throw new Error(`${key} has no distance method label`)
  return {
    modality: key,
    distance_label: method,
    pairs: source.pairwise_distances.map((pair, i) => {
      const a = trimmed(pair?.sample_a ?? pair?.a)
      const b = trimmed(pair?.sample_b ?? pair?.b)
      const d = pair?.distance
      if (!a || !b || a === b || typeof d !== 'number' || !Number.isFinite(d) || d < 0) {
        throw new Error(`${key} has an invalid distance at pair ${i + 1}`)
      }
      return { a, b, distance: d }
    }),
  }
}

/** Accept the full native W4 result, browser W4 result, or a named single view.
 * A view contains true source pairwise distances, not a 2D PCA/PCoA projection.
 */
export function extractFullDistanceViews(value) {
  const root = value?.results?.longitudinal || value?.result || value
  if (!root || typeof root !== 'object') throw new Error('Select a longitudinal distance JSON file')
  const output = []
  if (Array.isArray(root.modalities)) {
    for (const view of root.modalities) {
      const key = NAMES[view?.modality] || (validKeys.has(view?.modality) ? view.modality : '')
      if (key) output.push(normalizedView(key, view))
    }
  } else if (Array.isArray(root.views)) {
    for (const view of root.views) {
      const key = view?.modality || view?.key
      output.push(normalizedView(key, { ...view, pairwise_distances: view.pairwise_distances ?? view.pairs }))
    }
  } else {
    for (const modality of I1_MODALITIES) {
      const candidate = root[modality.key]
      if (candidate) output.push(normalizedView(modality.key, candidate.modality && typeof candidate.modality === 'object' ? candidate.modality : candidate))
    }
    if (!output.length && root.modality) {
      const key = NAMES[root.modality] || (validKeys.has(root.modality) ? root.modality : '')
      if (key) output.push(normalizedView(key, root))
    }
  }
  if (!output.length) throw new Error('No complete pairwise-distance results were found. Corrected group results or PCoA plots cannot be used for I2.')
  const seen = new Set()
  for (const view of output) {
    if (seen.has(view.modality)) throw new Error(`Duplicate full-distance data type: ${view.modality}`)
    seen.add(view.modality)
  }
  return output
}

/** Translate source sample IDs to canonical observations with exact I1 aliases.
 * Metadata-excluded, absent and ambiguous source rows are never included.
 * Invalid distance types cause an error. Unmatched source pairs are counted.
 */
export function prepareJointDistanceInput(alignment, sourceViews, { minViews = 2 } = {}) {
  if (alignment?.schema !== '4c-insight-observation-alignment-v1' || !Array.isArray(alignment.rows)) {
    throw new Error('Check sample alignment with current metadata before distance fusion')
  }
  if (!Array.isArray(sourceViews)) throw new Error('No distance views were supplied')
  const retained = alignment.rows.filter(row => !row.excluded)
  const ids = new Set(retained.map(row => row.observation))
  if (ids.size !== retained.length) throw new Error('I1 observation IDs are not unique')
  const diagnostics = []
  const included = []
  const used = new Set()
  for (const view of sourceViews) {
    const key = view.modality
    if (!validKeys.has(key) || used.has(key)) throw new Error(`Unknown or duplicate distance view ${key}`)
    used.add(key)
    const owners = new Map()
    for (const row of retained) {
      const status = row.modalities?.[key]
      if (status?.status !== 'present') continue
      const alias = trimmed(status.alias)
      if (!alias) throw new Error(`Empty ${key} alias in I1 alignment`)
      if (owners.has(alias)) throw new Error(`Ambiguous ${key} source ID '${alias}' in alignment`)
      owners.set(alias, row.observation)
    }
    const seenPairs = new Set()
    let unmatched = 0
    const pairs = []
    for (const pair of view.pairs || []) {
      const a = owners.get(pair.a)
      const b = owners.get(pair.b)
      if (!a || !b) { unmatched++; continue }
      if (a === b) throw new Error(`Self/aliased pair in ${key} for '${a}'`)
      if (typeof pair.distance !== 'number' || !Number.isFinite(pair.distance) || pair.distance < 0) throw new Error(`Invalid ${key} source distance`)
      const pairKey = JSON.stringify([a,b].sort())
      if (seenPairs.has(pairKey)) throw new Error(`Duplicate ${key} source pair ${pairKey}`)
      seenPairs.add(pairKey)
      pairs.push({ a, b, distance: pair.distance })
    }
    diagnostics.push({ modality: key, sourcePairs: view.pairs.length, matchedPairs: pairs.length, excludedOrUnmatchedPairs: unmatched })
    if (pairs.length >= 2) included.push({ modality: key, distance_label: view.distance_label, pairs })
  }
  if (included.length < 2) throw new Error('At least two aligned views with two or more valid pairs each are required')
  if (!Number.isInteger(minViews) || minViews < 2 || minViews > included.length) throw new Error('Minimum view coverage is outside the available data types')
  const observations = retained.map(row => row.observation).filter(id => ids.has(id))
  return {
    input: { observations, views: included, min_views: minViews },
    diagnostics,
    omittedViews: sourceViews.length - included.length,
  }
}

/** Complete export. No plot-thinning, row cap or q-value selection. */
export function jointDistancesTsv(result) {
  const protect = value => {
    const text = String(value ?? '')
    const safe = /^[=+@\-\t\r]/.test(text) ? `'${text}` : text
    return /[\t\r\n"]/.test(safe) ? `"${safe.replaceAll('"','""')}"` : safe
  }
  const lines = [['observation_a','observation_b','fused_distance','contributing_views','source_modalities'].join('\t')]
  for (const pair of result.pairs || []) {
    lines.push([pair.a,pair.b,pair.distance,pair.contributing_views,(pair.modalities || []).join('|')].map(protect).join('\t'))
  }
  return lines.join('\n')+'\n'
}
