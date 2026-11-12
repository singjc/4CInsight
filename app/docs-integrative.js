/** User-facing, browser-local Integrative Analysis and Pathways documentation.
 * Every value in the examples and diagrams is synthetic; none is read from a cohort.
 * No scientific calculation is run by this module.
 */
const ref = (label, href) => ({label, href})
const bh = ref('Benjamini & Hochberg (1995), Controlling the false discovery rate', 'https://doi.org/10.1111/j.2517-6161.1995.tb02031.x')
const gsea = ref('Subramanian et al. (2005), Gene set enrichment analysis', 'https://doi.org/10.1073/pnas.0506580102')
const reactome = ref('Jassal et al. (2020), The Reactome pathway knowledgebase', 'https://doi.org/10.1093/nar/gkz1031')
const pcoa = ref('Gower (1966), Some distance properties of latent root and vector methods', 'https://doi.org/10.1093/biomet/53.3-4.325')
const perm = ref('Anderson (2001), Non-parametric multivariate analysis of variance', 'https://doi.org/10.1111/j.1442-9993.2001.01070.pp.x')
const selection = ref('Ambroise & McLachlan (2002), Selection bias in gene extraction', 'https://doi.org/10.1073/pnas.102102699')
const sandve = ref('Sandve et al. (2013), Reproducible computational research', 'https://doi.org/10.1371/journal.pcbi.1003285')

