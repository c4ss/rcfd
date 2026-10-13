//! Riemann solvers for the compressible Euler equations of a perfect gas:
//! approximate numerical fluxes (Rusanov, Roe with Harten entropy fix, HLLC,
//! AUSM+) and the exact solver for the 1D Riemann problem.
//!
//! References: Toro, *Riemann Solvers and Numerical Methods for Fluid
//! Dynamics*, 3rd ed. (2009), ch. 4 (exact), ch. 10 (HLLC), ch. 11 (Roe);
//! Roe, J. Comput. Phys. 43 (1981); Harten, J. Comput. Phys. 49 (1983);
//! Liou, J. Comput. Phys. 129 (1996) (AUSM+).

use rcfd_core::vec3::{self, Vec3};

/// Conserved variables `(rho, rho u, rho v, rho w, rho E)`.
pub type Cons = [f64; 5];

/// Perfect gas with constant ratio of specific heats.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gas {
    /// Ratio of specific heats.
    pub gamma: f64,
}

impl Gas {
    /// Air-like diatomic gas, `gamma = 1.4`.
    pub const AIR: Gas = Gas { gamma: 1.4 };
}

/// Primitive state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prim {
    /// Density.
    pub rho: f64,
    /// Velocity.
    pub u: Vec3,
    /// Pressure.
    pub p: f64,
}

impl Prim {
    /// Conserved variables.
    pub fn to_cons(&self, g: Gas) -> Cons {
        let ke = 0.5 * self.rho * vec3::dot(self.u, self.u);
        [
            self.rho,
            self.rho * self.u[0],
            self.rho * self.u[1],
            self.rho * self.u[2],
            self.p / (g.gamma - 1.0) + ke,
        ]
    }

    /// Speed of sound.
    pub fn sound_speed(&self, g: Gas) -> f64 {
        (g.gamma * self.p / self.rho).sqrt()
    }

    /// Total specific enthalpy `H = (E + p)/rho`.
    pub fn total_enthalpy(&self, g: Gas) -> f64 {
        g.gamma / (g.gamma - 1.0) * self.p / self.rho + 0.5 * vec3::dot(self.u, self.u)
    }

    /// Physical flux through a unit normal `n`.
    pub fn flux(&self, g: Gas, n: Vec3) -> Cons {
        let un = vec3::dot(self.u, n);
        let e = self.p / (g.gamma - 1.0) + 0.5 * self.rho * vec3::dot(self.u, self.u);
        [
            self.rho * un,
            self.rho * un * self.u[0] + self.p * n[0],
            self.rho * un * self.u[1] + self.p * n[1],
            self.rho * un * self.u[2] + self.p * n[2],
            (e + self.p) * un,
        ]
    }

    /// Mirror the state in a wall with unit normal `n` (slip-wall ghost state).
    pub fn mirrored(&self, n: Vec3) -> Prim {
        let un = vec3::dot(self.u, n);
        Prim {
            rho: self.rho,
            u: vec3::sub(self.u, vec3::scale(n, 2.0 * un)),
            p: self.p,
        }
    }
}

/// Primitive state from conserved variables.
pub fn to_prim(g: Gas, c: &Cons) -> Prim {
    let rho = c[0];
    let u = [c[1] / rho, c[2] / rho, c[3] / rho];
    let p = (g.gamma - 1.0) * (c[4] - 0.5 * rho * vec3::dot(u, u));
    Prim { rho, u, p }
}

/// Numerical flux function choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiemannSolver {
    /// Local Lax-Friedrichs (Rusanov): robust and very diffusive.
    Rusanov,
    /// Roe's linearised solver with Harten's entropy fix.
    Roe,
    /// Harten-Lax-van Leer-Contact solver (Toro et al. 1994).
    Hllc,
    /// AUSM+ (Liou 1996).
    AusmPlus,
}

