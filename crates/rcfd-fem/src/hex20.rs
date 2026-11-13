//! Quadratic hexahedron (Hex20): shape functions, the 3 x 3 x 3 rule, its 8-node faces, stiffness, mass and the
//! quadratic upgrade of a hexahedron box. Declared by ADR 0291; filled by ADR 0292.
//!
//! Node order (ADR 0292 D1, the order of VTK type 25): corners 0 to 7 as Hex8 on `[-1, 1]^3`, then the mid-edge node `8 + k`
//! on edge `k` of (0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5), (2,6), (3,7). The functions are the
//! serendipity family (Zienkiewicz and Taylor, The Finite Element Method, vol. 1; derived in ADR 0292 D2): at a corner
//! `(1 + xi xi_a)(1 + eta eta_a)(1 + zeta zeta_a)(xi xi_a + eta eta_a + zeta zeta_a - 2) / 8`, at a mid-edge node with
//! `xi_a = 0` `(1 - xi^2)(1 + eta eta_a)(1 + zeta zeta_a) / 4` and its permutations. Stiffness and mass use the 3 x 3 x 3
//! Gauss rule (D3); the stress recovery fits the 27 triquadratic monomials to the 27 points (D4).
//!
//! Faces have 8 nodes (corners, then the mid-edge nodes of the face edges in order). Their consistent loads are given here
//! as per-node forces ([`pressure_loads`], [`traction_loads`], [`traction_field_loads`]) for
//! [`crate::solve::solve_static_with`] (D5): on a flat face a uniform pressure gives `-1/12` of `p A` at each corner (a force
//! pointing out of the solid) and `1/3` at each mid-edge node. [`upgrade_to_hex20`] and [`upgrade_to_hex20_mapped`] turn a
//! mesh of Hex8 into Hex20 with one node per edge (D6).

use crate::builders::MidEdgeNode;
use crate::element::Element;
use crate::error::{FemError, Result, SetKind};
use crate::loads::NodalForceAt;
use crate::mesh::{ElementKind, SolidMesh};
use crate::quadrature::{self, QuadratureRule};
use rcfd_core::vec3::{cross, norm, Vec3};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The work package that adds Hex20; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The 20-node serendipity hexahedron, fully integrated (3 x 3 x 3 Gauss).
#[derive(Debug, Clone, Copy, Default)]
pub struct Hex20;

static ELEMENT: Hex20 = Hex20;

/// The element of the kind.
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

const REF: [[f64; 3]; 20] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
    // Mid-edge nodes: the means of the corners of (0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5),
    // (2,6), (3,7).
    [0.0, -1.0, -1.0],
    [1.0, 0.0, -1.0],
    [0.0, 1.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, -1.0, 1.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [-1.0, 0.0, 1.0],
    [-1.0, -1.0, 0.0],
    [1.0, -1.0, 0.0],
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
];

const FACES: [&[usize]; 6] = [
    &[0, 3, 2, 1, 11, 10, 9, 8],
    &[4, 5, 6, 7, 12, 13, 14, 15],
    &[0, 1, 5, 4, 8, 17, 12, 16],
    &[1, 2, 6, 5, 9, 18, 13, 17],
    &[2, 3, 7, 6, 10, 19, 14, 18],
    &[3, 0, 4, 7, 11, 16, 15, 19],
];

/// Reference coordinates of the nodes.
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &REF
}

/// The local faces, corners first (counter-clockwise from outside), then the mid-edge nodes of the face edges
/// (c0 c1), (c1 c2), (c2 c3), (c3 c0).
pub fn faces() -> &'static [&'static [usize]] {
    &FACES
}

/// The axis along which a mid-edge node's reference coordinate is 0.
fn zero_axis(r: &[f64; 3]) -> usize {
    if r[0] == 0.0 {
        0
    } else if r[1] == 0.0 {
        1
    } else {
        2
    }
}

/// Shape function values at `xi`, in the node order.
pub fn shape_functions(xi: [f64; 3]) -> Vec<f64> {
    REF.iter()
        .enumerate()
        .map(|(a, r)| {
            let f = [1.0 + xi[0] * r[0], 1.0 + xi[1] * r[1], 1.0 + xi[2] * r[2]];
            if a < 8 {
                let s = xi[0] * r[0] + xi[1] * r[1] + xi[2] * r[2];
                f[0] * f[1] * f[2] * (s - 2.0) / 8.0
            } else {
                let m = zero_axis(r);
                let (p, q) = ((m + 1) % 3, (m + 2) % 3);
                (1.0 - xi[m] * xi[m]) * f[p] * f[q] / 4.0
            }
        })
        .collect()
}

