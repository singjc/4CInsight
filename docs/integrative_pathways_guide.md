# Integrative Analysis, Pathways, and Gene-sharing Networks

This guide describes the **current browser-local 4C Insight workflow**. It complements [the analysis guide](analysis_guide.md) and the searchable **Docs** page inside 4C Insight. All example identifiers and values below are **synthetic**. They are not results from a patient cohort.

The application reads source files selected on your computer and performs the analysis in the browser. It does not upload sample measurements to an analysis server. Downloaded gene-set resources are different: the optional Reactome button requests a public GMT file from Reactome; it does **not** send your genes or samples to Reactome.

## A. Choose files, IDs, and data types

In **Project**, select the cohort source files or folder. In **Sample Metadata**, check the observation IDs, patient IDs, visit annotations, and file-to-sample mapping. Resolve duplicate or unmatched IDs before an analysis.

Open **Integrative Analysis**. Select the metadata source and metadata file, then set these roles:

| Role in the UI | Meaning | Synthetic example |
| --- | --- | --- |
| Observation ID | One measured row or sample | `P01_Baseline` |
| Patient ID | One person, shared across visits | `P01` |
| Visit / timepoint | An optional visit label or date | `Baseline` |

If each person has **one** observation, the same column can identify the observation and patient. For repeated visits, use a distinct Patient ID column. The **One observation per patient** control changes which observations are kept for the joint structure.

Select any available types: **gene expression, transcript expression, BCR repertoire, TCR alpha, TCR beta, BCR isotypes, and immune infiltration**. Click **Run complete analysis**. A single run aligns samples, calculates measured source distances, fuses the available distances, fits the joint structure, checks stability, and prepares measured feature associations. The displayed progress stages describe this work. Unavailable data types and missing source evidence must be reviewed, not silently inferred.

**Worked example — repeated visits**

| Observation ID | Patient ID | Visit | Available measurements |
| --- | --- | --- | --- |
| `A_0` | `Patient A` | Baseline | Gene expression, BCR repertoire |
| `A_1` | `Patient A` | Month 3 | Gene expression, BCR repertoire |
| `B_0` | `Patient B` | Baseline | Gene expression, BCR repertoire |

`A_0` and `A_1` are different observations from the same patient. They must not be described as independent people.

## B. Read the Integrative Overview

The **Overview** first shows the number of **eligible observations**, **observations included in the joint model**, **observations omitted**, and **data types fitted**. Expand **source coverage** and **omitted observations** to see why samples were omitted.

**Worked example — coverage**: Six observations pass the metadata checks. Four have the complete joint distance structure needed for the fit. Two lack required pairwise distances. The Overview reports **6 eligible, 4 included, 2 omitted**. The omission report is the place to inspect the reason. A present workbook does not guarantee that a distance pair is usable.

The **minimum data types per pair** setting affects pairwise distance fusion. It does **not** guarantee that every eligible observation can enter a complete-distance joint clustering. The selected joint cohort is deterministic, but it is not guaranteed to be the largest possible complete cohort.

### Joint sample scatter (PCoA)

The joint PCoA shows variation in the fitted distance structure. You can colour the points by selected metadata, zoom within a fixed coordinate frame, pan, draw a box to zoom, reset, inspect samples, and see labels when there is enough room. PCo1 and PCo2 are **unsupervised axes**, not disease effects. Their signs are arbitrary.

**Worked example — association sign**: Gene `G1` has Spearman correlation `-0.60` with toy PCo1. This means lower `G1` scores occur at the positive end of this particular axis. It does **not** mean `G1` is downregulated in patients. If the PCo1 direction is reversed, the correlation sign changes but the observations do not change.

### Cluster diagnostics and stability

Review cluster sizes and distant observations. A singleton (one-observation cluster) needs inspection; it is not a confirmed disease subtype. The adjusted Rand index from the patient-subset stability analysis measures repeated cluster agreement. High agreement does **not** validate biological labels.

### Data-type agreement and influence

The modality cards describe measured-pair coverage, agreement of source and joint distances, and within/between-cluster distance summaries. The leave-one-modality-out view shows changes in distance correlation and local nearest neighbours when a data type is removed. It is a **descriptive sensitivity check**. It does not provide an independent importance or causal estimate.

### Exploratory feature rankings

The Overview ranks measured features against the joint PCoA coordinates. Gene, transcript, repertoire, isotype, and immune feature correlations are **post-fit associations**. These are not gene-level differential-expression q-values or validated biomarkers. Search the full feature explorer and export all measured associations. Do not select the top five genes and call them a confirmed disease signature.

