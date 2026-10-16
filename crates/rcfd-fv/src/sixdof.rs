//! Six-degree-of-freedom rigid body (F-229, ADR 0033 D6/D7): equations of motion with constraints, spring/damper and
//! user-force hooks, two time integrators, and the force and moment of the flow on a set of wall patches.
//!
//! State: centre of mass `x`, velocity `v` (world), orientation quaternion `q` (body to world), angular velocity `w`
//! (body frame). Equations: `m v' = F`, `q' = (1/2) q (0, w)`, `I w' = M_b - w x (I w)`. Single-axis rotations
//! (`AboutAxis`, `Hinge`) are carried by the angle `theta` and its rate; the orientation, and for a hinge the position
//! and velocity, are derived from them.

use crate::field::{ScalarField, VectorField};
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_core::vec3::{self, Vec3};

/// Unit quaternion `(w, x, y, z)`.
pub type Quat = [f64; 4];

/// The identity rotation.
pub const QUAT_IDENTITY: Quat = [1.0, 0.0, 0.0, 0.0];

/// Hamilton product `a b`.
pub fn quat_mul(a: Quat, b: Quat) -> Quat {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// Unit quaternion of a rotation by `angle` about `axis` (any non-zero length).
pub fn quat_from_axis_angle(axis: Vec3, angle: f64) -> Quat {
    let n = vec3::norm(axis);
    if n < 1e-300 {
        return QUAT_IDENTITY;
    }
    let (s, c) = (0.5 * angle).sin_cos();
    [c, s * axis[0] / n, s * axis[1] / n, s * axis[2] / n]
}

/// Normalised copy.
pub fn quat_normalize(q: Quat) -> Quat {
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    q.map(|x| x / n)
}

/// Rotation matrix (row-major, body to world) of a unit quaternion.
pub fn quat_matrix(q: Quat) -> [[f64; 3]; 3] {
    let [w, x, y, z] = q;
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - w * z),
            2.0 * (x * z + w * y),
        ],
        [
            2.0 * (x * y + w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - w * x),
        ],
        [
            2.0 * (x * z - w * y),
            2.0 * (y * z + w * x),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

fn mat_vec(m: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn mat_t_vec(m: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2])
}

fn mat_inverse(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let c = |a: usize, b: usize, c: usize, d: usize| m[a][b] * m[c][d] - m[a][d] * m[c][b];
    let det = m[0][0] * c(1, 1, 2, 2) - m[0][1] * c(1, 0, 2, 2) + m[0][2] * c(1, 0, 2, 1);
    if det.is_nan() || det.abs() <= 1e-300 {
        return None;
    }
    let inv = 1.0 / det;
    Some([
        [c(1, 1, 2, 2) * inv, -c(0, 1, 2, 2) * inv, c(0, 1, 1, 2) * inv],
        [-c(1, 0, 2, 2) * inv, c(0, 0, 2, 2) * inv, -c(0, 0, 1, 2) * inv],
        [c(1, 0, 2, 1) * inv, -c(0, 0, 2, 1) * inv, c(0, 0, 1, 1) * inv],
    ])
}

/// Rotational freedom of a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rotation {
    /// No rotation.
    Locked,
    /// Free three-dimensional rotation (Euler's equations).
    Free,
    /// Rotation about a world-fixed axis through the centre of mass; the axis must be a principal axis of the body at
    /// the initial orientation.
    AboutAxis {
        /// Axis (any non-zero length).
        axis: Vec3,
    },
    /// Rotation about a fixed axis through the point `point`; the effective inertia is `I_axis + m d^2`.
    Hinge {
        /// Point on the axis.
        point: Vec3,
        /// Axis (any non-zero length).
        axis: Vec3,
    },
}

/// Time integrator of [`Body::advance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrator {
    /// Classical fourth-order Runge-Kutta (all rotation modes), quaternion renormalised after each step.
    Rk4,
    /// Newmark average acceleration (`beta = 1/4`, `gamma = 1/2`) for the translations and the single-axis rotations;
    /// the forces at the new state by fixed-point iteration. Not available for free rotation.
    Newmark,
}

