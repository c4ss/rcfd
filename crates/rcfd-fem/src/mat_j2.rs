//! J2 plasticity with isotropic and kinematic hardening. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by
//! ADR 0475 (F-422, the registration of ADR 0325).
//!
//! Small-strain von Mises plasticity with linear isotropic and linear kinematic hardening, integrated by the radial return
//! (backward Euler, closed form for linear hardening) with its consistent tangent (ADR 0475 arithmetic 1, 2):
//!
//! * trial `sigma_tr = D (eps - eps_p)`, `xi_tr = dev sigma_tr - beta`, `f = |xi_tr| - sqrt(2/3) (sigma_y + H_iso alpha)`;
//! * if `f > 0`: `dg = f / (2 mu + (2/3)(H_iso + H_kin))`, `n = xi_tr / |xi_tr|`, `sigma = sigma_tr - 2 mu dg n`,
//!   `eps_p += dg n`, `beta += (2/3) H_kin dg n`, `alpha += sqrt(2/3) dg`;
//! * tangent `C = kappa 1 (x) 1 + 2 mu theta I_dev - 2 mu theta_bar n (x) n`, `theta = 1 - 2 mu dg / |xi_tr|`,
//!   `theta_bar = 1 / (1 + (H_iso + H_kin)/(3 mu)) - (1 - theta)`.
//!
//! Voigt order xx, yy, zz, xy, yz, zx; strains (and the plastic strain) with engineering shear, stresses (and the back
//! stress) with tensor shear. On the total Lagrangian driver the law acts on the Green-Lagrange strain and the second
//! Piola-Kirchhoff stress (large rotation, small strain; ADR 0475 arithmetic 6). Only the nonlinear static solve takes the
//! material; every other path refuses it naming [`FOLLOW_UP`] (ADR 0475 D2).

use crate::error::{FemError, Result};
use crate::material::{apply, isotropic_elasticity, Matrix6};
use crate::model::Material;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The package named where a path that does not take the J2 material is given one (ADR 0475 D2).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0475";

/// The declared state per integration point (ADR 0440 D3): the plastic strain (six), the back stress (six) and the
/// equivalent plastic strain.
pub const STATE_SIZE: usize = 13;

/// The result of one return map.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The stress (Voigt, tensor shear).
    pub stress: [f64; 6],
    /// The consistent tangent `d sigma / d eps` (strain with engineering shear).
    pub tangent: Matrix6,
    /// The state after the step.
    pub state: [f64; STATE_SIZE],
    /// Whether the step was plastic.
    pub plastic: bool,
}

/// The plastic strain of a state (Voigt, engineering shear).
pub fn plastic_strain(state: &[f64]) -> [f64; 6] {
    [state[0], state[1], state[2], state[3], state[4], state[5]]
}

/// The back stress of a state (Voigt, tensor shear).
pub fn back_stress(state: &[f64]) -> [f64; 6] {
    [state[6], state[7], state[8], state[9], state[10], state[11]]
}

/// The equivalent plastic strain `alpha` of a state.
pub fn equivalent_plastic_strain(state: &[f64]) -> f64 {
    state[12]
}

/// The weights of the tensor inner product in Voigt form (the shear components count twice).
const W: [f64; 6] = [1.0, 1.0, 1.0, 2.0, 2.0, 2.0];

/// The return map of a J2 material at the strain `strain` from the committed state `state` (ADR 0475 D1). The elastic
/// branch computes `apply(&isotropic_elasticity(e, nu), &(strain - eps_p))`, the bits of the elastic base when nothing has
/// yielded. A state of the wrong length or with a non-finite value is refused (key `"state"`).
pub fn return_map(
    e: f64,
    nu: f64,
    yield_stress: f64,
    h_iso: f64,
    h_kin: f64,
    strain: &[f64; 6],
    state: &[f64],
) -> Result<Update> {
    if state.len() != STATE_SIZE {
        return Err(FemError::FieldLength {
            key: "state",
            expected: STATE_SIZE,
            found: state.len(),
        });
    }
    if let Some(i) = state.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "state",
            reason: format!("J2 state value {i} is not finite"),
        });
    }
    let mut ee = [0.0; 6];
    for (k, v) in ee.iter_mut().enumerate() {
        *v = strain[k] - state[k];
    }
    let d = isotropic_elasticity(e, nu);
    let sigma_tr = apply(&d, &ee);
    let mean = (sigma_tr[0] + sigma_tr[1] + sigma_tr[2]) / 3.0;
    let mut xi = [0.0; 6];
    for k in 0..6 {
        let s = if k < 3 { sigma_tr[k] - mean } else { sigma_tr[k] };
        xi[k] = s - state[6 + k];
    }
    let norm = (0..6).map(|k| W[k] * xi[k] * xi[k]).sum::<f64>().sqrt();
    let alpha = state[12];
    let s23 = (2.0f64 / 3.0).sqrt();
    let f = norm - s23 * (yield_stress + h_iso * alpha);
    let mut out = [0.0; STATE_SIZE];
    out.copy_from_slice(state);
    if f <= 0.0 {
        return Ok(Update {
            stress: sigma_tr,
            tangent: d,
            state: out,
            plastic: false,
        });
    }
    let mu = e / (2.0 * (1.0 + nu));
    let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
    let kappa = lambda + 2.0 * mu / 3.0;
    let h = h_iso + h_kin;
    let dg = f / (2.0 * mu + 2.0 / 3.0 * h);
    let mut n = [0.0; 6];
    for k in 0..6 {
        n[k] = xi[k] / norm;
    }
    let mut stress = sigma_tr;
    for k in 0..6 {
        stress[k] -= 2.0 * mu * dg * n[k];
        out[k] += dg * n[k] * W[k];
        out[6 + k] += 2.0 / 3.0 * h_kin * dg * n[k];
    }
    out[12] += s23 * dg;
    let theta = 1.0 - 2.0 * mu * dg / norm;
    let theta_bar = 1.0 / (1.0 + h / (3.0 * mu)) - (1.0 - theta);
    let mut c = [[0.0; 6]; 6];
    for (i, row) in c.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let one = if i < 3 && j < 3 { 1.0 } else { 0.0 };
            let isym = if i == j {
                if i < 3 {
                    1.0
                } else {
                    0.5
                }
            } else {
                0.0
            };
            *v = kappa * one + 2.0 * mu * theta * (isym - one / 3.0) - 2.0 * mu * theta_bar * n[i] * n[j];
        }
    }
    Ok(Update {
        stress,
        tangent: c,
        state: out,
        plastic: true,
    })
}

/// The return map of `material`, which must be a J2 material (else refused, key `"material"`).
pub fn update(material: &Material, strain: &[f64; 6], state: &[f64]) -> Result<Update> {
    match material {
        Material::J2 {
            base,
            yield_stress,
            isotropic,
            kinematic,
        } => return_map(
            base.youngs_modulus(),
            base.poisson_ratio(),
            *yield_stress,
            *isotropic,
            *kinematic,
            strain,
            state,
        ),
        other => Err(FemError::InvalidValue {
            key: "material",
            reason: format!("a {} material: the return map of ADR 0475 takes a J2 material", other.name()),
        }),
    }
}
