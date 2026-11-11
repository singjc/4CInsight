import test from 'node:test'
import assert from 'node:assert/strict'
import {runPathwayEnrichment,enrichmentScore,hypergeometricTail,bhAdjust} from '../pathway-enrichment.js'

// ORA reference values were calculated separately with SciPy 1.17.0
// scipy.stats.hypergeom.sf(k-1,N,K,n); BH checked with statsmodels 0.14.6.
// These checks do not use the production hypergeometric code to form expectations.
const expected={
  early:{p:0.0003869969040247678,q:0.0011609907120743034},
  late:{p:1,q:1},
  mixed:{p:0.8708720330237358,q:1},
}
const input={source:'compare',method:'Predefined case/control scores',provenance:'Fixed scientific reference',
  diagnostics:{},rows:Array.from({length:20},(_,i)=>({gene:`G${i+1}`,score:10-i,q:i<6?0.002:0.9}))}
const sets={filename:'fixed-reference.gmt',source:'Synthetic reference',warnings:[],pathways:[
  {name:'early',description:'fixed-early',genes:['G1','G2','G3','G4','G5']},
  {name:'late',description:'fixed-late',genes:['G16','G17','G18','G19','G20']},
  {name:'mixed',description:'fixed-mixed',genes:['G2','G9','G14','G18','G20']},
]}
const close=(a,b,tol=1e-12)=>assert.ok(Math.abs(a-b)<tol,`${a} vs ${b}`)

test('P4: ORA agrees with external SciPy hypergeometric and statsmodels BH results',async()=>{
  const report=await runPathwayEnrichment({input,geneSets:sets,options:{mode:'ora',minSize:3,maxSize:20,qThreshold:.01,effectThreshold:1,direction:'both'}})
  assert.equal(report.backgroundGenes,20)
  assert.equal(report.selectedGenes,6)
  assert.equal(report.results.length,3)
  for(const r of report.results){close(r.p_value,expected[r.pathway].p);close(r.q_value,expected[r.pathway].q)}
  assert.equal(report.results.find(r=>r.pathway==='early').overlap,5)
  assert.equal(report.results.find(r=>r.pathway==='late').overlap,0)
  assert.equal(report.results.find(r=>r.pathway==='mixed').overlap,1)
})

// A deliberately simple independent full-length running-sum reference.
// It does not use the production gap-count/peak algorithm.
function weightedKsReference(indices,scores){
  const selected=new Set(indices),n=scores.length,m=selected.size
  const sum=indices.reduce((a,i)=>a+Math.abs(scores[i]),0)
  let cumulative=0,positive=-Infinity,negative=Infinity
  for(let i=0;i<n;i++){
    cumulative+=selected.has(i)?(sum?Math.abs(scores[i])/sum:1/m):(-1/(n-m))
    positive=Math.max(positive,cumulative)
    negative=Math.min(negative,cumulative)
  }
  return Math.abs(positive)>=Math.abs(negative)?positive:negative
}
test('P4: ranked enrichment weighted KS score equals independent full-length running sum',()=>{
  const scores=input.rows.map(r=>r.score)
  for(const names of [['G1','G2','G3','G4','G5'],['G16','G17','G18','G19','G20'],['G2','G9','G14','G18','G20']]){
    const ids=names.map(g=>Number(g.slice(1))-1)
    close(enrichmentScore(ids,scores).score,weightedKsReference(ids,scores))
  }
})
test('P4: ranked permutation results have valid empirical resolution and corrected q bounds',async()=>{
  const opts={mode:'ranked',minSize:3,maxSize:20,seed:323,permutations:199}
  const report=await runPathwayEnrichment({input,geneSets:sets,options:opts})
  assert.equal(report.results.length,3)
  for(const r of report.results){
    assert.ok(r.p_value>=1/(opts.permutations+1)-1e-12)
    assert.ok(r.p_value<=1&&r.q_value>=r.p_value-1e-12&&r.q_value<=1)
    assert.ok(Number.isFinite(r.enrichmentScore))
  }
  assert.deepEqual(report.results,(await runPathwayEnrichment({input,geneSets:sets,options:opts})).results)
  assert.match(report.correction,/not phenotype permutations/)
})
