//! Nodal, surface and body loads (nodal forces: ADR 0275; the others: ADR 0276). Declared by ADR 0271; filled by ADR 0275
//! and ADR 0276.
//!
//! [`nodal_load_vector`] is the one function the static solve calls for the right-hand side. It adds, in the model's order,
//! the nodal forces ([`Load::NodalForce`]), the consistent nodal loads `f_a = int N_a t dS` of a uniform traction and of a
//! uniform pressure on a surface set (`t = -p n`, positive pressing on the solid, `n` the outward normal), and the
//! consistent nodal loads `f_a = int N_a b dV` of gravity (`b = rho g`), of the centrifugal load of a rotation at constant
//! speed (`b = rho omega^2 r_perp`) and of a body force field given at the nodes (interpolated with the element's own
//! functions) and the thermal load `int B^T D eps_th dV` of a nodal temperature field ([`Load::Temperature`], ADR 0281,
//! [`crate::thermal`]), then the per-node forces ([`NodalForceAt`]). On Beam2 elements a uniform line load
//! ([`Load::LineLoad`]) and gravity (`q = rho A g`) give the consistent forces and moments of
//! [`crate::beam::line_load_vector`] (ADR 0422), and the centrifugal load the consistent forces and moments of
//! [`crate::beam::centrifugal_load_vector`] (ADR 0473). On Shell4 elements a uniform pressure along the element normals
//! ([`Load::ShellPressure`]) gives the consistent forces and moments of [`crate::shell::pressure_load_vector`] (ADR 0432),
//! and gravity and the centrifugal load give those of [`crate::shell::body_load_vector`] (`t int N_i b dA`, ADR 0435), a
//! nodal body-force field that of the interpolated field and a nodal temperature field the membrane thermal load of
//! [`crate::shell::thermal_load_vector`] (ADR 0445). Shell3 elements take the same loads from [`crate::shell3`] (ADR 0454);
//! [`Load::ShellPressureAngleWeighted`] shares each Shell3 element's pressure force by its angles (ADR 0461).
//!
//! Face integrals use the face functions of Tri3, Quad4, Tri6 and the 8-node face of Hex20 ([`face_shape_functions`]) in
//! the local face orders of ADR 0271 and ADR 0292 and the face rules of [`face_rule`] (the 8-node arms: ADR 0383); body integrals use the element's mass rule (ADR 0276 arithmetic 2: exact for
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

