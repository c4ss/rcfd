//! Strong (implicit) coupling of a rigid body with the moving-mesh flow step (F-251, ADR 0250). Opt-in: nothing in the library
//! calls it, and the loose coupling of ADR 0033 D7 is unchanged.
//!
//! One time step iterates on the fluid force and moment held over the body's step (ADR 0250 D4):
//!
//! ```text
//! F^1      = force of the flow at t_n                           (the force of the loose coupling)
//! iterate k: body from t_n with F^k -> mesh at the new pose -> flow step from t_n -> force Ft^k of the new flow
//!            r^k = Ft^k - F^k;   converged when |r^k| <= tol max(|Ft^k|, F_scale)   (and the same for the moment)
//!            F^{k+1} = F^k + w_k r^k
//! ```
//!
//! `w_k` is a fixed factor or Aitken's (D6). On the piston-column model the map `F^k -> Ft^k` has the slope `-m_a/(2M)`, so a
//! fixed factor converges for `0 < w < 4M/(2M + m_a)` and the best one is `2M/(2M + m_a)`
//! ([`relaxation_from_added_mass`], D5): the added-mass term of the body equation written as a relaxation.

use crate::ale::{move_mesh, set_follow_conditions};
use crate::pimple::{PimpleSolver, StepInfo};
use crate::sixdof::{patch_force_moment, Body, Integrator};
use rcfd_core::vec3::{self, Vec3};

/// Relaxation of the sub-iteration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Relaxation {
    /// The same factor in every iteration, `0 < w < 2`.
    Fixed(f64),
    /// Aitken's dynamic factor (ADR 0250 D6): `w_k = -w_{k-1} r_{k-1} . (r_k - r_{k-1}) / |r_k - r_{k-1}|^2`, started from
    /// `initial` in every time step, its magnitude capped at `max`.
    Aitken {
        /// Factor of the first update of a time step, `0 < initial < 2`.
        initial: f64,
        /// Cap of the magnitude of the factor, `>= initial`.
        max: f64,
    },
}

/// The factor of [`Relaxation`] for one sequence of 3-vector residuals.
#[derive(Debug, Clone, Copy)]
pub struct Relaxer {
    relaxation: Relaxation,
    omega: f64,
    previous: Option<Vec3>,
}

impl Relaxer {
    /// A sequence at its start.
    pub fn new(relaxation: Relaxation) -> Self {
        let omega = match relaxation {
            Relaxation::Fixed(w) => w,
            Relaxation::Aitken { initial, .. } => initial,
        };
        Relaxer {
            relaxation,
            omega,
            previous: None,
        }
    }

    /// The factor to apply to the residual `r` (the next iterate is `x + factor * r`). A zero or non-finite Aitken factor, or
    /// an unchanged residual, keeps the previous factor.
    pub fn factor(&mut self, r: Vec3) -> f64 {
        if let Relaxation::Aitken { max, .. } = self.relaxation {
            if let Some(p) = self.previous {
                let d = vec3::sub(r, p);
                let dd = vec3::dot(d, d);
                if dd > 0.0 {
                    let w = -self.omega * vec3::dot(p, d) / dd;
                    if w.is_finite() && w != 0.0 {
                        self.omega = w.clamp(-max, max);
                    }
                }
            }
            self.previous = Some(r);
        }
        self.omega
    }
}

/// The fixed relaxation factor (and the Aitken start) from an estimate of the added mass, `2M/(2M + m_a)` (ADR 0250 D5): the
/// factor that makes the sub-iteration converge in one update on the linear piston-column model, for a body advanced with a
/// constant force and a flow step whose wall velocity is the body's displacement over the step.
pub fn relaxation_from_added_mass(mass: f64, added_mass: f64) -> f64 {
    2.0 * mass / (2.0 * mass + added_mass)
}

/// Settings of the strong coupling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrongCoupling {
    /// Relaxation of the force and of the moment (one factor sequence each).
    pub relaxation: Relaxation,
    /// Converged when `max(|r_F|/max(|F|, force_scale), |r_M|/max(|M|, moment_scale)) <= tolerance`.
    pub tolerance: f64,
    /// Floor of the force norm in the convergence test (`>= 0`).
    pub force_scale: f64,
    /// Floor of the moment norm in the convergence test (`>= 0`).
    pub moment_scale: f64,
    /// Cap of the flow steps per time step (`>= 1`; 1 is the loose coupling).
    pub max_iterations: usize,
}

