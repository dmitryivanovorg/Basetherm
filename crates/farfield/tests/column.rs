use basetherm_farfield::{solve, Bottom, ColumnSpec, Options, HOURS};
use basetherm_grid::{Axis, GradingSpec};
use basetherm_mg::C64;
use basetherm_surface::{Convection, GroundSurface, Weather};
use std::f64::consts::PI;

fn graded(depth: f64) -> Axis {
    Axis::graded(
        0.0,
        depth,
        &[0.0],
        GradingSpec {
            h_min: 0.05,
            h_max: 4.0,
            ratio: 1.2,
        },
    )
    .unwrap()
}

fn spec(z: Axis, k: f64, cv: f64, bottom: Bottom) -> ColumnSpec {
    let n = z.len();
    ColumnSpec {
        z,
        k: vec![k; n],
        cv: vec![cv; n],
        bottom,
        freezing: None,
    }
}

fn weather(f: impl Fn(usize) -> f64) -> Vec<Weather> {
    (0..HOURS)
        .map(|i| Weather {
            t_air: f(i),
            wind: 3.0,
            ghi: 0.0,
            ir_sky: 300.0,
        })
        .collect()
}

#[test]
fn constant_weather_gives_a_uniform_column_at_the_equilibrium_temperature() {
    let s = GroundSurface::new(0.25, 0.95, Convection::Constant(15.0)).unwrap();
    let w = vec![
        Weather {
            t_air: 5.0,
            wind: 3.0,
            ghi: 100.0,
            ir_sky: 300.0
        };
        HOURS
    ];
    let col = solve(
        &spec(graded(100.0), 1.5, 2.0e6, Bottom::Adiabatic),
        &s,
        &w,
        &Options::default(),
    )
    .unwrap();
    let eq = s.equilibrium_temperature(&w[0]);
    assert!(
        col.converged && col.years == 1,
        "years {}, storage {}",
        col.years,
        col.mean_storage
    );
    for c in 0..col.cells() {
        assert!(
            (col.cell(4000, c) - eq).abs() < 1e-4,
            "cell {c}: {} vs {eq}",
            col.cell(4000, c)
        );
    }
}

#[test]
fn annual_air_cycle_matches_the_analytic_half_space_with_a_film() {
    // No radiation and no sun, so the surface balance is linear: T_air = 10 + 8 cos(w t) through a
    // film h. Exact periodic solution: theta(z) = A exp(-(1+i) z/d) / (1 + (1+i) k/(h d)).
    let (k, cv, h) = (1.5, 2.0e6, 20.0);
    let s = GroundSurface::new(0.0, 0.0, Convection::Constant(h)).unwrap();
    let w_ann = 2.0 * PI / (HOURS as f64 * 3600.0);
    let w = weather(|i| 10.0 + 8.0 * (w_ann * (i as f64 + 0.5) * 3600.0).cos());
    let z = graded(100.0);
    let col = solve(
        &spec(z.clone(), k, cv, Bottom::Adiabatic),
        &s,
        &w,
        &Options::default(),
    )
    .unwrap();
    assert!(col.converged, "storage {}", col.mean_storage);
    let d = (2.0 * (k / cv) / w_ann).sqrt();
    let denom = C64::new(1.0, 0.0) + C64::new(1.0, 1.0) * (k / (h * d));
    let mut worst: f64 = 0.0;
    for hour in [100, 2500, 6000, 8759] {
        let t = (hour as f64 + 1.0) * 3600.0;
        for c in (0..z.len()).filter(|c| z.center(*c) < 12.0) {
            let zz = z.center(c);
            let decay = (-zz / d).exp();
            let theta = C64::new(8.0, 0.0)
                * C64::new(decay * (zz / d).cos(), -decay * (zz / d).sin())
                / denom;
            worst = worst.max((col.cell(hour, c) - (10.0 + theta.re_at(w_ann * t))).abs());
        }
    }
    println!(
        "annual cycle: {} years, worst deviation from analytic {worst:.4} K",
        col.years
    );
    assert!(worst < 0.02, "{worst}");
}

