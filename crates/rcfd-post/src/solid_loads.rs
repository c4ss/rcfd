//! Loads of the F-237 deck applied to the solid surface (one-way coupling 1). Declared by ADR 0278; filled by ADR 0436 (the
//! registration of the planned ADR 0285).
//!
//! * [`shell_from_surface_set`] turns a surface set of a [`SolidMesh`] into the [`ShellSurface`] that the F-237 export
//!   ([`crate::structural`]) writes onto: the same nodes, one quad per 4-node face in the local face order (the outward normal
//!   of the solid, which points toward the fluid), and the parameter rectangles of the plane or z-axis cylinder parametrisation.
//! * [`deck_pressure_loads`] applies the `PLOAD4` cards of a deck as the consistent nodal loads of a uniform pressure on the
//!   matched solid faces (`f_a = -p int N_a (x_u x x_v)`, positive pressing on the solid), for
//!   `rcfd_fem::solve::solve_static_with`. On planar bilinear faces their resultant force and moment equal
//!   [`crate::structural::deck_force_moment`] to rounding (ADR 0436 D3).
//! * [`deck_temperature_load`] maps the `TEMP` cards of a deck to one value per solid node, `Load::temperature(nodal,
//!   reference)` (ADR 0281); the expansion coefficient is the material's.
//!
//! Deck grids are matched to solid nodes by coordinates within an absolute tolerance; a grid that matches no node (or two), or
//! an element that matches no face of the set, is refused naming its id.

use crate::structural::{ParamRect, ShellSurface};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::loads::{face_rule, face_shape_functions, NodalForceAt};
use rcfd_fem::{FaceRef, Load, SolidMesh};
use rcfd_io::nastran::{Deck, ElemKind};
use std::collections::{BTreeMap, HashMap};

/// The parametrisation of a surface for the F-237 overlap weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceParam {
    /// A plane, parametrised by the coordinates `axes[0]`, `axes[1]` (for example `[0, 1]` for `(x, y)`).
    Plane {
        /// The two coordinate indices.
        axes: [usize; 2],
    },
    /// A cylinder about the z axis, parametrised by `(theta, z)`.
    CylinderZ,
}

/// A shell surface built from a surface set of a solid mesh.
#[derive(Debug, Clone)]
pub struct SolidShell {
    /// The shell surface (nodes, quads with the normal toward the fluid, parameter rectangles).
    pub surface: ShellSurface,
    /// The solid node of each shell node.
    pub solid_nodes: Vec<usize>,
}

/// The shell surface of the surface set `set` (ADR 0436 D1): its nodes are the set's nodes (sorted by solid index, at the
/// solid's coordinates), one quad per face in the set's order with the face's corners in the local face order (outward normal
/// of the solid = toward the fluid). Refuses an unknown or empty set, a face that is not a 4-node face (naming the element and
/// the local face) and a face of zero parameter area.
pub fn shell_from_surface_set(mesh: &SolidMesh, set: &str, param: SurfaceParam) -> Result<SolidShell, String> {
    let faces = mesh
        .surface_set(set)
        .ok_or_else(|| format!("shell_from_surface_set: unknown surface set '{set}'"))?;
    if faces.is_empty() {
        return Err(format!("shell_from_surface_set: surface set '{set}' is empty"));
    }
    if let SurfaceParam::Plane { axes } = param {
        if axes[0] > 2 || axes[1] > 2 || axes[0] == axes[1] {
            return Err(format!(
                "shell_from_surface_set: plane axes {axes:?} must be two different indices in 0..3"
            ));
        }
    }
    let mut corner_lists = Vec::with_capacity(faces.len());
    for f in faces {
        let nodes = mesh.face_nodes(*f);
        if nodes.len() != 4 {
            return Err(format!(
                "shell_from_surface_set: face {} of element {} in set '{set}' has {} nodes; only 4-node (bilinear) faces \
                 map to CQUAD4 shells",
                f.face,
                f.element,
                nodes.len()
            ));
        }
        corner_lists.push([nodes[0], nodes[1], nodes[2], nodes[3]]);
    }
    let mut solid_nodes: Vec<usize> = corner_lists.iter().flatten().copied().collect();
    solid_nodes.sort_unstable();
    solid_nodes.dedup();
    let local: HashMap<usize, usize> = solid_nodes.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let nodes: Vec<Vec3> = solid_nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let mut quads = Vec::with_capacity(corner_lists.len());
    let mut rects = Vec::with_capacity(corner_lists.len());
    for (k, c) in corner_lists.iter().enumerate() {
        quads.push([local[&c[0]], local[&c[1]], local[&c[2]], local[&c[3]]]);
        let x: Vec<Vec3> = c.iter().map(|&n| mesh.nodes()[n]).collect();
        let r = param_rect(&x, param);
        let area = (r.u[1] - r.u[0]) * (r.v[1] - r.v[0]);
        if !(area > 0.0 && area.is_finite()) {
            return Err(format!(
                "shell_from_surface_set: face {} of element {} in set '{set}' has zero or invalid parameter area",
                faces[k].face, faces[k].element
            ));
        }
        rects.push(r);
    }
    Ok(SolidShell {
        surface: ShellSurface { nodes, quads, rects },
        solid_nodes,
    })
}

