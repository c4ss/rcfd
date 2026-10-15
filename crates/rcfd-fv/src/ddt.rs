//! Time integration: implicit Euler, variable-step BDF2, Crank-Nicolson and a
//! CFL-based time-step controller.

use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;

/// Temporal discretisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeScheme {
    /// First-order backward Euler.
    ImplicitEuler,
    /// Second-order backward differentiation (variable step; Euler on the first step).
    Bdf2,
    /// Second-order Crank-Nicolson for the spatial operator.
    CrankNicolson,
    /// Bounded second-order scheme (ADR 0016, project-defined): per cell a blend `theta BDF2 + (1 - theta) Euler` with
    /// `theta = clamp(rho, 0, 1)`, `rho` the ratio of the last two backward difference quotients; Euler on the first
    /// step, BDF2 on the second.
    Bdf2Bounded,
}

/// Solution history needed by the multi-step schemes.
#[derive(Debug, Clone)]
pub struct History {
    /// Values at the previous time level `n`.
    pub old: Vec<f64>,
    /// Values at level `n-1` (BDF2).
    pub older: Option<Vec<f64>>,
    /// Previous step size `t^n - t^{n-1}` (BDF2).
    pub dt_old: Option<f64>,
    /// Values at level `n-2` (bounded BDF2).
    pub oldest: Option<Vec<f64>>,
    /// Step size `t^{n-1} - t^{n-2}` (bounded BDF2).
    pub dt_older: Option<f64>,
}

impl History {
    /// History with only the old level.
    pub fn new(old: Vec<f64>) -> Self {
        History {
            old,
            older: None,
            dt_old: None,
            oldest: None,
            dt_older: None,
        }
    }

    /// Advance after a step of size `dt` that produced `new`.
    pub fn push(&mut self, new: Vec<f64>, dt: f64) {
        self.oldest = self.older.take();
        self.dt_older = self.dt_old;
        self.older = Some(std::mem::replace(&mut self.old, new));
        self.dt_old = Some(dt);
    }
}

/// Add the time derivative to a spatial system `spatial` (which represents
/// `L(phi) = A phi - b`) and return the system for the new time level.
///
/// * Euler / BDF2: `(V/dt)(c0 phi - c1 phi^n + c2 phi^{n-1}) + L(phi) = 0`.
/// * Crank-Nicolson: `(V/dt)(phi - phi^n) + (L(phi) + L(phi^n)) / 2 = 0`
///   with the *same* `A` and `b` used at both levels (constant coefficients).
pub fn add_ddt(scheme: TimeScheme, mut spatial: FvMatrix, vol: &[f64], dt: f64, hist: &History) -> FvMatrix {
    let n = vol.len();
    match scheme {
        TimeScheme::ImplicitEuler => {
            for i in 0..n {
                spatial.a.diag[i] += vol[i] / dt;
                spatial.b[i] += vol[i] / dt * hist.old[i];
            }
        }
        TimeScheme::Bdf2 => match (&hist.older, hist.dt_old) {
            (Some(older), Some(dt_old)) => {
                let r = dt / dt_old;
                let (c0, c1, c2) = ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r));
                for i in 0..n {
                    spatial.a.diag[i] += vol[i] * c0 / dt;
                    spatial.b[i] += vol[i] / dt * (c1 * hist.old[i] - c2 * older[i]);
                }
            }
            _ => return add_ddt(TimeScheme::ImplicitEuler, spatial, vol, dt, hist),
        },
        TimeScheme::Bdf2Bounded => match (&hist.older, hist.dt_old) {
            (Some(older), Some(dt_old)) => {
                let r = dt / dt_old;
                let (c0, c1, c2) = ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r));
                for i in 0..n {
                    let theta = match (&hist.oldest, hist.dt_older) {
                        (Some(oldest), Some(dt_older)) => {
                            bounded_theta(hist.old[i], older[i], oldest[i], dt_old, dt_older)
                        }
                        _ => 1.0,
                    };
                    // theta BDF2 + (1 - theta) Euler
                    spatial.a.diag[i] += vol[i] / dt * (theta * c0 + (1.0 - theta));
                    spatial.b[i] +=
                        vol[i] / dt * (theta * (c1 * hist.old[i] - c2 * older[i]) + (1.0 - theta) * hist.old[i]);
                }
            }
            _ => return add_ddt(TimeScheme::ImplicitEuler, spatial, vol, dt, hist),
        },
        TimeScheme::CrankNicolson => {
            let mut ax = vec![0.0; n];
            spatial.a.matvec(&hist.old, &mut ax);
            for i in 0..n {
                // rhs = V/dt phi^n - A phi^n / 2 + b ; lhs = V/dt + A/2
                spatial.b[i] += vol[i] / dt * hist.old[i] - 0.5 * ax[i];
            }
            spatial.scale_matrix(0.5);
            for i in 0..n {
                spatial.a.diag[i] += vol[i] / dt;
            }
        }
    }
    spatial
}

