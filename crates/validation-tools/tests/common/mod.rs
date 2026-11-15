#![allow(dead_code)]

use basetherm_validation_tools::bestest::Mesh;
use basetherm_validation_tools::reference::{lookup, Row};

pub fn reference(case: &str, quantity: &str) -> Row {
    lookup(case, quantity).unwrap_or_else(|| panic!("no reference for {case} {quantity}"))
}

/// Pass rule (SPEC.md): within 5% of the verified-model mean, or inside their min-max envelope.
pub fn assert_passes(case: &str, quantity: &str, value: f64) {
    let r = reference(case, quantity);
    let (lo, hi) = r.pass.expect("pass band");
    let dev = 100.0 * (value - r.mean) / r.mean;
    println!("{case} {quantity}: {value:.1} {} (reference mean {:.1}, {dev:+.2}%, band {lo:.1}..{hi:.1})", r.unit, r.mean);
    assert!(
        value >= lo && value <= hi,
        "{case} {quantity}: {value:.1} outside {lo:.1}..{hi:.1}"
    );
}

/// Mesh for the annual and hourly runs: `BESTEST_MESH=fine` selects the Fine preset; anything else
/// (or unset) gives Standard.
pub fn mesh() -> Mesh {
    match std::env::var("BESTEST_MESH").as_deref() {
        Ok("fine") => Mesh::fine(),
        _ => Mesh::standard(),
    }
}
