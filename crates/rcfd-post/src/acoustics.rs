//! Acoustic metrics: SPL, OASPL, A-weighting, power spectra, 1/3-octave band synthesis, sound power from an observer
//! sphere, and CSV/JSON writers of spectra (ADR 0066; formulas in `docs/acoustics.md`).
//!
//! References: A-weighting constants as printed on the Wikipedia A-weighting page and metricgate.com (IEC 61672-1 itself
//! was not read); base-10 1/3-octave bands as in IEC 61260-1 (not read).

use serde_json::{json, Value};

/// Reference pressure for sound pressure level in air, Pa.
pub const P_REF: f64 = 20e-6;

/// Error of the acoustics metrics; messages name the offending argument or sample.
#[derive(Debug, Clone, PartialEq)]
pub struct AcousticsError(pub String);

impl std::fmt::Display for AcousticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AcousticsError {}

type Res<T> = Result<T, AcousticsError>;

fn err<T>(m: String) -> Res<T> {
    Err(AcousticsError(m))
}

/// Level in dB re `P_REF` of a mean-square pressure; `None` for zero energy (no NaN, no infinity).
pub fn level_from_ms(ms: f64) -> Res<Option<f64>> {
    if !ms.is_finite() || ms < 0.0 {
        return err(format!("ms: mean-square pressure must be finite and >= 0 (got {ms})"));
    }
    if ms == 0.0 {
        return Ok(None);
    }
    Ok(Some(10.0 * (ms / (P_REF * P_REF)).log10()))
}

/// Sound pressure level `20 log10(p_rms / 20 uPa)`. Errors for negative or non-finite `p_rms` and for zero (undefined).
pub fn spl_db(p_rms: f64) -> Res<f64> {
    if !p_rms.is_finite() || p_rms < 0.0 {
        return err(format!("p_rms: must be finite and >= 0 (got {p_rms})"));
    }
    if p_rms == 0.0 {
        return err("p_rms: zero rms pressure, SPL is undefined".into());
    }
    Ok(20.0 * (p_rms / P_REF).log10())
}

fn check_series(signal: &[f64], dt: f64) -> Res<()> {
    if !(dt.is_finite() && dt > 0.0) {
        return err(format!("dt: must be finite and positive (got {dt})"));
    }
    if signal.len() < 8 {
        return err(format!("samples: need at least 8 samples (got {})", signal.len()));
    }
    if let Some(i) = signal.iter().position(|x| !x.is_finite()) {
        return err(format!("sample {i}: not finite ({})", signal[i]));
    }
    Ok(())
}

/// Mean square of the fluctuation (mean removed) of a series.
pub fn mean_square(signal: &[f64]) -> Res<f64> {
    if signal.is_empty() {
        return err("samples: empty series".into());
    }
    if let Some(i) = signal.iter().position(|x| !x.is_finite()) {
        return err(format!("sample {i}: not finite ({})", signal[i]));
    }
    let n = signal.len() as f64;
    let mean = signal.iter().sum::<f64>() / n;
    Ok(signal.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n)
}

/// Overall sound pressure level of a time series (mean removed). Errors if the fluctuation is identically zero.
pub fn oaspl_time(signal: &[f64]) -> Res<f64> {
    let ms = mean_square(signal)?;
    match level_from_ms(ms)? {
        Some(l) => Ok(l),
        None => err("signal: zero fluctuation, OASPL is undefined".into()),
    }
}

/// A-weighting in dB, `20 log10(R_A(f)/R_A(1000))` with `R_A(f) = 12194^2 f^4 / ((f^2 + 20.6^2) sqrt((f^2 + 107.7^2)
/// (f^2 + 737.9^2)) (f^2 + 12194^2))`; exactly 0 at 1 kHz.
pub fn a_weighting_db(f: f64) -> Res<f64> {
    if !f.is_finite() || f <= 0.0 {
        return err(format!("f: frequency must be finite and positive (got {f})"));
    }
    fn ra(f: f64) -> f64 {
        let f2 = f * f;
        let d = 12194.0_f64;
        d * d * f2 * f2 / ((f2 + 20.6 * 20.6) * ((f2 + 107.7 * 107.7) * (f2 + 737.9 * 737.9)).sqrt() * (f2 + d * d))
    }
    Ok(20.0 * (ra(f) / ra(1000.0)).log10())
}

