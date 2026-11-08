/**
 * Joint multi-omics sample structure from a fixed-panel, pair-complete distance matrix.
 * Pure numerical analysis, independent of phenotype, case/control and feature selection.
 * Browser worker executes this module; native Rust/WASM produces the source distances.
 *
 * Strict rule: all fitted pairs have exactly the same required modality set.
 * The deterministic highest-missing-degree pruning finds ONE complete-distance
 * subset. It is not a maximum-clique solver, and its selection can be biased.
 */
const scientificNumber = x => typeof x === 'number' && Number.isFinite(x)
const pairKey = (a, b) => a < b ? `${a}\u0000${b}` : `${b}\u0000${a}`

function validate(result, selectedModalities, patientByObservation) {
  if (!result || !Array.isArray(result.observations) || !Array.isArray(result.pairs) || !Array.isArray(result.views)) {
    throw new Error('Calculate a joint distance result before joint ordination')
  }
  if (!Array.isArray(selectedModalities) || selectedModalities.length < 2 || new Set(selectedModalities).size !== selectedModalities.length) {
    throw new Error('Select at least two distinct measured data types')
  }
  const validViews = new Set(result.views.map(v => v.modality))
  if (selectedModalities.some(v => !validViews.has(v))) throw new Error('Selected data type is missing from the distance result')
  if (result.views.length !== selectedModalities.length || result.min_views !== selectedModalities.length) {
    throw new Error('Recalculate joint distances with exactly the chosen data types and full pair coverage before ordination')
  }
  const ids = result.observations
  if (new Set(ids).size !== ids.length || ids.some(x => typeof x !== 'string' || !x || x.trim() !== x)) {
    throw new Error('Joint distance result has invalid observation identities')
  }
  if (patientByObservation && (typeof patientByObservation !== 'object' || Array.isArray(patientByObservation))) {
    throw new Error('Patient lookup must use observation IDs')
  }
}

/** A fixed-panel complete pair subset, with no numerical imputation. */
export function selectCompleteDistanceCohort(result, { selectedModalities, patientByObservation = {}, onePerPatient = false } = {}) {
  validate(result, selectedModalities, patientByObservation)
  const allowed = new Set(selectedModalities)
  const all = result.observations
  const skippedPatients = []
  const patientSeen = new Set()
  const retained = []
  for (const id of all) {
    const patient = patientByObservation[id]
    if (onePerPatient) {
      if (typeof patient !== 'string' || !patient.trim()) throw new Error(`No patient ID for '${id}'; turn off one observation per patient or select a patient column`)
      if (patientSeen.has(patient)) { skippedPatients.push(id); continue }
      patientSeen.add(patient)
    }
    retained.push(id)
  }
  const eligible = new Set(retained)
  const distances = new Map()
  let nonFixedPairs = 0
  for (const p of result.pairs) {
    if (!p || typeof p.a !== 'string' || typeof p.b !== 'string' || p.a === p.b || !scientificNumber(p.distance) || p.distance < 0) {
      throw new Error('Invalid numerical distance or pair identity')
    }
    if (!eligible.has(p.a) || !eligible.has(p.b)) continue
    if (!Array.isArray(p.modalities) || p.modalities.length !== allowed.size || p.modalities.some(x => !allowed.has(x)) || p.contributing_views !== allowed.size) {
      nonFixedPairs++
      continue
    }
    const k = pairKey(p.a,p.b)
    if (distances.has(k)) throw new Error('Duplicate unordered joint-distance pair')
    distances.set(k,p.distance)
  }
  const missingDegrees = new Map(retained.map(id => [id,0]))
  let availablePairCount = 0
  for (let i=0;i<retained.length;i++) for(let j=i+1;j<retained.length;j++) {
    if (distances.has(pairKey(retained[i],retained[j]))) availablePairCount++
    else {missingDegrees.set(retained[i],missingDegrees.get(retained[i])+1); missingDegrees.set(retained[j],missingDegrees.get(retained[j])+1)}
  }
  const active = new Set(retained)
  const removedForCoverage = []
  while (active.size >= 2) {
    let worst = null, worstMissing = 0
    // A tie removes the later metadata observation, preserving earlier rows.
    for (const id of retained) {
      if (!active.has(id)) continue
      const count = missingDegrees.get(id)
      if (count > worstMissing || (count === worstMissing && count > 0)) {worst = id; worstMissing = count}
    }
    if (!worstMissing) break
    active.delete(worst)
    removedForCoverage.push(worst)
    for (const other of active) {
      if (!distances.has(pairKey(worst,other))) missingDegrees.set(other, missingDegrees.get(other)-1)
    }
  }
  const included = retained.filter(id => active.has(id))
  if (included.length < 3) {
    throw new Error(`Only ${included.length} observations form a complete fixed-view distance matrix. Choose fewer data types or inspect source coverage. No missing distances were imputed.`)
  }
  const n = included.length
  const matrix = new Float64Array(n*n)
  for(let i=0;i<n;i++) for(let j=i+1;j<n;j++) {
    const d = distances.get(pairKey(included[i],included[j]))
    if (!scientificNumber(d)) throw new Error('Internal error: an incomplete pair entered the joint model')
    matrix[i*n+j] = d; matrix[j*n+i] = d
  }
  const eligiblePairs = retained.length*(retained.length-1)/2
  return {
    included, matrix,
    coverage: {
      metadata_observations: all.length,
      observations_after_patient_option: retained.length,
      patient_deduplicated: skippedPatients.length,
      available_fixed_view_pairs: availablePairCount,
      eligible_pairs: eligiblePairs,
      pair_coverage: eligiblePairs ? availablePairCount/eligiblePairs : 0,
      retained_observations: included.length,
      removed_for_missing_pairs: removedForCoverage.length,
      removed_observation_ids: removedForCoverage,
      patient_deduplicated_ids: skippedPatients,
      non_fixed_pairs: nonFixedPairs,
      selection: 'deterministic greedy highest-missing-degree removal; not guaranteed maximum cohort',
    }
  }
}

