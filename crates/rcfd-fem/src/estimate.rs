//! Stress recovery by element patches and an energy-norm error estimate. Declared by ADR 0440 (F-408, the scaffold of ADR 0311);
//! filled by ADR 0478 (F-432, the package of ADR 0335).
//!
//! * Sampling (D1): the raw stress `sigma_h = D B u_h` at the points of [`sampling_rule`] of each element's kind (Hex8 the
//!   centroid, Hex20 2 x 2 x 2 Gauss points, Tet4 the centroid, Tet10 the four points of its stiffness rule).
//! * Patches (D2): around every interior corner node (a corner of a solid element on no boundary face) the elements containing
//!   it; a least-squares fit of a complete polynomial ([`PatchBasis`]: linear for the linear kinds, quadratic for the quadratic
//!   ones) in coordinates centred on the node and scaled by the patch, by Householder QR. A patch with fewer points than terms
//!   or a rank-deficient fit is reported ([`Recovery::short_patches`]), not solved.
//! * Nodal values (D3): a solved centre takes its own fit at itself; every other node (boundary, corner, mid-edge, short
//!   centre) the mean of the fits of the nearest solved centres (Euclidean distance, ties within `1e-9` relative, in
//!   ascending centre order) evaluated at the node.
//! * The estimate (D4, Zienkiewicz and Zhu 1987): `eta_e^2 = int_e (sigma* - sigma_h) . D^{-1} (sigma* - sigma_h) dV` with
//!   `sigma* = sum_a N_a sigma*_a`, `eta^2 = sum_e eta_e^2`, integrated with [`estimate_rule`].
//!
//! Work is split over `std::thread::scope` workers in contiguous ranges and joined in order: the results are bit-identical
//! for any thread count (D5). Voigt order xx, yy, zz, xy, yz, zx; stresses tensor shear (ADR 0270 section 2.2).

use crate::assemble::element_dofs;
use crate::element::element_for;
use crate::error::{FemError, Result};
use crate::material::Matrix6;
use crate::mesh::ElementKind;
use crate::model::SolidModel;
use crate::quadrature::{self, QuadratureRule};
use crate::thermal;
use rcfd_core::vec3::Vec3;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Ties of the nearest-centre rule: distances within this relative amount of the smallest (ADR 0478 arithmetic 7).
pub const TIE_TOLERANCE: f64 = 1e-9;

/// Rank test of a patch fit: a diagonal entry of `R` below this fraction of the largest (ADR 0478 arithmetic 7).
pub const RANK_TOLERANCE: f64 = 1e-10;

/// The polynomial fitted over a patch, in the scaled coordinates `(x - x_c)/l`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatchBasis {
    /// `1, x, y, z`.
    Linear,
    /// `1, x, y, z, x^2, y^2, z^2, xy, yz, zx`.
    Quadratic,
}

impl PatchBasis {
    /// Number of terms.
    pub fn terms(self) -> usize {
        match self {
            PatchBasis::Linear => 4,
            PatchBasis::Quadratic => 10,
        }
    }

    fn values(self, p: Vec3) -> [f64; 10] {
        let [x, y, z] = p;
        match self {
            PatchBasis::Linear => [1.0, x, y, z, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            PatchBasis::Quadratic => [1.0, x, y, z, x * x, y * y, z * z, x * y, y * z, z * x],
        }
    }
}

/// Options of the recovery.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PatchOptions {
    /// The basis of every patch (`None`: [`default_basis`] of the patch's elements, the highest of them).
    pub basis: Option<PatchBasis>,
    /// Worker threads (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
}

impl PatchOptions {
    /// Default bases, the available parallelism.
    pub fn new() -> PatchOptions {
        PatchOptions::default()
    }
    /// One basis for every patch.
    pub fn basis(mut self, basis: PatchBasis) -> PatchOptions {
        self.basis = Some(basis);
        self
    }
    /// Worker threads.
    pub fn threads(mut self, threads: Option<usize>) -> PatchOptions {
        self.threads = threads;
        self
    }
}

