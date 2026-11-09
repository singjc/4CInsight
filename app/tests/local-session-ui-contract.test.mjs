import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const read = name => readFileSync(new URL(`../${name}`, import.meta.url), 'utf8')
const main = read('main.js')
const html = read('index.html')
const css = read('local-session.css')

test('permanent header has exactly one Clear state next to Load and Save, and a visible local status', () => {
  const header = html.match(/<header class="topbar"[^>]*>([\s\S]*?)<\/header>/)?.[1] || ''
  for (const id of ['load-state', 'save-state', 'clear-state', 'state-input', 'local-session-status'])
    assert.equal(header.split(`id="${id}"`).length - 1, 1, id)
  assert.ok(header.indexOf('id="save-state"') < header.indexOf('id="clear-state"'))
  assert.match(header, /no raw data|file references/i)
  assert.match(css, /\.local-session-status/)
})

test('auto-restore occurs at startup and saving follows non-busy render', () => {
  assert.match(main, /bindGlobalStateActions\(\)\s*render\(\)\s*void restoreActiveLocalSession\(\)/)
  assert.match(main, /function scheduleLocalSessionSave\(/)
  assert.match(main, /async function restoreActiveLocalSession\(/)
  assert.match(main, /async function saveCurrentLocalSession\(/)
  assert.match(main, /if \(!localSessionReady \|\| localSessionRestoring \|\| localSessionDisabled \|\| activeSessionWorkRunning\(\)\)/)
  assert.match(main, /readLocalResults\(saved\.resultTokens\)/)
  assert.match(main, /recoverSourceFile\(entry, fileKey\)/)
  assert.match(main, /expected\.has\(entry\.id\)/, 'Do not restore unrelated files')
  assert.match(main, /saved\.payload\.appVersion === APP_VERSION/, 'Do not load scientific results from an incompatible app build')
  assert.match(main, /scheduleLocalSessionSave\(\)/)
})

test('automatic session keeps integrative results, all data types and analysis settings without serializing raw bytes into manual JSON', () => {
  for (const key of ['i2bDirectViews', 'i2StructureResult', 'jointStabilityResult', 'bioFeatureResult', 'bioResult', 'qcResult', 'compareResult', 'longitudinalResult', 'hitResult', 'alignmentPreview'])
    assert.match(main, new RegExp(`['"]${key}['"]`))
  assert.match(main, /integrativeView: state\.integrativeView/)
  assert.match(main, /alignmentRoles: \{ \.\.\.state\.alignmentRoles \}/)
  assert.match(main, /jointStabilityConfig: \{ \.\.\.state\.jointStabilityConfig \}/)
  assert.match(main, /function applicationStatePayload\(\)/)
  assert.match(main, /Raw cohort bytes are not embedded in this state/)
  assert.match(main, /4c-insight-state\.4cstate\.json/)
})

test('Clear state is guarded and affects only active local session; original files and History are retained', () => {
  assert.match(main, /function clearSavedActiveSession\(/)
  assert.match(main, /window\.confirm\(/)
  assert.match(main, /await clearLocalSession\(\)/)
  assert.match(main, /original files and separate History entries will not be deleted/i)
  assert.match(main, /#clear-state/)
  assert.doesNotMatch(main.slice(main.indexOf('async function clearSavedActiveSession()'), main.indexOf('async function saveApplicationState()')), /clearHistoryEntries\(/)
})

test('cache-revision contract is preserved for existing application assets', () => {
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.equal(version, '0.22.8')
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.ok(main.includes(`local-session.js?v=${version}`))
  const scriptVersions = [...main.matchAll(/\.js\?v=(\d+\.\d+\.\d+(?:-[a-z0-9-]+)?)/g)]
  for (const hit of scriptVersions) assert.equal(hit[1], version, 'Do not add a stale versioned import')
})
