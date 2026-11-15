//! The surface boundary driving a 1D ground column: steady state against the exact nonlinear
//! balance, and the per-step energy closure with 0, 1 and 2 corrector (Picard) passes.

use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_grid::{Axis, Grid};
use basetherm_surface::{face_temperature, Convection, GroundSurface, Weather};

const K: f64 = 1.5;
const CV: f64 = 2.0e6;
const LEN: f64 = 2.0;
const CELLS: usize = 40;
const T_DEEP: f64 = 8.0;

fn surface() -> GroundSurface {
    GroundSurface::new(0.25, 0.95, Convection::WindLinear { a: 5.0, b: 4.0 }).unwrap()
}

struct Column {
    adi: Adi,
    bc: Boundaries,
    flux: Fluxes,
    t: Vec<f64>,
    ts: f64,
}

impl Column {
    fn new(t0: f64) -> Self {
        let z = Axis::uniform(0.0, LEN, CELLS).unwrap();
        let grid = Grid::new(
            Axis::collapsed(1.0).unwrap(),
            Axis::collapsed(1.0).unwrap(),
            z,
        );
        let n = grid.cell_count();
        let mut bc = Boundaries::adiabatic(&grid);
        bc.fill(Side::ZHi, Bc::Fixed(T_DEEP));
        let flux = Fluxes::zeros(&grid);
        let adi = Adi::new(grid, vec![K; n], vec![CV; n]).unwrap();
        Self {
            adi,
            bc,
            flux,
            t: vec![t0; n],
            ts: t0,
        }
    }

    /// One step with `1 + correctors` passes. Returns (closure error, W/m2): the heat flow the
    /// solver delivers into the ground minus the exact balance at the resulting surface temperature.
    fn step(&mut self, s: &GroundSurface, w: &Weather, dt: f64, correctors: usize) -> f64 {
        let saved = self.t.clone();
        let mut ts_ref = self.ts;
        let mut closure = 0.0;
        for _ in 0..=correctors {
            self.t.copy_from_slice(&saved);
            let bc = s.bc(w, ts_ref);
            self.bc.set(Side::ZLo, 0, bc);
            self.adi
                .step(&mut self.t, dt, &self.bc, &mut self.flux)
                .unwrap();
            let Bc::Convective { h, t_inf } = bc else {
                unreachable!()
            };
            let h_cell = 2.0 * K / (LEN / CELLS as f64);
            let ts_new = face_temperature(h, t_inf, h_cell, self.t[0]);
            let delivered = -self.flux.side(Side::ZLo)[0];
            closure = delivered - s.flux_in(w, ts_new);
            ts_ref = ts_new;
        }
        self.ts = ts_ref;
        closure
    }
}

#[test]
fn steady_surface_temperature_matches_the_exact_balance() {
    let s = surface();
    let w = Weather {
        t_air: 5.0,
        wind: 3.0,
        ghi: 200.0,
        ir_sky: 300.0,
    };
    let mut col = Column::new(8.0);
    for _ in 0..2000 {
        col.step(&s, &w, 86400.0, 0);
    }
    // Exact: balance into the ground equals conduction to the deep boundary, k (Ts - T_deep) / L.
    let f = |ts: f64| s.flux_in(&w, ts) - K * (ts - T_DEEP) / LEN;
    let (mut lo, mut hi) = (-100.0, 100.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid
        } else {
            hi = mid
        }
    }
    let exact = 0.5 * (lo + hi);
    println!(
        "steady surface temperature: model {:.6} C, exact {exact:.6} C",
        col.ts
    );
    assert!((col.ts - exact).abs() < 1e-6, "{} vs {exact}", col.ts);
}

#[test]
fn energy_closure_improves_with_corrector_passes() {
    let s = surface();
    let day = 24.0 * 3600.0;
    let weather_at = |h: usize| {
        let phase = 2.0 * std::f64::consts::PI * (h as f64 + 0.5) / 24.0;
        Weather {
            t_air: 5.0 + 8.0 * (phase - 1.5).sin(),
            wind: 3.0,
            ghi: (600.0 * (phase - 2.0).sin()).max(0.0),
            ir_sky: 280.0,
        }
    };
    let mut worst = [0.0_f64; 3];
    for (passes, slot) in worst.iter_mut().enumerate() {
        let mut col = Column::new(8.0);
        // Spin up 10 days with the full corrector so every run starts from the same state.
        for h in 0..240 {
            col.step(&s, &weather_at(h % 24), day / 24.0, 3);
        }
        let start = (col.t.clone(), col.ts);
        let mut col2 = Column::new(8.0);
        (col2.t, col2.ts) = start;
        for h in 0..72 {
            let e = col2.step(&s, &weather_at(h % 24), day / 24.0, passes);
            *slot = slot.max(e.abs());
        }
    }
    println!("worst closure error over 3 days (W/m2): lagged {:.4}, +1 corrector {:.4}, +2 correctors {:.4}", worst[0], worst[1], worst[2]);
    assert!(worst[0] > worst[1] && worst[1] > worst[2], "{worst:?}");
    // Measured: about 2 W/m2 lagged, 0.03 with one corrector, 0.0004 with two. Hourly steps, a diurnal
    // swing of 16 K in air temperature, clear-night sky.
    assert!(worst[0] < 3.0, "lagged closure error {} W/m2", worst[0]);
    assert!(worst[1] < 0.05, "after one corrector {} W/m2", worst[1]);
    assert!(worst[2] < 0.01, "after two correctors {} W/m2", worst[2]);
}
