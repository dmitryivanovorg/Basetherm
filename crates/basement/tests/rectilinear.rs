//! L-shaped footprints, polygon rooms, and partially insulated walls and floors.

use basetherm_basement::*;
use basetherm_materials::Material;

fn concrete() -> Material {
    Material::constant(1.7, 2.0e6)
}

fn foam() -> Material {
    Material::constant(0.035, 4.0e4)
}

fn poly_room(name: &str, poly: Vec<(f64, f64)>, t: f64) -> Room {
    Room {
        name: name.into(),
        poly,
        temperature: Schedule::Constant(t),
    }
}

/// An L: 8 x 6 m with the corner x > 5, y > 3 removed. One 2.5 m level, one L-shaped room.
fn ell() -> Building {
    let outline = vec![(0., 0.), (8., 0.), (8., 3.), (5., 3.), (5., 6.), (0., 6.)];
    Building {
        lx: 8.0,
        ly: 6.0,
        footprint: Some(outline.clone()),
        levels: vec![Level {
            name: "basement".into(),
            height: 2.5,
            rooms: vec![poly_room("all", outline, 20.0)],
        }],
        wall: vec![Layer::new(0.25, concrete()), Layer::new(0.10, foam())],
        floor: vec![Layer::new(0.10, concrete())],
        ground_floor_rooms: vec![],
        h_wall: 8.0,
        h_floor: 6.0,
        soil: vec![SoilLayer {
            top: 0.0,
            material: Material::constant(1.5, 2.2e6),
        }],
        insulation: None,
        domain: Domain {
            margin: 6.0,
            depth_below_floor: 6.0,
        },
    }
}

fn mat_k(m: &Model, x: f64, y: f64, z: f64) -> f64 {
    let g = &m.grid;
    let c = g.index(
        g.x.locate(x).unwrap(),
        g.y.locate(y).unwrap(),
        g.z.locate(z).unwrap(),
    );
    m.materials[usize::from(m.cell_material[c])].k_thawed
}

fn void_at(m: &Model, x: f64, y: f64, z: f64) -> bool {
    let g = &m.grid;
    m.interfaces.is_void(g.index(
        g.x.locate(x).unwrap(),
        g.y.locate(y).unwrap(),
        g.z.locate(z).unwrap(),
    ))
}

#[test]
fn an_l_shaped_footprint_gets_void_walls_and_soil_in_the_right_places() {
    let m = Model::build_with(&ell(), Preset::Draft, SymmetryMode::Off).unwrap();
    assert!(void_at(&m, 2.0, 4.5, 1.0) && void_at(&m, 7.0, 1.5, 1.0));
    assert!(
        !void_at(&m, 7.0, 4.5, 1.0),
        "the notch is outside the building"
    );
    assert_eq!(mat_k(&m, 7.0, 4.5, 1.0), 1.5, "soil in the notch");
    assert_eq!(mat_k(&m, 6.5, 4.5, 1.0), 1.5, "...beyond the wall");
    assert_eq!(
        mat_k(&m, 5.1, 4.5, 1.0),
        1.7,
        "wall concrete on the notch's inner face"
    );
    assert_eq!(mat_k(&m, 5.3, 4.5, 1.0), 0.035, "wall insulation");
    assert_eq!(mat_k(&m, 7.0, 3.1, 1.0), 1.7, "wall under the notch edge");
    // Footprint area 8*3 + 5*3 = 39; walls: perimeter 8+3+3+3+5+6... = 2 * (8 + 6) = 28 m.
    let area = |k| m.group_area(0, k);
    assert!(
        (area(Kind::Floor) - 39.0).abs() < 1e-9,
        "{}",
        area(Kind::Floor)
    );
    assert!(
        (area(Kind::Wall) - 28.0 * 2.5).abs() < 1e-9,
        "{}",
        area(Kind::Wall)
    );
    assert!(m.grid.x.faces().iter().any(|f| (f - 5.0).abs() < 1e-9));
    assert!(m.grid.y.faces().iter().any(|f| (f - 3.0).abs() < 1e-9));
}

#[test]
fn an_l_shape_is_not_mirror_symmetric_but_a_u_shape_is_in_x() {
    let m = Model::build(&ell(), Preset::Draft).unwrap();
    assert_eq!(m.symmetry, Symmetry { x: false, y: false });
    assert!(m.symmetry_notes.iter().any(|n| n.contains("footprint")));
    // A T: 8 x 6, with both top corners removed.
    let t = vec![
        (0., 0.),
        (8., 0.),
        (8., 3.),
        (6., 3.),
        (6., 6.),
        (2., 6.),
        (2., 3.),
        (0., 3.),
    ];
    let mut b = ell();
    b.footprint = Some(t.clone());
    b.levels[0].rooms = vec![poly_room("t", t, 20.0)];
    let m = Model::build(&b, Preset::Draft).unwrap();
    assert_eq!(m.symmetry, Symmetry { x: true, y: false });
    let full = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    for kind in [Kind::Floor, Kind::Wall] {
        let folded: f64 = m.room_weights[0]
            .iter()
            .map(|&(rp, w)| w * m.group_area(rp, kind))
            .sum();
        assert!((folded - full.group_area(0, kind)).abs() < 1e-9, "{kind:?}");
    }
}

