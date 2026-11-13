//! Geometric nonlinearity: total Lagrangian internal force and tangent, Newton driver with load steps. Declared by ADR
//! 0291; filled by ADR 0404 (the registration of ADR 0300, F-397).
//!
//! * Kinematics (ADR 0404 D1): `F = I + sum_a u_a (x) g_a` with the reference gradients `g_a = dN_a/dX`, the Green-Lagrange
//!   strain `E = (F^T F - I) / 2` (Voigt, engineering shear), the second Piola-Kirchhoff stress and the material tangent from
//!   [`Material::stress`] (St. Venant-Kirchhoff for the linear elastic material, `S = D E`).
//! * Element (D2): `f_int = sum_q w_q det J_q B_NL^T S` at the points of the element's stiffness rule, `B_NL` the variation of
//!   `E`; the tangent `K_T = sum_q w_q det J_q B_NL^T D B_NL + K_sigma(S)`, the geometric part by
//!   [`crate::geometric::element_geometric_stiffness`] (ADR 0390 D1). A point with `det F <= 0` is refused (key
//!   `"deformation gradient"`). [`crate::element::Element::internal_force`] and
//!   [`crate::element::Element::tangent`] call these functions.
//! * Assembly (D4): [`internal_force`] and [`tangent`], in element order, independent of the thread count.
//! * Driver (D5): [`solve`], load control of the model's dead loads with its prescribed displacements scaled by the same
//!   factor, Newton with a residual and an energy criterion, step halving on a failed attempt and a located error when the
//!   halvings are exhausted; element states committed per converged attempt and rolled back on a failed one.

use crate::assemble::{assemble_with, element_dofs};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce, Prescribed};
use crate::element::{Element, ElementMatrix, ElementState};
use crate::error::{FemError, Result};
use crate::geometric::element_geometric_stiffness;
use crate::loads::{nodal_load_vector, NodalForceAt};
use crate::material::PointContext;
use crate::mesh::SolidFormulation;
use crate::model::{Load, Material, SolidModel, SolidResult, SolverStats};
use crate::solve::PIVOT_TOLERANCE;
use rcfd_core::vec3::Vec3;
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;
use std::sync::Mutex;

/// The key of the refusal of an inverted point (ADR 0404 D2).
pub const INVERSION_KEY: &str = "deformation gradient";

/// The solver name of the Newton driver in [`SolverStats`] (ADR 0404 D5 step 6).
pub const SOLVER_NAME: &str = "newton-tl";

/// What one element evaluation gives.
struct Evaluation {
    force: Vec<f64>,
    tangent: Option<ElementMatrix>,
    stress: Vec<[f64; 6]>,
}

fn deformation_gradient(grad: &[[f64; 3]], u: &[f64]) -> [[f64; 3]; 3] {
    let mut f = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for (a, g) in grad.iter().enumerate() {
        for (i, row) in f.iter_mut().enumerate() {
            let ua = u[3 * a + i];
            for (j, v) in row.iter_mut().enumerate() {
                *v += ua * g[j];
            }
        }
    }
    f
}

fn det3(f: &[[f64; 3]; 3]) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0])
        + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

/// The Green-Lagrange strain `(F^T F - I) / 2` in Voigt order with engineering shear.
fn green_lagrange(f: &[[f64; 3]; 3]) -> [f64; 6] {
    let c = |i: usize, j: usize| f[0][i] * f[0][j] + f[1][i] * f[1][j] + f[2][i] * f[2][j];
    [
        0.5 * (c(0, 0) - 1.0),
        0.5 * (c(1, 1) - 1.0),
        0.5 * (c(2, 2) - 1.0),
        c(0, 1),
        c(1, 2),
        c(2, 0),
    ]
}

/// `B_NL` (6 x 3n, row-major): `delta E = B_NL delta u` (ADR 0404 D1).
fn b_nonlinear(grad: &[[f64; 3]], f: &[[f64; 3]; 3]) -> Vec<f64> {
    let cols = 3 * grad.len();
    let mut b = vec![0.0; 6 * cols];
    for (a, g) in grad.iter().enumerate() {
        for k in 0..3 {
            let c = 3 * a + k;
            let fk = &f[k];
            b[c] = fk[0] * g[0];
            b[cols + c] = fk[1] * g[1];
            b[2 * cols + c] = fk[2] * g[2];
            b[3 * cols + c] = fk[0] * g[1] + fk[1] * g[0];
            b[4 * cols + c] = fk[1] * g[2] + fk[2] * g[1];
            b[5 * cols + c] = fk[2] * g[0] + fk[0] * g[2];
        }
    }
    b
}

