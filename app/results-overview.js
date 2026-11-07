// UX10: View only. The scientific engine supplies all results and test statistics.
// No source-file reads, network requests, model fitting, or input-size limits.
const MODALITIES = [
  ['gene', 'Gene expression', 'Differential gene expression'],
  ['transcript', 'Transcript expression', 'Differential transcript expression'],
  ['bcr', 'BCR repertoire', 'Clonal diversity and sharing'],
  ['tcrAlpha', 'TCR alpha', 'T cell receptor diversity'],
  ['tcrBeta', 'TCR beta', 'T cell receptor diversity'],
  ['isotype', 'BCR isotypes', 'Relative antibody-class composition'],
  ['immune', 'Immune infiltration', 'Immune-signature enrichment'],
]
const esc = (s) => String(s ?? '').replaceAll('&','&amp;').replaceAll('<','&lt;')
  .replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'", '&#39;')
const val = (v) => v === null || v === undefined || v === '' ? NaN : Number(v)
const good = (v) => Number.isFinite(val(v))
const fmt = (v, digits = 3) => good(v) ? val(v).toLocaleString(undefined,{maximumFractionDigits:digits}) : '—'
const median = (values) => {
  const a = values.map(val).filter(Number.isFinite).sort((x,y) => x-y)
  return a.length ? (a[(a.length-1)>>1] + a[a.length>>1])/2 : NaN
}
const testsOf = (entry) => Array.isArray(entry?.analysis?.tests) ? entry.analysis.tests : []
const fdrCount = (tests, fdr, effect = null) => tests.filter(x =>
  good(x.q_value) && val(x.q_value) <= fdr &&
  (effect === null || (good(x.beta_group) && Math.abs(val(x.beta_group)) >= effect))).length
const numberLabel = (n) => Number(n).toLocaleString()
function featureName(row) {return String(row?.feature || row?.name || row?.secondary_id || 'Unnamed feature')}
function chartEmpty(message) {return `<p class="overview-chart-empty">${esc(message)}</p>`}

function signedEffectChart(rows, caption, fdr) {
  const sorted = [...rows].filter(r => good(r.beta_group))
    .sort((a,b) => val(a.q_value ?? 1) - val(b.q_value ?? 1)).slice(0, 9)
  if (!sorted.length) return chartEmpty('No adjusted feature effects are available.')
  const max = Math.max(1e-12, ...sorted.map(r=>Math.abs(val(r.beta_group))))
  const width = 660, left = 196, right = 45, center = left + (width-left-right)/2
  const half = (width-left-right)/2-12
  const height = 45 + sorted.length*33
  const markup = sorted.map((row,i) => {
    const effect = val(row.beta_group), length = Math.abs(effect)/max*half
    const y = 32+i*33, x = effect>=0 ? center : center-length
    const significant = good(row.q_value) && val(row.q_value)<=fdr
    const label = featureName(row)
    return `<g><title>${esc(label)}: effect ${fmt(effect,5)}; q ${fmt(row.q_value,5)}</title>
    <text x="8" y="${y+13}" class="overview-svg-name">${esc(label.length>25 ? `${label.slice(0,24)}…`:label)}</text>
    <rect x="${x.toFixed(2)}" y="${y}" width="${Math.max(1,length).toFixed(2)}" height="18" rx="3"
      fill="${effect>=0?'#d64556':'#3159d9'}" fill-opacity="${significant?'0.94':'0.32'}" />
    <text x="${effect>=0?Math.min(width-8,center+length+6):Math.max(left,center-length-6)}" y="${y+13}"
      text-anchor="${effect>=0?'start':'end'}" class="overview-svg-small">${fmt(effect)}${significant?'*':''}</text></g>`
  }).join('')
  return `<figure class="overview-figure"><svg class="overview-svg" viewBox="0 0 ${width} ${height}" role="img" aria-label="${esc(caption)}">
    <title>${esc(caption)}</title><line x1="${center}" x2="${center}" y1="23" y2="${height-8}" stroke="#94a3b8" stroke-dasharray="4 4"/>
    <text class="overview-svg-small" x="${center-8}" y="17" text-anchor="end">Lower in case</text>
    <text class="overview-svg-small" x="${center+8}" y="17">Higher in case</text>${markup}</svg>
    <figcaption>Bars: signed adjusted effect · * q ≤ ${esc(fdr)} · Faded bars do not meet FDR.</figcaption></figure>`
}

