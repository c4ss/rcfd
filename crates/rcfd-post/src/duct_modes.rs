//! Hard-walled duct modes, modal decomposition of a sampled section, axial split into the two directions and the
//! `rcfd-duct-modes` JSON export (F-333, ADR 0351 DR5 to DR8).
//!
//! Rectangular duct `0 <= y <= a`, `0 <= z <= b`: `psi_mn = sqrt(eps_m eps_n) cos(m pi y/a) cos(n pi z/b)`,
//! `k_mn^2 = (m pi/a)^2 + (n pi/b)^2`. Circular duct of radius `R` (section coordinates relative to the axis):
//! `psi_mn = J_m(j'_mn r/R) e^{i m phi} / N_mn`, `k_mn = j'_mn/R`, `j'_mn` the `n`-th non-negative zero of `J'_|m|`. Both are
//! normalised to an area mean of `|psi|^2` equal to 1. Axial wavenumber `k_z = sqrt(k^2 - k_mn^2)` (propagating towards
//! `+x`), `-i sqrt(k_mn^2 - k^2)` when cut off. Time factor `e^{+i omega t}`: a field is `p = Re(A e^{i omega t})` and a
//! wave towards `+x` is `A e^{-i k_z x}`.
//!
//! Bessel functions from Bessel's integral by the trapezoidal rule (exponentially convergent for the periodic
//! integrand); zeros by a scan and bisection (ADR 0351 DR6). Library only.

use rcfd_core::{Error, Result};
use serde_json::{json, Map, Value};
use std::f64::consts::PI;

/// A complex number (the workspace has no complex type; only what this module needs).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct C64 {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl C64 {
    /// `re + i im`.
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    /// `e^{i theta}`.
    pub fn cis(theta: f64) -> Self {
        Self::new(theta.cos(), theta.sin())
    }
    /// `e^{z}`.
    pub fn exp(self) -> Self {
        Self::cis(self.im).scale(self.re.exp())
    }
    /// Complex conjugate.
    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }
    /// Modulus.
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    /// Squared modulus.
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    /// Argument in `(-pi, pi]`.
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    /// Times a real number.
    pub fn scale(self, s: f64) -> Self {
        Self::new(self.re * s, self.im * s)
    }
}

