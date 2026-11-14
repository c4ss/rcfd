//! Cyclic symmetry: sector constraints ([`crate::model::Constraint::Cyclic`]), modes by harmonic index, expansion to the
//! full rotor. Declared by ADR 0291; filled by ADR 0386 (the registration of F-394; plan ADR 0297).
//!
//! A rotor of `N` identical sectors is solved on one sector. The high boundary of the sector is its low boundary rotated by
//! `alpha = 2 pi / N` about the axis (right-hand sense): [`boundary`] matches the two node for node within
//! [`MATCH_TOLERANCE`] and refuses the first unmatched node. With the cosine part `a` and the sine part `b` of the complex
//! sector amplitude of harmonic index `k` (`theta = k alpha`) the condition between the sectors is the real pair of rows
//! `a_H = R (cos theta a_L - sin theta b_L)`, `b_H = R (sin theta a_L + cos theta b_L)` (ADR 0386, derived there). They are
//! rows of the transformation `x = T q` of ADR 0296: for `k = 0` the rows `u_H = R u_L` are part of
//! [`crate::mpc::transformation`] (statics with a load that repeats in every sector, and the `k = 0` modes); for `0 < 2 k <
//! N` the problem over `(a, b)` has twice the size and every eigenvalue twice (a pair of full modes); for `2 k = N` the rows
//! `a_H = -R a_L` act on `a` alone ([`crate::mpc::harmonic_transformation`], [`modal_analysis_harmonic`]). [`full_rotor`]
//! copies the sector mesh to the full rotor and [`FullRotor::expand`] expands a sector mode to it, M-orthonormal.
//!
//! ADR 0395 (F-489): a static load of harmonic index `k` (a cosine and a sine part per sector) is solved on the sector by the
//! same rows ([`solve_static_harmonic`]; [`FullRotor::expand_load`] and [`FullRotor::expand_field`] give the full load and
//! the full solution), and a rigid set acts at `k > 0` with its six reference unknowns doubled (one block per part).

use crate::assemble::stiffness_with;
use crate::constraint::Prescribed;
use crate::error::{FemError, Result, SetKind};
use crate::loads::NodalForceAt;
use crate::mass::consistent_with;
use crate::mesh::{FaceRef, SolidMesh};
use crate::modal::{frequency, ModalOptions};
use crate::model::{Constraint, DofMap, SolidModel};
use crate::mpc::{self, ReferenceLoad, ReferenceMotion, Transformation, REFERENCE_UNKNOWNS};
use crate::solve::{StaticOptions, StaticSolver, PIVOT_TOLERANCE};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SturmCheck, SubspaceOptions};
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// The work package that adds cyclic symmetry; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Relative tolerance of the boundary match (ADR 0386 D1): the image of a low node must lie within `MATCH_TOLERANCE L` of a
/// high node, `L` the largest distance of a node of the two boundaries from the axis.
pub const MATCH_TOLERANCE: f64 = 1e-8;

/// A 3 x 3 matrix, rows first.
pub type Mat3 = [[f64; 3]; 3];

/// The rotation by `angle` about the unit vector `n`: `I + sin(angle) [n]x + (1 - cos(angle)) [n]x^2` (Rodrigues).
pub fn rotation_matrix(n: Vec3, angle: f64) -> Mat3 {
    let k = [[0.0, -n[2], n[1]], [n[2], 0.0, -n[0]], [-n[1], n[0], 0.0]];
    let (s, c1) = (angle.sin(), 1.0 - angle.cos());
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let k2: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = if i == j { 1.0 } else { 0.0 } + s * k[i][j] + c1 * k2;
        }
    }
    r
}

fn apply(r: &Mat3, v: Vec3) -> Vec3 {
    [
        r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
        r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
        r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
    ]
}

fn rotate_point(r: &Mat3, origin: Vec3, x: Vec3) -> Vec3 {
    let y = apply(r, [x[0] - origin[0], x[1] - origin[1], x[2] - origin[2]]);
    [origin[0] + y[0], origin[1] + y[1], origin[2] + y[2]]
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn cyclic_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "cyclic", reason }
}

/// The matched boundaries of a cyclic constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct CyclicBoundary {
    constraint: usize,
    sectors: usize,
    axis: Vec3,
    origin: Vec3,
    rotation: Mat3,
    pairs: Vec<(usize, usize)>,
    length: f64,
    tolerance: f64,
}

impl CyclicBoundary {
    /// Index of the constraint in [`SolidModel::constraints`].
    pub fn constraint(&self) -> usize {
        self.constraint
    }
    /// Number of sectors `N`.
    pub fn sectors(&self) -> usize {
        self.sectors
    }
    /// The unit axis.
    pub fn axis(&self) -> Vec3 {
        self.axis
    }
    /// The point on the axis.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }
    /// The rotation `R` by `2 pi / N` about the axis.
    pub fn rotation(&self) -> &Mat3 {
        &self.rotation
    }
    /// The matched nodes `(low, high)`, ascending in the low node.
    pub fn pairs(&self) -> &[(usize, usize)] {
        &self.pairs
    }
    /// The length `L`: the largest distance of a boundary node from the axis.
    pub fn length(&self) -> f64 {
        self.length
    }
    /// The absolute tolerance of the match, `MATCH_TOLERANCE L`.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }
}

