//! The full-year run: far-field column, periodic initial state, coarse (daily) warm-up, then hourly
//! reporting years until the peak hourly loss stops drifting. Results are per room, floor and walls.

use crate::bcstate::BcState;
use crate::error::Error;
use crate::forcing::Series;
use crate::init::{initial_state, undisturbed_state, InitInput};
use crate::model::{Model, TopKind};
use crate::results::{Results, SymmetryInfo, YearRecord};
use crate::sections::SectionAcc;
use crate::stepper::Stepper;
use basetherm_bc::{Fluxes, Side};
use basetherm_farfield::{solve as solve_far, Bottom, ColumnSpec, Options as FarOptions};
use basetherm_grid::Axis;
use basetherm_surface::{GroundSurface, Weather};

const HOURS: usize = 8760;
/// The far-field column is extended below the 3D domain to this depth, m.
const COLUMN_DEPTH: f64 = 100.0;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub ground: GroundSurface,
    /// Surface properties of exposed horizontal insulation (defaults to the ground's).
    pub insulation: Option<GroundSurface>,
    pub farfield: FarOptions,
    /// Most years of daily-step warm-up (0 switches the warm-up off).
    pub coarse_years: usize,
    /// Most hourly reporting years.
    pub hourly_years: usize,
    /// Report only when the peak hourly heat loss changes by less than this fraction between
    /// hourly years (daily warm-up years use the annual heat flow, at half of it).
    pub drift_tol: f64,
    /// Start from the periodic solution (mean plus annual harmonic). Off: every column starts at
    /// the undisturbed ground profile.
    pub periodic_init: bool,
    /// Couple the sides and bottom of the domain to the undisturbed ground column. Off: they are
    /// adiabatic.
    pub condition_edges: bool,
}

impl RunOptions {
    pub fn new(ground: GroundSurface) -> Self {
        Self {
            ground,
            insulation: None,
            farfield: FarOptions::default(),
            coarse_years: 20,
            hourly_years: 3,
            drift_tol: 0.001,
            periodic_init: true,
            condition_edges: true,
        }
    }
}

/// Where a run is, for progress reporting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    /// Far-field column and periodic initial state.
    Setup,
    /// Daily-step warm-up, 1-based year.
    Coarse(usize),
    /// Hourly reporting year, 1-based.
    Hourly(usize),
}

/// A progress report: the stage and the fraction of the current year done (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub stage: Stage,
    pub fraction: f64,
}

/// Wall-clock timer for the timing figures. `std::time::Instant` panics on wasm32-unknown-unknown,
/// so there it always reads zero.
#[derive(Clone, Copy)]
pub(crate) struct Clock(#[cfg(not(target_arch = "wasm32"))] std::time::Instant);

impl Clock {
    pub(crate) fn now() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        return Self(std::time::Instant::now());
        #[cfg(target_arch = "wasm32")]
        Self()
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        #[cfg(not(target_arch = "wasm32"))]
        return self.0.elapsed();
        #[cfg(target_arch = "wasm32")]
        std::time::Duration::ZERO
    }
}

fn run_err(e: impl std::fmt::Display) -> Error {
    Error::Run(e.to_string())
}

/// The undisturbed column: the 3D z-axis extended downward, soil properties by depth.
fn column(model: &Model, freezing: bool) -> Result<ColumnSpec, Error> {
    let faces = model.grid.z.faces();
    let mut z = faces.to_vec();
    let mut w = model.grid.z.width(model.grid.z.len() - 1).max(0.5);
    while *z.last().expect("faces") < COLUMN_DEPTH {
        w *= 1.3;
        z.push((z.last().expect("faces") + w).min(COLUMN_DEPTH));
    }
    let axis = Axis::from_faces(z).map_err(run_err)?;
    let soil = &model.spec.soil;
    let layer = |c: usize| {
        soil.iter()
            .rposition(|s| s.top <= axis.center(c))
            .unwrap_or(0)
    };
    let mats: Vec<_> = soil.iter().map(|s| s.material).collect();
    let cell: Vec<u16> = (0..axis.len()).map(|c| layer(c) as u16).collect();
    if freezing {
        ColumnSpec::with_freezing(axis, mats, cell, Bottom::Adiabatic).map_err(run_err)
    } else {
        let k = cell
            .iter()
            .map(|&m| mats[usize::from(m)].k_thawed)
            .collect();
        let cv = cell
            .iter()
            .map(|&m| mats[usize::from(m)].cv_thawed)
            .collect();
        Ok(ColumnSpec {
            z: axis,
            k,
            cv,
            bottom: Bottom::Adiabatic,
            freezing: None,
        })
    }
}

