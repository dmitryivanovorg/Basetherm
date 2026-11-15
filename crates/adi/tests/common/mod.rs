//! Helpers shared by the integration tests.
#![allow(dead_code)]

use basetherm_adi::Adi;
use basetherm_bc::{Boundaries, Fluxes};
use basetherm_grid::{Axis, GradingSpec, Grid};

/// A 1D column (x and y collapsed), z graded from 0 to `depth` with fine cells at z = 0.
pub fn column(depth: f64, h_min: f64, h_max: f64, ratio: f64) -> Grid {
    let z = Axis::graded(
        0.0,
        depth,
        &[0.0],
        GradingSpec {
            h_min,
            h_max,
            ratio,
        },
    )
    .unwrap();
    Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        z,
    )
}

pub fn uniform_props(grid: &Grid, k: f64, cv: f64) -> (Vec<f64>, Vec<f64>) {
    let n = grid.cell_count();
    (vec![k; n], vec![cv; n])
}

pub fn make(grid: Grid, k: f64, cv: f64) -> (Adi, Boundaries, Fluxes) {
    let (kv, cvv) = uniform_props(&grid, k, cv);
    let bc = Boundaries::adiabatic(&grid);
    let fl = Fluxes::zeros(&grid);
    (Adi::new(grid, kv, cvv).unwrap(), bc, fl)
}

/// Small deterministic generator for reproducible "random" fields.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}
