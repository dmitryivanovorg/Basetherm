//! Monthly mean temperature and heat flux on the two centre planes of the building: the longitudinal section
//! (plane `y = ly/2`, spanning x and depth) and the perpendicular one (plane `x = lx/2`, spanning
//! y and depth). Output only: it reads the temperature field and never feeds back into the solve.
//!
//! A mirror-symmetric model solves only one side of a centre plane, so the sections are unfolded
//! to the whole domain here. Cells of the building void are NaN.
//!
//! Heat flux `q = -k grad T` (W/m2) is a cell-centre value: the mean of the conductive fluxes through
//! the cell's faces that have a solid neighbour (series resistance of the two half cells, `k` at the
//! cell's temperature). Faces to the building void and to the surface are left out, so next to them
//! the value is the flux through the opposite face. Post-processing only; the solve never reads it.

use crate::model::Model;
use crate::results::Date;
use crate::symmetry::Symmetry;
use basetherm_freezing::EnthalpyCurve;
use basetherm_grid::Axis;

/// Monthly mean temperature sections, C. `[month][i * nz + k]`, `k` (depth) fastest.
#[derive(Debug, Clone, Default)]
pub struct Sections {
    /// Cell faces of the whole domain, m (x, y from the interior corner; z depth below grade).
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    /// Plane `y = ly/2`, cells along x.
    pub longitudinal: Vec<Vec<f64>>,
    /// Plane `x = lx/2`, cells along y.
    pub perpendicular: Vec<Vec<f64>>,
    /// Heat flux on plane `y = ly/2`: `[month][(x cell * nz + depth cell) * 2 + c]`, `c = 0` along +x,
    /// `c = 1` along +depth (down). W/m2; NaN in the void.
    pub flux_longitudinal: Vec<Vec<f64>>,
    /// Heat flux on plane `x = lx/2`: as above with `c = 0` along +y.
    pub flux_perpendicular: Vec<Vec<f64>>,
}

/// Accumulates hourly planes into monthly means.
pub(crate) struct SectionAcc {
    ix: usize,
    jy: usize,
    hours: [usize; 12],
    longitudinal: Vec<Vec<f64>>,
    perpendicular: Vec<Vec<f64>>,
    flux_longitudinal: Vec<Vec<f64>>,
    flux_perpendicular: Vec<Vec<f64>>,
    curves: Vec<EnthalpyCurve>,
}

impl SectionAcc {
    pub(crate) fn new(model: &Model) -> Self {
        let g = &model.grid;
        let (lx, ly) = (model.spec.lx, model.spec.ly);
        // With a mirror plane the grid starts at the centre plane; otherwise the cell just above it.
        let ix = g.x.locate(lx / 2.0).unwrap_or(0);
        let jy = g.y.locate(ly / 2.0).unwrap_or(0);
        let nz = g.z.len();
        Self {
            ix,
            jy,
            hours: [0; 12],
            longitudinal: vec![vec![0.0; g.x.len() * nz]; 12],
            perpendicular: vec![vec![0.0; g.y.len() * nz]; 12],
            flux_longitudinal: vec![vec![0.0; g.x.len() * nz * 2]; 12],
            flux_perpendicular: vec![vec![0.0; g.y.len() * nz * 2]; 12],
            curves: model
                .materials
                .iter()
                .map(EnthalpyCurve::from_material)
                .collect(),
        }
    }

