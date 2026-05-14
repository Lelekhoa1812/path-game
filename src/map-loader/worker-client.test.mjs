import assert from 'node:assert/strict'
import { MapLoaderClient } from './worker-client.js'

class FakeWorker {
  constructor() {
    this.messages = []
    this.onmessage = null
    this.onerror = null
    this.onmessageerror = null
    this.terminated = false
  }

  postMessage(message) {
    this.messages.push(message)
  }

  emit(message) {
    this.onmessage({ data: message })
  }

  emitError(message = 'worker failed') {
    this.onerror({ message })
  }

  emitMessageError(message = 'worker message failed') {
    this.onmessageerror({ message })
  }

  terminate() {
    this.terminated = true
  }
}

function puzzle({
  rows = 9,
  cols = rows,
  difficulty = 'Easy',
  waypoints = [],
  solution = [],
} = {}) {
  return {
    R: rows,
    C: cols,
    obstacles: [],
    waypoints,
    solution,
    difficulty,
  }
}

function emitResult(worker, jobId, puzzleOptions) {
  worker.emit({ type: 'result', jobId, puzzle: puzzle(puzzleOptions) })
}

const worker = new FakeWorker()
const client = new MapLoaderClient(() => worker)
const progress = []
const resultPromise = client.generate({
  seed: 1,
  onProgress: (event) => progress.push(event.stage),
})
const jobId = worker.messages[0].jobId

worker.emit({ type: 'progress', jobId: jobId + 100, stage: 'Stale progress' })
worker.emit({ type: 'progress', jobId, stage: 'Building map' })
emitResult(worker, jobId, {
  waypoints: [{ step: 1, pos: [0, 0] }],
  solution: [[0, 0]],
})
emitResult(worker, jobId, { rows: 11, difficulty: 'Hard' })

const result = await resultPromise
assert.deepEqual(progress, ['Building map'])
assert.equal(result.R, 9)
assert.equal(result.C, 9)
assert.equal(result.obstacles instanceof Uint8Array, true)
assert.deepEqual(result.waypoints[0].pos, [0, 0])

const cancelWorker = new FakeWorker()
const cancelClient = new MapLoaderClient(() => cancelWorker)
const cancelPromise = cancelClient.generate({ seed: 2 })
const cancelJobId = cancelWorker.messages[0].jobId
cancelClient.cancel()

assert.equal(cancelWorker.terminated, false)
assert.deepEqual(cancelWorker.messages[1], { type: 'cancel', jobId: cancelJobId })
cancelWorker.emit({ type: 'cancelled', jobId: cancelJobId })
await assert.rejects(cancelPromise, /Generation cancelled/)

emitResult(cancelWorker, cancelJobId, { rows: 11, difficulty: 'Hard' })

const staleWorker = new FakeWorker()
const staleClient = new MapLoaderClient(() => staleWorker)
const staleResult = staleClient.generate({ seed: 1 }).catch((error) => error.message)
const staleJobId = staleWorker.messages[0].jobId
staleClient.cancel()
assert.deepEqual(staleWorker.messages[1], { type: 'cancel', jobId: staleJobId })
emitResult(staleWorker, staleJobId)
assert.equal(await staleResult, 'Generation cancelled')

const errorWorker = new FakeWorker()
const errorClient = new MapLoaderClient(() => errorWorker)
const errorPromise = errorClient.generate({ seed: 3 })
errorWorker.emit({ type: 'error', jobId: errorWorker.messages[0].jobId, message: 'boom' })
await assert.rejects(errorPromise, /boom/)

const thrownWorker = new FakeWorker()
const thrownClient = new MapLoaderClient(() => thrownWorker)
const thrownPromise = thrownClient.generate({ seed: 4 })
thrownWorker.emitError('module import failed')
await assert.rejects(thrownPromise, /module import failed/)
assert.equal(thrownWorker.terminated, true)

const messageErrorWorker = new FakeWorker()
const messageErrorClient = new MapLoaderClient(() => messageErrorWorker)
const messageErrorPromise = messageErrorClient.generate({ seed: 5 })
messageErrorWorker.emitMessageError('message clone failed')
await assert.rejects(messageErrorPromise, /message clone failed/)
assert.equal(messageErrorWorker.terminated, true)

const freshWorkers = []
const freshClient = new MapLoaderClient(() => {
  const freshWorker = new FakeWorker()
  freshWorkers.push(freshWorker)
  return freshWorker
})
const firstFreshPromise = freshClient.generate({ seed: 6 })
const firstFreshJobId = freshWorkers[0].messages[0].jobId
freshClient.cancel()
assert.deepEqual(freshWorkers[0].messages[1], { type: 'cancel', jobId: firstFreshJobId })
await assert.rejects(firstFreshPromise, /Generation cancelled/)

const secondFreshPromise = freshClient.generate({ seed: 7 })
assert.equal(freshWorkers.length, 2)
assert.equal(freshWorkers[0].terminated, true)
assert.equal(freshWorkers[1].terminated, false)
emitResult(freshWorkers[1], freshWorkers[1].messages[0].jobId, { rows: 7 })
assert.equal((await secondFreshPromise).R, 7)

client.terminate()
assert.equal(worker.terminated, true)
