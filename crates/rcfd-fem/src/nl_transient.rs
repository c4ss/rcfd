//! Nonlinear time integration (Newton inside the Newmark and generalised-alpha step). Declared by ADR 0440 (F-408, the
//! scaffold of ADR 0311); filled by ADR 0479 (the registration of ADR 0326, F-423).
//!
//! `M a + C v + f_int(u) = f(t)` for the total Lagrangian solids of [`crate::nonlinear`] (with the J2 history of
//! [`crate::mat_j2`]), the consistent mass `M`, Rayleigh damping `C = alpha_R M + beta_R K_0` (`K_0` the tangent at `u = 0`)
//! and the load histories of [`crate::transient`]. The step (ADR 0479 D1) with the parameters `(alpha_m, alpha_f, beta,
//! gamma)` of [`Method::parameters`]: the unknown is `u_{n+1}`, `a_{n+1} = (u_{n+1} - u~)/(beta h^2)` with `u~ = u_n + h v_n +
//! h^2 (1/2 - beta) a_n`, `v_{n+1} = v~ + gamma h a_{n+1}` with `v~ = v_n + h (1 - gamma) a_n`, and
//! `M a_m + C v_f + (1 - alpha_f) f_int(u_{n+1}) + alpha_f f_int,n = (1 - alpha_f) f_{n+1} + alpha_f f_n`, `a_m = (1 - alpha_m)
//! a_{n+1} + alpha_m a_n`, `v_f = (1 - alpha_f) v_{n+1} + alpha_f v_n` (the internal force averaged at its end values, so that
//! `rho_inf = 1` is the trapezoidal rule and the linear case is the equation of ADR 0389). The equation divided by `1 -
//! alpha_f` is solved by one attempt of the Newton driver ([`nonlinear::solve_from`], no second loop) with the inertia as a
//! driver contribution in the residual reference. A step that fails is taken in `2^l` equal sub-steps, `l = 1 ..
//! max_halvings` (D4); after the last level the error names the step and its times. The state ([`NlTransientState`]: time,
//! step, `u`, `v`, `a`, the internal force and the material states) is saved and restored bit for bit. Each attempt is made
//! at the point context of its sub-step, its end time and length ([`nonlinear::solve_from_at`], ADR 0485 D5), so that the
//! viscoelastic material of [`crate::mat_visco`] relaxes over the step. A uniform temperature
//! ([`NlTransientOptions::temperature`], ADR 0499 D3) enters that context, the start and the strain energy, and so reaches the
//! materials that read it (the time-temperature shift of the viscoelastic material, the base of a tabulated material); without
//! one no temperature is given. A uniform temperature against time ([`NlTransientOptions::temperature_history`], ADR 0526
//! D3) is a piecewise linear table taken at the end time of every sub-step (and at the start, and at the time of the state
//! for the strain energy); the stiffness part of the Rayleigh damping then takes its tangent at that temperature (ADR 0526
//! D4; with the constant temperature at that one). A nodal temperature field against time
//! ([`NlTransientOptions::temperature_field`], ADR 0534 D5) is taken at the same times with the thermal strain of the
//! materials' expansion (the multiplicative stretch of [`crate::thermal::TemperatureFieldHistory`]); the damping tangent is
//! then taken at the field's temperatures without the thermal strain.
//!
//! The energy-momentum step ([`StepKind::EnergyMomentum`], ADR 0514, Simo and Tarnow) is opt-in: the midpoint step with the
//! internal force `B_NL(F_{n+1/2})^T (S_n + S_{n+1})/2`, which conserves the energy and the linear and angular momenta of an
//! isotropic St. Venant-Kirchhoff solid up to the Newton residual. The default [`StepKind::Method`] is the step above,
//! unchanged. With the energy-momentum step a neo-Hookean material is taken and its algorithmic stress is the discrete
//! gradient of Gonzalez ([`AlgorithmicStress`], ADR 0523), which conserves the energy of any stored energy.
//!
//! The reference of the relative residual of the `Method` step is the driver's with the inertia force (ADR 0479 D2); the
//! opt-in [`ResidualReference::InertiaTerms`] (ADR 0603) also takes the largest norm of the products the inertia and damping
//! force is a sum of, so that a converged step of a free vibration is not rejected at the rounding floor of the internal
//! force near a zero crossing (the test is then a relative tolerance on the step's displacement).
//!
//! Shell4 and Shell3 (ADR 0522, F-585) take the corotated element of [`crate::shell_corot`] (ADR 0490) with the constant
//! consistent shell mass on the additive unknowns: the translations exactly, the rotations with the rotary inertia of the rate
//! of the rotation vector (`rho t^3/12`, isotropic; exact at small rotation, no gyroscopic terms). Their strain energy is the
//! potential of the corotated element; [`Integrator::momentum`] gives the translational momenta only.

use crate::constraint::{prescribed_equations, reduce};
use crate::element::{element_for, ElementState};
use crate::error::{FemError, Result};
use crate::mass::consistent_with;
use crate::material::PointContext;
use crate::mesh::ElementKind;
use crate::model::{Load, Material, SolidModel};
use crate::nonlinear::{self, Contribution, NewtonOptions};
use crate::solve::PIVOT_TOLERANCE;
use crate::thermal::{FieldAt, TemperatureFieldHistory};
use crate::transient::{LoadHistory, LoadTable, Method, Rayleigh};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;
use std::borrow::Cow;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The solver name of the errors of the time step (ADR 0479 D4).
pub const SOLVER_NAME: &str = "nl_transient";

const MAGIC: &[u8; 8] = b"RCFDNLT1";
/// The magic of a state of the energy-momentum step (ADR 0514 D5).
const MAGIC_EMC: &[u8; 8] = b"RCFDNLE1";

mod dg;
mod emc;

pub use dg::AlgorithmicStress;

/// The step of the nonlinear integrator (ADR 0514 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum StepKind {
    /// The Newmark or generalised-alpha step of the method (ADR 0479 D1), the default.
    #[default]
    Method,
    /// The energy-momentum conserving midpoint step of Simo and Tarnow (ADR 0514 D1): isotropic elastic solids of the
    /// `Full` formulation only; the method's parameters are not used by the step.
    EnergyMomentum,
}

/// The reference of the relative residual of each Newton attempt of the `Method` step (ADR 0603 D1, D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ResidualReference {
    /// The driver's reference with the inertia force of ADR 0479 D2, the default.
    #[default]
    Driver,
    /// Also at least the largest norm of the products the inertia and damping force is a sum of (ADR 0603 D1): `c_m M
    /// u_{n+1}`, `c_m M u_n`, `c_m h M v_n`, `c_m h^2 |1/2 - beta| M a_n`, `alpha_m/(1 - alpha_f) M a_n`, `alpha_f/(1 -
    /// alpha_f) f_int,n` and the damping products, `c_m = (1 - alpha_m)/((1 - alpha_f) beta h^2)`; so that a converged step
    /// of a free vibration is not rejected at the rounding floor of the internal force near a zero crossing. Refused with
    /// the energy-momentum step.
    InertiaTerms,
}