/// Derivatives of the shape functions with respect to the reference coordinates.
pub fn shape_gradients(xi: [f64; 3]) -> Vec<[f64; 3]> {
    REF.iter()
        .enumerate()
        .map(|(a, r)| {
            let f = [1.0 + xi[0] * r[0], 1.0 + xi[1] * r[1], 1.0 + xi[2] * r[2]];
            let mut g = [0.0; 3];
            if a < 8 {
                let s = xi[0] * r[0] + xi[1] * r[1] + xi[2] * r[2];
                for (k, gk) in g.iter_mut().enumerate() {
                    let (p, q) = ((k + 1) % 3, (k + 2) % 3);
                    // d/dxi_k [(1 + xi_k r_k)(s - 2)] = r_k (s - 2) + r_k (1 + xi_k r_k) = r_k (s + xi_k r_k - 1).
                    *gk = r[k] * f[p] * f[q] * (s + xi[k] * r[k] - 1.0) / 8.0;
                }
            } else {
                let m = zero_axis(r);
                let (p, q) = ((m + 1) % 3, (m + 2) % 3);
                let b = 1.0 - xi[m] * xi[m];
                g[m] = -xi[m] * f[p] * f[q] / 2.0;
                g[p] = r[p] * b * f[q] / 4.0;
                g[q] = r[q] * b * f[p] / 4.0;
            }
            g
        })
        .collect()
}

/// The fitted polynomial space of the stress recovery at `xi`: the 27 monomials `xi^i eta^j zeta^k`, `i, j, k <= 2`, with
/// `i` fastest (ADR 0292 D4). Unisolvent on the 3 x 3 x 3 Gauss points.
pub fn fit_basis(xi: [f64; 3]) -> Vec<f64> {
    let p = |t: f64| [1.0, t, t * t];
    let (px, py, pz) = (p(xi[0]), p(xi[1]), p(xi[2]));
    let mut v = Vec::with_capacity(27);
    for z in pz {
        for y in py {
            for x in px {
                v.push(x * y * z);
            }
        }
    }
    v
}

/// The volume of an element, `sum_q w_q det J(xi_q)` over the 3 x 3 x 3 rule: exact for every Hex20 geometry (`det J` has
/// degree <= 5 per direction; ADR 0292 D3).
pub fn volume(jacobian_at: &dyn Fn([f64; 3]) -> f64) -> f64 {
    let rule = quadrature::gauss_3();
    rule.points()
        .iter()
        .zip(rule.weights())
        .map(|(&xi, &w)| w * jacobian_at(xi))
        .sum()
}

impl Element for Hex20 {
    fn kind(&self) -> ElementKind {
        ElementKind::Hex20
    }
    fn shape_functions(&self, xi: [f64; 3]) -> Vec<f64> {
        shape_functions(xi)
    }
    fn shape_derivatives(&self, xi: [f64; 3]) -> Vec<[f64; 3]> {
        shape_gradients(xi)
    }
    fn stiffness_rule(&self) -> &'static QuadratureRule {
        quadrature::gauss_3()
    }
    fn mass_rule(&self) -> &'static QuadratureRule {
        quadrature::gauss_3()
    }
}

// ------------------------------------------------------------------------------------------------------------------ faces

const FACE_REF: [[f64; 2]; 8] = [
    [-1.0, -1.0],
    [1.0, -1.0],
    [1.0, 1.0],
    [-1.0, 1.0],
    [0.0, -1.0],
    [1.0, 0.0],
    [0.0, 1.0],
    [-1.0, 0.0],
];

/// Values and derivatives `(d/du, d/dv)` of the 8-node face functions on `[-1, 1]^2`: corners `(-,-), (+,-), (+,+), (-,+)`
/// with `(1 + u u_a)(1 + v v_a)(u u_a + v v_a - 1) / 4`, then the mid-edge nodes `(0,-1), (1,0), (0,1), (-1,0)` with
/// `(1 - u^2)(1 + v v_a) / 2` or `(1 + u u_a)(1 - v^2) / 2` (the traces of the Hex20 functions).
pub fn face_shape_functions(uv: [f64; 2]) -> ([f64; 8], [[f64; 2]; 8]) {
    let [u, v] = uv;
    let mut n = [0.0; 8];
    let mut d = [[0.0; 2]; 8];
    for (a, r) in FACE_REF.iter().enumerate() {
        let (fu, fv) = (1.0 + u * r[0], 1.0 + v * r[1]);
        if a < 4 {
            let s = u * r[0] + v * r[1];
            n[a] = fu * fv * (s - 1.0) / 4.0;
            d[a] = [
                r[0] * fv * (s + u * r[0]) / 4.0,
                r[1] * fu * (s + v * r[1]) / 4.0,
            ];
        } else if r[0] == 0.0 {
            n[a] = (1.0 - u * u) * fv / 2.0;
            d[a] = [-u * fv, r[1] * (1.0 - u * u) / 2.0];
        } else {
            n[a] = fu * (1.0 - v * v) / 2.0;
            d[a] = [r[0] * (1.0 - v * v) / 2.0, -v * fu];
        }
    }
    (n, d)
}

