//! Droplet evaporation (ADR 0224 E1 to E8; library only).
//!
//! The quasi-steady film model with Bird's correction of the heat transfer (model "B" of Both, Mira and Lehmkuhl,
//! arXiv:2201.02818, Table 3): per real particle `mdot = pi d rho_m D_m Sh0 ln(1 + B_M)` and
//! `m c_l dT_p/dt = pi d lambda_m Nu0 g(beta) (T_s - T_p) - mdot L_v`, `g(beta) = beta/(e^beta - 1)`, `beta = phi ln(1 + B_M)`,
//! `phi = c_p,vap rho_m D_m Sh0/(lambda_m Nu0)`, Ranz-Marshall `Nu0`, `Sh0`; the surface vapour from the Clausius-Clapeyron
//! relation through the boiling point and Raoult's law. Over a sub-step the coefficients are held: `d^2` falls linearly
//! (`d(d^2)/dt = -4 rho_m D_m Sh0 ln(1 + B_M)/rho_l`) and the temperature takes one exponential Euler step, whose fixed point
//! is the wet bulb. The carrier is not changed: the vapour mass is booked per cell for the caller.

use crate::coupling::Thermal;
use crate::{err, Result};
use std::f64::consts::PI;

/// Molar gas constant `N_A k_B` [J/(mol K)] (CODATA, exact).
pub const R_UNIVERSAL: f64 = 8.314_462_618_153_24;

/// The evaporation inputs that are not carrier fields (ADR 0224 E1). All constant.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaporationSpec {
    /// Latent heat of vaporisation `L_v` [J/kg].
    pub latent_heat: f64,
    /// Boiling temperature `T_b` [K] at the pressure `boiling_pressure`.
    pub boiling_temperature: f64,
    /// Pressure `p_b` [Pa] at which the liquid boils at `T_b`.
    pub boiling_pressure: f64,
    /// Carrier pressure `p` [Pa].
    pub pressure: f64,
    /// Vapour molar mass `W_f` [kg/mol].
    pub vapour_molar_mass: f64,
    /// Molar mass of the bath gas `W_b` [kg/mol].
    pub gas_molar_mass: f64,
    /// Vapour diffusivity in the carrier `D_m` [m^2/s].
    pub diffusivity: f64,
    /// Vapour specific heat `c_p,vap` [J/(kg K)].
    pub vapour_cp: f64,
    /// Diameter [m] at which a droplet counts as evaporated (its rest is booked as vapour).
    pub min_diameter: f64,
    /// Largest sub-step as a fraction of the droplet's thermal time `tau_T`, in `(0, 1]`.
    pub max_step_fraction: f64,
}

fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

impl EvaporationSpec {
    /// Check the spec (ADR 0224 S1); the error names the first defect.
    pub fn validate(&self) -> Result<()> {
        let named = [
            ("the latent heat", self.latent_heat),
            ("the boiling temperature", self.boiling_temperature),
            ("the boiling pressure", self.boiling_pressure),
            ("the carrier pressure", self.pressure),
            ("the vapour molar mass", self.vapour_molar_mass),
            ("the gas molar mass", self.gas_molar_mass),
            ("the vapour diffusivity", self.diffusivity),
            ("the vapour specific heat", self.vapour_cp),
            ("the minimum diameter", self.min_diameter),
        ];
        for (what, v) in named {
            if !positive(v) {
                return err(format!("evaporation: {what} must be finite and positive"));
            }
        }
        if !(self.max_step_fraction > 0.0 && self.max_step_fraction <= 1.0) {
            return err("evaporation: the sub-step fraction must be in (0, 1]");
        }
        Ok(())
    }
}

fn check_vapour(y: &[f64], n: usize) -> Result<()> {
    if y.len() != n {
        return err("evaporation: the vapour mass fraction needs one value per cell");
    }
    if y.iter().any(|v| !(v.is_finite() && *v >= 0.0 && *v < 1.0)) {
        return err("evaporation: the vapour mass fraction must be finite and in [0, 1)");
    }
    Ok(())
}

