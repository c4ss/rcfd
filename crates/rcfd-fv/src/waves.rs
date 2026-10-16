//! Free-surface wave generation and absorption closures (F-227, ADR 0040): linear (Airy) and second-order Stokes kinematics for
//! wave-inlet values, irregular waves from Pierson-Moskowitz and JONSWAP spectra, relaxation-zone / numerical-beach weights, and
//! a 1-D linear shallow-water tank for reflection estimates.
//!
//! Library only: the free-surface solver (F-055) does not exist yet. Coordinates: `x` along propagation, `z` vertical with
//! `z = 0` the still-water level and the bed at `z = -h` (`h = f64::INFINITY` is deep water); `theta = k x - omega t + phi`.

use std::f64::consts::PI;

/// Result alias: errors are text naming the offending parameter.
pub type Result<T> = std::result::Result<T, String>;

fn pos(name: &str, v: f64) -> Result<()> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and > 0, got {v}"))
    }
}

/// Solve the finite-depth dispersion relation `omega^2 = g k tanh(k h)` for the wavenumber `k` (`h = INFINITY`: `omega^2/g`).
pub fn wavenumber(omega: f64, h: f64, g: f64) -> Result<f64> {
    pos("omega", omega)?;
    pos("g", g)?;
    if h.is_nan() || h <= 0.0 {
        return Err(format!("h must be > 0 (INFINITY for deep water), got {h}"));
    }
    let k_deep = omega * omega / g;
    if h.is_infinite() {
        return Ok(k_deep);
    }
    let mut k = k_deep / (omega * omega * h / g).tanh().sqrt();
    for _ in 0..100 {
        let th = (k * h).tanh();
        let f = g * k * th - omega * omega;
        if f.abs() <= 1e-14 * omega * omega {
            return Ok(k);
        }
        let sech2 = 1.0 - th * th;
        let df = g * th + g * k * h * sech2;
        let dk = f / df;
        k -= dk;
        if !(k.is_finite() && k > 0.0) {
            return Err(format!(
                "dispersion iteration left the domain for omega = {omega}, h = {h}"
            ));
        }
        if dk.abs() <= 1e-16 * k {
            return Ok(k);
        }
    }
    Err(format!(
        "dispersion iteration did not converge for omega = {omega}, h = {h}"
    ))
}

/// Phase speed `omega/k`.
pub fn phase_speed(omega: f64, k: f64) -> f64 {
    omega / k
}

/// Group velocity `(c/2)(1 + 2 k h/sinh(2 k h))`.
pub fn group_velocity(omega: f64, k: f64, h: f64) -> f64 {
    let c = omega / k;
    if h.is_infinite() {
        return 0.5 * c;
    }
    let x = 2.0 * k * h;
    0.5 * c * (1.0 + if x > 700.0 { 0.0 } else { x / x.sinh() })
}

/// Smooth start-up ramp, 0 for `t <= 0`, exactly 1 for `t >= t_ramp`.
pub fn ramp(t: f64, t_ramp: f64) -> f64 {
    if t <= 0.0 {
        0.0
    } else if t >= t_ramp {
        1.0
    } else {
        0.5 * (1.0 - (PI * t / t_ramp).cos())
    }
}

/// Exact volume fraction of liquid in a cell `[z_c - dz/2, z_c + dz/2]` below the free surface at `eta`.
pub fn liquid_fraction(z_c: f64, dz: f64, eta: f64) -> f64 {
    ((eta - (z_c - 0.5 * dz)) / dz).clamp(0.0, 1.0)
}

/// A progressive wave of first (Airy) or second (Stokes) order.
#[derive(Debug, Clone, Copy)]
pub struct Wave {
    /// First-harmonic amplitude, m.
    pub a: f64,
    /// Angular frequency, rad/s.
    pub omega: f64,
    /// Wavenumber, rad/m (from the dispersion relation).
    pub k: f64,
    /// Still-water depth, m (INFINITY allowed).
    pub h: f64,
    /// Gravity, m/s^2.
    pub g: f64,
    /// Phase offset, rad.
    pub phase: f64,
    /// 1 = Airy, 2 = second-order Stokes.
    pub order: u8,
}

/// Depth-dependent factors, overflow-safe (`ez = e^{kz}`, `q = e^{-2kh}`).
struct Depth {
    c1: f64,
    s1: f64,
    cp: f64,
    sp: f64,
    c2: f64,
    s2: f64,
}

