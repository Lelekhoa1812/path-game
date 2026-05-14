use map_loader_wasm::grid::Grid;
use map_loader_wasm::rng::Rng;
use map_loader_wasm::solver::{count_covering_paths_with_waypoints, is_valid_covering_path};
use map_loader_wasm::{generate_puzzle_for_test, GenerateRequest, PuzzleResponse};

fn request(seed: u64, sizes: Vec<usize>) -> GenerateRequest {
    request_with_budget(seed, 5_000, 10_000, sizes)
}

fn request_with_budget(
    seed: u64,
    target_ms: u32,
    max_ms: u32,
    sizes: Vec<usize>,
) -> GenerateRequest {
    GenerateRequest {
        seed,
        target_ms,
        max_ms,
        sizes,
        quality: "balanced".to_string(),
    }
}

fn solution_indices(puzzle: &PuzzleResponse) -> Vec<usize> {
    puzzle
        .solution
        .iter()
        .map(|cell| cell[0] * puzzle.c + cell[1])
        .collect()
}

fn interior_waypoint_count(puzzle: &PuzzleResponse) -> usize {
    puzzle
        .waypoints
        .iter()
        .filter(|waypoint| is_interior_waypoint_step(puzzle, waypoint.step))
        .count()
}

fn waypoint_constraints(puzzle: &PuzzleResponse) -> Vec<(usize, usize)> {
    puzzle
        .waypoints
        .iter()
        .map(|waypoint| (waypoint.step, waypoint.pos[0] * puzzle.c + waypoint.pos[1]))
        .collect()
}

fn waypoint_constrained_solution_count(puzzle: &PuzzleResponse) -> u32 {
    let grid = Grid::new(puzzle.r, puzzle.c);
    let constraints = waypoint_constraints(puzzle);
    count_covering_paths_with_waypoints(&grid, &puzzle.obstacles, &constraints, 2, 60_000).solutions
}

fn min_interior_waypoint_chebyshev(puzzle: &PuzzleResponse) -> usize {
    puzzle
        .waypoints
        .iter()
        .filter(|waypoint| is_interior_waypoint_step(puzzle, waypoint.step))
        .flat_map(|waypoint| {
            puzzle
                .waypoints
                .iter()
                .filter(move |other| other.step != waypoint.step)
                .map(move |other| chebyshev(waypoint.pos, other.pos))
        })
        .min()
        .unwrap_or(0)
}

fn is_interior_waypoint_step(puzzle: &PuzzleResponse, step: usize) -> bool {
    step != 1 && step != puzzle.solution.len()
}

fn waypoint_tortuosity_min(puzzle: &PuzzleResponse) -> f64 {
    puzzle
        .waypoints
        .windows(2)
        .filter_map(|pair| {
            let step_gap = pair[1].step - pair[0].step;
            let manhattan = manhattan(pair[0].pos, pair[1].pos);
            (manhattan > 0).then_some(step_gap as f64 / manhattan as f64)
        })
        .fold(f64::INFINITY, f64::min)
}

fn manhattan(a: [usize; 2], b: [usize; 2]) -> usize {
    a[0].abs_diff(b[0]) + a[1].abs_diff(b[1])
}

fn chebyshev(a: [usize; 2], b: [usize; 2]) -> usize {
    a[0].abs_diff(b[0]).max(a[1].abs_diff(b[1]))
}

#[test]
fn generator_returns_serializable_puzzle_contract() {
    let result = generate_puzzle_for_test(request(42, vec![9, 10, 11]));

    assert_eq!(result.r, 9);
    assert_eq!(result.r, result.c);
    assert_eq!(result.obstacles.len(), result.r * result.c);
    assert!(!result.solution.is_empty());
    assert!(result.waypoints.len() >= 2);
    assert_eq!(result.metrics.status, "success");
    assert_eq!(result.metrics.seed, 42);
    assert_eq!(result.metrics.size, 9);
    assert_eq!(result.metrics.quality, "balanced");
    assert!(result.metrics.quality_score > 0.0);
    assert_eq!(result.metrics.target_ms, 5_000);
    assert_eq!(result.metrics.max_ms, 10_000);
    assert!(result.metrics.total_ms <= 10_000.0);
    assert!(result.metrics.phase_timings.total_ms <= 10_000.0);
}

