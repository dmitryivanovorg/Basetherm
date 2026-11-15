//! Initial state from the direct periodic solve (mean + annual harmonic).
//!
//! The surface balance is linearised: radiation about the annual-mean surface temperature
//! (iterated three times), and the film coefficient by its annual mean, with the hourly equivalent
//! ambient taken as `h(t) T_eq(t) / h_mean`. Wind-temperature covariance is neglected; the hourly
//! run that follows corrects the initial state.

use crate::column::{Bottom, ColumnSpec};
use crate::error::Error;
use basetherm_bc::{Bc, Boundaries, Side};
use basetherm_climate_epw::{annual_harmonic, Harmonic};
use basetherm_mg::C64;
use basetherm_periodic::{solve, Amplitudes, Params};
use basetherm_surface::{face_temperature, GroundSurface, Weather};

/// The surface balance linearised about the mean surface temperature `t_m`: the annual-mean film
/// coefficient and the harmonic of the equivalent ambient temperature `h(t) T_eq(t) / h_mean`.
pub fn linearised_film(surf: &GroundSurface, weather: &[Weather], t_m: f64) -> (f64, Harmonic) {
    let n = weather.len() as f64;
    let films: Vec<(f64, f64)> = weather.iter().map(|w| surf.film(w, t_m)).collect();
    let h_mean = films.iter().map(|f| f.0).sum::<f64>() / n;
    let t_eq: Vec<f64> = films.iter().map(|&(h, t)| h * t / h_mean).collect();
    (h_mean, annual_harmonic(&t_eq))
}

/// Column temperatures at hour 0, and the mean surface temperature.
pub fn periodic_start(
    spec: &ColumnSpec,
    bottom: Bottom,
    surf: &GroundSurface,
    weather: &[Weather],
) -> Result<(Vec<f64>, f64), Error> {
    let grid = spec.grid();
    let n = weather.len() as f64;
    let mut t_m = weather.iter().map(|w| w.t_air).sum::<f64>() / n;
    // Soil properties for the linear solve: thawed, or frozen where the mean temperature is below
    // freezing (found by the same three passes as the surface linearisation).
    let (mut k, mut cv) = (spec.k.clone(), spec.cv.clone());
    let mut start = Vec::new();
    for _ in 0..3 {
        let h_cell = 2.0 * k[0] / spec.z.width(0);
        let (h_mean, harm) = linearised_film(surf, weather, t_m);

        let mut bc = Boundaries::adiabatic(&grid);
        bc.fill(
            Side::ZLo,
            Bc::Convective {
                h: h_mean,
                t_inf: harm.mean,
            },
        );
        if let Bottom::Fixed(t) = bottom {
            bc.fill(Side::ZHi, Bc::Fixed(t));
        }
        let mut amp = Amplitudes::zeros(&grid);
        amp.fill(Side::ZLo, C64::new(harm.amp_re, harm.amp_im));
        let sol = solve(&grid, &k, &cv, &bc, &amp, Params::annual())?;
        t_m = face_temperature(h_mean, harm.mean, h_cell, sol.mean[0]);
        start = (0..spec.z.len()).map(|c| sol.temperature(c, 0.0)).collect();
        if let Some(f) = &spec.freezing {
            for c in 0..spec.z.len() {
                let m = &f.materials[usize::from(f.cell_material[c])];
                let frozen = sol.mean[c] < m.t_freeze;
                k[c] = if frozen { m.k_frozen } else { m.k_thawed };
                cv[c] = if frozen { m.cv_frozen } else { m.cv_thawed };
            }
        }
    }
    Ok((start, t_m))
}
