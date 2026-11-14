//! Random vibration: spectral response, root-mean-square stress, spectral moments. Declared by ADR 0440 (F-408, the scaffold of
//! ADR 0311); filled by ADR 0477 (F-417, stub ADR 0320).
//!
//! A stationary input with a one-sided spectral density `G(f)` in Hz ([`Spectrum`]) through a linear system with the complex
//! frequency response `Y(f)` gives the output density `|Y|^2 G` and the mean square `m_0 = int |Y|^2 G df`; several inputs with
//! the cross-densities `G_ab = rho_ab sqrt(G_a G_b)` (a constant real correlation `rho`, ADR 0477 D4) give `S_ij = sum_ab G_ab
//! Re(Y_ia conj Y_jb)` between outputs `i` and `j`.
//!
//! * [`ModalChannels`] (D2): outputs of a modal system, `Y_oa = sum_r u_oar H_r(omega) + s_oa`, `H_r = 1/(lambda_r - omega^2 + i
//!   c_r omega)`, times `(i omega)^k` for the [`Quantity`], plus an absolute term for a base acceleration;
//!   [`spectral_response`] integrates them on the grid of [`frequency_grid`] (D3: steps of a quarter of the local scale, the
//!   distance to the nearest half-power peak or the frequency itself, 3-point Gauss-Legendre on each step).
//! * [`RandomSolver`] (D6): the solid-model path on the F-399 [`HarmonicSolver`] (its modal damping and static correction)
//!   under force patterns or a base acceleration ([`Excitation`]): moments and densities at chosen equations, and per
//!   integration point the stress covariance ([`RandomStress`]) with the root-mean-square von Mises stress
//!   ([`von_mises_mean_square`], D5).
//!
//! Spectral moments [`Moments`] `m_k = int f^k S df` (Hz); the zero up-crossing rate `sqrt(m_2/m_0)` by
//! [`crate::fatigue::spectral_rate`], which with the root-mean-square feeds [`crate::fatigue::narrow_band_damage_rate`].
//! Every loop is sequential in a fixed order: the results do not depend on the machine.

use crate::constraint::prescribed_equations;
use crate::error::{FemError, Result};
use crate::fatigue::spectral_rate;
use crate::harmonic::{HarmonicOptions, HarmonicSolver, ModalBasis};
use crate::mass::{consistent_with, rigid_motions};
use crate::model::SolidModel;
use crate::recover::IntegrationPointField;
use rcfd_core::vec3::Vec3;
use std::f64::consts::PI;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The steps per local scale of the registered grid ([`frequency_grid`] at refinement 1; ADR 0477 D3).
pub const STEPS_PER_SCALE: usize = 4;

/// The largest number of quadrature nodes of a grid.
pub const MAX_NODES: usize = 4_000_000;

/// A correlation matrix `rho` is accepted when `rho + CORRELATION_TOLERANCE I` has a Cholesky factor (ADR 0477 D4).
pub const CORRELATION_TOLERANCE: f64 = 1e-12;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

const SPECTRUM: &str = "spectrum";
const RANDOM: &str = "random";

// ------------------------------------------------------------------------------------------------------------ spectrum

/// How a [`Spectrum`] varies between its entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Interpolation {
    /// Linear in the frequency.
    Linear,
    /// A power law on each segment (straight in log-log axes).
    LogLog,
}

/// A one-sided spectral density table `(f_i, G_i)`, frequencies in Hz, zero outside `[f_0, f_n-1]` (ADR 0477 D1).
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    frequencies: Vec<f64>,
    values: Vec<f64>,
    interpolation: Interpolation,
}

impl Spectrum {
    /// A table with linear interpolation: at least two entries, the frequencies finite, `>= 0` and increasing, the values
    /// finite and `>= 0`; a refusal names the entry (zero-based) and its value.
    pub fn new(frequencies: Vec<f64>, values: Vec<f64>) -> Result<Spectrum> {
        Spectrum::build(frequencies, values, Interpolation::Linear)
    }

    /// A table with log-log interpolation: as [`Spectrum::new`] with positive frequencies and values.
    pub fn log_log(frequencies: Vec<f64>, values: Vec<f64>) -> Result<Spectrum> {
        Spectrum::build(frequencies, values, Interpolation::LogLog)
    }

    /// The flat spectrum `g` on `[f_lo, f_hi]`.
    pub fn flat(f_lo: f64, f_hi: f64, g: f64) -> Result<Spectrum> {
        Spectrum::new(vec![f_lo, f_hi], vec![g, g])
    }