function sampleMetricChart(summaries, result) {
  if (!Array.isArray(summaries) || !summaries.length) return chartEmpty('Sample-level receptor metrics are not available.')
  const caseLabel = String(result?.comparison?.case_label || summaries[0]?.group || 'Case')
  const controlLabel = String(result?.comparison?.control_label || summaries.find(s=>s.group!==caseLabel)?.group || 'Control')
  const fields = [
    ['Matched-depth richness',['rarefied_richness','matched_depth_richness','observed_richness'],'count'],
    ['Clonality',['clonality'],'fraction'],
    ['Public clonotypes',['public_fraction'],'fraction'],
  ]
  const width=660, start=235, end=612, height=232
  let shown=0
  const rows=fields.map(([label,candidates,unit],idx) => {
    const key=candidates.find(k=>summaries.some(s=>good(s[k])))
    if (!key) return ''
    shown++
    const values=summaries.map(s=>val(s[key])).filter(Number.isFinite)
    let min=Infinity,max=-Infinity
    for (const v of values) {if(v<min)min=v;if(v>max)max=v}
    const spread=Math.max(1e-10,max-min)
    const low=Math.max(0,min-spread*0.08), high=max+spread*0.08
    const scale=v=>start+(val(v)-low)/(high-low)*(end-start)
    const y=38+idx*66
    const dots=summaries.map((s,i)=>{
      if (!good(s[key])) return ''
      const c=s.group===caseLabel?'#d64556':'#3159d9', jitter=(i*19%9-4)*1.6
      const cy=y+(s.group===caseLabel?-6:8)+jitter
      return `<circle cx="${scale(s[key]).toFixed(2)}" cy="${cy.toFixed(2)}" r="2.9" fill="${c}" fill-opacity="0.68"><title>${esc(s.sample_id)} (${esc(s.group)}): ${fmt(s[key],4)}</title></circle>`
    }).join('')
    const medians=[caseLabel,controlLabel].map((group,i)=>{
      const m=median(summaries.filter(s=>s.group===group).map(s=>s[key]))
      return Number.isFinite(m)? `<path d="M${scale(m).toFixed(2)} ${y-20+i*14} v17" stroke="${i===0?'#d64556':'#3159d9'}" stroke-width="3"><title>${esc(group)} median ${fmt(m)}</title></path>`:''
    }).join('')
    return `<g><text x="8" y="${y}" class="overview-svg-name">${esc(label)}</text>
      <text x="8" y="${y+17}" class="overview-svg-small">${esc(unit==='count'?'Clonotypes':'Proportion')}</text>
      <line x1="${start}" x2="${end}" y1="${y}" y2="${y}" stroke="#cbd5e1"/>${dots}${medians}
      <text x="${start}" y="${y+29}" class="overview-svg-tick">${fmt(low,2)}</text>
      <text x="${end}" y="${y+29}" text-anchor="end" class="overview-svg-tick">${fmt(high,2)}</text></g>`
  }).join('')
  if (!shown) return chartEmpty('The receptor result has no supported sample metrics.')
  return `<figure class="overview-figure"><svg class="overview-svg" viewBox="0 0 ${width} ${height}" role="img" aria-label="Receptor sample distributions and group medians">
    <title>Receptor diversity and sharing by group</title>${rows}</svg><figcaption><span class="overview-legend"><i style="--legend-color:#d64556"></i>${esc(caseLabel)}</span>
    <span class="overview-legend"><i style="--legend-color:#3159d9"></i>${esc(controlLabel)}</span>
    Each dot represents a sample. Lines mark group medians. Each measure has its own scale.</figcaption></figure>`
}

