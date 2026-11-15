//! The property set a cell needs: frozen and thawed conductivity and heat capacity, plus latent heat.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Thawed conductivity, W/(m K).
    pub k_thawed: f64,
    /// Frozen conductivity, W/(m K).
    pub k_frozen: f64,
    /// Thawed volumetric heat capacity, J/(m3 K).
    pub cv_thawed: f64,
    /// Frozen volumetric heat capacity, J/(m3 K).
    pub cv_frozen: f64,
    /// Latent heat released on freezing, J/m3 (0 for solids and dry soil).
    pub latent: f64,
    /// Freezing temperature, C.
    pub t_freeze: f64,
    /// Half-width of the phase-change band, K (floor 0.5 K, see CLAUDE.md).
    pub half_width: f64,
}

impl Material {
    /// Smallest allowed half-width of the phase-change band, K.
    pub const MIN_HALF_WIDTH: f64 = 0.5;

    /// Constant-property solid with no phase change (slabs, walls, insulation, BESTEST soil).
    pub fn constant(k: f64, cv: f64) -> Self {
        Self {
            k_thawed: k,
            k_frozen: k,
            cv_thawed: cv,
            cv_frozen: cv,
            latent: 0.0,
            t_freeze: 0.0,
            half_width: Self::MIN_HALF_WIDTH,
        }
    }

    /// Constant solid from conductivity, density and specific heat.
    pub fn from_k_rho_c(k: f64, rho: f64, c: f64) -> Self {
        Self::constant(k, rho * c)
    }

    pub fn with_half_width(mut self, delta: f64) -> Self {
        self.half_width = delta.max(Self::MIN_HALF_WIDTH);
        self
    }

    pub fn has_phase_change(&self) -> bool {
        self.latent > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bestest_soil_is_constant() {
        let m = Material::from_k_rho_c(1.9, 1490.0, 1800.0);
        assert_eq!(m.k_thawed, 1.9);
        assert_eq!(m.k_frozen, 1.9);
        assert!((m.cv_thawed - 2_682_000.0).abs() < 1e-6);
        assert!(!m.has_phase_change());
    }

    #[test]
    fn half_width_has_a_floor() {
        assert_eq!(
            Material::constant(1.0, 1e6).with_half_width(0.1).half_width,
            0.5
        );
        assert_eq!(
            Material::constant(1.0, 1e6).with_half_width(1.5).half_width,
            1.5
        );
    }
}
