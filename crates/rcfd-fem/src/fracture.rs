//! Fracture parameters: domain integral, stress intensity factor, growth life. Declared by ADR 0440 (F-408, the scaffold of
//! ADR 0311); filled by ADR 0547 (F-429, the registration of the stub ADR 0332).
//!
//! * [`CrackFront`]: the nodes of a straight or circular front, the crack plane and the advance direction, validated (ADR 0547
//!   D1: a straight front within [`FRONT_TOLERANCE`] of its line, a circular front of its circle).
//! * [`j_integral`]: the energy release rate at each station of the front by the equivalent domain integral over a ring of
//!   elements (D2, D3), refusing a ring that reaches a boundary other than the crack faces, the mirror plane and the end faces
//!   of the front (D4).
//! * [`plane_strain_k`]: `K_I = sqrt(J E / (1 - nu^2))` (D5); [`opening_k`]: `K_I` from the opening behind the front by a
//!   least-squares line through the apparent factors (D6).
//! * [`paris_cycles`]: the cycles of the Paris law between two crack sizes, integrated numerically (D7).
//! * [`centre_crack_plate`] and [`penny_crack_cylinder`]: Hex20 meshes with a slit graded towards the front (D8).
//!
//! * ADR 0605 (F-632): [`CrackFront::elliptical`], an elliptical front in confocal coordinates (D1); [`interaction_integral`],
//!   `K_I`, `K_II` and `K_III` of a straight front by the interaction integral with the near-tip fields of
//!   [`near_tip_gradient`] (D5); [`elliptical_crack_block`], the penny mesh mapped onto an embedded elliptical crack (D3).
//! * ADR 0610 (F-682): [`interaction_integral_curved`], the interaction integral on straight, circular and elliptical fronts
//!   with the auxiliary fields in the frame of each point's foot ([`tip_frame`], [`curved_auxiliary`]) and the correction
//!   terms of their curvature (D1); [`penny_crack_half_cylinder`], the half `x >= 0` of a whole cracked cylinder (D4).
//!
//! Linear elastic, isotropic, homogeneous material in the domain; no body force, no thermal strain; traction-free crack faces.

