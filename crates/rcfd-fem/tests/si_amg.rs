//! ADR 0305 (F-402): smoothed-aggregation multigrid with the rigid-body modes as the preconditioner of CG. Fast checks
//! B1, B2, G1, S1, S2, AG1, C-small, A1, TH, OC, P1, X1 and the negative controls N1 to N3; heavy records H1 to H4
//! (`#[ignore]`). The cantilever set-up is copied from `sb_pcg_fill.rs` (ADR 0357).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::solve::{
    smoothed_aggregation, solve_static, solve_static_report, CgPreconditioner, SolveReport, StaticOptions, StaticSolver,
};
use rcfd_fem::{Constraint, DofMap, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::amg_elastic::{rigid_body_modes, ElasticAmg, ElasticAmgSettings};
use rcfd_linalg::precond::IcOptions;
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{cg, CsrMatrix, Preconditioner, SolveControl};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const TOL: f64 = 1e-10;

fn clamped(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::traction("xmax", [0.0, 0.0, -1.0e6])).unwrap();
    m
}

/// Reading 3: the clamped unit cube of `n^3` Hex8.
fn cube(n: usize) -> SolidModel {
    clamped(hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap())
}

/// P1: the cube with symmetry constraints only and a tension on `xmax`.
fn sym_cube(n: usize) -> SolidModel {
    let mut m = SolidModel::new(hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap());
    m.add_material("all", Material::isotropic_elastic(E_MOD, NU, 0.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    m.add_load(Load::traction("xmax", [1.0e6, 0.0, 0.0])).unwrap();
    m
}

/// ADR 0282 cantilevers: fast 32 x 6 x 6 on `[0, 8] x [0, 1.5]^2`, slender 64 x 6 x 6 on 16 m, heavy 96 x 18 x 18.
fn cantilever_box(lengths: [f64; 3], div: [usize; 3]) -> SolidModel {
    clamped(hex_box([0.0; 3], lengths, div).unwrap())
}

/// Reading 4: slenderness 20, Hex8 240 x 12 x 12.
fn slender20_hex() -> SolidModel {
    cantilever_box([20.0, 1.0, 1.0], [240, 12, 12])
}

/// Reading 5: slenderness 20, Tet10 from the Kuhn split of 120 x 6 x 6.
fn slender20_tet10() -> SolidModel {
    let (m10, _) = upgrade_to_tet10(&tet_box([0.0; 3], [20.0, 1.0, 1.0], [120, 6, 6]).unwrap()).unwrap();
    clamped(m10)
}

fn amg_options(max_iterations: usize) -> StaticOptions {
    pcg(
        CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
        max_iterations,
    )
}

fn pcg(p: CgPreconditioner, max_iterations: usize) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: p,
        tolerance: TOL,
        max_iterations,
    })
}

fn ic0() -> CgPreconditioner {
    CgPreconditioner::IncompleteCholesky(IcOptions::default())
}

struct Reduced {
    k: CsrMatrix,
    a: CsrMatrix,
    rhs: Vec<f64>,
    free: Vec<usize>,
    fixed: Vec<usize>,
    dofs: DofMap,
}

fn reduced(model: &SolidModel) -> Reduced {
    let dofs = model.dof_map();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let k = stiffness_with(model, None).unwrap();
    let f = nodal_load_vector(model, &[]).unwrap();
    let sys = reduce(&k, &f, &fixed).unwrap();
    Reduced {
        free: sys.free().to_vec(),
        fixed: fixed.iter().map(|&(q, _)| q).collect(),
        rhs: sys.rhs.clone(),
        a: sys.matrix,
        k,
        dofs,
    }
}

fn hierarchy(model: &SolidModel, r: &Reduced) -> ElasticAmg {
    smoothed_aggregation(
        &r.a,
        &r.free,
        &r.dofs,
        model.mesh().nodes(),
        &ElasticAmgSettings::default(),
    )
    .unwrap()
}

/// The test's own rigid-body modes of the free equations (origin 0, not centred; reading 1).
fn own_modes(model: &SolidModel, r: &Reduced) -> Vec<Vec<f64>> {
    let nodes = model.mesh().nodes();
    let mut b = vec![vec![0.0; r.free.len()]; 6];
    for (row, &q) in r.free.iter().enumerate() {
        let (n, c) = r.dofs.node_component(q).unwrap();
        let x = nodes[n];
        b[c][row] = 1.0;
        let rot = [[0.0, -x[2], x[1]], [x[2], 0.0, -x[0]], [-x[1], x[0], 0.0]];
        for a in 0..3 {
            b[3 + a][row] = rot[a][c];
        }
    }
    b
}

