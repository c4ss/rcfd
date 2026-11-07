//! A heterogeneous surface reaction between a solid in the parcels and gas species of the carrier (ADR 0208 D4; library
//! only).
//!
//! `nu_s S(solid) + sum_{nu_i < 0} abs(nu_i) G_i -> sum_{nu_i > 0} nu_i G_i`, at the rate
//! `omega = k_s rho_g Y_o / M_o` [mol/(m^2 s)] per unit external surface of a particle, first order in the molar
//! concentration of the gas species `o` in the cell where a sub-step starts (kinetic control: no film diffusion, no
//! temperature dependence; a choice of ADR 0208, Q7). A parcel holds a reacting mass `f_r m0` and an inert rest; the
//! particle shrinks at constant density, `dd/dt = -2 nu_s M_s omega / rho_p`, exactly over a sub-step with `omega` frozen.
//! The carrier is not changed: the mass every gas species gains or loses is booked per cell for the caller.

use crate::{err, Result};

/// One gas species of the reaction.
#[derive(Debug, Clone, PartialEq)]
pub struct GasSpecies {
    /// Name (for the caller; not interpreted).
    pub name: String,
    /// Molar mass [kg/mol] (> 0).
    pub molar_mass: f64,
    /// Stoichiometric coefficient: negative for a species taken from the gas, positive for one given to it (not 0).
    pub coefficient: f64,
}

/// The reaction (ADR 0208 D4).
#[derive(Debug, Clone, PartialEq)]
pub struct ReactionSpec {
    /// Molar mass of the solid [kg/mol] (> 0).
    pub solid_molar_mass: f64,
    /// Moles of solid per unit of reaction extent (> 0).
    pub solid_coefficient: f64,
    /// Gas species with signed coefficients.
    pub gas: Vec<GasSpecies>,
    /// Index into `gas` of the species whose concentration sets the rate (negative coefficient).
    pub oxidiser: usize,
    /// Rate coefficient `k_s` [m/s] (> 0).
    pub rate: f64,
    /// Reacting mass fraction `f_r` of a parcel at its first reacting sub-step, in `(0, 1)`.
    pub reacting_fraction: f64,
}

/// Largest relative imbalance `abs(sum_i nu_i M_i - nu_s M_s) / (nu_s M_s)` a reaction may have.
pub const BALANCE_TOLERANCE: f64 = 1e-12;

fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

impl ReactionSpec {
    /// Check the spec (ADR 0208 S1); the error names the first defect.
    pub fn validate(&self) -> Result<()> {
        if !positive(self.rate) {
            return err("reaction: the rate coefficient k_s must be finite and positive");
        }
        if !positive(self.solid_molar_mass) || !positive(self.solid_coefficient) {
            return err("reaction: the molar mass and the coefficient of the solid must be finite and positive");
        }
        if !(self.reacting_fraction > 0.0 && self.reacting_fraction < 1.0) {
            return err(
                "reaction: the reacting fraction must be in (0, 1) (a parcel keeps an inert rest, ADR 0208 Q9)",
            );
        }
        if self.gas.is_empty() {
            return err("reaction: at least one gas species is needed");
        }
        for g in &self.gas {
            if !positive(g.molar_mass) {
                return err(format!(
                    "reaction: gas species '{}': the molar mass must be finite and positive",
                    g.name
                ));
            }
            if !(g.coefficient.is_finite() && g.coefficient != 0.0) {
                return err(format!(
                    "reaction: gas species '{}': the coefficient must be finite and not zero",
                    g.name
                ));
            }
        }
        match self.gas.get(self.oxidiser) {
            None => {
                return err(format!(
                    "reaction: the oxidiser index {} is not a gas species (there are {})",
                    self.oxidiser,
                    self.gas.len()
                ))
            }
            Some(g) if g.coefficient >= 0.0 => {
                return err(format!(
                    "reaction: the oxidiser '{}' must be taken from the gas (negative coefficient)",
                    g.name
                ))
            }
            Some(_) => {}
        }
        let solid = self.solid_coefficient * self.solid_molar_mass;
        let gas: f64 = self.gas.iter().map(|g| g.coefficient * g.molar_mass).sum();
        if (gas - solid).abs() > BALANCE_TOLERANCE * solid {
            return err(format!(
                "reaction: the molar masses do not balance: the gas gains {gas} kg/mol of reaction, the solid loses {solid}"
            ));
        }
        Ok(())
    }
}

