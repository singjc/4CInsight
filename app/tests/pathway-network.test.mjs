import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {
  PATHWAY_NETWORK_SCHEMA,defaultPathwayNetworkConfig,normalizePathwayNetworkConfig,
  buildPathwayNetwork,layoutPathwayNetwork,pathwayNetworkSvg,
  pathwayNetworkNodesTsv,pathwayNetworkEdgesTsv,pathwayNetworkJson,
} from '../pathway-network.js'

const report={mode:'ranked',source:'exploratory',method:'Gene-label permutations',
  results:[
    {pathway:'P1',description:'R1',matchedSetSize:3,leadingGenes:['A','B'],q_value:0.01,p_value:0.01,normalizedScore:2.4,enrichmentScore:.4},
    {pathway:'P2',description:'R2',matchedSetSize:3,leadingGenes:['B','C'],q_value:0.02,p_value:0.02,normalizedScore:-1.5,enrichmentScore:-.3},
    {pathway:'P3',description:'R3',matchedSetSize:2,leadingGenes:['D'],q_value:0.04,p_value:0.03,normalizedScore:1.8,enrichmentScore:.3},
    {pathway:'P4',description:'R4',matchedSetSize:2,leadingGenes:['F'],q_value:0.4,p_value:0.2,normalizedScore:1.1,enrichmentScore:.1},
  ]}
const sets={filename:'test.gmt',pathways:[
  {name:'P1',description:'R1',genes:['A','B','C','Z']},
  {name:'P2',description:'R2',genes:['B','C','E']},
  {name:'P3',description:'R3',genes:['D','E']},
  {name:'P4',description:'R4',genes:['F','A']},
]}
const input={rows:['A','B','C','D','E','F'].map((gene,i)=>({gene,score:(i-3)/5,q:null}))}
const opts={...defaultPathwayNetworkConfig(),maxPathways:4,minCommon:1,minSimilarity:0,genes:'matched'}

