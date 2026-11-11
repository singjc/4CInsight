/** Browser-only pathway panel. Never sends gene identifiers or GMT data to a server. */
import {parseGeneSetsGmt,preparePathwayGenes,runPathwayEnrichment,pathwayInputSignature,pathwayResultsTsv,REACTOME_GMT_URL} from './pathway-enrichment.js?v=0.22.8'

const esc=x=>String(x??'').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const f=x=>typeof x==='number'&&Number.isFinite(x)?(x<0.001?x.toExponential(2):x.toFixed(3)):'—'
const selected=(value,current)=>value===current?' selected':''
const DEFAULT_CONFIG=Object.freeze({source:'compare',mode:'ora',axis:'pcoa1_spearman',minSize:10,maxSize:500,qThreshold:0.05,effectThreshold:1,direction:'both',permutations:199,seed:20261111,organism:'',release:''})
export function defaultPathwayConfig(){return {...DEFAULT_CONFIG}}
export function pathwayPanelHtml({sets,result,config={},status='',compareAvailable=false,exploratoryAvailable=false}={}) {
  const c={...DEFAULT_CONFIG,...config},nSets=sets?.pathways?.length||0
  return `<section class="pathway-panel" aria-label="Local pathway enrichment">
    <h5>Run pathway enrichment on this device</h5>
    <p class="method-note">This tool calculates pathway p-values and Benjamini–Hochberg q-values. It uses gene IDs from a local GMT file. <strong>No source data or gene list is uploaded.</strong> Gene-set matching uses uppercase exact identifiers. It does not convert Ensembl IDs to gene symbols.</p>
    <div class="pathway-grid">
      <label>Gene-set file (GMT)<input type="file" id="pathway-gmt" accept=".gmt,.txt,text/plain,text/tab-separated-values" aria-label="Import local GMT gene-set file"/></label>
      <div class="pathway-set-summary"><strong>${nSets?nSets.toLocaleString()+' gene sets':'No gene sets imported'}</strong><span>${nSets?esc(sets.filename):'Download a GMT file or use one you have.'}</span></div>
      <label>Gene-set organism (user verified)<input id="pathway-organism" type="text" maxlength="120" value="${esc(c.organism)}" placeholder="e.g. Homo sapiens"/></label>
      <label>Gene-set release / date<input id="pathway-release" type="text" maxlength="120" value="${esc(c.release)}" placeholder="e.g. 2026-10"/></label>
      <label>Gene-score source<select id="pathway-source"><option value="compare"${selected('compare',c.source)}>Corrected Compare Groups genes${compareAvailable?'':' (not available)'}</option><option value="exploratory"${selected('exploratory',c.source)}>Exploratory joint PCoA genes${exploratoryAvailable?'':' (not available)'}</option></select></label>
      <label>Test method<select id="pathway-mode"><option value="ora"${selected('ora',c.mode)}>Over-representation (ORA)</option><option value="ranked"${selected('ranked',c.mode)}>Ranked gene-label permutation</option></select></label>
      <label>PCoA axis (exploratory only)<select id="pathway-axis"${c.source==='compare'?' disabled':''}><option value="pcoa1_spearman"${selected('pcoa1_spearman',c.axis)}>PCo1</option><option value="pcoa2_spearman"${selected('pcoa2_spearman',c.axis)}>PCo2</option></select></label>
    </div>
    <p class="method-note"><a href="${REACTOME_GMT_URL}" target="_blank" rel="noopener noreferrer">Download Reactome's GMT ZIP</a> from its official site. Extract the <code>.gmt</code> file and select it above. You can also use a GMT from another gene-set provider. Confirm its organism, identifier type, license, and release before analysis. The link opens only when clicked; no analysis data is sent.</p>
    <details class="pathway-options"><summary>Pathway test settings and statistical interpretation</summary>
      <div class="pathway-grid">
        <label>Minimum matched genes in pathway<input id="pathway-min-size" type="number" min="2" step="1" value="${esc(c.minSize)}"/></label>
        <label>Maximum matched genes in pathway<input id="pathway-max-size" type="number" min="2" step="1" value="${esc(c.maxSize)}"/></label>
        <label>Gene q-value threshold (ORA)<input id="pathway-q" type="number" min="0" max="1" step="0.01" value="${esc(c.qThreshold)}"${c.mode==='ora'?'':' disabled'}/></label>
        <label>Effect direction (ORA)<select id="pathway-direction"${c.mode==='ora'?'':' disabled'}><option value="both"${selected('both',c.direction)}>Both directions</option><option value="up"${selected('up',c.direction)}>Higher in case group</option><option value="down"${selected('down',c.direction)}>Lower in case group</option></select></label>
        <label>Minimum absolute log2 effect (ORA)<input id="pathway-effect" type="number" min="0" step="0.1" value="${esc(c.effectThreshold)}"${c.mode==='ora'?'':' disabled'}/></label>
        <label>Gene-label permutations (ranked)<input id="pathway-permutations" type="number" min="99" max="9999" step="1" value="${esc(c.permutations)}"${c.mode==='ranked'?'':' disabled'}/></label>
        <label>Reproducible seed<input id="pathway-seed" type="number" min="1" max="4294967295" step="1" value="${esc(c.seed)}"/></label>
      </div>
      <p class="method-note"><strong>ORA:</strong> One-sided hypergeometric test. The background is every unique tested gene with a usable result, not only genes in the GMT. Selected genes require the corrected q-value and log2 group-effect thresholds. <strong>Ranked:</strong> weighted running-sum enrichment with shuffled gene labels over all available scores. Two-sided empirical p-values are corrected with BH across the tested sets. These are not phenotype permutations or the pooled FDR method of Broad GSEA. Overlapping pathways are not independent.</p>
    </details>
    <div class="pathway-controls"><button type="button" class="primary-button" id="pathway-run" ${!nSets?'disabled':''}>Run local pathway analysis</button><button type="button" class="secondary-button" id="pathway-stop" hidden>Stop</button><button type="button" class="secondary-button" id="pathway-export"${result?'':' disabled'}>Export full pathway results (TSV)</button><button type="button" class="secondary-button" id="pathway-audit"${result?'':' disabled'}>Export method and provenance (JSON)</button></div>
    <p id="pathway-status" role="status" aria-live="polite" class="pathway-status">${esc(status|| (result?`Last calculation: ${result.matchedSetCount} tested pathways from ${result.backgroundGenes} genes.`:'Import a local GMT to start.'))}</p>
    ${result?pathwayResultsHtml(result):'<p class="method-note">No pathway test has been run in this active session. Ranked PCoA results remain exploratory even after permutation testing. Independent cohort validation is still required.</p>'}
  </section>`
}
export function pathwayResultsHtml(report) {
  const hits=report.results||[],qCount=hits.filter(row=>row.q_value<=0.05).length
  const top=hits.slice(0,12).filter(row=>row.q_value>0 || row.q_value===0)
  const max=top.length?Math.max(...top.map(row=>Math.min(15,-Math.log10(Math.max(row.q_value,1e-15)))),1):1
  return `<section class="pathway-results" aria-label="Calculated pathway results"><div class="pathway-summary">
    <div><small>Gene background</small><strong>${report.backgroundGenes.toLocaleString()}</strong></div>
    <div><small>Sets tested</small><strong>${hits.length.toLocaleString()}</strong></div>
    <div><small>BH q ≤ 0.05</small><strong>${qCount.toLocaleString()}</strong></div>
    <div><small>Evidence</small><strong>${report.source==='exploratory'?'Exploratory':'Defined comparison'}</strong></div></div>
    <p class="method-note"><strong>Input:</strong> ${esc(report.provenance)} · ${esc(report.geneSetFile)} · ${esc(report.organism||"Organism not recorded")} · ${esc(report.geneSetRelease||"Gene-set release not recorded")} · ${esc(report.method)}. ${esc(report.correction)}</p>
    <h6>Leading pathways · −log10(BH q)</h6>
    <div class="pathway-bars">${top.map(row=>`<div class="pathway-bar-row" title="${esc(row.pathway)}: q=${esc(row.q_value)}"><span>${esc(row.pathway)}</span><div class="pathway-bar-track"><div style="width:${(Math.min(15,-Math.log10(Math.max(row.q_value,1e-15)))/max*100).toFixed(2)}%"></div></div><small>${f(row.q_value)}</small></div>`).join('')}</div>
    <div class="pathway-search"><label>Search pathways<input id="pathway-query" type="search" placeholder="Search pathway or gene" autocomplete="off"/></label><span id="pathway-count">${hits.length.toLocaleString()} results</span></div>
    <div class="table-scroll pathway-table"><table><thead><tr><th>Pathway</th><th>Matched genes</th><th>${report.mode==='ora'?'Selected genes':'ES'}</th><th>${report.mode==='ora'?'Fold enrichment':'Normalized ES'}</th><th>p-value</th><th>BH q-value</th><th>Leading / selected genes</th></tr></thead><tbody id="pathway-result-body"></tbody></table></div>
    ${report.warnings?.length?`<details class="pathway-warnings"><summary>Method notes and identifier warnings</summary>${report.warnings.map(w=>`<p class="method-note">${esc(w)}</p>`).join('')}</details>`:''}
  </section>`
}
export function bindPathwayResults(root,report) {
  const query=root.querySelector('#pathway-query'),body=root.querySelector('#pathway-result-body'),count=root.querySelector('#pathway-count')
  if(!body||!query||!count)return
  const draw=()=>{
    const search=query.value.trim().toLocaleLowerCase()
    const rows=report.results.filter(row=>!search||row.pathway.toLocaleLowerCase().includes(search)||
      row.description.toLocaleLowerCase().includes(search)||row.leadingGenes.some(g=>g.toLocaleLowerCase().includes(search)))
    count.textContent=`${rows.length.toLocaleString()} matching pathways of ${report.results.length.toLocaleString()} tested`
    body.innerHTML=rows.map(row=>`<tr><td><strong>${esc(row.pathway)}</strong><small>${esc(row.description)}</small></td><td>${row.overlap} / ${row.matchedSetSize}</td><td>${f(report.mode==='ora'?row.overlap:row.enrichmentScore)}</td><td>${f(report.mode==='ora'?row.foldEnrichment:row.normalizedScore)}</td><td>${f(row.p_value)}</td><td>${f(row.q_value)}</td><td title="${esc(row.leadingGenes.join(', '))}">${esc(row.leadingGenes.slice(0,12).join(', '))}${row.leadingGenes.length>12?' …':''}</td></tr>`).join('')||'<tr><td colspan="7">No pathways match your search.</td></tr>'
  }
  query.addEventListener('input',draw);draw()
}

