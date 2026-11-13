//! ADR 0375 (F-477): the opt-in smoother setting `ElasticAmgSettings::quadratic_smoothed()` (two block Gauss-Seidel sweeps
//! per side on top of the geometric aggregation of ADR 0370), and graded Tet10 boxes on which pass 2's restriction to the
//! candidates changes the aggregates (ADR 0370 mutation M5). Fast checks DF2, F1, F2, G and the negative control N1; the
//! heavy record H1 (`#[ignore]`) carries the acceptance on the Tet10 cantilever of slenderness 20 and the check D5. Set-ups
//! copied from `si_amg_quadratic.rs` (ADR 0370), not imported or edited.

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::solve::{
    node_blocks, smoothed_aggregation, solve_static, solve_static_report, CgPreconditioner, StaticOptions, StaticSolver,
};
use rcfd_fem::{Constraint, DofMap, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::amg_elastic::{Aggregation, ElasticAmg, ElasticAmgSettings, QUADRATIC_RATIO, QUADRATIC_SWEEPS};
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{CsrMatrix, Preconditioner};
use std::collections::BTreeMap;
use std::time::Instant;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const TOL: f64 = 1e-8;

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

/// The fast Tet10 case of ADR 0370: Kuhn 32 x 6 x 6 on `[0, 8] x [0, 1.5]^2`, upgraded.
fn fast_tet10() -> SolidModel {
    let (m10, _) = upgrade_to_tet10(&tet_box([0.0; 3], [8.0, 1.5, 1.5], [32, 6, 6]).unwrap()).unwrap();
    clamped(m10)
}

fn slender20_hex() -> SolidModel {
    clamped(hex_box([0.0; 3], [20.0, 1.0, 1.0], [240, 12, 12]).unwrap())
}

fn slender20_tet10() -> SolidModel {
    let (m10, _) = upgrade_to_tet10(&tet_box([0.0; 3], [20.0, 1.0, 1.0], [120, 6, 6]).unwrap()).unwrap();
    clamped(m10)
}

/// A Kuhn box whose corner coordinate `i L / n` along each axis is mapped to `L (g^i - 1) / (g^n - 1)` (`g = 1`: unchanged),
/// with every node, element and surface set copied, then upgraded to Tet10 (ADR 0375 arithmetic 6).
fn graded_tet10(lengths: [f64; 3], n: [usize; 3], growth: [f64; 3]) -> SolidModel {
    let base = tet_box([0.0; 3], lengths, n).unwrap();
    let map = |x: f64, a: usize| -> f64 {
        let g = growth[a];
        if g == 1.0 {
            return x;
        }
        let i = (x * n[a] as f64 / lengths[a]).round();
        lengths[a] * (g.powf(i) - 1.0) / (g.powi(n[a] as i32) - 1.0)
    };
    let nodes: Vec<[f64; 3]> = base
        .nodes()
        .iter()
        .map(|p| [map(p[0], 0), map(p[1], 1), map(p[2], 2)])
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    for b in base.blocks() {
        mesh.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    for name in base.node_set_names() {
        mesh.add_node_set(name, base.node_set(name).unwrap().to_vec()).unwrap();
    }
    for name in base.element_set_names() {
        mesh.add_element_set(name, base.element_set(name).unwrap().to_vec())
            .unwrap();
    }
    for name in base.surface_set_names() {
        mesh.add_surface_set(name, base.surface_set(name).unwrap().to_vec())
            .unwrap();
    }
    let (m10, _) = upgrade_to_tet10(&mesh).unwrap();
    clamped(m10)
}

/// G1: graded in x (growth 1.25 per cell), so also anisotropic.
fn g1() -> SolidModel {
    graded_tet10([4.0, 1.5, 1.5], [16, 6, 6], [1.25, 1.0, 1.0])
}

/// G2: graded in all three directions (growth 1.2, 1.3, 1.4).
fn g2() -> SolidModel {
    graded_tet10([3.0, 1.0, 1.0], [12, 8, 8], [1.2, 1.3, 1.4])
}

fn pcg(settings: ElasticAmgSettings, tolerance: f64) -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregation(settings),
        tolerance,
        max_iterations: 60_000,
    })
}

