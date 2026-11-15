//! One hour of the weather inputs the surface balance needs.

use crate::error::Error;

/// Stefan-Boltzmann constant, W/(m2 K4).
pub const SIGMA: f64 = 5.670_374_419e-8;
/// 0 C in kelvin.
pub const KELVIN: f64 = 273.15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weather {
    /// Dry-bulb air temperature, C.
    pub t_air: f64,
    /// Wind speed, m/s.
    pub wind: f64,
    /// Global horizontal irradiance, W/m2.
    pub ghi: f64,
    /// Horizontal infrared irradiance from the sky, W/m2.
    pub ir_sky: f64,
}

impl Weather {
    pub fn validate(&self) -> Result<(), Error> {
        if !(self.t_air.is_finite() && self.t_air > -90.0 && self.t_air < 70.0) {
            return Err(Error::BadWeather("air temperature"));
        }
        if !(self.wind.is_finite() && self.wind >= 0.0) {
            return Err(Error::BadWeather("wind speed"));
        }
        if !(self.ghi.is_finite() && self.ghi >= 0.0) {
            return Err(Error::BadWeather("global horizontal irradiance"));
        }
        // Real sky infrared lies between about 100 and 550 W/m2; EPW marks missing data with 9999.
        if !(self.ir_sky.is_finite() && (50.0..700.0).contains(&self.ir_sky)) {
            return Err(Error::BadWeather("horizontal infrared irradiance"));
        }
        Ok(())
    }

    /// Effective sky temperature, C.
    pub fn t_sky(&self) -> f64 {
        sky_temperature(self.ir_sky)
    }
}

/// Sky temperature from horizontal infrared irradiance: `(IR / sigma)^(1/4)`, in C.
pub fn sky_temperature(ir: f64) -> f64 {
    (ir / SIGMA).powf(0.25) - KELVIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_temperature_inverts_stefan_boltzmann() {
        let t: f64 = 253.15; // -20 C
        assert!((sky_temperature(SIGMA * t.powi(4)) - (-20.0)).abs() < 1e-9);
    }

    #[test]
    fn rejects_missing_and_unphysical_values() {
        let ok = Weather {
            t_air: 5.0,
            wind: 3.0,
            ghi: 100.0,
            ir_sky: 300.0,
        };
        assert!(ok.validate().is_ok());
        assert!(Weather {
            ir_sky: 9999.0,
            ..ok
        }
        .validate()
        .is_err());
        assert!(Weather { wind: -1.0, ..ok }.validate().is_err());
        assert!(Weather {
            ghi: f64::NAN,
            ..ok
        }
        .validate()
        .is_err());
        assert!(Weather { t_air: 99.0, ..ok }.validate().is_err());
    }
}
