//! Periodic solve against closed-form solutions and exact balances.

use basetherm_bc::{Bc, Boundaries, Side};
use basetherm_grid::{Axis, GradingSpec, Grid};
use basetherm_mg::C64;
use basetherm_periodic::{solve, Amplitudes, Error, Params};

const K: f64 = 1.9;
const CV: f64 = 1490.0 * 1800.0;

fn column(depth: f64, h_min: f64) -> Grid {
    let z = Axis::graded(
        0.0,
        depth,
        &[0.0],
        GradingSpec {
            h_min,
            h_max: 1.0,
            ratio: 1.15,
        },
    )
    .unwrap();
    Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        z,
    )
}

fn uniform(grid: &Grid) -> (Vec<f64>, Vec<f64>) {
    let n = grid.cell_count();
    (vec![K; n], vec![CV; n])
}

#[test]
fn half_space_amplitude_and_phase() {
    // Surface T = A sin(w t)  =  Re(-i A e^{i w t});  theta(z) = -iA exp(-(1+i) z / d).
    let amp = 10.0;
    let grid = column(40.0, 0.02);
    let (k, cv) = uniform(&grid);
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(Side::ZLo, Bc::Fixed(0.0));
    bc.fill(Side::ZHi, Bc::Fixed(0.0));
    let mut f = Amplitudes::zeros(&grid);
    f.fill(Side::ZLo, C64::new(0.0, -amp));
    let p = Params::annual();
    let sol = solve(&grid, &k, &cv, &bc, &f, p).unwrap();
    let d = (2.0 * (K / CV) / p.omega).sqrt();
    let mut worst: f64 = 0.0;
    for i in 0..grid.z.len() {
        let z = grid.z.center(i);
        let decay = (-z / d).exp();
        let exact = C64::new(0.0, -amp) * C64::new(decay * (z / d).cos(), -decay * (z / d).sin());
        worst = worst.max((sol.amp[i] - exact).abs());
    }
    assert!(worst < 0.005 * amp, "worst amplitude error {worst}");
    assert!(sol.mean.iter().all(|m| m.abs() < 1e-8));
}

#[test]
fn mean_is_linear_between_fixed_faces() {
    let len = 3.0;
    let z = Axis::uniform(0.0, len, 12).unwrap();
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        z,
    );
    let (k, cv) = uniform(&grid);
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(Side::ZLo, Bc::Fixed(20.0));
    bc.fill(Side::ZHi, Bc::Fixed(0.0));
    let sol = solve(
        &grid,
        &k,
        &cv,
        &bc,
        &Amplitudes::zeros(&grid),
        Params::annual(),
    )
    .unwrap();
    for i in 0..12 {
        let exact = 20.0 * (1.0 - grid.z.center(i) / len);
        assert!((sol.mean[i] - exact).abs() < 1e-8, "cell {i}");
    }
    // Steady heat flow k dT/L through unit area, entering at the low face.
    let (m, _) = sol.boundary_flux(Side::ZHi);
    assert!((m[0] - K * 20.0 / len).abs() < 1e-8);
}

#[test]
fn flux_balances_hold_in_3d() {
    // Mean: heat in = heat out. Harmonic: sum of outflow amplitudes = -i w * sum(C theta).
    let ax = |n| Axis::uniform(0.0, 4.0, n).unwrap();
    let z = Axis::graded(
        0.0,
        5.0,
        &[0.0],
        GradingSpec {
            h_min: 0.05,
            h_max: 0.8,
            ratio: 1.3,
        },
    )
    .unwrap();
    let grid = Grid::new(ax(7), ax(5), z);
    let n = grid.cell_count();
    let k: Vec<f64> = (0..n).map(|i| 0.8 + 0.1 * (i % 7) as f64).collect();
    let cv: Vec<f64> = (0..n).map(|i| 1.5e6 + 1.0e5 * (i % 5) as f64).collect();
    let mut bc = Boundaries::adiabatic(&grid);
    let mut f = Amplitudes::zeros(&grid);
    bc.fill(Side::ZLo, Bc::Convective { h: 9.0, t_inf: 8.0 });
    bc.fill(Side::ZHi, Bc::Fixed(8.0));
    for (idx, _) in bc.side(Side::ZLo).to_vec().iter().enumerate() {
        f.set(Side::ZLo, idx, C64::new(6.0 + 0.2 * idx as f64, -2.0));
    }
    let sol = solve(&grid, &k, &cv, &bc, &f, Params::annual()).unwrap();
    let (mut mean_sum, mut amp_sum) = (0.0, C64::new(0.0, 0.0));
    for side in Side::ALL {
        let (m, a) = sol.boundary_flux(side);
        mean_sum += m.iter().sum::<f64>();
        for v in a {
            amp_sum += v;
        }
    }
    assert!(mean_sum.abs() < 1e-7, "mean imbalance {mean_sum}");
    let mut stored = C64::new(0.0, 0.0);
    for (c, th) in sol.capacity().iter().zip(&sol.amp) {
        stored += *th * *c;
    }
    let expected = C64::new(0.0, -sol.omega) * stored;
    assert!(
        (amp_sum - expected).abs() < 1e-6 * expected.abs().max(1.0),
        "{amp_sum:?} vs {expected:?}"
    );
}

