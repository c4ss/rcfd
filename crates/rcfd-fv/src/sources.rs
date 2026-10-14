//! Momentum sources of steady incompressible flow (F-031): porous resistance, rotating
//! reference frame and prescribed accelerations, each restricted to a cell zone.
//!
//! * Porous media use the superficial-velocity formulation (the transported velocity is the
//!   volume flow per unit total area, the porosity does not enter the transport terms). The
//!   Darcy-Forchheimer sink per unit mass is `S = -(nu/K + C2 |u| / 2) u` with permeability `K`
//!   (m^2) and inertial resistance `C2` (1/m). The Ergun equation (Ergun 1952, Chem. Eng. Prog.
//!   48(2) 89-94; in the form `dp/L = 150 mu (1-e)^2 U / (e^3 d^2) + 1.75 rho (1-e) U^2 / (e^3 d)`
//!   of the textbooks) maps onto it with `1/K = 150 (1-e)^2 / (e^3 d^2)` and
//!   `C2 = 3.5 (1-e) / (e^3 d)`. The sink is linearised: `|u|` comes from the previous iterate
//!   and the whole sink is added to the momentum diagonal (implicit).
//! * A rotating frame (angular velocity `Omega` about an axis through `origin`) adds the
//!   relative-velocity-formulation accelerations `-2 Omega x u - Omega x (Omega x r)`. The
//!   centrifugal part is evaluated analytically at cell and face centres (so it balances a
//!   pressure field exactly at walls), the Coriolis part from the previous iterate, with boundary
//!   faces using the boundary velocity.
//!
//! A cell zone is a boolean per cell. A face belongs to the zone when both of its cells do (the
//! owner for a boundary face), so the force and the sink are consistent at zone interfaces.

use crate::geometry::FvGeometry;
use crate::simple::BodyForce;
use rcfd_core::vec3::{self, Vec3};

/// Darcy-Forchheimer coefficients `(1/K, C2)` of the Ergun equation for a packed bed of
/// particles of diameter `d` (m) and void fraction `porosity`.
pub fn ergun_coefficients(porosity: f64, d: f64) -> (f64, f64) {
    let e3 = porosity.powi(3);
    let s = 1.0 - porosity;
    (150.0 * s * s / (e3 * d * d), 3.5 * s / (e3 * d))
}

/// Pressure gradient magnitude (Pa/m) of the Ergun equation for superficial velocity `u`.
pub fn ergun_gradient(porosity: f64, d: f64, mu: f64, rho: f64, u: f64) -> f64 {
    let (inv_k, c2) = ergun_coefficients(porosity, d);
    mu * inv_k * u + 0.5 * rho * c2 * u * u
}

/// Face membership of a cell zone: both cells inside (owner for boundary faces).
pub fn face_mask(geom: &FvGeometry, cells: &[bool]) -> Vec<bool> {
    let mesh = &geom.mesh;
    (0..mesh.n_faces())
        .map(|f| {
            if f < geom.n_internal {
                cells[mesh.owner()[f]] && cells[mesh.neighbour()[f]]
            } else {
                cells[mesh.owner()[f]]
            }
        })
        .collect()
}

/// Implicit sink `nu/K + C2 |u| / 2` (1/s) in the cells of the zone, zero elsewhere.
pub fn porous_sink(nu: f64, inv_k: f64, c2: f64, u: &[Vec3], zone: &[bool], out: &mut [f64]) {
    for c in 0..u.len() {
        if zone[c] {
            out[c] += nu * inv_k + 0.5 * c2 * vec3::norm(u[c]);
        }
    }
}

/// Rotating-frame settings.
#[derive(Debug, Clone, Copy)]
pub struct RotatingFrame {
    /// A point on the rotation axis.
    pub origin: Vec3,
    /// Unit axis direction.
    pub axis: Vec3,
    /// Angular velocity (rad/s, right-handed about `axis`).
    pub omega: f64,
    /// Include the centrifugal acceleration (switch off only to isolate the Coriolis force).
    pub centrifugal: bool,
}

impl RotatingFrame {
    fn accel(&self, r: Vec3, u: Vec3) -> Vec3 {
        let w = vec3::scale(self.axis, self.omega);
        let rel = vec3::sub(r, self.origin);
        let perp = vec3::sub(rel, vec3::scale(self.axis, vec3::dot(rel, self.axis)));
        let coriolis = vec3::scale(vec3::cross(w, u), -2.0);
        let centrifugal = vec3::scale(perp, if self.centrifugal { self.omega * self.omega } else { 0.0 });
        vec3::add(coriolis, centrifugal)
    }

    /// Acceleration field as a [`BodyForce`] added to `out` in the zone. `u` are the cell
    /// velocities and `ub` the boundary-face velocities (index `f - n_internal`).
    pub fn add_force(&self, geom: &FvGeometry, u: &[Vec3], ub: &[Vec3], zone: &[bool], out: &mut BodyForce) {
        let mesh = &geom.mesh;
        let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
        let fz = face_mask(geom, zone);
        for c in 0..u.len() {
            if zone[c] {
                out.cell[c] = vec3::add(out.cell[c], self.accel(cc[c], u[c]));
            }
        }
        for f in 0..mesh.n_faces() {
            if !fz[f] || geom.empty[f] {
                continue;
            }
            let uf = if f < geom.n_internal {
                let w = geom.weights[f];
                vec3::add(
                    vec3::scale(u[mesh.owner()[f]], w),
                    vec3::scale(u[mesh.neighbour()[f]], 1.0 - w),
                )
            } else {
                ub[f - geom.n_internal]
            };
            out.face[f] += vec3::dot(self.accel(fc[f], uf), sf[f]);
        }
    }
}

