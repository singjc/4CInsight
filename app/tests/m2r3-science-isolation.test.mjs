import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { isolatedScienceRequest } from '../isolated-science.js'

class FakeWorker {
  constructor() { this.listeners = new Map(); this.terminated = false; this.lastRequest = null }
  addEventListener(type, fn) { this.listeners.set(type, fn) }
  removeEventListener(type) { this.listeners.delete(type) }
  emit(type, data) { this.listeners.get(type)?.({ data, message: data?.message, preventDefault() {} }) }
  postMessage(request) { this.lastRequest = request; this.onRequest?.(request) }
  terminate() { this.terminated = true }
}

function workerFactory(onRequest) {
  const workers = []
  return { workers, createWorker() {
    const worker = new FakeWorker()
    worker.onRequest = request => onRequest(worker, request)
    workers.push(worker)
    return worker
  }}
}

test('an isolated science request receives progress and result and releases its worker', async () => {
  const progress = [], memory = []
  const factory = workerFactory((worker, request) => queueMicrotask(() => {
    worker.emit('message', { type: 'analysis-progress', requestId: request.requestId, message: 'BCR: parsing 1/2: S1.xlsx' })
    worker.emit('message', { type: 'wasm-memory', requestId: request.requestId, allocatedBytes: 1024 })
    worker.emit('message', { type: 'expression-comparison-result', requestId: request.requestId, result: { bcr: { analysis: { files_used: 2 } } } })
  }))
  const result = await isolatedScienceRequest({ ...factory, type: 'run-expression-comparison', requestId: 'test-1', onProgress: v => progress.push(v), onMemory: v => memory.push(v) })
  assert.equal(result.result.bcr.analysis.files_used, 2)
  assert.equal(progress.length, 1)
  assert.equal(memory.length, 1)
  assert.equal(factory.workers[0].terminated, true)
})

test('WASM trap names the last modality file, and another worker can still run', async () => {
  const failure = workerFactory((worker, request) => queueMicrotask(() => {
    worker.emit('message', { type: 'analysis-progress', requestId: request.requestId, message: 'TCR beta: parsing 26/265: S026.xlsx' })
    worker.emit('message', { type: 'worker-error', requestId: request.requestId, message: 'unreachable executed' })
  }))
  await assert.rejects(isolatedScienceRequest({ createWorker: failure.createWorker, type: 'run-expression-comparison', requestId: 'trap' }), /TCR beta.*S026\.xlsx.*unreachable executed/)
  assert.equal(failure.workers[0].terminated, true)
  const success = workerFactory((worker, request) => queueMicrotask(() => {
    worker.emit('message', { type: 'expression-comparison-result', requestId: request.requestId, result: { immune: { analysis: {} } } })
  }))
  assert.ok((await isolatedScienceRequest({ createWorker: success.createWorker, type: 'run-expression-comparison', requestId: 'next' })).result.immune)
  assert.equal(success.workers[0].terminated, true)
})

test('an abort terminates the active worker and rejects without starting more work', async () => {
  const controller = new AbortController()
  const factory = workerFactory(() => {})
  const pending = isolatedScienceRequest({ createWorker: factory.createWorker, signal: controller.signal, requestId: 'stopped', type: 'run-expression-comparison' })
  controller.abort()
  await assert.rejects(pending, /Operation stopped by user/)
  assert.equal(factory.workers[0].terminated, true)
  await assert.rejects(isolatedScienceRequest({ createWorker: factory.createWorker, signal: controller.signal, requestId: 'stopped-again', type: 'run-expression-comparison' }), /Analysis stopped/)
  assert.equal(factory.workers.length, 1)
})

test('worker exception is reported and does not leak the worker', async () => {
  const factory = workerFactory(worker => queueMicrotask(() => worker.emit('error', { message: 'RuntimeError: unreachable executed' })))
  await assert.rejects(isolatedScienceRequest({ createWorker: factory.createWorker, requestId: 'exception', type: 'run-expression-comparison' }), /RuntimeError: unreachable executed/)
  assert.equal(factory.workers[0].terminated, true)
})

test('Compare Groups runs modalities one at a time and retains successful results', async () => {
  const main = await readFile(new URL('../main.js', import.meta.url), 'utf8')
  assert.match(main, /for \(const step of steps\)/)
  assert.match(main, /await isolatedScienceRequest\(/)
  assert.match(main, /result\[step\.key\] = response\.result\[step\.key\]/)
  assert.match(main, /result\.omicsErrors\[step\.kind\] = message/)
  assert.match(main, /activeCompareController\.abort\(\)/)
  assert.match(main, /function updateCompareProgress\(/)
})