impl RiemannSolver {
    /// Parse a solver name (`rusanov`, `roe`, `hllc`, `ausm+`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "rusanov" | "llf" => Some(Self::Rusanov),
            "roe" => Some(Self::Roe),
            "hllc" => Some(Self::Hllc),
            "ausm+" | "ausmplus" | "ausm" => Some(Self::AusmPlus),
            _ => None,
        }
    }

    /// Numerical flux through a face with unit normal `n` pointing from the
    /// left state to the right state (per unit area).
    pub fn flux(self, g: Gas, l: &Prim, r: &Prim, n: Vec3) -> Cons {
        match self {
            Self::Rusanov => rusanov(g, l, r, n),
            Self::Roe => roe(g, l, r, n),
            Self::Hllc => hllc(g, l, r, n),
            Self::AusmPlus => ausm_plus(g, l, r, n),
        }
    }
}

fn rusanov(g: Gas, l: &Prim, r: &Prim, n: Vec3) -> Cons {
    let (fl, fr) = (l.flux(g, n), r.flux(g, n));
    let (ul, ur) = (l.to_cons(g), r.to_cons(g));
    let s = (vec3::dot(l.u, n).abs() + l.sound_speed(g)).max(vec3::dot(r.u, n).abs() + r.sound_speed(g));
    std::array::from_fn(|k| 0.5 * (fl[k] + fr[k]) - 0.5 * s * (ur[k] - ul[k]))
}

fn roe(g: Gas, l: &Prim, r: &Prim, n: Vec3) -> Cons {
    let (fl, fr) = (l.flux(g, n), r.flux(g, n));
    let (sl, sr) = (l.rho.sqrt(), r.rho.sqrt());
    let w = 1.0 / (sl + sr);
    let rho = sl * sr;
    let u = vec3::scale(vec3::add(vec3::scale(l.u, sl), vec3::scale(r.u, sr)), w);
    let h = (sl * l.total_enthalpy(g) + sr * r.total_enthalpy(g)) * w;
    let q2 = vec3::dot(u, u);
    let c = ((g.gamma - 1.0) * (h - 0.5 * q2)).max(1e-30).sqrt();
    let un = vec3::dot(u, n);
    let (drho, dp) = (r.rho - l.rho, r.p - l.p);
    let du = vec3::sub(r.u, l.u);
    let dun = vec3::dot(du, n);
    let dut = vec3::sub(du, vec3::scale(n, dun));
    let c2 = c * c;
    let a1 = (dp - rho * c * dun) / (2.0 * c2);
    let a5 = (dp + rho * c * dun) / (2.0 * c2);
    let a_rho = drho - dp / c2;
    // Harten entropy fix on the acoustic waves.
    let delta = 0.1 * c;
    let fix = |lam: f64| {
        let a = lam.abs();
        if a < delta {
            0.5 * (a * a / delta + delta)
        } else {
            a
        }
    };
    let (l1, l5) = (fix(un - c), fix(un + c));
    let l2 = un.abs();
    let mut d = [0.0; 5];
    // acoustic waves
    let r1 = [1.0, u[0] - c * n[0], u[1] - c * n[1], u[2] - c * n[2], h - c * un];
    let r5 = [1.0, u[0] + c * n[0], u[1] + c * n[1], u[2] + c * n[2], h + c * un];
    for k in 0..5 {
        d[k] += l1 * a1 * r1[k] + l5 * a5 * r5[k];
    }
    // entropy wave
    let re = [1.0, u[0], u[1], u[2], 0.5 * q2];
    for k in 0..5 {
        d[k] += l2 * a_rho * re[k];
    }
    // shear waves
    let rs = [0.0, dut[0], dut[1], dut[2], vec3::dot(u, dut)];
    for k in 0..5 {
        d[k] += l2 * rho * rs[k];
    }
    std::array::from_fn(|k| 0.5 * (fl[k] + fr[k]) - 0.5 * d[k])
}

