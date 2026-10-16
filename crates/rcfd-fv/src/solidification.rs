//! Enthalpy-porosity melting and solidification (F-212, ADR 0035 D3).
//!
//! Fixed-grid enthalpy method: `h(T) = T + (L/c) f(T)` (K), `f` the linear liquid fraction between the solidus and the
//! liquidus, equal phase properties. One implicit-Euler step solves `V (h(T) - h^n)/dt + A_op T = b_op + V S` for the
//! cell temperatures by Newton's method on the piecewise-linear monotone `h` (apparent capacity), with backtracking on the
//! 2-norm of the true nonlinear residual. At convergence every cell satisfies the discrete enthalpy balance, so the global
//! balance is an invariant of the scheme. The Carman-Kozeny momentum damping is provided as a pure function.

use crate::diffusion::laplacian;
use crate::field::ScalarField;
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::least_squares;
use rcfd_linalg::SolveControl;

/// Linear liquid-fraction law between the solidus and the liquidus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseLaw {
    /// Solidus temperature (K): below it the material is solid.
    pub solidus: f64,
    /// Liquidus temperature (K): above it the material is liquid.
    pub liquidus: f64,
    /// Latent heat over specific heat `L/c` (K).
    pub latent_k: f64,
}

impl PhaseLaw {
    /// Liquid fraction in [0, 1].
    pub fn liquid_fraction(&self, t: f64) -> f64 {
        ((t - self.solidus) / (self.liquidus - self.solidus)).clamp(0.0, 1.0)
    }

    /// Specific enthalpy over `c` (K): `T + (L/c) f(T)`.
    pub fn enthalpy(&self, t: f64) -> f64 {
        t + self.latent_k * self.liquid_fraction(t)
    }

    /// Apparent capacity `dh/dT` (1 outside the mushy interval, `1 + (L/c)/(T_l - T_s)` inside).
    pub fn apparent_capacity(&self, t: f64) -> f64 {
        if t > self.solidus && t < self.liquidus {
            1.0 + self.latent_k / (self.liquidus - self.solidus)
        } else {
            1.0
        }
    }
}

/// Carman-Kozeny damping coefficient `A(f) = C (1 - f)^2 / (f^3 + b)` [kg/m^3/s] (Voller and Prakash type): `C` the mushy
/// constant, `b` the small regularisation that keeps `A` finite at `f = 0`.
pub fn carman_kozeny(f: f64, c: f64, b: f64) -> f64 {
    c * (1.0 - f) * (1.0 - f) / (f * f * f + b)
}

/// Implicit momentum sink `A(f)/rho` [1/s] per cell for the liquid fractions `f`.
pub fn momentum_sink(f: &[f64], c: f64, b: f64, rho: f64) -> Vec<f64> {
    f.iter().map(|&x| carman_kozeny(x, c, b) / rho).collect()
}

/// Outcome of one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepStats {
    /// Newton iterations used.
    pub iterations: usize,
    /// Converged within the iteration cap.
    pub converged: bool,
    /// Final residual relative to the sum of absolute terms.
    pub residual: f64,
}

/// Enthalpy-method state: the law, the diffusivity at the faces and the enthalpy of the old time level.
pub struct PhaseChange {
    /// Phase law.
    pub law: PhaseLaw,
    /// Diffusivity `k/(rho c)` at every face (m^2/s).
    pub gamma: Vec<f64>,
    /// Specific enthalpy over `c` (K) of the old level, per cell.
    pub h_old: Vec<f64>,
    /// Newton iteration cap per step.
    pub newton_max: usize,
    /// Linear-solver controls.
    pub solver: SolveControl,
    /// Largest Newton iteration count seen so far.
    pub max_iterations: usize,
}

impl PhaseChange {
    /// New state for the temperatures `t` (the enthalpy of the old level is computed from them).
    pub fn new(law: PhaseLaw, gamma: Vec<f64>, t: &[f64]) -> Self {
        PhaseChange {
            law,
            gamma,
            h_old: t.iter().map(|&x| law.enthalpy(x)).collect(),
            newton_max: 100,
            solver: SolveControl {
                tol_abs: 1e-300,
                tol_rel: 1e-13,
                max_iter: 5000,
            },
            max_iterations: 0,
        }
    }

    /// Liquid fraction per cell.
    pub fn liquid_fraction(&self, t: &[f64]) -> Vec<f64> {
        t.iter().map(|&x| self.law.liquid_fraction(x)).collect()
    }

