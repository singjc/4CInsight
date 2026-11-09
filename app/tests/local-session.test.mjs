import test from 'node:test'
import assert from 'node:assert/strict'
import {
  readLocalSession, saveLocalSession, readLocalResults, saveLocalResult,
  listStoredSourceIds, readStoredSources, saveStoredSource, removeStoredSources,
  clearLocalSession, recoverSourceFile, validLocalSession,
} from '../local-session.js'

// A small asynchronous IndexedDB contract fixture. No browser or external service.
function fakeIndexedDB() {
  const databases = new Map()
  return {
    open(name) {
      const request = {}
      queueMicrotask(() => {
        let db = databases.get(name)
        const isNew = !db
        if (!db) {
          const stores = new Map()
          db = {
            objectStoreNames: { contains: key => stores.has(key) },
            createObjectStore(key) { stores.set(key, new Map()); return {} },
            transaction(names) {
              const tx = { oncomplete: null, onabort: null, onerror: null, error: null, open: 0, everRequested: false }
              tx.objectStore = key => {
                assert.ok(stores.has(key), key)
                const map = stores.get(key)
                const exec = (act) => {
                  const req = { result: undefined, error: null }
                  tx.everRequested = true
                  tx.open++
                  queueMicrotask(() => {
                    try { req.result = act(); req.onsuccess?.() }
                    catch (e) { req.error = e; req.onerror?.() }
                    tx.open--
                    setTimeout(() => { if (tx.open === 0) tx.oncomplete?.() }, 0)
                  })
                  return req
                }
                return {
                  put: value => exec(() => { map.set(value.id, structuredClone(value)); return value.id }),
                  get: id => exec(() => structuredClone(map.get(id))),
                  getAll: () => exec(() => [...map.values()].map(v => structuredClone(v))),
                  getAllKeys: () => exec(() => [...map.keys()]),
                  delete: id => exec(() => { map.delete(id) }),
                  clear: () => exec(() => { map.clear() }),
                }
              }
              return tx
            },
            close() {},
          }
          databases.set(name, db)
        }
        request.result = db
        if (isNew) request.onupgradeneeded?.()
        request.onsuccess?.()
      })
      return request
    },
  }
}
const payload = {
  product: '4C Insight', kind: 'application-state', schemaVersion: 1,
  project: { files: [{ name: 'S1.xlsx', relativePath: 'BCR/S1.xlsx', size: 500 }] },
  configs: { compare: { groupColumn: 'status' } },
}

test('local workspace, versioned result and browser source file survive a new read', async () => {
  const old = globalThis.indexedDB
  globalThis.indexedDB = fakeIndexedDB()
  try {
    const entry = { format: '4c-insight-active-workspace-v1', payload, resultTokens: { i2Result: 'rev-1' } }
    await saveLocalResult('i2Result', 'rev-1', { distances: [0, 1, 2], cohort: ['S1', 'S2'] })
    await saveLocalSession(entry)
    const file = { name: 'S1.xlsx', size: 500, lastModified: 170000, webkitRelativePath: '' }
    await saveStoredSource('id-1', file, 'BCR/S1.xlsx')
    const restored = await readLocalSession()
    assert.equal(validLocalSession(restored), true)
    assert.equal(restored.payload.configs.compare.groupColumn, 'status')
    assert.deepEqual(await readLocalResults(restored.resultTokens), { i2Result: { distances: [0, 1, 2], cohort: ['S1', 'S2'] } })
    assert.deepEqual(await listStoredSourceIds(), ['id-1'])
    assert.equal((await readStoredSources())[0].relativePath, 'BCR/S1.xlsx')
    assert.equal(await readLocalResults({ i2Result: 'older-rev' }).then(v => Object.keys(v).length), 0, 'Do not load a previous result revision')
    await removeStoredSources(['id-1'])
    assert.deepEqual(await listStoredSourceIds(), [])
  } finally { globalThis.indexedDB = old }
})

test('Clear state deletes the active workspace, source cache and results, but not the separate History database', async () => {
  const old = globalThis.indexedDB
  globalThis.indexedDB = fakeIndexedDB()
  try {
    await saveLocalSession({ format: '4c-insight-active-workspace-v1', payload, resultTokens: {} })
    await saveLocalResult('qcResult', 'a', { ready: true })
    await saveStoredSource('S1', { name: 'S1.xlsx', size: 5, lastModified: 1 }, 'S1.xlsx')
    await clearLocalSession()
    assert.equal(await readLocalSession(), undefined)
    assert.deepEqual(await readStoredSources(), [])
    assert.deepEqual(await readLocalResults({ qcResult: 'a' }), {})
  } finally { globalThis.indexedDB = old }
})

test('refuse an unknown or malformed state and never guess a different source identity', () => {
  assert.equal(validLocalSession({ format: 'bad', payload, resultTokens: {} }), false)
  assert.equal(validLocalSession({ id: 'current', format: '4c-insight-active-workspace-v1', payload, resultTokens: {} }), true)
  assert.equal(recoverSourceFile({ id: 'different', relativePath: 'BCR/S1.xlsx', file: { name: 'S1.xlsx' } }, () => 'actual'), null)
  const exact = recoverSourceFile({ id: 'actual', relativePath: 'BCR/S1.xlsx', file: { name: 'S1.xlsx' } }, f => f.webkitRelativePath === 'BCR/S1.xlsx' ? 'actual' : 'wrong')
  assert.equal(exact[0], 'actual')
})

test('session persistence performs no network access and does not impose a fixed size cap', async () => {
  const { readFile } = await import('node:fs/promises')
  const source = await readFile(new URL('../local-session.js', import.meta.url), 'utf8')
  assert.doesNotMatch(source, /\bfetch\s*\(|XMLHttpRequest|WebSocket|\/api\//)
  assert.doesNotMatch(source, /maxFileSize|MAX_FILE|size\s*>\s*[0-9]/i)
  assert.match(source, /indexedDB\.open/)
  assert.match(source, /source-files/)
})
