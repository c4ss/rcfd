//! Moving-mesh helpers (F-063, ADR 0033): move a geometry to new point positions with its mesh flux, and the
//! velocity of a patch that follows the mesh or is a wall at rest on it (ADR 0183 P-B).

use crate::bc::{FixedValue, Profile};
use crate::field::VectorField;
use crate::geometry::FvGeometry;
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::motion::{min_volume_ratio, moved_mesh, swept_volumes};
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// A mesh moved to new points over one step.
#[derive(Debug, Clone)]
pub struct MeshStep {
    /// Geometry of the moved mesh.
    pub geom: Arc<FvGeometry>,
    /// Mesh flux `W_f/dt` of every face (positive out of the owner).
    pub mesh_flux: Vec<f64>,
    /// Smallest ratio of new to old cell volume.
    pub min_volume_ratio: f64,
}

/// Move `geom` to the point positions `new_points` during a step of size `dt`. Fails with a message when the moved mesh is not
/// valid (non-positive volume and the other checks of `Mesh::validate`).
pub fn move_mesh(geom: &FvGeometry, new_points: Vec<Vec3>, dt: f64) -> Result<MeshStep, String> {
    let w = swept_volumes(&geom.mesh, &new_points);
    let mesh = moved_mesh(&geom.mesh, new_points).map_err(|e| e.to_string())?;
    let ratio = min_volume_ratio(&geom.mesh, &mesh);
    let new_geom = FvGeometry::new(Arc::new(mesh)).map_err(|e| e.to_string())?;
    Ok(MeshStep {
        geom: Arc::new(new_geom),
        mesh_flux: w.iter().map(|x| x / dt).collect(),
        min_volume_ratio: ratio,
    })
}

/// Velocity of every face of `patch` for an impermeable wall that follows the mesh: the tangential part of the mean
/// face-centre velocity over the step plus the normal part that carries exactly the swept volume,
/// `u_b . S = m_f` (ADR 0033 D3).
pub fn follow_velocity(old: &Mesh, new: &Mesh, mesh_flux: &[f64], patch: &str, dt: f64) -> Option<Vec<Vec3>> {
    let p = new.patch(patch)?;
    Some(
        p.faces()
            .map(|f| {
                let w = vec3::scale(vec3::sub(new.face_centres()[f], old.face_centres()[f]), 1.0 / dt);
                let s = new.face_area_vectors()[f];
                let area = vec3::norm(s).max(1e-300);
                let n = vec3::scale(s, 1.0 / area);
                let wt = vec3::sub(w, vec3::scale(n, vec3::dot(w, n)));
                vec3::add(wt, vec3::scale(n, mesh_flux[f] / area))
            })
            .collect(),
    )
}

/// Install [`follow_velocity`] as the velocity condition of the given patches. Returns the outward-normal pressure
/// gradient of the moving walls, `dp/dn = -n . (u_b,new - u_b,old)/dt` (the fixed-flux condition: where the flux is
/// prescribed the pressure gradient balances the wall acceleration), per boundary face (index `f - n_internal`),
/// `NaN` on faces of other patches. Pass it to `PimpleSolver::step_ale_with_walls`.
pub fn set_follow_conditions(
    u: &mut VectorField,
    step: &MeshStep,
    old: &Mesh,
    patches: &[String],
    dt: f64,
) -> Result<Vec<f64>, String> {
    let ni = step.geom.n_internal;
    let mut dpdn = vec![f64::NAN; step.geom.mesh.n_boundary_faces()];
    let before = u.boundary_values(&step.geom);
    for name in patches {
        let v = follow_velocity(old, &step.geom.mesh, &step.mesh_flux, name, dt)
            .ok_or_else(|| format!("no patch '{name}'"))?;
        let patch = step.geom.mesh.patch(name).ok_or_else(|| format!("no patch '{name}'"))?;
        for (k, f) in patch.faces().enumerate() {
            let s = step.geom.mesh.face_area_vectors()[f];
            let n = vec3::scale(s, 1.0 / vec3::norm(s).max(1e-300));
            let a = vec3::scale(vec3::sub(v[k], before[f - ni]), 1.0 / dt);
            dpdn[f - ni] = -vec3::dot(n, a);
        }
        u.set_bc(&step.geom, name, Box::new(FixedValue(Profile::PerFace(v))))
            .map_err(|e| e.to_string())?;
    }
    Ok(dpdn)
}

