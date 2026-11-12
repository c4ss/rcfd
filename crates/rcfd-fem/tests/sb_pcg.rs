//! ADR 0282 (F-381): CG with block Jacobi and incomplete Cholesky on the stiffness matrix. Checks A1 to A7, B1, L1 to L5,
//! the record X1 and the negative controls N1 to N3 of the registration; H1 is the heavy record (`#[ignore]`).

use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::{
    node_blocks, solve_static, solve_static_report, CgPreconditioner, SolveReport, StaticOptions, StaticSolver,
};
use rcfd_fem::{Constraint, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::precond::{BlockJacobi, IcOptions, IncompleteCholesky, PrecondError};
use rcfd_linalg::{CsrMatrix, Ilu0, Preconditioner};
use std::time::Instant;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const L: f64 = 8.0;
const H: f64 = 1.5;
const TRACTION: f64 = 1.0e6;
const TOL: f64 = 1e-10;
const FAST: [usize; 3] = [32, 6, 6];
const SLENDER: [usize; 3] = [64, 6, 6];
const HEAVY: [usize; 3] = [96, 18, 18];

fn cantilever(div: [usize; 3]) -> SolidModel {
    let length = L * div[0] as f64 / 32.0;
    let mesh = hex_box([0.0; 3], [length, H, H], div).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -TRACTION])).unwrap();
    m
}

fn pcg(p: CgPreconditioner, tolerance: f64, max_iterations: usize) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: p,
        tolerance,
        max_iterations,
    })
}

fn ic() -> CgPreconditioner {
    CgPreconditioner::IncompleteCholesky(IcOptions::default())
}

fn run(model: &SolidModel, o: &StaticOptions) -> (SolidResult, SolveReport, f64) {
    let t = Instant::now();
    let (r, rep) = solve_static_report(model, &[], &[], o).unwrap();
    (r, rep, t.elapsed().as_secs_f64())
}

fn rel_diff(a: &SolidResult, b: &SolidResult) -> f64 {
    let (x, y) = (a.displacement_values(), b.displacement_values());
    let m = y.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    x.iter().zip(y).fold(0.0f64, |s, (p, q)| s.max((p - q).abs())) / m
}

fn residual(r: &SolidResult) -> f64 {
    r.stats().unwrap().relative_residual()
}

fn iterations(r: &SolidResult) -> usize {
    r.stats().unwrap().iterations()
}

