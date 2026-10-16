//! Device-resident linear system and the CG / BiCGStab solvers that run entirely on the device; only scalars cross to
//! the host (ADR 0038 D5).

use super::launch::{self, DeviceCsr, Scratch};
use super::runtime::{de, rt, DeviceBuf, Runtime};
use rcfd_linalg::{
    check_index_range, BackendError, ChebyshevCoeffs, CsrMatrix, GpuPrecond, LinearOptions, SolveControl, SolveStats,
};
use std::time::Instant;

/// Wall-clock split of one solve [s].
#[derive(Debug, Clone, Copy, Default)]
pub struct Timings {
    /// Host-to-device copies of the matrix (at construction) and of b and x.
    pub upload: f64,
    /// Iterations on the device (including the scalar transfers).
    pub solve: f64,
    /// Device-to-host copy of x.
    pub download: f64,
}

struct ChebDev {
    theta: f64,
    degree: usize,
    steps: Vec<(f64, f64)>,
}

struct Work {
    rc: DeviceBuf<f64>,
    d: DeviceBuf<f64>,
    t: DeviceBuf<f64>,
}

/// A matrix (and preconditioner data) resident on the device.
pub struct DeviceSystem {
    rt: &'static Runtime,
    n: usize,
    a: DeviceCsr,
    dinv: DeviceBuf<f64>,
    cheb: Option<ChebDev>,
    scratch: Scratch,
    /// Seconds spent copying the matrix and preconditioner data to the device.
    pub matrix_upload_seconds: f64,
}

fn validate_csr(a: &CsrMatrix) -> Result<(), BackendError> {
    check_index_range(a.n, a.val.len())?;
    if a.n != a.ncols {
        return Err(BackendError::new(format!(
            "matrix must be square, got {} rows x {} columns",
            a.n, a.ncols
        )));
    }
    if a.row_ptr.len() != a.n + 1 || a.col.len() != a.val.len() || a.row_ptr[a.n] != a.val.len() {
        return Err(BackendError::new(format!(
            "inconsistent CSR arrays: row_ptr has {} entries (expected {}), col {} and val {} entries, row_ptr[n] = {}",
            a.row_ptr.len(),
            a.n + 1,
            a.col.len(),
            a.val.len(),
            a.row_ptr.last().copied().unwrap_or(0)
        )));
    }
    if a.row_ptr.windows(2).any(|w| w[1] < w[0]) {
        return Err(BackendError::new("CSR row_ptr is not non-decreasing"));
    }
    if let Some(&c) = a.col.iter().find(|&&c| c >= a.n) {
        return Err(BackendError::new(format!(
            "CSR column index {c} is out of range for {} columns",
            a.n
        )));
    }
    Ok(())
}

impl DeviceSystem {
    /// Upload `a` and build the device preconditioner data selected by `o`.
    pub fn new(a: &CsrMatrix, o: &LinearOptions) -> Result<Self, BackendError> {
        validate_csr(a)?;
        let rt = rt()?;
        let t0 = Instant::now();
        let n = a.n;
        let rp: Vec<i32> = a.row_ptr.iter().map(|&v| v as i32).collect();
        let ci: Vec<i32> = a.col.iter().map(|&v| v as i32).collect();
        let (dinv_host, cheb) = match o.precond {
            GpuPrecond::None => (vec![1.0; n], None),
            GpuPrecond::Jacobi => (
                a.diag()
                    .iter()
                    .map(|&d| if d != 0.0 { 1.0 / d } else { 1.0 })
                    .collect::<Vec<f64>>(),
                None,
            ),
            GpuPrecond::Chebyshev => {
                let c = ChebyshevCoeffs::new(a, o.degree()).ok_or_else(|| {
                    BackendError::new(
                        "Chebyshev preconditioner: no finite positive spectral bound (empty matrix or zero rows)",
                    )
                })?;
                let ChebyshevCoeffs {
                    dinv,
                    theta,
                    degree,
                    steps,
                } = c;
                (dinv, Some(ChebDev { theta, degree, steps }))
            }
        };
        let dev = DeviceCsr {
            n,
            nnz: a.val.len(),
            row_ptr: DeviceBuf::from_host(rt, &rp)?,
            col: DeviceBuf::from_host(rt, &ci)?,
            val: DeviceBuf::from_host(rt, &a.val)?,
        };
        let dinv = DeviceBuf::from_host(rt, &dinv_host)?;
        let scratch = Scratch::new(rt)?;
        rt.stream.synchronize().map_err(de("stream synchronise"))?;
        Ok(DeviceSystem {
            rt,
            n,
            a: dev,
            dinv,
            cheb,
            scratch,
            matrix_upload_seconds: t0.elapsed().as_secs_f64(),
        })
    }

