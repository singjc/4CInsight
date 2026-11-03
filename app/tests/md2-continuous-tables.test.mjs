import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { tableWindow, tableRowRange, TABLE_ROW_HEIGHT } from '../table-viewport.js'
const read = name => readFile(new URL(`../${name}`, import.meta.url), 'utf8')

test('virtual table scrolls through 2983 samples without a page boundary', () => {
  const total = 2983
  const first = tableWindow(total, 0, 520)
  const middle = tableWindow(total, TABLE_ROW_HEIGHT * 1500, 520)
  const last = tableWindow(total, TABLE_ROW_HEIGHT * total, 520)
  assert.equal(first.first, 0)
  assert.ok(first.visible > 10 && first.visible < 50)
  assert.ok(middle.first < 1500 && middle.end > 1500)
  assert.equal(last.end, total)
  assert.ok(last.first > 2950)
  assert.equal(first.topPadding + first.visible * TABLE_ROW_HEIGHT + first.bottomPadding, total * TABLE_ROW_HEIGHT)
  assert.equal(middle.topPadding + middle.visible * TABLE_ROW_HEIGHT + middle.bottomPadding, total * TABLE_ROW_HEIGHT)
  assert.equal(last.topPadding + last.visible * TABLE_ROW_HEIGHT + last.bottomPadding, total * TABLE_ROW_HEIGHT)
  assert.match(tableRowRange(middle), /of 2,983 rows/)
})

test('small and empty tables and invalid scroll coordinates are safe', () => {
  assert.deepEqual(tableWindow(0, 0, 520), { first: 0, end: 0, topPadding: 0, bottomPadding: 0, visible: 0, total: 0 })
  const small = tableWindow(4, Number.NaN, 0)
  assert.equal(small.visible, 4)
  assert.equal(small.topPadding, 0)
  assert.equal(small.bottomPadding, 0)
  assert.throws(() => tableWindow(5, 0, 100, 0), /row height/)
})

test('metadata and mapping use continuous virtual scrolling and retain real source row indices', async () => {
  const main = await read('main.js')
  const css = await read('styles.css')
  assert.match(main, /function initializeMetadataViewport\(/)
  assert.match(main, /metadataFilteredRows\(\)/)
  assert.match(main, /metadataMappingFilteredRows\(\)/)
  assert.match(main, /data-row="\$\{index\}"/)
  assert.match(main, /data-file-id="\$\{escapeHtml\(record\.id\)\}"/)
  assert.doesNotMatch(main, /metadata-page-prev|metadata-page-next|mapping-prev|mapping-next|Page \$\{editor\.page/)
  assert.match(css, /\.metadata-editor-table thead th \{ position: sticky/)
  assert.match(css, /\.metadata-grid-scroll \{ height: min\(520px/)
})

test('differential and DTU tables scroll through all filtered records, without 500-row rendering cap', async () => {
  const main = await read('main.js')
  assert.match(main, /const drawDynamicWindow = \(\) =>/)
  assert.match(main, /dynamicRows\.slice\(window\.first, window\.end\)/)
  assert.match(main, /scroll\.addEventListener\('scroll', drawDynamicWindow/)
  assert.doesNotMatch(main, /source\.limit \|\| 500/)
  assert.match(main, /Scroll through all matching rows/)
})
