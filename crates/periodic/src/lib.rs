//! Direct mean + annual-harmonic initialisation, linearisation, coarse spin-up driver.
//!
//! This crate solves the linear, periodic-steady-state problem
//! `T(x,t) = T0(x) + Re(theta(x) e^{i w t})` for given per-cell properties and boundary forcing:
//! the mean field solves `A T0 = s0` (real SPD, PCG) and the harmonic `(A + i w C) theta = s1`
//! (complex-symmetric, COCG), both with a z-line block preconditioner.

mod amplitudes;
mod error;
mod precond;
mod solve;

pub use amplitudes::Amplitudes;
pub use error::Error;
pub use solve::{solve, solve_with, Params, PrecondKind, Solution};
