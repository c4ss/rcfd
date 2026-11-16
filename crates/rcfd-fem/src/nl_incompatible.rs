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

/// The local Newton iteration of arithmetic 2 from `alpha = 0`: the converged modes and the extended element there.
fn converged(
    points: &[ModePoint],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    pen: Option<&Penalty>,
) -> Result<([f64; MODES], Extended)> {
    let mut v = vec![0.0; ALL];
    v[..NODES].copy_from_slice(displacement);
    let mut alpha = [0.0; MODES];
    let mut last = f64::NAN;
    for it in 0..=LOCAL_ITERATIONS {
        v[NODES..].copy_from_slice(&alpha);
        let ext = extended(points, material, &v, state, context, pen)?;
        let r = mode_residual(&ext);
        let rmax = r.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
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

/// The condensed element of ADR 0608 D2 for the driver; with a rule other than `Off` the element of ADR 0614 D1 with the
/// coefficient of the rule (ADR 0630 D1).
pub(crate) fn evaluate(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
    context: &PointContext,
    stabilised: IncompatibleStabilisation,
) -> Result<Evaluation> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    check_state(material, state, points.len())?;
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let (_, ext) = converged(&points, material, displacement, state, context, pen.as_ref())?;
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
    Ok(Evaluation {
        force,
        tangent,
        stress: ext.stress,
        state: ext.state,
    })
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
pub fn modes_and_strains_with(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
) -> Result<([f64; MODES], Vec<[f64; 6]>)> {
    let stabilised = stabilised.into();
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let (alpha, ext) = converged(&points, material, displacement, state, context, pen.as_ref())?;
    Ok((alpha, ext.strain))
}

/// The condensed force (24 values), tangent (24 x 24) and second Piola-Kirchhoff stress at the points of an Incompatible
/// Hex8, with (`stabilised`, ADR 0614 D1; a rule of ADR 0630 D1) or without (ADR 0608 D2) the stabilisation, from the
/// committed `state` at the point context `context`.
pub fn condensed_force_and_tangent(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    context: &PointContext,
    stabilised: impl Into<IncompatibleStabilisation>,
) -> Result<(Vec<f64>, ElementMatrix, Vec<[f64; 6]>)> {
    let ev = evaluate(coords, material, displacement, state, true, context, stabilised.into())?;
    Ok((ev.force, ev.tangent.expect("tangent requested"), ev.stress))
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
/// Hex8 at the displacement and the given modes, without condensation (ADR 0608 D1; for checks).
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
    let pen = penalty_if(stabilised, coords, &points, material, displacement, context)?;
    let mut v = displacement.to_vec();
    v.extend_from_slice(modes);
    let ext = extended(&points, material, &v, state, context, pen.as_ref())?;
    let mut k = ElementMatrix::zeros(ALL);
    k.values_mut().copy_from_slice(&ext.tangent);
    Ok((ext.force, k))
}
