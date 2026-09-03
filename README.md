# 4C Cohort Explorer

A Rust application for memory-bounded RNA expression merging plus metadata-driven multi-modal cohort exploration across gene/transcript expression, BCR, BCR isotypes, TCR alpha/beta, and immune-infiltrate data.

The project contains both a command-line interface and an `egui` desktop interface. The desktop application is intended to let a collaborator select the reports, validate the sample names, choose exactly which measurements to export, merge the matrix, and perform lightweight QC/exploration without using R.

## What v0.15.1 adds

- Adds a dedicated **Metadata Builder** desktop tab for creating, curating, validating, and exporting a standardized one-row-per-sample multi-omics metadata TSV.
- Defines a 37-field standard schema covering canonical sample/subject identity, a recommended `analysis_group`, disease/cohort labels, longitudinal collection fields, specimen/source provenance, common covariates, technical batches, QC-review notes, and optional per-modality sample-ID aliases.
- Adds `templates/4C_multiomics_sample_metadata_template.tsv` plus `docs/sample_metadata_schema.md` as the repository-level metadata contract.
- Adds **custom metadata comparisons** to the unified Cohort Analysis workflow. Select any metadata column, assign one or more values to Case and Control, optionally provide presentation labels/name, then run the exact same corrected/statistical engine used by the built-in comparisons.
- Keeps the historical **HYP vs True_HC** and **HYP+ME/CFS vs ME/CFS-only** definitions as convenient built-in presets rather than hard-coded requirements.
- Adds canonical sample alias resolution for `gene_expression_id`, `transcript_expression_id`, `bcr_id`, `bcr_isotype_id`, `tcr_alpha_id`, `tcr_beta_id`, and `immune_infiltrate_id`. Modality aliases map back to one canonical `sample_id`, so they do not inflate case/control sample counts.
- Preserves custom/extra metadata columns and keeps them available for corrected-model covariates and ordination coloring.
- Records the exact built-in or custom comparison definition in the frozen unified run/full report. Rows not assigned to either custom group are simply outside that comparison.

## What v0.15.0 adds

- Replaces the user-facing Exploratory / Corrected / Focused / Diagnostics mode selector with **one coherent Cohort Analysis workflow**: cohort definition → sample mapping/QC → analysis settings → one **Run analysis** action → unified results/export.
- Adds a library-level `UnifiedCohortAnalysisRun` that freezes the cohort definition, mapping audit, corrected-model settings, reviewed cross-modal QC snapshot, corrected modality results, optional supporting descriptive checks, PBMC/confounding diagnostics, candidate prioritization, and warnings in one self-contained result.
- Keeps the existing **corrected / standard methods as the primary inferential engine**. The refactor orchestrates the existing corrected functions rather than introducing a second statistical implementation.
- Treats collaborator-supplied filtered metadata as an **optional cohort/subset definition**. It no longer implies a different analysis methodology.
- Moves cross-modal outlier QC into the unified sample-QC stage. Fresh automatic outlier calls are **flags only** and no longer preselect samples for exclusion; reviewed exclusions are explicitly applied in Analysis settings and frozen into the completed run.
- Integrates optional PBMC/source/composition diagnostics and candidate robustness labels into the same run. Focused-style candidate ranking is now a **presentation/prioritization layer derived from corrected results**.
- Adds two self-contained HTML report presets from the exact same completed run: **Simplified key-findings report** and **Full detailed report**. Choosing a report style never reruns statistics or changes sample inclusion.
- Adds a unified technical table export containing the frozen mapping/QC provenance, selected corrected modality results, pathways, diagnostics, key findings, warnings, and run summary.
- Keeps the legacy mode-specific implementation paths temporarily in source for parity/regression comparison; they are no longer the primary user-facing Cohort Analysis navigation and can be removed only after local v0.14.1 parity validation.

## What v0.14.1 adds