/// In-place radix-2 FFT (forward, `e^{-i...}`); the length must be a power of two.
pub fn fft(re: &mut [f64], im: &mut [f64]) -> Res<()> {
    let n = re.len();
    if im.len() != n {
        return err(format!("fft: re and im lengths differ ({n} vs {})", im.len()));
    }
    if n == 0 || !n.is_power_of_two() {
        return err(format!("fft: length must be a power of two (got {n})"));
    }
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (ang * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * c - im[b] * s;
                let ti = re[b] * s + im[b] * c;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        len <<= 1;
    }
    Ok(())
}

/// Analysis window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// No window.
    Rectangular,
    /// Periodic Hann window `0.5 (1 - cos(2 pi n/N))`.
    Hann,
}

/// One-sided mean-square spectrum on the bins `k df`, `k = 0..=N/2`, normalised so that the sum of `ms` equals the
/// (windowed) mean square of the fluctuation (Parseval).
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    /// Bin spacing, Hz.
    pub df: f64,
    /// Bin frequencies, Hz.
    pub freq: Vec<f64>,
    /// Mean square per bin, Pa^2.
    pub ms: Vec<f64>,
    /// Number of samples used (largest power of two not above the series length).
    pub n_used: usize,
}

/// Power spectrum of the first `2^m` samples of `signal` (the largest power of two not above its length), mean removed.
pub fn power_spectrum(signal: &[f64], dt: f64, window: Window) -> Res<Spectrum> {
    check_series(signal, dt)?;
    let mut n = 1usize;
    while n * 2 <= signal.len() {
        n *= 2;
    }
    let mean = signal[..n].iter().sum::<f64>() / n as f64;
    let w: Vec<f64> = (0..n)
        .map(|i| match window {
            Window::Rectangular => 1.0,
            Window::Hann => 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos()),
        })
        .collect();
    let sw2: f64 = w.iter().map(|x| x * x).sum();
    let mut re: Vec<f64> = (0..n).map(|i| (signal[i] - mean) * w[i]).collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im)?;
    let half = n / 2;
    let mut ms = Vec::with_capacity(half + 1);
    for k in 0..=half {
        let p = re[k] * re[k] + im[k] * im[k];
        let fold = if k == 0 || k == half { 1.0 } else { 2.0 };
        ms.push(fold * p / (n as f64 * sw2));
    }
    let df = 1.0 / (n as f64 * dt);
    Ok(Spectrum {
        df,
        freq: (0..=half).map(|k| k as f64 * df).collect(),
        ms,
        n_used: n,
    })
}

impl Spectrum {
    /// Sum of all bins (the windowed mean square).
    pub fn total_ms(&self) -> f64 {
        self.ms.iter().sum()
    }

    /// Overall level from the spectrum; `None` for zero energy.
    pub fn oaspl(&self) -> Res<Option<f64>> {
        level_from_ms(self.total_ms())
    }

    /// A-weighted overall level (bins weighted individually, DC skipped); `None` for zero energy.
    pub fn oaspl_a(&self) -> Res<Option<f64>> {
        let mut s = 0.0;
        for (f, m) in self.freq.iter().zip(&self.ms).skip(1) {
            s += m * 10f64.powf(0.1 * a_weighting_db(*f)?);
        }
        level_from_ms(s)
    }
}

/// One base-10 1/3-octave band.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// Band index `n` (centre `1000 * 10^(n/10)`).
    pub n: i32,
    /// Nominal (preferred number) label, Hz.
    pub nominal: f64,
    /// Exact centre frequency, Hz.
    pub centre: f64,
    /// Lower edge (inclusive), Hz.
    pub lo: f64,
    /// Upper edge (exclusive), Hz.
    pub hi: f64,
    /// Mean square in the band, Pa^2.
    pub ms: f64,
}

impl Band {
    /// Band level in dB re 20 uPa; `None` for zero energy.
    pub fn level(&self) -> Option<f64> {
        if self.ms > 0.0 {
            Some(10.0 * (self.ms / (P_REF * P_REF)).log10())
        } else {
            None
        }
    }

    /// A-weighted band level (weight at the exact centre frequency); `None` for zero energy.
    pub fn level_a(&self) -> Option<f64> {
        let a = a_weighting_db(self.centre).ok()?;
        self.level().map(|l| l + a)
    }
}

/// Preferred-number label of band `n`.
pub fn nominal_frequency(n: i32) -> f64 {
    const MANT: [f64; 10] = [1.0, 1.25, 1.6, 2.0, 2.5, 3.15, 4.0, 5.0, 6.3, 8.0];
    let m = n.rem_euclid(10) as usize;
    let dec = n.div_euclid(10);
    MANT[m] * 10f64.powi(3 + dec)
}

