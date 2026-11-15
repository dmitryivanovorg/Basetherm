use basetherm_grid::Axis;

/// Hourly undisturbed-ground temperatures for the last simulated year.
///
/// `cell(hour, c)` is the temperature of column cell `c` at the end of hour `hour` (0-based), which
/// is the instant a 3D implicit step covering that hour should see on its far-field boundaries.
#[derive(Debug, Clone)]
pub struct Undisturbed {
    pub(crate) z: Axis,
    pub(crate) k: Vec<f64>,
    pub(crate) temps: Vec<f64>,
    /// Hourly years simulated.
    pub years: usize,
    /// Mean net energy storage over the last year, W/m2 (about zero when converged).
    pub mean_storage: f64,
    /// Mean heat flow into the ground through the surface over the last year, W/m2 (zero for an
    /// adiabatic bottom at convergence).
    pub mean_surface_flux: f64,
    pub converged: bool,
}

impl Undisturbed {
    pub fn cells(&self) -> usize {
        self.z.len()
    }

    pub fn hours(&self) -> usize {
        self.temps.len() / self.z.len()
    }

    pub fn z(&self) -> &Axis {
        &self.z
    }

    /// Temperature of cell `c` at the end of `hour`, C.
    pub fn cell(&self, hour: usize, c: usize) -> f64 {
        self.temps[hour * self.z.len() + c]
    }

    /// Temperature at the interior face `f` (between cells `f-1` and `f`), weighting by the half-cell
    /// conductances `2k/width`, so it is exact for a locally linear profile in a layered soil.
    pub fn face(&self, hour: usize, f: usize) -> f64 {
        assert!(f >= 1 && f < self.z.len(), "interior faces only");
        let g = |c: usize| 2.0 * self.k[c] / self.z.width(c);
        let (g1, g2) = (g(f - 1), g(f));
        (g1 * self.cell(hour, f - 1) + g2 * self.cell(hour, f)) / (g1 + g2)
    }

    /// Annual mean temperature of cell `c`, C.
    pub fn annual_mean(&self, c: usize) -> f64 {
        (0..self.hours()).map(|h| self.cell(h, c)).sum::<f64>() / self.hours() as f64
    }

    /// Half the annual range of cell `c`, K.
    pub fn half_range(&self, c: usize) -> f64 {
        let (lo, hi) = (0..self.hours())
            .map(|h| self.cell(h, c))
            .fold((f64::MAX, f64::MIN), |(lo, hi), t| (lo.min(t), hi.max(t)));
        0.5 * (hi - lo)
    }
}
