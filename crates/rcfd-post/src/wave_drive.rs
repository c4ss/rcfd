//! A driven boundary of the linear acoustic wave solver built from a sampled surface (F-452, ADR 0372).
//!
//! An `rcfd-surface-series` file (ADR 0066, [`crate::surface_series`]) holds the pressure on a duct section, sampled over a
//! whole number of periods of a fundamental frequency. [`SurfaceDrive::from_series`] takes the harmonics `1..=H` of every
//! face by a DFT on the file's clock (the mean is not acoustic and is dropped), fits each harmonic's section field on a
//! basis of hard-walled duct modes ([`crate::duct_modes`]) and keeps the modal amplitudes. [`SurfaceDrive::ghost_state`]
//! plays the field back periodically as the ghost state of [`rcfd_fv::acoustic_wave::WaveBc::Driven`]: the wave entering
//! the domain along the duct axis, each mode with its own axial wavenumber and impedance `u = kz p / (rho0 omega)`, carried
//! from the sampling plane to the face by `e^{-i kz (xi - xi_s)}`, under a start-up ramp.
//!
//! The sampled pressure is taken to be the entering wave only; one section of pressure cannot separate a returning wave.
//! A series that does not cover a whole number of periods is refused (the DFT would leak the mean and the harmonics).
//!
//! Returning waves (F-478, ADR 0378): [`SurfaceDrive::from_sections`] takes two or more sampled sections of one duct, splits
//! every mode of every harmonic into the wave entering the domain and the one returning from it (`split_axial` with the
//! closed-form `kz`) and plays back the entering wave only; [`SurfaceDrive::entering_residual`] measures, against a second
//! sampled section, how far a single-section drive's entering-wave assumption is from the sampled field.

use crate::duct_modes::{axial_wavenumber, decompose_section, split_axial, DuctShape, HarmonicDft, ModeBasis, C64};
use crate::surface_series::{Motion, SeriesSpec};
use rcfd_core::{Error, Result};
use rcfd_fv::acoustic_wave::{GhostState, Medium};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

type V3 = [f64; 3];
/// Cached axial offset and mode shapes of a face centre, keyed by the bits of its position.
type ShapeCache = Mutex<HashMap<[u64; 3], Arc<(f64, Vec<C64>)>>>;

fn invalid<T>(path: &str, reason: impl Into<String>) -> Result<T> {
    Err(Error::InvalidValue {
        path: path.into(),
        reason: reason.into(),
    })
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The placement of the duct: `origin` is the zero of the section coordinates (rectangular: the corner `(0, 0)`;
/// circular: the axis), `axis` the unit direction into the wave domain, `e1`, `e2` the unit section axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuctFrame {
    /// Zero of the section coordinates (a point of the axial origin).
    pub origin: V3,
    /// Unit axis, pointing into the wave domain (the direction of the driven wave).
    pub axis: V3,
    /// First unit section axis.
    pub e1: V3,
    /// Second unit section axis.
    pub e2: V3,
}

impl DuctFrame {
    /// Section coordinates of a point.
    pub fn section(&self, x: V3) -> [f64; 2] {
        let d = sub(x, self.origin);
        [dot(d, self.e1), dot(d, self.e2)]
    }

    /// Axial coordinate of a point.
    pub fn axial(&self, x: V3) -> f64 {
        dot(sub(x, self.origin), self.axis)
    }

    fn check(&self) -> Result<()> {
        let v = [self.axis, self.e1, self.e2];
        if v.iter().chain([&self.origin]).any(|a| a.iter().any(|c| !c.is_finite())) {
            return invalid("wave_drive.frame", "components must be finite");
        }
        for (i, a) in v.iter().enumerate() {
            if (dot(*a, *a).sqrt() - 1.0).abs() > 1e-9 {
                return invalid("wave_drive.frame", format!("axis {i} is not a unit vector"));
            }
            for b in &v[i + 1..] {
                if dot(*a, *b).abs() > 1e-9 {
                    return invalid("wave_drive.frame", "axis, e1 and e2 must be orthogonal");
                }
            }
        }
        Ok(())
    }
}

/// Options of the drive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriveOptions {
    /// Fundamental frequency [Hz]; the series must cover a whole number of its periods.
    pub frequency: f64,
    /// Highest harmonic played back (`1..=max_harmonic`), below the Nyquist bin of the series.
    pub max_harmonic: usize,
    /// Ramp time [s]: the drive is `sin^2(pi t / (2 ramp_time))` times the field for `0 <= t < ramp_time` (0: no ramp).
    pub ramp_time: f64,
}

