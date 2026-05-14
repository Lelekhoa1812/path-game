let nextJobId = 1

const DEFAULT_TARGET_MS = 5000
const DEFAULT_MAX_MS = 10000
const DEFAULT_SIZES = [8, 9, 10, 11]

export function createGenerateRequest(options = {}) {
  return {
    type: 'generate',
    jobId: nextJobId++,
    seed: Number.isInteger(options.seed) ? options.seed : Date.now(),
    targetMs: Number.isFinite(options.targetMs) ? options.targetMs : DEFAULT_TARGET_MS,
    maxMs: Number.isFinite(options.maxMs) ? options.maxMs : DEFAULT_MAX_MS,
    sizes: Array.isArray(options.sizes) && options.sizes.length ? options.sizes.slice() : DEFAULT_SIZES.slice(),
    quality: options.quality || 'balanced',
  }
}

export function isCurrentJob(message, activeJobId) {
  return !!(message && message.jobId === activeJobId)
}

export function normalizePuzzlePayload(puzzle) {
  return {
    R: puzzle.R,
    C: puzzle.C,
    obstacles: puzzle.obstacles instanceof Uint8Array ? puzzle.obstacles : new Uint8Array(puzzle.obstacles),
    waypoints: puzzle.waypoints.map((wp) => ({ step: wp.step, pos: [wp.pos[0], wp.pos[1]] })),
    solution: puzzle.solution.map((cell) => [cell[0], cell[1]]),
    difficulty: puzzle.difficulty,
    metrics: normalizeMetrics(puzzle.metrics),
  }
}

function normalizeMetrics(metrics) {
  if (!metrics) {
    return null
  }

  const phaseTimings = metrics.phaseTimings || metrics.phase_timings || {}
  const value = (camelKey, snakeKey) => metrics[camelKey] ?? metrics[snakeKey]
  const phaseValue = (camelKey, snakeKey) => phaseTimings[camelKey] ?? phaseTimings[snakeKey]

  return {
    ...metrics,
    seed: metrics.seed,
    size: metrics.size,
    quality: metrics.quality,
    qualityScore: value('qualityScore', 'quality_score'),
    phaseTimings: {
      ...phaseTimings,
      candidateMs: phaseValue('candidateMs', 'candidate_ms'),
      qualityMs: phaseValue('qualityMs', 'quality_ms'),
      totalMs: phaseValue('totalMs', 'total_ms'),
    },
    totalMs: value('totalMs', 'total_ms'),
    targetMs: value('targetMs', 'target_ms'),
    maxMs: value('maxMs', 'max_ms'),
    degradationLevel: value('degradationLevel', 'degradation_level'),
    candidateAttempts: value('candidateAttempts', 'candidate_attempts'),
    solverCalls: value('solverCalls', 'solver_calls'),
    uniqueChecks: value('uniqueChecks', 'unique_checks'),
    cancelled: metrics.cancelled,
    fallback: metrics.fallback,
  }
}

export function serializeMetric(event, details = {}) {
  const metric = { ...details, event }
  for (const [key, value] of Object.entries(metric)) {
    if (typeof value === 'number' && Number.isFinite(value)) {
      metric[key] = Math.round(value * 100) / 100
    }
  }
  return metric
}
