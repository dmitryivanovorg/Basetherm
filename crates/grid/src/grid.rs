//! Three axes make a grid. A 1D or 2D run uses one or two collapsed axes (a single cell).

use crate::axis::Axis;

#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub x: Axis,
    pub y: Axis,
    pub z: Axis,
}

impl Grid {
    pub fn new(x: Axis, y: Axis, z: Axis) -> Self {
        Self { x, y, z }
    }

    pub fn dims(&self) -> [usize; 3] {
        [self.x.len(), self.y.len(), self.z.len()]
    }

    pub fn cell_count(&self) -> usize {
        self.x.len() * self.y.len() * self.z.len()
    }

    /// Linear index with `z` varying fastest (vertical lines are contiguous for the z sweeps).
    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        debug_assert!(i < self.x.len() && j < self.y.len() && k < self.z.len());
        (i * self.y.len() + j) * self.z.len() + k
    }

    pub fn cell_volume(&self, i: usize, j: usize, k: usize) -> f64 {
        self.x.width(i) * self.y.width(j) * self.z.width(k)
    }

    /// Number of collapsed (single-cell) axes: 0 = 3D, 1 = 2D, 2 = 1D.
    pub fn collapsed_axes(&self) -> usize {
        self.dims().iter().filter(|&&n| n == 1).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        Grid::new(
            Axis::uniform(0.0, 2.0, 2).unwrap(),
            Axis::collapsed(1.0).unwrap(),
            Axis::uniform(0.0, 3.0, 3).unwrap(),
        )
    }

    #[test]
    fn counts_and_indexing() {
        let g = grid();
        assert_eq!(g.dims(), [2, 1, 3]);
        assert_eq!(g.cell_count(), 6);
        assert_eq!(g.index(0, 0, 0), 0);
        assert_eq!(g.index(0, 0, 2), 2);
        assert_eq!(g.index(1, 0, 0), 3);
        assert_eq!(g.index(1, 0, 2), 5);
        assert_eq!(g.collapsed_axes(), 1);
    }

    #[test]
    fn volumes_sum_to_domain() {
        let g = grid();
        let total: f64 = (0..2)
            .flat_map(|i| (0..3).map(move |k| (i, k)))
            .map(|(i, k)| g.cell_volume(i, 0, k))
            .sum();
        assert!((total - 6.0).abs() < 1e-12);
    }
}