impl Wave {
    /// Build a wave from amplitude, angular frequency and depth.
    pub fn new(a: f64, omega: f64, h: f64, g: f64, phase: f64, order: u8) -> Result<Self> {
        if !a.is_finite() || a < 0.0 {
            return Err(format!("a must be finite and >= 0, got {a}"));
        }
        if !(order == 1 || order == 2) {
            return Err(format!("order must be 1 or 2, got {order}"));
        }
        if !phase.is_finite() {
            return Err(format!("phase must be finite, got {phase}"));
        }
        let k = wavenumber(omega, h, g)?;
        Ok(Self {
            a,
            omega,
            k,
            h,
            g,
            phase,
            order,
        })
    }

    /// Phase speed.
    pub fn celerity(&self) -> f64 {
        self.omega / self.k
    }
    /// Wavelength.
    pub fn wavelength(&self) -> f64 {
        2.0 * PI / self.k
    }
    /// Period.
    pub fn period(&self) -> f64 {
        2.0 * PI / self.omega
    }
    /// Group velocity.
    pub fn group_velocity(&self) -> f64 {
        group_velocity(self.omega, self.k, self.h)
    }
    /// Energy density per unit area, `rho g a^2/2`.
    pub fn energy_density(&self, rho: f64) -> f64 {
        0.5 * rho * self.g * self.a * self.a
    }
    /// Energy flux per unit crest width, `E c_g`.
    pub fn energy_flux(&self, rho: f64) -> f64 {
        self.energy_density(rho) * self.group_velocity()
    }
    /// Time-mean volume flux of the wave (Eulerian, per unit width), `E/(rho c)` = `g a^2/(2c)` (from the linear theory).
    pub fn mean_volume_flux(&self) -> f64 {
        0.5 * self.g * self.a * self.a / self.celerity()
    }

    fn q(&self) -> f64 {
        if self.h.is_infinite() {
            0.0
        } else {
            (-2.0 * self.k * self.h).exp()
        }
    }

    /// Amplitude of the second harmonic of the elevation (0 for order 1).
    pub fn eta2(&self) -> f64 {
        if self.order < 2 {
            return 0.0;
        }
        let q = self.q();
        let ratio = 4.0 * (1.0 + q) * (2.0 * q + 0.5 * (1.0 + q * q)) / (1.0 - q).powi(3);
        0.25 * self.k * self.a * self.a * ratio
    }

    fn depth(&self, z: f64) -> Depth {
        let q = self.q();
        let ez = (self.k * z).exp();
        let inv = q / ez;
        let d1 = 1.0 - q;
        let d4 = d1.powi(4);
        let e2 = ez * ez;
        Depth {
            c1: (ez + inv) / d1,
            s1: (ez - inv) / d1,
            cp: (ez + inv) / (1.0 + q),
            sp: (ez - inv) / (1.0 + q),
            c2: 8.0 * (e2 * q + q * q * q / e2) / d4,
            s2: 8.0 * (e2 * q - q * q * q / e2) / d4,
        }
    }

    fn theta(&self, x: f64, t: f64) -> f64 {
        self.k * x - self.omega * t + self.phase
    }

    fn two(&self) -> f64 {
        if self.order >= 2 {
            1.0
        } else {
            0.0
        }
    }

