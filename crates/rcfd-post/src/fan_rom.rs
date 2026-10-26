//! Reduced-order axial-fan performance (wave 46B, ADR 0119): blade-element strip theory with a stated low-Reynolds section
//! model and loss terms, the fan curve, the operating point against a system curve, fan laws, and the steady-loading
//! (Gutin-type) tonal harmonics of the strip loads. No CFD. It is a stated proxy, not a validated prediction.
//!
//! Model (ADR 0119, derivations there):
//! * Strips: `n_strips` equal radial strips between `r_hub` and `r_tip`, midpoint rule; each strip is an independent 2-D cascade
//!   (no radial velocity, no radial equilibrium coupling). Axial velocity `c_x = Q / (pi (r_tip^2 - r_hub^2))`, uniform; no inlet
//!   swirl. Vector-mean relative velocity `w_inf = U - dc/2`, `beta_inf = atan(w_inf / c_x)` from the axis, effective incidence
//!   `alpha_e = beta_inf - stagger - alpha_L0`.
//! * Cascade momentum: `F_theta = rho s c_x dc` per unit span, `F_theta = L cos b + D sin b`, `F_x = L sin b - D cos b`;
//!   total pressure rise `dp0 = F_x / s + rho dc^2 / 2` (axial momentum), shaft power from `F_theta U`.
//! * Section (thin cambered plate, LABELLED): attached `C_L = 2 pi alpha_e` (thin-airfoil theory, first principles) while
//!   `|C_L| <= cl_max`; post-stall flat plate `C_N = cd_normal sin alpha_e`, `C_L = C_N cos alpha_e`, `C_D = C_N sin alpha_e`
//!   (ASSUMED, UNVERIFIED). Thickness is not modelled. Drag: laminar Blasius friction on both sides `2.656 / sqrt(Re_c)` (first
//!   principles, a lower bound at chord Re 2e4-2e5), Howell-type annulus drag `annulus_drag s / h` and secondary drag
//!   `secondary_drag C_L^2` (UNVERIFIED recollections).
//! * Tip gap: pressure-driven leakage through the clearance, `P_tip = B C_d tau c dp_tip sqrt(2 dp_tip / rho)` with
//!   `dp_tip = rho W^2 |C_L| / 2` of the outermost strip, all of the jet head dissipated; `C_d = pi / (pi + 2)` (2-D free-streamline
//!   slot contraction, UNVERIFIED recollection).
//! * Static pressure (total-to-static, free discharge, no stator): the exit axial dynamic pressure `rho c_x^2 / 2` and the swirl
//!   kinetic energy are lost.
//!
//! NOT modelled: cascade (solidity) effects on lift, deviation, 3-D blade effects, radial redistribution, laminar separation
//! bubbles, inflow distortion, struts, unsteady loading, thickness noise, compressibility.

use crate::rotor::{Rotor, RotorError};
use rcfd_fv::lumped::{bisect, network_pressure};
use rcfd_mesh::fan::{camber_line, gauss_legendre, AxialFan, SectionFamily};
use std::f64::consts::{PI, TAU};

/// One radial section of the reduced-order blade (a subset of [`rcfd_mesh::fan::Station`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RomSection {
    /// Radius [m].
    pub radius: f64,
    /// Chord [m].
    pub chord: f64,
    /// Stagger from the axis [rad] (the `rcfd_mesh::fan` convention).
    pub stagger: f64,
    /// Maximum camber / chord.
    pub camber: f64,
    /// Chordwise position of maximum camber (NACA 4-digit only).
    pub camber_pos: f64,
}

/// Reduced-order blade row. Sections are interpolated linearly in `r` and clamped outside their range.
#[derive(Debug, Clone, PartialEq)]
pub struct RomFan {
    /// Blade count.
    pub n_blades: usize,
    /// Hub radius [m].
    pub r_hub: f64,
    /// Tip radius [m].
    pub r_tip: f64,
    /// Radial tip clearance [m].
    pub tip_gap: f64,
    /// Camber-line family.
    pub family: SectionFamily,
    /// Sections, strictly increasing radius.
    pub sections: Vec<RomSection>,
}

