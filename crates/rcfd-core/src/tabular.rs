//! Tabulated and piecewise material properties (F-220, ADR 0036).
//!
//! * [`PiecewiseLinear`] and [`PiecewisePoly`]: one-variable laws with located construction errors, a monotonicity report and an
//!   extrapolation policy.
//! * [`Vogel`]: `a exp(b / (T - c))` (Vogel-Fulcher-Tammann; `c = 0` is the Arrhenius/Andrade form).
//! * [`Tait`]: compressible-liquid density, `1 - rho_ref/rho = c ln((b + p)/(b + p_ref))`.
//! * [`PtTable`]: pressure-temperature tables with a saturation line separating a liquid and a vapour grid.
//!
//! No property data is shipped; tables are inputs. Everything is written from the definitions of linear/bilinear interpolation and
//! the closed forms above.

use crate::error::{Error, Result};

fn bad<T>(path: &str, reason: String) -> Result<T> {
    Err(Error::InvalidValue {
        path: path.into(),
        reason,
    })
}

/// What a law does outside its table range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Extrapolation {
    /// Hold the end value (default).
    #[default]
    Clamp,
    /// Continue the end segment.
    Linear,
    /// Out of range is an error (`eval` gives NaN, `try_eval` an `Err`).
    Error,
}

impl Extrapolation {
    /// Parse `"clamp"`, `"linear"`, `"error"`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "clamp" => Some(Self::Clamp),
            "linear" => Some(Self::Linear),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Monotonic character of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Monotonicity {
    /// Strictly increasing.
    Increasing,
    /// Strictly decreasing.
    Decreasing,
    /// Non-decreasing with at least one flat step and one rise.
    NonDecreasing,
    /// Non-increasing with at least one flat step and one fall.
    NonIncreasing,
    /// All values equal.
    Constant,
    /// Neither; the index is the first `i` where the direction changes (the step `y[i-1] -> y[i]`).
    None(usize),
}

fn check_axis(name: &str, v: &[f64]) -> Result<()> {
    for (i, x) in v.iter().enumerate() {
        if !x.is_finite() {
            return bad(&format!("{name}[{i}]"), format!("{name}[{i}] = {x} is not finite"));
        }
    }
    for i in 1..v.len() {
        if v[i] <= v[i - 1] {
            return bad(
                &format!("{name}[{i}]"),
                format!(
                    "{name}[{i}] = {} must exceed {name}[{}] = {} (strictly increasing)",
                    v[i],
                    i - 1,
                    v[i - 1]
                ),
            );
        }
    }
    Ok(())
}

/// Piecewise-linear interpolant of `y(x)`.
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewiseLinear {
    x: Vec<f64>,
    y: Vec<f64>,
    /// Out-of-range policy.
    pub extrapolation: Extrapolation,
}

impl PiecewiseLinear {
    /// Build and validate.
    pub fn new(x: Vec<f64>, y: Vec<f64>, extrapolation: Extrapolation) -> Result<Self> {
        if x.len() != y.len() {
            return bad(
                "values",
                format!("x has {} entries but y has {}; give one value per x", x.len(), y.len()),
            );
        }
        if x.len() < 2 {
            return bad("x", "a piecewise-linear table needs at least 2 points".into());
        }
        check_axis("x", &x)?;
        for (i, v) in y.iter().enumerate() {
            if !v.is_finite() {
                return bad(&format!("y[{i}]"), format!("y[{i}] = {v} is not finite"));
            }
        }
        Ok(Self { x, y, extrapolation })
    }

    /// Abscissae.
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// Ordinates.
    pub fn y(&self) -> &[f64] {
        &self.y
    }

    /// Range `(x_first, x_last)`.
    pub fn range(&self) -> (f64, f64) {
        (self.x[0], self.x[self.x.len() - 1])
    }

    /// Index `i` of the segment `[x_i, x_{i+1}]` containing `x` (clamped to the end segments).
    fn segment(&self, x: f64) -> usize {
        let n = self.x.len();
        match self.x.partition_point(|&v| v <= x) {
            0 => 0,
            k if k >= n => n - 2,
            k => k - 1,
        }
    }

    /// Value at `x`; NaN for NaN input and, with `Extrapolation::Error`, outside the range.
    pub fn eval(&self, x: f64) -> f64 {
        let (lo, hi) = self.range();
        let n = self.x.len();
        if x.is_nan() {
            return f64::NAN;
        }
        if x < lo || x > hi {
            return match self.extrapolation {
                Extrapolation::Clamp => {
                    if x < lo {
                        self.y[0]
                    } else {
                        self.y[n - 1]
                    }
                }
                Extrapolation::Error => f64::NAN,
                Extrapolation::Linear => {
                    let i = if x < lo { 0 } else { n - 2 };
                    self.lerp(i, x)
                }
            };
        }
        if x == hi {
            return self.y[n - 1];
        }
        self.lerp(self.segment(x), x)
    }

