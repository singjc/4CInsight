import test from 'node:test'
import assert from 'node:assert/strict'
import {rankExpressionFeatures} from '../biological-feature-ranking.js'
const samples=Array.from({length:12},(_,i)=>({alias:`S${i}`,observation:`O${i}`,x:i,y:i%3}))
const header=['Gene Symbol','NCBI Gene ID',...samples.map(s=>`${s.alias} TPM`)].join('\t')
test('all feature scores are retained, but sample overlay profiles stay bounded to 24',async()=>{
 const lines=[header]
 for(let j=0;j<37;j++){
  const values=samples.map((s,i)=>String(1+j+(j%2===0?i:11-i)+(j%4===0?(i%3):0)))
  lines.push([`GENE_${j}`,`ID${j}`,...values].join('\t'))
 }
 const result=await rankExpressionFeatures(new Blob([lines.join('\n')+'\n']),{modality:'gene',samples})
 assert.equal(result.features_ranked,37)
 assert.equal(result.rows.length,37)
 assert.equal(result.profiles.length,24)
 for(const p of result.profiles){
  assert.ok(result.rows.some(r=>r.feature===p.feature&&r.secondary_id===p.secondary_id))
  assert.equal(p.values.length,12)
  assert.equal(new Set(p.values.map(v=>v.observation)).size,12)
  assert.ok(p.values.every(v=>Number.isFinite(v.log2_tpm)))
 }
 assert.ok(result.profiles.every((p,i)=>i===0||result.profiles[i-1].strength>=p.strength))
 assert.ok(result.rows.every(r=>!('values' in r)), 'Do not copy every observation into every full feature row')
})
test('feature profile uses exact sample IDs and excludes invalid values instead of imputing',async()=>{
 const values=samples.map((s,i)=>i===0?'':String(i+1))
 const result=await rankExpressionFeatures(new Blob([header+'\n' + ['KEEP','ID',...values].join('\t')+'\n']),{modality:'gene',samples})
 assert.equal(result.features_ranked,1)
 assert.equal(result.profiles[0].values.length,11)
 assert.equal(result.profiles[0].values.some(v=>v.observation==='O0'),false)
})