- Makes **QC filtering fully auditable in exports and HTML reports**. When reviewed outlier exclusions are applied to Corrected / standard or Focused candidate analyses, the app snapshots the exact outlier audit and reviewed exclusion list used for that run.
- Corrected and focused exports now include `sample_outlier_qc.csv`, `sample_outlier_qc_modalities.tsv`, and `sample_outlier_qc_summary.txt` with the detection scope, robust-distance threshold, cross-modal rule, consensus flags, manual overrides, per-modality coordinates/distances, and the exact sample IDs removed.
- Corrected and focused HTML reports now include a dedicated **Sample QC and reviewed exclusions** section with before/after case/control counts, excluded IDs, consensus-vs-manual decisions, per-modality QC summaries, and interactive outlier scatter plots.
- Exploratory exports/reports also include the current outlier audit when one has been run, but explicitly state that those reviewed exclusions were **not applied** to the exploratory result.
- Run-specific QC snapshots are preserved even if the user later changes the outlier settings, preventing a report from accidentally describing a different exclusion configuration than the one used to generate the statistics.

## What v0.14 adds

- Adds a **cross-modal sample outlier QC** section to the Exploratory workflow. Outlier detection is unsupervised and is based on sample-level representations for the currently enabled modalities rather than on the significance of individual biomarkers.
- Gene/transcript QC uses PCA of top-variable expression features; BCR/TCR use robust-standardized repertoire summary metrics; BCR isotype uses CLR composition; immune infiltrate uses robust-standardized ssGSEA signatures.
- Adds a configurable **within-group vs across-all** detection scope. Within case/control groups is the recommended default so a genuine disease-wide shift is less likely to be mistaken for a technical outlier.
- Adds configurable cross-modal rules: **any modality**, **at least N modalities**, **majority**, or **all available selected modalities**, with a minimum-modality requirement for consensus rules.
- Adds an editable sample audit table and per-modality PCA view. Automatic flags can be accepted, overridden, or cleared; the app does not treat an automatic flag as proof that a sample is biologically invalid.
- Adds optional application of the reviewed exclusion list to **Corrected / standard** and/or **Focused candidate** analyses. Exploratory results remain inclusive so filtered analyses can be treated as sensitivity checks rather than silently replacing the original cohort.
- Adds CSV export of the sample-level consensus flags and per-modality robust outlier scores.
- Adds unit tests for the cross-modal consensus rules and robust-distance detection.

## What v0.13 adds

- Adds an optional **sample-metadata filter directly to Merge & Export**. Load XLSX/CSV/TSV/TXT metadata without changing the selected workbook set.
- Lets the user choose the **metadata sample-ID column** used to match expression-report filenames, using the same punctuation-insensitive sample-ID normalization used by Cohort Analysis.
- The metadata table can act directly as an **inclusion list**: enable metadata filtering and every uniquely matched workbook represented by a metadata row is eligible. This is useful for collaborator-supplied pre-filtered metadata tables.
- Adds an optional **value-filter column** (for example `Phenotype`, `Healthy`, `HYP`, cohort, batch, or any other metadata field) plus checkboxes when an additional subset is desired.
- Filtering is **off by default**. When enabled, only uniquely matched workbooks represented in the metadata are merged; if an optional value-filter column is selected, the workbook must also have a checked value. Unmatched and ambiguous files are reported and excluded rather than aborting the merge.
- Expands the sample preview with **metadata match status, filter value, and include/exclude status**, and updates the output-column preview and merge button to reflect the filtered sample set.
- Metadata-only rows never manufacture missing expression files: the filter only selects among workbooks that actually exist.

## What v0.12 adds

