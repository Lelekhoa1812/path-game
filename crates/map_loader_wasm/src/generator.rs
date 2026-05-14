use crate::grid::Grid;
use crate::rng::Rng;
use crate::solver::{
    count_covering_paths, find_up_to_solutions_with_waypoints, is_valid_covering_path, PathCount,
    SolutionSearchResult,
};
use crate::types::{
    GenerateMetrics, GeneratePhaseTimings, GenerateRequest, PuzzleResponse, Waypoint,
};

pub fn generate(request: GenerateRequest) -> PuzzleResponse {
    let started_at = now_ms();
    let size = requested_size(&request);
    let grid = Grid::new(size, size);
    let mut rng = Rng::new(request.seed);
    let budget = GenerationBudget::new(&request);
    let force_degraded = budget.force_degraded;
    let mut candidate_attempts = 0;
    let mut solver_calls = 0;
    let mut unique_checks = 0;
    let mut best_checked_candidate = None;

    if !force_degraded {
        for _ in 0..budget.attempts {
            if budget.expired(started_at) {
                break;
            }
            candidate_attempts += 1;
            let Some(candidate) = build_candidate(&grid, &mut rng) else {
                continue;
            };
            let (waypoints, path_count) = make_unique_waypoints(
                &grid,
                &candidate.obstacles,
                &candidate.path,
                target_waypoint_count(size),
                budget.solver_call_cap,
            );
            solver_calls += path_count.solver_calls;
            unique_checks += 1;

            let stats = complexity_stats(&grid, &candidate.path, &candidate.obstacles, &waypoints);
            let score = candidate_score(&stats);
            let scored = ScoredCandidate {
                path: candidate.path,
                obstacles: candidate.obstacles,
                waypoints,
                solutions: path_count.solutions,
                score,
            };
            let should_replace = best_checked_candidate
                .as_ref()
                .is_none_or(|best| scored.outranks(best));
            if should_replace {
                best_checked_candidate = Some(scored);
            }
        }
    }

    let (path, obstacles, waypoints, fallback) = if let Some(candidate) = best_checked_candidate {
        (
            candidate.path,
            candidate.obstacles,
            candidate.waypoints,
            candidate.solutions != 1,
        )
    } else {
        candidate_attempts += 1;
        let mut path = snake_path(&grid);
        if rng.range(2) == 1 {
            path.reverse();
        }
        let obstacle_target = (size * size / 5).max(3);
        let obstacles = place_tail_obstacles(&grid, &mut path, &mut rng, obstacle_target);
        let path_count =
            count_covering_paths(&grid, &obstacles, path[0], *path.last().unwrap(), 2, 12_000);
        solver_calls += path_count.solver_calls;
        unique_checks += 1;
        let waypoints = build_waypoints(&grid, &path, 4);
        (path, obstacles, waypoints, true)
    };

    debug_assert!(is_valid_covering_path(&grid, &obstacles, &path));

    let elapsed_before_quality = now_ms() - started_at;
    let degraded = force_degraded || fallback;
    let degradation_level = if degraded { 1 } else { 0 };
    let solution_cells = path.iter().map(|idx| grid.row_col(*idx)).collect();
    let stats = complexity_stats(&grid, &path, &obstacles, &waypoints);
    let difficulty = difficulty_for(fallback, &stats).to_string();
    let total_ms = now_ms() - started_at;
    let quality_ms = (total_ms - elapsed_before_quality).max(0.0);

    PuzzleResponse {
        r: size,
        c: size,
        obstacles,
        solution: solution_cells,
        waypoints,
        difficulty,
        metrics: GenerateMetrics {
            status: "success".to_string(),
            seed: request.seed,
            size,
            quality: request.quality,
            quality_score: quality_score(degraded, fallback, &stats),
            phase_timings: GeneratePhaseTimings {
                candidate_ms: elapsed_before_quality,
                quality_ms,
                total_ms,
            },
            total_ms,
            target_ms: request.target_ms,
            max_ms: request.max_ms,
            degradation_level,
            candidate_attempts,
            solver_calls,
            unique_checks,
            cancelled: false,
            fallback,
        },
    }
}

