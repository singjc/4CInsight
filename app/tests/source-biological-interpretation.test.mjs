import test from 'node:test'
import assert from 'node:assert/strict'
import {rankDirectSourceFeatures,inspectJointObservation,directFeatureTsv,DIRECT_FEATURE_MODALITIES} from '../source-biological-interpretation.js'

const ids=Array.from({length:12},(_,i)=>`O${i+1}`)
const fixed=['bcr','tcrAlpha','tcrBeta','isotype','immune']
const structure={schema:'4c-insight-joint-structure-v1',observations:ids,fixed_modalities:fixed,
 pcoa:{points:ids.map((id,i)=>({observation:id,x:i,y:11-i}))},
 distances:ids.flatMap((a,i)=>ids.map((b,j)=>Math.abs(i-j))),clustering:{merges:[]}}
const alignment={schema:'4c-insight-observation-alignment-v1',rows:ids.map((id,i)=>({observation:id,subject:id,excluded:false,rowNumber:i+2,
 modalities:Object.fromEntries(fixed.map(m=>[m,{status:'present',alias:`X${i+1}`}]))}))}
const makeView=(modality,fn=(i)=>i+1)=>({modality,distance_label:'native measured distance',
 feature_samples:ids.map((id,i)=>({sample_id:`X${i+1}`,complete_families:['V gene fraction'],features:[
   {family:'Repertoire metric',name:'Shannon diversity',value:fn(i),unit:'nats'},
   {family:'V gene fraction',name:'V-A',value:i%2?0.3:0.7,unit:'fraction'}]})),
 pairs:ids.flatMap((a,i)=>ids.slice(i+1).map((b,j)=>({a:`X${i+1}`,b:`X${i+2+j}`,distance:Math.abs(j+1)})))})

test('direct feature ranks all five measured non-RNA views without fitting group labels',()=>{
 const r=rankDirectSourceFeatures(structure,alignment,fixed.map(m=>makeView(m)),Object.fromEntries(ids.map((id,i)=>[id,i<6?1:2])))
 assert.equal(Object.keys(r.byModality).length,5)
 for(const m of fixed){assert.equal(r.byModality[m].sample_count,12);assert.ok(r.byModality[m].rows.some(x=>x.feature==='Shannon diversity'));assert.equal(r.byModality[m].rows.find(x=>x.feature==='Shannon diversity').pcoa1_spearman,1)}
 assert.doesNotMatch(JSON.stringify(r),/p_value|q_value|caseValues|controlValues/)
})
test('clinical labels do not change fitted feature ranks; only descriptive contrast may change',()=>{
 const views=[makeView('bcr')]
 const one=rankDirectSourceFeatures({...structure,fixed_modalities:['bcr']},alignment,views,{})
 const two=rankDirectSourceFeatures({...structure,fixed_modalities:['bcr']},alignment,views,Object.fromEntries(ids.map((id,i)=>[id,i<6?1:2])))
 assert.equal(one.byModality.bcr.rows[0].pcoa1_spearman,two.byModality.bcr.rows[0].pcoa1_spearman)
 assert.equal(one.byModality.bcr.rows[0].cluster1_mean_difference,null)
})
test('absent feature within a verified complete composition is zero, not missing',()=>{
 const view=makeView('bcr');view.feature_samples[0].features=view.feature_samples[0].features.filter(f=>f.family!=='V gene fraction')
 const r=rankDirectSourceFeatures({...structure,fixed_modalities:['bcr']},alignment,[view])
 assert.ok(r.byModality.bcr.rows.find(x=>x.feature==='V-A').n_used===12)
})
test('missing immune signature stays missing, not zero',()=>{
 const view=makeView('immune');view.feature_samples[0].features=view.feature_samples[0].features.filter(f=>f.family!=='Repertoire metric')
 const r=rankDirectSourceFeatures({...structure,fixed_modalities:['immune']},alignment,[view])
 assert.equal(r.byModality.immune.rows.find(x=>x.feature==='Shannon diversity').n_used,11)
})
test('missing source features do not fabricate ranks',()=>{
 const r=rankDirectSourceFeatures(structure,alignment,[makeView('bcr')]);assert.deepEqual(Object.keys(r.byModality),['bcr']);assert.equal(r.warnings.length,4)
})
test('ambiguous exact aliases are rejected',()=>{
 const wrong=structuredClone(alignment);wrong.rows[1].modalities.bcr.alias='X1'
 assert.throws(()=>rankDirectSourceFeatures({...structure,fixed_modalities:['bcr']},wrong,[makeView('bcr')]),/Ambiguous/)
})
test('sample inspection uses original distances, sample IDs, and no guessed results',()=>{
 const r=inspectJointObservation(structure,alignment,[makeView('bcr')],ids[0]);
 assert.equal(r.observation,'O1');assert.equal(r.patient,'O1');assert.equal(r.joint_median_distance,6)
 assert.equal(r.modalities[0].matched_pairs,11);assert.equal(r.modalities[0].nearest_neighbor,'O2')
 assert.equal(r.modalities[1].matched_pairs,0)
 assert.equal(r.modalities[1].median_distance,null)
})
test('unknown observation is rejected',()=>assert.throws(()=>inspectJointObservation(structure,alignment,[],'unseen'),/retained/))
test('TSV has every feature and formula-injection guard',()=>{
 const view=makeView('bcr');view.feature_samples.forEach(s=>s.features[0].name='=2+2')
 const r=rankDirectSourceFeatures({...structure,fixed_modalities:['bcr']},alignment,[view]);const t=directFeatureTsv(r)
 assert.match(t,/'=2\+2/);assert.match(t,/spearman_pcoa_2/)
})
test('five named modalities are stable',()=>assert.deepEqual(DIRECT_FEATURE_MODALITIES,fixed))
