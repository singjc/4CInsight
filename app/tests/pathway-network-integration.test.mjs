import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {buildPathwayNetwork,layoutPathwayNetwork,pathwayNetworkSvg} from '../pathway-network.js'
const src=p=>readFileSync(new URL('../'+p,import.meta.url),'utf8')

test('P5: pathways render network only inside calculated results, without another science fit',()=>{
  const view=src('pathway-view.js')
  assert.match(view,/id="pathway-network-widget"/)
  assert.match(view,/import \{mountPathwayNetwork,normalizePathwayNetworkConfig\} from '\.\/pathway-network\.js\?v=0\.22\.8'/)
  assert.match(view,/mountPathwayNetwork\(slot,\{report,geneSets:workspace\.pathwayGeneSets,input:prepared/)
  assert.match(view,/pathwayInputSignature\(prepared,workspace\.pathwayGeneSets\)===report\.inputSignature/)
  assert.match(view,/onPathway:index=>/)
  const start=view.indexOf('onPathway:index=>')
  assert.match(view.slice(start,start+700),/\.pathway-select\[data-pathway-index=/)
  assert.match(view.slice(start,start+700),/pathwaySelectionKey\(report\.results\[index\]\)/)
})

test('P5: network preferences restore without storing another copy of scores or GMT',()=>{
  const main=src('main.js')
  assert.match(main,/pathwayNetworkConfig: \{\}/)
  assert.match(main,/pathwayNetworkConfig: \{ \.\.\.state\.pathwayNetworkConfig \}/)
  assert.match(main,/saved\.pathwayNetworkConfig && typeof saved\.pathwayNetworkConfig === 'object'/)
  assert.match(main,/filename\.endsWith\('\.svg'\)\?'image\/svg\+xml'/)
  assert.doesNotMatch(main,/pathwayNetworkResult: null/)
})

test('P5: controls expose both graph types, zoom, pan, box selection, and exact export types',()=>{
  const source=src('pathway-network.js')
  for(const text of ['Pathway overlap','Genes ↔ pathways','Jaccard','Overlap coefficient','Leading edge / ORA selected',
    'All GMT matched genes','Maximum gene nodes (display only)','Box zoom','Zoom in','Zoom out','Reset',
    'Export nodes TSV','Export edges TSV','Export SVG','Export network JSON'])assert.ok(source.includes(text),text)
  for(const token of ['wheel','pointerdown','pointermove','pointerup','pointercancel','aria-label'])assert.ok(source.includes(token),token)
})

test('P5: SVG protects injected names and has no run-time layout randomness',()=>{
  const input={rows:['A','B','C'].map(gene=>({gene,score:1,q:0.01}))}
  const sets={pathways:[{name:'<svg onload=alert(1)>',description:'R1',genes:['A','B','C']}]}
  const report={mode:'ora',results:[{pathway:'<svg onload=alert(1)>',description:'R1',leadingGenes:['A'],matchedSetSize:3,q_value:0.02,p_value:0.001,foldEnrichment:3}]}
  const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,{maxPathways:2},input));graph.reportMode='ora'
  const svg=pathwayNetworkSvg(graph)
  assert.match(svg,/&lt;svg onload=alert\(1\)&gt;/)
  assert.doesNotMatch(svg,/<svg onload=alert\(1\)>/)
  assert.doesNotMatch(src('pathway-network.js'),/Math\.random\(/)
})

test('P5: original scientific pipeline, GMT parsing and gene inspection are separate from graph',()=>{
  const code=src('pathway-network.js')
  assert.doesNotMatch(code,/hypergeometricTail|runPathwayEnrichment|bhAdjust|preparePathwayGenes|ReactomePathways\.gmt\.zip/)
  assert.doesNotMatch(code,/\bfetch\s*\(|navigator\.sendBeacon|XMLHttpRequest|new WebSocket/)
})