    fn build(frequencies: Vec<f64>, values: Vec<f64>, interpolation: Interpolation) -> Result<Spectrum> {
        if frequencies.len() != values.len() {
            return Err(invalid(
                SPECTRUM,
                format!("{} frequencies and {} values", frequencies.len(), values.len()),
            ));
        }
        if frequencies.len() < 2 {
            return Err(invalid(
                SPECTRUM,
                format!("a spectrum needs at least two entries, found {}", frequencies.len()),
            ));
        }
        let log = interpolation == Interpolation::LogLog;
        for (i, (&f, &g)) in frequencies.iter().zip(&values).enumerate() {
            if !f.is_finite() || f < 0.0 || (log && f <= 0.0) {
                let need = if log {
                    "finite and positive (log-log)"
                } else {
                    "finite and >= 0"
                };
                return Err(invalid(SPECTRUM, format!("entry {i}: frequency {f:e} must be {need}")));
            }
            if i > 0 && f <= frequencies[i - 1] {
                return Err(invalid(
                    SPECTRUM,
                    format!(
                        "entry {i}: frequency {f:e} is not above entry {} ({:e}): the frequencies must increase",
                        i - 1,
                        frequencies[i - 1]
                    ),
                ));
            }
            if !g.is_finite() || g < 0.0 {
                return Err(invalid(
                    SPECTRUM,
                    format!("entry {i}: value {g:e} is negative or not finite"),
                ));
            }
            if log && g <= 0.0 {
                return Err(invalid(
                    SPECTRUM,
                    format!("entry {i}: value {g:e} must be positive on a log-log spectrum"),
                ));
            }
        }
        Ok(Spectrum {
            frequencies,
            values,
            interpolation,
        })
    }

    /// The frequencies, Hz.
    pub fn frequencies(&self) -> &[f64] {
        &self.frequencies
    }

    /// The values, units^2/Hz.
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// The interpolation.
    pub fn interpolation(&self) -> Interpolation {
        self.interpolation
    }

    /// The same table with every value times `factor` (finite, `>= 0`; positive for a log-log table).
    pub fn scaled(&self, factor: f64) -> Result<Spectrum> {
        Spectrum::build(
            self.frequencies.clone(),
            self.values.iter().map(|g| g * factor).collect(),
            self.interpolation,
        )
    }

    /// The density at `f` (0 outside the table).
    pub fn value(&self, f: f64) -> f64 {
        let fs = &self.frequencies;
        let n = fs.len();
        if !(f >= fs[0] && f <= fs[n - 1]) {
            return 0.0;
        }
        let i = fs.partition_point(|&x| x <= f).clamp(1, n - 1) - 1;
        let (f0, f1, g0, g1) = (fs[i], fs[i + 1], self.values[i], self.values[i + 1]);
        match self.interpolation {
            Interpolation::Linear => g0 + (f - f0) / (f1 - f0) * (g1 - g0),
            Interpolation::LogLog => {
                let alpha = (g1 / g0).ln() / (f1 / f0).ln();
                g0 * (f / f0).powf(alpha)
            }
        }
    }

    /// The exact integral `int G df` of the table (the input's mean square).
    pub fn mean_square(&self) -> f64 {
        let fs = &self.frequencies;
        let gs = &self.values;
        let mut sum = 0.0;
        for i in 0..fs.len() - 1 {
            let (f0, f1, g0, g1) = (fs[i], fs[i + 1], gs[i], gs[i + 1]);
            sum += match self.interpolation {
                Interpolation::Linear => 0.5 * (g0 + g1) * (f1 - f0),
                Interpolation::LogLog => {
                    // g0 f0 int_0^L e^{(alpha + 1) t} dt, L = ln(f1/f0)
                    let l = (f1 / f0).ln();
                    let e = (g1 / g0).ln() + l;
                    if e == 0.0 {
                        g0 * f0 * l
                    } else {
                        g0 * f0 * l * e.exp_m1() / e
                    }
                }
            };
        }
        sum
    }
}

// ------------------------------------------------------------------------------------------------------------- moments

/// The spectral moments `m_k = int f^k S(f) df` (Hz) of one output.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct Moments {
    /// The mean square.
    pub m0: f64,
    /// The first moment.
    pub m1: f64,
    /// The second moment.
    pub m2: f64,
    /// The fourth moment.
    pub m4: f64,
}

impl Moments {
    /// The root-mean-square `sqrt(m_0)`.
    pub fn rms(&self) -> f64 {
        self.m0.sqrt()
    }

    /// The zero up-crossing rate `sqrt(m_2/m_0)`, Hz (Rice), by [`crate::fatigue::spectral_rate`] (refuses `m_0 <= 0`).
    pub fn zero_crossing_rate(&self) -> Result<f64> {
        Ok(spectral_rate(self.m0, self.m2)?.1)
    }

    /// The rate of peaks `sqrt(m_4/m_2)`, Hz (`None` for `m_2 <= 0`).
    pub fn peak_rate(&self) -> Option<f64> {
        (self.m2 > 0.0).then(|| (self.m4 / self.m2).sqrt())
    }
}

/// What an output channel measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Quantity {
    /// The displacement (relative to the base under a base excitation).
    #[default]
    Displacement,
    /// The velocity (relative to the base under a base excitation).
    Velocity,
    /// The acceleration (absolute under a base excitation).
    Acceleration,
}

