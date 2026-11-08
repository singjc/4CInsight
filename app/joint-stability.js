/**
 * Patient-block, outcome-blind cluster sensitivity analysis.
 * Operates on the existing full, fixed-modality, complete-distance cohort.
 * Each repeat samples distinct patients WITHOUT replacement and exactly one
 * observation per chosen patient. No distances are re-estimated or imputed.
 * This is SUBSAMPLING, not a bootstrap and not predictive validation.
 */
import { hierarchicalAverageLinkage } from './joint-structure.js?v=0.22.4'

const choose2 = n => n < 2 ? 0 : n * (n - 1) / 2
const finite = n => typeof n === 'number' && Number.isFinite(n)
function rng32(seed) {
  let state = seed >>> 0
  if (!state) state = 0x6d2b79f5
  return () => {
    state ^= state << 13; state ^= state >>> 17; state ^= state << 5
    return (state >>> 0) / 4294967296
  }
}
function median(values) {
  if (!values.length) return null
  const ordered = values.slice().sort((a,b) => a-b)
  const mid = Math.floor(ordered.length / 2)
  return ordered.length % 2 ? ordered[mid] : (ordered[mid-1] + ordered[mid]) / 2
}

/** Cut a deterministic average-linkage tree into k groups. Labels are ordered
 * by the smallest observation index in each group, not by merge ID. */
export function cutHierarchicalTree(merges,n,k) {
  if (!Number.isInteger(n) || n<3 || !Array.isArray(merges) || merges.length!==n-1 ||
      !Number.isInteger(k) || k<2 || k>=n) throw new Error('Invalid cluster cut or hierarchy')
  const parent=Array.from({length:n},(_,i)=>i)
  const roots=new Map(Array.from({length:n},(_,i)=>[i,i]))
  const find=x=>{while(parent[x]!==x){parent[x]=parent[parent[x]];x=parent[x]}return x}
  for (let step=0;step<n-k;step++) {
    const m=merges[step]
    if(m?.id!==n+step || !roots.has(m.left) || !roots.has(m.right) || m.left===m.right) {
      throw new Error('Invalid ordered average-linkage merge')
    }
    const a=find(roots.get(m.left)), b=find(roots.get(m.right))
    if(a===b)throw new Error('Repeated merge of the same cluster')
    parent[b]=a
    roots.set(m.id,a)
  }
  const first=new Map()
  for(let i=0;i<n;i++){
    const id=find(i)
    if(!first.has(id))first.set(id,i)
  }
  const labels=new Map([...first.values()].sort((a,b)=>a-b).map((id,index)=>[find(id),index]))
  return Array.from({length:n},(_,i)=>labels.get(find(i)))
}

/** Adjusted Rand index. This uses one observation per patient so repeated
 * clinical visits cannot increase a patient's weight in the score. */
export function adjustedRand(a,b) {
  if(!Array.isArray(a)||!Array.isArray(b)||a.length!==b.length||a.length<3)throw new Error('ARI needs three or more paired assignments')
  const contingency=new Map(),left=new Map(),right=new Map()
  for(let i=0;i<a.length;i++){
    const x=a[i],y=b[i],key=JSON.stringify([x,y])
    contingency.set(key,(contingency.get(key)||0)+1)
    left.set(x,(left.get(x)||0)+1)
    right.set(y,(right.get(y)||0)+1)
  }
  const nPairs=choose2(a.length)
  const pairAgreement=[...contingency.values()].reduce((s,n)=>s+choose2(n),0)
  const ca=[...left.values()].reduce((s,n)=>s+choose2(n),0)
  const cb=[...right.values()].reduce((s,n)=>s+choose2(n),0)
  const expected=ca*cb/nPairs
  const max=(ca+cb)/2
  if(Math.abs(max-expected)<1e-12)return Math.abs(pairAgreement-expected)<1e-12?1:0
  return Math.max(-1,Math.min(1,(pairAgreement-expected)/(max-expected)))
}

