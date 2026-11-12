//! Nodal, surface and body loads (nodal forces: ADR 0275; the others: ADR 0276). Declared by ADR 0271; filled by ADR 0275
//! and ADR 0276.
//!
//! [`nodal_load_vector`] is the one function the static solve calls for the right-hand side. It adds, in the model's order,
//! the nodal forces ([`Load::NodalForce`]), the consistent nodal loads `f_a = int N_a t dS` of a uniform traction and of a
//! uniform pressure on a surface set (`t = -p n`, positive pressing on the solid, `n` the outward normal), and the
//! consistent nodal loads `f_a = int N_a b dV` of gravity (`b = rho g`), of the centrifugal load of a rotation at constant
//! speed (`b = rho omega^2 r_perp`) and of a body force field given at the nodes (interpolated with the element's own
//! functions), then the per-node forces ([`NodalForceAt`]). The thermal load ([`Load::Temperature`]) is refused naming the
//! package that adds it (ADR 0281), never ignored.
//!
//! Face integrals use the face functions of Tri3, Quad4 and Tri6 ([`face_shape_functions`]) in the local face orders of
//! ADR 0271 and the face rules of [`face_rule`]; body integrals use the element's mass rule (ADR 0276 arithmetic 2: exact for
//! these loads on Tet4, on trilinear Hex8 and on straight-sided Tet10). [`body_force_loads`] gives the consistent load of a
//! body force given by a function of position, as per-node forces for `solve_static_with` (ADR 0270 section 2.4). Every sum
//! is sequential in a fixed order (loads, then faces or elements, then nodes): the vector does not depend on threads.

use crate::element::{element_for, Element, IntegrationPoint};
use crate::error::{FemError, Result};
use crate::mesh::{FaceRef, SolidMesh};
use crate::model::{DofMap, Load, SolidModel};
use crate::quadrature::QuadratureRule;
use rcfd_core::vec3::{cross, dot, norm, sub, Vec3};

/// A force at one node, N.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodalForceAt {
    /// Node index.
    pub node: usize,
    /// The force.
    pub force: Vec3,
}

impl NodalForceAt {
    /// A force at a node.
    pub fn new(node: usize, force: Vec3) -> NodalForceAt {
        NodalForceAt { node, force }
    }
}

fn add_force(f: &mut [f64], dofs: &DofMap, node: usize, force: &Vec3) -> Result<()> {
    if node >= dofs.n_nodes() {
        return Err(FemError::MissingNode {
            role: "nodal force",
            node,
            n_nodes: dofs.n_nodes(),
        });
    }
    if force.iter().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "force",
            reason: format!("{force:?} at node {node} is not finite"),
        });
    }
    for (k, &fk) in force.iter().enumerate() {
        let eq = dofs.equation(node, k).ok_or(FemError::MissingComponent {
            role: "nodal force",
            node,
            component: k,
            available: dofs.components(node),
        })?;
        f[eq] += fk;
    }
    Ok(())
}

/// The global load vector of the model's loads and of `extra` per-node forces, one entry per unknown. Refuses a node that
/// does not exist (naming it), a non-finite force, a body load on an element without material, and the thermal load
/// (ADR 0281).
pub fn nodal_load_vector(model: &SolidModel, extra: &[NodalForceAt]) -> Result<Vec<f64>> {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let mut f = vec![0.0; dofs.n_equations()];
    let mut density: Option<Vec<f64>> = None;
    for load in model.loads() {
        match load {
            Load::NodalForce { set, force } => {
                let nodes = mesh.node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Node,
                    name: set.clone(),
                })?;
                for &node in nodes {
                    add_force(&mut f, &dofs, node, force)?;
                }
            }
            Load::Traction { set, traction } => {
                for &face in surface(mesh, set)? {
                    add_face_load(&mut f, &dofs, mesh, face, SurfaceLoad::Traction(*traction))?;
                }
            }
            Load::Pressure { set, pressure } => {
                for &face in surface(mesh, set)? {
                    add_face_load(&mut f, &dofs, mesh, face, SurfaceLoad::Pressure(*pressure))?;
                }
            }
            Load::Gravity { acceleration } => {
                let rho = densities(model, &mut density)?;
                let g = *acceleration;
                add_body_load(&mut f, &dofs, mesh, |e, _, _| {
                    Ok([rho[e] * g[0], rho[e] * g[1], rho[e] * g[2]])
                })?;
            }
            Load::Centrifugal { axis, origin, omega } => {
                let rho = densities(model, &mut density)?;
                let (axis, origin, w2) = (*axis, *origin, omega * omega);
                add_body_load(&mut f, &dofs, mesh, |e, ip, _| {
                    let r = perpendicular(ip.geometry.x, axis, origin);
                    let s = rho[e] * w2;
                    Ok([s * r[0], s * r[1], s * r[2]])
                })?;
            }
            Load::BodyForceNodal { values } => {
                add_body_load(&mut f, &dofs, mesh, |_, ip, nodes| {
                    let mut b = [0.0; 3];
                    for (nb, &node) in ip.geometry.n.iter().zip(nodes) {
                        for i in 0..3 {
                            b[i] += nb * values[node][i];
                        }
                    }
                    Ok(b)
                })?;
            }
            other => return Err(unsupported(other)),
        }
    }
    for p in extra {
        add_force(&mut f, &dofs, p.node, &p.force)?;
    }
    Ok(f)
}

