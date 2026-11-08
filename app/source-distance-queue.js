/** Local-only, bounded worker scheduler for independent unsupervised source views.
 * Each view retains its own isolated WASM session. The queue makes no decisions
 * using phenotype, adjusted hits, sample labels, or measurement values.
 */
export function validParallelWorkers(value) {
  const n = Number(value)
  return Number.isInteger(n) && n >= 1 && n <= 3 ? n : 2
}

export function sourceProgressSummary(rows) {
  if (!Array.isArray(rows)) throw new Error('Invalid source progress state')
  const total = rows.reduce((sum, row) => sum + row.total, 0)
  const completed = rows.reduce((sum, row) => sum + Math.max(0, Math.min(row.total, row.completed)), 0)
  const finished = rows.filter(row => ['complete','failed','stopped'].includes(row.status)).length
  return {total, completed, finished, views:rows.length, percent:total ? Math.round(completed*100/total) : 0}
}

/**
 * Run no more than `parallel` independent views simultaneously. Worker callbacks
 * report progress in [0,total]. Failures return as a separate result, not as an
 * exception that discards completed modalities. Abort stops launching new work.
 * Caller owns individual workers and uses the shared AbortSignal to stop them.
 */
export async function runBoundedSourceTasks(entries, {parallel = 2, signal, run, onUpdate = () => {}} = {}) {
  if (!Array.isArray(entries) || typeof run !== 'function') throw new Error('Invalid source calculation queue')
  const limit = validParallelWorkers(parallel)
  const rows = entries.map((entry, index) => ({
    index, modality:entry.modality,
    status:'queued', completed:0, total:Math.max(1, entry.files?.length || 1) + (entry.sampleIds ? 0 : 1),
    message:'Queued',
  }))
  const outcomes = new Array(entries.length)
  let cursor=0
  function update(index, patch) {
    const row = rows[index]
    Object.assign(row, patch)
    row.completed = Math.max(0, Math.min(row.total, Number(row.completed)||0))
    onUpdate(row, sourceProgressSummary(rows), rows)
  }
  async function runner() {
    while(cursor < entries.length && !signal?.aborted) {
      const index=cursor++
      const entry=entries[index]
      update(index,{status:'running',message:'Starting local worker'})
      try {
        const value=await run(entry,index,progress=>{
          if(signal?.aborted || rows[index].status!=='running')return
          const total=Number(progress?.total)
          const completed=Number(progress?.completed)
          update(index,{completed:Number.isFinite(completed)?completed:rows[index].completed,
            total:Number.isFinite(total)&&total>0?total:rows[index].total,
            message:String(progress?.message||'Calculating distances')})
        })
        if(signal?.aborted) {
          outcomes[index]={status:'stopped',entry}
          update(index,{status:'stopped',message:'Stopped'})
        }else{
          outcomes[index]={status:'complete',entry,value}
          update(index,{status:'complete',completed:rows[index].total,message:'Distance calculation complete'})
        }
      } catch(error) {
        if(signal?.aborted) {
          outcomes[index]={status:'stopped',entry}
          update(index,{status:'stopped',message:'Stopped'})
        } else {
          outcomes[index]={status:'failed',entry,error:error instanceof Error?error:new Error(String(error))}
          update(index,{status:'failed',completed:rows[index].total,message:'Needs review'})
        }
      }
    }
  }
  await Promise.all(Array.from({length:Math.min(limit,entries.length)},runner))
  if(signal?.aborted)for(let i=0;i<entries.length;i++)if(!outcomes[i]){
    outcomes[i]={status:'stopped',entry:entries[i]}
    update(i,{status:'stopped',message:'Not started'})
  }
  return {outcomes,rows,summary:sourceProgressSummary(rows)}
}
