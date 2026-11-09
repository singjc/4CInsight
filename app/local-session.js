/**
 * Browser-only active workspace persistence. Separate from durable History.
 * Source File objects are stored as blobs by the browser, not read into JS.
 * No network access, third-party storage or fixed size cap is used.
 */
const DB_NAME = '4c-insight-active-workspace'
const DB_VERSION = 1
const SESSION = 'workspace'
const RESULTS = 'results'
const SOURCES = 'source-files'

function finished(tx) {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve()
    tx.onabort = () => reject(tx.error || new Error('Local session transaction aborted'))
    tx.onerror = () => reject(tx.error || new Error('Local session transaction failed'))
  })
}
function returned(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error || new Error('Local session request failed'))
  })
}
function database() {
  if (!globalThis.indexedDB) return Promise.reject(new Error('Browser-local session storage is unavailable'))
  return new Promise((resolve, reject) => {
    const request = globalThis.indexedDB.open(DB_NAME, DB_VERSION)
    request.onupgradeneeded = () => {
      const db = request.result
      if (!db.objectStoreNames.contains(SESSION)) db.createObjectStore(SESSION, { keyPath: 'id' })
      if (!db.objectStoreNames.contains(RESULTS)) db.createObjectStore(RESULTS, { keyPath: 'id' })
      if (!db.objectStoreNames.contains(SOURCES)) db.createObjectStore(SOURCES, { keyPath: 'id' })
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error || new Error('Cannot open browser-local session storage'))
    request.onblocked = () => reject(new Error('Close other 4C Insight tabs before updating local session storage'))
  })
}
async function transact(store, mode, action) {
  const db = await database()
  try {
    const tx = db.transaction(store, mode)
    const completed = finished(tx) // Install handlers before awaiting any request.
    const value = await action(tx.objectStore(store))
    await completed
    return value
  } finally { db.close() }
}

export async function saveLocalSession(snapshot) {
  return transact(SESSION, 'readwrite', store => { store.put({ ...snapshot, id: 'current' }) })
}
export async function readLocalSession() {
  return transact(SESSION, 'readonly', store => returned(store.get('current')))
}
export async function saveLocalResult(id, token, value) {
  if (!id || typeof token !== 'string') throw new Error('Invalid result storage key')
  return transact(RESULTS, 'readwrite', store => { store.put({ id, token, value }) })
}
export async function readLocalResults(tokens) {
  if (!tokens || typeof tokens !== 'object') return {}
  const ids = Object.keys(tokens).filter(key => typeof tokens[key] === 'string')
  if (!ids.length) return {}
  const db = await database()
  try {
    const tx = db.transaction(RESULTS, 'readonly')
    const completed = finished(tx)
    const pending = ids.map(async id => [id, await returned(tx.objectStore(RESULTS).get(id))])
    const rows = await Promise.all(pending)
    await completed
    return Object.fromEntries(rows.filter(([id, entry]) => entry?.token === tokens[id]).map(([id, entry]) => [id, entry.value]))
  } finally { db.close() }
}
export async function readStoredSources() {
  return transact(SOURCES, 'readonly', store => returned(store.getAll()))
}
export async function listStoredSourceIds() {
  return transact(SOURCES, 'readonly', store => returned(store.getAllKeys()))
}
export async function saveStoredSource(id, file, relativePath) {
  if (typeof id !== 'string' || !id || !file || typeof file.name !== 'string') throw new Error('Invalid local source file')
  return transact(SOURCES, 'readwrite', store => { store.put({ id, file, relativePath: String(relativePath || file.name) }) })
}
export async function removeStoredSources(ids) {
  if (!ids?.length) return
  return transact(SOURCES, 'readwrite', store => { for (const id of ids) store.delete(id) })
}
export async function clearLocalSession() {
  const db = await database()
  try {
    const tx = db.transaction([SESSION, RESULTS, SOURCES], 'readwrite')
    const completed = finished(tx)
    tx.objectStore(SESSION).clear()
    tx.objectStore(RESULTS).clear()
    tx.objectStore(SOURCES).clear()
    await completed
  } finally { db.close() }
}

/** Restore an exact file identity; never silently assign a different path. */
export function recoverSourceFile(record, signature) {
  if (!record || !record.file || !record.id || typeof signature !== 'function') return null
  const file = record.file
  try { Object.defineProperty(file, 'webkitRelativePath', { configurable: true, value: record.relativePath || file.name }) }
  catch { /* Some browser File objects cannot be extended. */ }
  return signature(file) === record.id ? [record.id, file] : null
}
export function validLocalSession(session) {
  return !!(session && session.id === 'current' && session.format === '4c-insight-active-workspace-v1' && session.payload?.product === '4C Insight' && session.payload?.kind === 'application-state' && session.payload?.schemaVersion === 1 && Array.isArray(session.payload?.project?.files) && session.resultTokens && typeof session.resultTokens === 'object')
}
