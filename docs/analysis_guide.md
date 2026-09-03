# 4C Cohort Explorer analysis guide

This guide explains the current unified **Cohort Analysis** workflow in plain language. The application is intended for cohort QC, robust discovery, candidate prioritization, and transparent sensitivity analysis. It does **not** by itself establish a clinically validated biomarker.

## Big picture

Cohort Analysis now has one primary statistical workflow:

```text
Define cohort → review mapping & QC → choose analysis settings → Run analysis
                                                        ↓
                                            inspect unified results
                                                        ↓
                                  simplified report OR full report
```

The **corrected / standard analysis is the primary inferential engine**. Cross-modal sample QC, exploratory expression checks, and PBMC/confounding diagnostics are supporting QC or sensitivity evidence. Candidate prioritization is a presentation layer derived from the corrected result. A collaborator-supplied filtered metadata table defines a cohort/subset; it does not select a different statistical methodology.

Both HTML report styles and the technical table export are generated from the **same frozen completed analysis run**. Exporting a different report does not rerun statistics or change sample inclusion.

## 1. Cohort & metadata

Start by choosing the project folder and loading the primary sample metadata table.

The app recursively detects the supported data types and merged expression matrices. Sample IDs are matched with the same punctuation-insensitive normalization used throughout the project, and the sample-ID metadata column can be selected explicitly.

### Standard metadata and Metadata Builder

The **Metadata Builder** top-level tab can create or curate a standard UTF-8 TSV before analysis. The standard table uses one row per biological sample and includes:

- required canonical `sample_id`;
- recommended `subject_id` for repeated samples;
- recommended `analysis_group` as a general-purpose custom comparison field;
- cohort/diagnosis/phenotype and longitudinal fields;
- sample source/tissue/specimen provenance;
- demographic/family/site fields that may be relevant covariates;
- collection/processing/library/sequencing batch fields;
- optional modality-specific sample aliases;
- curator QC/provenance notes.

The builder can start from the samples discovered in the current cohort scan, add missing standard fields to an older metadata table without deleting extra columns, validate normalized sample-ID uniqueness, and save the result directly into Cohort Analysis. See `docs/sample_metadata_schema.md` and `templates/4C_multiomics_sample_metadata_template.tsv`.

If an upstream modality uses a different identifier from the canonical sample ID, populate its alias column (for example `gene_expression_id` or `bcr_id`). The alias resolves back to the canonical biological sample and therefore does not create an extra observation.

### Comparison definition

The unified workflow no longer requires one of the two historical 4C comparisons. Under **Analysis settings → Comparison definition**, choose either:

- **Built-in 4C presets** — `HYP vs True_HC` or `HYP+ME/CFS vs ME/CFS-only`; or
- **Custom from metadata** — choose any metadata column and assign one or more distinct values to Case and Control.

For example, `analysis_group` could compare `Disease` against `Healthy`, `treatment` could compare `Drug` against `Placebo`, or `timepoint` could compare `Post` against `Baseline`. Metadata rows assigned to neither side are outside that particular comparison. This selection only defines the samples/groups; the corrected/standard statistical methodology is unchanged.

### Optional filtered metadata subset

A collaborator may provide a smaller metadata table, for example a curated one-sample-per-person subset. In the unified workflow this is treated as a **cohort definition**:

- the filtered table determines which metadata rows/samples are eligible for that run;
- the same corrected/standard methods are used;
- the run records which metadata file and sample-ID column defined the cohort;
- full and simplified reports describe that cohort provenance.

A filtered subset is therefore not an independent “focused methodology.”

## 2. Sample mapping & QC

### Mapping and coverage audit

Before interpreting a comparison, review how metadata samples map to the detected files and modalities. The audit keeps visible:

- matched files,
- unmatched files,
- ambiguous matches,
- metadata-only samples,
- duplicate modality files,
- modality coverage for each sample.

Unavailable or malformed individual modalities are reported without aborting other usable analyses.

### Cross-modal sample outlier QC

Cross-modal outlier screening asks a different question from differential analysis: **does a sample look unusually different from comparable samples within one or more data types?**

Supported representations include:

- **Gene expression:** PCA of top-variable expression features, preferentially `log2(TPM+1)`.
- **Transcript expression:** PCA of top-variable transcript features.
- **BCR/TCR:** PCA of robust-standardized repertoire QC metrics.
- **BCR isotype:** CLR-transformed composition PCA.
- **Immune infiltrate:** PCA of robust-standardized ssGSEA signatures.

Detection can be done within case/control groups or across all samples. **Within-group detection is the recommended default** when a real disease-wide biological shift is possible, because a case/control difference should not by itself cause an entire group to be labelled abnormal.

The app can combine per-modality flags using:

- any selected modality,
- at least N modalities,
- a majority of available selected modalities,
- all available selected modalities,
- plus a configurable minimum number of available modalities for consensus rules.

### Automatic flags are not automatic exclusions

A new outlier audit creates **flags only**. It does not automatically select consensus-flagged samples for deletion.