impl Quantity {
    /// The factor `(i omega)^k` as (re, im).
    fn factor(self, omega: f64) -> (f64, f64) {
        match self {
            Quantity::Displacement => (1.0, 0.0),
            Quantity::Velocity => (0.0, omega),
            Quantity::Acceleration => (-omega * omega, 0.0),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------- grid

const GAUSS_X: f64 = 0.774_596_669_241_483_4; // sqrt(3/5)
const GAUSS_W_OUTER: f64 = 5.0 / 9.0;
const GAUSS_W_CENTRE: f64 = 8.0 / 9.0;

/// The quadrature nodes (Hz) and weights of ADR 0477 D3 for spectra and modes (`lambda_r`, `c_r = 2 zeta_r omega_r`):
/// between consecutive table frequencies, steps `ell(f)/(STEPS_PER_SCALE refinement)` with `ell(f) = min(max(f, f_ref),
/// min_r sqrt((f - f_r)^2 + b_r^2))`, `f_r = sqrt(lambda_r)/(2 pi)`, `b_r = c_r/(4 pi)`, and 3-point Gauss-Legendre on each
/// step. Refused: an undamped mode inside the band, a refinement of 0, more than [`MAX_NODES`] nodes.
pub fn frequency_grid(
    spectra: &[Spectrum],
    eigenvalues: &[f64],
    damping: &[f64],
    refinement: usize,
) -> Result<(Vec<f64>, Vec<f64>)> {
    if spectra.is_empty() {
        return Err(invalid(RANDOM, "no spectrum".to_string()));
    }
    if refinement == 0 {
        return Err(invalid(RANDOM, "the grid refinement must be at least 1".to_string()));
    }
    if eigenvalues.len() != damping.len() {
        return Err(invalid(
            RANDOM,
            format!("{} eigenvalues and {} damping terms", eigenvalues.len(), damping.len()),
        ));
    }
    let mut breaks: Vec<f64> = spectra.iter().flat_map(|s| s.frequencies.iter().copied()).collect();
    breaks.sort_by(f64::total_cmp);
    breaks.dedup();
    let (lo, hi) = (breaks[0], breaks[breaks.len() - 1]);
    let mut modes = Vec::with_capacity(eigenvalues.len());
    for (r, (&l, &c)) in eigenvalues.iter().zip(damping).enumerate() {
        if !(l.is_finite() && c.is_finite() && c >= 0.0) {
            return Err(invalid(
                RANDOM,
                format!("mode {r}: eigenvalue {l:e} and damping term {c:e} must be finite, the damping >= 0"),
            ));
        }
        let fr = l.max(0.0).sqrt() / (2.0 * PI);
        let br = c / (4.0 * PI);
        if br == 0.0 && fr >= lo && fr <= hi {
            return Err(invalid(
                RANDOM,
                format!(
                    "mode {r} ({fr:e} Hz) is undamped inside the band [{lo:e}, {hi:e}] Hz: the mean square is infinite"
                ),
            ));
        }
        modes.push((fr, br));
    }
    let f_ref = breaks
        .iter()
        .copied()
        .chain(modes.iter().map(|m| m.0))
        .filter(|&f| f > 0.0)
        .fold(f64::INFINITY, f64::min);
    let ell = |f: f64| -> f64 {
        let mut l = f.max(f_ref);
        for &(fr, br) in &modes {
            l = l.min((f - fr).hypot(br));
        }
        l
    };
    let p = (STEPS_PER_SCALE * refinement) as f64;
    let mut nodes = Vec::new();
    let mut weights = Vec::new();
    for w in breaks.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut f = a;
        while f < b {
            let g = (f + ell(f) / p).min(b);
            if g <= f {
                return Err(invalid(
                    RANDOM,
                    format!("the grid step at {f:e} Hz vanishes (a mode at the band's edge without damping?)"),
                ));
            }
            let mid = 0.5 * (f + g);
            let half = 0.5 * (g - f);
            nodes.extend([mid - half * GAUSS_X, mid, mid + half * GAUSS_X]);
            weights.extend([half * GAUSS_W_OUTER, half * GAUSS_W_CENTRE, half * GAUSS_W_OUTER]);
            if nodes.len() > MAX_NODES {
                return Err(invalid(
                    RANDOM,
                    format!("the frequency grid exceeds {MAX_NODES} nodes (refinement {refinement})"),
                ));
            }
            f = g;
        }
    }
    Ok((nodes, weights))
}

// --------------------------------------------------------------------------------------------------------- correlation

/// Checks a correlation matrix of `n` patterns (ADR 0477 D4).
fn check_correlation(rho: &[Vec<f64>], n: usize) -> Result<()> {
    const KEY: &str = "correlation";
    if rho.len() != n || rho.iter().any(|row| row.len() != n) {
        return Err(invalid(
            KEY,
            format!("the correlation must be {n} x {n} (one row per pattern)"),
        ));
    }
    for a in 0..n {
        if rho[a][a] != 1.0 {
            return Err(invalid(
                KEY,
                format!("diagonal entry {a} is {:e}, must be 1", rho[a][a]),
            ));
        }
        for b in 0..n {
            if !rho[a][b].is_finite() || rho[a][b] != rho[b][a] {
                return Err(invalid(
                    KEY,
                    format!(
                        "entries ({a}, {b}) {:e} and ({b}, {a}) {:e}: must be finite and symmetric",
                        rho[a][b], rho[b][a]
                    ),
                ));
            }
        }
    }
    let mut l = vec![vec![0.0; n]; n];
    for j in 0..n {
        let mut d = rho[j][j] + CORRELATION_TOLERANCE;
        for k in 0..j {
            d -= l[j][k] * l[j][k];
        }
        if d.is_nan() || d <= 0.0 {
            return Err(invalid(
                KEY,
                format!("not positive semi-definite: Cholesky pivot {j} of rho + {CORRELATION_TOLERANCE:e} I is {d:e}"),
            ));
        }
        let d = d.sqrt();
        l[j][j] = d;
        for i in j + 1..n {
            let mut s = rho[i][j];
            for k in 0..j {
                s -= l[i][k] * l[j][k];
            }
            l[i][j] = s / d;
        }
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------------------- the channels

/// Outputs of a modal system (ADR 0477 D2): for pattern `a` and output `o`, `Y_oa = sum_r u_oar H_r + s_oa`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalChannels {
    eigenvalues: Vec<f64>,
    damping: Vec<f64>,
    /// `[pattern][output * n_modes + r]`.
    gains: Vec<Vec<f64>>,
    /// `[pattern][output]`.
    residuals: Vec<Vec<f64>>,
    absolute: Option<Vec<f64>>,
    n_outputs: usize,
}

impl ModalChannels {
    /// Channels from the eigenvalues `lambda_r`, the damping terms `c_r = 2 zeta_r omega_r`, the gains `gains[a][o][r]`
    /// and the residuals `residuals[a][o]` (at least one pattern; every value finite).
    pub fn new(
        eigenvalues: Vec<f64>,
        damping: Vec<f64>,
        gains: Vec<Vec<Vec<f64>>>,
        residuals: Vec<Vec<f64>>,
    ) -> Result<ModalChannels> {
        let nm = eigenvalues.len();
        if damping.len() != nm {
            return Err(invalid(
                RANDOM,
                format!("{nm} eigenvalues and {} damping terms", damping.len()),
            ));
        }
        if gains.is_empty() || gains.len() != residuals.len() {
            return Err(invalid(
                RANDOM,
                format!(
                    "{} gain patterns and {} residual patterns (at least one)",
                    gains.len(),
                    residuals.len()
                ),
            ));
        }
        let no = gains[0].len();
        let mut flat = Vec::with_capacity(gains.len());
        for (a, (g, s)) in gains.iter().zip(&residuals).enumerate() {
            if g.len() != no || s.len() != no || g.iter().any(|row| row.len() != nm) {
                return Err(invalid(
                    RANDOM,
                    format!("pattern {a}: the gains must be {no} x {nm} and the residuals {no} long"),
                ));
            }
            let v: Vec<f64> = g.iter().flatten().copied().collect();
            if v.iter().chain(s).any(|x| !x.is_finite()) {
                return Err(invalid(
                    RANDOM,
                    format!("pattern {a}: a gain or residual is not finite"),
                ));
            }
            flat.push(v);
        }
        if eigenvalues.iter().chain(&damping).any(|x| !x.is_finite()) {
            return Err(invalid(
                RANDOM,
                "an eigenvalue or damping term is not finite".to_string(),
            ));
        }
        Ok(ModalChannels {
            eigenvalues,
            damping,
            gains: flat,
            residuals,
            absolute: None,
            n_outputs: no,
        })
    }

    /// The same channels with the absolute term `b_o` of a base acceleration (one pattern): the acceleration is `b_o -
    /// omega^2 Y_o`.
    pub fn with_absolute(mut self, absolute: Vec<f64>) -> Result<ModalChannels> {
        if self.gains.len() != 1 {
            return Err(invalid(
                RANDOM,
                format!("an absolute term needs one pattern, found {}", self.gains.len()),
            ));
        }
        if absolute.len() != self.n_outputs || absolute.iter().any(|x| !x.is_finite()) {
            return Err(invalid(
                RANDOM,
                format!("the absolute term must be {} finite values", self.n_outputs),
            ));
        }
        self.absolute = Some(absolute);
        Ok(self)
    }

    /// The number of input patterns.
    pub fn n_patterns(&self) -> usize {
        self.gains.len()
    }

    /// The number of outputs.
    pub fn n_outputs(&self) -> usize {
        self.n_outputs
    }

    /// The number of modes.
    pub fn n_modes(&self) -> usize {
        self.eigenvalues.len()
    }
}

/// Options of [`spectral_response`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SpectralOptions {
    /// The quantity (default displacement).
    pub quantity: Quantity,
    /// The size of the groups of consecutive outputs whose covariance is computed (default 1).
    pub group: usize,
    /// The grid refinement `R` (default 1, the registered grid).
    pub refinement: usize,
    /// Whether the output densities at the nodes are kept (default true).
    pub keep_psd: bool,
}

impl Default for SpectralOptions {
    fn default() -> Self {
        SpectralOptions {
            quantity: Quantity::Displacement,
            group: 1,
            refinement: 1,
            keep_psd: true,
        }
    }
}

impl SpectralOptions {
    /// The same options with a quantity.
    pub fn quantity(mut self, q: Quantity) -> SpectralOptions {
        self.quantity = q;
        self
    }
    /// The same options with a group size.
    pub fn group(mut self, g: usize) -> SpectralOptions {
        self.group = g;
        self
    }
    /// The same options with a refinement.
    pub fn refinement(mut self, r: usize) -> SpectralOptions {
        self.refinement = r;
        self
    }
    /// The same options keeping the densities or not.
    pub fn keep_psd(mut self, on: bool) -> SpectralOptions {
        self.keep_psd = on;
        self
    }
}

/// The result of [`spectral_response`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ChannelResponse {
    /// The quadrature nodes, Hz.
    pub frequencies: Vec<f64>,
    /// The quadrature weights, Hz.
    pub weights: Vec<f64>,
    /// The output densities at the nodes, `[output][node]` (empty unless kept).
    pub psd: Vec<Vec<f64>>,
    /// The moments of each output.
    pub moments: Vec<Moments>,
    /// The covariance `C_0` of each group, row-major `group x group`.
    pub covariance: Vec<Vec<f64>>,
    /// The covariance `C_2` weighted by `f^2`, row-major.
    pub rate_covariance: Vec<Vec<f64>>,
}

/// The response of modal channels to spectra `G_a` (one per pattern) with a correlation (`None`: uncorrelated), integrated
/// on the grid of [`frequency_grid`] (ADR 0477 D2 to D4).
pub fn spectral_response(
    channels: &ModalChannels,
    spectra: &[Spectrum],
    correlation: Option<&[Vec<f64>]>,
    options: &SpectralOptions,
) -> Result<ChannelResponse> {
    let np = channels.n_patterns();
    if spectra.len() != np {
        return Err(invalid(RANDOM, format!("{} spectra for {np} patterns", spectra.len())));
    }
    let rho: Vec<Vec<f64>> = match correlation {
        Some(c) => {
            check_correlation(c, np)?;
            c.to_vec()
        }
        None => (0..np)
            .map(|a| (0..np).map(|b| if a == b { 1.0 } else { 0.0 }).collect())
            .collect(),
    };
    let no = channels.n_outputs;
    let g = options.group;
    if g == 0 || no % g != 0 {
        return Err(invalid(
            RANDOM,
            format!("group size {g} does not divide the {no} outputs"),
        ));
    }
    let (nodes, weights) = frequency_grid(spectra, &channels.eigenvalues, &channels.damping, options.refinement)?;
    let nm = channels.n_modes();
    let ng = no / g;
    let mut psd = if options.keep_psd {
        vec![Vec::with_capacity(nodes.len()); no]
    } else {
        Vec::new()
    };
    let mut moments = vec![Moments::default(); no];
    let mut cov = vec![vec![0.0; g * g]; ng];
    let mut cov2 = vec![vec![0.0; g * g]; ng];
    let mut h = vec![(0.0, 0.0); nm];
    let mut yr = vec![vec![0.0; no]; np];
    let mut yi = vec![vec![0.0; no]; np];
    let mut gab = vec![vec![0.0; np]; np];
    let absolute = match options.quantity {
        Quantity::Acceleration => channels.absolute.as_deref(),
        _ => None,
    };
    for (&f, &w) in nodes.iter().zip(&weights) {
        let omega = 2.0 * PI * f;
        let w2 = omega * omega;
        for (r, hr) in h.iter_mut().enumerate() {
            let a = channels.eigenvalues[r] - w2;
            let b = channels.damping[r] * omega;
            let d = a * a + b * b;
            *hr = (a / d, -b / d);
        }
        let (fr, fi) = options.quantity.factor(omega);
        for a in 0..np {
            let gains = &channels.gains[a];
            let res = &channels.residuals[a];
            for o in 0..no {
                let mut re = res[o];
                let mut im = 0.0;
                for (r, &(hr, hi)) in h.iter().enumerate() {
                    let u = gains[o * nm + r];
                    re += u * hr;
                    im += u * hi;
                }
                let (mut zr, zi) = (re * fr - im * fi, re * fi + im * fr);
                if let Some(abs) = absolute {
                    zr += abs[o];
                }
                yr[a][o] = zr;
                yi[a][o] = zi;
            }
        }
        let gv: Vec<f64> = spectra.iter().map(|s| s.value(f)).collect();
        for a in 0..np {
            for b in 0..np {
                gab[a][b] = if a == b {
                    gv[a]
                } else {
                    rho[a][b] * (gv[a] * gv[b]).sqrt()
                };
            }
        }
        let cross = |i: usize, j: usize| -> f64 {
            let mut s = 0.0;
            for a in 0..np {
                for b in 0..np {
                    s += gab[a][b] * (yr[a][i] * yr[b][j] + yi[a][i] * yi[b][j]);
                }
            }
            s
        };
        for o in 0..no {
            let s = cross(o, o);
            let m = &mut moments[o];
            m.m0 += w * s;
            m.m1 += w * f * s;
            m.m2 += w * f * f * s;
            m.m4 += w * f * f * f * f * s;
            if options.keep_psd {
                psd[o].push(s);
            }
        }
        if g > 1 {
            for k in 0..ng {
                for i in 0..g {
                    for j in i..g {
                        let s = cross(k * g + i, k * g + j);
                        cov[k][i * g + j] += w * s;
                        cov2[k][i * g + j] += w * f * f * s;
                    }
                }
            }
        }
    }
    if g > 1 {
        for c in cov.iter_mut().chain(cov2.iter_mut()) {
            for i in 0..g {
                for j in 0..i {
                    c[i * g + j] = c[j * g + i];
                }
            }
        }
    } else {
        for (k, m) in moments.iter().enumerate() {
            cov[k][0] = m.m0;
            cov2[k][0] = m.m2;
        }
    }
    Ok(ChannelResponse {
        frequencies: nodes,
        weights,
        psd,
        moments,
        covariance: cov,
        rate_covariance: cov2,
    })
}

// ------------------------------------------------------------------------------------------------------------- stress

/// `E[sigma_vm^2] = tr(A C)` of a zero-mean random stress with the covariance `C` (Voigt xx, yy, zz, xy, yz, zx, tensor
/// shear): `C_11 + C_22 + C_33 - C_12 - C_23 - C_31 + 3 (C_44 + C_55 + C_66)` (ADR 0477 D5).
pub fn von_mises_mean_square(c: &[[f64; 6]; 6]) -> f64 {
    c[0][0] + c[1][1] + c[2][2] - c[0][1] - c[1][2] - c[2][0] + 3.0 * (c[3][3] + c[4][4] + c[5][5])
}

/// The random stress at the integration points of [`IntegrationPointField`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RandomStress {
    /// The covariance `C_0` at each point.
    pub covariance: Vec<[[f64; 6]; 6]>,
    /// The covariance `C_2` weighted by `f^2` at each point.
    pub rate_covariance: Vec<[[f64; 6]; 6]>,
    /// The positions of the points.
    pub positions: Vec<Vec3>,
}