    /// Operator matrix `A_op` and right-hand side `b_op` (diffusion with the field's boundary conditions, explicit
    /// non-orthogonal correction from the least-squares gradient of `cells`).
    fn operator(&self, geom: &FvGeometry, field: &ScalarField, cells: &[f64]) -> FvMatrix {
        let bc = field.boundary_coeffs(geom);
        let bvals = field.boundary_values(geom);
        let grad = least_squares(geom, cells, &bvals);
        laplacian(geom, &self.gamma, &bc, Some(&grad))
    }

    /// Nonlinear residual `V (h(T) - h^n)/dt + A_op T - b_op - V S` per cell and the sum of the absolute terms.
    fn residual(
        &self,
        geom: &FvGeometry,
        field: &ScalarField,
        cells: &[f64],
        dt: f64,
        source: Option<&[f64]>,
    ) -> (Vec<f64>, f64) {
        let vol = geom.mesh.cell_volumes();
        // field boundary values depend on the cell values only through the coefficients; the matrix is rebuilt for `cells`
        let m = self.operator(geom, field, cells);
        let mut ax = vec![0.0; cells.len()];
        m.a.matvec(cells, &mut ax);
        let mut r = vec![0.0; cells.len()];
        let mut scale = 0.0;
        for c in 0..cells.len() {
            let dh = vol[c] * (self.law.enthalpy(cells[c]) - self.h_old[c]) / dt;
            let s = source.map_or(0.0, |s| vol[c] * s[c]);
            r[c] = dh + ax[c] - m.b[c] - s;
            scale += dh.abs() + ax[c].abs() + m.b[c].abs() + s.abs();
        }
        (r, scale)
    }

    /// Advance the temperature field by one implicit-Euler step `dt` with the volumetric source `source` (K/s).
    /// The field's boundary conditions must be set. Updates the old-level enthalpy on exit.
    pub fn advance(
        &mut self,
        geom: &FvGeometry,
        field: &mut ScalarField,
        dt: f64,
        source: Option<&[f64]>,
    ) -> StepStats {
        let vol = geom.mesh.cell_volumes();
        let n = field.values.len();
        let norm2 = |r: &[f64]| r.iter().map(|x| x * x).sum::<f64>().sqrt();
        let mut cells = field.values.clone();
        let (mut r, mut scale) = self.residual(geom, field, &cells, dt, source);
        let mut stats = StepStats {
            iterations: 0,
            converged: false,
            residual: 0.0,
        };
        for it in 0..=self.newton_max {
            let rmax = r.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
            stats.residual = rmax / scale.max(1e-300);
            if stats.residual <= 1e-12 {
                stats.converged = true;
                stats.iterations = it;
                break;
            }
            if it == self.newton_max {
                stats.iterations = it;
                break;
            }
            // Newton step through the linearised matrix
            let mut m = self.operator(geom, field, &cells);
            for c in 0..n {
                let cap = self.law.apparent_capacity(cells[c]) * vol[c] / dt;
                m.a.diag[c] += cap;
                let s = source.map_or(0.0, |s| vol[c] * s[c]);
                m.b[c] += s + cap * cells[c] - vol[c] * (self.law.enthalpy(cells[c]) - self.h_old[c]) / dt;
            }
            let mut tn = cells.clone();
            m.solve(&mut tn, &self.solver);
            // backtracking on the 2-norm of the true residual
            let r0 = norm2(&r);
            let mut s = 1.0;
            let mut accepted = None;
            for _ in 0..=8 {
                let cand: Vec<f64> = cells.iter().zip(&tn).map(|(a, b)| a + s * (b - a)).collect();
                let (rc, sc) = self.residual(geom, field, &cand, dt, source);
                if norm2(&rc) < (1.0 - 1e-4 * s) * r0 || s < 1.0 / 256.0 {
                    accepted = Some((cand, rc, sc));
                    break;
                }
                s *= 0.5;
            }
            let (cand, rc, sc) = accepted.unwrap_or_else(|| {
                let (rc, sc) = self.residual(geom, field, &tn, dt, source);
                (tn.clone(), rc, sc)
            });
            cells = cand;
            r = rc;
            scale = sc;
        }
        self.max_iterations = self.max_iterations.max(stats.iterations);
        field.values = cells;
        self.h_old = field.values.iter().map(|&x| self.law.enthalpy(x)).collect();
        stats
    }
}
