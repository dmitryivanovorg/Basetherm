//! Mirror symmetry about the building's centre planes `x = lx/2` and `y = ly/2`.
//!
//! The model is mirror-symmetric when every room has a mirror partner on the same level (the
//! mirrored rectangle, with an identical temperature schedule). Wall and floor layers, soil layers,
//! the horizontal insulation, the domain margins and the (uniform) weather are symmetric by
//! construction. Symmetry is detected, used, and reported; it is never assumed.

use crate::geom;
use crate::spec::Building;

/// Which centre planes the reduced model uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Symmetry {
    pub x: bool,
    pub y: bool,
}

impl Symmetry {
    /// Number of mirror images of the reduced domain that make up the full one.
    pub fn images(&self) -> usize {
        1 << (usize::from(self.x) + usize::from(self.y))
    }
}

/// Whether to look for symmetry or always solve the full domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Auto,
    Off,
}

/// Index of the room that is the mirror image of room `i` (same level, same schedule), if any.
fn partner(b: &Building, i: usize, flip_x: bool, flip_y: bool) -> Result<usize, String> {
    let rooms = b.rooms();
    let (level, r) = rooms[i];
    let want = geom::canonical(&geom::mirror(&r.poly, b.lx, b.ly, flip_x, flip_y));
    let found = rooms
        .iter()
        .position(|(l, o)| *l == level && geom::canonical(&o.poly) == want);
    let j = found.ok_or_else(|| format!("room '{}' has no mirror partner", r.name))?;
    let o = rooms[j].1;
    if (0..8760).any(|h| (r.temperature.at(h) - o.temperature.at(h)).abs() > 1e-12) {
        return Err(format!(
            "rooms '{}' and '{}' mirror each other but have different temperatures",
            r.name, o.name
        ));
    }
    Ok(j)
}

/// Detect the symmetry planes of a building. The second value explains each plane that was not used.
pub fn detect(b: &Building) -> (Symmetry, Vec<String>) {
    let mut notes = Vec::new();
    let n = b.rooms().len();
    let outline = geom::canonical(&b.outline());
    let check = |flip_x: bool, flip_y: bool| -> Result<(), String> {
        if geom::canonical(&geom::mirror(&b.outline(), b.lx, b.ly, flip_x, flip_y)) != outline {
            return Err("the footprint is not symmetric".into());
        }
        (0..n).try_for_each(|i| partner(b, i, flip_x, flip_y).map(|_| ()))
    };
    let mut plane = |flip_x: bool, flip_y: bool, name: &str| match check(flip_x, flip_y) {
        Ok(()) => true,
        Err(why) => {
            notes.push(format!("no {name} symmetry: {why}"));
            false
        }
    };
    let x = plane(true, false, "x");
    let y = plane(false, true, "y");
    (Symmetry { x, y }, notes)
}

/// For every room `r`: the reduced-model rooms `r'` that fold onto it, with multiplicity `m`, so the
/// full flow of `r` is `sum m * Q(r')`. `m(r', r)` counts the reflections of the symmetry group that
/// map `r'` onto `r` (a room that is its own mirror gets 2 or 4).
pub fn weights(b: &Building, sym: Symmetry) -> Vec<Vec<(usize, f64)>> {
    let n = b.rooms().len();
    let mut reflections = vec![(false, false)];
    if sym.x {
        reflections.push((true, false));
    }
    if sym.y {
        reflections.push((false, true));
    }
    if sym.x && sym.y {
        reflections.push((true, true));
    }
    let mut w: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for rp in 0..n {
        for &(fx, fy) in &reflections {
            // Detection guarantees the partner exists; fall back to the room itself otherwise.
            let r = partner(b, rp, fx, fy).unwrap_or(rp);
            match w[r].iter_mut().find(|(q, _)| *q == rp) {
                Some(e) => e.1 += 1.0,
                None => w[r].push((rp, 1.0)),
            }
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::*;
    use basetherm_materials::Material;

    fn room(name: &str, x0: f64, x1: f64, y0: f64, y1: f64, t: f64) -> Room {
        Room::rect(name, x0, x1, y0, y1, Schedule::Constant(t))
    }

    fn building(rooms: Vec<Room>) -> Building {
        Building {
            lx: 8.0,
            ly: 6.0,
            levels: vec![Level {
                name: "b".into(),
                height: 2.5,
                rooms,
            }],
            footprint: None,
            wall: vec![Layer::new(0.2, Material::constant(1.7, 2.0e6))],
            floor: vec![Layer::new(0.1, Material::constant(1.7, 2.0e6))],
            ground_floor_rooms: vec![],
            h_wall: 8.0,
            h_floor: 6.0,
            soil: vec![SoilLayer {
                top: 0.0,
                material: Material::constant(1.5, 2.2e6),
            }],
            insulation: None,
            domain: Domain::default(),
        }
    }

    #[test]
    fn a_centred_room_and_a_mirror_pair_are_symmetric_in_both_directions() {
        let b = building(vec![
            room("west", 0.0, 3.0, 0.0, 6.0, 20.0),
            room("mid", 3.0, 5.0, 0.0, 6.0, 15.0),
            room("east", 5.0, 8.0, 0.0, 6.0, 20.0),
        ]);
        let (s, notes) = detect(&b);
        assert_eq!(s, Symmetry { x: true, y: true }, "{notes:?}");
        assert_eq!(s.images(), 4);
        let w = weights(&b, s);
        // west = east mirrored: each gets weight 2 of the other-or-itself (y reflection keeps it).
        let total = |r: usize| w[r].iter().map(|(_, m)| m).sum::<f64>();
        assert_eq!(total(0), 4.0);
        assert_eq!(total(1), 4.0);
        assert_eq!(
            w[1],
            vec![(1, 4.0)],
            "the centred room folds onto itself 4 times"
        );
        assert!(
            w[2].iter().any(|&(q, m)| q == 2 && m == 2.0)
                && w[2].iter().any(|&(q, m)| q == 0 && m == 2.0)
        );
    }

    #[test]
    fn different_temperatures_or_missing_partners_break_the_symmetry_with_a_reason() {
        let b = building(vec![
            room("west", 0.0, 4.0, 0.0, 6.0, 20.0),
            room("east", 4.0, 8.0, 0.0, 6.0, 18.0),
        ]);
        let (s, notes) = detect(&b);
        assert_eq!(s, Symmetry { x: false, y: true });
        assert!(
            notes.iter().any(|n| n.contains("different temperatures")),
            "{notes:?}"
        );
        let b = building(vec![
            room("west", 0.0, 3.0, 0.0, 6.0, 20.0),
            room("east", 3.0, 8.0, 0.0, 6.0, 20.0),
        ]);
        let (s, notes) = detect(&b);
        assert!(
            !s.x && notes.iter().any(|n| n.contains("no mirror partner")),
            "{notes:?}"
        );
    }

    #[test]
    fn a_single_full_room_is_symmetric_and_weights_are_its_image_count() {
        let b = building(vec![room("all", 0.0, 8.0, 0.0, 6.0, 20.0)]);
        let (s, _) = detect(&b);
        assert_eq!(s, Symmetry { x: true, y: true });
        assert_eq!(weights(&b, s)[0], vec![(0, 4.0)]);
    }
}
