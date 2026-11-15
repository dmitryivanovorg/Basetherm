//! Symmetric aggregation multigrid V-cycle used as a preconditioner for PCG and COCG.
//!
//! Pre-smoothing is a forward line Gauss-Seidel sweep over the chosen axes; post-smoothing is the
//! transposed sequence (reversed order, backward sweeps). Restriction is the transpose of
//! prolongation and coarse operators are Galerkin, so the cycle is symmetric. The same hierarchy
//! serves the real mean system and the complex harmonic system (the shift `i w C` is added on
//! every level).

use crate::dense::DenseLu;
use crate::grid7::{Aggregation, Grid7};
use crate::pcg::Precond;
use crate::scalar::Scalar;
use crate::smooth::LineFactor;
use std::cell::RefCell;

/// Which axes are coarsened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coarsen {
    /// All axes with more than one cell.
    All,
    /// Only x and y; the (finest, strongly coupled) vertical axis keeps full resolution.
    Horizontal,
}

/// Line-smoother axes, applied in order before the coarse correction (reversed after).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Smoother {
    /// Vertical (z) lines only.
    Z,
    /// z, then x, then y lines (for anisotropy in every direction).
    Alternating,
}

impl Smoother {
    pub fn axes(self) -> &'static [usize] {
        match self {
            Smoother::Z => &[2],
            Smoother::Alternating => &[2, 0, 1],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub coarsen: Coarsen,
    pub smoother: Smoother,
    /// Stop coarsening at or below this many cells and solve directly.
    pub coarse_limit: usize,
}

/// The default was chosen by measurement on the BESTEST systems (see `validation-tools/tests/mg_explore.rs`):
/// horizontal coarsening with the alternating line smoother needed 19-26 iterations from 0.17M to 0.83M
/// cells, against 420-860 for the z-line block preconditioner.
impl Default for Settings {
    fn default() -> Self {
        Self {
            coarsen: Coarsen::Horizontal,
            smoother: Smoother::Alternating,
            coarse_limit: 400,
        }
    }
}

/// Grids from fine to coarse and the aggregations between them. Independent of `w`.
pub struct Hierarchy {
    levels: Vec<Grid7>,
    aggs: Vec<Aggregation>,
    settings: Settings,
}

impl Hierarchy {
    pub fn build(fine: Grid7, settings: Settings) -> Self {
        let mut levels = vec![fine];
        let mut aggs = Vec::new();
        while levels.last().expect("non-empty").n() > settings.coarse_limit {
            let top = levels.last().expect("non-empty");
            let flags: [bool; 3] = std::array::from_fn(|d| {
                top.dims[d] > 1 && (settings.coarsen == Coarsen::All || d < 2)
            });
            if !flags.iter().any(|&f| f) {
                break;
            }
            let (coarse, agg) = top.coarsen(flags);
            levels.push(coarse);
            aggs.push(agg);
        }
        Self {
            levels,
            aggs,
            settings,
        }
    }

    pub fn level_sizes(&self) -> Vec<usize> {
        self.levels.iter().map(Grid7::n).collect()
    }
}

struct Work<S> {
    b: Vec<S>,
    x: Vec<S>,
    r: Vec<S>,
}

/// The preconditioner for `A + i w C` (use `omega = 0` for the real mean system).
pub struct Multigrid<'h, S> {
    h: &'h Hierarchy,
    omega: f64,
    factors: Vec<Vec<LineFactor<S>>>,
    coarse: DenseLu<S>,
    work: RefCell<(Vec<Work<S>>, Vec<S>)>,
}

impl<'h, S: Scalar> Multigrid<'h, S> {
    pub fn new(h: &'h Hierarchy, omega: f64) -> Self {
        let factors = h
            .levels
            .iter()
            .map(|g| {
                h.settings
                    .smoother
                    .axes()
                    .iter()
                    .map(|&ax| LineFactor::new(g, omega, ax))
                    .collect()
            })
            .collect();
        let coarse = DenseLu::from_grid(h.levels.last().expect("non-empty"), omega);
        let work = h
            .levels
            .iter()
            .map(|g| Work {
                b: vec![S::ZERO; g.n()],
                x: vec![S::ZERO; g.n()],
                r: vec![S::ZERO; g.n()],
            })
            .collect();
        Self {
            h,
            omega,
            factors,
            coarse,
            work: RefCell::new((work, Vec::new())),
        }
    }
}

