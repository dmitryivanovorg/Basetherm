//! The periodic solve: mean field by PCG, annual harmonic by COCG.

use crate::amplitudes::Amplitudes;
use crate::error::Error;
use crate::precond::ZLine;
use basetherm_bc::{Boundaries, Interfaces, Side};
use basetherm_conduction::Stencil;
use basetherm_grid::Grid;
use basetherm_mg::{pcg, Grid7, Hierarchy, LinOp, Multigrid, Outcome, Precond, Settings, C64};
use std::cell::RefCell;

/// Preconditioner for both solves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrecondKind {
    /// Block Jacobi over vertical lines.
    ZLine,
    /// Symmetric aggregation multigrid V-cycle.
    Multigrid(Settings),
}

#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// Angular frequency of the annual cycle, rad/s.
    pub omega: f64,
    /// Relative residual target.
    pub tol: f64,
    pub max_iter: usize,
    pub precond: PrecondKind,
}

impl Params {
    /// One-year period, tolerance 1e-10.
    pub fn annual() -> Self {
        Self {
            omega: 2.0 * std::f64::consts::PI / (365.0 * 86400.0),
            tol: 1e-10,
            max_iter: 2000,
            precond: PrecondKind::Multigrid(Settings::default()),
        }
    }
}

struct MeanOp<'a>(&'a Stencil);

impl LinOp<f64> for MeanOp<'_> {
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        self.0.apply(x, y);
    }
}

/// `A + i w C` applied to a complex vector via two real stencil products.
struct HarmonicOp<'a> {
    st: &'a Stencil,
    omega: f64,
    scratch: RefCell<[Vec<f64>; 4]>,
}

impl LinOp<C64> for HarmonicOp<'_> {
    fn apply(&self, x: &[C64], y: &mut [C64]) {
        let mut guard = self.scratch.borrow_mut();
        let [xr, xi, yr, yi] = &mut *guard;
        for (i, v) in x.iter().enumerate() {
            xr[i] = v.re;
            xi[i] = v.im;
        }
        self.st.apply(xr, yr);
        self.st.apply(xi, yi);
        for i in 0..x.len() {
            let wc = self.omega * self.st.cap[i];
            y[i] = C64::new(yr[i] - wc * xi[i], yi[i] + wc * xr[i]);
        }
    }
}

/// Periodic steady state `T = mean + Re(amp e^{i w t})`.
#[derive(Debug, Clone)]
pub struct Solution {
    pub omega: f64,
    pub mean: Vec<f64>,
    pub amp: Vec<C64>,
    pub mean_outcome: Outcome,
    pub harmonic_outcome: Outcome,
    stencil: Stencil,
    forcing: Amplitudes,
}

impl Solution {
    /// Cell temperature at time `t` (s) within the cycle.
    pub fn temperature(&self, cell: usize, t: f64) -> f64 {
        self.mean[cell] + self.amp[cell].re_at(self.omega * t)
    }

    /// Heat leaving the domain through each face cell of `side`: `(mean, amplitude)`, W.
    /// The instantaneous value is `mean + Re(amp e^{i w t})`.
    pub fn boundary_flux(&self, side: Side) -> (Vec<f64>, Vec<C64>) {
        let s = side as usize;
        let cells = &self.stencil.face_cell[s];
        let g = &self.stencil.face_g[s];
        let tb = &self.stencil.face_t[s];
        let mean = cells
            .iter()
            .enumerate()
            .map(|(f, &c)| g[f] * (self.mean[c] - tb[f]))
            .collect();
        let amp = cells
            .iter()
            .enumerate()
            .map(|(f, &c)| (self.amp[c] - self.forcing.get(side, f)) * g[f])
            .collect();
        (mean, amp)
    }

    /// Heat capacity of each cell, J/K.
    pub fn capacity(&self) -> &[f64] {
        &self.stencil.cap
    }

