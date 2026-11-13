//! Finite-element structural solver (ADR 0270, basic tier; ADR 0290, intermediate tier).
//!
//! This crate holds the data model of a solid model (ADR 0271): the mesh of finite elements with its node, element and
//! surface sets ([`mesh`]), the materials, constraints, loads, the model and its result ([`model`]), and the structured
//! builders ([`builders`]). The other modules are declared here once, so that each later work package fills its own file
//! and none edits this one. The modules of the intermediate tier ([`hex20`], [`hex8x`], [`beam`], [`shell`], [`mpc`],
//! [`cyclic`], [`geometric`], [`buckling`], [`rotating`], [`nonlinear`], [`transient`], [`harmonic`]; ADR 0290 section 2.2)
//! are declared by ADR 0291; what they add is refused with [`FemError::NotYetSupported`] until their package lands.
//!
//! Conventions (ADR 0270 section 2.2): SI units, global Cartesian axes; unknowns are numbered through
//! [`model::DofMap`]; symmetric tensors are six numbers in the order xx, yy, zz, xy, yz, zx, strains with engineering
//! shear; a surface set's normal points out of the solid; every refusal is an error naming the element, node, set or key.

pub mod assemble;
pub mod beam;
pub mod buckling;
pub mod builders;
pub mod constraint;
pub mod convert;
pub mod cyclic;
pub mod element;
pub mod error;
pub mod geometric;
pub mod harmonic;
pub mod hex20;
pub mod hex8x;
pub mod loads;
pub mod mass;
pub mod material;
pub mod mesh;
pub mod modal;
pub mod model;
pub mod mpc;
pub mod nonlinear;
pub mod quadrature;
pub mod recover;
pub mod rotating;
pub mod shell;
pub mod solve;
pub mod thermal;
pub mod transient;

pub use error::{FemError, Result, SetKind};
pub use mesh::{BeamSection, BlockProperty, ElementBlock, ElementKind, FaceRef, SolidFormulation, SolidMesh};
pub use model::{Constraint, DofMap, ElasticSupport, LinearTerm, Load, Material, SolidModel, SolidResult, SolverStats};