use crate::element::element_for;
use crate::error::{FemError, Result};
use crate::hex20::upgrade_to_hex20;
use crate::mesh::{ElementKind, FaceRef, SolidFormulation, SolidMesh};
use crate::model::{Load, Material, SolidModel};
use rcfd_core::vec3::{cross, dot, norm, sub, Vec3};
use std::collections::{BTreeSet, HashMap};
use std::f64::consts::PI;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The relative tolerance of the front geometry (ADR 0547 D1): a straight front within `FRONT_TOLERANCE L` of its line (`L`
/// the distance between its end nodes), a circular front within `FRONT_TOLERANCE R` of its circle; also the tolerance of the
/// crack plane in the boundary test (relative to the mesh's bounding-box diagonal).
pub const FRONT_TOLERANCE: f64 = 1e-6;

/// The tolerance of the directions: the advance normal to the front and in the plane, an end face normal to the front.
const PARALLEL_TOLERANCE: f64 = 1e-6;

fn invalid(key: &'static str, reason: impl Into<String>) -> FemError {
    FemError::InvalidValue {
        key,
        reason: reason.into(),
    }
}

fn scaled(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn unit_of(a: Vec3, key: &'static str, what: &str) -> Result<Vec3> {
    let l = norm(a);
    if !(l > 0.0 && l.is_finite()) {
        return Err(invalid(key, format!("{what} {a:?} has no direction")));
    }
    Ok(scaled(a, 1.0 / l))
}

// ----------------------------------------------------------------------------------------------------------------- front

/// The shape of a crack front (ADR 0547 D1).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FrontShape {
    /// A straight front with the crack-advance direction (in the crack plane, normal to the front, into the ligament).
    Straight {
        /// The advance direction (normalised when checked).
        advance: Vec3,
    },
    /// A circle in the crack plane about `centre`; the advance direction is radial, away from the axis.
    Circular {
        /// A point of the axis in the crack plane.
        centre: Vec3,
    },
    /// An ellipse `X^2/a^2 + Y^2/b^2 = 1` in the crack plane (ADR 0605 D1), `X` along `major_axis` from `centre`, `a > b`;
    /// the parameter is the elliptic angle `nu` of the confocal coordinates (the parametric angle on the front) and the
    /// advance direction the outward normal of the confocal ellipse through the point.
    Elliptical {
        /// The centre of the ellipse.
        centre: Vec3,
        /// The direction of the major semi-axis (in the crack plane; normalised when checked).
        major_axis: Vec3,
        /// The major semi-axis `a`.
        a: f64,
        /// The minor semi-axis `b < a`.
        b: f64,
    },
}

/// A crack front: its nodes in order along it (an odd number `>= 3`, corner, mid-edge, corner, ...; the corners are the
/// stations), the normal of the crack plane, the shape, and whether the model is the half on one side of the crack plane.
#[derive(Debug, Clone, PartialEq)]
pub struct CrackFront {
    nodes: Vec<usize>,
    normal: Vec3,
    shape: FrontShape,
    half_symmetric: bool,
}

impl CrackFront {
    /// A straight front with the crack-plane normal and the advance direction.
    pub fn straight(nodes: Vec<usize>, normal: Vec3, advance: Vec3) -> CrackFront {
        CrackFront {
            nodes,
            normal,
            shape: FrontShape::Straight { advance },
            half_symmetric: false,
        }
    }

    /// A circular front about `centre` in the plane of normal `normal`.
    pub fn circular(nodes: Vec<usize>, normal: Vec3, centre: Vec3) -> CrackFront {
        CrackFront {
            nodes,
            normal,
            shape: FrontShape::Circular { centre },
            half_symmetric: false,
        }
    }

    /// An elliptical front with semi-axes `a > b` about `centre`, the major axis along `major_axis`, in the plane of
    /// normal `normal` (ADR 0605 D1).
    pub fn elliptical(nodes: Vec<usize>, normal: Vec3, centre: Vec3, major_axis: Vec3, a: f64, b: f64) -> CrackFront {
        CrackFront {
            nodes,
            normal,
            shape: FrontShape::Elliptical {
                centre,
                major_axis,
                a,
                b,
            },
            half_symmetric: false,
        }
    }

    /// The same front in a model that is the half on the `+normal` side of the crack plane (the other half by mirror
    /// symmetry): J and the opening are doubled.
    pub fn half_symmetric(mut self) -> CrackFront {
        self.half_symmetric = true;
        self
    }

    /// The front nodes in order.
    pub fn nodes(&self) -> &[usize] {
        &self.nodes
    }
    /// The crack-plane normal as given.
    pub fn normal(&self) -> Vec3 {
        self.normal
    }
    /// The shape.
    pub fn shape(&self) -> &FrontShape {
        &self.shape
    }
    /// Whether the model is a half by mirror symmetry.
    pub fn is_half_symmetric(&self) -> bool {
        self.half_symmetric
    }
    /// The number of stations (the corner nodes at the even positions).
    pub fn n_stations(&self) -> usize {
        self.nodes.len().div_ceil(2)
    }

    /// Validates the front against a mesh (ADR 0547 D1).
    fn check(&self, mesh: &SolidMesh) -> Result<FrontGeometry> {
        const KEY: &str = "crack front";
        let nn = mesh.nodes().len();
        if self.nodes.len() < 3 || self.nodes.len() % 2 == 0 {
            return Err(invalid(
                KEY,
                format!(
                    "{} nodes: a front needs an odd number >= 3 (corner, mid-edge, corner, ...)",
                    self.nodes.len()
                ),
            ));
        }
        let mut seen = BTreeSet::new();
        for &n in &self.nodes {
            if n >= nn {
                return Err(invalid(
                    KEY,
                    format!("node {n} does not exist (the mesh has {nn} nodes)"),
                ));
            }
            if !seen.insert(n) {
                return Err(invalid(KEY, format!("node {n} appears twice")));
            }
        }
        let n = unit_of(self.normal, KEY, "the crack-plane normal")?;
        let x: Vec<Vec3> = self.nodes.iter().map(|&i| mesh.nodes()[i]).collect();
        let x0 = x[0];
        let (kind, s, length) = match &self.shape {
            FrontShape::Straight { advance } => {
                let chord = sub(*x.last().expect("3 nodes"), x0);
                let l = norm(chord);
                if l.is_nan() || l <= 0.0 {
                    return Err(invalid(KEY, "the end nodes of the front coincide"));
                }
                let t = scaled(chord, 1.0 / l);
                let mut s = Vec::with_capacity(x.len());
                for (k, &xi) in x.iter().enumerate() {
                    let d = sub(xi, x0);
                    let along = dot(d, t);
                    let off = norm(sub(d, scaled(t, along)));
                    if off > FRONT_TOLERANCE * l {
                        return Err(invalid(
                            KEY,
                            format!(
                                "the front is not straight: node {} at {xi:?} is {off:.3e} from the line of its end nodes \
                                 (tolerance {FRONT_TOLERANCE:e} times the length {l:.6e})",
                                self.nodes[k]
                            ),
                        ));
                    }
                    let h = dot(d, n).abs();
                    if h > FRONT_TOLERANCE * l {
                        return Err(invalid(
                            KEY,
                            format!("node {} is {h:.3e} off the crack plane", self.nodes[k]),
                        ));
                    }
                    s.push(along);
                }
                let e = unit_of(*advance, KEY, "the advance direction")?;
                if dot(e, t).abs() > PARALLEL_TOLERANCE || dot(e, n).abs() > PARALLEL_TOLERANCE {
                    return Err(invalid(
                        KEY,
                        format!(
                            "the advance direction {e:?} is not normal to the front {t:?} and in the plane of normal {n:?} \
                             (|e . t| = {:.3e}, |e . n| = {:.3e}, tolerance {PARALLEL_TOLERANCE:e})",
                            dot(e, t).abs(),
                            dot(e, n).abs()
                        ),
                    ));
                }
                (FrontKind::Straight { x0, t, e }, s, l)
            }
            FrontShape::Circular { centre } => {
                let d0 = sub(x0, *centre);
                let r0 = sub(d0, scaled(n, dot(d0, n)));
                let radius = norm(r0);
                if radius.is_nan() || radius <= 0.0 {
                    return Err(invalid(KEY, "the first front node is on the axis"));
                }
                let c = scaled(r0, 1.0 / radius);
                let b = cross(n, c);
                let mut s = Vec::with_capacity(x.len());
                for (k, &xi) in x.iter().enumerate() {
                    let d = sub(xi, *centre);
                    let h = dot(d, n);
                    let rho = norm(sub(d, scaled(n, h)));
                    let off = (rho - radius).abs().max(h.abs());
                    if off > FRONT_TOLERANCE * radius {
                        return Err(invalid(
                            KEY,
                            format!(
                                "node {} at {xi:?} is {off:.3e} off the circle of radius {radius:.6e} about {centre:?} \
                                 (tolerance {FRONT_TOLERANCE:e} times the radius)",
                                self.nodes[k]
                            ),
                        ));
                    }
                    s.push(radius * dot(d, b).atan2(dot(d, c)));
                }
                (
                    FrontKind::Circular {
                        centre: *centre,
                        c,
                        b,
                        radius,
                    },
                    s,
                    radius,
                )
            }
            FrontShape::Elliptical {
                centre,
                major_axis,
                a,
                b,
            } => {
                let (a, b) = (*a, *b);
                if !(a.is_finite() && b.is_finite() && b > 0.0 && a > b) {
                    return Err(invalid(
                        KEY,
                        format!("the semi-axes a = {a}, b = {b}: an ellipse needs a > b > 0 (a circle is FrontShape::Circular)"),
                    ));
                }
                let u0 = unit_of(*major_axis, KEY, "the major axis")?;
                if dot(u0, n).abs() > PARALLEL_TOLERANCE {
                    return Err(invalid(
                        KEY,
                        format!(
                            "the major axis {u0:?} is not in the plane of normal {n:?} (|u . n| = {:.3e}, tolerance \
                             {PARALLEL_TOLERANCE:e})",
                            dot(u0, n).abs()
                        ),
                    ));
                }
                let u = unit_of(sub(u0, scaled(n, dot(u0, n))), KEY, "the major axis")?;
                let v = cross(n, u);
                let c = (a * a - b * b).sqrt();
                let kind = FrontKind::Elliptical {
                    centre: *centre,
                    u,
                    v,
                    a,
                    b,
                    c,
                };
                let mut s = Vec::with_capacity(x.len());
                for (k, &xi) in x.iter().enumerate() {
                    let d = sub(xi, *centre);
                    let h = dot(d, n).abs();
                    let (xx, yy) = (dot(d, u), dot(d, v));
                    let g = (xx * xx / (a * a) + yy * yy / (b * b)).sqrt();
                    let grad = (xx * xx / (a * a * a * a) + yy * yy / (b * b * b * b)).sqrt() / g;
                    let off = ((g - 1.0).abs() / grad).max(h);
                    if off.is_nan() || off > FRONT_TOLERANCE * a {
                        return Err(invalid(
                            KEY,
                            format!(
                                "node {} at {xi:?} is {off:.3e} off the ellipse of semi-axes {a:.6e}, {b:.6e} about \
                                 {centre:?} (tolerance {FRONT_TOLERANCE:e} times a)",
                                self.nodes[k]
                            ),
                        ));
                    }
                    let Some((_, nu, _, _)) = confocal(*centre, u, v, c, xi) else {
                        return Err(invalid(
                            KEY,
                            format!("node {} at {xi:?} is on the focal segment", self.nodes[k]),
                        ));
                    };
                    s.push(nu);
                }
                (kind, s, a)
            }
        };
        for k in 1..s.len() {
            if s[k].is_nan() || s[k] <= s[k - 1] {
                return Err(invalid(
                    KEY,
                    format!(
                        "the nodes are not in order along the front (node {} at parameter {:.6e} after {:.6e})",
                        self.nodes[k],
                        s[k],
                        s[k - 1]
                    ),
                ));
            }
        }
        let stations: Vec<f64> = s.iter().step_by(2).copied().collect();
        Ok(FrontGeometry {
            n,
            kind,
            x0,
            stations,
            length,
            front_x: x,
            front_s: s,
        })
    }
}

#[derive(Debug, Clone)]
enum FrontKind {
    Straight {
        x0: Vec3,
        t: Vec3,
        e: Vec3,
    },
    Circular {
        centre: Vec3,
        c: Vec3,
        b: Vec3,
        radius: f64,
    },
    Elliptical {
        centre: Vec3,
        u: Vec3,
        v: Vec3,
        a: f64,
        b: f64,
        c: f64,
    },
}

/// The confocal elliptic coordinates `(mu, nu)` and the in-plane coordinates `(X, Y)` of `x` for the foci `(+/- c, 0)` on
/// the axis `u` (ADR 0605 D1): `X = c cosh mu cos nu`, `Y = c sinh mu sin nu`; `None` on the focal segment (`mu = 0`).
fn confocal(centre: Vec3, u: Vec3, v: Vec3, c: f64, x: Vec3) -> Option<(f64, f64, f64, f64)> {
    let d = sub(x, centre);
    let (xx, yy) = (dot(d, u), dot(d, v));
    let dp = ((xx - c) * (xx - c) + yy * yy).sqrt();
    let dm = ((xx + c) * (xx + c) + yy * yy).sqrt();
    let ch = ((dm + dp) / (2.0 * c)).max(1.0);
    let sh = (ch * ch - 1.0).sqrt();
    if sh.is_nan() || sh <= 0.0 {
        return None;
    }
    let mu = ch.acosh();
    let nu = (yy * ch).atan2(xx * sh);
    Some((mu, nu, xx, yy))
}

/// A checked front: unit normal, kind, first node, station parameters.
#[derive(Debug, Clone)]
struct FrontGeometry {
    n: Vec3,
    kind: FrontKind,
    x0: Vec3,
    stations: Vec<f64>,
    #[allow(dead_code)]
    length: f64,
    /// The front node positions and parameters (all nodes, in order).
    front_x: Vec<Vec3>,
    front_s: Vec<f64>,
}

impl FrontGeometry {
    /// The in-plane radial vector from the axis and its length (circular fronts).
    fn radial(&self, centre: Vec3, x: Vec3) -> (Vec3, f64) {
        let d = sub(x, centre);
        let r = sub(d, scaled(self.n, dot(d, self.n)));
        (r, norm(r))
    }

    /// The advance direction at `x`.
    fn advance(&self, x: Vec3) -> Result<Vec3> {
        match &self.kind {
            FrontKind::Straight { e, .. } => Ok(*e),
            FrontKind::Circular { centre, .. } => {
                let (r, l) = self.radial(*centre, x);
                if l.is_nan() || l <= 0.0 {
                    return Err(invalid(
                        "crack front",
                        format!("the point {x:?} is on the axis of the front"),
                    ));
                }
                Ok(scaled(r, 1.0 / l))
            }
            FrontKind::Elliptical { centre, u, v, c, .. } => {
                let Some((mu, _, xx, yy)) = confocal(*centre, *u, *v, *c, x) else {
                    return Err(invalid(
                        "crack front",
                        format!("the point {x:?} is on the focal segment of the front"),
                    ));
                };
                let th = mu.tanh();
                let d = [
                    xx * th * th * u[0] + yy * v[0],
                    xx * th * th * u[1] + yy * v[1],
                    xx * th * th * u[2] + yy * v[2],
                ];
                unit_of(d, "crack front", "the advance direction")
            }
        }
    }

    /// The front tangent at (the projection of) `x`.
    fn tangent(&self, x: Vec3) -> Option<Vec3> {
        match &self.kind {
            FrontKind::Straight { t, .. } => Some(*t),
            FrontKind::Circular { centre, .. } => {
                let (r, l) = self.radial(*centre, x);
                (l > 0.0).then(|| cross(self.n, scaled(r, 1.0 / l)))
            }
            FrontKind::Elliptical { .. } => self.advance(x).ok().map(|e| cross(self.n, e)),
        }
    }

    /// The front parameter of `x`.
    fn param(&self, x: Vec3) -> f64 {
        match &self.kind {
            FrontKind::Straight { x0, t, .. } => dot(sub(x, *x0), *t),
            FrontKind::Circular { centre, c, b, radius } => {
                let d = sub(x, *centre);
                radius * dot(d, *b).atan2(dot(d, *c))
            }
            FrontKind::Elliptical { centre, u, v, c, .. } => {
                confocal(*centre, *u, *v, *c, x).map_or(f64::NAN, |(_, nu, _, _)| nu)
            }
        }
    }

    /// The distance of `x` from the front measured in the crack plane's sense of D6.
    fn distance(&self, x: Vec3) -> f64 {
        match &self.kind {
            FrontKind::Straight { x0, t, .. } => {
                let d = sub(x, *x0);
                norm(sub(d, scaled(*t, dot(d, *t))))
            }
            FrontKind::Circular { centre, radius, .. } => (self.radial(*centre, x).1 - radius).abs(),
            FrontKind::Elliptical { centre, u, v, a, b, .. } => {
                // First-order distance |g - 1| / |grad g| (ADR 0605 D1); opening_k refuses this kind before using it.
                let d = sub(x, *centre);
                let (xx, yy) = (dot(d, *u), dot(d, *v));
                let g = (xx * xx / (a * a) + yy * yy / (b * b)).sqrt();
                let grad = (xx * xx / (a * a * a * a) + yy * yy / (b * b * b * b)).sqrt() / g;
                (g - 1.0).abs() / grad
            }
        }
    }

    /// The hat of station `j` at parameter `s` (half hats at the ends, 1 beyond them).
    fn hat(&self, j: usize, s: f64) -> f64 {
        let st = &self.stations;
        let last = st.len() - 1;
        let sj = st[j];
        if s <= sj {
            if j == 0 {
                return 1.0;
            }
            let sl = st[j - 1];
            if s <= sl {
                0.0
            } else {
                (s - sl) / (sj - sl)
            }
        } else {
            if j == last {
                return 1.0;
            }
            let sr = st[j + 1];
            if s >= sr {
                0.0
            } else {
                (sr - s) / (sr - sj)
            }
        }
    }

    /// `int phi_j ds`.
    fn hat_integral(&self, j: usize) -> f64 {
        if let FrontKind::Elliptical { .. } = self.kind {
            return self.edge_hat_integral(j);
        }
        let st = &self.stations;
        let last = st.len() - 1;
        let left = if j == 0 { st[0] } else { st[j - 1] };
        let right = if j == last { st[last] } else { st[j + 1] };
        0.5 * (right - left)
    }

    /// `int phi_j ds` along the quadratic front edges with the nodal hats interpolated (ADR 0605 D1), 8-point
    /// Gauss-Legendre on each edge.
    fn edge_hat_integral(&self, j: usize) -> f64 {
        let (gx, gw) = gauss_legendre(8);
        let mut total = 0.0;
        for edge in 0..self.stations.len() - 1 {
            let ids = [2 * edge, 2 * edge + 1, 2 * edge + 2];
            let h = ids.map(|i| self.hat(j, self.front_s[i]));
            if h == [0.0; 3] {
                continue;
            }
            let p = ids.map(|i| self.front_x[i]);
            for (&xi, &w) in gx.iter().zip(&gw) {
                let l = [0.5 * xi * (xi - 1.0), 1.0 - xi * xi, 0.5 * xi * (xi + 1.0)];
                let dl = [xi - 0.5, -2.0 * xi, xi + 0.5];
                let mut dx = [0.0; 3];
                for a in 0..3 {
                    for (i, dxi) in dx.iter_mut().enumerate() {
                        *dxi += dl[a] * p[a][i];
                    }
                }
                total += w * (l[0] * h[0] + l[1] * h[1] + l[2] * h[2]) * norm(dx);
            }
        }
        total
    }
}

// ----------------------------------------------------------------------------------------------------------------- rings

fn node_elements(mesh: &SolidMesh) -> Vec<Vec<usize>> {
    let mut of = vec![Vec::new(); mesh.nodes().len()];
    for e in 0..mesh.n_elements() {
        for &n in mesh.element_nodes(e) {
            of[n].push(e);
        }
    }
    of
}

/// The elements of rings `1..=ring` and `rho` (membership of `S_(ring-1)`) (ADR 0547 D2).
fn rings(mesh: &SolidMesh, adjacency: &[Vec<usize>], front: &[usize], ring: usize) -> Result<(Vec<usize>, Vec<bool>)> {
    if ring == 0 {
        return Err(invalid("ring", "ring 0 does not exist: rings are numbered from 1"));
    }
    let mut in_s = vec![false; mesh.nodes().len()];
    let mut frontier: Vec<usize> = Vec::new();
    for &n in front {
        in_s[n] = true;
        frontier.push(n);
    }
    let mut taken = vec![false; mesh.n_elements()];
    let mut elements = Vec::new();
    for level in 1..=ring {
        let mut new: Vec<usize> = Vec::new();
        for &n in &frontier {
            for &e in &adjacency[n] {
                if !taken[e] {
                    taken[e] = true;
                    new.push(e);
                }
            }
        }
        if new.is_empty() {
            return Err(invalid(
                "ring",
                format!("ring {level} has no element: the mesh is exhausted before ring {ring}"),
            ));
        }
        new.sort_unstable();
        elements.extend_from_slice(&new);
        if level < ring {
            frontier.clear();
            for &e in &new {
                for &n in mesh.element_nodes(e) {
                    if !in_s[n] {
                        in_s[n] = true;
                        frontier.push(n);
                    }
                }
            }
        }
    }
    Ok((elements, in_s))
}

fn bounding_diagonal(mesh: &SolidMesh) -> f64 {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in mesh.nodes() {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    norm(sub(hi, lo))
}

/// Refuses a ring whose `rho = 1` nodes lie on a boundary face that is not allowed (ADR 0547 D4).
fn check_boundary(mesh: &SolidMesh, geometry: &FrontGeometry, rho: &[bool], ring: usize) -> Result<()> {
    let diagonal = bounding_diagonal(mesh);
    for (_, faces) in mesh.face_incidence() {
        if faces.len() != 1 {
            continue;
        }
        let f: FaceRef = faces[0];
        let nodes = mesh.face_nodes(f);
        let Some(&hit) = nodes.iter().find(|&&a| rho[a]) else {
            continue;
        };
        let in_plane = nodes
            .iter()
            .all(|&a| dot(sub(mesh.nodes()[a], geometry.x0), geometry.n).abs() <= FRONT_TOLERANCE * diagonal);
        if in_plane {
            continue;
        }
        let mut centroid = [0.0; 3];
        for &a in &nodes {
            for i in 0..3 {
                centroid[i] += mesh.nodes()[a][i] / nodes.len() as f64;
            }
        }
        let area = mesh.face_area_vector(f);
        let la = norm(area);
        let end_face = la > 0.0
            && geometry
                .tangent(centroid)
                .is_some_and(|tau| (dot(area, tau) / la).abs() >= 1.0 - PARALLEL_TOLERANCE);
        if end_face {
            continue;
        }
        return Err(invalid(
            "ring",
            format!(
                "ring {ring} reaches the boundary: node {hit} at {:?} of the domain (q = 1) lies on the boundary face {} of \
                 element {}, which is neither a crack face nor a mirror plane nor an end face of the front",
                mesh.nodes()[hit],
                f.face,
                f.element
            ),
        ));
    }
    Ok(())
}

fn check_loads(model: &SolidModel) -> Result<()> {
    for l in model.loads() {
        let refused = match l {
            Load::Gravity { .. } => Some("gravity"),
            Load::Centrifugal { .. } => Some("a centrifugal load"),
            Load::BodyForceNodal { .. } => Some("a body force"),
            Load::Temperature { .. } => Some("a temperature field"),
            _ => None,
        };
        if let Some(what) = refused {
            return Err(invalid(
                "load",
                format!("the domain integral has no body-force or thermal term: the model has {what}"),
            ));
        }
    }
    Ok(())
}

/// `(E, nu)` of an element for the fracture functions: isotropic elastic, a solid kind, the full formulation.
fn element_elastic(model: &SolidModel, materials: &[usize], e: usize) -> Result<(f64, f64)> {
    let mesh = model.mesh();
    let kind = mesh.element_kind(e);
    if !kind.is_solid() {
        return Err(invalid(
            "element",
            format!(
                "element {e} is a {}: the domain integral takes solid elements",
                kind.name()
            ),
        ));
    }
    let formulation = mesh.blocks()[mesh.block_of(e)].formulation();
    if formulation != SolidFormulation::Full {
        return Err(invalid(
            "element",
            format!(
                "element {e} has the formulation {}: the domain integral takes the full formulation",
                formulation.name()
            ),
        ));
    }
    match &model.materials()[materials[e]].1 {
        Material::IsotropicElastic { e: young, nu, .. } => Ok((*young, *nu)),
        other => Err(invalid(
            "element",
            format!(
                "element {e} has the material {}: the domain integral takes an isotropic elastic material",
                other.name()
            ),
        )),
    }
}

// ------------------------------------------------------------------------------------------------------- domain integral

/// The energy release rate at one station of a front (ADR 0547 D3).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FrontValue {
    /// The station index (0 at the first front node).
    pub station: usize,
    /// The corner node of the station.
    pub node: usize,
    /// Its position.
    pub position: Vec3,
    /// Its front parameter (arc length from the first node's reference).
    pub s: f64,
    /// The energy release rate `J`, J/m^2.
    pub j: f64,
    /// `int phi ds` of its hat, m.
    pub weight: f64,
}

/// The energy release rate at each station of `front` by the equivalent domain integral over ring `ring` (ADR 0547 D2 to
/// D4): `q = rho phi_j e` at the nodes, `rho = 1` on the nodes of rings `1..ring-1` and the front, the integral
/// `int (sigma_ij u_i,k - W delta_jk) q_k,j dV` over the elements of rings `1..=ring` with each element's stiffness rule,
/// divided by `int phi_j ds` (doubled for a half-symmetric model). Refuses an invalid front, ring 0, a ring beyond the mesh,
/// a ring that reaches the boundary, an element of the rings that is not a full-formulation solid of an isotropic elastic
/// material, a model with a body or thermal load and a displacement of the wrong length.
pub fn j_integral(
    model: &SolidModel,
    displacement: &[f64],
    front: &CrackFront,
    ring: usize,
) -> Result<Vec<FrontValue>> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    check_loads(model)?;
    let mesh = model.mesh();
    let geometry = front.check(mesh)?;
    let adjacency = node_elements(mesh);
    let (elements, rho) = rings(mesh, &adjacency, &front.nodes, ring)?;
    check_boundary(mesh, &geometry, &rho, ring)?;
    let materials = model.material_indices()?;
    let n_st = geometry.stations.len();
    // Nodal q per station, only at the rho nodes: (direction, parameter).
    let mut nodal: HashMap<usize, (Vec3, f64)> = HashMap::new();
    for e in &elements {
        for &a in mesh.element_nodes(*e) {
            if rho[a] && !nodal.contains_key(&a) {
                let x = mesh.nodes()[a];
                nodal.insert(a, (geometry.advance(x)?, geometry.param(x)));
            }
        }
    }
    let mut g_bar = vec![0.0; n_st];
    for &e in &elements {
        let (young, nu) = element_elastic(model, &materials, e)?;
        let lambda = young * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = young / (2.0 * (1.0 + nu));
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let mut ue: Vec<Vec3> = Vec::with_capacity(nodes.len());
        for &n in nodes {
            let mut u = [0.0; 3];
            for (c, uc) in u.iter_mut().enumerate() {
                let q = dofs.equation(n, c).ok_or(FemError::MissingComponent {
                    role: "displacement",
                    node: n,
                    component: c,
                    available: dofs.components(n),
                })?;
                *uc = displacement[q];
            }
            ue.push(u);
        }
        // q_a for every station at this element's nodes.
        let mut qa: Vec<Vec<Vec3>> = vec![vec![[0.0; 3]; nodes.len()]; n_st];
        for (la, &a) in nodes.iter().enumerate() {
            if let Some(&(dir, s)) = nodal.get(&a) {
                for (j, qj) in qa.iter_mut().enumerate() {
                    let phi = geometry.hat(j, s);
                    qj[la] = scaled(dir, phi);
                }
            }
        }
        let points = element
            .integration_points(&coords, element.stiffness_rule())
            .map_err(|source| FemError::Element {
                element: e,
                source: Box::new(source),
            })?;
        for ip in points {
            let grad = &ip.geometry.grad;
            // du[i][k] = du_i/dx_k.
            let mut du = [[0.0; 3]; 3];
            for (a, g) in grad.iter().enumerate() {
                for i in 0..3 {
                    for k in 0..3 {
                        du[i][k] += ue[a][i] * g[k];
                    }
                }
            }
            let mut eps = [[0.0; 3]; 3];
            for i in 0..3 {
                for k in 0..3 {
                    eps[i][k] = 0.5 * (du[i][k] + du[k][i]);
                }
            }
            let tr = eps[0][0] + eps[1][1] + eps[2][2];
            let mut sigma = [[0.0; 3]; 3];
            for i in 0..3 {
                for k in 0..3 {
                    sigma[i][k] = 2.0 * mu * eps[i][k] + if i == k { lambda * tr } else { 0.0 };
                }
            }
            let mut w = 0.0;
            for i in 0..3 {
                for k in 0..3 {
                    w += 0.5 * sigma[i][k] * eps[i][k];
                }
            }
            // p[j][k] = sigma_ij u_i,k - W delta_jk.
            let mut p = [[0.0; 3]; 3];
            for j in 0..3 {
                for k in 0..3 {
                    let mut v = 0.0;
                    for i in 0..3 {
                        v += sigma[i][j] * du[i][k];
                    }
                    p[j][k] = v - if j == k { w } else { 0.0 };
                }
            }
            for (st, qj) in qa.iter().enumerate() {
                // dq[k][j] = dq_k/dx_j.
                let mut dq = [[0.0; 3]; 3];
                let mut any = false;
                for (a, g) in grad.iter().enumerate() {
                    let q = qj[a];
                    if q == [0.0; 3] {
                        continue;
                    }
                    any = true;
                    for k in 0..3 {
                        for j in 0..3 {
                            dq[k][j] += q[k] * g[j];
                        }
                    }
                }
                if !any {
                    continue;
                }
                let mut v = 0.0;
                for j in 0..3 {
                    for k in 0..3 {
                        v += p[j][k] * dq[k][j];
                    }
                }
                g_bar[st] += ip.weight * v;
            }
        }
    }
    let factor = if front.half_symmetric { 2.0 } else { 1.0 };
    Ok((0..n_st)
        .map(|j| {
            let node = front.nodes[2 * j];
            let weight = geometry.hat_integral(j);
            FrontValue {
                station: j,
                node,
                position: mesh.nodes()[node],
                s: geometry.stations[j],
                j: factor * g_bar[j] / weight,
                weight,
            }
        })
        .collect())
}

