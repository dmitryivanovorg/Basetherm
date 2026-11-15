use basetherm_climate_epw::{daily_means, parse, Error};

const DAYS: [(u8, u8); 12] = [
    (1, 31),
    (2, 28),
    (3, 31),
    (4, 30),
    (5, 31),
    (6, 30),
    (7, 31),
    (8, 31),
    (9, 30),
    (10, 31),
    (11, 30),
    (12, 31),
];

/// A syntactically valid 8760-hour EPW with the given per-hour values.
fn build(value: impl Fn(usize) -> (f64, f64, f64, f64)) -> String {
    let mut s = String::from("LOCATION,Test City,ST,TST,SYN,000000,45.00,-100.00,-6.0,100.0\n");
    for h in [
        "DESIGN CONDITIONS,0",
        "TYPICAL/EXTREME PERIODS,0",
        "GROUND TEMPERATURES,0",
        "HOLIDAYS/DAYLIGHT SAVING,No,0,0,0",
        "COMMENTS 1,x",
        "COMMENTS 2,x",
        "DATA PERIODS,1,1,Data,Sunday, 1/ 1,12/31",
    ] {
        s.push_str(h);
        s.push('\n');
    }
    let mut i = 0;
    for (month, len) in DAYS {
        for day in 1..=len {
            for hour in 1..=24u8 {
                let (t, wind, ghi, ir) = value(i);
                // year,month,day,hour,minute,flags,dry,dew,rh,p,exthor,extdn,IR,GHI,DNI,DHI,
                // 4 illuminances/luminance,wdir,wspd,sky,sky,vis,ceil,...
                s.push_str(&format!(
                    "1999,{month},{day},{hour},0,?0,{t},0.0,50,100000,0,0,{ir},{ghi},0,0,0,0,0,0,180,{wind},5,5,10.0,3000,9,999999999,10,0.1,0,88,0.0,999.0,99.0\n"
                ));
                i += 1;
            }
        }
    }
    s
}

fn plain(_: usize) -> (f64, f64, f64, f64) {
    (10.0, 3.0, 100.0, 300.0)
}

#[test]
fn parses_a_valid_year() {
    let epw = parse(&build(plain)).unwrap();
    assert_eq!(epw.records().len(), 8760);
    assert_eq!(epw.location.city, "Test City");
    assert_eq!(epw.location.latitude, 45.0);
    assert_eq!(epw.location.time_zone, -6.0);
    let w = epw.weather(4000);
    assert_eq!(
        (w.t_air, w.wind, w.ghi, w.ir_sky),
        (10.0, 3.0, 100.0, 300.0)
    );
    let m = epw.annual_means();
    assert!((m.t_air - 10.0).abs() < 1e-12 && (m.ghi - 100.0).abs() < 1e-12);
    let last = epw.records()[8759];
    assert_eq!((last.month, last.day, last.hour), (12, 31, 24));
}

#[test]
fn reads_each_value_from_its_own_column() {
    let epw = parse(&build(|i| {
        (
            i as f64 * 0.001,
            1.0 + (i % 7) as f64,
            (i % 11) as f64 * 10.0,
            200.0 + (i % 13) as f64,
        )
    }))
    .unwrap();
    let r = epw.records()[100];
    assert!((r.t_air - 0.1).abs() < 1e-12);
    assert_eq!(r.wind, 1.0 + 2.0);
    assert_eq!(r.ghi, 10.0);
    assert_eq!(r.ir_sky, 200.0 + 9.0);
}

#[test]
fn series_and_daily_means_work_on_parsed_data() {
    let epw = parse(&build(|i| ((i / 24) as f64 / 10.0, 3.0, 0.0, 300.0))).unwrap();
    let d = daily_means(&epw.series(|r| r.t_air));
    assert_eq!(d.len(), 365);
    assert!((d[200] - 20.0).abs() < 1e-12);
}

