//! F-343 (ADR 0247): the target-count, edge-length-weighted and register-based adaptation criteria of `rcfd_fv::adaptive`, the
//! target-count runs on the ADR 0242 cavity and the suction-layer case where adaptation pays. Limits registered in ADR 0247
//! before any of these tests was run.
//!
//! The K checks are ordinary tests. The cavity checks (T) and the layer checks (L) each share one computation and are ignored as
//! heavy: run once in release with `--include-ignored`.

use rcfd_core::vec3::Vec3;
use rcfd_core::Result;
use rcfd_fv::adaptive::{
    cell_extents, children_per_split, edge_length_weighted_indicator, fixed_fraction_marks, register_marks,
    restrict_marks, run_adaptive_simple, run_adaptive_simple_marked, run_adaptive_simple_target,
    run_adaptive_simple_with, target_count_marks, target_schedule, velocity_edge_length_indicator,
    velocity_gradient_indicator, AdaptiveRun, AdaptiveSettings, Register,
};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::gradient::least_squares;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::adapt::{
    level_histogram, max_face_level_jump, region_indicator, AmrMesh, Leaf, Mark, RefineTransfer, RegionShape,
};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::{Mesh, PatchKind};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

const AXES: [bool; 3] = [true, true, false];
const MAX_ITER: usize = 20000;
const TOL: f64 = 1e-7;

// ------------------------------------------------------------------------------------------------ meshes and set-ups

fn slab(coords: [Vec<f64>; 2], max_level: u8, names: [(&str, PatchKind); 4]) -> AmrMesh {
    let [x, y] = coords;
    let p = |n: (&str, PatchKind)| (n.0.to_string(), n.1);
    AmrMesh::new(
        [x, y, vec![0.0, 0.1]],
        AXES,
        max_level,
        [
            p(names[0].clone()),
            p(names[1].clone()),
            p(names[2].clone()),
            p(names[3].clone()),
            ("frontAndBack".into(), PatchKind::Empty),
            ("frontAndBack".into(), PatchKind::Empty),
        ],
    )
    .unwrap()
}

fn uniform_coords(n: usize) -> Vec<f64> {
    axis_coords(n, 1.0, Grading::Uniform).unwrap()
}

fn base(n: usize, max_level: u8) -> AmrMesh {
    slab(
        [uniform_coords(n), uniform_coords(n)],
        max_level,
        [
            ("walls", PatchKind::Wall),
            ("walls", PatchKind::Wall),
            ("walls", PatchKind::Wall),
            ("lid", PatchKind::Wall),
        ],
    )
}

fn sides(coords: [Vec<f64>; 2], max_level: u8) -> AmrMesh {
    slab(
        coords,
        max_level,
        [
            ("xmin", PatchKind::Wall),
            ("xmax", PatchKind::Wall),
            ("ymin", PatchKind::Wall),
            ("ymax", PatchKind::Wall),
        ],
    )
}

fn refine_cells(amr: &AmrMesh, cells: &[usize]) -> AmrMesh {
    let mut marks = vec![Mark::Keep; amr.n_cells()];
    for &c in cells {
        marks[c] = Mark::Refine;
    }
    amr.adapt(&marks, false).unwrap().0
}

fn geometry(amr: &AmrMesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(amr.to_mesh().unwrap())).unwrap())
}

fn cavity_settings() -> SimpleSettings {
    let mut s = SimpleSettings::new(0.01, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.non_orth_correctors = 1;
    s.simplec_non_orth_compensation = true;
    s
}

fn cavity_setup(geom: &Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)> {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    u.set_bc(geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))?;
    u.set_bc(geom, "walls", Box::new(FixedValue::vector([0.0; 3])))?;
    let p = ScalarField::uniform("p", geom, 0.0);
    Ok((u, p, cavity_settings()))
}

fn adaptive_settings(cycles: usize, max_iter: usize) -> AdaptiveSettings {
    AdaptiveSettings {
        cycles,
        refine_fraction: 0.4,
        balance: true,
        transfer: RefineTransfer::Linear,
        scale_by_length: true,
        max_iter,
        tol: TOL,
    }
}

fn lin_u(x: Vec3) -> Vec3 {
    [1.0 + 2.0 * x[0] - 3.0 * x[1], 0.5 - x[0] + 4.0 * x[1], 0.0]
}

