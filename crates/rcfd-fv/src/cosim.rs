//! Co-simulation participant interface for a structural solver (F-139, ADR 0256). Opt-in and in-process: nothing in the
//! library calls it, and there is no transport, case key or tool. The protocol is described in
//! `docs/cosimulation-protocol.md`.
//!
//! Two participants, a [`Fluid`] and a [`Structure`], exchange the displacement of a wall and the force of the fluid on it
//! through a coupler that owns the mapping between their two interface meshes ([`InterfaceMap`]). One coupling step
//! ([`coupled_step`]) iterates on the nodal force held over the step:
//!
//! ```text
//! checkpoint both;  f^1 = B^T forces()                         (the flow's force at t_n, on the structure's nodes)
//! iterate k: (restore both)  q^k = structure.advance(dt, f^k);  F^k = fluid.advance(dt, H q^k);  ft^k = B^T F^k
//!            r^k = ft^k - f^k;   converged when |r^k| <= tol max(|ft^k|, f_scale)
//!            f^{k+1} = f^k + w_k r^k                           (w_k fixed or Aitken's)
//! ```
//!
//! or, opt-in, the least-squares (interface quasi-Newton) update of ADR 0261 (`Relaxation::LeastSquares`): after the
//! relaxed first update, `f^{k+1} = ft^k + W c` with `c = argmin |V c + r^k|`, `V` and `W` the differences of the residuals
//! and of the outputs within the step (newest first, nearly dependent columns filtered out, nothing kept across steps).
//!
//! `H` takes nodal displacements to the flow's wall points (rows summing to 1: consistent), `B = P H` to the wall faces (`P`
//! the mean over a face's points), and the forces go back with the transpose `B^T`, so that the work and the resultant are
//! the same on both meshes. The fixed-point map has the slopes `-lambda_j/2`, `lambda_j` the added-mass to structural-mass
//! ratios of the coupled modes, so the unrelaxed iteration diverges when the largest exceeds 2 (ADR 0250 D5, ADR 0256 C1).
//!
//! [`FlowWall`] is the fluid participant of this repository (the moving-mesh PISO/PIMPLE step); [`SprungWall`] is a mock
//! structural participant (lumped nodes on springs).

use crate::ale::{move_mesh, set_follow_conditions};
use crate::field::VectorField;
use crate::fsi::{Relaxation, WallPressure};
use crate::geometry::FvGeometry;
use crate::pimple::{PimpleSolver, PimpleState, StepInfo};
use crate::sixdof::patch_face_forces;
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::motion::patch_points;
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// What every participant of a coupled step answers.
pub trait Participant {
    /// Name used in error messages.
    fn name(&self) -> &str;
    /// The participant's time [s].
    fn time(&self) -> f64;
    /// Keep the full state at the current time (replaces an earlier checkpoint).
    fn checkpoint(&mut self) -> Result<(), String>;
    /// Go back to the checkpoint, bit for bit. The checkpoint stays valid.
    fn restore(&mut self) -> Result<(), String>;
}

/// A structural participant: it receives forces on its interface nodes and returns their displacement.
pub trait Structure: Participant {
    /// Interface nodes in the reference configuration [m].
    fn node_coordinates(&self) -> Vec<Vec3>;
    /// Advance by `dt` [s] under the nodal forces of the fluid on the structure [N], held over the step. Returns the
    /// displacement of every node from the reference configuration [m] at the new time.
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String>;
}

/// A fluid participant: it receives the displacement of its wall points and returns the force on its wall faces.
pub trait Fluid: Participant {
    /// Wall points in the reference configuration [m].
    fn point_coordinates(&self) -> Vec<Vec3>;
    /// The points of every wall face (indices into the wall points).
    fn face_points(&self) -> Vec<Vec<usize>>;
    /// Force of the fluid on every wall face [N] at the participant's time.
    fn forces(&self) -> Result<Vec<Vec3>, String>;
    /// Advance by `dt` [s] with the wall at the given displacement from the reference configuration [m] at the new time
    /// (one vector per wall point). Returns the force of the fluid on every wall face [N] at the new time.
    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String>;
}

/// The mapping between a structure's interface nodes and a fluid's wall points and faces (ADR 0256 G2, G3).
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMap {
    rows: Vec<Vec<(usize, f64)>>,
    n_nodes: usize,
    face_points: Vec<Vec<usize>>,
}

