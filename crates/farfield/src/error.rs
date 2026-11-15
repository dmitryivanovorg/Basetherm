use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The column or weather input is inconsistent.
    Input(&'static str),
    Periodic(basetherm_periodic::Error),
    Solver(basetherm_conduction::Error),
    Weather(basetherm_surface::Error),
}

impl From<basetherm_periodic::Error> for Error {
    fn from(e: basetherm_periodic::Error) -> Self {
        Self::Periodic(e)
    }
}

impl From<basetherm_conduction::Error> for Error {
    fn from(e: basetherm_conduction::Error) -> Self {
        Self::Solver(e)
    }
}

impl From<basetherm_surface::Error> for Error {
    fn from(e: basetherm_surface::Error) -> Self {
        Self::Weather(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(w) => write!(f, "far-field column input: {w}"),
            Self::Periodic(e) => write!(f, "periodic initialisation: {e}"),
            Self::Solver(e) => write!(f, "column solver: {e}"),
            Self::Weather(e) => write!(f, "weather: {e}"),
        }
    }
}

impl std::error::Error for Error {}