/// Band index of a frequency: band `n` holds `f` with `n - 1/2 <= 10 log10(f/1000) < n + 1/2`.
pub fn band_index(f: f64) -> Res<i32> {
    if !f.is_finite() || f <= 0.0 {
        return err(format!("f: frequency must be finite and positive (got {f})"));
    }
    Ok((10.0 * (f / 1000.0).log10() + 0.5).floor() as i32)
}

/// Synthesise 1/3-octave band energies from a spectrum: every bin (DC excluded) is assigned to the band containing its
/// centre frequency; the bands from the one containing `f_min` to the one containing `f_max` are returned (bins in other bands are dropped).
pub fn third_octave_bands(spec: &Spectrum, f_min: f64, f_max: f64) -> Res<Vec<Band>> {
    if !(f_min.is_finite() && f_max.is_finite() && f_min > 0.0 && f_max > f_min) {
        return err(format!("f_min/f_max: need 0 < f_min < f_max (got {f_min}, {f_max})"));
    }
    if spec.freq.len() != spec.ms.len() {
        return err("freq: length differs from ms".into());
    }
    if spec
        .freq
        .windows(2)
        .any(|w| w[1].partial_cmp(&w[0]) != Some(std::cmp::Ordering::Greater))
    {
        return err("freq: frequencies must be strictly increasing".into());
    }
    let centre = |n: i32| 1000.0 * 10f64.powf(0.1 * n as f64);
    let n_lo = band_index(f_min)?;
    let n_hi = band_index(f_max)?;
    let mut bands: Vec<Band> = (n_lo..=n_hi)
        .map(|n| Band {
            n,
            nominal: nominal_frequency(n),
            centre: centre(n),
            lo: centre(n) * 10f64.powf(-0.05),
            hi: centre(n) * 10f64.powf(0.05),
            ms: 0.0,
        })
        .collect();
    for (f, m) in spec.freq.iter().zip(&spec.ms).skip(1) {
        let n = band_index(*f)?;
        if n >= n_lo && n <= n_hi {
            bands[(n - n_lo) as usize].ms += m;
        }
    }
    Ok(bands)
}

// ---------------------------------------------------------------------------------------------------------------
// Observer sphere and sound power
// ---------------------------------------------------------------------------------------------------------------

fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        out.push((x, 2.0 / ((1.0 - x * x) * dp * dp)));
    }
    out
}

/// Quadrature on the unit sphere: Gauss-Legendre in the polar cosine times `n_phi` equispaced azimuths. Returns
/// `(unit direction, weight)` with weights summing to `4 pi`. Exact for spherical harmonics of degree up to
/// `min(2 n_theta - 1, n_phi - 1)`.
pub fn sphere_quadrature(n_theta: usize, n_phi: usize) -> Res<Vec<([f64; 3], f64)>> {
    if n_theta < 1 || n_phi < 1 {
        return err(format!("n_theta/n_phi: must be >= 1 (got {n_theta}, {n_phi})"));
    }
    let mut pts = Vec::with_capacity(n_theta * n_phi);
    for (mu, w) in gauss_legendre(n_theta) {
        let s = (1.0 - mu * mu).max(0.0).sqrt();
        for j in 0..n_phi {
            let phi = 2.0 * std::f64::consts::PI * (j as f64 + 0.5) / n_phi as f64;
            pts.push((
                [s * phi.cos(), s * phi.sin(), mu],
                w * 2.0 * std::f64::consts::PI / n_phi as f64,
            ));
        }
    }
    Ok(pts)
}

/// Radiated sound power through a sphere of radius `radius`: `W = sum_i w_i R^2 <p_i'^2> / (rho0 c0)` with `w_i` the
/// solid-angle weights of [`sphere_quadrature`] and `<p'^2>` the mean square of each observer series (mean removed;
/// use whole periods or long records). Far-field intensity `I = <p'^2>/(rho0 c0)` is assumed on the sphere.
pub fn sound_power_sphere(radius: f64, weights: &[f64], series: &[Vec<f64>], rho0: f64, c0: f64) -> Res<f64> {
    if !(radius.is_finite() && radius > 0.0) {
        return err(format!("radius: must be finite and positive (got {radius})"));
    }
    if !(rho0 > 0.0 && c0 > 0.0 && rho0.is_finite() && c0.is_finite()) {
        return err(format!("rho0/c0: must be finite and positive (got {rho0}, {c0})"));
    }
    if weights.len() != series.len() || weights.is_empty() {
        return err(format!(
            "series: expected one series per weight (weights {}, series {})",
            weights.len(),
            series.len()
        ));
    }
    let mut w = 0.0;
    for (i, (wt, s)) in weights.iter().zip(series).enumerate() {
        let ms = mean_square(s).map_err(|e| AcousticsError(format!("series[{i}]: {e}")))?;
        w += wt * ms;
    }
    Ok(w * radius * radius / (rho0 * c0))
}