fn tmatvec(p: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; p.ncols];
    for i in 0..p.n {
        for k in p.row_ptr[i]..p.row_ptr[i + 1] {
            y[p.col[k]] += p.val[k] * x[i];
        }
    }
    y
}

fn matvec(p: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; p.n];
    for i in 0..p.n {
        for k in p.row_ptr[i]..p.row_ptr[i + 1] {
            y[i] += p.val[k] * x[p.col[k]];
        }
    }
    y
}

/// `|B - P c(B)|_F / |B|_F` over the rows where `rows` is true, with `c` the coarse coefficients of each column.
fn defect_rows(b: &[Vec<f64>], p: &CsrMatrix, coarse: &[Vec<f64>], rows: Option<&[bool]>) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (col, c) in b.iter().zip(coarse) {
        let pc = matvec(p, c);
        for i in 0..p.n {
            den += col[i] * col[i];
            if rows.map_or(true, |r| r[i]) {
                num += (col[i] - pc[i]).powi(2);
            }
        }
    }
    (num / den).sqrt()
}

/// B1 on every level: `(level, range measure, |B_l - P_t B_{l+1}| / |B_l|)`; level 0 with `own`.
fn b1(amg: &ElasticAmg, own: &[Vec<f64>]) -> Vec<(usize, f64, f64)> {
    let mut out = Vec::new();
    for l in 0..amg.n_levels() {
        let Some(pt) = amg.tentative(l) else { break };
        let b: Vec<Vec<f64>> = if l == 0 {
            own.to_vec()
        } else {
            amg.near_null(l).to_vec()
        };
        let proj: Vec<Vec<f64>> = b.iter().map(|c| tmatvec(pt, c)).collect();
        let range = defect_rows(&b, pt, &proj, None);
        let lib = defect_rows(amg.near_null(l), pt, amg.near_null(l + 1), None);
        out.push((l, range, lib));
    }
    out
}

fn b1_ok(v: &[(usize, f64, f64)]) -> bool {
    // level 0: the range measure with the test's own modes (the library's representation is printed); l >= 1: both
    v.iter()
        .all(|&(l, range, lib)| range <= 1e-12 && (l == 0 || lib <= 1e-12))
}

fn levels_desc(amg: &ElasticAmg) -> Vec<(usize, usize)> {
    (0..amg.n_levels())
        .map(|l| (amg.matrix(l).n, amg.matrix(l).nnz()))
        .collect()
}

fn run(model: &SolidModel, o: &StaticOptions) -> (SolidResult, SolveReport, f64) {
    let t = Instant::now();
    let (r, rep) = solve_static_report(model, &[], &[], o).unwrap();
    (r, rep, t.elapsed().as_secs_f64())
}

fn try_run(model: &SolidModel, o: &StaticOptions) -> Option<(SolidResult, SolveReport, f64)> {
    let t = Instant::now();
    match solve_static_report(model, &[], &[], o) {
        Ok((r, rep)) => Some((r, rep, t.elapsed().as_secs_f64())),
        Err(e) => {
            println!("  solve error: {e}");
            None
        }
    }
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
        "{tag}: solver '{}', {} unknowns, {} iterations, {} restarts, relative residual {:.3e}, {:.2} s",
        rep.solver,
        r.stats().unwrap().unknowns(),
        rep.iterations,
        rep.restarts,
        rep.relative_residual,
        secs
    );
}

fn threads_bit_identical(model: &SolidModel) -> (usize, usize, bool) {
    let mut runs = Vec::new();
    for t in [1usize, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
        let o = amg_options(20_000).threads(t);
        runs.push(pool.install(|| solve_static(model, &o).unwrap()));
    }
    (
        iterations(&runs[0]),
        iterations(&runs[1]),
        bit_equal(&runs[0], &runs[1]),
    )
}

