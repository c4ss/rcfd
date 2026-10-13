//! Sparse matrices (face-addressed LDU and CSR), deterministic parallel
//! vector kernels, preconditioners and Krylov solvers.
//!
//! Determinism: every reduction splits the data into fixed-size chunks
//! (independent of the thread count), computes chunk partial sums in
//! parallel and combines them sequentially in chunk order. Results are
//! therefore bit-identical for any number of threads.

pub mod amg;
pub mod matrix;
pub mod precond;
pub mod solvers;
pub mod vecops;

pub use amg::{Amg, AmgSettings};
pub use matrix::{CsrMatrix, LduAddressing, LduMatrix};
pub use precond::{Ic0, Identity, Ilu0, Jacobi, Preconditioner};
pub use solvers::{bicgstab, cg, gmres, LinearOperator, SolveControl, SolveStats};
