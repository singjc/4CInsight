/** Read-only cohort-coverage diagnostics for a completed integrative fit.
 * Uses only saved exact alignment, joint distances and fixed-cohort results.
 * Presence of a source alias is NOT proof of measured sample-pair distances.
 */
export const COVERAGE_MODALITIES = Object.freeze([
  ['gene','Gene expression'],['transcript','Transcript expression'],['bcr','BCR repertoire'],
  ['tcrAlpha','TCR alpha'],['tcrBeta','TCR beta'],['isotype','BCR isotypes'],['immune','Immune infiltration'],
])
const escape = s => String(s??'').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const validInteger = n => Number.isInteger(n)&&n>=0
const count = n => validInteger(n)?n.toLocaleString('en-GB'):'—'
const safeCell = value => {
  const s=String(value??'')
  const guarded=/^\s*[=+@]/.test(s)||/^\s*-(?!\d)/.test(s)?`'${s}`:s
  return /[\t\r\n"]/.test(guarded)?`"${guarded.replaceAll('"','""')}"`:guarded
}
const reasonLabel = Object.freeze({
  retained:'Included in joint model',
  'patient-deduplicated':'Additional observation for the same patient',
  'missing-fixed-pairs':'Removed to obtain complete fixed-view distances',
  unclassified:'Not classified by saved coverage data',
})

export function buildIntegrativeCoverage(alignment,structure,distances) {
  if(alignment?.schema!=='4c-insight-observation-alignment-v1'||!Array.isArray(alignment.rows)||
    structure?.schema!=='4c-insight-joint-structure-v1'||!Array.isArray(structure.observations))return null
  const eligible=alignment.rows.filter(r=>!r.excluded)
  const ids=new Set(eligible.map(r=>r.observation))
  if(ids.size!==eligible.length||structure.observations.some(id=>!ids.has(id)))return null
  const selected=Array.isArray(structure.fixed_modalities)?structure.fixed_modalities:[]
  if(!selected.length||new Set(selected).size!==selected.length)return null
  const included=new Set(structure.observations)
  const pruned=new Set(structure.coverage?.removed_observation_ids||[])
  const deduplicated=new Set(structure.coverage?.patient_deduplicated_ids||[])
  if([...pruned,...deduplicated].some(id=>!ids.has(id)||included.has(id))||[...pruned].some(id=>deduplicated.has(id)))return null
  const fullNeighbors=new Map(eligible.map(r=>[r.observation,0]))
  const fusedIds=new Set(distances?.observations||[])
  const fusedViews=new Set((distances?.views||[]).map(r=>r.modality))
  const pairs=Array.isArray(distances?.pairs)&&Array.isArray(distances?.observations)&&
      fusedIds.size===eligible.length&&eligible.every(r=>fusedIds.has(r.observation))&&
      fusedViews.size===selected.length&&selected.every(key=>fusedViews.has(key)) ? distances.pairs : null
  // Only complete fixed-panel pairs count. An incomplete fused pair cannot enter the fixed model.
  if(pairs)for(const p of pairs){
    if(!ids.has(p?.a)||!ids.has(p?.b)||p.a===p.b||!Array.isArray(p.modalities)||
      p.modalities.length!==selected.length||p.contributing_views!==selected.length||
      !selected.every(key=>p.modalities.includes(key)))continue
    fullNeighbors.set(p.a,fullNeighbors.get(p.a)+1)
    fullNeighbors.set(p.b,fullNeighbors.get(p.b)+1)
  }
  const rows=eligible.map(row=>{
    const missing=selected.filter(key=>row.modalities?.[key]?.status!=='present')
    const reason=included.has(row.observation)?'retained':deduplicated.has(row.observation)?'patient-deduplicated':pruned.has(row.observation)?'missing-fixed-pairs':'unclassified'
    return {observation:row.observation,patient:row.subject||'',visit:row.timepoint||'',reason,
      sourcePresent:selected.length-missing.length,sourceTotal:selected.length,
      sourceAbsent:missing.map(key=>{const label=COVERAGE_MODALITIES.find(v=>v[0]===key)?.[1]||key;const status=row.modalities?.[key]?.status||'unknown';return `${label} (${status})`}),
      fullPairNeighbors:pairs?fullNeighbors.get(row.observation):null}
  })
  const byReason=Object.fromEntries(Object.keys(reasonLabel).map(k=>[k,rows.filter(r=>r.reason===k).length]))
  const modalities=selected.map(key=>{
    const eligiblePresent=eligible.filter(r=>r.modalities?.[key]?.status==='present').length
    const retainedPresent=eligible.filter(r=>included.has(r.observation)&&r.modalities?.[key]?.status==='present').length
    const statusCounts={}
    for(const row of eligible){const status=row.modalities?.[key]?.status||'unknown';statusCounts[status]=(statusCounts[status]||0)+1}
    return {key,label:COVERAGE_MODALITIES.find(v=>v[0]===key)?.[1]||key,
      eligiblePresent,retainedPresent,eligible:eligible.length,retained:included.size,statusCounts}
  })
  return {rows,omitted:rows.filter(r=>r.reason!=='retained'),modalities,byReason,eligible:eligible.length,
    retained:included.size,excluded:eligible.length-included.size,
    haveFusedPairs:Boolean(pairs),coverage:structure.coverage}
}

export function integrativeCoverageHtml(report){
  if(!report)return '<p class="method-note">Source coverage diagnostics are unavailable. Recheck the alignment and joint model in Details.</p>'
  const excluded=report.excluded
  const percent=report.eligible>0?(excluded/report.eligible*100).toFixed(1):'0.0'
  const reasonRows=[['Additional observation for the same patient','patient-deduplicated'],
    ['Removed for missing fixed-view distances','missing-fixed-pairs'],
    ['Reason not recorded','unclassified']]
  const sorted=[...report.omitted].sort((a,b)=>a.reason.localeCompare(b.reason)||a.observation.localeCompare(b.observation))
  const metadataRows=sorted.map(r=>`<tr data-coverage-row="true"><td>${escape(r.observation)}</td><td>${escape(r.patient||'—')}</td><td>${escape(r.visit||'—')}</td><td>${escape(reasonLabel[r.reason])}</td><td>${count(r.sourcePresent)} / ${count(r.sourceTotal)}</td><td>${escape(r.sourceAbsent.join(', ')||'None recorded')}</td><td>${count(r.fullPairNeighbors)}</td></tr>`).join('')
  return `<div class="integrative-coverage" id="integrative-coverage">
    <p class="method-note"><strong>${count(excluded)} of ${count(report.eligible)} eligible observations (${percent}%)</strong> are not in the fixed-view joint model. This table uses exact saved selection records; it does not infer missing measurements from the PCoA plot.</p>
    <div class="integrative-coverage-reasons" role="list">${reasonRows.map(([label,key])=>`<div role="listitem"><strong>${count(report.byReason[key])}</strong><span>${escape(label)}</span></div>`).join('')}</div>
    <details class="integrative-coverage-detail"><summary>View source coverage for all ${count(report.eligible)} eligible observations</summary>
      <p class="method-note">Source coverage means an exact local source alias was found at alignment. It does not prove that a pairwise distance was calculated. The retained column uses the saved fixed-view cohort.</p>
      <div class="table-scroll"><table><thead><tr><th>Data type</th><th>Source found / eligible</th><th>Source found / retained</th><th>Missing or ambiguous / eligible</th></tr></thead><tbody>${report.modalities.map(r=>`<tr><td>${escape(r.label)}</td><td>${count(r.eligiblePresent)} / ${count(r.eligible)}</td><td>${count(r.retainedPresent)} / ${count(r.retained)}</td><td>${count(r.eligible-r.eligiblePresent)}</td></tr>`).join('')}</tbody></table></div>
    </details>
    ${excluded?`<details class="integrative-coverage-detail" id="integrative-omitted-detail"><summary>Inspect ${count(excluded)} omitted observations and recorded reasons</summary>
      <p class="method-note">An observation can lack fixed-panel pairs even when all source aliases are present. The recorded reason describes model selection, not the underlying biological or technical cause. ${report.haveFusedPairs?'Complete-panel partner counts use the saved fused-distance pairs before pruning.':'Full-panel pair counts are unavailable because a matching fused result was not saved.'}</p>
      <div class="integrative-coverage-controls"><label for="integrative-coverage-search">Search observations and reasons</label><input id="integrative-coverage-search" type="search" placeholder="Sample, patient, data type or reason"/><button id="integrative-coverage-export" type="button" class="secondary-button">Save omitted observations TSV</button><span id="integrative-coverage-count" role="status">${count(excluded)} rows</span></div>
      <div class="table-scroll integrative-coverage-table"><table><thead><tr>${['Observation','Patient','Visit','Selection reason','Sources found','Sources not confirmed','Complete-pair partners'].map((h,i)=>`<th><button type="button" data-coverage-sort="${i}" aria-label="Sort by ${escape(h)}">${escape(h)}</button></th>`).join('')}</tr></thead><tbody id="integrative-coverage-body">${metadataRows}</tbody></table></div>
    </details>`:'<p class="method-note">All eligible observations are in the joint model.</p>'}
    <p class="method-note">The complete-distance cohort is selected by a deterministic greedy rule. It is not guaranteed to be the largest possible complete cohort. No missing distances were filled in.</p>
  </div>`
}

export function integrativeOmittedTsv(report){
  if(!report)return ''
  const rows=[['observation','patient','timepoint','selection_reason','source_aliases_found','selected_data_types','data_types_without_exact_source','complete_panel_pair_partners']]
  for(const r of report.omitted)rows.push([r.observation,r.patient,r.visit,reasonLabel[r.reason],r.sourcePresent,r.sourceTotal,r.sourceAbsent.join('|'),r.fullPairNeighbors??''])
  return rows.map(row=>row.map(safeCell).join('\t')).join('\n')+'\n'
}

/** Native table filtering/sorting without data duplication or page reload. */
export function bindIntegrativeCoverage(root){
  const body=root.querySelector('#integrative-coverage-body')
  if(!body)return
  const input=root.querySelector('#integrative-coverage-search'),counter=root.querySelector('#integrative-coverage-count')
  const rows=[...body.querySelectorAll('tr')]
  let sortIndex=0,direction=1
  const update=()=>{
    const needle=String(input?.value||'').trim().toLocaleLowerCase()
    const matched=rows.filter(row=>!needle||row.textContent.toLocaleLowerCase().includes(needle))
    matched.sort((a,b)=>{
      const left=a.cells[sortIndex]?.textContent.trim()||'',right=b.cells[sortIndex]?.textContent.trim()||''
      return direction*left.localeCompare(right,undefined,{numeric:true,sensitivity:'base'})
    })
    body.replaceChildren(...matched)
    if(counter)counter.textContent=`${count(matched.length)} of ${count(rows.length)} rows`
  }
  input?.addEventListener('input',update)
  root.querySelectorAll('[data-coverage-sort]').forEach(b=>b.addEventListener('click',()=>{
    const index=Number(b.dataset.coverageSort)
    direction=sortIndex===index?-direction:1;sortIndex=index;update()
  }))
}