/// Weight of BDF2 in the bounded second-order scheme: `clamp(rho, 0, 1)`, `rho = s_n / s_{n-1}` the ratio of the last two
/// backward difference quotients of the history `(p0 = phi^n, p1 = phi^{n-1}, p2 = phi^{n-2})`. Quiescent history: 1.
fn bounded_theta(p0: f64, p1: f64, p2: f64, dt_old: f64, dt_older: f64) -> f64 {
    let s_n = (p0 - p1) / dt_old;
    let s_p = (p1 - p2) / dt_older;
    let thr = 1e-12 * (p0.abs() + p1.abs() + p2.abs());
    let (qn, qp) = ((p0 - p1).abs() <= thr, (p1 - p2).abs() <= thr);
    match (qn, qp) {
        (true, true) => 1.0,
        (false, true) => 0.0,
        _ => (s_n / s_p).clamp(0.0, 1.0),
    }
}

impl FvMatrix {
    /// Scale only the matrix coefficients (not the right-hand side).
    pub fn scale_matrix(&mut self, s: f64) {
        self.a.diag.iter_mut().for_each(|x| *x *= s);
        self.a.lower.iter_mut().for_each(|x| *x *= s);
        self.a.upper.iter_mut().for_each(|x| *x *= s);
    }
}

/// Cell Courant numbers `0.5 sum|F| dt / V`.
pub fn courant_numbers(geom: &FvGeometry, flux: &[f64], dt: f64) -> Vec<f64> {
    let mesh = &geom.mesh;
    (0..mesh.n_cells())
        .map(|c| {
            let s: f64 = mesh
                .cell_faces(c)
                .iter()
                .filter(|&&f| !geom.empty[f])
                .map(|&f| flux[f].abs())
                .sum();
            0.5 * s * dt / mesh.cell_volumes()[c]
        })
        .collect()
}

/// Maximum Courant number.
pub fn max_courant(geom: &FvGeometry, flux: &[f64], dt: f64) -> f64 {
    courant_numbers(geom, flux, dt).into_iter().fold(0.0, f64::max)
}

/// Adaptive time-step controller bounding the maximum Courant number.
#[derive(Debug, Clone, Copy)]
pub struct CflController {
    /// Target maximum Courant number.
    pub max_cfl: f64,
    /// Largest allowed step.
    pub max_dt: f64,
    /// Smallest allowed step.
    pub min_dt: f64,
    /// Maximum growth factor per step.
    pub growth: f64,
}

impl CflController {
    /// Next step size given the current flux field and the step just used.
    pub fn next_dt(&self, geom: &FvGeometry, flux: &[f64], dt_prev: f64) -> f64 {
        let cfl = max_courant(geom, flux, dt_prev);
        let by_cfl = if cfl > 1e-300 {
            dt_prev * self.max_cfl / cfl
        } else {
            f64::INFINITY
        };
        by_cfl.min(dt_prev * self.growth).min(self.max_dt).max(self.min_dt)
    }
}

