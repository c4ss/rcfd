//! Two-way exchange: per-cell momentum and energy given to the carrier (ADR 0057 D1).
//!
//! The record is conservative by construction: per integration sub-step the carrier receives exactly the momentum and the
//! total energy the particle gained from it. The tracker never reads the sources, so the particles are unchanged by recording.

use rcfd_core::vec3::Vec3;

/// Heat-transfer data of the carrier seen by the particles (Ranz-Marshall lumped sphere).
#[derive(Debug, Clone, PartialEq)]
pub struct Thermal {
    /// Particle specific heat [J/(kg K)].
    pub cp_p: f64,
    /// Carrier conductivity [W/(m K)].
    pub k_f: f64,
    /// Carrier specific heat [J/(kg K)].
    pub cp_f: f64,
    /// Carrier temperature per cell [K].
    pub t_fluid: Vec<f64>,
}

/// Nusselt number `2 + 0.6 Re^(1/2) Pr^(1/3)` (Ranz and Marshall 1952, secondary source).
pub fn nusselt(re: f64, pr: f64) -> f64 {
    2.0 + 0.6 * re.sqrt() * pr.cbrt()
}

/// Thermal relaxation time `rho_p c_p d^2/(6 k_f Nu)`.
pub fn tau_thermal(rho_p: f64, cp_p: f64, d: f64, k_f: f64, nu: f64) -> f64 {
    rho_p * cp_p * d * d / (6.0 * k_f * nu)
}

/// Accumulated exchange since the last [`Coupling::reset`].
#[derive(Debug, Clone, PartialEq)]
pub struct Coupling {
    /// Momentum given to the carrier per cell [N s].
    pub impulse: Vec<Vec3>,
    /// Mechanical total-energy part given to the carrier per cell [J]: `-(Delta KE - W_g)`.
    pub energy_mech: Vec<f64>,
    /// Heat given to the carrier per cell [J]: `-m c_p Delta T_p`.
    pub energy_heat: Vec<f64>,
    /// Momentum given to the particles by walls (reflections) [N s].
    pub wall_impulse: Vec3,
}

impl Coupling {
    /// Empty record for `n` cells.
    pub fn new(n: usize) -> Coupling {
        Coupling {
            impulse: vec![[0.0; 3]; n],
            energy_mech: vec![0.0; n],
            energy_heat: vec![0.0; n],
            wall_impulse: [0.0; 3],
        }
    }

    /// Clear the record.
    pub fn reset(&mut self) {
        self.impulse.fill([0.0; 3]);
        self.energy_mech.iter_mut().for_each(|x| *x = 0.0);
        self.energy_heat.iter_mut().for_each(|x| *x = 0.0);
        self.wall_impulse = [0.0; 3];
    }

    /// Sources over an exchange interval `dt` for cell volumes `vol`: momentum [N/m^3] and total energy [W/m^3] (mechanical + heat).
    pub fn sources(&self, vol: &[f64], dt: f64) -> (Vec<Vec3>, Vec<f64>) {
        let n = self.impulse.len();
        let mut sm = vec![[0.0; 3]; n];
        let mut se = vec![0.0; n];
        for c in 0..n {
            let f = 1.0 / (vol[c] * dt);
            sm[c] = [self.impulse[c][0] * f, self.impulse[c][1] * f, self.impulse[c][2] * f];
            se[c] = (self.energy_mech[c] + self.energy_heat[c]) * f;
        }
        (sm, se)
    }
}
