use basetherm_wasm_api::{epw_summary, mesh_info, run_project};

const EPW: &str = include_str!("../../../refs/epw/USA_CO_Golden-NREL.724666_TMY3.epw");

const PROJECT: &str = r#"{
  "version": 2,
  "preset": "draft",
  "surface": {"albedo": 0.25, "emissivity": 0.9, "convection": "waltonRough"},
  "options": {"coarseYears": 3, "hourlyYears": 1},
  "building": {
    "lx": 6, "ly": 5,
    "levels": [{"name": "Basement", "height": 2.4, "rooms": [
      {"name": "Main", "poly": [[0,0],[6,0],[6,5],[0,5]], "temperature": {"constant": 20}}]}],
    "wall": [{"thickness": 0.2, "material": {"k": 1.4, "cv": 2000000}}],
    "floor": [{"thickness": 0.15, "material": {"k": 1.4, "cv": 2000000}}],
    "hWall": 8.0, "hFloor": 6.0,
    "soil": [{"top": 0, "material": {"k": 1.5, "cv": 2500000}}],
    "domain": {"margin": 8, "depthBelowFloor": 10}
  }
}"#;

#[test]
fn epw_is_described() {
    let s: serde_json::Value = serde_json::from_str(&epw_summary(EPW).unwrap()).unwrap();
    assert_eq!(s["country"], "USA");
    assert!(s["meanAirTemperature"].as_f64().unwrap() > 5.0);
}

#[test]
fn bad_input_is_a_message_not_a_panic() {
    let mut none = |_: &str, _: usize, _: f64| {};
    assert!(run_project("{", EPW, &mut none)
        .unwrap_err()
        .contains("project file"));
    let v2 = PROJECT.replace("\"version\": 2", "\"version\": 3");
    assert!(run_project(&v2, EPW, &mut none)
        .unwrap_err()
        .contains("version"));
    assert!(run_project(PROJECT, "junk", &mut none)
        .unwrap_err()
        .contains("climate file"));
    let bad = PROJECT.replace("[6,0],[6,5]", "[60,0],[60,5]");
    assert!(run_project(&bad, EPW, &mut none)
        .unwrap_err()
        .contains("room"));
}

#[test]
#[ignore = "full small run, minutes"]
fn small_basement_loses_heat_and_reports_progress() {
    let mut stages = Vec::new();
    let mut cb = |s: &str, _y: usize, _f: f64| {
        if stages.last().map(String::as_str) != Some(s) {
            stages.push(s.to_string());
        }
    };
    let r = run_project(PROJECT, EPW, &mut cb).unwrap();
    assert_eq!(stages, ["setup", "warm-up", "hourly"]);
    println!("annual total {:.10} kWh", r.annual_total());
    assert!(r.annual_total() < 0.0);
}

#[test]
#[ignore = "full small run, minutes"]
fn no_warm_up_adiabatic_edges_history_and_flux_sections() {
    let p = PROJECT.replace(
        "\"options\": {\"coarseYears\": 3, \"hourlyYears\": 1}",
        "\"options\": {\"hourlyYears\": 2, \"warmUp\": false, \"conditionEdges\": false}",
    );
    assert_ne!(p, PROJECT);
    let mut stages = Vec::new();
    let mut cb = |s: &str, _y: usize, _f: f64| {
        if stages.last().map(String::as_str) != Some(s) {
            stages.push(s.to_string());
        }
    };
    let r = run_project(&p, EPW, &mut cb).unwrap();
    assert_eq!(stages, ["setup", "hourly"]);
    assert_eq!(r.coarse_years, 0);
    assert_eq!(r.history.len(), 2);
    assert!(r.history[0].drift.is_none() && r.history[1].drift.is_some());
    assert!(r
        .history
        .iter()
        .all(|y| y.hourly && y.kwh < 0.0 && y.peak.is_some_and(|p| p < 0.0)));
    // Flux sections: solid cells finite, void NaN; the ground under the basement loses heat downward.
    let f = &r.sections.flux_longitudinal;
    assert_eq!(f.len(), 12);
    let n = r.sections.x.len() - 1;
    let nz = r.sections.z.len() - 1;
    assert_eq!(f[0].len(), n * nz * 2);
    assert!(f[0].iter().any(|v| v.is_nan()) && f[0].iter().any(|v| v.is_finite()));
    println!("history {:?}", r.history);
}

#[test]
fn mesh_preview_reports_cells_symmetry_and_errors() {
    let m: serde_json::Value = serde_json::from_str(&mesh_info(PROJECT).unwrap()).unwrap();
    let d: Vec<usize> = m["dims"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(m["cells"].as_u64().unwrap() as usize, d[0] * d[1] * d[2]);
    assert_eq!(m["x"].as_array().unwrap().len(), d[0] + 1);
    // One centred room is mirror-symmetric both ways, so the grid starts at the centre planes.
    assert_eq!(m["symmetryX"], true);
    assert_eq!(m["symmetryY"], true);
    assert_eq!(m["x"][0].as_f64().unwrap(), 3.0);
    assert_eq!(m["overCap"], false);
    let bad = PROJECT.replace("[6,0],[6,5]", "[60,0],[60,5]");
    assert!(mesh_info(&bad).unwrap_err().contains("room"));
}

#[test]
fn l_footprints_limited_layers_and_soil_classes_are_read() {
    let l = PROJECT
        .replace(
            "\"lx\": 6",
            "\"footprint\": [[0,0],[6,0],[6,2],[3,2],[3,5],[0,5]], \"lx\": 6",
        )
        .replace(
            "\"poly\": [[0,0],[6,0],[6,5],[0,5]]",
            "\"poly\": [[0,0],[6,0],[6,2],[3,2],[3,5],[0,5]]",
        )
        .replace(
            "\"wall\": [{\"thickness\": 0.2, \"material\": {\"k\": 1.4, \"cv\": 2000000}}]",
            "\"wall\": [{\"thickness\": 0.1, \"material\": {\"k\": 0.04, \"cv\": 40000}, \"limit\": {\"kind\": \"depth\", \"value\": 1.0}}, {\"thickness\": 0.2, \"material\": {\"k\": 1.4, \"cv\": 2000000}}]",
        )
        .replace(
            "\"soil\": [{\"top\": 0, \"material\": {\"k\": 1.5, \"cv\": 2500000}}]",
            "\"soil\": [{\"top\": 0, \"soil\": {\"preset\": \"silt\", \"saturation\": 0.8, \"freezing\": true}}]",
        );
    assert_ne!(l, PROJECT);
    let m: serde_json::Value = serde_json::from_str(&mesh_info(&l).unwrap()).unwrap();
    assert_eq!(m["symmetryX"], false);
    assert!(m["x"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_f64() == Some(3.0)));
    let both = l.replace(
        "\"soil\": {\"preset\"",
        "\"material\": {\"k\": 1, \"cv\": 1e6}, \"soil\": {\"preset\"",
    );
    assert!(
        mesh_info(&both).is_err()
            || run_project(&both, EPW, &mut |_: &str, _: usize, _: f64| {}).is_err()
    );
}