export const integrativeDocs = [
  {
    id: 'integrative-workflow',
    title: 'Integrative Analysis: run all selected data types',
    keywords: 'integrative analysis multiomics seven modalities workflow complete analysis observation patient sample timepoint BCR TCR gene transcript immune',
    summary: 'Run one local workflow across the selected modalities, then read Overview, Details, and Pathways.',
    body: [
      'Open Integrative Analysis. Select the metadata source, metadata file, Observation ID, Patient ID, and optional Visit / timepoint column. Choose from gene expression, transcript expression, BCR repertoire, TCR alpha, TCR beta, BCR isotypes, and immune infiltration. Select Run complete analysis.',
      'Observation ID identifies one measured row. Patient ID identifies the person across visits. The same column can fill both roles only when each patient has one observation. For repeated visits, select a separate Patient ID column. The optional one-observation-per-patient setting changes the fitted cohort.',
      'The application aligns observations, calculates measured source distances, builds the joint distance structure, fits the PCoA/clusters, checks patient-level cluster stability, and summarizes measured feature associations. An unavailable data type can be excluded. These calculations stay in the browser.',
      'Overview gives the main findings and sample coverage. Details gives distance, stability, method, and export information. Pathways runs a separate gene-set enrichment analysis from available gene rankings; selecting the Pathways tab does not refit the joint model.',
    ],
    references: [sandve, pcoa],
  },
  {
    id: 'integrative-coverage',
    title: 'Joint-model coverage, missing data, and cluster stability',
    keywords: 'coverage retained omitted missing fixed distance eligible observations per-pair complete case sample alignment small cluster singleton ARI',
    summary: 'Understand why a sample is missing and why a stable cluster is not a clinical subtype.',
    body: [
      'Start with the eligible, retained, and omitted observation counts. Expand source coverage and the omitted-observation table. A source file can be present even when the pairwise distances needed by the joint structure are incomplete.',
      'Minimum data types per pair controls which source distances can contribute to a pair. The complete-distance clustering step still needs the joint pairwise distance structure required by that fit. Thus, an observation can pass source matching but be left out of the fitted joint model.',
      'A small cluster or singleton is a diagnostic finding, not a clinical subtype and not evidence of a new disease subtype. Inspect sample metadata, batch, visit, and measurement coverage. Patient-subset adjusted Rand index describes cluster stability across repeated fitting; it does not validate disease labels.',
      'The joint cohort is selected by the method shown in Details. It is not guaranteed to be the largest possible complete cohort. Review omitted observations before interpreting the PCoA or clusters.',
    ],
    references: [pcoa, sandve],
  },
  {
    id: 'integrative-patterns',
    title: 'Joint PCoA, modality influence, and molecular features',
    keywords: 'pcoa pco1 pco2 joint distance sample node zoom labels correlations spearman sensitivity leave one modality out exploratory feature ranking',
    summary: 'Inspect the shared sample pattern without confusing a post-fit association with differential expression.',
    body: [
      'The joint PCoA is a picture of the fitted distance structure. Colour points by a selected metadata field, inspect individual observations, and zoom into dense areas. PCoA axis directions can be reversed without changing the scientific result.',
      'The modality cards show coverage, agreement with joint distances, and between/within-cluster distances. Leave-one-data-type-out sensitivity asks how the joint distances and nearest neighbours change if a selected modality is omitted. These summaries are descriptive; a high agreement is not independent evidence of biological importance.',
      'Gene, transcript, repertoire, isotype, and immune features may be ranked by Spearman association with PCoA axes. A negative association does not mean a gene is downregulated in disease. The feature scores are post-fit, can reflect technical or biological variation, and do not carry gene-level adjusted p-values unless a separate corrected group comparison supplied them.',
      'For a disease question, first define cases and controls from metadata and run Compare Groups. Do not test a gene against a cluster derived from that same gene and call the result independent validation.',
    ],
    references: [pcoa, selection, perm],
  },
  {
    id: 'integrative-next-steps',
    title: 'From joint patterns to a biological question',
    keywords: 'next biological steps clinical metadata cases controls differential groups evidence interpretation validation ranks hypotheses',
    summary: 'Use cohort diagnostics, predefined groups, and measured features to form testable hypotheses.',
    body: [
      'Use Next biological steps in the Integrative Overview. First review coverage and unusual observations. Then colour the joint plot by an existing clinical or technical metadata field.',
      'If you have a predefined case/control question, run Compare Groups with the correct measured counts and metadata. Its corrected gene results can be used for ORA or ranked enrichment. The exploratory joint PCoA feature list answers a different question and must remain labelled exploratory.',
      'The cross-modality evidence table brings measured data types together for review. A previous adjusted test may use a different sample subset or comparison. It is not automatically an independent replication of the joint structure.',
      'Follow up a pathway result by inspecting its exact matched and leading-edge genes, checking batch and missingness, and seeking a held-out or independent cohort. Save the source, thresholds, comparison, and limitations with the result.',
    ],
    references: [sandve, selection],
  },
  {
    id: 'pathway-ora',
    title: 'Pathways: over-representation analysis (ORA)',
    keywords: 'pathways over representation ORA hypergeometric overlap selected genes q-value FDR case control universe log2 effect',
    summary: 'Test whether predefined selected genes occur more often in a pathway than expected from the tested-gene background.',
    body: [
      'Open Integrative Analysis → Pathways. In Step 1 select Corrected Compare Groups genes and Over-representation (ORA). A predefined group comparison must be available. ORA cannot use an exploratory PCoA ranking as if it were a corrected differential-expression result.',
      'In Advanced settings select the feature-level BH q-value cutoff, minimum absolute log2 group effect, and effect direction (higher in cases, lower in cases, or both). The ORA gene universe is all unique usable tested genes, not only the selected genes or the genes found in GMT.',
      'For each eligible gene set, a one-sided hypergeometric test counts the overlap between selected genes and the tested-gene background. The application adjusts the pathway p-values with Benjamini–Hochberg across the tested pathways.',
      'An ORA pathway is associated with the selected comparison and cutoffs. It does not establish a mechanism. Closely related Reactome pathways can share genes and are not independent confirmations.',
    ],
    references: [bh, reactome, sandve],
  },
  {
    id: 'pathway-ranked',
    title: 'Pathways: ranked enrichment and permutation limits',
    keywords: 'ranked enrichment GSEA weighted running sum NES gene label permutation empirical p-value seed PCoA sign BH ties 199',
    summary: 'Test whether pathway genes collect near one end of a full gene ranking.',
    body: [
      'In Step 1 select Ranked gene-label permutation enrichment. The ranking can come from corrected Compare Groups results or from the exploratory PCo1/PCo2 gene association scores. These input types answer different questions and retain different evidence labels.',
      'The method calculates a weighted running-sum enrichment score (ES), then a normalized score (NES). It shuffles gene labels and uses a two-sided empirical p-value, followed by BH adjustment across tested gene sets. This is not phenotype permutation and does not reproduce the pooled FDR calculation of Broad GSEA.',
      'With 199 permutations, the smallest empirical p-value is 1/(199+1) = 0.005. Many pathways can have the same p-value and BH q-value. A high number of low-q pathways in a post-fit PCoA analysis does not confirm a disease mechanism.',
      'PCoA axes have arbitrary direction. Positive or negative NES for a PCoA ranking means association with that chosen axis direction, not higher or lower gene expression in disease. Select a pathway to inspect its leading-edge genes and original scores.',
    ],
    references: [gsea, bh, selection],
  },
  {
    id: 'pathway-network',
    title: 'Pathway networks, gene links, and result provenance',
    keywords: 'network fuzzy nodes edges cluster jaccard overlap gene pathway membership leading edge connected groups expand zoom inspector export Reactome GMT ZIP',
    summary: 'Explore gene-sharing pathway groups and inspect every network node without inventing interactions.',
    body: [
      'Step 2 can download the default Reactome GMT ZIP or import your own GMT/ZIP. Gene-set matching uses exact uppercase identifiers. The tool does not convert Ensembl IDs to gene symbols or infer aliases. Verify the organism and exact gene-set release before reporting results; the Reactome “current” URL is not a pinned version.',
      'After enrichment, the Gene-sharing network offers Pathway overlap and Genes ↔ pathways views. Pathway overlap connects two pathways when they share scored genes. Jaccard = shared / union; overlap coefficient = shared / smaller set size. The gene–pathway view connects a gene with the pathways that contain it.',
      'Network filters include the pathway q-value cutoff, number of displayed pathways, leading-edge/ORA-selected versus all matched genes, minimum shared genes, and minimum overlap score. Network positions, connected groups, and displayed nodes depend on these controls. They do not change the underlying enrichment test.',
      'Use wheel zoom, pan, box zoom, Fit nodes, Expand plot, and connected-group focus to inspect crowded areas. Select a pathway node to open its statistics and full matched-gene inspector. Select a gene node to view its connected pathways and original gene score. Export node/edge/group TSV, network JSON, and SVG for reproducible reporting.',
      'An edge means gene membership or shared gene identifiers. It is not physical interaction evidence. It does not mean a protein-protein interaction, regulation, causality, or an independently validated biological module. Save the GMT provenance, analysis method, graph settings, and the source cohort context with any figure.',
    ],
    references: [reactome, gsea, sandve],
  },
]