/// The rates of one droplet at its present state (ADR 0224 E2 to E6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvapRates {
    /// Mass lost per second per real particle `mdot` [kg/s] (negative: condensation).
    pub mdot: f64,
    /// `K_loc = -d(d^2)/dt` [m^2/s].
    pub k: f64,
    /// Heat-transfer conductance with Bird's correction `pi d lambda_m Nu0 g(beta)` [W/K].
    pub h_eff: f64,
    /// `F = dT_p/dt` [K/s].
    pub f: f64,
    /// `J = dF/dT_p` [1/s] (diameter, Reynolds number and mass held).
    pub jac: f64,
    /// Thermal time `tau_T = m c_l / h_eff` [s].
    pub tau_t: f64,
    /// Spalding mass-transfer number `B_M`.
    pub b_m: f64,
    /// `Nu0` (Ranz-Marshall).
    pub nu: f64,
    /// `Sh0`.
    pub sh: f64,
}

/// Bird's factor `g(beta) = beta/(e^beta - 1)` and its derivative.
fn bird(beta: f64) -> (f64, f64) {
    if beta.abs() < 1e-6 {
        return (1.0 - 0.5 * beta + beta * beta / 12.0, -0.5 + beta / 6.0);
    }
    let em = beta.exp_m1();
    let g = beta / em;
    let dg = (em - beta * (em + 1.0)) / (em * em);
    (g, dg)
}

/// The evaporation of the parcels with its booking (an observation for the carrier).
#[derive(Debug, Clone)]
pub struct Evaporation {
    spec: EvaporationSpec,
    y_s: Vec<f64>,
    /// Vapour mass given to each cell [kg] since the start.
    source: Vec<f64>,
    evaporated: f64,
    completed: u64,
}

impl Evaporation {
    /// The model on a mesh of `n_cells` cells with the seen vapour mass fraction `y_s` per cell.
    pub fn new(spec: EvaporationSpec, y_s: Vec<f64>, n_cells: usize) -> Result<Evaporation> {
        spec.validate()?;
        check_vapour(&y_s, n_cells)?;
        Ok(Evaporation {
            spec,
            y_s,
            source: vec![0.0; n_cells],
            evaporated: 0.0,
            completed: 0,
        })
    }

    /// Replace the seen vapour mass fraction (same rule as [`Evaporation::new`]).
    pub fn set_vapour_fraction(&mut self, y_s: &[f64]) -> Result<()> {
        check_vapour(y_s, self.y_s.len())?;
        self.y_s.copy_from_slice(y_s);
        Ok(())
    }

    /// The spec.
    pub fn spec(&self) -> &EvaporationSpec {
        &self.spec
    }

    /// The seen vapour mass fraction per cell.
    pub fn vapour_fraction(&self) -> &[f64] {
        &self.y_s
    }

    /// Vapour mass given to each cell [kg] since the start.
    pub fn source(&self) -> &[f64] {
        &self.source
    }

    /// Total vapour mass given to the carrier [kg] since the start.
    pub fn evaporated(&self) -> f64 {
        self.evaporated
    }

    /// Parcels that reached the minimum diameter.
    pub fn completed(&self) -> u64 {
        self.completed
    }

    /// Surface vapour mole fraction `X(T) = (p_b/p) exp((L_v W_f/R)(1/T_b - 1/T))` (ADR 0224 E2).
    pub fn surface_mole_fraction(&self, t: f64) -> f64 {
        let s = &self.spec;
        let a = s.latent_heat * s.vapour_molar_mass / R_UNIVERSAL;
        s.boiling_pressure / s.pressure * (a * (1.0 / s.boiling_temperature - 1.0 / t)).exp()
    }

    /// Surface vapour mass fraction `X/(X + (1 - X) W_b/W_f)`.
    pub fn surface_mass_fraction(&self, t: f64) -> f64 {
        let s = &self.spec;
        let x = self.surface_mole_fraction(t);
        x / (x + (1.0 - x) * s.gas_molar_mass / s.vapour_molar_mass)
    }

