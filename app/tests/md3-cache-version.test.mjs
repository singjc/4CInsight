import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const read = (name) => readFile(new URL(`../${name}`, import.meta.url), 'utf8')

test('browser CSS and JavaScript assets share the current application cache version', async () => {
  const [html, main] = await Promise.all([read('index.html'), read('main.js')])
  const cssVersion = html.match(/href="\.\/styles\.css\?v=([^"]+)"/)?.[1]
  const jsVersion = html.match(/src="\.\/main\.js\?v=([^"]+)"/)?.[1]
  const appVersion = main.match(/const APP_VERSION = '([^']+)'/)?.[1]
  assert.ok(cssVersion, 'stylesheet must have a cache version')
  assert.ok(jsVersion, 'application script must have a cache version')
  assert.ok(appVersion, 'application code must declare its version')
  assert.equal(cssVersion, jsVersion, 'stylesheet and application cache versions differ')
  assert.equal(jsVersion, appVersion, 'cache version differs from APP_VERSION')
})
