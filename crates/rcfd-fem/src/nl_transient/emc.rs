//! The energy-momentum conserving step (ADR 0514; J. C. Simo and N. Tarnow, ZAMP 43 (1992) 757, cited from memory, the
//! conservation derived in the ADR).
//!
//! The midpoint step of length `h` from `(u_n, v_n)`: `v_{n+1} = 2 (u_{n+1} - u_n)/h - v_n` and on the free equations
//! `R = (2/h^2) M (u_{n+1} - u_n - h v_n) + C (u_{n+1} - u_n)/h + f_alg - (f_n + f_{n+1})/2 = 0`, with the algorithmic
//! internal force `f_alg = sum_q w_q B_NL(F_m)^T S_alg`, `F_m = I + (H_n + H_{n+1})/2`, `S_alg = (S(E_n) + S(E_{n+1}))/2`
//! (D1). For the isotropic St. Venant-Kirchhoff solid `u_{n+1} - u_n` dotted with `f_alg` is the change of the strain
//! energy, `sum_a f_alg,a = 0` and `sum_a x_m,a x f_alg,a = 0`: energy and momenta change only by the residual (ADR 0514).
//! Newton's iteration with the symmetric tangent `(2/h^2) M + C/h + sum_q w_q [B_m^T (D/2) B_m + G^T (S_alg/2) G]` (D2).
//! The neo-Hookean material and the discrete gradient of Gonzalez for `S_alg` (ADR 0523, [`super::dg`]) are chosen by
//! [`super::AlgorithmicStress`]; the isotropic material with `Auto` is the average above, unchanged. With
//! [`super::ResidualReference::InertiaTerms`] the reference of the relative residual is also at least the largest norm of the
//! products of the midpoint inertia and damping force (ADR 0609 D1).

use super::dg::{self, AlgorithmicStress};
use super::{factor, invalid, Advanced, Integrator};
use crate::assemble::{assemble_with, element_dofs};
use crate::constraint::reduce;
use crate::element::{element_for, Element, ElementMatrix, ElementState};
use crate::error::{FemError, Result};
use crate::geometric::element_geometric_stiffness;
use crate::material::PointContext;
use crate::mesh::SolidFormulation;
use crate::model::{Material, SolidModel};
use crate::nonlinear::{self, displacement_gradient, green_lagrange};
use rcfd_core::vec3::Vec3;
use std::sync::Mutex;

/// The registration named in the refusals of this step.
pub(super) const ADR: &str = "ADR 0514";

/// The refusals of ADR 0514 D4 that concern the model (key `"nl_transient"`): a block that is not a solid of the `Full`
/// formulation and a material other than the isotropic elastic one.
pub(super) fn check_model(model: &SolidModel) -> Result<()> {
    for (b, block) in model.mesh().blocks().iter().enumerate() {
        if !block.kind().is_solid() || block.formulation() != SolidFormulation::Full {
            return Err(invalid(
                "nl_transient",
                format!(
                    "block {b} ({} {}): the energy-momentum step takes solid elements of the Full formulation only \
                     ({ADR})",
                    block.kind().name(),
                    block.formulation().name()
                ),
            ));
        }
    }
    for (set, material) in model.materials() {
        if !matches!(material, Material::IsotropicElastic { .. }) {
            return Err(invalid(
                "nl_transient",
                format!(
                    "material {} of the set '{set}': the energy-momentum step takes the isotropic elastic (St. \
                     Venant-Kirchhoff) material only ({ADR})",
                    material.name()
                ),
            ));
        }
    }
    Ok(())
}

fn det3(f: &[[f64; 3]; 3]) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0])
        + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

/// `B_NL` (6 x 3n, row-major) of the deformation gradient `f`: `delta E = B_NL delta u` (ADR 0404 D1).
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

fn inverted(det: f64, q: usize, which: &str, element: &dyn Element) -> Option<FemError> {
    if det.is_nan() || det <= 0.0 || det.is_infinite() {
        Some(FemError::InvalidValue {
            key: nonlinear::INVERSION_KEY,
            reason: format!(
                "det F = {det:e} ({which}) at point {q} of a {} element (inverted)",
                element.kind().name()
            ),
        })
    } else {
        None
    }
}

