//! Enthalpy H(T) with a piecewise-linear latent band, and a conservative non-iterative time step.
//!
//! Freezing is a smoothed phase change over `T_f +/- delta` (delta >= 0.5 K). Each step runs the ADI
//! solve with a lagged secant capacity, then converts the cell's net heat input to temperature
//! through the exact inverse of `H(T)`, so energy is conserved exactly in enthalpy. There is no
//! Picard loop; one optional repeat is made when a cell moves a large part of its latent band.

mod curve;
mod stepper;

pub use curve::EnthalpyCurve;
pub use stepper::{FreezingStepper, StepInfo};
