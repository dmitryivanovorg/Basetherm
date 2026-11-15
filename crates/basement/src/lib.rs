//! The 3D basement model and its full-year driver.

mod bcstate;
pub mod bench;
mod error;
mod forcing;
pub mod geom;
mod init;
mod mesh;
mod model;
mod results;
mod run;
mod sections;
mod spec;
mod stepper;
mod symmetry;

pub use error::Error;
pub use mesh::{preview_grid, Preset};
pub use model::{Kind, Model, TopKind};
pub use results::{Date, Peak, Peaks, Results, Scope, SymmetryInfo, YearRecord};
pub use run::{run, run_with_progress, Progress, RunOptions, Stage};
pub use sections::Sections;
pub use spec::{
    Building, Domain, HorizontalInsulation, Layer, Level, Limit, Room, Schedule, SoilLayer,
};
pub use symmetry::{detect as detect_symmetry, Mode as SymmetryMode, Symmetry};
