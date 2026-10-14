//! Stiff ODE integrator: 3-stage Radau IIA (order 5) with simplified Newton iteration, forward-difference Jacobian,
//! dense LU, the embedded error estimate and step-size control of Hairer and Wanner (Solving ODEs II, section IV.8).
//! Written for this repository. An earlier extrapolated linearly implicit Euler version needed about 70000 steps on a
//! hydrogen ignition and was discarded (ADR 0013, tuning disclosure).

use std::fmt;

/// Integrator options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Relative tolerance.
    pub rtol: f64,
    /// Absolute tolerance per component (a single value is broadcast).
    pub atol: Vec<f64>,
    /// First step; 0 chooses one from the derivative.
    pub h0: f64,
    /// Largest step.
    pub h_max: f64,
    /// Maximum number of accepted steps.
    pub max_steps: usize,
}

impl Options {
    /// Options with a scalar absolute tolerance.
    pub fn new(rtol: f64, atol: f64, n: usize) -> Options {
        Options {
            rtol,
            atol: vec![atol; n],
            h0: 0.0,
            h_max: f64::INFINITY,
            max_steps: 1_000_000,
        }
    }
}

/// One accepted step with the derivative at both ends (for Hermite dense output).
#[derive(Debug, Clone)]
pub struct Step<'a> {
    /// Start time.
    pub t0: f64,
    /// End time.
    pub t1: f64,
    /// State at the start.
    pub y0: &'a [f64],
    /// State at the end.
    pub y1: &'a [f64],
    /// Derivative at the start.
    pub f0: &'a [f64],
    /// Derivative at the end.
    pub f1: &'a [f64],
}

impl Step<'_> {
    /// Cubic Hermite interpolation of component `i` at time `t` in `[t0, t1]`.
    pub fn hermite(&self, i: usize, t: f64) -> f64 {
        let h = self.t1 - self.t0;
        let s = (t - self.t0) / h;
        let (s2, s3) = (s * s, s * s * s);
        (2.0 * s3 - 3.0 * s2 + 1.0) * self.y0[i]
            + (s3 - 2.0 * s2 + s) * h * self.f0[i]
            + (-2.0 * s3 + 3.0 * s2) * self.y1[i]
            + (s3 - s2) * h * self.f1[i]
    }
}

/// What the observer wants to do after a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Flow {
    /// Keep integrating.
    Continue,
    /// Stop after this step.
    Stop,
}

/// Failure of the integration.
#[derive(Debug, Clone, PartialEq)]
pub enum OdeError {
    /// Step size fell below the minimum representable.
    StepTooSmall {
        /// Time reached.
        t: f64,
    },
    /// Step budget exhausted.
    TooManySteps {
        /// Time reached.
        t: f64,
    },
    /// Singular iteration matrix.
    Singular {
        /// Time reached.
        t: f64,
    },
}

impl fmt::Display for OdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OdeError::StepTooSmall { t } => write!(f, "ODE step size underflow at t = {t:e}"),
            OdeError::TooManySteps { t } => write!(f, "ODE step budget exhausted at t = {t:e}"),
            OdeError::Singular { t } => write!(f, "singular iteration matrix at t = {t:e}"),
        }
    }
}

impl std::error::Error for OdeError {}

/// Statistics of a run.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stats {
    /// Accepted steps.
    pub accepted: usize,
    /// Rejected steps.
    pub rejected: usize,
    /// Right-hand-side evaluations.
    pub rhs_evals: usize,
    /// Steps rejected because the Newton iteration failed.
    pub newton_failures: usize,
}

/// In-place LU factorisation with partial pivoting of a row-major n x n matrix.
pub fn lu_factor(a: &mut [f64], n: usize, piv: &mut [usize]) -> bool {
    for k in 0..n {
        let mut p = k;
        let mut mx = a[k * n + k].abs();
        for i in k + 1..n {
            if a[i * n + k].abs() > mx {
                mx = a[i * n + k].abs();
                p = i;
            }
        }
        if !(mx > 0.0) || !mx.is_finite() {
            return false;
        }
        piv[k] = p;
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
        }
        let d = a[k * n + k];
        for i in k + 1..n {
            let m = a[i * n + k] / d;
            a[i * n + k] = m;
            if m != 0.0 {
                for j in k + 1..n {
                    a[i * n + j] -= m * a[k * n + j];
                }
            }
        }
    }
    true
}

