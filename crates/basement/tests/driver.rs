//! End-to-end checks of the full-year driver on a small synthetic building. Slow; run with
//!   cargo test -p basetherm-basement --test driver -- --ignored --nocapture

use basetherm_basement::*;
use basetherm_materials::Material;
use basetherm_surface::{Convection, GroundSurface, Weather};
use std::f64::consts::PI;

fn weather() -> Vec<Weather> {
    let w = 2.0 * PI / 8760.0;
    (0..8760)
        .map(|i| Weather {
            t_air: 8.0 + 10.0 * (w * (i as f64 + 0.5) - 3.0).cos(),
            wind: 3.0,
            ghi: 100.0,
            ir_sky: 300.0,
        })
        .collect()
}

fn room(name: &str, x0: f64, x1: f64, t: f64) -> Room {
    Room::rect(name, x0, x1, 0.0, 8.0, Schedule::Constant(t))
}

/// 8 x 8 m interior, one 2.5 m basement level, insulated walls, small domain to keep runs short.
fn building(rooms: Vec<Room>) -> Building {
    Building {
        lx: 8.0,
        ly: 8.0,
        levels: vec![Level {
            name: "basement".into(),
            height: 2.5,
            rooms,
        }],
        wall: vec![
            Layer::new(0.25, Material::constant(1.7, 2.0e6)),
            Layer::new(0.08, Material::constant(0.035, 4.0e4)),
        ],
        floor: vec![Layer::new(0.12, Material::constant(1.7, 2.0e6))],
        ground_floor_rooms: vec![],
        h_wall: 8.0,
        h_floor: 6.0,
        soil: vec![SoilLayer {
            top: 0.0,
            material: Material::constant(1.5, 2.2e6),
        }],
        footprint: None,
        insulation: None,
        domain: Domain {
            margin: 5.0,
            depth_below_floor: 6.0,
        },
    }
}

fn options() -> RunOptions {
    let mut o = RunOptions::new(GroundSurface::new(0.25, 0.9, Convection::WALTON_ROUGH).unwrap());
    o.coarse_years = 6;
    o.hourly_years = 1;
    o
}

fn run_building(rooms: Vec<Room>) -> Results {
    let model = Model::build(&building(rooms), Preset::Draft).unwrap();
    println!("{} cells", model.grid.cell_count());
    run(&model, &weather(), &options()).unwrap()
}

#[test]
#[ignore = "slow: about 3 minutes (two full-year runs)"]
fn symmetric_rooms_get_equal_flows_and_a_split_room_adds_up_to_the_whole() {
    let split = run_building(vec![
        room("west", 0.0, 4.0, 20.0),
        room("east", 4.0, 8.0, 20.0),
    ]);
    let whole = run_building(vec![room("all", 0.0, 8.0, 20.0)]);
    let (w, e) = (split.annual_room(0), split.annual_room(1));
    println!(
        "west {w:.1} kWh, east {e:.1} kWh, whole {:.1} kWh (split total {:.1})",
        whole.annual_room(0),
        w + e
    );
    // Mirror symmetry: the two rooms are identical.
    assert!((w - e).abs() < 1e-3 * w.abs(), "west {w} vs east {e}");
    // The partition is adiabatic and both rooms are at the same temperature: the total is unchanged
    // (the mesh differs by one extra anchor line, so allow a small discretisation difference).
    let (a, b) = (w + e, whole.annual_room(0));
    assert!((a - b).abs() < 0.01 * b.abs(), "split {a} vs whole {b}");
    // A warm basement in cooler ground loses heat: negative in this project's sign convention.
    assert!(a < 0.0 && b < 0.0);
    // Floor and walls are reported separately, and both lose heat.
    assert!(split.annual(0, Kind::Floor) < 0.0 && split.annual(0, Kind::Wall) < 0.0);
}