/// Static description of a body.
#[derive(Debug, Clone)]
pub struct BodySpec {
    /// Mass (positive).
    pub mass: f64,
    /// Inertia tensor about the centre of mass in the body frame (symmetric, positive definite).
    pub inertia: [[f64; 3]; 3],
    /// A locked world axis has zero velocity and zero force component.
    pub translation_locked: [bool; 3],
    /// Rotational freedom.
    pub rotation: Rotation,
    /// Gravitational acceleration (zero = no weight).
    pub gravity: Vec3,
    /// Translational spring stiffness per world axis about `rest_position`.
    pub stiffness: [f64; 3],
    /// Translational damping per world axis.
    pub damping: [f64; 3],
    /// Rest position of the springs.
    pub rest_position: Vec3,
    /// Linear drag coefficient, `F = -drag v`.
    pub drag: f64,
    /// Torsional spring stiffness (single-axis rotations), about `rest_angle`.
    pub torsion_stiffness: f64,
    /// Torsional damping (single-axis rotations).
    pub torsion_damping: f64,
    /// Rest angle of the torsional spring.
    pub rest_angle: f64,
}

impl BodySpec {
    /// A body of the given mass and principal moments with every freedom locked, no spring, damper or weight.
    pub fn new(mass: f64, principal: Vec3) -> Self {
        BodySpec {
            mass,
            inertia: [
                [principal[0], 0.0, 0.0],
                [0.0, principal[1], 0.0],
                [0.0, 0.0, principal[2]],
            ],
            translation_locked: [true; 3],
            rotation: Rotation::Locked,
            gravity: vec3::ZERO,
            stiffness: [0.0; 3],
            damping: [0.0; 3],
            rest_position: vec3::ZERO,
            drag: 0.0,
            torsion_stiffness: 0.0,
            torsion_damping: 0.0,
            rest_angle: 0.0,
        }
    }
}

/// Dynamic state of a body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyState {
    /// Time.
    pub t: f64,
    /// Centre of mass.
    pub x: Vec3,
    /// Velocity of the centre of mass (world).
    pub v: Vec3,
    /// Orientation (body to world).
    pub q: Quat,
    /// Angular velocity in the body frame.
    pub w: Vec3,
    /// Rotation angle about the axis of a single-axis rotation, relative to the initial orientation.
    pub theta: f64,
    /// Rate of `theta`.
    pub theta_dot: f64,
}

impl BodyState {
    /// State at rest at `x` with the identity orientation.
    pub fn at(x: Vec3) -> Self {
        BodyState {
            t: 0.0,
            x,
            v: vec3::ZERO,
            q: QUAT_IDENTITY,
            w: vec3::ZERO,
            theta: 0.0,
            theta_dot: 0.0,
        }
    }
}

/// User force hook: `(state, time) -> (force world, moment about the centre of mass world)`.
pub type ForceHook = Box<dyn Fn(&BodyState, f64) -> (Vec3, Vec3) + Send + Sync>;

/// A rigid body with its equations of motion.
pub struct Body {
    /// Description.
    pub spec: BodySpec,
    /// Current state.
    pub state: BodyState,
    hook: Option<ForceHook>,
    inertia_inv: [[f64; 3]; 3],
    /// Unit axis of a single-axis rotation.
    axis: Vec3,
    /// Inertia about the axis (plus `m d^2` for a hinge).
    inertia_axis: f64,
    /// Initial position and orientation (the reference of the single-axis rotations and of [`Body::map_point`]).
    x_init: Vec3,
    q_init: Quat,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("spec", &self.spec)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone, Copy)]
struct Deriv {
    x: Vec3,
    v: Vec3,
    q: Quat,
    w: Vec3,
    theta: f64,
    theta_dot: f64,
}

