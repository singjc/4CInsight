import test from 'node:test'
import assert from 'node:assert/strict'
import { buildBiologicalFindings, findingsTsv, spearman, rankValues } from '../biological-findings.js'

const observations=['S1','S2','S3','S4','S5','S6','S7','S8']
const merges=[
  {id:8,left:0,right:1},{id:9,left:2,right:3},{id:10,left:4,right:5},{id:11,left:6,right:7},
  {id:12,left:8,right:9},{id:13,left:10,right:11},{id:14,left:12,right:13},
]
const D=Array.from({length:8},(_,i)=>Array.from({length:8},(_,j)=>i===j?0:Math.abs(i-j)+1)).flat()
const points=observations.map((observation,i)=>({observation,x:i-3.5,y:4-i}))
const structure={schema:'4c-insight-joint-structure-v1',observations,distances:D,
 fixed_modalities:['gene','immune'],clustering:{merges},pcoa:{points}}
const metadata={columns:['sample','phenotype','severity','qc_status','subject'],rows:observations.map((o,i)=>[
 o,i<4?'Control':'Case',String(i+1),i===2?'Failed':'Pass',i===1?'P1':o,
])}
const align={schema:'4c-insight-observation-alignment-v1',roles:{observationColumn:'sample'},
 rows:observations.map(o=>({observation:o,subject:o,excluded:false,timepoint:'',modalities:{
  gene:{status:'present',alias:o},immune:{status:'present',alias:o},}}))}
const pairs=[];for(let i=0;i<8;i++)for(let j=i+1;j<8;j++)pairs.push({a:observations[i],b:observations[j],distance:D[i*8+j]})
const views=[{modality:'gene',distance_label:'gene distance',pairs},
 {modality:'immune',distance_label:'signature distance',pairs:pairs.map(p=>({...p,distance:p.distance*2}))}]

test('tie-aware Spearman has correct identity, inverse and constant diagnostics',()=>{
 assert.deepEqual(rankValues([2,1,1,3]),[3,1.5,1.5,4])
 assert.equal(spearman([1,2,3,4],[1,2,3,4]),1)
 assert.equal(spearman([1,2,3,4],[4,3,2,1]),-1)
 assert.equal(spearman([0,0,0],[3,2,1]),null)
})
test('metadata is applied only after fixed PCoA and cluster fitting',()=>{
 const report=buildBiologicalFindings(structure,align,metadata,views,{column:'phenotype',clusters:2})
 assert.equal(report.observations,8)
 assert.equal(report.metadata.kind,'categorical')
 assert.equal(report.metadata.categories.find(x=>x.value==='Case').n,4)
 assert.equal(report.coverage.length,8)
 assert.equal(report.modality.length,2)
 assert.equal(report.modality[1].pairs,28)
 assert.equal(report.points[0].x,structure.pcoa.points[0].x)
 assert.equal(report.points[0].cluster,1)
 assert.match(report.warnings.join(' '),/exploratory/)
})
test('numeric covariates report in-sample rank correlation without p-values',()=>{
 const report=buildBiologicalFindings(structure,align,metadata,views,{column:'severity',clusters:2})
 assert.equal(report.metadata.kind,'numeric')
 assert.equal(report.metadata.pcoa_1_rho,1)
 assert.ok(!('p_value' in report.metadata))
})
test('distance contribution uses same fixed-cohort pairs and exact aliases',()=>{
 const report=buildBiologicalFindings(structure,align,metadata,views,{clusters:2})
 assert.equal(report.modality[0].joint_distance_correlation,1)
 assert.equal(report.modality[1].joint_distance_correlation,1)
 assert.equal(report.modality[0].expected_pairs,28)
 assert.ok(report.modality[0].between_within_ratio>0)
})
test('missing modality measurements do not become zeros',()=>{
 const shortened=views[0].pairs.slice(0,12)
 const report=buildBiologicalFindings(structure,align,metadata,[{...views[0],pairs:shortened},views[1]],{clusters:2})
 assert.equal(report.modality[0].pairs,12)
 assert.equal(report.modality[0].expected_pairs,28)
 assert.ok(report.modality[0].scale>0)
})
test('duplicate clinical observations and ambiguous source aliases fail closed',()=>{
 const duplicated={...metadata,rows:[...metadata.rows,metadata.rows[0]]}
 assert.throws(()=>buildBiologicalFindings(structure,align,duplicated,views,{column:'phenotype',clusters:2}),/Duplicate metadata/)
 const bad={...align,rows:align.rows.map(x=>({...x,modalities:{...x.modalities,gene:{...x.modalities.gene,alias:'identical'}}}))}
 assert.throws(()=>buildBiologicalFindings(structure,bad,metadata,views,{clusters:2}),/Ambiguous/)
})
test('TSV export preserves all samples and protects spreadsheet formulas',()=>{
 const values={...metadata,rows:metadata.rows.map((r,i)=>i===0?[r[0],'=SUM(1,2)',...r.slice(2)]:r)}
 const report=buildBiologicalFindings(structure,align,values,views,{column:'phenotype',clusters:2})
 const tsv=findingsTsv(report)
 assert.equal(tsv.trim().split('\n').length,9)
 assert.match(tsv,/'=SUM\(1,2\)/)
})
test('report has no significance p/q values, and patient/phenotype do not alter input distances',()=>{
 const a=buildBiologicalFindings(structure,align,metadata,views,{column:'phenotype',clusters:2})
 const b=buildBiologicalFindings(structure,align,metadata,views,{column:'qc_status',clusters:2})
 assert.deepEqual(a.modality,b.modality)
 assert.deepEqual(a.points.map(x=>[x.x,x.y,x.cluster]),b.points.map(x=>[x.x,x.y,x.cluster]))
 assert.doesNotMatch(JSON.stringify(a),/"(?:p_value|q_value|fdr)":/)
})
