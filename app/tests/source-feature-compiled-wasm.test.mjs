import test from 'node:test'
import assert from 'node:assert/strict'
import {readFile,access} from 'node:fs/promises'
const js=new URL('../wasm/four_c_insight_wasm.js',import.meta.url)
const bg=new URL('../wasm/four_c_insight_wasm_bg.wasm',import.meta.url)
const strict=process.env.CFS_REQUIRE_WASM_ARTIFACTS==='1'
test('compiled native I2B parser returns per-sample features for all five data types',{
 skip:!strict?'Requires fresh wasm-pack build and strict tests':false,
},async()=>{
 await access(js);await access(bg)
 const wasm=await import(js.href);await wasm.default({module_or_path:await readFile(bg)})
 for(const [modality,suffix,expected] of [
   ['bcr','bcr_heavy_chain_report','Repertoire metric'],
   ['tcrAlpha','tcr_alpha_clone_report','V gene fraction'],
   ['tcrBeta','tcr_beta_clone_report','V gene fraction'],
   ['isotype','bcr_isotype_composition','Isotype fraction'],
   ['immune','immune_infiltrate','Immune signature score'],
 ]){
   wasm.i2b_begin_json(modality,JSON.stringify({ambiguousPolicy:'exclude',determinedOnly:false}))
   for(let i=1;i<=8;i++){
     const name=`S${i}_${suffix}.xlsx`
     const bytes=await readFile(new URL(`../../tests/fixtures/m2_multiomics/${name}`,import.meta.url))
     wasm.i2b_add_sample_json(name,`S${i}`,bytes)
   }
   const view=JSON.parse(wasm.i2b_finish_json())
   assert.equal(view.feature_samples.length,8,`${modality} sample coverage`)
   assert.equal(new Set(view.feature_samples.map(s=>s.sample_id)).size,8)
   assert.ok(view.feature_samples.every(s=>s.features.length>0))
   assert.ok(view.feature_samples.every(s=>s.features.every(f=>Number.isFinite(f.value))))
   assert.ok(view.feature_samples.some(s=>s.features.some(f=>f.family===expected)),`${modality} native feature family`)
   assert.equal(view.pairs.length,28,`${modality} original scientific distances retained`)
 }
})