impl RomFan {
    /// From the parametric fan of `rcfd_mesh::fan` (reads fields only; the fan's own validation applies).
    pub fn from_axial_fan(f: &AxialFan) -> Result<RomFan, String> {
        if let Some(e) = f.validate().first() {
            return Err(e.to_string());
        }
        Ok(RomFan {
            n_blades: f.n_blades,
            r_hub: f.r_hub,
            r_tip: f.r_tip,
            tip_gap: f.tip_clearance,
            family: f.family,
            sections: f
                .stations
                .iter()
                .map(|s| RomSection {
                    radius: s.radius,
                    chord: s.chord,
                    stagger: s.stagger,
                    camber: s.camber,
                    camber_pos: s.camber_pos,
                })
                .collect(),
        })
    }

    /// Located validation.
    pub fn validate(&self) -> Result<(), String> {
        if self.n_blades == 0 {
            return Err("n_blades: must be >= 1".into());
        }
        if !(self.r_hub.is_finite() && self.r_hub > 0.0 && self.r_tip.is_finite() && self.r_tip > self.r_hub) {
            return Err(format!(
                "r_hub, r_tip: need 0 < r_hub < r_tip (got {}, {})",
                self.r_hub, self.r_tip
            ));
        }
        if !(self.tip_gap.is_finite() && self.tip_gap >= 0.0) {
            return Err(format!("tip_gap: must be finite and >= 0 (got {})", self.tip_gap));
        }
        if self.sections.is_empty() {
            return Err("sections: at least one section is needed".into());
        }
        for (i, s) in self.sections.iter().enumerate() {
            if !s.radius.is_finite() || (i > 0 && s.radius <= self.sections[i - 1].radius) {
                return Err(format!("sections[{i}].radius: must be finite and strictly increasing"));
            }
            if !(s.chord.is_finite() && s.chord > 0.0) {
                return Err(format!("sections[{i}].chord: must be positive (got {})", s.chord));
            }
            if !(s.stagger.is_finite() && s.stagger.abs() < 0.5 * PI) {
                return Err(format!("sections[{i}].stagger: must be finite with |stagger| < 90 deg"));
            }
            if !(s.camber.is_finite() && s.camber.abs() <= 0.3) {
                return Err(format!("sections[{i}].camber: must lie in [-0.3, 0.3]"));
            }
            if self.family == SectionFamily::Naca4 && s.camber != 0.0 && !(s.camber_pos > 0.05 && s.camber_pos < 0.95) {
                return Err(format!("sections[{i}].camber_pos: must lie in (0.05, 0.95)"));
            }
        }
        Ok(())
    }

    fn section_at(&self, r: f64) -> RomSection {
        let st = &self.sections;
        let n = st.len();
        if r <= st[0].radius {
            return RomSection { radius: r, ..st[0] };
        }
        if r >= st[n - 1].radius {
            return RomSection { radius: r, ..st[n - 1] };
        }
        let i = st.partition_point(|s| s.radius <= r) - 1;
        let (a, b) = (&st[i], &st[i + 1]);
        let w = (r - a.radius) / (b.radius - a.radius);
        let l = |x: f64, y: f64| x + (y - x) * w;
        RomSection {
            radius: r,
            chord: l(a.chord, b.chord),
            stagger: l(a.stagger, b.stagger),
            camber: l(a.camber, b.camber),
            camber_pos: l(a.camber_pos, b.camber_pos),
        }
    }
}

