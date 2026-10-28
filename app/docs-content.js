export const methodDocs = [
  {
    id: 'local-first',
    title: 'Local-first execution and scientific parity',
    keywords: 'privacy wasm webassembly worker local files parity browser github pages',
    summary: 'How 4C Insight runs analyses without uploading cohort data, and how native/byte parity is used as a release gate.',
    body: [
      '4C Insight references files selected on the user’s computer. Browser File objects stay local. A Web Worker reads source bytes only when a workflow needs them and calls the Rust/WebAssembly science engine.',
      'For browser-enabled science, fixed fixtures are run through native filesystem APIs and browser byte APIs. The browser workflow is enabled only after the relevant numerical outputs agree. This parity policy is a project validation rule, not a statistical method from the literature.',
    ],
    references: [
      { label: 'MDN Web Docs, WebAssembly', href: 'https://developer.mozilla.org/en-US/docs/WebAssembly' },
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
    title: 'Browser performance and parallel execution',
    keywords: 'performance rayon threads wasm webassembly parallel sharedarraybuffer worker memory cpu',
    summary: 'What runs in parallel now and what is required for true multi-core WebAssembly.',
    body: [
      'The browser runs the science engine in a Web Worker, which keeps heavy work off the UI thread. The current v0.21.0 WebAssembly science path is intentionally single-threaded inside Rust, even when the device reports multiple logical CPU threads. Native Rust paths already use bounded CPU parallelism for several expensive expression and repertoire operations.',
      'True Rayon parallelism in browser WebAssembly requires a threads-enabled WASM build, SharedArrayBuffer, cross-origin isolation, thread-pool initialization, and browser feature detection. 4C Insight is already cross-origin isolated, but the build/toolchain and fallback path must be changed and revalidated before enabling this safely.',
    ],
    references: [
      { label: 'Rayon documentation: WebAssembly behavior', href: 'https://docs.rs/rayon/latest/rayon/' },
      { label: 'wasm-bindgen-rayon documentation', href: 'https://docs.rs/wasm-bindgen-rayon/latest/wasm_bindgen_rayon/' },
    ],
  },
]
