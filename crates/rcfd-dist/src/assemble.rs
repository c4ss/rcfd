//! Distributed assembly of one steady laminar problem (ADR 0058, D10): steady scalar convection-diffusion with a prescribed
//! velocity field on a partitioned mesh. Each rank builds a [`RankGeometry`] holding only the geometry of the faces that touch
//! its owned cells (plus centres of the ghost cells) and assembles only its owned rows; the Green-Gauss gradient needs one halo
//! exchange of the field, the deferred linear-upwind correction one more of the gradient components.
//!
//! This is NOT a distributed SIMPLE solver: momentum, pressure-velocity coupling, turbulence and the boundary-condition
//! catalogue are not distributed.

use crate::comm::{CommError, Communicator};
use crate::halo::HaloPlan;
use crate::krylov::gnorm2;
use crate::matrix::DistCsr;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_linalg::solvers::{SolveControl, SolveStats};
use rcfd_linalg::CsrMatrix;
use rcfd_mesh::mesh::{Mesh, PatchKind};
use rcfd_mesh::partition::Decomposition;
use std::collections::HashMap;

/// Steady scalar transport `div(u phi) - div(Gamma grad phi) = 0` with a prescribed velocity and Dirichlet values on every
/// non-empty patch.
#[derive(Clone, Copy)]
pub struct ScalarTransport {
    /// Diffusivity `Gamma`.
    pub gamma: f64,
    /// Prescribed velocity at a point.
    pub velocity: fn(Vec3) -> Vec3,
    /// Dirichlet value for a patch name at a face centre.
    pub bc: fn(&str, Vec3) -> f64,
}

/// One face seen by a rank.
#[derive(Debug, Clone)]
pub struct LocalFace {
    /// Global face id.
    pub gid: usize,
    /// Owner side, local cell id.
    pub lo: usize,
    /// Neighbour side, local cell id; `usize::MAX` for a boundary face.
    pub ln: usize,
    /// Face centre.
    pub centre: Vec3,
    /// Area vector (owner to neighbour; outward on the boundary).
    pub s: Vec3,
    /// Interpolation weight of the owner cell (internal faces).
    pub w: f64,
    /// Orthogonal diffusion distance: centre to centre (internal) or centre to face (boundary).
    pub d: f64,
    /// Volumetric flux `u . S`.
    pub flux: f64,
    /// Dirichlet value (boundary faces).
    pub bc: f64,
}

/// Geometry of one rank: owned and ghost cell centres, owned volumes and the faces touching owned cells in ascending global
/// face order.
#[derive(Debug, Clone)]
pub struct RankGeometry {
    /// This rank.
    pub rank: usize,
    /// Owned cell count.
    pub n_owned: usize,
    /// Owned plus ghost count.
    pub n_local: usize,
    /// Cell centres in local numbering.
    pub centre: Vec<Vec3>,
    /// Volumes of owned cells.
    pub volume: Vec<f64>,
    /// Faces touching an owned cell (empty-patch faces omitted).
    pub faces: Vec<LocalFace>,
    /// For each owned cell the indices into `faces`, ascending.
    pub cell_faces: Vec<Vec<usize>>,
}