fn hllc(g: Gas, l: &Prim, r: &Prim, n: Vec3) -> Cons {
    let (unl, unr) = (vec3::dot(l.u, n), vec3::dot(r.u, n));
    let (cl, cr) = (l.sound_speed(g), r.sound_speed(g));
    // Roe-average based wave speed estimates (Einfeldt / Toro eq. 10.52-10.54).
    let (sl_, sr_) = (l.rho.sqrt(), r.rho.sqrt());
    let w = 1.0 / (sl_ + sr_);
    let un_roe = (sl_ * unl + sr_ * unr) * w;
    let h_roe = (sl_ * l.total_enthalpy(g) + sr_ * r.total_enthalpy(g)) * w;
    let u_roe = vec3::scale(vec3::add(vec3::scale(l.u, sl_), vec3::scale(r.u, sr_)), w);
    let c_roe = ((g.gamma - 1.0) * (h_roe - 0.5 * vec3::dot(u_roe, u_roe)))
        .max(1e-30)
        .sqrt();
    let s_l = (unl - cl).min(un_roe - c_roe);
    let s_r = (unr + cr).max(un_roe + c_roe);
    let s_star = (r.p - l.p + l.rho * unl * (s_l - unl) - r.rho * unr * (s_r - unr))
        / (l.rho * (s_l - unl) - r.rho * (s_r - unr));
    let fl = l.flux(g, n);
    let fr = r.flux(g, n);
    if s_l >= 0.0 {
        return fl;
    }
    if s_r <= 0.0 {
        return fr;
    }
    let star = |k: &Prim, un: f64, s: f64| -> Cons {
        let fac = k.rho * (s - un) / (s - s_star);
        let e_k = k.p / (g.gamma - 1.0) + 0.5 * k.rho * vec3::dot(k.u, k.u);
        let mut u = [0.0; 5];
        u[0] = fac;
        for i in 0..3 {
            u[1 + i] = fac * (k.u[i] + (s_star - un) * n[i]);
        }
        u[4] = fac * (e_k / k.rho + (s_star - un) * (s_star + k.p / (k.rho * (s - un))));
        u
    };
    if s_star >= 0.0 {
        let us = star(l, unl, s_l);
        let ul = l.to_cons(g);
        std::array::from_fn(|k| fl[k] + s_l * (us[k] - ul[k]))
    } else {
        let us = star(r, unr, s_r);
        let ur = r.to_cons(g);
        std::array::from_fn(|k| fr[k] + s_r * (us[k] - ur[k]))
    }
}

fn ausm_plus(g: Gas, l: &Prim, r: &Prim, n: Vec3) -> Cons {
    const ALPHA: f64 = 3.0 / 16.0;
    const BETA: f64 = 1.0 / 8.0;
    let (unl, unr) = (vec3::dot(l.u, n), vec3::dot(r.u, n));
    let (hl, hr) = (l.total_enthalpy(g), r.total_enthalpy(g));
    // Interface speed of sound from the critical (sonic) speeds of each side.
    let astar2 = |h: f64| 2.0 * (g.gamma - 1.0) / (g.gamma + 1.0) * h;
    let ahat = |astar2: f64, un: f64| astar2 / (astar2.sqrt().max(un));
    let (al, ar) = (ahat(astar2(hl), unl), ahat(astar2(hr), -unr));
    let a_half = al.min(ar);
    let (ml, mr) = (unl / a_half, unr / a_half);
    // M4+/-: +-1/4 (M +- 1)^2 +- beta (M^2-1)^2 for |M| < 1
    let m4b = |m: f64, s: f64| -> f64 {
        if m.abs() >= 1.0 {
            0.5 * (m + s * m.abs())
        } else {
            s * 0.25 * (m + s).powi(2) + s * BETA * (m * m - 1.0).powi(2)
        }
    };
    let p5 = |m: f64, s: f64| -> f64 {
        if m.abs() >= 1.0 {
            0.5 * (1.0 + s * m.signum())
        } else {
            0.25 * (m + s).powi(2) * (2.0 - s * m) + s * ALPHA * m * (m * m - 1.0).powi(2)
        }
    };
    let m_half = m4b(ml, 1.0) + m4b(mr, -1.0);
    let p_half = p5(ml, 1.0) * l.p + p5(mr, -1.0) * r.p;
    let mdot = a_half * m_half;
    let (rho, u, h) = if mdot >= 0.0 {
        (l.rho, l.u, hl)
    } else {
        (r.rho, r.u, hr)
    };
    let m = mdot * rho;
    [
        m,
        m * u[0] + p_half * n[0],
        m * u[1] + p_half * n[1],
        m * u[2] + p_half * n[2],
        m * h,
    ]
}