struct Reduced {
    a: CsrMatrix,
    rhs: Vec<f64>,
    free: Vec<usize>,
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
        rhs: sys.rhs.clone(),
        a: sys.matrix,
        dofs,
    }
}

fn hierarchy(model: &SolidModel, r: &Reduced, s: &ElasticAmgSettings) -> ElasticAmg {
    smoothed_aggregation(&r.a, &r.free, &r.dofs, model.mesh().nodes(), s).unwrap()
}

fn levels_desc(amg: &ElasticAmg) -> Vec<(usize, usize)> {
    (0..amg.n_levels())
        .map(|l| (amg.matrix(l).n, amg.matrix(l).nnz()))
        .collect()
}

fn iterations(r: &SolidResult) -> usize {
    r.stats().unwrap().iterations()
}

fn residual(r: &SolidResult) -> f64 {
    r.stats().unwrap().relative_residual()
}

fn bit_equal(a: &SolidResult, b: &SolidResult) -> bool {
    a.displacement_values()
        .iter()
        .zip(b.displacement_values())
        .all(|(p, q)| p.to_bits() == q.to_bits())
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

/// The test's own point of each level-0 block (the node of its first free equation).
fn block_points(model: &SolidModel, r: &Reduced, bp: &[usize]) -> Vec<[f64; 3]> {
    let nodes = model.mesh().nodes();
    bp[..bp.len() - 1]
        .iter()
        .map(|&s| nodes[r.dofs.node_component(r.free[s]).unwrap().0])
        .collect()
}

/// An independent implementation of ADR 0370 D1 on one level (copied from ADR 0370 E2) with one flag: `pass2_all` makes
/// pass 2 scan every stored neighbour instead of the candidates (ADR 0370 mutation M5). Returns the aggregates and the
/// number of blocks left after pass 1.
fn reference_aggregates(
    a: &CsrMatrix,
    bp: &[usize],
    pts: &[[f64; 3]],
    ratio: f64,
    pass2_all: bool,
) -> (Vec<usize>, usize) {
    let nb = bp.len() - 1;
    let mut blk = vec![0usize; a.n];
    for b in 0..nb {
        blk[bp[b]..bp[b + 1]].iter_mut().for_each(|x| *x = b);
    }
    let mut weights: Vec<BTreeMap<usize, f64>> = vec![BTreeMap::new(); nb];
    for i in 0..a.n {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let j = blk[a.col[k]];
            if j != blk[i] {
                *weights[blk[i]].entry(j).or_insert(0.0) += a.val[k] * a.val[k];
            }
        }
    }
    let d = |i: usize, j: usize| {
        let s: f64 = (0..3).map(|c| (pts[j][c] - pts[i][c]).powi(2)).sum();
        s.sqrt()
    };
    let cands: Vec<Vec<(usize, f64)>> = (0..nb)
        .map(|i| {
            let near = weights[i].keys().map(|&j| d(i, j)).fold(f64::INFINITY, f64::min);
            weights[i]
                .iter()
                .filter(|(&j, _)| d(i, j) <= ratio * near)
                .map(|(&j, &w)| (j, w.sqrt()))
                .collect()
        })
        .collect();
    let all: Vec<Vec<(usize, f64)>> = (0..nb)
        .map(|i| weights[i].iter().map(|(&j, &w)| (j, w.sqrt())).collect())
        .collect();
    let mut order: Vec<usize> = (0..nb).collect();
    order.sort_by(|&i, &j| {
        let key = |b: usize| [pts[b][2], pts[b][1], pts[b][0]];
        let (ki, kj) = (key(i), key(j));
        (0..3)
            .map(|c| ki[c].total_cmp(&kj[c]))
            .find(|o| o.is_ne())
            .unwrap_or(i.cmp(&j))
    });
    let mut agg: Vec<Option<usize>> = vec![None; nb];
    let mut next = 0;
    for &b in &order {
        if agg[b].is_none() && cands[b].iter().all(|(c, _)| agg[*c].is_none()) {
            agg[b] = Some(next);
            for (c, _) in &cands[b] {
                agg[*c] = Some(next);
            }
            next += 1;
        }
    }
    let first = agg.clone();
    let left = first.iter().filter(|g| g.is_none()).count();
    for &b in &order {
        if agg[b].is_some() {
            continue;
        }
        let mut best: Option<(f64, usize)> = None;
        let scan = if pass2_all { &all[b] } else { &cands[b] };
        for &(c, w) in scan {
            if let Some(g) = first[c] {
                if best.map_or(true, |(bw, _)| w > bw) {
                    best = Some((w, g));
                }
            }
        }
        agg[b] = best.map(|(_, g)| g);
    }
    for &b in &order {
        if agg[b].is_none() {
            agg[b] = Some(next);
            next += 1;
        }
    }
    (agg.into_iter().map(|g| g.unwrap()).collect(), left)
}