impl StrongCoupling {
    /// Aitken relaxation from `initial` with the cap 2, tolerance `1e-8`, no floors, at most 20 iterations.
    pub fn aitken(initial: f64) -> Self {
        StrongCoupling {
            relaxation: Relaxation::Aitken { initial, max: 2.0 },
            tolerance: 1e-8,
            force_scale: 0.0,
            moment_scale: 0.0,
            max_iterations: 20,
        }
    }

    /// Check the settings; the message names the offending one.
    pub fn validate(&self) -> Result<(), String> {
        let factor = |w: f64, what: &str| {
            if w.is_finite() && w > 0.0 && w < 2.0 {
                Ok(())
            } else {
                Err(format!("strong coupling: {what} must lie in (0, 2), got {w}"))
            }
        };
        match self.relaxation {
            Relaxation::Fixed(w) => factor(w, "the fixed relaxation factor")?,
            Relaxation::Aitken { initial, max } => {
                factor(initial, "the initial Aitken factor")?;
                if !(max.is_finite() && max >= initial) {
                    return Err(format!(
                        "strong coupling: the Aitken cap must be finite and at least the initial factor, got {max}"
                    ));
                }
            }
        }
        if !(self.tolerance.is_finite() && self.tolerance > 0.0) {
            return Err(format!(
                "strong coupling: the tolerance must be positive, got {}",
                self.tolerance
            ));
        }
        for (s, what) in [(self.force_scale, "force"), (self.moment_scale, "moment")] {
            if !(s.is_finite() && s >= 0.0) {
                return Err(format!(
                    "strong coupling: the {what} scale must be finite and not negative, got {s}"
                ));
            }
        }
        if self.max_iterations == 0 {
            return Err("strong coupling: at least one iteration is needed".into());
        }
        Ok(())
    }
}

/// What the coupling loop of ADR 0033 D7 needs to know about the body's surface.
#[derive(Debug, Clone)]
pub struct BodyCoupling {
    /// Fluid density (the flow solver's pressure is kinematic).
    pub rho: f64,
    /// Kinematic viscosity of the force evaluation.
    pub nu: f64,
    /// Wall patches whose force and moment act on the body.
    pub force_patches: Vec<String>,
    /// Patches whose velocity follows the mesh (`ale::set_follow_conditions`).
    pub follow_patches: Vec<String>,
    /// Integrator of the body.
    pub integrator: Integrator,
}

/// Record of one strongly coupled time step.
#[derive(Debug, Clone)]
pub struct CouplingStep {
    /// Flow steps taken.
    pub iterations: usize,
    /// Whether the convergence test was met (false: the cap was reached; the step is kept as the last iterate).
    pub converged: bool,
    /// The test quantity of every iteration.
    pub residuals: Vec<f64>,
    /// The force residual `Ft^k - F^k` of every iteration.
    pub force_residuals: Vec<Vec3>,
    /// The force relaxation factor applied after every iteration but the last.
    pub factors: Vec<f64>,
    /// Force held over the body's step in the last iterate.
    pub force: Vec3,
    /// Moment (about the centre of mass) held over the body's step in the last iterate.
    pub moment: Vec3,
    /// Diagnostics of the last flow step.
    pub flow: StepInfo,
    /// Mesh flux of the last mesh move.
    pub mesh_flux: Vec<f64>,
    /// Smallest ratio of new to old cell volume of the last mesh move.
    pub min_volume_ratio: f64,
}

fn ratio(r: Vec3, value: Vec3, scale: f64) -> f64 {
    let (n, d) = (vec3::norm(r), vec3::norm(value).max(scale));
    if n == 0.0 {
        0.0
    } else {
        n / d
    }
}