fn evaluate<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
) -> Result<Evaluation> {
    let size = 3 * element.n_nodes();
    if displacement.len() != size {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: size,
            found: displacement.len(),
        });
    }
    if let Some(i) = displacement.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "displacement",
            reason: format!("element unknown {i} is not finite"),
        });
    }
    let rule = element.stiffness_rule();
    let ns = material.state_size();
    if state.values().len() != rule.len() * ns {
        return Err(FemError::FieldLength {
            key: "state",
            expected: rule.len() * ns,
            found: state.values().len(),
        });
    }
    let context = PointContext::default();
    let mut force = vec![0.0; size];
    let mut k = if want_tangent {
        Some(ElementMatrix::zeros(size))
    } else {
        None
    };
    let mut stress = Vec::with_capacity(rule.len());
    let mut db = vec![0.0; 6 * size];
    for (q, ip) in element.integration_points(coords, rule)?.iter().enumerate() {
        let grad = &ip.geometry.grad;
        let f = deformation_gradient(grad, displacement);
        let det_f = det3(&f);
        if det_f.is_nan() || det_f <= 0.0 || det_f.is_infinite() {
            return Err(FemError::InvalidValue {
                key: INVERSION_KEY,
                reason: format!(
                    "det F = {det_f:e} at point {q} of a {} element (inverted)",
                    element.kind().name()
                ),
            });
        }
        let strain = green_lagrange(&f);
        let (s, d) = material.stress(&strain, &context, &state.values()[q * ns..(q + 1) * ns])?;
        let b = b_nonlinear(grad, &f);
        for (c, fc) in force.iter_mut().enumerate() {
            let mut v = 0.0;
            for (m, sm) in s.iter().enumerate() {
                v += b[m * size + c] * sm;
            }
            *fc += ip.weight * v;
        }
        if let Some(k) = k.as_mut() {
            // The material part with the loops of `Element::stiffness_with_rule`.
            for r in 0..6 {
                for c in 0..size {
                    let mut v = 0.0;
                    for (m, drm) in d[r].iter().enumerate() {
                        v += drm * b[m * size + c];
                    }
                    db[r * size + c] = v;
                }
            }
            let kv = k.values_mut();
            for i in 0..size {
                for j in 0..size {
                    let mut v = 0.0;
                    for m in 0..6 {
                        v += b[m * size + i] * db[m * size + j];
                    }
                    kv[i * size + j] += ip.weight * v;
                }
            }
        }
        stress.push(s);
    }
    if let Some(k) = k.as_mut() {
        let kg = element_geometric_stiffness(element, coords, &stress)?;
        for (v, g) in k.values_mut().iter_mut().zip(kg.values()) {
            *v += g;
        }
    }
    Ok(Evaluation {
        force,
        tangent: k,
        stress,
    })
}

/// The second Piola-Kirchhoff stress (Voigt, tensor shear) at the points of the element's stiffness rule, in the rule's
/// order, for the reference coordinates `coords` and the element displacement (node-major).
pub fn element_second_piola_kirchhoff<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<Vec<[f64; 6]>> {
    Ok(evaluate(element, coords, material, displacement, state, false)?.stress)
}

/// The total Lagrangian internal force `sum_q w_q det J_q B_NL^T S` of an element (ADR 0404 D2).
pub fn element_internal_force<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<Vec<f64>> {
    Ok(evaluate(element, coords, material, displacement, state, false)?.force)
}

/// The tangent `K_mat(F) + K_sigma(S)` of an element (ADR 0404 D2). Both triangles are computed.
pub fn element_tangent<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<ElementMatrix> {
    let ev = evaluate(element, coords, material, displacement, state, true)?;
    Ok(ev.tangent.expect("tangent requested"))
}

