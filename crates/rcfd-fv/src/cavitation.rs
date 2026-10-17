//! Schnerr-Sauer cavitation mass transfer with a bubble-number density (F-127, ADR 0056 D).
//!
//! Uniform spherical bubbles, `n` per unit volume of LIQUID, `x = n (4/3) pi R^3`, `alpha_v = x/(1 + x)`. Inertia-controlled growth
//! (Rayleigh-Plesset without surface tension and viscosity) gives the radius rate `R' = sign(p_sat - p) sqrt(2 |p_sat - p|/(3 rho_l))`
//! and, for incompressible phases, the vapour mass source
//! `mdot = (rho_v rho_l/rho_m) alpha_v (1 - alpha_v) (3/R) R'` (positive = evaporation, kg/(m^3 s)).

use std::f64::consts::PI;

/// Schnerr-Sauer parameters.
#[derive(Debug, Clone, Copy)]
pub struct Cavitation {
    /// Liquid phase: 1 (`alpha = 1`) or 2.
    pub liquid_phase: usize,
    /// Saturation (vapour) pressure, Pa.
    pub p_sat: f64,
    /// Bubble-number density per unit volume of liquid, 1/m^3.
    pub n0: f64,
    /// Nucleus radius, m (the radius never falls below it; also the inception seed).
    pub r_nucleus: f64,
    /// Evaporation factor (1 = the pure inertial rate).
    pub f_vap: f64,
    /// Condensation factor (1 = the pure inertial rate).
    pub f_cond: f64,
}

impl Cavitation {
    /// Vapour fraction of the nuclei, `x_0/(1 + x_0)`.
    pub fn alpha_floor(&self) -> f64 {
        let x0 = self.n0 * 4.0 / 3.0 * PI * self.r_nucleus.powi(3);
        x0 / (1.0 + x0)
    }

    /// Bubble radius for a vapour fraction in `(0, 1)`.
    pub fn radius(&self, alpha_v: f64) -> f64 {
        (3.0 * alpha_v / (4.0 * PI * self.n0 * (1.0 - alpha_v))).cbrt()
    }

    /// Vapour fraction of a bubble of radius `r`.
    pub fn alpha_of_radius(&self, r: f64) -> f64 {
        let x = self.n0 * 4.0 / 3.0 * PI * r.powi(3);
        x / (1.0 + x)
    }

    /// Radius rate `R'` (m/s, signed) at pressure `p`, including the evaporation / condensation factors.
    pub fn radius_rate(&self, rho_l: f64, p: f64) -> f64 {
        let dp = self.p_sat - p;
        if dp == 0.0 {
            return 0.0;
        }
        let r = (2.0 * dp.abs() / (3.0 * rho_l)).sqrt();
        if dp > 0.0 {
            self.f_vap * r
        } else {
            -self.f_cond * r
        }
    }

    /// Vapour mass source `mdot` (kg/(m^3 s)) of the Schnerr-Sauer form; zero at `alpha_v` outside `(0, 1)` and at saturation.
    pub fn rate(&self, alpha_v: f64, rho_l: f64, rho_v: f64, p: f64) -> f64 {
        if !(alpha_v > 0.0 && alpha_v < 1.0) {
            return 0.0;
        }
        let rdot = self.radius_rate(rho_l, p);
        if rdot == 0.0 {
            return 0.0;
        }
        let rho_m = alpha_v * rho_v + (1.0 - alpha_v) * rho_l;
        rho_v * rho_l / rho_m * alpha_v * (1.0 - alpha_v) * 3.0 / self.radius(alpha_v) * rdot
    }

    /// Advance the vapour fraction over `dt` at the frozen pressure `p`: the radius is advanced exactly, `R' = R + R'_rate dt`,
    /// bounded below by the nucleus radius (a bubble already smaller than the nucleus does not shrink further). The result is
    /// monotone in the sign of `p_sat - p`, finite, and in `[min(alpha_v, alpha_floor), 1]` for any `dt`.
    pub fn advance(&self, alpha_v: f64, rho_l: f64, p: f64, dt: f64) -> f64 {
        let rdot = self.radius_rate(rho_l, p);
        if rdot == 0.0 || !(alpha_v > 0.0 && alpha_v < 1.0) {
            return alpha_v;
        }
        let r = self.radius(alpha_v);
        let r_new = if rdot > 0.0 {
            r + rdot * dt
        } else {
            (r + rdot * dt).max(self.r_nucleus.min(r))
        };
        let a = self.alpha_of_radius(r_new);
        if rdot > 0.0 {
            a.max(alpha_v).min(1.0)
        } else {
            a.min(alpha_v).max(0.0)
        }
    }
}
