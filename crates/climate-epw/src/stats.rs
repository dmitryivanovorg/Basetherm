//! Series helpers: daily means (coarse spin-up forcing) and the annual harmonic (periodic initialisation).

use std::f64::consts::PI;

/// `x(t) = mean + amp_re cos(w t) - amp_im sin(w t)`, i.e. `mean + Re((amp_re + i amp_im) e^{i w t})`,
/// with `t` in seconds from Jan 1 00:00 and `w = 2 pi / (n hours)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Harmonic {
    pub mean: f64,
    pub amp_re: f64,
    pub amp_im: f64,
}

impl Harmonic {
    pub fn magnitude(&self) -> f64 {
        self.amp_re.hypot(self.amp_im)
    }
}

/// Project an hourly series onto its annual harmonic; hour `i` is placed at its mid-hour time.
pub fn annual_harmonic(series: &[f64]) -> Harmonic {
    let n = series.len() as f64;
    let w = 2.0 * PI / n; // rad per hour
    let mean = series.iter().sum::<f64>() / n;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &x) in series.iter().enumerate() {
        let phase = w * (i as f64 + 0.5);
        re += (x - mean) * phase.cos();
        im -= (x - mean) * phase.sin();
    }
    Harmonic {
        mean,
        amp_re: 2.0 * re / n,
        amp_im: 2.0 * im / n,
    }
}

/// Mean of each 24-hour day (a 365-value series for an 8760-hour year).
pub fn daily_means(series: &[f64]) -> Vec<f64> {
    series
        .chunks(24)
        .map(|d| d.iter().sum::<f64>() / d.len() as f64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harmonic_fit_recovers_a_pure_annual_cosine() {
        let w = 2.0 * PI / 8760.0;
        let series: Vec<f64> = (0..8760)
            .map(|i| 10.0 - 6.0 * (w * ((i as f64 + 0.5) - 14.0 * 24.0 - 3.5)).cos())
            .collect();
        let h = annual_harmonic(&series);
        assert!((h.mean - 10.0).abs() < 1e-9);
        assert!((h.magnitude() - 6.0).abs() < 1e-9, "{}", h.magnitude());
        // Reconstruct the series at mid-hour times.
        let t = 100.5;
        let rebuilt = h.mean + h.amp_re * (w * t).cos() - h.amp_im * (w * t).sin();
        assert!((rebuilt - series[100]).abs() < 1e-9);
    }

    #[test]
    fn diurnal_cycle_does_not_leak_into_the_annual_harmonic() {
        let series: Vec<f64> = (0..8760)
            .map(|i| 5.0 + 3.0 * (2.0 * PI * (i as f64 + 0.5) / 24.0).sin())
            .collect();
        let h = annual_harmonic(&series);
        assert!((h.mean - 5.0).abs() < 1e-9 && h.magnitude() < 1e-9);
    }

    #[test]
    fn daily_means_have_one_value_per_day() {
        let series: Vec<f64> = (0..8760).map(|i| (i / 24) as f64).collect();
        let d = daily_means(&series);
        assert_eq!(d.len(), 365);
        assert_eq!(d[0], 0.0);
        assert_eq!(d[364], 364.0);
    }
}