/// The internal force and the tangent of an element in one evaluation.
pub fn element_force_and_tangent<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<(Vec<f64>, ElementMatrix)> {
    let ev = evaluate(element, coords, material, displacement, state, true)?;
    Ok((ev.force, ev.tangent.expect("tangent requested")))
}

/// The refusals of ADR 0404 D5 step 1 that concern the model: a block that is not a solid of the `Full` formulation, the
/// transformation path and a temperature load (key `"nonlinear"`).
fn check_model(model: &SolidModel) -> Result<()> {
    model.check_elements_supported()?;
    for (b, block) in model.mesh().blocks().iter().enumerate() {
        if !block.kind().is_solid() || block.formulation() != SolidFormulation::Full {
            return Err(FemError::InvalidValue {
                key: "nonlinear",
                reason: format!(
                    "block {b} ({} {}): the nonlinear solve (ADR 0404) takes solid elements of the Full formulation only",
                    block.kind().name(),
                    block.formulation().name()
                ),
            });
        }
    }
    Ok(())
}

fn check_driver_model(model: &SolidModel) -> Result<()> {
    check_model(model)?;
    if crate::mpc::uses_transformation(model) {
        return Err(FemError::InvalidValue {
            key: "nonlinear",
            reason: "linear constraints, rigid sets, cyclic constraints and elastic supports (the transformation path) are \
                     not offered by the nonlinear static solve (ADR 0404)"
                .to_string(),
        });
    }
    if let Some(i) = model.loads().iter().position(|l| matches!(l, Load::Temperature { .. })) {
        return Err(FemError::InvalidValue {
            key: "nonlinear",
            reason: format!("load {i}: a temperature load is not offered by the nonlinear static solve (ADR 0404)"),
        });
    }
    Ok(())
}

/// The empty states of every element of `model` (`state_size()` values per point of the stiffness rule, zero).
pub fn initial_states(model: &SolidModel) -> Result<Vec<ElementState>> {
    model.check_elements_supported()?;
    let materials = model.material_indices()?;
    let mesh = model.mesh();
    Ok((0..mesh.n_elements())
        .map(|e| {
            let n = crate::element::element_for(mesh.element_kind(e)).stiffness_rule().len();
            let m = &model.materials()[materials[e]].1;
            ElementState::new(vec![0.0; n * m.state_size()])
        })
        .collect())
}

fn check_fields(model: &SolidModel, u: &[f64], states: &[ElementState]) -> Result<()> {
    let n = model.dof_map().n_equations();
    if u.len() != n {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: n,
            found: u.len(),
        });
    }
    let ne = model.mesh().n_elements();
    if states.len() != ne {
        return Err(FemError::FieldLength {
            key: "state",
            expected: ne,
            found: states.len(),
        });
    }
    Ok(())
}

/// The assembled internal force of `model` at the global displacement `u` with the element states (ADR 0404 D4), element
/// by element in element order. An element's error is located (`FemError::Element`).
pub fn internal_force(model: &SolidModel, u: &[f64], states: &[ElementState]) -> Result<Vec<f64>> {
    check_model(model)?;
    check_fields(model, u, states)?;
    let materials = model.material_indices()?;
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let mut out = vec![0.0; u.len()];
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let d = element_dofs(&dofs, nodes);
        let ue: Vec<f64> = d.iter().map(|&q| u[q]).collect();
        let el = crate::element::element_for(mesh.element_kind(e));
        let f = element_internal_force(el, &coords, &model.materials()[materials[e]].1, &ue, &states[e]).map_err(
            |source| FemError::Element {
                element: e,
                source: Box::new(source),
            },
        )?;
        for (&q, v) in d.iter().zip(&f) {
            out[q] += v;
        }
    }
    Ok(out)
}

/// The assembled tangent of `model` at `u` (ADR 0404 D4) on the pattern of the stiffness, with `threads` workers (`None`:
/// the available parallelism); bit-identical for any value.
pub fn tangent(model: &SolidModel, u: &[f64], states: &[ElementState], threads: Option<usize>) -> Result<CsrMatrix> {
    Ok(force_and_tangent(model, u, states, threads)?.1)
}

