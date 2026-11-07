// User-facing method notes and illustrative diagrams for the five added modalities.
// These examples are teaching aids, not results from a cohort and not scientific fixtures.
const citation = (label, doi) => ({ label, href: `https://doi.org/${doi}` })
const shannon = citation('Shannon (1948), A Mathematical Theory of Communication', '10.1002/j.1538-7305.1948.tb01338.x')
const rarefaction = citation('Chao & Jost (2012), Coverage-based rarefaction and extrapolation', '10.1890/11-1952.1')
const horn = citation('Horn (1966), Measurement of overlap in comparative ecological studies', '10.1086/282436')
const aitchison = citation('Aitchison (1982), The statistical analysis of compositional data', '10.1111/j.2517-6161.1982.tb01195.x')
const permanova = citation('Anderson (2001), Non-parametric multivariate analysis of variance', '10.1111/j.1442-9993.2001.01070.pp.x')
const immuneRef = citation('Hänzelmann, Castelo & Guinney (2013), Gene set variation analysis', '10.1186/1471-2105-14-7')
const ssGSEA = citation('Barbie et al. (2009), Single-sample enrichment methodology', '10.1038/nature08460')
const tcrRef = citation('Robins et al. (2009), Assessment of TCR beta-chain diversity', '10.1182/blood-2009-04-217604')
const clonotypeRef = citation('Bolotin et al. (2015), MiXCR: adaptive immunity profiling', '10.1038/nmeth.3364')

