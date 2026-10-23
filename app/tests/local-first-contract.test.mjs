import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const clientFiles = [
  new URL('../main.js', import.meta.url),
  new URL('../analysis-worker.js', import.meta.url),
  new URL('../save.js', import.meta.url),
]

test('primary browser client has no analysis API endpoint', async () => {
  for (const url of clientFiles) {
    const source = await readFile(url, 'utf8')
    assert.equal(source.includes('/api/'), false, `${url.pathname} contains an /api/ dependency`)
    assert.equal(source.includes('recovery_token'), false, `${url.pathname} contains legacy project credentials`)
    assert.equal(source.includes('cfs-server'), false, `${url.pathname} contains the retired server name`)
  }
})

test('static entry point is branded 4C Insight', async () => {
  const html = await readFile(new URL('../index.html', import.meta.url), 'utf8')
  assert.match(html, /<title>4C Insight<\/title>/)
  assert.doesNotMatch(html, /Cohort Explorer|cfs-expression-merger/i)
})
