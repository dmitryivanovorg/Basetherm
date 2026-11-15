//! What drives the model in one step: weather, far-field temperatures, room temperatures.

use crate::spec::Schedule;
use basetherm_farfield::Undisturbed;
use basetherm_surface::Weather;

#[derive(Debug, Clone)]
pub struct Forcing {
    pub weather: Weather,
    /// Undisturbed ground temperature at each z cell of the 3D domain, C.
    pub far: Vec<f64>,
    /// Undisturbed ground temperature at the bottom face of the 3D domain, C.
    pub far_bottom: f64,
    /// Interior temperature of each room, C.
    pub rooms: Vec<f64>,
}

pub struct Series<'a> {
    pub weather: &'a [Weather],
    pub far: &'a Undisturbed,
    /// Number of z cells of the 3D domain (the first cells of the far-field column).
    pub nz: usize,
    pub rooms: Vec<&'a Schedule>,
}

fn mean_weather(ws: &[Weather]) -> Weather {
    let n = ws.len() as f64;
    let m = |f: fn(&Weather) -> f64| ws.iter().map(f).sum::<f64>() / n;
    Weather {
        t_air: m(|w| w.t_air),
        wind: m(|w| w.wind),
        ghi: m(|w| w.ghi),
        ir_sky: m(|w| w.ir_sky),
    }
}

impl Series<'_> {
    /// Values at the end of 0-based hour `h` of the year.
    pub fn hourly(&self, h: usize) -> Forcing {
        Forcing {
            weather: self.weather[h],
            far: (0..self.nz).map(|k| self.far.cell(h, k)).collect(),
            far_bottom: self.far.face(h, self.nz),
            rooms: self.rooms.iter().map(|r| r.at(h)).collect(),
        }
    }

    /// Mean over the 24 hours of 0-based day `d`.
    pub fn daily(&self, d: usize) -> Forcing {
        let hours: Vec<Forcing> = (d * 24..d * 24 + 24).map(|h| self.hourly(h)).collect();
        let n = hours.len() as f64;
        let avg = |f: &dyn Fn(&Forcing) -> f64| hours.iter().map(f).sum::<f64>() / n;
        Forcing {
            weather: mean_weather(&self.weather[d * 24..d * 24 + 24]),
            far: (0..self.nz).map(|k| avg(&|f| f.far[k])).collect(),
            far_bottom: avg(&|f| f.far_bottom),
            rooms: (0..self.rooms.len())
                .map(|r| avg(&|f| f.rooms[r]))
                .collect(),
        }
    }
}