/** Numerically stable cyclic Jacobi diagonalization of real symmetric B. */
function symmetricEigenvaluesAndVectors(matrix,n) {
  const a = new Float64Array(matrix)
  const v = new Float64Array(n*n)
  for(let i=0;i<n;i++) v[i*n+i]=1
  const scale = Math.max(1, ...Array.from({length:n},(_,i)=>Math.abs(a[i*n+i])))
  const tolerance = 1e-12*scale
  // No user-data-size limit; O(n^3) cost is reported in the UI.
  const maxSweeps = 60
  let converged = false, sweeps = 0, maximumOffDiagonal = 0
  for (let sweep=0;sweep<maxSweeps;sweep++) {
    sweeps=sweep+1
    let maxOff=0, changed=false
    for(let p=0;p<n-1;p++) for(let q=p+1;q<n;q++) {
      const pq=p*n+q, apq=a[pq], abs=Math.abs(apq)
      if(abs>maxOff) maxOff=abs
      if(abs<=tolerance) continue
      changed=true
      const tau=(a[q*n+q]-a[p*n+p])/(2*apq)
      const t=(tau>=0?1:-1)/(Math.abs(tau)+Math.sqrt(1+tau*tau))
      const c=1/Math.sqrt(1+t*t), s=t*c
      const app=a[p*n+p], aqq=a[q*n+q]
      a[p*n+p]=app-t*apq; a[q*n+q]=aqq+t*apq
      a[pq]=0; a[q*n+p]=0
      for(let k=0;k<n;k++) {
        if(k!==p&&k!==q) {
          const akp=a[k*n+p], akq=a[k*n+q]
          const x=c*akp-s*akq, y=s*akp+c*akq
          a[k*n+p]=x; a[p*n+k]=x; a[k*n+q]=y; a[q*n+k]=y
        }
        const vkp=v[k*n+p], vkq=v[k*n+q]
        v[k*n+p]=c*vkp-s*vkq; v[k*n+q]=s*vkp+c*vkq
      }
    }
    maximumOffDiagonal=maxOff
    if(!changed||maxOff<=tolerance){converged=true;break}
  }
  if(!converged) throw new Error(`Joint PCoA eigensolver did not converge in ${maxSweeps} sweeps (largest off-diagonal ${maximumOffDiagonal})`)
  return {eigenvalues:Array.from({length:n},(_,i)=>a[i*n+i]),vectors:v,sweeps,maximumOffDiagonal}
}

