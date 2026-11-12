//! F-441 (ADR 0343): box coarsening inside the directional adaptive loop of `rcfd_fv::adaptive`, and the coarsening run on
//! the ADR 0247 / ADR 0267 suction layer. Limits registered in ADR 0343 before any of these tests was run.
//!
//! F1 to F4, K1 and CL are ordinary tests. The layer checks (S0, H1 to H6, NC-H1) share one computation and are ignored as
//! heavy: run once in release with `--include-ignored`. The set-up helpers are copies of those of `adapt_directional_f348.rs`
//! (S0 checks the copy against ADR 0267's run D).

use rcfd_core::vec3::Vec3;
use rcfd_core::Result;
use rcfd_fv::adaptive::{
    axis_variations, directional_axes, directional_coarsen_axes, directional_coarsen_marks,
    directional_coarsen_refine_step, directional_target_marks, edge_length_weighted_indicator, merged_gradients,
    run_adaptive_simple_directional_coarsen_target, run_adaptive_simple_directional_coarsening,
    run_adaptive_simple_directional_target, AdaptiveSettings, CycleRecord,
};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::adapt::aniso::{max_axis_level_jump, AnisoMesh, AxisLeaf, AxisMark};
use rcfd_mesh::adapt::{transfer_field, RefineTransfer, Source};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::{Mesh, PatchKind};
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

const AXES: [bool; 3] = [true, true, false];
const MAX_ITER: usize = 20000;
const TOL: f64 = 1e-7;
const THETA: f64 = 0.5;
const PHI: f64 = 0.125;

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

fn mark_all(amr: &AnisoMesh, m: AxisMark) -> AnisoMesh {
    amr.adapt(&vec![m; amr.n_cells()], false).unwrap().0
}

fn refine_one(amr: &AnisoMesh, cell: usize, axes: [bool; 3]) -> AnisoMesh {
    let mut marks = vec![AxisMark::KEEP; amr.n_cells()];
    marks[cell] = AxisMark::refine(axes);
    amr.adapt(&marks, false).unwrap().0
}

/// A fixed linear congruential generator in `[0, 1)`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ------------------------------------------------------------------------------------------------ F1

#[test]
fn f1_coarsening_axes_closed_form() {
    let f = |v: Vec3, l: [u8; 3]| directional_coarsen_axes(v, l, AXES, PHI).unwrap();
    assert_eq!(f([0.1, 1.0, 0.0], [1, 1, 0]), [true, false, false]);
    assert_eq!(f([0.125, 1.0, 0.0], [1, 1, 0]), [false, false, false]);
    assert_eq!(f([0.2, 1.0, 0.0], [1, 1, 0]), [false, false, false]);
    assert_eq!(f([0.1, 1.0, 0.0], [0, 1, 0]), [false, false, false]);
    assert_eq!(f([1.0, 0.1, 0.0], [1, 1, 0]), [false, true, false]);
    assert_eq!(f([0.0, 0.0, 0.0], [1, 1, 0]), [false, false, false]);
    assert_eq!(f([0.0, 1.0, 0.0], [2, 2, 0]), [true, false, false]);
    // a variation on the inactive z never selects z (and does not count as the largest)
    assert_eq!(f([0.0, 0.0, 5.0], [1, 1, 3]), [false, false, false]);
    assert_eq!(f([0.1, 1.0, 0.0], [1, 1, 3]), [true, false, false]);
    assert_eq!(
        directional_coarsen_axes([0.0, 1.0, 0.0], [2, 2, 0], AXES, 0.0).unwrap(),
        [false, false, false]
    );
    for phi in [-0.1, 0.3, f64::NAN] {
        let e = directional_coarsen_axes([0.1, 1.0, 0.0], [1, 1, 0], AXES, phi).unwrap_err();
        assert!(e.to_string().contains("phi"), "{e}");
    }
    for v in [[f64::NAN, 1.0, 0.0], [-0.1, 1.0, 0.0]] {
        let e = directional_coarsen_axes(v, [1, 1, 0], AXES, PHI).unwrap_err();
        assert!(e.to_string().contains("variations"), "{e}");
    }
    let marks = directional_coarsen_marks(
        &[[0.1, 1.0, 0.0], [0.5, 1.0, 0.0], [1.0, 0.0, 0.0]],
        &[[1, 1, 0], [1, 1, 0], [0, 2, 0]],
        AXES,
        PHI,
    )
    .unwrap();
    assert_eq!(
        marks,
        vec![
            AxisMark::coarsen([true, false, false]),
            AxisMark::KEEP,
            AxisMark::coarsen([false, true, false])
        ]
    );
    let e = directional_coarsen_marks(&[[0.1, 1.0, 0.0]], &[], AXES, PHI).unwrap_err();
    assert!(e.to_string().contains("levels"), "{e}");
}

