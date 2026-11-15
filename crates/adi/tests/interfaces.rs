//! Interior faces next to void cells: equivalence with an outer boundary, and exact energy balance.

use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Interfaces, InternalFace, Side};
use basetherm_grid::{Axis, Grid};

fn column(n: usize, len: f64) -> Grid {
    Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        Axis::uniform(0.0, len, n).unwrap(),
    )
}

#[test]
fn a_void_with_a_film_face_equals_an_outer_convective_boundary() {
    let (n, h, t_room, k, cv) = (20, 7.0, 21.0, 1.7, 2.1e6);
    // A: plain column, convective outer boundary at the low-z end.
    let ga = column(n, 4.0);
    let mut bca = Boundaries::adiabatic(&ga);
    bca.fill(Side::ZLo, Bc::Convective { h, t_inf: t_room });
    bca.fill(Side::ZHi, Bc::Fixed(5.0));
    let mut adi_a = Adi::new(ga.clone(), vec![k; n], vec![cv; n]).unwrap();
    // B: the same column behind one extra void cell; the film is an interior face.
    let gb = column(n + 1, 4.0 * (n + 1) as f64 / n as f64);
    let mut bcb = Boundaries::adiabatic(&gb);
    bcb.fill(Side::ZHi, Bc::Fixed(5.0));
    let mut void = vec![false; n + 1];
    void[0] = true;
    let mut ifc = Interfaces::new(void, 1);
    ifc.add_face(InternalFace {
        cell: 1,
        side: Side::ZLo,
        h,
        group: 0,
    });
    ifc.set_temperature(0, t_room);
    let mut adi_b = Adi::with_interfaces(gb.clone(), vec![k; n + 1], vec![cv; n + 1], ifc).unwrap();

    let (mut ta, mut tb) = (vec![8.0; n], vec![8.0; n + 1]);
    let (mut fa, mut fb) = (Fluxes::zeros(&ga), Fluxes::with_groups(&gb, 1));
    for _ in 0..200 {
        adi_a.step(&mut ta, 3600.0, &bca, &mut fa).unwrap();
        adi_b.step(&mut tb, 3600.0, &bcb, &mut fb).unwrap();
        let worst = (0..n)
            .map(|c| (ta[c] - tb[c + 1]).abs())
            .fold(0.0, f64::max);
        assert!(worst < 1e-10, "temperatures differ by {worst}");
        let (qa, qb) = (fa.side_total(Side::ZLo), fb.groups()[0]);
        assert!(
            (qa - qb).abs() < 1e-9 * (1.0 + qa.abs()),
            "flux {qa} vs {qb}"
        );
    }
    assert!((tb[0] - 8.0).abs() < 1e-12, "the void cell must not change");
}

#[test]
fn energy_balance_is_exact_with_an_interior_void_and_several_groups() {
    let ax = |n: usize, len: f64| Axis::uniform(0.0, len, n).unwrap();
    let grid = Grid::new(ax(8, 4.0), ax(7, 3.5), ax(9, 4.5));
    let [nx, ny, nz] = grid.dims();
    let n = grid.cell_count();
    // A void block in the middle: cells x 2..5, y 2..5, z 0..4.
    let inside = |i: usize, j: usize, k: usize| (2..5).contains(&i) && (2..5).contains(&j) && k < 4;
    let mut void = vec![false; n];
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                void[grid.index(i, j, k)] = inside(i, j, k);
            }
        }
    }
    let mut ifc = Interfaces::new(void, 3);
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                if inside(i, j, k) {
                    continue;
                }
                let c = grid.index(i, j, k);
                // Group 0: the floor (void above), group 1: x walls, group 2: y walls.
                let mut add = |side, group| {
                    ifc.add_face(InternalFace {
                        cell: c,
                        side,
                        h: 6.0 + group as f64,
                        group,
                    })
                };
                if k >= 4 && inside(i, j, k - 1) && k == 4 {
                    add(Side::ZLo, 0);
                }
                if inside(i + 1, j, k) {
                    add(Side::XHi, 1);
                }
                if i > 0 && inside(i - 1, j, k) {
                    add(Side::XLo, 1);
                }
                if inside(i, j + 1, k) {
                    add(Side::YHi, 2);
                }
                if j > 0 && inside(i, j - 1, k) {
                    add(Side::YLo, 2);
                }
            }
        }
    }
    ifc.set_temperature(0, 21.0);
    ifc.set_temperature(1, 19.0);
    ifc.set_temperature(2, 23.0);
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(
        Side::ZLo,
        Bc::Convective {
            h: 12.0,
            t_inf: -4.0,
        },
    );
    bc.fill(Side::ZHi, Bc::Fixed(9.0));
    bc.fill(Side::XHi, Bc::Convective { h: 3.0, t_inf: 6.0 });
    let mut s = 11u64;
    let mut next = move || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((s >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let k: Vec<f64> = (0..n).map(|_| 0.5 + 2.0 * next()).collect();
    let cv: Vec<f64> = (0..n).map(|_| 1.0e6 + 2.0e6 * next()).collect();
    let mut t: Vec<f64> = (0..n).map(|_| 5.0 + 10.0 * next()).collect();
    let t_start = t.clone();
    let mut adi = Adi::with_interfaces(grid.clone(), k, cv, ifc).unwrap();
    let mut flux = Fluxes::with_groups(&grid, 3);
    let dt = 5400.0;
    for step in 0..30 {
        let e0 = adi.heat_content(&t);
        adi.step(&mut t, dt, &bc, &mut flux).unwrap();
        let (de, out) = (adi.heat_content(&t) - e0, flux.total_out() * dt);
        assert!(
            (de + out).abs() < 1e-9 * (e0.abs() + out.abs()),
            "step {step}: dE {de}, out {out}"
        );
    }
    assert!(flux.groups().iter().all(|g| g.is_finite()) && flux.groups()[0].abs() > 0.0);
    // Void cells never change.
    let ifc = adi.interfaces().unwrap();
    assert!((0..n)
        .filter(|&c| ifc.is_void(c))
        .all(|c| (t[c] - t_start[c]).abs() < 1e-12));
}