/** Classical principal coordinates analysis on complete, nonnegative distances. */
export function principalCoordinates(matrix,n) {
  if(!(matrix instanceof Float64Array)||matrix.length!==n*n||!Number.isInteger(n)||n<3) throw new Error('PCoA needs a complete matrix with at least three observations')
  const rowMeans=new Float64Array(n)
  let grand=0
  for(let i=0;i<n;i++) for(let j=0;j<n;j++) {
    const d=matrix[i*n+j]
    if(!scientificNumber(d)||d<0||Math.abs(d-matrix[j*n+i])>1e-9) throw new Error('Distance matrix must be finite, nonnegative and symmetric')
    if(i===j&&Math.abs(d)>1e-10) throw new Error('Distance matrix diagonal must be zero')
    rowMeans[i]+=d*d
  }
  for(let i=0;i<n;i++){rowMeans[i]/=n;grand+=rowMeans[i]}
  grand/=n
  const b=new Float64Array(n*n)
  for(let i=0;i<n;i++) for(let j=0;j<n;j++) b[i*n+j]=-0.5*(matrix[i*n+j]**2-rowMeans[i]-rowMeans[j]+grand)
  const eigen=symmetricEigenvaluesAndVectors(b,n)
  const ranked=eigen.eigenvalues.map((value,i)=>({value,i})).sort((a,b)=>b.value-a.value)
  const maximum=Math.max(1,Math.abs(ranked[0].value))
  const threshold=maximum*1e-9
  const positive=ranked.filter(x=>x.value>threshold)
  const negative=ranked.filter(x=>x.value< -threshold)
  const positiveSum=positive.reduce((s,x)=>s+x.value,0)
  const negativeAbs=negative.reduce((s,x)=>s-Math.min(0,x.value),0)
  const axes=positive.slice(0,2)
  if(axes.length<2) throw new Error('The joint distance matrix has fewer than two positive PCoA axes')
  const coords=Array.from({length:n},(_,row)=>{
    const values=axes.map(({value,i})=>eigen.vectors[row*n+i]*Math.sqrt(value))
    // Principal eigenvectors are sign-ambiguous. Fix sign by largest |coordinate|.
    return values
  })
  for(let axis=0;axis<2;axis++){
    let max=-1, sign=1
    for(let row=0;row<n;row++){const x=coords[row][axis];if(Math.abs(x)>max){max=Math.abs(x);sign=x<0?-1:1}}
    if(sign<0) for(const item of coords)item[axis]=-item[axis]
  }
  return {
    coordinates:coords,
    eigenvalues:ranked.map(x=>x.value),
    positive_axes:positive.length,
    negative_axes:negative.length,
    positive_inertia:positiveSum,
    negative_inertia_absolute:negativeAbs,
    negative_inertia_ratio:positiveSum+negativeAbs ? negativeAbs/(positiveSum+negativeAbs):0,
    axis_variance_explained: axes.map(x=>positiveSum?x.value/positiveSum:0),
    eigen_sweeps:eigen.sweeps,
    eigen_max_off_diagonal:eigen.maximumOffDiagonal,
    method:'Classical PCoA on squared fixed-panel distances; negative eigenvalues retained as diagnostics, not plotted',
  }
}

/** Average-linkage (UPGMA) agglomeration of the SAME complete distance cohort. */
export function hierarchicalAverageLinkage(matrix,n) {
  if(!(matrix instanceof Float64Array)||matrix.length!==n*n||n<3) throw new Error('Hierarchical clustering needs a complete distance matrix')
  const capacity=2*n-1
  const distances=new Float64Array(capacity*capacity)
  for(let i=0;i<n;i++)for(let j=0;j<n;j++)distances[i*capacity+j]=matrix[i*n+j]
  const active=new Set(Array.from({length:n},(_,i)=>i))
  const sizes=new Int32Array(capacity).fill(1)
  const children=new Map()
  const merges=[]
  let next=n
  while(active.size>1){
    const ordered=[...active].sort((a,b)=>a-b)
    let ai=-1,bi=-1,best=Infinity
    for(let i=0;i<ordered.length;i++)for(let j=i+1;j<ordered.length;j++){
      const a=ordered[i],b=ordered[j],d=distances[a*capacity+b]
      if(d<best){best=d;ai=a;bi=b}
    }
    if(ai<0||!Number.isFinite(best))throw new Error('Nonfinite linkage distance')
    const id=next++
    const size=sizes[ai]+sizes[bi]
    sizes[id]=size
    children.set(id,[ai,bi])
    merges.push({id,left:ai,right:bi,height:best,size})
    for(const other of active) {
      if(other===ai||other===bi)continue
      const d=(sizes[ai]*distances[ai*capacity+other]+sizes[bi]*distances[bi*capacity+other])/size
      distances[id*capacity+other]=d;distances[other*capacity+id]=d
    }
    active.delete(ai);active.delete(bi);active.add(id)
  }
  const order=[]
  const visit=id=>{if(id<n){order.push(id);return}const [a,b]=children.get(id);visit(a);visit(b)}
  visit(next-1)
  return {merges,leaf_order:order,method:'Average linkage (UPGMA) on strict complete fixed-panel distances'}
}

