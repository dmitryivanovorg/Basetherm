//! Enthalpy, temperature, conductivity and capacity of one soil as functions of temperature.

use basetherm_materials::Material;

/// Unfrozen fraction rises linearly from 0 at `t_lo` to 1 at `t_hi`. Capacity and conductivity blend
/// with it, and the latent heat is spread over the band.
///
/// `H(T)` (J/m3, arbitrary zero) is
/// - below the band:  `h_lo + cv_f (T - t_lo)`
/// - in the band, `x = T - t_lo`:  `h_lo + b x + a x^2`, `b = cv_f + L/(2 delta)`, `a = (cv_t - cv_f)/(4 delta)`
/// - above the band:  `h_hi + cv_t (T - t_hi)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnthalpyCurve {
    t_lo: f64,
    t_hi: f64,
    h_lo: f64,
    h_hi: f64,
    cv_f: f64,
    cv_t: f64,
    k_f: f64,
    k_t: f64,
    a: f64,
    b: f64,
}

impl EnthalpyCurve {
    pub fn from_material(m: &Material) -> Self {
        let d = m.half_width.max(Material::MIN_HALF_WIDTH);
        let (t_lo, t_hi) = (m.t_freeze - d, m.t_freeze + d);
        let a = (m.cv_thawed - m.cv_frozen) / (4.0 * d);
        let b = m.cv_frozen + m.latent / (2.0 * d);
        let h_lo = m.cv_frozen * t_lo;
        let h_hi = h_lo + b * 2.0 * d + a * 4.0 * d * d;
        Self {
            t_lo,
            t_hi,
            h_lo,
            h_hi,
            cv_f: m.cv_frozen,
            cv_t: m.cv_thawed,
            k_f: m.k_frozen,
            k_t: m.k_thawed,
            a,
            b,
        }
    }

    pub fn band(&self) -> (f64, f64) {
        (self.t_lo, self.t_hi)
    }

    /// Energy needed to cross the whole band, J/m3.
    pub fn band_energy(&self) -> f64 {
        self.h_hi - self.h_lo
    }

    pub fn enthalpy(&self, t: f64) -> f64 {
        if t <= self.t_lo {
            self.h_lo + self.cv_f * (t - self.t_lo)
        } else if t >= self.t_hi {
            self.h_hi + self.cv_t * (t - self.t_hi)
        } else {
            let x = t - self.t_lo;
            self.h_lo + self.b * x + self.a * x * x
        }
    }

    /// Exact inverse of [`Self::enthalpy`].
    pub fn temperature(&self, h: f64) -> f64 {
        if h <= self.h_lo {
            self.t_lo + (h - self.h_lo) / self.cv_f
        } else if h >= self.h_hi {
            self.t_hi + (h - self.h_hi) / self.cv_t
        } else {
            // a x^2 + b x = c, solved without cancellation: x = 2c / (b + sqrt(b^2 + 4 a c)).
            let c = h - self.h_lo;
            self.t_lo + 2.0 * c / (self.b + (self.b * self.b + 4.0 * self.a * c).sqrt())
        }
    }

    pub fn unfrozen_fraction(&self, t: f64) -> f64 {
        ((t - self.t_lo) / (self.t_hi - self.t_lo)).clamp(0.0, 1.0)
    }

    pub fn conductivity(&self, t: f64) -> f64 {
        self.k_f + (self.k_t - self.k_f) * self.unfrozen_fraction(t)
    }

    /// Apparent heat capacity `dH/dT`, J/(m3 K).
    pub fn tangent_capacity(&self, t: f64) -> f64 {
        if t <= self.t_lo {
            self.cv_f
        } else if t >= self.t_hi {
            self.cv_t
        } else {
            self.b + 2.0 * self.a * (t - self.t_lo)
        }
    }