    /// Mean heat flow from the solid into interior-face group `group`, W. The group temperature is
    /// the one given when solving (its annual mean; room temperature swings are not part of the
    /// periodic solve).
    pub fn group_flux(&self, group: usize) -> f64 {
        let t = self.stencil.group_t[group];
        self.stencil
            .internal
            .iter()
            .filter(|f| f.2 as usize == group)
            .map(|&(cell, g, _)| g * (self.mean[cell] - t))
            .sum()
    }
}

/// Solve for the periodic steady state. `bc` holds the mean boundary temperatures and `forcing`
/// their annual amplitudes. Properties are per cell and constant (frozen or thawed as chosen by
/// the caller).
pub fn solve(
    grid: &Grid,
    k: &[f64],
    cv: &[f64],
    bc: &Boundaries,
    forcing: &Amplitudes,
    p: Params,
) -> Result<Solution, Error> {
    solve_with(grid, k, cv, bc, forcing, None, p)
}

/// [`solve`] for a domain with void cells and interior faces (a basement in the soil). Group
/// temperatures are taken at their current values and held constant.
pub fn solve_with(
    grid: &Grid,
    k: &[f64],
    cv: &[f64],
    bc: &Boundaries,
    forcing: &Amplitudes,
    interfaces: Option<&Interfaces>,
    p: Params,
) -> Result<Solution, Error> {
    if !(p.omega.is_finite() && p.omega > 0.0 && p.tol.is_finite() && p.tol > 0.0) {
        return Err(Error::BadParams("omega and tol must be positive"));
    }
    let st = Stencil::new(grid, k, cv, bc, interfaces)?;
    if !st.is_anchored() {
        return Err(Error::NotAnchored);
    }
    let n = st.n();

    // Mean field.
    let b0 = st.mean_source();
    let mut mean = vec![0.0; n];
    let hierarchy = match p.precond {
        PrecondKind::ZLine => None,
        PrecondKind::Multigrid(settings) => {
            let fine = Grid7 {
                dims: st.dims,
                g_next: st.g_next.clone(),
                diag: st.diag.clone(),
                cap: st.cap.clone(),
            };
            Some(Hierarchy::build(fine, settings))
        }
    };
    let pre0: Box<dyn Precond<f64> + '_> = match &hierarchy {
        Some(h) => Box::new(Multigrid::<f64>::new(h, 0.0)),
        None => Box::new(ZLine::new(&st, &st.diag)),
    };
    let mean_outcome = pcg(&MeanOp(&st), &pre0, &b0, &mut mean, p.tol, p.max_iter);
    if !mean_outcome.converged {
        return Err(Error::NotConverged {
            which: "mean",
            outcome: mean_outcome,
        });
    }

    // Annual harmonic.
    let mut b1 = vec![C64::new(0.0, 0.0); n];
    for side in Side::ALL {
        let s = side as usize;
        for (f, &cell) in st.face_cell[s].iter().enumerate() {
            b1[cell] += forcing.get(side, f) * st.face_g[s][f];
        }
    }
    let diag1: Vec<C64> = (0..n)
        .map(|i| C64::new(st.diag[i], p.omega * st.cap[i]))
        .collect();
    let pre1: Box<dyn Precond<C64> + '_> = match &hierarchy {
        Some(h) => Box::new(Multigrid::<C64>::new(h, p.omega)),
        None => Box::new(ZLine::new(&st, &diag1)),
    };
    let op = HarmonicOp {
        st: &st,
        omega: p.omega,
        scratch: RefCell::new(std::array::from_fn(|_| vec![0.0; n])),
    };
    let mut amp = vec![C64::new(0.0, 0.0); n];
    let harmonic_outcome = pcg(&op, &pre1, &b1, &mut amp, p.tol, p.max_iter);
    if !harmonic_outcome.converged {
        return Err(Error::NotConverged {
            which: "harmonic",
            outcome: harmonic_outcome,
        });
    }
    drop(op);

    Ok(Solution {
        omega: p.omega,
        mean,
        amp,
        mean_outcome,
        harmonic_outcome,
        stencil: st,
        forcing: forcing.clone(),
    })
}
