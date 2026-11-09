import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import vm from 'node:vm'
const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
const html=readFileSync(new URL('../index.html',import.meta.url),'utf8')

test('Overview exposes the primary IDs and science settings without extra buttons',()=>{
  for(const name of ['integrative-setting-source','integrative-setting-metadata','integrative-setting-observation',
    'integrative-setting-patient','integrative-setting-visit','integrative-setting-workers',
    'integrative-setting-min-views','integrative-setting-clusters','integrative-setting-repeats',
    'data-integrative-main-modality','integrative-setting-one-patient'])
    assert.ok(main.includes(name),`${name} is not visible in Overview`)
  assert.match(main,/function integrativeMainSettings\(/)
  assert.match(main,/\$\{integrativeMainSettings\(\)\}/)
  assert.match(main,/function bindIntegrativePrimarySettings\(/)
  assert.match(main,/bindIntegrativePrimarySettings\(\)/)
  assert.match(main,/Single metadata column can identify both/i, 'The same ID column must be allowed')
})

test('All workflow errors are shown in Details with the failed stage and source warnings',()=>{
  assert.match(main,/function integrativeRunDiagnostics\(/)
  assert.match(main,/\$\{integrativeRunDiagnostics\(\)\}/)
  for(const field of ['integrativeRunError','alignmentError','i2bErrors','i2Error','i2StructureError',
    'jointStabilityError','bioError','bioFeatureError','integrativeRunSteps'])
    assert.ok(main.includes(field),`The ${field} diagnostic is missing`)
  assert.match(main,/integrative-review-error/)
  assert.match(main,/state\.integrativeView='details'/)
})

test('Alignment snapshots are recovered only for unchanged inputs and exact validated schema',()=>{
  const first=main.indexOf('function integrativeInputFingerprint() {')
  const end=main.indexOf('function ensureBiologicalFindings() {',first)
  assert.ok(first>0 && end>first, 'Unable to isolate the guarded-alignment helper')
  const ctx={state:{records:[{id:'r1',name:'sample.tsv',kind:'metadata-candidate'}],
    files:new Map([['r1',{name:'sample.tsv',size:123,lastModified:42}]]),
    qcConfig:{metadataId:'r1'},compareConfig:{geneMergedId:'g',transcriptMergedId:'t'},
    alignmentSource:'active',alignmentRoles:{observation:'ID',subject:'ID',time:''},
    hitConfig:{integrativeModalities:{bcr:true}},alignmentPreview:null,alignmentTable:null},
    i1ExclusionRules:()=>[]}
  vm.createContext(ctx)
  vm.runInContext(main.slice(first,end),ctx)
  const fingerprint=vm.runInContext('integrativeInputFingerprint()',ctx)
  const data={schema:'4c-insight-observation-alignment-v1',summary:{eligible:3}}
  const pinned={preview:data,table:{columns:['ID']},fingerprint}
  const restored=vm.runInContext('integrativeRestoreCheckedAlignment',ctx)(pinned)
  assert.equal(restored.recovered,true)
  assert.equal(ctx.state.alignmentPreview,data)
  assert.equal(ctx.state.alignmentTable,pinned.table)
  assert.equal(vm.runInContext('integrativeRestoreCheckedAlignment',ctx)(pinned).recovered,false)
  ctx.state.alignmentPreview=null;ctx.state.alignmentTable=null
  ctx.state.files.set('r1',{name:'other.tsv',size:100,lastModified:42})
  assert.throws(()=>vm.runInContext('integrativeRestoreCheckedAlignment',ctx)(pinned),/files or alignment settings changed/i)
  assert.equal(ctx.state.alignmentPreview,null, 'Never restore a stale alignment')
})

test('Wrong alignment schema is rejected and cannot bypass scientific validation',()=>{
  const first=main.indexOf('function integrativeInputFingerprint() {')
  const end=main.indexOf('function ensureBiologicalFindings() {',first)
  const ctx={state:{records:[],files:new Map(),qcConfig:{metadataId:''},
    compareConfig:{geneMergedId:'',transcriptMergedId:''},alignmentSource:'active',alignmentRoles:{},
    hitConfig:{integrativeModalities:{}},alignmentPreview:null,alignmentTable:null},i1ExclusionRules:()=>[]}
  vm.createContext(ctx);vm.runInContext(main.slice(first,end),ctx)
  const pinned={preview:{schema:'unknown'},table:{columns:[]},fingerprint:vm.runInContext('integrativeInputFingerprint()',ctx)}
  assert.throws(()=>vm.runInContext('integrativeRestoreCheckedAlignment',ctx)(pinned),/unexpected schema/i)
  assert.equal(ctx.state.alignmentPreview,null)
})

test('One-click keeps the validated scientific steps and a matching cache version',()=>{
  for(const fn of ['i1RunAlignment','i2bRunDirect','i2RunFusion','runJointStructure','runJointStability','runBiologicalFeatureRanking'])
    assert.ok(main.includes(fn+'()'),`${fn} must remain in the complete workflow`)
  assert.match(main,/minViews:views\.length/,'Fixed-view structure must retain its strict complete coverage')
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version,'0.22.8')
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.doesNotMatch(main,/integrativeMainSettings\(\)[\s\S]*?<span class="tag neutral">I[012][ABC]/)
})
