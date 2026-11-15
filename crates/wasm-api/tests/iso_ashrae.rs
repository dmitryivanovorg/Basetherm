//! Solver versus hand-calculation methods, Surgut (Russia) climate. Slow: every case is a full run.
//!
//! Run: `CASES=all cargo test -p basetherm-wasm-api --release --features parallel --test iso_ashrae -- --ignored --nocapture`
//! (`CASES=name1,name2` runs a subset). Results and discussion: `validation/iso-ashrae/README.md`.
//!
//! Reference methods (all steady state; they know nothing about snow, freezing or the surface balance):
//! - **ISO 13370** (heat transfer via the ground), annual mean part: `B' = A / (0.5 P)`, floor and basement
//!   wall U-values, `Phi = (A U_bf + z P U_bw) (theta_i - theta_e,mean)`. The periodic part is not
//!   compared (its formulas could not be checked against the standard text).
//! - **ASHRAE-style path-length method** (Latta and Boileau concept used by the ASHRAE Handbook; implemented
//!   here from the concept, *not* from the Handbook tables): heat leaves a wall point at depth x or a floor
//!   point at offset y along a quarter circle of length `pi/2 * r` through soil of conductivity k, `r` the
//!   distance from the grade line at the outer wall; the design ground temperature is the annual mean air
//!   temperature minus its amplitude (ASHRAE winter design ground temperature).
use basetherm_wasm_api::run_project;
use std::f64::consts::PI;

const EPW: &str =
    include_str!("../../../refs/epw/surgut/RUS_KHM_Surgut.AP.238490_TMYx.2011-2025.epw");

const K_SOIL: f64 = 1.6;
const H_WALL: f64 = 8.0;
const H_FLOOR: f64 = 6.0;
const T_WALL: f64 = 0.25;
const T_FLOOR: f64 = 0.15;
const K_CONC: f64 = 1.4;
const DEPTH: f64 = 2.4;
const LX: f64 = 12.0;
const LY: f64 = 8.0;
const INS: f64 = 0.1;
const K_INS: f64 = 0.035;

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    slab: bool,
    insulated: bool,
    freezing: bool,
    /// Room temperatures: one room, or two rooms split at x = LX/2.
    temps: &'static [f64],
}

const CASES: &[Case] = &[
    Case {
        name: "B-plain-thaw",
        slab: false,
        insulated: false,
        freezing: false,
        temps: &[20.0],
    },
    Case {
        name: "B-plain-freeze",
        slab: false,
        insulated: false,
        freezing: true,
        temps: &[20.0],
    },
    Case {
        name: "B-ins-thaw",
        slab: false,
        insulated: true,
        freezing: false,
        temps: &[20.0],
    },
    Case {
        name: "B-ins-freeze",
        slab: false,
        insulated: true,
        freezing: true,
        temps: &[20.0],
    },
    Case {
        name: "S-plain-thaw",
        slab: true,
        insulated: false,
        freezing: false,
        temps: &[20.0],
    },
    Case {
        name: "S-ins-freeze",
        slab: true,
        insulated: true,
        freezing: true,
        temps: &[20.0],
    },
    Case {
        name: "B2-plain-same",
        slab: false,
        insulated: false,
        freezing: false,
        temps: &[20.0, 20.0],
    },
    Case {
        name: "B2-plain-diff",
        slab: false,
        insulated: false,
        freezing: false,
        temps: &[20.0, 12.0],
    },
    Case {
        name: "B2-ins-diff",
        slab: false,
        insulated: true,
        freezing: false,
        temps: &[20.0, 12.0],
    },
    Case {
        name: "B2-plain-diff-freeze",
        slab: false,
        insulated: false,
        freezing: true,
        temps: &[20.0, 12.0],
    },
];