- Adds a dedicated **PBMC / confounding diagnostics** analysis layer for investigating sample-source, composition, historical batch/family, low-count, and single-sample-driven expression signals before promoting them as disease-associated candidates.
- Adds **whole-transcriptome QC PCA** from top-variable `log2(TPM+1)` genes across the merged gene matrix, with robust sample-outlier scores and metadata-aware coloring.
- Adds per-sample **library/detection summaries** (total raw counts, detected genes, genes with counts ≥5/≥10) and integrates immune-infiltrate signatures when available.
- Adds an immune-signature PCA plus a descriptive **myeloid-high / lymphoid-low signature imbalance** score. These remain enrichment-score diagnostics rather than cell-percentage estimates.
- Adds user-editable lists for **known non-PBMC/wrong-source samples** and **suspect/unusual samples**. Suspect samples can be compared directly against ordinary samples without using disease status, making it easier to identify a source/processing expression program.
- Adds a **QC-restricted corrected gene sensitivity model** after sensitivity-only exclusions, plus an **immune-PC-adjusted corrected model** using the first two immune-signature PCs as covariates when enough immune profiles are available.
- Adds candidate-gene robustness diagnostics: raw-count and TPM medians, detection rates, count ≥5/≥10 fractions, top-two-sample contribution, leave-one-sample-out effect range, optional leave-batch/family/year-out effect range, a simple regularized-effect diagnostic, and comparison with the QC-restricted, immune-adjusted, and suspect-vs-ordinary effects.
- Adds a per-candidate **sample-level TPM plot** that highlights suspect or sensitivity-excluded samples, with raw-count/TPM/subject/batch/family information on hover.
- Adds **within-subject longitudinal QC** using maximum pairwise whole-transcriptome PCA distance across repeated samples.
- Adds one-click export of the sample QC audit, candidate robustness table, per-sample candidate values, longitudinal QC, suspect-vs-ordinary expression screen, and corrected sensitivity-model tables.

## What v0.11 adds

- Adds a dedicated **Docs** workspace to the desktop app, plus a **Docs** analysis layer inside Cohort Analysis. These pages explain the exploratory, corrected / standard, and focused workflows in plain language.
- Adds a lay-friendly **analysis guide** covering what each workflow is for, how to read the main plots, what conclusions are reasonable, and what important limitations to keep in mind.
- Adds simple **workflow schematics** in the app so non-technical collaborators can understand how the pieces fit together: merge & map → exploratory → corrected / standard → focused candidates.
- Adds a new repository document at **`docs/analysis_guide.md`** so the same explanation can be shared outside the app or quoted in methods/supporting notes.
- Adds **key literature links** for false-discovery-rate control, count-based expression modelling, compositional analysis, PERMANOVA, pathway analysis, Reactome, Hallmark/MSigDB, and ssGSEA.

## What v0.10 adds

- Adds a **built-in public GMT collection browser** to both the Corrected / standard and Focused candidate workflows. The catalog includes Reactome, Hallmark, GO Biological Process, ImmuneSigDB, HPO, Cell Type Signatures (C8), and WikiPathways from the human MSigDB v2026.1.Hs release.
- Adds **Download & use** for public collections. Downloads are cached under `<project>/gene_sets/msigdb_v2026.1.Hs/` and reused on later runs; selecting an already cached collection activates it immediately.
- Keeps **Choose custom GMT…** as a first-class option for local/private/custom pathway libraries.
- Shows collection-specific guidance in the GUI (for example, Reactome as the recommended mechanistic collection, Hallmark as the compact overview, and HPO as phenotype relevance rather than a literal pathway ontology), together with source/version/license links for provenance.
- Uses the operating system HTTPS client (`curl`, then `wget`, with PowerShell fallback on Windows) so the feature adds **no new Cargo dependency**. Downloaded files are validated as GMT before they are selected.
- Tightens the focused pathway shortlist so gene sets with **zero foreground overlap or p=1** cannot be promoted as candidate pathways.

## What v0.8 adds

