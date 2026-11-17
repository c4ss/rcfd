//! Geometric nonlinearity: total Lagrangian internal force and tangent, Newton driver with load steps. Declared by ADR
//! 0291; filled by ADR 0404 (the registration of ADR 0300, F-397).
//!
//! * Kinematics (ADR 0404 D1): `F = I + sum_a u_a (x) g_a` with the reference gradients `g_a = dN_a/dX`, the Green-Lagrange
//!   strain `E = (F^T F - I) / 2`, computed as `(H + H^T + H^T H) / 2` with `H = F - I` (amendment 1; Voigt, engineering
//!   shear), the second Piola-Kirchhoff stress and the material tangent from
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
//! * Extension points (ADR 0440 D7): [`solve_with`] takes [`DriverExtensions`], a list of extra [`Contribution`]s (force and
//!   tangent: contact, follower loads) and a [`StepControl`] hook that sets the target of each attempt; with neither, it is
//!   [`solve`] and executes the operations of ADR 0404. [`StateLayout`] is the per-element layout of the material states
//!   (ADR 0440 D3).
//! * History (ADR 0475 D3): a material with history (J2, [`crate::mat_j2`]) gives the trial state of each point from the
//!   committed state of the attempt with its stress; the attempt keeps the trial states of its latest evaluation and the
//!   driver commits them on convergence ([`element_trial_state`] gives them for one element).
//! * One attempt from a given state (ADR 0479 D2): [`solve_from`] runs the attempt of [`solve_with`] once, from a start
//!   displacement and committed states, with a full load vector and contributions; the nonlinear transient step of
//!   [`crate::nl_transient`] calls it with its inertia contribution, at the point context of its step ([`solve_from_at`],
//!   ADR 0485 D4: time, time increment and temperature for every point, read by the viscoelastic material of
//!   [`crate::mat_visco`] only; [`solve_from`] is the default context). The static driver [`solve_with`] has no time and
//!   refuses the viscoelastic material. A contribution may ask to enter the reference of the
//!   relative residual ([`Contribution::in_residual_reference`]); by default the reference is that of ADR 0404. When one
//!   does, an attempt whose relative residual is still above the tolerance at its last iteration is accepted if the
//!   residual is within the rounding bound `eps |(|K| |u|)|` of the iterate and the energy test holds (ADR 0604 D1, D2:
//!   the floor a translating or rotating body reaches); every attempt that converged before converges as before.
//! * Creep (ADR 0489): the creep material of [`crate::mat_creep`] gives its trial state as J2 and the viscoelastic material;
//!   the static driver refuses it; [`solve_in_time`] is a quasi-static time-stepping driver over [`solve_from_at`] with
//!   constant loads, a limit of the creep-strain increment of a step and step halving; a non-converged local creep iteration
//!   is a located error, as an inverted point.
//! * Histories (ADR 0499): [`solve_in_time_with`] is that driver with load and displacement histories against time
//!   ([`TimeLoading`]: fixed patterns times piecewise linear factors, taken at the end time of every attempt, halved or not)
//!   and a uniform temperature ([`TimeSteppingOptions::temperature`]); [`solve_in_time`] is it without histories.
//!   [`internal_force_at`] is the assembled internal force at a point context.
//! * The B-bar Hex8 (ADR 0506): a Hex8 of a block of the `BBar` formulation ([`crate::hex8x`]) takes the variational
//!   F-bar element on the same kinematics, `F_bar = (J_bar / det F)^(1/3) F` with `J_bar` the element mean of `det F`, the
//!   material at `E_bar = (F_bar^T F_bar - I)/2`, `f = sum_q w_q Bbar_NL^T S_bar` and its consistent, symmetric tangent
//!   (ADR 0506 arithmetic 1 and 2); at `u = 0` its tangent is the B-bar stiffness of ADR 0493. Every driver takes it through
//!   the assembly; [`hex8_force_and_tangent_at`], [`hex8_internal_force_at`], [`hex8_second_piola_kirchhoff`] and
//!   [`hex8_strains`] give one element.
//! * The incompatible-modes Hex8 (ADR 0608): a Hex8 of a block of the `Incompatible` formulation takes the enhanced
//!   deformation gradient of [`crate::nl_incompatible`], its nine modes condensed by a local Newton iteration at every
//!   evaluation; at `u = 0` its tangent is the condensed stiffness of ADR 0493. Every driver takes it through the assembly;
//!   [`hex8_force_and_tangent_at`], [`hex8_internal_force_at`] and [`hex8_second_piola_kirchhoff`] give one element, its
//!   strain [`crate::nl_incompatible::modes_and_strains`].

use crate::assemble::{assemble_with, element_dofs};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce, Prescribed};
use crate::element::{Element, ElementMatrix, ElementState, Hex8};
use crate::error::{FemError, Result};
use crate::geometric::element_geometric_stiffness;
use crate::loads::{nodal_load_vector, NodalForceAt};
use crate::material::{Matrix6, PointContext};
use crate::mesh::{ElementKind, SolidFormulation};
use crate::model::{Load, Material, SolidModel, SolidResult, SolverStats};
use crate::solve::{DirectFactor, PIVOT_TOLERANCE};
use crate::thermal::{FieldAt, TemperatureFieldHistory};
use crate::transient::{LoadHistory, LoadTable, MotionHistory};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::skyline::{PivotRule, SkylineError, SkylineLdlt, SkylineOptions, SkylineOrdering};
use rcfd_linalg::skyline_lu::EnvelopeLu;
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalAnalysis, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::sync::Mutex;

/// The key of the refusal of an inverted point (ADR 0404 D2).
pub const INVERSION_KEY: &str = "deformation gradient";

/// The solver name of the Newton driver in [`SolverStats`] (ADR 0404 D5 step 6).
pub const SOLVER_NAME: &str = "newton-tl";

/// What one element evaluation gives.
pub(crate) struct Evaluation {
    pub(crate) force: Vec<f64>,
    pub(crate) tangent: Option<ElementMatrix>,
    pub(crate) stress: Vec<[f64; 6]>,
    /// The new state values of a material with history (ADR 0475 D3), empty otherwise.
    pub(crate) state: Vec<f64>,
}

/// The displacement gradient `H = F - I`, `H_iJ = sum_a u_ai g_aJ`.
pub(crate) fn displacement_gradient(grad: &[[f64; 3]], u: &[f64]) -> [[f64; 3]; 3] {
    let mut h = [[0.0; 3]; 3];
    for (a, g) in grad.iter().enumerate() {
        for (i, row) in h.iter_mut().enumerate() {
            let ua = u[3 * a + i];
            for (j, v) in row.iter_mut().enumerate() {
                *v += ua * g[j];
            }
        }
    }
    h
}

/// `F = I + H`.
fn deformation_gradient(h: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut f = *h;
    for (i, row) in f.iter_mut().enumerate() {
        row[i] += 1.0;
    }
    f
}

fn det3(f: &[[f64; 3]; 3]) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0])
        + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

/// The Green-Lagrange strain `(F^T F - I) / 2 = (H + H^T + H^T H) / 2` in Voigt order with engineering shear, from `H`
/// without forming `F^T F - I` (ADR 0404 amendment 1: the rounding is relative to `H`, not to 1).
pub(crate) fn green_lagrange(h: &[[f64; 3]; 3]) -> [f64; 6] {
    let e = |i: usize, j: usize| h[i][j] + h[j][i] + h[0][i] * h[0][j] + h[1][i] * h[1][j] + h[2][i] * h[2][j];
    [0.5 * e(0, 0), 0.5 * e(1, 1), 0.5 * e(2, 2), e(0, 1), e(1, 2), e(2, 0)]
}

/// `B_NL` (6 x 3n, row-major): `delta E = B_NL delta u` (ADR 0404 D1).
pub(crate) fn b_nonlinear(grad: &[[f64; 3]], f: &[[f64; 3]; 3]) -> Vec<f64> {
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
    context: &PointContext,
) -> Result<Evaluation> {
    evaluate_with(
        element,
        coords,
        material,
        displacement,
        state,
        want_tangent,
        context,
        None,
    )
}

/// The nodal temperatures of one element in its node order, the reference and whether the thermal strain acts (ADR 0534 D2).
#[derive(Debug, Clone, Copy)]
struct ElementTemperatures<'a> {
    nodal: &'a [f64],
    reference: f64,
    expansion: bool,
}

/// [`evaluate`] with an optional nodal temperature field of the element (ADR 0534 D2): without one the operations of the
/// base; with one every point takes the context with its temperature and, when the expansion acts, the multiplicative
/// thermal stretch of [`thermal_point_update`].
#[allow(clippy::too_many_arguments)]
fn evaluate_with<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
    context: &PointContext,
    thermal: Option<ElementTemperatures<'_>>,
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
    let mut force = vec![0.0; size];
    let mut k = if want_tangent {
        Some(ElementMatrix::zeros(size))
    } else {
        None
    };
    let mut stress = Vec::with_capacity(rule.len());
    let mut new_state = Vec::new();
    let mut db = vec![0.0; 6 * size];
    for (q, ip) in element.integration_points(coords, rule)?.iter().enumerate() {
        let grad = &ip.geometry.grad;
        let h = displacement_gradient(grad, displacement);
        let f = deformation_gradient(&h);
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
        let strain = green_lagrange(&h);
        let committed = &state.values()[q * ns..(q + 1) * ns];
        let (s, d) = match thermal {
            None => point_update(material, &strain, committed, context, &mut new_state)?,
            Some(th) => thermal_point_update(
                material,
                &strain,
                committed,
                context,
                &ip.geometry.n,
                th,
                &mut new_state,
            )?,
        };
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
        state: new_state,
    })
}

/// The material at one point: the stress and the tangent at `strain` from the `committed` state, the trial state of a
/// material with history appended to `new_state`.
pub(crate) fn point_update(
    material: &Material,
    strain: &[f64; 6],
    committed: &[f64],
    context: &PointContext,
    new_state: &mut Vec<f64>,
) -> Result<([f64; 6], Matrix6)> {
    Ok(match material {
        // ADR 0475 D3: the return map from the committed state gives the trial state of the point.
        Material::J2 { .. } => {
            let u = crate::mat_j2::update(material, strain, committed)?;
            new_state.extend_from_slice(&u.state);
            (u.stress, u.tangent)
        }
        // ADR 0485 D3: the Prony update over the step of the point context gives the trial state of the point.
        Material::Viscoelastic { .. } if material.state_size() > 0 => {
            let u = crate::mat_visco::update(material, strain, committed, context)?;
            new_state.extend_from_slice(&u.state);
            (u.stress, u.tangent)
        }
        // ADR 0489 D3: the creep return over the step of the point context gives the trial state of the point.
        Material::Creep { .. } => {
            let u = crate::mat_creep::update(material, strain, committed, context)?;
            new_state.extend_from_slice(&u.state);
            (u.stress, u.tangent)
        }
        _ => material.stress(strain, context, committed)?,
    })
}

/// The material at one point with a nodal temperature field (ADR 0534 arithmetic 1 and 2): the point context with the
/// temperature `T_q = sum_b N_b T_b`; with the expansion acting and `a = alpha dT_q` not zero the multiplicative thermal
/// stretch `theta = 1 + a`: the material at `E_e = (E - (a + a^2/2) I)/theta^2` gives `S_e`, `D_e`, and the point gives `S =
/// theta S_e`, `D = D_e/theta`. A point with `theta <= 0` is refused (key `"temperature"`).
fn thermal_point_update(
    material: &Material,
    strain: &[f64; 6],
    committed: &[f64],
    context: &PointContext,
    n: &[f64],
    thermal: ElementTemperatures<'_>,
    new_state: &mut Vec<f64>,
) -> Result<([f64; 6], Matrix6)> {
    let (t_q, dt_q) = crate::thermal::point_temperature(n, thermal.nodal, thermal.reference);
    let mut at = context.clone();
    at.temperature = Some(t_q);
    let a = if thermal.expansion {
        material.expansion() * dt_q
    } else {
        0.0
    };
    if a == 0.0 {
        return point_update(material, strain, committed, &at, new_state);
    }
    let theta = 1.0 + a;
    if theta.is_nan() || theta <= 0.0 {
        return Err(FemError::InvalidValue {
            key: "temperature",
            reason: format!("the thermal stretch 1 + alpha dT = {theta:e} at a point is not positive (ADR 0534 D2)"),
        });
    }
    let e_th = a * (1.0 + 0.5 * a);
    let t2 = theta * theta;
    let elastic: [f64; 6] = std::array::from_fn(|i| (strain[i] - if i < 3 { e_th } else { 0.0 }) / t2);
    let (s_e, d_e) = point_update(material, &elastic, committed, &at, new_state)?;
    let s = s_e.map(|v| theta * v);
    let d = d_e.map(|row| row.map(|v| v / theta));
    Ok((s, d))
}

/// The unknowns of a Hex8.
const HEX8: usize = 24;

/// One point of the F-bar Hex8 (ADR 0506 D1).
struct FbarPoint {
    weight: f64,
    grad: Vec<[f64; 3]>,
    f: [[f64; 3]; 3],
    /// `d_a = F^{-T} g_a`, node-major.
    d: [f64; HEX8],
    /// `alpha^2 - 1`, `alpha = (J_bar / det F)^(1/3)`.
    a2m1: f64,
    /// The Green-Lagrange strain of `F` (Voigt, engineering shear).
    e: [f64; 6],
    /// The Green-Lagrange strain of `F_bar`.
    ebar: [f64; 6],
}

/// The kinematics of the F-bar Hex8: the points and `dbar = sum_q omega_q d_q`, `omega_q = w_q det F_q / (V J_bar)`.
struct FbarElement {
    points: Vec<FbarPoint>,
    omega: Vec<f64>,
    dbar: [f64; HEX8],
}

/// `det(I + H) - 1 = tr H + i2(H) + det H`, without the subtraction (ADR 0506 D1).
fn jacobian_minus_one(h: &[[f64; 3]; 3]) -> f64 {
    let tr = h[0][0] + h[1][1] + h[2][2];
    let mut tr2 = 0.0;
    for (i, row) in h.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            tr2 += v * h[j][i];
        }
    }
    tr + 0.5 * (tr * tr - tr2) + det3(h)
}

pub(crate) fn check_hex8_fields(coords: &[Vec3], displacement: &[f64]) -> Result<()> {
    if coords.len() != 8 {
        return Err(FemError::FieldLength {
            key: "coordinates",
            expected: 8,
            found: coords.len(),
        });
    }
    if displacement.len() != HEX8 {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: HEX8,
            found: displacement.len(),
        });
    }
    if let Some(i) = displacement.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "displacement",
            reason: format!("element unknown {i} is not finite"),
        });
    }
    Ok(())
}

/// The F-bar kinematics of a Hex8 at the points of its stiffness rule (ADR 0506 arithmetic 1, D1).
fn fbar_kinematics(coords: &[Vec3], displacement: &[f64]) -> Result<FbarElement> {
    check_hex8_fields(coords, displacement)?;
    let ips = Hex8.integration_points(coords, Hex8.stiffness_rule())?;
    let mut raw = Vec::with_capacity(ips.len());
    for (q, ip) in ips.iter().enumerate() {
        let grad = ip.geometry.grad.clone();
        let h = displacement_gradient(&grad, displacement);
        let f = deformation_gradient(&h);
        let det_f = det3(&f);
        if det_f.is_nan() || det_f <= 0.0 || det_f.is_infinite() {
            return Err(FemError::InvalidValue {
                key: INVERSION_KEY,
                reason: format!("det F = {det_f:e} at point {q} of a Hex8 element (inverted)"),
            });
        }
        // F^{-1} by the adjugate; d_a[i] = sum_j (F^{-1})_{ji} g_a[j].
        let mut inv = [[0.0; 3]; 3];
        for (r, row) in inv.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
                let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
                *v = (f[r1][c1] * f[r2][c2] - f[r1][c2] * f[r2][c1]) / det_f;
            }
        }
        let mut d = [0.0; HEX8];
        for (a, g) in grad.iter().enumerate() {
            for i in 0..3 {
                d[3 * a + i] = inv[0][i] * g[0] + inv[1][i] * g[1] + inv[2][i] * g[2];
            }
        }
        raw.push((ip.weight, grad, h, f, jacobian_minus_one(&h), d));
    }
    let volume: f64 = raw.iter().map(|r| r.0).sum();
    let jbar_m1 = raw.iter().map(|r| r.0 * r.4).sum::<f64>() / volume;
    let ln_jbar = jbar_m1.ln_1p();
    let jbar = 1.0 + jbar_m1;
    let omega: Vec<f64> = raw.iter().map(|r| r.0 * (1.0 + r.4) / (volume * jbar)).collect();
    let mut dbar = [0.0; HEX8];
    for (o, r) in omega.iter().zip(&raw) {
        for (b, d) in dbar.iter_mut().zip(&r.5) {
            *b += o * d;
        }
    }
    let points = raw
        .into_iter()
        .map(|(weight, grad, h, f, jm1, d)| {
            let a2m1 = (2.0 * (ln_jbar - jm1.ln_1p()) / 3.0).exp_m1();
            let e = green_lagrange(&h);
            let ebar = std::array::from_fn(|i| e[i] + a2m1 * (e[i] + if i < 3 { 0.5 } else { 0.0 }));
            FbarPoint {
                weight,
                grad,
                f,
                d,
                a2m1,
                e,
                ebar,
            }
        })
        .collect();
    Ok(FbarElement { points, omega, dbar })
}

