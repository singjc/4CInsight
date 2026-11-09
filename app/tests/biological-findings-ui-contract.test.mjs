import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
const read=p=>readFileSync(new URL(p,import.meta.url),'utf8')
test('Biological Findings is post-fit, visible, and independent of corrected groups',()=>{
 const main=read('../main.js'),worker=read('../analysis-worker.js')
 assert.match(main,/function biologicalFindingsPanel\(\)/)
 assert.match(main,/\$\{biologicalFindingsPanel\(\)\}/)
 assert.match(main,/bindBiologicalFindingsEvents\(\)/)
 assert.match(main,/buildBiologicalFindings\(state\.i2StructureResult/)
 assert.match(worker,/rankExpressionFeatures\(request\.file/)
 assert.match(main,/Rank associated features/)
 assert.match(main,/biological: \{ annotationColumn: state\.bioMetadataColumn/)
 const page=main.split('function significantHitsView() {')[1].split('async function runSignificantHits() {')[0]
 assert.doesNotMatch(page,/I[012][ABC]?\s*·|Run Compare Groups first/)
})
test('version and privacy contract keeps local-only dependencies',()=>{
 const main=read('../main.js'),html=read('../index.html')
 const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
 assert.equal(version,'0.22.7')
 assert.ok(html.includes(`main.js?v=${version}`))
 assert.ok(main.includes(`biological-findings.js?v=${version}`))
 assert.ok(read('../analysis-worker.js').includes(`biological-feature-ranking.js?v=${version}`))
 const src=read('../biological-feature-ranking.js')
 assert.doesNotMatch(src,/\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
})
test('feature ranking is worker-only and has no inferential significance labels',()=>{
 const main=read('../main.js'),code=read('../biological-feature-ranking.js')
 assert.match(main,/workerRequest\('run-biological-feature-ranking'/)
 assert.doesNotMatch(code,/\b(?:p_value|q_value|bh_adjusted|caseValues|controlValues)\b/)
 assert.match(code,/Post-selection|Post-fit|Post-fit feature rankings/i)
})