/// `K_I = sqrt(J E / (1 - nu^2))`, the plane-strain mode-I factor of an energy release rate (ADR 0547 D5). Refuses a negative
/// or non-finite `J`, `E <= 0` and `nu` outside `(-1, 0.5)`.
pub fn plane_strain_k(j: f64, e: f64, nu: f64) -> Result<f64> {
    if !(j.is_finite() && j >= 0.0) {
        return Err(invalid("j", format!("J = {j} must be finite and non-negative")));
    }
    if !(e.is_finite() && e > 0.0) || !(nu > -1.0 && nu < 0.5) {
        return Err(invalid(
            "material",
            format!("E = {e}, nu = {nu}: E > 0 and -1 < nu < 0.5"),
        ));
    }
    Ok((j * e / (1.0 - nu * nu)).sqrt())
}

// --------------------------------------------------------------------------------------------------- interaction integral

/// A mode of the near-tip field (ADR 0605 D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipMode {
    /// Opening (plane strain).
    I,
    /// In-plane sliding (plane strain).
    II,
    /// Out-of-plane tearing (antiplane).
    III,
}

/// The displacement and its gradient `g[i][k] = du_i/dx_k` of the near-tip field of unit stress intensity factor of `mode`
/// (ADR 0605 D5) at the polar position `(r, theta)` in the local frame of a straight front (`x1` the advance, `x2` the
/// crack-plane normal, `x3 = x1 x x2`, `theta` in `(-pi, pi]` with the crack faces at `+/- pi`), plane strain with shear
/// modulus `mu` and Poisson's ratio `nu`: I `u1, u2 = sqrt(r/(2 pi))/(2 mu) (cos, sin)(theta/2) (kappa - cos theta)`; II `u1 =
/// sqrt(r/(2 pi))/(2 mu) sin(theta/2) (kappa + 2 + cos theta)`, `u2 = -sqrt(r/(2 pi))/(2 mu) cos(theta/2) (kappa - 2 + cos
/// theta)`; III `u3 = 2/mu sqrt(r/(2 pi)) sin(theta/2)`; `kappa = 3 - 4 nu`. `r <= 0` gives non-finite values.
pub fn near_tip_gradient(mode: TipMode, r: f64, theta: f64, mu: f64, nu: f64) -> (Vec3, [[f64; 3]; 3]) {
    let kappa = 3.0 - 4.0 * nu;
    let (c2, s2) = ((0.5 * theta).cos(), (0.5 * theta).sin());
    let (ct, st) = (theta.cos(), theta.sin());
    // u_i = f sqrt(r/(2 pi)) g_i(theta); dg_i = g_i'(theta).
    let (f, g, dg) = match mode {
        TipMode::I => (
            0.5 / mu,
            [c2 * (kappa - ct), s2 * (kappa - ct), 0.0],
            [
                -0.5 * s2 * (kappa - ct) + c2 * st,
                0.5 * c2 * (kappa - ct) + s2 * st,
                0.0,
            ],
        ),
        TipMode::II => (
            0.5 / mu,
            [s2 * (kappa + 2.0 + ct), -c2 * (kappa - 2.0 + ct), 0.0],
            [
                0.5 * c2 * (kappa + 2.0 + ct) - s2 * st,
                0.5 * s2 * (kappa - 2.0 + ct) + c2 * st,
                0.0,
            ],
        ),
        TipMode::III => (0.5 / mu, [0.0, 0.0, 4.0 * s2], [0.0, 0.0, 2.0 * c2]),
    };
    let root = (r / (2.0 * PI)).sqrt();
    let u = [f * root * g[0], f * root * g[1], f * root * g[2]];
    let mut grad = [[0.0; 3]; 3];
    for i in 0..3 {
        let d_r = f * g[i] / (2.0 * (2.0 * PI * r).sqrt());
        let d_t = f * root * dg[i];
        grad[i][0] = d_r * ct - d_t * st / r;
        grad[i][1] = d_r * st + d_t * ct / r;
    }
    (u, grad)
}

/// The stress intensity factors at one station of a straight front (ADR 0605 D5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ModeFactors {
    /// The station index (0 at the first front node).
    pub station: usize,
    /// The corner node of the station.
    pub node: usize,
    /// Its position.
    pub position: Vec3,
    /// Its front parameter.
    pub s: f64,
    /// `K_I`, Pa m^0.5 (opening along the crack-plane normal).
    pub k_i: f64,
    /// `K_II`, Pa m^0.5 (sliding along the advance direction).
    pub k_ii: f64,
    /// `K_III`, Pa m^0.5 (tearing along `e x n`).
    pub k_iii: f64,
    /// `int phi ds` of its hat, m.
    pub weight: f64,
}

