//! The threaded stepper must give bitwise identical results for any number of threads.
#![cfg(feature = "parallel")]

use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Interfaces, InternalFace, Side};
use basetherm_grid::{Axis, Grid};

/// 3D block with a void cell block, interior-face groups, random properties and mixed boundaries;
/// returns all temperatures and fluxes after 12 steps.
fn run() -> Vec<f64> {
    let ax = |n: usize, len: f64| Axis::uniform(0.0, len, n).unwrap();
    let grid = Grid::new(ax(13, 6.0), ax(11, 5.0), ax(17, 7.0));
    let [nx, ny, nz] = grid.dims();
    let n = grid.cell_count();
    let inside = |i: usize, j: usize, k: usize| (3..7).contains(&i) && (3..6).contains(&j) && k < 6;
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
                if k == 6 && inside(i, j, 5) {
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
    ifc.set_temperature(0, 21.0);
    ifc.set_temperature(1, 18.0);
    let mut s = 5u64;
    let mut next = move || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((s >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let k: Vec<f64> = (0..n).map(|_| 0.4 + 2.0 * next()).collect();
    let cv: Vec<f64> = (0..n).map(|_| 1.0e6 + 2.0e6 * next()).collect();
    let mut t: Vec<f64> = (0..n).map(|_| 4.0 + 12.0 * next()).collect();
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(
        Side::ZLo,
        Bc::Convective {
            h: 15.0,
            t_inf: -3.0,
        },
    );
    bc.fill(Side::ZHi, Bc::Fixed(9.0));
    bc.fill(Side::XHi, Bc::Convective { h: 4.0, t_inf: 5.0 });
    bc.fill(Side::YLo, Bc::Fixed(7.0));
    let mut adi = Adi::with_interfaces(grid.clone(), k, cv, ifc).unwrap();
    let mut flux = Fluxes::with_groups(&grid, 2);
    let mut all = Vec::new();
    for _ in 0..12 {
        adi.step(&mut t, 3600.0, &bc, &mut flux).unwrap();
        all.extend(flux.groups());
        for side in Side::ALL {
            all.extend(flux.side(side));
        }
    }
    all.extend(&t);
    all
}

#[test]
fn results_are_bitwise_identical_for_any_thread_count() {
    let in_pool = |threads: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(run)
    };
    let reference = in_pool(1);
    assert!(reference.iter().all(|x| x.is_finite()));
    for threads in [2, 3, 5, 12] {
        let other = in_pool(threads);
        let same = reference
            .iter()
            .zip(&other)
            .all(|(a, b)| a.to_bits() == b.to_bits());
        assert!(same, "results differ between 1 and {threads} threads");
    }
}
