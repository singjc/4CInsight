/** Browser-only pathway panel. Never sends gene identifiers or GMT data to a server. */
import {parseGeneSetsGmt,preparePathwayGenes,runPathwayEnrichment,pathwayInputSignature,pathwayResultsTsv,REACTOME_GMT_URL} from './pathway-enrichment.js?v=0.22.8'
import {readGmtSelection,fetchDefaultReactomeGmt} from './pathway-gmt-loader.js?v=0.22.8'
import {pathwaySelectionKey,pathwayEvidenceBanner,pathwayScoreChartHtml,inspectPathwayGenes,pathwayInspectorHtml,pathwayInspectorTsv} from './pathway-inspector.js?v=0.22.8'

const esc=x=>String(x??'').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const f=x=>typeof x==='number'&&Number.isFinite(x)?(x<0.001?x.toExponential(2):x.toFixed(3)):'—'
const selected=(value,current)=>value===current?' selected':''
const DEFAULT_CONFIG=Object.freeze({source:'compare',mode:'ora',axis:'pcoa1_spearman',minSize:10,maxSize:500,qThreshold:0.05,effectThreshold:1,direction:'both',permutations:199,seed:20261111,organism:'',release:''})
export function defaultPathwayConfig(){return {...DEFAULT_CONFIG}}
export function pathwayPanelHtml({sets,result,config={},status='',compareAvailable=false,exploratoryAvailable=false,sourcePreview=null}={}) {
  const c={...DEFAULT_CONFIG,...config},nSets=sets?.pathways?.length||0
  const ready=(c.source==='compare'?compareAvailable:exploratoryAvailable)
  const compatible=c.mode!=='ora'||c.source==='compare'
  const canRun=nSets>0&&ready&&compatible
  return `<section class="pathway-panel" aria-label="Local pathway enrichment">
    <h5>Run pathway enrichment on this device</h5>
    <p class="method-note">This tool calculates pathway p-values and Benjamini–Hochberg q-values. It uses gene IDs from a local GMT file. <strong>No source data or gene list is uploaded.</strong> Gene-set matching uses uppercase exact identifiers. It does not convert Ensembl IDs to gene symbols.</p>
    <section class="pathway-guided-step" aria-labelledby="pathway-step-question">
      <div class="pathway-step-heading"><span class="pathway-step-index">1</span><div><h6 id="pathway-step-question">Choose your biological question</h6><p>Use a predefined case/control comparison when available, or explore associations with the fitted joint sample structure.</p></div></div>
      <div class="pathway-grid">
      <label>Gene-score source<select id="pathway-source"><option value="compare"${selected('compare',c.source)}>Corrected Compare Groups genes${compareAvailable?'':' (not available)'}</option><option value="exploratory"${selected('exploratory',c.source)}>Exploratory joint PCoA genes${exploratoryAvailable?'':' (not available)'}</option></select></label>
      <label>Test method<select id="pathway-mode"><option value="ora"${selected('ora',c.mode)}>Over-representation (ORA)</option><option value="ranked"${selected('ranked',c.mode)}>Ranked gene-label permutation</option></select></label>
      <label>PCoA axis (exploratory only)<select id="pathway-axis"${c.source==='compare'?' disabled':''}><option value="pcoa1_spearman"${selected('pcoa1_spearman',c.axis)}>PCo1</option><option value="pcoa2_spearman"${selected('pcoa2_spearman',c.axis)}>PCo2</option></select></label>
      </div>
      <p class="method-note">${c.source==='exploratory'?'<strong>Exploratory:</strong> PCoA gene rankings are post-fit. These tests cannot independently validate disease groups or clusters. Select Ranked enrichment.':compareAvailable?'Corrected Compare Groups gene results are available. ORA or ranked enrichment may be selected.':'No corrected gene results. Run Compare Groups first, or choose exploratory PCoA scores when available.'}</p>
      ${c.source==='exploratory'&&c.mode==='ora'?'<p class="analysis-warnings" role="status">ORA requires corrected Compare Groups results. Choose Ranked enrichment for exploratory PCoA scores.</p>':''}
    </section>
    <section class="pathway-guided-step" aria-labelledby="pathway-step-genesets">
      <div class="pathway-step-heading"><span class="pathway-step-index">2</span><div><h6 id="pathway-step-genesets">Choose a pathway database</h6><p>Download the default Reactome gene sets or upload a custom GMT or ZIP file. Both options are processed on your device.</p></div></div>
      <div class="pathway-grid">
      <div class="pathway-gmt-picker"><button type="button" class="secondary-button" id="pathway-default">Download &amp; use default Reactome</button>
        <p class="method-note">Get the current Reactome GMT ZIP after you click. The browser downloads and reads only the gene-set file; it does not upload your samples or gene scores.</p>
        <a href="${REACTOME_GMT_URL}" target="_blank" rel="noopener noreferrer">Download Reactome ZIP manually</a></div>
      <label>Or upload custom gene sets (GMT or ZIP)<input type="file" id="pathway-gmt" accept=".gmt,.txt,.zip,.gmt.zip,application/zip,text/plain,text/tab-separated-values" aria-label="Upload custom GMT or GMT ZIP gene sets"/></label>
      <div class="pathway-set-summary"><strong>${nSets?nSets.toLocaleString()+' gene sets':'No gene sets imported'}</strong><span>${nSets?esc(sets.filename):'Download a GMT file or use one you have.'}</span></div>
      <label>Gene-set organism (user verified)<input id="pathway-organism" type="text" maxlength="120" value="${esc(c.organism)}" placeholder="e.g. Homo sapiens"/></label>
      <label>Gene-set release / date<input id="pathway-release" type="text" maxlength="120" value="${esc(c.release)}" placeholder="e.g. 2026-10"/></label>
      </div>
      ${(!String(c.organism||'').trim()||!String(c.release||'').trim())?'<p class="analysis-warnings pathway-provenance-notice" role="status">Gene-set organism or exact release is not recorded. Confirm both before scientific reporting. You can run an exploratory analysis now; changing this information requires a new pathway run.</p>':'<p class="method-note">Gene-set organism and release have been entered by the user; 4C Insight has not independently verified them.</p>'}
    </section>
    <p class="method-note">You can use a custom <code>.gmt</code> or <code>.zip</code> file. If direct Reactome download is blocked by browser cross-origin rules, use the manual link and select the downloaded ZIP here. Check the organism, gene identifier type, license, and release for every gene-set source. The current Reactome URL is not a pinned release.</p>
    <section class="pathway-guided-step" aria-labelledby="pathway-step-run">
      <div class="pathway-step-heading"><span class="pathway-step-index">3</span><div><h6 id="pathway-step-run">Run and interpret</h6><p>Review the input and method. Calculations begin only when you select Run pathway analysis.</p></div></div>
      <p class="method-note"><strong>Selected input:</strong> ${c.source==='compare'?'Corrected case/control gene results':'Exploratory PCoA gene associations'} · <strong>Method:</strong> ${c.mode==='ora'?'Over-representation analysis (ORA)':'Ranked gene-label permutation enrichment'} · <strong>Gene universe:</strong> ${sourcePreview?.genes?.toLocaleString()||'not available'} unique usable genes · <strong>Gene sets:</strong> ${nSets.toLocaleString()} loaded. The exact tested-gene background is recorded in the results.</p>
    <details class="pathway-options"><summary>Advanced settings · thresholds, pathway sizes and permutations</summary>
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
    <div class="pathway-controls"><button type="button" class="primary-button" id="pathway-run" ${!canRun?'disabled':''}>Run local pathway analysis</button><button type="button" class="secondary-button" id="pathway-stop" hidden>Stop</button><button type="button" class="secondary-button" id="pathway-export"${result?'':' disabled'}>Export full pathway results (TSV)</button><button type="button" class="secondary-button" id="pathway-audit"${result?'':' disabled'}>Export method and provenance (JSON)</button></div>
    </section>
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
    ${(!String(report.organism||'').trim()||!String(report.geneSetRelease||'').trim())?'<p class="analysis-warnings pathway-provenance-notice" role="status"><strong>Source metadata incomplete.</strong> This saved result lacks an organism or pinned gene-set release. Do not use it as a documented scientific result until you verify those fields and rerun enrichment.</p>':''}
    ${pathwayEvidenceBanner(report)}
    ${pathwayScoreChartHtml(report)}
    <div class="pathway-search"><label>Search pathways<input id="pathway-query" type="search" placeholder="Search pathway or gene" autocomplete="off"/></label><span id="pathway-count">${hits.length.toLocaleString()} results</span></div>
    <div class="table-scroll pathway-table"><table><thead><tr><th>Pathway</th><th>Matched genes</th><th>${report.mode==='ora'?'Selected genes':'ES'}</th><th>${report.mode==='ora'?'Fold enrichment':'Normalized ES'}</th><th>p-value</th><th>BH q-value</th><th>Leading / selected genes</th></tr></thead><tbody id="pathway-result-body"></tbody></table></div>
    <section class="pathway-gene-inspector" id="pathway-gene-inspector" aria-label="Selected pathway and exact measured genes" aria-live="polite"></section>
    ${report.warnings?.length?`<details class="pathway-warnings"><summary>Method notes and identifier warnings</summary>${report.warnings.map(w=>`<p class="method-note">${esc(w)}</p>`).join('')}</details>`:''}
  </section>`
}
export function bindPathwayResults(root,report,{selectedIndex=-1,onSelect=()=>{}}={}) {
  const query=root.querySelector('#pathway-query'),body=root.querySelector('#pathway-result-body'),count=root.querySelector('#pathway-count')
  if(!body||!query||!count)return
  const indices=new Map((report.results||[]).map((r,i)=>[r,i]))
  const updatePressed=()=>root.querySelectorAll('.pathway-select').forEach(button=>{
    const active=Number(button.dataset.pathwayIndex)===selectedIndex
    button.setAttribute('aria-pressed',String(active))
    button.classList.toggle('is-selected',active)
  })
  root.querySelector('.pathway-results')?.addEventListener('click',event=>{
    const button=event.target.closest?.('.pathway-select')
    if(!button || !root.contains(button))return
    const index=Number(button.dataset.pathwayIndex)
    if(!Number.isInteger(index)||index<0||index>=report.results.length)return
    selectedIndex=index;updatePressed();onSelect(index)
  })
  const draw=()=>{
    const search=query.value.trim().toLocaleLowerCase()
    const rows=report.results.filter(row=>!search||row.pathway.toLocaleLowerCase().includes(search)||
      row.description.toLocaleLowerCase().includes(search)||row.leadingGenes.some(g=>g.toLocaleLowerCase().includes(search)))
    count.textContent=`${rows.length.toLocaleString()} matching pathways of ${report.results.length.toLocaleString()} tested`
    body.innerHTML=rows.map(row=>`<tr><td><button type="button" class="pathway-table-choice pathway-select" data-pathway-index="${indices.get(row)}" aria-pressed="false">${esc(row.pathway)}</button><small>${esc(row.description)}</small></td><td>${row.overlap} / ${row.matchedSetSize}</td><td>${f(report.mode==='ora'?row.overlap:row.enrichmentScore)}</td><td>${f(report.mode==='ora'?row.foldEnrichment:row.normalizedScore)}</td><td>${f(row.p_value)}</td><td>${f(row.q_value)}</td><td title="${esc(row.leadingGenes.join(', '))}">${esc(row.leadingGenes.slice(0,12).join(', '))}${row.leadingGenes.length>12?' …':''}</td></tr>`).join('')||'<tr><td colspan="7">No pathways match your search.</td></tr>'
  }
  query.addEventListener('input',draw);draw();updatePressed()
}

