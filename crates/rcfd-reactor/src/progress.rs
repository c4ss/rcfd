//! Premixed progress-variable tools (roadmap F-129, ADR 0053 D7): the normalised progress variable, a tabulated source with its beta-PDF
//! mean, a one-step Arrhenius source, and the steady one-dimensional laminar flame eigenvalue by the phase-plane method.

use crate::mixfrac::{integrate_beta, interp, MixError};

/// Progress variable `c = (phi - phi_u)/(phi_b - phi_u)` of a state variable between its unburnt and burnt values.
pub fn progress(value: f64, unburnt: f64, burnt: f64) -> Result<f64, MixError> {
    if burnt == unburnt {
        return Err(MixError(format!(
            "progress variable: unburnt value {unburnt} equals the burnt value {burnt}"
        )));
    }
    Ok((value - unburnt) / (burnt - unburnt))
}

/// Tabulated source `omega(c)` on a grid in `[0, 1]`, linear lookup.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceTable {
    /// Grid, strictly increasing.
    pub c: Vec<f64>,
    /// Source at the nodes.
    pub omega: Vec<f64>,
}

impl SourceTable {
    /// Tabulate a function on `n` equal intervals of `[0, 1]`.
    pub fn from_fn(n: usize, f: &dyn Fn(f64) -> f64) -> SourceTable {
        let c: Vec<f64> = (0..=n).map(|i| i as f64 / n as f64).collect();
        let omega = c.iter().map(|&x| f(x)).collect();
        SourceTable { c, omega }
    }

    /// Linear lookup.
    pub fn lookup(&self, c: f64) -> f64 {
        interp(&self.c, &self.omega, c)
    }

    /// Beta-PDF mean of the source at mean progress `cm` and variance `v`.
    pub fn mean(&self, cm: f64, v: f64) -> Result<f64, MixError> {
        integrate_beta(&self.c, &self.omega, cm, v)
    }
}

/// One-step Arrhenius source for the progress variable: `omega(c) = a (1 - c) exp(-ta/T(c))`, `T(c) = t_u + c (t_b - t_u)` (first order in
/// the unburnt fraction; `a` in 1/s, `ta` the activation temperature in K).
pub fn arrhenius_source(a: f64, ta: f64, t_u: f64, t_b: f64) -> impl Fn(f64) -> f64 {
    move |c: f64| a * (1.0 - c) * (-ta / (t_u + c * (t_b - t_u))).exp()
}

/// Result of the phase-plane flame calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct Front {
    /// Mass flux `m` (kg/m2/s) of the steady flame, `m c' = Gamma c'' + omega(c)`.
    pub m: f64,
    /// Grid in `c` of the profile `g(c) = dc/dx`.
    pub c: Vec<f64>,
    /// `g(c) = c'` along the heteroclinic orbit.
    pub g: Vec<f64>,
}

/// Outcome of one shot from the hot saddle.
enum Shot {
    /// The orbit reached `c_end` with `g > 0`: the flux was too small (`g` keeps growing as `c` falls while `m g < omega`).
    Positive(Vec<f64>),
    /// `g` reached zero before `c_end`: the flux was too large.
    HitsZero,
}

/// Steady planar flame `m c' = Gamma c'' + omega(c)` with `c(-inf) = 0`, `c(+inf) = 1` and `Gamma = rho D` constant. Phase plane
/// `g(c) = c'`: `Gamma g dg/dc = m g - omega(c)`, started at the hot saddle on the stable manifold `g = s (1 - c)` with
/// `Gamma s^2 + m s + omega'(1) = 0` (`d1 = omega'(1) <= 0`) and integrated by RK4 down to `c_end`; `m` by bisection on whether the orbit
/// reaches `c_end` with `g > 0` (flux too small) or hits zero first (flux too large). `omega` must vanish at `c = 0` and `c = 1` (a combustion-type source).
pub fn front_speed(gamma: f64, omega: &dyn Fn(f64) -> f64, d1: f64) -> Result<Front, MixError> {
    if !(gamma > 0.0) {
        return Err(MixError(format!("diffusivity gamma = {gamma} must be positive")));
    }
    if !(d1 < 0.0) {
        return Err(MixError(format!(
            "omega'(1) = {d1} must be negative (a stable burnt state)"
        )));
    }
    let nstep = 40_000usize;
    let eta = 1e-6;
    let c_end = 1e-5;
    let shoot = |m: f64| -> Shot {
        let s = (-m + (m * m - 4.0 * gamma * d1).sqrt()) / (2.0 * gamma);
        let mut c = 1.0 - eta;
        let mut g = s * eta;
        let h = (c - c_end) / nstep as f64;
        let rhs = |c: f64, g: f64| (m * g - omega(c)) / (gamma * g);
        let mut prof = Vec::with_capacity(nstep + 1);
        prof.push(g);
        for _ in 0..nstep {
            // integrate toward decreasing c
            let k1 = rhs(c, g);
            let k2 = rhs(c - 0.5 * h, g - 0.5 * h * k1);
            let k3 = rhs(c - 0.5 * h, g - 0.5 * h * k2);
            let k4 = rhs(c - h, g - h * k3);
            g -= h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
            c -= h;
            if !(g > 0.0) || !g.is_finite() {
                return Shot::HitsZero;
            }
            prof.push(g);
        }
        Shot::Positive(prof)
    };
    // bracket: positive orbit at small m, zero-hitting orbit at large m
    let mut lo = 1e-12;
    let mut hi = 1.0;
    let mut tries = 0;
    while matches!(shoot(hi), Shot::Positive(_)) {
        hi *= 4.0;
        tries += 1;
        if tries > 60 {
            return Err(MixError(
                "front speed: no flux with a zero-hitting orbit up to the search limit".into(),
            ));
        }
    }
    if matches!(shoot(lo), Shot::HitsZero) {
        return Err(MixError(
            "front speed: the orbit hits zero even at vanishing flux".into(),
        ));
    }
    for _ in 0..70 {
        let mid = 0.5 * (lo + hi);
        match shoot(mid) {
            Shot::Positive(_) => lo = mid,
            Shot::HitsZero => hi = mid,
        }
    }
    let m = 0.5 * (lo + hi);
    let prof = match shoot(lo) {
        Shot::Positive(p) => p,
        Shot::HitsZero => return Err(MixError("front speed: bisection ended on an invalid flux".into())),
    };
    let h = (1.0 - eta - c_end) / nstep as f64;
    let c: Vec<f64> = (0..=nstep).map(|i| 1.0 - eta - i as f64 * h).collect();
    Ok(Front { m, c, g: prof })
}

impl Front {
    /// `g` at `c` (linear interpolation of the orbit).
    pub fn g_at(&self, c: f64) -> f64 {
        // c descends; search the bracket
        let n = self.c.len();
        let mut lo = 0usize;
        let mut hi = n - 1;
        if c >= self.c[0] {
            return self.g[0];
        }
        if c <= self.c[n - 1] {
            return self.g[n - 1];
        }
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.c[mid] > c {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let w = (self.c[lo] - c) / (self.c[lo] - self.c[hi]);
        self.g[lo] + w * (self.g[hi] - self.g[lo])
    }
}