/// `K_I`, `K_II` and `K_III` at each station of a straight `front` by the interaction integral over ring `ring` (ADR 0605
/// D5): `M = int (sigma_ij u^aux_i,k + sigma^aux_ij u_i,k - sigma_ij eps^aux_ij delta_jk) q_k,j dV` with the unit near-tip
/// field of each mode ([`near_tip_gradient`]) in the frame `(e, n, e x n)` of the front, `q` and the rings of
/// [`j_integral`], divided by `int phi_j ds`; `K_I = E' M_I/2`, `K_II = E' M_II/2`, `K_III = mu M_III`, `E' = E/(1 - nu^2)`, with
/// `E, nu` of the element of the first front node. Refuses what [`j_integral`] refuses, a front that is not straight, a
/// half-symmetric front and a ring element of another `E, nu`.
pub fn interaction_integral(
    model: &SolidModel,
    displacement: &[f64],
    front: &CrackFront,
    ring: usize,
) -> Result<Vec<ModeFactors>> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    check_loads(model)?;
    if front.half_symmetric {
        return Err(invalid(
            "crack front",
            "the interaction integral takes a full model: modes II and III are antisymmetric about the crack plane, so a \
             half-symmetric front cannot be doubled",
        ));
    }
    let mesh = model.mesh();
    let geometry = front.check(mesh)?;
    let FrontKind::Straight { x0, e, .. } = geometry.kind else {
        return Err(invalid(
            "crack front",
            "the interaction integral takes a straight front: the near-tip fields are not exact on a curved one",
        ));
    };
    let n = geometry.n;
    let e3 = cross(e, n);
    let basis = [e, n, e3];
    let adjacency = node_elements(mesh);
    let (elements, rho) = rings(mesh, &adjacency, &front.nodes, ring)?;
    check_boundary(mesh, &geometry, &rho, ring)?;
    let materials = model.material_indices()?;
    let first = front.nodes[0];
    let Some(&e0) = adjacency[first].first() else {
        return Err(invalid(
            "crack front",
            format!("the front node {first} belongs to no element"),
        ));
    };
    let (young0, nu0) = element_elastic(model, &materials, e0)?;
    let n_st = geometry.stations.len();
    let mut nodal: HashMap<usize, (Vec3, f64)> = HashMap::new();
    for el in &elements {
        for &a in mesh.element_nodes(*el) {
            if rho[a] && !nodal.contains_key(&a) {
                let x = mesh.nodes()[a];
                nodal.insert(a, (geometry.advance(x)?, geometry.param(x)));
            }
        }
    }
    let modes = [TipMode::I, TipMode::II, TipMode::III];
    let mut m_bar = vec![[0.0; 3]; n_st];
    for &el in &elements {
        let (young, nu) = element_elastic(model, &materials, el)?;
        if young != young0 || nu != nu0 {
            return Err(invalid(
                "element",
                format!(
                    "element {el} has E = {young}, nu = {nu}, the element {e0} of the first front node E = {young0}, nu = \
                     {nu0}: the interaction integral takes one material in the domain"
                ),
            ));
        }
        let lambda = young * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = young / (2.0 * (1.0 + nu));
        let kind = mesh.element_kind(el);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(el);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let mut ue: Vec<Vec3> = Vec::with_capacity(nodes.len());
        for &a in nodes {
            let mut u = [0.0; 3];
            for (c, uc) in u.iter_mut().enumerate() {
                let q = dofs.equation(a, c).ok_or(FemError::MissingComponent {
                    role: "displacement",
                    node: a,
                    component: c,
                    available: dofs.components(a),
                })?;
                *uc = displacement[q];
            }
            ue.push(u);
        }
        let mut qa: Vec<Vec<Vec3>> = vec![vec![[0.0; 3]; nodes.len()]; n_st];
        for (la, &a) in nodes.iter().enumerate() {
            if let Some(&(dir, s)) = nodal.get(&a) {
                for (j, qj) in qa.iter_mut().enumerate() {
                    qj[la] = scaled(dir, geometry.hat(j, s));
                }
            }
        }
        let points = element
            .integration_points(&coords, element.stiffness_rule())
            .map_err(|source| FemError::Element {
                element: el,
                source: Box::new(source),
            })?;
        for ip in points {
            let grad = &ip.geometry.grad;
            let mut du = [[0.0; 3]; 3];
            for (a, g) in grad.iter().enumerate() {
                for i in 0..3 {
                    for k in 0..3 {
                        du[i][k] += ue[a][i] * g[k];
                    }
                }
            }
            let sigma = hooke(&du, lambda, mu);
            // dq[st][k][j] = dq_k/dx_j.
            let mut dq: Vec<Option<[[f64; 3]; 3]>> = Vec::with_capacity(n_st);
            for qj in &qa {
                let mut d = [[0.0; 3]; 3];
                let mut any = false;
                for (a, g) in grad.iter().enumerate() {
                    let q = qj[a];
                    if q == [0.0; 3] {
                        continue;
                    }
                    any = true;
                    for k in 0..3 {
                        for j in 0..3 {
                            d[k][j] += q[k] * g[j];
                        }
                    }
                }
                dq.push(any.then_some(d));
            }
            if dq.iter().all(Option::is_none) {
                continue;
            }
            let rel = sub(ip.geometry.x, x0);
            let (x1, x2) = (dot(rel, e), dot(rel, n));
            let r = (x1 * x1 + x2 * x2).sqrt();
            if r.is_nan() || r <= 0.0 {
                return Err(invalid(
                    "crack front",
                    format!("an integration point of element {el} lies on the front line"),
                ));
            }
            let theta = x2.atan2(x1);
            for (m, &mode) in modes.iter().enumerate() {
                let (_, local) = near_tip_gradient(mode, r, theta, mu, nu);
                // G[i][k] = sum_ab basis_a[i] local[a][b] basis_b[k].
                let mut ga = [[0.0; 3]; 3];
                for i in 0..3 {
                    for k in 0..3 {
                        let mut v = 0.0;
                        for (a, ba) in basis.iter().enumerate() {
                            for (b, bb) in basis.iter().enumerate() {
                                v += ba[i] * local[a][b] * bb[k];
                            }
                        }
                        ga[i][k] = v;
                    }
                }
                let sigma_aux = hooke(&ga, lambda, mu);
                let mut w_int = 0.0;
                for i in 0..3 {
                    for k in 0..3 {
                        w_int += sigma[i][k] * 0.5 * (ga[i][k] + ga[k][i]);
                    }
                }
                let mut p = [[0.0; 3]; 3];
                for j in 0..3 {
                    for k in 0..3 {
                        let mut v = 0.0;
                        for i in 0..3 {
                            v += sigma[i][j] * ga[i][k] + sigma_aux[i][j] * du[i][k];
                        }
                        p[j][k] = v - if j == k { w_int } else { 0.0 };
                    }
                }
                for (st, d) in dq.iter().enumerate() {
                    let Some(d) = d else { continue };
                    let mut v = 0.0;
                    for j in 0..3 {
                        for k in 0..3 {
                            v += p[j][k] * d[k][j];
                        }
                    }
                    m_bar[st][m] += ip.weight * v;
                }
            }
        }
    }
    let e_prime = young0 / (1.0 - nu0 * nu0);
    let mu0 = young0 / (2.0 * (1.0 + nu0));
    Ok((0..n_st)
        .map(|j| {
            let node = front.nodes[2 * j];
            let weight = geometry.hat_integral(j);
            let m = m_bar[j];
            ModeFactors {
                station: j,
                node,
                position: mesh.nodes()[node],
                s: geometry.stations[j],
                k_i: 0.5 * e_prime * m[0] / weight,
                k_ii: 0.5 * e_prime * m[1] / weight,
                k_iii: mu0 * m[2] / weight,
                weight,
            }
        })
        .collect())
}

/// `sigma = lambda tr(eps) I + 2 mu eps`, `eps = sym(g)`.
fn hooke(g: &[[f64; 3]; 3], lambda: f64, mu: f64) -> [[f64; 3]; 3] {
    let tr = g[0][0] + g[1][1] + g[2][2];
    let mut sigma = [[0.0; 3]; 3];
    for i in 0..3 {
        for k in 0..3 {
            sigma[i][k] = mu * (g[i][k] + g[k][i]) + if i == k { lambda * tr } else { 0.0 };
        }
    }
    sigma
}

// --------------------------------------------------------------------------------------------------------------- opening

/// A pair of crack-face nodes at the same position: `upper` on the `+normal` side, `lower` on the other (`None` in a
/// half-symmetric model, the opening then twice the upper node's normal displacement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningPair {
    /// The node on the `+normal` face.
    pub upper: usize,
    /// The coincident node on the `-normal` face.
    pub lower: Option<usize>,
}

/// The line `K_app = k + slope r` through the apparent factors (ADR 0547 D6).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct OpeningFit {
    /// The extrapolated factor at `r = 0`, Pa m^0.5.
    pub k: f64,
    /// The slope, Pa m^-0.5.
    pub slope: f64,
    /// The number of pairs used.
    pub points: usize,
}

/// `K_I` from the opening behind the front (ADR 0547 D6): `K_app(r) = E delta / (8 (1 - nu^2)) sqrt(2 pi / r)` at the pairs
/// whose distance `r` from the front is in `[r_min, r_max]`, `E, nu` of the element of the first front node, and the
/// least-squares line through `(r, K_app)` extrapolated to `r = 0`. Refuses fewer than 2 pairs in the interval, an empty
/// interval, a pair whose nodes do not coincide, and what [`j_integral`] refuses of the front.
pub fn opening_k(
    model: &SolidModel,
    displacement: &[f64],
    front: &CrackFront,
    pairs: &[OpeningPair],
    r_min: f64,
    r_max: f64,
) -> Result<OpeningFit> {
    const KEY: &str = "opening";
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    if !(r_min.is_finite() && r_max.is_finite() && r_min >= 0.0 && r_min < r_max) {
        return Err(invalid(
            KEY,
            format!("the interval [{r_min}, {r_max}] is empty or not finite"),
        ));
    }
    if let FrontShape::Elliptical { .. } = front.shape {
        return Err(invalid(
            KEY,
            "opening_k takes a straight or circular front (ADR 0605 D1), not an elliptical one",
        ));
    }
    let mesh = model.mesh();
    let geometry = front.check(mesh)?;
    let adjacency = node_elements(mesh);
    let materials = model.material_indices()?;
    let first = front.nodes[0];
    let Some(&e0) = adjacency[first].first() else {
        return Err(invalid(KEY, format!("the front node {first} belongs to no element")));
    };
    let (young, nu) = element_elastic(model, &materials, e0)?;
    let diagonal = bounding_diagonal(mesh);
    let u_of = |n: usize| -> Result<Vec3> {
        let mut u = [0.0; 3];
        for (c, uc) in u.iter_mut().enumerate() {
            let q = dofs.equation(n, c).ok_or(FemError::MissingComponent {
                role: "displacement",
                node: n,
                component: c,
                available: dofs.components(n),
            })?;
            *uc = displacement[q];
        }
        Ok(u)
    };
    let nn = mesh.nodes().len();
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for p in pairs {
        if p.upper >= nn || p.lower.is_some_and(|l| l >= nn) {
            return Err(invalid(KEY, format!("the pair {p:?} names a node that does not exist")));
        }
        let x = mesh.nodes()[p.upper];
        let delta = match p.lower {
            Some(l) => {
                let gap = norm(sub(mesh.nodes()[l], x));
                if gap > FRONT_TOLERANCE * diagonal {
                    return Err(invalid(
                        KEY,
                        format!("the nodes {} and {l} of a pair are {gap:.3e} apart", p.upper),
                    ));
                }
                dot(sub(u_of(p.upper)?, u_of(l)?), geometry.n)
            }
            None => 2.0 * dot(u_of(p.upper)?, geometry.n),
        };
        let r = geometry.distance(x);
        if r >= r_min && r <= r_max && r > 0.0 {
            let k_app = young * delta / (8.0 * (1.0 - nu * nu)) * (2.0 * PI / r).sqrt();
            pts.push((r, k_app));
        }
    }
    if pts.len() < 2 {
        return Err(invalid(
            KEY,
            format!(
                "{} pairs in [{r_min:.6e}, {r_max:.6e}]: the line needs at least 2",
                pts.len()
            ),
        ));
    }
    let n = pts.len() as f64;
    let mr = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let mk = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pts.iter().map(|p| (p.0 - mr) * (p.0 - mr)).sum();
    let sxy: f64 = pts.iter().map(|p| (p.0 - mr) * (p.1 - mk)).sum();
    let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    Ok(OpeningFit {
        k: mk - slope * mr,
        slope,
        points: pts.len(),
    })
}

// ----------------------------------------------------------------------------------------------------------------- Paris

/// Nodes and weights of the `n`-point Gauss-Legendre rule on `[-1, 1]` (Newton's iteration on `P_n`).
fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut x = vec![0.0; n];
    let mut w = vec![0.0; n];
    for i in 0..n.div_ceil(2) {
        let mut z = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 0.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, z);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * z * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (z * p1 - p0) / (z * z - 1.0);
            let dz = p1 / dp;
            z -= dz;
            if dz.abs() <= 1e-16 {
                break;
            }
        }
        x[i] = -z;
        x[n - 1 - i] = z;
        let wi = 2.0 / ((1.0 - z * z) * dp * dp);
        w[i] = wi;
        w[n - 1 - i] = wi;
    }
    (x, w)
}

/// The cycles of the Paris law `da/dN = C (Delta K)^m` from `a0` to `a1` (ADR 0547 D7): `int da / (C Delta K(a)^m)` with
/// `a = a0 (a1/a0)^tau`, composite 16-point Gauss-Legendre in `tau`, panels doubled until two sums agree within `1e-13`
/// relative. Refuses `C <= 0`, `m <= 0`, `a0 <= 0`, `a1 <= a0`, non-finite arguments, a `Delta K` that is not finite and
/// positive at a point used, and no convergence within `2^16` panels.
pub fn paris_cycles<F>(c: f64, m: f64, a0: f64, a1: f64, delta_k: F) -> Result<f64>
where
    F: Fn(f64) -> f64,
{
    const KEY: &str = "paris";
    if !(c.is_finite() && c > 0.0) || !(m.is_finite() && m > 0.0) {
        return Err(invalid(
            KEY,
            format!("C = {c}, m = {m}: both must be finite and positive"),
        ));
    }
    if !(a0.is_finite() && a1.is_finite() && a0 > 0.0 && a1 > a0) {
        return Err(invalid(KEY, format!("a0 = {a0}, a1 = {a1}: 0 < a0 < a1 and finite")));
    }
    let (gx, gw) = gauss_legendre(16);
    let ln = (a1 / a0).ln();
    let integrand = |tau: f64| -> Result<f64> {
        let a = a0 * (a1 / a0).powf(tau);
        let dk = delta_k(a);
        if !(dk.is_finite() && dk > 0.0) {
            return Err(invalid(
                KEY,
                format!("Delta K({a:.6e}) = {dk}: it must be finite and positive"),
            ));
        }
        Ok(a * ln / (c * dk.powf(m)))
    };
    let sum_panels = |panels: usize| -> Result<f64> {
        let h = 1.0 / panels as f64;
        let mut total = 0.0;
        for p in 0..panels {
            let mid = (p as f64 + 0.5) * h;
            let mut part = 0.0;
            for (x, w) in gx.iter().zip(&gw) {
                part += w * integrand(mid + 0.5 * h * x)?;
            }
            total += 0.5 * h * part;
        }
        Ok(total)
    };
    let mut panels = 1;
    let mut previous = sum_panels(panels)?;
    while panels < 1 << 16 {
        panels *= 2;
        let next = sum_panels(panels)?;
        if (next - previous).abs() <= 1e-13 * next.abs() {
            return Ok(next);
        }
        previous = next;
    }
    Err(invalid(
        KEY,
        format!("the integral did not converge within {panels} panels (last sum {previous:e})"),
    ))
}

// -------------------------------------------------------------------------------------------------------------- builders

/// The positions after a uniform zone: `n` elements of sizes `h0 r, ..., h0 r^n` summing to `length` with `r <= growth`, `n`
/// the least with `h0 sum growth^i >= length` (ADR 0547 D8). Returns the cumulative offsets `d_1 < ... < d_n = length`.
fn graded(length: f64, h0: f64, growth: f64) -> Vec<f64> {
    let total = |r: f64, n: usize| -> f64 {
        let mut s = 0.0;
        let mut p = 1.0;
        for _ in 0..n {
            p *= r;
            s += h0 * p;
        }
        s
    };
    let mut n = 1;
    while total(growth, n) < length {
        n += 1;
    }
    let (mut lo, mut hi) = (0.0, growth);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if total(mid, n) < length {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo <= 1e-15 * hi {
            break;
        }
    }
    let r = 0.5 * (lo + hi);
    let mut out = Vec::with_capacity(n);
    let mut s = 0.0;
    let mut p = 1.0;
    for _ in 0..n {
        p *= r;
        s += h0 * p;
        out.push(s);
    }
    *out.last_mut().expect("n >= 1") = length;
    out
}

