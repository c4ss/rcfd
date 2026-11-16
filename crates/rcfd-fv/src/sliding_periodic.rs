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
//!
//! At an offset where rotor and stator nodes coincide the graph of direct faces and wrap pairs falls into parts;
//! [`periodic_sliding_step_coupled`] (opt-in, F-721, ADR 0641) then couples the parts through the internal faces and the
//! seam pairs between them, as `sliding::sliding_step` does on the full annulus (ADR 0638), so that the remapped old flux
//! stays divergence-free for flows without the parts' symmetry.
//! [`periodic_rotor_wall_conditions`] gives the no-slip walls of a sector's rotor (and the stator faces of the same patches)
//! their velocity and the fixed-flux pressure gradient of the step, in the rotor's current image (F-711, ADR 0646 P2).

use crate::bc::{FixedValue, Profile};
use crate::convection::face_flux;
use crate::field::VectorField;
use crate::geometry::FvGeometry;
use crate::sliding::{graph_balance, InterfaceSolve};
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
    step_impl(old_geom, old, new, phi_old, u_old, dt, false)
}

/// [`periodic_sliding_step`] with the parts of the interface graph coupled at coinciding nodes (F-721, ADR 0641 D1, D2): when
/// a connected part of the graph of direct faces and wrap pairs keeps a target mean above `1e-10` of the largest target
/// (the GCL closure or the remap of the old flux), that solve is made on the graph together with the faces between the
/// parts (internal faces and seam pairs), whose mapped values receive the correction, so that every cell's old flux is
/// divergence-free also on the steps where rotor and stator nodes coincide. On every other step the result is that of
/// [`periodic_sliding_step`] bit for bit. Opt-in (ADR 0641 Q1).
pub fn periodic_sliding_step_coupled(
    old_geom: &FvGeometry,
    old: &PeriodicSlidingMesh,
    new: &PeriodicSlidingMesh,
    phi_old: &[f64],
    u_old: &[Vec3],
    dt: f64,
) -> Result<PeriodicSlidingStep, String> {
    step_impl(old_geom, old, new, phi_old, u_old, dt, true)
}

/// Relative size of a connected part's target mean above which the parts are coupled: the value of ADR 0638 D3
/// (`sliding::PART_MEAN_TOL`).
const PART_MEAN_TOL: f64 = 1e-10;

/// The coupling edges of the periodic interface graph (ADR 0641 D1).
#[derive(Debug, Clone, Default, PartialEq)]
struct Coupling {
    /// Internal faces between two parts (not direct interface faces), edges `(owner, neighbour)`.
    faces: Vec<usize>,
    /// Seam pairs `(f, g)` (periodic pairs that are not wrap pairs) between two parts, edges `(own[f], own[g])`, `+x` on `f`
    /// and `-x` on `g`.
    seams: Vec<(usize, usize)>,
}

impl Coupling {
    fn is_empty(&self) -> bool {
        self.faces.is_empty() && self.seams.is_empty()
    }
}

/// ADR 0641 D1: the internal faces and the seam pairs whose cells are both cells of the graph `edges` and lie in different
/// connected parts of it; `is_new(f)` marks the direct interface faces and the wrap faces.
fn coupling_edges(
    geom: &FvGeometry,
    edges: &[(usize, usize)],
    is_new: &dyn Fn(usize) -> bool,
) -> Result<Coupling, String> {
    let mesh = &geom.mesh;
    let nc = mesh.n_cells();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut parent: Vec<usize> = (0..nc).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut touched = vec![false; nc];
    for &(a, b) in edges {
        touched[a] = true;
        touched[b] = true;
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    }
    let mut between = |a: usize, b: usize| touched[a] && touched[b] && root(&mut parent, a) != root(&mut parent, b);
    let faces: Vec<usize> = (0..mesh.n_internal_faces())
        .filter(|&f| !is_new(f) && between(own[f], nei[f]))
        .collect();
    let pairs = geom.periodic_pairs().map_err(|e| e.to_string())?;
    let seams: Vec<(usize, usize)> = pairs
        .iter()
        .filter(|p| !is_new(p.f) && !is_new(p.g) && between(own[p.f], own[p.g]))
        .map(|p| (p.f, p.g))
        .collect();
    Ok(Coupling { faces, seams })
}