// ------------------------------------------------------------------------------------------------ F2

#[test]
fn f2_hysteresis_exact_on_a_grid() {
    let grid: Vec<f64> = (0..=128).map(|k| k as f64 / 64.0).collect();
    let (mut split_cases, mut coarsen_cases) = (0usize, 0usize);
    for &ex in &grid {
        for &ey in &grid {
            let e = [ex, ey, 0.0];
            // (a) a just-split axis is not coarsened back
            let s = directional_axes(e, [1, 1, 0], [3, 3, 0], AXES, THETA).unwrap();
            if s.contains(&true) {
                split_cases += 1;
                let mut e2 = e;
                let mut l2 = [1u8, 1, 0];
                for a in 0..3 {
                    if s[a] {
                        e2[a] /= 2.0;
                        l2[a] += 1;
                    }
                }
                let c = directional_coarsen_axes(e2, l2, AXES, PHI).unwrap();
                for a in 0..3 {
                    assert!(!(s[a] && c[a]), "split {s:?} then coarsened {c:?} at {e:?}");
                }
            }
            // (b) an axis merged by one or two levels is not split again
            let c = directional_coarsen_axes(e, [2, 2, 0], AXES, PHI).unwrap();
            if c.contains(&true) {
                coarsen_cases += 1;
                for m in 1..=2u8 {
                    let mut e2 = e;
                    let mut l2 = [2u8, 2, 0];
                    for a in 0..3 {
                        if c[a] {
                            e2[a] *= (1u32 << m) as f64;
                            l2[a] -= m;
                        }
                    }
                    let s2 = directional_axes(e2, l2, [3, 3, 0], AXES, THETA).unwrap();
                    for a in 0..3 {
                        assert!(!(c[a] && s2[a]), "coarsened {c:?} by {m} then split {s2:?} at {e:?}");
                    }
                }
            }
        }
    }
    println!("F-441 F2: {split_cases} pairs with a split, {coarsen_cases} with a coarsening");
    assert!(split_cases > 0 && coarsen_cases > 0);
}

// ------------------------------------------------------------------------------------------------ F3, F4

/// F3's lattice: a 4 x 2 slab, every base cell split along x, the right half of base cell 0 split along x again.
fn f3_lattice() -> AnisoMesh {
    let amr = dir_slab([uniform_coords(4), uniform_coords(2)], [2, 2], side_names());
    let amr = mark_all(&amr, AxisMark::refine([true, false, false]));
    assert_eq!(amr.n_cells(), 16);
    let amr = refine_one(&amr, 1, [true, false, false]);
    assert_eq!(amr.n_cells(), 17);
    assert_eq!(amr.leaves()[0].levels, [1, 0, 0]);
    assert_eq!(amr.leaves()[1].levels, [2, 0, 0]);
    assert_eq!(amr.leaves()[2].levels, [2, 0, 0]);
    amr
}

fn f3_grads(n: usize) -> Vec<Vec<Vec3>> {
    vec![(0..n).map(|i| [0.0, 1.0 + i as f64 / 16.0, 0.0]).collect()]
}

fn integral(mesh: &Mesh, q: &[f64]) -> (f64, f64) {
    let v = mesh.cell_volumes();
    (
        v.iter().zip(q).map(|(v, q)| v * q).sum(),
        v.iter().zip(q).map(|(v, q)| v * q.abs()).sum(),
    )
}

