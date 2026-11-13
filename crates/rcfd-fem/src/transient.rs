//! Linear transient dynamics: Newmark and generalised-alpha integration, damping, load histories, integrator state.
//! Declared by ADR 0291; filled by ADR 0389 (the registration of F-398; design stub ADR 0301).
//!
//! `M a + C v + K u = f(t)` on the system reduced by the model's zero prescribed displacements ([`crate::constraint::reduce`]
//! of [`crate::assemble::stiffness_with`] and [`crate::mass::consistent_with`]), `C = alpha_R M + beta_R K` (Rayleigh) and
//! `f(t) = sum_j table_j(t) pattern_j` ([`LoadHistory`]). The methods (ADR 0389 D1): Newmark (1959) with `beta`, `gamma`
//! (average acceleration `1/4, 1/2` by default) and the generalised-alpha method of Chung and Hulbert (1993) with the
//! high-frequency spectral radius `rho_inf`:
//! `alpha_m = (2 rho - 1)/(rho + 1)`, `alpha_f = rho/(rho + 1)`, `gamma = 1/2 - alpha_m + alpha_f`, `beta = (1 - alpha_m +
//! alpha_f)^2/4`, equilibrium `M a_{n+1-alpha_m} + C v_{n+1-alpha_f} + K u_{n+1-alpha_f} = (1 - alpha_f) f_{n+1} + alpha_f
//! f_n`. Newmark is the case `alpha_m = alpha_f = 0`. One step solves `S a_{n+1} = r` with the effective matrix `S = c_M M +
//! c_K K`, formed and factored once.
//!
//! The state ([`TransientState`]: `t0`, the step index `n`, `u`, `v`, `a`) is saved and restored bit for bit; the time is
//! `t0 + n dt`, never accumulated. The steps use no threads; the assembly is bit-identical for any thread count.

use crate::assemble::stiffness_with;
use crate::constraint::{prescribed_equations, reduce};
use crate::error::{FemError, Result};
use crate::loads::nodal_load_vector;
use crate::mass::consistent_with;
use crate::model::{DofMap, SolidModel};
use crate::solve::PIVOT_TOLERANCE;
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;

/// The time integration method.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Method {
    /// Newmark's method with its two parameters (finite, `>= 0`; the stability is the caller's choice).
    Newmark {
        /// `beta`.
        beta: f64,
        /// `gamma`.
        gamma: f64,
    },
    /// The generalised-alpha method with the spectral radius at infinite frequency, `0 <= rho_inf <= 1`.
    GeneralisedAlpha {
        /// `rho_inf`.
        rho_inf: f64,
    },
}

impl Default for Method {
    fn default() -> Method {
        Method::average_acceleration()
    }
}

impl Method {
    /// The average-acceleration Newmark method, `beta = 1/4`, `gamma = 1/2`.
    pub fn average_acceleration() -> Method {
        Method::Newmark {
            beta: 0.25,
            gamma: 0.5,
        }
    }

    /// The four parameters `(alpha_m, alpha_f, beta, gamma)` (ADR 0389 D1).
    pub fn parameters(&self) -> Result<[f64; 4]> {
        match *self {
            Method::Newmark { beta, gamma } => {
                if !(beta.is_finite() && beta >= 0.0 && gamma.is_finite() && gamma >= 0.0) {
                    return Err(FemError::InvalidValue {
                        key: "method",
                        reason: format!("Newmark beta {beta:e} and gamma {gamma:e} must be finite and >= 0"),
                    });
                }
                Ok([0.0, 0.0, beta, gamma])
            }
            Method::GeneralisedAlpha { rho_inf } => {
                if !(rho_inf.is_finite() && (0.0..=1.0).contains(&rho_inf)) {
                    return Err(FemError::InvalidValue {
                        key: "method",
                        reason: format!("generalised-alpha rho_inf {rho_inf:e} must lie in [0, 1]"),
                    });
                }
                let am = (2.0 * rho_inf - 1.0) / (rho_inf + 1.0);
                let af = rho_inf / (rho_inf + 1.0);
                let gamma = 0.5 - am + af;
                let g = 1.0 - am + af;
                Ok([am, af, 0.25 * g * g, gamma])
            }
        }
    }
}

/// Rayleigh damping `C = mass M + stiffness K`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rayleigh {
    /// The mass-proportional coefficient `alpha_R`, 1/s.
    pub mass: f64,
    /// The stiffness-proportional coefficient `beta_R`, s.
    pub stiffness: f64,
}

