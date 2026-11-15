//! Seven-point stencil operator on a structured grid, and its Galerkin coarsening.
//!
//! `A = diag - (neighbour couplings)`, with symmetric couplings `g_next[d][c]` between cell `c` and
//! its `+d` neighbour. The harmonic system adds `i w cap` to the diagonal.

use crate::scalar::Scalar;

#[derive(Debug, Clone)]
pub struct Grid7 {
    pub dims: [usize; 3],
    /// Coupling of each cell to its `+axis` neighbour (0 at the domain end).
    pub g_next: [Vec<f64>; 3],
    pub diag: Vec<f64>,
    /// Heat capacity per cell; scales the imaginary diagonal shift.
    pub cap: Vec<f64>,
}

/// How a coarse grid aggregates a fine grid: pairs of cells along each flagged axis.
#[derive(Debug, Clone, Copy)]
pub struct Aggregation {
    pub fine: [usize; 3],
    pub coarse: [usize; 3],
    pub flags: [bool; 3],
}

impl Aggregation {
    pub fn new(fine: [usize; 3], flags: [bool; 3]) -> Self {
        let coarse = std::array::from_fn(|d| {
            if flags[d] {
                fine[d].div_ceil(2)
            } else {
                fine[d]
            }
        });
        Self {
            fine,
            coarse,
            flags,
        }
    }

    fn coarse_index(&self, i: [usize; 3]) -> usize {
        let c: [usize; 3] = std::array::from_fn(|d| if self.flags[d] { i[d] / 2 } else { i[d] });
        (c[0] * self.coarse[1] + c[1]) * self.coarse[2] + c[2]
    }

    /// `rc = P^T r`: sum fine values into their aggregate.
    pub fn restrict<S: Scalar>(&self, r: &[S], rc: &mut [S]) {
        rc.fill(S::ZERO);
        for_each_cell(self.fine, |idx, i| rc[self.coarse_index(i)] += r[idx]);
    }

    /// `x += P xc`: piecewise-constant interpolation.
    pub fn prolong_add<S: Scalar>(&self, xc: &[S], x: &mut [S]) {
        for_each_cell(self.fine, |idx, i| x[idx] += xc[self.coarse_index(i)]);
    }
}

fn for_each_cell(dims: [usize; 3], mut f: impl FnMut(usize, [usize; 3])) {
    let mut idx = 0;
    for i in 0..dims[0] {
        for j in 0..dims[1] {
            for k in 0..dims[2] {
                f(idx, [i, j, k]);
                idx += 1;
            }
        }
    }
}

impl Grid7 {
    pub fn n(&self) -> usize {
        self.diag.len()
    }

    pub fn strides(&self) -> [usize; 3] {
        [self.dims[1] * self.dims[2], self.dims[2], 1]
    }