impl InterfaceMap {
    /// `rows[p]` are the weights `(node, H_pi)` of wall point `p` (each row sums to 1 within `1e-12`), `face_points[f]` the
    /// wall points of face `f`.
    pub fn new(rows: Vec<Vec<(usize, f64)>>, n_nodes: usize, face_points: Vec<Vec<usize>>) -> Result<Self, String> {
        for (p, row) in rows.iter().enumerate() {
            let mut sum = 0.0;
            for &(i, w) in row {
                if i >= n_nodes {
                    return Err(format!("interface map: wall point {p} refers to node {i} of {n_nodes}"));
                }
                if !w.is_finite() {
                    return Err(format!("interface map: wall point {p} has a non-finite weight"));
                }
                sum += w;
            }
            if (sum - 1.0).abs() > 1e-12 {
                return Err(format!(
                    "interface map: the weights of wall point {p} sum to {sum}, not 1 (an unmapped or partly mapped point)"
                ));
            }
        }
        for (f, pts) in face_points.iter().enumerate() {
            if pts.is_empty() {
                return Err(format!("interface map: wall face {f} has no points"));
            }
            if let Some(&q) = pts.iter().find(|&&q| q >= rows.len()) {
                return Err(format!(
                    "interface map: wall face {f} refers to wall point {q} of {}",
                    rows.len()
                ));
            }
        }
        Ok(InterfaceMap {
            rows,
            n_nodes,
            face_points,
        })
    }

    /// Number of structural nodes.
    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }

    /// Number of wall points of the fluid.
    pub fn n_points(&self) -> usize {
        self.rows.len()
    }

    /// Number of wall faces of the fluid.
    pub fn n_faces(&self) -> usize {
        self.face_points.len()
    }

    /// Displacement of the wall points from the nodal displacements, `d_p = sum_i H_pi q_i` (consistent).
    pub fn displacement_to_points(&self, nodes: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if nodes.len() != self.n_nodes {
            return Err(format!(
                "interface map: {} nodal displacements for {} nodes",
                nodes.len(),
                self.n_nodes
            ));
        }
        Ok(self
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(nodes[i], w)))
            })
            .collect())
    }

    /// Displacement of the wall faces, the mean of each face's points.
    pub fn face_displacements(&self, points: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if points.len() != self.rows.len() {
            return Err(format!(
                "interface map: {} point displacements for {} wall points",
                points.len(),
                self.rows.len()
            ));
        }
        Ok(self
            .face_points
            .iter()
            .map(|pts| {
                let sum = pts.iter().fold(vec3::ZERO, |a, &q| vec3::add(a, points[q]));
                vec3::scale(sum, 1.0 / pts.len() as f64)
            })
            .collect())
    }

    /// Nodal forces from the face forces, the transpose of the displacement map to the faces: each face's force goes in
    /// equal shares to its points and from a point to the nodes with the point's weights (conservative: the resultant and
    /// the work are those of the face forces).
    pub fn forces_to_nodes(&self, faces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if faces.len() != self.face_points.len() {
            return Err(format!(
                "interface map: {} face forces for {} wall faces",
                faces.len(),
                self.face_points.len()
            ));
        }
        let mut nodal = vec![vec3::ZERO; self.n_nodes];
        for (pts, force) in self.face_points.iter().zip(faces) {
            let share = vec3::scale(*force, 1.0 / pts.len() as f64);
            for &q in pts {
                for &(i, w) in &self.rows[q] {
                    nodal[i] = vec3::add(nodal[i], vec3::scale(share, w));
                }
            }
        }
        Ok(nodal)
    }
}

/// Rows of [`InterfaceMap`] for a line of nodes with a piecewise-linear displacement between them: `nodes` are the nodes'
/// coordinates along the line (strictly increasing), `points` the wall points' coordinates along the same line. With one
/// node every point follows it (a rigid segment). A point outside the nodes (by more than `1e-12` of their span) is an
/// error: there is no extrapolation.
pub fn piecewise_linear_rows(nodes: &[f64], points: &[f64]) -> Result<Vec<Vec<(usize, f64)>>, String> {
    if nodes.is_empty() {
        return Err("interface map: no structural node".into());
    }
    if nodes.iter().any(|x| !x.is_finite()) || nodes.windows(2).any(|w| w[1] <= w[0]) {
        return Err("interface map: the node coordinates must be finite and strictly increasing".into());
    }
    if nodes.len() == 1 {
        return Ok(points.iter().map(|_| vec![(0, 1.0)]).collect());
    }
    let (lo, hi) = (nodes[0], nodes[nodes.len() - 1]);
    let slack = 1e-12 * (hi - lo);
    let mut rows = Vec::with_capacity(points.len());
    for (p, &x) in points.iter().enumerate() {
        if !(x >= lo - slack && x <= hi + slack) {
            return Err(format!(
                "interface map: wall point {p} at {x} lies outside the structural nodes [{lo}, {hi}]"
            ));
        }
        let x = x.clamp(lo, hi);
        let e = nodes[1..nodes.len() - 1].partition_point(|&n| n <= x);
        let t = (x - nodes[e]) / (nodes[e + 1] - nodes[e]);
        let row: Vec<(usize, f64)> = [(e, 1.0 - t), (e + 1, t)].into_iter().filter(|r| r.1 != 0.0).collect();
        rows.push(row);
    }
    Ok(rows)
}