/// Solve with the factors from [`lu_factor`], in place on `b`.
pub fn lu_solve(a: &[f64], n: usize, piv: &[usize], b: &mut [f64]) {
    for k in 0..n {
        b.swap(k, piv[k]);
    }
    for k in 0..n {
        let bk = b[k];
        for i in k + 1..n {
            b[i] -= a[i * n + k] * bk;
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for j in k + 1..n {
            s -= a[k * n + j] * b[j];
        }
        b[k] = s / a[k * n + k];
    }
}

/// Real eigenvalue of the inverse Radau IIA(5) coefficient matrix (about 3.6378).
fn radau_u1(a: &[[f64; 3]; 3]) -> f64 {
    // inverse of a 3x3 matrix by cofactors
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    let mut inv = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let (r0, r1) = ((j + 1) % 3, (j + 2) % 3);
            let (c0, c1) = ((i + 1) % 3, (i + 2) % 3);
            inv[i][j] = (a[r0][c0] * a[r1][c1] - a[r0][c1] * a[r1][c0]) / det;
        }
    }
    let tr = inv[0][0] + inv[1][1] + inv[2][2];
    let mut m = 0.0;
    for (i, j) in [(0, 1), (0, 2), (1, 2)] {
        m += inv[i][i] * inv[j][j] - inv[i][j] * inv[j][i];
    }
    let d = 1.0 / det;
    let mut x: f64 = 3.6;
    for _ in 0..50 {
        let f = x * x * x - tr * x * x + m * x - d;
        let fp = 3.0 * x * x - 2.0 * tr * x + m;
        let dx = f / fp;
        x -= dx;
        if dx.abs() < 1e-15 {
            break;
        }
    }
    x
}

