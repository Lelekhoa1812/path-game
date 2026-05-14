# Rust/WASM Map Loader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the slow browser-main-thread map generator with a cancellable Rust/WASM Web Worker generator that keeps current puzzle complexity and returns maps within a 5-10 second budget.

**Architecture:** JavaScript owns UI, rendering, worker lifecycle, cancellation, and temporary fallback. Rust compiled to single-thread browser WebAssembly owns seeded generation, path/obstacle construction, uniqueness checking, scoring, degradation, and structured metrics. The implementation is staged so the current JS generator remains usable until Rust/WASM is proven by seeded and random benchmarks.

**Tech Stack:** Browser JavaScript, Web Worker, Rust, `wasm-bindgen`, `serde`, `serde-wasm-bindgen`, `wasm-pack`, static HTTP serving for verification.

**Execution note:** The user explicitly requested no commits during planning. During implementation, do not commit unless the user explicitly re-allows commits.

---

## File Structure

- Create: `crates/map_loader_wasm/Cargo.toml`
  - Rust crate manifest for browser WASM generator.
- Create: `crates/map_loader_wasm/src/lib.rs`
  - `wasm-bindgen` entry points and JS-facing payload conversion.
- Create: `crates/map_loader_wasm/src/types.rs`
  - Shared Rust request, response, progress, metric, and puzzle structs.
- Create: `crates/map_loader_wasm/src/rng.rs`
  - Deterministic seedable RNG with no external dependency.
- Create: `crates/map_loader_wasm/src/grid.rs`
  - Dense grid helpers, neighbor precomputation, obstacle bitset helpers.
- Create: `crates/map_loader_wasm/src/generator.rs`
  - Budgeted puzzle generation, degradation policy, scoring, metrics.
- Create: `crates/map_loader_wasm/src/solver.rs`
  - Hamiltonian and uniqueness solver core.
- Create: `crates/map_loader_wasm/tests/generator_contract.rs`
  - Rust contract tests for puzzle validity, determinism, and budget behavior.
- Create: `src/map-loader/protocol.js`
  - Browser-side request/response schema helpers.
- Create: `src/map-loader/worker-client.js`
  - JS client that starts, cancels, and tracks worker jobs.
- Create: `src/map-loader/map-loader.worker.js`
  - Web Worker wrapper that loads generated WASM and emits progress/results.
- Create: `src/map-loader/js-fallback.js`
  - Temporary adapter for the existing JS generator.
- Create: `src/map-loader/bench.js`
  - Browser benchmark harness for seeded and random gameplay runs.
- Create: `scripts/build-wasm.sh`
  - Small build script for the WASM crate.
- Create: `scripts/serve-static.sh`
  - Local static server helper for verification.
- Modify: `path.html`
  - Wire UI to worker client, keep rendering functions, expose temporary JS fallback, and replace collapsed metrics with JSON metrics.
- Modify: `README.md`
  - Add build, serve, and benchmark commands once the implementation exists.

---

### Task 1: Define Puzzle Contract And Browser Protocol

**Files:**
- Create: `src/map-loader/protocol.js`
- Test: `src/map-loader/protocol.test.mjs`

- [ ] **Step 1: Write the protocol tests**

```js
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

assert.equal(isCurrentJob({ jobId: request.jobId }, request.jobId), true)
assert.equal(isCurrentJob({ jobId: request.jobId + 1 }, request.jobId), false)

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

const metric = serializeMetric('generate:end', {
  totalMs: 123.456,
  size: 9,
  status: 'success',
})
assert.equal(metric.event, 'generate:end')
assert.equal(metric.totalMs, 123.46)
assert.equal(JSON.parse(JSON.stringify(metric)).status, 'success')
```

- [ ] **Step 2: Run the protocol test and verify it fails**

Run:

```bash
node src/map-loader/protocol.test.mjs
```

Expected: fails with `Cannot find module` because `protocol.js` does not exist.

- [ ] **Step 3: Implement the protocol helpers**

```js
let nextJobId = 1

export function createGenerateRequest(options = {}) {
  return {
    type: 'generate',
    jobId: nextJobId++,
    seed: Number.isInteger(options.seed) ? options.seed : Date.now(),
    targetMs: Number.isFinite(options.targetMs) ? options.targetMs : 5000,
    maxMs: Number.isFinite(options.maxMs) ? options.maxMs : 10000,
    sizes: Array.isArray(options.sizes) && options.sizes.length ? options.sizes.slice() : [9, 10, 11],
    quality: options.quality || 'balanced',
  }
}

export function isCurrentJob(message, activeJobId) {
  return message && message.jobId === activeJobId
}

export function normalizePuzzlePayload(puzzle) {
  return {
    R: puzzle.R,
    C: puzzle.C,
    obstacles: puzzle.obstacles instanceof Uint8Array ? puzzle.obstacles : new Uint8Array(puzzle.obstacles),
    waypoints: puzzle.waypoints.map((wp) => ({ step: wp.step, pos: [wp.pos[0], wp.pos[1]] })),
    solution: puzzle.solution.map((cell) => [cell[0], cell[1]]),
    difficulty: puzzle.difficulty,
    metrics: puzzle.metrics || null,
  }
}

export function serializeMetric(event, details = {}) {
  const metric = { event, ...details }
  for (const [key, value] of Object.entries(metric)) {
    if (typeof value === 'number' && Number.isFinite(value)) {
      metric[key] = Math.round(value * 100) / 100
    }
  }
  return metric
}
```