impl<S: Scalar> Precond<S> for Multigrid<'_, S> {
    fn apply(&self, rhs: &[S], z: &mut [S]) {
        let mut guard = self.work.borrow_mut();
        let (work, line) = &mut *guard;
        let last = self.h.levels.len() - 1;
        work[0].b.copy_from_slice(rhs);

        for l in 0..last {
            let g = &self.h.levels[l];
            let (w, rest) = work[l..].split_first_mut().expect("level");
            w.x.fill(S::ZERO);
            for f in &self.factors[l] {
                f.sweep(g, &w.b, &mut w.x, true, line);
            }
            g.apply(self.omega, &w.x, &mut w.r);
            for (r, b) in w.r.iter_mut().zip(&w.b) {
                *r = *b - *r;
            }
            self.h.aggs[l].restrict(&w.r, &mut rest[0].b);
        }
        let w = &mut work[last];
        self.coarse.solve(&w.b, &mut w.x);

        for l in (0..last).rev() {
            let g = &self.h.levels[l];
            let (w, rest) = work[l..].split_first_mut().expect("level");
            self.h.aggs[l].prolong_add(&rest[0].x, &mut w.x);
            for f in self.factors[l].iter().rev() {
                f.sweep(g, &w.b, &mut w.x, false, line);
            }
        }
        z.copy_from_slice(&work[0].x);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid7::tests::sample;
    use crate::pcg::{pcg, Identity, LinOp};
    use crate::scalar::C64;

    struct Op<'a>(&'a Grid7, f64);

    impl<S: Scalar> LinOp<S> for Op<'_> {
        fn apply(&self, x: &[S], y: &mut [S]) {
            self.0.apply(self.1, x, y);
        }
    }

    fn rhs<S: Scalar>(n: usize) -> Vec<S> {
        (0..n)
            .map(|i| S::from_parts((i as f64 * 0.31).sin() + 0.2, (i as f64 * 0.17).cos() * 0.3))
            .collect()
    }

    fn dotb<S: Scalar>(a: &[S], b: &[S]) -> S {
        let mut s = S::ZERO;
        for (x, y) in a.iter().zip(b) {
            s += *x * *y;
        }
        s
    }

    fn symmetric<S: Scalar>(omega: f64, settings: Settings) {
        let g = sample([9, 8, 11], 21);
        let h = Hierarchy::build(g, settings);
        assert!(h.level_sizes().len() > 1, "{:?}", h.level_sizes());
        let mg = Multigrid::<S>::new(&h, omega);
        let n = h.levels[0].n();
        let r1: Vec<S> = rhs(n);
        let r2: Vec<S> = (0..n)
            .map(|i| S::from_parts((i as f64 * 0.77).cos(), (i as f64 * 0.05).sin()))
            .collect();
        let (mut m1, mut m2) = (vec![S::ZERO; n], vec![S::ZERO; n]);
        mg.apply(&r1, &mut m1);
        mg.apply(&r2, &mut m2);
        let (a, b) = (dotb(&m1, &r2), dotb(&r1, &m2));
        assert!(
            (a - b).abs2().sqrt() < 1e-9 * a.abs2().sqrt().max(1.0),
            "bilinear forms differ: |a|={} |b|={}",
            a.abs2().sqrt(),
            b.abs2().sqrt()
        );
    }

    #[test]
    fn v_cycle_is_symmetric_for_all_option_combinations() {
        for coarsen in [Coarsen::All, Coarsen::Horizontal] {
            for smoother in [Smoother::Z, Smoother::Alternating] {
                let s = Settings {
                    coarsen,
                    smoother,
                    coarse_limit: 60,
                };
                symmetric::<f64>(0.0, s);
                symmetric::<C64>(0.9, s);
            }
        }
    }

    fn iterations<S: Scalar>(omega: f64, settings: Settings) -> (usize, usize) {
        let g = sample([14, 12, 16], 4);
        let n = g.n();
        let b: Vec<S> = rhs(n);
        let plain = pcg(
            &Op(&g, omega),
            &Identity,
            &b,
            &mut vec![S::ZERO; n],
            1e-10,
            2000,
        );
        let h = Hierarchy::build(g.clone(), settings);
        let mg = Multigrid::<S>::new(&h, omega);
        let mut x = vec![S::ZERO; n];
        let out = pcg(&Op(&g, omega), &mg, &b, &mut x, 1e-10, 200);
        assert!(out.converged, "{out:?}");
        let mut ax = vec![S::ZERO; n];
        g.apply(omega, &x, &mut ax);
        assert!(ax
            .iter()
            .zip(&b)
            .all(|(u, v)| (*u - *v).abs2().sqrt() < 1e-7));
        (plain.iterations, out.iterations)
    }

    #[test]
    fn preconditioned_cg_converges_much_faster_than_plain_cg() {
        let s = Settings {
            coarsen: Coarsen::All,
            smoother: Smoother::Z,
            coarse_limit: 100,
        };
        let (plain, mg) = iterations::<f64>(0.0, s);
        assert!(mg < 30 && mg * 2 < plain, "plain {plain}, multigrid {mg}");
        let (plain, mg) = iterations::<C64>(0.3, s);
        assert!(mg < 30 && mg <= plain, "plain {plain}, multigrid {mg}");
    }
}
