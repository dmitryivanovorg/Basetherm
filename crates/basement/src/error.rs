use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The building description is inconsistent.
    Spec(String),
    Mesh(String),
    /// A solver or far-field failure while running.
    Run(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spec(w) => write!(f, "building spec: {w}"),
            Self::Mesh(w) => write!(f, "mesh: {w}"),
            Self::Run(w) => write!(f, "run: {w}"),
        }
    }
}

impl std::error::Error for Error {}
