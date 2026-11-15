//! The direct periodic solve must agree with a time-stepped (ADI) run that has reached its
//! periodic state. Both share the same spatial discretisation, so the difference is time error
//! only: it must be small and fall first-order with dt.

use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_grid::{Axis, GradingSpec, Grid};
use basetherm_mg::C64;
use basetherm_periodic::{solve, Amplitudes, Params};

const YEAR: f64 = 365.0 * 86400.0;

/// Boundary values at time `t`: mean + Re(amp e^{i w t}).
fn bc_at(mean: &Boundaries, f: &Amplitudes, omega: f64, t: f64) -> Boundaries {
    let mut b = mean.clone();
    for side in Side::ALL {
        for (idx, a) in f.side(side).iter().enumerate() {
            let swing = a.re_at(omega * t);
            match mean.get(side, idx) {
                Bc::Adiabatic => {}
                Bc::Fixed(t0) => b.set(side, idx, Bc::Fixed(t0 + swing)),
                Bc::Convective { h, t_inf } => b.set(
                    side,
                    idx,
                    Bc::Convective {
                        h,
                        t_inf: t_inf + swing,
                    },
                ),
            }
        }
    }
    b
}

/// Spin up with ADI for `years`, then return max |T_adi - T_periodic| at the final instant.
fn max_difference(
    grid: &Grid,
    k: &[f64],
    cv: &[f64],
    mean: &Boundaries,
    f: &Amplitudes,
    dt: f64,
    years: f64,
) -> f64 {
    let p = Params::annual();
    let sol = solve(grid, k, cv, mean, f, p).unwrap();
    let mut adi = Adi::new(grid.clone(), k.to_vec(), cv.to_vec()).unwrap();
    let mut flux = Fluxes::zeros(grid);
    let mut t = sol.mean.clone(); // start at the mean field: the transient is the harmonic part
    let steps = (years * YEAR / dt).round() as usize;
    for n in 1..=steps {
        let b = bc_at(mean, f, p.omega, n as f64 * dt);
        adi.step(&mut t, dt, &b, &mut flux).unwrap();
    }
    let tend = steps as f64 * dt;
    (0..t.len())
        .map(|c| (t[c] - sol.temperature(c, tend)).abs())
        .fold(0.0, f64::max)
}

fn check_first_order(e_coarse: f64, e_fine: f64, abs_limit: f64) {
    println!(
        "max |ADI - periodic|: coarse {e_coarse:.5} K, fine {e_fine:.5} K, ratio {:.2}",
        e_coarse / e_fine
    );
    assert!(e_coarse < abs_limit, "coarse difference {e_coarse}");
    let ratio = e_coarse / e_fine;
    assert!(
        (1.6..2.5).contains(&ratio),
        "error ratio {ratio:.2} (coarse {e_coarse:.5}, fine {e_fine:.5})"
    );
}

#[test]
fn layered_column_matches_time_stepping() {
    let z = Axis::graded(
        0.0,
        6.0,
        &[0.0, 1.0],
        GradingSpec {
            h_min: 0.03,
            h_max: 0.5,
            ratio: 1.2,
        },
    )
    .unwrap();
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        z,
    );
    let n = grid.cell_count();
    let (mut k, mut cv) = (vec![2.0; n], vec![2.4e6; n]);
    for i in 0..n {
        if grid.z.center(i) < 1.0 {
            k[i] = 0.6;
            cv[i] = 1.6e6;
        }
    }
    let mut mean = Boundaries::adiabatic(&grid);
    mean.fill(
        Side::ZLo,
        Bc::Convective {
            h: 10.0,
            t_inf: 5.0,
        },
    );
    mean.fill(Side::ZHi, Bc::Fixed(5.0));
    let mut f = Amplitudes::zeros(&grid);
    f.fill(
        Side::ZLo,
        C64::new(8.0 * 0.3_f64.cos(), 8.0 * 0.3_f64.sin()),
    );
    let e1 = max_difference(&grid, &k, &cv, &mean, &f, 6.0 * 3600.0, 10.0);
    let e2 = max_difference(&grid, &k, &cv, &mean, &f, 3.0 * 3600.0, 10.0);
    check_first_order(e1, e2, 0.25);
}

#[test]
fn heterogeneous_3d_matches_time_stepping() {
    let ax = |n| Axis::uniform(0.0, 5.0, n).unwrap();
    let z = Axis::graded(
        0.0,
        5.0,
        &[0.0],
        GradingSpec {
            h_min: 0.2,
            h_max: 1.0,
            ratio: 1.4,
        },
    )
    .unwrap();
    let grid = Grid::new(ax(6), ax(5), z);
    let n = grid.cell_count();
    let k: Vec<f64> = (0..n).map(|i| 0.7 + 0.15 * ((i * 7) % 11) as f64).collect();
    let cv = vec![2.0e6; n];
    let mut mean = Boundaries::adiabatic(&grid);
    let mut f = Amplitudes::zeros(&grid);
    let [_, ny, _] = grid.dims();
    mean.fill(
        Side::ZLo,
        Bc::Convective {
            h: 12.0,
            t_inf: 8.0,
        },
    );
    mean.fill(Side::ZHi, Bc::Fixed(8.0));
    mean.fill(Side::XLo, Bc::Fixed(8.0));
    for idx in 0..mean.side(Side::ZLo).len() {
        let ix = idx / ny;
        f.set(Side::ZLo, idx, C64::new(6.0 * (1.0 + 0.2 * ix as f64), 2.0));
    }
    f.fill(Side::XLo, C64::new(1.5, -0.5));
    let e1 = max_difference(&grid, &k, &cv, &mean, &f, 86400.0, 8.0);
    let e2 = max_difference(&grid, &k, &cv, &mean, &f, 43200.0, 8.0);
    check_first_order(e1, e2, 0.25);
}
