/** B5: Read-only, bounded scientific evidence for an explicitly approved local Ollama prompt.
 * Deliberately excludes observations, subject IDs, metadata rows, raw files and all source paths.
 * No method here runs analysis, changes p/q values, or makes requests.
 */
const str = (value, max=140) => typeof value === 'string' ? value.trim().slice(0,max) : '';
const finite = value => typeof value === 'number' && Number.isFinite(value) ? value : null;
const count = value => Number.isSafeInteger(value) && value >= 0 ? value : null;
const probability = value => { const n=finite(value); return n!==null && n>=0 && n<=1?n:null; };
const id = value => str(value,65).replace(/[\r\n\t]/g,' ');
const boundedGenes = genes => Array.isArray(genes) ? [...new Set(genes.filter(g=>typeof g==='string').map(g=>id(g)).filter(Boolean))].slice(0,12) : [];

export const AI_EVIDENCE_SCOPES = Object.freeze([
  {id:'pathways',label:'Pathway results',description:'Pathway names, original scores, BH q-values, and a few leading genes'},
  {id:'joint',label:'Joint sample structure',description:'Aggregate cohort coverage and PCoA diagnostics; no sample or patient IDs'},
  {id:'compare',label:'Corrected group comparison',description:'Aggregate counts and leading corrected gene results; no sample records'},
  {id:'all',label:'All available evidence',description:'A bounded combination of these summaries'},
]);

function pathwayRecord(row){
  if(!row || typeof row!=='object') return null;
  const pathway=str(row.pathway,180);
  if(!pathway) return null;
  return {
    pathway,
    identifier:id(row.description),
    p_value:probability(row.p_value),
    bh_q_value:probability(row.q_value),
    matched_genes:count(row.matchedSetSize),
    selected_or_overlapping_genes:count(row.overlap),
    normalized_enrichment_score:finite(row.normalizedScore),
    raw_enrichment_score:finite(row.enrichmentScore),
    fold_enrichment:finite(row.foldEnrichment),
    rank_direction:['positive','negative'].includes(row.direction)?row.direction:null,
    leading_or_selected_genes:boundedGenes(row.leadingGenes),
  };
}

export function pathwayEvidence(report, selectedKey=''){
  if(!report || !Array.isArray(report.results)) return null;
  const sorted=report.results.filter(row=>row && typeof row.pathway==='string')
    .slice().sort((a,b)=>(probability(a.q_value)??Infinity)-(probability(b.q_value)??Infinity)
      ||(probability(a.p_value)??Infinity)-(probability(b.p_value)??Infinity)
      ||String(a.pathway).localeCompare(String(b.pathway)));
  const withQ=sorted.filter(r=>probability(r.q_value)!==null);
  const bestQ=withQ.length?probability(withQ[0].q_value):null;
  const bestQTies=bestQ===null?null:withQ.filter(r=>probability(r.q_value)===bestQ).length;
  const selected=selectedKey&&selectedKey.length<=5000
    ?sorted.find(r=>`${r.pathway}\u0000${r.description??''}`===selectedKey):null;
  return {
    evidence_level:report.source==='exploratory'?'exploratory_post_fit':'predefined_comparison_not_independent_validation',
    method:['ora','ranked'].includes(report.mode)?report.mode:'unverified',
    input_gene_ranking:str(report.rankMetric||report.sourceMethod,150),
    tested_pathway_sets:count(report.matchedSetCount)??sorted.length,
    pathways_bh_q_005:withQ.filter(row=>row.q_value<=0.05).length,
    scored_gene_background:count(report.backgroundGenes),
    organism_recorded:Boolean(str(report.organism)),
    gene_set_release_recorded:Boolean(str(report.geneSetRelease)),
    gene_set_source_type:report.geneSetSource?'user_selected_or_downloaded_GMT':'unknown',
    permutations:report.mode==='ranked'?count(report.options?.permutations):null,
    minimum_empirical_p:report.mode==='ranked'&&count(report.options?.permutations)?1/(report.options.permutations+1):null,
    smallest_bh_q_value:bestQ,
    number_tied_at_smallest_bh_q:bestQTies,
    ranking_rule:'BH q ascending, permutation p ascending, pathway name ascending. Equal q-values are ties, not independently distinct discoveries.',
    top_pathways:sorted.slice(0,15).map(pathwayRecord).filter(Boolean),
    selected_pathway:selected?pathwayRecord(selected):null,
    warning:report.source==='exploratory'
      ?'These scores were calculated from post-fit joint PCoA gene associations. PCoA axis sign is arbitrary. They do not establish clinical groups, disease causality or independent validation.'
      :'Enrichment follows saved corrected contrasts, but does not independently validate the comparison or establish causality.',
  };
}

