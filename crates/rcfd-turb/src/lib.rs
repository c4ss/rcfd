//! Turbulence modelling: wall distance, standard wall functions (F-040) and
//! eddy-viscosity models (algebraic mixing length, k-epsilon family F-042).

pub mod eddy;
pub mod kepsilon;
pub mod model;
pub mod sa;
pub mod sst;
pub mod wall;

pub use eddy::{effective_viscosity, patch_faces, MixingLength};
pub use kepsilon::{wall_cell_sources, KEpsilon, KEpsilonVariant, WallTreatment};
pub use model::{
    create_model, is_registered, ke_coefficient_table, model_keys, register_model, sa_infos, sst_coefficient_table,
    CoefficientInfo, ModelContext, TurbulenceModel,
};
pub use sa::{SLimiter, SaCoefficients, SaWallMode, SpalartAllmaras};
pub use sst::KOmegaSst;
pub use wall::{wall_distance, wall_y_plus, SpaldingLaw, WallConstants};
