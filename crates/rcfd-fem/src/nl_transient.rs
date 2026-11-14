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
//! step, `u`, `v`, `a`, the internal force and the material states) is saved and restored bit for bit.

use crate::constraint::{prescribed_equations, reduce};
use crate::element::{element_for, ElementState};
use crate::error::{FemError, Result};
use crate::mass::consistent_with;
use crate::material::PointContext;
use crate::model::{Load, SolidModel};
use crate::nonlinear::{self, Contribution, NewtonOptions};
use crate::solve::PIVOT_TOLERANCE;
use crate::transient::{LoadHistory, LoadTable, Method, Rayleigh};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The solver name of the errors of the time step (ADR 0479 D4).
pub const SOLVER_NAME: &str = "nl_transient";

const MAGIC: &[u8; 8] = b"RCFDNLT1";

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
}

fn state_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "state", reason }
}

impl NlTransientState {
    /// The time step the state was made with.
    pub fn time_step(&self) -> f64 {
        self.dt
    }

    /// The bytes of the state (ADR 0479 D3): magic `RCFDNLT1`, little-endian `u64` length, step and element count, the
    /// `f64` bits of `t0`, `dt`, the four parameters `(alpha_m, alpha_f, beta, gamma)`, `u`, `v`, `a`, the internal force,
    /// then per element its value count (`u64`) and its values.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.u.len();
        let mut b = Vec::with_capacity(8 * (10 + 4 * n));
        b.extend_from_slice(MAGIC);
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
        if bytes.len() < 32 || &bytes[..8] != MAGIC || bytes.len() % 8 != 0 {
            return Err(state_error(
                "not a saved nonlinear transient state (magic RCFDNLT1 missing or a truncated byte string)".to_string(),
            ));
        }
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
                return Err(state_error(format!("element {e}: {len} values beyond the end of the byte string")));
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
        })
    }
}

/// What one converged step reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepReport {
    /// The halvings of the time step before the step converged (its sub-steps are `2^halvings`).
    pub halvings: usize,
    /// Newton corrections of all its sub-steps.
    pub iterations: usize,
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

    fn add(&self, u: &[f64], _load_factor: f64, force: &mut [f64], tangent: &mut Vec<(usize, usize, f64)>) -> Result<()> {
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
}

/// The end of one sub-step.
struct Advanced {
    u: Vec<f64>,
    v: Vec<f64>,
    a: Vec<f64>,
    fint: Vec<f64>,
    states: Vec<ElementState>,
    iterations: usize,
}

impl Integrator {
    /// The integrator of a nonlinear solid model with its zero prescribed displacements, the load histories (full
    /// patterns) and the options (ADR 0479 D3). Refuses the transformation path and a temperature load (key
    /// `"nl_transient"`), a non-zero prescribed value (`"constraint"`), what the nonlinear driver refuses, a non-positive
    /// step (`"time step"`), `beta = 0` (`"method"`), invalid damping (`"damping"`) and a load pattern of the wrong length
    /// or with a non-finite value (`"load pattern"`).
    pub fn new(model: &SolidModel, loads: &[LoadHistory], options: &NlTransientOptions) -> Result<Integrator> {
        let dt = options.time_step;
        if !(dt.is_finite() && dt > 0.0) {
            return Err(invalid("time step", format!("{dt:e} (must be finite and > 0)")));
        }
        model.validate()?;
        model.check_supported_nonlinear()?;
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
                format!("load {i}: a temperature load is not offered by the nonlinear transient integrator (ADR 0479 D6)"),
            ));
        }
        nonlinear::check_driver_model(model)?;
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
        let c = if damping.mass != 0.0 || damping.stiffness != 0.0 {
            let k0 = nonlinear::tangent(model, &vec![0.0; n], &initial, threads)?;
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

    /// The consistent mass matrix (all equations).
    pub fn mass(&self) -> &CsrMatrix {
        &self.m
    }

    fn check_coverage(&self, t_a: f64, t_b: f64) -> Result<()> {
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

    fn damping_force(&self, v: &[f64]) -> Vec<f64> {
        match &self.c {
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
                return Err(invalid("initial state", format!("{key} {:e} at equation {e} is not finite", x[e])));
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
        let states = nonlinear::initial_states(&self.model)?;
        let fint = nonlinear::internal_force(&self.model, u0, &states)?;
        let f = self.load(t0);
        let cv = self.damping_force(v0);
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
            || state.states.iter().zip(&self.layout).any(|(s, &k)| s.values().len() != k)
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
            let mut failed = false;
            for k in 0..parts {
                let t_a = t_n + k as f64 * h;
                let t_b = if k + 1 == parts { t_end } else { t_n + (k + 1) as f64 * h };
                match self.substep(h, t_a, t_b, &u, &v, &a, &fint, &states, contributions) {
                    Ok(x) => {
                        u = x.u;
                        v = x.v;
                        a = x.a;
                        fint = x.fint;
                        states = x.states;
                        iterations += x.iterations;
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
        let [_, af, _, _] = self.parameters;
        let f0 = self.load(t_a);
        let f1 = self.load(t_b);
        let load: Vec<f64> = (0..f0.len())
            .map(|i| ((1.0 - af) * f1[i] + af * f0[i]) / (1.0 - af))
            .collect();
        let kin = Kinematics::new(u, v, a, self.parameters, h);
        let inertia = Inertia {
            m: &self.m,
            c: self.c.as_ref(),
            kin: &kin,
            fint_n: fint,
        };
        let mut list: Vec<&dyn Contribution> = Vec::with_capacity(1 + contributions.len());
        list.push(&inertia);
        list.extend_from_slice(contributions);
        let eq = nonlinear::solve_from(&self.model, &[], &load, u, states, &self.newton, &list)?;
        let (a1, v1) = kin.of(&eq.u);
        Ok(Advanced {
            iterations: eq.residuals.len() - 1,
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
    /// elastic material); refused for a material with history (key `"strain energy"`).
    pub fn strain_energy(&self) -> Result<f64> {
        let s = self.current()?;
        if self.history {
            return Err(invalid(
                "strain energy",
                "a material with history has no strain energy potential (ADR 0479 D3)".to_string(),
            ));
        }
        let model = &self.model;
        let mesh = model.mesh();
        let dofs = model.dof_map();
        let materials = model.material_indices()?;
        let context = PointContext::default();
        let mut w = 0.0;
        for e in 0..mesh.n_elements() {
            let nodes = mesh.element_nodes(e);
            let coords: Vec<Vec3> = nodes.iter().map(|&nd| mesh.nodes()[nd]).collect();
            let ue: Vec<f64> = crate::assemble::element_dofs(&dofs, nodes).iter().map(|&q| s.u[q]).collect();
            let el = element_for(mesh.element_kind(e));
            let material = &model.materials()[materials[e]].1;
            for ip in el.integration_points(&coords, el.stiffness_rule())? {
                let h = nonlinear::displacement_gradient(&ip.geometry.grad, &ue);
                let strain = nonlinear::green_lagrange(&h);
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
