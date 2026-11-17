//! Streamlines of a stored velocity field (ADR 0738 P2, F-809 row E-809b).
//!
//! - Velocity at a point (ADR 0738 Q4 (a)): linear (barycentric) over the tetrahedron of the decomposition of
//!   [`crate::surfaces`] (cell centre, face centre, two consecutive face points) that holds the point, from the values at
//!   its four vertices reconstructed as for iso-surfaces ([`crate::iso::tet_vertex_values`]). The field is continuous
//!   across faces, which a cell-constant field is not (Kenwright and Lane 1996 trace in tets the same way).
//! - Integration (Q3 (a)): classical fourth-order Runge-Kutta (Runge 1895, Kutta 1901; Hairer, Nørsett and Wanner 1993)
//!   of `dx/ds = ±u/|u|` in arc length `s`, with the step `h = step_fraction x` the length of the current cell
//!   (`rcfd_mesh::adapt::cell_lengths`, default 0.25). Darmofal and Haimes 1996 discuss the error of such integrators on
//!   interpolated fields.
//! - The point is followed from cell to neighbour across the face it is outside of (Löhner and Ambrosiano 1990); when the
//!   walk reaches a boundary face, every cell is tried before the point is said to have left the domain.
//!
//! A line stops when a stage point is outside the mesh (`left_domain`), the speed falls below `min_speed` (`stagnant`),
//! after `max_steps` steps or `max_length` of arc length, or when the velocity is not finite (`cell_walk_failed`).

use crate::iso::tet_vertex_values;
use crate::surfaces::tet_vertex_position;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;

/// Which way a line is followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Along the velocity.
    Forward,
    /// Against it.
    Backward,
}

impl Direction {
    /// `forward` or `backward`.
    pub fn name(self) -> &'static str {
        match self {
            Direction::Forward => "forward",
            Direction::Backward => "backward",
        }
    }
}

/// The directions traced from each seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directions {
    /// Forward only.
    Forward,
    /// Backward only.
    Backward,
    /// A backward line, then a forward line, per seed.
    Both,
}

/// Why a line stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// A stage point lies outside the mesh (or the seed does).
    LeftDomain,
    /// The speed fell below `min_speed`.
    Stagnant,
    /// `max_steps` steps were made.
    MaxSteps,
    /// `max_length` of arc length was reached.
    MaxLength,
    /// The velocity at a point was not finite.
    CellWalkFailed,
}

impl Stop {
    /// The name in replies.
    pub fn name(self) -> &'static str {
        match self {
            Stop::LeftDomain => "left_domain",
            Stop::Stagnant => "stagnant",
            Stop::MaxSteps => "max_steps",
            Stop::MaxLength => "max_length",
            Stop::CellWalkFailed => "cell_walk_failed",
        }
    }
}

/// Options of [`trace`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceOptions {
    /// Directions traced.
    pub direction: Directions,
    /// Steps per line at most (default 2000).
    pub max_steps: usize,
    /// Arc length per line at most; `None`: 4 x the diagonal of the mesh's bounding box.
    pub max_length: Option<f64>,
    /// A line stops where the speed is below this; `None`: 1e-9 x the largest stored speed.
    pub min_speed: Option<f64>,
    /// Step as a part of the current cell's length (default 0.25).
    pub step_fraction: f64,
}

impl Default for TraceOptions {
    fn default() -> Self {
        TraceOptions {
            direction: Directions::Forward,
            max_steps: 2000,
            max_length: None,
            min_speed: None,
            step_fraction: 0.25,
        }
    }
}

/// One streamline.
#[derive(Debug, Clone, PartialEq)]
pub struct Streamline {
    /// The seed it starts from (index into the seeds).
    pub seed: usize,
    /// Points, the seed first.
    pub points: Vec<Vec3>,
    /// The cell of each point.
    pub cells: Vec<usize>,
    /// The speed at each point.
    pub speed: Vec<f64>,
    /// Which way it was followed.
    pub direction: Direction,
    /// Why it stopped.
    pub stop: Stop,
}

impl Streamline {
    /// Arc length.
    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|w| vec3::norm(vec3::sub(w[1], w[0]))).sum()
    }
}

