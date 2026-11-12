//! Finite-element structural solver (ADR 0270, basic tier).
//!
//! This crate holds the data model of a solid model (ADR 0271): the mesh of finite elements with its node, element and
//! surface sets ([`mesh`]), the materials, constraints, loads, the model and its result ([`model`]), and the structured
//! builders ([`builders`]). The other modules are declared here once, so that each later work package fills its own file
//! and none edits this one.
//!
//! Conventions (ADR 0270 section 2.2): SI units, global Cartesian axes; unknowns are numbered through
//! [`model::DofMap`]; symmetric tensors are six numbers in the order xx, yy, zz, xy, yz, zx, strains with engineering
//! shear; a surface set's normal points out of the solid; every refusal is an error naming the element, node, set or key.

pub mod assemble;
pub mod builders;
pub mod constraint;
pub mod convert;
pub mod element;
pub mod error;
pub mod loads;
pub mod mass;
pub mod material;
pub mod mesh;
pub mod modal;
pub mod model;
pub mod quadrature;
pub mod recover;
pub mod solve;
pub mod thermal;

pub use error::{FemError, Result, SetKind};
pub use mesh::{ElementBlock, ElementKind, FaceRef, SolidMesh};
pub use model::{Constraint, DofMap, Load, Material, SolidModel, SolidResult, SolverStats};