/// Thin-airfoil zero-lift angle `-(1/pi) int_0^pi yc'(x) (cos t - 1) dt`, `x = (1 - cos t)/2` (rad), by 24-point Gauss-Legendre
/// on the two smooth pieces (split at the NACA camber position, at mid chord for the circular arc).
pub fn alpha_l0(family: SectionFamily, camber: f64, camber_pos: f64) -> f64 {
    if camber == 0.0 {
        return 0.0;
    }
    let split = match family {
        SectionFamily::Naca4 => (1.0 - 2.0 * camber_pos).acos(),
        SectionFamily::CircularArc => 0.5 * PI,
    };
    let (gx, gw) = gauss_legendre(24);
    let mut s = 0.0;
    for (a, b) in [(0.0, split), (split, PI)] {
        for (x, w) in gx.iter().zip(&gw) {
            let t = 0.5 * (a + b) + 0.5 * (b - a) * x;
            let (_, d) = camber_line(family, camber, camber_pos, 0.5 * (1.0 - t.cos()));
            s += w * 0.5 * (b - a) * d * (t.cos() - 1.0);
        }
    }
    -s / PI
}

/// Section and loss model. [`RomModel::default`] holds the registered values of ADR 0119.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RomModel {
    /// Radial strips.
    pub n_strips: usize,
    /// Attached-flow lift limit (stall when `2 pi |alpha_e|` exceeds it). ASSUMED 1.1 (as ADR 0078 EB), UNVERIFIED.
    pub cl_max: f64,
    /// Post-stall flat-plate normal-force coefficient at 90 deg. ASSUMED 2.0, UNVERIFIED.
    pub cd_normal: f64,
    /// Laminar Blasius skin friction on both sides of the section (first principles).
    pub friction: bool,
    /// Annulus (endwall) drag constant, `C_Da = annulus_drag s / h`. Howell 0.020, UNVERIFIED.
    pub annulus_drag: f64,
    /// Secondary-flow drag constant, `C_Ds = secondary_drag C_L^2`. Howell 0.018, UNVERIFIED.
    pub secondary_drag: f64,
    /// Tip-leakage discharge coefficient `C_d` (0 switches the tip loss off). `pi / (pi + 2)`, UNVERIFIED.
    pub tip_cd: f64,
}

impl Default for RomModel {
    fn default() -> Self {
        RomModel {
            n_strips: 40,
            cl_max: 1.1,
            cd_normal: 2.0,
            friction: true,
            annulus_drag: 0.020,
            secondary_drag: 0.018,
            tip_cd: PI / (PI + 2.0),
        }
    }
}

impl RomModel {
    /// Every loss switched off (inviscid strips, no tip loss); lift and stall as the default.
    pub fn lossless() -> RomModel {
        RomModel {
            friction: false,
            annulus_drag: 0.0,
            secondary_drag: 0.0,
            tip_cd: 0.0,
            ..RomModel::default()
        }
    }
}

/// Operating state of the air and the rotor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Operating {
    /// Rotational speed [rev/min].
    pub rpm: f64,
    /// Density [kg/m^3].
    pub rho: f64,
    /// Kinematic viscosity [m^2/s].
    pub nu: f64,
}

impl Operating {
    /// Angular velocity [rad/s].
    pub fn omega(&self) -> f64 {
        self.rpm * TAU / 60.0
    }
}

/// Which section-model branch a strip solution lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    /// Thin-airfoil lift, `|C_L| <= cl_max`.
    Attached,
    /// Post-stall flat plate.
    Stalled,
    /// Neither branch has a consistent root (hysteresis gap); `C_L = +-cl_max`, counted as stalled.
    Capped,
}

/// Solution of one strip. Forces are per blade per unit span, on the fluid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strip {
    /// Mid radius [m].
    pub r: f64,
    /// Strip width [m].
    pub dr: f64,
    /// Chord [m].
    pub chord: f64,
    /// Pitch `2 pi r / B` [m].
    pub pitch: f64,
    /// Blade speed [m/s].
    pub u: f64,
    /// Axial velocity [m/s].
    pub cx: f64,
    /// Swirl added `c_theta2 - c_theta1` [m/s].
    pub dc_theta: f64,
    /// Vector-mean relative flow angle from the axis [rad].
    pub beta_inf: f64,
    /// Vector-mean relative speed [m/s].
    pub w_inf: f64,
    /// Effective incidence `alpha - alpha_L0` [rad].
    pub alpha_e: f64,
    /// Lift coefficient.
    pub cl: f64,
    /// Drag coefficient (all terms).
    pub cd: f64,
    /// Chord Reynolds number `W c / nu`.
    pub re: f64,
    /// Section branch.
    pub branch: Branch,
    /// Tangential force [N/m].
    pub f_theta: f64,
    /// Axial force [N/m].
    pub f_x: f64,
    /// Total pressure rise of the strip, `F_x / s + rho dc^2 / 2` [Pa].
    pub dp0: f64,
}

