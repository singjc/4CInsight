/** Read-only follow-through for the fitted joint model.
 * Never refits a comparison, assigns a clinical subtype, or invents q-values.
 * Predefined clinical comparisons are separate results with unverified cohort overlap.
 */
import { BIO_MODALITIES } from './biological-dashboard.js?v=0.22.8'
import { summarizeIntegrativeEvidence } from './integrative-evidence.js?v=0.22.8'

const finite = value => typeof value === 'number' && Number.isFinite(value)
const esc = value => String(value ?? '').replaceAll('&','&amp;').replaceAll('<','&lt;')
  .replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const fmt = value => finite(value) ? value.toFixed(3) : '—'
const show = value => Number.isFinite(value) ? value.toLocaleString() : '—'
const safeText = value => {
  const text = String(value ?? '')
  const protectedText = /^\s*[=+@]/.test(text) || /^\s*-(?!\d)/.test(text) ? "'"+text : text
  return /[\t\r\n"]/.test(protectedText) ? `"${protectedText.replaceAll('"','""')}"` : protectedText
}

/** Return all available measured feature associations; labels are never clinical effects. */
export function biologicalCandidates(rna, source) {
  const rows=[]
  for (const [key,label] of BIO_MODALITIES) {
    const result=(key==='gene'||key==='transcript') ? rna?.byModality?.[key] : source?.byModality?.[key]
    for (const row of result?.rows || []) {
      const r1=finite(row.pcoa1_spearman) ? row.pcoa1_spearman : null
      const r2=finite(row.pcoa2_spearman) ? row.pcoa2_spearman : null
      if (r1===null && r2===null) continue
      rows.push({kind:key,modality:label,family:key==='gene'||key==='transcript'?'Expression abundance':String(row.family||'Measured source feature'),
        feature:String(row.feature||''),secondary_id:String(row.secondary_id||''),n_used:Number(row.n_used)||0,
        pcoa1_spearman:r1,pcoa2_spearman:r2,
        strength:Math.max(Math.abs(r1??0),Math.abs(r2??0)),
        source_type:'Post-fit PCoA association',evidence_class:'exploratory'})
    }
  }
  return rows
}

const candidateOrder = (a,b,sort) => {
  let difference=0
  if (sort==='pcoa1') difference=Math.abs(b.pcoa1_spearman??0)-Math.abs(a.pcoa1_spearman??0)
  else if (sort==='pcoa2') difference=Math.abs(b.pcoa2_spearman??0)-Math.abs(a.pcoa2_spearman??0)
  else if (sort==='feature') difference=a.feature.localeCompare(b.feature)
  else difference=b.strength-a.strength
  return difference || a.modality.localeCompare(b.modality) || a.feature.localeCompare(b.feature)
}
export function filterBiologicalCandidates(rows,{kind='all',query='',sort='strongest',limit=60}={}) {
  if (!['all',...BIO_MODALITIES.map(m=>m[0])].includes(kind)) throw new Error('Invalid data type')
  if (!['strongest','pcoa1','pcoa2','feature'].includes(sort)) throw new Error('Invalid candidate sort')
  if (!Number.isInteger(limit)||limit<1||limit>200) throw new Error('Invalid table display limit')
  const q=String(query).trim().toLocaleLowerCase()
  const filtered=rows.filter(row=>(kind==='all'||row.kind===kind) && (!q ||
    [row.modality,row.family,row.feature,row.secondary_id].some(value=>String(value).toLocaleLowerCase().includes(q))))
  filtered.sort((a,b)=>candidateOrder(a,b,sort))
  return {total:filtered.length,shown:filtered.slice(0,limit)}
}

/** Counts come from previously run Compare Groups. No cohort or feature match is assumed. */
export function biologicalEvidenceRows({rna,source,compare,thresholds={},enabled={}}={}) {
  const selected=summarizeIntegrativeEvidence(compare,thresholds,enabled)
  const compareByKey=new Map(selected.map(row=>[row.key,row]))
  return BIO_MODALITIES.map(([key,label])=>{
    const result=(key==='gene'||key==='transcript')?rna?.byModality?.[key]:source?.byModality?.[key]
    const leading=(result?.rows||[]).filter(row=>finite(row.pcoa1_spearman)||finite(row.pcoa2_spearman))
      .sort((a,b)=>Math.max(Math.abs(b.pcoa1_spearman||0),Math.abs(b.pcoa2_spearman||0))-
        Math.max(Math.abs(a.pcoa1_spearman||0),Math.abs(a.pcoa2_spearman||0)) ||
        String(a.feature).localeCompare(String(b.feature)))[0]||null
    const clinical=compareByKey.get(key)
    return {key,label,feature:leading?.feature||null,association:leading ?
      (Math.abs(leading.pcoa1_spearman||0)>=Math.abs(leading.pcoa2_spearman||0)?leading.pcoa1_spearman:leading.pcoa2_spearman):null,
      axis:leading?(Math.abs(leading.pcoa1_spearman||0)>=Math.abs(leading.pcoa2_spearman||0)?'PCo1':'PCo2'):null,
      scored:result?.rows?.length??null,measured:result?.sample_count??null,
      compared:Boolean(clinical?.available),tested:clinical?.available?clinical.tested:null,
      selected:clinical?.available?clinical.selected:null,included:clinical?.included??false}
  })
}

export function biologicalCandidatesTsv(rows) {
  const keys=['modality','family','feature','secondary_id','n_used','pcoa1_spearman','pcoa2_spearman','evidence_class','source_type']
  return [keys,...rows.map(row=>keys.map(key=>row[key]))].map(row=>row.map(safeText).join('\t')).join('\n')+'\n'
}

/** A separate, explicitly exploratory gene ranking. Duplicate IDs are removed, not guessed. */
export function exploratoryGeneRankingTsv(rows,axis='pcoa1_spearman') {
  if (!['pcoa1_spearman','pcoa2_spearman'].includes(axis)) throw new Error('Invalid PCoA axis')
  const genes=rows.filter(row=>row.kind==='gene'&&row.feature.trim()&&finite(row[axis]))
  const counts=new Map()
  for(const gene of genes)counts.set(gene.feature,(counts.get(gene.feature)||0)+1)
  const unique=genes.filter(row=>counts.get(row.feature)===1)
    .sort((a,b)=>b[axis]-a[axis]||a.feature.localeCompare(b.feature))
  return {total:genes.length,excludedDuplicateIds:genes.length-unique.length,
    tsv:[['gene_identifier','exploratory_spearman_'+axis,'n_used'],...unique.map(row=>[row.feature,row[axis],row.n_used])]
      .map(row=>row.map(safeText).join('\t')).join('\n')+'\n'}
}

export function biologicalNextStepsHtml({report,coverage,rna,source,compare,thresholds,enabled,observationRole,patientRole}={}) {
  const rows=biologicalEvidenceRows({rna,source,compare,thresholds,enabled})
  const clusterInfo=report?.membership?.map(row=>`Cluster ${row.cluster}: ${row.observations}`).join(' · ')||'No fitted clusters'
  const omitted=Number.isFinite(coverage?.excluded)?coverage.excluded:'—'
  const comparison=compare?`Previous comparison: <strong>${esc(compare.comparisonName||'Saved Compare Groups result')}</strong> (cohort overlap not checked)`:
    'No predefined group comparison has been run in this session.'
  const repeated=Boolean(observationRole&&patientRole&&observationRole===patientRole)
  return `<section class="integrative-overview-section biological-next-steps" id="biological-next-steps" aria-labelledby="bio-next-title">
    <div class="card-heading"><div><h4 id="bio-next-title">Next biological steps</h4><p>Move from shared sample patterns to testable biological questions. This section uses saved results; it does not refit any model.</p></div><span class="tag neutral">Interpretation workflow</span></div>
    <div class="bio-next-steps-grid">
      <article class="bio-next-step"><strong>1. Check the fitted cohort</strong><p>${esc(clusterInfo)} · ${esc(omitted)} omitted from the joint model.</p>
        <p>Review source coverage, small clusters, quality and batch variables.</p><button type="button" class="secondary-button" data-bio-next="coverage">Review cohort coverage</button></article>
      <article class="bio-next-step"><strong>2. Check clinical context</strong><p>Colour the PCoA by a known clinical, batch, or visit column. Inspect unusual observations.</p>
        ${repeated?'<p class="bio-next-caution">Observation and patient IDs are the same. Check for repeated visits.</p>':''}
        <button type="button" class="secondary-button" data-bio-next="metadata">Review metadata on plot</button></article>
      <article class="bio-next-step"><strong>3. Test a defined group contrast</strong><p>Select case/control values before analysis. Use correct count models and patient-aware settings as needed.</p>
        <button type="button" class="secondary-button" data-bio-next="compare">Open Compare Groups</button>${compare?'<button type="button" class="secondary-button" data-bio-next="results">Open existing Results</button>':''}</article>
      <article class="bio-next-step"><strong>4. Review features and pathways</strong><p>Use the full ranked lists for hypotheses. Compare with previous adjusted results, then validate.</p>
        <button type="button" class="secondary-button" data-bio-next="features">Explore ranked features</button><button type="button" class="secondary-button" data-bio-next="pathways">Run pathway analysis</button></article>
    </div>
    <p id="bio-next-export-status" role="status" aria-live="polite" hidden></p>
    <div class="bio-next-review" id="bio-next-evidence">
      <h5>Evidence across seven data types</h5>
      <p class="method-note">The PCoA associations are exploratory and in-sample. Previous Compare Groups tests use their own clinical contrast and test families. Their sample overlap, dependence and feature match with the joint model have <strong>not been verified</strong>; these two results are not independent confirmation.</p>
      <p class="method-note">${comparison}. Selected tests use the current q-value and modality-specific effect thresholds for display only; the original adjusted q-values are not changed.</p>
      <div class="table-scroll bio-next-evidence-table"><table><thead><tr><th>Data type</th><th>Leading joint association</th><th>Spearman</th><th>Features scored</th><th>Previous adjusted tests</th><th>Selected tests</th></tr></thead><tbody>
      ${rows.map(row=>`<tr><td><strong>${esc(row.label)}</strong></td><td>${row.feature?esc(row.feature):'Not available'}</td><td>${row.axis?`${esc(row.axis)}: ${fmt(row.association)}`:'—'}</td><td>${show(row.scored)}</td><td>${row.compared?show(row.tested):'Not run'}</td><td>${row.compared?show(row.selected):'—'}</td></tr>`).join('')}
      </tbody></table></div>
      <p class="method-note">The leading association and previous adjusted tests are separate results. A row does not imply that its leading feature has a significant q-value.</p>
    </div>
    <details class="bio-next-review bio-next-expand" id="bio-next-feature-explorer"><summary>Explore all measured feature associations</summary>
      <p class="method-note">Search, sort, and review PCo1 and PCo2 correlations. Positive and negative directions depend on arbitrary PCoA axis orientation. The visible table shows at most 60 rows; exports include every scored feature.</p>
      <div class="bio-next-controls"><label>Data type<select id="bio-next-kind"><option value="all">All seven data types</option>${BIO_MODALITIES.map(([key,label])=>`<option value="${key}">${esc(label)}</option>`).join('')}</select></label>
        <label>Sort<select id="bio-next-sort"><option value="strongest">Strongest absolute association</option><option value="pcoa1">Absolute PCo1 association</option><option value="pcoa2">Absolute PCo2 association</option><option value="feature">Feature name</option></select></label>
        <label>Search<input id="bio-next-query" type="search" placeholder="Feature, family or data type" autocomplete="off"/></label></div>
      <p class="method-note" id="bio-next-count" role="status" aria-live="polite"></p>
      <div class="table-scroll bio-next-feature-table"><table><thead><tr><th>Data type</th><th>Feature</th><th>Family</th><th>Samples</th><th>PCo1 ρ</th><th>PCo2 ρ</th></tr></thead><tbody id="bio-next-feature-body"></tbody></table></div>
      <div class="bio-next-actions"><button type="button" class="secondary-button" data-bio-next="export-features">Export all measured feature associations (TSV)</button><button type="button" class="secondary-button" data-bio-next="details">Review methods and sample-level features</button></div>
    </details>
    <details class="bio-next-review bio-next-expand" id="bio-next-pathways"><summary>Pathway analysis and validation</summary>
      <p class="method-note">Pathway enrichment has not been calculated by the joint model. Use the local module below to calculate actual pathway statistics from a user-selected GMT. Corrected group effects and exploratory PCoA correlations are separate inputs. PCoA correlations are not differential-expression statistics.</p>
      <div class="bio-next-actions"><label>Ranked gene axis<select id="bio-next-axis"><option value="pcoa1_spearman">PCo1 Spearman</option><option value="pcoa2_spearman">PCo2 Spearman</option></select></label><button type="button" class="secondary-button" data-bio-next="export-genes">Export exploratory gene ranks (TSV)</button></div>
      <p class="method-note">The full pathway test is in the Pathways view. It uses your saved gene results and does not need another joint-model fit.</p><button type="button" class="secondary-button" data-bio-next="pathways">Open Pathways</button>
      <p class="method-note">Independent validation: <strong>not available</strong>. Confirm observed clinical associations in held-out patients or independent data. Avoid testing cluster differences on the same measurements used to form those clusters.</p>
      <button type="button" class="secondary-button" data-bio-next="export-audit">Export interpretation audit (JSON)</button>
    </details>
  </section>`
}

/** UI-only table. No network access, changes to scientific state or input truncation. */
export function bindBiologicalFeatureExplorer(root,rows) {
  const kind=root.querySelector('#bio-next-kind'),sort=root.querySelector('#bio-next-sort'),query=root.querySelector('#bio-next-query')
  const body=root.querySelector('#bio-next-feature-body'),count=root.querySelector('#bio-next-count')
  if(!kind||!sort||!query||!body||!count)return
  function draw(){
    const filtered=filterBiologicalCandidates(rows,{kind:kind.value,sort:sort.value,query:query.value})
    count.textContent=`Showing ${filtered.shown.length.toLocaleString()} of ${filtered.total.toLocaleString()} matching measured features (${rows.length.toLocaleString()} total scored).`
    body.innerHTML=filtered.shown.map(row=>`<tr><td>${esc(row.modality)}</td><td>${esc(row.feature)}</td><td>${esc(row.family)}</td><td>${show(row.n_used)}</td><td>${fmt(row.pcoa1_spearman)}</td><td>${fmt(row.pcoa2_spearman)}</td></tr>`).join('')||'<tr><td colspan="6">No measured features match this search.</td></tr>'
  }
  for(const element of [kind,sort])element.addEventListener('change',draw)
  query.addEventListener('input',draw)
  draw()
}