/// Star-region solution and sampler of the exact 1D Riemann problem for a
/// perfect gas (Toro 2009, ch. 4). Velocity is the component normal to the
/// initial discontinuity; the tangential velocity is advected with the contact.
#[derive(Debug, Clone, Copy)]
pub struct ExactRiemann {
    g: Gas,
    /// Left state `(rho, u, p)`.
    pub l: [f64; 3],
    /// Right state `(rho, u, p)`.
    pub r: [f64; 3],
    /// Star-region pressure.
    pub p_star: f64,
    /// Star-region (contact) velocity.
    pub u_star: f64,
}

impl ExactRiemann {
    /// Solve for the star state; `None` when vacuum is generated.
    pub fn new(g: Gas, l: [f64; 3], r: [f64; 3]) -> Option<Self> {
        let gm = g.gamma;
        let cl = (gm * l[2] / l[0]).sqrt();
        let cr = (gm * r[2] / r[0]).sqrt();
        if 2.0 * (cl + cr) / (gm - 1.0) <= r[1] - l[1] {
            return None;
        }
        let f = |p: f64, k: &[f64; 3], c: f64| -> (f64, f64) {
            if p > k[2] {
                let a = 2.0 / ((gm + 1.0) * k[0]);
                let b = (gm - 1.0) / (gm + 1.0) * k[2];
                let q = (a / (p + b)).sqrt();
                ((p - k[2]) * q, q * (1.0 - 0.5 * (p - k[2]) / (p + b)))
            } else {
                let pr = p / k[2];
                (
                    2.0 * c / (gm - 1.0) * (pr.powf((gm - 1.0) / (2.0 * gm)) - 1.0),
                    pr.powf(-(gm + 1.0) / (2.0 * gm)) / (k[0] * c),
                )
            }
        };
        // PVRS initial guess
        let mut p = (0.5 * (l[2] + r[2]) - 0.125 * (r[1] - l[1]) * (l[0] + r[0]) * (cl + cr)).max(1e-8);
        for _ in 0..100 {
            let (fl, dfl) = f(p, &l, cl);
            let (fr, dfr) = f(p, &r, cr);
            let g_ = fl + fr + (r[1] - l[1]);
            let pn = (p - g_ / (dfl + dfr)).max(1e-12);
            let change = 2.0 * (pn - p).abs() / (pn + p);
            p = pn;
            if change < 1e-14 {
                break;
            }
        }
        let (fl, _) = f(p, &l, cl);
        let (fr, _) = f(p, &r, cr);
        Some(Self {
            g,
            l,
            r,
            p_star: p,
            u_star: 0.5 * (l[1] + r[1]) + 0.5 * (fr - fl),
        })
    }