/// What the coupler does when the iteration cap is reached without convergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AtCap {
    /// Restore both participants and return an error.
    #[default]
    Stop,
    /// Keep the last iterate and report `converged = false`.
    Keep,
}

/// Settings of the coupled iteration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CouplingSettings {
    /// Relaxation of the nodal force (one factor for all components), or the least-squares update (ADR 0261).
    pub relaxation: Relaxation,
    /// Converged when `|r|/max(|ft|, force_scale) <= tolerance` (Euclidean norms over all nodes and components).
    pub tolerance: f64,
    /// Floor of the force norm in the convergence test [N] (`>= 0`).
    pub force_scale: f64,
    /// Cap of the iterations per step (`>= 1`; 1 is the loose coupling).
    pub max_iterations: usize,
    /// What happens at the cap.
    pub at_cap: AtCap,
}

impl CouplingSettings {
    /// Aitken relaxation from `initial` with the cap 2, tolerance `1e-8`, no floor, at most 20 iterations, stop at the cap.
    pub fn aitken(initial: f64) -> Self {
        CouplingSettings {
            relaxation: Relaxation::Aitken { initial, max: 2.0 },
            tolerance: 1e-8,
            force_scale: 0.0,
            max_iterations: 20,
            at_cap: AtCap::Stop,
        }
    }

    /// The least-squares update (ADR 0261) with the relaxed first update `initial` and the column filter `1e-2`, tolerance
    /// `1e-8`, no floor, at most 20 iterations, stop at the cap.
    pub fn least_squares(initial: f64) -> Self {
        CouplingSettings {
            relaxation: Relaxation::LeastSquares { initial, filter: 1e-2 },
            ..Self::aitken(initial)
        }
    }

    /// Check the settings; the message names the offending one.
    pub fn validate(&self) -> Result<(), String> {
        let factor = |w: f64, what: &str| {
            if w.is_finite() && w > 0.0 && w < 2.0 {
                Ok(())
            } else {
                Err(format!("{what} must lie in (0, 2), got {w}"))
            }
        };
        match self.relaxation {
            Relaxation::Fixed(w) => factor(w, "the fixed relaxation factor")?,
            Relaxation::Aitken { initial, max } => {
                factor(initial, "the initial Aitken factor")?;
                if !(max.is_finite() && max >= initial) {
                    return Err(format!(
                        "the Aitken cap must be finite and at least the initial factor, got {max}"
                    ));
                }
            }
            Relaxation::LeastSquares { initial, filter } => {
                factor(initial, "the initial factor of the least-squares update")?;
                if !(filter.is_finite() && filter > 0.0 && filter < 1.0) {
                    return Err(format!(
                        "the column filter of the least-squares update must lie in (0, 1), got {filter}"
                    ));
                }
            }
        }
        if !(self.tolerance.is_finite() && self.tolerance > 0.0) {
            return Err(format!("the tolerance must be positive, got {}", self.tolerance));
        }
        if !(self.force_scale.is_finite() && self.force_scale >= 0.0) {
            return Err(format!(
                "the force scale must be finite and not negative, got {}",
                self.force_scale
            ));
        }
        if self.max_iterations == 0 {
            return Err("at least one iteration is needed".into());
        }
        Ok(())
    }
}

/// Why a coupled step stopped. Both participants are at the start of the step again.
#[derive(Debug, Clone, PartialEq)]
pub struct CouplingError {
    /// The participant that failed, or `coupler`.
    pub participant: String,
    /// Time at the start of the step [s].
    pub time: f64,
    /// Iteration in which it failed (0: before the first).
    pub iteration: usize,
    /// The participant's message.
    pub message: String,
}

impl std::fmt::Display for CouplingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "co-simulation: '{}' at t = {}, iteration {}: {}",
            self.participant, self.time, self.iteration, self.message
        )
    }
}

impl std::error::Error for CouplingError {}

/// Record of one coupled step.
#[derive(Debug, Clone)]
pub struct CoupledStep {
    /// Passes through both participants.
    pub iterations: usize,
    /// Whether the convergence test was met.
    pub converged: bool,
    /// The test quantity of every iteration.
    pub residuals: Vec<f64>,
    /// The norm of the force residual of every iteration [N].
    pub residual_norms: Vec<f64>,
    /// The relaxation factor applied after every iteration but the last (for the least-squares update: `initial` for a
    /// relaxed update, 1 for a least-squares one).
    pub factors: Vec<f64>,
    /// The number of difference columns of every update (0 for a relaxed update and for the fixed and Aitken factors).
    pub columns: Vec<usize>,
    /// Nodal forces held over the step in the last iterate [N].
    pub node_forces: Vec<Vec3>,
    /// Nodal displacements at the end of the step [m].
    pub node_displacements: Vec<Vec3>,
    /// Face forces of the fluid at the end of the step [N].
    pub face_forces: Vec<Vec3>,
}