impl RandomStress {
    /// The root-mean-square of component `c` at point `p`.
    pub fn rms(&self, p: usize, c: usize) -> f64 {
        self.covariance[p][c][c].sqrt()
    }

    /// The root-mean-square von Mises stress `sqrt(tr(A C_0))` at point `p`.
    pub fn rms_von_mises(&self, p: usize) -> f64 {
        von_mises_mean_square(&self.covariance[p]).max(0.0).sqrt()
    }

    /// The rate `sqrt(tr(A C_2)/tr(A C_0))` of the von Mises form at point `p`, Hz (`None` at zero).
    pub fn von_mises_rate(&self, p: usize) -> Option<f64> {
        let q0 = von_mises_mean_square(&self.covariance[p]);
        (q0 > 0.0).then(|| (von_mises_mean_square(&self.rate_covariance[p]) / q0).max(0.0).sqrt())
    }
}

fn to_6x6(v: &[f64]) -> [[f64; 6]; 6] {
    std::array::from_fn(|i| std::array::from_fn(|j| v[i * 6 + j]))
}

// ---------------------------------------------------------------------------------------------------- the solid model

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Forces {
        patterns: Vec<Vec<f64>>,
        spectra: Vec<Spectrum>,
        correlation: Option<Vec<Vec<f64>>>,
    },
    Base {
        direction: Vec3,
        spectrum: Spectrum,
    },
}