test('P5: exact scored background excludes GMT-only genes in matched mode',()=>{
  const graph=buildPathwayNetwork(report,sets,opts,input)
  assert.equal(graph.schema,PATHWAY_NETWORK_SCHEMA)
  assert.equal(graph.nodes.length,3)
  assert.equal(graph.nodes[0].geneSetSize,4) // Original file records Z
  assert.deepEqual(graph.nodes[0].genes,['A','B','C']) // Z was not scored
  assert.equal(graph.eligiblePathways,3)
})
test('P5: Jaccard and overlap coefficient have exact, independently checked values',()=>{
  const graph=buildPathwayNetwork(report,sets,opts,input)
  const p1p2=graph.edges.find(e=>e.source==='p0'&&e.target==='p1')
  assert.deepEqual(p1p2.commonGenes,['B','C'])
  assert.equal(p1p2.overlapGenes,2)
  assert.equal(p1p2.similarity,2/4)
  const coef=buildPathwayNetwork(report,sets,{...opts,metric:'overlap'},input)
  assert.equal(coef.edges.find(e=>e.source==='p0'&&e.target==='p1').similarity,2/3)
})
test('P5: leading-edge mode uses only saved leading-edge genes',()=>{
  const graph=buildPathwayNetwork(report,sets,{...opts,genes:'leading'},input)
  assert.deepEqual(graph.nodes[0].genes,['A','B'])
  assert.deepEqual(graph.nodes[1].genes,['B','C'])
  assert.deepEqual(graph.edges.find(e=>e.source==='p0'&&e.target==='p1').commonGenes,['B'])
  assert.equal(graph.edges.some(e=>e.overlapGenes===0),false)
})
test('P5: edge threshold, gene count and q cutoff are applied only to display selection',()=>{
  const graph=buildPathwayNetwork(report,sets,{...opts,minCommon:2,minSimilarity:0.55,qCutoff:0.04},input)
  assert.equal(graph.nodes.length,3)
  assert.equal(graph.edges.some(e=>e.source==='p0'&&e.target==='p1'),false) // 0.5 < 0.55
  assert.equal(report.results.length,4)
  const two=buildPathwayNetwork(report,sets,{...opts,maxPathways:2},input)
  assert.equal(two.nodes.length,2)
  assert.equal(report.results[2].q_value,0.04)
})
test('P5: no invented GMT membership when exact set is missing or ambiguous',()=>{
  const corrupted={...sets,pathways:[sets.pathways[0],sets.pathways[0],sets.pathways[2],sets.pathways[3]]}
  const graph=buildPathwayNetwork(report,corrupted,opts,input)
  assert.equal(graph.nodes.length,1)
  assert.equal(graph.excludedExactSets.length,2)
  assert.match(graph.excludedExactSets[0].reason,/Ambiguous/)
})
test('P5: saved matched gene count mismatch blocks the pathway instead of guessing',()=>{
  const changed={...report,results:[{...report.results[0],matchedSetSize:4}]}
  const graph=buildPathwayNetwork(changed,sets,opts,input)
  assert.equal(graph.nodes.some(n=>n.name==='P1'),false)
  assert.match(graph.excludedExactSets[0].reason,/differs/)
})
test('P5: gene-pathway network only uses selected measured memberships',()=>{
  const graph=buildPathwayNetwork(report,sets,{...opts,view:'bipartite',hideSingleGenes:true},input)
  const geneNodes=graph.nodes.filter(n=>n.type==='gene')
  assert.deepEqual(geneNodes.map(n=>n.name),['B','C','E'])
  assert.equal(graph.edges.every(e=>e.type==='gene_membership'),true)
  assert.equal(graph.edges.filter(e=>e.gene==='B').length,2)
  assert.equal(graph.edges.length,6)
})
test('P5: gene-only display cap keeps full enrichment results and scores intact',()=>{
  const graph=buildPathwayNetwork(report,sets,{...opts,view:'bipartite',maxGenes:1},input)
  assert.equal(graph.nodes.filter(n=>n.type==='gene').length,1)
  assert.equal(graph.totalEligibleGenes,5)
  assert.equal(report.results[0].normalizedScore,2.4)
  assert.equal(report.results.length,4)
})
test('P5: insufficient/absent original gene background explicitly blocks graph',()=>{
  assert.throws(()=>buildPathwayNetwork(report,sets,opts),/scored gene background/)
  assert.throws(()=>buildPathwayNetwork(report,sets,opts,{rows:[]}),/scored gene background/)
})
test('P5: deterministic layout and no modifications to source report',()=>{
  const before=JSON.stringify(report)
  const a=layoutPathwayNetwork(buildPathwayNetwork(report,sets,opts,input))
  const b=layoutPathwayNetwork(buildPathwayNetwork(report,sets,opts,input))
  assert.deepEqual(a,b)
  assert.equal(JSON.stringify(report),before)
  assert.ok(a.nodes.every(n=>Number.isFinite(n.x)&&Number.isFinite(n.y)))
})
test('P5: SVG keeps a fixed coordinate frame, visible tooltips and accessible nodes',()=>{
  const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,opts,input));graph.reportMode='ranked'
  const one=pathwayNetworkSvg(graph)
  const zoomed=pathwayNetworkSvg(graph,{view:{cx:500,cy:285,k:3}})
  assert.match(one,/viewBox="0 0 1000 570"/)
  assert.match(one,/data-network-node=/)
  assert.match(one,/role="button"/)
  assert.match(zoomed,/viewBox="0 0 1000 570"/)
  assert.doesNotMatch(zoomed,/transform="scale\(/)
  assert.doesNotMatch(one,/protein interaction confirmed/i)
})
test('P5: exports retain exact nodes/edges and escape spreadsheet formulas',()=>{
  const malicious={...report,results:[{...report.results[0],pathway:'=bad()'}]}
  const maliciousSets={pathways:[{name:'=bad()',description:'R1',genes:['A','B','C']}]}
  const graph=buildPathwayNetwork(malicious,maliciousSets,opts,input)
  const nodes=pathwayNetworkNodesTsv(graph)
  assert.match(nodes,/'=bad\(\)/)
  assert.equal(nodes.trim().split('\n').length,2)
  const edges=pathwayNetworkEdgesTsv(graph)
  assert.match(edges,/source\ttarget\tedge_type/)
  assert.ok(JSON.parse(pathwayNetworkJson(graph,malicious)).graph.nodes.length===1)
})
test('P5: valid settings are bounded for visual rendering but no source-file limit is imposed',()=>{
  const c=normalizePathwayNetworkConfig({maxPathways:99999,maxGenes:99999,qCutoff:4,seed:-10})
  assert.equal(c.maxPathways,250)
  assert.equal(c.maxGenes,500)
  assert.equal(c.qCutoff,1)
  assert.equal(c.seed,1)
  const code=readFileSync(new URL('../pathway-network.js',import.meta.url),'utf8')
  assert.doesNotMatch(code,/\bfetch\s*\(|WebSocket|XMLHttpRequest|sendBeacon|\/api\//)
  assert.doesNotMatch(code,/Math\.random|d3-force|cytoscape/)
})
