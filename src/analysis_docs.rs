//! Canonical plain-language documentation for cohort-analysis methods.
//!
//! The GUI and both unified HTML report styles render these same sections so
//! interpretation language does not drift between user-facing surfaces.

#[derive(Debug, Clone, Copy)]
pub struct AnalysisDocSubsection {
    pub title: &'static str,
    pub paragraphs: &'static [&'static str],
    pub bullets: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub struct AnalysisDocSection {
    pub title: &'static str,
    pub summary: &'static str,
    pub default_open: bool,
    pub subsections: &'static [AnalysisDocSubsection],
}

pub fn analysis_doc_sections() -> &'static [AnalysisDocSection] {
    ANALYSIS_DOC_SECTIONS
}

pub static ANALYSIS_DOC_SECTIONS: &[AnalysisDocSection] = &[
    AnalysisDocSection {
        title: "Start here: what the cohort analysis is trying to answer",
        summary: "The application asks whether a defined Case group differs from a defined Control group, while keeping sample identity, repeated sampling, selected covariates, QC decisions, and multi-omics provenance explicit.",
        default_open: true,
        subsections: &[
            AnalysisDocSubsection {
                title: "The basic unit of comparison",
                paragraphs: &[
                    "A biological sample is the basic observation presented to a modality. The standardized metadata table gives that sample a canonical sample_id. Different assays may use different file IDs, but modality-specific aliases are mapped back to the same biological sample rather than being counted as extra observations.",
                    "A subject_id represents the person or participant. If one person has repeated samples, the samples remain separate measurements, but the corrected models can recognize that observations from the same person are correlated rather than fully independent.",
                ],
                bullets: &[
                    "Case and Control are labels for the two groups selected for the current run. They do not have to mean disease and healthy; a custom metadata comparison can represent treatment groups, time-defined cohorts, sites, phenotypes, or any other curated categorical split.",
                    "A covariate is a metadata variable included in the model so the Case/Control effect is estimated after accounting for that variable. Examples might include age, sex, batch, collection period, or another pre-specified factor.",
                    "A sensitivity analysis asks whether the conclusion changes under a reasonable alternative view of the same data. It supports robustness assessment; it is not an independent replication cohort.",
                ],
            },
            AnalysisDocSubsection {
                title: "What counts as primary evidence",
                paragraphs: &[
                    "The corrected / standard analysis is the primary inferential engine. It produces the main effect estimates, uncertainty estimates, p values, false-discovery-rate-adjusted q values, ordinations, compositional tests, and repertoire comparisons.",
                    "Cross-modal QC, simple TPM direction checks, PBMC/source/composition diagnostics, leave-one-out checks, and candidate prioritization are supporting layers around that primary analysis. They help answer whether a result is credible and stable, but they do not create a second independent statistical vote.",
                ],
                bullets: &[
                    "Simplified and full reports are two presentations of the same frozen completed run.",
                    "Changing report style does not rerun statistics or change sample inclusion.",
                    "The software supports discovery and robustness assessment; it does not by itself establish a clinically validated biomarker.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Cohort definition, metadata, sample mapping, and covariates",
        summary: "Before any biology is tested, the app must know which files belong to which biological samples, which samples are Case or Control, which repeated samples belong to the same subject, and which metadata variables should be adjusted for.",
        default_open: true,
        subsections: &[
            AnalysisDocSubsection {
                title: "How custom comparisons are created",
                paragraphs: &[
                    "For a custom comparison, the user selects one metadata column and one or more values for the Case side and one or more values for the Control side. The resulting sample IDs are passed to the same statistical engines used by the built-in 4C comparisons.",
                    "For complex cohort logic, the recommended approach is to curate an explicit analysis_group column in the metadata rather than hide complicated inclusion rules inside the software. This keeps cohort membership visible and auditable.",
                ],
                bullets: &[
                    "sample_id is the canonical biological sample identity.",
                    "subject_id is the participant/person identity used for repeated-measure handling when selected.",
                    "Modality aliases such as gene_expression_id, bcr_id, or tcr_beta_id are matching keys only; they do not create additional biological samples.",
                    "A collaborator-supplied filtered metadata table defines a subset/cohort. It does not invoke a different statistical methodology.",
                ],
            },
            AnalysisDocSubsection {
                title: "How covariates are encoded",
                paragraphs: &[
                    "The adjusted design always contains an intercept and a Case-versus-Control indicator. Selected covariates are then added to that design.",
                    "If every non-missing value in a covariate can be parsed as a finite number, the covariate is treated as numeric, centered at its mean, and divided by its standard deviation. Otherwise it is treated as categorical. Categorical levels are sorted deterministically and represented with indicator columns relative to the first level.",
                    "Samples with missing values for a selected covariate are removed from that adjusted model. The report records how many samples were dropped for missing covariates. A singular or over-specified design fails rather than silently fitting an unstable model.",
                ],
                bullets: &[
                    "The Case/Control coefficient should be read as the group difference after the selected covariates are included in the same model.",
                    "Adjustment cannot rescue a badly confounded study design. If group and batch are nearly the same thing, the model may have little information with which to separate them.",
                    "A subject cannot appear as both Case and Control in the repeated-measure design; that metadata conflict must be resolved before analysis.",
                ],
            },
            AnalysisDocSubsection {
                title: "Why the mapping audit matters",
                paragraphs: &[
                    "A statistical method can only be as correct as the sample mapping feeding it. The mapping audit therefore shows matched, unmatched, ambiguous, and metadata-only samples before analysis.",
                    "An ambiguous alias is not guessed. If one assay ID could map to more than one metadata row, the sample is treated as ambiguous so the curator can fix the metadata rather than allowing silent misassignment.",
                ],
                bullets: &[
                    "Check that expected Case and Control sample counts are present before running the analysis.",
                    "Check modality coverage: a sample can be present for RNA but absent for BCR/TCR/immune data, and each modality reports the samples it actually used.",
                    "Treat mapping changes as provenance changes because they can alter every downstream statistic.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Sample QC and cross-modal outlier analysis",
        summary: "Outlier QC asks whether a sample looks unusually far from other samples in one or more data types. It is deliberately a flag-for-review system, not an automatic sample-deletion system.",
        default_open: true,
        subsections: &[
            AnalysisDocSubsection {
                title: "What representation is used for each modality",
                paragraphs: &[
                    "Each selected modality is converted into a low-dimensional sample representation before outlier scoring. Gene and transcript expression use PCA of variable expression features; repertoire data use PCA of robust-standardized repertoire QC metrics; BCR-isotype data use a CLR-transformed composition representation; immune data use PCA of robust-standardized ssGSEA signatures.",
                    "The outlier score is based on the first two principal coordinates. For each coordinate, the app finds a median center and a robust scale based primarily on the median absolute deviation. The two robust standardized coordinate deviations are combined as the square root of z1 squared plus z2 squared.",
                ],
                bullets: &[
                    "Default expression feature count for outlier representations: 300 variable features.",
                    "Default robust-distance flag threshold: 4.0.",
                    "Default scope: calculate distances within Case and Control separately when each stratum has enough samples; this reduces the chance that a real group-wide biological shift is mislabeled as an outlier.",
                    "If a within-group stratum has fewer than five samples, the implementation falls back to the complete comparison for the robust center/scale calculation.",
                ],
            },
            AnalysisDocSubsection {
                title: "How cross-modal consensus works",
                paragraphs: &[
                    "A sample can be unusual in one assay but ordinary in another. The consensus rule therefore combines only the selected modalities actually available for that sample.",
                    "The default consensus is 'outlier in at least N modalities' with N=2 and at least 2 available modalities. Other choices are any modality, a strict majority of available modalities, or all available selected modalities.",
                ],
                bullets: &[
                    "A consensus flag means the automatic rule was met; it does not mean the sample has been excluded.",
                    "The reviewer can retain an automatically flagged sample or explicitly exclude another sample when provenance or QC evidence supports that decision.",
                    "When reviewed exclusions are applied, the exact automatic flags, manual decisions, excluded IDs, and before/after group counts are frozen into the completed run and exported with it.",
                ],
            },
            AnalysisDocSubsection {
                title: "What an outlier does and does not mean",
                paragraphs: &[
                    "A sample can be statistically unusual because of a technical problem, wrong tissue/source, contamination, unusual cell composition, batch history, a rare but real biological state, or simply small-sample geometry. The score alone cannot distinguish these explanations.",
                    "The strongest reason to exclude a sample is usually external or multi-source evidence: for example confirmed wrong tissue, failed assay QC, a sample-identity problem, or a consistent abnormal pattern across several modalities. A disease-associated sample should not be removed merely because it makes the groups look different.",
                ],
                bullets: &[
                    "Prefer review + provenance + sensitivity analysis over blind automatic filtering.",
                    "Compare inclusive and reasonably QC-restricted results when the exclusion decision could materially affect a leading finding.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Gene-expression corrected analysis",
        summary: "The gene analysis asks, for each tested gene, whether modeled RNA count abundance differs between Case and Control after library-size normalization and any selected covariates, with repeated-subject-aware uncertainty when a subject column is provided.",
        default_open: true,
        subsections: &[
            AnalysisDocSubsection {
                title: "What input values are used",
                paragraphs: &[
                    "The inferential gene model uses the merged Raw Count columns, not TPM, FPKM, or percentile rank. Raw counts retain the mean/variance relationship expected by the negative-binomial count model.",
                    "The app first estimates one size factor per sample using positive-count median ratios. For each gene with at least two positive samples, it computes a geometric mean over the positive counts and contributes sample-to-geometric-mean ratios. The median of those positive ratios gives each sample's relative library-size factor, and the factors are re-centered to geometric mean 1.",
                ],
                bullets: &[
                    "Default feature filter: normalized count at least 10 in at least 20% of modeled samples, with a minimum of three samples required by the implementation.",
                    "Genes failing that support filter are not included in the multiple-testing family and are counted as filtered features.",
                    "For descriptive means in the result table, counts are divided by the size factor. The model itself uses the raw count with log(size factor) as an offset.",
                ],
            },
            AnalysisDocSubsection {
                title: "What statistical model is fitted",
                paragraphs: &[
                    "Each retained gene is fitted with a native NB2 negative-binomial generalized linear model using a log link. In plain language, the model is designed for non-negative count data whose variability can be larger than a simple Poisson model would allow.",
                    "The linear predictor contains Case/Control plus the selected covariates. A per-gene dispersion parameter alpha describes extra count variability and is estimated directly for that gene. Unlike DESeq2 or edgeR, this implementation does not shrink dispersion estimates across genes using an empirical-Bayes framework.",
                    "The reported group coefficient is converted from natural-log units to log2 units. Therefore beta_group is an adjusted log2 fold change: +1 corresponds to an approximately two-fold modeled increase in Case, -1 to an approximately two-fold decrease, and 0 to no modeled group difference.",
                ],
                bullets: &[
                    "Positive beta_group = modeled abundance is higher in Case than Control after adjustment.",
                    "Negative beta_group = modeled abundance is lower in Case than Control after adjustment.",
                    "robust_se is the uncertainty of the group effect in log2 units.",
                    "z_value is beta_group divided by its standard error; the two-sided p value is calculated from that statistic.",
                ],
            },
            AnalysisDocSubsection {
                title: "How repeated samples are handled",
                paragraphs: &[
                    "If a subject column is selected and at least three subject clusters are represented with repeated observations, the model keeps the individual samples but calculates subject-clustered sandwich standard errors. Samples from the same participant can therefore contribute repeated measurements without being treated as completely independent replicates for uncertainty estimation.",
                    "A second sensitivity calculation collapses log2-normalized values within each subject and performs a Case-versus-Control Mann-Whitney comparison on the subject summaries. subject_effect and subject_q_value come from this simpler subject-collapsed view; they are supporting robustness statistics, not a replacement for the primary negative-binomial coefficient.",
                ],
                bullets: &[
                    "Without a selected subject column, each sample behaves as its own subject and the negative-binomial result uses model-based Wald uncertainty.",
                    "If a gene has a strong primary effect but the subject-collapsed direction reverses, inspect whether repeated sampling or a few subjects are driving the result.",
                ],
            },
            AnalysisDocSubsection {
                title: "p values, q values, and what to expect",
                paragraphs: &[
                    "A p value asks how incompatible the observed group coefficient is with a no-group-effect model under the assumptions used for that gene. Because thousands of genes are tested, a small p value alone is not enough.",
                    "The app applies the Benjamini-Hochberg procedure across the tested genes to create q values. A q value is an FDR-adjusted measure: using a threshold such as q<=0.05 is intended to control the expected proportion of false discoveries among the set called significant under that threshold, not to give a 95% probability that one particular gene is true.",
                ],
                bullets: &[
                    "Interpret q value together with effect size, sample support, QC sensitivity, and biological plausibility.",
                    "A very small q value with a tiny effect may be statistically strong but biologically modest; a large effect with poor support may be unstable.",
                    "No universal q or fold-change cutoff makes a feature a validated biomarker.",
                ],
            },
            AnalysisDocSubsection {
                title: "PCA and dendrogram for gene expression",
                paragraphs: &[
                    "For unsupervised sample structure, the app uses up to the 300 most variable retained features after size-factor normalization and log2(x+0.5) transformation. PCA compresses that many-gene space into two principal axes for visualization.",
                    "The dendrogram uses Euclidean distances in the same top-variable normalized feature space and average-linkage hierarchical clustering. Neither plot knows which genes were statistically significant; they summarize broad sample similarity.",
                ],
                bullets: &[
                    "Case/Control separation can support the idea of a broad phenotype-associated expression shift, but it is not itself a differential-expression test.",
                    "Clustering by subject, batch, collection period, tissue/source, or another metadata field can reveal important alternative structure.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Transcript-expression and differential transcript-usage analysis",
        summary: "Transcript analysis contains two related questions: whether individual transcript abundance differs between groups, and whether the relative use of transcripts within the same gene changes between groups.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "Differential transcript abundance",
                paragraphs: &[
                    "The transcript abundance analysis uses Total Reads columns as count input and otherwise follows the same corrected count-model logic as gene expression: positive-count median-ratio size factors, low-count support filtering, a negative-binomial group/covariate model, optional subject-clustered uncertainty, BH q values, and a subject-collapsed sensitivity result.",
                    "The beta_group value therefore describes an adjusted log2 fold change for that transcript's total abundance between Case and Control.",
                ],
                bullets: &[
                    "A transcript can change because the whole gene changes, because isoform usage changes, or both.",
                    "Transcript abundance tests should not automatically be interpreted as differential splicing; the DTU screen addresses relative within-gene usage separately.",
                ],
            },
            AnalysisDocSubsection {
                title: "Differential transcript usage (DTU)",
                paragraphs: &[
                    "For genes with multiple transcripts, the DTU screen asks whether the composition of transcript counts within a gene differs between groups. It first requires the gene-level total transcript count to pass the same general count-support concept.",
                    "Within each eligible gene, every transcript count receives a pseudocount of 0.5 and is transformed with a centered log-ratio (CLR): log(transcript+pseudocount) minus the average log abundance of the gene's transcripts in that sample. This changes the question from 'how much transcript is there?' to 'is this transcript relatively more or less used compared with the other transcripts from the same gene?'.",
                    "Each transcript CLR value is modeled with the same adjusted linear-model framework used for other continuous features. Transcript p values within a gene are combined with the Simes procedure to form a gene-level DTU p value, and those gene-level p values are BH-adjusted across tested genes.",
                ],
                bullets: &[
                    "Positive transcript CLR effect = that transcript occupies a larger relative share of its gene's transcript mixture in Case, after adjustment.",
                    "max_abs_clr_effect summarizes the largest absolute transcript-level CLR effect among tested transcripts for the gene.",
                    "The screen suppresses essentially absent transcripts before multiple-testing correction when mean usage is below 0.001 and the transcript lacks sufficient prevalence.",
                    "This is a dependency-light DTU screen, not DEXSeq/DRIMSeq Dirichlet-multinomial inference; the report states that limitation explicitly.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "BCR and TCR repertoire analysis",
        summary: "Repertoire analysis asks several different biological questions: how diverse or dominated each sample's receptor repertoire is, how clonotypes are shared, whether whole repertoires are globally similar, and whether V/D/J or V-J compositions shift between groups.",
        default_open: true,
        subsections: &[
            AnalysisDocSubsection {
                title: "What the app treats as a clonotype",
                paragraphs: &[
                    "Clone Count is the abundance weight read from each repertoire workbook row. For clonotype-sharing and repertoire-distance calculations, the canonical amino-acid clonotype key is built from an unambiguous V call, an unambiguous J call, and the CDR3 amino-acid sequence. Rows lacking a usable CDR3 or unambiguous V/J under the selected ambiguity policy do not contribute to that amino-acid clonotype key.",
                    "V/D/J composition tables are related but separate: they use clone-count-weighted feature fractions after applying the selected ambiguity policy. BCR includes isotype, V, D, and J compositions; TCR includes V, J, and V-J-pair compositions.",
                ],
                bullets: &[
                    "Default ambiguity policy is conservative: exclude comma-separated tied gene calls from that composition.",
                    "Other options split weight across tied hits, take the first reported hit, or collapse tied calls to gene family.",
                ],
            },
            AnalysisDocSubsection {
                title: "Clonality: exactly what it means here",
                paragraphs: &[
                    "Clonality is calculated separately for each sample across that sample's entire observed clonotype abundance distribution. It is not calculated separately for every V gene, CDR3 type, or clone, and the raw clonality number is not itself a pooled Case-versus-Control statistic.",
                    "For one sample, the app calculates Shannon entropy from the clonotype proportions and then computes clonality = 1 - Shannon / ln(observed richness). This normalizes entropy by the maximum entropy possible for the observed number of clonotypes. Values near 0 indicate a relatively even repertoire; larger values indicate that a smaller subset of clonotypes dominates the reads. A single-clonotype repertoire does not receive a meaningful normalized clonality value in the corrected summary.",
                    "After every sample has one clonality value, those sample-level values are passed to the adjusted Case/Control model. Therefore the reported BCR/TCR 'Clonality' beta, p, and q answer whether average sample-level repertoire clonality differs between Case and Control after the selected covariates and repeated-subject handling.",
                ],
                bullets: &[
                    "Higher clonality does not mean 'more clonotypes'. Richness measures how many clonotypes are observed; clonality measures how unevenly abundance is concentrated among them.",
                    "Two samples can have the same richness but different clonality if one has a few dominant clones and the other is more even.",
                    "A Case-versus-Control clonality effect is a group comparison of one summary number per sample, not a statement that one particular clone expanded.",
                ],
            },
            AnalysisDocSubsection {
                title: "Matched-depth richness and Shannon diversity",
                paragraphs: &[
                    "Sequencing depth strongly affects how many clonotypes are observed. To make diversity summaries more comparable, the corrected analysis rarefies every repertoire to a common read depth: the smallest total positive clone count among modeled samples, capped by the user-selected rarefaction cap.",
                    "Rarefaction is exact sampling without replacement from the clone-count distribution. The default is 3 rarefaction replicates and a depth cap of 50,000 reads; replicate richness and Shannon values are averaged.",
                ],
                bullets: &[
                    "Matched-depth richness = the number of clonotypes observed after downsampling to the common depth.",
                    "Matched-depth Shannon diversity combines richness and evenness after that depth matching; larger values generally mean a more diverse/even repertoire.",
                    "Observed richness and unrarefied Shannon are retained as sample summaries, but the corrected group-test table prioritizes matched-depth richness and matched-depth Shannon to reduce depth bias.",
                ],
            },
            AnalysisDocSubsection {
                title: "Other per-sample repertoire metrics",
                paragraphs: &[
                    "Top clone fraction is the abundance of the largest clone divided by total clone abundance in that sample. A high value means one clone accounts for a large fraction of the repertoire.",
                    "Weighted mean CDR3 amino-acid length averages CDR3 length using Clone Count as the weight, so abundant clonotypes contribute more strongly than rare clonotypes.",
                ],
                bullets: &[
                    "Every metric is first calculated per sample, then Case and Control are compared with the adjusted linear-model framework.",
                    "For continuous repertoire summaries, beta_group is the adjusted difference in the metric's native units, not a log2 fold change unless the metric itself is on a log scale.",
                ],
            },
            AnalysisDocSubsection {
                title: "Private, longitudinally shared, and public clonotype read fractions",
                paragraphs: &[
                    "The app examines where each amino-acid clonotype appears across samples and subjects, then reports the fraction of a sample's clonotype reads falling into three sharing categories.",
                    "With a subject column, a clonotype observed in at least two different subjects is public. A clonotype seen in at least two samples from the same subject but not in multiple subjects is longitudinally shared. Remaining clonotype reads are private. The fractions are read-weighted, so an abundant shared clone contributes more than a very rare shared clone.",
                    "Without a subject column, longitudinal sharing cannot be distinguished properly and the app warns that sharing is effectively sample-based.",
                ],
                bullets: &[
                    "Private fraction asks how much of a sample's clonotype abundance is unique to that sample/subject context.",
                    "Longitudinally shared fraction asks how much is repeatedly observed within the same person across samples.",
                    "Public fraction asks how much belongs to clonotypes seen in at least two different subjects in the analyzed dataset.",
                    "These definitions are dataset-relative: adding or removing subjects can change whether a clonotype is classified as public.",
                ],
            },
            AnalysisDocSubsection {
                title: "TCR amino-acid convergence",
                paragraphs: &[
                    "For TCR modalities, the app can identify amino-acid clonotypes for which more than one distinct nucleotide clonal sequence produces the same V/J/CDR3-amino-acid clonotype key. It sums the reads assigned to those convergent amino-acid clonotypes and divides by total clone count.",
                    "The resulting AA convergence read fraction is one number per TCR sample and is then compared between Case and Control with the adjusted metric model.",
                ],
                bullets: &[
                    "A higher value means a larger fraction of repertoire reads belongs to amino-acid receptor solutions reached by more than one nucleotide sequence in the observed data.",
                    "This metric depends on the nucleotide sequence field being available and on the clonotype-key definition used by the app.",
                ],
            },
            AnalysisDocSubsection {
                title: "Whole-repertoire distances: Morisita-Horn and Jaccard",
                paragraphs: &[
                    "The app also compares samples using the full set of amino-acid clonotypes instead of reducing each sample to one diversity number.",
                    "Morisita-Horn similarity is abundance-weighted: common/high-abundance clonotypes matter strongly. The analysis uses 1 - Morisita-Horn similarity as a distance. Jaccard focuses on presence/absence: it asks how much the sets of observed clonotypes overlap regardless of their counts.",
                ],
                bullets: &[
                    "If Morisita-Horn separates groups but Jaccard does not, abundance structure may differ more than simple clonotype membership.",
                    "If Jaccard separates groups, the sets of clonotypes present differ, even if abundance weighting is ignored.",
                    "PCoA turns the distance matrix into a two-dimensional map for visualization; nearby samples have more similar repertoires under that distance.",
                    "PERMANOVA tests whether the Case/Control grouping explains global distance structure. Default permutation count is 499. When subject IDs are supplied, labels are permuted at the subject-block level so repeated samples from one subject move together.",
                    "The Morisita-Horn dendrogram uses average-linkage hierarchical clustering of the same abundance-weighted distance and is descriptive rather than a separate hypothesis test.",
                ],
            },
            AnalysisDocSubsection {
                title: "V/D/J, V-J, and repertoire composition tests",
                paragraphs: &[
                    "Gene-segment and isotype fractions are compositional: all fractions in a family compete for a fixed total. Increasing one component necessarily changes the relative fractions of others, so the app does not treat raw percentages as independent outcomes.",
                    "Features are retained by default when their mean fraction is at least 0.002 and they are present in at least 5% of samples. A small pseudocount of 1e-6 is added, then each sample is CLR-transformed by subtracting the mean log abundance across retained components. The adjusted linear model is fitted to each CLR component.",
                ],
                bullets: &[
                    "Positive CLR beta = the component is relatively more prominent in Case compared with the geometric center of the other retained components.",
                    "Negative CLR beta = relatively less prominent in Case.",
                    "Aitchison distance is Euclidean distance in CLR space. PCoA/PERMANOVA and the dendrogram summarize whole-composition differences, while component tests identify contributors.",
                    "Because CLR effects are relative, do not read a positive V-gene effect as an absolute cell count increase without additional evidence.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "BCR isotype composition analysis",
        summary: "The dedicated BCR-isotype modality asks whether the relative mixture of antibody isotypes differs between groups using the precomputed Isotype Read Fraction values from the isotype workbook.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "What values are used",
                paragraphs: &[
                    "For each sample, rows are read from the IGH Isotype/Isotype and Isotype Read Fraction columns. Fractions for the same isotype are summed and then renormalized so the retained isotypes sum to one.",
                    "The user can keep Undetermined as part of the composition or exclude Undetermined and renormalize the remaining determined isotypes. This option changes the composition being interpreted and is therefore recorded in the run provenance.",
                ],
                bullets: &[
                    "This dedicated isotype file analysis is distinct from the BCR repertoire workbook's clone-count-weighted Isotype composition, although both are compositional questions.",
                    "The same CLR/Aitchison framework, feature filtering, covariate adjustment, PCA/PCoA-style ordination logic, PERMANOVA, and hierarchical clustering concepts apply.",
                ],
            },
            AnalysisDocSubsection {
                title: "How to interpret an isotype effect",
                paragraphs: &[
                    "An isotype CLR coefficient describes a relative shift in the isotype mixture. It does not directly estimate the absolute number of B cells of that isotype.",
                    "A positive effect means the isotype occupies a larger relative position in the Case composition after adjustment; a negative effect means a smaller relative position. Because the components sum to one, interpretation should consider the whole mixture rather than one percentage in isolation.",
                ],
                bullets: &[
                    "Use the whole-composition ordination/PERMANOVA to ask whether isotype mixtures differ globally.",
                    "Use individual CLR tests to identify which isotypes contribute to that relative difference.",
                    "Compare results with and without Undetermined when that category is large or differs by group.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Immune-infiltrate / ssGSEA analysis",
        summary: "The immune analysis compares per-sample ssGSEA signature scores for immune-cell labels and separately summarizes the overall pattern of signatures across samples.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "What an ssGSEA score means here",
                paragraphs: &[
                    "The input workbook provides an Immune Cell Type and an ssGSEA Score for each sample. The score is an enrichment score summarizing how strongly the expression profile supports that gene signature in that sample.",
                    "An ssGSEA score is not a literal percentage of cells and should not be interpreted as a direct flow-cytometry cell fraction unless separately calibrated and validated for that purpose.",
                ],
                bullets: &[
                    "Each immune-cell signature is tested on its original ssGSEA scale using the adjusted linear model.",
                    "Positive beta_group = higher adjusted ssGSEA score in Case; negative beta_group = lower adjusted score in Case.",
                    "BH q values correct for testing multiple immune signatures.",
                ],
            },
            AnalysisDocSubsection {
                title: "Immune PCA, clustering, and PERMANOVA",
                paragraphs: &[
                    "For sample-structure plots, each immune signature is standardized across samples to a z-score so signatures with larger numeric scales do not automatically dominate the geometry. Euclidean distances across those standardized signature rows summarize overall immune-profile difference.",
                    "PCA displays the strongest coordinated axes of immune-signature variation. PERMANOVA tests whether the global standardized immune profile differs between Case and Control, with subject-blocked permutations when repeated-subject information is available. The dendrogram shows average-linkage clustering of the same standardized profile distances.",
                ],
                bullets: &[
                    "A significant individual signature does not necessarily imply a change in literal cell abundance.",
                    "A strong PCA axis may reflect biology, tissue/source, batch, or a broad compositional shift; color the plot by relevant metadata when possible.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Pathway over-representation analysis (ORA)",
        summary: "Pathway ORA asks whether genes already selected from the corrected gene analysis are represented in a pathway more often than expected from the set of genes that were actually tested.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "How the foreground and background are defined",
                paragraphs: &[
                    "The background/universe is the set of gene symbols that passed expression filtering and received a corrected gene test. This is preferable to using every gene in the genome because only tested genes had a chance to enter the foreground.",
                    "The foreground contains tested genes meeting both a q-value threshold and an absolute adjusted log2 fold-change threshold. The GUI defaults are q<=0.05 and |adjusted log2FC|>=0.5, but the run stores the exact user-selected thresholds.",
                ],
                bullets: &[
                    "Only GMT sets with 10 to 500 genes overlapping the measured universe are tested.",
                    "The source GMT path/collection is recorded in the full run provenance.",
                    "Changing the foreground thresholds can materially change ORA results because it changes which genes are called members of the selected list.",
                ],
            },
            AnalysisDocSubsection {
                title: "Hypergeometric p value, odds ratio, overlap, and q value",
                paragraphs: &[
                    "For each pathway, the hypergeometric upper-tail test asks whether the observed number of foreground genes in that pathway is unusually large given the size of the measured universe, the size of the foreground, and the number of measured pathway genes.",
                    "The odds ratio summarizes enrichment strength. Values above 1 indicate that foreground genes are over-represented in the pathway relative to the rest of the tested universe. A 0.5 continuity correction is used in the odds-ratio calculation to avoid undefined divisions when a contingency-table cell is zero.",
                ],
                bullets: &[
                    "Overlap = number of foreground genes belonging to the pathway.",
                    "Measured set size = number of pathway genes that were actually in the tested-gene universe.",
                    "p value = enrichment evidence before correcting for the number of pathways tested.",
                    "BH q value = FDR-adjusted pathway enrichment result.",
                    "Overlap genes are shown so the biological interpretation can be traced back to the gene-level result.",
                ],
            },
            AnalysisDocSubsection {
                title: "What pathway significance does not prove",
                paragraphs: &[
                    "ORA is a gene-list enrichment calculation. It does not use the full ranked gene distribution and it does not prove that a pathway is activated, inhibited, causal, or specific to the disease mechanism.",
                    "Closely related pathways frequently share many genes, so ten significant pathways can represent one underlying biological program rather than ten independent discoveries.",
                ],
                bullets: &[
                    "Interpret pathway direction by returning to the contributing gene effects; ORA itself is primarily an over-representation test.",
                    "Prioritize coherent themes supported by robust genes and reasonable sample-level evidence rather than counting pathway names.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "PBMC, source, composition, and candidate-robustness diagnostics",
        summary: "These diagnostics ask whether striking gene signals could be explained or strongly influenced by unusual sample source, cell-composition structure, sparse counts, one or two samples, or a historical/family/batch level.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "Sample-level expression and immune QC",
                paragraphs: &[
                    "The diagnostics scan the merged gene matrix to calculate library raw-count totals, numbers of detected genes, genes with counts at least 5 or 10, and whole-transcriptome PCA using a default of the top 300 variable log2(TPM+1) genes.",
                    "Transcriptome and immune PCA outlier scores use robust distance in the first two principal coordinates. The default diagnostic threshold is 4.0. These automatic diagnostic outliers are flags by default; they are not automatically added to the main analysis exclusion list.",
                ],
                bullets: &[
                    "Known non-PBMC/wrong-source IDs and reviewer-supplied suspect IDs can be carried explicitly into the diagnostic record.",
                    "The myeloid-high / lymphoid-low score is a descriptive signature-imbalance measure used to identify broad composition differences; it is not a clinical cell-count assay.",
                    "When subject IDs are available, longitudinal PCA distances can highlight a historical sample that is unusually far from later samples from the same participant.",
                ],
            },
            AnalysisDocSubsection {
                title: "Candidate sample-support statistics",
                paragraphs: &[
                    "For leading corrected genes, the diagnostics return to the raw counts and TPM values of every Case and Control sample. This makes it possible to see whether a group effect is broadly supported or concentrated in a few observations.",
                ],
                bullets: &[
                    "Detection fraction = fraction of samples with raw count > 0.",
                    "Count>=5 and Count>=10 fractions summarize how many samples have more substantial count support.",
                    "Default low-count warning: fewer than 20% of all analyzed samples have count >=10 for the candidate.",
                    "Top-two contribution = fraction of the group's total TPM contributed by the two highest-TPM samples. Default concentration warning threshold is 0.50.",
                    "Case/control raw-count and TPM medians are shown because a large mean effect can coexist with a near-zero median when only a few samples are high.",
                ],
            },
            AnalysisDocSubsection {
                title: "Leave-one-out and leave-level-out stability",
                paragraphs: &[
                    "Leave-one-sample-out repeatedly removes one Case sample or one Control sample and recalculates a simple log2 mean TPM effect. The reported minimum and maximum show how much the descriptive effect can move when any single sample is omitted.",
                    "If a sensitivity metadata column is selected, leave-level-out removes one complete level at a time, such as one batch, family, year, or collection period, then recalculates the descriptive effect. A range crossing zero indicates a sign reversal under at least one omission.",
                ],
                bullets: &[
                    "A sign-stable range supports robustness to the tested omission; it does not guarantee robustness to every unmeasured confounder.",
                    "A sign reversal is a strong reason to inspect which sample or metadata level is driving the result.",
                ],
            },
            AnalysisDocSubsection {
                title: "QC-restricted, immune-adjusted, and source-effect comparisons",
                paragraphs: &[
                    "The QC-restricted sensitivity model reruns the corrected gene model after the configured sensitivity exclusions. Its effect and q value can be compared with the primary corrected result.",
                    "When enough immune-profile data are available, an immune-adjusted gene model adds major immune principal-component covariates. If a disease coefficient collapses or reverses after those broad composition axes are included, the original association may be composition-sensitive.",
                    "A suspect-versus-ordinary source-effect screen compares reviewer-designated unusual samples with ordinary samples without using disease status. A strong source effect overlapping the disease effect is evidence that sample source/composition and disease may be entangled.",
                ],
                bullets: &[
                    "QC-restricted sign reversal = the group effect changes direction after the sensitivity exclusions.",
                    "Immune-adjusted sign reversal = the group effect changes direction after major immune PCs are included.",
                    "Effect attenuation flag = baseline absolute effect is >0.25 and the sensitivity-model absolute effect drops below half the baseline magnitude.",
                ],
            },
            AnalysisDocSubsection {
                title: "Regularized effect and robustness labels",
                paragraphs: &[
                    "The diagnostic regularized_log2fc is a shrinkage-style display statistic: the corrected beta is multiplied by tau-squared divided by tau-squared plus the squared robust standard error. Tau-squared is estimated from the robust spread of corrected effects across genes, with a minimum prior variance. No p or q value is generated from this diagnostic shrinkage value.",
                    "Candidate classifications summarize a specific rule set rather than an external validation result. Sign reversals or sign-unstable omission ranges produce 'confounding-sensitive'; low-count or top-two concentration flags produce 'outlier / low-count sensitive'; a baseline q<=0.05 plus a same-direction QC-restricted result with q<=0.10 can produce 'robust preliminary candidate'; otherwise the label is 'needs review'.",
                ],
                bullets: &[
                    "A robustness label is a concise annotation of the implemented checks, not a probability that the gene is biologically true.",
                    "The underlying flags and actual effect/q values should be inspected whenever a candidate matters scientifically.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Candidate prioritization and the simplified report",
        summary: "Candidate prioritization is a ranking/presentation layer built from the completed corrected results. It helps decide what to inspect first; it does not create new evidence.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "How candidates are shortlisted",
                paragraphs: &[
                    "The candidate layer ranks corrected gene, transcript, pathway, and other-modality results using their existing corrected statistical support and effect magnitude. The default presentation settings keep up to 15 genes, 10 transcripts, 12 pathways, and 20 other modality signals, with a default minimum absolute gene log2 fold change of 0.5.",
                    "Evidence labels such as FDR-supported preliminary signal or promising preliminary signal are communication labels derived from q/p thresholds. They are intentionally phrased as preliminary.",
                ],
                bullets: &[
                    "Exploratory TPM direction concordance can annotate whether a simple descriptive gene effect points in the same direction; it does not alter the corrected result.",
                    "Pathway support can annotate a gene when it belongs to shortlisted enriched pathways; pathway membership does not make the gene independently significant.",
                    "PBMC/confounding robustness labels can be attached when diagnostics were run.",
                ],
            },
            AnalysisDocSubsection {
                title: "What the simplified report is for",
                paragraphs: &[
                    "The simplified report deliberately shows fewer result rows and plots so a collaborator can see the comparison, QC summary, strongest preliminary candidates, pathway themes, multi-modal signals, and limitations without reading the entire technical audit.",
                    "It is still generated from exactly the same completed run as the full report. The documentation appendix is included in both report styles so readers can look up what the displayed statistics mean.",
                ],
                bullets: &[
                    "Use 'candidate', 'preliminary signal', or 'finding for follow-up'.",
                    "Avoid 'validated biomarker' unless there is genuinely independent validation beyond this analysis.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "How to read PCA, PCoA, PERMANOVA, dendrograms, and effect plots",
        summary: "These plots answer different questions. Ordination and clustering describe sample-to-sample structure; feature tables and effect plots test individual features; PERMANOVA tests a global distance-based group difference.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "PCA",
                paragraphs: &[
                    "Principal component analysis starts from a sample-by-feature geometry and finds directions that explain large amounts of variation. PC1 is the strongest such direction, PC2 is the next orthogonal direction. The percentages in axis labels show how much of the represented variation each plotted axis explains.",
                    "PCA is unsupervised: the Case/Control label is not used to choose the axes. Group separation is therefore descriptive context, not a p value for the phenotype.",
                ],
                bullets: &[
                    "Points close together have similar profiles in the feature space used for that PCA.",
                    "Coloring by subject, batch, timepoint, site, tissue/source, or another metadata field can reveal competing explanations for the visible structure.",
                ],
            },
            AnalysisDocSubsection {
                title: "PCoA and distance choice",
                paragraphs: &[
                    "Principal coordinates analysis starts from a distance matrix rather than directly from raw features. The meaning of 'close together' therefore depends on the distance: Morisita-Horn emphasizes abundance-weighted repertoire overlap, Jaccard emphasizes presence/absence, and Aitchison reflects relative composition after CLR transformation.",
                ],
                bullets: &[
                    "Always read the plot title/distance label before interpreting the geometry.",
                    "Two PCoA plots from the same samples can legitimately look different because they ask different similarity questions.",
                ],
            },
            AnalysisDocSubsection {
                title: "PERMANOVA",
                paragraphs: &[
                    "PERMANOVA is a global distance-based test. It compares the amount of squared distance associated with the Case/Control split with the amount remaining within groups, then uses label permutations to ask how unusual the observed pseudo-F statistic is.",
                    "The app uses the configured permutation count, default 499, and permutes at subject-block level when subject IDs are provided. A small PERMANOVA p value means group labels align with the chosen distance structure more strongly than expected under those permutations.",
                ],
                bullets: &[
                    "PERMANOVA does not identify which feature caused the separation.",
                    "A significant PERMANOVA can coexist with substantial overlap between groups in the plot.",
                    "Distance-based group differences can reflect location and/or dispersion structure, so interpretation should remain tied to study design and complementary feature-level results.",
                ],
            },
            AnalysisDocSubsection {
                title: "Dendrogram",
                paragraphs: &[
                    "Average-linkage hierarchical clustering starts with individual samples and repeatedly joins the clusters with the smallest average between-cluster distance. The dendrogram displays that nested sequence of joins.",
                ],
                bullets: &[
                    "Nearby leaves/branches indicate similarity under the named distance, not necessarily shared diagnosis.",
                    "Branch height is related to the distance at which clusters join; it is not a p value.",
                ],
            },
            AnalysisDocSubsection {
                title: "Volcano/effect-significance and result tables",
                paragraphs: &[
                    "An effect-significance scatter places effect size on the horizontal axis and statistical support on the vertical axis. In corrected gene/transcript results, left/right usually correspond to negative/positive adjusted effects; higher points have smaller q values or stronger significance transformations.",
                ],
                bullets: &[
                    "Look for a useful combination of effect magnitude, q value, sample support, and robustness rather than ranking by one axis alone.",
                    "Case mean and Control mean in corrected tables are descriptive sample summaries; the adjusted beta is the model-based group effect and can differ from the raw difference when covariates matter.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "Statistical glossary: common terms in the results",
        summary: "This glossary translates the recurring statistical terms into the specific role they play in 4C Cohort Explorer.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "Effects and uncertainty",
                paragraphs: &[],
                bullets: &[
                    "Effect / beta_group — the fitted Case-versus-Control coefficient after selected covariates. For count expression it is reported as adjusted log2 fold change; for continuous metrics/CLR/ssGSEA it is a difference in the modeled scale.",
                    "log2 fold change — base-2 log of a ratio. +1 is approximately 2x Case vs Control; -1 is approximately 0.5x; +2 is approximately 4x.",
                    "SE / standard error — estimated uncertainty of the fitted group effect. Smaller SE means the coefficient is estimated more precisely under the model.",
                    "Sandwich / robust SE — an uncertainty estimate designed to be less dependent on a perfectly specified residual variance model; subject-clustered versions also account for correlation within a subject cluster.",
                    "z value — fitted effect divided by its standard error. Larger absolute z values generally produce smaller two-sided p values.",
                    "Dispersion alpha in NB2 — gene-specific extra-Poisson variance. In the NB2 parameterization, variability increases beyond the mean as alpha grows.",
                ],
            },
            AnalysisDocSubsection {
                title: "Significance and multiple testing",
                paragraphs: &[],
                bullets: &[
                    "p value — a model-based measure of how extreme the observed statistic is under a null/no-group-effect reference. It is not the probability that the null hypothesis is true.",
                    "BH q value — Benjamini-Hochberg FDR-adjusted p value across a family of tests. It helps control the expected false-discovery proportion among discoveries selected at a q threshold.",
                    "FDR — false discovery rate, the expected fraction of called discoveries that are false under the method's assumptions and selection rule.",
                    "Simes p value — a way to combine several related p values, used here to summarize transcript-level DTU evidence into a gene-level test.",
                    "PERMANOVA p value — permutation-derived evidence for a global group difference in a selected distance matrix; it is not a feature-level p value.",
                ],
            },
            AnalysisDocSubsection {
                title: "Diversity and composition",
                paragraphs: &[],
                bullets: &[
                    "Richness — number of distinct observed clonotypes. It ignores how evenly their abundances are distributed.",
                    "Shannon diversity — an entropy measure that increases with both the number of clonotypes and how evenly abundance is spread across them.",
                    "Clonality — 1 minus Shannon divided by log richness in this app; larger values mean more abundance concentration/dominance within a sample.",
                    "CLR — centered log-ratio transformation. Each component is expressed as its log abundance relative to the geometric center of the other retained components in that sample.",
                    "Aitchison distance — Euclidean distance between CLR-transformed compositions; appropriate for relative compositional data.",
                    "Morisita-Horn — abundance-weighted repertoire similarity; the app uses 1-similarity as a distance.",
                    "Jaccard — presence/absence overlap between clonotype sets.",
                    "Rarefaction — subsampling without replacement to a common read depth so diversity comparisons are less driven by sequencing depth.",
                ],
            },
            AnalysisDocSubsection {
                title: "Descriptive versus inferential values",
                paragraphs: &[],
                bullets: &[
                    "Mean / median — descriptive summaries of the observed sample values. They do not automatically include covariate adjustment.",
                    "Detection fraction — fraction of samples with a positive count; useful for identifying sparse signals.",
                    "Top-two contribution — proportion of a group's total TPM supplied by its two highest samples; high values flag concentration of the signal.",
                    "PCA/PCoA coordinates — visualization coordinates, not effect sizes or significance probabilities.",
                    "Robust outlier distance — distance from a median-centered, robustly scaled PC1/PC2 center; it is a QC score, not a disease association statistic.",
                ],
            },
        ],
    },
    AnalysisDocSection {
        title: "What conclusions are reasonable, and what still needs validation",
        summary: "A strong result combines correct mapping, adequate sample support, meaningful effect size, FDR support, sensible sample-level behavior, and stability across reasonable QC/confounding checks. Even then, it remains a discovery result until replicated independently.",
        default_open: false,
        subsections: &[
            AnalysisDocSubsection {
                title: "A practical interpretation sequence",
                paragraphs: &[],
                bullets: &[
                    "1. Verify cohort definition, canonical sample mapping, and modality coverage.",
                    "2. Review cross-modal QC flags and provenance before excluding anything.",
                    "3. Read the corrected effect and q value as the primary feature-level evidence.",
                    "4. Check whether the signal is broadly supported across samples rather than driven by one or two extremes.",
                    "5. Inspect repeated-subject, batch/family/time, source/composition, and QC-restricted sensitivity results when available.",
                    "6. Look for coherent multi-modal or pathway context without treating correlated analyses as independent replications.",
                    "7. Use the candidate shortlist to prioritize follow-up, then seek independent replication or a pre-specified validation cohort before biomarker claims.",
                ],
            },
            AnalysisDocSubsection {
                title: "Important limitations of the current implementation",
                paragraphs: &[
                    "The native RNA count model is intentionally dependency-light and does not reproduce the empirical-Bayes dispersion/effect shrinkage machinery of DESeq2, edgeR, or limma-voom. The DTU screen is also not a replacement for dedicated DEXSeq/DRIMSeq-style models.",
                    "Automated QC and sensitivity analyses can expose obvious fragility but cannot detect every confounder, batch effect, or biological source of heterogeneity. Small cohorts can produce unstable estimates and unstable outlier geometry.",
                    "Pathway and multi-omic concordance are useful biological context but are not automatically independent evidence because the data types and derived features can be correlated.",
                ],
                bullets: &[
                    "Prefer transparent wording: preliminary signal, candidate gene, candidate pathway, repertoire difference, or finding for follow-up.",
                    "Reserve validated biomarker/clinical marker language for appropriately designed independent validation.",
                ],
            },
        ],
    },
];