/// The random input of a solid model (ADR 0477 D6).
#[derive(Debug, Clone, PartialEq)]
pub struct Excitation {
    kind: Kind,
}

impl Excitation {
    /// One load pattern (one value per equation) with its force spectrum.
    pub fn force(pattern: Vec<f64>, spectrum: Spectrum) -> Excitation {
        Excitation {
            kind: Kind::Forces {
                patterns: vec![pattern],
                spectra: vec![spectrum],
                correlation: None,
            },
        }
    }

    /// Several patterns, one spectrum each, with a correlation matrix (`None`: uncorrelated; ADR 0477 D4).
    pub fn forces(
        patterns: Vec<Vec<f64>>,
        spectra: Vec<Spectrum>,
        correlation: Option<Vec<Vec<f64>>>,
    ) -> Result<Excitation> {
        if patterns.is_empty() || patterns.len() != spectra.len() {
            return Err(invalid(
                RANDOM,
                format!(
                    "{} patterns and {} spectra (at least one)",
                    patterns.len(),
                    spectra.len()
                ),
            ));
        }
        if let Some(c) = &correlation {
            check_correlation(c, patterns.len())?;
        }
        Ok(Excitation {
            kind: Kind::Forces {
                patterns,
                spectra,
                correlation,
            },
        })
    }

