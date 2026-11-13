//! Weights of an [`InterfaceMap`](super::InterfaceMap) built from the two interface meshes (F-362, ADR 0263): the
//! structure's nodes and elements, and the fluid's wall points.
//!
//! A map with one weight per (wall point, node) pair reproduces every rigid motion of the structure (translation and any
//! rotation) exactly when every row sums to 1 and reproduces the point's own coordinates, `sum_i H_pi x_i = x_p`; and its
//! transpose then conserves the resultant and the moment of the face forces, while it conserves their work for any
//! weights (ADR 0263 D1, D2). Two builders:
//!
//! - [`MapMethod::Projection`]: the barycentric coordinates of the wall point's nearest point on the nearest structural
//!   element, plus (by default) the smallest correction on the element's ring of neighbours that removes the offset of
//!   the point from the element (a curved wall);
//! - [`MapMethod::BilinearProjection`] (opt-in, F-504, ADR 0414): as the projection, but on a quadrilateral the nearest
//!   point of its bilinear patch and the bilinear shape functions there instead of the split into two triangles;
//! - [`MapMethod::RadialBasis`]: interpolation with Wendland's compactly supported function `(1 - r)^4 (4 r + 1)` and a
//!   degree-1 polynomial built from the nodes' own directions, solved densely by Cholesky factorisations.
//!
//! A wall point farther than `max_distance` from the structure is refused, or follows its nearest structural point
//! ([`Unmapped`]).

use rcfd_core::vec3::{self, Vec3};
use std::collections::BTreeMap;

/// How the weights are built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapMethod {
    /// Nearest-element projection. With `correct_offset` the row is corrected on the ring of elements around the
    /// nearest one so that it reproduces the wall point's coordinates (exact rigid motion on a curved wall; some weights
    /// negative); without, the plain barycentric row (non-negative, one element).
    Projection {
        /// Apply the offset correction.
        correct_offset: bool,
    },
    /// Nearest-element projection that evaluates the bilinear shape functions on quadrilaterals at the nearest point of
    /// the bilinear patch (ADR 0414); points, segments and triangles as [`MapMethod::Projection`]. The offset correction
    /// as there.
    BilinearProjection {
        /// Apply the offset correction.
        correct_offset: bool,
    },
    /// Radial-basis interpolation with Wendland's `C^2` function of compact support `support` [m] and a degree-1
    /// polynomial.
    RadialBasis {
        /// Support radius [m] (`> 0`).
        support: f64,
    },
}

/// What happens to a wall point farther than `max_distance` from the structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unmapped {
    /// The build fails, naming the point.
    #[default]
    Refuse,
    /// The point moves with its nearest point on the structure (the method evaluated there).
    Nearest,
}

/// Settings of [`interface_rows`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapSettings {
    /// The builder.
    pub method: MapMethod,
    /// Largest distance [m] of a wall point from the structure that counts as on it (`>= 0`).
    pub max_distance: f64,
    /// Policy for a point farther away.
    pub unmapped: Unmapped,
}

impl MapSettings {
    /// Nearest-element projection with the offset correction; points farther than `max_distance` are refused.
    pub fn projection(max_distance: f64) -> Self {
        MapSettings {
            method: MapMethod::Projection { correct_offset: true },
            max_distance,
            unmapped: Unmapped::Refuse,
        }
    }

    /// Bilinear projection on quadrilaterals (ADR 0414) with the offset correction; points farther than `max_distance` are
    /// refused.
    pub fn bilinear_projection(max_distance: f64) -> Self {
        MapSettings {
            method: MapMethod::BilinearProjection { correct_offset: true },
            max_distance,
            unmapped: Unmapped::Refuse,
        }
    }

    /// Radial-basis interpolation of support `support`; points farther than `max_distance` are refused.
    pub fn radial_basis(support: f64, max_distance: f64) -> Self {
        MapSettings {
            method: MapMethod::RadialBasis { support },
            max_distance,
            unmapped: Unmapped::Refuse,
        }
    }

