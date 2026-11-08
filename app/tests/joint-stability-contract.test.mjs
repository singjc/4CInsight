import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8')
test('patient-level stability is a stoppable on-device worker without outcome labels',()=>{
  const main=read('../main.js'),worker=read('../analysis-worker.js'),engine=read('../joint-stability.js')
  assert.match(main,/function jointStabilityPanel\(/)
  assert.match(main,/function runJointStability\(/)
  assert.match(main,/workerRequest\('run-joint-stability'/)
  assert.match(main,/bindJointStabilityEvents\(\)/)
  assert.match(main,/jointStabilityPanel\(\)/)
  assert.match(worker,/runPatientStabilityAsync/)
  assert.match(worker,/run-joint-stability/)
  assert.doesNotMatch(engine,/fetch\(|XMLHttpRequest|WebSocket|\/api\//)
  assert.doesNotMatch(engine,/beta_group|caseValues|q_value|controlValues/)
})
test('native parity export is compiled for Rust and WASM with strict test',()=>{
  const root=read('../../src/lib.rs'),wasm=read('../../wasm/src/lib.rs'),ci=read('../../.github/workflows/ci.yml')
  assert.match(root,/pub mod joint_structure_reference;/)
  assert.match(wasm,/mod joint_structure_reference;/)
  assert.match(wasm,/pub fn reference_joint_structure_json\(/)
  assert.match(ci,/joint-native-parity\.test\.mjs/)
})
