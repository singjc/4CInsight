import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'

const file = path => new URL(path,import.meta.url)
const source = async name => readFile(file(name),'utf8')

test('I2A native and WASM share the same Rust fusion kernel, not group-selected features', async () => {
  const [root,wasm,kernel,worker] = await Promise.all([
    source('../../src/lib.rs'),source('../../wasm/src/lib.rs'),source('../../src/integrative_distance.rs'),source('../analysis-worker.js')
  ])
  assert.match(root,/pub mod integrative_distance;/)
  assert.match(wasm,/mod integrative_distance;/)
  assert.match(wasm,/pub fn fuse_integrative_distances_json\(/)
  assert.match(wasm,/integrative_distance::fuse_distances_json\(input_json\)/)
  assert.match(worker,/wasm\.fuse_integrative_distances_json\(JSON\.stringify\(request\.input\)\)/)
  assert.match(kernel,/median_positive/)
  assert.match(kernel,/contributions\.len\(\) < request\.min_views/)
  assert.doesNotMatch(kernel.split('#[cfg(test)]')[0],/beta_group|q_value|caseValues|controlValues|group_case/)
})

test('I2A UI consumes only W4 full distances and retains I1 alignment', async () => {
  const [main,bridge,index]=await Promise.all([source('../main.js'),source('../integrative-distance-input.js'),source('../index.html')])
  assert.match(main,/i2DistancePanel\(\)/)
  assert.match(main,/prepareJointDistanceInput\(state\.alignmentPreview/)
  assert.match(main,/workerRequest\('run-integrative-distance'/)
  assert.match(main,/Joint sample similarity/)
  assert.match(bridge,/pairwise_distances/)
  assert.match(bridge,/status !== 'present'/)
  assert.match(bridge,/if \(!a \|\| !b\) \{ unmatched\+\+; continue \}/)
  assert.doesNotMatch(bridge,/q_value|beta_group|pca\.coordinates/)
  assert.match(main,/sample alignment/)
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version,'0.22.5')
  assert.match(index,new RegExp(`main.js\\?v=${version.replaceAll('.','\\.')}`))
})

const requireWasm=process.env.CFS_REQUIRE_WASM_ARTIFACTS==='1'
const js=file('../wasm/four_c_insight_wasm.js')
const bg=file('../wasm/four_c_insight_wasm_bg.wasm')
async function exists(url) {try{await access(url);return true}catch(e){if(e.code==='ENOENT')return false;throw e}}
const [hasJs,hasBg]=await Promise.all([exists(js),exists(bg)])

test('compiled Rust/WASM I2A fusion matches independent hand-calculated distances', {
  skip: !requireWasm ? 'Requires a fresh wasm-pack build and CFS_REQUIRE_WASM_ARTIFACTS=1' : false,
},async()=>{
  assert.ok(hasJs&&hasBg,'Fresh generated WASM artifacts missing; build wasm-pack first')
  const wasm=await import(js.href)
  await wasm.default({module_or_path:await readFile(bg)})
  assert.equal(typeof wasm.fuse_integrative_distances_json,'function','The compiled binary must contain the new I2A export')
  const observations=['A','B','C','D']
  const sourcePairs=[['A','B',1],['A','C',2],['A','D',3],['B','C',1],['B','D',2],['C','D',1]]
  const v=(modality, factor)=>({modality,distance_label:modality,pairs:sourcePairs.map(([a,b,distance])=>({a,b,distance:distance*factor}))})
  const result=JSON.parse(wasm.fuse_integrative_distances_json(JSON.stringify({observations,views:[v('gene',2),v('bcr',0.1),v('immune',3)],min_views:2})))
  assert.equal(result.fused_pairs,6)
  assert.equal(result.unavailable_pairs,0)
  assert.equal(result.views.length,3)
  assert.equal(result.nearest_neighbors[0].neighbor,'B')
  assert.ok(Math.abs(result.pairs[0].distance-(2/3))<1e-12)
  assert.ok(result.ablation.every(row=>Math.abs(row.pearson_distance_r-1)<1e-12))
  assert.throws(()=>wasm.fuse_integrative_distances_json(JSON.stringify({observations,views:[v('gene',-2),v('bcr',0.1)]})))
})