fn param_rect(x: &[Vec3], param: SurfaceParam) -> ParamRect {
    let mut r = ParamRect {
        u: [f64::INFINITY, f64::NEG_INFINITY],
        v: [f64::INFINITY, f64::NEG_INFINITY],
    };
    match param {
        SurfaceParam::Plane { axes } => {
            for p in x {
                r.u = [r.u[0].min(p[axes[0]]), r.u[1].max(p[axes[0]])];
                r.v = [r.v[0].min(p[axes[1]]), r.v[1].max(p[axes[1]])];
            }
        }
        SurfaceParam::CylinderZ => {
            // As patch_rects_cylinder: the angle of each corner relative to the angle of the face centre.
            let two_pi = 2.0 * std::f64::consts::PI;
            let n = x.len() as f64;
            let c = x.iter().fold([0.0; 3], |s, p| vec3::add(s, *p));
            let tc = (c[1] / n).atan2(c[0] / n);
            for p in x {
                let mut d = p[1].atan2(p[0]) - tc;
                while d > std::f64::consts::PI {
                    d -= two_pi;
                }
                while d < -std::f64::consts::PI {
                    d += two_pi;
                }
                r.u = [r.u[0].min(tc + d), r.u[1].max(tc + d)];
                r.v = [r.v[0].min(p[2]), r.v[1].max(p[2])];
            }
        }
    }
    r
}

/// Candidate solid nodes sorted by x, for the match by coordinates within a tolerance.
struct NodeMatch<'a> {
    mesh: &'a SolidMesh,
    by_x: Vec<(f64, usize)>,
    tol: f64,
}

