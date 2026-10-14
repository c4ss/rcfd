//! Real-gas thermodynamics for the variable-density kernel (ADR 0007): a cubic equation of state
//! (`rcfd_core::fluids::CubicEos`, Peng-Robinson 1976 or Redlich-Kwong 1949) for the density, an ideal-gas part with
//! constant `cp0` plus the EOS departure functions for enthalpy and entropy.
//!
//! All properties are per unit mass (J/kg, J/kg/K, kg/m^3). The vapour-like (largest compressibility) root is used.

use rcfd_core::fluids::{CubicEos, CubicKind, Phase, R_UNIVERSAL};

/// Reference temperature and pressure of the ideal-gas entropy (arbitrary; only differences matter).
const T_REF: f64 = 300.0;
const P_REF: f64 = 1.0e5;

/// A real gas: cubic EOS plus a constant ideal-gas specific heat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealGas {
    /// Cubic equation of state.
    pub eos: CubicEos,
    /// Ideal-gas specific heat at constant pressure (J/kg/K).
    pub cp0: f64,
}

impl RealGas {
    /// Specific gas constant `R_u/M` (J/kg/K).
    pub fn r(&self) -> f64 {
        R_UNIVERSAL / self.eos.molar_mass
    }

    /// Compressibility factor of the vapour-like root; the ideal value 1 if the cubic has no root above `B`
    /// (not a state the kernel can reach with a sane pressure and temperature).
    pub fn z(&self, p: f64, t: f64) -> f64 {
        self.eos.z(t, p, Phase::Vapour).unwrap_or(1.0)
    }

    /// Density (kg/m^3).
    pub fn rho(&self, p: f64, t: f64) -> f64 {
        p / (self.z(p, t) * self.r() * t)
    }

    /// Molar volume `M/rho` (m^3/mol).
    fn v_molar(&self, p: f64, t: f64) -> f64 {
        self.z(p, t) * R_UNIVERSAL * t / p
    }

    /// `dp/dv|_T` and `dp/dT|_v` (molar basis) at molar volume `v`.
    fn p_derivs(&self, t: f64, v: f64) -> (f64, f64) {
        let (a, da) = self.eos.a(t);
        let b = self.eos.b();
        let rt = R_UNIVERSAL * t;
        match self.eos.kind {
            CubicKind::RedlichKwong => {
                let d = v * (v + b);
                (
                    -rt / ((v - b) * (v - b)) + a * (2.0 * v + b) / (d * d),
                    R_UNIVERSAL / (v - b) - da / d,
                )
            }
            CubicKind::PengRobinson => {
                let d = v * v + 2.0 * b * v - b * b;
                (
                    -rt / ((v - b) * (v - b)) + a * (2.0 * v + 2.0 * b) / (d * d),
                    R_UNIVERSAL / (v - b) - da / d,
                )
            }
        }
    }

    /// Compressibility `psi = d rho/d p|_T` (s^2/m^2); `1/(R T)` for the ideal gas.
    pub fn psi(&self, p: f64, t: f64) -> f64 {
        let v = self.v_molar(p, t);
        let (dpdv, _) = self.p_derivs(t, v);
        -self.eos.molar_mass / (v * v * dpdv)
    }

    /// Enthalpy departure `h - h_ideal` at the same `T` and `p` (J/kg).
    pub fn h_departure(&self, p: f64, t: f64) -> f64 {
        self.eos.residual_enthalpy(t, p, self.z(p, t))
    }

    /// Entropy departure `s - s_ideal` at the same `T` and `p` (J/kg/K), from `h_dep - T s_dep = R T ln(phi)`.
    pub fn s_departure(&self, p: f64, t: f64) -> f64 {
        let z = self.z(p, t);
        self.eos.residual_enthalpy(t, p, z) / t - self.r() * self.eos.ln_phi(t, p, z)
    }

    /// Specific enthalpy `cp0 T + h_dep` (J/kg).
    pub fn h(&self, p: f64, t: f64) -> f64 {
        self.cp0 * t + self.h_departure(p, t)
    }

    /// Specific entropy `cp0 ln(T/300) - R ln(p/1e5) + s_dep` (J/kg/K).
    pub fn s(&self, p: f64, t: f64) -> f64 {
        self.cp0 * (t / T_REF).ln() - self.r() * (p / P_REF).ln() + self.s_departure(p, t)
    }

    /// `cp = dh/dT|_p` (J/kg/K), central difference of the analytic enthalpy.
    pub fn cp(&self, p: f64, t: f64) -> f64 {
        let dt = 1e-4 * t;
        (self.h(p, t + dt) - self.h(p, t - dt)) / (2.0 * dt)
    }

    /// `cv` (J/kg/K) from `cp - cv = -T (dp/dT|_v)^2 / (dp/dv|_T)` per mass.
    pub fn cv(&self, p: f64, t: f64) -> f64 {
        let v = self.v_molar(p, t);
        let (dpdv, dpdt) = self.p_derivs(t, v);
        self.cp(p, t) + t * dpdt * dpdt / dpdv / self.eos.molar_mass
    }