/// The F-bar Hex8 (ADR 0506 D1): the force `sum_q w_q Bbar_NL^T S_bar` and the tangent of arithmetic 2.
fn evaluate_fbar(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
    context: &PointContext,
) -> Result<Evaluation> {
    let kin = fbar_kinematics(coords, displacement)?;
    let n = HEX8;
    let ns = material.state_size();
    let np = kin.points.len();
    if state.values().len() != np * ns {
        return Err(FemError::FieldLength {
            key: "state",
            expected: np * ns,
            found: state.values().len(),
        });
    }
    let mut force = vec![0.0; n];
    let mut k = if want_tangent {
        Some(ElementMatrix::zeros(n))
    } else {
        None
    };
    let mut stress = Vec::with_capacity(np);
    let mut scaled = Vec::with_capacity(np);
    let mut new_state = Vec::new();
    // The coefficient of the mean part of M_q (arithmetic 2), summed over the points.
    let mut mean_coefficient = 0.0;
    let mut bbar = vec![0.0; 6 * n];
    let mut db = vec![0.0; 6 * n];
    for (q, p) in kin.points.iter().enumerate() {
        let committed = &state.values()[q * ns..(q + 1) * ns];
        let (s, dm) = point_update(material, &p.ebar, committed, context, &mut new_state)?;
        let a2 = 1.0 + p.a2m1;
        let c: [f64; 6] = std::array::from_fn(|i| if i < 3 { 1.0 + 2.0 * p.e[i] } else { 2.0 * p.e[i] });
        let g: Vec<f64> = (0..n).map(|j| kin.dbar[j] - p.d[j]).collect();
        let b = b_nonlinear(&p.grad, &p.f);
        for r in 0..6 {
            for j in 0..n {
                bbar[r * n + j] = a2 * (b[r * n + j] + c[r] * g[j] / 3.0);
            }
        }
        for (j, fj) in force.iter_mut().enumerate() {
            let mut v = 0.0;
            for (m, sm) in s.iter().enumerate() {
                v += bbar[m * n + j] * sm;
            }
            *fj += p.weight * v;
        }
        if let Some(k) = k.as_mut() {
            for r in 0..6 {
                for j in 0..n {
                    let mut v = 0.0;
                    for (m, drm) in dm[r].iter().enumerate() {
                        v += drm * bbar[m * n + j];
                    }
                    db[r * n + j] = v;
                }
            }
            let s_b: Vec<f64> = (0..n).map(|j| (0..6).map(|m| b[m * n + j] * s[m]).sum()).collect();
            let s_c: f64 = s.iter().zip(&c).map(|(a, b)| a * b).sum();
            let w = p.weight;
            let kv = k.values_mut();
            for i in 0..n {
                for j in 0..n {
                    let mut v = 0.0;
                    for m in 0..6 {
                        v += bbar[m * n + i] * db[m * n + j];
                    }
                    // The second variation of E_bar without its geometric part and the mean part of M_q.
                    let t_q = p.d[3 * (j / 3) + i % 3] * p.d[3 * (i / 3) + j % 3];
                    let second = (2.0 / 3.0) * (s_b[i] * g[j] + g[i] * s_b[j])
                        + (2.0 / 9.0) * s_c * g[i] * g[j]
                        + (s_c / 3.0) * t_q;
                    kv[i * n + j] += w * (v + a2 * second);
                }
            }
            mean_coefficient += w * a2 * s_c / 3.0;
        }
        scaled.push(std::array::from_fn(|i| a2 * s[i]));
        stress.push(s);
    }
    if let Some(k) = k.as_mut() {
        // The mean part of M_q: sum_p omega_p (d_p d_p^T - T_p) - dbar dbar^T.
        let kv = k.values_mut();
        for i in 0..n {
            for j in 0..n {
                let mut m = -kin.dbar[i] * kin.dbar[j];
                for (o, p) in kin.omega.iter().zip(&kin.points) {
                    m += o * (p.d[i] * p.d[j] - p.d[3 * (j / 3) + i % 3] * p.d[3 * (i / 3) + j % 3]);
                }
                kv[i * n + j] += mean_coefficient * m;
            }
        }
        let kg = element_geometric_stiffness(&Hex8, coords, &scaled)?;
        for (v, gv) in kv.iter_mut().zip(kg.values()) {
            *v += gv;
        }
    }
    Ok(Evaluation {
        force,
        tangent: k,
        stress,
        state: new_state,
    })
}

/// One Hex8 of a formulation: `Full` [`evaluate`], `BBar` [`evaluate_fbar`], `Incompatible` the condensed element of
/// [`crate::nl_incompatible`] (ADR 0608 D3).
fn hex8_evaluate(
    formulation: SolidFormulation,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
    context: &PointContext,
) -> Result<Evaluation> {
    match formulation {
        SolidFormulation::Full => evaluate(&Hex8, coords, material, displacement, state, want_tangent, context),
        SolidFormulation::BBar => evaluate_fbar(coords, material, displacement, state, want_tangent, context),
        SolidFormulation::Incompatible => crate::nl_incompatible::evaluate(
            coords,
            material,
            displacement,
            state,
            want_tangent,
            context,
            crate::nl_incompatible::IncompatibleStabilisation::Off,
            false,
            false,
        ),
    }
}

/// The internal force and the tangent of a Hex8 of a formulation at the point context `context` (ADR 0506 D5): `Full` is
/// [`element_force_and_tangent_at`] with the Hex8 element, `BBar` the F-bar element of ADR 0506 D1, `Incompatible` the
/// condensed element of ADR 0608 D2.
pub fn hex8_force_and_tangent_at(
    formulation: SolidFormulation,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
) -> Result<(Vec<f64>, ElementMatrix)> {
    let ev = hex8_evaluate(formulation, coords, material, displacement, state, true, context)?;
    Ok((ev.force, ev.tangent.expect("tangent requested")))
}

/// The internal force of a Hex8 of a formulation at the point context `context` (ADR 0506 D5).
pub fn hex8_internal_force_at(
    formulation: SolidFormulation,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
) -> Result<Vec<f64>> {
    Ok(hex8_evaluate(formulation, coords, material, displacement, state, false, context)?.force)
}

/// The second Piola-Kirchhoff stress at the points of the stiffness rule of a Hex8 of a formulation, at the default point
/// context (ADR 0506 D5): for `BBar` the stress of the material at `E_bar`.
pub fn hex8_second_piola_kirchhoff(
    formulation: SolidFormulation,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<Vec<[f64; 6]>> {
    Ok(hex8_evaluate(
        formulation,
        coords,
        material,
        displacement,
        state,
        false,
        &PointContext::default(),
    )?
    .stress)
}

/// The Green-Lagrange strain (Voigt, engineering shear) at the points of the stiffness rule of a Hex8 of a formulation
/// (ADR 0506 D5): of `F` for `Full`, of `F_bar` for `BBar`; `Incompatible` refused (key `"formulation"`: its strain depends
/// on the material through the condensed modes, [`crate::nl_incompatible::modes_and_strains`], ADR 0608 D3).
pub fn hex8_strains(formulation: SolidFormulation, coords: &[Vec3], displacement: &[f64]) -> Result<Vec<[f64; 6]>> {
    match formulation {
        SolidFormulation::Full => {
            check_hex8_fields(coords, displacement)?;
            Ok(Hex8
                .integration_points(coords, Hex8.stiffness_rule())?
                .iter()
                .map(|ip| green_lagrange(&displacement_gradient(&ip.geometry.grad, displacement)))
                .collect())
        }
        SolidFormulation::BBar => Ok(fbar_kinematics(coords, displacement)?
            .points
            .iter()
            .map(|p| p.ebar)
            .collect()),
        SolidFormulation::Incompatible => Err(FemError::InvalidValue {
            key: "formulation",
            reason:
                "the strain of an Incompatible Hex8 at finite strain depends on the material through its condensed \
                     modes: nl_incompatible::modes_and_strains gives it (ADR 0608)"
                    .to_string(),
        }),
    }
}

/// Whether element `e` of `model` is a Hex8 of the BBar formulation (ADR 0506 D2).
fn is_fbar(model: &SolidModel, e: usize) -> bool {
    crate::hex8x::opted(model.mesh(), e) == Some(SolidFormulation::BBar)
}

/// Whether element `e` of `model` is a Hex8 of the Incompatible formulation (ADR 0608 D3).
fn is_incompatible(model: &SolidModel, e: usize) -> bool {
    crate::hex8x::opted(model.mesh(), e) == Some(SolidFormulation::Incompatible)
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
    Ok(evaluate(
        element,
        coords,
        material,
        displacement,
        state,
        false,
        &PointContext::default(),
    )?
    .stress)
}

/// The total Lagrangian internal force `sum_q w_q det J_q B_NL^T S` of an element (ADR 0404 D2).
pub fn element_internal_force<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<Vec<f64>> {
    Ok(evaluate(
        element,
        coords,
        material,
        displacement,
        state,
        false,
        &PointContext::default(),
    )?
    .force)
}

/// The tangent `K_mat(F) + K_sigma(S)` of an element (ADR 0404 D2). Both triangles are computed.
pub fn element_tangent<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<ElementMatrix> {
    let ev = evaluate(
        element,
        coords,
        material,
        displacement,
        state,
        true,
        &PointContext::default(),
    )?;
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
    let ev = evaluate(
        element,
        coords,
        material,
        displacement,
        state,
        true,
        &PointContext::default(),
    )?;
    Ok((ev.force, ev.tangent.expect("tangent requested")))
}

/// [`element_force_and_tangent`] at the point context `context` (ADR 0489 D3): the material evaluations of the element get
/// its time, time increment and temperature.
pub fn element_force_and_tangent_at<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
) -> Result<(Vec<f64>, ElementMatrix)> {
    let ev = evaluate(element, coords, material, displacement, state, true, context)?;
    Ok((ev.force, ev.tangent.expect("tangent requested")))
}

/// The trial state of an element at `displacement` from the committed `state` (ADR 0475 D3): for a material with
/// history (J2; the viscoelastic material at the default point context, an increment of zero length, ADR 0485 D3) the state
/// after the update at every point of the stiffness rule; for the others the empty state.
pub fn element_trial_state<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
) -> Result<ElementState> {
    Ok(ElementState::new(
        evaluate(
            element,
            coords,
            material,
            displacement,
            state,
            false,
            &PointContext::default(),
        )?
        .state,
    ))
}