pub(crate) fn add_force(f: &mut [f64], dofs: &DofMap, node: usize, force: &Vec3) -> Result<()> {
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

/// Adds a moment to the rotation components 3 to 5 of a node (ADR 0417 D2).
pub(crate) fn add_moment(f: &mut [f64], dofs: &DofMap, node: usize, moment: &Vec3) -> Result<()> {
    for (k, &mk) in moment.iter().enumerate() {
        let eq = dofs.equation(node, 3 + k).ok_or(FemError::MissingComponent {
            role: "nodal moment",
            node,
            component: 3 + k,
            available: dofs.components(node),
        })?;
        f[eq] += mk;
    }
    Ok(())
}

/// The global load vector of the model's loads and of `extra` per-node forces, one entry per unknown. Refuses a node that
/// does not exist (naming it), a non-finite force, a body or thermal load on an element without material, and a second
/// temperature field (ADR 0281).
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
            Load::NodalMoment { set, moment } => {
                let nodes = mesh.node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Node,
                    name: set.clone(),
                })?;
                for &node in nodes {
                    add_moment(&mut f, &dofs, node, moment)?;
                }
            }
            Load::LineLoad { set, force } => {
                let elements = mesh.element_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Element,
                    name: set.clone(),
                })?;
                for &e in elements {
                    add_line_load(&mut f, &dofs, mesh, e, *force)?;
                }
            }
            Load::ShellPressure { set, pressure } => {
                let elements = mesh.element_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Element,
                    name: set.clone(),
                })?;
                for &e in elements {
                    add_shell_pressure(&mut f, &dofs, mesh, e, *pressure, false)?;
                }
            }
            Load::ShellPressureAngleWeighted { set, pressure } => {
                let elements = mesh.element_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Element,
                    name: set.clone(),
                })?;
                for &e in elements {
                    add_shell_pressure(&mut f, &dofs, mesh, e, *pressure, true)?;
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
                // ADR 0422 D3: a Beam2 element takes the line load rho A g; every other element as before.
                let beam = |e: usize| -> Result<BeamBody> {
                    let (section, _) = crate::beam::beam_property(mesh, e).ok_or_else(|| FemError::InvalidValue {
                        key: "property",
                        reason: format!(
                            "block {} (Beam2) has no Beam property: a beam needs its section and orientation",
                            mesh.block_of(e)
                        ),
                    })?;
                    let ra = rho[e] * section.area();
                    Ok(BeamBody::Uniform([ra * g[0], ra * g[1], ra * g[2]]))
                };
                // ADR 0435 D2: a Shell4 element takes the body force rho g over its thickness.
                let shell = |e: usize, _: &[f64], _: Vec3| -> Result<Vec3> {
                    Ok([rho[e] * g[0], rho[e] * g[1], rho[e] * g[2]])
                };
                add_body_load_with(
                    &mut f,
                    &dofs,
                    mesh,
                    |e, _, _| Ok([rho[e] * g[0], rho[e] * g[1], rho[e] * g[2]]),
                    Some(&beam),
                    Some(&shell),
                )?;
            }
            Load::Centrifugal { axis, origin, omega } => {
                let rho = densities(model, &mut density)?;
                let (axis, origin, w2) = (*axis, *origin, omega * omega);
                // ADR 0473 D5: a Beam2 element takes the consistent load of its linear force per unit length and its section's
                // moment.
                let materials = model.material_indices()?;
                let beam = |e: usize| -> Result<BeamBody> {
                    let (section, v) = crate::beam::beam_property(mesh, e).ok_or_else(|| FemError::InvalidValue {
                        key: "property",
                        reason: format!(
                            "block {} (Beam2) has no Beam property: a beam needs its section and orientation",
                            mesh.block_of(e)
                        ),
                    })?;
                    let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
                    let material = &model.materials()[materials[e]].1;
                    crate::beam::centrifugal_load_vector(&coords, &section, v, material, axis, origin, w2)
                        .map(BeamBody::Vector)
                };
                // ADR 0435 D2: a Shell4 element takes the body force rho omega^2 r_perp over its thickness.
                let shell = |e: usize, _: &[f64], x: Vec3| -> Result<Vec3> {
                    let r = perpendicular(x, axis, origin);
                    let s = rho[e] * w2;
                    Ok([s * r[0], s * r[1], s * r[2]])
                };
                add_body_load_with(
                    &mut f,
                    &dofs,
                    mesh,
                    |e, ip, _| {
                        let r = perpendicular(ip.geometry.x, axis, origin);
                        let s = rho[e] * w2;
                        Ok([s * r[0], s * r[1], s * r[2]])
                    },
                    Some(&beam),
                    Some(&shell),
                )?;
            }
            Load::BodyForceNodal { values } => {
                // ADR 0445 D1: a Shell4 element takes the interpolated field sum_j N_j b_j over its thickness.
                let shell = |e: usize, n: &[f64], _: Vec3| -> Result<Vec3> {
                    let mut b = [0.0; 3];
                    for (nb, &node) in n.iter().zip(mesh.element_nodes(e)) {
                        for i in 0..3 {
                            b[i] += nb * values[node][i];
                        }
                    }
                    Ok(b)
                };
                add_body_load_with(
                    &mut f,
                    &dofs,
                    mesh,
                    |_, ip, nodes| {
                        let mut b = [0.0; 3];
                        for (nb, &node) in ip.geometry.n.iter().zip(nodes) {
                            for i in 0..3 {
                                b[i] += nb * values[node][i];
                            }
                        }
                        Ok(b)
                    },
                    None,
                    Some(&shell),
                )?;
            }
            Load::Temperature { nodal, reference } => {
                crate::thermal::temperature_field(model)?;
                crate::thermal::add_thermal_load(&mut f, &dofs, model, nodal, *reference)?;
            }
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
    add_body_load_with(f, dofs, mesh, b, None, None)
}