impl RankGeometry {
    /// Collect the geometry rank `rank` needs from `mesh`.
    pub fn build(mesh: &Mesh, dec: &Decomposition, rank: usize, prob: &ScalarTransport) -> Result<Self> {
        let sub = dec.subdomains.get(rank).ok_or_else(|| {
            Error::Mesh(format!(
                "rank {rank} is not a part of a {}-part decomposition",
                dec.n_parts
            ))
        })?;
        let local_of: HashMap<usize, usize> = sub
            .owned
            .iter()
            .chain(sub.ghosts.iter())
            .enumerate()
            .map(|(i, &c)| (c, i))
            .collect();
        let cc = mesh.cell_centres();
        let centre: Vec<Vec3> = sub.owned.iter().chain(sub.ghosts.iter()).map(|&c| cc[c]).collect();
        let volume: Vec<f64> = sub.owned.iter().map(|&c| mesh.cell_volumes()[c]).collect();
        let n_owned = sub.n_owned();
        let mut faces = Vec::new();
        let mut cell_faces = vec![Vec::new(); n_owned];
        for f in 0..mesh.n_internal_faces() {
            let (a, b) = (mesh.owner()[f], mesh.neighbour()[f]);
            let (la, lb) = (local_of.get(&a).copied(), local_of.get(&b).copied());
            let owned_a = la.is_some_and(|i| i < n_owned);
            let owned_b = lb.is_some_and(|i| i < n_owned);
            if !owned_a && !owned_b {
                continue;
            }
            let (Some(lo), Some(ln)) = (la, lb) else {
                return Err(Error::Mesh(format!(
                    "rank {rank}: face {f} touches an owned cell but its other cell ({}) is not a ghost",
                    if la.is_none() { a } else { b }
                )));
            };
            let s = mesh.face_area_vectors()[f];
            let fc = mesh.face_centres()[f];
            let area = vec3::norm(s);
            let nh = vec3::scale(s, 1.0 / area);
            let dp = vec3::dot(vec3::sub(fc, cc[a]), nh).abs();
            let dn = vec3::dot(vec3::sub(cc[b], fc), nh).abs();
            let idx = faces.len();
            faces.push(LocalFace {
                gid: f,
                lo,
                ln,
                centre: fc,
                s,
                w: dn / (dp + dn),
                d: vec3::norm(vec3::sub(cc[b], cc[a])),
                flux: vec3::dot((prob.velocity)(fc), s),
                bc: 0.0,
            });
            if owned_a {
                cell_faces[lo].push(idx);
            }
            if owned_b {
                cell_faces[ln].push(idx);
            }
        }
        // boundary faces: interleave in ascending global id after the internal ones (ids are larger)
        for patch in mesh.patches() {
            if patch.kind == PatchKind::Empty {
                continue;
            }
            for f in patch.start..patch.start + patch.size {
                let a = mesh.owner()[f];
                let Some(&lo) = local_of.get(&a).filter(|&&i| i < n_owned) else {
                    continue;
                };
                let s = mesh.face_area_vectors()[f];
                let fc = mesh.face_centres()[f];
                let area = vec3::norm(s);
                let nh = vec3::scale(s, 1.0 / area);
                let d = vec3::dot(vec3::sub(fc, cc[a]), nh).abs();
                let idx = faces.len();
                faces.push(LocalFace {
                    gid: f,
                    lo,
                    ln: usize::MAX,
                    centre: fc,
                    s,
                    w: 1.0,
                    d,
                    flux: vec3::dot((prob.velocity)(fc), s),
                    bc: (prob.bc)(&patch.name, fc),
                });
                cell_faces[lo].push(idx);
            }
        }
        for cf in cell_faces.iter_mut() {
            cf.sort_by_key(|&i| faces[i].gid);
        }
        Ok(RankGeometry {
            rank,
            n_owned,
            n_local: sub.n_local(),
            centre,
            volume,
            faces,
            cell_faces,
        })
    }
}

/// Green-Gauss gradient of a field with linear face interpolation. `phi` has `n_local` entries; its ghosts are updated first.
/// Returns the three components, each of length `n_owned`. `bc_phi` is used on boundary faces (`LocalFace::bc`).
pub fn green_gauss_gradient(
    geom: &RankGeometry,
    plan: &HaloPlan,
    comm: &dyn Communicator,
    phi: &mut [f64],
) -> std::result::Result<[Vec<f64>; 3], CommError> {
    plan.exchange(comm, phi)?;
    Ok(gradient_local(geom, phi))
}

fn gradient_local(geom: &RankGeometry, phi: &[f64]) -> [Vec<f64>; 3] {
    let mut g = [
        vec![0.0; geom.n_owned],
        vec![0.0; geom.n_owned],
        vec![0.0; geom.n_owned],
    ];
    for c in 0..geom.n_owned {
        let mut acc = [0.0; 3];
        for &fi in &geom.cell_faces[c] {
            let f = &geom.faces[fi];
            let (pf, sign) = if f.ln == usize::MAX {
                (f.bc, 1.0)
            } else {
                (
                    f.w * phi[f.lo] + (1.0 - f.w) * phi[f.ln],
                    if f.lo == c { 1.0 } else { -1.0 },
                )
            };
            for (a, s) in acc.iter_mut().zip(f.s) {
                *a += sign * pf * s;
            }
        }
        for (k, a) in acc.iter().enumerate() {
            g[k][c] = a / geom.volume[c];
        }
    }
    g
}

/// Constant matrix (upwind convection plus central diffusion) of the owned rows, in the local numbering.
pub fn assemble_matrix(geom: &RankGeometry, sub: &rcfd_mesh::partition::SubDomain, prob: &ScalarTransport) -> DistCsr {
    let mut rows: Vec<Vec<(usize, f64)>> = Vec::with_capacity(geom.n_owned);
    for c in 0..geom.n_owned {
        let mut diag = 0.0;
        let mut off: Vec<(usize, f64)> = Vec::new();
        for &fi in &geom.cell_faces[c] {
            let f = &geom.faces[fi];
            if f.ln == usize::MAX {
                diag += prob.gamma * vec3::norm(f.s) / f.d;
                continue;
            }
            let (other, sign) = if f.lo == c { (f.ln, 1.0) } else { (f.lo, -1.0) };
            let fc = sign * f.flux;
            let dd = prob.gamma * vec3::norm(f.s) / f.d;
            diag += fc.max(0.0) + dd;
            off.push((other, fc.min(0.0) - dd));
        }
        let mut row = vec![(c, diag)];
        row.extend(off);
        rows.push(row);
    }
    DistCsr::from_rows(sub, rows)
}

