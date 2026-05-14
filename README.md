# Path Game with core algorithms being written in Rust and compiled/interacted to/via WebAssembly (WASM)

idk, this game kinda fun though, read the instruction first :))

## JS to Rust/WASM map-loader migration

The map loader has been migrated from the browser main-thread JavaScript
implementation to a Rust core compiled to WebAssembly and executed through a
module worker. JavaScript still owns UI, rendering, input handling, worker
lifecycle, cancellation guards, JSON logging, and the temporary JS fallback.
Rust owns the hot path: seeded candidate generation, path validation,
waypoint-constrained uniqueness checks, complexity scoring, budget degradation,
and the final puzzle payload.

The migration keeps the original game complexity instead of replacing it with a
simple fast path:

- generated maps still cover every playable cell with a valid path
- waypoint constraints are used to prove uniqueness, not just endpoint checks
- divergent path alternatives are split with scored hint placement
- hint placement preserves spatial spacing, detour/trickiness, bisection, buffer,
  and step-gap heuristics from the JS algorithm
- difficulty and quality are derived from turns, obstacle spread, tortuosity,
  adjacency, waypoint density, and obstacle ratio
- current supported sizes are `8x8`, `9x9`, `10x10`, and `11x11`

## Performance result

The benchmark logs in `logs/` show the performance change:

| Run | Measured time | Notes |
| --- | ---: | --- |
| `logs/bench-without-rust-map-loader.txt.log` | `98,280ms` | JS main-thread load session |
| `logs/bench-with-rust-map-loader.txt.log` | `171ms` | Rust/WASM generator core |
| `logs/bench-with-rust-map-loader.txt.log` | `720.8ms` | full browser load session including UI/render overhead |

That is roughly:

- `~575x` faster for generation core compared with the old JS session time
- `~136x` faster end-to-end for the measured browser session
- build-board/rendering remained cheap in both versions (`3.4ms` before,
  `0.6ms` after), confirming generation was the bottleneck

The Rust run logged:

```text
size=9 qualityScore=0.96 candidateAttempts=40 solverCalls=5342729 uniqueChecks=36 fallback=false
```

So the fast path is not the fallback path; it is still doing the solver and
uniqueness work needed for the game.

## Rust/WASM map loader

Build the browser WASM generator:

```bash
./scripts/build-wasm.sh
```

Serve the game through a local HTTP server before testing worker/WASM loading.

## Benchmarking

Serve the repo, open the game, then run this in the browser console:

```js
await runMapLoaderBench({
  seeds: [1, 2, 3, 4, 5],
  sizes: [8, 9, 10, 11],
  modes: ['rust', 'js', 'random'],
  randomRuns: 3,
  cancellation: true,
})
```

The benchmark logs copyable JSON with the `[map-load-bench]` prefix, including
Rust/WASM, JS fallback, random-run, success-under-10s, and cancellation response
metrics.

## Cancellation check

The browser map loader sends a cancel message to the worker first and handles a
`cancelled` acknowledgement when the worker can process it. The current
single-thread WASM generator runs synchronously, so Rust-side cooperative
safe-point cancellation is not available here; if a synchronous job cannot
respond, the client ignores the stale result and uses a fresh worker for the
next request.

1. Serve the game locally.
2. Open DevTools console.
3. Click New Game twice quickly.
4. Expected: the UI remains responsive, the older job is cancelled or ignored, and only the latest puzzle renders.
