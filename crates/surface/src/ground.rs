//! The ground-surface balance and its linearisation into a convective boundary condition.

use crate::error::Error;
use crate::weather::{Weather, KELVIN, SIGMA};
use basetherm_bc::Bc;

/// Convective heat transfer coefficient model. No default correlation is baked in: the choice of
/// wind dependence is a documented modelling decision for the user.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Convection {
    /// Fixed `h_c`, W/(m2 K).
    Constant(f64),
    /// `h_c = a + b v` with wind speed `v` in m/s, W/(m2 K).
    WindLinear { a: f64, b: f64 },
    /// `h_c = d + e v + f v^2` with wind speed `v` in m/s, W/(m2 K), convective part only.
    Quadratic { d: f64, e: f64, f: f64 },
}

impl Convection {
    /// Equivalent radiative part of the Walton/ASHRAE combined coefficient, W/(m2 K), as used by
    /// IEA BESTEST (NREL/TP-550-43388, Appendix D). Subtracted so the explicit long-wave term of the
    /// surface balance does not count radiation twice.
    pub const WALTON_RADIATIVE_PART: f64 = 4.63;

    /// Walton/ASHRAE exterior coefficient for a rough surface (brick or rough plaster): the combined
    /// coefficient `12.49 + 4.065 v + 0.028 v^2` minus [`Self::WALTON_RADIATIVE_PART`]. Reproduces the
    /// convective-only values published in the BESTEST report: 11.95 W/m2K at 1 m/s, 99.84 at 19.9 m/s
    /// and 215.26 at 40 m/s. The project default.
    pub const WALTON_ROUGH: Convection = Convection::Quadratic {
        d: 12.49 - Self::WALTON_RADIATIVE_PART,
        e: 4.065,
        f: 0.028,
    };

