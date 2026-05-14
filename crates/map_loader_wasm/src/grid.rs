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
        Self {
            rows,
            cols,
            neighbors,
        }
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
