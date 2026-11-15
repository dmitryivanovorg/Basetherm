//! Assembly of one grid line along an axis: capacities, conductances, boundary couplings.

use basetherm_bc::{Bc, Boundaries, Interfaces, Side};
use basetherm_grid::Grid;

/// Read-only data a line is assembled from.
pub struct Ctx<'a> {
    pub grid: &'a Grid,
    pub k: &'a [f64],
    pub cv: &'a [f64],
    pub bc: &'a Boundaries,
    /// Void cells and their interior faces (basement interiors); `None` for a plain box.
    pub interfaces: Option<&'a Interfaces>,
}

/// Coupling of a line end to its boundary: conductance (W/K) to a fixed temperature (C).
#[derive(Debug, Clone, Copy, Default)]
pub struct End {
    pub g: f64,
    pub t: f64,
}

/// An interior face met along a line: solid cell at position `pos`, conductance `g` (W/K) to the
/// group temperature `t` (C).
#[derive(Debug, Clone, Copy)]
pub struct LineFace {
    pub pos: usize,
    pub g: f64,
    pub t: f64,
    pub group: u32,
}

/// Everything the sweep needs for one line. Buffers are reused between lines.
#[derive(Debug, Default)]
pub struct Line {
    pub n: usize,
    /// Heat capacity of each cell, J/K.
    pub c: Vec<f64>,
    /// Conductance between cell `i` and `i+1`, W/K (`n-1` values).
    pub g: Vec<f64>,
    pub lo: End,
    pub hi: End,
    /// Index of the first cell in the field array, and the stride along the line.
    pub base: usize,
    pub stride: usize,
    /// Index of this line's face cell in the side arrays.
    pub face: usize,
    /// Per cell: total film conductance of its interior faces along this line, W/K.
    pub ext_g: Vec<f64>,
    /// Per cell: the same conductances times their group temperatures, W.
    pub ext_gt: Vec<f64>,
    /// The interior faces along this line, for flux accounting.
    pub faces: Vec<LineFace>,
}

/// Axes other than `d`, in increasing order.
pub fn others(d: usize) -> (usize, usize) {
    match d {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

fn strides(grid: &Grid) -> [usize; 3] {
    let [_, ny, nz] = grid.dims();
    [ny * nz, nz, 1]
}

fn widths(grid: &Grid, d: usize) -> impl Fn(usize) -> f64 + '_ {
    move |i| match d {
        0 => grid.x.width(i),
        1 => grid.y.width(i),
        _ => grid.z.width(i),
    }
}

fn end(bc: Bc, k: f64, w: f64, area: f64) -> End {
    match bc {
        Bc::Adiabatic => End { g: 0.0, t: 0.0 },
        Bc::Fixed(t) => End {
            g: 2.0 * k * area / w,
            t,
        },
        Bc::Convective { h, t_inf } => End {
            g: area / (1.0 / h + w / (2.0 * k)),
            t: t_inf,
        },
    }
}

impl Line {
    /// Fill this line for axis `d` at position `(pa, pb)` along the other two axes.
    pub fn fill(&mut self, ctx: &Ctx, d: usize, pa: usize, pb: usize) {
        let Ctx {
            grid,
            k,
            cv,
            bc,
            interfaces,
        } = *ctx;
        let (a, b) = others(d);
        let dims = grid.dims();
        let st = strides(grid);
        let (wa, wb, wd) = (widths(grid, a), widths(grid, b), widths(grid, d));
        let area = wa(pa) * wb(pb);
        let n = dims[d];
        let void = |c: usize| interfaces.is_some_and(|x| x.is_void(c));

        self.n = n;
        self.stride = st[d];
        self.base = pa * st[a] + pb * st[b];
        self.face = pa * dims[b] + pb;
        self.c.clear();
        self.g.clear();
        self.ext_g.clear();
        self.ext_gt.clear();
        self.faces.clear();
        for i in 0..n {
            let c = self.base + i * self.stride;
            self.c.push(cv[c] * wd(i) * area);
            let (mut eg, mut egt) = (0.0, 0.0);
            if let Some(x) = interfaces {
                for side in [Side::lo(d), Side::hi(d)] {
                    if let Some(f) = x.face_at(c, side) {
                        let g = area / (1.0 / f.h + wd(i) / (2.0 * k[c]));
                        let t = x.temperature(f.group as usize);
                        eg += g;
                        egt += g * t;
                        self.faces.push(LineFace {
                            pos: i,
                            g,
                            t,
                            group: f.group,
                        });
                    }
                }
            }
            self.ext_g.push(eg);
            self.ext_gt.push(egt);
        }
        for i in 0..n.saturating_sub(1) {
            let (c0, c1) = (
                self.base + i * self.stride,
                self.base + (i + 1) * self.stride,
            );
            // No conduction into or out of a void cell.
            if void(c0) || void(c1) {
                self.g.push(0.0);
            } else {
                self.g
                    .push(area / (wd(i) / (2.0 * k[c0]) + wd(i + 1) / (2.0 * k[c1])));
            }
        }
        let (cf, cl) = (self.base, self.base + (n - 1) * self.stride);
        self.lo = if void(cf) {
            End::default()
        } else {
            end(bc.get(Side::lo(d), self.face), k[cf], wd(0), area)
        };
        self.hi = if void(cl) {
            End::default()
        } else {
            end(bc.get(Side::hi(d), self.face), k[cl], wd(n - 1), area)
        };
    }

    /// `(M u)_i`: the linear part of the conduction operator (sum of conductance times temperature
    /// difference to neighbours, minus the boundary and interior-face conductances times `u_i`).
    pub fn m_u(&self, u: &[f64], i: usize) -> f64 {
        let at = |j: usize| u[self.base + j * self.stride];
        let ui = at(i);
        let mut s = 0.0;
        if i > 0 {
            s += self.g[i - 1] * (at(i - 1) - ui);
        }
        if i + 1 < self.n {
            s += self.g[i] * (at(i + 1) - ui);
        }
        if i == 0 {
            s -= self.lo.g * ui;
        }
        if i + 1 == self.n {
            s -= self.hi.g * ui;
        }
        s - self.ext_g[i] * ui
    }
}
