use basetherm_basement::*;
use basetherm_materials::Material;

fn concrete() -> Material {
    Material::constant(1.7, 2.0e6)
}

fn foam() -> Material {
    Material::constant(0.035, 4.0e4)
}

fn room(name: &str, x0: f64, x1: f64, t: f64) -> Room {
    Room::rect(name, x0, x1, 0.0, 8.0, Schedule::Constant(t))
}

/// 10 x 8 m interior, one 2.5 m level with two rooms (split at x = 6), insulated walls and floor.
fn basement() -> Building {
    Building {
        lx: 10.0,
        ly: 8.0,
        levels: vec![Level {
            name: "basement".into(),
            height: 2.5,
            rooms: vec![room("west", 0.0, 6.0, 20.0), room("east", 6.0, 10.0, 18.0)],
        }],
        wall: vec![Layer::new(0.25, concrete()), Layer::new(0.10, foam())],
        floor: vec![Layer::new(0.10, concrete()), Layer::new(0.05, foam())],
        ground_floor_rooms: vec![],
        h_wall: 8.0,
        h_floor: 6.0,
        soil: vec![
            SoilLayer {
                top: 0.0,
                material: Material::constant(1.2, 1.6e6),
            },
            SoilLayer {
                top: 3.0,
                material: Material::constant(1.8, 2.4e6),
            },
        ],
        footprint: None,
        insulation: None,
        domain: Domain {
            margin: 12.0,
            depth_below_floor: 12.0,
        },
    }
}

fn cell_at(m: &Model, x: f64, y: f64, z: f64) -> usize {
    let g = &m.grid;
    g.index(
        g.x.locate(x).unwrap(),
        g.y.locate(y).unwrap(),
        g.z.locate(z).unwrap(),
    )
}

#[test]
fn void_volume_and_per_room_areas_match_the_geometry() {
    let m = Model::build_with(&basement(), Preset::Draft, SymmetryMode::Off).unwrap();
    let [nx, ny, nz] = m.grid.dims();
    let mut void_volume = 0.0;
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                if m.interfaces.is_void(m.grid.index(i, j, k)) {
                    void_volume += m.grid.cell_volume(i, j, k);
                }
            }
        }
    }
    assert!(
        (void_volume - 10.0 * 8.0 * 2.5).abs() < 1e-9,
        "{void_volume}"
    );
    let area = |r, k| m.group_area(r, k);
    // West room: floor 6x8; exterior walls: west 8 x 2.5, south and north 6 x 2.5 each (the partition is adiabatic).
    assert!((area(0, Kind::Floor) - 48.0).abs() < 1e-9);
    assert!(
        (area(0, Kind::Wall) - (20.0 + 15.0 + 15.0)).abs() < 1e-9,
        "{}",
        area(0, Kind::Wall)
    );
    // East room: floor 4x8; walls: east 8 x 2.5, south and north 4 x 2.5 each.
    assert!((area(1, Kind::Floor) - 32.0).abs() < 1e-9);
    assert!(
        (area(1, Kind::Wall) - (20.0 + 10.0 + 10.0)).abs() < 1e-9,
        "{}",
        area(1, Kind::Wall)
    );
    assert_eq!(m.rooms.len(), 2);
}

#[test]
fn layers_soil_and_void_are_assigned_to_the_right_cells() {
    let m = Model::build_with(&basement(), Preset::Draft, SymmetryMode::Off).unwrap();
    let mat = |x, y, z| &m.materials[usize::from(m.cell_material[cell_at(&m, x, y, z)])];
    assert_eq!(mat(-0.10, 4.0, 1.0).k_thawed, 1.7, "wall concrete");
    assert_eq!(mat(-0.30, 4.0, 1.0).k_thawed, 0.035, "wall insulation");
    assert_eq!(
        mat(-1.0, 4.0, 1.0).k_thawed,
        1.2,
        "upper soil beside the wall"
    );
    assert_eq!(mat(-1.0, 4.0, 4.0).k_thawed, 1.8, "lower soil");
    assert_eq!(mat(5.0, 4.0, 2.55).k_thawed, 1.7, "floor concrete");
    assert_eq!(mat(5.0, 4.0, 2.63).k_thawed, 0.035, "floor insulation");
    assert!(m.interfaces.is_void(cell_at(&m, 5.0, 4.0, 1.0)));
    assert!(!m.interfaces.is_void(cell_at(&m, 5.0, 4.0, 2.55)));
    // Above the void nothing happens at grade; open ground is ground; the wall tops are adiabatic.
    let [_, ny, _] = m.grid.dims();
    let face = |x: f64, y: f64| m.grid.x.locate(x).unwrap() * ny + m.grid.y.locate(y).unwrap();
    assert_eq!(m.top[face(5.0, 4.0)], TopKind::None);
    assert_eq!(m.top[face(-0.1, 4.0)], TopKind::Adiabatic);
    assert_eq!(m.top[face(-3.0, 4.0)], TopKind::Ground);
}

