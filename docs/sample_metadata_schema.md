# Standard multi-omics sample metadata

4C Insight v0.18.0 defines a standard **one-row-per-biological-sample** metadata table. The table is intentionally a plain UTF-8 TSV so it can be edited in Excel/LibreOffice/R/Python or curated with a local metadata editor.

## Core rules

- `sample_id` is the canonical unique sample identifier and is required.
- `subject_id` is strongly recommended whenever a participant contributes repeated samples.
- `analysis_group` is the preferred standardized grouping field for simple custom case/control comparisons.
- Any metadata column can be used for a custom comparison in Cohort Analysis; `analysis_group` is only a recommended convention.
- `Phenotype` and `HYP` are retained for compatibility with the two historical 4C presets.
- Rows may contain arbitrary additional metadata columns. They are preserved and can be selected as corrected-model covariates or plot-coloring fields.
- Automatic outlier/QC decisions are **not** stored as final truth in this metadata table. `qc_review_status`, `known_wrong_source`, and related fields are curator provenance; run-specific outlier evidence/manual decisions remain frozen inside each completed analysis run.
- A curator-authored metadata flag **can** be used as an explicit per-run sample exclusion rule. In the unified analysis UI, choose any metadata column and one or more observed values to exclude. This does not require running automatic outlier QC, and the selected rule plus exact excluded IDs are frozen into the completed run.

## Flexible column roles for imported longitudinal metadata

The standard 4C template convention remains `sample_id = biological observation/specimen` and `subject_id = participant/patient`. Imported collaborator metadata does **not** have to follow those literal names.

For the dedicated **Longitudinal patient-signature clustering** workflow, the workflow provides dropdowns for:

- the observation / longitudinal sample ID column;
- the stable patient / person ID column;
- an optional collection-date / timepoint column; and
- an optional QC/review column plus the distinct values to exclude.

The selected roles, not the header spelling, define the analysis. For example, a collaborator sheet in which `subject_id` identifies each longitudinal specimen while `sample_id` identifies the person can be analyzed by selecting `subject_id` as **Observation / longitudinal sample ID** and `sample_id` as **Patient / person ID**. There is no requirement to rewrite the collaborator's headers.

If the selected QC/review column contains a value named `Failed`, the UI can automatically propose `Failed` for exclusion; the user can add/remove any distinct status values before running. The completed run freezes the chosen role mapping and excluded values into the result/report provenance.

## Cross-modality identifiers

The canonical `sample_id` should represent the biological sample. If an upstream modality uses a different identifier, use the optional alias columns:

- `gene_expression_id`
- `transcript_expression_id`
- `bcr_id`
- `bcr_isotype_id`
- `tcr_alpha_id`
- `tcr_beta_id`
- `immune_infiltrate_id`

4C Insight resolves these aliases back to the canonical `sample_id`. Aliases therefore do not create additional biological observations or inflate case/control counts.

## Custom comparisons

The unified Cohort Analysis workflow supports:

1. the historical built-in 4C presets; or
2. **Custom from metadata**.

For a custom comparison, select one metadata column and assign one or more distinct values to **Case** and **Control**. Rows with values assigned to neither group remain outside that comparison. Examples include:

- `analysis_group`: `HYP` vs `True_HC`;
- `diagnosis`: `DiseaseA` vs `Healthy`;
- `treatment`: `Drug` vs `Placebo`;
- `timepoint`: `Post` vs `Baseline`;
- a combined field curated by the user, such as `analysis_group = Severe_case / Mild_case / Control`.

The corrected/standard statistical engine is unchanged; the metadata selector only determines which canonical samples populate the generic case/control comparison.

## Fields

| Field | Level | Section | Purpose |
|---|---|---|---|
| `sample_id` | required | Identity | Canonical unique sample identifier. |
| `subject_id` | recommended | Identity | Participant/patient ID for repeated-measure handling. |
| `analysis_group` | recommended | Cohort | Preferred general-purpose custom comparison field. |
| `cohort`, `diagnosis`, `disease_subtype` | optional | Cohort | Cohort and disease classification. |
| `Phenotype`, `HYP` | legacy | Cohort | Compatibility with historical 4C presets. |
| `timepoint`, `collection_date` | recommended/optional | Collection | Longitudinal visit/date provenance. |
| `sample_source`, `tissue`, `specimen_type` | recommended/optional | Specimen | Tissue/source information used for QC and sensitivity interpretation. |
| `sex`, `age_years`, `family_id` | optional | Covariates | Common biological/structural covariates. |
| `site`, `study_id` | optional | Provenance | Recruitment/project provenance. |
| `collection_batch`, `processing_batch`, `library_batch`, `sequencing_batch`, `plate`, `well`, `technical_replicate` | optional/recommended | Batch | Potential technical/confounding structure. |
| modality `*_id` columns | optional | Modality mapping | Per-modality aliases mapped to canonical `sample_id`. |
| `analysis_include`, `known_wrong_source`, `qc_review_status`, `qc_exclusion_reason`, `notes` | optional | Review | Curation/provenance fields; do not replace frozen per-run QC. |

The repository template is `templates/4C_multiomics_sample_metadata_template.tsv`. The Metadata Builder can also create the same schema directly from a scanned cohort project and pre-populate canonical sample IDs.