/// The index of the one cyclic constraint of a model; none or more than one is refused.
pub fn the_cyclic_constraint(model: &SolidModel) -> Result<usize> {
    let list: Vec<usize> = model
        .constraints()
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, Constraint::Cyclic { .. }))
        .map(|(i, _)| i)
        .collect();
    match list.as_slice() {
        &[i] => Ok(i),
        _ => Err(cyclic_error(format!(
            "a cyclic analysis needs exactly one Cyclic constraint; the model has {}",
            list.len()
        ))),
    }
}

/// Matches the boundaries of cyclic constraint `constraint` (ADR 0386 D1): each low node's image under the rotation by `2 pi
/// / N` must lie within `MATCH_TOLERANCE L` of exactly one high node, and every high node must be such an image. Refuses a
/// node on both boundaries, the first low node without a partner (naming it, its image and the nearest high node), a high
/// node taken twice and the first high node left without a partner.
pub fn boundary(model: &SolidModel, constraint: usize) -> Result<CyclicBoundary> {
    let Some(Constraint::Cyclic {
        low,
        high,
        axis,
        origin,
        sectors,
    }) = model.constraints().get(constraint)
    else {
        return Err(cyclic_error(format!(
            "constraint {constraint} is not a Cyclic constraint"
        )));
    };
    if *sectors < 2 {
        return Err(FemError::InvalidValue {
            key: "sectors",
            reason: format!("{sectors} sectors: a cyclic rotor has at least 2"),
        });
    }
    let mesh = model.mesh();
    let lows = mesh.surface_set_nodes(low)?;
    let highs = mesh.surface_set_nodes(high)?;
    if let Some(&p) = lows.iter().find(|p| highs.binary_search(p).is_ok()) {
        return Err(cyclic_error(format!(
            "node {p} is on both boundaries '{low}' and '{high}' (a node on the axis or a set given twice)"
        )));
    }
    let an = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let n = [axis[0] / an, axis[1] / an, axis[2] / an];
    let x = mesh.nodes();
    let mut length: f64 = 0.0;
    for &p in lows.iter().chain(&highs) {
        let d = [x[p][0] - origin[0], x[p][1] - origin[1], x[p][2] - origin[2]];
        let along = d[0] * n[0] + d[1] * n[1] + d[2] * n[2];
        let perp = [d[0] - along * n[0], d[1] - along * n[1], d[2] - along * n[2]];
        length = length.max((perp[0] * perp[0] + perp[1] * perp[1] + perp[2] * perp[2]).sqrt());
    }
    if length == 0.0 {
        return Err(cyclic_error(format!(
            "every node of '{low}' and '{high}' is on the axis"
        )));
    }
    let tolerance = MATCH_TOLERANCE * length;
    let window = tolerance;
    let accept = |d: f64| d <= tolerance;
    let rotation = rotation_matrix(n, 2.0 * PI / *sectors as f64);
    // The high nodes sorted by their first coordinate, for a window search.
    let mut sorted: Vec<(f64, usize)> = highs.iter().map(|&h| (x[h][0], h)).collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut partner_of_high: Vec<Option<usize>> = vec![None; x.len()];
    let mut pairs = Vec::with_capacity(lows.len());
    for &l in &lows {
        let y = rotate_point(&rotation, *origin, x[l]);
        let start = sorted.partition_point(|e| e.0 < y[0] - window);
        let mut best: Option<(usize, f64)> = None;
        for &(xh, h) in &sorted[start..] {
            if xh > y[0] + window {
                break;
            }
            let d = distance(y, x[h]);
            if best.map_or(true, |(_, b)| d < b) {
                best = Some((h, d));
            }
        }
        let found = match best {
            Some((h, d)) if accept(d) => h,
            _ => {
                let (h, d) = highs
                    .iter()
                    .map(|&h| (h, distance(y, x[h])))
                    .fold((usize::MAX, f64::INFINITY), |b, c| if c.1 < b.1 { c } else { b });
                return Err(cyclic_error(format!(
                    "low node {l} at {:?} of '{low}': its image {y:?} under the rotation by 2 pi/{sectors} has no node of \
                     the high boundary '{high}' within {tolerance:e} (nearest: node {h} at {d:e})",
                    x[l]
                )));
            }
        };
        if let Some(l0) = partner_of_high[found] {
            return Err(cyclic_error(format!(
                "low nodes {l0} and {l} of '{low}' both map to high node {found} of '{high}'"
            )));
        }
        partner_of_high[found] = Some(l);
        pairs.push((l, found));
    }
    if let Some(&h) = highs.iter().find(|&&h| partner_of_high[h].is_none()) {
        return Err(cyclic_error(format!(
            "high node {h} at {:?} of '{high}' is the image of no node of the low boundary '{low}' (tolerance {tolerance:e})",
            x[h]
        )));
    }
    Ok(CyclicBoundary {
        constraint,
        sectors: *sectors,
        axis: n,
        origin: *origin,
        rotation,
        pairs,
        length,
        tolerance,
    })
}

