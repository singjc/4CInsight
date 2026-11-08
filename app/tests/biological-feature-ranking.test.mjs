import test from 'node:test'
import assert from 'node:assert/strict'
import {rankExpressionFeatures,selectFeatureSamples,expressionAssociationsTsv,tsvRecords} from '../biological-feature-ranking.js'

const samples=Array.from({length:10},(_,i)=>({alias:`S${i+1}`,observation:`O${i+1}`,x:i,y:9-i}))
const fakeStructure={pcoa:{points:samples.map(s=>({...s}))}}
const fakeAlign={rows:samples.map(s=>({observation:s.observation,excluded:false,modalities:{gene:{status:'present',alias:s.alias}}}))}
const head=['Gene Symbol','NCBI Gene ID',...samples.map(s=>`${s.alias} TPM`)].join('\t')
const data=[head,...['G1\t1','G2\t2','G3\t3','G4\t4'].map((prefix,k)=>prefix+'\t'+samples.map((s,i)=>k===0?i+1:k===1?10-i:k===2?0:'').join('\t'))].join('\n')+'\n'

test('feature source aliases use exact selected observations, never group names',()=>{
 assert.equal(selectFeatureSamples(fakeStructure,fakeAlign,'gene').length,10)
 assert.throws(()=>selectFeatureSamples(fakeStructure,{rows:[]},'gene'),/eight exact/)
 assert.throws(()=>selectFeatureSamples(fakeStructure,fakeAlign,'immune'),/gene and transcript/)
})
test('streaming reader preserves quoted feature names and chunk boundaries',async()=>{
 const rows=[]
 for await (const row of tsvRecords(new Blob(['A\tB\n"gene\tone"\t2\n']))){rows.push(row)}
 assert.deepEqual(rows,[['A','B'],['gene\tone','2']])
})
test('feature rank uses Spearman against fixed coordinates, no feature p-values',async()=>{
 const input=selectFeatureSamples(fakeStructure,fakeAlign,'gene')
 const result=await rankExpressionFeatures(new Blob([data]),{modality:'gene',samples:input})
 assert.equal(result.features_scanned,4)
 assert.equal(result.features_ranked,2)
 assert.equal(result.rows[0].n_used,10)
 assert.equal(result.rows.find(x=>x.feature==='G1').pcoa1_spearman,1)
 assert.equal(result.rows.find(x=>x.feature==='G2').pcoa1_spearman,-1)
 assert.equal(result.rows.some(r=>'p_value' in r||'q_value' in r),false)
})
test('missing TPM columns and invalid duplicate IDs fail without fuzzy match',async()=>{
 const input=selectFeatureSamples(fakeStructure,fakeAlign,'gene')
 assert.rejects(()=>rankExpressionFeatures(new Blob([data.replace('S5 TPM','S5 Raw Count')]),{modality:'gene',samples:input}),/Missing exact gene TPM/)
 assert.rejects(()=>rankExpressionFeatures(new Blob([data]),{modality:'gene',samples:[...input.slice(0,9),input[0]]}),/Repeated source aliases/)
})
test('reader excludes invalid and incomplete feature rows, not imputes them',async()=>{
 const input=selectFeatureSamples(fakeStructure,fakeAlign,'gene')
 const one=[head,`BAD\t1\t${samples.map((x,i)=>i<7?i+1:'').join('\t')}`].join('\n')
 const result=await rankExpressionFeatures(new Blob([one]),{modality:'gene',samples:input})
 assert.equal(result.features_ranked,0)
 assert.equal(result.features_skipped,1)
})
test('full TSV export retains scored rows and guards formula feature names',async()=>{
 const input=selectFeatureSamples(fakeStructure,fakeAlign,'gene')
 const result=await rankExpressionFeatures(new Blob([data.replace('G1\t1','=SUM(1,2)\t1')]),{modality:'gene',samples:input})
 const tsv=expressionAssociationsTsv(result)
 assert.equal(tsv.trim().split('\n').length,3)
 assert.match(tsv,/'=SUM\(1,2\)/)
})
