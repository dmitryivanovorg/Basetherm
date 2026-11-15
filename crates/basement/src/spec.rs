//! The building description. Coordinates: `x`, `y` horizontal with the interior floor area
//! `[0, lx] x [0, ly]`; `z` is depth below grade (positive down). All SI.

use crate::error::Error;
use crate::geom::{self, Pt};
use basetherm_materials::Material;

/// Interior air temperature of a room, C.
#[derive(Debug, Clone, PartialEq)]
pub enum Schedule {
    Constant(f64),
    /// January to December.
    Monthly([f64; 12]),
    /// One value per hour of the 8760-hour year.
    Hourly(Vec<f64>),
}

const MONTH_DAYS: [usize; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

impl Schedule {
    /// Temperature in 0-based hour `hour` of the year.
    pub fn at(&self, hour: usize) -> f64 {
        match self {
            Schedule::Constant(t) => *t,
            Schedule::Monthly(m) => {
                let mut day = hour / 24;
                for (i, d) in MONTH_DAYS.iter().enumerate() {
                    if day < *d {
                        return m[i];
                    }
                    day -= d;
                }
                m[11]
            }
            Schedule::Hourly(h) => h[hour % h.len()],
        }
    }

    pub fn mean(&self) -> f64 {
        (0..8760).map(|h| self.at(h)).sum::<f64>() / 8760.0
    }

    fn validate(&self) -> Result<(), Error> {
        let ok = match self {
            Schedule::Constant(t) => t.is_finite(),
            Schedule::Monthly(m) => m.iter().all(|t| t.is_finite()),
            Schedule::Hourly(h) => h.len() == 8760 && h.iter().all(|t| t.is_finite()),
        };
        ok.then_some(())
            .ok_or_else(|| Error::Spec("room schedule must be finite (hourly: 8760 values)".into()))
    }
}

/// A room on a level: a rectilinear polygon in interior coordinates. Partitions between rooms are
/// adiabatic.
#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    pub name: String,
    pub poly: Vec<Pt>,
    pub temperature: Schedule,
}

impl Room {
    pub fn rect(name: &str, x0: f64, x1: f64, y0: f64, y1: f64, temperature: Schedule) -> Self {
        Room {
            name: name.into(),
            poly: geom::rectangle(x0, x1, y0, y1),
            temperature,
        }
    }
}

/// A storey below grade. Levels stack downward from grade in the order given.
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    pub name: String,
    pub height: f64,
    pub rooms: Vec<Room>,
}

/// Where a layer exists. Layers are listed interior side first; where one is absent the cells take
/// the next layer outward, or soil if none is left, or (for the innermost layers) are open to the room.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Limit {
    #[default]
    Full,
    /// Wall layer only: present above this depth below grade, m.
    Depth(f64),
    /// Floor layer only: present within this distance of the footprint edge (and under the walls), m.
    Perimeter(f64),
    /// Floor layer only: present further than this distance from the footprint edge, m.
    Core(f64),
}

/// A solid layer (concrete, insulation, ...), thickness in m.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub thickness: f64,
    pub material: Material,
    pub limit: Limit,
}

impl Layer {
    pub fn new(thickness: f64, material: Material) -> Self {
        Layer {
            thickness,
            material,
            limit: Limit::Full,
        }
    }

    pub fn limited(mut self, limit: Limit) -> Self {
        self.limit = limit;
        self
    }

    /// Whether a wall layer exists at depth `z`.
    pub fn wall_present(&self, z: f64) -> bool {
        match self.limit {
            Limit::Depth(d) => z < d,
            _ => true,
        }
    }

    /// Whether a floor layer exists where the footprint edge is `e` away (`e = 0` outside it).
    pub fn floor_present(&self, e: f64) -> bool {
        match self.limit {
            Limit::Perimeter(w) => e <= w,
            Limit::Core(w) => e > w,
            _ => true,
        }
    }
}

