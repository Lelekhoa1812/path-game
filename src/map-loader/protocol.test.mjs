import assert from 'node:assert/strict'
import {
  createGenerateRequest,
  isCurrentJob,
  normalizePuzzlePayload,
  serializeMetric,
} from './protocol.js'

const request = createGenerateRequest({ seed: 1234, sizes: [9, 10, 11] })
assert.equal(request.type, 'generate')
assert.equal(request.targetMs, 5000)
assert.equal(request.maxMs, 10000)
assert.deepEqual(request.sizes, [9, 10, 11])
assert.equal(Number.isInteger(request.jobId), true)

const defaultRequest = createGenerateRequest({ seed: 4321 })
assert.deepEqual(defaultRequest.sizes, [8, 9, 10, 11])

assert.equal(isCurrentJob({ jobId: request.jobId }, request.jobId), true)
assert.equal(isCurrentJob({ jobId: request.jobId + 1 }, request.jobId), false)
assert.equal(isCurrentJob(null, request.jobId), false)
assert.equal(isCurrentJob(undefined, request.jobId), false)

const puzzle = normalizePuzzlePayload({
  R: 9,
  C: 9,
  obstacles: [0, 1, 0],
  waypoints: [{ step: 1, pos: [0, 0] }],
  solution: [[0, 0], [0, 1]],
  difficulty: 'Medium',
})
assert.equal(puzzle.R, 9)
assert.equal(puzzle.C, 9)
assert.equal(puzzle.obstacles instanceof Uint8Array, true)
assert.equal(puzzle.waypoints[0].step, 1)
assert.equal(puzzle.metrics, null)

const puzzleWithMetrics = normalizePuzzlePayload({
  R: 11,
  C: 11,
  obstacles: [0, 1, 0],
  waypoints: [{ step: 1, pos: [0, 0] }],
  solution: [[0, 0], [0, 1]],
  difficulty: 'Hard',
  metrics: {
    status: 'success',
    seed: 42,
    size: 11,
    quality: 'balanced',
    quality_score: 0.75,
    phase_timings: {
      candidate_ms: 12.3,
      quality_ms: 1.5,
      total_ms: 13.8,
    },
    total_ms: 13.8,
    target_ms: 5000,
    max_ms: 10000,
    degradation_level: 1,
    candidate_attempts: 4,
    solver_calls: 6,
    unique_checks: 3,
    cancelled: false,
    fallback: true,
  },
})
assert.deepEqual(puzzleWithMetrics.metrics, {
  status: 'success',
  seed: 42,
  size: 11,
  quality: 'balanced',
  quality_score: 0.75,
  qualityScore: 0.75,
  phase_timings: {
    candidate_ms: 12.3,
    quality_ms: 1.5,
    total_ms: 13.8,
  },
  phaseTimings: {
    candidate_ms: 12.3,
    quality_ms: 1.5,
    total_ms: 13.8,
    candidateMs: 12.3,
    qualityMs: 1.5,
    totalMs: 13.8,
  },
  total_ms: 13.8,
  totalMs: 13.8,
  target_ms: 5000,
  targetMs: 5000,
  max_ms: 10000,
  maxMs: 10000,
  degradation_level: 1,
  degradationLevel: 1,
  candidate_attempts: 4,
  candidateAttempts: 4,
  solver_calls: 6,
  solverCalls: 6,
  unique_checks: 3,
  uniqueChecks: 3,
  cancelled: false,
  fallback: true,
})

const puzzleWithCamelMetrics = normalizePuzzlePayload({
  R: 9,
  C: 9,
  obstacles: [0],
  waypoints: [{ step: 1, pos: [0, 0] }],
  solution: [[0, 0]],
  difficulty: 'Medium',
  metrics: {
    seed: 7,
    size: 9,
    qualityScore: 1,
    phaseTimings: { candidateMs: 1, qualityMs: 2, totalMs: 3 },
    totalMs: 3,
    targetMs: 5000,
    maxMs: 10000,
    degradationLevel: 0,
    candidateAttempts: 1,
    solverCalls: 1,
    uniqueChecks: 1,
    cancelled: false,
    fallback: false,
  },
})
assert.equal(puzzleWithCamelMetrics.metrics.qualityScore, 1)
assert.equal(puzzleWithCamelMetrics.metrics.phaseTimings.candidateMs, 1)
assert.equal(puzzleWithCamelMetrics.metrics.targetMs, 5000)

const metric = serializeMetric('generate:end', {
  event: 'override',
  totalMs: 123.456,
  size: 9,
  status: 'success',
})
assert.deepEqual(metric, {
  event: 'generate:end',
  totalMs: 123.46,
  size: 9,
  status: 'success',
})
