//! Power-law creep with time hardening. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0489 (F-411,
//! the registration of ADR 0314).
//!
//! The von Mises form of the law `d eps_c/dt = (3/2) A q^(n-1) t^m s` (`s` the stress deviator, `q = sqrt(3/2 s:s)`; in
//! uniaxial stress `d eps_c/dt = A sigma^n t^m`), on an isotropic elastic base, integrated by backward Euler in the stress
//! with the time exponent integrated exactly (ADR 0489 arithmetic 1):
//!
//! * trial `sigma_tr = D (eps - eps_c)`, `q_tr` its equivalent stress, the time factor `tau = int_{t0}^{t1} t^m dt`;
//! * `q` the root of `q_tr - q - 3 G A tau q^n = 0` in `(0, q_tr]` by Newton from `q_tr` (monotone for `n >= 1`), at most
//!   [`MAX_LOCAL_ITERATIONS`] iterations, else an error (key [`ITERATION_KEY`]);
//! * `Delta eps_bar = (q_tr - q)/(3 G)`, `eps_c += Delta eps_bar (3/2) s_tr/q_tr`, `sigma = sigma_tr - 3 G Delta eps_bar
//!   s_tr/q_tr`;
//! * tangent `K 1 (x) 1 + 2 G theta I_dev + 2 G (beta - theta) N (x) N`, `theta = q/q_tr`, `beta = 1/(1 + 3 G A tau n
//!   q^(n-1))`, `N = s_tr/|s_tr|`.
//!
//! The state of a point is `[eps_c (6, engineering shear), eps_bar]` (ADR 0440 D3). The time and the time increment come from
//! the point context; no increment is an increment of zero length (the elastic response from the committed state). Time
//! counts from `t = 0`. On the total Lagrangian driver the law acts on the Green-Lagrange strain and the second
//! Piola-Kirchhoff stress (ADR 0489 Limits). Only the nonlinear driver at a point context
//! ([`crate::nonlinear::solve_from_at`], the time-stepping driver [`crate::nonlinear::solve_in_time`]) and the nonlinear
//! transient integrator take the material; every other path refuses it naming [`FOLLOW_UP`].

use crate::error::{FemError, Result};
use crate::material::{apply, isotropic_elasticity, Matrix6, PointContext};
use crate::model::Material;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The package named where a path that does not take the creep material is given one (ADR 0489 D2).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0489";

/// The declared state per integration point (ADR 0440 D3): the creep strain (six values) and the accumulated equivalent
/// creep strain.
pub const STATE_SIZE: usize = 7;

/// The largest number of Newton iterations of the local equation (ADR 0489 D1).
pub const MAX_LOCAL_ITERATIONS: usize = 50;

/// The local iteration stops when the correction is at most this fraction of the trial equivalent stress (ADR 0489 D1).
pub const LOCAL_TOLERANCE: f64 = 1e-14;

/// The key of the error of a non-converged local iteration (ADR 0489 D4).
pub const ITERATION_KEY: &str = "creep iteration";

/// The result of one update.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The stress (Voigt, tensor shear).
    pub stress: [f64; 6],
    /// The consistent tangent `d sigma / d eps` (strain with engineering shear).
    pub tangent: Matrix6,
    /// The state after the step.
    pub state: [f64; STATE_SIZE],
    /// Whether the step crept (a time factor and a deviatoric trial stress).
    pub creeping: bool,
    /// The local Newton iterations.
    pub iterations: usize,
}

/// The creep strain of a state (Voigt, engineering shear).
pub fn creep_strain(state: &[f64]) -> [f64; 6] {
    [state[0], state[1], state[2], state[3], state[4], state[5]]
}

/// The equivalent creep strain of a state.
pub fn equivalent_creep_strain(state: &[f64]) -> f64 {
    state[6]
}

/// The weights of the tensor inner product in Voigt form (the shear components count twice).
const W: [f64; 6] = [1.0, 1.0, 1.0, 2.0, 2.0, 2.0];

/// The time factor `int_{t1 - dt}^{t1} t^m dt` (`dt` exactly for `m = 0`; ADR 0489 arithmetic 1). No validation.
pub fn time_factor(m: f64, t1: f64, dt: f64) -> f64 {
    if dt == 0.0 {
        return 0.0;
    }
    if m == 0.0 {
        return dt;
    }
    let t0 = (t1 - dt).max(0.0);
    (t1.powf(m + 1.0) - t0.powf(m + 1.0)) / (m + 1.0)
}

fn time_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "time", reason }
}

/// The time factor of a step of the point context: 0 without an increment; with one, the end time is needed and the start
/// may not lie before `t = 0` (key `"time"`).
fn point_time_factor(m: f64, point: &PointContext) -> Result<f64> {
    let dt = point.time_increment.unwrap_or(0.0);
    if dt == 0.0 {
        return Ok(0.0);
    }
    let t1 = point
        .time
        .ok_or_else(|| time_error("a creep step with a time increment needs the time of its end".to_string()))?;
    let t0 = t1 - dt;
    if t0 < -4.0 * f64::EPSILON * t1.abs() {
        return Err(time_error(format!(
            "the step [{t0:e}, {t1:e}] s starts before t = 0: time hardening counts from t = 0"
        )));
    }
    Ok(time_factor(m, t1, dt))
}

