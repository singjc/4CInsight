/** Local-only exploratory association of merged TPM measurements with already-fitted PCoA.
 * Post-selection correlation and cluster contrast. No p-values, FDR, validation or pathway claims.
 * Streams one text record at a time; results hold one score per feature, not the full input matrix.
 */
import {parseTsvHeader} from './multiomics-alignment.js?v=0.22.7'
import {spearman} from './biological-findings.js?v=0.22.7'

const validNumber=x=>typeof x==='number'&&Number.isFinite(x)
const exact=x=>String(x??'').trim()

/** Worker-safe async line reader with RFC-style quoted TSV fields. */
export async function* tsvRecords(file,{signal,onBytes}={}){
  if(typeof file?.stream!=='function')throw new Error('Select a local merged TSV file')
  const stream=file.stream(),reader=stream.getReader(),decode=new TextDecoder('utf-8',{fatal:true})
  let buffer='',quoted=false,bytes=0
  try{
    while(true){
      if(signal?.aborted)throw new Error('Feature reading stopped')
      const next=await reader.read();if(next.done)break
      bytes+=next.value.byteLength
      const text=decode.decode(next.value,{stream:true})
      for(let i=0;i<text.length;i++){
        const ch=text[i]
        if(ch==='"')quoted=!quoted
        if(ch==='\n'&&!quoted){
          const record=buffer.endsWith('\r')?buffer.slice(0,-1):buffer
          buffer=''
          if(record)yield parseTsvHeader(record)
        }else buffer+=ch
      }
      if(onBytes)onBytes(bytes,Number(file.size)||bytes)
    }
    buffer+=decode.decode()
    if(quoted)throw new Error('Unterminated quotation in merged TSV')
    if(buffer){yield parseTsvHeader(buffer.endsWith('\r')?buffer.slice(0,-1):buffer)}
  }finally{try{await reader.cancel()}catch{}try{reader.releaseLock()}catch{}}
}

