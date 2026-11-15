//! Two-phase Stefan problem (Neumann): initially thawed soil at T_i, surface held at T_s < T_f.
//! Frozen:  T = T_s + A erf(x / 2 sqrt(a_f t)),   A = (T_f - T_s) / erf(lambda)
//! Thawed:  T = T_i - B erfc(x / 2 sqrt(a_t t)),  B = (T_i - T_f) / erfc(lambda sqrt(a_f/a_t))
//! Front:   X = 2 lambda sqrt(a_f t), with lambda from the interface energy balance
//!          L dX/dt = k_f T_x(frozen side) - k_t T_x(thawed side).

use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_freezing::FreezingStepper;
use basetherm_grid::{Axis, Grid};
use basetherm_materials::Material;

const K_F: f64 = 2.2;
const K_T: f64 = 1.5;
const CV_F: f64 = 2.0e6;
const CV_T: f64 = 3.0e6;
const LATENT: f64 = 1.0e8;
const T_S: f64 = -10.0;
const T_I: f64 = 5.0;
const DAY: f64 = 86400.0;

/// Abramowitz & Stegun 7.1.26, absolute error below 1.5e-7.
fn erf(x: f64) -> f64 {
    let s = x.signum();
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    s * (1.0 - poly * (-x * x).exp())
}

fn erfc(x: f64) -> f64 {
    1.0 - erf(x)
}

fn alphas() -> (f64, f64) {
    (K_F / CV_F, K_T / CV_T)
}

fn residual(l: f64) -> f64 {
    let (af, at) = alphas();
    let r = (af / at).sqrt();
    let a = (0.0 - T_S) / erf(l);
    let b = (T_I - 0.0) / erfc(l * r);
    CV_F * a * (-l * l).exp() - K_T * b * (-l * l * af / at).exp() / (at * af).sqrt() - L_SQRT_PI(l)
}

#[allow(non_snake_case)]
fn L_SQRT_PI(l: f64) -> f64 {
    LATENT * l * std::f64::consts::PI.sqrt()
}

fn lambda() -> f64 {
    let (mut lo, mut hi) = (1e-6, 3.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if residual(mid) > 0.0 {
            lo = mid
        } else {
            hi = mid
        }
    }
    0.5 * (lo + hi)
}

fn front(t: f64) -> f64 {
    2.0 * lambda() * (alphas().0 * t).sqrt()
}

fn exact_temperature(x: f64, t: f64) -> f64 {
    let (af, at) = alphas();
    let l = lambda();
    if x < front(t) {
        T_S + (0.0 - T_S) / erf(l) * erf(x / (2.0 * (af * t).sqrt()))
    } else {
        T_I - T_I / erfc(l * (af / at).sqrt()) * erfc(x / (2.0 * (at * t).sqrt()))
    }
}

#[test]
fn the_analytic_solution_satisfies_the_stefan_condition() {
    // Check the transcription independently: finite differences of the profiles at the front.
    let t = 50.0 * DAY;
    let x = front(t);
    let h = 1e-5;
    let dtdx_frozen = (exact_temperature(x - 1e-9, t) - exact_temperature(x - h, t)) / (h - 1e-9);
    let dtdx_thawed = (exact_temperature(x + h, t) - exact_temperature(x + 1e-9, t)) / (h - 1e-9);
    let dxdt = (front(t * 1.000001) - front(t * 0.999999)) / (2e-6 * t);
    let lhs = LATENT * dxdt;
    let rhs = K_F * dtdx_frozen - K_T * dtdx_thawed;
    assert!(
        (lhs - rhs).abs() < 2e-3 * lhs.abs(),
        "Stefan condition: {lhs} vs {rhs}"
    );
    // Temperature is continuous at the front.
    assert!(
        exact_temperature(x - 1e-9, t).abs() < 1e-4 && exact_temperature(x + 1e-9, t).abs() < 1e-4
    );
    println!(
        "lambda = {:.5}, front after 100 days = {:.3} m",
        lambda(),
        front(100.0 * DAY)
    );
}

fn soil(half_width: f64) -> Material {
    Material {
        k_thawed: K_T,
        k_frozen: K_F,
        cv_thawed: CV_T,
        cv_frozen: CV_F,
        latent: LATENT,
        t_freeze: 0.0,
        half_width,
    }
}

