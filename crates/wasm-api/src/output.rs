//! Results as JSON (annual energy and dated peaks) plus the hourly series as typed arrays.

use basetherm_basement::{Kind, Peak, Results, Scope};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PeakJson {
    watts: f64,
    hour: usize,
    label: String,
}

#[derive(Serialize)]
struct PeaksJson {
    loss: Option<PeakJson>,
    gain: Option<PeakJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RoomJson {
    name: String,
    floor_kwh: f64,
    wall_kwh: f64,
    total_kwh: f64,
    peaks: PeaksJson,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YearJson {
    hourly: bool,
    kwh: f64,
    peak: Option<f64>,
    drift: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryJson {
    history: Vec<YearJson>,
    rooms: Vec<RoomJson>,
    total_kwh: f64,
    basement_peaks: PeaksJson,
    drift: f64,
    converged: bool,
    coarse_years: usize,
    hourly_years: usize,
    symmetry: String,
    report: String,
}

fn peak(p: Option<Peak>) -> Option<PeakJson> {
    p.map(|p| PeakJson {
        watts: p.watts,
        hour: p.hour,
        label: p.date.to_string(),
    })
}

fn peaks(r: &Results, scope: Scope) -> PeaksJson {
    let p = r.peaks(scope, None);
    PeaksJson {
        loss: peak(p.loss),
        gain: peak(p.gain),
    }
}

pub fn summary_json(r: &Results) -> Result<String, String> {
    let rooms = r
        .room_names
        .iter()
        .enumerate()
        .map(|(i, name)| RoomJson {
            name: name.clone(),
            floor_kwh: r.annual(i, Kind::Floor),
            wall_kwh: r.annual(i, Kind::Wall),
            total_kwh: r.annual_room(i),
            peaks: peaks(r, Scope::Room(i)),
        })
        .collect();
    let s = SummaryJson {
        history: r
            .history
            .iter()
            .map(|y| YearJson {
                hourly: y.hourly,
                kwh: y.kwh,
                peak: y.peak,
                drift: y.drift,
            })
            .collect(),
        rooms,
        total_kwh: r.annual_total(),
        basement_peaks: peaks(r, Scope::Basement),
        drift: r.drift,
        converged: r.converged,
        coarse_years: r.coarse_years,
        hourly_years: r.hourly_years,
        symmetry: r.symmetry.to_string(),
        report: r.report(),
    };
    serde_json::to_string(&s).map_err(|e| e.to_string())
}

/// Hourly flows (W) of the reporting year, row-major `[hour][room][floor, wall]`.
pub fn hourly(r: &Results) -> Vec<f64> {
    let rooms = r.room_names.len();
    let mut v = Vec::with_capacity(r.hours() * rooms * 2);
    for h in 0..r.hours() {
        for room in 0..rooms {
            v.push(r.flow(h, room, Kind::Floor));
            v.push(r.flow(h, room, Kind::Wall));
        }
    }
    v
}

/// Faces of the section planes, as JSON: `{"x": [...], "y": [...], "z": [...]}` (m).
pub fn section_axes_json(r: &Results) -> Result<String, String> {
    let s = &r.sections;
    serde_json::to_string(&serde_json::json!({ "x": s.x, "y": s.y, "z": s.z }))
        .map_err(|e| e.to_string())
}

/// Monthly means of a centre plane (temperature in C, or heat flux in W/m2), month-major, then cell along the plane, then
/// depth (fastest). NaN marks the building void.
pub fn section(months: &[Vec<f64>]) -> Vec<f64> {
    months.iter().flatten().copied().collect()
}
