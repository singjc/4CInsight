# 4C Insight — Integrative Analysis science and implementation contract

**Date:** 2026-11-07 · **Phase:** I0 implemented as read-only evidence integration; I1–I3 not implemented.

## Goal and definitions

The user should be able to include any selection of gene expression, transcript expression,
BCR, TCR alpha, TCR beta, BCR isotypes, and immune infiltration in a coherent analysis.

**I0 (current overlay):** Display and export corrected evidence from every available modality.
Each adjusted test retains its original BH q-value, effect value, feature family, and modality.
This is **not** a joint model, a joint FDR procedure, a combined PCA, or cross-modality statistical inference.
Do not compare effect magnitudes across different scales, or claim added predictive performance.
The existing gene/transcript selected-hit PCA and clustering remain separate and are not a seven-modality model.

## I1 — Sample alignment and quality gates (required before joint analysis)

- Choose observation/sample ID and patient ID from metadata; preserve timepoint and visit.
- Resolve exact aliases with the existing metadata mapping rules; reject ambiguous collisions.
- Show per-modality availability and missingness for **each biological observation**.
- Make explicit whether the study uses complete-case or partial multi-view data.
- Keep repeat measures grouped by patient. Do not treat them as independent biological patients.
- Make per-modality QC and exclusion provenance visible and exportable.
- Process source workbooks incrementally; avoid holding all XLSX source bytes in memory.

## I2 — Unsupervised joint sample structure

- Gene/transcript: use normalization and transformations from the validated expression engine.
- BCR/TCR: use abundance-weighted clonotype distances, not raw sequences as arbitrary numeric features.
  Respect V/J ambiguity options, clonotype identity and rarefaction where appropriate.
- Isotypes: use compositional CLR/Aitchison geometry with declared zero handling.
- Immune infiltration: use standardized ssGSEA-signature values; **not** estimates of cell percentages.
- Normalize *modality contributions*, not just individual feature ranges. Avoid gene/transcript
  domination because they contain more features than the receptor or immune views.
- Compare validated multi-view strategies such as distance-fusion (Similarity Network Fusion)
  and a latent-factor model (MOFA-like). These are **candidate research approaches**,
  not scientific features that I0 implements.
- If data are missing for some modalities, record the eligible sample set and validate a
  partial-data strategy; do not silently substitute zeros for absent data.
- Report fused embedding, clustering, stability, agreement between modalities,
  and leave-one-modality-out sensitivity analyses.
- Provide matched **native/WASM reference tests**, deterministic fixtures, and real-cohort parity.

## I3 — Supervised integration (optional, separate from exploratory analysis)

- Require a defined outcome and use patient/group-aware training/test splits.
- Select features, normalize, tune weights and train **inside each cross-validation fold**.
- Keep the held-out test set untouched until final evaluation.
- Report performance per modality and jointly, calibration, uncertainty,
  missing-modality sensitivity, and confounder review.
- Multiblock PLS-DA/DIABLO is a candidate, but no supervised integrated model is implemented today.

## I0 release acceptance

1. All seven data types appear in Integrative Analysis, with correct missing/available state.
2. Selected data types control the evidence table and complete cross-modal CSV export.
3. Each selected feature preserves the native model's q-value and original effect scale.
4. The five receptor/immune tables are full width, searchable/sortable and sticky headed.
5. No false joint PCA/clustering, joint BH-FDR or out-of-sample accuracy claims.
6. Existing gene/transcript selected-hit analysis and History identifiers stay compatible.
7. Full JavaScript suite, narrow/desktop Firefox checks, GitHub CI, and real-cohort checks pass.

## References

- Wang et al. (2014), *Similarity network fusion for aggregating data types on a genomic scale*,
  Nature Methods 11:333–337. https://doi.org/10.1038/nmeth.2810
- MOFA2 Bioconductor vignette (2026), https://bioconductor.org/packages/release/bioc/vignettes/MOFA2/inst/doc/getting_started_R.html
- mixOmics N-Integration methods, https://mixomics.org/methods/n-integration-methods/
