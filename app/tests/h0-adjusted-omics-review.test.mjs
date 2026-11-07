import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import { collectAdjustedTests, selectAdjustedTests, adjustedTestsCsv } from '../extended-hit-review.js'

const base = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const source = readFileSync(path.join(base, 'main.js'), 'utf8')

function testRow(feature, q, effect) { return { feature, q_value: q, beta_group: effect, p_value: q/2, secondary_id: '' } }
const compare = {
  bcr: { analysis: { metric_tests: [testRow('richness', 0.003, -0.7)], compositions: [{family:'V_gene', tests:[testRow('IGHV1',0.01,1.2)]}] } },
  tcrAlpha: { analysis: { metric_tests: [], compositions: [{family:'VJ', tests:[testRow('TRAJ12',0.2,1.3)]}] } },
  tcrBeta: { analysis: { metric_tests: [], compositions: [] } },
  isotype: { analysis: { tests: [testRow('IgG1',0.04,-2.5)] } },
  immune: { analysis: { tests: [testRow('CD8 T cells',0.03,0.6)] } },
}

test('all five modalities use only completed corrected tests', () => {
  const groups = collectAdjustedTests(compare)
  assert.deepEqual(groups.map(g => g.key), ['bcr','tcrAlpha','tcrBeta','isotype','immune'])
  assert.deepEqual(groups.map(g => g.rows.length), [2,1,0,1,1])
  assert.match(groups[0].rows[1].family, /CLR composition/)
  assert.match(groups[3].rows[0].family, /isotype CLR/)
  assert.match(groups[4].rows[0].family, /ssGSEA/)
})

test('missing corrected modality produces no feature tests', () => {
  const out=collectAdjustedTests({ immune: { error:'failed' } })
  assert.equal(out.filter(x => x.available).length, 0)
  assert.equal(out.flatMap(x=>x.rows).length,0)
})

test('FDR and adjusted effects select only tested finite features', () => {
  const rows=collectAdjustedTests(compare).flatMap(g=>g.rows)
  assert.deepEqual(selectAdjustedTests(rows, 0.05, 0.8).map(r=>r.feature), ['IGHV1','IgG1'])
  assert.deepEqual(selectAdjustedTests(rows, 0.05, 0).map(r=>r.feature), ['richness','IGHV1','IgG1','CD8 T cells'])
  assert.throws(()=>selectAdjustedTests(rows,1.3,0))
  assert.throws(()=>selectAdjustedTests(rows,0.1,-1))
  assert.deepEqual(selectAdjustedTests([{feature:'f', q_value:NaN,adjusted_effect:10}],0.05),[])
  const noValues=collectAdjustedTests({immune:{analysis:{tests:[{feature:'missing effect',q_value:0.001,beta_group:null},{feature:'missing q',q_value:null,beta_group:2}]}}})[4].rows
  assert.equal(selectAdjustedTests(noValues,0.05,0).length,0)
})

test('CSV exports unthinned data with correct escaping', () => {
  const csv=adjustedTestsCsv([{modality:'BCR',family:'V,J',feature:'"quote"',secondary_id:'a\nb',adjusted_effect:0.5,q_value:0.01,p_value:0.001}])
  assert.match(csv,/"V,J"/)
  assert.match(csv,/"""quote"""/)
  assert.match(csv,/"a\nb"/)
})

test('review does not claim to perform a new significant-hit clustering fit', () => {
  assert.match(source,/collectAdjustedTests/)
  assert.match(source,/selectAdjustedTests/)
  assert.match(source,/export-omics-adjusted-hits/)
  assert.match(source,/Full selected-hit PCA and clustering for these modalities are not yet available/)
  assert.doesNotMatch(source,/function runAdjustedOmicsFit/)
})