function volcanoChart(tests, fdr, effectCutoff) {
  const usable=tests.filter(t=>good(t.beta_group)&&good(t.q_value))
  if (!usable.length) return chartEmpty('No adjusted expression effects are available.')
  const width=660,height=230,left=48,right=18,top=20,bottom=32
  let maxX=Math.max(effectCutoff+0.01,1),maxY=Math.max(1.5,-Math.log10(Math.max(fdr,1e-15)))
  for(const t of usable){maxX=Math.max(maxX,Math.abs(val(t.beta_group)));maxY=Math.max(maxY,-Math.log10(Math.max(val(t.q_value),1e-300)))}
  maxX*=1.08;maxY*=1.05
  const sx=x=>left+(val(x)+maxX)/(2*maxX)*(width-left-right)
  const sy=y=>height-bottom-val(y)/maxY*(height-top-bottom)
  const points=usable.length<=1500?usable:usable.filter((_,i)=>i%Math.ceil(usable.length/1500)===0)
  const circles=points.map(t=>{
    const x=val(t.beta_group),q=val(t.q_value)
    const sig=q<=fdr&&Math.abs(x)>=effectCutoff
    const color=!sig?'#94a3b8':x>0?'#d64556':'#3159d9'
    return `<circle cx="${sx(x).toFixed(2)}" cy="${sy(-Math.log10(Math.max(q,1e-300))).toFixed(2)}" r="2.1" fill="${color}" fill-opacity=".7"><title>${esc(featureName(t))}: effect ${fmt(x)}, q ${fmt(q,5)}</title></circle>`
  }).join('')
  return `<figure class="overview-figure"><svg class="overview-svg" viewBox="0 0 ${width} ${height}" role="img" aria-label="Expression effect against adjusted significance">
    <title>Adjusted expression differences</title><line x1="${left}" x2="${width-right}" y1="${height-bottom}" y2="${height-bottom}" stroke="#64748b"/>
    <line x1="${sx(0)}" x2="${sx(0)}" y1="${top}" y2="${height-bottom}" stroke="#cbd5e1"/>
    <line x1="${sx(-effectCutoff)}" x2="${sx(-effectCutoff)}" y1="${top}" y2="${height-bottom}" stroke="#94a3b8" stroke-dasharray="4 4"/>
    <line x1="${sx(effectCutoff)}" x2="${sx(effectCutoff)}" y1="${top}" y2="${height-bottom}" stroke="#94a3b8" stroke-dasharray="4 4"/>
    <line x1="${left}" x2="${width-right}" y1="${sy(-Math.log10(fdr))}" y2="${sy(-Math.log10(fdr))}" stroke="#94a3b8" stroke-dasharray="4 4"/>
    ${circles}<text x="${width/2}" y="${height-7}" text-anchor="middle" class="overview-svg-small">Adjusted log2 group effect</text>
    <text x="18" y="${height/2}" text-anchor="middle" transform="rotate(-90 18 ${height/2})" class="overview-svg-small">−log10(q)</text></svg>
    <figcaption>Red: higher in case · Blue: lower in case · Gray: below chosen cutoffs. For large results, only plotted points are thinned; the counts use all rows.</figcaption></figure>`
}
function resultStats(entry,key,fdr,cutoff){
  const tests=testsOf(entry), expression=key==='gene'||key==='transcript'
  const selected=fdrCount(tests,fdr,expression?cutoff:null)
  const top=[...tests].filter(t=>good(t.q_value)).sort((a,b)=>val(a.q_value)-val(b.q_value))[0]
  return {tests,selected,top,expression}
}
function overviewCard(result,key,title,subtitle,fdr,cutoff,expanded=false){
  const entry=result?.[key]
  if (!entry?.analysis) return ''
  const {tests,selected,top,expression}=resultStats(entry,key,fdr,cutoff)
  const summaries=entry.analysis.sample_summaries || []
  const sampleCount=entry.analysis.n_model_samples ?? entry.analysis.n_samples ?? entry.analysis.files_used ?? entry.files_used ?? summaries.length
  const context = expression ? `q ≤ ${fdr}, |log2 effect| ≥ ${cutoff}` : `q ≤ ${fdr}`
  const takeaway=tests.length
    ? `${numberLabel(selected)} of ${numberLabel(tests.length)} adjusted tests meet the selected threshold${selected?`; strongest adjusted feature: ${featureName(top)}`:''}.`
    : 'The analysis is available, but no adjusted feature tests are available for this summary.'
  const chart = key==='bcr'||key==='tcrAlpha'||key==='tcrBeta'
    ? sampleMetricChart(summaries,entry)
    : expression ? volcanoChart(tests,fdr,cutoff)
      : signedEffectChart(tests,key==='isotype'?'Signed CLR isotype effects':'Signed immune-signature effects',fdr)
  const caveat = key==='isotype'?'These are relative composition effects, not antibody concentrations.'
    : key==='immune'?'These are enrichment-score effects, not estimates of immune-cell percentages.'
      : expression?'Display cutoffs do not refit the model.':'Sample distributions are descriptive. Use adjusted tests for group inference.'
  return `<section class="card overview-modality${expanded ? ' overview-expanded' : ''}" data-overview-modality="${key}"><header class="overview-modality-heading"><div>
    <span class="overview-kicker">${esc(subtitle)}</span><h3>${esc(title)}</h3></div><div class="overview-heading-actions"><span class="tag success">${esc(numberLabel(sampleCount))} samples</span>
    <button type="button" class="overview-expand-button" data-overview-expand="${key}" aria-expanded="${expanded}">${expanded ? 'Reduce' : 'Expand'}</button></div></header>
    <div class="overview-summary-row"><div class="overview-big-number"><strong>${numberLabel(selected)}</strong><span>Selected findings</span><small>${esc(context)}</small></div>
    <p class="overview-takeaway">${esc(takeaway)}</p></div><div class="overview-chart-wrap">${chart}</div>
    <div class="overview-card-tools"><span class="overview-plot-hint">Zoom with buttons or Ctrl/⌘ + scroll; drag to pan; double-click to reset.</span></div>
    <p class="results-export-error" hidden role="alert"></p>
    <footer class="overview-footer"><p>${esc(caveat)}</p><button type="button" class="secondary-button overview-open-details" data-results-detail="${key}">View detailed results</button></footer></section>`
}
export function renderResultsOverview(result, options={}) {
  if (!result) return '<section class="card"><h3>No comparison results yet</h3><p>Run a cohort comparison to see the main results.</p></section>'
  const fdr=good(options.fdr) ? Math.max(1e-6,Math.min(1,val(options.fdr))):.05
  const cutoff=good(options.absLog2Fc)?Math.max(0,val(options.absLog2Fc)):1
  const expandedModalities = new Set(Array.isArray(options.expandedModalities) ? options.expandedModalities : [])
  const available=MODALITIES.filter(([key])=>result[key]?.analysis)
  const errors=Object.entries(result.omicsErrors || {})
  return `<section class="card overview-intro"><div><span class="eyebrow">At a glance</span><h3>What did the cohort comparison show?</h3>
    <p>${available.length} of ${available.length+errors.length} data types produced results. Review the main plots, then open Detailed Results for full methods and evidence.</p></div>
    <div class="overview-at-a-glance"><strong>${available.length}</strong><span>Data types with results</span></div></section>
    <div class="overview-modality-grid">${MODALITIES.map(([key,title,subtitle])=>overviewCard(result,key,title,subtitle,fdr,cutoff,expandedModalities.has(key))).join('')}</div>
    ${errors.length?`<section class="card overview-errors"><h3>Analyses that did not finish</h3>${errors.map(([kind,message])=>`<p><strong>${esc(kind)}:</strong> ${esc(message)}</p>`).join('')}</section>`:''}
    <p class="overview-method-note">FDR controls the displayed findings. Non-significant results do not prove that groups are equivalent. See Detailed Results for full tables, exclusions, warnings, and exports.</p>`
}