/// The refusals of ADR 0404 D5 step 1 that concern the model: a block that is not a solid of the `Full` formulation, the
/// transformation path and a temperature load (key `"nonlinear"`).
fn check_model(model: &SolidModel) -> Result<()> {
    // ADR 0490 D3: Shell4 and Shell3 blocks take the corotated shell element.
    model.check_elements_supported_nonlinear_shells()?;
    let mut shells = false;
    for (b, block) in model.mesh().blocks().iter().enumerate() {
        if is_shell(block.kind()) {
            shells = true;
            continue;
        }
        if !block.kind().is_solid() || block.formulation() != SolidFormulation::Full {
            // ADR 0506 D2: a BBar block takes the F-bar Hex8; ADR 0608 D3: an Incompatible block the enhanced Hex8.
            if block.kind().is_solid() {
                match block.formulation() {
                    SolidFormulation::BBar | SolidFormulation::Incompatible => continue,
                    _ => {}
                }
            }
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
    if shells {
        crate::shell_corot::check_model(model)?;
    }
    Ok(())
}

/// A Shell4 or Shell3 element: evaluated by [`crate::shell_corot`] (ADR 0490 D3).
fn is_shell(kind: ElementKind) -> bool {
    matches!(kind, ElementKind::Shell4 | ElementKind::Shell3)
}

pub(crate) fn check_driver_model(model: &SolidModel) -> Result<()> {
    check_model(model)?;
    if crate::mpc::uses_transformation(model) {
        return Err(FemError::InvalidValue {
            key: "nonlinear",
            reason:
                "linear constraints, rigid sets, cyclic constraints and elastic supports (the transformation path) are \
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
    // ADR 0612 D4.
    if let Some(i) = model
        .loads()
        .iter()
        .position(|l| matches!(l, Load::ShellTemperatureGradient { .. }))
    {
        return Err(FemError::InvalidValue {
            key: "nonlinear",
            reason: format!(
                "load {i}: a shell temperature gradient is not offered by the nonlinear static solve (ADR 0612)"
            ),
        });
    }
    Ok(())
}

/// The empty states of every element of `model` (`state_size()` values per point of the stiffness rule, zero).
pub fn initial_states(model: &SolidModel) -> Result<Vec<ElementState>> {
    model.check_elements_supported_nonlinear_shells()?;
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
    internal_force_at(model, u, states, &PointContext::default())
}

/// [`internal_force`] at the point context `context` (ADR 0499 D4): every material evaluation gets its time, time increment
/// and temperature. With the default context it is [`internal_force`].
pub fn internal_force_at(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    context: &PointContext,
) -> Result<Vec<f64>> {
    internal_force_field(model, u, states, context, None)
}

/// [`internal_force_at`] with a nodal temperature field and its thermal strain (ADR 0534 D2): `temperatures` one value per
/// node, K, `reference` the stress-free temperature. Refused: the models of ADR 0534 D4 (a shell or BBar block, a material
/// without a single expansion coefficient, a field of another length, a non-positive thermal stretch).
pub fn internal_force_with_field(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    context: &PointContext,
    temperatures: &[f64],
    reference: f64,
) -> Result<Vec<f64>> {
    let field = checked_field(model, temperatures, reference)?;
    internal_force_field(model, u, states, context, Some(&field))
}

/// [`tangent_at`] with a nodal temperature field and its thermal strain (ADR 0534 D2), refused as
/// [`internal_force_with_field`].
pub fn tangent_with_field(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    threads: Option<usize>,
    context: &PointContext,
    temperatures: &[f64],
    reference: f64,
) -> Result<CsrMatrix> {
    let field = checked_field(model, temperatures, reference)?;
    Ok(force_and_tangent(model, u, states, threads, context, Some(&field))?.1)
}

/// One nodal field checked against the model (ADR 0534 D4), with the expansion acting.
fn checked_field(model: &SolidModel, temperatures: &[f64], reference: f64) -> Result<FieldAt> {
    let history = TemperatureFieldHistory::constant(reference, temperatures.to_vec(), 0.0, 1.0)?;
    crate::thermal::check_field_model(model, &history)?;
    Ok(FieldAt {
        nodal: temperatures.to_vec(),
        reference,
        expansion: true,
    })
}

/// The element slice of a nodal field.
fn element_field<'a>(
    field: Option<&FieldAt>,
    nodes: &[usize],
    buffer: &'a mut Vec<f64>,
) -> Option<ElementTemperatures<'a>> {
    let field = field?;
    buffer.clear();
    buffer.extend(nodes.iter().map(|&n| field.nodal[n]));
    Some(ElementTemperatures {
        nodal: buffer.as_slice(),
        reference: field.reference,
        expansion: field.expansion,
    })
}

/// The assembled internal force at a context with an optional nodal field (ADR 0534 D2).
pub(crate) fn internal_force_field(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    context: &PointContext,
    field: Option<&FieldAt>,
) -> Result<Vec<f64>> {
    check_model(model)?;
    check_fields(model, u, states)?;
    let materials = model.material_indices()?;
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let mut out = vec![0.0; u.len()];
    let mut buffer = Vec::new();
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let d = element_dofs(&dofs, nodes);
        let ue: Vec<f64> = d.iter().map(|&q| u[q]).collect();
        let kind = mesh.element_kind(e);
        let material = &model.materials()[materials[e]].1;
        let f = if is_shell(kind) {
            crate::shell_corot::model_internal_force(mesh, e, &coords, material, &ue)
        } else if is_fbar(model, e) {
            evaluate_fbar(&coords, material, &ue, &states[e], false, context).map(|ev| ev.force)
        } else if is_incompatible(model, e) {
            crate::nl_incompatible::evaluate(
                &coords,
                material,
                &ue,
                &states[e],
                false,
                context,
                model.incompatible_stabilisation_rule(),
                model.incompatible_mode_update(),
                model.incompatible_mode_warm_start(),
            )
            .map(|ev| ev.force)
        } else {
            evaluate_with(
                crate::element::element_for(kind),
                &coords,
                material,
                &ue,
                &states[e],
                false,
                context,
                element_field(field, nodes, &mut buffer),
            )
            .map(|ev| ev.force)
        }
        .map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
        for (&q, v) in d.iter().zip(&f) {
            out[q] += v;
        }
    }
    Ok(out)
}

/// The assembled tangent of `model` at `u` (ADR 0404 D4) on the pattern of the stiffness, with `threads` workers (`None`:
/// the available parallelism); bit-identical for any value.
pub fn tangent(model: &SolidModel, u: &[f64], states: &[ElementState], threads: Option<usize>) -> Result<CsrMatrix> {
    tangent_at(model, u, states, threads, &PointContext::default())
}

/// [`tangent`] at the point context `context` (ADR 0526 D4: the Rayleigh stiffness tangent of the nonlinear transient
/// integrator at its temperature); with the default context it is [`tangent`].
pub fn tangent_at(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    threads: Option<usize>,
    context: &PointContext,
) -> Result<CsrMatrix> {
    Ok(force_and_tangent(model, u, states, threads, context, None)?.1)
}

/// [`tangent_at`] with an optional nodal field (ADR 0534 D5: the damping tangent at the field's temperatures).
pub(crate) fn tangent_field(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    threads: Option<usize>,
    context: &PointContext,
    field: Option<&FieldAt>,
) -> Result<CsrMatrix> {
    Ok(force_and_tangent(model, u, states, threads, context, field)?.1)
}

/// The internal force and the tangent in one pass: element matrices by [`assemble_with`], element forces stored per element
/// and added in element order; with them the trial states of the elements (ADR 0475 D3, empty for a material without
/// history).
fn force_and_tangent(
    model: &SolidModel,
    u: &[f64],
    states: &[ElementState],
    threads: Option<usize>,
    context: &PointContext,
    field: Option<&FieldAt>,
) -> Result<(Vec<f64>, CsrMatrix, Vec<ElementState>)> {
    check_model(model)?;
    check_fields(model, u, states)?;
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let ne = mesh.n_elements();
    let forces: Vec<Mutex<Vec<f64>>> = (0..ne).map(|_| Mutex::new(Vec::new())).collect();
    let trial: Vec<Mutex<Vec<f64>>> = (0..ne).map(|_| Mutex::new(Vec::new())).collect();
    let k = assemble_with(model, threads, |ctx| {
        let d = element_dofs(&dofs, ctx.nodes);
        let ue: Vec<f64> = d.iter().map(|&q| u[q]).collect();
        let ev = if is_shell(ctx.kind) {
            let (force, k) =
                crate::shell_corot::model_force_and_tangent(mesh, ctx.element, ctx.coords, ctx.material, &ue)?;
            Evaluation {
                force,
                tangent: Some(k),
                stress: Vec::new(),
                state: Vec::new(),
            }
        } else if is_fbar(model, ctx.element) {
            evaluate_fbar(ctx.coords, ctx.material, &ue, &states[ctx.element], true, context)?
        } else if is_incompatible(model, ctx.element) {
            crate::nl_incompatible::evaluate(
                ctx.coords,
                ctx.material,
                &ue,
                &states[ctx.element],
                true,
                context,
                model.incompatible_stabilisation_rule(),
                model.incompatible_mode_update(),
                model.incompatible_mode_warm_start(),
            )?
        } else {
            let el = crate::element::element_for(ctx.kind);
            let mut buffer = Vec::new();
            let thermal = element_field(field, ctx.nodes, &mut buffer);
            evaluate_with(
                el,
                ctx.coords,
                ctx.material,
                &ue,
                &states[ctx.element],
                true,
                context,
                thermal,
            )?
        };
        *forces[ctx.element].lock().expect("element force slot") = ev.force;
        *trial[ctx.element].lock().expect("element state slot") = ev.state;
        Ok(ev.tangent.expect("tangent requested"))
    })?;
    let mut out = vec![0.0; u.len()];
    for (e, slot) in forces.into_iter().enumerate() {
        let f = slot.into_inner().expect("element force slot");
        let d = element_dofs(&dofs, mesh.element_nodes(e));
        for (&q, v) in d.iter().zip(&f) {
            out[q] += v;
        }
    }
    let trial = trial
        .into_iter()
        .map(|slot| ElementState::new(slot.into_inner().expect("element state slot")))
        .collect();
    Ok((out, k, trial))
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
    /// Record per iteration the yielded points and whether that set changed (ADR 0515 D2); off by default, no arithmetic
    /// depends on it.
    pub record_active_set: bool,
    /// The factor of the reduced tangent of every attempt without a load-factor control (ADR 0542 D1, D2):
    /// [`DirectFactor::Envelope`] (the default, the envelope `L D L^T` after reverse Cuthill-McKee) or
    /// [`DirectFactor::SupernodalNestedDissection`] (the supernodal multifrontal `L D L^T` after nested dissection by level
    /// structures of the tangent's pattern), each with the same pivot tests; any other value is refused (key `"solver"`).
    pub factor: DirectFactor,
    /// The full tangent (ADR 0560 D4): the contributions by [`Contribution::add_full`] and the reduced tangent factored by
    /// the unsymmetric envelope `L D U` ([`rcfd_linalg::skyline_lu::EnvelopeLu`], reverse Cuthill-McKee) with the same pivot
    /// tests; off by default. Refused with [`DirectFactor::SupernodalNestedDissection`] (key `"solver"`).
    pub unsymmetric: bool,
    /// The line search on the energy (ADR 0607 D1): each correction of an attempt without a load-factor control is taken
    /// with the step length `alpha` at which the derivative of the energy along it, `s = -d . r(u + alpha d)`, has fallen to
    /// at most half of its value at `alpha = 0` (the full step whenever it passes, else Illinois regula falsi on `(0, 1)`,
    /// at most 8 further trials); the lengths are recorded in [`LoadStep::step_lengths`]. Off by default; refused with
    /// `unsymmetric` (key `"line_search"`).
    pub line_search: bool,
    /// The stagnation acceptance (ADR 0619 D1): with a contribution in the residual reference, an attempt is also accepted
    /// at an iterate `k` with `3 <= k < max_iterations` whose residual and those of the two iterates before it are within
    /// the rounding bound `eps |(|K| |u|)|_free` of ADR 0604 D1, whose residual is above half of the one two iterates
    /// before (the last two corrections did not halve it), and whose energy test holds. Off by default; the relative test
    /// is tried first at every iterate, and the acceptance of ADR 0604 D2 at the last iterate is kept either way.
    pub stagnation: bool,
    /// The damping of the predictor (ADR 0688 D3): iteration 0 of an attempt with a non-zero prescribed increment, without a
    /// load-factor control, takes its free correction `du` with the step length `beta` of the rule of ADR 0607 D1 from the
    /// lifted iterate (the committed free values, the prescribed values at the target), a trial being admissible only when
    /// its reduced tangent also passes the driver's pivot test; recorded in [`LoadStep::step_lengths`]. Off by default;
    /// refused with `unsymmetric` and in an attempt with a contribution (key `"predictor_damping"`).
    pub predictor_damping: bool,
    /// The indefinite tangent (ADR 0694 D2): every factor of an attempt without a load-factor control takes the `L D L^T`
    /// with non-zero pivots (`PivotRule::NonZero`), a pivot refused only when `|d| <= 1e-10 |a_ii|` or not finite (the test
    /// of the arc-length correction, ADR 0480), so that Newton's method also iterates on a regular tangent that is not
    /// positive definite. Off by default; refused with `unsymmetric` (key `"indefinite_tangent"`).
    pub indefinite_tangent: bool,
    /// The backtracking of the predictor (ADR 0694 D3): iteration 0 of an attempt with a non-zero prescribed increment,
    /// without a load-factor control, takes its free correction `du` with the first `beta` of `1, 1/2, ..., 2^-10` whose
    /// trial (the prescribed values at the target) is admissible (its evaluation succeeds and its reduced tangent passes the
    /// driver's positive pivot test), `beta = 1` if none is; recorded in [`LoadStep::step_lengths`], and a failed attempt
    /// whose predictor was backtracked names its `beta`. Off by default; refused with `unsymmetric`, with `predictor_damping`
    /// and in an attempt with a contribution (key `"predictor_backtracking"`).
    pub predictor_backtracking: bool,
    /// The retry with the indefinite tangent (ADR 0719 D1): read only by [`solve`] and [`solve_with`]. An attempt that fails
    /// with a refused factor (its cause starts with `"singular tangent"`) is made once more from the same committed state to
    /// the same load factor with [`NewtonOptions::indefinite_tangent`]; a retry that also fails is the attempt's failure, its
    /// cause followed by `"; retried with the indefinite tangent (ADR 0719 D1) after: "` and the first cause, and the driver
    /// halves as without it. Every accepted state's inertia is recorded in [`LoadStep::inertia`] (ADR 0719 D2). Off by
    /// default; refused (key `"indefinite_retry"`) with `indefinite_tangent`, with `unsymmetric`, with a contribution, and by
    /// every other entry point that takes the options.
    pub indefinite_retry: bool,
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
            record_active_set: false,
            factor: DirectFactor::Envelope,
            unsymmetric: false,
            line_search: false,
            stagnation: false,
            predictor_damping: false,
            indefinite_tangent: false,
            predictor_backtracking: false,
            indefinite_retry: false,
        }
    }
    /// The same options with the active-set record of [`LoadStep::yielded`] switched on or off.
    pub fn record_active_set(mut self, on: bool) -> NewtonOptions {
        self.record_active_set = on;
        self
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
    /// The same options with another factor of the reduced tangent (ADR 0542 D1).
    pub fn factor(mut self, factor: DirectFactor) -> NewtonOptions {
        self.factor = factor;
        self
    }
    /// The same options with the full tangent and the unsymmetric factor switched on or off (ADR 0560 D4).
    pub fn unsymmetric(mut self, on: bool) -> NewtonOptions {
        self.unsymmetric = on;
        self
    }
    /// The same options with the line search on the energy switched on or off (ADR 0607 D1).
    pub fn line_search(mut self, on: bool) -> NewtonOptions {
        self.line_search = on;
        self
    }
    /// The same options with the stagnation acceptance at the rounding bound switched on or off (ADR 0619 D1).
    pub fn stagnation(mut self, on: bool) -> NewtonOptions {
        self.stagnation = on;
        self
    }
    /// The same options with the damping of the predictor switched on or off (ADR 0688 D3).
    pub fn predictor_damping(mut self, on: bool) -> NewtonOptions {
        self.predictor_damping = on;
        self
    }
    /// The same options with the indefinite tangent switched on or off (ADR 0694 D2).
    pub fn indefinite_tangent(mut self, on: bool) -> NewtonOptions {
        self.indefinite_tangent = on;
        self
    }
    /// The same options with the backtracking of the predictor switched on or off (ADR 0694 D3).
    pub fn predictor_backtracking(mut self, on: bool) -> NewtonOptions {
        self.predictor_backtracking = on;
        self
    }
    /// The same options with the retry with the indefinite tangent switched on or off (ADR 0719 D1).
    pub fn indefinite_retry(mut self, on: bool) -> NewtonOptions {
        self.indefinite_retry = on;
        self
    }

    /// ADR 0719 D1: the refusal of [`NewtonOptions::indefinite_retry`] by an entry point that does not read it (`entry`
    /// names it).
    pub(crate) fn refuse_indefinite_retry(&self, entry: &str) -> Result<()> {
        if self.indefinite_retry {
            return Err(FemError::InvalidValue {
                key: "indefinite_retry",
                reason: format!(
                    "{entry}: the retry with the indefinite tangent is read only by the load-stepping driver (ADR 0719 D1)"
                ),
            });
        }
        Ok(())
    }

    pub(crate) fn check(&self) -> Result<()> {
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
        if !matches!(
            self.factor,
            DirectFactor::Envelope | DirectFactor::SupernodalNestedDissection
        ) {
            return Err(FemError::InvalidValue {
                key: "solver",
                reason: format!(
                    "{:?}: the Newton driver factors by Envelope or SupernodalNestedDissection (ADR 0542)",
                    self.factor
                ),
            });
        }
        if self.unsymmetric && self.factor != DirectFactor::Envelope {
            return Err(FemError::InvalidValue {
                key: "solver",
                reason: format!(
                    "{:?} with the unsymmetric tangent: the unsymmetric factor is the envelope L D U (ADR 0560 D4)",
                    self.factor
                ),
            });
        }
        if self.line_search && self.unsymmetric {
            return Err(FemError::InvalidValue {
                key: "line_search",
                reason: "with the unsymmetric tangent: the line search needs a symmetric positive definite factor (ADR 0607 D1)"
                    .to_string(),
            });
        }
        if self.predictor_damping && self.unsymmetric {
            return Err(FemError::InvalidValue {
                key: "predictor_damping",
                reason: "with the unsymmetric tangent: the damping of the predictor needs the symmetric factor (ADR 0688 D3)"
                    .to_string(),
            });
        }
        if self.indefinite_tangent && self.unsymmetric {
            return Err(FemError::InvalidValue {
                key: "indefinite_tangent",
                reason: "with the unsymmetric tangent: the indefinite tangent is the symmetric L D L^T with non-zero pivots (ADR \
                         0694 D2)"
                    .to_string(),
            });
        }
        if self.predictor_backtracking && self.unsymmetric {
            return Err(FemError::InvalidValue {
                key: "predictor_backtracking",
                reason: "with the unsymmetric tangent: the backtracking of the predictor needs the symmetric factor (ADR 0694 \
                         D3)"
                    .to_string(),
            });
        }
        if self.predictor_backtracking && self.predictor_damping {
            return Err(FemError::InvalidValue {
                key: "predictor_backtracking",
                reason: "with predictor_damping: both set the step length of the predictor (ADR 0694 D3)".to_string(),
            });
        }
        if self.indefinite_retry && self.indefinite_tangent {
            return Err(FemError::InvalidValue {
                key: "indefinite_retry",
                reason: "with indefinite_tangent: every factor is indefinite already, nothing to retry (ADR 0719 D1)"
                    .to_string(),
            });
        }
        if self.indefinite_retry && self.unsymmetric {
            return Err(FemError::InvalidValue {
                key: "indefinite_retry",
                reason: "with the unsymmetric tangent: the retry takes the symmetric indefinite tangent (ADR 0719 D1)"
                    .to_string(),
            });
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
    /// With [`NewtonOptions::record_active_set`], per iteration (one value per residual) the number of integration points
    /// whose trial state differs from the committed one (the yielded points of a history material); empty otherwise.
    pub yielded: Vec<usize>,
    /// With the record, per iteration whether that set differs from the one of the previous iteration (`false` at
    /// iteration 0); empty otherwise.
    pub set_changed: Vec<bool>,
    /// With [`NewtonOptions::line_search`] or [`NewtonOptions::predictor_damping`], the step length of every correction
    /// (ADR 0607 D1, ADR 0688 D3); empty otherwise.
    pub step_lengths: Vec<f64>,
    /// With [`NewtonOptions::indefinite_retry`], the inertia of the accepted state (ADR 0719 D2): the number of negative
    /// pivots of the `L D L^T` with non-zero pivots of the reduced tangent at the converged iterate, `None` when that factor
    /// refuses a pivot; `None` without the option.
    pub inertia: Option<usize>,
    /// With [`NewtonOptions::indefinite_retry`], whether the increment was accepted by the retry with the indefinite tangent
    /// (ADR 0719 D1); `false` otherwise.
    pub retried: bool,
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
    // ADR 0489 D4: a non-converged local creep iteration stays located, as an inverted point.
    let inverted = matches!(&e, FemError::Element { source, .. }
        if matches!(source.as_ref(), FemError::InvalidValue { key, .. }
            if *key == INVERSION_KEY || *key == crate::mat_creep::ITERATION_KEY));
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
    /// The active-set record of the iterations (ADR 0515 D2); empty unless asked for.
    yielded: Vec<usize>,
    set_changed: Vec<bool>,
    /// The step lengths of the line search (ADR 0607 D1); empty without it.
    step_lengths: Vec<f64>,
    /// The internal force of the converged iterate, without the contributions (ADR 0479 D2).
    fint: Vec<f64>,
}

/// The points whose trial state differs bitwise from the committed one, element-major (ADR 0515 D2). With `modes_after`
/// (the model's warm start, ADR 0660 D1) the values of an element are cut to its material part first (ADR 0666 D1: the
/// committed modes after the material state are not points).
fn yielded_points(
    layout: &StateLayout,
    committed: &[ElementState],
    trial: &[ElementState],
    modes_after: bool,
) -> Vec<bool> {
    let mut out = Vec::new();
    for (e, (c, t)) in committed.iter().zip(trial).enumerate() {
        let k = layout.values_per_point(e);
        if k == 0 {
            out.extend(std::iter::repeat(false).take(layout.points(e)));
            continue;
        }
        let (cv, tv) = if modes_after {
            let n = layout.points(e) * k;
            let (cv, tv) = (c.values(), t.values());
            (&cv[..n.min(cv.len())], &tv[..n.min(tv.len())])
        } else {
            (c.values(), t.values())
        };
        for (a, b) in cv.chunks(k).zip(tv.chunks(k)) {
            out.push(a.iter().zip(b).any(|(x, y)| x.to_bits() != y.to_bits()));
        }
    }
    out
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

/// An extra contribution to the Newton driver (ADR 0440 D7): a force added to the internal force and a tangent added to
/// the tangent at every iteration. An external load that depends on the displacement (a follower pressure) enters with a
/// minus sign; a contact force enters as it is.
pub trait Contribution {
    /// The name used in the driver's errors.
    fn name(&self) -> &str;
    /// Adds the contribution at the displacement `u` (one value per equation of the model's `DofMap`) and the attempt's
    /// load factor: `force` (zero on entry, one value per equation) and the tangent as `(row, column, value)` triplets in
    /// equations. Triplets off the tangent's pattern extend it.
    fn add(&self, u: &[f64], load_factor: f64, force: &mut [f64], tangent: &mut Vec<(usize, usize, f64)>)
        -> Result<()>;
    /// Whether the reference of the driver's relative residual is also at least the norm of this contribution's force and
    /// of the internal force without the contributions (ADR 0479 D2: an inertia force that balances the internal force of
    /// a free body). `false` (the default) keeps the reference of ADR 0404.
    fn in_residual_reference(&self) -> bool {
        false
    }
    /// An extra lower bound of the reference of the driver's relative residual at `(u, load_factor)` (ADR 0603 D3), read
    /// only when [`Contribution::in_residual_reference`] is `true` and taken only when `> 0`: the scale of the terms whose
    /// cancellation the contribution's force is. The default `0.0` keeps the reference of ADR 0479 D2 bit for bit.
    fn residual_reference_terms(&self, _u: &[f64], _load_factor: f64) -> f64 {
        0.0
    }
    /// Adds the derivative of this contribution's force with the load factor at `(u, load_factor)` to `out` (one value per
    /// equation), for the load direction of the arc-length iteration with `ArcLengthOptions::contribution_load` (ADR 0560
    /// D1, D3). The default adds nothing: a force that does not depend on the load factor.
    fn add_load_factor_derivative(&self, u: &[f64], load_factor: f64, out: &mut [f64]) -> Result<()> {
        let _ = (u, load_factor, out);
        Ok(())
    }
    /// The contribution with its full (possibly unsymmetric) tangent, called instead of [`Contribution::add`] when
    /// [`NewtonOptions::unsymmetric`] is set (ADR 0560 D1, D4). The default is [`Contribution::add`].
    fn add_full(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> Result<()> {
        self.add(u, load_factor, force, tangent)
    }
    /// Whether the contribution has a tangent without its curvature part, taken by the driver for an iteration whose exact
    /// tangent fails the pivot test (ADR 0570 D1, D2). The default is `false`: the driver then never changes its matrix.
    fn has_fallback_tangent(&self) -> bool {
        false
    }
    /// The triplets of the tangent without the curvature part at `(u, load_factor)` (no force; ADR 0570 D1). Called only
    /// when [`Contribution::has_fallback_tangent`] is true; the default adds nothing.
    fn add_fallback_tangent(&self, u: &[f64], load_factor: f64, tangent: &mut Vec<(usize, usize, f64)>) -> Result<()> {
        let _ = (u, load_factor, tangent);
        Ok(())
    }
    /// The force of [`Contribution::add`] at `(u, load_factor)` for a trial evaluation of the line search (ADR 0607 D2),
    /// without side effects. The default calls `add` with a scratch tangent that is dropped; a contribution that records its
    /// evaluations overrides it.
    fn add_trial_force(&self, u: &[f64], load_factor: f64, force: &mut [f64]) -> Result<()> {
        let mut scratch = Vec::new();
        self.add(u, load_factor, force, &mut scratch)
    }
}

/// The step-control hook of the Newton driver (ADR 0440 D7): it sets the load factor each attempt aims at and is told the
/// outcome. Halving and the load steps stay the driver's.
pub trait StepControl {
    /// The name used in the driver's errors.
    fn name(&self) -> &str;
    /// The target of the next attempt from the committed load factor; `proposed` is the load-control target the driver
    /// would take, `step` the user load step (1-based), `halvings` the halvings of that step so far. The value must lie in
    /// `(committed, target of the user step]`.
    fn load_factor(&mut self, committed: f64, proposed: f64, step: usize, halvings: usize) -> Result<f64>;
    /// After a converged attempt.
    fn converged(&mut self, increment: &LoadStep) {
        let _ = increment;
    }
    /// After a failed attempt from `committed` to `attempted`.
    fn failed(&mut self, committed: f64, attempted: f64) {
        let _ = (committed, attempted);
    }
}

/// The extension points given to [`solve_with`]; the default (no contribution, no hook) is [`solve`].
#[derive(Default)]
pub struct DriverExtensions<'a> {
    /// Extra contributions, called in list order.
    pub contributions: Vec<&'a dyn Contribution>,
    /// The step-control hook.
    pub step_control: Option<&'a mut dyn StepControl>,
}

/// Adds `triplets` to `a`: entries on the pattern in place, in triplet order; if any entry is off the pattern, the matrix
/// is rebuilt on the union pattern by [`CsrMatrix::from_triplets`] of every entry (ADR 0440 D7).
pub fn add_triplets(a: &mut CsrMatrix, triplets: &[(usize, usize, f64)]) {
    let mut outside = Vec::new();
    for &(r, c, v) in triplets {
        let row = &a.col[a.row_ptr[r]..a.row_ptr[r + 1]];
        match row.binary_search(&c) {
            Ok(k) => {
                let at = a.row_ptr[r] + k;
                a.val[at] += v;
            }
            Err(_) => outside.push((r, c, v)),
        }
    }
    if outside.is_empty() {
        return;
    }
    let n = a.row_ptr.len() - 1;
    let mut all = Vec::with_capacity(a.val.len() + outside.len());
    for r in 0..n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            all.push((r, a.col[k], a.val[k]));
        }
    }
    all.extend(outside);
    *a = CsrMatrix::from_triplets(n, all);
}

/// The contributions at `u` and `load_factor`: the force added to `fint` and the triplets to `k` (if given). Returns the
/// extra reference of the relative residual (ADR 0479 D2): if a contribution is in the reference, the largest of the norm
/// of `fint` on entry and of the forces of those contributions, else 0.
fn contribute(
    contributions: &[&dyn Contribution],
    u: &[f64],
    load_factor: f64,
    fint: &mut [f64],
    k: Option<&mut CsrMatrix>,
) -> std::result::Result<f64, String> {
    contribute_with(contributions, u, load_factor, fint, k, false).map(|(r, _)| r)
}

/// [`contribute`], with `full` the contributions by [`Contribution::add_full`] (ADR 0560 D4); also whether a contribution
/// entered the reference (ADR 0604 D2: the rounding bound of the last iterate applies).
fn contribute_with(
    contributions: &[&dyn Contribution],
    u: &[f64],
    load_factor: f64,
    fint: &mut [f64],
    k: Option<&mut CsrMatrix>,
    full: bool,
) -> std::result::Result<(f64, bool), String> {
    let mut triplets = Vec::new();
    let mut reference = 0.0f64;
    let mut bounded = false;
    if contributions.iter().any(|c| c.in_residual_reference()) {
        reference = norm(fint.iter().copied());
    }
    for c in contributions {
        let mut force = vec![0.0; u.len()];
        if full {
            c.add_full(u, load_factor, &mut force, &mut triplets)
        } else {
            c.add(u, load_factor, &mut force, &mut triplets)
        }
        .map_err(|e| format!("contribution '{}': {e}", c.name()))?;
        if c.in_residual_reference() {
            bounded = true;
            reference = reference.max(norm(force.iter().copied()));
            // ADR 0603 D3: the contribution's terms, only when it gives a positive bound.
            let terms = c.residual_reference_terms(u, load_factor);
            if terms > 0.0 {
                reference = reference.max(terms);
            }
        }
        for (a, b) in fint.iter_mut().zip(&force) {
            *a += b;
        }
    }
    if let Some(k) = k {
        add_triplets(k, &triplets);
    }
    Ok((reference, bounded))
}

/// [`contribute`] for the fallback of ADR 0570 D2: the same forces, the same reference and the same triplets added to `k`
/// in the same order, and also the matrix `k` before the contributions, each contribution's own triplets and whether a
/// contribution entered the reference (ADR 0604 D2).
#[allow(clippy::type_complexity)]
fn contribute_split(
    contributions: &[&dyn Contribution],
    u: &[f64],
    load_factor: f64,
    fint: &mut [f64],
    k: &mut CsrMatrix,
) -> std::result::Result<(f64, CsrMatrix, Vec<Vec<(usize, usize, f64)>>, bool), String> {
    let before = k.clone();
    let mut per = Vec::with_capacity(contributions.len());
    let mut reference = 0.0f64;
    let mut bounded = false;
    if contributions.iter().any(|c| c.in_residual_reference()) {
        reference = norm(fint.iter().copied());
    }
    for c in contributions {
        let mut force = vec![0.0; u.len()];
        let mut triplets = Vec::new();
        c.add(u, load_factor, &mut force, &mut triplets)
            .map_err(|e| format!("contribution '{}': {e}", c.name()))?;
        if c.in_residual_reference() {
            bounded = true;
            reference = reference.max(norm(force.iter().copied()));
            // ADR 0603 D3: the contribution's terms, only when it gives a positive bound.
            let terms = c.residual_reference_terms(u, load_factor);
            if terms > 0.0 {
                reference = reference.max(terms);
            }
        }
        for (a, b) in fint.iter_mut().zip(&force) {
            *a += b;
        }
        per.push(triplets);
    }
    add_triplets(k, &per.concat());
    Ok((reference, before, per, bounded))
}

/// The rounding bound of the residual at the iterate `u` (ADR 0604 D1): `eps |(|K| |u|)|` over the free equations, `K` the
/// assembled tangent with the contributions. The residual that one unit roundoff in every displacement and an equal
/// evaluation error can produce; no float vector is expected to do better.
fn rounding_bound(k: &CsrMatrix, u: &[f64], free: &[usize]) -> f64 {
    let mut s = 0.0f64;
    for &i in free {
        let row: f64 = (k.row_ptr[i]..k.row_ptr[i + 1])
            .map(|p| k.val[p].abs() * u[k.col[p]].abs())
            .sum();
        s += row * row;
    }
    f64::EPSILON * s.sqrt()
}

/// The per-element layout of the material states of a model (ADR 0440 D3): per element the number of points of its
/// stiffness rule and the values per point (`Material::state_size`). With the warm start of ADR 0660 D1 the state of an
/// Incompatible Hex8 carries its nine committed modes after these `points x values_per_point` values (ADR 0668 D2): the
/// layout describes the material part only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateLayout {
    points: Vec<usize>,
    per_point: Vec<usize>,
    offsets: Vec<usize>,
}

impl StateLayout {
    /// The layout of `model` (every element must have exactly one material; a pending variant is not refused: this is a
    /// layout, not a solve).
    pub fn of(model: &SolidModel) -> Result<StateLayout> {
        let materials = model.material_indices()?;
        let mesh = model.mesh();
        let mut points = Vec::with_capacity(mesh.n_elements());
        let mut per_point = Vec::with_capacity(mesh.n_elements());
        let mut offsets = Vec::with_capacity(mesh.n_elements() + 1);
        offsets.push(0);
        for (e, &m) in materials.iter().enumerate() {
            let n = crate::element::element_for(mesh.element_kind(e)).stiffness_rule().len();
            let k = model.materials()[m].1.state_size();
            points.push(n);
            per_point.push(k);
            offsets.push(offsets[e] + n * k);
        }
        Ok(StateLayout {
            points,
            per_point,
            offsets,
        })
    }
    /// Points of the stiffness rule of element `e`.
    pub fn points(&self, e: usize) -> usize {
        self.points[e]
    }
    /// State values per point of element `e`.
    pub fn values_per_point(&self, e: usize) -> usize {
        self.per_point[e]
    }
    /// The offset of element `e` in one array of all states (element-major, then point-major).
    pub fn offset(&self, e: usize) -> usize {
        self.offsets[e]
    }
    /// The number of state values of the model.
    pub fn total(&self) -> usize {
        *self.offsets.last().expect("one offset at least")
    }
    /// Zero states of every element.
    pub fn empty_states(&self) -> Vec<ElementState> {
        self.points
            .iter()
            .zip(&self.per_point)
            .map(|(n, k)| ElementState::new(vec![0.0; n * k]))
            .collect()
    }
}

/// What the driver works with.
struct Problem<'a> {
    model: &'a SolidModel,
    f: Vec<f64>,
    fixed: Vec<(usize, f64)>,
    free: Vec<usize>,
    options: &'a NewtonOptions,
    contributions: &'a [&'a dyn Contribution],
    /// The cached analysis of the supernodal factor (ADR 0566 D2); not read by the envelope factor.
    reuse: &'a FactorReuse,
    /// The point context of every evaluation (ADR 0485 D3); the default for the static driver.
    context: PointContext,
    /// A nodal temperature field of the attempt (ADR 0534 D2); `None` for every path without one.
    field: Option<&'a FieldAt>,
}

