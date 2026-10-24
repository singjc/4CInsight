const workbookPatterns = [
  ['_tumor_rna_gene_expression_report.xlsx', 'gene-expression', 'Gene expression'],
  ['_tumor_rna_transcript_expression_report.xlsx', 'transcript-expression', 'Transcript expression'],
  ['_bcr_heavy_chain_report.xlsx', 'bcr', 'BCR'],
  ['_bcr_isotype_composition.xlsx', 'bcr-isotype', 'BCR isotype'],
  ['_tcr_alpha_clone_report.xlsx', 'tcr-alpha', 'TCR alpha'],
  ['_tcr_beta_clone_report.xlsx', 'tcr-beta', 'TCR beta'],
  ['_immune_infiltrate.xlsx', 'immune-infiltrate', 'Immune infiltrate'],
]

function stripXlsxCopySuffix(name) {
  return name.replace(/ \(\d+\)(\.xlsx)$/i, '$1')
}

export function classifyInputFilename(name) {
  const canonicalName = stripXlsxCopySuffix(name)
  const lower = canonicalName.toLowerCase()

  for (const [suffix, kind, label] of workbookPatterns) {
    if (lower.endsWith(suffix)) {
      const sampleId = canonicalName.slice(0, canonicalName.length - suffix.length).trim()
      if (sampleId) return { recognized: true, kind, label, sampleId }
    }
  }

  const rawLower = name.toLowerCase()
  if (rawLower.endsWith('.tsv') && rawLower.includes('merged_gene_expression')) {
    return { recognized: true, kind: 'merged-gene-expression', label: 'Merged gene expression', sampleId: null }
  }
  if (rawLower.endsWith('.tsv') && rawLower.includes('merged_transcript_expression')) {
    return { recognized: true, kind: 'merged-transcript-expression', label: 'Merged transcript expression', sampleId: null }
  }
  const metadataExtension = ['.xlsx', '.xls', '.tsv', '.csv', '.txt'].some((extension) => rawLower.endsWith(extension))
  if (
    metadataExtension &&
    (rawLower.includes('sample') || rawLower.includes('cohort')) &&
    (rawLower.includes('submission') || rawLower.includes('metadata'))
  ) {
    return { recognized: true, kind: 'metadata-candidate', label: 'Metadata candidate', sampleId: null }
  }
  return { recognized: false, kind: null, label: null, sampleId: null }
}