/// Integrate `y' = f(t, y)` from `t0` to `t1` with the 3-stage Radau IIA method of order 5 (Hairer and Wanner,
/// Solving ODEs II, section IV.8): simplified Newton iteration on the stage values with a forward-difference Jacobian
/// and dense LU, the embedded error estimate of Radau5 and step-size control. The observer is called after every accepted step.
pub fn integrate(
    rhs: &mut dyn FnMut(f64, &[f64], &mut [f64]),
    t0: f64,
    t1: f64,
    y0: &[f64],
    opt: &Options,
    observer: &mut dyn FnMut(&Step<'_>) -> Flow,
) -> Result<(Vec<f64>, Stats), OdeError> {
    let n = y0.len();
    let n3 = 3 * n;
    let s6 = 6.0_f64.sqrt();
    let c = [(4.0 - s6) / 10.0, (4.0 + s6) / 10.0, 1.0];
    let a: [[f64; 3]; 3] = [
        [
            (88.0 - 7.0 * s6) / 360.0,
            (296.0 - 169.0 * s6) / 1800.0,
            (-2.0 + 3.0 * s6) / 225.0,
        ],
        [
            (296.0 + 169.0 * s6) / 1800.0,
            (88.0 + 7.0 * s6) / 360.0,
            (-2.0 - 3.0 * s6) / 225.0,
        ],
        [(16.0 - s6) / 36.0, (16.0 + s6) / 36.0, 1.0 / 9.0],
    ];
    let u1 = radau_u1(&a);
    let dd = [-(13.0 + 7.0 * s6) / 3.0, (-13.0 + 7.0 * s6) / 3.0, -1.0 / 3.0];
    let mut st = Stats::default();
    let mut t = t0;
    let mut y = y0.to_vec();
    let mut f = vec![0.0; n];
    rhs(t, &y, &mut f);
    st.rhs_evals += 1;
    let dir = (t1 - t0).signum();
    let scale = |y: &[f64], i: usize| opt.atol[i] + opt.rtol * y[i].abs();
    let mut h = if opt.h0 > 0.0 {
        opt.h0
    } else {
        let (mut d0, mut d1) = (0.0, 0.0);
        for i in 0..n {
            d0 += (y[i] / scale(&y, i)).powi(2);
            d1 += (f[i] / scale(&y, i)).powi(2);
        }
        let (d0, d1) = ((d0 / n as f64).sqrt(), (d1 / n as f64).sqrt());
        let h = if d0 < 1e-5 || d1 < 1e-5 { 1e-6 } else { 0.01 * d0 / d1 };
        h.min((t1 - t0).abs())
    }
    .min(opt.h_max)
    .max(1e-300);
    let mut jac = vec![0.0; n * n];
    let mut jac_fresh = false;
    let mut jac_ok = false;
    let mut m3 = vec![0.0; n3 * n3];
    let mut piv3 = vec![0usize; n3];
    let mut lu_h = 0.0;
    let mut lu_ok = false;
    let mut e1 = vec![0.0; n * n];
    let mut piv1 = vec![0usize; n];
    let mut z = vec![0.0; n3];
    let mut z_prev: Option<(Vec<f64>, f64)> = None;
    let mut ff = vec![0.0; n3];
    let mut g = vec![0.0; n3];
    let mut ytmp = vec![0.0; n];
    let mut ftmp = vec![0.0; n];
    let mut last_newton = 1usize;
    let mut first_or_rejected = true;
    let mut fpert = vec![0.0; n];
    let mut fails = 0;
    while (t1 - t) * dir > 1e-14 * t1.abs().max(1e-300) {
        if st.accepted >= opt.max_steps {
            return Err(OdeError::TooManySteps { t });
        }
        let mut hh = h.min((t1 - t).abs()).min(opt.h_max);
        let final_step = (t1 - t).abs() <= hh * (1.0 + 1e-12);
        if final_step {
            hh = (t1 - t).abs();
        }
        if hh < 1e-15 * t.abs().max(1e-30) || hh < 1e-300 {
            return Err(OdeError::StepTooSmall { t });
        }
        let hs = hh * dir;
        if !jac_ok || last_newton >= 4 {
            for j in 0..n {
                let delta = 1.0e-7 * y[j].abs().max(opt.atol[j] / opt.rtol.max(1e-12)).max(1e-6);
                let save = y[j];
                y[j] = save + delta;
                rhs(t, &y, &mut fpert);
                y[j] = save;
                for i in 0..n {
                    jac[i * n + j] = (fpert[i] - f[i]) / delta;
                }
            }
            st.rhs_evals += n;
            jac_ok = true;
            jac_fresh = true;
            lu_ok = false;
            last_newton = 1;
        }
        if !lu_ok || (hs / lu_h - 1.0).abs() > 0.05 {
            for bi in 0..3 {
                for bj in 0..3 {
                    for i in 0..n {
                        for j in 0..n {
                            m3[(bi * n + i) * n3 + bj * n + j] = -hs * a[bi][bj] * jac[i * n + j];
                        }
                    }
                }
            }
            for i in 0..n3 {
                m3[i * n3 + i] += 1.0;
            }
            if !lu_factor(&mut m3, n3, &mut piv3) {
                return Err(OdeError::Singular { t });
            }
            lu_h = hs;
            lu_ok = true;
        }
        // starting values from the previous collocation polynomial
        match &z_prev {
            Some((zp, hp)) if fails == 0 => {
                let r = hs / hp;
                let nodes = [0.0, c[0], c[1], 1.0];
                for (s, ci) in c.iter().enumerate() {
                    let x = 1.0 + ci * r;
                    for i in 0..n {
                        let vals = [0.0, zp[i], zp[n + i], zp[2 * n + i]];
                        let mut q = 0.0;
                        for k in 0..4 {
                            let mut l = 1.0;
                            for m in 0..4 {
                                if m != k {
                                    l *= (x - nodes[m]) / (nodes[k] - nodes[m]);
                                }
                            }
                            q += l * vals[k];
                        }
                        z[s * n + i] = q - zp[2 * n + i];
                    }
                }
            }
            _ => z.iter_mut().for_each(|v| *v = 0.0),
        }
        // simplified Newton iteration
        let mut converged = false;
        let mut dold: f64 = 0.0;
        let mut newton_iters = 0;
        let mut bad = false;
        let mut theta_final = 0.0;
        for k in 0..10 {
            newton_iters = k + 1;
            for s in 0..3 {
                for i in 0..n {
                    ytmp[i] = y[i] + z[s * n + i];
                }
                rhs(t + c[s] * hs, &ytmp, &mut ftmp);
                if ftmp.iter().any(|v| !v.is_finite()) {
                    bad = true;
                    break;
                }
                ff[s * n..(s + 1) * n].copy_from_slice(&ftmp);
            }
            st.rhs_evals += 3;
            if bad {
                break;
            }
            for s in 0..3 {
                for i in 0..n {
                    let mut acc = 0.0;
                    for q in 0..3 {
                        acc += a[s][q] * ff[q * n + i];
                    }
                    g[s * n + i] = -(z[s * n + i] - hs * acc);
                }
            }
            lu_solve(&m3, n3, &piv3, &mut g);
            let mut dn = 0.0;
            for s in 0..3 {
                for i in 0..n {
                    dn += (g[s * n + i] / scale(&y, i)).powi(2);
                }
            }
            let dn = (dn / n3 as f64).sqrt();
            for i in 0..n3 {
                z[i] += g[i];
            }
            if !dn.is_finite() {
                bad = true;
                break;
            }
            if k > 0 {
                let theta = dn / dold.max(1e-300);
                if theta >= 0.99 {
                    bad = true;
                    break;
                }
                if theta / (1.0 - theta) * dn <= 0.03 {
                    theta_final = theta;
                    converged = true;
                    break;
                }
            } else if dn <= 1e-4 {
                converged = true;
                break;
            }
            dold = dn;
        }
        if !converged || bad {
            st.rejected += 1;
            st.newton_failures += 1;
            fails += 1;
            first_or_rejected = true;
            if fails > 40 {
                return Err(OdeError::StepTooSmall { t });
            }
            if jac_fresh {
                h = hh * 0.5;
            } else {
                jac_ok = false; // retry the same step with a fresh Jacobian
            }
            continue;
        }
        // embedded error estimate
        for i in 0..n * n {
            e1[i] = -jac[i];
        }
        for i in 0..n {
            e1[i * n + i] += u1 / hs;
        }
        if !lu_factor(&mut e1, n, &mut piv1) {
            return Err(OdeError::Singular { t });
        }
        let mut cont = vec![0.0; n];
        for i in 0..n {
            cont[i] = f[i] + (dd[0] * z[i] + dd[1] * z[n + i] + dd[2] * z[2 * n + i]) / hs;
        }
        let f2: Vec<f64> = cont.clone();
        lu_solve(&e1, n, &piv1, &mut cont);
        let ynew: Vec<f64> = (0..n).map(|i| y[i] + z[2 * n + i]).collect();
        let norm_err = |cont: &[f64]| -> f64 {
            let mut e = 0.0;
            for i in 0..n {
                let sc = opt.atol[i] + opt.rtol * y[i].abs().max(ynew[i].abs());
                e += (cont[i] / sc).powi(2);
            }
            (e / n as f64).sqrt()
        };
        let mut err = norm_err(&cont);
        if err >= 1.0 && first_or_rejected {
            for i in 0..n {
                ytmp[i] = y[i] + cont[i];
            }
            rhs(t, &ytmp, &mut ftmp);
            st.rhs_evals += 1;
            for i in 0..n {
                cont[i] = ftmp[i] + (f2[i] - f[i]);
            }
            lu_solve(&e1, n, &piv1, &mut cont);
            err = norm_err(&cont);
        }
        if !err.is_finite() {
            st.rejected += 1;
            h = hh * 0.25;
            fails += 1;
            continue;
        }
        let fac = (0.9 * err.max(1e-8).powf(-0.25)).clamp(0.2, 5.0);
        if err <= 1.0 {
            let mut fnew = vec![0.0; n];
            rhs(t + hs, &ynew, &mut fnew);
            st.rhs_evals += 1;
            let tn = if final_step { t1 } else { t + hs };
            let flow = observer(&Step {
                t0: t,
                t1: tn,
                y0: &y,
                y1: &ynew,
                f0: &f,
                f1: &fnew,
            });
            t = tn;
            y.copy_from_slice(&ynew);
            f.copy_from_slice(&fnew);
            st.accepted += 1;
            z_prev = Some((z.clone(), hs));
            last_newton = newton_iters;
            // keep the Jacobian only while the iteration converges very fast (as Radau5 does)
            if theta_final >= 1e-3 {
                jac_ok = false;
            }
            jac_fresh = false;
            fails = 0;
            first_or_rejected = false;
            let mut hn = hh * fac;
            if (1.0..1.2).contains(&(hn / hh)) {
                hn = hh;
            }
            h = hn;
            if flow == Flow::Stop {
                return Ok((y, st));
            }
        } else {
            st.rejected += 1;
            fails += 1;
            first_or_rejected = true;
            h = hh * fac.min(0.9);
            if fails > 40 {
                return Err(OdeError::StepTooSmall { t });
            }
        }
    }
    Ok((y, st))
}
