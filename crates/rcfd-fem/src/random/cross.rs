//! Complex cross-spectral matrices and frequency-dependent coherence of random loads (ADR 0510, F-567).
//!
//! The cross-spectral density of the load spectra `F_a(omega)` is `G_ab(f) = E[conj(F_a) F_b]` per Hz: Hermitian
//! (`G_ba = conj(G_ab)`) and positive semi-definite at every frequency ([`check_cross_matrix`], refused with the frequency
//! otherwise). The output cross-density between outputs `i` and `j` is `S_ij = Re sum_ab conj(Y_ia) Y_jb G_ab`; its real part
//! `Gr (yr yr + yi yi)` is the term of ADR 0477, the imaginary part adds `- Gi (yr_ia yi_jb - yi_ia yr_jb)`.
//!
//! * [`CrossSpectrum::table`]: the table `G_ab(f_k)` with real and imaginary parts, linear interpolation of every entry.
//! * [`CrossSpectrum::coherent`]: spectra `G_a` and a [`Coherence`] `gamma_ab(f)` (Hermitian, unit diagonal), `G_ab = gamma_ab
//!   sqrt(G_a G_b)`; the coherence is a real constant matrix ([`Coherence::constant`], the correlation of ADR 0477), a table, or
//!   a function of the frequency ([`Coherence::function`]).
//! * [`exponential_coherence`]: the matrix `exp(-|x_a - x_b|/L)` of a field of points.
//! * [`spectral_response_cross`]: the integral on the grid of ADR 0477 D3; a real constant coherence is handed to
//!   [`spectral_response`] (ADR 0510 D3), so its result is the F-417 result bit for bit.

use super::*;
use std::sync::Arc;

/// The tolerance of [`check_cross_matrix`] (ADR 0510 D1), relative to the largest diagonal entry (symmetry) and to the trace
/// (positive semi-definiteness).
pub const CROSS_TOLERANCE: f64 = 1e-12;

const CROSS: &str = "cross spectrum";

