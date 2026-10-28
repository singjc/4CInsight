// Match portable manifest entries to files that the user explicitly selected.
// This module does not read file contents or request filesystem permissions.
function path(value) {
  return String(value || '').replaceAll('\\', '/').replace(/^\.\//, '').replace(/\/+$/, '')
}

function normalizedPath(record) {
  return path(record?.relativePath || record?.webkitRelativePath || record?.name)
}

function fileName(record) {
  return String(record?.name || normalizedPath(record).split('/').at(-1) || '')
}

function signature(record) {
  return `${fileName(record)}\u0000${Number(record?.size)}`
}

// Compare complete trailing path segments, not arbitrary string substrings.
// This keeps subgroup paths useful after a cohort root has been renamed.
function trailingPathSegments(a, b) {
  if (!a || !b) return 0
  const left = a.split('/')
  const right = b.split('/')
  let count = 0
  while (count < left.length && count < right.length && left[left.length - 1 - count] === right[right.length - 1 - count]) count += 1
  return count
}

export function matchSavedFiles(manifest, selected) {
  const expected = Array.isArray(manifest) ? manifest : []
  const available = Array.isArray(selected) ? selected : []
  const bySignature = new Map()
  for (const record of available) {
    const key = signature(record)
    if (!bySignature.has(key)) bySignature.set(key, [])
    bySignature.get(key).push(record)
  }

  const matches = new Map()
  const missing = []
  const ambiguous = []
  const consumed = new Set()

  for (let index = 0; index < expected.length; index += 1) {
    const saved = expected[index]
    const candidates = (bySignature.get(signature(saved)) || []).filter((item) => !consumed.has(item))
    const savedPath = normalizedPath(saved)
    // Exact paths win. Otherwise choose the longest matching suffix path.
    // A filename-only match is safe only when it identifies one candidate.
    const scored = candidates.map((item) => ({
      item,
      score: normalizedPath(item) === savedPath ? 10_000 : trailingPathSegments(normalizedPath(item), savedPath),
    }))
    let bestScore = 0
    for (const entry of scored) if (entry.score > bestScore) bestScore = entry.score
    const best = scored.filter((entry) => entry.score === bestScore).map((entry) => entry.item)
    if (best.length === 1) {
      consumed.add(best[0])
      matches.set(index, best[0])
    } else if (best.length > 1) {
      ambiguous.push(saved)
    } else {
      missing.push(saved)
    }
  }
  return {
    expected: expected.length,
    matched: matches.size,
    missing,
    ambiguous,
    matches,
    additional: available.length - matches.size,
  }
}

export function matchSavedReference(reference, selected) {
  if (!reference) return null
  const result = matchSavedFiles([reference], selected)
  return result.matches.get(0) || null
}
