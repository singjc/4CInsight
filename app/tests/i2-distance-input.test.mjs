import test from 'node:test'
import assert from 'node:assert/strict'
import { extractFullDistanceViews, prepareJointDistanceInput, jointDistancesTsv } from '../integrative-distance-input.js'

const pairs = (names, factor = 1) => {
  const rows = []
  for (let i = 0; i < names.length; i++) for (let j = i+1; j < names.length; j++) {
    rows.push({sample_a:names[i], sample_b:names[j], distance:(j-i)*factor})
  }
  return rows
}
const named = (key, names, factor) => ({[key]:{modality:{distance_label:`unsupervised ${key} distance`,pairwise_distances:pairs(names, factor)}}})
const alignment = () => ({
  schema:'4c-insight-observation-alignment-v1',
  rows:['S1','S2','S3','S4','S5'].map((observation, index) => ({
    observation, excluded: index === 4,
    modalities:Object.fromEntries(['gene','transcript','bcr','tcrAlpha','tcrBeta','isotype','immune'].map(key => [key,{
      status:index === 3 && key === 'bcr' ? 'missing' : 'present',
      alias:key === 'bcr' ? `${observation}_bcr` : observation,
    }]))
  }))
})

test('I2A accepts full native Longitudinal distance results and expression W4', () => {
  const native = { modalities: [
    {modality:'BCR',distance_label:'Morisita-Horn',pairwise_distances:pairs(['S1_bcr','S2_bcr','S3_bcr','S4_bcr'],0.1)},
    {modality:'TCR alpha',distance_label:'Morisita-Horn',pairwise_distances:pairs(['S1','S2','S3','S4'],0.2)},
    {modality:'TCR beta',distance_label:'Jaccard',pairwise_distances:pairs(['S1','S2','S3','S4'],0.2)},
    {modality:'BCR isotype',distance_label:'CLR Aitchison',pairwise_distances:pairs(['S1','S2','S3','S4'],0.4)},
    {modality:'Immune infiltration',distance_label:'standardized ssGSEA',pairwise_distances:pairs(['S1','S2','S3','S4'],1)},
  ] }
  const nativeViews = extractFullDistanceViews(native)
  assert.deepEqual(nativeViews.map(v=>v.modality), ['bcr','tcrAlpha','tcrBeta','isotype','immune'])
  const browser = extractFullDistanceViews({...named('gene',['S1','S2','S3','S4'],2),...named('transcript',['S1','S2','S3','S4'],3)})
  assert.deepEqual(browser.map(v=>v.modality), ['gene','transcript'])
  const combined = prepareJointDistanceInput(alignment(), [...nativeViews,...browser])
  assert.equal(combined.input.views.length,7)
  assert.equal(combined.input.observations.length,4)
  assert.ok(combined.diagnostics.find(v=>v.modality==='bcr').excludedOrUnmatchedPairs>0)
  assert.ok(combined.input.views.every(view=>view.pairs.every(p=>p.a !== 'S5' && p.b !== 'S5')))
  assert.ok(combined.input.views.find(v=>v.modality==='bcr').pairs.every(p=>!p.a.endsWith('_bcr')))
})

test('I2A does not accept corrected q-values or two-coordinate PCoA results', () => {
  assert.throws(()=>extractFullDistanceViews({ gene:{analysis:{tests:[{q_value:0.01}]}} }),/full pairwise distance/)
  assert.throws(()=>extractFullDistanceViews({ gene:{modality:{distance_label:'PCoA', coordinates:[1,2]}} }),/full pairwise distance/)
  assert.throws(()=>extractFullDistanceViews({modalities:[{modality:'BCR',distance_label:'',pairwise_distances:[]}]}),/distance method/)
})

test('I2A source mapping requires strict exact IDs, no fuzzy matches', () => {
  const source = extractFullDistanceViews(named('gene',['S1','S2','S3','S4'],2))
  const other = extractFullDistanceViews(named('transcript',['s1','s2','s3','s4'],3))
  assert.throws(()=>prepareJointDistanceInput(alignment(), [...source,...other]),/At least two aligned views/)
})