/// The internal force and the tangent in one pass: element matrices by [`assemble_with`], element forces stored per element
/// and added in element order.
fn force_and_tangent(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    threads: Option<usize>,
) -> Result<(Vec<f64>, CsrMatrix)> {
    check_model(model)?;
    check_fields(model, u, states)?;
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let ne = mesh.n_elements();
    let forces: Vec<Mutex<Vec<f64>>> = (0..ne).map(|_| Mutex::new(Vec::new())).collect();
    let k = assemble_with(model, threads, |ctx| {
        let d = element_dofs(&dofs, ctx.nodes);
        let ue: Vec<f64> = d.iter().map(|&q| u[q]).collect();
        let el = crate::element::element_for(ctx.kind);
        let (f, ke) = element_force_and_tangent(el, ctx.coords, ctx.material, &ue, &states[ctx.element])?;
        *forces[ctx.element].lock().expect("element force slot") = f;
        Ok(ke)
    })?;
    let mut out = vec![0.0; u.len()];
    for (e, slot) in forces.into_iter().enumerate() {
        let f = slot.into_inner().expect("element force slot");
        let d = element_dofs(&dofs, mesh.element_nodes(e));
        for (&q, v) in d.iter().zip(&f) {
            out[q] += v;
        }
    }
    Ok((out, k))
}

/// Options of the Newton driver (ADR 0404 D5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NewtonOptions {
    /// Load steps of equal size (at least 1).
    pub steps: usize,
    /// Newton corrections allowed per attempt.
    pub max_iterations: usize,
    /// The relative residual `|R_F| / max(|lambda f_F|, |f_int|)` at convergence.
    pub residual_tolerance: f64,
    /// The energy of the last correction relative to the first one of the attempt, at convergence.
    pub energy_tolerance: f64,
    /// Halvings of the increment allowed per load step.
    pub max_halvings: usize,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
}

impl NewtonOptions {
    /// `steps` load steps with the registered defaults: 20 iterations, residual `1e-10`, energy `1e-10`, 5 halvings.
    pub fn new(steps: usize) -> NewtonOptions {
        NewtonOptions {
            steps,
            max_iterations: 20,
            residual_tolerance: 1e-10,
            energy_tolerance: 1e-10,
            max_halvings: 5,
            threads: None,
        }
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> NewtonOptions {
        self.threads = Some(threads);
        self
    }
    /// The same options with another iteration cap.
    pub fn max_iterations(mut self, n: usize) -> NewtonOptions {
        self.max_iterations = n;
        self
    }

    fn check(&self) -> Result<()> {
        if self.steps == 0 {
            return Err(FemError::InvalidValue {
                key: "steps",
                reason: "0 load steps: at least 1".to_string(),
            });
        }
        if self.max_iterations == 0 {
            return Err(FemError::InvalidValue {
                key: "max_iterations",
                reason: "0 iterations: at least 1".to_string(),
            });
        }
        for (key, v) in [
            ("residual_tolerance", self.residual_tolerance),
            ("energy_tolerance", self.energy_tolerance),
        ] {
            if !(v > 0.0 && v.is_finite()) {
                return Err(FemError::InvalidValue {
                    key,
                    reason: format!("{v:e}: must be positive and finite"),
                });
            }
        }
        Ok(())
    }
}

/// One converged increment of the driver.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadStep {
    /// The user load step (1-based) the increment belongs to.
    pub step: usize,
    /// The load factor reached.
    pub load_factor: f64,
    /// The relative residuals `r_0, r_1, ...` of the iterations, the last one at convergence.
    pub residuals: Vec<f64>,
    /// The energies `|du_k . b_{k-1}|` of the corrections, `k = 1, 2, ...`.
    pub energies: Vec<f64>,
    /// Halvings of the user step before this increment converged.
    pub halvings: usize,
}

/// The result of [`solve`]: the displacement, the reactions and the solver statistics at load factor 1, and the converged
/// increments.
#[derive(Debug, Clone, PartialEq)]
pub struct NonlinearResult {
    /// Displacement, reactions (`f_int - f` at the prescribed equations) and statistics.
    pub result: SolidResult,
    /// The converged increments in order.
    pub steps: Vec<LoadStep>,
    /// The committed element states at load factor 1.
    pub states: Vec<ElementState>,
}