    /// Number of rows.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Stored entries.
    pub fn nnz(&self) -> usize {
        self.a.nnz
    }

    /// Bytes of the resident matrix and preconditioner data.
    pub fn resident_bytes(&self) -> usize {
        self.a.nnz * 12 + (self.n + 1) * 4 + self.n * 8
    }

    /// `y = A x` through the device (uploads x, downloads y).
    pub fn matvec(&mut self, x: &[f64], y: &mut [f64]) -> Result<(), BackendError> {
        if x.len() != self.n || y.len() != self.n {
            return Err(BackendError::new(format!(
                "matvec: x has length {}, y has length {}, the matrix has {} rows",
                x.len(),
                y.len(),
                self.n
            )));
        }
        let rt = self.rt;
        let xd = DeviceBuf::from_host(rt, x)?;
        let mut yd = DeviceBuf::<f64>::zeros(rt, self.n)?;
        launch::spmv(rt, &self.a, &xd, &mut yd)?;
        yd.to_host(rt, y)
    }

    fn work(&self) -> Result<Option<Work>, BackendError> {
        if self.cheb.is_none() {
            return Ok(None);
        }
        Ok(Some(Work {
            rc: DeviceBuf::zeros(self.rt, self.n)?,
            d: DeviceBuf::zeros(self.rt, self.n)?,
            t: DeviceBuf::zeros(self.rt, self.n)?,
        }))
    }

    /// `z = M r`
    fn apply_m(&self, w: &mut Option<Work>, r: &DeviceBuf<f64>, z: &mut DeviceBuf<f64>) -> Result<(), BackendError> {
        let (rt, n) = (self.rt, self.n);
        match (&self.cheb, w.as_mut()) {
            (Some(c), Some(w)) => {
                launch::cheb_init(rt, n, c.theta, &self.dinv, r, &mut w.rc, &mut w.d)?;
                rt.stream.memset_zeros(&mut z.s).map_err(de("memset"))?;
                for i in 0..c.degree {
                    launch::axpy(rt, n, 1.0, &w.d, z)?;
                    if i + 1 < c.degree {
                        launch::spmv(rt, &self.a, &w.d, &mut w.t)?;
                        let (c1, c2) = c.steps[i];
                        launch::cheb_step(rt, n, c1, c2, &self.dinv, &w.t, &mut w.rc, &mut w.d)?;
                    }
                }
                Ok(())
            }
            _ => launch::mul(rt, n, &self.dinv, r, z),
        }
    }

    fn pre(&self, b: &[f64], x: &[f64]) -> Result<(), BackendError> {
        if b.len() != self.n || x.len() != self.n {
            return Err(BackendError::new(format!(
                "solve: b has length {}, x has length {}, the matrix has {} rows",
                b.len(),
                x.len(),
                self.n
            )));
        }
        Ok(())
    }