#[test]
#[ignore = "slow: about 3 minutes (a full-year run)"]
fn a_warmer_room_loses_more_and_hourly_flows_sum_to_the_annual_energy() {
    let r = run_building(vec![
        room("warm", 0.0, 4.0, 22.0),
        room("cool", 4.0, 8.0, 14.0),
    ]);
    let (warm, cool) = (r.annual_room(0), r.annual_room(1));
    println!(
        "warm {warm:.1} kWh, cool {cool:.1} kWh, drift {:.4}, years {}+{}",
        r.drift, r.coarse_years, r.hourly_years
    );
    assert!(
        warm < cool,
        "the warmer room must lose more (more negative): {warm} vs {cool}"
    );
    let hourly: f64 = (0..8760)
        .map(|h| r.flow(h, 0, Kind::Floor) + r.flow(h, 0, Kind::Wall))
        .sum::<f64>()
        / 1000.0;
    assert!(
        (hourly - warm).abs() < 1e-6 * warm.abs(),
        "{hourly} vs {warm}"
    );
    assert_eq!(r.room_names, vec!["warm", "cool"]);

    // Monthly centre-plane sections: 12 months, faces of the whole domain, building void is NaN,
    // the soil is finite and the ground just under the warm floor is warmer in winter than far down.
    let s = &r.sections;
    let (nx, ny, nz) = (s.x.len() - 1, s.y.len() - 1, s.z.len() - 1);
    assert_eq!((s.longitudinal.len(), s.perpendicular.len()), (12, 12));
    assert!(s.longitudinal.iter().all(|m| m.len() == nx * nz));
    assert!(s.perpendicular.iter().all(|m| m.len() == ny * nz));
    assert!(
        s.x[0] < 0.0 && s.x.last().unwrap() > &8.0,
        "x covers the whole domain"
    );
    let jan = &s.longitudinal[0];
    let void = jan.iter().filter(|t| t.is_nan()).count();
    assert!(
        void > 0 && void < jan.len(),
        "void cells are masked, soil is not"
    );
    assert!(jan
        .iter()
        .filter(|t| !t.is_nan())
        .all(|t| (-30.0..40.0).contains(t)));
}

/// 4 x 4 m interior with three rooms across x: west and east are mirror images (20 C), the middle one
/// is centred (16 C), and every room spans the full y range, so the building is symmetric in both
/// directions and a quarter of the domain suffices.
fn symmetric_small() -> Building {
    let r = |name: &str, x0: f64, x1: f64, t: f64| {
        Room::rect(name, x0, x1, 0.0, 4.0, Schedule::Constant(t))
    };
    let mut b = building(vec![
        r("west", 0.0, 1.5, 20.0),
        r("mid", 1.5, 2.5, 16.0),
        r("east", 2.5, 4.0, 20.0),
    ]);
    b.lx = 4.0;
    b.ly = 4.0;
    b.domain = Domain {
        margin: 3.0,
        depth_below_floor: 3.0,
    };
    b
}

#[test]
#[ignore = "slow: about 90 seconds (a full-domain and a quarter-domain year)"]
fn a_quarter_domain_gives_the_same_room_flows_and_peaks_as_the_full_domain() {
    let b = symmetric_small();
    let full = Model::build_with(&b, Preset::Draft, SymmetryMode::Off).unwrap();
    let quarter = Model::build(&b, Preset::Draft).unwrap();
    assert_eq!(quarter.symmetry, Symmetry { x: true, y: true });
    assert!(
        quarter.grid.cell_count() * 3 < full.grid.cell_count(),
        "{} vs {}",
        quarter.grid.cell_count(),
        full.grid.cell_count()
    );
    let (rf, rq) = (
        run(&full, &weather(), &options()).unwrap(),
        run(&quarter, &weather(), &options()).unwrap(),
    );
    println!("full: {}\nquarter: {}", rf.symmetry, rq.symmetry);
    println!("{}", rq.report());
    for room in 0..3 {
        for kind in [Kind::Floor, Kind::Wall] {
            let (a, q) = (rf.annual(room, kind), rq.annual(room, kind));
            assert!(
                (a - q).abs() < 1e-3 * a.abs(),
                "room {room} {kind:?}: full {a} vs quarter {q}"
            );
        }
    }
    let same = |a: Option<Peak>, b: Option<Peak>, what: &str| match (a, b) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            assert_eq!(a.hour, b.hour, "{what}: peak hour");
            assert_eq!(a.date, b.date, "{what}: peak date");
            assert!(
                (a.watts - b.watts).abs() < 1e-3 * a.watts.abs(),
                "{what}: {} vs {}",
                a.watts,
                b.watts
            );
        }
        other => panic!("{what}: peak present in only one run: {other:?}"),
    };
    for scope in [
        Scope::Room(0),
        Scope::Room(1),
        Scope::Room(2),
        Scope::Basement,
    ] {
        let (pf, pq) = (rf.peaks(scope, None), rq.peaks(scope, None));
        same(pf.loss, pq.loss, &format!("{scope:?} loss"));
        same(pf.gain, pq.gain, &format!("{scope:?} gain"));
    }
    // The warm rooms lose heat all year; the cool middle room, surrounded by warmer rooms' soil, only gains.
    assert!(
        rq.peaks(Scope::Room(0), None).loss.is_some()
            && rq.peaks(Scope::Room(1), None).loss.is_none()
    );
    // The text report says what was done.
    assert!(
        rq.symmetry.to_string().contains("quarter")
            && rf.symmetry.to_string().contains("full domain")
    );
}

