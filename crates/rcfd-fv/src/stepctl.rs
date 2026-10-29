//! Adaptive time stepping and convergence accelerators for the transient PISO/PIMPLE solver (F-209, ADR 0050).
//!
//! * Courant-number control with a MAX and/or a volume-weighted RMS criterion, safety factor, growth / shrink limits and
//!   `min_dt` / `max_dt` ([`StepController`]).
//! * Local-error control by step doubling for the implicit Euler scheme, per unit time: a step is accepted when
//!   `e / dt <= tol` with `e = max|u_2 - u_1| / max|u_2|` ([`ControlledRun`]); the global error then scales linearly with the
//!   tolerance (error-per-step control would scale as `tol^(p/(p+1))`).
//! * Period-based steps (`dt = T/N`) and exact landing on write times: targets are `k * grid` from an integer counter or entries
//!   of a sorted list, never accumulated; the time is snapped to the target after the landing step.
//! * Extrapolated initial guess of the next step ([`extrapolate_initial`]) and Aitken delta-squared relaxation ([`Aitken`],
//!   a library block, not wired into the PIMPLE outer loop).
//!
//! The module only uses the public interface of [`PimpleSolver`]; with adaptivity unused nothing changes.

use crate::ddt::{courant_numbers, TimeScheme};
use crate::geometry::FvGeometry;
use crate::pimple::{PimpleSolver, StepInfo};
use rcfd_core::vec3::Vec3;

/// Courant statistics of a flux field for a given step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CourantStats {
    /// Largest cell Courant number.
    pub max: f64,
    /// Volume-weighted root mean square, `sqrt(sum V Co^2 / sum V)`.
    pub rms: f64,
}

/// Courant statistics `0.5 sum|F| dt / V` of the flux `flux` for the step `dt`.
pub fn courant_stats(geom: &FvGeometry, flux: &[f64], dt: f64) -> CourantStats {
    let co = courant_numbers(geom, flux, dt);
    let vol = geom.mesh.cell_volumes();
    let (mut max, mut s, mut v) = (0.0_f64, 0.0, 0.0);
    for (c, x) in co.iter().enumerate() {
        max = max.max(*x);
        s += vol[c] * x * x;
        v += vol[c];
    }
    CourantStats {
        max,
        rms: (s / v.max(1e-300)).sqrt(),
    }
}

/// Settings of the step controller. Every control is off at its sentinel (0 / empty).
#[derive(Debug, Clone, PartialEq)]
pub struct StepSettings {
    /// Target MAX Courant number (0 = criterion off).
    pub co_max: f64,
    /// Target RMS Courant number (0 = criterion off).
    pub co_rms: f64,
    /// The step aims at `safety * target`.
    pub safety: f64,
    /// Largest growth factor per step.
    pub growth: f64,
    /// Smallest factor per step (the step shrinks at most by this factor).
    pub shrink: f64,
    /// Smallest step.
    pub min_dt: f64,
    /// Largest step.
    pub max_dt: f64,
    /// Error tolerance per unit time of the step-doubling control (0 = off).
    pub error_tol: f64,
    /// Period `T` of period-based steps (0 = off).
    pub period: f64,
    /// Steps per period `N`.
    pub steps_per_period: usize,
    /// Sorted list of times to land on exactly.
    pub write_times: Vec<f64>,
    /// Land on every multiple of this interval (0 = off).
    pub write_interval: f64,
}

impl Default for StepSettings {
    fn default() -> Self {
        StepSettings {
            co_max: 0.0,
            co_rms: 0.0,
            safety: 0.9,
            growth: 1.2,
            shrink: 0.5,
            min_dt: 0.0,
            max_dt: f64::INFINITY,
            error_tol: 0.0,
            period: 0.0,
            steps_per_period: 0,
            write_times: Vec::new(),
            write_interval: 0.0,
        }
    }
}

impl StepSettings {
    /// Whether any control is active (otherwise the fixed step of the caller is used).
    pub fn is_active(&self) -> bool {
        self.co_max > 0.0
            || self.co_rms > 0.0
            || self.error_tol > 0.0
            || self.period > 0.0
            || !self.write_times.is_empty()
            || self.write_interval > 0.0
    }