#[test]
fn generator_preserves_requested_size_first() {
    let result = generate_puzzle_for_test(request(7, vec![11, 10, 9]));

    assert_eq!(result.r, 11);
    assert_eq!(result.c, 11);
}

#[test]
fn generator_accepts_eight_by_eight_when_requested_first() {
    let result = generate_puzzle_for_test(request(8, vec![8, 9, 10, 11]));

    assert_eq!(result.r, 8);
    assert_eq!(result.c, 8);
    assert_eq!(result.metrics.size, 8);
}

#[test]
fn generator_reports_budget_metrics() {
    let puzzle = generate_puzzle_for_test(request(12, vec![11, 10, 9]));

    assert_eq!(puzzle.metrics.seed, 12);
    assert_eq!(puzzle.metrics.size, 11);
    assert_eq!(puzzle.metrics.quality, "balanced");
    assert!(puzzle.metrics.quality_score > 0.0);
    assert_eq!(puzzle.metrics.target_ms, 5_000);
    assert_eq!(puzzle.metrics.max_ms, 10_000);
    assert!(puzzle.metrics.candidate_attempts >= 1);
    assert!(!puzzle.metrics.cancelled);
    assert!(puzzle.metrics.solver_calls > 0);
    assert!(puzzle.metrics.unique_checks > 0);
    assert_eq!(puzzle.metrics.status, "success");
    assert!(puzzle.metrics.phase_timings.candidate_ms >= 0.0);
    assert!(puzzle.metrics.phase_timings.quality_ms >= 0.0);
    assert_eq!(
        puzzle.metrics.phase_timings.total_ms,
        puzzle.metrics.total_ms
    );
}

#[test]
fn generator_preserves_waypoint_constrained_uniqueness_without_fallback() {
    let puzzle = generate_puzzle_for_test(request(0, vec![9, 10, 11]));

    assert_eq!(waypoint_constrained_solution_count(&puzzle), 1);
    assert_eq!(puzzle.metrics.status, "success");
    assert!(!puzzle.metrics.fallback);
    assert_eq!(puzzle.metrics.degradation_level, 0);
    assert!(puzzle.metrics.solver_calls > 0);
    assert!(puzzle.metrics.unique_checks > 0);
}

#[test]
fn normal_budget_sizes_keep_complexity_waypoints_and_unique_solution() {
    for (size, expected_waypoints) in [(8, 4), (9, 6), (10, 8), (11, 10)] {
        let puzzle = generate_puzzle_for_test(request(100 + size as u64, vec![size]));

        assert_eq!(puzzle.r, size);
        assert!(!puzzle.metrics.fallback);
        assert!(
            puzzle.waypoints.len() >= expected_waypoints,
            "size {size} returned only {} waypoints",
            puzzle.waypoints.len()
        );
        assert_eq!(waypoint_constrained_solution_count(&puzzle), 1);
    }
}

#[test]
fn generated_waypoints_prefer_non_adjacent_interior_spacing() {
    let puzzle = generate_puzzle_for_test(request(2718, vec![10]));

    assert!(puzzle.waypoints.len() >= 8);
    assert_eq!(puzzle.waypoints.first().unwrap().step, 1);
    assert_eq!(puzzle.waypoints.last().unwrap().step, puzzle.solution.len());
    for pair in puzzle.waypoints.windows(2) {
        assert!(
            pair[1].step - pair[0].step >= 2,
            "adjacent waypoint steps {:?} and {:?}",
            pair[0],
            pair[1]
        );
    }
    assert!(
        min_interior_waypoint_chebyshev(&puzzle) >= 2,
        "interior waypoint hints should avoid touching existing hints spatially"
    );
    assert!(
        waypoint_tortuosity_min(&puzzle) >= 1.0,
        "waypoint legs should not be visually easier than shortest path"
    );
}

