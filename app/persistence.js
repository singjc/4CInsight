const DB_NAME = '4c-insight-local'
const DB_VERSION = 1
const HISTORY_STORE = 'history'
const SNAPSHOT_STORE = 'history-snapshots'

function requestResult(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error || new Error('IndexedDB request failed'))
  })
}

function transactionDone(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve()
    transaction.onabort = () => reject(transaction.error || new Error('IndexedDB transaction aborted'))
    transaction.onerror = () => reject(transaction.error || new Error('IndexedDB transaction failed'))
  })
}

function openDatabase() {
  if (!('indexedDB' in globalThis)) return Promise.reject(new Error('Browser history storage is unavailable.'))
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION)
    request.onupgradeneeded = () => {
      const database = request.result
      if (!database.objectStoreNames.contains(HISTORY_STORE)) {
        const history = database.createObjectStore(HISTORY_STORE, { keyPath: 'id' })
        history.createIndex('createdAt', 'createdAt')
      }
      if (!database.objectStoreNames.contains(SNAPSHOT_STORE)) {
        database.createObjectStore(SNAPSHOT_STORE, { keyPath: 'id' })
      }
    }
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(request.error || new Error('Could not open browser history storage.'))
  })
}

export async function saveHistoryEntry(entry, snapshot) {
  const database = await openDatabase()
  try {
    const transaction = database.transaction([HISTORY_STORE, SNAPSHOT_STORE], 'readwrite')
    transaction.objectStore(HISTORY_STORE).put(entry)
    transaction.objectStore(SNAPSHOT_STORE).put({ id: entry.id, snapshot })
    await transactionDone(transaction)
  } finally {
    database.close()
  }
}

export async function listHistoryEntries() {
  const database = await openDatabase()
  try {
    const transaction = database.transaction(HISTORY_STORE, 'readonly')
    const entries = await requestResult(transaction.objectStore(HISTORY_STORE).getAll())
    await transactionDone(transaction)
    return entries.sort((left, right) => String(right.createdAt).localeCompare(String(left.createdAt)))
  } finally {
    database.close()
  }
}

export async function loadHistorySnapshot(id) {
  const database = await openDatabase()
  try {
    const transaction = database.transaction(SNAPSHOT_STORE, 'readonly')
    const record = await requestResult(transaction.objectStore(SNAPSHOT_STORE).get(id))
    await transactionDone(transaction)
    return record?.snapshot || null
  } finally {
    database.close()
  }
}

export async function deleteHistoryEntry(id) {
  const database = await openDatabase()
  try {
    const transaction = database.transaction([HISTORY_STORE, SNAPSHOT_STORE], 'readwrite')
    transaction.objectStore(HISTORY_STORE).delete(id)
    transaction.objectStore(SNAPSHOT_STORE).delete(id)
    await transactionDone(transaction)
  } finally {
    database.close()
  }
}

export async function clearHistoryEntries() {
  const database = await openDatabase()
  try {
    const transaction = database.transaction([HISTORY_STORE, SNAPSHOT_STORE], 'readwrite')
    transaction.objectStore(HISTORY_STORE).clear()
    transaction.objectStore(SNAPSHOT_STORE).clear()
    await transactionDone(transaction)
  } finally {
    database.close()
  }
}

export async function storageEstimate() {
  if (!navigator.storage?.estimate) return null
  const estimate = await navigator.storage.estimate()
  return {
    usage: Number(estimate.usage || 0),
    quota: Number(estimate.quota || 0),
  }
}
