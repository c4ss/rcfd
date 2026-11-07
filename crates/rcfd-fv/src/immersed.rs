//! Immersed (non-conforming) solid bodies with modified forcing (F-219, ADR 0218).
//!
//! A body is an analytic shape on a mesh that does not conform to it. The forcing is an implicit momentum term
//! `-(coeff u - source)` per unit mass, added by [`crate::simple::SimpleSolver`] and [`crate::pimple::PimpleSolver`] to the
//! momentum diagonal and right-hand side when their `immersed` field is set:
//!
//! * a **solid cell** (its centre inside a body) gets `coeff = K`, `source = K u_b(x_c)`: the volume penalisation of the cell
//!   velocity to the rigid solid velocity `u_b(x) = velocity + omega x (x - origin)` with the penalty `K` (1/s), the implicit
//!   form of a direct forcing for a vanishing penalty time;
//! * with `modified`, the **first fluid cells** get, for every internal face `f` to a solid cell `s`, the change of the face's
//!   Laplacian term that imposes the solid velocity at the true crossing of the surface on the segment from the fluid centre to
//!   the solid centre (fraction `theta`) instead of at the solid centre: `coeff += k_f (1/theta - 1)/V_c`,
//!   `source += k_f (u_b(x_G)/theta - u_b(x_s))/V_c`, `k_f = gamma_f lap_coeff_f` (ADR 0218 D2). On an orthogonal mesh this is
//!   the linear extrapolation of a Dirichlet value to the crossing along a grid line.
//!
//! The force of the fluid on a body is the reaction of its forcing, `F = rho sum V (coeff u - source)` over the body's terms
//! ([`ImmersedForcing::force_moment`]). The shapes do not move; the forcing is built once for a mesh.

use crate::geometry::FvGeometry;
use rcfd_core::vec3::{self, Vec3};

/// Analytic shape of an immersed body; `signed_distance` is negative inside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolidShape {
    /// Infinite circular cylinder (a circle on a 2-D mesh whose axis is the empty direction).
    Cylinder {
        /// A point on the axis.
        centre: Vec3,
        /// Axis direction (normalised by the shape).
        axis: Vec3,
        /// Radius.
        radius: f64,
    },
    /// Sphere.
    Sphere {
        /// Centre.
        centre: Vec3,
        /// Radius.
        radius: f64,
    },
    /// Axis-aligned box.
    Cuboid {
        /// Lower corner.
        lo: Vec3,
        /// Upper corner.
        hi: Vec3,
    },
}

impl SolidShape {
    /// Signed distance to the surface (negative inside).
    pub fn signed_distance(&self, x: Vec3) -> f64 {
        match *self {
            SolidShape::Cylinder { centre, axis, radius } => {
                let a = vec3::unit(axis);
                let r = vec3::sub(x, centre);
                let perp = vec3::sub(r, vec3::scale(a, vec3::dot(r, a)));
                vec3::norm(perp) - radius
            }
            SolidShape::Sphere { centre, radius } => vec3::norm(vec3::sub(x, centre)) - radius,
            SolidShape::Cuboid { lo, hi } => {
                let q: Vec3 = [0, 1, 2].map(|k| (x[k] - 0.5 * (lo[k] + hi[k])).abs() - 0.5 * (hi[k] - lo[k]));
                let outside = vec3::norm(q.map(|v| v.max(0.0)));
                let inside = q[0].max(q[1]).max(q[2]).min(0.0);
                outside + inside
            }
        }
    }

