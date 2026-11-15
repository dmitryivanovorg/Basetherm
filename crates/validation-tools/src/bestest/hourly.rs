//! Hourly steady-periodic year: peak-hour floor conduction, its time, and annual total.
//!
//! The periodic solve (mean + annual harmonic fitted to the weather file) gives the initial state;
//! the diurnal part of the forcing is not in it and decays within days, so a short warm-up is run
//! before the recorded year. Weather hour `i` (0-based, covering `[i, i+1]` h) is held constant
//! over the step.

use super::case::Case;
use super::model::{surface_bc, Mesh, Model};
use basetherm_adi::Adi;
use basetherm_bc::{Fluxes, Side};
use basetherm_mg::C64;
use basetherm_periodic::{solve, Amplitudes, Error, Params};

const HOUR: f64 = 3600.0;
const YEAR_HOURS: usize = 8760;
/// Global 0-based hour index of the coldest hour of the weather files (Jan 15, hour 4).
pub const COLDEST_HOUR: usize = 14 * 24 + 3;

#[derive(Debug, Clone)]
pub struct HourlyResult {
    /// Floor heat loss in each hour of the recorded (last) year, W (positive = loss).
    pub hourly_w: Vec<f64>,
    pub annual_kwh: f64,
    pub peak_w: f64,
    /// 0-based hour index of the peak within the year.
    pub peak_hour: usize,
    /// Hours from the coldest hour to the peak.
    pub phase_shift_h: i64,
}

/// Run `years` recorded years after a `warmup_hours` warm-up; results are for the last year.
pub fn run_hourly(
    case: &Case,
    mesh: &Mesh,
    t_out: &[f64],
    warmup_hours: usize,
    years: usize,
) -> Result<HourlyResult, Error> {
    assert_eq!(t_out.len(), YEAR_HOURS, "need a full 8760-hour year");
    let h = basetherm_climate_epw::annual_harmonic(t_out);
    let (mean, amp) = (h.mean, C64::new(h.amp_re, h.amp_im));
    let mut model = Model::build(case, mesh, mean).map_err(|_| Error::BadParams("mesh"))?;
    let mut forcing = Amplitudes::zeros(&model.grid);
    for &f in &model.exterior_faces {
        forcing.set(Side::ZLo, f, amp);
    }
    let p = Params::annual();
    let sol = solve(&model.grid, &model.k, &model.cv, &model.bc, &forcing, p)?;

    // Start `warmup_hours` before Jan 1 (the end of the previous, identical, year).
    let start_hour = YEAR_HOURS - warmup_hours;
    let mut t: Vec<f64> = (0..model.grid.cell_count())
        .map(|c| sol.temperature(c, start_hour as f64 * HOUR))
        .collect();
    let mut adi =
        Adi::new(model.grid.clone(), model.k.clone(), model.cv.clone()).map_err(Error::from)?;
    let mut flux = Fluxes::zeros(&model.grid);

    let mut step = |hour_of_year: usize, t: &mut Vec<f64>| -> Result<f64, Error> {
        let temp = t_out[hour_of_year % YEAR_HOURS];
        for &f in &model.exterior_faces {
            model.bc.set(Side::ZLo, f, surface_bc(case.exterior, temp));
        }
        adi.step(t, HOUR, &model.bc, &mut flux)
            .map_err(Error::from)?;
        let out: f64 = model
            .slab_faces
            .iter()
            .map(|&f| flux.side(Side::ZLo)[f])
            .sum();
        Ok(-4.0 * out)
    };

    for h in start_hour..YEAR_HOURS {
        step(h, &mut t)?;
    }
    let mut hourly_w = vec![0.0; YEAR_HOURS];
    for _ in 0..years {
        for (h, slot) in hourly_w.iter_mut().enumerate() {
            *slot = step(h, &mut t)?;
        }
    }
    let annual_kwh = hourly_w.iter().sum::<f64>() / 1000.0;
    let (peak_hour, &peak_w) = hourly_w
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).expect("finite"))
        .expect("non-empty");
    Ok(HourlyResult {
        hourly_w,
        annual_kwh,
        peak_w,
        peak_hour,
        phase_shift_h: peak_hour as i64 - COLDEST_HOUR as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coldest_hour_index_matches_report_convention() {
        // Feb 1, hour 13 (0-based index 31*24 + 12) is 417 h after the coldest hour.
        assert_eq!(31 * 24 + 12 - COLDEST_HOUR, 417);
    }
}
