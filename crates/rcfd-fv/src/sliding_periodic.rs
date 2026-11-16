//! Step data of a planar or sector sliding interface with periodic seams for the incompressible PISO/PIMPLE path (F-304,
//! ADR 0629 M5, M6).
//!
//! From the joined mesh `M0` at the old offset to `M1` at the new one ([`rcfd_mesh::sliding_periodic`]) the cells are the
//! same, the faces off the interface keep their identity, and the direct interface faces and the wrap pairs (a rotor copy
//! and a stator copy coupled as periodic faces) are new. [`periodic_sliding_step`] builds what the moving step needs on
//! `M1`, as `sliding::sliding_step` (ADR 0216 M3, M4) with the wrap pairs as further edges of the interface graph: on a
//! wrap pair the rotor copy carries `x` and the stator copy `-x`, so the pair is antisymmetric as the periodic coupling of
//! the solver requires. `PimpleSolver::step_sliding_periodic` advances one step with it.
//!
//! A step between two meshes of `PeriodicSlidingInterface::mesh_turned` whose images differ by one is a re-imaging step
//! (F-704, ADR 0634 R2): the old mesh is read in the new image, so the rotor's swept volumes are those of the motion from
//! the old offset less one period, the rotor cells' old vectors are turned back by one period for the interpolations on the
//! new faces, and [`PeriodicSlidingStep::reimage`] tells the solver to turn its rotor state likewise.

use crate::convection::face_flux;
use crate::geometry::FvGeometry;
use crate::sliding::graph_balance;
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::motion::{mat_vec, rotation_matrix};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingMesh, Periodicity, WRAP_A, WRAP_B};
use std::borrow::Cow;
use std::sync::Arc;

/// What `PimpleSolver::step_sliding_periodic` needs for one step from the old to the new joined mesh (ADR 0629 S2). Public
/// so that a caller can inspect or alter it.
#[derive(Debug, Clone)]
pub struct PeriodicSlidingStep {
    /// Geometry of the new joined mesh.
    pub geom: Arc<FvGeometry>,
    /// Offset of the old mesh.
    pub old_offset: f64,
    /// Offset of the new mesh.
    pub offset: f64,
    /// Mesh flux of every face of the new mesh (`W_f/dt`, positive out of the owner; zero on empty faces).
    pub mesh_flux: Vec<f64>,
    /// Cell volumes of the old mesh.
    pub vol_old: Vec<f64>,
    /// Old-level face flux on the faces of the new mesh.
    pub phi_old: Vec<f64>,
    /// Old-level Rhie-Chow defect on the internal faces of the new mesh (zero on the interface faces), as
    /// `sliding::SlidingStep::old_defect`.
    pub old_defect: Vec<f64>,
    /// Direct interface faces of the new mesh.
    pub interface_faces: Vec<usize>,
    /// Wrap pairs `(rotor copy, stator copy)` of the new mesh.
    pub wrap_pairs: Vec<(usize, usize)>,
    /// `max_c abs(V1_c - V0_c - dt sum_f sigma_cf m_f) / mean(V1)` of this mesh flux.
    pub gcl_residual: f64,
    /// `max_c abs(sum_f sigma_cf phi_old,f) / max_f abs(S_f)` of the remapped old flux.
    pub flux_residual: f64,
    /// The image change of the step, `new.image - old.image` (`0`, or `1` on a re-imaging step; ADR 0634 R2).
    pub images: i64,
    /// On a re-imaging step of a sector: the rotor cells and the rotation the solver turns their velocity and its history
    /// by before the step (ADR 0634 R2). `None` otherwise (a translation turns no vector).
    pub reimage: Option<Reimage>,
}

/// The re-imaging of the rotor's vectors on one step (F-704, ADR 0634 R2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reimage {
    /// The rotor cells are the first `n_rotor_cells` cells.
    pub n_rotor_cells: usize,
    /// The rotation applied to every vector of a rotor cell (by minus one pitch per image).
    pub rotation: [[f64; 3]; 3],
}

impl PeriodicSlidingStep {
    /// The names of the two wrap patches, whose sizes change from step to step.
    pub fn wrap_patches() -> [&'static str; 2] {
        [WRAP_A, WRAP_B]
    }
}

