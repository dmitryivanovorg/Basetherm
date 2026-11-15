//! 3D decay of a sine mode: u = sin(pi x) sin(pi y) sin(pi z) exp(-3 pi^2 alpha t) on the unit cube
//! with u = 0 on all faces. Exercises the splitting in all three directions at once.

mod common;
use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_grid::{Axis, Grid};
use std::f64::consts::PI;

/// Relative max error at t = 0.05 for `steps` implicit-Euler steps (k = cv = 1, so alpha = 1).
fn run(n: usize, steps: usize) -> f64 {
    let ax = || Axis::uniform(0.0, 1.0, n).unwrap();
    let grid = Grid::new(ax(), ax(), ax());
    let (mut adi, _, mut fl) = common::make(grid.clone(), 1.0, 1.0);
    let mut bc = Boundaries::adiabatic(&grid);
    for s in Side::ALL {
        bc.fill(s, Bc::Fixed(0.0));
    }
    let mode = |i: usize, j: usize, k: usize| {
        (PI * grid.x.center(i)).sin()
            * (PI * grid.y.center(j)).sin()
            * (PI * grid.z.center(k)).sin()
    };
    let mut t = vec![0.0; grid.cell_count()];
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                t[grid.index(i, j, k)] = mode(i, j, k);
            }
        }
    }
    let tend = 0.05;
    let dt = tend / steps as f64;
    for _ in 0..steps {
        adi.step(&mut t, dt, &bc, &mut fl).unwrap();
    }
    let decay = (-3.0 * PI * PI * tend).exp();
    let mut worst: f64 = 0.0;
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let e = (t[grid.index(i, j, k)] - decay * mode(i, j, k)).abs();
                worst = worst.max(e);
            }
        }
    }
    let _: &Fluxes = &fl;
    worst / decay
}

#[test]
fn three_d_mode_decays_at_the_right_rate() {
    let e = run(24, 100);
    assert!(e < 0.02, "relative error {e:.4}");
}

#[test]
fn time_error_is_first_order_in_3d() {
    // Fine space so that the time error dominates.
    let (e1, e2) = (run(20, 25), run(20, 50));
    let order = (e1 / e2).log2();
    assert!(
        (0.7..1.4).contains(&order),
        "order {order:.2} (e1={e1:.4}, e2={e2:.4})"
    );
}
