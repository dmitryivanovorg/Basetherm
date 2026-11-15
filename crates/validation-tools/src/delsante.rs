//! Steady 3D heat flow through a rectangular floor surrounded by a perimeter strip with a linear
//! surface-temperature ramp, on a semi-infinite solid (Delsante, Stokes & Walsh 1983), as given
//! in NREL/TP-550-43388 eqs. GC10a-1 and GC10a-2 (transcribed from the page image, p. 25).

use std::f64::consts::PI;

/// The geometric function `F(L, B, W)` with units of length, m.
pub fn shape_function(l: f64, b: f64, w: f64) -> f64 {
    let (g, d, r2) = (l - b, l + b, 2.0_f64.sqrt());
    let ln1 = (r2 - 1.0).ln();
    let mut t = (2.0 + (l + b) / w) * ((l + w).powi(2) + (b + w).powi(2)).sqrt()
        - r2 * (1.0 + l / (2.0 * w)) * (l * l + (l + 2.0 * w).powi(2)).sqrt();
    t += -r2 * (1.0 + b / (2.0 * w)) * (b * b + (b + 2.0 * w).powi(2)).sqrt()
        - ((l + b) / w) * (l * l + b * b).sqrt();
    t += ((l * l + b * b) / w) * (1.0 + r2 * ln1) + 2.0 * w * (r2 + ln1);
    t -= (r2 * g * g / w)
        * (((g * g + (d + 2.0 * w).powi(2)).sqrt() + d + 2.0 * w) / ((g * g + d * d).sqrt() + d))
            .ln();
    let diag = ((l + w).powi(2) + (b + w).powi(2)).sqrt();
    t += ((g * g - (b + w).powi(2)) / w) * ((diag + b + w) / (l + w)).ln();
    t += ((g * g - (l + w).powi(2)) / w) * ((diag + l + w) / (b + w)).ln();
    let lb = (l * l + b * b).sqrt();
    t += (l * (2.0 * b - l) / w) * ((lb + b) / l).ln()
        + (b * (2.0 * l - b) / w) * ((lb + l) / b).ln();
    let wl = (w * w + (l + w).powi(2)).sqrt();
    let wb = (w * w + (b + w).powi(2)).sqrt();
    t -= ((l * l - w * w) / w) * ((wl + w) / (l + w)).ln();
    t -= ((b * b - w * w) / w) * ((wb + w) / (b + w)).ln();
    t += (2.0 * l + w) * ((wl + l + w) / w).ln() + (2.0 * b + w) * ((wb + b + w) / w).ln();
    t += (r2 * l * l / w) * (((l * l + (l + 2.0 * w).powi(2)).sqrt() + l + 2.0 * w) / l).ln();
    t += (r2 * b * b / w) * (((b * b + (b + 2.0 * w).powi(2)).sqrt() + b + 2.0 * w) / b).ln();
    t
}

/// Heat flow through the floor into the ground, W (positive = from the floor into the ground).
pub fn floor_heat_flow(k: f64, t_in: f64, t_out: f64, l: f64, b: f64, w: f64) -> f64 {
    k * (t_in - t_out) * shape_function(l, b, w) / PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc10a_base_case_matches_the_report() {
        // Report Table (a-series summary): analytic solution 2433 W.
        let q = floor_heat_flow(1.9, 30.0, 10.0, 12.0, 12.0, 0.24);
        assert!((q - 2433.0).abs() < 1.0, "{q}");
    }

    #[test]
    fn scales_linearly_with_conductivity_and_temperature_difference() {
        let q1 = floor_heat_flow(1.0, 30.0, 10.0, 12.0, 4.0, 0.24);
        let q2 = floor_heat_flow(2.0, 40.0, 10.0, 12.0, 4.0, 0.24);
        assert!((q2 / q1 - 3.0).abs() < 1e-12);
    }
}
