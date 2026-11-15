//! 1D undisturbed-ground column with its own spin-up; supplies the temperature of the far-field sides
//! and bottom of the 3D domain.
//!
//! The column uses the same vertical grid and soil properties as the 3D domain (extended deeper) and
//! is driven by the same hourly surface balance. With an adiabatic bottom the steady state is found by
//! searching for the flat deep temperature at which the mean surface flux is zero (see `run.rs`). Test-only
//! adiabatic far-field and fixed-temperature-bottom options live with the validation tools, not here.

mod column;
mod error;
mod init;
mod profile;
mod run;

pub use column::{Bottom, ColumnSpec, Freezing, Options};
pub use error::Error;
pub use init::linearised_film;
pub use profile::Undisturbed;
pub use run::solve;

/// Hours in the (365-day) weather year.
pub const HOURS: usize = 8760;
