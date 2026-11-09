import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
const source=readFileSync(new URL('../integrative-metadata-columns.js',import.meta.url),'utf8')

test('Overview reads selected local metadata header before alignment',()=>{
  assert.match(main,/function ensureIntegrativeMetadataColumns\(\)/)
  assert.match(main,/readIntegrativeMetadataColumns\(file\)/)
  assert.match(main,/integrativeSettingColumns\(\)/)
  assert.match(main,/if \(state\.tab === 'hits'\) \{ bindSignificantHitEvents\(\); void ensureIntegrativeMetadataColumns\(\) \}/)
  assert.match(main,/Metadata columns:/)
  assert.doesNotMatch(main,/Read on initial alignment/)
})

test('Overview owns the one editable set of identity inputs',()=>{
  for(const id of ['integrative-setting-observation','integrative-setting-patient','integrative-setting-visit'])
    assert.equal(main.match(new RegExp(`id="${id}"`,'g'))?.length,1,id)
  for(const id of ['i1-observation-column','i1-subject-column','i1-time-column','i1-source'])
    assert.doesNotMatch(main,new RegExp(`id="${id}"`),`Obsolete duplicate input: ${id}`)
  assert.match(main,/i1SelectionSummary\(\)/)
  assert.match(main,/Alignment settings from Overview/)
})

test('Same metadata column may identify observation and patient, but timepoint is separate',()=>{
  assert.match(main,/A single metadata column can identify both the observation and the patient/)
  assert.match(source,/subject = found\.has\(roles\.subject\) \? roles\.subject/)
  assert.match(source,/roles\.time !== observation && roles\.time !== subject/)
})

test('Inputs invalidate previous results and stale in-flight metadata discovery cannot commit',()=>{
  assert.match(main,/integrativeMetadataRequest\+\+/)
  assert.match(main,/request!==integrativeMetadataRequest/)
  assert.match(main,/selectedFile\(state\.qcConfig\.metadataId\)!==file/)
  assert.match(main,/i1ResetPreview\(\);invalidateIntegrativeFit\(\)/)
  assert.match(main,/!integrativeSettingColumns\(\)\.length/)
})

test('Metadata column reader stays browser-local and avoids whole-file reads',()=>{
  assert.match(source,/file\.stream\(\)\.getReader\(\)/)
  assert.doesNotMatch(source,/\bfetch\s*\(|XMLHttpRequest|WebSocket|\.arrayBuffer\(|\.text\s*\(/)
})

// Exercise the real state transition code without DOM, network, or source uploads.
import vm from 'node:vm'
import { validIntegrativeIdentityRoles } from '../integrative-metadata-columns.js'

function discoveryHarness(options={}) {
  const start=main.indexOf('let integrativeMetadataFile=null')
  const end=main.indexOf('function integrativeMainSettings() {',start)
  assert.ok(start>=0 && end>start,'Metadata discovery source must be present')
  let selected=options.file||{name:'cohort.tsv'}
  const filesRead=[]
  const context={
    state:{tab:'hits',alignmentSource:'active',alignmentColumns:[],alignmentRoles:{observation:'',subject:'',time:''},
      qcConfig:{metadataId:'metadata1',metadataIdColumn:'4C ID'},metadataEditor:{table:null},integrativeColumnError:'',integrativeColumnBusy:false},
    selectedFile:()=>selected,
    readIntegrativeMetadataColumns: async file=>{filesRead.push(file);return options.reader?options.reader(file):['4C ID','visit','patient_id']},
    validIntegrativeIdentityRoles,
    workerRequest:async (_kind,request)=>({table:{columns:options.xlsxColumns||['sample_id','visit']}}),
    render:()=>{context.renders++},
    renders:0,
  }
  vm.createContext(context)
  vm.runInContext(main.slice(start,end),context)
  return {...{context,filesRead},setFile:file=>{selected=file},ensure:vm.runInContext('ensureIntegrativeMetadataColumns',context)}
}

test('initial Overview visit discovers ID columns and supports same ID for patient',async()=>{
  const {context,ensure,filesRead}=discoveryHarness()
  await ensure()
  assert.deepEqual(Array.from(context.state.alignmentColumns),['4C ID','visit','patient_id'])
  assert.equal(context.state.alignmentRoles.observation,'4C ID')
  assert.equal(context.state.integrativeColumnBusy,false)
  assert.equal(context.renders,1)
  context.state.alignmentRoles.subject='4C ID'
  await ensure()
  assert.equal(filesRead.length,1,'A completed header read must be cached')
  assert.equal(context.state.alignmentRoles.subject,'4C ID')
})

test('a malformed file produces one error and does not trigger repeated reads',async()=>{
  const {context,ensure,filesRead}=discoveryHarness({reader:async()=>{throw new Error('bad header')}})
  await ensure()
  assert.match(context.state.integrativeColumnError,/bad header/)
  assert.equal(context.state.integrativeColumnBusy,false)
  await ensure()
  assert.equal(filesRead.length,1,'Do not loop on invalid metadata')
})

test('header discovery ignores stale responses after a file change',async()=>{
  const promises=new Map()
  const a={name:'old.tsv'},b={name:'new.tsv'}
  const {context,setFile,ensure}=discoveryHarness({file:a,reader:file=>new Promise(resolve=>promises.set(file,resolve))})
  const older=ensure()
  context.state.qcConfig.metadataId='metadata2'
  setFile(b)
  const newer=ensure()
  promises.get(a)(['old_id'])
  await older
  assert.equal(context.state.alignmentColumns.length,0,'Old header must not be committed')
  promises.get(b)(['new_id','visit'])
  await newer
  assert.equal(context.state.alignmentColumns[0],'new_id')
})


test('Excel metadata obtains columns from the existing local WASM metadata parser',async()=>{
  const file={name:'metadata.xlsx'}
  const {context,ensure,filesRead}=discoveryHarness({file,xlsxColumns:['Patient ID','Sample ID','Visit']})
  await ensure()
  assert.equal(context.state.alignmentColumns[0],'Patient ID')
  assert.equal(context.state.alignmentColumns[1],'Sample ID')
  assert.equal(filesRead.length,0,'Do not treat a binary XLSX file as a text stream')
})