/// Finds the cell of a point: convex cells, the point on the inner side of every face plane.
pub struct Locator<'a> {
    mesh: &'a Mesh,
}

impl<'a> Locator<'a> {
    /// A locator of `mesh`.
    pub fn new(mesh: &'a Mesh) -> Self {
        Locator { mesh }
    }

    /// The largest signed distance of `p` outside a face plane of cell `c` (negative inside), and that face.
    fn outside(&self, c: usize, p: Vec3) -> (f64, usize) {
        let m = self.mesh;
        let (own, cf, sf) = (m.owner(), m.face_centres(), m.face_area_vectors());
        let mut worst = (f64::NEG_INFINITY, usize::MAX);
        for &f in m.cell_faces(c) {
            let s = if own[f] == c { sf[f] } else { vec3::scale(sf[f], -1.0) };
            let d = vec3::dot(vec3::sub(p, cf[f]), s) / vec3::norm(s).max(1e-300);
            if d > worst.0 {
                worst = (d, f);
            }
        }
        worst
    }

    fn tol(&self, c: usize) -> f64 {
        let m = self.mesh;
        let cc = m.cell_centres()[c];
        let r = m
            .cell_faces(c)
            .iter()
            .map(|&f| vec3::norm(vec3::sub(m.face_centres()[f], cc)))
            .fold(0.0, f64::max);
        1e-12 * r.max(1e-300)
    }

    /// Whether cell `c` holds `p`.
    pub fn contains(&self, c: usize, p: Vec3) -> bool {
        self.outside(c, p).0 <= self.tol(c)
    }

    /// The cell holding `p`: walked to from `hint` across the faces `p` is outside of, else every cell is tried (the cell
    /// with the nearest centre first).
    pub fn locate(&self, p: Vec3, hint: Option<usize>) -> Option<usize> {
        let m = self.mesh;
        if let Some(mut c) = hint {
            for _ in 0..256 {
                let (d, f) = self.outside(c, p);
                if d <= self.tol(c) {
                    return Some(c);
                }
                if f >= m.n_internal_faces() {
                    break;
                }
                c = if m.owner()[f] == c {
                    m.neighbour()[f]
                } else {
                    m.owner()[f]
                };
            }
        }
        let cc = m.cell_centres();
        let d2 = |c: usize| {
            let r = vec3::sub(cc[c], p);
            vec3::dot(r, r)
        };
        let nearest = (0..m.n_cells()).min_by(|&a, &b| d2(a).total_cmp(&d2(b)))?;
        if self.contains(nearest, p) {
            return Some(nearest);
        }
        (0..m.n_cells()).find(|&c| self.contains(c, p))
    }
}

/// The tet of cell `c`'s decomposition that holds `p` (the one with the largest smallest barycentric coordinate) and its
/// barycentric coordinates: `(vertex ids, weights)`.
pub fn tet_weights(mesh: &Mesh, c: usize, p: Vec3) -> ([usize; 4], [f64; 4]) {
    let (np, nf) = (mesh.n_points(), mesh.n_faces());
    let cid = np + nf + c;
    let mut best = ([cid; 4], [1.0, 0.0, 0.0, 0.0]);
    let mut best_min = f64::NEG_INFINITY;
    for &f in mesh.cell_faces(c) {
        let fp = mesh.face_points(f);
        for k in 0..fp.len() {
            let ids = [cid, np + f, fp[k], fp[(k + 1) % fp.len()]];
            let v = ids.map(|i| tet_vertex_position(mesh, i));
            let (a, b, cc) = (vec3::sub(v[1], v[0]), vec3::sub(v[2], v[0]), vec3::sub(v[3], v[0]));
            let det = vec3::dot(a, vec3::cross(b, cc));
            if det.abs() < 1e-300 {
                continue;
            }
            let r = vec3::sub(p, v[0]);
            let l1 = vec3::dot(r, vec3::cross(b, cc)) / det;
            let l2 = vec3::dot(a, vec3::cross(r, cc)) / det;
            let l3 = vec3::dot(a, vec3::cross(b, r)) / det;
            let w = [1.0 - l1 - l2 - l3, l1, l2, l3];
            let mn = w.iter().copied().fold(f64::INFINITY, f64::min);
            if mn > best_min {
                best_min = mn;
                best = (ids, w);
                if mn >= 0.0 {
                    return best;
                }
            }
        }
    }
    best
}

