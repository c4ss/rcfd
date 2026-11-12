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
//!
//! **Second-order moment (F-337, ADR 0239; opt-in, [`ImmersedForcing::force_moment_second_order`]).** `force_moment` applies a
//! modification term at the surface crossing; the moment of the forcing is the angular-momentum balance of the cells, and with
//! it the moment of the fluid on the body, only when every term is applied at the centre of its cell. The difference is a
//! first-order error of the moment (not of the force).
//!
//! **Unsteady flow (F-346, ADR 0249; opt-in, [`ImmersedForcing::force_moment_unsteady`]).** The reaction of the forcing is the
//! force on the staircase surface of the solid cells minus the rate of change of the momentum those cells hold. For a body at
//! rest that momentum is zero and the reaction is the force; for a body whose rigid velocity changes in time the solid cells
//! hold the momentum of a fictitious fluid that fills the body, and its rate of change is added back. The first fluid cells
//! hold fluid momentum and do not enter.
//!
//! **Second-order treatment (F-309, ADR 0234; opt-in, [`ImmersedForcing::build_second_order`]).** The forcing above closes the
//! viscous flux of a first fluid cell with the extension of the fluid profile through the surface, but leaves the mass flux of
//! the face to the solid cell to the continuity of that solid cell, which pins it to the rigid velocity: a face velocity error of
//! first order. With the flux faces set, the solvers replace the interpolated `HbyA` flux of every fluid-solid face by the flux of
//! the same extension, `(HbyA_c + (t_f/theta)(u_b(x_G) - HbyA_c)) . S_f`, make the net flux of each body equal to that of its
//! rigid velocity, and give the solid cells the volume source that releases their continuity
//! ([`ImmersedForcing::reconstruct_face_flux`]).
//!
//! **Bounded weights (F-355, ADR 0254; opt-in, [`ImmersedForcing::build_second_order_bounded`]).** The extension above
//! multiplies the velocity of the fluid cell relative to the wall by `t_f/theta - 1`, which is large when the crossing lies
//! close to the fluid centre. That is harmless while that velocity is of order `theta` (a resolved profile) and destroys a
//! transient run when it is not (an impulsive start, a layer thinner than the cell: DFG 2D-2 stopped at step 6). The bounded
//! form takes, on the faces with `2 theta < t_f`, the wall value reflected through the value at the image point of the face
//! in the fluid, `2 u_b(x_G) - ((1 - a) HbyA_c + a HbyA_d)` with `d` the fluid cell behind `c`: second order, with weights of
//! magnitude at most 1. [`crate::pimple::PimpleSolver`] refuses the unbounded form where it can amplify
//! ([`ImmersedForcing::flux_growth`]).

use crate::geometry::FvGeometry;
use rcfd_core::vec3::{self, Vec3};

/// The rate of change of the cell velocity at the new time level as the transient solvers difference it
/// ([`crate::ddt::add_ddt`]; F-346, ADR 0249 D2): `(u - u_old)/dt` without `older` (implicit Euler, and the first step of BDF2),
/// and with `older = Some((u_older, dt_old))` the variable-step BDF2 difference `(c0 u - c1 u_old + c2 u_older)/dt`,
/// `r = dt/dt_old`, `c0 = (1 + 2r)/(1 + r)`, `c1 = 1 + r`, `c2 = r^2/(1 + r)`. Crank-Nicolson and the bounded BDF2 are not
/// covered.
///
/// # Panics
/// When the fields differ in length.
pub fn velocity_rate(u: &[Vec3], u_old: &[Vec3], older: Option<(&[Vec3], f64)>, dt: f64) -> Vec<Vec3> {
    assert_eq!(u.len(), u_old.len(), "velocity_rate: u and u_old differ in length");
    match older {
        None => u
            .iter()
            .zip(u_old)
            .map(|(a, b)| [0, 1, 2].map(|k| (a[k] - b[k]) / dt))
            .collect(),
        Some((u_older, dt_old)) => {
            assert_eq!(u.len(), u_older.len(), "velocity_rate: u and u_older differ in length");
            let r = dt / dt_old;
            let (c0, c1, c2) = ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r));
            (0..u.len())
                .map(|i| [0, 1, 2].map(|k| (c0 * u[i][k] - c1 * u_old[i][k] + c2 * u_older[i][k]) / dt))
                .collect()
        }
    }
}

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