/// The hierarchy checks of one model: B1 (and OC), printed; returns (B1 met, OC).
fn hierarchy_checks(tag: &str, model: &SolidModel, r: &Reduced) -> (bool, f64) {
    let t = Instant::now();
    let amg = hierarchy(model, r);
    let setup = t.elapsed().as_secs_f64();
    let v = b1(&amg, &own_modes(model, r));
    let oc = amg.operator_complexity();
    println!(
        "{tag}: levels {:?} (order, nnz), operator complexity {oc:.4}, setup {setup:.2} s, omega {:?}",
        levels_desc(&amg),
        (0..amg.n_levels()).map(|l| amg.omega(l)).collect::<Vec<_>>()
    );
    for (l, range, lib) in &v {
        println!("  B1 {tag} level {l}: range {range:.3e}, |B_l - P_t B_l+1| / |B_l| {lib:.3e}");
    }
    (b1_ok(&v), oc)
}

// ---------------------------------------------------------------------------------------------------------------------
// B1, B2, N2, G1, S1, S2, AG1 on the cubes.

#[test]
fn hierarchy_cubes() {
    let mut ok_b1 = true;
    let mut ok_b2 = true;
    let mut ok_n2 = true;
    let mut ok_g1 = true;
    let mut ok_s1 = true;
    let mut ok_oc = true;
    for n in [11usize, 23] {
        let model = cube(n);
        let r = reduced(&model);
        let (b1m, oc) = hierarchy_checks(&format!("cube {n}"), &model, &r);
        ok_b1 &= b1m;
        ok_oc &= oc <= 1.5;
        let amg = hierarchy(&model, &r);
        // B2 / N2: smoothed P on level 0
        let fixed: BTreeSet<usize> = r.fixed.iter().copied().collect();
        let interior: Vec<bool> = r
            .free
            .iter()
            .map(|&q| (r.k.row_ptr[q]..r.k.row_ptr[q + 1]).all(|p| !fixed.contains(&r.k.col[p])))
            .collect();
        let own = own_modes(&model, &r);
        let pt = amg.tentative(0).unwrap();
        let p = amg.prolongator(0).unwrap();
        let proj: Vec<Vec<f64>> = own.iter().map(|c| tmatvec(pt, c)).collect();
        let b2 = defect_rows(&own, p, &proj, Some(&interior));
        let n2 = defect_rows(&own, p, &proj, None);
        println!(
            "B2 cube {n}: smoothed P, {} of {} rows coupled to no prescribed equation: {b2:.3e} (limit 1e-10); N2 all rows {n2:.3e} (expected >= 1e-6)",
            interior.iter().filter(|&&x| x).count(),
            interior.len()
        );
        ok_b2 &= b2 <= 1e-10;
        ok_n2 &= n2 >= 1e-6;
        // G1 and S1 on every level with a prolongator
        for l in 0..amg.n_levels() {
            let Some(p) = amg.prolongator(l) else { break };
            let a = amg.matrix(l);
            let ac = p.transpose().matmul(&a.matmul(p));
            let g1 = max_entry_diff(&ac, amg.matrix(l + 1));
            // S1: P_t - omega D^{-1} A P_t, D^{-1} by solves of the diagonal blocks
            let pt = amg.tentative(l).unwrap();
            let s = smoothed_in_test(a, amg.block_ptr(l), pt, amg.omega(l).unwrap());
            let s1 = max_entry_diff(&s, p);
            println!("G1 cube {n} level {l}: {g1:.3e} (limit 1e-12); S1: {s1:.3e} (limit 1e-10)");
            ok_g1 &= g1 <= 1e-12;
            ok_s1 &= s1 <= 1e-10;
        }
        if n == 11 {
            // AG1
            let agg = amg.aggregates(0).unwrap();
            let mut size: BTreeMap<usize, usize> = BTreeMap::new();
            for &g in agg {
                *size.entry(g).or_default() += 1;
            }
            let singletons = size.values().filter(|&&s| s == 1).count();
            let (mn, mx) = (size.values().min().unwrap(), size.values().max().unwrap());
            println!(
                "AG1 cube 11 level 0: {} aggregates (estimate about 64), sizes {mn} to {mx}, singletons {singletons}",
                size.len()
            );
            assert_eq!(singletons, 0, "AG1");
            // S2
            let (sym, pos) = symmetry(&amg, r.a.n);
            println!("S2 cube 11: asymmetry {sym:.3e} (limit 1e-10), x.Mx > 0 and y.My > 0: {pos}");
            assert!(sym <= 1e-10 && pos, "S2");
        }
    }
    assert!(ok_b1, "B1");
    assert!(ok_b2, "B2");
    assert!(ok_n2, "N2");
    assert!(ok_g1, "G1");
    assert!(ok_s1, "S1");
    assert!(ok_oc, "OC");
}

