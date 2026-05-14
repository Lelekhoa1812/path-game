import { MapLoaderClient } from './worker-client.js'

const DEFAULT_SEEDS = [1, 2, 3, 4, 5]
const DEFAULT_SIZES = [8, 9, 10, 11]
const DEFAULT_TARGET_MS = 5000
const DEFAULT_MAX_MS = 10000

function now() {
  return performance.now()
}

function roundMs(value) {
  return Math.round(value * 100) / 100
}

function summarize(results) {
  const completed = results.filter((result) => result.status === 'success')
  const underBudget = completed.filter((result) => result.totalMs <= DEFAULT_MAX_MS)

  return {
    total: results.length,
    success: completed.length,
    failed: results.length - completed.length,
    successUnder10s: underBudget.length,
    successRate: results.length ? roundMs(completed.length / results.length) : 0,
    under10sRate: results.length ? roundMs(underBudget.length / results.length) : 0,
    averageMs: completed.length
      ? roundMs(completed.reduce((sum, result) => sum + result.totalMs, 0) / completed.length)
      : null,
  }
}

async function measure(mode, seed, sizes, generate) {
  const startedAt = now()
  try {
    const puzzle = await generate()
    const totalMs = roundMs(now() - startedAt)
    return {
      mode,
      seed,
      size: puzzle.R,
      difficulty: puzzle.difficulty,
      status: 'success',
      totalMs,
      under10s: totalMs <= DEFAULT_MAX_MS,
      metrics: puzzle.metrics || null,
    }
  } catch (error) {
    return {
      mode,
      seed,
      sizes,
      status: 'error',
      totalMs: roundMs(now() - startedAt),
      message: error && error.message ? error.message : String(error),
    }
  }
}

export async function runMapLoaderBench({
  seeds = DEFAULT_SEEDS,
  sizes = DEFAULT_SIZES,
  modes = ['rust'],
  jsGenerate,
  jsGuaranteed,
  logStage = () => {},
  randomRuns = 0,
  cancellation = true,
} = {}) {
  const client = new MapLoaderClient()
  const results = []

  if (modes.includes('rust')) {
    for (const seed of seeds) {
      results.push(await measure('rust', seed, sizes, () => (
        client.generate({ seed, sizes, targetMs: DEFAULT_TARGET_MS, maxMs: DEFAULT_MAX_MS })
      )))
    }
  }

  if (modes.includes('js')) {
    if (typeof jsGenerate !== 'function' || typeof jsGuaranteed !== 'function') {
      throw new Error('JS benchmark mode requires jsGenerate and jsGuaranteed')
    }
    const { generateWithJsFallback } = await import('./js-fallback.js')
    for (const seed of seeds) {
      results.push(await measure('js', seed, sizes, () => (
        generateWithJsFallback(jsGenerate, jsGuaranteed, logStage, { sizes })
      )))
    }
  }

  if (modes.includes('random')) {
    for (let i = 0; i < randomRuns; i++) {
      const seed = Date.now() + i
      results.push(await measure('random', seed, sizes, () => (
        client.generate({ seed, sizes, targetMs: DEFAULT_TARGET_MS, maxMs: DEFAULT_MAX_MS })
      )))
    }
  }

  let cancellationResult = null
  if (cancellation) {
    const cancelStartedAt = now()
    const cancelled = client.generate({ seed: seeds[0] || 1, sizes })
      .then(() => 'resolved')
      .catch((error) => error.message)
    client.cancel()
    cancellationResult = {
      mode: 'cancel',
      status: await cancelled,
      responseMs: roundMs(now() - cancelStartedAt),
    }
  }

  const summary = summarize(results)
  if (cancellationResult) summary.cancellation = cancellationResult

  console.table(results.map((row) => ({
    mode: row.mode,
    seed: row.seed,
    size: row.size,
    difficulty: row.difficulty,
    status: row.status,
    totalMs: row.totalMs,
    under10s: row.under10s,
    qualityScore: row.metrics && row.metrics.qualityScore,
    degradationLevel: row.metrics && row.metrics.degradationLevel,
  })))
  console.log(`[map-load-bench] ${JSON.stringify({ summary, results })}`)
  client.terminate()
  return { summary, results }
}

window.runMapLoaderBench = runMapLoaderBench
