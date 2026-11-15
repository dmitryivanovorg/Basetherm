//! Timing of a realistic basement run: `basement-timing [draft|standard|fine] [steps] [--freezing] [--sym]`.
//!
//! Prints the mesh size and the time of each phase. The checksum is the sum of all temperatures
//! after the timed steps; runs that are supposed to be identical must print identical checksums.

use basetherm_basement::bench::time_steps;
use basetherm_basement::*;
use basetherm_materials::Material;
use basetherm_surface::{Convection, GroundSurface, Weather};
use std::f64::consts::PI;

fn weather() -> Vec<Weather> {
    let w = 2.0 * PI / 8760.0;
    (0..8760)
        .map(|i| Weather {
            t_air: 8.0
                + 11.0 * (w * (i as f64 + 0.5) - 3.0).cos()
                + 3.0 * (2.0 * PI * (i % 24) as f64 / 24.0 - 2.5).cos(),
            wind: 3.0 + (i % 17) as f64 * 0.2,
            ghi: if (6..18).contains(&(i % 24)) {
                300.0
            } else {
                0.0
            },
            ir_sky: 300.0,
        })
        .collect()
}

fn building(freezing: bool, symmetric: bool) -> Building {
    let (latent, t_freeze) = if freezing { (6.0e7, 0.0) } else { (0.0, 0.0) };
    let soil = |k_t: f64, k_f: f64, cv_t: f64| Material {
        k_thawed: k_t,
        k_frozen: k_f,
        cv_thawed: cv_t,
        cv_frozen: cv_t * 0.75,
        latent,
        t_freeze,
        half_width: 0.5,
    };
    let room =
        |name: &str, x0, x1, y0, y1, t| Room::rect(name, x0, x1, y0, y1, Schedule::Constant(t));
    // Symmetric: two mirror rooms and a centred one. Asymmetric: three different rooms.
    let rooms = if symmetric {
        vec![
            room("west", 0.0, 4.5, 0.0, 9.0, 20.0),
            room("mid", 4.5, 7.5, 0.0, 9.0, 16.0),
            room("east", 7.5, 12.0, 0.0, 9.0, 20.0),
        ]
    } else {
        vec![
            room("living", 0.0, 7.0, 0.0, 9.0, 21.0),
            room("bedroom", 7.0, 12.0, 0.0, 5.0, 19.0),
            room("utility", 7.0, 12.0, 5.0, 9.0, 16.0),
        ]
    };
    Building {
        lx: 12.0,
        ly: 9.0,
        levels: vec![Level {
            name: "basement".into(),
            height: 2.4,
            rooms,
        }],
        wall: vec![
            Layer::new(0.25, Material::constant(1.7, 2.0e6)),
            Layer::new(0.10, Material::constant(0.035, 4.0e4)),
        ],
        floor: vec![
            Layer::new(0.10, Material::constant(1.7, 2.0e6)),
            Layer::new(0.05, Material::constant(0.035, 4.0e4)),
        ],
        ground_floor_rooms: vec![],
        h_wall: 8.0,
        h_floor: 6.0,
        soil: vec![
            SoilLayer {
                top: 0.0,
                material: soil(1.2, 1.7, 2.4e6),
            },
            SoilLayer {
                top: 1.5,
                material: soil(1.8, 2.3, 2.6e6),
            },
        ],
        footprint: None,
        insulation: None,
        domain: Domain::default(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let preset = match args
        .iter()
        .find(|a| ["draft", "standard", "fine"].contains(&a.as_str()))
        .map(String::as_str)
    {
        Some("draft") => Preset::Draft,
        Some("fine") => Preset::Fine,
        _ => Preset::Standard,
    };
    let steps = args
        .iter()
        .find_map(|a| a.parse::<usize>().ok())
        .unwrap_or(24);
    let (freezing, symmetric) = (flag("--freezing"), flag("--sym"));

    let clock = std::time::Instant::now();
    let mode = if symmetric {
        SymmetryMode::Auto
    } else {
        SymmetryMode::Off
    };
    let model = Model::build_with(&building(freezing, symmetric), preset, mode).unwrap();
    let build_s = clock.elapsed().as_secs_f64();
    let opts = RunOptions::new(GroundSurface::new(0.25, 0.9, Convection::WALTON_ROUGH).unwrap());
    let t = time_steps(&model, &weather(), &opts, steps).unwrap();

    let step_s = t.step_ms / 1000.0;
    println!("preset {preset:?}, freezing {freezing}, symmetric {symmetric}");
    println!(
        "grid {:?} = {} cells ({:.0} MB for temperature+properties)",
        t.dims,
        t.cells,
        t.cells as f64 * 8.0 * 8.0 / 1e6
    );
    println!(
        "model build {build_s:.2} s, far-field {:.2} s, periodic initial state {:.2} s",
        t.farfield_s, t.init_s
    );
    println!(
        "hourly step {:.1} ms (mean of {} steps): forcing+boundaries {:.2} ms, stepper {:.1} ms, surface state {:.2} ms",
        t.step_ms, t.steps, t.phases_ms[0], t.phases_ms[1], t.phases_ms[2]
    );
    println!(
        "estimates: hourly year {:.1} min, coarse daily year {:.1} min; a full run (20 coarse + 2 hourly years) {:.0} min",
        8760.0 * step_s / 60.0,
        365.0 * step_s / 60.0,
        (20.0 * 365.0 + 2.0 * 8760.0) * step_s / 60.0
    );
    println!("threads {}, checksum {:.9}", rayon_threads(), t.checksum);
}

fn rayon_threads() -> String {
    std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "1 (no threading yet)".to_string())
}