- Adds **average-linkage hierarchical clustering / dendrograms** throughout the corrected layer: gene/transcript expression on top-variable normalized features, BCR/TCR on Morisita-Horn distance, V/D/J/V-J and isotype compositions on Aitchison distance, and immune infiltrate on standardized cell-type profiles.
- Adds a **PCA / PCoA / dendrogram color selector** in the app. Samples can be colored by analysis group, subject/patient, or any loaded metadata column to inspect phenotype, batch, sex, timepoint, or other structure.
- Fixes **pathway point hover in the desktop app** and keeps pathway hover concise in HTML reports. Full overlap-gene lists remain available in the result tables and exports.
- Improves **scatter responsiveness** by drawing ordinations/pathways as grouped point series and resolving hover labels from the nearest point. Large exploratory gene/transcript scatter plots use a bounded representative display while preserving every result in tables/exports.
- Expands **table viewports** and enables horizontal as well as vertical scrolling for large cohort/QC tables so wide or long results do not collapse to only a few visible rows.
- Adds modality-specific **analysis guides and interpretation notes** to both the corrected desktop results and corrected HTML reports. These explain what is being tested, what the unsupervised plots mean, and important cautions such as compositionality, sequencing depth, ssGSEA scale, and pathway redundancy.
- Removes implementation-oriented wording from the user-facing analysis descriptions so reports focus on methods, assumptions, interpretation, and limitations.
- Corrected result exports now include **dendrogram leaf-order TSVs** for reproducible sample-clustering review.

## What v0.6 adds

- Adds **optional transcript-expression analysis** to Cohort Analysis. The merged transcript table can be screened with TPM or Total Reads using the same exploratory Mann–Whitney/BH/Cliff's-delta framework as gene expression.
- Adds first-pass **TCR alpha and TCR beta comparison modules**. Each chain is analyzed independently for clone depth, richness, Shannon/Simpson diversity, clonality, effective clonotypes, top-clone fraction, weighted CDR3 amino-acid length, V-gene usage, J-gene usage, and V–J-pair usage.
- Adds an optional **immune-infiltrate module** for the per-sample ssGSEA workbooks, including per-cell-type group tests, effect plots, group-mean visualization, and exported sample values.
- Makes all newly enabled modalities **fault tolerant**: absent files, samples outside the comparison, duplicates, and malformed individual workbooks are reported without aborting the other selected analyses.
- Extends **Save result tables…** and the standalone HTML report to transcript, TCR alpha, TCR beta, and immune-infiltrate outputs.
- Reworks all report scatter plots to use explicit Plotly hover text. Hovering a point now reports the biological feature name (gene/transcript/V gene/J gene/V–J pair/BCR isotype/immune-cell type), secondary annotation where available, effect, Cliff's delta, p value, BH q value, group medians/means, sample counts, and detection fractions when applicable.
- Adds a dedicated BCR-isotype effect/significance scatter to the HTML report.

The TCR module in v0.6 is intentionally the first comparison layer. The existing Python workflow's matched-depth rarefaction, Morisita–Horn similarity/PCoA, public/private clonotypes, amino-acid convergence, and related repertoire-wide analyses remain candidates for the next advanced TCR iteration.

## What v0.5 adds

- Adds a **Generate HTML report…** action to Cohort Analysis using the `report-builder` Rust crate. The report combines the comparison definition, mapping audit, interactive plots, ranked tables, analysis caveats, and modality warnings into one HTML file.
- Adds a **gene detection-shift plot** comparing the fraction of case versus control samples in which each gene is detected.
- Adds selectable **case/control BCR distribution box plots** for Shannon diversity, clonality, unique clones, top-clone fraction, effective clonotypes, and weighted mean CDR3 AA length, including individual sample points.
- Adds a **group-mean BCR-isotype composition plot** alongside the existing effect/test table.
- Adds an automated report-generation unit test in addition to the existing cohort tests.

## What v0.4.1 adds

- Renames the desktop application to **4C Cohort Explorer**.
- Adds a one-click **Export mapping CSV…** action in the sample mapping & coverage audit so unmatched/ambiguous/metadata-only samples and cross-modality coverage can be sent back for metadata correction.
- Makes large cohort result tables substantially taller and reserves a fixed scrollable viewport so V/D/J-gene and differential-feature tables no longer collapse to only a few visible rows inside the outer page.
- Increases the on-screen sample mapping audit height for easier review.

