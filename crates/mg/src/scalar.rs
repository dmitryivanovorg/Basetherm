//! The scalar types the solver runs on: `f64` and a small complex type.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

pub trait Scalar:
    Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
{
    const ZERO: Self;
    fn from_real(x: f64) -> Self;
    /// `re + i im`; real scalars require `im == 0` and ignore it.
    fn from_parts(re: f64, im: f64) -> Self;
    /// Squared modulus, for norms.
    fn abs2(self) -> f64;
}

impl Scalar for f64 {
    const ZERO: Self = 0.0;
    fn from_real(x: f64) -> Self {
        x
    }
    fn from_parts(re: f64, _im: f64) -> Self {
        re
    }
    fn abs2(self) -> f64 {
        self * self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// `self * e^{i phase}` real part: the value at phase angle `phase` (rad).
    pub fn re_at(self, phase: f64) -> f64 {
        self.re * phase.cos() - self.im * phase.sin()
    }
}

impl Add for C64 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for C64 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for C64 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

impl Mul<f64> for C64 {
    type Output = Self;
    fn mul(self, o: f64) -> Self {
        Self::new(self.re * o, self.im * o)
    }
}

impl Div for C64 {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        let d = o.re * o.re + o.im * o.im;
        Self::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}

impl Neg for C64 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.re, -self.im)
    }
}

impl AddAssign for C64 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl SubAssign for C64 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

impl Scalar for C64 {
    const ZERO: Self = Self { re: 0.0, im: 0.0 };
    fn from_real(x: f64) -> Self {
        Self::real(x)
    }
    fn from_parts(re: f64, im: f64) -> Self {
        Self::new(re, im)
    }
    fn abs2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_arithmetic() {
        let (a, b) = (C64::new(1.0, 2.0), C64::new(3.0, -1.0));
        assert_eq!(a * b, C64::new(5.0, 5.0));
        let q = (a * b) / b;
        assert!((q.re - 1.0).abs() < 1e-15 && (q.im - 2.0).abs() < 1e-15);
        // Re(i e^{i pi/2}) = Re(i * i) = -1
        assert!((C64::new(0.0, 1.0).re_at(std::f64::consts::FRAC_PI_2) + 1.0).abs() < 1e-15);
    }
}
