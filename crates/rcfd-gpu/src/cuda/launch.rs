//! Kernel launches. This is the ONLY module of the workspace that contains `unsafe` code (ADR 0038 D2). Every block
//! carries a `SAFETY:` comment. All wrappers take typed device buffers whose lengths are checked against the element
//! count before the launch, and pass arguments in exactly the order and types of the kernel signatures in `kernels.cu`
//! (`int` as `i32`, `double` as `f64`, pointers as device slices).
#![allow(unsafe_code)]

use super::runtime::{de, DeviceBuf, Runtime};
use cudarc::driver::{LaunchConfig, PushKernelArg};
use rcfd_linalg::BackendError;

const RB: u32 = 256;
const MAX_BLOCKS: usize = 1024;

/// Whether the CUDA driver library can be loaded (the loaders of the binding panic when it cannot).
pub(super) fn driver_present() -> bool {
    // SAFETY: `is_culib_present` only tries to open the driver library by name and reports whether that worked; it has
    // no preconditions and does not keep the handle.
    unsafe { cudarc::driver::sys::is_culib_present() }
}

/// Whether the NVRTC library can be loaded.
pub(super) fn nvrtc_present() -> bool {
    // SAFETY: as for `driver_present`, a pure library-presence probe by name.
    unsafe { cudarc::nvrtc::sys::is_culib_present() }
}

/// Number of blocks of a reduction over `n` elements (depends only on `n`, which fixes the summation order).
pub(super) fn reduce_blocks(n: usize) -> usize {
    n.div_ceil(RB as usize).clamp(1, MAX_BLOCKS)
}

fn cfg_elems(n: usize) -> LaunchConfig {
    LaunchConfig {
        grid_dim: ((n.div_ceil(RB as usize)).max(1) as u32, 1, 1),
        block_dim: (RB, 1, 1),
        shared_mem_bytes: 0,
    }
}

fn cfg_reduce(blocks: usize) -> LaunchConfig {
    LaunchConfig {
        grid_dim: (blocks as u32, 1, 1),
        block_dim: (RB, 1, 1),
        shared_mem_bytes: 0,
    }
}

fn need(len: usize, n: usize, what: &str) {
    assert!(len >= n, "device buffer `{what}` has {len} elements, kernel needs {n}");
}

/// Device CSR matrix with 32-bit indices.
pub(crate) struct DeviceCsr {
    pub n: usize,
    pub nnz: usize,
    pub row_ptr: DeviceBuf<i32>,
    pub col: DeviceBuf<i32>,
    pub val: DeviceBuf<f64>,
}

/// `y = A x`
pub(super) fn spmv(
    rt: &Runtime,
    a: &DeviceCsr,
    x: &DeviceBuf<f64>,
    y: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(x.len(), a.n, "x");
    need(y.len(), a.n, "y");
    let n = a.n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.spmv);
    b.arg(&n)
        .arg(&a.row_ptr.s)
        .arg(&a.col.s)
        .arg(&a.val.s)
        .arg(&x.s)
        .arg(&mut y.s);
    // SAFETY: kernel `spmv(int n, const int*, const int*, const double*, const double*, double*)` receives `n`, the
    // three CSR arrays (row_ptr has n+1 entries, col/val have nnz entries, validated at upload) and x, y with at
    // least n elements (checked above); the column indices were validated to lie in [0, n) at upload.
    unsafe { b.launch(cfg_elems(a.n)) }.map_err(de("launch spmv"))?;
    Ok(())
}

/// `y += alpha x`
pub(super) fn axpy(
    rt: &Runtime,
    n: usize,
    alpha: f64,
    x: &DeviceBuf<f64>,
    y: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(x.len(), n, "x");
    need(y.len(), n, "y");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.axpy);
    b.arg(&ni).arg(&alpha).arg(&x.s).arg(&mut y.s);
    // SAFETY: kernel `axpy(int, double, const double*, double*)`; x and y hold at least n elements (checked above).
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch axpy"))?;
    Ok(())
}

/// `y = x + beta y`
pub(super) fn xpby(
    rt: &Runtime,
    n: usize,
    beta: f64,
    x: &DeviceBuf<f64>,
    y: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(x.len(), n, "x");
    need(y.len(), n, "y");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.xpby);
    b.arg(&ni).arg(&beta).arg(&x.s).arg(&mut y.s);
    // SAFETY: kernel `xpby(int, double, const double*, double*)`; x and y hold at least n elements (checked above).
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch xpby"))?;
    Ok(())
}