fn positive(key: &'static str, values: &[(&str, f64)]) -> Result<()> {
    for (name, v) in values {
        if !(v.is_finite() && *v > 0.0) {
            return Err(invalid(key, format!("{name} = {v} must be finite and positive")));
        }
    }
    Ok(())
}

/// The centre-cracked plate of ADR 0547 D8.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PlateCrack {
    /// Half crack length `a`.
    pub half_length: f64,
    /// Half-width `W`.
    pub half_width: f64,
    /// Half-height `H`.
    pub half_height: f64,
    /// Thickness `t` (one element layer).
    pub thickness: f64,
    /// Element size `h0` of the uniform zone.
    pub h0: f64,
    /// Largest growth ratio of the graded zones.
    pub growth: f64,
    /// Element layers of the uniform zone on each side of a tip.
    pub uniform_layers: usize,
}

impl PlateCrack {
    /// The plate with the given dimensions and grading.
    pub fn new(
        half_length: f64,
        half_width: f64,
        half_height: f64,
        thickness: f64,
        h0: f64,
        growth: f64,
        uniform_layers: usize,
    ) -> PlateCrack {
        PlateCrack {
            half_length,
            half_width,
            half_height,
            thickness,
            h0,
            growth,
            uniform_layers,
        }
    }
}

/// The mesh of a centre-cracked plate and what the fracture functions need.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CrackedPlate {
    /// The Hex20 mesh (element set `all`, node sets `all`, `right_pin`, `left_pin`, surface sets `top`, `bottom`, `left`,
    /// `right`).
    pub mesh: SolidMesh,
    /// The right-tip front, `(a, 0, 0), (a, 0, t/2), (a, 0, t)`.
    pub front: Vec<usize>,
    /// The opening pairs at `z = 0`, `0 < x < a`.
    pub pairs: Vec<OpeningPair>,
    /// Element columns between the right tip and `x = W`.
    pub columns_to_edge: usize,
}

impl CrackedPlate {
    /// The right-tip front as a [`CrackFront`]: plane normal `y`, advance `x`.
    pub fn crack_front(&self) -> CrackFront {
        CrackFront::straight(self.front.clone(), [0.0, 1.0, 0.0], [1.0, 0.0, 0.0])
    }
}

/// A plate `[-W, W] x [-H, H] x [0, t]` with a centre slit `|x| < a` on `y = 0` of doubled nodes, Hex20 graded towards both
/// tips (ADR 0547 D8).
pub fn centre_crack_plate(spec: &PlateCrack) -> Result<CrackedPlate> {
    const KEY: &str = "plate crack";
    let PlateCrack {
        half_length: a,
        half_width: w,
        half_height: hh,
        thickness: t,
        h0,
        growth: g,
        uniform_layers: nu_,
    } = *spec;
    positive(
        KEY,
        &[("a", a), ("W", w), ("H", hh), ("t", t), ("h0", h0), ("growth", g)],
    )?;
    let zone = nu_ as f64 * h0;
    if nu_ == 0 || g <= 1.0 || zone >= a || a + zone >= w || zone >= hh {
        return Err(invalid(
            KEY,
            format!(
                "uniform zone {nu_} x {h0} must fit inside the crack, the width and the height (a = {a}, W = {w}, H = {hh}); \
                 growth {g} > 1"
            ),
        ));
    }
    // x >= 0 positions.
    let mut right = vec![0.0];
    let inner = graded(a - zone, h0, g);
    for d in inner.iter().rev().skip(1) {
        right.push(a - zone - d);
    }
    for i in 0..nu_ {
        right.push(a - (nu_ - i) as f64 * h0);
    }
    right.push(a);
    for i in 1..=nu_ {
        right.push(a + i as f64 * h0);
    }
    for d in graded(w - a - zone, h0, g) {
        right.push(a + zone + d);
    }
    *right.last_mut().expect("nonempty") = w;
    let mut xs: Vec<f64> = right.iter().skip(1).rev().map(|x| -x).collect();
    xs.extend_from_slice(&right);
    let mut up = vec![0.0];
    for i in 1..=nu_ {
        up.push(i as f64 * h0);
    }
    for d in graded(hh - zone, h0, g) {
        up.push(zone + d);
    }
    *up.last_mut().expect("nonempty") = hh;
    let mut ys: Vec<f64> = up.iter().skip(1).rev().map(|y| -y).collect();
    ys.extend_from_slice(&up);
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let jc = ny / 2;
    let i0 = nx / 2;
    let i_r = xs.iter().position(|&x| x == a).expect("a is a grid line");
    let i_l = xs.iter().position(|&x| x == -a).expect("-a is a grid line");
    let base = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let n_base = (nx + 1) * (ny + 1) * 2;
    let n_inner = i_r - i_l - 1;
    let lower = |i: usize, k: usize| n_base + (i - i_l - 1) + n_inner * k;
    let mut nodes = Vec::with_capacity(n_base + 2 * n_inner);
    for k in 0..2 {
        for &y in &ys {
            for &x in &xs {
                nodes.push([x, y, if k == 0 { 0.0 } else { t }]);
            }
        }
    }
    for k in 0..2 {
        for &x in &xs[i_l + 1..i_r] {
            nodes.push([x, 0.0, if k == 0 { 0.0 } else { t }]);
        }
    }
    let nid = |i: usize, j: usize, k: usize, below: bool| {
        if below && j == jc && i > i_l && i < i_r {
            lower(i, k)
        } else {
            base(i, j, k)
        }
    };
    let mut conn = Vec::with_capacity(8 * nx * ny);
    for j in 0..ny {
        let below = j + 1 == jc;
        for i in 0..nx {
            conn.extend_from_slice(&[
                nid(i, j, 0, below),
                nid(i + 1, j, 0, below),
                nid(i + 1, j + 1, 0, below),
                nid(i, j + 1, 0, below),
                nid(i, j, 1, below),
                nid(i + 1, j, 1, below),
                nid(i + 1, j + 1, 1, below),
                nid(i, j + 1, 1, below),
            ]);
        }
    }
    let mut mesh = SolidMesh::new(nodes)?;
    mesh.add_block(ElementKind::Hex8, conn)?;
    let ne = mesh.n_elements();
    let n_nodes = mesh.nodes().len();
    mesh.add_element_set("all", (0..ne).collect())?;
    mesh.add_node_set("all", (0..n_nodes).collect())?;
    mesh.add_node_set("right_pin", vec![base(nx, jc, 0), base(nx, jc, 1)])?;
    mesh.add_node_set("left_pin", vec![base(0, jc, 0), base(0, jc, 1)])?;
    let el = |i: usize, j: usize| i + nx * j;
    mesh.add_surface_set(
        "top",
        (0..nx)
            .map(|i| FaceRef {
                element: el(i, ny - 1),
                face: 4,
            })
            .collect(),
    )?;
    mesh.add_surface_set(
        "bottom",
        (0..nx)
            .map(|i| FaceRef {
                element: el(i, 0),
                face: 2,
            })
            .collect(),
    )?;
    mesh.add_surface_set(
        "left",
        (0..ny)
            .map(|j| FaceRef {
                element: el(0, j),
                face: 5,
            })
            .collect(),
    )?;
    mesh.add_surface_set(
        "right",
        (0..ny)
            .map(|j| FaceRef {
                element: el(nx - 1, j),
                face: 3,
            })
            .collect(),
    )?;
    let (mesh, mids) = upgrade_to_hex20(&mesh)?;
    let mid: HashMap<(usize, usize), usize> = mids
        .iter()
        .map(|m| ((m.ends[0].min(m.ends[1]), m.ends[0].max(m.ends[1])), m.node))
        .collect();
    let edge = |p: usize, q: usize| -> Result<usize> {
        mid.get(&(p.min(q), p.max(q)))
            .copied()
            .ok_or_else(|| invalid(KEY, format!("no mid-edge node between {p} and {q}")))
    };
    let front = vec![
        base(i_r, jc, 0),
        edge(base(i_r, jc, 0), base(i_r, jc, 1))?,
        base(i_r, jc, 1),
    ];
    let mut pairs = Vec::new();
    for i in i0..i_r {
        if i > i0 {
            pairs.push(OpeningPair {
                upper: base(i, jc, 0),
                lower: Some(lower(i, 0)),
            });
        }
        let up_next = base(i + 1, jc, 0);
        let low_next = if i + 1 == i_r { up_next } else { lower(i + 1, 0) };
        pairs.push(OpeningPair {
            upper: edge(base(i, jc, 0), up_next)?,
            lower: Some(edge(lower(i, 0), low_next)?),
        });
    }
    Ok(CrackedPlate {
        mesh,
        front,
        pairs,
        columns_to_edge: nx - i_r,
    })
}

/// The penny-shaped crack in a cylinder of ADR 0547 D8.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PennyCrack {
    /// Crack radius `a`.
    pub radius: f64,
    /// Cylinder radius `R`.
    pub cylinder_radius: f64,
    /// Half-height `H` of the cylinder.
    pub half_height: f64,
    /// Element size `h0` of the uniform zone.
    pub h0: f64,
    /// Element layers of the uniform zone on each side of the front and above it.
    pub uniform_layers: usize,
    /// Elements over the quarter circle (even).
    pub n_theta: usize,
    /// Side `s` of the core square.
    pub core_side: f64,
    /// Layers of the transition from the core square to the circle `r_t`.
    pub transition_layers: usize,
    /// The radius `r_t` where the polar region begins.
    pub transition_radius: f64,
    /// Largest growth ratio from the uniform zone inward to `r_t`.
    pub inner_growth: f64,
    /// Largest growth ratio from the uniform zone outward to `R` and up to `H`.
    pub outer_growth: f64,
}

impl PennyCrack {
    /// The cylinder with the given dimensions and grading.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        radius: f64,
        cylinder_radius: f64,
        half_height: f64,
        h0: f64,
        uniform_layers: usize,
        n_theta: usize,
        core_side: f64,
        transition_layers: usize,
        transition_radius: f64,
        inner_growth: f64,
        outer_growth: f64,
    ) -> PennyCrack {
        PennyCrack {
            radius,
            cylinder_radius,
            half_height,
            h0,
            uniform_layers,
            n_theta,
            core_side,
            transition_layers,
            transition_radius,
            inner_growth,
            outer_growth,
        }
    }
}

/// The mesh of the quarter of the half cylinder with a penny crack.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CrackedCylinder {
    /// The Hex20 mesh (element set `all`, node sets `all`, `ligament`, `x0`, `y0`, surface set `top`).
    pub mesh: SolidMesh,
    /// The front nodes at `z = 0`, radius `a`, ordered by angle from `y = 0` to `x = 0`.
    pub front: Vec<usize>,
}

impl CrackedCylinder {
    /// The front as a [`CrackFront`]: circular about the origin, normal `z`, half-symmetric.
    pub fn crack_front(&self) -> CrackFront {
        CrackFront::circular(self.front.clone(), [0.0, 0.0, 1.0], [0.0; 3]).half_symmetric()
    }
}

