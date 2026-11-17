//! The incompatible-modes Hex8 at finite strain (ADR 0608, F-596): an enhanced deformation gradient with the internal modes
//! condensed in the Newton loop.
//!
//! * Kinematics (ADR 0608 arithmetic 1; the enhanced deformation gradient of Simo and Armero, Int. J. Numer. Meth. Engng 33
//!   (1992) 1413, with the incompatible modes of Wilson et al. and Taylor's correction, cited from memory and derived in the
//!   ADR): `F = I + sum_a u_a (x) g_a + sum_k alpha_k (x) G_k` with the nine modes `alpha` of [`crate::hex8x`] (order `3 k +
//!   c`) and their gradients `G_k = (det J_0 / det J) J_0^{-T} dP_k/dxi`, which integrate to zero over any element; the
//!   material at the Green-Lagrange strain of that `F` from the committed state of the point (the driver's material update).
//!   Over the 33 unknowns `(u, alpha)` the element is the total Lagrangian element of ADR 0404 over eleven gradients
//!   ([`enhanced_force_and_tangent`]).
//! * Condensation (arithmetic 2): at every evaluation the modes solve `R_a(u, alpha) = 0` by a local Newton iteration from
//!   `alpha = 0` ([`LOCAL_TOLERANCE`], [`LOCAL_ITERATIONS`]); the element gives the condensed force `f_u - K_ua K_aa^{-1}
//!   R_a` and tangent `K_uu - K_ua K_aa^{-1} K_au` at the converged modes, the exact derivative of its force. The element is
//!   a function of `u` and the committed material state: no state is added. A singular `K_aa` and a local iteration that
//!   does not converge are refused (key [`ITERATION_KEY`]).
//! * At `u = 0` the tangent is the condensed stiffness of [`crate::hex8x::element_stiffness`] (arithmetic 3); a homogeneous
//!   deformation gives `alpha = 0` and the Full element's force (arithmetic 4, the patch test).
//!
//! * Stabilisation (ADR 0614, F-685, opt-in per model by [`crate::model::SolidModel::set_incompatible_stabilisation`]): the
//!   element energy gains `Pi_s = (c(u)/2) sum_q w_q |F_enh,q|^2` with `F_enh = sum_k alpha_k (x) G_k` and the coefficient
//!   `c = (s : s) / G0`, `s = D0 : e0`, `e0 = (C0 - C0^{-1})/4` of the compatible gradient `F0` at the element centre, `D0`
//!   the rest elasticity of the material and `G0` its mean shear modulus ([`stabilisation_coefficient`]): zero with its first
//!   derivative at rest and on every rigid motion, and with `alpha = 0` on every homogeneous state, so the element at rest and
//!   its patch test are those of ADR 0608; it removes the spurious hourglass mode of the element under large homogeneous
//!   compression (ADR 0614 arithmetic 1 to 4). Force and tangent are the exact derivatives of `Pi_s` (ADR 0614 arithmetic 5).
//! * Rules of the coefficient (ADR 0630, F-703; [`IncompatibleStabilisation`], opt-in per model by the same setter): besides
//!   the coefficient above (`RestStress`), `Deviatoric` `c = (dev s : dev s) / G0` (for an isotropic material `4 G0 |dev
//!   e0|^2`, without the bulk modulus) and `LinearStress` `c = (s : s) / (G0 + |s|)` (equal to the coefficient above to
//!   second order at rest, linear in the stress at large stress: the order of the pressure-driven need of ADR 0630
//!   arithmetic 2). Both keep the properties at rest and on homogeneous states, and are at most the coefficient above.
//!
//! The nonlinear driver ([`crate::nonlinear`]) evaluates a Hex8 of a block of the `Incompatible` formulation by this module.

use crate::element::{Element, ElementMatrix, ElementState, Hex8};
use crate::error::{FemError, Result};
use crate::hex8x::{mode_points, ModePoint};
use crate::material::{Matrix6, PointContext};
use crate::model::Material;
use crate::nonlinear::{
    b_nonlinear, check_hex8_fields, displacement_gradient, green_lagrange, point_update, Evaluation, INVERSION_KEY,
};
use rcfd_core::vec3::Vec3;

/// The local iteration stops when every mode residual is at most this fraction of the largest sum of the magnitudes of the
/// terms it is made of (ADR 0608 arithmetic 2).
pub const LOCAL_TOLERANCE: f64 = 1e-12;

/// The most local Newton iterations of the modes before the evaluation is refused (ADR 0608 D2).
pub const LOCAL_ITERATIONS: usize = 25;

/// The key of the refusal of a singular mode matrix or a local iteration that does not converge.
pub const ITERATION_KEY: &str = "internal modes";

const NODES: usize = 24;
const MODES: usize = 9;
const ALL: usize = NODES + MODES;

/// The extended element at `(u, alpha)`: force and tangent over the 33 unknowns, the stress, strain and trial state at the
/// points, and the scale of the mode residual.
struct Extended {
    force: Vec<f64>,
    tangent: Vec<f64>,
    stress: Vec<[f64; 6]>,
    strain: Vec<[f64; 6]>,
    state: Vec<f64>,
    mode_scale: f64,
}

fn det3(f: &[[f64; 3]; 3]) -> f64 {
    f[0][0] * (f[1][1] * f[2][2] - f[1][2] * f[2][1]) - f[0][1] * (f[1][0] * f[2][2] - f[1][2] * f[2][0])
        + f[0][2] * (f[1][0] * f[2][1] - f[1][1] * f[2][0])
}

fn check_state(material: &Material, state: &ElementState, points: usize) -> Result<usize> {
    let ns = material.state_size();
    if state.values().len() != points * ns {
        return Err(FemError::FieldLength {
            key: "state",
            expected: points * ns,
            found: state.values().len(),
        });
    }
    Ok(ns)
}

/// The rule of the coefficient of the stabilisation of the modes of an Incompatible Hex8 (ADR 0614 D1, ADR 0630 D1). `Off`
/// is the element of ADR 0608; `true` converts to `RestStress` (the coefficient of ADR 0614), `false` to `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum IncompatibleStabilisation {
    /// No stabilisation (ADR 0608 D2; the default).
    #[default]
    Off,
    /// `c = (s : s) / G0`, `s = D0 : (C0 - C0^{-1})/4` (ADR 0614 D1).
    RestStress,
    /// `c = (dev s : dev s) / G0` (ADR 0630 arithmetic 3, rule D).
    Deviatoric,
    /// `c = (s : s) / (G0 + |s|)` (ADR 0630 arithmetic 3, rule L).
    LinearStress,
}

impl From<bool> for IncompatibleStabilisation {
    fn from(on: bool) -> IncompatibleStabilisation {
        if on {
            IncompatibleStabilisation::RestStress
        } else {
            IncompatibleStabilisation::Off
        }
    }
}

/// The local update of the modes of an Incompatible Hex8 (ADR 0658 D1): `Plain` is the local Newton iteration of ADR 0608
/// D2 (the default); `Backtracking` halves a step whose iterate inverts a point (ADR 0655 D1); `BacktrackingWithDecrease`
/// also halves a step whose iterate does not decrease the mode residual sufficiently (ADR 0658 arithmetic 4,
/// [`DECREASE_FRACTION`]). `false` converts to `Plain`, `true` to `Backtracking`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum IncompatibleModeUpdate {
    /// The full Newton step (ADR 0608 D2; the default).
    #[default]
    Plain,
    /// An inverted iterate halves the step, at most [`MODE_HALVINGS`] times (ADR 0655 D1).
    Backtracking,
    /// As `Backtracking`, and a trial with `|r(t)|_2^2 > (1 - 2 c t) |r_j|_2^2`, `c =` [`DECREASE_FRACTION`], is halved as
    /// well, in the same sequence (ADR 0658 D1).
    BacktrackingWithDecrease,
    /// The Levenberg-Marquardt step of ADR 0660 D2 (a trust region on the mode step): `d` minimises `|r - J d|_2^2 + mu
    /// |d|_2^2`, accepted on the gain ratio of the merit `|r|_2^2 / 2` ([`TRUST_REGION_ACCEPT`]).
    TrustRegion,
}