    /// Surface elevation.
    pub fn eta(&self, x: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        self.a * th.cos() + self.eta2() * (2.0 * th).cos()
    }
    /// `d eta/dt`.
    pub fn eta_t(&self, x: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        self.a * self.omega * th.sin() + 2.0 * self.omega * self.eta2() * (2.0 * th).sin()
    }
    /// `d eta/dx`.
    pub fn eta_x(&self, x: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        -self.k * self.a * th.sin() - 2.0 * self.k * self.eta2() * (2.0 * th).sin()
    }
    /// Horizontal and vertical velocity `(u, w)`.
    pub fn velocity(&self, x: f64, z: f64, t: f64) -> (f64, f64) {
        let th = self.theta(x, t);
        let d = self.depth(z);
        let b = 0.75 * self.a * self.a * self.omega * self.k * self.two();
        (
            self.a * self.omega * d.c1 * th.cos() + b * d.c2 * (2.0 * th).cos(),
            self.a * self.omega * d.s1 * th.sin() + b * d.s2 * (2.0 * th).sin(),
        )
    }
    /// `d w/d z` (equal to `- d u/d x`).
    pub fn dw_dz(&self, x: f64, z: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        let d = self.depth(z);
        let b = 0.75 * self.a * self.a * self.omega * self.k * self.two();
        self.a * self.omega * self.k * d.c1 * th.sin() + 2.0 * self.k * b * d.c2 * (2.0 * th).sin()
    }
    /// Velocity potential.
    pub fn potential(&self, x: f64, z: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        let d = self.depth(z);
        self.a * self.g / self.omega * d.cp * th.sin()
            + 0.375 * self.a * self.a * self.omega * d.c2 * (2.0 * th).sin() * self.two()
    }
    /// `d phi/d t`.
    pub fn potential_t(&self, x: f64, z: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        let d = self.depth(z);
        -self.a * self.g * d.cp * th.cos()
            - 0.75 * self.a * self.a * self.omega * self.omega * d.c2 * (2.0 * th).cos() * self.two()
    }
    /// `d^2 phi/(d t d z)`.
    pub fn potential_tz(&self, x: f64, z: f64, t: f64) -> f64 {
        let th = self.theta(x, t);
        let d = self.depth(z);
        -self.a * self.g * self.k * d.sp * th.cos()
            - 1.5 * self.a * self.a * self.omega * self.omega * self.k * d.s2 * (2.0 * th).cos() * self.two()
    }
    /// Dynamic pressure of the first-order wave, `rho g a cosh(k(z+h))/cosh(kh) cos(theta)`.
    pub fn dynamic_pressure(&self, rho: f64, x: f64, z: f64, t: f64) -> f64 {
        rho * self.g * self.a * self.depth(z).cp * self.theta(x, t).cos()
    }
    /// Orbital-velocity amplitude ratio of the first harmonic, `u_amp(z)/u_amp(0)`.
    pub fn orbital_ratio(&self, z: f64) -> f64 {
        self.depth(z).c1 / self.depth(0.0).c1
    }

