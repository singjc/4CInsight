import { multiomicsVisualDefinitions } from './docs-new-modalities.js?v=0.21.0-ux14-input-guide-modality-first'
// Visual teaching aids for the illustrative examples in the 4C Insight Docs.
// These diagrams are not results from your cohort.
// Keep their numeric labels and claims consistent with docs-toy-examples.js.

const C = Object.freeze({
  ink: '#24334d', muted: '#697992', grid: '#dce5f0', faint: '#f5f8fd',
  blue: '#3159d9', paleBlue: '#e2ebff', red: '#c44252', paleRed: '#fde8eb',
  teal: '#16856b', paleTeal: '#e5f7f0', amber: '#a96b18', paleAmber: '#fff2db',
  gray: '#b8c4d2', white: '#ffffff',
})

const escapeSvg = (s) => String(s ?? '').replace(/&/g, '&amp;')
  .replace(/</g, '&lt;').replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;').replace(/'/g, '&#39;')
const num = (value) => Number(value).toFixed(1).replace(/\.0$/, '')
const rect = (x, y, w, h, fill = C.faint, rx = 7, stroke = 'none', strokeWidth = 1) =>
  `<rect x="${num(x)}" y="${num(y)}" width="${num(w)}" height="${num(h)}" rx="${rx}" fill="${fill}" stroke="${stroke}" stroke-width="${strokeWidth}"/>`
const line = (x1, y1, x2, y2, color = C.grid, width = 1.5, dash = '') =>
  `<line x1="${num(x1)}" y1="${num(y1)}" x2="${num(x2)}" y2="${num(y2)}" stroke="${color}" stroke-width="${width}" ${dash ? `stroke-dasharray="${dash}"` : ''}/>`
const dot = (x, y, r, color, stroke = C.white) =>
  `<circle cx="${num(x)}" cy="${num(y)}" r="${num(r)}" fill="${color}" stroke="${stroke}" stroke-width="2"/>`
const text = (x, y, s, opts = {}) =>
  `<text x="${num(x)}" y="${num(y)}" text-anchor="${opts.anchor || 'start'}" font-family="system-ui, sans-serif" font-size="${opts.size || 12}" font-weight="${opts.weight || 500}" fill="${opts.color || C.ink}">${escapeSvg(s)}</text>`
const arrow = (x1, y1, x2, y2, color = C.blue) => {
  const a = Math.atan2(y2 - y1, x2 - x1)
  const head = 8
  const px = x2 - head * Math.cos(a - Math.PI / 6)
  const py = y2 - head * Math.sin(a - Math.PI / 6)
  const qx = x2 - head * Math.cos(a + Math.PI / 6)
  const qy = y2 - head * Math.sin(a + Math.PI / 6)
  return `${line(x1, y1, x2, y2, color, 2.5)}<path d="M${num(px)} ${num(py)} L${num(x2)} ${num(y2)} L${num(qx)} ${num(qy)}" fill="none" stroke="${color}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>`
}
const tag = (x, y, value, color = C.blue, fill = C.paleBlue, w = 110) =>
  `${rect(x, y, w, 25, fill, 8)}${text(x + w / 2, y + 17, value, { size: 11, color, weight: 700, anchor: 'middle' })}`
const panel = (x, y, w, h, title) =>
  `${rect(x, y, w, h, C.white, 10, C.grid)}${text(x + 15, y + 25, title, { size: 13, weight: 700 })}`

function canvas(id, title, desc, content, height = 250) {
  const safeId = id.replace(/[^a-z0-9-]/gi, '-')
  return `<div class="docs-visual-overflow"><svg class="docs-visual-svg" viewBox="0 0 680 ${height}" role="img" aria-labelledby="visual-title-${safeId} visual-desc-${safeId}" xmlns="http://www.w3.org/2000/svg">
    <title id="visual-title-${safeId}">${escapeSvg(title)}</title>
    <desc id="visual-desc-${safeId}">${escapeSvg(desc)}</desc>
    ${rect(0, 0, 680, height, C.faint, 12)}${content}
  </svg></div>`
}