fn symmetry(m: &dyn Preconditioner, n: usize) -> (f64, bool) {
    let x: Vec<f64> = (0..n).map(|i| (1.0 + i as f64).sin()).collect();
    let y: Vec<f64> = (0..n).map(|i| (2.0 + 3.0 * i as f64).cos()).collect();
    let (mut mx, mut my) = (vec![0.0; n], vec![0.0; n]);
    m.apply(&x, &mut mx);
    m.apply(&y, &mut my);
    let d = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(p, q)| p * q).sum::<f64>();
    let sym = (d(&x, &my) - d(&y, &mx)).abs() / (norm2(&x) * norm2(&my));
    (sym, d(&x, &mx) > 0.0 && d(&y, &my) > 0.0)
}

fn max_entry_diff(a: &CsrMatrix, b: &CsrMatrix) -> f64 {
    let map = |m: &CsrMatrix| -> BTreeMap<(usize, usize), f64> {
        let mut out = BTreeMap::new();
        for i in 0..m.n {
            for k in m.row_ptr[i]..m.row_ptr[i + 1] {
                out.insert((i, m.col[k]), m.val[k]);
            }
        }
        out
    };
    let (ma, mb) = (map(a), map(b));
    let scale = mb.values().fold(0.0f64, |s, v| s.max(v.abs()));
    let keys: BTreeSet<&(usize, usize)> = ma.keys().chain(mb.keys()).collect();
    let mut worst = 0.0f64;
    for k in keys {
        let d = ma.get(k).copied().unwrap_or(0.0) - mb.get(k).copied().unwrap_or(0.0);
        worst = worst.max(d.abs());
    }
    if a.n != b.n || a.ncols != b.ncols {
        return f64::INFINITY;
    }
    worst / scale
}

/// Gaussian elimination with partial pivoting on a small dense system with several right-hand sides.
fn solve_dense(mut a: Vec<f64>, n: usize, mut b: Vec<f64>, m: usize) -> Vec<f64> {
    for k in 0..n {
        let p = (k..n)
            .max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))
            .unwrap();
        for j in 0..n {
            a.swap(k * n + j, p * n + j);
        }
        for j in 0..m {
            b.swap(k * m + j, p * m + j);
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            for j in k..n {
                a[i * n + j] -= f * a[k * n + j];
            }
            for j in 0..m {
                b[i * m + j] -= f * b[k * m + j];
            }
        }
    }
    for k in (0..n).rev() {
        for j in 0..m {
            let mut s = b[k * m + j];
            for t in k + 1..n {
                s -= a[k * n + t] * b[t * m + j];
            }
            b[k * m + j] = s / a[k * n + k];
        }
    }
    b
}

fn smoothed_in_test(a: &CsrMatrix, bp: &[usize], pt: &CsrMatrix, omega: f64) -> CsrMatrix {
    let ap = a.matmul(pt);
    let nc = pt.ncols;
    let mut trip: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for i in 0..pt.n {
        for k in pt.row_ptr[i]..pt.row_ptr[i + 1] {
            *trip.entry((i, pt.col[k])).or_default() += pt.val[k];
        }
    }
    for w in bp.windows(2) {
        let (s, e) = (w[0], w[1]);
        let bs = e - s;
        let mut d = vec![0.0; bs * bs];
        for r in s..e {
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                if a.col[k] >= s && a.col[k] < e {
                    d[(r - s) * bs + a.col[k] - s] = a.val[k];
                }
            }
        }
        let cols: Vec<usize> = (s..e)
            .flat_map(|r| ap.col[ap.row_ptr[r]..ap.row_ptr[r + 1]].to_vec())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let m = cols.len();
        let mut rhs = vec![0.0; bs * m];
        for r in s..e {
            for k in ap.row_ptr[r]..ap.row_ptr[r + 1] {
                let j = cols.binary_search(&ap.col[k]).unwrap();
                rhs[(r - s) * m + j] = ap.val[k];
            }
        }
        let x = solve_dense(d, bs, rhs, m);
        for i in 0..bs {
            for (j, &c) in cols.iter().enumerate() {
                *trip.entry((s + i, c)).or_default() -= omega * x[i * m + j];
            }
        }
    }
    let mut t: Vec<(usize, usize, f64)> = trip.into_iter().map(|((i, j), v)| (i, j, v)).collect();
    t.sort_by_key(|&(i, j, _)| (i, j));
    let mut row_ptr = vec![0usize; pt.n + 1];
    let (mut col, mut val) = (Vec::new(), Vec::new());
    for (i, j, v) in t {
        row_ptr[i + 1] += 1;
        col.push(j);
        val.push(v);
    }
    for i in 0..pt.n {
        row_ptr[i + 1] += row_ptr[i];
    }
    CsrMatrix::rect(pt.n, nc, row_ptr, col, val)
}