/// The consistent nodal load `f_a = int N_a b(x) dV` of a body force per unit volume given by a function of position,
/// N/m^3, with each element's mass rule; one entry per node, in node order (to be passed to `solve_static_with`). A
/// non-finite value of `b` is refused naming the element.
pub fn body_force_loads<F>(model: &SolidModel, b: F) -> Result<Vec<NodalForceAt>>
where
    F: Fn(Vec3) -> Vec3,
{
    let mesh = model.mesh();
    let dofs = DofMap::uniform(mesh.nodes().len(), 3);
    let mut f = vec![0.0; dofs.n_equations()];
    add_body_load(&mut f, &dofs, mesh, |_, ip, _| {
        let x = ip.geometry.x;
        let v = b(x);
        if v.iter().all(|c| c.is_finite()) {
            Ok(v)
        } else {
            Err(FemError::InvalidValue {
                key: "body force",
                reason: format!("{v:?} at {x:?} is not finite"),
            })
        }
    })?;
    Ok(f.chunks(3)
        .enumerate()
        .map(|(node, c)| NodalForceAt::new(node, [c[0], c[1], c[2]]))
        .collect())
}

fn surface<'a>(mesh: &'a SolidMesh, set: &str) -> Result<&'a [FaceRef]> {
    mesh.surface_set(set).ok_or_else(|| FemError::UnknownSet {
        kind: crate::error::SetKind::Surface,
        name: set.to_string(),
    })
}

fn densities<'a>(model: &SolidModel, cache: &'a mut Option<Vec<f64>>) -> Result<&'a [f64]> {
    if cache.is_none() {
        let idx = model.material_indices()?;
        *cache = Some(idx.iter().map(|&k| model.materials()[k].1.density()).collect());
    }
    Ok(cache.as_deref().expect("filled above"))
}

/// The component of `x - origin` perpendicular to the axis (any non-zero length).
fn perpendicular(x: Vec3, axis: Vec3, origin: Vec3) -> Vec3 {
    let len = norm(axis);
    let a = [axis[0] / len, axis[1] / len, axis[2] / len];
    let d = sub(x, origin);
    let s = dot(d, a);
    [d[0] - s * a[0], d[1] - s * a[1], d[2] - s * a[2]]
}

#[derive(Clone, Copy)]
enum SurfaceLoad {
    Traction(Vec3),
    Pressure(f64),
}

fn add_face_load(f: &mut [f64], dofs: &DofMap, mesh: &SolidMesh, face: FaceRef, load: SurfaceLoad) -> Result<()> {
    let nodes = mesh.face_nodes(face);
    let x: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let mut fe = vec![[0.0; 3]; nodes.len()];
    for (uv, w) in face_rule(nodes.len())? {
        let (n, d) = face_shape_functions(nodes.len(), uv)?;
        let mut xu = [0.0; 3];
        let mut xv = [0.0; 3];
        for (xa, da) in x.iter().zip(&d) {
            for i in 0..3 {
                xu[i] += xa[i] * da[0];
                xv[i] += xa[i] * da[1];
            }
        }
        let c = cross(xu, xv);
        let t = match load {
            SurfaceLoad::Traction(t) => {
                let s = norm(c);
                [t[0] * s, t[1] * s, t[2] * s]
            }
            SurfaceLoad::Pressure(p) => [-p * c[0], -p * c[1], -p * c[2]],
        };
        for (fa, na) in fe.iter_mut().zip(&n) {
            for i in 0..3 {
                fa[i] += w * na * t[i];
            }
        }
    }
    for (&node, fa) in nodes.iter().zip(&fe) {
        add_force(f, dofs, node, fa)?;
    }
    Ok(())
}

/// The rule of the body loads: the element's mass rule (ADR 0276 arithmetic 2).
fn body_rule(element: &dyn Element) -> &'static QuadratureRule {
    element.mass_rule()
}

fn add_body_load<B>(f: &mut [f64], dofs: &DofMap, mesh: &SolidMesh, b: B) -> Result<()>
where
    B: Fn(usize, &IntegrationPoint, &[usize]) -> Result<Vec3>,
{
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let fe = element_body_load(mesh, e, &b).map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
        for (&node, fa) in nodes.iter().zip(&fe) {
            add_force(f, dofs, node, fa)?;
        }
    }
    Ok(())
}

fn element_body_load<B>(mesh: &SolidMesh, e: usize, b: &B) -> Result<Vec<Vec3>>
where
    B: Fn(usize, &IntegrationPoint, &[usize]) -> Result<Vec3>,
{
    let element = element_for(mesh.element_kind(e));
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let mut fe = vec![[0.0; 3]; nodes.len()];
    for ip in element.integration_points(&coords, body_rule(element))? {
        let bq = b(e, &ip, nodes)?;
        for (fa, na) in fe.iter_mut().zip(&ip.geometry.n) {
            for i in 0..3 {
                fa[i] += ip.weight * na * bq[i];
            }
        }
    }
    Ok(fe)
}

