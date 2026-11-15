//! A Shell3 pressure load built from a local curvature estimate, exact for the uniform membrane state on irregular curved
//! triangulations (ADR 0567, F-633). Opt-in through [`crate::Load::shell_pressure_curvature`].
//!
//! The load at node `i` of the loaded set ([`pressure_load_curvature`], ADR 0567 design 1) is
//! `F_i = (p / (2 kappa_i)) (G_i - sum_e (|e| / 2) (nu_e - (nu_e . m_e) m_e))`: `G_i = sum_t dA_t / dx_i`, the CST nodal forces of
//! a unit isotropic membrane tension (`dA_t / dx_i = (1/2) e_3 x (x_k - x_j)`); `p / (2 kappa)` the membrane force that balances
//! the pressure (Laplace's relation); the sum over the boundary edges `e` of the set at `i` replaces each edge's pull
//! `(|e| / 2) nu_e` (`nu_e` the facet's outward conormal) by the part a smooth continuation of the surface across the edge
//! leaves at the node, `m_e` the bisector of the fitted normals at the edge's ends. The uniform isotropic membrane state is then
//! the exact discrete solution on any triangulation inscribed in a sphere (symmetry cuts included), and the hoop state on a
//! circular cylinder of rectangle cells (free hoop edges and symmetry cuts included).
//!
//! The curvature `kappa_i` and the normal `n_i` ([`curvature_estimates`], ADR 0567 design 3) are those of the implicit quadric
//! `a s^2 + b r^2 + c q^2 + d s r + e r q + f q s + g s + k r + q = 0` through the node, fitted by least squares (Householder QR)
//! to the `k_i`-ring of the node, `k_i = max(2, 4 - d_i)` with `d_i` the edge distance to the set's boundary, in the frame of
//! the angle-weighted facet normal with coordinates scaled by the stencil's root-mean-square distance. The estimate is exact
//! on any quadric surface the stencil determines (spheres, cylinders); elsewhere it is consistent. `kappa` is positive where
//! the surface bends away from its `e_3` side.
//!
//! Refused (ADR 0567 design 4): an edge of more than two triangles or an interior edge traversed the same way by both its
//! triangles (key `"orientation"`); a stencil of fewer than [`MIN_STENCIL`] points, a least-squares matrix of numerical rank
//! below eight ([`RANK_TOLERANCE`]) or `|kappa| h` below [`CURVATURE_TOLERANCE`] (key `"curvature"`); triangles refused by
//! [`crate::shell3::facet`]. Every set and map is ordered and every sum runs in a fixed order: the result does not depend on
//! threads or hashing.

use crate::error::{FemError, Result};
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// The smallest number of stencil points of a node (the quadric has eight coefficients).
pub const MIN_STENCIL: usize = 8;

/// A diagonal entry of the QR factor at most this fraction of the largest one means the stencil does not determine a quadric.
pub const RANK_TOLERANCE: f64 = 1e-10;

/// `|kappa| h` below this value (h the stencil's root-mean-square distance) means the node has no curvature to scale by.
pub const CURVATURE_TOLERANCE: f64 = 1e-8;

/// The curvature estimate at one node (ADR 0567 design 3).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct NodalCurvature {
    /// The mean curvature, positive where the surface bends away from its `e_3` side, 1/m.
    pub kappa: f64,
    /// The unit normal of the fitted quadric at the node, on the `e_3` side.
    pub normal: Vec3,
    /// The number of rings of the stencil.
    pub rings: usize,
}

struct Topology {
    /// The nodes used by the triangles, ascending.
    used: Vec<usize>,
    /// Neighbours (local indices), per local node.
    adj: Vec<BTreeSet<usize>>,
    /// The angle-weighted facet normal, per local node.
    aw_normal: Vec<Vec3>,
    /// `G_i`, per local node.
    area_gradient: Vec<Vec3>,
    /// Boundary edges: (from, to, the opposite node, the facet normal), local indices, in edge order.
    boundary: Vec<(usize, usize, usize, Vec3)>,
    /// The edge distance to the boundary, per local node.
    distance: Vec<usize>,
}

fn angle(u: Vec3, v: Vec3) -> f64 {
    norm(cross(u, v)).atan2(dot(u, v))
}

