//! Johansen-Kersten soil model: conductivity and heat capacity as functions of saturation S.

use crate::consts::{CV_ICE, CV_WATER, K_ICE, K_WATER, LATENT, RHO_WATER};
use crate::material::Material;

/// Kersten number family: coarse (sand, gravel) or fine (silt, clay, peat) soils.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoilKind {
    Coarse,
    Fine,
}

/// Class parameters (see `presets` for the table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoilParams {
    pub kind: SoilKind,
    /// Conductivity of the solid grains, W/(m K).
    pub k_solid: f64,
    /// Conductivity of the dry soil, W/(m K).
    pub k_dry: f64,
    /// Volumetric heat capacity of the solid grains, J/(m3 K).
    pub cv_solid: f64,
    /// Porosity, 0..1.
    pub porosity: f64,
}

impl SoilParams {
    /// Kersten number for thawed soil.
    fn kersten_thawed(&self, s: f64) -> f64 {
        // Johansen (1975): Ke = 0.7 log S + 1 (coarse, S > 0.05), Ke = log S + 1 (fine, S > 0.1).
        let (a, s_min) = match self.kind {
            SoilKind::Coarse => (0.7, 0.05),
            SoilKind::Fine => (1.0, 0.1),
        };
        if s < s_min {
            0.0
        } else {
            (a * s.log10() + 1.0).max(0.0)
        }
    }

    /// Thawed conductivity at saturation `s` (clamped to 0..1).
    pub fn k_thawed(&self, s: f64) -> f64 {
        let s = s.clamp(0.0, 1.0);
        let k_sat = self.k_solid.powf(1.0 - self.porosity) * K_WATER.powf(self.porosity);
        self.k_dry + (k_sat - self.k_dry) * self.kersten_thawed(s)
    }

    /// Frozen conductivity: Johansen with Ke = S and ice-filled pores.
    pub fn k_frozen(&self, s: f64) -> f64 {
        let s = s.clamp(0.0, 1.0);
        let k_sat = self.k_solid.powf(1.0 - self.porosity) * K_ICE.powf(self.porosity);
        self.k_dry + (k_sat - self.k_dry) * s
    }

    pub fn cv_thawed(&self, s: f64) -> f64 {
        let s = s.clamp(0.0, 1.0);
        (1.0 - self.porosity) * self.cv_solid + s * self.porosity * CV_WATER
    }

    pub fn cv_frozen(&self, s: f64) -> f64 {
        let s = s.clamp(0.0, 1.0);
        (1.0 - self.porosity) * self.cv_solid + s * self.porosity * CV_ICE
    }

    /// Latent heat per m3: all pore water freezes at saturation `s`.
    pub fn latent(&self, s: f64) -> f64 {
        s.clamp(0.0, 1.0) * self.porosity * RHO_WATER * LATENT
    }

    /// Material for a given saturation. `delta` is the phase-change half-width (floored at 0.5 K).
    pub fn material(&self, s: f64, t_freeze: f64, delta: f64) -> Material {
        Material {
            k_thawed: self.k_thawed(s),
            k_frozen: self.k_frozen(s),
            cv_thawed: self.cv_thawed(s),
            cv_frozen: self.cv_frozen(s),
            latent: self.latent(s),
            t_freeze,
            half_width: delta.max(Material::MIN_HALF_WIDTH),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Illustrative numbers for testing the formulas only; not a soil table.
    fn sandy() -> SoilParams {
        SoilParams {
            kind: SoilKind::Coarse,
            k_solid: 7.7,
            k_dry: 0.3,
            cv_solid: 2.0e6,
            porosity: 0.4,
        }
    }

    #[test]
    fn dry_and_saturated_limits() {
        let p = sandy();
        assert!((p.k_thawed(0.0) - p.k_dry).abs() < 1e-12);
        let k_sat = 7.7_f64.powf(0.6) * 0.57_f64.powf(0.4);
        assert!((p.k_thawed(1.0) - k_sat).abs() < 1e-12);
    }

    #[test]
    fn conductivity_rises_with_saturation() {
        let p = sandy();
        let ks: Vec<f64> = (0..=10).map(|i| p.k_thawed(i as f64 / 10.0)).collect();
        assert!(ks.windows(2).all(|w| w[1] >= w[0]));
    }

    #[test]
    fn frozen_is_more_conductive_than_thawed_when_wet() {
        let p = sandy();
        assert!(p.k_frozen(1.0) > p.k_thawed(1.0));
        assert!((p.k_frozen(0.0) - p.k_dry).abs() < 1e-12);
    }

    #[test]
    fn heat_capacity_and_latent_follow_saturation() {
        let p = sandy();
        assert!((p.cv_thawed(0.0) - 0.6 * 2.0e6).abs() < 1e-6);
        assert!(p.cv_thawed(1.0) > p.cv_frozen(1.0));
        assert!((p.latent(1.0) - 0.4 * 1000.0 * 334_000.0).abs() < 1e-3);
        assert_eq!(p.latent(0.0), 0.0);
    }

    #[test]
    fn material_clamps_and_floors() {
        let m = sandy().material(1.5, -0.1, 0.1);
        assert_eq!(m.half_width, 0.5);
        assert_eq!(m.k_thawed, sandy().k_thawed(1.0));
        assert!(m.has_phase_change());
    }
}
