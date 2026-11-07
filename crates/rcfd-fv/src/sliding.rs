//! Step data of a sliding (rotating) interface for the incompressible PISO/PIMPLE path (F-250, ADR 0216 M3, M4, M6).
//!
//! From the joined mesh `M0` at the old rotor angle to `M1` at the new one ([`rcfd_mesh::sliding`]) the cells are the
//! same, the faces off the interface are matched, and the interface faces are new. [`sliding_step`] builds what the moving
//! step needs on `M1`:
//!
//! - the mesh flux: the rotor faces' rigid swept volumes over `dt`, zero on stator and empty faces, and on the interface
//!   faces the area-weighted minimum-norm values that close the geometric conservation law (GCL) of the cells next to the
//!   interface, whose slivers change with the overlaps (M3, [`interface_balance`]);
//! - the old-level face flux: mapped faces keep their value; an interface face gets the face interpolation of the old
//!   velocity plus the area-weighted minimum-norm correction that makes every interface cell's old flux divergence-free
//!   (M4);
//! - the old-level Rhie-Chow defect: as `step_ale` on the mapped internal faces, zero on the interface faces (M4).
//!
//! `PimpleSolver::step_sliding` advances one step with it. [`rotating_wall_conditions`] gives a wall on the rotor its
//! velocity and the fixed-flux pressure gradient (M6).

use crate::bc::{FixedValue, Profile};
use crate::convection::face_flux;
use crate::field::VectorField;
use crate::geometry::FvGeometry;
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::sliding::SlidingMesh;
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Result of [`interface_balance`].
#[derive(Debug, Clone)]
pub struct InterfaceSolve {
    /// One value per face of the given list, positive out of the owner.
    pub values: Vec<f64>,
    /// Largest `abs(sum_f sigma_cf x_f - (t_c - mean))` over the cells of the graph.
    pub residual: f64,
    /// Largest absolute mean of the targets over a connected part of the graph (removed before the solve).
    pub removed_mean: f64,
    /// Conjugate-gradient iterations, summed over the refinement passes.
    pub iterations: usize,
}

