//! One implicit-Euler step by Douglas ADI splitting.
//!
//! With `C du/dt = sum_d A_d(u)`, `A_d(u) = M_d u + s_d` (conduction along axis `d` plus its boundary
//! terms), the step is
//!   stage 0: `C u0 = C u_old + dt * sum_d A_d(u_old)`
//!   stage d: `(C - dt M_d) u_d = C u_{d-1} - dt M_d u_old`   (a tridiagonal solve per line)
//! The last stage equals `C u_new = C u_old + dt * sum_d A_d(u_d)`, so heat leaving through the two
//! sides of axis `d`, measured on `u_d`, balances the energy change exactly. Interior faces next to
//! void cells (see `basetherm_bc::Interfaces`) are boundary terms of the axis they are normal to and
//! are accounted the same way, per group.
//!
//! The lines of one axis are independent, so each pass is split into fixed slabs (one per x-block for
//! the y and z passes, one per y-slice for the x pass) that run in parallel when the `parallel`
//! feature is on. Slab boundaries depend only on the grid, and reductions (the group fluxes) are
//! added in slab order, so the result does not depend on the number of threads.

use crate::kernel::{JobOut, Kernel, LineId, Scratch};
use crate::par::{copy, for_each_chunk, for_each_plain};
use basetherm_bc::{Boundaries, Fluxes, Interfaces, Side};
use basetherm_conduction::{check_property, Error as AdiError};
use basetherm_grid::Grid;

pub struct Adi {
    grid: Grid,
    k: Vec<f64>,
    cv: Vec<f64>,
    /// Heat capacity of each cell, J/K.
    cap: Vec<f64>,
    old: Vec<f64>,
    cur: Vec<f64>,
    next: Vec<f64>,
    /// Output slabs of the x pass (`ny` slabs of `nx * nz` values).
    xbuf: Vec<f64>,
    outs: Vec<JobOut>,
    interfaces: Option<Interfaces>,
}

/// What a pass does with the values it computes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Stage 0: add `dt * A_d(old)` to the target.
    Explicit,
    /// Stages 1-3: solve from `prev`, overwrite the target, report fluxes.
    Solve,
}

impl Adi {
    /// `k` in W/(m K) and `cv` in J/(m3 K), one value per cell (index as in [`Grid::index`]).
    pub fn new(grid: Grid, k: Vec<f64>, cv: Vec<f64>) -> Result<Self, AdiError> {
        let n = grid.cell_count();
        let [nx, ny, _] = grid.dims();
        let mut me = Self {
            grid,
            k: Vec::new(),
            cv: Vec::new(),
            cap: vec![0.0; n],
            old: vec![0.0; n],
            cur: vec![0.0; n],
            next: vec![0.0; n],
            xbuf: vec![0.0; n],
            outs: (0..nx.max(ny)).map(|_| JobOut::default()).collect(),
            interfaces: None,
        };
        me.set_properties(k, cv)?;
        Ok(me)
    }

    /// A solver for a domain with void cells and interior faces (a basement in the soil). Void cells
    /// need positive placeholder `k` and `cv`; they carry no heat.
    pub fn with_interfaces(
        grid: Grid,
        k: Vec<f64>,
        cv: Vec<f64>,
        interfaces: Interfaces,
    ) -> Result<Self, AdiError> {
        let mut me = Self::new(grid, k, cv)?;
        me.interfaces = Some(interfaces);
        Ok(me)
    }

    pub fn interfaces(&self) -> Option<&Interfaces> {
        self.interfaces.as_ref()
    }

    /// Change group temperatures between steps.
    pub fn interfaces_mut(&mut self) -> Option<&mut Interfaces> {
        self.interfaces.as_mut()
    }