export const multiomicsDocs = [
  {
    id: 'repertoire-diversity', title: 'Immune repertoires: diversity and matched depth',
    keywords: 'BCR TCR repertoire clones clonotypes rarefaction richness shannon clonality top clone depth',
    summary: 'Why BCR and TCR samples need comparable sequencing depth before diversity is compared.',
    body: [
      'A clonotype is a distinct receptor sequence represented in a sample. Richness counts observed clonotypes. Shannon diversity considers both the number of clonotypes and how evenly their counts are distributed. Clonality increases when a few clonotypes dominate.',
      'A sample with more reads can contain more observed clonotypes simply because it was sequenced more deeply. 4C Insight also reports diversity after matched-depth subsampling (rarefaction). The target depth is limited by the smallest usable repertoire and the selected maximum depth; repeated draws summarize sampling variation.',
      'The corrected comparison reports sample summaries, adjusted group tests, repertoire distance plots, and clustering. Sampling depth, cohort size, missing data, and selected covariates affect interpretation. An observed clonotype is not proof of antigen specificity.',
    ],
    references: [shannon, rarefaction, tcrRef],
  },
  {
    id: 'repertoire-sharing', title: 'Clonotype sharing, overlap, and repertoire distances',
    keywords: 'private public longitudinal shared clonotype convergence jaccard morisita horn PCoA PERMANOVA',
    summary: 'How the app distinguishes shared receptor sequences and compares whole repertoires.',
    body: [
      'Private clonotypes occur in only one observation. With valid subject IDs, longitudinally shared clonotypes occur in repeated samples of one subject, while public clonotypes occur in different subjects. These are read-weighted fractions in the sample summaries. Without subject IDs, interpretations of sharing are limited.',
      'Jaccard compares whether clonotypes are present. Morisita–Horn also considers their relative abundance. The two distances can give different views when a few expanded clones dominate. Principal coordinates analysis (PCoA) shows relationships based on these distances; a dendrogram groups similar samples.',
      'A PERMANOVA result tests whether the multivariate group separation is greater than expected under the selected permutation design. It is not a diagnostic classifier. Differences in dispersion can affect a distance-based group test.',
    ],
    references: [horn, permanova, clonotypeRef],
  },
  {
    id: 'bcr-repertoire', title: 'BCR repertoire: B-cell receptor analysis',
    keywords: 'BCR heavy chain V D J CDR3 clonotype B cell diversity composition adjusted model',
    summary: 'Interpret B-cell clone expansion, diversity, V/D/J usage, and group differences.',
    body: [
      'BCR heavy-chain workbooks describe receptor clonotypes and their counts. The app reports richness, Shannon diversity, clonality, dominant-clone fraction, CDR3 length, and sharing. It also compares V-gene, D-gene, J-gene, and V–J pairing compositions.',
      'A large dominant clone may indicate clonal expansion, but sequencing depth and sample preparation can also change its apparent proportion. V/D/J compositions are analyzed with centered log ratios (CLR) to respect that the reported fractions share a fixed total.',
      'Use the sample summaries together with the Morisita–Horn and Jaccard ordinations, dendrogram, adjusted tests, and reported warnings. A change in receptor composition alone does not establish its biological cause.',
    ],
    references: [clonotypeRef, shannon, aitchison],
  },
  {
    id: 'tcr-alpha', title: 'TCR-alpha: T-cell receptor alpha chains',
    keywords: 'TCR alpha chain TRA V J CDR3 receptor diversity clone usage pairing',
    summary: 'Understand alpha-chain clonotypes, V–J usage, and repertoire sharing.',
    body: [
      'TCR-alpha chains use V and J gene segments. They do not contain a D segment. The alpha-chain analysis reports CDR3 clonotype counts, sample diversity, sharing, V-gene usage, J-gene usage, and V–J pair composition.',
      'The diversity and distance methods follow the shared repertoire guide. A single-chain workbook cannot establish which alpha chain pairs with a beta chain in a cell. No paired-chain inference is made from separate alpha and beta reports.',
      'Review source coverage and matched-depth summaries before interpreting a difference in group composition. Significant V–J changes are associations within the analyzed cohort, not proof of antigen specificity.',
    ],
    references: [tcrRef, clonotypeRef, aitchison],
  },
  {
    id: 'tcr-beta', title: 'TCR-beta: T-cell receptor beta chains',
    keywords: 'TCR beta chain TRB V D J CDR3 clonality expanded clonotypes group comparison',
    summary: 'Understand beta-chain diversity, V/D/J composition, and expanded clonotypes.',
    body: [
      'TCR-beta chains use V, D, and J segments. The app compares observed and matched-depth diversity, clonality, the largest-clone fraction, CDR3 length, sharing, and V/D/J-related composition. The output also includes abundance-aware and presence-based repertoire distances.',
      'An expanded beta-chain clonotype can increase clonality while reducing evenness. A difference in a V or J composition test does not identify the antigen recognized by that receptor.',
      'Alpha and beta results are separate measurements. The app does not pair them into full receptors without paired-chain data. Repeated observations need the correct patient/subject mapping for meaningful sharing and adjusted comparisons.',
    ],
    references: [tcrRef, clonotypeRef, horn],
  },
  {
    id: 'bcr-isotypes', title: 'BCR isotypes: antibody-class composition',
    keywords: 'BCR isotype IgA IgG IgM undetermined CLR Aitchison PCoA robust comparison',
    summary: 'Read antibody-class proportions and adjusted differences in their relative balance.',
    body: [
      'BCR-isotype workbooks contain the relative composition of antibody classes, such as IgA, IgG, and IgM. Since the proportions sum to a whole, the app uses centered log-ratio (CLR) features for adjusted comparisons and Aitchison distances for PCoA and clustering.',
      'CLR effects describe differences relative to the other retained isotypes, not absolute concentrations. The retained isotype set matters. If you select the option to exclude “Undetermined”, the analysis uses only the remaining classes and reports that choice.',
      'The adjusted feature tests use robust uncertainty estimates and correction for multiple tests. An isotype proportion change is not a direct measurement of antibody titre, antigen affinity, or cell abundance.',
    ],
    references: [aitchison, permanova],
  },
  {
    id: 'immune-infiltration', title: 'Immune infiltration: cell-signature scores',
    keywords: 'immune infiltrate ssGSEA cell type immune signature enrichment PCA adjusted effect',
    summary: 'Interpret immune-signature scores and their adjusted differences across groups.',
    body: [
      'Immune-infiltration workbooks contain single-sample gene-set enrichment (ssGSEA) signature scores. These are relative enrichment signals. They are not direct cell counts or verified percentages of cells in a sample.',
      '4C Insight compares scores with an adjusted robust linear model. It shows feature-level effects, multiple-testing-adjusted results, PCA, and a dendrogram of standardized immune-signature scores.',
      'A higher signature score can reflect a stronger gene-expression pattern linked to that cell type. It does not prove that more cells of that type are present. Review the signature source, sample composition, and potential batch effects before making biological conclusions.',
    ],
    references: [ssGSEA, immuneRef, permanova],
  },
]

