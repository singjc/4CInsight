import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {buildIntegrativeCoverage,integrativeCoverageHtml,integrativeOmittedTsv,bindIntegrativeCoverage}
  from '../integrative-coverage.js'
import {integrativeClusterCautionHtml,integrativeScatterMarkup} from '../integrative-ux.js'

const modalities=['gene','bcr','immune']
const aligned=(observation,patient,missing=[])=>({observation,subject:patient,timepoint:'T1',excluded:false,
  modalities:Object.fromEntries(modalities.map(m=>[m,{status:missing.includes(m)?'missing':'present',alias:observation}]))})
const alignment={schema:'4c-insight-observation-alignment-v1',rows:[
  aligned('A','p1'),aligned('B','p2'),aligned('C','p3'),aligned('D','p4',['immune']),aligned('E','p3')]}
const structure={schema:'4c-insight-joint-structure-v1',fixed_modalities:modalities,
  observations:['A','B','C'],coverage:{metadata_observations:5,removed_observation_ids:['D'],
  patient_deduplicated_ids:['E'],selection:'deterministic greedy'}}
const pair=(a,b)=>({a,b,distance:.5,contributing_views:3,modalities})
const distances={observations:['A','B','C','D','E'],views:modalities.map(modality=>({modality})),pairs:[
  pair('A','B'),pair('A','C'),pair('B','C'),{...pair('A','D'),contributing_views:2,modalities:['gene','bcr']} ]}

test('omitted observations have exact mutually exclusive saved selection reasons',()=>{
  const report=buildIntegrativeCoverage(alignment,structure,distances)
  assert.equal(report.eligible,5)
  assert.equal(report.retained,3)
  assert.equal(report.excluded,2)
  assert.deepEqual(report.byReason,{'retained':3,'patient-deduplicated':1,'missing-fixed-pairs':1,unclassified:0})
  assert.equal(report.omitted.find(r=>r.observation==='D').reason,'missing-fixed-pairs')
  assert.equal(report.omitted.find(r=>r.observation==='E').reason,'patient-deduplicated')
  assert.deepEqual(report.omitted.find(r=>r.observation==='D').sourceAbsent,['Immune infiltration (missing)'])
  assert.equal(report.omitted.find(r=>r.observation==='D').fullPairNeighbors,0)
  assert.equal(report.modalities.find(r=>r.key==='immune').eligiblePresent,4)
  assert.equal(report.modalities.find(r=>r.key==='immune').retainedPresent,3)
  const html=integrativeCoverageHtml(report)
  assert.match(html,/2 of 5 eligible observations \(40\.0%\)/)
  assert.match(html,/Source coverage means an exact local source alias/)
  assert.match(html,/does not prove that a pairwise distance was calculated/)
  assert.match(html,/data-coverage-sort="6"/)
  assert.match(html,/integrative-coverage-search/)
  assert.match(html,/integrative-coverage-export/)
  assert.match(integrativeOmittedTsv(report),/complete_panel_pair_partners/)
})

test('malformed and mismatched saved state does not claim pair counts or infer coverage',()=>{
  assert.equal(buildIntegrativeCoverage({...alignment,rows:[...alignment.rows,aligned('A','p6')]},structure,distances),null)
  assert.equal(buildIntegrativeCoverage(alignment,{...structure,coverage:{removed_observation_ids:['B'],patient_deduplicated_ids:['E']}},distances),null)
  const partial=buildIntegrativeCoverage(alignment,structure,{...distances,views:[{modality:'gene'}]})
  assert.equal(partial.haveFusedPairs,false)
  assert.equal(partial.omitted[0].fullPairNeighbors,null)
  assert.match(integrativeCoverageHtml(partial),/counts are unavailable/)
  assert.match(integrativeCoverageHtml(null),/unavailable/)
  assert.equal(typeof bindIntegrativeCoverage,'function')
})

test('omitted TSV neutralizes formula injection; HTML escapes local IDs and labels',()=>{
  const malicious=aligned('=HYPERLINK("http://bad")','<script>alert(1)</script>',['gene'])
  const a={...alignment,rows:[...alignment.rows,malicious]}
  const s={...structure,coverage:{removed_observation_ids:['D',malicious.observation],patient_deduplicated_ids:['E']}}
  const result=buildIntegrativeCoverage(a,s,null)
  assert.match(integrativeOmittedTsv(result),/"'=HYPERLINK/m)
  const html=integrativeCoverageHtml(result)
  assert.doesNotMatch(html,/<script>/)
  assert.match(html,/&lt;script&gt;/)
  assert.doesNotMatch(html,/<img /)
})

test('singleton warning is actionable, non-clinical and contains safe sample IDs',()=>{
  const report={membership:[{cluster:1,observations:4},{cluster:2,observations:1}],
    points:[{observation:'<small>',cluster:2,x:12,y:3},{observation:'A',cluster:1,x:1,y:2}]}
  const html=integrativeClusterCautionHtml(report)
  assert.match(html,/cluster 2 \(1 observation\)/)
  assert.match(html,/cannot establish a biological subtype/)
  assert.match(html,/data-integrative-inspect-sample="&lt;small&gt;"/)
  assert.doesNotMatch(html,/<small>/)
  assert.equal(integrativeClusterCautionHtml({membership:[{cluster:1,observations:10}]}),'')
  const plot=integrativeScatterMarkup(report,[.3,.1])
  assert.match(plot,/role="button" aria-label="Inspect observation &lt;small&gt;"/)
  assert.match(plot,/data-scatter-metadata="true"/)
})

test('integrative Overview wiring keeps settings warning once and science methods intact',()=>{
  const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
  const css=readFileSync(new URL('../styles.css',import.meta.url),'utf8')
  for(const item of ['integrativeCoverageHtml(coverageReport)','integrativeClusterCautionHtml(report','currentIntegrativeCoverage()','showIntegrativeSampleMetadata','bindIntegrativeCoverage(document)','integrativeOmittedTsv(report)',"saveTextFile('4c-insight-joint-cohort-omissions.tsv'"])
    assert.match(main,new RegExp(item.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')))
  assert.equal(main.match(/Observation ID and Patient ID use the same column/g)?.length,1)
  assert.match(css,/integrative-coverage-table th\{position:sticky/)
  assert.match(main,/await runJointStructure\(\)/)
  assert.match(main,/await runJointStability\(\)/)
  assert.doesNotMatch(readFileSync(new URL('../integrative-coverage.js',import.meta.url),'utf8'),/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
})