/// Face values `x_f` on the internal faces `faces` whose sums over the cells they touch equal `target` (one value per cell
/// of `mesh`; the cells not touched are ignored), each connected part of the graph (cells as nodes, faces as edges) with
/// the mean of its targets removed: `sum_{f in faces(c)} sigma_cf x_f = t_c - mean`, `sigma_cf = +1` when `c` owns `f`.
/// Of all such values the area-weighted minimum-norm one, `x_f = |S_f| (lambda_owner - lambda_neighbour)` with
/// `L lambda = t - mean` (`L` the graph Laplacian with the face areas as weights), solved by conjugate gradients with two
/// passes of refinement on the true residual (ADR 0216 M3).
pub fn interface_balance(mesh: &Mesh, faces: &[usize], target: &[f64]) -> Result<InterfaceSolve, String> {
    let nc = mesh.n_cells();
    if target.len() != nc {
        return Err(format!(
            "interface_balance: {} targets for a mesh of {nc} cells",
            target.len()
        ));
    }
    let ni = mesh.n_internal_faces();
    if let Some(&f) = faces.iter().find(|&&f| f >= ni) {
        return Err(format!("interface_balance: face {f} is not an internal face"));
    }
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut node = vec![usize::MAX; nc];
    let mut cells: Vec<usize> = Vec::new();
    for &f in faces {
        for c in [own[f], nei[f]] {
            if node[c] == usize::MAX {
                node[c] = cells.len();
                cells.push(c);
            }
        }
    }
    let n = cells.len();
    let edges: Vec<(usize, usize)> = faces.iter().map(|&f| (node[own[f]], node[nei[f]])).collect();
    let w: Vec<f64> = faces.iter().map(|&f| mesh.face_area(f)).collect();
    // connected parts (union-find)
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for &(a, b) in &edges {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb {
            parent[ra] = rb;
        }
    }
    let mut part_of_root = vec![usize::MAX; n];
    let mut part = vec![0usize; n];
    let mut n_parts = 0;
    for i in 0..n {
        let r = root(&mut parent, i);
        if part_of_root[r] == usize::MAX {
            part_of_root[r] = n_parts;
            n_parts += 1;
        }
        part[i] = part_of_root[r];
    }
    let mut count = vec![0.0f64; n_parts];
    for &p in &part {
        count[p] += 1.0;
    }
    let project = |v: &mut [f64]| -> f64 {
        let mut mean = vec![0.0f64; n_parts];
        for (i, x) in v.iter().enumerate() {
            mean[part[i]] += x;
        }
        for (m, c) in mean.iter_mut().zip(&count) {
            *m /= c;
        }
        for (i, x) in v.iter_mut().enumerate() {
            *x -= mean[part[i]];
        }
        mean.iter().fold(0.0f64, |a, m| a.max(m.abs()))
    };
    let mut t: Vec<f64> = cells.iter().map(|&c| target[c]).collect();
    let removed_mean = project(&mut t);
    let apply = |x: &[f64], y: &mut [f64]| {
        y.iter_mut().for_each(|v| *v = 0.0);
        for (e, &(a, b)) in edges.iter().enumerate() {
            let d = w[e] * (x[a] - x[b]);
            y[a] += d;
            y[b] -= d;
        }
    };
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let amax = |a: &[f64]| a.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let tnorm = amax(&t);
    let mut values = vec![0.0; faces.len()];
    let mut iterations = 0;
    let divergence = |x: &[f64]| {
        let mut d = vec![0.0; n];
        for (e, &(a, b)) in edges.iter().enumerate() {
            d[a] += x[e];
            d[b] -= x[e];
        }
        d
    };
    if tnorm > 0.0 {
        let max_iter = 20 * n + 100;
        let mut rhs = t.clone();
        for _pass in 0..3 {
            let rnorm = amax(&rhs);
            if rnorm <= 1e-15 * tnorm {
                break;
            }
            // conjugate gradients on L lambda = rhs (rhs has zero mean on every part)
            let mut lam = vec![0.0; n];
            let mut r = rhs.clone();
            let mut p = r.clone();
            let mut q = vec![0.0; n];
            let mut rr = dot(&r, &r);
            for _ in 0..max_iter {
                apply(&p, &mut q);
                let pq = dot(&p, &q);
                if pq <= 0.0 || !pq.is_finite() {
                    break;
                }
                let alpha = rr / pq;
                for i in 0..n {
                    lam[i] += alpha * p[i];
                    r[i] -= alpha * q[i];
                }
                project(&mut r);
                iterations += 1;
                let rr_new = dot(&r, &r);
                if amax(&r) <= 1e-14 * rnorm {
                    break;
                }
                let beta = rr_new / rr;
                for i in 0..n {
                    p[i] = r[i] + beta * p[i];
                }
                rr = rr_new;
            }
            for (e, &(a, b)) in edges.iter().enumerate() {
                values[e] += w[e] * (lam[a] - lam[b]);
            }
            let d = divergence(&values);
            rhs = t.iter().zip(&d).map(|(a, b)| a - b).collect();
            project(&mut rhs);
        }
    }
    let d = divergence(&values);
    let residual = t.iter().zip(&d).fold(0.0f64, |m, (a, b)| m.max((a - b).abs()));
    if !residual.is_finite() {
        return Err("interface_balance: the solve did not give finite values".into());
    }
    Ok(InterfaceSolve {
        values,
        residual,
        removed_mean,
        iterations,
    })
}