    fn lerp(&self, i: usize, x: f64) -> f64 {
        let (x0, x1, y0, y1) = (self.x[i], self.x[i + 1], self.y[i], self.y[i + 1]);
        if x == x0 {
            return y0;
        }
        y0 + (y1 - y0) * ((x - x0) / (x1 - x0))
    }

    /// Like [`eval`](Self::eval), but an out-of-range query with `Extrapolation::Error` is an `Err`.
    pub fn try_eval(&self, x: f64) -> Result<f64> {
        let v = self.eval(x);
        if v.is_nan() {
            let (lo, hi) = self.range();
            return bad("x", format!("x = {x} is outside the table range [{lo}, {hi}]"));
        }
        Ok(v)
    }

    /// Monotonic character of `y`.
    pub fn monotonicity(&self) -> Monotonicity {
        monotonicity_of(&self.y)
    }

    /// Error unless `y` is monotonic (strictly when `strict`); the message names the first violating index.
    pub fn require_monotonic(&self, strict: bool) -> Result<()> {
        match (self.monotonicity(), strict) {
            (Monotonicity::Increasing | Monotonicity::Decreasing, _) => Ok(()),
            (Monotonicity::NonDecreasing | Monotonicity::NonIncreasing | Monotonicity::Constant, false) => Ok(()),
            (Monotonicity::None(i), _) => bad(
                &format!("y[{i}]"),
                format!("y is not monotonic: the direction changes at y[{i}] = {}", self.y[i]),
            ),
            _ => {
                let i = (1..self.y.len()).find(|&i| self.y[i] == self.y[i - 1]).unwrap_or(1);
                bad(
                    &format!("y[{i}]"),
                    format!("y is not strictly monotonic: y[{i}] equals y[{}]", i - 1),
                )
            }
        }
    }
}

fn monotonicity_of(y: &[f64]) -> Monotonicity {
    let (mut up, mut down, mut flat) = (false, false, false);
    for i in 1..y.len() {
        if y[i] > y[i - 1] {
            up = true;
        } else if y[i] < y[i - 1] {
            down = true;
        } else {
            flat = true;
        }
        if up && down {
            return Monotonicity::None(i);
        }
    }
    match (up, down, flat) {
        (false, false, _) => Monotonicity::Constant,
        (true, false, false) => Monotonicity::Increasing,
        (true, false, true) => Monotonicity::NonDecreasing,
        (false, true, false) => Monotonicity::Decreasing,
        (false, true, true) => Monotonicity::NonIncreasing,
        _ => unreachable!(),
    }
}

fn horner(c: &[f64], t: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, &k| acc * t + k)
}

fn horner_derivative(c: &[f64], t: f64) -> f64 {
    let mut acc = 0.0;
    for (k, &ck) in c.iter().enumerate().skip(1).rev() {
        acc = acc * t + ck * k as f64;
    }
    acc
}

/// Piecewise polynomial in the absolute variable (ascending powers per piece).
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewisePoly {
    breaks: Vec<f64>,
    coeffs: Vec<Vec<f64>>,
    /// Out-of-range policy.
    pub extrapolation: Extrapolation,
}

impl PiecewisePoly {
    /// Build and validate (`breaks` has one more entry than `coeffs`).
    pub fn new(breaks: Vec<f64>, coeffs: Vec<Vec<f64>>, extrapolation: Extrapolation) -> Result<Self> {
        if breaks.len() < 2 {
            return bad("breaks", "a piecewise polynomial needs at least 2 breakpoints".into());
        }
        if coeffs.len() + 1 != breaks.len() {
            return bad(
                "coefficients",
                format!(
                    "{} breakpoints define {} pieces but {} coefficient lists were given",
                    breaks.len(),
                    breaks.len() - 1,
                    coeffs.len()
                ),
            );
        }
        check_axis("breaks", &breaks)?;
        for (i, c) in coeffs.iter().enumerate() {
            if c.is_empty() {
                return bad(&format!("piece {i}"), format!("piece {i} has no coefficients"));
            }
            if let Some(k) = c.iter().position(|v| !v.is_finite()) {
                return bad(
                    &format!("piece {i}"),
                    format!("piece {i}: coefficient {k} is not finite"),
                );
            }
        }
        Ok(Self {
            breaks,
            coeffs,
            extrapolation,
        })
    }

