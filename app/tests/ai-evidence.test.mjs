import test from 'node:test';
import assert from 'node:assert/strict';
import {availableAiEvidence,createAiEvidence,previewAiEvidence} from '../ai-evidence.js';
import {promptMessages} from '../ai-runtime.js';
const fixture=()=>({
  pathway:{mode:'ranked',source:'exploratory',matchedSetCount:1713,backgroundGenes:21789,
    options:{permutations:199},rankMetric:'Post-fit PCoA2 Spearman association',
    geneSetFile:'PRIVATE/path/ReactomePathways.gmt',geneSetRelease:'',organism:'',
    results:[
      {pathway:'ABC transporter disorders',description:'R-HSA-5619084',p_value:0.005,q_value:0.018,matchedSetSize:64,overlap:64,normalizedScore:2.570,enrichmentScore:0.632,leadingGenes:['ABCA1','ABCB1','ABCC2']},
      {pathway:'ABC-family protein mediated transport',description:'R-HSA-382556',p_value:0.005,q_value:0.018,matchedSetSize:89,overlap:89,normalizedScore:2.333,enrichmentScore:0.547,leadingGenes:['ABCA1','ABCB1']},
      {pathway:'Secondary pathway',description:'R-HSA-3',p_value:0.021,q_value:0.035,matchedSetSize:17,normalizedScore:-1.2,leadingGenes:['ABCG1']},
    ]},
  selectedPathwayKey:'ABC-family protein mediated transport\u0000R-HSA-382556',
  joint:{observations:['PAT-PRIVATE-001','PAT-PRIVATE-002'],fixed_modalities:['gene','transcript'],coverage:{removed_for_missing_pairs:85,pair_coverage:0.93},pcoa:{positive_axes:2,negative_axes:1,negative_inertia_ratio:0.02},rawFiles:['SECRET.raw']},
  compare:{comparisonName:'PRIVATE PATIENT 002',gene:{analysis:{tests:[
    {feature:'IFI27',q_value:0.0004,beta_group:1.25,sample_id:'PAT-PRIVATE-001'},
    {feature:'OAS1',q_value:0.005,beta_group:-0.9},
    {feature:'DNAH9',q_value:0.4,beta_group:0.1},
  ]}}},
  secret:'HIGHLY_PRIVATE',
});

test('exact measured pathway names, scores, q-value ties, selected pathway, no invented report fields',()=>{
  const data=createAiEvidence(fixture(),'pathways').pathways;
  assert.equal(data.tested_pathway_sets,1713);
  assert.equal(data.scored_gene_background,21789);
  assert.equal(data.smallest_bh_q_value,0.018);
  assert.equal(data.number_tied_at_smallest_bh_q,2);
  assert.equal(data.top_pathways[0].pathway,'ABC transporter disorders');
  assert.equal(data.top_pathways[0].normalized_enrichment_score,2.570);
  assert.equal(data.top_pathways[0].bh_q_value,0.018);
  assert.equal(data.selected_pathway.pathway,'ABC-family protein mediated transport');
  assert.equal(data.selected_pathway.leading_or_selected_genes[0],'ABCA1');
  assert.equal(data.evidence_level,'exploratory_post_fit');
  assert.equal(data.minimum_empirical_p,0.005);
  assert.equal(data.organism_recorded,false);
  assert.equal(data.gene_set_release_recorded,false);
  assert.doesNotMatch(JSON.stringify(data),/PRIVATE\/path|PAT-PRIVATE|SECRET\.raw/);
});
test('no raw samples, patient identifiers, filenames, or arbitrary report fields in joint/compare/all',()=>{
  const data=previewAiEvidence(fixture(),'all');
  assert.match(data,/"retained_observations": 2/);
  assert.match(data,/"corrected_gene_hits_bh_q_005": 2/);
  assert.match(data,/"gene": "IFI27"/);
  assert.doesNotMatch(data,/PAT-PRIVATE|PRIVATE PATIENT|SECRET\.raw|HIGHLY_PRIVATE|PRIVATE\/path/);
  assert.doesNotMatch(data,/"observations"|"rawFiles"|"sample_id"|"comparisonName"/);
});
test('unavailable data stays unavailable; no guessed values or forced numeric zeros',()=>{
  assert.deepEqual(availableAiEvidence({}),[]);
  const data=createAiEvidence({},'pathways');
  assert.ok(!Object.hasOwn(data,'pathways'));
  assert.throws(()=>createAiEvidence({},'untrusted'),/Unknown evidence/);
});
test('real bounded evidence reaches model prompt when approved; not otherwise',()=>{
  const data=createAiEvidence(fixture(),'pathways');
  const included=promptMessages('Which pathway is most significant?',[],data).at(-1).content;
  assert.match(included,/ABC transporter disorders/);
  assert.match(included,/"number_tied_at_smallest_bh_q":2/);
  assert.match(included,/0\.018/);
  const excluded=promptMessages('Which pathway is most significant?').at(-1).content;
  assert.doesNotMatch(excluded,/ABC transporter disorders|0\.018/);
  assert.match(promptMessages('Hi')[0].content,/4C Insight is the application name/);
});
test('bounded top pathways, leading genes and other sections',()=>{
  const f=fixture();
  f.pathway.results=Array.from({length:3000},(_,i)=>({pathway:`Pathway ${i}`,description:`R-${i}`,p_value:0.005,q_value:0.01+i/100000,leadingGenes:Array.from({length:300},(_,n)=>`G${n}`)}));
  assert.equal(createAiEvidence(f,'pathways').pathways.top_pathways.length,15);
  assert.equal(createAiEvidence(f,'pathways').pathways.top_pathways[0].leading_or_selected_genes.length,12);
  assert.ok(previewAiEvidence(f,'pathways').length<12500);
});
test('unusually long but valid GMT names never crash the evidence preview',()=>{
  const f=fixture();
  f.pathway.results=Array.from({length:40},(_,i)=>({
    pathway:'P'.repeat(150)+i,description:'R'.repeat(60)+i,
    p_value:0.005,q_value:0.018,normalizedScore:1.1,
    leadingGenes:Array.from({length:25},(_,j)=>'GENE'+`${j}`+'X'.repeat(55)),
  }));
  const display=previewAiEvidence(f,'pathways');
  assert.ok(display.length<12500);
  assert.match(display,/"preview_limited": true/);
});