/// The quarter `x, y >= 0` of the half `0 <= z <= H` of a cylinder of radius `R` with a penny crack of radius `a` on `z = 0`,
/// Hex20, the circumferential edges of the polar region on their circles (ADR 0547 D8).
pub fn penny_crack_cylinder(spec: &PennyCrack) -> Result<CrackedCylinder> {
    const KEY: &str = "penny crack";
    let PennyCrack {
        radius: a,
        cylinder_radius: big_r,
        half_height: hh,
        h0,
        uniform_layers: n_u,
        n_theta,
        core_side: s,
        transition_layers: n_t,
        transition_radius: r_t,
        inner_growth: g_in,
        outer_growth: g_out,
    } = *spec;
    positive(
        KEY,
        &[
            ("a", a),
            ("R", big_r),
            ("H", hh),
            ("h0", h0),
            ("s", s),
            ("r_t", r_t),
            ("inner growth", g_in),
            ("outer growth", g_out),
        ],
    )?;
    let zone = n_u as f64 * h0;
    if n_u == 0
        || n_theta < 2
        || n_theta % 2 != 0
        || n_t == 0
        || g_in <= 1.0
        || g_out <= 1.0
        || s * 2f64.sqrt() >= r_t
        || r_t >= a - zone
        || a + zone >= big_r
        || zone >= hh
    {
        return Err(invalid(
            KEY,
            format!(
                "needs n_theta even >= 2, layers >= 1, growth > 1 and s sqrt 2 < r_t < a - n_u h0, a + n_u h0 < R, n_u h0 < H \
                 ({spec:?})"
            ),
        ));
    }
    let m = n_theta / 2;
    // Radii of the polar region from r_t.
    let mut radii = vec![r_t];
    let inner = graded(a - zone - r_t, h0, g_in);
    for d in inner.iter().rev().skip(1) {
        radii.push(a - zone - d);
    }
    for i in 0..n_u {
        radii.push(a - (n_u - i) as f64 * h0);
    }
    let k_a = radii.len();
    radii.push(a);
    for i in 1..=n_u {
        radii.push(a + i as f64 * h0);
    }
    for d in graded(big_r - a - zone, h0, g_out) {
        radii.push(a + zone + d);
    }
    *radii.last_mut().expect("nonempty") = big_r;
    let mut zs = vec![0.0];
    for i in 1..=n_u {
        zs.push(i as f64 * h0);
    }
    for d in graded(hh - zone, h0, g_out) {
        zs.push(zone + d);
    }
    *zs.last_mut().expect("nonempty") = hh;
    let angle = |l: usize| -> (f64, f64) {
        if l == 0 {
            (1.0, 0.0)
        } else if l == n_theta {
            (0.0, 1.0)
        } else {
            let th = 0.5 * PI * l as f64 / n_theta as f64;
            (th.cos(), th.sin())
        }
    };
    // 2-D nodes.
    let mut p2: Vec<[f64; 2]> = Vec::new();
    for j in 0..=m {
        for i in 0..=m {
            p2.push([s * i as f64 / m as f64, s * j as f64 / m as f64]);
        }
    }
    let core = |i: usize, j: usize| i + (m + 1) * j;
    let square = |l: usize| if l <= m { core(m, l) } else { core(2 * m - l, m) };
    let n_core = (m + 1) * (m + 1);
    for tau in 1..=n_t {
        let f = tau as f64 / n_t as f64;
        for l in 0..=n_theta {
            let sp = p2[square(l)];
            let (c, sn) = angle(l);
            let cp = [r_t * c, r_t * sn];
            if tau == n_t {
                p2.push(cp);
            } else {
                p2.push([(1.0 - f) * sp[0] + f * cp[0], (1.0 - f) * sp[1] + f * cp[1]]);
            }
        }
    }
    for &rk in &radii[1..] {
        for l in 0..=n_theta {
            let (c, sn) = angle(l);
            p2.push([rk * c, rk * sn]);
        }
    }
    let layers = n_t + radii.len() - 1;
    let ring_node = |layer: usize, l: usize| {
        if layer == 0 {
            square(l)
        } else {
            n_core + (layer - 1) * (n_theta + 1) + l
        }
    };
    let mut quads: Vec<[usize; 4]> = Vec::new();
    for j in 0..m {
        for i in 0..m {
            quads.push([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]);
        }
    }
    for layer in 0..layers {
        for l in 0..n_theta {
            quads.push([
                ring_node(layer, l),
                ring_node(layer + 1, l),
                ring_node(layer + 1, l + 1),
                ring_node(layer, l + 1),
            ]);
        }
    }
    let n2 = p2.len();
    let nz = zs.len() - 1;
    let mut nodes = Vec::with_capacity(n2 * (nz + 1));
    for &z in &zs {
        for p in &p2 {
            nodes.push([p[0], p[1], z]);
        }
    }
    let id = |n: usize, k: usize| n + n2 * k;
    let mut conn = Vec::with_capacity(8 * quads.len() * nz);
    for k in 0..nz {
        for q in &quads {
            conn.extend_from_slice(&[
                id(q[0], k),
                id(q[1], k),
                id(q[2], k),
                id(q[3], k),
                id(q[0], k + 1),
                id(q[1], k + 1),
                id(q[2], k + 1),
                id(q[3], k + 1),
            ]);
        }
    }
    let mut mesh = SolidMesh::new(nodes)?;
    mesh.add_block(ElementKind::Hex8, conn)?;
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect())?;
    mesh.add_node_set("all", (0..mesh.nodes().len()).collect())?;
    // Polar layer index of the front: ring layer n_t + k_a.
    let front_layer = n_t + k_a;
    let mut ligament = Vec::new();
    for layer in front_layer..=layers {
        for l in 0..=n_theta {
            ligament.push(id(ring_node(layer, l), 0));
        }
    }
    mesh.add_node_set("ligament", ligament)?;
    let mut x0 = Vec::new();
    let mut y0 = Vec::new();
    for k in 0..=nz {
        for (n, p) in p2.iter().enumerate() {
            if p[0] == 0.0 {
                x0.push(id(n, k));
            }
            if p[1] == 0.0 {
                y0.push(id(n, k));
            }
        }
    }
    mesh.add_node_set("x0", x0)?;
    mesh.add_node_set("y0", y0)?;
    let nq = quads.len();
    mesh.add_surface_set(
        "top",
        (0..nq)
            .map(|q| FaceRef {
                element: q + nq * (nz - 1),
                face: 1,
            })
            .collect(),
    )?;
    let (mesh, mids) = upgrade_to_hex20(&mesh)?;
    // The circumferential mid-edge nodes onto their circles.
    let mut moved = mesh.nodes().to_vec();
    let rho = |p: Vec3| (p[0] * p[0] + p[1] * p[1]).sqrt();
    for mnode in &mids {
        let (pa, pb) = (moved[mnode.ends[0]], moved[mnode.ends[1]]);
        let (ra, rb) = (rho(pa), rho(pb));
        if ra >= r_t * (1.0 - 1e-12) && (ra - rb).abs() <= 1e-12 * ra {
            let p = moved[mnode.node];
            let rp = rho(p);
            if rp > 0.0 {
                let f = ra / rp;
                moved[mnode.node] = [p[0] * f, p[1] * f, p[2]];
            }
        }
    }
    let mut out = SolidMesh::new(moved)?;
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec())?;
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).expect("listed").to_vec())?;
    }
    let mid: HashMap<(usize, usize), usize> = mids
        .iter()
        .map(|m| ((m.ends[0].min(m.ends[1]), m.ends[0].max(m.ends[1])), m.node))
        .collect();
    let mut front = Vec::with_capacity(2 * n_theta + 1);
    for l in 0..=n_theta {
        let c = id(ring_node(front_layer, l), 0);
        if l > 0 {
            let p = id(ring_node(front_layer, l - 1), 0);
            front.push(
                *mid.get(&(p.min(c), p.max(c)))
                    .ok_or_else(|| invalid(KEY, format!("no mid-edge node between {p} and {c}")))?,
            );
        }
        front.push(c);
    }
    Ok(CrackedCylinder { mesh: out, front })
}

/// The embedded elliptical crack of ADR 0605 D3: the penny mesh of [`PennyCrack`] (radius `a`, its other parameters in the
/// polar coordinates of that mesh) mapped in the plane onto a crack of semi-axes `a` (along `x`) and `b < a` (along `y`).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EllipticalCrack {
    /// The penny mesh to map (its `radius` is the major semi-axis `a`).
    pub penny: PennyCrack,
    /// The minor semi-axis `b`.
    pub semi_minor: f64,
}

impl EllipticalCrack {
    /// The crack with minor semi-axis `semi_minor` on the penny mesh `penny`.
    pub fn new(penny: PennyCrack, semi_minor: f64) -> EllipticalCrack {
        EllipticalCrack { penny, semi_minor }
    }
}

/// The mesh of the quarter of the half block with an embedded elliptical crack.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CrackedEllipse {
    /// The Hex20 mesh (the sets of [`CrackedCylinder`]: `all`, `ligament`, `x0`, `y0`, surface set `top`).
    pub mesh: SolidMesh,
    /// The front nodes at `z = 0` ordered by the elliptic angle from `y = 0` to `x = 0`.
    pub front: Vec<usize>,
    /// The major semi-axis `a`.
    pub a: f64,
    /// The minor semi-axis `b`.
    pub b: f64,
}

impl CrackedEllipse {
    /// The front as a [`CrackFront`]: elliptical about the origin, normal `z`, major axis `x`, half-symmetric.
    pub fn crack_front(&self) -> CrackFront {
        CrackFront::elliptical(
            self.front.clone(),
            [0.0, 0.0, 1.0],
            [0.0; 3],
            [1.0, 0.0, 0.0],
            self.a,
            self.b,
        )
        .half_symmetric()
    }
}

/// The quarter `x, y >= 0` of the half `z >= 0` of a block with an elliptical crack of semi-axes `a` (x) and `b` (y) on `z =
/// 0` (ADR 0605 D3): the penny mesh with every node mapped in the plane, `rho >= r_t` by the confocal map `mu = mu0 + ln(rho/a)`,
/// `(x, y) = (c cosh mu X/rho, c sinh mu Y/rho)`, the core `rho < r_t` by the affine map onto the ellipse `mu_t`. Refuses `b >=
/// a`, `mu_t <= 0` and what [`penny_crack_cylinder`] refuses.
pub fn elliptical_crack_block(spec: &EllipticalCrack) -> Result<CrackedEllipse> {
    const KEY: &str = "elliptical crack";
    let a = spec.penny.radius;
    let b = spec.semi_minor;
    let r_t = spec.penny.transition_radius;
    if !(a.is_finite() && b.is_finite() && b > 0.0 && b < a) {
        return Err(invalid(KEY, format!("semi-axes a = {a}, b = {b}: 0 < b < a")));
    }
    let c = (a * a - b * b).sqrt();
    let mu0 = (b / a).atanh();
    let mu_t = mu0 + (r_t / a).ln();
    if mu_t.is_nan() || mu_t <= 0.0 {
        return Err(invalid(
            KEY,
            format!(
                "the transition radius r_t = {r_t} maps inside the focal segment (mu_t = {mu_t:.4e}): r_t > a c/(a + b) = {:.6e}",
                a * c / (a + b)
            ),
        ));
    }
    let cyl = penny_crack_cylinder(&spec.penny)?;
    let (at, bt) = (c * mu_t.cosh(), c * mu_t.sinh());
    let mapped: Vec<Vec3> = cyl
        .mesh
        .nodes()
        .iter()
        .map(|p| {
            let rho = (p[0] * p[0] + p[1] * p[1]).sqrt();
            if rho >= r_t * (1.0 - 1e-12) {
                let mu = mu0 + (rho / a).ln();
                [c * mu.cosh() * p[0] / rho, c * mu.sinh() * p[1] / rho, p[2]]
            } else {
                [p[0] * at / r_t, p[1] * bt / r_t, p[2]]
            }
        })
        .collect();
    let mesh = &cyl.mesh;
    let mut out = SolidMesh::new(mapped)?;
    for blk in mesh.blocks() {
        out.add_block(blk.kind(), blk.connectivity().to_vec())?;
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).expect("listed").to_vec())?;
    }
    Ok(CrackedEllipse {
        mesh: out,
        front: cyl.front,
        a,
        b,
    })
}

// ------------------------------------------------------------------------------------------ curved interaction integral

/// A smooth plane front curve for the curved interaction integral (ADR 0610 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum FrontCurve {
    /// A straight line through `point`, the advance direction `advance` in the crack plane of normal `normal`.
    Line {
        /// A point of the line (in the crack plane).
        point: Vec3,
        /// The advance direction (made normal to `normal` and unit).
        advance: Vec3,
        /// The crack-plane normal.
        normal: Vec3,
    },
    /// A circle of radius `radius` about `centre` (a point of the crack plane), the advance direction radial outward.
    Circle {
        /// The centre, in the crack plane.
        centre: Vec3,
        /// The crack-plane normal.
        normal: Vec3,
        /// The radius.
        radius: f64,
    },
    /// An ellipse `X^2/a^2 + Y^2/b^2 = 1` about `centre` (in the crack plane), `X` along `major_axis`, `a > b`.
    Ellipse {
        /// The centre, in the crack plane.
        centre: Vec3,
        /// The crack-plane normal.
        normal: Vec3,
        /// The major axis (in the plane).
        major_axis: Vec3,
        /// The major semi-axis.
        a: f64,
        /// The minor semi-axis.
        b: f64,
    },
}

/// The near-tip frame of a point (ADR 0610 D1): the foot on the curve (the nearest point in the plane), the basis `(N, n, N
/// x n)` there (`N` the in-plane outward normal), the local coordinates `x1 = (x - foot) . N`, `x2` the height above the
/// plane, and the curvature `kappa` of the curve at the foot.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TipFrame {
    /// The foot on the curve.
    pub foot: Vec3,
    /// `[N, n, N x n]`.
    pub basis: [Vec3; 3],
    /// The in-plane distance from the foot along `N`.
    pub x1: f64,
    /// The height above the crack plane along `n`.
    pub x2: f64,
    /// The curvature of the curve at the foot (0 for a line).
    pub curvature: f64,
}

/// The near-tip frame of `x` for `curve` (ADR 0610 D1). The foot of an ellipse, `(a cos t, b sin t)`, solves `(a^2 - b^2) sin
/// t cos t - a X sin t + b Y cos t = 0` by Newton iterations from `atan2(a Y, b X)`. Refuses (key `"crack front"`) an invalid
/// curve, a point on the axis of a circle, no convergence of the foot and a point at or beyond the centre of curvature (`1 +
/// kappa x1 <= 0`).
pub fn tip_frame(curve: &FrontCurve, x: Vec3) -> Result<TipFrame> {
    const KEY: &str = "crack front";
    let in_plane = |v: Vec3, n: Vec3, what: &str| unit_of(sub(v, scaled(n, dot(v, n))), KEY, what);
    let frame = match *curve {
        FrontCurve::Line { point, advance, normal } => {
            let n = unit_of(normal, KEY, "the crack-plane normal")?;
            let e = in_plane(advance, n, "the advance direction")?;
            let d = sub(x, point);
            let (x1, x2) = (dot(d, e), dot(d, n));
            TipFrame {
                foot: sub(sub(x, scaled(e, x1)), scaled(n, x2)),
                basis: [e, n, cross(e, n)],
                x1,
                x2,
                curvature: 0.0,
            }
        }
        FrontCurve::Circle { centre, normal, radius } => {
            let n = unit_of(normal, KEY, "the crack-plane normal")?;
            if !(radius.is_finite() && radius > 0.0) {
                return Err(invalid(KEY, format!("the radius {radius} must be finite and positive")));
            }
            let d = sub(x, centre);
            let x2 = dot(d, n);
            let p = sub(d, scaled(n, x2));
            let rho = norm(p);
            if !(rho > 0.0 && rho.is_finite()) {
                return Err(invalid(
                    KEY,
                    format!("the point {x:?} is on the axis of the circular front"),
                ));
            }
            let big_n = scaled(p, 1.0 / rho);
            TipFrame {
                foot: [
                    centre[0] + radius * big_n[0],
                    centre[1] + radius * big_n[1],
                    centre[2] + radius * big_n[2],
                ],
                basis: [big_n, n, cross(big_n, n)],
                x1: rho - radius,
                x2,
                curvature: 1.0 / radius,
            }
        }
        FrontCurve::Ellipse {
            centre,
            normal,
            major_axis,
            a,
            b,
        } => {
            let n = unit_of(normal, KEY, "the crack-plane normal")?;
            if !(a.is_finite() && b.is_finite() && b > 0.0 && a > b) {
                return Err(invalid(
                    KEY,
                    format!("the semi-axes a = {a}, b = {b}: an ellipse needs a > b > 0"),
                ));
            }
            let u = in_plane(major_axis, n, "the major axis")?;
            let v = cross(n, u);
            let d = sub(x, centre);
            let (xx, yy, x2) = (dot(d, u), dot(d, v), dot(d, n));
            let c2 = a * a - b * b;
            let mut t = (a * yy).atan2(b * xx);
            let mut converged = false;
            for _ in 0..60 {
                let (st, ct) = t.sin_cos();
                let f = c2 * st * ct - a * xx * st + b * yy * ct;
                let df = c2 * (ct * ct - st * st) - a * xx * ct - b * yy * st;
                let dt = f / df;
                if !dt.is_finite() {
                    break;
                }
                t -= dt;
                if dt.abs() <= 1e-14 {
                    converged = true;
                    break;
                }
            }
            if !converged {
                return Err(invalid(
                    KEY,
                    format!("the foot of {x:?} on the elliptical front did not converge"),
                ));
            }
            let (st, ct) = t.sin_cos();
            let (foot_x, foot_y) = (a * ct, b * st);
            let big_n = unit_of(
                [
                    b * ct * u[0] + a * st * v[0],
                    b * ct * u[1] + a * st * v[1],
                    b * ct * u[2] + a * st * v[2],
                ],
                KEY,
                "the normal of the ellipse",
            )?;
            let foot = [
                centre[0] + foot_x * u[0] + foot_y * v[0],
                centre[1] + foot_x * u[1] + foot_y * v[1],
                centre[2] + foot_x * u[2] + foot_y * v[2],
            ];
            let x1 = (xx - foot_x) * dot(big_n, u) + (yy - foot_y) * dot(big_n, v);
            let curvature = a * b / (a * a * st * st + b * b * ct * ct).powf(1.5);
            TipFrame {
                foot,
                basis: [big_n, n, cross(big_n, n)],
                x1,
                x2,
                curvature,
            }
        }
    };
    let h = 1.0 + frame.curvature * frame.x1;
    if h.is_nan() || h <= 0.0 {
        return Err(invalid(
            KEY,
            format!("the point {x:?} is at or beyond the centre of curvature of the front (1 + kappa x1 = {h:.3e})"),
        ));
    }
    Ok(frame)
}

