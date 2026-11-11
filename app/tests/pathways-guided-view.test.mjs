import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const read = name => readFileSync(new URL('../' + name, import.meta.url), 'utf8')
const main = read('main.js')
const next = read('biological-next-steps.js')
const pathway = read('pathway-view.js')
const styles = read('styles.css')

// Keep tests concerned with navigation and presentation. Statistical engine tests
// continue independently in pathway-enrichment.test.mjs.
test('Integrative Analysis has three accessible secondary views and one Pathways panel', () => {
  assert.match(main, /id="integrative-view-overview"/)
  assert.match(main, /id="integrative-view-details"/)
  assert.match(main, /id="integrative-view-pathways"[^>]*role="tab"[^>]*aria-controls="integrative-pathways-panel"/)
  assert.match(main, /role="tabpanel" id="integrative-pathways-panel"/)
  assert.match(main, /pathways\?integrativePathwaysPanel\(\):detail\?integrativeDetailsPanel\(\):integrativeOverviewPanel\(\)/)
  assert.equal((main.match(/id="pathway-analysis-widget"/g)||[]).length,1)
  assert.doesNotMatch(next,/id="pathway-analysis-widget"/)
})
test('Pathways view restores without visiting Overview or fitting the joint model again', () => {
  assert.match(main,/\['overview', 'details', 'pathways'\]\.includes\(saved\.integrativeView\)/)
  assert.match(main,/integrativeView: state\.integrativeView/)
  const section=main.slice(main.indexOf('function pathwaysSavedGeneRows()'),main.indexOf('function integrativeDetailsPanel()'))
  assert.match(section,/biologicalCandidates\(state\.bioFeatureResult, null\)/)
  assert.doesNotMatch(section,/runCompleteIntegrativeAnalysis|runPathwayEnrichment|currentBiologicalNextRows/)
  assert.match(main,/exploratoryRows:pathwaysSavedGeneRows\(\)/)
})
test('Overview routes to Pathways and preserves feature exports and method audit', () => {
  assert.match(next,/data-bio-next="pathways"/)
  assert.match(next,/data-bio-next="export-genes"/)
  assert.match(next,/data-bio-next="export-audit"/)
  assert.match(main,/if\(action==='pathways'\)\{\s*state\.integrativeView='pathways';render\(\)/)
  assert.match(main,/pathwayResult/)
})
test('Pathway panel has exactly three ordered guided steps with original local controls', () => {
  const steps=['pathway-step-question','pathway-step-genesets','pathway-step-run']
  const positions=steps.map(id=>pathway.indexOf(`id="${id}"`))
  assert.ok(positions.every(n=>n>=0),`Missing step heading: ${positions}`)
  assert.ok(positions[0]<positions[1] && positions[1]<positions[2])
  for(const id of ['pathway-source','pathway-mode','pathway-axis','pathway-default','pathway-gmt','pathway-organism','pathway-release','pathway-run','pathway-stop','pathway-export','pathway-audit']){
    assert.ok(pathway.includes(`id="${id}"`),`Missing control ${id}`)
  }
  assert.match(pathway,/Advanced settings · thresholds, pathway sizes and permutations/)
  assert.match(pathway,/canRun=nSets>0&&ready&&compatible/)
})
test('Exploratory ranking is labelled, unsupported ORA is blocked, and comparisons are user-selected', () => {
  assert.match(pathway,/PCoA gene rankings are post-fit/)
  assert.match(pathway,/ORA requires corrected Compare Groups results/)
  assert.match(pathway,/c\.mode!=='ora'\|\|c\.source==='compare'/)
  assert.match(main,/Run Compare Groups for a predefined case\/control analysis/)
  assert.doesNotMatch(main.slice(main.indexOf('function integrativePathwaysPanel()'),main.indexOf('function integrativeDetailsPanel()')),/clusterAsDisease|fitNewModel/)
})
test('Pathway runs block active-session save and preserve a final save after work ends', () => {
  assert.match(main,/state\.pathwayBusy/)
  assert.match(pathway,/workspace\.pathwayBusy=true/)
  assert.match(pathway,/workspace\.pathwayBusy=false\s*onChanged\?\.\(\)/)
  assert.match(main,/pathwayGeneSets', 'pathwayResult'/)
})
test('Dedicated layout is responsive and does not introduce a native helper or remote analysis', () => {
  assert.match(styles,/\.pathway-guided-step/)
  assert.match(styles,/@media \(max-width: 640px\)/)
  assert.doesNotMatch(pathway,/\/api\/pathway|XMLHttpRequest|sendBeacon|WebSocket/)
  assert.match(pathway,/Download &amp; use default Reactome/)
})