/// The sampling rule of a solid kind (D1); `None` for a structural kind.
pub fn sampling_rule(kind: ElementKind) -> Option<&'static QuadratureRule> {
    match kind {
        ElementKind::Tet4 => Some(quadrature::tet_1()),
        ElementKind::Tet10 => Some(quadrature::tet_4()),
        ElementKind::Hex8 => Some(quadrature::gauss_1()),
        ElementKind::Hex20 => Some(quadrature::gauss_2()),
        ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => None,
    }
}

/// The rule of the element integrals of the estimate (D4); `None` for a structural kind.
pub fn estimate_rule(kind: ElementKind) -> Option<&'static QuadratureRule> {
    match kind {
        ElementKind::Tet4 | ElementKind::Tet10 => Some(quadrature::tet_14()),
        ElementKind::Hex8 | ElementKind::Hex20 => Some(quadrature::gauss_3()),
        ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => None,
    }
}

/// The default basis of a solid kind (D1); `None` for a structural kind.
pub fn default_basis(kind: ElementKind) -> Option<PatchBasis> {
    match kind {
        ElementKind::Tet4 | ElementKind::Hex8 => Some(PatchBasis::Linear),
        ElementKind::Tet10 | ElementKind::Hex20 => Some(PatchBasis::Quadratic),
        ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => None,
    }
}

/// The solved fit of one patch.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchFit {
    centre: usize,
    origin: Vec3,
    scale: f64,
    basis: PatchBasis,
    coefficients: Vec<[f64; 6]>,
    elements: Vec<usize>,
    points: usize,
}

impl PatchFit {
    /// The centre node.
    pub fn centre(&self) -> usize {
        self.centre
    }
    /// The elements of the patch, ascending.
    pub fn elements(&self) -> &[usize] {
        &self.elements
    }
    /// Number of sampling points fitted.
    pub fn n_points(&self) -> usize {
        self.points
    }
    /// The basis.
    pub fn basis(&self) -> PatchBasis {
        self.basis
    }
    /// The fitted stress at a physical point.
    pub fn evaluate(&self, x: Vec3) -> [f64; 6] {
        let p = [0, 1, 2].map(|i| (x[i] - self.origin[i]) / self.scale);
        let b = self.basis.values(p);
        let mut s = [0.0; 6];
        for (bk, ak) in b.iter().zip(&self.coefficients) {
            for (sc, a) in s.iter_mut().zip(ak) {
                *sc += bk * a;
            }
        }
        s
    }
}

/// A patch that was not solved (D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortPatch {
    /// The centre node.
    pub node: usize,
    /// Number of sampling points.
    pub points: usize,
    /// Number of terms of the basis.
    pub terms: usize,
    /// `"too few points"` or `"rank deficient"`.
    pub reason: &'static str,
}

/// The recovered nodal stress (D3).
#[derive(Debug, Clone, PartialEq)]
pub struct Recovery {
    nodal: Vec<Option<[f64; 6]>>,
    sources: Vec<Vec<usize>>,
    fits: Vec<PatchFit>,
    fit_of: Vec<Option<usize>>,
    short: Vec<ShortPatch>,
}

impl Recovery {
    /// The recovered stress of every node (`None` for a node of no solid element).
    pub fn nodal(&self) -> &[Option<[f64; 6]>] {
        &self.nodal
    }
    /// The centres whose fits gave the node's value, ascending (a solved centre: itself).
    pub fn sources(&self, node: usize) -> &[usize] {
        &self.sources[node]
    }
    /// The fit of a solved centre.
    pub fn patch(&self, centre: usize) -> Option<&PatchFit> {
        self.fit_of.get(centre).copied().flatten().map(|k| &self.fits[k])
    }
    /// Every solved fit, by ascending centre.
    pub fn patches(&self) -> &[PatchFit] {
        &self.fits
    }
    /// The patches that were not solved, by ascending centre.
    pub fn short_patches(&self) -> &[ShortPatch] {
        &self.short
    }
    /// The recovered field `sum_a N_a sigma*_a` at reference point `xi` of an element.
    pub fn stress_at(&self, model: &SolidModel, element: usize, xi: [f64; 3]) -> Result<[f64; 6]> {
        let mesh = model.mesh();
        if element >= mesh.n_elements() {
            return Err(FemError::InvalidValue {
                key: "element",
                reason: format!("element {element} of {}", mesh.n_elements()),
            });
        }
        let n = element_for(mesh.element_kind(element)).shape_functions(xi);
        Ok(self.interpolate(mesh.element_nodes(element), &n))
    }

