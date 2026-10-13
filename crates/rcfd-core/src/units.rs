//! SI-based units and quantities with runtime dimension checking.
//!
//! All internal values are stored in coherent SI base units. Unit strings
//! such as `"km/h"`, `"kg/m^3"`, `"W/(m^2*K)"`, `"degC"` or `"mPa*s"` are
//! parsed into a conversion factor, an optional offset (temperature scales)
//! and a [`Dimension`].

use crate::error::{Error, Result};
use std::fmt;
use std::ops::{Div, Mul};

/// Exponents of the seven SI base dimensions: m, kg, s, K, A, mol, cd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dimension(pub [i8; 7]);

const NAMES: [&str; 7] = ["m", "kg", "s", "K", "A", "mol", "cd"];

impl Dimension {
    /// Dimensionless.
    pub const NONE: Dimension = Dimension([0; 7]);
    /// Length.
    pub const LENGTH: Dimension = Dimension([1, 0, 0, 0, 0, 0, 0]);
    /// Mass.
    pub const MASS: Dimension = Dimension([0, 1, 0, 0, 0, 0, 0]);
    /// Time.
    pub const TIME: Dimension = Dimension([0, 0, 1, 0, 0, 0, 0]);
    /// Thermodynamic temperature.
    pub const TEMPERATURE: Dimension = Dimension([0, 0, 0, 1, 0, 0, 0]);
    /// Amount of substance.
    pub const AMOUNT: Dimension = Dimension([0, 0, 0, 0, 0, 1, 0]);
    /// Velocity, m/s.
    pub const VELOCITY: Dimension = Dimension([1, 0, -1, 0, 0, 0, 0]);
    /// Acceleration, m/s^2.
    pub const ACCELERATION: Dimension = Dimension([1, 0, -2, 0, 0, 0, 0]);
    /// Force, N.
    pub const FORCE: Dimension = Dimension([1, 1, -2, 0, 0, 0, 0]);
    /// Pressure, Pa.
    pub const PRESSURE: Dimension = Dimension([-1, 1, -2, 0, 0, 0, 0]);
    /// Density, kg/m^3.
    pub const DENSITY: Dimension = Dimension([-3, 1, 0, 0, 0, 0, 0]);
    /// Dynamic viscosity, Pa s.
    pub const DYNAMIC_VISCOSITY: Dimension = Dimension([-1, 1, -1, 0, 0, 0, 0]);
    /// Kinematic viscosity, m^2/s.
    pub const KINEMATIC_VISCOSITY: Dimension = Dimension([2, 0, -1, 0, 0, 0, 0]);
    /// Energy, J.
    pub const ENERGY: Dimension = Dimension([2, 1, -2, 0, 0, 0, 0]);
    /// Power, W.
    pub const POWER: Dimension = Dimension([2, 1, -3, 0, 0, 0, 0]);
    /// Specific heat capacity, J/(kg K).
    pub const SPECIFIC_HEAT: Dimension = Dimension([2, 0, -2, -1, 0, 0, 0]);
    /// Thermal conductivity, W/(m K).
    pub const CONDUCTIVITY: Dimension = Dimension([1, 1, -3, -1, 0, 0, 0]);
    /// Heat flux, W/m^2.
    pub const HEAT_FLUX: Dimension = Dimension([0, 1, -3, 0, 0, 0, 0]);
    /// Angular velocity, rad/s (dimension 1/s).
    pub const ANGULAR_VELOCITY: Dimension = Dimension([0, 0, -1, 0, 0, 0, 0]);

    /// Raise to an integer power.
    pub fn powi(self, n: i8) -> Dimension {
        let mut d = self.0;
        for e in d.iter_mut() {
            *e *= n;
        }
        Dimension(d)
    }

    /// Whether all exponents are zero.
    pub fn is_dimensionless(&self) -> bool {
        *self == Dimension::NONE
    }
}

