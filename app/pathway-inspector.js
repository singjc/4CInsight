/** Read-only pathway interpretation. Values must come from a calculated report, exact GMT,
 * and the original scored gene rows. No scoring, new p-values or external requests.
 */
const txt = value => String(value ?? '')
const esc = value => txt(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const fmt = value => typeof value==='number'&&Number.isFinite(value)?(Math.abs(value)<0.001&&value!==0?value.toExponential(2):value.toFixed(3)):'—'
const gene = value => txt(value).trim().toUpperCase()
export const pathwaySelectionKey = row => `${txt(row?.pathway)}\u0000${txt(row?.description)}`

/** Distinguish exploratory PCoA enrichment from corrected predefined comparisons. */
export function pathwayEvidenceBanner(report) {
  if (!report) return ''
  const exploratory=report.source==='exploratory'
  const n=Number(report.options?.permutations)
  const resolution=report.mode==='ranked'&&Number.isInteger(n)&&n>0
    ?`With ${n.toLocaleString()} permutations, the minimum empirical p-value is ${fmt(1/(n+1))}. Several pathways can have the same adjusted q-value.`:''
  return `<aside class="pathway-evidence-banner" aria-label="Pathway evidence and limits">
    <strong>${exploratory?'Exploratory joint sample pattern':'Predefined comparison'} · ${report.mode==='ranked'?'Ranked gene-label permutations':'Over-representation analysis'}</strong>
    <p>${exploratory?'Genes were ranked against a fitted PCoA axis. These associations do not establish disease pathways or independent cluster validation. Axis orientation has no biological direction.':'Pathways are associated with the saved, predefined group comparison. Enrichment does not establish a causal mechanism or independent validation.'}</p>
    <p>${esc(resolution)}</p>
    <small>Gene sets: ${esc(report.geneSetFile||'Not recorded')} · Organism: ${esc(report.organism||'Not recorded')} · Release: ${esc(report.geneSetRelease||'Not recorded')}</small>
  </aside>`
}

/** A score-scale chart rather than near-identical q-value bars. No invented effect directions. */
export function pathwayScoreChartHtml(report) {
  const rows=(report?.results||[]).slice(0,12)
  const ranked=report?.mode==='ranked'
  const scoreOf=row=>ranked?row.normalizedScore:row.foldEnrichment
  const max=Math.max(1e-10,...rows.map(r=>Math.abs(Number(scoreOf(r)))||0))
  return `<div class="pathway-score-chart" aria-label="Leading measured pathway scores">
    <h6>${ranked?'Signed normalized enrichment score (NES)':'Fold enrichment (selected / expected)'}</h6>
    <p class="method-note">Select a pathway to inspect its measured genes. BH q-values are shown separately; equal q-values do not imply equal effect scores.</p>
    ${rows.map((row,i)=>{
      const v=scoreOf(row),finite=typeof v==='number'&&Number.isFinite(v)
      const ratio=finite?Math.min(100,Math.abs(v)/max*100):0
      const score=finite?fmt(v):'—'
      const negative=ranked&&finite&&v<0
      return `<button type="button" class="pathway-chart-choice pathway-select" data-pathway-index="${i}" aria-pressed="false" title="Inspect ${esc(row.pathway)}; score ${esc(score)}; BH q ${esc(fmt(row.q_value))}">
        <span class="pathway-chart-name">${esc(row.pathway)}</span>
        <span class="pathway-score-track ${ranked?'pathway-score-track-signed':''}">
          <span class="pathway-score-bar ${negative?'negative':'positive'}" style="width:${(ranked?ratio/2:ratio).toFixed(2)}%"></span>
        </span><span class="pathway-score-value">${esc(score)}</span><span class="pathway-score-q">q ${esc(fmt(row.q_value))}</span>
      </button>`
    }).join('')}
  </div>`
}

/** Use exact gene identifiers only. A missing set or score source is an explicit unavailable result. */
export function inspectPathwayGenes(report,row,input,geneSets) {
  if(!report||!row) return {available:false,reason:'No calculated pathway is selected.'}
  if(!Array.isArray(input?.rows)||!Array.isArray(geneSets?.pathways))
    return {available:false,reason:'The original gene ranking or GMT is not available. Restore the source results before inspecting genes.'}
  const matches=geneSets.pathways.filter(s=>s.name===row.pathway&&s.description===row.description)
  if(matches.length!==1)return {available:false,reason:'The matching GMT pathway is missing or ambiguous. Gene membership cannot be reconstructed safely.'}
  const scored=new Map(input.rows.map((g,i)=>[gene(g.gene),{...g,rank:i+1}]))
  const leading=new Set((row.leadingGenes||[]).map(gene))
  const entries=[...new Set(matches[0].genes.map(gene))].map(id=>scored.get(id))
    .filter(Boolean).map(g=>({gene:gene(g.gene),rank:g.rank,score:g.score,q:g.q,selected:leading.has(gene(g.gene))}))
  // Put saved leading-edge / ORA-selected genes first, then preserve the original measured rank.
  entries.sort((a,b)=>Number(b.selected)-Number(a.selected)||a.rank-b.rank||a.gene.localeCompare(b.gene))
  // In ranked analysis, 'selected' means leading edge, not statistically significant.
  // In ORA, 'selected' means that the saved ORA threshold selected this gene.
  return {available:true,rows:entries,matched:entries.length,inputSetSize:matches[0].genes.length,
    selectedCount:entries.filter(g=>g.selected).length,selectionLabel:report.mode==='ranked'?'Leading edge':'Selected by ORA thresholds'}
}

export function pathwayInspectorHtml(report,row,inspection,{selectedOnly=false}={}) {
  if(!row)return '<div class="pathway-gene-inspector-empty">Select a pathway in the chart or table to inspect its contributing genes.</div>'
  const ranked=report.mode==='ranked',exploratory=report.source==='exploratory'
  const shown=inspection?.available&&selectedOnly?inspection.rows.filter(g=>g.selected):inspection?.rows||[]
  const label=ranked?'Ranking score': 'Group log2 effect' 
  const scoreLabel=exploratory?'Spearman correlation with fitted PCoA axis':label
  const description=ranked
    ?'Leading-edge genes contribute to this enrichment score. They are not necessarily differentially expressed or statistically significant.'
    :'Selected genes met the saved q-value, effect-size, and direction thresholds. Pathway association is not independent validation.'
  return `<div class="pathway-inspector-inner"><div class="pathway-inspector-heading">
      <div><h6>${esc(row.pathway)}</h6><p class="method-note">Pathway identifier: ${esc(row.description||'Not recorded')}</p></div>
      <div class="pathway-inspector-stats"><span>p ${esc(fmt(row.p_value))}</span><span>BH q ${esc(fmt(row.q_value))}</span><span>${ranked?'NES':'Fold enrichment'} ${esc(fmt(ranked?row.normalizedScore:row.foldEnrichment))}</span></div>
    </div>
    <p class="method-note">${description} ${exploratory?'The direction of PCoA is arbitrary; correlations do not identify upregulated or downregulated genes.':''}</p>
    ${!inspection?.available?`<p class="analysis-warnings" role="status">${esc(inspection?.reason||'Gene measurements are not available.')}</p>`:
      `<p class="method-note"><strong>${inspection.matched.toLocaleString()}</strong> genes matched the scored background; <strong>${inspection.selectedCount.toLocaleString()}</strong> are ${ranked?'in the leading edge':'selected by the ORA thresholds'}. Matched genes are not automatically significant.</p>
      <div class="pathway-inspector-controls"><label><input type="checkbox" id="pathway-inspector-selected-only" ${selectedOnly?'checked':''}/> ${ranked?'Show leading-edge genes only':'Show ORA-selected genes only'}</label>
        <span role="status">Showing ${shown.length.toLocaleString()} of ${inspection.rows.length.toLocaleString()} measured pathway genes</span></div>
      <div class="table-scroll pathway-inspector-table"><table><thead><tr><th>Gene</th><th>Rank</th><th>${esc(scoreLabel)}</th><th>${exploratory?'Gene q-value unavailable':'Corrected gene q-value'}</th><th>${ranked?'Leading edge':'ORA selected'}</th></tr></thead>
      <tbody>${shown.map(g=>`<tr><td>${esc(g.gene)}</td><td>${g.rank}</td><td>${fmt(g.score)}</td><td>${fmt(g.q)}</td><td>${g.selected?'Yes':'No'}</td></tr>`).join('')||'<tr><td colspan="5">No selected genes in this pathway.</td></tr>'}</tbody></table></div>
      <button type="button" class="secondary-button" id="pathway-export-genes">Export all exact matched genes (TSV)</button>`}
    </div>`
}

const safeTsv=value=>{
  const clean=txt(value).replaceAll(/[\r\n\t]/g,' ')
  return /^\s*[=+@]/.test(clean)||/^\s*-(?!\d)/.test(clean)?"'"+clean:clean
}
export function pathwayInspectorTsv(report,row,inspection) {
  if(!report||!row||!inspection?.available)throw new Error('Select a pathway with exact available gene measurements')
  const columns=['pathway','pathway_id','source','method','gene','rank','score','corrected_gene_q','leading_edge_or_ora_selected']
  return [columns.join('\t'),...inspection.rows.map(g=>[row.pathway,row.description,report.source,report.method,g.gene,g.rank,g.score,g.q??'',g.selected?'yes':'no'].map(safeTsv).join('\t'))].join('\n')+'\n'
}
