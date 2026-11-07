import { multiomicsDocs } from './docs-new-modalities.js?v=0.21.0-md5-immune-guides'
export const methodDocs = [
  {
    id: 'local-first',
    title: 'Your data stays on your device',
    keywords: 'privacy files local saved project data access sharing security',
    summary: 'What stays private when you load data, run an analysis, or save a project.',
    body: [
      '4C Insight reads only files you choose from your computer. The analysis runs in this browser. Source cohort files are not uploaded to an analysis server.',
      'The saved project contains file references, selected settings, and analysis state. It is not a copy of the original cohort files. To use a saved project later, reconnect the source folder and check that the files match.',
      'Results saved in History stay in this browser profile. Exported files are stored only where you choose to save them. Treat exported results and metadata as research data and share them only as appropriate.',
    ],
    references: [
      { label: 'Sandve et al. (2013), Ten Simple Rules for Reproducible Computational Research', href: 'https://doi.org/10.1371/journal.pcbi.1003285' },
    ],
  },
  {
    id: 'file-merging',
    title: 'Gene and transcript workbook merging',
    keywords: 'gene transcript sample workbooks merge xlsx raw count total reads tpm reference workbook missing unknown feature',
    summary: 'Build one feature-by-sample table from individual expression workbooks.',
    body: [
      'The merger reads sample workbooks in a deterministic order. The first sorted workbook defines the feature identifiers and required structure. Later workbooks are matched to those identifiers, not to worksheet row positions.',
      'The merge report counts missing and unknown features. The output includes the selected metrics and column order. The browser writes the merged table locally for download or downstream analysis.',
      'A merge joins measurements. It does not normalize expression, remove batch effects, or test for differential expression.',
    ],
    references: [
      { label: 'Sandve et al. (2013), Ten Simple Rules for Reproducible Computational Research', href: 'https://doi.org/10.1371/journal.pcbi.1003285' },
    ],
  },
  {
    id: 'sample-metadata',
    title: 'Sample metadata, editing, and file mapping',
    keywords: 'metadata viewer creator editor sample id subject group file mapping aliases validation duplicate id export',
    summary: 'Link source files to samples and review the cohort description before analysis.',
    body: [
      'The Sample Metadata workspace shows existing sample fields or creates a new sample sheet. Users can edit rows, search, filter, and assign expression files to sample identifiers. Blank and duplicate sample IDs are flagged for correction.',
      'File mappings can be exported separately and can populate standard modality-alias columns. Downstream analyses still match expression table columns to the selected metadata ID role; the file-mapping manifest does not silently rename expression columns.',
      'Download edited metadata and mapping tables to preserve them. A portable state JSON stores settings and references, not full raw data files.',
    ],
    references: [
      { label: 'Sandve et al. (2013), Ten Simple Rules for Reproducible Computational Research', href: 'https://doi.org/10.1371/journal.pcbi.1003285' },
    ],
  },
  {
    id: 'expression-qc',
    title: 'Expression QC and count-scale inputs',
    keywords: 'expression qc tpm raw count total reads median quartile zero missing normalization',
    summary: 'Descriptive expression-distribution checks and why TPM means are not a between-sample QC signal.',
    body: [
      'Explore & QC reports per-sample distribution summaries such as median, quartiles, zero fraction, and missing fraction. These are descriptive checks. They do not perform group testing or automatic outlier exclusion.',
      'For differential gene analysis, the corrected model uses Raw Count columns. For transcript analysis, it uses Total Reads. TPM remains useful for descriptive abundance distributions, but its sample total is constrained by definition, so the mean TPM is expected to be similar when the same feature universe is present.',
    ],
    references: [
      { label: 'Anders & Huber (2010), Differential expression analysis for sequence count data', href: 'https://doi.org/10.1186/gb-2010-11-10-r106' },
      { label: 'Love, Huber & Anders (2014), DESeq2', href: 'https://doi.org/10.1186/s13059-014-0550-8' },
    ],
  },
  {
    id: 'normalization-nb2',
    title: 'Median-ratio normalization and corrected NB2 model',
    keywords: 'negative binomial nb2 glm log link size factor median ratio differential expression robust standard error repeated measures',
    summary: 'The corrected count model used by Compare Groups.',
    body: [
      '4C Insight estimates positive-count median-ratio size factors, then fits each retained feature with a negative-binomial NB2 generalized linear model using a log link and log(size factor) offset.',
      'The fitted group coefficient is converted from the natural-log link scale to log2 fold change. The displayed “log2 group effect” is therefore the model coefficient on the log2 scale.',
      'The implementation is not DESeq2. In particular, 4C Insight currently estimates an unshrunk feature-level NB2 dispersion by an iterative method-of-moments procedure. DESeq2 is cited because median-ratio normalization and negative-binomial count modeling are closely related methodological foundations, not because the software is calling DESeq2.',
      'When a patient/subject column is selected and repeated observations are present, 4C Insight uses a subject-clustered sandwich covariance estimate. This adjusts standard errors for within-subject dependence while keeping the fitted mean model unchanged.',
    ],
    references: [
      { label: 'Anders & Huber (2010), Differential expression analysis for sequence count data', href: 'https://doi.org/10.1186/gb-2010-11-10-r106' },
      { label: 'Love, Huber & Anders (2014), Moderated estimation of fold change and dispersion for RNA-seq data with DESeq2', href: 'https://doi.org/10.1186/s13059-014-0550-8' },
      { label: 'Liang & Zeger (1986), Longitudinal data analysis using generalized linear models', href: 'https://doi.org/10.1093/biomet/73.1.13' },
    ],
  },
  {
    id: 'fdr-volcano',
    title: 'Multiple testing, FDR, and volcano plots',
    keywords: 'benjamini hochberg fdr false discovery rate volcano plot log2 fold change q value cutoff',
    summary: 'How feature-level q-values and the adjustable volcano thresholds are interpreted.',
    body: [
      'Feature p-values are adjusted with the Benjamini–Hochberg procedure. The reported q-value is the BH-adjusted feature-level value used by the application’s FDR threshold controls.',
      'The volcano plot uses model log2 fold change on the x-axis and −log10(BH q-value) on the y-axis. Dashed vertical lines show the selected absolute log2FC cutoff and the dashed horizontal line shows the selected FDR cutoff.',
      'Changing the FDR or log2FC controls does not refit the differential model. It changes result classification, volcano colours, counts, and the default post-selection Significant Hits threshold.',
    ],
    references: [
      { label: 'Benjamini & Hochberg (1995), Controlling the False Discovery Rate', href: 'https://doi.org/10.1111/j.2517-6161.1995.tb02031.x' },
      { label: 'Love, Huber & Anders (2014), DESeq2', href: 'https://doi.org/10.1186/s13059-014-0550-8' },
    ],
  },
  {
    id: 'pca-clustering',
    title: 'PCA and hierarchical clustering',
    keywords: 'pca principal component analysis average linkage euclidean dendrogram clustering top variable features',
    summary: 'Unsupervised cohort structure in Compare Groups.',
    body: [
      'Global PCA and the Compare Groups dendrogram use top-variable normalized features. Case/Control labels do not select these features. The plots are therefore descriptive views of the global expression geometry rather than post-selection views.',
      'PCA constructs orthogonal directions that successively capture variance in the analyzed feature matrix. The dendrogram uses Euclidean sample distance and average-linkage agglomeration.',
      'Interactive zoom changes the plotted coordinate domain. Sample labels are shown directly when the plot is sufficiently zoomed and there is enough screen separation between nearby points.',
    ],
    references: [
      { label: 'Jolliffe & Cadima (2016), Principal component analysis: a review and recent developments', href: 'https://doi.org/10.1098/rsta.2015.0202' },
      { label: 'Sokal & Michener (1958), A statistical method for evaluating systematic relationships', href: 'https://www.biodiversitylibrary.org/page/41974026' },
    ],
  },
  {
    id: 'transcript-usage',
    title: 'Transcript usage and compositional log ratios',
    keywords: 'transcript isoform dtu differential transcript usage clr centered log ratio compositional aitchison',
    summary: 'The transcript-usage screen and why log-ratio geometry is used.',
    body: [
      'The transcript workflow includes a differential transcript-usage screen. Transcript abundances are considered within a gene as a composition and transformed with centered log ratios (CLR) after adding the configured pseudocount.',
      'This avoids interpreting proportions as if they were unconstrained independent measurements. Gene-level and transcript-level tests are adjusted for multiple testing before they are reported.',
    ],
    references: [
      { label: 'Aitchison (1982), The Statistical Analysis of Compositional Data', href: 'https://doi.org/10.1111/j.2517-6161.1982.tb01195.x' },
      { label: 'Benjamini & Hochberg (1995), FDR control', href: 'https://doi.org/10.1111/j.2517-6161.1995.tb02031.x' },
    ],
  },
  {
    id: 'longitudinal',
    title: 'Longitudinal patient-signature analysis',
    keywords: 'longitudinal patient identity auc nearest neighbour within between distance repeated samples stability',
    summary: 'How repeated samples are evaluated for patient-specific stability.',
    body: [
      'Longitudinal analysis first builds an unsupervised expression distance matrix without using patient identity. Patient labels are applied only after the distances are complete.',
      'Patient identity AUC is the probability that a randomly chosen same-patient pair ranks closer than a randomly chosen different-patient pair. A value near 0.5 indicates little ranking signal; 1.0 indicates perfect separation of same- versus different-patient pair distances.',
      'Nearest-neighbour patient accuracy asks whether the closest other observation belongs to the same patient. Exact distance ties receive fractional credit. Within- and between-patient medians provide a direct distance-scale summary.',
    ],
    references: [
      { label: 'Hanley & McNeil (1982), The meaning and use of the area under an ROC curve', href: 'https://doi.org/10.1148/radiology.143.1.7063747' },
      { label: 'Cover & Hart (1967), Nearest neighbor pattern classification', href: 'https://doi.org/10.1109/TIT.1967.1053964' },
    ],
  },
  {
    id: 'significant-hits',
    title: 'Significant Hits: post-selection PCA, clustering, and heatmap',
    keywords: 'significant hits post selection heatmap silhouette permanova nearest neighbour selection bias validation',
    summary: 'Why strong separation after feature selection is descriptive and not independent validation.',
    body: [
      'Significant Hits reuses the completed Compare Groups result. Features are selected using the global BH-FDR and absolute log2FC thresholds, then the selected matrix is used for PCA, average-linkage clustering, and the z-score heatmap.',
      'Silhouette measures how much each sample is closer to its assigned group than to the other group in the selected-feature geometry. Nearest-neighbour group accuracy summarizes local group coherence. PERMANOVA tests multivariate group separation by permutation; when subject IDs are available, 4C Insight keeps permutations within the subject-aware design used by the workflow.',
      'These quantities are post-selection descriptive measures. The same labels were used to select the features and then to display/evaluate their structure. They must not be interpreted as held-out predictive performance. Independent performance requires feature selection to occur inside each training split or an external test cohort.',
    ],
    references: [
      { label: 'Rousseeuw (1987), Silhouettes', href: 'https://doi.org/10.1016/0377-0427(87)90125-7' },
      { label: 'Anderson (2001), A new method for non-parametric multivariate analysis of variance', href: 'https://doi.org/10.1111/j.1442-9993.2001.01070.pp.x' },
      { label: 'Ambroise & McLachlan (2002), Selection bias in gene extraction', href: 'https://doi.org/10.1073/pnas.102102699' },
    ],
  },
  {
    id: 'exclusions',
    title: 'Metadata exclusions and provenance',
    keywords: 'qc exclusion outlier failed metadata provenance sample filter audit',
    summary: 'How user-reviewed exclusions are applied and recorded.',
    body: [
      'Reviewed-QC and custom metadata exclusion rules are optional and independent. Each rule uses a user-selected metadata column and one or more exact observed values.',
      'Matching observations are removed before cohort construction and model fitting. The selected rule and excluded sample IDs are frozen into the analysis result and browser History so the filtering decision remains auditable.',
    ],
    references: [
      { label: 'Sandve et al. (2013), Ten Simple Rules for Reproducible Computational Research', href: 'https://doi.org/10.1371/journal.pcbi.1003285' },
    ],
  },
  {
    id: 'performance',
    title: 'Performance and large datasets',
    keywords: 'performance browser memory cpu large dataset resource settings file size',
    summary: 'Understand why large cohorts can take time and what the device information means.',
    body: [
      'Large cohorts may take time to read and analyze. 4C Insight processes selected files on your device. You can stop a run and check its progress without sending the files to a server.',
      'The Settings page shows CPU information when the browser makes it available. It also shows allocated analysis memory. This is an estimate of memory reserved for calculations, not the total RAM or free memory of your computer.',
      'There is no fixed input-file size cap. Very large analyses can still use more memory than the browser can provide. Close other demanding tabs if needed. The app can release analysis memory after a run.',
    ],
    references: [
      { label: 'Sandve et al. (2013), Reproducible Computational Research', href: 'https://doi.org/10.1371/journal.pcbi.1003285' },
    ],
  },
  ...multiomicsDocs,
]