/// The backward Euler return of a creep material with the isotropic base `(e, nu)` and the law `(a, n, m)` at the strain
/// `strain` from the committed `state` over a step of time factor `tau` (ADR 0489 D1). Without creep (`tau = 0` or no
/// deviatoric trial stress) it returns `apply(&isotropic_elasticity(e, nu), &(strain - eps_c))` and the state unchanged. A
/// state of the wrong length or with a non-finite value is refused (key `"state"`), `n < 1` (key `"n"`), a non-converged
/// local iteration (key [`ITERATION_KEY`]).
pub fn return_map(e: f64, nu: f64, a: f64, n: f64, tau: f64, strain: &[f64; 6], state: &[f64]) -> Result<Update> {
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
            reason: format!("creep state value {i} is not finite"),
        });
    }
    if n < 1.0 {
        return Err(FemError::InvalidMaterial {
            key: "n",
            value: n,
            reason: "the implicit creep return of ADR 0489 needs a stress exponent n >= 1",
        });
    }
    let mut ee = [0.0; 6];
    for (k, v) in ee.iter_mut().enumerate() {
        *v = strain[k] - state[k];
    }
    let d = isotropic_elasticity(e, nu);
    let sigma_tr = apply(&d, &ee);
    let mut out = [0.0; STATE_SIZE];
    out.copy_from_slice(state);
    let mean = (sigma_tr[0] + sigma_tr[1] + sigma_tr[2]) / 3.0;
    let mut s = [0.0; 6];
    for k in 0..6 {
        s[k] = if k < 3 { sigma_tr[k] - mean } else { sigma_tr[k] };
    }
    let norm = (0..6).map(|k| W[k] * s[k] * s[k]).sum::<f64>().sqrt();
    let q_tr = 1.5f64.sqrt() * norm;
    if tau == 0.0 || q_tr == 0.0 {
        return Ok(Update {
            stress: sigma_tr,
            tangent: d,
            state: out,
            creeping: false,
            iterations: 0,
        });
    }
    let g = e / (2.0 * (1.0 + nu));
    let kappa = e / (3.0 * (1.0 - 2.0 * nu));
    let c = 3.0 * g * a * tau;
    let mut q = q_tr;
    let mut iterations = 0;
    loop {
        let r = q_tr - q - c * q.powf(n);
        let dr = -1.0 - c * n * q.powf(n - 1.0);
        let dq = -r / dr;
        q += dq;
        iterations += 1;
        if !q.is_finite() || q <= 0.0 {
            return Err(FemError::InvalidValue {
                key: ITERATION_KEY,
                reason: format!(
                    "the local creep iteration left (0, q_tr] at iteration {iterations} (q = {q:e}, q_tr = {q_tr:e})"
                ),
            });
        }
        if dq.abs() <= LOCAL_TOLERANCE * q_tr {
            break;
        }
        if iterations == MAX_LOCAL_ITERATIONS {
            return Err(FemError::InvalidValue {
                key: ITERATION_KEY,
                reason: format!(
                    "the local creep iteration did not converge in {MAX_LOCAL_ITERATIONS} iterations (q/q_tr = {:e}, \
                     last correction {:e} q_tr)",
                    q / q_tr,
                    dq / q_tr
                ),
            });
        }
    }
    let de = (q_tr - q) / (3.0 * g);
    let mut stress = sigma_tr;
    for k in 0..6 {
        stress[k] -= 3.0 * g * de * s[k] / q_tr;
        out[k] += de * 1.5 * s[k] / q_tr * W[k];
    }
    out[6] += de;
    let theta = q / q_tr;
    let beta = 1.0 / (1.0 + c * n * q.powf(n - 1.0));
    let mut nn = [0.0; 6];
    for k in 0..6 {
        nn[k] = s[k] / norm;
    }
    let mut t = [[0.0; 6]; 6];
    for (i, row) in t.iter_mut().enumerate() {
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
            *v = kappa * one + 2.0 * g * theta * (isym - one / 3.0) + 2.0 * g * (beta - theta) * nn[i] * nn[j];
        }
    }
    Ok(Update {
        stress,
        tangent: t,
        state: out,
        creeping: true,
        iterations,
    })
}

/// The update of `material`, which must be a creep material on an isotropic base, at the strain `strain` from the committed
/// `state` over the step of the point context (ADR 0489 D1): the time factor of the step (no time increment: 0, the elastic
/// response; with one, the end time is needed, key `"time"`), then [`return_map`]. Refuses another material (key
/// `"material"`) and an orthotropic base (`NotYetSupported`).
pub fn update(material: &Material, strain: &[f64; 6], state: &[f64], point: &PointContext) -> Result<Update> {
    match material {
        Material::Creep { base, a, n, m } => match base.as_ref() {
            Material::IsotropicElastic { e, nu, .. } => {
                let tau = point_time_factor(*m, point)?;
                return_map(*e, *nu, *a, *n, tau, strain, state)
            }
            _ => Err(FemError::NotYetSupported {
                item: "material",
                variant: material.name(),
                location: "an orthotropic base".to_string(),
                package: FOLLOW_UP,
            }),
        },
        other => Err(FemError::InvalidValue {
            key: "material",
            reason: format!(
                "a {} material: the creep return of ADR 0489 takes a Creep material",
                other.name()
            ),
        }),
    }
}