/// A face between a fluid cell and a solid cell whose flux is reconstructed at the surface crossing (ADR 0234 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluxFace {
    /// The internal face.
    pub face: usize,
    /// Its fluid cell.
    pub fluid: usize,
    /// Its solid cell.
    pub solid: usize,
    /// The body of the solid cell.
    pub body: usize,
    /// `+1` when the fluid cell is the owner (the face area vector points into the solid), else `-1`.
    pub sign: f64,
    /// `t_f/theta_e`: the fraction of the face on the segment from the fluid centre to the solid centre over the bounded
    /// crossing fraction.
    pub weight: f64,
    /// The solid velocity at the crossing.
    pub wall: Vec3,
    /// The flux of the rigid solid velocity at the face centre, `u_b(x_f) . S_f`.
    pub rigid: f64,
    /// The face area vector.
    pub sf: Vec3,
    /// Its norm.
    pub area: f64,
    /// The image form of ADR 0254 (`None`: the extension of ADR 0234 D1 with `weight`): the fluid cell `d` behind the fluid
    /// cell on the line to the solid centre and the fraction `a` of the way to it at which the image point of the face lies;
    /// the flux velocity is `2 wall - ((1 - a) HbyA_fluid + a HbyA_d)`. `(fluid, 0.0)` when there is no such cell.
    pub image: Option<(usize, f64)>,
}

/// How the flux faces of a forcing are built.
#[derive(Debug, Clone, Copy, PartialEq)]
enum FluxMode {
    /// No flux faces ([`ImmersedForcing::build`]).
    Off,
    /// ADR 0234 D1 on every face ([`ImmersedForcing::build_second_order`]).
    Linear,
    /// ADR 0254: the image form on the faces with a weight above 2 ([`ImmersedForcing::build_second_order_bounded`]).
    Bounded,
}