impl Rayleigh {
    /// The coefficients giving the damping ratio `zeta` at the two circular frequencies `omega_1` and `omega_2`:
    /// `alpha_R = 2 zeta w1 w2/(w1 + w2)`, `beta_R = 2 zeta/(w1 + w2)`; with `w1 = w2 = w`, `zeta w` and `zeta/w`.
    pub fn from_ratio(zeta: f64, omega_1: f64, omega_2: f64) -> Result<Rayleigh> {
        if !(zeta.is_finite() && zeta >= 0.0 && omega_1 > 0.0 && omega_2 > 0.0 && (omega_1 + omega_2).is_finite()) {
            return Err(FemError::InvalidValue {
                key: "damping",
                reason: format!(
                    "ratio {zeta:e} must be finite and >= 0, frequencies {omega_1:e} and {omega_2:e} finite and > 0"
                ),
            });
        }
        let s = omega_1 + omega_2;
        Ok(Rayleigh {
            mass: 2.0 * zeta * omega_1 * omega_2 / s,
            stiffness: 2.0 * zeta / s,
        })
    }

    fn check(&self) -> Result<()> {
        let ok = |x: f64| x.is_finite() && x >= 0.0;
        if !(ok(self.mass) && ok(self.stiffness)) {
            return Err(FemError::InvalidValue {
                key: "damping",
                reason: format!(
                    "Rayleigh coefficients {:e} (mass) and {:e} (stiffness) must be finite and >= 0",
                    self.mass, self.stiffness
                ),
            });
        }
        Ok(())
    }
}

/// A piecewise linear table of a load factor against time.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadTable {
    points: Vec<(f64, f64)>,
}

impl LoadTable {
    /// A table from `(time, value)` points: at least two, all finite, times strictly increasing.
    pub fn new(points: Vec<(f64, f64)>) -> Result<LoadTable> {
        if points.len() < 2 {
            return Err(FemError::InvalidValue {
                key: "load table",
                reason: format!("{} points (at least 2)", points.len()),
            });
        }
        for (i, &(t, v)) in points.iter().enumerate() {
            if !(t.is_finite() && v.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "load table",
                    reason: format!("point {i} ({t:e}, {v:e}) is not finite"),
                });
            }
            if i > 0 && t <= points[i - 1].0 {
                return Err(FemError::InvalidValue {
                    key: "load table",
                    reason: format!("time {t:e} of point {i} does not follow {:e}", points[i - 1].0),
                });
            }
        }
        Ok(LoadTable { points })
    }

    /// A constant factor over `[t_start, t_end]`.
    pub fn constant(value: f64, t_start: f64, t_end: f64) -> Result<LoadTable> {
        LoadTable::new(vec![(t_start, value), (t_end, value)])
    }

    /// The points.
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// The first and last time.
    pub fn range(&self) -> (f64, f64) {
        (self.points[0].0, self.points[self.points.len() - 1].0)
    }

    /// Whether the table covers `[t_a, t_b]` (exact comparison).
    pub fn covers(&self, t_a: f64, t_b: f64) -> bool {
        let (lo, hi) = self.range();
        lo <= t_a && t_b <= hi
    }

    /// The value at `t`, `None` outside the table.
    pub fn value(&self, t: f64) -> Option<f64> {
        let (lo, hi) = self.range();
        if !(lo <= t && t <= hi) {
            return None;
        }
        // The first point with time >= t.
        let j = self.points.partition_point(|&(ti, _)| ti < t);
        let (tj, vj) = self.points[j];
        if tj == t || j == 0 {
            return Some(vj);
        }
        let (ti, vi) = self.points[j - 1];
        let s = (t - ti) / (tj - ti);
        Some(vi + s * (vj - vi))
    }
}

/// A fixed spatial load pattern scaled by a table.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadHistory {
    /// One value per equation (of the model's `DofMap`, or of the matrix size).
    pub pattern: Vec<f64>,
    /// The factor against time.
    pub table: LoadTable,
}

impl LoadHistory {
    /// A pattern scaled by a table.
    pub fn new(pattern: Vec<f64>, table: LoadTable) -> LoadHistory {
        LoadHistory { pattern, table }
    }

    /// The model's own loads ([`crate::loads::nodal_load_vector`]) scaled by a table.
    pub fn model_loads(model: &SolidModel, table: LoadTable) -> Result<LoadHistory> {
        Ok(LoadHistory {
            pattern: nodal_load_vector(model, &[])?,
            table,
        })
    }
}

