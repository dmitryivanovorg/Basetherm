use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// An array does not match the grid cell count.
    WrongLength {
        what: &'static str,
        expected: usize,
        got: usize,
    },
    /// A property is not a positive finite number.
    BadProperty { what: &'static str, index: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength {
                what,
                expected,
                got,
            } => {
                write!(f, "{what}: expected {expected} values, got {got}")
            }
            Self::BadProperty { what, index } => {
                write!(f, "{what} must be positive and finite (cell {index})")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Check that `v` has `n` entries, all positive and finite.
pub fn check_property(what: &'static str, v: &[f64], n: usize) -> Result<(), Error> {
    if v.len() != n {
        return Err(Error::WrongLength {
            what,
            expected: n,
            got: v.len(),
        });
    }
    match v.iter().position(|&x| !(x.is_finite() && x > 0.0)) {
        Some(index) => Err(Error::BadProperty { what, index }),
        None => Ok(()),
    }
}
