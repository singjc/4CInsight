# 4C Cohort Explorer analysis guide

This guide is the plain-language companion to the app. It explains what each analysis layer is for, what the main plots mean, and where the main methods come from.

## Big picture

The app is organized as a simple progression:

```text
Merge & map samples
        ↓
Exploratory discovery
        ↓
Corrected / standard analysis
        ↓
Focused candidate signals
```

### 1. Merge & map samples

This step brings together the different files (gene expression, transcript expression, BCR, BCR isotype, TCR alpha, TCR beta, immune infiltrate) and matches them to the metadata table.

The **Merge & Export** workspace can also use an optional metadata file to decide which gene/transcript workbooks should be included in a merged matrix. Choose the metadata sample-ID column, then enable metadata filtering to use the metadata rows themselves as the inclusion list. If needed, add a second value filter such as phenotype/cohort/status and check only the values to retain. Filtering is off by default, and unmatched/ambiguous files are shown rather than causing the merge to fail.

Why it matters:

- it checks whether the sample IDs line up,
- it identifies unmatched or ambiguous files,
- and it creates a transparent audit trail before any statistics are run.

### 2. Exploratory discovery

This is the first-pass screening layer.

Use it when you want to know:

- Do the groups look different at all?
- Which genes or repertoire summaries look most different?
- Are there obvious outliers or broad patterns?

Key idea:

- Each feature is compared between groups on its own.
- The main outputs are an effect size, a p value, and a false-discovery-rate adjusted q value.
- Gene and transcript analyses also include volcano-style plots and detection-shift summaries.

Good for:

- quick discovery,
- spotting large effects,
- deciding whether a comparison is promising.

Main caveats:

- repeated samples from the same person are still separate observations here,
- covariates are not adjusted,
- repertoire composition and sequencing depth are handled more carefully only in the corrected layer.

### 3. Corrected / standard analysis

This is the more rigorous layer and is usually the main layer to report.

Use it when you want:

- methods that are closer to standard cohort analysis,
- subject-aware or covariate-aware testing,
- depth-aware repertoire analyses,
- compositional analyses for V/J/isotype usage,
- ordination and clustering summaries,
- pathway over-representation analysis.

#### Gene and transcript expression

The corrected layer uses count-aware modelling and multiple-testing correction. When a subject column is provided, it also produces a subject-collapsed sensitivity view so repeated measures do not dominate the interpretation.

#### BCR and TCR repertoires

Two kinds of questions are separated:

1. **How diverse is the repertoire?**
   These summaries are more fairly compared with matched-depth rarefaction.
2. **How is the repertoire composed?**
   Composition questions (isotype, V gene, J gene, V–J pairs) are handled with compositional methods, ordination, and PERMANOVA.

#### Immune infiltrate

Immune cell-type scores are analyzed as a feature matrix so group shifts, ordination structure, and clustering can be reviewed alongside the other modalities.

#### Pathways

Pathway analysis asks whether a biological pathway is over-represented among the differentially expressed genes. This helps turn long gene lists into broader biological themes.

### 4. Focused candidate signals

This layer is designed for simpler, more presentation-friendly comparisons.

Typical use case:

- a collaborator supplies a filtered metadata table,
- one sample per individual is retained,
- the cohort definition is simplified,
- and the goal is to produce a short list of promising preliminary signals.

The focused workflow:

1. takes the filtered metadata table,
2. reruns the corrected analysis on that simplified cohort,
3. extracts a shortlist of candidate genes, pathways, transcripts, and other signals,
4. optionally checks whether the candidate direction is consistent with the broader exploratory screen.

Interpretation:

- these are **not validated biomarkers**,
- they are **preliminary candidates** that are easier to present and prioritize for follow-up.


## PBMC / confounding diagnostics

Use this layer when a striking disease-associated gene program may actually reflect sample source, cell composition, processing history, family/batch concentration, or only a few extreme samples.

The diagnostics intentionally **do not replace the disease model**. They ask whether the leading signals are stable when the data are examined from a QC/provenance perspective.

### Sample-level checks

- whole-transcriptome PCA using top-variable `log2(TPM+1)` genes,
- robust PC1/PC2 outlier scores,
- library raw-count and gene-detection summaries,
- immune-signature PCA when immune-infiltrate files are available,
- a descriptive myeloid-high / lymphoid-low signature imbalance score,
- metadata context such as subject, batch/collection period, and family.