impl Body {
    /// Build a body; fails with a located message for a non-positive mass, a singular inertia, a zero axis or a
    /// non-principal `AboutAxis` axis. Locked translation components of the initial velocity are set to zero.
    pub fn new(spec: BodySpec, mut state: BodyState) -> Result<Body, String> {
        if spec.mass.is_nan() || spec.mass <= 0.0 {
            return Err("six_dof.mass: must be positive".into());
        }
        let inertia_inv = mat_inverse(&spec.inertia).ok_or("six_dof.inertia: singular tensor")?;
        for i in 0..3 {
            for j in 0..3 {
                if (spec.inertia[i][j] - spec.inertia[j][i]).abs() > 1e-12 * (1.0 + spec.inertia[i][i].abs()) {
                    return Err("six_dof.inertia: the tensor must be symmetric".into());
                }
            }
            if spec.inertia[i][i].is_nan() || spec.inertia[i][i] <= 0.0 {
                return Err("six_dof.inertia: the moments must be positive".into());
            }
        }
        state.q = quat_normalize(state.q);
        for k in 0..3 {
            if spec.translation_locked[k] {
                state.v[k] = 0.0;
            }
        }
        let (axis, inertia_axis) = match spec.rotation {
            Rotation::AboutAxis { axis } | Rotation::Hinge { axis, .. } => {
                if vec3::norm(axis) < 1e-12 {
                    return Err("six_dof.axis: the rotation axis must be non-zero".into());
                }
                let a = vec3::unit(axis);
                let r = quat_matrix(state.q);
                // world inertia I_w = R I R^T applied to the axis
                let ia = mat_vec(&r, mat_vec(&spec.inertia, mat_t_vec(&r, a)));
                let iaa = vec3::dot(a, ia);
                if let Rotation::AboutAxis { .. } = spec.rotation {
                    let off = vec3::norm(vec3::sub(ia, vec3::scale(a, iaa)));
                    if off > 1e-9 * iaa.abs().max(1e-300) {
                        return Err("six_dof.axis: the axis is not a principal axis of the body".into());
                    }
                }
                let extra = match spec.rotation {
                    Rotation::Hinge { point, .. } => {
                        let r = vec3::sub(state.x, point);
                        let perp = vec3::sub(r, vec3::scale(a, vec3::dot(r, a)));
                        spec.mass * vec3::dot(perp, perp)
                    }
                    _ => 0.0,
                };
                (a, iaa + extra)
            }
            _ => ([0.0, 0.0, 1.0], 1.0),
        };
        let mut b = Body {
            x_init: state.x,
            q_init: state.q,
            spec,
            state,
            hook: None,
            inertia_inv,
            axis,
            inertia_axis,
        };
        let mut s = b.state;
        b.sync(&mut s);
        b.state = s;
        Ok(b)
    }

    /// Install the user force hook (added to every force evaluation).
    pub fn set_hook(&mut self, hook: ForceHook) {
        self.hook = Some(hook);
    }

    /// Rotation matrix (body to world) of the current orientation.
    pub fn rotation_matrix(&self) -> [[f64; 3]; 3] {
        quat_matrix(self.state.q)
    }

    /// Current position of the point that was at `p0` in the initial state and is rigidly attached to the body.
    pub fn map_point(&self, p0: Vec3) -> Vec3 {
        let r0 = quat_matrix(self.q_init);
        let rb = mat_t_vec(&r0, vec3::sub(p0, self.x_init));
        let r = quat_matrix(self.state.q);
        vec3::add(self.state.x, mat_vec(&r, rb))
    }

    /// Angular momentum about the centre of mass in the world frame.
    pub fn angular_momentum_world(&self) -> Vec3 {
        let r = quat_matrix(self.state.q);
        mat_vec(&r, mat_vec(&self.spec.inertia, self.state.w))
    }

    /// Rotational plus translational kinetic energy.
    pub fn kinetic_energy(&self) -> f64 {
        let iw = mat_vec(&self.spec.inertia, self.state.w);
        0.5 * self.spec.mass * vec3::dot(self.state.v, self.state.v) + 0.5 * vec3::dot(self.state.w, iw)
    }