#[test]
fn tiny_budget_degrades_quality_before_size() {
    let puzzle = generate_puzzle_for_test(request_with_budget(12, 1, 1, vec![11, 10, 9]));

    assert_eq!(puzzle.r, 11);
    assert!(puzzle.metrics.degradation_level >= 1);
}

#[test]
fn generator_is_seed_deterministic() {
    let request = request(99, vec![10]);

    let a = generate_puzzle_for_test(request.clone());
    let b = generate_puzzle_for_test(request);

    assert_eq!(a.obstacles, b.obstacles);
    assert_eq!(a.solution, b.solution);
    assert_eq!(a.waypoints, b.waypoints);
    assert_eq!(a.difficulty, b.difficulty);
    assert_eq!(a.metrics.fallback, b.metrics.fallback);
    assert_eq!(a.metrics.degradation_level, b.metrics.degradation_level);
}

#[test]
fn generator_adds_obstacles_without_breaking_solution() {
    let puzzle = generate_puzzle_for_test(request(314, vec![9]));

    let obstacle_count = puzzle.obstacles.iter().filter(|value| **value == 1).count();
    assert!(obstacle_count >= 3);
    for cell in &puzzle.solution {
        let idx = cell[0] * puzzle.c + cell[1];
        assert_eq!(puzzle.obstacles[idx], 0);
    }

    let grid = Grid::new(puzzle.r, puzzle.c);
    let path = solution_indices(&puzzle);
    assert!(is_valid_covering_path(&grid, &puzzle.obstacles, &path));
}

#[test]
fn generator_adds_interior_waypoints_for_complexity() {
    let puzzle = generate_puzzle_for_test(request(2718, vec![10]));

    assert!(puzzle.waypoints.len() >= 4);
    assert_eq!(puzzle.waypoints.first().unwrap().step, 1);
    assert_eq!(puzzle.waypoints.last().unwrap().step, puzzle.solution.len());
    assert!(
        interior_waypoint_count(&puzzle) >= 4,
        "normal 10x10 generation should include multiple interior hints"
    );
}

#[test]
fn generated_solution_is_valid_covering_path_for_normal_complex_sizes() {
    for (size, seed) in [(9, 42), (10, 99), (11, 12)] {
        let puzzle = generate_puzzle_for_test(request(seed, vec![size]));
        let grid = Grid::new(puzzle.r, puzzle.c);
        let path = solution_indices(&puzzle);

        assert!(
            is_valid_covering_path(&grid, &puzzle.obstacles, &path),
            "{size}x{size} seed {seed} must publish a valid covering path"
        );
    }
}

#[test]
fn waypoints_prove_unique_covering_path_for_normal_complex_generation() {
    for (size, seed) in [(9, 42), (10, 99), (11, 12)] {
        let puzzle = generate_puzzle_for_test(request(seed, vec![size]));

        assert_eq!(
            waypoint_constrained_solution_count(&puzzle),
            1,
            "{size}x{size} seed {seed} must be unique after applying all waypoint constraints"
        );
    }
}

#[test]
fn normal_budget_complex_generation_avoids_fallback_for_representative_seeds() {
    for (size, seed) in [(9, 42), (10, 99), (11, 12)] {
        let puzzle = generate_puzzle_for_test(request(seed, vec![size]));

        assert!(
            !puzzle.metrics.fallback,
            "{size}x{size} seed {seed} should not use fallback under normal budget"
        );
        assert_eq!(
            puzzle.metrics.degradation_level, 0,
            "{size}x{size} seed {seed} should not degrade under normal budget"
        );
    }
}

#[test]
fn complex_normal_generation_reports_hard_difficulty_and_high_quality() {
    for (size, seed) in [(9, 42), (10, 99), (11, 12)] {
        let puzzle =
            generate_puzzle_for_test(request_with_budget(seed, 60_000, 120_000, vec![size]));

        assert!(
            matches!(puzzle.difficulty.as_str(), "Hard" | "Expert"),
            "{size}x{size} seed {seed} should report Hard or Expert difficulty"
        );
        assert!(
            puzzle.metrics.quality_score >= 0.9,
            "{size}x{size} seed {seed} should keep high quality metrics"
        );
        assert!(
            puzzle.metrics.quality_score < 1.0 || puzzle.waypoints.len() >= 10,
            "quality score should come from puzzle complexity, not a constant pass flag"
        );
    }
}