#[test]
fn the_mesh_puts_faces_on_every_boundary_and_stays_graded() {
    let m = Model::build_with(&basement(), Preset::Standard, SymmetryMode::Off).unwrap();
    let has =
        |axis: &basetherm_grid::Axis, v: f64| axis.faces().iter().any(|f| (f - v).abs() < 1e-9);
    for v in [0.0, 6.0, 10.0, -0.25, -0.35, 10.35] {
        assert!(has(&m.grid.x, v), "x face at {v}");
    }
    for v in [0.0, 2.5, 2.6, 2.65, 3.0] {
        assert!(has(&m.grid.z, v), "z face at {v}");
    }
    // The growth limit holds inside each graded segment; where segments meet at layer boundaries the
    // jump is slightly larger (measured on this building: 1.30, 1.30, 1.43).
    for axis in [&m.grid.x, &m.grid.y, &m.grid.z] {
        assert!(
            axis.max_neighbour_ratio() <= 1.5,
            "{}",
            axis.max_neighbour_ratio()
        );
    }
    assert!(m.grid.cell_count() < 2_000_000, "{}", m.grid.cell_count());
}

#[test]
fn invalid_buildings_are_rejected() {
    let mut b = basement();
    b.levels[0].rooms[1] = room("east", 5.0, 10.0, 18.0); // overlaps the west room
    assert!(matches!(b.validate(), Err(Error::Spec(m)) if m.contains("overlap")));
    let mut b = basement();
    b.levels[0].rooms[0] = room("west", 0.0, 12.0, 20.0);
    assert!(b.validate().is_err());
    let mut b = basement();
    b.soil[0].top = 1.0;
    assert!(b.validate().is_err());
    let mut b = basement();
    b.wall.clear();
    assert!(b.validate().is_err());
    let mut b = basement();
    b.insulation = Some(HorizontalInsulation {
        width: 20.0,
        thickness: 0.05,
        depth: 0.0,
        material: foam(),
    });
    assert!(
        Model::build_with(&b, Preset::Draft, SymmetryMode::Off).is_err(),
        "insulation wider than the margin"
    );
}

#[test]
fn slab_on_grade_has_room_floors_on_the_top_face() {
    let mut b = basement();
    b.levels.clear();
    b.wall.clear();
    b.ground_floor_rooms = vec![room("west", 0.0, 6.0, 20.0), room("east", 6.0, 10.0, 18.0)];
    let m = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    assert!(
        m.interfaces.faces().is_empty(),
        "no void, so no interior faces"
    );
    assert!((m.group_area(0, Kind::Floor) - 48.0).abs() < 1e-9);
    assert!((m.group_area(1, Kind::Floor) - 32.0).abs() < 1e-9);
    let [_, ny, _] = m.grid.dims();
    let face = |x: f64, y: f64| m.grid.x.locate(x).unwrap() * ny + m.grid.y.locate(y).unwrap();
    assert_eq!(m.top[face(3.0, 4.0)], TopKind::Room(0));
    assert_eq!(m.top[face(8.0, 4.0)], TopKind::Room(1));
    assert_eq!(m.top[face(-3.0, 4.0)], TopKind::Ground);
}

#[test]
fn horizontal_insulation_forms_a_ring_with_its_own_top_kind() {
    let mut b = basement();
    b.insulation = Some(HorizontalInsulation {
        width: 1.5,
        thickness: 0.05,
        depth: 0.0,
        material: foam(),
    });
    let m = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    let mat = |x, y, z| m.materials[usize::from(m.cell_material[cell_at(&m, x, y, z)])].k_thawed;
    assert_eq!(
        mat(-0.35 - 0.7, 4.0, 0.02),
        0.035,
        "inside the ring, at the surface"
    );
    assert_eq!(mat(-0.35 - 2.0, 4.0, 0.02), 1.2, "beyond the ring");
    assert_eq!(mat(-0.35 - 0.7, 4.0, 0.5), 1.2, "below the ring");
    let [_, ny, _] = m.grid.dims();
    let face = |x: f64, y: f64| m.grid.x.locate(x).unwrap() * ny + m.grid.y.locate(y).unwrap();
    assert_eq!(m.top[face(-0.35 - 0.7, 4.0)], TopKind::Insulation);
}

#[test]
fn reduced_models_fold_back_to_the_full_areas_exactly() {
    // `basement()` is symmetric in y only (rooms span the whole y range, the x split is off-centre):
    // a half domain. The three-room variant below is symmetric in both directions: a quarter.
    let mut three = basement();
    three.lx = 8.0;
    three.levels[0].rooms = vec![
        room("west", 0.0, 3.0, 20.0),
        room("mid", 3.0, 5.0, 16.0),
        room("east", 5.0, 8.0, 20.0),
    ];
    for (name, b, expect) in [
        ("half", basement(), Symmetry { x: false, y: true }),
        ("quarter", three, Symmetry { x: true, y: true }),
    ] {
        let full = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
        let red = Model::build(&b, Preset::Draft).unwrap();
        assert_eq!(red.symmetry, expect, "{name}");
        assert!(red.grid.cell_count() < full.grid.cell_count(), "{name}");
        for r in 0..full.rooms.len() {
            for kind in [Kind::Floor, Kind::Wall] {
                let folded: f64 = red.room_weights[r]
                    .iter()
                    .map(|&(rp, m)| m * red.group_area(rp, kind))
                    .sum();
                let direct = full.group_area(r, kind);
                assert!(
                    (folded - direct).abs() < 1e-9 * direct.max(1.0),
                    "{name} room {r} {kind:?}: {folded} vs {direct}"
                );
            }
        }
    }
}
