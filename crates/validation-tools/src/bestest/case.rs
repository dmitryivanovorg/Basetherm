//! The case table (SPEC.md): geometry, surface conditions, soil extent, weather file.

/// Surface condition on the slab top or the exterior ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Surface {
    /// Fixed surface temperature (direct-input surface temperature).
    Fixed,
    /// Film coefficient to the air temperature, W/(m2 K).
    Film(f64),
}

/// What lies on the ground surface in the strip under the wall (width W around the slab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    Adiabatic,
    /// Linear ramp from the slab temperature to the ground temperature across the strip (GC10a).
    Ramp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub name: &'static str,
    /// Slab length (x) and width (y), m.
    pub l: f64,
    pub b: f64,
    pub interior: Surface,
    pub exterior: Surface,
    pub ring: Ring,
    /// Deep boundary depth E and far-field distance F from the slab edge, m.
    pub depth: f64,
    pub far: f64,
    /// Soil and slab conductivity, W/(m K).
    pub k: f64,
    /// TM2 weather file (outdoor dry-bulb), or `None` for a constant 10 C.
    pub weather: Option<&'static str>,
}

pub const T_IN: f64 = 30.0;
pub const T_DEEP: f64 = 10.0;
/// Width of the perimeter strip (wall thickness), m.
pub const RING_W: f64 = 0.24;
pub const RHO: f64 = 1490.0;
pub const C_P: f64 = 1800.0;

#[allow(clippy::too_many_arguments)]
fn mk(
    name: &'static str,
    (l, b): (f64, f64),
    interior: Surface,
    exterior: Surface,
    (depth, far): (f64, f64),
    k: f64,
    weather: Option<&'static str>,
) -> Case {
    let ring = if name == "GC10a" {
        Ring::Ramp
    } else {
        Ring::Adiabatic
    };
    Case {
        name,
        l,
        b,
        interior,
        exterior,
        ring,
        depth,
        far,
        k,
        weather,
    }
}

/// All cases from SPEC.md.
pub fn cases() -> Vec<Case> {
    use Surface::{Film, Fixed};
    let sq = (12.0, 12.0);
    let h = Film(100.0);
    let (hi, he) = (Film(7.95), Film(11.95));
    vec![
        mk("GC10a", sq, Fixed, Fixed, (40.0, 40.0), 1.9, None),
        mk("GC30a", sq, Fixed, Fixed, (30.0, 20.0), 1.9, None),
        mk("GC30b", sq, h, h, (15.0, 15.0), 1.9, None),
        mk("GC60b", sq, hi, h, (15.0, 15.0), 1.9, None),
        mk("GC65b", sq, hi, he, (15.0, 15.0), 1.9, None),
        mk("GC30c", sq, hi, Fixed, (15.0, 8.0), 1.9, None),
        mk(
            "GC40a",
            sq,
            Fixed,
            Fixed,
            (30.0, 20.0),
            1.9,
            Some("GCSP-W40.TM2"),
        ),
        mk("GC40b", sq, h, h, (15.0, 15.0), 1.9, Some("GCSP-W20.TM2")),
        mk(
            "GC45b",
            (36.0, 4.0),
            h,
            h,
            (15.0, 15.0),
            1.9,
            Some("GCSP-W20.TM2"),
        ),
        mk(
            "GC50b",
            (80.0, 80.0),
            h,
            h,
            (15.0, 15.0),
            1.9,
            Some("GCSP-W20.TM2"),
        ),
        mk("GC55b", sq, h, h, (2.0, 15.0), 1.9, Some("GCSP-W20.TM2")),
        mk("GC70b", sq, hi, he, (15.0, 15.0), 1.9, Some("GCSP-W01.TM2")),
        mk("GC80b", sq, h, h, (15.0, 15.0), 0.5, Some("GCSP-W20.TM2")),
        mk(
            "GC40c",
            sq,
            hi,
            Fixed,
            (15.0, 8.0),
            1.9,
            Some("GCSP-W40.TM2"),
        ),
        mk(
            "GC45c",
            (36.0, 4.0),
            hi,
            Fixed,
            (15.0, 8.0),
            1.9,
            Some("GCSP-W40.TM2"),
        ),
        mk(
            "GC55c",
            sq,
            hi,
            Fixed,
            (5.0, 8.0),
            1.9,
            Some("GCSP-W40.TM2"),
        ),
        mk(
            "GC80c",
            sq,
            hi,
            Fixed,
            (15.0, 8.0),
            0.85,
            Some("GCSP-W40.TM2"),
        ),
    ]
}

impl Case {
    pub fn by_name(name: &str) -> Option<Case> {
        cases().into_iter().find(|c| c.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_all_seventeen_cases_and_one_ramp() {
        let c = cases();
        assert_eq!(c.len(), 17);
        assert_eq!(c.iter().filter(|c| c.ring == Ring::Ramp).count(), 1);
        assert_eq!(Case::by_name("GC55b").unwrap().depth, 2.0);
        assert_eq!(Case::by_name("GC80c").unwrap().k, 0.85);
    }
}
