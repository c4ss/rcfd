//! Complex numbers for the advanced tier of the structural solver (ADR 0310 section 2.3, filled by ADR 0440, F-408):
//! [`C64`] with the field operations, Smith's division, a lossless text form, and two vector helpers.
//!
//! The text form is `{re}+{im}i` or `{re}-{|im|}i`, each part written by Rust's `Debug` of `f64` (the shortest decimal that
//! parses back to the same bits), so [`C64::to_string`] and [`str::parse`] round-trip every finite value, signed zeros and
//! the infinities exactly; a NaN parses back as a NaN. `rcfd-post::rotordyn` and `rcfd-post::duct_modes` keep their own
//! types.

use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::str::FromStr;

/// A complex number `re + i im`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct C64 {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl C64 {
    /// Zero.
    pub const ZERO: C64 = C64 { re: 0.0, im: 0.0 };
    /// One.
    pub const ONE: C64 = C64 { re: 1.0, im: 0.0 };
    /// The imaginary unit.
    pub const I: C64 = C64 { re: 0.0, im: 1.0 };

    /// `re + i im`.
    pub const fn new(re: f64, im: f64) -> C64 {
        C64 { re, im }
    }
    /// A real number.
    pub const fn from_real(re: f64) -> C64 {
        C64 { re, im: 0.0 }
    }
    /// `cos theta + i sin theta`.
    pub fn exp_i(theta: f64) -> C64 {
        C64 {
            re: theta.cos(),
            im: theta.sin(),
        }
    }
    /// The conjugate `re - i im`.
    pub fn conj(self) -> C64 {
        C64 {
            re: self.re,
            im: -self.im,
        }
    }
    /// `re^2 + im^2`.
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    /// The modulus, `hypot(re, im)` (no overflow for large parts).
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    /// The argument in `(-pi, pi]`.
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    /// `s z` for a real `s`.
    pub fn scale(self, s: f64) -> C64 {
        C64 {
            re: s * self.re,
            im: s * self.im,
        }
    }
    /// Whether both parts are finite.
    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

impl Add for C64 {
    type Output = C64;
    fn add(self, o: C64) -> C64 {
        C64::new(self.re + o.re, self.im + o.im)
    }
}

impl Sub for C64 {
    type Output = C64;
    fn sub(self, o: C64) -> C64 {
        C64::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for C64 {
    type Output = C64;
    /// `(a c - b d) + i (a d + b c)`.
    fn mul(self, o: C64) -> C64 {
        C64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

impl Div for C64 {
    type Output = C64;
    /// Smith's method (Comm. ACM 5 (1962) 435, from memory; ADR 0440 D9): with `(a + i b) / (c + i d)`, for `|c| >= |d|`
    /// `r = d / c`, `den = c + d r`, the quotient `((a + b r) + i (b - a r)) / den`; otherwise `r = c / d`, `den = c r + d`,
    /// `((a r + b) + i (b r - a)) / den`. It never forms `c^2 + d^2`.
    fn div(self, o: C64) -> C64 {
        let (a, b, c, d) = (self.re, self.im, o.re, o.im);
        if c.abs() >= d.abs() {
            let r = d / c;
            let den = c + d * r;
            C64::new((a + b * r) / den, (b - a * r) / den)
        } else {
            let r = c / d;
            let den = c * r + d;
            C64::new((a * r + b) / den, (b * r - a) / den)
        }
    }
}

impl Neg for C64 {
    type Output = C64;
    fn neg(self) -> C64 {
        C64::new(-self.re, -self.im)
    }
}

impl Mul<f64> for C64 {
    type Output = C64;
    fn mul(self, s: f64) -> C64 {
        C64::new(self.re * s, self.im * s)
    }
}

impl Div<f64> for C64 {
    type Output = C64;
    fn div(self, s: f64) -> C64 {
        C64::new(self.re / s, self.im / s)
    }
}

impl AddAssign for C64 {
    fn add_assign(&mut self, o: C64) {
        *self = *self + o;
    }
}

impl SubAssign for C64 {
    fn sub_assign(&mut self, o: C64) {
        *self = *self - o;
    }
}

impl MulAssign for C64 {
    fn mul_assign(&mut self, o: C64) {
        *self = *self * o;
    }
}

impl DivAssign for C64 {
    fn div_assign(&mut self, o: C64) {
        *self = *self / o;
    }
}

impl fmt::Display for C64 {
    /// `{re:?}+{im:?}i`, or `{re:?}-{|im|:?}i` when the sign bit of `im` is set.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.im.is_sign_negative() {
            write!(f, "{:?}-{:?}i", self.re, -self.im)
        } else {
            write!(f, "{:?}+{:?}i", self.re, self.im)
        }
    }
}

/// A text that is not a complex number in the form of [`C64`]'s `Display`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseC64Error(pub String);

impl fmt::Display for ParseC64Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not a complex number '{}': expected <re>+<im>i or <re>-<im>i",
            self.0
        )
    }
}

impl std::error::Error for ParseC64Error {}

impl FromStr for C64 {
    type Err = ParseC64Error;
    /// The inverse of `Display`: split at the last `+` or `-` that is not the first character and does not follow an `e` or
    /// `E`, the imaginary part ending in `i`.
    fn from_str(s: &str) -> Result<C64, ParseC64Error> {
        let err = || ParseC64Error(s.to_string());
        let body = s.trim().strip_suffix('i').ok_or_else(err)?;
        let bytes = body.as_bytes();
        let split = (1..bytes.len())
            .rev()
            .find(|&k| (bytes[k] == b'+' || bytes[k] == b'-') && !matches!(bytes[k - 1], b'e' | b'E'))
            .ok_or_else(err)?;
        let re: f64 = body[..split].parse().map_err(|_| err())?;
        let im_abs = &body[split + 1..];
        if im_abs.starts_with(['+', '-']) {
            return Err(err());
        }
        let im: f64 = im_abs.parse().map_err(|_| err())?;
        let im = if bytes[split] == b'-' { -im } else { im };
        Ok(C64 { re, im })
    }
}

/// `sum_k x_k y_k` without conjugation, in index order. The slices must have the same length (checked).
pub fn dot_unconj(x: &[C64], y: &[C64]) -> C64 {
    assert_eq!(x.len(), y.len(), "dot_unconj: lengths differ");
    let mut s = C64::ZERO;
    for (a, b) in x.iter().zip(y) {
        s += *a * *b;
    }
    s
}

/// `sqrt(sum_k |x_k|^2)`, the sum in index order.
pub fn norm2(x: &[C64]) -> f64 {
    x.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt()
}
