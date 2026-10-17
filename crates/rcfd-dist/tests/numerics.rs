//! ADR 0058 checks B4 (matvec), B5 (reductions, determinism), B6/B7 (solves, P = 1 bit identity), B8 (iteration counts),
//! B9 (strong scaling, reported only).

mod common;

use common::*;
use rcfd_dist::krylov::gdot;
use rcfd_dist::{
    run_ranks, solve_distributed, solve_rank, Communicator, DistCsr, DistOperator, Method, Precond, SerialComm,
};
use rcfd_linalg::precond::{Identity, Ilu0, Jacobi, Preconditioner};
use rcfd_linalg::solvers::{bicgstab, cg, SolveControl, SolveStats};
use rcfd_linalg::vecops::{dot, norm2};
use rcfd_linalg::CsrMatrix;

const U: f64 = 1.1102230246251565e-16; // 2^-53

fn gamma(k: usize) -> f64 {
    k as f64 * U / (1.0 - k as f64 * U)
}

fn ctl() -> SolveControl {
    SolveControl {
        tol_abs: 1e-30,
        tol_rel: 1e-10,
        max_iter: 2000,
    }
}

fn serial(a: &CsrMatrix, b: &[f64], method: Method, precond: Precond, c: &SolveControl) -> (Vec<f64>, SolveStats) {
    let m: Box<dyn Preconditioner> = match precond {
        Precond::None => Box::new(Identity),
        Precond::Jacobi => Box::new(Jacobi::new(&a.diag())),
        Precond::BlockJacobi => Box::new(Ilu0::new(a)),
    };
    let mut x = vec![0.0; a.n];
    let st = match method {
        Method::Cg => cg(a, b, &mut x, m.as_ref(), c),
        Method::BiCgStab => bicgstab(a, b, &mut x, m.as_ref(), c),
    };
    (x, st)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// B4: distributed matvec versus serial.
#[test]
fn b4_matvec() {
    let mesh = cavity(48);
    let a = poisson(&mesh);
    let x = lcg(a.n, 7);
    let mut ys = vec![0.0; a.n];
    a.matvec(&x, &mut ys);
    let mut worst_ratio: f64 = 0.0;
    for p in [1usize, 2, 4, 8] {
        let dec = decompose(&mesh, p);
        let mats: Vec<DistCsr> = (0..p)
            .map(|r| DistCsr::from_global(&a, &dec, r).expect("dist matrix"))
            .collect();
        let parts = run_ranks(p, |comm| {
            let m = &mats[comm.rank()];
            let op = DistOperator::new(m, comm);
            let xl: Vec<f64> = m.owned.iter().map(|&c| x[c]).collect();
            let mut y = vec![0.0; m.n_owned];
            op.try_apply(&xl, &mut y).expect("apply");
            y
        });
        let mut yd = vec![0.0; a.n];
        for (r, y) in parts.iter().enumerate() {
            for (&c, v) in mats[r].owned.iter().zip(y) {
                yd[c] = *v;
            }
        }
        if p == 1 {
            assert_eq!(bits(&yd), bits(&ys), "P=1 matvec must be bit-identical");
        }
        let mut n_diff = 0;
        for i in 0..a.n {
            let k = a.row_ptr[i + 1] - a.row_ptr[i];
            let absax: f64 = (a.row_ptr[i]..a.row_ptr[i + 1])
                .map(|j| (a.val[j] * x[a.col[j]]).abs())
                .sum();
            let bound = 2.0 * gamma(k) * absax;
            let d = (yd[i] - ys[i]).abs();
            assert!(d <= bound, "P={p} row {i}: |dy| {d:e} > bound {bound:e}");
            if d != 0.0 {
                n_diff += 1;
            }
            worst_ratio = worst_ratio.max(if bound > 0.0 { d / bound } else { 0.0 });
        }
        println!("B4 P={p}: rows differing from serial in any bit: {n_diff} of {}", a.n);
    }
    println!("B4 worst |dy|/bound = {worst_ratio:e}");
}

/// B5: distributed dot versus serial dot; P = 1 bit-identical.
#[test]
fn b5_dot_and_determinism() {
    let n = 2304;
    let a = lcg(n, 1);
    let b = lcg(n, 2);
    let ds = dot(&a, &b);
    let sabs: f64 = a.iter().zip(&b).map(|(x, y)| (x * y).abs()).sum();
    let mesh = cavity(48);
    for p in [1usize, 2, 4, 8] {
        let dec = decompose(&mesh, p);
        let out = run_ranks(p, |comm| {
            let s = &dec.subdomains[comm.rank()];
            let al: Vec<f64> = s.owned.iter().map(|&c| a[c]).collect();
            let bl: Vec<f64> = s.owned.iter().map(|&c| b[c]).collect();
            gdot(comm, &al, &bl).expect("gdot")
        });
        for w in &out {
            assert_eq!(w.to_bits(), out[0].to_bits(), "P={p}: ranks disagree");
        }
        if p == 1 {
            assert_eq!(out[0].to_bits(), ds.to_bits(), "P=1 dot bit-identical");
        }
        let diff = (out[0] - ds).abs();
        assert!(
            diff <= gamma(n) * sabs,
            "P={p}: dot differs by {diff:e} > {:e}",
            gamma(n) * sabs
        );
        println!("B5 P={p}: |dot_d - dot_s| = {diff:e} (bound {:e})", gamma(n) * sabs);
    }
}

fn solve_with_pools(
    a: &CsrMatrix,
    b: &[f64],
    mesh: &rcfd_mesh::mesh::Mesh,
    p: usize,
    threads: usize,
) -> (Vec<u64>, SolveStats) {
    let dec = decompose(mesh, p);
    let mats: Vec<DistCsr> = (0..p)
        .map(|r| DistCsr::from_global(a, &dec, r).expect("dist matrix"))
        .collect();
    let out = run_ranks(p, |comm| {
        let m = &mats[comm.rank()];
        let bl: Vec<f64> = m.owned.iter().map(|&c| b[c]).collect();
        let mut xl = vec![0.0; m.n_owned];
        // a private rayon pool per rank: the blocking receives happen on the rank thread's own pool worker only
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("pool");
        let st = pool
            .install(|| solve_rank(m, comm, &bl, &mut xl, Method::Cg, Precond::BlockJacobi, &ctl()))
            .expect("solve");
        (xl, st)
    });
    let mut x = vec![0.0; a.n];
    for (r, (xl, _)) in out.iter().enumerate() {
        for (&c, v) in mats[r].owned.iter().zip(xl) {
            x[c] = *v;
        }
    }
    (bits(&x), out[0].1)
}

/// B5 run-to-run: 20 repeats at fixed P are bit-identical, also across rayon pool sizes.
#[test]
fn b5_run_to_run() {
    let mesh = cavity(48);
    let a = poisson(&mesh);
    let b = rhs(&mesh);
    for p in [2usize, 4, 8] {
        let (x0, s0) = solve_with_pools(&a, &b, &mesh, p, 2);
        for rep in 0..20 {
            let th = [1usize, 2, 4][rep % 3];
            let (x, s) = solve_with_pools(&a, &b, &mesh, p, th);
            assert_eq!(x, x0, "P={p} repeat {rep} (pool {th}) differs");
            assert_eq!(s, s0, "P={p} repeat {rep} stats differ");
        }
    }
}

fn l2diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

/// B6, B7, B8 in one sweep over methods, preconditioners and P.
#[test]
fn b6_b7_b8_solves() {
    let mesh = cavity(48);
    let a = poisson(&mesh);
    let b = rhs(&mesh);
    let c = ctl();
    let bn = norm2(&b);
    let mut table = Vec::new();
    for method in [Method::Cg, Method::BiCgStab] {
        for precond in [Precond::None, Precond::Jacobi, Precond::BlockJacobi] {
            let (xs, ss) = serial(&a, &b, method, precond, &c);
            assert!(ss.converged, "serial {method:?}/{precond:?} did not converge");
            let xn = norm2(&xs);
            for p in [1usize, 2, 4, 8] {
                let dec = decompose(&mesh, p);
                let tag = format!("{method:?}/{precond:?}/P={p}");
                let x0 = vec![0.0; a.n];
                let r = solve_distributed(&a, &b, &x0, &dec, method, precond, &c, false).expect("solve");
                let st = r.stats[0];
                for s in &r.stats {
                    assert_eq!(*s, st, "{tag}: ranks disagree on stats");
                }
                assert!(st.converged, "{tag}: not converged");
                // true residual
                let mut res = vec![0.0; a.n];
                a.residual(&b, &r.x, &mut res);
                let rn = norm2(&res);
                assert!(
                    rn <= 1.01e-10 * bn,
                    "{tag}: true residual {:e} > {:e}",
                    rn / bn,
                    1.01e-10
                );
                // difference to serial (B6)
                let rel = l2diff(&r.x, &xs) / xn;
                assert!(rel <= 2e-7, "{tag}: relative L2 difference {rel:e} > 2e-7");
                println!(
                    "B6 {tag}: iters {} (serial {}), true res/|b| {:e}, rel diff to serial {:e}, bit-identical {}",
                    st.iterations,
                    ss.iterations,
                    rn / bn,
                    rel,
                    bits(&r.x) == bits(&xs)
                );
                if p == 1 {
                    // B7: Serial path and in-process P=1 threads both bit-identical to serial linalg
                    let rt = solve_distributed(&a, &b, &x0, &dec, method, precond, &c, true).expect("solve");
                    for (nm, rr) in [("serial-comm", &r), ("threads", &rt)] {
                        assert_eq!(bits(&rr.x), bits(&xs), "{tag} B7 {nm}: solution bits");
                        assert_eq!(rr.stats[0].iterations, ss.iterations, "{tag} B7 {nm}: iterations");
                        assert_eq!(
                            rr.stats[0].final_residual.to_bits(),
                            ss.final_residual.to_bits(),
                            "{tag} B7 {nm}"
                        );
                    }
                }
                table.push((method, precond, p, st.iterations, ss.iterations));
            }
        }
    }
    // B8 iteration counts
    let find =
        |m: Method, pc: Precond, p: usize| table.iter().find(|t| t.0 == m && t.1 == pc && t.2 == p).expect("entry");
    for p in [2usize, 4, 8] {
        let t = find(Method::Cg, Precond::Jacobi, p);
        assert!(
            (t.3 as i64 - t.4 as i64).abs() <= 2,
            "point Jacobi CG P={p}: {} vs serial {}",
            t.3,
            t.4
        );
        for m in [Method::Cg, Method::BiCgStab] {
            let t = find(m, Precond::BlockJacobi, p);
            assert!(
                t.3 as f64 <= 2.5 * t.4 as f64,
                "block-Jacobi {m:?} P={p}: {} iterations > 2.5 x serial ILU0 {}",
                t.3,
                t.4
            );
        }
    }
    assert!(find(Method::Cg, Precond::BlockJacobi, 8).3 <= find(Method::Cg, Precond::Jacobi, 8).3);
    for t in &table {
        println!(
            "B8 {:?}/{:?}/P={}: iterations {} (serial same preconditioner {})",
            t.0, t.1, t.2, t.3, t.4
        );
    }
}

/// B9: strong scaling, REPORTED only.
#[test]
#[ignore = "timing report only; run with --ignored --nocapture"]
fn b9_strong_scaling_report() {
    let mesh = cavity(192);
    let a = poisson(&mesh);
    let b = rhs(&mesh);
    let c = SolveControl {
        tol_abs: 1e-30,
        tol_rel: 1e-8,
        max_iter: 5000,
    };
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);
    println!("B9 strong scaling: cavity 192x192 ({} cells), CG + block-Jacobi ILU0, tol_rel 1e-8, host logical cores {cores}", a.n);
    println!("B9 NOTE: in-process threads share one machine's memory bandwidth and rayon pool; this is NOT a cluster measurement.");
    let mut t1 = 0.0;
    for p in [1usize, 2, 4, 8] {
        let dec = decompose(&mesh, p);
        let mut times = Vec::new();
        let mut iters = 0;
        for _ in 0..3 {
            let r = solve_distributed(
                &a,
                &b,
                &vec![0.0; a.n],
                &dec,
                Method::Cg,
                Precond::BlockJacobi,
                &c,
                false,
            )
            .expect("solve");
            assert!(r.stats[0].converged);
            iters = r.stats[0].iterations;
            times.push(r.solve_secs);
        }
        times.sort_by(|x, y| x.total_cmp(y));
        let med = times[1];
        if p == 1 {
            t1 = med;
        }
        println!(
            "B9 P={p}: iterations {iters}, median solve {:.4} s, speed-up {:.2}, efficiency {:.2}",
            med,
            t1 / med,
            t1 / med / p as f64
        );
    }
}

#[test]
fn serial_comm_is_identity() {
    let c = SerialComm;
    assert_eq!(c.rank(), 0);
    assert_eq!(c.size(), 1);
    assert_eq!(c.allreduce_sum(2.5).expect("sum"), 2.5);
    assert!(c.barrier().is_ok());
}
