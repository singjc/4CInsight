import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
const read=x=>readFileSync(new URL(x,import.meta.url),'utf8')
test('Biological Findings shows one combined all-seven-modality dashboard',()=>{
 const main=read('../main.js')
 assert.match(main,/modalityOverviewHtml\(report\)/)
 assert.match(main,/featureLollipopHtml\(feature/)
 assert.match(main,/combinedFeatureRows\(feature\)/)
 assert.match(main,/biologicalFeatureScatterSvg\(report,profile\)/)
 assert.match(main,/biologicalFindingsPanel\(\)/)
 assert.match(main,/Seven data types are shown above/)
})
test('post-fit dashboard updates metadata without another Run or Next action',()=>{
 const main=read('../main.js')
 const fn=main.slice(main.indexOf('function biologicalFindingsPanel() {'),main.indexOf('function significantHitsView() {'))
 assert.match(fn,/if\(structure && state\.alignmentPreview && state\.alignmentTable && !state\.bioResult/)
 assert.match(fn,/buildBiologicalFindings\(state\.i2StructureResult,state\.alignmentPreview/)
 assert.doesNotMatch(fn,/id="bio-run"|id="bio-feature-kind"/)
 assert.match(fn,/Rank associated features from both RNA types/)
 assert.match(fn,/for\(const kind of \['gene','transcript'\]\)/)
 assert.match(fn,/operation stopped\|operation cancelled\|aborted/)
})
test('no invented feature ranking for TCR, BCR, isotypes or immune signatures',()=>{
 const main=read('../main.js'),helper=read('../biological-dashboard.js')
 assert.match(main,/feature-level ranking is currently available only for gene and transcript TPM/i)
 assert.match(helper,/Not in fitted model/)
 assert.doesNotMatch(helper,/p_value|q_value|BH-adjusted/)
})
test('full searchable table and exports preserve all RNA rows',()=>{
 const main=read('../main.js')
 assert.match(main,/dynamicTableSources\.set\('bio-feature-table'/)
 assert.match(main,/data-dynamic-table="bio-feature-table"/)
 assert.match(main,/combinedFeatureTsv\(state\.bioFeatureResult\)/)
 assert.match(main,/bio-features-json/)
})
test('cache revision is coherent and feature worker stays local',()=>{
 const main=read('../main.js'),html=read('../index.html'),worker=read('../analysis-worker.js')
 const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
 assert.equal(version,'0.22.5')
 assert.ok(html.includes(`main.js?v=${version}`))
 assert.ok(html.includes(`styles.css?v=${version}`))
 assert.ok(main.includes(`biological-dashboard.js?v=${version}`))
 assert.ok(worker.includes(`biological-feature-ranking.js?v=${version}`))
 for(const src of [read('../biological-dashboard.js'),read('../biological-feature-ranking.js')])
  assert.doesNotMatch(src,/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
})