impl From<bool> for IncompatibleModeUpdate {
    fn from(on: bool) -> IncompatibleModeUpdate {
        if on {
            IncompatibleModeUpdate::Backtracking
        } else {
            IncompatibleModeUpdate::Plain
        }
    }
}

/// The constant `c` of the sufficient-decrease test of the mode update (ADR 0658 arithmetic 4: the Armijo condition of the
/// backtracking line search for nonlinear equations, Dennis and Schnabel 1983, cited from memory).
pub const DECREASE_FRACTION: f64 = 1e-4;

/// The initial scale-free damping `lambda_0` of the trust-region mode step, `mu = lambda max_i (J^T J)_ii` (ADR 0660
/// arithmetic 5: a direction of singular value at least `1e-3` of the largest stays within one per cent of the Newton step).
pub const TRUST_REGION_DAMPING: f64 = 1e-8;

/// The smallest damping `lambda` of the trust-region mode step (ADR 0660 arithmetic 5).
pub const TRUST_REGION_DAMPING_FLOOR: f64 = 1e-16;

/// The gain ratio above which a trial of the trust-region mode step is accepted (ADR 0660 arithmetic 5, `eta`).
pub const TRUST_REGION_ACCEPT: f64 = 1e-4;

/// The deviatoric part of a Voigt stress (the mean of the normal components removed; ADR 0630 D1).
fn deviatoric(s: [f64; 6]) -> [f64; 6] {
    let m = (s[0] + s[1] + s[2]) / 3.0;
    [s[0] - m, s[1] - m, s[2] - m, s[3], s[4], s[5]]
}

/// The coefficient of a rule at the (for `Deviatoric` projected) rest stress `s` (ADR 0614 D1, ADR 0630 arithmetic 3).
fn rule_coefficient(rule: IncompatibleStabilisation, s: &[f64; 6], g0: f64) -> f64 {
    match rule {
        IncompatibleStabilisation::Off => 0.0,
        IncompatibleStabilisation::LinearStress => {
            let ss = inner(s, s);
            ss / (g0 + ss.sqrt())
        }
        _ => inner(s, s) / g0,
    }
}

/// The stabilisation of ADR 0614 D1 at a nodal displacement: the coefficient `c`, its gradient and Hessian over the 24
/// nodal unknowns (row-major), and the matrix `M` of `P(alpha) = alpha^T M alpha = sum_q w_q |F_enh,q|^2`.
struct Penalty {
    c: f64,
    dc: Vec<f64>,
    d2c: Vec<f64>,
    m: [[f64; MODES]; MODES],
}

type M3 = [[f64; 3]; 3];

fn mul3(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j]))
}

fn inverse3(a: &M3) -> M3 {
    let det = det3(a);
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            (a[r1][c1] * a[r2][c2] - a[r1][c2] * a[r2][c1]) / det
        })
    })
}

fn transpose3(a: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}

/// `D0 : voigt(e)` of a symmetric tensor `e` (engineering shear in, tensor stress out).
fn rest_stress(d0: &Matrix6, e: &M3) -> [f64; 6] {
    let v = [e[0][0], e[1][1], e[2][2], 2.0 * e[0][1], 2.0 * e[1][2], 2.0 * e[2][0]];
    std::array::from_fn(|i| (0..6).map(|j| d0[i][j] * v[j]).sum())
}

/// The tensor inner product of two Voigt stresses (tensor shear).
fn inner(a: &[f64; 6], b: &[f64; 6]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + 2.0 * (a[3] * b[3] + a[4] * b[4] + a[5] * b[5])
}

/// The rest elasticity and the mean shear modulus of the stabilisation (ADR 0614 D1): of the material, or of its elastic
/// base for J2, creep and viscoelastic materials (ADR 0614 Limits).
fn rest_moduli(material: &Material, context: &PointContext) -> Result<(Matrix6, f64)> {
    let elastic = match material {
        Material::J2 { .. } | Material::Creep { .. } | Material::Viscoelastic { .. } => {
            material.base().unwrap_or(material)
        }
        _ => material,
    };
    let d0 = elastic.linear_elasticity_at(context)?;
    let g0 = (d0[3][3] + d0[4][4] + d0[5][5]) / 3.0;
    if g0.is_nan() || g0 <= 0.0 || g0.is_infinite() {
        return Err(FemError::InvalidValue {
            key: ITERATION_KEY,
            reason: format!(
                "the stabilisation of an Incompatible Hex8 needs a positive rest shear modulus, found {g0:e} (ADR 0614 D1)"
            ),
        });
    }
    Ok((d0, g0))
}

/// The compatible gradient at the element centre and the shape gradients there.
fn centre_gradient(coords: &[Vec3], displacement: &[f64]) -> Result<(M3, Vec<[f64; 3]>)> {
    let g = Hex8.point(coords, [0.0; 3])?.grad;
    let mut f = [[0.0; 3]; 3];
    for (a, ga) in g.iter().enumerate() {
        for (i, row) in f.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v += displacement[3 * a + i] * ga[j];
            }
        }
    }
    for (i, row) in f.iter_mut().enumerate() {
        row[i] += 1.0;
    }
    Ok((f, g))
}

/// `e0 = (C0 - C0^{-1})/4` and `C0^{-1}` of a centre gradient.
fn mean_strain(f0: &M3) -> (M3, M3, M3) {
    let c = mul3(&transpose3(f0), f0);
    let ci = inverse3(&c);
    let e = std::array::from_fn(|i| std::array::from_fn(|j| 0.25 * (c[i][j] - ci[i][j])));
    (e, c, ci)
}