fn topology(nodes: &[Vec3], tris: &[[usize; 3]]) -> Result<Topology> {
    let mut seen = BTreeSet::new();
    for tri in tris {
        for &q in tri {
            if q >= nodes.len() {
                return Err(FemError::MissingNode {
                    role: "curvature pressure",
                    node: q,
                    n_nodes: nodes.len(),
                });
            }
            seen.insert(q);
        }
    }
    let used: Vec<usize> = seen.into_iter().collect();
    let mut local = vec![usize::MAX; nodes.len()];
    for (l, &g) in used.iter().enumerate() {
        local[g] = l;
    }
    let n = used.len();
    let mut adj = vec![BTreeSet::new(); n];
    let mut aw = vec![[0.0; 3]; n];
    let mut g = vec![[0.0; 3]; n];
    // edge (min, max) -> list of (from, to, opposite, triangle)
    let mut edges: BTreeMap<(usize, usize), Vec<(usize, usize, usize, usize)>> = BTreeMap::new();
    let mut normals = Vec::with_capacity(tris.len());
    for (t, tri) in tris.iter().enumerate() {
        let x = tri.map(|q| nodes[q]);
        let f = crate::shell3::facet(&x)?;
        let e3 = f.axes[2];
        normals.push(e3);
        let l = tri.map(|q| local[q]);
        for i in 0..3 {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            let a = angle(sub(x[j], x[i]), sub(x[k], x[i]));
            aw[l[i]] = add(aw[l[i]], scale(e3, a));
            g[l[i]] = add(g[l[i]], scale(cross(e3, sub(x[k], x[j])), 0.5));
            adj[l[i]].insert(l[j]);
            adj[l[i]].insert(l[k]);
            let key = (l[i].min(l[j]), l[i].max(l[j]));
            edges.entry(key).or_default().push((l[i], l[j], l[k], t));
        }
    }
    let mut boundary = Vec::new();
    for (key, list) in &edges {
        match list.len() {
            1 => {
                let (a, b, c, t) = list[0];
                boundary.push((a, b, c, normals[t]));
            }
            2 if list[0].0 != list[1].0 => {}
            2 => {
                return Err(FemError::InvalidValue {
                    key: "orientation",
                    reason: format!(
                        "the edge of nodes {} and {} is traversed the same way by triangles {} and {}: the curvature pressure needs \
                         consistently oriented triangles (ADR 0567 design 4)",
                        used[key.0], used[key.1], list[0].3, list[1].3
                    ),
                });
            }
            m => {
                return Err(FemError::InvalidValue {
                    key: "orientation",
                    reason: format!(
                        "the edge of nodes {} and {} belongs to {m} triangles: the curvature pressure needs a manifold surface (ADR \
                         0567 design 4)",
                        used[key.0], used[key.1]
                    ),
                });
            }
        }
    }
    for v in &mut aw {
        let s = norm(*v);
        *v = scale(*v, 1.0 / s);
    }
    let mut distance = vec![usize::MAX; n];
    let mut front: Vec<usize> = Vec::new();
    for &(a, b, _, _) in &boundary {
        for q in [a, b] {
            if distance[q] != 0 {
                distance[q] = 0;
                front.push(q);
            }
        }
    }
    let mut d = 0;
    while !front.is_empty() {
        d += 1;
        let mut next = Vec::new();
        for &q in &front {
            for &r in &adj[q] {
                if distance[r] > d {
                    distance[r] = d;
                    next.push(r);
                }
            }
        }
        front = next;
    }
    Ok(Topology {
        used,
        adj,
        aw_normal: aw,
        area_gradient: g,
        boundary,
        distance,
    })
}

/// Least squares `min |A c - b|` by Householder QR; `None` when a diagonal entry of `R` is at most [`RANK_TOLERANCE`] of the
/// largest.
fn least_squares(mut a: Vec<[f64; 8]>, mut b: Vec<f64>) -> Option<[f64; 8]> {
    let m = a.len();
    let mut rdiag = [0.0; 8];
    for j in 0..8 {
        let s: f64 = (j..m).map(|i| a[i][j] * a[i][j]).sum::<f64>().sqrt();
        if s == 0.0 {
            rdiag[j] = 0.0;
            continue;
        }
        let alpha = if a[j][j] > 0.0 { -s } else { s };
        let mut v: Vec<f64> = (j..m).map(|i| a[i][j]).collect();
        v[0] -= alpha;
        let vv: f64 = v.iter().map(|x| x * x).sum();
        if vv > 0.0 {
            for col in j..8 {
                let w: f64 = (j..m).map(|i| v[i - j] * a[i][col]).sum::<f64>() * 2.0 / vv;
                for i in j..m {
                    a[i][col] -= w * v[i - j];
                }
            }
            let w: f64 = (j..m).map(|i| v[i - j] * b[i]).sum::<f64>() * 2.0 / vv;
            for i in j..m {
                b[i] -= w * v[i - j];
            }
        }
        rdiag[j] = a[j][j];
    }
    let big = rdiag.iter().fold(0.0f64, |acc, r| acc.max(r.abs()));
    if big.is_nan() || big <= 0.0 || rdiag.iter().any(|r| r.abs() <= RANK_TOLERANCE * big) {
        return None;
    }
    let mut c = [0.0; 8];
    for j in (0..8).rev() {
        let mut s = b[j];
        for l in (j + 1)..8 {
            s -= a[j][l] * c[l];
        }
        c[j] = s / a[j][j];
    }
    Some(c)
}

