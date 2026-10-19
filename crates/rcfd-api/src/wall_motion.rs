//! Wall motion and shear boundary conditions (F-204, ADR 0016): `moving_wall`, `shear`, `partial_slip` and
//! `marangoni` kinds of `boundaries.<patch>.velocity`. All are impermeable and count as walls for the turbulence model.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_s, get_v};
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{Bc, FixedValue, Profile, SlipLaw, SlipWall};
use rcfd_fv::FvGeometry;

/// The velocity kinds of this module.
pub const KINDS: [&str; 4] = ["moving_wall", "shear", "partial_slip", "marangoni"];

/// True when the velocity kind of `boundaries.<patch>` is one of [`KINDS`].
pub fn is_wall_motion(case: &Case, patch: &str) -> bool {
    KINDS.contains(&get_s(case, &format!("boundaries.{patch}.velocity")).as_str())
}

fn diag(path: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }]
}

/// Wall velocity of every face of `patch`: translation plus rotation about the axis.
fn wall_velocity(case: &Case, geom: &FvGeometry, patch: &str, b: &str) -> Result<Vec<Vec3>, Vec<Diagnostic>> {
    let u_t = get_v(case, &format!("{b}.velocity_value"), [0.0; 3]);
    let rate = get_f(case, &format!("{b}.wall_rotation_rate"));
    let axis = get_v(case, &format!("{b}.wall_rotation_axis"), [0.0, 0.0, 1.0]);
    let origin = get_v(case, &format!("{b}.wall_rotation_origin"), [0.0; 3]);
    if rate != 0.0 && vec3::norm(axis) == 0.0 {
        return Err(diag(
            &format!("{b}.wall_rotation_axis"),
            "the rotation axis must have non-zero length",
        ));
    }
    let omega = if rate != 0.0 {
        vec3::scale(axis, rate / vec3::norm(axis))
    } else {
        [0.0; 3]
    };
    let mesh = &geom.mesh;
    let p = mesh.patch(patch).ok_or_else(|| diag(b, "no such patch"))?;
    let mut out = Vec::new();
    for f in p.faces() {
        let r = vec3::sub(mesh.face_centres()[f], origin);
        let mut u = vec3::cross(omega, r);
        vec3::axpy(&mut u, 1.0, u_t);
        out.push(u);
    }
    Ok(out)
}

/// Maxwell slip length from a specularity and a mean free path: `l = lambda (1 + s)/(1 - s)`.
pub fn maxwell_slip_length(specularity: f64, mean_free_path: f64) -> Option<f64> {
    (0.0..1.0)
        .contains(&specularity)
        .then(|| mean_free_path * (1.0 + specularity) / (1.0 - specularity))
}