/// Velocity of every face of `patch` for a wall at rest on a moving mesh (ADR 0183 P-B): no tangential part, and the
/// normal part that carries exactly the swept volume, `u_b = (m_f/|S_f|) n_f` with the area vector of the new mesh, so
/// `u_b . S = m_f` (no relative flux through the wall).
pub fn rest_velocity(new: &Mesh, mesh_flux: &[f64], patch: &str) -> Option<Vec<Vec3>> {
    let p = new.patch(patch)?;
    Some(
        p.faces()
            .map(|f| {
                let s = new.face_area_vectors()[f];
                let area = vec3::norm(s).max(1e-300);
                vec3::scale(s, mesh_flux[f] / (area * area))
            })
            .collect(),
    )
}

/// Install [`rest_velocity`] as the velocity condition of the given patches (ADR 0183 P-B). Returns the fixed-flux
/// pressure gradient of these walls, `dp/dn = -n . (u_b,new - u_b,old)/dt` per boundary face (index `f - n_internal`),
/// `NaN` on faces of other patches, as [`set_follow_conditions`] does for the following walls.
pub fn set_rest_conditions(
    u: &mut VectorField,
    step: &MeshStep,
    patches: &[String],
    dt: f64,
) -> Result<Vec<f64>, String> {
    let ni = step.geom.n_internal;
    let mut dpdn = vec![f64::NAN; step.geom.mesh.n_boundary_faces()];
    let before = u.boundary_values(&step.geom);
    for name in patches {
        let v = rest_velocity(&step.geom.mesh, &step.mesh_flux, name).ok_or_else(|| format!("no patch '{name}'"))?;
        let patch = step.geom.mesh.patch(name).ok_or_else(|| format!("no patch '{name}'"))?;
        for (k, f) in patch.faces().enumerate() {
            let s = step.geom.mesh.face_area_vectors()[f];
            let n = vec3::scale(s, 1.0 / vec3::norm(s).max(1e-300));
            let a = vec3::scale(vec3::sub(v[k], before[f - ni]), 1.0 / dt);
            dpdn[f - ni] = -vec3::dot(n, a);
        }
        u.set_bc(&step.geom, name, Box::new(FixedValue(Profile::PerFace(v))))
            .map_err(|e| e.to_string())?;
    }
    Ok(dpdn)
}

/// What the three-level follow conditions of the next step need of this one (F-345, ADR 0379 D6): the patches, the step
/// size, and per boundary face the mean face-centre velocity and the mesh flux of the step and the wall value before it.
#[derive(Debug, Clone, Default)]
pub struct FollowHistory {
    last: Option<FollowLevel>,
}

#[derive(Debug, Clone)]
struct FollowLevel {
    patches: Vec<String>,
    dt: f64,
    /// Mean face-centre velocity of the step (index `f - n_internal`).
    mean: Vec<Vec3>,
    /// Mesh flux `W_f/dt` of the step (index `f - n_internal`).
    flux: Vec<f64>,
    /// Boundary values of the velocity before the step's conditions were installed.
    wall: Vec<Vec3>,
}

impl FollowHistory {
    /// An empty history: the next call of [`set_follow_conditions_bdf2`] is [`set_follow_conditions`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether no step has been recorded.
    pub fn is_empty(&self) -> bool {
        self.last.is_none()
    }

    /// Forget the recorded step.
    pub fn clear(&mut self) {
        self.last = None;
    }
}