    /// Breakpoints.
    pub fn breaks(&self) -> &[f64] {
        &self.breaks
    }

    /// Range `(first, last)` breakpoint.
    pub fn range(&self) -> (f64, f64) {
        (self.breaks[0], self.breaks[self.breaks.len() - 1])
    }

    fn piece(&self, t: f64) -> usize {
        let n = self.coeffs.len();
        match self.breaks.partition_point(|&v| v <= t) {
            0 => 0,
            k if k > n => n - 1,
            k => k - 1,
        }
    }

    /// Value at `t`; NaN with `Extrapolation::Error` outside the range.
    pub fn eval(&self, t: f64) -> f64 {
        if t.is_nan() {
            return f64::NAN;
        }
        let (lo, hi) = self.range();
        let n = self.coeffs.len();
        if t < lo || t > hi {
            return match self.extrapolation {
                Extrapolation::Error => f64::NAN,
                Extrapolation::Clamp => horner(
                    &self.coeffs[if t < lo { 0 } else { n - 1 }],
                    if t < lo { lo } else { hi },
                ),
                Extrapolation::Linear => {
                    let (i, e) = if t < lo { (0, lo) } else { (n - 1, hi) };
                    horner(&self.coeffs[i], e) + horner_derivative(&self.coeffs[i], e) * (t - e)
                }
            };
        }
        horner(&self.coeffs[self.piece(t)], t)
    }

    /// Like [`eval`](Self::eval), but an out-of-range query with `Extrapolation::Error` is an `Err`.
    pub fn try_eval(&self, t: f64) -> Result<f64> {
        let v = self.eval(t);
        if v.is_nan() {
            let (lo, hi) = self.range();
            return bad("T", format!("T = {t} is outside the table range [{lo}, {hi}]"));
        }
        Ok(v)
    }

    /// Largest value jump `|p_{i+1}(b) - p_i(b)|` over the interior breakpoints and the index `i` of the break (0 = the first
    /// interior break); `(0, 0)` for a single piece.
    pub fn max_jump(&self) -> (f64, usize) {
        let mut best = (0.0, 0);
        for i in 1..self.coeffs.len() {
            let b = self.breaks[i];
            let j = (horner(&self.coeffs[i], b) - horner(&self.coeffs[i - 1], b)).abs();
            if j > best.0 {
                best = (j, i - 1);
            }
        }
        best
    }

    /// Error unless every interior jump is at most `tol`.
    pub fn require_continuous(&self, tol: f64) -> Result<()> {
        let (j, i) = self.max_jump();
        if j > tol {
            return bad(
                &format!("break {i}"),
                format!(
                    "the pieces are discontinuous at break {i} (T = {}): jump {j:e} exceeds {tol:e}",
                    self.breaks[i + 1]
                ),
            );
        }
        Ok(())
    }
}

/// `a exp(b / (T - c))`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vogel {
    /// Prefactor (> 0).
    pub a: f64,
    /// Exponent constant (K).
    pub b: f64,
    /// Vogel temperature (K).
    pub c: f64,
}

impl Vogel {
    /// Value at `t` (NaN for `t <= c`).
    pub fn eval(&self, t: f64) -> f64 {
        if t <= self.c {
            return f64::NAN;
        }
        self.a * (self.b / (t - self.c)).exp()
    }

    /// Finite constants and a positive prefactor.
    pub fn validate(&self) -> Result<()> {
        if !(self.a.is_finite() && self.a > 0.0) {
            return bad("vogel_a", "the prefactor must be finite and positive".into());
        }
        if !self.b.is_finite() {
            return bad("vogel_b", "must be finite".into());
        }
        if !self.c.is_finite() {
            return bad("vogel_c", "must be finite".into());
        }
        Ok(())
    }
}

/// Tait compressible-liquid density law.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tait {
    /// Reference density.
    pub rho_ref: f64,
    /// Reference pressure.
    pub p_ref: f64,
    /// Tait pressure constant `b` (same unit as p).
    pub b: f64,
    /// Dimensionless Tait constant `c`.
    pub c: f64,
}

impl Tait {
    /// Build and validate.
    pub fn new(rho_ref: f64, p_ref: f64, b: f64, c: f64) -> Result<Self> {
        let t = Self { rho_ref, p_ref, b, c };
        if !(rho_ref > 0.0 && b > 0.0 && c > 0.0 && p_ref.is_finite() && b + p_ref > 0.0) {
            return bad(
                "tait",
                "needs rho_ref > 0, b > 0, c > 0 and b + p_ref > 0 (all finite)".into(),
            );
        }
        Ok(t)
    }