/// Aitken's factor for a sequence of residual vectors (ADR 0250 D6), or a fixed one.
struct SliceRelaxer {
    relaxation: Relaxation,
    omega: f64,
    previous: Option<Vec<f64>>,
}

impl SliceRelaxer {
    fn new(relaxation: Relaxation) -> Self {
        let omega = match relaxation {
            Relaxation::Fixed(w) => w,
            Relaxation::Aitken { initial, .. } => initial,
            Relaxation::LeastSquares { initial, .. } => initial,
        };
        SliceRelaxer {
            relaxation,
            omega,
            previous: None,
        }
    }

    fn factor(&mut self, r: &[f64]) -> f64 {
        if let Relaxation::Aitken { max, .. } = self.relaxation {
            if let Some(p) = &self.previous {
                let (mut pd, mut dd) = (0.0, 0.0);
                for (a, b) in r.iter().zip(p) {
                    let d = a - b;
                    pd += b * d;
                    dd += d * d;
                }
                if dd > 0.0 {
                    let w = -self.omega * pd / dd;
                    if w.is_finite() && w != 0.0 {
                        self.omega = w.clamp(-max, max);
                    }
                }
            }
            self.previous = Some(r.to_vec());
        }
        self.omega
    }
}

/// The least-squares update of ADR 0261 within one coupling step: the previous residual and output, and the difference
/// columns `V` (residuals) and `W` (outputs), newest first.
#[derive(Default)]
struct LeastSquaresUpdate {
    previous: Option<(Vec<f64>, Vec<f64>)>,
    v: Vec<Vec<f64>>,
    w: Vec<Vec<f64>>,
}

impl LeastSquaresUpdate {
    /// The next input from the input `f`, the output `ft` and the residual `r = ft - f` of the current iteration, and the
    /// number of columns used (0: the relaxed update `f + initial r`).
    fn update(&mut self, f: &[f64], ft: &[f64], r: &[f64], initial: f64, filter: f64) -> (Vec<f64>, usize) {
        let relaxed = || f.iter().zip(r).map(|(a, b)| a + initial * b).collect::<Vec<f64>>();
        let Some((r_prev, ft_prev)) = self.previous.replace((r.to_vec(), ft.to_vec())) else {
            return (relaxed(), 0);
        };
        self.v.insert(0, r.iter().zip(&r_prev).map(|(a, b)| a - b).collect());
        self.w.insert(0, ft.iter().zip(&ft_prev).map(|(a, b)| a - b).collect());
        let (c, kept) = least_squares(&self.v, r, filter);
        // filtered columns are dropped for the rest of the step
        let keep = |columns: &mut Vec<Vec<f64>>| {
            *columns = kept.iter().map(|&j| std::mem::take(&mut columns[j])).collect();
        };
        keep(&mut self.v);
        keep(&mut self.w);
        if kept.is_empty() || !c.iter().all(|x| x.is_finite()) {
            return (relaxed(), 0);
        }
        let mut next = ft.to_vec();
        for (cj, wj) in c.iter().zip(&self.w) {
            for (x, y) in next.iter_mut().zip(wj) {
                *x += cj * y;
            }
        }
        if !next.iter().all(|x| x.is_finite()) {
            return (relaxed(), 0);
        }
        (next, kept.len())
    }
}

/// `x <- (I - 2 h h^T) x` for a unit vector `h`.
fn reflect(h: &[f64], x: &mut [f64]) {
    let d = 2.0 * h.iter().zip(x.iter()).map(|(a, b)| a * b).sum::<f64>();
    for (xi, hi) in x.iter_mut().zip(h) {
        *xi -= d * hi;
    }
}

