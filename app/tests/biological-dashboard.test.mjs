import test from 'node:test'
import assert from 'node:assert/strict'
import {allModalityRows,modalityOverviewHtml,combinedFeatureRows,topFeatureRows,
  featureLollipopHtml,featuredProfiles,chosenProfile,combinedFeatureTsv} from '../biological-dashboard.js'
const report={fixed_modalities:['gene','bcr','immune'],modality:[
  {modality:'gene',pairs:3,expected_pairs:6,between_within_ratio:1.5,joint_distance_correlation:0.85,distance_label:'log TPM Euclidean'},
  {modality:'bcr',pairs:6,expected_pairs:6,between_within_ratio:0.75,joint_distance_correlation:-0.25,distance_label:'Morisita–Horn'},
  {modality:'immune',pairs:0,expected_pairs:6,between_within_ratio:null,joint_distance_correlation:null,distance_label:'signature Euclidean'},
]}
const feature=(feature,score)=>({feature,secondary_id:feature+'-ID',n_used:10,pcoa1_spearman:score,pcoa2_spearman:-score,
  cluster1_log2_tpm_difference:0.3})
const combined={byModality:{
  gene:{rows:[feature('A',0.91),feature('=HYPERLINK(',0.82),feature('X',null)],profiles:[{feature:'A',secondary_id:'A-ID',values:[{observation:'O1',log2_tpm:1.2}]}],features_ranked:3},
  transcript:{rows:[feature('T',-0.88)],profiles:[{feature:'T',secondary_id:'T-ID',values:[{observation:'O1',log2_tpm:2.1}]}],features_ranked:1},
}}
test('all seven data types are always represented, with missing views clearly unavailable',()=>{
 const rows=allModalityRows(report)
 assert.equal(rows.length,7)
 assert.deepEqual(rows.map(r=>r.key),['gene','transcript','bcr','tcrAlpha','tcrBeta','isotype','immune'])
 assert.equal(rows.find(r=>r.key==='gene').pair_coverage,0.5)
 assert.equal(rows.find(r=>r.key==='transcript').included,false)
 assert.equal(rows.find(r=>r.key==='immune').joint_distance_correlation,null)
 const html=modalityOverviewHtml(report)
 for(const row of rows)assert.ok(html.includes(row.label))
 assert.match(html,/Not in fitted model/)
 assert.doesNotMatch(html,/>NaN<|>Infinity<|width:NaN/)
})
test('distance agreement and between-within ratios stay separate and signed',()=>{
 const html=modalityOverviewHtml(report)
 assert.match(html,/Agreement with joint distances/)
 assert.match(html,/Between\/within cluster distance/)
 assert.match(html,/-0\.250/)
 assert.match(html,/1\.500/)
})
test('feature table unifies RNA modalities without erasing data types or limiting rows',()=>{
 const rows=combinedFeatureRows(combined)
 assert.equal(rows.length,4)
 assert.deepEqual(new Set(rows.map(row=>row.modality)),new Set(['gene','transcript']))
 assert.equal(rows.find(r=>r.feature==='T').modality_label,'Transcript expression')
 assert.equal(combinedFeatureRows({byModality:{}}).length,0)
})
test('top ranked chart is visual-only and full export keeps every row',()=>{
 assert.deepEqual(topFeatureRows(combined,'gene','pcoa1_spearman',1).map(r=>r.feature),['A'])
 assert.deepEqual(topFeatureRows(combined,'transcript','pcoa2_spearman',5).map(r=>r.feature),['T'])
 const html=featureLollipopHtml(combined)
 assert.match(html,/data-bio-profile="gene:0"/)
 assert.match(html,/data-bio-profile="transcript:0"/)
 assert.match(html,/Spearman correlation/)
 assert.equal(combinedFeatureTsv(combined).trim().split('\n').length,5)
})
test('top profiles preserve exact IDs and return null for an unknown selection',()=>{
 assert.equal(featuredProfiles(combined).length,2)
 assert.equal(chosenProfile(combined,'gene:0').feature,'A')
 assert.equal(chosenProfile(combined,'immune:0'),null)
})
test('export protects spreadsheet-formula feature names',()=>{
 const tsv=combinedFeatureTsv(combined)
 assert.match(tsv,/'=HYPERLINK\(/)
})
test('HTML escapes untrusted feature labels and measured source labels',()=>{
 const evil={byModality:{gene:{rows:[feature('<img src=x onerror=alert(1)>',0.8)],profiles:[],features_ranked:1}}}
 const html=featureLollipopHtml(evil)
 assert.doesNotMatch(html,/<img/)
 assert.match(html,/&lt;img/)
})
test('unsupported feature axis and invalid chart limits are rejected',()=>{
 assert.throws(()=>topFeatureRows(combined,'immune'),/Unsupported/)
 assert.throws(()=>topFeatureRows(combined,'gene','p_value'),/Unsupported/)
 assert.throws(()=>topFeatureRows(combined,'gene','pcoa1_spearman',0),/Invalid/)
})