    /// Preconditioned CG on the device (same recurrences and stopping rule as `rcfd_linalg::cg`).
    pub fn cg(&mut self, b: &[f64], x: &mut [f64], c: &SolveControl) -> Result<(SolveStats, Timings), BackendError> {
        self.pre(b, x)?;
        let (rt, n) = (self.rt, self.n);
        let mut tm = Timings::default();
        let t0 = Instant::now();
        let bd = DeviceBuf::from_host(rt, b)?;
        let mut xd = DeviceBuf::from_host(rt, x)?;
        let mut r = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut z = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut p = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut q = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut w = self.work()?;
        rt.stream.synchronize().map_err(de("stream synchronise"))?;
        tm.upload = t0.elapsed().as_secs_f64();

        let t1 = Instant::now();
        launch::spmv(rt, &self.a, &xd, &mut r)?;
        launch::rsub(rt, n, &bd, &mut r)?;
        let res0 = self.scratch.dot(rt, n, &r, &r)?.sqrt();
        let mut res = res0;
        let done = |res: f64| res <= c.tol_abs || res <= c.tol_rel * res0;
        let mut stats = SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: done(res),
        };
        if !done(res) && res.is_finite() {
            self.apply_m(&mut w, &r, &mut z)?;
            rt.stream.memcpy_dtod(&z.s, &mut p.s).map_err(de("device copy"))?;
            let mut rz = self.scratch.dot(rt, n, &r, &z)?;
            let mut it = 0;
            stats.converged = false;
            while it < c.max_iter {
                launch::spmv(rt, &self.a, &p, &mut q)?;
                let pq = self.scratch.dot(rt, n, &p, &q)?;
                if pq == 0.0 || !pq.is_finite() {
                    break;
                }
                let alpha = rz / pq;
                launch::axpy(rt, n, alpha, &p, &mut xd)?;
                launch::axpy(rt, n, -alpha, &q, &mut r)?;
                it += 1;
                self.apply_m(&mut w, &r, &mut z)?;
                let (rr, rz_new) = self.scratch.dot2(rt, n, &r, &r, &r, &z)?;
                res = rr.sqrt();
                if done(res) {
                    stats.converged = true;
                    break;
                }
                if !res.is_finite() {
                    break;
                }
                let beta = rz_new / rz;
                rz = rz_new;
                launch::xpby(rt, n, beta, &z, &mut p)?;
            }
            stats.iterations = it;
            stats.final_residual = res;
        }
        rt.stream.synchronize().map_err(de("stream synchronise"))?;
        tm.solve = t1.elapsed().as_secs_f64();
        let t2 = Instant::now();
        xd.to_host(rt, x)?;
        tm.download = t2.elapsed().as_secs_f64();
        Ok((stats, tm))
    }

    /// Preconditioned BiCGStab on the device (same recurrences, breakdown exits and stopping rule as
    /// `rcfd_linalg::bicgstab`).
    pub fn bicgstab(
        &mut self,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
    ) -> Result<(SolveStats, Timings), BackendError> {
        self.pre(b, x)?;
        let (rt, n) = (self.rt, self.n);
        let mut tm = Timings::default();
        let t0 = Instant::now();
        let bd = DeviceBuf::from_host(rt, b)?;
        let mut xd = DeviceBuf::from_host(rt, x)?;
        let mut r = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut r0 = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut p = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut v = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut ph = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut s = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut sh = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut t = DeviceBuf::<f64>::zeros(rt, n)?;
        let mut w = self.work()?;
        rt.stream.synchronize().map_err(de("stream synchronise"))?;
        tm.upload = t0.elapsed().as_secs_f64();

        let t1 = Instant::now();
        launch::spmv(rt, &self.a, &xd, &mut r)?;
        launch::rsub(rt, n, &bd, &mut r)?;
        let res0 = self.scratch.dot(rt, n, &r, &r)?.sqrt();
        let mut res = res0;
        let done = |res: f64| res <= c.tol_abs || res <= c.tol_rel * res0;
        let mut stats = SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: done(res),
        };
        if !done(res) && res.is_finite() {
            stats.converged = false;
            rt.stream.memcpy_dtod(&r.s, &mut r0.s).map_err(de("device copy"))?;
            let (mut rho, mut alpha, mut omega) = (1.0f64, 1.0f64, 1.0f64);
            let mut rho_next = self.scratch.dot(rt, n, &r0, &r)?;
            let mut it = 0;
            while it < c.max_iter {
                let rho_new = rho_next;
                if rho_new == 0.0 || omega == 0.0 || !rho_new.is_finite() {
                    break;
                }
                let beta = (rho_new / rho) * (alpha / omega);
                rho = rho_new;
                launch::bicg_p(rt, n, beta, omega, &r, &v, &mut p)?;
                self.apply_m(&mut w, &p, &mut ph)?;
                launch::spmv(rt, &self.a, &ph, &mut v)?;
                let r0v = self.scratch.dot(rt, n, &r0, &v)?;
                if r0v == 0.0 || !r0v.is_finite() {
                    break;
                }
                alpha = rho / r0v;
                launch::sub_scaled(rt, n, alpha, &r, &v, &mut s)?;
                it += 1;
                let sn = self.scratch.dot(rt, n, &s, &s)?.sqrt();
                if done(sn) {
                    launch::axpy(rt, n, alpha, &ph, &mut xd)?;
                    res = sn;
                    stats.converged = true;
                    break;
                }
                if !sn.is_finite() {
                    res = sn;
                    break;
                }
                self.apply_m(&mut w, &s, &mut sh)?;
                launch::spmv(rt, &self.a, &sh, &mut t)?;
                let (tt, ts) = self.scratch.dot2(rt, n, &t, &t, &t, &s)?;
                if tt == 0.0 || !tt.is_finite() {
                    break;
                }
                omega = ts / tt;
                launch::axpy(rt, n, alpha, &ph, &mut xd)?;
                launch::axpy(rt, n, omega, &sh, &mut xd)?;
                launch::sub_scaled(rt, n, omega, &s, &t, &mut r)?;
                let (rr, rn) = self.scratch.dot2(rt, n, &r, &r, &r0, &r)?;
                rho_next = rn;
                res = rr.sqrt();
                if done(res) {
                    stats.converged = true;
                    break;
                }
                if !res.is_finite() {
                    break;
                }
            }
            stats.iterations = it;
            stats.final_residual = res;
        }
        rt.stream.synchronize().map_err(de("stream synchronise"))?;
        tm.solve = t1.elapsed().as_secs_f64();
        let t2 = Instant::now();
        xd.to_host(rt, x)?;
        tm.download = t2.elapsed().as_secs_f64();
        Ok((stats, tm))
    }
}