impl Problem<'_> {
    /// [`Problem::attempt_with`] without a load-factor control.
    fn attempt(
        &self,
        u_c: &[f64],
        lc: f64,
        lt: f64,
        states_c: &[ElementState],
    ) -> std::result::Result<Attempt, Failure> {
        self.attempt_with(u_c, lc, lt, states_c, None)
    }

    /// One attempt from the committed `(u_c, lc, states_c)` to the load factor `lt` (ADR 0404 D5 step 4). With a control
    /// (ADR 0480 D4) the load factor starts at `lt` and changes at every iteration by the control's correction. ADR 0694 D3:
    /// a failed attempt whose predictor was backtracked names its step length at the end of its cause.
    fn attempt_with(
        &self,
        u_c: &[f64],
        lc: f64,
        lt: f64,
        states_c: &[ElementState],
        control: Option<&mut dyn LoadFactorControl>,
    ) -> std::result::Result<Attempt, Failure> {
        let backtracked = Cell::new(None);
        match self.iterate(u_c, lc, lt, states_c, control, &backtracked) {
            Err(failure) => match backtracked.get() {
                Some(beta) => Err(backtracked_failure(failure, beta)),
                None => Err(failure),
            },
            ok => ok,
        }
    }

    /// The iteration of [`Problem::attempt_with`]; `backtracked` receives the step length of a backtracked predictor (ADR
    /// 0694 D3) when it is below 1.
    fn iterate(
        &self,
        u_c: &[f64],
        lc: f64,
        lt: f64,
        states_c: &[ElementState],
        mut control: Option<&mut dyn LoadFactorControl>,
        backtracked: &Cell<Option<f64>>,
    ) -> std::result::Result<Attempt, Failure> {
        let o = self.options;
        let mut lt = lt;
        let mut u = u_c.to_vec();
        let mut residuals = Vec::new();
        let mut energies = Vec::new();
        let (mut yielded, mut set_changed) = (Vec::new(), Vec::new());
        let mut step_lengths = Vec::new();
        // ADR 0607 D1: the line search, decided once per attempt.
        let search = o.line_search && control.is_none() && !o.unsymmetric;
        // ADR 0688 D3: the damping of the predictor, decided once per attempt; refused with a contribution.
        let damp = o.predictor_damping && control.is_none() && !o.unsymmetric;
        if o.predictor_damping && !self.contributions.is_empty() {
            return Err(Failure::Other(
                "predictor_damping with a contribution: the trial tangent would need the contributions' tangents (ADR \
                 0688 D3)"
                    .to_string(),
            ));
        }
        // ADR 0694 D3: the backtracking of the predictor, decided once per attempt; refused with a contribution.
        let back = o.predictor_backtracking && control.is_none() && !o.unsymmetric;
        if o.predictor_backtracking && !self.contributions.is_empty() {
            return Err(Failure::Other(
                "predictor_backtracking with a contribution: the trial tangent would need the contributions' tangents \
                 (ADR 0694 D3)"
                    .to_string(),
            ));
        }
        let mut previous_set: Option<Vec<bool>> = None;
        let (mut e_first, mut e_last) = (0.0, 0.0);
        let dofs = self.model.dof_map();
        // ADR 0534 amendment 1: with a field whose expansion acts, the thermal force f_int(0) at the empty states enters the
        // reference of the residual.
        // ADR 0570 D2: a contribution with a tangent without its curvature part makes the attempt keep the element tangent.
        let fallback =
            control.is_none() && !o.unsymmetric && self.contributions.iter().any(|c| c.has_fallback_tangent());
        let thermal_reference = match self.field {
            Some(field) if field.expansion => {
                let empty = initial_states(self.model).map_err(classify)?;
                let f0 = internal_force_field(self.model, &vec![0.0; u.len()], &empty, &self.context, Some(field))
                    .map_err(classify)?;
                norm(self.free.iter().map(|&i| f0[i]))
            }
            _ => 0.0,
        };
        // ADR 0619 D1: the residual of each iterate and whether it was within the rounding bound (only with the opt-in).
        let mut floor: Vec<(f64, bool)> = Vec::new();
        for k in 0..=o.max_iterations {
            // The trial states: a material with history updates them, always from the committed states (ADR 0475 D3);
            // the linear material has none (D5).
            let (mut fint, mut kt, trial) =
                force_and_tangent(self.model, &u, states_c, o.threads, &self.context, self.field).map_err(classify)?;
            // ADR 0479 D2: the internal force without the contributions is kept for the converged iterate, and a
            // contribution may extend the reference of the residual.
            let mut elastic = None;
            let mut extra_reference = 0.0;
            let mut split = None;
            let mut bounded = false;
            if !self.contributions.is_empty() {
                elastic = Some(fint.clone());
                (extra_reference, bounded) = if fallback {
                    let (r, ke, per, b) =
                        contribute_split(self.contributions, &u, lt, &mut fint, &mut kt).map_err(Failure::Other)?;
                    split = Some((ke, per));
                    (r, b)
                } else {
                    contribute_with(self.contributions, &u, lt, &mut fint, Some(&mut kt), o.unsymmetric)
                        .map_err(Failure::Other)?
                };
            }
            let rfull: Vec<f64> = self.f.iter().zip(&fint).map(|(fi, gi)| lt * fi - gi).collect();
            let rn = norm(self.free.iter().map(|&i| rfull[i]));
            let mut reference = norm(self.free.iter().map(|&i| lt * self.f[i])).max(norm(fint.iter().copied()));
            if extra_reference > 0.0 {
                reference = reference.max(extra_reference);
            }
            if thermal_reference > 0.0 {
                reference = reference.max(thermal_reference);
            }
            let r = if reference > 0.0 { rn / reference } else { rn };
            if !r.is_finite() {
                return Err(Failure::Other(format!("non-finite residual at iteration {k}")));
            }
            residuals.push(r);
            if o.record_active_set {
                let layout = StateLayout::of(self.model).map_err(classify)?;
                let set = yielded_points(&layout, states_c, &trial, self.model.incompatible_mode_warm_start());
                yielded.push(set.iter().filter(|&&y| y).count());
                set_changed.push(previous_set.as_ref().is_some_and(|p| *p != set));
                previous_set = Some(set);
            }
            if k >= 1 && r <= o.residual_tolerance && e_last <= o.energy_tolerance * e_first {
                return Ok(Attempt {
                    u,
                    states: trial,
                    residuals,
                    energies,
                    yielded,
                    set_changed,
                    step_lengths,
                    fint: elastic.unwrap_or(fint),
                });
            }
            // ADR 0619 D1: the stagnation acceptance (opt-in): this iterate and the two before it within the rounding bound,
            // the last two corrections did not halve the residual, the energy test holds.
            if o.stagnation {
                let within = bounded && rn <= rounding_bound(&kt, &u, &self.free);
                floor.push((rn, within));
                if within
                    && k >= 3
                    && k < o.max_iterations
                    && floor[k - 1].1
                    && floor[k - 2].1
                    && rn > 0.5 * floor[k - 2].0
                    && e_last <= o.energy_tolerance * e_first
                {
                    return Ok(Attempt {
                        u,
                        states: trial,
                        residuals,
                        energies,
                        yielded,
                        set_changed,
                        step_lengths,
                        fint: elastic.unwrap_or(fint),
                    });
                }
            }
            // ADR 0604 D2: at the last iterate, a residual at the rounding bound of a step with a contribution in the
            // reference is accepted (the relative test above is unchanged).
            if k == o.max_iterations
                && k >= 1
                && bounded
                && e_last <= o.energy_tolerance * e_first
                && rn <= rounding_bound(&kt, &u, &self.free)
            {
                return Ok(Attempt {
                    u,
                    states: trial,
                    residuals,
                    energies,
                    yielded,
                    set_changed,
                    step_lengths,
                    fint: elastic.unwrap_or(fint),
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
            let mut sys = reduce(&kt, &rfull, &increment).map_err(|e| Failure::Other(e.to_string()))?;
            // ADR 0480 D4: with a control the correction carries the change of the load factor.
            let mut rhs_control = None;
            let du = match control.as_deref_mut() {
                None => {
                    if sys.free().is_empty() {
                        Vec::new()
                    } else if o.unsymmetric {
                        factor_solve_unsymmetric(&sys.matrix, &sys.rhs, sys.free(), &dofs)?
                    } else if let Some((ke, per)) = &split {
                        let exact = factor_solve(
                            &sys.matrix,
                            &sys.rhs,
                            sys.free(),
                            &dofs,
                            o.factor,
                            self.reuse,
                            o.indefinite_tangent,
                        );
                        match exact {
                            Ok(du) => du,
                            // ADR 0570 D2: the exact tangent failed the pivot test; this iteration takes the contributions'
                            // tangents without their curvature part.
                            Err(Failure::Other(m)) if m.starts_with("singular tangent") => {
                                let mut trip = Vec::new();
                                for (c, t) in self.contributions.iter().zip(per) {
                                    if c.has_fallback_tangent() {
                                        c.add_fallback_tangent(&u, lt, &mut trip)
                                            .map_err(|e| Failure::Other(format!("contribution '{}': {e}", c.name())))?;
                                    } else {
                                        trip.extend_from_slice(t);
                                    }
                                }
                                let mut kf = ke.clone();
                                add_triplets(&mut kf, &trip);
                                sys = reduce(&kf, &rfull, &increment).map_err(|e| Failure::Other(e.to_string()))?;
                                factor_solve(
                                    &sys.matrix,
                                    &sys.rhs,
                                    sys.free(),
                                    &dofs,
                                    o.factor,
                                    self.reuse,
                                    o.indefinite_tangent,
                                )
                                .map_err(|f| match f {
                                    Failure::Other(m) => Failure::Other(format!("without the curvature part: {m}")),
                                    other => other,
                                })?
                            }
                            Err(e) => return Err(e),
                        }
                    } else {
                        factor_solve(
                            &sys.matrix,
                            &sys.rhs,
                            sys.free(),
                            &dofs,
                            o.factor,
                            self.reuse,
                            o.indefinite_tangent,
                        )?
                    }
                }
                Some(c) => {
                    // ADR 0560 D3: the load direction with the contributions' derivatives, at the iterate.
                    let f_lambda = if c.contribution_load() {
                        Some(load_direction(&self.f, self.contributions, &u, lt).map_err(Failure::Other)?)
                    } else {
                        None
                    };
                    let f = f_lambda.as_deref().unwrap_or(&self.f);
                    let (du, rhs, dl) = controlled_correction(&sys, f, k, &dofs, c, o.unsymmetric)?;
                    lt += dl;
                    rhs_control = Some(rhs);
                    du
                }
            };
            if du.iter().any(|v| !v.is_finite()) {
                return Err(Failure::Other(format!("non-finite correction at iteration {k}")));
            }
            let e = du
                .iter()
                .zip(rhs_control.as_deref().unwrap_or(&sys.rhs))
                .map(|(a, b)| a * b)
                .sum::<f64>()
                .abs();
            if k == 0 {
                e_first = e;
            }
            e_last = e;
            energies.push(e);
            if search || damp || back {
                // ADR 0607 D1: only an iteration without a prescribed increment is searched; ADR 0688 D3: one with it is
                // damped from the lifted iterate; ADR 0694 D3: or backtracked from the full step.
                let alpha = if increment.iter().all(|&(_, v)| v == 0.0) {
                    if search {
                        self.step_length(&u, k, lt, states_c, sys.free(), &du, &rfull)
                    } else {
                        1.0
                    }
                } else if damp {
                    self.predictor_length(&u, lt, states_c, sys.free(), &du)
                } else if back {
                    let beta = self.predictor_backtrack(&u, lt, states_c, sys.free(), &du);
                    if beta < 1.0 {
                        backtracked.set(Some(beta));
                    }
                    beta
                } else {
                    1.0
                };
                step_lengths.push(alpha);
                if alpha == 1.0 {
                    for (&q, d) in sys.free().iter().zip(&du) {
                        u[q] += d;
                    }
                } else {
                    for (&q, d) in sys.free().iter().zip(&du) {
                        u[q] += alpha * d;
                    }
                }
            } else {
                for (&q, d) in sys.free().iter().zip(&du) {
                    u[q] += d;
                }
            }
            if k == 0 {
                for &(eq, v) in &self.fixed {
                    u[eq] = lt * v;
                }
            }
        }
        unreachable!("the loop returns at k = max_iterations")
    }

    /// The derivative of the energy along the correction `du` (on the equations `free`) at the trial `u + alpha du` (ADR
    /// 0607 D1): `-du . r` on the free equations, the fixed equations at their values after iteration `k`, the contributions
    /// by [`Contribution::add_trial_force`]. `None` for a failed evaluation or a non-finite value.
    #[allow(clippy::too_many_arguments)]
    fn trial_slope(
        &self,
        u: &[f64],
        k: usize,
        lt: f64,
        states_c: &[ElementState],
        free: &[usize],
        du: &[f64],
        alpha: f64,
        pivot_test: bool,
    ) -> Option<f64> {
        let mut x = u.to_vec();
        for (&q, d) in free.iter().zip(du) {
            x[q] += alpha * d;
        }
        if k == 0 {
            for &(eq, v) in &self.fixed {
                x[eq] = lt * v;
            }
        }
        let (mut fint, kt, _) = force_and_tangent(
            self.model,
            &x,
            states_c,
            self.options.threads,
            &self.context,
            self.field,
        )
        .ok()?;
        for c in self.contributions {
            let mut force = vec![0.0; x.len()];
            c.add_trial_force(&x, lt, &mut force).ok()?;
            for (a, b) in fint.iter_mut().zip(&force) {
                *a += b;
            }
        }
        let s = -free
            .iter()
            .zip(du)
            .map(|(&q, d)| d * (lt * self.f[q] - fint[q]))
            .sum::<f64>();
        if !s.is_finite() {
            return None;
        }
        // ADR 0688 D3: a trial of the damped predictor is admissible only when its reduced tangent passes the pivot test.
        if pivot_test {
            let zero: Vec<(usize, f64)> = self.fixed.iter().map(|&(eq, _)| (eq, 0.0)).collect();
            let sys = reduce(&kt, &vec![0.0; x.len()], &zero).ok()?;
            if !sys.free().is_empty() {
                factor_solve(
                    &sys.matrix,
                    &sys.rhs,
                    sys.free(),
                    &self.model.dof_map(),
                    self.options.factor,
                    self.reuse,
                    false,
                )
                .ok()?;
            }
        }
        Some(s)
    }

    /// ADR 0719 D2: the inertia of the state `u` with the trial states from the committed `states_c`: the reduced tangent
    /// there (the driver's assembly, zero increments on the prescribed equations) by [`negative_pivots`]; `Some(0)` with no
    /// free equation, `None` when the assembly fails or a pivot is refused.
    fn inertia_at(&self, u: &[f64], states_c: &[ElementState]) -> Option<usize> {
        let (_, kt, _) =
            force_and_tangent(self.model, u, states_c, self.options.threads, &self.context, self.field).ok()?;
        let zero: Vec<(usize, f64)> = self.fixed.iter().map(|&(eq, _)| (eq, 0.0)).collect();
        let sys = reduce(&kt, &vec![0.0; u.len()], &zero).ok()?;
        if sys.free().is_empty() {
            return Some(0);
        }
        negative_pivots(&sys.matrix, self.options.factor)
    }

    /// The step length `beta` of the damped predictor (ADR 0688 D3): the rule of ADR 0607 D1 from the lifted iterate (`u`
    /// with the prescribed values at `lt`), `s_0` by a trial evaluation there, every trial admissible only when its reduced
    /// tangent passes the pivot test; 1 when `s_0` is not negative or its evaluation fails.
    fn predictor_length(&self, u: &[f64], lt: f64, states_c: &[ElementState], free: &[usize], du: &[f64]) -> f64 {
        match self.trial_slope(u, 0, lt, states_c, free, du, 0.0, false) {
            Some(s0) if s0 < 0.0 => self.search_from(s0, u, 0, lt, states_c, free, du, true),
            _ => 1.0,
        }
    }

    /// The step length `beta` of the backtracked predictor (ADR 0694 D3): the first of `1, 1/2, ..., 2^-PREDICTOR_HALVINGS`
    /// whose trial (the prescribed values at `lt`) is admissible, the evaluation succeeding and the reduced tangent passing
    /// the driver's positive pivot test; 1 when none is.
    fn predictor_backtrack(&self, u: &[f64], lt: f64, states_c: &[ElementState], free: &[usize], du: &[f64]) -> f64 {
        let mut beta = 1.0;
        for _ in 0..=PREDICTOR_HALVINGS {
            if self.trial_slope(u, 0, lt, states_c, free, du, beta, true).is_some() {
                return beta;
            }
            beta *= 0.5;
        }
        1.0
    }

    /// The step length of the line search (ADR 0607 D1): 1 when the full step passes `s(1) <= eta |s_0|`, else the Illinois
    /// regula falsi on `(0, 1)` until `|s(alpha)| <= eta |s_0|` or 8 further trials (the last trial's length).
    #[allow(clippy::too_many_arguments)]
    fn step_length(
        &self,
        u: &[f64],
        k: usize,
        lt: f64,
        states_c: &[ElementState],
        free: &[usize],
        du: &[f64],
        rfull: &[f64],
    ) -> f64 {
        let s0 = -free.iter().zip(du).map(|(&q, d)| d * rfull[q]).sum::<f64>();
        if s0.is_nan() || s0 >= 0.0 {
            return 1.0;
        }
        self.search_from(s0, u, k, lt, states_c, free, du, false)
    }

    /// The search of [`Problem::step_length`] from the slope `s0 < 0` at `alpha = 0` (ADR 0607 D1); with `admissible` (ADR
    /// 0688 D3) every trial must also pass the pivot test (a refused trial is an overshoot), and when the trials are spent
    /// the bracket's lower end is taken if it is above 0.
    #[allow(clippy::too_many_arguments)]
    fn search_from(
        &self,
        s0: f64,
        u: &[f64],
        k: usize,
        lt: f64,
        states_c: &[ElementState],
        free: &[usize],
        du: &[f64],
        admissible: bool,
    ) -> f64 {
        const ETA: f64 = 0.5;
        const TRIALS: usize = 8;
        let tol = ETA * s0.abs();
        let s1 = self.trial_slope(u, k, lt, states_c, free, du, 1.0, admissible);
        if let Some(v) = s1 {
            if v <= tol {
                return 1.0;
            }
        }
        // The bracket: s_a < 0 at a, s_b > 0 (or a failed evaluation, None) at b.
        let (mut a, mut sa) = (0.0f64, s0);
        let (mut b, mut sb) = (1.0f64, s1);
        // The end retained at the last trial: -1 a, +1 b, 0 none (the Illinois halving).
        let mut kept = 0i8;
        let mut alpha = 1.0;
        for _ in 0..TRIALS {
            alpha = match sb {
                Some(vb) => (a * vb - b * sa) / (vb - sa),
                None => 0.5 * (a + b),
            };
            if alpha.is_nan() || alpha <= a || alpha >= b {
                alpha = 0.5 * (a + b);
            }
            let s = self.trial_slope(u, k, lt, states_c, free, du, alpha, admissible);
            if let Some(v) = s {
                if v.abs() <= tol {
                    return alpha;
                }
            }
            match s {
                Some(v) if v < 0.0 => {
                    a = alpha;
                    sa = v;
                    if kept == 1 {
                        sb = sb.map(|x| 0.5 * x);
                    }
                    kept = 1;
                }
                _ => {
                    b = alpha;
                    sb = s;
                    if kept == -1 {
                        sa *= 0.5;
                    }
                    kept = -1;
                }
            }
        }
        if admissible && a > 0.0 {
            a
        } else {
            alpha
        }
    }
}

/// The nested dissection order and symbolic analysis of the supernodal factor kept between the factorisations of the Newton
/// driver (ADR 0566 D2): the analysis of the last reduced tangent's pattern, used again while the pattern is unchanged and
/// made again when it changes. The factor's bits are those without the reuse. The envelope factor does not use it.
#[derive(Debug, Default)]
pub struct FactorReuse {
    analysis: RefCell<Option<SupernodalAnalysis>>,
    analyses: Cell<usize>,
    factorisations: Cell<usize>,
}

impl FactorReuse {
    /// An empty cache.
    pub fn new() -> FactorReuse {
        FactorReuse::default()
    }

    /// The analyses made (one per new pattern).
    pub fn analyses(&self) -> usize {
        self.analyses.get()
    }

    /// The supernodal factorisations made.
    pub fn factorisations(&self) -> usize {
        self.factorisations.get()
    }
}

/// The envelope `L D L^T` solve of the reduced tangent with the pivot test of the static solve (ADR 0275 arithmetic 4), or
/// with `factor` [`DirectFactor::SupernodalNestedDissection`] the supernodal one with the same tests and texts (ADR 0542 D2),
/// its order and symbolic analysis taken from `reuse` while the pattern is unchanged (ADR 0566 D2).
#[allow(clippy::too_many_arguments)]
fn factor_solve(
    a: &CsrMatrix,
    b: &[f64],
    free: &[usize],
    dofs: &crate::model::DofMap,
    factor: DirectFactor,
    reuse: &FactorReuse,
    indefinite: bool,
) -> std::result::Result<Vec<f64>, Failure> {
    let diag = a.diag();
    let at = |row: usize| {
        let (n, c) = dofs.node_component(free[row]).expect("free equation exists");
        format!("node {n} component {c}")
    };
    if indefinite {
        return factor_solve_indefinite(a, b, &diag, &at, factor);
    }
    if factor == DirectFactor::SupernodalNestedDissection {
        let mut slot = reuse.analysis.borrow_mut();
        if !slot.as_ref().is_some_and(|x| x.matches(a)) {
            let made = SupernodalAnalysis::nested_dissection(a)
                .map_err(|e| Failure::Other(format!("tangent factorisation: {e}")))?;
            *slot = Some(made);
            reuse.analyses.set(reuse.analyses.get() + 1);
        }
        let analysis = slot.as_ref().expect("the analysis of this pattern");
        reuse.factorisations.set(reuse.factorisations.get() + 1);
        let ldlt = SupernodalLdlt::factor_analysed(a, analysis).map_err(|e| match e {
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
        return Ok(ldlt.solve(b));
    }
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

/// ADR 0694 D2: the solve of [`factor_solve`] with the `L D L^T` of non-zero pivots (the same orders: reverse Cuthill-McKee
/// for the envelope, nested dissection for the supernodal factor, made without the analysis cache) and the pivot test of the
/// arc-length correction, `|d| <= PIVOT_TOLERANCE |a_ii|` refused, with the texts of [`factor_solve`].
fn factor_solve_indefinite(
    a: &CsrMatrix,
    b: &[f64],
    diag: &[f64],
    at: &dyn Fn(usize) -> String,
    factor: DirectFactor,
) -> std::result::Result<Vec<f64>, Failure> {
    let pivot_error = |e: SkylineError| match e {
        SkylineError::Pivot { row, pivot, .. } => Failure::Other(format!(
            "singular tangent: pivot {pivot:e} at {} (diagonal {:e})",
            at(row),
            diag[row]
        )),
        other => Failure::Other(format!("tangent factorisation: {other}")),
    };
    let check = |order: &[usize], pivots: &[f64]| -> std::result::Result<(), Failure> {
        for (&row, &d) in order.iter().zip(pivots) {
            if d.is_nan() || d.abs() <= PIVOT_TOLERANCE * diag[row].abs() {
                return Err(Failure::Other(format!(
                    "singular tangent: pivot {d:e} at {} (diagonal {:e})",
                    at(row),
                    diag[row]
                )));
            }
        }
        Ok(())
    };
    if factor == DirectFactor::SupernodalNestedDissection {
        let options = SparseOptions {
            ordering: SparseOrdering::NestedDissection,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        };
        let ldlt = SupernodalLdlt::factor_with(a, &options).map_err(pivot_error)?;
        check(ldlt.order(), ldlt.pivots())?;
        return Ok(ldlt.solve(b));
    }
    let options = SkylineOptions {
        pivots: PivotRule::NonZero,
        ..SkylineOptions::default()
    };
    let ldlt = SkylineLdlt::factor_with(a, &options).map_err(pivot_error)?;
    check(ldlt.order(), ldlt.pivots())?;
    Ok(ldlt.solve(b))
}

/// ADR 0694 D3: a failure of an attempt whose predictor was backtracked to `beta`, with the step length at the end of its
/// cause (the reason of a located inversion, or the text).
fn backtracked_failure(failure: Failure, beta: f64) -> Failure {
    let note = format!("; the predictor backtracked to beta = {beta:e} (ADR 0694 D3)");
    match failure {
        Failure::Other(cause) => Failure::Other(format!("{cause}{note}")),
        Failure::Inverted(FemError::Element { element, source }) => Failure::Inverted(FemError::Element {
            element,
            source: Box::new(match *source {
                FemError::InvalidValue { key, reason } => FemError::InvalidValue {
                    key,
                    reason: format!("{reason}{note}"),
                },
                other => other,
            }),
        }),
        other => other,
    }
}

/// ADR 0719 D1: the attempt of `problem`, and when it fails with a refused factor the same attempt of `retry` (the options
/// with the indefinite tangent); `true` with an attempt accepted by the retry. A retry that fails gives its failure with the
/// first cause appended.
fn retrying(
    problem: &Problem<'_>,
    retry: &Problem<'_>,
    u_c: &[f64],
    lc: f64,
    lt: f64,
    states_c: &[ElementState],
) -> std::result::Result<(Attempt, bool), Failure> {
    match problem.attempt(u_c, lc, lt, states_c) {
        Ok(a) => Ok((a, false)),
        Err(Failure::Other(first)) if first.starts_with("singular tangent") => {
            match retry.attempt(u_c, lc, lt, states_c) {
                Ok(a) => Ok((a, true)),
                Err(failure) => Err(retried_failure(failure, &first)),
            }
        }
        Err(failure) => Err(failure),
    }
}

/// ADR 0719 D1: the failure of a retry after the refused factor `first`, with that cause at the end of its own (the reason of
/// a located inversion, or the text).
fn retried_failure(failure: Failure, first: &str) -> Failure {
    let note = format!("; retried with the indefinite tangent (ADR 0719 D1) after: {first}");
    match failure {
        Failure::Other(cause) => Failure::Other(format!("{cause}{note}")),
        Failure::Inverted(FemError::Element { element, source }) => Failure::Inverted(FemError::Element {
            element,
            source: Box::new(match *source {
                FemError::InvalidValue { key, reason } => FemError::InvalidValue {
                    key,
                    reason: format!("{reason}{note}"),
                },
                other => other,
            }),
        }),
        other => other,
    }
}

/// ADR 0719 D2: the number of negative pivots of the `L D L^T` with non-zero pivots of `a` by the factor `factor` (the
/// factors of [`factor_solve_indefinite`]); `None` when a pivot is refused (`|d| <= PIVOT_TOLERANCE |a_ii|` or not finite).
fn negative_pivots(a: &CsrMatrix, factor: DirectFactor) -> Option<usize> {
    let diag = a.diag();
    let passes = |order: &[usize], pivots: &[f64]| {
        order
            .iter()
            .zip(pivots)
            .all(|(&row, &d)| d.is_finite() && d.abs() > PIVOT_TOLERANCE * diag[row].abs())
    };
    if factor == DirectFactor::SupernodalNestedDissection {
        let options = SparseOptions {
            ordering: SparseOrdering::NestedDissection,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        };
        let ldlt = SupernodalLdlt::factor_with(a, &options).ok()?;
        return passes(ldlt.order(), ldlt.pivots()).then(|| ldlt.inertia().0);
    }
    let options = SkylineOptions {
        pivots: PivotRule::NonZero,
        ..SkylineOptions::default()
    };
    let ldlt = SkylineLdlt::factor_with(a, &options).ok()?;
    passes(ldlt.order(), ldlt.pivots()).then(|| ldlt.inertia().0)
}

/// The most halvings of the backtracked predictor (ADR 0694 D3).
pub const PREDICTOR_HALVINGS: usize = 10;

/// The unsymmetric envelope `L D U` solve of the reduced tangent (ADR 0560 D4) with the pivot test and the texts of
/// [`factor_solve`].
fn factor_solve_unsymmetric(
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
    let lu = EnvelopeLu::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => Failure::Other(format!(
            "singular tangent: pivot {pivot:e} at {} (diagonal {:e})",
            at(row),
            diag[row]
        )),
        other => Failure::Other(format!("tangent factorisation: {other}")),
    })?;
    for (&row, &d) in lu.order().iter().zip(lu.pivots()) {
        if d.is_nan() || d <= PIVOT_TOLERANCE * diag[row] {
            return Err(Failure::Other(format!(
                "singular tangent: pivot {d:e} at {} (diagonal {:e})",
                at(row),
                diag[row]
            )));
        }
    }
    Ok(lu.solve(b))
}

/// The load direction of the arc-length iteration with the contributions (ADR 0560 D3): `f - sum_c d g_c / d lambda` at
/// `(u, lambda)`, one value per equation.
pub(crate) fn load_direction(
    f: &[f64],
    contributions: &[&dyn Contribution],
    u: &[f64],
    lambda: f64,
) -> std::result::Result<Vec<f64>, String> {
    let mut d = vec![0.0; f.len()];
    for c in contributions {
        c.add_load_factor_derivative(u, lambda, &mut d)
            .map_err(|e| format!("contribution '{}': {e}", c.name()))?;
    }
    Ok(f.iter().zip(&d).map(|(a, b)| a - b).collect())
}

/// A correction of the load factor at every iteration of an attempt (ADR 0480 D4: arc-length): given the solutions of
/// `K_T du_r = R` and `K_T du_f = f` on the free equations (in the order of the reduced system) and the number of negative
/// pivots of `K_T`, the change `d lambda` of the load factor; the correction of the iteration is `du_r + d lambda du_f`. An
/// error ends the attempt as failed.
pub(crate) trait LoadFactorControl {
    /// The change of the load factor at iteration `k` (0: from the committed state).
    fn correct(
        &mut self,
        k: usize,
        du_residual: &[f64],
        du_load: &[f64],
        negative_pivots: usize,
    ) -> std::result::Result<f64, String>;
    /// Whether the load direction `du_load` is that of the contributions too (ADR 0560 D3); `false`: the dead loads.
    fn contribution_load(&self) -> bool {
        false
    }
}

/// The controlled correction (ADR 0480 D4): the reduced tangent factored as indefinite (a pivot `|d| <= PIVOT_TOLERANCE
/// |a_ii|` refused), `du_r`, `du_f`, the control's `d lambda`; returns `du_r + d lambda du_f`, its right-hand side `R + d
/// lambda f` (for the energy) and `d lambda`.
fn controlled_correction(
    sys: &crate::constraint::ReducedSystem,
    f: &[f64],
    k: usize,
    dofs: &crate::model::DofMap,
    control: &mut dyn LoadFactorControl,
    unsymmetric: bool,
) -> std::result::Result<(Vec<f64>, Vec<f64>, f64), Failure> {
    let free = sys.free();
    if free.is_empty() {
        return Err(Failure::Other("no free equation".to_string()));
    }
    let a = &sys.matrix;
    let diag = a.diag();
    let at = |row: usize| {
        let (n, c) = dofs.node_component(free[row]).expect("free equation exists");
        format!("node {n} component {c}")
    };
    let pivot_error = |e: SkylineError| match e {
        SkylineError::Pivot { row, pivot, .. } => Failure::Other(format!(
            "singular tangent: pivot {pivot:e} at {} (diagonal {:e})",
            at(row),
            diag[row]
        )),
        other => Failure::Other(format!("tangent factorisation: {other}")),
    };
    let check_pivots = |order: &[usize], pivots: &[f64]| -> std::result::Result<(), Failure> {
        for (&row, &d) in order.iter().zip(pivots) {
            if d.is_nan() || d.abs() <= PIVOT_TOLERANCE * diag[row].abs() {
                return Err(Failure::Other(format!(
                    "singular tangent: pivot {d:e} at {} (diagonal {:e})",
                    at(row),
                    diag[row]
                )));
            }
        }
        Ok(())
    };
    let ff: Vec<f64> = free.iter().map(|&q| f[q]).collect();
    // ADR 0560 D4: the unsymmetric envelope factor with the same pivot test; its negative pivots are not an inertia.
    let (du_r, du_f, negative) = if unsymmetric {
        let lu = EnvelopeLu::factor_with(a, &SkylineOrdering::Rcm, PivotRule::NonZero).map_err(pivot_error)?;
        check_pivots(lu.order(), lu.pivots())?;
        (lu.solve(&sys.rhs), lu.solve(&ff), lu.negative_pivots())
    } else {
        let options = SkylineOptions {
            pivots: PivotRule::NonZero,
            ..SkylineOptions::default()
        };
        let ldlt = SkylineLdlt::factor_with(a, &options).map_err(pivot_error)?;
        check_pivots(ldlt.order(), ldlt.pivots())?;
        (ldlt.solve(&sys.rhs), ldlt.solve(&ff), ldlt.inertia().0)
    };
    let dl = control.correct(k, &du_r, &du_f, negative).map_err(Failure::Other)?;
    let du = du_r.iter().zip(&du_f).map(|(a, b)| a + dl * b).collect();
    let rhs = sys.rhs.iter().zip(&ff).map(|(a, b)| a + dl * b).collect();
    Ok((du, rhs, dl))
}

/// One converged attempt of [`controlled_attempt`]: the displacement, the trial states (to be committed), the residuals and
/// the energies of the iterations.
pub(crate) struct ControlledAttempt {
    pub(crate) u: Vec<f64>,
    pub(crate) states: Vec<ElementState>,
    pub(crate) residuals: Vec<f64>,
    pub(crate) energies: Vec<f64>,
}

/// One attempt of the driver with a load-factor control (ADR 0480 D4) from the committed `(u_c, lambda_c, states_c)`: the
/// load `lambda f`, the prescribed equations `fixed` (held at `lambda` times their values), the contributions at the current
/// load factor, the iteration and the criteria of `options`. The load factor starts at `lambda_c`; the control follows it.
/// A failure is described.
#[allow(clippy::too_many_arguments)]
pub(crate) fn controlled_attempt(
    model: &SolidModel,
    f: &[f64],
    fixed: &[(usize, f64)],
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
    u_c: &[f64],
    lambda_c: f64,
    states_c: &[ElementState],
    control: &mut dyn LoadFactorControl,
) -> std::result::Result<ControlledAttempt, String> {
    let n = f.len();
    let mut is_fixed = vec![false; n];
    for &(eq, _) in fixed {
        is_fixed[eq] = true;
    }
    let reuse = FactorReuse::new();
    let problem = Problem {
        model,
        f: f.to_vec(),
        fixed: fixed.to_vec(),
        free: (0..n).filter(|&i| !is_fixed[i]).collect(),
        options,
        contributions,
        reuse: &reuse,
        // ADR 0485 D3: the arc-length step of ADR 0480 is static, at the default point context.
        context: PointContext::default(),
        field: None,
    };
    match problem.attempt_with(u_c, lambda_c, lambda_c, states_c, Some(control)) {
        Ok(a) => Ok(ControlledAttempt {
            u: a.u,
            states: a.states,
            residuals: a.residuals,
            energies: a.energies,
        }),
        Err(Failure::Inverted(e)) => Err(e.to_string()),
        Err(Failure::Other(cause)) => Err(cause),
    }
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
    solve_with(model, prescribed, forces, options, DriverExtensions::default())
}

/// [`solve`] with the extension points of ADR 0440 D7: the contributions are added to the internal force and the tangent
/// at every iteration (and to the reactions), and the step-control hook sets the target of each attempt. With an empty list
/// and no hook it is [`solve`].
pub fn solve_with(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &NewtonOptions,
    extensions: DriverExtensions<'_>,
) -> Result<NonlinearResult> {
    let DriverExtensions {
        contributions,
        mut step_control,
    } = extensions;
    model.validate()?;
    model.check_supported_nonlinear_shells()?;
    check_driver_model(model)?;
    // ADR 0485 D4: the static driver has no time; the viscoelastic material needs one.
    if let Some((i, (set, m))) = model
        .materials()
        .iter()
        .enumerate()
        .find(|(_, (_, m))| matches!(m, Material::Viscoelastic { .. }) && m.state_size() > 0)
    {
        return Err(FemError::NotYetSupported {
            item: "material",
            variant: m.name(),
            location: format!("material {i}, element set '{set}': the nonlinear static solve (no time)"),
            package: crate::mat_visco::FOLLOW_UP,
        });
    }
    // ADR 0489 D3: nor the creep material.
    if let Some((i, (set, m))) = model
        .materials()
        .iter()
        .enumerate()
        .find(|(_, (_, m))| matches!(m, Material::Creep { .. }))
    {
        return Err(FemError::NotYetSupported {
            item: "material",
            variant: m.name(),
            location: format!("material {i}, element set '{set}': the nonlinear static solve (no time)"),
            package: crate::mat_creep::FOLLOW_UP,
        });
    }
    options.check()?;
    // ADR 0719 D1: the retry's inertia would need the contributions' tangents.
    if options.indefinite_retry && !contributions.is_empty() {
        return Err(FemError::InvalidValue {
            key: "indefinite_retry",
            reason:
                "with a contribution: the inertia of the accepted state would need the contributions' tangents (ADR \
                     0719 D1)"
                    .to_string(),
        });
    }
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
    let reuse = FactorReuse::new();
    let problem = Problem {
        model,
        f,
        fixed,
        free,
        options,
        contributions: &contributions,
        reuse: &reuse,
        context: PointContext::default(),
        field: None,
    };
    // ADR 0719 D1: the retry's options and problem, only with the opt-in.
    let retry_options = options.indefinite_retry.then(|| NewtonOptions {
        indefinite_tangent: true,
        indefinite_retry: false,
        ..options.clone()
    });
    let retry = retry_options.as_ref().map(|o| Problem {
        model,
        f: problem.f.clone(),
        fixed: problem.fixed.clone(),
        free: problem.free.clone(),
        options: o,
        contributions: &contributions,
        reuse: &reuse,
        context: PointContext::default(),
        field: None,
    });
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
            let proposed = if target - (lambda + dl) <= 1e-12 * dl {
                target
            } else {
                lambda + dl
            };
            let lt = match step_control.as_mut() {
                None => proposed,
                Some(hook) => {
                    let v = hook.load_factor(lambda, proposed, step, halvings)?;
                    if !(v > lambda && v <= target) {
                        return Err(FemError::InvalidValue {
                            key: "step control",
                            reason: format!(
                                "the hook '{}' gave the load factor {v:e} at load step {step}: it must lie in ({lambda:e}, \
                                 {target:e}]",
                                hook.name()
                            ),
                        });
                    }
                    v
                }
            };
            let outcome = match &retry {
                None => problem.attempt(&u, lambda, lt, &states).map(|a| (a, false)),
                Some(rp) => retrying(&problem, rp, &u, lambda, lt, &states),
            };
            match outcome {
                Ok((a, retried)) => {
                    // ADR 0719 D2: the inertia of the accepted state, before its states are committed.
                    let inertia = match &retry {
                        None => None,
                        Some(_) => problem.inertia_at(&a.u, &states),
                    };
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
                        yielded: a.yielded,
                        set_changed: a.set_changed,
                        step_lengths: a.step_lengths,
                        inertia,
                        retried,
                    });
                    if let Some(hook) = step_control.as_mut() {
                        hook.converged(records.last().expect("just pushed"));
                    }
                }
                Err(failure) => {
                    if let Some(hook) = step_control.as_mut() {
                        hook.failed(lambda, lt);
                    }
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
    let mut fint = internal_force(model, &u, &states)?;
    if !contributions.is_empty() {
        contribute(&contributions, &u, 1.0, &mut fint, None).map_err(|reason| FemError::Solver {
            solver: "newton",
            reason,
        })?;
    }
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

/// The outcome of [`solve_from`]: the converged displacement, the trial states of the converged iterate (to be committed by
/// the caller), the internal force there without the contributions, and the residuals and energies of the iterations.
#[derive(Debug, Clone, PartialEq)]
pub struct Equilibrium {
    /// The displacement, one value per equation.
    pub u: Vec<f64>,
    /// The element states of the converged iterate, from the committed states given.
    pub states: Vec<ElementState>,
    /// The internal force at `u` with those states, without the contributions.
    pub internal_force: Vec<f64>,
    /// The relative residuals of the iterations, the last one at convergence.
    pub residuals: Vec<f64>,
    /// The energies of the corrections.
    pub energies: Vec<f64>,
    /// With [`NewtonOptions::line_search`] or [`NewtonOptions::predictor_damping`], the step length of every correction
    /// (ADR 0607 D1, ADR 0688 D3); empty otherwise.
    pub step_lengths: Vec<f64>,
}

/// One attempt of [`solve_with`] from a committed state (ADR 0688 D2): from the committed displacement `committed` (which
/// must carry `lc` times the value of every prescribed equation, bit for bit), the committed element `states` and the
/// committed load factor `lc`, to the load factor `lt` (`0 <= lc < lt`), with the load vector `load` at factor 1: the
/// driver's predictor with the prescribed increment `(lt - lc) v`, then its Newton iteration, with its options and criteria
/// (`options.steps` and `options.max_halvings` are not used: one attempt, no halving). The iterates are those of the
/// attempt of [`solve_with`] from the same committed state, bit for bit. A failed attempt gives the located inversion
/// (`FemError::Element`) or `FemError::Solver { solver: "newton" }` with the cause.
#[allow(clippy::too_many_arguments)]
pub fn solve_increment(
    model: &SolidModel,
    prescribed: &[Prescribed],
    load: &[f64],
    committed: &[f64],
    states: &[ElementState],
    lc: f64,
    lt: f64,
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
) -> Result<Equilibrium> {
    model.validate()?;
    model.check_supported_nonlinear_shells()?;
    check_driver_model(model)?;
    options.check()?;
    options.refuse_indefinite_retry("solve_increment")?;
    check_fields(model, committed, states)?;
    if !(lc.is_finite() && lt.is_finite() && 0.0 <= lc && lc < lt) {
        return Err(FemError::InvalidValue {
            key: "load factor",
            reason: format!("from {lc:e} to {lt:e}: 0 <= lc < lt, finite"),
        });
    }
    let n = model.dof_map().n_equations();
    if load.len() != n {
        return Err(FemError::FieldLength {
            key: "load vector",
            expected: n,
            found: load.len(),
        });
    }
    for (key, x) in [("load vector", load), ("displacement", committed)] {
        if let Some(i) = x.iter().position(|v| !v.is_finite()) {
            return Err(FemError::InvalidValue {
                key,
                reason: format!("value {:e} at equation {i} is not finite", x[i]),
            });
        }
    }
    let fixed = prescribed_equations(model, prescribed)?;
    if let Some(&(eq, v)) = fixed
        .iter()
        .find(|&&(eq, v)| committed[eq].to_bits() != (lc * v).to_bits())
    {
        return Err(FemError::InvalidValue {
            key: "displacement",
            reason: format!(
                "the committed displacement has {:e} at the prescribed equation {eq} (lc times its value {:e}): it must carry \
                 the committed prescribed values",
                committed[eq],
                lc * v
            ),
        });
    }
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    let free: Vec<usize> = (0..n).filter(|&i| !is_fixed[i]).collect();
    let reuse = FactorReuse::new();
    let problem = Problem {
        model,
        f: load.to_vec(),
        fixed,
        free,
        options,
        contributions,
        reuse: &reuse,
        context: PointContext::default(),
        field: None,
    };
    match problem.attempt(committed, lc, lt, states) {
        Ok(a) => Ok(Equilibrium {
            u: a.u,
            states: a.states,
            internal_force: a.fint,
            residuals: a.residuals,
            energies: a.energies,
            step_lengths: a.step_lengths,
        }),
        Err(Failure::Inverted(e)) => Err(e),
        Err(Failure::Other(cause)) => Err(FemError::Solver {
            solver: "newton",
            reason: cause,
        }),
    }
}

/// One attempt of the Newton driver (ADR 0479 D2): `f_int(u) + sum of the contributions(u) = load` on the free equations,
/// from the displacement `start` and the committed element `states`, the model's prescribed equations and `prescribed`
/// held at their values (load factor 1 from 1: `start` must carry them). The iteration is the attempt of [`solve_with`]
/// (ADR 0404 D5 step 4) with its convergence criteria and options (`options.steps` and `options.max_halvings` are not
/// used: one attempt, no halving). The prescribed equations need not remove the rigid-body motions (a contribution such as
/// an inertia term may make the tangent regular); a singular tangent is refused by the pivot test. A failed attempt gives
/// the located inversion (`FemError::Element`, key `"deformation gradient"`) or `FemError::Solver { solver: "newton" }`
/// with the cause.
pub fn solve_from(
    model: &SolidModel,
    prescribed: &[Prescribed],
    load: &[f64],
    start: &[f64],
    states: &[ElementState],
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
) -> Result<Equilibrium> {
    solve_from_at(
        model,
        prescribed,
        load,
        start,
        states,
        options,
        contributions,
        &PointContext::default(),
    )
}

/// [`solve_from`] at the point context `context` (ADR 0485 D4): every material evaluation of the attempt gets it (time,
/// time increment and temperature of the step), so that the viscoelastic material of [`crate::mat_visco`] relaxes over the
/// step; the other materials do not read the time. With the default context it is [`solve_from`].
#[allow(clippy::too_many_arguments)]
pub fn solve_from_at(
    model: &SolidModel,
    prescribed: &[Prescribed],
    load: &[f64],
    start: &[f64],
    states: &[ElementState],
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
    context: &PointContext,
) -> Result<Equilibrium> {
    solve_from_field(
        model,
        prescribed,
        load,
        start,
        states,
        options,
        contributions,
        context,
        None,
    )
}

/// [`solve_from`] with the analysis of the supernodal factor kept in `reuse` (ADR 0566 D2): a sequence of attempts given
/// the same `reuse` analyses each pattern of the reduced tangent once while it is unchanged. The result is bit for bit that
/// of [`solve_from`]; with the envelope factor `reuse` is not used.
#[allow(clippy::too_many_arguments)]
pub fn solve_from_reusing(
    model: &SolidModel,
    prescribed: &[Prescribed],
    load: &[f64],
    start: &[f64],
    states: &[ElementState],
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
    reuse: &FactorReuse,
) -> Result<Equilibrium> {
    solve_from_field_reusing(
        model,
        prescribed,
        load,
        start,
        states,
        options,
        contributions,
        &PointContext::default(),
        None,
        reuse,
    )
}

/// [`solve_from_at`] with an optional nodal temperature field of the attempt and its thermal strain (ADR 0534 D2; the
/// model is checked against the field by the caller).
#[allow(clippy::too_many_arguments)]
pub(crate) fn solve_from_field(
    model: &SolidModel,
    prescribed: &[Prescribed],
    load: &[f64],
    start: &[f64],
    states: &[ElementState],
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
    context: &PointContext,
    field: Option<&FieldAt>,
) -> Result<Equilibrium> {
    solve_from_field_reusing(
        model,
        prescribed,
        load,
        start,
        states,
        options,
        contributions,
        context,
        field,
        &FactorReuse::new(),
    )
}

/// [`solve_from_field`] with the cached analysis `reuse` (ADR 0566 D2).
#[allow(clippy::too_many_arguments)]
fn solve_from_field_reusing(
    model: &SolidModel,
    prescribed: &[Prescribed],
    load: &[f64],
    start: &[f64],
    states: &[ElementState],
    options: &NewtonOptions,
    contributions: &[&dyn Contribution],
    context: &PointContext,
    field: Option<&FieldAt>,
    reuse: &FactorReuse,
) -> Result<Equilibrium> {
    model.validate()?;
    // ADR 0522 D4: Shell4 and Shell3 take the corotated element of ADR 0490.
    model.check_supported_nonlinear_shells()?;
    check_driver_model(model)?;
    options.check()?;
    options.refuse_indefinite_retry("solve_from")?;
    check_fields(model, start, states)?;
    let n = model.dof_map().n_equations();
    if load.len() != n {
        return Err(FemError::FieldLength {
            key: "load vector",
            expected: n,
            found: load.len(),
        });
    }
    for (key, x) in [("load vector", load), ("displacement", start)] {
        if let Some(i) = x.iter().position(|v| !v.is_finite()) {
            return Err(FemError::InvalidValue {
                key,
                reason: format!("value {:e} at equation {i} is not finite", x[i]),
            });
        }
    }
    let fixed = prescribed_equations(model, prescribed)?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(eq, v)| start[eq].to_bits() != v.to_bits()) {
        return Err(FemError::InvalidValue {
            key: "displacement",
            reason: format!(
                "the start has {:e} at the prescribed equation {eq} (prescribed {v:e}): it must carry the prescribed values",
                start[eq]
            ),
        });
    }
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    let free: Vec<usize> = (0..n).filter(|&i| !is_fixed[i]).collect();
    let problem = Problem {
        model,
        f: load.to_vec(),
        fixed,
        free,
        options,
        contributions,
        reuse,
        context: context.clone(),
        field,
    };
    match problem.attempt(start, 1.0, 1.0, states) {
        Ok(a) => Ok(Equilibrium {
            u: a.u,
            states: a.states,
            internal_force: a.fint,
            residuals: a.residuals,
            energies: a.energies,
            step_lengths: a.step_lengths,
        }),
        Err(Failure::Inverted(e)) => Err(e),
        Err(Failure::Other(cause)) => Err(FemError::Solver {
            solver: "newton",
            reason: cause,
        }),
    }
}

/// The options of [`solve_in_time`] (ADR 0489 D5): the Newton options of each attempt, the largest equivalent creep-strain
/// increment of a step at any point of a creep material (`f64::INFINITY`: no limit), and the halvings of one step.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TimeSteppingOptions {
    /// The options of each attempt (`steps` and `max_halvings` are not used).
    pub newton: NewtonOptions,
    /// The largest equivalent creep-strain increment of a committed sub-step; positive (infinite: no limit).
    pub max_creep_increment: f64,
    /// The halvings of one step before the step is refused.
    pub max_halvings: usize,
    /// A uniform temperature of every point, K (ADR 0499 D2; `None`: no temperature in the point context).
    pub temperature: Option<f64>,
    /// A uniform temperature against time, K (ADR 0526 D2): a piecewise linear table taken at the end time of every attempt
    /// (`None`: none; refused together with `temperature`).
    pub temperature_history: Option<LoadTable>,
    /// A nodal temperature field against time with the thermal strain of the materials' expansion (ADR 0534 D3): taken at the
    /// end time of every attempt (`None`: none; refused together with `temperature` or `temperature_history`).
    pub temperature_field: Option<TemperatureFieldHistory>,
}

impl TimeSteppingOptions {
    /// The limit `max_creep_increment`, the Newton defaults of [`NewtonOptions::new`] and 5 halvings.
    pub fn new(max_creep_increment: f64) -> TimeSteppingOptions {
        TimeSteppingOptions {
            newton: NewtonOptions::new(1),
            max_creep_increment,
            max_halvings: 5,
            temperature: None,
            temperature_history: None,
            temperature_field: None,
        }
    }
    /// With the Newton options of each attempt.
    pub fn newton(mut self, newton: NewtonOptions) -> TimeSteppingOptions {
        self.newton = newton;
        self
    }
    /// With the halvings of one step.
    pub fn max_halvings(mut self, halvings: usize) -> TimeSteppingOptions {
        self.max_halvings = halvings;
        self
    }
    /// With a uniform temperature of every point, K (ADR 0499 D2).
    pub fn temperature(mut self, temperature: f64) -> TimeSteppingOptions {
        self.temperature = Some(temperature);
        self
    }
    /// With a uniform temperature against time, K (ADR 0526 D2), taken at the end time of every attempt.
    pub fn temperature_history(mut self, table: LoadTable) -> TimeSteppingOptions {
        self.temperature_history = Some(table);
        self
    }
    /// With a nodal temperature field against time and its thermal strain (ADR 0534 D3), taken at the end time of every
    /// attempt.
    pub fn temperature_field(mut self, field: TemperatureFieldHistory) -> TimeSteppingOptions {
        self.temperature_field = Some(field);
        self
    }
    fn check(&self) -> Result<()> {
        if let Some(t) = self.temperature {
            PointContext::new().with_temperature(t)?;
        }
        if self.temperature.is_some() && self.temperature_history.is_some() {
            return Err(FemError::InvalidValue {
                key: "temperature",
                reason: "a constant temperature and a temperature history together (ADR 0526 D1)".to_string(),
            });
        }
        if self.temperature_field.is_some() && (self.temperature.is_some() || self.temperature_history.is_some()) {
            return Err(FemError::InvalidValue {
                key: "temperature",
                reason:
                    "a temperature field together with a constant temperature or a temperature history (ADR 0534 D3)"
                        .to_string(),
            });
        }
        if self.max_creep_increment.is_nan() || self.max_creep_increment <= 0.0 {
            return Err(FemError::InvalidValue {
                key: "max_creep_increment",
                reason: format!("{:e} must be positive", self.max_creep_increment),
            });
        }
        self.newton.refuse_indefinite_retry("solve_in_time")?;
        self.newton.check()
    }
}

/// The committed solution at one step end of [`solve_in_time`].
#[derive(Debug, Clone, PartialEq)]
pub struct TimeStep {
    /// The time of the step end, s (0 for the instantaneous response).
    pub time: f64,
    /// The displacement, one value per equation.
    pub u: Vec<f64>,
    /// The committed element states (with the warm start of ADR 0660 D1 an Incompatible Hex8's state ends with its nine
    /// committed modes, ADR 0668 D2).
    pub states: Vec<ElementState>,
    /// The internal force at `u` with those states.
    pub internal_force: Vec<f64>,
    /// The halvings of the step.
    pub halvings: usize,
    /// The committed sub-steps of the step.
    pub substeps: usize,
    /// The largest equivalent creep-strain increment among the committed sub-steps of the step.
    pub max_creep_increment: f64,
    /// The Newton corrections of the committed sub-steps.
    pub iterations: usize,
}

/// The outcome of [`solve_in_time`]: the instantaneous response at `t = 0`, then one entry per step end.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeHistory {
    /// The step ends in order.
    pub steps: Vec<TimeStep>,
}

/// Whether an error is the located non-converged local creep iteration (ADR 0489 D4).
fn is_creep_iteration(e: &FemError) -> bool {
    matches!(e, FemError::Element { source, .. }
        if matches!(source.as_ref(), FemError::InvalidValue { key, .. } if *key == crate::mat_creep::ITERATION_KEY))
}

/// The largest increment of the equivalent creep strain between two sets of element states, over the points of the elements
/// marked in `creep`. With `cut` (the model's warm start, ADR 0668 D1) the values of element `e` are cut to their first
/// `cut[e]` values, the material part, first: the committed modes of ADR 0660 D1 after it are not points.
fn creep_increment(creep: &[bool], cut: Option<&[usize]>, old: &[ElementState], new: &[ElementState]) -> f64 {
    let ns = crate::mat_creep::STATE_SIZE;
    let mut worst = 0.0f64;
    for (e, ((&c, o), n)) in creep.iter().zip(old).zip(new).enumerate() {
        if !c {
            continue;
        }
        let (ov, nv) = match cut {
            Some(cut) => {
                let (ov, nv) = (o.values(), n.values());
                (&ov[..cut[e].min(ov.len())], &nv[..cut[e].min(nv.len())])
            }
            None => (o.values(), n.values()),
        };
        for (po, pn) in ov.chunks(ns).zip(nv.chunks(ns)) {
            worst = worst.max(pn[ns - 1] - po[ns - 1]);
        }
    }
    worst
}

/// The load and displacement histories of [`solve_in_time_with`] (ADR 0499 D1): each a fixed pattern times a piecewise
/// linear factor of time, taken at the end time of every attempt.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimeLoading {
    /// Load histories: `pattern s(t)` (one value per equation) added to the model's dead loads, in list order.
    pub loads: Vec<LoadHistory>,
    /// Displacement histories: the listed node components prescribed to `value s(t)`.
    pub motions: Vec<MotionHistory>,
}

