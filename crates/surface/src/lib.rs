//! Surface boundary: convection, GHI absorption, ground-to-sky long-wave. SI units, temperatures in C.
//!
//! The balance at the ground surface, heat flow into the ground per unit area:
//! `q = h_c (Ta - Ts) + (1 - albedo) GHI + eps sigma (Tsky^4 - Ts^4)`
//! with `Tsky = (IR / sigma)^(1/4)` from the horizontal infrared irradiance. Evaporation and latent
//! flux are neglected. The radiation term is linearised about a reference surface temperature, so the
//! balance becomes a [`basetherm_bc::Bc::Convective`] boundary that the solver already handles; snow
//! or other covers can later be added as cells or a series resistance without touching the solver.

mod error;
mod ground;
mod weather;

pub use error::Error;
pub use ground::{face_temperature, Convection, GroundSurface};
pub use weather::{sky_temperature, Weather, KELVIN, SIGMA};