/// One point of the fan curve.
#[derive(Debug, Clone, PartialEq)]
pub struct FanPoint {
    /// Volume flow [m^3/s].
    pub q: f64,
    /// Total pressure rise (mass-averaged, tip loss deducted) [Pa].
    pub dp_t: f64,
    /// Total-to-static pressure rise [Pa].
    pub dp_s: f64,
    /// Shaft power `Omega M` [W].
    pub power: f64,
    /// Torque on the fluid [N m].
    pub torque: f64,
    /// Axial force on the fluid [N].
    pub thrust: f64,
    /// Total efficiency `Q dp_t / P` (NaN when `P <= 0`).
    pub eta_t: f64,
    /// Static efficiency `Q dp_s / P` (NaN when `P <= 0`).
    pub eta_s: f64,
    /// Power dissipated by section drag (profile, annulus, secondary) [W].
    pub drag_loss: f64,
    /// Power dissipated by tip leakage [W].
    pub tip_loss: f64,
    /// Indices of stalled strips (branch not `Attached`).
    pub stalled: Vec<usize>,
    /// Strips whose chord Reynolds number lies outside the model range [2e4, 2e5].
    pub re_outside: usize,
    /// Strip solutions.
    pub strips: Vec<Strip>,
}

#[derive(Clone, Copy)]
struct StripGeom {
    r: f64,
    dr: f64,
    chord: f64,
    stagger: f64,
    a0: f64,
    pitch: f64,
}

/// A prepared reduced-order fan (strip geometry evaluated once).
#[derive(Clone)]
pub struct FanRom {
    /// Blade row.
    pub fan: RomFan,
    /// Model.
    pub model: RomModel,
    strips: Vec<StripGeom>,
}