export function jointEvidence(report){
  if(!report || typeof report!=='object')return null;
  const cov=report.coverage||{};
  const pcoa=report.pcoa||{};
  const modalities=Array.isArray(report.fixed_modalities)
    ?report.fixed_modalities.filter(m=>typeof m==='string').map(m=>id(m)).slice(0,7):[];
  return {
    retained_observations:Array.isArray(report.observations)?report.observations.length:null,
    excluded_for_missing_distances:count(cov.removed_for_missing_pairs),
    fixed_view_pair_coverage:finite(cov.pair_coverage),
    positive_pcoa_axes:count(pcoa.positive_axes),
    negative_pcoa_axes:count(pcoa.negative_axes),
    negative_inertia_ratio:finite(pcoa.negative_inertia_ratio),
    selected_modalities:modalities,
    warning:'PCoA and unsupervised clusters describe the fitted cohort. No patient names, sample identifiers, clinical subgroup labels or independent validation are included.',
  };
}
export function comparisonEvidence(report){
  if(!report || typeof report!=='object')return null;
  const tests=Array.isArray(report.gene?.analysis?.tests)?report.gene.analysis.tests:[];
  const withQ=tests.filter(t=>probability(t?.q_value)!==null);
  const sorted=withQ.slice().sort((a,b)=>a.q_value-b.q_value||String(a.feature).localeCompare(String(b.feature)));
  return {
    corrected_gene_tests:tests.length,
    corrected_gene_hits_bh_q_005:withQ.filter(t=>t.q_value<=0.05).length,
    top_gene_features:sorted.slice(0,10).map(t=>({gene:id(t.feature),bh_q_value:probability(t.q_value),signed_group_effect:finite(t.beta_group)}))
      .filter(t=>t.gene),
    warning:'Gene q-values refer to the corrected Compare Groups method. A selected gene is not a validated biomarker. Sample identifiers and the contrast label are not included.',
  };
}

export function availableAiEvidence(reports={}){
  const found=[];
  if(reports.pathway && Array.isArray(reports.pathway.results) && reports.pathway.results.length)found.push('pathways');
  if(reports.joint)found.push('joint');
  if(reports.compare)found.push('compare');
  return found;
}
export function createAiEvidence(reports={},scope='pathways'){
  if(!AI_EVIDENCE_SCOPES.some(s=>s.id===scope))throw Error('Unknown evidence selection');
  const data={schema:'4c-ai-scientific-evidence-v1',approved_source:'4C Insight in-memory calculated results',
    scope,independent_validation:'not established by this context'};
  if(scope==='pathways'||scope==='all'){
    const p=pathwayEvidence(reports.pathway,reports.selectedPathwayKey);
    if(p)data.pathways=p;
  }
  if(scope==='joint'||scope==='all'){
    const j=jointEvidence(reports.joint);if(j)data.joint=j;
  }
  if(scope==='compare'||scope==='all'){
    const c=comparisonEvidence(reports.compare);if(c)data.corrected_comparison=c;
  }
  // Keep previews and prompts bounded even when custom GMT identifiers are unusually long.
  if(JSON.stringify(data,null,2).length>10500 && data.pathways){
    for(const row of data.pathways.top_pathways)row.leading_or_selected_genes=row.leading_or_selected_genes.slice(0,4);
    if(data.pathways.selected_pathway)data.pathways.selected_pathway.leading_or_selected_genes=data.pathways.selected_pathway.leading_or_selected_genes.slice(0,4);
    while(data.pathways.top_pathways.length>1 && JSON.stringify(data,null,2).length>10500)data.pathways.top_pathways.pop();
    data.pathways.preview_limited=true;
  }
  return data;
}

export function previewAiEvidence(reports={},scope='pathways'){
  const evidence=createAiEvidence(reports,scope);
  // Avoid unexpected prompt bloat. Context is assembled from fixed typed fields with bounded arrays.
  const json=JSON.stringify(evidence,null,2);
  if(json.length>12500)throw Error('The selected evidence is too large to send. Choose a narrower evidence section.');
  return json;
}
