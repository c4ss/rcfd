//! Frictionless contact (node to surface, augmented Lagrangian). Declared by ADR 0440 (F-408, the scaffold of ADR 0311);
//! filled by ADR 0507 (the registration of ADR 0330, F-427).
//!
//! * Gap (ADR 0507 D1): a pair is a contactor surface (Hex8 faces; its corner nodes are the contactor nodes) and a target, a
//!   rigid plane or a deformable surface of Hex8 faces. Against a face the gap is the signed distance to the closest point of
//!   the bilinear face at the current positions `X + u`, plus the node's initial-gap offset; the search over the target faces
//!   is repeated at every evaluation. The gradient and the second derivative of the gap are exact (the closest-point
//!   variation, the normal's variation and the face's twist `x_{,xi eta}`).
//! * Augmented Lagrangian (D2; Simo and Laursen, Comput. Struct. 42 (1992) 97; Wriggers, Computational Contact Mechanics,
//!   2006; from memory, re-derived in ADR 0507): per node a multiplier `lambda`, the force `t = max(0, lambda - eps A g)`,
//!   the energy `t^2/(2 eps A)`, its exact force and tangent; Uzawa updates `lambda <- t` after each converged Newton solve
//!   until the largest penetration and the largest gap at an active node are within the tolerance (amendment 3).
//! * Driver (D3): [`solve`] and [`solve_with`], load control with the augmentation loop per step, a linear body
//!   ([`Kinematics::Linear`], the stiffness of [`crate::assemble::stiffness`], Newton here with a sparse `L D L^T`, by
//!   default after minimum degree, or the supernodal one of `ContactOptions::factor`, ADR 0533) or the total
//!   Lagrangian body of [`crate::nonlinear::solve_from`] ([`Kinematics::TotalLagrangian`], the contact as its first
//!   [`Contribution`]).
//! * Cycling (D6): an active set that comes back after a change more than `max_cycles` times in one Newton solve is an error
//!   naming the nodes that switched.
//! * [`ContactSet`] is the contribution; it can be evaluated on its own.

use crate::assemble::stiffness;
use crate::constraint::{prescribed_equations, reduce, Prescribed};
use crate::element::ElementState;
use crate::error::{FemError, Result, SetKind};
use crate::loads::{nodal_load_vector, NodalForceAt};
use crate::mesh::{ElementKind, FaceRef};
use crate::model::{Constraint, DofMap, SolidModel, SolidResult, SolverStats};
use crate::nonlinear::{add_triplets, initial_states, solve_from, Contribution, NewtonOptions};
use crate::solve::DirectFactor;
use rcfd_core::vec3::{add, cross, dot, norm, sub, Vec3};
use rcfd_linalg::skyline::SkylineError;
use rcfd_linalg::sparse_chol::{SparseLdlt, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The solver name of the contact driver in its errors and [`SolverStats`].
pub const SOLVER_NAME: &str = "contact";

/// The target of a contact pair (ADR 0507 D1).
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A rigid plane through `point` with the unit `normal` toward the side where the contactor may be.
    RigidPlane {
        /// A point of the plane.
        point: Vec3,
        /// The normal (normalised by the pair).
        normal: Vec3,
    },
    /// A deformable surface: a surface set of Hex8 faces (outward normals).
    Surface(String),
}

/// A contact pair: the contactor surface set, the target, the penalty per unit area (N/m^3) and the initial-gap offsets.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactPair {
    /// The contactor surface set (Hex8 faces).
    pub contactor: String,
    /// The target.
    pub target: Target,
    /// The penalty per unit area, N/m^3; a node's penalty is this times its area.
    pub penalty: f64,
    /// Initial-gap offsets `(node, g0)` added to the gap of contactor nodes (default 0).
    pub initial_gaps: Vec<(usize, f64)>,
}

impl ContactPair {
    /// A pair without initial-gap offsets.
    pub fn new(contactor: &str, target: Target, penalty: f64) -> ContactPair {
        ContactPair {
            contactor: contactor.to_string(),
            target,
            penalty,
            initial_gaps: Vec::new(),
        }
    }
    /// The same pair with the initial-gap offset `g0` at the contactor node `node`.
    pub fn initial_gap(mut self, node: usize, g0: f64) -> ContactPair {
        self.initial_gaps.push((node, g0));
        self
    }
}

/// The body of the contact driver (ADR 0507 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kinematics {
    /// The linear body `K u` (small displacements); the gap at `X + u`.
    #[default]
    Linear,
    /// The total Lagrangian body of [`crate::nonlinear`] (F-397).
    TotalLagrangian,
}

/// Options of the contact driver.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ContactOptions {
    /// Load steps of equal size (at least 1).
    pub steps: usize,
    /// The largest penetration, and the largest gap at an active node, accepted after augmentation, m (absolute; ADR 0507
    /// amendment 3).
    pub penetration_tolerance: f64,
    /// Newton solves after the first one of a step (Uzawa updates); 0 is the pure penalty without a tolerance test.
    pub max_augmentations: usize,
    /// Cycle events allowed in one Newton solve (ADR 0507 D6).
    pub max_cycles: usize,
    /// The search distance, m (`None`: the longest current edge of each target face).
    pub search_distance: Option<f64>,
    /// The body.
    pub kinematics: Kinematics,
    /// The Newton iteration and its criteria (`steps` and `max_halvings` are not used).
    pub newton: NewtonOptions,
    /// The factor of the linear body's reduced tangent (ADR 0533): [`DirectFactor::SparseMinimumDegree`] (the default, the
    /// up-looking sparse `L D L^T` after approximate minimum degree) or [`DirectFactor::SupernodalNestedDissection`] (the
    /// supernodal multifrontal `L D L^T` after nested dissection by level structures of the tangent's pattern). The envelope
    /// is refused. With [`Kinematics::TotalLagrangian`] the supernodal choice is passed to [`solve_from`] as
    /// [`NewtonOptions::factor`] (ADR 0542 D3); with the linear body `newton.factor` must be the envelope (key `"solver"`).
    pub factor: DirectFactor,
}