    /// A base acceleration along `direction` (normalised) with its spectrum: every prescribed equation moves with the base.
    pub fn base(direction: Vec3, spectrum: Spectrum) -> Result<Excitation> {
        let norm = (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2]).sqrt();
        if !(norm.is_finite() && norm > 0.0) {
            return Err(invalid(
                "base excitation",
                format!("direction {direction:?} must be finite and non-zero"),
            ));
        }
        Ok(Excitation {
            kind: Kind::Base {
                direction: [direction[0] / norm, direction[1] / norm, direction[2] / norm],
                spectrum,
            },
        })
    }
}

/// What a random response computes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RandomRequest {
    /// The output equations.
    pub equations: Vec<usize>,
    /// Their quantity (default displacement).
    pub quantity: Quantity,
    /// Whether the stress covariance is computed (default false).
    pub stress: bool,
    /// The grid refinement (default 1).
    pub refinement: usize,
}

impl RandomRequest {
    /// The displacement at these equations, no stress, the registered grid.
    pub fn new(equations: Vec<usize>) -> RandomRequest {
        RandomRequest {
            equations,
            quantity: Quantity::Displacement,
            stress: false,
            refinement: 1,
        }
    }
    /// The same request with a quantity.
    pub fn quantity(mut self, q: Quantity) -> RandomRequest {
        self.quantity = q;
        self
    }
    /// The same request with or without the stress.
    pub fn stress(mut self, on: bool) -> RandomRequest {
        self.stress = on;
        self
    }
    /// The same request with a refinement.
    pub fn refinement(mut self, r: usize) -> RandomRequest {
        self.refinement = r;
        self
    }
}