#[test]
fn f3_coarsen_then_refine_step_closed_form() {
    let amr = f3_lattice();
    let mesh = amr.to_mesh().unwrap();
    let grads = f3_grads(17);
    let var = axis_variations(&mesh, &grads, AXES).unwrap();
    let cmarks = directional_coarsen_marks(&var, &amr.levels(), AXES, PHI).unwrap();
    assert!(cmarks.iter().all(|m| *m == AxisMark::coarsen([true, false, false])));
    let step = directional_coarsen_refine_step(&amr, &mesh, &grads, &cmarks, 10, 2.0, THETA, false).unwrap();
    let (mid, mid_mesh, t1) = step.coarsened.as_ref().expect("a merge");
    assert_eq!(mid.n_cells(), 8);
    assert!(mid.levels().iter().all(|l| *l == [0, 0, 0]));
    assert_eq!(step.coarsen_report.coarsened, 8);
    assert_eq!(t1.sources[0], Source::Merged(vec![0, 1, 2]));
    for b in 1..8 {
        assert_eq!(t1.sources[b], Source::Merged(vec![2 * b + 1, 2 * b + 2]));
    }
    let mg = merged_gradients(&mesh, t1, &grads).unwrap();
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-14 * b.abs();
    assert_eq!(mg.len(), 1);
    assert!(close(mg[0][0][1], 1.0 + 0.75 / 16.0), "{:?}", mg[0][0]);
    for b in 1..8 {
        assert!(
            close(mg[0][b][1], 1.0 + (2.0 * b as f64 + 1.5) / 16.0),
            "{b}: {:?}",
            mg[0][b]
        );
    }
    assert!(mg[0].iter().all(|g| g[0] == 0.0 && g[2] == 0.0));
    assert_eq!(step.next.n_cells(), 10);
    assert_eq!(step.report.refined, 2);
    let mut split: Vec<AxisLeaf> = step
        .next
        .leaves()
        .iter()
        .filter(|l| l.levels != [0, 0, 0])
        .copied()
        .collect();
    split.sort_by_key(|l| l.ijk);
    assert_eq!(split.len(), 4);
    assert!(split.iter().all(|l| l.levels == [0, 1, 0]));
    let xs: HashSet<i64> = split.iter().map(|l| l.ijk[0]).collect();
    assert_eq!(xs, HashSet::from([2, 3]));
    assert!(split.iter().all(|l| l.ijk[1] >= 2));
    // fields through the two transfers
    let next_mesh = step.next.to_mesh().unwrap();
    let lin = |c: Vec3| 1.3 + 0.7 * c[0] - 0.4 * c[1];
    let q: Vec<f64> = mesh.cell_centres().iter().map(|&c| lin(c)).collect();
    let q1 = transfer_field(&mesh, mid_mesh, t1, &q, RefineTransfer::Linear, AXES).unwrap();
    let q2 = transfer_field(mid_mesh, &next_mesh, &step.transfer, &q1, RefineTransfer::Linear, AXES).unwrap();
    let range = 0.7 + 0.4;
    for (c, v) in next_mesh.cell_centres().iter().zip(&q2) {
        assert!((v - lin(*c)).abs() <= 1e-12 * range, "{v} at {c:?}");
    }
    let mut g = Lcg(441);
    let rnd: Vec<f64> = (0..17).map(|_| g.next() - 0.3).collect();
    let r1 = transfer_field(&mesh, mid_mesh, t1, &rnd, RefineTransfer::Linear, AXES).unwrap();
    let (a, s) = integral(&mesh, &rnd);
    let (b, _) = integral(mid_mesh, &r1);
    assert!((a - b).abs() <= 1e-13 * s, "{a} {b}");
}