impl ContactOptions {
    /// `steps` load steps and the penetration tolerance; 20 augmentations, 3 cycle events, the linear body, the Newton
    /// options of `NewtonOptions::new(1)`.
    pub fn new(steps: usize, penetration_tolerance: f64) -> ContactOptions {
        ContactOptions {
            steps,
            penetration_tolerance,
            max_augmentations: 20,
            max_cycles: 3,
            search_distance: None,
            kinematics: Kinematics::Linear,
            newton: NewtonOptions::new(1),
            factor: DirectFactor::SparseMinimumDegree,
        }
    }
    /// The same options with another body.
    pub fn kinematics(mut self, k: Kinematics) -> ContactOptions {
        self.kinematics = k;
        self
    }
    /// The same options with another augmentation count.
    pub fn max_augmentations(mut self, n: usize) -> ContactOptions {
        self.max_augmentations = n;
        self
    }
    /// The same options with another cycle count.
    pub fn max_cycles(mut self, n: usize) -> ContactOptions {
        self.max_cycles = n;
        self
    }
    /// The same options with a search distance.
    pub fn search_distance(mut self, d: f64) -> ContactOptions {
        self.search_distance = Some(d);
        self
    }
    /// The same options with other Newton options.
    pub fn newton(mut self, o: NewtonOptions) -> ContactOptions {
        self.newton = o;
        self
    }
    /// The same options with another factor of the linear body's tangent (ADR 0533).
    pub fn factor(mut self, f: DirectFactor) -> ContactOptions {
        self.factor = f;
        self
    }

    fn check(&self) -> Result<()> {
        if self.steps == 0 {
            return Err(FemError::InvalidValue {
                key: "steps",
                reason: "0 load steps: at least 1".to_string(),
            });
        }
        if !(self.penetration_tolerance >= 0.0 && self.penetration_tolerance.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "penetration_tolerance",
                reason: format!("{:e}: must be finite and not negative", self.penetration_tolerance),
            });
        }
        if let Some(d) = self.search_distance {
            if !(d > 0.0 && d.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "search_distance",
                    reason: format!("{d:e}: must be positive and finite"),
                });
            }
        }
        match self.factor {
            DirectFactor::SparseMinimumDegree => {}
            DirectFactor::SupernodalNestedDissection => {}
            other => {
                return Err(FemError::InvalidValue {
                    key: "solver",
                    reason: format!(
                        "{other:?}: the contact driver factors by SparseMinimumDegree or SupernodalNestedDissection"
                    ),
                })
            }
        }
        if self.kinematics == Kinematics::Linear && self.newton.factor != DirectFactor::Envelope {
            return Err(FemError::InvalidValue {
                key: "solver",
                reason: format!(
                    "newton.factor {:?}: the linear body's factor is ContactOptions::factor (ADR 0542 D3)",
                    self.newton.factor
                ),
            });
        }
        let mut o = self.newton.clone();
        o.steps = 1;
        o.check()
    }
}

/// The state of one contactor node.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactNode {
    /// The pair index.
    pub pair: usize,
    /// The node.
    pub node: usize,
    /// The node's area (D4), m^2.
    pub area: f64,
    /// The gap (with the offset), m; `None` when no target face is in reach.
    pub gap: Option<f64>,
    /// The force `t = max(0, lambda - eps A g)`, N (0 when not active).
    pub force: f64,
    /// `force / area`, Pa.
    pub pressure: f64,
    /// Whether the node is active (`lambda - eps A g >= 0` with a gap).
    pub active: bool,
    /// The target face (deformable targets).
    pub face: Option<FaceRef>,
    /// The local coordinates of the closest point on that face.
    pub local: [f64; 2],
    /// The target's normal there.
    pub normal: Vec3,
}

/// One Newton solve of the driver.
#[derive(Debug, Clone, PartialEq)]
pub struct NewtonSolve {
    /// The relative residuals of the iterations.
    pub residuals: Vec<f64>,
    /// The energies of the corrections.
    pub energies: Vec<f64>,
    /// The active contactor nodes (ascending, all pairs) at every evaluation of the contact set.
    pub active_sets: Vec<Vec<usize>>,
    /// The largest penetration at the converged state, m.
    pub penetration: f64,
    /// Iterations taken with the contact matrix without its curvature part because the exact tangent failed the pivot
    /// test (linear body, ADR 0507 amendment 2); 0 for the total Lagrangian body.
    pub convex_iterations: usize,
}

/// One load step of the driver.
#[derive(Debug, Clone, PartialEq)]
pub struct ContactStep {
    /// The load step (1-based).
    pub step: usize,
    /// The load factor.
    pub load_factor: f64,
    /// The Newton solves: the first one and one per augmentation.
    pub solves: Vec<NewtonSolve>,
    /// The displacement at the end of the step.
    pub displacement: Vec<f64>,
    /// The reactions at the end of the step (`f_int + f_c + f_extra - f` at the prescribed equations, 0 elsewhere).
    pub reactions: Vec<f64>,
    /// The contactor nodes at the end of the step, with the forces of the accepted solve.
    pub nodes: Vec<ContactNode>,
}

/// The result of [`solve`].
#[derive(Debug, Clone, PartialEq)]
pub struct ContactResult {
    /// Displacement, reactions (`f_int + f_c + f_extra - f` at the prescribed equations) and statistics at load factor 1.
    pub result: SolidResult,
    /// The steps in order.
    pub steps: Vec<ContactStep>,
    /// The contactor nodes at load factor 1 (pair by pair, nodes ascending), with the forces of the accepted solve.
    pub nodes: Vec<ContactNode>,
    /// The committed element states (total Lagrangian; empty states for the linear body).
    pub states: Vec<ElementState>,
}

#[derive(Debug, Clone)]
enum TargetData {
    Plane { point: Vec3, normal: Vec3 },
    Faces(Vec<(FaceRef, [usize; 4])>),
}

#[derive(Debug, Clone)]
struct PairData {
    target: TargetData,
    nodes: Vec<usize>,
    areas: Vec<f64>,
    offsets: Vec<f64>,
    eps: Vec<f64>,
}