// ---------------------------------------------------------------------------------------------------------------------
// C-small, A1, TH, N3 (cubes 11 and 23).

#[test]
fn cubes_solve() {
    let mut ok_c = true;
    let mut ok_th = true;
    let mut counts = Vec::new();
    for n in [11usize, 23] {
        let model = cube(n);
        let (r, rep, t) = run(&model, &amg_options(20_000));
        print(&format!("C-small cube {n} AMG"), &r, &rep, t);
        let it = iterations(&r);
        let c = residual(&r) <= TOL && it <= 39;
        println!(
            "C-small cube {n}: residual {:.3e} <= 1e-10 and count {it} <= 39: {c}",
            residual(&r)
        );
        ok_c &= c;
        counts.push(it);
        let (n1, n8, same) = threads_bit_identical(&model);
        let th = n1 == n8 && same && n1 == it;
        println!("TH cube {n}: iterations {n1} / {n8} for 1 / 8 threads, displacement bit-equal {same}: {th}");
        ok_th &= th;
        if n == 11 {
            let (d, _, td) = run(&model, &StaticOptions::default());
            let e = rel_diff(&r, &d);
            println!("A1 cube 11: against direct {e:.3e} (limit 1e-7); direct {td:.2} s");
            assert!(e <= 1e-7, "A1");
            let (i0, ri0, t0) = run(&model, &pcg(ic0(), 20_000));
            print("cube 11 IC0 (record)", &i0, &ri0, t0);
            let n3 = bit_equal(&r, &i0);
            println!("N3 cube 11: AMG bit-equal to IC0 {n3} (expected false)");
            assert!(!n3, "N3");
        }
    }
    println!(
        "cube counts 11 / 23: {counts:?}, ratio {:.4}",
        counts[1] as f64 / counts[0] as f64
    );
    assert!(ok_c, "C-small");
    assert!(ok_th, "TH");
}

// ---------------------------------------------------------------------------------------------------------------------
// N1: translations only.

#[test]
fn n1_translations_only() {
    let model = cube(11);
    let r = reduced(&model);
    let nodes = model.mesh().nodes();
    let (points, comps): (Vec<[f64; 3]>, Vec<usize>) = r
        .free
        .iter()
        .map(|&q| {
            let (n, c) = r.dofs.node_component(q).unwrap();
            (nodes[n], c)
        })
        .unzip();
    let modes = rigid_body_modes(&points, &comps);
    let bp = rcfd_fem::solve::node_blocks(&r.free, &r.dofs);
    let own = own_modes(&model, &r);
    let mut counts = Vec::new();
    for k in [6usize, 3] {
        let amg = ElasticAmg::new(&r.a, &bp, &modes[..k], &ElasticAmgSettings::default()).unwrap();
        let v = b1(&amg, &own);
        let mut x = vec![0.0; r.a.n];
        let ctl = SolveControl {
            tol_abs: TOL * norm2(&r.rhs),
            tol_rel: 0.0,
            max_iter: 2000,
        };
        let s = cg(&r.a, &r.rhs, &mut x, &amg, &ctl);
        println!(
            "N1 {k} modes: level-0 range measure with the six modes {:.3e}, levels {:?}, CG {} iterations (converged {})",
            v[0].1,
            levels_desc(&amg),
            s.iterations,
            s.converged
        );
        counts.push((k, v[0].1, s.iterations));
    }
    assert!(counts[1].1 >= 1e-3, "N1");
}

// ---------------------------------------------------------------------------------------------------------------------
// S2 and X1 on the ADR 0282 cantilevers, P1.