const makeExample = (title, question, columns, rows, steps, meaning, limit) => ({title, question, columns, rows, steps, meaning, limit})
export const integrativeToyExamples = {
  'integrative-workflow': [makeExample(
    'Use separate observation and patient IDs',
    'How do three visits from two people enter the joint analysis?',
    ['Observation ID', 'Patient ID', 'Visit', 'Measured modalities'],
    [['A_0', 'Patient A', 'Baseline', 'Gene, BCR'], ['A_1', 'Patient A', 'Month 3', 'Gene, BCR'], ['B_0', 'Patient B', 'Baseline', 'Gene, BCR']],
    ['Each observation is a measured row.', 'The two A visits share one Patient ID.', 'The optional one-observation-per-patient setting would reduce eligible repeated visits before fitting.'],
    'A joint analysis can keep separate visits while recognizing which visits belong to one patient.',
    'This is metadata alignment only. It does not infer treatment effects or patient stability.'
  )],
  'integrative-coverage': [makeExample(
    'Read eligible, retained, and omitted counts',
    'What does it mean when only four of six eligible observations are fitted?',
    ['Toy observation', 'Eligible', 'Fitted joint structure', 'Reason'],
    [['S1', 'Yes', 'Yes', 'Complete distances'], ['S2', 'Yes', 'Yes', 'Complete distances'], ['S3', 'Yes', 'Yes', 'Complete distances'], ['S4', 'Yes', 'Yes', 'Complete distances'], ['S5', 'Yes', 'No', 'Missing required pair distances'], ['S6', 'Yes', 'No', 'Missing required pair distances']],
    ['Six observations pass metadata eligibility.', 'Four have the complete joint distance structure needed for this fit.', 'Two are omitted. Review the recorded distance and source coverage before interpreting clusters.'],
    'The fitted cohort is smaller than the eligible cohort; the missingness review matters.',
    'The example does not imply that a present source workbook always produces a usable pairwise distance.'
  )],
  'integrative-patterns': [makeExample(
    'A negative PCoA correlation is not downregulation',
    'What can we say when a feature score is negative on PCo1?',
    ['Toy feature', 'PCo1 association', 'Valid interpretation'],
    [['G1', '-0.60', 'Lower score at the positive PCo1 end'], ['G2', '+0.50', 'Higher score at the positive PCo1 end']],
    ['PCo1 comes from a fitted distance structure, not a case/control effect.', 'The fitted axis can be reversed: both correlation signs then reverse.', 'Use Compare Groups to ask if a gene is higher or lower in cases.'],
    'PCoA associations help explain sample variation; they are not differential-expression tests.',
    'No gene-level q-value or clinical subtype is inferred from these two toy correlations.'
  )],
  'integrative-next-steps': [makeExample(
    'Move from an exploratory cluster to a defined comparison',
    'Which result can support a case-versus-control pathway question?',
    ['Observation', 'Clinical group', 'Joint cluster', 'Action'],
    [['A', 'Case', 'Cluster 1', 'Use case/control metadata'], ['B', 'Control', 'Cluster 1', 'Use case/control metadata'], ['C', 'Case', 'Cluster 2', 'Inspect unusual cluster, not a disease label']],
    ['Review group and batch metadata before naming clusters.', 'Choose Case and Control from recorded metadata in Compare Groups.', 'Use the corrected gene result for ORA, and validate in an independent sample set.'],
    'Predefined group results address a clinical contrast; unsupervised clusters remain descriptive.',
    'A comparison on the same observations is not independent validation of a joint model.'
  )],
  'pathway-ora': [makeExample(
    'Hypergeometric overlap, using the full tested background',
    'Are three selected genes in a six-gene pathway unusually frequent?',
    ['Quantity', 'Toy value', 'Meaning'],
    [['Background', '20 genes', 'All unique usable tested genes'], ['Selected', '5 genes', 'Meet corrected q/effect settings'], ['Pathway', '6 genes', 'Of the 20 tested genes'], ['Overlap', '3 genes', 'Selected genes in that pathway'], ['One-sided p-value', '0.1313', 'Not below 0.05 before BH correction']],
    ['Let N=20 tested, K=6 pathway, n=5 selected, k=3 overlap.', 'Add the hypergeometric probabilities for k=3,4,5: sum C(6,k)×C(14,5-k)/C(20,5).', 'The one-sided p-value is approximately 0.1313; correcting across multiple pathways cannot make this a significant toy result.'],
    'An overlap of three genes is not necessarily enriched when the tested-gene background is considered.',
    'This is an illustrative calculation, not a result from Reactome or your sample cohort.'
  )],
  'pathway-ranked': [makeExample(
    'Why low permutation p-values can tie',
    'What is the smallest p-value when the ranked analysis uses 199 gene-label permutations?',
    ['Input', 'Toy value', 'Consequence'],
    [['Gene ranking', 'G1, G2, G3, G4', 'Sorted by a gene-level association'], ['Pathway genes', 'G1, G2', 'A running score increases near top ranks'], ['Permutations', '199', 'Labels shuffled; not disease groups'], ['Smallest empirical p-value', '1/(199+1) = 0.005', 'Multiple pathways can share this value']],
    ['Compute the weighted running sum over the ranked genes.', 'Repeat after 199 label permutations using a saved seed.', 'Calculate the empirical p-value and then BH q-values across tested gene sets.'],
    'Tied pathway q-values can reflect limited permutation resolution and overlapping gene sets.',
    'PCoA ranks are post-fit. This toy does not supply a disease effect or an independent validation result.'
  )],
  'pathway-network': [makeExample(
    'Build pathway links from exact shared genes',
    'When do two pathways share an edge in a Jaccard network?',
    ['Pathway', 'Scored gene members', 'Shared with P1'],
    [['P1', 'G1, G2, G3', '—'], ['P2', 'G2, G3, G4', 'G2, G3'], ['P3', 'G5, G6', 'None']],
    ['P1 and P2 share two genes. Their union has four genes.', 'Jaccard(P1,P2)=2/4=0.50. The overlap coefficient is 2/3.', 'P3 has no shared measured genes with P1, so no link is drawn. A graph layout is descriptive only.'],
    'An edge indicates gene-set overlap, not physical interaction between gene products.',
    'The graph uses only scored genes and the selected display rules; toy node positions and groups have no inferential meaning.'
  )],
}

