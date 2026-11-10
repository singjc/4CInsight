/** Presentation helpers for post-fit biological interpretation.
 * All seven modality names are displayed. Only RNA views have feature scores.
 * This module never creates or changes the fitted distance model.
 */
export const BIO_MODALITIES = Object.freeze([
  ['gene','Gene expression'],['transcript','Transcript expression'],
  ['bcr','BCR repertoire'],['tcrAlpha','TCR alpha'],['tcrBeta','TCR beta'],
  ['isotype','BCR isotypes'],['immune','Immune infiltration'],
])
const finite = value => typeof value === 'number' && Number.isFinite(value)
const esc = value => String(value ?? '').replaceAll('&','&amp;').replaceAll('<','&lt;')
  .replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const num = (value, digits=3) => finite(value) ? value.toFixed(digits) : '—'
const pct = value => finite(value) ? `${(100*value).toFixed(1)}%` : '—'

export function allModalityRows(report) {
  const allowed = new Set(report?.fixed_modalities || [])
  const byKey = new Map((report?.modality || []).map(row => [row.modality,row]))
  return BIO_MODALITIES.map(([key,label]) => {
    const item = byKey.get(key)
    const included = allowed.has(key)
    return {key,label,included,pairs:item?.pairs ?? 0,
      expected_pairs:item?.expected_pairs ?? 0,
      pair_coverage:item?.expected_pairs ? item.pairs/item.expected_pairs : null,
      between_within_ratio:finite(item?.between_within_ratio)?item.between_within_ratio:null,
      joint_distance_correlation:finite(item?.joint_distance_correlation)?item.joint_distance_correlation:null,
      distance_label:item?.distance_label||''}
  })
}

/** Do not merge statistical scales. Correlation and between/within are separate panels. */
export function modalityOverviewHtml(report) {
  const ratioMaximum = Math.max(2,...allModalityRows(report).filter(r=>r.included&&finite(r.between_within_ratio)).map(r=>r.between_within_ratio))
  return `<div class="bio-modality-grid" role="list" aria-label="All seven measured data types">${allModalityRows(report).map(row => {
    if (!row.included) return `<article class="bio-modality-tile bio-modality-unavailable" role="listitem"><strong>${esc(row.label)}</strong><span>Not in fitted model</span><small>No feature-level result is inferred</small></article>`
    const r = row.between_within_ratio, c = row.joint_distance_correlation
    const w = finite(c) ? Math.min(100,100*Math.abs(c)) : 0
    const v = finite(r) ? Math.min(100,Math.max(0,100*r/ratioMaximum)) : 0
    return `<article class="bio-modality-tile" role="listitem"><strong>${esc(row.label)}</strong>
      <small>${row.pairs.toLocaleString()} of ${row.expected_pairs.toLocaleString()} pairs (${pct(row.pair_coverage)})</small>
      <span>Agreement with joint distances <b>${num(c)}</b></span>
      <div class="bio-meter bio-correlation" role="img" aria-label="Pearson distance correlation ${num(c)}"><i style="width:${w.toFixed(1)}%;background:${c<0?'#b44a59':'#3159d9'}"></i></div>
      <span>Between/within cluster distance <b>${num(r)}</b></span>
      <div class="bio-meter" role="img" aria-label="Between within ratio ${num(r)}; shared visual range 0 to ${num(ratioMaximum)}"><i style="width:${v.toFixed(1)}%;background:#16856b"></i></div>
      <small>${esc(row.distance_label)} · ratios and correlations are descriptive. Ratio bar scale: 0–${num(ratioMaximum)} for every tile.</small>
    </article>`
  }).join('')}</div>`
}

export function combinedFeatureRows(aggregate) {
  const rows=[]
  for(const [kind,label] of BIO_MODALITIES.slice(0,2)) {
    const result=aggregate?.byModality?.[kind]
    if (!result) continue
    for(const row of result.rows||[]) rows.push({...row,modality:kind,modality_label:label})
  }
  return rows
}

