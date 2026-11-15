//! EPW text to hourly records. Column positions follow the EnergyPlus Auxiliary Programs data
//! dictionary (0-based CSV index): month 1, day 2, hour 3, dry bulb 6, horizontal infrared 12,
//! global horizontal 13, wind speed 21. Missing codes: 99.9, 9999, 9999, 999.

use crate::error::Error;
use basetherm_surface::Weather;

pub const HOURS_PER_YEAR: usize = 8760;
const HEADER_LINES: usize = 8;
const MIN_FIELDS: usize = 22;

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub city: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    /// Hours from UTC.
    pub time_zone: f64,
    pub elevation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record {
    pub month: u8,
    pub day: u8,
    /// 1..=24; hour 1 covers 00:00-01:00.
    pub hour: u8,
    pub t_air: f64,
    pub wind: f64,
    pub ghi: f64,
    pub ir_sky: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Epw {
    pub location: Location,
    records: Vec<Record>,
}

impl Epw {
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    pub fn weather(&self, hour: usize) -> Weather {
        let r = &self.records[hour];
        Weather {
            t_air: r.t_air,
            wind: r.wind,
            ghi: r.ghi,
            ir_sky: r.ir_sky,
        }
    }

    pub fn series(&self, f: impl Fn(&Record) -> f64) -> Vec<f64> {
        self.records.iter().map(f).collect()
    }

    /// Annual mean of each input, as one `Weather` (the mean-field forcing).
    pub fn annual_means(&self) -> Weather {
        let n = self.records.len() as f64;
        let mean = |f: fn(&Record) -> f64| self.records.iter().map(f).sum::<f64>() / n;
        Weather {
            t_air: mean(|r| r.t_air),
            wind: mean(|r| r.wind),
            ghi: mean(|r| r.ghi),
            ir_sky: mean(|r| r.ir_sky),
        }
    }
}

fn num(fields: &[&str], idx: usize, line: usize, what: &'static str) -> Result<f64, Error> {
    fields[idx].trim().parse::<f64>().map_err(|_| Error::Line {
        line,
        why: format!("{what}: not a number: {:?}", fields[idx]),
    })
}

fn location(line: &str) -> Result<Location, Error> {
    let f: Vec<&str> = line.split(',').collect();
    if f.len() < 10 || f[0].trim() != "LOCATION" {
        return Err(Error::Header(
            "first record must be LOCATION with 10 fields".into(),
        ));
    }
    let n = |i: usize| {
        f[i].trim()
            .parse::<f64>()
            .map_err(|_| Error::Header(format!("LOCATION field {i} is not a number")))
    };
    Ok(Location {
        city: f[1].trim().to_string(),
        country: f[3].trim().to_string(),
        latitude: n(6)?,
        longitude: n(7)?,
        time_zone: n(8)?,
        elevation: n(9)?,
    })
}

/// Cumulative days before each month in a non-leap year.
const DAYS_BEFORE: [usize; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
const MONTH_LEN: [u8; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

pub fn parse(text: &str) -> Result<Epw, Error> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < HEADER_LINES
        || !lines[HEADER_LINES - 1]
            .trim_start()
            .starts_with("DATA PERIODS")
    {
        return Err(Error::Header(
            "expected 8 header records ending with DATA PERIODS".into(),
        ));
    }
    let location = location(lines[0])?;
    let data: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .skip(HEADER_LINES)
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| (i + 1, *l))
        .collect();
    if data.len() != HOURS_PER_YEAR {
        return Err(Error::Length { found: data.len() });
    }
    let mut records = Vec::with_capacity(HOURS_PER_YEAR);
    for (i, (line, text)) in data.into_iter().enumerate() {
        let f: Vec<&str> = text.split(',').collect();
        if f.len() < MIN_FIELDS {
            return Err(Error::Line {
                line,
                why: format!("only {} fields", f.len()),
            });
        }
        let (month, day, hour) = (
            num(&f, 1, line, "month")? as u8,
            num(&f, 2, line, "day")? as u8,
            num(&f, 3, line, "hour")? as u8,
        );
        // Records must run through the year in order, 24 per day, on a 365-day calendar.
        let day_of_year = i / 24;
        let expect_month = DAYS_BEFORE
            .iter()
            .rposition(|&d| d <= day_of_year)
            .expect("month") as u8
            + 1;
        let expect_day = (day_of_year - DAYS_BEFORE[expect_month as usize - 1]) as u8 + 1;
        debug_assert!(expect_day <= MONTH_LEN[expect_month as usize - 1]);
        if (month, day, hour) != (expect_month, expect_day, (i % 24) as u8 + 1) {
            return Err(Error::Line {
                line,
                why: format!("expected month {expect_month} day {expect_day} hour {}, found {month}/{day}/{hour}", i % 24 + 1),
            });
        }
        let t_air = num(&f, 6, line, "dry bulb")?;
        let ir_sky = num(&f, 12, line, "horizontal infrared")?;
        let ghi = num(&f, 13, line, "global horizontal radiation")?;
        let wind = num(&f, 21, line, "wind speed")?;
        for (value, missing, field) in [
            (t_air, 99.9, "dry bulb temperature"),
            (ir_sky, 9999.0, "horizontal infrared radiation"),
            (ghi, 9999.0, "global horizontal radiation"),
            (wind, 999.0, "wind speed"),
        ] {
            if value >= missing {
                return Err(Error::Missing { line, field });
            }
        }
        let rec = Record {
            month,
            day,
            hour,
            t_air,
            wind,
            ghi,
            ir_sky,
        };
        let w = Weather {
            t_air,
            wind,
            ghi,
            ir_sky,
        };
        w.validate().map_err(|e| Error::Line {
            line,
            why: e.to_string(),
        })?;
        records.push(rec);
    }
    Ok(Epw { location, records })
}
