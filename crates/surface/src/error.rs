use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A parameter is outside its physical range.
    BadParameter(&'static str),
    /// A weather value is missing or unphysical (for example EPW's 9999 "missing" flag).
    BadWeather(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadParameter(w) => write!(f, "bad surface parameter: {w}"),
            Self::BadWeather(w) => write!(f, "bad weather value: {w}"),
        }
    }
}

impl std::error::Error for Error {}