export function topFeatureRows(aggregate,modality,axis='pcoa1_spearman',limit=8) {
  if(!['gene','transcript'].includes(modality)||!['pcoa1_spearman','pcoa2_spearman'].includes(axis))
    throw new Error('Unsupported feature ranking view')
  if(!Number.isInteger(limit)||limit<1||limit>100)throw new Error('Invalid display limit')
  const rows=aggregate?.byModality?.[modality]?.rows || []
  return rows.filter(row=>finite(row[axis]))
    .sort((a,b)=>Math.abs(b[axis])-Math.abs(a[axis]) || String(a.feature).localeCompare(String(b.feature)))
    .slice(0,limit)
}

export function featuredProfiles(aggregate) {
  const out=[]
  for(const kind of ['gene','transcript']) {
    const profiles=aggregate?.byModality?.[kind]?.profiles || []
    for(let i=0;i<profiles.length;i++)out.push({key:`${kind}:${i}`,modality:kind,...profiles[i]})
  }
  return out
}
export function chosenProfile(aggregate,selection) {
  return featuredProfiles(aggregate).find(row=>row.key===selection)||null
}

export function featureLollipopHtml(aggregate,axis='pcoa1_spearman',limit=8) {
  const columns=[]
  for(const [kind,label] of BIO_MODALITIES.slice(0,2)) {
    const analysis=aggregate?.byModality?.[kind]
    if(!analysis){columns.push(`<article class="bio-feature-column"><h5>${esc(label)}</h5><p class="method-note">No scored local TPM table. Feature-level results are unavailable.</p></article>`);continue}
    const top=topFeatureRows(aggregate,kind,axis,limit)
    const profiles=analysis.profiles||[]
    columns.push(`<article class="bio-feature-column"><h5>${esc(label)}</h5><small>${analysis.features_ranked.toLocaleString()} scored features · top ${top.length} shown</small>
      <div class="bio-rank-axis" aria-hidden="true"><span>−1</span><span>0</span><span>+1</span></div>
      ${top.map(row=>{
        const rho=row[axis],magnitude=Math.min(50,Math.abs(rho)*50)
        const left=rho<0?50-magnitude:50
        const pi=profiles.findIndex(p=>p.feature===row.feature&&p.secondary_id===row.secondary_id)
        const name=esc(row.feature)
        return `<div class="bio-rank-row"><div class="bio-rank-name">${pi>=0?`<button type="button" class="bio-signal-link" data-bio-profile="${kind}:${pi}" title="Color samples by ${name}">${name}</button>`:name}</div><div class="bio-rank-track" role="img" aria-label="${name}: ${axis==='pcoa1_spearman'?'PCo1':'PCo2'} Spearman ${num(rho)}"><i class="bio-rank-zero"></i><b style="left:${left.toFixed(1)}%;width:${magnitude.toFixed(1)}%;background:${rho<0?'#c54b58':'#3159d9'}"></b></div><span class="bio-rank-score">${num(rho)}</span></div>`
      }).join('')}
    </article>`)
  }
  return `<div class="bio-rank-columns">${columns.join('')}</div><p class="method-note">Bars show signed Spearman correlation with the selected joint PCoA axis. Values range from −1 to +1. These are post-fit, in-sample rankings, not changes between clinical groups. Click an available feature to color the joint ordination.</p>`
}

function safeCell(value){const s=String(value??'');const v=/^\s*[=+@]/.test(s)||/^\s*-(?!\d)/.test(s)?`'${s}`:s;return /[\t\r\n"]/.test(v)?`"${v.replaceAll('"','""')}"`:v}
export function combinedFeatureTsv(aggregate) {
  const header=['data_type','feature','secondary_id','samples_used','spearman_pcoa_1','spearman_pcoa_2','cluster_1_mean_log2_tpm_difference']
  const rows=combinedFeatureRows(aggregate).map(r=>[r.modality_label,r.feature,r.secondary_id,r.n_used,r.pcoa1_spearman,r.pcoa2_spearman,r.cluster1_log2_tpm_difference])
  return [header,...rows].map(row=>row.map(safeCell).join('\t')).join('\n')+'\n'
}