/// What `PimpleSolver::step_sliding` needs for one step from the old to the new joined mesh (ADR 0216 S2). Public so that a
/// caller can inspect or alter it.
#[derive(Debug, Clone)]
pub struct SlidingStep {
    /// Geometry of the new joined mesh.
    pub geom: Arc<FvGeometry>,
    /// Rotor angle of the old mesh.
    pub old_angle: f64,
    /// Rotor angle of the new mesh.
    pub angle: f64,
    /// Point on the axis.
    pub origin: Vec3,
    /// Unit axis.
    pub axis: Vec3,
    /// Mesh flux of every face of the new mesh (`W_f/dt`, positive out of the owner; zero on empty faces).
    pub mesh_flux: Vec<f64>,
    /// Cell volumes of the old mesh.
    pub vol_old: Vec<f64>,
    /// Old-level face flux on the faces of the new mesh (M4).
    pub phi_old: Vec<f64>,
    /// Old-level flux minus the old velocity's face flux on the old geometry, on the internal faces of the new mesh (zero on
    /// the interface faces); the step multiplies it by `w V0_o/V1_o + (1 - w) V0_n/V1_n` as `step_ale`.
    pub old_defect: Vec<f64>,
    /// Interface faces of the new mesh.
    pub interface_faces: Vec<usize>,
    /// `max_c abs(V1_c - V0_c - dt sum_f sigma_cf m_f) / mean(V1)` of this mesh flux.
    pub gcl_residual: f64,
    /// `max_c abs(sum_f sigma_cf phi_old,f) / max_f abs(S_f)` of the remapped old flux.
    pub flux_residual: f64,
}

/// The step data from the old joined mesh `old` (whose geometry the solver holds, `old_geom`, with the face flux `phi_old`
/// and the cell velocity `u_old`) to the new joined mesh `new` of the same pair, for a step of size `dt` (ADR 0216 M2 to
/// M4). Fails when the meshes are not of one pair, the sizes do not fit or the new mesh is not a valid geometry.
pub fn sliding_step(
    old_geom: &FvGeometry,
    old: &SlidingMesh,
    new: &SlidingMesh,
    phi_old: &[f64],
    u_old: &[Vec3],
    dt: f64,
) -> Result<SlidingStep, String> {
    if !(dt > 0.0 && dt.is_finite()) {
        return Err(format!(
            "sliding_step: the step size must be positive and finite (got {dt})"
        ));
    }
    if old_geom.n_cells() != old.mesh.n_cells() || old_geom.mesh.n_faces() != old.mesh.n_faces() {
        return Err("sliding_step: the old geometry is not that of the old sliding mesh".into());
    }
    if phi_old.len() != old.mesh.n_faces() || u_old.len() != old.mesh.n_cells() {
        return Err(format!(
            "sliding_step: {} old face fluxes and {} velocities for a mesh of {} faces and {} cells",
            phi_old.len(),
            u_old.len(),
            old.mesh.n_faces(),
            old.mesh.n_cells()
        ));
    }
    let map = new.face_map_from(old).map_err(|e| e.to_string())?;
    let geom = Arc::new(FvGeometry::new(Arc::new(new.mesh.clone())).map_err(|e| e.to_string())?);
    let mesh = &geom.mesh;
    let (n, nf, ni) = (mesh.n_cells(), mesh.n_faces(), mesh.n_internal_faces());
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let faces = new.report.interface_faces.clone();
    // M3: mesh flux
    let w = new.rotor_swept_volumes(old.angle).map_err(|e| e.to_string())?;
    let mut m: Vec<f64> = w.iter().map(|x| x / dt).collect();
    for (f, x) in m.iter_mut().enumerate() {
        if geom.empty[f] || new.is_interface(f) {
            *x = 0.0;
        }
    }
    let vol_old = old_geom.mesh.cell_volumes().to_vec();
    let vol_new = mesh.cell_volumes();
    let mut t: Vec<f64> = (0..n).map(|c| (vol_new[c] - vol_old[c]) / dt).collect();
    for f in 0..nf {
        if !new.is_interface(f) {
            t[own[f]] -= m[f];
            if f < ni {
                t[nei[f]] += m[f];
            }
        }
    }
    let gcl = interface_balance(mesh, &faces, &t)?;
    for (k, &f) in faces.iter().enumerate() {
        m[f] = gcl.values[k];
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
    // M4: old-level flux and defect
    let interp_new = face_flux(&geom, u_old, &vec![vec3::ZERO; mesh.n_boundary_faces()]);
    let interp_old = face_flux(old_geom, u_old, &vec![vec3::ZERO; old.mesh.n_boundary_faces()]);
    let mut phi = vec![0.0; nf];
    let mut old_defect = vec![0.0; ni];
    for f in 0..nf {
        match map[f] {
            Some(g) => {
                phi[f] = phi_old[g];
                if f < ni {
                    old_defect[f] = phi_old[g] - interp_old[g];
                }
            }
            None => phi[f] = interp_new[f],
        }
    }
    let mut e = vec![0.0; n];
    for f in 0..nf {
        let x = if new.is_interface(f) { 0.0 } else { phi[f] };
        e[own[f]] -= x;
        if f < ni {
            e[nei[f]] += x;
        }
    }
    for &f in &faces {
        e[own[f]] -= interp_new[f];
        e[nei[f]] += interp_new[f];
    }
    let cor = interface_balance(mesh, &faces, &e)?;
    for (k, &f) in faces.iter().enumerate() {
        phi[f] = interp_new[f] + cor.values[k];
    }
    let mut div = vec![0.0; n];
    for f in 0..nf {
        div[own[f]] += phi[f];
        if f < ni {
            div[nei[f]] -= phi[f];
        }
    }
    let smax = mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max);
    let flux_residual = div.iter().fold(0.0f64, |a, x| a.max(x.abs())) / smax.max(1e-300);
    Ok(SlidingStep {
        geom,
        old_angle: old.angle,
        angle: new.angle,
        origin: new.origin(),
        axis: new.axis(),
        mesh_flux: m,
        vol_old,
        phi_old: phi,
        old_defect,
        interface_faces: faces,
        gcl_residual,
        flux_residual,
    })
}

