import test from 'node:test'
import assert from 'node:assert/strict'
import { hierarchicalAverageLinkage } from '../joint-structure.js'
import { cutHierarchicalTree, adjustedRand, checkPatientStabilityInput, runPatientStability, runPatientStabilityAsync } from '../joint-stability.js'

function fixture(n=14, visits=1){
 const points=Array.from({length:n},(_,i)=>[Math.floor(i/(n/2))*5 + (i%5)*.23,Math.cos(i*1.23)])
 const matrix=new Float64Array(n*n)
 for(let i=0;i<n;i++)for(let j=0;j<n;j++)matrix[i*n+j]=Math.hypot(points[i][0]-points[j][0],points[i][1]-points[j][1])
 const linkage=hierarchicalAverageLinkage(matrix,n)
 const obs=Array.from({length:n},(_,i)=>`S${i}`)
 const patients=Object.fromEntries(obs.map((x,i)=>[x,`P${Math.floor(i/visits)}`]))
 return {structure:{schema:'4c-insight-joint-structure-v1',observations:obs,distances:Array.from(matrix),clustering:linkage},patients}
}

test('tree cut respects merge order and forms exact k groups',()=>{
 const {structure}=fixture()
 const labels=cutHierarchicalTree(structure.clustering.merges,14,2)
 assert.equal(new Set(labels).size,2)
 assert.deepEqual(labels.slice(0,7),Array(7).fill(0))
 assert.deepEqual(labels.slice(7),Array(7).fill(1))
 assert.throws(()=>cutHierarchicalTree(structure.clustering.merges,14,15),/Invalid/)
})

test('ARI equals 1 under cluster label permutation and near 0 for changed partitions',()=>{
 assert.equal(adjustedRand([0,0,1,1,2,2],[9,9,7,7,4,4]),1)
 assert.ok(adjustedRand([0,0,1,1,2,2],[0,1,0,1,0,1])<0.1)
})

test('patient-block resampling is deterministic and does not resample identical patients twice',()=>{
 const {structure,patients}=fixture(18)
 const options={clusters:2,repeats:20,fraction:.75,seed:43}
 const a=runPatientStability(structure,patients,options)
 const b=runPatientStability(structure,patients,options)
 assert.deepEqual(a,b)
 assert.equal(a.patients,18)
 assert.equal(a.sampled_patients_per_repeat,14)
 assert.equal(a.adjusted_rand.per_repeat.length,20)
 assert.ok(a.adjusted_rand.mean>0.5)
 assert.equal(a.assignments.length,18)
 assert.equal(a.clusters_summary.reduce((a,r)=>a+r.observations,0),18)
})

test('the resampling unit is the patient even when several visits exist',()=>{
 const {structure,patients}=fixture(18,2)
 const r=runPatientStability(structure,patients,{clusters:2,repeats:15,fraction:.7,seed:99})
 assert.equal(r.patients,9)
 assert.equal(r.patients_with_repeated_visits,9)
 assert.equal(r.sampled_patients_per_repeat,7)
 assert.equal(r.assignments.length,18)
 assert.equal(r.pair_support.informative_patient_pairs,15*7*6/2)
})

test('reports patients whose longitudinal visits fall in different original clusters',()=>{
 const {structure,patients}=fixture(14)
 // Pair one observation in the first cluster with one in the second cluster.
 patients.S13=patients.S0
 const result=runPatientStability(structure,patients,{clusters:2,repeats:15,seed:31})
 assert.ok(result.patients_crossing_original_clusters>=1)
 assert.ok(result.warnings.some(message=>message.includes('visits in different baseline clusters')))
})

test('rejects missing patient IDs rather than treating repeated visits as independent',()=>{
 const {structure,patients}=fixture()
 delete patients.S3
 assert.throws(()=>runPatientStability(structure,patients),/Missing or ambiguous patient ID/)
})

test('group/phenotype and adjusted tests are not used for stability fit',()=>{
 const {structure,patients}=fixture()
 const a=runPatientStability(structure,patients,{clusters:2,repeats:10,seed:1})
 const modified={...structure,caseValues:['Case'],diagnosis:['Affected'],q_value:0.001}
 const b=runPatientStability(modified,patients,{clusters:2,repeats:10,seed:1})
 assert.deepEqual(a,b)
})

test('bad matrices and invalid cluster parameters are rejected',()=>{
 const {structure,patients}=fixture()
 assert.throws(()=>runPatientStability(structure,patients,{clusters:1}),/Cluster count/)
 assert.throws(()=>runPatientStability(structure,patients,{repeats:2}),/10–500/)
 assert.throws(()=>runPatientStability(structure,patients,{fraction:1}),/fraction/)
 assert.throws(()=>runPatientStability(structure,patients,{seed:-1}),/Seed/)
 assert.throws(()=>runPatientStability({...structure,distances:structure.distances.slice(2)},patients),/complete-distance/)
})

test('worker-compatible asynchronous entry reports start and finish',async()=>{
 const {structure,patients}=fixture()
 const calls=[]
 const r=await runPatientStabilityAsync(structure,patients,{repeats:10,clusters:2},(...args)=>calls.push(args))
 assert.ok(calls.length>=2)
 assert.equal(r.adjusted_rand.per_repeat.length,10)
})

test('217 observations and no source labels can complete 10 resampling repeats',()=>{
 const {structure,patients}=fixture(217)
 const t=Date.now()
 const r=runPatientStability(structure,patients,{repeats:10,clusters:3,seed:333})
 assert.equal(r.observations,217)
 assert.equal(r.clusters_summary.length,3)
 assert.ok(Number.isFinite(r.adjusted_rand.mean))
 console.log('PERF_217_MS='+String(Date.now()-t))
})