impl std::ops::Add for C64 {
    type Output = C64;
    fn add(self, o: C64) -> C64 {
        C64::new(self.re + o.re, self.im + o.im)
    }
}
impl std::ops::Sub for C64 {
    type Output = C64;
    fn sub(self, o: C64) -> C64 {
        C64::new(self.re - o.re, self.im - o.im)
    }
}
impl std::ops::Mul for C64 {
    type Output = C64;
    fn mul(self, o: C64) -> C64 {
        C64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}
impl std::ops::Div for C64 {
    type Output = C64;
    fn div(self, o: C64) -> C64 {
        let d = o.norm_sqr();
        C64::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}
impl std::ops::Neg for C64 {
    type Output = C64;
    fn neg(self) -> C64 {
        C64::new(-self.re, -self.im)
    }
}
impl std::ops::AddAssign for C64 {
    fn add_assign(&mut self, o: C64) {
        self.re += o.re;
        self.im += o.im;
    }
}

fn invalid<T>(path: &str, reason: impl Into<String>) -> Result<T> {
    Err(Error::InvalidValue {
        path: path.into(),
        reason: reason.into(),
    })
}

fn positive(path: &str, v: f64) -> Result<()> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        invalid(path, format!("must be finite and > 0, got {v}"))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Bessel functions
// ---------------------------------------------------------------------------------------------------------------------

/// `J_m(x)` for an integer order, from Bessel's integral `(1/2 pi) int_0^{2 pi} cos(m tau - x sin tau) d tau` by the
/// trapezoidal rule on `2 (ceil(|x|) + |m| + 32)` points (ADR 0351 DR6).
pub fn bessel_j(m: i32, x: f64) -> f64 {
    let n = 2 * (x.abs().ceil() as usize + m.unsigned_abs() as usize + 32);
    let mf = m as f64;
    let s: f64 = (0..n)
        .map(|j| {
            let tau = 2.0 * PI * j as f64 / n as f64;
            (mf * tau - x * tau.sin()).cos()
        })
        .sum();
    s / n as f64
}

/// `J'_m(x) = (J_{m-1}(x) - J_{m+1}(x))/2`.
pub fn bessel_jp(m: i32, x: f64) -> f64 {
    0.5 * (bessel_j(m - 1, x) - bessel_j(m + 1, x))
}

/// The first `count` non-negative zeros of `J'_|m|`, increasing: for `m = 0` the first is 0 (the plane wave); for
/// `m != 0` the zero at the origin is not counted. Scan in steps of 0.05, then bisection.
pub fn bessel_jp_zeros(m: i32, count: usize) -> Vec<f64> {
    let m = m.abs();
    let mut out = Vec::with_capacity(count);
    if count == 0 {
        return out;
    }
    if m == 0 {
        out.push(0.0);
    }
    let step = 0.05;
    let mut x0 = step;
    let mut f0 = bessel_jp(m, x0);
    while out.len() < count {
        let x1 = x0 + step;
        let f1 = bessel_jp(m, x1);
        if f0 == 0.0 {
            out.push(x0);
        } else if f0 * f1 < 0.0 {
            let (mut a, mut b, mut fa) = (x0, x1, f0);
            while b - a > 1e-14 * b.max(1.0) {
                let mid = 0.5 * (a + b);
                let fm = bessel_jp(m, mid);
                if fm == 0.0 {
                    a = mid;
                    b = mid;
                    break;
                }
                if fa * fm < 0.0 {
                    b = mid;
                } else {
                    a = mid;
                    fa = fm;
                }
            }
            out.push(0.5 * (a + b));
        }
        x0 = x1;
        f0 = f1;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Ducts and modes
// ---------------------------------------------------------------------------------------------------------------------

/// The section of a hard-walled duct.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuctShape {
    /// Rectangle `0 <= y <= a`, `0 <= z <= b` (a two-dimensional channel of height `a` uses the modes `(m, 0)`).
    Rectangular {
        /// Width along the first section coordinate [m].
        a: f64,
        /// Width along the second section coordinate [m].
        b: f64,
    },
    /// Circle of the given radius about the origin of the section coordinates.
    Circular {
        /// Radius [m].
        radius: f64,
    },
}

impl DuctShape {
    fn check(&self) -> Result<()> {
        match *self {
            DuctShape::Rectangular { a, b } => {
                positive("duct.a", a)?;
                positive("duct.b", b)
            }
            DuctShape::Circular { radius } => positive("duct.radius", radius),
        }
    }

    /// Area of the section [m2].
    pub fn area(&self) -> f64 {
        match *self {
            DuctShape::Rectangular { a, b } => a * b,
            DuctShape::Circular { radius } => PI * radius * radius,
        }
    }
}

/// A mode index: rectangular `(m, n)` with `m, n >= 0`; circular `m` the azimuthal order (any sign) and `n` the radial
/// order (0 is the first zero of `J'_|m|`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mode {
    /// First index.
    pub m: i32,
    /// Second index.
    pub n: u32,
}

/// Axial wavenumber of a mode: `sqrt(k^2 - k_cut^2)` when it propagates, `-i sqrt(k_cut^2 - k^2)` when cut off.
pub fn axial_wavenumber(k: f64, k_cut: f64) -> C64 {
    let d = k * k - k_cut * k_cut;
    if d >= 0.0 {
        C64::new(d.sqrt(), 0.0)
    } else {
        C64::new(0.0, -(-d).sqrt())
    }
}

/// A list of modes of one duct with their cut-off wavenumbers and normalisations.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeBasis {
    /// The duct.
    pub shape: DuctShape,
    /// The modes.
    pub modes: Vec<Mode>,
    /// Cut-off wavenumber `k_mn` of each mode [1/m].
    pub k_cut: Vec<f64>,
    /// `j'_mn` of each circular mode (0 for rectangular modes).
    zero: Vec<f64>,
    /// Normalisation factor of each mode (the shape is multiplied by it).
    norm: Vec<f64>,
}

impl ModeBasis {
    /// The basis of the listed modes; an error for an invalid duct or index.
    pub fn new(shape: DuctShape, modes: &[Mode]) -> Result<Self> {
        shape.check()?;
        let mut k_cut = Vec::with_capacity(modes.len());
        let mut zero = Vec::with_capacity(modes.len());
        let mut norm = Vec::with_capacity(modes.len());
        for (i, md) in modes.iter().enumerate() {
            match shape {
                DuctShape::Rectangular { a, b } => {
                    if md.m < 0 {
                        return invalid(&format!("modes[{i}].m"), "rectangular modes need m >= 0");
                    }
                    let (m, n) = (md.m as f64, md.n as f64);
                    k_cut.push(((m * PI / a).powi(2) + (n * PI / b).powi(2)).sqrt());
                    zero.push(0.0);
                    let em: f64 = if md.m == 0 { 1.0 } else { 2.0 };
                    let en = if md.n == 0 { 1.0 } else { 2.0 };
                    norm.push((em * en).sqrt());
                }
                DuctShape::Circular { radius } => {
                    let j = bessel_jp_zeros(md.m, md.n as usize + 1)[md.n as usize];
                    k_cut.push(j / radius);
                    zero.push(j);
                    let ms = (md.m as f64).powi(2);
                    let mean = if j == 0.0 {
                        1.0
                    } else {
                        (1.0 - ms / (j * j)) * bessel_j(md.m, j).powi(2)
                    };
                    norm.push(1.0 / mean.sqrt());
                }
            }
        }
        Ok(Self {
            shape,
            modes: modes.to_vec(),
            k_cut,
            zero,
            norm,
        })
    }

