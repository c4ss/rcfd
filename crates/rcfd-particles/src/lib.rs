//! One-way coupled Lagrangian particle tracking (F-056, ADR 0028).
//!
//! Point particles with Stokes or Schiller-Naumann drag, gravity with buoyancy, integrated by an analytic exponential
//! scheme or Euler / midpoint / RK4, located and tracked across the faces of an unstructured polyhedral mesh, with wall
//! reflection, trapping and escape, point and surface injection, constant and Rosin-Rammler diameters and VTK output.
//!
//! The crate depends on `rcfd-mesh` only; the fluid velocity and its gradient are inputs ([`Flow`]).

pub mod coupling;
pub mod dispersion;
pub mod drag;
pub mod inject;
pub mod locate;
pub mod rng;
pub mod tracker;
pub mod vtp;

pub use drag::{drag_factor, step_particle, DragLaw, Fluid, Scheme};
pub use coupling::{Coupling, Thermal};
pub use dispersion::Dispersion;
pub use inject::{rr_oversize, rr_sample, ConeSpec, InjKind, InjectorExt, InjectorSpec, SizeDist, VelMode};
pub use locate::{Locator, Walk};
pub use rng::SplitMix64;
pub use tracker::{fluid_at, Flow, Interp, Options, Particle, State, Tracker, WallAction};

/// Errors of the particle tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleError(pub String);

impl std::fmt::Display for ParticleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParticleError {}

/// Result type of the crate.
pub type Result<T> = std::result::Result<T, ParticleError>;

pub(crate) fn err<T>(m: impl Into<String>) -> Result<T> {
    Err(ParticleError(m.into()))
}