/** No phenotype/diagnosis/case-control labels enter this function. */
export function analyzeJointStructure(result, options={}) {
  const selectedModalities=options.selectedModalities||result?.views?.map(view=>view.modality)
  const {included,matrix,coverage}=selectCompleteDistanceCohort(result,{selectedModalities,
    patientByObservation:options.patientByObservation||{}, onePerPatient:options.onePerPatient===true})
  const n=included.length
  const pcoa=principalCoordinates(matrix,n)
  const clustering=hierarchicalAverageLinkage(matrix,n)
  return {
    schema:'4c-insight-joint-structure-v1',
    input_model:result.model,
    fixed_modalities:[...selectedModalities],
    observations:included,
    complete_pairs:n*(n-1)/2,
    coverage,
    pcoa:{...pcoa,points:included.map((id,i)=>({observation:id,x:pcoa.coordinates[i][0],y:pcoa.coordinates[i][1]}))},
    clustering,
    distances:Array.from(matrix),
    warnings:[
      'The complete-distance cohort is selected by a deterministic greedy rule, not by random sampling or maximum-clique optimization.',
      'PCoA and clustering are descriptive. No phenotype labels, adjusted features or outcome data were used to fit them.',
      ...(pcoa.negative_inertia_ratio>0.1?['The fused distances have substantial non-Euclidean structure (negative PCoA inertia). Interpret the two-dimensional plot cautiously.']:[]),
    ],
  }
}


/**
 * Compare a leave-one-modality-out fusion on EXACTLY the original fitted
 * observation cohort. Missing pairs make this diagnostic unavailable;
 * observations are never reselected to improve ablation concordance.
 */
export function compareFixedCohortAblation(fullStructure, reducedFusion, omitted) {
  if (fullStructure?.schema !== '4c-insight-joint-structure-v1' ||
      !Array.isArray(reducedFusion?.views) || !Array.isArray(reducedFusion?.pairs)) {
    throw new Error('Invalid fixed-cohort ablation inputs')
  }
  const n=fullStructure.observations.length
  const remaining=fullStructure.fixed_modalities.filter(view=>view!==omitted)
  const expected=new Set(remaining)
  if (remaining.length<2 || reducedFusion.views.length!==remaining.length ||
      reducedFusion.views.some(view=>!expected.has(view.modality))) {
    return {omitted,available:false,reason:'At least two unchanged data types are required for a valid ablation'}
  }
  const ids=new Set(fullStructure.observations)
  const pairMap=new Map()
  for(const p of reducedFusion.pairs){
    if(!ids.has(p.a)||!ids.has(p.b))continue
    if(p.contributing_views!==remaining.length || !Array.isArray(p.modalities) ||
      p.modalities.length!==remaining.length || p.modalities.some(v=>!expected.has(v)) ||
      !scientificNumber(p.distance)||p.distance<0)continue
    const k=pairKey(p.a,p.b)
    if(pairMap.has(k))throw new Error('Duplicate fixed-cohort ablation pair')
    pairMap.set(k,p.distance)
  }
  const required=n*(n-1)/2
  if(pairMap.size!==required) return {omitted,available:false,reason:`Only ${pairMap.size}/${required} original cohort pairs retain fixed modality coverage`}
  const original=fullStructure.distances,idsOrdered=fullStructure.observations
  const full=[],reduced=[],nearestOld=[],nearestNew=[]
  for(let i=0;i<n;i++){
    let bestOld=Infinity,bestNew=Infinity,idOld='',idNew=''
    for(let j=0;j<n;j++){
      if(i===j)continue
      const x=original[i*n+j],y=pairMap.get(pairKey(idsOrdered[i],idsOrdered[j]))
      if(x<bestOld || (x===bestOld&&idsOrdered[j]<idOld)){bestOld=x;idOld=idsOrdered[j]}
      if(y<bestNew || (y===bestNew&&idsOrdered[j]<idNew)){bestNew=y;idNew=idsOrdered[j]}
      if(i<j){full.push(x);reduced.push(y)}
    }
    nearestOld.push(idOld);nearestNew.push(idNew)
  }
  const avg=(x)=>x.reduce((sum,v)=>sum+v,0)/x.length
  const mx=avg(full),my=avg(reduced)
  let a=0,b=0,c=0
  for(let k=0;k<full.length;k++){
    const dx=full[k]-mx,dy=reduced[k]-my
    a+=dx*dy;b+=dx*dx;c+=dy*dy
  }
  const den=Math.sqrt(b*c)
  return {omitted,available:true,fixed_observations:n,overlapping_pairs:required,
    distance_correlation:den>0?a/den:null,
    nearest_neighbor_retention:nearestOld.filter((id,i)=>id===nearestNew[i]).length/n,
    method:'Same observations and complete pairs, leave one data type out; descriptive sensitivity, not biological validation'}
}