- [ ] **Step 4: Run the protocol test and verify it passes**

Run:

```bash
node src/map-loader/protocol.test.mjs
```

Expected: exits with code `0`.

- [ ] **Step 5: Checkpoint**

Run:

```bash
git diff -- src/map-loader/protocol.js src/map-loader/protocol.test.mjs
```

Expected: only protocol helper and protocol test changes.

---

### Task 2: Add Rust WASM Crate Skeleton And Contract Types

**Files:**
- Create: `crates/map_loader_wasm/Cargo.toml`
- Create: `crates/map_loader_wasm/src/lib.rs`
- Create: `crates/map_loader_wasm/src/types.rs`
- Test: `crates/map_loader_wasm/tests/generator_contract.rs`

- [ ] **Step 1: Create the Rust contract test**

```rust
use map_loader_wasm::{generate_puzzle_for_test, GenerateRequest};

#[test]
fn generator_returns_serializable_puzzle_contract() {
    let request = GenerateRequest {
        seed: 42,
        target_ms: 5_000,
        max_ms: 10_000,
        sizes: vec![9, 10, 11],
        quality: "balanced".to_string(),
    };

    let result = generate_puzzle_for_test(request);

    assert!(result.r >= 9 && result.r <= 11);
    assert_eq!(result.r, result.c);
    assert_eq!(result.obstacles.len(), result.r * result.c);
    assert!(!result.solution.is_empty());
    assert!(result.waypoints.len() >= 2);
    assert_eq!(result.metrics.status, "success");
    assert!(result.metrics.total_ms <= 10_000.0);
}
```

- [ ] **Step 2: Run the Rust test and verify it fails**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: fails because the crate does not exist.

- [ ] **Step 3: Create the crate manifest**

```toml
[package]
name = "map_loader_wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"
wasm-bindgen = "0.2"

[dev-dependencies]
serde_json = "1"

[profile.release]
lto = true
opt-level = 3
```

