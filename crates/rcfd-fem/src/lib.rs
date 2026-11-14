//! Finite-element structural solver (ADR 0270, basic tier; ADR 0290, intermediate tier).
//!
//! This crate holds the data model of a solid model (ADR 0271): the mesh of finite elements with its node, element and
//! surface sets ([`mesh`]), the materials, constraints, loads, the model and its result ([`model`]), and the structured
//! builders ([`builders`]). The other modules are declared here once, so that each later work package fills its own file
//! and none edits this one. The modules of the intermediate tier ([`hex20`], [`hex8x`], [`beam`], [`shell`], [`mpc`],
//! [`cyclic`], [`geometric`], [`buckling`], [`rotating`], [`nonlinear`], [`transient`], [`harmonic`]; ADR 0290 section 2.2)
//! are declared by ADR 0291; what they add is refused with [`FemError::NotYetSupported`] until their package lands. The
//! modules of the advanced tier (ADR 0310 section 2.2: the materials [`mat_ortho`], [`mat_table`], [`mat_visco`],
//! [`mat_creep`], [`mat_j2`], [`mat_hyper`]; [`shell3`], [`laminate`], [`geometric_struct`], [`nonlinear_md`],
//! [`nl_transient`], [`follower`], [`arclength`], [`tie`], [`contact`], [`harmonic_direct`], [`damped`], [`gyro`],
//! [`rotor`], [`random`], [`fatigue`], [`fracture`], [`cms`], [`estimate`]) are declared by ADR 0440 (F-408), each with
//! its `PENDING` package; the new material variants, `Shell3` and the layup are refused the same way.
//!
//! Conventions (ADR 0270 section 2.2): SI units, global Cartesian axes; unknowns are numbered through
//! [`model::DofMap`]; symmetric tensors are six numbers in the order xx, yy, zz, xy, yz, zx, strains with engineering
//! shear; a surface set's normal points out of the solid; every refusal is an error naming the element, node, set or key.

pub mod arclength;
pub mod assemble;
pub mod beam;
pub mod buckling;
pub mod builders;
pub mod cms;
pub mod constraint;
pub mod contact;
pub mod convert;
pub mod cyclic;
pub mod damped;
pub mod element;
pub mod error;
pub mod estimate;
pub mod fatigue;
pub mod follower;
pub mod fracture;
pub mod free_motions;
pub mod geometric;
pub mod geometric_struct;
pub mod gyro;
pub mod harmonic;
pub mod harmonic_direct;
pub mod hex20;
pub mod hex8x;
pub mod laminate;
pub mod loads;
pub mod mass;
pub mod mat_creep;
pub mod mat_hyper;
pub mod mat_j2;
pub mod mat_ortho;
pub mod mat_table;
pub mod mat_visco;
pub mod material;
pub mod mesh;
pub mod modal;
pub mod model;
pub mod mpc;
pub mod nl_transient;
pub mod nonlinear;
pub mod nonlinear_md;
pub mod quadrature;
pub mod random;
pub mod recover;
pub mod refine;
pub mod rotating;
pub mod rotor;
pub mod shell;
pub mod shell3;
pub mod shell_corot;
pub mod solve;
pub mod thermal;
pub mod tie;
pub mod transient;

pub use error::{FemError, Result, SetKind};
pub use mesh::{BeamSection, BlockProperty, ElementBlock, ElementKind, FaceRef, Ply, SolidFormulation, SolidMesh};
pub use model::{Constraint, DofMap, ElasticSupport, LinearTerm, Load, Material, SolidModel, SolidResult, SolverStats};