/// One time step of size `dt` of the body `body` and the flow `flow`, strongly coupled (ADR 0250 D4, D7). `points` gives the
/// mesh points for a pose of the body (the caller's smoothing), as in the loose loop.
///
/// With `max_iterations = 1` this is the loose step of ADR 0033 D7, operation for operation. When the cap is reached the step
/// is kept (body, mesh and flow of the last iterate) and `converged` is false.
///
/// Fails, leaving the flow and the body as they were, for invalid settings, a set `corrector_control` (its schedule would be
/// reset by the repeated steps), a failure of `points`, of the mesh move, of the body's integrator or of the flow step, and a
/// non-finite force, moment or velocity.
pub fn strong_step(
    flow: &mut PimpleSolver,
    body: &mut Body,
    dt: f64,
    coupling: &BodyCoupling,
    strong: &StrongCoupling,
    points: &mut dyn FnMut(&Body) -> Result<Vec<Vec3>, String>,
) -> Result<CouplingStep, String> {
    strong.validate()?;
    if flow.settings.corrector_control.is_some() {
        return Err("strong coupling: the adaptive corrector count (corrector_control) is not supported".into());
    }
    let body0 = body.state;
    let geom0 = flow.geom.clone();
    let u0 = flow.u.clone();
    let snap = flow.snapshot();
    let history0 = flow.history.clone();
    let lag0 = (flow.lag_estimates.len(), flow.lag_warnings.len());
    let (mut f, mut m) = patch_force_moment(
        &flow.geom,
        &flow.u,
        &flow.p,
        coupling.rho,
        coupling.nu,
        &coupling.force_patches,
        body0.x,
    )?;
    let mut relax_f = Relaxer::new(strong.relaxation);
    let mut relax_m = Relaxer::new(strong.relaxation);
    let mut out = CouplingStep {
        iterations: 0,
        converged: false,
        residuals: Vec::new(),
        force_residuals: Vec::new(),
        factors: Vec::new(),
        force: f,
        moment: m,
        flow: StepInfo {
            step: 0,
            time: flow.time,
            dt,
            courant: 0.0,
            continuity: 0.0,
            res_u: 0.0,
            res_p: 0.0,
            p_iters: 0,
        },
        mesh_flux: Vec::new(),
        min_volume_ratio: 1.0,
    };
    let mut started = false;
    let mut failure: Option<String> = None;
    for k in 1..=strong.max_iterations {
        if started {
            // back to t_n
            flow.geom = geom0.clone();
            flow.u = u0.clone();
            flow.restore(&snap).map_err(|e| e.to_string())?;
            flow.history = history0.clone();
            flow.lag_estimates.truncate(lag0.0);
            flow.lag_warnings.truncate(lag0.1);
            body.state = body0;
        }
        started = true;
        let iterate = (|| -> Result<(Vec3, Vec3), String> {
            body.advance(dt, coupling.integrator, (f, m))?;
            let pts = points(body)?;
            let st = move_mesh(&flow.geom, pts, dt)?;
            let walls = set_follow_conditions(&mut flow.u, &st, &geom0.mesh, &coupling.follow_patches, dt)?;
            out.min_volume_ratio = st.min_volume_ratio;
            out.mesh_flux = st.mesh_flux.clone();
            out.flow = flow.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls)?;
            if !flow.u.values.iter().flatten().all(|x| x.is_finite()) {
                return Err("strong coupling: non-finite velocity".into());
            }
            let (ft, mt) = patch_force_moment(
                &flow.geom,
                &flow.u,
                &flow.p,
                coupling.rho,
                coupling.nu,
                &coupling.force_patches,
                body.state.x,
            )?;
            if !ft.iter().chain(&mt).all(|x| x.is_finite()) {
                return Err("strong coupling: non-finite force or moment".into());
            }
            Ok((ft, mt))
        })();
        let (ft, mt) = match iterate {
            Ok(v) => v,
            Err(e) => {
                failure = Some(format!("{e} (iteration {k})"));
                break;
            }
        };
        let (rf, rm) = (vec3::sub(ft, f), vec3::sub(mt, m));
        let e = ratio(rf, ft, strong.force_scale).max(ratio(rm, mt, strong.moment_scale));
        out.iterations = k;
        out.residuals.push(e);
        out.force_residuals.push(rf);
        out.force = f;
        out.moment = m;
        if e <= strong.tolerance {
            out.converged = true;
            break;
        }
        if k == strong.max_iterations {
            break;
        }
        let wf = relax_f.factor(rf);
        let wm = relax_m.factor(rm);
        out.factors.push(wf);
        f = vec3::add(f, vec3::scale(rf, wf));
        m = vec3::add(m, vec3::scale(rm, wm));
    }
    if let Some(e) = failure {
        flow.geom = geom0;
        flow.u = u0;
        flow.restore(&snap).map_err(|e| e.to_string())?;
        flow.history = history0;
        flow.lag_estimates.truncate(lag0.0);
        flow.lag_warnings.truncate(lag0.1);
        body.state = body0;
        return Err(e);
    }
    Ok(out)
}