/// One harmonic: its angular frequency, the modal amplitudes at the sampling plane and the axial wavenumbers.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonicModes {
    /// Harmonic number.
    pub harmonic: usize,
    /// Angular frequency [rad/s].
    pub omega: f64,
    /// Amplitude of each basis mode at the sampling plane (`p = Re(a psi e^{i omega t})`).
    pub amplitudes: Vec<C64>,
    /// Axial wavenumber of each basis mode (closed form).
    pub kz: Vec<C64>,
    /// Weighted RMS residual of the section fit relative to the RMS of the face amplitudes (0 for a zero harmonic).
    pub residual: f64,
}

/// A periodic driven boundary built from a sampled section (module documentation).
#[derive(Debug, Clone)]
pub struct SurfaceDrive {
    frame: DuctFrame,
    basis: ModeBasis,
    medium: Medium,
    ramp_time: f64,
    /// The options it was built with (for [`SurfaceDrive::entering_residual`]).
    opts: DriveOptions,
    /// Axial coordinate of the sampling plane (area-weighted mean of the faces).
    pub section_xi: f64,
    /// Number of periods of the fundamental covered by the series.
    pub periods: usize,
    /// Area- and time-mean of the sampled pressure (not played back).
    pub mean: f64,
    /// Fraction of the variance about each face's mean not represented by the harmonics kept.
    pub unrepresented: f64,
    /// The harmonics.
    pub harmonics: Vec<HarmonicModes>,
}

