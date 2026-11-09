/** 4C Insight local-only workflow coordination.
 * The caller supplies the validated science steps. This module does not fit models.
 * Stop and failed required steps do not silently count as success.
 */
export async function runWorkflowSteps(stages, {signal, onUpdate=()=>{}}={}) {
  if (!Array.isArray(stages) || !stages.length || stages.some(s=>!s?.id || !s?.label || typeof s.run !== 'function')) {
    throw new Error('Invalid complete-analysis workflow')
  }
  const ids=new Set(stages.map(s=>s.id))
  if(ids.size!==stages.length)throw new Error('Repeated analysis step ID')
  const rows=stages.map(s=>({id:s.id,label:s.label,status:'queued',message:''}))
  const emit=()=>onUpdate(rows.map(row=>({...row})))
  emit()
  for (let i=0;i<stages.length;i++) {
    if (signal?.aborted) {
      rows[i].status='stopped';rows[i].message='Stopped by the user';emit()
      throw new Error('Complete analysis stopped')
    }
    const s=stages[i], row=rows[i]
    row.status='running'; row.message='In progress'; emit()
    try {
      const result=await s.run()
      if(signal?.aborted)throw new Error('Complete analysis stopped')
      if(result?.ok===false)throw new Error(result.message||`${s.label} did not complete`)
      if(result?.status==='skipped')row.status='skipped'
      else if(result?.status==='warning')row.status='warning'
      else row.status='complete'
      row.message=result?.message|| (row.status==='complete'?'Complete':'Not run')
    } catch(error) {
      if(signal?.aborted || /complete analysis stopped/i.test(String(error?.message))) {
        row.status='stopped';row.message='Stopped by the user';emit()
        throw new Error('Complete analysis stopped')
      }
      row.message=error instanceof Error?error.message:String(error)
      row.status=s.optional?'warning':'failed';emit()
      if(!s.optional)throw new Error(`${s.label}: ${row.message}`)
    }
    emit()
  }
  return rows
}

const num=x=>typeof x==='number'&&Number.isFinite(x)
/** Only source-supported counts and descriptive measures, never clinical claims. */
export function integratedOverviewMetrics({alignment,structure,stability,features,sourceFeatures}={}) {
  const eligible=alignment?.summary?.eligible
  const retained=structure?.observations?.length
  const modalities=structure?.fixed_modalities||[]
  const ari=stability?.adjusted_rand?.mean
  const rna=Object.values(features?.byModality||{}).reduce((n,v)=>n+(v?.features_ranked||0),0)
  const nonRna=Object.values(sourceFeatures?.byModality||{}).reduce((n,v)=>n+(v?.features_ranked||0),0)
  return {
    eligible:num(eligible)?eligible:null,
    retained:Number.isInteger(retained)?retained:null,
    excluded:num(eligible)&&Number.isInteger(retained)?Math.max(0,eligible-retained):null,
    modalities:modalities.length,
    modalityNames:[...modalities],
    meanAri:num(ari)?ari:null,
    rnaFeatureCount:rna,
    otherFeatureCount:nonRna,
  }
}