    fn check(&self) -> Result<(), String> {
        let finite = |v: Vec3| v.iter().all(|x| x.is_finite());
        match *self {
            SolidShape::Cylinder { centre, axis, radius } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return Err(format!("cylinder radius must be positive (got {radius})"));
                }
                if !finite(centre) || !finite(axis) || vec3::norm(axis) <= 0.0 {
                    return Err("cylinder centre must be finite and its axis non-zero".into());
                }
            }
            SolidShape::Sphere { centre, radius } => {
                if !(radius.is_finite() && radius > 0.0) {
                    return Err(format!("sphere radius must be positive (got {radius})"));
                }
                if !finite(centre) {
                    return Err("sphere centre must be finite".into());
                }
            }
            SolidShape::Cuboid { lo, hi } => {
                if !finite(lo) || !finite(hi) || (0..3).any(|k| hi[k] <= lo[k]) {
                    return Err(
                        "cuboid must have finite corners with hi > lo in every direction (empty cuboid)".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

/// One immersed body: a shape (solid inside, or outside when `inverted`) and its rigid velocity
/// `u_b(x) = velocity + omega x (x - origin)`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImmersedBody {
    /// Name used in messages.
    pub name: String,
    /// Shape.
    pub shape: SolidShape,
    /// The solid is the outside of the shape.
    pub inverted: bool,
    /// Translational velocity of the solid.
    pub velocity: Vec3,
    /// Angular velocity of the solid (rad/s, vector).
    pub omega: Vec3,
    /// Point of the rotation axis.
    pub origin: Vec3,
}

impl ImmersedBody {
    /// A stationary body.
    pub fn fixed(name: &str, shape: SolidShape) -> Self {
        ImmersedBody {
            name: name.to_string(),
            shape,
            inverted: false,
            velocity: vec3::ZERO,
            omega: vec3::ZERO,
            origin: vec3::ZERO,
        }
    }

    /// Level set: negative in the solid, positive in the fluid.
    pub fn level(&self, x: Vec3) -> f64 {
        let d = self.shape.signed_distance(x);
        if self.inverted {
            -d
        } else {
            d
        }
    }

    /// Velocity of the solid at `x`.
    pub fn velocity_at(&self, x: Vec3) -> Vec3 {
        vec3::add(self.velocity, vec3::cross(self.omega, vec3::sub(x, self.origin)))
    }

    /// Fraction `theta` in `[0, 1]` of the segment `a -> b` (`a` in the fluid, `b` in the solid) at which the level set changes
    /// sign, by bisection to round-off.
    pub fn crossing(&self, a: Vec3, b: Vec3) -> f64 {
        let d = vec3::sub(b, a);
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        if self.level(a) <= 0.0 {
            return 0.0;
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if mid <= lo || mid >= hi {
                break;
            }
            if self.level(vec3::add(a, vec3::scale(d, mid))) < 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        0.5 * (lo + hi)
    }
}

/// Settings of the forcing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImmersedSettings {
    /// Penalty `K` of the solid cells (1/s).
    pub penalty: f64,
    /// Modify the forcing in the first fluid cells (no-slip at the true crossing); `false` gives the plain staircase forcing.
    pub modified: bool,
    /// Lower bound of the crossing fraction `theta` used in the modification.
    pub theta_min: f64,
}

impl ImmersedSettings {
    /// Modified forcing with penalty `penalty` and `theta_min = 1e-2` (ADR 0218 D1).
    pub fn new(penalty: f64) -> Self {
        ImmersedSettings {
            penalty,
            modified: true,
            theta_min: 1e-2,
        }
    }
}

/// One term of the forcing of a body: the momentum term `-(coeff u_c - source)` per unit mass in `cell`, applied at `point`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForcingTerm {
    /// Cell.
    pub cell: usize,
    /// Coefficient (1/s).
    pub coeff: f64,
    /// Source (m/s^2).
    pub source: Vec3,
    /// Point of application of the reaction (the cell centre of a solid cell, the crossing of a modification).
    pub point: Vec3,
    /// The internal face of a modification term (`None` for a solid-cell term).
    pub face: Option<usize>,
    /// Theta of a modification term (the crossing fraction before the lower bound; 1 for a solid-cell term).
    pub theta: f64,
}

/// The forcing of one body.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyForcing {
    /// Name of the body.
    pub name: String,
    /// Its solid cells.
    pub solid_cells: Vec<usize>,
    /// Its terms (solid cells first, then the modification terms).
    pub terms: Vec<ForcingTerm>,
}

/// The forcing of all immersed bodies of a mesh (ADR 0218 D1, D2).
#[derive(Debug, Clone, PartialEq)]
pub struct ImmersedForcing {
    /// Coefficient per cell (1/s), the sum of the terms of the cell.
    pub coeff: Vec<f64>,
    /// Source per cell (m/s^2), the sum of the terms of the cell.
    pub source: Vec<Vec3>,
    /// The body whose solid holds the cell centre.
    pub body_of_cell: Vec<Option<usize>>,
    /// Per body, its terms.
    pub bodies: Vec<BodyForcing>,
    /// Cell volumes of the mesh the forcing was built for.
    volumes: Vec<f64>,
}

impl ImmersedForcing {
    /// Build the forcing of `bodies` on `geom` with the momentum face diffusivity `gamma` (one value per face, e.g.
    /// [`crate::diffusion::uniform_gamma`] of the kinematic viscosity). Fails, with a located message, for a penalty that is not
    /// positive, `theta_min` outside `(0, 1]`, an invalid shape, a cell centre inside two bodies, or a solid cell on a periodic
    /// face.
    pub fn build(
        geom: &FvGeometry,
        bodies: &[ImmersedBody],
        settings: ImmersedSettings,
        gamma: &[f64],
    ) -> Result<ImmersedForcing, String> {
        if !(settings.penalty.is_finite() && settings.penalty > 0.0) {
            return Err(format!(
                "immersed solid: penalty must be positive and finite (got {})",
                settings.penalty
            ));
        }
        if !(settings.theta_min > 0.0 && settings.theta_min <= 1.0) {
            return Err(format!(
                "immersed solid: theta_min must lie in (0, 1] (got {})",
                settings.theta_min
            ));
        }
        for b in bodies {
            b.shape
                .check()
                .map_err(|e| format!("immersed solid '{}': {e}", b.name))?;
        }
        let mesh = &geom.mesh;
        let n = geom.n_cells();
        if gamma.len() != mesh.n_faces() {
            return Err(format!(
                "immersed solid: gamma needs one value per face ({} given, {} faces)",
                gamma.len(),
                mesh.n_faces()
            ));
        }
        let cc = mesh.cell_centres();
        let vol = mesh.cell_volumes();
        let mut body_of_cell: Vec<Option<usize>> = vec![None; n];
        for c in 0..n {
            for (i, b) in bodies.iter().enumerate() {
                if b.level(cc[c]) < 0.0 {
                    if let Some(j) = body_of_cell[c] {
                        return Err(format!(
                            "immersed solid: cell {c} at ({:.4e}, {:.4e}, {:.4e}) is inside both '{}' and '{}'",
                            cc[c][0], cc[c][1], cc[c][2], bodies[j].name, b.name
                        ));
                    }
                    body_of_cell[c] = Some(i);
                }
            }
        }
        let n_int = geom.n_internal;
        for f in n_int..mesh.n_faces() {
            let c = mesh.owner()[f];
            if geom.coupled_face[f - n_int] {
                if let Some(i) = body_of_cell[c] {
                    return Err(format!(
                        "immersed solid '{}': solid cell {c} has a periodic boundary face {f} (a body must not touch a periodic patch)",
                        bodies[i].name
                    ));
                }
            }
        }
        let k = settings.penalty;
        let mut out: Vec<BodyForcing> = bodies
            .iter()
            .map(|b| BodyForcing {
                name: b.name.clone(),
                solid_cells: Vec::new(),
                terms: Vec::new(),
            })
            .collect();
        for c in 0..n {
            if let Some(i) = body_of_cell[c] {
                out[i].solid_cells.push(c);
                out[i].terms.push(ForcingTerm {
                    cell: c,
                    coeff: k,
                    source: vec3::scale(bodies[i].velocity_at(cc[c]), k),
                    point: cc[c],
                    face: None,
                    theta: 1.0,
                });
            }
        }
        if settings.modified {
            for f in 0..n_int {
                if geom.empty[f] {
                    continue;
                }
                let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
                let (c, s, i) = match (body_of_cell[o], body_of_cell[nb]) {
                    (None, Some(i)) => (o, nb, i),
                    (Some(i), None) => (nb, o, i),
                    _ => continue,
                };
                let b = &bodies[i];
                let theta = b.crossing(cc[c], cc[s]);
                let te = theta.max(settings.theta_min);
                let xg = vec3::add(cc[c], vec3::scale(vec3::sub(cc[s], cc[c]), theta));
                let kf = gamma[f] * geom.lap_coeff[f];
                let ug = b.velocity_at(xg);
                let us = b.velocity_at(cc[s]);
                let src = [0, 1, 2].map(|d| kf * (ug[d] / te - us[d]) / vol[c]);
                out[i].terms.push(ForcingTerm {
                    cell: c,
                    coeff: kf * (1.0 / te - 1.0) / vol[c],
                    source: src,
                    point: xg,
                    face: Some(f),
                    theta,
                });
            }
        }
        let mut coeff = vec![0.0; n];
        let mut source = vec![vec3::ZERO; n];
        for b in &out {
            for t in &b.terms {
                coeff[t.cell] += t.coeff;
                source[t.cell] = vec3::add(source[t.cell], t.source);
            }
        }
        Ok(ImmersedForcing {
            coeff,
            source,
            body_of_cell,
            bodies: out,
            volumes: vol.to_vec(),
        })
    }

