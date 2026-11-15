//! One grid axis: monotonically increasing cell faces, in metres.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum AxisError {
    TooFewFaces,
    NotIncreasing { index: usize },
    NonPositiveLength,
    BadGrading(&'static str),
}

impl fmt::Display for AxisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewFaces => write!(f, "an axis needs at least two faces"),
            Self::NotIncreasing { index } => write!(f, "faces not increasing at index {index}"),
            Self::NonPositiveLength => write!(f, "axis length must be positive"),
            Self::BadGrading(why) => write!(f, "invalid grading: {why}"),
        }
    }
}

impl std::error::Error for AxisError {}

/// Cell faces `x[0] < x[1] < ... < x[n]`; there are `n` cells.
/// A collapsed dimension (1D/2D runs) is an axis with a single cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    faces: Vec<f64>,
}

impl Axis {
    pub fn from_faces(faces: Vec<f64>) -> Result<Self, AxisError> {
        if faces.len() < 2 {
            return Err(AxisError::TooFewFaces);
        }
        if let Some(i) = faces.windows(2).position(|w| !(w[1] > w[0])) {
            return Err(AxisError::NotIncreasing { index: i + 1 });
        }
        Ok(Self { faces })
    }

    pub fn uniform(start: f64, length: f64, cells: usize) -> Result<Self, AxisError> {
        if !(length > 0.0) || cells == 0 {
            return Err(AxisError::NonPositiveLength);
        }
        let h = length / cells as f64;
        let mut faces: Vec<f64> = (0..cells).map(|i| start + h * i as f64).collect();
        faces.push(start + length);
        Self::from_faces(faces)
    }

    /// A single cell of the given width (a collapsed dimension).
    pub fn collapsed(width: f64) -> Result<Self, AxisError> {
        Self::uniform(0.0, width, 1)
    }

    pub fn len(&self) -> usize {
        self.faces.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn faces(&self) -> &[f64] {
        &self.faces
    }

    pub fn start(&self) -> f64 {
        self.faces[0]
    }

    pub fn end(&self) -> f64 {
        self.faces[self.faces.len() - 1]
    }

    pub fn width(&self, i: usize) -> f64 {
        self.faces[i + 1] - self.faces[i]
    }

    pub fn center(&self, i: usize) -> f64 {
        0.5 * (self.faces[i] + self.faces[i + 1])
    }

    /// Index of the cell containing `x`; the end face belongs to the last cell.
    pub fn locate(&self, x: f64) -> Option<usize> {
        if x < self.start() || x > self.end() {
            return None;
        }
        let i = self.faces.partition_point(|&f| f <= x);
        Some(i.saturating_sub(1).min(self.len() - 1))
    }

    /// Largest ratio between neighbouring cell widths (1.0 for a single cell).
    pub fn max_neighbour_ratio(&self) -> f64 {
        (1..self.len())
            .map(|i| {
                let (a, b) = (self.width(i - 1), self.width(i));
                (a / b).max(b / a)
            })
            .fold(1.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_faces() {
        assert_eq!(Axis::from_faces(vec![0.0]), Err(AxisError::TooFewFaces));
        assert_eq!(
            Axis::from_faces(vec![0.0, 1.0, 1.0]),
            Err(AxisError::NotIncreasing { index: 2 })
        );
        assert!(Axis::from_faces(vec![0.0, f64::NAN]).is_err());
    }

    #[test]
    fn uniform_widths_and_centers() {
        let a = Axis::uniform(1.0, 2.0, 4).unwrap();
        assert_eq!(a.len(), 4);
        assert!((a.width(2) - 0.5).abs() < 1e-15);
        assert!((a.center(0) - 1.25).abs() < 1e-15);
        assert_eq!(a.end(), 3.0);
    }

    #[test]
    fn locate_handles_edges() {
        let a = Axis::uniform(0.0, 1.0, 4).unwrap();
        assert_eq!(a.locate(-0.1), None);
        assert_eq!(a.locate(0.0), Some(0));
        assert_eq!(a.locate(0.25), Some(1));
        assert_eq!(a.locate(1.0), Some(3));
        assert_eq!(a.locate(1.1), None);
    }

    #[test]
    fn collapsed_is_one_cell() {
        let a = Axis::collapsed(1.0).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a.max_neighbour_ratio(), 1.0);
    }
}