/// An L-shaped footprint (6 x 6 m with a 3 x 3 m corner removed), as one room or as two rooms at the
/// same temperature: the partition is adiabatic, so the totals must agree.
fn l_building(rooms: Vec<Room>) -> Building {
    let outline = vec![(0., 0.), (6., 0.), (6., 3.), (3., 3.), (3., 6.), (0., 6.)];
    let mut b = building(rooms);
    b.lx = 6.0;
    b.ly = 6.0;
    b.footprint = Some(outline);
    b.domain = Domain {
        margin: 4.0,
        depth_below_floor: 4.0,
    };
    b
}

#[test]
#[ignore = "slow: about 1 minute (two full-year runs of an L-shaped basement)"]
fn an_l_shaped_basement_split_into_two_rooms_adds_up_to_the_whole() {
    let poly = |p: &[(f64, f64)]| p.to_vec();
    let mk = |rooms| {
        let b = l_building(rooms);
        let m = Model::build(&b, Preset::Draft).unwrap();
        assert_eq!(m.symmetry, Symmetry { x: false, y: false });
        run(&m, &weather(), &options()).unwrap()
    };
    let whole = mk(vec![Room {
        name: "L".into(),
        poly: poly(&[(0., 0.), (6., 0.), (6., 3.), (3., 3.), (3., 6.), (0., 6.)]),
        temperature: Schedule::Constant(20.0),
    }]);
    let split = mk(vec![
        Room::rect("south", 0.0, 6.0, 0.0, 3.0, Schedule::Constant(20.0)),
        Room::rect("north", 0.0, 3.0, 3.0, 6.0, Schedule::Constant(20.0)),
    ]);
    let (a, b) = (split.annual_total(), whole.annual_total());
    println!(
        "L whole {b:.1} kWh, split {a:.1} kWh ({:.1} + {:.1})",
        split.annual_room(0),
        split.annual_room(1)
    );
    assert!(b < 0.0 && (a - b).abs() < 0.01 * b.abs(), "{a} vs {b}");
    // The south room (18 m2 + the shared corner block) is the larger.
    assert!(split.annual_room(0) < split.annual_room(1));
}

#[test]
#[ignore = "slow: about 1 minute (three full-year runs)"]
fn insulating_only_the_top_of_the_wall_loses_between_none_and_all() {
    let wall_loss = |foam: Option<Limit>| {
        let mut b = building(vec![room("all", 0.0, 8.0, 20.0)]);
        b.wall[1] = match foam {
            Some(l) => b.wall[1].clone().limited(l),
            None => Layer::new(0.08, Material::constant(1.7, 2.0e6)), // concrete instead of foam
        };
        let m = Model::build(&b, Preset::Draft).unwrap();
        run(&m, &weather(), &options())
            .unwrap()
            .annual(0, Kind::Wall)
    };
    let (none, partial, full) = (
        wall_loss(None),
        wall_loss(Some(Limit::Depth(1.0))),
        wall_loss(Some(Limit::Full)),
    );
    println!("wall: none {none:.0}, top 1 m {partial:.0}, full {full:.0} kWh");
    assert!(none < partial && partial < full && full < 0.0);
}