/// `eps |(|K_FF| |u_F|)| / |b_F|` at the solution (ADR 0305 amendment 2).
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

/// The library's level-0 aggregates of `quadratic()` and the two references on one model.
struct AggCompare {
    lib_equal: bool,
    differing: usize,
    left: usize,
    blocks: usize,
    n_agg: usize,
}

fn compare_pass2(model: &SolidModel) -> AggCompare {
    let r = reduced(model);
    let amg = hierarchy(model, &r, &ElasticAmgSettings::quadratic());
    let rho = 0.5 * (3.0f64.sqrt() + 2.0);
    let bp0 = node_blocks(&r.free, &r.dofs);
    let p0 = block_points(model, &r, &bp0);
    let (cand, left) = reference_aggregates(amg.matrix(0), &bp0, &p0, rho, false);
    let (all, _) = reference_aggregates(amg.matrix(0), &bp0, &p0, rho, true);
    let lib = amg.aggregates(0).expect("level 0 coarsened");
    AggCompare {
        lib_equal: lib == cand.as_slice(),
        differing: cand.iter().zip(&all).filter(|(p, q)| p != q).count(),
        left,
        blocks: bp0.len() - 1,
        n_agg: cand.iter().max().map_or(0, |m| m + 1),
    }
}

// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn df2_settings() {
    let d = ElasticAmgSettings::default();
    let ok_default = d.theta == 0.0
        && d.max_coarse == 1000
        && d.max_levels == 10
        && d.pre_sweeps == 1
        && d.post_sweeps == 1
        && d.drop_tolerance == 1e-10
        && d.aggregation == Aggregation::Greedy;
    let q = ElasticAmgSettings::quadratic();
    let ok_q = q
        == ElasticAmgSettings {
            aggregation: Aggregation::Geometric { ratio: QUADRATIC_RATIO },
            ..d
        };
    let s = ElasticAmgSettings::quadratic_smoothed();
    let ok_s = QUADRATIC_SWEEPS == 2
        && s == ElasticAmgSettings {
            pre_sweeps: 2,
            post_sweeps: 2,
            ..q
        };
    println!("DF2: default as ADR 0305 {ok_default}; quadratic as ADR 0370 {ok_q}; quadratic_smoothed {s:?}: {ok_s}");
    assert!(ok_default && ok_q && ok_s, "DF2");
}