/// The auxiliary field of one mode at a point near a curved front (ADR 0610 D1): the Williams gradient `G` of
/// [`near_tip_gradient`] in the local coordinates of the [`TipFrame`], rotated into its basis (`H = R G R^T`), and its stress
/// (`sigma^aux = C sym H`), global and local.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CurvedAuxiliary {
    /// The frame of the point.
    pub frame: TipFrame,
    /// `H[i][k]`, the auxiliary displacement gradient in global components.
    pub grad: [[f64; 3]; 3],
    /// The auxiliary stress in global components.
    pub stress: [[f64; 3]; 3],
    /// `G[a][b]` in the local basis.
    pub local_grad: [[f64; 3]; 3],
    /// The auxiliary stress in the local basis.
    pub local_stress: [[f64; 3]; 3],
}

/// The auxiliary field of `mode` (unit stress intensity factor) at `x` for `curve`, shear modulus `mu`, Poisson's ratio `nu`
/// (ADR 0610 D1). Refuses what [`tip_frame`] refuses and a point on the front (`r = 0`).
pub fn curved_auxiliary(curve: &FrontCurve, mode: TipMode, x: Vec3, mu: f64, nu: f64) -> Result<CurvedAuxiliary> {
    let frame = tip_frame(curve, x)?;
    let lambda = 2.0 * mu * nu / (1.0 - 2.0 * nu);
    curved_auxiliary_in(&frame, mode, lambda, mu, nu)
}

fn curved_auxiliary_in(frame: &TipFrame, mode: TipMode, lambda: f64, mu: f64, nu: f64) -> Result<CurvedAuxiliary> {
    let (x1, x2) = (frame.x1, frame.x2);
    let r = (x1 * x1 + x2 * x2).sqrt();
    if r.is_nan() || r <= 0.0 {
        return Err(invalid("crack front", "an integration point lies on the front line"));
    }
    let theta = x2.atan2(x1);
    let (_, local) = near_tip_gradient(mode, r, theta, mu, nu);
    let basis = &frame.basis;
    let mut ga = [[0.0; 3]; 3];
    for i in 0..3 {
        for k in 0..3 {
            let mut v = 0.0;
            for (a, ba) in basis.iter().enumerate() {
                for (b, bb) in basis.iter().enumerate() {
                    v += ba[i] * local[a][b] * bb[k];
                }
            }
            ga[i][k] = v;
        }
    }
    Ok(CurvedAuxiliary {
        frame: *frame,
        grad: ga,
        stress: hooke(&ga, lambda, mu),
        local_grad: local,
        local_stress: hooke(&local, lambda, mu),
    })
}

/// `R^T A R` for the basis `R = [b_1 | b_2 | b_3]`.
fn to_local(basis: &[Vec3; 3], m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for a in 0..3 {
        for b in 0..3 {
            let mut v = 0.0;
            for i in 0..3 {
                for k in 0..3 {
                    v += basis[a][i] * m[i][k] * basis[b][k];
                }
            }
            out[a][b] = v;
        }
    }
    out
}

impl CurvedAuxiliary {
    /// The correction integrand `P_jk,j q_k = sigma^aux_ij,j u_i,k q_k + sigma_ij (H_ik,j - eps^aux_ij,k) q_k` at the point,
    /// for an actual field in equilibrium with stress `sigma` and displacement gradient `grad_u[i][k] = du_i/dx_k` there and
    /// the vector `q` (ADR 0610 D1: C1 + C2). Zero on a straight front.
    pub fn correction(&self, sigma: &[[f64; 3]; 3], grad_u: &[[f64; 3]; 3], q: Vec3) -> f64 {
        let kappa = self.frame.curvature;
        if kappa == 0.0 {
            return 0.0;
        }
        let basis = &self.frame.basis;
        let h = 1.0 + kappa * self.frame.x1;
        let s = to_local(basis, sigma);
        let g_u = to_local(basis, grad_u);
        let ql = [dot(q, basis[0]), dot(q, basis[1]), dot(q, basis[2])];
        let g = &self.local_grad;
        let sa = &self.local_stress;
        // C1: div sigma^aux = (kappa/h) (sa_11 - sa_33, sa_21, 2 sa_13) against (grad u) q.
        let d = [sa[0][0] - sa[2][2], sa[1][0], 2.0 * sa[0][2]];
        let mut c1 = 0.0;
        for a in 0..3 {
            c1 += d[a] * (g_u[a][0] * ql[0] + g_u[a][1] * ql[1] + g_u[a][2] * ql[2]);
        }
        c1 *= kappa / h;
        // C2, first part: -(kappa/h) [s_13 (G q)_3 - s_33 (G q)_1 - (s_13 G_11 + s_23 G_21 + s_33 G_31) q_3].
        let gq = |a: usize| g[a][0] * ql[0] + g[a][1] * ql[1] + g[a][2] * ql[2];
        let c2a = -(kappa / h)
            * (s[0][2] * gq(2) - s[2][2] * gq(0) - (s[0][2] * g[0][0] + s[1][2] * g[1][0] + s[2][2] * g[2][0]) * ql[2]);
        // C2, second part: (q_3/h) sum_ab (s Omega - Omega s)_ab eps^aux_ba, Omega_13 = kappa, Omega_31 = -kappa.
        let mut omega = [[0.0; 3]; 3];
        omega[0][2] = kappa;
        omega[2][0] = -kappa;
        let mut c2b = 0.0;
        for a in 0..3 {
            for b in 0..3 {
                let mut comm = 0.0;
                for c in 0..3 {
                    comm += s[a][c] * omega[c][b] - omega[a][c] * s[c][b];
                }
                c2b += comm * 0.5 * (g[b][a] + g[a][b]);
            }
        }
        c2b *= ql[2] / h;
        c1 + c2a + c2b
    }
}

/// The terms of [`interaction_integral_curved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CurvedTerms {
    /// The domain integral and the correction terms of the auxiliary fields (ADR 0610 D1).
    Corrected,
    /// The domain integral only: not exact on a curved front (for comparison).
    DomainOnly,
}

/// `K_I`, `K_II` and `K_III` at each station of a straight, circular or elliptical `front` by the interaction integral over
/// ring `ring`, the auxiliary fields in the frame of each point's foot on the front (ADR 0610 D1): `M = int P_jk q_k,j dV + int
/// P_jk,j q_k dV` (the second integral only with [`CurvedTerms::Corrected`]) divided by `int phi_j ds`; `K_I = E' M_I/2`,
/// `K_II = E' M_II/2`, `K_III = mu M_III`, with `E, nu` of the element of the first front node. In a half-symmetric model
/// `K_I` is doubled and `K_II = K_III = 0` (the antisymmetric modes vanish by symmetry). Refuses what [`j_integral`] refuses, a
/// ring element of another `E, nu` and an integration point that [`tip_frame`] refuses.
pub fn interaction_integral_curved(
    model: &SolidModel,
    displacement: &[f64],
    front: &CrackFront,
    ring: usize,
    terms: CurvedTerms,
) -> Result<Vec<ModeFactors>> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    check_loads(model)?;
    let mesh = model.mesh();
    let geometry = front.check(mesh)?;
    let n = geometry.n;
    let x0g = geometry.x0;
    let in_plane = |c: Vec3| {
        let off = dot(sub(x0g, c), n);
        [c[0] + off * n[0], c[1] + off * n[1], c[2] + off * n[2]]
    };
    let curve = match &geometry.kind {
        FrontKind::Straight { x0, e, .. } => FrontCurve::Line {
            point: *x0,
            advance: *e,
            normal: n,
        },
        FrontKind::Circular { centre, radius, .. } => FrontCurve::Circle {
            centre: in_plane(*centre),
            normal: n,
            radius: *radius,
        },
        FrontKind::Elliptical { centre, u, a, b, .. } => FrontCurve::Ellipse {
            centre: in_plane(*centre),
            normal: n,
            major_axis: *u,
            a: *a,
            b: *b,
        },
    };
    let adjacency = node_elements(mesh);
    let (elements, rho) = rings(mesh, &adjacency, &front.nodes, ring)?;
    check_boundary(mesh, &geometry, &rho, ring)?;
    let materials = model.material_indices()?;
    let first = front.nodes[0];
    let Some(&e0) = adjacency[first].first() else {
        return Err(invalid(
            "crack front",
            format!("the front node {first} belongs to no element"),
        ));
    };
    let (young0, nu0) = element_elastic(model, &materials, e0)?;
    let n_st = geometry.stations.len();
    let mut nodal: HashMap<usize, (Vec3, f64)> = HashMap::new();
    for el in &elements {
        for &a in mesh.element_nodes(*el) {
            if rho[a] && !nodal.contains_key(&a) {
                let x = mesh.nodes()[a];
                nodal.insert(a, (geometry.advance(x)?, geometry.param(x)));
            }
        }
    }
    let modes = [TipMode::I, TipMode::II, TipMode::III];
    let mut m_bar = vec![[0.0; 3]; n_st];
    for &el in &elements {
        let (young, nu) = element_elastic(model, &materials, el)?;
        if young != young0 || nu != nu0 {
            return Err(invalid(
                "element",
                format!(
                    "element {el} has E = {young}, nu = {nu}, the element {e0} of the first front node E = {young0}, nu = \
                     {nu0}: the interaction integral takes one material in the domain"
                ),
            ));
        }
        let lambda = young * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let mu = young / (2.0 * (1.0 + nu));
        let kind = mesh.element_kind(el);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(el);
        let coords: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
        let mut ue: Vec<Vec3> = Vec::with_capacity(nodes.len());
        for &a in nodes {
            let mut u = [0.0; 3];
            for (c, uc) in u.iter_mut().enumerate() {
                let q = dofs.equation(a, c).ok_or(FemError::MissingComponent {
                    role: "displacement",
                    node: a,
                    component: c,
                    available: dofs.components(a),
                })?;
                *uc = displacement[q];
            }
            ue.push(u);
        }
        let mut qa: Vec<Vec<Vec3>> = vec![vec![[0.0; 3]; nodes.len()]; n_st];
        for (la, &a) in nodes.iter().enumerate() {
            if let Some(&(dir, s)) = nodal.get(&a) {
                for (j, qj) in qa.iter_mut().enumerate() {
                    qj[la] = scaled(dir, geometry.hat(j, s));
                }
            }
        }
        let points = element
            .integration_points(&coords, element.stiffness_rule())
            .map_err(|source| FemError::Element {
                element: el,
                source: Box::new(source),
            })?;
        for ip in points {
            let grad = &ip.geometry.grad;
            let shape = &ip.geometry.n;
            let mut du = [[0.0; 3]; 3];
            for (a, g) in grad.iter().enumerate() {
                for i in 0..3 {
                    for k in 0..3 {
                        du[i][k] += ue[a][i] * g[k];
                    }
                }
            }
            let sigma = hooke(&du, lambda, mu);
            // Per station: dq[k][j] = dq_k/dx_j and q at the point.
            let mut dq: Vec<Option<([[f64; 3]; 3], Vec3)>> = Vec::with_capacity(n_st);
            for qj in &qa {
                let mut d = [[0.0; 3]; 3];
                let mut qp = [0.0; 3];
                let mut any = false;
                for (a, g) in grad.iter().enumerate() {
                    let q = qj[a];
                    if q == [0.0; 3] {
                        continue;
                    }
                    any = true;
                    for k in 0..3 {
                        qp[k] += shape[a] * q[k];
                        for j in 0..3 {
                            d[k][j] += q[k] * g[j];
                        }
                    }
                }
                dq.push(any.then_some((d, qp)));
            }
            if dq.iter().all(Option::is_none) {
                continue;
            }
            let frame = tip_frame(&curve, ip.geometry.x)?;
            for (m, &mode) in modes.iter().enumerate() {
                let aux = curved_auxiliary_in(&frame, mode, lambda, mu, nu)?;
                let ga = &aux.grad;
                let sigma_aux = &aux.stress;
                let mut w_int = 0.0;
                for i in 0..3 {
                    for k in 0..3 {
                        w_int += sigma[i][k] * 0.5 * (ga[i][k] + ga[k][i]);
                    }
                }
                let mut p = [[0.0; 3]; 3];
                for j in 0..3 {
                    for k in 0..3 {
                        let mut v = 0.0;
                        for i in 0..3 {
                            v += sigma[i][j] * ga[i][k] + sigma_aux[i][j] * du[i][k];
                        }
                        p[j][k] = v - if j == k { w_int } else { 0.0 };
                    }
                }
                for (st, entry) in dq.iter().enumerate() {
                    let Some((d, qp)) = entry else { continue };
                    let mut v = 0.0;
                    for j in 0..3 {
                        for k in 0..3 {
                            v += p[j][k] * d[k][j];
                        }
                    }
                    if terms == CurvedTerms::Corrected {
                        v += aux.correction(&sigma, &du, *qp);
                    }
                    m_bar[st][m] += ip.weight * v;
                }
            }
        }
    }
    let e_prime = young0 / (1.0 - nu0 * nu0);
    let mu0 = young0 / (2.0 * (1.0 + nu0));
    let half = front.half_symmetric;
    Ok((0..n_st)
        .map(|j| {
            let node = front.nodes[2 * j];
            let weight = geometry.hat_integral(j);
            let m = m_bar[j];
            let (k_i, k_ii, k_iii) = if half {
                (e_prime * m[0] / weight, 0.0, 0.0)
            } else {
                (
                    0.5 * e_prime * m[0] / weight,
                    0.5 * e_prime * m[1] / weight,
                    mu0 * m[2] / weight,
                )
            };
            ModeFactors {
                station: j,
                node,
                position: mesh.nodes()[node],
                s: geometry.stations[j],
                k_i,
                k_ii,
                k_iii,
                weight,
            }
        })
        .collect())
}

