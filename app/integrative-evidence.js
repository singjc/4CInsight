/**
 * I0: read-only seven-modality evidence summary.
 * This combines labels and previously fitted evidence, NOT sample-level data.
 * The BH q-values remain those produced by each source analysis.
 */
import { collectAdjustedTests } from './extended-hit-review.js?v=0.21.0-i2b-source-distances'

export const INTEGRATIVE_MODALITIES = Object.freeze([
  ['gene', 'Gene expression'],
  ['transcript', 'Transcript expression'],
  ['bcr', 'BCR repertoire'],
  ['tcrAlpha', 'TCR alpha'],
  ['tcrBeta', 'TCR beta'],
  ['isotype', 'BCR isotypes'],
  ['immune', 'Immune infiltration'],
])

const isScientificNumber = (value) =>
  (typeof value === 'number' && Number.isFinite(value)) ||
  (typeof value === 'string' && value.trim() !== '' && Number.isFinite(Number(value)))

function selectedAdjustedTest(row, fdr, effectMin) {
  return typeof row?.feature === 'string' && row.feature.trim().length > 0
    && isScientificNumber(row?.q_value) && Number(row.q_value) >= 0 && Number(row.q_value) <= fdr
    && isScientificNumber(row?.beta_group) && Math.abs(Number(row.beta_group)) >= effectMin
}

export function summarizeIntegrativeEvidence(compare, thresholds = {}, enabled = {}) {
  const q = Number(thresholds.fdr)
  const fdr = Number.isFinite(q) && q >= 0 && q <= 1 ? q : 0.05
  const expressionMin = Number(thresholds.absLog2Fc)
  const log2Min = Number.isFinite(expressionMin) && expressionMin >= 0 ? expressionMin : 0
  const omicsMinInput = Number(thresholds.omicsMinAdjustedEffect)
  const omicsMin = Number.isFinite(omicsMinInput) && omicsMinInput >= 0 ? omicsMinInput : 0
  const others = new Map(collectAdjustedTests(compare).map(g => [g.key, g]))
  return INTEGRATIVE_MODALITIES.map(([key, label]) => {
    const analysis = compare?.[key]?.analysis || null
    const available = Boolean(analysis)
    const included = available && enabled?.[key] !== false
    const isExpression = key === 'gene' || key === 'transcript'
    const rows = isExpression ? (Array.isArray(analysis?.tests) ? analysis.tests : []) : (others.get(key)?.rows || [])
    const selected = isExpression
      ? rows.filter(row => selectedAdjustedTest(row, fdr, log2Min)).length
      : rows.filter(row => isScientificNumber(row.q_value) && Number(row.q_value) >= 0 && Number(row.q_value) <= fdr
          && isScientificNumber(row.adjusted_effect) && Math.abs(Number(row.adjusted_effect)) >= omicsMin).length
    const n = Number(analysis?.n_model_samples ?? analysis?.files_used ?? analysis?.sample_summaries?.length)
    return {
      key, label, available, included,
      tested: rows.length, selected,
      samples: available && Number.isFinite(n) && n >= 0 ? n : null,
      effectType: isExpression ? 'log2 count-model effect' : 'Modality-specific adjusted effect',
    }
  })
}

export function collectIntegrativeSelectedRows(compare, thresholds = {}, enabled = {}) {
  const fdrInput = Number(thresholds.fdr)
  const fdr = Number.isFinite(fdrInput) && fdrInput >= 0 && fdrInput <= 1 ? fdrInput : 0.05
  const expressionMinInput = Number(thresholds.absLog2Fc)
  const expressionMin = Number.isFinite(expressionMinInput) && expressionMinInput >= 0 ? expressionMinInput : 0
  const omicsMinInput = Number(thresholds.omicsMinAdjustedEffect)
  const omicsMin = Number.isFinite(omicsMinInput) && omicsMinInput >= 0 ? omicsMinInput : 0
  const otherGroups = new Map(collectAdjustedTests(compare).map(group => [group.key, group]))
  const chosen = []
  for (const [key, label] of INTEGRATIVE_MODALITIES) {
    if (enabled?.[key] === false) continue
    if (key === 'gene' || key === 'transcript') {
      const tests = compare?.[key]?.analysis?.tests || []
      for (const test of tests) {
        if (!selectedAdjustedTest(test, fdr, expressionMin)) continue
        chosen.push({
          modality: label,
          family: key === 'gene' ? 'Adjusted gene expression (log2 effect)' : 'Adjusted transcript expression (log2 effect)',
          feature: String(test.feature ?? ''), secondary_id: String(test.secondary_id ?? ''),
          adjusted_effect: Number(test.beta_group), q_value: Number(test.q_value),
          p_value: isScientificNumber(test.p_value) ? Number(test.p_value) : null,
        })
      }
    } else {
      for (const row of otherGroups.get(key)?.rows || []) {
        if (!(isScientificNumber(row.q_value) && Number(row.q_value) >= 0 && Number(row.q_value) <= fdr
          && isScientificNumber(row.adjusted_effect) && Math.abs(Number(row.adjusted_effect)) >= omicsMin)) continue
        chosen.push({...row})
      }
    }
  }
  return chosen
}

export function integrativeSummaryTotals(rows) {
  const chosen = rows.filter(row => row.included)
  return {
    included: chosen.length,
    available: chosen.filter(row => row.available).length,
    adjustedTests: chosen.reduce((sum, row) => sum + (row.available ? row.tested : 0), 0),
    selectedTests: chosen.reduce((sum, row) => sum + (row.available ? row.selected : 0), 0),
  }
}