/// One mode of the real sector problem: the cosine part `a` and the sine part `b` (zero for a single harmonic), each over
/// the model's [`DofMap`].
#[derive(Debug, Clone, PartialEq)]
pub struct SectorMode {
    /// The cosine part `a`.
    pub cos: Vec<f64>,
    /// The sine part `b`.
    pub sin: Vec<f64>,
}

/// The modes of one harmonic index of a sector (ADR 0386 D1).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicModes {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// The number of sectors `N`.
    pub sectors: usize,
    /// 1 for `k = 0` and `2 k = N` (single modes), 2 otherwise (each sector frequency is a pair of full modes).
    pub multiplicity: usize,
    /// Every eigenvalue of the real problem, ascending (`values.len() * multiplicity` of them).
    pub eigenvalues: Vec<f64>,
    /// One eigenvalue per sector frequency (a pair merged by its mean), rad^2/s^2.
    pub values: Vec<f64>,
    /// The frequencies of `values`, Hz.
    pub frequencies: Vec<f64>,
    /// The relative split `|lambda_2i+1 - lambda_2i| / |mean|` of each pair (0 for a single harmonic).
    pub pair_splits: Vec<f64>,
    /// One mode per eigenvalue of the real problem, M-orthonormal over `(a, b)`.
    pub modes: Vec<SectorMode>,
    /// The residuals of the eigenpairs of the real problem.
    pub residuals: Vec<f64>,
    /// Iterations of the subspace iteration.
    pub iterations: usize,
    /// The shift used.
    pub shift: f64,
    /// The Sturm check of the eigensolver.
    pub sturm: SturmCheck,
    /// Number of independent unknowns of the real problem.
    pub unknowns: usize,
    /// Whether the factor at zero shift was not accepted (a free motion of the harmonic).
    pub rigid_free: bool,
    /// The transformation.
    pub transformation: Transformation,
    /// `T^T K T` (`K` doubled block-diagonally for a pair).
    pub reduced_stiffness: CsrMatrix,
    /// `T^T M T`.
    pub reduced_mass: CsrMatrix,
}

/// `blockdiag(a, a)`.
pub(crate) fn doubled_matrix(a: &CsrMatrix) -> CsrMatrix {
    let n = a.n;
    let mut row_ptr = a.row_ptr.clone();
    let mut col = a.col.clone();
    let mut val = a.val.clone();
    let nnz = a.col.len();
    for r in 0..n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            col.push(a.col[p] + n);
            val.push(a.val[p]);
        }
        row_ptr.push(nnz + a.row_ptr[r + 1]);
    }
    CsrMatrix::rect(2 * n, 2 * n, row_ptr, col, val)
}

fn number_of_sectors(model: &SolidModel, ci: usize) -> usize {
    match &model.constraints()[ci] {
        Constraint::Cyclic { sectors, .. } => *sectors,
        _ => unreachable!("the cyclic constraint"),
    }
}