## C. Move to a defined biological question

Use the **Next biological steps** panel in Overview:

1. Review omitted observations, small clusters, metadata, batch, and visit.
2. Colour the plot by a relevant clinical or technical metadata field.
3. For a predefined case/control question, open **Compare Groups** and run its corrected analysis on the intended cohort. Use count-scale inputs appropriate to the method.
4. Inspect adjusted gene results and compare them with measured repertoire, isotype, and immune features. Record any mismatch in cohorts or comparisons.
5. Use **Pathways** to test biological gene sets and inspect their genes. Seek independent or held-out validation before reporting disease mechanisms.

**Worked example — don't train and test on the same clusters**: Three toy observations belong to two unsupervised clusters. Their recorded clinical labels are Case, Control, Case. Selecting genes using those clusters and then testing the same data against the same clusters is circular. A metadata-defined Case/Control comparison answers a separate question; if it uses the same observations, it is still **not independent validation** of the unsupervised clusters.

## D. Pathways: choose an input, gene sets, and a test

Open **Integrative Analysis → Pathways**. The page uses three numbered steps.

### Step 1 — Choose the research question

| Source | Supported method | Evidence statement |
| --- | --- | --- |
| Corrected Compare Groups gene results | ORA or ranked enrichment | Tied to the specified case/control comparison, sample selection, and model |
| Exploratory joint PCo1 or PCo2 gene scores | Ranked enrichment only | Post-fit association with joint sample variation; not independent disease evidence |

**ORA cannot use PCoA gene associations** as if they were corrected differential-expression tests.

### Step 2 — Choose a gene-set database

Select **Download & use default Reactome**, or upload a **custom GMT/ZIP**. A default Reactome download depends on browser cross-origin access. If it fails, follow the manual link, save the ZIP, and import that file. The browser can read the ZIP directly.

GMT matching uses **exact identifiers after uppercase normalization**. 4C Insight does **not** convert Ensembl IDs to symbols, resolve aliases, or infer orthology. Check that the identifiers in your ranked results and the GMT use compatible naming. A human-symbol GMT is not suitable for a mouse or Ensembl-only ranking without a documented mapping step outside this module.

**Record the organism and exact gene-set release** before scientific reporting. The URL ending in `/current/` changes with Reactome releases and is not a fixed version. The application permits exploratory calculations without this record, but warns against scientific reporting when organism or release is missing.

### Step 3 — Run and interpret

Open **Advanced settings** for gene thresholds, pathway-size limits, permutation count, and random seed. The panel reports the tested gene background, number of tested sets, number with BH q-values at or below 0.05, and the evidence type. Use **Export full pathway results (TSV)** and **Export method and provenance (JSON)** to preserve the calculation and source context.

## E. ORA: corrected gene selection and the hypergeometric test

Over-representation analysis selects genes by corrected **gene-level BH q-value**, **absolute log2 group effect**, and optionally **effect direction**. Its background is **every unique usable tested gene**, not only the selected genes or the genes present in the GMT. Each eligible pathway gets a one-sided hypergeometric p-value, followed by BH correction across the pathways tested.

**Worked example — exact overlap calculation**

- Tested universe: `N = 20` unique genes.
- Corrected selected genes: `n = 5`.
- Tested genes that belong to a pathway: `K = 6`.
- Selected genes in that pathway: `k = 3`.

The probability of overlap at least three is:

\[
P(X \ge 3) = \sum_{j=3}^{5} \frac{\binom{6}{j}\binom{14}{5-j}}{\binom{20}{5}}
\approx 0.1313.
\]

This toy pathway is **not significant even before** the correction across tested pathways. Three matched genes can look interesting but still be consistent with the background.

## F. Ranked enrichment: full gene ranks and permutations

Ranked enrichment uses the **full usable gene list**, not an arbitrary top-five subset. It computes a weighted running-sum enrichment score (**ES**), a normalized score (**NES**), an empirical p-value from shuffled gene labels, and BH-adjusted pathway q-values. The current method is **not phenotype permutation** and **does not reproduce Broad GSEA's pooled FDR calculation**.

**Worked example — resolution with 199 permutations**: The smallest possible empirical p-value is `1 / (199 + 1) = 0.005`. Different pathways can therefore have the same p-value and BH q-value. Many pathways can also overlap in genes, so a long list of low-q pathways is not a list of independent discoveries.

When the input is an exploratory PCoA ranking, the result remains **exploratory** even when its pathway q-value is low. A negative or positive NES for an arbitrarily oriented PCoA axis does not mean a pathway is downregulated or upregulated in disease.

