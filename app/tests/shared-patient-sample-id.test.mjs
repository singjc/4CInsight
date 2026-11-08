import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { alignObservations, alignmentTsv } from '../multiomics-alignment.js'

const record = id => ({kind:'bcr', sampleId:id, name:`${id}_bcr.xlsx`, relativePath:`bcr/${id}_bcr.xlsx`})
const identity = (values, options={}) => alignObservations({
  table:{columns:['4C ID','visit'],rows:values.map(([id,visit])=>[id,visit])},
  observationColumn:'4C ID',subjectColumn:'4C ID',timeColumn:'visit',
  records:values.map(([id])=>record(id)),merged:{},...options,
})

test('single-observation metadata can use one column as observation and patient ID',()=>{
  const result=identity([['4C_001','baseline']])
  assert.equal(result.summary.total,1)
  assert.equal(result.rows[0].observation,'4C_001')
  assert.equal(result.rows[0].subject,'4C_001')
  assert.equal(result.rows[0].timepoint,'baseline')
  assert.equal(result.rows[0].modalities.bcr.status,'present')
  assert.deepEqual(result.roles,{observationColumn:'4C ID',subjectColumn:'4C ID',timeColumn:'visit'})
  assert.match(alignmentTsv(result),/4C_001\t4C_001\tbaseline/)
})

test('unique observations may also identify unique patients in a multi-sample cohort',()=>{
  const r=identity([['P1','v1'],['P2','v1'],['P3','v2']])
  assert.deepEqual(r.rows.map(x=>x.subject),['P1','P2','P3'])
  assert.equal(r.summary.counts.find(x=>x.key==='bcr').present,3)
  assert.equal(r.warnings.some(x=>x.includes('replicates')),false)
})

test('separate observation and shared patient columns retain repeated visits',()=>{
  const r=alignObservations({
    table:{columns:['sample_id','patient_id','visit'],rows:[['A1','P1','T0'],['A2','P1','T1'],['B1','P2','T0']]},
    observationColumn:'sample_id',subjectColumn:'patient_id',timeColumn:'visit',
    records:[record('A1'),record('A2'),record('B1')],
  })
  assert.deepEqual(r.rows.map(x=>x.subject),['P1','P1','P2'])
  assert.deepEqual(r.rows.map(x=>x.observation),['A1','A2','B1'])
})

test('duplicate observation IDs are still rejected, even when patient and observation columns coincide',()=>{
  assert.throws(()=>identity([['P1','v1'],['P1','v2']]),/Duplicate observation ID/)
})

test('a selected timepoint cannot reuse the identity column',()=>{
  assert.throws(()=>identity([['S1','v1']],{timeColumn:'4C ID'}),/Timepoint must use a separate metadata column/)
})

test('missing or invalid patient and observation columns are not guessed',()=>{
  assert.throws(()=>identity([['S1','v1']],{subjectColumn:'unknown'}),/not found/)
  assert.throws(()=>identity([['S1','v1']],{observationColumn:'unknown'}),/not found/)
  assert.throws(()=>identity([['','v1']]),/empty observation ID/)
})

test('metadata identity roles must be independently selectable throughout the app',()=>{
  const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
  const rust=readFileSync(new URL('../../src/longitudinal.rs',import.meta.url),'utf8')
  const portable=readFileSync(new URL('../../src/portable_longitudinal.rs',import.meta.url),'utf8')
  assert.doesNotMatch(main,/columns\.filter\(\(column\) => column !== cfg\.observationIdColumn\), cfg\.patientIdColumn/)
  assert.doesNotMatch(main,/cfg\.observationIdColumn !== cfg\.patientIdColumn/)
  assert.doesNotMatch(main,/if \(cfg\.patientIdColumn === cfg\.observationIdColumn\) cfg\.patientIdColumn = ''/)
  assert.doesNotMatch(main,/columns\.filter\(\(column\) => column !== state\.qcConfig\.metadataIdColumn\), cfg\.subjectColumn/)
  assert.doesNotMatch(rust,/observation_idx == patient_idx/)
  assert.doesNotMatch(portable,/observation_column == patient_column/)
  assert.match(main,/One column may identify both the observation and patient/)
  assert.match(rust,/one observation per patient/i)
})