    /// Every mode with `k_mn <= k_max`, sorted by `k_mn`, then `m`, then `n` (rectangular: `m, n >= 0`; circular: both
    /// signs of `m`).
    pub fn below(shape: DuctShape, k_max: f64) -> Result<Self> {
        shape.check()?;
        if !(k_max.is_finite() && k_max >= 0.0) {
            return invalid("k_max", format!("must be finite and >= 0, got {k_max}"));
        }
        let mut list: Vec<(f64, Mode)> = Vec::new();
        match shape {
            DuctShape::Rectangular { a, b } => {
                let mmax = (k_max * a / PI).floor() as i32;
                let nmax = (k_max * b / PI).floor() as u32;
                for m in 0..=mmax {
                    for n in 0..=nmax {
                        let kc = (((m as f64) * PI / a).powi(2) + ((n as f64) * PI / b).powi(2)).sqrt();
                        if kc <= k_max {
                            list.push((kc, Mode { m, n }));
                        }
                    }
                }
            }
            DuctShape::Circular { radius } => {
                let xmax = k_max * radius;
                // j'_{m,0} >= m for m >= 1, so m <= xmax suffices.
                for m in 0..=(xmax.floor() as i32) {
                    let mut n = 0usize;
                    loop {
                        let j = bessel_jp_zeros(m, n + 1)[n];
                        if j > xmax {
                            break;
                        }
                        list.push((j / radius, Mode { m, n: n as u32 }));
                        if m != 0 {
                            list.push((j / radius, Mode { m: -m, n: n as u32 }));
                        }
                        n += 1;
                    }
                }
            }
        }
        list.sort_by(|x, y| {
            x.0.partial_cmp(&y.0)
                .unwrap()
                .then(x.1.m.cmp(&y.1.m))
                .then(x.1.n.cmp(&y.1.n))
        });
        let modes: Vec<Mode> = list.into_iter().map(|(_, m)| m).collect();
        Self::new(shape, &modes)
    }

    /// Number of modes.
    pub fn len(&self) -> usize {
        self.modes.len()
    }

    /// True when there is no mode.
    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    /// The normalised shape of mode `q` at the section point `x` (rectangular: `(y, z)` from the corner; circular:
    /// Cartesian coordinates from the axis).
    pub fn eval(&self, q: usize, x: [f64; 2]) -> C64 {
        let md = self.modes[q];
        match self.shape {
            DuctShape::Rectangular { a, b } => {
                let v = (md.m as f64 * PI * x[0] / a).cos() * (md.n as f64 * PI * x[1] / b).cos();
                C64::new(v * self.norm[q], 0.0)
            }
            DuctShape::Circular { radius } => {
                let r = x[0].hypot(x[1]);
                let phi = x[1].atan2(x[0]);
                let radial = if self.zero[q] == 0.0 {
                    1.0
                } else {
                    bessel_j(md.m, self.zero[q] * r / radius)
                };
                C64::cis(md.m as f64 * phi).scale(radial * self.norm[q])
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Decomposition
// ---------------------------------------------------------------------------------------------------------------------

/// Solve the complex system `a x = b` (row-major `n x n`) by Gaussian elimination with partial pivoting.
fn solve_complex(mut a: Vec<C64>, mut b: Vec<C64>, n: usize) -> Result<Vec<C64>> {
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&i, &j| a[i * n + col].abs().partial_cmp(&a[j * n + col].abs()).unwrap())
            .unwrap();
        if a[piv * n + col].abs() == 0.0 || !a[piv * n + col].abs().is_finite() {
            return Err(Error::Numerical("modal fit: singular normal equations".into()));
        }
        if piv != col {
            for k in 0..n {
                a.swap(piv * n + k, col * n + k);
            }
            b.swap(piv, col);
        }
        let d = a[col * n + col];
        for r in col + 1..n {
            let f = a[r * n + col] / d;
            if f == C64::default() {
                continue;
            }
            for k in col..n {
                let v = a[col * n + k];
                a[r * n + k] = a[r * n + k] - f * v;
            }
            let v = b[col];
            b[r] = b[r] - f * v;
        }
    }
    let mut x = vec![C64::default(); n];
    for r in (0..n).rev() {
        let mut s = b[r];
        for k in r + 1..n {
            s = s - a[r * n + k] * x[k];
        }
        x[r] = s / a[r * n + r];
    }
    Ok(x)
}

/// The modal amplitudes of one section and the weighted RMS residual of the fit (relative to nothing: in the units of
/// the field).
#[derive(Debug, Clone, PartialEq)]
pub struct SectionFit {
    /// Amplitude of each mode of the basis.
    pub amplitudes: Vec<C64>,
    /// `sqrt(sum w |f - fit|^2 / sum w)`.
    pub residual: f64,
}

/// The weighted least-squares modal amplitudes of the complex field `values` at the section points `points` with the
/// weights `weights` (the cell areas) on `basis` (ADR 0351 DR7).
pub fn decompose_section(
    basis: &ModeBasis,
    points: &[[f64; 2]],
    weights: &[f64],
    values: &[C64],
) -> Result<SectionFit> {
    let ns = points.len();
    if weights.len() != ns || values.len() != ns {
        return invalid(
            "section",
            format!("{ns} points, {} weights and {} values", weights.len(), values.len()),
        );
    }
    if let Some(i) = weights.iter().position(|w| !(w.is_finite() && *w > 0.0)) {
        return invalid(&format!("weights[{i}]"), "must be finite and > 0");
    }
    let nm = basis.len();
    if nm == 0 || ns < nm {
        return invalid("section", format!("{nm} modes need at least as many samples, got {ns}"));
    }
    let psi: Vec<Vec<C64>> = points
        .iter()
        .map(|&x| (0..nm).map(|q| basis.eval(q, x)).collect())
        .collect();
    let mut g = vec![C64::default(); nm * nm];
    let mut rhs = vec![C64::default(); nm];
    for j in 0..ns {
        let w = weights[j];
        for q in 0..nm {
            let cq = psi[j][q].conj().scale(w);
            for l in 0..nm {
                g[q * nm + l] += cq * psi[j][l];
            }
            rhs[q] += cq * values[j];
        }
    }
    let amplitudes = solve_complex(g, rhs, nm)?;
    let (mut num, mut den) = (0.0, 0.0);
    for j in 0..ns {
        let mut fit = C64::default();
        for q in 0..nm {
            fit += amplitudes[q] * psi[j][q];
        }
        num += weights[j] * (values[j] - fit).norm_sqr();
        den += weights[j];
    }
    Ok(SectionFit {
        amplitudes,
        residual: (num / den).sqrt(),
    })
}

/// The complex amplitude at the frequency `omega` of real time series: `A = (2/N) sum_n v(t_n) e^{-i omega t_n}` (ADR 0351
/// DR7). Exact for a harmonic at `omega` when the samples are uniform over an integer number of periods.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonicDft {
    /// Angular frequency [rad/s].
    pub omega: f64,
    count: usize,
    acc: Vec<C64>,
}

impl HarmonicDft {
    /// An accumulator for `len` series.
    pub fn new(omega: f64, len: usize) -> Result<Self> {
        positive("omega", omega)?;
        Ok(Self {
            omega,
            count: 0,
            acc: vec![C64::default(); len],
        })
    }