#[test]
fn layered_soil_with_a_fixed_bottom_is_piecewise_linear_and_the_face_value_is_exact() {
    let s = GroundSurface::new(0.25, 0.95, Convection::Constant(15.0)).unwrap();
    let w = vec![
        Weather {
            t_air: 5.0,
            wind: 3.0,
            ghi: 200.0,
            ir_sky: 300.0
        };
        HOURS
    ];
    let z = Axis::uniform(0.0, 20.0, 40).unwrap();
    let (k1, k2, t_bottom) = (0.8, 2.0, 8.0);
    let k: Vec<f64> = (0..40)
        .map(|c| if z.center(c) < 5.0 { k1 } else { k2 })
        .collect();
    let sp = ColumnSpec {
        z: z.clone(),
        k,
        cv: vec![2.0e6; 40],
        bottom: Bottom::Fixed(t_bottom),
        freezing: None,
    };
    let col = solve(&sp, &s, &w, &Options::default()).unwrap();
    assert!(col.converged, "storage {}", col.mean_storage);
    // Exact steady state: flux_in(Ts) = (Ts - T_bottom) / (R1 + R2).
    let (r1, r2) = (5.0 / k1, 15.0 / k2);
    let f = |ts: f64| s.flux_in(&w[0], ts) - (ts - t_bottom) / (r1 + r2);
    let (mut lo, mut hi) = (-50.0, 50.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid
        } else {
            hi = mid
        }
    }
    let ts = 0.5 * (lo + hi);
    let q = (ts - t_bottom) / (r1 + r2);
    let exact = |zz: f64| {
        if zz < 5.0 {
            ts - q * zz / k1
        } else {
            ts - q * (r1 + (zz - 5.0) / k2)
        }
    };
    for c in 0..40 {
        assert!(
            (col.cell(5000, c) - exact(z.center(c))).abs() < 1e-3,
            "cell {c}"
        );
    }
    // Face 10 lies exactly on the layer interface at 5 m.
    assert!(
        (col.face(5000, 10) - exact(5.0)).abs() < 1e-3,
        "{} vs {}",
        col.face(5000, 10),
        exact(5.0)
    );
}

/// A real TMY3-derived EPW downloaded into refs/epw/; skipped when absent.
#[test]
fn real_weather_gives_a_converged_physically_plausible_profile() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../refs/epw/USA_CO_Golden-NREL.724666_TMY3.epw");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("skipping: {} not found", path.display());
        return;
    };
    let epw = basetherm_climate_epw::parse(&text).unwrap();
    let w: Vec<Weather> = (0..HOURS).map(|i| epw.weather(i)).collect();
    let s = GroundSurface::new(0.25, 0.95, Convection::WALTON_ROUGH).unwrap();
    let z = graded(100.0);
    let col = solve(
        &spec(z.clone(), 1.5, 2.2e6, Bottom::Adiabatic),
        &s,
        &w,
        &Options::default(),
    )
    .unwrap();
    println!(
        "Golden CO: {} years, storage {:.5} W/m2, surface flux {:.5} W/m2, converged {}",
        col.years, col.mean_storage, col.mean_surface_flux, col.converged
    );
    let at = |depth: f64| {
        (0..z.len())
            .min_by(|&a, &b| {
                (z.center(a) - depth)
                    .abs()
                    .total_cmp(&(z.center(b) - depth).abs())
            })
            .unwrap()
    };
    let mut means = vec![];
    for depth in [0.1, 0.5, 2.0, 5.0, 10.0, 25.0] {
        let c = at(depth);
        println!(
            "  {depth:>5.1} m: mean {:.2} C, half-range {:.2} K",
            col.annual_mean(c),
            col.half_range(c)
        );
        means.push((col.annual_mean(c), col.half_range(c)));
    }
    let air_mean = epw.annual_means().t_air;
    assert!(col.converged);
    assert!(
        (8.0..16.0).contains(&means[4].0) && means[4].0 > air_mean - 0.5,
        "{} (air {air_mean})",
        means[4].0
    );
    assert!(
        means.windows(2).all(|p| p[1].1 < p[0].1),
        "half-range must fall with depth"
    );
    // Steady state under an adiabatic bottom: zero mean flux at every depth, so a flat mean profile.
    let hi = means.iter().map(|m| m.0).fold(f64::MIN, f64::max);
    let lo = means.iter().map(|m| m.0).fold(f64::MAX, f64::min);
    println!("  spread of annual means over depth: {:.3} K", hi - lo);
    assert!(
        hi - lo < 0.15,
        "annual mean must be flat with depth, spread {}",
        hi - lo
    );
    assert!(
        col.mean_surface_flux.abs() < 0.002,
        "{}",
        col.mean_surface_flux
    );
}