/** `workspace` is the existing app state; pathway sets/results are cached in browser-local results. */
export function mountPathwayModule(root,{workspace,compare,exploratoryRows,onChanged,exportFile}={}){
  const slot=root.querySelector('#pathway-analysis-widget');if(!slot)return
  const config=workspace.pathwayConfig||defaultPathwayConfig()
  workspace.pathwayConfig=config
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
    let sourcePreview=null
    try {
      const prepared=preparePathwayGenes({compare,exploratoryRows,source:config.source,axis:config.axis})
      sourcePreview={genes:prepared.rows.length}
    } catch (_error) { sourcePreview=null }
    slot.innerHTML=pathwayPanelHtml({sets:workspace.pathwayGeneSets,result:workspace.pathwayResult,
      config,status:workspace.pathwayStatus||'',compareAvailable,exploratoryAvailable,sourcePreview})
    if(optionsOpen){const options=slot.querySelector('.pathway-options');if(options)options.open=true}
    const report=workspace.pathwayResult
    const selected=report?.results?.findIndex(row=>pathwaySelectionKey(row)===workspace.pathwaySelectedKey)??-1
    const initialIndex=selected<0&&report?.results?.length?0:selected
    function inspectSelected(index){
      const target=slot.querySelector('#pathway-gene-inspector')
      if(!target)return
      const row=report?.results?.[index]||null
      let inspection={available:false,reason:'Restore the matching gene ranking and GMT to inspect this pathway.'}
      if(row){
        try {
          const prepared=preparePathwayGenes({compare,exploratoryRows,source:report.source,axis:config.axis})
          if(pathwayInputSignature(prepared,workspace.pathwayGeneSets)===report.inputSignature){
            inspection=inspectPathwayGenes(report,row,prepared,workspace.pathwayGeneSets)
          }
        } catch (_error) { /* Never invent missing genes. */ }
      }
      target.innerHTML=pathwayInspectorHtml(report,row,inspection,{selectedOnly:workspace.pathwayLeadingOnly===true})
      target.querySelector('#pathway-inspector-selected-only')?.addEventListener('change',event=>{
        workspace.pathwayLeadingOnly=event.currentTarget.checked
        inspectSelected(index)
        onChanged?.()
      })
      target.querySelector('#pathway-export-genes')?.addEventListener('click',()=>{
        try {
          exportFile?.('4c-insight-selected-pathway-genes.tsv',pathwayInspectorTsv(report,row,inspection))
        }catch(error){workspace.pathwayStatus=`Pathway gene export failed: ${error.message}`}
      })
    }
    bindPathwayResults(slot,report||{results:[]},{selectedIndex:initialIndex,onSelect:index=>{
      workspace.pathwaySelectedKey=pathwaySelectionKey(report.results[index])
      inspectSelected(index)
      onChanged?.()
    }})
    inspectSelected(initialIndex)
    bind()
  }
  const getConfig=()=>Object.fromEntries([
    ['source','pathway-source'],['mode','pathway-mode'],['axis','pathway-axis'],['minSize','pathway-min-size'],
    ['maxSize','pathway-max-size'],['qThreshold','pathway-q'],['effectThreshold','pathway-effect'],
    ['direction','pathway-direction'],['permutations','pathway-permutations'],['seed','pathway-seed'],
    ['organism','pathway-organism'],['release','pathway-release']].map(([key,id])=>[key,slot.querySelector('#'+id)?.value]))
  function bind(){
    // Keep the gene-set provenance and result cache consistent for both import paths.
    function acceptGeneSets(parsed, message) {
      workspace.pathwayGeneSets=parsed
      workspace.pathwayResult=null
      config.organism='' // The organism of a new GMT must be verified, not inherited.
      config.release='' // A moving 'current' URL is not a pinned database release.
      workspace.pathwayStatus=message+' Verify organism and gene-set release before reporting.'
      onChanged?.();paint()
    }
    slot.querySelector('#pathway-default')?.addEventListener('click',async()=>{
      const button=slot.querySelector('#pathway-default')
      const status=slot.querySelector('#pathway-status')
      button.disabled=true
      if(status)status.textContent='Downloading the official Reactome GMT ZIP. No sample data is sent…'
      try{
        const fetched=await fetchDefaultReactomeGmt()
        const parsed=parseGeneSetsGmt(fetched.text,fetched.filename)
        parsed.source=`Official Reactome current GMT ZIP; retrieved ${fetched.retrievedAt} from ${fetched.url}; release not pinned`
        acceptGeneSets(parsed,`Loaded ${parsed.imported.toLocaleString()} Reactome gene sets.`)
      }catch(error){
        workspace.pathwayStatus=`Default download unavailable: ${error.message}`
        paint() // Preserve the previous gene-set selection and calculated result.
      }
    })
    slot.querySelector('#pathway-gmt')?.addEventListener('change',async event=>{
      const file=event.currentTarget.files?.[0];if(!file)return
      try{
        const loaded=await readGmtSelection(file)
        const parsed=parseGeneSetsGmt(loaded.text,loaded.filename)
        parsed.source=`User-selected local ${loaded.archiveName?'ZIP ('+loaded.archiveName+')':'GMT'}; release and organism supplied by user`
        acceptGeneSets(parsed,`Loaded ${parsed.imported.toLocaleString()} gene sets from ${file.name}. ${parsed.invalidLines?parsed.invalidLines+' invalid rows were skipped.':''}`)
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
      const abort=new AbortController();run.disabled=true;stop.hidden=false;workspace.pathwayBusy=true
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
      workspace.pathwayBusy=false
      onChanged?.()
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