/// The algorithmic stress and the material tangent of one point: the F-570 average for the isotropic material with `Auto`
/// or `Average` (ADR 0514 D1, the same operations), the discrete gradient or the average of ADR 0523 D3, D4 otherwise.
fn algorithmic_stress(
    material: &Material,
    e0: &[f64; 6],
    e1: &[f64; 6],
    context: &PointContext,
    mode: AlgorithmicStress,
) -> Result<([f64; 6], crate::material::Matrix6)> {
    if let Material::Hyperelastic { energy, .. } = material {
        // ADR 0544 D1: F-425's energy with `MaterialDiscreteGradient`.
        if mode == AlgorithmicStress::MaterialDiscreteGradient {
            return dg::material_stress(energy, e0, e1);
        }
        return dg::hyperelastic_stress(&dg::NeoHookean::of(energy)?, e0, e1, mode);
    }
    if matches!(
        mode,
        AlgorithmicStress::DiscreteGradient | AlgorithmicStress::MaterialDiscreteGradient
    ) {
        return dg::elastic_discrete_gradient(material, context, e0, e1);
    }
    let (s0, _) = material.stress(e0, context, &[])?;
    let (s1, d) = material.stress(e1, context, &[])?;
    let mut sa = [0.0; 6];
    for m in 0..6 {
        sa[m] = 0.5 * (s0[m] + s1[m]);
    }
    Ok((sa, d))
}

/// The algorithmic force of one element from `u0` (start of the step) to `u1`, and with `want_tangent` its symmetric
/// tangent with respect to `u1` (D2).
#[allow(clippy::too_many_arguments)]
fn element_step(
    element: &dyn Element,
    coords: &[Vec3],
    material: &Material,
    u0: &[f64],
    u1: &[f64],
    context: &PointContext,
    mode: AlgorithmicStress,
    want_tangent: bool,
) -> Result<(Vec<f64>, Option<ElementMatrix>)> {
    let size = 3 * element.n_nodes();
    let mut force = vec![0.0; size];
    let mut k = if want_tangent {
        Some(ElementMatrix::zeros(size))
    } else {
        None
    };
    let mut half_stress = Vec::new();
    let mut db = vec![0.0; 6 * size];
    for (q, ip) in element
        .integration_points(coords, element.stiffness_rule())?
        .iter()
        .enumerate()
    {
        let grad = &ip.geometry.grad;
        let h0 = displacement_gradient(grad, u0);
        let h1 = displacement_gradient(grad, u1);
        let mut f1 = h1;
        let mut fm = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                fm[i][j] = 0.5 * (h0[i][j] + h1[i][j]);
            }
            f1[i][i] += 1.0;
            fm[i][i] += 1.0;
        }
        if let Some(e) = inverted(det3(&f1), q, "end of the step", element) {
            return Err(e);
        }
        if let Some(e) = inverted(det3(&fm), q, "midpoint", element) {
            return Err(e);
        }
        let (sa, d) = algorithmic_stress(material, &green_lagrange(&h0), &green_lagrange(&h1), context, mode)?;
        let b = b_nonlinear(grad, &fm);
        for (c, fc) in force.iter_mut().enumerate() {
            let mut v = 0.0;
            for (m, sm) in sa.iter().enumerate() {
                v += b[m * size + c] * sm;
            }
            *fc += ip.weight * v;
        }
        if let Some(k) = k.as_mut() {
            for r in 0..6 {
                for c in 0..size {
                    let mut v = 0.0;
                    for (m, drm) in d[r].iter().enumerate() {
                        v += drm * b[m * size + c];
                    }
                    db[r * size + c] = 0.5 * v;
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
            half_stress.push(sa.map(|x| 0.5 * x));
        }
    }
    if let Some(k) = k.as_mut() {
        let kg = element_geometric_stiffness(element, coords, &half_stress)?;
        for (v, g) in k.values_mut().iter_mut().zip(kg.values()) {
            *v += g;
        }
    }
    Ok((force, k))
}

