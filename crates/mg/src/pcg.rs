//! Preconditioned conjugate gradients with the bilinear inner product.
//!
//! For real SPD systems this is ordinary PCG. For complex-symmetric systems (`A = A^T`, not
//! Hermitian) it is COCG. The preconditioner must also be symmetric.

use crate::scalar::Scalar;

pub trait LinOp<S> {
    fn apply(&self, x: &[S], y: &mut [S]);
}

pub trait Precond<S> {
    fn apply(&self, r: &[S], z: &mut [S]);
}

impl<S, P: Precond<S> + ?Sized> Precond<S> for Box<P> {
    fn apply(&self, r: &[S], z: &mut [S]) {
        (**self).apply(r, z);
    }
}

/// No preconditioning.
pub struct Identity;

impl<S: Copy> Precond<S> for Identity {
    fn apply(&self, r: &[S], z: &mut [S]) {
        z.copy_from_slice(r);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    pub iterations: usize,
    /// Final `|r| / |b|`.
    pub residual: f64,
    pub converged: bool,
}

fn dot<S: Scalar>(a: &[S], b: &[S]) -> S {
    let mut s = S::ZERO;
    for (x, y) in a.iter().zip(b) {
        s += *x * *y;
    }
    s
}

fn norm<S: Scalar>(a: &[S]) -> f64 {
    a.iter().map(|x| x.abs2()).sum::<f64>().sqrt()
}

/// Solve `A x = b`, starting from the given `x`, until `|r| <= tol |b|` or `max_iter`.
pub fn pcg<S: Scalar, A: LinOp<S>, P: Precond<S>>(
    a: &A,
    p: &P,
    b: &[S],
    x: &mut [S],
    tol: f64,
    max_iter: usize,
) -> Outcome {
    let n = b.len();
    let bnorm = norm(b);
    if bnorm == 0.0 {
        x.fill(S::ZERO);
        return Outcome {
            iterations: 0,
            residual: 0.0,
            converged: true,
        };
    }
    let mut r = vec![S::ZERO; n];
    let mut ap = vec![S::ZERO; n];
    a.apply(x, &mut ap);
    for i in 0..n {
        r[i] = b[i] - ap[i];
    }
    let mut z = vec![S::ZERO; n];
    p.apply(&r, &mut z);
    let mut dir = z.clone();
    let mut rz = dot(&r, &z);
    let mut res = norm(&r) / bnorm;
    let mut it = 0;
    while res > tol && it < max_iter {
        a.apply(&dir, &mut ap);
        let alpha = rz / dot(&dir, &ap);
        for i in 0..n {
            x[i] += alpha * dir[i];
            r[i] -= alpha * ap[i];
        }
        p.apply(&r, &mut z);
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        rz = rz_new;
        for i in 0..n {
            dir[i] = z[i] + beta * dir[i];
        }
        res = norm(&r) / bnorm;
        it += 1;
    }
    Outcome {
        iterations: it,
        residual: res,
        converged: res <= tol,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scalar::C64;

    /// 1D Laplacian with Dirichlet ends: tridiagonal(-1, 2, -1).
    struct Lap(usize);

    impl LinOp<f64> for Lap {
        fn apply(&self, x: &[f64], y: &mut [f64]) {
            for i in 0..self.0 {
                let l = if i > 0 { x[i - 1] } else { 0.0 };
                let r = if i + 1 < self.0 { x[i + 1] } else { 0.0 };
                y[i] = 2.0 * x[i] - l - r;
            }
        }
    }

    /// The same Laplacian plus a complex diagonal shift: complex-symmetric.
    struct LapShift(usize, C64);

    impl LinOp<C64> for LapShift {
        fn apply(&self, x: &[C64], y: &mut [C64]) {
            for i in 0..self.0 {
                let l = if i > 0 { x[i - 1] } else { C64::ZERO };
                let r = if i + 1 < self.0 { x[i + 1] } else { C64::ZERO };
                y[i] = (C64::real(2.0) + self.1) * x[i] - l - r;
            }
        }
    }

    #[test]
    fn real_spd_converges() {
        let n = 50;
        let b = vec![1.0; n];
        let mut x = vec![0.0; n];
        let out = pcg(&Lap(n), &Identity, &b, &mut x, 1e-12, 200);
        assert!(out.converged && out.iterations <= n + 1, "{out:?}");
        let mut ax = vec![0.0; n];
        Lap(n).apply(&x, &mut ax);
        assert!(ax.iter().all(|v| (v - 1.0).abs() < 1e-9));
    }

    #[test]
    fn complex_symmetric_converges() {
        let n = 40;
        let op = LapShift(n, C64::new(0.1, 0.7));
        let b: Vec<C64> = (0..n).map(|i| C64::new(1.0, i as f64 * 0.01)).collect();
        let mut x = vec![C64::ZERO; n];
        let out = pcg(&op, &Identity, &b, &mut x, 1e-12, 200);
        assert!(out.converged, "{out:?}");
        let mut ax = vec![C64::ZERO; n];
        op.apply(&x, &mut ax);
        assert!(ax.iter().zip(&b).all(|(u, v)| (*u - *v).abs() < 1e-9));
    }

    #[test]
    fn zero_rhs_gives_zero() {
        let mut x = vec![5.0; 3];
        let out = pcg(&Lap(3), &Identity, &[0.0; 3], &mut x, 1e-10, 10);
        assert!(out.converged);
        assert_eq!(x, vec![0.0; 3]);
    }
}
