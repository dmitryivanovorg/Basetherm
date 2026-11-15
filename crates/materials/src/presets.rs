//! Soil classes for the moisture model. Sources:
//!
//! - Johansen, O. (1975), *Thermal conductivity of soils*, PhD thesis, NTH Trondheim (CRREL Draft
//!   Translation 637, 1977); as presented in Farouki, O. T. (1981), *Thermal properties of soils*,
//!   CRREL Monograph 81-1, and Andersland & Ladanyi, *Frozen Ground Engineering*:
//!   - conductivity of the grains `k_s = 7.7^q * 2.0^(1-q)` (quartz fraction `q`; 7.7 W/mK quartz,
//!     2.0 W/mK other minerals) and saturated conductivity `k_s^(1-n) * k_w^n` (in `soil.rs`),
//!   - dry conductivity of natural soils `k_dry = (0.135 d + 64.7) / (2700 - 0.947 d)`, `d` the dry
//!     density in kg/m3, with `d = 2700 (1 - n)`,
//!   - Kersten numbers (in `soil.rs`).
//! - Heat capacity of the grains: de Vries (1963), quartz and clay minerals 2.0 and 2.4 MJ/(m3 K),
//!   organic matter 2.5 MJ/(m3 K).
//!
//! Porosity and quartz fraction per class are typical mid-range values chosen for this table, not
//! measurements; replace them with site data when it exists. Peat is organic: its dry conductivity
//! is set directly (the mineral formula does not apply).

use crate::soil::{SoilKind, SoilParams};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub params: SoilParams,
}

const QUARTZ: f64 = 7.7;
const OTHER: f64 = 2.0;

fn dry(n: f64) -> f64 {
    let d = 2700.0 * (1.0 - n);
    (0.135 * d + 64.7) / (2700.0 - 0.947 * d)
}

fn mineral(
    id: &'static str,
    name: &'static str,
    kind: SoilKind,
    q: f64,
    n: f64,
    cv: f64,
) -> Preset {
    Preset {
        id,
        name,
        params: SoilParams {
            kind,
            k_solid: QUARTZ.powf(q) * OTHER.powf(1.0 - q),
            k_dry: dry(n),
            cv_solid: cv,
            porosity: n,
        },
    }
}

fn presets() -> [Preset; 5] {
    [
        mineral("gravel", "Gravel", SoilKind::Coarse, 0.3, 0.30, 2.0e6),
        mineral("sand", "Sand", SoilKind::Coarse, 0.6, 0.40, 2.0e6),
        mineral("silt", "Silt", SoilKind::Fine, 0.3, 0.45, 2.1e6),
        mineral("clay", "Clay", SoilKind::Fine, 0.1, 0.50, 2.3e6),
        Preset {
            id: "peat",
            name: "Peat",
            params: SoilParams {
                kind: SoilKind::Fine,
                k_solid: 0.25,
                k_dry: 0.06,
                cv_solid: 2.5e6,
                porosity: 0.85,
            },
        },
    ]
}

/// All presets, in display order.
pub static PRESETS: std::sync::LazyLock<[Preset; 5]> = std::sync::LazyLock::new(presets);

pub fn preset(id: &str) -> Option<Preset> {
    PRESETS.iter().find(|p| p.id == id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_conductivity_is_in_the_textbook_range_and_falls_with_porosity() {
        let (sand, clay) = (
            preset("sand").unwrap().params,
            preset("clay").unwrap().params,
        );
        assert!((0.2..0.35).contains(&sand.k_dry), "{}", sand.k_dry);
        assert!(clay.k_dry < sand.k_dry);
    }

    #[test]
    fn saturated_values_are_plausible_and_ordered() {
        let k = |id: &str, s: f64| preset(id).unwrap().params.k_thawed(s);
        // Saturated: sand 1.8-3.5 (by quartz content), clay 1.0-1.6, peat 0.4-0.7 W/(m K).
        assert!((1.8..3.6).contains(&k("sand", 1.0)), "{}", k("sand", 1.0));
        assert!((1.0..1.8).contains(&k("clay", 1.0)), "{}", k("clay", 1.0));
        assert!((0.3..0.8).contains(&k("peat", 1.0)), "{}", k("peat", 1.0));
        assert!(k("sand", 1.0) > k("silt", 1.0) && k("silt", 1.0) > k("clay", 1.0));
        for p in PRESETS.iter() {
            assert!(p.params.k_thawed(0.5) > p.params.k_dry, "{}", p.id);
        }
    }

    #[test]
    fn unknown_ids_are_none() {
        assert!(preset("granite").is_none());
    }
}