    /// Check the settings; the message names the offending one.
    pub fn validate(&self) -> Result<(), String> {
        if let MapMethod::RadialBasis { support } = self.method {
            if !(support.is_finite() && support > 0.0) {
                return Err(format!(
                    "interface map: the support radius must be positive, got {support}"
                ));
            }
        }
        if !(self.max_distance.is_finite() && self.max_distance >= 0.0) {
            return Err(format!(
                "interface map: max_distance must be finite and not negative, got {}",
                self.max_distance
            ));
        }
        Ok(())
    }
}

/// What the builder found.
#[derive(Debug, Clone, PartialEq)]
pub struct MapReport {
    /// Distance of every wall point from the structure [m].
    pub distances: Vec<f64>,
    /// The wall points farther than `max_distance` (mapped by [`Unmapped::Nearest`]).
    pub outside: Vec<usize>,
    /// Largest `|sum_i H_pi x_i - x_p|` over the points inside [m]: zero to rounding unless the structure cannot represent
    /// a point's offset (a flat or line structure with points off it).
    pub precision_defect: f64,
    /// Radial basis: an estimate (from below) of the condition number of the kernel matrix.
    pub condition: Option<f64>,
}

/// Rows of weights `(node, H_pi)`, one per wall point.
pub type Rows = Vec<Vec<(usize, f64)>>;

/// Directions whose spread is below this fraction of the largest are not counted (ADR 0263 D5).
const RANK: f64 = 1e-6;
/// Steps of the power and inverse iterations of the condition estimate.
const POWER_STEPS: usize = 100;

/// Rows of [`InterfaceMap`](super::InterfaceMap) for the wall `points` from the structure's `nodes` and `elements` (each
/// 1 to 4 node indices: a point, a segment, a triangle, a quadrilateral split into `(0, 1, 2)` and `(0, 2, 3)`, or with
/// [`MapMethod::BilinearProjection`] its bilinear patch).
pub fn interface_rows(
    nodes: &[Vec3],
    elements: &[Vec<usize>],
    points: &[Vec3],
    settings: &MapSettings,
) -> Result<(Rows, MapReport), String> {
    settings.validate()?;
    if nodes.is_empty() {
        return Err("interface map: no structural node".into());
    }
    if let Some(i) = nodes.iter().position(|x| !x.iter().all(|c| c.is_finite())) {
        return Err(format!(
            "interface map: structural node {i} has a non-finite coordinate"
        ));
    }
    if let Some(p) = points.iter().position(|x| !x.iter().all(|c| c.is_finite())) {
        return Err(format!("interface map: wall point {p} has a non-finite coordinate"));
    }
    if elements.is_empty() {
        return Err("interface map: no structural element".into());
    }
    for (e, el) in elements.iter().enumerate() {
        if el.is_empty() || el.len() > 4 {
            return Err(format!(
                "interface map: structural element {e} has {} nodes (1 to 4 allowed)",
                el.len()
            ));
        }
        if let Some(&i) = el.iter().find(|&&i| i >= nodes.len()) {
            return Err(format!(
                "interface map: structural element {e} refers to node {i} of {}",
                nodes.len()
            ));
        }
    }
    let rbf = match settings.method {
        MapMethod::RadialBasis { support } => Some(RadialBasis::new(nodes, support)?),
        MapMethod::Projection { .. } | MapMethod::BilinearProjection { .. } => None,
    };
    let bilinear = matches!(settings.method, MapMethod::BilinearProjection { .. });
    let mut rings = Rings::new(nodes.len(), elements);
    let mut report = MapReport {
        distances: Vec::with_capacity(points.len()),
        outside: Vec::new(),
        precision_defect: 0.0,
        condition: rbf.as_ref().map(|r| r.condition),
    };
    let mut rows = Vec::with_capacity(points.len());
    for (p, &x) in points.iter().enumerate() {
        let near = nearest(nodes, elements, x, bilinear);
        report.distances.push(near.distance);
        let outside = near.distance > settings.max_distance;
        if outside {
            if settings.unmapped == Unmapped::Refuse {
                return Err(format!(
                    "interface map: wall point {p} at {:?} lies {} from the structure, more than max_distance {} \
                     (refused; Unmapped::Nearest maps it to its nearest structural point)",
                    x.map(short),
                    short(near.distance),
                    settings.max_distance
                ));
            }
            report.outside.push(p);
        }
        let row = match (&rbf, settings.method) {
            (Some(r), _) => r.row(if outside { near.foot } else { x }),
            (None, MapMethod::Projection { correct_offset })
            | (None, MapMethod::BilinearProjection { correct_offset }) => {
                let mut row = near.weights.clone();
                if correct_offset && !outside {
                    correct(&mut row, nodes, rings.ring(elements, near.element), x);
                }
                row
            }
            (None, MapMethod::RadialBasis { .. }) => unreachable!("the radial basis is built above"),
        };
        if !outside {
            let image = row
                .iter()
                .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(nodes[i], w)));
            report.precision_defect = report.precision_defect.max(vec3::norm(vec3::sub(image, x)));
        }
        rows.push(row);
    }
    Ok((rows, report))
}

