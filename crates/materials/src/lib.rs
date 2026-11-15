//! Soil classes, Johansen-Kersten k(S), heat capacity, layered soil, solids (slab, wall, insulation).
//!
//! `presets` holds the soil classes (gravel, sand, silt, clay, peat) with their sources.

mod material;
mod presets;
mod soil;

pub use material::Material;
pub use presets::{preset, Preset, PRESETS};
pub use soil::{SoilKind, SoilParams};

/// Physical constants, SI. Standard handbook values.
pub mod consts {
    /// Thermal conductivity of liquid water, W/(m K).
    pub const K_WATER: f64 = 0.57;
    /// Thermal conductivity of ice, W/(m K).
    pub const K_ICE: f64 = 2.2;
    /// Volumetric heat capacity of liquid water, J/(m3 K).
    pub const CV_WATER: f64 = 4.18e6;
    /// Volumetric heat capacity of ice, J/(m3 K).
    pub const CV_ICE: f64 = 1.93e6;
    /// Latent heat of freezing, J/kg (CLAUDE.md: 334 kJ/kg).
    pub const LATENT: f64 = 334_000.0;
    /// Density of water, kg/m3.
    pub const RHO_WATER: f64 = 1000.0;
}
