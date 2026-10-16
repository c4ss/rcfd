//! Benchmark routines shared by the `gpu_bench` example and the ignored F1 test (ADR 0038 D7). CG with the same
//! tolerance on every path; 1 warm-up run, then the median of N wall-clock runs.
#![allow(dead_code, missing_docs)]
#![cfg(feature = "cuda")]

use super::{grid_matrix, matvec_ref};
use rcfd_gpu::DeviceSystem;
use rcfd_linalg::{cg, Amg, AmgSettings, CsrMatrix, GpuPrecond, Jacobi, LinearOptions, SolveControl};
use std::time::Instant;

pub fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Run `f` once as warm-up and `reps` times measured; returns (median seconds, last result).
pub fn timed<T>(reps: usize, mut f: impl FnMut() -> T) -> (f64, Vec<f64>, T) {
    let _ = f();
    let mut times = Vec::with_capacity(reps);
    let mut last = None;
    for _ in 0..reps {
        let t = Instant::now();
        let r = f();
        times.push(t.elapsed().as_secs_f64());
        last = Some(r);
    }
    let mut s = times.clone();
    (median(&mut s), times, last.expect("reps >= 1"))
}

pub fn control() -> SolveControl {
    SolveControl {
        tol_abs: 0.0,
        tol_rel: 1e-6,
        max_iter: 5000,
    }
}

pub fn problem(cube: usize) -> (CsrMatrix, Vec<f64>) {
    let a = grid_matrix(cube, cube, cube, 0.0);
    let xe: Vec<f64> = (0..a.n).map(|i| ((i * 37 + 11) % 17) as f64 / 8.0 - 1.0).collect();
    let b = matvec_ref(&a, &xe);
    (a, b)
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap()
}

#[derive(Debug, Clone, Default)]
pub struct Row {
    pub cube: usize,
    pub n: usize,
    pub nnz: usize,
    pub cpu1_s: f64,
    pub cpu1_iters: usize,
    pub cpun_s: f64,
    pub cpun_threads: usize,
    pub amg1_setup_s: f64,
    pub amg1_solve_s: f64,
    pub amg1_iters: usize,
    pub gpu_resident_s: f64,
    pub gpu_iters: usize,
    pub gpu_oneshot_s: f64,
    pub gpu_cheb_resident_s: f64,
    pub gpu_cheb_iters: usize,
    pub up_matrix_s: f64,
    pub up_bytes: usize,
    pub up_vec_s: f64,
    pub down_s: f64,
    pub solve_s: f64,
}

/// Measure CPU 1 thread, CPU all threads, CPU 1 thread AMG-CG (optional) and the GPU paths for one cube size.
pub fn bench_size(cube: usize, reps: usize, with_amg: bool, with_cpu: bool) -> Row {
    let (a, b) = problem(cube);
    let c = control();
    let mut row = Row {
        cube,
        n: a.n,
        nnz: a.nnz(),
        ..Row::default()
    };
    let diag = a.diag();
    if with_cpu {
        let p1 = pool(1);
        let m = Jacobi::new(&diag);
        let (t, _, st) = timed(reps, || {
            p1.install(|| {
                let mut x = vec![0.0; a.n];
                cg(&a, &b, &mut x, &m, &c)
            })
        });
        row.cpu1_s = t;
        row.cpu1_iters = st.iterations;
        let nthreads = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1);
        let pn = pool(nthreads);
        let (t, _, _) = timed(reps, || {
            pn.install(|| {
                let mut x = vec![0.0; a.n];
                cg(&a, &b, &mut x, &m, &c)
            })
        });
        row.cpun_s = t;
        row.cpun_threads = nthreads;
        if with_amg {
            let mut setups = Vec::new();
            let mut solves = Vec::new();
            let mut iters = 0;
            let _ = p1.install(|| {
                let amg = Amg::new(&a, AmgSettings::default());
                let mut x = vec![0.0; a.n];
                cg(&a, &b, &mut x, &amg, &c)
            });
            for _ in 0..reps {
                let (amg, ts) = {
                    let t = Instant::now();
                    let amg = p1.install(|| Amg::new(&a, AmgSettings::default()));
                    (amg, t.elapsed().as_secs_f64())
                };
                let t = Instant::now();
                let st = p1.install(|| {
                    let mut x = vec![0.0; a.n];
                    cg(&a, &b, &mut x, &amg, &c)
                });
                solves.push(t.elapsed().as_secs_f64());
                setups.push(ts);
                iters = st.iterations;
            }
            row.amg1_setup_s = median(&mut setups);
            row.amg1_solve_s = median(&mut solves);
            row.amg1_iters = iters;
        }
    }
    // GPU, matrix resident
    let o = LinearOptions::default();
    let mut sys = DeviceSystem::new(&a, &o).expect("device system");
    row.up_matrix_s = sys.matrix_upload_seconds;
    row.up_bytes = sys.resident_bytes();
    let mut parts = Vec::new();
    let (t, _, st) = timed(reps, || {
        let mut x = vec![0.0; a.n];
        let (st, tm) = sys.cg(&b, &mut x, &c).expect("gpu cg");
        parts.push(tm);
        st
    });
    row.gpu_resident_s = t;
    row.gpu_iters = st.iterations;
    let mut up: Vec<f64> = parts.iter().map(|t| t.upload).collect();
    let mut dn: Vec<f64> = parts.iter().map(|t| t.download).collect();
    let mut sv: Vec<f64> = parts.iter().map(|t| t.solve).collect();
    row.up_vec_s = median(&mut up);
    row.down_s = median(&mut dn);
    row.solve_s = median(&mut sv);
    drop(sys);
    // GPU one-shot: matrix upload + solve + download, from host arrays
    let (t, _, _) = timed(reps, || {
        let mut x = vec![0.0; a.n];
        let mut s = DeviceSystem::new(&a, &o).expect("device system");
        s.cg(&b, &mut x, &c).expect("gpu cg").0
    });
    row.gpu_oneshot_s = t;
    // GPU Chebyshev(4), matrix resident
    let oc = LinearOptions {
        precond: GpuPrecond::Chebyshev,
        chebyshev_degree: 4,
        ..LinearOptions::default()
    };
    let mut sysc = DeviceSystem::new(&a, &oc).expect("device system");
    let (t, _, st) = timed(reps, || {
        let mut x = vec![0.0; a.n];
        sysc.cg(&b, &mut x, &c).expect("gpu cg").0
    });
    row.gpu_cheb_resident_s = t;
    row.gpu_cheb_iters = st.iterations;
    row
}
