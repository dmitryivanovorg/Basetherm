//! The versioned JSON project description (SI units) and its conversion to solver types.

use basetherm_basement::{
    Building, Domain, HorizontalInsulation, Layer, Level, Limit, Preset, Room, RunOptions,
    Schedule, SoilLayer,
};
use basetherm_materials::{preset, Material};
use basetherm_surface::{Convection, GroundSurface};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 2;

/// A material. Frozen properties default to the thawed ones; `latent > 0` switches freezing on.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialJson {
    /// Thawed conductivity, W/(m K).
    pub k: f64,
    /// Thawed volumetric heat capacity, J/(m3 K).
    pub cv: f64,
    pub k_frozen: Option<f64>,
    pub cv_frozen: Option<f64>,
    /// J/m3.
    #[serde(default)]
    pub latent: f64,
    /// C.
    #[serde(default)]
    pub t_freeze: f64,
    /// K, floored at 0.5 K.
    pub half_width: Option<f64>,
}

impl From<&MaterialJson> for Material {
    fn from(m: &MaterialJson) -> Self {
        let mut mat = Material::constant(m.k, m.cv);
        mat.k_frozen = m.k_frozen.unwrap_or(m.k);
        mat.cv_frozen = m.cv_frozen.unwrap_or(m.cv);
        mat.latent = m.latent;
        mat.t_freeze = m.t_freeze;
        mat.with_half_width(m.half_width.unwrap_or(Material::MIN_HALF_WIDTH))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScheduleJson {
    Constant(f64),
    Monthly(Vec<f64>),
    Hourly(Vec<f64>),
}

/// A room: a rectilinear polygon, corners as `[x, y]` in interior coordinates, m.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomJson {
    pub name: String,
    pub poly: Vec<[f64; 2]>,
    pub temperature: ScheduleJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelJson {
    pub name: String,
    pub height: f64,
    pub rooms: Vec<RoomJson>,
}

/// Where a layer exists (omitted: everywhere). Wall layers take `depth`, floor layers `perimeter`
/// or `core`; `value` in m.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitJson {
    pub kind: LimitKind,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LimitKind {
    Depth,
    Perimeter,
    Core,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerJson {
    pub thickness: f64,
    pub material: MaterialJson,
    #[serde(default)]
    pub limit: Option<LimitJson>,
}

/// A soil from the class table at a degree of saturation (0..1). With `freezing` the pore water
/// freezes (latent heat); without it the soil is a constant-property solid.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoilSpecJson {
    pub preset: String,
    pub saturation: f64,
    #[serde(default)]
    pub freezing: bool,
    pub half_width: Option<f64>,
}

/// A soil layer: either explicit properties or a class at a saturation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilJson {
    pub top: f64,
    pub material: Option<MaterialJson>,
    pub soil: Option<SoilSpecJson>,
}

impl SoilSpecJson {
    pub fn material(&self) -> Result<Material, String> {
        let p = preset(&self.preset).ok_or_else(|| format!("unknown soil '{}'", self.preset))?;
        if !(0.0..=1.0).contains(&self.saturation) {
            return Err("the degree of saturation must be between 0 and 1".into());
        }
        let mut m = p.params.material(
            self.saturation,
            0.0,
            self.half_width.unwrap_or(Material::MIN_HALF_WIDTH),
        );
        if !self.freezing {
            m.latent = 0.0;
        }
        Ok(m)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsulationJson {
    pub width: f64,
    pub thickness: f64,
    pub depth: f64,
    pub material: MaterialJson,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainJson {
    pub margin: f64,
    pub depth_below_floor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingJson {
    pub lx: f64,
    pub ly: f64,
    /// Footprint corners `[x, y]`; omitted: the rectangle `[0, lx] x [0, ly]`.
    #[serde(default)]
    pub footprint: Option<Vec<[f64; 2]>>,
    #[serde(default)]
    pub levels: Vec<LevelJson>,
    pub wall: Vec<LayerJson>,
    pub floor: Vec<LayerJson>,
    #[serde(default)]
    pub ground_floor_rooms: Vec<RoomJson>,
    pub h_wall: f64,
    pub h_floor: f64,
    pub soil: Vec<SoilJson>,
    pub insulation: Option<InsulationJson>,
    pub domain: Option<DomainJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConvectionJson {
    /// The project default (Walton/ASHRAE rough surface).
    WaltonRough,
    Constant(f64),
    WindLinear {
        a: f64,
        b: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceJson {
    pub albedo: f64,
    pub emissivity: f64,
    pub convection: ConvectionJson,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresetJson {
    Draft,
    Standard,
    Fine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsJson {
    pub coarse_years: Option<usize>,
    pub hourly_years: Option<usize>,
    pub drift_tol: Option<f64>,
    /// Daily-step warm-up before the hourly years (default on).
    pub warm_up: Option<bool>,
    /// Start from the periodic solution (default on); off: the undisturbed ground profile.
    pub periodic_init: Option<bool>,
    /// Couple the domain sides and bottom to the undisturbed ground (default on); off: adiabatic.
    pub condition_edges: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub building: BuildingJson,
    pub preset: PresetJson,
    pub surface: SurfaceJson,
    pub options: Option<OptionsJson>,
}

fn schedule(s: &ScheduleJson) -> Result<Schedule, String> {
    Ok(match s {
        ScheduleJson::Constant(t) => Schedule::Constant(*t),
        ScheduleJson::Monthly(m) => Schedule::Monthly(
            m.as_slice()
                .try_into()
                .map_err(|_| "a monthly schedule needs 12 values".to_string())?,
        ),
        ScheduleJson::Hourly(h) => Schedule::Hourly(h.clone()),
    })
}

fn room(r: &RoomJson) -> Result<Room, String> {
    Ok(Room {
        name: r.name.clone(),
        poly: r.poly.iter().map(|p| (p[0], p[1])).collect(),
        temperature: schedule(&r.temperature)?,
    })
}

fn layers(l: &[LayerJson]) -> Vec<Layer> {
    l.iter()
        .map(|l| {
            let layer = Layer::new(l.thickness, (&l.material).into());
            layer.limited(match &l.limit {
                None => Limit::Full,
                Some(j) => match j.kind {
                    LimitKind::Depth => Limit::Depth(j.value),
                    LimitKind::Perimeter => Limit::Perimeter(j.value),
                    LimitKind::Core => Limit::Core(j.value),
                },
            })
        })
        .collect()
}

fn soil_layer(s: &SoilJson) -> Result<SoilLayer, String> {
    let material = match (&s.material, &s.soil) {
        (Some(m), None) => m.into(),
        (None, Some(spec)) => spec.material()?,
        _ => return Err("a soil layer needs either 'material' or 'soil'".into()),
    };
    Ok(SoilLayer {
        top: s.top,
        material,
    })
}

impl BuildingJson {
    pub fn to_building(&self) -> Result<Building, String> {
        let rooms = |r: &[RoomJson]| r.iter().map(room).collect::<Result<Vec<_>, _>>();
        let levels = self
            .levels
            .iter()
            .map(|l| {
                Ok(Level {
                    name: l.name.clone(),
                    height: l.height,
                    rooms: rooms(&l.rooms)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Building {
            lx: self.lx,
            ly: self.ly,
            footprint: self
                .footprint
                .as_ref()
                .map(|f| f.iter().map(|p| (p[0], p[1])).collect()),
            levels,
            wall: layers(&self.wall),
            floor: layers(&self.floor),
            ground_floor_rooms: rooms(&self.ground_floor_rooms)?,
            h_wall: self.h_wall,
            h_floor: self.h_floor,
            soil: self
                .soil
                .iter()
                .map(soil_layer)
                .collect::<Result<Vec<_>, _>>()?,
            insulation: self.insulation.as_ref().map(|h| HorizontalInsulation {
                width: h.width,
                thickness: h.thickness,
                depth: h.depth,
                material: (&h.material).into(),
            }),
            domain: self
                .domain
                .as_ref()
                .map_or_else(Domain::default, |d| Domain {
                    margin: d.margin,
                    depth_below_floor: d.depth_below_floor,
                }),
        })
    }
}

impl Project {
    pub fn parse(json: &str) -> Result<Self, String> {
        let p: Project = serde_json::from_str(json).map_err(|e| format!("project file: {e}"))?;
        if p.version != SCHEMA_VERSION {
            return Err(format!(
                "project schema version {} is not supported (expected {SCHEMA_VERSION})",
                p.version
            ));
        }
        Ok(p)
    }

    pub fn preset(&self) -> Preset {
        match self.preset {
            PresetJson::Draft => Preset::Draft,
            PresetJson::Standard => Preset::Standard,
            PresetJson::Fine => Preset::Fine,
        }
    }

    pub fn run_options(&self) -> Result<RunOptions, String> {
        let s = &self.surface;
        let convection = match s.convection {
            ConvectionJson::WaltonRough => Convection::WALTON_ROUGH,
            ConvectionJson::Constant(h) => Convection::Constant(h),
            ConvectionJson::WindLinear { a, b } => Convection::WindLinear { a, b },
        };
        let ground =
            GroundSurface::new(s.albedo, s.emissivity, convection).map_err(|e| e.to_string())?;
        let mut o = RunOptions::new(ground);
        if let Some(j) = &self.options {
            o.coarse_years = j.coarse_years.unwrap_or(o.coarse_years);
            o.hourly_years = j.hourly_years.unwrap_or(o.hourly_years);
            o.drift_tol = j.drift_tol.unwrap_or(o.drift_tol);
            if j.warm_up == Some(false) {
                o.coarse_years = 0;
            }
            o.periodic_init = j.periodic_init.unwrap_or(o.periodic_init);
            o.condition_edges = j.condition_edges.unwrap_or(o.condition_edges);
        }
        Ok(o)
    }
}