/// `x` rounded to 12 significant digits (for messages).
fn short(x: f64) -> f64 {
    format!("{x:.11e}").parse().unwrap_or(x)
}

// ---- nearest point on the structure (ADR 0263 D4) ------------------------------------------------------------------

struct Near {
    distance: f64,
    element: usize,
    foot: Vec3,
    weights: Vec<(usize, f64)>,
}

/// The nearest point of the first element at the smallest distance.
fn nearest(nodes: &[Vec3], elements: &[Vec<usize>], x: Vec3, bilinear: bool) -> Near {
    let mut best: Option<Near> = None;
    for (e, el) in elements.iter().enumerate() {
        let (foot, weights) = if bilinear && el.len() == 4 {
            on_bilinear(nodes, el, x)
        } else {
            nearest_on_element(nodes, el, x)
        };
        let distance = vec3::norm(vec3::sub(x, foot));
        if best.as_ref().map_or(true, |b| distance < b.distance) {
            best = Some(Near {
                distance,
                element: e,
                foot,
                weights,
            });
        }
    }
    best.expect("at least one element")
}

fn nearest_on_element(nodes: &[Vec3], el: &[usize], x: Vec3) -> (Vec3, Vec<(usize, f64)>) {
    let pick = |local: &[(usize, f64)]| -> Vec<(usize, f64)> {
        let mut out: BTreeMap<usize, f64> = BTreeMap::new();
        for &(k, w) in local {
            *out.entry(el[k]).or_insert(0.0) += w;
        }
        out.into_iter().filter(|r| r.1 != 0.0).collect()
    };
    match el.len() {
        1 => (nodes[el[0]], vec![(el[0], 1.0)]),
        2 => {
            let (y, t) = on_segment(nodes[el[0]], nodes[el[1]], x);
            (y, pick(&[(0, 1.0 - t), (1, t)]))
        }
        3 => {
            let (y, l) = on_triangle(nodes[el[0]], nodes[el[1]], nodes[el[2]], x);
            (y, pick(&[(0, l[0]), (1, l[1]), (2, l[2])]))
        }
        _ => {
            let (y1, l1) = on_triangle(nodes[el[0]], nodes[el[1]], nodes[el[2]], x);
            let (y2, l2) = on_triangle(nodes[el[0]], nodes[el[2]], nodes[el[3]], x);
            if vec3::norm(vec3::sub(x, y2)) < vec3::norm(vec3::sub(x, y1)) {
                (y2, pick(&[(0, l2[0]), (2, l2[1]), (3, l2[2])]))
            } else {
                (y1, pick(&[(0, l1[0]), (1, l1[1]), (2, l1[2])]))
            }
        }
    }
}

/// Largest number of Newton steps of the interior search on a bilinear patch (ADR 0414 D2).
const BILINEAR_STEPS: usize = 50;
/// Newton stops when the largest component of its step in `(s, t)` is at most this (ADR 0414 D2).
const BILINEAR_STEP_TOL: f64 = 1e-14;