impl FanRom {
    /// Validate and prepare.
    pub fn new(fan: RomFan, model: RomModel) -> Result<FanRom, String> {
        fan.validate()?;
        if model.n_strips == 0 {
            return Err("n_strips: must be >= 1".into());
        }
        for (n, v) in [
            ("cl_max", model.cl_max),
            ("cd_normal", model.cd_normal),
            ("annulus_drag", model.annulus_drag),
            ("secondary_drag", model.secondary_drag),
            ("tip_cd", model.tip_cd),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("{n}: must be finite and >= 0 (got {v})"));
            }
        }
        let dr = (fan.r_tip - fan.r_hub) / model.n_strips as f64;
        let strips = (0..model.n_strips)
            .map(|i| {
                let r = fan.r_hub + (i as f64 + 0.5) * dr;
                let s = fan.section_at(r);
                StripGeom {
                    r,
                    dr,
                    chord: s.chord,
                    stagger: s.stagger,
                    a0: alpha_l0(fan.family, s.camber, s.camber_pos),
                    pitch: TAU * r / fan.n_blades as f64,
                }
            })
            .collect();
        Ok(FanRom { fan, model, strips })
    }

    /// Bladed annulus area [m^2].
    pub fn area(&self) -> f64 {
        PI * (self.fan.r_tip * self.fan.r_tip - self.fan.r_hub * self.fan.r_hub)
    }

    /// Mid radii of the strips.
    pub fn strip_radii(&self) -> Vec<f64> {
        self.strips.iter().map(|g| g.r).collect()
    }

    /// Lift and drag coefficients on a branch at effective incidence `ae`, chord Re `re`, pitch `s`.
    fn coefficients(&self, branch: Branch, ae: f64, re: f64, s: f64) -> (f64, f64) {
        let m = &self.model;
        let cl = match branch {
            Branch::Attached => TAU * ae,
            Branch::Stalled => m.cd_normal * ae.sin() * ae.cos(),
            Branch::Capped => m.cl_max.copysign(ae),
        };
        let mut cd = m.annulus_drag * s / (self.fan.r_tip - self.fan.r_hub) + m.secondary_drag * cl * cl;
        if m.friction {
            cd += 2.656 / re.sqrt();
        }
        if branch == Branch::Stalled {
            cd += m.cd_normal * ae.sin() * ae.sin();
        }
        (cl, cd)
    }

    /// Strip state for a trial swirl `dc` on `branch`.
    fn strip_state(&self, g: &StripGeom, op: &Operating, cx: f64, dc: f64, branch: Branch) -> Strip {
        let u = op.omega() * g.r;
        let w = u - 0.5 * dc;
        let w_inf = cx.hypot(w);
        let b = w.atan2(cx);
        let ae = b - g.stagger - g.a0;
        let re = w_inf * g.chord / op.nu;
        let (cl, cd) = self.coefficients(branch, ae, re, g.pitch);
        let q_dyn = 0.5 * op.rho * w_inf * w_inf * g.chord;
        let (f_theta, f_x) = (
            q_dyn * (cl * b.cos() + cd * b.sin()),
            q_dyn * (cl * b.sin() - cd * b.cos()),
        );
        Strip {
            r: g.r,
            dr: g.dr,
            chord: g.chord,
            pitch: g.pitch,
            u,
            cx,
            dc_theta: dc,
            beta_inf: b,
            w_inf,
            alpha_e: ae,
            cl,
            cd,
            re,
            branch,
            f_theta,
            f_x,
            dp0: f_x / g.pitch + 0.5 * op.rho * dc * dc,
        }
    }

    /// Root of `F_theta(dc) / (rho s c_x) - dc` on one branch (decreasing in `dc`), bracketed then bisected.
    fn solve_branch(&self, g: &StripGeom, op: &Operating, cx: f64, branch: Branch) -> Result<Strip, String> {
        let f = |dc: f64| self.strip_state(g, op, cx, dc, branch).f_theta / (op.rho * g.pitch * cx) - dc;
        let u = op.omega() * g.r;
        let (mut lo, mut hi) = (-u, 2.0 * u);
        let mut k = 0;
        while !(f(lo) > 0.0 && f(hi) < 0.0) {
            k += 1;
            if k > 60 {
                return Err(format!(
                    "strip r = {:.6} m: no bracket of the swirl equation on the {branch:?} branch",
                    g.r
                ));
            }
            lo = 2.0 * lo - u;
            hi = 2.0 * hi + u;
        }
        Ok(self.strip_state(g, op, cx, bisect(f, lo, hi), branch))
    }

    /// Strip solution with the registered branch rule: the attached root if it is attached, else the post-stall root if it
    /// is stalled, else the capped root.
    fn solve_strip(&self, g: &StripGeom, op: &Operating, cx: f64) -> Result<Strip, String> {
        let a_s = self.model.cl_max / TAU;
        let s = self.solve_branch(g, op, cx, Branch::Attached)?;
        if s.alpha_e.abs() <= a_s {
            return Ok(s);
        }
        let s = self.solve_branch(g, op, cx, Branch::Stalled)?;
        if s.alpha_e.abs() > a_s {
            return Ok(s);
        }
        self.solve_branch(g, op, cx, Branch::Capped)
    }

    /// Fan performance at volume flow `q > 0`.
    pub fn point(&self, op: &Operating, q: f64) -> Result<FanPoint, String> {
        for (n, v) in [("rpm", op.rpm), ("rho", op.rho), ("nu", op.nu), ("q", q)] {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("{n}: must be finite and positive (got {v})"));
            }
        }
        let cx = q / self.area();
        let strips = self
            .strips
            .iter()
            .map(|g| self.solve_strip(g, op, cx))
            .collect::<Result<Vec<_>, _>>()?;
        let bl = self.fan.n_blades as f64;
        let (mut torque, mut thrust, mut q_dp0, mut swirl_ke, mut drag_loss) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for s in &strips {
            let dq = cx * TAU * s.r * s.dr;
            torque += bl * s.f_theta * s.r * s.dr;
            thrust += bl * s.f_x * s.dr;
            q_dp0 += s.dp0 * dq;
            swirl_ke += 0.5 * op.rho * s.dc_theta * s.dc_theta * dq;
            drag_loss += bl * 0.5 * op.rho * s.w_inf.powi(3) * s.chord * s.cd * s.dr;
        }
        let tip_loss = match strips.last() {
            Some(t) if self.model.tip_cd > 0.0 && self.fan.tip_gap > 0.0 => {
                let dp = 0.5 * op.rho * t.w_inf * t.w_inf * t.cl.abs();
                bl * self.model.tip_cd * self.fan.tip_gap * t.chord * dp * (2.0 * dp / op.rho).sqrt()
            }
            _ => 0.0,
        };
        let power = op.omega() * torque;
        let dp_t = (q_dp0 - tip_loss) / q;
        let dp_s = dp_t - 0.5 * op.rho * cx * cx - swirl_ke / q;
        let eta = |dp: f64| if power > 0.0 { q * dp / power } else { f64::NAN };
        Ok(FanPoint {
            q,
            dp_t,
            dp_s,
            power,
            torque,
            thrust,
            eta_t: eta(dp_t),
            eta_s: eta(dp_s),
            drag_loss,
            tip_loss,
            stalled: (0..strips.len())
                .filter(|&i| strips[i].branch != Branch::Attached)
                .collect(),
            re_outside: strips.iter().filter(|s| !(2e4..=2e5).contains(&s.re)).count(),
            strips,
        })
    }

    /// Fan curve at the given flows.
    pub fn curve(&self, op: &Operating, qs: &[f64]) -> Result<Vec<FanPoint>, String> {
        qs.iter().map(|&q| self.point(op, q)).collect()
    }

    /// Operating point against `system` on the total-to-static rise, scanning `n_scan` intervals of `[q_lo, q_hi]`.
    /// Located failures: no intersection, or the operating point has stalled strips or a rising static characteristic.
    pub fn operating_point(
        &self,
        op: &Operating,
        system: &SystemCurve,
        q_lo: f64,
        q_hi: f64,
        n_scan: usize,
    ) -> Result<(OperatingPoint, FanPoint), String> {
        let dp = |q: f64| self.point(op, q).map(|p| p.dp_s);
        let opp = operating_point(&dp, system, q_lo, q_hi, n_scan)?;
        let p = self.point(op, opp.q)?;
        if !p.stalled.is_empty() {
            let rs: Vec<String> = p.stalled.iter().map(|&i| format!("{:.4}", p.strips[i].r)).collect();
            return Err(format!(
                "operating point q = {:.6e} m^3/s is stalled: {} of {} strips beyond cl_max (r = {} m)",
                opp.q,
                p.stalled.len(),
                p.strips.len(),
                rs.join(", ")
            ));
        }
        Ok((opp, p))
    }

    /// Steady-loading tonal harmonic `m` at an observer (distance `r_obs`, polar angle `theta` from the axis toward the
    /// thrust on the fluid), from the strip loads of `point` (see [`tonal_harmonic`]).
    pub fn tone(
        &self,
        op: &Operating,
        point: &FanPoint,
        c0: f64,
        m: usize,
        r_obs: f64,
        theta: f64,
    ) -> Result<f64, String> {
        let bl = self.fan.n_blades as f64;
        let loads: Vec<(f64, f64, f64)> = point
            .strips
            .iter()
            .map(|s| (s.r, bl * s.f_x * s.dr, bl * s.f_theta * s.r * s.dr))
            .collect();
        tonal_harmonic(self.fan.n_blades, op.omega(), c0, &loads, m, r_obs, theta).map_err(|e| e.to_string())
    }
}

