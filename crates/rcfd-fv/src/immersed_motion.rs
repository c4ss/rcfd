//! Moving immersed shapes (F-308, ADR 0631; opt-in, nothing in the library calls it).
//!
//! The forcing of [`crate::immersed`] (ADR 0218) is built for shapes at rest. A moving shape is that forcing rebuilt before every
//! step at the pose and the rigid velocity of the new time level: [`crate::pimple::PimpleSolver`] reads its `immersed` field in
//! every outer iteration and keeps nothing derived from it across steps, so [`MovingImmersed::step`] sets the field to
//! [`MovingImmersed::forcing_at`] of `time + dt` and calls the step. Implicit Euler and BDF2 treat the forcing at the new level.
//!
//! Cells that change side need no special treatment (ADR 0631 D1): a cell that leaves the solid starts its first fluid step from
//! its penalised velocity, the rigid velocity of the body at its centre, which is the no-slip value of the fluid next to the
//! surface; a cell that enters the solid is penalised in its first solid step. The force of the fluid on a moving body is the
//! discrete force on the staircase surface of its solid cells at the new level, the reaction plus the storage of those cells
//! (`ImmersedForcing::force_moment_unsteady` with [`crate::immersed::velocity_rate`] of the step, ADR 0631 D3): the storage of a
//! cell that has just entered the solid cancels the part of its reaction that forced its fluid velocity to the rigid one.
//!
//! Motion: a harmonic translation per body ([`HarmonicTranslation`]) on top of the body's own constant velocity and spin. The
//! first-order face flux of `ImmersedForcing::build` only; not on a moving mesh (the solver refuses an immersed forcing there).

use crate::geometry::FvGeometry;
use crate::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use crate::pimple::{PimpleSolver, StepInfo};
use rcfd_core::vec3::{self, Vec3};

/// A harmonic translation `d(t) = amplitude sin(omega t + phase)` of a body from its mean pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicTranslation {
    /// Amplitude vector (m).
    pub amplitude: Vec3,
    /// Angular frequency (rad/s).
    pub omega: f64,
    /// Phase (rad).
    pub phase: f64,
}

impl HarmonicTranslation {
    /// Displacement from the mean pose at time `t`.
    pub fn displacement(&self, t: f64) -> Vec3 {
        vec3::scale(self.amplitude, (self.omega * t + self.phase).sin())
    }

    /// Velocity at time `t`, `amplitude omega cos(omega t + phase)`.
    pub fn velocity(&self, t: f64) -> Vec3 {
        vec3::scale(self.amplitude, self.omega * (self.omega * t + self.phase).cos())
    }

    /// Acceleration at time `t`, `-amplitude omega^2 sin(omega t + phase)`.
    pub fn acceleration(&self, t: f64) -> Vec3 {
        vec3::scale(
            self.amplitude,
            -self.omega * self.omega * (self.omega * t + self.phase).sin(),
        )
    }

    fn check(&self) -> Result<(), String> {
        if !self.amplitude.iter().all(|a| a.is_finite()) || !self.omega.is_finite() || !self.phase.is_finite() {
            return Err(format!(
                "amplitude, omega and phase must be finite (got amplitude {:?}, omega {}, phase {})",
                self.amplitude, self.omega, self.phase
            ));
        }
        Ok(())
    }
}

/// `shape` moved by `d`.
pub fn translate_shape(shape: SolidShape, d: Vec3) -> SolidShape {
    match shape {
        SolidShape::Cylinder { centre, axis, radius } => SolidShape::Cylinder {
            centre: vec3::add(centre, d),
            axis,
            radius,
        },
        SolidShape::Sphere { centre, radius } => SolidShape::Sphere {
            centre: vec3::add(centre, d),
            radius,
        },
        SolidShape::Cuboid { lo, hi } => SolidShape::Cuboid {
            lo: vec3::add(lo, d),
            hi: vec3::add(hi, d),
        },
    }
}

/// Immersed bodies with prescribed motion (ADR 0631 D1).
#[derive(Debug, Clone, PartialEq)]
pub struct MovingImmersed {
    /// The bodies at their mean pose, with their own constant velocity and spin.
    pub bodies: Vec<ImmersedBody>,
    /// One slot per body: its translation, `None` for a body that does not move.
    pub motions: Vec<Option<HarmonicTranslation>>,
    /// Settings of the forcing.
    pub settings: ImmersedSettings,
    /// Momentum face diffusivity, one value per face (as for `ImmersedForcing::build`).
    pub gamma: Vec<f64>,
    /// Put the velocity of the motion into the rigid velocity of the forcing (`true` from `new`). `false` is the negative control
    /// of ADR 0631 N1: the shape moves, its solid holds only the body's own velocity.
    pub velocity_in_forcing: bool,
}

impl MovingImmersed {
    /// Bodies with their motions. Fails, with a message naming ADR 0631, for a motion count different from the body count or a
    /// motion with a non-finite amplitude, omega or phase. The shapes, the settings and `gamma` are checked by the build.
    pub fn new(
        bodies: Vec<ImmersedBody>,
        motions: Vec<Option<HarmonicTranslation>>,
        settings: ImmersedSettings,
        gamma: Vec<f64>,
    ) -> Result<MovingImmersed, String> {
        if motions.len() != bodies.len() {
            return Err(format!(
                "moving immersed solid (ADR 0631): {} motions for {} bodies (one slot per body, None for a fixed body)",
                motions.len(),
                bodies.len()
            ));
        }
        for (b, m) in bodies.iter().zip(&motions) {
            if let Some(m) = m {
                m.check()
                    .map_err(|e| format!("moving immersed solid '{}' (ADR 0631): {e}", b.name))?;
            }
        }
        Ok(MovingImmersed {
            bodies,
            motions,
            settings,
            gamma,
            velocity_in_forcing: true,
        })
    }

    /// The bodies at time `t`: each moving body's shape and rotation origin translated by its displacement, its velocity its own
    /// plus that of the motion (the latter left out when `velocity_in_forcing` is false).
    pub fn bodies_at(&self, t: f64) -> Vec<ImmersedBody> {
        self.bodies
            .iter()
            .zip(&self.motions)
            .map(|(b, m)| match m {
                None => b.clone(),
                Some(m) => {
                    let d = m.displacement(t);
                    let mut out = b.clone();
                    out.shape = translate_shape(b.shape, d);
                    out.origin = vec3::add(b.origin, d);
                    if self.velocity_in_forcing {
                        out.velocity = vec3::add(b.velocity, m.velocity(t));
                    }
                    out
                }
            })
            .collect()
    }

    /// The forcing of ADR 0218 for the bodies at time `t` on `geom`.
    pub fn forcing_at(&self, geom: &FvGeometry, t: f64) -> Result<ImmersedForcing, String> {
        ImmersedForcing::build(geom, &self.bodies_at(t), self.settings, &self.gamma)
            .map_err(|e| format!("moving immersed solid (ADR 0631) at t = {t}: {e}"))
    }

    /// One step of `solver` with the forcing at the pose of the new time level `solver.time + dt`. The solver's own refusals
    /// (the fractional step, ADR 0218 D4) apply.
    pub fn step(&self, solver: &mut PimpleSolver, dt: f64) -> Result<StepInfo, String> {
        let t_new = solver.time + dt;
        solver.immersed = Some(self.forcing_at(&solver.geom, t_new)?);
        Ok(solver.step(dt))
    }
}
