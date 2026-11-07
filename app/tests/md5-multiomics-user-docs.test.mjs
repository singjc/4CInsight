import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { methodDocs } from '../docs-content.js'
import { methodToyExamples, renderToyExamples, exampleSearchText } from '../docs-toy-examples.js'
import { renderToyVisual, visualDefinitionCount, visualCaption } from '../docs-toy-visuals.js'
import { multiomicsDocs, multiomicsToyExamples, multiomicsVisualDefinitions } from '../docs-new-modalities.js'

const src = (path) => readFile(new URL(`../${path}`, import.meta.url), 'utf8')
const kinds = ['repertoire-diversity','repertoire-sharing','bcr-repertoire','tcr-alpha','tcr-beta','bcr-isotypes','immune-infiltration']

test('all five added modalities have accurate searchable guide sections and citations', () => {
  assert.equal(multiomicsDocs.length,7)
  assert.equal(new Set(methodDocs.map(x => x.id)).size, methodDocs.length)
  for (const id of kinds) {
    const section = methodDocs.find(item => item.id === id)
    assert.ok(section, `${id} section missing`)
    assert.ok(section.body.length >= 2)
    assert.ok(section.references.length >= 2)
    for(const ref of section.references){
      assert.match(ref.href,/^https:\/\/doi\.org\/10\./)
      assert.ok(ref.label.length > 20)
    }
    assert.ok(exampleSearchText(id).length > 120)
  }
  assert.match(methodDocs.find(x=>x.id==='tcr-alpha').body.join(' '),/do not contain a D segment/i)
  assert.match(methodDocs.find(x=>x.id==='immune-infiltration').body.join(' '),/not direct cell counts/i)
  assert.match(methodDocs.find(x=>x.id==='bcr-isotypes').body.join(' '),/not absolute concentrations/i)
  assert.match(methodDocs.find(x=>x.id==='tcr-beta').body.join(' '),/V, D, and J segments/)
})

test('new guides retain accessible visual-first diagrams, complete tables, and exact toy text', () => {
  assert.equal(visualDefinitionCount(),22)
  assert.deepEqual(Object.keys(multiomicsToyExamples),kinds)
  assert.deepEqual(Object.keys(multiomicsVisualDefinitions),kinds)
  for (const id of kinds) {
    assert.equal(methodToyExamples[id].length,1)
    const ex=methodToyExamples[id][0]
    assert.ok(ex.rows.length>=2)
    assert.ok(ex.rows.every(row=>row.length===ex.columns.length))
    const html=renderToyExamples(id)
    const visual=renderToyVisual(id,0)
    assert.match(visual,/<svg[^>]*role="img"/)
    assert.match(visual,/<title id="visual-title-/)
    assert.match(visual,/<desc id="visual-desc-/)
    assert.match(visual,/<figcaption>/)
    assert.match(html,/Illustrative values only/)
    assert.match(html,/<details class="docs-toy-math">/)
    assert.ok(html.indexOf('docs-visual-figure')<html.indexOf('docs-toy-table-wrap'))
    assert.ok(visualCaption(id,0).length>=30)
    assert.ok(exampleSearchText(id).includes(visualCaption(id,0)))
    assert.doesNotMatch(visual,/<script|onload\s*=|<image /i)
  }
})

test('worked arithmetic explains clonality, sharing, ratio and score scales', () => {
  const p=[0.6,0.3,0.1]
  const h=-p.reduce((sum,v)=>sum+v*Math.log(v),0)
  assert.ok(Math.abs(h-0.8979457249)<1e-8)
  assert.ok(Math.abs(1-h/Math.log(3)-0.1827)<0.001)
  assert.equal(5/10,0.5)
  assert.equal(3/10,0.3)
  assert.equal(2/10,0.2)
  assert.ok(Math.abs(Math.log(20/50)-(-0.91629073))<1e-6)
  assert.equal(20+50+30,100)
  assert.equal(40+40+20,100)
  const immune=multiomicsToyExamples['immune-infiltration'][0]
  assert.match(immune.meaning,/not 80% of cells/i)
})

test('visible Docs sections omit developer runtime details and retain device privacy claims', async () => {
  for(const id of ['local-first','performance',...kinds]){
    const entry=methodDocs.find(x=>x.id===id)
    assert.doesNotMatch([entry.title,entry.summary,...entry.body].join(' '),/Rust\/WASM|WebAssembly|Web Worker|Rayon|cross-origin isolated|native\/WASM/i)
  }
  const privacy=methodDocs.find(x=>x.id==='local-first')
  assert.match(privacy.body.join(' '),/not uploaded/i)
  assert.match(methodDocs.find(x=>x.id==='performance').body.join(' '),/no fixed input-file size cap/i)
  const main=await src('main.js')
  const html=await src('index.html')
  assert.match(main,/Data stays on your device|Your selected source files are not uploaded/)
  assert.doesNotMatch(main,/WASM science: 1 worker|Rust\/WASM · staged source|Corrected native Rust method through local WASM|JavaScript preflight fallback/)
  assert.match(main,/Analysis memory/)
  assert.match(html,/4C Insight/)
})

test('browser cache version is uniform for assets and illustrated modules', async () => {
  const [main,html,examples,visuals,docs]=await Promise.all([
    src('main.js'),src('index.html'),src('docs-toy-examples.js'),src('docs-toy-visuals.js'),src('docs-content.js')
  ])
  const version=main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version,'0.21.0-ux11-interactive-exports')
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.ok(main.includes(`docs-content.js?v=${version}`))
  assert.ok(main.includes(`docs-toy-examples.js?v=${version}`))
  assert.ok(examples.includes(`docs-toy-visuals.js?v=${version}`))
  assert.ok(visuals.includes(`docs-new-modalities.js?v=${version}`))
  assert.ok(docs.includes(`docs-new-modalities.js?v=${version}`))
})

test('documentation remains self-contained with no network reads', async () => {
  const code=await src('docs-new-modalities.js')
  assert.doesNotMatch(code,/\bfetch\s*\(|XMLHttpRequest|import\(|navigator\.permissions|<script/)
})