/// Velocity condition of walls on the rotor for the step `step` (ADR 0216 M6): each face of the listed patches gets the
/// tangential part of the rigid velocity `omega_w a x (x_f - o)` at its new centre (`omega_w` the wall's angular velocity
/// about the step's axis; 0 for a wall at rest) plus the normal part that carries the face's mesh flux, `u_b . S_f = m_f`.
/// Returns the fixed-flux pressure gradient `dp/dn = -n . (u_b,new - u_b,old)/dt` per boundary face of the new mesh (index
/// `f - n_internal`), `NaN` on the faces of other patches, with the old value from the same boundary face of `old_geom`
/// (the geometry the solver holds before the step). Pass it to `PimpleSolver::step_sliding`.
pub fn rotating_wall_conditions(
    u: &mut VectorField,
    step: &SlidingStep,
    old_geom: &FvGeometry,
    patches: &[String],
    omega_w: f64,
    dt: f64,
) -> Result<Vec<f64>, String> {
    let g = &step.geom;
    let (ni, nb) = (g.n_internal, g.mesh.n_boundary_faces());
    if old_geom.mesh.n_boundary_faces() != nb {
        return Err("rotating_wall_conditions: the old geometry has another number of boundary faces".into());
    }
    let before = u.boundary_values(old_geom);
    let mut dpdn = vec![f64::NAN; nb];
    let (sf, cf) = (g.mesh.face_area_vectors(), g.mesh.face_centres());
    for name in patches {
        let patch = g
            .mesh
            .patch(name)
            .ok_or_else(|| format!("rotating_wall_conditions: no patch '{name}'"))?;
        let mut v = Vec::with_capacity(patch.size);
        for f in patch.faces() {
            let area = vec3::norm(sf[f]).max(1e-300);
            let nrm = vec3::scale(sf[f], 1.0 / area);
            let rigid = vec3::scale(vec3::cross(step.axis, vec3::sub(cf[f], step.origin)), omega_w);
            let tangential = vec3::sub(rigid, vec3::scale(nrm, vec3::dot(rigid, nrm)));
            let ub = vec3::add(tangential, vec3::scale(nrm, step.mesh_flux[f] / area));
            let acc = vec3::scale(vec3::sub(ub, before[f - ni]), 1.0 / dt);
            dpdn[f - ni] = -vec3::dot(nrm, acc);
            v.push(ub);
        }
        u.set_bc(g, name, Box::new(FixedValue(Profile::PerFace(v))))
            .map_err(|e| e.to_string())?;
    }
    Ok(dpdn)
}