/// The penalty of ADR 0614 D1 and arithmetic 5 for one element at its nodal displacement.
fn penalty(
    coords: &[Vec3],
    points: &[ModePoint],
    material: &Material,
    displacement: &[f64],
    context: &PointContext,
    rule: IncompatibleStabilisation,
) -> Result<Penalty> {
    let (d0, g0) = rest_moduli(material, context)?;
    let (f0, g) = centre_gradient(coords, displacement)?;
    let (e, _, ci) = mean_strain(&f0);
    // ADR 0630 D1: rule D takes the deviatoric part of the rest stress and of its derivatives (linear in the stress).
    let project = |v: [f64; 6]| {
        if rule == IncompatibleStabilisation::Deviatoric {
            deviatoric(v)
        } else {
            v
        }
    };
    let s = project(rest_stress(&d0, &e));
    let c = rule_coefficient(rule, &s, g0);
    // ADR 0630 arithmetic 3, rule L: dc = a s : ds, d2c = a (ds : ds + s : d2s) + b (s : ds)(s : ds).
    let soft = if rule == IncompatibleStabilisation::LinearStress {
        let r = inner(&s, &s).sqrt();
        let gr = g0 + r;
        let b = if r > 0.0 {
            -(3.0 * g0 + r) / (gr * gr * gr * r)
        } else {
            0.0
        };
        Some(((2.0 * g0 + r) / (gr * gr), b))
    } else {
        None
    };
    // The nine directions dF = e_i (x) e_J (d = 3 i + J): dC = dF^T F0 + F0^T dF, d(C^-1) = -C^-1 dC C^-1, de = (dC -
    // d(C^-1))/4, ds = D0 de.
    let dcm: Vec<M3> = (0..9)
        .map(|d| {
            let (i, jj) = (d / 3, d % 3);
            std::array::from_fn(|m| {
                std::array::from_fn(|n| (if jj == m { f0[i][n] } else { 0.0 }) + (if jj == n { f0[i][m] } else { 0.0 }))
            })
        })
        .collect();
    let cdc: Vec<M3> = dcm.iter().map(|dc| mul3(&ci, &mul3(dc, &ci))).collect();
    let ds: Vec<[f64; 6]> = (0..9)
        .map(|d| {
            let de: M3 = std::array::from_fn(|m| std::array::from_fn(|n| 0.25 * (dcm[d][m][n] + cdc[d][m][n])));
            project(rest_stress(&d0, &de))
        })
        .collect();
    let dcf: Vec<f64> = ds
        .iter()
        .map(|dsd| match soft {
            None => 2.0 * inner(&s, dsd) / g0,
            Some((a, _)) => a * inner(&s, dsd),
        })
        .collect();
    let mut d2cf = [[0.0; 9]; 9];
    for a in 0..9 {
        for b in a..9 {
            let (ia, ja) = (a / 3, a % 3);
            let (ib, jb) = (b / 3, b % 3);
            // d2C[m][n] = delta_(ia ib) (delta_(ja m) delta_(jb n) + delta_(jb m) delta_(ja n)).
            let d2c_m: M3 = std::array::from_fn(|m| {
                std::array::from_fn(|n| {
                    if ia != ib {
                        0.0
                    } else {
                        (if ja == m && jb == n { 1.0 } else { 0.0 }) + (if jb == m && ja == n { 1.0 } else { 0.0 })
                    }
                })
            });
            // d2(C^-1) = Ci dC_a Ci dC_b Ci + Ci dC_b Ci dC_a Ci - Ci d2C Ci.
            let t1 = mul3(&cdc[a], &mul3(&dcm[b], &ci));
            let t2 = mul3(&cdc[b], &mul3(&dcm[a], &ci));
            let t3 = mul3(&ci, &mul3(&d2c_m, &ci));
            let d2e: M3 = std::array::from_fn(|m| {
                std::array::from_fn(|n| 0.25 * (d2c_m[m][n] - (t1[m][n] + t2[m][n] - t3[m][n])))
            });
            let d2s = project(rest_stress(&d0, &d2e));
            let v = match soft {
                None => 2.0 * (inner(&ds[a], &ds[b]) + inner(&s, &d2s)) / g0,
                Some((sa, sb)) => {
                    sa * (inner(&ds[a], &ds[b]) + inner(&s, &d2s)) + sb * inner(&s, &ds[a]) * inner(&s, &ds[b])
                }
            };
            d2cf[a][b] = v;
            d2cf[b][a] = v;
        }
    }
    // To the nodal unknowns: dF0_iJ / du_(a,i) = g_a(0)_J.
    let mut dc = vec![0.0; NODES];
    for (a, ga) in g.iter().enumerate() {
        for i in 0..3 {
            dc[3 * a + i] = (0..3).map(|jj| dcf[3 * i + jj] * ga[jj]).sum();
        }
    }
    let mut d2c = vec![0.0; NODES * NODES];
    for (a, ga) in g.iter().enumerate() {
        for i in 0..3 {
            for (b, gb) in g.iter().enumerate() {
                for k in 0..3 {
                    let mut t = 0.0;
                    for (jj, gaj) in ga.iter().enumerate() {
                        for (l, gbl) in gb.iter().enumerate() {
                            t += d2cf[3 * i + jj][3 * k + l] * gaj * gbl;
                        }
                    }
                    d2c[(3 * a + i) * NODES + 3 * b + k] = t;
                }
            }
        }
    }
    let mut m = [[0.0; MODES]; MODES];
    for p in points {
        for k in 0..3 {
            for l in 0..3 {
                let gk = &p.modes[k];
                let gl = &p.modes[l];
                let gg = p.weight * (gk[0] * gl[0] + gk[1] * gl[1] + gk[2] * gl[2]);
                for i in 0..3 {
                    m[3 * k + i][3 * l + i] += gg;
                }
            }
        }
    }
    Ok(Penalty { c, dc, d2c, m })
}

/// The penalty of a stabilised element, `None` without the stabilisation (ADR 0614 D1, ADR 0630 D1).
fn penalty_if(
    stabilised: IncompatibleStabilisation,
    coords: &[Vec3],
    points: &[ModePoint],
    material: &Material,
    displacement: &[f64],
    context: &PointContext,
) -> Result<Option<Penalty>> {
    if stabilised == IncompatibleStabilisation::Off {
        Ok(None)
    } else {
        Ok(Some(penalty(
            coords,
            points,
            material,
            displacement,
            context,
            stabilised,
        )?))
    }
}

/// The extended element of arithmetic 1 at the unknowns `v = (u, alpha)`; with `pen` the penalty of ADR 0614 D1 added.
fn extended(
    points: &[ModePoint],
    material: &Material,
    v: &[f64],
    state: &ElementState,
    context: &PointContext,
    pen: Option<&Penalty>,
) -> Result<Extended> {
    let ns = check_state(material, state, points.len())?;
    let mut force = vec![0.0; ALL];
    let mut k = vec![0.0; ALL * ALL];
    let mut stress = Vec::with_capacity(points.len());
    let mut strain = Vec::with_capacity(points.len());
    let mut new_state = Vec::new();
    let mut scale = [0.0; MODES];
    let mut db = vec![0.0; 6 * ALL];
    for (q, p) in points.iter().enumerate() {
        let grads: Vec<[f64; 3]> = p.grad.iter().chain(&p.modes).copied().collect();
        let h = displacement_gradient(&grads, v);
        let mut f = h;
        for (i, row) in f.iter_mut().enumerate() {
            row[i] += 1.0;
        }
        let det_f = det3(&f);
        if det_f.is_nan() || det_f <= 0.0 || det_f.is_infinite() {
            return Err(FemError::InvalidValue {
                key: INVERSION_KEY,
                reason: format!("det F = {det_f:e} at point {q} of an Incompatible Hex8 element (inverted)"),
            });
        }
        let e = green_lagrange(&h);
        let committed = &state.values()[q * ns..(q + 1) * ns];
        let (s, d) = point_update(material, &e, committed, context, &mut new_state)?;
        let b = b_nonlinear(&grads, &f);
        let w = p.weight;
        for (c, fc) in force.iter_mut().enumerate() {
            let mut t = 0.0;
            for (m, sm) in s.iter().enumerate() {
                t += b[m * ALL + c] * sm;
            }
            *fc += w * t;
        }
        for (j, sc) in scale.iter_mut().enumerate() {
            let c = NODES + j;
            let mut t = 0.0;
            for (m, sm) in s.iter().enumerate() {
                t += (b[m * ALL + c] * sm).abs();
            }
            *sc += w * t;
        }
        // The material part.
        for r in 0..6 {
            for c in 0..ALL {
                let mut t = 0.0;
                for (m, drm) in d[r].iter().enumerate() {
                    t += drm * b[m * ALL + c];
                }
                db[r * ALL + c] = t;
            }
        }
        for i in 0..ALL {
            for j in 0..ALL {
                let mut t = 0.0;
                for m in 0..6 {
                    t += b[m * ALL + i] * db[m * ALL + j];
                }
                k[i * ALL + j] += w * t;
            }
        }
        // The geometric part over the eleven gradients: delta_ij grad_m . S grad_n.
        let st = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
        for (m, gm) in grads.iter().enumerate() {
            let sg: [f64; 3] = std::array::from_fn(|r| st[r][0] * gm[0] + st[r][1] * gm[1] + st[r][2] * gm[2]);
            for (n, gn) in grads.iter().enumerate() {
                let g = w * (sg[0] * gn[0] + sg[1] * gn[1] + sg[2] * gn[2]);
                for i in 0..3 {
                    k[(3 * m + i) * ALL + 3 * n + i] += g;
                }
            }
        }
        stress.push(s);
        strain.push(e);
    }
    if let Some(p) = pen {
        // ADR 0614 arithmetic 5: the exact derivatives of (c/2) alpha^T M alpha.
        let alpha = &v[NODES..];
        let ma: [f64; MODES] = std::array::from_fn(|r| (0..MODES).map(|j| p.m[r][j] * alpha[j]).sum());
        let pa: f64 = (0..MODES).map(|r| alpha[r] * ma[r]).sum();
        for i in 0..NODES {
            force[i] += 0.5 * pa * p.dc[i];
            for j in 0..NODES {
                k[i * ALL + j] += 0.5 * pa * p.d2c[i * NODES + j];
            }
            for (r, mar) in ma.iter().enumerate() {
                k[i * ALL + NODES + r] += p.dc[i] * mar;
                k[(NODES + r) * ALL + i] += p.dc[i] * mar;
            }
        }
        for (r, mar) in ma.iter().enumerate() {
            force[NODES + r] += p.c * mar;
            scale[r] += (p.c * mar).abs();
            for j in 0..MODES {
                k[(NODES + r) * ALL + NODES + j] += p.c * p.m[r][j];
            }
        }
    }
    Ok(Extended {
        force,
        tangent: k,
        stress,
        strain,
        state: new_state,
        mode_scale: scale.iter().fold(0.0f64, |a, &b| a.max(b)),
    })
}