enum Failure {
    /// An inverted point: the located error.
    Inverted(FemError),
    /// Any other failure, described.
    Other(String),
}

fn classify(e: FemError) -> Failure {
    let inverted = matches!(&e, FemError::Element { source, .. }
        if matches!(source.as_ref(), FemError::InvalidValue { key, .. } if *key == INVERSION_KEY));
    if inverted {
        Failure::Inverted(e)
    } else {
        Failure::Other(e.to_string())
    }
}

struct Attempt {
    u: Vec<f64>,
    states: Vec<ElementState>,
    residuals: Vec<f64>,
    energies: Vec<f64>,
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

/// What the driver works with.
struct Problem<'a> {
    model: &'a SolidModel,
    f: Vec<f64>,
    fixed: Vec<(usize, f64)>,
    free: Vec<usize>,
    options: &'a NewtonOptions,
}

impl Problem<'_> {
    /// One attempt from the committed `(u_c, lc, states_c)` to the load factor `lt` (ADR 0404 D5 step 4).
    fn attempt(
        &self,
        u_c: &[f64],
        lc: f64,
        lt: f64,
        states_c: &[ElementState],
    ) -> std::result::Result<Attempt, Failure> {
        let o = self.options;
        let mut u = u_c.to_vec();
        // The trial states: a material with history updates them; the linear material has none (D5).
        let trial = states_c.to_vec();
        let mut residuals = Vec::new();
        let mut energies = Vec::new();
        let (mut e_first, mut e_last) = (0.0, 0.0);
        let dofs = self.model.dof_map();
        for k in 0..=o.max_iterations {
            let (fint, kt) = force_and_tangent(self.model, &u, states_c, o.threads).map_err(classify)?;
            let rfull: Vec<f64> = self.f.iter().zip(&fint).map(|(fi, gi)| lt * fi - gi).collect();
            let rn = norm(self.free.iter().map(|&i| rfull[i]));
            let reference = norm(self.free.iter().map(|&i| lt * self.f[i])).max(norm(fint.iter().copied()));
            let r = if reference > 0.0 { rn / reference } else { rn };
            if !r.is_finite() {
                return Err(Failure::Other(format!("non-finite residual at iteration {k}")));
            }
            residuals.push(r);
            if k >= 1 && r <= o.residual_tolerance && e_last <= o.energy_tolerance * e_first {
                return Ok(Attempt {
                    u,
                    states: trial,
                    residuals,
                    energies,
                });
            }
            if k == o.max_iterations {
                return Err(Failure::Other(format!(
                    "not converged in {} iterations (relative residual {r:e}, energy ratio {:e})",
                    o.max_iterations,
                    if e_first > 0.0 { e_last / e_first } else { e_last }
                )));
            }
            let increment: Vec<(usize, f64)> = self
                .fixed
                .iter()
                .map(|&(eq, v)| (eq, if k == 0 { (lt - lc) * v } else { 0.0 }))
                .collect();
            let sys = reduce(&kt, &rfull, &increment).map_err(|e| Failure::Other(e.to_string()))?;
            let du = if sys.free().is_empty() {
                Vec::new()
            } else {
                factor_solve(&sys.matrix, &sys.rhs, sys.free(), &dofs)?
            };
            if du.iter().any(|v| !v.is_finite()) {
                return Err(Failure::Other(format!("non-finite correction at iteration {k}")));
            }
            let e = du.iter().zip(&sys.rhs).map(|(a, b)| a * b).sum::<f64>().abs();
            if k == 0 {
                e_first = e;
            }
            e_last = e;
            energies.push(e);
            for (&q, d) in sys.free().iter().zip(&du) {
                u[q] += d;
            }
            if k == 0 {
                for &(eq, v) in &self.fixed {
                    u[eq] = lt * v;
                }
            }
        }
        unreachable!("the loop returns at k = max_iterations")
    }
}