test('I2A rejects ambiguous source ownership, invalid and repeated source pairs', () => {
  const a=alignment(); a.rows[1].modalities.gene.alias='S1'
  assert.throws(()=>prepareJointDistanceInput(a,[...extractFullDistanceViews(named('gene',['S1','S2','S3','S4'],2)),...extractFullDistanceViews(named('transcript',['S1','S2','S3','S4'],2))]),/Ambiguous gene source/)
  assert.throws(()=>extractFullDistanceViews({ views:[{modality:'gene',distance_label:'x',pairs:[{a:'S1',b:'S2',distance:-1}]}] }),/invalid distance/)
  const a2=extractFullDistanceViews(named('gene',['S1','S2','S3','S4'],2));
  a2[0].pairs.push({...a2[0].pairs[0]})
  const b2=extractFullDistanceViews(named('transcript',['S1','S2','S3','S4'],2));
  assert.throws(()=>prepareJointDistanceInput(alignment(), [...a2,...b2]),/Duplicate gene source pair/)
})

test('I2A does not impute missing view or source and retains exclusion in ledger', () => {
  const a=alignment()
  const source=extractFullDistanceViews(named('gene',['S1','S2','S3','S4','S5'],1))
  const second=extractFullDistanceViews(named('bcr',['S1_bcr','S2_bcr','S3_bcr','S4_bcr','S5_bcr'],1))
  const prepared=prepareJointDistanceInput(a,[...source,...second])
  const bcr=prepared.input.views.find(v=>v.modality==='bcr')
  assert.equal(bcr.pairs.length,3)
  assert.equal(prepared.diagnostics.find(v=>v.modality==='bcr').excludedOrUnmatchedPairs,7)
  assert.ok(prepared.input.views.every(v=>v.pairs.every(p=>!['S5'].includes(p.a)&&!['S5'].includes(p.b))))
})

test('I2A rejects a view that passes min view coverage but its numerical support is not valid', () => {
  const a=alignment()
  const v=[...extractFullDistanceViews(named('gene',['S1','S2','S3','S4'],1)),...extractFullDistanceViews(named('transcript',['S1','S2','S3','S4'],2))]
  assert.throws(()=>prepareJointDistanceInput(a,v,{minViews:3}),/Minimum view coverage/)
  assert.throws(()=>prepareJointDistanceInput(a,[...v,v[0]]),/duplicate distance view/)
})

test('I2A pair export includes all rows and guards spreadsheet formulas', () => {
  const tsv=jointDistancesTsv({pairs:[{a:'=HYPERLINK()',b:'B',distance:1.2,contributing_views:2,modalities:['gene','bcr']},{a:'C',b:'D',distance:0.8,contributing_views:3,modalities:['bcr','isotype','immune']}]})
  assert.equal(tsv.trim().split('\n').length,3)
  assert.match(tsv,/'=HYPERLINK\(\)/)
  assert.match(tsv,/bcr\|isotype\|immune/)
})

test('I2A handles 2983 observations without a hard row limit or automatic imputation', () => {
  const rows=Array.from({length:2983},(_,i)=>({observation:`S${i}`,excluded:false,modalities:{gene:{status:'present',alias:`S${i}`},bcr:{status:'present',alias:`S${i}`}}}))
  const a={schema:'4c-insight-observation-alignment-v1',rows}
  const p=[{a:'S0',b:'S1',distance:1},{a:'S1',b:'S2',distance:2}]
  const prep=prepareJointDistanceInput(a,[{modality:'gene',distance_label:'feature distance',pairs:p},{modality:'bcr',distance_label:'Morisita',pairs:p}])
  assert.equal(prep.input.observations.length,2983)
  assert.equal(prep.input.views.length,2)
  assert.equal(prep.input.views[0].pairs.length,2)
})
