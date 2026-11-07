/**
 * Read-only view of the *corrected* feature tests that have already completed
 * in Compare Groups. This does not refit any statistics and does not infer
 * selected-hit ordination or cluster results from global PCA coordinates.
 */
export const ADDITIONAL_HIT_MODALITIES = Object.freeze([
  ['bcr', 'BCR repertoire'],
  ['tcrAlpha', 'TCR alpha'],
  ['tcrBeta', 'TCR beta'],
  ['isotype', 'BCR isotypes'],
  ['immune', 'Immune infiltration'],
])

function scientificNumber(value) {
  if (typeof value === 'number') return Number.isFinite(value) ? value : NaN
  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number(value)
    return Number.isFinite(parsed) ? parsed : NaN
  }
  return NaN
}

function adjustedRow(modality, family, test) {
  // No synthetic p/q/effect values are supplied when an adjusted test is absent.
  if (!test || typeof test !== 'object') return null
  if (typeof test.feature !== 'string' || !test.feature.trim()) return null
  return {
    modality,
    family,
    feature: test.feature,
    secondary_id: String(test.secondary_id ?? ''),
    adjusted_effect: scientificNumber(test.beta_group),
    q_value: scientificNumber(test.q_value),
    p_value: scientificNumber(test.p_value),
  }
}

export function collectAdjustedTests(compare) {
  const byModality = []
  for (const [key, label] of ADDITIONAL_HIT_MODALITIES) {
    const analysis = compare?.[key]?.analysis
    const rows = []
    if (analysis && typeof analysis === 'object') {
      if (key === 'bcr' || key === 'tcrAlpha' || key === 'tcrBeta') {
        for (const test of analysis.metric_tests || []) {
          const row = adjustedRow(label, 'Adjusted repertoire metrics', test)
          if (row) rows.push(row)
        }
        for (const composition of analysis.compositions || []) {
          const family = `Adjusted CLR composition: ${String(composition.family || 'unknown')}`
          for (const test of composition.tests || []) {
            const row = adjustedRow(label, family, test)
            if (row) rows.push(row)
          }
        }
      } else {
        const family = key === 'isotype' ? 'Adjusted isotype CLR' : 'Adjusted ssGSEA signature'
        for (const test of analysis.tests || []) {
          const row = adjustedRow(label, family, test)
          if (row) rows.push(row)
        }
      }
    }
    byModality.push({ key, label, available: Boolean(analysis), rows })
  }
  return byModality
}

export function selectAdjustedTests(rows, qThreshold, minAbsoluteEffect = 0) {
  const q = Number(qThreshold)
  const effect = Number(minAbsoluteEffect)
  if (!Number.isFinite(q) || q < 0 || q > 1) throw new Error('FDR must be between 0 and 1')
  if (!Number.isFinite(effect) || effect < 0) throw new Error('Minimum absolute adjusted effect must be non-negative')
  return rows.filter(row => Number.isFinite(row.q_value)
    && row.q_value >= 0 && row.q_value <= q
    && Number.isFinite(row.adjusted_effect)
    && Math.abs(row.adjusted_effect) >= effect)
}

function csvCell(value) {
  const text = Number.isFinite(value) ? String(value) : (value === null || value === undefined || Number.isNaN(value) ? '' : String(value))
  return /[\r\n",]/.test(text) ? `"${text.replaceAll('"','""')}"` : text
}

export function adjustedTestsCsv(rows) {
  const headers = ['modality', 'family', 'feature', 'secondary_id', 'adjusted_effect', 'q_value', 'p_value']
  return headers.join(',') + '\n' + rows.map(row => headers.map(key => csvCell(row[key])).join(',')).join('\n') + (rows.length ? '\n' : '')
}
