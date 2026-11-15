//! Structured graded rectilinear grid, mesh builder and presets (Draft/Standard/Fine), symmetry detection.

// Guards like `!(a > b)` are deliberate: they also reject NaN.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

mod axis;
mod graded;
mod grid;

pub use axis::{Axis, AxisError};
pub use graded::GradingSpec;
pub use grid::Grid;