/// System curve `dp = k Q^2 + l Q` (the lumped network of F-113 with zero reference pressure).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemCurve {
    /// Quadratic coefficient [Pa s^2 / m^6].
    pub k: f64,
    /// Linear coefficient [Pa s / m^3].
    pub l: f64,
}

impl SystemCurve {
    /// Pressure drop at flow `q >= 0`.
    pub fn dp(&self, q: f64) -> f64 {
        network_pressure(0.0, self.l, self.k, q)
    }
}

/// Intersection of a fan characteristic with a system curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperatingPoint {
    /// Flow [m^3/s].
    pub q: f64,
    /// Pressure rise at the intersection [Pa].
    pub dp: f64,
    /// Slope of the fan characteristic there (central difference over 1e-6 of the scan range) [Pa s / m^3].
    pub slope: f64,
    /// Number of fan-above-to-below crossings found in the scan (> 1: the point is ambiguous; the highest flow is returned).
    pub crossings: usize,
}

/// Highest-flow intersection of `fan(q)` with `system` on `[q_lo, q_hi]` (scan of `n_scan` intervals, then bisection).
/// Located failures: an evaluation error, no crossing, or a rising fan characteristic at the crossing.
pub fn operating_point(
    fan: &dyn Fn(f64) -> Result<f64, String>,
    system: &SystemCurve,
    q_lo: f64,
    q_hi: f64,
    n_scan: usize,
) -> Result<OperatingPoint, String> {
    if !(q_lo.is_finite() && q_hi.is_finite() && 0.0 < q_lo && q_lo < q_hi) || n_scan == 0 {
        return Err(format!(
            "scan: need 0 < q_lo < q_hi and n_scan >= 1 (got {q_lo}, {q_hi}, {n_scan})"
        ));
    }
    if !(system.k.is_finite() && system.l.is_finite()) {
        return Err("system: k and l must be finite".into());
    }
    let h = (q_hi - q_lo) / n_scan as f64;
    let mut f = Vec::with_capacity(n_scan + 1);
    for j in 0..=n_scan {
        let q = q_lo + j as f64 * h;
        f.push(fan(q).map_err(|e| format!("fan at q = {q:.6e}: {e}"))? - system.dp(q));
    }
    let crossings: Vec<usize> = (0..n_scan).filter(|&j| f[j] > 0.0 && f[j + 1] <= 0.0).collect();
    let Some(&j) = crossings.last() else {
        let (jmax, fmax) = f
            .iter()
            .enumerate()
            .fold((0, f64::NEG_INFINITY), |a, (i, &v)| if v > a.1 { (i, v) } else { a });
        return Err(format!(
            "no intersection of the fan and system curves in q = [{q_lo:.6e}, {q_hi:.6e}] m^3/s: fan minus system is {:.6e} Pa at \
             q_lo, {:.6e} Pa at q_hi, largest {fmax:.6e} Pa at q = {:.6e}",
            f[0],
            f[n_scan],
            q_lo + jmax as f64 * h
        ));
    };
    let g = |q: f64| fan(q).map(|p| p - system.dp(q)).unwrap_or(f64::NAN);
    let q = bisect(g, q_lo + j as f64 * h, q_lo + (j + 1) as f64 * h);
    let dp = fan(q)?;
    let e = 1e-6 * (q_hi - q_lo);
    let slope = (fan(q + e)? - fan(q - e)?) / (2.0 * e);
    if slope > 0.0 {
        return Err(format!(
            "operating point q = {q:.6e} m^3/s lies on a rising fan characteristic (slope {slope:.6e} Pa s/m^3): stall region"
        ));
    }
    Ok(OperatingPoint {
        q,
        dp,
        slope,
        crossings: crossings.len(),
    })
}

