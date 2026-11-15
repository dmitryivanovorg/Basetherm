//! Complex amplitude of the annual boundary temperature per face cell.

use basetherm_bc::{side_len, Side};
use basetherm_grid::Grid;
use basetherm_mg::C64;

/// For a `Convective` boundary this is the amplitude of the ambient temperature; for a `Fixed`
/// boundary it is the amplitude of the surface temperature. The boundary temperature is
/// `mean + Re(amp e^{i w t})`; the mean comes from the [`basetherm_bc::Boundaries`] values.
/// Ignored on adiabatic faces.
#[derive(Debug, Clone, PartialEq)]
pub struct Amplitudes {
    sides: [Vec<C64>; 6],
}

impl Amplitudes {
    pub fn zeros(grid: &Grid) -> Self {
        Self {
            sides: Side::ALL.map(|s| vec![C64::new(0.0, 0.0); side_len(grid, s)]),
        }
    }

    pub fn fill(&mut self, side: Side, a: C64) {
        self.sides[side as usize].fill(a);
    }

    pub fn set(&mut self, side: Side, index: usize, a: C64) {
        self.sides[side as usize][index] = a;
    }

    pub fn get(&self, side: Side, index: usize) -> C64 {
        self.sides[side as usize][index]
    }

    pub fn side(&self, side: Side) -> &[C64] {
        &self.sides[side as usize]
    }
}
