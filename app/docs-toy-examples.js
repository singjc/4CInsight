import { multiomicsToyExamples } from './docs-new-modalities.js?v=0.21.0-ux14-input-guide'
// Explanatory examples for the 4C Insight Docs page.
// Numbers are illustrative. They are not fitted results or scientific fixtures.
// Keep these examples separate from real cohort results.
import { renderToyVisual, visualCaption } from './docs-toy-visuals.js?v=0.21.0-ux14-input-guide'
export { bindToyVisualControls } from './docs-toy-visuals.js?v=0.21.0-ux14-input-guide'
export const methodToyExamples = {
  ...multiomicsToyExamples,
  'local-first': [
    {
      title: 'Save a project without copying its input files',
      question: 'What happens when you save a state and open it next week?',
      columns: ['Step', 'What the browser keeps', 'What the user does'],
      rows: [
        ['Select files', 'Local file access for this session', 'Select a cohort folder'],
        ['Save state', 'File names, references, and settings in JSON', 'Download the state JSON'],
        ['Load state later', 'References only; no raw expression data', 'Reconnect the cohort folder once'],
      ],
      steps: [
        'A saved state is not a copy of the source data.',
        'After reconnecting, the application matches paths and sizes. It reports missing and ambiguous files.',
      ],
      meaning: 'Your raw files stay on your computer. A state JSON can be shared without embedding those files.',
      limit: 'File references do not grant permanent permission to read local files.',
    },
  ],
  'file-merging': [
    {
      title: 'Merge two sample workbooks into one gene table',
      question: 'How does 4C Insight turn one workbook per sample into a feature-by-sample table?',
      columns: ['Gene ID', 'S01.xlsx Raw Count', 'S02.xlsx Raw Count'],
      rows: [['G1', '10', '20'], ['G2', '30', '50'], ['G3', 'Not in reference', '7 (extra row)']],
      steps: [
        'The first sorted workbook defines the feature list. Here, S01.xlsx contains G1 and G2.',
        'The merger matches G1 and G2 by feature identifier, not by row number.',
        'The output has two sample columns and the canonical rows G1 and G2. The extra G3 row is counted in the merge report.',
      ],
      meaning: 'Output: G1 = [10, 20] and G2 = [30, 50]. Each column belongs to a sample.',
      limit: 'G3 is not added because it was absent from the reference workbook. Check unknown- and missing-feature counts before analysis.',
    },
  ],
  'sample-metadata': [
    {
      title: 'Link expression files to biological samples',
      question: 'How can file names and biological sample IDs stay separate?',
      columns: ['sample_id', 'analysis_group', 'gene_expression_id'],
      rows: [['PatientA_T0', 'Control', 'GX_001'], ['PatientA_T1', 'Case', 'GX_002']],
      steps: [
        'Create one metadata row per analyzed observation. Choose a stable sample ID.',
        'Map GX_001 and GX_002 source files to the correct rows. Review ambiguous matches.',
        'Export the edited metadata TSV and a separate file-mapping TSV.',
      ],
      meaning: 'The mapping makes source-file identities traceable without changing original file names.',
      limit: 'The mapping manifest alone does not rewrite expression-table columns. Compare Groups still requires an appropriate metadata ID column that matches the expression columns.',
    },
  ],
  'expression-qc': [
    {
      title: 'Find zeros and missing values before comparison',
      question: 'What do distribution summaries tell you?',
      columns: ['Feature', 'Sample A count', 'Sample B count'],
      rows: [['G1', '0', '5'], ['G2', '10', '10'], ['G3', '20', '15'], ['G4', 'Missing', '20']],
      steps: [
        'Sample A has three measured values: 0, 10, and 20. The median measured value is 10.',
        'One of four entries is missing (25%). One of three measured entries is zero (about 33%).',
        'Compare these summaries with other samples to identify unusual distributions for review.',
      ],
      meaning: 'A high missing or zero fraction is a QC signal. It is not proof that the sample must be excluded.',
      limit: 'This is a small arithmetic example. Actual missing-value handling depends on the selected input and QC metric.',
    },
  ],
  'normalization-nb2': [
    {
      title: 'Separate sequencing depth from expression',
      question: 'Why does a sample with twice the raw counts not always have twice the expression?',
      columns: ['Gene', 'Sample A raw count', 'Sample B raw count'],
      rows: [['G1', '10', '20'], ['G2', '20', '40'], ['G3', '40', '80']],
      steps: [
        'For G1, the across-sample geometric mean is √(10 × 20) ≈ 14.14.',
        'The ratio for G1 is 10 / 14.14 ≈ 0.707 in A and 20 / 14.14 ≈ 1.414 in B. G2 and G3 have the same ratios.',
        'Median-ratio size factors are therefore about 0.707 and 1.414, after geometric-mean centering.',
        'Divide each count by its size factor. G1 becomes about 14.14 in both samples.',
      ],
      meaning: 'These samples differ in total count depth but not in normalized gene expression in this example.',
      limit: 'The full model also estimates per-feature NB2 dispersion and covariate-adjusted effects. Normalized counts alone are not a statistical test.',
    },
    {
      title: 'Read a corrected group effect',
      question: 'What does a model log2 group effect of +1 mean?',
      columns: ['Comparison', 'Adjusted expression scale', 'Effect'],
      rows: [['Control', '10', 'Reference'], ['Case', '20', 'log2(20 / 10) = +1']],
      steps: [
        'The NB2 generalized linear model uses a log link and a log(size factor) offset.',
        'Convert the fitted group coefficient from natural-log units to log2 units: β_group / ln(2).',
        'A log2 effect of +1 means a two-fold fitted mean ratio for Case relative to Control, holding modeled covariates constant.',
      ],
      meaning: 'A positive effect points toward greater expression in Case. A negative effect points toward lower expression.',
      limit: 'The table is only an effect-size illustration. A p-value requires fitted dispersion, uncertainty, and sample-level data.',
    },
  ],
  'fdr-volcano': [
    {
      title: 'Adjust four p-values with Benjamini–Hochberg',
      question: 'Which features pass FDR ≤ 0.05?',
      columns: ['Sorted rank', 'Raw p-value', 'BH adjusted q-value'],
      rows: [['1', '0.001', '0.004'], ['2', '0.020', '0.040'], ['3', '0.040', '≈ 0.0533'], ['4', '0.200', '0.200']],
      steps: [
        'There are m = 4 tests. For each sorted p-value, compute p × m / rank.',
        'Make the adjusted values monotone from the largest rank to the smallest, and cap at 1.',
        'The first two q-values are at or below 0.05, so two features pass the FDR threshold.',
      ],
      meaning: 'BH adjustment accounts for testing many features. It does not mean that each selected feature has a 5% probability of being false.',
      limit: 'This is a hand-calculated example. The application uses all tested features, not a subset of four.',
    },
    {
      title: 'Apply the two volcano thresholds',
      question: 'Why can a significant feature remain gray?',
      columns: ['Feature', 'log2 effect', 'BH q', 'Result at q ≤ 0.05, |effect| ≥ 1'],
      rows: [['A', '+1.5', '0.004', 'Up (red)'], ['B', '−0.8', '0.040', 'Below effect cutoff (gray)'], ['C', '−2.0', '0.080', 'Below FDR cutoff (gray)']],
      steps: [
        'Place each feature at x = log2 effect and y = −log10(q).',
        'For q = 0.05, the dashed horizontal cutoff is y ≈ 1.301.',
        'Only feature A meets both thresholds. Changing the controls changes its classification, not the fitted model.',
      ],
      meaning: 'A volcano plot combines effect size and evidence of a nonzero effect.',
      limit: 'Do not use the display cutoffs as a substitute for independent validation.',
    },
  ],
  'pca-clustering': [
    {
      title: 'See how PCA finds the main direction of variation',
      question: 'Which samples look similar without using group labels?',
      columns: ['Sample', 'Normalized feature 1', 'Normalized feature 2'],
      rows: [['S1', '2', '2'], ['S2', '3', '3'], ['S3', '8', '8'], ['S4', '9', '9']],
      steps: [
        'Both features increase together. After centering, the strongest direction runs along the diagonal of the two-feature plot.',
        'The first principal component captures all variation in this exact toy matrix. The second captures none.',
        'S1 and S2 appear near each other; S3 and S4 appear near each other. No Case/Control labels were needed.',
      ],
      meaning: 'PCA shows large sources of variation. It cannot, by itself, identify the biological cause.',
      limit: 'Real data have many features. Scaling, normalization, and feature selection affect the displayed coordinates.',
    },
    {
      title: 'Join similar samples in a dendrogram',
      question: 'How does Euclidean distance determine the first clusters?',
      columns: ['Sample pair', 'Distance', 'Interpretation'],
      rows: [['S1–S2', '√2 ≈ 1.41', 'Near'], ['S3–S4', '√2 ≈ 1.41', 'Near'], ['S1–S4', '√98 ≈ 9.90', 'Far']],
      steps: [
        'Euclidean distance is the square root of summed squared feature differences.',
        'Agglomerative clustering joins the nearest observations or clusters first.',
        'Average linkage uses the mean pairwise distance between members of two clusters.',
      ],
      meaning: 'The dendrogram summarizes similarity. Branches are not evidence of distinct biological subtypes on their own.',
      limit: 'Distances here use the two-feature toy matrix above, not an actual cohort fit.',
    },
  ],
  'transcript-usage': [
    {
      title: 'Find a change in transcript mixture',
      question: 'What if total gene count stays the same but isoform use changes?',
      columns: ['Sample', 'Transcript T1 count', 'Transcript T2 count'],
      rows: [['Sample A', '9', '1'], ['Sample B', '1', '9']],
      steps: [
        'Both samples have 10 gene-level reads. A total-gene comparison alone would miss the composition change.',
        'The DTU screen adds a pseudocount of 0.5 to each transcript count.',
        'For A, CLR(T1) = ½ ln(9.5 / 1.5) ≈ +0.923. For B, CLR(T1) ≈ −0.923. T2 has the opposite sign.',
        'The screen tests transcript-level CLR effects and combines p-values within each gene with Simes, then adjusts gene-level p-values with BH.',
      ],
      meaning: 'DTU concerns relative transcript use within a gene, not only total gene expression.',
      limit: 'Two samples cannot establish significance. The values illustrate the CLR transformation, not a fitted DTU result.',
    },
  ],
  longitudinal: [
    {
      title: 'Calculate patient-identity ranking AUC',
      question: 'Are repeat samples from one patient closer than samples from different patients?',
      columns: ['Pair type', 'Toy sample distances'],
      rows: [['Same patient', '0.2, 1.3'], ['Different patients', '0.8, 1.0, 1.2, 1.4']],
      steps: [
        'Compare every same-patient distance with every different-patient distance: 2 × 4 = 8 comparisons.',
        'The 0.2 distance beats all four different-patient distances. The 1.3 distance beats only 1.4.',
        'There are 5 favorable comparisons, so patient-identity AUC = 5 / 8 = 0.625.',
      ],
      meaning: 'AUC above 0.5 indicates some same-patient ranking signal in this example.',
      limit: 'This uses a toy distance list. In a real cohort, the distance matrix is computed before patient labels are used.',
    },
  ],
  'significant-hits': [
    {
      title: 'Understand post-selection separation',
      question: 'Why can a heatmap look clean without proving prediction accuracy?',
      columns: ['Feature', 'BH q', '|log2 effect|', 'Selected at q ≤ 0.05 and |effect| ≥ 1'],
      rows: [['G1', '0.003', '2.0', 'Yes'], ['G2', '0.030', '1.2', 'Yes'], ['G3', '0.200', '0.4', 'No']],
      steps: [
        'The Case/Control labels help select G1 and G2.',
        'PCA, clustering, and the heatmap are then drawn using only G1 and G2.',
        'If the selected samples separate well, that is a description of the same data used to select the features.',
      ],
      meaning: 'The plots can show a useful expression pattern, but they cannot measure out-of-sample prediction accuracy.',
      limit: 'Independent evaluation needs an external cohort or feature selection inside each training split.',
    },
  ],
  exclusions: [
    {
      title: 'Exclude failed QC samples before model fitting',
      question: 'What happens when QC_status = Failed is selected as an exclusion rule?',
      columns: ['sample_id', 'Group', 'QC_status', 'Included?'],
      rows: [['S01', 'Case', 'Pass', 'Yes'], ['S02', 'Case', 'Failed', 'No'], ['S03', 'Control', 'Pass', 'Yes'], ['S04', 'Control', 'Pass', 'Yes']],
      steps: [
        'Choose QC_status as the metadata column and Failed as the value to exclude.',
        'S02 is removed before group construction. The other three samples remain.',
        'The result records the rule and the excluded sample ID so the choice can be reviewed later.',
      ],
      meaning: 'The analysis uses only the included samples. Exclusion can change group balance and statistical power.',
      limit: 'No sample is automatically rejected just because a QC value looks unusual; the user must select the rule.',
    },
  ],
  performance: [
    {
      title: 'Understand the information shown in Settings',
      question: 'What do browser CPU and memory figures tell you?',
      columns: ['Resource', 'Illustrative reading', 'Meaning'],
      rows: [['Browser CPU report', '20 logical threads', 'Capacity estimate'], ['Analysis status', '1 worker', 'Runs one task at a time'], ['Analysis memory', '512 MiB allocated', 'Reserved for calculations, not total RAM']],
      steps: [
        'The reported CPU count is device information. It does not guarantee that every task runs in parallel.',
        'Analyses run one task at a time. The application keeps the interface available for other actions.',
        'The analysis-memory figure is only the amount allocated for calculations. It does not show total device RAM.',
      ],
      meaning: 'CPU information and analysis memory describe different device resources.',
      limit: 'Firefox does not expose exact total or available physical RAM to ordinary websites.',
    },
  ],
}

