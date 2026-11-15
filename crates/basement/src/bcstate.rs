//! Boundary values for a step: the surface balance on open ground and exposed insulation, room
//! films on a slab on grade, and the far-field temperatures on the sides and bottom.

use crate::forcing::Forcing;
use crate::model::{Model, TopKind};
use basetherm_bc::{Bc, Boundaries, Side};
use basetherm_surface::{face_temperature, GroundSurface};

pub struct BcState {
    pub bc: Boundaries,
    /// Lagged surface temperature per top-face cell, C.
    ts: Vec<f64>,
    /// Half-cell conductance `2k/width` per top-face cell, W/(m2 K).
    h_cell: Vec<f64>,
    /// The film used in the last `apply`, per top-face cell: (h, t_inf).
    film: Vec<Option<(f64, f64)>>,
    ground: GroundSurface,
    insulation: GroundSurface,
    /// Sides and bottom follow the undisturbed column; off = adiabatic.
    condition_edges: bool,
}

impl BcState {
    pub fn new(
        model: &Model,
        ground: GroundSurface,
        insulation: GroundSurface,
        ts0: f64,
        condition_edges: bool,
    ) -> Self {
        let [nx, ny, _] = model.grid.dims();
        let h_cell = (0..nx * ny)
            .map(|f| {
                let c = model.grid.index(f / ny, f % ny, 0);
                let k = model.materials[usize::from(model.cell_material[c])].k_thawed;
                2.0 * k / model.grid.z.width(0)
            })
            .collect();
        Self {
            bc: Boundaries::adiabatic(&model.grid),
            ts: vec![ts0; nx * ny],
            h_cell,
            film: vec![None; nx * ny],
            ground,
            insulation,
            condition_edges,
        }
    }

    pub fn apply(&mut self, model: &Model, f: &Forcing) {
        let [nx, ny, nz] = model.grid.dims();
        for (face, kind) in model.top.iter().enumerate() {
            self.film[face] = None;
            let b = match *kind {
                TopKind::None | TopKind::Adiabatic => Bc::Adiabatic,
                TopKind::Ground | TopKind::Insulation => {
                    let s = if *kind == TopKind::Ground {
                        &self.ground
                    } else {
                        &self.insulation
                    };
                    let (h, t_inf) = s.film(&f.weather, self.ts[face]);
                    self.film[face] = Some((h, t_inf));
                    Bc::Convective { h, t_inf }
                }
                TopKind::Room(r) => Bc::Convective {
                    h: model.spec.h_floor,
                    t_inf: f.rooms[r],
                },
            };
            self.bc.set(Side::ZLo, face, b);
        }
        if !self.condition_edges {
            return;
        }
        for k in 0..nz {
            let t = Bc::Fixed(f.far[k]);
            for j in 0..ny {
                if !model.symmetry.x {
                    self.bc.set(Side::XLo, j * nz + k, t);
                }
                self.bc.set(Side::XHi, j * nz + k, t);
            }
            for i in 0..nx {
                if !model.symmetry.y {
                    self.bc.set(Side::YLo, i * nz + k, t);
                }
                self.bc.set(Side::YHi, i * nz + k, t);
            }
        }
        self.bc.fill(Side::ZHi, Bc::Fixed(f.far_bottom));
    }

    /// Update the lagged surface temperatures from the new state.
    pub fn after_step(&mut self, model: &Model, t: &[f64]) {
        let [_, ny, _] = model.grid.dims();
        for (face, film) in self.film.iter().enumerate() {
            if let Some((h, t_inf)) = *film {
                let c = model.grid.index(face / ny, face % ny, 0);
                self.ts[face] = face_temperature(h, t_inf, self.h_cell[face], t[c]);
            }
        }
    }
}
