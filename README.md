# CFS Expression Merger

A native Rust application for merging many per-sample RNA expression Excel reports into collaborator-friendly wide tables without retaining the entire cohort in RAM.

The project contains both a command-line interface and an `egui` desktop interface. The desktop application is intended to let a collaborator select the reports, validate the sample names, choose exactly which measurements to export, merge the matrix, and perform lightweight QC/exploration without using R.

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
