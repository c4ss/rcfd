//! Moving-mesh helpers (F-063, ADR 0033): move a geometry to new point positions with its mesh flux, and the
//! velocity of a patch that follows the mesh.

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
