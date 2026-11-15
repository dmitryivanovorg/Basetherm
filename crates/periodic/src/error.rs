use basetherm_mg::Outcome;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Conduction(basetherm_conduction::Error),
    /// No boundary conducts, so the mean temperature is undetermined.
    NotAnchored,
    BadParams(&'static str),
    NotConverged {
        which: &'static str,
        outcome: Outcome,
    },
}

impl From<basetherm_conduction::Error> for Error {
    fn from(e: basetherm_conduction::Error) -> Self {
        Self::Conduction(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conduction(e) => write!(f, "{e}"),
            Self::NotAnchored => write!(f, "no boundary conducts: the mean field is undetermined"),
            Self::BadParams(why) => write!(f, "bad parameters: {why}"),
            Self::NotConverged { which, outcome } => write!(
                f,
                "{which} solve did not converge: {} iterations, residual {:.2e}",
                outcome.iterations, outcome.residual
            ),
        }
    }
}

impl std::error::Error for Error {}