/// Boundary condition of one `moving_wall`, `shear`, `partial_slip` or `marangoni` entry. `mu` is the dynamic
/// viscosity `rho nu` used to turn a traction into a velocity gradient.
pub fn build(case: &Case, geom: &FvGeometry, patch: &str, b: &str, mu: f64) -> Result<Box<dyn Bc>, Vec<Diagnostic>> {
    let kind = get_s(case, &format!("{b}.velocity"));
    let mut uw = wall_velocity(case, geom, patch, b)?;
    let faces: Vec<usize> = geom.mesh.patch(patch).map(|p| p.faces().collect()).unwrap_or_default();
    let projection = get_s(case, &format!("{b}.wall_velocity_projection"));
    if projection == "tangential" {
        if kind != "moving_wall" {
            return Err(diag(
                &format!("{b}.wall_velocity_projection"),
                "the tangential projection is available for velocity = \"moving_wall\" only",
            ));
        }
        // ADR 0072: a wall of revolution moving in itself, represented by facets whose normals are not exactly perpendicular
        // to its velocity; the tangential part of the velocity keeps the faceted wall impermeable (zero face flux)
        let sf: Vec<Vec3> = faces.iter().map(|&f| geom.mesh.face_area_vectors()[f]).collect();
        uw = rcfd_fv::bc::tangential_projection(&uw, &sf).0;
    }
    // the wall velocity must be tangential on every face (impermeable)
    if matches!(kind.as_str(), "moving_wall" | "partial_slip") {
        for (u, &f) in uw.iter().zip(&faces) {
            let n = geom.normal(f);
            if vec3::dot(*u, n).abs() > 1e-9 * vec3::norm(*u).max(1e-30) {
                return Err(diag(
                    &format!("{b}.velocity_value"),
                    "the wall velocity must be tangential to the wall (the boundary is impermeable); use a fixed velocity for an inflow",
                ));
            }
        }
    }
    match kind.as_str() {
        "moving_wall" => Ok(Box::new(FixedValue(Profile::PerFace(uw)))),
        "shear" => {
            if mu <= 0.0 {
                return Err(diag(
                    &format!("{b}.shear_stress"),
                    "a shear stress needs a positive physics.rho and physics.nu",
                ));
            }
            let tau = get_v(case, &format!("{b}.shear_stress"), [0.0; 3]);
            Ok(Box::new(SlipWall {
                law: SlipLaw::Shear,
                wall_velocity: Profile::Uniform([0.0; 3]),
                stress: Profile::Uniform(vec3::scale(tau, 1.0 / mu)),
            }))
        }
        "partial_slip" => {
            let (l0, s, lambda) = (
                get_f(case, &format!("{b}.slip_length")),
                get_f(case, &format!("{b}.specularity")),
                get_f(case, &format!("{b}.mean_free_path")),
            );
            let l = if s >= 0.0 {
                if l0 > 0.0 {
                    return Err(diag(
                        &format!("{b}.slip_length"),
                        "give either slip_length or specularity with mean_free_path, not both",
                    ));
                }
                maxwell_slip_length(s, lambda).ok_or_else(|| {
                    diag(
                        &format!("{b}.specularity"),
                        "the specularity must lie in [0, 1); a fully specular wall (1) is a symmetry plane, use velocity = \"symmetry\"",
                    )
                })?
            } else {
                l0
            };
            Ok(Box::new(SlipWall {
                law: SlipLaw::Navier(l),
                wall_velocity: Profile::PerFace(uw),
                stress: Profile::Uniform([0.0; 3]),
            }))
        }
        "marangoni" => {
            if !get_b(case, "energy.enabled") {
                return Err(diag(
                    &format!("{b}.velocity"),
                    "marangoni needs the energy equation (energy.enabled = true)",
                ));
            }
            if mu <= 0.0 {
                return Err(diag(
                    &format!("{b}.marangoni_coefficient"),
                    "a thermocapillary stress needs a positive physics.rho and physics.nu",
                ));
            }
            // the stress follows the temperature field; it starts at zero and is refreshed every iteration or step
            Ok(Box::new(SlipWall {
                law: SlipLaw::Shear,
                wall_velocity: Profile::Uniform([0.0; 3]),
                stress: Profile::Uniform([0.0; 3]),
            }))
        }
        other => Err(diag(
            &format!("{b}.velocity"),
            format!("'{other}' is not a wall-motion kind"),
        )),
    }
}

/// Kinematic thermocapillary stress `(d sigma/d T) grad T / mu` per face of `patch` from cell temperature gradients;
/// the boundary condition projects it onto the face tangent plane.
pub fn marangoni_stress(geom: &FvGeometry, patch: &str, grad_t: &[Vec3], coefficient: f64, mu: f64) -> Profile {
    let own = geom.mesh.owner();
    let faces = geom
        .mesh
        .patch(patch)
        .map(|p| p.faces().collect::<Vec<_>>())
        .unwrap_or_default();
    Profile::PerFace(
        faces
            .iter()
            .map(|&f| vec3::scale(grad_t[own[f]], coefficient / mu))
            .collect(),
    )
}

/// The boundary condition carrying a refreshed thermocapillary stress.
pub fn marangoni_bc(stress: Profile) -> Box<dyn Bc> {
    Box::new(SlipWall {
        law: SlipLaw::Shear,
        wall_velocity: Profile::Uniform([0.0; 3]),
        stress,
    })
}
