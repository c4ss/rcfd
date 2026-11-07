//! Turbulent dispersion by the discrete random walk (ADR 0057 D2).
//!
//! Homogeneous isotropic Gaussian turbulence of kinetic energy `k` and dissipation `eps`: a particle sees the carrier velocity
//! plus a fluctuation `u'_i = zeta_i sigma`, `sigma = sqrt(2k/3)`, held for the interaction time `min(t_e, t_cross)` of one eddy.

use crate::rng::SplitMix64;
use rcfd_core::vec3::Vec3;

/// `C_mu` of the eddy length `L_e = C_mu^(3/4) k^(3/2)/eps`.
pub const C_MU: f64 = 0.09;

/// Model settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dispersion {
    /// `C_L` of the integral time `T_L = C_L k/eps`.
    pub c_l: f64,
    /// Limit the interaction time by the eddy crossing time.
    pub crossing: bool,
}

impl Default for Dispersion {
    fn default() -> Self {
        Dispersion {
            c_l: 0.15,
            crossing: true,
        }
    }
}

/// Fluctuation standard deviation per component, `sqrt(2k/3)`.
pub fn sigma(k: f64) -> f64 {
    (2.0 * k / 3.0).sqrt()
}

/// Lagrangian integral time `T_L = C_L k/eps`.
pub fn t_lagrangian(c_l: f64, k: f64, eps: f64) -> f64 {
    c_l * k / eps
}

/// Eddy length `L_e = C_mu^(3/4) k^(3/2)/eps`.
pub fn eddy_length(k: f64, eps: f64) -> f64 {
    C_MU.powf(0.75) * k.powf(1.5) / eps
}

/// Eddy crossing time of a Stokes-type particle with effective relaxation time `tau` and relative speed `w` through an eddy of
/// length `l_e`: the relative distance `tau w (1 - exp(-t/tau))` equals `l_e` at `t = -tau ln(1 - l_e/(tau w))`; infinite when
/// the particle stops inside the eddy (`l_e >= tau w`) or does not move relative to it.
pub fn crossing_time(tau: f64, l_e: f64, w: f64) -> f64 {
    let x = l_e / (tau * w);
    if tau.is_finite() && x.is_finite() && x < 1.0 {
        -tau * (-x).ln_1p()
    } else {
        f64::INFINITY
    }
}

/// `(1 - exp(-y)) / y`, 1 at `y = 0`, without cancellation for small `y`.
fn phi(y: f64) -> f64 {
    if y == 0.0 {
        1.0
    } else {
        -(-y).exp_m1() / y
    }
}

/// `(exp(-c t) - exp(-e t)) / ((e - c) t)`, finite and free of cancellation when `c = e`.
fn divided_exp(c: f64, e: f64, t: f64) -> f64 {
    (-c.min(e) * t).exp() * phi((e - c).abs() * t)
}

/// Taylor's mean-square displacement per component of a fluid particle whose velocity has the autocorrelation
/// `sigma2 exp(-abs(s)/t_l)` (ADR 0208 D3 with `tau = 0`): `2 sigma2 t_l^2 (t/t_l - 1 + exp(-t/t_l))`.
pub fn taylor_msd(sigma2: f64, t_l: f64, t: f64) -> f64 {
    let x = t / t_l;
    2.0 * sigma2 * t_l * t_l * (x + (-x).exp_m1())
}

/// Mean-square displacement per component at time `t` of a Stokes particle of relaxation time `tau` released at rest at
/// `t = 0` in a stationary fluid velocity of autocorrelation `sigma2 exp(-abs(s)/t_l)` (ADR 0208 D3):
/// `sigma2 (I0 - 2 I1 + I2)`. It is the random walk of [`Eddy::draw`] without crossing times in homogeneous fields with no
/// mean flow.
pub fn drw_msd(sigma2: f64, t_l: f64, tau: f64, t: f64) -> f64 {
    let (a, b) = (1.0 / tau, 1.0 / t_l);
    let i0 = 2.0 * t_l * t_l * (t / t_l + (-t / t_l).exp_m1());
    let i1 = t_l * (-2.0 * (-a * t).exp_m1() / a + (-(a + b) * t).exp_m1() / (a + b) - t * divided_exp(b, a, t));
    let i2 = (-(-(a + b) * t).exp_m1() - (a + b) * t * divided_exp(a + b, 2.0 * a, t)) / (a * (a + b));
    sigma2 * (i0 - 2.0 * i1 + i2)
}

/// The eddy currently seen by one parcel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eddy {
    /// Velocity fluctuation [m/s].
    pub uprime: Vec3,
    /// Remaining interaction time [s]; `<= 0` = draw a new eddy.
    pub t_left: f64,
}

impl Default for Eddy {
    fn default() -> Self {
        Eddy {
            uprime: [0.0; 3],
            t_left: 0.0,
        }
    }
}

impl Eddy {
    /// Draw a new eddy. `tau_eff` is the effective relaxation time of the particle (infinite without drag), `u_mean` the mean
    /// carrier velocity and `v` the particle velocity (for the crossing time).
    pub fn draw(
        &mut self,
        rng: &mut SplitMix64,
        m: &Dispersion,
        k: f64,
        eps: f64,
        tau_eff: f64,
        u_mean: Vec3,
        v: Vec3,
    ) {
        if !(k > 0.0 && eps > 0.0) {
            self.uprime = [0.0; 3];
            self.t_left = f64::INFINITY;
            return;
        }
        let s = sigma(k);
        let up = [rng.gauss() * s, rng.gauss() * s, rng.gauss() * s];
        let tl = t_lagrangian(m.c_l, k, eps);
        let mut t = -tl * rng.uniform_open0().ln();
        if m.crossing {
            let w = rcfd_core::vec3::norm(rcfd_core::vec3::sub(rcfd_core::vec3::add(u_mean, up), v));
            t = t.min(crossing_time(tau_eff, eddy_length(k, eps), w));
        }
        self.uprime = up;
        self.t_left = t.max(1e-12 * tl);
    }
}