Review the sample-level plots, metadata/provenance, longitudinal context, and whether an abnormality is assay-specific or cross-modal. A reviewer can explicitly retain an automatically flagged sample or mark another sample for exclusion when the provenance/QC evidence supports that decision.

If reviewed exclusions are applied to the analysis, the run freezes:

- outlier scope and robust-distance threshold,
- consensus rule and minimum-modality requirement,
- automatic consensus flags,
- manual retain/exclude overrides,
- exact applied exclusion list,
- case/control counts before and after filtering,
- per-modality coordinates, robust distances, and flags.

Changing the live QC settings after the run does not rewrite this frozen provenance.

## 3. Analysis settings

### Comparison

Choose the metadata-defined case/control comparison. Current built-in presets include:

- **HYP vs True_HC**;
- **HYP+ME/CFS vs ME/CFS-only**.

The active primary or filtered metadata table defines which rows are eligible.

### Modalities

Any available combination can be selected:

- gene expression,
- transcript expression,
- BCR,
- BCR isotype,
- TCR alpha,
- TCR beta,
- immune infiltrate,
- pathway over-representation analysis.

The workflow is fault tolerant: failure or absence of one modality is recorded as a warning while successful modalities remain available.

### Repeated measures and covariates

A subject/patient column can be supplied for repeated sampling. Optional metadata covariates can also be selected. These settings are part of the frozen run configuration.

### Repertoire and pathway settings

The existing repertoire controls remain available, including:

- exact matched-depth rarefaction settings,
- PERMANOVA permutations,
- V/D/J/V–J ambiguity policy,
- BCR-isotype `Undetermined` exclusion/renormalization sensitivity.

For pathway analysis, either choose a cached public MSigDB human symbol GMT collection or a custom GMT. The report preserves the collection/path provenance used for the run.

## 4. Run analysis: primary corrected / standard engine

There is one obvious **Run analysis** action. It calls the existing corrected statistical implementations rather than a new parallel engine.

### Gene and transcript expression

The corrected expression layer uses:

- positive-count median-ratio normalization,
- a negative-binomial log-link count model with log size-factor offset,
- feature-wise dispersion,
- optional metadata covariates,
- subject-clustered sandwich standard errors when repeated sampling is configured,
- Benjamini–Hochberg FDR,
- a subject-collapsed sensitivity analysis,
- PCA on variable normalized features,
- average-linkage hierarchical clustering/dendrograms.

Transcript analysis additionally retains the within-gene CLR/Simes/BH differential-transcript-usage screen.

Important caveat: this native model does **not** claim to reproduce the exact empirical-Bayes/dispersion-shrinkage machinery of DESeq2, edgeR, or limma-voom.

### BCR and TCR repertoires

The corrected repertoire analyses retain:

- exact without-replacement matched-depth Monte-Carlo rarefaction,
- diversity and repertoire-summary metrics,
- subject-aware private/longitudinal/public clonotype fractions,
- Morisita-Horn and Jaccard distances,
- PCoA,
- subject-blocked permutation PERMANOVA,
- CLR/Aitchison compositional analyses,
- V/D/J/V–J ambiguity policies,
- average-linkage dendrograms,
- TCR amino-acid convergence read fraction when nucleotide sequences are available.

### BCR isotype

Dedicated isotype analysis uses CLR/Aitchison composition methods and retains the optional sensitivity analysis that excludes `Undetermined` and renormalizes the remaining fractions.

### Immune infiltrate

Immune ssGSEA signatures retain covariate-adjusted feature models, standardized-signature PCA, and clustering/dendrogram views.

### Pathways

Optional GMT-driven over-representation analysis is derived from the corrected gene result using:

- the tested-gene universe,
- hypergeometric testing,
- odds ratios,
- overlap genes,
- BH FDR.

Related pathways can share many genes, so several significant pathways may represent one broader biological program rather than independent discoveries.

## Supporting descriptive direction checks

An optional TPM-based exploratory expression calculation can be attached to the run for **direction concordance only**. It is useful for saying that a shortlisted feature points in the same or opposite direction in a simple descriptive screen.

This support calculation:

- does not replace the corrected model,
- does not change corrected p/q values,
- is not an independent validation dataset,
- should not be counted as a second statistical vote.

## PBMC / confounding sensitivity diagnostics

PBMC/source/composition diagnostics are an optional robustness stage of the same completed run. They are especially useful when a striking expression program may reflect wrong sample source, unusual PBMC composition, processing history, family/batch concentration, low counts, or only a few extreme samples.

### Sample-level checks

The diagnostics can include:

- whole-transcriptome PCA using top-variable `log2(TPM+1)` genes,
- robust transcriptome outlier scores,
- library raw-count and gene-detection summaries,
- immune-signature PCA when immune-infiltrate files are available,
- a descriptive myeloid-high / lymphoid-low signature-imbalance score,
- known non-PBMC/wrong-source sample IDs,
- reviewer-supplied suspect sample IDs,
- metadata context such as subject, batch/collection period, and family.

An outlier flag remains a **reason to investigate**, not proof that a sample is invalid. Known tissue/source information and longitudinal consistency should carry more weight than one automated threshold alone.

