//! The work for one grid line: the explicit increment (stage 0) and the implicit tridiagonal solve
//! (stages 1-3). Reads shared data only; results go out through a `put(position, value)` callback
//! and a [`JobOut`], so any layout and any scheduling gives the same numbers.

use crate::thomas;
use basetherm_bc::{Boundaries, Interfaces};
use basetherm_conduction::line::{Ctx, Line};
use basetherm_grid::Grid;

/// Read-only inputs of a step.
pub(crate) struct Kernel<'a> {
    pub grid: &'a Grid,
    pub k: &'a [f64],
    pub cv: &'a [f64],
    pub bc: &'a Boundaries,
    pub interfaces: Option<&'a Interfaces>,
    /// Temperatures at the start of the step.
    pub old: &'a [f64],
    pub dt: f64,
}

/// A grid line: along axis `d`, at position `(pa, pb)` along the other two axes.
#[derive(Clone, Copy)]
pub(crate) struct LineId {
    pub d: usize,
    pub pa: usize,
    pub pb: usize,
}

/// Per-thread buffers.
pub(crate) struct Scratch {
    line: Line,
    sub: Vec<f64>,
    diag: Vec<f64>,
    sup: Vec<f64>,
    rhs: Vec<f64>,
    work: Vec<f64>,
}

impl Scratch {
    pub fn new(longest: usize) -> Self {
        Self {
            line: Line::default(),
            sub: vec![0.0; longest],
            diag: vec![0.0; longest],
            sup: vec![0.0; longest],
            rhs: vec![0.0; longest],
            work: vec![0.0; longest],
        }
    }
}

/// What the lines of one slab report besides their temperatures.
#[derive(Debug, Default)]
pub(crate) struct JobOut {
    /// `(face index, lo flux, hi flux)` per line, W out of the domain.
    pub lines: Vec<(usize, f64, f64)>,
    /// Flux into each interior-face group from this slab's lines, W.
    pub groups: Vec<f64>,
}

impl JobOut {
    pub fn reset(&mut self, groups: usize) {
        self.lines.clear();
        self.groups.clear();
        self.groups.resize(groups, 0.0);
    }
}

impl Kernel<'_> {
    fn fill(&self, s: &mut Scratch, id: LineId) {
        let ctx = Ctx {
            grid: self.grid,
            k: self.k,
            cv: self.cv,
            bc: self.bc,
            interfaces: self.interfaces,
        };
        s.line.fill(&ctx, id.d, id.pa, id.pb);
    }

    /// Stage 0: `dt * (A_d(old) + interior-face sources)` for each cell of the line.
    pub fn explicit_line(&self, s: &mut Scratch, id: LineId, mut put: impl FnMut(usize, f64)) {
        self.fill(s, id);
        let l = &s.line;
        for i in 0..l.n {
            let mut a_d = l.m_u(self.old, i);
            if i == 0 {
                a_d += l.lo.g * l.lo.t;
            }
            if i + 1 == l.n {
                a_d += l.hi.g * l.hi.t;
            }
            put(i, self.dt * (a_d + l.ext_gt[i]));
        }
    }

    /// Stages 1-3: `(C - dt M_d) x = C prev - dt M_d old` along one line.
    pub fn solve_line(
        &self,
        s: &mut Scratch,
        id: LineId,
        prev: &[f64],
        out: &mut JobOut,
        mut put: impl FnMut(usize, f64),
    ) {
        self.fill(s, id);
        let dt = self.dt;
        let Scratch {
            line: l,
            sub,
            diag,
            sup,
            rhs,
            work,
        } = s;
        for i in 0..l.n {
            let g_prev = if i > 0 { l.g[i - 1] } else { 0.0 };
            let g_next = if i + 1 < l.n { l.g[i] } else { 0.0 };
            let g_bnd = if i == 0 { l.lo.g } else { 0.0 } + if i + 1 == l.n { l.hi.g } else { 0.0 };
            sub[i] = -dt * g_prev;
            sup[i] = -dt * g_next;
            diag[i] = l.c[i] + dt * (g_prev + g_next + g_bnd + l.ext_g[i]);
            rhs[i] = l.c[i] * prev[l.base + i * l.stride] - dt * l.m_u(self.old, i);
        }
        thomas::solve(sub, diag, sup, &mut rhs[..l.n], work);
        for (i, &x) in rhs[..l.n].iter().enumerate() {
            put(i, x);
        }
        let lo = l.lo.g * (rhs[0] - l.lo.t);
        let hi = l.hi.g * (rhs[l.n - 1] - l.hi.t);
        out.lines.push((l.face, lo, hi));
        for f in &l.faces {
            if let Some(g) = out.groups.get_mut(f.group as usize) {
                *g += f.g * (rhs[f.pos] - f.t);
            }
        }
    }
}
