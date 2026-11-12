//! Two-way exchange: per-cell momentum and energy given to the carrier (ADR 0057 D1).
//!
//! The record is conservative by construction: per integration sub-step the carrier receives exactly the momentum and the
//! total energy the particle gained from it. The tracker never reads the sources, so the particles are unchanged by recording.
//!
//! With [`crate::Tracker::enable_droplet_exchange`] the record also holds what evaporating parcels give to the carrier (the
//! vapour mass with its momentum, kinetic energy and enthalpy) and what breakup gives to the parcels without a carrier
//! counterpart (ADR 0230 D2 to D6).

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
    /// Kinetic energy given to the particles by walls (reflections) [J] (ADR 0230 D6).
    pub wall_energy: f64,
    /// Momentum given to the particles by gravity with buoyancy [N s]: the sum of `m g_eff h` (ADR 0230 D6).
    pub gravity_impulse: Vec3,
    /// Work of gravity with buoyancy on the particles [J]: the sum of `m g_eff . (x1 - x0)` (ADR 0230 D6).
    pub gravity_work: f64,
    /// Vapour mass given to the carrier per cell [kg] (negative: condensation) (ADR 0230 D3).
    pub vapour_mass: Vec<f64>,
    /// Momentum carried by that vapour per cell [N s]: `dm v1`.
    pub vapour_impulse: Vec<Vec3>,
    /// Kinetic energy carried by that vapour per cell [J]: `dm v1^2/2`.
    pub vapour_kinetic: Vec<f64>,
    /// Enthalpy carried by that vapour per cell [J]: `dm (c_l T1 + L_v)`, the liquid at 0 K being the datum (ADR 0230 D4).
    pub vapour_enthalpy: Vec<f64>,
    /// Momentum given to the parcels by breakup (the normal velocity) without a carrier counterpart [N s] (ADR 0230 D5).
    pub breakup_impulse: Vec3,
    /// Kinetic energy given to the parcels by breakup without a carrier counterpart [J] (ADR 0230 D5).
    pub breakup_energy: f64,
}

/// Per-cell sources of a record with the droplet terms (ADR 0230 D7).
#[derive(Debug, Clone, PartialEq)]
pub struct DropletSources {
    /// Vapour mass source [kg/(m^3 s)].
    pub mass: Vec<f64>,
    /// Momentum source [N/m^3]: drag plus the momentum of the vapour.
    pub momentum: Vec<Vec3>,
    /// Total-energy source [W/m^3]: mechanical, heat, and the kinetic energy and enthalpy of the vapour.
    pub energy: Vec<f64>,
}

impl Coupling {
    /// Empty record for `n` cells.
    pub fn new(n: usize) -> Coupling {
        Coupling {
            impulse: vec![[0.0; 3]; n],
            energy_mech: vec![0.0; n],
            energy_heat: vec![0.0; n],
            wall_impulse: [0.0; 3],
            wall_energy: 0.0,
            gravity_impulse: [0.0; 3],
            gravity_work: 0.0,
            vapour_mass: vec![0.0; n],
            vapour_impulse: vec![[0.0; 3]; n],
            vapour_kinetic: vec![0.0; n],
            vapour_enthalpy: vec![0.0; n],
            breakup_impulse: [0.0; 3],
            breakup_energy: 0.0,
        }
    }

    /// Clear the record.
    pub fn reset(&mut self) {
        self.impulse.fill([0.0; 3]);
        self.energy_mech.iter_mut().for_each(|x| *x = 0.0);
        self.energy_heat.iter_mut().for_each(|x| *x = 0.0);
        self.wall_impulse = [0.0; 3];
        self.wall_energy = 0.0;
        self.gravity_impulse = [0.0; 3];
        self.gravity_work = 0.0;
        self.vapour_mass.fill(0.0);
        self.vapour_impulse.fill([0.0; 3]);
        self.vapour_kinetic.fill(0.0);
        self.vapour_enthalpy.fill(0.0);
        self.breakup_impulse = [0.0; 3];
        self.breakup_energy = 0.0;
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

    /// Sources over an exchange interval `dt` for cell volumes `vol`, with the droplet terms (ADR 0230 D7).
    pub fn droplet_sources(&self, vol: &[f64], dt: f64) -> DropletSources {
        let n = self.impulse.len();
        let mut out = DropletSources {
            mass: vec![0.0; n],
            momentum: vec![[0.0; 3]; n],
            energy: vec![0.0; n],
        };
        for c in 0..n {
            let f = 1.0 / (vol[c] * dt);
            out.mass[c] = self.vapour_mass[c] * f;
            for k in 0..3 {
                out.momentum[c][k] = (self.impulse[c][k] + self.vapour_impulse[c][k]) * f;
            }
            out.energy[c] =
                (self.energy_mech[c] + self.energy_heat[c] + self.vapour_kinetic[c] + self.vapour_enthalpy[c]) * f;
        }
        out
    }
}
