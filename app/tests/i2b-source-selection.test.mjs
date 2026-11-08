import test from 'node:test'
import assert from 'node:assert/strict'
import { prepareI2BRequests } from '../i2b-source-selection.js'
import { I1_MODALITIES } from '../multiomics-alignment.js'

const makeRow = (id, excluded=false) => ({
  observation:id, excluded,
  modalities:Object.fromEntries(I1_MODALITIES.map(m=>[m.key,{status:'present',alias:id}]))
})
const alignment = {schema:'4c-insight-observation-alignment-v1',rows:['S1','S2','S3','S4'].map(id=>makeRow(id))}
const records = I1_MODALITIES.flatMap(mod=> mod.mergedKind
  ? [{id:`merged-${mod.key}`,name:`merged-${mod.key}.tsv`,kind:mod.mergedKind}]
  : alignment.rows.map(r=>({id:`${mod.key}-${r.observation}`,name:`${r.observation}.${mod.kind}.xlsx`,sampleId:r.observation,kind:mod.kind})))
const files = new Map(records.map(r=>[r.id,{name:r.name,arrayBuffer:()=>Promise.resolve(new ArrayBuffer(4))}]))
const chosen={gene:'merged-gene',transcript:'merged-transcript'}

test('I2B selects all seven views from exact I1 sources, no labels or adjusted hits',()=>{
  const {requests,errors}=prepareI2BRequests(alignment,records,id=>files.get(id),chosen)
  assert.equal(errors.length,0)
  assert.equal(requests.length,7)
  assert.deepEqual(requests.map(r=>r.modality),I1_MODALITIES.map(m=>m.key))
  assert.deepEqual(requests[0].sampleIds,['S1','S2','S3','S4'])
  assert.equal(requests[2].files.length,4)
  assert.deepEqual(requests[2].files.map(f=>f.sampleId),['S1','S2','S3','S4'])
  assert.ok(!JSON.stringify(requests).includes('caseValues'))
})

test('I2B does not read excluded rows, invent missing data, or use fuzzy aliases',()=>{
  const input={...alignment,rows:[...alignment.rows.slice(0,3),makeRow('S4',true)]}
  const {requests,errors}=prepareI2BRequests(input,records,id=>files.get(id),chosen,{tcrAlpha:false})
  assert.equal(errors.length,0)
  assert.equal(requests.length,6)
  assert.equal(requests.find(x=>x.modality==='bcr').files.length,3)
  const wrong={...alignment,rows:alignment.rows.map((r,i)=>i===0? {...r,modalities:{...r.modalities,bcr:{status:'present',alias:'s1'}}}:r)}
  const checked=prepareI2BRequests(wrong,records,id=>files.get(id),chosen)
  assert.equal(checked.requests.some(x=>x.modality==='bcr'),false)
  assert.match(checked.errors.find(x=>x.modality==='bcr').message,/expected one source file/)
})

test('I2B rejects ambiguous and disconnected sources without blocking other modalities',()=>{
  const extra={...records.find(r=>r.kind==='tcr-beta'),id:'duplicate-tcr'}
  const bad=prepareI2BRequests(alignment,[...records,extra],id=>files.get(id),chosen)
  assert.ok(bad.errors.find(e=>e.modality==='tcrBeta'))
  assert.equal(bad.requests.length,6)
  const noFile=prepareI2BRequests(alignment,records,id=>id==='bcr-S1'?null:files.get(id),chosen)
  assert.ok(noFile.errors.find(e=>e.modality==='bcr'))
  assert.equal(noFile.requests.length,6)
})

test('I2B rejects invalid and duplicate I1 observation or source aliases',()=>{
  assert.throws(()=>prepareI2BRequests({},records,id=>files.get(id),chosen),/Run the I1 sample alignment/)
  const dup={...alignment,rows:[...alignment.rows,{...makeRow('ANOTHER'),modalities:{...makeRow('ANOTHER').modalities,bcr:{status:'present',alias:'S1'}}}]}
  const output=prepareI2BRequests(dup,records,id=>files.get(id),chosen)
  assert.ok(output.errors.some(e=>e.modality==='bcr' && /Duplicate/.test(e.message)))
})
