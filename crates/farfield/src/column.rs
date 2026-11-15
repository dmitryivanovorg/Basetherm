use crate::error::Error;
use basetherm_grid::{Axis, Grid};
use basetherm_materials::Material;

/// Condition at the bottom of the column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bottom {
    /// No heat flow (no geothermal flux). The default.
    Adiabatic,
    /// Fixed temperature, C.
    Fixed(f64),
}

/// The vertical column: `z` runs downward from the surface (z = 0), one conductivity and heat
/// capacity per cell. Use the 3D domain's z-axis extended to the column depth so cells coincide.
#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub z: Axis,
    /// Thawed conductivity and heat capacity per cell (used as is when `freezing` is `None`).
    pub k: Vec<f64>,
    pub cv: Vec<f64>,
    pub bottom: Bottom,
    /// Soils with a freezing curve; when present the hourly run uses the enthalpy stepper.
    pub freezing: Option<Freezing>,
}

/// Soil table and per-cell material index for a column that can freeze.
#[derive(Debug, Clone)]
pub struct Freezing {
    pub materials: Vec<Material>,
    pub cell_material: Vec<u16>,
}

impl ColumnSpec {
    /// A column of soils that can freeze; `k` and `cv` are filled with the thawed values.
    pub fn with_freezing(
        z: Axis,
        materials: Vec<Material>,
        cell_material: Vec<u16>,
        bottom: Bottom,
    ) -> Result<Self, Error> {
        if cell_material.len() != z.len()
            || cell_material
                .iter()
                .any(|&m| usize::from(m) >= materials.len())
        {
            return Err(Error::Input(
                "cell_material must index the materials, one per cell",
            ));
        }
        let k = cell_material
            .iter()
            .map(|&m| materials[usize::from(m)].k_thawed)
            .collect();
        let cv = cell_material
            .iter()
            .map(|&m| materials[usize::from(m)].cv_thawed)
            .collect();
        Ok(Self {
            z,
            k,
            cv,
            bottom,
            freezing: Some(Freezing {
                materials,
                cell_material,
            }),
        })
    }

    pub fn validate(&self) -> Result<(), Error> {
        let n = self.z.len();
        if let Some(f) = &self.freezing {
            if f.cell_material.len() != n
                || f.cell_material
                    .iter()
                    .any(|&m| usize::from(m) >= f.materials.len())
            {
                return Err(Error::Input(
                    "freezing: cell_material must index the materials, one per cell",
                ));
            }
        }
        if self.k.len() != n || self.cv.len() != n {
            return Err(Error::Input("k and cv need one value per cell"));
        }
        if self.z.start() != 0.0 {
            return Err(Error::Input("the column must start at the surface, z = 0"));
        }
        Ok(())
    }

    /// The column as a grid with x and y collapsed to one cell of unit width.
    pub fn grid(&self) -> Grid {
        let unit = || Axis::collapsed(1.0).expect("unit axis");
        Grid::new(unit(), unit(), self.z.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Most root-search evaluations of the deep temperature (adiabatic bottom only).
    pub max_iterations: usize,
    /// Most hourly years per evaluation while spinning up.
    pub max_years: usize,
    /// Converged when the mean net surface flux and the annual storage are below this, W/m2.
    pub flux_tol: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            max_years: 10,
            flux_tol: 0.001,
        }
    }
}
