//! Grid-free chemistry of one CFD cell (ADR 0027, F-054): finite-rate kinetics, the Magnussen-Hjertager eddy-dissipation
//! rate and the Eddy Dissipation Concept scales, on a constant-density cell with an optional constant-`cp` energy balance.
//!
//! State vector `[T, Y_1 .. Y_n]` (as everywhere in this crate). The cell has a constant mass density `rho`; with heat
//! release a constant heat capacity `cp` (J/kg/K) turns the chemical enthalpy release into a temperature change,
//! `dT/dt = -sum_k h_k omega_k / (rho cp)`, the enthalpy balance of an incompressible, constant-`cp` flow (not the
//! constant-volume energy equation of [`crate::reactors`], see ADR 0027 D1).

use crate::mech::{Mechanism, R_GAS};
use crate::ode::{integrate, Flow, Options};
use crate::reactors::{ReactorError, Tol};

/// Constants and switches of the eddy-dissipation closure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdParams {
    /// Rate constant `A` (published value 4.0, unverified against the primary paper).
    pub a: f64,
    /// Product-limiter constant `B` (published value 0.5, unverified).
    pub b: f64,
    /// Turbulent mixing rate `eps / k` (1/s); zero switches the closure off.
    pub eps_over_k: f64,
    /// Include the product-limited term (the published model); `false` is the reactant-only form.
    pub product_limit: bool,
}

/// How the rate of progress of a reaction is obtained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Closure {
    /// Arrhenius (mechanism) rates, laminar.
    Kinetic,
    /// Mixing-limited rate only; irreversible reactions.
    EddyDissipation(EdParams),
    /// `min(kinetic, mixing)` for a positive forward kinetic rate.
    KineticLimited(EdParams),
}

/// Rate of progress (mol/m3/s) of reaction `r` of the eddy-dissipation model for mass fractions `y` (kg/kg) and density
/// `rho`: `A rho (eps/k) min( min_R Y_R / (nu'_R W_R), B Y_P / sum_P nu''_j W_j )`.
pub fn eddy_dissipation_progress(mech: &Mechanism, r: usize, rho: f64, y: &[f64], p: &EdParams) -> f64 {
    if !(p.eps_over_k > 0.0) {
        return 0.0;
    }
    let rx = &mech.reactions[r];
    let mut m = f64::INFINITY;
    for &(k, nu) in &rx.reactants {
        m = m.min(y[k].max(0.0) / (nu * mech.species[k].w));
    }
    if p.product_limit && !rx.products.is_empty() {
        let yp: f64 = rx.products.iter().map(|&(k, _)| y[k].max(0.0)).sum();
        let den: f64 = rx.products.iter().map(|&(k, nu)| nu * mech.species[k].w).sum();
        m = m.min(p.b * yp / den);
    }
    if !m.is_finite() {
        return 0.0;
    }
    p.a * rho * p.eps_over_k * m
}

/// Scales of the Eddy Dissipation Concept: fine-structure length fraction `xi*`, residence time `tau*` (s) and the mass
/// exchange rate `m = xi*^2 / (tau* (1 - xi*^3))` (1/s), from `nu`, `k`, `eps`. `None` for `k <= 0` or `eps <= 0`.
pub fn edc_scales(nu: f64, k: f64, eps: f64, c_xi: f64, c_tau: f64) -> Option<(f64, f64, f64)> {
    if !(k > 0.0 && eps > 0.0 && nu > 0.0) {
        return None;
    }
    let xi = c_xi * (nu * eps / (k * k)).powf(0.25);
    let tau = c_tau * (nu / eps).sqrt();
    let m = xi * xi / (tau * (1.0 - xi * xi * xi));
    Some((xi, tau, m))
}

/// Chemistry of one constant-density cell.
#[derive(Debug, Clone)]
pub struct CellModel<'a> {
    /// The mechanism.
    pub mech: &'a Mechanism,
    /// Constant mass density, kg/m3.
    pub rho: f64,
    /// Constant heat capacity for the heat release, J/kg/K; `None` is isothermal chemistry.
    pub cp: Option<f64>,
    /// Rate closure.
    pub closure: Closure,
    /// Integrator tolerances.
    pub tol: Tol,
}

