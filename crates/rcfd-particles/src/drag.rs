//! Drag laws and the single-particle integrators (ADR 0028 D2, D3).
//!
//! `dv/dt = f(Re) (u - v) / tau_p + (1 - rho_f/rho_p) g`, `dx/dt = v`, `tau_p = rho_p d^2 / (18 mu)`,
//! `Re = rho_f d |u - v| / mu`, `f = C_D Re / 24`.

use rcfd_core::vec3::{add, dot, norm, scale, sub, Vec3};

/// Drag law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragLaw {
    /// `C_D = 24/Re`.
    Stokes,
    /// `C_D = 24/Re (1 + 0.15 Re^0.687)` for `Re <= 1000`, `0.44` above.
    SchillerNaumann,
    /// No drag (ballistic).
    None,
}

/// Integration scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// Exponential relaxation with the fluid velocity and drag factor frozen over the step.
    Analytic,
    /// Forward Euler.
    Euler,
    /// Midpoint rule.
    Rk2,
    /// Classical fourth-order Runge-Kutta.
    Rk4,
}

/// Constant fluid properties seen by the particles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fluid {
    /// Density [kg/m^3].
    pub rho: f64,
    /// Dynamic viscosity [Pa s].
    pub mu: f64,
}

/// Particle relaxation time `rho_p d^2 / (18 mu)`.
pub fn tau_p(d: f64, rho_p: f64, mu: f64) -> f64 {
    rho_p * d * d / (18.0 * mu)
}

/// Drag factor `f(Re) = C_D Re / 24` (1 for Stokes).
pub fn drag_factor(law: DragLaw, re: f64) -> f64 {
    match law {
        DragLaw::None => 0.0,
        DragLaw::Stokes => 1.0,
        DragLaw::SchillerNaumann => {
            if re <= 1000.0 {
                1.0 + 0.15 * re.powf(0.687)
            } else {
                0.44 * re / 24.0
            }
        }
    }
}

fn reynolds(fl: Fluid, d: f64, u: Vec3, v: Vec3) -> f64 {
    fl.rho * d * norm(sub(u, v)) / fl.mu
}

/// Acceleration of a particle.
pub fn accel(law: DragLaw, fl: Fluid, d: f64, rho_p: f64, g: Vec3, u: Vec3, v: Vec3) -> Vec3 {
    let f = drag_factor(law, reynolds(fl, d, u, v));
    let geff = scale(g, 1.0 - fl.rho / rho_p);
    add(scale(sub(u, v), f / tau_p(d, rho_p, fl.mu)), geff)
}

/// One step of size `h` of a particle at `x` with velocity `v`. `u_at(x)` returns the fluid velocity at a position (it is
/// called with the start position only by the analytic scheme). Returns the new position and velocity.
pub fn step_particle(
    scheme: Scheme,
    law: DragLaw,
    fl: Fluid,
    d: f64,
    rho_p: f64,
    g: Vec3,
    x: Vec3,
    v: Vec3,
    h: f64,
    u_at: &mut dyn FnMut(Vec3) -> Vec3,
) -> (Vec3, Vec3) {
    let geff = scale(g, 1.0 - fl.rho / rho_p);
    match scheme {
        Scheme::Analytic => {
            let u = u_at(x);
            let f = drag_factor(law, reynolds(fl, d, u, v));
            if f == 0.0 {
                let xn = add(add(x, scale(v, h)), scale(geff, 0.5 * h * h));
                return (xn, add(v, scale(geff, h)));
            }
            let tau = tau_p(d, rho_p, fl.mu) / f;
            let w = add(u, scale(geff, tau));
            let e = (-h / tau).exp();
            let dv = sub(v, w);
            let vn = add(w, scale(dv, e));
            // -expm1 keeps the factor accurate for h << tau.
            let xn = add(add(x, scale(w, h)), scale(dv, tau * -(-h / tau).exp_m1()));
            (xn, vn)
        }
        Scheme::Euler => {
            let a = accel(law, fl, d, rho_p, g, u_at(x), v);
            (add(x, scale(v, h)), add(v, scale(a, h)))
        }
        Scheme::Rk2 => {
            let a1 = accel(law, fl, d, rho_p, g, u_at(x), v);
            let xm = add(x, scale(v, 0.5 * h));
            let vm = add(v, scale(a1, 0.5 * h));
            let a2 = accel(law, fl, d, rho_p, g, u_at(xm), vm);
            (add(x, scale(vm, h)), add(v, scale(a2, h)))
        }
        Scheme::Rk4 => {
            let k1v = accel(law, fl, d, rho_p, g, u_at(x), v);
            let k1x = v;
            let x2 = add(x, scale(k1x, 0.5 * h));
            let v2 = add(v, scale(k1v, 0.5 * h));
            let k2v = accel(law, fl, d, rho_p, g, u_at(x2), v2);
            let k2x = v2;
            let x3 = add(x, scale(k2x, 0.5 * h));
            let v3 = add(v, scale(k2v, 0.5 * h));
            let k3v = accel(law, fl, d, rho_p, g, u_at(x3), v3);
            let k3x = v3;
            let x4 = add(x, scale(k3x, h));
            let v4 = add(v, scale(k3v, h));
            let k4v = accel(law, fl, d, rho_p, g, u_at(x4), v4);
            let k4x = v4;
            let sx = add(add(k1x, scale(k2x, 2.0)), add(scale(k3x, 2.0), k4x));
            let sv = add(add(k1v, scale(k2v, 2.0)), add(scale(k3v, 2.0), k4v));
            (add(x, scale(sx, h / 6.0)), add(v, scale(sv, h / 6.0)))
        }
    }
}

/// Specular reflection with restitution `e`: the normal component is multiplied by `-e`, the tangential one is kept.
pub fn reflect(v: Vec3, n: Vec3, e: f64) -> Vec3 {
    sub(v, scale(n, (1.0 + e) * dot(v, n)))
}

/// Terminal velocity magnitude of the Stokes law in quiescent fluid, `d^2 (rho_p - rho_f) g / (18 mu)`.
pub fn stokes_terminal_speed(d: f64, rho_p: f64, fl: Fluid, g: f64) -> f64 {
    d * d * (rho_p - fl.rho) * g / (18.0 * fl.mu)
}