    /// Wave-inlet boundary values `(eta, u, w)` at the inlet position `x_b`, height `z` and time `t`, scaled by the start-up ramp.
    pub fn inlet_state(&self, x_b: f64, z: f64, t: f64, t_ramp: f64) -> (f64, f64, f64) {
        let r = ramp(t, t_ramp);
        let (u, w) = self.velocity(x_b, z, t);
        (r * self.eta(x_b, t), r * u, r * w)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Irregular waves
// ---------------------------------------------------------------------------------------------------------------------

/// Wave spectrum in angular frequency, m^2 s/rad.
#[derive(Debug, Clone, Copy)]
pub enum Spectrum {
    /// Bretschneider / Pierson-Moskowitz in significant-height and peak-frequency form.
    PiersonMoskowitz {
        /// Significant wave height, m.
        hs: f64,
        /// Peak angular frequency, rad/s.
        wp: f64,
    },
    /// JONSWAP, scaled so that `m0 = hs^2/16` (`norm` is set by [`Spectrum::jonswap`]).
    Jonswap {
        /// Significant wave height, m.
        hs: f64,
        /// Peak angular frequency, rad/s.
        wp: f64,
        /// Peak enhancement factor.
        gamma: f64,
        /// Normalisation constant of the unscaled shape.
        norm: f64,
    },
}

fn jonswap_shape(w: f64, wp: f64, gamma: f64) -> f64 {
    let s = if w <= wp { 0.07 } else { 0.09 };
    let r = (-(w - wp) * (w - wp) / (2.0 * s * s * wp * wp)).exp();
    w.powi(-5) * (-1.25 * (wp / w).powi(4)).exp() * gamma.powf(r)
}

impl Spectrum {
    /// Pierson-Moskowitz spectrum.
    pub fn pierson_moskowitz(hs: f64, wp: f64) -> Result<Self> {
        pos("hs", hs)?;
        pos("wp", wp)?;
        Ok(Self::PiersonMoskowitz { hs, wp })
    }

    /// JONSWAP spectrum normalised to `m0 = hs^2/16` by quadrature over `[0.2 wp, 60 wp]`.
    pub fn jonswap(hs: f64, wp: f64, gamma: f64) -> Result<Self> {
        pos("hs", hs)?;
        pos("wp", wp)?;
        if !(gamma.is_finite() && gamma >= 1.0) {
            return Err(format!("gamma must be finite and >= 1, got {gamma}"));
        }
        let n = 200_000;
        let (lo, hi) = (0.2 * wp, 60.0 * wp);
        let dw = (hi - lo) / n as f64;
        let integral: f64 = (0..n)
            .map(|i| jonswap_shape(lo + (i as f64 + 0.5) * dw, wp, gamma))
            .sum::<f64>()
            * dw;
        Ok(Self::Jonswap {
            hs,
            wp,
            gamma,
            norm: hs * hs / 16.0 / integral,
        })
    }

    /// Peak angular frequency parameter.
    pub fn peak_frequency(&self) -> f64 {
        match *self {
            Self::PiersonMoskowitz { wp, .. } | Self::Jonswap { wp, .. } => wp,
        }
    }

    /// Target significant wave height.
    pub fn target_hs(&self) -> f64 {
        match *self {
            Self::PiersonMoskowitz { hs, .. } | Self::Jonswap { hs, .. } => hs,
        }
    }

    /// Spectral density at `w > 0`.
    pub fn density(&self, w: f64) -> f64 {
        match *self {
            Self::PiersonMoskowitz { hs, wp } => {
                5.0 / 16.0 * hs * hs * wp.powi(4) * w.powi(-5) * (-1.25 * (wp / w).powi(4)).exp()
            }
            Self::Jonswap { wp, gamma, norm, .. } => norm * jonswap_shape(w, wp, gamma),
        }
    }
}

/// Spectral moment `m_n = int w^n S dw` by midpoint quadrature.
pub fn spectral_moment(spec: &Spectrum, n: i32, w_lo: f64, w_hi: f64, bins: usize) -> f64 {
    let dw = (w_hi - w_lo) / bins as f64;
    (0..bins)
        .map(|i| {
            let w = w_lo + (i as f64 + 0.5) * dw;
            w.powi(n) * spec.density(w)
        })
        .sum::<f64>()
        * dw
}

/// `splitmix64` stream (reproducible, dependency free).
#[derive(Debug, Clone)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// Seeded generator.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// One spectral component.
#[derive(Debug, Clone, Copy)]
pub struct Component {
    /// Amplitude, m.
    pub a: f64,
    /// Angular frequency, rad/s.
    pub omega: f64,
    /// Wavenumber, rad/m.
    pub k: f64,
    /// Phase, rad, in `[0, 2 pi)`.
    pub phase: f64,
}

/// Irregular wave field: deterministic amplitudes from the spectrum, seeded random phases.
#[derive(Debug, Clone)]
pub struct IrregularWave {
    /// Components.
    pub components: Vec<Component>,
    /// Depth, m.
    pub h: f64,
    /// Gravity, m/s^2.
    pub g: f64,
    /// Frequency spacing, rad/s.
    pub d_omega: f64,
}

impl IrregularWave {
    /// `n` equal midpoint bins on `[w_lo, w_hi]`.
    pub fn new(spec: &Spectrum, h: f64, g: f64, w_lo: f64, w_hi: f64, n: usize, seed: u64) -> Result<Self> {
        pos("w_lo", w_lo)?;
        if !(w_hi.is_finite() && w_hi > w_lo) {
            return Err(format!("w_hi ({w_hi}) must exceed w_lo ({w_lo})"));
        }
        if n == 0 {
            return Err("n must be >= 1".to_string());
        }
        let dw = (w_hi - w_lo) / n as f64;
        let mut rng = SplitMix64::new(seed);
        let mut components = Vec::with_capacity(n);
        for i in 0..n {
            let omega = w_lo + (i as f64 + 0.5) * dw;
            let a = (2.0 * spec.density(omega) * dw).sqrt();
            let k = wavenumber(omega, h, g)?;
            components.push(Component {
                a,
                omega,
                k,
                phase: 2.0 * PI * rng.next_f64(),
            });
        }
        Ok(Self {
            components,
            h,
            g,
            d_omega: dw,
        })
    }

    /// Surface elevation.
    pub fn eta(&self, x: f64, t: f64) -> f64 {
        self.components
            .iter()
            .map(|c| c.a * (c.k * x - c.omega * t + c.phase).cos())
            .sum()
    }

    /// Horizontal and vertical velocity by linear superposition.
    pub fn velocity(&self, x: f64, z: f64, t: f64) -> (f64, f64) {
        let mut u = 0.0;
        let mut w = 0.0;
        for c in &self.components {
            let wave = Wave {
                a: c.a,
                omega: c.omega,
                k: c.k,
                h: self.h,
                g: self.g,
                phase: c.phase,
                order: 1,
            };
            let (du, dw) = wave.velocity(x, z, t);
            u += du;
            w += dw;
        }
        (u, w)
    }

    /// Variance of the record, `sum a_i^2/2`.
    pub fn variance(&self) -> f64 {
        self.components.iter().map(|c| 0.5 * c.a * c.a).sum()
    }

    /// Recovered significant wave height `4 sqrt(m0)`.
    pub fn significant_height(&self) -> f64 {
        4.0 * self.variance().sqrt()
    }

    /// Period `4 pi/d_omega` after which the record repeats exactly when `w_lo` is an integer multiple of `d_omega`
    /// (the midpoint frequencies are then odd multiples of `d_omega/2`); otherwise it is a conservative scale only.
    pub fn repetition_period(&self) -> f64 {
        4.0 * PI / self.d_omega
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Relaxation zones
// ---------------------------------------------------------------------------------------------------------------------

/// Blending shape from the computed solution (weight 1) to the target (weight 0).
#[derive(Debug, Clone, Copy)]
pub enum Blend {
    /// `w = 1 - (exp(chi^p) - 1)/(e - 1)`.
    Exponential {
        /// Exponent.
        p: f64,
    },
    /// `w = (1 - chi)^n`.
    Polynomial {
        /// Order.
        n: f64,
    },
}

impl Blend {
    /// The usual exponential blending, `p = 3.5`.
    pub fn exponential() -> Self {
        Self::Exponential { p: 3.5 }
    }

    /// Weight of the computed solution at the zone coordinate `chi` (0 inner edge, 1 outer edge); exact at the ends.
    pub fn weight(&self, chi: f64) -> f64 {
        if chi.is_nan() || chi <= 0.0 {
            return 1.0;
        }
        if chi >= 1.0 {
            return 0.0;
        }
        match *self {
            Self::Exponential { p } => (1.0 - chi.powf(p).exp_m1() / (std::f64::consts::E - 1.0)).max(0.0),
            Self::Polynomial { n } => (1.0 - chi).powf(n),
        }
    }
}

/// Zone coordinate `chi in [0, 1]` of a point, 0 at `x_inner` and 1 at `x_outer` (either orientation), clamped.
pub fn zone_chi(x: f64, x_inner: f64, x_outer: f64) -> f64 {
    ((x - x_inner) / (x_outer - x_inner)).clamp(0.0, 1.0)
}

/// Relaxation blend `w computed + (1 - w) target`.
pub fn relax(w: f64, computed: f64, target: f64) -> f64 {
    w * computed + (1.0 - w) * target
}

/// Sponge rate profile `sigma_max chi^n` (1/s).
pub fn sponge_rate(chi: f64, sigma_max: f64, n: f64) -> f64 {
    sigma_max * chi.clamp(0.0, 1.0).powf(n)
}

// ---------------------------------------------------------------------------------------------------------------------
// 1-D linear shallow-water tank
// ---------------------------------------------------------------------------------------------------------------------

/// Absorbing zone of the tank.
#[derive(Debug, Clone, Copy)]
pub enum TankZone {
    /// No zone (rigid wall at the right end).
    None,
    /// Relaxation zone `[x_start, x_end]` with blending, applied as a multiplication per step (target = still water).
    Relaxation {
        /// Inner edge, m.
        x_start: f64,
        /// Outer edge, m.
        x_end: f64,
        /// Blending shape.
        blend: Blend,
    },
    /// Sponge `exp(-sigma(chi) dt)` per step.
    Sponge {
        /// Inner edge, m.
        x_start: f64,
        /// Outer edge, m.
        x_end: f64,
        /// Maximum rate, 1/s.
        sigma_max: f64,
        /// Profile exponent.
        n: f64,
    },
}

/// Settings of the tank.
#[derive(Debug, Clone, Copy)]
pub struct TankConfig {
    /// Gravity.
    pub g: f64,
    /// Depth.
    pub h: f64,
    /// Cell size.
    pub dx: f64,
    /// Time step.
    pub dt: f64,
    /// Number of cells.
    pub n_cells: usize,
    /// Incident amplitude.
    pub amplitude: f64,
    /// Incident period.
    pub period: f64,
    /// Start-up ramp time.
    pub ramp_time: f64,
    /// Absorbing zone.
    pub zone: TankZone,
}

/// Linear shallow-water tank: `eta_t = -h u_x`, `u_t = -g eta_x`, staggered leapfrog.
#[derive(Debug, Clone)]
pub struct WaveTank {
    cfg: TankConfig,
    /// Surface elevation at cell centres.
    pub eta: Vec<f64>,
    /// Velocity at faces (`n_cells + 1`).
    pub u: Vec<f64>,
    /// Current time.
    pub t: f64,
    weight_eta: Vec<f64>,
    weight_u: Vec<f64>,
}

impl WaveTank {
    /// Create the tank at rest.
    pub fn new(cfg: TankConfig) -> Result<Self> {
        pos("g", cfg.g)?;
        pos("h", cfg.h)?;
        pos("dx", cfg.dx)?;
        pos("dt", cfg.dt)?;
        pos("period", cfg.period)?;
        pos("ramp_time", cfg.ramp_time)?;
        if cfg.n_cells < 2 {
            return Err("n_cells must be >= 2".to_string());
        }
        let c = (cfg.g * cfg.h).sqrt();
        if c * cfg.dt / cfg.dx > 1.0 {
            return Err(format!("CFL number {} exceeds 1", c * cfg.dt / cfg.dx));
        }
        let n = cfg.n_cells;
        let factor = |x: f64| -> f64 {
            match cfg.zone {
                TankZone::None => 1.0,
                TankZone::Relaxation { x_start, x_end, blend } => blend.weight(zone_chi(x, x_start, x_end)),
                TankZone::Sponge {
                    x_start,
                    x_end,
                    sigma_max,
                    n,
                } => (-sponge_rate(zone_chi(x, x_start, x_end), sigma_max, n) * cfg.dt).exp(),
            }
        };
        let weight_eta = (0..n).map(|i| factor((i as f64 + 0.5) * cfg.dx)).collect();
        let weight_u = (0..=n).map(|j| factor(j as f64 * cfg.dx)).collect();
        Ok(Self {
            cfg,
            eta: vec![0.0; n],
            u: vec![0.0; n + 1],
            t: 0.0,
            weight_eta,
            weight_u,
        })
    }

    /// Incident elevation at the left boundary at time `t` (with ramp).
    pub fn incident(&self, t: f64) -> f64 {
        self.cfg.amplitude * ramp(t, self.cfg.ramp_time) * (2.0 * PI * t / self.cfg.period).sin()
    }

    /// Advance one step.
    pub fn step(&mut self) {
        let TankConfig {
            g,
            h,
            dx,
            dt,
            n_cells: n,
            ..
        } = self.cfg;
        let c = (g * h).sqrt();
        for j in 1..n {
            self.u[j] -= g * dt / dx * (self.eta[j] - self.eta[j - 1]);
        }
        self.u[0] = c / h * self.incident(self.t + 0.5 * dt);
        self.u[n] = 0.0;
        for i in 0..n {
            self.eta[i] -= h * dt / dx * (self.u[i + 1] - self.u[i]);
        }
        self.t += dt;
        if !matches!(self.cfg.zone, TankZone::None) {
            for i in 0..n {
                self.eta[i] *= self.weight_eta[i];
            }
            for j in 0..=n {
                self.u[j] *= self.weight_u[j];
            }
        }
    }
}

/// Amplitude of the harmonic at `omega` in `samples` taken every `dt` starting at time `t_start`:
/// `2 |sum f e^{-i omega t}|/N`. Exact for a pure harmonic when the window holds an integer number of periods.
pub fn harmonic_amplitude(samples: &[f64], dt: f64, t_start: f64, omega: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &f) in samples.iter().enumerate() {
        let t = t_start + i as f64 * dt;
        re += f * (omega * t).cos();
        im -= f * (omega * t).sin();
    }
    2.0 * (re * re + im * im).sqrt() / samples.len() as f64
}