impl<'a> CellModel<'a> {
    /// Net molar production rates (mol/m3/s) and rates of progress for the state `[T, Y...]`.
    pub fn production(&self, state: &[f64], wdot: &mut [f64], q: &mut [f64]) {
        let m = self.mech;
        let ns = m.species.len();
        let t = state[0];
        let yk: Vec<f64> = state[1..].iter().map(|v| v.max(0.0)).collect();
        let c: Vec<f64> = (0..ns).map(|k| self.rho * yk[k] / m.species[k].w).collect();
        m.rates_of_progress(t, &c, q);
        match self.closure {
            Closure::Kinetic => {}
            Closure::EddyDissipation(p) => {
                for (r, qr) in q.iter_mut().enumerate() {
                    *qr = eddy_dissipation_progress(m, r, self.rho, &yk, &p);
                }
            }
            Closure::KineticLimited(p) => {
                for (r, qr) in q.iter_mut().enumerate() {
                    if *qr > 0.0 {
                        *qr = qr.min(eddy_dissipation_progress(m, r, self.rho, &yk, &p));
                    }
                }
            }
        }
        for w in wdot.iter_mut() {
            *w = 0.0;
        }
        for (r, qr) in m.reactions.iter().zip(q.iter()) {
            for &(k, nu) in &r.reactants {
                wdot[k] -= nu * qr;
            }
            for &(k, nu) in &r.products {
                wdot[k] += nu * qr;
            }
        }
    }

    /// Time derivative `[dT/dt, dY_k/dt]` of the state.
    pub fn rhs(&self, state: &[f64], f: &mut [f64]) {
        let m = self.mech;
        let ns = m.species.len();
        let mut wdot = vec![0.0; ns];
        let mut q = vec![0.0; m.reactions.len()];
        self.production(state, &mut wdot, &mut q);
        let t = state[0];
        for k in 0..ns {
            f[1 + k] = wdot[k] * m.species[k].w / self.rho;
        }
        f[0] = match self.cp {
            None => 0.0,
            Some(cp) => {
                let h: f64 = (0..ns).map(|k| R_GAS * t * m.species[k].h_rt(t) * wdot[k]).sum();
                -h / (self.rho * cp)
            }
        };
    }

    /// Advance the state by `dt` with the stiff integrator. Temperature is left unchanged without heat release.
    pub fn advance(&self, state: &mut [f64], dt: f64) -> Result<(), ReactorError> {
        let n = state.len();
        let mut opt = Options::new(self.tol.rtol, self.tol.atol_y, n);
        opt.atol[0] = self.tol.atol_t;
        let mut rhs = |_t: f64, y: &[f64], f: &mut [f64]| self.rhs(y, f);
        let (y, _) = integrate(&mut rhs, 0.0, dt, state, &opt, &mut |_| Flow::Continue)?;
        if y.iter().any(|v| !v.is_finite()) {
            return Err(ReactorError::Input("non-finite state after the chemistry step".into()));
        }
        state.copy_from_slice(&y);
        Ok(())
    }

    /// Diagonal implicit-sink coefficients `c_k = max(0, -dYdot_k/dY_k)` (1/s) by forward differences in each mass
    /// fraction, for the steady source-term form.
    pub fn sink_coefficients(&self, state: &[f64], f0: &[f64]) -> Vec<f64> {
        let n = self.mech.species.len();
        let mut s = state.to_vec();
        let mut f = vec![0.0; state.len()];
        let mut out = vec![0.0; n];
        for k in 0..n {
            let y = state[1 + k];
            let d = (1e-6 * y.abs()).max(1e-10);
            s[1 + k] = y + d;
            self.rhs(&s, &mut f);
            s[1 + k] = y;
            out[k] = (-(f[1 + k] - f0[1 + k]) / d).max(0.0);
        }
        out
    }

    /// State reached after the fine-structure residence time `tau` (EDC): `(T*, Y*)` in the layout of `state`.
    pub fn fine_structure(&self, state: &[f64], tau: f64) -> Result<Vec<f64>, ReactorError> {
        let mut s = state.to_vec();
        self.advance(&mut s, tau)?;
        Ok(s)
    }
}

/// Exact update of the linear relaxation `dY/dt = m (Y* - Y)` over `dt` (EDC with a frozen fine-structure state).
pub fn relax_towards(y: f64, y_star: f64, m: f64, dt: f64) -> f64 {
    y_star + (y - y_star) * (-m * dt).exp()
}
