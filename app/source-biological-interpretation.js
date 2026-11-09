/** Post-fit, on-device feature interpretation from the SAME native workbook parser
 * used for direct source distances. No phenotype, adjusted-hit, or group label
 * enters the fitted distance model. This module reports descriptive associations.
 */
import { spearman } from './biological-findings.js?v=0.22.7'

export const DIRECT_FEATURE_MODALITIES = Object.freeze(['bcr','tcrAlpha','tcrBeta','isotype','immune'])
const finite = x => typeof x === 'number' && Number.isFinite(x)
const key = (a,b) => JSON.stringify(a < b ? [a,b] : [b,a])
const median = values => { const a=[...values].sort((x,y)=>x-y);return a.length ? a.length%2?a[(a.length-1)/2]:(a[a.length/2-1]+a[a.length/2])/2 : null }
const mean = values => values.reduce((s,x)=>s+x,0)/values.length

/** Map original source IDs only through exact, unambiguous, eligible aliases. */
function sourceOwners(alignment, modality) {
  if (alignment?.schema !== '4c-insight-observation-alignment-v1' || !Array.isArray(alignment.rows)) throw new Error('Check exact sample alignment first')
  const owners=new Map()
  for(const row of alignment.rows){
    const item=row.modalities?.[modality]
    if(row.excluded || item?.status!=='present')continue
    const id=String(item.alias||'').trim()
    if(!id)throw new Error(`Empty ${modality} source alias`)
    if(owners.has(id))throw new Error(`Ambiguous ${modality} source alias: ${id}`)
    owners.set(id,row.observation)
  }
  return owners
}

/** Score measured non-RNA signals against fixed PCoA, not against clinical labels.
 * A family declared complete has genuine zeros when a name is absent. Immune
 * measurements have missing values when absent; they are NEVER filled with 0.
 */
export function rankDirectSourceFeatures(structure, alignment, sourceViews, clusterByObservation={}) {
  if(structure?.schema!=='4c-insight-joint-structure-v1' || !Array.isArray(structure.pcoa?.points))throw new Error('Run joint ordination before feature interpretation')
  const points=new Map(structure.pcoa.points.map(p=>[p.observation,p]))
  const usedViews=new Set(structure.fixed_modalities||[])
  const byModality={};const warnings=[]
  for(const modality of DIRECT_FEATURE_MODALITIES){
    if(!usedViews.has(modality))continue
    const view=(sourceViews||[]).find(v=>v.modality===modality)
    if(!Array.isArray(view?.feature_samples)){warnings.push(`${modality}: rerun local source distances to extract native per-sample features`);continue}
    const owners=sourceOwners(alignment,modality)
    const mapped=new Map()
    for(const entry of view.feature_samples){
      const observation=owners.get(entry?.sample_id)
      if(!observation || !points.has(observation))continue
      if(mapped.has(observation))throw new Error(`Duplicate ${modality} feature sample for ${observation}`)
      const features=new Map()
      for(const f of entry.features||[]){
        if(typeof f?.family!=='string'||typeof f?.name!=='string'||!finite(f.value))continue
        const name=JSON.stringify([f.family,f.name])
        if(features.has(name))throw new Error(`Duplicate ${modality} feature ${name}`)
        features.set(name,{...f})
      }
      mapped.set(observation,{features,complete:new Set(entry.complete_families||[])})
    }
    const all=new Set()
    for(const entry of mapped.values())for(const k of entry.features.keys())all.add(k)
    const rows=[];const profiles=[]
    for(const name of all){
      const [family,feature]=JSON.parse(name)
      const values=[],xs=[],ys=[],cluster1=[],other=[],sampleValues=[]
      let unit=''
      for(const [observation,entry] of mapped){
        const f=entry.features.get(name)
        const value=f?.value ?? (entry.complete.has(family)?0:null)
        if(!finite(value))continue
        unit=f?.unit || unit || (entry.complete.has(family)?'fraction':'score')
        const p=points.get(observation)
        if(!finite(p.x)||!finite(p.y))continue
        values.push(value);xs.push(p.x);ys.push(p.y)
        sampleValues.push({observation,value})
        const c=clusterByObservation[observation]
        if(c===1)cluster1.push(value)
        else if(Number.isInteger(c)&&c>1)other.push(value)
      }
      const required=Math.max(8,Math.ceil(points.size*0.7))
      if(values.length<required)continue
      const x=spearman(values,xs),y=spearman(values,ys)
      if(x===null&&y===null)continue
      const delta=cluster1.length>=3&&other.length>=3?mean(cluster1)-mean(other):null
      const row={modality,family,feature,unit,n_used:values.length,pcoa1_spearman:x,pcoa2_spearman:y,cluster1_mean_difference:delta}
      rows.push(row)
      const strength=Math.max(Math.abs(x||0),Math.abs(y||0))
      if(profiles.length<16 || strength>profiles.at(-1).strength ||
         (strength===profiles.at(-1).strength && feature.localeCompare(profiles.at(-1).feature)<0)){
        profiles.push({...row,strength,values:sampleValues})
        profiles.sort((a,b)=>b.strength-a.strength||a.feature.localeCompare(b.feature))
        if(profiles.length>16)profiles.length=16
      }
    }
    const score=r=>Math.max(Math.abs(r.pcoa1_spearman||0),Math.abs(r.pcoa2_spearman||0))
    rows.sort((a,b)=>score(b)-score(a)||a.family.localeCompare(b.family)||a.feature.localeCompare(b.feature))
    byModality[modality]={modality,method:'Measured, post-fit Spearman association with fixed joint coordinates',sample_count:mapped.size,features_ranked:rows.length,rows,profiles:profiles.slice(0,16)}
    if(mapped.size<points.size)warnings.push(`${modality}: ${points.size-mapped.size} fitted observations lack an exact native feature summary`)
  }
  return {schema:'4c-insight-exploratory-source-features-v1',byModality,warnings,
    scientific_limit:'Post-fit, in-sample exploratory associations; no p-values, FDR, inferred biomarkers or external validation'}
}

