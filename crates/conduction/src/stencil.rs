//! Whole-grid coefficients of the steady conduction matrix `A = -M` (symmetric positive
//! semi-definite; positive definite once any boundary conducts), for fast products and sources.

use crate::error::{check_property, Error};
use crate::line::{others, Ctx, Line};
use basetherm_bc::{Boundaries, Interfaces, Side};
use basetherm_grid::Grid;

#[derive(Debug, Clone)]
pub struct Stencil {
    pub dims: [usize; 3],
    pub strides: [usize; 3],
    /// Heat capacity of each cell, J/K.
    pub cap: Vec<f64>,
    /// Conductance from each cell to its `+axis` neighbour, W/K (0 at the domain end).
    pub g_next: [Vec<f64>; 3],
    /// Diagonal of `A`: all conductances out of the cell, including boundary ones.
    pub diag: Vec<f64>,
    /// Boundary conductance per side face cell, W/K (0 for adiabatic).
    pub face_g: [Vec<f64>; 6],
    /// Boundary temperature per side face cell, C.
    pub face_t: [Vec<f64>; 6],
    /// Cell index adjacent to each side face cell.
    pub face_cell: [Vec<usize>; 6],
    /// Per cell: film conductance times group temperature of its interior faces, W.
    pub ext_src: Vec<f64>,
    /// Interior faces: `(cell, conductance W/K, group)`.
    pub internal: Vec<(usize, f64, u32)>,
    /// Temperature of each interior-face group when the stencil was built, C.
    pub group_t: Vec<f64>,
}

impl Stencil {
    pub fn new(
        grid: &Grid,
        k: &[f64],
        cv: &[f64],
        bc: &Boundaries,
        interfaces: Option<&Interfaces>,
    ) -> Result<Self, Error> {
        let n = grid.cell_count();
        check_property("k", k, n)?;
        check_property("cv", cv, n)?;
        let dims = grid.dims();
        let strides = [dims[1] * dims[2], dims[2], 1];
        let mut me = Self {
            dims,
            strides,
            cap: vec![0.0; n],
            g_next: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
            diag: vec![0.0; n],
            face_g: Side::ALL.map(|s| vec![0.0; basetherm_bc::side_len(grid, s)]),
            face_t: Side::ALL.map(|s| vec![0.0; basetherm_bc::side_len(grid, s)]),
            face_cell: Side::ALL.map(|s| vec![0; basetherm_bc::side_len(grid, s)]),
            ext_src: vec![0.0; n],
            internal: Vec::new(),
            group_t: interfaces.map_or_else(Vec::new, |x| x.temperatures().to_vec()),
        };
        for i in 0..dims[0] {
            for j in 0..dims[1] {
                for l in 0..dims[2] {
                    let idx = grid.index(i, j, l);
                    me.cap[idx] = cv[idx] * grid.cell_volume(i, j, l);
                }
            }
        }
        let ctx = Ctx {
            grid,
            k,
            cv,
            bc,
            interfaces,
        };
        let mut line = Line::default();
        for d in 0..3 {
            let (a, b) = others(d);
            for pa in 0..dims[a] {
                for pb in 0..dims[b] {
                    line.fill(&ctx, d, pa, pb);
                    let at = |i: usize| line.base + i * line.stride;
                    for i in 0..line.n - 1 {
                        me.g_next[d][at(i)] = line.g[i];
                        me.diag[at(i)] += line.g[i];
                        me.diag[at(i + 1)] += line.g[i];
                    }
                    for (side, end, cell) in [
                        (Side::lo(d), line.lo, at(0)),
                        (Side::hi(d), line.hi, at(line.n - 1)),
                    ] {
                        let s = side as usize;
                        me.face_g[s][line.face] = end.g;
                        me.face_t[s][line.face] = end.t;
                        me.face_cell[s][line.face] = cell;
                        me.diag[cell] += end.g;
                    }
                    for i in 0..line.n {
                        me.diag[at(i)] += line.ext_g[i];
                        me.ext_src[at(i)] += line.ext_gt[i];
                    }
                    for f in &line.faces {
                        me.internal.push((at(f.pos), f.g, f.group));
                    }
                }
            }
        }
        // A void cell has no couplings: make its row the identity so the system stays nonsingular.
        if let Some(x) = interfaces {
            for (c, &is_void) in x.void_cells().iter().enumerate() {
                if is_void {
                    me.diag[c] = 1.0;
                }
            }
        }
        Ok(me)
    }

    pub fn n(&self) -> usize {
        self.diag.len()
    }

    /// `y = A x` with `A = -M`.
    pub fn apply(&self, x: &[f64], y: &mut [f64]) {
        let [nx, ny, nz] = self.dims;
        let [sx, sy, _] = self.strides;
        let [gx, gy, gz] = &self.g_next;
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let c = i * sx + j * sy + k;
                    let mut s = self.diag[c] * x[c];
                    if i + 1 < nx {
                        s -= gx[c] * x[c + sx];
                    }
                    if i > 0 {
                        s -= gx[c - sx] * x[c - sx];
                    }
                    if j + 1 < ny {
                        s -= gy[c] * x[c + sy];
                    }
                    if j > 0 {
                        s -= gy[c - sy] * x[c - sy];
                    }
                    if k + 1 < nz {
                        s -= gz[c] * x[c + 1];
                    }
                    if k > 0 {
                        s -= gz[c - 1] * x[c - 1];
                    }
                    y[c] = s;
                }
            }
        }
    }

    /// Source of the mean problem: `sum g_b T_b` into each cell.
    pub fn mean_source(&self) -> Vec<f64> {
        let mut s = vec![0.0; self.n()];
        for side in 0..6 {
            for (f, &cell) in self.face_cell[side].iter().enumerate() {
                s[cell] += self.face_g[side][f] * self.face_t[side][f];
            }
        }
        for (c, v) in self.ext_src.iter().enumerate() {
            s[c] += v;
        }
        s
    }

    /// True when at least one boundary conducts (so the steady problem is not singular).
    pub fn is_anchored(&self) -> bool {
        self.face_g.iter().flatten().any(|&g| g > 0.0) || !self.internal.is_empty()
    }
}