struct GenerationBudget {
    attempts: usize,
    solver_call_cap: u32,
    max_ms: f64,
    force_degraded: bool,
}

impl GenerationBudget {
    fn new(request: &GenerateRequest) -> Self {
        let max_ms = request.max_ms as f64;
        let force_degraded = request.max_ms <= 25 || request.target_ms <= 1;
        let attempts = if force_degraded {
            1
        } else {
            ((request.target_ms as usize) / 125).clamp(8, 48)
        };
        let solver_call_cap = if force_degraded {
            2_000
        } else {
            (request.max_ms.saturating_mul(3)).clamp(8_000, 30_000)
        };

        Self {
            attempts,
            solver_call_cap,
            max_ms,
            force_degraded,
        }
    }

    fn expired(&self, started_at: f64) -> bool {
        now_ms() - started_at >= self.max_ms
    }
}

#[derive(Clone, Copy, Debug)]
struct ComplexityStats {
    turn_ratio: f64,
    obstacle_spread: f64,
    tort_avg: f64,
    tort_min: f64,
    waypoint_density: f64,
    adjacent_obstacle_ratio: f64,
    obstacle_ratio: f64,
    waypoint_count: usize,
    size: usize,
}

fn quality_score(degraded: bool, fallback: bool, stats: &ComplexityStats) -> f64 {
    if fallback {
        return 0.5;
    }
    if degraded {
        return 0.75;
    }

    let spread_floor = stats.size as f64 * 0.25;
    let spread_score = (stats.obstacle_spread / spread_floor.max(1.0)).min(1.0);
    let tort_avg_score = (stats.tort_avg / 1.20).min(1.0);
    let tort_min_score = (stats.tort_min / 1.00).min(1.0);
    let adjacency_score = (1.0 - (stats.adjacent_obstacle_ratio / 0.20).min(1.0)).max(0.0);
    let waypoint_score = (1.0 - (stats.waypoint_density / 0.30).min(1.0)).max(0.0);
    let turn_score = (stats.turn_ratio / 0.25).min(1.0);

    let complexity_score = spread_score * 0.18
        + tort_avg_score * 0.24
        + tort_min_score * 0.18
        + adjacency_score * 0.14
        + waypoint_score * 0.10
        + turn_score * 0.16;

    (0.85 + complexity_score * 0.14).clamp(0.0, 0.99)
}

fn difficulty_for(fallback: bool, stats: &ComplexityStats) -> &'static str {
    if fallback {
        return "Easy";
    }

    let score = stats.size as f64
        + stats.turn_ratio * 24.0
        + stats.waypoint_count as f64 * 1.15
        + stats.obstacle_ratio * 20.0
        + stats.tort_avg * 3.0
        + stats.tort_min * 2.0;

    if score >= 42.0 {
        "Expert"
    } else if score >= 30.0 {
        "Hard"
    } else {
        "Medium"
    }
}

struct Candidate {
    path: Vec<usize>,
    obstacles: Vec<u8>,
}

struct ScoredCandidate {
    path: Vec<usize>,
    obstacles: Vec<u8>,
    waypoints: Vec<Waypoint>,
    solutions: u32,
    score: i64,
}

impl ScoredCandidate {
    fn outranks(&self, other: &Self) -> bool {
        self.solutions < other.solutions
            || (self.solutions == other.solutions && self.score > other.score)
    }
}

fn requested_size(request: &GenerateRequest) -> usize {
    request
        .sizes
        .iter()
        .copied()
        .find(|size| matches!(size, 8..=11))
        .unwrap_or(9)
}

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

fn build_candidate(grid: &Grid, rng: &mut Rng) -> Option<Candidate> {
    let total = grid.rows * grid.cols;
    let min_len = total * 2 / 3;
    let target_len = total - (total / 5).max(3);
    let mut visited = vec![false; total];
    let mut path = Vec::with_capacity(target_len);
    let start = rng.range(total);
    visited[start] = true;
    path.push(start);

    while path.len() < target_len {
        let current = *path.last().unwrap();
        let mut neighbors = grid
            .neighbors(current)
            .iter()
            .copied()
            .filter(|idx| !visited[*idx])
            .collect::<Vec<_>>();
        if neighbors.is_empty() {
            break;
        }
        rng.shuffle(&mut neighbors);
        neighbors.sort_by_key(|idx| onward_count(grid, &visited, *idx));
        let next = neighbors[0];
        visited[next] = true;
        path.push(next);
    }

    if path.len() < min_len {
        return None;
    }

    let mut obstacles = vec![1; total];
    for idx in &path {
        obstacles[*idx] = 0;
    }

    if is_valid_covering_path(grid, &obstacles, &path) {
        Some(Candidate { path, obstacles })
    } else {
        None
    }
}