    /// Check the settings; the error message names the offending field.
    pub fn validate(&self) -> Result<(), (&'static str, String)> {
        let e = |k: &'static str, m: &str| Err((k, m.to_string()));
        if self.co_max < 0.0 || self.co_rms < 0.0 {
            return e("max_courant", "Courant targets must not be negative");
        }
        if !(self.safety > 0.0 && self.safety <= 1.0) {
            return e("dt_safety", "the safety factor must lie in (0, 1]");
        }
        if self.growth < 1.0 {
            return e("dt_growth", "the growth factor must be at least 1");
        }
        if !(self.shrink > 0.0 && self.shrink <= 1.0) {
            return e("dt_shrink", "the shrink factor must lie in (0, 1]");
        }
        if self.min_dt < 0.0 || self.max_dt <= 0.0 || self.min_dt > self.max_dt {
            return e("min_dt", "need 0 <= min_dt <= max_dt and max_dt > 0");
        }
        if self.error_tol < 0.0 {
            return e("error_tolerance", "the tolerance must not be negative");
        }
        if self.period < 0.0 {
            return e("period", "the period must not be negative");
        }
        if (self.period > 0.0) != (self.steps_per_period > 0) {
            return e("steps_per_period", "period and steps_per_period must be given together");
        }
        if self.period > 0.0 && (self.co_max > 0.0 || self.co_rms > 0.0 || self.error_tol > 0.0) {
            return e(
                "period",
                "period-based steps cannot be combined with Courant or error control",
            );
        }
        if self.error_tol > 0.0 && (self.co_max > 0.0 || self.co_rms > 0.0) {
            return e(
                "error_tolerance",
                "error control cannot be combined with Courant control",
            );
        }
        if self.write_interval < 0.0 {
            return e("write_time_interval", "the interval must not be negative");
        }
        if self.write_times.windows(2).any(|w| w[1] <= w[0]) || self.write_times.iter().any(|t| *t <= 0.0) {
            return e("write_times", "write times must be positive and strictly increasing");
        }
        Ok(())
    }

    /// Next landing target strictly after `t` among the list, the interval grid and the period grid.
    pub fn next_target(&self, t: f64) -> Option<f64> {
        let mut best: Option<f64> = None;
        let mut offer = |x: f64| {
            if best.map_or(true, |b| x < b) {
                best = Some(x);
            }
        };
        let tol = 1e-12 * t.abs().max(1e-300);
        if let Some(x) = self.write_times.iter().find(|x| **x > t + tol) {
            offer(*x);
        }
        let mut grid = |g: f64| {
            if g > 0.0 {
                let mut k = (t / g + 1e-9).floor() + 1.0;
                while k * g <= t + tol {
                    k += 1.0;
                }
                offer(k * g);
            }
        };
        grid(self.write_interval);
        if self.period > 0.0 && self.steps_per_period > 0 {
            grid(self.period / self.steps_per_period as f64);
        }
        best
    }
}

/// Controller state (serialisable) and step proposals.
#[derive(Debug, Clone)]
pub struct StepController {
    /// Settings.
    pub settings: StepSettings,
    /// Proposed next step (the unshortened proposal; landing may shorten the step actually taken).
    pub dt_next: f64,
    /// Accepted steps.
    pub steps: usize,
    /// Rejected trial steps (error control).
    pub rejected: usize,
}

impl StepController {
    /// Controller whose first proposal is `dt0` (period mode starts with `T/N`).
    pub fn new(settings: StepSettings, dt0: f64) -> Self {
        let dt_next = if settings.period > 0.0 && settings.steps_per_period > 0 {
            settings.period / settings.steps_per_period as f64
        } else {
            dt0
        };
        StepController {
            settings,
            dt_next,
            steps: 0,
            rejected: 0,
        }
    }

    /// Serialise the state (`dt_next`, steps, rejected).
    pub fn to_f64s(&self) -> Vec<f64> {
        vec![self.dt_next, self.steps as f64, self.rejected as f64]
    }

