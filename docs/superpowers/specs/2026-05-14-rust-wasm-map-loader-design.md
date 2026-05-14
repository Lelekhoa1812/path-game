# Rust/WASM Map Loader Design

## Context

The current map loader runs inside `path.html` on the browser main thread. Metrics from `bench-without-rust-map-loader.txt.log` show a full map-load session around 98 seconds while `buildBoard` took only a few milliseconds. The slow path is generation work: repeated candidate attempts, uniqueness solving, Hamiltonian path search, and quality filtering.

The game should remain browser-playable. A native `napi-rs` addon is not the right first runtime for that constraint because it does not run directly in a normal browser page. The first optimization target is Rust compiled to WebAssembly and executed inside a Web Worker.

## Goals

- Keep the game browser-playable with static-friendly built output.
- Keep current board sizes, including larger `10x10` and `11x11` puzzles.
- Target normal map generation around 5 seconds, with a hard cap near 10 seconds.
- Keep the UI responsive while maps generate.
- Preserve the existing puzzle rules: one path covers every non-obstacle cell exactly once, obstacles block cells, numbered waypoints constrain the path, and the puzzle should have a unique intended solution.
- Preserve player-facing complexity while allowing internal algorithm changes.
- Produce copyable JSON metrics for JS and Rust/WASM comparisons.
- Keep the current JS generator only as a temporary fallback during migration.

## Non-Goals

- Do not preserve the current tournament algorithm as an implementation requirement.
- Do not implement threaded WebAssembly in the first version.
- Do not move map generation to a backend service.
- Do not optimize rendering as part of this work; current evidence points to generation, not board rendering.

## Architecture

The UI remains JavaScript. A dedicated Web Worker owns map generation. Rust code is compiled to browser WebAssembly and loaded by that worker.

JavaScript responsibilities:

- Start, cancel, and track generation jobs.
- Render player-friendly loading states.
- Render final puzzle data into the existing board UI.
- Ignore late results from cancelled or superseded jobs.
- Keep temporary JS fallback during migration and benchmarking.

Rust/WASM worker responsibilities:

- Seeded random number generation.
- Candidate generation.
- Obstacle and path construction.
- Hamiltonian search.
- Uniqueness solving.
- Difficulty scoring.
- Timeout and degradation policy.
- Structured progress and metrics events.

The worker API should stay small and stable:

```ts
generatePuzzle({
  seed,
  targetMs: 5000,
  maxMs: 10000,
  sizes: [9, 10, 11],
  quality: "balanced"
})
```

The worker returns progress events, a final puzzle payload, cancellation acknowledgement, or an error payload.

## Generation Strategy

The Rust implementation should replace the current retry-heavy tournament with a bounded, degradation-aware strategy.

Primary flow:

1. Choose the requested target size.
2. Generate or construct a valid Hamiltonian path first.
3. Place obstacles in ways that preserve that path where possible.
4. Add numbered waypoints while checking uniqueness.
5. Score candidate puzzles for turn density, obstacle spread, waypoint density, and path trickiness.
6. Return the best valid candidate within the time budget.

The implementation may still use a small tournament over viable candidates, but it must not repeatedly spend large budgets on unlikely layouts. Candidate attempts, solver calls, and uniqueness checks need explicit limits.

## Degradation Policy

The generator should preserve board size before preserving difficulty.

When time is running low:

1. Keep the target size and relax quality thresholds.
2. Reduce candidate count.
3. Allow fewer obstacles.
4. Allow fewer waypoint splits.
5. Relax spread, tortuosity, and trickiness thresholds.
6. Degrade size only as a last resort if no valid unique puzzle can be returned before the hard deadline.

The generator must not exceed the hard deadline just to preserve difficulty.

## Cancellation

Generation must be cancellable.

If the user clicks New Game while a generation job is active:

1. JS sends `cancel(jobId)` to the worker.
2. The worker checks cancellation at safe points in generation and solver loops.
3. JS starts the next job or updates the loading state immediately.
4. Late results from old jobs are ignored by job id.

Cancellation should be cooperative rather than abruptly terminating the worker unless the worker becomes unresponsive.

## Progress And Metrics

Player-facing progress should be simple:

- Building map
- Checking uniqueness
- Tuning difficulty
- Finalizing

Detailed metrics should go to console and benchmark logs as JSON, not as collapsed console objects.

Each generation result should include:

- `seed`
- `size`
- `difficulty`
- `status`
- `totalMs`
- `targetMs`
- `maxMs`
- `degradationLevel`
- `candidateAttempts`
- `solverCalls`
- `uniqueChecks`
- phase timings
- cancellation status
- fallback status

This makes JS-vs-Rust and random gameplay comparisons reliable.

## Benchmarking

Benchmarking needs two modes:

- Seeded benchmark mode for direct JS and Rust/WASM comparison.
- Random gameplay metrics for real user-facing load behavior.

Benchmark comparisons should track:

- Total generation time.
- Success rate under 10 seconds.
- Board size returned.
- Difficulty and quality score.
- Number of candidate attempts.
- Number and cost of solver calls.
- Whether degradation was needed.
- UI responsiveness while generation runs.
- Cancellation behavior.

The current JS generator remains as a fallback until Rust/WASM consistently meets the target.

## Success Criteria

- Normal map generation usually completes in about 5 seconds.
- Generation has a hard cap around 10 seconds.
- UI remains interactive during generation.
- `9x9`, `10x10`, and `11x11` remain supported.
- Puzzle rules remain unchanged.
- Degradation preserves size before reducing size.
- Metrics are copyable JSON.
- JS fallback can be removed after Rust/WASM is proven by seeded and gameplay benchmarks.

## Risks

- Rust may improve raw solver speed without fully fixing worst-case exponential search.
- WASM integration adds build and loading complexity.
- Preserving `11x11` quality under 10 seconds may require algorithm changes, not only a direct port.
- Duplicate JS and Rust generators during migration can drift unless benchmarks and schemas stay strict.

## Open Implementation Notes

- Prefer single-thread WASM in a worker first for static compatibility.
- Shape the worker contract so a future multi-threaded WASM implementation can replace the internals without changing UI code.
- Use compact Rust data structures: bitsets, dense arrays, precomputed neighbor lists, deterministic RNG, and minimal path cloning.
- Keep generated puzzle payloads plain and serializable for easy worker transfer.
