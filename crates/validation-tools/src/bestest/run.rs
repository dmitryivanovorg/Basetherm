//! Runs the mean (steady) solve for a case and reports floor heat flow.

use super::case::Case;
use super::model::{Mesh, Model};
use basetherm_bc::Side;
use basetherm_periodic::{solve, Amplitudes, Error, Params};

#[derive(Debug, Clone, Copy)]
pub struct MeanResult {
    /// Heat flow through the whole floor, W, in the project's convention: loss is negative.
    pub floor_w: f64,
    pub cells: usize,
    pub iterations: usize,
}

/// Steady (mean) floor heat flow for outdoor temperature `t_out`.
pub fn run_mean(case: &Case, mesh: &Mesh, t_out: f64) -> Result<MeanResult, Error> {
    let model = Model::build(case, mesh, t_out).map_err(|_| Error::BadParams("mesh"))?;
    let forcing = Amplitudes::zeros(&model.grid);
    let sol = solve(
        &model.grid,
        &model.k,
        &model.cv,
        &model.bc,
        &forcing,
        Params::annual(),
    )?;
    let (mean, _) = sol.boundary_flux(Side::ZLo);
    // Quarter model: x4. Outflow through the slab is negative when the building loses heat.
    let floor_w = 4.0 * model.slab_faces.iter().map(|&f| mean[f]).sum::<f64>();
    Ok(MeanResult {
        floor_w,
        cells: model.grid.cell_count(),
        iterations: sol.mean_outcome.iterations,
    })
}

/// Annual floor loss, kWh (positive number), from the mean flux: the harmonic parts of a linear,
/// constant-property problem average to zero over a year.
pub fn annual_kwh(floor_w: f64) -> f64 {
    -floor_w * 8760.0 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annual_total_from_mean_flux() {
        assert!((annual_kwh(-2500.0) - 21900.0).abs() < 1e-9);
    }
}