/// `r = b - r`
pub(super) fn rsub(rt: &Runtime, n: usize, bv: &DeviceBuf<f64>, r: &mut DeviceBuf<f64>) -> Result<(), BackendError> {
    need(bv.len(), n, "b");
    need(r.len(), n, "r");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.rsub);
    b.arg(&ni).arg(&bv.s).arg(&mut r.s);
    // SAFETY: kernel `rsub(int, const double*, double*)`; both buffers hold at least n elements (checked above).
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch rsub"))?;
    Ok(())
}

/// `out = a * b`
pub(super) fn mul(
    rt: &Runtime,
    n: usize,
    a: &DeviceBuf<f64>,
    bv: &DeviceBuf<f64>,
    out: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(a.len(), n, "a");
    need(bv.len(), n, "b");
    need(out.len(), n, "out");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.mul);
    b.arg(&ni).arg(&a.s).arg(&bv.s).arg(&mut out.s);
    // SAFETY: kernel `mul(int, const double*, const double*, double*)`; all buffers hold at least n elements.
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch mul"))?;
    Ok(())
}

/// `p = r + beta (p - omega v)`
pub(super) fn bicg_p(
    rt: &Runtime,
    n: usize,
    beta: f64,
    omega: f64,
    r: &DeviceBuf<f64>,
    v: &DeviceBuf<f64>,
    p: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(r.len(), n, "r");
    need(v.len(), n, "v");
    need(p.len(), n, "p");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.bicg_p);
    b.arg(&ni).arg(&beta).arg(&omega).arg(&r.s).arg(&v.s).arg(&mut p.s);
    // SAFETY: kernel `bicg_p(int, double, double, const double*, const double*, double*)`; buffers hold n elements.
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch bicg_p"))?;
    Ok(())
}

/// `out = a - alpha b`
pub(super) fn sub_scaled(
    rt: &Runtime,
    n: usize,
    alpha: f64,
    a: &DeviceBuf<f64>,
    bv: &DeviceBuf<f64>,
    out: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(a.len(), n, "a");
    need(bv.len(), n, "b");
    need(out.len(), n, "out");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.sub_scaled);
    b.arg(&ni).arg(&alpha).arg(&a.s).arg(&bv.s).arg(&mut out.s);
    // SAFETY: kernel `sub_scaled(int, double, const double*, const double*, double*)`; buffers hold n elements.
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch sub_scaled"))?;
    Ok(())
}

/// Chebyshev start: `rc = dinv r`, `d = rc / theta`
pub(super) fn cheb_init(
    rt: &Runtime,
    n: usize,
    theta: f64,
    dinv: &DeviceBuf<f64>,
    r: &DeviceBuf<f64>,
    rc: &mut DeviceBuf<f64>,
    d: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(dinv.len(), n, "dinv");
    need(r.len(), n, "r");
    need(rc.len(), n, "rc");
    need(d.len(), n, "d");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.cheb_init);
    b.arg(&ni)
        .arg(&theta)
        .arg(&dinv.s)
        .arg(&r.s)
        .arg(&mut rc.s)
        .arg(&mut d.s);
    // SAFETY: kernel `cheb_init(int, double, const double*, const double*, double*, double*)`; buffers hold n elements.
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch cheb_init"))?;
    Ok(())
}

/// Chebyshev step: `rc -= dinv t`, `d = c1 d + c2 rc`
pub(super) fn cheb_step(
    rt: &Runtime,
    n: usize,
    c1: f64,
    c2: f64,
    dinv: &DeviceBuf<f64>,
    t: &DeviceBuf<f64>,
    rc: &mut DeviceBuf<f64>,
    d: &mut DeviceBuf<f64>,
) -> Result<(), BackendError> {
    need(dinv.len(), n, "dinv");
    need(t.len(), n, "t");
    need(rc.len(), n, "rc");
    need(d.len(), n, "d");
    let ni = n as i32;
    let mut b = rt.stream.launch_builder(&rt.k.cheb_step);
    b.arg(&ni)
        .arg(&c1)
        .arg(&c2)
        .arg(&dinv.s)
        .arg(&t.s)
        .arg(&mut rc.s)
        .arg(&mut d.s);
    // SAFETY: kernel `cheb_step(int, double, double, const double*, const double*, double*, double*)`; buffers hold n
    // elements (checked above).
    unsafe { b.launch(cfg_elems(n)) }.map_err(de("launch cheb_step"))?;
    Ok(())
}