An outlier flag is a **reason to investigate**, not proof that a sample should be removed. Known tissue/source information and longitudinal consistency should carry more weight than an automated PCA threshold alone.

### Suspect-vs-ordinary screen

You can paste a list of unusual samples and compare them directly against the remaining ordinary expression samples **without using disease status**. If the same genes dominate both the disease analysis and this source-effect screen, the disease result may be strongly entangled with source/composition.

### Candidate robustness checks

For leading corrected genes the app reports:

- raw-count and TPM medians in each group,
- percent detected,
- fraction of samples with counts at least 5 and 10,
- how much of the group signal comes from the top two samples,
- leave-one-sample-out effect range,
- optional leave-batch/family/year-out effect range,
- a regularized-effect diagnostic,
- the original corrected effect,
- the QC-restricted effect after sensitivity-only exclusions,
- the immune-PC-adjusted effect,
- and the effect in the suspect-vs-ordinary screen.

A strong preliminary disease candidate should ideally retain direction and a similar effect across these views rather than disappearing after one problematic sample, one historical batch, or the major immune-composition axes are accounted for.

### Longitudinal QC

When a subject/patient column is available, repeated samples from the same person are summarized by their maximum whole-transcriptome PCA distance. A historical sample that sits far away from later samples from the same individual can be an especially useful clue about collection/processing or major composition differences.

## How to read the main plots

### Volcano or effect-significance scatter

- Left/right = negative/positive effect.
- Higher = stronger statistical support.
- The most interesting points are usually the ones that combine a large effect with a small q value.

### Detection-shift plot

Shows whether a gene is detected in a larger fraction of one group than the other. This helps highlight sparse or group-specific signals.

### PCA / PCoA

These are maps of the samples. Samples that sit near each other are more similar. Coloring by status, subject, or another metadata column helps show whether the main structure follows disease status, family, timepoint, batch, or something else.

### Dendrogram

A dendrogram shows hierarchical clustering as a tree. It is another way to ask whether samples group primarily by disease status or by another source of structure.

### Pathway ORA plot

Each point is a pathway. Higher points indicate stronger statistical support. Several related pathways can all appear together because they may share many genes and represent one broader biological theme.

## Suggested interpretation strategy

For a lay audience:

1. Start with the **focused** results to show a short, easy-to-follow shortlist.
2. Use the **corrected / standard** layer as the statistical backbone.
3. Use the **exploratory** layer to explain how the signals first emerged.
4. Treat all candidates as **preliminary** until validated in a larger or independent cohort.

## Key references

- **False discovery rate:** Benjamini Y, Hochberg Y (1995). *Controlling the false discovery rate: a practical and powerful approach to multiple testing.*  
  https://doi.org/10.1111/j.2517-6161.1995.tb02031.x
- **DESeq2:** Love MI, Huber W, Anders S (2014). *Moderated estimation of fold change and dispersion for RNA-seq data with DESeq2.*  
  https://doi.org/10.1186/s13059-014-0550-8
- **edgeR:** Robinson MD, McCarthy DJ, Smyth GK (2010). *edgeR: a Bioconductor package for differential expression analysis of digital gene expression data.*  
  https://doi.org/10.1093/bioinformatics/btp616
- **Compositional data analysis:** Aitchison J (1982). *The statistical analysis of compositional data.*  
  https://doi.org/10.1111/j.2517-6161.1982.tb01195.x
- **PERMANOVA:** Anderson MJ (2001). *A new method for non-parametric multivariate analysis of variance.*  
  https://onlinelibrary.wiley.com/doi/10.1111/j.1442-9993.2001.01070.pp.x
- **Gene set enrichment:** Subramanian A et al. (2005). *Gene set enrichment analysis: a knowledge-based approach for interpreting genome-wide expression profiles.*  
  https://doi.org/10.1073/pnas.0506580102
- **MSigDB Hallmark:** Liberzon A et al. (2015). *The Molecular Signatures Database Hallmark Gene Set Collection.*  
  https://doi.org/10.1016/j.cels.2015.12.004
- **Reactome:** Jassal B et al. (2020). *The Reactome pathway knowledgebase.*  
  https://doi.org/10.1093/nar/gkz1031
- **ssGSEA:** Barbie DA et al. (2009). *Systematic RNA interference reveals that oncogenic KRAS-driven cancers require TBK1.*  
  https://doi.org/10.1038/nature08460

## Practical note

The app is intended to help with discovery and interpretation. It does not replace external validation, domain-specific clinical interpretation, or a full study-design discussion.