impl TimeLoading {
    /// No history.
    pub fn new() -> TimeLoading {
        TimeLoading::default()
    }
    /// With one more load history.
    pub fn load(mut self, history: LoadHistory) -> TimeLoading {
        self.loads.push(history);
        self
    }
    /// With one more displacement history.
    pub fn motion(mut self, history: MotionHistory) -> TimeLoading {
        self.motions.push(history);
        self
    }
}

/// The quasi-static time-stepping driver (ADR 0489 D5): the model's dead loads and its prescribed displacements and
/// `prescribed`, constant in time, applied at `t = 0` by one attempt of [`solve_from_at`] (the instantaneous response, the
/// first entry), then the steps to the end times `times` (finite, strictly increasing, the first positive; key `"times"`),
/// each in sub-steps of one attempt of [`solve_from_at`] at the context of the sub-step's end time and length. A sub-step
/// whose largest equivalent creep-strain increment exceeds `options.max_creep_increment`, or whose attempt fails, is
/// discarded and its length halved (kept to the end of the step); after `options.max_halvings` halvings of one step the error
/// is `FemError::Solver { solver: "creep time stepping" }` naming the step. A non-converged local creep iteration is
/// returned at once with its element (`FemError::Element`, key [`crate::mat_creep::ITERATION_KEY`]). It is
/// [`solve_in_time_with`] without histories.
pub fn solve_in_time(
    model: &SolidModel,
    prescribed: &[Prescribed],
    times: &[f64],
    options: &TimeSteppingOptions,
) -> Result<TimeHistory> {
    solve_in_time_with(model, prescribed, &TimeLoading::default(), times, options)
}