/// Options of the nonlinear integrator.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NlTransientOptions {
    /// The method (Newmark with `beta > 0`, or generalised-alpha).
    pub method: Method,
    /// The time step `dt`, s (finite, `> 0`).
    pub time_step: f64,
    /// Rayleigh damping (`None`: undamped); the stiffness part with the tangent at `u = 0`.
    pub damping: Option<Rayleigh>,
    /// The Newton options of each step: iterations, tolerances, threads; `max_halvings` are the halvings of the time step
    /// (ADR 0479 D4); `steps` is not used.
    pub newton: NewtonOptions,
    /// A uniform temperature of every point, K (ADR 0499 D3; `None`: no temperature in the point context).
    pub temperature: Option<f64>,
    /// A uniform temperature against time, K (ADR 0526 D3): a piecewise linear table taken at the end time of every
    /// sub-step (`None`: none; refused together with `temperature`).
    pub temperature_history: Option<LoadTable>,
    /// A nodal temperature field against time with the thermal strain of the materials' expansion (ADR 0534 D5): taken at
    /// the end time of every sub-step (`None`: none; refused together with `temperature` or `temperature_history`, with the
    /// energy-momentum step and with the models of ADR 0534 D4).
    pub temperature_field: Option<TemperatureFieldHistory>,
    /// The step (ADR 0514 D3; default [`StepKind::Method`]).
    pub step: StepKind,
    /// The algorithmic stress of the energy-momentum step (ADR 0523 D4; default [`AlgorithmicStress::Auto`]); not read by
    /// the `Method` step.
    pub stress: AlgorithmicStress,
    /// The reference of the relative residual (ADR 0603 D2; default [`ResidualReference::Driver`]).
    pub residual_reference: ResidualReference,
}

impl NlTransientOptions {
    /// The average-acceleration method with a time step, undamped, the driver defaults (20 iterations, tolerances `1e-10`,
    /// 5 halvings).
    pub fn new(time_step: f64) -> NlTransientOptions {
        NlTransientOptions {
            method: Method::average_acceleration(),
            time_step,
            damping: None,
            newton: NewtonOptions::new(1),
            temperature: None,
            temperature_history: None,
            temperature_field: None,
            step: StepKind::Method,
            stress: AlgorithmicStress::Auto,
            residual_reference: ResidualReference::Driver,
        }
    }
    /// The same options with a method.
    pub fn method(mut self, method: Method) -> NlTransientOptions {
        self.method = method;
        self
    }
    /// The same options with Rayleigh damping.
    pub fn damping(mut self, damping: Rayleigh) -> NlTransientOptions {
        self.damping = Some(damping);
        self
    }
    /// The same options with other Newton options.
    pub fn newton(mut self, newton: NewtonOptions) -> NlTransientOptions {
        self.newton = newton;
        self
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> NlTransientOptions {
        self.newton = self.newton.threads(threads);
        self
    }
    /// The same options with a uniform temperature of every point, K (ADR 0499 D3).
    pub fn temperature(mut self, temperature: f64) -> NlTransientOptions {
        self.temperature = Some(temperature);
        self
    }
    /// The same options with a uniform temperature against time, K (ADR 0526 D3), taken at the end time of every sub-step.
    pub fn temperature_history(mut self, table: LoadTable) -> NlTransientOptions {
        self.temperature_history = Some(table);
        self
    }
    /// The same options with a nodal temperature field against time and its thermal strain (ADR 0534 D5), taken at the end
    /// time of every sub-step.
    pub fn temperature_field(mut self, field: TemperatureFieldHistory) -> NlTransientOptions {
        self.temperature_field = Some(field);
        self
    }
    /// The same options with the energy-momentum step (ADR 0514 D3).
    pub fn energy_momentum(mut self) -> NlTransientOptions {
        self.step = StepKind::EnergyMomentum;
        self
    }
    /// The same options with a step kind (ADR 0514 D3).
    pub fn step_kind(mut self, step: StepKind) -> NlTransientOptions {
        self.step = step;
        self
    }
    /// The same options with an algorithmic stress of the energy-momentum step (ADR 0523 D4).
    pub fn algorithmic_stress(mut self, stress: AlgorithmicStress) -> NlTransientOptions {
        self.stress = stress;
        self
    }
    /// The same options with a reference of the relative residual (ADR 0603 D2).
    pub fn residual_reference(mut self, reference: ResidualReference) -> NlTransientOptions {
        self.residual_reference = reference;
        self
    }
}

/// The state of the nonlinear integrator (full vectors: one value per equation of the model's `DofMap`).
#[derive(Debug, Clone, PartialEq)]
pub struct NlTransientState {
    /// The start time `t0`; the time of the state is `t0 + step dt`.
    pub t0: f64,
    /// The step index `n`.
    pub step: u64,
    /// Displacement.
    pub u: Vec<f64>,
    /// Velocity.
    pub v: Vec<f64>,
    /// Acceleration.
    pub a: Vec<f64>,
    /// The internal force at `u` with the committed states.
    pub internal_force: Vec<f64>,
    /// The committed element states (the material history).
    pub states: Vec<ElementState>,
    dt: f64,
    parameters: [f64; 4],
    kind: StepKind,
}

fn state_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "state", reason }
}

impl NlTransientState {
    /// The time step the state was made with.
    pub fn time_step(&self) -> f64 {
        self.dt
    }

    /// The step the state was made with (ADR 0514 D5).
    pub fn step_kind(&self) -> StepKind {
        self.kind
    }

    /// The bytes of the state (ADR 0479 D3): magic `RCFDNLT1`, little-endian `u64` length, step and element count, the
    /// `f64` bits of `t0`, `dt`, the four parameters `(alpha_m, alpha_f, beta, gamma)`, `u`, `v`, `a`, the internal force,
    /// then per element its value count (`u64`) and its values. A state of the energy-momentum step has the magic
    /// `RCFDNLE1` and the same layout (ADR 0514 D5).
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.u.len();
        let mut b = Vec::with_capacity(8 * (10 + 4 * n));
        b.extend_from_slice(match self.kind {
            StepKind::Method => MAGIC,
            StepKind::EnergyMomentum => MAGIC_EMC,
        });
        b.extend_from_slice(&(n as u64).to_le_bytes());
        b.extend_from_slice(&self.step.to_le_bytes());
        b.extend_from_slice(&(self.states.len() as u64).to_le_bytes());
        for x in [self.t0, self.dt]
            .iter()
            .chain(&self.parameters)
            .chain(&self.u)
            .chain(&self.v)
            .chain(&self.a)
            .chain(&self.internal_force)
        {
            b.extend_from_slice(&x.to_bits().to_le_bytes());
        }
        for s in &self.states {
            b.extend_from_slice(&(s.values().len() as u64).to_le_bytes());
            for x in s.values() {
                b.extend_from_slice(&x.to_bits().to_le_bytes());
            }
        }
        b
    }

    /// The state from [`NlTransientState::to_bytes`]; refuses a wrong magic and a wrong or inconsistent length.
    pub fn from_bytes(bytes: &[u8]) -> Result<NlTransientState> {
        if bytes.len() < 32 || (&bytes[..8] != MAGIC && &bytes[..8] != MAGIC_EMC) || bytes.len() % 8 != 0 {
            return Err(state_error(
                "not a saved nonlinear transient state (magic RCFDNLT1 or RCFDNLE1 missing or a truncated byte string)"
                    .to_string(),
            ));
        }
        let kind = if &bytes[..8] == MAGIC_EMC {
            StepKind::EnergyMomentum
        } else {
            StepKind::Method
        };
        let words = bytes.len() / 8;
        let word = |i: usize| -> u64 {
            let mut w = [0u8; 8];
            w.copy_from_slice(&bytes[8 * i..8 * i + 8]);
            u64::from_le_bytes(w)
        };
        let n = word(1);
        let step = word(2);
        let ne = word(3);
        let head = 4u128 + 6 + 4 * n as u128;
        if (words as u128) < head + ne as u128 {
            return Err(state_error(format!(
                "{} bytes for a state of {n} unknowns and {ne} elements",
                bytes.len()
            )));
        }
        let n = n as usize;
        let f = |i: usize| f64::from_bits(word(i));
        let vec_at = |start: usize, len: usize| (0..len).map(|k| f(start + k)).collect::<Vec<f64>>();
        let mut at = head as usize;
        let mut states = Vec::with_capacity(ne as usize);
        for e in 0..ne as usize {
            if at >= words {
                return Err(state_error(format!("the byte string ends before element {e}")));
            }
            let len = word(at) as u128;
            if (at as u128) + 1 + len > words as u128 {
                return Err(state_error(format!(
                    "element {e}: {len} values beyond the end of the byte string"
                )));
            }
            let len = len as usize;
            states.push(ElementState::new(vec_at(at + 1, len)));
            at += 1 + len;
        }
        if at != words {
            return Err(state_error(format!(
                "{} bytes for a state that ends at byte {}",
                bytes.len(),
                8 * at
            )));
        }
        Ok(NlTransientState {
            t0: f(4),
            dt: f(5),
            parameters: [f(6), f(7), f(8), f(9)],
            step,
            u: vec_at(10, n),
            v: vec_at(10 + n, n),
            a: vec_at(10 + 2 * n, n),
            internal_force: vec_at(10 + 3 * n, n),
            states,
            kind,
        })
    }
}