/// The uniform line load of a beam element as its consistent nodal forces and moments (ADR 0422 D3).
fn add_line_load(f: &mut [f64], dofs: &DofMap, mesh: &SolidMesh, e: usize, q: Vec3) -> Result<()> {
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let fe = crate::beam::line_load_vector(&coords, q).map_err(|source| FemError::Element {
        element: e,
        source: Box::new(source),
    })?;
    for (a, &node) in nodes.iter().enumerate() {
        add_force(f, dofs, node, &[fe[6 * a], fe[6 * a + 1], fe[6 * a + 2]])?;
        add_moment(f, dofs, node, &[fe[6 * a + 3], fe[6 * a + 4], fe[6 * a + 5]])?;
    }
    Ok(())
}

/// The uniform pressure on a shell element as its consistent nodal forces and moments (ADR 0432 D5, ADR 0454 D2); with
/// `angle_weighted` a Shell3 element takes the angle-weighted split of ADR 0461 instead.
fn add_shell_pressure(
    f: &mut [f64],
    dofs: &DofMap,
    mesh: &SolidMesh,
    e: usize,
    p: f64,
    angle_weighted: bool,
) -> Result<()> {
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let fe: Vec<f64> = if mesh.element_kind(e) == crate::mesh::ElementKind::Shell3 && angle_weighted {
        crate::shell3::pressure_load_vector_angle_weighted(&coords, p).map(|v| v.to_vec())
    } else if mesh.element_kind(e) == crate::mesh::ElementKind::Shell3 {
        crate::shell3::pressure_load_vector(&coords, p).map(|v| v.to_vec())
    } else {
        crate::shell::pressure_load_vector(&coords, p).map(|v| v.to_vec())
    }
    .map_err(|source| FemError::Element {
        element: e,
        source: Box::new(source),
    })?;
    for (a, &node) in nodes.iter().enumerate() {
        add_force(f, dofs, node, &[fe[6 * a], fe[6 * a + 1], fe[6 * a + 2]])?;
        add_moment(f, dofs, node, &[fe[6 * a + 3], fe[6 * a + 4], fe[6 * a + 5]])?;
    }
    Ok(())
}

/// The body load of a Beam2 element: a uniform force per unit length (ADR 0422 D3) or its consistent nodal vector, forces and
/// moments per node in the element's unknown order (the centrifugal load, ADR 0473 D5).
enum BeamBody {
    Uniform(Vec3),
    Vector([f64; 12]),
}

/// The consistent nodal vector of a beam element added node by node, force then moment (as [`add_line_load`]).
fn add_beam_vector(f: &mut [f64], dofs: &DofMap, mesh: &SolidMesh, e: usize, fe: &[f64; 12]) -> Result<()> {
    for (a, &node) in mesh.element_nodes(e).iter().enumerate() {
        add_force(f, dofs, node, &[fe[6 * a], fe[6 * a + 1], fe[6 * a + 2]])?;
        add_moment(f, dofs, node, &[fe[6 * a + 3], fe[6 * a + 4], fe[6 * a + 5]])?;
    }
    Ok(())
}

/// The body force per unit volume of shell element `e` at a point, from the values of the facet's shape functions there and
/// the point (ADR 0435 D2, ADR 0445 D1).
type ShellRule<'a> = dyn Fn(usize, &[f64], Vec3) -> Result<Vec3> + 'a;