#[test]
fn f1_f2_n2_fast_tet10() {
    let model = fast_tet10();
    let r = reduced(&model);
    let t = Instant::now();
    let q = hierarchy(&model, &r, &ElasticAmgSettings::quadratic());
    let setup_q = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let s = hierarchy(&model, &r, &ElasticAmgSettings::quadratic_smoothed());
    let setup_s = t.elapsed().as_secs_f64();
    let same_setup = levels_desc(&q) == levels_desc(&s);
    println!(
        "N2 fast Tet10: levels quadratic {:?} ({setup_q:.2} s), quadratic_smoothed {:?} ({setup_s:.2} s), equal {same_setup}; OC {:.4}",
        levels_desc(&q),
        levels_desc(&s),
        s.operator_complexity()
    );
    let (sym, pos) = symmetry(&s, r.a.n);
    println!("F1 S2 quadratic_smoothed: asymmetry {sym:.3e} (limit 1e-10), positive {pos}");
    drop((q, s));
    let t = Instant::now();
    let (sq, rq) = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::quadratic(), TOL)).unwrap();
    let tq = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let res = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::quadratic_smoothed(), TOL));
    let ts = t.elapsed().as_secs_f64();
    let cq = iterations(&sq);
    println!(
        "F1 CG to 1e-8 quadratic: {cq} iterations, {} restarts, residual {:.3e}, {tq:.2} s",
        rq.restarts,
        residual(&sq)
    );
    let f1_cg = match &res {
        Ok((ss, rs)) => {
            println!(
                "F1 CG to 1e-8 quadratic_smoothed: {} iterations, {} restarts, residual {:.3e}, {ts:.2} s",
                rs.iterations,
                rs.restarts,
                residual(ss)
            );
            residual(ss) <= TOL && iterations(ss) < cq
        }
        Err(e) => {
            println!("F1 quadratic_smoothed: solve error {e}");
            false
        }
    };
    let f1_q = cq == 27;
    println!("F1: quadratic count {cq} == 27 {f1_q}; quadratic_smoothed Ok, <= 1e-8 and fewer {f1_cg}");
    // F2
    let mut runs = Vec::new();
    for th in [1usize, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(th).build().unwrap();
        let o = pcg(ElasticAmgSettings::quadratic_smoothed(), TOL).threads(th);
        runs.push(pool.install(|| solve_static(&model, &o).unwrap()));
    }
    let f2 = iterations(&runs[0]) == iterations(&runs[1]) && bit_equal(&runs[0], &runs[1]);
    println!(
        "F2 fast Tet10 quadratic_smoothed 1 / 8 threads: {} / {} iterations, bit-equal {}",
        iterations(&runs[0]),
        iterations(&runs[1]),
        bit_equal(&runs[0], &runs[1])
    );
    assert!(same_setup, "N2 / F1 levels");
    assert!(sym <= 1e-10 && pos, "F1 S2");
    assert!(f1_q && f1_cg, "F1 CG");
    assert!(f2, "F2");
}

#[test]
fn g_graded_boxes_pass2_restriction() {
    let mut lib_ok = true;
    let mut seen = false;
    for (tag, model) in [("G1", g1()), ("G2", g2())] {
        let c = compare_pass2(&model);
        println!(
            "G {tag}: {} blocks, {} aggregates, {} left after pass 1; library equal to the reference {}; blocks whose aggregate differs with pass 2 over every neighbour {}",
            c.blocks, c.n_agg, c.left, c.lib_equal, c.differing
        );
        lib_ok &= c.lib_equal;
        seen |= c.differing >= 1;
    }
    // records on G1: convergence
    let model = g1();
    for (tag, s) in [
        ("default", ElasticAmgSettings::default()),
        ("quadratic", ElasticAmgSettings::quadratic()),
        ("quadratic_smoothed", ElasticAmgSettings::quadratic_smoothed()),
    ] {
        match solve_static_report(&model, &[], &[], &pcg(s, TOL)) {
            Ok((sr, rep)) => println!(
                "record G1 {tag}: {} iterations, {} restarts, residual {:.3e}",
                rep.iterations,
                rep.restarts,
                residual(&sr)
            ),
            Err(e) => println!("record G1 {tag}: solve error {e}"),
        }
    }
    println!(
        "G: (i) library equal on both {lib_ok}; (ii) the restriction changes the aggregates on at least one {seen}"
    );
    assert!(lib_ok, "G (i)");
    assert!(seen, "G (ii)");
}

