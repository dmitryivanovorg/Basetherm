//! Cell classification and boundary structure for a building: which cells are void, which material
//! each solid cell has, the interior faces per room, and what each top-face cell is.

mod region;

use crate::error::Error;
use crate::geom::{self, Pt};
use crate::mesh::{build_grid, Preset};
use crate::spec::Building;
use crate::symmetry::{self, Mode, Symmetry};
use basetherm_bc::{Interfaces, InternalFace, Side};
use basetherm_grid::Grid;
use basetherm_materials::Material;
use region::Region;

/// Which part of the envelope an interior face belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Floor = 0,
    Wall = 1,
}

/// What the top face (grade level) of a column of cells is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopKind {
    /// Above a void: not part of the solid domain.
    None,
    /// Under the building walls or an unassigned floor area: no heat flow.
    Adiabatic,
    /// Open ground: the surface energy balance.
    Ground,
    /// Horizontal insulation exposed at the surface.
    Insulation,
    /// Slab on grade: the interior floor surface of room `.0` (index into [`Model::rooms`]).
    Room(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoomInfo {
    pub name: String,
    pub level: usize,
}

pub struct Model {
    pub spec: Building,
    pub grid: Grid,
    /// Soil layers, wall layers, floor layers, insulation, and last a placeholder for void cells.
    pub materials: Vec<Material>,
    pub cell_material: Vec<u16>,
    pub interfaces: Interfaces,
    pub rooms: Vec<RoomInfo>,
    /// One entry per top-face cell, indexed `ix * ny + iy`.
    pub top: Vec<TopKind>,
    soil_count: usize,
    /// Footprint outline, counter-clockwise.
    outline: Vec<Pt>,
    /// Mirror planes used (the grid starts at each used centre plane, which is adiabatic).
    pub symmetry: Symmetry,
    /// Why symmetry planes were not used (empty when none are missing).
    pub symmetry_notes: Vec<String>,
    /// For every room: the rooms of this (possibly reduced) model that fold onto it, with multiplicity.
    pub room_weights: Vec<Vec<(usize, f64)>>,
}

impl Model {
    /// Build the model, using mirror symmetry when it is detected.
    pub fn build(spec: &Building, preset: Preset) -> Result<Self, Error> {
        Self::build_with(spec, preset, Mode::Auto)
    }

    pub fn build_with(spec: &Building, preset: Preset, mode: Mode) -> Result<Self, Error> {
        spec.validate()?;
        let (sym, notes) = match mode {
            Mode::Auto => symmetry::detect(spec),
            Mode::Off => (
                Symmetry::default(),
                vec!["symmetry detection is switched off".to_string()],
            ),
        };
        let grid = build_grid(spec, preset, sym)?;
        let mut me = Self::classify(spec, grid)?;
        me.symmetry = sym;
        me.symmetry_notes = notes;
        me.room_weights = symmetry::weights(spec, sym);
        me.build_interfaces();
        Ok(me)
    }

    /// Full-building flows per group from the flows of this model's groups (undoes the folding).
    pub fn expand_flows(&self, groups: &[f64]) -> Vec<f64> {
        let mut full = vec![0.0; groups.len()];
        for (r, parts) in self.room_weights.iter().enumerate() {
            for kind in 0..2 {
                full[2 * r + kind] = parts.iter().map(|&(rp, m)| m * groups[2 * rp + kind]).sum();
            }
        }
        full
    }

    /// Interface group of a room's floor or walls.
    pub fn group(&self, room: usize, kind: Kind) -> usize {
        2 * room + kind as usize
    }

    pub fn void_material(&self) -> u16 {
        (self.materials.len() - 1) as u16
    }

    fn classify(spec: &Building, grid: Grid) -> Result<Self, Error> {
        let mut materials: Vec<Material> = spec.soil.iter().map(|s| s.material).collect();
        let soil_count = materials.len();
        materials.extend(spec.wall.iter().map(|l| l.material));
        materials.extend(spec.floor.iter().map(|l| l.material));
        if let Some(h) = &spec.insulation {
            materials.push(h.material);
        }
        materials.push(Material::constant(1.0, 1.0e6));
        let rooms = spec
            .rooms()
            .iter()
            .map(|(level, r)| RoomInfo {
                name: r.name.clone(),
                level: *level,
            })
            .collect();
        let n = grid.cell_count();
        let mut me = Self {
            spec: spec.clone(),
            grid,
            materials,
            cell_material: vec![0; n],
            interfaces: Interfaces::new(vec![false; n], 0),
            rooms,
            top: Vec::new(),
            soil_count,
            outline: geom::ccw(&spec.outline()),
            symmetry: Symmetry::default(),
            symmetry_notes: Vec::new(),
            room_weights: Vec::new(),
        };
        let [nx, ny, nz] = me.grid.dims();
        let wall0 = me.soil_count;
        let floor0 = wall0 + spec.wall.len();
        let ins = floor0 + spec.floor.len();
        let placeholder = me.void_material();
        let mut void = vec![false; n];
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let (x, y, z) = (
                        me.grid.x.center(i),
                        me.grid.y.center(j),
                        me.grid.z.center(k),
                    );
                    let c = me.grid.index(i, j, k);
                    let m = match me.region(x, y, z) {
                        Region::Void => {
                            void[c] = true;
                            placeholder as usize
                        }
                        Region::Wall(l) => wall0 + l,
                        Region::Floor(l) => floor0 + l,
                        Region::Insulation => ins,
                        Region::Soil(l) => l,
                    };
                    me.cell_material[c] = m as u16;
                }
            }
        }
        me.interfaces = Interfaces::new(void, 2 * me.rooms.len());
        let d0 = spec.depth() == 0.0;
        for i in 0..nx {
            for j in 0..ny {
                let (x, y, z) = (
                    me.grid.x.center(i),
                    me.grid.y.center(j),
                    me.grid.z.center(0),
                );
                me.top.push(match me.region(x, y, z) {
                    Region::Void => TopKind::None,
                    Region::Wall(_) | Region::Floor(_) => match (d0, me.room_at(x, y, z)) {
                        (true, Some(r)) if geom::contains(&me.outline, x, y) => TopKind::Room(r),
                        _ => TopKind::Adiabatic,
                    },
                    Region::Insulation => TopKind::Insulation,
                    Region::Soil(_) => TopKind::Ground,
                });
            }
        }
        Ok(me)
    }

    fn build_interfaces(&mut self) {
        let [nx, ny, nz] = self.grid.dims();
        let (hw, hf) = (self.spec.h_wall, self.spec.h_floor);
        let mut faces = Vec::new();
        for i in 0..nx {
            for j in 0..ny {
                for k in 0..nz {
                    let c = self.grid.index(i, j, k);
                    if !self.interfaces.is_void(c) {
                        continue;
                    }
                    let (x, y, z) = (
                        self.grid.x.center(i),
                        self.grid.y.center(j),
                        self.grid.z.center(k),
                    );
                    let Some(room) = self.room_at(x, y, z) else {
                        continue;
                    };
                    // (neighbour index, side of the solid neighbour that faces this void cell)
                    let mut neighbours = Vec::with_capacity(6);
                    if i + 1 < nx {
                        neighbours.push((self.grid.index(i + 1, j, k), Side::XLo));
                    }
                    if i > 0 {
                        neighbours.push((self.grid.index(i - 1, j, k), Side::XHi));
                    }
                    if j + 1 < ny {
                        neighbours.push((self.grid.index(i, j + 1, k), Side::YLo));
                    }
                    if j > 0 {
                        neighbours.push((self.grid.index(i, j - 1, k), Side::YHi));
                    }
                    if k + 1 < nz {
                        neighbours.push((self.grid.index(i, j, k + 1), Side::ZLo));
                    }
                    if k > 0 {
                        // The underside of a layer that is absent below it (partial insulation).
                        neighbours.push((self.grid.index(i, j, k - 1), Side::ZHi));
                    }
                    for (cell, side) in neighbours {
                        if self.interfaces.is_void(cell) {
                            continue;
                        }
                        let (kind, h) = if side == Side::ZLo {
                            (Kind::Floor, hf)
                        } else {
                            (Kind::Wall, hw)
                        };
                        faces.push(InternalFace {
                            cell,
                            side,
                            h,
                            group: self.group(room, kind) as u32,
                        });
                    }
                }
            }
        }
        for f in faces {
            self.interfaces.add_face(f);
        }
    }

    /// Area of the interior faces of a group, m2 (for slab on grade the floor is the top faces).
    pub fn group_area(&self, room: usize, kind: Kind) -> f64 {
        let [_, ny, nz] = self.grid.dims();
        let area = |c: usize, side: Side| {
            let (i, j, k) = (c / (ny * nz), (c / nz) % ny, c % nz);
            let (wx, wy, wz) = (
                self.grid.x.width(i),
                self.grid.y.width(j),
                self.grid.z.width(k),
            );
            match side {
                Side::XLo | Side::XHi => wy * wz,
                Side::YLo | Side::YHi => wx * wz,
                _ => wx * wy,
            }
        };
        let g = self.group(room, kind) as u32;
        let mut total: f64 = self
            .interfaces
            .faces()
            .iter()
            .filter(|f| f.group == g)
            .map(|f| area(f.cell, f.side))
            .sum();
        if kind == Kind::Floor {
            for (face, t) in self.top.iter().enumerate() {
                if *t == TopKind::Room(room) {
                    total += self.grid.x.width(face / ny) * self.grid.y.width(face % ny);
                }
            }
        }
        total
    }
}

/// Index of the layer containing distance `d` from the interior face.
pub(super) fn layer_index(thicknesses: impl Iterator<Item = f64>, d: f64) -> usize {
    let mut acc = 0.0;
    let mut last = 0;
    for (i, t) in thicknesses.enumerate() {
        acc += t;
        last = i;
        if d <= acc {
            return i;
        }
    }
    last
}
