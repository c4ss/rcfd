//! Benchmark of the device CG against the CPU CG on 3-D Poisson systems (ADR 0038 D7).
//!
//! `cargo run -p rcfd-gpu --features cuda --release --example gpu_bench -- [--sizes 16,32,64,100,171] [--n 5] [--no-amg]`
//!
//! Prints the table that goes into docs/gpu.md. Every number is a median of N runs after one warm-up run; speedups below
//! one are printed as they are.
#![allow(missing_docs)]

#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().position(|a| a == name);
    let sizes: Vec<usize> = flag("--sizes")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').filter_map(|v| v.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![16, 32, 64, 100, 171]);
    let reps: usize = flag("--n")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let with_amg = flag("--no-amg").is_none();
    let with_cpu = flag("--no-cpu").is_none();

    if rcfd_gpu::skip_without_device("gpu_bench") {
        std::process::exit(2);
    }
    println!("{}", rcfd_gpu::probe().describe());
    println!(
        "first-call cost (toolkit location + context + NVRTC compile + module load): {:.3} s",
        rcfd_gpu::init_seconds().unwrap_or(f64::NAN)
    );
    println!("CG, 3-D 7-point Poisson, tol_rel 1e-6, x0 = 0, warm-up 1, median of {reps}");
    let mut rows = Vec::new();
    for &cube in &sizes {
        eprintln!("running {cube}^3 ...");
        rows.push(common::bench::bench_size(cube, reps, with_amg, with_cpu));
    }
    println!();
    println!("| unknowns | CPU 1 thr CG-Jacobi [s] (its) | CPU {} thr [s] | CPU 1 thr AMG-CG setup+solve [s] (its) | GPU resident CG-Jacobi [s] (its) | GPU one-shot [s] | speedup resident vs 1 thr | speedup one-shot vs 1 thr | speedup one-shot vs 1 thr AMG |", rows.first().map(|r| r.cpun_threads).unwrap_or(0));
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in &rows {
        let amg = r.amg1_setup_s + r.amg1_solve_s;
        println!(
            "| {} | {:.4} ({}) | {:.4} | {:.4} + {:.4} = {:.4} ({}) | {:.4} ({}) | {:.4} | {:.2}x | {:.2}x | {:.2}x |",
            r.n,
            r.cpu1_s,
            r.cpu1_iters,
            r.cpun_s,
            r.amg1_setup_s,
            r.amg1_solve_s,
            amg,
            r.amg1_iters,
            r.gpu_resident_s,
            r.gpu_iters,
            r.gpu_oneshot_s,
            r.cpu1_s / r.gpu_resident_s,
            r.cpu1_s / r.gpu_oneshot_s,
            amg / r.gpu_oneshot_s
        );
    }
    println!();
    println!("| unknowns | nnz | matrix upload [ms] (bytes) | b,x upload + buffers [ms] | x download [ms] | device iterations [ms] | GPU Chebyshev(4) resident [s] (its) | per-iteration GPU [us] | per-iteration CPU 1 thr [us] |");
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in &rows {
        println!(
            "| {} | {} | {:.2} ({}) | {:.2} | {:.2} | {:.2} | {:.4} ({}) | {:.1} | {:.1} |",
            r.n,
            r.nnz,
            r.up_matrix_s * 1e3,
            r.up_bytes,
            r.up_vec_s * 1e3,
            r.down_s * 1e3,
            r.solve_s * 1e3,
            r.gpu_cheb_resident_s,
            r.gpu_cheb_iters,
            r.solve_s / r.gpu_iters.max(1) as f64 * 1e6,
            r.cpu1_s / r.cpu1_iters.max(1) as f64 * 1e6
        );
    }
}