function stateDiagram(id) {
  let s = panel(15, 48, 175, 138, 'Local source files')
  s += text(30, 104, 'counts.xlsx', { weight: 700 }) + text(30, 130, 'metadata.tsv')
  s += tag(27, 151, 'Raw bytes stay here', C.teal, C.paleTeal, 149)
  s += arrow(195, 113, 246, 113)
  s += panel(252, 48, 174, 138, 'Saved state JSON')
  s += text(266, 103, 'Settings + file paths') + text(266, 128, 'No raw data')
  s += arrow(432, 113, 481, 113)
  s += panel(487, 48, 178, 138, 'Load saved state')
  s += text(501, 103, 'Reconnect folder') + text(501, 128, 'Match files again')
  s += tag(232, 203, 'Permission required', C.amber, C.paleAmber, 210)
  return canvas(id, 'How saved states reconnect local files', 'File access stays on the device. The state JSON does not contain raw files. Reconnecting requires user permission.', s)
}
function mergeDiagram(id) {
  let s = panel(12, 37, 195, 177, 'S01.xlsx · reference')
  s += panel(235, 37, 195, 177, 'S02.xlsx · later file')
  for (const [i, label, v1, v2] of [[0, 'G1', '10', '20'], [1, 'G2', '30', '50']]) {
    const y = 91 + i * 42
    s += rect(23, y - 20, 172, 32, C.paleBlue) + text(38, y, label, { weight: 700 }) + text(181, y, v1, { anchor: 'end' })
    s += rect(246, y - 20, 172, 32, C.paleBlue) + text(262, y, label, { weight: 700 }) + text(405, y, v2, { anchor: 'end' })
  }
  s += rect(246, 164, 172, 32, C.paleAmber) + text(262, 185, 'G3 (extra)') + text(405, 185, '7', { anchor: 'end' })
  s += arrow(434, 129, 469, 129)
  s += panel(475, 37, 192, 177, 'Merged TSV')
  s += text(486, 90, 'Gene    S01    S02', { size: 11, weight: 700 })
  s += text(486, 124, 'G1         10       20') + text(486, 155, 'G2         30       50')
  s += text(487, 190, 'G3: report extra row', { size: 11, color: C.amber })
  return canvas(id, 'Two sample workbooks become one expression table', 'Feature IDs from the first sorted workbook define output rows; G3, found only in S02, is reported rather than added.', s)
}
function metadataDiagram(id) {
  let s = panel(15, 42, 255, 176, 'Biological metadata') + panel(408, 42, 257, 176, 'Expression file identities')
  s += tag(30, 89, 'PatientA_T0', C.blue, C.paleBlue, 215)
  s += tag(30, 152, 'PatientA_T1', C.blue, C.paleBlue, 215)
  s += tag(424, 89, 'GX_001', C.teal, C.paleTeal, 224)
  s += tag(424, 152, 'GX_002', C.teal, C.paleTeal, 224)
  s += arrow(280, 101, 400, 101, C.teal) + arrow(280, 164, 400, 164, C.teal)
  s += text(336, 87, 'map', { color: C.muted, anchor: 'middle', size: 11 })
  s += text(336, 151, 'map', { color: C.muted, anchor: 'middle', size: 11 })
  return canvas(id, 'Map file IDs to stable sample IDs', 'A mapping links GX_001 and GX_002 to biological observations, without renaming their source files.', s)
}
function qcDiagram(id) {
  let s = panel(14, 35, 460, 193, 'Observed count distribution')
  const left = 87, end = 423, scale = (v) => left + v / 20 * (end - left)
  for (const k of [0, 5, 10, 15, 20]) {
    s += line(scale(k), 86, scale(k), 184, C.grid, 1) + text(scale(k), 204, k, { anchor: 'middle', size: 10, color: C.muted })
  }
  s += text(78, 115, 'A', { anchor: 'end', weight: 700 }) + text(78, 166, 'B', { anchor: 'end', weight: 700 })
  s += line(left, 110, end, 110, C.gray) + line(left, 161, end, 161, C.gray)
  for (const v of [0, 10, 20]) s += dot(scale(v), 110, 7, C.blue)
  for (const v of [5, 10, 15, 20]) s += dot(scale(v), 161, 7, C.teal)
  s += panel(486, 35, 180, 193, 'Sample A flags')
  s += tag(497, 85, '1 missing / 4', C.amber, C.paleAmber, 157)
  s += tag(497, 127, '1 zero / 3 seen', C.blue, C.paleBlue, 157)
  s += text(503, 187, 'Review, do not auto-drop', { color: C.muted, size: 11 })
  return canvas(id, 'Compare observed counts and QC flags', 'Sample A observed counts are 0,10,20 and one missing entry; Sample B counts are 5,10,15,20. Missingness and zeros use different denominators.', s)
}
function normalizationDiagram(id) {
  let s = panel(11, 33, 323, 193, 'Before: raw counts') + panel(346, 33, 323, 193, 'After: median-ratio correction')
  const drawBars = (x, valuesA, valuesB, max) => {
    let r = ''
    const base = 179, span = 107
    for (let i = 0; i < 3; i++) {
      const gx = x + 51 + i * 87
      const ha = valuesA[i] / max * span, hb = valuesB[i] / max * span
      r += rect(gx, base - ha, 24, ha, C.blue, 2) + rect(gx + 27, base - hb, 24, hb, C.red, 2)
      r += text(gx + 26, 197, `G${i + 1}`, { anchor: 'middle', size: 11 })
    }
    return r + line(x + 28, base, x + 292, base, C.muted)
  }
  s += drawBars(11, [10, 20, 40], [20, 40, 80], 85)
  s += drawBars(346, [14.142, 28.284, 56.569], [14.142, 28.284, 56.569], 60)
  s += rect(170, 239, 12, 12, C.blue, 2) + text(188, 250, 'Sample A', { size: 11 })
  s += rect(295, 239, 12, 12, C.red, 2) + text(313, 250, 'Sample B', { size: 11 })
  s += text(424, 250, 'Paired bars now match', { size: 11, color: C.teal, weight: 700 })
  return canvas(id, 'Median-ratio normalization corrects for depth', 'Before normalization B has double A counts for all three genes; after dividing by size factors 0.707 and 1.414, values align.', s, 267)
}
function effectDiagram(id) {
  let s = panel(35, 37, 610, 185, 'Illustrative model-adjusted group means')
  s += line(105, 177, 560, 177, C.muted)
  s += rect(179, 121, 104, 56, C.blue) + rect(365, 65, 104, 112, C.red)
  s += text(231, 115, '10', { anchor: 'middle', weight: 800, size: 18 }) + text(417, 60, '20', { anchor: 'middle', weight: 800, size: 18 })
  s += text(231, 199, 'Control', { anchor: 'middle', size: 12 }) + text(417, 199, 'Case', { anchor: 'middle', size: 12 })
  s += tag(491, 110, '2× ratio', C.teal, C.paleTeal, 123)
  s += tag(491, 143, 'log2FC = +1', C.teal, C.paleTeal, 123)
  return canvas(id, 'Read a model effect as a fold change', 'Means 10 and 20 yield a case-versus-control log2 effect of +1. These are illustrative adjusted means, not fitted experimental data.', s)
}
function bhDiagram(id) {
  let s = panel(12, 29, 656, 204, 'Multiple testing: raw p versus adjusted q')
  const raw = [0.001, 0.02, 0.04, 0.2], adjusted = [0.004, 0.04, 0.0533333, 0.2]
  const xp = (i) => 106 + i * 160, yp = (v) => 190 - v / 0.22 * 118
  s += line(91, 190, 602, 190, C.muted) + line(91, 58, 91, 190, C.muted)
  s += line(91, yp(0.05), 602, yp(0.05), C.amber, 1.6, '5 4') + text(606, yp(0.05) + 4, '0.05', { size: 10, color: C.amber })
  for (let i = 0; i < 4; i++) {
    s += line(xp(i), yp(raw[i]), xp(i), yp(adjusted[i]), C.grid, 3)
    s += dot(xp(i) - 7, yp(raw[i]), 5.3, C.blue) + dot(xp(i) + 7, yp(adjusted[i]), 6, i < 2 ? C.teal : C.red)
    s += text(xp(i), 209, `rank ${i + 1}`, { size: 11, anchor: 'middle' })
  }
  s += rect(160, 239, 11, 11, C.blue, 2) + text(177, 250, 'raw p', { size: 11 })
  s += rect(300, 239, 11, 11, C.teal, 2) + text(317, 250, 'q ≤ 0.05', { size: 11 })
  s += rect(452, 239, 11, 11, C.red, 2) + text(469, 250, 'q > 0.05', { size: 11 })
  return canvas(id, 'BH adjustment across four tests', 'The q-values for ranks 1 and 2 are below 0.05, while ranks 3 and 4 are above the FDR cutoff.', s, 262)
}

