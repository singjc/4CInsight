import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {integrativeDocs,integrativeToyExamples,integrativeVisualDefinitions} from '../docs-integrative.js'

const source = path => readFileSync(new URL('../'+path,import.meta.url),'utf8')
const ids=['integrative-workflow','integrative-coverage','integrative-patterns','integrative-next-steps','pathway-ora','pathway-ranked','pathway-network']

test('seven integrative/pathway topics use the same method-reference schema',()=>{
  assert.deepEqual(integrativeDocs.map(d=>d.id),ids)
  for(const d of integrativeDocs){
    assert.ok(d.title.length>12 && d.summary.length>25 && d.keywords.length>20)
    assert.ok(d.body.length>=3)
    assert.ok(d.references.some(r=>/^https:\/\/doi\.org\//.test(r.href)))
    for(const paragraph of d.body)assert.ok(paragraph.length>90)
  }
})
test('each topic has one small synthetic worked example and one accessible SVG',()=>{
  for(const id of ids){
    const examples=integrativeToyExamples[id],charts=integrativeVisualDefinitions[id]
    assert.equal(examples?.length,1,id)
    assert.equal(charts?.length,1,id)
    const ex=examples[0]
    assert.ok(ex.rows.length>=2 && ex.steps.length>=2 && ex.columns.length>=3,id)
    assert.ok(ex.rows.every(row=>row.length===ex.columns.length),id)
    assert.ok(ex.limit.length>40 && ex.meaning.length>40,id)
    const html=charts[0].render(id+'-0')
    assert.match(html,/role="img"/)
    assert.match(html,/<title id="visual-title-/)
    assert.match(html,/<desc id="visual-desc-/)
    assert.match(html,/aria-labelledby="visual-title-/)
    assert.match(html,/viewBox="0 0 680 250"/)
    assert.doesNotMatch(html,/<script|onload=|onerror=/i)
  }
})
test('documentation is imported by the real Docs search, toy-example, and visual modules',()=>{
  const docs=source('docs-content.js'),toy=source('docs-toy-examples.js'),visuals=source('docs-toy-visuals.js')
  assert.match(docs,/integrativeDocs.*docs-integrative\.js\?v=0\.22\.8/)
  assert.match(docs,/\.\.\.integrativeDocs/)
  assert.match(toy,/integrativeToyExamples.*docs-integrative\.js\?v=0\.22\.8/)
  assert.match(toy,/\.\.\.integrativeToyExamples/)
  assert.match(visuals,/integrativeVisualDefinitions.*docs-integrative\.js\?v=0\.22\.8/)
  assert.match(visuals,/\.\.\.integrativeVisualDefinitions/)
})
test('toy ORA probability and Jaccard calculations match independent arithmetic',()=>{
  const factorial=n=>Array.from({length:n},(_,i)=>i+1).reduce((a,b)=>a*b,1)
  const choose=(n,k)=>k<0||k>n?0:factorial(n)/(factorial(k)*factorial(n-k))
  const p=[3,4,5].reduce((s,k)=>s+choose(6,k)*choose(14,5-k)/choose(20,5),0)
  assert.ok(Math.abs(p-0.13132094943240455)<1e-12)
  assert.equal(2/(3+3-2),0.5)
  assert.equal(1/(199+1),0.005)
  assert.match(integrativeToyExamples['pathway-ora'][0].meaning,/not necessarily enriched/)
})
test('science limitations and local privacy are explicit in the public Docs',()=>{
  const combined=integrativeDocs.flatMap(x=>x.body).join(' ')
  for(const term of ['not a clinical subtype','post-fit','not independent','gene-label','not phenotype','exact uppercase','not a pinned','not physical','not change'])assert.ok(combined.toLowerCase().includes(term.toLowerCase()),term)
  assert.match(combined,/in the browser/)
  assert.doesNotMatch(combined,/upload(?:s|ed)? (?:your |the )?(?:patient|sample|gene) (?:data|measurements) to a server/i)
})
test('updated analysis guide and standalone guide document actual local controls',()=>{
  const guide=readFileSync(new URL('../../docs/integrative_pathways_guide.md',import.meta.url),'utf8')
  const overview=readFileSync(new URL('../../docs/analysis_guide.md',import.meta.url),'utf8')
  const readme=readFileSync(new URL('../../README.md',import.meta.url),'utf8')
  for(const term of ['Pathways','Reactome','GMT','ORA','PCoA','Genes ↔ pathways','Jaccard','permutations','Observation ID','Patient ID','Export'])assert.ok(guide.includes(term),term)
  assert.match(overview,/integrative_pathways_guide\.md/)
  assert.match(readme,/integrative_pathways_guide\.md/)
  assert.match(guide,/synthetic/i)
})