## What v0.4 adds

v0.4 introduces a new **Cohort Analysis** workspace while preserving the existing Merge & Export and Explore & QC workflows.

- **Recursive project scan** detects gene expression, transcript expression, BCR heavy-chain, BCR isotype, TCR alpha, TCR beta, immune-infiltrate, and merged expression files.
- **Generic sample metadata loading** from XLSX/CSV/TSV/TXT. `4C_ID` is selected automatically when present and the ID column can be changed in the GUI.
- **Cross-modality mapping audit** reports matched, unmatched, ambiguous and duplicate sample/file combinations without aborting the analysis.
- **Built-in exploratory comparison presets** implement the current study questions: all HYP versus `True_HC`, and `HYP+ME/CFS` versus ME/CFS-only.
- **Gene-expression biomarker screen** uses the merged TPM matrix, Mann–Whitney U tests on `log2(TPM+1)`, Benjamini–Hochberg FDR, Cliff's delta, detection fractions and a simple log2 mean fold-change effect. Results are shown as an interactive volcano-style plot and ranked table.
- **BCR first-pass comparison** calculates total clone count, unique clones, Shannon/Simpson diversity, normalized clonality, effective clonotype count, top-clone fraction, weighted mean CDR3 amino-acid length, and heavy-chain isotype/V/D/J usage. Group tests use Mann–Whitney U, BH correction and Cliff's delta.
- **Dedicated BCR-isotype comparison** tests the `Isotype Read Fraction` values from the composition workbooks.
- **Fault-tolerant modality execution** means a missing/unsupported modality or a malformed individual BCR workbook is reported while other available analyses continue.
- TCR and immune-infiltrate files are detected and included in the sample coverage audit; v0.6 adds their first-pass group-comparison modules.

The v0.4 cohort tests are deliberately an **exploratory first pass**. They do not yet adjust for repeated sampling, family structure, age, sex, ethnicity or other covariates.

## What v0.3 adds

The v0.3 desktop interface keeps the **Merge & Export** and **Explore & QC** workspaces, but makes them much more practical for repeated use:

- **Remembered file-dialog directory** — the last directory is persisted across application launches and reused by the native file/folder dialogs.
- **Drag and drop** — drop multiple `.xlsx` files, an entire folder, or a previously merged `.tsv` onto the application. A pasted path can also be added from the input card.
- **Merged TSV as the preferred QC source** — after a successful merge, Explore & QC automatically points at the generated TSV instead of reopening every Excel workbook. A prior merged TSV can also be opened directly, without loading the original workbooks at all.
- **Bounded-memory merged-file QC** — sample distribution summaries read the merged TSV in bounded sample chunks rather than building the full cohort matrix in memory.
- **Scrollable identifier browser** — all genes/transcripts are visible in a virtualized table even before a search is entered. Search filters that table instead of hiding the catalogue entirely.
- **Faster, clearer gene/transcript profiles** — a row-level **Plot** action and a prominent **Plot profile** button make selection explicit. With a merged TSV, a profile needs only one TSV scan instead of reopening every workbook.
- **Two profile views** — preserve sample order or rank samples from low to high expression, with hover annotations and a collapsible sample/value table.
- **Three sample-QC views** — distribution box plots, non-zero detection rate across samples, and median-abundance versus detection-rate scatter.
- **Improved box-plot readability** — translucent boxes plus high-contrast median and mean markers make sample centers easier to compare.

The output side still provides explicit value/suffix checkboxes, **Default / All / Clear** shortcuts, sample-prefix versus value-suffix column ordering, searchable sample preview, hover help, and an output-column preview.

## Supported report schemas

### Gene expression

Expected columns include:

- `Gene Symbol`
- `NCBI Gene ID`
- `RNA-Seq Raw Counts`
- `FPKM`
- `CPM`
- `TPM`
- `Percentile Rank`
- `Is Expressed`