#[test]
fn n1_uniform_box_pass2_equivalent() {
    let c = compare_pass2(&fast_tet10());
    println!(
        "N1 fast Tet10: {} blocks, {} left after pass 1; library equal {}; differing blocks {} (expected 0)",
        c.blocks, c.left, c.lib_equal, c.differing
    );
    assert_eq!(c.differing, 0, "N1");
}

// ---------------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "heavy record (ADR 0375, H1): Tet10 cantilever of slenderness 20, 121 680 unknowns"]
fn h1_slender20_tet10_quadratic_smoothed() {
    let model = slender20_tet10();
    let r = reduced(&model);
    let t = Instant::now();
    let q = hierarchy(&model, &r, &ElasticAmgSettings::quadratic());
    let setup_q = t.elapsed().as_secs_f64();
    let n1q = q.matrix(1).n;
    let levels_q = levels_desc(&q);
    drop(q);
    let t = Instant::now();
    let s = hierarchy(&model, &r, &ElasticAmgSettings::quadratic_smoothed());
    let setup_s = t.elapsed().as_secs_f64();
    let oc = s.operator_complexity();
    let same = levels_desc(&s) == levels_q;
    println!(
        "H1b: quadratic_smoothed levels {:?}, OC {oc:.4} (limit 1.5); equal to quadratic's {same}",
        levels_desc(&s)
    );
    drop(s);
    let h1b = oc <= 1.5 && same;
    // D5: quadratic()
    let t = Instant::now();
    let (sq, rq) = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::quadratic(), TOL)).unwrap();
    let tq = t.elapsed().as_secs_f64();
    println!(
        "D5 quadratic: {} iterations, {} restarts, residual {:.3e}, first coarse level {n1q}, floor estimate {:.3e}",
        rq.iterations,
        rq.restarts,
        residual(&sq),
        floor_estimate(&r, &sq)
    );
    let d5 = iterations(&sq) == 31 && n1q == 12_000;
    drop(sq);
    // H1a
    let t = Instant::now();
    let res = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::quadratic_smoothed(), TOL));
    let ts = t.elapsed().as_secs_f64();
    let h1a = match &res {
        Ok((ss, rs)) => {
            println!(
                "H1a quadratic_smoothed: {} iterations (limit 30), {} restarts, residual {:.3e} (limit 1e-8), floor estimate {:.3e}",
                rs.iterations,
                rs.restarts,
                residual(ss),
                floor_estimate(&r, ss)
            );
            residual(ss) <= TOL && iterations(ss) <= 30
        }
        Err(e) => {
            println!("H1a: solve error {e}");
            false
        }
    };
    drop(res);
    println!(
        "H1c times: setup quadratic {setup_q:.2} s, quadratic_smoothed {setup_s:.2} s; solve (assembly included) quadratic {tq:.2} s, quadratic_smoothed {ts:.2} s (ADR 0370: 31 iterations, 3.1 s)"
    );
    // record: Hex8 with quadratic_smoothed
    let hex = slender20_hex();
    let t = Instant::now();
    match solve_static_report(&hex, &[], &[], &pcg(ElasticAmgSettings::quadratic_smoothed(), TOL)) {
        Ok((sh, rh)) => println!(
            "record Hex8 slender 20 quadratic_smoothed: {} iterations, {} restarts, residual {:.3e}, {:.2} s",
            rh.iterations,
            rh.restarts,
            residual(&sh),
            t.elapsed().as_secs_f64()
        ),
        Err(e) => println!("record Hex8: solve error {e}"),
    }
    println!("H1 summary: H1a {h1a}, H1b {h1b}, H1c printed, D5 {d5}");
    assert!(h1a && h1b && d5, "H1");
}
