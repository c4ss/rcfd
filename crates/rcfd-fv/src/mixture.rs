//! Mixture model with an algebraic-slip (drift-flux) relative velocity (F-127, ADR 0056).
//!
//! Formulation (volume-averaged velocity `j`, phase 1 fraction `alpha`, relative velocity `v_12 = u_1 - u_2`):
//! `u_1 = j + (1 - alpha) v_12`, `u_2 = j - alpha v_12`; the phase-fraction flux is `alpha j + alpha (1 - alpha) v_12`; the mixture
//! mass flux is `rho_m j + J` with the drift mass flux `J = alpha (1 - alpha)(rho_1 - rho_2) v_12`; the drift (diffusion) stress is
//! `T = alpha (1 - alpha)[(1 - alpha) rho_1 + alpha rho_2] v_12 (x) v_12`.
//!
//! Closure (the form of Manninen, Taivassalo and Kallio 1996, derived in ADR 0056 B, gravity acceleration only): the dispersed
//! phase `d` in the continuous phase `c` moves relative to the continuous phase with
//! `v_dc = s sign(rho_d - rho_m) g/|g|`, `s f(Re(s)) = d_p^2 |rho_d - rho_m| |g|/(18 mu_c)`, `Re = rho_c s d_p/mu_c`, `f = 1`
//! (Stokes) or `1 + 0.15 Re^0.687` (Schiller and Naumann 1935). With `rho_d - rho_m = (1 - alpha_d)(rho_d - rho_c)` this is the
//! single-particle speed times the hindrance `(1 - alpha_d)` in the Stokes regime.

/// Particle drag law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragLaw {
    /// `C_D = 24/Re`.
    Stokes,
    /// `C_D = 24/Re (1 + 0.15 Re^0.687)`.
    SchillerNaumann,
}

impl DragLaw {
    /// The correction factor `f(Re)`.
    pub fn factor(self, re: f64) -> f64 {
        match self {
            DragLaw::Stokes => 1.0,
            DragLaw::SchillerNaumann => 1.0 + 0.15 * re.powf(0.687),
        }
    }
}

/// Solve `s f(Re(s)) = v_st` for the relative speed `s >= 0` (`Re = rho_c s d/mu_c`). `s f` is convex and increasing, so Newton
/// iteration started at `s = v_st` (where the residual is non-negative) converges monotonically.
pub fn solve_speed(law: DragLaw, d: f64, rho_c: f64, mu_c: f64, v_st: f64) -> f64 {
    if v_st <= 0.0 {
        return 0.0;
    }
    if law == DragLaw::Stokes {
        return v_st;
    }
    let k = rho_c * d / mu_c;
    let mut s = v_st;
    for _ in 0..200 {
        let re = k * s;
        let f = law.factor(re);
        let resid = s * f - v_st;
        // d(s f)/ds = 1 + 0.15 * 1.687 Re^0.687
        let dres = 1.0 + 0.15 * 1.687 * re.powf(0.687);
        let ds = resid / dres;
        s -= ds;
        if ds.abs() <= 1e-16 * s.abs() {
            break;
        }
    }
    s.max(0.0)
}

/// Mixture-model settings of the VOF host.
#[derive(Debug, Clone, Copy)]
pub struct MixtureSettings {
    /// Dispersed phase: 1 (`alpha = 1`) or 2.
    pub dispersed_phase: usize,
    /// Particle / bubble / droplet diameter, m.
    pub diameter: f64,
    /// Drag law.
    pub drag: DragLaw,
}

impl MixtureSettings {
    /// Signed relative velocity of phase 1 with respect to phase 2 along the unit vector of gravity, `v_12 = c(alpha) g/|g|`
    /// (m/s). `rho`, `mu` are the phase properties, `g_mag` is `|g|`. Exactly `0.0` when the phase densities are equal.
    pub fn slip_coefficient(&self, rho: [f64; 2], mu: [f64; 2], g_mag: f64, alpha1: f64) -> f64 {
        let a1 = alpha1.clamp(0.0, 1.0);
        let (ad, rd, rc, muc) = if self.dispersed_phase == 1 {
            (a1, rho[0], rho[1], mu[1])
        } else {
            (1.0 - a1, rho[1], rho[0], mu[0])
        };
        // rho_d - rho_m = (1 - alpha_d)(rho_d - rho_c): exact zero for equal densities
        let drho = (1.0 - ad) * (rd - rc);
        if drho == 0.0 {
            return 0.0;
        }
        let v_st = self.diameter * self.diameter * drho.abs() * g_mag / (18.0 * muc);
        let s = solve_speed(self.drag, self.diameter, rc, muc, v_st);
        let v_dc = if drho > 0.0 { s } else { -s };
        if self.dispersed_phase == 1 {
            v_dc
        } else {
            -v_dc
        }
    }

    /// Upper bound of the face signal speed used for the sub-step Courant number, `1.5 c(0)` (m/s): the single-particle speed.
    pub fn speed_bound(&self, rho: [f64; 2], mu: [f64; 2], g_mag: f64) -> f64 {
        let a0 = if self.dispersed_phase == 1 { 0.0 } else { 1.0 };
        1.5 * self.slip_coefficient(rho, mu, g_mag, a0).abs()
    }
}

/// Slip data of the phase-fraction transport: closure plus the geometric factor `(g/|g|) . S` of every internal face.
#[derive(Debug, Clone)]
pub struct SlipTransport {
    /// Closure.
    pub mixture: MixtureSettings,
    /// Phase densities.
    pub rho: [f64; 2],
    /// Phase viscosities.
    pub mu: [f64; 2],
    /// `|g|`.
    pub g_mag: f64,
    /// `(g/|g|) . S_f` per internal face (positive owner to neighbour).
    pub gs: Vec<f64>,
    /// Face signal-speed bound `1.5 c(0)`.
    pub bound: f64,
}

impl SlipTransport {
    /// Slip speed `c(alpha)`.
    pub fn c(&self, alpha1: f64) -> f64 {
        self.mixture.slip_coefficient(self.rho, self.mu, self.g_mag, alpha1)
    }

    /// Godunov flux of the scalar law with the flux function `F(alpha) = gs c(alpha) alpha (1 - alpha)` between the states
    /// `a_l` (owner) and `a_r` (neighbour): the minimum (`a_l <= a_r`) or maximum of `F` over the interval, sampled at 10 points.
    pub fn godunov(&self, gs: f64, a_l: f64, a_r: f64) -> f64 {
        let n = 9;
        let f = |a: f64| gs * self.c(a) * a * (1.0 - a);
        let lo_state = a_l <= a_r;
        let mut best = f(a_l);
        for k in 1..=n {
            let a = a_l + (a_r - a_l) * (k as f64 / n as f64);
            let v = f(a);
            if lo_state {
                best = best.min(v);
            } else {
                best = best.max(v);
            }
        }
        best
    }

    /// Central slip flux of a face at the interpolated fraction `a`.
    pub fn central(&self, gs: f64, a: f64) -> f64 {
        gs * self.c(a) * a * (1.0 - a)
    }
}
