//! Mesh builder: fine cells at every geometric edge, geometric growth away from them.

use crate::error::Error;
use crate::geom::{self, Pt};
use crate::spec::Building;
use crate::spec::Limit;
use crate::symmetry::{self, Symmetry};
use basetherm_grid::{Axis, GradingSpec, Grid};

/// Grading presets, defined by cell sizes and growth, not cell counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Draft,
    Standard,
    Fine,
}

impl Preset {
    /// (horizontal finest cell, vertical finest cell, largest cell, growth ratio), m.
    pub fn sizes(self) -> (f64, f64, f64, f64) {
        match self {
            Preset::Draft => (0.20, 0.15, 3.0, 1.4),
            Preset::Standard => (0.10, 0.075, 2.0, 1.25),
            Preset::Fine => (0.05, 0.04, 1.5, 1.15),
        }
    }
}

/// Cumulative layer boundaries, m from the interior face.
fn cumulative(thicknesses: impl Iterator<Item = f64>) -> Vec<f64> {
    thicknesses
        .scan(0.0, |acc, t| {
            *acc += t;
            Some(*acc)
        })
        .collect()
}

/// Horizontal anchors along axis `ax` (0 = x, 1 = y). Every distance field of the model (wall layers,
/// horizontal insulation, floor limits) is a Chebyshev distance to the footprint edges, so its
/// contours lie at each edge's extent along the axis, moved outward by the offset.
/// `out` are offsets that matter outside the footprint, `inn` those that matter on both sides.
fn horizontal_anchors(
    outline: &[Pt],
    ax: usize,
    out: &[f64],
    inn: &[f64],
    edges: impl Iterator<Item = f64>,
) -> Vec<f64> {
    let at = |p: Pt, i: usize| if i == 0 { p.0 } else { p.1 };
    let n = outline.len();
    let mut a = Vec::new();
    for i in 0..n {
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        let (lo, hi) = (at(p, ax).min(at(q, ax)), at(p, ax).max(at(q, ax)));
        a.extend([lo, hi]);
        if hi - lo < 1e-9 {
            // Perpendicular edge: outward is away from the polygon interior (counter-clockwise).
            let along = at(q, 1 - ax) - at(p, 1 - ax);
            let outward = if ax == 0 {
                along.signum()
            } else {
                -along.signum()
            };
            a.extend(out.iter().map(|c| lo + outward * c));
            a.extend(inn.iter().flat_map(|w| [lo - w, lo + w]));
        } else {
            a.extend(out.iter().flat_map(|c| [lo - c, hi + c]));
            a.extend(inn.iter().flat_map(|w| [lo - w, hi + w]));
        }
    }
    a.extend(edges);
    a
}

/// The grid for a building. Faces fall exactly on every layer, room and level boundary, so a
/// cell never straddles two materials.
pub fn build_grid(b: &Building, preset: Preset, sym: Symmetry) -> Result<Grid, Error> {
    let (h_xy, h_z, h_max, ratio) = preset.sizes();
    let rooms = b.rooms();
    let tw = b.wall_thickness();
    let reach = tw + b.domain.margin;
    if let Some(h) = &b.insulation {
        if h.width > b.domain.margin {
            return Err(Error::Mesh(
                "horizontal insulation reaches beyond the domain margin".into(),
            ));
        }
    }
    let spec = |h_min| GradingSpec {
        h_min,
        h_max,
        ratio,
    };
    // Far-field ends carry almost no gradient (coupled to the undisturbed column), so they are not
    // refined; a mirror plane and the ground surface are real features and stay fine.
    let axis = |lo: f64, hi: f64, anchors: Vec<f64>, h_min: f64, fine: [bool; 2]| {
        Axis::graded_ends(lo, hi, &anchors, spec(h_min), fine)
            .map_err(|e| Error::Mesh(e.to_string()))
    };
    // With a mirror plane the domain starts at the centre plane, which gets a face of its own.
    let outline = geom::ccw(&b.outline());
    let mut out = cumulative(b.wall.iter().map(|l| l.thickness));
    if let Some(h) = &b.insulation {
        out.push(tw + h.width);
    }
    let inn: Vec<f64> = b
        .floor
        .iter()
        .filter_map(|l| match l.limit {
            Limit::Perimeter(w) | Limit::Core(w) => Some(w),
            _ => None,
        })
        .collect();
    let anchors = |ax: usize, len: f64| {
        let room_edges = rooms
            .iter()
            .flat_map(|(_, r)| r.poly.iter().map(move |p| if ax == 0 { p.0 } else { p.1 }));
        horizontal_anchors(&outline, ax, &out, &inn, room_edges.chain([len / 2.0]))
    };
    let x = axis(
        if sym.x { b.lx / 2.0 } else { -reach },
        b.lx + reach,
        anchors(0, b.lx),
        h_xy,
        [sym.x, false],
    )?;
    let y = axis(
        if sym.y { b.ly / 2.0 } else { -reach },
        b.ly + reach,
        anchors(1, b.ly),
        h_xy,
        [sym.y, false],
    )?;

    let d = b.depth();
    let mut za = vec![0.0, d];
    za.extend(cumulative(b.levels.iter().map(|l| l.height)));
    let mut acc = d;
    for l in &b.floor {
        acc += l.thickness;
        za.push(acc);
    }
    za.extend(b.soil.iter().map(|s| s.top));
    za.extend(b.wall.iter().filter_map(|l| match l.limit {
        Limit::Depth(d) => Some(d),
        _ => None,
    }));
    if let Some(h) = &b.insulation {
        za.push(h.depth);
        za.push(h.depth + h.thickness);
    }
    let z_end = d + b.floor_thickness() + b.domain.depth_below_floor;
    let z = axis(0.0, z_end, za, h_z, [true, false])?;
    Ok(Grid::new(x, y, z))
}

/// The grid a run would use, with the symmetry planes it would use and the notes for planes it would
/// not. Validates the building and meshes it, but classifies no cells, so it is cheap enough to call
/// while the user edits.
pub fn preview_grid(b: &Building, preset: Preset) -> Result<(Grid, Symmetry, Vec<String>), Error> {
    b.validate()?;
    let (sym, notes) = symmetry::detect(b);
    Ok((build_grid(b, preset, sym)?, sym, notes))
}