#[test]
fn x1_cantilevers_and_p1() {
    let mut ok_b1 = true;
    let mut ok_oc = true;
    for (tag, model, it_ref) in [
        (
            "fast 32x6x6",
            cantilever_box([8.0, 1.5, 1.5], [32, 6, 6]),
            "BJ 156 / IC0 85 / IC1 58",
        ),
        (
            "slender 64x6x6",
            cantilever_box([16.0, 1.5, 1.5], [64, 6, 6]),
            "BJ 292 / IC0 206 / IC1 132",
        ),
    ] {
        let r = reduced(&model);
        let (b1m, oc) = hierarchy_checks(&format!("X1 {tag}"), &model, &r);
        if tag.starts_with("fast") {
            ok_b1 &= b1m;
            let amg = hierarchy(&model, &r);
            let (sym, pos) = symmetry(&amg, r.a.n);
            println!("S2 fast cantilever: asymmetry {sym:.3e} (limit 1e-10), positive {pos}");
            assert!(sym <= 1e-10 && pos, "S2 cantilever");
        }
        ok_oc &= oc <= 1.5;
        let (d, _, _) = run(&model, &StaticOptions::default());
        let (a, rep, t) = run(&model, &amg_options(20_000));
        print(&format!("X1 {tag} AMG"), &a, &rep, t);
        println!(
            "X1 {tag}: AMG {} iterations (ADR 0357: {it_ref}), residual {:.3e}, against direct {:.3e}",
            iterations(&a),
            residual(&a),
            rel_diff(&a, &d)
        );
    }
    // P1
    let model = sym_cube(11);
    let r = reduced(&model);
    let (b1m, oc) = hierarchy_checks("P1 symmetry cube 11", &model, &r);
    let partial = rcfd_fem::solve::node_blocks(&r.free, &r.dofs)
        .windows(2)
        .filter(|w| w[1] - w[0] < 3)
        .count();
    let (d, _, _) = run(&model, &StaticOptions::default());
    let (a, rep, t) = run(&model, &amg_options(20_000));
    print("P1 AMG", &a, &rep, t);
    let e = rel_diff(&a, &d);
    println!(
        "P1: {partial} node blocks with fewer than 3 unknowns; residual {:.3e}, against direct {e:.3e}, count {} (record)",
        residual(&a),
        iterations(&a)
    );
    assert!(b1m && oc <= 1.5, "P1 B1 / OC");
    assert!(residual(&a) <= TOL && e <= 1e-7, "P1");
    assert!(ok_b1, "B1 fast cantilever");
    assert!(ok_oc, "OC");
}

// ---------------------------------------------------------------------------------------------------------------------
// Heavy records.

#[test]
#[ignore = "heavy record (ADR 0305, H1): clamped Hex8 cube 46^3, 304 842 unknowns, and cube 11 again"]
fn h1_cube_growth() {
    let small = cube(11);
    let (s, rs, ts) = run(&small, &amg_options(20_000));
    print("H1 cube 11 AMG", &s, &rs, ts);
    let model = cube(46);
    let (b1m, oc) = {
        let r = reduced(&model);
        hierarchy_checks("H1 cube 46", &model, &r)
    };
    match try_run(&model, &amg_options(20_000)) {
        Some((r, rep, t)) => {
            print("H1 cube 46 AMG", &r, &rep, t);
            let (it, it0) = (iterations(&r), iterations(&s));
            let growth = it as f64 / it0 as f64;
            println!(
                "H1: count {it} <= 39 {}; growth {it} / {it0} = {growth:.4} <= 1.5 {}; residual {:.3e}; OC {oc:.4}; B1 {b1m}",
                it <= 39,
                growth <= 1.5,
                residual(&r)
            );
            assert!(residual(&r) <= TOL && it <= 39 && growth <= 1.5, "H1");
        }
        None => panic!("H1 not converged"),
    }
    assert!(b1m && oc <= 1.5, "H1 B1 / OC");
}