/// The body load of a shell element as its consistent nodal forces and moments (ADR 0435 D2).
fn add_shell_body_load(f: &mut [f64], dofs: &DofMap, mesh: &SolidMesh, e: usize, rule: &ShellRule) -> Result<()> {
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let fe: Vec<f64> = crate::shell::thickness_or_error(mesh, e)
        .and_then(|t| {
            if mesh.element_kind(e) == crate::mesh::ElementKind::Shell3 {
                crate::shell3::body_load_vector_at(&coords, t, |n, x| rule(e, n, x)).map(|v| v.to_vec())
            } else {
                crate::shell::body_load_vector_at(&coords, t, |n, x| rule(e, n, x)).map(|v| v.to_vec())
            }
        })
        .map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
    for (a, &node) in nodes.iter().enumerate() {
        add_force(f, dofs, node, &[fe[6 * a], fe[6 * a + 1], fe[6 * a + 2]])?;
        add_moment(f, dofs, node, &[fe[6 * a + 3], fe[6 * a + 4], fe[6 * a + 5]])?;
    }
    Ok(())
}

/// [`add_body_load`] with an optional rule for Beam2 elements: the uniform line load the element carries (ADR 0422 D3),
/// and an optional rule for Shell4 elements: the body force per unit volume of the element at a point (from the values of
/// the facet's shape functions there and the point; ADR 0445 D1), integrated over the
/// thickness by [`crate::shell::body_load_vector`] (ADR 0435 D2). Without them, or for every other element, the element's
/// consistent body load as before.
fn add_body_load_with<B>(
    f: &mut [f64],
    dofs: &DofMap,
    mesh: &SolidMesh,
    b: B,
    beam: Option<&dyn Fn(usize) -> Result<BeamBody>>,
    shell: Option<&ShellRule<'_>>,
) -> Result<()>
where
    B: Fn(usize, &IntegrationPoint, &[usize]) -> Result<Vec3>,
{
    for e in 0..mesh.n_elements() {
        if let Some(rule) = beam {
            if mesh.element_kind(e) == crate::mesh::ElementKind::Beam2 {
                let body = rule(e).map_err(|source| FemError::Element {
                    element: e,
                    source: Box::new(source),
                })?;
                match body {
                    BeamBody::Uniform(q) => add_line_load(f, dofs, mesh, e, q)?,
                    BeamBody::Vector(fe) => add_beam_vector(f, dofs, mesh, e, &fe)?,
                }
                continue;
            }
        }
        if let Some(rule) = shell {
            if matches!(
                mesh.element_kind(e),
                crate::mesh::ElementKind::Shell4 | crate::mesh::ElementKind::Shell3
            ) {
                add_shell_body_load(f, dofs, mesh, e, rule)?;
                continue;
            }
        }
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
/// (c0 c1), (c1 c2), (c2 c0), with `(L0, L1, L2) = (1 - u - v, u, v)`; the 8-node face of Hex20 as
/// [`crate::hex20::face_shape_functions`] (ADR 0383). Another node count is refused.
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
        8 => {
            let (n, d) = crate::hex20::face_shape_functions(uv);
            Ok((n.to_vec(), d.to_vec()))
        }
        n => Err(face_count_error(n)),
    }
}

fn face_count_error(n: usize) -> FemError {
    FemError::InvalidValue {
        key: "face nodes",
        reason: format!("a face of {n} nodes (faces of 3, 4, 6 or 8 nodes are supported)"),
    }
}

// Three-point Gauss-Legendre rule on [-1, 1]: the roots 0, +-sqrt(3/5) of P3, weights 8/9 and 5/9 (ADR 0276 arithmetic 2).
const GAUSS3_X: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
const GAUSS3_W: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];

/// The quadrature rule of a face with `n_nodes` nodes, as (point, weight): for the quadrilateral the 3 x 3 Gauss rule on
/// `[-1, 1]^2` (exact to degree 5 per variable); for the triangles the collapsed product rule `u = s (1 - r)`, `v = s r`
/// with 3-point Gauss in s and r on `[0, 1]` and weight `w_s w_r s` (exact to total degree 4; ADR 0276 arithmetic 2); for
/// the 8-node face of Hex20 [`crate::hex20::face_rule`] (ADR 0383).
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
        8 => out.extend_from_slice(&crate::hex20::face_rule()),
        n => return Err(face_count_error(n)),
    }
    Ok(out)
}