/// Fan laws (geometric similarity, equal Reynolds number): `(q, dp, power)` at speed ratio `n_ratio` and density ratio
/// `rho_ratio`: `q n`, `dp rho n^2`, `power rho n^3`.
pub fn fan_law(q: f64, dp: f64, power: f64, n_ratio: f64, rho_ratio: f64) -> (f64, f64, f64) {
    (
        q * n_ratio,
        dp * rho_ratio * n_ratio * n_ratio,
        power * rho_ratio * n_ratio * n_ratio * n_ratio,
    )
}

/// Steady-loading tonal harmonic `m` (peak amplitude, Pa) of a rotor whose loading is a set of compact rings
/// `(radius, thrust, torque)` that share one blade azimuth: the signed sum of [`Rotor::harmonic_pressure`] over the rings.
pub fn tonal_harmonic(
    blades: usize,
    omega: f64,
    c0: f64,
    loads: &[(f64, f64, f64)],
    m: usize,
    r_obs: f64,
    theta: f64,
) -> Result<f64, RotorError> {
    let mut p = 0.0;
    for &(r, thrust, torque) in loads {
        p += Rotor {
            blades,
            omega,
            r_tip: r,
            thrust,
            torque,
            c0,
        }
        .harmonic_pressure(m, r_obs, theta)?;
    }
    Ok(p.abs())
}

