//! ADR 0357 (F-460): incomplete Cholesky with one level of fill and IC(0) after a node-wise reverse Cuthill-McKee ordering
//! on the cantilevers of ADR 0282. Checks A0 to A4, C3, F1, F2, S1, P1, R1, the negative controls N1 to N3 and the record
//! X1; H1 is the heavy record (`#[ignore]`). The set-up helpers are copied from `sb_pcg.rs` (ADR 0282).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::solve::{
    node_blocks, solve_static, solve_static_report, CgPreconditioner, IcOrdering, SolveReport, StaticOptions,
    StaticSolver,
};
use rcfd_fem::{Constraint, DofMap, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::precond::{ic_fill_pattern, IcOptions};
use rcfd_linalg::reorder::block_rcm_order;
use rcfd_linalg::CsrMatrix;
use std::collections::BTreeSet;
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

/// Fast: `[0, 8] x [0, 1.5]^2`, 32 x 6 x 6 (L/h 5.3). Slender: 16 m, 64 x 6 x 6 (L/h 10.7). Heavy: `[0, 8] x [0, 1.5]^2`,
/// 96 x 18 x 18.
fn fast() -> SolidModel {
    cantilever_box([L, H, H], FAST)
}
fn slender() -> SolidModel {
    cantilever_box([2.0 * L, H, H], SLENDER)
}
fn heavy() -> SolidModel {
    cantilever_box([L, H, H], HEAVY)
}

fn pcg(p: CgPreconditioner, tolerance: f64, max_iterations: usize) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: p,
        tolerance,
        max_iterations,
    })
}

fn ic0() -> CgPreconditioner {
    CgPreconditioner::IncompleteCholesky(IcOptions::default())
}

fn fill(level: usize, ordering: IcOrdering, options: IcOptions) -> CgPreconditioner {
    CgPreconditioner::IncompleteCholeskyFill {
        options,
        level,
        ordering,
    }
}