/// ADR 0641 D2: when the base solve `base` of `target` on the graph `edges` removed a part mean above [`PART_MEAN_TOL`] times
/// the largest target over the graph's cells and coupling edges exist, the values of the solve on the edges "`edges`, the
/// coupling faces, the seam pairs" (weights `weights`, then the face areas), in that order. `None` keeps the base solve. The
/// coupling edges are found on first need and kept in `found`.
fn coupled_solve(
    geom: &FvGeometry,
    edges: &[(usize, usize)],
    weights: &[f64],
    target: &[f64],
    base: &InterfaceSolve,
    is_new: &dyn Fn(usize) -> bool,
    found: &mut Option<Coupling>,
) -> Result<Option<Vec<f64>>, String> {
    let tmax = edges
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .fold(0.0f64, |m, c| m.max(target[c].abs()));
    if base.removed_mean.partial_cmp(&(PART_MEAN_TOL * tmax)) != Some(std::cmp::Ordering::Greater) {
        return Ok(None);
    }
    if found.is_none() {
        *found = Some(coupling_edges(geom, edges, is_new)?);
    }
    let cp = found.as_ref().expect("coupling edges just found");
    if cp.is_empty() {
        return Ok(None);
    }
    let mesh = &geom.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut all = edges.to_vec();
    all.extend(cp.faces.iter().map(|&f| (own[f], nei[f])));
    all.extend(cp.seams.iter().map(|&(f, g)| (own[f], own[g])));
    let mut w = weights.to_vec();
    w.extend(cp.faces.iter().map(|&f| mesh.face_area(f)));
    w.extend(cp.seams.iter().map(|&(f, _)| mesh.face_area(f)));
    Ok(Some(graph_balance(&all, &w, target)?.values))
}

/// Adds the coupling-edge values `vals` (coupling faces, then seam pairs) to `x` (ADR 0641 D2).
fn add_coupling(x: &mut [f64], cp: &Coupling, vals: &[f64]) {
    let nf = cp.faces.len();
    for (k, &f) in cp.faces.iter().enumerate() {
        x[f] += vals[k];
    }
    for (k, &(f, g)) in cp.seams.iter().enumerate() {
        x[f] += vals[nf + k];
        x[g] -= vals[nf + k];
    }
}