/// Free-vortex inverse design: the stagger at every strip mid radius (returned as sections there) that makes the strip
/// solution on the attached branch give `dc_theta = k_vortex / r` at flow `q`, with the given chord / camber and the drag
/// terms of `model` (the secondary drag makes the required `C_L` the root of a quadratic). The tip-loss term does not enter.
pub fn free_vortex_design(
    base: &RomFan,
    model: &RomModel,
    op: &Operating,
    q: f64,
    k_vortex: f64,
) -> Result<RomFan, String> {
    let rom = FanRom::new(base.clone(), *model)?;
    let cx = q / rom.area();
    let mut sections = Vec::with_capacity(rom.strips.len());
    for g in &rom.strips {
        let u = op.omega() * g.r;
        let dc = k_vortex / g.r;
        let w = u - 0.5 * dc;
        let (wi, b) = (cx.hypot(w), w.atan2(cx));
        let re = wi * g.chord / op.nu;
        // F_theta / (rho W^2 c / 2) = cl cos b + (cd0 + ks cl^2) sin b = t
        let t = 2.0 * g.pitch * cx * dc / (wi * wi * g.chord);
        let (_, cd0) = rom.coefficients(Branch::Attached, 0.0, re, g.pitch); // secondary term zero at cl = 0
        let (a, rhs) = (model.secondary_drag * b.sin(), t - cd0 * b.sin());
        let disc = b.cos() * b.cos() + 4.0 * a * rhs;
        if disc < 0.0 {
            return Err(format!(
                "strip r = {:.6} m: no lift coefficient gives the target swirl",
                g.r
            ));
        }
        let cl = 2.0 * rhs / (b.cos() + disc.sqrt());
        if cl.abs() > model.cl_max {
            return Err(format!(
                "strip r = {:.6} m: needs C_L = {cl:.4} > cl_max = {}",
                g.r, model.cl_max
            ));
        }
        let s0 = base.section_at(g.r);
        sections.push(RomSection {
            stagger: b - g.a0 - cl / TAU,
            ..s0
        });
    }
    Ok(RomFan {
        sections,
        ..base.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_l0_of_the_parabolic_camber_line_is_minus_two_m() {
        let e = (alpha_l0(SectionFamily::Naca4, 0.04, 0.5) + 0.08).abs();
        assert!(e < 1e-14, "{e:e}");
    }

    #[test]
    fn operating_point_reports_no_intersection_with_its_range() {
        let e = operating_point(&|q: f64| Ok(10.0 - q), &SystemCurve { k: 100.0, l: 20.0 }, 1.0, 2.0, 10).unwrap_err();
        assert!(e.contains("no intersection") && e.contains("1.000000e0"), "{e}");
    }
}
