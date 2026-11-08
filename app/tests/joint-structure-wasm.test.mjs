import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'
import { analyzeJointStructure } from '../joint-structure.js'
const required=process.env.CFS_REQUIRE_WASM_ARTIFACTS==='1'
const js=new URL('../wasm/four_c_insight_wasm.js',import.meta.url)
const bg=new URL('../wasm/four_c_insight_wasm_bg.wasm',import.meta.url)
test('compiled shared Rust fusion feeds fixed-panel joint ordination and clustering',{
  skip:!required?'Needs a fresh wasm-pack build and CFS_REQUIRE_WASM_ARTIFACTS=1':false,
},async()=>{
  await access(js);await access(bg)
  const wasm=await import(js.href)
  await wasm.default({module_or_path:await readFile(bg)})
  assert.equal(typeof wasm.fuse_integrative_distances_json,'function')
  const ids=['A','B','C','D'],coords={A:[0,0],B:[1,0],C:[0,1],D:[1,1]}
  const pairs=ids.flatMap((a,i)=>ids.slice(i+1).map(b=>({a,b,distance:Math.hypot(coords[a][0]-coords[b][0],coords[a][1]-coords[b][1])})))
  const views=['gene','bcr'].map(modality=>({modality,distance_label:'Independent full synthetic distances',pairs}))
  const fused=JSON.parse(wasm.fuse_integrative_distances_json(JSON.stringify({observations:ids,views,min_views:2})))
  const structure=analyzeJointStructure(fused,{selectedModalities:['gene','bcr']})
  assert.equal(structure.complete_pairs,6)
  assert.equal(structure.pcoa.negative_axes,0)
  assert.equal(structure.clustering.merges.length,3)
  assert.deepEqual(structure.observations,ids)
  assert.ok(Math.abs(structure.distances[1]-1)<1e-9)
})