/// The random response of a solid model.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RandomResponse {
    /// The quadrature nodes, Hz.
    pub frequencies: Vec<f64>,
    /// The quadrature weights, Hz.
    pub weights: Vec<f64>,
    /// The output equations.
    pub equations: Vec<usize>,
    /// The density of each output at the nodes.
    pub psd: Vec<Vec<f64>>,
    /// The moments of each output.
    pub moments: Vec<Moments>,
    /// The stress, if requested.
    pub stress: Option<RandomStress>,
}

/// Random response of a solid model by mode superposition on the F-399 solver (ADR 0477 D6).
#[derive(Debug, Clone)]
pub struct RandomSolver {
    harmonic: HarmonicSolver,
    threads: Option<usize>,
}

impl RandomSolver {
    /// The solver of a model from a basis of its modes: the F-399 [`HarmonicSolver`] with these options (its modal damping
    /// and static correction, and its refusals).
    pub fn new(model: &SolidModel, basis: ModalBasis, options: &HarmonicOptions) -> Result<RandomSolver> {
        Ok(RandomSolver {
            harmonic: HarmonicSolver::new(model, basis, options)?,
            threads: options.threads,
        })
    }

    /// The underlying harmonic solver.
    pub fn harmonic(&self) -> &HarmonicSolver {
        &self.harmonic
    }

