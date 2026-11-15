//! PCG and COCG with a pluggable preconditioner for the periodic solve.
//!
//! The mean field is real SPD (PCG). The annual harmonic `(K + i w C) x = b` is complex-symmetric:
//! COCG is the same iteration with the bilinear (unconjugated) inner product. One generic
//! implementation serves both. Geometric multigrid will be added as a `Precond` if benchmarks need it.

mod dense;
mod grid7;
mod multigrid;
mod pcg;
mod scalar;
mod smooth;

pub use grid7::{Aggregation, Grid7};
pub use multigrid::{Coarsen, Hierarchy, Multigrid, Settings, Smoother};
pub use pcg::{pcg, Identity, LinOp, Outcome, Precond};
pub use scalar::{Scalar, C64};