export function checkPatientStabilityInput(structure,patientByObservation,options={}) {
  if(structure?.schema!=='4c-insight-joint-structure-v1' ||
      !Array.isArray(structure.observations) || !Array.isArray(structure.distances) ||
      !Array.isArray(structure.clustering?.merges)) throw new Error('Run joint ordination and clustering before stability analysis')
  const n=structure.observations.length
  if(n<5 || structure.distances.length!==n*n)throw new Error('Patient stability requires five or more complete-distance observations')
  if(!patientByObservation || typeof patientByObservation!=='object')throw new Error('Select a patient ID column in sample alignment')
  const patients=new Map(),seen=new Set()
  for(let i=0;i<n;i++){
    const id=structure.observations[i]
    if(typeof id!=='string' || !id.trim() || seen.has(id))throw new Error('Invalid or repeated joint observation ID')
    seen.add(id)
    const patient=patientByObservation[id]
    if(typeof patient!=='string'||!patient.trim()||patient!==patient.trim())throw new Error(`Missing or ambiguous patient ID for observation '${id}'. Select an exact patient ID column.`)
    if(!patients.has(patient))patients.set(patient,[])
    patients.get(patient).push(i)
    for(let j=0;j<n;j++){
      const d=structure.distances[i*n+j]
      if(!finite(d)||d<0||Math.abs(d-structure.distances[j*n+i])>1e-8 || (i===j&&Math.abs(d)>1e-10))throw new Error('Joint distances must be finite, symmetric and complete')
    }
  }
  const k=Number(options.clusters ?? 3), repeats=Number(options.repeats ?? 40)
  const fraction=Number(options.fraction ?? 0.8),seed=Number(options.seed ?? 20261108)
  if(!Number.isInteger(k)||k<2||k>Math.min(12,patients.size-2))throw new Error('Cluster count must be 2–12 and leave at least two other patients')
  if(!Number.isInteger(repeats)||repeats<10||repeats>500)throw new Error('Use 10–500 patient subsampling repeats')
  if(!finite(fraction)||fraction<0.6||fraction>0.95)throw new Error('Subsampling fraction must be 0.60–0.95')
  if(!Number.isInteger(seed)||seed<0||seed>4294967295)throw new Error('Seed must be an unsigned 32-bit integer')
  const count=Math.ceil(fraction*patients.size)
  if(count<=k || count>=patients.size)throw new Error('Subsampling must retain more patients than clusters and omit at least one patient')
  const baseline=cutHierarchicalTree(structure.clustering.merges,n,k)
  const visitsDisagree=[...patients.values()].filter(indices=>new Set(indices.map(i=>baseline[i])).size>1)
  return {n,patients,baseline,k,repeats,fraction,seed,count,visitsDisagree:visitsDisagree.length}
}