/// A solver whose `U` is `f` in the cells and on every non-empty boundary face (no iteration is made).
fn solver_with(amr: &AmrMesh, f: fn(Vec3) -> Vec3) -> SimpleSolver {
    let geom = geometry(amr);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let patches: Vec<(String, Vec<Vec3>)> = geom
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty)
        .map(|p| {
            (
                p.name.clone(),
                p.faces().map(|fc| f(geom.mesh.face_centres()[fc])).collect(),
            )
        })
        .collect();
    for (name, vals) in patches {
        u.set_bc(&geom, &name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    u.values = geom.mesh.cell_centres().iter().map(|&c| f(c)).collect();
    let p = ScalarField::uniform("p", &geom, 0.0);
    SimpleSolver::new(geom, u, p, cavity_settings())
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

fn refine_count(m: &[Mark]) -> usize {
    m.iter().filter(|x| **x == Mark::Refine).count()
}

// ------------------------------------------------------------------------------------------------ K0, K0b: the length scaling

#[test]
fn k0_velocity_gradient_indicator_is_scaled_by_the_cell_length() {
    let amr = refine_cells(&base(8, 2), &[0]);
    assert_eq!(amr.n_cells(), 67);
    let s = solver_with(&amr, lin_u);
    let g = 30f64.sqrt();
    let on = velocity_gradient_indicator(&s, AXES, true).unwrap();
    let off = velocity_gradient_indicator(&s, AXES, false).unwrap();
    let (mut e0, mut e1) = (vec![], vec![]);
    for (c, l) in amr.leaves().iter().enumerate() {
        let h = if l.level == 0 { 1.0 / 8.0 } else { 1.0 / 16.0 };
        assert!(rel(on[c], g * h) <= 1e-9, "cell {c}: {} against {}", on[c], g * h);
        assert!(rel(off[c], g) <= 1e-9, "cell {c}: {} against {g}", off[c]);
        if l.level == 0 {
            e0.push(on[c]);
        } else {
            e1.push(on[c]);
        }
    }
    for a in &e0 {
        for b in &e1 {
            assert!(rel(a / b, 2.0) <= 1e-9, "ratio {}", a / b);
        }
    }
}

fn first_call_setup(
    f: fn(Vec3) -> Vec3,
) -> impl FnMut(&Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)> {
    let mut calls = 0;
    move |geom: &Arc<FvGeometry>| {
        let (mut u, p, s) = cavity_setup(geom)?;
        if calls == 0 {
            u.values = geom.mesh.cell_centres().iter().map(|&c| f(c)).collect();
        }
        calls += 1;
        Ok((u, p, s))
    }
}

fn quad_u(x: Vec3) -> Vec3 {
    [x[0] * x[0] + x[1] * x[1], x[0] * x[1], 0.0]
}

#[test]
fn k0b_the_scaling_reaches_the_loop() {
    let on = adaptive_settings(2, 0);
    let mut off = on;
    off.scale_by_length = false;
    let a = run_adaptive_simple(base(12, 2), first_call_setup(quad_u), &on).unwrap();
    let b = run_adaptive_simple_with(base(12, 2), first_call_setup(quad_u), &on, |s: &SimpleSolver| {
        velocity_gradient_indicator(s, AXES, true)
    })
    .unwrap();
    let c = run_adaptive_simple(base(12, 2), first_call_setup(quad_u), &off).unwrap();
    let leaves = |r: &AdaptiveRun| -> Vec<Leaf> { r.amr.leaves().to_vec() };
    println!(
        "K0b: cells scaled {}, explicit {}, unscaled {}",
        a.amr.n_cells(),
        b.amr.n_cells(),
        c.amr.n_cells()
    );
    assert_eq!(leaves(&a), leaves(&b));
    assert_ne!(leaves(&a), leaves(&c));
}

// ------------------------------------------------------------------------------------------------ K5: defaults bit for bit

#[test]
fn k5_marked_loop_with_fixed_fraction_is_run_adaptive_simple_bit_for_bit() {
    let set = adaptive_settings(2, 30);
    let a = run_adaptive_simple(base(12, 2), cavity_setup, &set).unwrap();
    let b = run_adaptive_simple_marked(
        base(12, 2),
        cavity_setup,
        &set,
        |s: &SimpleSolver, amr: &AmrMesh, _k: usize| {
            let ind = velocity_gradient_indicator(s, AXES, true)?;
            fixed_fraction_marks(&ind, &amr.levels(), amr.max_level(), 0.4)
        },
    )
    .unwrap();
    assert_eq!(a.amr.leaves(), b.amr.leaves());
    assert_eq!(a.cycles, b.cycles);
    assert!(a.solver.u.values == b.solver.u.values);
    assert!(a.solver.p.values == b.solver.p.values);
    assert!(a.solver.phi == b.solver.phi);
    assert!(a.amr.n_cells() > 144);
}

// ------------------------------------------------------------------------------------------------ K1: target count

#[test]
fn k1_target_count_marks_closed_form() {
    let ind: Vec<f64> = (0..100).map(|i| ((37 * i) % 100) as f64).collect();
    let lv0 = [0u8; 100];
    let largest = |m: &[Mark], k: usize| {
        for i in 0..100 {
            assert_eq!(m[i] == Mark::Refine, ind[i] >= (100 - k) as f64, "cell {i}, k {k}");
        }
    };
    largest(&target_count_marks(&ind, &lv0, 1, 160, 4).unwrap(), 20);
    largest(&target_count_marks(&ind, &lv0, 1, 161, 4).unwrap(), 21);
    for t in [100, 50] {
        let m = target_count_marks(&ind, &lv0, 1, t, 4).unwrap();
        assert!(m.iter().all(|x| *x == Mark::Keep), "target {t}");
    }
    let mut lv = [0u8; 100];
    lv[..10].iter_mut().for_each(|l| *l = 1);
    let m = target_count_marks(&ind, &lv, 1, 400, 4).unwrap();
    for i in 0..100 {
        assert_eq!(m[i] == Mark::Refine, i >= 10, "cell {i}");
    }
    // the adapted count without balance
    let amr = base(10, 1);
    for (t, want) in [(160, 160), (161, 163), (162, 163), (163, 163)] {
        let m = target_count_marks(&ind, &amr.levels(), 1, t, children_per_split(AXES)).unwrap();
        let (next, _, _) = amr.adapt(&m, false).unwrap();
        assert_eq!(next.n_cells(), want, "target {t}");
    }
    // 3-D
    let c = uniform_coords(4);
    let w = |s: &str| (s.to_string(), PatchKind::Wall);
    let cube = AmrMesh::new(
        [c.clone(), c.clone(), c],
        [true; 3],
        1,
        [w("a"), w("a"), w("a"), w("a"), w("a"), w("a")],
    )
    .unwrap();
    assert_eq!(children_per_split([true; 3]), 8);
    assert_eq!(children_per_split(AXES), 4);
    let ind3: Vec<f64> = (0..64).map(|i| ((37 * i) % 64) as f64).collect();
    let m = target_count_marks(&ind3, &cube.levels(), 1, 100, 8).unwrap();
    assert_eq!(refine_count(&m), 6);
    assert_eq!(cube.adapt(&m, false).unwrap().0.n_cells(), 106);
    // schedule
    assert_eq!(
        (0..=2).map(|k| target_schedule(576, 2304, 2, k)).collect::<Vec<_>>(),
        vec![576, 1152, 2304]
    );
    for k in 1..5 {
        let want = (256.0 * 15f64.powf(k as f64 / 5.0)).round() as usize;
        assert_eq!(target_schedule(256, 3840, 5, k), want, "k {k}");
    }
    assert_eq!(target_schedule(256, 3840, 5, 5), 3840);
    assert_eq!(target_schedule(256, 3840, 5, 0), 256);
    // located errors
    let e = target_count_marks(&ind, &lv0, 1, 200, 1).unwrap_err().to_string();
    assert!(e.contains("children"), "{e}");
    let e = target_count_marks(&[1.0, f64::NAN], &[0, 0], 1, 5, 4)
        .unwrap_err()
        .to_string();
    assert!(e.contains("indicator"), "{e}");
    let e = target_count_marks(&[1.0, 2.0], &[0], 1, 5, 4).unwrap_err().to_string();
    assert!(e.contains("levels"), "{e}");
}

// ------------------------------------------------------------------------------------------------ K2: edge-length weighting

fn graded() -> (AmrMesh, Vec<f64>, Vec<f64>) {
    let x = vec![0.0, 0.1, 0.3, 0.6, 1.0];
    let y = vec![0.0, 0.2, 0.25, 0.5, 1.0];
    let amr = refine_cells(&sides([x.clone(), y.clone()], 1), &[5]);
    (amr, x, y)
}

/// Expected edge lengths of every leaf of a slab lattice from its base nodes.
fn expected_extents(amr: &AmrMesh, x: &[f64], y: &[f64]) -> Vec<[f64; 2]> {
    amr.leaves()
        .iter()
        .map(|l| {
            let s = 1i64 << l.level;
            let (i, j) = ((l.ijk[0] / s) as usize, (l.ijk[1] / s) as usize);
            let f = 1.0 / s as f64;
            [(x[i + 1] - x[i]) * f, (y[j + 1] - y[j]) * f]
        })
        .collect()
}

#[test]
fn k2_edge_length_weighted_indicator_closed_form() {
    let (amr, x, y) = graded();
    assert_eq!(amr.n_cells(), 19);
    let mesh: Mesh = amr.to_mesh().unwrap();
    let want = expected_extents(&amr, &x, &y);
    let ext = cell_extents(&mesh);
    for c in 0..mesh.n_cells() {
        assert!((ext[c][0] - want[c][0]).abs() <= 1e-14, "cell {c} x");
        assert!((ext[c][1] - want[c][1]).abs() <= 1e-14, "cell {c} y");
        assert!((ext[c][2] - 0.1).abs() <= 1e-14, "cell {c} z");
    }
    let closed = |c: usize, r: f64| {
        let [hx, hy] = want[c];
        (5.0 * hx * hx + 25.0 * hy * hy).sqrt() * (hx * hy).powf((r - 1.0) / 2.0)
    };
    let n = mesh.n_cells();
    let grads = vec![vec![[2.0, -3.0, 0.0]; n], vec![[-1.0, 4.0, 0.0]; n], vec![[0.0; 3]; n]];
    for r in [0.0, 1.0, 2.0, 3.5] {
        let eta = edge_length_weighted_indicator(&mesh, &grads, AXES, r).unwrap();
        for c in 0..n {
            assert!(
                rel(eta[c], closed(c, r)) <= 1e-13,
                "r {r} cell {c}: {} against {}",
                eta[c],
                closed(c, r)
            );
        }
    }
    let s = solver_with(&amr, lin_u);
    for r in [0.0, 1.0, 2.0, 3.5] {
        let eta = velocity_edge_length_indicator(&s, AXES, r).unwrap();
        let worst = (0..n).map(|c| rel(eta[c], closed(c, r))).fold(0.0, f64::max);
        println!("K2 velocity_edge_length_indicator r {r}: worst relative {worst:.2e}");
        assert!(worst <= 1e-9, "r {r}: {worst}");
    }
    // located errors
    let e = edge_length_weighted_indicator(&mesh, &grads, AXES, -1.0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("exponent"), "{e}");
    let e = edge_length_weighted_indicator(&mesh, &grads, [false; 3], 1.0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("axis"), "{e}");
    let short = vec![vec![[0.0; 3]; n - 1]];
    let e = edge_length_weighted_indicator(&mesh, &short, AXES, 1.0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("grads"), "{e}");
}

#[test]
fn k2_edge_length_weighted_marks_closed_form() {
    let amr = refine_cells(&base(8, 2), &[0]);
    assert_eq!(amr.n_cells(), 67);
    let s = solver_with(&amr, lin_u);
    let eta = velocity_edge_length_indicator(&s, AXES, 1.0).unwrap();
    let lv = amr.levels();
    let e0 = (0..67).find(|&c| lv[c] == 0).map(|c| eta[c]).unwrap();
    for c in 0..67 {
        let want = if lv[c] == 0 { e0 } else { e0 / 2.0 };
        assert!(
            rel(eta[c], want) <= 1e-9,
            "cell {c} level {}: {} against {want}",
            lv[c],
            eta[c]
        );
    }
    let m = target_count_marks(&eta, &lv, 2, 67 + 48, 4).unwrap();
    assert_eq!(refine_count(&m), 16);
    for c in 0..67 {
        if m[c] == Mark::Refine {
            assert_eq!(lv[c], 0, "cell {c}");
        }
    }
    let uni = base(8, 2);
    let s = solver_with(&uni, lin_u);
    let a = velocity_edge_length_indicator(&s, AXES, 1.0).unwrap();
    let b = velocity_gradient_indicator(&s, AXES, true).unwrap();
    for c in 0..64 {
        assert!(rel(a[c], b[c]) <= 1e-12, "cell {c}: {} against {}", a[c], b[c]);
    }
}

// ------------------------------------------------------------------------------------------------ K3, K3b: registers

#[test]
fn k3_register_closed_form() {
    let amr = sides([uniform_coords(10), uniform_coords(10)], 1);
    let mesh = amr.to_mesh().unwrap();
    let cc = mesh.cell_centres().to_vec();
    let ij: Vec<(i64, i64)> = cc
        .iter()
        .map(|c| ((c[0] * 10.0).floor() as i64, (c[1] * 10.0).floor() as i64))
        .collect();
    let set = |f: &dyn Fn(i64, i64) -> bool| -> Vec<bool> { ij.iter().map(|&(i, j)| f(i, j)).collect() };
    let phi: Vec<f64> = cc.iter().map(|c| c[0] + 10.0 * c[1]).collect();
    let range = Register::in_range(&phi, 2.0, 3.0).unwrap();
    let want_range = set(&|i, j| (i >= 5 && j == 1) || (i <= 4 && j == 2));
    assert_eq!(range.mask(), &want_range[..]);
    assert_eq!(range.count(), 10);
    let bx = region_indicator(
        &mesh,
        &RegionShape::Box {
            lo: [-1.0, -1.0, -1.0],
            hi: [0.5, 2.0, 2.0],
        },
    )
    .unwrap();
    let left = Register::from_indicator(&bx).unwrap();
    assert_eq!(left.mask(), &set(&|i, _| i <= 4)[..]);
    assert_eq!(range.and(&left).unwrap().mask(), &set(&|i, j| i <= 4 && j == 2)[..]);
    assert_eq!(
        range.or(&left).unwrap().mask(),
        &set(&|i, j| i <= 4 || (i >= 5 && j == 1))[..]
    );
    assert_eq!(range.complement().count(), 90);
    assert_eq!(
        range.complement().mask(),
        &set(&|i, j| !((i >= 5 && j == 1) || (i <= 4 && j == 2)))[..]
    );
    let one = Register::from_mask(set(&|i, j| i == 5 && j == 5));
    let g1 = one.grow(&mesh, 1).unwrap();
    assert_eq!(g1.mask(), &set(&|i, j| (i - 5).abs() + (j - 5).abs() <= 1)[..]);
    assert_eq!(g1.count(), 5);
    let g2 = one.grow(&mesh, 2).unwrap();
    assert_eq!(g2.mask(), &set(&|i, j| (i - 5).abs() + (j - 5).abs() <= 2)[..]);
    assert_eq!(g2.count(), 13);
    assert_eq!(one.grow(&mesh, 0).unwrap(), one);
    // marks
    let levels: Vec<u8> = ij.iter().map(|&(i, _)| if i == 9 { 1 } else { 0 }).collect();
    let reg = range.or(&left).unwrap();
    let m = register_marks(&reg, &levels, 1).unwrap();
    for c in 0..100 {
        let want = reg.contains(c) && levels[c] < 1;
        assert_eq!(m[c] == Mark::Refine, want, "cell {c}");
        assert!(m[c] == Mark::Refine || m[c] == Mark::Keep);
    }
    let marks: Vec<Mark> = (0..100)
        .map(|c| match c % 3 {
            0 => Mark::Refine,
            1 => Mark::Coarsen,
            _ => Mark::Keep,
        })
        .collect();
    let r = restrict_marks(&marks, &left).unwrap();
    for c in 0..100 {
        assert_eq!(r[c], if left.contains(c) { marks[c] } else { Mark::Keep }, "cell {c}");
    }
    assert_eq!(
        Register::levels_in(&[0, 1, 2, 3], 1, 2).mask(),
        &[false, true, true, false][..]
    );
    // located errors
    let short = Register::from_mask(vec![true; 3]);
    for e in [
        range.and(&short).unwrap_err().to_string(),
        range.or(&short).unwrap_err().to_string(),
        restrict_marks(&marks, &short).unwrap_err().to_string(),
        register_marks(&short, &levels, 1).unwrap_err().to_string(),
        short.grow(&mesh, 1).unwrap_err().to_string(),
    ] {
        assert!(e.contains("register"), "{e}");
    }
    let e = Register::in_range(&phi, 3.0, 2.0).unwrap_err().to_string();
    assert!(e.contains("lo"), "{e}");
    let e = Register::in_range(&[1.0, f64::NAN], 0.0, 2.0).unwrap_err().to_string();
    assert!(e.contains("field"), "{e}");
    let e = Register::from_indicator(&[f64::NAN]).unwrap_err().to_string();
    assert!(e.contains("indicator"), "{e}");
}

#[test]
fn k3b_register_confines_the_loop() {
    let mut set = adaptive_settings(2, 20);
    set.balance = false;
    let run = run_adaptive_simple_marked(
        base(12, 2),
        cavity_setup,
        &set,
        |s: &SimpleSolver, amr: &AmrMesh, _k: usize| {
            let ind = velocity_gradient_indicator(s, AXES, true)?;
            let ff = fixed_fraction_marks(&ind, &amr.levels(), amr.max_level(), 0.4)?;
            let top = region_indicator(
                &s.geom.mesh,
                &RegionShape::Box {
                    lo: [-1.0, 0.5, -1.0],
                    hi: [2.0, 2.0, 2.0],
                },
            )?;
            restrict_marks(&ff, &Register::from_indicator(&top)?)
        },
    )
    .unwrap();
    let n = run.amr.n_cells();
    println!("K3b: {n} cells, levels {:?}", level_histogram(&run.amr.levels()));
    assert!(n > 144, "{n} cells");
    let cc = run.solver.geom.mesh.cell_centres();
    for (c, l) in run.amr.leaves().iter().enumerate() {
        if l.level > 0 {
            assert!(cc[c][1] > 0.5, "refined cell {c} at y = {}", cc[c][1]);
        }
    }
}

// ------------------------------------------------------------------------------------------------ cavity target-count runs

struct Sampler {
    lo: Vec<Vec3>,
    hi: Vec<Vec3>,
    cc: Vec<Vec3>,
    u: Vec<Vec3>,
    g: [Vec<Vec3>; 3],
}

impl Sampler {
    fn new(s: &SimpleSolver) -> Sampler {
        let geom = &s.geom;
        let m = &geom.mesh;
        let pts = m.points();
        let mut lo = Vec::with_capacity(m.n_cells());
        let mut hi = Vec::with_capacity(m.n_cells());
        for c in 0..m.n_cells() {
            let mut l = [f64::INFINITY; 3];
            let mut h = [f64::NEG_INFINITY; 3];
            for &f in m.cell_faces(c) {
                for &p in m.face_points(f) {
                    for a in 0..3 {
                        l[a] = l[a].min(pts[p][a]);
                        h[a] = h[a].max(pts[p][a]);
                    }
                }
            }
            lo.push(l);
            hi.push(h);
        }
        let bu = s.u.boundary_values(geom);
        let g = [0, 1, 2].map(|k| {
            let cells: Vec<f64> = s.u.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = bu.iter().map(|v| v[k]).collect();
            least_squares(geom, &cells, &bv)
        });
        Sampler {
            lo,
            hi,
            cc: m.cell_centres().to_vec(),
            u: s.u.values.clone(),
            g,
        }
    }

    fn at(&self, x: Vec3) -> Vec3 {
        let mut sum = [0.0; 3];
        let mut n = 0.0;
        for c in 0..self.cc.len() {
            if (0..2).all(|a| x[a] >= self.lo[c][a] && x[a] <= self.hi[c][a]) {
                for k in 0..3 {
                    let d = [x[0] - self.cc[c][0], x[1] - self.cc[c][1], 0.0];
                    sum[k] += self.u[c][k] + self.g[k][c][0] * d[0] + self.g[k][c][1] * d[1];
                }
                n += 1.0;
            }
        }
        assert!(n > 0.0, "no cell contains {x:?}");
        [sum[0] / n, sum[1] / n, sum[2] / n]
    }

    fn dense(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(198);
        for j in 1..100 {
            out.push(self.at([0.5, j as f64 / 100.0, 0.05])[0]);
        }
        for j in 1..100 {
            out.push(self.at([j as f64 / 100.0, 0.5, 0.05])[1]);
        }
        out
    }
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

struct CavityUniform {
    cells: usize,
    converged: bool,
    dense: Vec<f64>,
}

fn cavity_uniform(n: usize) -> CavityUniform {
    let t = Instant::now();
    let geom = geometry(&base(n, 0));
    let (u, p, s) = cavity_setup(&geom).unwrap();
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(MAX_ITER, TOL);
    println!(
        "F-343 cavity uniform {n} x {n}: {} cells, {} iterations, converged {}, {:.1} s",
        solver.geom.n_cells(),
        sum.iterations,
        sum.converged,
        t.elapsed().as_secs_f64()
    );
    CavityUniform {
        cells: solver.geom.n_cells(),
        converged: sum.converged,
        dense: Sampler::new(&solver).dense(),
    }
}

fn print_cycles(name: &str, run: &AdaptiveRun) {
    for (k, r) in run.cycles.iter().enumerate() {
        println!(
            "F-343 {name} cycle {k}: {} cells, {} iterations, converged {}, refined {}, ignored at max level {}",
            r.cells, r.iterations, r.converged, r.refined, r.ignored_at_max_level
        );
    }
    println!("F-343 {name}: levels {:?}", level_histogram(&run.amr.levels()));
}

struct CavityTarget {
    run: AdaptiveRun,
    dense: Vec<f64>,
}

fn cavity_target(name: &str, target: usize) -> CavityTarget {
    let t = Instant::now();
    let run = run_adaptive_simple_target(
        base(24, 2),
        cavity_setup,
        &adaptive_settings(2, MAX_ITER),
        target,
        |s| velocity_gradient_indicator(s, AXES, true),
    )
    .unwrap();
    print_cycles(name, &run);
    println!("F-343 {name}: {:.1} s", t.elapsed().as_secs_f64());
    let dense = Sampler::new(&run.solver).dense();
    CavityTarget { run, dense }
}

struct CavityRuns {
    f: CavityUniform,
    u48: CavityUniform,
    t1: CavityTarget,
    t2: CavityTarget,
}

fn cavity_runs() -> &'static CavityRuns {
    static RUNS: OnceLock<CavityRuns> = OnceLock::new();
    RUNS.get_or_init(|| {
        let r = CavityRuns {
            f: cavity_uniform(96),
            u48: cavity_uniform(48),
            t1: cavity_target("T1", 2304),
            t2: cavity_target("T2", 4608),
        };
        println!(
            "F-343 cavity results: N(T1) {} N(T2) {} N_F {}; D(T1) {:.5} D(T2) {:.5} D(U48) {:.5}",
            r.t1.run.amr.n_cells(),
            r.t2.run.amr.n_cells(),
            r.f.cells,
            max_diff(&r.t1.dense, &r.f.dense),
            max_diff(&r.t2.dense, &r.f.dense),
            max_diff(&r.u48.dense, &r.f.dense)
        );
        r
    })
}

#[test]
#[ignore = "heavy (ADR 0247 cavity): run in release with --include-ignored"]
fn t1_target_count_lands_within_10_percent_on_the_cavity() {
    let n = cavity_runs().t1.run.amr.n_cells() as f64;
    assert!((n - 2304.0).abs() <= 230.4, "N(T1) = {n}");
}

#[test]
#[ignore = "heavy (ADR 0247 cavity): run in release with --include-ignored"]
fn t2_target_count_lands_within_10_percent_at_half_the_fine_count() {
    let n = cavity_runs().t2.run.amr.n_cells() as f64;
    assert!((n - 4608.0).abs() <= 460.8, "N(T2) = {n}");
}

fn assert_sound(name: &str, run: &AdaptiveRun) {
    for (k, rec) in run.cycles.iter().enumerate() {
        assert!(rec.converged, "{name} cycle {k} not converged");
    }
    let mesh = run.amr.to_mesh().unwrap();
    mesh.validate().unwrap();
    assert!(max_face_level_jump(&run.amr, &mesh) <= 1, "{name}: level jump");
    assert!(run.cycles.windows(2).all(|w| w[1].cells > w[0].cells), "{name}: cells");
}

#[test]
#[ignore = "heavy (ADR 0247 cavity): run in release with --include-ignored"]
fn t3_target_count_runs_are_sound() {
    let r = cavity_runs();
    assert!(r.f.converged && r.u48.converged, "uniform not converged");
    assert_sound("T1", &r.t1.run);
    assert_sound("T2", &r.t2.run);
}

// ------------------------------------------------------------------------------------------------ the suction layer

const R_LAYER: f64 = 80.0;

/// The exact solution `u*(y)` of Couette flow with suction (ADR 0247 D7), `U = V = H = 1`.
fn u_star(y: f64) -> f64 {
    ((R_LAYER * (y - 1.0)).exp() - (-R_LAYER).exp()) / (1.0 - (-R_LAYER).exp())
}

fn layer_base(n: usize, max_level: u8) -> AmrMesh {
    slab(
        [uniform_coords(n), uniform_coords(n)],
        max_level,
        [
            ("inlet", PatchKind::Inlet),
            ("outlet", PatchKind::Outlet),
            ("bottom", PatchKind::Inlet),
            ("top", PatchKind::Wall),
        ],
    )
}

fn layer_setup(geom: &Arc<FvGeometry>) -> Result<(VectorField, ScalarField, SimpleSettings)> {
    let mut u = VectorField::uniform("U", geom, [0.0, 1.0, 0.0]);
    let inlet: Vec<Vec3> = geom
        .mesh
        .patch("inlet")
        .expect("inlet")
        .faces()
        .map(|f| [u_star(geom.mesh.face_centres()[f][1]), 1.0, 0.0])
        .collect();
    u.set_bc(geom, "inlet", Box::new(FixedValue(Profile::PerFace(inlet))))?;
    u.set_bc(geom, "bottom", Box::new(FixedValue::vector([0.0, 1.0, 0.0])))?;
    u.set_bc(geom, "top", Box::new(FixedValue::vector([1.0, 1.0, 0.0])))?;
    u.set_bc(geom, "outlet", Box::new(ZeroGradient))?;
    let mut p = ScalarField::uniform("p", geom, 0.0);
    p.set_bc(geom, "outlet", Box::new(FixedValue::scalar(0.0)))?;
    let mut s = SimpleSettings::new(1.0 / R_LAYER, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.non_orth_correctors = 1;
    s.simplec_non_orth_compensation = true;
    Ok((u, p, s))
}

fn layer_error(s: &SimpleSolver) -> f64 {
    let cc = s.geom.mesh.cell_centres();
    s.u.values
        .iter()
        .zip(cc)
        .map(|(u, c)| (u[0] - u_star(c[1])).abs().max((u[1] - 1.0).abs()))
        .fold(0.0, f64::max)
}

struct LayerUniform {
    cells: usize,
    converged: bool,
    e: f64,
}

fn layer_uniform(n: usize) -> LayerUniform {
    let t = Instant::now();
    let geom = geometry(&layer_base(n, 0));
    let (u, p, s) = layer_setup(&geom).unwrap();
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(MAX_ITER, TOL);
    let r = LayerUniform {
        cells: solver.geom.n_cells(),
        converged: sum.converged,
        e: layer_error(&solver),
    };
    println!(
        "F-343 layer uniform {n} x {n}: {} cells, {} iterations, converged {}, E {:.5e}, {:.1} s",
        r.cells,
        sum.iterations,
        r.converged,
        r.e,
        t.elapsed().as_secs_f64()
    );
    r
}

struct LayerAdapted {
    run: AdaptiveRun,
    e: f64,
}

fn layer_adapted(name: &str, r: f64, inverted: bool) -> LayerAdapted {
    let t = Instant::now();
    let set = adaptive_settings(5, MAX_ITER);
    let run = run_adaptive_simple_target(layer_base(16, 4), layer_setup, &set, 3840, move |s: &SimpleSolver| {
        let eta = velocity_edge_length_indicator(s, AXES, r)?;
        if inverted {
            let top = eta.iter().cloned().fold(0.0, f64::max);
            Ok(eta.iter().map(|e| top - e).collect())
        } else {
            Ok(eta)
        }
    })
    .unwrap();
    print_cycles(name, &run);
    let e = layer_error(&run.solver);
    println!(
        "F-343 {name}: {} cells, E {e:.5e}, {:.1} s",
        run.amr.n_cells(),
        t.elapsed().as_secs_f64()
    );
    LayerAdapted { run, e }
}

struct LayerRuns {
    u32: LayerUniform,
    u64: LayerUniform,
    u90: LayerUniform,
    f: LayerUniform,
    a: LayerAdapted,
    ainv: LayerAdapted,
    a1: LayerAdapted,
}

fn layer_runs() -> &'static LayerRuns {
    static RUNS: OnceLock<LayerRuns> = OnceLock::new();
    RUNS.get_or_init(|| {
        let r = LayerRuns {
            u32: layer_uniform(32),
            u64: layer_uniform(64),
            u90: layer_uniform(90),
            f: layer_uniform(128),
            a: layer_adapted("A", 2.0, false),
            ainv: layer_adapted("Ainv", 2.0, true),
            a1: layer_adapted("A1", 1.0, false),
        };
        let order = |a: &LayerUniform, b: &LayerUniform| {
            (a.e / b.e).ln() / ((b.cells as f64).sqrt() / (a.cells as f64).sqrt()).ln()
        };
        println!(
            "F-343 layer results: E(U32) {:.5e} E(U64) {:.5e} E(U90) {:.5e} E(F) {:.5e} E(A) {:.5e} E(Ainv) {:.5e} E(A1) {:.5e}; \
             N_A {} N_Ainv {} N_A1 {} N_F {}; E(A)/E(F) {:.4} E(U90)/E(F) {:.4} E(A1)/E(A) {:.4}; orders 32-64 {:.3} 64-90 {:.3} \
             90-128 {:.3}",
            r.u32.e,
            r.u64.e,
            r.u90.e,
            r.f.e,
            r.a.e,
            r.ainv.e,
            r.a1.e,
            r.a.run.amr.n_cells(),
            r.ainv.run.amr.n_cells(),
            r.a1.run.amr.n_cells(),
            r.f.cells,
            r.a.e / r.f.e,
            r.u90.e / r.f.e,
            r.a1.e / r.a.e,
            order(&r.u32, &r.u64),
            order(&r.u64, &r.u90),
            order(&r.u90, &r.f)
        );
        r
    })
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn l1_adapted_reaches_the_fine_error() {
    let r = layer_runs();
    assert!(r.a.e <= r.f.e, "E(A) {} > E(F) {}", r.a.e, r.f.e);
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn l2_adapted_has_at_most_half_the_fine_cells() {
    let r = layer_runs();
    assert!(r.a.run.amr.n_cells() * 2 <= r.f.cells, "N_A {}", r.a.run.amr.n_cells());
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn l3_layer_target_count_lands_within_10_percent() {
    let n = layer_runs().a.run.amr.n_cells() as f64;
    assert!((n - 3840.0).abs() <= 384.0, "N_A = {n}");
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn l4_layer_solves_converge_and_the_adapted_mesh_is_sound() {
    let r = layer_runs();
    assert!(r.f.converged, "F not converged");
    assert!(r.u90.converged, "U90 not converged");
    assert_sound("A", &r.a.run);
    assert!(level_histogram(&r.a.run.amr.levels()).contains_key(&4), "no level 4");
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn l5_adapted_has_at_most_half_the_coarsest_uniform_mesh_reaching_the_fine_error() {
    let r = layer_runs();
    assert!(r.a.e <= r.f.e, "E(A) {} > E(F) {}", r.a.e, r.f.e);
    assert!(r.a.run.amr.n_cells() <= 4140, "N_A {}", r.a.run.amr.n_cells());
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn ncl1_uniform_with_half_the_cells_misses_the_fine_error() {
    let r = layer_runs();
    assert!(r.u90.cells * 2 <= r.f.cells);
    assert!(r.u90.e > r.f.e, "E(U90) {} <= E(F) {}", r.u90.e, r.f.e);
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn ncl2_inverted_indicator_misses_the_fine_error() {
    let r = layer_runs();
    assert!(r.ainv.e > r.f.e, "E(Ainv) {} <= E(F) {}", r.ainv.e, r.f.e);
}

#[test]
#[ignore = "heavy (ADR 0247 layer): run in release with --include-ignored"]
fn ncl3_uniform_sequence_is_monotone() {
    let r = layer_runs();
    assert!(
        r.u32.e > r.u64.e && r.u64.e > r.u90.e && r.u90.e > r.f.e,
        "{} {} {} {}",
        r.u32.e,
        r.u64.e,
        r.u90.e,
        r.f.e
    );
}