fn onward_count(grid: &Grid, visited: &[bool], idx: usize) -> usize {
    grid.neighbors(idx)
        .iter()
        .filter(|next| !visited[**next])
        .count()
}

fn place_tail_obstacles(
    grid: &Grid,
    path: &mut Vec<usize>,
    rng: &mut Rng,
    target_count: usize,
) -> Vec<u8> {
    let mut obstacles = vec![0; grid.rows * grid.cols];
    let removal_count = target_count.min(path.len().saturating_sub(8));
    for _ in 0..removal_count {
        let remove_from_start = rng.range(2) == 0;
        let removed = if remove_from_start {
            path.remove(0)
        } else {
            path.pop().unwrap()
        };
        obstacles[removed] = 1;
    }
    obstacles
}

fn target_waypoint_count(size: usize) -> usize {
    match size {
        0..=8 => 4,
        9 => 6,
        10 => 8,
        _ => 10,
    }
}

fn make_unique_waypoints(
    grid: &Grid,
    obstacles: &[u8],
    path: &[usize],
    desired_count: usize,
    solver_call_cap: u32,
) -> (Vec<Waypoint>, PathCount) {
    let mut waypoints = build_waypoints(grid, path, desired_count);
    let max_waypoints = (desired_count + 8).min(path.len());
    let mut solver_calls = 0;

    for _ in 0..max_waypoints {
        let constraints = waypoint_constraints(&waypoints, grid.cols);
        let result =
            find_up_to_solutions_with_waypoints(grid, obstacles, &constraints, 2, solver_call_cap);
        solver_calls += result.solver_calls;

        if !result.exhausted && result.solutions.len() == 1 {
            return (waypoints, waypoint_path_count(&result, solver_calls));
        }

        if waypoints.len() >= max_waypoints {
            return (waypoints, waypoint_path_count(&result, solver_calls));
        }

        let next_step = result
            .solutions
            .first()
            .zip(result.solutions.get(1))
            .and_then(|(first, second)| {
                choose_divergence_step(grid, path, obstacles, &waypoints, first, second)
            })
            .unwrap_or_else(|| largest_waypoint_gap_midpoint(&waypoints));

        if !insert_waypoint_near_step(grid, path, &mut waypoints, next_step) {
            return (waypoints, waypoint_path_count(&result, solver_calls));
        }
    }

    (
        waypoints,
        PathCount {
            solutions: 2,
            solver_calls,
        },
    )
}

fn waypoint_path_count(result: &SolutionSearchResult, solver_calls: u32) -> PathCount {
    PathCount {
        solutions: if result.exhausted {
            2
        } else {
            result.solutions.len() as u32
        },
        solver_calls,
    }
}

fn waypoint_constraints(waypoints: &[Waypoint], cols: usize) -> Vec<(usize, usize)> {
    waypoints
        .iter()
        .map(|waypoint| (waypoint.step, waypoint.pos[0] * cols + waypoint.pos[1]))
        .collect()
}

fn divergent_steps<'a>(
    first: &'a [usize],
    second: &'a [usize],
) -> impl Iterator<Item = usize> + 'a {
    first
        .iter()
        .zip(second.iter())
        .enumerate()
        .filter_map(|(offset, (a, b))| (a != b).then_some(offset + 1))
}

fn choose_divergence_step(
    grid: &Grid,
    path: &[usize],
    obstacles: &[u8],
    waypoints: &[Waypoint],
    first: &[usize],
    second: &[usize],
) -> Option<usize> {
    divergent_steps(first, second)
        .filter(|step| can_insert_waypoint(grid, path, waypoints, *step))
        .max_by(|a, b| {
            waypoint_candidate_score(grid, path, obstacles, waypoints, *a).total_cmp(
                &waypoint_candidate_score(grid, path, obstacles, waypoints, *b),
            )
        })
}