/// The LU factors (partial pivoting) of the 9 x 9 mode block of an extended tangent.
struct ModeFactor {
    lu: [[f64; MODES]; MODES],
    perm: [usize; MODES],
}

impl ModeFactor {
    fn new(k: &[f64]) -> Result<ModeFactor> {
        let mut lu = [[0.0; MODES]; MODES];
        let mut amax = 0.0f64;
        for (i, row) in lu.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = k[(NODES + i) * ALL + NODES + j];
                amax = amax.max(v.abs());
            }
        }
        let mut perm: [usize; MODES] = std::array::from_fn(|i| i);
        for c in 0..MODES {
            let p = (c..MODES)
                .max_by(|&a, &b| lu[a][c].abs().total_cmp(&lu[b][c].abs()))
                .expect("a row");
            let pivot = lu[p][c];
            if pivot.is_nan() || pivot.abs() <= 1e-14 * amax || !pivot.is_finite() {
                return Err(FemError::InvalidValue {
                    key: ITERATION_KEY,
                    reason: format!(
                        "the mode matrix of an Incompatible Hex8 is singular (pivot {pivot:e} at column {c}, largest entry {amax:e})"
                    ),
                });
            }
            lu.swap(c, p);
            perm.swap(c, p);
            for r in c + 1..MODES {
                let l = lu[r][c] / pivot;
                lu[r][c] = l;
                for j in c + 1..MODES {
                    lu[r][j] -= l * lu[c][j];
                }
            }
        }
        Ok(ModeFactor { lu, perm })
    }

    fn solve(&self, b: &[f64; MODES]) -> [f64; MODES] {
        let mut x: [f64; MODES] = std::array::from_fn(|i| b[self.perm[i]]);
        for i in 0..MODES {
            for j in 0..i {
                x[i] -= self.lu[i][j] * x[j];
            }
        }
        for i in (0..MODES).rev() {
            for j in i + 1..MODES {
                x[i] -= self.lu[i][j] * x[j];
            }
            x[i] /= self.lu[i][i];
        }
        x
    }
}

fn mode_residual(ext: &Extended) -> [f64; MODES] {
    std::array::from_fn(|k| ext.force[NODES + k])
}

/// One accepted iterate of the local iteration of the modes (ADR 0658 D1, for checks): the 2-norm and the largest magnitude
/// of the mode residual, the scale of its terms, and the halvings of the step that led to it (0 at `alpha = 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeIterate {
    /// `|r|_2` of the mode residual at the iterate.
    pub residual_norm: f64,
    /// `|r|_inf` of the mode residual at the iterate.
    pub residual_max: f64,
    /// The scale of the mode residual (the convergence test is `|r|_inf <= LOCAL_TOLERANCE scale`).
    pub scale: f64,
    /// The halvings `m` of the step to this iterate (`t = 2^-m`).
    pub halvings: usize,
}

fn norm2(r: &[f64; MODES]) -> f64 {
    r.iter().map(|v| v * v).sum::<f64>().sqrt()
}

/// The local iteration of arithmetic 2 from `alpha = 0`: the converged modes and the extended element there. With
/// `Backtracking` (ADR 0655 D1, opt-in) a step whose iterate inverts a point is halved, at most [`MODE_HALVINGS`] times;
/// with `BacktrackingWithDecrease` (ADR 0658 D1) also a step without sufficient decrease. With `record` the accepted
/// iterates are pushed to it (for checks; no value depends on it).
#[allow(clippy::too_many_arguments)]
fn converged(
    points: &[ModePoint],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    pen: Option<&Penalty>,
    update: IncompatibleModeUpdate,
    start: Option<[f64; MODES]>,
    mut record: Option<&mut Vec<ModeIterate>>,
) -> Result<([f64; MODES], Extended)> {
    if update == IncompatibleModeUpdate::TrustRegion {
        // ADR 0660 D2: a separate iteration; its iterates recorded as ModeIterate (halvings = rejections).
        let mut trec = record.as_ref().map(|_| Vec::new());
        let out = trust_region(
            points,
            material,
            displacement,
            state,
            context,
            pen,
            start,
            trec.as_mut(),
        );
        if let (Some(rec), Some(t)) = (record, trec) {
            rec.extend(t.iter().map(|i| ModeIterate {
                residual_norm: i.residual_norm,
                residual_max: i.residual_max,
                scale: i.scale,
                halvings: i.rejections,
            }));
        }
        return out;
    }
    let mut v = vec![0.0; ALL];
    v[..NODES].copy_from_slice(displacement);
    // ADR 0660 D1: the warm start from the given modes (None: alpha = 0, the iteration of ADR 0608 D2).
    let mut alpha = start.unwrap_or([0.0; MODES]);
    let mut last = f64::NAN;
    // ADR 0655 D1: the iterate before the last step and the step, for the backtracking; ADR 0658 D1: with the 2-norm of
    // the residual there (NaN without the decrease test).
    let mut step: Option<([f64; MODES], [f64; MODES], f64)> = None;
    let backtracking = update != IncompatibleModeUpdate::Plain;
    let decrease = update == IncompatibleModeUpdate::BacktrackingWithDecrease;
    for it in 0..=LOCAL_ITERATIONS {
        v[NODES..].copy_from_slice(&alpha);
        let (ext, halvings) = if backtracking {
            backtracked(
                points,
                material,
                &mut v,
                &mut alpha,
                step.as_ref(),
                decrease,
                state,
                context,
                pen,
            )?
        } else {
            (extended(points, material, &v, state, context, pen)?, 0)
        };
        let r = mode_residual(&ext);
        let rmax = r.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
        if let Some(rec) = record.as_deref_mut() {
            rec.push(ModeIterate {
                residual_norm: norm2(&r),
                residual_max: rmax,
                scale: ext.mode_scale,
                halvings,
            });
        }
        if !rmax.is_finite() {
            return Err(FemError::InvalidValue {
                key: ITERATION_KEY,
                reason: format!("the mode residual of an Incompatible Hex8 is not finite at local iteration {it}"),
            });
        }
        if rmax <= LOCAL_TOLERANCE * ext.mode_scale {
            return Ok((alpha, ext));
        }
        last = rmax / ext.mode_scale;
        if it == LOCAL_ITERATIONS {
            break;
        }
        let delta = ModeFactor::new(&ext.tangent)?.solve(&r);
        if backtracking {
            step = Some((alpha, delta, if decrease { norm2(&r) } else { f64::NAN }));
        }
        for (a, d) in alpha.iter_mut().zip(&delta) {
            *a -= d;
        }
    }
    Err(FemError::InvalidValue {
        key: ITERATION_KEY,
        reason: format!(
            "the modes of an Incompatible Hex8 did not converge in {LOCAL_ITERATIONS} local iterations (relative mode residual {last:e}, ADR 0608 D2)"
        ),
    })
}

