//! Backend-neutral boundary for sparse linear solves (ADR 0038). The CPU path elsewhere in this crate is the default and
//! is not routed through this module; it exists so that an accelerator crate (`rcfd-gpu`) can implement the same
//! interface and so that the CPU twin of a device preconditioner can be used as the reference.

use crate::matrix::CsrMatrix;
use crate::precond::{Chebyshev, Identity, Jacobi, Preconditioner};
use crate::solvers::{bicgstab, cg, SolveControl, SolveStats};
use std::fmt;

/// Where a linear solve runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinearDevice {
    /// The unchanged CPU path (default).
    #[default]
    Cpu,
    /// The CUDA device; an error if unavailable.
    Cuda,
    /// The device when it is available and the system is large enough, otherwise the CPU path.
    Auto,
}

impl LinearDevice {
    /// Parse `cpu | cuda | auto`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cpu" => Some(LinearDevice::Cpu),
            "cuda" => Some(LinearDevice::Cuda),
            "auto" => Some(LinearDevice::Auto),
            _ => None,
        }
    }

    /// Setting text.
    pub fn as_str(self) -> &'static str {
        match self {
            LinearDevice::Cpu => "cpu",
            LinearDevice::Cuda => "cuda",
            LinearDevice::Auto => "auto",
        }
    }
}

/// Preconditioners available on both the host (as a twin) and the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuPrecond {
    /// No preconditioning (API only; the case schema offers `jacobi` and `chebyshev`).
    None,
    /// Inverse diagonal.
    #[default]
    Jacobi,
    /// Chebyshev polynomial of the Jacobi-scaled operator.
    Chebyshev,
}

/// Default number of Chebyshev terms when the setting is unset (0).
pub const DEFAULT_CHEBYSHEV_DEGREE: usize = 4;

/// Options of a backend solve.
#[derive(Debug, Clone, Copy, Default)]
pub struct LinearOptions {
    /// Requested device.
    pub device: LinearDevice,
    /// Preconditioner used by the device solvers.
    pub precond: GpuPrecond,
    /// Chebyshev terms, 0 = default ([`DEFAULT_CHEBYSHEV_DEGREE`]).
    pub chebyshev_degree: usize,
}

impl LinearOptions {
    /// Effective Chebyshev degree.
    pub fn degree(&self) -> usize {
        if self.chebyshev_degree == 0 {
            DEFAULT_CHEBYSHEV_DEGREE
        } else {
            self.chebyshev_degree
        }
    }
}

/// Located error of a backend (message names the step and the quantities involved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    /// Human-readable, located message.
    pub message: String,
}