    /// Restore the state written by [`to_f64s`](Self::to_f64s).
    pub fn restore_f64s(&mut self, v: &[f64]) -> Result<(), String> {
        if v.len() != 3 || !v[0].is_finite() || v[0] <= 0.0 {
            return Err("step-controller state must be three numbers with a positive step".into());
        }
        self.dt_next = v[0];
        self.steps = v[1] as usize;
        self.rejected = v[2] as usize;
        Ok(())
    }

    /// Proposal from the Courant statistics `stats` measured for the step `d` that was taken after proposing `dt_prop`:
    /// the minimum over the active criteria of `dt_prop aim/Co(dt_prop)`, limited by the growth / shrink factors relative to
    /// `dt_prop` and by `min_dt` / `max_dt`. `Co` scales with the step, `Co(dt_prop) = Co(d) dt_prop/d`.
    pub fn propose_courant(&self, stats: CourantStats, d: f64, dt_prop: f64) -> f64 {
        let s = &self.settings;
        let scale = dt_prop / d;
        let mut dt = dt_prop * s.growth;
        for (target, co) in [(s.co_max, stats.max), (s.co_rms, stats.rms)] {
            if target > 0.0 && co > 1e-300 {
                dt = dt.min(dt_prop * s.safety * target / (co * scale));
            }
        }
        self.clamp(dt.max(dt_prop * s.shrink))
    }

    /// Proposal from an error rate `e/dt` of a step with proposal `dt_prop` (order `p` of the scheme).
    pub fn propose_error(&self, rate: f64, dt_prop: f64, p: f64) -> f64 {
        let s = &self.settings;
        let f = if rate > 0.0 {
            (s.safety * (s.error_tol / rate).powf(1.0 / p)).clamp(s.shrink, s.growth)
        } else {
            s.growth
        };
        self.clamp(dt_prop * f)
    }

    fn clamp(&self, dt: f64) -> f64 {
        dt.min(self.settings.max_dt).max(self.settings.min_dt)
    }

    /// Step to take from the proposal `dt` at time `t` with the final time `t_end`: `(step, target)`. When the nearest target
    /// (write time or end time) is within `LAND_STRETCH * dt` the step is stretched or shortened to land on it and `target` is
    /// `Some`; otherwise the full proposal is taken (only the landing step is changed, so the Courant band holds elsewhere).
    pub fn land(&self, t: f64, dt: f64, t_end: f64) -> (f64, Option<f64>) {
        let mut target = t_end;
        if let Some(x) = self.settings.next_target(t) {
            if x < target {
                target = x;
            }
        }
        let r = target - t;
        if r <= 0.0 {
            return (dt, None);
        }
        if r <= LAND_STRETCH * dt {
            (r, Some(target))
        } else {
            (dt, None)
        }
    }
}

/// A step may be stretched by this factor to land on a target instead of leaving a small remainder (ADR 0050 D3, revised).
pub const LAND_STRETCH: f64 = 1.05;

/// Replace the velocity and the face flux of `s` by the linear extrapolation `x^n + (x^n - x^{n-1}) dt/dt_old` of their two
/// previous time levels (the time-level history is untouched), as the initial iterate of the step `dt` (ADR 0050 D5). Nothing
/// happens before two levels exist.
pub fn extrapolate_initial(s: &mut PimpleSolver, dt: f64) {
    let st = s.snapshot();
    let (Some(dt_old), Some(_)) = (st.hist_u[0].dt_old, st.hist_u[0].older.as_ref()) else {
        return;
    };
    let r = dt / dt_old;
    let comp = |k: usize| {
        let h = &st.hist_u[k];
        let o = h.older.as_ref().expect("older level");
        (0..h.old.len())
            .map(|c| h.old[c] + (h.old[c] - o[c]) * r)
            .collect::<Vec<f64>>()
    };
    let (ux, uy, uz) = (comp(0), comp(1), comp(2));
    for (c, v) in s.u.values.iter_mut().enumerate() {
        *v = [ux[c], uy[c], uz[c]] as Vec3;
    }
    if let Some(po) = st.phi_hist.older.as_ref() {
        for f in 0..s.phi.len() {
            s.phi[f] = st.phi_hist.old[f] + (st.phi_hist.old[f] - po[f]) * r;
        }
    }
}