fn project(c: &Case) -> String {
    let soil = if c.freezing {
        // Sandy loam at about 80 % saturation: porosity 0.4, 0.32 of ice: latent 0.32 * 917 * 334e3 ~ 98 MJ/m3.
        r#"{"k":1.6,"cv":2.8e6,"kFrozen":2.0,"cvFrozen":2.1e6,"latent":9.8e7,"tFreeze":0}"#
    } else {
        r#"{"k":1.6,"cv":2.8e6}"#
    };
    let conc = |t: f64| format!(r#"{{"thickness":{t},"material":{{"k":{K_CONC},"cv":2.0e6}}}}"#);
    let ins = format!(r#"{{"thickness":{INS},"material":{{"k":{K_INS},"cv":3.0e4}}}}"#);
    // Layers run from the room outward: the board is the outer wall layer and the lower floor layer.
    let wall = if c.slab {
        String::new()
    } else if c.insulated {
        format!("{},{ins}", conc(T_WALL))
    } else {
        conc(T_WALL)
    };
    let floor = if c.insulated {
        format!("{},{ins}", conc(T_FLOOR))
    } else {
        conc(T_FLOOR)
    };
    let rooms: Vec<String> = if c.temps.len() == 1 {
        vec![format!(
            r#"{{"name":"A","poly":[[0,0],[{LX},0],[{LX},{LY}],[0,{LY}]],"temperature":{{"constant":{}}}}}"#,
            c.temps[0]
        )]
    } else {
        let h = LX / 2.0;
        vec![
            format!(
                r#"{{"name":"A","poly":[[0,0],[{h},0],[{h},{LY}],[0,{LY}]],"temperature":{{"constant":{}}}}}"#,
                c.temps[0]
            ),
            format!(
                r#"{{"name":"B","poly":[[{h},0],[{LX},0],[{LX},{LY}],[{h},{LY}]],"temperature":{{"constant":{}}}}}"#,
                c.temps[1]
            ),
        ]
    };
    let rooms = rooms.join(",");
    let (levels, ground) = if c.slab {
        (String::new(), rooms)
    } else {
        (
            format!(r#"{{"name":"Basement","height":{DEPTH},"rooms":[{rooms}]}}"#),
            String::new(),
        )
    };
    format!(
        r#"{{"version":2,"preset":"draft",
  "surface":{{"albedo":0.25,"emissivity":0.9,"convection":"waltonRough"}},
  "options":{{"coarseYears":20,"hourlyYears":8,"driftTol":0.001}},
  "building":{{"lx":{LX},"ly":{LY},"levels":[{levels}],"groundFloorRooms":[{ground}],
    "wall":[{wall}],"floor":[{floor}],"hWall":{H_WALL},"hFloor":{H_FLOOR},
    "soil":[{{"top":0,"material":{soil}}}],
    "domain":{{"margin":15,"depthBelowFloor":15}}}}}}"#
    )
}

struct Climate {
    monthly: [f64; 12],
    mean: f64,
    amp: f64,
}

fn climate() -> Climate {
    let (mut sum, mut n) = ([0.0; 12], [0.0; 12]);
    for l in EPW.lines().skip(8) {
        let f: Vec<&str> = l.split(',').collect();
        if f.len() < 7 {
            continue;
        }
        let m: usize = f[1].parse().unwrap();
        sum[m - 1] += f[6].parse::<f64>().unwrap();
        n[m - 1] += 1.0;
    }
    let monthly: [f64; 12] = std::array::from_fn(|i| sum[i] / n[i]);
    let mean = monthly.iter().sum::<f64>() / 12.0;
    let amp = (monthly.iter().cloned().fold(f64::MIN, f64::max)
        - monthly.iter().cloned().fold(f64::MAX, f64::min))
        / 2.0;
    Climate { monthly, mean, amp }
}

/// Resistance of the construction from the room air to the ground side, m2 K/W.
fn r_wall(c: &Case) -> f64 {
    1.0 / H_WALL + T_WALL / K_CONC + if c.insulated { INS / K_INS } else { 0.0 }
}
fn r_floor(c: &Case) -> f64 {
    1.0 / H_FLOOR + T_FLOOR / K_CONC + if c.insulated { INS / K_INS } else { 0.0 }
}

/// ISO 13370 U-values `(floor, wall)` for a building of floor area `a` and exposed perimeter `p`.
fn iso_u(c: &Case, a: f64, p: f64, w: f64) -> (f64, f64) {
    let lam = K_SOIL;
    let b = a / (0.5 * p);
    let z = if c.slab { 0.0 } else { DEPTH };
    let dt = w + lam * r_floor(c);
    let dd = dt + 0.5 * z;
    let ubf = if dd < b {
        2.0 * lam / (PI * b + dd) * (PI * b / dd + 1.0).ln()
    } else {
        lam / (0.457 * b + dd)
    };
    let ubw = if c.slab {
        0.0
    } else {
        let dw = lam * r_wall(c);
        2.0 * lam / (PI * z) * (1.0 + 0.5 * dw / (dw + z)) * (z / dw + 1.0).ln()
    };
    (ubf, ubw)
}

/// Path-length method `(floor, wall)` average U-values, W/(m2 K); `width` is the shortest building width.
fn path_u(c: &Case, width: f64) -> (f64, f64) {
    let z = if c.slab { 0.0 } else { DEPTH };
    let n = 20000;
    let wall = if c.slab {
        0.0
    } else {
        (0..n)
            .map(|i| {
                let x = (i as f64 + 0.5) / n as f64 * z;
                1.0 / (r_wall(c) + PI * x / 2.0 / K_SOIL)
            })
            .sum::<f64>()
            / n as f64
    };
    let half = width / 2.0;
    let floor = (0..n)
        .map(|i| {
            let y = (i as f64 + 0.5) / n as f64 * half;
            1.0 / (r_floor(c) + PI / 2.0 * (z * z + y * y).sqrt() / K_SOIL)
        })
        .sum::<f64>()
        / n as f64;
    (floor, wall)
}

/// Floor area and exterior wall length of each room (two rooms: split at x = LX/2; the partition is not exterior wall).
fn room_geometry(rooms: usize) -> (f64, f64) {
    if rooms == 1 {
        (LX * LY, 2.0 * (LX + LY))
    } else {
        (LX / 2.0 * LY, LX + LY)
    }
}

#[test]
#[ignore = "full runs, minutes to an hour"]
fn solver_versus_iso_13370_and_ashrae_path_method() {
    let clim = climate();
    let want = std::env::var("CASES").unwrap_or_else(|_| "all".into());
    println!(
        "Surgut: mean air {:.2} C, amplitude of monthly means {:.2} C, monthly {:?}",
        clim.mean,
        clim.amp,
        clim.monthly.map(|t| (t * 10.0).round() / 10.0)
    );
    println!(
        "{:<22}{:>9}{:>9}{:>9}{:>8}{:>10}{:>10}{:>9}{:>10}",
        "case",
        "solver W",
        "ISO W",
        "path W",
        "s/ISO",
        "cold-mo W",
        "design W",
        "s/des",
        "peak hr W"
    );
    for c in CASES
        .iter()
        .filter(|c| want == "all" || want.split(',').any(|n| n == c.name))
    {
        let t0 = std::time::Instant::now();
        let mut cb = |_: &str, _: usize, _: f64| {};
        let r = run_project(&project(c), EPW, &mut cb).unwrap();
        let hours = r.hours();
        let rooms = c.temps.len();
        // External dimensions for ISO (wall thickness added all round).
        let w = T_WALL;
        let (ea, ep) = ((LX + 2.0 * w) * (LY + 2.0 * w), 2.0 * (LX + LY + 4.0 * w));
        let (ubf, ubw) = iso_u(c, ea, ep, w);
        let (pf, pw) = path_u(c, LY);
        let z = if c.slab { 0.0 } else { DEPTH };
        // Per-room hand calculations: area and exterior wall length of each room.
        let (mut s_iso, mut s_path, mut s_design) = (0.0, 0.0, 0.0);
        for k in 0..rooms {
            let (area, per) = room_geometry(rooms);
            let ti = c.temps[k];
            s_iso += (area * ubf + z * per * ubw) * (ti - clim.mean);
            s_path += (area * pf + z * per * pw) * (ti - clim.mean);
            s_design += (area * pf + z * per * pw) * (ti - (clim.mean - clim.amp));
        }
        // Solver: annual mean flow and the coldest monthly mean (loss negative -> positive W here).
        let hour_flow = |h: usize| -r.scope_flow(basetherm_basement::Scope::Basement, None, h);
        let mean: f64 = (0..hours).map(hour_flow).sum::<f64>() / hours as f64;
        let mut month_sum = [0.0; 12];
        let mut month_n = [0.0; 12];
        for h in 0..hours {
            let m = basetherm_basement::Date::from_hour(h).month as usize - 1;
            month_sum[m] += hour_flow(h);
            month_n[m] += 1.0;
        }
        let cold = (0..12)
            .map(|m| month_sum[m] / month_n[m])
            .fold(f64::MIN, f64::max);
        let peak = (0..hours).map(hour_flow).fold(f64::MIN, f64::max);
        println!(
            "{:<22}{:>9.0}{:>9.0}{:>9.0}{:>8.2}{:>10.0}{:>10.0}{:>9.2}{:>10.0}   [{} + {} years, drift {:.3}%, {:.0} s]",
            c.name, mean, s_iso, s_path, mean / s_iso, cold, s_design, cold / s_design, peak,
            r.coarse_years, r.hourly_years, 100.0 * r.drift, t0.elapsed().as_secs_f64()
        );
        if rooms == 2 {
            for k in 0..2 {
                let rm = -r.annual_room(k) * 1000.0 / hours as f64;
                let (area, per) = room_geometry(2);
                let iso = (area * ubf + z * per * ubw) * (c.temps[k] - clim.mean);
                println!(
                    "    room {} ({:.0} C): solver {:.0} W, ISO per room {:.0} W, ratio {:.2}",
                    k,
                    c.temps[k],
                    rm,
                    iso,
                    rm / iso
                );
            }
        }
        if !c.slab && rooms == 1 {
            let f: f64 = -(0..rooms)
                .map(|k| r.annual(k, basetherm_basement::Kind::Floor))
                .sum::<f64>()
                * 1000.0
                / hours as f64;
            let wl: f64 = -(0..rooms)
                .map(|k| r.annual(k, basetherm_basement::Kind::Wall))
                .sum::<f64>()
                * 1000.0
                / hours as f64;
            println!(
                "    split: floor {:.0} W (ISO {:.0}), walls {:.0} W (ISO {:.0})",
                f,
                ubf * LX * LY * (c.temps[0] - clim.mean),
                wl,
                ubw * z * 2.0 * (LX + LY) * (c.temps[0] - clim.mean)
            );
        }
    }
}
