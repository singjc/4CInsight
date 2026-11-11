import test from 'node:test'
import assert from 'node:assert/strict'
import {parseGeneSetsGmt,preparePathwayGenes,runPathwayEnrichment} from '../pathway-enrichment.js'
import {buildPathwayNetwork,defaultPathwayNetworkConfig} from '../pathway-network.js'

const tests=Array.from({length:30},(_,i)=>({feature:`G${i+1}`,beta_group:30-i,q_value:i<7?0.002:0.8}))
const compare={comparisonName:'Defined cases vs controls',gene:{analysis:{tests}}}
const sets=parseGeneSetsGmt([
  'Signal alpha\tA\tG1\tG2\tG3\tG4\tG5\tG6\tG7\tG8\tZ_NOT_SCORED',
  'Signal beta\tB\tG4\tG5\tG6\tG7\tG8\tG9\tG10\tG11',
  'Signal gamma\tC\tG20\tG21\tG22\tG23\tG24\tG25',
].join('\n'),'synthetic-reference.gmt')
const input=preparePathwayGenes({compare,source:'compare'})

test('P5: ORA graph uses the same tested universe and exact overlap as saved enrichment',async()=>{
  const report=await runPathwayEnrichment({input,geneSets:sets,options:{mode:'ora',minSize:2,maxSize:12,qThreshold:.05,effectThreshold:1}})
  const before=JSON.stringify(report)
  const graph=buildPathwayNetwork(report,sets,{...defaultPathwayNetworkConfig(),genes:'matched',minCommon:1,minSimilarity:0,qCutoff:1},input)
  assert.equal(report.backgroundGenes,30)
  assert.equal(graph.nodes.length,report.results.length)
  assert.ok(graph.nodes.every(n=>n.genes.length===report.results[n.reportIndex].matchedSetSize))
  assert.equal(graph.nodes.some(n=>n.genes.includes('Z_NOT_SCORED')),false)
  assert.equal(JSON.stringify(report),before)
})

test('P5: ranked graph uses stored leading-edge IDs and does not compute replacement p-values',async()=>{
  const report=await runPathwayEnrichment({input,geneSets:sets,options:{mode:'ranked',minSize:2,maxSize:12,permutations:99,seed:1234}})
  const graph=buildPathwayNetwork(report,sets,{...defaultPathwayNetworkConfig(),genes:'leading',minCommon:1,minSimilarity:0,qCutoff:1},input)
  const original=new Map(report.results.map(row=>[`${row.pathway}\0${row.description}`,row]))
  for(const node of graph.nodes){
    const saved=original.get(node.pathwayKey)
    assert.ok(saved)
    assert.deepEqual([...node.genes].sort(),[...saved.leadingGenes].sort())
    assert.equal(node.q_value,saved.q_value)
    assert.equal(node.normalizedScore,saved.normalizedScore)
  }
  assert.equal(report.results.length,3)
})