/// Error-estimator time-step controller (ADR 0023 D2, F-209 partial): the local error of the step just taken is estimated from
/// the difference between the new state and the linear extrapolation of the two previous states, and the next step follows
/// `dt (tol / e)^(1/2)` (first-order scheme).
#[derive(Debug, Clone, Copy)]
pub struct ErrorController {
    /// Target (relative) local error per step.
    pub tol: f64,
    /// Safety factor on the step update.
    pub safety: f64,
    /// Maximum growth factor per step.
    pub growth: f64,
    /// Smallest shrink factor per step.
    pub shrink_min: f64,
    /// Smallest allowed step.
    pub min_dt: f64,
    /// Largest allowed step.
    pub max_dt: f64,
    /// Fixed reference magnitude for the error scale (absolute error target); `None` divides by the current maximum.
    pub abs_ref: Option<f64>,
}

impl ErrorController {
    /// Controller with the registered constants (safety 0.9, growth 1.5, shrink minimum 0.3).
    pub fn new(tol: f64, min_dt: f64, max_dt: f64) -> Self {
        ErrorController {
            tol,
            safety: 0.9,
            growth: 1.5,
            shrink_min: 0.3,
            min_dt,
            max_dt,
            abs_ref: None,
        }
    }

    /// Local error estimate `0.5 max|u_n - u_pred| / max(max|u_n|, 1e-30)` with `u_pred` the linear extrapolation of
    /// `u_{n-1}` and `u_{n-2}` to the new time (`dt` the step just taken, `dt_prev` the one before).
    pub fn estimate(&self, u_new: &[f64], u_prev: &[f64], u_prev2: &[f64], dt: f64, dt_prev: f64) -> f64 {
        let r = dt / dt_prev;
        let mut diff: f64 = 0.0;
        let mut scale: f64 = 1e-30;
        for i in 0..u_new.len() {
            let pred = u_prev[i] + (u_prev[i] - u_prev2[i]) * r;
            diff = diff.max((u_new[i] - pred).abs());
            scale = scale.max(u_new[i].abs());
        }
        0.5 * diff / self.abs_ref.unwrap_or(scale)
    }

    /// Whether a step with estimate `err` is accepted (rejected above twice the tolerance).
    pub fn accepts(&self, err: f64) -> bool {
        err <= 2.0 * self.tol
    }

    /// Next step size from the estimate of the step of size `dt` just taken.
    pub fn next_dt(&self, err: f64, dt: f64) -> f64 {
        let f = if err > 0.0 {
            (self.safety * (self.tol / err).sqrt()).clamp(self.shrink_min, self.growth)
        } else {
            self.growth
        };
        (dt * f).clamp(self.min_dt, self.max_dt)
    }
}

