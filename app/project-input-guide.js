/**
 * User-facing file preparation guide.
 * The suffixes mirror app/file-classifier.js and src/input_conventions.rs.
 * All examples are illustrative, never assumed to be required folder paths.
 */
export const sampleWorkbookSuffixes = Object.freeze([
  ['Gene expression', '_tumor_rna_gene_expression_report.xlsx'],
  ['Transcript expression', '_tumor_rna_transcript_expression_report.xlsx'],
  ['BCR repertoire', '_bcr_heavy_chain_report.xlsx'],
  ['BCR isotypes', '_bcr_isotype_composition.xlsx'],
  ['TCR alpha', '_tcr_alpha_clone_report.xlsx'],
  ['TCR beta', '_tcr_beta_clone_report.xlsx'],
  ['Immune infiltration', '_immune_infiltrate.xlsx'],
])

export function renderProjectInputGuide() {
  const rows = sampleWorkbookSuffixes.map(([label, suffix]) => `
    <tr><th scope="row">${label}</th><td><code>SAMPLE${suffix}</code></td></tr>`).join('')
  return `
    <section class="project-guide-panel" aria-labelledby="project-guide-heading">
      <details class="project-guide-details" id="project-input-guide">
        <summary class="project-guide-summary">
          <span class="project-guide-symbol" aria-hidden="true">?</span>
          <span class="project-guide-summary-copy"><strong id="project-guide-heading">How to prepare your project files</strong>
            <small>See an example folder layout, supported filenames, and sample metadata.</small>
          </span>
          <span class="project-guide-action" aria-hidden="true">View guide <span class="project-guide-chevron">⌄</span></span>
        </summary>
        <div class="project-guide-content">
          <p class="project-guide-lead">Choose the folders supplied by your collaborators, or select files individually. <strong>No specific folder names are required.</strong> You do not need to sort files into per-sample folders. 4C Insight detects supported data types from filenames and reads files on this computer.</p>
          <div class="project-guide-grid">
            <section class="project-guide-subsection">
              <h4>Common layout: folders by data type</h4>
              <p class="project-guide-note project-guide-bulk-note"><strong>Keep the bulk folders you receive from collaborators.</strong> You do not have to move hundreds of workbooks into separate folders for each sample.</p>
              <pre class="project-guide-tree" aria-label="Example cohort organized by data type">my_cohort/
├── cohort_sample_metadata.tsv
├── 20260820_merged_gene_expression.tsv         (optional)
├── 20260820_merged_transcript_expression.tsv   (optional)
├── Expression_Gene_Files/
│   ├── 4C_100_tumor_rna_gene_expression_report.xlsx
│   └── 4C_101_tumor_rna_gene_expression_report.xlsx
├── Expression_Transcript_Files/
│   └── 4C_100_tumor_rna_transcript_expression_report.xlsx
├── BCR/
│   ├── 4C_100_bcr_heavy_chain_report.xlsx
│   └── 4C_101_bcr_heavy_chain_report.xlsx
├── BCR_Isotypes/
│   └── 4C_100_bcr_isotype_composition.xlsx
├── TCR/
│   ├── 4C_100_tcr_alpha_clone_report.xlsx
│   └── 4C_101_tcr_beta_clone_report.xlsx
└── Immune_Infiltrate/
    ├── 4C_100_immune_infiltrate.xlsx
    └── 4C_101_immune_infiltrate.xlsx</pre>
              <p class="project-guide-note">Folder names are examples, not rules. Select the parent folder to include its subfolders. Keep collaborator delivery folders as they are; check detected file types and sample mapping before analysis. Merged expression tables are optional alternatives to individual expression files.</p>
              <details class="project-guide-alternative">
                <summary>Alternative: folders by sample (optional)</summary>
                <p class="project-guide-note">Use this layout only if the data already comes organized by sample. You do not need to create it.</p>
                <pre class="project-guide-tree" aria-label="Optional example organized by sample">my_cohort/
├── cohort_sample_metadata.tsv
└── samples/
    ├── 4C_100/
    │   ├── 4C_100_bcr_heavy_chain_report.xlsx
    │   └── 4C_100_tcr_alpha_clone_report.xlsx
    └── 4C_101/
        └── 4C_101_immune_infiltrate.xlsx</pre>
              </details>
            </section>
            <section class="project-guide-subsection">
              <h4>Recognized sample filenames</h4>
              <p class="project-guide-note">The table shows recognized filename suffixes. <code>SAMPLE</code> is the sample identifier, for example <code>4C_100</code>. Folder names do not affect detection. If a collaborator uses another filename convention, review the unrecognized files before analysis.</p>
              <div class="project-guide-table-scroll"><table class="project-guide-table"><thead><tr><th scope="col">Data type</th><th scope="col">Filename pattern</th></tr></thead>
                <tbody>${rows}</tbody></table></div>
              <p class="project-guide-note">Metadata candidates can use <code>.xlsx</code>, <code>.xls</code>, <code>.tsv</code>, <code>.csv</code>, or <code>.txt</code> when the filename contains <code>sample</code> or <code>cohort</code> and also <code>metadata</code> or <code>submission</code>. Merged expression TSV filenames must contain <code>merged_gene_expression</code> or <code>merged_transcript_expression</code>.</p>
            </section>
          </div>
          <div class="project-guide-grid project-guide-lower">
            <section class="project-guide-subsection">
              <h4>Sample metadata</h4>
              <p class="project-guide-note">Use one row per biological sample. Select the ID and group columns in Compare Groups. Add a subject ID if a person has repeated samples.</p>
              <pre class="project-guide-metadata" aria-label="Example metadata with two samples">sample_id    subject_id    analysis_group
4C_100       Person_01     Case
4C_101       Person_02     Control</pre>
              <p class="project-guide-note">The column names shown are recommended, not required. If sample IDs differ between data types, use the Sample Metadata workspace to map the exact file names and aliases.</p>
            </section>
            <section class="project-guide-subsection">
              <h4>Inside the files</h4>
              <ul class="project-guide-checklist">
                <li><strong>Gene / transcript:</strong> 4C expression workbooks with named features and measurements, such as raw counts and TPM. The app can merge individual workbooks.</li>
                <li><strong>BCR / TCR:</strong> clonotype workbooks with a <code>Clone Count</code> column. V/J calls and CDR3 sequence columns support additional analyses.</li>
                <li><strong>BCR isotypes:</strong> <code>IGH Isotype</code> (or <code>Isotype</code>) and <code>Isotype Read Fraction</code>.</li>
                <li><strong>Immune infiltration:</strong> <code>Immune Cell Type</code> and <code>ssGSEA Score</code>.</li>
              </ul>
            </section>
          </div>
          <p class="project-guide-footnote"><strong>Before analysis:</strong> Select folder or files, check the detected types and sample IDs, then review sample mapping. A file marked <strong>Review</strong> was not classified automatically. Filenames alone do not confirm that a workbook has valid content.</p>
        </div>
      </details>
    </section>`
}