// Local teaching controls only. They never change Compare Groups model fitting.
export function sanitizeToyThresholds(input = {}) {
  const rawFdr = Number(input.fdr)
  const rawEffect = Number(input.effect)
  return {
    fdr: Number.isFinite(rawFdr) ? Math.min(0.20, Math.max(0.01, rawFdr)) : 0.05,
    effect: Number.isFinite(rawEffect) ? Math.min(2.5, Math.max(0, rawEffect)) : 1,
  }
}
export function classifyToyVolcano(input = {}) {
  const { fdr, effect } = sanitizeToyThresholds(input)
  const points = [
    { name: 'A', effect: 1.5, q: 0.004 },
    { name: 'B', effect: -0.8, q: 0.040 },
    { name: 'C', effect: -2.0, q: 0.080 },
  ]
  return points.map((p) => ({
    ...p, classification: p.q <= fdr && p.effect >= effect ? 'up'
      : p.q <= fdr && p.effect <= -effect ? 'down' : 'neutral',
  }))
}
export function renderToyVolcano(input = {}) {
  const { fdr, effect } = sanitizeToyThresholds(input)
  const points = classifyToyVolcano({ fdr, effect })
  const x = (v) => 344 + v * 104, y = (q) => 198 - (-Math.log10(q)) / 3 * 143
  let s = panel(12, 24, 656, 201, 'Volcano: model effect versus adjusted q')
  const top = 55, bottom = 198, left = 83, right = 619
  for (const tick of [-2, -1, 0, 1, 2]) {
    s += line(x(tick), top, x(tick), bottom, C.grid, 1) + text(x(tick), 214, tick, { anchor: 'middle', size: 10 })
  }
  for (const tick of [0, 1, 2, 3]) s += text(left - 12, 202 - tick * (143 / 3), tick, { anchor: 'end', size: 10, color: C.muted })
  s += line(left, bottom, right, bottom, C.muted) + line(left, top, left, bottom, C.muted)
  s += line(x(0), top, x(0), bottom, C.gray, 1.2, '3 4')
  s += line(x(effect), top, x(effect), bottom, C.amber, 1.8, '6 4')
  s += line(x(-effect), top, x(-effect), bottom, C.amber, 1.8, '6 4')
  s += line(left, y(fdr), right, y(fdr), C.amber, 1.8, '6 4')
  s += text(621, y(fdr) - 6, `q = ${fdr.toFixed(2)}`, { anchor: 'end', size: 10, color: C.amber })
  for (const p of points) {
    const col = p.classification === 'up' ? C.red : p.classification === 'down' ? C.blue : C.gray
    s += dot(x(p.effect), y(p.q), 8, col)
    s += text(x(p.effect) + 13, y(p.q) - 7, p.name, { weight: 800, size: 13, color: C.ink })
  }
  s += text(346, 241, 'Adjusted log2 group effect', { anchor: 'middle', size: 11 })
  s += text(24, 120, '−log10(q)', { size: 11, color: C.muted })
  return canvas('fdr-volcano-1', 'Interactive toy volcano plot', 'Three toy features A B C; dashed boundaries show adjustable FDR and effect thresholds. Red means up; blue means down; gray means below at least one cutoff.', s, 257)
}
export function volcanoSummary(input = {}) {
  const pts = classifyToyVolcano(input)
  const up = pts.filter((p) => p.classification === 'up').length
  const down = pts.filter((p) => p.classification === 'down').length
  return `${up + down} of ${pts.length} pass both cutoffs (${up} up, ${down} down)`
}
function pcaDiagram(id) {
  let s = panel(13, 31, 323, 203, 'Raw feature coordinates') + panel(347, 31, 320, 203, 'Projection onto PC1')
  const points = [{ id: 'S1', v: 2 }, { id: 'S2', v: 3 }, { id: 'S3', v: 8 }, { id: 'S4', v: 9 }]
  const fx = (v) => 60 + v * 24, fy = (v) => 209 - v * 15.3
  s += line(60, 207, 306, 50, C.amber, 2.5, '5 3')
  s += text(133, 64, 'PC1 direction', { size: 11, color: C.amber })
  for (const p of points) {
    s += dot(fx(p.v), fy(p.v), 7, p.v < 5 ? C.blue : C.teal)
    s += text(fx(p.v) + 9, fy(p.v) - 6, p.id, { size: 11, weight: 700 })
  }
  s += line(395, 156, 620, 156, C.muted, 1.6)
  for (const p of points) {
    const pc1 = (p.v - 5.5) * Math.SQRT2
    const xx = 510 + pc1 * 19
    s += dot(xx, 156, 7, p.v < 5 ? C.blue : C.teal)
    s += text(xx, 136, p.id, { anchor: 'middle', size: 11, weight: 700 })
  }
  s += text(510, 209, 'PC1: 100% toy variance', { anchor: 'middle', size: 12, color: C.teal })
  return canvas(id, 'PCA rotates the data to its main direction', 'Four sample points lie on a diagonal in the original two-feature space; projecting onto PC1 retains all toy variation.', s)
}
function clusteringDiagram(id) {
  let s = panel(34, 28, 614, 205, 'Average-linkage hierarchical clustering')
  const left = 140, scale = (distance) => left + 45 * distance
  const ys = [79, 118, 157, 196]
  for (let i = 0; i < 4; i++) s += text(100, ys[i] + 5, `S${i + 1}`, { size: 12, weight: 700 })
  const nearX = scale(Math.SQRT2), finalX = scale(6 * Math.SQRT2)
  s += line(left, ys[0], nearX, ys[0], C.blue, 3) + line(left, ys[1], nearX, ys[1], C.blue, 3)
  s += line(nearX, ys[0], nearX, ys[1], C.blue, 3)
  s += line(left, ys[2], nearX, ys[2], C.teal, 3) + line(left, ys[3], nearX, ys[3], C.teal, 3)
  s += line(nearX, ys[2], nearX, ys[3], C.teal, 3)
  const mid1 = (ys[0] + ys[1]) / 2, mid2 = (ys[2] + ys[3]) / 2
  s += line(nearX, mid1, finalX, mid1, C.amber, 3) + line(nearX, mid2, finalX, mid2, C.amber, 3)
  s += line(finalX, mid1, finalX, mid2, C.amber, 3)
  s += text(nearX - 6, 63, '1.41', { anchor: 'middle', size: 11, color: C.blue })
  s += text(finalX, 63, '8.49', { anchor: 'middle', size: 11, color: C.amber })
  s += text(329, 221, 'Distance at each merge', { size: 11, color: C.muted })
  return canvas(id, 'Dendrogram built from Euclidean distances', 'Samples S1-S2 and S3-S4 join at distance 1.41; average-linkage joins the two clusters at distance 8.49.', s)
}
function transcriptDiagram(id) {
  let s = panel(29, 32, 622, 199, 'Two transcript mixtures, same gene total')
  const xs = [165, 436], width = 87, base = 193, full = 100
  for (let i = 0; i < xs.length; i++) {
    const frac = i === 0 ? 0.9 : 0.1
    const blueH = full * frac, amberH = full - blueH
    s += rect(xs[i], base - blueH, width, blueH, C.blue, 2)
    s += rect(xs[i], base - full, width, amberH, C.amber, 2)
    s += text(xs[i] + width / 2, 215, `Sample ${i ? 'B' : 'A'}`, { anchor: 'middle', weight: 700 })
    s += text(xs[i] + width / 2, 82, 'Total = 10', { anchor: 'middle', size: 12 })
    s += tag(xs[i] - 20, 224, `CLR(T1) ${i ? '−' : '+'}0.923`, C.teal, C.paleTeal, 127)
  }
  s += rect(305, 95, 12, 12, C.blue, 2) + text(325, 105, 'T1', { size: 11 })
  s += rect(305, 121, 12, 12, C.amber, 2) + text(325, 131, 'T2', { size: 11 })
  return canvas(id, 'Transcript use can change without gene count changing', 'Sample A has T1=9 T2=1; Sample B has T1=1 T2=9. Both total 10 reads; the fractions and CLR values reverse.', s, 264)
}
function longitudinalDiagram(id) {
  let s = panel(19, 30, 641, 204, 'Eight same-versus-different distance comparisons')
  const vals = [0.8, 1.0, 1.2, 1.4], same = [0.2, 1.3]
  for (let j = 0; j < 4; j++) s += text(242 + j * 100, 93, vals[j].toFixed(1), { anchor: 'middle', size: 12, weight: 700 })
  s += text(245, 74, 'Different-patient distances', { size: 11, color: C.muted })
  for (let i = 0; i < 2; i++) {
    s += text(100, 132 + i * 65, `Same ${same[i].toFixed(1)}`, { size: 12, weight: 700 })
    for (let j = 0; j < 4; j++) {
      const win = same[i] < vals[j], x = 199 + j * 100, y = 106 + i * 65
      s += rect(x, y, 86, 46, win ? C.paleTeal : C.paleAmber, 7, win ? C.teal : '#e5ba74')
      s += text(x + 43, y + 28, win ? 'Closer ✓' : 'Not closer', { anchor: 'middle', color: win ? C.teal : C.amber, size: 11, weight: 700 })
    }
  }
  s += tag(470, 242, 'AUC = 5/8 = 0.625', C.teal, C.paleTeal, 175)
  return canvas(id, 'How patient-identity AUC ranks distances', 'Five of eight pairwise comparisons place a same-patient distance below a different-patient distance. The toy AUC is 0.625.', s, 277)
}
function significantDiagram(id) {
  let s = panel(36, 29, 608, 204, 'Illustrative post-selection heatmap')
  const genes = ['G1', 'G2', 'G3'], values = [[-1, -0.8, 0.7, 1], [-0.7, -1, 0.9, 0.8], [0.3, -0.3, 0.2, -0.2]]
  const xs = [202, 284, 366, 448]
  for (let j = 0; j < 4; j++) s += text(xs[j] + 32, 80, ['Ctrl1', 'Ctrl2', 'Case1', 'Case2'][j], { anchor: 'middle', size: 11 })
  for (let i = 0; i < 3; i++) {
    const y = 89 + i * 40
    s += text(103, y + 24, genes[i], { weight: 700, size: 12 })
    s += tag(127, y + 5, i < 2 ? 'selected' : 'excluded', i < 2 ? C.teal : C.amber, i < 2 ? C.paleTeal : C.paleAmber, 71)
    for (let j = 0; j < 4; j++) {
      const v = values[i][j]
      s += rect(xs[j], y, 68, 33, v > 0 ? C.paleRed : C.paleBlue, 3)
      s += text(xs[j] + 34, y + 22, v.toFixed(1), { anchor: 'middle', size: 11, color: v > 0 ? C.red : C.blue })
    }
  }
  s += text(350, 223, 'Same labels used to select G1/G2 and show separation', { anchor: 'middle', color: C.amber, size: 11 })
  return canvas(id, 'Feature selection can exaggerate separation in-sample', 'G1 and G2 pass illustrative thresholds and have a clean-looking selected heatmap, but this is not independent predictive validation.', s)
}
function exclusionDiagram(id) {
  let s = panel(17, 33, 303, 187, 'Before exclusion · four samples')
  s += panel(384, 33, 279, 187, 'After user-selected rule')
  for (let i = 0; i < 4; i++) {
    const color = i === 1 ? C.paleRed : C.paleTeal
    s += rect(33 + i * 71, 104, 64, 47, color)
    s += text(65 + i * 71, 125, `S0${i + 1}`, { anchor: 'middle', weight: 700, size: 11 })
    s += text(65 + i * 71, 142, i === 1 ? 'Failed' : 'Pass', { anchor: 'middle', size: 10, color: i === 1 ? C.red : C.teal })
    if (i !== 1) {
      const j = i === 0 ? 0 : i - 1
      s += rect(401 + j * 80, 104, 70, 47, C.paleTeal)
      s += text(436 + j * 80, 132, `S0${i + 1}`, { anchor: 'middle', size: 11, color: C.teal, weight: 700 })
    }
  }
  s += arrow(326, 124, 373, 124, C.amber)
  s += tag(174, 172, 'QC_status = Failed', C.amber, C.paleAmber, 192)
  return canvas(id, 'A QC exclusion removes an observation before fitting', 'The user chooses QC_status Failed; S02 is excluded, while S01 S03 and S04 remain. The exclusion is recorded for audit.', s)
}
function performanceDiagram(id) {
  let s = panel(24, 30, 632, 204, 'Device CPU information')
  for (let i = 0; i < 20; i++) {
    const x = 53 + (i % 10) * 56, y = 87 + Math.floor(i / 10) * 41
    s += rect(x, y, 43, 29, i === 0 ? C.paleTeal : C.paleBlue, 5, i === 0 ? C.teal : C.grid)
    s += text(x + 21.5, y + 19, i + 1, { anchor: 'middle', size: 11, color: i === 0 ? C.teal : C.muted })
  }
  s += text(52, 72, '20 logical CPU threads reported', { size: 12, weight: 700 })
  s += rect(50, 181, 15, 15, C.paleTeal, 3, C.teal)
  s += text(73, 193, 'Analyses run one task at a time', { size: 12, weight: 700, color: C.teal })
  s += text(350, 193, '512 MiB shown: analysis memory', { size: 11, color: C.muted })
  return canvas(id, 'Device information and analysis memory', 'CPU counts do not guarantee parallel calculations. Analysis memory does not measure total or free device RAM.', s)
}

