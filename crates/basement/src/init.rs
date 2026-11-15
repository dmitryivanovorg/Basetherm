//! Periodic initial state of the 3D domain: the direct solve of the mean field and the annual
//! harmonic, with the surface balance linearised, the far-field sides and bottom driven by the
//! undisturbed column's mean and harmonic, and rooms at their annual-mean temperature.

use crate::error::Error;
use crate::model::{Model, TopKind};
use basetherm_bc::{Bc, Boundaries, Side};
use basetherm_climate_epw::annual_harmonic;
use basetherm_farfield::{linearised_film, Undisturbed};
use basetherm_mg::C64;
use basetherm_periodic::{solve_with, Amplitudes, Params};
use basetherm_surface::{GroundSurface, Weather};

pub struct InitInput<'a> {
    pub model: &'a Model,
    pub weather: &'a [Weather],
    pub far: &'a Undisturbed,
    pub ground: &'a GroundSurface,
    pub insulation: &'a GroundSurface,
    /// Reference surface temperature for the radiation linearisation, C.
    pub ts_ref: f64,
    pub freezing: bool,
    /// Sides and bottom follow the undisturbed column; off = adiabatic.
    pub condition_edges: bool,
}

fn run_err(e: impl std::fmt::Display) -> Error {
    Error::Run(e.to_string())
}

/// Temperatures of all cells at hour 0 when the periodic solve is off: every column at the
/// undisturbed ground profile at the end of the far-field year (void cells are set to 0).
pub fn undisturbed_state(m: &Model, far: &Undisturbed) -> Vec<f64> {
    let nz = m.grid.dims()[2];
    let last = far.hours() - 1;
    (0..m.grid.cell_count())
        .map(|cell| {
            if m.interfaces.is_void(cell) {
                0.0
            } else {
                far.cell(last, cell % nz)
            }
        })
        .collect()
}

/// Temperatures of all cells at hour 0 (void cells are set to 0).
pub fn initial_state(i: &InitInput) -> Result<Vec<f64>, Error> {
    let m = i.model;
    let [nx, ny, nz] = m.grid.dims();
    let n = m.grid.cell_count();
    let hours = i.weather.len();

    let series = |f: &dyn Fn(usize) -> f64| annual_harmonic(&(0..hours).map(f).collect::<Vec<_>>());
    let far_h: Vec<_> = (0..nz).map(|k| series(&|h| i.far.cell(h, k))).collect();
    let bottom_h = series(&|h| i.far.face(h, nz));
    let (h_ground, harm_ground) = linearised_film(i.ground, i.weather, i.ts_ref);
    let (h_ins, harm_ins) = linearised_film(i.insulation, i.weather, i.ts_ref);
    let room_mean: Vec<f64> = m
        .spec
        .rooms()
        .iter()
        .map(|(_, r)| r.temperature.mean())
        .collect();

    let mut bc = Boundaries::adiabatic(&m.grid);
    let mut amp = Amplitudes::zeros(&m.grid);
    let c = |re: f64, im: f64| C64::new(re, im);
    for (face, kind) in m.top.iter().enumerate() {
        match *kind {
            TopKind::Ground => {
                bc.set(
                    Side::ZLo,
                    face,
                    Bc::Convective {
                        h: h_ground,
                        t_inf: harm_ground.mean,
                    },
                );
                amp.set(Side::ZLo, face, c(harm_ground.amp_re, harm_ground.amp_im));
            }
            TopKind::Insulation => {
                bc.set(
                    Side::ZLo,
                    face,
                    Bc::Convective {
                        h: h_ins,
                        t_inf: harm_ins.mean,
                    },
                );
                amp.set(Side::ZLo, face, c(harm_ins.amp_re, harm_ins.amp_im));
            }
            TopKind::Room(r) => bc.set(
                Side::ZLo,
                face,
                Bc::Convective {
                    h: m.spec.h_floor,
                    t_inf: room_mean[r],
                },
            ),
            TopKind::None | TopKind::Adiabatic => {}
        }
    }
    for (k, h) in far_h.iter().enumerate().filter(|_| i.condition_edges) {
        for j in 0..ny {
            for side in [Side::XLo, Side::XHi] {
                if side == Side::XLo && m.symmetry.x {
                    continue; // the centre plane is adiabatic
                }
                bc.set(side, j * nz + k, Bc::Fixed(h.mean));
                amp.set(side, j * nz + k, c(h.amp_re, h.amp_im));
            }
        }
        for ix in 0..nx {
            for side in [Side::YLo, Side::YHi] {
                if side == Side::YLo && m.symmetry.y {
                    continue;
                }
                bc.set(side, ix * nz + k, Bc::Fixed(h.mean));
                amp.set(side, ix * nz + k, c(h.amp_re, h.amp_im));
            }
        }
    }
    if i.condition_edges {
        bc.fill(Side::ZHi, Bc::Fixed(bottom_h.mean));
        amp.fill(Side::ZHi, c(bottom_h.amp_re, bottom_h.amp_im));
    }

    let mut ifc = m.interfaces.clone();
    for (r, &t) in room_mean.iter().enumerate() {
        ifc.set_temperature(2 * r, t);
        ifc.set_temperature(2 * r + 1, t);
    }

    // Thawed properties first; with freezing, cells whose mean is below freezing get frozen ones.
    let prop = |frozen: &dyn Fn(usize) -> bool| {
        let get = |f: &dyn Fn(&basetherm_materials::Material, bool) -> f64| -> Vec<f64> {
            (0..n)
                .map(|cell| {
                    f(
                        &m.materials[usize::from(m.cell_material[cell])],
                        frozen(cell),
                    )
                })
                .collect()
        };
        (
            get(&|mat, fr| if fr { mat.k_frozen } else { mat.k_thawed }),
            get(&|mat, fr| if fr { mat.cv_frozen } else { mat.cv_thawed }),
        )
    };
    let (k, cv) = prop(&|_| false);
    let mut sol =
        solve_with(&m.grid, &k, &cv, &bc, &amp, Some(&ifc), Params::annual()).map_err(run_err)?;
    if i.freezing {
        let frozen =
            |cell: usize| sol.mean[cell] < m.materials[usize::from(m.cell_material[cell])].t_freeze;
        let (k, cv) = prop(&frozen);
        sol = solve_with(&m.grid, &k, &cv, &bc, &amp, Some(&ifc), Params::annual())
            .map_err(run_err)?;
    }
    Ok((0..n)
        .map(|cell| {
            if m.interfaces.is_void(cell) {
                0.0
            } else {
                sol.temperature(cell, 0.0)
            }
        })
        .collect())
}
