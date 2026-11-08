//! Droplet breakup by the TAB method of O'Rourke and Amsden (1987) (ADR 0224 B1 to B6; library only).
//!
//! The distortion `y` of a drop obeys the forced damped oscillator `ydd = (C_F/C_b)(rho_g/rho_l)(u^2/r^2) -
//! (C_k sigma/(rho_l r^3)) y - (C_d mu_l/(rho_l r^2)) yd` with the coefficients held over a sub-step; the drop breaks when
//! `y` reaches 1, at the time given by the undamped oscillation of the current state (the report's eq. 26). The product
//! drops take the Sauter mean radius of the energy relation (eq. 22), the parcel count follows the liquid mass, and the
//! parcel gains the normal velocity `C_v C_b r yd` (eq. 14) in a random direction normal to the relative velocity.

use crate::rng::SplitMix64;
use crate::{err, Result};
use std::f64::consts::PI;

/// The breakup model and its constants (ADR 0224 B1).
#[derive(Debug, Clone, PartialEq)]
pub struct BreakupSpec {
    /// Surface tension `sigma` [N/m].
    pub surface_tension: f64,
    /// Liquid dynamic viscosity `mu_l` [Pa s].
    pub liquid_viscosity: f64,
    /// Forcing constant `C_F`.
    pub c_f: f64,
    /// Spring constant `C_k`.
    pub c_k: f64,
    /// Damping constant `C_d`.
    pub c_d: f64,
    /// Breakup amplitude constant `C_b`.
    pub c_b: f64,
    /// Ratio `K` of the energy in all modes to the energy in the fundamental mode.
    pub k: f64,
    /// Normal-velocity constant `C_v`.
    pub c_v: f64,
    /// Seed of the generator of the normal-velocity directions.
    pub seed: u64,
}

impl BreakupSpec {
    /// The TAB constants of O'Rourke and Amsden (1987), eqs. 7 to 10, 14 and the text after eq. 22: `C_F = 1/3`, `C_k = 8`,
    /// `C_d = 5`, `C_b = 1/2`, `K = 10/3`, `C_v = 1`.
    pub fn tab(surface_tension: f64, liquid_viscosity: f64, seed: u64) -> BreakupSpec {
        BreakupSpec {
            surface_tension,
            liquid_viscosity,
            c_f: 1.0 / 3.0,
            c_k: 8.0,
            c_d: 5.0,
            c_b: 0.5,
            k: 10.0 / 3.0,
            c_v: 1.0,
            seed,
        }
    }

    /// Check the spec (ADR 0224 B6); the error names the first defect.
    pub fn validate(&self) -> Result<()> {
        let pos = |x: f64| x.is_finite() && x > 0.0;
        let nonneg = |x: f64| x.is_finite() && x >= 0.0;
        if !pos(self.surface_tension) {
            return err("breakup: the surface tension must be finite and positive");
        }
        if !nonneg(self.liquid_viscosity) {
            return err("breakup: the liquid viscosity must be finite and not negative");
        }
        if !(pos(self.c_f) && pos(self.c_k) && pos(self.c_b)) {
            return err("breakup: C_F, C_k and C_b must be finite and positive");
        }
        if !(nonneg(self.c_d) && nonneg(self.c_v)) {
            return err("breakup: C_d and C_v must be finite and not negative");
        }
        if !(self.k.is_finite() && self.k >= 5.0 / 6.0) {
            return err("breakup: K must be finite and at least 5/6 (ADR 0224 B6)");
        }
        Ok(())
    }

    /// Critical Weber number `C_k C_b/(2 C_F)` of a drop at rest without viscosity (eq. 6).
    pub fn critical_weber(&self) -> f64 {
        self.c_k * self.c_b / (2.0 * self.c_f)
    }
}

/// The oscillator coefficients of a drop over a sub-step (eqs. 3, 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oscillator {
    /// Equilibrium distortion `W = C_F We/(C_k C_b)`.
    pub w: f64,
    /// Damping rate `1/t_d = C_d mu_l/(2 rho_l r^2)`.
    pub inv_td: f64,
    /// `omega^2 = C_k sigma/(rho_l r^3) - 1/t_d^2`.
    pub omega2: f64,
}