    /// The response to an excitation.
    pub fn response(
        &self,
        model: &SolidModel,
        excitation: &Excitation,
        request: &RandomRequest,
    ) -> Result<RandomResponse> {
        let basis = self.harmonic.basis();
        let dofs = model.dof_map();
        if &dofs != basis.dofs() {
            return Err(invalid(
                RANDOM,
                "the model numbers its unknowns differently from the solver's basis".to_string(),
            ));
        }
        let n = dofs.n_equations();
        if let Some(&e) = request.equations.iter().find(|&&e| e >= n) {
            return Err(invalid(RANDOM, format!("output equation {e} (the model has {n})")));
        }
        let (patterns, spectra, correlation, absolute) = match &excitation.kind {
            Kind::Forces {
                patterns,
                spectra,
                correlation,
            } => {
                if let Some(p) = patterns.iter().find(|p| p.len() != n) {
                    return Err(FemError::FieldLength {
                        key: "random load",
                        expected: n,
                        found: p.len(),
                    });
                }
                (patterns.clone(), spectra.clone(), correlation.clone(), None)
            }
            Kind::Base { direction, spectrum } => {
                let (load, r) = self.base_load(model, *direction)?;
                (vec![load], vec![spectrum.clone()], None, Some(r))
            }
        };
        let mut modal = Vec::with_capacity(patterns.len());
        let mut residual = Vec::with_capacity(patterns.len());
        for p in &patterns {
            let (q, x) = self.harmonic.modal_decomposition(p)?;
            modal.push(q);
            residual.push(x);
        }
        let modes = basis.modes();
        let eigenvalues = basis.eigenvalues().to_vec();
        let damping = self.harmonic.damping_coefficients().to_vec();
        let gains: Vec<Vec<Vec<f64>>> = modal
            .iter()
            .map(|q| {
                request
                    .equations
                    .iter()
                    .map(|&e| modes.iter().zip(q).map(|(phi, qr)| phi[e] * qr).collect())
                    .collect()
            })
            .collect();
        let residuals: Vec<Vec<f64>> = residual
            .iter()
            .map(|x| {
                request
                    .equations
                    .iter()
                    .map(|&e| x.as_ref().map_or(0.0, |x| x[e]))
                    .collect()
            })
            .collect();
        let mut channels = ModalChannels::new(eigenvalues.clone(), damping.clone(), gains, residuals)?;
        if let Some(r) = &absolute {
            channels = channels.with_absolute(request.equations.iter().map(|&e| r[e]).collect())?;
        }
        let corr = correlation.as_deref();
        let out = spectral_response(
            &channels,
            &spectra,
            corr,
            &SpectralOptions::default()
                .quantity(request.quantity)
                .refinement(request.refinement),
        )?;
        let stress = if request.stress {
            let zero = IntegrationPointField::compute(model, &vec![0.0; n])?;
            let sigma = |x: &[f64]| -> Result<Vec<f64>> {
                let field = IntegrationPointField::compute(model, x)?;
                Ok(field
                    .stress()
                    .iter()
                    .zip(zero.stress())
                    .flat_map(|(s, z)| (0..6).map(move |c| s[c] - z[c]))
                    .collect())
            };
            let modal_stress: Vec<Vec<f64>> = modes.iter().map(|phi| sigma(phi)).collect::<Result<_>>()?;
            let np6 = 6 * zero.n_points();
            let gains: Vec<Vec<Vec<f64>>> = modal
                .iter()
                .map(|q| {
                    (0..np6)
                        .map(|k| modal_stress.iter().zip(q).map(|(s, qr)| s[k] * qr).collect())
                        .collect()
                })
                .collect();
            let residuals: Vec<Vec<f64>> = residual
                .iter()
                .map(|x| match x {
                    Some(x) => sigma(x),
                    None => Ok(vec![0.0; np6]),
                })
                .collect::<Result<_>>()?;
            let ch = ModalChannels::new(eigenvalues, damping, gains, residuals)?;
            let s = spectral_response(
                &ch,
                &spectra,
                corr,
                &SpectralOptions::default()
                    .group(6)
                    .refinement(request.refinement)
                    .keep_psd(false),
            )?;
            Some(RandomStress {
                covariance: s.covariance.iter().map(|c| to_6x6(c)).collect(),
                rate_covariance: s.rate_covariance.iter().map(|c| to_6x6(c)).collect(),
                positions: zero.positions().to_vec(),
            })
        } else {
            None
        };
        Ok(RandomResponse {
            frequencies: out.frequencies,
            weights: out.weights,
            equations: request.equations.clone(),
            psd: out.psd,
            moments: out.moments,
            stress,
        })
    }

    /// The load `-M r` of a unit base acceleration and the translation `r` (ADR 0477 D6).
    fn base_load(&self, model: &SolidModel, d: Vec3) -> Result<(Vec<f64>, Vec<f64>)> {
        const KEY: &str = "base excitation";
        if crate::mpc::uses_transformation(model) {
            return Err(invalid(
                KEY,
                "linear constraints, rigid sets, cyclic constraints and elastic supports are not taken by a base \
                 excitation (ADR 0477: the supports must move as one rigid base)"
                    .to_string(),
            ));
        }
        let fixed = prescribed_equations(model, &[])?;
        if fixed.is_empty() {
            return Err(invalid(
                KEY,
                "the model has no prescribed equation: there is no base to move".to_string(),
            ));
        }
        if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
            return Err(invalid(
                KEY,
                format!("prescribed value {v:e} at equation {eq}: a base excitation takes zero values only"),
            ));
        }
        let m = consistent_with(model, self.threads)?;
        let rigid = rigid_motions(model, [0.0; 3]);
        let r: Vec<f64> = (0..m.n)
            .map(|i| d[0] * rigid[0][i] + d[1] * rigid[1][i] + d[2] * rigid[2][i])
            .collect();
        let mut mr = vec![0.0; m.n];
        m.matvec(&r, &mut mr);
        let mut load: Vec<f64> = mr.iter().map(|x| -x).collect();
        for &(eq, _) in &fixed {
            load[eq] = 0.0;
        }
        Ok((load, r))
    }
}
