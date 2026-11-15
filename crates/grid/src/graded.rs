//! Graded axis builder: fine cells at anchor positions, geometric growth to a maximum.

use crate::axis::{Axis, AxisError};

/// Grading parameters. All lengths in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradingSpec {
    /// Cell width at an anchor (surface, wall, floor, insulation edge).
    pub h_min: f64,
    /// Largest allowed cell width.
    pub h_max: f64,
    /// Largest allowed ratio between neighbouring cell widths (> 1).
    pub ratio: f64,
}

impl GradingSpec {
    fn validate(&self) -> Result<(), AxisError> {
        if !(self.h_min > 0.0) || !(self.h_max >= self.h_min) {
            return Err(AxisError::BadGrading("need 0 < h_min <= h_max"));
        }
        if !(self.ratio > 1.0) {
            return Err(AxisError::BadGrading("ratio must exceed 1"));
        }
        Ok(())
    }

    /// Cell widths filling a segment of `length`, finest at each end flagged in `fine`
    /// (`[left, right]`); an unflagged end is free to grow to `h_max`.
    /// Neighbour ratios never exceed `ratio`; widths are scaled down uniformly to fit.
    fn segment_widths(&self, length: f64, fine: [bool; 2]) -> Vec<f64> {
        if length <= self.h_min {
            return vec![length];
        }
        let profile = |n: usize| -> Vec<f64> {
            (0..n)
                .map(|j| {
                    let from_left = if fine[0] {
                        self.h_min * self.ratio.powi(j as i32)
                    } else {
                        self.h_max
                    };
                    let from_right = if fine[1] {
                        self.h_min * self.ratio.powi((n - 1 - j) as i32)
                    } else {
                        self.h_max
                    };
                    from_left.min(from_right).min(self.h_max)
                })
                .collect()
        };
        let mut n = 1;
        while profile(n).iter().sum::<f64>() < length {
            n += 1;
        }
        let w = profile(n);
        let scale = length / w.iter().sum::<f64>();
        w.into_iter().map(|x| x * scale).collect()
    }
}

impl Axis {
    /// Build an axis over `[start, end]` with a face exactly at each anchor
    /// (anchors outside the range are ignored; `start` and `end` are always anchors).
    pub fn graded(
        start: f64,
        end: f64,
        anchors: &[f64],
        spec: GradingSpec,
    ) -> Result<Self, AxisError> {
        Self::graded_ends(start, end, anchors, spec, [true, true])
    }

    /// Like [`Axis::graded`], but an end flagged `false` in `fine_ends` (`[start, end]`) is not
    /// refined: the cells there grow to `h_max` (far-field boundaries, which carry no gradient).
    /// Both ends still get a face.
    pub fn graded_ends(
        start: f64,
        end: f64,
        anchors: &[f64],
        spec: GradingSpec,
        fine_ends: [bool; 2],
    ) -> Result<Self, AxisError> {
        if !(end > start) {
            return Err(AxisError::NonPositiveLength);
        }
        spec.validate()?;
        let mut pts: Vec<f64> = anchors
            .iter()
            .copied()
            .filter(|&a| a > start && a < end)
            .collect();
        pts.push(start);
        pts.push(end);
        pts.sort_by(|a, b| a.partial_cmp(b).expect("anchors must not be NaN"));
        pts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

        let mut faces = vec![start];
        let last = pts.len() - 2;
        for (k, seg) in pts.windows(2).enumerate() {
            let fine = [k > 0 || fine_ends[0], k < last || fine_ends[1]];
            let mut x = seg[0];
            for w in spec.segment_widths(seg[1] - seg[0], fine) {
                x += w;
                faces.push(x);
            }
            *faces.last_mut().expect("non-empty") = seg[1]; // remove round-off drift
        }
        Axis::from_faces(faces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: GradingSpec = GradingSpec {
        h_min: 0.05,
        h_max: 2.0,
        ratio: 1.3,
    };

    #[test]
    fn unrefined_end_grows_to_h_max() {
        let a = Axis::graded_ends(0.0, 40.0, &[0.0], SPEC, [true, false]).unwrap();
        assert!((a.width(a.len() - 1) - SPEC.h_max).abs() < 0.2 * SPEC.h_max);
        assert!(a.max_neighbour_ratio() <= SPEC.ratio + 1e-9);
        let f = Axis::graded(0.0, 40.0, &[0.0], SPEC).unwrap();
        assert!(a.len() < f.len());
        assert!((a.width(0) / f.width(0) - 1.0).abs() < 0.2);
    }

    #[test]
    fn covers_range_and_hits_anchors() {
        let a = Axis::graded(-15.0, 25.0, &[0.0, 3.0, 3.24], SPEC).unwrap();
        assert_eq!(a.start(), -15.0);
        assert_eq!(a.end(), 25.0);
        for anchor in [0.0, 3.0, 3.24] {
            assert!(
                a.faces().iter().any(|&f| (f - anchor).abs() < 1e-9),
                "{anchor}"
            );
        }
    }

    #[test]
    fn respects_ratio_and_max() {
        let a = Axis::graded(0.0, 40.0, &[0.0], SPEC).unwrap();
        assert!(a.max_neighbour_ratio() <= SPEC.ratio + 1e-9);
        assert!((0..a.len()).all(|i| a.width(i) <= SPEC.h_max + 1e-12));
    }

    #[test]
    fn is_fine_at_anchor_and_coarse_far_away() {
        let a = Axis::graded(0.0, 40.0, &[0.0], SPEC).unwrap();
        assert!(a.width(0) <= SPEC.h_min + 1e-12);
        assert!(a.width(a.len() / 2) > 10.0 * a.width(0));
        assert!(a.len() < 80);
    }

    #[test]
    fn short_segment_is_one_cell() {
        let a = Axis::graded(0.0, 0.03, &[], SPEC).unwrap();
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn rejects_bad_spec() {
        let bad = GradingSpec { ratio: 1.0, ..SPEC };
        assert!(Axis::graded(0.0, 1.0, &[], bad).is_err());
        assert!(Axis::graded(1.0, 1.0, &[], SPEC).is_err());
    }
}