/// What one converged step reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    /// The halvings of the time step before the step converged (its sub-steps are `2^halvings`).
    pub halvings: usize,
    /// Newton corrections of all its sub-steps.
    pub iterations: usize,
    /// The relative residual at convergence of its last sub-step.
    pub residual: f64,
}

/// The time integrator of a nonlinear solid model (ADR 0479).
#[derive(Debug, Clone)]
pub struct Integrator {
    model: SolidModel,
    m: CsrMatrix,
    c: Option<CsrMatrix>,
    loads: Vec<(Vec<f64>, LoadTable)>,
    fixed: Vec<(usize, f64)>,
    free: Vec<usize>,
    parameters: [f64; 4],
    dt: f64,
    newton: NewtonOptions,
    layout: Vec<usize>,
    history: bool,
    temperature: Option<f64>,
    /// The temperature against time (ADR 0526 D3).
    temperature_history: Option<LoadTable>,
    /// The Rayleigh coefficients (ADR 0526 D4: the damping is re-formed at another temperature of the history).
    damping: Rayleigh,
    /// The temperature at which `c` was formed (ADR 0526 D4).
    c_temperature: Option<f64>,
    /// The nodal temperature field against time (ADR 0534 D5).
    temperature_field: Option<TemperatureFieldHistory>,
    /// The nodal temperatures at which `c` was formed with a field (ADR 0534 D5).
    c_field: Option<Vec<f64>>,
    step: StepKind,
    /// The algorithmic stress of the energy-momentum step (ADR 0523 D4).
    stress: AlgorithmicStress,
    /// A model with a neo-Hookean material in the energy-momentum step (ADR 0523 D5).
    hyper: bool,
    /// The reference of the relative residual (ADR 0603 D2).
    residual_reference: ResidualReference,
    state: Option<NlTransientState>,
}

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

/// `a M + b K` on the union of the patterns.
fn combine(a: f64, m: &CsrMatrix, b: f64, k: &CsrMatrix) -> CsrMatrix {
    let mut t = Vec::with_capacity(m.nnz() + k.nnz());
    for (c, x) in [(a, m), (b, k)] {
        for r in 0..x.n {
            for p in x.row_ptr[r]..x.row_ptr[r + 1] {
                t.push((r, x.col[p], c * x.val[p]));
            }
        }
    }
    CsrMatrix::from_triplets(m.n, t)
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Whether a failure of one driver attempt may be retried with a smaller step (ADR 0479 D4): the driver's own failure or a
/// located inversion.
fn retryable(e: &FemError) -> bool {
    match e {
        FemError::Solver { solver, .. } => *solver == "newton",
        FemError::Element { source, .. } => {
            matches!(source.as_ref(), FemError::InvalidValue { key, .. } if *key == nonlinear::INVERSION_KEY)
        }
        _ => false,
    }
}

/// The inertia and damping of one step as a driver contribution (ADR 0479 D1): force `(M a_m + C v_f + alpha_f f_int,n)/(1
/// - alpha_f)`, tangent `[(1 - alpha_m)/(beta h^2) M + (1 - alpha_f) gamma/(beta h) C]/(1 - alpha_f)`.
struct Inertia<'a> {
    m: &'a CsrMatrix,
    c: Option<&'a CsrMatrix>,
    kin: &'a Kinematics<'a>,
    fint_n: &'a [f64],
    /// The inertia-term reference of ADR 0603 D1 (`None`: the driver's reference).
    terms: Option<InertiaTerms>,
}

/// The products of ADR 0603 D1 that do not depend on the iterate: their largest norm and the factors of `M u_{n+1}` and
/// `C u_{n+1}`.
struct InertiaTerms {
    fixed: f64,
    cm: f64,
    cc: f64,
}

impl InertiaTerms {
    /// The products of the start of the sub-step (ADR 0603 D1).
    fn new(m: &CsrMatrix, c: Option<&CsrMatrix>, kin: &Kinematics<'_>, u_n: &[f64], fint_n: &[f64]) -> InertiaTerms {
        let [am, af, beta, gamma] = kin.parameters;
        let h = kin.h;
        let nrm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        let cm = (1.0 - am) / ((1.0 - af) * beta * h * h);
        let cc = gamma / (beta * h);
        let p = h * h * (0.5 - beta).abs();
        let (mu, mv, ma) = (nrm(&matvec(m, u_n)), nrm(&matvec(m, kin.v_n)), nrm(&matvec(m, kin.a_n)));
        let mut fixed = [
            cm * mu,
            cm * h * mv,
            cm * p * ma,
            am / (1.0 - af) * ma,
            af / (1.0 - af) * nrm(fint_n),
        ]
        .into_iter()
        .fold(0.0f64, f64::max);
        if let Some(c) = c {
            let (cu, cv, ca) = (nrm(&matvec(c, u_n)), nrm(&matvec(c, kin.v_n)), nrm(&matvec(c, kin.a_n)));
            fixed = [
                cv,
                h * (1.0 - gamma).abs() * ca,
                cc * cu,
                cc * h * cv,
                cc * p * ca,
                af / (1.0 - af) * cv,
            ]
            .into_iter()
            .fold(fixed, f64::max);
        }
        InertiaTerms { fixed, cm, cc }
    }
}

