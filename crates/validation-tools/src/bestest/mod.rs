//! BESTEST ground-coupling cases: definitions, model builder, runner.

mod case;
mod hourly;
mod model;
mod run;

pub use case::{cases, Case, Ring, Surface};
pub use hourly::{run_hourly, HourlyResult, COLDEST_HOUR};
pub use model::{surface_bc, Mesh, Model};
pub use run::{annual_kwh, run_mean, MeanResult};