    /// Add the field `t` of 0-based hour `hour` of the year.
    pub(crate) fn add(&mut self, model: &Model, t: &[f64], hour: usize) {
        let m = usize::from(Date::from_hour(hour).month) - 1;
        let g = &model.grid;
        let [nx, ny, nz] = g.dims();
        self.hours[m] += 1;
        for i in 0..nx {
            let row = g.index(i, self.jy, 0);
            for (a, v) in self.longitudinal[m][i * nz..(i + 1) * nz]
                .iter_mut()
                .zip(&t[row..row + nz])
            {
                *a += v;
            }
        }
        for j in 0..ny {
            let row = g.index(self.ix, j, 0);
            for (a, v) in self.perpendicular[m][j * nz..(j + 1) * nz]
                .iter_mut()
                .zip(&t[row..row + nz])
            {
                *a += v;
            }
        }
        let k = |c: usize| self.curves[usize::from(model.cell_material[c])].conductivity(t[c]);
        let void = model.void_material();
        let solid = |c: usize| model.cell_material[c] != void;
        let (jy, ix) = (self.jy, self.ix);
        let lon = plane_flux(&g.x, &g.z, |i, kz| g.index(i, jy, kz), t, &k, &solid);
        let per = plane_flux(&g.y, &g.z, |j, kz| g.index(ix, j, kz), t, &k, &solid);
        for (a, v) in self.flux_longitudinal[m].iter_mut().zip(lon) {
            *a += v;
        }
        for (a, v) in self.flux_perpendicular[m].iter_mut().zip(per) {
            *a += v;
        }
    }

    pub(crate) fn finish(self, model: &Model) -> Sections {
        let g = &model.grid;
        let [nx, ny, nz] = g.dims();
        let void = model.void_material();
        let mean = |mut sums: Vec<Vec<f64>>, plane: &dyn Fn(usize) -> usize| {
            for (month, s) in sums.iter_mut().enumerate() {
                let n = self.hours[month].max(1) as f64;
                for (c, v) in s.iter_mut().enumerate() {
                    *v = if model.cell_material[plane(c)] == void {
                        f64::NAN
                    } else {
                        *v / n
                    };
                }
            }
            sums
        };
        let (jy, ix) = (self.jy, self.ix);
        let lon = mean(self.longitudinal.clone(), &|c| g.index(c / nz, jy, c % nz));
        let per = mean(self.perpendicular.clone(), &|c| g.index(ix, c / nz, c % nz));
        let Symmetry { x: sx, y: sy } = model.symmetry;
        let (x, lon) = unfold(g.x.faces(), nx, nz, sx, lon);
        let (y, per) = unfold(g.y.faces(), ny, nz, sy, per);
        let flux = |mut sums: Vec<Vec<f64>>, plane: &dyn Fn(usize) -> usize| {
            for (month, s) in sums.iter_mut().enumerate() {
                let n = self.hours[month].max(1) as f64;
                for (c, v) in s.iter_mut().enumerate() {
                    let void = model.cell_material[plane(c / 2)] == void;
                    *v = if void { f64::NAN } else { *v / n };
                }
            }
            sums
        };
        let fl = flux(self.flux_longitudinal.clone(), &|c| {
            g.index(c / nz, jy, c % nz)
        });
        let fp = flux(self.flux_perpendicular.clone(), &|c| {
            g.index(ix, c / nz, c % nz)
        });
        let fl = unfold_flux(nx, nz, sx, fl);
        let fp = unfold_flux(ny, nz, sy, fp);
        Sections {
            x,
            y,
            z: g.z.faces().to_vec(),
            longitudinal: lon,
            perpendicular: per,
            flux_longitudinal: fl,
            flux_perpendicular: fp,
        }
    }
}

