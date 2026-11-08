import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const read = path => readFileSync(new URL(path,import.meta.url),'utf8')

test('I2B shared native and WASM kernels reuse scientific W4 distance definitions',()=>{
  const [rust,wasm,worker]=[read('../../src/corrected.rs'),read('../../wasm/src/lib.rs'),read('../analysis-worker.js')]
  assert.match(rust,/fn unsupervised_repertoire_from_parsed\(/)
  assert.match(rust,/repertoire_distance_matrices\(&parsed, exact_clonotypes\)/)
  assert.match(rust,/unsupervised_isotype_distance_from_maps\(maps, spec, warnings\)/)
  assert.match(rust,/unsupervised_immune_distance_from_maps\(maps, warnings\)/)
  assert.match(rust,/finish_unsupervised\(self\)/)
  assert.match(wasm,/pub fn i2b_expression_distance_json\(/)
  assert.match(wasm,/pub fn i2b_begin_json\(/)
  assert.match(wasm,/pub fn i2b_add_sample_json\(/)
  assert.match(wasm,/pub fn i2b_finish_json\(/)
  assert.match(wasm,/pub fn i2b_abort\(/)
  assert.match(wasm,/run_unsupervised_expression_distance_bytes\(/)
  assert.match(worker,/request\.type === 'run-i2b-source-distance'/)
  assert.match(worker,/wasm\.i2b_finish_json\(\)/)
  assert.doesNotMatch(wasm.slice(wasm.indexOf('// I2B: a separate unsupervised'),wasm.indexOf('fn js_error(')),/is_case_id|is_control_id|beta_group|q_value/)
})

test('I2B UI starts local isolated workers and preserves view errors',()=>{
  const [main,select]=[read('../main.js'),read('../i2b-source-selection.js')]
  assert.match(main,/function i2bRunDirect\(/)
  assert.match(main,/state\.i2bDirectViews\.push\(\{/)
  assert.match(main,/isolatedScienceRequest\(\{/)
  assert.match(main,/activeI2bController\.abort\(\)/)
  assert.match(main,/prepareI2BRequests\(state\.alignmentPreview/)
  assert.match(main,/id="i2b-run-direct"/)
  assert.match(main,/for \(const view of state\.i2bDirectViews\) merged\.set/)
  assert.doesNotMatch(select,/fetch\(|XMLHttpRequest|localStorage|\/api\/|q_value|beta_group/)
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version,'0.22.3')
  assert.ok(read('../index.html').includes(`main.js?v=${version}`))
})