    fn interpolate(&self, nodes: &[usize], n: &[f64]) -> [f64; 6] {
        let mut s = [0.0; 6];
        for (&node, na) in nodes.iter().zip(n) {
            let v = self.nodal[node].expect("a node of a solid element has a value");
            for (sc, x) in s.iter_mut().zip(&v) {
                *sc += na * x;
            }
        }
        s
    }
}

/// The energy-norm estimate (D4).
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyError {
    element: Vec<f64>,
    estimate: f64,
    solution: f64,
}

impl EnergyError {
    /// The indicator `eta_e` of every element.
    pub fn element_errors(&self) -> &[f64] {
        &self.element
    }
    /// The estimate `eta = (sum eta_e^2)^{1/2}`.
    pub fn estimate(&self) -> f64 {
        self.estimate
    }
    /// The energy norm of the solution, `(int sigma_h . D^{-1} sigma_h dV)^{1/2}`.
    pub fn solution_norm(&self) -> f64 {
        self.solution
    }
    /// The relative estimate `eta/(||u_h||^2 + eta^2)^{1/2}`.
    pub fn relative(&self) -> f64 {
        self.estimate / (self.solution * self.solution + self.estimate * self.estimate).sqrt()
    }
    /// The elements by descending indicator, ties to the lower index.
    pub fn ranked(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.element.len()).collect();
        v.sort_by(|&a, &b| self.element[b].total_cmp(&self.element[a]).then(a.cmp(&b)));
        v
    }
}

// ------------------------------------------------------------------------------------------------------------- helpers

fn worker_count(threads: Option<usize>, len: usize) -> usize {
    let t = threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .max(1);
    t.min(len.max(1))
}

/// `f(0..n)` in order, computed by contiguous ranges on `threads` workers.
fn parallel_map<T, F>(n: usize, threads: Option<usize>, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let workers = worker_count(threads, n);
    if workers <= 1 {
        return (0..n).map(&f).collect();
    }
    let per = n.div_ceil(workers);
    let parts: Vec<Vec<T>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let (lo, hi) = ((w * per).min(n), ((w + 1) * per).min(n));
                let f = &f;
                s.spawn(move || (lo..hi).map(f).collect::<Vec<T>>())
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("estimate worker panicked"))
            .collect()
    });
    let mut out = Vec::with_capacity(n);
    for p in parts {
        out.extend(p);
    }
    out
}

/// Inverse of a 6 x 6 matrix by Gauss-Jordan elimination with partial pivoting; `None` if singular.
fn invert6(a: &Matrix6) -> Option<Matrix6> {
    let mut a = *a;
    let mut inv = [[0.0; 6]; 6];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..6 {
        let mut piv = col;
        for r in col + 1..6 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if a[piv][col] == 0.0 || !a[piv][col].is_finite() {
            return None;
        }
        a.swap(col, piv);
        inv.swap(col, piv);
        let d = a[col][col];
        for j in 0..6 {
            a[col][j] /= d;
            inv[col][j] /= d;
        }
        for r in 0..6 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for j in 0..6 {
                        a[r][j] -= f * a[col][j];
                        inv[r][j] -= f * inv[col][j];
                    }
                }
            }
        }
    }
    Some(inv)
}

fn apply6(d: &Matrix6, v: &[f64; 6]) -> [f64; 6] {
    std::array::from_fn(|r| (0..6).map(|c| d[r][c] * v[c]).sum())
}

