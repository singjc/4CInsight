# 4C Cohort Explorer

A native Rust application for memory-bounded RNA expression merging plus metadata-driven multi-modal cohort exploration across gene/transcript expression, BCR, BCR isotypes, TCR alpha/beta, and immune-infiltrate data.

The project contains both a command-line interface and an `egui` desktop interface. The desktop application is intended to let a collaborator select the reports, validate the sample names, choose exactly which measurements to export, merge the matrix, and perform lightweight QC/exploration without using R.

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
2. Load the sample metadata workbook/table; `4C_ID` is preferred automatically when available.
3. Review the mapping/coverage audit. Unmatched, ambiguous, metadata-only, and duplicate entries remain visible and do not abort the project.
4. Choose one of the current study presets: **HYP vs True_HC** or **HYP+ME/CFS vs ME/CFS-only**.
5. Select any combination of Gene expression, Transcript expression, BCR, BCR isotype, TCR alpha, TCR beta, and Immune infiltrate. Gene expression defaults to TPM; transcript expression can use TPM or Total Reads.
6. Run the exploratory comparison. Each modality runs independently so a missing or malformed input does not prevent the others from completing.
7. Inspect the interactive result panels. Transcript, TCR alpha/beta, and immune-infiltrate results receive their own tabs; failed/unavailable modalities remain visible as warnings instead of aborting the comparison.
8. Use **Save result tables…** to export the mapping audit and statistical tables as TSV files, or **Generate HTML report…** to create a single shareable HTML summary with interactive Plotly figures and ranked tables.

The cohort analysis is intentionally exploratory. Longitudinal/repeated samples are currently treated as observations, matching the first-pass study request; covariate-aware and repeated-measures models belong in a later rigorous-analysis mode.


### HTML reports

The cohort results card can generate a single HTML report using the [`report-builder`](https://github.com/singjc/report-builder) Rust crate. The report includes:

- comparison definition and first-pass modelling caveats;
- sample mapping/coverage audit;
- gene- and transcript-expression volcano/detection-shift plots plus ranked feature tables;
- BCR group box plots for key repertoire metrics and ranked sample-metric/isotype/V/D/J tests;
- dedicated BCR-isotype composition and effect/significance plots;
- TCR alpha/beta repertoire-summary plots and V/J/V–J composition tests when requested;
- immune-cell ssGSEA group plots and per-cell-type tests when requested;
- rich feature-aware hover text on scatter plots rather than anonymous x/y coordinates;
- file warnings and unavailable-modality messages rather than silently dropping failed analyses.

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
- A more specialized DTU model (e.g. Dirichlet-multinomial/DEXSeq-like) if we decide to reproduce that machinery natively; the current corrected layer provides a transparent CLR/Simes DTU screen.
- Gene-expression PCA/correlation heatmaps and selected top-gene heatmaps.
- Cross-modal candidate-biomarker summaries that combine gene, immune-infiltrate, BCR, BCR-isotype, and later TCR evidence.
- A later rigorous-analysis mode for longitudinal/repeated measures and covariate adjustment.

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

## v0.7 corrected / standard analysis layer

Cohort Analysis now has two explicit layers:

- **Exploratory discovery** preserves the original all-sample first-pass screens requested for hypothesis generation.
- **Corrected / standard** adds subject-aware and covariate-adjusted analyses implemented directly in Rust with no new numerical/statistical runtime dependencies.

The corrected layer supports:

- Gene raw counts and transcript Total Reads: positive-count median-ratio normalization, a native NB2 GLM with log(size-factor) offset and per-feature unshrunk dispersion, metadata covariates, subject-clustered sandwich SEs for repeated sampling, BH FDR, subject-collapsed log2-normalized sensitivity tests, and PCA of variable normalized features. Transcript data additionally gets a within-gene CLR/Simes/BH differential-transcript-usage screen (explicitly labeled as a dependency-light alternative, not DEXSeq/DRIMSeq).
- BCR/TCR: exact without-replacement matched-depth rarefaction, adjusted repertoire metrics, subject-aware private/longitudinal/public clonotype fractions, Morisita-Horn and Jaccard PCoA, subject-blocked permutation PERMANOVA, and CLR/Aitchison composition analysis.
- TCR: AA-convergence read fraction when nucleotide clonal sequences are available.
- BCR/TCR V/D/J/V-J ambiguity handling: exclude tied calls (default), split weight, legacy first-hit, or gene-family collapse.
- Dedicated BCR isotypes: CLR/Aitchison analysis, optionally excluding `Undetermined` and renormalizing as a sensitivity analysis.
- Immune infiltrate: covariate-adjusted ssGSEA models plus PCA on per-cell-type z-scores.
- Optional pathway analysis from corrected gene-count results using a user-supplied/versioned GMT: hypergeometric over-representation testing against the tested-gene universe, odds ratios, explicit overlap genes, and BH FDR.
- Corrected TSV exports and a separate self-contained corrected HTML report.

The native expression model deliberately does **not** claim to reproduce DESeq2, edgeR, or limma-voom empirical-Bayes/dispersion machinery. It provides a transparent dependency-light NB model with unshrunk feature-wise dispersion while keeping the exploratory layer available for comparison.