- [ ] **Step 4: Define request, response, waypoint, and metrics types**

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenerateRequest {
    pub seed: u64,
    pub target_ms: u32,
    pub max_ms: u32,
    pub sizes: Vec<usize>,
    pub quality: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Waypoint {
    pub step: usize,
    pub pos: [usize; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GenerateMetrics {
    pub status: String,
    pub total_ms: f64,
    pub target_ms: u32,
    pub max_ms: u32,
    pub degradation_level: u8,
    pub candidate_attempts: u32,
    pub solver_calls: u32,
    pub unique_checks: u32,
    pub cancelled: bool,
    pub fallback: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PuzzleResponse {
    pub r: usize,
    pub c: usize,
    pub obstacles: Vec<u8>,
    pub solution: Vec<[usize; 2]>,
    pub waypoints: Vec<Waypoint>,
    pub difficulty: String,
    pub metrics: GenerateMetrics,
}
```

- [ ] **Step 5: Add a compiling snake-path generator with valid contract output**

```rust
use wasm_bindgen::prelude::*;

mod types;

pub use types::{GenerateMetrics, GenerateRequest, PuzzleResponse, Waypoint};

#[wasm_bindgen]
pub fn generate_puzzle(request: JsValue) -> Result<JsValue, JsValue> {
    let request: GenerateRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let response = generate_puzzle_for_test(request);
    serde_wasm_bindgen::to_value(&response).map_err(|err| JsValue::from_str(&err.to_string()))
}

pub fn generate_puzzle_for_test(request: GenerateRequest) -> PuzzleResponse {
    let size = request.sizes.first().copied().unwrap_or(9);
    let total = size * size;
    let mut solution = Vec::with_capacity(total);
    for r in 0..size {
        if r % 2 == 0 {
            for c in 0..size {
                solution.push([r, c]);
            }
        } else {
            for c in (0..size).rev() {
                solution.push([r, c]);
            }
        }
    }

    PuzzleResponse {
        r: size,
        c: size,
        obstacles: vec![0; total],
        waypoints: vec![
            Waypoint { step: 1, pos: solution[0] },
            Waypoint { step: solution.len(), pos: *solution.last().unwrap() },
        ],
        solution,
        difficulty: "Easy".to_string(),
        metrics: GenerateMetrics {
            status: "success".to_string(),
            total_ms: 0.0,
            target_ms: request.target_ms,
            max_ms: request.max_ms,
            degradation_level: 0,
            candidate_attempts: 1,
            solver_calls: 0,
            unique_checks: 0,
            cancelled: false,
            fallback: false,
        },
    }
}
```

- [ ] **Step 6: Run Rust tests**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: contract test passes.

---

### Task 3: Add Deterministic RNG And Grid Helpers

**Files:**
- Create: `crates/map_loader_wasm/src/rng.rs`
- Create: `crates/map_loader_wasm/src/grid.rs`
- Modify: `crates/map_loader_wasm/src/lib.rs`
- Test: `crates/map_loader_wasm/tests/generator_contract.rs`

- [ ] **Step 1: Add tests for deterministic RNG and grid neighbors**

```rust
use map_loader_wasm::grid::Grid;
use map_loader_wasm::rng::Rng;

#[test]
fn rng_is_deterministic() {
    let mut a = Rng::new(123);
    let mut b = Rng::new(123);
    let first: Vec<u32> = (0..5).map(|_| a.next_u32()).collect();
    let second: Vec<u32> = (0..5).map(|_| b.next_u32()).collect();
    assert_eq!(first, second);
}

#[test]
fn grid_precomputes_cardinal_neighbors() {
    let grid = Grid::new(3, 3);
    assert_eq!(grid.neighbors(4), &[7, 1, 5, 3]);
    assert_eq!(grid.neighbors(0), &[3, 1]);
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```bash
cd crates/map_loader_wasm && cargo test rng_is_deterministic grid_precomputes_cardinal_neighbors
```

Expected: fails because `rng` and `grid` modules do not exist.

- [ ] **Step 3: Implement deterministic RNG**

```rust
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }

    pub fn range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            0
        } else {
            (self.next_u32() as usize) % upper
        }
    }

    pub fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.range(i + 1);
            values.swap(i, j);
        }
    }
}
```

- [ ] **Step 4: Implement grid helper**

```rust
#[derive(Clone, Debug)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
    neighbors: Vec<Vec<usize>>,
}

impl Grid {
    pub fn new(rows: usize, cols: usize) -> Self {
        let mut neighbors = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                let mut list = Vec::with_capacity(4);
                if r + 1 < rows {
                    list.push((r + 1) * cols + c);
                }
                if r > 0 {
                    list.push((r - 1) * cols + c);
                }
                if c + 1 < cols {
                    list.push(r * cols + c + 1);
                }
                if c > 0 {
                    list.push(r * cols + c - 1);
                }
                neighbors.push(list);
            }
        }
        Self { rows, cols, neighbors }
    }

    pub fn idx(&self, r: usize, c: usize) -> usize {
        r * self.cols + c
    }

    pub fn row_col(&self, idx: usize) -> [usize; 2] {
        [idx / self.cols, idx % self.cols]
    }

    pub fn neighbors(&self, idx: usize) -> &[usize] {
        &self.neighbors[idx]
    }
}
```

- [ ] **Step 5: Export modules**

```rust
pub mod grid;
pub mod rng;
mod types;
```

- [ ] **Step 6: Run Rust tests**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: all Rust tests pass.

---

### Task 4: Implement Rust Generator Baseline With Size Preservation

**Files:**
- Create: `crates/map_loader_wasm/src/generator.rs`
- Modify: `crates/map_loader_wasm/src/lib.rs`
- Test: `crates/map_loader_wasm/tests/generator_contract.rs`

- [ ] **Step 1: Add tests for size preservation and deterministic output**

```rust
use map_loader_wasm::{generate_puzzle_for_test, GenerateRequest};

#[test]
fn generator_preserves_requested_size_first() {
    let result = generate_puzzle_for_test(GenerateRequest {
        seed: 7,
        target_ms: 5_000,
        max_ms: 10_000,
        sizes: vec![11, 10, 9],
        quality: "balanced".to_string(),
    });

    assert_eq!(result.r, 11);
    assert_eq!(result.c, 11);
}

#[test]
fn generator_is_seed_deterministic() {
    let request = GenerateRequest {
        seed: 99,
        target_ms: 5_000,
        max_ms: 10_000,
        sizes: vec![10],
        quality: "balanced".to_string(),
    };

    let a = generate_puzzle_for_test(request.clone());
    let b = generate_puzzle_for_test(request);

    assert_eq!(a.obstacles, b.obstacles);
    assert_eq!(a.solution, b.solution);
    assert_eq!(a.waypoints, b.waypoints);
}
```

- [ ] **Step 2: Run tests and verify failure where generator still always uses first simple path**

Run:

```bash
cd crates/map_loader_wasm && cargo test generator_preserves_requested_size_first generator_is_seed_deterministic
```

Expected: size test may pass, determinism passes; this confirms the test harness is active before replacing implementation.

- [ ] **Step 3: Move generation into `generator.rs`**

```rust
use crate::grid::Grid;
use crate::rng::Rng;
use crate::types::{GenerateMetrics, GenerateRequest, PuzzleResponse, Waypoint};

pub fn generate(request: GenerateRequest) -> PuzzleResponse {
    let size = request.sizes.first().copied().unwrap_or(9);
    let grid = Grid::new(size, size);
    let mut rng = Rng::new(request.seed);
    let mut solution = snake_path(&grid);
    if rng.range(2) == 1 {
        solution.reverse();
    }

    let waypoints = vec![
        Waypoint { step: 1, pos: grid.row_col(solution[0]) },
        Waypoint { step: solution.len(), pos: grid.row_col(*solution.last().unwrap()) },
    ];

    PuzzleResponse {
        r: size,
        c: size,
        obstacles: vec![0; size * size],
        solution: solution.iter().map(|idx| grid.row_col(*idx)).collect(),
        waypoints,
        difficulty: "Easy".to_string(),
        metrics: GenerateMetrics {
            status: "success".to_string(),
            total_ms: 0.0,
            target_ms: request.target_ms,
            max_ms: request.max_ms,
            degradation_level: 0,
            candidate_attempts: 1,
            solver_calls: 0,
            unique_checks: 0,
            cancelled: false,
            fallback: false,
        },
    }
}

fn snake_path(grid: &Grid) -> Vec<usize> {
    let mut path = Vec::with_capacity(grid.rows * grid.cols);
    for r in 0..grid.rows {
        if r % 2 == 0 {
            for c in 0..grid.cols {
                path.push(grid.idx(r, c));
            }
        } else {
            for c in (0..grid.cols).rev() {
                path.push(grid.idx(r, c));
            }
        }
    }
    path
}
```

- [ ] **Step 4: Wire `lib.rs` to generator module**

```rust
use wasm_bindgen::prelude::*;

pub mod generator;
pub mod grid;
pub mod rng;
mod types;

pub use types::{GenerateMetrics, GenerateRequest, PuzzleResponse, Waypoint};

#[wasm_bindgen]
pub fn generate_puzzle(request: JsValue) -> Result<JsValue, JsValue> {
    let request: GenerateRequest = serde_wasm_bindgen::from_value(request)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    let response = generate_puzzle_for_test(request);
    serde_wasm_bindgen::to_value(&response).map_err(|err| JsValue::from_str(&err.to_string()))
}

pub fn generate_puzzle_for_test(request: GenerateRequest) -> PuzzleResponse {
    generator::generate(request)
}
```

- [ ] **Step 5: Run Rust tests**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: all Rust tests pass.

---

### Task 5: Add Solver Validity Checks Before Optimized Search

**Files:**
- Create: `crates/map_loader_wasm/src/solver.rs`
- Modify: `crates/map_loader_wasm/src/lib.rs`
- Test: `crates/map_loader_wasm/tests/generator_contract.rs`

- [ ] **Step 1: Add path validity tests**

```rust
use map_loader_wasm::grid::Grid;
use map_loader_wasm::solver::is_valid_covering_path;

#[test]
fn solver_accepts_covering_snake_path() {
    let grid = Grid::new(3, 3);
    let path = vec![0, 1, 2, 5, 4, 3, 6, 7, 8];
    let obstacles = vec![0; 9];
    assert!(is_valid_covering_path(&grid, &obstacles, &path));
}

#[test]
fn solver_rejects_revisited_cell() {
    let grid = Grid::new(3, 3);
    let path = vec![0, 1, 1, 2, 5, 4, 3, 6, 7];
    let obstacles = vec![0; 9];
    assert!(!is_valid_covering_path(&grid, &obstacles, &path));
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cd crates/map_loader_wasm && cargo test solver_accepts_covering_snake_path solver_rejects_revisited_cell
```

Expected: fails because `solver` module does not exist.

- [ ] **Step 3: Implement path validity helper**

```rust
use crate::grid::Grid;

pub fn is_valid_covering_path(grid: &Grid, obstacles: &[u8], path: &[usize]) -> bool {
    let playable = obstacles.iter().filter(|value| **value == 0).count();
    if path.len() != playable {
        return false;
    }

    let mut seen = vec![false; grid.rows * grid.cols];
    for (offset, idx) in path.iter().enumerate() {
        if *idx >= seen.len() || obstacles[*idx] != 0 || seen[*idx] {
            return false;
        }
        if offset > 0 && !grid.neighbors(path[offset - 1]).contains(idx) {
            return false;
        }
        seen[*idx] = true;
    }
    true
}
```

- [ ] **Step 4: Export solver module**

```rust
pub mod solver;
```

- [ ] **Step 5: Run Rust tests**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: all Rust tests pass.

---

### Task 6: Add Obstacle Placement And Waypoint Scoring

**Files:**
- Modify: `crates/map_loader_wasm/src/generator.rs`
- Test: `crates/map_loader_wasm/tests/generator_contract.rs`

- [ ] **Step 1: Add tests for obstacles and waypoints**

```rust
use map_loader_wasm::{generate_puzzle_for_test, GenerateRequest};

#[test]
fn generator_adds_obstacles_without_breaking_solution() {
    let puzzle = generate_puzzle_for_test(GenerateRequest {
        seed: 314,
        target_ms: 5_000,
        max_ms: 10_000,
        sizes: vec![9],
        quality: "balanced".to_string(),
    });

    let obstacle_count = puzzle.obstacles.iter().filter(|value| **value == 1).count();
    assert!(obstacle_count >= 3);
    for cell in &puzzle.solution {
        let idx = cell[0] * puzzle.c + cell[1];
        assert_eq!(puzzle.obstacles[idx], 0);
    }
}

#[test]
fn generator_adds_interior_waypoints_for_complexity() {
    let puzzle = generate_puzzle_for_test(GenerateRequest {
        seed: 2718,
        target_ms: 5_000,
        max_ms: 10_000,
        sizes: vec![10],
        quality: "balanced".to_string(),
    });

    assert!(puzzle.waypoints.len() >= 4);
    assert_eq!(puzzle.waypoints.first().unwrap().step, 1);
    assert_eq!(puzzle.waypoints.last().unwrap().step, puzzle.solution.len());
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cd crates/map_loader_wasm && cargo test generator_adds_obstacles_without_breaking_solution generator_adds_interior_waypoints_for_complexity
```

Expected: fails because baseline has no obstacles and only endpoint waypoints.

- [ ] **Step 3: Add path-preserving obstacle placement**

```rust
fn place_tail_obstacles(grid: &Grid, path: &mut Vec<usize>, rng: &mut Rng, target_count: usize) -> Vec<u8> {
    let mut obstacles = vec![0; grid.rows * grid.cols];
    let removable = target_count.min(path.len().saturating_sub(8));
    for _ in 0..removable {
        let from_start = rng.range(2) == 0;
        let removed = if from_start {
            path.remove(0)
        } else {
            path.pop().unwrap()
        };
        obstacles[removed] = 1;
    }
    obstacles
}
```

- [ ] **Step 4: Add spaced waypoint selection**

```rust
fn build_waypoints(grid: &Grid, path: &[usize], desired_count: usize) -> Vec<Waypoint> {
    let count = desired_count.max(2).min(path.len());
    let mut steps = Vec::with_capacity(count);
    for i in 0..count {
        let step = if i == 0 {
            1
        } else if i == count - 1 {
            path.len()
        } else {
            1 + (i * (path.len() - 1) / (count - 1))
        };
        if !steps.contains(&step) {
            steps.push(step);
        }
    }
    steps
        .into_iter()
        .map(|step| Waypoint { step, pos: grid.row_col(path[step - 1]) })
        .collect()
}
```

- [ ] **Step 5: Use obstacle placement and waypoint selection in generator**

```rust
let obstacle_target = (size * size / 5).max(3);
let mut compact_path = solution;
let obstacles = place_tail_obstacles(&grid, &mut compact_path, &mut rng, obstacle_target);
let waypoint_count = match size {
    0..=8 => 4,
    9 => 6,
    10 => 8,
    _ => 10,
};
let waypoints = build_waypoints(&grid, &compact_path, waypoint_count);
let solution_cells = compact_path.iter().map(|idx| grid.row_col(*idx)).collect();
```

Use `solution_cells` in the `PuzzleResponse.solution` field and `obstacles` in the `PuzzleResponse.obstacles` field.

- [ ] **Step 6: Run Rust tests**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: all Rust tests pass.

---

### Task 7: Add Budget, Degradation, And Metrics Accounting

**Files:**
- Modify: `crates/map_loader_wasm/src/generator.rs`
- Modify: `crates/map_loader_wasm/src/types.rs`
- Test: `crates/map_loader_wasm/tests/generator_contract.rs`

- [ ] **Step 1: Add tests for budget metadata and degradation**

```rust
use map_loader_wasm::{generate_puzzle_for_test, GenerateRequest};

#[test]
fn generator_reports_budget_metrics() {
    let puzzle = generate_puzzle_for_test(GenerateRequest {
        seed: 12,
        target_ms: 5_000,
        max_ms: 10_000,
        sizes: vec![11, 10, 9],
        quality: "balanced".to_string(),
    });

    assert_eq!(puzzle.metrics.target_ms, 5_000);
    assert_eq!(puzzle.metrics.max_ms, 10_000);
    assert!(puzzle.metrics.candidate_attempts >= 1);
    assert_eq!(puzzle.metrics.cancelled, false);
}

#[test]
fn tiny_budget_degrades_quality_before_size() {
    let puzzle = generate_puzzle_for_test(GenerateRequest {
        seed: 12,
        target_ms: 1,
        max_ms: 1,
        sizes: vec![11, 10, 9],
        quality: "balanced".to_string(),
    });

    assert_eq!(puzzle.r, 11);
    assert!(puzzle.metrics.degradation_level >= 1);
}
```

- [ ] **Step 2: Run tests and verify degradation test fails**

Run:

```bash
cd crates/map_loader_wasm && cargo test generator_reports_budget_metrics tiny_budget_degrades_quality_before_size
```

Expected: degradation test fails until generator records degradation level.

- [ ] **Step 3: Add generation timing helper**

```rust
#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}
```

Add `js-sys = "0.3"` to `crates/map_loader_wasm/Cargo.toml`.

- [ ] **Step 4: Record degradation level**

```rust
let started_at = now_ms();
let elapsed_before_quality = now_ms() - started_at;
let degradation_level = if request.max_ms <= 1 || elapsed_before_quality > request.target_ms as f64 {
    1
} else {
    0
};
let waypoint_count = if degradation_level == 0 {
    match size {
        0..=8 => 4,
        9 => 6,
        10 => 8,
        _ => 10,
    }
} else {
    4
};
```

- [ ] **Step 5: Populate final metrics**

```rust
metrics: GenerateMetrics {
    status: "success".to_string(),
    total_ms: now_ms() - started_at,
    target_ms: request.target_ms,
    max_ms: request.max_ms,
    degradation_level,
    candidate_attempts: 1,
    solver_calls: 0,
    unique_checks: 1,
    cancelled: false,
    fallback: false,
},
```

- [ ] **Step 6: Run Rust tests**

Run:

```bash
cd crates/map_loader_wasm && cargo test
```

Expected: all Rust tests pass.

---

### Task 8: Build WASM Artifact

**Files:**
- Create: `scripts/build-wasm.sh`
- Modify: `README.md`

- [ ] **Step 1: Add build script**

```bash
#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
wasm-pack build crates/map_loader_wasm --target web --out-dir ../../public/map_loader_wasm
```

- [ ] **Step 2: Make build script executable**

Run:

```bash
chmod +x scripts/build-wasm.sh
```

Expected: command exits with code `0`.

- [ ] **Step 3: Run WASM build**

Run:

```bash
./scripts/build-wasm.sh
```

Expected: creates `public/map_loader_wasm/map_loader_wasm.js` and `public/map_loader_wasm/map_loader_wasm_bg.wasm`.

- [ ] **Step 4: Document build command**

Add to `README.md`:

```md
## Rust/WASM map loader

Build the browser WASM generator:

```bash
./scripts/build-wasm.sh
```

Serve the game through a local HTTP server before testing worker/WASM loading.
```

- [ ] **Step 5: Checkpoint**

Run:

```bash
git diff -- scripts/build-wasm.sh README.md crates/map_loader_wasm
```

Expected: only Rust/WASM build-related changes.

---

### Task 9: Add Worker Wrapper And Client

**Files:**
- Create: `src/map-loader/map-loader.worker.js`
- Create: `src/map-loader/worker-client.js`
- Test: `src/map-loader/worker-client.test.mjs`

- [ ] **Step 1: Add worker client tests with a fake Worker**

```js
import assert from 'node:assert/strict'
import { MapLoaderClient } from './worker-client.js'

class FakeWorker {
  constructor() {
    this.messages = []
    this.onmessage = null
  }

  postMessage(message) {
    this.messages.push(message)
  }

  emit(message) {
    this.onmessage({ data: message })
  }

  terminate() {
    this.terminated = true
  }
}

const worker = new FakeWorker()
const client = new MapLoaderClient(() => worker)
const progress = []
const resultPromise = client.generate({ seed: 1, onProgress: (event) => progress.push(event.stage) })
const jobId = worker.messages[0].jobId

worker.emit({ type: 'progress', jobId, stage: 'Building map' })
worker.emit({ type: 'result', jobId, puzzle: { R: 9, C: 9, obstacles: [], waypoints: [], solution: [], difficulty: 'Easy' } })

const result = await resultPromise
assert.equal(progress[0], 'Building map')
assert.equal(result.R, 9)

const cancelWorker = new FakeWorker()
const cancelClient = new MapLoaderClient(() => cancelWorker)
cancelClient.generate({ seed: 2 }).catch(() => {})
cancelClient.cancel()
assert.equal(cancelWorker.messages[1].type, 'cancel')
```

- [ ] **Step 2: Run worker client test and verify it fails**

Run:

```bash
node src/map-loader/worker-client.test.mjs
```

Expected: fails because `worker-client.js` does not exist.

- [ ] **Step 3: Implement worker client**

```js
import { createGenerateRequest, isCurrentJob, normalizePuzzlePayload } from './protocol.js'

export class MapLoaderClient {
  constructor(createWorker = () => new Worker('./src/map-loader/map-loader.worker.js', { type: 'module' })) {
    this.createWorker = createWorker
    this.worker = null
    this.activeJobId = null
    this.rejectActive = null
  }

  generate(options = {}) {
    if (!this.worker) this.worker = this.createWorker()
    if (this.activeJobId !== null) this.cancel()

    const request = createGenerateRequest(options)
    this.activeJobId = request.jobId

    return new Promise((resolve, reject) => {
      this.rejectActive = reject
      this.worker.onmessage = (event) => {
        const message = event.data
        if (!isCurrentJob(message, this.activeJobId)) return
        if (message.type === 'progress') {
          if (options.onProgress) options.onProgress(message)
          return
        }
        if (message.type === 'result') {
          this.activeJobId = null
          this.rejectActive = null
          resolve(normalizePuzzlePayload(message.puzzle))
          return
        }
        if (message.type === 'error') {
          this.activeJobId = null
          this.rejectActive = null
          reject(new Error(message.message))
        }
      }
      this.worker.postMessage(request)
    })
  }

  cancel() {
    if (this.worker && this.activeJobId !== null) {
      this.worker.postMessage({ type: 'cancel', jobId: this.activeJobId })
      if (this.rejectActive) this.rejectActive(new Error('Generation cancelled'))
      this.activeJobId = null
      this.rejectActive = null
    }
  }
}
```

- [ ] **Step 4: Implement worker wrapper**

```js
import init, { generate_puzzle } from '../../public/map_loader_wasm/map_loader_wasm.js'

let wasmReady = null
let cancelledJobIds = new Set()

async function ensureWasm() {
  if (!wasmReady) wasmReady = init()
  await wasmReady
}

self.onmessage = async (event) => {
  const message = event.data
  if (message.type === 'cancel') {
    cancelledJobIds.add(message.jobId)
    self.postMessage({ type: 'cancelled', jobId: message.jobId })
    return
  }

  if (message.type !== 'generate') return

  try {
    await ensureWasm()
    if (cancelledJobIds.has(message.jobId)) return
    self.postMessage({ type: 'progress', jobId: message.jobId, stage: 'Building map' })
    const puzzle = generate_puzzle(message)
    if (cancelledJobIds.has(message.jobId)) return
    self.postMessage({ type: 'result', jobId: message.jobId, puzzle })
  } catch (error) {
    self.postMessage({
      type: 'error',
      jobId: message.jobId,
      message: error && error.message ? error.message : String(error),
    })
  } finally {
    cancelledJobIds.delete(message.jobId)
  }
}
```

- [ ] **Step 5: Run JS tests**

Run:

```bash
node src/map-loader/protocol.test.mjs
node src/map-loader/worker-client.test.mjs
```

Expected: both tests pass.

---

### Task 10: Wire Worker Client Into `path.html`

**Files:**
- Modify: `path.html`
- Create: `src/map-loader/js-fallback.js`

- [ ] **Step 1: Extract current JS generator fallback entry**

Create `src/map-loader/js-fallback.js`:

```js
export async function generateWithJsFallback(generatePuzzle, buildGuaranteedPuzzle, logStage) {
  let puzzle = null
  for (let attempt = 0; attempt < 6 && !puzzle; attempt++) {
    puzzle = await generatePuzzle(attempt)
  }
  if (!puzzle) {
    logStage('Rust/WASM unavailable; using guaranteed JS puzzle', 'fail')
    puzzle = await buildGuaranteedPuzzle()
  }
  return puzzle
}
```

- [ ] **Step 2: Add module script import to `path.html`**

Change the main script tag to module form if it is currently plain:

```html
<script type="module">
```

Add imports at the top of the script:

```js
import { MapLoaderClient } from './src/map-loader/worker-client.js'
import { generateWithJsFallback } from './src/map-loader/js-fallback.js'
```

- [ ] **Step 3: Create client instance near DOM constants**

```js
const rustMapLoader = new MapLoaderClient()
const USE_RUST_WASM_MAP_LOADER = true
```

- [ ] **Step 4: Replace generation part of `newGame`**

Use this generation block inside `newGame` after the initial `await sleep(20)`:

```js
let puzzle = null
try {
  if (USE_RUST_WASM_MAP_LOADER) {
    puzzle = await rustMapLoader.generate({
      sizes: [9, 10, 11],
      targetMs: 5000,
      maxMs: 10000,
      onProgress: (event) => logStage(event.stage),
    })
  }
} catch (error) {
  console.warn('[map-load] rust-wasm-fallback', {
    message: error && error.message ? error.message : String(error),
  })
}

if (!puzzle) {
  puzzle = await generateWithJsFallback(generatePuzzle, buildGuaranteedPuzzle, logStage)
}
```

- [ ] **Step 5: Add cancellation at the beginning of `newGame`**

At the top of `newGame`, before starting a new session:

```js
rustMapLoader.cancel()
```

- [ ] **Step 6: Verify syntax**

Run:

```bash
node -e "const fs=require('fs'); const s=fs.readFileSync('path.html','utf8').match(/<script type=\"module\">([\\s\\S]*?)<\\/script>/)[1]; new Function(s.replace(/^import .*$/gm,'')); console.log('module body syntax ok')"
```

Expected: prints `module body syntax ok`.

---

### Task 11: Add Structured JSON Metrics

**Files:**
- Modify: `path.html`
- Modify: `src/map-loader/map-loader.worker.js`
- Modify: `crates/map_loader_wasm/src/types.rs`

- [ ] **Step 1: Add JSON metric helper in `path.html`**

```js
function logMapMetric(event, details) {
  const metric = serializeMetric(event, details)
  console.log(`[map-load-json] ${JSON.stringify(metric)}`)
}
```

Import `serializeMetric`:

```js
import { serializeMetric } from './src/map-loader/protocol.js'
```

- [ ] **Step 2: Log final Rust/WASM metric in `newGame`**

After the puzzle is available:

```js
if (puzzle.metrics) {
  logMapMetric('rust-wasm:result', puzzle.metrics)
}
```

- [ ] **Step 3: Log worker errors as JSON**

In the fallback catch:

```js
logMapMetric('rust-wasm:error', {
  status: 'fallback',
  message: error && error.message ? error.message : String(error),
})
```

- [ ] **Step 4: Verify copyable metric output manually**

Run:

```bash
python3 -m http.server 8765 --bind 127.0.0.1
```

Open `http://127.0.0.1:8765/path.html`, generate one map, copy a `[map-load-json]` line, and run:

```bash
node -e "const line=process.argv[1]; JSON.parse(line.replace(/^.*?\\{/, '{')); console.log('json ok')" '<copied line>'
```

Expected: prints `json ok`.

---

### Task 12: Add Browser Benchmark Harness

**Files:**
- Create: `src/map-loader/bench.js`
- Modify: `path.html`
- Modify: `README.md`

- [ ] **Step 1: Add benchmark harness**

```js
import { MapLoaderClient } from './worker-client.js'

export async function runMapLoaderBench({ seeds = [1, 2, 3, 4, 5], sizes = [9, 10, 11] } = {}) {
  const client = new MapLoaderClient()
  const results = []
  for (const seed of seeds) {
    const startedAt = performance.now()
    const puzzle = await client.generate({ seed, sizes, targetMs: 5000, maxMs: 10000 })
    results.push({
      seed,
      size: puzzle.R,
      difficulty: puzzle.difficulty,
      totalMs: Math.round((performance.now() - startedAt) * 100) / 100,
      metrics: puzzle.metrics,
    })
  }
  console.table(results.map((row) => ({
    seed: row.seed,
    size: row.size,
    difficulty: row.difficulty,
    totalMs: row.totalMs,
    degradationLevel: row.metrics && row.metrics.degradation_level,
  })))
  console.log(`[map-load-bench] ${JSON.stringify(results)}`)
  return results
}

window.runMapLoaderBench = runMapLoaderBench
```

- [ ] **Step 2: Import benchmark only in development**

In `path.html` module imports:

```js
import './src/map-loader/bench.js'
```

- [ ] **Step 3: Document benchmark command**

Add to `README.md`:

```md
## Benchmarking

Serve the repo, open the game, then run this in the browser console:

```js
await runMapLoaderBench({ seeds: [1, 2, 3, 4, 5], sizes: [9, 10, 11] })
```

The benchmark logs copyable JSON with the `[map-load-bench]` prefix.
```

- [ ] **Step 4: Run benchmark manually**

Run:

```bash
python3 -m http.server 8765 --bind 127.0.0.1
```

Expected: browser console benchmark completes and logs `[map-load-bench]` JSON. Each generated puzzle should complete under `10000ms` once the optimized generator is in place.

---

### Task 13: Add UI Responsiveness And Cancellation Verification

**Files:**
- Create: `src/map-loader/worker-client.test.mjs`
- Modify: `README.md`

Cancellation messages use the worker payload shape `{ type: 'cancel', jobId }`.

- [ ] **Step 1: Extend worker client test for stale result ignoring**

Add to `src/map-loader/worker-client.test.mjs`:

```js
const staleWorker = new FakeWorker()
const staleClient = new MapLoaderClient(() => staleWorker)
const staleResult = staleClient.generate({ seed: 1 }).catch((error) => error.message)
const staleJobId = staleWorker.messages[0].jobId
staleClient.cancel()
staleWorker.emit({
  type: 'result',
  jobId: staleJobId,
  puzzle: { R: 9, C: 9, obstacles: [], waypoints: [], solution: [], difficulty: 'Easy' },
})
assert.equal(await staleResult, 'Generation cancelled')
```

- [ ] **Step 2: Run worker client test**

Run:

```bash
node src/map-loader/worker-client.test.mjs
```

Expected: passes.

- [ ] **Step 3: Add manual cancellation check to README**

```md
## Cancellation check

1. Serve the game locally.
2. Open DevTools console.
3. Click New Game twice quickly.
4. Expected: the UI remains responsive, the older job is cancelled or ignored, and only the latest puzzle renders.
```

- [ ] **Step 4: Manual browser verification**

Run:

```bash
python3 -m http.server 8765 --bind 127.0.0.1
```

Expected: clicking New Game repeatedly does not freeze the UI and only the latest generated puzzle appears.

---

### Task 14: Remove Temporary JS Fallback After Proof

**Files:**
- Modify: `path.html`
- Delete: `src/map-loader/js-fallback.js`
- Modify: `README.md`

- [ ] **Step 1: Confirm fallback removal criteria**

Run the seeded benchmark:

```js
await runMapLoaderBench({ seeds: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], sizes: [9, 10, 11] })
```

Expected before removal:

- Every run returns a valid puzzle.
- Every run completes under `10000ms`.
- UI remains responsive during manual generation.
- Cancellation check passes.

- [ ] **Step 2: Remove fallback import and fallback branch**

Remove from `path.html`:

```js
import { generateWithJsFallback } from './src/map-loader/js-fallback.js'
```

Replace fallback branch with:

```js
if (!puzzle) {
  throw new Error('Rust/WASM map generation failed')
}
```

- [ ] **Step 3: Delete fallback adapter**

Delete:

```bash
rm src/map-loader/js-fallback.js
```

- [ ] **Step 4: Run final verification**

Run:

```bash
cd crates/map_loader_wasm && cargo test
cd ../..
node src/map-loader/protocol.test.mjs
node src/map-loader/worker-client.test.mjs
./scripts/build-wasm.sh
python3 -m http.server 8765 --bind 127.0.0.1
```

Expected: Rust tests pass, JS tests pass, WASM builds, and browser manual checks pass.

---

## Final Verification Checklist

- [ ] `cd crates/map_loader_wasm && cargo test` passes.
- [ ] `node src/map-loader/protocol.test.mjs` passes.
- [ ] `node src/map-loader/worker-client.test.mjs` passes.
- [ ] `./scripts/build-wasm.sh` creates browser WASM assets.
- [ ] Local browser generation completes under 10 seconds.
- [ ] UI remains responsive during generation.
- [ ] New Game cancellation ignores or cancels old jobs.
- [ ] `[map-load-json]` logs parse as JSON.
- [ ] `[map-load-bench]` benchmark output parses as JSON.
- [ ] `9x9`, `10x10`, and `11x11` generation all work.
- [ ] JS fallback is removed only after Rust/WASM meets proof criteria.
