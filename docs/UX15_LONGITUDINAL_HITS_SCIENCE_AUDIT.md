# Scientific workflow audit — Longitudinal and Significant Hits

Date: 2026-11-07. Basis: native code in `src/longitudinal.rs`, `src/hit_structure.rs`; portable browser code in `src/portable_longitudinal.rs`, `src/portable_hit_structure.rs`; browser bridge and UI. This document is a scope and verification plan, not a claim that all seven data types are supported in these two browser workflows.

## Current boundary

- **Compare Groups:** all seven data types are available with the current M2/M2-R6 browser bridge.
- **Longitudinal:** browser UI and portable scientific entry point cover gene and transcript expression only. Native Rust (`run_longitudinal_analysis`) already dispatches gene, transcript, BCR, BCR isotypes, TCR alpha, TCR beta, and immune infiltration. Native coverage is not browser parity.
- **Significant Hits:** browser UI/portable path covers gene/transcript selected-hit structure only. Native Rust (`run_significant_hit_structure`) also handles receptor composition, BCR isotype CLR, and immune signatures if corresponding adjusted features pass selection. Native coverage is not browser parity.

## Recommended phase L1 — longitudinal integration

1. Expose all five new modality toggles. Preserve user-selected observation ID, patient ID, optional visit/time column, and exclusions. Do not interpret repeated measures as independent patients.
2. Share the M2 byte parser and isolate each modality. Avoid loading the entire workbook set into JavaScript arrays. Keep the source files local and retain per-modality errors.
3. Use the same unsupervised distance method per modality as native Rust. Do not concatenate raw, non-comparable measurement scales across modalities.
4. Summarize patient-identity AUC, within vs. between patient distance, nearest-neighbor identity, and longitudinal stability with per-modality counts and missingness.
5. Preserve independent modality comparisons and warnings when patients lack an assay. Confirm minimum repeated observations per patient and distinguish patient-level from sample-level counts.
6. Require native-file vs browser-byte fixture parity and real-cohort flight tests before release.

## Recommended phase H1 — selected-hit structure integration

1. Reuse the **frozen corrected comparison** from Compare Groups, including group labels, subject mapping, exclusions, covariates, and threshold choices. Do not silently refit different models.
2. Use only science-supported selected features: receptor V/J/composition adjusted feature tests (not raw individual clonotypes), isotype CLR tests, and adjusted immune-signature scores. Repertoire diversity descriptive metrics are not automatically adjusted feature tests.
3. Plot selected-hit PCA, hierarchical clustering, clustered heatmaps, and subject-blocked PERMANOVA by modality; identify the precise distance and transformation in each caption.
4. Label the plots as post-selection descriptive visualizations, not independent validation, classifier accuracy, or prediction.
5. If no adjusted tests or no significant features exist, show an explicit state (not a misleading value of zero) and retain other available results.
6. Require feature-ID equivalence, p/q/effect parity, sample-order checks, controlled permutation seeds, and native/browser numeric tolerance tests.

## Cross-cutting scientific and operational release gates

- Metametadata sample mapping, repeated measurements, missing-modality rules and exclusions.
- Synthetic 8+ sample balanced tests and longitudinal repeated-patient tests, then full real-cohort flight tests.
- Per-modality error isolation and bounded parsing of hundreds of XLSX files.
- Memory and duration measurements, no fixed file-size cap, cancellation and resumability.
- Same schema, result provenance, model settings, exports, and local-only data protections.

UX15 intentionally changes no scientific engine or longitudinal/significant-hit capability. It adds a feedback path and records this migration plan.