#[allow(clippy::suspicious_arithmetic_impl)] // exponents add when dimensions multiply
impl Mul for Dimension {
    type Output = Dimension;
    fn mul(self, o: Dimension) -> Dimension {
        let mut d = self.0;
        for (a, b) in d.iter_mut().zip(o.0) {
            *a += b;
        }
        Dimension(d)
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl Div for Dimension {
    type Output = Dimension;
    fn div(self, o: Dimension) -> Dimension {
        self * o.powi(-1)
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_dimensionless() {
            return write!(f, "1");
        }
        let mut first = true;
        for (n, e) in NAMES.iter().zip(self.0) {
            if e == 0 {
                continue;
            }
            if !first {
                write!(f, " ")?;
            }
            first = false;
            if e == 1 {
                write!(f, "{n}")?;
            } else {
                write!(f, "{n}^{e}")?;
            }
        }
        Ok(())
    }
}

/// A resolved unit: `si = value * factor + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    /// Multiplicative factor to SI.
    pub factor: f64,
    /// Additive offset to SI (non-zero only for temperature scales).
    pub offset: f64,
    /// Dimension.
    pub dim: Dimension,
}

impl Unit {
    /// Convert a value expressed in this unit to SI.
    pub fn to_si(&self, v: f64) -> f64 {
        v * self.factor + self.offset
    }
    /// Convert an SI value to this unit.
    pub fn from_si(&self, x: f64) -> f64 {
        (x - self.offset) / self.factor
    }

    /// Parse a unit expression such as `"kg/m^3"` or `"W/(m^2*K)"`.
    pub fn parse(s: &str) -> Result<Unit> {
        let mut p = Parser {
            s: s.as_bytes(),
            i: 0,
            src: s,
        };
        let u = p.expr()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(p.err("unexpected trailing characters"));
        }
        Ok(u)
    }
}

const PI: f64 = std::f64::consts::PI;

const fn u(factor: f64, dim: [i8; 7]) -> (f64, f64, Dimension) {
    (factor, 0.0, Dimension(dim))
}

/// (symbol, factor, offset, dim, accepts SI prefix)
fn table(sym: &str) -> Option<((f64, f64, Dimension), bool)> {
    let r = match sym {
        "1" | "-" => (u(1.0, [0; 7]), false),
        "m" => (u(1.0, [1, 0, 0, 0, 0, 0, 0]), true),
        "g" => (u(1e-3, [0, 1, 0, 0, 0, 0, 0]), true),
        "s" => (u(1.0, [0, 0, 1, 0, 0, 0, 0]), true),
        "K" => (u(1.0, [0, 0, 0, 1, 0, 0, 0]), true),
        "A" => (u(1.0, [0, 0, 0, 0, 1, 0, 0]), true),
        "mol" => (u(1.0, [0, 0, 0, 0, 0, 1, 0]), true),
        "cd" => (u(1.0, [0, 0, 0, 0, 0, 0, 1]), false),
        "N" => (u(1.0, [1, 1, -2, 0, 0, 0, 0]), true),
        "Pa" => (u(1.0, [-1, 1, -2, 0, 0, 0, 0]), true),
        "J" => (u(1.0, [2, 1, -2, 0, 0, 0, 0]), true),
        "W" => (u(1.0, [2, 1, -3, 0, 0, 0, 0]), true),
        "Hz" => (u(1.0, [0, 0, -1, 0, 0, 0, 0]), true),
        "P" => (u(0.1, [-1, 1, -1, 0, 0, 0, 0]), true),  // poise
        "St" => (u(1e-4, [2, 0, -1, 0, 0, 0, 0]), true), // stokes
        "L" => (u(1e-3, [3, 0, 0, 0, 0, 0, 0]), true),
        "bar" => (u(1e5, [-1, 1, -2, 0, 0, 0, 0]), true),
        "atm" => (u(101_325.0, [-1, 1, -2, 0, 0, 0, 0]), false),
        "psi" => (u(6_894.757_293_168, [-1, 1, -2, 0, 0, 0, 0]), false),
        "min" => (u(60.0, [0, 0, 1, 0, 0, 0, 0]), false),
        "h" => (u(3600.0, [0, 0, 1, 0, 0, 0, 0]), false),
        "day" => (u(86_400.0, [0, 0, 1, 0, 0, 0, 0]), false),
        "in" => (u(0.0254, [1, 0, 0, 0, 0, 0, 0]), false),
        "ft" => (u(0.3048, [1, 0, 0, 0, 0, 0, 0]), false),
        "lb" => (u(0.453_592_37, [0, 1, 0, 0, 0, 0, 0]), false),
        "lbf" => (u(4.448_221_615_260_5, [1, 1, -2, 0, 0, 0, 0]), false),
        "cal" => (u(4.184, [2, 1, -2, 0, 0, 0, 0]), true),
        "rad" => (u(1.0, [0; 7]), false),
        "deg" => (u(PI / 180.0, [0; 7]), false),
        "rpm" => (u(2.0 * PI / 60.0, [0, 0, -1, 0, 0, 0, 0]), false),
        "degC" | "C" => ((1.0, 273.15, Dimension::TEMPERATURE), false),
        "degF" | "F" => ((5.0 / 9.0, 459.67 * 5.0 / 9.0, Dimension::TEMPERATURE), false),
        "degR" => (u(5.0 / 9.0, [0, 0, 0, 1, 0, 0, 0]), false),
        _ => return None,
    };
    Some(r)
}