/// The modal analysis of harmonic index `harmonic` of a sector with one cyclic constraint (ADR 0386 D1): `options.modes`
/// sector frequencies (a pair counted once), with the per-node prescribed values `extra` (all zero). `k = 0` runs
/// [`mpc::modal_analysis_mpc`] (its rows are in the transformation); `0 < 2 k < N` the doubled real problem over the cosine and
/// sine parts, `2 k = N` the single one ([`mpc::harmonic_transformation`]; a rigid set with its reference unknowns doubled,
/// ADR 0395). Refuses a harmonic index above `N / 2`, a model without exactly one cyclic constraint and a non-zero
/// constraint value.
pub fn modal_analysis_harmonic(
    model: &SolidModel,
    extra: &[Prescribed],
    harmonic: usize,
    options: &ModalOptions,
) -> Result<HarmonicModes> {
    model.validate()?;
    model.check_supported()?;
    let ci = the_cyclic_constraint(model)?;
    let sectors = number_of_sectors(model, ci);
    if 2 * harmonic > sectors {
        return Err(FemError::InvalidValue {
            key: "harmonic",
            reason: format!(
                "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                sectors / 2
            ),
        });
    }
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    if harmonic == 0 {
        let s = mpc::modal_analysis_mpc(model, extra, options)?;
        let r = s.result;
        let modes = r
            .modes
            .iter()
            .map(|m| SectorMode {
                cos: m.clone(),
                sin: vec![0.0; n],
            })
            .collect();
        return Ok(HarmonicModes {
            harmonic,
            sectors,
            multiplicity: 1,
            values: r.eigenvalues.clone(),
            frequencies: r.frequencies.clone(),
            pair_splits: vec![0.0; r.eigenvalues.len()],
            eigenvalues: r.eigenvalues,
            modes,
            residuals: r.residuals,
            iterations: r.iterations,
            shift: r.shift,
            sturm: r.sturm,
            unknowns: r.unknowns,
            rigid_free: r.rigid_free,
            transformation: s.transformation,
            reduced_stiffness: s.reduced_stiffness,
            reduced_mass: s.reduced_mass,
        });
    }
    let tr = mpc::harmonic_transformation(model, extra, harmonic)?;
    let multiplicity = if 2 * harmonic < sectors { 2 } else { 1 };
    let mut k = stiffness_with(model, options.threads)?;
    mpc::add_supports(&mut k, model)?;
    let m = consistent_with(model, options.threads)?;
    let (k2, m2) = if multiplicity == 2 {
        (doubled_matrix(&k), doubled_matrix(&m))
    } else {
        (k, m)
    };
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k2, &zero)?;
    let (mr, _) = tr.reduce(&m2, &zero)?;
    let nq = kr.n;
    let count = options.modes * multiplicity;
    if options.modes == 0 || count >= nq {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes ({count} eigenvalues of the real problem) requested of harmonic {harmonic} with {nq} free \
                 unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (a, d) in mr.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            let i = tr.independent()[a];
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!("{} has no mass (diagonal {d:e})", unknown_label(&tr, model, i)),
            });
        }
    }
    let opts = SubspaceOptions {
        shift: options.shift.unwrap_or(Shift::Auto),
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr, &mr, count, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let rigid_free = match options.shift {
        None => eig.shift != 0.0,
        Some(_) => mpc::zero_shift_refused(&kr),
    };
    let modes: Vec<SectorMode> = eig
        .vectors
        .iter()
        .map(|q| {
            let x = tr.expand(q);
            SectorMode {
                cos: x[..n].to_vec(),
                sin: if multiplicity == 2 {
                    x[n..2 * n].to_vec()
                } else {
                    vec![0.0; n]
                },
            }
        })
        .collect();
    let (values, pair_splits): (Vec<f64>, Vec<f64>) = if multiplicity == 2 {
        eig.values
            .chunks(2)
            .map(|p| {
                let mean = 0.5 * (p[0] + p[1]);
                let split = if mean == 0.0 {
                    (p[1] - p[0]).abs()
                } else {
                    (p[1] - p[0]).abs() / mean.abs()
                };
                (mean, split)
            })
            .unzip()
    } else {
        (eig.values.clone(), vec![0.0; eig.values.len()])
    };
    Ok(HarmonicModes {
        harmonic,
        sectors,
        multiplicity,
        frequencies: values.iter().map(|&l| frequency(l)).collect(),
        values,
        pair_splits,
        eigenvalues: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        shift: eig.shift,
        sturm: eig.sturm,
        unknowns: nq,
        rigid_free,
        transformation: tr,
        reduced_stiffness: kr,
        reduced_mass: mr,
    })
}

/// The full rotor built from a sector mesh (ADR 0386 D1): `N` rotated copies, each high node of sector `j` the low node of
/// sector `j + 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct FullRotor {
    mesh: SolidMesh,
    node_of: Vec<Vec<usize>>,
    is_high: Vec<bool>,
    sectors: usize,
    axis: Vec3,
}

impl FullRotor {
    /// The mesh of the full rotor.
    pub fn mesh(&self) -> &SolidMesh {
        &self.mesh
    }
    /// The number of sectors.
    pub fn sectors(&self) -> usize {
        self.sectors
    }
    /// The node of the full mesh that is node `node` of sector `sector`.
    pub fn node(&self, sector: usize, node: usize) -> usize {
        self.node_of[sector][node]
    }

    /// The full mode of a sector mode of harmonic index `harmonic` (ADR 0386): at node `p` of sector `j`, `s R^j (cos(j
    /// theta) a_p - sin(j theta) b_p)`, `theta = 2 pi k / N`, `s = sqrt(2 / N)` for a pair and `sqrt(1 / N)` for a single
    /// harmonic, so that an M-orthonormal sector mode gives an M-orthonormal full mode. `sector_dofs` and `full_dofs` are the
    /// numberings of the sector model and of the full model (three components per node).
    pub fn expand(&self, sector_dofs: &DofMap, full_dofs: &DofMap, harmonic: usize, mode: &SectorMode) -> Vec<f64> {
        let nsec = self.sectors;
        let single = harmonic % nsec == 0 || 2 * (harmonic % nsec) == nsec;
        let scale = if single {
            (1.0 / nsec as f64).sqrt()
        } else {
            (2.0 / nsec as f64).sqrt()
        };
        let mut out = vec![0.0; full_dofs.n_equations()];
        for j in 0..nsec {
            let r = rotation_matrix(self.axis, 2.0 * PI * j as f64 / nsec as f64);
            let jk = (j * harmonic) % nsec;
            let (c, s) = if jk == 0 {
                (1.0, 0.0)
            } else if 2 * jk == nsec {
                (-1.0, 0.0)
            } else {
                let t = 2.0 * PI * jk as f64 / nsec as f64;
                (t.cos(), t.sin())
            };
            for p in 0..self.is_high.len() {
                if self.is_high[p] {
                    continue;
                }
                let mut v = [0.0; 3];
                for (i, vi) in v.iter_mut().enumerate() {
                    let e = sector_dofs.equation(p, i).expect("sector component");
                    *vi = c * mode.cos[e] - s * mode.sin[e];
                }
                let w = apply(&r, v);
                let full = self.node_of[j][p];
                for (i, wi) in w.iter().enumerate() {
                    out[full_dofs.equation(full, i).expect("full component")] = scale * wi;
                }
            }
        }
        out
    }