/// [`solve_in_time`] with load and displacement histories (ADR 0499 D1): every attempt (the instantaneous one at `t = 0` and
/// every sub-step, halved or not, to its end time `t1`) takes the load `f_dead + sum_j pattern_j s_j(t1)` and the prescribed
/// values of the model and `prescribed` (constant) and of each motion, `value s_j(t1)` (written into the start of the
/// attempt). With `options.temperature` every point context carries that uniform temperature (D2); with
/// `options.temperature_history` the table's value at the attempt's end time `t1` (ADR 0526 D2: the material relaxes over
/// the attempt at that temperature); with `options.temperature_field` the nodal field's value at `t1` and its thermal strain
/// (ADR 0534 D3). Refused before any attempt: a load pattern of the wrong length or with a non-finite
/// value (key `"load pattern"`), a table that does not cover `[0, t_last]` (keys `"load history"`, `"motion history"`,
/// `"temperature"`), a motion of an equation already prescribed by the model, by `prescribed` or by an earlier motion (key
/// `"motion history"`), a temperature together with a temperature history (key `"temperature"`), a temperature field not
/// covering `[0, t_last]` or together with either, and the models of ADR 0534 D4.
pub fn solve_in_time_with(
    model: &SolidModel,
    prescribed: &[Prescribed],
    loading: &TimeLoading,
    times: &[f64],
    options: &TimeSteppingOptions,
) -> Result<TimeHistory> {
    model.validate()?;
    model.check_supported_nonlinear()?;
    check_driver_model(model)?;
    options.check()?;
    // ADR 0558 D3: a creep material with the Arrhenius factor needs a temperature, refused before any attempt.
    if options.temperature.is_none() && options.temperature_history.is_none() && options.temperature_field.is_none() {
        if let Some((i, (set, _))) = model.materials().iter().enumerate().find(|(_, (_, m))| {
            matches!(
                m,
                Material::Creep {
                    activation: Some(_),
                    ..
                }
            )
        }) {
            return Err(FemError::InvalidValue {
                key: "temperature",
                reason: format!(
                    "material {i}, element set '{set}': a creep material with an Arrhenius factor needs a temperature (TimeSteppingOptions::temperature, temperature_history or temperature_field)"
                ),
            });
        }
    }
    // ADR 0578 D2: a non-positive constant temperature or history point of a factor material, refused before any attempt.
    crate::mat_creep::check_factor_temperature(
        model,
        options.temperature,
        options.temperature_history.as_ref(),
        options.temperature_field.is_some(),
    )?;
    for (k, &t) in times.iter().enumerate() {
        let previous = if k == 0 { 0.0 } else { times[k - 1] };
        if !(t.is_finite() && t > previous) {
            return Err(FemError::InvalidValue {
                key: "times",
                reason: format!(
                    "time {k} is {t:e} s: the step ends must be finite, strictly increasing and the first positive"
                ),
            });
        }
    }
    let n = model.dof_map().n_equations();
    let load = nodal_load_vector(model, &[])?;
    let fixed = prescribed_equations(model, prescribed)?;
    let t_last = times.last().copied().unwrap_or(0.0);
    let histories = Histories::new(model, &fixed, loading, t_last)?;
    if let Some(table) = &options.temperature_history {
        if !table.covers(0.0, t_last) {
            let (lo, hi) = table.range();
            return Err(FemError::InvalidValue {
                key: "temperature",
                reason: format!(
                    "the temperature history covers [{lo:e}, {hi:e}] s, not [0, {t_last:e}] s (ADR 0526 D2)"
                ),
            });
        }
    }
    // ADR 0534 D3, D4: the field covers the run and the model takes it.
    if let Some(field) = &options.temperature_field {
        if !field.covers(0.0, t_last) {
            let (lo, hi) = field.range();
            return Err(FemError::InvalidValue {
                key: "temperature",
                reason: format!("the temperature field covers [{lo:e}, {hi:e}] s, not [0, {t_last:e}] s (ADR 0534 D3)"),
            });
        }
        crate::thermal::check_field_model(model, field)?;
    }
    // ADR 0534 D3: the field at the end time of an attempt, with the thermal strain.
    let field_at = |t: f64| options.temperature_field.as_ref().map(|f| FieldAt::of(f, t, true));
    let mut u = vec![0.0; n];
    for &(eq, v) in &fixed {
        u[eq] = v;
    }
    let materials = model.material_indices()?;
    let creep: Vec<bool> = materials
        .iter()
        .map(|&m| matches!(model.materials()[m].1, Material::Creep { .. }))
        .collect();
    // ADR 0668 D1: with the warm start the creep reading is cut to the material part of each element.
    let cut: Option<Vec<usize>> = if model.incompatible_mode_warm_start() {
        let layout = StateLayout::of(model)?;
        Some(
            (0..materials.len())
                .map(|e| layout.points(e) * layout.values_per_point(e))
                .collect(),
        )
    } else {
        None
    };
    let context = |t: f64, h: f64| -> Result<PointContext> {
        let c = PointContext::new().with_time(t, h)?;
        // ADR 0526 D2: the temperature history at the end time of the attempt.
        if let Some(table) = &options.temperature_history {
            return c.with_temperature(table.value(t).expect("temperature coverage checked"));
        }
        match options.temperature {
            None => Ok(c),
            Some(temperature) => c.with_temperature(temperature),
        }
    };
    let start = initial_states(model)?;
    let at0 = context(0.0, 0.0)?;
    let eq = {
        let (f, p, s) = histories.at(0.0, &load, prescribed, &u);
        let field = field_at(0.0);
        // ADR 0539 D6: the attempt at t = 0 starts from the undeformed reference state.
        let s = thermal_start(model, options, &p, &s, None, 0.0)?;
        solve_from_field(model, &p, &f, &s, &start, &options.newton, &[], &at0, field.as_ref())?
    };
    let mut steps = vec![TimeStep {
        time: 0.0,
        u: eq.u.clone(),
        states: eq.states.clone(),
        internal_force: eq.internal_force,
        halvings: 0,
        substeps: 1,
        max_creep_increment: creep_increment(&creep, cut.as_deref(), &start, &eq.states),
        iterations: eq.residuals.len() - 1,
    }];
    let mut u = eq.u;
    let mut states = eq.states;
    let mut t = 0.0;
    for (k, &target) in times.iter().enumerate() {
        let mut h = target - t;
        let (mut halvings, mut substeps, mut iterations) = (0, 0, 0);
        let mut largest = 0.0f64;
        let mut internal = Vec::new();
        while t < target {
            let t1 = if target - (t + h) <= 1e-12 * h { target } else { t + h };
            let ctx = context(t1, t1 - t)?;
            let attempt = {
                let (f, p, s) = histories.at(t1, &load, prescribed, &u);
                let field = field_at(t1);
                // ADR 0539 D6: from the committed time `t` to the end of the attempt.
                let s = thermal_start(model, options, &p, &s, Some(t), t1)?;
                solve_from_field(model, &p, &f, &s, &states, &options.newton, &[], &ctx, field.as_ref())
            };
            let failure = match attempt {
                Ok(eq) => {
                    let inc = creep_increment(&creep, cut.as_deref(), &states, &eq.states);
                    if inc <= options.max_creep_increment {
                        iterations += eq.residuals.len() - 1;
                        substeps += 1;
                        largest = largest.max(inc);
                        u = eq.u;
                        states = eq.states;
                        internal = eq.internal_force;
                        t = t1;
                        continue;
                    }
                    format!(
                        "the creep-strain increment {inc:e} exceeds the limit {:e}",
                        options.max_creep_increment
                    )
                }
                Err(e) if is_creep_iteration(&e) => return Err(e),
                Err(e) => e.to_string(),
            };
            if halvings == options.max_halvings {
                return Err(FemError::Solver {
                    solver: "creep time stepping",
                    reason: format!(
                        "step {} of {} ([{t:e}, {target:e}] s, sub-step to {t1:e} s) after {halvings} halvings: {failure}",
                        k + 1,
                        times.len()
                    ),
                });
            }
            halvings += 1;
            h *= 0.5;
        }
        steps.push(TimeStep {
            time: target,
            u: u.clone(),
            states: states.clone(),
            internal_force: internal,
            halvings,
            substeps,
            max_creep_increment: largest,
            iterations,
        });
    }
    Ok(TimeHistory { steps })
}

