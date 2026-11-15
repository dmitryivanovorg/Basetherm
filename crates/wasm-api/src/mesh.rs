//! Mesh preview: the grid a run would use, without classifying cells or solving.

use crate::project::Project;
use basetherm_basement::preview_grid;
use serde::Serialize;

/// Cell count above which the UI warns, and the hard cap (docs/DESIGN.md, Budgets).
pub const WARN_CELLS: usize = 2_000_000;
pub const MAX_CELLS: usize = 4_000_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshInfo {
    dims: [usize; 3],
    cells: usize,
    /// Mirror planes the run would use (the grid then covers a half or quarter of the domain).
    symmetry_x: bool,
    symmetry_y: bool,
    /// Why a plane is not used.
    notes: Vec<String>,
    /// Cell faces along each axis, m (x, y horizontal from the interior corner; z depth below grade).
    x: Vec<f64>,
    y: Vec<f64>,
    z: Vec<f64>,
    /// Smallest and largest cell width per axis, m.
    h_min: [f64; 3],
    h_max: [f64; 3],
    warn: bool,
    over_cap: bool,
}

fn widths(faces: &[f64]) -> (f64, f64) {
    faces
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold((f64::INFINITY, 0.0), |(lo, hi), w| (lo.min(w), hi.max(w)))
}

/// Describe the mesh of a project (JSON), as JSON.
pub fn mesh_info(project_json: &str) -> Result<String, String> {
    let p = Project::parse(project_json)?;
    let building = p.building.to_building()?;
    let (grid, sym, notes) = preview_grid(&building, p.preset()).map_err(|e| e.to_string())?;
    let (x, y, z) = (grid.x.faces(), grid.y.faces(), grid.z.faces());
    let (wx, wy, wz) = (widths(x), widths(y), widths(z));
    let cells = grid.cell_count();
    serde_json::to_string(&MeshInfo {
        dims: grid.dims(),
        cells,
        symmetry_x: sym.x,
        symmetry_y: sym.y,
        notes,
        x: x.to_vec(),
        y: y.to_vec(),
        z: z.to_vec(),
        h_min: [wx.0, wy.0, wz.0],
        h_max: [wx.1, wy.1, wz.1],
        warn: cells > WARN_CELLS,
        over_cap: cells > MAX_CELLS,
    })
    .map_err(|e| e.to_string())
}
