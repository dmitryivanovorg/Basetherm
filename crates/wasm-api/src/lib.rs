//! wasm-bindgen entry points, typed interface for the Web Worker. Only crate that knows about WASM.
//!
//! Everything crosses the boundary as JSON text or flat `Float64Array`s. The functions that do the
//! work take and return plain Rust types (`run_project`, `epw_summary`) so they run in native tests.

mod mesh;
mod output;
mod project;
mod soil;

use basetherm_basement::{Model, Progress, Results, Stage};
use basetherm_surface::Weather;
pub use mesh::mesh_info;
use project::Project;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EpwSummary {
    city: String,
    country: String,
    latitude: f64,
    longitude: f64,
    mean_air_temperature: f64,
}

fn weather(text: &str) -> Result<(basetherm_climate_epw::Epw, Vec<Weather>), String> {
    let epw = basetherm_climate_epw::parse(text).map_err(|e| format!("climate file: {e}"))?;
    let w = (0..basetherm_climate_epw::HOURS_PER_YEAR)
        .map(|h| epw.weather(h))
        .collect();
    Ok((epw, w))
}

/// Parse an EPW file and describe it, as JSON.
pub fn epw_summary(text: &str) -> Result<String, String> {
    let (epw, _) = weather(text)?;
    let l = &epw.location;
    serde_json::to_string(&EpwSummary {
        city: l.city.clone(),
        country: l.country.clone(),
        latitude: l.latitude,
        longitude: l.longitude,
        mean_air_temperature: epw.annual_means().t_air,
    })
    .map_err(|e| e.to_string())
}

/// Run a project (JSON) against EPW text. `progress` gets `(stage, year, fraction)`.
pub fn run_project(
    project_json: &str,
    epw_text: &str,
    progress: &mut dyn FnMut(&str, usize, f64),
) -> Result<Results, String> {
    let p = Project::parse(project_json)?;
    let building = p.building.to_building()?;
    let model = Model::build(&building, p.preset()).map_err(|e| e.to_string())?;
    let (_, w) = weather(epw_text)?;
    let opts = p.run_options()?;
    let mut report = |pr: Progress| match pr.stage {
        Stage::Setup => progress("setup", 0, pr.fraction),
        Stage::Coarse(y) => progress("warm-up", y, pr.fraction),
        Stage::Hourly(y) => progress("hourly", y, pr.fraction),
    };
    basetherm_basement::run_with_progress(&model, &w, &opts, &mut report).map_err(|e| e.to_string())
}

/// A finished run: summary as JSON, hourly flows as an array.
#[wasm_bindgen]
pub struct RunResult(Results);

#[wasm_bindgen]
impl RunResult {
    /// Annual energy and peaks per room and for the basement, as JSON.
    pub fn summary(&self) -> Result<String, JsError> {
        output::summary_json(&self.0).map_err(|e| JsError::new(&e))
    }

    /// Hourly flows in W, `[hour][room][floor, wall]` flattened. Negative is loss.
    pub fn hourly(&self) -> Vec<f64> {
        output::hourly(&self.0)
    }

    /// Cell faces of the section planes (x, y, z, m), as JSON.
    #[wasm_bindgen(js_name = sectionAxes)]
    pub fn section_axes(&self) -> Result<String, JsError> {
        output::section_axes_json(&self.0).map_err(|e| JsError::new(&e))
    }

    /// Plane y = ly/2 (along x): 12 monthly means in C, `[month][x cell][depth cell]`, NaN in the void.
    pub fn longitudinal(&self) -> Vec<f64> {
        output::section(&self.0.sections.longitudinal)
    }

    /// Plane x = lx/2 (along y): 12 monthly means in C, `[month][y cell][depth cell]`, NaN in the void.
    pub fn perpendicular(&self) -> Vec<f64> {
        output::section(&self.0.sections.perpendicular)
    }

    /// Monthly mean heat flux on plane y = ly/2 in W/m2: `[month][x cell][depth cell][along x, down]`.
    #[wasm_bindgen(js_name = fluxLongitudinal)]
    pub fn flux_longitudinal(&self) -> Vec<f64> {
        output::section(&self.0.sections.flux_longitudinal)
    }

    /// Monthly mean heat flux on plane x = lx/2 in W/m2: `[month][y cell][depth cell][along y, down]`.
    #[wasm_bindgen(js_name = fluxPerpendicular)]
    pub fn flux_perpendicular(&self) -> Vec<f64> {
        output::section(&self.0.sections.flux_perpendicular)
    }
}

#[wasm_bindgen(js_name = epwSummary)]
pub fn epw_summary_js(text: &str) -> Result<String, JsError> {
    epw_summary(text).map_err(|e| JsError::new(&e))
}

/// `on_progress(stage: string, year: number, fraction: number)` is called about once per day.
#[wasm_bindgen(js_name = runProject)]
pub fn run_project_js(
    project_json: &str,
    epw_text: &str,
    on_progress: &js_sys::Function,
) -> Result<RunResult, JsError> {
    let mut cb = |stage: &str, year: usize, f: f64| {
        let _ = on_progress.call3(
            &JsValue::NULL,
            &JsValue::from_str(stage),
            &JsValue::from_f64(year as f64),
            &JsValue::from_f64(f),
        );
    };
    run_project(project_json, epw_text, &mut cb)
        .map(RunResult)
        .map_err(|e| JsError::new(&e))
}

/// The mesh a project would use (dimensions, faces, symmetry), as JSON. Meshes only; no solve.
#[wasm_bindgen(js_name = meshInfo)]
pub fn mesh_info_js(project_json: &str) -> Result<String, JsError> {
    mesh_info(project_json).map_err(|e| JsError::new(&e))
}

/// The soil classes of the table, as JSON.
#[wasm_bindgen(js_name = soilPresets)]
pub fn soil_presets_js() -> Result<String, JsError> {
    soil::presets_json().map_err(|e| JsError::new(&e))
}

/// Properties of a soil class at a degree of saturation (`{preset, saturation, freezing}`), as JSON.
#[wasm_bindgen(js_name = soilProperties)]
pub fn soil_properties_js(spec_json: &str) -> Result<String, JsError> {
    soil::properties_json(spec_json).map_err(|e| JsError::new(&e))
}

/// The schema version this build reads and writes.
#[wasm_bindgen(js_name = schemaVersion)]
pub fn schema_version() -> u32 {
    project::SCHEMA_VERSION
}

// Thread pool entry point for JS: `await initThreadPool(n)` before the first run.
#[cfg(all(target_arch = "wasm32", feature = "parallel"))]
pub use wasm_bindgen_rayon::init_thread_pool;