/// The weight of a flux face without an image above which [`ImmersedForcing::flux_growth`] looks at it (ADR 0254 E2: weights
/// up to 5 were measured to survive an impulsive start).
pub const FLUX_WEIGHT_LIMIT: f64 = 5.0;

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
    /// The fluid-solid faces whose flux is reconstructed (ADR 0234); empty for the forcing of [`ImmersedForcing::build`].
    pub flux_faces: Vec<FluxFace>,
    /// Whether [`ImmersedForcing::flux_growth`] reports (ADR 0254 E2; `true` from every constructor). `false` lets the transient
    /// solver run the unbounded flux faces of [`ImmersedForcing::build_second_order`] wherever they lead.
    pub flux_weight_check: bool,
    /// Cell volumes of the mesh the forcing was built for.
    volumes: Vec<f64>,
    /// Cell centres of the mesh the forcing was built for (ADR 0239).
    centres: Vec<Vec3>,
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
        Self::build_impl(geom, bodies, settings, gamma, FluxMode::Off)
    }

    /// [`ImmersedForcing::build`] (the same coefficients and sources) with the fluid-solid faces listed in `flux_faces`, so that
    /// the solvers reconstruct their flux at the surface crossing and release the continuity of the solid cells (F-309, ADR 0234
    /// D1 to D5). Needs `settings.modified`: the flux of the extension without the viscous flux of the extension is inconsistent.
    pub fn build_second_order(
        geom: &FvGeometry,
        bodies: &[ImmersedBody],
        settings: ImmersedSettings,
        gamma: &[f64],
    ) -> Result<ImmersedForcing, String> {
        if !settings.modified {
            return Err(
                "immersed solid: the second-order face flux (ADR 0234) needs the modified forcing (settings.modified = true)"
                    .into(),
            );
        }
        Self::build_impl(geom, bodies, settings, gamma, FluxMode::Linear)
    }

    /// [`ImmersedForcing::build_second_order`] with bounded weights (F-355, ADR 0254 E1): on the fluid-solid faces whose
    /// crossing fraction is below half the face fraction (`2 theta_e < t_f`, a weight above 2) the flux is that of the wall
    /// value reflected through the image point of the face in the fluid, interpolated between the fluid cell and the fluid cell
    /// behind it, instead of the linear extension through the crossing. The other faces, the coefficients and the sources are
    /// those of `build_second_order`. This is the form for transient runs. Needs `settings.modified`.
    pub fn build_second_order_bounded(
        geom: &FvGeometry,
        bodies: &[ImmersedBody],
        settings: ImmersedSettings,
        gamma: &[f64],
    ) -> Result<ImmersedForcing, String> {
        if !settings.modified {
            return Err(
                "immersed solid: the second-order face flux (ADR 0234, ADR 0254) needs the modified forcing \
                 (settings.modified = true)"
                    .into(),
            );
        }
        Self::build_impl(geom, bodies, settings, gamma, FluxMode::Bounded)
    }

    fn build_impl(
        geom: &FvGeometry,
        bodies: &[ImmersedBody],
        settings: ImmersedSettings,
        gamma: &[f64],
        mode: FluxMode,
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
        let mut flux_faces = Vec::new();
        if settings.modified {
            let (sf, cf) = (mesh.face_area_vectors(), mesh.face_centres());
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
                if mode != FluxMode::Off {
                    // ADR 0234 D1: the fraction of the face on the segment from the fluid centre to the solid centre
                    let t_f = if c == o { 1.0 - geom.weights[f] } else { geom.weights[f] };
                    // ADR 0254 E1: the image point of the face lies behind the fluid centre
                    let image = (mode == FluxMode::Bounded && 2.0 * te < t_f).then(|| {
                        let dir = vec3::sub(cc[s], cc[c]);
                        let len = vec3::norm(dir);
                        let mut best: Option<(usize, f64)> = None;
                        for &g in mesh.cell_faces(c) {
                            if g >= n_int || geom.empty[g] || g == f {
                                continue;
                            }
                            let d = if mesh.owner()[g] == c {
                                mesh.neighbour()[g]
                            } else {
                                mesh.owner()[g]
                            };
                            if body_of_cell[d].is_some() {
                                continue;
                            }
                            let e = vec3::sub(cc[d], cc[c]);
                            let cos = vec3::dot(e, dir) / (vec3::norm(e) * len);
                            if cos <= -0.99 && best.map_or(true, |(_, b)| cos < b) {
                                best = Some((d, cos));
                            }
                        }
                        match best {
                            Some((d, _)) => (
                                d,
                                ((t_f - 2.0 * te) * len / vec3::norm(vec3::sub(cc[d], cc[c]))).min(1.0),
                            ),
                            None => (c, 0.0),
                        }
                    });
                    flux_faces.push(FluxFace {
                        face: f,
                        fluid: c,
                        solid: s,
                        body: i,
                        sign: if c == o { 1.0 } else { -1.0 },
                        weight: t_f / te,
                        wall: ug,
                        rigid: vec3::dot(b.velocity_at(cf[f]), sf[f]),
                        sf: sf[f],
                        area: vec3::norm(sf[f]),
                        image,
                    });
                }
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
            flux_faces,
            flux_weight_check: true,
            volumes: vol.to_vec(),
            centres: cc.to_vec(),
        })
    }

    /// The face flux of the fluid-solid faces at the surface crossing and the volume source of the solid cells (F-309, ADR 0234
    /// D1 to D3). `hby_a` is the cell field the solver interpolates to the faces (`H/a_P`), `phi_hby_a` its face flux. Without
    /// flux faces (the forcing of [`ImmersedForcing::build`]) nothing is touched and `None` is returned. Otherwise, per flux face,
    /// `phi_hby_a = (HbyA_c + weight (u_G - HbyA_c)) . S_f` (the linear extension of the fluid profile through the no-slip value
    /// at the crossing, at the face); per body the net of these fluxes is made equal to the net rigid flux by a uniform normal
    /// velocity; and the returned cell field holds, for each solid cell, the net outflow `-sum sign (phi_hby_a - rigid)` to pass
    /// to the pressure equation as its volume source (`solve_pressure_parts_src`), zero elsewhere.
    ///
    /// # Panics
    /// With flux faces and `plain = false`: the caller's pressure path does not take the volume source (ADR 0234 D7).
    pub fn reconstruct_face_flux(&self, hby_a: &[Vec3], phi_hby_a: &mut [f64], plain: bool) -> Option<Vec<f64>> {
        if self.flux_faces.is_empty() {
            return None;
        }
        assert!(
            plain,
            "immersed solid: the second-order face flux (ADR 0234) is defined with the plain pressure equation only (no \
             gradient options, body-force-weighted or implicit non-orthogonal pressure)"
        );
        let mut net = vec![0.0; self.bodies.len()];
        let mut area = vec![0.0; self.bodies.len()];
        for ff in &self.flux_faces {
            let h = hby_a[ff.fluid];
            let uf: Vec3 = match ff.image {
                None => [0, 1, 2].map(|k| h[k] + ff.weight * (ff.wall[k] - h[k])),
                // ADR 0254 E1: the wall value reflected through the value at the image point
                Some((d, a)) => {
                    let hd = hby_a[d];
                    [0, 1, 2].map(|k| 2.0 * ff.wall[k] - ((1.0 - a) * h[k] + a * hd[k]))
                }
            };
            phi_hby_a[ff.face] = vec3::dot(uf, ff.sf);
            net[ff.body] += ff.sign * (phi_hby_a[ff.face] - ff.rigid);
            area[ff.body] += ff.area;
        }
        let mut src = vec![0.0; self.coeff.len()];
        for ff in &self.flux_faces {
            // ADR 0234 D3: the body neither creates nor removes fluid
            phi_hby_a[ff.face] -= ff.sign * ff.area * net[ff.body] / area[ff.body];
            src[ff.solid] -= ff.sign * (phi_hby_a[ff.face] - ff.rigid);
        }
        Some(src)
    }

    /// The refusal of ADR 0254 E2, for the transient solver: over the flux faces without an image whose weight is above
    /// [`FLUX_WEIGHT_LIMIT`], the largest `(weight - 1) c0_by_dt V_c / a_p[c]` that exceeds 1, with its fluid cell. `a_p` is the
    /// momentum diagonal of the step (volume-integrated), `c0_by_dt` the coefficient of the new level of the time scheme over
    /// the step. The number is the velocity, in units of what the time term carries over from the old level, that the linear
    /// extension can put on the face with its sign reversed; above 1 with a large weight the forcing of
    /// [`ImmersedForcing::build_second_order`] was measured to destroy a run whose first fluid cells are not yet slaved to the
    /// wall. `None` without such a face, without flux faces, and with `flux_weight_check = false`. A sufficient condition for
    /// a stop, not a stability criterion.
    pub fn flux_growth(&self, c0_by_dt: f64, a_p: &[f64]) -> Option<(usize, f64)> {
        if !self.flux_weight_check {
            return None;
        }
        let mut worst: Option<(usize, f64)> = None;
        for ff in &self.flux_faces {
            if ff.image.is_some() || ff.weight <= FLUX_WEIGHT_LIMIT {
                continue;
            }
            let g = (ff.weight - 1.0) * c0_by_dt * self.volumes[ff.fluid] / a_p[ff.fluid];
            if g > 1.0 && worst.map_or(true, |(_, w)| g > w) {
                worst = Some((ff.fluid, g));
            }
        }
        worst
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
    /// The moment has a first-order error from the arm of the modification terms (the crossing);
    /// [`ImmersedForcing::force_moment_second_order`] removes it (ADR 0239).
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

    /// Force and moment about `x_ref` of the fluid on body `body` with the moment to the order of the velocity field (F-337,
    /// ADR 0239 D1): the force of [`ImmersedForcing::force_moment`] (the same sum, bit for bit) and
    /// `M = sum_t (x_c(t) - x_ref) x dF_t` with every term applied at the centre of its cell (the fluid cell of a modification
    /// term, not the crossing). In a steady state on an orthogonal mesh this sum is the angular-momentum balance of the solid
    /// cells and the first fluid cells of the body: the moment of the pressure, of the full viscous stress and of the momentum
    /// flux on the regular fluid faces that enclose them. The remark of `force_moment` on a body that touches the domain boundary
    /// applies; for a body whose rigid velocity changes in time see [`ImmersedForcing::force_moment_unsteady`] (ADR 0249).
    pub fn force_moment_second_order(&self, body: usize, u: &[Vec3], rho: f64, x_ref: Vec3) -> (Vec3, Vec3) {
        let (mut force, mut moment) = (vec3::ZERO, vec3::ZERO);
        for t in &self.bodies[body].terms {
            let v = self.volumes[t.cell];
            let df: Vec3 = [0, 1, 2].map(|k| rho * v * (t.coeff * u[t.cell][k] - t.source[k]));
            force = vec3::add(force, df);
            let arm = vec3::sub(self.centres[t.cell], x_ref);
            moment = vec3::add(moment, vec3::cross(arm, df));
        }
        (force, moment)
    }

    /// Force and moment about `x_ref` of the fluid on body `body` in unsteady flow (F-346, ADR 0249 D1, D2): the pair of
    /// [`ImmersedForcing::force_moment_second_order`] plus the rate of change of the momentum and of the angular momentum that
    /// the solid cells of the body hold, `F += rho sum_s V_s dudt_s`, `M += rho sum_s (x_s - x_ref) x V_s dudt_s`. `dudt` is the
    /// rate of change of the cell velocity at the time level of `u`, one vector per cell (see [`velocity_rate`]).
    ///
    /// The reaction of the forcing is the discrete force on the staircase surface of the solid cells minus the storage of those
    /// cells; the storage is the momentum of a fictitious fluid inside the body, not fluid momentum, and is added back. The
    /// modification terms of the first fluid cells are surface forces of the staircase faces and the momentum of those cells is
    /// fluid momentum, so they do not enter. For a body at rest the result is the reaction (to the size of the solid velocity);
    /// with `dudt = 0` it is that of `force_moment_second_order`. Not accounted for: the fluid between the staircase and the
    /// surface (the staircase volume error times the acceleration of the body). The remark of `force_moment` on a body that
    /// touches the domain boundary applies.
    pub fn force_moment_unsteady(&self, body: usize, u: &[Vec3], dudt: &[Vec3], rho: f64, x_ref: Vec3) -> (Vec3, Vec3) {
        let (mut force, mut moment) = self.force_moment_second_order(body, u, rho, x_ref);
        for &c in &self.bodies[body].solid_cells {
            let dp = vec3::scale(dudt[c], rho * self.volumes[c]);
            force = vec3::add(force, dp);
            let arm = vec3::sub(self.centres[c], x_ref);
            moment = vec3::add(moment, vec3::cross(arm, dp));
        }
        (force, moment)
    }
}
