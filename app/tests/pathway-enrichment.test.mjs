import test from 'node:test'
import assert from 'node:assert/strict'
import {readFileSync} from 'node:fs'
import {parseGeneSetsGmt,preparePathwayGenes,hypergeometricTail,bhAdjust,enrichmentScore,runPathwayEnrichment,pathwayInputSignature,pathwayResultsTsv} from '../pathway-enrichment.js'
import {pathwayPanelHtml,pathwayResultsHtml,defaultPathwayConfig} from '../pathway-view.js'
const rna = Array.from({length:40},(_,i)=>({feature:`G${i+1}`,beta_group:40-i,q_value:i<8?0.001:0.95,p_value:i<8?0.0001:0.9}))
const compare={comparisonName:'Case vs Control',gene:{analysis:{tests:rna}}}
const gmt=['Top genes\tR-HSA-1\tReactome Pathway\t'+Array.from({length:10},(_,i)=>`G${i+1}`).join('\t'),
  'Bottom genes\tR-HSA-2\tReactome Pathway\t'+Array.from({length:10},(_,i)=>`G${i+31}`).join('\t'),
  'Mixed\tcustom\tG2\tG11\tG20\tG32\tG35'].join('\n')
const sets=parseGeneSetsGmt(gmt,'Reactome.gmt')
test('GMT parser reads Reactome third metadata column and standard GMT, without invented mapping',()=>{
  assert.equal(sets.imported,3)
  assert.deepEqual(sets.pathways[0].genes,['G1','G2','G3','G4','G5','G6','G7','G8','G9','G10'])
  assert.deepEqual(sets.pathways[2].genes,['G2','G11','G20','G32','G35'])
  const duplicate=parseGeneSetsGmt('A\tdesc\tx\tX\ty\nA\tdesc\tz\nB\tdesc\tZ')
  assert.equal(duplicate.imported,2)
  assert.equal(duplicate.invalidLines,1)
  assert.deepEqual(duplicate.pathways[0].genes,['X','Y'])
  assert.throws(()=>parseGeneSetsGmt('A\tmissing'),/No valid gene sets/)
})
test('fixed compare background excludes duplicate identifiers rather than guessing and preserves provenance',()=>{
  const x=preparePathwayGenes({compare:{comparisonName:'Clinical',gene:{analysis:{tests:[...rna,{feature:'g1',beta_group:5,q_value:.02}]}}},source:'compare'})
  assert.equal(x.rows.length,39)
  assert.equal(x.diagnostics.duplicateRows,2)
  assert.ok(!x.rows.some(r=>r.gene==='G1'))
  assert.equal(x.provenance,'Clinical')
  assert.throws(()=>preparePathwayGenes({source:'compare'}),/Run corrected/)
})
test('hypergeometric upper tail matches exact enumerated probability',()=>{
  // N=10; 4 set genes; 3 selected; overlap at least 2 -> 0.3333333.
  assert.ok(Math.abs(hypergeometricTail(10,4,3,2)-(
    (6*6+4*1)/120))<1e-12)
  assert.equal(hypergeometricTail(20,5,2,0),1)
  assert.throws(()=>hypergeometricTail(5,7,2,1),/Invalid/)
})
test('Benjamini Hochberg monotonic q values across all tested sets',()=>{
  const rows=[{p_value:.04},{p_value:.01},{p_value:.03},{p_value:NaN}]
  bhAdjust(rows)
  assert.ok(Math.abs(rows[0].q_value-.04)<1e-12)
  assert.ok(Math.abs(rows[1].q_value-.03)<1e-12)
  assert.ok(Math.abs(rows[2].q_value-.04)<1e-12)
  assert.equal(rows[3].q_value,undefined)
})
test('ORA uses all tested genes, not just pathway-contained genes, and computes real q values',async()=>{
  const input=preparePathwayGenes({compare,source:'compare'})
  const res=await runPathwayEnrichment({input,geneSets:sets,options:{mode:'ora',minSize:3,maxSize:20,qThreshold:.01,effectThreshold:1}})
  assert.equal(res.backgroundGenes,40)
  assert.equal(res.selectedGenes,8)
  assert.equal(res.results.length,3)
  assert.equal(res.results.find(r=>r.pathway==='Top genes').overlap,8)
  assert.equal(res.results.find(r=>r.pathway==='Bottom genes').overlap,0)
  assert.ok(res.results.find(r=>r.pathway==='Top genes').q_value<.05)
  assert.equal(res.source,'compare')
  assert.ok(!res.warnings.some(x=>x.includes('PCoA ranking')))
  assert.match(pathwayResultsTsv(res),/Top genes/)
  await assert.rejects(()=>runPathwayEnrichment({input,geneSets:sets,options:{mode:'ora',minSize:50,maxSize:100}}),/No gene sets/)
})
test('ORA can select higher, lower, or both directions using the corrected log2 group effect',async()=>{
  const tests=rna.map((r,i)=>({...r,beta_group:i<8?(i%2?2:-2):0}))
  const input=preparePathwayGenes({compare:{gene:{analysis:{tests}}},source:'compare'})
  const opts={mode:'ora',minSize:3,maxSize:20,qThreshold:.05,effectThreshold:1}
  const both=await runPathwayEnrichment({input,geneSets:sets,options:{...opts,direction:'both'}})
  const up=await runPathwayEnrichment({input,geneSets:sets,options:{...opts,direction:'up'}})
  const down=await runPathwayEnrichment({input,geneSets:sets,options:{...opts,direction:'down'}})
  assert.equal(both.selectedGenes,8)
  assert.equal(up.selectedGenes,4)
  assert.equal(down.selectedGenes,4)
  assert.equal(up.options.direction,'up')
})
test('ranked GSEA weighted KS identifies top and bottom and is repeatable with fixed seed',async()=>{
  const input=preparePathwayGenes({compare,source:'compare'}),options={mode:'ranked',minSize:3,maxSize:20,permutations:199,seed:134}
  const a=await runPathwayEnrichment({input,geneSets:sets,options})
  const b=await runPathwayEnrichment({input,geneSets:sets,options})
  assert.deepEqual(a.results,b.results)
  assert.equal(a.results.find(x=>x.pathway==='Top genes').direction,'positive')
  assert.equal(a.results.find(x=>x.pathway==='Bottom genes').direction,'negative')
  assert.ok(a.results.find(x=>x.pathway==='Top genes').p_value>=1/200)
  assert.ok(a.results.every(x=>x.q_value>=0&&x.q_value<=1))
  assert.match(a.correction,/not phenotype permutations/)
  assert.deepEqual(enrichmentScore([0,1],[2,1,0,-1]),{score:1,direction:'positive',peak:1})
})
test('exploratory PCoA has statistical output but cannot run clinical ORA',async()=>{
  const rows=rna.map(row=>({kind:'gene',feature:row.feature,pcoa1_spearman:row.beta_group/40,pcoa2_spearman:-(row.beta_group/40)}))
  const input=preparePathwayGenes({source:'exploratory',axis:'pcoa1_spearman',exploratoryRows:rows})
  assert.equal(input.evidence.includes('Exploratory'),true)
  await assert.rejects(()=>runPathwayEnrichment({input,geneSets:sets,options:{mode:'ora',minSize:3,maxSize:20}}),/ORA requires corrected/)
  const result=await runPathwayEnrichment({input,geneSets:sets,options:{mode:'ranked',minSize:3,maxSize:20,permutations:99}})
  assert.ok(result.warnings.some(w=>/in-sample and post-fit/.test(w)))
  assert.equal(result.source,'exploratory')
  assert.notEqual(pathwayInputSignature(input,sets),pathwayInputSignature({...input,rows:input.rows.slice(1)},sets))
})
test('pathway HTML escapes user-controlled names and has no automatic data upload',()=>{
  const evil=parseGeneSetsGmt('A\tbad\tG1\tG2','<img src=x onerror=alert(1)>.gmt')
  const html=pathwayPanelHtml({sets:evil,config:defaultPathwayConfig(),compareAvailable:true})
  assert.match(html,/&lt;img/) // File name / tested path may also be malicious
  assert.doesNotMatch(html,/<img src=x onerror/)
  assert.match(html,/No gene sets imported|1 gene sets/)
  assert.match(html,/Reactome/)
  const results=pathwayResultsHtml({mode:'ora',source:'compare',provenance:'<script>x</script>',geneSetFile:'a.gmt',method:'ORA',correction:'BH',backgroundGenes:40,results:[{pathway:'<img src=x>',description:'x',q_value:.1,p_value:.2,overlap:3,matchedSetSize:10,leadingGenes:['G1']} ]})
  assert.doesNotMatch(results,/<img src=x>/)
  assert.match(results,/&lt;script&gt;/)
  for(const path of ['pathway-view.js','pathway-enrichment.js']) {
    const source=readFileSync(new URL('../'+path,import.meta.url),'utf8')
    assert.doesNotMatch(source,/\bfetch\s*\(|XMLHttpRequest|WebSocket|sendBeacon|\/api\//)
  }
})
test('integrative page mounts the module, persists local results, exports and protects science state',()=>{
  const main=readFileSync(new URL('../main.js',import.meta.url),'utf8')
  const next=readFileSync(new URL('../biological-next-steps.js',import.meta.url),'utf8')
  for(const term of ['mountPathwayModule(document','pathwayGeneSets',"'pathwayResult'",'pathwayConfig','saveLocalResult(key, token, value)'] .slice(0,6))assert.ok(main.includes(term),term)
  assert.ok(next.includes('id="pathway-analysis-widget"'))
  assert.ok(main.includes("if(action==='pathways')"))
  assert.ok(main.includes('state.pathwayResult?{status:\'calculated\''))
})