impl SurfaceDrive {
    /// Build the drive from a parsed series (ADR 0372 D3 to D5). `medium` is the wave solver's; the file must agree.
    pub fn from_series(
        spec: &SeriesSpec,
        medium: Medium,
        frame: DuctFrame,
        basis: ModeBasis,
        opts: DriveOptions,
    ) -> Result<Self> {
        let series_err = |e: crate::surface_series::SeriesError| Error::InvalidValue {
            path: "surface_series".into(),
            reason: e.0,
        };
        spec.validate().map_err(series_err)?;
        frame.check()?;
        if !(opts.frequency.is_finite() && opts.frequency > 0.0) {
            return invalid(
                "wave_drive.frequency",
                format!("must be finite and > 0, got {}", opts.frequency),
            );
        }
        if opts.max_harmonic == 0 {
            return invalid("wave_drive.max_harmonic", "at least one harmonic is required");
        }
        if !(opts.ramp_time.is_finite() && opts.ramp_time >= 0.0) {
            return invalid(
                "wave_drive.ramp_time",
                format!("must be finite and >= 0, got {}", opts.ramp_time),
            );
        }
        for (path, file, own) in [("$.rho0", spec.rho0, medium.rho0), ("$.c0", spec.c0, medium.c)] {
            if (file / own - 1.0).abs() > 1e-9 {
                return invalid(path, format!("{file} differs from the wave solver's {own}"));
            }
        }
        if spec.flow != [0.0; 3] {
            return invalid(
                "$.flow",
                "the wave solver's medium is at rest; a uniform flow is not supported",
            );
        }
        if let Motion::Rotation { .. } = spec.motion {
            return invalid(
                "$.motion",
                "a moving surface cannot drive a duct section; only static is supported",
            );
        }
        // Whole periods (D3).
        let n = spec.samples();
        let r = n as f64 * spec.dt * opts.frequency;
        let periods = r.round();
        if periods < 1.0 || (r - periods).abs() > 1e-6 * r.max(1.0) {
            return invalid(
                "$.faces[0].p",
                format!(
                    "{n} samples of dt = {} s cover {r} periods of {} Hz, not a whole number of periods",
                    spec.dt, opts.frequency
                ),
            );
        }
        let periods = periods as usize;
        if 2 * opts.max_harmonic * periods >= n {
            return invalid(
                "wave_drive.max_harmonic",
                format!(
                    "harmonic {} lies at or above the Nyquist bin of {n} samples over {periods} periods",
                    opts.max_harmonic
                ),
            );
        }
        // Geometry (D5).
        let size = match basis.shape {
            DuctShape::Rectangular { a, b } => a.max(b),
            DuctShape::Circular { radius } => radius,
        };
        let tol = 1e-6 * size;
        let area: f64 = spec.faces.iter().map(|f| f.area).sum();
        let xi_s = spec.faces.iter().map(|f| f.area * frame.axial(f.x)).sum::<f64>() / area;
        let mut points = Vec::with_capacity(spec.faces.len());
        for (k, f) in spec.faces.iter().enumerate() {
            if dot(f.n, frame.axis).abs() < 1.0 - 1e-6 {
                return invalid(&format!("$.faces[{k}].n"), "the face normal is not along the duct axis");
            }
            let xi = frame.axial(f.x);
            if (xi - xi_s).abs() > tol {
                // Name the face farthest from the plane (ADR 0378 D6), found once a face off it is known.
                let (far, off) = spec
                    .faces
                    .iter()
                    .map(|g| frame.axial(g.x) - xi_s)
                    .enumerate()
                    .fold((k, xi - xi_s), |b, (j, o)| if o.abs() > b.1.abs() { (j, o) } else { b });
                return invalid(
                    &format!("$.faces[{far}].x"),
                    format!("the face lies {off} m off the section plane"),
                );
            }
            let s = frame.section(f.x);
            let inside = match basis.shape {
                DuctShape::Rectangular { a, b } => s[0] >= -tol && s[0] <= a + tol && s[1] >= -tol && s[1] <= b + tol,
                DuctShape::Circular { radius } => s[0].hypot(s[1]) <= radius + tol,
            };
            if !inside {
                return invalid(
                    &format!("$.faces[{k}].x"),
                    format!("section point ({}, {}) lies outside the duct", s[0], s[1]),
                );
            }
            points.push(s);
        }
        if basis.len() > spec.faces.len() {
            return invalid(
                "wave_drive.basis",
                format!(
                    "{} modes need at least as many faces, got {}",
                    basis.len(),
                    spec.faces.len()
                ),
            );
        }
        let weights: Vec<f64> = spec.faces.iter().map(|f| f.area).collect();
        // Mean and variance.
        let mut mean = 0.0;
        let mut variance = 0.0;
        for f in &spec.faces {
            let m = f.p.iter().sum::<f64>() / n as f64;
            mean += f.area * m;
            variance += f.area * f.p.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n as f64;
        }
        mean /= area;
        // Harmonics.
        let omega0 = 2.0 * PI * opts.frequency;
        let mut harmonics = Vec::with_capacity(opts.max_harmonic);
        let mut kept = 0.0;
        for h in 1..=opts.max_harmonic {
            let omega = omega0 * h as f64;
            let mut dft = HarmonicDft::new(omega, spec.faces.len())?;
            let mut row = vec![0.0; spec.faces.len()];
            for i in 0..n {
                for (v, f) in row.iter_mut().zip(&spec.faces) {
                    *v = f.p[i];
                }
                dft.add(spec.t0 + i as f64 * spec.dt, &row)?;
            }
            let face_amps = dft.amplitudes()?;
            let energy: f64 = face_amps.iter().zip(&weights).map(|(a, w)| w * a.norm_sqr()).sum();
            kept += 0.5 * energy;
            let fit = decompose_section(&basis, &points, &weights, &face_amps)?;
            let rms = (energy / area).sqrt();
            let k = omega / medium.c;
            harmonics.push(HarmonicModes {
                harmonic: h,
                omega,
                amplitudes: fit.amplitudes,
                kz: basis.k_cut.iter().map(|&kc| axial_wavenumber(k, kc)).collect(),
                residual: if rms > 0.0 { fit.residual / rms } else { 0.0 },
            });
        }
        let unrepresented = if variance > 0.0 {
            (1.0 - kept / variance).max(0.0)
        } else {
            0.0
        };
        Ok(Self {
            frame,
            basis,
            medium,
            ramp_time: opts.ramp_time,
            opts,
            section_xi: xi_s,
            periods,
            mean,
            unrepresented,
            harmonics,
        })
    }

    /// The basis.
    pub fn basis(&self) -> &ModeBasis {
        &self.basis
    }

    fn shapes(&self, x: V3) -> (f64, Vec<C64>) {
        let s = self.frame.section(x);
        (
            self.frame.axial(x) - self.section_xi,
            (0..self.basis.len()).map(|q| self.basis.eval(q, s)).collect(),
        )
    }