/// Checks a cross-spectral matrix `G = re + i im` (`n x n`) at the frequency `f`, Hz (ADR 0510 D1): finite, real non-negative
/// diagonal, Hermitian to [`CROSS_TOLERANCE`] times the largest diagonal entry, and positive semi-definite in the sense that
/// the smallest eigenvalue of the Hermitian part is at least `-CROSS_TOLERANCE` times the trace (the complex Cholesky
/// factorisation of the part plus that shift has positive pivots). An all-zero matrix is accepted. Every refusal names the
/// frequency.
pub fn check_cross_matrix(f: f64, re: &[Vec<f64>], im: &[Vec<f64>]) -> Result<()> {
    let n = re.len();
    if n == 0 || im.len() != n || re.iter().chain(im).any(|r| r.len() != n) {
        return Err(invalid(
            CROSS,
            format!("at {f:e} Hz: the real and imaginary parts must be square matrices of the same size"),
        ));
    }
    for a in 0..n {
        for b in 0..n {
            if !(re[a][b].is_finite() && im[a][b].is_finite()) {
                return Err(invalid(
                    CROSS,
                    format!(
                        "at {f:e} Hz: entry ({a}, {b}) is not finite ({:e}, {:e})",
                        re[a][b], im[a][b]
                    ),
                ));
            }
        }
    }
    let scale = (0..n).map(|a| re[a][a]).fold(0.0, f64::max);
    let trace: f64 = (0..n).map(|a| re[a][a]).sum();
    let tol = CROSS_TOLERANCE * scale;
    for a in 0..n {
        if re[a][a] < 0.0 {
            return Err(invalid(
                CROSS,
                format!("at {f:e} Hz: diagonal entry {a} is negative ({:e})", re[a][a]),
            ));
        }
        if im[a][a].abs() > tol {
            return Err(invalid(
                CROSS,
                format!(
                    "at {f:e} Hz: diagonal entry {a} has the imaginary part {:e} (must be real)",
                    im[a][a]
                ),
            ));
        }
        for b in a + 1..n {
            let dr = (re[a][b] - re[b][a]).abs();
            let di = (im[a][b] + im[b][a]).abs();
            if dr > tol || di > tol {
                return Err(invalid(
                    CROSS,
                    format!(
                        "at {f:e} Hz: not Hermitian, entries ({a}, {b}) = ({:e}, {:e}) and ({b}, {a}) = ({:e}, {:e}): G_ba must be the conjugate of G_ab",
                        re[a][b], im[a][b], re[b][a], im[b][a]
                    ),
                ));
            }
        }
    }
    if re.iter().chain(im).all(|r| r.iter().all(|&x| x == 0.0)) {
        return Ok(());
    }
    // L L^H = H + eps I, H the Hermitian part.
    let eps = CROSS_TOLERANCE * trace;
    let mut lr = vec![vec![0.0; n]; n];
    let mut li = vec![vec![0.0; n]; n];
    for j in 0..n {
        let mut d = re[j][j] + eps;
        for k in 0..j {
            d -= lr[j][k] * lr[j][k] + li[j][k] * li[j][k];
        }
        if d.is_nan() || d <= 0.0 {
            return Err(invalid(
                CROSS,
                format!(
                    "at {f:e} Hz: not positive semi-definite: pivot {j} of the Hermitian part plus {CROSS_TOLERANCE:e} times the trace is {d:e}"
                ),
            ));
        }
        let d = d.sqrt();
        lr[j][j] = d;
        for i in j + 1..n {
            let mut sr = 0.5 * (re[i][j] + re[j][i]);
            let mut si = 0.5 * (im[i][j] - im[j][i]);
            for k in 0..j {
                // L_ik conj(L_jk)
                sr -= lr[i][k] * lr[j][k] + li[i][k] * li[j][k];
                si -= li[i][k] * lr[j][k] - lr[i][k] * li[j][k];
            }
            lr[i][j] = sr / d;
            li[i][j] = si / d;
        }
    }
    Ok(())
}

type Fun = Arc<dyn Fn(f64, usize, usize) -> (f64, f64) + Send + Sync>;

#[derive(Clone)]
enum CoherenceKind {
    Constant(Vec<Vec<f64>>),
    Table {
        frequencies: Vec<f64>,
        re: Vec<Vec<Vec<f64>>>,
        im: Vec<Vec<Vec<f64>>>,
    },
    Function(Fun),
}

/// A coherence `gamma_ab(f)`: Hermitian, unit diagonal, positive semi-definite (ADR 0510 D2).
#[derive(Clone)]
pub struct Coherence {
    n: usize,
    kind: CoherenceKind,
}

impl std::fmt::Debug for Coherence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match &self.kind {
            CoherenceKind::Constant(_) => "constant",
            CoherenceKind::Table { .. } => "table",
            CoherenceKind::Function(_) => "function",
        };
        write!(f, "Coherence {{ n: {}, kind: {kind} }}", self.n)
    }
}