/// The assembled algorithmic force (element order) and the symmetric tangent of its part (D2).
fn force_and_tangent(
    model: &SolidModel,
    u0: &[f64],
    u1: &[f64],
    threads: Option<usize>,
    context: &PointContext,
    mode: AlgorithmicStress,
) -> Result<(Vec<f64>, rcfd_linalg::CsrMatrix)> {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let forces: Vec<Mutex<Vec<f64>>> = (0..mesh.n_elements()).map(|_| Mutex::new(Vec::new())).collect();
    let k = assemble_with(model, threads, |ctx| {
        let d = element_dofs(&dofs, ctx.nodes);
        let ue0: Vec<f64> = d.iter().map(|&q| u0[q]).collect();
        let ue1: Vec<f64> = d.iter().map(|&q| u1[q]).collect();
        let el = element_for(ctx.kind);
        let (f, k) = element_step(el, ctx.coords, ctx.material, &ue0, &ue1, context, mode, true).map_err(|source| {
            FemError::Element {
                element: ctx.element,
                source: Box::new(source),
            }
        })?;
        *forces[ctx.element].lock().expect("element force slot") = f;
        Ok(k.expect("tangent requested"))
    })?;
    let mut out = vec![0.0; u0.len()];
    for (e, slot) in forces.into_iter().enumerate() {
        let f = slot.into_inner().expect("element force slot");
        for (&q, v) in element_dofs(&dofs, mesh.element_nodes(e)).iter().zip(&f) {
            out[q] += v;
        }
    }
    Ok((out, k))
}

/// The internal force `sum_q w B_NL(F)^T S(E)` of a model with a neo-Hookean material at `u` (ADR 0523 D5): the element
/// routine with `u_0 = u_1`, whose algorithmic stress is `S(E)` (the midpoint branch at `dE = 0`), in element order; in the
/// mode `MaterialDiscreteGradient` with F-425's stress, in `Auto` (the step-local stress) otherwise (ADR 0544 D3).
pub(super) fn internal_force(
    model: &SolidModel,
    u: &[f64],
    context: &PointContext,
    mode: AlgorithmicStress,
) -> Result<Vec<f64>> {
    let mode = match mode {
        AlgorithmicStress::MaterialDiscreteGradient => mode,
        _ => AlgorithmicStress::Auto,
    };
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let materials = model.material_indices()?;
    let mut out = vec![0.0; u.len()];
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&nd| mesh.nodes()[nd]).collect();
        let d = element_dofs(&dofs, nodes);
        let ue: Vec<f64> = d.iter().map(|&q| u[q]).collect();
        let el = element_for(mesh.element_kind(e));
        let material = &model.materials()[materials[e]].1;
        let (f, _) = element_step(el, &coords, material, &ue, &ue, context, mode, false).map_err(|source| {
            FemError::Element {
                element: e,
                source: Box::new(source),
            }
        })?;
        for (&q, v) in d.iter().zip(&f) {
            out[q] += v;
        }
    }
    Ok(out)
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

fn newton_failure(reason: String) -> FemError {
    FemError::Solver {
        solver: "newton",
        reason,
    }
}