/// One year of stepping; returns the annual energy per group (J) and, when `record` is set, the
/// hourly flows (W).
pub(crate) struct YearRun<'a> {
    pub model: &'a Model,
    pub series: &'a Series<'a>,
    pub stepper: &'a mut Stepper,
    pub bcs: &'a mut BcState,
    pub flux: Fluxes,
    pub t: &'a mut Vec<f64>,
    pub room_faces: Vec<Vec<usize>>,
}

impl YearRun<'_> {
    /// Step the given 0-based hours of the year (no recording); for timing. Returns the seconds spent
    /// building the forcing and boundary values, in the stepper, and updating the surface state.
    pub(crate) fn hours(&mut self, range: std::ops::Range<usize>) -> Result<[f64; 3], Error> {
        let mut spent = [0.0; 3];
        let lap = |t: &mut Clock, slot: &mut f64| {
            *slot += t.elapsed().as_secs_f64();
            *t = Clock::now();
        };
        for h in range {
            let mut t0 = Clock::now();
            let f = self.series.hourly(h);
            self.bcs.apply(self.model, &f);
            self.stepper.set_room_temps(&f.rooms);
            lap(&mut t0, &mut spent[0]);
            self.stepper
                .step(self.t, 3_600.0, &self.bcs.bc, &mut self.flux)
                .map_err(run_err)?;
            lap(&mut t0, &mut spent[1]);
            self.bcs.after_step(self.model, self.t);
            lap(&mut t0, &mut spent[2]);
        }
        Ok(spent)
    }

    fn year(
        &mut self,
        daily: bool,
        mut record: Option<&mut Vec<f64>>,
        mut sections: Option<&mut SectionAcc>,
        report: &mut dyn FnMut(f64),
    ) -> Result<Vec<f64>, Error> {
        let groups = self.flux.groups().len();
        let mut energy = vec![0.0; groups];
        let (steps, dt) = if daily {
            (365, 86_400.0)
        } else {
            (HOURS, 3_600.0)
        };
        for s in 0..steps {
            if s % 24 == 0 {
                report(s as f64 / steps as f64);
            }
            let f = if daily {
                self.series.daily(s)
            } else {
                self.series.hourly(s)
            };
            self.bcs.apply(self.model, &f);
            self.stepper.set_room_temps(&f.rooms);
            self.stepper
                .step(self.t, dt, &self.bcs.bc, &mut self.flux)
                .map_err(run_err)?;
            self.bcs.after_step(self.model, self.t);
            if let Some(acc) = sections.as_deref_mut() {
                acc.add(self.model, self.t, s);
            }
            let mut flows = self.flux.groups().to_vec();
            for (r, faces) in self.room_faces.iter().enumerate() {
                flows[2 * r] += faces
                    .iter()
                    .map(|&fc| self.flux.side(Side::ZLo)[fc])
                    .sum::<f64>();
            }
            // Undo the mirror-symmetry folding: flows of the whole building.
            let flows = self.model.expand_flows(&flows);
            for (e, q) in energy.iter_mut().zip(&flows) {
                *e += q * dt;
            }
            if let Some(rec) = record.as_deref_mut() {
                rec.extend_from_slice(&flows);
            }
        }
        Ok(energy)
    }
}

/// Peak hourly loss of the whole basement (the most negative hourly sum over all groups), W.
fn peak_loss(hourly: &[f64], groups: usize) -> f64 {
    hourly
        .chunks(groups)
        .map(|h| h.iter().sum::<f64>())
        .fold(0.0, f64::min)
}

/// Relative change between two peaks.
fn peak_drift(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        0.0
    } else {
        (a - b).abs() / b.abs()
    }
}