/// Device scratch of the reductions.
pub(crate) struct Scratch {
    part: DeviceBuf<f64>,
    out: DeviceBuf<f64>,
}

impl Scratch {
    pub fn new(rt: &Runtime) -> Result<Self, BackendError> {
        Ok(Scratch {
            part: DeviceBuf::zeros(rt, 2 * MAX_BLOCKS)?,
            out: DeviceBuf::zeros(rt, 2)?,
        })
    }

    fn finish(&mut self, rt: &Runtime, blocks: usize, k: usize) -> Result<[f64; 2], BackendError> {
        let m = blocks as i32;
        let ki = k as i32;
        let mut b = rt.stream.launch_builder(&rt.k.reduce_final);
        b.arg(&m).arg(&ki).arg(&self.part.s).arg(&mut self.out.s);
        // SAFETY: kernel `reduce_final(int m, int k, const double*, double*)`; `part` holds 2*MAX_BLOCKS >= k*m
        // elements (k <= 2, m <= MAX_BLOCKS) and `out` holds 2 elements.
        unsafe { b.launch(cfg_reduce(1)) }.map_err(de("launch reduce_final"))?;
        let mut host = [0.0f64; 2];
        self.out.to_host(rt, &mut host)?;
        Ok(host)
    }

    /// `sum_i a_i b_i`
    pub fn dot(
        &mut self,
        rt: &Runtime,
        n: usize,
        a: &DeviceBuf<f64>,
        bv: &DeviceBuf<f64>,
    ) -> Result<f64, BackendError> {
        if n == 0 {
            return Ok(0.0);
        }
        need(a.len(), n, "a");
        need(bv.len(), n, "b");
        let blocks = reduce_blocks(n);
        let ni = n as i32;
        let mut b = rt.stream.launch_builder(&rt.k.dot_partial);
        b.arg(&ni).arg(&a.s).arg(&bv.s).arg(&mut self.part.s);
        // SAFETY: kernel `dot_partial(int, const double*, const double*, double*)`; a and b hold n elements (checked),
        // `part` holds MAX_BLOCKS >= blocks elements.
        unsafe { b.launch(cfg_reduce(blocks)) }.map_err(de("launch dot_partial"))?;
        Ok(self.finish(rt, blocks, 1)?[0])
    }

    /// `(sum a_i b_i, sum c_i d_i)` in one pass.
    pub fn dot2(
        &mut self,
        rt: &Runtime,
        n: usize,
        a: &DeviceBuf<f64>,
        bv: &DeviceBuf<f64>,
        c: &DeviceBuf<f64>,
        d: &DeviceBuf<f64>,
    ) -> Result<(f64, f64), BackendError> {
        if n == 0 {
            return Ok((0.0, 0.0));
        }
        need(a.len(), n, "a");
        need(bv.len(), n, "b");
        need(c.len(), n, "c");
        need(d.len(), n, "d");
        let blocks = reduce_blocks(n);
        let ni = n as i32;
        let mut b = rt.stream.launch_builder(&rt.k.dot2_partial);
        b.arg(&ni)
            .arg(&a.s)
            .arg(&bv.s)
            .arg(&c.s)
            .arg(&d.s)
            .arg(&mut self.part.s);
        // SAFETY: kernel `dot2_partial(int, 4 x const double*, double*)`; the four inputs hold n elements (checked),
        // `part` holds 2*MAX_BLOCKS >= 2*blocks elements.
        unsafe { b.launch(cfg_reduce(blocks)) }.map_err(de("launch dot2_partial"))?;
        let h = self.finish(rt, blocks, 2)?;
        Ok((h[0], h[1]))
    }

    /// `sum_i a_i`
    pub fn sum(&mut self, rt: &Runtime, n: usize, a: &DeviceBuf<f64>) -> Result<f64, BackendError> {
        if n == 0 {
            return Ok(0.0);
        }
        need(a.len(), n, "a");
        let blocks = reduce_blocks(n);
        let ni = n as i32;
        let mut b = rt.stream.launch_builder(&rt.k.sum_partial);
        b.arg(&ni).arg(&a.s).arg(&mut self.part.s);
        // SAFETY: kernel `sum_partial(int, const double*, double*)`; a holds n elements (checked), `part` holds
        // MAX_BLOCKS >= blocks elements.
        unsafe { b.launch(cfg_reduce(blocks)) }.map_err(de("launch sum_partial"))?;
        Ok(self.finish(rt, blocks, 1)?[0])
    }
}