    /// Add the samples of all series at the time `t`.
    pub fn add(&mut self, t: f64, values: &[f64]) -> Result<()> {
        if values.len() != self.acc.len() {
            return invalid(
                "values",
                format!("{} values for {} series", values.len(), self.acc.len()),
            );
        }
        let e = C64::cis(-self.omega * t);
        for (a, v) in self.acc.iter_mut().zip(values) {
            *a += e.scale(*v);
        }
        self.count += 1;
        Ok(())
    }

    /// Number of samples added.
    pub fn count(&self) -> usize {
        self.count
    }

    /// The amplitudes; an error before the first sample.
    pub fn amplitudes(&self) -> Result<Vec<C64>> {
        if self.count == 0 {
            return invalid("dft", "no samples");
        }
        let s = 2.0 / self.count as f64;
        Ok(self.acc.iter().map(|a| a.scale(s)).collect())
    }
}

/// The two waves of one mode along the axis: `A(x) = plus e^{-i kz (x - x_ref)} + minus e^{+i kz (x - x_ref)}`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxialFit {
    /// The axial wavenumber used or fitted.
    pub kz: C64,
    /// Amplitude of the wave towards `+x` at `x_ref`.
    pub plus: C64,
    /// Amplitude of the wave towards `-x` at `x_ref`.
    pub minus: C64,
    /// `sqrt(sum |A - fit|^2 / count)`.
    pub residual: f64,
    /// True when a fitted wavenumber lies within `1e-3` of an end of its search bracket (a failed fit).
    pub at_bracket_edge: bool,
}

fn axial_ls(xs: &[f64], amps: &[C64], kz: C64, x_ref: f64) -> Result<(C64, C64, f64)> {
    let mut g = [C64::default(); 4];
    let mut b = [C64::default(); 2];
    let basis = |x: f64| {
        let d = x - x_ref;
        let ip = (C64::new(0.0, -1.0) * kz).scale(d).exp();
        let im = (C64::new(0.0, 1.0) * kz).scale(d).exp();
        [ip, im]
    };
    for (&x, &a) in xs.iter().zip(amps) {
        let e = basis(x);
        for q in 0..2 {
            for l in 0..2 {
                g[q * 2 + l] += e[q].conj() * e[l];
            }
            b[q] += e[q].conj() * a;
        }
    }
    let s = solve_complex(g.to_vec(), b.to_vec(), 2)?;
    let mut r = 0.0;
    for (&x, &a) in xs.iter().zip(amps) {
        let e = basis(x);
        r += (a - s[0] * e[0] - s[1] * e[1]).norm_sqr();
    }
    Ok((s[0], s[1], (r / xs.len() as f64).sqrt()))
}

/// The axial split of one mode's station amplitudes `amps` at `xs` with a given wavenumber (ADR 0351 DR8).
pub fn split_axial(xs: &[f64], amps: &[C64], kz: C64, x_ref: f64) -> Result<AxialFit> {
    if xs.len() != amps.len() || xs.len() < 2 {
        return invalid(
            "stations",
            format!("{} positions and {} amplitudes (need >= 2)", xs.len(), amps.len()),
        );
    }
    let (plus, minus, residual) = axial_ls(xs, amps, kz, x_ref)?;
    Ok(AxialFit {
        kz,
        plus,
        minus,
        residual,
        at_bracket_edge: false,
    })
}

fn golden(lo: f64, hi: f64, tol: f64, mut f: impl FnMut(f64) -> f64) -> f64 {
    let g = 0.5 * (5f64.sqrt() - 1.0);
    let (mut a, mut b) = (lo, hi);
    let mut c = b - g * (b - a);
    let mut d = a + g * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    while b - a > tol {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - g * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + g * (b - a);
            fd = f(d);
        }
    }
    0.5 * (a + b)
}