/// The coefficients for a drop of radius `r` and density `rho_l` at the relative speed `u` in a gas of density `rho_g`.
pub fn oscillator(spec: &BreakupSpec, rho_g: f64, rho_l: f64, r: f64, u: f64) -> Oscillator {
    let we = rho_g * u * u * r / spec.surface_tension;
    let inv_td = spec.c_d * spec.liquid_viscosity / (2.0 * rho_l * r * r);
    Oscillator {
        w: spec.c_f * we / (spec.c_k * spec.c_b),
        inv_td,
        omega2: spec.c_k * spec.surface_tension / (rho_l * r * r * r) - inv_td * inv_td,
    }
}

impl Oscillator {
    /// Time from now to breakup by eq. 26 (the undamped oscillation of the state `(y, yd)`), infinite when the drop cannot
    /// break (`omega^2 <= 0` or `W + A <= 1`), 0 when `y >= 1` (ADR 0224 B3).
    pub fn time_to_breakup(&self, y: f64, yd: f64) -> f64 {
        if self.omega2 <= 0.0 {
            return f64::INFINITY;
        }
        if y >= 1.0 {
            return 0.0;
        }
        let om = self.omega2.sqrt();
        let (cx, sx) = (y - self.w, -yd / om);
        let a = cx.hypot(sx);
        if self.w + a <= 1.0 {
            return f64::INFINITY;
        }
        let alpha = ((1.0 - self.w) / a).clamp(-1.0, 1.0).acos();
        let phi = sx.atan2(cx);
        if phi < -alpha {
            (-alpha - phi) / om
        } else if phi > alpha {
            (2.0 * PI - alpha - phi) / om
        } else {
            0.0
        }
    }

    /// The state after `s` by eqs. 25a, 25b (`omega^2 > 0`); `(0, 0)` when `omega^2 <= 0`.
    pub fn advance(&self, y: f64, yd: f64, s: f64) -> (f64, f64) {
        if self.omega2 <= 0.0 {
            return (0.0, 0.0);
        }
        let om = self.omega2.sqrt();
        let a = y - self.w;
        let b = (yd + a * self.inv_td) / om;
        let e = (-s * self.inv_td).exp();
        let (sn, cs) = (om * s).sin_cos();
        let y1 = self.w + e * (a * cs + b * sn);
        let yd1 = (self.w - y1) * self.inv_td + om * e * (b * cs - a * sn);
        (y1, yd1)
    }
}

/// Sauter mean radius of the product drops, eq. 22 with `y = 1`: `r/(1 + 8K/20 + rho_l r^3 yd^2 (6K - 5)/(120 sigma))`.
pub fn sauter_radius(spec: &BreakupSpec, rho_l: f64, r: f64, yd: f64) -> f64 {
    r / (1.0
        + 8.0 * spec.k / 20.0
        + rho_l * r * r * r * yd * yd * (6.0 * spec.k - 5.0) / (120.0 * spec.surface_tension))
}

/// Normal speed of the product drops, eq. 14: `C_v C_b r yd`.
pub fn normal_speed(spec: &BreakupSpec, r: f64, yd: f64) -> f64 {
    spec.c_v * spec.c_b * r * yd
}

/// The breakup model with its generator and event count.
#[derive(Debug, Clone)]
pub struct Breakup {
    spec: BreakupSpec,
    pub(crate) rng: SplitMix64,
    events: u64,
}

impl Breakup {
    /// The model (validated).
    pub fn new(spec: BreakupSpec) -> Result<Breakup> {
        spec.validate()?;
        let rng = SplitMix64::new(spec.seed);
        Ok(Breakup { spec, rng, events: 0 })
    }

    /// The spec.
    pub fn spec(&self) -> &BreakupSpec {
        &self.spec
    }

    /// Breakup events since the start.
    pub fn events(&self) -> u64 {
        self.events
    }

    pub(crate) fn count(&mut self) {
        self.events += 1;
    }
}
