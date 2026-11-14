//! The free rigid motions of a model, counted from its prescribed displacements, linear constraints, rigid sets and elastic
//! supports (ADR 0399).
//!
//! A small rigid motion `u(x) = t0 + w x (x - x_c)` stores no energy in the elements. It is a free motion of the model if
//! every constraint row holds on it (a prescribed equation `u_i = 0`, a linear row `sum a_k u_k = 0`, each row
//! `u_a - t - w x (x_a - x_R) = 0` of a rigid set, with the reference point moving with the body) and the supports store no
//! energy on it (`u^T K_s u = 0`, equivalently `K_s u = 0`, `K_s` being positive semidefinite). Over the six scaled rigid
//! fields `X_j` these conditions are the rows of a matrix `G` with six columns, and the number of free rigid motions is
//! `6 - rank(G)`, the rank taken from the singular values of `G` (one-sided Jacobi) with the relative threshold
//! [`COUNT_TOLERANCE`]. The rows of a rigid set hold identically on the extended rigid fields (derived in ADR 0399): a
//! rigid set never holds a rigid motion. Mechanisms (zero-energy motions that are not rigid) are not counted.
//!
//! Rotational unknowns (ADR 0426): the rigid field `t0 + w x (x - x_c)` has the rotation `w` at every point, so at a
//! rotational unknown the scaled fields take their rotation; a six-unknown node of a rigid set adds the rows
//! `theta_a - w = 0`, which hold identically on the rigid fields as the translation rows do. The rule is per node and
//! covers the six-unknown nodes of Shell4 elements as well (ADR 0435 D3).

use crate::constraint::{prescribed_equations, Prescribed};
use crate::error::{FemError, Result, SetKind};
use crate::model::{Constraint, SolidModel};
use crate::mpc::support_triplets;
use rcfd_core::vec3::Vec3;
use std::collections::BTreeMap;

/// Relative threshold of the rank of `G` (ADR 0399): a singular value `s_i <= tau_c s_max` is a free direction, and an
/// entry of a row below `tau_c` times the sum of the magnitudes it is made of is a rounding residue.
pub const COUNT_TOLERANCE: f64 = 1e-8;

/// Orthogonality reached by the one-sided Jacobi sweeps: `|g_p . g_q| <= 1e-15 |g_p| |g_q|`.
const JACOBI_ORTHOGONALITY: f64 = 1e-15;
/// Cap of the one-sided Jacobi sweeps.
const JACOBI_SWEEPS: usize = 60;

/// The free rigid motions of a model (ADR 0399).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FreeRigidMotions {
    /// The number of free rigid motions, `6 - rank(G)`.
    pub count: usize,
    /// The singular values of `G`, descending.
    pub singular_values: [f64; 6],
    /// The coefficients `c` (orthonormal) of a basis of the free motions `X c`, over the scaled rigid fields
    /// (`j = 0, 1, 2` translations along x, y, z; `j = 3, 4, 5` rotations about x, y, z through [`Self::centre`] divided by
    /// [`Self::length`]).
    pub free: Vec<[f64; 6]>,
    /// The centre of the rotation fields: the mean of the node coordinates.
    pub centre: Vec3,
    /// The scale of the rotation fields: the largest distance of a node from the centre.
    pub length: f64,
    /// The number of rows of `G` (constraint rows and mesh unknowns with a support entry).
    pub rows: usize,
}

impl FreeRigidMotions {
    /// The free basis as fields `X c` over the mesh unknowns.
    pub fn fields(&self, model: &SolidModel) -> Vec<Vec<f64>> {
        let x: Vec<Vec<f64>> = (0..6)
            .map(|j| rigid_field(model, self.centre, self.length, j))
            .collect();
        self.free
            .iter()
            .map(|c| {
                let mut u = vec![0.0; x[0].len()];
                for (xj, &cj) in x.iter().zip(c) {
                    for (ui, v) in u.iter_mut().zip(xj) {
                        *ui += cj * v;
                    }
                }
                u
            })
            .collect()
    }
}

/// The value of scaled rigid field `j` at point `p` (component `comp`): `e_j` for `j < 3`, `e_(j-3) x (p - centre) / length`
/// for `j >= 3`; at a rotational component `3 + a` the rotation of the field, `0` for `j < 3` and `delta_(a, j-3) / length`
/// for `j >= 3` (ADR 0426).
fn field_at(p: Vec3, centre: Vec3, length: f64, j: usize, comp: usize) -> f64 {
    if comp > 2 {
        // Rotation e_(j-3) / length of field j >= 3 at component 3 + (j - 3) = j.
        return if j == comp { 1.0 / length } else { 0.0 };
    }
    if j < 3 {
        return if comp == j { 1.0 } else { 0.0 };
    }
    let axis = j - 3;
    let d = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
    // (e_a x d)_(a+1) = -d_(a+2), (e_a x d)_(a+2) = d_(a+1), indices cyclic.
    let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
    if comp == a1 {
        -d[a2] / length
    } else if comp == a2 {
        d[a1] / length
    } else {
        0.0
    }
}