### Suspect-vs-ordinary source-effect screen

Reviewer-supplied unusual samples can be compared against ordinary expression samples without using disease status. Strong overlap between this source-effect signal and the disease result is evidence that disease and source/composition may be entangled.

### Candidate robustness checks

For leading corrected genes the diagnostics can report:

- raw-count and TPM medians,
- detection fractions,
- fractions with counts at least 5 and 10,
- top-two-sample contribution,
- leave-one-sample-out effect range,
- optional leave-batch/family/year-level-out effect range,
- a regularized-effect diagnostic,
- original corrected effect,
- QC-restricted corrected effect,
- immune-PC-adjusted corrected effect,
- suspect-vs-ordinary source effect.

The candidate shortlist can inherit the existing robustness classification and flags from these diagnostics.

A stronger preliminary disease candidate should usually retain direction and a similar effect across reasonable sensitivity views rather than disappearing after one sample, one historical group, or the major immune-composition axes are accounted for.

### Longitudinal QC

When a subject/patient column is available, repeated samples can be summarized by their maximum whole-transcriptome PCA distance. A historical sample far from later samples from the same person can be an especially useful clue about collection/processing or major composition differences.

## 5. Unified results

One completed run owns the result/provenance required by all views and exports. The GUI groups the output into:

- Overview / key findings,
- Gene,
- Transcript,
- BCR,
- BCR isotype,
- TCR alpha,
- TCR beta,
- Immune,
- Pathways,
- QC / robustness.

### Candidate prioritization

The shortlist of genes, pathways, transcripts, and other modality signals is derived from the primary corrected results. Ranking remains transparent, based on corrected statistical support and effect magnitude rather than an opaque biomarker score.

Optional descriptive direction concordance and robustness classifications are annotations to that shortlist. They do not create a separate inferential analysis.

Use language such as:

- **preliminary signal**,
- **candidate gene**,
- **candidate pathway**,
- **promising feature for follow-up**.

Avoid describing a result as a **validated biomarker** without independent validation.

## Reports and exports

### Simplified key-findings report

The simplified report is intended for presentations and collaborator-facing summaries. It contains only the most decision-relevant material, such as:

- comparison and analyzed sample counts,
- concise QC statement,
- main takeaway,
- strongest preliminary candidate genes,
- strongest pathway themes,
- strongest non-expression modality signals,
- a small number of main plots,
- concise confidence/limitations language.

It is a view of the frozen completed run, not a rerun of a “focused” engine.

### Full detailed report

The full report is the technical/audit record and can include:

- cohort definition and metadata provenance,
- mapping/coverage audit,
- exact reviewed exclusions and reasons,
- automatic/manual QC decisions,
- per-modality outlier evidence,
- all selected corrected modality results,
- expression/repertoire/immune ordinations and dendrograms,
- DTU and composition analyses,
- PERMANOVA/repertoire summaries,
- pathway source and enrichment results,
- PBMC/confounding diagnostics,
- QC-restricted and composition-adjusted sensitivity models,
- candidate robustness diagnostics,
- warnings and unavailable modalities,
- methods, assumptions, and limitations.

### Technical tables

The technical export writes the mapping/QC provenance and selected corrected/diagnostic/candidate tables from the same completed run. No export silently changes sample inclusion.

## How to read the main plots

### Volcano or effect-significance scatter

- Left/right = negative/positive effect.
- Higher = stronger statistical support.
- The most interesting points usually combine a meaningful effect size with a small q value.

### Detection-shift plot

Shows whether a gene is detected in a larger fraction of one group than the other. This can reveal sparse or group-specific support that mean effects alone may hide.

### PCA / PCoA

These are low-dimensional maps of sample structure. Samples plotted close together are more similar in the chosen feature/distance space. Coloring by group, subject, batch, timepoint, or another metadata field can show whether the dominant structure follows biology or a possible confounder.

### Dendrogram

A dendrogram shows hierarchical clustering as a tree. It asks a similar sample-structure question to PCA/PCoA but can make nested groups or repeated-subject clustering easier to see.

### Pathway ORA plot

Each point is a pathway. Stronger statistical support appears higher in the plot, while odds ratio/overlap summarize enrichment strength. Closely related pathways frequently share genes and should be interpreted as broader themes rather than independent replications.

## Suggested interpretation strategy

1. Confirm that the cohort definition and sample mapping are correct.
2. Review cross-modal QC flags and provenance; do not remove samples merely because they are unusual.
3. Use the corrected/standard results as the statistical backbone.
4. Use PBMC/source/composition and leave-one/leave-level sensitivity diagnostics to test whether leading effects are robust.
5. Use the key-findings shortlist to communicate the strongest preliminary signals without overstating them.
6. Use the simplified report for presentation and the full report for technical audit; both come from the same run.
7. Prioritize independent replication or a pre-specified follow-up cohort before making biomarker claims.

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

4C Cohort Explorer is intended to help with transparent discovery, QC, robustness assessment, and interpretation. It does not replace independent validation, domain-specific clinical interpretation, or a full study-design discussion.
