//! Scalar precision: `f64` is the default (`Real`), kernels may be generic
//! over [`Scalar`] to allow `f32`.

use std::fmt::Debug;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Default working precision.
pub type Real = f64;

/// Minimal floating point abstraction implemented for `f32` and `f64`.
pub trait Scalar:
    Copy
    + Debug
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + Send
    + Sync
    + 'static
{
    /// Additive identity.
    const ZERO: Self;
    /// Multiplicative identity.
    const ONE: Self;
    /// Machine epsilon.
    const EPS: Self;
    /// Tiny positive number used to guard divisions.
    const SMALL: Self;
    /// Convert from `f64` (rounding for `f32`).
    fn from_f64(v: f64) -> Self;
    /// Widen to `f64`.
    fn to_f64(self) -> f64;
    /// Square root.
    fn sqrt(self) -> Self;
    /// Absolute value.
    fn abs(self) -> Self;
    /// Maximum of two values.
    fn maxs(self, o: Self) -> Self {
        if self > o {
            self
        } else {
            o
        }
    }
    /// Minimum of two values.
    fn mins(self, o: Self) -> Self {
        if self < o {
            self
        } else {
            o
        }
    }
}

macro_rules! impl_scalar {
    ($t:ty, $small:expr) => {
        impl Scalar for $t {
            const ZERO: Self = 0.0;
            const ONE: Self = 1.0;
            const EPS: Self = <$t>::EPSILON;
            const SMALL: Self = $small;
            fn from_f64(v: f64) -> Self {
                v as $t
            }
            fn to_f64(self) -> f64 {
                self as f64
            }
            fn sqrt(self) -> Self {
                <$t>::sqrt(self)
            }
            fn abs(self) -> Self {
                <$t>::abs(self)
            }
        }
    };
}
impl_scalar!(f64, 1e-300);
impl_scalar!(f32, 1e-30);

/// Dot product written generically, as an example of precision-generic code.
pub fn dot<S: Scalar>(a: &[S], b: &[S]) -> S {
    a.iter().zip(b).fold(S::ZERO, |acc, (x, y)| acc + *x * *y)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn norm<S: Scalar>(a: &[S]) -> S {
        dot(a, a).sqrt()
    }
    #[test]
    fn generic_f32_and_f64() {
        assert!((norm(&[3.0f64, 4.0]) - 5.0).abs() < 1e-14);
        assert!((norm(&[3.0f32, 4.0]) - 5.0).abs() < 1e-6);
        assert_eq!(f32::from_f64(0.5).to_f64(), 0.5);
        assert_eq!(Scalar::maxs(1.0f64, 2.0), 2.0);
        assert!(<f32 as Scalar>::EPS > <f64 as Scalar>::EPS as f32);
    }
}
