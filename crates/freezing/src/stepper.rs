//! One conservative freezing step on top of the ADI solver.
//!
//! The Douglas ADI step satisfies `C_eff (T* - T_old) = dt * (net heat input)` cell by cell, so the
//! enthalpy that really entered each cell is `dH = C_eff (T* - T_old)`. The new temperature is the
//! exact inverse `T = H^-1(H(T_old) + dH)`: energy is conserved in enthalpy whatever `C_eff` was, and
//! `C_eff` only affects accuracy. `C_eff` is the tangent `dH/dT` at `T_old`; there is no iteration.
//!
//! A step that is far too long for the physics can over-extract heat: a cell in the latent band is
//! stiff, so the implicit solve leaves it near its old temperature, yet the heat drawn from cold
//! neighbours may exceed what it holds. The result then leaves the range that conduction allows
//! (the old temperatures and the boundary temperatures: the maximum principle). Such a step is
//! discarded and redone as two half-steps, recursively. Every sub-step conserves energy and the
//! fluxes are averaged, so the total stays exact. Hourly and daily steps are never split.

use crate::curve::EnthalpyCurve;
use basetherm_adi::Adi;
use basetherm_bc::{Bc, Boundaries, Fluxes, Interfaces, Side};
use basetherm_conduction::Error;
use basetherm_grid::Grid;
use basetherm_materials::Material;

/// What a step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepInfo {
    /// Number of sub-steps the step was split into (1 = not split).
    pub substeps: usize,
}

pub struct FreezingStepper {
    adi: Adi,
    curves: Vec<EnthalpyCurve>,
    cell_curve: Vec<u16>,
    volume: Vec<f64>,
    t_old: Vec<f64>,
    k: Vec<f64>,
    cv: Vec<f64>,
    /// A step whose result leaves the physical range by more than this (K) is halved. The default is
    /// the smallest band half-width: a smaller excursion is below the smearing of the phase change.
    pub overshoot_limit: f64,
}

impl FreezingStepper {
    /// Most times a step is halved (so a step is split into at most 2^8 parts).
    const MAX_HALVINGS: usize = 8;

    /// `materials[cell_material[c]]` is the soil of cell `c`; `t0` is the initial temperature field.
    pub fn new(
        grid: Grid,
        materials: &[Material],
        cell_material: Vec<u16>,
        t0: &[f64],
    ) -> Result<Self, Error> {
        Self::build(grid, materials, cell_material, t0, None)
    }

    /// For a domain with void cells and interior faces. Void cells still need a material index (any
    /// valid one); they carry no heat and are ignored by the physical-range check.
    pub fn with_interfaces(
        grid: Grid,
        materials: &[Material],
        cell_material: Vec<u16>,
        t0: &[f64],
        interfaces: Interfaces,
    ) -> Result<Self, Error> {
        Self::build(grid, materials, cell_material, t0, Some(interfaces))
    }

    /// Change interior-face group temperatures (room temperatures) between steps.
    pub fn interfaces_mut(&mut self) -> Option<&mut Interfaces> {
        self.adi.interfaces_mut()
    }

    fn build(
        grid: Grid,
        materials: &[Material],
        cell_material: Vec<u16>,
        t0: &[f64],
        interfaces: Option<Interfaces>,
    ) -> Result<Self, Error> {
        let n = grid.cell_count();
        if cell_material.len() != n {
            return Err(Error::WrongLength {
                what: "cell_material",
                expected: n,
                got: cell_material.len(),
            });
        }
        if let Some(index) = cell_material
            .iter()
            .position(|&m| usize::from(m) >= materials.len())
        {
            return Err(Error::BadProperty {
                what: "cell_material index",
                index,
            });
        }
        let curves: Vec<EnthalpyCurve> =
            materials.iter().map(EnthalpyCurve::from_material).collect();
        let mut volume = vec![0.0; n];
        for i in 0..grid.x.len() {
            for j in 0..grid.y.len() {
                for k in 0..grid.z.len() {
                    volume[grid.index(i, j, k)] = grid.cell_volume(i, j, k);
                }
            }
        }
        let curve = |c: usize| &curves[usize::from(cell_material[c])];
        let k: Vec<f64> = (0..n).map(|c| curve(c).conductivity(t0[c])).collect();
        let cv: Vec<f64> = (0..n).map(|c| curve(c).tangent_capacity(t0[c])).collect();
        let adi = match interfaces {
            Some(i) => Adi::with_interfaces(grid, k.clone(), cv.clone(), i)?,
            None => Adi::new(grid, k.clone(), cv.clone())?,
        };
        Ok(Self {
            adi,
            curves,
            cell_curve: cell_material,
            volume,
            t_old: t0.to_vec(),
            k,
            cv,
            overshoot_limit: Material::MIN_HALF_WIDTH,
        })
    }

