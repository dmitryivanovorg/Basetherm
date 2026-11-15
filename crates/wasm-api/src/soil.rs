//! Soil class table and derived properties for the UI.

use crate::project::SoilSpecJson;
use basetherm_materials::PRESETS;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresetInfo {
    id: &'static str,
    name: &'static str,
    porosity: f64,
    k_dry: f64,
    /// Conductivity when saturated, W/(m K).
    k_sat: f64,
}

/// The soil classes (gravel, sand, silt, clay, peat), as JSON.
pub fn presets_json() -> Result<String, String> {
    let list: Vec<PresetInfo> = PRESETS
        .iter()
        .map(|p| PresetInfo {
            id: p.id,
            name: p.name,
            porosity: p.params.porosity,
            k_dry: p.params.k_dry,
            k_sat: p.params.k_thawed(1.0),
        })
        .collect();
    serde_json::to_string(&list).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Properties {
    k: f64,
    k_frozen: f64,
    cv: f64,
    cv_frozen: f64,
    /// J/m3 (zero when freezing is off).
    latent: f64,
}

/// Properties of a class at a saturation (the spec's JSON), as JSON.
pub fn properties_json(spec_json: &str) -> Result<String, String> {
    let spec: SoilSpecJson =
        serde_json::from_str(spec_json).map_err(|e| format!("soil spec: {e}"))?;
    let m = spec.material()?;
    serde_json::to_string(&Properties {
        k: m.k_thawed,
        k_frozen: m.k_frozen,
        cv: m.cv_thawed,
        cv_frozen: m.cv_frozen,
        latent: m.latent,
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_and_properties_come_back_as_json() {
        let p: serde_json::Value = serde_json::from_str(&presets_json().unwrap()).unwrap();
        assert_eq!(p.as_array().unwrap().len(), 5);
        let wet = properties_json(r#"{"preset":"sand","saturation":1.0,"freezing":true}"#).unwrap();
        let dry = properties_json(r#"{"preset":"sand","saturation":0.0}"#).unwrap();
        let (wet, dry): (serde_json::Value, serde_json::Value) = (
            serde_json::from_str(&wet).unwrap(),
            serde_json::from_str(&dry).unwrap(),
        );
        assert!(wet["k"].as_f64() > dry["k"].as_f64());
        assert!(wet["latent"].as_f64().unwrap() > 1.0e8);
        assert_eq!(dry["latent"].as_f64(), Some(0.0));
        assert!(properties_json(r#"{"preset":"sand","saturation":2.0}"#).is_err());
    }
}
