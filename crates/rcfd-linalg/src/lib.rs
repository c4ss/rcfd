//! Sparse matrices (face-addressed LDU and CSR), deterministic parallel
//! vector kernels, preconditioners and Krylov solvers.
//!
//! Determinism: every reduction splits the data into fixed-size chunks
//! (independent of the thread count), computes chunk partial sums in
//! parallel and combines them sequentially in chunk order. Results are
//! therefore bit-identical for any number of threads.

pub mod amg;
pub mod backend;
pub mod matrix;
pub mod precond;
pub mod reorder;
pub mod solvers;
pub mod vecops;

pub use amg::{Amg, AmgSettings};
pub use backend::{
    check_index_range, check_system, cpu_preconditioner, BackendError, CpuBackend, GpuPrecond, LinearBackend,
    LinearDevice, LinearOptions, DEFAULT_CHEBYSHEV_DEGREE,
};
pub use matrix::{CsrMatrix, LduAddressing, LduMatrix};
pub use precond::{Chebyshev, ChebyshevCoeffs, Ic0, Identity, Ilu0, Jacobi, Preconditioner};
pub use reorder::{bandwidth, inverse_permutation, rcm_order};
pub use solvers::{bicgstab, cg, gmres, LinearOperator, SolveControl, SolveStats};