fn quadratic_form(c: &Matrix6, v: &[f64; 6]) -> f64 {
    let mut s = 0.0;
    for r in 0..6 {
        for k in 0..6 {
            s += v[r] * c[r][k] * v[k];
        }
    }
    s
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Least squares `min |P a - s|` for `m` rows of `t` columns and six right-hand sides by Householder QR (no pivoting).
/// `Err(())` when a diagonal entry of `R` is below [`RANK_TOLERANCE`] of the largest.
fn least_squares(mut p: Vec<f64>, mut s: Vec<[f64; 6]>, m: usize, t: usize) -> std::result::Result<Vec<[f64; 6]>, ()> {
    let mut diag = vec![0.0; t];
    for k in 0..t {
        let mut norm2 = 0.0;
        for i in k..m {
            norm2 += p[i * t + k] * p[i * t + k];
        }
        let norm = norm2.sqrt();
        if norm == 0.0 {
            diag[k] = 0.0;
            continue;
        }
        let alpha = if p[k * t + k] > 0.0 { -norm } else { norm };
        // v = x - alpha e1, stored in place of column k (rows k..m).
        p[k * t + k] -= alpha;
        let mut vnorm2 = 0.0;
        for i in k..m {
            vnorm2 += p[i * t + k] * p[i * t + k];
        }
        diag[k] = alpha;
        if vnorm2 == 0.0 {
            continue;
        }
        for j in k + 1..t {
            let mut d = 0.0;
            for i in k..m {
                d += p[i * t + k] * p[i * t + j];
            }
            let f = 2.0 * d / vnorm2;
            for i in k..m {
                p[i * t + j] -= f * p[i * t + k];
            }
        }
        for c in 0..6 {
            let mut d = 0.0;
            for i in k..m {
                d += p[i * t + k] * s[i][c];
            }
            let f = 2.0 * d / vnorm2;
            for i in k..m {
                s[i][c] -= f * p[i * t + k];
            }
        }
    }
    let largest = diag.iter().fold(0.0f64, |a, d| a.max(d.abs()));
    if largest == 0.0 || diag.iter().any(|d| d.abs() < RANK_TOLERANCE * largest) {
        return Err(());
    }
    let mut a = vec![[0.0; 6]; t];
    for k in (0..t).rev() {
        for c in 0..6 {
            let mut v = s[k][c];
            for j in k + 1..t {
                v -= p[k * t + j] * a[j][c];
            }
            a[k][c] = v / diag[k];
        }
    }
    Ok(a)
}

/// A uniform grid over the solved centres for the nearest-centre rule (D3).
struct Grid {
    lo: Vec3,
    cell: f64,
    dims: [usize; 3],
    buckets: Vec<Vec<usize>>,
}

impl Grid {
    fn new(points: &[Vec3]) -> Grid {
        let mut lo = points[0];
        let mut hi = points[0];
        for p in points {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        let extent = (0..3).fold(0.0f64, |a, i| a.max(hi[i] - lo[i]));
        let per_axis = (points.len() as f64).cbrt().ceil().max(1.0);
        let cell = if extent > 0.0 { extent / per_axis } else { 1.0 };
        let dims = [0, 1, 2].map(|i| (((hi[i] - lo[i]) / cell).floor() as usize + 1).max(1));
        let mut buckets = vec![Vec::new(); dims[0] * dims[1] * dims[2]];
        for (k, p) in points.iter().enumerate() {
            let c = Grid::cell_of(lo, cell, dims, *p);
            buckets[(c[2] * dims[1] + c[1]) * dims[0] + c[0]].push(k);
        }
        Grid {
            lo,
            cell,
            dims,
            buckets,
        }
    }

    fn cell_of(lo: Vec3, cell: f64, dims: [usize; 3], p: Vec3) -> [usize; 3] {
        [0, 1, 2].map(|i| {
            let f = ((p[i] - lo[i]) / cell).floor();
            if f <= 0.0 {
                0
            } else {
                (f as usize).min(dims[i] - 1)
            }
        })
    }

    /// The indices (into `points`) of the nearest points to `x`, every point within [`TIE_TOLERANCE`] of the smallest
    /// distance, ascending.
    fn nearest(&self, points: &[Vec3], x: Vec3) -> Vec<usize> {
        let c = Grid::cell_of(self.lo, self.cell, self.dims, x);
        let mut seen: Vec<(f64, usize)> = Vec::new();
        let mut best = f64::INFINITY;
        let max_r = self.dims.iter().copied().max().unwrap_or(1);
        for r in 0..=max_r {
            let lo = [0, 1, 2].map(|i| c[i].saturating_sub(r));
            let hi = [0, 1, 2].map(|i| (c[i] + r).min(self.dims[i] - 1));
            for k in lo[2]..=hi[2] {
                for j in lo[1]..=hi[1] {
                    for i in lo[0]..=hi[0] {
                        let ring = (i.abs_diff(c[0])).max(j.abs_diff(c[1])).max(k.abs_diff(c[2]));
                        if ring != r {
                            continue;
                        }
                        for &q in &self.buckets[(k * self.dims[1] + j) * self.dims[0] + i] {
                            let d = distance(points[q], x);
                            best = best.min(d);
                            seen.push((d, q));
                        }
                    }
                }
            }
            // Every point of ring r + 1 or beyond is at least r cells away.
            if best.is_finite() && (r as f64) * self.cell > best * (1.0 + 2.0 * TIE_TOLERANCE) {
                break;
            }
        }
        let mut out: Vec<usize> = seen
            .into_iter()
            .filter(|&(d, _)| d <= best * (1.0 + TIE_TOLERANCE))
            .map(|(_, q)| q)
            .collect();
        out.sort_unstable();
        out
    }
}

// -------------------------------------------------------------------------------------------------------- the recovery

/// What every element contributes: its sampling points and raw stresses.
struct Samples {
    positions: Vec<Vec3>,
    stress: Vec<[f64; 6]>,
}

/// The checked model data: material index and elasticity of every element.
struct Prepared {
    materials: Vec<usize>,
    d: Vec<Option<Matrix6>>,
}

fn prepare(model: &SolidModel, displacement: &[f64]) -> Result<Prepared> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    let mesh = model.mesh();
    for (block, b) in mesh.blocks().iter().enumerate() {
        if sampling_rule(b.kind()).is_none() {
            return Err(FemError::UnsupportedKind {
                block,
                kind: b.kind().name(),
                operation: "patch recovery (ADR 0478: solid elements only)",
            });
        }
    }
    if thermal::temperature_field(model)?.is_some() {
        return Err(FemError::InvalidValue {
            key: "temperature",
            reason: "patch recovery (ADR 0478) does not cover a model with a temperature field".into(),
        });
    }
    let materials = model.material_indices()?;
    let mut d: Vec<Option<Matrix6>> = vec![None; model.materials().len()];
    for (e, &m) in materials.iter().enumerate() {
        if d[m].is_none() {
            let dm = model.materials()[m]
                .1
                .elasticity_matrix()
                .map_err(|source| FemError::Element {
                    element: e,
                    source: Box::new(source),
                })?;
            d[m] = Some(dm);
        }
    }
    Ok(Prepared { materials, d })
}

fn element_samples(model: &SolidModel, prepared: &Prepared, displacement: &[f64], e: usize) -> Result<Samples> {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let kind = mesh.element_kind(e);
    let element = element_for(kind);
    let nodes = mesh.element_nodes(e);
    let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
    let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| displacement[q]).collect();
    let d = prepared.d[prepared.materials[e]].expect("checked");
    let rule = sampling_rule(kind).expect("checked");
    let mut s = Samples {
        positions: Vec::with_capacity(rule.len()),
        stress: Vec::with_capacity(rule.len()),
    };
    for &xi in rule.points() {
        let p = element.point(&coords, xi)?;
        let eps = element.strain_at(&coords, &ue, xi)?;
        s.positions.push(p.x);
        s.stress.push(apply6(&d, &eps));
    }
    Ok(s)
}