/// Driver of a [`PimpleSolver`] with a [`StepController`].
#[derive(Debug, Clone)]
pub struct ControlledRun {
    /// Controller (state included).
    pub ctl: StepController,
    /// Extrapolate the initial iterate of every step (see [`extrapolate_initial`]).
    pub extrapolate: bool,
    /// Times at which a step landed on a target (write time or end time), as snapped.
    pub landed: Vec<f64>,
}

impl ControlledRun {
    /// New driver.
    pub fn new(ctl: StepController, extrapolate: bool) -> Self {
        ControlledRun {
            ctl,
            extrapolate,
            landed: Vec::new(),
        }
    }

    /// Advance one accepted step towards `t_end`.
    ///
    /// # Panics
    /// Error control needs `TimeScheme::ImplicitEuler` (first-order step doubling).
    pub fn advance(&mut self, s: &mut PimpleSolver, t_end: f64) -> StepInfo {
        let dt_prop = self.ctl.dt_next;
        let t0 = s.time;
        let (mut dt, mut target) = self.ctl.land(t0, dt_prop, t_end);
        let info;
        let next;
        if self.ctl.settings.error_tol > 0.0 {
            assert!(
                s.settings.time_scheme == TimeScheme::ImplicitEuler,
                "error control by step doubling is implemented for the implicit Euler scheme only"
            );
            let backup = s.history.clone();
            let snap = s.snapshot();
            let mut prop = dt_prop;
            loop {
                if self.extrapolate {
                    extrapolate_initial(s, dt);
                }
                s.step(dt);
                let u1: Vec<Vec3> = s.u.values.clone();
                s.restore(&snap)
                    .expect("restore of a snapshot taken from the same solver");
                if self.extrapolate {
                    extrapolate_initial(s, 0.5 * dt);
                }
                s.step(0.5 * dt);
                if self.extrapolate {
                    extrapolate_initial(s, 0.5 * dt);
                }
                let last = s.step(0.5 * dt);
                let (mut diff, mut scale) = (0.0_f64, 1e-30_f64);
                for (a, b) in s.u.values.iter().zip(&u1) {
                    for k in 0..3 {
                        diff = diff.max((a[k] - b[k]).abs());
                        scale = scale.max(a[k].abs());
                    }
                }
                let rate = diff / scale / dt;
                if rate <= self.ctl.settings.error_tol || dt <= self.ctl.settings.min_dt * (1.0 + 1e-12) {
                    next = self.ctl.propose_error(rate, prop, 1.0);
                    let mut i = last;
                    i.dt = dt;
                    s.history = backup;
                    s.history.push(i);
                    info = i;
                    break;
                }
                self.ctl.rejected += 1;
                s.restore(&snap).expect("restore");
                prop = self.ctl.propose_error(rate, prop, 1.0);
                (dt, target) = self.ctl.land(t0, prop, t_end);
            }
        } else {
            if self.extrapolate {
                extrapolate_initial(s, dt);
            }
            if dt != dt_prop {
                // ADR 0162 D2: a step landed on a target is not compared by the corrector count's dt trigger
                s.correctors.mark_landing_step();
            }
            let i = s.step(dt);
            self.finish(s, dt, target);
            return i;
        }
        if let Some(tw) = target {
            s.time = tw;
            self.landed.push(tw);
        }
        self.ctl.dt_next = next;
        self.ctl.steps += 1;
        info
    }

    /// Step to take at the current time of `s` towards `t_end`: `(dt, landing target)` (see [`StepController::land`]).
    /// For callers that run the step themselves (the case-tree driver) and then call [`finish`](Self::finish).
    pub fn plan(&self, s: &PimpleSolver, t_end: f64) -> (f64, Option<f64>) {
        self.ctl.land(s.time, self.ctl.dt_next, t_end)
    }