    fn curve(&self, c: usize) -> &EnthalpyCurve {
        &self.curves[usize::from(self.cell_curve[c])]
    }

    /// Total enthalpy `sum V H(T)`, J (arbitrary zero). Changes by exactly the boundary heat input.
    pub fn heat_content(&self, t: &[f64]) -> f64 {
        (0..t.len())
            .map(|c| self.volume[c] * self.curve(c).enthalpy(t[c]))
            .sum()
    }

    /// Range that pure conduction cannot leave: old temperatures of solid cells, boundary temperatures
    /// and interior-face group temperatures.
    fn physical_range(&self, old: &[f64], bc: &Boundaries) -> (f64, f64) {
        let void = self.adi.interfaces().map(Interfaces::void_cells);
        let solid = |c: usize| !void.is_some_and(|v| v[c]);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for (_, &t) in old.iter().enumerate().filter(|(c, _)| solid(*c)) {
            lo = lo.min(t);
            hi = hi.max(t);
        }
        if let Some(i) = self.adi.interfaces() {
            for &t in i.temperatures() {
                lo = lo.min(t);
                hi = hi.max(t);
            }
        }
        for side in Side::ALL {
            for b in bc.side(side) {
                let t = match *b {
                    Bc::Adiabatic => continue,
                    Bc::Fixed(t) => t,
                    Bc::Convective { t_inf, .. } => t_inf,
                };
                lo = lo.min(t);
                hi = hi.max(t);
            }
        }
        (lo, hi)
    }

    /// One pass: tangent capacity and conductivity at the old state, ADI solve, enthalpy inversion.
    fn step_once(
        &mut self,
        t: &mut [f64],
        dt: f64,
        bc: &Boundaries,
        flux: &mut Fluxes,
    ) -> Result<(), Error> {
        for (c, &tc) in t.iter().enumerate() {
            let (cv, k) = {
                let curve = self.curve(c);
                (curve.tangent_capacity(tc), curve.conductivity(tc))
            };
            self.cv[c] = cv;
            self.k[c] = k;
        }
        self.adi.set_properties(self.k.clone(), self.cv.clone())?;
        self.t_old.copy_from_slice(t);
        self.adi.step(t, dt, bc, flux)?;
        for (c, tc) in t.iter_mut().enumerate() {
            let curve = self.curve(c);
            let d_h = self.cv[c] * (*tc - self.t_old[c]);
            *tc = curve.temperature(curve.enthalpy(self.t_old[c]) + d_h);
        }
        Ok(())
    }

    /// Advance `t` by `dt` seconds. `flux` receives the heat leaving through each boundary face cell
    /// averaged over the step (W); `dt * sum(flux)` is exactly the enthalpy lost by the domain.
    pub fn step(
        &mut self,
        t: &mut [f64],
        dt: f64,
        bc: &Boundaries,
        flux: &mut Fluxes,
    ) -> Result<StepInfo, Error> {
        self.step_adaptive(t, dt, bc, flux, 0)
    }

    fn step_adaptive(
        &mut self,
        t: &mut [f64],
        dt: f64,
        bc: &Boundaries,
        flux: &mut Fluxes,
        depth: usize,
    ) -> Result<StepInfo, Error> {
        let saved = t.to_vec();
        self.step_once(t, dt, bc, flux)?;
        let (lo, hi) = self.physical_range(&saved, bc);
        let void = self.adi.interfaces().map(Interfaces::void_cells);
        let overshoot = t
            .iter()
            .enumerate()
            .filter(|(c, _)| !void.is_some_and(|v| v[*c]))
            .fold(0.0_f64, |m, (_, &x)| m.max(lo - x).max(x - hi));
        let sound = t.iter().all(|x| x.is_finite()) && overshoot <= self.overshoot_limit;
        if sound || depth >= Self::MAX_HALVINGS {
            return Ok(StepInfo { substeps: 1 });
        }
        t.copy_from_slice(&saved);
        let mut first_half = flux.clone();
        let a = self.step_adaptive(t, 0.5 * dt, bc, &mut first_half, depth + 1)?;
        let b = self.step_adaptive(t, 0.5 * dt, bc, flux, depth + 1)?;
        for side in Side::ALL {
            for (second, first) in flux.side_mut(side).iter_mut().zip(first_half.side(side)) {
                *second = 0.5 * (*second + *first);
            }
        }
        Ok(StepInfo {
            substeps: a.substeps + b.substeps,
        })
    }
}
