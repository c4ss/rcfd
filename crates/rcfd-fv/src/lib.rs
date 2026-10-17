//! Finite-volume discretisation on polyhedral meshes: geometry, fields and
//! boundary conditions, gradients, convection and diffusion operators,
//! time integration and the pressure-velocity coupled solvers.

pub mod actuator;
pub mod ale;
pub mod bc;
pub mod cavitation;
pub mod cht;
pub mod compressible;
pub mod compressible_bc;
pub mod convection;
pub mod coupled;
pub mod ddt;
pub mod diffusion;
pub mod electric;
pub mod energy;
pub mod field;
pub mod fvmatrix;
pub mod geometry;
pub mod gradient;
pub mod hx;
pub mod lorentz;
pub mod lumped;
pub mod masstransfer;
pub mod mixture;
pub mod mixing_plane;
pub mod outlets;
pub mod pimple;
pub mod popbal;
pub mod potential;
pub mod pressure;
pub mod radiation;
pub mod realgas;
pub mod riemann;
pub mod rotor_interface;
pub mod scalar;
pub mod simple;
pub mod sixdof;
pub mod smoothing;
pub mod solidification;
pub mod sources;
pub mod stepctl;
pub mod thermal_bc;
pub mod varden;
pub mod viscoelastic;
pub mod vof;
pub mod waves;

pub use bc::{
    Bc, BcCtx, Directed, Empty, FaceCoeffs, FixedGradient, FixedValue, Freestream, Mixed, Periodic, Profile, Switched,
    Symmetry, ZeroGradient,
};
pub use field::{ScalarField, VectorField};
pub use fvmatrix::FvMatrix;
pub use geometry::FvGeometry;
pub use thermal_bc::ConvectiveWall;