## G. Read a pathway and its gene-level evidence

Select a pathway from the **NES/fold-enrichment chart**, **results table**, or **network**. The selected-pathway inspector shows the pathway identifier, matched genes, scores, and **leading-edge/ORA-selected** status. The leading-edge filter places these genes first but does not remove other genes from the full export. If input genes came from exploratory PCoA, gene-level differential-expression q-values are marked **unavailable** rather than invented.

A Reactome pathway name, such as a disease-related pathway title, describes a gene set. It does not diagnose that disease in the samples.

## H. Read the two network types

### Pathway overlap network

- **Pathway node:** a tested pathway included by the chosen network display limits.
- **Edge:** two pathways share scored genes according to the current gene basis and thresholds.
- **Jaccard:** `|A ∩ B| / |A ∪ B|`.
- **Overlap coefficient:** `|A ∩ B| / min(|A|, |B|)`.
- **Connected group:** a graph component under the current display thresholds, not a statistically validated biological module.

**Worked example — overlap**: `P1={G1,G2,G3}` and `P2={G2,G3,G4}` share `G2,G3`. Their union contains four genes, so **Jaccard = 2/4 = 0.50**, and the overlap coefficient is **2/3 ≈ 0.67**. `P3={G5,G6}` has no edge to either pathway.

### Genes ↔ pathways network

The smaller gene nodes connect to pathway nodes where the scored genes are matched. Select a **blue pathway node** to see its enrichment score, q-value, and measured genes above the graph. Select a **grey gene node** to see its score and the pathways that contain it. Use **Open full pathway gene table** to inspect complete gene evidence.

### Controls and exports

Use **Zoom in/out, wheel zoom, Pan, Box zoom, Fit nodes, Reset, Expand plot, group focus**, and **Show all groups**. The popup uses the same graph and selected-node state. The legend explains colours, node sizes, and edges. Export nodes/edges/groups TSV, SVG, and network JSON. Network display limits do not change the enrichment result.

**Important:** An edge is **not** evidence of protein-protein interaction, physical contact, regulation, or causal signalling. Force-layout positions and connected groups describe this **displayed graph**, not a separate statistical finding.

## I. Refresh, export, privacy, and reproducibility

4C Insight keeps the active session in browser-local storage when storage is available. Browser quotas, private browsing, or cleared storage can prevent restoration of large source files. **Save state** writes a portable JSON with settings and file references, not the original source bytes. Reconnect the source folder when asked. **History** is separate from the active session; **Clear state** clears the active session and its file cache, not the original files.

Before you report a finding, preserve:

- The **exact sample cohort**, metadata role choices, exclusions, comparison, and number of fitted observations.
- The **analysis method**, threshold values, direction, number of permutations, seed, and tested-gene background.
- The **gene-set source**, organism, identifier type, and exact release or file version.
- The **pathway result**, matched/leading-edge genes, and full genes/exported scores.
- The **network options** (gene basis, overlap metric, thresholds, pathway count, seed) for a network figure.
- The **limitations**, including post-fit selection, possible batch effects, missing observations, overlapping pathways, and any independent validation that has or has not been done.

A network figure or low pathway q-value alone is not a validated biomarker, patient subtype, physical interaction, or clinical prediction.

## J. Key references

- Gower JC (1966). *Some distance properties of latent root and vector methods used in multivariate analysis.* Biometrika. https://doi.org/10.1093/biomet/53.3-4.325
- Benjamini Y, Hochberg Y (1995). *Controlling the False Discovery Rate.* JRSS B. https://doi.org/10.1111/j.2517-6161.1995.tb02031.x
- Subramanian A et al. (2005). *Gene set enrichment analysis.* PNAS. https://doi.org/10.1073/pnas.0506580102
- Jassal B et al. (2020). *The Reactome pathway knowledgebase.* Nucleic Acids Research. https://doi.org/10.1093/nar/gkz1031
- Ambroise C, McLachlan GJ (2002). *Selection bias in gene extraction on the basis of microarray gene-expression data.* PNAS. https://doi.org/10.1073/pnas.102102699
- Anderson MJ (2001). *A new method for non-parametric multivariate analysis of variance.* Austral Ecology. https://doi.org/10.1111/j.1442-9993.2001.01070.pp.x
- Sandve GK et al. (2013). *Ten Simple Rules for Reproducible Computational Research.* PLoS Computational Biology. https://doi.org/10.1371/journal.pcbi.1003285
