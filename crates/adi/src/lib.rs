//! Douglas-Gunn ADI sweeps (tridiagonal), implicit Euler time stepping.
//!
//! Finite volume on the graded grid. Properties (`k`, `cv`) are per cell and fixed during a step;
//! the freezing update changes them between steps. SI units throughout.

mod kernel;
mod par;
mod stepper;
mod thomas;

pub use basetherm_conduction::Error as AdiError;
pub use stepper::Adi;
