import test from 'node:test'
import assert from 'node:assert/strict'
import { analyzeJointStructure, selectCompleteDistanceCohort, principalCoordinates, hierarchicalAverageLinkage, compareFixedCohortAblation } from '../joint-structure.js'

const ids = ['A','B','C','D']
const xy = [[0,0],[1,0],[0,1],[1,1]]
const views = [{modality:'gene',distance_label:'Euclidean'},{modality:'bcr',distance_label:'Morisita'}]
const model=(idsArg=ids, skip=[])=>({
  model:'unsupervised-median-scaled-mean-distance',observations:idsArg,views,min_views:2,
  pairs:idsArg.flatMap((a,i)=>idsArg.slice(i+1).filter(b=>!skip.includes([a,b].sort().join(':'))).map(b=>({
    a,b,modalities:['gene','bcr'],contributing_views:2,
    distance:Math.hypot(...xy[ids.indexOf(a)].map((v,k)=>v-xy[ids.indexOf(b)][k])),
  })))
})

test('fixed panel square PCoA reproduces Euclidean geometry and complete UPGMA tree',()=>{
  const output=analyzeJointStructure(model())
  assert.equal(output.schema,'4c-insight-joint-structure-v1')
  assert.equal(output.observations.length,4)
  assert.equal(output.complete_pairs,6)
  assert.equal(output.coverage.pair_coverage,1)
  assert.equal(output.pcoa.negative_axes,0)
  assert.ok(Math.abs(output.pcoa.axis_variance_explained[0]-0.5)<1e-7)
  assert.ok(Math.abs(output.pcoa.axis_variance_explained[1]-0.5)<1e-7)
  for(let i=0;i<4;i++)for(let j=i+1;j<4;j++){
    const a=output.pcoa.points[i],b=output.pcoa.points[j]
    assert.ok(Math.abs(Math.hypot(a.x-b.x,a.y-b.y)-Math.hypot(...xy[i].map((v,k)=>v-xy[j][k])))<1e-7)
  }
  assert.equal(output.clustering.merges.length,3)
  assert.deepEqual([...output.clustering.leaf_order].sort((a,b)=>a-b),[0,1,2,3])
  assert.deepEqual(output.fixed_modalities,['gene','bcr'])
})

test('an incomplete pair is never set to zero or silently filled',()=>{
  const output=analyzeJointStructure(model(ids,['C:D']))
  assert.equal(output.coverage.available_fixed_view_pairs,5)
  assert.equal(output.coverage.removed_for_missing_pairs,1)
  assert.equal(output.observations.length,3)
  assert.equal(output.distances.length,9)
  assert.equal(output.complete_pairs,3)
})

test('no complete triangle fails instead of inventing distances',()=>{
  assert.throws(()=>analyzeJointStructure(model(ids,['A:B','A:C','A:D','B:C','B:D'])),/Only .* complete fixed-view/)
})

test('rejects partial-view coverage even if source result contains distance',()=>{
  const bad=model()
  bad.pairs[0].modalities=['gene'];bad.pairs[0].contributing_views=1
  const output=analyzeJointStructure(bad)
  assert.equal(output.coverage.non_fixed_pairs,1)
  assert.equal(output.coverage.removed_for_missing_pairs,1)
})

test('refuses incorrect minimum coverage and cannot reuse a mixed-panel fusion',()=>{
  const bad=model();bad.min_views=1
  assert.throws(()=>analyzeJointStructure(bad),/Recalculate joint distances/)
})

test('patient option keeps only first observation and requires a patient ID',()=>{
  const bad=model()
  assert.throws(()=>analyzeJointStructure(bad,{onePerPatient:true}),/No patient ID/)
  const full=analyzeJointStructure(bad,{onePerPatient:true,patientByObservation:{A:'p1',B:'p1',C:'p2',D:'p3'}})
  assert.equal(full.coverage.patient_deduplicated,1)
  assert.deepEqual(full.observations,['A','C','D'])
})

test('zero and degenerate distances have clear PCoA failure, not numeric NaN',()=>{
  const zero=new Float64Array(9)
  assert.throws(()=>principalCoordinates(zero,3),/fewer than two positive PCoA axes/)
})

test('native shape validators reject invalid, negative, asymmetric or duplicate pairs',()=>{
  const bad=model();bad.pairs[0].distance=-1
  assert.throws(()=>analyzeJointStructure(bad),/Invalid numerical distance/)
  const duplicate=model();duplicate.pairs.push({...duplicate.pairs[0]})
  assert.throws(()=>analyzeJointStructure(duplicate),/Duplicate unordered/)
  assert.throws(()=>hierarchicalAverageLinkage(new Float64Array(3),3),/complete distance matrix/)
})

test('highly non-euclidean triangle records negative eigen inertia',()=>{
  const d=new Float64Array([0,1,1,1, 1,0,1,1, 1,1,0,3, 1,1,3,0])
  const x=principalCoordinates(d,4)
  assert.ok(x.negative_axes>0)
  assert.ok(x.negative_inertia_ratio>0)
})

test('medium cohort computes stable unlabelled axes and complete clustering',()=>{
  const n=48
  const p=Array.from({length:n},(_,i)=>[Math.sin(i*.27),Math.cos(i*.13),i*.012])
  const mat=new Float64Array(n*n)
  for(let i=0;i<n;i++)for(let j=0;j<n;j++)mat[i*n+j]=Math.hypot(...p[i].map((x,k)=>x-p[j][k]))
  const coords=principalCoordinates(mat,n)
  assert.ok(coords.negative_inertia_ratio<1e-7)
  assert.ok(coords.axis_variance_explained[0]>0)
  assert.equal(hierarchicalAverageLinkage(mat,n).leaf_order.length,n)
})


test('leave-one-modality-out uses the identical cohort, not reselected observations',()=>{
  const full=analyzeJointStructure(model())
  const remaining={views:[{modality:'gene'}],pairs:model().pairs}
  assert.equal(compareFixedCohortAblation(full,remaining,'bcr').available,false)
  const three={...model(),views:[...model().views,{modality:'immune'}],min_views:3,
    pairs:model().pairs.map(p=>({...p,modalities:['gene','bcr','immune'],contributing_views:3}))}
  const actual=analyzeJointStructure(three)
  const drop={...model(),views:[{modality:'gene'},{modality:'bcr'}]}
  const test=compareFixedCohortAblation(actual,drop,'immune')
  assert.equal(test.available,true)
  assert.equal(test.fixed_observations,4)
  assert.equal(test.overlapping_pairs,6)
  assert.ok(Math.abs(test.distance_correlation-1)<1e-12)
  assert.equal(test.nearest_neighbor_retention,1)
  const missing={...drop,pairs:drop.pairs.slice(1)}
  assert.equal(compareFixedCohortAblation(actual,missing,'immune').available,false)
})


test('group and outcome annotations do not change unsupervised ordination or clustering',()=>{
  const input=model()
  const baseline=analyzeJointStructure(input)
  const annotated={...model(),caseValues:['Case'],controlValues:['Control'],
    phenotype:{A:'case',B:'control',C:'case',D:'control'},
    pairs:input.pairs.map((pair,i)=>({...pair,diagnosis:i%2?'Control':'Case',q_value:0.001,beta_group:2}))}
  const output=analyzeJointStructure(annotated,{phenotype:{A:'control',B:'case',C:'control',D:'case'}})
  assert.deepEqual(output.distances,baseline.distances)
  assert.deepEqual(output.pcoa.points,baseline.pcoa.points)
  assert.deepEqual(output.clustering,baseline.clustering)
})
