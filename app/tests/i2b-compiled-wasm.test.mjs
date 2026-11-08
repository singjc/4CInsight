import test from 'node:test'
import assert from 'node:assert/strict'
import {readFile, access} from 'node:fs/promises'
const js=new URL('../wasm/four_c_insight_wasm.js',import.meta.url)
const bg=new URL('../wasm/four_c_insight_wasm_bg.wasm',import.meta.url)
const strict=process.env.CFS_REQUIRE_WASM_ARTIFACTS==='1'
async function exists(url){try{await access(url);return true}catch(e){if(e.code==='ENOENT')return false;throw e}}
const both=(await exists(js))&&(await exists(bg))

test('fresh compiled I2B WASM parses all five raw sample types and both merged expression tables',{
  skip:!strict ? 'Requires fresh wasm-pack build and CFS_REQUIRE_WASM_ARTIFACTS=1' : false,
},async()=>{
  assert.ok(both,'Fresh compiled WASM missing; rebuild before I2B validation')
  const wasm=await import(js.href)
  await wasm.default({module_or_path:await readFile(bg)})
  for(const fn of ['i2b_begin_json','i2b_add_sample_json','i2b_finish_json','i2b_abort','i2b_expression_distance_json']){
    assert.equal(typeof wasm[fn],'function',`${fn} missing in built WASM`)
  }
  for(const [kind,suffix,expected] of [
    ['bcr','bcr_heavy_chain_report','1 - Morisita-Horn similarity'],
    ['tcrAlpha','tcr_alpha_clone_report','1 - Morisita-Horn similarity'],
    ['tcrBeta','tcr_beta_clone_report','1 - Morisita-Horn similarity'],
    ['isotype','bcr_isotype_composition','Aitchison distance'],
    ['immune','immune_infiltrate','Euclidean distance on standardized immune-signature scores'],
  ]) {
    wasm.i2b_begin_json(kind,JSON.stringify({ambiguousPolicy:'exclude',determinedOnly:false}))
    const firstName=`S1_${suffix}.xlsx`
    assert.throws(()=>wasm.i2b_add_sample_json(firstName,'S1',new Uint8Array([1,2,3,4])),/./)
    for(let i=1;i<=8;i++){
      const name=`S${i}_${suffix}.xlsx`
      const bytes=await readFile(new URL(`../../tests/fixtures/m2_multiomics/${name}`,import.meta.url))
      wasm.i2b_add_sample_json(name,`S${i}`,bytes)
    }
    const view=JSON.parse(wasm.i2b_finish_json())
    assert.equal(view.modality,kind)
    assert.equal(view.distance_label,expected)
    assert.equal(view.samples,8)
    assert.equal(view.pairs.length,28)
    assert.ok(view.pairs.every(p=>p.distance>=0&&Number.isFinite(p.distance)))
    assert.equal(view.pairs[0].a,'S1')
  }
  const ids=['C1_T0','C1_T1','C2_T0','C2_T1','N1_T0','N1_T1','N2_T0','N2_T1']
  for(const [kind,file] of [['gene','w3_merged_gene_expression.tsv'],['transcript','w3_merged_transcript_expression.tsv']]){
    const bytes=await readFile(new URL(`../../tests/fixtures/${file}`,import.meta.url))
    const view=JSON.parse(wasm.i2b_expression_distance_json(kind,bytes,JSON.stringify(ids),JSON.stringify({minCount:10,minFractionSamples:0.2})))
    assert.equal(view.modality,kind)
    assert.equal(view.pairs.length,28)
    assert.ok(view.pairs.every(p=>Number.isFinite(p.distance)&&p.distance>=0))
  }
  assert.throws(()=>wasm.i2b_begin_json('bcr',JSON.stringify({ambiguousPolicy:'bad'})),/./)
})