    /// Density at pressure `p`; NaN when the law breaks down (`c ln(..) >= 1` or `b + p <= 0`).
    pub fn density(&self, p: f64) -> f64 {
        if self.b + p <= 0.0 {
            return f64::NAN;
        }
        let d = 1.0 - self.c * ((self.b + p) / (self.b + self.p_ref)).ln();
        if d <= 0.0 {
            return f64::NAN;
        }
        self.rho_ref / d
    }

    /// Pressure at density `rho` (inverse of [`density`](Self::density)).
    pub fn pressure(&self, rho: f64) -> f64 {
        (self.b + self.p_ref) * ((1.0 - self.rho_ref / rho) / self.c).exp() - self.b
    }
}

/// Phase of a [`PtTable`] lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Compressed liquid (`p > p_sat(T)`).
    Liquid,
    /// Vapour (`p <= p_sat(T)` or above the saturation range).
    Vapour,
}

/// Pressure-temperature table with a saturation line.
#[derive(Debug, Clone, PartialEq)]
pub struct PtTable {
    t: Vec<f64>,
    p: Vec<f64>,
    liquid: Vec<f64>,
    vapour: Vec<f64>,
    psat: PiecewiseLinear,
}

impl PtTable {
    /// Build. `liquid` and `vapour` are `t.len() x p.len()` row-major (row = temperature), NaN = no data. `sat_t`, `sat_p` the
    /// saturation line (both strictly increasing).
    pub fn new(
        t: Vec<f64>,
        p: Vec<f64>,
        liquid: Vec<f64>,
        vapour: Vec<f64>,
        sat_t: Vec<f64>,
        sat_p: Vec<f64>,
    ) -> Result<Self> {
        if t.len() < 2 {
            return bad("t", "needs at least 2 temperatures".into());
        }
        if p.len() < 2 {
            return bad("p", "needs at least 2 pressures".into());
        }
        check_axis("t", &t)?;
        check_axis("p", &p)?;
        for (name, g) in [("liquid", &liquid), ("vapour", &vapour)] {
            if g.len() != t.len() * p.len() {
                return bad(
                    name,
                    format!(
                        "{name} grid has {} values, expected {} x {} = {}",
                        g.len(),
                        t.len(),
                        p.len(),
                        t.len() * p.len()
                    ),
                );
            }
            if let Some(i) = g.iter().position(|v| v.is_infinite()) {
                return bad(name, format!("{name}[{i}] is infinite (use NaN for no data)"));
            }
        }
        if sat_t.len() != sat_p.len() {
            return bad(
                "saturation",
                "saturation temperature and pressure lists differ in length".into(),
            );
        }
        for i in 1..sat_p.len() {
            if sat_p[i] <= sat_p[i - 1] {
                return bad(
                    "saturation",
                    format!("the saturation pressure must be strictly increasing with temperature (saturation p[{i}] = {} <= {})", sat_p[i], sat_p[i - 1]),
                );
            }
        }
        let psat = PiecewiseLinear::new(sat_t, sat_p, Extrapolation::Error).map_err(|e| match e {
            Error::InvalidValue { reason, .. } => Error::InvalidValue {
                path: "saturation".into(),
                reason: format!("saturation line: {reason}"),
            },
            e => e,
        })?;
        Ok(Self {
            t,
            p,
            liquid,
            vapour,
            psat,
        })
    }

    /// Saturation pressure at `t` (error outside the saturation range).
    pub fn psat(&self, t: f64) -> Result<f64> {
        self.psat.try_eval(t)
    }

    /// Saturation temperature at `p`: the exact inverse of the piecewise-linear saturation line.
    pub fn tsat(&self, p: f64) -> Result<f64> {
        let (xs, ys) = (self.psat.x(), self.psat.y());
        let n = ys.len();
        if !(p >= ys[0] && p <= ys[n - 1]) {
            return bad(
                "p",
                format!("p = {p} is outside the saturation range [{}, {}]", ys[0], ys[n - 1]),
            );
        }
        let k = ys.partition_point(|&v| v <= p);
        let i = if k >= n { n - 2 } else { k.saturating_sub(1) };
        if p == ys[i] {
            return Ok(xs[i]);
        }
        Ok(xs[i] + (xs[i + 1] - xs[i]) * ((p - ys[i]) / (ys[i + 1] - ys[i])))
    }

    /// Phase at `(t, p)`.
    pub fn phase(&self, t: f64, p: f64) -> Phase {
        match self.psat.try_eval(t) {
            Ok(ps) if p > ps => Phase::Liquid,
            _ => Phase::Vapour,
        }
    }

