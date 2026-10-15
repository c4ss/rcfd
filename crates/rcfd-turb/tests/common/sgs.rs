//! Helpers of the ADR 0026 tests: a neutral `ModelContext` and exact linear boundary values.
#![allow(dead_code)]

use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::{FvGeometry, VectorField};
use rcfd_turb::kepsilon::WallTreatment;
use rcfd_turb::{KEpsilonVariant, MixingProfile, ModelContext, SLimiter, SaWallMode};
use std::sync::Arc;

pub fn ctx(geom: &Arc<FvGeometry>, nu: f64, walls: &[&str]) -> ModelContext {
    ModelContext {
        geom: geom.clone(),
        nu,
        walls: walls.iter().map(|s| s.to_string()).collect(),
        k0: 1.0,
        eps0: 1.0,
        omega0: 1.0,
        nutilde0: 3.0 * nu,
        variant: KEpsilonVariant::Standard,
        wall_treatment: WallTreatment::Smooth,
        sa_wall_mode: SaWallMode::Auto,
        sa_limiter: SLimiter::Allmaras,
        sa_negative: true,
        kappa: 0.41,
        e_wall: 9.8,
        convection: ConvScheme::Upwind,
        relax_turb: 0.7,
        relax_nut: 0.5,
        wall_damping: false,
        length_scale: 0.0,
        mixing_profile: MixingProfile::Linear,
        time_accurate: true,
    }
}

/// Set the exact value `f(x_face)` on every face of the named patches.
pub fn set_exact_bc(u: &mut VectorField, geom: &Arc<FvGeometry>, patches: &[&str], f: &dyn Fn([f64; 3]) -> [f64; 3]) {
    for p in patches {
        let fc = geom.mesh.face_centres();
        let vals: Vec<[f64; 3]> = geom.mesh.patch(p).unwrap().faces().map(|face| f(fc[face])).collect();
        u.set_bc(geom, p, Box::new(FixedValue(Profile::PerFace(vals)))).unwrap();
    }
}
