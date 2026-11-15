//! Symmetric z-line block preconditioner: the inverse of the block-diagonal part of the matrix,
//! one tridiagonal block per vertical line. Vertical cells are the finest, so this removes the
//! strongest anisotropy. Works for real (mean) and complex (harmonic) systems.

use basetherm_conduction::Stencil;
use basetherm_mg::{Precond, Scalar};

pub struct ZLine<S> {
    nz: usize,
    /// Conductance to the next cell up the line (the off-diagonal is its negative).
    gz: Vec<f64>,
    /// Pivots of the factorisation.
    m: Vec<S>,
    /// Scaled upper off-diagonal after elimination.
    cp: Vec<S>,
}

impl<S: Scalar> ZLine<S> {
    /// `diag` is the full diagonal of the system matrix (stencil diagonal plus any shift).
    pub fn new(st: &Stencil, diag: &[S]) -> Self {
        let nz = st.dims[2];
        let gz = st.g_next[2].clone();
        let n = st.n();
        let mut m = vec![S::ZERO; n];
        let mut cp = vec![S::ZERO; n];
        for base in (0..n).step_by(nz) {
            for i in 0..nz {
                let c = base + i;
                let up = if i + 1 < nz {
                    -S::from_real(gz[c])
                } else {
                    S::ZERO
                };
                if i == 0 {
                    m[c] = diag[c];
                } else {
                    let sub = -S::from_real(gz[c - 1]);
                    m[c] = diag[c] - sub * cp[c - 1];
                }
                cp[c] = up / m[c];
            }
        }
        Self { nz, gz, m, cp }
    }
}

impl<S: Scalar> Precond<S> for ZLine<S> {
    fn apply(&self, r: &[S], z: &mut [S]) {
        let nz = self.nz;
        for base in (0..r.len()).step_by(nz) {
            z[base] = r[base] / self.m[base];
            for i in 1..nz {
                let c = base + i;
                let sub = -S::from_real(self.gz[c - 1]);
                z[c] = (r[c] - sub * z[c - 1]) / self.m[c];
            }
            for i in (0..nz - 1).rev() {
                let c = base + i;
                z[c] -= self.cp[c] * z[c + 1];
            }
        }
    }
}