    /// Solution `(rho, u, p)` at similarity coordinate `s = (x - x0)/t`.
    pub fn sample(&self, s: f64) -> [f64; 3] {
        let gm = self.g.gamma;
        let (l, r, ps, us) = (self.l, self.r, self.p_star, self.u_star);
        let (g1, g2) = ((gm - 1.0) / (2.0 * gm), (gm + 1.0) / (2.0 * gm));
        if s <= us {
            // left of the contact
            let cl = (gm * l[2] / l[0]).sqrt();
            if ps > l[2] {
                let sl = l[1] - cl * (g2 * ps / l[2] + g1).sqrt();
                if s <= sl {
                    l
                } else {
                    let pr = ps / l[2];
                    [
                        l[0] * ((pr + (gm - 1.0) / (gm + 1.0)) / ((gm - 1.0) / (gm + 1.0) * pr + 1.0)),
                        us,
                        ps,
                    ]
                }
            } else {
                let sh = l[1] - cl;
                let cstar = cl * (ps / l[2]).powf(g1);
                let st = us - cstar;
                if s <= sh {
                    l
                } else if s >= st {
                    [l[0] * (ps / l[2]).powf(1.0 / gm), us, ps]
                } else {
                    let c = 2.0 / (gm + 1.0) * (cl + (gm - 1.0) / 2.0 * (l[1] - s));
                    [
                        l[0] * (c / cl).powf(2.0 / (gm - 1.0)),
                        2.0 / (gm + 1.0) * (cl + (gm - 1.0) / 2.0 * l[1] + s),
                        l[2] * (c / cl).powf(2.0 * gm / (gm - 1.0)),
                    ]
                }
            }
        } else {
            let cr = (gm * r[2] / r[0]).sqrt();
            if ps > r[2] {
                let sr = r[1] + cr * (g2 * ps / r[2] + g1).sqrt();
                if s >= sr {
                    r
                } else {
                    let pr = ps / r[2];
                    [
                        r[0] * ((pr + (gm - 1.0) / (gm + 1.0)) / ((gm - 1.0) / (gm + 1.0) * pr + 1.0)),
                        us,
                        ps,
                    ]
                }
            } else {
                let sh = r[1] + cr;
                let cstar = cr * (ps / r[2]).powf(g1);
                let st = us + cstar;
                if s >= sh {
                    r
                } else if s <= st {
                    [r[0] * (ps / r[2]).powf(1.0 / gm), us, ps]
                } else {
                    let c = 2.0 / (gm + 1.0) * (cr - (gm - 1.0) / 2.0 * (r[1] - s));
                    [
                        r[0] * (c / cr).powf(2.0 / (gm - 1.0)),
                        2.0 / (gm + 1.0) * (-cr + (gm - 1.0) / 2.0 * r[1] + s),
                        r[2] * (c / cr).powf(2.0 * gm / (gm - 1.0)),
                    ]
                }
            }
        }
    }
}

