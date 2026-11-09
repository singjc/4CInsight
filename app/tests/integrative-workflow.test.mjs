import test from 'node:test'
import assert from 'node:assert/strict'
import {runWorkflowSteps,integratedOverviewMetrics} from '../integrative-workflow.js'

test('runs exact stages in order and publishes independent stage states',async()=>{
 const called=[],updates=[]
 const rows=await runWorkflowSteps([
  {id:'align',label:'Align',run:async()=>{called.push('align');return{message:'3 observations'}}},
  {id:'dist',label:'Distances',run:async()=>{called.push('dist');return{status:'warning',message:'One view failed'}}},
  {id:'structure',label:'Structure',run:async()=>{called.push('structure')}}
 ],{onUpdate:rows=>updates.push(rows)})
 assert.deepEqual(called,['align','dist','structure'])
 assert.deepEqual(rows.map(x=>x.status),['complete','warning','complete'])
 assert.ok(updates.some(x=>x[1].status==='running'))
 assert.equal(updates.at(-1)[0].status,'complete')
})
test('a failed required scientific stage blocks later results',async()=>{
 let later=false
 await assert.rejects(runWorkflowSteps([
  {id:'align',label:'Align',run:async()=>({ok:false,message:'Missing sample identity'})},
  {id:'fit',label:'Joint fit',run:async()=>{later=true}}
 ]),/Align: Missing sample identity/)
 assert.equal(later,false)
})
test('an optional unsupported stage does not masquerade as success',async()=>{
 const rows=await runWorkflowSteps([
  {id:'stability',label:'Stability',optional:true,run:async()=>{throw new Error('No patient IDs')}},
  {id:'summary',label:'Summary',run:async()=>({message:'Ready'})},
 ])
 assert.deepEqual(rows.map(x=>x.status),['warning','complete'])
 assert.match(rows[0].message,/No patient IDs/)
})
test('stop aborts without starting the next scientific stage',async()=>{
 const controller=new AbortController();let called=0
 await assert.rejects(runWorkflowSteps([
  {id:'dist',label:'Distances',run:async()=>{called++;controller.abort()}},
  {id:'joint',label:'Joint',run:async()=>{called++}},
 ],{signal:controller.signal}),/stopped/i)
 assert.equal(called,1)
})
test('already aborted signal prevents all processing',async()=>{
 const c=new AbortController();c.abort()
 await assert.rejects(runWorkflowSteps([{id:'x',label:'X',run:async()=>{throw Error('should not run')}}],{signal:c.signal}),/stopped/)
})
test('rejects repeated step identity before any fitting',async()=>{
 await assert.rejects(runWorkflowSteps([{id:'x',label:'One',run:async()=>{}},{id:'x',label:'Two',run:async()=>{}}]),/Repeated/)
})
test('overview uses only observed counts and exploratory stability',()=>{
 const out=integratedOverviewMetrics({alignment:{summary:{eligible:322}},structure:{observations:Array(217).fill('S'),fixed_modalities:['gene','bcr','immune']},stability:{adjusted_rand:{mean:0.36}},features:{byModality:{gene:{features_ranked:120}}},sourceFeatures:{byModality:{bcr:{features_ranked:14}}}})
 assert.deepEqual({retained:out.retained,excluded:out.excluded,views:out.modalities,ari:out.meanAri,rna:out.rnaFeatureCount,nonRna:out.otherFeatureCount},{retained:217,excluded:105,views:3,ari:.36,rna:120,nonRna:14})
})
test('overview does not invent unavailable model and biological evidence',()=>{
 const out=integratedOverviewMetrics()
 assert.equal(out.eligible,null)
 assert.equal(out.meanAri,null)
 assert.equal(out.otherFeatureCount,0)
})