    /// Slope of `H` between two temperatures (the tangent when they coincide).
    pub fn secant_capacity(&self, t0: f64, t1: f64) -> f64 {
        if (t1 - t0).abs() < 1e-9 {
            self.tangent_capacity(t0)
        } else {
            (self.enthalpy(t1) - self.enthalpy(t0)) / (t1 - t0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soil() -> Material {
        Material {
            k_thawed: 1.5,
            k_frozen: 2.2,
            cv_thawed: 3.0e6,
            cv_frozen: 2.0e6,
            latent: 1.0e8,
            t_freeze: -0.2,
            half_width: 0.5,
        }
    }

    #[test]
    fn enthalpy_is_continuous_and_strictly_increasing() {
        let c = EnthalpyCurve::from_material(&soil());
        let ts: Vec<f64> = (-400..=400).map(|i| f64::from(i) * 0.01).collect();
        assert!(ts.windows(2).all(|w| c.enthalpy(w[1]) > c.enthalpy(w[0])));
        let (lo, hi) = c.band();
        assert!((lo + 0.7).abs() < 1e-12 && (hi - 0.3).abs() < 1e-12);
        // Continuity at both band edges.
        for edge in [lo, hi] {
            assert!((c.enthalpy(edge + 1e-9) - c.enthalpy(edge - 1e-9)).abs() < 1.0);
        }
    }

    #[test]
    fn inverse_round_trips_everywhere() {
        let c = EnthalpyCurve::from_material(&soil());
        for i in -2000..=2000 {
            let t = f64::from(i) * 0.005;
            assert!((c.temperature(c.enthalpy(t)) - t).abs() < 1e-9, "t = {t}");
        }
    }

    #[test]
    fn latent_heat_equals_the_band_energy_minus_sensible_part() {
        let m = soil();
        let c = EnthalpyCurve::from_material(&m);
        // Band energy = latent + mean(cv) over the band (capacity blends linearly).
        let sensible = 0.5 * (m.cv_frozen + m.cv_thawed) * 1.0;
        assert!((c.band_energy() - (m.latent + sensible)).abs() < 1e-3 * m.latent);
    }

    #[test]
    fn tangent_matches_the_numerical_slope_and_secant_matches_endpoints() {
        let c = EnthalpyCurve::from_material(&soil());
        for t in [-3.0, -0.5, -0.2, 0.1, 2.0] {
            let num = (c.enthalpy(t + 1e-6) - c.enthalpy(t - 1e-6)) / 2e-6;
            assert!((c.tangent_capacity(t) - num).abs() < 1e-3 * num, "t = {t}");
        }
        let s = c.secant_capacity(-2.0, 2.0);
        assert!((s * 4.0 - (c.enthalpy(2.0) - c.enthalpy(-2.0))).abs() < 1e-6);
        assert_eq!(c.secant_capacity(1.0, 1.0), c.tangent_capacity(1.0));
    }

    #[test]
    fn conductivity_blends_across_the_band() {
        let c = EnthalpyCurve::from_material(&soil());
        assert_eq!(c.conductivity(-5.0), 2.2);
        assert_eq!(c.conductivity(5.0), 1.5);
        assert!((c.conductivity(-0.2) - 1.85).abs() < 1e-12);
    }

    #[test]
    fn constant_material_is_a_straight_line() {
        let c = EnthalpyCurve::from_material(&Material::constant(1.9, 2.682e6));
        for t in [-10.0, 0.0, 7.0] {
            assert!((c.enthalpy(t) - c.enthalpy(0.0) - 2.682e6 * t).abs() < 1e-3);
            assert_eq!(c.tangent_capacity(t), 2.682e6);
        }
    }

    #[test]
    fn also_works_when_the_thawed_capacity_is_smaller() {
        let m = Material {
            cv_thawed: 1.5e6,
            cv_frozen: 2.5e6,
            ..soil()
        };
        let c = EnthalpyCurve::from_material(&m);
        for i in -300..=300 {
            let t = f64::from(i) * 0.01;
            assert!((c.temperature(c.enthalpy(t)) - t).abs() < 1e-9);
        }
    }
}