fn largest_waypoint_gap_midpoint(waypoints: &[Waypoint]) -> usize {
    waypoints
        .windows(2)
        .max_by_key(|pair| pair[1].step - pair[0].step)
        .map(|pair| pair[0].step + (pair[1].step - pair[0].step) / 2)
        .unwrap_or(1)
}

fn insert_waypoint_near_step(
    grid: &Grid,
    path: &[usize],
    waypoints: &mut Vec<Waypoint>,
    preferred_step: usize,
) -> bool {
    let max_delta = path.len();
    for delta in 0..=max_delta {
        for step in candidate_steps(preferred_step, delta, path.len()) {
            if can_insert_waypoint(grid, path, waypoints, step) {
                waypoints.push(Waypoint {
                    step,
                    pos: grid.row_col(path[step - 1]),
                });
                waypoints.sort_by_key(|waypoint| waypoint.step);
                return true;
            }
        }
    }
    false
}

fn candidate_steps(preferred_step: usize, delta: usize, path_len: usize) -> Vec<usize> {
    if delta == 0 {
        return vec![preferred_step.clamp(2, path_len.saturating_sub(1))];
    }

    let mut steps = Vec::with_capacity(2);
    if preferred_step > delta {
        steps.push((preferred_step - delta).clamp(2, path_len.saturating_sub(1)));
    }
    steps.push((preferred_step + delta).clamp(2, path_len.saturating_sub(1)));
    steps
}

fn can_insert_waypoint(grid: &Grid, path: &[usize], waypoints: &[Waypoint], step: usize) -> bool {
    if waypoints.iter().any(|waypoint| waypoint.step == step) {
        return false;
    }
    let pos = grid.row_col(path[step - 1]);
    waypoints
        .iter()
        .all(|waypoint| waypoint.step.abs_diff(step) >= 2 && chebyshev(pos, waypoint.pos) >= 2)
}

fn waypoint_candidate_score(
    grid: &Grid,
    path: &[usize],
    obstacles: &[u8],
    waypoints: &[Waypoint],
    step: usize,
) -> f64 {
    let pos = grid.row_col(path[step - 1]);
    let mut sorted = waypoints.to_vec();
    sorted.sort_by_key(|waypoint| waypoint.step);

    let min_cheb = sorted
        .iter()
        .map(|waypoint| chebyshev(pos, waypoint.pos))
        .min()
        .unwrap_or(0) as f64;
    let prev = sorted
        .iter()
        .rev()
        .find(|waypoint| waypoint.step < step)
        .unwrap_or(&sorted[0]);
    let next = sorted
        .iter()
        .find(|waypoint| waypoint.step > step)
        .unwrap_or_else(|| sorted.last().unwrap());
    let prev_manhattan = manhattan(pos, prev.pos);
    let next_manhattan = manhattan(pos, next.pos);
    let trickiness = (step - prev.step).saturating_sub(prev_manhattan)
        + (next.step - step).saturating_sub(next_manhattan);
    let bisection = (step - prev.step).min(next.step - step);
    let min_step_gap = sorted
        .iter()
        .map(|waypoint| waypoint.step.abs_diff(step))
        .min()
        .unwrap_or(0);
    let buffer = grid
        .neighbors(path[step - 1])
        .iter()
        .filter(|neighbor| obstacles[**neighbor] == 0)
        .count();

    trickiness as f64 * 1.5
        + bisection as f64 * 0.6
        + min_cheb * 0.3
        + buffer as f64 * 0.4
        + min_step_gap as f64 * 0.8
}

fn candidate_score(stats: &ComplexityStats) -> i64 {
    (stats.turn_ratio * 500.0
        + stats.obstacle_spread * 20.0
        + stats.tort_avg * 140.0
        + stats.tort_min * 100.0
        + (1.0 - stats.adjacent_obstacle_ratio).max(0.0) * 50.0
        - stats.waypoint_density * 450.0
        + stats.obstacle_ratio * 80.0) as i64
}

