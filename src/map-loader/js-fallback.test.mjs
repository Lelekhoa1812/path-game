import assert from 'node:assert/strict'
import { generateWithJsFallback } from './js-fallback.js'

const retryCalls = []
const retriedPuzzle = await generateWithJsFallback(
  (attempt, options) => {
    retryCalls.push({ attempt, options })
    return attempt === 2 ? { R: options.targetSize, C: options.targetSize } : null
  },
  () => {
    throw new Error('guaranteed fallback should not run after a retry succeeds')
  },
  () => {},
  { targetSize: 10 },
)

assert.equal(retriedPuzzle.R, 10)
assert.equal(retriedPuzzle.C, 10)
assert.deepEqual(
  retryCalls.map((call) => [call.attempt, call.options.targetSize, call.options.sizes]),
  [
    [0, 10, [10]],
    [1, 10, [10]],
    [2, 10, [10]],
  ],
)

let loggedFallback = false
const guaranteedPuzzle = await generateWithJsFallback(
  () => null,
  (options) => ({ R: options.targetSize, C: options.targetSize }),
  () => {
    loggedFallback = true
  },
  { targetSize: 11 },
)

assert.equal(guaranteedPuzzle.R, 11)
assert.equal(guaranteedPuzzle.C, 11)
assert.equal(loggedFallback, true)
