use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The eight header records are missing or malformed.
    Header(String),
    /// A data line is malformed (1-based line number in the file).
    Line { line: usize, why: String },
    /// A needed value carries the EPW "missing" code.
    Missing { line: usize, field: &'static str },
    /// The file does not hold exactly one 8760-hour year.
    Length { found: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(w) => write!(f, "EPW header: {w}"),
            Self::Line { line, why } => write!(f, "EPW line {line}: {why}"),
            Self::Missing { line, field } => {
                write!(f, "EPW line {line}: {field} is missing (no fallback)")
            }
            Self::Length { found } => write!(f, "EPW must have 8760 hourly records, found {found}"),
        }
    }
}

impl std::error::Error for Error {}