    /// Recompute the quantities derived from the angle of a single-axis rotation.
    fn sync(&self, s: &mut BodyState) {
        match self.spec.rotation {
            Rotation::AboutAxis { .. } | Rotation::Hinge { .. } => {
                let qt = quat_from_axis_angle(self.axis, s.theta);
                s.q = quat_normalize(quat_mul(qt, self.q_init));
                let r = quat_matrix(s.q);
                s.w = mat_t_vec(&r, vec3::scale(self.axis, s.theta_dot));
                if let Rotation::Hinge { point, .. } = self.spec.rotation {
                    let rot = quat_matrix(qt);
                    let rel = mat_vec(&rot, vec3::sub(self.x_init, point));
                    s.x = vec3::add(point, rel);
                    s.v = vec3::scale(vec3::cross(self.axis, rel), s.theta_dot);
                }
            }
            _ => {}
        }
    }

    /// Force and moment (world, about the centre of mass) without the fluid.
    fn applied(&self, s: &BodyState, fluid: (Vec3, Vec3)) -> (Vec3, Vec3) {
        let sp = &self.spec;
        let mut f = vec3::add(vec3::scale(sp.gravity, sp.mass), fluid.0);
        for k in 0..3 {
            f[k] -= sp.stiffness[k] * (s.x[k] - sp.rest_position[k]) + (sp.damping[k] + sp.drag) * s.v[k];
        }
        let mut m = fluid.1;
        if let Some(h) = &self.hook {
            let (hf, hm) = h(s, s.t);
            f = vec3::add(f, hf);
            m = vec3::add(m, hm);
        }
        if matches!(sp.rotation, Rotation::AboutAxis { .. } | Rotation::Hinge { .. }) {
            let tau = -sp.torsion_stiffness * (s.theta - sp.rest_angle) - sp.torsion_damping * s.theta_dot;
            m = vec3::add(m, vec3::scale(self.axis, tau));
        }
        (f, m)
    }

    /// Accelerations: `(a, w_dot, theta_ddot)`.
    fn accel(&self, s: &BodyState, fluid: (Vec3, Vec3)) -> (Vec3, Vec3, f64) {
        let (f, m) = self.applied(s, fluid);
        let mut a = vec3::scale(f, 1.0 / self.spec.mass);
        for k in 0..3 {
            if self.spec.translation_locked[k] {
                a[k] = 0.0;
            }
        }
        match self.spec.rotation {
            Rotation::Locked => (a, vec3::ZERO, 0.0),
            Rotation::Free => {
                let r = quat_matrix(s.q);
                let mb = mat_t_vec(&r, m);
                let iw = mat_vec(&self.spec.inertia, s.w);
                let rhs = vec3::sub(mb, vec3::cross(s.w, iw));
                (a, mat_vec(&self.inertia_inv, rhs), 0.0)
            }
            Rotation::AboutAxis { .. } => (a, vec3::ZERO, vec3::dot(self.axis, m) / self.inertia_axis),
            Rotation::Hinge { point, .. } => {
                let r = vec3::sub(s.x, point);
                let ma = vec3::dot(self.axis, vec3::add(m, vec3::cross(r, f)));
                (vec3::ZERO, vec3::ZERO, ma / self.inertia_axis)
            }
        }
    }

    fn deriv(&self, s: &BodyState, fluid: (Vec3, Vec3)) -> Deriv {
        let (a, wd, thdd) = self.accel(s, fluid);
        let half_w = quat_mul(s.q, [0.0, s.w[0], s.w[1], s.w[2]]).map(|x| 0.5 * x);
        Deriv {
            x: s.v,
            v: a,
            q: half_w,
            w: wd,
            theta: s.theta_dot,
            theta_dot: thdd,
        }
    }

    fn offset(&self, s: &BodyState, d: &Deriv, h: f64) -> BodyState {
        let mut o = *s;
        o.t += h;
        o.x = vec3::add(s.x, vec3::scale(d.x, h));
        o.v = vec3::add(s.v, vec3::scale(d.v, h));
        for k in 0..4 {
            o.q[k] = s.q[k] + h * d.q[k];
        }
        o.w = vec3::add(s.w, vec3::scale(d.w, h));
        o.theta = s.theta + h * d.theta;
        o.theta_dot = s.theta_dot + h * d.theta_dot;
        self.sync(&mut o);
        o
    }