#[test]
fn invalid_polygons_and_rooms_outside_the_footprint_are_rejected() {
    let mut b = ell();
    b.levels[0].rooms = vec![poly_room(
        "big",
        vec![(0., 0.), (8., 0.), (8., 6.), (0., 6.)],
        20.0,
    )];
    assert!(matches!(b.validate(), Err(Error::Spec(m)) if m.contains("inside the footprint")));
    let mut b = ell();
    b.footprint = Some(vec![
        (0., 0.),
        (8., 0.),
        (8., 3.),
        (5., 3.),
        (5., 7.),
        (0., 7.),
    ]);
    assert!(
        b.validate().is_err(),
        "footprint must span [0, lx] x [0, ly]"
    );
    let mut b = ell();
    b.footprint = Some(vec![(0., 0.), (8., 6.), (0., 6.), (0., 3.)]);
    assert!(b.validate().is_err(), "diagonal edge");
}

fn straight() -> Building {
    let mut b = ell();
    b.footprint = None;
    b.lx = 6.0;
    b.ly = 6.0;
    b.levels[0].rooms = vec![Room::rect(
        "r",
        0.0,
        6.0,
        0.0,
        6.0,
        Schedule::Constant(20.0),
    )];
    b
}

#[test]
fn wall_insulation_to_a_depth_leaves_bare_concrete_below_it() {
    let mut b = straight();
    b.wall[1] = Layer::new(0.10, foam()).limited(Limit::Depth(1.0));
    let m = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    assert_eq!(mat_k(&m, -0.30, 3.0, 0.5), 0.035, "insulated above 1 m");
    assert_eq!(
        mat_k(&m, -0.30, 3.0, 1.5),
        1.5,
        "soil against the concrete below it"
    );
    assert_eq!(mat_k(&m, -0.10, 3.0, 1.5), 1.7, "the concrete itself");
    assert!(m.grid.z.faces().iter().any(|f| (f - 1.0).abs() < 1e-9));
}

#[test]
fn interior_insulation_to_a_depth_opens_the_wall_cells_below_it_to_the_room() {
    let mut b = straight();
    // Interior side first: foam, then concrete. The foam stops at 1 m depth.
    b.wall = vec![
        Layer::new(0.10, foam()).limited(Limit::Depth(1.0)),
        Layer::new(0.25, concrete()),
    ];
    let m = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    assert_eq!(mat_k(&m, -0.05, 3.0, 0.5), 0.035);
    assert!(
        void_at(&m, -0.05, 3.0, 1.5),
        "no board there: open to the room"
    );
    assert_eq!(mat_k(&m, -0.2, 3.0, 1.5), 1.7);
    // Walls: the foam faces (4 x 6 x 1 m2), the concrete beyond the open strip (a 6.2 m square ring,
    // 1.5 m high) and the underside of the board (the ring's 6.2^2 - 6^2 m2). The strip's floor is a
    // ledge that belongs to the floor group.
    let ring = 6.2 * 6.2 - 36.0;
    let wall = m.group_area(0, Kind::Wall);
    assert!(
        (wall - (24.0 + 4.0 * 6.2 * 1.5 + ring)).abs() < 1e-9,
        "{wall}"
    );
    let floor = m.group_area(0, Kind::Floor);
    assert!((floor - (36.0 + ring)).abs() < 1e-9, "{floor}");
}

#[test]
fn floor_insulation_can_cover_the_perimeter_or_the_core_only() {
    for (limit, edge_foam, core_foam) in [
        (Limit::Perimeter(1.0), true, false),
        (Limit::Core(1.0), false, true),
    ] {
        let mut b = straight();
        b.floor = vec![
            Layer::new(0.10, concrete()),
            Layer::new(0.05, foam()).limited(limit),
        ];
        let m = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
        let z = 2.5 + 0.10 + 0.02;
        let k = |x: f64| mat_k(&m, x, 3.0, z);
        assert_eq!(k(0.5) == 0.035, edge_foam, "{limit:?} near the wall");
        assert_eq!(k(3.0) == 0.035, core_foam, "{limit:?} in the middle");
        let other = if edge_foam { k(3.0) } else { k(0.5) };
        assert_eq!(other, 1.5, "soil where the board is missing");
        assert!((m.group_area(0, Kind::Floor) - 36.0).abs() < 1e-9);
    }
}

#[test]
fn floor_insulation_on_top_of_the_slab_leaves_a_ledge_where_it_is_missing() {
    let mut b = straight();
    b.floor = vec![
        Layer::new(0.05, foam()).limited(Limit::Core(1.0)),
        Layer::new(0.10, concrete()),
    ];
    let m = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    assert!(
        void_at(&m, 0.5, 3.0, 2.52),
        "no board near the walls: open to the room"
    );
    assert_eq!(mat_k(&m, 3.0, 3.0, 2.52), 0.035);
    assert!(!void_at(&m, 3.0, 3.0, 2.52));
    // The floor area is still the whole 6 x 6; the board edge adds a step to the wall group.
    assert!((m.group_area(0, Kind::Floor) - 36.0).abs() < 1e-9);
}

#[test]
fn layer_limits_are_validated() {
    let mut b = straight();
    b.wall[1] = Layer::new(0.1, foam()).limited(Limit::Perimeter(1.0));
    assert!(b.validate().is_err());
    let mut b = straight();
    b.floor[0] = Layer::new(0.1, foam()).limited(Limit::Core(1.0));
    assert!(b.validate().is_err(), "a floor needs a full layer");
    let mut b = straight();
    b.wall[0] = Layer::new(0.1, foam()).limited(Limit::Depth(1.0));
    b.wall[1] = Layer::new(0.1, foam()).limited(Limit::Depth(2.0));
    assert!(b.validate().is_err(), "a wall needs a full layer");
}