/// The scaled rigid field `j` (ADR 0399) over the mesh unknowns of the model.
pub fn rigid_field(model: &SolidModel, centre: Vec3, length: f64, j: usize) -> Vec<f64> {
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    (0..dofs.n_equations())
        .map(|eq| {
            let (node, comp) = dofs.node_component(eq).expect("mesh unknown");
            field_at(nodes[node], centre, length, j, comp)
        })
        .collect()
}

/// Appends the row `g_j = sum_k a_k X_j(k) / max_k |a_k|` from the terms `(a_k, X(k))`; an entry with `|g_j| <= tau_c sum_k
/// |a_k X_j(k)| / max_k |a_k|` is a rounding residue and set to 0.
fn push_row(g: &mut Vec<[f64; 6]>, terms: &[(f64, [f64; 6])]) {
    let amax = terms.iter().fold(0.0f64, |m, t| m.max(t.0.abs()));
    if amax == 0.0 {
        return;
    }
    let mut row = [0.0; 6];
    for (j, r) in row.iter_mut().enumerate() {
        let (mut s, mut mag) = (0.0, 0.0);
        for (a, x) in terms {
            s += a * x[j];
            mag += (a * x[j]).abs();
        }
        *r = if s.abs() <= COUNT_TOLERANCE * mag {
            0.0
        } else {
            s / amax
        };
    }
    g.push(row);
}