#[test]
fn preconditioner_keeps_iterations_low_on_anisotropic_grid() {
    // 5 cm vertical cells against 1 m horizontal cells.
    let ax = |n| Axis::uniform(0.0, 20.0, n).unwrap();
    let z = Axis::graded(
        0.0,
        20.0,
        &[0.0, 3.0],
        GradingSpec {
            h_min: 0.05,
            h_max: 2.0,
            ratio: 1.25,
        },
    )
    .unwrap();
    let grid = Grid::new(ax(20), ax(20), z);
    let (k, cv) = uniform(&grid);
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(
        Side::ZLo,
        Bc::Convective {
            h: 10.0,
            t_inf: 10.0,
        },
    );
    bc.fill(Side::ZHi, Bc::Fixed(10.0));
    let mut f = Amplitudes::zeros(&grid);
    f.fill(Side::ZLo, C64::new(8.0, 0.0));
    let sol = solve(&grid, &k, &cv, &bc, &f, Params::annual()).unwrap();
    assert!(sol.mean_outcome.iterations < 100, "{:?}", sol.mean_outcome);
    assert!(
        sol.harmonic_outcome.iterations < 100,
        "{:?}",
        sol.harmonic_outcome
    );
}

#[test]
fn all_adiabatic_is_rejected() {
    let grid = column(2.0, 0.1);
    let (k, cv) = uniform(&grid);
    let bc = Boundaries::adiabatic(&grid);
    let r = solve(
        &grid,
        &k,
        &cv,
        &bc,
        &Amplitudes::zeros(&grid),
        Params::annual(),
    );
    assert_eq!(r.unwrap_err(), Error::NotAnchored);
}

#[test]
fn multigrid_and_zline_give_the_same_answer() {
    use basetherm_periodic::PrecondKind;
    let ax = |n| Axis::uniform(0.0, 20.0, n).unwrap();
    let z = Axis::graded(
        0.0,
        20.0,
        &[0.0, 3.0],
        GradingSpec {
            h_min: 0.05,
            h_max: 2.0,
            ratio: 1.25,
        },
    )
    .unwrap();
    let grid = Grid::new(ax(20), ax(20), z);
    let (k, cv) = uniform(&grid);
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(
        Side::ZLo,
        Bc::Convective {
            h: 10.0,
            t_inf: 10.0,
        },
    );
    bc.fill(Side::ZHi, Bc::Fixed(10.0));
    let mut f = Amplitudes::zeros(&grid);
    f.fill(Side::ZLo, C64::new(8.0, 3.0));
    let run = |kind| {
        let p = Params {
            precond: kind,
            ..Params::annual()
        };
        solve(&grid, &k, &cv, &bc, &f, p).unwrap()
    };
    let (mg, zl) = (run(Params::annual().precond), run(PrecondKind::ZLine));
    assert!(
        mg.mean_outcome.iterations * 3 < zl.mean_outcome.iterations,
        "{:?} vs {:?}",
        mg.mean_outcome,
        zl.mean_outcome
    );
    assert!(mg.harmonic_outcome.iterations * 3 < zl.harmonic_outcome.iterations);
    let worst = mg
        .mean
        .iter()
        .zip(&zl.mean)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let worst_h = mg
        .amp
        .iter()
        .zip(&zl.amp)
        .map(|(a, b)| (*a - *b).abs())
        .fold(0.0, f64::max);
    println!(
        "max difference: mean {worst:.2e} K, harmonic {worst_h:.2e} K; residuals {:.1e}/{:.1e}",
        mg.mean_outcome.residual, zl.mean_outcome.residual
    );
    // A relative residual of 1e-10 leaves temperature errors of order 1e-7 K; allow 1e-6 K.
    assert!(worst < 1e-6 && worst_h < 1e-6);
    for (a, b) in mg.mean.iter().zip(&zl.mean) {
        assert!((a - b).abs() < 1e-6);
    }
    for (a, b) in mg.amp.iter().zip(&zl.amp) {
        assert!((*a - *b).abs() < 1e-6);
    }
}
