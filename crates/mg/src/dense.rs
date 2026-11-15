//! Dense LU with partial pivoting, for the coarsest multigrid level.

use crate::grid7::Grid7;
use crate::scalar::Scalar;

pub(crate) struct DenseLu<S> {
    n: usize,
    lu: Vec<S>,
    piv: Vec<usize>,
}

impl<S: Scalar> DenseLu<S> {
    /// Factor `A + i w C` of a (small) grid.
    pub fn from_grid(g: &Grid7, omega: f64) -> Self {
        let n = g.n();
        let mut a = vec![S::ZERO; n * n];
        let mut e = vec![S::ZERO; n];
        let mut col = vec![S::ZERO; n];
        for j in 0..n {
            e[j] = S::from_real(1.0);
            g.apply(omega, &e, &mut col);
            e[j] = S::ZERO;
            for i in 0..n {
                a[i * n + j] = col[i];
            }
        }
        let mut piv: Vec<usize> = (0..n).collect();
        for k in 0..n {
            let p = (k..n)
                .max_by(|&x, &y| a[x * n + k].abs2().total_cmp(&a[y * n + k].abs2()))
                .expect("non-empty");
            if p != k {
                for c in 0..n {
                    a.swap(k * n + c, p * n + c);
                }
                piv.swap(k, p);
            }
            let d = a[k * n + k];
            for i in k + 1..n {
                let f = a[i * n + k] / d;
                a[i * n + k] = f;
                for c in k + 1..n {
                    let t = f * a[k * n + c];
                    a[i * n + c] -= t;
                }
            }
        }
        Self { n, lu: a, piv }
    }

    pub fn solve(&self, b: &[S], x: &mut [S]) {
        let n = self.n;
        for i in 0..n {
            x[i] = b[self.piv[i]];
        }
        for i in 1..n {
            let mut s = x[i];
            for (l, xc) in self.lu[i * n..i * n + i].iter().zip(&x[..i]) {
                s -= *l * *xc;
            }
            x[i] = s;
        }
        for i in (0..n).rev() {
            let mut s = x[i];
            for (u, xc) in self.lu[i * n + i + 1..(i + 1) * n].iter().zip(&x[i + 1..]) {
                s -= *u * *xc;
            }
            x[i] = s / self.lu[i * n + i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid7::tests::sample;
    use crate::scalar::C64;

    fn check<S: Scalar + std::fmt::Debug>(omega: f64) {
        let g = sample([4, 3, 5], 5);
        let n = g.n();
        let b: Vec<S> = (0..n)
            .map(|i| S::from_parts((i as f64).sin(), 0.0))
            .collect();
        let lu = DenseLu::<S>::from_grid(&g, omega);
        let mut x = vec![S::ZERO; n];
        lu.solve(&b, &mut x);
        let mut ax = vec![S::ZERO; n];
        g.apply(omega, &x, &mut ax);
        for (u, v) in ax.iter().zip(&b) {
            assert!((*u - *v).abs2().sqrt() < 1e-9);
        }
    }

    #[test]
    fn solves_real_and_complex_systems() {
        check::<f64>(0.0);
        check::<C64>(0.7);
    }
}