#[test]
fn f4_located_errors_and_the_no_op() {
    let set = adaptive_settings(1, 5);
    let lattice = || dir_slab([uniform_coords(4), uniform_coords(4)], [2, 2], cavity_names());
    for phi in [0.2, f64::NAN, -0.1] {
        let e = run_adaptive_simple_directional_coarsen_target(lattice(), cavity_setup, &set, 30, 2.0, THETA, phi)
            .err()
            .expect("an error");
        assert!(e.to_string().contains("phi"), "{e}");
    }
    // all-KEEP coarsening marks: the step is the refinement of the given lattice
    let amr = f3_lattice();
    let mesh = amr.to_mesh().unwrap();
    let grads = f3_grads(17);
    let keep = vec![AxisMark::KEEP; 17];
    let step = directional_coarsen_refine_step(&amr, &mesh, &grads, &keep, 20, 2.0, THETA, false).unwrap();
    assert!(step.coarsened.is_none());
    let eta = edge_length_weighted_indicator(&mesh, &grads, AXES, 2.0).unwrap();
    let var = axis_variations(&mesh, &grads, AXES).unwrap();
    let marks = directional_target_marks(&eta, &var, &amr.levels(), amr.max_levels(), AXES, 20, THETA).unwrap();
    let (next, tr, rep) = amr.adapt(&marks, false).unwrap();
    assert_eq!(step.next.leaves(), next.leaves());
    assert_eq!(step.transfer, tr);
    assert_eq!(step.report, rep);
    let e = directional_coarsen_refine_step(&amr, &mesh, &grads, &keep[..3], 20, 2.0, THETA, false)
        .err()
        .expect("an error");
    assert!(e.to_string().contains("coarsen_marks"), "{e}");
}

// ------------------------------------------------------------------------------------------------ K1

