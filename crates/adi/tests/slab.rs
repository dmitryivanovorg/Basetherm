//! Steady convective slab and heat balance.

mod common;
use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_grid::{Axis, Grid};
use common::{make, Lcg};

#[test]
fn steady_flux_through_convective_slab() {
    // Left: film h to 20 C. Right: fixed 0 C. Slab 0.5 m, k = 2.
    let (h, k, len) = (8.0, 2.0, 0.5);
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        Axis::uniform(0.0, len, 20).unwrap(),
    );
    let (mut adi, mut bc, mut fl) = make(grid, k, 2.0e6);
    bc.fill(Side::ZLo, Bc::Convective { h, t_inf: 20.0 });
    bc.fill(Side::ZHi, Bc::Fixed(0.0));
    let mut t = vec![0.0; 20];
    for _ in 0..200 {
        adi.step(&mut t, 1.0e5, &bc, &mut fl).unwrap();
    }
    let q_exact = 20.0 / (1.0 / h + len / k); // W/m2, unit area
    assert!((fl.side_total(Side::ZHi) - q_exact).abs() < 1e-6 * q_exact);
    assert!((fl.side_total(Side::ZLo) + q_exact).abs() < 1e-6 * q_exact); // heat enters on the left
}

#[test]
fn energy_balance_is_exact_with_random_3d_fields() {
    let mut rng = Lcg(42);
    let ax = |n: usize, rng: &mut Lcg| {
        let mut f = vec![0.0];
        for _ in 0..n {
            f.push(f.last().unwrap() + 0.2 + rng.next());
        }
        Axis::from_faces(f).unwrap()
    };
    let grid = Grid::new(ax(5, &mut rng), ax(4, &mut rng), ax(6, &mut rng));
    let n = grid.cell_count();
    let k: Vec<f64> = (0..n).map(|_| 0.2 + 3.0 * rng.next()).collect();
    let cv: Vec<f64> = (0..n).map(|_| 1.0e6 + 3.0e6 * rng.next()).collect();
    let mut adi = basetherm_adi::Adi::new(grid.clone(), k, cv).unwrap();
    let mut bc = Boundaries::adiabatic(&grid);
    for (si, side) in Side::ALL.into_iter().enumerate() {
        for f in 0..bc.side(side).len() {
            let t_b = 20.0 * rng.next();
            let b = match (si + f) % 3 {
                0 => Bc::Adiabatic,
                1 => Bc::Fixed(t_b),
                _ => Bc::Convective {
                    h: 1.0 + 20.0 * rng.next(),
                    t_inf: t_b,
                },
            };
            bc.set(side, f, b);
        }
    }
    let mut t: Vec<f64> = (0..n).map(|_| 20.0 * rng.next()).collect();
    let mut fl = Fluxes::zeros(&grid);
    let dt = 3600.0;
    for _ in 0..20 {
        let e0 = adi.heat_content(&t);
        adi.step(&mut t, dt, &bc, &mut fl).unwrap();
        let de = adi.heat_content(&t) - e0;
        let out = fl.total_out() * dt;
        assert!(
            (de + out).abs() < 1e-9 * (e0.abs() + out.abs()),
            "dE={de}, out={out}"
        );
    }
}

#[test]
fn three_d_with_adiabatic_sides_equals_1d() {
    let z = Axis::uniform(0.0, 2.0, 25).unwrap();
    let g1 = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        z.clone(),
    );
    let g3 = Grid::new(
        Axis::uniform(0.0, 3.0, 4).unwrap(),
        Axis::uniform(0.0, 2.0, 5).unwrap(),
        z,
    );
    let run = |g: Grid| {
        let (mut adi, mut bc, mut fl) = make(g.clone(), 1.5, 2.0e6);
        bc.fill(Side::ZLo, Bc::Fixed(15.0));
        bc.fill(
            Side::ZHi,
            Bc::Convective {
                h: 5.0,
                t_inf: -5.0,
            },
        );
        let mut t = vec![3.0; g.cell_count()];
        for _ in 0..50 {
            adi.step(&mut t, 7200.0, &bc, &mut fl).unwrap();
        }
        t
    };
    let (t1, t3) = (run(g1), run(g3));
    for (idx, x) in t3.iter().enumerate() {
        assert!((x - t1[idx % 25]).abs() < 1e-11, "cell {idx}");
    }
}

#[test]
fn stays_stable_and_bounded_for_huge_time_steps() {
    let grid = common::column(10.0, 0.05, 1.0, 1.3);
    let n = grid.cell_count();
    let (mut adi, mut bc, mut fl) = make(grid, 1.0, 2.0e6);
    bc.fill(Side::ZLo, Bc::Fixed(30.0));
    bc.fill(Side::ZHi, Bc::Fixed(10.0));
    let mut t = vec![10.0; n];
    for _ in 0..10 {
        adi.step(&mut t, 1.0e9, &bc, &mut fl).unwrap();
        assert!(t.iter().all(|&x| (10.0 - 1e-9..=30.0 + 1e-9).contains(&x)));
    }
}
