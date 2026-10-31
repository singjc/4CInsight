import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, access } from 'node:fs/promises'
import { detectBrowserResources, measureAppMemory, normalizeResources } from '../resource-controls.js'

const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')
const ci = await readFile(new URL('../../.github/workflows/ci.yml', import.meta.url), 'utf8')
const pages = await readFile(new URL('../../.github/workflows/pages.yml', import.meta.url), 'utf8')

test('Firefox-like browser reports CPU threads but does not invent RAM', () => {
  const result = detectBrowserResources({ platform: 'Linux x86_64', hardwareConcurrency: 20 }, {}, true)
  assert.equal(result.platform, 'Linux x86_64')
  assert.equal(result.threads, 20)
  assert.equal(result.deviceMemoryGiB, null)
  assert.match(result.memory, /unavailable/)
  assert.match(result.memoryNote, /does not expose physical RAM/)
  assert.equal(result.crossOriginIsolated, true)
})

test('Chromium-like browser labels device memory as an approximate class', () => {
  const result = detectBrowserResources({ platform: 'Linux', hardwareConcurrency: 12, deviceMemory: 8 })
  assert.equal(result.threads, 12)
  assert.equal(result.deviceMemoryGiB, 8)
  assert.match(result.memory, /≈8 GiB RAM class \(approximate\)/)
  assert.match(result.memoryNote, /rounded and may be capped/)
  assert.notEqual(result.memory, '8 GiB available')
})

test('application memory reports only its supported measurement, not host RAM', async () => {
  const mem = await measureAppMemory({ measureUserAgentSpecificMemory: async () => ({ bytes: 210000000 }) }, true)
  assert.equal(mem.kind, 'app')
  assert.equal(mem.bytes, 210000000)
  const firefox = await measureAppMemory({}, true)
  assert.equal(firefox.available, false)
  assert.equal(firefox.kind, 'unavailable')
  assert.equal(Object.hasOwn(normalizeResources({ physicalRamGiB: 32 }), 'physicalRamGiB'), false)
})

test('UX8 has no native monitor download, token, localhost connection or native CI job', async () => {
  assert.match(main, /setTimeout\(\(\) => \{ void refreshAutomaticAppMemory\(\) \}, 2500\)/)
  assert.match(main, /setInterval\(\(\) => \{ void refreshAutomaticAppMemory\(\) \}, 120000\)/)
  assert.match(main, /Scientific WASM memory/)
  assert.match(main, /No installation or manual RAM entry is required/)
  assert.doesNotMatch(main, /nativeMonitor|connect-native-monitor|disconnect-native-monitor|native-monitor-token|resource-ram/)
  assert.doesNotMatch(main, /127\.0\.0\.1|http:\/\//)
  assert.doesNotMatch(ci, /native-system-monitor/)
  assert.doesNotMatch(pages, /system-monitor\.js/)
  assert.match(html, /m1-merge/)
  await assert.rejects(access(new URL('../system-monitor.js', import.meta.url)), /ENOENT/)
  await assert.rejects(access(new URL('../../native/system-monitor/Cargo.toml', import.meta.url)), /ENOENT/)
})