/// [`set_follow_conditions`] for a BDF2 moving step (F-345, ADR 0379 D6). With an empty `hist` it is
/// `set_follow_conditions` bit for bit. With the step before recorded in `hist` (`r = dt / dt_old`, the BDF2 weights
/// `c0, c1, c2`), every face of the patches gets the wall velocity `u_b = (w3 - (w3 . n) n) + (m3 / |S|) n` with
/// `w3 = c0 w^{n+1} - (c2 / r) w^n` the mean face-centre velocities of the two steps extrapolated to the end of the step and
/// `m3 = c0 m^{n+1} - (c2 / r) m^n` the three-level mesh flux of `PimpleSolver::step_ale_with_walls` (so `u_b . S = m3`),
/// and the fixed-flux pressure gradient `dp/dn = -n . (c0 u_b^{n+1} - c1 u_b^n + c2 u_b^{n-1}) / dt`. The step is recorded
/// in `hist`. Fails, leaving `u` and `hist` unchanged, for a patch that does not exist and for other patches or another
/// number of boundary faces than those recorded.
pub fn set_follow_conditions_bdf2(
    u: &mut VectorField,
    step: &MeshStep,
    old: &Mesh,
    patches: &[String],
    dt: f64,
    hist: &mut FollowHistory,
) -> Result<Vec<f64>, String> {
    let ni = step.geom.n_internal;
    let nb = step.geom.mesh.n_boundary_faces();
    for name in patches {
        step.geom.mesh.patch(name).ok_or_else(|| format!("no patch '{name}'"))?;
    }
    if let Some(h) = &hist.last {
        if h.patches != patches || h.mean.len() != nb {
            return Err("set_follow_conditions_bdf2: the patches or the boundary differ from the recorded step".into());
        }
    }
    let before = u.boundary_values(&step.geom);
    let (cn, co) = (step.geom.mesh.face_centres(), old.face_centres());
    let mut mean = vec![[0.0; 3]; nb];
    let mut flux = vec![0.0; nb];
    for name in patches {
        for f in step.geom.mesh.patch(name).expect("checked").faces() {
            mean[f - ni] = vec3::scale(vec3::sub(cn[f], co[f]), 1.0 / dt);
            flux[f - ni] = step.mesh_flux[f];
        }
    }
    let dpdn = match hist.last.take() {
        None => set_follow_conditions(u, step, old, patches, dt)?,
        Some(h) => {
            let r = dt / h.dt;
            let (c0, c1, c2) = ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r));
            let mut dpdn = vec![f64::NAN; nb];
            let mut values: Vec<Vec<Vec3>> = Vec::with_capacity(patches.len());
            for name in patches {
                let patch = step.geom.mesh.patch(name).expect("checked");
                let v: Vec<Vec3> = patch
                    .faces()
                    .map(|f| {
                        let i = f - ni;
                        let w3 = vec3::sub(vec3::scale(mean[i], c0), vec3::scale(h.mean[i], c2 / r));
                        let m3 = c0 * step.mesh_flux[f] - c2 / r * h.flux[i];
                        let s = step.geom.mesh.face_area_vectors()[f];
                        let area = vec3::norm(s).max(1e-300);
                        let n = vec3::scale(s, 1.0 / area);
                        let wt = vec3::sub(w3, vec3::scale(n, vec3::dot(w3, n)));
                        let ub = vec3::add(wt, vec3::scale(n, m3 / area));
                        let a = [0, 1, 2].map(|k| (c0 * ub[k] - c1 * before[i][k] + c2 * h.wall[i][k]) / dt);
                        dpdn[i] = -vec3::dot(n, a);
                        ub
                    })
                    .collect();
                values.push(v);
            }
            for (name, v) in patches.iter().zip(values) {
                if let Err(e) = u.set_bc(&step.geom, name, Box::new(FixedValue(Profile::PerFace(v)))) {
                    hist.last = Some(h);
                    return Err(e.to_string());
                }
            }
            dpdn
        }
    };
    hist.last = Some(FollowLevel {
        patches: patches.to_vec(),
        dt,
        mean,
        flux,
        wall: before,
    });
    Ok(dpdn)
}