/// Options of the integrator.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TransientOptions {
    /// The method.
    pub method: Method,
    /// The time step `dt`, s (finite, `> 0`).
    pub time_step: f64,
    /// Rayleigh damping (`None`: undamped).
    pub damping: Option<Rayleigh>,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
}

impl TransientOptions {
    /// The average-acceleration method with a time step, undamped.
    pub fn new(time_step: f64) -> TransientOptions {
        TransientOptions {
            method: Method::average_acceleration(),
            time_step,
            damping: None,
            threads: None,
        }
    }
    /// The same options with a method.
    pub fn method(mut self, method: Method) -> TransientOptions {
        self.method = method;
        self
    }
    /// The same options with Rayleigh damping.
    pub fn damping(mut self, damping: Rayleigh) -> TransientOptions {
        self.damping = Some(damping);
        self
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> TransientOptions {
        self.threads = Some(threads);
        self
    }
}

/// The state of the integrator (reduced vectors: one value per free equation).
#[derive(Debug, Clone, PartialEq)]
pub struct TransientState {
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
    dt: f64,
    parameters: [f64; 4],
}

const MAGIC: &[u8; 8] = b"RCFDTRS1";

fn state_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "state", reason }
}

impl TransientState {
    /// The time step the state was made with.
    pub fn time_step(&self) -> f64 {
        self.dt
    }

    /// The bytes of the state: magic `RCFDTRS1`, little-endian `u64` length and step, the `f64` bits of `t0`, `dt`, the four
    /// parameters `(alpha_m, alpha_f, beta, gamma)`, then `u`, `v`, `a`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let n = self.u.len();
        let mut b = Vec::with_capacity(8 * (9 + 3 * n));
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&(n as u64).to_le_bytes());
        b.extend_from_slice(&self.step.to_le_bytes());
        for x in [self.t0, self.dt]
            .iter()
            .chain(&self.parameters)
            .chain(&self.u)
            .chain(&self.v)
            .chain(&self.a)
        {
            b.extend_from_slice(&x.to_bits().to_le_bytes());
        }
        b
    }

    /// The state from [`TransientState::to_bytes`]; refuses a wrong magic or a wrong length.
    pub fn from_bytes(bytes: &[u8]) -> Result<TransientState> {
        if bytes.len() < 24 || &bytes[..8] != MAGIC {
            return Err(state_error("not a saved transient state (magic RCFDTRS1 missing)".to_string()));
        }
        let word = |i: usize| -> u64 {
            let mut w = [0u8; 8];
            w.copy_from_slice(&bytes[8 * i..8 * i + 8]);
            u64::from_le_bytes(w)
        };
        let n = word(1);
        let expected = (n as u128) * 24 + 72;
        if bytes.len() as u128 != expected {
            return Err(state_error(format!(
                "{} bytes for a state of {n} unknowns (expected {expected})",
                bytes.len()
            )));
        }
        let n = n as usize;
        let step = word(2);
        let f = |i: usize| f64::from_bits(word(i));
        let vec_at = |start: usize| (0..n).map(|k| f(start + k)).collect::<Vec<f64>>();
        Ok(TransientState {
            t0: f(3),
            dt: f(4),
            parameters: [f(5), f(6), f(7), f(8)],
            step,
            u: vec_at(9),
            v: vec_at(9 + n),
            a: vec_at(9 + 2 * n),
        })
    }
}

/// The time integrator of a linear model (ADR 0389).
#[derive(Debug, Clone)]
pub struct Integrator {
    k: CsrMatrix,
    m: CsrMatrix,
    s: SkylineLdlt,
    free: Vec<usize>,
    n_full: usize,
    dofs: Option<DofMap>,
    loads: Vec<(Vec<f64>, LoadTable)>,
    parameters: [f64; 4],
    dt: f64,
    damping: Rayleigh,
    state: Option<TransientState>,
}

