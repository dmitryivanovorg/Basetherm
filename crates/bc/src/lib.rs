//! Boundary conditions per face cell, and the heat flows through them. SI units.

mod interfaces;

use basetherm_grid::Grid;

pub use interfaces::{Interfaces, InternalFace};

/// The six sides of the box. x, y, z low and high.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    XLo = 0,
    XHi = 1,
    YLo = 2,
    YHi = 3,
    ZLo = 4,
    ZHi = 5,
}

impl Side {
    pub const ALL: [Side; 6] = [
        Side::XLo,
        Side::XHi,
        Side::YLo,
        Side::YHi,
        Side::ZLo,
        Side::ZHi,
    ];

    pub fn lo(axis: usize) -> Side {
        Self::ALL[2 * axis]
    }

    pub fn hi(axis: usize) -> Side {
        Self::ALL[2 * axis + 1]
    }
}

/// Condition on one boundary face cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bc {
    /// No heat flow.
    Adiabatic,
    /// Fixed surface temperature, C.
    Fixed(f64),
    /// Film coefficient `h` (W/m2K) to an ambient temperature `t_inf` (C).
    Convective { h: f64, t_inf: f64 },
}

/// Number of face cells on a side: the product of the other two axes' cell counts.
pub fn side_len(grid: &Grid, side: Side) -> usize {
    let [nx, ny, nz] = grid.dims();
    match side {
        Side::XLo | Side::XHi => ny * nz,
        Side::YLo | Side::YHi => nx * nz,
        Side::ZLo | Side::ZHi => nx * ny,
    }
}

/// Face cells are indexed `a * n_b + b`, where `a < b` are the two axes other than the side's
/// normal (x sides: y then z; y sides: x then z; z sides: x then y).
#[derive(Debug, Clone, PartialEq)]
pub struct Boundaries {
    sides: [Vec<Bc>; 6],
}

impl Boundaries {
    /// All sides adiabatic.
    pub fn adiabatic(grid: &Grid) -> Self {
        Self {
            sides: Side::ALL.map(|s| vec![Bc::Adiabatic; side_len(grid, s)]),
        }
    }

    pub fn fill(&mut self, side: Side, bc: Bc) {
        self.sides[side as usize].fill(bc);
    }

    pub fn set(&mut self, side: Side, index: usize, bc: Bc) {
        self.sides[side as usize][index] = bc;
    }

    pub fn get(&self, side: Side, index: usize) -> Bc {
        self.sides[side as usize][index]
    }

    pub fn side(&self, side: Side) -> &[Bc] {
        &self.sides[side as usize]
    }
}

/// Heat flowing out of the domain through each boundary face cell during a step, W.
/// Positive means heat leaves the domain.
#[derive(Debug, Clone, PartialEq)]
pub struct Fluxes {
    sides: [Vec<f64>; 6],
    groups: Vec<f64>,
}

impl Fluxes {
    pub fn zeros(grid: &Grid) -> Self {
        Self::with_groups(grid, 0)
    }

    /// Fluxes with room for `groups` interior-face groups (see [`Interfaces`]).
    pub fn with_groups(grid: &Grid, groups: usize) -> Self {
        Self {
            sides: Side::ALL.map(|s| vec![0.0; side_len(grid, s)]),
            groups: vec![0.0; groups],
        }
    }

    /// Heat leaving the solid into each interior-face group during the step, W.
    pub fn groups(&self) -> &[f64] {
        &self.groups
    }

    pub fn group_mut(&mut self, group: usize) -> Option<&mut f64> {
        self.groups.get_mut(group)
    }

    pub fn reset_groups(&mut self) {
        self.groups.fill(0.0);
    }

    pub fn side(&self, side: Side) -> &[f64] {
        &self.sides[side as usize]
    }

    pub fn side_mut(&mut self, side: Side) -> &mut [f64] {
        &mut self.sides[side as usize]
    }

    pub fn side_total(&self, side: Side) -> f64 {
        self.sides[side as usize].iter().sum()
    }

    /// Net heat leaving the solid through all sides and interior-face groups, W.
    pub fn total_out(&self) -> f64 {
        Side::ALL.iter().map(|&s| self.side_total(s)).sum::<f64>() + self.groups.iter().sum::<f64>()
    }
}
