import test from 'node:test'
import assert from 'node:assert/strict'
import {validParallelWorkers,runBoundedSourceTasks,sourceProgressSummary} from '../source-distance-queue.js'
const jobs=Array.from({length:7},(_,i)=>({modality:`view-${i}`,files:[{name:`s${i}`}]}))
const nap = ()=>new Promise(resolve=>setImmediate(resolve))

test('bounded pool overlaps independent data types but never exceeds the limit',async()=>{
  let active=0,peak=0
  const seen=[]
  const result=await runBoundedSourceTasks(jobs,{parallel:2,run:async(job,index,progress)=>{
    active++;peak=Math.max(active,peak)
    progress({completed:1,total:2,message:`Parsing ${job.modality}`})
    await nap();await nap()
    active--;seen.push(index)
    return job.modality
  }})
  assert.equal(peak,2)
  assert.equal(seen.length,7)
  assert.equal(result.outcomes.filter(x=>x.status==='complete').length,7)
  assert.equal(result.summary.percent,100)
})

test('worker failure does not discard other modality results',async()=>{
  const x=await runBoundedSourceTasks(jobs,{parallel:3,run:async(job,i)=>{
    if(i===2)throw new Error('damaged workbook')
    return `result-${i}`
  }})
  assert.equal(x.outcomes.filter(o=>o.status==='failed').length,1)
  assert.equal(x.outcomes.filter(o=>o.status==='complete').length,6)
  assert.equal(x.outcomes[2].error.message,'damaged workbook')
  assert.equal(x.outcomes[6].value,'result-6')
})

test('abort does not launch queued tasks and keeps completed results',async()=>{
  const control=new AbortController()
  const seen=[]
  const x=await runBoundedSourceTasks(jobs,{parallel:2,signal:control.signal,run:async(job,i)=>{
    seen.push(i)
    if(i===0){control.abort();return 'completed first'}
    await nap();throw new Error('stopped')
  }})
  assert.ok(seen.length<=2)
  assert.ok(x.outcomes.every(v=>v.status==='stopped'))
  assert.equal(x.outcomes[6].status,'stopped')
})

test('one-worker mode remains sequential and settings are bounded',async()=>{
  assert.deepEqual([validParallelWorkers('1'),validParallelWorkers(2),validParallelWorkers(3),validParallelWorkers(99),validParallelWorkers('oops')],[1,2,3,2,2])
  let running=0,peak=0
  await runBoundedSourceTasks(jobs,{parallel:1,run:async()=>{running++;peak=Math.max(peak,running);await nap();running--}})
  assert.equal(peak,1)
})

test('progress data remains finite and completion counts preserve sample totals',async()=>{
  const snapshots=[]
  const x=await runBoundedSourceTasks([jobs[0]],{run:async(job,i,progress)=>{
    progress({completed:0,total:10,message:'Parsing sample 1'})
    progress({completed:8,total:10,message:'Parsing sample 9'})
    return 'ok'
  },onUpdate:(row,sum)=>snapshots.push({row:{...row},sum:{...sum}})})
  assert.ok(snapshots.find(s=>s.row.completed===8 && s.sum.percent===80))
  assert.equal(x.rows[0].completed,10)
  assert.equal(sourceProgressSummary([{total:2,completed:50,status:'running'}]).percent,100)
})

test('parallel source scheduler has no labels, fetch or remote dependency',async()=>{
  const fs=await import('node:fs/promises')
  const source=await fs.readFile(new URL('../source-distance-queue.js',import.meta.url),'utf8')
  assert.doesNotMatch(source,/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\/|caseValues|controlValues|q_value|beta_group/)
})