/// The axial split with a fitted complex wavenumber `kz = kr - i ki` (ADR 0351 DR9): the least-squares residual is
/// minimised over `kr` in `[0.9, 1.1] guess` and, inside, over `ki` in `[-0.02, 0.05] guess` by golden-section searches.
pub fn fit_axial(xs: &[f64], amps: &[C64], guess: f64, x_ref: f64) -> Result<AxialFit> {
    positive("guess", guess)?;
    split_axial(xs, amps, C64::new(guess, 0.0), x_ref)?;
    let (klo, khi) = (0.9 * guess, 1.1 * guess);
    let (ilo, ihi) = (-0.02 * guess, 0.05 * guess);
    let tol = 1e-12 * guess;
    let resid = |kr: f64, ki: f64| {
        axial_ls(xs, amps, C64::new(kr, -ki), x_ref)
            .map(|r| r.2)
            .unwrap_or(f64::INFINITY)
    };
    let best_ki = |kr: f64| golden(ilo, ihi, tol, |ki| resid(kr, ki));
    let kr = golden(klo, khi, tol, |kr| resid(kr, best_ki(kr)));
    let ki = best_ki(kr);
    let edge = 1e-3 * guess;
    let at_bracket_edge = kr - klo < edge || khi - kr < edge || ki - ilo < edge || ihi - ki < edge;
    let mut fit = split_axial(xs, amps, C64::new(kr, -ki), x_ref)?;
    fit.at_bracket_edge = at_bracket_edge;
    Ok(fit)
}

/// One decaying exponential `A(x) = A0 e^{-(alpha + i kr) x}` fitted to station amplitudes: straight lines through
/// `ln|A|` and the unwrapped phase. Returns `(alpha, kr)`.
pub fn fit_exponential(xs: &[f64], amps: &[C64]) -> Result<(f64, f64)> {
    if xs.len() != amps.len() || xs.len() < 2 {
        return invalid(
            "stations",
            format!("{} positions and {} amplitudes (need >= 2)", xs.len(), amps.len()),
        );
    }
    let mut phase = Vec::with_capacity(amps.len());
    let mut prev = amps[0].arg();
    let mut offset = 0.0;
    for a in amps {
        let mut ph = a.arg() + offset;
        while ph - prev > PI {
            ph -= 2.0 * PI;
            offset -= 2.0 * PI;
        }
        while ph - prev < -PI {
            ph += 2.0 * PI;
            offset += 2.0 * PI;
        }
        phase.push(ph);
        prev = ph;
    }
    let ln: Vec<f64> = amps.iter().map(|a| a.abs().ln()).collect();
    let slope = |ys: &[f64]| {
        let n = xs.len() as f64;
        let mx = xs.iter().sum::<f64>() / n;
        let my = ys.iter().sum::<f64>() / n;
        let sxy: f64 = xs.iter().zip(ys).map(|(x, y)| (x - mx) * (y - my)).sum();
        let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
        sxy / sxx
    };
    Ok((-slope(&ln), -slope(&phase)))
}

// ---------------------------------------------------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------------------------------------------------

/// The values of one mode in an export.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModeValues {
    /// The amplitude on one section.
    Section {
        /// Complex amplitude [Pa].
        amplitude: C64,
    },
    /// The two axial waves at `x_ref` and their powers.
    Axial {
        /// Wave towards `+x` [Pa].
        plus: C64,
        /// Wave towards `-x` [Pa].
        minus: C64,
        /// Power carried towards `+x` [W] (zero for a cut-off mode).
        power_plus: f64,
        /// Power carried towards `-x` [W].
        power_minus: f64,
    },
}

/// One mode of an export.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeRecord {
    /// The mode.
    pub mode: Mode,
    /// Cut-off wavenumber [1/m].
    pub k_cut: f64,
    /// Axial wavenumber [1/m].
    pub k_axial: C64,
    /// The values.
    pub values: ModeValues,
}

/// A modal export, format `rcfd-duct-modes` version 1 (ADR 0351 DR8).
#[derive(Debug, Clone, PartialEq)]
pub struct ModalExport {
    /// The duct.
    pub duct: DuctShape,
    /// Frequency [Hz].
    pub frequency: f64,
    /// Sound speed [m/s].
    pub sound_speed: f64,
    /// Density [kg/m3].
    pub density: f64,
    /// Reference position of the axial waves (`None` for a section export).
    pub x_ref: Option<f64>,
    /// Residual of the fit that produced the amplitudes.
    pub residual: f64,
    /// The modes; all of one kind (section or axial).
    pub modes: Vec<ModeRecord>,
}

/// The name of the format.
pub const FORMAT: &str = "rcfd-duct-modes";
/// The version of the format.
pub const VERSION: u64 = 1;