/// The envelope `L D L^T` solve of the reduced tangent with the pivot test of the static solve (ADR 0275 arithmetic 4).
fn factor_solve(
    a: &CsrMatrix,
    b: &[f64],
    free: &[usize],
    dofs: &crate::model::DofMap,
) -> std::result::Result<Vec<f64>, Failure> {
    let diag = a.diag();
    let at = |row: usize| {
        let (n, c) = dofs.node_component(free[row]).expect("free equation exists");
        format!("node {n} component {c}")
    };
    let ldlt = SkylineLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => Failure::Other(format!(
            "singular tangent: pivot {pivot:e} at {} (diagonal {:e})",
            at(row),
            diag[row]
        )),
        other => Failure::Other(format!("tangent factorisation: {other}")),
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d.is_nan() || d <= PIVOT_TOLERANCE * diag[row] {
            return Err(Failure::Other(format!(
                "singular tangent: pivot {d:e} at {} (diagonal {:e})",
                at(row),
                diag[row]
            )));
        }
    }
    Ok(ldlt.solve(b))
}

/// The nonlinear static solve (ADR 0404 D5): total Lagrangian solids, the model's dead loads and `forces` times the load
/// factor `lambda`, its prescribed displacements and `prescribed` times `lambda`, `lambda` from 0 to 1 in `options.steps`
/// equal steps, each solved by Newton's method with the consistent tangent. A failed attempt halves the increment; after
/// `options.max_halvings` halvings of one step the error names the inverted element (`FemError::Element`, key `"deformation
/// gradient"`) or the step (`FemError::Solver { solver: "newton" }`).
pub fn solve(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &NewtonOptions,
) -> Result<NonlinearResult> {
    model.validate()?;
    model.check_supported()?;
    check_driver_model(model)?;
    options.check()?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    check_rigid_body_motions(model, &fixed)?;
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    let free: Vec<usize> = (0..n).filter(|&i| !is_fixed[i]).collect();
    let problem = Problem {
        model,
        f,
        fixed,
        free,
        options,
    };
    let mut u = vec![0.0; n];
    let mut lambda = 0.0;
    let mut states = initial_states(model)?;
    let mut records = Vec::new();
    let mut iterations = 0;
    let mut last_residual = 0.0;
    let ns = options.steps;
    for step in 1..=ns {
        let target = step as f64 / ns as f64;
        let mut dl = 1.0 / ns as f64;
        let mut halvings = 0;
        while lambda < target {
            let lt = if target - (lambda + dl) <= 1e-12 * dl {
                target
            } else {
                lambda + dl
            };
            match problem.attempt(&u, lambda, lt, &states) {
                Ok(a) => {
                    iterations += a.residuals.len() - 1;
                    last_residual = *a.residuals.last().expect("one residual at least");
                    u = a.u;
                    states = a.states;
                    lambda = lt;
                    records.push(LoadStep {
                        step,
                        load_factor: lt,
                        residuals: a.residuals,
                        energies: a.energies,
                        halvings,
                    });
                }
                Err(failure) => {
                    if halvings == options.max_halvings {
                        let at = format!(
                            "load step {step} of {ns} (load factor {lambda:e} to {lt:e}) after {halvings} halvings"
                        );
                        return Err(match failure {
                            Failure::Inverted(FemError::Element { element, source }) => FemError::Element {
                                element,
                                source: Box::new(match *source {
                                    FemError::InvalidValue { key, reason } => FemError::InvalidValue {
                                        key,
                                        reason: format!("{reason}; {at}"),
                                    },
                                    other => other,
                                }),
                            },
                            Failure::Inverted(other) => other,
                            Failure::Other(cause) => FemError::Solver {
                                solver: "newton",
                                reason: format!("{at}: {cause}"),
                            },
                        });
                    }
                    halvings += 1;
                    dl *= 0.5;
                }
            }
        }
    }
    let fint = internal_force(model, &u, &states)?;
    let mut reactions = vec![0.0; n];
    for &(eq, _) in &problem.fixed {
        reactions[eq] = fint[eq] - problem.f[eq];
    }
    let nf = problem.free.len();
    let result = SolidResult::new(dofs, u)?
        .with_reactions(reactions)?
        .with_stats(SolverStats::new(SOLVER_NAME, nf, iterations, last_residual));
    Ok(NonlinearResult {
        result,
        steps: records,
        states,
    })
}