/// The mesh of the half `x >= 0` of a whole cylinder (both sides of the crack) with a penny crack (ADR 0610 D4).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CrackedHalfCylinder {
    /// The Hex20 mesh (element set `all`; node sets `all`, `x0`, `top_centre`, `bottom_centre`; surface sets `top`, `bottom`,
    /// `side`).
    pub mesh: SolidMesh,
    /// The front nodes at `z = 0`, radius `a`, ordered by angle from `-pi/2` (at `(0, -a, 0)`) to `pi/2`.
    pub front: Vec<usize>,
}

impl CrackedHalfCylinder {
    /// The front as a [`CrackFront`]: circular about the origin, normal `z` (a full model on both sides of the crack).
    pub fn crack_front(&self) -> CrackFront {
        CrackFront::circular(self.front.clone(), [0.0, 0.0, 1.0], [0.0; 3])
    }
}

/// The half `x >= 0` of the whole cylinder `-H <= z <= H` of radius `R` with a penny crack of radius `a` on `z = 0` (ADR 0610
/// D4): the plane mesh of [`penny_crack_cylinder`] (the same parameters) reflected in `y`, extruded over both sides of the
/// crack plane, the nodes of `z = 0` inside the crack doubled (the lower face), Hex20 with the circumferential edges of the
/// polar region on their circles. Refuses what [`penny_crack_cylinder`] refuses.
pub fn penny_crack_half_cylinder(spec: &PennyCrack) -> Result<CrackedHalfCylinder> {
    const KEY: &str = "penny crack";
    let PennyCrack {
        radius: a,
        cylinder_radius: big_r,
        half_height: hh,
        h0,
        uniform_layers: n_u,
        n_theta,
        core_side: s,
        transition_layers: n_t,
        transition_radius: r_t,
        inner_growth: g_in,
        outer_growth: g_out,
    } = *spec;
    positive(
        KEY,
        &[
            ("a", a),
            ("R", big_r),
            ("H", hh),
            ("h0", h0),
            ("s", s),
            ("r_t", r_t),
            ("inner growth", g_in),
            ("outer growth", g_out),
        ],
    )?;
    let zone = n_u as f64 * h0;
    if n_u == 0
        || n_theta < 2
        || n_theta % 2 != 0
        || n_t == 0
        || g_in <= 1.0
        || g_out <= 1.0
        || s * 2f64.sqrt() >= r_t
        || r_t >= a - zone
        || a + zone >= big_r
        || zone >= hh
    {
        return Err(invalid(
            KEY,
            format!(
                "needs n_theta even >= 2, layers >= 1, growth > 1 and s sqrt 2 < r_t < a - n_u h0, a + n_u h0 < R, n_u h0 < H \
                 ({spec:?})"
            ),
        ));
    }
    // The plane quarter of ADR 0547 D8 (as in penny_crack_cylinder).
    let m = n_theta / 2;
    let mut radii = vec![r_t];
    let inner = graded(a - zone - r_t, h0, g_in);
    for d in inner.iter().rev().skip(1) {
        radii.push(a - zone - d);
    }
    for i in 0..n_u {
        radii.push(a - (n_u - i) as f64 * h0);
    }
    let k_a = radii.len();
    radii.push(a);
    for i in 1..=n_u {
        radii.push(a + i as f64 * h0);
    }
    for d in graded(big_r - a - zone, h0, g_out) {
        radii.push(a + zone + d);
    }
    *radii.last_mut().expect("nonempty") = big_r;
    let mut zs = vec![0.0];
    for i in 1..=n_u {
        zs.push(i as f64 * h0);
    }
    for d in graded(hh - zone, h0, g_out) {
        zs.push(zone + d);
    }
    *zs.last_mut().expect("nonempty") = hh;
    let angle = |l: usize| -> (f64, f64) {
        if l == 0 {
            (1.0, 0.0)
        } else if l == n_theta {
            (0.0, 1.0)
        } else {
            let th = 0.5 * PI * l as f64 / n_theta as f64;
            (th.cos(), th.sin())
        }
    };
    let mut p2: Vec<[f64; 2]> = Vec::new();
    for j in 0..=m {
        for i in 0..=m {
            p2.push([s * i as f64 / m as f64, s * j as f64 / m as f64]);
        }
    }
    let core = |i: usize, j: usize| i + (m + 1) * j;
    let square = |l: usize| if l <= m { core(m, l) } else { core(2 * m - l, m) };
    let n_core = (m + 1) * (m + 1);
    for tau in 1..=n_t {
        let f = tau as f64 / n_t as f64;
        for l in 0..=n_theta {
            let sp = p2[square(l)];
            let (c, sn) = angle(l);
            let cp = [r_t * c, r_t * sn];
            if tau == n_t {
                p2.push(cp);
            } else {
                p2.push([(1.0 - f) * sp[0] + f * cp[0], (1.0 - f) * sp[1] + f * cp[1]]);
            }
        }
    }
    for &rk in &radii[1..] {
        for l in 0..=n_theta {
            let (c, sn) = angle(l);
            p2.push([rk * c, rk * sn]);
        }
    }
    let layers = n_t + radii.len() - 1;
    let ring_node = |layer: usize, l: usize| {
        if layer == 0 {
            square(l)
        } else {
            n_core + (layer - 1) * (n_theta + 1) + l
        }
    };
    let mut quads: Vec<[usize; 4]> = Vec::new();
    for j in 0..m {
        for i in 0..m {
            quads.push([core(i, j), core(i + 1, j), core(i + 1, j + 1), core(i, j + 1)]);
        }
    }
    for layer in 0..layers {
        for l in 0..n_theta {
            quads.push([
                ring_node(layer, l),
                ring_node(layer + 1, l),
                ring_node(layer + 1, l + 1),
                ring_node(layer, l + 1),
            ]);
        }
    }
    // Reflected in y: the nodes on y = 0 shared, the reflected quads reversed (counter-clockwise again).
    let mut refl = vec![0usize; p2.len()];
    let mut ph = p2.clone();
    for (i, p) in p2.iter().enumerate() {
        if p[1] == 0.0 {
            refl[i] = i;
        } else {
            refl[i] = ph.len();
            ph.push([p[0], -p[1]]);
        }
    }
    let mut qh = quads.clone();
    for q in &quads {
        qh.push([refl[q[0]], refl[q[3]], refl[q[2]], refl[q[1]]]);
    }
    // Levels -H .. 0 .. H; the crack plane is level nz.
    let nz = zs.len() - 1;
    let mut levels: Vec<f64> = zs[1..].iter().rev().map(|z| -z).collect();
    levels.extend_from_slice(&zs);
    let n2 = ph.len();
    let crack: Vec<bool> = ph
        .iter()
        .map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt() < a * (1.0 - 1e-9))
        .collect();
    let mut nodes = Vec::with_capacity(n2 * levels.len() + n2);
    for &z in &levels {
        for p in &ph {
            nodes.push([p[0], p[1], z]);
        }
    }
    let mut lower = vec![usize::MAX; n2];
    for (i, p) in ph.iter().enumerate() {
        if crack[i] {
            lower[i] = nodes.len();
            nodes.push([p[0], p[1], 0.0]);
        }
    }
    let id = |n: usize, k: usize| n + n2 * k;
    let mut conn = Vec::with_capacity(8 * qh.len() * 2 * nz);
    for k in 0..2 * nz {
        let top = |n: usize| {
            if k + 1 == nz && crack[n] {
                lower[n]
            } else {
                id(n, k + 1)
            }
        };
        for q in &qh {
            conn.extend_from_slice(&[
                id(q[0], k),
                id(q[1], k),
                id(q[2], k),
                id(q[3], k),
                top(q[0]),
                top(q[1]),
                top(q[2]),
                top(q[3]),
            ]);
        }
    }
    let mut mesh = SolidMesh::new(nodes)?;
    mesh.add_block(ElementKind::Hex8, conn)?;
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect())?;
    mesh.add_node_set("all", (0..mesh.nodes().len()).collect())?;
    let x0: Vec<usize> = (0..mesh.nodes().len()).filter(|&i| mesh.nodes()[i][0] == 0.0).collect();
    mesh.add_node_set("x0", x0)?;
    mesh.add_node_set("top_centre", vec![id(core(0, 0), 2 * nz)])?;
    mesh.add_node_set("bottom_centre", vec![id(core(0, 0), 0)])?;
    let (mut top, mut bottom, mut side) = (Vec::new(), Vec::new(), Vec::new());
    for (_, faces) in mesh.face_incidence() {
        if faces.len() != 1 {
            continue;
        }
        let f = faces[0];
        let pts: Vec<Vec3> = mesh.face_nodes(f).iter().map(|&i| mesh.nodes()[i]).collect();
        if pts.iter().all(|p| p[2] >= hh * (1.0 - 1e-12)) {
            top.push(f);
        } else if pts.iter().all(|p| p[2] <= -hh * (1.0 - 1e-12)) {
            bottom.push(f);
        } else if pts
            .iter()
            .all(|p| (p[0] * p[0] + p[1] * p[1]).sqrt() >= big_r * (1.0 - 1e-12))
        {
            side.push(f);
        }
    }
    mesh.add_surface_set("top", top)?;
    mesh.add_surface_set("bottom", bottom)?;
    mesh.add_surface_set("side", side)?;
    let (mesh, mids) = upgrade_to_hex20(&mesh)?;
    // The circumferential mid-edge nodes onto their circles (as penny_crack_cylinder).
    let mut moved = mesh.nodes().to_vec();
    let rho = |p: Vec3| (p[0] * p[0] + p[1] * p[1]).sqrt();
    for mnode in &mids {
        let (pa, pb) = (moved[mnode.ends[0]], moved[mnode.ends[1]]);
        let (ra, rb) = (rho(pa), rho(pb));
        if ra >= r_t * (1.0 - 1e-12) && (ra - rb).abs() <= 1e-12 * ra {
            let p = moved[mnode.node];
            let rp = rho(p);
            if rp > 0.0 {
                let f = ra / rp;
                moved[mnode.node] = [p[0] * f, p[1] * f, p[2]];
            }
        }
    }
    let mut out = SolidMesh::new(moved)?;
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec())?;
    }
    for name in mesh.node_set_names() {
        out.add_node_set(name, mesh.node_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.element_set_names() {
        out.add_element_set(name, mesh.element_set(name).expect("listed").to_vec())?;
    }
    for name in mesh.surface_set_names() {
        out.add_surface_set(name, mesh.surface_set(name).expect("listed").to_vec())?;
    }
    let mid: HashMap<(usize, usize), usize> = mids
        .iter()
        .map(|m| ((m.ends[0].min(m.ends[1]), m.ends[0].max(m.ends[1])), m.node))
        .collect();
    let front_layer = n_t + k_a;
    let mut corners: Vec<usize> = (1..=n_theta)
        .rev()
        .map(|l| id(refl[ring_node(front_layer, l)], nz))
        .collect();
    corners.extend((0..=n_theta).map(|l| id(ring_node(front_layer, l), nz)));
    let mut front = Vec::with_capacity(2 * corners.len() - 1);
    for (k, &c) in corners.iter().enumerate() {
        if k > 0 {
            let p = corners[k - 1];
            front.push(
                *mid.get(&(p.min(c), p.max(c)))
                    .ok_or_else(|| invalid(KEY, format!("no mid-edge node between {p} and {c}")))?,
            );
        }
        front.push(c);
    }
    Ok(CrackedHalfCylinder { mesh: out, front })
}