/// One accepted iterate of the trust-region iteration of the modes (ADR 0660 D2, for checks): as [`ModeIterate`], with the
/// rejected trials of the step that led to it, the damping `lambda` and the gain ratio `rho` of that step (NaN at the start).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrustRegionIterate {
    /// `|r|_2` of the mode residual at the iterate.
    pub residual_norm: f64,
    /// `|r|_inf` of the mode residual at the iterate.
    pub residual_max: f64,
    /// The scale of the mode residual.
    pub scale: f64,
    /// The rejected trials of the step to this iterate.
    pub rejections: usize,
    /// The damping `lambda` of the accepted step (`mu = lambda max_i (J^T J)_ii`).
    pub damping: f64,
    /// The gain ratio `rho` of the accepted step.
    pub gain: f64,
}

/// The mode block `J = K_aa` of an extended tangent.
fn mode_block(k: &[f64]) -> [[f64; MODES]; MODES] {
    std::array::from_fn(|i| std::array::from_fn(|c| k[(NODES + i) * ALL + NODES + c]))
}

/// ADR 0660 arithmetic 5: the damped least-squares step `d = argmin |[J; sqrt(mu) I] d - [r; 0]|_2` by Householder QR of
/// the 18 x 9 stacked matrix; `None` if it is rank deficient or not finite.
fn damped_step(j: &[[f64; MODES]; MODES], r: &[f64; MODES], mu: f64) -> Option<[f64; MODES]> {
    const ROWS: usize = 2 * MODES;
    let mut a = [[0.0; MODES]; ROWS];
    let mut b = [0.0; ROWS];
    a[..MODES].copy_from_slice(j);
    b[..MODES].copy_from_slice(r);
    let sm = mu.sqrt();
    for (i, row) in a[MODES..].iter_mut().enumerate() {
        row[i] = sm;
    }
    for c in 0..MODES {
        let norm = a[c..].iter().map(|row| row[c] * row[c]).sum::<f64>().sqrt();
        if norm.is_nan() || norm <= 0.0 || norm.is_infinite() {
            return None;
        }
        let head = if a[c][c] > 0.0 { -norm } else { norm };
        let mut v = [0.0; ROWS];
        for (vi, row) in v[c..].iter_mut().zip(&a[c..]) {
            *vi = row[c];
        }
        v[c] -= head;
        let vv: f64 = v[c..].iter().map(|x| x * x).sum();
        for col in c..MODES {
            let s = 2.0 * v[c..].iter().zip(&a[c..]).map(|(vi, row)| vi * row[col]).sum::<f64>() / vv;
            for (vi, row) in v[c..].iter().zip(a[c..].iter_mut()) {
                row[col] -= s * vi;
            }
        }
        let s = 2.0 * v[c..].iter().zip(&b[c..]).map(|(vi, bi)| vi * bi).sum::<f64>() / vv;
        for (vi, bi) in v[c..].iter().zip(b[c..].iter_mut()) {
            *bi -= s * vi;
        }
    }
    let mut d = [0.0; MODES];
    for i in (0..MODES).rev() {
        let mut t = b[i];
        for k in i + 1..MODES {
            t -= a[i][k] * d[k];
        }
        if a[i][i] == 0.0 {
            return None;
        }
        d[i] = t / a[i][i];
    }
    if d.iter().all(|x| x.is_finite()) {
        Some(d)
    } else {
        None
    }
}

/// ADR 0660 D2: the local iteration of the modes with the Levenberg-Marquardt step from `start` (`None`: `alpha = 0`). A
/// trial `alpha_j - d` is accepted when its gain ratio `rho = (f(0) - f(trial)) / pred` exceeds [`TRUST_REGION_ACCEPT`],
/// `f = |r|_2^2 / 2`, `pred = (|r|^2 - |r - J d|^2) / 2`; an inverted or non-finite trial is rejected. After a rejection or
/// `rho < 1/4` the damping is multiplied by 4, after `rho > 3/4` divided by 4 (not below [`TRUST_REGION_DAMPING_FLOOR`]).
/// At most [`MODE_HALVINGS`] rejections per step and [`LOCAL_ITERATIONS`] accepted steps; the convergence test of ADR 0608
/// D2.
#[allow(clippy::too_many_arguments)]
fn trust_region(
    points: &[ModePoint],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    pen: Option<&Penalty>,
    start: Option<[f64; MODES]>,
    mut record: Option<&mut Vec<TrustRegionIterate>>,
) -> Result<([f64; MODES], Extended)> {
    let mut v = vec![0.0; ALL];
    v[..NODES].copy_from_slice(displacement);
    let mut alpha = start.unwrap_or([0.0; MODES]);
    v[NODES..].copy_from_slice(&alpha);
    let mut ext = extended(points, material, &v, state, context, pen)?;
    let mut lambda = TRUST_REGION_DAMPING;
    let (mut rejections, mut used, mut gain) = (0, f64::NAN, f64::NAN);
    let mut last = f64::NAN;
    for it in 0..=LOCAL_ITERATIONS {
        let r = mode_residual(&ext);
        let rmax = r.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
        let rn = norm2(&r);
        if let Some(rec) = record.as_deref_mut() {
            rec.push(TrustRegionIterate {
                residual_norm: rn,
                residual_max: rmax,
                scale: ext.mode_scale,
                rejections,
                damping: used,
                gain,
            });
        }
        if !rmax.is_finite() {
            return Err(FemError::InvalidValue {
                key: ITERATION_KEY,
                reason: format!("the mode residual of an Incompatible Hex8 is not finite at local iteration {it}"),
            });
        }
        if rmax <= LOCAL_TOLERANCE * ext.mode_scale {
            return Ok((alpha, ext));
        }
        last = rmax / ext.mode_scale;
        if it == LOCAL_ITERATIONS {
            break;
        }
        let j = mode_block(&ext.tangent);
        let dmax = (0..MODES)
            .map(|c| j.iter().map(|row| row[c] * row[c]).sum::<f64>())
            .fold(0.0f64, f64::max);
        let f0 = 0.5 * rn * rn;
        rejections = 0;
        loop {
            let mu = lambda * dmax;
            let mut accepted = None;
            let mut rho = f64::NEG_INFINITY;
            if let Some(d) = damped_step(&j, &r, mu) {
                let res: [f64; MODES] =
                    std::array::from_fn(|i| r[i] - j[i].iter().zip(&d).map(|(jik, dk)| jik * dk).sum::<f64>());
                let pred = 0.5 * (rn * rn - norm2(&res).powi(2));
                let trial: [f64; MODES] = std::array::from_fn(|k| alpha[k] - d[k]);
                v[NODES..].copy_from_slice(&trial);
                match extended(points, material, &v, state, context, pen) {
                    Ok(e) => {
                        let ft = 0.5 * norm2(&mode_residual(&e)).powi(2);
                        if pred > 0.0 {
                            rho = (f0 - ft) / pred;
                        }
                        if rho.is_finite() && rho > TRUST_REGION_ACCEPT {
                            accepted = Some((trial, e));
                        }
                    }
                    Err(e) => {
                        let inverted = matches!(&e, FemError::InvalidValue { key, .. } if *key == INVERSION_KEY);
                        if !inverted {
                            return Err(e);
                        }
                    }
                }
            }
            match accepted {
                Some((trial, e)) => {
                    used = lambda;
                    gain = rho;
                    if rho < 0.25 {
                        lambda *= 4.0;
                    } else if rho > 0.75 {
                        lambda = (lambda / 4.0).max(TRUST_REGION_DAMPING_FLOOR);
                    }
                    alpha = trial;
                    ext = e;
                    break;
                }
                None => {
                    if rejections >= MODE_HALVINGS {
                        return Err(FemError::InvalidValue {
                            key: ITERATION_KEY,
                            reason: format!(
                                "the trust-region step of the modes of an Incompatible Hex8 was rejected {} times at local iteration {it} (|r|_2 {rn:e}, ADR 0660 D2)",
                                MODE_HALVINGS + 1
                            ),
                        });
                    }
                    rejections += 1;
                    lambda *= 4.0;
                }
            }
        }
    }
    Err(FemError::InvalidValue {
        key: ITERATION_KEY,
        reason: format!(
            "the modes of an Incompatible Hex8 did not converge in {LOCAL_ITERATIONS} local iterations (relative mode residual {last:e}, ADR 0608 D2)"
        ),
    })
}