    fn cell(axis: &[f64], x: f64) -> Option<usize> {
        let (lo, hi) = (axis[0], axis[axis.len() - 1]);
        if !(x >= lo && x <= hi) {
            return None;
        }
        let k = axis.partition_point(|&v| v <= x);
        Some(if k >= axis.len() {
            axis.len() - 2
        } else {
            k.saturating_sub(1)
        })
    }

    /// Bilinear value of the grid of the phase at `(t, p)`, or a located error (outside the axes, or a corner without data).
    pub fn eval(&self, t: f64, p: f64) -> Result<f64> {
        let phase = self.phase(t, p);
        let grid = match phase {
            Phase::Liquid => &self.liquid,
            Phase::Vapour => &self.vapour,
        };
        let psat = self.psat.try_eval(t).ok();
        let (Some(i), Some(j)) = (Self::cell(&self.t, t), Self::cell(&self.p, p)) else {
            return bad(
                "(t, p)",
                format!(
                    "(T = {t}, p = {p}) is outside the table axes T in [{}, {}], p in [{}, {}]",
                    self.t[0],
                    self.t[self.t.len() - 1],
                    self.p[0],
                    self.p[self.p.len() - 1]
                ),
            );
        };
        let np = self.p.len();
        let g = |a: usize, b: usize| grid[a * np + b];
        let (v00, v01, v10, v11) = (g(i, j), g(i, j + 1), g(i + 1, j), g(i + 1, j + 1));
        if [v00, v01, v10, v11].iter().any(|v| v.is_nan()) {
            return bad(
                "(t, p)",
                format!(
                    "no {phase:?} data at the corner of cell (T index {i}, p index {j}) containing (T = {t}, p = {p}); saturation pressure at T = {}: a cell that straddles the saturation line cannot be interpolated",
                    psat.map_or("outside the saturation range".to_string(), |v| format!("{v}"))
                ),
            );
        }
        let (u, w) = (
            (t - self.t[i]) / (self.t[i + 1] - self.t[i]),
            (p - self.p[j]) / (self.p[j + 1] - self.p[j]),
        );
        if u == 0.0 && w == 0.0 {
            return Ok(v00);
        }
        Ok((1.0 - u) * ((1.0 - w) * v00 + w * v01) + u * ((1.0 - w) * v10 + w * v11))
    }

    /// Density-specific variant of [`eval`](Self::eval) (ADR 0044 D2): bilinear interpolation of `q = v T / p` (equal to
    /// `1/(R_s Z)` for a density grid `v = rho`, a constant for an ideal gas), returned as `q p / T`. Exact for an ideal gas at any
    /// point; for a real vapour the error is set by the curvature of `1/Z` instead of that of `rho`, which is much smaller far from
    /// saturation. Only meaningful for a density grid (it is not applicable to other properties, nor to a liquid grid), needs
    /// positive temperature and pressure axes, and is opt-in: [`eval`](Self::eval) is unchanged.
    pub fn eval_density_ideal_scaled(&self, t: f64, p: f64) -> Result<f64> {
        if !(t > 0.0 && p > 0.0) {
            return bad(
                "(t, p)",
                format!("the ideal-gas scaling needs T > 0 and p > 0, got T = {t}, p = {p}"),
            );
        }
        if !(self.t[0] > 0.0 && self.p[0] > 0.0) {
            return bad(
                "(t, p)",
                "the ideal-gas scaling needs positive temperature and pressure axes".into(),
            );
        }
        let grid = match self.phase(t, p) {
            Phase::Liquid => &self.liquid,
            Phase::Vapour => &self.vapour,
        };
        // the bilinear weights and corner selection are those of `eval`; the corner values are scaled by T/p of their node
        let np = self.p.len();
        let (Some(i), Some(j)) = (Self::cell(&self.t, t), Self::cell(&self.p, p)) else {
            return self.eval(t, p); // the located out-of-axes error of `eval`
        };
        let q = |a: usize, b: usize| grid[a * np + b] * self.t[a] / self.p[b];
        let (v00, v01, v10, v11) = (q(i, j), q(i, j + 1), q(i + 1, j), q(i + 1, j + 1));
        if [v00, v01, v10, v11].iter().any(|v| v.is_nan()) {
            return self.eval(t, p); // the located NaN-corner error of `eval`
        }
        let (u, w) = (
            (t - self.t[i]) / (self.t[i + 1] - self.t[i]),
            (p - self.p[j]) / (self.p[j + 1] - self.p[j]),
        );
        let qv = (1.0 - u) * ((1.0 - w) * v00 + w * v01) + u * ((1.0 - w) * v10 + w * v11);
        Ok(qv * p / t)
    }
}