/// Multiple-reference-frame (frozen rotor) zone in the absolute-velocity formulation (ADR 0008): the zone rotates with
/// `omega` about the axis through `origin`; the solved velocity stays absolute, momentum is convected with the relative flux
/// `phi - (Omega x r) . S` and the lagged acceleration `-Omega x u` is added.
#[derive(Debug, Clone, Copy)]
pub struct MrfZone {
    /// A point on the rotation axis.
    pub origin: Vec3,
    /// Unit axis direction.
    pub axis: Vec3,
    /// Angular velocity (rad/s, right-handed about `axis`).
    pub omega: f64,
}

impl MrfZone {
    /// Frame velocity `Omega x (r - origin)`.
    pub fn frame_velocity(&self, r: Vec3) -> Vec3 {
        vec3::cross(vec3::scale(self.axis, self.omega), vec3::sub(r, self.origin))
    }

    /// Frame flux `(Omega x r) . S` at every face whose two cells are in the zone (boundary and interface faces: 0),
    /// added to `offset`.
    pub fn add_frame_flux(&self, geom: &FvGeometry, zone: &[bool], offset: &mut [f64]) {
        let mesh = &geom.mesh;
        let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
        for f in 0..geom.n_internal {
            if zone[mesh.owner()[f]] && zone[mesh.neighbour()[f]] && !geom.empty[f] {
                offset[f] += vec3::dot(self.frame_velocity(fc[f]), sf[f]);
            }
        }
    }

    /// Worst relative value of `|(Omega x r) . S| / (|Omega| |r_perp| |S|)` over the faces of the zone cells that do not
    /// receive the frame flux (zone boundary faces, interface faces): zero for a surface of revolution about the axis.
    pub fn worst_non_axisymmetry(&self, geom: &FvGeometry, zone: &[bool]) -> f64 {
        let mesh = &geom.mesh;
        let (fc, sf) = (mesh.face_centres(), mesh.face_area_vectors());
        let mut worst: f64 = 0.0;
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            let (o, nb) = (
                mesh.owner()[f],
                if f < geom.n_internal {
                    Some(mesh.neighbour()[f])
                } else {
                    None
                },
            );
            let in_o = zone[o];
            let in_n = nb.is_some_and(|n| zone[n]);
            if !(in_o || in_n) || (in_o && in_n) {
                continue;
            }
            let rel = vec3::sub(fc[f], self.origin);
            let perp = vec3::sub(rel, vec3::scale(self.axis, vec3::dot(rel, self.axis)));
            let denom = self.omega.abs() * vec3::norm(perp) * vec3::norm(sf[f]);
            if denom > 0.0 {
                worst = worst.max(vec3::dot(self.frame_velocity(fc[f]), sf[f]).abs() / denom);
            }
        }
        worst
    }

    /// Add the acceleration `-Omega x u` in the zone: cell values from `u`, face values `a_f . S_f` on every face that
    /// touches a zone cell (internal faces from the linearly interpolated acceleration of the cells, zero for cells
    /// outside the zone; boundary faces from the boundary velocity `ub`, index `f - n_internal`).
    pub fn add_force(&self, geom: &FvGeometry, u: &[Vec3], ub: &[Vec3], zone: &[bool], out: &mut BodyForce) {
        let mesh = &geom.mesh;
        let sf = mesh.face_area_vectors();
        let w = vec3::scale(self.axis, self.omega);
        let accel = |v: Vec3| vec3::scale(vec3::cross(w, v), -1.0);
        for c in 0..u.len() {
            if zone[c] {
                out.cell[c] = vec3::add(out.cell[c], accel(u[c]));
            }
        }
        for f in 0..mesh.n_faces() {
            if geom.empty[f] {
                continue;
            }
            let o = mesh.owner()[f];
            let a = if f < geom.n_internal {
                let n = mesh.neighbour()[f];
                if !(zone[o] || zone[n]) {
                    continue;
                }
                let wt = geom.weights[f];
                let ao = if zone[o] { accel(u[o]) } else { [0.0; 3] };
                let an = if zone[n] { accel(u[n]) } else { [0.0; 3] };
                vec3::add(vec3::scale(ao, wt), vec3::scale(an, 1.0 - wt))
            } else if zone[o] {
                accel(ub[f - geom.n_internal])
            } else {
                continue;
            };
            out.face[f] += vec3::dot(a, sf[f]);
        }
    }
}

/// Constant acceleration `a` in the zone, added to `out`.
pub fn add_constant_acceleration(geom: &FvGeometry, a: Vec3, zone: &[bool], out: &mut BodyForce) {
    let mesh = &geom.mesh;
    let sf = mesh.face_area_vectors();
    let fz = face_mask(geom, zone);
    for c in 0..zone.len() {
        if zone[c] {
            out.cell[c] = vec3::add(out.cell[c], a);
        }
    }
    for f in 0..mesh.n_faces() {
        if fz[f] {
            out.face[f] += vec3::dot(a, sf[f]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ergun_coefficients_reproduce_the_ergun_gradient() {
        let (e, d, mu, rho, u) = (0.4, 0.005, 1.8e-5, 1.2, 0.1);
        let (inv_k, c2) = ergun_coefficients(e, d);
        // closed form of the two terms
        let visc = 150.0 * mu * (1.0 - e).powi(2) * u / (e.powi(3) * d * d);
        let inert = 1.75 * rho * (1.0 - e) * u * u / (e.powi(3) * d);
        assert!((mu * inv_k * u - visc).abs() < 1e-12 * visc);
        assert!((0.5 * rho * c2 * u * u - inert).abs() < 1e-12 * inert);
        assert!((ergun_gradient(e, d, mu, rho, u) - visc - inert).abs() < 1e-9);
    }
}