    /// Bookkeeping after a step `dt` planned by [`plan`](Self::plan) has been taken (not for error control): snaps the time
    /// onto the landing target and updates the proposal from the Courant statistics of the new flux.
    pub fn finish(&mut self, s: &mut PimpleSolver, dt: f64, target: Option<f64>) {
        let dt_prop = self.ctl.dt_next;
        let next = if self.ctl.settings.period > 0.0 {
            self.ctl.settings.period / self.ctl.settings.steps_per_period as f64
        } else if self.ctl.settings.co_max > 0.0 || self.ctl.settings.co_rms > 0.0 {
            let st = courant_stats(&s.geom, &s.phi, dt);
            self.ctl.propose_courant(st, dt, dt_prop)
        } else {
            dt_prop
        };
        if let Some(tw) = target {
            s.time = tw;
            self.landed.push(tw);
        }
        self.ctl.dt_next = next;
        self.ctl.steps += 1;
    }

    /// Run until `t_end` (reached exactly).
    pub fn run(&mut self, s: &mut PimpleSolver, t_end: f64) -> Vec<StepInfo> {
        let mut out = Vec::new();
        while s.time < t_end - 1e-12 * t_end.abs().max(1e-300) {
            out.push(self.advance(s, t_end));
        }
        out
    }
}

/// Aitken delta-squared dynamic relaxation of a fixed-point iteration `x <- x + omega r(x)`,
/// `omega_{k+1} = -omega_k r_{k-1}.(r_k - r_{k-1}) / |r_k - r_{k-1}|^2`, limited to `[omega_min, omega_max]`.
#[derive(Debug, Clone)]
pub struct Aitken {
    /// Current relaxation factor.
    pub omega: f64,
    /// Lower limit.
    pub omega_min: f64,
    /// Upper limit.
    pub omega_max: f64,
    prev: Option<Vec<f64>>,
}

impl Aitken {
    /// Start with `omega0`.
    pub fn new(omega0: f64, omega_min: f64, omega_max: f64) -> Self {
        Aitken {
            omega: omega0,
            omega_min,
            omega_max,
            prev: None,
        }
    }

    /// Factor to apply to the residual `r` of the current iteration (the first call returns `omega0`).
    pub fn factor(&mut self, r: &[f64]) -> f64 {
        if let Some(p) = &self.prev {
            let (mut num, mut den) = (0.0, 0.0);
            for (a, b) in r.iter().zip(p) {
                let d = a - b;
                num += b * d;
                den += d * d;
            }
            if den > 1e-300 {
                self.omega = (-self.omega * num / den).clamp(self.omega_min, self.omega_max);
            }
        }
        self.prev = Some(r.to_vec());
        self.omega
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_target_uses_integer_grids() {
        let s = StepSettings {
            write_times: vec![0.37, 2.5],
            write_interval: 1.0,
            ..Default::default()
        };
        assert_eq!(s.next_target(0.0), Some(0.37));
        assert_eq!(s.next_target(0.37), Some(1.0));
        assert_eq!(s.next_target(1.0), Some(2.0));
        assert_eq!(s.next_target(2.0), Some(2.5));
        assert_eq!(s.next_target(2.5), Some(3.0));
    }

    #[test]
    fn landing_changes_only_the_landing_step() {
        let c = StepController::new(StepSettings::default(), 0.1);
        let (d, t) = c.land(0.0, 0.1, 0.25);
        assert!(t.is_none() && d == 0.1);
        let (d, t) = c.land(0.2, 0.1, 0.25);
        assert_eq!(t, Some(0.25));
        assert!((d - 0.05).abs() < 1e-15);
        let (d, t) = c.land(0.0, 0.3, 0.25);
        assert_eq!(t, Some(0.25));
        assert!((d - 0.25).abs() < 1e-15);
    }

    #[test]
    fn validation_names_the_field() {
        let s = StepSettings {
            period: 1.0,
            ..Default::default()
        };
        assert_eq!(s.validate().unwrap_err().0, "steps_per_period");
        assert!(StepSettings::default().validate().is_ok());
        assert!(!StepSettings::default().is_active());
    }

    #[test]
    fn state_round_trips() {
        let mut c = StepController::new(StepSettings::default(), 0.1);
        c.dt_next = 0.123456789;
        c.steps = 7;
        let v = c.to_f64s();
        let mut d = StepController::new(StepSettings::default(), 1.0);
        d.restore_f64s(&v).unwrap();
        assert_eq!(d.dt_next.to_bits(), c.dt_next.to_bits());
        assert_eq!(d.steps, 7);
        assert!(d.restore_f64s(&[1.0]).is_err());
    }
}
