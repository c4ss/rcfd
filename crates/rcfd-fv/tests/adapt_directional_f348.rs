//! F-348 (ADR 0267): the directional criterion and loop of `rcfd_fv::adaptive` on the lattice with per-axis levels, and the
//! directional run on the ADR 0247 suction layer. Limits registered in ADR 0267 before any of these tests was run.
//!
//! The C checks are ordinary tests. The layer checks (S0, H1 to H4, NC-H1) share one computation and are ignored as heavy: run
//! once in release with `--include-ignored`. The set-up helpers are copies of those of `adapt_criteria_f343.rs` (S0 checks the
//! copy against ADR 0247's numbers).

use rcfd_core::vec3::Vec3;
use rcfd_core::Result;
use rcfd_fv::adaptive::{
    axis_variations, directional_axes, directional_target_marks, run_adaptive_simple_directional,
    run_adaptive_simple_directional_target, run_adaptive_simple_target, target_count_marks, target_schedule,
    velocity_axis_variations, velocity_edge_length_indicator, velocity_gradient_indicator, AdaptiveRun,
    AdaptiveSettings, CycleRecord, DirectionalRun,
};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::adapt::aniso::{max_axis_level_jump, AnisoMesh, AxisMark};
use rcfd_mesh::adapt::{AmrMesh, Mark, RefineTransfer};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

const AXES: [bool; 3] = [true, true, false];
const MAX_ITER: usize = 20000;
const TOL: f64 = 1e-7;

// ------------------------------------------------------------------------------------------------ meshes and set-ups