/// The most halvings of one local step of the modes with the backtracking (ADR 0655 D1, arithmetic 3).
pub const MODE_HALVINGS: usize = 20;

/// ADR 0655 D1: the extended element at the iterate `alpha` (in `v`); if a point inverts, the last step `(alpha_j, delta)` is
/// halved, `alpha = alpha_j - 2^-m delta`, `m = 1 ..= MODE_HALVINGS`, until it does not. Without a last step (at `alpha =
/// 0`), after the halvings, or on any other error, the error is returned. ADR 0658 D1: with `decrease` a trial is accepted
/// only if also `|r|_2^2 <= (1 - 2 c t) |r_j|_2^2` (`c =` [`DECREASE_FRACTION`], `t = 2^-m`); after the halvings without
/// it the iteration error is returned. Returns the element and the halvings `m`.
#[allow(clippy::too_many_arguments)]
fn backtracked(
    points: &[ModePoint],
    material: &Material,
    v: &mut [f64],
    alpha: &mut [f64; MODES],
    step: Option<&([f64; MODES], [f64; MODES], f64)>,
    decrease: bool,
    state: &ElementState,
    context: &PointContext,
    pen: Option<&Penalty>,
) -> Result<(Extended, usize)> {
    let mut m = 0;
    loop {
        match extended(points, material, v, state, context, pen) {
            Ok(ext) => {
                let Some((base, delta, r0)) = step.filter(|_| decrease) else {
                    return Ok((ext, m));
                };
                // ADR 0658 D1: the sufficient decrease of the mode residual.
                let t = 0.5f64.powi(m as i32);
                let rn = norm2(&mode_residual(&ext));
                if rn * rn <= (1.0 - 2.0 * DECREASE_FRACTION * t) * r0 * r0 {
                    return Ok((ext, m));
                }
                if m >= MODE_HALVINGS {
                    return Err(FemError::InvalidValue {
                        key: ITERATION_KEY,
                        reason: format!(
                            "the mode residual of an Incompatible Hex8 did not decrease sufficiently in {MODE_HALVINGS} halvings of a local step (|r|_2 {rn:e} against {r0:e}, ADR 0658 D1)"
                        ),
                    });
                }
                m += 1;
                let t = 0.5f64.powi(m as i32);
                *alpha = std::array::from_fn(|k| base[k] - t * delta[k]);
                v[NODES..].copy_from_slice(alpha);
            }
            Err(e) => {
                let inverted = matches!(&e, FemError::InvalidValue { key, .. } if *key == INVERSION_KEY);
                match step {
                    Some((base, delta, _)) if inverted && m < MODE_HALVINGS => {
                        m += 1;
                        let t = 0.5f64.powi(m as i32);
                        *alpha = std::array::from_fn(|k| base[k] - t * delta[k]);
                        v[NODES..].copy_from_slice(alpha);
                    }
                    _ => return Err(e),
                }
            }
        }
    }
}

/// The condensed element of ADR 0608 D2 for the driver; with a rule other than `Off` the element of ADR 0614 D1 with the
/// coefficient of the rule (ADR 0630 D1); with an `update` other than `Plain` the local iteration of ADR 0655 D1 or ADR 0658
/// D1.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
    context: &PointContext,
    stabilised: IncompatibleStabilisation,
    update: impl Into<IncompatibleModeUpdate>,
    warm: bool,
) -> Result<Evaluation> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    // ADR 0660 D1: with the warm start, the state may carry the committed modes after the material state.
    let split = if warm {
        Some(warm_state(material, state, points.len())?)
    } else {
        check_state(material, state, points.len())?;
        None
    };
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let update = update.into();
    let (alpha, ext) = match &split {
        None => converged(
            &points,
            material,
            displacement,
            state,
            context,
            pen.as_ref(),
            update,
            None,
            None,
        )?,
        Some((material_state, start)) => {
            let first = start.map(|s| {
                converged(
                    &points,
                    material,
                    displacement,
                    material_state,
                    context,
                    pen.as_ref(),
                    update,
                    Some(s),
                    None,
                )
            });
            match first {
                Some(Ok(found)) => found,
                // ADR 0660 D1: no committed modes, or the warm iteration failed: the iteration from alpha = 0.
                _ => converged(
                    &points,
                    material,
                    displacement,
                    material_state,
                    context,
                    pen.as_ref(),
                    update,
                    None,
                    None,
                )?,
            }
        }
    };
    let factor = ModeFactor::new(&ext.tangent)?;
    let kt = &ext.tangent;
    let y = factor.solve(&mode_residual(&ext));
    let mut force = ext.force[..NODES].to_vec();
    for (i, fi) in force.iter_mut().enumerate() {
        let mut t = 0.0;
        for (k, yk) in y.iter().enumerate() {
            t += kt[i * ALL + NODES + k] * yk;
        }
        *fi -= t;
    }
    let tangent = if want_tangent {
        // X = K_aa^{-1} K_au, column j from column j of K_au.
        let x: Vec<[f64; MODES]> = (0..NODES)
            .map(|j| factor.solve(&std::array::from_fn(|k| kt[(NODES + k) * ALL + j])))
            .collect();
        let s = |i: usize, j: usize| -> f64 { (0..MODES).map(|k| kt[i * ALL + NODES + k] * x[j][k]).sum() };
        let mut m = ElementMatrix::zeros(NODES);
        let mv = m.values_mut();
        for i in 0..NODES {
            for j in i..NODES {
                let sym = 0.5 * (s(i, j) + s(j, i));
                mv[i * NODES + j] = kt[i * ALL + j] - sym;
                mv[j * NODES + i] = kt[j * ALL + i] - sym;
            }
        }
        Some(m)
    } else {
        None
    };
    let mut trial_state = ext.state;
    if warm {
        // ADR 0660 D1: the converged modes after the material trial state.
        trial_state.extend_from_slice(&alpha);
    }
    Ok(Evaluation {
        force,
        tangent,
        stress: ext.stress,
        state: trial_state,
    })
}

/// ADR 0660 D1: the material part of a warm element state and the committed modes, if present (a state of `points x
/// state_size` values, or of nine more).
fn warm_state(
    material: &Material,
    state: &ElementState,
    points: usize,
) -> Result<(ElementState, Option<[f64; MODES]>)> {
    let n = points * material.state_size();
    let values = state.values();
    if values.len() == n {
        Ok((ElementState::new(values.to_vec()), None))
    } else if values.len() == n + MODES {
        Ok((
            ElementState::new(values[..n].to_vec()),
            Some(std::array::from_fn(|k| values[n + k])),
        ))
    } else {
        Err(FemError::FieldLength {
            key: "state",
            expected: n + MODES,
            found: values.len(),
        })
    }
}

/// The points of the stiffness rule of the element (2 x 2 x 2, the points of `mode_points`).
const POINTS: usize = 8;

/// ADR 0729 D1: whether a state given to a public entry point is a warm state, the material layout (`8 x state_size`
/// values) followed by the nine committed modes (ADR 0660 D1). A state of the material layout is not one and takes the path
/// it took before; a state of any other length is refused as before.
pub(crate) fn given_warm(material: &Material, state: &ElementState) -> bool {
    state.values().len() == POINTS * material.state_size() + MODES
}