/// The kinematics of one node against its target at one state.
struct Kin {
    /// The nodes of the gradient: the contactor node, then the face's corners.
    nodes: Vec<usize>,
    /// The gap without the offset.
    gap: f64,
    normal: Vec3,
    grad: Vec<f64>,
    /// The second derivative (row-major, `3 nodes` square); `None` for a plane.
    hess: Option<Vec<f64>>,
    face: Option<FaceRef>,
    local: [f64; 2],
}

struct Eval {
    kin: Option<Kin>,
    gap: f64,
    force: f64,
    active: bool,
}

#[derive(Default)]
struct History {
    sets: Vec<Vec<bool>>,
    events: usize,
    label: String,
    failure: Option<FemError>,
}

/// The contact contribution (ADR 0507 D5): the pairs, their node areas and multipliers, the search and the cycle record.
pub struct ContactSet {
    pairs: Vec<PairData>,
    dofs: DofMap,
    reference: Vec<Vec3>,
    search: Option<f64>,
    max_cycles: usize,
    multipliers: RefCell<Vec<Vec<f64>>>,
    history: RefCell<History>,
}

const CORNERS: [[f64; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];

fn hex8_faces(model: &SolidModel, set: &str) -> Result<Vec<(FaceRef, [usize; 4])>> {
    let mesh = model.mesh();
    let faces = mesh.surface_set(set).ok_or_else(|| FemError::UnknownSet {
        kind: SetKind::Surface,
        name: set.to_string(),
    })?;
    let mut out = Vec::with_capacity(faces.len());
    for &f in faces {
        let kind = mesh.element_kind(f.element);
        let nodes = mesh.face_nodes(f);
        if kind != ElementKind::Hex8 || nodes.len() != 4 {
            return Err(FemError::InvalidValue {
                key: "contact surface",
                reason: format!(
                    "face {} of element {} ({kind:?}) in the set '{set}': only Hex8 faces (ADR 0507 limits)",
                    f.face, f.element
                ),
            });
        }
        out.push((f, [nodes[0], nodes[1], nodes[2], nodes[3]]));
    }
    Ok(out)
}

/// The bilinear shape functions and their derivatives at `xi`.
fn shape(xi: [f64; 2]) -> ([f64; 4], [[f64; 2]; 4]) {
    let mut n = [0.0; 4];
    let mut d = [[0.0; 2]; 4];
    for (a, c) in CORNERS.iter().enumerate() {
        n[a] = 0.25 * (1.0 + xi[0] * c[0]) * (1.0 + xi[1] * c[1]);
        d[a][0] = 0.25 * c[0] * (1.0 + xi[1] * c[1]);
        d[a][1] = 0.25 * c[1] * (1.0 + xi[0] * c[0]);
    }
    (n, d)
}

fn comb(w: &[f64; 4], x: &[Vec3; 4]) -> Vec3 {
    let mut out = [0.0; 3];
    for a in 0..4 {
        for i in 0..3 {
            out[i] += w[a] * x[a][i];
        }
    }
    out
}

fn inv2(m: [[f64; 2]; 2]) -> Option<[[f64; 2]; 2]> {
    let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    Some([[m[1][1] / det, -m[0][1] / det], [-m[1][0] / det, m[0][0] / det]])
}

fn mul2(a: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let mut c = [[0.0; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            c[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j];
        }
    }
    c
}

/// The closest point of `xs` on the bilinear face `x` (ADR 0507 D1): `(xi, gap, normal, gradient, second derivative)` over
/// the 15 coordinates `xs, x_0 .. x_3`, or `None` when the projection does not converge inside the face.
#[allow(clippy::type_complexity)]
fn project(xs: Vec3, x: &[Vec3; 4]) -> Option<([f64; 2], f64, Vec3, Vec<f64>, Vec<f64>)> {
    let twist = [0.25, -0.25, 0.25, -0.25];
    let x12 = comb(&twist, x);
    let mut xi = [0.0f64; 2];
    let mut converged = false;
    let mut last_step = f64::INFINITY;
    for _ in 0..30 {
        let (n, d) = shape(xi);
        let p = comb(&n, x);
        let a1 = comb(&[d[0][0], d[1][0], d[2][0], d[3][0]], x);
        let a2 = comb(&[d[0][1], d[1][1], d[2][1], d[3][1]], x);
        let dv = sub(xs, p);
        let rhs = [dot(dv, a1), dot(dv, a2)];
        let c = dot(dv, x12);
        let m = [[dot(a1, a1), dot(a1, a2) - c], [dot(a1, a2) - c, dot(a2, a2)]];
        let mi = inv2(m)?;
        let step = [
            mi[0][0] * rhs[0] + mi[0][1] * rhs[1],
            mi[1][0] * rhs[0] + mi[1][1] * rhs[1],
        ];
        xi[0] += step[0];
        xi[1] += step[1];
        if !(xi[0].is_finite() && xi[1].is_finite()) || xi[0].abs() > 10.0 || xi[1].abs() > 10.0 {
            return None;
        }
        last_step = step[0].abs().max(step[1].abs());
        if last_step <= 1e-14 {
            converged = true;
            break;
        }
    }
    // ADR 0517 amendment 1: on a face small against its coordinates the steps stay at the rounding floor
    // (about ulp(|x|) over the face's half-size) above 1e-14; a last step at most 1e-10 is that floor.
    if !converged && last_step <= 1e-10 {
        converged = true;
    }
    if !converged {
        return None;
    }
    // ADR 0507 amendment 2: the bilinear face extended by one percent of its half-width beyond its edges.
    let tol = 1.0 + 1e-2;
    if xi[0].abs() > tol || xi[1].abs() > tol {
        return None;
    }
    let (nn, d) = shape(xi);
    let p = comb(&nn, x);
    let a = [
        comb(&[d[0][0], d[1][0], d[2][0], d[3][0]], x),
        comb(&[d[0][1], d[1][1], d[2][1], d[3][1]], x),
    ];
    let nr = cross(a[0], a[1]);
    let len = norm(nr);
    if len <= 0.0 || !len.is_finite() {
        return None;
    }
    let normal = [nr[0] / len, nr[1] / len, nr[2] / len];
    let g = dot(sub(xs, p), normal);
    // Gradient: blocks n (contactor) and -N_a n.
    let mut grad = vec![0.0; 15];
    for i in 0..3 {
        grad[i] = normal[i];
        for b in 0..4 {
            grad[3 + 3 * b + i] = -nn[b] * normal[i];
        }
    }
    // T_alpha (blocks a_alpha, -N_a a_alpha) and P_alpha (blocks 0, N_{a,alpha} n).
    let mut t = [vec![0.0; 15], vec![0.0; 15]];
    let mut pv = [vec![0.0; 15], vec![0.0; 15]];
    for al in 0..2 {
        for i in 0..3 {
            t[al][i] = a[al][i];
            for b in 0..4 {
                t[al][3 + 3 * b + i] = -nn[b] * a[al][i];
                pv[al][3 + 3 * b + i] = d[b][al] * normal[i];
            }
        }
    }
    let m = [[dot(a[0], a[0]), dot(a[0], a[1])], [dot(a[1], a[0]), dot(a[1], a[1])]];
    let h12 = dot(normal, x12);
    let h = [[0.0, h12], [h12, 0.0]];
    let am = [
        [m[0][0] - g * h[0][0], m[0][1] - g * h[0][1]],
        [m[1][0] - g * h[1][0], m[1][1] - g * h[1][1]],
    ];
    let mi = inv2(m)?;
    let ai = inv2(am)?;
    let c = mul2(mul2(mi, h), ai);
    // d2g = - T^T C T - T^T A^-1 P - P^T A^-1 T - g P^T A^-1 P.
    let mut hess = vec![0.0; 225];
    for al in 0..2 {
        for be in 0..2 {
            let (cab, aab) = (c[al][be], ai[al][be]);
            for r in 0..15 {
                let (tr, pr) = (t[al][r], pv[al][r]);
                for s in 0..15 {
                    hess[15 * r + s] -=
                        cab * tr * t[be][s] + aab * (tr * pv[be][s] + pr * t[be][s]) + g * aab * pr * pv[be][s];
                }
            }
        }
    }
    Some((xi, g, normal, grad, hess))
}

impl ContactSet {
    /// The contact set of `pairs` on `model` with the search distance and cycle count of `options`; the multipliers start at
    /// zero. Refuses an unknown set, a face that is not a Hex8 face, a penalty that is not positive and finite, a plane normal
    /// of zero or not finite length, an offset at a node that is not a contactor node or not finite, and a node without three
    /// translations.
    pub fn new(model: &SolidModel, pairs: &[ContactPair], options: &ContactOptions) -> Result<ContactSet> {
        let mesh = model.mesh();
        let dofs = model.dof_map();
        let reference = mesh.nodes().to_vec();
        let mut out = Vec::with_capacity(pairs.len());
        for (k, p) in pairs.iter().enumerate() {
            if !(p.penalty > 0.0 && p.penalty.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "penalty",
                    reason: format!("pair {k}: {:e}: must be positive and finite", p.penalty),
                });
            }
            // D4: the consistent share of a unit pressure, 2 x 2 Gauss on the reference face.
            let mut area: BTreeMap<usize, f64> = BTreeMap::new();
            let gp = 1.0 / 3f64.sqrt();
            for (_, nodes) in hex8_faces(model, &p.contactor)? {
                let x = nodes.map(|v| reference[v]);
                for &(s, r) in &[(-gp, -gp), (gp, -gp), (gp, gp), (-gp, gp)] {
                    let (n, d) = shape([s, r]);
                    let a1 = comb(&[d[0][0], d[1][0], d[2][0], d[3][0]], &x);
                    let a2 = comb(&[d[0][1], d[1][1], d[2][1], d[3][1]], &x);
                    let j = norm(cross(a1, a2));
                    for b in 0..4 {
                        *area.entry(nodes[b]).or_insert(0.0) += n[b] * j;
                    }
                }
            }
            let nodes: Vec<usize> = area.keys().copied().collect();
            let areas: Vec<f64> = area.values().copied().collect();
            let mut offsets = vec![0.0; nodes.len()];
            for &(node, g0) in &p.initial_gaps {
                let i = nodes.binary_search(&node).map_err(|_| FemError::InvalidValue {
                    key: "initial gap",
                    reason: format!(
                        "pair {k}: node {node} is not a node of the contactor set '{}'",
                        p.contactor
                    ),
                })?;
                if !g0.is_finite() {
                    return Err(FemError::InvalidValue {
                        key: "initial gap",
                        reason: format!("pair {k}: {g0:e} at node {node} is not finite"),
                    });
                }
                offsets[i] = g0;
            }
            let target = match &p.target {
                Target::RigidPlane { point, normal } => {
                    let l = norm(*normal);
                    if !(l > 0.0 && l.is_finite()) || point.iter().any(|v| !v.is_finite()) {
                        return Err(FemError::InvalidValue {
                            key: "rigid plane",
                            reason: format!("pair {k}: point {point:?}, normal {normal:?}: a finite point and normal"),
                        });
                    }
                    TargetData::Plane {
                        point: *point,
                        normal: [normal[0] / l, normal[1] / l, normal[2] / l],
                    }
                }
                Target::Surface(set) => TargetData::Faces(hex8_faces(model, set)?),
            };
            let mut all: Vec<usize> = nodes.clone();
            if let TargetData::Faces(f) = &target {
                all.extend(f.iter().flat_map(|(_, n)| n.iter().copied()));
            }
            for &v in &all {
                if dofs.components(v) < 3 {
                    return Err(FemError::InvalidValue {
                        key: "contact node",
                        reason: format!(
                            "pair {k}: node {v} has {} unknowns, three translations needed",
                            dofs.components(v)
                        ),
                    });
                }
            }
            let eps = areas.iter().map(|a| p.penalty * a).collect();
            out.push(PairData {
                target,
                nodes,
                areas,
                offsets,
                eps,
            });
        }
        let multipliers = out.iter().map(|p| vec![0.0; p.nodes.len()]).collect();
        Ok(ContactSet {
            pairs: out,
            dofs,
            reference,
            search: options.search_distance,
            max_cycles: options.max_cycles,
            multipliers: RefCell::new(multipliers),
            history: RefCell::new(History::default()),
        })
    }

    /// The multipliers, pair by pair, nodes ascending (N).
    pub fn multipliers(&self) -> Vec<Vec<f64>> {
        self.multipliers.borrow().clone()
    }

    /// Sets the multipliers of pair `pair` (one per contactor node, ascending); refuses a wrong length or a negative or not
    /// finite value.
    pub fn set_multipliers(&self, pair: usize, values: &[f64]) -> Result<()> {
        let mut m = self.multipliers.borrow_mut();
        let slot = m.get_mut(pair).ok_or_else(|| FemError::InvalidValue {
            key: "multipliers",
            reason: format!("pair {pair} does not exist"),
        })?;
        if values.len() != slot.len() {
            return Err(FemError::FieldLength {
                key: "multipliers",
                expected: slot.len(),
                found: values.len(),
            });
        }
        if let Some(v) = values.iter().find(|v| !(**v >= 0.0 && v.is_finite())) {
            return Err(FemError::InvalidValue {
                key: "multipliers",
                reason: format!("{v:e}: must be finite and not negative"),
            });
        }
        slot.copy_from_slice(values);
        Ok(())
    }

    /// The contactor nodes of pair `pair`, ascending.
    pub fn nodes(&self, pair: usize) -> &[usize] {
        &self.pairs[pair].nodes
    }

    /// Starts a Newton solve: clears the record of active sets (D6); `label` locates its errors.
    pub fn begin(&self, label: &str) {
        let mut h = self.history.borrow_mut();
        *h = History::default();
        h.label = label.to_string();
    }

    /// The active sets recorded since [`ContactSet::begin`] (active contactor nodes, ascending, all pairs).
    pub fn active_sets(&self) -> Vec<Vec<usize>> {
        let h = self.history.borrow();
        h.sets.iter().map(|s| self.flag_nodes(s)).collect()
    }

    fn flag_nodes(&self, flags: &[bool]) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .pairs
            .iter()
            .flat_map(|p| p.nodes.iter())
            .zip(flags)
            .filter(|(_, &f)| f)
            .map(|(&n, _)| n)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    fn take_failure(&self) -> Option<FemError> {
        self.history.borrow_mut().failure.take()
    }

    fn position(&self, u: &[f64], node: usize) -> Vec3 {
        let x = self.reference[node];
        let e = self.dofs.equation(node, 0).expect("checked node");
        [x[0] + u[e], x[1] + u[e + 1], x[2] + u[e + 2]]
    }

    fn displacement(&self, u: &[f64], node: usize) -> Vec3 {
        let e = self.dofs.equation(node, 0).expect("checked node");
        [u[e], u[e + 1], u[e + 2]]
    }

    /// ADR 0507 amendment 3: the gap `n . sum_a N_a ((X_s - X_a) + (u_s - u_a))` from differences.
    fn face_gap(&self, u: &[f64], node: usize, face: &[usize; 4], xi: [f64; 2], n: Vec3) -> f64 {
        let (nn, _) = shape(xi);
        let (xs, us) = (self.reference[node], self.displacement(u, node));
        let mut g = 0.0;
        for (b, &v) in face.iter().enumerate() {
            let d = add(sub(xs, self.reference[v]), sub(us, self.displacement(u, v)));
            g += nn[b] * dot(d, n);
        }
        g
    }

    fn kin(&self, pair: &PairData, u: &[f64], node: usize) -> Option<Kin> {
        let xs = self.position(u, node);
        match &pair.target {
            TargetData::Plane { point, normal } => Some(Kin {
                nodes: vec![node],
                // ADR 0507 amendment 3: from differences.
                gap: dot(sub(self.reference[node], *point), *normal) + dot(self.displacement(u, node), *normal),
                normal: *normal,
                grad: normal.to_vec(),
                hess: None,
                face: None,
                local: [0.0; 2],
            }),
            TargetData::Faces(faces) => {
                let mut best: Option<Kin> = None;
                for (f, fn_) in faces {
                    if fn_.contains(&node) {
                        continue;
                    }
                    let x = fn_.map(|v| self.position(u, v));
                    let reach = match self.search {
                        Some(d) => d,
                        None => (0..4).map(|a| norm(sub(x[(a + 1) % 4], x[a]))).fold(0.0, f64::max),
                    };
                    let inside = (0..3).all(|i| {
                        let lo = x.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min) - reach;
                        let hi = x.iter().map(|p| p[i]).fold(f64::NEG_INFINITY, f64::max) + reach;
                        xs[i] >= lo && xs[i] <= hi
                    });
                    if !inside {
                        continue;
                    }
                    let Some((xi, _, n, grad, hess)) = project(xs, &x) else {
                        continue;
                    };
                    let g = self.face_gap(u, node, fn_, xi, n);
                    if g.abs() > reach {
                        continue;
                    }
                    if best.as_ref().map_or(true, |b| g.abs() < b.gap.abs()) {
                        let mut nodes = vec![node];
                        nodes.extend_from_slice(fn_);
                        best = Some(Kin {
                            nodes,
                            gap: g,
                            normal: n,
                            grad,
                            hess: Some(hess),
                            face: Some(*f),
                            local: xi,
                        });
                    }
                }
                best
            }
        }
    }

    fn evaluate_all(&self, u: &[f64]) -> Result<Vec<Vec<Eval>>> {
        if u.len() != self.dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: self.dofs.n_equations(),
                found: u.len(),
            });
        }
        let lam = self.multipliers.borrow();
        let mut out = Vec::with_capacity(self.pairs.len());
        for (k, p) in self.pairs.iter().enumerate() {
            let mut v = Vec::with_capacity(p.nodes.len());
            for (i, &node) in p.nodes.iter().enumerate() {
                let kin = self.kin(p, u, node);
                let (gap, force, active) = match &kin {
                    None => (f64::INFINITY, 0.0, false),
                    Some(kn) => {
                        let g = kn.gap + p.offsets[i];
                        let trial = lam[k][i] - p.eps[i] * g;
                        if !trial.is_finite() {
                            return Err(FemError::InvalidValue {
                                key: "contact",
                                reason: format!("pair {k}, node {node}: the gap {g:e} is not finite"),
                            });
                        }
                        (g, trial.max(0.0), trial >= 0.0)
                    }
                };
                v.push(Eval {
                    kin,
                    gap,
                    force,
                    active,
                });
            }
            out.push(v);
        }
        Ok(out)
    }

    /// The contactor nodes at the displacement `u` with the current multipliers (nothing is recorded).
    pub fn evaluate(&self, u: &[f64]) -> Result<Vec<ContactNode>> {
        let all = self.evaluate_all(u)?;
        let mut out = Vec::new();
        for (k, (p, ev)) in self.pairs.iter().zip(&all).enumerate() {
            for (i, e) in ev.iter().enumerate() {
                let force = if e.active { e.force } else { 0.0 };
                out.push(ContactNode {
                    pair: k,
                    node: p.nodes[i],
                    area: p.areas[i],
                    gap: e.kin.as_ref().map(|_| e.gap),
                    force,
                    pressure: force / p.areas[i],
                    active: e.active,
                    face: e.kin.as_ref().and_then(|kn| kn.face),
                    local: e.kin.as_ref().map_or([0.0; 2], |kn| kn.local),
                    normal: e.kin.as_ref().map_or([0.0; 3], |kn| kn.normal),
                });
            }
        }
        Ok(out)
    }

    /// The force and the triplets of the state `all` without recording the active set; without `curvature` the matrix is
    /// `eps G G^T` only (ADR 0507 amendment 2).
    fn force_and_tangent(
        &self,
        all: &[Vec<Eval>],
        force: &mut [f64],
        tangent: Option<&mut Vec<(usize, usize, f64)>>,
        curvature: bool,
    ) {
        let mut trip = tangent;
        for (p, ev) in self.pairs.iter().zip(all) {
            for (i, e) in ev.iter().enumerate() {
                if !e.active {
                    continue;
                }
                let kn = e.kin.as_ref().expect("active has kinematics");
                let eqs: Vec<usize> = kn
                    .nodes
                    .iter()
                    .flat_map(|&v| {
                        let e0 = self.dofs.equation(v, 0).expect("checked node");
                        [e0, e0 + 1, e0 + 2]
                    })
                    .collect();
                let m = eqs.len();
                for (r, &er) in eqs.iter().enumerate() {
                    force[er] -= e.force * kn.grad[r];
                }
                if let Some(t) = trip.as_deref_mut() {
                    let eps = p.eps[i];
                    for r in 0..m {
                        for s in 0..m {
                            let mut v = eps * kn.grad[r] * kn.grad[s];
                            if let (true, Some(h)) = (curvature, &kn.hess) {
                                v -= e.force * h[m * r + s];
                            }
                            if v != 0.0 {
                                t.push((eqs[r], eqs[s], v));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Records the active set of an evaluation and returns the cycle error when the events exceed the count (D6).
    fn record(&self, all: &[Vec<Eval>]) -> Result<()> {
        let flags: Vec<bool> = all.iter().flat_map(|v| v.iter().map(|e| e.active)).collect();
        let mut h = self.history.borrow_mut();
        let k = h.sets.len();
        if k >= 1 && h.sets[k - 1] != flags && h.sets[..k - 1].contains(&flags) {
            h.events += 1;
            if h.events > self.max_cycles {
                let prev = h.sets[k - 1].clone();
                let switched: Vec<bool> = flags.iter().zip(&prev).map(|(a, b)| a != b).collect();
                let nodes = self.flag_nodes(&switched);
                let err = FemError::Solver {
                    solver: SOLVER_NAME,
                    reason: format!(
                        "{}: the active set cycled {} times in one Newton solve (more than {}) at Newton iteration {k}; \
                         nodes switching: {nodes:?}",
                        h.label, h.events, self.max_cycles
                    ),
                };
                h.failure = Some(err.clone());
                h.sets.push(flags);
                return Err(err);
            }
        }
        h.sets.push(flags);
        Ok(())
    }

    /// Sets the multipliers to the forces of the state `all` (the Uzawa update, D2).
    fn update(&self, all: &[Vec<Eval>]) {
        let mut m = self.multipliers.borrow_mut();
        for (k, ev) in all.iter().enumerate() {
            for (i, e) in ev.iter().enumerate() {
                m[k][i] = if e.active { e.force } else { 0.0 };
            }
        }
    }
}

impl Contribution for ContactSet {
    fn name(&self) -> &str {
        SOLVER_NAME
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> Result<()> {
        let all = self.evaluate_all(u)?;
        self.record(&all)?;
        self.force_and_tangent(&all, force, Some(tangent), true);
        Ok(())
    }
}

fn norm2(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

struct Solved {
    reference: f64,
    convex: usize,
    u: Vec<f64>,
    residuals: Vec<f64>,
    energies: Vec<f64>,
    states: Option<Vec<ElementState>>,
    internal: Vec<f64>,
}

/// The factor of the linear body's reduced tangent (ADR 0533 D2).
enum Factored {
    Sparse(SparseLdlt),
    Supernodal(SupernodalLdlt),
}

impl Factored {
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        match self {
            Factored::Sparse(l) => l.solve(b),
            Factored::Supernodal(l) => l.solve(b),
        }
    }
}

/// Factors `a` into `cache` by `factor` unless the cached matrix has the same bits.
fn factor_cached(
    a: &CsrMatrix,
    factor: DirectFactor,
    cache: &mut Option<(CsrMatrix, Factored)>,
) -> std::result::Result<(), SkylineError> {
    let same = cache.as_ref().is_some_and(|(c, _)| {
        c.row_ptr == a.row_ptr && c.col == a.col && c.val.iter().zip(&a.val).all(|(x, y)| x.to_bits() == y.to_bits())
    });
    if !same {
        let l = if factor == DirectFactor::SupernodalNestedDissection {
            Factored::Supernodal(SupernodalLdlt::factor(a)?)
        } else {
            Factored::Sparse(SparseLdlt::factor(a)?)
        };
        *cache = Some((a.clone(), l));
    }
    Ok(())
}

/// Newton for the linear body (ADR 0507 D3): `K u + f_c(u) + f_extra(u) = f` on the free equations, the criteria of ADR
/// 0404 D5, the reduced tangent by a sparse `L D L^T` (`factor`, ADR 0533), reused while its bits do not change.
#[allow(clippy::too_many_arguments)]
fn linear_solve(
    k: &CsrMatrix,
    f: &[f64],
    fixed: &[(usize, f64)],
    start: &[f64],
    set: &ContactSet,
    extra: &[&dyn Contribution],
    o: &NewtonOptions,
    factor: DirectFactor,
    cache: &mut Option<(CsrMatrix, Factored)>,
    floor: f64,
) -> Result<Solved> {
    let n = k.n;
    let mut is_fixed = vec![false; n];
    for &(eq, _) in fixed {
        is_fixed[eq] = true;
    }
    let free: Vec<usize> = (0..n).filter(|&i| !is_fixed[i]).collect();
    let zero: Vec<(usize, f64)> = fixed.iter().map(|&(eq, _)| (eq, 0.0)).collect();
    let fail = |reason: String| FemError::Solver {
        solver: SOLVER_NAME,
        reason,
    };
    let mut u = start.to_vec();
    let (mut residuals, mut energies) = (Vec::new(), Vec::new());
    let (mut e_first, mut e_last) = (0.0, 0.0);
    let mut convex = 0;
    for it in 0..=o.max_iterations {
        let mut fint = vec![0.0; n];
        k.matvec(&u, &mut fint);
        let internal = fint.clone();
        let elastic_norm = norm2(internal.iter().copied());
        let mut trip = Vec::new();
        let mut trip_x = Vec::new();
        let mut fc = vec![0.0; n];
        set.add(&u, 1.0, &mut fc, &mut trip)?;
        let contact_norm = norm2(fc.iter().copied());
        for (a, b) in fint.iter_mut().zip(&fc) {
            *a += b;
        }
        for c in extra {
            let mut fx = vec![0.0; n];
            c.add(&u, 1.0, &mut fx, &mut trip_x)
                .map_err(|e| fail(format!("contribution '{}': {e}", c.name())))?;
            for (a, b) in fint.iter_mut().zip(&fx) {
                *a += b;
            }
        }
        let rfull: Vec<f64> = f.iter().zip(&fint).map(|(a, b)| a - b).collect();
        let rn = norm2(free.iter().map(|&i| rfull[i]));
        // ADR 0507 amendment 2: the reference is at least the largest of the run's earlier converged solves.
        // ADR 0507 amendment 3: and at least |K u| and |f_c| over every equation (the pattern of ADR 0479 D2).
        let reference = norm2(free.iter().map(|&i| f[i]))
            .max(norm2(fint.iter().copied()))
            .max(elastic_norm)
            .max(contact_norm)
            .max(floor);
        let r = if reference > 0.0 { rn / reference } else { rn };
        if !r.is_finite() {
            return Err(fail(format!("non-finite residual at Newton iteration {it}")));
        }
        residuals.push(r);
        if it >= 1 && r <= o.residual_tolerance && e_last <= o.energy_tolerance * e_first {
            return Ok(Solved {
                reference,
                convex,
                u,
                residuals,
                energies,
                states: None,
                internal,
            });
        }
        if it == o.max_iterations {
            return Err(fail(format!(
                "not converged in {} iterations (relative residual {r:e}, energy ratio {:e})",
                o.max_iterations,
                if e_first > 0.0 { e_last / e_first } else { e_last }
            )));
        }
        let build = |contact: &[(usize, usize, f64)]| {
            let mut kt = k.clone();
            add_triplets(&mut kt, contact);
            add_triplets(&mut kt, &trip_x);
            reduce(&kt, &rfull, &zero)
        };
        let sys = build(&trip)?;
        let du = if sys.free().is_empty() {
            Vec::new()
        } else {
            match factor_cached(&sys.matrix, factor, cache) {
                Ok(()) => {}
                Err(SkylineError::Pivot { .. }) => {
                    // ADR 0507 amendment 2: the exact tangent is indefinite; this iteration takes eps G G^T.
                    let all = set.evaluate_all(&u)?;
                    let mut scratch = vec![0.0; n];
                    let mut tc = Vec::new();
                    set.force_and_tangent(&all, &mut scratch, Some(&mut tc), false);
                    let convex_sys = build(&tc)?;
                    factor_cached(&convex_sys.matrix, factor, cache).map_err(|e| {
                        fail(format!(
                            "tangent factorisation at Newton iteration {it} (without the curvature part): {e}"
                        ))
                    })?;
                    convex += 1;
                }
                Err(e) => return Err(fail(format!("tangent factorisation at Newton iteration {it}: {e}"))),
            }
            cache.as_ref().expect("factored").1.solve(&sys.rhs)
        };
        if du.iter().any(|v| !v.is_finite()) {
            return Err(fail(format!("non-finite correction at Newton iteration {it}")));
        }
        let e = du.iter().zip(&sys.rhs).map(|(a, b)| a * b).sum::<f64>().abs();
        if it == 0 {
            e_first = e;
        }
        e_last = e;
        energies.push(e);
        for (&q, d) in sys.free().iter().zip(&du) {
            u[q] += d;
        }
    }
    unreachable!("the loop returns at max_iterations")
}

/// The contact solve (ADR 0507 D3) without extra contributions: see [`solve_with`].
pub fn solve(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    pairs: &[ContactPair],
    options: &ContactOptions,
) -> Result<ContactResult> {
    solve_with(model, prescribed, forces, pairs, options, &[])
}

/// The contact solve (ADR 0507 D3): load control `lambda = k/n` of the model's loads, `forces` and the prescribed values
/// (the model's and `prescribed`), per step Newton solves with the contact set and `extra` (after the contact set; they are
/// called with the load factor 1 of the step's load, as by [`solve_from`]) and Uzawa updates of the multipliers until the
/// largest penetration and active gap are within `options.penetration_tolerance` (D2, amendment 3). Errors: the cycle
/// error of D6, the augmentations exhausted (naming the nodes above the tolerance), a failed Newton solve (located by step
/// and augmentation), and with [`Kinematics::TotalLagrangian`] a non-zero model displacement constraint.
pub fn solve_with(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    pairs: &[ContactPair],
    options: &ContactOptions,
    extra: &[&dyn Contribution],
) -> Result<ContactResult> {
    model.validate()?;
    options.check()?;
    let set = ContactSet::new(model, pairs, options)?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    let tl = options.kinematics == Kinematics::TotalLagrangian;
    if tl {
        for (i, c) in model.constraints().iter().enumerate() {
            if let Constraint::Displacement {
                set, component, value, ..
            } = c
            {
                if *value != 0.0 {
                    return Err(FemError::InvalidValue {
                        key: "constraint",
                        reason: format!(
                            "constraint {i} (node set '{set}', component {component}) has the value {value:e}: the total \
                             Lagrangian contact driver takes non-zero prescribed values through `prescribed` only"
                        ),
                    });
                }
            }
        }
    }
    let k_lin = if tl { None } else { Some(stiffness(model)?) };
    let mut states = if tl {
        initial_states(model)?
    } else {
        crate::nonlinear::StateLayout::of(model)?.empty_states()
    };
    // ADR 0542 D3: the total Lagrangian body takes the supernodal choice through its Newton options.
    let mut newton_tl = options.newton.clone();
    if options.factor == DirectFactor::SupernodalNestedDissection {
        newton_tl.factor = DirectFactor::SupernodalNestedDissection;
    }
    let mut cache = None;
    let mut run_reference = 0.0f64;
    let mut u = vec![0.0; n];
    let ns = options.steps;
    let mut steps = Vec::with_capacity(ns);
    let mut nodes = Vec::new();
    let mut reactions = vec![0.0; n];
    let mut iterations = 0;
    let mut last_residual = 0.0;
    for step in 1..=ns {
        let lf = step as f64 / ns as f64;
        let fstep: Vec<f64> = f.iter().map(|v| lf * v).collect();
        let fixed_step: Vec<(usize, f64)> = fixed.iter().map(|&(eq, v)| (eq, lf * v)).collect();
        let pres_step: Vec<Prescribed> = prescribed
            .iter()
            .map(|p| Prescribed::new(p.node, p.component, lf * p.value))
            .collect();
        for &(eq, v) in &fixed_step {
            u[eq] = v;
        }
        let mut solves = Vec::new();
        for aug in 0..=options.max_augmentations {
            let label = format!("load step {step} of {ns}, augmentation {aug}");
            set.begin(&label);
            let out = if tl {
                let mut list: Vec<&dyn Contribution> = vec![&set];
                list.extend_from_slice(extra);
                solve_from(model, &pres_step, &fstep, &u, &states, &newton_tl, &list).map(|e| Solved {
                    reference: 0.0,
                    convex: 0,
                    u: e.u,
                    residuals: e.residuals,
                    energies: e.energies,
                    states: Some(e.states),
                    internal: e.internal_force,
                })
            } else {
                linear_solve(
                    k_lin.as_ref().expect("linear"),
                    &fstep,
                    &fixed_step,
                    &u,
                    &set,
                    extra,
                    &options.newton,
                    options.factor,
                    &mut cache,
                    run_reference,
                )
            };
            let out = match out {
                Ok(o) => o,
                Err(e) => {
                    if let Some(cycle) = set.take_failure() {
                        return Err(cycle);
                    }
                    return Err(match e {
                        FemError::Solver { solver, reason } if solver == SOLVER_NAME => FemError::Solver {
                            solver,
                            reason: format!("{label}: {reason}"),
                        },
                        other => FemError::Solver {
                            solver: SOLVER_NAME,
                            reason: format!("{label}: {other}"),
                        },
                    });
                }
            };
            run_reference = run_reference.max(out.reference);
            iterations += out.residuals.len() - 1;
            last_residual = *out.residuals.last().expect("one residual at least");
            u = out.u;
            let all = set.evaluate_all(&u)?;
            let mut penetration = 0.0f64;
            let mut over = Vec::new();
            for (p, ev) in set.pairs.iter().zip(&all) {
                for (i, e) in ev.iter().enumerate() {
                    if e.kin.is_some() && e.active {
                        let pen = (-e.gap).max(0.0);
                        penetration = penetration.max(pen);
                        // ADR 0507 amendment 3: |g| at every active node (a penetrating node is active).
                        if e.gap.abs() > options.penetration_tolerance {
                            over.push((p.nodes[i], e.gap.abs()));
                        }
                    }
                }
            }
            solves.push(NewtonSolve {
                residuals: out.residuals,
                energies: out.energies,
                active_sets: set.active_sets(),
                penetration,
                convex_iterations: out.convex,
            });
            let accepted = options.max_augmentations == 0 || over.is_empty();
            if accepted {
                if let Some(s) = out.states {
                    states = s;
                }
                {
                    // Reactions at the accepted state with the forces of that solve (before the update).
                    let mut total = out.internal;
                    set.force_and_tangent(&all, &mut total, None, true);
                    for c in extra {
                        let mut fx = vec![0.0; n];
                        let mut t = Vec::new();
                        c.add(&u, 1.0, &mut fx, &mut t).map_err(|e| FemError::Solver {
                            solver: SOLVER_NAME,
                            reason: format!("contribution '{}': {e}", c.name()),
                        })?;
                        for (a, b) in total.iter_mut().zip(&fx) {
                            *a += b;
                        }
                    }
                    reactions = vec![0.0; n];
                    for &(eq, _) in &fixed {
                        reactions[eq] = total[eq] - fstep[eq];
                    }
                    nodes = set.evaluate(&u)?;
                }
                set.update(&all);
                break;
            }
            if aug == options.max_augmentations {
                over.sort_by_key(|a| a.0);
                return Err(FemError::Solver {
                    solver: SOLVER_NAME,
                    reason: format!(
                        "{label}: the gap tolerance {:e} (penetration, or gap at an active node) not met after {} \
                         augmentations; nodes and |gap|: {over:?}",
                        options.penetration_tolerance, options.max_augmentations
                    ),
                });
            }
            set.update(&all);
        }
        steps.push(ContactStep {
            step,
            load_factor: lf,
            solves,
            displacement: u.clone(),
            reactions: reactions.clone(),
            nodes: nodes.clone(),
        });
    }
    let nf = n - fixed.len();
    let result = SolidResult::new(dofs, u)?
        .with_reactions(reactions)?
        .with_stats(SolverStats::new(SOLVER_NAME, nf, iterations, last_residual));
    Ok(ContactResult {
        result,
        steps,
        nodes,
        states,
    })
}
