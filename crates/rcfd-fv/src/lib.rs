//! Finite-volume discretisation on polyhedral meshes: geometry, fields and
//! boundary conditions, gradients, convection and diffusion operators,
//! time integration and the pressure-velocity coupled solvers.

pub mod bc;
pub mod compressible;
pub mod compressible_bc;
pub mod convection;
pub mod coupled;
pub mod ddt;
pub mod diffusion;
pub mod energy;
pub mod field;
pub mod fvmatrix;
pub mod geometry;
pub mod gradient;
pub mod lumped;
pub mod outlets;
pub mod pimple;
pub mod potential;
pub mod pressure;
pub mod radiation;
pub mod realgas;
pub mod riemann;
pub mod scalar;
pub mod simple;
pub mod sources;
pub mod varden;

pub use bc::{
    Bc, BcCtx, Directed, Empty, FaceCoeffs, FixedGradient, FixedValue, Freestream, Mixed, Periodic, Profile, Switched,
    Symmetry, ZeroGradient,
};
pub use field::{ScalarField, VectorField};
pub use fvmatrix::FvMatrix;
pub use geometry::FvGeometry;