enum Outcome {
    Solved(PatchFit),
    Short(ShortPatch),
}

fn fit_patch(
    model: &SolidModel,
    samples: &[Samples],
    centre: usize,
    elements: &[usize],
    options: &PatchOptions,
) -> Outcome {
    let mesh = model.mesh();
    let basis = options.basis.unwrap_or_else(|| {
        elements
            .iter()
            .map(|&e| default_basis(mesh.element_kind(e)).expect("checked"))
            .max()
            .unwrap_or(PatchBasis::Linear)
    });
    let t = basis.terms();
    let origin = mesh.nodes()[centre];
    let m: usize = elements.iter().map(|&e| samples[e].positions.len()).sum();
    if m < t {
        return Outcome::Short(ShortPatch {
            node: centre,
            points: m,
            terms: t,
            reason: "too few points",
        });
    }
    let mut scale = 0.0f64;
    for &e in elements {
        for &x in &samples[e].positions {
            scale = scale.max(distance(x, origin));
        }
    }
    if scale == 0.0 {
        scale = 1.0;
    }
    let mut p = Vec::with_capacity(m * t);
    let mut s = Vec::with_capacity(m);
    for &e in elements {
        for (x, sigma) in samples[e].positions.iter().zip(&samples[e].stress) {
            let local = [0, 1, 2].map(|i| (x[i] - origin[i]) / scale);
            p.extend_from_slice(&basis.values(local)[..t]);
            s.push(*sigma);
        }
    }
    match least_squares(p, s, m, t) {
        Ok(coefficients) => Outcome::Solved(PatchFit {
            centre,
            origin,
            scale,
            basis,
            coefficients,
            elements: elements.to_vec(),
            points: m,
        }),
        Err(()) => Outcome::Short(ShortPatch {
            node: centre,
            points: m,
            terms: t,
            reason: "rank deficient",
        }),
    }
}

