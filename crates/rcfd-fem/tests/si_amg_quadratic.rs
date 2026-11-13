//! ADR 0370 (F-470): the opt-in geometric aggregation of the elasticity multigrid that coarsens Tet10 like Hex8. Fast
//! checks DF, E1 to E5 and the negative control N1; the heavy record H1 (`#[ignore]`) carries the acceptance on the
//! Tet10 cantilever of slenderness 20 and the default checks D5, D6. Set-ups copied from `si_amg.rs` (ADR 0305).

use rcfd_fem::assemble::stiffness_with;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::{prescribed_equations, reduce};
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::mesh::SolidMesh;
use rcfd_fem::solve::{
    node_blocks, smoothed_aggregation, solve_static, solve_static_report, CgPreconditioner, StaticOptions, StaticSolver,
};
use rcfd_fem::{Constraint, DofMap, Load, Material, SolidModel, SolidResult};
use rcfd_linalg::amg_elastic::{
    rigid_body_modes, Aggregation, ElasticAmg, ElasticAmgError, ElasticAmgSettings, QUADRATIC_RATIO,
};
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{CsrMatrix, Preconditioner};
use std::collections::BTreeMap;
use std::time::Instant;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const TOL_SLENDER: f64 = 1e-8;

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

fn cube(n: usize) -> SolidModel {
    clamped(hex_box([0.0; 3], [1.0; 3], [n; 3]).unwrap())
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

fn defect(b: &[Vec<f64>], p: &CsrMatrix, coarse: &[Vec<f64>]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (col, c) in b.iter().zip(coarse) {
        let pc = matvec(p, c);
        for i in 0..p.n {
            den += col[i] * col[i];
            num += (col[i] - pc[i]).powi(2);
        }
    }
    (num / den).sqrt()
}

/// B1 of ADR 0305 on every level; returns whether every measure is `<= 1e-12` and the largest.
fn b1(amg: &ElasticAmg, own: &[Vec<f64>], tag: &str) -> (bool, f64) {
    let (mut ok, mut worst) = (true, 0.0f64);
    for l in 0..amg.n_levels() {
        let Some(pt) = amg.tentative(l) else { break };
        let b: Vec<Vec<f64>> = if l == 0 {
            own.to_vec()
        } else {
            amg.near_null(l).to_vec()
        };
        let proj: Vec<Vec<f64>> = b.iter().map(|c| tmatvec(pt, c)).collect();
        let range = defect(&b, pt, &proj);
        let lib = defect(amg.near_null(l), pt, amg.near_null(l + 1));
        println!("  B1 {tag} level {l}: range {range:.3e}, |B_l - P_t B_l+1| / |B_l| {lib:.3e}");
        ok &= range <= 1e-12 && (l == 0 || lib <= 1e-12);
        worst = worst.max(range).max(if l == 0 { 0.0 } else { lib });
    }
    (ok, worst)
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

/// E2: an independent implementation of ADR 0370 D1 on one level.
fn reference_aggregates(a: &CsrMatrix, bp: &[usize], pts: &[[f64; 3]], ratio: f64) -> Vec<usize> {
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
    for &b in &order {
        if agg[b].is_some() {
            continue;
        }
        let mut best: Option<(f64, usize)> = None;
        for &(c, w) in &cands[b] {
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
    agg.into_iter().map(|g| g.unwrap()).collect()
}

fn centroids(agg: &[usize], pts: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let n = agg.iter().max().map_or(0, |m| m + 1);
    let mut s = vec![[0.0; 3]; n];
    let mut c = vec![0.0; n];
    for (b, &g) in agg.iter().enumerate() {
        for d in 0..3 {
            s[g][d] += pts[b][d];
        }
        c[g] += 1.0;
    }
    (0..n)
        .map(|g| [s[g][0] / c[g], s[g][1] / c[g], s[g][2] / c[g]])
        .collect()
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

// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn df_settings() {
    let d = ElasticAmgSettings::default();
    let ok_default = d.theta == 0.0
        && d.max_coarse == 1000
        && d.max_levels == 10
        && d.pre_sweeps == 1
        && d.post_sweeps == 1
        && d.drop_tolerance == 1e-10
        && d.aggregation == Aggregation::Greedy;
    let q = ElasticAmgSettings::quadratic();
    let rho = 0.5 * (3.0f64.sqrt() + 2.0);
    let ratio = match q.aggregation {
        Aggregation::Geometric { ratio } => ratio,
        _ => f64::NAN,
    };
    let ok_q = ElasticAmgSettings {
        aggregation: Aggregation::Greedy,
        ..q
    } == d
        && (ratio - rho).abs() <= 1e-15
        && QUADRATIC_RATIO == ratio;
    println!("DF: default as ADR 0305 with Greedy {ok_default}; quadratic ratio {ratio:.17} against {rho:.17}: {ok_q}");
    assert!(ok_default && ok_q, "DF");
}

#[test]
fn e1_hex8_cube_identity() {
    let model = cube(11);
    let r = reduced(&model);
    let d = hierarchy(&model, &r, &ElasticAmgSettings::default());
    let q = hierarchy(&model, &r, &ElasticAmgSettings::quadratic());
    let same_agg = d.aggregates(0) == q.aggregates(0);
    let o = |s| {
        StaticOptions::with_solver(StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::SmoothedAggregation(s),
            tolerance: 1e-10,
            max_iterations: 20_000,
        })
    };
    let sd = solve_static(&model, &o(ElasticAmgSettings::default())).unwrap();
    let sq = solve_static(&model, &o(ElasticAmgSettings::quadratic())).unwrap();
    println!(
        "E1 cube 11: level-0 aggregates equal {same_agg}; levels default {:?} quadratic {:?}; counts {} / {}; bit-equal {}",
        levels_desc(&d),
        levels_desc(&q),
        iterations(&sd),
        iterations(&sq),
        bit_equal(&sd, &sq)
    );
    assert!(
        same_agg && iterations(&sd) == iterations(&sq) && bit_equal(&sd, &sq),
        "E1"
    );
}

#[test]
fn e2_reference_aggregation() {
    let model = fast_tet10();
    let r = reduced(&model);
    let amg = hierarchy(&model, &r, &ElasticAmgSettings::quadratic());
    let rho = 0.5 * (3.0f64.sqrt() + 2.0);
    let bp0 = node_blocks(&r.free, &r.dofs);
    let p0 = block_points(&model, &r, &bp0);
    let ref0 = reference_aggregates(amg.matrix(0), &bp0, &p0, rho);
    let lib0 = amg.aggregates(0).expect("level 0 coarsened").to_vec();
    let ok0 = ref0 == lib0;
    let n_agg0 = ref0.iter().max().unwrap() + 1;
    let mut ok1 = None;
    if amg.n_levels() > 2 {
        let bp1 = amg.block_ptr(1).to_vec();
        assert_eq!(bp1.len() - 1, n_agg0, "every aggregate has coarse unknowns");
        let p1 = centroids(&lib0, &p0);
        let ref1 = reference_aggregates(amg.matrix(1), &bp1, &p1, rho);
        ok1 = Some(amg.aggregates(1).map(|a| a == ref1.as_slice()).unwrap_or(false));
    }
    println!(
        "E2 fast Tet10: levels {:?}; level 0 {} aggregates, equal to the reference {ok0}; level 1 equal {:?}",
        levels_desc(&amg),
        n_agg0,
        ok1
    );
    assert!(ok0 && ok1 == Some(true), "E2");
}

#[test]
fn e3_e4_n1_fast_tet10() {
    let model = fast_tet10();
    let r = reduced(&model);
    let t = Instant::now();
    let q = hierarchy(&model, &r, &ElasticAmgSettings::quadratic());
    let setup = t.elapsed().as_secs_f64();
    let d = hierarchy(&model, &r, &ElasticAmgSettings::default());
    let (b1ok, b1w) = b1(&q, &own_modes(&model, &r), "fast Tet10");
    let (sym, pos) = symmetry(&q, r.a.n);
    let n0 = q.matrix(0).n;
    let (n1q, n1d) = (q.matrix(1).n, d.matrix(1).n);
    println!(
        "E3 fast Tet10 quadratic: levels {:?}, OC {:.4}, n_1 / n_0 = {n1q} / {n0} = 1/{:.2}, setup {setup:.2} s; default levels {:?}, OC {:.4}",
        levels_desc(&q),
        q.operator_complexity(),
        n0 as f64 / n1q as f64,
        levels_desc(&d),
        d.operator_complexity()
    );
    println!("E3 B1 largest {b1w:.3e} (limit 1e-12): {b1ok}; S2 asymmetry {sym:.3e} (limit 1e-10), positive {pos}");
    let t = Instant::now();
    let (sq, rq) = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::quadratic(), TOL_SLENDER)).unwrap();
    let tq = t.elapsed().as_secs_f64();
    let (sd, rd) = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::default(), TOL_SLENDER)).unwrap();
    println!(
        "E3 CG to 1e-8: quadratic {} iterations, {} restarts, residual {:.3e}, {tq:.2} s; default {} iterations, {} restarts, residual {:.3e}",
        rq.iterations,
        rq.restarts,
        residual(&sq),
        rd.iterations,
        rd.restarts,
        residual(&sd)
    );
    // E4
    let mut runs = Vec::new();
    for th in [1usize, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(th).build().unwrap();
        let o = pcg(ElasticAmgSettings::quadratic(), TOL_SLENDER).threads(th);
        runs.push(pool.install(|| solve_static(&model, &o).unwrap()));
    }
    let e4 = iterations(&runs[0]) == iterations(&runs[1]) && bit_equal(&runs[0], &runs[1]);
    println!(
        "E4 fast Tet10 quadratic 1 / 8 threads: {} / {} iterations, bit-equal {}",
        iterations(&runs[0]),
        iterations(&runs[1]),
        bit_equal(&runs[0], &runs[1])
    );
    let n1 = n1q as f64 / n1d as f64;
    println!("N1 fast Tet10: first coarse level quadratic / default = {n1q} / {n1d} = {n1:.3} (expected >= 2)");
    assert!(b1ok, "E3 B1");
    assert!(sym <= 1e-10 && pos, "E3 S2");
    assert!(residual(&sq) <= TOL_SLENDER, "E3 CG");
    assert!(e4, "E4");
    assert!(n1 >= 2.0, "N1");
}

