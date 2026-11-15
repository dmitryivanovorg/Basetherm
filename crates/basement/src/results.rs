//! Results of a full-year run: hourly and annual flows per room, peak loss and gain with dates.
//!
//! Sign convention: heat loss (interior to ground) is negative, heat gain positive. All flows are
//! for the whole building, with any mirror-symmetry folding already undone.

use crate::model::Kind;
use crate::sections::Sections;
use crate::symmetry::Symmetry;
use std::fmt;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTH_DAYS: [usize; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Calendar position of an hour of the 365-day weather year. Hour 1 covers 00:00-01:00.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub month: u8,
    pub day: u8,
    /// 1..=24.
    pub hour: u8,
}

impl Date {
    /// From a 0-based hour of the year.
    pub fn from_hour(h: usize) -> Date {
        let (mut day, hour) = (h / 24, (h % 24) as u8 + 1);
        for (m, len) in MONTH_DAYS.iter().enumerate() {
            if day < *len {
                return Date {
                    month: m as u8 + 1,
                    day: day as u8 + 1,
                    hour,
                };
            }
            day -= len;
        }
        Date {
            month: 12,
            day: 31,
            hour,
        }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}, hour {} ({:02}:00-{:02}:00)",
            MONTHS[usize::from(self.month) - 1],
            self.day,
            self.hour,
            self.hour - 1,
            self.hour % 24
        )
    }
}

/// An extreme hourly heat flow and when it happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    /// Heat flow, W (negative for a loss).
    pub watts: f64,
    /// 0-based hour of the year.
    pub hour: usize,
    pub date: Date,
}

/// Largest loss (most negative flow) and largest gain (most positive flow) of a year. Either is
/// `None` if the flow never has that sign.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peaks {
    pub loss: Option<Peak>,
    pub gain: Option<Peak>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Room(usize),
    /// The whole basement: the sum over all rooms, hour by hour.
    Basement,
}

/// Which mirror planes the run used, or why not.
#[derive(Debug, Clone, PartialEq)]
pub struct SymmetryInfo {
    pub symmetry: Symmetry,
    pub notes: Vec<String>,
}

impl fmt::Display for SymmetryInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.symmetry;
        match (s.x, s.y) {
            (true, true) => write!(
                f,
                "mirror symmetry in x and y: a quarter of the domain was solved"
            )?,
            (true, false) => write!(f, "mirror symmetry in x: half of the domain was solved")?,
            (false, true) => write!(f, "mirror symmetry in y: half of the domain was solved")?,
            (false, false) => write!(f, "full domain solved")?,
        }
        if !self.notes.is_empty() {
            write!(f, " ({})", self.notes.join("; "))?;
        }
        Ok(())
    }
}

/// One simulated year of the run (warm-up or hourly), for the convergence graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YearRecord {
    /// An hourly year (else a daily-step warm-up year).
    pub hourly: bool,
    /// Annual heat flow of the whole building, kWh (loss negative).
    pub kwh: f64,
    /// Peak hourly heat loss of the whole building, W (most negative hourly sum); `None` for a
    /// warm-up year, which has no hourly flows.
    pub peak: Option<f64>,
    /// Change from the previous year (the convergence measure: annual flow for warm-up years, peak
    /// loss for hourly years); `None` for the first year.
    pub drift: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Results {
    pub room_names: Vec<String>,
    pub(crate) groups: usize,
    /// Hourly flows of the last year, W: `hourly[hour * groups + group]`.
    pub(crate) hourly: Vec<f64>,
    /// Annual energy per group, kWh.
    pub(crate) annual_kwh: Vec<f64>,
    /// Relative change of the annual heat flow between the last two years (L1 over groups).
    pub drift: f64,
    pub converged: bool,
    pub coarse_years: usize,
    pub hourly_years: usize,
    pub symmetry: SymmetryInfo,
    /// Monthly mean temperature on the two centre planes (last hourly year).
    pub sections: Sections,
    /// Every simulated year in order.
    pub history: Vec<YearRecord>,
}

impl Results {
    pub fn hours(&self) -> usize {
        self.hourly.len() / self.groups
    }

    /// Heat flow through a room's floor or walls in 0-based hour `hour`, W.
    pub fn flow(&self, hour: usize, room: usize, kind: Kind) -> f64 {
        self.hourly[hour * self.groups + 2 * room + kind as usize]
    }

    /// Annual energy through a room's floor or walls, kWh.
    pub fn annual(&self, room: usize, kind: Kind) -> f64 {
        self.annual_kwh[2 * room + kind as usize]
    }

    /// Annual energy of a room (floor plus walls), kWh.
    pub fn annual_room(&self, room: usize) -> f64 {
        self.annual(room, Kind::Floor) + self.annual(room, Kind::Wall)
    }

    pub fn annual_total(&self) -> f64 {
        self.annual_kwh.iter().sum()
    }

    /// Hourly flow of a scope, W. `kind` restricts it to floor or walls (`None`: both).
    pub fn scope_flow(&self, scope: Scope, kind: Option<Kind>, hour: usize) -> f64 {
        let rooms = self.room_names.len();
        let one = |r: usize| match kind {
            Some(k) => self.flow(hour, r, k),
            None => self.flow(hour, r, Kind::Floor) + self.flow(hour, r, Kind::Wall),
        };
        match scope {
            Scope::Room(r) => one(r),
            Scope::Basement => (0..rooms).map(one).sum(),
        }
    }

