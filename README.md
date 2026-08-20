# CFS Expression Merger

A Rust replacement for the current R workflow that merges many per-sample RNA expression Excel reports into collaborator-friendly wide tables without retaining the entire cohort in RAM.

## What v0.1 does

- Reads `.xlsx` files produced as either:
  - gene expression reports (`Gene Symbol`, `NCBI Gene ID`, `RNA-Seq Raw Counts`, `FPKM`, `CPM`, `TPM`, `Percentile Rank`, `Is Expressed`)
  - transcript expression reports (`Gene Symbol`, `NCBI Gene ID`, `Transcript Name`, `Transcript Length`, `Effective Transcript Length`, `Total Reads`, `TPM`, `Percentile Rank`, `Is Expressed`)
- Auto-detects gene vs transcript reports from the headers.
- Strips the standard report suffix from filenames, so `4C_101_tumor_rna_gene_expression_report.xlsx` becomes sample `4C_101`.
- Default collaborator preset:
  - gene: raw count + TPM
  - transcript: `Transcript Name`, `NCBI Gene ID`, then total reads + TPM for each sample
- `All measurements` and custom metric selections are supported.
- Outputs TSV in either:
  - **adjacent-by-sample** order (default): `4C_101 Raw Count`, `4C_101 TPM`, `4C_104 Raw Count`, `4C_104 TPM`, ...
  - **grouped-by-measurement** order: all raw-count columns, then all TPM columns, ...
- Includes both CLI and native egui desktop front end.

## Memory strategy

The first workbook establishes the canonical row index. Subsequent workbooks are processed in bounded groups. Only one workbook plus a bounded set of selected measurement arrays is retained in memory. Each group is transposed to a temporary row-major binary chunk, and the final TSV is streamed row-by-row from those chunks.

This avoids the R pattern of keeping every workbook/data frame and the full long/wide merged table in memory at the same time.

## Important row policy in v0.1

The first workbook defines the canonical set and order of genes/transcripts.

- Missing rows in later workbooks are emitted as blank values and counted in the summary.
- Rows that occur only in later workbooks are skipped and counted as `unknown rows`.

For these standardized reports this is usually desirable. A future `--row-policy union` mode can do a two-pass union when datasets legitimately contain different row sets.

## Build

Install a current Rust toolchain, then:

```bash
cargo build --release
```

Binaries:

```text
target/release/cfs-expression-merge
target/release/cfs-expression-merge-gui
```

On Windows the files have `.exe` extensions.

## CLI examples

Gene expression, collaborator default (raw counts + TPM):

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_merged.tsv \
  --kind gene \
  --preset collaborator
```

Transcript expression, collaborator default (Total Reads + TPM):

```bash
cfs-expression-merge \
  --input ./Transcript_expression \
  --output ./transcript_expression_merged.tsv \
  --kind transcript \
  --preset collaborator
```

Auto-detect report type and export all available measurements:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_all.tsv \
  --kind auto \
  --preset all
```

Custom gene measurements:

```bash
cfs-expression-merge \
  --input ./Gene_expression \
  --output ./gene_expression_tpm_fpkm_cpm.tsv \
  --kind gene \
  --preset custom \
  --metrics tpm,fpkm,cpm
```

## Desktop GUI

Run:

```bash
cargo run --release --bin cfs-expression-merge-gui
```

The collaborator can:

1. choose a folder or select multiple `.xlsx` files;
2. use auto-detection or choose Gene/Transcript;
3. choose Collaborator default, All measurements, or Custom;
4. choose adjacent-by-sample or grouped-by-measurement columns;
5. choose an output TSV;
6. click **Merge datasets**.

The merge runs on a worker thread and reports progress in the UI.

## Cross-platform packaging

A single compiled executable cannot run unchanged on Linux, Windows, and macOS because the operating systems use different binary formats and ABIs. The same Rust source can, however, produce one native build per OS. For a collaborator-facing release, publish three artifacts:

- Windows `.exe` / zip
- macOS binary or `.app` bundle
- Linux binary/AppImage

For macOS/Windows distribution outside a development team, signing/notarization is recommended to avoid Gatekeeper/SmartScreen warnings.

## Suggested next additions

1. XLSX output using `rust_xlsxwriter` constant-memory mode.
2. `union`, `intersection`, and `strict` row-set policies.
3. Preview/validation screen showing detected columns and sample IDs before merge.
4. User-editable sample-name extraction rule.
5. Save/load presets.
6. Optional long-format export matching the legacy R `rbindlist(..., idcol="file")` output.
7. A QA report listing missing/extra rows, duplicate identifiers, and schema differences per sample.
8. GitHub Actions release builds for Linux/Windows/macOS.