    /// `y = (A + i w C) x`.
    pub fn apply<S: Scalar>(&self, omega: f64, x: &[S], y: &mut [S]) {
        let [nx, ny, nz] = self.dims;
        let [sx, sy, _] = self.strides();
        let [gx, gy, gz] = &self.g_next;
        let mut c = 0;
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let mut s = S::from_parts(self.diag[c], omega * self.cap[c]) * x[c];
                    if i + 1 < nx {
                        s -= S::from_real(gx[c]) * x[c + sx];
                    }
                    if i > 0 {
                        s -= S::from_real(gx[c - sx]) * x[c - sx];
                    }
                    if j + 1 < ny {
                        s -= S::from_real(gy[c]) * x[c + sy];
                    }
                    if j > 0 {
                        s -= S::from_real(gy[c - sy]) * x[c - sy];
                    }
                    if k + 1 < nz {
                        s -= S::from_real(gz[c]) * x[c + 1];
                    }
                    if k > 0 {
                        s -= S::from_real(gz[c - 1]) * x[c - 1];
                    }
                    y[c] = s;
                    c += 1;
                }
            }
        }
    }

    /// Galerkin coarse operator `P^T A P` for piecewise-constant `P`: couplings across a coarse
    /// interface are the sum of the fine couplings crossing it; faces inside an aggregate drop
    /// out of the diagonal.
    pub fn coarsen(&self, flags: [bool; 3]) -> (Grid7, Aggregation) {
        let agg = Aggregation::new(self.dims, flags);
        let nc = agg.coarse.iter().product::<usize>();
        let mut coarse = Grid7 {
            dims: agg.coarse,
            g_next: [vec![0.0; nc], vec![0.0; nc], vec![0.0; nc]],
            diag: vec![0.0; nc],
            cap: vec![0.0; nc],
        };
        let strides = self.strides();
        for_each_cell(self.dims, |idx, i| {
            let c = agg.coarse_index(i);
            coarse.cap[c] += self.cap[idx];
            coarse.diag[c] += self.diag[idx];
            for d in 0..3 {
                if i[d] + 1 >= self.dims[d] {
                    continue;
                }
                let mut j = i;
                j[d] += 1;
                let c2 = agg.coarse_index(j);
                let g = self.g_next[d][idx];
                debug_assert!(idx + strides[d] < self.n());
                if c2 == c {
                    coarse.diag[c] -= 2.0 * g;
                } else {
                    coarse.g_next[d][c] += g;
                }
            }
        });
        (coarse, agg)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small random-coefficient grid; the diagonal makes the operator positive definite.
    pub(crate) fn sample(dims: [usize; 3], seed: u64) -> Grid7 {
        let n = dims.iter().product::<usize>();
        let mut s = seed;
        let mut rnd = move || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 11) as f64) / ((1u64 << 53) as f64)
        };
        let strides = [dims[1] * dims[2], dims[2], 1];
        let mut g = Grid7 {
            dims,
            g_next: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
            diag: (0..n).map(|_| 0.1 * rnd()).collect(), // boundary-like leakage
            cap: (0..n).map(|_| 1.0 + rnd()).collect(),
        };
        let mut idx = 0;
        for i in 0..dims[0] {
            for j in 0..dims[1] {
                for k in 0..dims[2] {
                    for (d, &p) in [i, j, k].iter().enumerate() {
                        if p + 1 < dims[d] {
                            let v = 0.2 + 3.0 * rnd();
                            g.g_next[d][idx] = v;
                            g.diag[idx] += v;
                            g.diag[idx + strides[d]] += v;
                        }
                    }
                    idx += 1;
                }
            }
        }
        g
    }

    #[test]
    fn galerkin_matches_ptap() {
        let fine = sample([5, 4, 7], 3);
        let (coarse, agg) = fine.coarsen([true, false, true]);
        assert_eq!(coarse.dims, [3, 4, 4]);
        // P^T A P xc computed explicitly must equal the coarse operator applied to xc.
        let nc = coarse.n();
        let xc: Vec<f64> = (0..nc).map(|i| (i as f64 * 0.37).sin() + 0.5).collect();
        let mut x = vec![0.0; fine.n()];
        agg.prolong_add(&xc, &mut x);
        let mut ax = vec![0.0; fine.n()];
        fine.apply(0.0, &x, &mut ax);
        let mut ptap = vec![0.0; nc];
        agg.restrict(&ax, &mut ptap);
        let mut direct = vec![0.0; nc];
        coarse.apply(0.0, &xc, &mut direct);
        for (a, b) in ptap.iter().zip(&direct) {
            assert!((a - b).abs() < 1e-10, "{a} vs {b}");
        }
    }

    #[test]
    fn capacity_is_conserved_by_coarsening() {
        let fine = sample([4, 4, 4], 9);
        let (coarse, _) = fine.coarsen([true, true, true]);
        let (a, b): (f64, f64) = (fine.cap.iter().sum(), coarse.cap.iter().sum());
        assert!((a - b).abs() < 1e-12);
        assert_eq!(coarse.dims, [2, 2, 2]);
    }

    #[test]
    fn restriction_is_the_transpose_of_prolongation() {
        let agg = Aggregation::new([5, 3, 4], [true, true, false]);
        let fine_n = 5 * 3 * 4;
        let coarse_n = agg.coarse.iter().product::<usize>();
        let r: Vec<f64> = (0..fine_n).map(|i| (i as f64).cos()).collect();
        let xc: Vec<f64> = (0..coarse_n).map(|i| (i as f64 * 0.7).sin()).collect();
        let mut rc = vec![0.0; coarse_n];
        agg.restrict(&r, &mut rc);
        let mut px = vec![0.0; fine_n];
        agg.prolong_add(&xc, &mut px);
        let lhs: f64 = rc.iter().zip(&xc).map(|(a, b)| a * b).sum();
        let rhs: f64 = r.iter().zip(&px).map(|(a, b)| a * b).sum();
        assert!((lhs - rhs).abs() < 1e-12);
    }
}
