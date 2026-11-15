//! The periodic (steady-mean) solve with an interior void agrees with time stepping to steady state.

use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Interfaces, InternalFace, Side};
use basetherm_grid::{Axis, Grid};
use basetherm_periodic::{solve_with, Amplitudes, Params};

#[test]
fn mean_solve_with_a_void_matches_stepping_to_steady_state_and_balances() {
    let ax = |n: usize, len: f64| Axis::uniform(0.0, len, n).unwrap();
    let grid = Grid::new(ax(7, 3.5), ax(6, 3.0), ax(8, 4.0));
    let [nx, ny, nz] = grid.dims();
    let n = grid.cell_count();
    let inside = |i: usize, j: usize, k: usize| (2..4).contains(&i) && (2..4).contains(&j) && k < 3;
    let mut void = vec![false; n];
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                void[grid.index(i, j, k)] = inside(i, j, k);
            }
        }
    }
    let mut ifc = Interfaces::new(void, 2);
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                if inside(i, j, k) {
                    continue;
                }
                let c = grid.index(i, j, k);
                let mut add = |side, group| {
                    ifc.add_face(InternalFace {
                        cell: c,
                        side,
                        h: 7.0,
                        group,
                    })
                };
                if k == 3 && inside(i, j, 2) {
                    add(Side::ZLo, 0);
                }
                if inside(i + 1, j, k) {
                    add(Side::XHi, 1);
                }
                if i > 0 && inside(i - 1, j, k) {
                    add(Side::XLo, 1);
                }
                if inside(i, j + 1, k) {
                    add(Side::YHi, 1);
                }
                if j > 0 && inside(i, j - 1, k) {
                    add(Side::YLo, 1);
                }
            }
        }
    }
    ifc.set_temperature(0, 22.0);
    ifc.set_temperature(1, 18.0);
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(
        Side::ZLo,
        Bc::Convective {
            h: 14.0,
            t_inf: 3.0,
        },
    );
    bc.fill(Side::ZHi, Bc::Fixed(10.0));
    let k: Vec<f64> = (0..n).map(|c| 0.9 + 0.1 * (c % 9) as f64).collect();
    let cv = vec![2.0e6; n];

    let sol = solve_with(
        &grid,
        &k,
        &cv,
        &bc,
        &Amplitudes::zeros(&grid),
        Some(&ifc),
        Params::annual(),
    )
    .unwrap();

    // Energy balance of the steady state: what leaves into the rooms and the outer boundaries is zero.
    let mut total: f64 = (0..2).map(|g| sol.group_flux(g)).sum();
    for side in Side::ALL {
        total += sol.boundary_flux(side).0.iter().sum::<f64>();
    }
    let scale: f64 = sol.group_flux(0).abs() + sol.group_flux(1).abs();
    assert!(
        total.abs() < 1e-7 * scale,
        "imbalance {total} against flows of {scale}"
    );

    // Time stepping (steps of about six days) converges to the same field. Much larger steps (58 days)
    // do not converge to the steady state within 400 steps, which is irrelevant for hourly or daily use.
    let mut adi = Adi::with_interfaces(grid.clone(), k, cv, ifc).unwrap();
    let mut t = vec![12.0; n];
    let mut flux = Fluxes::with_groups(&grid, 2);
    for _ in 0..2000 {
        adi.step(&mut t, 5.0e5, &bc, &mut flux).unwrap();
    }
    let worst = (0..n)
        .filter(|&c| !adi.interfaces().unwrap().is_void(c))
        .map(|c| (t[c] - sol.mean[c]).abs())
        .fold(0.0, f64::max);
    assert!(
        worst < 1e-8,
        "stepped steady state differs from the direct solve by {worst} K"
    );
    for g in 0..2 {
        let (a, b) = (flux.groups()[g], sol.group_flux(g));
        assert!(
            (a - b).abs() < 1e-8 * (1.0 + b.abs()),
            "group {g}: {a} vs {b}"
        );
    }
}
