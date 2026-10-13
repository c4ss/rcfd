//! Workspace-wide error type.

use crate::units::Dimension;
use thiserror::Error;

/// Convenient result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by rcfd crates.
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// A unit string could not be parsed.
    #[error("unit parse error: {0}")]
    UnitParse(String),
    /// Two quantities (or a quantity and a requirement) have different dimensions.
    #[error("dimension mismatch: expected {expected}, found {found}")]
    DimensionMismatch {
        /// Dimension that was required.
        expected: Dimension,
        /// Dimension that was supplied.
        found: Dimension,
    },
    /// A settings path does not exist.
    #[error("settings path not found: {0}")]
    PathNotFound(String),
    /// A settings value failed validation.
    #[error("invalid value at '{path}': {reason}")]
    InvalidValue {
        /// Dotted path of the offending setting.
        path: String,
        /// Human readable reason.
        reason: String,
    },
    /// Mesh invariants are violated or mesh input is malformed.
    #[error("mesh error: {0}")]
    Mesh(String),
    /// File format or IO problem.
    #[error("io error: {0}")]
    Io(String),
    /// Numerical failure (divergence, singular matrix, ...).
    #[error("numerical error: {0}")]
    Numerical(String),
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}
