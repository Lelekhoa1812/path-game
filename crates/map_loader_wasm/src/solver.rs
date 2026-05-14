use crate::grid::Grid;

type NormalizedWaypoints = (Vec<Option<usize>>, Vec<Option<usize>>, usize);

pub fn is_valid_covering_path(grid: &Grid, obstacles: &[u8], path: &[usize]) -> bool {
    if obstacles.len() != grid.rows * grid.cols {
        return false;
    }

    let playable_cell_count = obstacles.iter().filter(|value| **value == 0).count();
    if path.len() != playable_cell_count {
        return false;
    }

    let mut seen = vec![false; grid.rows * grid.cols];
    for (path_offset, cell_idx) in path.iter().enumerate() {
        if *cell_idx >= seen.len() || obstacles[*cell_idx] != 0 || seen[*cell_idx] {
            return false;
        }
        if path_offset > 0 && !grid.neighbors(path[path_offset - 1]).contains(cell_idx) {
            return false;
        }
        seen[*cell_idx] = true;
    }
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PathCount {
    pub solutions: u32,
    pub solver_calls: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolutionSearchResult {
    pub solutions: Vec<Vec<usize>>,
    pub solver_calls: u32,
    pub exhausted: bool,
}

impl SolutionSearchResult {
    fn empty() -> Self {
        Self {
            solutions: Vec::new(),
            solver_calls: 0,
            exhausted: false,
        }
    }
}

pub fn count_covering_paths(
    grid: &Grid,
    obstacles: &[u8],
    start: usize,
    end: usize,
    solution_cap: u32,
    call_cap: u32,
) -> PathCount {
    if obstacles.len() != grid.rows * grid.cols {
        return PathCount {
            solutions: 0,
            solver_calls: 0,
        };
    }

    let open_count = obstacles.iter().filter(|value| **value == 0).count();
    count_covering_paths_with_waypoints(
        grid,
        obstacles,
        &[(1, start), (open_count, end)],
        solution_cap,
        call_cap,
    )
}

pub fn count_covering_paths_with_waypoints(
    grid: &Grid,
    obstacles: &[u8],
    waypoints: &[(usize, usize)],
    solution_cap: u32,
    call_cap: u32,
) -> PathCount {
    let result =
        find_up_to_solutions_with_waypoints(grid, obstacles, waypoints, solution_cap, call_cap);

    PathCount {
        solutions: if result.exhausted {
            solution_cap.max(1)
        } else {
            result.solutions.len() as u32
        },
        solver_calls: result.solver_calls,
    }
}

pub fn find_up_to_solutions_with_waypoints(
    grid: &Grid,
    obstacles: &[u8],
    waypoints: &[(usize, usize)],
    solution_cap: u32,
    call_cap: u32,
) -> SolutionSearchResult {
    if obstacles.len() != grid.rows * grid.cols {
        return SolutionSearchResult::empty();
    }

    let open_count = obstacles.iter().filter(|value| **value == 0).count();
    let Some((waypoint_at_step, reserved_step_by_cell, start)) =
        normalize_waypoints(obstacles, waypoints, open_count)
    else {
        return SolutionSearchResult::empty();
    };

    let mut search = PathSearch {
        grid,
        obstacles,
        visited: vec![false; obstacles.len()],
        open_count,
        solution_cap: solution_cap.max(1),
        call_cap: call_cap.max(1),
        waypoint_at_step,
        reserved_step_by_cell,
        current_path: vec![start],
        solutions: Vec::new(),
        solver_calls: 0,
        exhausted: false,
    };
    search.visited[start] = true;
    search.visit(start, 1);

    SolutionSearchResult {
        solutions: search.solutions,
        solver_calls: search.solver_calls,
        exhausted: search.exhausted,
    }
}

fn normalize_waypoints(
    obstacles: &[u8],
    waypoints: &[(usize, usize)],
    open_count: usize,
) -> Option<NormalizedWaypoints> {
    if open_count == 0 || waypoints.is_empty() {
        return None;
    }

    let mut waypoint_at_step = vec![None; open_count + 1];
    let mut reserved_step_by_cell = vec![None; obstacles.len()];
    for (step, cell) in waypoints {
        if *step == 0 || *step > open_count || *cell >= obstacles.len() || obstacles[*cell] != 0 {
            return None;
        }
        if waypoint_at_step[*step].is_some_and(|existing_cell| existing_cell != *cell) {
            return None;
        }
        if reserved_step_by_cell[*cell].is_some_and(|existing_step| existing_step != *step) {
            return None;
        }
        waypoint_at_step[*step] = Some(*cell);
        reserved_step_by_cell[*cell] = Some(*step);
    }

    waypoint_at_step[1].map(|start| (waypoint_at_step, reserved_step_by_cell, start))
}

fn future_reserved_cell_is_early(
    reserved_step_by_cell: &[Option<usize>],
    next: usize,
    next_step: usize,
) -> bool {
    reserved_step_by_cell[next].is_some_and(|reserved_step| reserved_step != next_step)
}

struct PathSearch<'a> {
    grid: &'a Grid,
    obstacles: &'a [u8],
    visited: Vec<bool>,
    open_count: usize,
    solution_cap: u32,
    call_cap: u32,
    waypoint_at_step: Vec<Option<usize>>,
    reserved_step_by_cell: Vec<Option<usize>>,
    current_path: Vec<usize>,
    solutions: Vec<Vec<usize>>,
    solver_calls: u32,
    exhausted: bool,
}

impl PathSearch<'_> {
    fn visit(&mut self, current: usize, visited_count: usize) {
        if self.solutions.len() as u32 >= self.solution_cap || self.exhausted {
            return;
        }
        self.solver_calls += 1;
        if self.solver_calls >= self.call_cap {
            self.exhausted = true;
            return;
        }

        if self.waypoint_at_step[visited_count]
            .map(|expected| expected != current)
            .unwrap_or(false)
        {
            return;
        }

        if visited_count == self.open_count {
            self.solutions.push(self.current_path.clone());
            return;
        }

        let next_step = visited_count + 1;
        let mut neighbors = self.available_neighbors(current, next_step);
        neighbors.sort_by_key(|idx| self.onward_count(*idx));

        for next in neighbors {
            self.visited[next] = true;
            self.current_path.push(next);
            self.visit(next, visited_count + 1);
            self.current_path.pop();
            self.visited[next] = false;
        }
    }

    fn available_neighbors(&self, idx: usize, next_step: usize) -> Vec<usize> {
        if let Some(required_next) = self.waypoint_at_step[next_step] {
            return if self.grid.neighbors(idx).contains(&required_next)
                && self.can_visit(required_next, next_step)
            {
                vec![required_next]
            } else {
                Vec::new()
            };
        }

        self.grid
            .neighbors(idx)
            .iter()
            .copied()
            .filter(|next| self.can_visit(*next, next_step))
            .collect()
    }

    fn can_visit(&self, next: usize, next_step: usize) -> bool {
        self.obstacles[next] == 0
            && !self.visited[next]
            && !future_reserved_cell_is_early(&self.reserved_step_by_cell, next, next_step)
    }

    fn onward_count(&self, idx: usize) -> usize {
        self.grid
            .neighbors(idx)
            .iter()
            .filter(|next| self.obstacles[**next] == 0 && !self.visited[**next])
            .count()
    }
}