fn recover_prepared(
    model: &SolidModel,
    prepared: &Prepared,
    displacement: &[f64],
    options: &PatchOptions,
) -> Result<Recovery> {
    let mesh = model.mesh();
    let ne = mesh.n_elements();
    let n_nodes = mesh.nodes().len();
    let samples: Vec<Samples> = parallel_map(ne, options.threads, |e| {
        element_samples(model, prepared, displacement, e).map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })
    })
    .into_iter()
    .collect::<Result<_>>()?;

    // Boundary nodes: every node of a face of one element.
    let mut boundary = vec![false; n_nodes];
    for faces in mesh.face_incidence().values() {
        if faces.len() == 1 {
            for node in mesh.face_nodes(faces[0]) {
                boundary[node] = true;
            }
        }
    }
    // Corner nodes and their elements (ascending); nodes of solid elements.
    let mut in_solid = vec![false; n_nodes];
    let mut star: Vec<Vec<usize>> = vec![Vec::new(); n_nodes];
    for e in 0..ne {
        let kind = mesh.element_kind(e);
        let nodes = mesh.element_nodes(e);
        for &n in nodes {
            in_solid[n] = true;
        }
        for &n in &nodes[..kind.corners()] {
            star[n].push(e);
        }
    }
    let centres: Vec<usize> = (0..n_nodes).filter(|&n| !star[n].is_empty() && !boundary[n]).collect();
    // A patch across a material interface is refused (D2).
    for &c in &centres {
        let first = prepared.materials[star[c][0]];
        for &e in &star[c][1..] {
            let m = prepared.materials[e];
            if m != first && model.materials()[m].1 != model.materials()[first].1 {
                return Err(FemError::InvalidValue {
                    key: "patch recovery",
                    reason: format!(
                        "the patch of node {c} joins elements of the material sets '{}' and '{}' with different \
                         materials (ADR 0478 D2)",
                        model.materials()[first].0,
                        model.materials()[m].0
                    ),
                });
            }
        }
    }
    let outcomes = parallel_map(centres.len(), options.threads, |k| {
        let c = centres[k];
        fit_patch(model, &samples, c, &star[c], options)
    });
    let mut fits = Vec::new();
    let mut short = Vec::new();
    let mut fit_of = vec![None; n_nodes];
    for o in outcomes {
        match o {
            Outcome::Solved(f) => {
                fit_of[f.centre] = Some(fits.len());
                fits.push(f);
            }
            Outcome::Short(s) => short.push(s),
        }
    }
    if fits.is_empty() {
        let mut list: Vec<String> = short
            .iter()
            .take(20)
            .map(|s| {
                format!(
                    "node {}: {} points for {} terms, {}",
                    s.node, s.points, s.terms, s.reason
                )
            })
            .collect();
        if short.len() > 20 {
            list.push(format!("and {} more", short.len() - 20));
        }
        return Err(FemError::InvalidValue {
            key: "patch recovery",
            reason: format!(
                "no patch can be solved (ADR 0478 D3): {} interior corner nodes, {} short patches{}{}",
                centres.len(),
                short.len(),
                if list.is_empty() { "" } else { ": " },
                list.join("; ")
            ),
        });
    }
    let positions: Vec<Vec3> = fits.iter().map(|f| f.origin).collect();
    let grid = Grid::new(&positions);
    let mut nodal = vec![None; n_nodes];
    let mut sources = vec![Vec::new(); n_nodes];
    for n in 0..n_nodes {
        if !in_solid[n] {
            continue;
        }
        let x = mesh.nodes()[n];
        let chosen: Vec<usize> = match fit_of[n] {
            Some(k) => vec![k],
            None => grid.nearest(&positions, x),
        };
        let mut s = [0.0; 6];
        for &k in &chosen {
            let v = fits[k].evaluate(x);
            for (sc, vc) in s.iter_mut().zip(&v) {
                *sc += vc;
            }
        }
        let count = chosen.len() as f64;
        nodal[n] = Some(s.map(|v| v / count));
        sources[n] = chosen.iter().map(|&k| fits[k].centre).collect();
    }
    Ok(Recovery {
        nodal,
        sources,
        fits,
        fit_of,
        short,
    })
}

