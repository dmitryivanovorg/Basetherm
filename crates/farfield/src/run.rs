//! Spin-up and the search for the deep temperature.
//!
//! Below roughly eight annual penetration depths the undisturbed profile is flat (zero mean flux at
//! every depth), so with an adiabatic bottom the whole steady state hangs on one number: the deep
//! temperature `T_flat`. The top `SEARCH_DEPTH` metres are run with a fixed bottom at a candidate
//! `T_flat`, spun up (its time constant is a few years), and the mean net surface flux is measured.
//! `T_flat` is found where that flux is zero (secant search); cells below are filled with `T_flat`.

use crate::column::{Bottom, ColumnSpec, Options};
use crate::error::Error;
use crate::init::periodic_start;
use crate::profile::Undisturbed;
use crate::HOURS;
use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Side};
use basetherm_freezing::FreezingStepper;
use basetherm_grid::Axis;
use basetherm_surface::{face_temperature, GroundSurface, Weather};

const HOUR: f64 = 3600.0;
const YEAR: f64 = HOURS as f64 * HOUR;
/// Depth below which the profile is treated as flat, m (more than 8 penetration depths for soils
/// with diffusivity up to about 1.2e-6 m2/s).
const SEARCH_DEPTH: f64 = 30.0;

/// The hourly stepper: plain conduction, or conduction with freezing.
enum Stepper {
    Linear(Box<Adi>),
    Freezing(Box<FreezingStepper>),
}

impl Stepper {
    fn step(&mut self, t: &mut [f64], bc: &Boundaries, flux: &mut Fluxes) -> Result<(), Error> {
        match self {
            Stepper::Linear(a) => a.step(t, HOUR, bc, flux)?,
            Stepper::Freezing(f) => {
                f.step(t, HOUR, bc, flux)?;
            }
        }
        Ok(())
    }

    /// Heat content (enthalpy when freezing is modelled), J per m2 of column.
    fn heat_content(&self, t: &[f64]) -> f64 {
        match self {
            Stepper::Linear(a) => a.heat_content(t),
            Stepper::Freezing(f) => f.heat_content(t),
        }
    }
}

/// One spun-up evaluation of a column with a given bottom condition.
struct Eval {
    /// Last year, hour-major, one value per cell of the evaluated column.
    temps: Vec<f64>,
    /// Mean net energy storage over the last year, W/m2.
    storage: f64,
    /// Mean heat flow into the ground through the surface over the last year, W/m2.
    surface_flux: f64,
    years: usize,
}

fn evaluate(
    spec: &ColumnSpec,
    bottom: Bottom,
    surf: &GroundSurface,
    weather: &[Weather],
    opts: &Options,
) -> Result<Eval, Error> {
    let grid = spec.grid();
    let n = spec.z.len();
    let (mut t, mut ts) = periodic_start(spec, bottom, surf, weather)?;
    let mut stepper = match &spec.freezing {
        Some(f) => Stepper::Freezing(Box::new(FreezingStepper::new(
            grid.clone(),
            &f.materials,
            f.cell_material.clone(),
            &t,
        )?)),
        None => Stepper::Linear(Box::new(Adi::new(
            grid.clone(),
            spec.k.clone(),
            spec.cv.clone(),
        )?)),
    };
    let mut bc = Boundaries::adiabatic(&grid);
    if let Bottom::Fixed(tb) = bottom {
        bc.fill(Side::ZHi, Bc::Fixed(tb));
    }
    let mut flux = Fluxes::zeros(&grid);
    let h_cell = 2.0 * spec.k[0] / spec.z.width(0);
    let mut temps = vec![0.0; HOURS * n];
    let (mut years, mut storage, mut surface_flux) = (0, 0.0, 0.0);
    // Previous year-end state, storage and surface temperature, for the extrapolation below.
    let mut prev: Option<(Vec<f64>, f64, f64)> = None;
    while years < opts.max_years {
        years += 1;
        let e0 = stepper.heat_content(&t);
        let mut into_ground = 0.0;
        for (h, w) in weather.iter().enumerate() {
            let (film, t_inf) = surf.film(w, ts);
            bc.set(Side::ZLo, 0, Bc::Convective { h: film, t_inf });
            stepper.step(&mut t, &bc, &mut flux)?;
            ts = face_temperature(film, t_inf, h_cell, t[0]);
            into_ground -= flux.side(Side::ZLo)[0];
            temps[h * n..(h + 1) * n].copy_from_slice(&t);
        }
        storage = (stepper.heat_content(&t) - e0) / YEAR;
        surface_flux = into_ground / HOURS as f64;
        if storage.abs() < opts.flux_tol {
            break;
        }
        // The slowest mode decays by a constant factor r each year, so storage_n = r storage_{n-1}.
        // Once two consecutive years show that, jump the state to where the mode has fully decayed.
        if let Some((t_prev, s_prev, ts_prev)) = &prev {
            let r = storage / s_prev;
            if r > 0.2 && r < 0.95 {
                let gain = r / (1.0 - r);
                for (x, p) in t.iter_mut().zip(t_prev) {
                    *x += (*x - p) * gain;
                }
                ts += (ts - ts_prev) * gain;
                prev = None;
                continue;
            }
        }
        prev = Some((t.clone(), storage, ts));
    }
    Ok(Eval {
        temps,
        storage,
        surface_flux,
        years,
    })
}