    fn synth(&self, d: f64, psi: &[C64], t: f64) -> (f64, V3) {
        let (mut p, mut u) = (0.0, 0.0);
        for hm in &self.harmonics {
            let rw = 1.0 / (self.medium.rho0 * hm.omega);
            for (q, ps) in psi.iter().enumerate() {
                let kz = hm.kz[q];
                let e = ((C64::new(0.0, -1.0) * kz).scale(d) + C64::new(0.0, hm.omega * t)).exp();
                let a = hm.amplitudes[q] * *ps * e;
                p += a.re;
                u += (kz * a).re * rw;
            }
        }
        let ax = self.frame.axis;
        (p, [u * ax[0], u * ax[1], u * ax[2]])
    }

    fn ramp(&self, t: f64) -> f64 {
        if t < 0.0 {
            0.0
        } else if t < self.ramp_time {
            (PI * t / (2.0 * self.ramp_time)).sin().powi(2)
        } else {
            1.0
        }
    }

    /// The entering wave `(p, u)` at the point `x` and time `t`, without the ramp.
    pub fn state(&self, x: V3, t: f64) -> (f64, V3) {
        let (d, psi) = self.shapes(x);
        self.synth(d, &psi, t)
    }

    /// The ghost state for [`rcfd_fv::acoustic_wave::WaveBc::Driven`] (with the ramp); the mode shapes are cached per
    /// face centre.
    pub fn ghost_state(&self) -> GhostState {
        let me = self.clone();
        let cache: ShapeCache = Mutex::new(HashMap::new());
        Arc::new(move |x: V3, t: f64| {
            let key = x.map(f64::to_bits);
            let entry = {
                let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
                c.entry(key).or_insert_with(|| Arc::new(me.shapes(x))).clone()
            };
            let w = me.ramp(t);
            let (p, u) = me.synth(entry.0, &entry.1, t);
            (w * p, [w * u[0], w * u[1], w * u[2]])
        })
    }
}

/// The smallest separation `sigma` of the two-wave split accepted by [`SurfaceDrive::from_sections`] (ADR 0378 D3).
pub const MIN_SEPARATION: f64 = 0.05;

/// A drive built from two or more sampled sections with the returning wave split off (ADR 0378 D2, D5).
#[derive(Debug, Clone)]
pub struct SplitDrive {
    /// The drive of the entering wave only, referenced at the plane of the first section.
    pub drive: SurfaceDrive,
    /// Axial coordinate of each section's plane.
    pub sections_xi: Vec<f64>,
    /// Per harmonic and mode: amplitude of the returning wave at the first section's plane.
    pub returning: Vec<Vec<C64>>,
    /// Per harmonic and mode: separation `sigma` of the split (1: well separated, 0: singular).
    pub separation: Vec<Vec<f64>>,
    /// Smallest separation over the harmonics and modes.
    pub min_separation: f64,
    /// Per harmonic: `sqrt(sum |returning|^2 / sum |entering|^2)` at the first section's plane (0 for no entering wave).
    pub reflection: Vec<f64>,
    /// Per harmonic: RMS residual of the two-wave fits relative to the RMS of the sampled modal amplitudes (zero up to
    /// round-off for two sections).
    pub split_residual: Vec<f64>,
}

fn locate(prefix: &str, e: Error) -> Error {
    match e {
        Error::InvalidValue { path, reason } => Error::InvalidValue {
            path: format!("{prefix}/{path}"),
            reason,
        },
        other => other,
    }
}