    /// Advance by `dt` with the fluid force and moment `fluid` (world frame, moment about the centre of mass) held
    /// constant over the step (explicit loose coupling).
    pub fn advance(&mut self, dt: f64, integrator: Integrator, fluid: (Vec3, Vec3)) -> Result<(), String> {
        match integrator {
            Integrator::Rk4 => {
                let s0 = self.state;
                let k1 = self.deriv(&s0, fluid);
                let s2 = self.offset(&s0, &k1, 0.5 * dt);
                let k2 = self.deriv(&s2, fluid);
                let s3 = self.offset(&s0, &k2, 0.5 * dt);
                let k3 = self.deriv(&s3, fluid);
                let s4 = self.offset(&s0, &k3, dt);
                let k4 = self.deriv(&s4, fluid);
                let mut s = s0;
                s.t += dt;
                let c = dt / 6.0;
                for k in 0..3 {
                    s.x[k] += c * (k1.x[k] + 2.0 * k2.x[k] + 2.0 * k3.x[k] + k4.x[k]);
                    s.v[k] += c * (k1.v[k] + 2.0 * k2.v[k] + 2.0 * k3.v[k] + k4.v[k]);
                    s.w[k] += c * (k1.w[k] + 2.0 * k2.w[k] + 2.0 * k3.w[k] + k4.w[k]);
                }
                for k in 0..4 {
                    s.q[k] += c * (k1.q[k] + 2.0 * k2.q[k] + 2.0 * k3.q[k] + k4.q[k]);
                }
                s.q = quat_normalize(s.q);
                s.theta += c * (k1.theta + 2.0 * k2.theta + 2.0 * k3.theta + k4.theta);
                s.theta_dot += c * (k1.theta_dot + 2.0 * k2.theta_dot + 2.0 * k3.theta_dot + k4.theta_dot);
                for k in 0..3 {
                    if self.spec.translation_locked[k] {
                        s.x[k] = s0.x[k];
                        s.v[k] = 0.0;
                    }
                }
                if self.spec.rotation == Rotation::Locked {
                    s.q = s0.q;
                    s.w = vec3::ZERO;
                }
                self.sync(&mut s);
                self.state = s;
                Ok(())
            }
            Integrator::Newmark => self.newmark(dt, fluid),
        }
    }

    fn newmark(&mut self, dt: f64, fluid: (Vec3, Vec3)) -> Result<(), String> {
        if self.spec.rotation == Rotation::Free {
            return Err(
                "six_dof.integrator: Newmark does not support free three-dimensional rotation (use rk4)".into(),
            );
        }
        let (beta, gamma) = (0.25, 0.5);
        let s0 = self.state;
        let (a0, _, th0) = self.accel(&s0, fluid);
        let hinge = matches!(self.spec.rotation, Rotation::Hinge { .. });
        let single = matches!(self.spec.rotation, Rotation::AboutAxis { .. } | Rotation::Hinge { .. });
        let (mut a1, mut th1) = (a0, th0);
        for _ in 0..100 {
            let mut s = s0;
            s.t += dt;
            for k in 0..3 {
                if hinge || self.spec.translation_locked[k] {
                    continue;
                }
                s.x[k] = s0.x[k] + dt * s0.v[k] + dt * dt * ((0.5 - beta) * a0[k] + beta * a1[k]);
                s.v[k] = s0.v[k] + dt * ((1.0 - gamma) * a0[k] + gamma * a1[k]);
            }
            if single {
                s.theta = s0.theta + dt * s0.theta_dot + dt * dt * ((0.5 - beta) * th0 + beta * th1);
                s.theta_dot = s0.theta_dot + dt * ((1.0 - gamma) * th0 + gamma * th1);
            }
            self.sync(&mut s);
            let (a_new, _, th_new) = self.accel(&s, fluid);
            let scale = a_new.iter().fold(th_new.abs(), |m, v| m.max(v.abs())).max(1e-300);
            let diff = (0..3)
                .map(|k| (a_new[k] - a1[k]).abs())
                .fold((th_new - th1).abs(), f64::max);
            a1 = a_new;
            th1 = th_new;
            if diff <= 1e-14 * scale {
                let mut f = s0;
                f.t += dt;
                for k in 0..3 {
                    if hinge || self.spec.translation_locked[k] {
                        continue;
                    }
                    f.x[k] = s0.x[k] + dt * s0.v[k] + dt * dt * ((0.5 - beta) * a0[k] + beta * a1[k]);
                    f.v[k] = s0.v[k] + dt * ((1.0 - gamma) * a0[k] + gamma * a1[k]);
                }
                if single {
                    f.theta = s0.theta + dt * s0.theta_dot + dt * dt * ((0.5 - beta) * th0 + beta * th1);
                    f.theta_dot = s0.theta_dot + dt * ((1.0 - gamma) * th0 + gamma * th1);
                }
                self.sync(&mut f);
                self.state = f;
                return Ok(());
            }
        }
        Err("six_dof.integrator: the Newmark force iteration did not converge in 100 iterations".into())
    }
}

