//! BESTEST annual floor conduction. For these linear, constant-property cases the annual total
//! equals the mean flux times 8760 h, so the steady mean solve with the weather file's mean
//! temperature is exact. Needs the downloaded weather files in `refs/` and is skipped without them.

mod common;
use basetherm_validation_tools::bestest::{annual_kwh, cases, run_mean};
use basetherm_validation_tools::tm2;
use common::assert_passes;
use std::path::PathBuf;

fn refs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../refs/bestest-gc/data")
}

#[test]
fn annual_totals_pass() {
    let dir = refs_dir();
    let mut ran = 0;
    for case in cases().into_iter().filter(|c| c.weather.is_some()) {
        let path = dir.join(case.weather.unwrap());
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("skipping {}: {} not found", case.name, path.display());
            continue;
        };
        let t = tm2::dry_bulb(&text).unwrap();
        assert_eq!(t.len(), 8760, "{}", case.name);
        let t_out = tm2::mean(&t);
        let r = run_mean(&case, &common::mesh(), t_out).unwrap();
        println!(
            "{}: mean outdoor {t_out:.3} C, {} cells, {} iterations",
            case.name, r.cells, r.iterations
        );
        assert_passes(case.name, "floor_conduction_annual", annual_kwh(r.floor_w));
        ran += 1;
    }
    println!("{ran} annual cases checked");
}