/// The top part of the column that is actually simulated: cells down to `SEARCH_DEPTH`.
fn truncated(spec: &ColumnSpec) -> Result<ColumnSpec, Error> {
    let faces = spec.z.faces();
    let m = faces
        .iter()
        .rposition(|&f| f <= SEARCH_DEPTH + 1e-9)
        .unwrap_or(1)
        .max(1);
    if m == spec.z.len() {
        return Ok(spec.clone());
    }
    let z = Axis::from_faces(faces[..=m].to_vec()).map_err(|_| Error::Input("column axis"))?;
    Ok(ColumnSpec {
        z,
        k: spec.k[..m].to_vec(),
        cv: spec.cv[..m].to_vec(),
        bottom: spec.bottom,
        freezing: spec.freezing.as_ref().map(|f| crate::Freezing {
            materials: f.materials.clone(),
            cell_material: f.cell_material[..m].to_vec(),
        }),
    })
}

/// Solve the undisturbed column. `weather` must be one full year of hourly values.
pub fn solve(
    spec: &ColumnSpec,
    surf: &GroundSurface,
    weather: &[Weather],
    opts: &Options,
) -> Result<Undisturbed, Error> {
    spec.validate()?;
    if weather.len() != HOURS {
        return Err(Error::Input("weather must have 8760 hourly values"));
    }
    for w in weather {
        w.validate()?;
    }
    let n = spec.z.len();
    let make = |temps: Vec<f64>, years, storage, surface_flux, converged| Undisturbed {
        z: spec.z.clone(),
        k: spec.k.clone(),
        temps,
        years,
        mean_storage: storage,
        mean_surface_flux: surface_flux,
        converged,
    };

    if let Bottom::Fixed(_) = spec.bottom {
        let e = evaluate(spec, spec.bottom, surf, weather, opts)?;
        let ok = e.storage.abs() < opts.flux_tol;
        return Ok(make(e.temps, e.years, e.storage, e.surface_flux, ok));
    }

    // Adiabatic bottom: search for the deep temperature on the truncated column.
    let top = truncated(spec)?;
    let ns = top.z.len();
    let (start, _) = periodic_start(&top, Bottom::Adiabatic, surf, weather)?;
    let eval_at = |t_flat: f64| evaluate(&top, Bottom::Fixed(t_flat), surf, weather, opts);
    let (mut x0, mut x1) = (start[ns - 1], start[ns - 1] + 0.25);
    let mut best = eval_at(x0)?;
    let mut years = best.years;
    let mut best_t = x0;
    let ok = |e: &Eval| e.surface_flux.abs() < opts.flux_tol && e.storage.abs() < opts.flux_tol;
    let (mut f0, mut iter) = (best.surface_flux, 1);
    while !ok(&best) && iter < opts.max_iterations {
        let e1 = eval_at(x1)?;
        years += e1.years;
        iter += 1;
        let f1 = e1.surface_flux;
        let improved = f1.abs() < best.surface_flux.abs();
        let next = if (f1 - f0).abs() > f64::MIN_POSITIVE {
            x1 - f1 * (x1 - x0) / (f1 - f0)
        } else {
            x1
        };
        if improved {
            best = e1;
            best_t = x1;
        }
        (x0, f0, x1) = (x1, f1, next);
    }
    let converged = ok(&best);
    let mut temps = vec![0.0; HOURS * n];
    for h in 0..HOURS {
        temps[h * n..h * n + ns].copy_from_slice(&best.temps[h * ns..(h + 1) * ns]);
        temps[h * n + ns..(h + 1) * n].fill(best_t);
    }
    Ok(make(
        temps,
        years,
        best.storage,
        best.surface_flux,
        converged,
    ))
}