/** Pure repeat engine for reproducible, independent fixture testing. */
export function runPatientStability(structure,patientByObservation,options={},onIteration=null) {
  const input=checkPatientStabilityInput(structure,patientByObservation,options)
  const {n,patients,baseline,k,repeats,fraction,seed,count,visitsDisagree}=input
  const random=rng32(seed),allPatients=[...patients.keys()]
  const original=structure.distances
  const agreements=[]
  let sampledPatientPairs=0,sameBaselinePairs=0,sameBaselineReclustered=0,
      differentBaselinePairs=0,differentBaselineReclustered=0
  const clusterStats=Array.from({length:k},()=>({coobserved:0,coassigned:0}))
  const perPatientVisits=new Map([...patients].map(([id,indices])=>[id,indices.length]))
  // Running state is local to each invocation; no clinical identifiers leave the worker.
  function runOneRepeat() {
    const shuffled=allPatients.slice()
    for(let i=shuffled.length-1;i>0;i--){const j=Math.floor(random()*(i+1));[shuffled[i],shuffled[j]]=[shuffled[j],shuffled[i]]}
    const indices=shuffled.slice(0,count).map(patient=>{
      const visits=patients.get(patient)
      return visits[Math.floor(random()*visits.length)]
    }).sort((a,b)=>a-b)
    const m=indices.length, matrix=new Float64Array(m*m)
    for(let i=0;i<m;i++)for(let j=0;j<m;j++)matrix[i*m+j]=original[indices[i]*n+indices[j]]
    const fitted=hierarchicalAverageLinkage(matrix,m)
    const assignment=cutHierarchicalTree(fitted.merges,m,k)
    const old=indices.map(i=>baseline[i])
    agreements.push(adjustedRand(old,assignment))
    for(let i=0;i<m;i++)for(let j=i+1;j<m;j++){
      sampledPatientPairs++
      const stable=assignment[i]===assignment[j]
      if(old[i]===old[j]){
        sameBaselinePairs++;if(stable)sameBaselineReclustered++
        const stat=clusterStats[old[i]];stat.coobserved++;if(stable)stat.coassigned++
      }else{
        differentBaselinePairs++;if(stable)differentBaselineReclustered++
      }
    }
  }
  for(let iteration=0;iteration<repeats;iteration++){
    runOneRepeat()
    if(typeof onIteration==='function' && ((iteration+1)%5===0 || iteration+1===repeats))onIteration(iteration+1,repeats)
  }
  const membership=Array.from({length:k},(_,cluster)=>{
    const observed=baseline.reduce((s,v)=>s+(v===cluster?1:0),0)
    const patientCount=[...patients.values()].filter(visits=>visits.some(i=>baseline[i]===cluster)).length
    const stat=clusterStats[cluster]
    return {cluster:cluster+1,observations:observed,patients:patientCount,
      informative_pairs:stat.coobserved,
      within_cluster_coassignment:stat.coobserved?stat.coassigned/stat.coobserved:null}
  })
  const mean=agreements.reduce((a,b)=>a+b,0)/agreements.length
  const labelAssignments=structure.observations.map((observation,i)=>({observation,patient:patientByObservation[observation],cluster:baseline[i]+1}))
  return {
    schema:'4c-insight-patient-stability-v1',
    method:'patient-block subsampling without replacement; one randomly selected visit per patient per repeat; hierarchical average linkage of unchanged joint distances',
    seed,repeats,fraction,clusters:k,patients:patients.size,observations:n,
    sampled_patients_per_repeat:count,patients_with_repeated_visits:[...perPatientVisits.values()].filter(x=>x>1).length,
    patients_crossing_original_clusters:visitsDisagree,
    adjusted_rand:{mean,median:median(agreements),minimum:Math.min(...agreements),maximum:Math.max(...agreements),per_repeat:agreements},
    pair_support:{informative_patient_pairs:sampledPatientPairs,
      within_baseline_cluster_pairs:sameBaselinePairs,
      within_cluster_coassignment:sameBaselinePairs?sameBaselineReclustered/sameBaselinePairs:null,
      between_baseline_cluster_pairs:differentBaselinePairs,
      between_cluster_separation:differentBaselinePairs?1-differentBaselineReclustered/differentBaselinePairs:null},
    clusters_summary:membership,
    assignments:labelAssignments,
    warnings:[
      'This is a fixed-distance sensitivity analysis. It does not measure uncertainty in source features or support clinical prediction.',
      'Cluster number is user-specified. No optimal number of clusters or statistical p-value was estimated.',
      'One observation per patient per repeat prevents patients with repeated visits from receiving extra sampling weight.',
      ...(visitsDisagree ? [`${visitsDisagree} patients have visits in different baseline clusters. Review longitudinal variation before biological interpretation.`]:[]),
      ...([...perPatientVisits.values()].filter(x=>x>1).length ? ['For repeated-visit patients, the sampled observation is selected reproducibly from their eligible visits.']:[]),
    ],
  }
}

/** Async worker entry yields between repeats to keep the Stop control responsive. */
export async function runPatientStabilityAsync(structure,patientByObservation,options={},onProgress=()=>{}) {
  const input=checkPatientStabilityInput(structure,patientByObservation,options)
  // The caller runs this in a stoppable isolated browser worker. Progress
  // messages are posted during fitting; terminating the worker cancels it.
  onProgress(0,input.repeats,'Calculating patient-block cluster stability')
  return runPatientStability(structure,patientByObservation,options,(done,total)=>
    onProgress(done,total,`Patient subsets analyzed: ${done} / ${total}`))
}