impl SurfaceDrive {
    /// Build the drive of the entering wave from two or more sampled sections of one duct (ADR 0378 D2, D3). Each
    /// section is read by [`SurfaceDrive::from_series`] (its refusals located as `sections[i]/...`); per harmonic and mode
    /// the waves towards and against the axis are split over the sections' planes with the closed-form `kz`, referenced
    /// at the first section's plane; the split is refused when its separation is below [`MIN_SEPARATION`].
    pub fn from_sections(
        specs: &[SeriesSpec],
        medium: Medium,
        frame: DuctFrame,
        basis: ModeBasis,
        opts: DriveOptions,
    ) -> Result<SplitDrive> {
        if specs.len() < 2 {
            return invalid(
                "sections",
                format!("the split needs at least two sections, got {}", specs.len()),
            );
        }
        let mut drives = Vec::with_capacity(specs.len());
        for (i, spec) in specs.iter().enumerate() {
            let d = Self::from_series(spec, medium, frame, basis.clone(), opts)
                .map_err(|e| locate(&format!("sections[{i}]"), e))?;
            drives.push(d);
        }
        let xis: Vec<f64> = drives.iter().map(|d| d.section_xi).collect();
        let x0 = xis[0];
        let nm = basis.len();
        let mut drive = drives[0].clone();
        let mut returning = Vec::new();
        let mut separation = Vec::new();
        let mut reflection = Vec::new();
        let mut split_residual = Vec::new();
        let mut min_separation = f64::INFINITY;
        for (hi, hm) in drive.harmonics.iter_mut().enumerate() {
            let mut plus = Vec::with_capacity(nm);
            let mut minus = Vec::with_capacity(nm);
            let mut seps = Vec::with_capacity(nm);
            let (mut num, mut den) = (0.0, 0.0);
            for q in 0..nm {
                let kz = hm.kz[q];
                let (mut guu, mut gvv, mut c) = (0.0, 0.0, C64::default());
                for &xi in &xis {
                    let u = (C64::new(0.0, -1.0) * kz).scale(xi - x0).exp();
                    let v = (C64::new(0.0, 1.0) * kz).scale(xi - x0).exp();
                    guu += u.norm_sqr();
                    gvv += v.norm_sqr();
                    c += u.conj() * v;
                }
                let g = c.abs() / (guu * gvv).sqrt();
                let sigma = (1.0 - g * g).max(0.0).sqrt();
                let md = basis.modes[q];
                if sigma.is_nan() || sigma < MIN_SEPARATION {
                    return invalid(
                        "sections",
                        format!(
                            "harmonic {} mode ({}, {}): the sections at {xis:?} separate the two waves by {sigma:.3e}, \
                             below {MIN_SEPARATION}",
                            hm.harmonic, md.m, md.n
                        ),
                    );
                }
                let amps: Vec<C64> = drives.iter().map(|d| d.harmonics[hi].amplitudes[q]).collect();
                let fit = split_axial(&xis, &amps, kz, x0)?;
                if ![fit.plus.re, fit.plus.im, fit.minus.re, fit.minus.im]
                    .iter()
                    .all(|v| v.is_finite())
                {
                    return invalid(
                        "sections",
                        format!(
                            "harmonic {} mode ({}, {}): the split is not finite (the sections are too far apart for a \
                             cut-off mode)",
                            hm.harmonic, md.m, md.n
                        ),
                    );
                }
                num += fit.residual * fit.residual * xis.len() as f64;
                den += amps.iter().map(|a| a.norm_sqr()).sum::<f64>();
                min_separation = min_separation.min(sigma);
                plus.push(fit.plus);
                minus.push(fit.minus);
                seps.push(sigma);
            }
            let ep: f64 = plus.iter().map(|a| a.norm_sqr()).sum();
            let em: f64 = minus.iter().map(|a| a.norm_sqr()).sum();
            reflection.push(if ep > 0.0 { (em / ep).sqrt() } else { 0.0 });
            split_residual.push(if den > 0.0 { (num / den).sqrt() } else { 0.0 });
            hm.amplitudes = plus;
            returning.push(minus);
            separation.push(seps);
        }
        Ok(SplitDrive {
            drive,
            sections_xi: xis,
            returning,
            separation,
            min_separation,
            reflection,
            split_residual,
        })
    }

    /// The residual of the entering-wave assumption of this drive against a second sampled section `check` (ADR 0378
    /// D4): per harmonic, `sqrt(sum_q |a^c_q - a_q e^{-i kz_q (xi_c - xi_s)}|^2 / sum_q |a^c_q|^2)` (0 when the check
    /// section has no content at that harmonic). Zero up to round-off when the sampled field is the entering wave only.
    pub fn entering_residual(&self, check: &SeriesSpec) -> Result<Vec<f64>> {
        let c = Self::from_series(check, self.medium, self.frame, self.basis.clone(), self.opts)
            .map_err(|e| locate("check", e))?;
        let d = c.section_xi - self.section_xi;
        Ok(self
            .harmonics
            .iter()
            .zip(&c.harmonics)
            .map(|(hm, hc)| {
                let (mut num, mut den) = (0.0, 0.0);
                for (q, (a, ac)) in hm.amplitudes.iter().zip(&hc.amplitudes).enumerate() {
                    let e = (C64::new(0.0, -1.0) * hm.kz[q]).scale(d).exp();
                    num += (*ac - *a * e).norm_sqr();
                    den += ac.norm_sqr();
                }
                if den > 0.0 {
                    (num / den).sqrt()
                } else {
                    0.0
                }
            })
            .collect())
    }
}