/** Exact sample aliases; post-fit annotations are not an input to original distance model. */
export function selectFeatureSamples(structure,alignment,modality){
  if(!['gene','transcript'].includes(modality))throw new Error('Feature-level scores currently support gene and transcript TPM only')
  const rows=new Map(alignment?.rows?.filter(r=>!r.excluded).map(r=>[r.observation,r])||[])
  const selected=[],used=new Set()
  for(const point of structure?.pcoa?.points||[]){
    const source=rows.get(point.observation)?.modalities?.[modality]
    if(source?.status!=='present')continue
    const alias=exact(source.alias)
    if(!alias||used.has(alias))throw new Error(`Missing or duplicate ${modality} source ID`)
    used.add(alias)
    selected.push({alias,observation:point.observation,x:point.x,y:point.y})
  }
  if(selected.length<8)throw new Error(`At least eight exact ${modality} samples must have TPM values`)
  return selected
}
function mean(xs){return xs.reduce((a,b)=>a+b,0)/xs.length}
/** Score all features with sufficient observed values; no missing value is imputed. */
export async function rankExpressionFeatures(file,{modality,samples,clusterByObservation={},onProgress=()=>{},signal}={}){
  if(!['gene','transcript'].includes(modality))throw new Error('Unsupported feature table type')
  if(!Array.isArray(samples)||samples.length<8)throw new Error('At least eight selected sample IDs are required')
  const headerName=modality==='gene'?'Gene Symbol':'Transcript Name'
  let header=null,idx=[],scanned=0,skipped=0,usable=0,rows=[],profiles=[]
  const usedNames=new Set(samples.map(s=>s.alias))
  if(usedNames.size!==samples.length)throw new Error('Repeated source aliases cannot be scored')
  let lastReport=0
  for await(const record of tsvRecords(file,{signal,onBytes:(completed,total)=>{
    if(completed-lastReport>1024*1024||completed===total){lastReport=completed;onProgress(completed,total,`${modality}: read ${(completed/(1024*1024)).toFixed(1)} MiB, scored ${usable} features`)}}
  })){
    if(!header){
      header=record
      if(header[0]!==headerName||header[1]!=='NCBI Gene ID')throw new Error(`Selected file is not a 4C merged ${modality} TSV`)
      idx=samples.map(s=>header.indexOf(`${s.alias} TPM`))
      if(idx.some(i=>i<0))throw new Error(`Missing exact ${modality} TPM column for ${samples.filter((s,i)=>idx[i]<0).slice(0,4).map(s=>s.alias).join(', ')}`)
      if(new Set(idx).size!==idx.length)throw new Error('Repeated TPM columns are ambiguous')
      continue
    }
    scanned++
    if(record.length!==header.length){skipped++;continue}
    const x=[],y=[],values=[],groups=[],observations=[]
    for(let k=0;k<samples.length;k++){
      const raw=record[idx[k]].trim()
      if(raw==='')continue
      const value=Number(raw)
      if(!validNumber(value)||value<0)continue
      values.push(Math.log2(1+value));x.push(samples[k].x);y.push(samples[k].y)
      groups.push(clusterByObservation[samples[k].observation]??null)
      observations.push(samples[k].observation)
    }
    const required=Math.max(8,Math.ceil(samples.length*0.7))
    if(values.length<required){skipped++;continue}
    const rho1=spearman(values,x),rho2=spearman(values,y)
    if(rho1===null&&rho2===null){skipped++;continue}
    const label=exact(record[0]),secondary=exact(record[1])
    if(!label&&!secondary){skipped++;continue}
    // In-sample descriptive contrast for first cluster vs other fitted clusters.
    const inCluster=values.filter((v,i)=>groups[i]===1),outCluster=values.filter((v,i)=>groups[i]!==null&&groups[i]!==1)
    const delta=inCluster.length>=3&&outCluster.length>=3?mean(inCluster)-mean(outCluster):null
    const scored={feature:label||secondary,secondary_id:secondary,n_used:values.length,
      pcoa1_spearman:rho1,pcoa2_spearman:rho2,cluster1_log2_tpm_difference:delta}
    rows.push(scored)
    // Keep only 24 sample profiles for display, not the entire feature matrix.
    // Exact values for all scored features remain in the full score table.
    const strength=Math.max(Math.abs(rho1??0),Math.abs(rho2??0))
    const worst=profiles.at(-1)
    if(profiles.length<24 || strength>worst.strength ||
       (strength===worst.strength && scored.feature.localeCompare(worst.feature)<0)) {
      profiles.push({feature:scored.feature,secondary_id:secondary,strength,
        values:observations.map((observation,i)=>({observation,log2_tpm:values[i]}))})
      profiles.sort((a,b)=>b.strength-a.strength||a.feature.localeCompare(b.feature)||a.secondary_id.localeCompare(b.secondary_id))
      if(profiles.length>24)profiles.length=24
    }
    usable++
  }
  if(!header)throw new Error('Merged expression TSV is empty')
  rows.sort((a,b)=>Math.max(Math.abs(b.pcoa1_spearman??0),Math.abs(b.pcoa2_spearman??0))-
     Math.max(Math.abs(a.pcoa1_spearman??0),Math.abs(a.pcoa2_spearman??0)) ||a.feature.localeCompare(b.feature))
  return {schema:'4c-insight-exploratory-expression-associations-v1',modality,metric:'log2(TPM + 1)',
    method:'In-sample Spearman correlation with fixed joint PCoA coordinates; no inferential p/q values',
    samples:samples.length,features_scanned:scanned,features_ranked:usable,features_skipped:skipped,rows,profiles,
    warnings:['Post-fit feature rankings are circular when the same expression view contributed to clustering. Do not claim statistical significance or independent validation.',
      'Samples with missing or invalid TPM values are omitted per feature; no missing feature measurements are imputed.',
      'This output is descriptive. It is not differential expression, pathway enrichment or a test of disease association.']}
}
export function expressionAssociationsTsv(result){
  const safe=v=>{const s=String(v??'');const x=/^\s*[=+@]/.test(s)||/^\s*-(?!\d)/.test(s)?"'"+s:s;return /[\t\r\n"]/.test(x)?'"'+x.replaceAll('"','""')+'"':x}
  return [['feature','secondary_id','samples_used','pcoa1_spearman','pcoa2_spearman','cluster1_mean_log2_tpm_difference'],
    ...result.rows.map(r=>[r.feature,r.secondary_id,r.n_used,r.pcoa1_spearman,r.pcoa2_spearman,r.cluster1_log2_tpm_difference])]
    .map(row=>row.map(safe).join('\t')).join('\n')+'\n'
}