    /// Speed of sound `c^2 = (cp/cv) dp/drho|_T` (m/s).
    pub fn sound_speed(&self, p: f64, t: f64) -> f64 {
        let gamma = self.cp(p, t) / self.cv(p, t);
        (gamma / self.psi(p, t)).sqrt()
    }

    /// Temperature at pressure `p` with static enthalpy `h`, Newton from `guess`.
    pub fn t_from_h(&self, h: f64, p: f64, guess: f64) -> f64 {
        let mut t = guess.max(1.0);
        for _ in 0..40 {
            let f = self.h(p, t) - h;
            let dt = f / self.cp(p, t);
            t = (t - dt).max(0.5 * t).max(1.0);
            if dt.abs() < 1e-11 * t {
                break;
            }
        }
        t
    }

    /// Temperature at pressure `p` with entropy `s`, Newton from `guess`.
    pub fn t_from_s(&self, s: f64, p: f64, guess: f64) -> f64 {
        let mut t = guess.max(1.0);
        for _ in 0..40 {
            let f = self.s(p, t) - s;
            let dt = f * t / self.cp(p, t);
            t = (t - dt).max(0.5 * t).max(1.0);
            if dt.abs() < 1e-11 * t {
                break;
            }
        }
        t
    }

    /// Static state `(p, T)` of the isentropic expansion from the stagnation state `(p0, t0)` to the kinetic energy
    /// `ke = |u|^2/2` (J/kg): `s = s0`, `h + ke = h0`. Bisection on `p` between `0.01 p0` and `p0`; a kinetic energy
    /// beyond the range returns the lowest pressure.
    pub fn static_from_stagnation(&self, p0: f64, t0: f64, ke: f64) -> (f64, f64) {
        let s0 = self.s(p0, t0);
        let h0 = self.h(p0, t0);
        let state = |p: f64| {
            let t = self.t_from_s(s0, p, t0);
            (t, self.h(p, t) + ke - h0)
        };
        let (mut lo, mut hi) = (0.01 * p0, p0);
        let (t_lo, g_lo) = state(lo);
        if g_lo > 0.0 {
            return (lo, t_lo);
        }
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if state(mid).1 < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let p = 0.5 * (lo + hi);
        (p, state(p).0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn co2(kind: CubicKind) -> RealGas {
        RealGas {
            eos: CubicEos::new(kind, 304.1282, 7.3773e6, 0.22394, 0.0440098).unwrap(),
            cp0: 940.0,
        }
    }

    #[test]
    fn analytic_derivatives_match_differences() {
        for kind in [CubicKind::PengRobinson, CubicKind::RedlichKwong] {
            let g = co2(kind);
            for &(p, t) in &[(2.0e6, 350.0), (5.0e6, 400.0), (1.0e5, 300.0)] {
                let dp = 1e-5 * p;
                let fd = (g.rho(p + dp, t) - g.rho(p - dp, t)) / (2.0 * dp);
                assert!((g.psi(p, t) / fd - 1.0).abs() < 1e-7, "psi {kind:?} {p} {t}");
                // thermodynamic identity: dh/dp|T = v - T dv/dT|p
                let dt = 1e-4 * t;
                let dvdt = (1.0 / g.rho(p, t + dt) - 1.0 / g.rho(p, t - dt)) / (2.0 * dt);
                let dhdp = (g.h(p + dp, t) - g.h(p - dp, t)) / (2.0 * dp);
                let v = 1.0 / g.rho(p, t);
                assert!(
                    (dhdp - (v - t * dvdt)).abs() < 1e-6 * v.abs(),
                    "h/p identity {kind:?} {p} {t}: {dhdp} vs {}",
                    v - t * dvdt
                );
                // Maxwell: ds/dp|T = -dv/dT|p
                let dsdp = (g.s(p + dp, t) - g.s(p - dp, t)) / (2.0 * dp);
                assert!((dsdp + dvdt).abs() < 1e-6 * dvdt.abs(), "s/p identity {kind:?} {p} {t}");
            }
        }
    }

    #[test]
    fn inversions_and_stagnation_state() {
        let g = co2(CubicKind::PengRobinson);
        let (p, t) = (3.0e6, 380.0);
        assert!((g.t_from_h(g.h(p, t), p, 300.0) / t - 1.0).abs() < 1e-9);
        assert!((g.t_from_s(g.s(p, t), p, 300.0) / t - 1.0).abs() < 1e-9);
        let (p0, t0) = (5.0e6, 400.0);
        let (ps, ts) = g.static_from_stagnation(p0, t0, 0.0);
        assert!((ps / p0 - 1.0).abs() < 1e-9 && (ts / t0 - 1.0).abs() < 1e-9);
        let ke = 0.5 * 100.0 * 100.0;
        let (ps, ts) = g.static_from_stagnation(p0, t0, ke);
        assert!((g.s(ps, ts) - g.s(p0, t0)).abs() < 1e-6);
        assert!((g.h(ps, ts) + ke - g.h(p0, t0)).abs() < 1e-5);
        assert!(ps < p0 && ts < t0);
    }
}