/** Audit one fitted observation against the EXACT cohort, per modality.
 * Medians and neighbors use observed source distances only; no imputation.
 */
export function inspectJointObservation(structure, alignment, views, observation) {
  if(structure?.schema!=='4c-insight-joint-structure-v1')throw new Error('Run joint structure first')
  if(!structure.observations?.includes(observation))throw new Error('Choose a retained fitted observation')
  const ledger=alignment?.rows?.find(r=>r.observation===observation && !r.excluded)
  if(!ledger)throw new Error('Fitted observation has no eligible exact alignment')
  const n=structure.observations.length,i=structure.observations.indexOf(observation)
  const distances=structure.distances
  if(distances?.length!==n*n)throw new Error('Expected complete fixed-cohort distances')
  const joint=structure.observations.map((id,j)=>({observation:id,distance:distances[i*n+j]})).filter(x=>x.observation!==observation&&finite(x.distance))
  joint.sort((a,b)=>a.distance-b.distance||a.observation.localeCompare(b.observation))
  const modal=[]
  for(const modality of structure.fixed_modalities){
    const view=(views||[]).find(v=>v.modality===modality)
    const owners=sourceOwners(alignment,modality)
    const aliases=new Map([...owners].map(([src,id])=>[id,src]))
    const selectedAlias=aliases.get(observation)
    const peers=[];const seen=new Set()
    for(const pair of view?.pairs||[]){
      if(pair.a!==selectedAlias&&pair.b!==selectedAlias)continue
      const peerAlias=pair.a===selectedAlias?pair.b:pair.a
      const other=owners.get(peerAlias)
      if(!other||other===observation||!structure.observations.includes(other))continue
      if(!finite(pair.distance)||pair.distance<0)continue
      if(seen.has(other))throw new Error(`Duplicate ${modality} source pair`)
      seen.add(other);peers.push({observation:other,distance:pair.distance})
    }
    peers.sort((a,b)=>a.distance-b.distance||a.observation.localeCompare(b.observation))
    const selectedSource=(view?.feature_samples||[]).find(x=>x.sample_id===selectedAlias)
    const featureSummary=(selectedSource?.features||[]).filter(x=>finite(x.value)).slice(0,80)
    modal.push({modality,status:ledger.modalities?.[modality]?.status||'missing',matched_pairs:peers.length,expected_pairs:n-1,
      median_distance:median(peers.map(p=>p.distance)),nearest_neighbor:peers[0]?.observation||null,
      nearest_distance:peers[0]?.distance??null,features:featureSummary})
  }
  return {schema:'4c-insight-joint-sample-inspection-v1',observation,patient:ledger.subject||'',timepoint:ledger.timepoint||'',
    metadata_row:ledger.rowNumber,
    joint_median_distance:median(joint.map(x=>x.distance)),joint_nearest:joint.slice(0,5),modalities:modal,
    warnings:['Large distances can reflect QC, missing modalities, or biology. This view does not classify an outlier.','Only exact aligned sources from the fitted cohort are used; no source pair is imputed.']}
}

export function directFeatureTsv(result) {
  const safe=v=>{const s=String(v??'');const q=/^\s*[=+@]/.test(s)||/^\s*-(?!\d)/.test(s)?"'"+s:s;return /[\t\r\n"]/.test(q)?`"${q.replaceAll('"','""')}"`:q}
  const header=['data_type','family','feature','measurement_unit','samples_used','spearman_pcoa_1','spearman_pcoa_2','cluster_1_mean_difference']
  const rows=Object.values(result?.byModality||{}).flatMap(v=>v.rows||[]).map(r=>[r.modality,r.family,r.feature,r.unit,r.n_used,r.pcoa1_spearman,r.pcoa2_spearman,r.cluster1_mean_difference])
  return [header,...rows].map(row=>row.map(safe).join('\t')).join('\n')+'\n'
}