/// The start of an attempt (ADR 0539 D6): `start` itself, or with the thermal predictor of the field of the options on, `start`
/// plus the free thermal expansion increment of the change of the field from the committed time `from` (`None`: the reference
/// state, `T = T_ref` everywhere) to the end time `to` of the attempt.
fn thermal_start<'b>(
    model: &SolidModel,
    options: &TimeSteppingOptions,
    prescribed: &[Prescribed],
    start: &'b [f64],
    from: Option<f64>,
    to: f64,
) -> Result<Cow<'b, [f64]>> {
    let Some(field) = options.temperature_field.as_ref().filter(|f| f.thermal_predictor()) else {
        return Ok(Cow::Borrowed(start));
    };
    let new = field.value(to).expect("temperature field coverage checked");
    let old = match from {
        Some(t) => field.value(t).expect("temperature field coverage checked"),
        None => vec![field.reference(); new.len()],
    };
    crate::thermal::free_expansion_start(model, prescribed, start, &old, &new, field.reference()).map(Cow::Owned)
}

/// The inputs of one attempt of [`solve_in_time_with`]: the load vector, the prescribed list and the start displacement.
type AttemptInputs<'b> = (Cow<'b, [f64]>, Cow<'b, [Prescribed]>, Cow<'b, [f64]>);