impl PartialEq for Coherence {
    fn eq(&self, other: &Coherence) -> bool {
        self.n == other.n
            && match (&self.kind, &other.kind) {
                (CoherenceKind::Constant(a), CoherenceKind::Constant(b)) => a == b,
                (
                    CoherenceKind::Table {
                        frequencies: fa,
                        re: ra,
                        im: ia,
                    },
                    CoherenceKind::Table {
                        frequencies: fb,
                        re: rb,
                        im: ib,
                    },
                ) => fa == fb && ra == rb && ia == ib,
                (CoherenceKind::Function(a), CoherenceKind::Function(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

fn check_table_shape(
    what: &str,
    frequencies: &[f64],
    re: &[Vec<Vec<f64>>],
    im: &[Vec<Vec<f64>>],
    min_entries: usize,
) -> Result<usize> {
    let k = frequencies.len();
    if k < min_entries || re.len() != k || im.len() != k {
        return Err(invalid(
            CROSS,
            format!(
                "{what}: {k} frequencies, {} real and {} imaginary matrices (at least {min_entries}, one matrix per frequency)",
                re.len(),
                im.len()
            ),
        ));
    }
    for (i, &f) in frequencies.iter().enumerate() {
        if !f.is_finite() || f < 0.0 {
            return Err(invalid(
                CROSS,
                format!("{what}: entry {i}: frequency {f:e} must be finite and >= 0"),
            ));
        }
        if i > 0 && f <= frequencies[i - 1] {
            return Err(invalid(
                CROSS,
                format!(
                    "{what}: entry {i}: frequency {f:e} is not above entry {} ({:e}): the frequencies must increase",
                    i - 1,
                    frequencies[i - 1]
                ),
            ));
        }
    }
    let n = re[0].len();
    for i in 0..k {
        if n == 0 || re[i].len() != n || im[i].len() != n || re[i].iter().chain(&im[i]).any(|r| r.len() != n) {
            return Err(invalid(
                CROSS,
                format!(
                    "{what}: entry {i} ({:e} Hz): the matrices must all be {n} x {n}",
                    frequencies[i]
                ),
            ));
        }
    }
    Ok(n)
}

/// The linear interpolation `g0 + (f - f0)/(f1 - f0) (g1 - g0)` of ADR 0477 D1, at the table row `i` and the frequency `f`.
fn lerp(f: f64, f0: f64, f1: f64, g0: f64, g1: f64) -> f64 {
    g0 + (f - f0) / (f1 - f0) * (g1 - g0)
}

impl Coherence {
    /// A real constant coherence (the correlation matrix of ADR 0477 D4: unit diagonal, symmetric, positive semi-definite).
    pub fn constant(rho: Vec<Vec<f64>>) -> Result<Coherence> {
        let n = rho.len();
        check_correlation(&rho, n)?;
        Ok(Coherence {
            n,
            kind: CoherenceKind::Constant(rho),
        })
    }

    /// A table `gamma_ab(f_k)` (`re[k][a][b]`, `im[k][a][b]`): the frequencies increase, every entry is interpolated linearly
    /// and the end rows hold outside the table; each matrix has a unit diagonal and passes [`check_cross_matrix`] (refused
    /// with the table frequency otherwise).
    pub fn table(frequencies: Vec<f64>, re: Vec<Vec<Vec<f64>>>, im: Vec<Vec<Vec<f64>>>) -> Result<Coherence> {
        let n = check_table_shape("coherence table", &frequencies, &re, &im, 1)?;
        for (k, &f) in frequencies.iter().enumerate() {
            for a in 0..n {
                if re[k][a][a] != 1.0 || im[k][a][a] != 0.0 {
                    return Err(invalid(
                        CROSS,
                        format!(
                            "coherence table at {f:e} Hz: diagonal entry {a} is ({:e}, {:e}), must be (1, 0)",
                            re[k][a][a], im[k][a][a]
                        ),
                    ));
                }
            }
            check_cross_matrix(f, &re[k], &im[k])?;
        }
        Ok(Coherence {
            n,
            kind: CoherenceKind::Table { frequencies, re, im },
        })
    }

    /// A coherence of `n` patterns given by a function `(f, a, b) -> (re, im)` for `a < b` (the lower triangle is the conjugate,
    /// the diagonal is 1). The matrix is checked by [`check_cross_matrix`] at every quadrature node of a response (refused
    /// with the frequency).
    pub fn function(n: usize, gamma: impl Fn(f64, usize, usize) -> (f64, f64) + Send + Sync + 'static) -> Coherence {
        Coherence {
            n,
            kind: CoherenceKind::Function(Arc::new(gamma)),
        }
    }

    /// The number of patterns.
    pub fn n_patterns(&self) -> usize {
        self.n
    }

    /// The matrix at `f` in `re`, `im` (row-major `n x n`).
    fn fill(&self, f: f64, re: &mut [f64], im: &mut [f64]) {
        let n = self.n;
        match &self.kind {
            CoherenceKind::Constant(rho) => {
                for a in 0..n {
                    for b in 0..n {
                        re[a * n + b] = rho[a][b];
                        im[a * n + b] = 0.0;
                    }
                }
            }
            CoherenceKind::Table {
                frequencies,
                re: tr,
                im: ti,
            } => {
                let k = frequencies.len();
                if f <= frequencies[0] {
                    for a in 0..n {
                        for b in 0..n {
                            re[a * n + b] = tr[0][a][b];
                            im[a * n + b] = ti[0][a][b];
                        }
                    }
                } else if f >= frequencies[k - 1] {
                    for a in 0..n {
                        for b in 0..n {
                            re[a * n + b] = tr[k - 1][a][b];
                            im[a * n + b] = ti[k - 1][a][b];
                        }
                    }
                } else {
                    let i = frequencies.partition_point(|&x| x <= f).clamp(1, k - 1) - 1;
                    let (f0, f1) = (frequencies[i], frequencies[i + 1]);
                    for a in 0..n {
                        for b in 0..n {
                            re[a * n + b] = lerp(f, f0, f1, tr[i][a][b], tr[i + 1][a][b]);
                            im[a * n + b] = lerp(f, f0, f1, ti[i][a][b], ti[i + 1][a][b]);
                        }
                    }
                }
            }
            CoherenceKind::Function(g) => {
                for a in 0..n {
                    re[a * n + a] = 1.0;
                    im[a * n + a] = 0.0;
                    for b in a + 1..n {
                        let (r, i) = g(f, a, b);
                        re[a * n + b] = r;
                        im[a * n + b] = i;
                        re[b * n + a] = r;
                        im[b * n + a] = -i;
                    }
                }
            }
        }
    }

    /// The real constant matrix of this coherence if it is one: a constant, or a table with every imaginary entry exactly 0
    /// and the same real matrix at every frequency (ADR 0510 D3.1), which also passes [`check_correlation`].
    fn real_constant(&self) -> Option<Vec<Vec<f64>>> {
        let rho = match &self.kind {
            CoherenceKind::Constant(rho) => rho.clone(),
            CoherenceKind::Table { re, im, .. } => {
                if im.iter().flatten().flatten().any(|&x| x != 0.0) || re.iter().any(|m| *m != re[0]) {
                    return None;
                }
                re[0].clone()
            }
            CoherenceKind::Function(_) => return None,
        };
        check_correlation(&rho, self.n).ok()?;
        Some(rho)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CrossKind {
    Table {
        frequencies: Vec<f64>,
        re: Vec<Vec<Vec<f64>>>,
        im: Vec<Vec<Vec<f64>>>,
    },
    Coherent {
        spectra: Vec<Spectrum>,
        coherence: Coherence,
    },
}

/// The cross-spectral densities of several load patterns (ADR 0510 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct CrossSpectrum {
    n: usize,
    kind: CrossKind,
}

impl CrossSpectrum {
    /// The table `G_ab(f_k) = re[k][a][b] + i im[k][a][b]` (units^2/Hz): at least two increasing frequencies, linear
    /// interpolation of every entry, zero outside `[f_0, f_last]`; every matrix passes [`check_cross_matrix`] (Hermitian and
    /// positive semi-definite, refused with the table frequency otherwise).
    pub fn table(frequencies: Vec<f64>, re: Vec<Vec<Vec<f64>>>, im: Vec<Vec<Vec<f64>>>) -> Result<CrossSpectrum> {
        let n = check_table_shape("cross-spectral table", &frequencies, &re, &im, 2)?;
        for (k, &f) in frequencies.iter().enumerate() {
            check_cross_matrix(f, &re[k], &im[k])?;
        }
        Ok(CrossSpectrum {
            n,
            kind: CrossKind::Table { frequencies, re, im },
        })
    }

    /// Spectra `G_a` (one per pattern) and a coherence: `G_aa = G_a`, `G_ab = gamma_ab sqrt(G_a G_b)`.
    pub fn coherent(spectra: Vec<Spectrum>, coherence: Coherence) -> Result<CrossSpectrum> {
        if spectra.is_empty() || spectra.len() != coherence.n {
            return Err(invalid(
                CROSS,
                format!(
                    "{} spectra for a coherence of {} patterns (at least one)",
                    spectra.len(),
                    coherence.n
                ),
            ));
        }
        Ok(CrossSpectrum {
            n: spectra.len(),
            kind: CrossKind::Coherent { spectra, coherence },
        })
    }

    /// The number of patterns.
    pub fn n_patterns(&self) -> usize {
        self.n
    }

    /// The break frequencies of the quadrature grid.
    fn breaks(&self) -> Vec<f64> {
        match &self.kind {
            CrossKind::Table { frequencies, .. } => frequencies.clone(),
            CrossKind::Coherent { spectra, coherence } => {
                let mut b: Vec<f64> = spectra.iter().flat_map(|s| s.frequencies.iter().copied()).collect();
                if let CoherenceKind::Table { frequencies, .. } = &coherence.kind {
                    let lo = b.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = b.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    b.extend(frequencies.iter().copied().filter(|&f| f > lo && f < hi));
                }
                b
            }
        }
    }

    /// `G(f)` in `gr`, `gi` (row-major), checking a function coherence at `f`.
    fn matrix_at(&self, f: f64, work: &mut Work) -> Result<()> {
        let n = self.n;
        match &self.kind {
            CrossKind::Table {
                frequencies,
                re: tr,
                im: ti,
            } => {
                let k = frequencies.len();
                if !(f >= frequencies[0] && f <= frequencies[k - 1]) {
                    work.gr.iter_mut().chain(work.gi.iter_mut()).for_each(|x| *x = 0.0);
                    return Ok(());
                }
                let i = frequencies.partition_point(|&x| x <= f).clamp(1, k - 1) - 1;
                let (f0, f1) = (frequencies[i], frequencies[i + 1]);
                for a in 0..n {
                    for b in 0..n {
                        work.gr[a * n + b] = lerp(f, f0, f1, tr[i][a][b], tr[i + 1][a][b]);
                        work.gi[a * n + b] = lerp(f, f0, f1, ti[i][a][b], ti[i + 1][a][b]);
                    }
                }
            }
            CrossKind::Coherent { spectra, coherence } => {
                coherence.fill(f, &mut work.cr, &mut work.ci);
                if let CoherenceKind::Function(_) = coherence.kind {
                    let re: Vec<Vec<f64>> = (0..n).map(|a| work.cr[a * n..(a + 1) * n].to_vec()).collect();
                    let im: Vec<Vec<f64>> = (0..n).map(|a| work.ci[a * n..(a + 1) * n].to_vec()).collect();
                    check_cross_matrix(f, &re, &im)?;
                }
                for (g, s) in work.gv.iter_mut().zip(spectra) {
                    *g = s.value(f);
                }
                for a in 0..n {
                    for b in 0..n {
                        if a == b {
                            work.gr[a * n + b] = work.gv[a];
                            work.gi[a * n + b] = 0.0;
                        } else {
                            let root = (work.gv[a] * work.gv[b]).sqrt();
                            work.gr[a * n + b] = work.cr[a * n + b] * root;
                            work.gi[a * n + b] = work.ci[a * n + b] * root;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

struct Work {
    gr: Vec<f64>,
    gi: Vec<f64>,
    cr: Vec<f64>,
    ci: Vec<f64>,
    gv: Vec<f64>,
}

/// The matrix `exp(-|x_a - x_b|/L)` of points `x` (unit diagonal; `L` finite and positive): the coherence of a field with the
/// correlation length `L` (ADR 0510).
pub fn exponential_coherence(positions: &[Vec3], length: f64) -> Result<Vec<Vec<f64>>> {
    if !(length.is_finite() && length > 0.0) {
        return Err(invalid(
            CROSS,
            format!("the correlation length {length:e} must be finite and positive"),
        ));
    }
    let n = positions.len();
    let mut m = vec![vec![1.0; n]; n];
    for a in 0..n {
        for b in a + 1..n {
            let d = positions[a]
                .iter()
                .zip(&positions[b])
                .map(|(p, q)| (p - q) * (p - q))
                .sum::<f64>()
                .sqrt();
            if !d.is_finite() {
                return Err(invalid(
                    CROSS,
                    format!("the distance between points {a} and {b} is not finite"),
                ));
            }
            let g = (-d / length).exp();
            m[a][b] = g;
            m[b][a] = g;
        }
    }
    Ok(m)
}

/// The response of modal channels to a cross-spectrum, on the grid of [`frequency_grid`] (ADR 0510). A real constant coherence
/// is handed to [`spectral_response`] unless `options.real_route` is off.
pub fn spectral_response_cross(
    channels: &ModalChannels,
    cross: &CrossSpectrum,
    options: &SpectralOptions,
) -> Result<ChannelResponse> {
    let np = channels.n_patterns();
    if cross.n != np {
        return Err(invalid(
            RANDOM,
            format!("a cross-spectrum of {} patterns for {np} patterns", cross.n),
        ));
    }
    if options.real_route {
        if let CrossKind::Coherent { spectra, coherence } = &cross.kind {
            if let Some(rho) = coherence.real_constant() {
                return spectral_response(channels, spectra, Some(&rho), options);
            }
        }
    }
    if options.refinement == 0 {
        return Err(invalid(RANDOM, "the grid refinement must be at least 1".to_string()));
    }
    let no = channels.n_outputs;
    let g = options.group;
    if g == 0 || no % g != 0 {
        return Err(invalid(
            RANDOM,
            format!("group size {g} does not divide the {no} outputs"),
        ));
    }
    let (nodes, weights) = grid_from_breaks(
        cross.breaks(),
        &channels.eigenvalues,
        &channels.damping,
        options.refinement,
    )?;
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
    let mut cov1 = vec![vec![0.0; g * g]; ng];
    let mut cov4 = vec![vec![0.0; g * g]; ng];
    let mut h = vec![(0.0, 0.0); nm];
    let mut yr = vec![vec![0.0; no]; np];
    let mut yi = vec![vec![0.0; no]; np];
    let mut work = Work {
        gr: vec![0.0; np * np],
        gi: vec![0.0; np * np],
        cr: vec![0.0; np * np],
        ci: vec![0.0; np * np],
        gv: vec![0.0; np],
    };
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
        cross.matrix_at(f, &mut work)?;
        let (gr, gi) = (&work.gr, &work.gi);
        let pair = |i: usize, j: usize| -> f64 {
            let mut s = 0.0;
            let mut t = 0.0;
            for a in 0..np {
                for b in 0..np {
                    s += gr[a * np + b] * (yr[a][i] * yr[b][j] + yi[a][i] * yi[b][j]);
                    t += gi[a * np + b] * (yr[a][i] * yi[b][j] - yi[a][i] * yr[b][j]);
                }
            }
            s - t
        };
        for o in 0..no {
            let s = pair(o, o);
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
                        let s = pair(k * g + i, k * g + j);
                        cov[k][i * g + j] += w * s;
                        cov2[k][i * g + j] += w * f * f * s;
                        cov1[k][i * g + j] += w * f * s;
                        cov4[k][i * g + j] += w * f * f * f * f * s;
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
        for c in cov1.iter_mut().chain(cov4.iter_mut()) {
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
            cov1[k][0] = m.m1;
            cov4[k][0] = m.m4;
        }
    }
    Ok(ChannelResponse {
        frequencies: nodes,
        weights,
        psd,
        moments,
        covariance: cov,
        rate_covariance: cov2,
        first_covariance: cov1,
        fourth_covariance: cov4,
    })
}