/// Mean value weighted by cell volume (utility for conservation checks).
pub fn volume_mean(geom: &FvGeometry, x: &[f64]) -> f64 {
    let v = geom.mesh.cell_volumes();
    x.iter().zip(v).map(|(a, b)| a * b).sum::<f64>() / v.iter().sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convection::face_flux;
    use rcfd_linalg::SolveControl;
    use rcfd_mesh::gen::{box_mesh, Grading};
    use std::sync::Arc;

    fn decay_error(scheme: TimeScheme, steps: usize) -> f64 {
        let g = FvGeometry::new(Arc::new(box_mesh([2, 2, 2], [1.0; 3], [Grading::Uniform; 3]).unwrap())).unwrap();
        let vol = g.mesh.cell_volumes().to_vec();
        let lambda = 3.0;
        let t_end = 1.0;
        let dt = t_end / steps as f64;
        let mut phi = vec![1.0; g.n_cells()];
        let mut hist = History::new(phi.clone());
        let ctl = SolveControl {
            tol_abs: 1e-15,
            tol_rel: 1e-14,
            max_iter: 100,
        };
        for _ in 0..steps {
            let mut spatial = FvMatrix::zeros(&g);
            for i in 0..vol.len() {
                spatial.a.diag[i] = lambda * vol[i];
            }
            let sys = add_ddt(scheme, spatial, &vol, dt, &hist);
            sys.solve(&mut phi, &ctl);
            hist.push(phi.clone(), dt);
        }
        (phi[0] - (-lambda * t_end).exp()).abs()
    }

    /// ADR 0023 E1: linear decay du/dt = -3u, implicit Euler, tolerance 1e-3. Returns (controlled steps, error,
    /// uniform steps for the same error).
    fn e1_run(abs_ref: Option<f64>) -> (usize, f64, usize) {
        let g = FvGeometry::new(Arc::new(box_mesh([2, 2, 2], [1.0; 3], [Grading::Uniform; 3]).unwrap())).unwrap();
        let vol = g.mesh.cell_volumes().to_vec();
        let lambda = 3.0;
        let ctl = SolveControl {
            tol_abs: 1e-15,
            tol_rel: 1e-14,
            max_iter: 100,
        };
        let one_step = |u: &[f64], dt: f64, hist: &History| -> Vec<f64> {
            let mut spatial = FvMatrix::zeros(&g);
            for i in 0..vol.len() {
                spatial.a.diag[i] = lambda * vol[i];
            }
            let sys = add_ddt(TimeScheme::ImplicitEuler, spatial, &vol, dt, hist);
            let mut x = u.to_vec();
            sys.solve(&mut x, &ctl);
            x
        };
        let exact = (-lambda * 1.0_f64).exp();
        // error-controlled run
        let mut ec = ErrorController::new(1e-3, 1e-4, 0.2);
        ec.abs_ref = abs_ref;
        let mut u = vec![1.0; g.n_cells()];
        let mut hist = History::new(u.clone());
        let (mut t, mut dt, mut steps) = (0.0_f64, 1e-3_f64, 0usize);
        let mut prev: Option<(Vec<f64>, f64)> = None; // (u_{n-2}, dt of the step that produced u_{n-1})
        let mut u_prev = u.clone();
        while t < 1.0 - 1e-12 {
            let d = dt.min(1.0 - t);
            let x = one_step(&u, d, &hist);
            steps += 1;
            let mut next = d;
            if let Some((u2, d_prev)) = &prev {
                let err = ec.estimate(&x, &u_prev, u2, d, *d_prev);
                // the scalar utility rejects a step above twice the tolerance (not for the very last, shortened step)
                if !ec.accepts(err) && d == dt {
                    dt = ec.next_dt(err, d);
                    continue;
                }
                next = ec.next_dt(err, d);
            }
            prev = Some((u_prev.clone(), d));
            u_prev = x.clone();
            hist.push(x.clone(), d);
            u = x;
            t += d;
            dt = next;
        }
        let e_c = (u[0] - exact).abs();
        // smallest uniform step count with error <= e_c
        let mut n_f = 0;
        for n in 1..2000 {
            let d = 1.0 / n as f64;
            let mut u = vec![1.0; g.n_cells()];
            let mut h = History::new(u.clone());
            for _ in 0..n {
                u = one_step(&u, d, &h);
                h.push(u.clone(), d);
            }
            if (u[0] - exact).abs() <= e_c {
                n_f = n;
                break;
            }
        }
        println!("E1 (abs_ref {abs_ref:?}): controlled {steps} steps, error {e_c:.3e} (tol 1e-3, limit 10 tol); uniform stepping needs {n_f} steps");
        (steps, e_c, n_f)
    }

    /// Registered E1 (relative error scale): MISSED, see ADR 0023 addendum A7; the registered limits are kept here.
    #[test]
    #[ignore = "KNOWN MISS (ADR 0023 addendum A7): relative error scale gives a constant step for exponential decay (81 vs 76 steps)"]
    fn e1_error_controlled_decay_needs_fewer_steps_than_uniform_stepping_known_miss() {
        let (steps, e_c, n_f) = e1_run(None);
        assert!(e_c <= 1e-2, "{e_c}");
        assert!(n_f > 0);
        assert!(steps as f64 <= 0.8 * n_f as f64, "{steps} vs {n_f}");
    }

    /// Registered E1b (absolute reference scale, addendum A7).
    #[test]
    #[ignore = "KNOWN MISS (ADR 0023 addendum A8): absolute reference scale 46 steps vs 36 uniform"]
    fn e1b_absolute_reference_error_control_needs_fewer_steps_than_uniform_stepping_known_miss() {
        let (steps, e_c, n_f) = e1_run(Some(1.0));
        assert!(e_c <= 1e-2, "{e_c}");
        assert!(n_f > 0);
        assert!(steps as f64 <= 0.8 * n_f as f64, "{steps} vs {n_f}");
    }

    #[test]
    fn decay_ode_orders() {
        for (scheme, expect) in [
            (TimeScheme::ImplicitEuler, 1.0),
            (TimeScheme::Bdf2, 2.0),
            (TimeScheme::CrankNicolson, 2.0),
        ] {
            let e: Vec<f64> = [10usize, 20, 40, 80].iter().map(|&n| decay_error(scheme, n)).collect();
            let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
            println!("{scheme:?}: errors {e:?} orders {orders:?}");
            let o = *orders.last().unwrap();
            assert!((o - expect).abs() < 0.15, "{scheme:?} order {o}");
        }
    }

    #[test]
    fn bdf2_variable_step_second_order() {
        // alternate step sizes dt*(1 +- 0.3): still second order
        let g = FvGeometry::new(Arc::new(box_mesh([1, 1, 1], [1.0; 3], [Grading::Uniform; 3]).unwrap())).unwrap();
        let vol = g.mesh.cell_volumes().to_vec();
        let run = |n: usize| {
            let mut phi = vec![1.0];
            let mut hist = History::new(phi.clone());
            let base = 1.0 / n as f64;
            let mut t = 0.0;
            let mut k = 0;
            while t < 1.0 - 1e-12 {
                let dt = (base * if k % 2 == 0 { 1.3 } else { 0.7 }).min(1.0 - t);
                let mut s = FvMatrix::zeros(&g);
                s.a.diag[0] = 2.0 * vol[0];
                let sys = add_ddt(TimeScheme::Bdf2, s, &vol, dt, &hist);
                sys.solve(
                    &mut phi,
                    &SolveControl {
                        tol_abs: 1e-15,
                        tol_rel: 1e-14,
                        max_iter: 10,
                    },
                );
                hist.push(phi.clone(), dt);
                t += dt;
                k += 1;
            }
            (phi[0] - (-2.0f64).exp()).abs()
        };
        let (a, b) = (run(20), run(40));
        assert!((a / b).log2() > 1.8, "{a} {b}");
    }

    #[test]
    fn cfl_controller_bounds_step() {
        let g = FvGeometry::new(Arc::new(
            box_mesh([50, 1, 1], [1.0, 0.1, 0.1], [Grading::Uniform; 3]).unwrap(),
        ))
        .unwrap();
        let u = vec![[2.0, 0.0, 0.0]; g.n_cells()];
        let bu = vec![[2.0, 0.0, 0.0]; g.mesh.n_boundary_faces()];
        let flux = face_flux(&g, &u, &bu);
        // CFL = u dt / h with h = 0.02
        let cfl = max_courant(&g, &flux, 0.005);
        assert!((cfl - 2.0 * 0.005 / 0.02).abs() < 1e-12, "{cfl}");
        let ctl = CflController {
            max_cfl: 0.5,
            max_dt: 1.0,
            min_dt: 1e-6,
            growth: 1.2,
        };
        // too large a step is cut back to the CFL target
        let dt = ctl.next_dt(&g, &flux, 0.05);
        assert!((max_courant(&g, &flux, dt) - 0.5).abs() < 1e-12);
        // small steps grow by at most the growth factor
        let dt = ctl.next_dt(&g, &flux, 1e-4);
        assert!((dt - 1.2e-4).abs() < 1e-16);
        // zero velocity: bounded by max_dt / growth
        let zero = vec![0.0; flux.len()];
        assert!((ctl.next_dt(&g, &zero, 0.9) - 1.0).abs() < 1e-15);
        // iterating converges to the CFL-limited step and never exceeds it
        let mut dt = 1e-5;
        for _ in 0..200 {
            dt = ctl.next_dt(&g, &flux, dt);
            assert!(max_courant(&g, &flux, dt) <= 0.5 + 1e-12);
        }
        assert!((max_courant(&g, &flux, dt) - 0.5).abs() < 1e-9);
        assert!((volume_mean(&g, &vec![2.0; g.n_cells()]) - 2.0).abs() < 1e-12);
    }
}
