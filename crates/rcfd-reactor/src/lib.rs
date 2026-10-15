//! 0D reactor toolkit (roadmap F-112): CHEMKIN-format mechanisms with NASA-7 thermodynamics, an in-repo stiff
//! integrator, Gibbs-minimisation equilibrium, and batch / perfectly stirred / plug-flow reactor models.
//! See `docs/adr/0013-reactor-and-fwh.md` for the checks and limits.

// NaN-rejecting comparisons (`!(x > 0.0)`) are intentional; the integrator callback types are used once.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::type_complexity)]

pub mod case;
pub mod cell;
pub mod equil;
pub mod mech;
pub mod ode;
pub mod reactors;
pub mod toy;

pub use mech::{MechError, Mechanism, R_GAS};

/// Mole-fraction vector from `(species, amount)` pairs (normalised). Unknown names are an error.
pub fn mole_fractions(mech: &Mechanism, comp: &[(String, f64)]) -> Result<Vec<f64>, String> {
    let mut x = vec![0.0; mech.species.len()];
    for (name, v) in comp {
        let Some(k) = mech.species_index(name) else {
            return Err(format!("unknown species '{name}'"));
        };
        if !(*v >= 0.0) {
            return Err(format!("amount of '{name}' must be non-negative (got {v})"));
        }
        x[k] += v;
    }
    let tot: f64 = x.iter().sum();
    if !(tot > 0.0) {
        return Err("composition sums to zero".into());
    }
    Ok(x.iter().map(|v| v / tot).collect())
}