/// Nearest point of the bilinear patch of the quadrilateral `el` to `x` and the bilinear shape functions there (ADR 0414
/// D2): the interior stationary point found by Newton's method from the centre when it is a minimum inside the patch and
/// no farther than the nearest point of the four edges, else that edge point.
fn on_bilinear(nodes: &[Vec3], el: &[usize], x: Vec3) -> (Vec3, Vec<(usize, f64)>) {
    let c = [nodes[el[0]], nodes[el[1]], nodes[el[2]], nodes[el[3]]];
    let pick = |local: [f64; 4]| -> Vec<(usize, f64)> {
        let mut out: BTreeMap<usize, f64> = BTreeMap::new();
        for (k, w) in local.into_iter().enumerate() {
            *out.entry(el[k]).or_insert(0.0) += w;
        }
        out.into_iter().filter(|r| r.1 != 0.0).collect()
    };
    let shape = |s: f64, t: f64| [(1.0 - s) * (1.0 - t), s * (1.0 - t), s * t, (1.0 - s) * t];
    let point = |n: [f64; 4]| {
        (0..4).fold(vec3::ZERO, |a, k| vec3::add(a, vec3::scale(c[k], n[k])))
    };
    let xst = vec3::add(vec3::sub(c[0], c[1]), vec3::sub(c[2], c[3]));
    let (mut s, mut t) = (0.5, 0.5);
    let mut interior = None;
    for _ in 0..BILINEAR_STEPS {
        let xs = vec3::add(vec3::scale(vec3::sub(c[1], c[0]), 1.0 - t), vec3::scale(vec3::sub(c[2], c[3]), t));
        let xt = vec3::add(vec3::scale(vec3::sub(c[3], c[0]), 1.0 - s), vec3::scale(vec3::sub(c[2], c[1]), s));
        let r = vec3::sub(point(shape(s, t)), x);
        let (g0, g1) = (vec3::dot(r, xs), vec3::dot(r, xt));
        let h00 = vec3::dot(xs, xs);
        let h01 = vec3::dot(xs, xt) + vec3::dot(r, xst);
        let h11 = vec3::dot(xt, xt);
        let det = h00 * h11 - h01 * h01;
        if !(h00 > 0.0 && det > 1e-12 * h00 * h11 && det.is_finite()) {
            break;
        }
        let ds = -(h11 * g0 - h01 * g1) / det;
        let dt = -(h00 * g1 - h01 * g0) / det;
        s += ds;
        t += dt;
        if (s - 0.5).abs() > 1.5 || (t - 0.5).abs() > 1.5 {
            break;
        }
        if ds.abs().max(dt.abs()) <= BILINEAR_STEP_TOL {
            if (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t) {
                interior = Some((s, t));
            }
            break;
        }
    }
    // the nearest point of the four edges (straight segments; the bilinear functions are linear along them)
    let mut best: Option<(f64, Vec3, [f64; 4])> = None;
    for (i, j) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        let (y, u) = on_segment(c[i], c[j], x);
        let d = vec3::norm(vec3::sub(x, y));
        if best.as_ref().map_or(true, |b| d < b.0) {
            let mut n = [0.0; 4];
            n[i] = 1.0 - u;
            n[j] += u;
            best = Some((d, y, n));
        }
    }
    let (d_edge, y_edge, n_edge) = best.expect("four edges");
    if let Some((s, t)) = interior {
        let n = shape(s, t);
        let y = point(n);
        if vec3::norm(vec3::sub(x, y)) <= d_edge {
            return (y, pick(n));
        }
    }
    (y_edge, pick(n_edge))
}