/// Uniform cells of `fine` metres down to 3 m, then geometric growth to 12 m.
fn axis(fine: f64) -> Axis {
    let mut faces = vec![0.0];
    while *faces.last().unwrap() < 3.0 - 1e-9 {
        faces.push(faces.last().unwrap() + fine);
    }
    let mut w = fine;
    while *faces.last().unwrap() < 12.0 {
        w *= 1.1;
        faces.push(faces.last().unwrap() + w);
    }
    Axis::from_faces(faces).unwrap()
}

struct Run {
    z: Axis,
    t: Vec<f64>,
    extra_substeps: usize,
    steps: usize,
}

fn simulate(fine: f64, half_width: f64, dt: f64, days: f64) -> Run {
    let z = axis(fine);
    let grid = Grid::new(
        Axis::collapsed(1.0).unwrap(),
        Axis::collapsed(1.0).unwrap(),
        z.clone(),
    );
    let n = grid.cell_count();
    let mut t = vec![T_I; n];
    let mut st = FreezingStepper::new(grid.clone(), &[soil(half_width)], vec![0; n], &t).unwrap();
    let mut bc = Boundaries::adiabatic(&grid);
    bc.fill(Side::ZLo, Bc::Fixed(T_S));
    bc.fill(Side::ZHi, Bc::Fixed(T_I));
    let mut flux = Fluxes::zeros(&grid);
    let steps = (days * DAY / dt).round() as usize;
    let mut substeps = 0;
    for _ in 0..steps {
        substeps += st.step(&mut t, dt, &bc, &mut flux).unwrap().substeps - 1;
    }
    Run {
        z,
        t,
        extra_substeps: substeps,
        steps,
    }
}

/// Depth where the temperature crosses 0 C (linear interpolation between cell centres).
fn numerical_front(r: &Run) -> f64 {
    let c = (1..r.t.len())
        .find(|&c| r.t[c] >= 0.0)
        .expect("front inside the domain");
    let (z0, z1) = (r.z.center(c - 1), r.z.center(c));
    z0 + (z1 - z0) * (0.0 - r.t[c - 1]) / (r.t[c] - r.t[c - 1])
}

#[test]
fn front_position_matches_the_neumann_solution() {
    for days in [20.0, 50.0, 100.0] {
        let r = simulate(0.02, 0.5, 3600.0, days);
        let (x, exact) = (numerical_front(&r), front(days * DAY));
        println!(
            "{days:>5.0} days: front {x:.4} m, Neumann {exact:.4} m ({:+.2}%), extra substeps {} in {} steps",
            100.0 * (x - exact) / exact,
            r.extra_substeps,
            r.steps
        );
        assert!(
            (x - exact).abs() < 0.05 * exact,
            "{days} days: {x} vs {exact}"
        );
    }
}

#[test]
fn temperature_profile_matches_away_from_the_front() {
    let days = 100.0;
    let r = simulate(0.02, 0.5, 3600.0, days);
    let t = days * DAY;
    let mut worst: f64 = 0.0;
    for c in 0..r.z.len() {
        let x = r.z.center(c);
        if (x - front(t)).abs() > 0.3 && x < 6.0 {
            worst = worst.max((r.t[c] - exact_temperature(x, t)).abs());
        }
    }
    println!("worst profile deviation away from the front: {worst:.3} K");
    assert!(worst < 0.5, "{worst}");
}

#[test]
fn result_is_nearly_independent_of_the_time_step() {
    let days = 50.0;
    let ref_front = numerical_front(&simulate(0.02, 0.5, 900.0, days));
    for dt in [3600.0, 6.0 * 3600.0, 24.0 * 3600.0] {
        let x = numerical_front(&simulate(0.02, 0.5, dt, days));
        println!(
            "dt {:>6.0} s: front {x:.4} m vs dt 900 s {ref_front:.4} m ({:+.2}%)",
            dt,
            100.0 * (x - ref_front) / ref_front
        );
        assert!(
            (x - ref_front).abs() < 0.03 * ref_front,
            "dt {dt}: {x} vs {ref_front}"
        );
    }
}