/// Right-hand side: boundary terms plus the explicit deferred-correction term from the gradient components (`grad` has
/// `n_local` entries per component, ghosts filled).
pub fn assemble_rhs(geom: &RankGeometry, prob: &ScalarTransport, grad: &[Vec<f64>; 3]) -> Vec<f64> {
    let mut b = vec![0.0; geom.n_owned];
    for (c, bc_) in b.iter_mut().enumerate() {
        let mut acc = 0.0;
        for &fi in &geom.cell_faces[c] {
            let f = &geom.faces[fi];
            if f.ln == usize::MAX {
                acc += prob.gamma * vec3::norm(f.s) / f.d * f.bc - f.flux * f.bc;
                continue;
            }
            let sign = if f.lo == c { 1.0 } else { -1.0 };
            let fc = sign * f.flux;
            let up = if f.flux >= 0.0 { f.lo } else { f.ln };
            let g = [grad[0][up], grad[1][up], grad[2][up]];
            let corr = vec3::dot(g, vec3::sub(f.centre, geom.centre[up]));
            acc -= fc * corr;
        }
        *bc_ = acc;
    }
    b
}

/// Outcome of the outer (deferred-correction) iteration on one rank.
#[derive(Debug, Clone)]
pub struct TransportResult {
    /// Owned values.
    pub phi: Vec<f64>,
    /// Outer iterations used.
    pub outer: usize,
    /// Final maximum change between outer iterations (global).
    pub change: f64,
    /// Total inner (Krylov) iterations.
    pub inner: usize,
}

/// Linear solve callback: `(matrix, comm, rhs, x)`.
pub type LinearSolve<'a> = dyn FnMut(&DistCsr, &dyn Communicator, &[f64], &mut [f64]) -> Result<SolveStats> + 'a;

/// Solve the steady transport problem by deferred-correction outer iterations until the global maximum change of `phi`
/// is at most `outer_tol` (or `max_outer` is reached).
pub fn solve_transport(
    geom: &RankGeometry,
    sub: &rcfd_mesh::partition::SubDomain,
    comm: &dyn Communicator,
    prob: &ScalarTransport,
    outer_tol: f64,
    max_outer: usize,
    solver: &mut LinearSolve,
) -> Result<TransportResult> {
    let plan = HaloPlan::from_subdomain(sub);
    let mat = assemble_matrix(geom, sub, prob);
    let mut phi = vec![0.0; geom.n_local];
    let mut outer = 0;
    let mut change = f64::INFINITY;
    let mut inner = 0;
    while outer < max_outer && change > outer_tol {
        plan.exchange(comm, &mut phi)?;
        let g = gradient_local(geom, &phi);
        let mut gl: [Vec<f64>; 3] = [
            vec![0.0; geom.n_local],
            vec![0.0; geom.n_local],
            vec![0.0; geom.n_local],
        ];
        for k in 0..3 {
            gl[k][..geom.n_owned].copy_from_slice(&g[k]);
            plan.exchange(comm, &mut gl[k])?;
        }
        let b = assemble_rhs(geom, prob, &gl);
        let mut x: Vec<f64> = phi[..geom.n_owned].to_vec();
        let st = solver(&mat, comm, &b, &mut x)?;
        inner += st.iterations;
        let local_change = x
            .iter()
            .zip(&phi[..geom.n_owned])
            .fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
        change = comm.allreduce_max(local_change)?;
        phi[..geom.n_owned].copy_from_slice(&x);
        outer += 1;
    }
    plan.exchange(comm, &mut phi)?;
    Ok(TransportResult {
        phi: phi[..geom.n_owned].to_vec(),
        outer,
        change,
        inner,
    })
}

/// Control used by the default inner solve: BiCGStab with block-Jacobi, relative tolerance `1e-12` and an absolute floor of
/// `1e-14` times the global right-hand-side norm.
pub fn default_linear_solve(mat: &DistCsr, comm: &dyn Communicator, b: &[f64], x: &mut [f64]) -> Result<SolveStats> {
    let bn = gnorm2(comm, b)?;
    let ctl = SolveControl {
        tol_abs: 1e-14 * bn,
        tol_rel: 1e-12,
        max_iter: 2000,
    };
    crate::solve::solve_rank(
        mat,
        comm,
        b,
        x,
        crate::solve::Method::BiCgStab,
        crate::solve::Precond::BlockJacobi,
        &ctl,
    )
}

/// Serial reference solve of the same system on a one-part decomposition: the global matrix is the single rank's block and the
/// `rcfd-linalg` BiCGStab with ILU(0) is used instead of the distributed routines.
pub fn serial_linear_solve(mat: &DistCsr, _comm: &dyn Communicator, b: &[f64], x: &mut [f64]) -> Result<SolveStats> {
    let a: &CsrMatrix = &mat.local;
    let bn = rcfd_linalg::vecops::norm2(b);
    let ctl = SolveControl {
        tol_abs: 1e-14 * bn,
        tol_rel: 1e-12,
        max_iter: 2000,
    };
    let m = rcfd_linalg::precond::Ilu0::new(&mat.owned_block());
    Ok(rcfd_linalg::solvers::bicgstab(a, b, x, &m, &ctl))
}
