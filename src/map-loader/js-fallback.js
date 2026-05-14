const JS_FALLBACK_ATTEMPTS = 6

export async function generateWithJsFallback(generatePuzzle, buildGuaranteedPuzzle, logStage, options = {}) {
  const targetSize = Number.isInteger(options.targetSize) ? options.targetSize : null
  const sizes = targetSize
    ? [targetSize]
    : Array.isArray(options.sizes) && options.sizes.length
      ? options.sizes.slice()
      : []
  const generationOptions = { ...options, sizes, targetSize }
  let puzzle = null
  for (let attempt = 0; attempt < JS_FALLBACK_ATTEMPTS && !puzzle; attempt++) {
    puzzle = await generatePuzzle(attempt, generationOptions)
  }
  if (!puzzle) {
    logStage('Rust/WASM unavailable; using guaranteed JS puzzle', 'fail')
    puzzle = await buildGuaranteedPuzzle(generationOptions)
  }
  return puzzle
}