fn check_fraction(y: &[f64], n: usize) -> Result<()> {
    if y.len() != n {
        return err("reaction: the oxidiser mass fraction needs one value per cell");
    }
    if y.iter().any(|v| !(v.is_finite() && (0.0..=1.0).contains(v))) {
        return err("reaction: the oxidiser mass fraction must be finite and in [0, 1]");
    }
    Ok(())
}

/// The reaction with its booking (an observation for the carrier).
#[derive(Debug, Clone)]
pub struct SurfaceReaction {
    spec: ReactionSpec,
    y_o: Vec<f64>,
    /// Per gas species, per cell: the mass given to the gas [kg] (negative = taken).
    sources: Vec<Vec<f64>>,
    solid_reacted: f64,
    extent: f64,
}

impl SurfaceReaction {
    /// The reaction on a mesh of `n_cells` cells with the oxidiser mass fraction `y_o` per cell.
    pub fn new(spec: ReactionSpec, y_o: Vec<f64>, n_cells: usize) -> Result<SurfaceReaction> {
        spec.validate()?;
        check_fraction(&y_o, n_cells)?;
        let sources = vec![vec![0.0; n_cells]; spec.gas.len()];
        Ok(SurfaceReaction {
            spec,
            y_o,
            sources,
            solid_reacted: 0.0,
            extent: 0.0,
        })
    }

    /// Replace the oxidiser mass fraction (same rule as [`SurfaceReaction::new`]).
    pub fn set_oxidiser_fraction(&mut self, y_o: &[f64]) -> Result<()> {
        check_fraction(y_o, self.y_o.len())?;
        self.y_o.copy_from_slice(y_o);
        Ok(())
    }

    /// The spec.
    pub fn spec(&self) -> &ReactionSpec {
        &self.spec
    }

    /// The oxidiser mass fraction per cell.
    pub fn oxidiser_fraction(&self) -> &[f64] {
        &self.y_o
    }

    /// Mass given to the gas per cell [kg] for gas species `i` (negative = taken from it), since the start.
    pub fn sources(&self, i: usize) -> &[f64] {
        &self.sources[i]
    }

    /// Solid mass the parcels lost to the reaction [kg], since the start.
    pub fn solid_reacted(&self) -> f64 {
        self.solid_reacted
    }

    /// Reaction extent [mol], since the start.
    pub fn extent(&self) -> f64 {
        self.extent
    }

    /// Molar rate per unit surface `k_s rho_g Y_o / M_o` [mol/(m^2 s)] in `cell`.
    pub fn rate_density(&self, rho_g: f64, cell: usize) -> f64 {
        let o = &self.spec.gas[self.spec.oxidiser];
        self.spec.rate * rho_g * self.y_o[cell] / o.molar_mass
    }

    /// React one parcel over `h` (ADR 0208 R3, R4): diameter `d`, density `rho_p`, weight `n`, reacting mass `m_r` and
    /// inert mass `m_i` per real particle, the sub-step starting in `cell`. Returns the new diameter when mass reacted
    /// (`m_r` updated, the booking made), `None` when nothing reacted (nothing changed).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn react(
        &mut self,
        cell: usize,
        rho_g: f64,
        rho_p: f64,
        d: f64,
        n: f64,
        m_r: &mut f64,
        m_i: f64,
        h: f64,
    ) -> Option<f64> {
        let s = &self.spec;
        let omega = self.rate_density(rho_g, cell);
        let d_lin = d - 2.0 * s.solid_coefficient * s.solid_molar_mass * omega * h / rho_p;
        let c = std::f64::consts::PI * rho_p / 6.0;
        let dm = m_r.min(c * (d * d * d - d_lin.max(0.0).powi(3)));
        if dm <= 0.0 {
            return None;
        }
        if dm >= *m_r {
            *m_r = 0.0;
        } else {
            *m_r -= dm;
        }
        let dxi = n * dm / (s.solid_coefficient * s.solid_molar_mass);
        for (src, g) in self.sources.iter_mut().zip(&s.gas) {
            src[cell] += g.coefficient * g.molar_mass * dxi;
        }
        self.solid_reacted += n * dm;
        self.extent += dxi;
        Some((6.0 * (m_i + *m_r) / (std::f64::consts::PI * rho_p)).cbrt())
    }

    /// The booking as bits, for fingerprints.
    pub(crate) fn bits(&self) -> Vec<u64> {
        let mut out: Vec<u64> = self.sources.iter().flatten().map(|x| x.to_bits()).collect();
        out.push(self.solid_reacted.to_bits());
        out.push(self.extent.to_bits());
        out
    }
}