    /// Number of cells of the mesh the forcing was built for.
    pub fn n_cells(&self) -> usize {
        self.coeff.len()
    }

    /// True for the solid cells (centre inside some body).
    pub fn solid_mask(&self) -> Vec<bool> {
        self.body_of_cell.iter().map(|b| b.is_some()).collect()
    }

    /// Force and moment about `x_ref` of the fluid on body `body` for the cell velocities `u`:
    /// `F = rho sum_t V_c (coeff_t u_c - source_t)`, `M = sum_t (x_t - x_ref) x dF_t` (ADR 0218 D1). This is the reaction of the
    /// whole forcing: for a body whose solid cells touch the domain boundary it also holds the force of the boundary on those cells
    /// (the boundary pressure and wall terms), so it is the fluid force only for a body that lies inside the domain (ADR 0218 Q5).
    pub fn force_moment(&self, body: usize, u: &[Vec3], rho: f64, x_ref: Vec3) -> (Vec3, Vec3) {
        let (mut force, mut moment) = (vec3::ZERO, vec3::ZERO);
        for t in &self.bodies[body].terms {
            let v = self.volumes[t.cell];
            let df: Vec3 = [0, 1, 2].map(|k| rho * v * (t.coeff * u[t.cell][k] - t.source[k]));
            force = vec3::add(force, df);
            moment = vec3::add(moment, vec3::cross(vec3::sub(t.point, x_ref), df));
        }
        (force, moment)
    }
}