fn check_time_step(dt: f64) -> Result<()> {
    if !(dt.is_finite() && dt > 0.0) {
        return Err(FemError::InvalidValue {
            key: "time step",
            reason: format!("{dt:e} (must be finite and > 0)"),
        });
    }
    Ok(())
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

impl Integrator {
    /// The integrator of a model with its zero prescribed displacements, the load histories (full patterns) and the options.
    pub fn new(model: &SolidModel, loads: &[LoadHistory], options: &TransientOptions) -> Result<Integrator> {
        check_time_step(options.time_step)?;
        model.validate()?;
        model.check_supported()?;
        if crate::mpc::uses_transformation(model) {
            return Err(FemError::InvalidValue {
                key: "constraint",
                reason: "linear constraints, rigid sets and elastic supports are not taken by the transient integrator \
                         (ADR 0389: the transformation path is not included)"
                    .to_string(),
            });
        }
        let dofs = model.dof_map();
        let fixed = prescribed_equations(model, &[])?;
        if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
            let (node, component) = dofs.node_component(eq).expect("existing equation");
            return Err(FemError::InvalidValue {
                key: "constraint",
                reason: format!(
                    "prescribed value {v:e} at node {node} component {component}: the transient integrator takes zero \
                     values only"
                ),
            });
        }
        let k = stiffness_with(model, options.threads)?;
        let m = consistent_with(model, options.threads)?;
        let zero = vec![0.0; k.n];
        let kr = reduce(&k, &zero, &fixed)?;
        let mr = reduce(&m, &zero, &fixed)?;
        let free = kr.free().to_vec();
        Integrator::build(kr.matrix, mr.matrix, free, k.n, Some(dofs), loads, options)
    }

    /// The integrator of given symmetric matrices `K` and `M` without constraints (patterns of the matrix size).
    pub fn from_matrices(
        k: CsrMatrix,
        m: CsrMatrix,
        loads: &[LoadHistory],
        options: &TransientOptions,
    ) -> Result<Integrator> {
        check_time_step(options.time_step)?;
        if k.n != k.ncols || m.n != m.ncols || k.n != m.n {
            return Err(FemError::FieldLength {
                key: "mass matrix",
                expected: k.n,
                found: m.n,
            });
        }
        let n = k.n;
        Integrator::build(k, m, (0..n).collect(), n, None, loads, options)
    }

    fn build(
        k: CsrMatrix,
        m: CsrMatrix,
        free: Vec<usize>,
        n_full: usize,
        dofs: Option<DofMap>,
        loads: &[LoadHistory],
        options: &TransientOptions,
    ) -> Result<Integrator> {
        let parameters = options.method.parameters()?;
        let damping = options.damping.unwrap_or(Rayleigh {
            mass: 0.0,
            stiffness: 0.0,
        });
        damping.check()?;
        let mut reduced_loads = Vec::with_capacity(loads.len());
        for l in loads {
            if l.pattern.len() != n_full {
                return Err(FemError::FieldLength {
                    key: "load pattern",
                    expected: n_full,
                    found: l.pattern.len(),
                });
            }
            if let Some(i) = l.pattern.iter().position(|x| !x.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "load pattern",
                    reason: format!("value {:e} at equation {i} is not finite", l.pattern[i]),
                });
            }
            reduced_loads.push((free.iter().map(|&e| l.pattern[e]).collect(), l.table.clone()));
        }
        let dt = options.time_step;
        let [am, af, beta, gamma] = parameters;
        let c_m = (1.0 - am) + (1.0 - af) * gamma * dt * damping.mass;
        let c_k = (1.0 - af) * beta * dt * dt + (1.0 - af) * gamma * dt * damping.stiffness;
        let s_matrix = combine(c_m, &m, c_k, &k);
        let s = factor(&s_matrix, "effective matrix", &free, dofs.as_ref())?;
        Ok(Integrator {
            k,
            m,
            s,
            free,
            n_full,
            dofs,
            loads: reduced_loads,
            parameters,
            dt,
            damping,
            state: None,
        })
    }

    /// The free equations (reduced index `i` is full equation `free()[i]`).
    pub fn free(&self) -> &[usize] {
        &self.free
    }

    /// The four parameters `(alpha_m, alpha_f, beta, gamma)`.
    pub fn parameters(&self) -> [f64; 4] {
        self.parameters
    }

    /// The time step.
    pub fn time_step(&self) -> f64 {
        self.dt
    }

    fn check_coverage(&self, t_a: f64, t_b: f64) -> Result<()> {
        for (j, (_, table)) in self.loads.iter().enumerate() {
            if !table.covers(t_a, t_b) {
                let (lo, hi) = table.range();
                return Err(FemError::InvalidValue {
                    key: "load table",
                    reason: format!("table {j} covers [{lo:e}, {hi:e}], the run needs [{t_a:e}, {t_b:e}]"),
                });
            }
        }
        Ok(())
    }

    /// The reduced load at `t` (the tables cover `t`).
    fn load(&self, t: f64) -> Vec<f64> {
        let mut f = vec![0.0; self.free.len()];
        for (pattern, table) in &self.loads {
            let s = table.value(t).expect("coverage checked");
            for (fi, pi) in f.iter_mut().zip(pattern) {
                *fi += s * pi;
            }
        }
        f
    }

    fn reduced(&self, full: &[f64], key: &'static str) -> Result<Vec<f64>> {
        if full.len() != self.n_full {
            return Err(FemError::FieldLength {
                key,
                expected: self.n_full,
                found: full.len(),
            });
        }
        let mut is_free = vec![false; self.n_full];
        for &e in &self.free {
            is_free[e] = true;
        }
        for (e, &x) in full.iter().enumerate() {
            if !x.is_finite() || (!is_free[e] && x != 0.0) {
                return Err(FemError::InvalidValue {
                    key: "initial state",
                    reason: format!("{key} {x:e} at equation {e} (finite, and 0 at a prescribed equation)"),
                });
            }
        }
        Ok(self.free.iter().map(|&e| full[e]).collect())
    }

    /// The damping force `C x`.
    fn damping_force(&self, x: &[f64]) -> Vec<f64> {
        let mut c = vec![0.0; x.len()];
        if self.damping.mass != 0.0 || self.damping.stiffness != 0.0 {
            let mx = matvec(&self.m, x);
            let kx = matvec(&self.k, x);
            for i in 0..x.len() {
                c[i] = self.damping.mass * mx[i] + self.damping.stiffness * kx[i];
            }
        }
        c
    }

    /// Starts at `t0` from the full displacement and velocity (0 at prescribed equations); the acceleration solves `M a0 =
    /// f(t0) - C v0 - K u0`.
    pub fn start(&mut self, t0: f64, u0: &[f64], v0: &[f64]) -> Result<()> {
        if !t0.is_finite() {
            return Err(FemError::InvalidValue {
                key: "initial state",
                reason: format!("start time {t0:e} is not finite"),
            });
        }
        let u = self.reduced(u0, "initial displacement")?;
        let v = self.reduced(v0, "initial velocity")?;
        self.check_coverage(t0, t0)?;
        let f = self.load(t0);
        let ku = matvec(&self.k, &u);
        let cv = self.damping_force(&v);
        let rhs: Vec<f64> = (0..u.len()).map(|i| f[i] - cv[i] - ku[i]).collect();
        let mf = factor(&self.m, "mass matrix", &self.free, self.dofs.as_ref())?;
        let a = mf.solve(&rhs);
        self.state = Some(TransientState {
            t0,
            step: 0,
            u,
            v,
            a,
            dt: self.dt,
            parameters: self.parameters,
        });
        Ok(())
    }

    fn current(&self) -> Result<&TransientState> {
        self.state
            .as_ref()
            .ok_or_else(|| state_error("the integrator has no state (call start or restore first)".to_string()))
    }

    /// The state (a clone).
    pub fn state(&self) -> Result<TransientState> {
        self.current().cloned()
    }

    /// Replaces the state; refuses another length, another time step or another method (by their bits).
    pub fn restore(&mut self, state: TransientState) -> Result<()> {
        let n = self.free.len();
        if state.u.len() != n || state.v.len() != n || state.a.len() != n {
            return Err(state_error(format!(
                "state of {}/{}/{} unknowns for an integrator of {n}",
                state.u.len(),
                state.v.len(),
                state.a.len()
            )));
        }
        if state.dt.to_bits() != self.dt.to_bits() {
            return Err(state_error(format!(
                "state of time step {:e} for an integrator of time step {:e}",
                state.dt, self.dt
            )));
        }
        if state.parameters.iter().zip(&self.parameters).any(|(a, b)| a.to_bits() != b.to_bits()) {
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

    /// The step index of the state.
    pub fn step_index(&self) -> Result<u64> {
        Ok(self.current()?.step)
    }

    /// Advances `steps` steps; refuses (before any step) a table that does not cover the run.
    pub fn run(&mut self, steps: u64) -> Result<()> {
        let (t0, n) = {
            let s = self.current()?;
            (s.t0, s.step)
        };
        self.check_coverage(self.time_of(t0, n), self.time_of(t0, n + steps))?;
        for _ in 0..steps {
            self.advance();
        }
        Ok(())
    }

    /// Advances one step.
    pub fn step(&mut self) -> Result<()> {
        self.run(1)
    }

    fn advance(&mut self) {
        let [am, af, beta, gamma] = self.parameters;
        let dt = self.dt;
        let state = self.state.as_ref().expect("state checked");
        let n = state.step;
        let f0 = self.load(self.time_of(state.t0, n));
        let f1 = self.load(self.time_of(state.t0, n + 1));
        let len = state.u.len();
        let (u, v, a) = (&state.u, &state.v, &state.a);
        let mut ut = vec![0.0; len];
        let mut vt = vec![0.0; len];
        let mut w_m = vec![0.0; len];
        let mut w_k = vec![0.0; len];
        for i in 0..len {
            ut[i] = u[i] + dt * v[i] + dt * dt * (0.5 - beta) * a[i];
            vt[i] = v[i] + dt * (1.0 - gamma) * a[i];
            let w_v = (1.0 - af) * vt[i] + af * v[i];
            w_m[i] = am * a[i] + self.damping.mass * w_v;
            w_k[i] = (1.0 - af) * ut[i] + af * u[i] + self.damping.stiffness * w_v;
        }
        let mw = matvec(&self.m, &w_m);
        let kw = matvec(&self.k, &w_k);
        let rhs: Vec<f64> = (0..len)
            .map(|i| (1.0 - af) * f1[i] + af * f0[i] - mw[i] - kw[i])
            .collect();
        let a1 = self.s.solve(&rhs);
        let u1: Vec<f64> = (0..len).map(|i| ut[i] + beta * dt * dt * a1[i]).collect();
        let v1: Vec<f64> = (0..len).map(|i| vt[i] + gamma * dt * a1[i]).collect();
        let state = self.state.as_mut().expect("state checked");
        state.u = u1;
        state.v = v1;
        state.a = a1;
        state.step = n + 1;
    }

    fn expand(&self, x: &[f64]) -> Vec<f64> {
        let mut full = vec![0.0; self.n_full];
        for (&e, &v) in self.free.iter().zip(x) {
            full[e] = v;
        }
        full
    }

    /// The displacement, one value per equation (0 at prescribed equations).
    pub fn displacement(&self) -> Result<Vec<f64>> {
        Ok(self.expand(&self.current()?.u))
    }

    /// The velocity, one value per equation.
    pub fn velocity(&self) -> Result<Vec<f64>> {
        Ok(self.expand(&self.current()?.v))
    }

    /// The acceleration, one value per equation.
    pub fn acceleration(&self) -> Result<Vec<f64>> {
        Ok(self.expand(&self.current()?.a))
    }

    /// The kinetic energy `v^T M v/2`.
    pub fn kinetic_energy(&self) -> Result<f64> {
        let v = &self.current()?.v;
        Ok(0.5 * dot(v, &matvec(&self.m, v)))
    }

    /// The strain energy `u^T K u/2`.
    pub fn strain_energy(&self) -> Result<f64> {
        let u = &self.current()?.u;
        Ok(0.5 * dot(u, &matvec(&self.k, u)))
    }

    /// The residual of the equation of motion at the state, `M a + C v + K u - f(t)` (reduced; the tables cover `t`).
    pub fn equilibrium_residual(&self) -> Result<Vec<f64>> {
        let s = self.current()?;
        let t = self.time_of(s.t0, s.step);
        self.check_coverage(t, t)?;
        let f = self.load(t);
        let ma = matvec(&self.m, &s.a);
        let cv = self.damping_force(&s.v);
        let ku = matvec(&self.k, &s.u);
        Ok((0..f.len()).map(|i| ma[i] + cv[i] + ku[i] - f[i]).collect())
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Factors a symmetric positive definite matrix; refuses a failed factorisation or a pivot `d_i <= PIVOT_TOLERANCE a_ii`.
fn factor(a: &CsrMatrix, key: &'static str, free: &[usize], dofs: Option<&DofMap>) -> Result<SkylineLdlt> {
    let diag = a.diag();
    let at = |row: usize| match dofs.and_then(|d| d.node_component(free[row])) {
        Some((node, component)) => format!("reduced equation {row} (node {node} component {component})"),
        None => format!("equation {row}"),
    };
    let ldlt = SkylineLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => FemError::InvalidValue {
            key,
            reason: format!(
                "singular or not positive definite at {}: pivot {pivot:e}, diagonal {:e}",
                at(row),
                diag[row]
            ),
        },
        other => FemError::InvalidValue {
            key,
            reason: other.to_string(),
        },
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d <= PIVOT_TOLERANCE * diag[row] {
            return Err(FemError::InvalidValue {
                key,
                reason: format!(
                    "singular at {}: pivot {d:e} at most {PIVOT_TOLERANCE:e} of the diagonal {:e}",
                    at(row),
                    diag[row]
                ),
            });
        }
    }
    Ok(ldlt)
}
