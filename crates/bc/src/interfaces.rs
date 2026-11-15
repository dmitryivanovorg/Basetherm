//! Interior faces: a solid cell next to a void (inactive) cell, with a film to a group temperature.
//!
//! A basement is a hole in the soil, but the grid is a box. The hole's cells are marked void: they
//! carry no heat and have no couplings. The faces between solid cells and void cells act like outer
//! boundaries with a film coefficient `h` to the temperature of the room the void belongs to. Faces
//! are tagged with a group (for example room, level and floor-or-wall) so heat flow can be reported
//! per group. The temperature of a group can be changed at any time.

use crate::Side;

const NONE: u32 = u32::MAX;

/// A film between solid cell `cell` and the void on its `side`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InternalFace {
    pub cell: usize,
    /// Direction from the solid cell towards the void.
    pub side: Side,
    /// Film coefficient, W/(m2 K).
    pub h: f64,
    pub group: u32,
}

#[derive(Debug, Clone)]
pub struct Interfaces {
    void: Vec<bool>,
    faces: Vec<InternalFace>,
    /// Per cell and per side: index into `faces`, or `NONE`.
    slot: [Vec<u32>; 6],
    temps: Vec<f64>,
}

impl Interfaces {
    /// `void[c]` marks cells that are not solid; `groups` is the number of face groups.
    pub fn new(void: Vec<bool>, groups: usize) -> Self {
        let n = void.len();
        Self {
            void,
            faces: Vec::new(),
            slot: std::array::from_fn(|_| vec![NONE; n]),
            temps: vec![0.0; groups],
        }
    }

    /// Add a face. A solid cell can have one face per side. Panics on a void cell, a duplicate, or
    /// an unknown group (these are construction errors in the geometry builder).
    pub fn add_face(&mut self, face: InternalFace) {
        assert!(!self.void[face.cell], "an interior face needs a solid cell");
        assert!(
            (face.group as usize) < self.temps.len(),
            "unknown group {}",
            face.group
        );
        assert!(
            face.h > 0.0 && face.h.is_finite(),
            "film coefficient must be positive"
        );
        let slot = &mut self.slot[face.side as usize][face.cell];
        assert_eq!(*slot, NONE, "duplicate interior face");
        *slot = self.faces.len() as u32;
        self.faces.push(face);
    }

    pub fn is_void(&self, cell: usize) -> bool {
        self.void[cell]
    }

    pub fn void_cells(&self) -> &[bool] {
        &self.void
    }

    pub fn faces(&self) -> &[InternalFace] {
        &self.faces
    }

    pub fn face_at(&self, cell: usize, side: Side) -> Option<&InternalFace> {
        match self.slot[side as usize][cell] {
            NONE => None,
            i => Some(&self.faces[i as usize]),
        }
    }

    pub fn groups(&self) -> usize {
        self.temps.len()
    }

    pub fn temperature(&self, group: usize) -> f64 {
        self.temps[group]
    }

    pub fn set_temperature(&mut self, group: usize, t: f64) {
        self.temps[group] = t;
    }

    pub fn temperatures(&self) -> &[f64] {
        &self.temps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_are_found_by_cell_and_side() {
        let mut i = Interfaces::new(vec![false, false, true], 2);
        i.add_face(InternalFace {
            cell: 1,
            side: Side::ZHi,
            h: 8.0,
            group: 1,
        });
        assert!(i.is_void(2) && !i.is_void(1));
        assert_eq!(i.face_at(1, Side::ZHi).unwrap().group, 1);
        assert!(i.face_at(1, Side::ZLo).is_none() && i.face_at(0, Side::ZHi).is_none());
        i.set_temperature(1, 19.5);
        assert_eq!(i.temperature(1), 19.5);
        assert_eq!(i.faces().len(), 1);
    }

    #[test]
    #[should_panic(expected = "duplicate")]
    fn duplicate_faces_are_rejected() {
        let mut i = Interfaces::new(vec![false, true], 1);
        let f = InternalFace {
            cell: 0,
            side: Side::XHi,
            h: 5.0,
            group: 0,
        };
        i.add_face(f);
        i.add_face(f);
    }

    #[test]
    #[should_panic(expected = "solid cell")]
    fn faces_on_void_cells_are_rejected() {
        let mut i = Interfaces::new(vec![true], 1);
        i.add_face(InternalFace {
            cell: 0,
            side: Side::XHi,
            h: 5.0,
            group: 0,
        });
    }
}