fn seasonal(mean: f64, amp: f64) -> Vec<Weather> {
    let w = 2.0 * PI / HOURS as f64;
    weather(|i| mean + amp * (w * (i as f64 + 0.5)).cos())
}

fn frost_soil(latent: f64) -> basetherm_materials::Material {
    basetherm_materials::Material {
        k_thawed: 1.5,
        k_frozen: 2.0,
        cv_thawed: 2.8e6,
        cv_frozen: 2.0e6,
        latent,
        t_freeze: 0.0,
        half_width: 0.5,
    }
}

#[test]
fn freezing_with_no_latent_heat_and_equal_properties_is_the_linear_column() {
    let s = GroundSurface::new(0.25, 0.9, Convection::Constant(15.0)).unwrap();
    let w = seasonal(2.0, 14.0);
    let z = graded(60.0);
    let n = z.len();
    let m = basetherm_materials::Material::constant(1.5, 2.2e6);
    let lin = solve(
        &spec(z.clone(), 1.5, 2.2e6, Bottom::Adiabatic),
        &s,
        &w,
        &Options::default(),
    )
    .unwrap();
    let frz = ColumnSpec::with_freezing(z, vec![m], vec![0; n], Bottom::Adiabatic).unwrap();
    let frz = solve(&frz, &s, &w, &Options::default()).unwrap();
    assert!(lin.converged && frz.converged);
    let worst = (0..lin.cells())
        .map(|c| (lin.cell(3000, c) - frz.cell(3000, c)).abs())
        .fold(0.0, f64::max);
    assert!(worst < 1e-6, "{worst}");
}

#[test]
fn seasonal_frost_is_buffered_by_latent_heat() {
    // Annual air cycle 6 +/- 16 C: the top metre freezes every winter while the deep ground stays well
    // above 0 C (a mean near 0 C puts the deep soil at the freezing point, where the effective capacity
    // is about 35 times the sensible one and the spin-up takes centuries). Latent heat must damp the
    // winter minimum at depth compared with the same soil without latent heat, and the search converges.
    let s = GroundSurface::new(0.25, 0.9, Convection::Constant(15.0)).unwrap();
    let w = seasonal(6.0, 16.0);
    let z = graded(60.0);
    let n = z.len();
    let solve_with = |latent: f64| {
        let sp = ColumnSpec::with_freezing(
            z.clone(),
            vec![frost_soil(latent)],
            vec![0; n],
            Bottom::Adiabatic,
        )
        .unwrap();
        solve(&sp, &s, &w, &Options::default()).unwrap()
    };
    let (dry, wet) = (solve_with(0.0), solve_with(1.0e8));
    // The reference (no latent heat, but a frozen/thawed conductivity contrast) spins up more slowly;
    // the 1 m winter minimum compared below equilibrates within the first years either way.
    assert!(wet.converged, "storage {}", wet.mean_storage);
    assert!(
        dry.mean_storage.abs() < 0.02,
        "storage {}",
        dry.mean_storage
    );
    let c = (0..n)
        .min_by(|&a, &b| {
            (z.center(a) - 1.0)
                .abs()
                .total_cmp(&(z.center(b) - 1.0).abs())
        })
        .unwrap();
    let coldest = |col: &basetherm_farfield::Undisturbed| {
        (0..HOURS).map(|h| col.cell(h, c)).fold(f64::MAX, f64::min)
    };
    assert!(
        coldest(&dry) < -1.0,
        "the winter must freeze the soil at 1 m ({})",
        coldest(&dry)
    );
    assert!(
        coldest(&wet) > coldest(&dry) + 0.3,
        "latent heat must damp the minimum: {} vs {}",
        coldest(&wet),
        coldest(&dry)
    );
}

#[test]
fn freezing_column_rejects_a_bad_material_index() {
    let z = Axis::uniform(0.0, 4.0, 4).unwrap();
    assert!(ColumnSpec::with_freezing(
        z.clone(),
        vec![frost_soil(1.0e8)],
        vec![0, 0, 1, 0],
        Bottom::Adiabatic
    )
    .is_err());
    assert!(
        ColumnSpec::with_freezing(z, vec![frost_soil(1.0e8)], vec![0; 3], Bottom::Adiabatic)
            .is_err()
    );
}