#[test]
fn e5_refusals() {
    let model = cube(3);
    let r = reduced(&model);
    let bp = node_blocks(&r.free, &r.dofs);
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
    let q = ElasticAmgSettings::quadratic();
    let a = ElasticAmg::new(&r.a, &bp, &modes, &q).err();
    let pts = block_points(&model, &r, &bp);
    let b = ElasticAmg::with_points(&r.a, &bp, &modes, &pts[1..], &q).err();
    println!("E5: new with quadratic {a:?}; with_points one point short {b:?}");
    assert_eq!(a, Some(ElasticAmgError::MissingPoints), "E5 new");
    assert!(
        matches!(b, Some(ElasticAmgError::PointsLength { expected, got }) if expected == pts.len() && got == pts.len() - 1),
        "E5 with_points"
    );
}

// ---------------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "known miss (ADR 0370, H1, run once): Tet10 cantilever of slenderness 20, 121 680 unknowns, geometric aggregation; limit: CG with one V-cycle to 1e-8 in <= 30 iterations; measured: 31 iterations, 0 restarts, residual 9.17e-9 (floor estimate 8.64e-9); H1a first coarse level 12 000 (1/10.14, in [8112, 20280]), H1c OC 1.4762, H1d B1 2.8e-16, D5 Hex8 default 15, D6 Tet10 default 2220, N2 held"]
fn h1_slender20_tet10_quadratic_known_miss() {
    // D5: Hex8 default at 1e-8 (ADR 0305 H2b: 15)
    let hex = slender20_hex();
    let t = Instant::now();
    let (sh, rh) = solve_static_report(&hex, &[], &[], &pcg(ElasticAmgSettings::default(), TOL_SLENDER)).unwrap();
    println!(
        "D5 Hex8 slender 20 default: {} iterations, {} restarts, residual {:.3e}, {:.1} s",
        rh.iterations,
        rh.restarts,
        residual(&sh),
        t.elapsed().as_secs_f64()
    );
    let d5 = iterations(&sh) == 15;
    // record: Hex8 with the geometric rule
    let t = Instant::now();
    let (shq, rhq) = solve_static_report(&hex, &[], &[], &pcg(ElasticAmgSettings::quadratic(), TOL_SLENDER)).unwrap();
    println!(
        "record Hex8 slender 20 quadratic: {} iterations, {} restarts, residual {:.3e}, {:.1} s",
        rhq.iterations,
        rhq.restarts,
        residual(&shq),
        t.elapsed().as_secs_f64()
    );
    drop((sh, shq));
    let model = slender20_tet10();
    let r = reduced(&model);
    // D6, N2: default Tet10 first coarse level
    let n1d = {
        let d = hierarchy(&model, &r, &ElasticAmgSettings::default());
        println!(
            "D6 Tet10 default: levels {:?}, OC {:.4}",
            levels_desc(&d),
            d.operator_complexity()
        );
        d.matrix(1).n
    };
    let (lo, hi) = (121_680.0 / 15.0, 121_680.0 / 6.0);
    let d6 = n1d == 2220;
    let n2 = !(n1d as f64 >= lo && n1d as f64 <= hi);
    println!("D6: default first coarse level {n1d} (== 2220: {d6}); N2 outside [{lo:.0}, {hi:.0}]: {n2}");
    // H1a, H1c, H1d
    let t = Instant::now();
    let q = hierarchy(&model, &r, &ElasticAmgSettings::quadratic());
    let setup = t.elapsed().as_secs_f64();
    let n0 = q.matrix(0).n;
    let n1 = q.matrix(1).n;
    let oc = q.operator_complexity();
    println!(
        "H1 Tet10 quadratic: levels {:?}, setup {setup:.2} s, omega {:?}",
        levels_desc(&q),
        (0..q.n_levels()).map(|l| q.omega(l)).collect::<Vec<_>>()
    );
    let h1a = n1 as f64 >= lo && n1 as f64 <= hi;
    println!(
        "H1a: first coarse level {n1} of {n0} = 1/{:.3}, in [{lo:.0}, {hi:.0}]: {h1a}",
        n0 as f64 / n1 as f64
    );
    let h1c = oc <= 1.5;
    println!("H1c: operator complexity {oc:.4} <= 1.5: {h1c}");
    let (h1d, w) = b1(&q, &own_modes(&model, &r), "H1 Tet10");
    println!("H1d: B1 largest {w:.3e} <= 1e-12: {h1d}");
    drop(q);
    // H1b
    let t = Instant::now();
    let res = solve_static_report(&model, &[], &[], &pcg(ElasticAmgSettings::quadratic(), TOL_SLENDER));
    let secs = t.elapsed().as_secs_f64();
    let h1b = match &res {
        Ok((s, rep)) => {
            println!(
                "H1b: {} iterations (limit 30), {} restarts, residual {:.3e} (limit 1e-8), floor estimate {:.3e}, {secs:.1} s",
                rep.iterations,
                rep.restarts,
                residual(s),
                floor_estimate(&r, s)
            );
            residual(s) <= TOL_SLENDER && iterations(s) <= 30
        }
        Err(e) => {
            println!("H1b: solve error {e}");
            false
        }
    };
    println!("H1 summary: H1a {h1a}, H1b {h1b}, H1c {h1c}, H1d {h1d}, D5 {d5}, D6 {d6}, N2 {n2}");
    assert!(h1a && h1b && h1c && h1d && d5 && d6 && n2, "H1");
}