    pub fn h_c(&self, wind: f64) -> f64 {
        match *self {
            Convection::Constant(h) => h,
            Convection::WindLinear { a, b } => a + b * wind,
            Convection::Quadratic { d, e, f } => d + e * wind + f * wind * wind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundSurface {
    /// Solar reflectance, 0..1.
    pub albedo: f64,
    /// Long-wave emissivity (equal to absorptivity), 0..1.
    pub emissivity: f64,
    pub convection: Convection,
}

impl GroundSurface {
    pub fn new(albedo: f64, emissivity: f64, convection: Convection) -> Result<Self, Error> {
        if !(0.0..=1.0).contains(&albedo) || !albedo.is_finite() {
            return Err(Error::BadParameter("albedo must be in 0..1"));
        }
        if !(0.0..=1.0).contains(&emissivity) || !emissivity.is_finite() {
            return Err(Error::BadParameter("emissivity must be in 0..1"));
        }
        // Finite coefficients, and a positive h_c over the whole EPW wind range (0..=40 m/s).
        let finite = match convection {
            Convection::Constant(h) => h.is_finite(),
            Convection::WindLinear { a, b } => a.is_finite() && b.is_finite(),
            Convection::Quadratic { d, e, f } => d.is_finite() && e.is_finite() && f.is_finite(),
        };
        let ok = finite && (0..=40).all(|v| convection.h_c(f64::from(v)) > 0.0);
        if !ok {
            return Err(Error::BadParameter(
                "convection coefficient must be positive",
            ));
        }
        Ok(Self {
            albedo,
            emissivity,
            convection,
        })
    }

    /// Absorbed solar flux, W/m2.
    pub fn absorbed_solar(&self, w: &Weather) -> f64 {
        (1.0 - self.albedo) * w.ghi
    }

    /// Exact balance: heat flow into the ground at surface temperature `ts` (C), W/m2.
    pub fn flux_in(&self, w: &Weather, ts: f64) -> f64 {
        let (tk, tsk) = (ts + KELVIN, w.t_sky() + KELVIN);
        self.convection.h_c(w.wind) * (w.t_air - ts)
            + self.absorbed_solar(w)
            + self.emissivity * SIGMA * (tsk.powi(4) - tk.powi(4))
    }

    /// Radiative film coefficient linearised about `ts_ref` (C), W/(m2 K); exact at `ts = ts_ref`.
    pub fn h_radiative(&self, w: &Weather, ts_ref: f64) -> f64 {
        let (tk, tsk) = (ts_ref + KELVIN, w.t_sky() + KELVIN);
        self.emissivity * SIGMA * (tk + tsk) * (tk * tk + tsk * tsk)
    }

    /// The balance as a convective boundary: film `h_c + h_r` to an equivalent ambient
    /// temperature that also carries the absorbed solar flux. `ts_ref` is the reference surface
    /// temperature for the radiation linearisation. The boundary flux differs from the exact balance
    /// by `(h_r(ts_ref) - h_r(Ts)) (Tsky - Ts)`: first order in `ts_ref - Ts`. Use the previous
    /// step's surface temperature, then optionally repeat the step with the new one (Picard).
    pub fn bc(&self, w: &Weather, ts_ref: f64) -> Bc {
        let (h, t_inf) = self.film(w, ts_ref);
        Bc::Convective { h, t_inf }
    }

    /// The linearised film as a pair: coefficient `h_c + h_r` (W/m2 K) and equivalent ambient (C).
    pub fn film(&self, w: &Weather, ts_ref: f64) -> (f64, f64) {
        let h_c = self.convection.h_c(w.wind);
        let h_r = self.h_radiative(w, ts_ref);
        let h = h_c + h_r;
        (
            h,
            (h_c * w.t_air + h_r * w.t_sky() + self.absorbed_solar(w)) / h,
        )
    }

    /// Surface temperature at which the net flux into the ground is zero (adiabatic ground), C.
    pub fn equilibrium_temperature(&self, w: &Weather) -> f64 {
        // The flux decreases monotonically with Ts: bisect.
        let (mut lo, mut hi) = (-150.0, 150.0);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if self.flux_in(w, mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }
}

/// Surface (face) temperature from the adjacent cell temperature: the surface sits between the
/// film (`h_film`, to `t_inf`) and the half-cell conductance `h_cell = 2k/width`, both per m2.
pub fn face_temperature(h_film: f64, t_inf: f64, h_cell: f64, t_cell: f64) -> f64 {
    (h_film * t_inf + h_cell * t_cell) / (h_film + h_cell)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface() -> GroundSurface {
        GroundSurface::new(0.25, 0.95, Convection::WindLinear { a: 5.0, b: 4.0 }).unwrap()
    }

    fn weather(t_air: f64, wind: f64, ghi: f64, ir: f64) -> Weather {
        Weather {
            t_air,
            wind,
            ghi,
            ir_sky: ir,
        }
    }

    #[test]
    fn rejects_bad_parameters() {
        assert!(GroundSurface::new(1.2, 0.9, Convection::Constant(10.0)).is_err());
        assert!(GroundSurface::new(0.2, -0.1, Convection::Constant(10.0)).is_err());
        assert!(GroundSurface::new(0.2, 0.9, Convection::Constant(0.0)).is_err());
        assert!(GroundSurface::new(0.2, 0.9, Convection::WindLinear { a: 5.0, b: -1.0 }).is_err());
        assert!(GroundSurface::new(f64::NAN, 0.9, Convection::Constant(5.0)).is_err());
    }

    #[test]
    fn walton_rough_reproduces_the_bestest_convective_values() {
        // NREL/TP-550-43388: convective-only h_ext 11.95 (1 m/s), 99.84 (19.9 m/s), 215.26 (40 m/s).
        for (v, h) in [(1.0, 11.95), (19.9, 99.84), (40.0, 215.26)] {
            assert!(
                (Convection::WALTON_ROUGH.h_c(v) - h).abs() < 0.006,
                "{v}: {}",
                Convection::WALTON_ROUGH.h_c(v)
            );
        }
        assert!(GroundSurface::new(0.25, 0.95, Convection::WALTON_ROUGH).is_ok());
    }

    #[test]
    fn quadratic_must_stay_positive_over_the_wind_range() {
        assert!(GroundSurface::new(
            0.2,
            0.9,
            Convection::Quadratic {
                d: 8.0,
                e: 4.0,
                f: -0.057
            }
        )
        .is_ok());
        assert!(GroundSurface::new(
            0.2,
            0.9,
            Convection::Quadratic {
                d: 8.0,
                e: 0.0,
                f: -0.1
            }
        )
        .is_err());
        assert!(GroundSurface::new(
            0.2,
            0.9,
            Convection::Quadratic {
                d: f64::NAN,
                e: 1.0,
                f: 0.0
            }
        )
        .is_err());
    }

    #[test]
    fn convection_models() {
        assert_eq!(Convection::Constant(8.0).h_c(5.0), 8.0);
        assert_eq!(Convection::WindLinear { a: 5.0, b: 4.0 }.h_c(2.0), 13.0);
    }

    #[test]
    fn robin_flux_equals_exact_balance_at_the_reference_temperature() {
        let s = surface();
        for (ta, v, g, ir) in [
            (5.0, 3.0, 200.0, 300.0),
            (-15.0, 0.0, 0.0, 200.0),
            (30.0, 8.0, 900.0, 420.0),
        ] {
            let w = weather(ta, v, g, ir);
            for ts in [-20.0, -3.0, 0.0, 12.0, 40.0] {
                let Bc::Convective { h, t_inf } = s.bc(&w, ts) else {
                    panic!()
                };
                let robin = h * (t_inf - ts);
                let exact = s.flux_in(&w, ts);
                assert!(
                    (robin - exact).abs() < 1e-9 * (1.0 + exact.abs()),
                    "{robin} vs {exact}"
                );
            }
        }
    }

    #[test]
    fn linearisation_error_is_first_order_in_the_reference_offset() {
        // Robin flux minus exact balance = (h_r(ref) - h_r(Ts)) (Tsky - Ts): first order in the
        // offset, small in size, and removed by re-linearising about the new surface temperature.
        let s = surface();
        let w = weather(5.0, 3.0, 200.0, 300.0);
        let ts = 4.0;
        let err = |d: f64| {
            let Bc::Convective { h, t_inf } = s.bc(&w, ts + d) else {
                panic!()
            };
            (h * (t_inf - ts) - s.flux_in(&w, ts)).abs()
        };
        let ratio = err(4.0) / err(2.0);
        assert!((1.9..2.2).contains(&ratio), "ratio {ratio}");
        assert!(err(2.0) < 0.5, "error at a 2 K offset: {} W/m2", err(2.0));
        assert!(err(0.0) < 1e-9);
    }

    #[test]
    fn equilibrium_temperature_has_zero_net_flux() {
        let s = surface();
        for w in [
            weather(5.0, 3.0, 200.0, 300.0),
            weather(-20.0, 1.0, 0.0, 180.0),
            weather(25.0, 0.0, 700.0, 420.0),
        ] {
            let te = s.equilibrium_temperature(&w);
            assert!(s.flux_in(&w, te).abs() < 1e-8, "{}", s.flux_in(&w, te));
        }
    }

    #[test]
    fn night_surface_is_colder_than_air_and_sun_warms_it() {
        let s = surface();
        let night = s.equilibrium_temperature(&weather(5.0, 1.0, 0.0, 280.0));
        let day = s.equilibrium_temperature(&weather(5.0, 1.0, 600.0, 280.0));
        assert!(night < 5.0 && day > night);
    }

    #[test]
    fn face_temperature_is_between_film_and_cell() {
        let t = face_temperature(10.0, 20.0, 30.0, 0.0);
        assert!((t - 5.0).abs() < 1e-12);
    }
}
