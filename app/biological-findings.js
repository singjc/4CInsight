/** Descriptive, post-fit multi-omics interpretation.
 * No clinical or molecular feature enters distance fusion, PCoA or clustering.
 * Do not attach inferential p/q values to clusters chosen from these features.
 */
import {cutHierarchicalTree} from './joint-stability.js?v=0.22.7'

const finite = v => typeof v === 'number' && Number.isFinite(v)
const exact = v => String(v ?? '').trim()
function median(a) {if(!a.length)return null;const b=[...a].sort((x,y)=>x-y),m=b.length>>1;return b.length%2?b[m]:(b[m-1]+b[m])/2}
export function rankValues(values){
  const order=values.map((v,i)=>({v,i})).sort((a,b)=>a.v-b.v || a.i-b.i)
  const ranks=Array(values.length)
  for(let i=0;i<order.length;){let j=i+1;while(j<order.length&&order[j].v===order[i].v)j++; const rank=(i+j+1)/2;for(let k=i;k<j;k++)ranks[order[k].i]=rank;i=j}
  return ranks
}
export function pearson(x,y){
  if(x.length!==y.length||x.length<3)return null
  const mean=a=>a.reduce((s,v)=>s+v,0)/a.length,mx=mean(x),my=mean(y)
  let xy=0,xx=0,yy=0
  for(let i=0;i<x.length;i++){const a=x[i]-mx,b=y[i]-my;xy+=a*b;xx+=a*a;yy+=b*b}
  return xx>0&&yy>0?Math.max(-1,Math.min(1,xy/Math.sqrt(xx*yy))):null
}
export function spearman(x,y){return pearson(rankValues(x),rankValues(y))}
function key(a,b){return a<b?JSON.stringify([a,b]):JSON.stringify([b,a])}
function tableAnnotations(structure, alignment, metadata, column){
  const obs=structure.observations, obsCol=alignment.roles.observationColumn
  if(!metadata?.columns?.includes(obsCol)||!Array.isArray(metadata.rows))throw new Error('Recheck sample alignment before interpreting clinical metadata')
  if(column&&!metadata.columns.includes(column))throw new Error('Selected metadata column is missing')
  const obsIdx=metadata.columns.indexOf(obsCol),idx=column?metadata.columns.indexOf(column):-1
  const byObs=new Map()
  for(const row of metadata.rows){const id=exact(row[obsIdx]);if(byObs.has(id))throw new Error(`Duplicate metadata observation '${id}'`);byObs.set(id,idx<0?'':exact(row[idx]))}
  return obs.map(id=>byObs.get(id)??'')
}
function metadataSummary(values,labels,points,column){
  const missing=values.filter(v=>!v).length, non=values.filter(Boolean)
  const parsed=non.map(v=>Number(v)),numeric=non.length>=8&&parsed.every(Number.isFinite)&&new Set(non).size>=4
  const distinct=new Set(non)
  if(numeric){
    const kept=values.map((v,i)=>({v,point:points[i]})).filter(x=>x.v!==''&&finite(Number(x.v)))
    return {column,kind:'numeric',missing,observed:kept.length,unique:distinct.size,
      pcoa_1_rho:spearman(kept.map(x=>Number(x.v)),kept.map(x=>x.point.x)),
      pcoa_2_rho:spearman(kept.map(x=>Number(x.v)),kept.map(x=>x.point.y)),
      range:parsed.reduce((r,v)=>[Math.min(r[0],v),Math.max(r[1],v)],[Infinity,-Infinity])}
  }
  const categories=[...distinct].sort((a,b)=>a.localeCompare(b))
  const counts=categories.map(value=>({value,n:values.filter(v=>v===value).length,
    clusters:[...new Set(labels)].sort((a,b)=>a-b).map(cluster=>({cluster,count:values.filter((v,i)=>v===value&&labels[i]===cluster).length}))}))
  return {column,kind:'categorical',missing,observed:non.length,unique:categories.length,
    categories:counts,too_many_categories:categories.length>14}
}
function modalitySummary(structure,alignment,views,clusters){
  const obs=structure.observations,n=obs.length,allowed=new Set(structure.fixed_modalities),outputs=[]
  const rows=new Map(alignment.rows.filter(r=>!r.excluded).map(r=>[r.observation,r]))
  for(const view of views){
    if(!allowed.has(view.modality))continue
    const owners=new Map()
    for(const id of obs){const status=rows.get(id)?.modalities?.[view.modality];if(status?.status!=='present')continue
      const alias=exact(status.alias);if(owners.has(alias))throw new Error(`Ambiguous ${view.modality} source alias`);owners.set(alias,id)}
    const sourcePairs=new Map(),positive=[]
    for(const p of view.pairs||[]){const a=owners.get(p.a),b=owners.get(p.b);if(!a||!b||a===b)continue
      if(!finite(p.distance)||p.distance<0)throw new Error(`Invalid ${view.modality} distance`)
      const k=key(a,b);if(sourcePairs.has(k))throw new Error(`Duplicate ${view.modality} distance`)
      sourcePairs.set(k,p.distance);if(p.distance>0)positive.push(p.distance)}
    const scale=median(positive)
    const within=[],between=[],all=[],joint=[]
    for(let i=0;i<n;i++)for(let j=i+1;j<n;j++){
      const d=sourcePairs.get(key(obs[i],obs[j]));if(d===undefined)continue
      const norm=scale>0?d/scale:d
      if(clusters[i]===clusters[j])within.push(norm);else between.push(norm)
      all.push(norm);joint.push(structure.distances[i*n+j])
    }
    const mw=within.length?within.reduce((a,b)=>a+b,0)/within.length:null
    const mb=between.length?between.reduce((a,b)=>a+b,0)/between.length:null
    outputs.push({modality:view.modality,distance_label:view.distance_label,
      pairs:all.length,expected_pairs:n*(n-1)/2,scale,
      within_mean:mw,between_mean:mb,between_within_ratio:mw>0?mb/mw:null,
      joint_distance_correlation:pearson(all,joint),method:'In-sample, post-fit descriptive separation; not independent feature importance'})
  }
  return outputs
}
export function buildBiologicalFindings(structure,alignment,metadata,views,{column='',clusters=3}={}){
  if(structure?.schema!=='4c-insight-joint-structure-v1'||!Array.isArray(structure.observations)||!Array.isArray(structure.distances))throw new Error('Run joint ordination and clustering first')
  const n=structure.observations.length
  if(n<5||structure.distances.length!==n*n)throw new Error('Invalid complete joint distance matrix')
  if(!Number.isInteger(clusters)||clusters<2||clusters>=n)throw new Error('Cluster count must be between 2 and N−1')
  if(alignment?.schema!=='4c-insight-observation-alignment-v1')throw new Error('Recheck source alignment')
  const labels=cutHierarchicalTree(structure.clustering.merges,n,clusters).map(x=>x+1)
  const points=structure.pcoa.points
  if(points.length!==n||points.some((p,i)=>p.observation!==structure.observations[i]))throw new Error('PCoA observations are not aligned')
  const values=tableAnnotations(structure,alignment,metadata,column)
  const metadataReport=column?metadataSummary(values,labels,points,column):null
  const clusterCounts=Array.from({length:clusters},(_,i)=>({cluster:i+1,observations:labels.filter(x=>x===i+1).length}))
  const sourceRows=new Map(alignment.rows.map(r=>[r.observation,r]))
  const coverage=structure.observations.map((observation,i)=>({observation,cluster:labels[i],annotation:values[i],
    patient:sourceRows.get(observation)?.subject||'',timepoint:sourceRows.get(observation)?.timepoint||'',
    measured:structure.fixed_modalities.filter(k=>sourceRows.get(observation)?.modalities?.[k]?.status==='present').length}))
  const modality=modalitySummary(structure,alignment,views,labels)
  return {schema:'4c-insight-biological-findings-v1',method:'Post-fit descriptive metadata and modality associations; no inferential tests',
    column,clusters,observations:n,fixed_modalities:[...structure.fixed_modalities],
    membership:clusterCounts,points:points.map((p,i)=>({observation:p.observation,x:p.x,y:p.y,cluster:labels[i],annotation:values[i]})),
    metadata:metadataReport,modality,coverage,
    warnings:['Clusters and axes were fitted using these same modalities. Associations are exploratory and cannot validate those clusters.',
      'Distance-to-cluster separation can be circular. Review fixed-cohort leave-one-modality-out results and source QC.',
      'PCoA axis signs can reverse between analyses. Compare the same fitted cohort and orient axes consistently before interpreting directions.',
      ...(new Set(coverage.map(r=>r.patient).filter(Boolean)).size<coverage.length?['Some patients have repeated observations; cluster summaries count observations, not independent patients.']:[]),
      ...(metadataReport?.too_many_categories?['Selected metadata has many categories. Review whether this is an identifier instead of a clinical grouping.']:[])]}
}
export function findingsTsv(report){
  const escape=v=>{const s=String(v??'');const guarded=/^\s*[=+@]/.test(s)||/^\s*-(?!\d)/.test(s)?"'"+s:s;return /[\t\n\r"]/.test(guarded)?'"'+guarded.replaceAll('"','""')+'"':guarded}
  const rows=[['observation','joint_cluster','pc1','pc2','selected_metadata','patient','timepoint','measured_modalities']]
  for(let i=0;i<report.points.length;i++){const p=report.points[i],v=report.coverage[i];rows.push([p.observation,p.cluster,p.x,p.y,p.annotation,v.patient,v.timepoint,v.measured])}
  return rows.map(r=>r.map(escape).join('\t')).join('\n')+'\n'
}