impl ModalExport {
    /// A section export of the amplitudes of `basis` at the frequency `frequency`.
    pub fn section(
        basis: &ModeBasis,
        frequency: f64,
        sound_speed: f64,
        density: f64,
        fit: &SectionFit,
    ) -> Result<Self> {
        positive("frequency", frequency)?;
        positive("sound_speed", sound_speed)?;
        positive("density", density)?;
        if fit.amplitudes.len() != basis.len() {
            return invalid(
                "amplitudes",
                format!("{} amplitudes for {} modes", fit.amplitudes.len(), basis.len()),
            );
        }
        let k = 2.0 * PI * frequency / sound_speed;
        let modes = (0..basis.len())
            .map(|q| ModeRecord {
                mode: basis.modes[q],
                k_cut: basis.k_cut[q],
                k_axial: axial_wavenumber(k, basis.k_cut[q]),
                values: ModeValues::Section {
                    amplitude: fit.amplitudes[q],
                },
            })
            .collect();
        Ok(Self {
            duct: basis.shape,
            frequency,
            sound_speed,
            density,
            x_ref: None,
            residual: fit.residual,
            modes,
        })
    }

    /// An axial export: per mode of `basis` the two waves `(plus, minus)` at `x_ref` and their powers
    /// `W = S |a|^2 Re(kz)/(2 rho0 omega)`.
    pub fn axial(
        basis: &ModeBasis,
        frequency: f64,
        sound_speed: f64,
        density: f64,
        x_ref: f64,
        waves: &[(C64, C64)],
        residual: f64,
    ) -> Result<Self> {
        positive("frequency", frequency)?;
        positive("sound_speed", sound_speed)?;
        positive("density", density)?;
        if !x_ref.is_finite() {
            return invalid("x_ref", "must be finite");
        }
        if waves.len() != basis.len() {
            return invalid("waves", format!("{} wave pairs for {} modes", waves.len(), basis.len()));
        }
        let omega = 2.0 * PI * frequency;
        let k = omega / sound_speed;
        let area = basis.shape.area();
        let modes = (0..basis.len())
            .map(|q| {
                let kz = axial_wavenumber(k, basis.k_cut[q]);
                let w = |a: C64| area * a.norm_sqr() * kz.re / (2.0 * density * omega);
                let (plus, minus) = waves[q];
                ModeRecord {
                    mode: basis.modes[q],
                    k_cut: basis.k_cut[q],
                    k_axial: kz,
                    values: ModeValues::Axial {
                        plus,
                        minus,
                        power_plus: w(plus),
                        power_minus: w(minus),
                    },
                }
            })
            .collect();
        Ok(Self {
            duct: basis.shape,
            frequency,
            sound_speed,
            density,
            x_ref: Some(x_ref),
            residual,
            modes,
        })
    }

    /// The JSON value.
    pub fn to_json(&self) -> Value {
        let c = |z: C64| json!([z.re, z.im]);
        let duct = match self.duct {
            DuctShape::Rectangular { a, b } => json!({"shape": "rectangular", "a": a, "b": b}),
            DuctShape::Circular { radius } => json!({"shape": "circular", "radius": radius}),
        };
        let kind = if self.x_ref.is_some() { "axial" } else { "section" };
        let modes: Vec<Value> = self
            .modes
            .iter()
            .map(|r| {
                let mut o = Map::new();
                o.insert("m".into(), json!(r.mode.m));
                o.insert("n".into(), json!(r.mode.n));
                o.insert("k_cut".into(), json!(r.k_cut));
                o.insert("k_axial".into(), c(r.k_axial));
                match r.values {
                    ModeValues::Section { amplitude } => {
                        o.insert("amplitude".into(), c(amplitude));
                    }
                    ModeValues::Axial {
                        plus,
                        minus,
                        power_plus,
                        power_minus,
                    } => {
                        o.insert("plus".into(), c(plus));
                        o.insert("minus".into(), c(minus));
                        o.insert("power_plus".into(), json!(power_plus));
                        o.insert("power_minus".into(), json!(power_minus));
                    }
                }
                Value::Object(o)
            })
            .collect();
        let mut v = json!({
            "format": FORMAT,
            "version": VERSION,
            "kind": kind,
            "duct": duct,
            "frequency": self.frequency,
            "sound_speed": self.sound_speed,
            "density": self.density,
            "residual": self.residual,
            "modes": modes,
        });
        if let Some(x) = self.x_ref {
            v["x_ref"] = json!(x);
        }
        v
    }

