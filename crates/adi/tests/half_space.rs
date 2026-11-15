//! 1D semi-infinite solid with a sinusoidal surface temperature (analytic, CLAUDE.md validation).
//! T(z,t) = A exp(-z/d) sin(w t - z/d),  d = sqrt(2 alpha / w).

mod common;
use basetherm_bc::{Bc, Side};
use std::f64::consts::PI;

const K: f64 = 1.9;
const CV: f64 = 1490.0 * 1800.0;
const YEAR: f64 = 365.0 * 86400.0;

fn exact(z: f64, t: f64, amp: f64, w: f64) -> f64 {
    let d = (2.0 * (K / CV) / w).sqrt();
    amp * (-z / d).exp() * (w * t - z / d).sin()
}

/// Run `years` annual cycles from the analytic state at t = 0; return max error / amplitude.
fn run(dt: f64, years: f64, h_min: f64) -> f64 {
    let (amp, w) = (10.0, 2.0 * PI / YEAR);
    let grid = common::column(40.0, h_min, 1.0, 1.15);
    let zc: Vec<f64> = (0..grid.z.len()).map(|i| grid.z.center(i)).collect();
    let (mut adi, mut bc, mut fl) = common::make(grid, K, CV);
    bc.fill(Side::ZHi, Bc::Fixed(0.0));
    let mut t: Vec<f64> = zc.iter().map(|&z| exact(z, 0.0, amp, w)).collect();
    let steps = (years * YEAR / dt).round() as usize;
    for n in 1..=steps {
        bc.fill(Side::ZLo, Bc::Fixed(amp * (w * n as f64 * dt).sin()));
        adi.step(&mut t, dt, &bc, &mut fl).unwrap();
    }
    let tend = steps as f64 * dt;
    zc.iter()
        .zip(&t)
        .map(|(&z, &x)| (x - exact(z, tend, amp, w)).abs())
        .fold(0.0, f64::max)
        / amp
}

#[test]
fn matches_analytic_solution_after_one_year() {
    let err = run(3.0 * 3600.0, 1.0, 0.02);
    assert!(err < 0.005, "max error {err:.5} of amplitude");
}

#[test]
fn error_falls_with_time_step_first_order() {
    // Implicit Euler: halving dt should roughly halve the (time-dominated) error.
    let e1 = run(24.0 * 3600.0, 1.0, 0.02);
    let e2 = run(12.0 * 3600.0, 1.0, 0.02);
    let order = (e1 / e2).log2();
    assert!(
        (0.8..1.3).contains(&order),
        "observed order {order:.2} (e1={e1:.5}, e2={e2:.5})"
    );
}