/// The converged modes (order `3 k + c`) and the Green-Lagrange strain at the points of the stiffness rule (Voigt,
/// engineering shear) of an Incompatible Hex8 for its displacement (24 values, node-major), from the committed `state`, at
/// the point context `context` (ADR 0608 D2).
pub fn modes_and_strains(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
) -> Result<([f64; MODES], Vec<[f64; 6]>)> {
    modes_and_strains_with(coords, material, displacement, state, context, false)
}

/// [`modes_and_strains`] of the element with (`stabilised`, ADR 0614 D1; a rule of ADR 0630 D1) or without the
/// stabilisation.
///
/// ADR 0729 D1, D2: `state` is the material state, or the material state followed by the nine committed modes of the warm
/// start (ADR 0660 D1, a committed state of the driver with the warm start); the local iteration then starts from those
/// modes and, if it fails, runs from `alpha = 0`.
pub fn modes_and_strains_with(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
) -> Result<([f64; MODES], Vec<[f64; 6]>)> {
    if given_warm(material, state) {
        return modes_and_strains_warm(
            coords,
            material,
            displacement,
            state,
            context,
            stabilised.into(),
            IncompatibleModeUpdate::Plain,
        );
    }
    modes_and_strains_opt(coords, material, displacement, state, context, stabilised.into(), false)
}

/// [`modes_and_strains_with`] with the local update of the model's opt-in (`update`: ADR 0655 D1, ADR 0658 D1; `false` is
/// `Plain`).
pub(crate) fn modes_and_strains_opt(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: IncompatibleStabilisation,
    update: impl Into<IncompatibleModeUpdate>,
) -> Result<([f64; MODES], Vec<[f64; 6]>)> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let (alpha, ext) = converged(
        &points,
        material,
        displacement,
        state,
        context,
        pen.as_ref(),
        update.into(),
        None,
        None,
    )?;
    Ok((alpha, ext.strain))
}

/// [`modes_and_strains_opt`] from the committed modes (ADR 0666 D2, the energy record of the transient path with the warm
/// start): `state` the material state, optionally followed by the nine committed modes; the local iteration starts from them
/// and, if there are none or that iteration fails, runs from `alpha = 0` (the start rule of ADR 0660 D1).
pub(crate) fn modes_and_strains_warm(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: IncompatibleStabilisation,
    update: impl Into<IncompatibleModeUpdate>,
) -> Result<([f64; MODES], Vec<[f64; 6]>)> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    let (material_state, start) = warm_state(material, state, points.len())?;
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let update = update.into();
    let first = start.map(|s| {
        converged(
            &points,
            material,
            displacement,
            &material_state,
            context,
            pen.as_ref(),
            update,
            Some(s),
            None,
        )
    });
    let (alpha, ext) = match first {
        Some(Ok(found)) => found,
        _ => converged(
            &points,
            material,
            displacement,
            &material_state,
            context,
            pen.as_ref(),
            update,
            None,
            None,
        )?,
    };
    Ok((alpha, ext.strain))
}

/// The condensed force (24 values), tangent (24 x 24) and second Piola-Kirchhoff stress at the points of an Incompatible
/// Hex8, with (`stabilised`, ADR 0614 D1; a rule of ADR 0630 D1) or without (ADR 0608 D2) the stabilisation, from the
/// committed `state` at the point context `context`. `state` may carry the nine committed modes of the warm start after the
/// material state (ADR 0729 D1, D2): the evaluation is then the driver's warm evaluation of ADR 0660 D1.
pub fn condensed_force_and_tangent(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
) -> Result<(Vec<f64>, ElementMatrix, Vec<[f64; 6]>)> {
    let ev = evaluate(
        coords,
        material,
        displacement,
        state,
        true,
        context,
        stabilised.into(),
        false,
        given_warm(material, state),
    )?;
    Ok((ev.force, ev.tangent.expect("tangent requested"), ev.stress))
}

/// [`condensed_force_and_tangent`] with the backtracking of the mode update of ADR 0655 D1 (for checks).
pub fn condensed_force_and_tangent_backtracked(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
) -> Result<(Vec<f64>, ElementMatrix, Vec<[f64; 6]>)> {
    let ev = evaluate(
        coords,
        material,
        displacement,
        state,
        true,
        context,
        stabilised.into(),
        true,
        given_warm(material, state),
    )?;
    Ok((ev.force, ev.tangent.expect("tangent requested"), ev.stress))
}

/// [`condensed_force_and_tangent`] with the local update `update` of the modes (ADR 0655 D1, ADR 0658 D1; for checks).
pub fn condensed_force_and_tangent_update(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
    update: IncompatibleModeUpdate,
) -> Result<(Vec<f64>, ElementMatrix, Vec<[f64; 6]>)> {
    let ev = evaluate(
        coords,
        material,
        displacement,
        state,
        true,
        context,
        stabilised.into(),
        update,
        given_warm(material, state),
    )?;
    Ok((ev.force, ev.tangent.expect("tangent requested"), ev.stress))
}

/// The modes and the accepted iterates of a local iteration (ADR 0658 D1).
pub type ModeRecord = (Vec<ModeIterate>, Result<[f64; MODES]>);

/// The accepted iterates of the local iteration of the modes of an Incompatible Hex8 with the local update `update` (ADR
/// 0658 D1, for checks), from the same iteration as the driver's evaluation, and its outcome: the converged modes or the
/// error. With a warm `state` (ADR 0729 D2: the material state followed by the nine committed modes) the iteration from those
/// modes, or, if its outcome is an error, the iteration from `alpha = 0` (the start rule of ADR 0660 D1).
pub fn mode_iteration_record(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
    update: IncompatibleModeUpdate,
) -> Result<ModeRecord> {
    mode_record(
        coords,
        material,
        displacement,
        state,
        context,
        stabilised.into(),
        update,
        None,
    )
}

/// [`mode_iteration_record`] from the modes `start` (ADR 0660 D1, for checks; `None`: from `alpha = 0`, or from the committed
/// modes of a warm `state` as [`mode_iteration_record`]; a given `start` is used as given, ADR 0729 D2). With `TrustRegion`
/// the iterates of [`trust_region_record`] (halvings: the rejected trials).
#[allow(clippy::too_many_arguments)]
pub fn mode_iteration_record_from(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
    update: IncompatibleModeUpdate,
    start: Option<&[f64; MODES]>,
) -> Result<ModeRecord> {
    mode_record(
        coords,
        material,
        displacement,
        state,
        context,
        stabilised.into(),
        update,
        start,
    )
}

/// ADR 0729 D1: the material state of a state given to an entry point if it is a warm state, and its committed modes (a
/// state of the material layout is checked as before and used as given).
type GivenState = (Option<ElementState>, Option<[f64; MODES]>);

fn given_state(material: &Material, state: &ElementState, points: usize) -> Result<GivenState> {
    if given_warm(material, state) {
        let (m, modes) = warm_state(material, state, points)?;
        Ok((Some(m), modes))
    } else {
        check_state(material, state, points)?;
        Ok((None, None))
    }
}

/// ADR 0729 D2: the iteration `run` from the given `start`; without one, from the committed modes and, if its outcome is
/// an error, from `alpha = 0`; without either, from `alpha = 0`.
type Run<T> = (Vec<T>, Result<[f64; MODES]>);

fn record_with_start<T>(
    start: Option<&[f64; MODES]>,
    committed: Option<[f64; MODES]>,
    run: impl Fn(Option<[f64; MODES]>) -> Run<T>,
) -> Run<T> {
    match (start, committed) {
        (Some(s), _) => run(Some(*s)),
        (None, Some(c)) => {
            let first = run(Some(c));
            if first.1.is_ok() {
                first
            } else {
                run(None)
            }
        }
        (None, None) => run(None),
    }
}

