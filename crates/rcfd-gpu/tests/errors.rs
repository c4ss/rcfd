//! ADR 0038 check E1, API half: located errors and non-panicking failure modes.
#![cfg(feature = "cuda")]

mod common;
use common::*;
use rcfd_gpu::CudaBackend;
use rcfd_linalg::{check_index_range, CsrMatrix, LinearBackend, LinearOptions, SolveControl};

#[test]
fn e1_shape_errors_are_located() {
    if rcfd_gpu::skip_without_device("e1_shape_errors") {
        return;
    }
    let a = grid_matrix(5, 5, 1, 0.0);
    let (c, o) = (SolveControl::default(), LinearOptions::default());
    let mut x = vec![0.0; 25];
    let e = CudaBackend.cg(&a, &[1.0; 24], &mut x, &c, &o).unwrap_err();
    println!("E1 b mismatch: {e}");
    assert!(
        e.message.contains("right-hand side") && e.message.contains("24") && e.message.contains("25"),
        "{e}"
    );
    let mut short = vec![0.0; 3];
    let e = CudaBackend.bicgstab(&a, &[1.0; 25], &mut short, &c, &o).unwrap_err();
    println!("E1 x mismatch: {e}");
    assert!(e.message.contains("solution vector x"), "{e}");
    let rect = CsrMatrix::rect(2, 3, vec![0, 1, 2], vec![0, 2], vec![1.0, 1.0]);
    let e = CudaBackend.cg(&rect, &[1.0, 1.0], &mut [0.0, 0.0], &c, &o).unwrap_err();
    println!("E1 non-square: {e}");
    assert!(e.message.contains("square"), "{e}");
    let bad = CsrMatrix::rect(2, 2, vec![0, 1, 2], vec![0, 5], vec![1.0, 1.0]);
    let e = CudaBackend.cg(&bad, &[1.0, 1.0], &mut [0.0, 0.0], &c, &o).unwrap_err();
    println!("E1 column out of range: {e}");
    assert!(e.message.contains("column index"), "{e}");
    // size validation with synthetic counts, no allocation
    let e = check_index_range(10, i32::MAX as usize + 1).unwrap_err();
    assert!(e.message.contains("nonzero count"), "{e}");
    let e = check_index_range(i32::MAX as usize + 1, 10).unwrap_err();
    assert!(e.message.contains("row count"), "{e}");
}

#[test]
fn e1_exhausted_iterations_and_nan_do_not_panic_or_hang() {
    if rcfd_gpu::skip_without_device("e1_exhausted") {
        return;
    }
    let a = grid_matrix(60, 60, 1, 0.0);
    let b = vec![1.0; a.n];
    let o = LinearOptions::default();
    let c = SolveControl {
        tol_abs: 0.0,
        tol_rel: 1e-14,
        max_iter: 3,
    };
    let mut x = vec![0.0; a.n];
    let s = CudaBackend.cg(&a, &b, &mut x, &c, &o).unwrap();
    println!("E1 exhausted: {s:?}");
    assert!(!s.converged && s.iterations == 3);
    let mut bn = b.clone();
    bn[10] = f64::NAN;
    let c2 = SolveControl { max_iter: 50, ..c };
    for bicg in [false, true] {
        let mut x = vec![0.0; a.n];
        let s = if bicg {
            CudaBackend.bicgstab(&a, &bn, &mut x, &c2, &o).unwrap()
        } else {
            CudaBackend.cg(&a, &bn, &mut x, &c2, &o).unwrap()
        };
        println!("E1 NaN rhs (bicgstab={bicg}): {s:?}");
        assert!(!s.converged && s.iterations <= 50);
    }
}