fn estimates(nodes: &[Vec3], topo: &Topology) -> Result<Vec<NodalCurvature>> {
    let n = topo.used.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let rings = 2usize.max(4usize.saturating_sub(topo.distance[i]));
        let mut ring = BTreeSet::new();
        ring.insert(i);
        let mut front = vec![i];
        for _ in 0..rings {
            let mut next = Vec::new();
            for &q in &front {
                for &r in &topo.adj[q] {
                    if ring.insert(r) {
                        next.push(r);
                    }
                }
            }
            front = next;
        }
        ring.remove(&i);
        let node = topo.used[i];
        if ring.len() < MIN_STENCIL {
            return Err(FemError::InvalidValue {
                key: "curvature",
                reason: format!(
                    "node {node}: its {rings}-ring has {} points, fewer than the {MIN_STENCIL} the quadric fit needs (ADR 0567 design 4)",
                    ring.len()
                ),
            });
        }
        let nz = topo.aw_normal[i];
        let a0 = if nz[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let t1 = {
            let c = cross(nz, a0);
            scale(c, 1.0 / norm(c))
        };
        let t2 = cross(nz, t1);
        let xi = nodes[node];
        let pts: Vec<Vec3> = ring
            .iter()
            .map(|&q| {
                let d = sub(nodes[topo.used[q]], xi);
                [dot(d, t1), dot(d, t2), dot(d, nz)]
            })
            .collect();
        let h = (pts.iter().map(|p| dot(*p, *p)).sum::<f64>() / pts.len() as f64).sqrt();
        let rows: Vec<[f64; 8]> = pts
            .iter()
            .map(|p| {
                let (s, r, q) = (p[0] / h, p[1] / h, p[2] / h);
                [s * s, r * r, q * q, s * r, r * q, q * s, s, r]
            })
            .collect();
        let rhs: Vec<f64> = pts.iter().map(|p| -p[2] / h).collect();
        let c = least_squares(rows, rhs).ok_or_else(|| FemError::InvalidValue {
            key: "curvature",
            reason: format!(
                "node {node}: its {rings}-ring does not determine a quadric (a flat or degenerate patch; ADR 0567 design 4)"
            ),
        })?;
        let grad = [c[6] / h, c[7] / h, 1.0 / h];
        let hs = [
            [2.0 * c[0], c[3], c[5]],
            [c[3], 2.0 * c[1], c[4]],
            [c[5], c[4], 2.0 * c[2]],
        ]
        .map(|row| row.map(|v| v / (h * h)));
        let gn = norm(grad);
        let trace = hs[0][0] + hs[1][1] + hs[2][2];
        let hg = [dot(hs[0], grad), dot(hs[1], grad), dot(hs[2], grad)];
        let kappa = (gn * gn * trace - dot(grad, hg)) / (2.0 * gn * gn * gn);
        if !kappa.is_finite() || kappa.abs() * h < CURVATURE_TOLERANCE {
            return Err(FemError::InvalidValue {
                key: "curvature",
                reason: format!(
                    "node {node}: mean curvature {kappa:e} on a stencil of size {h:e}: the curvature pressure needs a curved surface \
                     (ADR 0567 design 4)"
                ),
            });
        }
        let nl = scale(grad, 1.0 / gn);
        let normal = add(add(scale(t1, nl[0]), scale(t2, nl[1])), scale(nz, nl[2]));
        out.push(NodalCurvature { kappa, normal, rings });
    }
    Ok(out)
}

/// The curvature estimate at every node of the triangles `tris` (indices into `nodes`), as `(node, estimate)` in ascending
/// node order (ADR 0567 design 3).
pub fn curvature_estimates(nodes: &[Vec3], tris: &[[usize; 3]]) -> Result<Vec<(usize, NodalCurvature)>> {
    let topo = topology(nodes, tris)?;
    let est = estimates(nodes, &topo)?;
    Ok(topo.used.iter().copied().zip(est).collect())
}

/// The nodal forces of a uniform pressure `p` along the facet normals `e_3` (positive pushing towards `+e_3`) on the Shell3
/// triangles `tris` (indices into `nodes`), as `(node, force)` in ascending node order, no moments (ADR 0567 design 1).
pub fn pressure_load_curvature(nodes: &[Vec3], tris: &[[usize; 3]], p: f64) -> Result<Vec<(usize, Vec3)>> {
    let topo = topology(nodes, tris)?;
    let est = estimates(nodes, &topo)?;
    let mut f = topo.area_gradient.clone();
    for &(a, b, c, e3) in &topo.boundary {
        let (xa, xb) = (nodes[topo.used[a]], nodes[topo.used[b]]);
        let ed = sub(xb, xa);
        let len = norm(ed);
        let mut nu = cross(ed, e3);
        nu = scale(nu, 1.0 / norm(nu));
        if dot(nu, sub(nodes[topo.used[c]], xa)) > 0.0 {
            nu = scale(nu, -1.0);
        }
        let m = add(est[a].normal, est[b].normal);
        let m = scale(m, 1.0 / norm(m));
        let removed = sub(nu, scale(m, dot(nu, m)));
        for q in [a, b] {
            f[q] = sub(f[q], scale(removed, 0.5 * len));
        }
    }
    Ok(topo
        .used
        .iter()
        .enumerate()
        .map(|(l, &g)| (g, scale(f[l], p / (2.0 * est[l].kappa))))
        .collect())
}
