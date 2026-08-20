# CFS Expression Merger

A native Rust application for merging many per-sample RNA expression Excel reports into collaborator-friendly wide tables without retaining the entire cohort in RAM.

The project contains both a command-line interface and an `egui` desktop interface. The desktop application is intended to let a collaborator select the reports, validate the sample names, choose exactly which measurements to export, merge the matrix, and perform lightweight QC/exploration without using R.

## What v0.2 adds

The v0.2 desktop interface is reorganized into two workspaces:

- **Merge & Export** — input selection, sample preview, value/suffix checkboxes, column ordering, output preview, and bounded-memory export.
- **Explore & QC** — per-sample distribution plots and searchable gene/transcript expression profiles.

New GUI features include:

- Explicit checkboxes for every available output value/suffix.
- **Default**, **All**, and **Clear** selection shortcuts.
- Hover help on interactive controls.
- Column ordering by either:
  - **sample prefix**: `4C_101 Raw Count`, `4C_101 TPM`, `4C_104 Raw Count`, `4C_104 TPM`, ...
  - **value suffix**: `4C_101 Raw Count`, `4C_104 Raw Count`, ..., `4C_101 TPM`, `4C_104 TPM`, ...
- Searchable sample preview.
- Output-column preview before writing the matrix.
- Per-sample box plots for a selected numeric measurement such as Raw Count, Total Reads, TPM, FPKM, CPM, or Percentile Rank.
- Optional exclusion of zeros from box-plot statistics.
- Optional `log10(value + 1)` display transformation.
- QC summary table with median, mean, non-zero fraction, missing count, and number of plotted values.
- Search by gene symbol, NCBI Gene ID, or transcript accession/name.
- Interactive selected-gene/transcript profile across samples, with hover labels containing the identifier, sample prefix, measurement, and value.
- Plotting and QC run on worker threads so the GUI remains responsive.

`egui_plot` is used for the interactive plots.

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

The QC and selected-identifier plotting functions also process workbooks one at a time. Building a plot over hundreds of XLSX files can therefore take some time, but its memory use does not grow as a full cohort-wide expression matrix.

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

1. Choose a numeric measurement.
2. Optionally ignore zeros for distribution statistics and/or use the log10 display transform.
3. Click **Build sample QC** to compute one box plot per sample.
4. Search for a gene/transcript and select an identifier.
5. Click **Plot selected** to scan the selected measurement across all sample workbooks.
6. Hover plotted points/boxes for annotations.

A selected gene/transcript has one value per sample, so the profile view uses individual sample points plus a connecting trend rather than pretending each sample has enough observations for its own gene-specific box plot. The per-sample box-plot panel instead summarizes the distribution of all genes/transcripts in that sample.

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
- Save/load user presets.
- Metadata/group import so plots can compare biological groups rather than only individual sample prefixes.
- Multi-gene selection with heatmaps or grouped box/violin plots.
- Correlation matrix / sample-sample QC using a bounded or sampled representation.
- Optional XLSX export for modest-sized matrices.
- Long-format export matching the legacy R workflow.
- A detailed QA report listing schema mismatches, missing/extra rows, and duplicate identifiers per sample.