/// Force and moment of the flow on the body for the wall patches `patches` (ADR 0033 D6):
/// `F = rho sum_f [p_f S_f - nu ((grad u)_f + (grad u)_f^T) . S_f]`, `M = sum_f (fc_f - x_ref) x dF_f`, where `S_f` are
/// the outward (from the fluid) area vectors, `p` is the kinematic pressure and `u` the velocity of the fluid. The
/// boundary velocity gradient replaces the normal derivative of the cell gradient by the one-sided difference
/// `(u_b - u_c)/dn`; the face pressure is the cell value extrapolated with the cell gradient.
pub fn patch_force_moment(
    geom: &FvGeometry,
    u: &VectorField,
    p: &ScalarField,
    rho: f64,
    nu: f64,
    patches: &[String],
    x_ref: Vec3,
) -> Result<(Vec3, Vec3), String> {
    let mesh = &geom.mesh;
    let n_int = geom.n_internal;
    let ub = u.boundary_values(geom);
    let pb = p.boundary_values(geom);
    let grad_u: Vec<Vec<Vec3>> = (0..3)
        .map(|k| {
            let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            least_squares(geom, &cells, &bv)
        })
        .collect();
    let grad_p = least_squares(geom, &p.values, &pb);
    let (mut force, mut moment) = (vec3::ZERO, vec3::ZERO);
    for name in patches {
        let patch = mesh.patch(name).ok_or_else(|| format!("no patch '{name}'"))?;
        for f in patch.faces() {
            if geom.empty[f] {
                continue;
            }
            let c = mesh.owner()[f];
            let s = mesh.face_area_vectors()[f];
            let area = vec3::norm(s);
            let n = vec3::scale(s, 1.0 / area.max(1e-300));
            let fc = mesh.face_centres()[f];
            let dn = geom.dn[f - n_int];
            let pf = p.values[c] + vec3::dot(grad_p[c], vec3::sub(fc, mesh.cell_centres()[c]));
            // G[i][j] = d u_i / d x_j at the face
            let mut g = [[0.0; 3]; 3];
            for i in 0..3 {
                let gc = grad_u[i][c];
                let corr = (ub[f - n_int][i] - u.values[c][i]) / dn - vec3::dot(gc, n);
                for j in 0..3 {
                    g[i][j] = gc[j] + corr * n[j];
                }
            }
            let mut df = vec3::scale(s, pf);
            for i in 0..3 {
                let tij: f64 = (0..3).map(|j| (g[i][j] + g[j][i]) * s[j]).sum();
                df[i] -= nu * tij;
            }
            let df = vec3::scale(df, rho);
            force = vec3::add(force, df);
            moment = vec3::add(moment, vec3::cross(vec3::sub(fc, x_ref), df));
        }
    }
    Ok((force, moment))
}