The **Default** measurement selection is:

- Raw Count
- TPM

### Transcript expression

Expected columns include:

- `Gene Symbol`
- `NCBI Gene ID`
- `Transcript Name`
- `Transcript Length`
- `Effective Transcript Length`
- `Total Reads`
- `TPM`
- `Percentile Rank`
- `Is Expressed`

The **Default** measurement selection is:

- Total Reads
- TPM

The application auto-detects gene vs transcript reports from the workbook headers.

## Sample names

Standard report suffixes are removed automatically. For example:

```text
4C_101_tumor_rna_gene_expression_report.xlsx
```

becomes:

```text
4C_101
```

The sample preview lets you inspect these names before merging.

## Memory strategy

The first workbook establishes the canonical row index. Subsequent workbooks are processed in bounded groups. Only one workbook plus a bounded set of selected measurement arrays is retained in memory. Each group is transposed to a temporary row-major binary chunk, and the final TSV is streamed row-by-row from those chunks.

This avoids the previous R pattern of retaining all workbooks/data frames, a large long-format object, and a wide pivoted object in memory simultaneously.

Explore & QC now prefers a merged TSV whenever one is available. Selected-identifier profiles are extracted with a single TSV scan. Sample-distribution QC uses bounded sample chunks sized around the same working-memory target as the merger, so a large merged transcript table does not need to be loaded as one giant matrix. If no merged TSV exists, the application still falls back to processing the Excel workbooks one at a time.

## Row policy

The first workbook currently defines the canonical set and order of genes/transcripts.

- Missing rows in later workbooks are emitted as blank values and counted in the merge summary.
- Rows that occur only in later workbooks are skipped and counted as unknown rows.

This is a deliberate safety policy for the standardized reports. Future versions can add explicit `strict`, `union`, and `intersection` policies.

## Build

Rust 1.95 or newer is required by the current `eframe`/`egui_plot` stack.

```bash
cargo build --release
```

Binaries:

```text
target/release/cfs-expression-merge
target/release/cfs-expression-merge-gui
```

On Windows the binaries use the `.exe` extension.

Run the desktop application:

```bash
./target/release/cfs-expression-merge-gui
```

or during development:

```bash
cargo run --release --bin cfs-expression-merge-gui
```

## Desktop workflow

### Merge & Export

1. Choose individual XLSX files or a folder.
2. Check the sample-prefix preview.
3. Leave report type on **Auto** unless an override is needed.
4. Check the values/suffixes that should be written.
5. Use **Default** for Raw Count + TPM (gene) or Total Reads + TPM (transcript).
6. Choose **By sample prefix** or **By value suffix** column ordering.
7. Review the output-column preview.
8. Choose the output TSV.
9. Click **Merge datasets**.

### Explore & QC

1. Use the merged TSV created in the current session, click **Open merged TSV…** to load an older merged result, or explicitly fall back to the selected Excel workbooks. Direct TSV loading currently expects the collaborator-style headers generated by this tool (`Gene Symbol` or `Transcript Name`, `NCBI Gene ID`, then columns such as `4C_101 TPM`).
2. Choose a numeric measurement available from that source.
3. Optionally ignore zeros for distribution statistics and/or use the log10 display transform.
4. Click **Build / refresh QC**, then switch among **Distributions**, **Detection rate**, and **Median vs detection**.
5. Browse the full scrollable gene/transcript table or filter it with the search box.
6. Click a row, then **Plot profile**, or use the row's **Plot** button directly.
7. Switch the profile between **Sample order** and **Ranked** views and hover points for sample/value annotations.

A selected gene/transcript has one value per sample, so the profile view uses individual sample points rather than a misleading per-sample box plot. The sample-level distribution panel instead summarizes all genes/transcripts within each sample.

### Cohort Analysis