/// The checked histories of [`solve_in_time_with`]: the load histories and the motions with their equations.
struct Histories<'a> {
    loads: &'a [LoadHistory],
    motions: Vec<(Vec<(usize, Prescribed)>, &'a LoadTable)>,
}

impl<'a> Histories<'a> {
    fn new(model: &SolidModel, fixed: &[(usize, f64)], loading: &'a TimeLoading, t_last: f64) -> Result<Histories<'a>> {
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        for (j, l) in loading.loads.iter().enumerate() {
            if l.pattern.len() != n {
                return Err(FemError::FieldLength {
                    key: "load pattern",
                    expected: n,
                    found: l.pattern.len(),
                });
            }
            if let Some(i) = l.pattern.iter().position(|x| !x.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "load pattern",
                    reason: format!(
                        "load history {j}: value {:e} at equation {i} is not finite",
                        l.pattern[i]
                    ),
                });
            }
            if !l.table.covers(0.0, t_last) {
                let (lo, hi) = l.table.range();
                return Err(FemError::InvalidValue {
                    key: "load history",
                    reason: format!("load history {j}: the table covers [{lo:e}, {hi:e}] s, not [0, {t_last:e}] s"),
                });
            }
        }
        let mut taken = vec![false; n];
        for &(eq, _) in fixed {
            taken[eq] = true;
        }
        let mut motions = Vec::with_capacity(loading.motions.len());
        for (j, m) in loading.motions.iter().enumerate() {
            let motion_error = |reason: String| FemError::InvalidValue {
                key: "motion history",
                reason: format!("motion history {j}: {reason}"),
            };
            if !m.table.covers(0.0, t_last) {
                let (lo, hi) = m.table.range();
                return Err(motion_error(format!(
                    "the table covers [{lo:e}, {hi:e}] s, not [0, {t_last:e}] s"
                )));
            }
            let mut list = Vec::with_capacity(m.prescribed.len());
            for p in &m.prescribed {
                if !p.value.is_finite() {
                    return Err(motion_error(format!(
                        "the value {:e} at node {} is not finite",
                        p.value, p.node
                    )));
                }
                let eq = dofs.equation(p.node, p.component).ok_or_else(|| {
                    motion_error(format!(
                        "node {} component {} is not an equation of the model",
                        p.node, p.component
                    ))
                })?;
                if taken[eq] {
                    return Err(motion_error(format!(
                        "node {} component {} is already prescribed",
                        p.node, p.component
                    )));
                }
                taken[eq] = true;
                list.push((eq, *p));
            }
            motions.push((list, &m.table));
        }
        Ok(Histories {
            loads: &loading.loads,
            motions,
        })
    }

    /// The load vector, the prescribed list and the start displacement of an attempt to the time `t` from the committed
    /// displacement `u`; without histories the inputs themselves (the operations of ADR 0489 D5).
    fn at<'b>(&self, t: f64, load: &'b [f64], prescribed: &'b [Prescribed], u: &'b [f64]) -> AttemptInputs<'b> {
        let f = if self.loads.is_empty() {
            Cow::Borrowed(load)
        } else {
            let mut f = load.to_vec();
            for l in self.loads {
                let s = l.table.value(t).expect("load coverage checked");
                for (fi, p) in f.iter_mut().zip(&l.pattern) {
                    *fi += p * s;
                }
            }
            Cow::Owned(f)
        };
        if self.motions.is_empty() {
            return (f, Cow::Borrowed(prescribed), Cow::Borrowed(u));
        }
        let mut list = prescribed.to_vec();
        let mut start = u.to_vec();
        for (motion, table) in &self.motions {
            let s = table.value(t).expect("motion coverage checked");
            for &(eq, p) in motion {
                let v = p.value * s;
                list.push(Prescribed::new(p.node, p.component, v));
                start[eq] = v;
            }
        }
        (f, Cow::Owned(list), Cow::Owned(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR 0668 D3, U1 to U3: `creep_increment` with the cut reads only the material part of a warm creep element.
    #[test]
    fn creep_increment_reads_the_material_part_with_the_warm_start() {
        let ns = crate::mat_creep::STATE_SIZE;
        let points = 8;
        let n = points * ns;
        // Element 0: creep, 8 points and nine trailing values; element 1: not creep (its values change by 5).
        let mut old0 = vec![0.0; n + 9];
        let mut new0 = vec![0.0; n + 9];
        for p in 0..points {
            old0[p * ns + ns - 1] = 1.0;
            new0[p * ns + ns - 1] = 1.0 + if p == points - 1 { 0.3 } else { 0.1 };
        }
        for j in 0..9 {
            old0[n + j] = 2.0;
            new0[n + j] = 2.0 + 1e3;
        }
        let old1 = vec![0.0; n];
        let new1 = vec![5.0; n];
        let creep = [true, false];
        let old = [ElementState::new(old0.clone()), ElementState::new(old1.clone())];
        let new = [ElementState::new(new0.clone()), ElementState::new(new1.clone())];
        let cut = [n, n];
        let with_cut = creep_increment(&creep, Some(&cut), &old, &new);
        // U1: the last point's increment, bitwise.
        assert_eq!(with_cut.to_bits(), ((1.0 + 0.3) - 1.0f64).to_bits(), "U1: {with_cut:e}");
        // U2: the same as the reading without the cut on the states without the trailing values.
        let old_m = [ElementState::new(old0[..n].to_vec()), ElementState::new(old1)];
        let new_m = [ElementState::new(new0[..n].to_vec()), ElementState::new(new1)];
        let plain = creep_increment(&creep, None, &old_m, &new_m);
        assert_eq!(
            with_cut.to_bits(),
            plain.to_bits(),
            "U2: {with_cut:e} against {plain:e}"
        );
        // U3: no creep element, no increment.
        assert_eq!(
            creep_increment(&[false, false], None, &old_m, &new_m).to_bits(),
            0.0f64.to_bits(),
            "U3"
        );
    }
}
