//! ADR 0038 checks S1 (CG) and S2 (BiCGStab): device vs CPU solution, iteration counts and true residual.
#![cfg(feature = "cuda")]

mod common;
use common::*;
use rcfd_gpu::DeviceSystem;
use rcfd_linalg::{CpuBackend, CsrMatrix, GpuPrecond, LinearBackend, LinearOptions, SolveControl};

fn control() -> SolveControl {
    SolveControl {
        tol_abs: 0.0,
        tol_rel: 1e-12,
        max_iter: 5000,
    }
}

fn rhs(a: &CsrMatrix) -> Vec<f64> {
    let xe: Vec<f64> = (0..a.n).map(|i| ((i * 37 + 11) % 17) as f64 / 8.0 - 1.0).collect();
    matvec_ref(a, &xe)
}

fn true_residual(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let ax = matvec_ref(a, x);
    let r: f64 = ax.iter().zip(b).map(|(p, q)| (p - q) * (p - q)).sum::<f64>().sqrt();
    let bn: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    r / bn
}

fn run(name: &str, a: &CsrMatrix, symmetric: bool) {
    let b = rhs(a);
    let c = control();
    for (pname, precond) in [
        ("none", GpuPrecond::None),
        ("jacobi", GpuPrecond::Jacobi),
        ("chebyshev4", GpuPrecond::Chebyshev),
    ] {
        let o = LinearOptions {
            precond,
            chebyshev_degree: 4,
            ..LinearOptions::default()
        };
        let mut xc = vec![0.0; a.n];
        let sc = if symmetric {
            CpuBackend.cg(a, &b, &mut xc, &c, &o).unwrap()
        } else {
            CpuBackend.bicgstab(a, &b, &mut xc, &c, &o).unwrap()
        };
        let mut xg = vec![0.0; a.n];
        let mut sys = DeviceSystem::new(a, &o).unwrap();
        let (sg, tm) = if symmetric {
            sys.cg(&b, &mut xg, &c).unwrap()
        } else {
            sys.bicgstab(&b, &mut xg, &c).unwrap()
        };
        let diff = rel2(&xg, &xc);
        let res = true_residual(a, &b, &xg);
        println!(
            "{} {name} {pname}: iters cpu {} gpu {} | rel2(x_gpu,x_cpu) {diff:.3e} | true res gpu {res:.3e} | gpu solve {:.1} ms",
            if symmetric { "S1 CG" } else { "S2 BiCGStab" },
            sc.iterations,
            sg.iterations,
            tm.solve * 1e3
        );
        assert!(sc.converged && sg.converged, "{name} {pname}: cpu {sc:?} gpu {sg:?}");
        assert!(diff <= 1e-10, "{name} {pname}: solution difference {diff:e}");
        assert!(
            (sc.iterations as i64 - sg.iterations as i64).abs() <= 2,
            "{name} {pname}: iterations cpu {} gpu {}",
            sc.iterations,
            sg.iterations
        );
        assert!(res <= 1e-10, "{name} {pname}: true residual {res:e}");
    }
}

#[test]
fn s1_cg_matches_cpu() {
    if rcfd_gpu::skip_without_device("s1_cg") {
        return;
    }
    run("poisson2d_64", &grid_matrix(64, 64, 1, 0.0), true);
    run("poisson3d_24", &grid_matrix(24, 24, 24, 0.0), true);
    run("random_spd_20000", &random_spd(20_000, 5), true);
}

#[test]
fn s2_bicgstab_matches_cpu() {
    if rcfd_gpu::skip_without_device("s2_bicgstab") {
        return;
    }
    run("convdiff2d_100", &grid_matrix(100, 100, 1, 2.0), false);
    run("convdiff3d_20", &grid_matrix(20, 20, 20, 2.0), false);
}
