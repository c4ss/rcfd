//! Injection specifications and size distributions (ADR 0028 D7).

use crate::rng::SplitMix64;
use rcfd_core::vec3::Vec3;

/// Where parcels are injected.
#[derive(Debug, Clone, PartialEq)]
pub enum InjKind {
    /// A point.
    Point(Vec3),
    /// Uniformly over the faces of a boundary patch.
    Surface(String),
}

/// Initial velocity of injected parcels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VelMode {
    /// A given velocity.
    Given(Vec3),
    /// The local fluid velocity.
    Fluid,
}

/// Diameter distribution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeDist {
    /// One diameter.
    Constant(f64),
    /// Rosin-Rammler: oversize mass fraction `exp(-(d/d_bar)^n)`, optionally truncated to `[d_min, d_max]`.
    RosinRammler {
        /// Size constant `d_bar` [m].
        d_bar: f64,
        /// Spread parameter `n`.
        n: f64,
        /// Lower truncation (`None` = none).
        d_min: Option<f64>,
        /// Upper truncation (`None` = none).
        d_max: Option<f64>,
    },
}

/// One injector.
#[derive(Debug, Clone, PartialEq)]
pub struct InjectorSpec {
    /// Name (for messages).
    pub name: String,
    /// Location.
    pub kind: InjKind,
    /// Parcels per second.
    pub rate: f64,
    /// Start time [s].
    pub t_start: f64,
    /// End time [s] (inclusive).
    pub t_end: f64,
    /// Initial velocity.
    pub velocity: VelMode,
    /// Diameter distribution.
    pub dist: SizeDist,
    /// Particle density [kg/m^3].
    pub density: f64,
    /// Real particles per parcel.
    pub parcel_count: f64,
    /// Seed of the injector's generator.
    pub seed: u64,
}

impl InjectorSpec {
    /// Number of parcels emitted up to time `t` (`floor(rate (t - t_start))` with `t` clipped to the window).
    pub fn emitted_by(&self, t: f64) -> u64 {
        if t <= self.t_start {
            return 0;
        }
        let te = t.min(self.t_end);
        let x = self.rate * (te - self.t_start);
        if x <= 0.0 {
            0
        } else {
            (x + 1e-9).floor() as u64
        }
    }

    /// Emission time of the `m`-th parcel (`m >= 1`).
    pub fn emission_time(&self, m: u64) -> f64 {
        self.t_start + m as f64 / self.rate
    }
}

/// Rosin-Rammler oversize mass fraction.
pub fn rr_oversize(d: f64, d_bar: f64, n: f64) -> f64 {
    (-(d / d_bar).powf(n)).exp()
}

/// Rosin-Rammler diameter with oversize fraction `u` (`0 < u <= 1`): `d = d_bar (-ln u)^(1/n)`.
pub fn rr_sample(u: f64, d_bar: f64, n: f64) -> f64 {
    d_bar * (-u.ln()).powf(1.0 / n)
}

impl SizeDist {
    /// Draw a diameter.
    pub fn draw(&self, rng: &mut SplitMix64) -> f64 {
        match *self {
            SizeDist::Constant(d) => d,
            SizeDist::RosinRammler { d_bar, n, d_min, d_max } => {
                let lo = d_min.unwrap_or(0.0);
                let hi = d_max.unwrap_or(f64::INFINITY);
                for _ in 0..10_000 {
                    let d = rr_sample(rng.uniform_open0(), d_bar, n);
                    if d >= lo && d <= hi {
                        return d;
                    }
                }
                // The accepted window has a negligible probability; fall back to the nearer bound.
                lo.max(d_bar.min(hi))
            }
        }
    }
}
