//! The time stepper: plain conduction, or conduction with freezing, behind one interface.

use crate::error::Error;
use crate::model::Model;
use basetherm_adi::Adi;
use basetherm_bc::{Boundaries, Fluxes};
use basetherm_freezing::FreezingStepper;

pub enum Stepper {
    Linear(Box<Adi>),
    Freezing(Box<FreezingStepper>),
}

fn run_err(e: impl std::fmt::Display) -> Error {
    Error::Run(e.to_string())
}

impl Stepper {
    /// `t0` is only used to initialise the freezing properties.
    pub fn new(model: &Model, t0: &[f64], freezing: bool) -> Result<Self, Error> {
        let cells = &model.cell_material;
        let interfaces = model.interfaces.clone();
        if freezing {
            let s = FreezingStepper::with_interfaces(
                model.grid.clone(),
                &model.materials,
                cells.clone(),
                t0,
                interfaces,
            )
            .map_err(run_err)?;
            Ok(Stepper::Freezing(Box::new(s)))
        } else {
            let k = cells
                .iter()
                .map(|&m| model.materials[usize::from(m)].k_thawed)
                .collect();
            let cv = cells
                .iter()
                .map(|&m| model.materials[usize::from(m)].cv_thawed)
                .collect();
            let a = Adi::with_interfaces(model.grid.clone(), k, cv, interfaces).map_err(run_err)?;
            Ok(Stepper::Linear(Box::new(a)))
        }
    }

    /// Set the interior temperature of every room (floor and wall groups alike).
    pub fn set_room_temps(&mut self, temps: &[f64]) {
        let ifc = match self {
            Stepper::Linear(a) => a.interfaces_mut(),
            Stepper::Freezing(f) => f.interfaces_mut(),
        };
        if let Some(ifc) = ifc {
            for (r, &t) in temps.iter().enumerate() {
                ifc.set_temperature(2 * r, t);
                ifc.set_temperature(2 * r + 1, t);
            }
        }
    }

    pub fn step(
        &mut self,
        t: &mut [f64],
        dt: f64,
        bc: &Boundaries,
        flux: &mut Fluxes,
    ) -> Result<(), Error> {
        match self {
            Stepper::Linear(a) => a.step(t, dt, bc, flux).map_err(run_err),
            Stepper::Freezing(f) => f.step(t, dt, bc, flux).map(|_| ()).map_err(run_err),
        }
    }
}
