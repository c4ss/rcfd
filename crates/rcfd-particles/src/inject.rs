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
    /// Diameter uniform in `[d_min, d_max]` (ADR 0057 D3).
    Uniform {
        /// Lower bound [m].
        d_min: f64,
        /// Upper bound [m].
        d_max: f64,
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
            SizeDist::Uniform { d_min, d_max } => d_min + rng.uniform() * (d_max - d_min),
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

/// Hollow or solid cone of initial directions (ADR 0057 D3): `cos(phi)` uniform on `[cos(half_angle), cos(inner_half_angle)]`
/// (uniform in solid angle), azimuth uniform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConeSpec {
    /// Axis (any non-zero vector; normalised on use).
    pub axis: Vec3,
    /// Half angle `theta` in `(0, pi]` [rad].
    pub half_angle: f64,
    /// Inner half angle in `[0, theta)` [rad] (0 = solid cone).
    pub inner_half_angle: f64,
    /// Speed [m/s] (> 0).
    pub speed: f64,
}

impl ConeSpec {
    /// Direction for the uniform numbers `u1, u2` in `[0, 1)`.
    pub fn direction(&self, u1: f64, u2: f64) -> Vec3 {
        use rcfd_core::vec3::{add, cross, scale, unit};
        let a = unit(self.axis);
        let (c0, c1) = (self.half_angle.cos(), self.inner_half_angle.cos());
        let c = (c0 + u1 * (c1 - c0)).clamp(-1.0, 1.0);
        let s = (1.0 - c * c).max(0.0).sqrt();
        let psi = 2.0 * std::f64::consts::PI * u2;
        // Helper vector least aligned with the axis.
        let h = if a[0].abs() <= a[1].abs() && a[0].abs() <= a[2].abs() {
            [1.0, 0.0, 0.0]
        } else if a[1].abs() <= a[2].abs() {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let e1 = unit(cross(a, h));
        let e2 = cross(a, e1);
        add(scale(a, c), add(scale(e1, s * psi.cos()), scale(e2, s * psi.sin())))
    }
}

/// Optional extensions of an injector (ADR 0057 D3), stored beside the [`InjectorSpec`] so that existing literals stay valid.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct InjectorExt {
    /// Cone of directions (overrides the velocity mode).
    pub cone: Option<ConeSpec>,
    /// Mass flow [kg/s]: equal-mass parcels, weight `(mass_flow/rate)/(rho_p pi d^3/6)`.
    pub mass_flow: Option<f64>,
    /// Initial particle temperature [K] (with heat transfer; default the carrier temperature at injection).
    pub temperature: Option<f64>,
}

impl InjectorExt {
    /// Check the values; the message names the offending field.
    pub fn validate(&self) -> std::result::Result<(), (&'static str, String)> {
        if let Some(c) = &self.cone {
            if !(c.half_angle > 0.0 && c.half_angle <= std::f64::consts::PI) {
                return Err(("cone_half_angle", "must be in (0, pi] radians".into()));
            }
            if !(c.inner_half_angle >= 0.0 && c.inner_half_angle < c.half_angle) {
                return Err(("cone_inner_half_angle", "must be in [0, cone_half_angle)".into()));
            }
            if !(c.speed.is_finite() && c.speed > 0.0) {
                return Err(("cone_speed", "must be positive".into()));
            }
            if !(c.axis.iter().all(|x| x.is_finite()) && rcfd_core::vec3::norm(c.axis) > 0.0) {
                return Err(("cone_axis", "must be a non-zero vector".into()));
            }
        }
        if let Some(m) = self.mass_flow {
            if !(m.is_finite() && m > 0.0) {
                return Err(("mass_flow", "must be positive".into()));
            }
        }
        Ok(())
    }
}