/// `c = argmin |V c + r|` over the columns of `v` that pass the filter (ADR 0261 D5), by Householder QR with the columns in
/// the given order: a column is dropped when its part outside the kept columns before it is below `filter` times its norm,
/// when it is zero or not finite, or when every row is used. Returns the coefficients of the kept columns and their
/// indices; every pivot is at least `filter` times its column's norm.
fn least_squares(v: &[Vec<f64>], r: &[f64], filter: f64) -> (Vec<f64>, Vec<usize>) {
    let m = r.len();
    let mut reflectors: Vec<(usize, Vec<f64>)> = Vec::new();
    let mut upper: Vec<Vec<f64>> = Vec::new();
    let mut kept = Vec::new();
    for (j, column) in v.iter().enumerate() {
        let row = reflectors.len();
        let length = norm(column);
        if row >= m || !(length.is_finite() && length > 0.0) {
            continue;
        }
        let mut a = column.clone();
        for (k, h) in &reflectors {
            reflect(h, &mut a[*k..]);
        }
        let s = norm(&a[row..]);
        if s.is_nan() || s < filter * length {
            continue;
        }
        let alpha = if a[row] >= 0.0 { -s } else { s };
        let mut h = a[row..].to_vec();
        h[0] -= alpha;
        let hn = norm(&h);
        for x in &mut h {
            *x /= hn;
        }
        a.truncate(row + 1);
        a[row] = alpha;
        upper.push(a);
        reflectors.push((row, h));
        kept.push(j);
    }
    let mut b: Vec<f64> = r.iter().map(|x| -x).collect();
    for (k, h) in &reflectors {
        reflect(h, &mut b[*k..]);
    }
    let n = kept.len();
    let mut c = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= upper[j][i] * c[j];
        }
        c[i] = s / upper[i][i];
    }
    (c, kept)
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn flat(v: &[Vec3]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

fn checked(v: Vec<Vec3>, n: usize, what: &str) -> Result<Vec<Vec3>, String> {
    if v.len() != n {
        return Err(format!("returned {} {what}, {n} expected", v.len()));
    }
    if !v.iter().flatten().all(|x| x.is_finite()) {
        return Err(format!("returned a non-finite value among the {what}"));
    }
    Ok(v)
}

/// One coupling step of size `dt` of a fluid and a structure (ADR 0256, the protocol): both cover `[t, t + dt]`, the
/// coupler iterates on the nodal force until the force of the flow at the end of the step is the force the structure was
/// advanced with.
///
/// With `max_iterations = 1` this is the loose coupling. At the cap, [`AtCap::Stop`] is an error and [`AtCap::Keep`]
/// returns the last iterate with `converged = false`.
///
/// Fails, with both participants restored to the start of the step, for a participant that returns an error, a wrong
/// array length or a non-finite value or reports a time other than `t + dt` (within `1e-9 dt`), and for the cap with
/// `Stop`. Fails before anything is touched for invalid settings, a `dt` that is not positive, a map that does not fit the
/// participants and participants whose times differ.
pub fn coupled_step(
    fluid: &mut dyn Fluid,
    structure: &mut dyn Structure,
    map: &InterfaceMap,
    dt: f64,
    settings: &CouplingSettings,
) -> Result<CoupledStep, CouplingError> {
    let t0 = fluid.time();
    let fail = |participant: &str, iteration: usize, message: String| CouplingError {
        participant: participant.to_string(),
        time: t0,
        iteration,
        message,
    };
    settings.validate().map_err(|e| fail("coupler", 0, e))?;
    if !(dt.is_finite() && dt > 0.0) {
        return Err(fail(
            "coupler",
            0,
            format!("the coupling time step must be positive, got {dt}"),
        ));
    }
    let (n_nodes, n_points, n_faces) = (map.n_nodes(), map.n_points(), map.n_faces());
    if structure.node_coordinates().len() != n_nodes
        || fluid.point_coordinates().len() != n_points
        || fluid.face_points().len() != n_faces
    {
        return Err(fail(
            "coupler",
            0,
            format!(
                "the map is for {n_nodes} nodes, {n_points} wall points and {n_faces} wall faces; the participants have {}, {} and {}",
                structure.node_coordinates().len(),
                fluid.point_coordinates().len(),
                fluid.face_points().len()
            ),
        ));
    }
    let slack = 1e-9 * dt;
    if (structure.time() - t0).abs() > slack {
        return Err(fail(
            "coupler",
            0,
            format!(
                "the participants are at different times: {} at {t0}, {} at {}",
                fluid.name(),
                structure.name(),
                structure.time()
            ),
        ));
    }
    let (fluid_name, structure_name) = (fluid.name().to_string(), structure.name().to_string());
    fluid.checkpoint().map_err(|e| fail(&fluid_name, 0, e))?;
    structure.checkpoint().map_err(|e| fail(&structure_name, 0, e))?;

    let at_end = |t: f64| {
        if (t - (t0 + dt)).abs() <= slack {
            Ok(())
        } else {
            Err(format!("reports the time {t} after the step, {} expected", t0 + dt))
        }
    };
    let mut run = || -> Result<CoupledStep, CouplingError> {
        let start = fluid
            .forces()
            .and_then(|v| checked(v, n_faces, "face forces"))
            .map_err(|e| fail(&fluid_name, 0, e))?;
        let mut f = map.forces_to_nodes(&start).map_err(|e| fail("coupler", 0, e))?;
        let mut relaxer = SliceRelaxer::new(settings.relaxation);
        let mut least = LeastSquaresUpdate::default();
        let mut out = CoupledStep {
            iterations: 0,
            converged: false,
            residuals: Vec::new(),
            residual_norms: Vec::new(),
            factors: Vec::new(),
            columns: Vec::new(),
            node_forces: f.clone(),
            node_displacements: Vec::new(),
            face_forces: Vec::new(),
        };
        for k in 1..=settings.max_iterations {
            if k > 1 {
                fluid.restore().map_err(|e| fail(&fluid_name, k, e))?;
                structure.restore().map_err(|e| fail(&structure_name, k, e))?;
            }
            let q = structure
                .advance(dt, &f)
                .and_then(|v| checked(v, n_nodes, "node displacements"))
                .map_err(|e| fail(&structure_name, k, e))?;
            at_end(structure.time()).map_err(|e| fail(&structure_name, k, e))?;
            let points = map.displacement_to_points(&q).map_err(|e| fail("coupler", k, e))?;
            let faces = fluid
                .advance(dt, &points)
                .and_then(|v| checked(v, n_faces, "face forces"))
                .map_err(|e| fail(&fluid_name, k, e))?;
            at_end(fluid.time()).map_err(|e| fail(&fluid_name, k, e))?;
            let ft = map.forces_to_nodes(&faces).map_err(|e| fail("coupler", k, e))?;
            let r: Vec<f64> = flat(&ft).iter().zip(flat(&f)).map(|(a, b)| a - b).collect();
            let rn = norm(&r);
            let e = if rn == 0.0 {
                0.0
            } else {
                rn / norm(&flat(&ft)).max(settings.force_scale)
            };
            out.iterations = k;
            out.residuals.push(e);
            out.residual_norms.push(rn);
            out.node_forces.clone_from(&f);
            out.node_displacements = q;
            out.face_forces = faces;
            if e <= settings.tolerance {
                out.converged = true;
                break;
            }
            if k == settings.max_iterations {
                break;
            }
            if let Relaxation::LeastSquares { initial, filter } = settings.relaxation {
                let (next, columns) = least.update(&flat(&f), &flat(&ft), &r, initial, filter);
                out.factors.push(if columns == 0 { initial } else { 1.0 });
                out.columns.push(columns);
                for (fi, x) in f.iter_mut().zip(next.chunks_exact(3)) {
                    *fi = [x[0], x[1], x[2]];
                }
                continue;
            }
            let w = relaxer.factor(&r);
            out.factors.push(w);
            out.columns.push(0);
            for (fi, ri) in f.iter_mut().zip(r.chunks_exact(3)) {
                *fi = vec3::add(*fi, vec3::scale([ri[0], ri[1], ri[2]], w));
            }
        }
        if !out.converged && settings.at_cap == AtCap::Stop {
            return Err(fail(
                "coupler",
                out.iterations,
                format!(
                    "the coupled iteration did not converge in {} iterations (test quantity {:e}, tolerance {:e})",
                    out.iterations,
                    out.residuals.last().copied().unwrap_or(f64::NAN),
                    settings.tolerance
                ),
            ));
        }
        Ok(out)
    };
    match run() {
        Ok(step) => Ok(step),
        Err(mut e) => {
            for r in [fluid.restore(), structure.restore()] {
                if let Err(m) = r {
                    e.message.push_str(&format!("; and the restore failed: {m}"));
                }
            }
            Err(e)
        }
    }
}

/// The caller's mesh motion of a [`FlowWall`]: from the displacement of the wall points (in the order of
/// [`Fluid::point_coordinates`]) to the position of every mesh point (the smoothing).
pub type MeshMotion = Box<dyn FnMut(&[Vec3]) -> Result<Vec<Vec3>, String>>;

struct FlowCheckpoint {
    geom: Arc<FvGeometry>,
    u: VectorField,
    state: PimpleState,
    history: Vec<StepInfo>,
    lag: (usize, usize),
    wall_dpdn: Vec<f64>,
    last_step: Option<StepInfo>,
    mesh_flux: Vec<f64>,
    min_volume_ratio: f64,
}

/// The fluid participant of this repository: a moving-mesh PISO/PIMPLE flow whose wall patch follows the displacement it
/// is given (`ale::move_mesh`, `ale::set_follow_conditions`, `PimpleSolver::step_ale_with_walls`) and returns the load of
/// every wall face (`sixdof::patch_face_forces`).
pub struct FlowWall {
    /// The flow.
    pub flow: PimpleSolver,
    /// Fluid density (the flow solver's pressure is kinematic).
    pub rho: f64,
    /// Kinematic viscosity of the force evaluation.
    pub nu: f64,
    /// How the wall pressure of the force is evaluated (ADR 0252). The constructor sets the prescribed gradient.
    pub wall_pressure: WallPressure,
    /// The fixed-flux pressure gradient of the walls of the last flow step (empty before the first); part of the state.
    pub wall_dpdn: Vec<f64>,
    /// Diagnostics of the last flow step.
    pub last_step: Option<StepInfo>,
    /// Mesh flux of the last mesh move.
    pub mesh_flux: Vec<f64>,
    /// Smallest ratio of new to old cell volume of the last mesh move.
    pub min_volume_ratio: f64,
    name: String,
    patch: Vec<String>,
    reference: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    motion: MeshMotion,
    saved: Option<FlowCheckpoint>,
}

impl std::fmt::Debug for FlowWall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlowWall")
            .field("name", &self.name)
            .field("patch", &self.patch)
            .field("time", &self.flow.time)
            .field("wall_pressure", &self.wall_pressure)
            .finish_non_exhaustive()
    }
}

