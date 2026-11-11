import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {
  pathwayNetworkComponents,planPathwayNetworkLabels,pathwayNetworkSvg,
  pathwayNetworkGroupsTsv,buildPathwayNetwork,layoutPathwayNetwork,
} from '../pathway-network.js'

const source=readFileSync(new URL('../pathway-network.js',import.meta.url),'utf8')
const report={mode:'ranked',source:'exploratory',results:[
  {pathway:'Alpha',description:'a',matchedSetSize:3,leadingGenes:['X','Y'],q_value:.01,p_value:.01,normalizedScore:2.2},
  {pathway:'Beta',description:'b',matchedSetSize:3,leadingGenes:['Y','Z'],q_value:.02,p_value:.01,normalizedScore:-1.8},
  {pathway:'Gamma',description:'c',matchedSetSize:2,leadingGenes:['U'],q_value:.04,p_value:.02,normalizedScore:1.2},
]}
const sets={pathways:[
  {name:'Alpha',description:'a',genes:['X','Y','Z']},
  {name:'Beta',description:'b',genes:['Y','Z','V']},
  {name:'Gamma',description:'c',genes:['U','V']},
]}
const input={rows:['X','Y','Z','V','U'].map(gene=>({gene,score:.3}))}
const config={genes:'matched',maxPathways:3,minCommon:1,minSimilarity:0}

function intersects(a,b){return a.x<b.x+b.w&&a.x+a.w>b.x&&a.y<b.y+b.h&&a.y+a.h>b.y}

test('P5.2: overlap groups come from visible edges and report shared measured genes only',()=>{
  const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,{...config,minCommon:2},input))
  const groups=pathwayNetworkComponents(graph)
  assert.deepEqual(groups.map(g=>g.pathways.length),[2,1])
  assert.deepEqual(groups[0].sharedGenes,['Y','Z'])
  assert.deepEqual(groups[1].sharedGenes,[])
  assert.equal(groups[0].edgeCount,1)
  assert.equal(groups[1].edgeCount,0)
  const exported=pathwayNetworkGroupsTsv(graph)
  assert.match(exported,/group\tpathway_count\tdisplayed_gene_nodes/)
  assert.match(exported,/Alpha;Beta\tY;Z/)
  assert.equal(report.results[0].q_value,.01)
})

test('P5.2: bipartite groups follow displayed gene links, not hidden GMT membership',()=>{
  const c={...config,view:'bipartite',hideSingleGenes:true,maxGenes:1}
  const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,c,input))
  const groups=pathwayNetworkComponents(graph)
  const overlap=groups.find(g=>g.pathways.length===2)
  assert.ok(overlap)
  assert.equal(overlap.displayedGenes,1)
  assert.deepEqual(overlap.sharedGenes,['Y'])
  assert.ok(groups.some(g=>g.pathways.length===1))
  assert.equal(report.results.length,3)
})

test('P5.2: labels avoid each other, other node circles and fixed-viewport edges',()=>{
  const graph={width:1000,height:570,view:'overlap',nodes:Array.from({length:60},(_,i)=>({
    id:`p${i}`,name:`A long descriptive pathway title ${i}`,type:'pathway',
    x:490+(i%10)*7,y:260+Math.floor(i/10)*8,q_value:.001+i*.0001,
  }))}
  const before=JSON.stringify(graph)
  const labels=planPathwayNetworkLabels(graph)
  for(const label of labels){
    assert.ok(label.box.x>=4&&label.box.x+label.box.w<=996)
    assert.ok(label.box.y>=4&&label.box.y+label.box.h<=566)
  }
  for(let i=0;i<labels.length;i++)for(let j=i+1;j<labels.length;j++){
    assert.equal(intersects(labels[i].box,labels[j].box),false)
  }
  assert.ok(labels.length<=6)
  assert.equal(JSON.stringify(graph),before)
})

test('P5.2: selection gets label priority and zoom can expose more labels when space permits',()=>{
  const nodes=Array.from({length:15},(_,i)=>({id:`p${i}`,name:`Pathway ${i}`,type:'pathway',x:50+i*55,y:280,q_value:.01+i*.001,genes:[]}))
  const graph={width:1000,height:570,nodes,edges:[],view:'overlap'}
  const base=planPathwayNetworkLabels(graph,{selectedId:'p12'})
  assert.ok(base.some(label=>label.nodeId==='p12'))
  const zoomed=planPathwayNetworkLabels(graph,{view:{cx:500,cy:285,k:2}})
  assert.ok(zoomed.length>=base.length-1)
  const svg=pathwayNetworkSvg(graph,{selectedId:'p12'})
  assert.match(svg,/class="pathway-network-labels"/)
  assert.match(svg,/pointer-events="none"/)
  assert.match(svg,/data-network-node="p12"/)
})

test('P5.2: component focus dims only display elements and preserves graph geometry and values',()=>{
  const graph=layoutPathwayNetwork(buildPathwayNetwork(report,sets,{...config,minCommon:2},input))
  const before=JSON.stringify(graph)
  const group=pathwayNetworkComponents(graph)[0]
  const svg=pathwayNetworkSvg(graph,{visibleNodeIds:group.nodeIds})
  assert.match(svg,/opacity="\.18"/)
  assert.match(svg,/stroke-opacity="\.07"|stroke-opacity="\.34"/)
  assert.equal(JSON.stringify(graph),before)
})

test('P5.3: legend and connected group summary stay in expanded dialog with the active SVG',()=>{
  assert.match(source,/class="pathway-network-legend" aria-label="Network symbols explained"/)
  assert.match(source,/class="pathway-network-components"/)
  assert.match(source,/id="pathway-network-group-select"/)
  assert.match(source,/data-network-group-path/)
  assert.match(source,/\.pathway-network-legend','\.pathway-network-components'/)
  assert.match(source,/id="pathway-network-export-groups"/)
  assert.match(source,/pathwayNetworkGroupsTsv\(graph\)/)
  assert.match(source,/showGroup\(\);paint\(\)/)
})

test('P5.4: labels and summaries are descriptive; engine and source data are untouched',()=>{
  assert.doesNotMatch(source,/\bfetch\s*\(|XMLHttpRequest|WebSocket|Math\.random\(/)
  assert.doesNotMatch(source,/runPathwayEnrichment|pathwayPvalue|updateReportStatistics/)
  assert.match(source,/They are not independently tested biological modules/)
  assert.match(source,/shared measured genes/)
})

test('P5.4: malicious pathway or gene text is escaped in SVG, group controls and exports',()=>{
  const g={...report,results:[{...report.results[0],pathway:'<script>bad</script>',description:'a'}]}
  const s={pathways:[{name:'<script>bad</script>',description:'a',genes:['X','Y','Z']}]}
  const graph=layoutPathwayNetwork(buildPathwayNetwork(g,s,{...config,maxPathways:2},input))
  assert.match(pathwayNetworkSvg(graph),/&lt;script&gt;bad&lt;\/script&gt;/)
  const tsv=pathwayNetworkGroupsTsv(graph)
  assert.match(tsv,/<script>bad<\/script>/) // TSV data is literal, not HTML
})
