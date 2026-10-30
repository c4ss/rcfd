//! Turbulence modelling: wall distance, standard wall functions (F-040) and
//! eddy-viscosity models (algebraic mixing length, k-epsilon family F-042).

pub mod eddy;
pub mod kepsilon;
pub mod les;
pub mod model;
pub mod rsm;
pub mod sa;
pub mod sst;
pub mod wall;
pub mod zeroeq;

pub use eddy::{effective_viscosity, patch_faces, MixingLength};
pub use kepsilon::{
    wall_cell_sources, KEpsilon, KEpsilonVariant, WallTreatment, REALIZABLE_A0_DEFAULT, REALIZABLE_A0_ORIGINAL,
};
pub use les::{AlgebraicSgs, SgsKind};
pub use model::{
    create_model, is_registered, is_sgs_key, ke_coefficient_table, mixing_length_coefficient_table, model_keys,
    register_model, sa_infos, smagorinsky_coefficient_table, sst_coefficient_table, supports_transient,
    wale_coefficient_table, CoefficientInfo, ModelContext, TurbulenceModel,
};
pub use rsm::{rsm_coefficient_table, PressureStrain, ReynoldsStress, RsmCoefficients};
pub use sa::{
    cell_delta, ddes_fd, ddes_length, ddes_rd, des_length, sa_local_source, sa_s_fw, DesDelta, DesMode, SLimiter,
    SaCoefficients, SaInnerLaw, SaWallMode, SpalartAllmaras,
};
pub use sst::KOmegaSst;
pub use wall::{wall_distance, wall_y_plus, SpaldingLaw, WallConstants};
pub use zeroeq::{MixingLengthModel, MixingProfile};