/// One energy-momentum sub-step of length `h` from `t_a` to `t_b` (D1, D2). A failed iteration is
/// `FemError::Solver { solver: "newton" }` and an inverted point the located inversion, both retried by the halving.
pub(super) fn substep(
    it: &Integrator,
    h: f64,
    t_a: f64,
    t_b: f64,
    u: &[f64],
    v: &[f64],
    states: &[ElementState],
) -> Result<Advanced> {
    let n = u.len();
    let options = &it.newton;
    let f0 = it.load(t_a);
    let f1 = it.load(t_b);
    let fm: Vec<f64> = (0..n).map(|i| 0.5 * (f0[i] + f1[i])).collect();
    let mut context = PointContext::new().with_time(t_b, h)?;
    // ADR 0526 D3, D4: the temperature and the damping at the sub-step's end.
    if let Some(t) = it.temperature_at(t_b) {
        context = context.with_temperature(t)?;
    }
    let damping = it.damping_at(t_b)?;
    let ci = 2.0 / (h * h);
    // ADR 0609 D1: with the inertia-term reference the products of the start, formed once per sub-step.
    let start_terms = match it.residual_reference {
        super::ResidualReference::Driver => None,
        super::ResidualReference::InertiaTerms => {
            let mut t = (ci * norm(super::matvec(&it.m, u).into_iter()))
                .max(2.0 / h * norm(super::matvec(&it.m, v).into_iter()));
            if let Some(c) = damping.as_deref() {
                t = t.max(norm(super::matvec(c, u).into_iter()) / h);
            }
            Some(t)
        }
    };
    let mut u1 = u.to_vec();
    let (mut e_first, mut e_last) = (0.0, 0.0);
    for k in 0..=options.max_iterations {
        let (fa, ka) = force_and_tangent(&it.model, u, &u1, options.threads, &context, it.stress)?;
        let w: Vec<f64> = (0..n).map(|i| u1[i] - u[i] - h * v[i]).collect();
        let mut inertia = super::matvec(&it.m, &w);
        for x in inertia.iter_mut() {
            *x *= ci;
        }
        if let Some(c) = damping.as_deref() {
            let du: Vec<f64> = (0..n).map(|i| (u1[i] - u[i]) / h).collect();
            for (x, y) in inertia.iter_mut().zip(super::matvec(c, &du)) {
                *x += y;
            }
        }
        let r_full: Vec<f64> = (0..n).map(|i| inertia[i] + fa[i] - fm[i]).collect();
        let rn = norm(it.free.iter().map(|&i| r_full[i]));
        let mut reference = norm(it.free.iter().map(|&i| fm[i]))
            .max(norm(fa.iter().copied()))
            .max(norm(inertia.iter().copied()));
        // ADR 0609 D1: the products of the iterate, opt-in.
        if let Some(t) = start_terms {
            let mut r1 = t.max(ci * norm(super::matvec(&it.m, &u1).into_iter()));
            if let Some(c) = damping.as_deref() {
                r1 = r1.max(norm(super::matvec(c, &u1).into_iter()) / h);
            }
            reference = reference.max(r1);
        }
        let r = if reference > 0.0 { rn / reference } else { rn };
        if !r.is_finite() {
            return Err(newton_failure(format!(
                "energy-momentum step: non-finite residual at iteration {k} ({ADR})"
            )));
        }
        if k >= 1 && r <= options.residual_tolerance && e_last <= options.energy_tolerance * e_first {
            let v1: Vec<f64> = (0..n).map(|i| 2.0 * (u1[i] - u[i]) / h - v[i]).collect();
            let a1: Vec<f64> = (0..n).map(|i| (v1[i] - v[i]) / h).collect();
            let fint = if it.hyper {
                internal_force(&it.model, &u1, &context, it.stress)?
            } else {
                nonlinear::internal_force_at(&it.model, &u1, states, &context)?
            };
            return Ok(Advanced {
                iterations: k,
                residual: r,
                u: u1,
                v: v1,
                a: a1,
                fint,
                states: states.to_vec(),
            });
        }
        if k == options.max_iterations {
            return Err(newton_failure(format!(
                "energy-momentum step: not converged in {} iterations (relative residual {r:e}, energy ratio {:e}) \
                 ({ADR})",
                options.max_iterations,
                if e_first > 0.0 { e_last / e_first } else { e_last }
            )));
        }
        let mut kt = super::combine(ci, &it.m, 1.0, &ka);
        if let Some(c) = damping.as_deref() {
            kt = super::combine(1.0, &kt, 1.0 / h, c);
        }
        let rhs: Vec<f64> = r_full.iter().map(|x| -x).collect();
        let sys = reduce(&kt, &rhs, &it.fixed)?;
        let du = if sys.free().is_empty() {
            Vec::new()
        } else {
            factor(&sys.matrix, "tangent")
                .map_err(|e| newton_failure(format!("energy-momentum step: {e} ({ADR})")))?
                .solve(&sys.rhs)
        };
        if du.iter().any(|x| !x.is_finite()) {
            return Err(newton_failure(format!(
                "energy-momentum step: non-finite correction at iteration {k} ({ADR})"
            )));
        }
        let e = du.iter().zip(&sys.rhs).map(|(a, b)| a * b).sum::<f64>().abs();
        if k == 0 {
            e_first = e;
        }
        e_last = e;
        for (&q, d) in sys.free().iter().zip(&du) {
            u1[q] += d;
        }
    }
    unreachable!("the loop returns at k = max_iterations")
}