/// One-shot device reductions and elementwise kernels on host slices (used by tests and diagnostics).
pub mod ops {
    use super::*;

    fn pair(a: &[f64], b: &[f64]) -> Result<(), BackendError> {
        if a.len() != b.len() {
            return Err(BackendError::new(format!(
                "vector lengths differ: {} and {}",
                a.len(),
                b.len()
            )));
        }
        Ok(())
    }

    /// Device dot product `a . b`.
    pub fn dot(a: &[f64], b: &[f64]) -> Result<f64, BackendError> {
        pair(a, b)?;
        let rt = rt()?;
        let (ad, bd) = (DeviceBuf::from_host(rt, a)?, DeviceBuf::from_host(rt, b)?);
        Scratch::new(rt)?.dot(rt, a.len(), &ad, &bd)
    }

    /// Device Euclidean norm.
    pub fn norm2(a: &[f64]) -> Result<f64, BackendError> {
        Ok(dot(a, a)?.sqrt())
    }

    /// Device sum of the entries.
    pub fn sum(a: &[f64]) -> Result<f64, BackendError> {
        let rt = rt()?;
        let ad = DeviceBuf::from_host(rt, a)?;
        Scratch::new(rt)?.sum(rt, a.len(), &ad)
    }

    /// Device `y += alpha x`.
    pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) -> Result<(), BackendError> {
        pair(x, y)?;
        let rt = rt()?;
        let xd = DeviceBuf::from_host(rt, x)?;
        let mut yd = DeviceBuf::from_host(rt, y)?;
        launch::axpy(rt, x.len(), alpha, &xd, &mut yd)?;
        yd.to_host(rt, y)
    }
}