/// Conductive flux (W/m2) at the centre of every cell of one plane, `[(i * nz + k) * 2 + c]`: `c = 0`
/// along the horizontal axis (cells `h`), `c = 1` downward (cells `z`). `cell(i, k)` is the grid index.
fn plane_flux(
    h: &Axis,
    z: &Axis,
    cell: impl Fn(usize, usize) -> usize,
    t: &[f64],
    k: &dyn Fn(usize) -> f64,
    solid: &dyn Fn(usize) -> bool,
) -> Vec<f64> {
    let (n, nz) = (h.len(), z.len());
    // Flux from cell `a` to cell `b` (neighbours, half widths `wa`, `wb`), None if either is void.
    let face = |a: usize, b: usize, wa: f64, wb: f64| {
        (solid(a) && solid(b)).then(|| (t[a] - t[b]) / (wa / (2.0 * k(a)) + wb / (2.0 * k(b))))
    };
    let mean = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) => 0.5 * (a + b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => 0.0,
    };
    let mut out = vec![0.0; n * nz * 2];
    for i in 0..n {
        for kz in 0..nz {
            let c = cell(i, kz);
            if !solid(c) {
                continue;
            }
            let lo = (i > 0)
                .then(|| face(cell(i - 1, kz), c, h.width(i - 1), h.width(i)))
                .flatten();
            let hi = (i + 1 < n)
                .then(|| face(c, cell(i + 1, kz), h.width(i), h.width(i + 1)))
                .flatten();
            let up = (kz > 0)
                .then(|| face(cell(i, kz - 1), c, z.width(kz - 1), z.width(kz)))
                .flatten();
            let dn = (kz + 1 < nz)
                .then(|| face(c, cell(i, kz + 1), z.width(kz), z.width(kz + 1)))
                .flatten();
            out[(i * nz + kz) * 2] = mean(lo, hi);
            out[(i * nz + kz) * 2 + 1] = mean(up, dn);
        }
    }
    out
}

/// Like [`unfold`] for flux planes (two components per cell); the horizontal component changes sign
/// on the mirrored side.
fn unfold_flux(n: usize, nz: usize, mirrored: bool, planes: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    if !mirrored {
        return planes;
    }
    planes
        .into_iter()
        .map(|p| {
            let mut out = Vec::with_capacity(2 * p.len());
            for i in (0..n).rev() {
                for kz in 0..nz {
                    let q = (i * nz + kz) * 2;
                    out.extend_from_slice(&[-p[q], p[q + 1]]);
                }
            }
            out.extend_from_slice(&p);
            out
        })
        .collect()
}

/// Mirror a half domain (faces starting at its centre plane) into the whole one.
fn unfold(
    faces: &[f64],
    n: usize,
    nz: usize,
    mirrored: bool,
    planes: Vec<Vec<f64>>,
) -> (Vec<f64>, Vec<Vec<f64>>) {
    if !mirrored {
        return (faces.to_vec(), planes);
    }
    let c = faces[0];
    let mut full: Vec<f64> = faces[1..].iter().rev().map(|f| 2.0 * c - f).collect();
    full.extend_from_slice(faces);
    let planes = planes
        .into_iter()
        .map(|p| {
            let mut out = Vec::with_capacity(2 * p.len());
            for i in (0..n).rev() {
                out.extend_from_slice(&p[i * nz..(i + 1) * nz]);
            }
            out.extend_from_slice(&p);
            out
        })
        .collect();
    (full, planes)
}

#[cfg(test)]
mod tests {
    use super::unfold;

    #[test]
    fn unfolding_mirrors_faces_and_cells() {
        // Two cells [1, 2] and [2, 4] starting at the centre plane x = 1, one depth cell.
        let (faces, planes) = unfold(&[1.0, 2.0, 4.0], 2, 1, true, vec![vec![10.0, 20.0]]);
        assert_eq!(faces, vec![-2.0, 0.0, 1.0, 2.0, 4.0]);
        assert_eq!(planes, vec![vec![20.0, 10.0, 10.0, 20.0]]);
    }

    #[test]
    fn mirrored_flux_flips_the_horizontal_component() {
        // Cells 0 and 1 (centre plane at the start of cell 0), one depth cell.
        let p = super::unfold_flux(2, 1, true, vec![vec![1.0, 2.0, 3.0, 4.0]]);
        assert_eq!(p, vec![vec![-3.0, 4.0, -1.0, 2.0, 1.0, 2.0, 3.0, 4.0]]);
    }

    #[test]
    fn whole_domain_is_left_alone() {
        let (faces, planes) = unfold(&[0.0, 1.0], 1, 1, false, vec![vec![5.0]]);
        assert_eq!(faces, vec![0.0, 1.0]);
        assert_eq!(planes, vec![vec![5.0]]);
    }
}
