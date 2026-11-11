import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {
  biologicalCandidates,filterBiologicalCandidates,biologicalEvidenceRows,biologicalCandidatesTsv,
  exploratoryGeneRankingTsv,biologicalNextStepsHtml,bindBiologicalFeatureExplorer,
} from '../biological-next-steps.js'

const rna={byModality:{
  gene:{rows:[
    {feature:'IGJ',secondary_id:'ENSG1',n_used:237,pcoa1_spearman:-.59,pcoa2_spearman:.15},
    {feature:'LCK',secondary_id:'ENSG2',n_used:230,pcoa1_spearman:.3,pcoa2_spearman:-.7},
    {feature:'=1+1',secondary_id:'',n_used:223,pcoa1_spearman:.1,pcoa2_spearman:.2},
    {feature:'IGJ',secondary_id:'ENSG3',n_used:222,pcoa1_spearman:.3,pcoa2_spearman:.4},
  ]},
  transcript:{rows:[{feature:'NM_01001',secondary_id:'',n_used:237,pcoa1_spearman:.55,pcoa2_spearman:null}]},
}}
const source={byModality:{bcr:{sample_count:237,rows:[
  {feature:'V-J pairing <img src=x>',family:'V-J',n_used:221,pcoa1_spearman:.8,pcoa2_spearman:0}]},
  immune:{sample_count:230,rows:[{feature:'Myeloid signature',family:'ssGSEA',n_used:230,pcoa1_spearman:0,pcoa2_spearman:.65}]}}}
const compare={comparisonName:'Predefined Case < Control',gene:{analysis:{tests:[
  {feature:'IGJ',beta_group:.4,q_value:.2},
  {feature:'LCK',beta_group:1.4,q_value:.01},
 ],n_model_samples:201}},bcr:{analysis:{metric_tests:[{feature:'V-J diversity',beta_group:.9,q_value:.03}]}}}
const candidates=biologicalCandidates(rna,source)

test('all seven data types have evidence slots without inventing missing measurements',()=>{
  const rows=biologicalEvidenceRows({rna,source,compare,thresholds:{fdr:.05,absLog2Fc:1},enabled:{}})
  assert.equal(rows.length,7)
  assert.equal(rows.find(r=>r.key==='gene').feature,'LCK')
  assert.equal(rows.find(r=>r.key==='gene').compared,true)
  assert.equal(rows.find(r=>r.key==='gene').selected,1)
  assert.equal(rows.find(r=>r.key==='gene').tested,2)
  assert.equal(rows.find(r=>r.key==='bcr').selected,1)
  assert.equal(rows.find(r=>r.key==='tcrBeta').feature,null)
  assert.equal(rows.find(r=>r.key==='tcrBeta').compared,false)
  assert.equal(rows.find(r=>r.key==='tcrBeta').selected,null)
  assert.equal(rows.find(r=>r.key==='tcrBeta').scored,null)
})
test('ranked candidate explorer searches all measured modalities without imposing an input cap',()=>{
  assert.equal(candidates.length,7)
  assert.equal(candidates[0].source_type,'Post-fit PCoA association')
  assert.equal(filterBiologicalCandidates(candidates,{kind:'gene',query:'igj',limit:60}).total,2)
  assert.equal(filterBiologicalCandidates(candidates,{kind:'bcr',query:'V-J'}).total,1)
  const rank=filterBiologicalCandidates(candidates,{sort:'pcoa2'})
  assert.equal(rank.shown[0].feature,'LCK')
})
test('Explorer validates choices and limits visible rows only, not exported rows',()=>{
  const hundred=Array.from({length:125},(_,i)=>({kind:'gene',modality:'Gene expression',family:'RNA',feature:`g${i}`,pcoa1_spearman:i/125,pcoa2_spearman:0,strength:i/125,n_used:120}))
  const data=filterBiologicalCandidates(hundred)
  assert.equal(data.total,125);assert.equal(data.shown.length,60)
  assert.equal(biologicalCandidatesTsv(hundred).split('\n').length,127)
  assert.throws(()=>filterBiologicalCandidates(candidates,{kind:'wrong'}),/Invalid data type/)
  assert.throws(()=>filterBiologicalCandidates(candidates,{sort:'-x'}),/Invalid candidate sort/)
})
test('Exploratory gene export sorts full rank and excludes ambiguous duplicate IDs',()=>{
  const result=exploratoryGeneRankingTsv(candidates,'pcoa1_spearman')
  assert.equal(result.total,4)
  assert.equal(result.excludedDuplicateIds,2)
  assert.doesNotMatch(result.tsv,/\tIGJ\t/)
  assert.match(result.tsv,/^gene_identifier\t/m)
  assert.match(result.tsv,/'=1\+1/)
  assert.doesNotMatch(result.tsv,/q_value|p_value|FDR/)
  assert.throws(()=>exploratoryGeneRankingTsv(candidates,'q_value'),/Invalid PCoA axis/)
})
test('HTML escapes source labels, shows methodological limits and useful route actions',()=>{
  const html=biologicalNextStepsHtml({report:{membership:[{cluster:1,observations:225},{cluster:2,observations:11},{cluster:3,observations:1}]},
    coverage:{excluded:85},rna,source,compare,thresholds:{fdr:.05,absLog2Fc:1},
    observationRole:'4C_ID',patientRole:'4C_ID'})
  for(const name of ['Next biological steps','Review cohort coverage','Open Compare Groups',
    'Explore ranked features','not been verified','not been calculated','not available',
    'data-bio-next="export-features"','data-bio-next="export-genes"','data-bio-next="export-audit"'])
    assert.match(html,new RegExp(name.replace(/[.*+?^${}()|[\]\\]/g,'\\$&'),'i'),name)
  assert.match(html,/225/)
  assert.match(html,/Predefined Case &lt; Control/)
  assert.match(html,/Observation and patient IDs are the same/)
  assert.doesNotMatch(html,/<img src=x>/)
  assert.doesNotMatch(html,/\bcluster.*p_value\b/i)
  const without=biologicalNextStepsHtml({report:null,coverage:null})
  assert.match(without,/Not run/)
  assert.doesNotMatch(without,/NaN|undefined|>null</)
})
test('TSV escapes spreadsheet formulas from untrusted sample-provided feature IDs',()=>{
  const output=biologicalCandidatesTsv(candidates)
  assert.match(output,/'=1\+1/)
  assert.match(output,/V-J pairing <img src=x>/)
  assert.equal(output.split('\n').length,candidates.length+2)
})
test('app uses existing workflows without inserting clinical labels into joint science',()=>{
  const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
  const style=readFileSync(new URL('../styles.css',import.meta.url),'utf8')
  for(const token of ['biologicalNextStepsHtml({report,coverage:coverageReport',
    'bindBiologicalFeatureExplorer(document,currentBiologicalNextRows)',
    "if(action==='compare'){", "state.tab='compare';render()",
    "if(action==='export-features')",'exploratoryGeneRankingTsv(',
    'cohort_overlap_with_joint:\'not_verified\'', 'pathway_enrichment:state.pathwayResult?'])
    assert.ok(main.includes(token),token)
  assert.match(style,/bio-next-feature-table th\{position:sticky/)
  assert.match(main,/await runJointStructure\(\)/)
  const module=readFileSync(new URL('../biological-next-steps.js',import.meta.url),'utf8')
  assert.doesNotMatch(module,/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
  assert.equal(typeof bindBiologicalFeatureExplorer,'function')
})