fn drift(a: &[f64], b: &[f64]) -> f64 {
    let scale: f64 = b.iter().map(|x| x.abs()).sum();
    if scale == 0.0 {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f64>() / scale
}

/// Everything a run needs before it starts stepping.
pub(crate) struct Setup {
    pub far: basetherm_farfield::Undisturbed,
    pub t: Vec<f64>,
    pub stepper: Stepper,
    pub bcs: BcState,
    pub room_faces: Vec<Vec<usize>>,
    pub freezing: bool,
    /// Seconds spent on the far-field column and on the periodic initial state.
    pub seconds: (f64, f64),
}

/// Far-field column, periodic initial state, stepper and boundary state.
pub(crate) fn setup(model: &Model, weather: &[Weather], opts: &RunOptions) -> Result<Setup, Error> {
    if weather.len() != HOURS {
        return Err(Error::Run("weather must have 8760 hourly values".into()));
    }
    let freezing = model.materials.iter().any(|m| m.latent > 0.0);
    let insulation = opts.insulation.unwrap_or(opts.ground);
    let clock = Clock::now();
    let far = solve_far(
        &column(model, freezing)?,
        &opts.ground,
        weather,
        &opts.farfield,
    )
    .map_err(run_err)?;
    let t_far = clock.elapsed().as_secs_f64();
    let ts_ref = far.annual_mean(0);
    let init = InitInput {
        model,
        weather,
        far: &far,
        ground: &opts.ground,
        insulation: &insulation,
        ts_ref,
        freezing,
        condition_edges: opts.condition_edges,
    };
    let clock = Clock::now();
    let t = if opts.periodic_init {
        initial_state(&init)?
    } else {
        undisturbed_state(model, &far)
    };
    let t_init = clock.elapsed().as_secs_f64();
    let stepper = Stepper::new(model, &t, freezing)?;
    let bcs = BcState::new(model, opts.ground, insulation, ts_ref, opts.condition_edges);
    let mut room_faces = vec![Vec::new(); model.rooms.len()];
    for (face, k) in model.top.iter().enumerate() {
        if let TopKind::Room(r) = k {
            room_faces[*r].push(face);
        }
    }
    Ok(Setup {
        far,
        t,
        stepper,
        bcs,
        room_faces,
        freezing,
        seconds: (t_far, t_init),
    })
}

/// Run the model for one weather year (8760 hourly values).
pub fn run(model: &Model, weather: &[Weather], opts: &RunOptions) -> Result<Results, Error> {
    run_with_progress(model, weather, opts, &mut |_| {})
}

/// Like [`run`], reporting progress about once per simulated day (hourly years) or per year
/// (daily warm-up years). Results do not depend on the callback.
pub fn run_with_progress(
    model: &Model,
    weather: &[Weather],
    opts: &RunOptions,
    progress: &mut dyn FnMut(Progress),
) -> Result<Results, Error> {
    progress(Progress {
        stage: Stage::Setup,
        fraction: 0.0,
    });
    let Setup {
        far,
        mut t,
        mut stepper,
        mut bcs,
        room_faces,
        ..
    } = setup(model, weather, opts)?;
    let nz = model.grid.z.len();
    let room_specs = model.spec.rooms();
    let schedules: Vec<&crate::spec::Schedule> =
        room_specs.iter().map(|(_, r)| &r.temperature).collect();
    let series = Series {
        weather,
        far: &far,
        nz,
        rooms: schedules,
    };
    let groups = model.interfaces.groups();
    let mut yr = YearRun {
        model,
        series: &series,
        stepper: &mut stepper,
        bcs: &mut bcs,
        flux: Fluxes::with_groups(&model.grid, groups),
        t: &mut t,
        room_faces,
    };

    // Coarse warm-up with daily steps until the annual flow stops changing (skipped when
    // `coarse_years` is 0).
    let mut at = |stage, fraction| progress(Progress { stage, fraction });
    let total = |e: &[f64]| e.iter().sum::<f64>() / 3.6e6;
    let mut history = Vec::new();
    let mut prev: Option<Vec<f64>> = None;
    let mut coarse_years = 0;
    while coarse_years < opts.coarse_years {
        at(Stage::Coarse(coarse_years + 1), 0.0);
        let e = yr.year(true, None, None, &mut |_| {})?;
        coarse_years += 1;
        let d = prev.as_deref().map(|p| drift(&e, p));
        history.push(YearRecord {
            hourly: false,
            kwh: total(&e),
            peak: None,
            drift: d,
        });
        let done = d.is_some_and(|d| d < 0.5 * opts.drift_tol);
        prev = Some(e);
        if done {
            break;
        }
    }

    // Hourly reporting years.
    let mut hourly = Vec::new();
    let (mut d, mut years) = (f64::INFINITY, 0);
    let mut kwh = Vec::new();
    let mut prev_peak: Option<f64> = None;
    let mut sections = None;
    while years < opts.hourly_years {
        hourly.clear();
        let mut acc = SectionAcc::new(model);
        let e = yr.year(false, Some(&mut hourly), Some(&mut acc), &mut |f| {
            at(Stage::Hourly(years + 1), f)
        })?;
        sections = Some(acc.finish(model));
        years += 1;
        // Hourly years converge on the peak loss, so the first one has nothing to compare with.
        let peak = peak_loss(&hourly, groups);
        let dr = prev_peak.map(|p| peak_drift(peak, p));
        d = dr.unwrap_or(f64::INFINITY);
        prev_peak = Some(peak);
        history.push(YearRecord {
            hourly: true,
            kwh: total(&e),
            peak: Some(peak),
            drift: dr,
        });
        kwh = e.iter().map(|j| j / 3.6e6).collect();
        if d < opts.drift_tol {
            break;
        }
    }
    Ok(Results {
        room_names: model.rooms.iter().map(|r| r.name.clone()).collect(),
        groups,
        hourly,
        annual_kwh: kwh,
        drift: d,
        converged: d < opts.drift_tol,
        coarse_years,
        hourly_years: years,
        symmetry: SymmetryInfo {
            symmetry: model.symmetry,
            notes: model.symmetry_notes.clone(),
        },
        sections: sections.unwrap_or_default(),
        history,
    })
}