/// The velocity of a stored cell field, linear over the tets (see the module comment).
pub struct TetVelocity<'a> {
    mesh: &'a Mesh,
    values: [Vec<f64>; 3],
}

impl<'a> TetVelocity<'a> {
    /// From the three cell components.
    pub fn new(mesh: &'a Mesh, u: [&[f64]; 3]) -> Result<Self> {
        Ok(TetVelocity {
            mesh,
            values: [
                tet_vertex_values(mesh, u[0])?,
                tet_vertex_values(mesh, u[1])?,
                tet_vertex_values(mesh, u[2])?,
            ],
        })
    }

    /// The velocity at `p` in cell `c`.
    pub fn at(&self, c: usize, p: Vec3) -> Vec3 {
        let (ids, w) = tet_weights(self.mesh, c, p);
        let mut u = [0.0; 3];
        for (k, uk) in u.iter_mut().enumerate() {
            *uk = (0..4).map(|i| w[i] * self.values[k][ids[i]]).sum();
        }
        u
    }
}

/// Bounding box diagonal of the mesh points.
pub fn diagonal(mesh: &Mesh) -> f64 {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in mesh.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    vec3::norm(vec3::sub(hi, lo))
}

/// Streamlines of the stored cell velocity `u` (three components) from `seeds` (see the module comment).
pub fn trace(mesh: &Mesh, u: [&[f64]; 3], seeds: &[Vec3], opts: &TraceOptions) -> Result<Vec<Streamline>> {
    let n = mesh.n_cells();
    if u.iter().any(|c| c.len() != n) {
        return Err(Error::Other(format!(
            "trace: velocity components need {n} cell values each"
        )));
    }
    let vmax = (0..n)
        .map(|c| vec3::norm([u[0][c], u[1][c], u[2][c]]))
        .filter(|x| x.is_finite())
        .fold(0.0, f64::max);
    let tv = TetVelocity::new(mesh, u)?;
    let opts = TraceOptions {
        min_speed: Some(opts.min_speed.unwrap_or(1e-9 * vmax)),
        ..*opts
    };
    trace_with(mesh, &|c, p| tv.at(c, p), seeds, &opts)
}