/** `workspace` is the existing app state; pathway sets/results are cached in browser-local results. */
export function mountPathwayModule(root,{workspace,compare,exploratoryRows,onChanged,exportFile}={}){
  const slot=root.querySelector('#pathway-analysis-widget');if(!slot)return
  const config=workspace.pathwayConfig||defaultPathwayConfig()
  const compareAvailable=Boolean(compare?.gene?.analysis?.tests?.length)
  const exploratoryAvailable=Boolean(exploratoryRows?.some(r=>r.kind==='gene'))
  // Active-session results must not be shown with a changed gene ranking or GMT.
  // This guard also prevents stale output after a browser-local session restore.
  if (workspace.pathwayResult) {
    try {
      const source=workspace.pathwayResult.source
      const input=preparePathwayGenes({compare,exploratoryRows,source,axis:config.axis})
      if (!workspace.pathwayGeneSets || pathwayInputSignature(input,workspace.pathwayGeneSets)!==workspace.pathwayResult.inputSignature ||
          workspace.pathwayResult.mode!==config.mode || source!==config.source) {
        workspace.pathwayResult=null
        workspace.pathwayStatus='Source results or settings changed. Run the pathway test again.'
      }
    } catch(error) {
      workspace.pathwayResult=null
      workspace.pathwayStatus='The prior pathway inputs are not available. Reconnect or rerun the source analysis.'
    }
  }
  function paint(){
    const optionsOpen=Boolean(slot.querySelector('.pathway-options')?.open)
    slot.innerHTML=pathwayPanelHtml({sets:workspace.pathwayGeneSets,result:workspace.pathwayResult,
      config,status:workspace.pathwayStatus||'',compareAvailable,exploratoryAvailable})
    if(optionsOpen){const options=slot.querySelector('.pathway-options');if(options)options.open=true}
    bindPathwayResults(slot,workspace.pathwayResult||{results:[]})
    bind()
  }
  const getConfig=()=>Object.fromEntries([
    ['source','pathway-source'],['mode','pathway-mode'],['axis','pathway-axis'],['minSize','pathway-min-size'],
    ['maxSize','pathway-max-size'],['qThreshold','pathway-q'],['effectThreshold','pathway-effect'],
    ['direction','pathway-direction'],['permutations','pathway-permutations'],['seed','pathway-seed'],
    ['organism','pathway-organism'],['release','pathway-release']].map(([key,id])=>[key,slot.querySelector('#'+id)?.value]))
  function bind(){
    slot.querySelector('#pathway-gmt')?.addEventListener('change',async event=>{
      const file=event.currentTarget.files?.[0];if(!file)return
      try{
        if(/\.zip$/i.test(file.name))throw new Error('Extract the GMT file from the ZIP archive before import')
        const parsed=parseGeneSetsGmt(await file.text(),file.name)
        workspace.pathwayGeneSets=parsed;workspace.pathwayResult=null
        workspace.pathwayStatus=`Loaded ${parsed.imported.toLocaleString()} gene sets. ${parsed.invalidLines?parsed.invalidLines+' invalid rows were skipped.':''}`
        onChanged?.();paint()
      }catch(error){workspace.pathwayStatus=`GMT import failed: ${error.message}`;paint()}
    })
    for(const id of ['pathway-source','pathway-mode','pathway-axis','pathway-min-size','pathway-max-size','pathway-q','pathway-effect','pathway-direction','pathway-permutations','pathway-seed','pathway-organism','pathway-release']){
      slot.querySelector('#'+id)?.addEventListener('change',()=>{
        const raw=getConfig()
        for(const [key,value] of Object.entries(raw))if(value!==undefined)config[key]=value
        // Preserve the current report only when its input configuration remains unchanged.
        workspace.pathwayResult=null;workspace.pathwayStatus='Settings changed. Run pathway analysis again.'
        onChanged?.();setTimeout(paint,0) // Do not replace the focused input during its blur/change event.
      })
    }
    slot.querySelector('#pathway-run')?.addEventListener('click',async()=>{
      const run=slot.querySelector('#pathway-run'),stop=slot.querySelector('#pathway-stop'),status=slot.querySelector('#pathway-status')
      const abort=new AbortController();run.disabled=true;stop.hidden=false
      stop.addEventListener('click',()=>abort.abort(),{once:true})
      try{
        const input=preparePathwayGenes({compare,exploratoryRows,source:config.source,axis:config.axis})
        const signature=pathwayInputSignature(input,workspace.pathwayGeneSets)
        status.textContent=`Matched ${input.rows.length.toLocaleString()} unique genes. Testing pathways…`
        const report=await runPathwayEnrichment({input,geneSets:workspace.pathwayGeneSets,options:config,signal:abort.signal,
          onProgress:(n,total)=>{if(status)status.textContent=`Testing ${n.toLocaleString()} of ${total.toLocaleString()} gene sets locally…`}})
        report.inputSignature=signature
        workspace.pathwayResult=report
        workspace.pathwayStatus=`Tested ${report.matchedSetCount.toLocaleString()} gene sets; ${report.results.filter(r=>r.q_value<=0.05).length} with BH q ≤ 0.05.`
        onChanged?.()
      }catch(error){workspace.pathwayStatus=`Pathway analysis stopped or failed: ${error.message}`}
      paint()
    })
    slot.querySelector('#pathway-export')?.addEventListener('click',()=>{
      if(!workspace.pathwayResult)return
      exportFile?.('4c-insight-local-pathway-results.tsv',pathwayResultsTsv(workspace.pathwayResult))
    })
    slot.querySelector('#pathway-audit')?.addEventListener('click',()=>{
      if(!workspace.pathwayResult)return
      const report=workspace.pathwayResult
      const audit={...report,results:undefined,schema:'4c-insight-pathway-method-audit-v1',
        generatedAt:new Date().toISOString(),identifierMapping:'exact uppercase strings; no alias or orthology inference',
        independentValidation:false,privacy:'All calculations on device; user-selected local GMT'}
      exportFile?.('4c-insight-pathway-method-audit.json',JSON.stringify(audit,null,2)+'\n')
    })
  }
  paint()
}
