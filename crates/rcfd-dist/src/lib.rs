//! Transport-agnostic distributed-memory layer (F-100, ADR 0058).
//!
//! * [`comm`]: the [`Communicator`] trait (point-to-point primitives; halo exchange and deterministic global reductions are
//!   built on them), the serial fallback [`SerialComm`] and the in-process multi-rank transport [`ThreadComm`] /
//!   [`run_ranks`]. An MPI or socket backend implements the same trait; it is NOT part of this crate (a later user decision).
//! * [`halo`]: [`HaloPlan`], the halo exchange over `rcfd_mesh::partition::Decomposition`.
//! * [`validate`]: located consistency errors for a decomposition.
//! * [`matrix`]: [`DistCsr`] (owned rows, local numbering), [`DistOperator`] (a `LinearOperator`), [`BlockJacobi`]
//!   (a `Preconditioner`).
//! * [`krylov`]: distributed CG and BiCGStab with deterministic global reductions.
//! * [`solve`]: in-process driver with the serial fallback.

pub mod comm;
pub mod halo;
pub mod krylov;
pub mod matrix;
pub mod solve;
pub mod validate;

pub use comm::{run_ranks, run_ranks_with_timeout, CommError, Communicator, SerialComm, ThreadComm};
pub use halo::HaloPlan;
pub use matrix::{BlockJacobi, DistCsr, DistOperator};
pub use solve::{solve_distributed, solve_rank, DistSolveResult, Method, Precond};
pub use validate::validate_decomposition;