const escapeHtml = (value) => String(value ?? '')
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;')

export function exampleSearchText(sectionId) {
  return (methodToyExamples[sectionId] || []).flatMap((example) => [
    example.title, example.question, ...example.columns,
    ...example.rows.flat(), ...example.steps, example.meaning, example.limit,
    visualCaption(sectionId, (methodToyExamples[sectionId] || []).indexOf(example)),
  ]).join(' ')
}

export function renderToyExamples(sectionId) {
  const examples = methodToyExamples[sectionId] || []
  return examples.map((example, index) => `<details class="docs-toy-example" data-doc-example>
    <summary><span class="docs-toy-marker" aria-hidden="true">PLOT</span><span>Visual example ${index + 1}: ${escapeHtml(example.title)}</span></summary>
    <div class="docs-toy-content">
      <p class="docs-toy-question">${escapeHtml(example.question)}</p>
      ${renderToyVisual(sectionId, index)}
      <p class="docs-toy-meaning"><strong>Interpretation:</strong> ${escapeHtml(example.meaning)}</p>
      <p class="docs-toy-limit"><strong>Important:</strong> ${escapeHtml(example.limit)}</p>
      <details class="docs-toy-math"><summary>Show input values and the calculation</summary>
        <div class="docs-toy-table-wrap"><table class="docs-toy-table"><thead><tr>${example.columns.map((column) => `<th scope="col">${escapeHtml(column)}</th>`).join('')}</tr></thead><tbody>${example.rows.map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join('')}</tr>`).join('')}</tbody></table></div>
        <strong class="docs-toy-subheading">Follow the calculation</strong>
        <ol class="docs-toy-steps">${example.steps.map((step) => `<li>${escapeHtml(step)}</li>`).join('')}</ol>
      </details>
      <small class="docs-toy-note">Illustrative values only. This example is not a result from your cohort.</small>
    </div>
  </details>`).join('')
}