fn prefix(c: &str) -> Option<f64> {
    Some(match c {
        "n" => 1e-9,
        "u" | "\u{b5}" => 1e-6,
        "m" => 1e-3,
        "c" => 1e-2,
        "d" => 1e-1,
        "h" => 1e2,
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        _ => return None,
    })
}

fn lookup(sym: &str) -> Option<(f64, f64, Dimension)> {
    if let Some((e, _)) = table(sym) {
        return Some(e);
    }
    let mut chars = sym.chars();
    let first = chars.next()?;
    let rest = chars.as_str();
    let pf = prefix(&sym[..first.len_utf8()])?;
    match table(rest) {
        Some((e, true)) => Some((e.0 * pf, e.1, e.2)),
        _ => None,
    }
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    src: &'a str,
}

impl Parser<'_> {
    fn err(&self, why: &str) -> Error {
        Error::UnitParse(format!("'{}': {} at position {}", self.src, why, self.i))
    }
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i] == b' ' {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    // expr := term (('*' | '/' | '-'?) term)* ; '-' is not an operator, use * and /.
    fn expr(&mut self) -> Result<Unit> {
        let mut acc = self.factor()?;
        loop {
            match self.peek() {
                Some(b'*') | Some(b'.') => {
                    self.i += 1;
                    let r = self.factor()?;
                    acc = self.combine(acc, r, false)?;
                }
                Some(b'/') => {
                    self.i += 1;
                    let r = self.factor()?;
                    acc = self.combine(acc, r, true)?;
                }
                _ => return Ok(acc),
            }
        }
    }
    fn combine(&self, a: Unit, b: Unit, div: bool) -> Result<Unit> {
        if a.offset != 0.0 || b.offset != 0.0 {
            return Err(self.err("offset temperature units (degC/degF) cannot be combined; use K"));
        }
        Ok(if div {
            Unit {
                factor: a.factor / b.factor,
                offset: 0.0,
                dim: a.dim / b.dim,
            }
        } else {
            Unit {
                factor: a.factor * b.factor,
                offset: 0.0,
                dim: a.dim * b.dim,
            }
        })
    }
    fn factor(&mut self) -> Result<Unit> {
        let base = if self.peek() == Some(b'(') {
            self.i += 1;
            let inner = self.expr()?;
            if self.peek() != Some(b')') {
                return Err(self.err("missing ')'"));
            }
            self.i += 1;
            inner
        } else {
            let start = self.i;
            while self.i < self.s.len()
                && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] >= 0x80 || self.s[self.i] == b'%')
            {
                self.i += 1;
            }
            if start == self.i {
                return Err(self.err("expected unit symbol"));
            }
            let sym = &self.src[start..self.i];
            let (f, o, d) = lookup(sym).ok_or_else(|| self.err(&format!("unknown unit '{sym}'")))?;
            Unit {
                factor: f,
                offset: o,
                dim: d,
            }
        };
        if self.peek() == Some(b'^') {
            self.i += 1;
            self.ws();
            let start = self.i;
            if self.s.get(self.i) == Some(&b'-') {
                self.i += 1;
            }
            while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                self.i += 1;
            }
            let n: i8 = self.src[start..self.i].parse().map_err(|_| self.err("bad exponent"))?;
            if base.offset != 0.0 {
                return Err(self.err("cannot raise an offset unit to a power"));
            }
            return Ok(Unit {
                factor: base.factor.powi(n as i32),
                offset: 0.0,
                dim: base.dim.powi(n),
            });
        }
        Ok(base)
    }
}