/// The singular values (descending) and right singular vectors of the `rows x 6` matrix `g` by one-sided Jacobi
/// (Hestenes): rotations of column pairs until every pair is orthogonal; the singular values are the column norms.
fn singular_values(g: &[[f64; 6]]) -> ([f64; 6], [[f64; 6]; 6]) {
    let mut a: Vec<[f64; 6]> = g.to_vec();
    let mut v = [[0.0; 6]; 6];
    for (i, vi) in v.iter_mut().enumerate() {
        vi[i] = 1.0;
    }
    for _ in 0..JACOBI_SWEEPS {
        let mut rotated = false;
        for p in 0..5 {
            for q in p + 1..6 {
                let (mut alpha, mut beta, mut gamma) = (0.0, 0.0, 0.0);
                for r in &a {
                    alpha += r[p] * r[p];
                    beta += r[q] * r[q];
                    gamma += r[p] * r[q];
                }
                if gamma == 0.0 || gamma.abs() <= JACOBI_ORTHOGONALITY * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                // The rotation [c s; -s c] that makes columns p and q orthogonal: tan of the angle t is the smaller
                // root of t^2 + 2 zeta t - 1 = 0, zeta = (beta - alpha) / (2 gamma).
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for r in a.iter_mut() {
                    let (x, y) = (r[p], r[q]);
                    r[p] = c * x - s * y;
                    r[q] = s * x + c * y;
                }
                for r in v.iter_mut() {
                    let (x, y) = (r[p], r[q]);
                    r[p] = c * x - s * y;
                    r[q] = s * x + c * y;
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let mut sv = [0.0; 6];
    for (j, s) in sv.iter_mut().enumerate() {
        *s = a.iter().map(|r| r[j] * r[j]).sum::<f64>().sqrt();
    }
    let mut order: Vec<usize> = (0..6).collect();
    order.sort_by(|&x, &y| sv[y].total_cmp(&sv[x]).then(x.cmp(&y)));
    let mut sorted = [0.0; 6];
    let mut vectors = [[0.0; 6]; 6];
    for (k, &j) in order.iter().enumerate() {
        sorted[k] = sv[j];
        for i in 0..6 {
            vectors[k][i] = v[i][j];
        }
    }
    (sorted, vectors)
}

/// The free rigid motions of a model with the per-node prescribed displacements `extra` (ADR 0399): the rank of the
/// prescribed equations, the linear constraints, the rows of the rigid sets and the support stiffness on the six scaled
/// rigid fields. The constraint values are not used (the homogeneous rows are counted). Rotational unknowns take the
/// rotation of the fields (ADR 0426). Refused: a cyclic constraint, a mesh without nodes.
pub fn free_rigid_motions(model: &SolidModel, extra: &[Prescribed]) -> Result<FreeRigidMotions> {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let nodes = mesh.nodes();
    if nodes.is_empty() {
        return Err(FemError::InvalidValue {
            key: "mesh",
            reason: "no nodes: the rigid motions are not defined".into(),
        });
    }
    let mut centre = [0.0; 3];
    for x in nodes {
        for i in 0..3 {
            centre[i] += x[i];
        }
    }
    for c in centre.iter_mut() {
        *c /= nodes.len() as f64;
    }
    let length = nodes.iter().fold(0.0f64, |m, x| {
        m.max(((x[0] - centre[0]).powi(2) + (x[1] - centre[1]).powi(2) + (x[2] - centre[2]).powi(2)).sqrt())
    });
    let length = if length > 0.0 { length } else { 1.0 };
    let at = |eq: usize| -> [f64; 6] {
        let (node, comp) = dofs.node_component(eq).expect("mesh unknown");
        std::array::from_fn(|j| field_at(nodes[node], centre, length, j, comp))
    };
    let mut g: Vec<[f64; 6]> = Vec::new();
    // Prescribed equations u_i = 0.
    for &(eq, _) in &prescribed_equations(model, extra)? {
        push_row(&mut g, &[(1.0, at(eq))]);
    }
    for (i, c) in model.constraints().iter().enumerate() {
        match c {
            Constraint::Linear { terms, .. } => {
                let mut row = Vec::with_capacity(terms.len());
                for t in terms {
                    let eq = dofs.equation(t.node, t.component).ok_or(FemError::MissingComponent {
                        role: "linear constraint",
                        node: t.node,
                        component: t.component,
                        available: dofs.components(t.node),
                    })?;
                    row.push((t.coefficient, at(eq)));
                }
                push_row(&mut g, &row);
            }
            Constraint::RigidSet { set, reference } => {
                let members = mesh.node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: SetKind::Node,
                    name: set.clone(),
                })?;
                // The reference point moves with the body: t = X_j(x_R), w = 0 (translations) or e_(j-3) / length.
                let t_ref: [[f64; 6]; 3] =
                    std::array::from_fn(|comp| std::array::from_fn(|j| field_at(*reference, centre, length, j, comp)));
                let w_ref: [[f64; 6]; 3] = std::array::from_fn(|comp| {
                    std::array::from_fn(|j| if j >= 3 && j - 3 == comp { 1.0 / length } else { 0.0 })
                });
                for &node in members {
                    let x = nodes[node];
                    let d = [x[0] - reference[0], x[1] - reference[1], x[2] - reference[2]];
                    for comp in 0..3 {
                        let eq = dofs.equation(node, comp).ok_or(FemError::MissingComponent {
                            role: "rigid set",
                            node,
                            component: comp,
                            available: dofs.components(node),
                        })?;
                        // u_c - t_c - (w x d)_c = u_c - t_c - w_j d_k + w_k d_j, (j, k) = (c + 1, c + 2).
                        let (j, k) = ((comp + 1) % 3, (comp + 2) % 3);
                        let mut row = vec![(1.0, at(eq)), (-1.0, t_ref[comp])];
                        if d[k] != 0.0 {
                            row.push((-d[k], w_ref[j]));
                        }
                        if d[j] != 0.0 {
                            row.push((d[j], w_ref[k]));
                        }
                        push_row(&mut g, &row);
                    }
                    // ADR 0426: a node with rotations turns with the set, theta_c - w_c = 0.
                    if dofs.components(node) == 6 {
                        for comp in 0..3 {
                            let eq = dofs.equation(node, 3 + comp).expect("rotation unknown");
                            push_row(&mut g, &[(1.0, at(eq)), (-1.0, w_ref[comp])]);
                        }
                    }
                }
            }
            Constraint::Cyclic { .. } => {
                return Err(FemError::InvalidValue {
                    key: "free rigid motions",
                    reason: format!("constraint {i} (Cyclic): the count of ADR 0399 does not cover cyclic constraints"),
                });
            }
            _ => {}
        }
    }
    // The support stiffness: one row per mesh unknown with an entry, (K_s X)_i / max_j |K_s(i, j)|.
    let mut ks: BTreeMap<usize, BTreeMap<usize, f64>> = BTreeMap::new();
    for (i, j, v) in support_triplets(model)? {
        *ks.entry(i).or_default().entry(j).or_insert(0.0) += v;
    }
    for row in ks.values() {
        let terms: Vec<(f64, [f64; 6])> = row.iter().map(|(&j, &v)| (v, at(j))).collect();
        push_row(&mut g, &terms);
    }
    let (sv, vectors) = singular_values(&g);
    let smax = sv[0];
    let rank = if smax > 0.0 {
        sv.iter().filter(|&&s| s > COUNT_TOLERANCE * smax).count()
    } else {
        0
    };
    Ok(FreeRigidMotions {
        count: 6 - rank,
        singular_values: sv,
        free: vectors[rank..].to_vec(),
        centre,
        length,
        rows: g.len(),
    })
}