    /// `(c, s) = (cos(j theta), sin(j theta))` of sector `j`, exact at `j k = 0` and `2 j k = N` (mod N).
    fn phase(&self, harmonic: usize, j: usize) -> (f64, f64) {
        let nsec = self.sectors;
        let jk = (j * harmonic) % nsec;
        if jk == 0 {
            (1.0, 0.0)
        } else if 2 * jk == nsec {
            (-1.0, 0.0)
        } else {
            let t = 2.0 * PI * jk as f64 / nsec as f64;
            (t.cos(), t.sin())
        }
    }

    /// The full field of a harmonic static solution (ADR 0395): at node `p` of sector `j`, `R^j (cos(j theta) a_p - sin(j
    /// theta) b_p)`, scale 1, each full node written once from the sector in which it is not a high node.
    pub fn expand_field(
        &self,
        sector_dofs: &DofMap,
        full_dofs: &DofMap,
        harmonic: usize,
        cos: &[f64],
        sin: &[f64],
    ) -> Vec<f64> {
        let mut out = vec![0.0; full_dofs.n_equations()];
        for j in 0..self.sectors {
            let r = rotation_matrix(self.axis, 2.0 * PI * j as f64 / self.sectors as f64);
            let (c, s) = self.phase(harmonic, j);
            for p in 0..self.is_high.len() {
                if self.is_high[p] {
                    continue;
                }
                let mut v = [0.0; 3];
                for (i, vi) in v.iter_mut().enumerate() {
                    let e = sector_dofs.equation(p, i).expect("sector component");
                    *vi = c * cos[e] - s * sin[e];
                }
                let w = apply(&r, v);
                let full = self.node_of[j][p];
                for (i, wi) in w.iter().enumerate() {
                    out[full_dofs.equation(full, i).expect("full component")] = *wi;
                }
            }
        }
        out
    }

    /// The expanded load of a harmonic load (ADR 0395): the assembly over the sectors of `R^j (cos(j theta) f_a - sin(j
    /// theta) f_b)` at every node of sector `j` (high nodes included: a shared node sums the contributions of its two
    /// sectors).
    pub fn expand_load(
        &self,
        sector_dofs: &DofMap,
        full_dofs: &DofMap,
        harmonic: usize,
        cos: &[f64],
        sin: &[f64],
    ) -> Vec<f64> {
        let mut out = vec![0.0; full_dofs.n_equations()];
        for j in 0..self.sectors {
            let r = rotation_matrix(self.axis, 2.0 * PI * j as f64 / self.sectors as f64);
            let (c, s) = self.phase(harmonic, j);
            for p in 0..self.is_high.len() {
                let mut v = [0.0; 3];
                for (i, vi) in v.iter_mut().enumerate() {
                    let e = sector_dofs.equation(p, i).expect("sector component");
                    *vi = c * cos[e] - s * sin[e];
                }
                let w = apply(&r, v);
                let full = self.node_of[j][p];
                for (i, wi) in w.iter().enumerate() {
                    out[full_dofs.equation(full, i).expect("full component")] += *wi;
                }
            }
        }
        out
    }
}