1. Choose the top-level cohort project folder. The app recursively detects supported modalities and existing merged expression tables.
2. Load the primary sample metadata table. Optionally load a collaborator-supplied **filtered metadata subset**; this changes which samples define the run, not the statistical methodology.
3. Review **Sample mapping & QC**. The mapping audit keeps unmatched, ambiguous, metadata-only, duplicate, and modality-coverage information visible without aborting other usable modalities.
4. Optionally run **cross-modal sample QC**. Automatic consensus outliers are review flags only. Explicitly select any samples that should be excluded after review; no sample is automatically selected for deletion.
5. In **Analysis settings**, choose the comparison, modalities, subject/repeated-measure column, covariates, repertoire settings, GMT/pathway options, whether reviewed QC exclusions should be applied, and optional PBMC/confounding sensitivity diagnostics.
6. Click the single **Run analysis** button. The corrected/standard methods are the primary inferential engine; modality failures remain fault tolerant. Optional exploratory TPM calculations are used only as descriptive direction-concordance support.
7. Inspect **Unified results** by modality plus the QC / robustness view. Candidate genes/pathways/other signals are a shortlist derived from the same corrected results, not a separate analysis.
8. Export either **Simplified key-findings report**, **Full detailed report**, or the complete technical table set. All three exports use the exact same frozen completed run and therefore cannot silently change sample inclusion or rerun a different statistical engine.

The application supports discovery, robustness assessment, and candidate prioritization. Preliminary candidates should not be described as validated clinical biomarkers without independent replication/validation.

### HTML reports

A completed unified run supports two report detail levels:

- **Simplified key-findings report** — presentation-friendly comparison/QC summary, concise main takeaway, strongest preliminary genes/pathways/other modality signals, a small number of main plots, and short limitations. Candidate wording explicitly avoids validated-biomarker claims.
- **Full detailed report** — cohort definition, mapping audit, exact reviewed QC exclusions and reasons, per-modality outlier evidence, all selected corrected modality results, ordinations/dendrograms, pathway provenance, PBMC/confounding sensitivity analyses, candidate robustness, warnings, methods, assumptions, and limitations.

Both reports are rendered from the same `UnifiedCohortAnalysisRun`; report generation does **not** rerun statistics. The full technical table export is generated from that same frozen object as well.

`report-builder` 0.1.1 normally references Plotly and optional table helpers from public CDNs. v0.5+ compiles Plotly with its `plotly_embed_js` feature, injects that bundled JavaScript into the report, and removes the unused DataTables/FileSaver CDN helpers during finalization. The resulting `.html` is therefore one self-contained file whose interactive plots can be opened offline. This post-processing is pinned to the current report-builder 0.1.1 template and is covered by a report-generation unit test.

## CLI examples

Gene expression, default Raw Count + TPM:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_merged.tsv \
  --kind gene \
  --preset collaborator
```

Transcript expression, default Total Reads + TPM:

```bash
cfs-expression-merge \
  --input ./Transcript_expression \
  --output ./transcript_expression_merged.tsv \
  --kind transcript \
  --preset collaborator
```

Export all available measurements:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_all.tsv \
  --kind auto \
  --preset all
```

Custom measurements:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_tpm_fpkm_cpm.tsv \
  --kind gene \
  --preset custom \
  --metrics tpm,fpkm,cpm
