import test from 'node:test'
import assert from 'node:assert/strict'
import { access, readFile } from 'node:fs/promises'
import { analyzeJointStructure } from '../joint-structure.js'
const requireWasm=process.env.CFS_REQUIRE_WASM_ARTIFACTS==='1'
const js=new URL('../wasm/four_c_insight_wasm.js',import.meta.url)
const bg=new URL('../wasm/four_c_insight_wasm_bg.wasm',import.meta.url)
async function exists(url){try{await access(url);return true}catch(e){if(e.code==='ENOENT')return false;throw e}}
const hasArtifacts=await exists(js)&&await exists(bg)

test('independent Rust/WASM PCoA and clustering agree with browser on fixed distances',{
  skip:!requireWasm?'Requires fresh release wasm-pack build and CFS_REQUIRE_WASM_ARTIFACTS=1':false,
},async()=>{
  assert.ok(hasArtifacts,'Build fresh WASM artifacts before claiming native parity')
  const wasm=await import(js.href)
  await wasm.default({module_or_path:await readFile(bg)})
  assert.equal(typeof wasm.reference_joint_structure_json,'function','Missing independent Rust reference export')
  const obs=['A','B','C','D','E','F']
  const xy=[[0,0],[1,0],[0,1],[1,1],[1.8,0.3],[.2,1.7]]
  const n=obs.length,distances=Array.from({length:n*n},(_,idx)=>{
    const i=Math.floor(idx/n),j=idx%n
    return Math.hypot(xy[i][0]-xy[j][0],xy[i][1]-xy[j][1])
  })
  const pairs=[]
  for(let i=0;i<n;i++)for(let j=i+1;j<n;j++)pairs.push({a:obs[i],b:obs[j],distance:distances[i*n+j],modalities:['gene','bcr'],contributing_views:2})
  const fitted={model:'unsupervised-median-scaled-mean-distance',min_views:2,observations:obs,
    views:[{modality:'gene'},{modality:'bcr'}],pairs}
  const browser=analyzeJointStructure(fitted,{selectedModalities:['gene','bcr']})
  const reference=JSON.parse(wasm.reference_joint_structure_json(JSON.stringify({observations:browser.observations,distances:browser.distances})))
  assert.equal(reference.schema,'4c-insight-native-joint-reference-v1')
  assert.deepEqual(reference.observations,browser.observations)
  assert.equal(reference.merges.length,browser.clustering.merges.length)
  assert.deepEqual(reference.leaf_order,browser.clustering.leaf_order)
  for(let i=0;i<reference.merges.length;i++){
    const a=reference.merges[i],b=browser.clustering.merges[i]
    assert.equal(a.id,b.id); assert.equal(a.left,b.left);assert.equal(a.right,b.right)
    assert.ok(Math.abs(a.height-b.height)<1e-9)
  }
  assert.ok(Math.abs(reference.negative_inertia_ratio-browser.pcoa.negative_inertia_ratio)<1e-7)
  for(let k=0;k<2;k++)assert.ok(Math.abs(reference.axis_variance_explained[k]-browser.pcoa.axis_variance_explained[k])<1e-7)
  // Eigenvector sign and rotations in degenerate eigenspaces are arbitrary.
  // Compare pairwise PCoA distances instead of raw axes.
  for(let i=0;i<n;i++)for(let j=i+1;j<n;j++){
    const a=reference.points,b=browser.pcoa.points
    const ref=Math.hypot(a[i].x-a[j].x,a[i].y-a[j].y)
    const got=Math.hypot(b[i].x-b[j].x,b[i].y-b[j].y)
    assert.ok(Math.abs(ref-got)<1e-6,`PCoA relative geometry differs for ${obs[i]} / ${obs[j]}`)
  }
})
