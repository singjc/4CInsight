/**
 * Continuous table viewport. All data rows stay in the model. Only the visible
 * rows and a small overscan region are placed in the DOM.
 *
 * Fixed row heights are required. The CSS and window calculation must agree.
 */
export const TABLE_ROW_HEIGHT = 48
export const TABLE_OVERSCAN = 8

export function tableWindow(totalRows, scrollTop, viewportHeight, rowHeight = TABLE_ROW_HEIGHT, overscan = TABLE_OVERSCAN) {
  const total = Math.max(0, Math.trunc(Number(totalRows) || 0))
  const height = Number(rowHeight)
  if (!Number.isFinite(height) || height <= 0) throw new RangeError('Table row height must be positive')
  const top = Math.max(0, Number.isFinite(Number(scrollTop)) ? Number(scrollTop) : 0)
  const view = Math.max(height, Number.isFinite(Number(viewportHeight)) ? Number(viewportHeight) : height)
  const extra = Math.max(0, Math.trunc(Number(overscan) || 0))
  const maxTop = Math.max(0, total * height - view)
  const first = Math.max(0, Math.floor(Math.min(top, maxTop) / height) - extra)
  const end = Math.min(total, Math.ceil((Math.min(top, maxTop) + view) / height) + extra)
  return { first, end, topPadding: first * height, bottomPadding: (total - end) * height, visible: end - first, total }
}

export function tableRowRange(window) {
  if (!window.total) return '0 of 0 rows'
  return `${(window.first + 1).toLocaleString()}–${window.end.toLocaleString()} of ${window.total.toLocaleString()} rows`
}
