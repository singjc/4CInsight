import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import test from 'node:test'

const app = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const html = readFileSync(path.join(app, 'index.html'), 'utf8')
const main = readFileSync(path.join(app, 'main.js'), 'utf8')
const css = readFileSync(path.join(app, 'styles.css'), 'utf8')
const header = html.match(/<header\s+class="topbar"[^>]*>([\s\S]*?)<\/header>/)?.[1] ?? ''
const project = main.slice(main.indexOf('function projectView() {'), main.indexOf('function ', main.indexOf('function projectView() {') + 24))
const projectBind = main.slice(main.indexOf('function bindProjectEvents() {'), main.indexOf('function bindQcEvents() {'))

function count(haystack, needle) {
  return haystack.split(needle).length - 1
}

test('load, save and hidden chooser are in the permanent banner exactly once', () => {
  assert.ok(header, 'expected a permanent topbar')
  for (const id of ['load-state', 'save-state', 'state-input']) {
    assert.equal(count(header, `id="${id}"`), 1, `${id} must be a single banner control`)
    assert.equal(count(html, `id="${id}"`), 1, `${id} cannot be duplicated`)
    assert.equal(count(project, `id="${id}"`), 0, `${id} must leave Project`)
  }
  assert.match(header, /aria-label="Project state"/)
  assert.match(header, /state-input[^>]*type="file"[^>]*accept="\.json,\.4cstate"/)
  assert.match(project, /Private by design/)
})

test('global state controls bind once, outside per-tab render and Project', () => {
  assert.equal(count(main, 'function bindGlobalStateActions()'), 1)
  assert.match(main, /void registerIsolationServiceWorker\(\)\s*bindGlobalStateActions\(\)\s*render\(\)/)
  assert.equal(count(main, 'bindGlobalStateActions()'), 2, 'one definition and one call')
  assert.doesNotMatch(projectBind, /#(?:load-state|save-state|state-input)|stateInput/)
  assert.match(main, /function bindGlobalStateActions\([\s\S]*?saveApplicationState\(\)/)
  assert.match(main, /function bindGlobalStateActions\([\s\S]*?loadApplicationState\(file\)/)
})

test('saving state does not reset a zoomed view on another tab', () => {
  const start = main.indexOf('async function saveApplicationState() {')
  const end = main.indexOf('async function loadApplicationState(file) {', start)
  const save = main.slice(start, end)
  assert.match(save, /if \(state\.tab === 'project'\) render\(\)/)
  assert.doesNotMatch(save, /(?<!\))\n  render\(\)/)
})

test('state chooser allows the same file to be selected again', () => {
  assert.match(main, /const file = event\.currentTarget\.files\?\.\[0\]/)
  assert.match(main, /event\.currentTarget\.value = ''/)
  assert.match(main, /if \(!file\) return/)
})

test('state file keeps its existing format, local references, and separate results', () => {
  assert.match(main, /async function saveApplicationState\(\)/)
  assert.match(main, /async function loadApplicationState\(file\)/)
  assert.match(main, /4c-insight-state\.4cstate\.json/)
  assert.match(main, /not raw cohort data/)
  assert.match(header, /Analysis results are saved separately/)
})

test('banner buttons are keyboard accessible and fit smaller windows', () => {
  assert.match(css, /\.topbar-state-button:focus-visible/)
  assert.match(css, /\.topbar-state-actions/)
  assert.match(css, /@media \(max-width: 640px\)/)
  assert.match(header, /aria-live="polite"/)
})

test('main, stylesheet and HTML use one cache revision', () => {
  const rev = '0.21.0-ux14-input-guide'
  assert.match(main, /const APP_VERSION = '0\.21\.0-ux14-input-guide'/)
  assert.match(html, /main\.js\?v=0\.21\.0-ux14-input-guide/)
  assert.match(html, /styles\.css\?v=0\.21\.0-ux14-input-guide/)
  assert.doesNotMatch(main, /0\.21\.0-ux12-true-axis-zoom/)
})