/// [`mode_iteration_record_from`] with the rule converted.
#[allow(clippy::too_many_arguments)]
fn mode_record(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: IncompatibleStabilisation,
    update: IncompatibleModeUpdate,
    start: Option<&[f64; MODES]>,
) -> Result<ModeRecord> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    let (cut, committed) = given_state(material, state, points.len())?;
    let state = cut.as_ref().unwrap_or(state);
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    Ok(record_with_start(start, committed, |s| {
        let mut rec = Vec::new();
        let out = converged(
            &points,
            material,
            displacement,
            state,
            context,
            pen.as_ref(),
            update,
            s,
            Some(&mut rec),
        )
        .map(|(alpha, _)| alpha);
        (rec, out)
    }))
}

/// The modes and the accepted iterates of a trust-region iteration (ADR 0660 D2).
pub type TrustRegionRecord = (Vec<TrustRegionIterate>, Result<[f64; MODES]>);

/// The accepted iterates of the trust-region iteration of the modes of an Incompatible Hex8 (ADR 0660 D2, for checks) from
/// `start` (`None`: `alpha = 0`, or the committed modes of a warm `state` and, if that iteration fails, `alpha = 0`, ADR
/// 0729 D2), from the same iteration as the driver's evaluation, and its outcome.
pub fn trust_region_record(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
    start: Option<&[f64; MODES]>,
) -> Result<TrustRegionRecord> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    let (cut, committed) = given_state(material, state, points.len())?;
    let state = cut.as_ref().unwrap_or(state);
    let pen = penalty_if(stabilised.into(), coords, &points, material, displacement, context)?;
    Ok(record_with_start(start, committed, |s| {
        let mut rec = Vec::new();
        let out = trust_region(
            &points,
            material,
            displacement,
            state,
            context,
            pen.as_ref(),
            s,
            Some(&mut rec),
        )
        .map(|(alpha, _)| alpha);
        (rec, out)
    }))
}

/// The condensed force, tangent and stress of the driver's warm evaluation (ADR 0660 D1) and its trial state (the material
/// trial state, then the nine converged modes).
pub type WarmEvaluation = (Vec<f64>, ElementMatrix, Vec<[f64; 6]>, Vec<f64>);

/// [`condensed_force_and_tangent_update`] with the warm start of ADR 0660 D1 (for checks): `state` the material state,
/// optionally followed by the nine committed modes the iteration starts from.
pub fn condensed_force_and_tangent_warm(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
    update: IncompatibleModeUpdate,
) -> Result<WarmEvaluation> {
    let ev = evaluate(
        coords,
        material,
        displacement,
        state,
        true,
        context,
        stabilised.into(),
        update,
        true,
    )?;
    Ok((ev.force, ev.tangent.expect("tangent requested"), ev.stress, ev.state))
}

/// The compatible displacement gradient `H_c` and the mode part `F_m` at a point (ADR 0655 reading c2).
pub type PointGradients = ([[f64; 3]; 3], [[f64; 3]; 3]);

/// Per point of the stiffness rule of an Incompatible Hex8 (ADR 0608 D1), the compatible displacement gradient `H_c = sum_a
/// u_a (x) g_a` and the mode part `F_m = sum_k alpha_k (x) G_k` at the displacement and the modes, so that `F = I + H_c +
/// F_m` (ADR 0655 reading c2; for checks).
pub fn point_gradients(coords: &[Vec3], displacement: &[f64], modes: &[f64; MODES]) -> Result<Vec<PointGradients>> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    Ok(points
        .iter()
        .map(|p| {
            (
                displacement_gradient(&p.grad, displacement),
                displacement_gradient(&p.modes, modes),
            )
        })
        .collect())
}

/// The coefficient `c` (Pa) of the stabilisation of ADR 0614 D1 of an element at its displacement.
pub fn stabilisation_coefficient(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    context: &PointContext,
) -> Result<f64> {
    stabilisation_coefficient_with(
        coords,
        material,
        displacement,
        context,
        IncompatibleStabilisation::RestStress,
    )
}

/// The coefficient `c` (Pa) of the stabilisation of an element at its displacement by a rule (ADR 0630 D2; 0 for `Off`).
pub fn stabilisation_coefficient_with(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    context: &PointContext,
    rule: IncompatibleStabilisation,
) -> Result<f64> {
    check_hex8_fields(coords, displacement)?;
    if rule == IncompatibleStabilisation::Off {
        return Ok(0.0);
    }
    let (d0, g0) = rest_moduli(material, context)?;
    let (f0, _) = centre_gradient(coords, displacement)?;
    let s = rest_stress(&d0, &mean_strain(&f0).0);
    let s = if rule == IncompatibleStabilisation::Deviatoric {
        deviatoric(s)
    } else {
        s
    };
    Ok(rule_coefficient(rule, &s, g0))
}

/// The energy `Pi_s = (c/2) sum_q w_q |F_enh,q|^2` of the stabilisation of ADR 0614 D1 at the displacement and the modes.
pub fn stabilisation_energy(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    modes: &[f64; MODES],
    context: &PointContext,
) -> Result<f64> {
    stabilisation_energy_with(
        coords,
        material,
        displacement,
        modes,
        context,
        IncompatibleStabilisation::RestStress,
    )
}

/// The energy `Pi_s` of the stabilisation by a rule (ADR 0630 D2; 0 for `Off`).
pub fn stabilisation_energy_with(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    modes: &[f64; MODES],
    context: &PointContext,
    rule: IncompatibleStabilisation,
) -> Result<f64> {
    check_hex8_fields(coords, displacement)?;
    if rule == IncompatibleStabilisation::Off {
        return Ok(0.0);
    }
    let points = mode_points(coords)?;
    let p = penalty(coords, &points, material, displacement, context, rule)?;
    let mut pa = 0.0;
    for (r, row) in p.m.iter().enumerate() {
        for (j, mrj) in row.iter().enumerate() {
            pa += modes[r] * mrj * modes[j];
        }
    }
    Ok(0.5 * p.c * pa)
}

/// The force (33 values: the 24 nodal unknowns, then the nine modes) and the tangent (33 x 33) of the extended Incompatible
/// Hex8 at the displacement and the given modes, without condensation (ADR 0608 D1; for checks). Of a warm `state` (ADR 0729
/// D2) the material part is used and the committed modes are ignored.
pub fn enhanced_force_and_tangent(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    modes: &[f64; MODES],
    state: &ElementState,
    context: &PointContext,
) -> Result<(Vec<f64>, ElementMatrix)> {
    enhanced_force_and_tangent_with(coords, material, displacement, modes, state, context, false)
}

/// [`enhanced_force_and_tangent`] with (`stabilised`, ADR 0614 D1; a rule of ADR 0630 D1) or without the stabilisation.
pub fn enhanced_force_and_tangent_with(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    modes: &[f64; MODES],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
) -> Result<(Vec<f64>, ElementMatrix)> {
    let stabilised = stabilised.into();
    check_hex8_fields(coords, displacement)?;
    if let Some(i) = modes.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: ITERATION_KEY,
            reason: format!("mode {i} is not finite"),
        });
    }
    let points = mode_points(coords)?;
    // ADR 0729 D2: of a warm state the material part; the committed modes are not read (the modes are given).
    let cut = if given_warm(material, state) {
        Some(warm_state(material, state, points.len())?.0)
    } else {
        None
    };
    let state = cut.as_ref().unwrap_or(state);
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let mut v = displacement.to_vec();
    v.extend_from_slice(modes);
    let ext = extended(&points, material, &v, state, context, pen.as_ref())?;
    let mut k = ElementMatrix::zeros(ALL);
    k.values_mut().copy_from_slice(&ext.tangent);
    Ok((ext.force, k))
}
