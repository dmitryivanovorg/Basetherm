//! Which material a point of the domain is made of.

use super::{layer_index, Model};
use crate::geom;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Region {
    Void,
    Wall(usize),
    Floor(usize),
    Insulation,
    Soil(usize),
}

enum Pick {
    Layer(usize),
    Void,
    Soil,
}

/// The layer that fills a cell of layer `li` when some layers are absent: the layer itself if it
/// is there; open to the room if it and everything inside it is absent (and `allow_void`); else
/// the next layer outward; else soil.
fn pick(present: &[bool], li: usize, allow_void: bool) -> Pick {
    if present[li] {
        return Pick::Layer(li);
    }
    if allow_void && !present[..li].iter().any(|&p| p) {
        return Pick::Void;
    }
    match (li + 1..present.len()).find(|&j| present[j]) {
        Some(j) => Pick::Layer(j),
        None if present[..li].iter().any(|&p| p) => Pick::Soil,
        None => Pick::Layer(li),
    }
}

impl Model {
    pub(super) fn region(&self, x: f64, y: f64, z: f64) -> Region {
        let b = &self.spec;
        let (d, tw, tf) = (b.depth(), b.wall_thickness(), b.floor_thickness());
        let inside = geom::contains(&self.outline, x, y);
        let e = geom::edge_distance(&self.outline, x, y);
        // Distance outside the footprint, and distance inside it.
        let (dxy, depth_in) = if inside { (0.0, e) } else { (e, 0.0) };
        let soil = || Region::Soil(b.soil.iter().rposition(|s| s.top <= z).unwrap_or(0));
        if z < d {
            if inside {
                return Region::Void;
            }
            if dxy <= tw {
                let li = layer_index(b.wall.iter().map(|l| l.thickness), dxy);
                let present: Vec<bool> = b.wall.iter().map(|l| l.wall_present(z)).collect();
                return match pick(&present, li, true) {
                    Pick::Layer(j) => Region::Wall(j),
                    Pick::Void => Region::Void,
                    Pick::Soil => soil(),
                };
            }
        } else if z < d + tf && dxy <= tw {
            let li = layer_index(b.floor.iter().map(|l| l.thickness), z - d);
            let present: Vec<bool> = b.floor.iter().map(|l| l.floor_present(depth_in)).collect();
            // Open to the room only inside a basement footprint; elsewhere the layer is filled.
            return match pick(&present, li, inside && d > 0.0) {
                Pick::Layer(j) => Region::Floor(j),
                Pick::Void => Region::Void,
                Pick::Soil => soil(),
            };
        }
        if let Some(h) = &b.insulation {
            let out = dxy - tw;
            if out > 0.0 && out <= h.width && z >= h.depth && z < h.depth + h.thickness {
                return Region::Insulation;
            }
        }
        soil()
    }

    /// Index (into `rooms`) of the room containing `(x, y)` at depth `z`, if any. A point outside the
    /// footprint (a wall cell opened to the room by missing insulation) belongs to the nearest room.
    pub(super) fn room_at(&self, x: f64, y: f64, z: f64) -> Option<usize> {
        let b = &self.spec;
        let level = if b.levels.is_empty() {
            0
        } else {
            let mut top = 0.0;
            b.levels
                .iter()
                .position(|l| {
                    top += l.height;
                    z < top
                })
                .unwrap_or(b.levels.len() - 1)
        };
        let rooms = b.rooms();
        let on_level = rooms.iter().enumerate().filter(|(_, (li, _))| *li == level);
        if let Some((i, _)) = on_level
            .clone()
            .find(|(_, (_, r))| geom::contains(&r.poly, x, y))
        {
            return Some(i);
        }
        if geom::contains(&self.outline, x, y) {
            return None;
        }
        let reach = b.wall_thickness() + 1e-9;
        on_level
            .map(|(i, (_, r))| (i, geom::edge_distance(&r.poly, x, y)))
            .filter(|&(_, dist)| dist <= reach)
            .min_by(|a, c| a.1.total_cmp(&c.1))
            .map(|(i, _)| i)
    }
}