const COLORS = {navy:'#24334d',muted:'#65758d',blue:'#3159d9',pale:'#e2ebff',green:'#18856c',gray:'#aebccf',orange:'#a96b18',border:'#dce5f0',white:'#ffffff',bg:'#f5f8fd'}
const xml = s => String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;')
const txt = (x,y,s,fill=COLORS.navy,size=12,weight=500,anchor='start') => `<text x="${x}" y="${y}" font-family="system-ui,sans-serif" fill="${fill}" font-size="${size}" font-weight="${weight}" text-anchor="${anchor}">${xml(s)}</text>`
const dot = (x,y,r,color) => `<circle cx="${x}" cy="${y}" r="${r}" fill="${color}" stroke="white" stroke-width="2"/>`
const box = (x,y,w,h,color=COLORS.white) => `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${color}" stroke="${COLORS.border}" rx="10"/>`
const ln = (x1,y1,x2,y2,color=COLORS.gray,width=2) => `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${color}" stroke-width="${width}"/>`
function figure(id,title,description,shapes) {
  const key=id.replace(/[^a-z0-9-]/gi,'-')
  return `<div class="docs-visual-overflow"><svg class="docs-visual-svg" viewBox="0 0 680 250" role="img" aria-labelledby="visual-title-${key} visual-desc-${key}" xmlns="http://www.w3.org/2000/svg"><title id="visual-title-${key}">${xml(title)}</title><desc id="visual-desc-${key}">${xml(description)}</desc><rect width="680" height="250" rx="12" fill="${COLORS.bg}"/>${shapes}</svg></div>`
}
function workflowFigure(id) {
  let s=''
  for (const [i,name] of ['Choose IDs','Align data types','Fit joint model','Review pathways'].entries()) {
    const x=15+i*169
    s+=box(x,84,153,73)+txt(x+76.5,114,name,COLORS.navy,12,650,'middle')+txt(x+76.5,137,['Sample / patient','Seven types possible','PCoA + clusters','Separate gene test'][i],COLORS.muted,10,450,'middle')
    if(i<3)s+=ln(x+153,120,x+168,120,COLORS.blue,3)
  }
  s+=txt(24,50,'All scientific inputs stay on this device',COLORS.blue,15,700)
  s+=txt(24,211,'Select Overview, Details, or Pathways after a run.',COLORS.muted,12)
  return figure(id,'Local integrative workflow','Four stages from metadata roles to separately calculated pathways.',s)
}
function coverageFigure(id){
  let s=txt(22,45,'6 eligible observations',COLORS.navy,15,700)
  for(let i=0;i<6;i++){const x=53+i*100;s+=dot(x,112,21,i<4?COLORS.blue:COLORS.orange)+txt(x,150,`S${i+1}`,COLORS.muted,12,600,'middle')}
  s+=box(34,178,294,44,COLORS.pale)+txt(181,205,'4 retained: full joint distances',COLORS.blue,12,600,'middle')
  s+=box(343,178,304,44,'#fff1dc')+txt(495,205,'2 omitted: inspect missing pairs',COLORS.orange,12,600,'middle')
  return figure(id,'Joint-model inclusion','Four of six eligible observations are fitted; two require missing-pair review.',s)
}
function associationFigure(id){
  let s=ln(70,191,590,191,COLORS.gray,1.5)+ln(70,35,70,191,COLORS.gray,1.5)
  for(const [x,y] of [[121,55],[182,83],[241,108],[306,135],[372,153],[430,171],[508,184]])s+=dot(x,y,8,COLORS.blue)
  s+=txt(322,228,'Toy PCo1',COLORS.navy,12,650,'middle')+txt(108,37,'G1 decreases to the right',COLORS.blue,13,700)
  s+=txt(398,61,'Reverse axis → reverse sign',COLORS.orange,11,600)
  return figure(id,'Negative correlation with a PCoA axis','Synthetic decreasing feature scores with increasing PCo1. Reversing the axis reverses the sign.',s)
}
function comparisonFigure(id){
  let s=box(20,32,190,174)+txt(115,58,'Unsupervised',COLORS.navy,14,700,'middle')+txt(115,83,'Joint clusters',COLORS.muted,12,600,'middle')
  for (const [x,y,c] of [[75,128,COLORS.blue],[105,145,COLORS.blue],[132,120,COLORS.orange],[155,152,COLORS.blue]])s+=dot(x,y,11,c)
  s+=box(244,32,190,174)+txt(339,58,'Predefined groups',COLORS.navy,14,700,'middle')+txt(339,83,'Case vs Control',COLORS.muted,12,600,'middle')
  s+=txt(269,132,'Case',COLORS.blue,13,700)+txt(348,132,'Control',COLORS.green,13,700)
  s+=ln(211,117,243,117,COLORS.gray,2)+ln(436,117,465,117,COLORS.blue,2)
  s+=box(468,49,191,139,COLORS.pale)+txt(564,80,'Corrected results',COLORS.blue,12,700,'middle')+txt(564,105,'Gene tests',COLORS.muted,12,600,'middle')+txt(564,136,'Then ORA / ranked',COLORS.muted,11,600,'middle')
  return figure(id,'Do not equate clusters with disease groups','A predefined comparison uses recorded metadata and is separate from the unsupervised joint structure.',s)
}
function oraFigure(id){
  let s=txt(27,42,'N=20 tested genes',COLORS.navy,14,700)
  for(let i=0;i<20;i++){const x=48+(i%10)*60,y=78+Math.floor(i/10)*58;s+=dot(x,y,12,i<5?COLORS.blue:COLORS.gray)}
  s+=box(34,183,284,47,COLORS.pale)+txt(176,211,'5 selected genes',COLORS.blue,13,700,'middle')
  s+=box(344,183,305,47,'#fff1dc')+txt(496,211,'6 in pathway; 3 selected overlap',COLORS.orange,12,700,'middle')
  s+=txt(405,40,'One-sided p ≈ 0.1313',COLORS.navy,12,650)
  return figure(id,'Toy hypergeometric ORA universe','A 20-gene universe contains five selected genes. A pathway contains six tested genes with three selected overlaps.',s)
}
function rankedFigure(id){
  let s=txt(24,38,'Full ranked list',COLORS.navy,14,700)
  let pts=[[74,161],[150,108],[226,88],[302,145],[378,163],[454,174],[530,188],[603,178]]
  for(let i=0;i<pts.length-1;i++)s+=ln(...pts[i],...pts[i+1],COLORS.blue,3)
  for(let i=0;i<pts.length;i++)s+=dot(...pts[i],7,(i===1||i===2)?COLORS.green:COLORS.blue)
  s+=txt(25,222,'Ranked genes G1 → G8',COLORS.muted,12)
  s+=box(384,31,268,45,COLORS.pale)+txt(518,58,'199 permutations: min p = 0.005',COLORS.blue,11,700,'middle')
  return figure(id,'Toy ranked running-sum enrichment','A synthetic running sum rises at matched pathway genes near the top. The example is not a real fitted ES or NES.',s)
}
function networkFigure(id){
  let s=ln(150,112,340,105,COLORS.blue,5)+dot(150,112,33,COLORS.blue)+dot(340,105,33,COLORS.blue)+dot(535,152,29,COLORS.gray)
  s+=txt(150,117,'P1',COLORS.white,13,750,'middle')+txt(340,110,'P2',COLORS.white,13,750,'middle')+txt(535,157,'P3',COLORS.white,13,750,'middle')
  s+=txt(243,72,'2 shared / 4 union',COLORS.navy,12,700,'middle')+txt(243,90,'Jaccard = 0.50',COLORS.blue,12,700,'middle')
  s+=txt(535,206,'No shared genes',COLORS.muted,12,600,'middle')+txt(26,229,'An edge does not mean a physical interaction.',COLORS.orange,11,650)
  return figure(id,'Pathway gene-overlap network','P1 and P2 share two scored genes out of four in their union; P3 shares none.',s)
}
export const integrativeVisualDefinitions = {
  'integrative-workflow': [{title:'One-button local multi-omics workflow',caption:'Select identity roles, align data types, fit the joint structure, and review separate pathway analysis.',render:workflowFigure}],
  'integrative-coverage': [{title:'Eligible and retained observation sets',caption:'Missing pairwise distances can exclude eligible observations from the joint model.',render:coverageFigure}],
  'integrative-patterns': [{title:'PCoA correlation is not a disease effect',caption:'Changing the arbitrary PCoA axis direction reverses the sign, not the underlying biological data.',render:associationFigure}],
  'integrative-next-steps': [{title:'Exploration followed by a predefined comparison',caption:'Do not equate unsupervised clusters with disease case/control labels.',render:comparisonFigure}],
  'pathway-ora': [{title:'Hypergeometric gene universe',caption:'Three selected genes in a six-gene pathway, from 20 tested genes, give p ≈ 0.1313.',render:oraFigure}],
  'pathway-ranked': [{title:'Ranked running-sum and permutations',caption:'199 gene-label permutations give a minimum empirical p-value of 0.005.',render:rankedFigure}],
  'pathway-network': [{title:'Jaccard pathway overlap',caption:'Two shared genes from four in the union give Jaccard 0.50; no protein interaction is inferred.',render:networkFigure}],
}