/// The step's kinematics: `a_{n+1}` and `v_{n+1}` of `u_{n+1}` (ADR 0479 D1).
struct Kinematics<'a> {
    ut: Vec<f64>,
    vt: Vec<f64>,
    a_n: &'a [f64],
    v_n: &'a [f64],
    parameters: [f64; 4],
    h: f64,
}

impl<'a> Kinematics<'a> {
    fn new(u: &[f64], v: &'a [f64], a: &'a [f64], parameters: [f64; 4], h: f64) -> Kinematics<'a> {
        let [_, _, beta, gamma] = parameters;
        let ut = (0..u.len())
            .map(|i| u[i] + h * v[i] + h * h * (0.5 - beta) * a[i])
            .collect();
        let vt = (0..u.len()).map(|i| v[i] + h * (1.0 - gamma) * a[i]).collect();
        Kinematics {
            ut,
            vt,
            a_n: a,
            v_n: v,
            parameters,
            h,
        }
    }

    /// `(a_{n+1}, v_{n+1})` of `u_{n+1}`.
    fn of(&self, u: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let [_, _, beta, gamma] = self.parameters;
        let h = self.h;
        let a: Vec<f64> = (0..u.len()).map(|i| (u[i] - self.ut[i]) / (beta * h * h)).collect();
        let v = (0..u.len()).map(|i| self.vt[i] + gamma * h * a[i]).collect();
        (a, v)
    }
}

impl Contribution for Inertia<'_> {
    fn name(&self) -> &str {
        "inertia"
    }

    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> Result<()> {
        let [am, af, beta, gamma] = self.kin.parameters;
        let h = self.kin.h;
        let (a1, v1) = self.kin.of(u);
        let n = u.len();
        let am_vec: Vec<f64> = (0..n).map(|i| (1.0 - am) * a1[i] + am * self.kin.a_n[i]).collect();
        let ma = matvec(self.m, &am_vec);
        let cv = match self.c {
            Some(c) => {
                let vf: Vec<f64> = (0..n).map(|i| (1.0 - af) * v1[i] + af * self.kin.v_n[i]).collect();
                matvec(c, &vf)
            }
            None => vec![0.0; n],
        };
        for i in 0..n {
            force[i] += (ma[i] + cv[i] + af * self.fint_n[i]) / (1.0 - af);
        }
        let cm = (1.0 - am) / ((1.0 - af) * beta * h * h);
        for r in 0..self.m.n {
            for p in self.m.row_ptr[r]..self.m.row_ptr[r + 1] {
                tangent.push((r, self.m.col[p], cm * self.m.val[p]));
            }
        }
        if let Some(c) = self.c {
            let cc = gamma / (beta * h);
            for r in 0..c.n {
                for p in c.row_ptr[r]..c.row_ptr[r + 1] {
                    tangent.push((r, c.col[p], cc * c.val[p]));
                }
            }
        }
        Ok(())
    }

    fn in_residual_reference(&self) -> bool {
        true
    }

    fn residual_reference_terms(&self, u: &[f64], _load_factor: f64) -> f64 {
        let Some(t) = &self.terms else {
            return 0.0;
        };
        let nrm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mut r = t.fixed.max(t.cm * nrm(&matvec(self.m, u)));
        if let Some(c) = self.c {
            r = r.max(t.cc * nrm(&matvec(c, u)));
        }
        r
    }
}

/// The end of one sub-step.
struct Advanced {
    u: Vec<f64>,
    v: Vec<f64>,
    a: Vec<f64>,
    fint: Vec<f64>,
    states: Vec<ElementState>,
    iterations: usize,
    residual: f64,
}