#[test]
fn k1_no_coarsening_asked_is_the_directional_loop_bit_for_bit() {
    let set = adaptive_settings(2, 30);
    let lattice = || dir_slab([uniform_coords(12), uniform_coords(12)], [2, 2], cavity_names());
    let a = run_adaptive_simple_directional_target(lattice(), cavity_setup, &set, 600, 2.0, THETA).unwrap();
    let b =
        run_adaptive_simple_directional_coarsen_target(lattice(), cavity_setup, &set, 600, 2.0, THETA, 0.0).unwrap();
    assert_eq!(a.cycles, b.cycles);
    assert_eq!(a.amr.leaves(), b.amr.leaves());
    assert!(a.solver.geom.mesh.points() == b.solver.geom.mesh.points());
    assert!(a.solver.u.values == b.solver.u.values);
    assert!(a.solver.p.values == b.solver.p.values);
    assert!(a.solver.phi == b.solver.phi);
    assert!(b.coarsened.iter().all(|&c| c == 0));
    assert!(a.amr.n_cells() >= 600);
    println!("F-441 K1: {} cells, records {:?}", a.amr.n_cells(), a.cycles);
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

fn layer_dir(n: usize, max_level: [u8; 2]) -> AnisoMesh {
    dir_slab([uniform_coords(n), uniform_coords(n)], max_level, layer_names())
}

fn print_records(name: &str, cycles: &[CycleRecord], merges: &[usize]) {
    for (k, r) in cycles.iter().enumerate() {
        println!(
            "F-441 {name} cycle {k}: {} cells, {} iterations, converged {}, refined {}, ignored at max level {}, merges {}",
            r.cells,
            r.iterations,
            r.converged,
            r.refined,
            r.ignored_at_max_level,
            merges.get(k).copied().unwrap_or(0)
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

/// Leaves of cycle `k + 1` that were leaves at cycle `k - 1` and not at cycle `k`, summed over the cycles.
fn alternations(history: &[Vec<AxisLeaf>]) -> usize {
    let sets: Vec<HashSet<AxisLeaf>> = history.iter().map(|h| h.iter().copied().collect()).collect();
    (2..sets.len())
        .map(|k| {
            sets[k]
                .iter()
                .filter(|l| !sets[k - 1].contains(l) && sets[k - 2].contains(l))
                .count()
        })
        .sum()
}

/// What the layer checks read of a run.
struct Layer {
    amr: AnisoMesh,
    mesh: Arc<Mesh>,
    cycles: Vec<CycleRecord>,
    merges: Vec<usize>,
    e: f64,
    x_leaves: usize,
}

fn layer_report(
    name: &str,
    amr: AnisoMesh,
    solver: &SimpleSolver,
    cycles: Vec<CycleRecord>,
    merges: Vec<usize>,
    alt: Option<usize>,
    t: Instant,
) -> Layer {
    print_records(name, &cycles, &merges);
    let e = layer_error(solver);
    let cc = solver.geom.mesh.cell_centres();
    let with_x: Vec<usize> = (0..amr.n_cells()).filter(|&c| amr.leaves()[c].levels[0] > 0).collect();
    let first_two = with_x.iter().filter(|&&c| cc[c][0] < 0.125).count();
    let outside = with_x.iter().filter(|&&c| cc[c][1] < 0.9).count();
    let x_max = with_x.iter().map(|&c| cc[c][0]).fold(0.0, f64::max);
    let y_min = with_x.iter().map(|&c| cc[c][1]).fold(1.0, f64::min);
    println!(
        "F-441 {name}: {} cells, E {e:.6e}, x levels {:?}, y levels {:?}, level sums {:?}; leaves with l_x > 0: {} ({} with \
         x < 1/8, {} with y < 0.9, largest x {x_max:.4}, smallest y {y_min:.4}); jump {:?}; merges {}; alternations {:?}; \
         {:.1} s",
        amr.n_cells(),
        axis_histogram(&amr, 0),
        axis_histogram(&amr, 1),
        level_sums(&amr),
        with_x.len(),
        first_two,
        outside,
        max_axis_level_jump(&amr, &solver.geom.mesh),
        merges.iter().sum::<usize>(),
        alt,
        t.elapsed().as_secs_f64()
    );
    Layer {
        mesh: solver.geom.mesh.clone(),
        amr,
        cycles,
        merges,
        e,
        x_leaves: with_x.len(),
    }
}

/// ADR 0267's D (or D1280) through the unchanged function.
fn layer_plain(name: &str, target: usize) -> Layer {
    let t = Instant::now();
    let set = adaptive_settings(6, MAX_ITER);
    let run =
        run_adaptive_simple_directional_target(layer_dir(16, [5, 5]), layer_setup, &set, target, 2.0, THETA).unwrap();
    let n = run.cycles.len();
    layer_report(name, run.amr, &run.solver, run.cycles, vec![0; n], None, t)
}

fn layer_coarsen(name: &str, target: usize) -> Layer {
    let t = Instant::now();
    let set = adaptive_settings(6, MAX_ITER);
    let run = run_adaptive_simple_directional_coarsen_target(
        layer_dir(16, [5, 5]),
        layer_setup,
        &set,
        target,
        2.0,
        THETA,
        PHI,
    )
    .unwrap();
    let alt = alternations(&run.leaves);
    layer_report(name, run.amr, &run.solver, run.cycles, run.coarsened, Some(alt), t)
}

/// The negative control: every cell marked to coarsen along every axis in every cycle.
fn layer_coarsen_all(name: &str) -> Layer {
    let t = Instant::now();
    let set = adaptive_settings(6, MAX_ITER);
    let run = run_adaptive_simple_directional_coarsening(
        layer_dir(16, [5, 5]),
        layer_setup,
        &set,
        1280,
        2.0,
        THETA,
        |_v: &[Vec3], amr: &AnisoMesh, _k: usize| Ok(vec![AxisMark::coarsen([true; 3]); amr.n_cells()]),
    )
    .unwrap();
    let alt = alternations(&run.leaves);
    layer_report(name, run.amr, &run.solver, run.cycles, run.coarsened, Some(alt), t)
}

// ------------------------------------------------------------------------------------------------ CL: the fast layer run

#[test]
fn cl_coarsening_layer_loop_fast() {
    let t = Instant::now();
    let set = adaptive_settings(3, MAX_ITER);
    let run =
        run_adaptive_simple_directional_coarsen_target(layer_dir(16, [3, 3]), layer_setup, &set, 400, 2.0, THETA, PHI)
            .unwrap();
    let alt = alternations(&run.leaves);
    let r = layer_report("CL", run.amr, &run.solver, run.cycles, run.coarsened, Some(alt), t);
    r.mesh.validate().unwrap();
    let jump = max_axis_level_jump(&r.amr, &r.mesh);
    let n = r.amr.n_cells();
    let sums = level_sums(&r.amr);
    assert!(r.cycles.iter().all(|c| c.converged), "a solve did not converge");
    assert!(jump.iter().all(|&j| j <= 1), "jump {jump:?}");
    assert!((400..=440).contains(&n), "N = {n}");
    assert!(strictly_increasing(&r.cycles));
    assert!(sums[1] > sums[0], "level sums {sums:?}");
    assert_eq!(r.amr.leaves().iter().map(|l| l.levels[1]).max(), Some(3));
    println!("F-441 CL: merges per cycle {:?}", r.merges);
}

// ------------------------------------------------------------------------------------------------ the heavy layer runs

struct LayerRuns {
    d: Layer,
    d1280: Layer,
    dc: Layer,
    dcall: Layer,
    dc1536: Layer,
}

fn layer_runs() -> &'static LayerRuns {
    static RUNS: OnceLock<LayerRuns> = OnceLock::new();
    RUNS.get_or_init(|| {
        let r = LayerRuns {
            d: layer_plain("D", 1536),
            d1280: layer_plain("D1280", 1280),
            dc: layer_coarsen("DC", 1280),
            dcall: layer_coarsen_all("DCall"),
            dc1536: layer_coarsen("DC1536", 1536),
        };
        println!(
            "F-441 layer results: D N {} E {:.6e} x {}; D1280 N {} E {:.6e} x {}; DC N {} E {:.6e} x {} E(DC)/E(D) {:.5}; \
             DCall N {} E {:.6e} x {}; DC1536 N {} E {:.6e} x {}",
            r.d.amr.n_cells(),
            r.d.e,
            r.d.x_leaves,
            r.d1280.amr.n_cells(),
            r.d1280.e,
            r.d1280.x_leaves,
            r.dc.amr.n_cells(),
            r.dc.e,
            r.dc.x_leaves,
            r.dc.e / r.d.e,
            r.dcall.amr.n_cells(),
            r.dcall.e,
            r.dcall.x_leaves,
            r.dc1536.amr.n_cells(),
            r.dc1536.e,
            r.dc1536.x_leaves
        );
        r
    })
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn s0_the_copied_set_up_reproduces_adr_0267_run_d() {
    let d = &layer_runs().d;
    assert_eq!(d.amr.n_cells(), 1547);
    assert!((d.e - 0.0029752).abs() <= 1e-7, "E(D) {}", d.e);
    assert_eq!(d.x_leaves, 704);
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn h1_coarsening_run_reaches_the_error_of_run_d() {
    let e = layer_runs().dc.e;
    assert!(e <= 0.00298, "E(DC) {e}");
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn h2_coarsening_run_has_fewer_cells_than_run_d() {
    let n = layer_runs().dc.amr.n_cells();
    assert!(n < 1547, "N_DC {n}");
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn h3_coarsening_run_has_fewer_x_split_leaves_than_run_d() {
    let x = layer_runs().dc.x_leaves;
    assert!(x < 704, "x(DC) {x}");
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn h4_coarsening_run_is_sound_and_coarsens() {
    let dc = &layer_runs().dc;
    assert!(dc.cycles.iter().all(|c| c.converged), "a solve of DC did not converge");
    dc.mesh.validate().unwrap();
    let jump = max_axis_level_jump(&dc.amr, &dc.mesh);
    assert!(jump.iter().all(|&j| j <= 1), "jump {jump:?}");
    assert!(strictly_increasing(&dc.cycles));
    assert!(dc.amr.leaves().iter().any(|l| l.levels[1] == 5), "no leaf at y level 5");
    assert!(dc.merges.iter().sum::<usize>() >= 1, "no merge");
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn h5_coarsening_has_fewer_x_split_leaves_than_the_same_target_without() {
    let r = layer_runs();
    assert!(
        r.dc.x_leaves < r.d1280.x_leaves,
        "x(DC) {} x(D1280) {}",
        r.dc.x_leaves,
        r.d1280.x_leaves
    );
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn h6_coarsening_target_count_lands_within_10_percent() {
    let n = layer_runs().dc.amr.n_cells() as f64;
    assert!((n - 1280.0).abs() <= 128.0, "N_DC {n}");
}

#[test]
#[ignore = "heavy (ADR 0343 layer): run in release with --include-ignored"]
fn nch1_coarsening_everything_misses_the_error() {
    let e = layer_runs().dcall.e;
    assert!(e > 0.00298, "E(DCall) {e}");
}
