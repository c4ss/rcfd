//! One-way coupled Lagrangian particle tracking (F-056, ADR 0028).
//!
//! Point particles with Stokes or Schiller-Naumann drag, gravity with buoyancy, integrated by an analytic exponential
//! scheme or Euler / midpoint / RK4, located and tracked across the faces of an unstructured polyhedral mesh, with wall
//! reflection, trapping and escape, point and surface injection, constant and Rosin-Rammler diameters and VTK output;
//! a wall film (ADR 0192) with optional gas buoyancy and gas shear (ADR 0206), erosion and rough-wall rebound (ADR 0194),
//! random-walk dispersion on per-cell fields that a caller may replace every step and a surface reaction (ADR 0208),
//! droplet evaporation with the d-squared law and TAB breakup (ADR 0224), their mass, heat and momentum in the exchange
//! record (ADR 0230).
//!
//! The crate depends on `rcfd-mesh` only; the fluid velocity and its gradient are inputs ([`Flow`]).

pub mod breakup;
pub mod coupling;
pub mod dispersion;
pub mod drag;
pub mod erosion;
pub mod evaporation;
pub mod film;
pub mod inject;
pub mod locate;
pub mod reaction;
pub mod rng;
pub mod rough;
pub mod tracker;
pub mod vtp;

pub use breakup::{normal_speed, oscillator, sauter_radius, Breakup, BreakupSpec, Oscillator};
pub use coupling::{Coupling, DropletSources, Thermal};
pub use dispersion::{drw_msd, taylor_msd, Dispersion};
pub use drag::{drag_factor, step_particle, DragLaw, Fluid, Scheme};
pub use erosion::{
    dnv_angle_function, erosion_preset, impact_angle, Erosion, ErosionPreset, ErosionSpec, DNV_A, EROSION_PRESETS,
    PRESET_PROVENANCE,
};
pub use evaporation::{EvapRates, Evaporation, EvaporationSpec, R_UNIVERSAL};
pub use film::{
    buoyant_thickness, film_velocity, nusselt_thickness, shear_film_velocity, shear_thickness, Film, FilmGas, FilmSpec,
};
pub use inject::{rr_oversize, rr_sample, ConeSpec, InjKind, InjectorExt, InjectorSpec, SizeDist, VelMode};
pub use locate::{Locator, Walk};
pub use reaction::{GasSpecies, ReactionSpec, SurfaceReaction};
pub use rng::SplitMix64;
pub use rough::{rough_rebound, tangent_pair, RoughCounts, RoughSpec, RoughWall};
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