// Three-point Gauss-Legendre rule on [-1, 1] (as the 3 x 3 x 3 rule of `quadrature`).
const GAUSS3_X: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
const GAUSS3_W: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];

/// The 3 x 3 Gauss rule of a face, as (point, weight), `u` fastest: exact to degree 5 per direction.
pub fn face_rule() -> [([f64; 2], f64); 9] {
    let mut out = [([0.0; 2], 0.0); 9];
    for j in 0..3 {
        for i in 0..3 {
            out[3 * j + i] = ([GAUSS3_X[i], GAUSS3_X[j]], GAUSS3_W[i] * GAUSS3_W[j]);
        }
    }
    out
}

fn face_coords(coords: &[Vec3]) -> Result<()> {
    if coords.len() != 8 {
        return Err(FemError::FieldLength {
            key: "face coordinates",
            expected: 8,
            found: coords.len(),
        });
    }
    if coords.iter().flatten().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "face coordinates",
            reason: format!("{coords:?} is not finite"),
        });
    }
    Ok(())
}

/// The consistent load `f_a = int N_a t dS` of an 8-node face with the given node positions: `t(x, a)` is the force per
/// unit reference area at the point `x`, given the area vector density `a = x_u x x_v` (outward for a face of an element's
/// local face list). Summed over [`face_rule`].
pub fn face_load<T>(coords: &[Vec3], t: T) -> Result<[Vec3; 8]>
where
    T: Fn(Vec3, Vec3) -> Vec3,
{
    face_coords(coords)?;
    let mut fe = [[0.0; 3]; 8];
    for (uv, w) in face_rule() {
        let (n, d) = face_shape_functions(uv);
        let mut xu = [0.0; 3];
        let mut xv = [0.0; 3];
        let mut xq = [0.0; 3];
        for ((xa, da), na) in coords.iter().zip(&d).zip(&n) {
            for i in 0..3 {
                xu[i] += xa[i] * da[0];
                xv[i] += xa[i] * da[1];
                xq[i] += xa[i] * na;
            }
        }
        let tq = t(xq, cross(xu, xv));
        if tq.iter().any(|c| !c.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "face load",
                reason: format!("{tq:?} at {xq:?} is not finite"),
            });
        }
        for (fa, na) in fe.iter_mut().zip(&n) {
            for i in 0..3 {
                fa[i] += w * na * tq[i];
            }
        }
    }
    Ok(fe)
}

/// The area of an 8-node face, the 3 x 3 Gauss sum of `|x_u x x_v|`.
pub fn face_area(coords: &[Vec3]) -> Result<f64> {
    face_coords(coords)?;
    let mut area = 0.0;
    for (uv, w) in face_rule() {
        let (_, d) = face_shape_functions(uv);
        let mut xu = [0.0; 3];
        let mut xv = [0.0; 3];
        for (xa, da) in coords.iter().zip(&d) {
            for i in 0..3 {
                xu[i] += xa[i] * da[0];
                xv[i] += xa[i] * da[1];
            }
        }
        area += w * norm(cross(xu, xv));
    }
    Ok(area)
}

fn set_loads<T>(mesh: &SolidMesh, set: &str, t: T) -> Result<Vec<NodalForceAt>>
where
    T: Fn(Vec3, Vec3) -> Vec3,
{
    let faces = mesh.surface_set(set).ok_or_else(|| FemError::UnknownSet {
        kind: SetKind::Surface,
        name: set.to_string(),
    })?;
    let mut acc: BTreeMap<usize, Vec3> = BTreeMap::new();
    for &face in faces {
        let kind = mesh.element_kind(face.element);
        if kind != ElementKind::Hex20 {
            return Err(FemError::UnsupportedKind {
                block: mesh.block_of(face.element),
                kind: kind.name(),
                operation: "the Hex20 face loads",
            });
        }
        let nodes = mesh.face_nodes(face);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let fe = face_load(&coords, &t)?;
        for (&node, fa) in nodes.iter().zip(&fe) {
            let e = acc.entry(node).or_insert([0.0; 3]);
            for i in 0..3 {
                e[i] += fa[i];
            }
        }
    }
    Ok(acc.into_iter().map(|(n, f)| NodalForceAt::new(n, f)).collect())
}

fn finite(key: &'static str, v: &[f64]) -> Result<()> {
    if v.iter().all(|c| c.is_finite()) {
        Ok(())
    } else {
        Err(FemError::InvalidValue {
            key,
            reason: format!("{v:?} is not finite"),
        })
    }
}