/// The full rotor of a sector model with one cyclic constraint (ADR 0386 D1): the sector mesh copied `N` times (copy `j`
/// rotated by `j 2 pi / N`), the low nodes of copy `j >= 1` identified with the matched high nodes of copy `j - 1` and the
/// high nodes of copy `N - 1` with the low nodes of copy 0. Blocks keep their index, kind and property (copy `j` of element
/// `e` of block `b` is element `start_b + j n_b + (e - start_b^sector)`); node, element and surface sets are the unions of
/// their copies. Refuses a block of a structural kind.
pub fn full_rotor(model: &SolidModel) -> Result<FullRotor> {
    let ci = the_cyclic_constraint(model)?;
    let bd = boundary(model, ci)?;
    let mesh = model.mesh();
    for (b, block) in mesh.blocks().iter().enumerate() {
        if !block.kind().is_solid() {
            return Err(FemError::UnsupportedKind {
                block: b,
                kind: block.kind().name(),
                operation: "full_rotor",
            });
        }
    }
    let nsec = bd.sectors();
    let x = mesh.nodes();
    let n = x.len();
    let mut high_of_low: Vec<Option<usize>> = vec![None; n];
    let mut low_of_high: Vec<Option<usize>> = vec![None; n];
    let mut is_high = vec![false; n];
    for &(l, h) in bd.pairs() {
        high_of_low[l] = Some(h);
        low_of_high[h] = Some(l);
        is_high[h] = true;
    }
    let mut nodes: Vec<Vec3> = Vec::with_capacity(n * nsec);
    let mut node_of = vec![vec![usize::MAX; n]; nsec];
    for j in 0..nsec {
        let r = rotation_matrix(bd.axis(), 2.0 * PI * j as f64 / nsec as f64);
        for p in 0..n {
            node_of[j][p] = match (high_of_low[p], low_of_high[p]) {
                (Some(h), _) if j >= 1 => node_of[j - 1][h],
                (_, Some(l)) if j == nsec - 1 => node_of[0][l],
                _ => {
                    nodes.push(rotate_point(&r, bd.origin(), x[p]));
                    nodes.len() - 1
                }
            };
        }
    }
    let mut full = SolidMesh::new(nodes)?;
    // Element numbering: block by block, sector by sector.
    let mut sector_start = Vec::new();
    let mut full_start = Vec::new();
    let (mut s0, mut f0) = (0usize, 0usize);
    for (b, block) in mesh.blocks().iter().enumerate() {
        let npe = block.kind().nodes_per_element();
        let mut conn = Vec::with_capacity(block.connectivity().len() * nsec);
        for map in &node_of {
            conn.extend(block.connectivity().iter().map(|&p| map[p]));
        }
        let fb = full.add_block(block.kind(), conn)?;
        debug_assert_eq!(fb, b);
        if let Some(prop) = block.property() {
            full.set_block_property(fb, prop.clone())?;
        }
        sector_start.push(s0);
        full_start.push(f0);
        let ne = block.connectivity().len() / npe;
        s0 += ne;
        f0 += ne * nsec;
    }
    let element_of = |j: usize, e: usize| -> usize {
        let b = mesh.block_of(e);
        let ne = mesh.blocks()[b].len();
        full_start[b] + j * ne + (e - sector_start[b])
    };
    for name in mesh.node_set_names() {
        let set = mesh.node_set(name).expect("listed");
        let mut v: Vec<usize> = node_of.iter().flat_map(|map| set.iter().map(|&p| map[p])).collect();
        v.sort_unstable();
        v.dedup();
        full.add_node_set(name, v)?;
    }
    for name in mesh.element_set_names() {
        let set = mesh.element_set(name).expect("listed");
        let mut v: Vec<usize> = (0..nsec)
            .flat_map(|j| set.iter().map(move |&e| (j, e)))
            .map(|(j, e)| element_of(j, e))
            .collect();
        v.sort_unstable();
        full.add_element_set(name, v)?;
    }
    for name in mesh.surface_set_names() {
        let set = mesh.surface_set(name).ok_or_else(|| FemError::UnknownSet {
            kind: SetKind::Surface,
            name: name.to_string(),
        })?;
        let mut v: Vec<FaceRef> = Vec::with_capacity(set.len() * nsec);
        for j in 0..nsec {
            for f in set {
                v.push(FaceRef {
                    element: element_of(j, f.element),
                    face: f.face,
                });
            }
        }
        full.add_surface_set(name, v)?;
    }
    Ok(FullRotor {
        mesh: full,
        node_of,
        is_high,
        sectors: nsec,
        axis: bd.axis(),
    })
}

/// A static load of harmonic index `harmonic` on a sector (ADR 0395): the cosine part `cos` and the sine part `sin`, each a
/// load vector over the model's [`DofMap`]; the load of sector `j` in its own components is `cos(j theta) f_a - sin(j theta)
/// f_b`. Reference loads on rigid sets likewise, one list per part.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicLoad {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// The cosine part `f_a`.
    pub cos: Vec<f64>,
    /// The sine part `f_b` (zero for `k = 0` and `2 k = N`).
    pub sin: Vec<f64>,
    /// Forces and moments at reference points, cosine part.
    pub reference_cos: Vec<ReferenceLoad>,
    /// Forces and moments at reference points, sine part.
    pub reference_sin: Vec<ReferenceLoad>,
}

impl HarmonicLoad {
    /// A harmonic load with its cosine and sine parts and no reference loads.
    pub fn new(harmonic: usize, cos: Vec<f64>, sin: Vec<f64>) -> HarmonicLoad {
        HarmonicLoad {
            harmonic,
            cos,
            sin,
            reference_cos: Vec::new(),
            reference_sin: Vec::new(),
        }
    }
    /// The same load with reference loads (cosine and sine parts).
    pub fn references(mut self, cos: Vec<ReferenceLoad>, sin: Vec<ReferenceLoad>) -> HarmonicLoad {
        self.reference_cos = cos;
        self.reference_sin = sin;
        self
    }
}

