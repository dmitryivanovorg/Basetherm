//! Timing of the pieces of a run, for the benchmark program (not needed to use the model).

use crate::error::Error;
use crate::forcing::Series;
use crate::model::Model;
use crate::run::{setup, RunOptions, YearRun};
use basetherm_bc::Fluxes;
use basetherm_surface::Weather;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Timing {
    pub cells: usize,
    pub dims: [usize; 3],
    pub freezing: bool,
    pub farfield_s: f64,
    pub init_s: f64,
    /// Mean time of one hourly step, ms.
    pub step_ms: f64,
    pub steps: usize,
    /// Mean ms per step spent on forcing and boundary values, in the stepper, and on the surface state.
    pub phases_ms: [f64; 3],
    /// Sum of all temperatures after the steps: identical runs must give identical values.
    pub checksum: f64,
}

/// Set up a run, then time `steps` hourly steps (starting at hour 0).
pub fn time_steps(
    model: &Model,
    weather: &[Weather],
    opts: &RunOptions,
    steps: usize,
) -> Result<Timing, Error> {
    let s = setup(model, weather, opts)?;
    let room_specs = model.spec.rooms();
    let schedules: Vec<&crate::spec::Schedule> =
        room_specs.iter().map(|(_, r)| &r.temperature).collect();
    let series = Series {
        weather,
        far: &s.far,
        nz: model.grid.z.len(),
        rooms: schedules,
    };
    let (seconds, freezing) = (s.seconds, s.freezing);
    let (mut t, mut stepper, mut bcs, room_faces) = (s.t, s.stepper, s.bcs, s.room_faces);
    let mut yr = YearRun {
        model,
        series: &series,
        stepper: &mut stepper,
        bcs: &mut bcs,
        flux: Fluxes::with_groups(&model.grid, model.interfaces.groups()),
        t: &mut t,
        room_faces,
    };
    let clock = Instant::now();
    let phases = yr.hours(0..steps)?;
    let step_ms = 1000.0 * clock.elapsed().as_secs_f64() / steps as f64;
    let checksum = yr.t.iter().sum();
    Ok(Timing {
        cells: model.grid.cell_count(),
        dims: model.grid.dims(),
        freezing,
        farfield_s: seconds.0,
        init_s: seconds.1,
        step_ms,
        steps,
        phases_ms: phases.map(|p| 1000.0 * p / steps as f64),
        checksum,
    })
}
