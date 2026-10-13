//! Core types shared by every rcfd crate: scalar precision, units and
//! quantities, the error type, lightweight logging and the settings tree.
//!
//! Clean-room note: everything here is derived from standard references
//! (SI brochure for units, JSON Schema for schema export).

pub mod error;
pub mod expr;
pub mod hash;
pub mod log;
pub mod scalar;
pub mod settings;
pub mod table;
pub mod units;
pub mod vec3;

pub use error::{Error, Result};
pub use scalar::{Real, Scalar};
pub use units::{Dimension, Quantity};
