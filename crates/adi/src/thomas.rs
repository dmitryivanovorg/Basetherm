//! Tridiagonal solve (Thomas algorithm) on caller-owned buffers.

/// Solve `sub[i]*x[i-1] + diag[i]*x[i] + sup[i]*x[i+1] = rhs[i]` in place; the answer is left in
/// `rhs`. `sub[0]` and `sup[n-1]` are ignored. `work` must be at least `n` long.
/// The matrix must be diagonally dominant (it is, for implicit heat conduction).
pub(crate) fn solve(sub: &[f64], diag: &[f64], sup: &[f64], rhs: &mut [f64], work: &mut [f64]) {
    let n = rhs.len();
    debug_assert!(sub.len() >= n && diag.len() >= n && sup.len() >= n && work.len() >= n);
    work[0] = sup[0] / diag[0];
    rhs[0] /= diag[0];
    for i in 1..n {
        let m = diag[i] - sub[i] * work[i - 1];
        work[i] = sup[i] / m;
        rhs[i] = (rhs[i] - sub[i] * rhs[i - 1]) / m;
    }
    for i in (0..n.saturating_sub(1)).rev() {
        rhs[i] -= work[i] * rhs[i + 1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_known_system() {
        // [2 -1 0; -1 2 -1; 0 -1 2] x = [1 0 1]  ->  x = [1 1 1]
        let sub = [0.0, -1.0, -1.0];
        let diag = [2.0, 2.0, 2.0];
        let sup = [-1.0, -1.0, 0.0];
        let mut rhs = [1.0, 0.0, 1.0];
        let mut work = [0.0; 3];
        solve(&sub, &diag, &sup, &mut rhs, &mut work);
        for x in rhs {
            assert!((x - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn single_unknown() {
        let mut rhs = [6.0];
        solve(&[0.0], &[3.0], &[0.0], &mut rhs, &mut [0.0]);
        assert_eq!(rhs[0], 2.0);
    }
}
