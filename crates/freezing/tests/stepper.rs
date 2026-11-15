//! Conservation, equivalence with the plain solver, and robustness of the freezing step.

use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_freezing::FreezingStepper;
use basetherm_grid::{Axis, Grid};
use basetherm_materials::Material;

fn soil(k_t: f64, latent: f64, t_freeze: f64) -> Material {
    Material {
        k_thawed: k_t,
        k_frozen: k_t * 1.4,
        cv_thawed: 3.0e6,
        cv_frozen: 2.0e6,
        latent,
        t_freeze,
        half_width: 0.5,
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

fn conservation_run(dt: f64, steps: usize) -> usize {
    let ax = |n: usize, len: f64| Axis::uniform(0.0, len, n).unwrap();
    let grid = Grid::new(ax(4, 2.0), ax(3, 1.5), ax(12, 3.0));
    let n = grid.cell_count();
    let mats = [
        soil(1.2, 1.0e8, -0.3),
        soil(2.0, 6.0e7, 0.0),
        soil(0.9, 1.4e8, -0.6),
    ];
    let cell_material: Vec<u16> = (0..n).map(|c| ((c % 12) / 4) as u16).collect(); // layers in z
    let mut rng = Lcg(7);
    let mut t: Vec<f64> = (0..n).map(|_| 3.0 + rng.next()).collect();
    let mut st = FreezingStepper::new(grid.clone(), &mats, cell_material, &t).unwrap();
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(
        Side::ZLo,
        Bc::Convective {
            h: 15.0,
            t_inf: -8.0,
        },
    );
    bc.fill(Side::ZHi, Bc::Fixed(3.0));
    bc.fill(Side::XHi, Bc::Convective { h: 4.0, t_inf: 1.0 });
    let mut flux = Fluxes::zeros(&grid);
    let (mut extra, mut froze) = (0, false);
    for _ in 0..steps {
        let h0 = st.heat_content(&t);
        let info = st.step(&mut t, dt, &bc, &mut flux).unwrap();
        extra += info.substeps - 1;
        froze |= t.iter().any(|&x| x < -0.5);
        let dh = st.heat_content(&t) - h0;
        let out = flux.total_out() * dt;
        assert!(
            (dh + out).abs() < 1e-9 * (h0.abs() + out.abs()),
            "dH {dh}, out {out}"
        );
    }
    assert!(froze, "the test must actually freeze part of the soil");
    extra
}

#[test]
fn enthalpy_is_conserved_exactly_through_freezing_in_a_layered_3d_block() {
    // Normal steps, and steps long enough that the halving path is exercised.
    let normal = conservation_run(6.0 * 3600.0, 120);
    let long = conservation_run(30.0 * 86400.0, 12);
    println!("6 h steps: {normal} extra substeps in 120 steps; 30 day steps: {long} extra substeps in 12");
    assert_eq!(normal, 0, "ordinary steps must not be split");
    assert!(long > 0, "the long-step run must exercise the halving path");
}

#[test]
fn without_latent_heat_it_reproduces_the_plain_solver() {
    let ax = |n: usize, len: f64| Axis::uniform(0.0, len, n).unwrap();
    let grid = Grid::new(ax(5, 2.0), ax(4, 1.0), ax(8, 2.0));
    let n = grid.cell_count();
    let m = Material::from_k_rho_c(1.9, 1490.0, 1800.0);
    let t0: Vec<f64> = (0..n).map(|c| 5.0 + (c % 7) as f64).collect();
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(Side::ZLo, Bc::Fixed(-5.0));
    bc.fill(Side::YHi, Bc::Convective { h: 6.0, t_inf: 2.0 });
    let mut plain = Adi::new(grid.clone(), vec![m.k_thawed; n], vec![m.cv_thawed; n]).unwrap();
    let mut st = FreezingStepper::new(grid.clone(), &[m], vec![0; n], &t0).unwrap();
    let (mut a, mut b) = (t0.clone(), t0);
    let (mut fa, mut fb) = (Fluxes::zeros(&grid), Fluxes::zeros(&grid));
    for _ in 0..40 {
        plain.step(&mut a, 3600.0, &bc, &mut fa).unwrap();
        st.step(&mut b, 3600.0, &bc, &mut fb).unwrap();
    }
    let worst = a
        .iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max);
    assert!(worst < 1e-9, "{worst}");
}

#[test]
fn huge_time_steps_stay_bounded_and_finite() {
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        Axis::uniform(0.0, 5.0, 40).unwrap(),
    );
    let n = grid.cell_count();
    let mut t = vec![4.0; n];
    let mut st =
        FreezingStepper::new(grid.clone(), &[soil(1.5, 1.0e8, 0.0)], vec![0; n], &t).unwrap();
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(Side::ZLo, Bc::Fixed(-12.0));
    bc.fill(Side::ZHi, Bc::Fixed(4.0));
    let mut flux = Fluxes::zeros(&grid);
    for dt in [1.0e5, 1.0e6, 1.0e7, 1.0e8] {
        st.step(&mut t, dt, &bc, &mut flux).unwrap();
        assert!(
            t.iter()
                .all(|x| x.is_finite() && (-12.0 - 1e-6..=4.0 + 1e-6).contains(x)),
            "dt {dt}: {t:?}"
        );
    }
}

#[test]
fn bad_inputs_are_rejected() {
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        Axis::uniform(0.0, 1.0, 4).unwrap(),
    );
    let m = soil(1.5, 1.0e8, 0.0);
    assert!(FreezingStepper::new(grid.clone(), &[m], vec![0; 3], &[0.0; 4]).is_err());
    assert!(FreezingStepper::new(grid, &[m], vec![0, 0, 1, 0], &[0.0; 4]).is_err());
}

#[test]
fn a_step_too_long_for_the_physics_is_halved_and_stays_physical() {
    // From a frozen-through state, a huge step over-extracts heat from a stiff band cell (it reached
    // -430 C against a boundary minimum of -12 C before the halving was added). Such steps are now
    // split until the result is physical, and energy stays exact.
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        Axis::uniform(0.0, 5.0, 40).unwrap(),
    );
    let n = grid.cell_count();
    let m = Material {
        k_thawed: 1.5,
        k_frozen: 2.1,
        cv_thawed: 3.0e6,
        cv_frozen: 2.0e6,
        latent: 1.0e8,
        t_freeze: 0.0,
        half_width: 0.5,
    };
    let mut t = vec![4.0; n];
    let mut st = FreezingStepper::new(grid.clone(), &[m], vec![0; n], &t).unwrap();
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(Side::ZLo, Bc::Fixed(-12.0));
    bc.fill(Side::ZHi, Bc::Fixed(4.0));
    let mut flux = Fluxes::zeros(&grid);
    let mut split = 0;
    for dt in [1.0e5, 1.0e6, 1.0e7, 1.0e8] {
        let h0 = st.heat_content(&t);
        let info = st.step(&mut t, dt, &bc, &mut flux).unwrap();
        split += info.substeps - 1;
        println!("dt {dt:.0e}: {} sub-steps", info.substeps);
        assert!(
            t.iter().all(|x| (-12.0 - 0.5..=4.0 + 0.5).contains(x)),
            "dt {dt}"
        );
        let (dh, out) = (st.heat_content(&t) - h0, flux.total_out() * dt);
        assert!(
            (dh + out).abs() < 1e-9 * (h0.abs() + out.abs()),
            "dt {dt}: dH {dh}, out {out}"
        );
    }
    assert!(
        split > 0,
        "at least one of these steps must have needed splitting"
    );
}