/// Nearest point of the segment `a b` to `x` and its parameter `t` (`y = a + t (b - a)`).
fn on_segment(a: Vec3, b: Vec3, x: Vec3) -> (Vec3, f64) {
    let ab = vec3::sub(b, a);
    let ll = vec3::dot(ab, ab);
    let t = if ll > 0.0 {
        (vec3::dot(vec3::sub(x, a), ab) / ll).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (vec3::add(a, vec3::scale(ab, t)), t)
}

/// Nearest point of the triangle `a b c` to `x` and its barycentric coordinates.
fn on_triangle(a: Vec3, b: Vec3, c: Vec3, x: Vec3) -> (Vec3, [f64; 3]) {
    let n = vec3::cross(vec3::sub(b, a), vec3::sub(c, a));
    let nn = vec3::dot(n, n);
    if nn > 0.0 && nn.is_finite() {
        let xp = vec3::sub(x, vec3::scale(n, vec3::dot(vec3::sub(x, a), n) / nn));
        let la = vec3::dot(vec3::cross(vec3::sub(b, xp), vec3::sub(c, xp)), n) / nn;
        let lb = vec3::dot(vec3::cross(vec3::sub(c, xp), vec3::sub(a, xp)), n) / nn;
        let lc = 1.0 - la - lb;
        if la >= 0.0 && lb >= 0.0 && lc >= 0.0 {
            return (xp, [la, lb, lc]);
        }
    }
    // the nearest point lies on the boundary: the nearest of the three edges
    let corners = [a, b, c];
    let mut best: Option<(f64, Vec3, [f64; 3])> = None;
    for (i, j) in [(0, 1), (1, 2), (2, 0)] {
        let (y, t) = on_segment(corners[i], corners[j], x);
        let d = vec3::norm(vec3::sub(x, y));
        if best.as_ref().map_or(true, |b| d < b.0) {
            let mut l = [0.0; 3];
            l[i] = 1.0 - t;
            l[j] += t;
            best = Some((d, y, l));
        }
    }
    let (_, y, l) = best.expect("three edges");
    (y, l)
}

// ---- the offset correction (ADR 0263 D5) ---------------------------------------------------------------------------

/// The nodes of every element that shares a node with an element, computed on demand.
struct Rings {
    node_elements: Vec<Vec<usize>>,
    cache: BTreeMap<usize, Vec<usize>>,
}

impl Rings {
    fn new(n_nodes: usize, elements: &[Vec<usize>]) -> Self {
        let mut node_elements = vec![Vec::new(); n_nodes];
        for (e, el) in elements.iter().enumerate() {
            for &i in el {
                if node_elements[i].last() != Some(&e) {
                    node_elements[i].push(e);
                }
            }
        }
        Rings {
            node_elements,
            cache: BTreeMap::new(),
        }
    }

    fn ring(&mut self, elements: &[Vec<usize>], e: usize) -> &[usize] {
        let node_elements = &self.node_elements;
        self.cache.entry(e).or_insert_with(|| {
            let mut ring: Vec<usize> = elements[e]
                .iter()
                .flat_map(|&i| node_elements[i].iter().flat_map(|&f| elements[f].iter().copied()))
                .collect();
            ring.sort_unstable();
            ring.dedup();
            ring
        })
    }
}

/// Add to `row` the smallest correction on the nodes `ring` that keeps the row sum and makes the row reproduce `x`, in
/// the directions the ring spans.
fn correct(row: &mut Vec<(usize, f64)>, nodes: &[Vec3], ring: &[usize], x: Vec3) {
    let image = row
        .iter()
        .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(nodes[i], w)));
    let g = vec3::sub(x, image);
    if g == vec3::ZERO {
        return;
    }
    let pts: Vec<Vec3> = ring.iter().map(|&i| nodes[i]).collect();
    let (centre, dirs) = directions(&pts);
    if dirs.is_empty() {
        return;
    }
    let beta = dirs.iter().fold(vec3::ZERO, |a, &(mu, e)| {
        vec3::add(a, vec3::scale(e, vec3::dot(e, g) / mu))
    });
    let mut merged: BTreeMap<usize, f64> = row.iter().copied().collect();
    for (&i, &xi) in ring.iter().zip(&pts) {
        *merged.entry(i).or_insert(0.0) += vec3::dot(beta, vec3::sub(xi, centre));
    }
    *row = merged.into_iter().filter(|r| r.1 != 0.0).collect();
}