/// The body of [`periodic_sliding_step`] (`couple` false: the base, F-304 and F-704) and of
/// [`periodic_sliding_step_coupled`] (`couple` true, ADR 0641).
fn step_impl(
    old_geom: &FvGeometry,
    old: &PeriodicSlidingMesh,
    new: &PeriodicSlidingMesh,
    phi_old: &[f64],
    u_old: &[Vec3],
    dt: f64,
    couple: bool,
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
    let mut found: Option<Coupling> = None;
    let coupled_gcl = if couple {
        coupled_solve(&geom, &edges, &weights, &t, &gcl, &is_new, &mut found)?
    } else {
        None
    };
    match coupled_gcl {
        None => put(&mut m, &gcl.values),
        // ADR 0641 D2: a part with a mean above rounding, coupled through the faces and seam pairs between the parts
        Some(values) => {
            put(&mut m, &values[..edges.len()]);
            let cp = found.as_ref().expect("coupling edges of a coupled solve");
            add_coupling(&mut m, cp, &values[edges.len()..]);
        }
    }
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
    let coupled_cor = if couple {
        coupled_solve(&geom, &edges, &weights, &e, &cor, &is_new, &mut found)?
    } else {
        None
    };
    match coupled_cor {
        None => {
            let vals: Vec<f64> = interp_edge.iter().zip(&cor.values).map(|(a, b)| a + b).collect();
            put(&mut phi, &vals);
        }
        // ADR 0641 D2: as for the mesh flux; the defect of a coupling face stays its mapped one
        Some(values) => {
            let vals: Vec<f64> = interp_edge.iter().zip(&values).map(|(a, b)| a + b).collect();
            put(&mut phi, &vals);
            let cp = found.as_ref().expect("coupling edges of a coupled solve");
            add_coupling(&mut phi, cp, &values[edges.len()..]);
        }
    }
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

/// The wall velocity of the listed patches of a sector pair for the step `step` to the joined mesh `new` (F-711, ADR 0646
/// P2), as `sliding::rotor_wall_conditions` (ADR 0637 D4) on the periodic mesh: a rotor face (`new.is_rotor_face`) gets the
/// velocity of a wall turning at `omega_w` about the pair's axis, `(omega_w a x (x_f - o))_t + n_f m_f / |S_f|` at the face
/// centre as `new` shows it (in the rotor's current image), a stator face that of a wall at rest (`0`, its mesh flux being
/// zero); the result is the fixed-flux gradient `dp/dn = -n . (u_b,new - u_b,old) / dt` of every boundary face of the listed
/// patches (NaN elsewhere). On a re-imaging step (`step.reimage` set) the old value of a rotor face is turned by
/// `step.reimage.rotation` first (it was held in the old image, ADR 0634 R2). `old_geom` is the geometry the solver holds
/// (that of the old mesh), on which `u`'s current boundary values are the old ones. A planar pair is refused.
pub fn periodic_rotor_wall_conditions(
    u: &mut VectorField,
    step: &PeriodicSlidingStep,
    old_geom: &FvGeometry,
    new: &PeriodicSlidingMesh,
    patches: &[String],
    omega_w: f64,
    dt: f64,
) -> Result<Vec<f64>, String> {
    let (origin, axis) = match new.periodicity() {
        Periodicity::Rotation { origin, axis, .. } => (origin, vec3::unit(axis)),
        Periodicity::Translation { .. } => {
            return Err(
                "periodic_rotor_wall_conditions: a planar pair has no rotor walls here (sector pairs only)".into(),
            )
        }
    };
    let g = &step.geom;
    let (ni, nb) = (g.n_internal, g.mesh.n_boundary_faces());
    if new.mesh.n_faces() != g.mesh.n_faces() || new.mesh.n_cells() != g.mesh.n_cells() {
        return Err("periodic_rotor_wall_conditions: the sliding mesh is not the one of the step".into());
    }
    if old_geom.mesh.n_boundary_faces() < new.n_fixed_boundary_faces()
        || old_geom.n_cells() != g.n_cells()
        || step.mesh_flux.len() != g.mesh.n_faces()
    {
        return Err("periodic_rotor_wall_conditions: the old geometry or the step data do not fit the new mesh".into());
    }
    let before = u.boundary_values(old_geom);
    let mut dpdn = vec![f64::NAN; nb];
    let (sf, cf, own) = (g.mesh.face_area_vectors(), g.mesh.face_centres(), g.mesh.owner());
    for name in patches {
        let patch = g
            .mesh
            .patch(name)
            .ok_or_else(|| format!("periodic_rotor_wall_conditions: no patch '{name}'"))?;
        if patch.start + patch.size > ni + new.n_fixed_boundary_faces() {
            return Err(format!(
                "periodic_rotor_wall_conditions: patch '{name}' is not an original boundary patch"
            ));
        }
        let mut v = Vec::with_capacity(patch.size);
        for f in patch.faces() {
            let rotor = new.is_rotor_face(f);
            let w = if rotor { omega_w } else { 0.0 };
            let area = vec3::norm(sf[f]).max(1e-300);
            let nrm = vec3::scale(sf[f], 1.0 / area);
            let rigid = vec3::scale(vec3::cross(axis, vec3::sub(cf[f], origin)), w);
            let tangential = vec3::sub(rigid, vec3::scale(nrm, vec3::dot(rigid, nrm)));
            let ub = vec3::add(tangential, vec3::scale(nrm, step.mesh_flux[f] / area));
            let old = match &step.reimage {
                Some(ri) if rotor && own[f] < ri.n_rotor_cells => mat_vec(&ri.rotation, before[f - ni]),
                _ => before[f - ni],
            };
            let acc = vec3::scale(vec3::sub(ub, old), 1.0 / dt);
            dpdn[f - ni] = -vec3::dot(nrm, acc);
            v.push(ub);
        }
        u.set_bc(g, name, Box::new(FixedValue(Profile::PerFace(v))))
            .map_err(|e| e.to_string())?;
    }
    Ok(dpdn)
}

#[cfg(test)]
mod tests {
    //! T1 of ADR 0641 (registered before this code ran).
    use super::*;
    use rcfd_mesh::gen::{annulus_sector, Grading};
    use rcfd_mesh::sliding_periodic::PeriodicSlidingInterface;
    use std::f64::consts::{FRAC_PI_3, PI};

    fn pair_ss() -> PeriodicSlidingInterface {
        let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
        let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
        PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, [0.0, 0.0, 1.0], FRAC_PI_3, 1e-8).unwrap()
    }

    /// T1: on SS at a coincidence (`r = pi/72`: one seam pair; `r = 0`: both seams coincide, two seam pairs) the coupling
    /// edges are 8, and the coupled solve of a target with a zero total and part means of order one meets every target.
    #[test]
    fn t1_coupling_edges_and_coupled_solve_on_the_sector_at_coincidences() {
        let sc = pair_ss();
        for (r, seams_expected) in [(PI / 72.0, 1usize), (0.0, 2)] {
            let m = sc.mesh_at(r).unwrap();
            let geom = FvGeometry::new(Arc::new(m.mesh.clone())).unwrap();
            let mesh = &geom.mesh;
            let (own, nei) = (mesh.owner(), mesh.neighbour());
            let faces = &m.report.join.interface_faces;
            let pairs = &m.report.wrap_pairs;
            let is_new = |f: usize| m.is_interface(f) || m.is_wrap(f);
            let mut edges: Vec<(usize, usize)> = faces.iter().map(|&f| (own[f], nei[f])).collect();
            edges.extend(pairs.iter().map(|&(fa, fb)| (own[fa], own[fb])));
            let mut weights: Vec<f64> = faces.iter().map(|&f| mesh.face_area(f)).collect();
            weights.extend(pairs.iter().map(|&(fa, _)| mesh.face_area(fa)));
            let mut on = vec![false; mesh.n_cells()];
            for &(a, b) in &edges {
                on[a] = true;
                on[b] = true;
            }
            let cells: Vec<usize> = (0..mesh.n_cells()).filter(|&c| on[c]).collect();
            let mut t = vec![0.0; mesh.n_cells()];
            for &c in &cells {
                t[c] = (1.7 * c as f64).sin() + 0.3 * (c as f64 * 0.05).cos();
            }
            let mean = cells.iter().map(|&c| t[c]).sum::<f64>() / cells.len() as f64;
            for &c in &cells {
                t[c] -= mean;
            }
            let tmax = cells.iter().fold(0.0f64, |a, &c| a.max(t[c].abs()));
            let base = graph_balance(&edges, &weights, &t).unwrap();
            let mut found = None;
            let values = coupled_solve(&geom, &edges, &weights, &t, &base, &is_new, &mut found)
                .unwrap()
                .expect("coupled");
            let cp = found.unwrap();
            let mut all = edges.clone();
            all.extend(cp.faces.iter().map(|&f| (own[f], nei[f])));
            all.extend(cp.seams.iter().map(|&(f, g)| (own[f], own[g])));
            let mut d = vec![0.0; mesh.n_cells()];
            for (k, &(a, b)) in all.iter().enumerate() {
                d[a] += values[k];
                d[b] -= values[k];
            }
            let res = cells.iter().fold(0.0f64, |a, &c| a.max((d[c] - t[c]).abs())) / tmax;
            println!(
                "T1 r = {r:.6}: base removed mean {:.3e}, coupling faces {}, seam pairs {} ({seams_expected}), residual \
                 {res:.2e} max|t| (1e-12)",
                base.removed_mean,
                cp.faces.len(),
                cp.seams.len()
            );
            assert_eq!(cp.faces.len() + cp.seams.len(), 8, "T1 r = {r}: coupling edges");
            assert_eq!(cp.seams.len(), seams_expected, "T1 r = {r}: seam pairs");
            assert!(res <= 1e-12, "T1 r = {r}: residual {res:.2e}");
        }
    }
}

