//! ADR 0038 check M1: no device-buffer leaks across repeated solves. This binary holds a single test because the
//! allocator counters are process-wide.
#![cfg(feature = "cuda")]

mod common;
use common::*;
use rcfd_gpu::{alloc_stats, CudaBackend, DeviceSystem};
use rcfd_linalg::{LinearBackend, LinearOptions, SolveControl};

#[test]
fn m1_no_leaks_across_repeated_solves() {
    if rcfd_gpu::skip_without_device("m1_leaks") {
        return;
    }
    let a = grid_matrix(100, 100, 1, 0.0); // n = 10000
    let b = vec![1.0; a.n];
    let c = SolveControl {
        tol_abs: 0.0,
        tol_rel: 1e-8,
        max_iter: 2000,
    };
    let o = LinearOptions::default();
    // warm-up: module, scratch and any cached state are created here
    let mut x = vec![0.0; a.n];
    assert!(CudaBackend.cg(&a, &b, &mut x, &c, &o).unwrap().converged);
    let base = alloc_stats();
    let free0 = rcfd_gpu::device_free_bytes();
    for _ in 0..200 {
        let mut x = vec![0.0; a.n];
        assert!(CudaBackend.cg(&a, &b, &mut x, &c, &o).unwrap().converged);
    }
    for _ in 0..50 {
        let mut sys = DeviceSystem::new(&a, &o).unwrap();
        let mut x = vec![0.0; a.n];
        assert!(sys.cg(&b, &mut x, &c).unwrap().0.converged);
        assert!(sys.bicgstab(&b, &mut x, &c).unwrap().0.converged);
        drop(sys);
    }
    let end = alloc_stats();
    let free1 = rcfd_gpu::device_free_bytes();
    println!(
        "M1: before live {} B / {} buffers; after live {} B / {} buffers; allocations made {} ; peak {} B; driver free memory {:?} -> {:?}",
        base.live_bytes,
        base.live_buffers,
        end.live_bytes,
        end.live_buffers,
        end.total_allocations - base.total_allocations,
        end.peak_bytes,
        free0,
        free1
    );
    assert!(
        end.total_allocations > base.total_allocations + 250,
        "the loop did not allocate"
    );
    assert_eq!(end.live_bytes, base.live_bytes);
    assert_eq!(end.live_buffers, base.live_buffers);
}