// ---------------------------------------------------------------------------------------------------------------
// Report (narrowband spectrum + 1/3-octave bands + overall levels) and writers
// ---------------------------------------------------------------------------------------------------------------

/// Spectral report of one signal.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Window used.
    pub window: Window,
    /// Narrowband spectrum.
    pub spectrum: Spectrum,
    /// 1/3-octave bands.
    pub bands: Vec<Band>,
    /// Overall level of the series (time domain, mean removed), dB; `None` if the fluctuation is zero.
    pub oaspl: Option<f64>,
    /// A-weighted overall level from the spectrum, dB(A); `None` if zero.
    pub oaspl_a: Option<f64>,
}

/// Build the report of a pressure series sampled at `dt`; bands cover centres in `[f_min, f_max]`.
pub fn spectrum_report(signal: &[f64], dt: f64, window: Window, f_min: f64, f_max: f64) -> Res<Report> {
    let spectrum = power_spectrum(signal, dt, window)?;
    let bands = third_octave_bands(&spectrum, f_min, f_max)?;
    let oaspl = level_from_ms(mean_square(&signal[..spectrum.n_used])?)?;
    let oaspl_a = spectrum.oaspl_a()?;
    Ok(Report {
        window,
        spectrum,
        bands,
        oaspl,
        oaspl_a,
    })
}

fn opt(v: Option<f64>) -> Value {
    v.map_or(Value::Null, |x| json!(x))
}

fn csv_opt(v: Option<f64>) -> String {
    v.map_or(String::new(), |x| format!("{x:e}"))
}

impl Report {
    /// JSON document (`rcfd-spectrum` v1). Levels of empty bands are `null`.
    pub fn to_json(&self) -> Value {
        let nb: Vec<Value> = self
            .spectrum
            .freq
            .iter()
            .zip(&self.spectrum.ms)
            .map(|(f, m)| json!({"f": f, "ms": m, "level_db": opt(level_from_ms(*m).ok().flatten())}))
            .collect();
        let bands: Vec<Value> = self
            .bands
            .iter()
            .map(|b| {
                json!({"nominal": b.nominal, "centre": b.centre, "lo": b.lo, "hi": b.hi, "ms": b.ms,
                       "level_db": opt(b.level()), "level_dba": opt(b.level_a())})
            })
            .collect();
        json!({
            "format": "rcfd-spectrum", "version": 1,
            "window": match self.window { Window::Rectangular => "rectangular", Window::Hann => "hann" },
            "df": self.spectrum.df, "n_used": self.spectrum.n_used,
            "oaspl_db": opt(self.oaspl), "oaspl_dba": opt(self.oaspl_a),
            "narrowband": nb, "third_octave": bands,
        })
    }

    /// CSV with the 1/3-octave bands: `nominal_hz,centre_hz,lo_hz,hi_hz,ms_pa2,level_db,level_dba`.
    pub fn bands_csv(&self) -> String {
        let mut s = String::from("nominal_hz,centre_hz,lo_hz,hi_hz,ms_pa2,level_db,level_dba\n");
        for b in &self.bands {
            s.push_str(&format!(
                "{},{:e},{:e},{:e},{:e},{},{}\n",
                b.nominal,
                b.centre,
                b.lo,
                b.hi,
                b.ms,
                csv_opt(b.level()),
                csv_opt(b.level_a())
            ));
        }
        s
    }

    /// CSV of the narrowband spectrum: `f_hz,ms_pa2,level_db`.
    pub fn narrowband_csv(&self) -> String {
        let mut s = String::from("f_hz,ms_pa2,level_db\n");
        for (f, m) in self.spectrum.freq.iter().zip(&self.spectrum.ms) {
            s.push_str(&format!(
                "{:e},{:e},{}\n",
                f,
                m,
                csv_opt(level_from_ms(*m).ok().flatten())
            ));
        }
        s
    }
}
