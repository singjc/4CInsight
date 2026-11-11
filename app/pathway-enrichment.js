/** Local, dependency-free pathway tests. User-provided GMT only; no remote services.
 * ORA: right-tail hypergeometric with an explicit tested-gene universe.
 * Ranked: weighted KS ES and two-sided gene-label permutations, BH correction.
 * BH on empirical gene-label p is NOT the pooled FDR algorithm of Broad GSEA.
 */
const valid = x => typeof x === 'number' && Number.isFinite(x)
export const PATHWAY_SCHEMA = '4c-insight-local-pathway-v1'
export const REACTOME_GMT_URL = 'https://reactome.org/download/current/ReactomePathways.gmt.zip'
const symbol = x => String(x ?? '').trim().toUpperCase()
const numeric = x => (typeof x==='number' && Number.isFinite(x)) ? x :
  (typeof x==='string' && x.trim()!=='' && Number.isFinite(Number(x)) ? Number(x) : NaN)
const label = x => String(x ?? '').trim()
const isFiniteProbability = x => valid(x) && x>=0 && x<=1

/** Parse standard GMT and Reactome's optional third 'Reactome Pathway' metadata column.
 * Ensembl and symbol namespaces are NOT inferred or converted.
 */
export function parseGeneSetsGmt(text, filename='local-gene-sets.gmt') {
  if(typeof text!=='string' || !text.trim()) throw new Error('The GMT file is empty')
  const pathways=[],seen=new Set(),errors=[]
  for(const [i,line] of text.replace(/^\uFEFF/,'').split(/\r?\n/).entries()) {
    if(!line.trim() || line.startsWith('#')) continue
    const cells=line.split('\t'),name=label(cells[0]),description=label(cells[1])
    if(cells.length<3 || !name){errors.push(`Line ${i+1}: invalid GMT columns or missing name`);continue}
    // Reactome GMT includes stable ID followed by a third 'Reactome Pathway' token.
    const start=cells.length>=4 && /^reactome pathway$/i.test(label(cells[2])) ? 3:2
    const genes=[...new Set(cells.slice(start).map(symbol).filter(Boolean))]
    if(!genes.length){errors.push(`Line ${i+1}: pathway has no gene identifiers`);continue}
    const key=`${name}\u0000${description}`
    if(seen.has(key)){errors.push(`Line ${i+1}: duplicate pathway and description`);continue}
    seen.add(key)
    pathways.push({name,description,genes})
  }
  if(!pathways.length) throw new Error(`No valid gene sets found. ${errors[0]||'Expected tab-separated GMT: pathway, description, genes…'}`)
  return {schema:'gmt-v1',filename:label(filename)||'local.gmt',pathways,
    imported:pathways.length,invalidLines:errors.length,warnings:errors.slice(0,8),
    source:'User-selected local GMT file; not an automatic online database update'}
}

/** Deduplicate by identifier; ambiguous genes are removed in their entirety, not averaged. */
export function preparePathwayGenes({compare,exploratoryRows,source='compare',axis='pcoa1_spearman'}={}) {
  if(!['compare','exploratory'].includes(source)) throw new Error('Invalid gene-score source')
  let input,method,provenance,sourceRows=0
  if(source==='compare') {
    if(!compare?.gene?.analysis?.tests?.length) throw new Error('Run corrected Gene expression in Compare Groups first, or choose exploratory PCoA scores')
    input=compare.gene.analysis.tests.map(r=>({gene:r.feature,score:numeric(r.beta_group),q:numeric(r.q_value)}))
    method='Signed log2 case-versus-control effect';provenance=compare.comparisonName||'Compare Groups (unnamed)'
    sourceRows=compare.gene.analysis.tests.length
  } else {
    if(!['pcoa1_spearman','pcoa2_spearman'].includes(axis)) throw new Error('Invalid PCoA axis')
    input=(exploratoryRows||[]).filter(r=>r.kind==='gene').map(r=>({gene:r.feature,score:numeric(r[axis]),q:NaN}))
    method=`Post-fit ${axis} Spearman association (exploratory)`
    provenance='Joint PCoA: in-sample association, not differential expression'
    sourceRows=input.length
  }
  const counts=new Map()
  for(const row of input){const id=symbol(row.gene);if(id)counts.set(id,(counts.get(id)||0)+1)}
  const rows=input.filter(row=>symbol(row.gene)&&counts.get(symbol(row.gene))===1&&valid(row.score)&&
    (source==='exploratory'||isFiniteProbability(row.q)))
    .map(row=>({gene:symbol(row.gene),score:row.score,q:isFiniteProbability(row.q)?row.q:null}))
  rows.sort((a,b)=>b.score-a.score||a.gene.localeCompare(b.gene))
  if(rows.length<3) throw new Error('At least three unique, numeric gene scores are required')
  const diagnostics={totalSourceRows:sourceRows,retainedGenes:rows.length,
    duplicateRows:[...counts].reduce((n,[,count])=>n+(count>1?count:0),0),
    missingInvalidRows:sourceRows-rows.length-[...counts].reduce((n,[,count])=>n+(count>1?count:0),0)}
  return {rows,source,axis:source==='exploratory'?axis:null,method,provenance,diagnostics,
    evidence:source==='compare'?'Predefined case/control association; pathway test is not independent validation':
      'Exploratory post-fit ranking; gene-label p-values do not validate disease groups or clusters'}
}