fn count_turns(path: &[usize], cols: usize) -> usize {
    path.windows(3)
        .filter(|cells| {
            let a = cells[0];
            let b = cells[1];
            let c = cells[2];
            (a / cols != b / cols || b / cols != c / cols)
                && (a % cols != b % cols || b % cols != c % cols)
        })
        .count()
}

fn complexity_stats(
    grid: &Grid,
    path: &[usize],
    obstacles: &[u8],
    waypoints: &[Waypoint],
) -> ComplexityStats {
    let obstacle_count = obstacles.iter().filter(|value| **value == 1).count();
    let adjacent_pairs = adjacent_obstacle_pairs(grid, obstacles);
    let (tort_avg, tort_min) = tortuosity_stats(waypoints);

    ComplexityStats {
        turn_ratio: count_turns(path, grid.cols) as f64 / path.len().max(1) as f64,
        obstacle_spread: obstacle_spread_average(grid, obstacles),
        tort_avg,
        tort_min,
        waypoint_density: waypoints.len() as f64 / path.len().max(1) as f64,
        adjacent_obstacle_ratio: adjacent_pairs as f64 / obstacle_count.max(1) as f64,
        obstacle_ratio: obstacle_count as f64 / obstacles.len().max(1) as f64,
        waypoint_count: waypoints.len(),
        size: grid.rows.max(grid.cols),
    }
}

fn adjacent_obstacle_pairs(grid: &Grid, obstacles: &[u8]) -> usize {
    obstacles
        .iter()
        .enumerate()
        .filter(|(_, value)| **value == 1)
        .map(|(idx, _)| {
            let [r, c] = grid.row_col(idx);
            usize::from(r + 1 < grid.rows && obstacles[grid.idx(r + 1, c)] == 1)
                + usize::from(c + 1 < grid.cols && obstacles[grid.idx(r, c + 1)] == 1)
        })
        .sum()
}

fn obstacle_spread_average(grid: &Grid, obstacles: &[u8]) -> f64 {
    let points = obstacles
        .iter()
        .enumerate()
        .filter_map(|(idx, value)| (*value == 1).then_some(grid.row_col(idx)))
        .collect::<Vec<_>>();
    if points.len() < 2 {
        return 0.0;
    }

    let mut sum = 0;
    let mut count = 0;
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            sum += chebyshev(points[i], points[j]);
            count += 1;
        }
    }
    sum as f64 / count as f64
}

fn tortuosity_stats(waypoints: &[Waypoint]) -> (f64, f64) {
    let mut sum = 0.0;
    let mut count = 0;
    let mut min_value = f64::INFINITY;

    for pair in waypoints.windows(2) {
        let step_gap = pair[1].step - pair[0].step;
        let distance = manhattan(pair[0].pos, pair[1].pos);
        if distance == 0 {
            continue;
        }
        let value = step_gap as f64 / distance as f64;
        sum += value;
        count += 1;
        min_value = min_value.min(value);
    }

    if count == 0 {
        (1.0, 1.0)
    } else {
        (sum / count as f64, min_value)
    }
}

fn manhattan(a: [usize; 2], b: [usize; 2]) -> usize {
    a[0].abs_diff(b[0]) + a[1].abs_diff(b[1])
}

fn chebyshev(a: [usize; 2], b: [usize; 2]) -> usize {
    a[0].abs_diff(b[0]).max(a[1].abs_diff(b[1]))
}

fn build_waypoints(grid: &Grid, path: &[usize], desired_count: usize) -> Vec<Waypoint> {
    let count = desired_count.max(2).min(path.len());
    let mut waypoints = vec![
        Waypoint {
            step: 1,
            pos: grid.row_col(path[0]),
        },
        Waypoint {
            step: path.len(),
            pos: grid.row_col(path[path.len() - 1]),
        },
    ];

    for i in 1..count.saturating_sub(1) {
        let step = 1 + (i * (path.len() - 1) / (count - 1));
        insert_waypoint_near_step(grid, path, &mut waypoints, step);
    }
    waypoints.sort_by_key(|waypoint| waypoint.step);
    waypoints
}