type Names = [(&'static str, PatchKind); 4];

fn patches(names: Names) -> [(String, PatchKind); 6] {
    let [a, b, c, d] = names;
    let p = |n: (&str, PatchKind)| (n.0.to_string(), n.1);
    [
        p(a),
        p(b),
        p(c),
        p(d),
        ("frontAndBack".into(), PatchKind::Empty),
        ("frontAndBack".into(), PatchKind::Empty),
    ]
}

fn iso_slab(coords: [Vec<f64>; 2], max_level: u8, names: Names) -> AmrMesh {
    let [x, y] = coords;
    AmrMesh::new([x, y, vec![0.0, 0.1]], AXES, max_level, patches(names)).unwrap()
}

fn dir_slab(coords: [Vec<f64>; 2], max_level: [u8; 2], names: Names) -> AnisoMesh {
    let [x, y] = coords;
    AnisoMesh::new(
        [x, y, vec![0.0, 0.1]],
        AXES,
        [max_level[0], max_level[1], 0],
        patches(names),
    )
    .unwrap()
}

fn uniform_coords(n: usize) -> Vec<f64> {
    axis_coords(n, 1.0, Grading::Uniform).unwrap()
}

fn cavity_names() -> Names {
    [
        ("walls", PatchKind::Wall),
        ("walls", PatchKind::Wall),
        ("walls", PatchKind::Wall),
        ("lid", PatchKind::Wall),
    ]
}

fn side_names() -> Names {
    [
        ("xmin", PatchKind::Wall),
        ("xmax", PatchKind::Wall),
        ("ymin", PatchKind::Wall),
        ("ymax", PatchKind::Wall),
    ]
}

fn layer_names() -> Names {
    [
        ("inlet", PatchKind::Inlet),
        ("outlet", PatchKind::Outlet),
        ("bottom", PatchKind::Inlet),
        ("top", PatchKind::Wall),
    ]
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

/// A solver on `mesh` whose `U` is `f` in the cells and on every non-empty boundary face (no iteration is made).
fn solver_with(mesh: Mesh, f: fn(Vec3) -> Vec3) -> SimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let bnd: Vec<(String, Vec<Vec3>)> = geom
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
    for (name, vals) in bnd {
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

fn refine_one(amr: &AnisoMesh, cell: usize, axes: [bool; 3]) -> AnisoMesh {
    let mut marks = vec![AxisMark::KEEP; amr.n_cells()];
    marks[cell] = AxisMark::refine(axes);
    amr.adapt(&marks, false).unwrap().0
}

// ------------------------------------------------------------------------------------------------ C1: per-axis variations

#[test]
fn c1_axis_variations_closed_form() {
    let x = vec![0.0, 0.1, 0.3, 0.6, 1.0];
    let y = vec![0.0, 0.2, 0.25, 0.5, 1.0];
    let base = dir_slab([x.clone(), y.clone()], [1, 1], side_names());
    let amr = refine_one(&base, 5, [true, false, false]);
    assert_eq!(amr.n_cells(), 17);
    let mesh = amr.to_mesh().unwrap();
    // the edge lengths from the lattice: node spacing over 2^level
    let h: Vec<[f64; 2]> = amr
        .leaves()
        .iter()
        .map(|l| {
            let i = (l.ijk[0] >> l.levels[0]) as usize;
            let j = (l.ijk[1] >> l.levels[1]) as usize;
            [
                (x[i + 1] - x[i]) / (1u32 << l.levels[0]) as f64,
                (y[j + 1] - y[j]) / (1u32 << l.levels[1]) as f64,
            ]
        })
        .collect();
    let n = mesh.n_cells();
    let grads = vec![
        vec![[2.0, -3.0, 0.0]; n],
        vec![[-1.0, 4.0, 0.0]; n],
        vec![[0.0, 0.0, 0.0]; n],
    ];
    let e = axis_variations(&mesh, &grads, AXES).unwrap();
    for c in 0..n {
        assert!(rel(e[c][0], 5f64.sqrt() * h[c][0]) <= 1e-13, "cell {c} E_x");
        assert!(rel(e[c][1], 5.0 * h[c][1]) <= 1e-13, "cell {c} E_y");
        assert_eq!(e[c][2], 0.0);
    }
    let solver = solver_with(mesh, lin_u);
    let ev = velocity_axis_variations(&solver, AXES).unwrap();
    let mut worst: f64 = 0.0;
    for c in 0..n {
        worst = worst
            .max(rel(ev[c][0], 5f64.sqrt() * h[c][0]))
            .max(rel(ev[c][1], 5.0 * h[c][1]));
        assert_eq!(ev[c][2], 0.0);
    }
    println!("C1: velocity_axis_variations worst relative difference {worst:.3e}");
    assert!(worst <= 1e-9, "worst {worst}");
    // the two children of cell 5 have half its E_x and its E_y
    let e0 = axis_variations(
        &base.to_mesh().unwrap(),
        &[vec![[2.0, -3.0, 0.0]; 16], vec![[-1.0, 4.0, 0.0]; 16]],
        AXES,
    )
    .unwrap();
    let kids: Vec<usize> = (0..n).filter(|&c| amr.leaves()[c].levels == [1, 0, 0]).collect();
    assert_eq!(kids.len(), 2);
    for &c in &kids {
        assert!(rel(e[c][0], 0.5 * e0[5][0]) <= 1e-13);
        assert!(rel(e[c][1], e0[5][1]) <= 1e-13);
    }
    // located errors
    let err = axis_variations(&solver.geom.mesh, &grads, [false; 3])
        .unwrap_err()
        .to_string();
    assert!(err.contains("no active axis"), "{err}");
    let err = axis_variations(&solver.geom.mesh, &[vec![[0.0; 3]; 3]], AXES)
        .unwrap_err()
        .to_string();
    assert!(err.contains("grads[0] has 3 entries"), "{err}");
}

// ------------------------------------------------------------------------------------------------ C2: the choice of axes

#[test]
fn c2_directional_axes_closed_form() {
    let pick = |v: Vec3, levels: [u8; 3], theta: f64| directional_axes(v, levels, [2, 2, 0], AXES, theta).unwrap();
    assert_eq!(pick([1.0, 0.49, 0.0], [0; 3], 0.5), [true, false, false]);
    assert_eq!(pick([1.0, 0.5, 0.0], [0; 3], 0.5), [true, true, false]);
    assert_eq!(pick([0.2, 1.0, 0.0], [0; 3], 0.5), [false, true, false]);
    assert_eq!(pick([0.0, 0.0, 0.0], [0; 3], 0.5), [true, true, false]);
    assert_eq!(pick([1.0, 0.2, 0.0], [2, 0, 0], 0.5), [false, false, false]);
    assert_eq!(pick([1.0, 0.6, 0.0], [2, 0, 0], 0.5), [false, true, false]);
    assert_eq!(pick([1.0, 0.99, 0.0], [0; 3], 1.0), [true, false, false]);
    assert_eq!(pick([0.3, 0.99, 0.0], [0; 3], 1.0), [false, true, false]);
    // a variation on the inactive axis neither selects it nor counts in the maximum
    assert_eq!(pick([1.0, 0.6, 5.0], [0; 3], 0.5), [true, true, false]);
    for theta in [0.0, 1.5, f64::NAN, -0.5] {
        let e = directional_axes([1.0, 0.5, 0.0], [0; 3], [2, 2, 0], AXES, theta)
            .unwrap_err()
            .to_string();
        assert!(e.contains("theta"), "{e}");
    }
}

// ------------------------------------------------------------------------------------------------ C3: directional target count

#[test]
fn c3_directional_target_marks_closed_form() {
    let amr = dir_slab([uniform_coords(10), uniform_coords(10)], [2, 2], side_names());
    let n = amr.n_cells();
    assert_eq!(n, 100);
    let indicator: Vec<f64> = (0..n).map(|i| ((37 * i) % 100) as f64).collect();
    let variations: Vec<Vec3> = (0..n)
        .map(|i| if i % 2 == 0 { [1.0, 0.0, 0.0] } else { [1.0, 1.0, 0.0] })
        .collect();
    let levels = amr.levels();
    let marks = |target: usize| {
        directional_target_marks(&indicator, &variations, &levels, amr.max_levels(), AXES, target, 0.5).unwrap()
    };
    let marked = |m: &[AxisMark]| -> Vec<(usize, [bool; 3])> {
        m.iter()
            .enumerate()
            .filter(|(_, k)| **k != AxisMark::KEEP)
            .map(|(i, k)| (i, k.refine))
            .collect()
    };
    let x = [true, false, false];
    let xy = [true, true, false];
    for (target, want, count) in [
        (108, vec![(8, x), (27, xy), (54, x), (81, xy)], 108),
        (109, vec![(8, x), (27, xy), (35, xy), (54, x), (81, xy)], 111),
        (107, vec![(27, xy), (54, x), (81, xy)], 107),
        (100, vec![], 100),
        (50, vec![], 100),
    ] {
        let m = marks(target);
        assert_eq!(marked(&m), want, "target {target}");
        assert!(m.iter().all(|k| k.coarsen == [false; 3]));
        let (next, _, _) = amr.adapt(&m, false).unwrap();
        assert_eq!(next.n_cells(), count, "target {target}");
    }
    let e = directional_target_marks(&indicator[..99], &variations, &levels, [2, 2, 0], AXES, 108, 0.5)
        .unwrap_err()
        .to_string();
    assert!(e.contains("indicator has 99 entries"), "{e}");
    let mut bad = indicator.clone();
    bad[3] = f64::NAN;
    let e = directional_target_marks(&bad, &variations, &levels, [2, 2, 0], AXES, 108, 0.5)
        .unwrap_err()
        .to_string();
    assert!(e.contains("indicator[3] is not finite"), "{e}");
}

// ------------------------------------------------------------------------------------------------ C4: isotropic marks

#[test]
fn c4_directional_loop_with_all_axes_marks_is_the_isotropic_loop_bit_for_bit() {
    let set = adaptive_settings(2, 30);
    let coords = || [uniform_coords(12), uniform_coords(12)];
    let a = run_adaptive_simple_target(
        iso_slab(coords(), 2, cavity_names()),
        cavity_setup,
        &set,
        600,
        |s: &SimpleSolver| velocity_gradient_indicator(s, AXES, true),
    )
    .unwrap();
    let b = run_adaptive_simple_directional(
        dir_slab(coords(), [2, 2], cavity_names()),
        cavity_setup,
        &set,
        |s: &SimpleSolver, amr: &AnisoMesh, k: usize| {
            let ind = velocity_gradient_indicator(s, AXES, true)?;
            let levels: Vec<u8> = amr.levels().iter().map(|l| l[0]).collect();
            let marks = target_count_marks(&ind, &levels, 2, target_schedule(144, 600, 2, k), 4)?;
            Ok(marks
                .iter()
                .map(|m| {
                    if *m == Mark::Refine {
                        AxisMark::refine([true; 3])
                    } else {
                        AxisMark::KEEP
                    }
                })
                .collect())
        },
    )
    .unwrap();
    assert_eq!(a.cycles, b.cycles);
    assert_eq!(a.amr.n_cells(), b.amr.n_cells());
    assert!(a.solver.geom.mesh.points() == b.solver.geom.mesh.points());
    assert!(a.solver.u.values == b.solver.u.values);
    assert!(a.solver.p.values == b.solver.p.values);
    assert!(a.solver.phi == b.solver.phi);
    assert!(a.amr.n_cells() >= 600);
    assert!(b.amr.levels().iter().all(|l| l[0] == l[1] && l[2] == 0));
}

// ------------------------------------------------------------------------------------------------ the suction layer

const R_LAYER: f64 = 80.0;

/// The exact solution `u*(y)` of Couette flow with suction (ADR 0247 D7), `U = V = H = 1`.
fn u_star(y: f64) -> f64 {
    ((R_LAYER * (y - 1.0)).exp() - (-R_LAYER).exp()) / (1.0 - (-R_LAYER).exp())
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

fn layer_iso(n: usize, max_level: u8) -> AmrMesh {
    iso_slab([uniform_coords(n), uniform_coords(n)], max_level, layer_names())
}

fn layer_dir(n: usize, max_level: [u8; 2]) -> AnisoMesh {
    dir_slab([uniform_coords(n), uniform_coords(n)], max_level, layer_names())
}

fn print_records(name: &str, cycles: &[CycleRecord]) {
    for (k, r) in cycles.iter().enumerate() {
        println!(
            "F-348 {name} cycle {k}: {} cells, {} iterations, converged {}, refined {}, ignored at max level {}",
            r.cells, r.iterations, r.converged, r.refined, r.ignored_at_max_level
        );
    }
}

fn axis_histogram(amr: &AnisoMesh, a: usize) -> BTreeMap<u8, usize> {
    let mut h = BTreeMap::new();
    for l in amr.leaves() {
        *h.entry(l.levels[a]).or_insert(0) += 1;
    }
    h
}

fn level_sums(amr: &AnisoMesh) -> [usize; 2] {
    let mut s = [0; 2];
    for l in amr.leaves() {
        s[0] += l.levels[0] as usize;
        s[1] += l.levels[1] as usize;
    }
    s
}

fn strictly_increasing(cycles: &[CycleRecord]) -> bool {
    cycles.windows(2).all(|w| w[1].cells > w[0].cells)
}

// ------------------------------------------------------------------------------------------------ C5: the fast layer run

/// The registered C5, unchanged. FAILED at its one run after 277 s: "adaptive cycle 3: edge_length_weighted_indicator:
/// grads[0][0] is not finite". Its 8 x 8 base is at cell Peclet 10, where the unlimited linear-upwind solution is unbounded
/// (`u` about -1.9 in the wall row) and SIMPLEC does not converge on the first directional mesh even from the initial fields
/// (ADR 0267 amendment 2; the corrected check is C5b below).
#[test]
#[ignore = "known miss, defective set-up (ADR 0267 amendment 2): record of the registered C5; about 5 minutes"]
fn c5_directional_layer_loop_fast_known_miss() {
    let set = adaptive_settings(3, MAX_ITER);
    let run = run_adaptive_simple_directional_target(layer_dir(8, [3, 3]), layer_setup, &set, 100, 2.0, 0.5).unwrap();
    print_records("C5", &run.cycles);
    let mesh = &run.solver.geom.mesh;
    mesh.validate().unwrap();
    let jump = max_axis_level_jump(&run.amr, mesh);
    let n = run.amr.n_cells();
    let sums = level_sums(&run.amr);
    println!(
        "F-348 C5: {n} cells, jump {jump:?}, x levels {:?}, y levels {:?}, level sums {sums:?}, E {:.4e}",
        axis_histogram(&run.amr, 0),
        axis_histogram(&run.amr, 1),
        layer_error(&run.solver)
    );
    assert!(jump.iter().all(|&j| j <= 1), "jump {jump:?}");
    assert!((100..=110).contains(&n), "N = {n}");
    assert!(strictly_increasing(&run.cycles));
    assert!(sums[1] > sums[0], "level sums {sums:?}");
    assert_eq!(run.amr.leaves().iter().map(|l| l.levels[1]).max(), Some(3));
}

// ------------------------------------------------------------------------------------------------ C5b (amendment 2)

#[test]
fn c5b_directional_layer_loop_fast() {
    let set = adaptive_settings(3, MAX_ITER);
    let run = run_adaptive_simple_directional_target(layer_dir(16, [3, 3]), layer_setup, &set, 400, 2.0, 0.5).unwrap();
    print_records("C5b", &run.cycles);
    let mesh = &run.solver.geom.mesh;
    mesh.validate().unwrap();
    let jump = max_axis_level_jump(&run.amr, mesh);
    let n = run.amr.n_cells();
    let sums = level_sums(&run.amr);
    println!(
        "F-348 C5b: {n} cells, jump {jump:?}, x levels {:?}, y levels {:?}, level sums {sums:?}, E {:.4e}",
        axis_histogram(&run.amr, 0),
        axis_histogram(&run.amr, 1),
        layer_error(&run.solver)
    );
    assert!(run.cycles.iter().all(|c| c.converged), "a solve did not converge");
    assert!(jump.iter().all(|&j| j <= 1), "jump {jump:?}");
    assert!((400..=440).contains(&n), "N = {n}");
    assert!(strictly_increasing(&run.cycles));
    assert!(sums[1] > sums[0], "level sums {sums:?}");
    assert_eq!(run.amr.leaves().iter().map(|l| l.levels[1]).max(), Some(3));
}

// ------------------------------------------------------------------------------------------------ the heavy layer runs

struct Directional {
    run: DirectionalRun,
    e: f64,
}

fn report(name: &str, run: DirectionalRun, t: Instant) -> Directional {
    print_records(name, &run.cycles);
    let e = layer_error(&run.solver);
    let cc = run.solver.geom.mesh.cell_centres();
    let with_x: Vec<usize> = (0..run.amr.n_cells())
        .filter(|&c| run.amr.leaves()[c].levels[0] > 0)
        .collect();
    let first_two = with_x.iter().filter(|&&c| cc[c][0] < 0.125).count();
    let x_max = with_x.iter().map(|&c| cc[c][0]).fold(0.0, f64::max);
    let y_min = with_x.iter().map(|&c| cc[c][1]).fold(1.0, f64::min);
    println!(
        "F-348 {name}: {} cells, E {e:.5e}, x levels {:?}, y levels {:?}, level sums {:?}; leaves with l_x > 0: {} ({} with \
         x < 1/8, largest x {x_max:.4}, smallest y {y_min:.4}); jump {:?}; {:.1} s",
        run.amr.n_cells(),
        axis_histogram(&run.amr, 0),
        axis_histogram(&run.amr, 1),
        level_sums(&run.amr),
        with_x.len(),
        first_two,
        max_axis_level_jump(&run.amr, &run.solver.geom.mesh),
        t.elapsed().as_secs_f64()
    );
    Directional { run, e }
}

fn layer_directional(name: &str, max_level: [u8; 2], cycles: usize, target: usize) -> Directional {
    let t = Instant::now();
    let set = adaptive_settings(cycles, MAX_ITER);
    let run =
        run_adaptive_simple_directional_target(layer_dir(16, max_level), layer_setup, &set, target, 2.0, 0.5).unwrap();
    report(name, run, t)
}

/// D with the two variations of every cell exchanged before the marks (the negative control).
fn layer_swapped(name: &str) -> Directional {
    let t = Instant::now();
    let set = adaptive_settings(6, MAX_ITER);
    let run = run_adaptive_simple_directional(
        layer_dir(16, [5, 5]),
        layer_setup,
        &set,
        |s: &SimpleSolver, amr: &AnisoMesh, k: usize| {
            let eta = velocity_edge_length_indicator(s, AXES, 2.0)?;
            let var: Vec<Vec3> = velocity_axis_variations(s, AXES)?
                .iter()
                .map(|v| [v[1], v[0], v[2]])
                .collect();
            directional_target_marks(
                &eta,
                &var,
                &amr.levels(),
                amr.max_levels(),
                AXES,
                target_schedule(256, 1536, 6, k),
                0.5,
            )
        },
    )
    .unwrap();
    report(name, run, t)
}

struct Isotropic {
    run: AdaptiveRun,
    e: f64,
}

/// ADR 0247's run A, through the unchanged isotropic path.
fn layer_isotropic() -> Isotropic {
    let t = Instant::now();
    let set = adaptive_settings(5, MAX_ITER);
    let run = run_adaptive_simple_target(layer_iso(16, 4), layer_setup, &set, 3840, |s: &SimpleSolver| {
        velocity_edge_length_indicator(s, AXES, 2.0)
    })
    .unwrap();
    print_records("A", &run.cycles);
    let e = layer_error(&run.solver);
    println!(
        "F-348 A: {} cells, E {e:.5e}, {:.1} s",
        run.amr.n_cells(),
        t.elapsed().as_secs_f64()
    );
    Isotropic { run, e }
}

struct LayerRuns {
    a: Isotropic,
    d: Directional,
    dswap: Directional,
    d960: Directional,
    d4: Directional,
}

fn layer_runs() -> &'static LayerRuns {
    static RUNS: OnceLock<LayerRuns> = OnceLock::new();
    RUNS.get_or_init(|| {
        let r = LayerRuns {
            a: layer_isotropic(),
            d: layer_directional("D", [5, 5], 6, 1536),
            dswap: layer_swapped("Dswap"),
            d960: layer_directional("D960", [5, 5], 6, 960),
            d4: layer_directional("D4", [4, 4], 5, 960),
        };
        println!(
            "F-348 layer results: E(A) {:.5e} N_A {}; E(D) {:.5e} N_D {} E(D)/E(A) {:.4} N_D/N_A {:.4}; E(Dswap) {:.5e} N {}; \
             E(D960) {:.5e} N {}; E(D4) {:.5e} N {}",
            r.a.e,
            r.a.run.amr.n_cells(),
            r.d.e,
            r.d.run.amr.n_cells(),
            r.d.e / r.a.e,
            r.d.run.amr.n_cells() as f64 / r.a.run.amr.n_cells() as f64,
            r.dswap.e,
            r.dswap.run.amr.n_cells(),
            r.d960.e,
            r.d960.run.amr.n_cells(),
            r.d4.e,
            r.d4.run.amr.n_cells()
        );
        r
    })
}

#[test]
#[ignore = "heavy (ADR 0267 layer): run in release with --include-ignored"]
fn s0_the_copied_set_up_reproduces_adr_0247_run_a() {
    let r = layer_runs();
    assert_eq!(r.a.run.amr.n_cells(), 3841);
    assert!((r.a.e - 0.01159).abs() <= 1e-5, "E(A) {}", r.a.e);
}

#[test]
#[ignore = "heavy (ADR 0267 layer): run in release with --include-ignored"]
fn h1_directional_run_reaches_the_isotropic_adapted_error() {
    let r = layer_runs();
    assert!(r.d.e <= r.a.e, "E(D) {} > E(A) {}", r.d.e, r.a.e);
}

#[test]
#[ignore = "heavy (ADR 0267 layer): run in release with --include-ignored"]
fn h2_directional_run_has_at_most_half_the_isotropic_cells() {
    let r = layer_runs();
    assert!(r.d.run.amr.n_cells() <= 1920, "N_D {}", r.d.run.amr.n_cells());
}

#[test]
#[ignore = "heavy (ADR 0267 layer): run in release with --include-ignored"]
fn h3_directional_target_count_lands_within_10_percent() {
    let n = layer_runs().d.run.amr.n_cells() as f64;
    assert!((n - 1536.0).abs() <= 153.6, "N_D {n}");
}

#[test]
#[ignore = "heavy (ADR 0267 layer): run in release with --include-ignored"]
fn h4_directional_solves_converge_and_the_mesh_is_sound() {
    let d = &layer_runs().d.run;
    assert!(d.cycles.iter().all(|c| c.converged), "a solve of D did not converge");
    let mesh = &d.solver.geom.mesh;
    mesh.validate().unwrap();
    let jump = max_axis_level_jump(&d.amr, mesh);
    assert!(jump.iter().all(|&j| j <= 1), "jump {jump:?}");
    assert!(strictly_increasing(&d.cycles));
    assert!(d.amr.leaves().iter().any(|l| l.levels[1] == 5), "no leaf at y level 5");
}

#[test]
#[ignore = "heavy (ADR 0267 layer): run in release with --include-ignored"]
fn nch1_swapped_axes_miss_the_isotropic_adapted_error() {
    let r = layer_runs();
    assert!(r.dswap.e > r.a.e, "E(Dswap) {} <= E(A) {}", r.dswap.e, r.a.e);
}