impl Integrator {
    /// The integrator of a nonlinear solid model with its zero prescribed displacements, the load histories (full
    /// patterns) and the options (ADR 0479 D3). Refuses the transformation path and a temperature load (key
    /// `"nl_transient"`), a non-zero prescribed value (`"constraint"`), what the nonlinear driver refuses, a non-positive
    /// step (`"time step"`), `beta = 0` (`"method"`), invalid damping (`"damping"`) and a load pattern of the wrong length
    /// or with a non-finite value (`"load pattern"`), and a creep material with the Arrhenius factor without a temperature
    /// option or with a constant temperature or history point that is not positive and finite (`"temperature"`, ADR 0578 D2).
    pub fn new(model: &SolidModel, loads: &[LoadHistory], options: &NlTransientOptions) -> Result<Integrator> {
        let dt = options.time_step;
        if !(dt.is_finite() && dt > 0.0) {
            return Err(invalid("time step", format!("{dt:e} (must be finite and > 0)")));
        }
        // ADR 0523 D5 and amendment 1: a neo-Hookean model of the energy-momentum step with a stress other than `Auto` is
        // checked and given its mass and K_0 through the isotropic surrogate of its small-strain moduli, then holds its own
        // materials (with `Auto` it is refused as before).
        if options.step == StepKind::EnergyMomentum && options.stress != AlgorithmicStress::Auto {
            if let Some(surrogate) = dg::surrogate(model, options.stress)? {
                let mut it = Integrator::new(&surrogate, loads, options)?;
                it.model = model.clone();
                it.hyper = true;
                return Ok(it);
            }
        }
        // ADR 0603 D2: the inertia-term reference is a reference of the Method step's driver call only.
        if options.step == StepKind::EnergyMomentum && options.residual_reference != ResidualReference::Driver {
            return Err(invalid(
                "nl_transient",
                "the inertia-term residual reference with the energy-momentum step: the midpoint step has no inertia \
                 contribution (ADR 0603 D2)"
                    .to_string(),
            ));
        }
        model.validate()?;
        // ADR 0522 D4: Shell4 and Shell3 take the corotated element of ADR 0490 with the consistent shell mass (D2).
        model.check_supported_nonlinear_shells()?;
        if crate::mpc::uses_transformation(model) {
            return Err(invalid(
                "nl_transient",
                "linear constraints, rigid sets, cyclic constraints and elastic supports (the transformation path) are not \
                 offered by the nonlinear transient integrator (ADR 0479 D6)"
                    .to_string(),
            ));
        }
        if let Some(i) = model.loads().iter().position(|l| matches!(l, Load::Temperature { .. })) {
            return Err(invalid(
                "nl_transient",
                format!(
                    "load {i}: a temperature load is not offered by the nonlinear transient integrator (ADR 0479 D6)"
                ),
            ));
        }
        nonlinear::check_driver_model(model)?;
        // ADR 0555 D4: the Method step takes the hyperelastic material through the driver's functions (ADR 0541 D4
        // refused it here).
        if options.step == StepKind::EnergyMomentum {
            emc::check_model(model)?;
        }
        if let Some(t) = options.temperature {
            PointContext::new().with_temperature(t)?;
        }
        if options.temperature.is_some() && options.temperature_history.is_some() {
            return Err(invalid(
                "temperature",
                "a constant temperature and a temperature history together (ADR 0526 D1)".to_string(),
            ));
        }
        // ADR 0534 D4, D5: the field alone, on the Method step, and a model that takes it.
        if let Some(field) = &options.temperature_field {
            if options.temperature.is_some() || options.temperature_history.is_some() {
                return Err(invalid(
                    "temperature",
                    "a temperature field together with a constant temperature or a temperature history (ADR 0534 D5)"
                        .to_string(),
                ));
            }
            if options.step == StepKind::EnergyMomentum {
                return Err(invalid(
                    "temperature",
                    "a temperature field on the energy-momentum step: the midpoint step has no thermal strain (ADR 0534 D4)"
                        .to_string(),
                ));
            }
            crate::thermal::check_field_model(model, field)?;
        }
        // ADR 0578 D2: a creep material with the Arrhenius factor needs a positive temperature option, refused before any step.
        crate::mat_creep::check_factor_temperature(
            model,
            options.temperature,
            options.temperature_history.as_ref(),
            options.temperature_field.is_some(),
        )?;
        let parameters = options.method.parameters()?;
        if parameters[2] <= 0.0 {
            return Err(invalid(
                "method",
                format!(
                    "Newmark beta {:e}: the nonlinear step solves for the displacement and needs beta > 0",
                    parameters[2]
                ),
            ));
        }
        let damping = options.damping.unwrap_or(Rayleigh {
            mass: 0.0,
            stiffness: 0.0,
        });
        let ok = |x: f64| x.is_finite() && x >= 0.0;
        if !(ok(damping.mass) && ok(damping.stiffness)) {
            return Err(invalid(
                "damping",
                format!(
                    "Rayleigh coefficients {:e} (mass) and {:e} (stiffness) must be finite and >= 0",
                    damping.mass, damping.stiffness
                ),
            ));
        }
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        let fixed = prescribed_equations(model, &[])?;
        if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
            let (node, component) = dofs.node_component(eq).expect("existing equation");
            return Err(invalid(
                "constraint",
                format!(
                    "prescribed value {v:e} at node {node} component {component}: the nonlinear transient integrator takes \
                     zero values only"
                ),
            ));
        }
        let mut is_fixed = vec![false; n];
        for &(eq, _) in &fixed {
            is_fixed[eq] = true;
        }
        let free = (0..n).filter(|&i| !is_fixed[i]).collect();
        let mut table = Vec::with_capacity(loads.len());
        for l in loads {
            if l.pattern.len() != n {
                return Err(FemError::FieldLength {
                    key: "load pattern",
                    expected: n,
                    found: l.pattern.len(),
                });
            }
            if let Some(i) = l.pattern.iter().position(|x| !x.is_finite()) {
                return Err(invalid(
                    "load pattern",
                    format!("value {:e} at equation {i} is not finite", l.pattern[i]),
                ));
            }
            table.push((l.pattern.clone(), l.table.clone()));
        }
        let threads = options.newton.threads;
        let m = consistent_with(model, threads)?;
        let initial = nonlinear::initial_states(model)?;
        // ADR 0526 D4: the stiffness part at the temperature (the constant one, or the history's first value).
        let c_temperature = match &options.temperature_history {
            Some(table) => Some(table.points()[0].1),
            None => options.temperature,
        };
        // ADR 0534 D5: with a field, K_0 at its first values without the thermal strain.
        let c_field = options
            .temperature_field
            .as_ref()
            .map(|f| FieldAt::of(f, f.range().0, false));
        let c = if damping.mass != 0.0 || damping.stiffness != 0.0 {
            let k0 = match c_temperature {
                _ if c_field.is_some() => nonlinear::tangent_field(
                    model,
                    &vec![0.0; n],
                    &initial,
                    threads,
                    &PointContext::default(),
                    c_field.as_ref(),
                )?,
                None => nonlinear::tangent(model, &vec![0.0; n], &initial, threads)?,
                Some(t) => nonlinear::tangent_at(
                    model,
                    &vec![0.0; n],
                    &initial,
                    threads,
                    &PointContext::new().with_temperature(t)?,
                )?,
            };
            Some(combine(damping.mass, &m, damping.stiffness, &k0))
        } else {
            None
        };
        let layout: Vec<usize> = initial.iter().map(|s| s.values().len()).collect();
        let history = layout.iter().any(|&k| k > 0);
        Ok(Integrator {
            model: model.clone(),
            m,
            c,
            loads: table,
            fixed,
            free,
            parameters,
            dt,
            newton: options.newton.clone(),
            layout,
            history,
            temperature: options.temperature,
            temperature_history: options.temperature_history.clone(),
            damping,
            c_temperature,
            temperature_field: options.temperature_field.clone(),
            c_field: c_field.map(|f| f.nodal),
            step: options.step,
            stress: options.stress,
            hyper: false,
            residual_reference: options.residual_reference,
            state: None,
        })
    }

    /// The parameters `(alpha_m, alpha_f, beta, gamma)`.
    pub fn parameters(&self) -> [f64; 4] {
        self.parameters
    }

    /// The time step.
    pub fn time_step(&self) -> f64 {
        self.dt
    }

    /// The step kind (ADR 0514 D3).
    pub fn step_kind(&self) -> StepKind {
        self.step
    }

    /// The consistent mass matrix (all equations).
    pub fn mass(&self) -> &CsrMatrix {
        &self.m
    }

    fn check_coverage(&self, t_a: f64, t_b: f64) -> Result<()> {
        if let Some(field) = &self.temperature_field {
            if !field.covers(t_a, t_b) {
                let (lo, hi) = field.range();
                return Err(invalid(
                    "temperature",
                    format!(
                        "the temperature field covers [{lo:e}, {hi:e}], the run needs [{t_a:e}, {t_b:e}] (ADR 0534 D5)"
                    ),
                ));
            }
        }
        if let Some(table) = &self.temperature_history {
            if !table.covers(t_a, t_b) {
                let (lo, hi) = table.range();
                return Err(invalid(
                    "temperature",
                    format!(
                        "the temperature history covers [{lo:e}, {hi:e}], the run needs [{t_a:e}, {t_b:e}] (ADR 0526 D3)"
                    ),
                ));
            }
        }
        for (j, (_, table)) in self.loads.iter().enumerate() {
            if !table.covers(t_a, t_b) {
                let (lo, hi) = table.range();
                return Err(invalid(
                    "load table",
                    format!("table {j} covers [{lo:e}, {hi:e}], the run needs [{t_a:e}, {t_b:e}]"),
                ));
            }
        }
        Ok(())
    }

    /// The load at `t` (the tables cover `t`), all equations.
    fn load(&self, t: f64) -> Vec<f64> {
        let mut f = vec![0.0; self.m.n];
        for (pattern, table) in &self.loads {
            let s = table.value(t).expect("coverage checked");
            for (fi, pi) in f.iter_mut().zip(pattern) {
                *fi += s * pi;
            }
        }
        f
    }

    /// The temperature of the points at `t` (ADR 0526 D3): the history's value (the table covers `t`), else the constant
    /// temperature, else none.
    fn temperature_at(&self, t: f64) -> Option<f64> {
        match &self.temperature_history {
            Some(table) => Some(table.value(t).expect("temperature coverage checked")),
            None => self.temperature,
        }
    }

    /// The damping matrix at the temperature of `t` (ADR 0526 D4): the stored one, unless a temperature history gives
    /// another temperature and the damping has a stiffness part; then `alpha M + beta K_0` at `u = 0`, the empty states and
    /// that temperature. The neo-Hookean integrator keeps the stored one (its surrogate's `K_0` does not read the
    /// temperature).
    fn damping_at(&self, t: f64) -> Result<Option<Cow<'_, CsrMatrix>>> {
        let Some(c) = &self.c else {
            return Ok(None);
        };
        // ADR 0534 D5: with a field, re-formed at nodal temperatures other than those of the stored matrix.
        if let Some(field) = &self.temperature_field {
            if self.damping.stiffness == 0.0 {
                return Ok(Some(Cow::Borrowed(c)));
            }
            let at = FieldAt::of(field, t, false);
            let stored = self.c_field.as_deref().expect("formed at the field");
            if at.nodal.iter().zip(stored).all(|(a, b)| a.to_bits() == b.to_bits()) {
                return Ok(Some(Cow::Borrowed(c)));
            }
            let n = self.m.n;
            let initial = nonlinear::initial_states(&self.model)?;
            let k0 = nonlinear::tangent_field(
                &self.model,
                &vec![0.0; n],
                &initial,
                self.newton.threads,
                &PointContext::default(),
                Some(&at),
            )?;
            return Ok(Some(Cow::Owned(combine(
                self.damping.mass,
                &self.m,
                self.damping.stiffness,
                &k0,
            ))));
        }
        if self.temperature_history.is_none() || self.damping.stiffness == 0.0 || self.hyper {
            return Ok(Some(Cow::Borrowed(c)));
        }
        let temperature = self.temperature_at(t).expect("a history gives a temperature");
        if self.c_temperature.map(f64::to_bits) == Some(temperature.to_bits()) {
            return Ok(Some(Cow::Borrowed(c)));
        }
        let n = self.m.n;
        let initial = nonlinear::initial_states(&self.model)?;
        let context = PointContext::new().with_temperature(temperature)?;
        let k0 = nonlinear::tangent_at(&self.model, &vec![0.0; n], &initial, self.newton.threads, &context)?;
        Ok(Some(Cow::Owned(combine(
            self.damping.mass,
            &self.m,
            self.damping.stiffness,
            &k0,
        ))))
    }

    fn damping_force(c: Option<&CsrMatrix>, v: &[f64]) -> Vec<f64> {
        match c {
            Some(c) => matvec(c, v),
            None => vec![0.0; v.len()],
        }
    }

    /// Starts at `t0` from the full displacement and velocity (0 at the prescribed equations): empty material states, the
    /// internal force at `u0`, the acceleration from `M a0 = f(t0) - C v0 - f_int(u0)` on the free equations. A model with
    /// a material with history starts from `u0 = 0` only (key `"initial state"`).
    pub fn start(&mut self, t0: f64, u0: &[f64], v0: &[f64]) -> Result<()> {
        if !t0.is_finite() {
            return Err(invalid("initial state", format!("start time {t0:e} is not finite")));
        }
        let n = self.m.n;
        for (key, x) in [("initial displacement", u0), ("initial velocity", v0)] {
            if x.len() != n {
                return Err(FemError::FieldLength {
                    key,
                    expected: n,
                    found: x.len(),
                });
            }
            if let Some(e) = x.iter().position(|v| !v.is_finite()) {
                return Err(invalid(
                    "initial state",
                    format!("{key} {:e} at equation {e} is not finite", x[e]),
                ));
            }
            if let Some(&(eq, _)) = self.fixed.iter().find(|&&(eq, _)| x[eq] != 0.0) {
                return Err(invalid(
                    "initial state",
                    format!("{key} {:e} at the prescribed equation {eq} (must be 0)", x[eq]),
                ));
            }
        }
        if self.history && u0.iter().any(|&x| x != 0.0) {
            return Err(invalid(
                "initial state",
                "a model with a material with history starts from a zero displacement (ADR 0479 D3)".to_string(),
            ));
        }
        self.check_coverage(t0, t0)?;
        // ADR 0523 amendment 2: a neo-Hookean integrator takes the empty states of its surrogate's layout.
        let states = if self.hyper {
            self.layout.iter().map(|&k| ElementState::new(vec![0.0; k])).collect()
        } else {
            nonlinear::initial_states(&self.model)?
        };
        let fint = match self.temperature_at(t0) {
            // ADR 0523 D5: the neo-Hookean internal force of the energy-momentum step.
            _ if self.hyper => emc::internal_force(&self.model, u0, &PointContext::default(), self.stress)?,
            // ADR 0534 D5: the start at the field with its thermal strain.
            _ if self.temperature_field.is_some() => {
                let field = FieldAt::of(self.temperature_field.as_ref().expect("checked"), t0, true);
                nonlinear::internal_force_field(&self.model, u0, &states, &PointContext::default(), Some(&field))?
            }
            None => nonlinear::internal_force(&self.model, u0, &states)?,
            // ADR 0499 D3: the start at the temperature (a shifted material needs it).
            Some(t) => {
                nonlinear::internal_force_at(&self.model, u0, &states, &PointContext::new().with_temperature(t)?)?
            }
        };
        let f = self.load(t0);
        let c0 = self.damping_at(t0)?;
        let cv = Integrator::damping_force(c0.as_deref(), v0);
        let rhs: Vec<f64> = (0..n).map(|i| f[i] - cv[i] - fint[i]).collect();
        let sys = reduce(&self.m, &rhs, &self.fixed)?;
        let mut a = vec![0.0; n];
        if !sys.free().is_empty() {
            let x = factor(&sys.matrix, "mass matrix")?.solve(&sys.rhs);
            for (&eq, xi) in sys.free().iter().zip(x) {
                a[eq] = xi;
            }
        }
        self.state = Some(NlTransientState {
            t0,
            step: 0,
            u: u0.to_vec(),
            v: v0.to_vec(),
            a,
            internal_force: fint,
            states,
            dt: self.dt,
            parameters: self.parameters,
            kind: self.step,
        });
        Ok(())
    }

    fn current(&self) -> Result<&NlTransientState> {
        self.state
            .as_ref()
            .ok_or_else(|| state_error("the integrator has no state (call start or restore first)".to_string()))
    }

    /// The state (a clone).
    pub fn state(&self) -> Result<NlTransientState> {
        self.current().cloned()
    }

    /// Replaces the state; refuses another length, another element layout, another time step or another method (bits).
    pub fn restore(&mut self, state: NlTransientState) -> Result<()> {
        let n = self.m.n;
        for (name, x) in [
            ("u", &state.u),
            ("v", &state.v),
            ("a", &state.a),
            ("internal force", &state.internal_force),
        ] {
            if x.len() != n {
                return Err(state_error(format!(
                    "{name} of {} values for an integrator of {n} equations",
                    x.len()
                )));
            }
        }
        if state.states.len() != self.layout.len()
            || state
                .states
                .iter()
                .zip(&self.layout)
                .any(|(s, &k)| s.values().len() != k)
        {
            return Err(state_error(format!(
                "material states of {} elements that do not match the model's layout ({} elements)",
                state.states.len(),
                self.layout.len()
            )));
        }
        if state.dt.to_bits() != self.dt.to_bits() {
            return Err(state_error(format!(
                "state of time step {:e} for an integrator of time step {:e}",
                state.dt, self.dt
            )));
        }
        if state
            .parameters
            .iter()
            .zip(&self.parameters)
            .any(|(a, b)| a.to_bits() != b.to_bits())
        {
            return Err(state_error(format!(
                "state of the method parameters {:?} for an integrator of {:?}",
                state.parameters, self.parameters
            )));
        }
        if !state.t0.is_finite() {
            return Err(state_error(format!("start time {:e} is not finite", state.t0)));
        }
        if state.kind != self.step {
            return Err(state_error(format!(
                "state of the step {:?} for an integrator of the step {:?} (ADR 0514 D5)",
                state.kind, self.step
            )));
        }
        self.state = Some(state);
        Ok(())
    }

    fn time_of(&self, t0: f64, step: u64) -> f64 {
        t0 + step as f64 * self.dt
    }

    /// The time of the state.
    pub fn time(&self) -> Result<f64> {
        let s = self.current()?;
        Ok(self.time_of(s.t0, s.step))
    }

    /// Advances `steps` steps; refuses (before any step) a table that does not cover the run.
    pub fn run(&mut self, steps: u64) -> Result<Vec<StepReport>> {
        self.run_with(steps, &[])
    }

    /// Advances one step.
    pub fn step(&mut self) -> Result<StepReport> {
        Ok(self.run_with(1, &[])?[0])
    }

    /// Advances one step with extra driver contributions in every Newton iteration (after the inertia).
    pub fn step_with(&mut self, contributions: &[&dyn Contribution]) -> Result<StepReport> {
        Ok(self.run_with(1, contributions)?[0])
    }

    /// Advances `steps` steps with extra driver contributions; refuses (before any step) a table that does not cover the
    /// run. A step that does not converge after the halvings returns the error with its times and leaves the state of
    /// the last converged step.
    pub fn run_with(&mut self, steps: u64, contributions: &[&dyn Contribution]) -> Result<Vec<StepReport>> {
        let (t0, n) = {
            let s = self.current()?;
            (s.t0, s.step)
        };
        if self.step == StepKind::EnergyMomentum && !contributions.is_empty() {
            return Err(invalid(
                "nl_transient",
                format!(
                    "{} driver contributions: the energy-momentum step has no midpoint form of a contribution ({})",
                    contributions.len(),
                    emc::ADR
                ),
            ));
        }
        self.check_coverage(self.time_of(t0, n), self.time_of(t0, n + steps))?;
        let mut reports = Vec::with_capacity(steps as usize);
        for _ in 0..steps {
            reports.push(self.advance(contributions)?);
        }
        Ok(reports)
    }

    /// One step `n -> n+1` with the halvings of ADR 0479 D4.
    fn advance(&mut self, contributions: &[&dyn Contribution]) -> Result<StepReport> {
        let committed = self.state.as_ref().expect("state checked");
        let n = committed.step;
        let t_n = self.time_of(committed.t0, n);
        let t_end = self.time_of(committed.t0, n + 1);
        let max = self.newton.max_halvings;
        let mut last = String::new();
        for level in 0..=max {
            let parts = 1u64 << level;
            let h = self.dt / parts as f64;
            let mut u = committed.u.clone();
            let mut v = committed.v.clone();
            let mut a = committed.a.clone();
            let mut fint = committed.internal_force.clone();
            let mut states = committed.states.clone();
            let mut iterations = 0;
            let mut residual = 0.0;
            let mut failed = false;
            for k in 0..parts {
                let t_a = t_n + k as f64 * h;
                let t_b = if k + 1 == parts {
                    t_end
                } else {
                    t_n + (k + 1) as f64 * h
                };
                match self.substep(h, t_a, t_b, &u, &v, &a, &fint, &states, contributions) {
                    Ok(x) => {
                        u = x.u;
                        v = x.v;
                        a = x.a;
                        fint = x.fint;
                        states = x.states;
                        iterations += x.iterations;
                        residual = x.residual;
                    }
                    Err(e) if retryable(&e) => {
                        last = e.to_string();
                        failed = true;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
            if !failed {
                let s = self.state.as_mut().expect("state checked");
                s.u = u;
                s.v = v;
                s.a = a;
                s.internal_force = fint;
                s.states = states;
                s.step = n + 1;
                return Ok(StepReport {
                    halvings: level,
                    iterations,
                    residual,
                });
            }
        }
        Err(FemError::Solver {
            solver: SOLVER_NAME,
            reason: format!(
                "step {} from t = {t_n:e} to t = {t_end:e} did not converge after {max} halvings of the time step {:e}: \
                 {last}",
                n + 1,
                self.dt
            ),
        })
    }

    /// One driver attempt of size `h` from `t_a` to `t_b` (ADR 0479 D1).
    #[allow(clippy::too_many_arguments)]
    fn substep(
        &self,
        h: f64,
        t_a: f64,
        t_b: f64,
        u: &[f64],
        v: &[f64],
        a: &[f64],
        fint: &[f64],
        states: &[ElementState],
        contributions: &[&dyn Contribution],
    ) -> Result<Advanced> {
        if self.step == StepKind::EnergyMomentum {
            return emc::substep(self, h, t_a, t_b, u, v, states);
        }
        let [_, af, _, _] = self.parameters;
        let f0 = self.load(t_a);
        let f1 = self.load(t_b);
        let load: Vec<f64> = (0..f0.len())
            .map(|i| ((1.0 - af) * f1[i] + af * f0[i]) / (1.0 - af))
            .collect();
        let kin = Kinematics::new(u, v, a, self.parameters, h);
        // ADR 0526 D4: the damping at the temperature of the sub-step's end.
        let c = self.damping_at(t_b)?;
        // ADR 0603 D1: the inertia-term reference, opt-in.
        let terms = match self.residual_reference {
            ResidualReference::Driver => None,
            ResidualReference::InertiaTerms => Some(InertiaTerms::new(&self.m, c.as_deref(), &kin, u, fint)),
        };
        let inertia = Inertia {
            m: &self.m,
            c: c.as_deref(),
            kin: &kin,
            fint_n: fint,
            terms,
        };
        let mut list: Vec<&dyn Contribution> = Vec::with_capacity(1 + contributions.len());
        list.push(&inertia);
        list.extend_from_slice(contributions);
        // ADR 0485 D5: the point context of the sub-step (its end time and length), read by the viscoelastic material only.
        let mut context = PointContext::new().with_time(t_b, h)?;
        // ADR 0526 D3: the temperature at the sub-step's end.
        if let Some(t) = self.temperature_at(t_b) {
            context = context.with_temperature(t)?;
        }
        // ADR 0534 D5: the nodal field at the sub-step's end with its thermal strain.
        let field = self.temperature_field.as_ref().map(|f| FieldAt::of(f, t_b, true));
        // ADR 0539 D6: with the thermal predictor the Newton start is the committed displacement plus the free expansion
        // increment of the change of the field from `t_a` to `t_b`.
        let start: Cow<[f64]> = match self.temperature_field.as_ref().filter(|f| f.thermal_predictor()) {
            Some(f) => {
                let old = FieldAt::of(f, t_a, true).nodal;
                let new = field.as_ref().expect("field of the sub-step").nodal.clone();
                Cow::Owned(crate::thermal::free_expansion_start(
                    &self.model,
                    &[],
                    u,
                    &old,
                    &new,
                    f.reference(),
                )?)
            }
            None => Cow::Borrowed(u),
        };
        let eq = nonlinear::solve_from_field(
            &self.model,
            &[],
            &load,
            &start,
            states,
            &self.newton,
            &list,
            &context,
            field.as_ref(),
        )?;
        let (a1, v1) = kin.of(&eq.u);
        Ok(Advanced {
            iterations: eq.residuals.len() - 1,
            residual: *eq.residuals.last().expect("one residual at least"),
            u: eq.u,
            v: v1,
            a: a1,
            fint: eq.internal_force,
            states: eq.states,
        })
    }

    /// The displacement, one value per equation.
    pub fn displacement(&self) -> Result<Vec<f64>> {
        Ok(self.current()?.u.clone())
    }

    /// The velocity, one value per equation.
    pub fn velocity(&self) -> Result<Vec<f64>> {
        Ok(self.current()?.v.clone())
    }

    /// The acceleration, one value per equation.
    pub fn acceleration(&self) -> Result<Vec<f64>> {
        Ok(self.current()?.a.clone())
    }

    /// The kinetic energy `v^T M v/2`.
    pub fn kinetic_energy(&self) -> Result<f64> {
        let v = &self.current()?.v;
        Ok(0.5 * dot(v, &matvec(&self.m, v)))
    }

    /// The strain energy `sum_q w_q S . E/2` at the points of the stiffness rule (the St. Venant-Kirchhoff potential of an
    /// elastic material; `E` the F-bar strain for a BBar Hex8, ADR 0506 D3, the enhanced strain for an Incompatible Hex8,
    /// ADR 0608 D4; the neo-Hookean energy of a hyperelastic
    /// material in the energy-momentum step, ADR 0523 D5); refused for a material with history (key `"strain energy"`).
    pub fn strain_energy(&self) -> Result<f64> {
        let s = self.current()?;
        if self.history {
            return Err(invalid(
                "strain energy",
                "a material with history has no strain energy potential (ADR 0479 D3)".to_string(),
            ));
        }
        if self.temperature_field.is_some() {
            return Err(invalid(
                "strain energy",
                "the strain energy with a temperature field is not offered (ADR 0534 D4)".to_string(),
            ));
        }
        let model = &self.model;
        let mesh = model.mesh();
        let dofs = model.dof_map();
        let materials = model.material_indices()?;
        let context = match self.temperature_at(self.time_of(s.t0, s.step)) {
            None => PointContext::default(),
            Some(t) => PointContext::new().with_temperature(t)?,
        };
        let mut w = 0.0;
        for e in 0..mesh.n_elements() {
            let nodes = mesh.element_nodes(e);
            let coords: Vec<Vec3> = nodes.iter().map(|&nd| mesh.nodes()[nd]).collect();
            let ue: Vec<f64> = crate::assemble::element_dofs(&dofs, nodes)
                .iter()
                .map(|&q| s.u[q])
                .collect();
            let material = &model.materials()[materials[e]].1;
            // ADR 0522 D4: a shell element takes the energy of the corotated element.
            if matches!(mesh.element_kind(e), ElementKind::Shell4 | ElementKind::Shell3) {
                w += crate::shell_corot::model_strain_energy(mesh, e, &coords, material, &ue)?;
                continue;
            }
            let el = element_for(mesh.element_kind(e));
            // ADR 0506 D3: an opted Hex8 takes the strain of its formulation (the F-bar strain of a BBar block); ADR 0608 D4:
            // the enhanced strain of an Incompatible block at its condensed modes.
            // ADR 0614 D4: a stabilised model's element at its stabilised modes, with the energy of the stabilisation.
            let opted = match crate::hex8x::opted(mesh, e) {
                Some(crate::mesh::SolidFormulation::Incompatible) => {
                    let stabilised = model.incompatible_stabilisation();
                    let (modes, strains) = crate::nl_incompatible::modes_and_strains_with(
                        &coords,
                        material,
                        &ue,
                        &ElementState::new(Vec::new()),
                        &context,
                        stabilised,
                    )?;
                    if stabilised {
                        w += crate::nl_incompatible::stabilisation_energy(&coords, material, &ue, &modes, &context)?;
                    }
                    Some(strains)
                }
                Some(f) => Some(nonlinear::hex8_strains(f, &coords, &ue)?),
                None => None,
            };
            for (q, ip) in el
                .integration_points(&coords, el.stiffness_rule())?
                .into_iter()
                .enumerate()
            {
                let strain = match &opted {
                    Some(s) => s[q],
                    None => {
                        let h = nonlinear::displacement_gradient(&ip.geometry.grad, &ue);
                        nonlinear::green_lagrange(&h)
                    }
                };
                // ADR 0523 D5: the neo-Hookean energy of the energy-momentum step; F-425's energy with
                // `MaterialDiscreteGradient` (ADR 0544 D3) and on the Method step (ADR 0555 D4).
                if let Material::Hyperelastic { energy, .. } = material {
                    w += ip.weight
                        * if self.stress == AlgorithmicStress::MaterialDiscreteGradient || self.step == StepKind::Method
                        {
                            crate::mat_hyper::energy(energy, &strain)?
                        } else {
                            dg::energy(energy, &strain)?
                        };
                    continue;
                }
                let (stress, _) = material.stress(&strain, &context, &[])?;
                w += ip.weight * 0.5 * dot(&stress, &strain);
            }
        }
        Ok(w)
    }

    /// The linear momentum `P = sum over the nodes of (M v)_node` and the angular momentum about the origin `J = sum over
    /// the nodes of (X + u)_node x (M v)_node` (ADR 0479 arithmetic 3).
    pub fn momentum(&self) -> Result<([f64; 3], [f64; 3])> {
        let s = self.current()?;
        let p = matvec(&self.m, &s.v);
        let dofs = self.model.dof_map();
        let nodes = self.model.mesh().nodes();
        let (mut lin, mut ang) = ([0.0; 3], [0.0; 3]);
        for (node, x0) in nodes.iter().enumerate() {
            let mut x = [0.0; 3];
            let mut pn = [0.0; 3];
            for c in 0..3 {
                let q = dofs.equation(node, c).expect("a solid node has three components");
                x[c] = x0[c] + s.u[q];
                pn[c] = p[q];
                lin[c] += p[q];
            }
            ang[0] += x[1] * pn[2] - x[2] * pn[1];
            ang[1] += x[2] * pn[0] - x[0] * pn[2];
            ang[2] += x[0] * pn[1] - x[1] * pn[0];
        }
        Ok((lin, ang))
    }

    /// The free equations, ascending.
    pub fn free(&self) -> &[usize] {
        &self.free
    }
}

/// Factors a symmetric positive definite matrix; refuses a failed factorisation or a pivot `d_i <= PIVOT_TOLERANCE a_ii`.
fn factor(a: &CsrMatrix, key: &'static str) -> Result<SkylineLdlt> {
    let diag = a.diag();
    let ldlt = SkylineLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => invalid(
            key,
            format!(
                "singular or not positive definite at reduced equation {row}: pivot {pivot:e}, diagonal {:e}",
                diag[row]
            ),
        ),
        other => invalid(key, other.to_string()),
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d.is_nan() || d <= PIVOT_TOLERANCE * diag[row] {
            return Err(invalid(
                key,
                format!(
                    "singular at reduced equation {row}: pivot {d:e} at most {PIVOT_TOLERANCE:e} of the diagonal {:e}",
                    diag[row]
                ),
            ));
        }
    }
    Ok(ldlt)
}
