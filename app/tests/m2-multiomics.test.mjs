import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'
import { classifyInputFilename } from '../file-classifier.js'

const read = name => readFile(new URL(`../${name}`, import.meta.url), 'utf8')
const types = [
  ['bcr_heavy_chain_report', 'bcr'],
  ['tcr_alpha_clone_report', 'tcr-alpha'],
  ['tcr_beta_clone_report', 'tcr-beta'],
  ['bcr_isotype_composition', 'bcr-isotype'],
  ['immune_infiltrate', 'immune-infiltrate'],
]

test('source inventory recognizes all five native non-expression modalities', () => {
  for (const [suffix, expected] of types) {
    const item = classifyInputFilename(`S1_${suffix}.xlsx`)
    assert.equal(item.kind, expected)
    assert.equal(item.sampleId, 'S1')
  }
})

test('Compare Groups exposes five independent choices and model controls', async () => {
  const main = await read('main.js')
  for (const kind of types.map(item => item[1])) {
    assert.match(main, /compare-\$\{kind\}-enabled/)
    assert.ok(main.includes(`'${kind}'`))
  }
  assert.match(main, /compare-repertoire-policy/)
  assert.match(main, /compare-isotype-determined/)
  assert.match(main, /compare-repertoire-reps/)
  assert.match(main, /compare-repertoire-cap/)
  assert.match(main, /compare-repertoire-permutations/)
  assert.match(main, /otherOmicsControls\(\)/)
  assert.match(main, /multiomicsResultSection\(/)
  assert.match(main, /await isolatedScienceRequest\(/)
})

test('science worker reads one XLSX file before calling Rust and accepts partial modality failures', async () => {
  const worker = await read('analysis-worker.js')
  assert.match(worker, /for \(let i = 0; i < records\.length; i\+\+\)/)
  assert.match(worker, /const bytes = await wasmBytes\(record\.file\)/)
  assert.match(worker, /wasm\.multiomics_add_sample_json/)
  assert.match(worker, /wasm\.multiomics_begin_json/)
  assert.match(worker, /wasm\.multiomics_finish_json/)
  assert.match(worker, /wasm\.multiomics_sample_role_json/)
  assert.match(worker, /wasm\.multiomics_abort/)
  assert.match(worker, /omicsErrors/)
  assert.doesNotMatch(worker, /Promise\.all\(entry\.files\.map/)
})

test('scientific code retains original adjusted methods and per-file byte decoders', async () => {
  const corrected = await readFile(new URL('../../src/corrected.rs', import.meta.url), 'utf8')
  for (const method of ['run_corrected_repertoire_from_parsed', 'run_corrected_isotype_from_maps', 'run_corrected_immune_from_maps', 'parse_isotype_bytes', 'parse_immune_bytes', 'PortableRepertoireAccumulator']) {
    assert.ok(corrected.includes(method), method)
  }
  assert.match(corrected, /run_corrected_repertoire_from_parsed\(\s*parsed,\s*metadata,\s*comparison,\s*modality,\s*spec,\s*warnings,\s*progress,?\s*\)/)
  assert.match(corrected, /first_sheet_from_workbook/)
  assert.match(corrected, /release_repertoire_working_set/)
})

const wasmJs = new URL('../wasm/four_c_insight_wasm.js', import.meta.url)
const wasmBg = new URL('../wasm/four_c_insight_wasm_bg.wasm', import.meta.url)
const mustHaveBuild = process.env.CFS_REQUIRE_WASM_ARTIFACTS === '1'

test('compiled WASM performs corrected isotype and immune analyses from local XLSX bytes', {
  skip: !mustHaveBuild ? 'Requires a fresh wasm-pack build in the wasm-bridge CI job' : false,
}, async () => {
  await access(wasmJs)
  await access(wasmBg)
  const wasm = await import(wasmJs.href)
  await wasm.default({ module_or_path: await readFile(wasmBg) })
  for (const name of ['multiomics_begin_json', 'multiomics_sample_role_json', 'multiomics_add_sample_json', 'multiomics_finish_json', 'multiomics_abort']) {
    assert.equal(typeof wasm[name], 'function', `Missing compiled export: ${name}`)
  }
  const metaRows = Array.from({ length: 8 }, (_, index) => {
    const i = index + 1
    return `S${i}\t${[1, 2, 5, 6].includes(i) ? 'Case' : 'Control'}\tP${i}`
  })
  const meta = new TextEncoder().encode(['sample_id\tPhenotype\tsubject_id', ...metaRows, ''].join('\n'))
  const config = JSON.stringify({kind:'gene-expression', metadataIdColumn:'sample_id', groupColumn:'Phenotype', caseValues:['Case'], controlValues:['Control'], comparisonName:'Case vs Control', subjectColumn:'subject_id', covariates:[], exclusions:[], minCount:10, minFractionSamples:0.2, permutations:11})
  for (const [kind, suffix, resultKey] of [
    ['bcr-isotype', 'bcr_isotype_composition', 'tests'],
    ['immune-infiltrate', 'immune_infiltrate', 'tests'],
  ]) {
    wasm.multiomics_begin_json('synthetic.tsv', meta, config, kind, JSON.stringify({permutations:11}))
    try {
      for (let i = 1; i <= 8; i++) {
        const fname = `S${i}_${suffix}.xlsx`
        const b = await readFile(new URL(`../../tests/fixtures/m2_multiomics/${fname}`, import.meta.url))
        assert.equal(JSON.parse(wasm.multiomics_add_sample_json(fname, `S${i}`, b)).status, 'added')
      }
      const result = JSON.parse(wasm.multiomics_finish_json())
      assert.equal(result.analysis.files_used, 8)
      assert.ok(Array.isArray(result.analysis[resultKey]))
      assert.ok(result.analysis[resultKey].length > 0)
      assert.equal(result.skippedFilesOutsideGroups, 0)
    } finally { wasm.multiomics_abort() }
  }
})


test('M2 parity uses eight distinct synthetic samples and retains the adjusted model sample-size check', async () => {
  const corrected = await readFile(new URL('../../src/corrected.rs', import.meta.url), 'utf8')
  assert.match(corrected, /rows:\s*\(1\.\.=8\)/)
  assert.match(corrected, /if x\.len\(\) <= column_names\.len\(\) \+ 2/)
  for (const suffix of ['bcr_heavy_chain_report', 'bcr_isotype_composition', 'immune_infiltrate', 'tcr_alpha_clone_report', 'tcr_beta_clone_report']) {
    for (let i = 1; i <= 8; i++) {
      await access(new URL(`../../tests/fixtures/m2_multiomics/S${i}_${suffix}.xlsx`, import.meta.url))
    }
  }
})