    /// Replace the cell properties (the freezing update calls this between steps).
    pub fn set_properties(&mut self, k: Vec<f64>, cv: Vec<f64>) -> Result<(), AdiError> {
        let n = self.grid.cell_count();
        check_property("k", &k, n)?;
        check_property("cv", &cv, n)?;
        let [nx, ny, nz] = self.grid.dims();
        let grid = &self.grid;
        let block = ny * nz;
        let cv_ref = &cv;
        for_each_plain(&mut self.cap, block, |i, chunk| {
            for j in 0..ny {
                for l in 0..nz {
                    chunk[j * nz + l] = cv_ref[i * block + j * nz + l] * grid.cell_volume(i, j, l);
                }
            }
        });
        let _ = nx;
        self.k = k;
        self.cv = cv;
        Ok(())
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Heat content `sum C T`, J (relative to 0 C).
    pub fn heat_content(&self, t: &[f64]) -> f64 {
        self.cap.iter().zip(t).map(|(c, x)| c * x).sum()
    }

    /// One pass along axis `d`. `prev` is the stage input (solve mode); the result goes to `target`.
    #[allow(clippy::too_many_arguments)]
    fn pass(
        kernel: &Kernel,
        d: usize,
        mode: Mode,
        prev: &[f64],
        target: &mut [f64],
        xbuf: &mut [f64],
        outs: &mut [JobOut],
        groups: usize,
    ) {
        let [nx, ny, nz] = kernel.grid.dims();
        let longest = nx.max(ny).max(nz);
        let block = ny * nz;
        let add = mode == Mode::Explicit;
        let init = || Scratch::new(longest);
        let write = |slot: &mut f64, v: f64| {
            if add {
                *slot += v;
            } else {
                *slot = v;
            }
        };
        let run_line = |s: &mut Scratch,
                        out: &mut JobOut,
                        pa: usize,
                        pb: usize,
                        put: &mut dyn FnMut(usize, f64)| {
            match mode {
                Mode::Explicit => kernel.explicit_line(s, LineId { d, pa, pb }, put),
                Mode::Solve => kernel.solve_line(s, LineId { d, pa, pb }, prev, out, put),
            }
        };
        match d {
            // z lines (i, j): contiguous inside the x-block of `i`.
            2 => for_each_chunk(target, block, &mut outs[..nx], init, |s, i, chunk, out| {
                out.reset(groups);
                for j in 0..ny {
                    run_line(s, out, i, j, &mut |pos, v| {
                        write(&mut chunk[j * nz + pos], v)
                    });
                }
            }),
            // y lines (i, k): strided inside the x-block of `i`.
            1 => for_each_chunk(target, block, &mut outs[..nx], init, |s, i, chunk, out| {
                out.reset(groups);
                for k in 0..nz {
                    run_line(s, out, i, k, &mut |pos, v| {
                        write(&mut chunk[pos * nz + k], v)
                    });
                }
            }),
            // x lines (j, k) span all x-blocks: solve into slabs, then copy the slabs in parallel.
            _ => {
                for_each_chunk(xbuf, nx * nz, &mut outs[..ny], init, |s, j, slab, out| {
                    out.reset(groups);
                    for k in 0..nz {
                        run_line(s, out, j, k, &mut |pos, v| slab[pos * nz + k] = v);
                    }
                });
                let slabs: &[f64] = xbuf;
                for_each_plain(target, block, |i, chunk| {
                    for j in 0..ny {
                        for k in 0..nz {
                            write(&mut chunk[j * nz + k], slabs[j * nx * nz + i * nz + k]);
                        }
                    }
                });
            }
        }
    }

    /// Advance `t` by `dt` seconds with boundary values held at their end-of-step values.
    /// `flux` receives the heat leaving through each boundary face cell, W, and through each
    /// interior-face group.
    pub fn step(
        &mut self,
        t: &mut [f64],
        dt: f64,
        bc: &Boundaries,
        flux: &mut Fluxes,
    ) -> Result<(), AdiError> {
        let n = self.grid.cell_count();
        if t.len() != n {
            return Err(AdiError::WrongLength {
                what: "temperature",
                expected: n,
                got: t.len(),
            });
        }
        copy(&mut self.old, t);
        flux.reset_groups();
        let groups = flux.groups().len();
        let [_, ny, nz] = self.grid.dims();
        let block = ny * nz;
        let Self {
            grid,
            k,
            cv,
            cap,
            old,
            cur,
            next,
            xbuf,
            outs,
            interfaces,
        } = self;
        let kernel = Kernel {
            grid,
            k,
            cv,
            bc,
            interfaces: interfaces.as_ref(),
            old,
            dt,
        };

        // Stage 0: C u_old plus the explicit increments of all three axes, then divide by C.
        for_each_plain(cur, block, |i, chunk| {
            for (x, idx) in chunk.iter_mut().zip(i * block..) {
                *x = cap[idx] * old[idx];
            }
        });
        for d in 0..3 {
            Self::pass(&kernel, d, Mode::Explicit, &[], cur, xbuf, outs, groups);
        }
        for_each_plain(cur, block, |i, chunk| {
            for (x, idx) in chunk.iter_mut().zip(i * block..) {
                *x /= cap[idx];
            }
        });

        // Stages 1..3: implicit solve along each axis in turn.
        for d in 0..3 {
            Self::pass(&kernel, d, Mode::Solve, cur, next, xbuf, outs, groups);
            std::mem::swap(cur, next);
            let slabs = if d == 0 { ny } else { grid.dims()[0] };
            for out in &outs[..slabs] {
                for &(face, lo, hi) in &out.lines {
                    flux.side_mut(Side::lo(d))[face] = lo;
                    flux.side_mut(Side::hi(d))[face] = hi;
                }
                for (g, part) in out.groups.iter().enumerate() {
                    if let Some(total) = flux.group_mut(g) {
                        *total += part;
                    }
                }
            }
        }
        copy(t, cur);
        Ok(())
    }
}