/// A physical quantity stored in SI base units together with its dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    /// Value in coherent SI units.
    pub si: f64,
    /// Physical dimension.
    pub dim: Dimension,
}

impl Quantity {
    /// Build directly from an SI value.
    pub fn from_si(si: f64, dim: Dimension) -> Self {
        Quantity { si, dim }
    }

    /// Build from a value in the given unit string.
    pub fn new(value: f64, unit: &str) -> Result<Self> {
        let u = Unit::parse(unit)?;
        Ok(Quantity {
            si: u.to_si(value),
            dim: u.dim,
        })
    }

    /// Parse `"<number> <unit>"`, e.g. `"10 km/h"`; a bare number is dimensionless.
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        let split = s
            .char_indices()
            .find(|(i, c)| {
                c.is_alphabetic() && !(matches!(*c, 'e' | 'E') && is_exponent(&s[..*i], &s[*i + 1..]))
                    || *c == '('
                    || *c == '%'
            })
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        let (num, unit) = s.split_at(split);
        let value: f64 = num
            .trim()
            .parse()
            .map_err(|_| Error::UnitParse(format!("'{s}': cannot read number '{}'", num.trim())))?;
        let unit = unit.trim();
        if unit.is_empty() {
            Ok(Quantity {
                si: value,
                dim: Dimension::NONE,
            })
        } else {
            Quantity::new(value, unit)
        }
    }

    /// Express the quantity in `unit`, checking the dimension.
    pub fn to(&self, unit: &str) -> Result<f64> {
        let u = Unit::parse(unit)?;
        if u.dim != self.dim {
            return Err(Error::DimensionMismatch {
                expected: u.dim,
                found: self.dim,
            });
        }
        Ok(u.from_si(self.si))
    }

    /// Return the SI value if the dimension equals `expected`, else an error.
    pub fn expect(&self, expected: Dimension) -> Result<f64> {
        if self.dim != expected {
            return Err(Error::DimensionMismatch {
                expected,
                found: self.dim,
            });
        }
        Ok(self.si)
    }

    /// Dimension-checked addition.
    pub fn checked_add(self, o: Quantity) -> Result<Quantity> {
        if self.dim != o.dim {
            return Err(Error::DimensionMismatch {
                expected: self.dim,
                found: o.dim,
            });
        }
        Ok(Quantity {
            si: self.si + o.si,
            dim: self.dim,
        })
    }

    /// Dimension-checked subtraction.
    pub fn checked_sub(self, o: Quantity) -> Result<Quantity> {
        if self.dim != o.dim {
            return Err(Error::DimensionMismatch {
                expected: self.dim,
                found: o.dim,
            });
        }
        Ok(Quantity {
            si: self.si - o.si,
            dim: self.dim,
        })
    }
}

fn is_exponent(before: &str, after: &str) -> bool {
    // 'e' belongs to a number if preceded by a digit/dot and followed by digit or sign+digit.
    let prev_ok = before.chars().last().is_some_and(|c| c.is_ascii_digit() || c == '.');
    let mut it = after.chars();
    let next = it.next();
    let ok = match next {
        Some(c) if c.is_ascii_digit() => true,
        Some('+') | Some('-') => it.next().is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    };
    prev_ok && ok
}

impl Mul for Quantity {
    type Output = Quantity;
    fn mul(self, o: Quantity) -> Quantity {
        Quantity {
            si: self.si * o.si,
            dim: self.dim * o.dim,
        }
    }
}