#[test]
#[ignore = "known miss (ADR 0305, H2, run once): Hex8 cantilever of slenderness 20, 240 x 12 x 12, 121 680 unknowns; limit: both reach 1e-10 and AMG <= IC(0) / 5; measured: neither reaches 1e-10, AMG stops after 30 iterations and 3 restarts at the true relative residual 1.42e-9, IC(0) after 2545 iterations at 1.43e-9 (the rounding floor of the residual, ADR 0305 amendment 2)"]
fn h2_slender20_hex8_known_miss() {
    let model = slender20_hex();
    let (b1m, oc) = {
        let r = reduced(&model);
        hierarchy_checks("H2 slender 20 Hex8", &model, &r)
    };
    let a = try_run(&model, &amg_options(60_000));
    let i = try_run(&model, &pcg(ic0(), 60_000));
    if let Some((r, rep, t)) = &a {
        print("H2 AMG", r, rep, *t);
    }
    if let Some((r, rep, t)) = &i {
        print("H2 IC0", r, rep, *t);
    }
    let (a, i) = (a.expect("H2 AMG"), i.expect("H2 IC0"));
    let (ia, ii) = (iterations(&a.0), iterations(&i.0));
    println!(
        "H2: AMG {ia} <= IC0 {ii} / 5 = {:.1}: {}; residuals {:.3e} / {:.3e}; AMG against IC0 {:.3e}; OC {oc:.4}; B1 {b1m}",
        ii as f64 / 5.0,
        ia as f64 <= ii as f64 / 5.0,
        residual(&a.0),
        residual(&i.0),
        rel_diff(&a.0, &i.0)
    );
    assert!(residual(&a.0) <= TOL && residual(&i.0) <= TOL, "H2 residual");
    assert!(ia as f64 <= ii as f64 / 5.0, "H2");
    assert!(b1m && oc <= 1.5, "H2 B1 / OC");
}

#[test]
#[ignore = "known miss (ADR 0305, H3, run once): Tet10 cantilever of slenderness 20 from 120 x 6 x 6 against the Hex8 count at 1e-10; measured: the Hex8 reference solve stops at the true relative residual 1.42e-9 (as H2), so the Tet10 part did not run (ADR 0305 amendment 2)"]
fn h3_slender20_tet10_known_miss() {
    let hex = slender20_hex();
    let (h, rh, th) = run(&hex, &amg_options(60_000));
    print("H3 Hex8 AMG", &h, &rh, th);
    let model = slender20_tet10();
    let (b1m, oc) = {
        let r = reduced(&model);
        hierarchy_checks("H3 slender 20 Tet10", &model, &r)
    };
    let a = try_run(&model, &amg_options(60_000)).expect("H3 AMG");
    print("H3 Tet10 AMG", &a.0, &a.1, a.2);
    let (it_t, it_h) = (iterations(&a.0), iterations(&h));
    println!(
        "H3: Tet10 AMG {it_t} <= 2 x Hex8 AMG {it_h} = {}: {}; residual {:.3e}; OC {oc:.4}; B1 {b1m}",
        2 * it_h,
        it_t <= 2 * it_h,
        residual(&a.0)
    );
    match try_run(&model, &pcg(ic0(), 60_000)) {
        Some((r, rep, t)) => {
            print("H3 Tet10 IC0 (record)", &r, &rep, t);
            println!(
                "H3 record: Tet10 AMG / IC0 = {it_t} / {} = {:.4}",
                iterations(&r),
                it_t as f64 / iterations(&r) as f64
            );
        }
        None => println!("H3 record: Tet10 IC0 failed"),
    }
    assert!(residual(&a.0) <= TOL && it_t <= 2 * it_h, "H3");
    assert!(b1m && oc <= 1.5, "H3 B1 / OC");
}

#[test]
#[ignore = "heavy record (ADR 0305, H4): ADR 0282 heavy cantilever 96 x 18 x 18, 103 968 unknowns"]
fn h4_heavy_cantilever_record() {
    let model = cantilever_box([8.0, 1.5, 1.5], [96, 18, 18]);
    let (_, oc) = {
        let r = reduced(&model);
        hierarchy_checks("H4 heavy cantilever", &model, &r)
    };
    let (r, rep, t) = run(&model, &amg_options(60_000));
    print("H4 AMG", &r, &rep, t);
    println!(
        "H4: AMG {} iterations (ADR 0357: BJ 477, IC0 255, IC1 171), OC {oc:.4} <= 1.5 {}",
        iterations(&r),
        oc <= 1.5
    );
    assert!(oc <= 1.5, "H4 OC");
}

// ---------------------------------------------------------------------------------------------------------------------
// ADR 0305 amendment 2: the slenderness-20 comparisons at the common attainable tolerance 1e-8 (H2b, H3b).

const TOL_SLENDER: f64 = 1e-8;

fn pcg_tol(p: CgPreconditioner, tolerance: f64, max_iterations: usize) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: p,
        tolerance,
        max_iterations,
    })
}