/// Centroid of `pts` and the eigenpairs `(mu_k, e_k)` of their scatter matrix that pass the rank rule.
fn directions(pts: &[Vec3]) -> (Vec3, Vec<(f64, Vec3)>) {
    let centre = vec3::scale(
        pts.iter().fold(vec3::ZERO, |a, &p| vec3::add(a, p)),
        1.0 / pts.len() as f64,
    );
    let mut c = [[0.0; 3]; 3];
    for p in pts {
        let d = vec3::sub(*p, centre);
        for (r, row) in c.iter_mut().enumerate() {
            for (s, v) in row.iter_mut().enumerate() {
                *v += d[r] * d[s];
            }
        }
    }
    let (mu, e) = symmetric_eigen(c);
    let largest = mu.iter().copied().fold(0.0, f64::max);
    let dirs = (0..3)
        .filter(|&k| largest > 0.0 && mu[k] > 0.0 && (mu[k] / largest).sqrt() >= RANK)
        .map(|k| (mu[k], [e[0][k], e[1][k], e[2][k]]))
        .collect();
    (centre, dirs)
}

/// Eigenvalues and eigenvectors (columns) of a symmetric 3 x 3 matrix by the cyclic Jacobi method.
fn symmetric_eigen(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..64 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        let diag = a[0][0] * a[0][0] + a[1][1] * a[1][1] + a[2][2] * a[2][2];
        if off == 0.0 || off <= 1e-36 * diag {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in a.iter_mut() {
                let (kp, kq) = (row[p], row[q]);
                row[p] = c * kp - s * kq;
                row[q] = s * kp + c * kq;
            }
            for k in 0..3 {
                let (pk, qk) = (a[p][k], a[q][k]);
                a[p][k] = c * pk - s * qk;
                a[q][k] = s * pk + c * qk;
            }
            for row in v.iter_mut() {
                let (kp, kq) = (row[p], row[q]);
                row[p] = c * kp - s * kq;
                row[q] = s * kp + c * kq;
            }
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

// ---- radial-basis interpolation (ADR 0263 D6) ----------------------------------------------------------------------

/// Wendland's `C^2` function of `r` in units of the support.
fn wendland(r: f64) -> f64 {
    if r < 1.0 {
        let s = 1.0 - r;
        let s2 = s * s;
        s2 * s2 * (4.0 * r + 1.0)
    } else {
        0.0
    }
}

struct RadialBasis {
    nodes: Vec<Vec3>,
    support: f64,
    centre: Vec3,
    /// The kept directions divided by the length scale.
    axes: Vec<Vec3>,
    /// Cholesky factor of the kernel matrix (lower, row-major `n x n`).
    phi: Vec<f64>,
    /// `G = Phi^{-1} P` (row-major `n x m`).
    g: Vec<f64>,
    /// `P` (row-major `n x m`).
    p: Vec<f64>,
    /// Cholesky factor of `S = P^T G` (row-major `m x m`).
    s: Vec<f64>,
    condition: f64,
}

impl RadialBasis {
    fn new(nodes: &[Vec3], support: f64) -> Result<Self, String> {
        let n = nodes.len();
        let (centre, dirs) = directions(nodes);
        let scale = nodes
            .iter()
            .map(|x| vec3::norm(vec3::sub(*x, centre)))
            .fold(0.0, f64::max);
        let axes: Vec<Vec3> = dirs.iter().map(|&(_, e)| vec3::scale(e, 1.0 / scale)).collect();
        let m = 1 + axes.len();
        let mut kernel = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                kernel[i * n + j] = wendland(vec3::norm(vec3::sub(nodes[i], nodes[j])) / support);
            }
        }
        let phi = cholesky(&kernel, n)
            .ok_or("interface map: the radial-basis matrix is not positive definite (coincident structural nodes?)")?;
        let largest = power_estimate(&kernel, n, |v| multiply(&kernel, n, v));
        let smallest = power_estimate(&kernel, n, |v| solve(&phi, n, v));
        let mut p = vec![0.0; n * m];
        for (i, x) in nodes.iter().enumerate() {
            p[i * m..(i + 1) * m].copy_from_slice(&poly(centre, &axes, *x));
        }
        let mut g = vec![0.0; n * m];
        for k in 0..m {
            let col: Vec<f64> = (0..n).map(|i| p[i * m + k]).collect();
            for (i, v) in solve(&phi, n, &col).into_iter().enumerate() {
                g[i * m + k] = v;
            }
        }
        let mut schur = vec![0.0; m * m];
        for a in 0..m {
            for b in 0..m {
                schur[a * m + b] = (0..n).map(|i| p[i * m + a] * g[i * m + b]).sum();
            }
        }
        let s = cholesky(&schur, m).ok_or("interface map: the polynomial part of the radial basis is singular")?;
        Ok(RadialBasis {
            nodes: nodes.to_vec(),
            support,
            centre,
            axes,
            phi,
            g,
            p,
            s,
            condition: largest / smallest,
        })
    }

    /// The weights of the interpolant at `x`: `h = a - G S^{-1} (P^T a - p(x))`, `a = Phi^{-1} phi(x)`.
    fn row(&self, x: Vec3) -> Vec<(usize, f64)> {
        let n = self.nodes.len();
        let m = 1 + self.axes.len();
        let phi_x: Vec<f64> = self
            .nodes
            .iter()
            .map(|xi| wendland(vec3::norm(vec3::sub(x, *xi)) / self.support))
            .collect();
        let a = solve(&self.phi, n, &phi_x);
        let px = poly(self.centre, &self.axes, x);
        let r: Vec<f64> = (0..m)
            .map(|k| (0..n).map(|i| self.p[i * m + k] * a[i]).sum::<f64>() - px[k])
            .collect();
        let s = solve(&self.s, m, &r);
        (0..n)
            .map(|i| (i, a[i] - (0..m).map(|k| self.g[i * m + k] * s[k]).sum::<f64>()))
            .filter(|r| r.1 != 0.0)
            .collect()
    }
}

