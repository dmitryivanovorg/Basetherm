//! EPW parser (dry-bulb, wind, GHI, horizontal IR), hourly series, daily means, annual harmonic.
//!
//! Parsed in the browser or natively from text; nothing is uploaded. Hour `i` (0-based) covers
//! `[i, i+1]` hours from Jan 1 00:00. Missing values are errors: there is no fallback for missing
//! infrared (the sky temperature has no verified substitute formula yet).

mod error;
mod parse;
mod stats;

pub use error::Error;
pub use parse::{parse, Epw, Location, Record, HOURS_PER_YEAR};
pub use stats::{annual_harmonic, daily_means, Harmonic};
