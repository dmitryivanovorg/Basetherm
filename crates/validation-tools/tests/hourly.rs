//! BESTEST hourly steady-periodic year: annual total, peak-hour conduction, and phase shift of the
//! peak versus the coldest hour. Slow (minutes per case); run with
//!   cargo test -p basetherm-validation-tools --test hourly -- --ignored --nocapture
//! and restrict with BESTEST_CASES=GC40b,GC55b. Needs the weather files in `refs/`.

mod common;
use basetherm_validation_tools::bestest::{cases, run_hourly};
use basetherm_validation_tools::tm2;
use common::{assert_passes, reference};
use std::path::PathBuf;
use std::time::Instant;

const WARMUP_HOURS: usize = 240;

fn weather(name: &str) -> Option<Vec<f64>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../refs/bestest-gc/data")
        .join(name);
    tm2::dry_bulb(&std::fs::read_to_string(path).ok()?).ok()
}

fn selected(name: &str) -> bool {
    match std::env::var("BESTEST_CASES") {
        Ok(list) => list.split(',').any(|c| c.trim() == name),
        Err(_) => true,
    }
}

#[test]
#[ignore = "slow: hourly year per case"]
fn hourly_year_matches_reference() {
    let mut ran = 0;
    for case in cases()
        .into_iter()
        .filter(|c| c.weather.is_some() && selected(c.name))
    {
        let Some(t_out) = weather(case.weather.unwrap()) else {
            println!("skipping {}: weather file not found", case.name);
            continue;
        };
        let start = Instant::now();
        let r = run_hourly(&case, &common::mesh(), &t_out, WARMUP_HOURS, 1).unwrap();
        println!(
            "{}: {:.0} s; annual {:.0} kWh, peak {:.0} W at hour {} (phase shift {} h)",
            case.name,
            start.elapsed().as_secs_f64(),
            r.annual_kwh,
            r.peak_w,
            r.peak_hour,
            r.phase_shift_h
        );
        assert_passes(case.name, "floor_conduction_annual", r.annual_kwh);
        assert_passes(case.name, "floor_conduction_peak_hour", r.peak_w);
        let ph = reference(case.name, "peak_phase_shift");
        let (lo, hi) = ph.pass.expect("phase band");
        println!(
            "{} phase shift {} h, reference {:.0}..{:.0} h (envelope +/-24 h: {lo:.0}..{hi:.0})",
            case.name, r.phase_shift_h, ph.min, ph.max
        );
        assert!(
            (lo..=hi).contains(&(r.phase_shift_h as f64)),
            "{}: phase shift {} outside {lo}..{hi}",
            case.name,
            r.phase_shift_h
        );
        ran += 1;
    }
    println!("{ran} hourly cases checked");
}