/// [`trace`] with any velocity of (cell, point): the tests give an analytic one.
pub fn trace_with(
    mesh: &Mesh,
    velocity: &dyn Fn(usize, Vec3) -> Vec3,
    seeds: &[Vec3],
    opts: &TraceOptions,
) -> Result<Vec<Streamline>> {
    if !(opts.step_fraction.is_finite() && opts.step_fraction > 0.0) {
        return Err(Error::Other(format!(
            "trace: step_fraction {} must be positive",
            opts.step_fraction
        )));
    }
    let lengths = rcfd_mesh::adapt::cell_lengths(mesh, [true; 3]);
    let max_length = opts.max_length.unwrap_or(4.0 * diagonal(mesh));
    let min_speed = opts.min_speed.unwrap_or(0.0);
    let loc = Locator::new(mesh);
    let dirs: &[Direction] = match opts.direction {
        Directions::Forward => &[Direction::Forward],
        Directions::Backward => &[Direction::Backward],
        Directions::Both => &[Direction::Backward, Direction::Forward],
    };
    let mut out = Vec::new();
    for (si, &seed) in seeds.iter().enumerate() {
        let start = loc.locate(seed, None);
        for &dir in dirs {
            out.push(one(
                &loc, velocity, &lengths, si, seed, start, dir, opts, max_length, min_speed,
            ));
        }
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn one(
    loc: &Locator,
    velocity: &dyn Fn(usize, Vec3) -> Vec3,
    lengths: &[f64],
    seed_index: usize,
    seed: Vec3,
    start: Option<usize>,
    direction: Direction,
    opts: &TraceOptions,
    max_length: f64,
    min_speed: f64,
) -> Streamline {
    let mut line = Streamline {
        seed: seed_index,
        points: Vec::new(),
        cells: Vec::new(),
        speed: Vec::new(),
        direction,
        stop: Stop::LeftDomain,
    };
    let Some(mut c) = start else {
        return line;
    };
    let sign = if direction == Direction::Forward { 1.0 } else { -1.0 };
    let mut x = seed;
    let u0 = velocity(c, x);
    let s0 = vec3::norm(u0);
    line.points.push(x);
    line.cells.push(c);
    line.speed.push(s0);
    if !s0.is_finite() {
        line.stop = Stop::CellWalkFailed;
        return line;
    }
    if s0 < min_speed || s0 == 0.0 {
        line.stop = Stop::Stagnant;
        return line;
    }
    let mut length = 0.0;
    // the unit direction at a point, or why there is none
    let dir_at = |p: Vec3, hint: usize| -> std::result::Result<(Vec3, usize), Stop> {
        let cell = loc.locate(p, Some(hint)).ok_or(Stop::LeftDomain)?;
        let u = velocity(cell, p);
        let s = vec3::norm(u);
        if !s.is_finite() {
            return Err(Stop::CellWalkFailed);
        }
        if s < min_speed || s == 0.0 {
            return Err(Stop::Stagnant);
        }
        Ok((vec3::scale(u, sign / s), cell))
    };
    let mut steps = 0;
    loop {
        if steps >= opts.max_steps {
            line.stop = Stop::MaxSteps;
            return line;
        }
        if length >= max_length {
            line.stop = Stop::MaxLength;
            return line;
        }
        let h = (opts.step_fraction * lengths[c]).min(max_length - length);
        let stage = || -> std::result::Result<Vec3, Stop> {
            let (k1, c1) = dir_at(x, c)?;
            let (k2, c2) = dir_at(vec3::add(x, vec3::scale(k1, 0.5 * h)), c1)?;
            let (k3, c3) = dir_at(vec3::add(x, vec3::scale(k2, 0.5 * h)), c2)?;
            let (k4, _) = dir_at(vec3::add(x, vec3::scale(k3, h)), c3)?;
            let mut inc = [0.0; 3];
            for k in 0..3 {
                inc[k] = h / 6.0 * (k1[k] + 2.0 * k2[k] + 2.0 * k3[k] + k4[k]);
            }
            Ok(vec3::add(x, inc))
        };
        let next = match stage() {
            Ok(p) => p,
            Err(why) => {
                line.stop = why;
                return line;
            }
        };
        let Some(cn) = loc.locate(next, Some(c)) else {
            line.stop = Stop::LeftDomain;
            return line;
        };
        let u = velocity(cn, next);
        length += vec3::norm(vec3::sub(next, x));
        x = next;
        c = cn;
        line.points.push(x);
        line.cells.push(c);
        line.speed.push(vec3::norm(u));
        steps += 1;
    }
}

/// `n` seeds evenly from `p0` to `p1` (both ends; `p0` alone for `n = 1`).
pub fn rake(p0: Vec3, p1: Vec3, n: usize) -> Vec<Vec3> {
    match n {
        0 => Vec::new(),
        1 => vec![p0],
        _ => (0..n)
            .map(|i| vec3::add(p0, vec3::scale(vec3::sub(p1, p0), i as f64 / (n - 1) as f64)))
            .collect(),
    }
}

/// At most `n` seeds at face centres of the patch `name`: every k-th face, `k = ceil(faces / n)`, nudged a millionth of
/// the owner's centre distance inward so the seed is inside its cell.
pub fn patch_seeds(mesh: &Mesh, name: &str, n: usize) -> Result<Vec<Vec3>> {
    let p = mesh
        .patches()
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| Error::Mesh(format!("unknown patch '{name}'")))?;
    if n == 0 || p.size == 0 {
        return Ok(Vec::new());
    }
    let k = p.size.div_ceil(n);
    let (cf, cc, own) = (mesh.face_centres(), mesh.cell_centres(), mesh.owner());
    Ok((p.start..p.start + p.size)
        .step_by(k)
        .map(|f| vec3::add(cf[f], vec3::scale(vec3::sub(cc[own[f]], cf[f]), 1e-6)))
        .collect())
}