    /// The JSON text (pretty-printed).
    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).expect("a JSON value always serialises")
    }

    /// Read an export; errors name the field.
    pub fn from_json_str(text: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(text).map_err(|e| Error::Io(format!("rcfd-duct-modes: {e}")))?;
        Self::from_json(&v)
    }

    /// Read an export from a JSON value; errors name the field.
    pub fn from_json(v: &Value) -> Result<Self> {
        let num = |v: &Value, path: &str| -> Result<f64> {
            v.as_f64().ok_or_else(|| Error::InvalidValue {
                path: path.into(),
                reason: "expected a number".into(),
            })
        };
        let field = |o: &Value, key: &str, path: &str| -> Result<f64> {
            match o.get(key) {
                Some(x) => num(x, &format!("{path}{key}")),
                None => invalid(&format!("{path}{key}"), "missing"),
            }
        };
        let cpx = |o: &Value, key: &str, path: &str| -> Result<C64> {
            let p = format!("{path}{key}");
            match o.get(key).and_then(|x| x.as_array()) {
                Some(a) if a.len() == 2 && a[0].is_number() && a[1].is_number() => {
                    Ok(C64::new(a[0].as_f64().unwrap(), a[1].as_f64().unwrap()))
                }
                _ => invalid(&p, "expected [re, im]"),
            }
        };
        if v.get("format").and_then(|x| x.as_str()) != Some(FORMAT) {
            return invalid("format", format!("expected \"{FORMAT}\""));
        }
        if v.get("version").and_then(|x| x.as_u64()) != Some(VERSION) {
            return invalid("version", format!("expected {VERSION}"));
        }
        let axial = match v.get("kind").and_then(|x| x.as_str()) {
            Some("axial") => true,
            Some("section") => false,
            _ => return invalid("kind", "expected \"section\" or \"axial\""),
        };
        let d = v.get("duct").ok_or_else(|| Error::InvalidValue {
            path: "duct".into(),
            reason: "missing".into(),
        })?;
        let duct = match d.get("shape").and_then(|x| x.as_str()) {
            Some("rectangular") => DuctShape::Rectangular {
                a: field(d, "a", "duct.")?,
                b: field(d, "b", "duct.")?,
            },
            Some("circular") => DuctShape::Circular {
                radius: field(d, "radius", "duct.")?,
            },
            _ => return invalid("duct.shape", "expected \"rectangular\" or \"circular\""),
        };
        duct.check()?;
        let x_ref = if axial { Some(field(v, "x_ref", "")?) } else { None };
        let list = v
            .get("modes")
            .and_then(|x| x.as_array())
            .ok_or_else(|| Error::InvalidValue {
                path: "modes".into(),
                reason: "expected an array".into(),
            })?;
        let mut modes = Vec::with_capacity(list.len());
        for (i, r) in list.iter().enumerate() {
            let p = format!("modes[{i}].");
            let m = r.get("m").and_then(|x| x.as_i64()).ok_or_else(|| Error::InvalidValue {
                path: format!("{p}m"),
                reason: "expected an integer".into(),
            })?;
            let n = r.get("n").and_then(|x| x.as_u64()).ok_or_else(|| Error::InvalidValue {
                path: format!("{p}n"),
                reason: "expected a non-negative integer".into(),
            })?;
            let values = if axial {
                ModeValues::Axial {
                    plus: cpx(r, "plus", &p)?,
                    minus: cpx(r, "minus", &p)?,
                    power_plus: field(r, "power_plus", &p)?,
                    power_minus: field(r, "power_minus", &p)?,
                }
            } else {
                ModeValues::Section {
                    amplitude: cpx(r, "amplitude", &p)?,
                }
            };
            modes.push(ModeRecord {
                mode: Mode {
                    m: i32::try_from(m).map_err(|_| Error::InvalidValue {
                        path: format!("{p}m"),
                        reason: "out of range".into(),
                    })?,
                    n: u32::try_from(n).map_err(|_| Error::InvalidValue {
                        path: format!("{p}n"),
                        reason: "out of range".into(),
                    })?,
                },
                k_cut: field(r, "k_cut", &p)?,
                k_axial: cpx(r, "k_axial", &p)?,
                values,
            });
        }
        Ok(Self {
            duct,
            frequency: field(v, "frequency", "")?,
            sound_speed: field(v, "sound_speed", "")?,
            density: field(v, "density", "")?,
            x_ref,
            residual: field(v, "residual", "")?,
            modes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factorial(n: u32) -> f64 {
        (1..=n).map(|k| k as f64).product()
    }

    /// B1 (ADR 0351): Bessel's integral against the power series, m = 0..5, x in [0, 8], within 1e-12.
    #[test]
    fn b1_bessel_integral_against_series() {
        let mut worst: f64 = 0.0;
        for m in 0..=5i32 {
            for i in 0..=80 {
                let x = 0.1 * i as f64;
                let series: f64 = (0..40u32)
                    .map(|k| {
                        (-1f64).powi(k as i32) * (x / 2.0).powi(2 * k as i32 + m)
                            / (factorial(k) * factorial(k + m as u32))
                    })
                    .sum();
                worst = worst.max((bessel_j(m, x) - series).abs());
                // Negative orders: J_{-m} = (-1)^m J_m.
                worst = worst.max((bessel_j(-m, x) - (-1f64).powi(m) * series).abs());
            }
        }
        println!("B1 worst |J_integral - J_series| = {worst:.3e}");
        assert!(worst <= 1e-12, "B1: {worst:e}");
    }

    /// B2 (ADR 0351): zeros of J'_m, m = 0..6, n <= 4: |J'_m(j)| <= 1e-12, increasing, Bessel's equation holds there.
    #[test]
    fn b2_derivative_zeros() {
        let mut worst_jp: f64 = 0.0;
        let mut worst_ode: f64 = 0.0;
        for m in 0..=6i32 {
            let z = bessel_jp_zeros(m, 5);
            assert_eq!(z.len(), 5);
            for w in z.windows(2) {
                assert!(w[1] > w[0] + 0.5, "B2: zeros of m = {m} not increasing: {z:?}");
            }
            if m == 0 {
                assert_eq!(z[0], 0.0);
            } else {
                assert!(z[0] >= m as f64, "B2: j'_m0 >= m for m = {m}: {}", z[0]);
            }
            for &j in &z {
                if j == 0.0 {
                    continue;
                }
                worst_jp = worst_jp.max(bessel_jp(m, j).abs());
                // x^2 J'' + x J' + (x^2 - m^2) J = 0 with J'' by a central difference.
                let h = 1e-4;
                let jpp = (bessel_j(m, j + h) - 2.0 * bessel_j(m, j) + bessel_j(m, j - h)) / (h * h);
                let ode = j * j * jpp + j * bessel_jp(m, j) + (j * j - (m * m) as f64) * bessel_j(m, j);
                worst_ode = worst_ode.max(ode.abs() / (j * j));
            }
        }
        println!("B2 worst |J'(j)| = {worst_jp:.3e}, Bessel residual = {worst_ode:.3e}");
        assert!(worst_jp <= 1e-12, "B2: {worst_jp:e}");
        assert!(worst_ode <= 1e-6, "B2 (difference quotient): {worst_ode:e}");
    }

    /// B3 (ADR 0351): the area mean of |psi|^2 is 1 within 1e-5 by quadrature, rectangular and circular modes.
    #[test]
    fn b3_normalisation_by_quadrature() {
        let rect = ModeBasis::new(
            DuctShape::Rectangular { a: 0.12, b: 0.08 },
            &[
                Mode { m: 0, n: 0 },
                Mode { m: 1, n: 0 },
                Mode { m: 0, n: 1 },
                Mode { m: 2, n: 1 },
                Mode { m: 3, n: 2 },
            ],
        )
        .unwrap();
        let nq = 2000;
        let mut worst: f64 = 0.0;
        for q in 0..rect.len() {
            let mut s = 0.0;
            for i in 0..nq {
                let y = 0.12 * (i as f64 + 0.5) / nq as f64;
                for j in 0..nq {
                    let z = 0.08 * (j as f64 + 0.5) / nq as f64;
                    s += rect.eval(q, [y, z]).norm_sqr();
                }
            }
            worst = worst.max((s / (nq * nq) as f64 - 1.0).abs());
        }
        let circ = ModeBasis::below(DuctShape::Circular { radius: 0.06 }, 7.5 / 0.06).unwrap();
        let (nr, nt) = (400, 256);
        for q in 0..circ.len() {
            let mut s = 0.0;
            for i in 0..nr {
                let (r0, r1) = (0.06 * i as f64 / nr as f64, 0.06 * (i + 1) as f64 / nr as f64);
                let r = 0.5 * (r0 + r1);
                let w = 0.5 * (r1 * r1 - r0 * r0) * 2.0 * PI / nt as f64;
                for j in 0..nt {
                    let phi = 2.0 * PI * (j as f64 + 0.5) / nt as f64;
                    s += w * circ.eval(q, [r * phi.cos(), r * phi.sin()]).norm_sqr();
                }
            }
            worst = worst.max((s / (PI * 0.06 * 0.06) - 1.0).abs());
        }
        println!(
            "B3 worst |mean |psi|^2 - 1| = {worst:.3e} over {} modes",
            rect.len() + circ.len()
        );
        assert!(worst <= 1e-5, "B3: {worst:e}");
    }

    /// J1 (ADR 0351): read(write(e)) == e for both kinds; malformed inputs name the field.
    #[test]
    fn j1_json_round_trip_and_errors() {
        let basis = ModeBasis::below(DuctShape::Circular { radius: 0.06 }, 4.5 / 0.06).unwrap();
        let amps: Vec<C64> = (0..basis.len())
            .map(|q| C64::new(0.1 * q as f64 + 1.0 / 3.0, -0.7 / (q as f64 + 1.0)))
            .collect();
        let sec = ModalExport::section(
            &basis,
            2361.1,
            340.0,
            1.2,
            &SectionFit {
                amplitudes: amps.clone(),
                residual: 1.234e-17,
            },
        )
        .unwrap();
        assert_eq!(ModalExport::from_json_str(&sec.to_json_string()).unwrap(), sec);
        let waves: Vec<(C64, C64)> = amps.iter().map(|a| (*a, a.conj().scale(0.3))).collect();
        let rect = ModeBasis::below(DuctShape::Rectangular { a: 0.12, b: 0.08 }, 60.0).unwrap();
        let ax = ModalExport::axial(&rect, 3246.8, 340.0, 1.2, 0.1, &waves[..rect.len()], 0.0).unwrap();
        assert_eq!(ModalExport::from_json_str(&ax.to_json_string()).unwrap(), ax);
        let text = ax.to_json_string();
        let cases = [
            (text.replace("rcfd-duct-modes", "other"), "format"),
            (text.replace("\"version\": 1", "\"version\": 2"), "version"),
            (text.replace("\"axial\"", "\"other\""), "kind"),
            (text.replacen("\"plus\": [", "\"plus\": [\"x\", ", 1), "modes[0].plus"),
            (text.replacen("\"x_ref\"", "\"x_rf\"", 1), "x_ref"),
            (text.replace("\"rectangular\"", "\"oval\""), "duct.shape"),
        ];
        for (bad, path) in cases {
            match ModalExport::from_json_str(&bad) {
                Err(Error::InvalidValue { path: p, .. }) => assert_eq!(p, path),
                other => panic!("J1: expected an error at {path}, got {other:?}"),
            }
        }
    }
}