    /// Peak loss and peak gain of a room or of the whole basement over the reporting year.
    pub fn peaks(&self, scope: Scope, kind: Option<Kind>) -> Peaks {
        let (mut loss, mut gain): (Option<Peak>, Option<Peak>) = (None, None);
        for hour in 0..self.hours() {
            let w = self.scope_flow(scope, kind, hour);
            let peak = || Peak {
                watts: w,
                hour,
                date: Date::from_hour(hour),
            };
            if w < 0.0 && loss.is_none_or(|p| w < p.watts) {
                loss = Some(peak());
            }
            if w > 0.0 && gain.is_none_or(|p| w > p.watts) {
                gain = Some(peak());
            }
        }
        Peaks { loss, gain }
    }

    /// A plain-text summary: annual energy and the dated peaks per room and for the whole basement.
    pub fn report(&self) -> String {
        let fmt_peak = |p: Option<Peak>| {
            p.map_or("none".to_string(), |p| {
                format!("{:.0} W on {}", p.watts, p.date)
            })
        };
        let mut s = format!("{}\n", self.symmetry);
        s.push_str(&format!(
            "annual drift {:.2}% ({}), {} coarse + {} hourly years\n",
            100.0 * self.drift,
            if self.converged {
                "converged"
            } else {
                "NOT converged"
            },
            self.coarse_years,
            self.hourly_years
        ));
        let scopes = (0..self.room_names.len())
            .map(Scope::Room)
            .chain([Scope::Basement]);
        for scope in scopes {
            let (name, annual) = match scope {
                Scope::Room(r) => (self.room_names[r].clone(), self.annual_room(r)),
                Scope::Basement => ("whole basement".to_string(), self.annual_total()),
            };
            let p = self.peaks(scope, None);
            s.push_str(&format!(
                "{name}: annual {annual:.0} kWh; peak loss {}; peak gain {}\n",
                fmt_peak(p.loss),
                fmt_peak(p.gain)
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_follow_the_epw_calendar() {
        assert_eq!(
            Date::from_hour(0),
            Date {
                month: 1,
                day: 1,
                hour: 1
            }
        );
        assert_eq!(
            Date::from_hour(23),
            Date {
                month: 1,
                day: 1,
                hour: 24
            }
        );
        assert_eq!(
            Date::from_hour(31 * 24),
            Date {
                month: 2,
                day: 1,
                hour: 1
            }
        );
        assert_eq!(
            Date::from_hour((31 + 28) * 24 + 5),
            Date {
                month: 3,
                day: 1,
                hour: 6
            }
        );
        assert_eq!(
            Date::from_hour(8759),
            Date {
                month: 12,
                day: 31,
                hour: 24
            }
        );
        assert_eq!(
            Date::from_hour(14 * 24 + 3).to_string(),
            "Jan 15, hour 4 (03:00-04:00)"
        );
        assert_eq!(
            Date::from_hour(23).to_string(),
            "Jan 1, hour 24 (23:00-00:00)"
        );
    }

    fn results(flows: &[(f64, f64)]) -> Results {
        // Two rooms; `flows[hour] = (room 0 floor+walls, room 1 floor+walls)` put in the wall group.
        let mut hourly = Vec::new();
        for &(a, b) in flows {
            hourly.extend([0.0, a, 0.0, b]);
        }
        Results {
            room_names: vec!["a".into(), "b".into()],
            groups: 4,
            hourly,
            annual_kwh: vec![0.0; 4],
            drift: 0.0,
            converged: true,
            coarse_years: 1,
            hourly_years: 1,
            symmetry: SymmetryInfo {
                symmetry: Symmetry::default(),
                notes: vec![],
            },
            sections: Default::default(),
            history: vec![],
        }
    }

    #[test]
    fn peaks_of_rooms_and_of_the_sum_are_found_with_their_hours() {
        // hour:      0         1         2          3
        let r = results(&[(-10.0, -5.0), (-30.0, 4.0), (2.0, -40.0), (-20.0, -30.0)]);
        let a = r.peaks(Scope::Room(0), None);
        assert_eq!((a.loss.unwrap().watts, a.loss.unwrap().hour), (-30.0, 1));
        assert_eq!((a.gain.unwrap().watts, a.gain.unwrap().hour), (2.0, 2));
        let b = r.peaks(Scope::Room(1), None);
        assert_eq!((b.loss.unwrap().watts, b.loss.unwrap().hour), (-40.0, 2));
        assert_eq!((b.gain.unwrap().watts, b.gain.unwrap().hour), (4.0, 1));
        // Whole basement: sums are -15, -26, -38, -50: the peak of the sum is hour 3 (-50), which is
        // not the sum of the individual peaks (-70). There is no net gain at all.
        let all = r.peaks(Scope::Basement, None);
        assert_eq!(
            (all.loss.unwrap().watts, all.loss.unwrap().hour),
            (-50.0, 3)
        );
        assert!(all.gain.is_none());
        // Floor-only filter: nothing flows through floors here.
        assert!(r.peaks(Scope::Room(0), Some(Kind::Floor)).loss.is_none());
    }

    #[test]
    fn the_report_names_rooms_peaks_dates_and_symmetry() {
        let r = results(&[(-10.0, -5.0), (-30.0, 4.0)]);
        let text = r.report();
        assert!(text.contains("full domain solved") && text.contains("whole basement"));
        assert!(
            text.contains("a: annual")
                && text.contains("peak loss -30 W on Jan 1, hour 2 (01:00-02:00)")
        );
        assert!(text.contains("peak gain 4 W on Jan 1, hour 2"));
    }
}