impl FlowWall {
    /// The wall `patch` of `flow` as a participant. `reference` is the mesh in the reference configuration (the one the
    /// displacements are measured from; same topology as the flow's), `motion` the mesh motion. The wall points are the
    /// points of the patch in ascending order of their mesh index ([`FlowWall::point_ids`]).
    ///
    /// Fails for an unknown patch, a reference mesh of another size and a set `corrector_control` (its schedule would be
    /// reset by the repeated steps).
    pub fn new(
        flow: PimpleSolver,
        patch: &str,
        reference: &Mesh,
        rho: f64,
        nu: f64,
        motion: MeshMotion,
    ) -> Result<Self, String> {
        if flow.settings.corrector_control.is_some() {
            return Err("flow wall: the adaptive corrector count (corrector_control) is not supported".into());
        }
        if reference.n_points() != flow.geom.mesh.n_points() || reference.n_faces() != flow.geom.mesh.n_faces() {
            return Err("flow wall: the reference mesh does not have the topology of the flow's mesh".into());
        }
        let ids = patch_points(reference, patch).ok_or_else(|| format!("flow wall: no patch '{patch}'"))?;
        let p = reference
            .patch(patch)
            .ok_or_else(|| format!("flow wall: no patch '{patch}'"))?;
        let faces = p
            .faces()
            .map(|f| {
                reference
                    .face_points(f)
                    .iter()
                    .map(|q| {
                        ids.binary_search(q)
                            .map_err(|_| "flow wall: a face point is not a patch point".to_string())
                    })
                    .collect::<Result<Vec<usize>, String>>()
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(FlowWall {
            flow,
            rho,
            nu,
            wall_pressure: WallPressure::PrescribedGradient,
            wall_dpdn: Vec::new(),
            last_step: None,
            mesh_flux: Vec::new(),
            min_volume_ratio: 1.0,
            name: "fluid".into(),
            patch: vec![patch.to_string()],
            reference: ids.iter().map(|&q| reference.points()[q]).collect(),
            faces,
            motion,
            saved: None,
        })
    }

    /// Mesh point indices of the wall points of `patch` on `mesh`, in the order of [`Fluid::point_coordinates`].
    pub fn point_ids(mesh: &Mesh, patch: &str) -> Option<Vec<usize>> {
        patch_points(mesh, patch)
    }

    fn face_forces(&self) -> Result<Vec<Vec3>, String> {
        let gradient = (self.wall_pressure == WallPressure::PrescribedGradient && !self.wall_dpdn.is_empty())
            .then_some(self.wall_dpdn.as_slice());
        patch_face_forces(
            &self.flow.geom,
            &self.flow.u,
            &self.flow.p,
            self.rho,
            self.nu,
            &self.patch,
            gradient,
        )
    }
}

impl Participant for FlowWall {
    fn name(&self) -> &str {
        &self.name
    }

    fn time(&self) -> f64 {
        self.flow.time
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some(FlowCheckpoint {
            geom: self.flow.geom.clone(),
            u: self.flow.u.clone(),
            state: self.flow.snapshot(),
            history: self.flow.history.clone(),
            lag: (self.flow.lag_estimates.len(), self.flow.lag_warnings.len()),
            wall_dpdn: self.wall_dpdn.clone(),
            last_step: self.last_step,
            mesh_flux: self.mesh_flux.clone(),
            min_volume_ratio: self.min_volume_ratio,
        });
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        let saved = self.saved.as_ref().ok_or("flow wall: restore without a checkpoint")?;
        self.flow.geom = saved.geom.clone();
        self.flow.u = saved.u.clone();
        self.flow.restore(&saved.state).map_err(|e| e.to_string())?;
        self.flow.history.clone_from(&saved.history);
        self.flow.lag_estimates.truncate(saved.lag.0);
        self.flow.lag_warnings.truncate(saved.lag.1);
        self.wall_dpdn.clone_from(&saved.wall_dpdn);
        self.last_step = saved.last_step;
        self.mesh_flux.clone_from(&saved.mesh_flux);
        self.min_volume_ratio = saved.min_volume_ratio;
        Ok(())
    }
}

impl Fluid for FlowWall {
    fn point_coordinates(&self) -> Vec<Vec3> {
        self.reference.clone()
    }

    fn face_points(&self) -> Vec<Vec<usize>> {
        self.faces.clone()
    }

    fn forces(&self) -> Result<Vec<Vec3>, String> {
        self.face_forces()
    }

    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if displacements.len() != self.reference.len() {
            return Err(format!(
                "flow wall: {} displacements for {} wall points",
                displacements.len(),
                self.reference.len()
            ));
        }
        let points = (self.motion)(displacements)?;
        let old = self.flow.geom.clone();
        let step = move_mesh(&old, points, dt)?;
        let walls = set_follow_conditions(&mut self.flow.u, &step, &old.mesh, &self.patch, dt)?;
        self.min_volume_ratio = step.min_volume_ratio;
        self.mesh_flux.clone_from(&step.mesh_flux);
        self.wall_dpdn.clone_from(&walls);
        self.last_step = Some(self.flow.step_ale_with_walls(dt, step.geom, step.mesh_flux, walls)?);
        if !self.flow.u.values.iter().flatten().all(|x| x.is_finite()) {
            return Err("flow wall: non-finite velocity".into());
        }
        self.face_forces()
    }
}

/// A mock structural participant (ADR 0256 P1, G1): a wall of lumped nodes that move along one direction, each with its
/// own mass and its own spring about the reference position and nothing between neighbours (an elastic foundation with
/// mass), advanced by the classical Runge-Kutta scheme with the nodal force held over the step.
#[derive(Debug, Clone)]
pub struct SprungWall {
    /// Displacement of every node along the direction [m].
    pub q: Vec<f64>,
    /// Velocity of every node along the direction [m/s].
    pub v: Vec<f64>,
    /// Time [s].
    pub t: f64,
    name: String,
    nodes: Vec<Vec3>,
    direction: Vec3,
    mass: Vec<f64>,
    stiffness: Vec<f64>,
    saved: Option<(Vec<f64>, Vec<f64>, f64)>,
}

impl SprungWall {
    /// Nodes at `nodes` (reference configuration) moving along `direction` (made a unit vector), with the masses `mass`
    /// [kg] and the springs `stiffness` [N/m], at rest at the displacements `q0` at time 0.
    pub fn new(
        nodes: Vec<Vec3>,
        direction: Vec3,
        mass: Vec<f64>,
        stiffness: Vec<f64>,
        q0: Vec<f64>,
    ) -> Result<Self, String> {
        let n = nodes.len();
        if n == 0 || mass.len() != n || stiffness.len() != n || q0.len() != n {
            return Err(format!(
                "sprung wall: {n} nodes, {} masses, {} springs, {} initial displacements",
                mass.len(),
                stiffness.len(),
                q0.len()
            ));
        }
        if mass.iter().any(|m| !(m.is_finite() && *m > 0.0)) {
            return Err("sprung wall: every mass must be positive".into());
        }
        if stiffness.iter().any(|k| !(k.is_finite() && *k >= 0.0)) || q0.iter().any(|x| !x.is_finite()) {
            return Err("sprung wall: springs must be finite and not negative, displacements finite".into());
        }
        let length = vec3::norm(direction);
        if !(length.is_finite() && length > 0.0) {
            return Err("sprung wall: the direction must not be zero".into());
        }
        Ok(SprungWall {
            v: vec![0.0; n],
            q: q0,
            t: 0.0,
            name: "structure".into(),
            nodes,
            direction: vec3::scale(direction, 1.0 / length),
            mass,
            stiffness,
            saved: None,
        })
    }