/// The consistent nodal forces of a uniform pressure `p` (positive pressing on the solid, `t = -p n`) on the Hex20 faces of
/// a surface set, one entry per node of the faces, sorted by node. A face of another kind is refused naming its block.
pub fn pressure_loads(mesh: &SolidMesh, set: &str, p: f64) -> Result<Vec<NodalForceAt>> {
    finite("pressure", &[p])?;
    set_loads(mesh, set, |_, a| [-p * a[0], -p * a[1], -p * a[2]])
}

/// The consistent nodal forces of a uniform traction `t` (force per area) on the Hex20 faces of a surface set.
pub fn traction_loads(mesh: &SolidMesh, set: &str, t: Vec3) -> Result<Vec<NodalForceAt>> {
    finite("traction", &t)?;
    set_loads(mesh, set, |_, a| {
        let s = norm(a);
        [t[0] * s, t[1] * s, t[2] * s]
    })
}

/// The consistent nodal forces of a traction given by a function of position on the Hex20 faces of a surface set.
pub fn traction_field_loads<F>(mesh: &SolidMesh, set: &str, t: F) -> Result<Vec<NodalForceAt>>
where
    F: Fn(Vec3) -> Vec3,
{
    set_loads(mesh, set, |x, a| {
        let s = norm(a);
        let v = t(x);
        [v[0] * s, v[1] * s, v[2] * s]
    })
}

// ---------------------------------------------------------------------------------------------------------------- upgrade

/// Upgrades a mesh of Hex8 to Hex20 with straight edges: [`upgrade_to_hex20_mapped`] with the identity map.
pub fn upgrade_to_hex20(mesh: &SolidMesh) -> Result<(SolidMesh, Vec<MidEdgeNode>)> {
    upgrade_to_hex20_mapped(mesh, |x| x)
}

/// Upgrades a mesh of Hex8 to Hex20 (ADR 0292 D6): one new node per edge, numbered after the existing nodes in the order of
/// first appearance (element order, then local edge order). The input positions are parameters: every output node is
/// `map(p)`, the corners `map(p_a)` and the mid-edge node of an edge `map(0.5 (p_a + p_b))`, so that a curved map puts the
/// mid-edge nodes on the curved edges. Elements keep their numbers, element and surface sets their members; a node set
/// gains the mid-edge node of each edge whose two ends it contains. A block of another kind and a non-finite mapped node
/// are refused.
pub fn upgrade_to_hex20_mapped<F>(mesh: &SolidMesh, map: F) -> Result<(SolidMesh, Vec<MidEdgeNode>)>
where
    F: Fn(Vec3) -> Vec3,
{
    for (block, b) in mesh.blocks().iter().enumerate() {
        if b.kind() != ElementKind::Hex8 {
            return Err(FemError::UnsupportedKind {
                block,
                kind: b.kind().name(),
                operation: "upgrade_to_hex20",
            });
        }
    }
    let params = mesh.nodes();
    let mut nodes: Vec<Vec3> = Vec::with_capacity(params.len());
    for &p in params {
        nodes.push(map(p));
    }
    let mut edge_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut mids = Vec::new();
    let mut blocks = Vec::new();
    for b in mesh.blocks() {
        let mut conn20 = Vec::with_capacity(b.len() / 8 * 20);
        for h in b.connectivity().chunks(8) {
            conn20.extend_from_slice(h);
            for [la, lb] in ElementKind::Hex8.edges() {
                let (a, c) = (h[*la], h[*lb]);
                let key = (a.min(c), a.max(c));
                let node = *edge_index.entry(key).or_insert_with(|| {
                    let (pa, pc) = (params[key.0], params[key.1]);
                    nodes.push(map([0.5 * (pa[0] + pc[0]), 0.5 * (pa[1] + pc[1]), 0.5 * (pa[2] + pc[2])]));
                    mids.push(MidEdgeNode {
                        node: nodes.len() - 1,
                        ends: [key.0, key.1],
                    });
                    nodes.len() - 1
                });
                conn20.push(node);
            }
        }
        blocks.push(conn20);
    }
    for (i, p) in nodes.iter().enumerate() {
        if p.iter().any(|c| !c.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "mapped node",
                reason: format!("node {i} maps to {p:?}, which is not finite"),
            });
        }
    }
    let mut out = SolidMesh::new(nodes)?;
    for conn20 in blocks {
        out.add_block(ElementKind::Hex20, conn20)?;
    }
    for name in mesh.node_set_names() {
        let set = mesh.node_set(name).expect("listed");
        let members: BTreeSet<usize> = set.iter().copied().collect();
        let mut v = set.to_vec();
        v.extend(
            mids.iter()
                .filter(|m| members.contains(&m.ends[0]) && members.contains(&m.ends[1]))
                .map(|m| m.node),
        );
        out.add_node_set(name, v)?;
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).expect("listed").to_vec())?;
    }
    Ok((out, mids))
}
