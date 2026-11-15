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
//! The nonlinear driver ([`crate::nonlinear`]) evaluates a Hex8 of a block of the `Incompatible` formulation by this module.

use crate::element::{ElementMatrix, ElementState};
use crate::error::{FemError, Result};
use crate::hex8x::{mode_points, ModePoint};
use crate::material::PointContext;
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

/// The extended element of arithmetic 1 at the unknowns `v = (u, alpha)`.
fn extended(
    points: &[ModePoint],
    material: &Material,
    v: &[f64],
    state: &ElementState,
    context: &PointContext,
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
            if !(pivot.abs() > 1e-14 * amax) || !pivot.is_finite() {
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
) -> Result<([f64; MODES], Extended)> {
    let mut v = vec![0.0; ALL];
    v[..NODES].copy_from_slice(displacement);
    let mut alpha = [0.0; MODES];
    let mut last = f64::NAN;
    for it in 0..=LOCAL_ITERATIONS {
        v[NODES..].copy_from_slice(&alpha);
        let ext = extended(points, material, &v, state, context)?;
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

/// The condensed element of ADR 0608 D2 for the driver.
pub(crate) fn evaluate(
    coords: &[Vec3],
    material: &Material,
    displacement: &[f64],
    state: &ElementState,
    want_tangent: bool,
    context: &PointContext,
) -> Result<Evaluation> {
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    check_state(material, state, points.len())?;
    let (_, ext) = converged(&points, material, displacement, state, context)?;
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
    check_hex8_fields(coords, displacement)?;
    let points = mode_points(coords)?;
    let (alpha, ext) = converged(&points, material, displacement, state, context)?;
    Ok((alpha, ext.strain))
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
    check_hex8_fields(coords, displacement)?;
    if let Some(i) = modes.iter().position(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: ITERATION_KEY,
            reason: format!("mode {i} is not finite"),
        });
    }
    let points = mode_points(coords)?;
    let mut v = displacement.to_vec();
    v.extend_from_slice(modes);
    let ext = extended(&points, material, &v, state, context)?;
    let mut k = ElementMatrix::zeros(ALL);
    k.values_mut().copy_from_slice(&ext.tangent);
    Ok((ext.force, k))
}