#[test]
fn rejects_wrong_record_count() {
    let text = build(plain);
    let short: String = text
        .lines()
        .take(8 + 8759)
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(parse(&short).unwrap_err(), Error::Length { found: 8759 });
}

#[test]
fn missing_infrared_is_an_error_with_a_line_number() {
    let e = parse(&build(|i| {
        (10.0, 3.0, 100.0, if i == 500 { 9999.0 } else { 300.0 })
    }))
    .unwrap_err();
    assert_eq!(
        e,
        Error::Missing {
            line: 8 + 501,
            field: "horizontal infrared radiation"
        }
    );
}

#[test]
fn other_missing_codes_are_errors() {
    assert!(matches!(
        parse(&build(|i| (
            if i == 3 { 99.9 } else { 10.0 },
            3.0,
            100.0,
            300.0
        ))),
        Err(Error::Missing {
            field: "dry bulb temperature",
            ..
        })
    ));
    assert!(matches!(
        parse(&build(|i| (
            10.0,
            if i == 3 { 999.0 } else { 3.0 },
            100.0,
            300.0
        ))),
        Err(Error::Missing {
            field: "wind speed",
            ..
        })
    ));
    assert!(matches!(
        parse(&build(|i| (
            10.0,
            3.0,
            if i == 3 { 9999.0 } else { 100.0 },
            300.0
        ))),
        Err(Error::Missing {
            field: "global horizontal radiation",
            ..
        })
    ));
}

#[test]
fn rejects_out_of_order_records() {
    let text = build(plain);
    let mut lines: Vec<&str> = text.lines().collect();
    lines.swap(8 + 10, 8 + 11);
    let swapped = lines.join("\n");
    assert!(matches!(parse(&swapped), Err(Error::Line { .. })));
}

#[test]
fn rejects_bad_headers_and_short_lines() {
    assert!(matches!(parse("nonsense"), Err(Error::Header(_))));
    let text = build(plain).replace(
        "LOCATION,Test City,ST,TST,SYN,000000,45.00,-100.00,-6.0,100.0",
        "LOCATION,Only,Three",
    );
    assert!(matches!(parse(&text), Err(Error::Header(_))));
    let mut lines: Vec<String> = build(plain).lines().map(String::from).collect();
    lines[20] = "1999,1,1,13,0".into();
    assert!(matches!(parse(&lines.join("\n")), Err(Error::Line { .. })));
}

#[test]
fn rejects_unphysical_weather() {
    assert!(matches!(
        parse(&build(|i| (
            10.0,
            3.0,
            100.0,
            if i == 7 { 20.0 } else { 300.0 }
        ))),
        Err(Error::Line { .. })
    ));
}

/// A real TMY3-derived EPW downloaded into refs/epw/; skipped when absent.
#[test]
fn parses_the_real_golden_co_file() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../refs/epw/USA_CO_Golden-NREL.724666_TMY3.epw");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("skipping: {} not found", path.display());
        return;
    };
    let epw = parse(&text).unwrap();
    assert_eq!(epw.records().len(), 8760);
    assert!((epw.location.latitude - 39.74).abs() < 1e-9 && epw.location.country == "USA");
    let first = epw.records()[0];
    assert_eq!((first.month, first.day, first.hour), (1, 1, 1));
    assert_eq!(
        (first.t_air, first.ir_sky, first.wind, first.ghi),
        (-3.0, 257.0, 0.0, 0.0)
    );
    let m = epw.annual_means();
    println!(
        "Golden CO means: T {:.2} C, wind {:.2} m/s, GHI {:.1} W/m2, IR {:.1} W/m2",
        m.t_air, m.wind, m.ghi, m.ir_sky
    );
    assert!((8.0..12.0).contains(&m.t_air), "{}", m.t_air);
    assert!(
        (150.0..250.0).contains(&m.ghi)
            && (250.0..330.0).contains(&m.ir_sky)
            && (1.0..6.0).contains(&m.wind)
    );
}