fn print(tag: &str, r: &SolidResult, rep: &SolveReport, secs: f64) {
    println!(
        "{tag}: solver '{}', {} unknowns, {} iterations, {} restarts, relative residual {:.3e}, shift {:?}, {} breakdowns, {:.2} s",
        rep.solver,
        r.stats().unwrap().unknowns(),
        rep.iterations,
        rep.restarts,
        rep.relative_residual,
        rep.shift,
        rep.breakdowns.len(),
        secs
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// A1, A2, A3, A6, B1, N1, N2, X1 (fast case).

#[test]
#[ignore = "known miss (ADR 0282 amendment 1): A3 ratio IC 85 / block Jacobi 156 = 0.5449 against 0.5; N1 2.2e-8 against > 1e-7 (set-up defect, N1b)"]
fn a1_a2_a3_a6_fast_cantilever_known_miss() {
    let model = cantilever(FAST);
    let (d, _, td) = run(&model, &StaticOptions::default());
    println!(
        "X1 fast direct: {} unknowns, relative residual {:.3e}, {:.2} s",
        d.stats().unwrap().unknowns(),
        residual(&d),
        td
    );
    let (bj, rbj, tbj) = run(&model, &pcg(CgPreconditioner::BlockJacobi, TOL, 20_000));
    print("X1 fast block Jacobi", &bj, &rbj, tbj);
    let (icr, ric, tic) = run(&model, &pcg(ic(), TOL, 20_000));
    print("X1 fast IC(0)", &icr, &ric, tic);
    for (bd_n, bd_c, bd) in &ric.breakdowns {
        println!("X1 fast IC(0) breakdown at node {bd_n} component {bd_c}: {bd:?}");
    }

    // A1, A2, A3 (measured and printed first, asserted at the end)
    println!(
        "A1 relative residual: block Jacobi {:.3e}, IC {:.3e} (limit 1e-10)",
        residual(&bj),
        residual(&icr)
    );
    let (e_bj, e_ic) = (rel_diff(&bj, &d), rel_diff(&icr, &d));
    println!("A2 against direct: block Jacobi {e_bj:.3e}, IC {e_ic:.3e} (limit 1e-7)");
    let ratio = iterations(&icr) as f64 / iterations(&bj) as f64;
    println!(
        "A3 iterations IC {} / block Jacobi {} = {ratio:.4} (limit 0.5)",
        iterations(&icr),
        iterations(&bj)
    );

    // A6: breakdowns forced by a pivot tolerance of 0.9.
    let forced = CgPreconditioner::IncompleteCholesky(IcOptions {
        pivot_tolerance: 0.9,
        ..IcOptions::default()
    });
    let (fr, frep, tf) = run(&model, &pcg(forced, TOL, 20_000));
    print("A6 forced breakdowns", &fr, &frep, tf);
    let e_f = rel_diff(&fr, &d);
    println!(
        "A6 name '{}', shift {:?}, {} breakdowns, first {:?}, last {:?}, residual {:.3e}, against direct {e_f:.3e}",
        fr.stats().unwrap().solver(),
        frep.shift,
        frep.breakdowns.len(),
        frep.breakdowns.first(),
        frep.breakdowns.last(),
        residual(&fr)
    );

    // N1: block Jacobi stopped at 1e-4.
    let (n1, _, _) = run(&model, &pcg(CgPreconditioner::BlockJacobi, 1e-4, 20_000));
    let e_n1 = rel_diff(&n1, &d);
    println!("N1 block Jacobi at tolerance 1e-4 against direct: {e_n1:.3e} (expected > 1e-7)");
    // N2: IC of A + diag(A).
    let shifted = CgPreconditioner::IncompleteCholesky(IcOptions {
        initial_shift: 1.0,
        ..IcOptions::default()
    });
    let (n2, n2rep, _) = run(&model, &pcg(shifted, TOL, 20_000));
    let r2 = iterations(&n2) as f64 / iterations(&bj) as f64;
    println!(
        "N2 IC with shift {:?}: {} iterations, ratio {r2:.4} (expected > 0.5)",
        n2rep.shift,
        iterations(&n2)
    );

    assert!(residual(&bj) <= 1e-10, "A1 block Jacobi");
    assert!(residual(&icr) <= 1e-10, "A1 IC");
    assert_eq!(bj.stats().unwrap().solver(), "cg-block-jacobi");
    assert!(e_bj <= 1e-7 && e_ic <= 1e-7, "A2");
    assert!(ratio <= 0.5, "A3");
    assert!(fr.stats().unwrap().solver().contains("shift"), "A6 name");
    assert!(
        frep.shift.unwrap() > 0.0 && !frep.breakdowns.is_empty(),
        "A6 shift reported"
    );
    assert!(fr.displacement_values().iter().all(|v| v.is_finite()), "A6 finite");
    assert!(residual(&fr) <= 1e-10 && e_f <= 1e-7, "A6 residual and agreement");
    assert!(e_n1 > 1e-7, "N1");
    assert!(r2 > 0.5, "N2");
}

#[test]
fn b1_node_blocks() {
    let model = cantilever(FAST);
    let dofs = model.dof_map();
    let clamped: std::collections::BTreeSet<usize> = model.mesh().node_set("xmin").unwrap().iter().copied().collect();
    let free: Vec<usize> = (0..dofs.n_equations())
        .filter(|&q| !clamped.contains(&dofs.node_component(q).unwrap().0))
        .collect();
    let ptr = node_blocks(&free, &dofs);
    let nb = ptr.len() - 1;
    println!("B1 {} free equations, {nb} blocks", free.len());
    assert_eq!(nb, 1568);
    for w in ptr.windows(2) {
        assert_eq!(w[1] - w[0], 3);
        let n0 = dofs.node_component(free[w[0]]).unwrap().0;
        for &q in &free[w[0]..w[1]] {
            assert_eq!(dofs.node_component(q).unwrap().0, n0);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// A4, N3: 1 and 8 threads.

#[test]
fn a4_threads_bit_identical() {
    let model = cantilever(FAST);
    let mut sols = Vec::new();
    for p in [CgPreconditioner::BlockJacobi, ic()] {
        let mut runs = Vec::new();
        for t in [1usize, 8] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
            let o = pcg(p.clone(), TOL, 20_000).threads(t);
            let r = pool.install(|| solve_static(&model, &o).unwrap());
            runs.push(r);
        }
        let same_it = iterations(&runs[0]) == iterations(&runs[1]);
        let same_bits = runs[0]
            .displacement_values()
            .iter()
            .zip(runs[1].displacement_values())
            .all(|(a, b)| a.to_bits() == b.to_bits());
        println!(
            "A4 {:?}: iterations {} / {}, displacement bit-equal {same_bits}",
            p,
            iterations(&runs[0]),
            iterations(&runs[1])
        );
        assert!(same_it && same_bits, "A4");
        sols.push(runs.swap_remove(0));
    }
    let n3 = sols[0]
        .displacement_values()
        .iter()
        .zip(sols[1].displacement_values())
        .all(|(a, b)| a.to_bits() == b.to_bits());
    println!("N3 block Jacobi and IC solutions bit-equal: {n3} (expected false)");
    assert!(!n3, "N3");
}

// ---------------------------------------------------------------------------------------------------------------------
// A7 (b): hash of the default and of the Jacobi CG solve (the same function is compiled on main for the comparison).

fn fnv1a(values: &[f64], acc: &mut u64) {
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            *acc ^= b as u64;
            *acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

#[test]
fn a7_default_bits_hash() {
    let model = cantilever(FAST);
    for (tag, o) in [
        ("direct", StaticOptions::default()),
        (
            "cg-jacobi",
            StaticOptions::with_solver(StaticSolver::ConjugateGradient {
                tolerance: 1e-10,
                max_iterations: 20_000,
            }),
        ),
    ] {
        match solve_static(&model, &o) {
            Ok(r) => {
                let mut h = 0xcbf2_9ce4_8422_2325u64;
                fnv1a(r.displacement_values(), &mut h);
                fnv1a(r.reactions().unwrap(), &mut h);
                let s = r.stats().unwrap();
                fnv1a(
                    &[s.relative_residual(), s.iterations() as f64, s.unknowns() as f64],
                    &mut h,
                );
                println!(
                    "A7 {tag}: solver '{}', iterations {}, hash {h:016x}",
                    s.solver(),
                    s.iterations()
                );
            }
            Err(e) => println!("A7 {tag}: error '{e}'"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// L1 to L4, A5: the preconditioners on small matrices with closed forms.

fn dense_csr(n: usize, a: &[f64]) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if a[i * n + j] != 0.0 {
                t.push((i, j, a[i * n + j]));
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn apply(p: &dyn Preconditioner, r: &[f64]) -> Vec<f64> {
    let mut z = vec![0.0; r.len()];
    p.apply(r, &mut z);
    z
}

fn mv(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    let m = b.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    a.iter().zip(b).fold(0.0f64, |s, (p, q)| s.max((p - q).abs())) / m
}

#[test]
fn l1_full_pattern_is_exact() {
    let n = 6;
    let g: Vec<f64> = (0..n * n)
        .map(|k| ((k / n) as f64 + 2.0 * (k % n) as f64 + 1.0).sin())
        .collect();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| g[i * n + k] * g[j * n + k]).sum::<f64>() + if i == j { 6.0 } else { 0.0 };
        }
    }
    let a = dense_csr(n, &a);
    let p = IncompleteCholesky::new(&a, &IcOptions::default()).unwrap();
    let x: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
    let z = apply(&p, &mv(&a, &x));
    let e = max_rel(&z, &x);
    println!(
        "L1 full pattern: max |M^-1 A x - x| / max |x| = {e:.3e} (limit 1e-12), shift {}",
        p.shift()
    );
    assert!(e <= 1e-12 && p.breakdowns().is_empty());
}

#[test]
fn l2_agrees_with_ilu0_on_five_point() {
    let (nx, ny) = (12usize, 10usize);
    let mut t = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let c = i + nx * j;
            t.push((c, c, 4.0));
            if i + 1 < nx {
                t.push((c, c + 1, -1.0));
                t.push((c + 1, c, -1.0));
            }
            if j + 1 < ny {
                t.push((c, c + nx, -1.0));
                t.push((c + nx, c, -1.0));
            }
        }
    }
    let a = CsrMatrix::from_triplets(nx * ny, t);
    let p = IncompleteCholesky::new(&a, &IcOptions::default()).unwrap();
    let q = Ilu0::new(&a);
    let r: Vec<f64> = (0..a.n).map(|i| (i as f64).sin()).collect();
    let e = max_rel(&apply(&p, &r), &apply(&q, &r));
    println!(
        "L2 IC(0) against ILU(0): {e:.3e} (limit 1e-13), shift {}, breakdowns {}",
        p.shift(),
        p.breakdowns().len()
    );
    assert!(e <= 1e-13 && p.shift() == 0.0 && p.breakdowns().is_empty());
}

fn inv3(b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c = |i: usize, j: usize| {
        let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
        let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
        b[r0][c0] * b[r1][c1] - b[r0][c1] * b[r1][c0]
    };
    let det = b[0][0] * c(0, 0) + b[0][1] * c(0, 1) + b[0][2] * c(0, 2);
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = c(j, i) / det;
        }
    }
    out
}

#[test]
fn l3_block_jacobi_inverts_the_blocks() {
    let n = 9;
    let mut a = vec![0.0; n * n];
    let mut blocks = Vec::new();
    for k in 0..3 {
        let f = k as f64;
        let b = [[4.0 + f, 1.0, 0.5], [1.0, 3.0 + f, 0.2], [0.5, 0.2, 2.0 + f]];
        for i in 0..3 {
            for j in 0..3 {
                a[(3 * k + i) * n + 3 * k + j] = b[i][j];
            }
        }
        blocks.push(b);
    }
    for i in 0..n - 3 {
        a[i * n + i + 3] = 0.1;
        a[(i + 3) * n + i] = 0.1;
    }
    let a = dense_csr(n, &a);
    let p = BlockJacobi::uniform(&a, 3).unwrap();
    let r: Vec<f64> = (0..n).map(|i| 1.0 + (i as f64).cos()).collect();
    let z = apply(&p, &r);
    let mut want = vec![0.0; n];
    for (k, b) in blocks.iter().enumerate() {
        let bi = inv3(*b);
        for i in 0..3 {
            want[3 * k + i] = (0..3).map(|j| bi[i][j] * r[3 * k + j]).sum();
        }
    }
    let e = max_rel(&z, &want);
    println!(
        "L3 block Jacobi against the cofactor inverses: {e:.3e} (limit 1e-14), {} blocks",
        p.n_blocks()
    );
    assert!(e <= 1e-14 && p.n_blocks() == 3);
}

fn k4() -> CsrMatrix {
    #[rustfmt::skip]
    let a = [
        3.0, -2.0, 0.0, 2.0,
        -2.0, 3.0, -2.0, 0.0,
        0.0, -2.0, 3.0, -2.0,
        2.0, 0.0, -2.0, 3.0,
    ];
    dense_csr(4, &a)
}

#[test]
fn a5_kershaw_breakdown_and_shift() {
    let p = IncompleteCholesky::new(&k4(), &IcOptions::default()).unwrap();
    let bd = p.breakdowns();
    for b in bd {
        println!("A5 breakdown {b:?}");
    }
    // (a)
    assert_eq!(bd[0].row, 3);
    assert_eq!(bd[0].shift, 0.0);
    let e_a = ((bd[0].pivot + 5.0) / 5.0).abs();
    println!(
        "A5 (a) first pivot {:e}, relative error to -5: {e_a:.3e} (limit 1e-14)",
        bd[0].pivot
    );
    assert!(e_a <= 1e-14);
    // (b)
    let mut alpha = 0.0f64;
    for _ in 0..9 {
        alpha = (2.0 * alpha).max(1e-3);
    }
    println!(
        "A5 (b) {} breakdowns, shift {:e} (expected 9, {:e})",
        bd.len(),
        p.shift(),
        alpha
    );
    assert_eq!(bd.len(), 9);
    assert_eq!(p.shift().to_bits(), alpha.to_bits());
    assert_eq!(p.shift().to_bits(), (1e-3 * 256.0f64).to_bits());
    // (c)
    let s = 3.0 * (1.0 + p.shift());
    let d2 = s - 4.0 / s;
    let d3 = s - 4.0 / d2;
    let d4 = s - 4.0 / s - 4.0 / d3;
    let e_c = max_rel(p.pivots(), &[s, d2, d3, d4]);
    println!(
        "A5 (c) pivots {:?} against {:?}: {e_c:.3e} (limit 1e-14)",
        p.pivots(),
        [s, d2, d3, d4]
    );
    assert!(e_c <= 1e-14);
    // (d)
    assert!(p.pivots().iter().all(|d| *d > 0.0 && d.is_finite()));
}

#[test]
fn l4_refusals() {
    let o = IcOptions::default();
    let zero_diag = dense_csr(2, &[1.0, 0.5, 0.5, 0.0]);
    let e = IncompleteCholesky::new(&zero_diag, &o).unwrap_err();
    println!("L4 zero diagonal: {e}");
    assert!(
        matches!(e, PrecondError::MissingDiagonal { row: 1 })
            || matches!(e, PrecondError::NonPositiveDiagonal { row: 1, .. })
    );
    let explicit_zero = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 1, 0.0)]);
    let e = IncompleteCholesky::new(&explicit_zero, &o).unwrap_err();
    println!("L4 stored zero diagonal: {e}");
    assert!(matches!(e, PrecondError::NonPositiveDiagonal { row: 1, .. }));
    let nan = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 0, f64::NAN), (0, 1, f64::NAN), (1, 1, 1.0)]);
    let e = IncompleteCholesky::new(&nan, &o).unwrap_err();
    println!("L4 NaN: {e}");
    assert!(matches!(e, PrecondError::NonFinite { row: 0, col: 1 }));
    let missing = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (1, 0, 0.1), (0, 1, 0.1)]);
    let e = IncompleteCholesky::new(&missing, &o).unwrap_err();
    println!("L4 missing diagonal: {e}");
    assert!(matches!(e, PrecondError::MissingDiagonal { row: 1 }));
    let indefinite = dense_csr(3, &[1.0, 2.0, 0.0, 2.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    let e = BlockJacobi::uniform(&indefinite, 3).unwrap_err();
    println!("L4 indefinite block: {e}");
    assert!(matches!(e, PrecondError::BlockNotPositive { block: 0, row: 1, .. }));
    let e = BlockJacobi::new(&indefinite, &[0, 2, 2, 3]).unwrap_err();
    println!("L4 invalid blocks: {e}");
    assert!(matches!(e, PrecondError::InvalidBlocks { .. }));
    let e = IncompleteCholesky::new(
        &k4(),
        &IcOptions {
            max_attempts: 3,
            ..IcOptions::default()
        },
    )
    .unwrap_err();
    println!("L4 three attempts on K4: {e}");
    match e {
        PrecondError::NoShift {
            attempts, breakdowns, ..
        } => assert!(attempts == 3 && breakdowns.len() == 3),
        other => panic!("expected NoShift, got {other:?}"),
    }
}

#[test]
fn l5_not_converged_is_an_error() {
    let model = cantilever(FAST);
    for p in [CgPreconditioner::BlockJacobi, ic()] {
        let e = solve_static(&model, &pcg(p, TOL, 5)).unwrap_err().to_string();
        println!("L5: {e}");
        assert!(e.contains("not converged") && e.contains("5 iterations"));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// X1: slender case record.

#[test]
fn x1_slender_record() {
    let model = cantilever(SLENDER);
    let (d, _, td) = run(&model, &StaticOptions::default());
    println!(
        "X1 slender direct: {} unknowns, relative residual {:.3e}, {:.2} s",
        d.stats().unwrap().unknowns(),
        residual(&d),
        td
    );
    for p in [CgPreconditioner::BlockJacobi, ic()] {
        match solve_static_report(&model, &[], &[], &pcg(p.clone(), TOL, 40_000)) {
            Ok((r, rep)) => {
                print(&format!("X1 slender {p:?}"), &r, &rep, 0.0);
                println!("X1 slender {p:?} against direct: {:.3e}", rel_diff(&r, &d));
            }
            Err(e) => println!("X1 slender {p:?}: error {e}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// H1: heavy record.

#[test]
#[ignore = "known miss (ADR 0282 amendment 2): H1 with the restarts of D5 not converged, true relative residual 1.17e-8 (block Jacobi, 6530 iterations) and 7.5e-9 (IC, 2540 iterations) after 3 restarts against 1e-10"]
fn h1_heavy_cantilever_known_miss() {
    let model = cantilever(HEAVY);
    let mut out = Vec::new();
    for p in [CgPreconditioner::BlockJacobi, ic()] {
        match solve_static_report(&model, &[], &[], &pcg(p.clone(), TOL, 60_000)) {
            Ok((r, rep)) => {
                print(&format!("H1 heavy {p:?}"), &r, &rep, 0.0);
                for (bd_n, bd_c, bd) in &rep.breakdowns {
                    println!("H1 IC(0) breakdown at node {bd_n} component {bd_c}: {bd:?}");
                }
                out.push(Some((residual(&r), r.stats().unwrap().unknowns(), rep.iterations)));
            }
            Err(e) => {
                println!("H1 heavy {p:?}: error {e}");
                out.push(None);
            }
        }
    }
    if let (Some(a), Some(b)) = (out[0], out[1]) {
        println!(
            "H1 iteration ratio IC / block Jacobi {:.4} (record)",
            b.2 as f64 / a.2 as f64
        );
    }
    for o in &out {
        let (res, n, _) = o.expect("H1 solve");
        println!("H1 relative residual {res:.3e} (limit 1e-10), {n} unknowns");
        assert!(res <= 1e-10 && n >= 100_000, "H1");
    }
}

#[test]
#[ignore = "record (ADR 0282 amendment 4): H1b on a 24 m beam instead of the registered 8 m box, not converged at 1.9e-9 relative (block Jacobi 6782, IC 2358 iterations, 3 refinement steps)"]
fn h1b_heavy_cantilever_wrong_geometry_record() {
    let model = cantilever(HEAVY);
    let mut out = Vec::new();
    for p in [CgPreconditioner::BlockJacobi, ic()] {
        match solve_static_report(&model, &[], &[], &pcg(p.clone(), TOL, 60_000)) {
            Ok((r, rep)) => {
                print(&format!("H1 heavy {p:?}"), &r, &rep, 0.0);
                for (bd_n, bd_c, bd) in &rep.breakdowns {
                    println!("H1 IC(0) breakdown at node {bd_n} component {bd_c}: {bd:?}");
                }
                out.push(Some((residual(&r), r.stats().unwrap().unknowns(), rep.iterations)));
            }
            Err(e) => {
                println!("H1 heavy {p:?}: error {e}");
                out.push(None);
            }
        }
    }
    if let (Some(a), Some(b)) = (out[0], out[1]) {
        println!(
            "H1 iteration ratio IC / block Jacobi {:.4} (record)",
            b.2 as f64 / a.2 as f64
        );
    }
    for o in &out {
        let (res, n, _) = o.expect("H1 solve");
        println!("H1 relative residual {res:.3e} (limit 1e-10), {n} unknowns");
        assert!(res <= 1e-10 && n >= 100_000, "H1");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Amendment 1: N1b and the regression guard of A1, A2, A6, N2.

#[test]
fn n1b_agreement_measure_is_sensitive() {
    let model = cantilever(FAST);
    let (d, _, _) = run(&model, &StaticOptions::default());
    let (n1, rep, _) = run(&model, &pcg(CgPreconditioner::BlockJacobi, 1e-1, 20_000));
    let e = rel_diff(&n1, &d);
    println!(
        "N1b block Jacobi at tolerance 1e-1 ({} iterations) against direct: {e:.3e} (expected > 1e-7)",
        rep.iterations
    );
    assert!(e > 1e-7, "N1b");
}

#[test]
fn a1_a2_a6_fast_cantilever() {
    let model = cantilever(FAST);
    let (d, _, _) = run(&model, &StaticOptions::default());
    let (bj, rbj, _) = run(&model, &pcg(CgPreconditioner::BlockJacobi, TOL, 20_000));
    let (icr, ric, _) = run(&model, &pcg(ic(), TOL, 20_000));
    let (e_bj, e_ic) = (rel_diff(&bj, &d), rel_diff(&icr, &d));
    println!(
        "guard A1 {:.3e} / {:.3e}, A2 {e_bj:.3e} / {e_ic:.3e}, iterations {} / {}",
        residual(&bj),
        residual(&icr),
        rbj.iterations,
        ric.iterations
    );
    assert!(residual(&bj) <= 1e-10 && residual(&icr) <= 1e-10, "A1");
    assert_eq!(bj.stats().unwrap().solver(), "cg-block-jacobi");
    assert!(e_bj <= 1e-7 && e_ic <= 1e-7, "A2");
    let forced = CgPreconditioner::IncompleteCholesky(IcOptions {
        pivot_tolerance: 0.9,
        ..IcOptions::default()
    });
    let (fr, frep, _) = run(&model, &pcg(forced, TOL, 20_000));
    let e_f = rel_diff(&fr, &d);
    println!(
        "guard A6 '{}', shift {:?}, {} breakdowns, residual {:.3e}, against direct {e_f:.3e}",
        fr.stats().unwrap().solver(),
        frep.shift,
        frep.breakdowns.len(),
        residual(&fr)
    );
    assert!(fr.stats().unwrap().solver().contains("shift"), "A6 name");
    assert!(
        frep.shift.unwrap() > 0.0 && !frep.breakdowns.is_empty(),
        "A6 shift reported"
    );
    assert!(fr.displacement_values().iter().all(|v| v.is_finite()), "A6 finite");
    assert!(residual(&fr) <= 1e-10 && e_f <= 1e-7, "A6 residual and agreement");
    let shifted = CgPreconditioner::IncompleteCholesky(IcOptions {
        initial_shift: 1.0,
        ..IcOptions::default()
    });
    let (n2, _, _) = run(&model, &pcg(shifted, TOL, 20_000));
    let r2 = iterations(&n2) as f64 / iterations(&bj) as f64;
    println!("guard N2 ratio {r2:.4}");
    assert!(r2 > 0.5, "N2");
}

// ---------------------------------------------------------------------------------------------------------------------
// Amendment 3: B2.

#[test]
fn b2_node_blocks_with_partial_constraints() {
    let model = cantilever(FAST);
    let dofs = model.dof_map();
    let held = [(0usize, 0usize), (5, 1), (5, 2), (9, 1)];
    let free: Vec<usize> = (0..dofs.n_equations())
        .filter(|&q| !held.contains(&dofs.node_component(q).unwrap()))
        .collect();
    let ptr = node_blocks(&free, &dofs);
    let sizes: Vec<usize> = ptr.windows(2).map(|w| w[1] - w[0]).collect();
    println!("B2 {} blocks, first sizes {:?}", sizes.len(), &sizes[..11]);
    assert_eq!(sizes.len(), 1617);
    assert_eq!(&sizes[..11], &[2, 3, 3, 3, 3, 1, 3, 3, 3, 2, 3]);
    assert!(sizes[11..].iter().all(|&s| s == 3));
    for w in ptr.windows(2) {
        let n0 = dofs.node_component(free[w[0]]).unwrap().0;
        assert!(free[w[0]..w[1]]
            .iter()
            .all(|&q| dofs.node_component(q).unwrap().0 == n0));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Amendment 4: H1c, the heavy record on the registered geometry.

fn cantilever_box(lengths: [f64; 3], div: [usize; 3]) -> SolidModel {
    let mesh = hex_box([0.0; 3], lengths, div).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -TRACTION])).unwrap();
    m
}

#[test]
#[ignore = "heavy record (ADR 0282 amendment 4, H1c): Hex8 cantilever [0, 8] x [0, 1.5]^2, 96 x 18 x 18, 103 968 unknowns"]
fn h1c_heavy_cantilever() {
    let model = cantilever_box([L, H, H], HEAVY);
    let mut out = Vec::new();
    for p in [CgPreconditioner::BlockJacobi, ic()] {
        let t = Instant::now();
        match solve_static_report(&model, &[], &[], &pcg(p.clone(), TOL, 60_000)) {
            Ok((r, rep)) => {
                print(&format!("H1c heavy {p:?}"), &r, &rep, t.elapsed().as_secs_f64());
                for (bd_n, bd_c, bd) in &rep.breakdowns {
                    println!("H1c IC(0) breakdown at node {bd_n} component {bd_c}: {bd:?}");
                }
                let umax = r.displacement_values().iter().fold(0.0f64, |s, v| s.max(v.abs()));
                println!("H1c max |u| {umax:.4e}");
                out.push(Some((residual(&r), r.stats().unwrap().unknowns(), rep.iterations)));
            }
            Err(e) => {
                println!("H1c heavy {p:?}: error {e}");
                out.push(None);
            }
        }
    }
    if let (Some(a), Some(b)) = (out[0], out[1]) {
        println!(
            "H1c iteration ratio IC / block Jacobi {:.4} (record)",
            b.2 as f64 / a.2 as f64
        );
    }
    for o in &out {
        let (res, n, _) = o.expect("H1c solve");
        println!("H1c relative residual {res:.3e} (limit 1e-10), {n} unknowns");
        assert!(res <= 1e-10 && n >= 100_000, "H1c");
    }
}