/// The step data from the old joined mesh `old` (whose geometry the solver holds, `old_geom`, with the face flux `phi_old`
/// and the cell velocity `u_old`) to the new joined mesh `new` of the same pair, for a step of size `dt` (ADR 0629 M4 to
/// M6). Fails when the meshes are not of one pair, the sizes do not fit or the new mesh is not a valid geometry.
///
/// When `new.image` is `old.image + 1` the step is a re-imaging step (F-704, ADR 0634 R2; `u_old` is then the velocity of
/// the old image, as the solver holds it); any other image change is refused.
pub fn periodic_sliding_step(
    old_geom: &FvGeometry,
    old: &PeriodicSlidingMesh,
    new: &PeriodicSlidingMesh,
    phi_old: &[f64],
    u_old: &[Vec3],
    dt: f64,
) -> Result<PeriodicSlidingStep, String> {
    if !(dt > 0.0 && dt.is_finite()) {
        return Err(format!(
            "periodic_sliding_step: the step size must be positive and finite (got {dt})"
        ));
    }
    if old_geom.n_cells() != old.mesh.n_cells() || old_geom.mesh.n_faces() != old.mesh.n_faces() {
        return Err("periodic_sliding_step: the old geometry is not that of the old sliding mesh".into());
    }
    if phi_old.len() != old.mesh.n_faces() || u_old.len() != old.mesh.n_cells() {
        return Err(format!(
            "periodic_sliding_step: {} old face fluxes and {} velocities for a mesh of {} faces and {} cells",
            phi_old.len(),
            u_old.len(),
            old.mesh.n_faces(),
            old.mesh.n_cells()
        ));
    }
    let images = new.image - old.image;
    if images != 0 && images != 1 {
        return Err(format!(
            "periodic_sliding_step: the image changes by {images} (old image {}, new image {}); a step re-images the rotor              by at most one period, forwards",
            old.image, new.image
        ));
    }
    let map = new.face_map_from(old).map_err(|e| e.to_string())?;
    let geom = Arc::new(FvGeometry::new(Arc::new(new.mesh.clone())).map_err(|e| e.to_string())?);
    let mesh = &geom.mesh;
    let (n, nf, ni) = (mesh.n_cells(), mesh.n_faces(), mesh.n_internal_faces());
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let faces = new.report.join.interface_faces.clone();
    let pairs = new.report.wrap_pairs.clone();
    let is_new = |f: usize| new.is_interface(f) || new.is_wrap(f);
    // the interface graph: direct faces, then wrap pairs (rotor copy owner -> stator copy owner)
    let mut edges: Vec<(usize, usize)> = faces.iter().map(|&f| (own[f], nei[f])).collect();
    edges.extend(pairs.iter().map(|&(fa, fb)| (own[fa], own[fb])));
    let mut weights: Vec<f64> = faces.iter().map(|&f| mesh.face_area(f)).collect();
    weights.extend(pairs.iter().map(|&(fa, _)| mesh.face_area(fa)));
    let nd = faces.len();
    let put = |x: &mut [f64], vals: &[f64]| {
        for (k, &f) in faces.iter().enumerate() {
            x[f] = vals[k];
        }
        for (k, &(fa, fb)) in pairs.iter().enumerate() {
            x[fa] = vals[nd + k];
            x[fb] = -vals[nd + k];
        }
    };
    // M5: mesh flux
    let old_offset = if images == 0 {
        old.offset
    } else {
        old.offset - images as f64 * new.periodicity().motion_period()
    };
    let w = new.rotor_swept_volumes(old_offset).map_err(|e| e.to_string())?;
    let mut m: Vec<f64> = w.iter().map(|x| x / dt).collect();
    for (f, x) in m.iter_mut().enumerate() {
        if geom.empty[f] || is_new(f) {
            *x = 0.0;
        }
    }
    let vol_old = old_geom.mesh.cell_volumes().to_vec();
    let vol_new = mesh.cell_volumes();
    let mut t: Vec<f64> = (0..n).map(|c| (vol_new[c] - vol_old[c]) / dt).collect();
    for f in 0..nf {
        if !is_new(f) {
            t[own[f]] -= m[f];
            if f < ni {
                t[nei[f]] += m[f];
            }
        }
    }
    let gcl = graph_balance(&edges, &weights, &t)?;
    put(&mut m, &gcl.values);
    let mut swept = vec![0.0; n];
    for f in 0..nf {
        swept[own[f]] += m[f] * dt;
        if f < ni {
            swept[nei[f]] -= m[f] * dt;
        }
    }
    let mean_v = vol_new.iter().sum::<f64>() / n as f64;
    let gcl_residual = (0..n).fold(0.0f64, |a, c| a.max((vol_new[c] - vol_old[c] - swept[c]).abs())) / mean_v;
    // M6: old-level flux and defect; on a re-imaging step the rotor's vectors in the new image (ADR 0634 R2)
    let reimage = match new.periodicity() {
        Periodicity::Rotation { axis, angle, .. } if images != 0 => Some(Reimage {
            n_rotor_cells: new.n_rotor_cells(),
            rotation: rotation_matrix(axis, -(images as f64) * angle),
        }),
        _ => None,
    };
    let u_new: Cow<[Vec3]> = match &reimage {
        None => Cow::Borrowed(u_old),
        Some(ri) => Cow::Owned(
            u_old
                .iter()
                .enumerate()
                .map(|(c, &v)| {
                    if c < ri.n_rotor_cells {
                        mat_vec(&ri.rotation, v)
                    } else {
                        v
                    }
                })
                .collect(),
        ),
    };
    let u_new: &[Vec3] = &u_new;
    let interp_new = face_flux(&geom, u_new, &vec![vec3::ZERO; mesh.n_boundary_faces()]);
    let interp_old = face_flux(old_geom, u_old, &vec![vec3::ZERO; old.mesh.n_boundary_faces()]);
    let rot = new.wrap_rotation();
    let sf = mesh.face_area_vectors();
    let interp_edge: Vec<f64> = faces
        .iter()
        .map(|&f| interp_new[f])
        .chain(pairs.iter().map(|&(fa, fb)| {
            let (dn_a, dn_b) = (geom.dn[fa - ni], geom.dn[fb - ni]);
            let wa = dn_b / (dn_a + dn_b);
            let ub = u_new[own[fb]];
            let ub = rot.as_ref().map_or(ub, |r| mat_vec(r, ub));
            let ua = u_new[own[fa]];
            let uf = [0, 1, 2].map(|k| wa * ua[k] + (1.0 - wa) * ub[k]);
            vec3::dot(uf, sf[fa])
        }))
        .collect();
    let mut phi = vec![0.0; nf];
    let mut old_defect = vec![0.0; ni];
    for f in 0..nf {
        if let Some(g) = map[f] {
            phi[f] = phi_old[g];
            if f < ni {
                old_defect[f] = phi_old[g] - interp_old[g];
            }
        }
    }
    let mut e = vec![0.0; n];
    for f in 0..nf {
        if !is_new(f) {
            e[own[f]] -= phi[f];
            if f < ni {
                e[nei[f]] += phi[f];
            }
        }
    }
    for (k, &(a, b)) in edges.iter().enumerate() {
        e[a] -= interp_edge[k];
        e[b] += interp_edge[k];
    }
    let cor = graph_balance(&edges, &weights, &e)?;
    let vals: Vec<f64> = interp_edge.iter().zip(&cor.values).map(|(a, b)| a + b).collect();
    put(&mut phi, &vals);
    let mut div = vec![0.0; n];
    for f in 0..nf {
        div[own[f]] += phi[f];
        if f < ni {
            div[nei[f]] -= phi[f];
        }
    }
    let smax = sf.iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
    let flux_residual = div.iter().fold(0.0f64, |a, x| a.max(x.abs())) / smax.max(1e-300);
    Ok(PeriodicSlidingStep {
        geom,
        old_offset: old.offset,
        offset: new.offset,
        mesh_flux: m,
        vol_old,
        phi_old: phi,
        old_defect,
        interface_faces: faces,
        wrap_pairs: pairs,
        gcl_residual,
        flux_residual,
        images,
        reimage,
    })
}