/// Oblique-shock relation (Anderson, *Modern Compressible Flow*, ch. 4): the
/// weak-shock wave angle `beta` (radians) for upstream Mach `m1` and flow
/// deflection `theta` (radians), or `None` if the shock detaches.
pub fn oblique_shock_beta(gamma: f64, m1: f64, theta: f64) -> Option<f64> {
    // theta-beta-M: tan(theta) = 2 cot(beta) (M^2 sin^2 beta - 1) / (M^2 (gamma + cos 2beta) + 2)
    let f = |b: f64| {
        2.0 / b.tan() * (m1 * m1 * b.sin().powi(2) - 1.0) / (m1 * m1 * (gamma + (2.0 * b).cos()) + 2.0) - theta.tan()
    };
    let mu = (1.0 / m1).asin();
    // maximum-deflection wave angle by scanning
    let n = 20000;
    let (mut best_b, mut best_f) = (mu, f64::MIN);
    for i in 1..n {
        let b = mu + (std::f64::consts::FRAC_PI_2 - mu) * i as f64 / n as f64;
        let v = f(b) + theta.tan();
        if v > best_f {
            best_f = v;
            best_b = b;
        }
    }
    if best_f < theta.tan() {
        return None;
    }
    let (mut a, mut b) = (mu, best_b);
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if f(a) * f(m) <= 0.0 {
            b = m;
        } else {
            a = m;
        }
    }
    Some(0.5 * (a + b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Gas = Gas::AIR;

    fn state(rho: f64, u: Vec3, p: f64) -> Prim {
        Prim { rho, u, p }
    }

    #[test]
    fn consistency_all_solvers() {
        // F(U, U) must equal the physical flux for every solver.
        let s = state(1.3, [0.4, -0.2, 0.1], 2.1);
        let n = vec3::unit([0.3, 0.8, -0.2]);
        let f = s.flux(G, n);
        for sol in [
            RiemannSolver::Rusanov,
            RiemannSolver::Roe,
            RiemannSolver::Hllc,
            RiemannSolver::AusmPlus,
        ] {
            let num = sol.flux(G, &s, &s, n);
            for k in 0..5 {
                assert!(
                    (num[k] - f[k]).abs() < 1e-12,
                    "{sol:?} component {k}: {} vs {}",
                    num[k],
                    f[k]
                );
            }
        }
    }

    #[test]
    fn isolated_stationary_contact_is_preserved_by_hllc_and_roe() {
        // equal p and u = 0, density jump: exact solution is a stationary contact
        let l = state(1.0, [0.0; 3], 1.0);
        let r = state(0.2, [0.0; 3], 1.0);
        for sol in [RiemannSolver::Roe, RiemannSolver::Hllc, RiemannSolver::AusmPlus] {
            let f = sol.flux(G, &l, &r, [1.0, 0.0, 0.0]);
            assert!(f[0].abs() < 1e-14 && f[4].abs() < 1e-14, "{sol:?} {f:?}");
            assert!((f[1] - 1.0).abs() < 1e-14);
        }
    }

    #[test]
    fn supersonic_upwinding() {
        let l = state(1.0, [3.0, 0.0, 0.0], 1.0);
        let r = state(0.5, [3.5, 0.0, 0.0], 0.8);
        let n = [1.0, 0.0, 0.0];
        let fl = l.flux(G, n);
        for sol in [RiemannSolver::Roe, RiemannSolver::Hllc, RiemannSolver::AusmPlus] {
            let f = sol.flux(G, &l, &r, n);
            for k in 0..5 {
                assert!((f[k] - fl[k]).abs() < 1e-12, "{sol:?}");
            }
        }
    }

    #[test]
    fn exact_sod_star_state() {
        // Classic Sod problem (rho, u, p) = (1, 0, 1) | (0.125, 0, 0.1): p* = 0.30313, u* = 0.92745.
        // Verified against an independent published analytic Sod solution (arXiv:2108.01709 lists
        // u* = 0.92745, p* = 0.30313, rho*L = 0.42632, rho*R = 0.26557; see docs/reference-data.md).
        // The old "Toro Table 4.3 test 1" attribution could not be confirmed (Toro's test-suite Test 1
        // is the modified Sod with uL = 0.75), so the table number is not cited. The
        // Rankine-Hugoniot check below does not depend on these constants.
        let e = ExactRiemann::new(G, [1.0, 0.0, 1.0], [0.125, 0.0, 0.1]).unwrap();
        assert!((e.p_star - 0.30313).abs() < 5e-6, "p* = {}", e.p_star);
        assert!((e.u_star - 0.92745).abs() < 5e-6, "u* = {}", e.u_star);
        // right shock: mass and momentum jump conditions across it
        let rs = e.sample(e.u_star + 1e-9);
        let rho_s = rs[0];
        let shock_speed = (e.r[0] * e.r[1] - rho_s * e.u_star) / (e.r[0] - rho_s);
        let mom_l = rho_s * e.u_star * (e.u_star - shock_speed) + e.p_star;
        let mom_r = e.r[0] * e.r[1] * (e.r[1] - shock_speed) + e.r[2];
        assert!((mom_l - mom_r).abs() < 1e-9, "RH momentum {mom_l} vs {mom_r}");
        // the sampled shock location moves at that speed
        let just_behind = e.sample(shock_speed - 1e-6);
        let just_ahead = e.sample(shock_speed + 1e-6);
        assert!((just_behind[2] - e.p_star).abs() < 1e-12 && (just_ahead[2] - e.r[2]).abs() < 1e-12);
    }

    #[test]
    fn exact_detects_vacuum() {
        assert!(ExactRiemann::new(G, [1.0, -20.0, 0.4], [1.0, 20.0, 0.4]).is_none());
    }

    #[test]
    fn oblique_shock_matches_tables() {
        // M = 2, theta = 10 deg, gamma = 1.4 -> beta = 39.31 deg (weak branch). Verified by
        // independently solving the theta-beta-M relation (Anderson ch. 4; same form on Wikipedia
        // "Oblique shock"): 39.314 deg, strong 83.70 deg, max deflection 22.97 deg; an online
        // calculator quotes 39.3 deg. See docs/reference-data.md.
        let b = oblique_shock_beta(1.4, 2.0, 10f64.to_radians()).unwrap().to_degrees();
        assert!((b - 39.31).abs() < 0.02, "beta = {b}");
        assert!(oblique_shock_beta(1.4, 2.0, 30f64.to_radians()).is_none());
    }
}