/// Values and derivatives `(d/du, d/dv)` of the functions of a face with `n_nodes` nodes in the local face order of ADR
/// 0271: Tri3 `(1 - u - v, u, v)` on the reference triangle; Quad4 `(1 + u u_a)(1 + v v_a)/4` on `[-1, 1]^2` with nodes
/// `(-,-), (+,-), (+,+), (-,+)`; Tri6 `L_i (2 L_i - 1)` at the corners and `4 L_i L_j` at the mid-edge nodes of the edges
/// (c0 c1), (c1 c2), (c2 c0), with `(L0, L1, L2) = (1 - u - v, u, v)`. Another node count is refused.
pub fn face_shape_functions(n_nodes: usize, uv: [f64; 2]) -> Result<(Vec<f64>, Vec<[f64; 2]>)> {
    let [u, v] = uv;
    match n_nodes {
        3 => Ok((vec![1.0 - u - v, u, v], vec![[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]])),
        4 => {
            let r = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
            let n = r.iter().map(|a| (1.0 + u * a[0]) * (1.0 + v * a[1]) / 4.0).collect();
            let d = r
                .iter()
                .map(|a| [a[0] * (1.0 + v * a[1]) / 4.0, (1.0 + u * a[0]) * a[1] / 4.0])
                .collect();
            Ok((n, d))
        }
        6 => {
            let l = [1.0 - u - v, u, v];
            let dl = [[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]];
            let mut n = Vec::with_capacity(6);
            let mut d = Vec::with_capacity(6);
            for i in 0..3 {
                n.push(l[i] * (2.0 * l[i] - 1.0));
                let s = 4.0 * l[i] - 1.0;
                d.push([s * dl[i][0], s * dl[i][1]]);
            }
            for [i, j] in [[0usize, 1usize], [1, 2], [2, 0]] {
                n.push(4.0 * l[i] * l[j]);
                d.push([
                    4.0 * (l[i] * dl[j][0] + l[j] * dl[i][0]),
                    4.0 * (l[i] * dl[j][1] + l[j] * dl[i][1]),
                ]);
            }
            Ok((n, d))
        }
        n => Err(face_count_error(n)),
    }
}

fn face_count_error(n: usize) -> FemError {
    FemError::InvalidValue {
        key: "face nodes",
        reason: format!("a face of {n} nodes (faces of 3, 4 or 6 nodes are supported)"),
    }
}

// Three-point Gauss-Legendre rule on [-1, 1]: the roots 0, +-sqrt(3/5) of P3, weights 8/9 and 5/9 (ADR 0276 arithmetic 2).
const GAUSS3_X: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
const GAUSS3_W: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];

/// The quadrature rule of a face with `n_nodes` nodes, as (point, weight): for the quadrilateral the 3 x 3 Gauss rule on
/// `[-1, 1]^2` (exact to degree 5 per variable); for the triangles the collapsed product rule `u = s (1 - r)`, `v = s r`
/// with 3-point Gauss in s and r on `[0, 1]` and weight `w_s w_r s` (exact to total degree 4; ADR 0276 arithmetic 2).
pub fn face_rule(n_nodes: usize) -> Result<Vec<([f64; 2], f64)>> {
    let mut out = Vec::with_capacity(9);
    match n_nodes {
        4 => {
            for j in 0..3 {
                for i in 0..3 {
                    out.push(([GAUSS3_X[i], GAUSS3_X[j]], GAUSS3_W[i] * GAUSS3_W[j]));
                }
            }
        }
        3 | 6 => {
            for j in 0..3 {
                let (r, wr) = ((1.0 + GAUSS3_X[j]) / 2.0, GAUSS3_W[j] / 2.0);
                for i in 0..3 {
                    let (s, ws) = ((1.0 + GAUSS3_X[i]) / 2.0, GAUSS3_W[i] / 2.0);
                    out.push(([s * (1.0 - r), s * r], ws * wr * s));
                }
            }
        }
        n => return Err(face_count_error(n)),
    }
    Ok(out)
}

fn unsupported(load: &Load) -> FemError {
    let (load, package) = match load {
        Load::NodalForce { .. } => ("NodalForce", "ADR 0275"),
        Load::Traction { .. } => ("Traction", "ADR 0276"),
        Load::Pressure { .. } => ("Pressure", "ADR 0276"),
        Load::Gravity { .. } => ("Gravity", "ADR 0276"),
        Load::Centrifugal { .. } => ("Centrifugal", "ADR 0276"),
        Load::BodyForceNodal { .. } => ("BodyForceNodal", "ADR 0276"),
        Load::Temperature { .. } => ("Temperature", "ADR 0281"),
    };
    FemError::UnsupportedLoad { load, package }
}