/// The solution of a harmonic static load on a sector (ADR 0395).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicStatic {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// The number of sectors `N`.
    pub sectors: usize,
    /// The cosine part `a` over the model's [`DofMap`].
    pub cos: Vec<f64>,
    /// The sine part `b` (zero for `k = 0` and `2 k = N`).
    pub sin: Vec<f64>,
    /// The reference motions of the rigid sets, cosine part, in the order of the constraints.
    pub reference_cos: Vec<ReferenceMotion>,
    /// The reference motions of the rigid sets, sine part (zero for a single harmonic).
    pub reference_sin: Vec<ReferenceMotion>,
    /// `|f_r - K_r q| / |f_r|`.
    pub relative_residual: f64,
    /// Number of independent unknowns.
    pub unknowns: usize,
    /// The transformation.
    pub transformation: Transformation,
    /// `T^T K T` (`K` doubled block-diagonally for a pair).
    pub reduced_matrix: CsrMatrix,
}

fn harmonic_load_error(reason: String) -> FemError {
    FemError::InvalidValue {
        key: "harmonic load",
        reason,
    }
}

/// The name of unknown `i` of a (harmonic) transformation: "node 4 component 1", "node 4 component 1 (sine part)" or "the
/// reference rotation y of constraint 3 (RigidSet, sine part)".
pub(crate) fn unknown_label(tr: &Transformation, model: &SolidModel, i: usize) -> String {
    let n = model.dof_map().n_equations();
    if i < tr.n_mesh() {
        let (node, component) = model.dof_map().node_component(i % n).expect("mesh unknown");
        let part = if i >= n { " (sine part)" } else { "" };
        return format!("node {node} component {component}{part}");
    }
    let Some(&(constraint, r0)) = tr.references().iter().rev().find(|&&(_, o)| o <= i) else {
        return format!("unknown {i}");
    };
    let first = tr
        .references()
        .iter()
        .find(|&&(c, _)| c == constraint)
        .map_or(r0, |&(_, o)| o);
    let part = if r0 > first { ", sine part" } else { "" };
    let what = [
        "translation x",
        "translation y",
        "translation z",
        "rotation x",
        "rotation y",
        "rotation z",
    ][(i - r0).min(REFERENCE_UNKNOWNS - 1)];
    format!("the reference {what} of constraint {constraint} (RigidSet{part})")
}