const chartDefinitions = {
  ...multiomicsVisualDefinitions,
  'local-first': [{ title: 'Saved state and local file reconnection', caption: 'Only references go into the JSON; the browser must reconnect the source folder.', render: stateDiagram }],
  'file-merging': [{ title: 'Two workbooks to one expression table', caption: 'Rows match by feature ID. An extra feature is reported, not silently merged.', render: mergeDiagram }],
  'sample-metadata': [{ title: 'Sample-to-file mapping', caption: 'A biological sample ID and a source file ID can be different.', render: metadataDiagram }],
  'expression-qc': [{ title: 'Counts, missingness, and zeros', caption: 'Missing values and measured zeros are different QC signals.', render: qcDiagram }],
  'normalization-nb2': [
    { title: 'Before and after median-ratio normalization', caption: 'Two samples with different sequencing depth become aligned after normalization.', render: normalizationDiagram },
    { title: 'Model group effect', caption: 'A two-fold adjusted mean ratio corresponds to a log2 group effect of +1.', render: effectDiagram },
  ],
  'fdr-volcano': [
    { title: 'BH-adjusted q-values', caption: 'The first two of four toy features pass the 0.05 FDR threshold.', render: bhDiagram },
    { title: 'Interactive volcano thresholds', caption: 'Move either threshold to see why a point changes category; the fitted model stays unchanged.', interactive: true, render: renderToyVolcano },
  ],
  'pca-clustering': [
    { title: 'Original features and PCA projection', caption: 'PCA rotates a cloud of points to show the strongest variation.', render: pcaDiagram },
    { title: 'Distance-based dendrogram', caption: 'The shortest distances join first. Branch positions encode merge distances.', render: clusteringDiagram },
  ],
  'transcript-usage': [{ title: 'Transcript mixtures and CLR', caption: 'Gene totals can match while transcript proportions and CLR values change.', render: transcriptDiagram }],
  longitudinal: [{ title: 'Pairwise patient ranking AUC', caption: 'Count the times a same-patient distance is less than a different-patient distance.', render: longitudinalDiagram }],
  'significant-hits': [{ title: 'Post-selection heatmap', caption: 'Clear separation in selected features is descriptive, not independent test performance.', render: significantDiagram }],
  exclusions: [{ title: 'Sample QC exclusion', caption: 'A user-reviewed rule removes one failed sample before analysis.', render: exclusionDiagram }],
  performance: [{ title: 'CPU and memory information', caption: 'CPU availability and memory allocation describe different resources.', render: performanceDiagram }],
}
export function visualDefinitionCount() {
  return Object.values(chartDefinitions).reduce((sum, defs) => sum + defs.length, 0)
}
export function visualCaption(sectionId, index) {
  return chartDefinitions[sectionId]?.[index]?.caption || ''
}
export function renderToyVisual(sectionId, index) {
  const spec = chartDefinitions[sectionId]?.[index]
  if (!spec) return ''
  const id = `${sectionId}-${index}`
  const visual = spec.interactive ? spec.render({ fdr: 0.05, effect: 1 }) : spec.render(id)
  const controls = spec.interactive ? `<div class="docs-visual-controls" data-toy-volcano>
    <label>FDR ≤ <output data-toy-fdr-output>0.05</output><input type="range" min="0.01" max="0.20" step="0.01" value="0.05" data-toy-fdr aria-label="Toy volcano FDR cutoff"/></label>
    <label>Minimum |log2 effect| ≥ <output data-toy-effect-output>1.00</output><input type="range" min="0" max="2.5" step="0.25" value="1" data-toy-effect aria-label="Toy volcano minimum log2 effect"/></label>
    <output class="docs-visual-count" data-toy-count aria-live="polite">${volcanoSummary({ fdr: 0.05, effect: 1 })}</output>
  </div>` : ''
  const slot = spec.interactive ? `<div data-toy-chart-slot>${visual}</div>` : visual
  return `<figure class="docs-visual-figure" data-visual-example="${escapeSvg(id)}">
    <div class="docs-visual-head"><strong>${escapeSvg(spec.title)}</strong><span>Illustrative diagram</span></div>
    ${slot}${controls}
    <figcaption><strong>What to notice:</strong> ${escapeSvg(spec.caption)}</figcaption>
  </figure>`
}

export function bindToyVisualControls(root) {
  for (const group of root.querySelectorAll('[data-toy-volcano]')) {
    const container = group.closest('[data-visual-example]')
    const fdr = group.querySelector('[data-toy-fdr]')
    const effect = group.querySelector('[data-toy-effect]')
    if (!container || !fdr || !effect) continue
    const update = () => {
      const thresholds = sanitizeToyThresholds({ fdr: fdr.value, effect: effect.value })
      const slot = container.querySelector('[data-toy-chart-slot]')
      if (!slot) return
      slot.innerHTML = renderToyVolcano(thresholds)
      group.querySelector('[data-toy-fdr-output]').textContent = thresholds.fdr.toFixed(2)
      group.querySelector('[data-toy-effect-output]').textContent = thresholds.effect.toFixed(2)
      group.querySelector('[data-toy-count]').textContent = volcanoSummary(thresholds)
    }
    fdr.addEventListener('input', update)
    effect.addEventListener('input', update)
  }
}