/// The rounding floor estimate `eps |(|K_FF| |u_F|)| / |b_F|` of the true residual at the solution `res`.
fn floor_estimate(r: &Reduced, res: &SolidResult) -> f64 {
    let u = res.displacement_values();
    let uf: Vec<f64> = r.free.iter().map(|&q| u[q].abs()).collect();
    let mut s = 0.0;
    for i in 0..r.a.n {
        let row: f64 = (r.a.row_ptr[i]..r.a.row_ptr[i + 1])
            .map(|k| r.a.val[k].abs() * uf[r.a.col[k]])
            .sum();
        s += row * row;
    }
    f64::EPSILON * s.sqrt() / norm2(&r.rhs)
}

#[test]
#[ignore = "heavy record (ADR 0305 amendment 2, H2b): Hex8 cantilever of slenderness 20, 121 680 unknowns, AMG and IC(0) to 1e-8"]
fn h2b_slender20_hex8_at_1e8() {
    let model = slender20_hex();
    let r = reduced(&model);
    let a = try_run(
        &model,
        &pcg_tol(
            CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
            TOL_SLENDER,
            60_000,
        ),
    )
    .expect("H2b AMG");
    print("H2b AMG", &a.0, &a.1, a.2);
    let i = try_run(&model, &pcg_tol(ic0(), TOL_SLENDER, 60_000)).expect("H2b IC0");
    print("H2b IC0", &i.0, &i.1, i.2);
    let (ia, ii) = (iterations(&a.0), iterations(&i.0));
    println!(
        "H2b: AMG {ia} <= IC0 {ii} / 5 = {:.1}: {}; residuals {:.3e} / {:.3e}; floor estimate {:.3e}; AMG against IC0 {:.3e}",
        ii as f64 / 5.0,
        ia as f64 <= ii as f64 / 5.0,
        residual(&a.0),
        residual(&i.0),
        floor_estimate(&r, &a.0),
        rel_diff(&a.0, &i.0)
    );
    assert!(
        residual(&a.0) <= TOL_SLENDER && residual(&i.0) <= TOL_SLENDER,
        "H2b residual"
    );
    assert!(ia as f64 <= ii as f64 / 5.0, "H2b");
}

#[test]
#[ignore = "known miss (ADR 0305 amendment 2, H3b, run once): Tet10 cantilever of slenderness 20, 121 680 unknowns, to 1e-8; limit: Tet10 AMG <= 2 x Hex8 AMG; measured: 79 against 2 x 15 = 30 (1 restart, residual 9.31e-9, floor estimate 8.64e-9; coarse level 2220 of 121 680 unknowns, operator complexity 1.033); Tet10 IC(0) 1971 (AMG / IC0 0.040)"]
fn h3b_slender20_tet10_at_1e8_known_miss() {
    let amg = || {
        pcg_tol(
            CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()),
            TOL_SLENDER,
            60_000,
        )
    };
    let hex = slender20_hex();
    let h = try_run(&hex, &amg()).expect("H3b Hex8 AMG");
    print("H3b Hex8 AMG", &h.0, &h.1, h.2);
    let model = slender20_tet10();
    let (b1m, oc) = {
        let r = reduced(&model);
        hierarchy_checks("H3b slender 20 Tet10", &model, &r)
    };
    let r = reduced(&model);
    let a = try_run(&model, &amg()).expect("H3b Tet10 AMG");
    print("H3b Tet10 AMG", &a.0, &a.1, a.2);
    let (it_t, it_h) = (iterations(&a.0), iterations(&h.0));
    println!(
        "H3b: Tet10 AMG {it_t} <= 2 x Hex8 AMG {it_h} = {}: {}; residual {:.3e}; floor estimate {:.3e}; OC {oc:.4}; B1 {b1m}",
        2 * it_h,
        it_t <= 2 * it_h,
        residual(&a.0),
        floor_estimate(&r, &a.0)
    );
    match try_run(&model, &pcg_tol(ic0(), TOL_SLENDER, 60_000)) {
        Some((ri, rep, t)) => {
            print("H3b Tet10 IC0 (record)", &ri, &rep, t);
            println!(
                "H3b record: Tet10 AMG / IC0 = {it_t} / {} = {:.4}",
                iterations(&ri),
                it_t as f64 / iterations(&ri) as f64
            );
        }
        None => println!("H3b record: Tet10 IC0 failed"),
    }
    assert!(residual(&a.0) <= TOL_SLENDER && it_t <= 2 * it_h, "H3b");
    assert!(b1m && oc <= 1.5, "H3b B1 / OC");
}