export function bhAdjust(rows,pKey='p_value') {
  const indices=rows.map((row,i)=>({i,p:row[pKey]})).filter(r=>isFiniteProbability(r.p)).sort((a,b)=>a.p-b.p||a.i-b.i)
  let lower=1
  for(let rank=indices.length;rank>=1;rank--){const item=indices[rank-1];lower=Math.min(lower,item.p*indices.length/rank);rows[item.i].q_value=lower}
  return rows
}
function logs(n){const out=new Float64Array(n+1);for(let i=1;i<=n;i++)out[i]=out[i-1]+Math.log(i);return out}
function logChoose(f,n,k){return k<0||k>n?-Infinity:f[n]-f[k]-f[n-k]}
/** Exact one-sided hypergeometric probability P[X >= observed]. */
export function hypergeometricTail(N,K,n,k,logFac) {
  if(![N,K,n,k].every(Number.isInteger)||N<0||K<0||K>N||n<0||n>N||k<0)throw new Error('Invalid hypergeometric parameters')
  if(k===0)return 1
  const top=Math.min(n,K),bottom=Math.max(k,n-(N-K)),f=logFac||logs(N)
  if(bottom>top)return 0
  const denominator=logChoose(f,N,n)
  let maxLog=-Infinity,terms=[]
  for(let i=bottom;i<=top;i++){
    const log=logChoose(f,K,i)+logChoose(f,N-K,n-i)-denominator
    terms.push(log);if(log>maxLog)maxLog=log
  }
  return Math.min(1,Math.exp(maxLog)*terms.reduce((sum,x)=>sum+Math.exp(x-maxLog),0))
}
function matchedSets(geneSets,ranked,minSize,maxSize) {
  const position=new Map(ranked.map((row,i)=>[row.gene,i]))
  const accepted=[],excluded=[]
  for(const set of geneSets.pathways||[]){
    const positions=[...new Set((set.genes||[]).map(symbol))].map(g=>position.get(g)).filter(i=>i!==undefined).sort((a,b)=>a-b)
    if(positions.length<minSize||positions.length>maxSize||positions.length>=ranked.length){excluded.push({name:set.name,overlap:positions.length});continue}
    accepted.push({name:set.name,description:set.description,positions,genes:positions.map(i=>ranked[i].gene),
      setSize:set.genes.length,overlap:positions.length})
  }
  return {accepted,excluded}
}
function checkOptions(options,source) {
  const mode=options?.mode||'ora',minSize=Number(options?.minSize??10),maxSize=Number(options?.maxSize??500)
  if(!['ora','ranked'].includes(mode))throw new Error('Choose ORA or ranked enrichment')
  if(!Number.isInteger(minSize)||!Number.isInteger(maxSize)||minSize<2||maxSize<minSize)throw new Error('Invalid pathway size range')
  if(mode==='ora'&&source!=='compare')throw new Error('ORA requires corrected Compare Groups results and a defined gene-selection rule')
  const direction=options?.direction||'both'
  if(!['both','up','down'].includes(direction))throw new Error('Invalid ORA effect direction')
  const qThreshold=Number(options?.qThreshold??0.05),effectThreshold=Number(options?.effectThreshold??1)
  if(mode==='ora'&&(!isFiniteProbability(qThreshold)||!valid(effectThreshold)||effectThreshold<0))throw new Error('Invalid gene selection thresholds')
  const permutations=Number(options?.permutations??199),seed=Number(options?.seed??20261111)
  if(mode==='ranked'&&(!Number.isInteger(permutations)||permutations<99||permutations>9999))throw new Error('Permutations must be an integer from 99 to 9999')
  if(!Number.isInteger(seed)||seed<1||seed>4294967295)throw new Error('Seed must be a positive 32-bit integer')
  const organism=String(options?.organism??'').trim().slice(0,120),release=String(options?.release??'').trim().slice(0,120)
  return {mode,minSize,maxSize,qThreshold,effectThreshold,direction,permutations,seed,organism,release}
}
function fnv(text) {let h=2166136261;for(let i=0;i<text.length;i++){h^=text.charCodeAt(i);h=Math.imul(h,16777619)}return h>>>0}
export function pathwayInputSignature(input,sets) {
  let sig=2166136261
  function mix(s){sig^=fnv(s);sig=Math.imul(sig,16777619)}
  for(const row of input.rows||[])mix(`${row.gene}\t${row.score}\t${row.q}`)
  for(const set of sets?.pathways||[])mix(`${set.name}\t${set.description}\t${set.genes.join(',')}`)
  return `${(sig>>>0).toString(16)}-${input.rows.length}-${sets?.pathways?.length||0}`
}
function xorshift(seed){let x=seed>>>0||1;return()=>{x^=x<<13;x^=x>>>17;x^=x<<5;return(x>>>0)/4294967296}}
// Equivalent to permuting gene labels over the full fixed ranked background.
function randomPositions(N,m,rand){const set=new Set();for(let j=N-m;j<N;j++){const t=Math.floor(rand()*(j+1));set.add(set.has(t)?j:t)}return [...set].sort((a,b)=>a-b)}
/** Weighted KS walk with constant-size arithmetic per hit; missed genes are counted by gaps. */
export function enrichmentScore(positions,scores) {
  const N=scores.length,m=positions.length
  if(!m||m>=N)throw new Error('A pathway must match at least one but not all ranked genes')
  let total=0
  for(const p of positions)total+=Math.abs(scores[p])
  const weight=p=>total?Math.abs(scores[p])/total:1/m
  let walk=0,previous=-1,max=-Infinity,min=Infinity,maxAt=-1,minAt=-1
  for(const p of positions){
    walk-=(p-previous-1)/(N-m)
    if(walk<min){min=walk;minAt=p-1}
    if(walk>max){max=walk;maxAt=p-1}
    walk+=weight(p)
    if(walk<min){min=walk;minAt=p}
    if(walk>max){max=walk;maxAt=p}
    previous=p
  }
  // After last hit, the running walk returns to zero.
  if(Math.abs(max)>=Math.abs(min))return {score:max,direction:'positive',peak:maxAt}
  return {score:min,direction:'negative',peak:minAt}
}
export async function runPathwayEnrichment({input,geneSets,options={},onProgress=()=>{},signal}={}){
  if(!input?.rows?.length||!Array.isArray(geneSets?.pathways))throw new Error('Gene results and a local GMT are required')
  const config=checkOptions(options,input.source),N=input.rows.length
  const {accepted,excluded}=matchedSets(geneSets,input.rows,config.minSize,config.maxSize)
  if(!accepted.length)throw new Error('No gene sets meet the overlap size range. Check that the GMT identifiers match the gene results (for example HGNC symbols versus Ensembl IDs).')
  const results=[],scores=input.rows.map(row=>row.score),method=config.mode==='ora'?'Hypergeometric right tail':'Weighted KS, gene-label permutations'
  if(config.mode==='ranked'&&new Set(scores).size<3) throw new Error('The ranked scores have too little variation for enrichment')
  if(config.mode==='ora'){
    const selected=new Set(input.rows.filter(r=>r.q<=config.qThreshold&&Math.abs(r.score)>=config.effectThreshold
      &&(config.direction==='both'||(config.direction==='up'?r.score>0:r.score<0))).map(r=>r.gene))
    if(!selected.size)throw new Error('No genes meet the selected Compare Groups q-value and effect thresholds')
    const n=selected.size,f=logs(N)
    for(const [i,set] of accepted.entries()){
      const hitGenes=set.genes.filter(g=>selected.has(g)),k=hitGenes.length
      results.push({pathway:set.name,description:set.description,p_value:hypergeometricTail(N,set.overlap,n,k,f),
        q_value:null,overlap:k,matchedSetSize:set.overlap,inputSetSize:set.setSize,
        selectedGenes:n,backgroundGenes:N,expected:n*set.overlap/N,
        foldEnrichment:k/(n*set.overlap/N),direction:null,leadingGenes:hitGenes,statistic:k})
      if(i%150===0)onProgress(i+1,accepted.length)
    }
  } else {
    for(const [i,set] of accepted.entries()){
      if(signal?.aborted)throw new Error('Pathway analysis stopped')
      const observed=enrichmentScore(set.positions,scores)
      const rand=xorshift((config.seed^fnv(`${set.name}\t${set.description}`))>>>0)
      let count=0,sameSignSum=0,sameSignN=0
      for(let j=0;j<config.permutations;j++){
        const perm=enrichmentScore(randomPositions(N,set.overlap,rand),scores)
        if(Math.abs(perm.score)>=Math.abs(observed.score)-1e-12)count++
        if(perm.direction===observed.direction){sameSignSum+=Math.abs(perm.score);sameSignN++}
      }
      const leading=set.positions.filter(p=>observed.direction==='positive'?p<=observed.peak:p>=observed.peak).map(p=>input.rows[p].gene)
      results.push({pathway:set.name,description:set.description,p_value:(count+1)/(config.permutations+1),q_value:null,
        overlap:set.overlap,matchedSetSize:set.overlap,inputSetSize:set.setSize,selectedGenes:null,backgroundGenes:N,
        direction:observed.direction,enrichmentScore:observed.score,
        normalizedScore:sameSignN>0?observed.score/(sameSignSum/sameSignN):null,
        leadingGenes:leading,statistic:observed.score})
      if((i+1)%12===0||i===accepted.length-1){onProgress(i+1,accepted.length);await new Promise(resolve=>setTimeout(resolve,0))}
    }
  }
  bhAdjust(results)
  results.sort((a,b)=>a.q_value-b.q_value||a.p_value-b.p_value||a.pathway.localeCompare(b.pathway))
  return {schema:PATHWAY_SCHEMA,mode:config.mode,method,source:input.source,sourceMethod:input.method,
    provenance:input.provenance,evidence:input.evidence,geneSetFile:geneSets.filename,
    geneSetSource:geneSets.source,organism:config.organism,geneSetRelease:config.release,
    geneSetCount:geneSets.pathways.length,matchedSetCount:accepted.length,
    excludedSetCount:excluded.length,backgroundGenes:N,selectedGenes:config.mode==='ora'?results[0]?.selectedGenes:null,
    options:config,inputDiagnostics:input.diagnostics,rankMetric:input.method,
    correction:'Benjamini–Hochberg over all tested sets; empirical ranked p-values from gene-label permutations, not phenotype permutations or Broad GSEA pooled FDR',
    warnings:[...geneSets.warnings,
      'Gene identifiers are matched by uppercase exact text, not orthology or identifier conversion. Missing identifiers are not inferred.',
      ...(!config.organism||!config.release?['Organism or gene-set release is missing. Record both before scientific reporting.']:[]),
      'Pathway tests are dependent because gene sets share genes. Adjusted pathway q-values do not imply independent validation.',
      ...(input.source==='exploratory'?['PCoA ranking is in-sample and post-fit. Enrichment is exploratory even if permutation p/q values are small.']:[])],
    results}
}
export function pathwayResultsTsv(report) {
  const headers=['pathway','description','mode','source','overlap','matchedSetSize','backgroundGenes','selectedGenes','direction','enrichmentScore','normalizedScore','foldEnrichment','p_value','q_value','leadingGenes']
  const safe=v=>{const str=String(v??'');return /^\s*[=+@]/.test(str)||/^\s*-(?!\d)/.test(str)?"'"+str:str}
  return [headers,...(report?.results||[]).map(row=>headers.map(h=>h==='mode'?report.mode:h==='source'?report.source:h==='leadingGenes'?row.leadingGenes.join(';'):row[h]??''))]
    .map(row=>row.map(x=>safe(x).replaceAll('\t',' ').replaceAll('\r',' ').replaceAll('\n',' ')).join('\t')).join('\n')+'\n'
}
