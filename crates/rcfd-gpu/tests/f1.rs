//! ADR 0038 check F1 (the only performance criterion): at 171^3 unknowns the one-shot device CG (matrix upload,
//! solve, download; median of 5 after 1 warm-up) is faster than the 1-thread CPU CG with the same preconditioner and
//! tolerance. Several minutes; run with `-- --ignored --nocapture`.
#![cfg(feature = "cuda")]

mod common;

#[test]
#[ignore = "several minutes: 171^3 unknowns, median of 5 on one CPU thread"]
fn f1_gpu_vs_one_thread() {
    if rcfd_gpu::skip_without_device("f1") {
        return;
    }
    let r = common::bench::bench_size(171, 5, false, true);
    println!(
        "F1: n = {} ; CPU 1 thread {:.3} s ({} its) ; GPU one-shot {:.3} s ({} its) ; GPU resident {:.3} s ; speedup one-shot {:.2}x, resident {:.2}x",
        r.n, r.cpu1_s, r.cpu1_iters, r.gpu_oneshot_s, r.gpu_iters, r.gpu_resident_s, r.cpu1_s / r.gpu_oneshot_s, r.cpu1_s / r.gpu_resident_s
    );
    assert!(
        r.gpu_oneshot_s < r.cpu1_s,
        "GPU one-shot {:.3} s is not faster than 1 CPU thread {:.3} s",
        r.gpu_oneshot_s,
        r.cpu1_s
    );
}
