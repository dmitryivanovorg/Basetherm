//! Builds the solver inputs for a case: a quarter-domain model using the two mirror planes
//! (x = 0 and y = 0 adiabatic), the slab and exterior ground on the top face, and a fixed deep
//! temperature at the bottom. Far-field vertical planes are adiabatic.

use super::case::{Case, Ring, Surface, C_P, RHO, RING_W, T_DEEP, T_IN};
use basetherm_bc::{Bc, Boundaries, Side};
use basetherm_grid::{Axis, GradingSpec, Grid};
use basetherm_materials::Material;

/// Mesh grading: finest cells at the surface and at the strip edges, geometric growth outward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mesh {
    pub h_min_xy: f64,
    pub h_min_z: f64,
    pub h_max: f64,
    pub ratio: f64,
}

impl Mesh {
    pub fn standard() -> Self {
        Self {
            h_min_xy: 0.06,
            h_min_z: 0.05,
            h_max: 1.5,
            ratio: 1.2,
        }
    }

    pub fn fine() -> Self {
        Self {
            h_min_xy: 0.03,
            h_min_z: 0.025,
            h_max: 1.0,
            ratio: 1.12,
        }
    }
}

pub struct Model {
    pub grid: Grid,
    pub k: Vec<f64>,
    pub cv: Vec<f64>,
    pub bc: Boundaries,
    /// Face-cell indices (on the top face) belonging to the slab, one quarter.
    pub slab_faces: Vec<usize>,
    /// Face-cell indices of the exterior ground surface (driven by the outdoor temperature).
    pub exterior_faces: Vec<usize>,
}

/// Boundary condition for a surface at temperature `t`.
pub fn surface_bc(s: Surface, t: f64) -> Bc {
    match s {
        Surface::Fixed => Bc::Fixed(t),
        Surface::Film(h) => Bc::Convective { h, t_inf: t },
    }
}

impl Model {
    /// `t_out` is the mean outdoor air (or exterior surface) temperature.
    pub fn build(case: &Case, mesh: &Mesh, t_out: f64) -> Result<Self, basetherm_grid::AxisError> {
        let (hx, hy) = (case.l / 2.0, case.b / 2.0);
        let spec = |h_min| GradingSpec {
            h_min,
            h_max: mesh.h_max,
            ratio: mesh.ratio,
        };
        let axis = |half: f64| {
            Axis::graded_ends(
                0.0,
                half + case.far,
                &[half, half + RING_W],
                spec(mesh.h_min_xy),
                [true, false],
            )
        };
        let x = axis(hx)?;
        let y = axis(hy)?;
        let z = Axis::graded(0.0, case.depth, &[0.0], spec(mesh.h_min_z))?;
        let grid = Grid::new(x, y, z);
        let n = grid.cell_count();
        let m = Material::from_k_rho_c(case.k, RHO, C_P);

        let mut bc = Boundaries::adiabatic(&grid);
        bc.fill(Side::ZHi, Bc::Fixed(T_DEEP));
        let [nx, ny, _] = grid.dims();
        let mut slab_faces = Vec::new();
        let mut exterior_faces = Vec::new();
        for i in 0..nx {
            for j in 0..ny {
                let (xc, yc) = (grid.x.center(i), grid.y.center(j));
                let face = i * ny + j;
                let in_slab = xc < hx && yc < hy;
                let in_ring = xc < hx + RING_W && yc < hy + RING_W && !in_slab;
                let b = if in_slab {
                    slab_faces.push(face);
                    surface_bc(case.interior, T_IN)
                } else if in_ring {
                    match case.ring {
                        Ring::Adiabatic => Bc::Adiabatic,
                        Ring::Ramp => {
                            let d = (xc - hx).max(yc - hy).max(0.0);
                            Bc::Fixed(T_IN + (t_out - T_IN) * d / RING_W)
                        }
                    }
                } else {
                    exterior_faces.push(face);
                    surface_bc(case.exterior, t_out)
                };
                bc.set(Side::ZLo, face, b);
            }
        }
        Ok(Self {
            grid,
            k: vec![m.k_thawed; n],
            cv: vec![m.cv_thawed; n],
            bc,
            slab_faces,
            exterior_faces,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slab_faces_cover_one_quarter_of_the_floor() {
        let case = Case::by_name("GC30b").unwrap();
        let m = Model::build(&case, &Mesh::standard(), 10.0).unwrap();
        let [_, ny, _] = m.grid.dims();
        let area: f64 = m
            .slab_faces
            .iter()
            .map(|&f| m.grid.x.width(f / ny) * m.grid.y.width(f % ny))
            .sum();
        assert!((area - 36.0).abs() < 1e-9, "{area}");
    }

    #[test]
    fn strip_faces_have_the_expected_conditions() {
        let gc10 = Model::build(&Case::by_name("GC10a").unwrap(), &Mesh::standard(), 10.0).unwrap();
        let gc30 = Model::build(&Case::by_name("GC30b").unwrap(), &Mesh::standard(), 10.0).unwrap();
        let count = |m: &Model, f: &dyn Fn(Bc) -> bool| {
            m.bc.side(Side::ZLo).iter().filter(|&&b| f(b)).count()
        };
        assert!(count(&gc30, &|b| b == Bc::Adiabatic) > 0);
        assert_eq!(count(&gc10, &|b| b == Bc::Adiabatic), 0);
        // Ramp values lie strictly between the slab and ground temperatures.
        assert!(
            count(
                &gc10,
                &|b| matches!(b, Bc::Fixed(t) if t > 10.0 + 1e-9 && t < 30.0 - 1e-9)
            ) > 0
        );
    }
}