```

## Cross-platform packaging

The same source can be compiled into native releases for each operating system:

- Windows `.exe` / zip
- macOS binary or `.app` bundle
- Linux binary/AppImage

A single executable cannot run unchanged on all three operating systems because their binary formats and ABIs differ. The included GitHub Actions workflow is the starting point for creating per-OS release artifacts.

## Good next additions

For cohort analysis in particular, useful next modules are:

- Repertoire similarity heatmaps / dendrograms and richer public/longitudinal clonotype browsers on top of the corrected Morisita-Horn/Jaccard and matched-depth analyses.
- Immune-infiltrate cell-type heatmaps and cross-modal correlation with expression/repertoire features.
- Ranked pathway enrichment in addition to the corrected GMT-based ORA now available.
- A more specialized DTU model (e.g. Dirichlet-multinomial/DEXSeq-like); the current corrected layer provides a CLR/Simes DTU screen with its assumptions stated explicitly.
- Gene-expression PCA/correlation heatmaps and selected top-gene heatmaps.
- Cross-modal candidate-biomarker summaries that combine gene, immune-infiltrate, BCR, BCR-isotype, and later TCR evidence.
- Further validation/regression fixtures for the unified runner across representative longitudinal/covariate configurations.

General merge/QC additions:

- Preflight schema validation for every workbook, not only the first report.
- Duplicate sample-prefix detection and editable sample names.
- `strict`, `union`, and `intersection` row policies.
- Save/load named export and QC presets.
- Metadata/group import so plots can compare biological groups, diagnoses, treatments, or batches.
- Multi-gene selection with heatmaps and grouped box/violin plots.
- Sample-sample correlation heatmap and PCA using the merged TSV with a bounded/sampled representation.
- Highly-variable-gene ranking and top-variable-gene browser.
- Optional XLSX export for modest-sized matrices.
- Long-format export matching the legacy R workflow.
- A detailed QA report listing schema mismatches, missing/extra rows, and duplicate identifiers per sample.

## Unified Cohort Analysis architecture (v0.15.0)

The corrected / standard layer remains the statistical backbone, but the desktop UX now presents one workflow rather than asking users to choose between competing analysis modes. A completed `UnifiedCohortAnalysisRun` owns the cohort definition, mapping audit, frozen QC provenance, settings, corrected results, optional supporting diagnostics, candidate shortlist, and warnings.

The primary corrected methods are preserved:

- Gene raw counts and transcript Total Reads: positive-count median-ratio normalization, negative-binomial log-link count models with log(size-factor) offsets and per-feature dispersion, metadata covariates, subject-clustered sandwich SEs for repeated sampling, BH FDR, subject-collapsed log2-normalized sensitivity tests, PCA, and hierarchical clustering. Transcript data additionally retains the within-gene CLR/Simes/BH DTU screen.
- BCR/TCR: exact without-replacement matched-depth rarefaction, adjusted repertoire metrics, subject-aware private/longitudinal/public clonotype fractions, Morisita-Horn and Jaccard PCoA, subject-blocked permutation PERMANOVA, CLR/Aitchison composition analysis, and dendrograms.
- TCR: AA-convergence read fraction when nucleotide clonal sequences are available.
- BCR/TCR V/D/J/V-J ambiguity handling: exclude tied calls (default), split weight, legacy first-hit, or gene-family collapse.
- Dedicated BCR isotypes: CLR/Aitchison analysis, optionally excluding `Undetermined` and renormalizing as a sensitivity analysis.
- Immune infiltrate: covariate-adjusted ssGSEA models plus PCA and clustering of standardized signatures.
- Optional GMT pathway ORA from corrected gene-count results using the tested-gene universe, odds ratios, explicit overlap genes, and BH FDR. Public MSigDB collections and custom GMT files remain supported.

Cross-modal outlier QC and PBMC/confounding diagnostics are now QC/sensitivity evidence attached to the same run. Automatic cross-modal outliers are flags requiring review; when reviewed exclusions are applied, the exact audit, manual overrides, excluded IDs, and before/after group counts are frozen for exports/reports.

Candidate prioritization is likewise derived from the corrected result. A collaborator-filtered metadata table simply defines a narrower cohort, and the same corrected methodology is used. Optional TPM direction concordance and PBMC/confounding robustness classifications annotate the shortlist without becoming independent statistical evidence.

The expression count model does **not** claim to reproduce the full DESeq2, edgeR, or limma-voom empirical-Bayes/dispersion machinery. Final publication-level findings can still be cross-checked with specialized packages; 4C Cohort Explorer is intended for transparent cohort discovery, robustness assessment, and follow-up prioritization.