/// Soil from depth `top` (m below grade) down to the next layer; the last layer goes to the bottom.
#[derive(Debug, Clone, PartialEq)]
pub struct SoilLayer {
    pub top: f64,
    pub material: Material,
}

/// Horizontal insulation around the building, adjacent to the outer wall face.
#[derive(Debug, Clone, PartialEq)]
pub struct HorizontalInsulation {
    pub width: f64,
    pub thickness: f64,
    /// Depth of its top below grade; 0 = exposed at the surface.
    pub depth: f64,
    pub material: Material,
}

/// Domain extent beyond the building.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Domain {
    /// Lateral distance from the outer wall face to the far-field boundary, m (default 25).
    pub margin: f64,
    /// Soil depth below the underside of the floor layers, m.
    pub depth_below_floor: f64,
}

impl Default for Domain {
    fn default() -> Self {
        Self {
            margin: 25.0,
            depth_below_floor: 25.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Building {
    /// Bounding box of the interior floor area, `lx` by `ly`, m. All levels share the footprint.
    pub lx: f64,
    pub ly: f64,
    /// Outline of the footprint (rectilinear); `None` is the full rectangle `[0, lx] x [0, ly]`.
    pub footprint: Option<Vec<Pt>>,
    /// From grade downward. Empty means slab on grade (the floor surface is at grade).
    pub levels: Vec<Level>,
    /// Below-grade wall layers, interior side first.
    pub wall: Vec<Layer>,
    /// Floor layers below the lowest level, interior side first.
    pub floor: Vec<Layer>,
    /// Slab on grade only: rooms of the single floor (used when `levels` is empty).
    pub ground_floor_rooms: Vec<Room>,
    /// Interior film coefficients, W/(m2 K).
    pub h_wall: f64,
    pub h_floor: f64,
    pub soil: Vec<SoilLayer>,
    pub insulation: Option<HorizontalInsulation>,
    pub domain: Domain,
}

impl Building {
    /// Total depth of the levels, m (0 for slab on grade).
    pub fn depth(&self) -> f64 {
        self.levels.iter().map(|l| l.height).sum()
    }

    pub fn outline(&self) -> Vec<Pt> {
        self.footprint
            .clone()
            .unwrap_or_else(|| geom::rectangle(0.0, self.lx, 0.0, self.ly))
    }

    pub fn wall_thickness(&self) -> f64 {
        self.wall.iter().map(|l| l.thickness).sum()
    }

    pub fn floor_thickness(&self) -> f64 {
        self.floor.iter().map(|l| l.thickness).sum()
    }

    /// All rooms, level by level (slab on grade: the ground-floor rooms), with their level index.
    pub fn rooms(&self) -> Vec<(usize, &Room)> {
        if self.levels.is_empty() {
            return self.ground_floor_rooms.iter().map(|r| (0, r)).collect();
        }
        self.levels
            .iter()
            .enumerate()
            .flat_map(|(i, l)| l.rooms.iter().map(move |r| (i, r)))
            .collect()
    }

    pub fn validate(&self) -> Result<(), Error> {
        let bad = |m: &str| Err(Error::Spec(m.to_string()));
        if !(self.lx > 0.0 && self.ly > 0.0) {
            return bad("the interior footprint must have positive size");
        }
        if self.wall.is_empty() && !self.levels.is_empty() {
            return bad("a basement needs at least one wall layer");
        }
        if self.floor.is_empty() {
            return bad("the floor needs at least one layer");
        }
        for l in self.wall.iter().chain(&self.floor) {
            if !(l.thickness > 0.0 && l.thickness.is_finite()) {
                return bad("layer thickness must be positive");
            }
        }
        if self
            .levels
            .iter()
            .any(|l| !(l.height.is_finite() && l.height > 0.0))
        {
            return bad("level heights must be positive");
        }
        if !(self.h_wall > 0.0 && self.h_floor > 0.0) {
            return bad("interior film coefficients must be positive");
        }
        if self.soil.is_empty()
            || self.soil[0].top != 0.0
            || self.soil.windows(2).any(|w| w[1].top <= w[0].top)
        {
            return bad("soil layers must start at depth 0 and have increasing tops");
        }
        if !(self.domain.margin > 0.0 && self.domain.depth_below_floor > 0.0) {
            return bad("domain extents must be positive");
        }
        if let Some(h) = &self.insulation {
            if !(h.width > 0.0 && h.thickness > 0.0 && h.depth >= 0.0) {
                return bad("horizontal insulation needs positive width and thickness, depth >= 0");
            }
        }
        self.validate_layers()?;
        let outline = self.outline();
        if let Some(why) = geom::problem(&outline) {
            return Err(Error::Spec(format!("footprint: {why}")));
        }
        let (x0, x1, y0, y1) = geom::bounds(&outline);
        let eps = 1e-6;
        if x0.abs() > eps
            || y0.abs() > eps
            || (x1 - self.lx).abs() > eps
            || (y1 - self.ly).abs() > eps
        {
            return bad("the footprint must span exactly [0, lx] x [0, ly]");
        }
        let rooms = self.rooms();
        if rooms.is_empty() {
            return bad("the building needs at least one room");
        }
        for (i, (li, r)) in rooms.iter().enumerate() {
            r.temperature.validate()?;
            if let Some(why) = geom::problem(&r.poly) {
                return Err(Error::Spec(format!("room '{}': {why}", r.name)));
            }
            if !geom::covers(&outline, &r.poly) {
                return Err(Error::Spec(format!(
                    "room '{}' must lie inside the footprint",
                    r.name
                )));
            }
            for (lj, o) in rooms.iter().skip(i + 1) {
                if li == lj && geom::overlap(&r.poly, &o.poly) {
                    return Err(Error::Spec(format!(
                        "rooms '{}' and '{}' overlap",
                        r.name, o.name
                    )));
                }
            }
        }
        Ok(())
    }
}

impl Building {
    fn validate_layers(&self) -> Result<(), Error> {
        let bad = |m: &str| Err(Error::Spec(m.to_string()));
        let positive = |v: f64| v > 0.0 && v.is_finite();
        for l in &self.wall {
            if !matches!(l.limit, Limit::Full | Limit::Depth(_)) {
                return bad("a wall layer can only be limited in depth");
            }
            if let Limit::Depth(d) = l.limit {
                if !positive(d) {
                    return bad("the depth limit of a wall layer must be positive");
                }
            }
        }
        for l in &self.floor {
            match l.limit {
                Limit::Depth(_) => return bad("a floor layer cannot be limited in depth"),
                Limit::Perimeter(w) | Limit::Core(w) if !positive(w) => {
                    return bad("the width limit of a floor layer must be positive")
                }
                _ => {}
            }
        }
        if !self.wall.is_empty() && self.wall.iter().all(|l| l.limit != Limit::Full) {
            return bad("at least one wall layer must extend over the full depth");
        }
        if self.floor.iter().all(|l| l.limit != Limit::Full) {
            return bad("at least one floor layer must cover the whole floor");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_schedule_follows_the_calendar() {
        let s = Schedule::Monthly([1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 12.]);
        assert_eq!(s.at(0), 1.0);
        assert_eq!(s.at(31 * 24 - 1), 1.0);
        assert_eq!(s.at(31 * 24), 2.0);
        assert_eq!(s.at(8759), 12.0);
        assert!(
            (s.mean()
                - (31. + 56. + 93. + 120. + 155. + 180. + 217. + 248. + 270. + 310. + 330. + 372.)
                    / 365.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn constant_and_hourly_schedules() {
        assert_eq!(Schedule::Constant(20.0).at(5000), 20.0);
        let h = Schedule::Hourly((0..8760).map(|i| i as f64).collect());
        assert_eq!(h.at(100), 100.0);
        assert!(Schedule::Hourly(vec![1.0; 10]).validate().is_err());
    }
}
