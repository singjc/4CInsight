import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { createSystemMonitor, MONITOR_URL, validateNativeMetrics } from '../system-monitor.js'

const sample = () => ({
  schema: 1, source: '4c-native-sysinfo', timestamp_ms: Date.now(),
  total_memory_bytes: 34359738368, available_memory_bytes: 21474836480,
  used_memory_bytes: 12884901888, total_swap_bytes: 1073741824,
  used_swap_bytes: 0, logical_cpus: 20, cpu_usage_percent: 8.2,
})
const TOKEN = 'a'.repeat(32)

test('native metrics schema accepts good host metrics, rejects fake and stale readings', () => {
  assert.equal(validateNativeMetrics(sample()).logical_cpus, 20)
  assert.throws(() => validateNativeMetrics({ ...sample(), source: 'unknown' }), /Unexpected/)
  assert.throws(() => validateNativeMetrics({ ...sample(), available_memory_bytes: 1e20 }), /inconsistent/)
  assert.throws(() => validateNativeMetrics({ ...sample(), timestamp_ms: 100 }), /stale/)
  assert.throws(() => validateNativeMetrics({ ...sample(), used_swap_bytes: 'a' }), /Invalid/)
})

test('native helper is opt-in, never polls automatically or stores token', async () => {
  const requests = []
  const client = createSystemMonitor({ fetchImpl: async (url, options) => {
    requests.push({ url, options })
    return { ok: true, json: async () => sample() }
  } })
  assert.deepEqual(requests, [])
  assert.equal(client.getState().status, 'disconnected')
  assert.equal((await client.connect(TOKEN)).status, 'connected')
  assert.equal(requests[0].url, MONITOR_URL)
  assert.equal(requests[0].options.headers.Authorization, `Bearer ${TOKEN}`)
  assert.equal(requests[0].options.credentials, 'omit')
  assert.equal(requests[0].options.cache, 'no-store')
  assert.equal((await client.refresh()).status, 'connected')
  assert.equal(requests.length, 2)
  assert.equal(client.disconnect().last, null)
  assert.equal((await client.refresh()).status, 'disconnected')
  assert.equal(requests.length, 2)
  assert.equal(JSON.stringify(client.getState()).includes(TOKEN), false)
})

test('wrong format and unauthorized token show errors without fetching stale data', async () => {
  let count = 0
  const client = createSystemMonitor({ fetchImpl: async () => { count++; return { ok: false, status: 401 } } })
  assert.equal((await client.connect('invalid')).status, 'disconnected')
  assert.match(client.getState().error, /32-character/)
  assert.equal(count, 0)
  assert.equal((await client.connect(TOKEN)).status, 'disconnected')
  assert.match(client.getState().error, /Incorrect monitor/)
  assert.equal(count, 1)
})

test('helper failure clears old readings, and disconnect prevents late fetch response', async () => {
  let fail = false
  const client = createSystemMonitor({ fetchImpl: async () => {
    if (fail) throw new TypeError('Failed to fetch')
    return { ok: true, json: async () => sample() }
  } })
  await client.connect(TOKEN)
  fail = true
  const state = await client.refresh()
  assert.equal(state.status, 'unavailable')
  assert.equal(state.last, null)
  assert.match(state.error, /Failed to fetch/)

  let resume
  const pending = createSystemMonitor({ fetchImpl: () => new Promise(resolve => { resume = resolve }) })
  const connecting = pending.connect(TOKEN)
  pending.disconnect()
  resume({ ok: true, json: async () => sample() })
  assert.equal((await connecting).status, 'disconnected')
})

test('native helper binds IPv4 loopback, validates allowed Origin and uses a session token', async () => {
  const source = await readFile(new URL('../../native/system-monitor/src/main.rs', import.meta.url), 'utf8')
  assert.match(source, /TcpListener::bind\(\("127\.0\.0\.1", PORT\)\)/)
  assert.match(source, /headers\.get\("origin"\)/)
  assert.match(source, /headers\.get\("authorization"\)/)
  assert.match(source, /Uuid::new_v4\(\)/)
  assert.match(source, /system\.available_memory\(\)/)
  assert.match(source, /Access-Control-Allow-Origin/)
  assert.doesNotMatch(source, /Access-Control-Allow-Origin: \*/)
})

test('UX7 browser UI keeps native monitor optional and token outside persistence', async () => {
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')
  assert.match(main, /nativeMonitor\.connect\(token\)/)
  assert.match(main, /id="native-monitor-token" type="password"/)
  assert.match(main, /nativeMonitor\.disconnect\(\)/)
  assert.match(main, /setInterval\(\(\) => void pollNativeMonitor\(\), 3000\)/)
  assert.match(html, /ux7-native-monitor/)
  assert.doesNotMatch(main, /localStorage\.setItem\([^\n]*token/i)
})