fn ic1() -> CgPreconditioner {
    fill(1, IcOrdering::Natural, IcOptions::default())
}
fn rcm0() -> CgPreconditioner {
    fill(0, IcOrdering::NodeReverseCuthillMcKee, IcOptions::default())
}
fn rcm1() -> CgPreconditioner {
    fill(1, IcOrdering::NodeReverseCuthillMcKee, IcOptions::default())
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

fn bit_equal(a: &SolidResult, b: &SolidResult) -> bool {
    a.displacement_values()
        .iter()
        .zip(b.displacement_values())
        .all(|(p, q)| p.to_bits() == q.to_bits())
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

/// The full stiffness, the reduced stiffness, the free equations and the dof map, as in `solve_static_report`.
fn reduced(model: &SolidModel) -> (CsrMatrix, CsrMatrix, Vec<usize>, DofMap) {
    let dofs = model.dof_map();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let k = stiffness_with(model, None).unwrap();
    let f = vec![0.0; dofs.n_equations()];
    let sys = reduce(&k, &f, &fixed).unwrap();
    let free = sys.free().to_vec();
    (k, sys.matrix, free, dofs)
}

fn threads_bit_identical(model: &SolidModel, p: &CgPreconditioner) -> (usize, usize, bool) {
    let mut runs = Vec::new();
    for t in [1usize, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
        let o = pcg(p.clone(), TOL, 20_000).threads(t);
        runs.push(pool.install(|| solve_static(model, &o).unwrap()));
    }
    (iterations(&runs[0]), iterations(&runs[1]), bit_equal(&runs[0], &runs[1]))
}

// ---------------------------------------------------------------------------------------------------------------------
// A0 to A4, C3, N1, X1 (fast case).

#[test]
fn a_fast_cantilever_fill_and_ordering() {
    let model = fast();
    let (d, _, td) = run(&model, &StaticOptions::default());
    println!(
        "X1 fast direct: {} unknowns, relative residual {:.3e}, {:.2} s",
        d.stats().unwrap().unknowns(),
        residual(&d),
        td
    );
    let (bj, rbj, t) = run(&model, &pcg(CgPreconditioner::BlockJacobi, TOL, 20_000));
    print("X1 fast BJ", &bj, &rbj, t);
    let (i0, ri0, t) = run(&model, &pcg(ic0(), TOL, 20_000));
    print("X1 fast IC0", &i0, &ri0, t);
    let it_bj = iterations(&bj);
    println!(
        "A0 BJ {} (expected 156), IC0 {} (expected 85)",
        it_bj,
        iterations(&i0)
    );

    let mut meets = Vec::new();
    let mut sols = Vec::new();
    for (tag, p) in [("IC1", ic1()), ("RCM0", rcm0()), ("RCM1", rcm1())] {
        let (r, rep, t) = run(&model, &pcg(p.clone(), TOL, 20_000));
        print(&format!("X1 fast {tag}"), &r, &rep, t);
        let res = residual(&r);
        let e = rel_diff(&r, &d);
        let ratio = iterations(&r) as f64 / it_bj as f64;
        println!(
            "X1 fast {tag}: against direct {e:.3e}, ratio to BJ {} / {it_bj} = {ratio:.4}",
            iterations(&r)
        );
        if tag == "RCM1" {
            continue; // record only
        }
        let (n1, n8, same) = threads_bit_identical(&model, &p);
        println!("A4 {tag}: iterations {n1} / {n8} for 1 / 8 threads, displacement bit-equal {same}");
        let a1 = res <= 1e-10;
        let a2 = e <= 1e-7;
        let a3 = ratio <= 0.5;
        let a4 = n1 == n8 && same && n1 == iterations(&r);
        println!(
            "{tag}: A1 {res:.3e} <= 1e-10 {a1}; A2 {e:.3e} <= 1e-7 {a2}; A3 {ratio:.4} <= 0.5 {a3}; A4 {a4}"
        );
        meets.push((tag, a1 && a2 && a3 && a4));
        sols.push((tag, r));
    }
    let n1: Vec<(&str, bool)> = sols.iter().map(|(tag, r)| (*tag, bit_equal(r, &i0))).collect();
    println!("N1 bit-equal to IC0: {n1:?} (expected false, both)");
    println!("C3 per variant: {meets:?}");

    assert_eq!(it_bj, 156, "A0 BJ");
    assert_eq!(iterations(&i0), 85, "A0 IC0");
    assert!(n1.iter().all(|(_, eq)| !eq), "N1");
    assert!(meets.iter().any(|(_, ok)| *ok), "C3");
}

// ---------------------------------------------------------------------------------------------------------------------
// F1, F2: forced breakdowns with the shift strategy of ADR 0282.

#[test]
fn f1_f2_forced_breakdowns() {
    let model = fast();
    let (d, _, _) = run(&model, &StaticOptions::default());
    let (k, _, _, dofs) = reduced(&model);
    let kd = k.diag();
    let forced = IcOptions {
        pivot_tolerance: 0.9,
        ..IcOptions::default()
    };
    for (tag, p) in [
        ("IC1", fill(1, IcOrdering::Natural, forced)),
        ("RCM0", fill(0, IcOrdering::NodeReverseCuthillMcKee, forced)),
    ] {
        let (r, rep, t) = run(&model, &pcg(p, TOL, 20_000));
        print(&format!("F1 {tag} forced"), &r, &rep, t);
        let e = rel_diff(&r, &d);
        println!(
            "F1 {tag}: name '{}', first {:?}, last {:?}, residual {:.3e}, against direct {e:.3e}",
            r.stats().unwrap().solver(),
            rep.breakdowns.first(),
            rep.breakdowns.last(),
            residual(&r)
        );
        let mut f2 = true;
        for (n, c, bd) in &rep.breakdowns {
            let q = dofs.equation(*n, *c).unwrap();
            let want = (1.0 + bd.shift) * kd[q];
            if want.to_bits() != bd.diagonal.to_bits() {
                println!("F2 {tag}: node {n} component {c}: diagonal {:e} against {want:e}", bd.diagonal);
                f2 = false;
            }
        }
        println!("F2 {tag}: every breakdown's diagonal is (1 + shift) K_qq of its equation: {f2}");
        assert!(r.stats().unwrap().solver().contains("shift"), "F1 name");
        assert!(rep.shift.unwrap() > 0.0 && !rep.breakdowns.is_empty(), "F1 shift");
        assert!(r.displacement_values().iter().all(|v| v.is_finite()), "F1 finite");
        assert!(residual(&r) <= 1e-10 && e <= 1e-7, "F1 residual and agreement");
        assert!(f2, "F2");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// S1: level 0, natural order through the new variant is the existing IC(0).

#[test]
fn s1_level_zero_natural_is_ic0() {
    let model = fast();
    let (a, ra, _) = run(&model, &pcg(ic0(), TOL, 20_000));
    let (b, rb, _) = run(&model, &pcg(fill(0, IcOrdering::Natural, IcOptions::default()), TOL, 20_000));
    let same = iterations(&a) == iterations(&b) && bit_equal(&a, &b);
    println!(
        "S1 IC0 '{}' {} iterations, level 0 natural '{}' {} iterations, bit-equal {same}",
        ra.solver, ra.iterations, rb.solver, rb.iterations
    );
    assert!(same, "S1");
}

// ---------------------------------------------------------------------------------------------------------------------
// P1, N2: the level-1 pattern of the fast reduced stiffness against the closed form.

#[test]
fn p1_fill_pattern_of_the_cantilever() {
    let (_, a, _, _) = reduced(&fast());
    let n = a.n;
    let mut stored: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for p in a.row_ptr[i]..a.row_ptr[i + 1] {
            let k = a.col[p];
            if k < i {
                stored.insert((i, k));
                below[k].push(i);
            }
        }
    }
    let mut want1 = stored.clone();
    for rows in &below {
        for (x, &i) in rows.iter().enumerate() {
            for &j in &rows[..x] {
                want1.insert((i.max(j), i.min(j)));
            }
        }
    }
    let as_set = |(rp, c): (Vec<usize>, Vec<usize>)| -> BTreeSet<(usize, usize)> {
        (0..n)
            .flat_map(|i| c[rp[i]..rp[i + 1]].iter().map(move |&k| (i, k)).collect::<Vec<_>>())
            .collect()
    };
    let (rp1, c1) = ic_fill_pattern(&a, 1);
    let count1 = c1.len();
    let got1 = as_set((rp1, c1));
    let got0 = as_set(ic_fill_pattern(&a, 0));
    println!(
        "P1 level 1: {} entries (closed form {}, expected 317 751), equal {}; level 0: {} entries (stored {}, expected 150 351), equal {}",
        count1,
        want1.len(),
        got1 == want1,
        got0.len(),
        stored.len(),
        got0 == stored
    );
    println!("N2 level 0 and level 1 patterns equal: {} (expected false)", got0 == got1);
    assert_eq!(count1, got1.len(), "P1 no duplicates");
    assert!(got1 == want1 && got0 == stored, "P1");
    assert!(got0 != got1, "N2");
}

// ---------------------------------------------------------------------------------------------------------------------
// R1, N3: the node-wise RCM order.

fn node_bandwidth(a: &CsrMatrix, block_of: &[usize], pos: &[usize]) -> usize {
    let mut bw = 0;
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            bw = bw.max(pos[block_of[r]].abs_diff(pos[block_of[a.col[p]]]));
        }
    }
    bw
}

#[test]
fn r1_node_rcm_order() {
    let (_, a, free, dofs) = reduced(&fast());
    let ptr = node_blocks(&free, &dofs);
    let nb = ptr.len() - 1;
    let mut block_of = vec![0; a.n];
    for (b, w) in ptr.windows(2).enumerate() {
        block_of[w[0]..w[1]].fill(b);
    }
    let order = block_rcm_order(&a, &ptr);
    let mut seen = vec![false; a.n];
    let perm = order.len() == a.n && order.iter().all(|&o| o < a.n && !std::mem::replace(&mut seen[o], true));
    // node sequence and contiguity
    let mut node_seq = Vec::new();
    let mut contiguous = true;
    let mut i = 0;
    while i < order.len() {
        let b = block_of[order[i]];
        let len = ptr[b + 1] - ptr[b];
        contiguous &= (0..len).all(|t| i + t < order.len() && order[i + t] == ptr[b] + t);
        node_seq.push(b);
        i += len;
    }
    let mut pos = vec![0; nb];
    for (p, &b) in node_seq.iter().enumerate() {
        if b < nb {
            pos[b] = p;
        }
    }
    let natural: Vec<usize> = (0..nb).collect();
    let bw_nat = node_bandwidth(&a, &block_of, &natural);
    let bw_rcm = node_bandwidth(&a, &block_of, &pos);
    println!(
        "R1 permutation {perm}, nodes contiguous and ascending {contiguous} ({} nodes), node bandwidth RCM {bw_rcm} \
         (estimate <= 217), natural {bw_nat}; first nodes {:?}",
        node_seq.len(),
        &node_seq[..8.min(node_seq.len())]
    );
    println!("N3 natural node bandwidth {bw_nat} (expected 257)");
    assert!(perm && contiguous && node_seq.len() == nb, "R1 order");
    assert!(bw_rcm < bw_nat, "R1 bandwidth");
    assert_eq!(bw_nat, 257, "N3");
}

// ---------------------------------------------------------------------------------------------------------------------
// X1: slender record (L/h = 10.7).

#[test]
fn x1_slender_record() {
    let model = slender();
    let (d, _, td) = run(&model, &StaticOptions::default());
    println!(
        "X1 slender direct: {} unknowns, relative residual {:.3e}, {:.2} s",
        d.stats().unwrap().unknowns(),
        residual(&d),
        td
    );
    for (tag, m) in [("fast", fast()), ("slender", slender())] {
        let (_, a, free, dofs) = reduced(&m);
        let order = block_rcm_order(&a, &node_blocks(&free, &dofs));
        let b = a.permute_symmetric(&order);
        println!(
            "X1 {tag} factor entries (strict lower): natural level 0 {}, level 1 {}; RCM level 0 {}, level 1 {}",
            ic_fill_pattern(&a, 0).1.len(),
            ic_fill_pattern(&a, 1).1.len(),
            ic_fill_pattern(&b, 0).1.len(),
            ic_fill_pattern(&b, 1).1.len()
        );
    }
    let mut it_bj = None;
    for (tag, p) in [
        ("BJ", CgPreconditioner::BlockJacobi),
        ("IC0", ic0()),
        ("IC1", ic1()),
        ("RCM0", rcm0()),
        ("RCM1", rcm1()),
    ] {
        let t = Instant::now();
        match solve_static_report(&model, &[], &[], &pcg(p, TOL, 40_000)) {
            Ok((r, rep)) => {
                print(&format!("X1 slender {tag}"), &r, &rep, t.elapsed().as_secs_f64());
                let it = iterations(&r);
                let base = *it_bj.get_or_insert(it);
                println!(
                    "X1 slender {tag}: against direct {:.3e}, ratio to BJ {:.4}",
                    rel_diff(&r, &d),
                    it as f64 / base as f64
                );
            }
            Err(e) => println!("X1 slender {tag}: error {e}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// H1: heavy record (103 968 unknowns).

#[test]
#[ignore = "heavy record (ADR 0357, H1): Hex8 cantilever [0, 8] x [0, 1.5]^2, 96 x 18 x 18, 103 968 unknowns, five preconditioners"]
fn h1_heavy_fill_record() {
    let model = heavy();
    let mut it_bj = None;
    for (tag, p) in [
        ("BJ", CgPreconditioner::BlockJacobi),
        ("IC0", ic0()),
        ("IC1", ic1()),
        ("RCM0", rcm0()),
        ("RCM1", rcm1()),
    ] {
        let t = Instant::now();
        match solve_static_report(&model, &[], &[], &pcg(p, TOL, 60_000)) {
            Ok((r, rep)) => {
                print(&format!("H1 heavy {tag}"), &r, &rep, t.elapsed().as_secs_f64());
                let it = iterations(&r);
                let base = *it_bj.get_or_insert(it);
                println!("H1 heavy {tag}: ratio to BJ {:.4}", it as f64 / base as f64);
            }
            Err(e) => println!("H1 heavy {tag}: error {e}"),
        }
    }
}