/// The static solve of a harmonic load on a sector with one cyclic constraint (ADR 0395): `k = 0` on
/// [`mpc::transformation`] (any prescribed values), `0 < 2 k < N` on the doubled problem over the cosine and sine parts and
/// `2 k = N` on the single one ([`mpc::harmonic_transformation`], zero values only), rigid sets with their reference
/// unknowns per part; the direct solver only. The full solution is [`FullRotor::expand_field`] of the result under the load
/// [`FullRotor::expand_load`]. Refuses a harmonic above `N / 2`, a model with its own loads (give the parts as vectors), a
/// part of the wrong length, a non-finite entry, a non-zero sine part at `k = 0` or `2 k = N`, a reference load on a
/// constraint that is not a rigid set and a solver other than `Direct`.
pub fn solve_static_harmonic(
    model: &SolidModel,
    extra: &[Prescribed],
    load: &HarmonicLoad,
    options: &StaticOptions,
) -> Result<HarmonicStatic> {
    model.validate()?;
    model.check_supported()?;
    let ci = the_cyclic_constraint(model)?;
    let sectors = number_of_sectors(model, ci);
    let harmonic = load.harmonic;
    if 2 * harmonic > sectors {
        return Err(FemError::InvalidValue {
            key: "harmonic",
            reason: format!(
                "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                sectors / 2
            ),
        });
    }
    if !matches!(options.solver, StaticSolver::Direct) {
        return Err(FemError::InvalidValue {
            key: "solver",
            reason: "a harmonic static solve takes the Direct solver only (ADR 0395)".into(),
        });
    }
    if !model.loads().is_empty() {
        return Err(FemError::InvalidValue {
            key: "loads",
            reason: format!(
                "the model has {} loads of its own; a harmonic static solve takes its load as the cosine and sine \
                 vectors (loads::nodal_load_vector of a model with the loads builds them)",
                model.loads().len()
            ),
        });
    }
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    for (key, v) in [("cosine part", &load.cos), ("sine part", &load.sin)] {
        if v.len() != n {
            return Err(FemError::FieldLength {
                key,
                expected: n,
                found: v.len(),
            });
        }
        if let Some(i) = v.iter().position(|x| !x.is_finite()) {
            return Err(harmonic_load_error(format!("{key} entry {i} is not finite")));
        }
    }
    let doubled = harmonic != 0 && 2 * harmonic < sectors;
    if !doubled {
        if let Some(i) = load.sin.iter().position(|&x| x != 0.0) {
            return Err(harmonic_load_error(format!(
                "sine part entry {i} is {:e}: harmonic index {harmonic} of {sectors} sectors has no sine part \
                 (sin(j theta) = 0 in every sector)",
                load.sin[i]
            )));
        }
        if let Some(l) = load
            .reference_sin
            .iter()
            .find(|l| l.force.iter().chain(&l.moment).any(|&x| x != 0.0))
        {
            return Err(harmonic_load_error(format!(
                "sine reference load on constraint {}: harmonic index {harmonic} of {sectors} sectors has no sine part",
                l.constraint
            )));
        }
    }
    let tr = if harmonic == 0 {
        mpc::transformation(model, extra)?
    } else {
        mpc::harmonic_transformation(model, extra, harmonic)?
    };
    let nn = tr.n_unknowns();
    let mut f = load.cos.clone();
    if doubled {
        f.extend_from_slice(&load.sin);
    }
    f.resize(nn, 0.0);
    let parts: &[&[ReferenceLoad]] = if doubled {
        &[&load.reference_cos, &load.reference_sin]
    } else {
        &[&load.reference_cos]
    };
    for (part, list) in parts.iter().enumerate() {
        for l in list.iter() {
            let r0 = tr
                .references()
                .iter()
                .filter(|&&(c, _)| c == l.constraint)
                .nth(part)
                .map(|&(_, o)| o)
                .ok_or_else(|| FemError::InvalidValue {
                    key: "reference load",
                    reason: format!("constraint {} is not a rigid set", l.constraint),
                })?;
            if l.force.iter().chain(&l.moment).any(|v| !v.is_finite()) {
                return Err(FemError::InvalidValue {
                    key: "reference load",
                    reason: format!("force {:?} or moment {:?} is not finite", l.force, l.moment),
                });
            }
            for i in 0..3 {
                f[r0 + i] += l.force[i];
                f[r0 + 3 + i] += l.moment[i];
            }
        }
    }
    let mut k = stiffness_with(model, options.threads)?;
    mpc::add_supports(&mut k, model)?;
    let k = if doubled { doubled_matrix(&k) } else { k };
    let (kr, fr) = tr.reduce(&k, &f)?;
    let q = if kr.n == 0 {
        Vec::new()
    } else {
        let diag = kr.diag();
        let singular = |row: usize, pivot: f64| FemError::Solver {
            solver: "skyline-ldlt",
            reason: format!(
                "singular reduced stiffness at {}: pivot {pivot:e}, diagonal {:e}",
                unknown_label(&tr, model, tr.independent()[row]),
                diag[row]
            ),
        };
        let ldlt = SkylineLdlt::factor(&kr).map_err(|e| match e {
            SkylineError::Pivot { row, pivot, .. } => singular(row, pivot),
            other => FemError::Solver {
                solver: "skyline-ldlt",
                reason: other.to_string(),
            },
        })?;
        for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
            if d <= PIVOT_TOLERANCE * diag[row] {
                return Err(singular(row, d));
            }
        }
        ldlt.solve(&fr)
    };
    let relative_residual = {
        let bn = fr.iter().map(|v| v * v).sum::<f64>().sqrt();
        if bn == 0.0 || q.is_empty() {
            0.0
        } else {
            let mut r = vec![0.0; fr.len()];
            kr.residual(&fr, &q, &mut r);
            r.iter().map(|v| v * v).sum::<f64>().sqrt() / bn
        }
    };
    let x = tr.expand(&q);
    let motion = |constraint: usize, r0: usize| ReferenceMotion {
        constraint,
        translation: [x[r0], x[r0 + 1], x[r0 + 2]],
        rotation: [x[r0 + 3], x[r0 + 4], x[r0 + 5]],
    };
    let mut reference_cos = Vec::new();
    let mut reference_sin = Vec::new();
    for &(c, r0) in tr.references() {
        if reference_cos.iter().any(|m: &ReferenceMotion| m.constraint == c) {
            reference_sin.push(motion(c, r0));
        } else {
            reference_cos.push(motion(c, r0));
            if !doubled {
                reference_sin.push(ReferenceMotion {
                    constraint: c,
                    translation: [0.0; 3],
                    rotation: [0.0; 3],
                });
            }
        }
    }
    Ok(HarmonicStatic {
        harmonic,
        sectors,
        cos: x[..n].to_vec(),
        sin: if doubled { x[n..2 * n].to_vec() } else { vec![0.0; n] },
        reference_cos,
        reference_sin,
        relative_residual,
        unknowns: kr.n,
        transformation: tr,
        reduced_matrix: kr,
    })
}

/// A load vector over `dofs` as nodal forces (one per node with a non-zero force, ascending), for
/// [`mpc::solve_static_mpc`] of the full model under an expanded load (ADR 0395).
pub fn nodal_forces_of(dofs: &DofMap, f: &[f64]) -> Vec<NodalForceAt> {
    let mut out = Vec::new();
    for node in 0..dofs.n_nodes() {
        let mut v = [0.0; 3];
        for (i, vi) in v.iter_mut().enumerate() {
            if let Some(e) = dofs.equation(node, i) {
                *vi = f[e];
            }
        }
        if v.iter().any(|&x| x != 0.0) {
            out.push(NodalForceAt::new(node, v));
        }
    }
    out
}
