//! Line Gauss-Seidel smoother along one axis. Lines are solved exactly (tridiagonal) with the
//! couplings to the other four neighbours taken from the current iterate. A forward sweep and a
//! backward sweep are transposes of each other, which keeps the V-cycle symmetric.

use crate::grid7::Grid7;
use crate::scalar::Scalar;

pub(crate) struct LineFactor<S> {
    axis: usize,
    m: Vec<S>,
    cp: Vec<S>,
}

fn others(d: usize) -> (usize, usize) {
    match d {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

impl<S: Scalar> LineFactor<S> {
    pub fn new(g: &Grid7, omega: f64, axis: usize) -> Self {
        let n = g.n();
        let strides = g.strides();
        let (a, b) = others(axis);
        let (st, nd) = (strides[axis], g.dims[axis]);
        let mut m = vec![S::ZERO; n];
        let mut cp = vec![S::ZERO; n];
        for pa in 0..g.dims[a] {
            for pb in 0..g.dims[b] {
                let base = pa * strides[a] + pb * strides[b];
                for i in 0..nd {
                    let c = base + i * st;
                    let d = S::from_parts(g.diag[c], omega * g.cap[c]);
                    m[c] = if i == 0 {
                        d
                    } else {
                        let sub = -S::from_real(g.g_next[axis][c - st]);
                        d - sub * cp[c - st]
                    };
                    let up = if i + 1 < nd {
                        -S::from_real(g.g_next[axis][c])
                    } else {
                        S::ZERO
                    };
                    cp[c] = up / m[c];
                }
            }
        }
        Self { axis, m, cp }
    }

    /// One Gauss-Seidel sweep over all lines of this axis: `x <- T^{-1} (b + off-line couplings x)`.
    pub fn sweep(&self, g: &Grid7, b: &[S], x: &mut [S], forward: bool, line: &mut Vec<S>) {
        let d = self.axis;
        let (a, bb) = others(d);
        let strides = g.strides();
        let (st, nd) = (strides[d], g.dims[d]);
        line.resize(nd, S::ZERO);
        let pas: Vec<usize> = if forward {
            (0..g.dims[a]).collect()
        } else {
            (0..g.dims[a]).rev().collect()
        };
        let pbs: Vec<usize> = if forward {
            (0..g.dims[bb]).collect()
        } else {
            (0..g.dims[bb]).rev().collect()
        };
        for &pa in &pas {
            for &pb in &pbs {
                let base = pa * strides[a] + pb * strides[bb];
                let pos = [(a, pa), (bb, pb)];
                for i in 0..nd {
                    let c = base + i * st;
                    let mut r = b[c];
                    for &(ax, p) in &pos {
                        if p + 1 < g.dims[ax] {
                            r += S::from_real(g.g_next[ax][c]) * x[c + strides[ax]];
                        }
                        if p > 0 {
                            r += S::from_real(g.g_next[ax][c - strides[ax]]) * x[c - strides[ax]];
                        }
                    }
                    line[i] = if i == 0 {
                        r / self.m[c]
                    } else {
                        (r + S::from_real(g.g_next[d][c - st]) * line[i - 1]) / self.m[c]
                    };
                }
                for i in (0..nd).rev() {
                    let c = base + i * st;
                    x[c] = if i + 1 < nd {
                        line[i] - self.cp[c] * x[c + st]
                    } else {
                        line[i]
                    };
                }
            }
        }
    }
}
