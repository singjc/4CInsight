import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8')
test('feature extraction reuses already parsed native I2B data and leaves source distance kernel intact',()=>{
 const wasm=read('../../wasm/src/lib.rs'),corrected=read('../../src/corrected.rs')
 assert.match(wasm,/feature_samples: Vec<serde_json::Value>/)
 assert.match(wasm,/last_sample_feature_values/)
 assert.match(corrected,/pub\(crate\) fn last_sample_feature_values/)
 assert.match(wasm,/unsupervised_isotype_distance_from_maps/)
 assert.match(wasm,/unsupervised_immune_distance_from_maps/)
 assert.match(wasm,/\.finish_unsupervised\(\)/)
})
test('source feature and inspection UI retains local-only/no-inference safeguards',()=>{
 const main=read('../main.js'),module=read('../source-biological-interpretation.js')
 assert.match(main,/function sourceBiologicalPanel\(/)
 assert.match(main,/\$\{sourceBiologicalPanel\(\)\}/)
 assert.match(main,/bindSourceBiologicalEvents\(\)/)
 assert.match(main,/Inspect an individual observation/)
 assert.doesNotMatch(module,/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
 assert.doesNotMatch(module,/\b(?:beta_group|q_value|p_value|caseValues|controlValues)\b/)
})