impl BackendError {
    /// New error.
    pub fn new(message: impl Into<String>) -> Self {
        BackendError {
            message: message.into(),
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BackendError {}

/// A linear-solver backend operating on CSR systems.
pub trait LinearBackend: Send + Sync {
    /// Short name (`cpu`, `cuda`).
    fn name(&self) -> &str;
    /// Preconditioned CG for SPD systems.
    fn cg(
        &self,
        a: &CsrMatrix,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
        o: &LinearOptions,
    ) -> Result<SolveStats, BackendError>;
    /// Preconditioned BiCGStab for general systems.
    fn bicgstab(
        &self,
        a: &CsrMatrix,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
        o: &LinearOptions,
    ) -> Result<SolveStats, BackendError>;
}

/// Check the shapes of a system `A x = b`.
pub fn check_system(a: &CsrMatrix, b: &[f64], x: &[f64]) -> Result<(), BackendError> {
    if a.n != a.ncols {
        return Err(BackendError::new(format!(
            "matrix must be square, got {} rows x {} columns",
            a.n, a.ncols
        )));
    }
    if b.len() != a.n {
        return Err(BackendError::new(format!(
            "right-hand side b has length {} but the matrix has {} rows",
            b.len(),
            a.n
        )));
    }
    if x.len() != a.n {
        return Err(BackendError::new(format!(
            "solution vector x has length {} but the matrix has {} rows",
            x.len(),
            a.n
        )));
    }
    if a.row_ptr.len() != a.n + 1 {
        return Err(BackendError::new(format!(
            "row_ptr has length {} but {} was expected",
            a.row_ptr.len(),
            a.n + 1
        )));
    }
    Ok(())
}

/// Check that a system fits the 32-bit indices of the device kernels (pure function of the counts).
pub fn check_index_range(n_rows: usize, nnz: usize) -> Result<(), BackendError> {
    let max = i32::MAX as usize;
    if n_rows > max {
        return Err(BackendError::new(format!(
            "row count {n_rows} exceeds the 32-bit index limit {max} of the device kernels"
        )));
    }
    if nnz > max {
        return Err(BackendError::new(format!(
            "nonzero count {nnz} exceeds the 32-bit index limit {max} of the device kernels"
        )));
    }
    Ok(())
}

/// The CPU twin of the device preconditioner selected by `o`.
pub fn cpu_preconditioner(a: &CsrMatrix, o: &LinearOptions) -> Result<Box<dyn Preconditioner>, BackendError> {
    match o.precond {
        GpuPrecond::None => Ok(Box::new(Identity)),
        GpuPrecond::Jacobi => Ok(Box::new(Jacobi::new(&a.diag()))),
        GpuPrecond::Chebyshev => Chebyshev::new(a, o.degree())
            .map(|c| Box::new(c) as Box<dyn Preconditioner>)
            .ok_or_else(|| {
                BackendError::new(
                    "Chebyshev preconditioner: no finite positive spectral bound (empty matrix or zero rows)",
                )
            }),
    }
}

/// CPU reference backend: the existing host solvers with the CPU twin of the device preconditioner.
pub struct CpuBackend;

impl LinearBackend for CpuBackend {
    fn name(&self) -> &str {
        "cpu"
    }

    fn cg(
        &self,
        a: &CsrMatrix,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
        o: &LinearOptions,
    ) -> Result<SolveStats, BackendError> {
        check_system(a, b, x)?;
        let m = cpu_preconditioner(a, o)?;
        Ok(cg(a, b, x, m.as_ref(), c))
    }

    fn bicgstab(
        &self,
        a: &CsrMatrix,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
        o: &LinearOptions,
    ) -> Result<SolveStats, BackendError> {
        check_system(a, b, x)?;
        let m = cpu_preconditioner(a, o)?;
        Ok(bicgstab(a, b, x, m.as_ref(), c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_parse_roundtrip() {
        for d in [LinearDevice::Cpu, LinearDevice::Cuda, LinearDevice::Auto] {
            assert_eq!(LinearDevice::parse(d.as_str()), Some(d));
        }
        assert_eq!(LinearDevice::parse("gpu"), None);
        assert_eq!(LinearDevice::default(), LinearDevice::Cpu);
    }

    #[test]
    fn index_range_is_checked_without_allocation() {
        assert!(check_index_range(10, 100).is_ok());
        let e = check_index_range(10, i32::MAX as usize + 1).unwrap_err();
        assert!(e.message.contains("nonzero count"), "{e}");
        let e = check_index_range(i32::MAX as usize + 1, 5).unwrap_err();
        assert!(e.message.contains("row count"), "{e}");
    }

    #[test]
    fn cpu_backend_checks_shapes_and_solves() {
        let a = CsrMatrix::from_triplets(2, vec![(0, 0, 4.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)]);
        let c = SolveControl::default();
        let o = LinearOptions::default();
        let mut x = vec![0.0; 2];
        let e = CpuBackend.cg(&a, &[1.0], &mut x, &c, &o).unwrap_err();
        assert!(e.message.contains("right-hand side"), "{e}");
        let st = CpuBackend.cg(&a, &[1.0, 2.0], &mut x, &c, &o).unwrap();
        assert!(st.converged);
        assert!((4.0 * x[0] + x[1] - 1.0).abs() < 1e-6);
        let o = LinearOptions {
            precond: GpuPrecond::Chebyshev,
            ..LinearOptions::default()
        };
        let mut x = vec![0.0; 2];
        assert!(CpuBackend.bicgstab(&a, &[1.0, 2.0], &mut x, &c, &o).unwrap().converged);
    }
}