impl Div for Quantity {
    type Output = Quantity;
    fn div(self, o: Quantity) -> Quantity {
        Quantity {
            si: self.si / o.si,
            dim: self.dim / o.dim,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    #[test]
    fn basic_conversions() {
        assert!(close(Quantity::new(36.0, "km/h").unwrap().si, 10.0));
        assert!(close(Quantity::new(1.0, "bar").unwrap().to("Pa").unwrap(), 1e5));
        assert!(close(Quantity::new(1.0, "atm").unwrap().to("kPa").unwrap(), 101.325));
        assert!(close(Quantity::new(1.0, "ft").unwrap().to("in").unwrap(), 12.0));
        assert!(close(Quantity::new(1.0, "cP").unwrap().to("Pa*s").unwrap(), 1e-3));
        assert!(close(
            Quantity::new(60.0, "rpm").unwrap().to("rad/s").unwrap(),
            2.0 * PI
        ));
        assert!(close(Quantity::new(180.0, "deg").unwrap().si, PI));
        assert!(close(Quantity::new(1.0, "mm").unwrap().si, 1e-3));
        assert!(close(Quantity::new(2.0, "kg").unwrap().to("g").unwrap(), 2000.0));
    }

    #[test]
    fn temperature_scales() {
        assert!(close(Quantity::new(20.0, "degC").unwrap().si, 293.15));
        assert!(close(Quantity::new(32.0, "degF").unwrap().to("degC").unwrap(), 0.0));
        assert!(close(Quantity::new(212.0, "degF").unwrap().to("degC").unwrap(), 100.0));
        assert!(close(Quantity::new(0.0, "degC").unwrap().to("K").unwrap(), 273.15));
        assert!(Unit::parse("degC/s").is_err());
    }

    #[test]
    fn compound_and_dimensions() {
        let rho = Quantity::new(1.2, "kg/m^3").unwrap();
        assert_eq!(rho.dim, Dimension::DENSITY);
        let k = Quantity::new(0.026, "W/(m*K)").unwrap();
        assert_eq!(k.dim, Dimension::CONDUCTIVITY);
        let h = Quantity::new(10.0, "W/m^2/K").unwrap();
        assert_eq!(h.dim, Dimension::HEAT_FLUX / Dimension::TEMPERATURE);
        let cp = Quantity::new(1005.0, "J/kg/K").unwrap();
        assert_eq!(cp.dim, Dimension::SPECIFIC_HEAT);
        assert_eq!(Quantity::new(1.0, "m^2/s").unwrap().dim, Dimension::KINEMATIC_VISCOSITY);
        // nu = mu / rho
        let mu = Quantity::new(1.8e-5, "Pa*s").unwrap();
        assert_eq!((mu / rho).dim, Dimension::KINEMATIC_VISCOSITY);
        // pressure = rho v^2
        let v = Quantity::new(10.0, "m/s").unwrap();
        assert_eq!((rho * v * v).dim, Dimension::PRESSURE);
    }

    #[test]
    fn dimension_checking() {
        let v = Quantity::new(1.0, "m/s").unwrap();
        let p = Quantity::new(1.0, "Pa").unwrap();
        assert!(matches!(v.checked_add(p), Err(Error::DimensionMismatch { .. })));
        assert!(matches!(v.to("Pa"), Err(Error::DimensionMismatch { .. })));
        assert!(v.expect(Dimension::VELOCITY).is_ok());
        assert!(v.expect(Dimension::PRESSURE).is_err());
        assert!(v.checked_add(v).unwrap().si == 2.0);
    }

    #[test]
    fn parsing_quantities_and_errors() {
        assert!(close(
            Quantity::parse("10 km/h").unwrap().to("m/s").unwrap(),
            10.0 / 3.6
        ));
        assert!(close(Quantity::parse("1.5e3 Pa").unwrap().si, 1500.0));
        assert!(close(Quantity::parse("300K").unwrap().si, 300.0));
        let n = Quantity::parse("0.5").unwrap();
        assert!(n.dim.is_dimensionless());
        assert!(Quantity::new(1.0, "furlong").is_err());
        assert!(Quantity::new(1.0, "m/").is_err());
        assert!(Quantity::new(1.0, "(m").is_err());
        assert!(Quantity::parse("abc m").is_err());
    }

    #[test]
    fn dimension_display() {
        assert_eq!(Dimension::PRESSURE.to_string(), "m^-1 kg s^-2");
        assert_eq!(Dimension::NONE.to_string(), "1");
    }
}