#[test]
fn tight_normal_budget_degrades_instead_of_ignoring_max_ms() {
    let puzzle = generate_puzzle_for_test(request_with_budget(12, 10, 10, vec![11, 10, 9]));

    assert_eq!(puzzle.r, 11);
    assert!(puzzle.metrics.degradation_level >= 1);
    assert!(puzzle.metrics.candidate_attempts <= 2);
}

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

#[test]
fn request_accepts_js_camel_case_budget_fields() {
    let request: GenerateRequest = serde_json::from_value(serde_json::json!({
        "seed": 55,
        "targetMs": 5000,
        "maxMs": 10000,
        "sizes": [9, 10, 11],
        "quality": "balanced"
    }))
    .unwrap();

    assert_eq!(request.target_ms, 5_000);
    assert_eq!(request.max_ms, 10_000);
}

#[test]
fn response_serializes_js_uppercase_dimensions() {
    let puzzle = generate_puzzle_for_test(request(55, vec![9]));
    let value = serde_json::to_value(&puzzle).unwrap();

    assert_eq!(value["R"], 9);
    assert_eq!(value["C"], 9);
    assert!(value.get("r").is_none());
    assert!(value.get("c").is_none());
}

#[test]
fn response_keeps_rust_dimension_deserialization_ergonomics() {
    let puzzle = generate_puzzle_for_test(request(55, vec![9]));
    let value = serde_json::to_value(&puzzle).unwrap();
    let round_trip: PuzzleResponse = serde_json::from_value(value).unwrap();

    assert_eq!(round_trip.r, puzzle.r);
    assert_eq!(round_trip.c, puzzle.c);
}

#[test]
fn response_serializes_browser_facing_metrics_contract() {
    let puzzle = generate_puzzle_for_test(request(55, vec![9]));
    let value = serde_json::to_value(&puzzle).unwrap();
    let metrics = &value["metrics"];

    assert_eq!(metrics["seed"], 55);
    assert_eq!(metrics["size"], 9);
    assert_eq!(metrics["quality"], "balanced");
    assert!(metrics["qualityScore"].as_f64().unwrap() > 0.0);
    assert!(metrics["phaseTimings"]["candidateMs"].as_f64().unwrap() >= 0.0);
    assert!(metrics["phaseTimings"]["qualityMs"].as_f64().unwrap() >= 0.0);
    assert_eq!(metrics["phaseTimings"]["totalMs"], metrics["totalMs"]);
    assert_eq!(metrics["targetMs"], 5_000);
    assert_eq!(metrics["maxMs"], 10_000);
    assert!(metrics["degradationLevel"].is_number());
    assert!(metrics["candidateAttempts"].as_u64().unwrap() >= 1);
    assert!(metrics["solverCalls"].as_u64().unwrap() > 0);
    assert!(metrics["uniqueChecks"].as_u64().unwrap() > 0);
    assert!(metrics["cancelled"].is_boolean());
    assert!(metrics["fallback"].is_boolean());
    assert!(metrics.get("target_ms").is_none());
    assert!(metrics.get("max_ms").is_none());
    assert!(metrics.get("degradation_level").is_none());
    assert!(metrics.get("candidate_attempts").is_none());
    assert!(metrics.get("solver_calls").is_none());
    assert!(metrics.get("unique_checks").is_none());
}

#[test]
fn tiny_budget_uses_bounded_fallback_with_metrics() {
    let puzzle = generate_puzzle_for_test(request_with_budget(12, 0, 0, vec![11, 10, 9]));

    assert_eq!(puzzle.r, 11);
    assert!(puzzle.metrics.degradation_level >= 1);
    assert!(puzzle.metrics.candidate_attempts >= 1);
    assert!(puzzle.metrics.solver_calls > 0);
    assert!(puzzle.metrics.unique_checks > 0);
    assert!(puzzle.metrics.fallback);
}