/// The recovered nodal stress of a displacement (one value per unknown of `model.dof_map()`). Refuses a displacement of
/// the wrong length, a structural element kind, a temperature field, a material without a constant elasticity matrix, a
/// patch across a material interface and a model in which no patch can be solved.
pub fn recover(model: &SolidModel, displacement: &[f64], options: &PatchOptions) -> Result<Recovery> {
    let prepared = prepare(model, displacement)?;
    recover_prepared(model, &prepared, displacement, options)
}

/// The recovery and the energy-norm estimate of a displacement (D4); refusals as [`recover`].
pub fn energy_error(
    model: &SolidModel,
    displacement: &[f64],
    options: &PatchOptions,
) -> Result<(Recovery, EnergyError)> {
    let prepared = prepare(model, displacement)?;
    let recovery = recover_prepared(model, &prepared, displacement, options)?;
    let compliance: Vec<Option<Matrix6>> = prepared.d.iter().map(|d| d.as_ref().and_then(invert6)).collect();
    let mesh = model.mesh();
    let dofs = model.dof_map();
    let per_element = parallel_map(mesh.n_elements(), options.threads, |e| -> Result<(f64, f64)> {
        let located = |source: FemError| FemError::Element {
            element: e,
            source: Box::new(source),
        };
        let kind = mesh.element_kind(e);
        let element = element_for(kind);
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| displacement[q]).collect();
        let m = prepared.materials[e];
        let d = prepared.d[m].expect("checked");
        let c = compliance[m].ok_or_else(|| {
            located(FemError::InvalidValue {
                key: "elasticity",
                reason: "the elasticity matrix is singular".into(),
            })
        })?;
        let rule = estimate_rule(kind).expect("checked");
        let (mut err, mut sol) = (0.0, 0.0);
        for ip in element.integration_points(&coords, rule).map_err(located)? {
            let eps = element.strain_at(&coords, &ue, ip.geometry.xi).map_err(located)?;
            let sh = apply6(&d, &eps);
            let star = recovery.interpolate(nodes, &ip.geometry.n);
            let diff: [f64; 6] = std::array::from_fn(|k| star[k] - sh[k]);
            err += ip.weight * quadratic_form(&c, &diff);
            sol += ip.weight * quadratic_form(&c, &sh);
        }
        Ok((err, sol))
    });
    let mut element = Vec::with_capacity(per_element.len());
    let (mut total, mut solution) = (0.0, 0.0);
    for r in per_element {
        let (err, sol) = r?;
        total += err;
        solution += sol;
        element.push(err.sqrt());
    }
    Ok((
        recovery,
        EnergyError {
            element,
            estimate: total.sqrt(),
            solution: solution.sqrt(),
        },
    ))
}