    /// The rates of a droplet of diameter `d`, liquid density `rho_l` and temperature `t_p` in `cell`, seeing the relative
    /// speed `rel_speed`, with the mean-state carrier density `rho_g` and viscosity `mu_g` and the heat-transfer data `th`.
    /// An error when the droplet is at or above its boiling point at the carrier pressure (ADR 0224 E2, Q5).
    #[allow(clippy::too_many_arguments)]
    pub fn rates(
        &self,
        th: &Thermal,
        rho_g: f64,
        mu_g: f64,
        cell: usize,
        d: f64,
        rho_l: f64,
        t_p: f64,
        rel_speed: f64,
    ) -> Result<EvapRates> {
        let s = &self.spec;
        let x = self.surface_mole_fraction(t_p);
        if x.is_nan() || x >= 1.0 {
            return err(format!(
                "evaporation: a droplet at {t_p} K is at or above its boiling point at the carrier pressure (ADR 0224 E2)"
            ));
        }
        let ratio = s.gas_molar_mass / s.vapour_molar_mass;
        let den = x + (1.0 - x) * ratio;
        let y_i = x / den;
        let y_s = self.y_s[cell];
        let b_m = (y_i - y_s) / (1.0 - y_i);
        let l1 = b_m.ln_1p();
        // dY_i/dT through X(T)
        let a = s.latent_heat * s.vapour_molar_mass / R_UNIVERSAL;
        let dx = x * a / (t_p * t_p);
        let dy = ratio / (den * den) * dx;
        let dl1 = dy / (1.0 - y_i);
        let re = rho_g * d * rel_speed / mu_g;
        let pr = mu_g * th.cp_f / th.k_f;
        let sc = mu_g / (rho_g * s.diffusivity);
        let nu = 2.0 + 0.6 * re.sqrt() * pr.cbrt();
        let sh = 2.0 + 0.6 * re.sqrt() * sc.cbrt();
        let c_m = PI * d * rho_g * s.diffusivity * sh;
        let mdot = c_m * l1;
        let dmdot = c_m * dl1;
        let phi = s.vapour_cp * rho_g * s.diffusivity * sh / (th.k_f * nu);
        let (g, dg) = bird(phi * l1);
        let c_h = PI * d * th.k_f * nu;
        let h_eff = c_h * g;
        let dh_eff = c_h * dg * phi * dl1;
        let t_s = th.t_fluid[cell];
        let mc = rho_l * PI * d * d * d / 6.0 * th.cp_p;
        let f = (h_eff * (t_s - t_p) - mdot * s.latent_heat) / mc;
        let jac = (dh_eff * (t_s - t_p) - h_eff - dmdot * s.latent_heat) / mc;
        Ok(EvapRates {
            mdot,
            k: 4.0 * rho_g * s.diffusivity * sh * l1 / rho_l,
            h_eff,
            f,
            jac,
            tau_t: mc / h_eff,
            b_m,
            nu,
            sh,
        })
    }

    /// Book `dm` kg of vapour (all real particles of a parcel) in `cell`.
    pub(crate) fn book(&mut self, cell: usize, dm: f64) {
        self.source[cell] += dm;
        self.evaporated += dm;
    }

    /// Count a parcel that reached the minimum diameter.
    pub(crate) fn complete(&mut self) {
        self.completed += 1;
    }

    /// The booking as bits, for fingerprints.
    pub(crate) fn bits(&self) -> Vec<u64> {
        let mut out: Vec<u64> = self.source.iter().map(|x| x.to_bits()).collect();
        out.push(self.evaporated.to_bits());
        out.push(self.completed);
        out
    }
}

/// `phi1(z) = (e^z - 1)/z`, `phi1(0) = 1`.
pub(crate) fn phi1(z: f64) -> f64 {
    if z == 0.0 {
        1.0
    } else {
        z.exp_m1() / z
    }
}