export const multiomicsToyExamples = {
  'repertoire-diversity': [{
    title: 'Compare the same repertoire depth',
    question: 'Can we compare observed diversity when one sample has twice as many reads?',
    columns: ['Sample', 'Clone A', 'Clone B', 'Clone C', 'Total reads', 'Target depth'],
    rows: [['S1', '60', '30', '10', '100', '50'], ['S2', '20', '15', '15', '50', '50']],
    steps: [
      'S1 has 100 reads; S2 has 50. The common target is 50 reads in this toy example.',
      'Sample 50 reads without replacement from S1; S2 already has 50. Repeat to reduce sensitivity to one random sample.',
      'Calculate richness and Shannon diversity in each matched-depth draw, then summarize across draws.',
    ],
    meaning: 'Matched depth reduces the advantage of deeper sequencing when comparing diversity.',
    limit: 'The example shows the target depth, not a deterministic rarefaction result. The app may also apply a maximum depth.',
  }],
  'repertoire-sharing': [{
    title: 'Classify private, repeated, and public clonotypes',
    question: 'Which receptor reads are unique, shared over time, or found in different people?',
    columns: ['Sample', 'Clone A', 'Clone B', 'Clone C', 'Other clones'],
    rows: [['P1, visit 1', '5', '3', '2', '—'], ['P1, visit 2', '4', '—', '—', 'Clone D'], ['P2, visit 1', '—', '—', '6', 'Clone E']],
    steps: [
      'Clone A appears in both visits for P1 only: it is longitudinally shared.',
      'Clone B occurs only in the first P1 visit: it is private.',
      'Clone C appears in P1 and P2: it is public.',
      'For P1 visit 1, read-weighted fractions are 50% longitudinal, 30% private, and 20% public.',
    ],
    meaning: 'Sharing uses receptor identity across samples, and the selected subject column determines what counts as a different person.',
    limit: 'Use exact clonotype identity and complete subject mapping. A shared sequence alone does not establish a shared antigen.',
  }],
  'bcr-repertoire': [{
    title: 'Calculate Shannon diversity and clonality',
    question: 'How does the size of an expanded B-cell clone change repertoire evenness?',
    columns: ['BCR clone', 'Reads', 'Relative frequency'],
    rows: [['BCR-A', '60', '0.60'], ['BCR-B', '30', '0.30'], ['BCR-C', '10', '0.10']],
    steps: [
      'Observed richness is three distinct clonotypes.',
      'Shannon diversity H = −(0.6 ln 0.6 + 0.3 ln 0.3 + 0.1 ln 0.1) ≈ 0.898.',
      'Clonality = 1 − H / ln(3) ≈ 0.183. The top-clone fraction is 60%.',
    ],
    meaning: 'An expanded clone increases dominance and generally increases the clonality score.',
    limit: 'This is one sample. Group inference needs multiple independent observations and the adjusted model.',
  }],
  'tcr-alpha': [{
    title: 'Read an alpha-chain V–J pairing change',
    question: 'Can two groups use the same V and J genes in different proportions?',
    columns: ['V–J pair', 'Control reads', 'Case reads'],
    rows: [['V1–J2', '40', '10'], ['V2–J1', '10', '40']],
    steps: [
      'Control has 80% V1–J2 reads. Case has 20%.',
      'The opposite V2–J1 change balances the composition.',
      'The full workflow compares relative usage with CLR-based adjusted tests; it does not infer a D segment or pair an alpha chain to a beta chain.',
    ],
    meaning: 'V–J usage can differ even when both groups have the same total read count.',
    limit: 'These are pooled toy counts, not biological replicates or a statistically tested difference.',
  }],
  'tcr-beta': [{
    title: 'Recognize a dominant beta-chain clonotype',
    question: 'What happens when one TCR-beta sequence becomes much more common?',
    columns: ['Beta chain', 'Control reads', 'Case reads'],
    rows: [['V1–D1–J2', '10', '70'], ['V2–D2–J1', '45', '15'], ['V3–D1–J1', '45', '15']],
    steps: [
      'Both groups have 100 reads in this toy comparison.',
      'The largest beta-chain fraction rises from 45% to 70%. This suggests greater dominance in Case.',
      'The corrected repertoire analysis considers clone diversity, shared clonotypes, V/D/J composition, and between-sample distances.',
    ],
    meaning: 'An expanded beta clonotype changes both evenness and abundance-weighted repertoire overlap.',
    limit: 'The figures do not demonstrate a cell expansion, antigen response, or statistically significant group difference.',
  }],
  'bcr-isotypes': [{
    title: 'Interpret a change in antibody-class balance',
    question: 'Why does an IgA fraction change also affect the other relative fractions?',
    columns: ['Isotype', 'Control', 'Case'],
    rows: [['IgA', '20%', '40%'], ['IgG', '50%', '40%'], ['IgM', '30%', '20%']],
    steps: [
      'Each sample composition sums to 100%. It reports relative abundance, not concentration.',
      'The IgA-to-IgG ratio moves from 20/50 = 0.4 to 40/40 = 1.0.',
      'The natural log ratio changes from ln(0.4) ≈ −0.916 to ln(1) = 0.',
      'The app fits CLR-transformed features after selecting its retained set of isotypes.',
    ],
    meaning: 'The change concerns the balance of antibody classes, not the absolute amount of IgA.',
    limit: 'Ratios here illustrate composition. They are not the fitted CLR group coefficients or p-values.',
  }],
  'immune-infiltration': [{
    title: 'Compare immune-signature enrichment',
    question: 'Does a higher CD8 signature score mean that 80% of cells are CD8 T cells?',
    columns: ['Signature', 'Control score', 'Case score'],
    rows: [['CD8 T-cell pattern', '0.20', '0.80'], ['B-cell pattern', '0.60', '0.30']],
    steps: [
      'Read each score as relative enrichment of its gene-expression pattern.',
      'CD8 signature enrichment is higher in Case in this toy matrix.',
      'The app fits an adjusted robust comparison across samples and corrects feature-level p-values for multiple testing.',
    ],
    meaning: 'A score of 0.80 is a signature score, not 80% of cells or a cell count.',
    limit: 'Score scale and interpretation depend on the upstream signature method and input processing.',
  }],
}