    /// The direction of motion (unit vector).
    pub fn direction(&self) -> Vec3 {
        self.direction
    }

    /// Masses of the nodes [kg].
    pub fn mass(&self) -> &[f64] {
        &self.mass
    }
}

impl Participant for SprungWall {
    fn name(&self) -> &str {
        &self.name
    }

    fn time(&self) -> f64 {
        self.t
    }

    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some((self.q.clone(), self.v.clone(), self.t));
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        let (q, v, t) = self.saved.as_ref().ok_or("sprung wall: restore without a checkpoint")?;
        self.q.clone_from(q);
        self.v.clone_from(v);
        self.t = *t;
        Ok(())
    }
}

impl Structure for SprungWall {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.nodes.clone()
    }

    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if forces.len() != self.q.len() {
            return Err(format!(
                "sprung wall: {} forces for {} nodes",
                forces.len(),
                self.q.len()
            ));
        }
        if !(dt.is_finite() && dt > 0.0) || !forces.iter().flatten().all(|x| x.is_finite()) {
            return Err("sprung wall: the step must be positive and the forces finite".into());
        }
        for (i, force) in forces.iter().enumerate() {
            let (m, k, f) = (self.mass[i], self.stiffness[i], vec3::dot(*force, self.direction));
            let acc = |x: f64| (f - k * x) / m;
            let (x0, v0) = (self.q[i], self.v[i]);
            let (k1x, k1v) = (v0, acc(x0));
            let (k2x, k2v) = (v0 + 0.5 * dt * k1v, acc(x0 + 0.5 * dt * k1x));
            let (k3x, k3v) = (v0 + 0.5 * dt * k2v, acc(x0 + 0.5 * dt * k2x));
            let (k4x, k4v) = (v0 + dt * k3v, acc(x0 + dt * k3x));
            self.q[i] = x0 + dt / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
            self.v[i] = v0 + dt / 6.0 * (k1v + 2.0 * k2v + 2.0 * k3v + k4v);
        }
        self.t += dt;
        Ok(self.q.iter().map(|x| vec3::scale(self.direction, *x)).collect())
    }
}