impl<'a> NodeMatch<'a> {
    fn new(mesh: &'a SolidMesh, candidates: &[usize], tol: f64) -> Result<NodeMatch<'a>, String> {
        if !(tol > 0.0 && tol.is_finite()) {
            return Err(format!("the match tolerance {tol} must be positive and finite"));
        }
        let mut by_x: Vec<(f64, usize)> = candidates.iter().map(|&n| (mesh.nodes()[n][0], n)).collect();
        by_x.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Ok(NodeMatch { mesh, by_x, tol })
    }

    /// The one candidate within `tol` of `x`; refuses none or two, naming the deck grid.
    fn find(&self, grid: i64, x: Vec3, what: &str) -> Result<usize, String> {
        let lo = self.by_x.partition_point(|e| e.0 < x[0] - self.tol);
        let mut found: Option<usize> = None;
        for &(xs, n) in &self.by_x[lo..] {
            if xs > x[0] + self.tol {
                break;
            }
            if vec3::norm(vec3::sub(self.mesh.nodes()[n], x)) <= self.tol {
                if let Some(m) = found {
                    return Err(format!(
                        "deck grid {grid} at {x:?} matches two solid nodes ({m} and {n}) within {:e} m",
                        self.tol
                    ));
                }
                found = Some(n);
            }
        }
        found.ok_or_else(|| {
            format!(
                "deck grid {grid} at {x:?} matches no solid node {what} within {:e} m",
                self.tol
            )
        })
    }
}

/// The consistent nodal loads of the `PLOAD4` cards of load set `sid` on the surface set `set` (ADR 0436 D3), one entry per
/// loaded node in node order, for `solve_static_with`. Every loaded element must be a `CQUAD4` whose grids match nodes of the
/// set within `tol` (m) and are the corners of one face of the set, with a uniform pressure (`P2..P4` blank or equal to `P1`)
/// and a normal pointing out of the solid; otherwise the element or grid is refused by its id.
pub fn deck_pressure_loads(
    mesh: &SolidMesh,
    set: &str,
    deck: &Deck,
    sid: i64,
    tol: f64,
) -> Result<Vec<NodalForceAt>, String> {
    let faces = mesh
        .surface_set(set)
        .ok_or_else(|| format!("deck_pressure_loads: unknown surface set '{set}'"))?;
    let set_nodes = mesh.surface_set_nodes(set).map_err(|e| e.to_string())?;
    let matcher = NodeMatch::new(mesh, &set_nodes, tol)?;
    let mut face_of: BTreeMap<Vec<usize>, FaceRef> = BTreeMap::new();
    for f in faces {
        let mut key = mesh.face_nodes(*f);
        key.sort_unstable();
        face_of.insert(key, *f);
    }
    let grids: HashMap<i64, Vec3> = deck.grids.iter().cloned().collect();
    let elements: HashMap<i64, _> = deck.elements.iter().map(|e| (e.id, e)).collect();
    let mut matched: HashMap<i64, usize> = HashMap::new();
    let mut sum = vec![[0.0; 3]; mesh.nodes().len()];
    let mut touched = vec![false; mesh.nodes().len()];
    for l in deck.pload4.iter().filter(|l| l.sid == sid) {
        let e = elements.get(&l.eid).ok_or_else(|| {
            format!(
                "PLOAD4 of set {sid} on deck element {}, which the deck does not define",
                l.eid
            )
        })?;
        if e.kind != ElemKind::Quad4 {
            return Err(format!(
                "deck element {} is not a CQUAD4 (only quad shells map to solid faces)",
                e.id
            ));
        }
        let mut xs = Vec::with_capacity(4);
        let mut nodes = Vec::with_capacity(4);
        for g in &e.nodes {
            let x = *grids.get(g).ok_or_else(|| {
                format!(
                    "deck element {} refers to deck grid {g}, which the deck does not define",
                    e.id
                )
            })?;
            let n = match matched.get(g) {
                Some(&n) => n,
                None => {
                    let n = matcher.find(*g, x, &format!("of surface set '{set}'"))?;
                    matched.insert(*g, n);
                    n
                }
            };
            xs.push(x);
            nodes.push(n);
        }
        let mut key = nodes.clone();
        key.sort_unstable();
        let face = *face_of.get(&key).ok_or_else(|| {
            format!("deck element {}: its grids match solid nodes {nodes:?}, which are not the corners of a face of surface set '{set}'", e.id)
        })?;
        let given: Vec<f64> = l.p.iter().copied().filter(|v| !v.is_nan()).collect();
        let p = l.p[0];
        if given.iter().any(|&v| v != p) {
            return Err(format!(
                "deck element {}: corner-varying PLOAD4 pressures {:?} are not applied (uniform pressures only)",
                e.id, l.p
            ));
        }
        let a_deck = vec3::scale(vec3::cross(vec3::sub(xs[2], xs[0]), vec3::sub(xs[3], xs[1])), 0.5);
        if vec3::dot(a_deck, mesh.face_area_vector(face)) <= 0.0 {
            return Err(format!(
                "deck element {}: its normal points into the solid (face {} of element {}); the card would pull on the solid",
                e.id, face.face, face.element
            ));
        }
        let fe = face_pressure_load(mesh, face, p).map_err(|m| format!("deck element {}: {m}", e.id))?;
        for (n, fa) in mesh.face_nodes(face).into_iter().zip(fe) {
            sum[n] = vec3::add(sum[n], fa);
            touched[n] = true;
        }
    }
    Ok((0..sum.len())
        .filter(|&n| touched[n])
        .map(|n| NodalForceAt::new(n, sum[n]))
        .collect())
}

/// The consistent nodal loads `f_a = -p int N_a (x_u x x_v) du dv` of a uniform pressure on a face (the arithmetic of
/// `rcfd-fem`'s `Load::pressure`), in the face's node order.
fn face_pressure_load(mesh: &SolidMesh, face: FaceRef, p: f64) -> Result<Vec<Vec3>, String> {
    let nodes = mesh.face_nodes(face);
    let x: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let mut fe = vec![[0.0; 3]; nodes.len()];
    for (uv, w) in face_rule(nodes.len()).map_err(|e| e.to_string())? {
        let (n, d) = face_shape_functions(nodes.len(), uv).map_err(|e| e.to_string())?;
        let mut xu = [0.0; 3];
        let mut xv = [0.0; 3];
        for (xa, da) in x.iter().zip(&d) {
            for i in 0..3 {
                xu[i] += xa[i] * da[0];
                xv[i] += xa[i] * da[1];
            }
        }
        let c = vec3::cross(xu, xv);
        let t = [-p * c[0], -p * c[1], -p * c[2]];
        for (fa, na) in fe.iter_mut().zip(&n) {
            for i in 0..3 {
                fa[i] += w * na * t[i];
            }
        }
    }
    Ok(fe)
}

/// Resultant force and moment about `about` of per-node forces: `sum f`, `sum (x - about) x f`.
pub fn nodal_force_moment(mesh: &SolidMesh, forces: &[NodalForceAt], about: Vec3) -> (Vec3, Vec3) {
    let (mut f, mut m) = ([0.0; 3], [0.0; 3]);
    for p in forces {
        f = vec3::add(f, p.force);
        m = vec3::add(m, vec3::cross(vec3::sub(mesh.nodes()[p.node], about), p.force));
    }
    (f, m)
}

/// The `TEMP` cards of load set `sid` as the temperature load of the solid (ADR 0436 D4): each value goes to the solid node its
/// deck grid matches within `tol` (m); every solid node must receive exactly one value. Refuses a `TEMP` grid the deck does not
/// define or that matches no (or two) solid nodes, naming the grid; a solid node reached twice, naming the second grid; a solid
/// node not reached, naming the node. Returns `Load::temperature(nodal, reference)`.
pub fn deck_temperature_load(
    mesh: &SolidMesh,
    deck: &Deck,
    sid: i64,
    reference: f64,
    tol: f64,
) -> Result<Load, String> {
    let n = mesh.nodes().len();
    let all: Vec<usize> = (0..n).collect();
    let matcher = NodeMatch::new(mesh, &all, tol)?;
    let grids: HashMap<i64, Vec3> = deck.grids.iter().cloned().collect();
    let mut nodal: Vec<Option<f64>> = vec![None; n];
    for &(s, g, t) in &deck.temps {
        if s != sid {
            continue;
        }
        let x = *grids
            .get(&g)
            .ok_or_else(|| format!("TEMP of set {sid} on deck grid {g}, which the deck does not define"))?;
        let node = matcher.find(g, x, "of the solid")?;
        if nodal[node].is_some() {
            return Err(format!(
                "deck grid {g}: solid node {node} already has a TEMP value of set {sid} (one value per solid node)"
            ));
        }
        nodal[node] = Some(t);
    }
    let mut out = Vec::with_capacity(n);
    for (node, v) in nodal.into_iter().enumerate() {
        out.push(
            v.ok_or_else(|| format!("solid node {node} has no TEMP value of set {sid} (one value per solid node)"))?,
        );
    }
    Ok(Load::temperature(out, reference))
}
