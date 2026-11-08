import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

const app = fileURLToPath(new URL('../', import.meta.url))
const read = name => readFileSync(join(app, name), 'utf8')

test('UX15/R1 keeps Project guide version checks without pinning the old release name', () => {
  const main = read('main.js')
  const html = read('index.html')
  const testSource = read('tests/ux14-project-input-guide.test.mjs')
  const version = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.ok(version, 'Missing application cache version')
  assert.equal(version, '0.21.0-i0-integrative-evidence')
  assert.match(testSource, /Application cache revision must be defined/)
  assert.doesNotMatch(testSource, /version\?\.includes\('ux14-input-guide'\)/)
  assert.ok(html.includes(`styles.css?v=${version}`))
  assert.ok(html.includes(`main.js?v=${version}`))
  assert.ok(main.includes(`project-input-guide.js?v=${version}`))
  assert.match(read('styles.css'), /\.project-guide-details/)
})