/// `1` and the scaled coordinates along the kept axes.
fn poly(centre: Vec3, axes: &[Vec3], x: Vec3) -> Vec<f64> {
    let d = vec3::sub(x, centre);
    std::iter::once(1.0)
        .chain(axes.iter().map(|e| vec3::dot(*e, d)))
        .collect()
}

/// Lower Cholesky factor of the symmetric `n x n` matrix `a` (row-major), or `None` at a pivot `d <= 2 n eps a_jj`: one
/// that rounding cannot tell from zero (ADR 0263 amendment 1).
fn cholesky(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let d = a[j * n + j] - (0..j).map(|k| l[j * n + k] * l[j * n + k]).sum::<f64>();
        if !(d > 2.0 * n as f64 * f64::EPSILON * a[j * n + j] && d.is_finite()) {
            return None;
        }
        let ljj = d.sqrt();
        l[j * n + j] = ljj;
        for i in j + 1..n {
            let s = a[i * n + j] - (0..j).map(|k| l[i * n + k] * l[j * n + k]).sum::<f64>();
            l[i * n + j] = s / ljj;
        }
    }
    Some(l)
}

/// `x = (L L^T)^{-1} b` for a lower Cholesky factor `l`.
fn solve(l: &[f64], n: usize, b: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; n];
    for i in 0..n {
        let s = b[i] - (0..i).map(|k| l[i * n + k] * y[k]).sum::<f64>();
        y[i] = s / l[i * n + i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s = y[i] - (i + 1..n).map(|k| l[k * n + i] * x[k]).sum::<f64>();
        x[i] = s / l[i * n + i];
    }
    x
}

fn multiply(a: &[f64], n: usize, v: &[f64]) -> Vec<f64> {
    (0..n).map(|i| (0..n).map(|j| a[i * n + j] * v[j]).sum()).collect()
}

/// The Rayleigh quotient `v^T A v` after `POWER_STEPS` steps of `v <- step(v)/|step(v)|` from the vector of ones: the
/// largest eigenvalue of `A` for `step = A`, the smallest for `step = A^{-1}`.
fn power_estimate(a: &[f64], n: usize, step: impl Fn(&[f64]) -> Vec<f64>) -> f64 {
    let unit = |w: Vec<f64>| {
        let len = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        w.into_iter().map(|x| x / len).collect::<Vec<f64>>()
    };
    let mut v = unit(vec![1.0; n]);
    for _ in 0..POWER_STEPS {
        v = unit(step(&v));
    }
    multiply(a, n, &v).iter().zip(&v).map(|(x, y)| x * y).sum()
}
