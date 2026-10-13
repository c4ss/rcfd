//! Finite-volume matrix: an LDU matrix plus right-hand side, representing
//! the discretised system `A phi = b`.

use crate::geometry::FvGeometry;
use rcfd_linalg::{bicgstab, cg, Amg, AmgSettings, Ic0, Ilu0, LduMatrix, SolveControl, SolveStats};

/// Discretised linear system for one cell-centred unknown.
#[derive(Debug, Clone)]
pub struct FvMatrix {
    /// Coefficient matrix.
    pub a: LduMatrix,
    /// Right-hand side.
    pub b: Vec<f64>,
}

impl FvMatrix {
    /// Zero system on the mesh addressing.
    pub fn zeros(geom: &FvGeometry) -> Self {
        FvMatrix {
            a: LduMatrix::zeros(geom.addressing.clone()),
            b: vec![0.0; geom.n_cells()],
        }
    }

    /// Add another system on the same addressing.
    pub fn add(&mut self, other: &FvMatrix) {
        for (x, y) in self.a.diag.iter_mut().zip(&other.a.diag) {
            *x += y;
        }
        for (x, y) in self.a.lower.iter_mut().zip(&other.a.lower) {
            *x += y;
        }
        for (x, y) in self.a.upper.iter_mut().zip(&other.a.upper) {
            *x += y;
        }
        for (x, y) in self.b.iter_mut().zip(&other.b) {
            *x += y;
        }
    }

    /// Multiply the whole system by a constant.
    pub fn scale(&mut self, s: f64) {
        self.a.diag.iter_mut().for_each(|x| *x *= s);
        self.a.lower.iter_mut().for_each(|x| *x *= s);
        self.a.upper.iter_mut().for_each(|x| *x *= s);
        self.b.iter_mut().for_each(|x| *x *= s);
    }

    /// Implicit under-relaxation with factor `alpha` about `x_old`.
    pub fn relax(&mut self, alpha: f64, x_old: &[f64]) {
        self.a.relax(alpha, x_old, &mut self.b);
    }

    /// Residual vector `b - A x`.
    pub fn residual(&self, x: &[f64]) -> Vec<f64> {
        let mut r = vec![0.0; x.len()];
        self.a.matvec(x, &mut r);
        r.iter_mut().zip(&self.b).for_each(|(ri, bi)| *ri = bi - *ri);
        r
    }

    /// Solve in place. Symmetric systems use CG with DIC; others BiCGStab
    /// with ILU(0).
    pub fn solve(&self, x: &mut [f64], ctl: &SolveControl) -> SolveStats {
        if self.a.is_symmetric() {
            cg(&self.a, &self.b, x, &Ic0::new(&self.a), ctl)
        } else {
            let csr = self.a.to_csr();
            bicgstab(&csr, &self.b, x, &Ilu0::new(&csr), ctl)
        }
    }

    /// Solve a symmetric positive definite system with CG preconditioned by
    /// one aggregation-AMG V-cycle (built from the current coefficients).
    pub fn solve_amg(&self, x: &mut [f64], ctl: &SolveControl) -> SolveStats {
        let amg = Amg::new(&self.a.to_csr(), AmgSettings::default());
        cg(&self.a, &self.b, x, &amg, ctl)
    }
}