#[cfg(test)]
mod tests_f711 {
    //! W1p to W4p of ADR 0646 (registered before this code ran).
    use super::*;
    use crate::sliding::{rotor_wall_conditions, sliding_step, SlidingStep};
    use rcfd_mesh::gen::{annulus_3d, annulus_3d_sector, rectilinear_2d, Grading};
    use rcfd_mesh::sliding::{SlidingInterface, SlidingMesh};
    use rcfd_mesh::sliding_periodic::PeriodicSlidingInterface;

    const Z: Vec3 = [0.0, 0.0, 1.0];
    const DT: f64 = 0.005;

    fn pitch() -> f64 {
        60f64.to_radians()
    }

    /// The sector pair of the tree's set-ups (ADR 0646 SP) and the full annulus TP of ADR 0637.
    fn pairs() -> (PeriodicSlidingInterface, SlidingInterface) {
        let a = annulus_3d_sector(24, 8, 1, 1.0, 1.5, 0.1, pitch(), Grading::Uniform).unwrap();
        let b = annulus_3d_sector(20, 8, 1, 1.5, 2.0, 0.1, pitch(), Grading::Uniform).unwrap();
        let sc = PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, pitch(), 1e-8).unwrap();
        let a = annulus_3d(144, 8, 1, 1.0, 1.5, 0.1, 0.0, Grading::Uniform).unwrap();
        let b = annulus_3d(120, 8, 1, 1.5, 2.0, 0.1, 0.0, Grading::Uniform).unwrap();
        let fa = SlidingInterface::cylindrical(a, "outer", b, "inner", vec3::ZERO, Z, 1e-8).unwrap();
        (sc, fa)
    }

    const WALLS: [&str; 2] = ["inner", "zmin"];

    /// The old state of WP on the geometry `g0`: `u = (x, -y, 0.1 z)`, the walls `inner` and `zmin` holding the tangential
    /// rigid velocity `(a x x_f)_t` of their faces, every other non-cyclic patch fixed 0; its face flux.
    fn old_state(g0: &FvGeometry) -> (VectorField, Vec<f64>) {
        let mut u = VectorField::from_fn("U", g0, |x| [x[0], -x[1], 0.1 * x[2]]);
        let (sf, cf) = (g0.mesh.face_area_vectors(), g0.mesh.face_centres());
        for p in g0.mesh.patches() {
            if p.kind == rcfd_mesh::PatchKind::Cyclic {
                continue;
            }
            let v: Vec<Vec3> = p
                .faces()
                .map(|f| {
                    if WALLS.contains(&p.name.as_str()) {
                        let n = vec3::unit(sf[f]);
                        let r = vec3::cross(Z, cf[f]);
                        vec3::sub(r, vec3::scale(n, vec3::dot(r, n)))
                    } else {
                        [0.0; 3]
                    }
                })
                .collect();
            u.set_bc(g0, &p.name, Box::new(FixedValue(Profile::PerFace(v))))
                .unwrap();
        }
        let phi = face_flux(g0, &u.values, &vec![vec3::ZERO; g0.mesh.n_boundary_faces()]);
        (u, phi)
    }

    fn geom(m: &rcfd_mesh::Mesh) -> Arc<FvGeometry> {
        Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
    }

    struct Sector {
        st: PeriodicSlidingStep,
        new: PeriodicSlidingMesh,
        u: VectorField,
        dpdn: Vec<f64>,
    }

    fn sector_step(sc: &PeriodicSlidingInterface, t0: f64, t1: f64) -> Sector {
        let (old, new) = (sc.mesh_turned(t0).unwrap(), sc.mesh_turned(t1).unwrap());
        let g0 = geom(&old.mesh);
        let (mut u, phi) = old_state(&g0);
        let st = periodic_sliding_step(&g0, &old, &new, &phi, &u.values, DT).unwrap();
        let names: Vec<String> = WALLS.iter().map(|s| s.to_string()).collect();
        let dpdn = periodic_rotor_wall_conditions(&mut u, &st, &g0, &new, &names, 1.0, DT).unwrap();
        Sector { st, new, u, dpdn }
    }

    struct Annulus {
        st: SlidingStep,
        u: VectorField,
        dpdn: Vec<f64>,
    }

    fn annulus_step(fa: &SlidingInterface, t0: f64, t1: f64) -> Annulus {
        let (old, new): (SlidingMesh, SlidingMesh) = (fa.mesh_at(t0).unwrap(), fa.mesh_at(t1).unwrap());
        let g0 = geom(&old.mesh);
        let (mut u, phi) = old_state(&g0);
        let st = sliding_step(&g0, &old, &new, &phi, &u.values, DT).unwrap();
        let names: Vec<String> = WALLS.iter().map(|s| s.to_string()).collect();
        let dpdn = rotor_wall_conditions(&mut u, &st, &g0, &new, &names, 1.0, DT).unwrap();
        Annulus { st, u, dpdn }
    }

    /// Largest differences `(values, dpdn)` over the wall faces of the sector against the annulus, the annulus face found at
    /// the sector face centre turned by `turn` and its value turned back; the largest match distance; the stator faces of
    /// `zmin` that are not exactly zero; the faces compared.
    fn compare(s: &Sector, a: &Annulus, turn: f64) -> (f64, f64, f64, usize, usize) {
        let (gs, ga) = (&s.st.geom, &a.st.geom);
        let (vs, va) = (s.u.boundary_values(gs), a.u.boundary_values(ga));
        let (cs, ca) = (gs.mesh.face_centres(), ga.mesh.face_centres());
        let (nis, nia) = (gs.n_internal, ga.n_internal);
        let fwd = rotation_matrix(Z, turn);
        let back = rotation_matrix(Z, -turn);
        let (mut dv, mut dd, mut dist, mut stator_bad, mut n) = (0.0f64, 0.0f64, 0.0f64, 0, 0);
        for name in WALLS {
            let pa: Vec<usize> = ga.mesh.patch(name).unwrap().faces().collect();
            for f in gs.mesh.patch(name).unwrap().faces() {
                let x = mat_vec(&fwd, cs[f]);
                let (g, d) = pa
                    .iter()
                    .map(|&g| (g, vec3::norm(vec3::sub(ca[g], x))))
                    .fold((usize::MAX, f64::INFINITY), |b, c| if c.1 < b.1 { c } else { b });
                dist = dist.max(d);
                let ua = mat_vec(&back, va[g - nia]);
                dv = dv.max(vec3::norm(vec3::sub(vs[f - nis], ua)));
                dd = dd.max((s.dpdn[f - nis] - a.dpdn[g - nia]).abs());
                if name == "zmin" && !s.new.is_rotor_face(f) && vs[f - nis] != [0.0; 3] {
                    stator_bad += 1;
                }
                n += 1;
            }
        }
        (dv, dd, dist, stator_bad, n)
    }

    #[test]
    fn w1p_w3p_step_without_reimaging_equals_the_full_annulus() {
        let (sc, fa) = pairs();
        let s = sector_step(&sc, 0.0123, 0.0173);
        let a = annulus_step(&fa, 0.0123, 0.0173);
        assert!(s.st.reimage.is_none());
        let (dv, dd, dist, bad, n) = compare(&s, &a, 0.0);
        println!(
            "W1p: {n} faces, match {dist:.2e} (1e-9), values {dv:.2e} (1e-12), dpdn {dd:.2e} (1e-9); W3p: {bad} stator faces of zmin not zero"
        );
        assert!(n > 0 && dist <= 1e-9 && dv <= 1e-12 && dd <= 1e-9, "W1p");
        assert_eq!(bad, 0, "W3p");
    }

    #[test]
    fn w2p_w3p_reimaging_step_equals_the_full_annulus_in_the_image() {
        let (sc, fa) = pairs();
        let p = pitch();
        let s = sector_step(&sc, p - 0.0025, p + 0.0025);
        let a = annulus_step(&fa, p - 0.0025, p + 0.0025);
        assert!(s.st.reimage.is_some() && s.new.image == 1);
        let (dv, dd, dist, bad, n) = compare(&s, &a, p);
        println!(
            "W2p: {n} faces, match {dist:.2e} (1e-9), values {dv:.2e} (1e-12), dpdn {dd:.2e} (1e-9); W3p: {bad} stator faces of zmin not zero"
        );
        assert!(n > 0 && dist <= 1e-9 && dv <= 1e-12 && dd <= 1e-9, "W2p");
        assert_eq!(bad, 0, "W3p");
    }

    #[test]
    fn w4p_refusals() {
        let (sc, _) = pairs();
        let s = sector_step(&sc, 0.0123, 0.0173);
        let names = vec!["inner".to_string()];
        let other = sc.mesh_turned(0.0).unwrap(); // 40 direct faces (coincident nodes), not the step's 44
        let g0 = geom(&sc.mesh_turned(0.0123).unwrap().mesh);
        let mut u = old_state(&g0).0;
        let e1 = periodic_rotor_wall_conditions(&mut u, &s.st, &g0, &other, &names, 1.0, DT);
        // a planar pair
        let xs: Vec<f64> = (0..=20).map(|i| i as f64 / 20.0).collect();
        let xb: Vec<f64> = (0..=16).map(|i| i as f64 / 16.0).collect();
        let ya: Vec<f64> = (0..=4).map(|i| 0.5 + 0.5 * i as f64 / 4.0).collect();
        let yb: Vec<f64> = (0..=4).map(|i| 0.5 * i as f64 / 4.0).collect();
        let per = |mut m: rcfd_mesh::Mesh| {
            m.set_periodic("xmin", "xmax").unwrap();
            m.set_periodic("xmax", "xmin").unwrap();
            m
        };
        let a = per(rectilinear_2d(&xs, &ya, 0.1).unwrap());
        let b = per(rectilinear_2d(&xb, &yb, 0.1).unwrap());
        let pp = PeriodicSlidingInterface::planar(a, "ymin", b, "ymax", [1.0, 0.0, 0.0], 1e-8).unwrap();
        let (o, nw) = (pp.mesh_turned(0.01).unwrap(), pp.mesh_turned(0.015).unwrap());
        let gp = geom(&o.mesh);
        let mut up = VectorField::uniform("U", &gp, [1.0, 0.0, 0.0]);
        let phi = face_flux(&gp, &up.values, &vec![vec3::ZERO; gp.mesh.n_boundary_faces()]);
        let stp = periodic_sliding_step(&gp, &o, &nw, &phi, &up.values, DT).unwrap();
        let e2 = periodic_rotor_wall_conditions(&mut up, &stp, &gp, &nw, &["ymax".to_string()], 1.0, DT);
        println!(
            "W4p: another mesh {:?}; planar pair {:?}",
            e1.as_ref().err(),
            e2.as_ref().err()
        );
        assert!(e1.is_err() && e2.is_err());
    }
}
