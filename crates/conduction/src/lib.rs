//! Finite-volume conduction coefficients shared by the time stepper and the periodic solver.

mod error;
pub mod line;
mod stencil;

pub use error::{check_property, Error};
pub use stencil::Stencil;
