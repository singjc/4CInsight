import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const persistence = await readFile(new URL('../persistence.js', import.meta.url), 'utf8')
const styles = await readFile(new URL('../styles.css', import.meta.url), 'utf8')

test('project inventory accepts local drag and drop without upload staging', () => {
  assert.match(main, /id="input-drop-zone"/)
  assert.match(main, /event\.dataTransfer\?\.files/)
  assert.match(main, /addFiles\(files\)/)
  assert.match(main, /Files are referenced in place/)
  assert.doesNotMatch(main, /uploadWorkerFile|FormData\(/)
  assert.match(styles, /\.input-drop-zone\.is-dragover/)
})

test('application state saves settings and file references but not raw file bytes', () => {
  assert.match(main, /kind: 'application-state'/)
  assert.match(main, /4c-insight-state\.4cstate\.json/)
  assert.match(main, /Raw cohort bytes are not embedded in this state/)
  assert.match(main, /loadApplicationState/)
  assert.match(main, /pendingStateConfig/)
  assert.doesNotMatch(main, /arrayBuffer\(\).*application-state/s)
})

test('history uses browser-local IndexedDB snapshots with view export and delete actions', () => {
  assert.match(persistence, /indexedDB\.open/)
  assert.match(persistence, /history-snapshots/)
  assert.match(main, /\['history', 'History', 'H'\]/)
  assert.match(main, /recordHistory\('compare-groups-expression'/)
  assert.match(main, /recordHistory\('longitudinal-patient-signature-expression'/)
  assert.match(main, /recordHistory\('significant-hit-structure-expression'/)
  assert.match(main, /Original cohort files are not touched/)
  assert.match(main, /history-open/)
  assert.match(main, /history-export/)
  assert.match(main, /history-delete/)
})