// Small, accessible inline SVG diagrams. No image downloads or remote assets.
const color = { ink: '#26354e', muted: '#66758f', blue: '#315ad7', red: '#c74553', green: '#18856c', orange: '#af741e', bg: '#f6f9fe', grid: '#e0e7f2', white: '#fff' }
const esc = (v) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
const n = (v) => Number(v).toFixed(1).replace(/\.0$/, '')
const r = (x,y,w,h,fill,rx=6,stroke='none') => `<rect x="${n(x)}" y="${n(y)}" width="${n(w)}" height="${n(h)}" rx="${rx}" fill="${fill}" stroke="${stroke}"/>`
const t = (x,y,value,opts={}) => `<text x="${n(x)}" y="${n(y)}" font-family="system-ui,sans-serif" font-size="${opts.size||13}" font-weight="${opts.weight||500}" text-anchor="${opts.anchor||'start'}" fill="${opts.fill||color.ink}">${esc(value)}</text>`
const l = (x1,y1,x2,y2,stroke=color.grid,width=2,dash='') => `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${stroke}" stroke-width="${width}" ${dash?`stroke-dasharray="${dash}"`:''}/>`
const frame=(id,title,description,s)=> `<div class="docs-visual-overflow"><svg xmlns="http://www.w3.org/2000/svg" class="docs-visual-svg" viewBox="0 0 680 254" role="img" aria-labelledby="visual-title-${esc(id)} visual-desc-${esc(id)}"><title id="visual-title-${esc(id)}">${esc(title)}</title><desc id="visual-desc-${esc(id)}">${esc(description)}</desc>${r(0,0,680,254,color.bg,12)}${s}</svg></div>`
function diversityChart(id) {
  let s=t(24,30,'Reads before matching depth',{weight:700})+t(362,30,'Reads after matching depth',{weight:700})
  for (const [i,name,original,matched,fill] of [[0,'S1',100,50,color.blue],[1,'S2',50,50,color.green]]){
    const y=71+i*74
    s+=t(26,y+20,name,{weight:700})+r(66,y,240,33,color.grid)+r(66,y,original*2.4,33,fill)+t(325,y+22,`${original}`,{weight:700,anchor:'end'})
    s+=t(361,y+20,name,{weight:700})+r(401,y,228,33,color.grid)+r(401,y,matched*4.56,33,fill)+t(644,y+22,`${matched}`,{weight:700,anchor:'end'})
  }
  s+=t(26,226,'S1 is subsampled. Repeat draws to estimate matched-depth diversity.',{size:12,fill:color.muted})
  return frame(id,'Matched-depth repertoire comparison','S1 has 100 reads and S2 has 50. Both compare at depth 50 after subsampling S1.',s)
}
function sharingChart(id){
  let s=t(24,29,'P1 · visit 1',{weight:700})+t(242,29,'P1 · visit 2',{weight:700})+t(464,29,'P2 · visit 1',{weight:700})
  const clones=[['A','5',color.blue,'A','4',color.blue,'—','',color.grid],['B','3',color.green,'D','6',color.grid,'E','4',color.grid],['C','2',color.orange,'—','',color.grid,'C','6',color.orange]]
  for(let i=0;i<3;i++){
    const y=52+i*49, row=clones[i]
    for(let j=0;j<3;j++){
      const x=24+j*220, key=row[j*3],amount=row[j*3+1],fill=row[j*3+2]
      s+=r(x,y,186,39,fill===color.grid?'#eef2f8':fill,7)+t(x+15,y+25,`Clone ${key}`,{weight:700,fill:fill===color.grid?color.muted:color.white})+t(x+167,y+25,amount,{weight:700,anchor:'end',fill:fill===color.grid?color.muted:color.white})
    }
  }
  s+=t(24,229,'A = shared over time       B = private       C = public across people',{size:12,weight:700})
  return frame(id,'Private, longitudinally shared, and public clonotypes','A is shared within P1, B is private, C is present in both P1 and P2.',s)
}
function bcrChart(id){
  let s=t(27,29,'B-cell receptor clone distribution',{weight:700})
  for(const [i,name,value,fill] of [[0,'Clone A',60,color.blue],[1,'Clone B',30,color.green],[2,'Clone C',10,color.orange]]){
    const y=60+i*50; s+=t(28,y+17,name)+r(129,y,460,28,color.grid)+r(129,y,value*4.6,28,fill)+t(621,y+19,`${value}%`,{weight:700,anchor:'end'})
  }
  s+=r(27,218,622,26,'#e4ebff')+t(340,235,'Richness = 3    •    Shannon ≈ 0.898    •    Clonality ≈ 0.183',{anchor:'middle',size:12,weight:700})
  return frame(id,'Illustrative BCR clone frequencies','The largest clone uses 60% of reads; richness, Shannon entropy and clonality summarize different diversity properties.',s)
}
function chainChart(id,kind){
  const alpha=kind==='alpha'
  let s=t(24,32,alpha?'TCR alpha chain: V and J':'TCR beta chain: V, D, and J',{weight:700})
  const parts=alpha
    ?[['V segment',color.blue,24,279],['J segment',color.green,313,337]]
    :[['V segment',color.blue,24,212],['D segment',color.orange,246,154],['J segment',color.green,410,240]]
  for(const [name,fill,x,w] of parts){
    s+=r(x,85,w,62,fill,8)+t(x+w/2,121,name,{weight:700,fill:color.white,anchor:'middle',size:12})
  }
  // CDR3 spans the joining region; it is not another V, D, or J gene segment.
  const from=alpha?266:199, to=alpha?358:456
  s+=l(from,155,from,170,color.red,2)+l(from,170,to,170,color.red,2)+l(to,155,to,170,color.red,2)
  s+=t((from+to)/2,187,'CDR3 region',{size:12,weight:700,anchor:'middle',fill:color.red})
  s+=t(24,211,alpha?'Alpha-chain usage is analyzed as V, J, and V–J pairs.':'Beta-chain usage includes V, D, J, and V–J pairs.',{size:13})
  s+=t(24,236,'Separate alpha and beta files do not establish paired receptors.',{size:12,fill:color.muted})
  return frame(id,alpha?'TCR-alpha segment schematic':'TCR-beta segment schematic',alpha?'An alpha chain has V and J segments. CDR3 spans the joining region.':'A beta chain contains V, D and J segments. CDR3 spans the joining region.',s)
}
function isotypeChart(id){
  let s=t(24,32,'Relative antibody-class composition',{weight:700}); const data=[['Control',[20,50,30]],['Case',[40,40,20]]]
  for(const [i,row] of data.entries()){
    const y=72+i*83;s+=t(25,y+23,row[0],{weight:700});let x=111
    for(const [j,value] of row[1].entries()){
      const w=value*4.9;s+=r(x,y,w,43,[color.blue,color.green,color.orange][j],1)+t(x+w/2,y+27,`${value}%`,{anchor:'middle',weight:700,fill:color.white});x+=w
    }
  }
  s+=t(113,235,'IgA',{fill:color.blue,weight:700})+t(213,235,'IgG',{fill:color.green,weight:700})+t(313,235,'IgM',{fill:color.orange,weight:700})
  s+=t(439,235,'Ratios, not absolute amounts',{size:12,fill:color.muted})
  return frame(id,'BCR isotype compositions','Control IgA/IgG/IgM is 20/50/30 percent; Case is 40/40/20 percent. Both sum to 100%.',s)
}
function immuneChart(id){
  let s=t(24,30,'Illustrative enrichment scores, not cell percentages',{weight:700})
  s+=t(24,76,'CD8 T-cell')+t(24,168,'B-cell')
  for(const [i,values] of [[0,[0.2,0.8]],[1,[0.6,0.3]]]){
    const y=94+i*92;s+=l(140,y,600,y,color.grid,4)
    for(const tick of [0,0.5,1]) {const x=140+tick*460;s+=l(x,y-15,x,y+15,color.grid,1)+t(x,y+34,`${tick}`,{anchor:'middle',size:11,fill:color.muted})}
    for(const [j,val] of values.entries()){
      const x=140+val*460; s+=`<circle cx="${n(x)}" cy="${y}" r="${j===0?10:8}" fill="${j===0?color.blue:color.red}" stroke="${color.white}" stroke-width="2"/>`
    }
  }
  s+=r(444,29,13,13,color.blue,3)+t(465,40,'Control',{size:12})+r(547,29,13,13,color.red,3)+t(568,40,'Case',{size:12})
  return frame(id,'Immune-signature enrichment','CD8 and B-cell pattern scores differ between two toy samples. They are not percentages of cells.',s)
}
export const multiomicsVisualDefinitions={
  'repertoire-diversity':[{title:'Match repertoire depth before comparing diversity',caption:'Read depth changes observed diversity. Subsampling sets a common comparison depth.',render:diversityChart}],
  'repertoire-sharing':[{title:'Private, repeated, and public sequences',caption:'Subject identity determines whether a sequence is shared over time or across people.',render:sharingChart}],
  'bcr-repertoire':[{title:'One expanded BCR clone changes evenness',caption:'Richness counts clones. Shannon and clonality also use their relative frequencies.',render:bcrChart}],
  'tcr-alpha':[{title:'Alpha chains use V and J segments',caption:'Do not assume an alpha-chain D segment or alpha–beta pairing from these files.',render:(id)=>chainChart(id,'alpha')}],
  'tcr-beta':[{title:'Beta chains use V, D, and J segments',caption:'Beta-chain clone dominance and gene-segment usage are distinct summaries.',render:(id)=>chainChart(id,'beta')}],
  'bcr-isotypes':[{title:'Compare antibody classes as a composition',caption:'CLR tests describe relative class balance. A changed fraction is not a concentration.',render:isotypeChart}],
  'immune-infiltration':[{title:'Immune scores describe signatures, not cell fractions',caption:'An ssGSEA score of 0.8 does not mean that 80% of cells have that identity.',render:immuneChart}],
}
