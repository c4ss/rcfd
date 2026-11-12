//! F-102 (ADR 0242): the adapted lid-driven cavity against the fine uniform solution, and the checks of the solver-loop hook
//! `rcfd_fv::adaptive`. Limits registered in ADR 0242 before any of these tests was run.
//!
//! The cavity checks share one computation (C 24 x 24, F 96 x 96, U48 48 x 48, the adapted run A and the inverted-indicator run
//! Ainv) and are ignored as heavy: run once in release with `--include-ignored`.

use rcfd_core::vec3::Vec3;
use rcfd_core::Result;
use rcfd_fv::adaptive::{
    fixed_fraction_marks, run_adaptive_simple, run_adaptive_simple_with, velocity_gradient_indicator, AdaptiveRun,
    AdaptiveSettings,
};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::gradient::least_squares;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::adapt::{level_histogram, max_face_level_jump, AmrMesh, Mark, RefineTransfer};
use rcfd_mesh::gen::{axis_coords, Grading};
use rcfd_mesh::PatchKind;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

const AXES: [bool; 3] = [true, true, false];
const MAX_ITER: usize = 20000;
const TOL: f64 = 1e-7;

fn base(n: usize, max_level: u8) -> AmrMesh {
    let w = |s: &str| (s.to_string(), PatchKind::Wall);
    let c = [
        axis_coords(n, 1.0, Grading::Uniform).unwrap(),
        axis_coords(n, 1.0, Grading::Uniform).unwrap(),
        vec![0.0, 0.1],
    ];
    AmrMesh::new(
        c,
        AXES,
        max_level,
        [
            w("walls"),
            w("walls"),
            w("walls"),
            w("lid"),
            ("frontAndBack".into(), PatchKind::Empty),
            ("frontAndBack".into(), PatchKind::Empty),
        ],
    )
    .unwrap()
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

fn read_csv(name: &str) -> Vec<Vec<f64>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../cases/cavity/reference")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.trim().parse().unwrap()).collect())
        .collect()
}

/// Point sampler: mean over the cells whose closed box contains the point of `U_c + G_c . (x - x_c)`.
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

    /// The dense set: u on x = 0.5 at y = j/100, then v on y = 0.5 at x = j/100.
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

    /// Max |U - Ghia| over the 15 interior rows of each Ghia table.
    fn ghia(&self) -> f64 {
        let u_ref = read_csv("ghia1982_u_centerline.csv");
        let v_ref = read_csv("ghia1982_v_centerline_Re100.csv");
        let mut worst = 0.0f64;
        let mut rows = 0;
        for r in u_ref.iter().filter(|r| r[0] > 0.0 && r[0] < 1.0) {
            worst = worst.max((self.at([0.5, r[0], 0.05])[0] - r[1]).abs());
            rows += 1;
        }
        for r in v_ref.iter().filter(|r| r[0] > 0.0 && r[0] < 1.0) {
            worst = worst.max((self.at([r[0], 0.5, 0.05])[1] - r[1]).abs());
            rows += 1;
        }
        assert_eq!(rows, 30);
        worst
    }
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

struct Uniform {
    cells: usize,
    iterations: usize,
    converged: bool,
    dense: Vec<f64>,
    ghia: f64,
}

fn uniform(n: usize) -> Uniform {
    let t = Instant::now();
    let mesh = base(n, 0).to_mesh().unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p, s) = cavity_setup(&geom).unwrap();
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(MAX_ITER, TOL);
    let sm = Sampler::new(&solver);
    let r = Uniform {
        cells: solver.geom.n_cells(),
        iterations: sum.iterations,
        converged: sum.converged,
        dense: sm.dense(),
        ghia: sm.ghia(),
    };
    println!(
        "F-102 uniform {n} x {n}: {} cells, {} iterations, converged {}, G {:.5}, {:.1} s",
        r.cells,
        r.iterations,
        r.converged,
        r.ghia,
        t.elapsed().as_secs_f64()
    );
    r
}

struct Adapted {
    run: AdaptiveRun,
    dense: Vec<f64>,
    ghia: f64,
}

fn adapted(name: &str, inverted: bool) -> Adapted {
    let t = Instant::now();
    let set = adaptive_settings(2, MAX_ITER);
    let run = if inverted {
        run_adaptive_simple_with(base(24, 2), cavity_setup, &set, |s: &SimpleSolver| {
            let eta = velocity_gradient_indicator(s, AXES, true)?;
            let top = eta.iter().cloned().fold(0.0, f64::max);
            Ok(eta.iter().map(|e| top - e).collect())
        })
        .unwrap()
    } else {
        run_adaptive_simple(base(24, 2), cavity_setup, &set).unwrap()
    };
    let sm = Sampler::new(&run.solver);
    let (dense, ghia) = (sm.dense(), sm.ghia());
    for (k, r) in run.cycles.iter().enumerate() {
        println!(
            "F-102 {name} cycle {k}: {} cells, {} iterations, converged {}, refined {}, ignored at max level {}",
            r.cells, r.iterations, r.converged, r.refined, r.ignored_at_max_level
        );
    }
    println!(
        "F-102 {name}: levels {:?}, G {ghia:.5}, {:.1} s",
        level_histogram(&run.amr.levels()),
        t.elapsed().as_secs_f64()
    );
    Adapted { run, dense, ghia }
}

struct Runs {
    c: Uniform,
    f: Uniform,
    u48: Uniform,
    a: Adapted,
    ainv: Adapted,
}

impl Runs {
    fn d(&self, dense: &[f64]) -> f64 {
        max_diff(dense, &self.f.dense)
    }
}

fn runs() -> &'static Runs {
    static RUNS: OnceLock<Runs> = OnceLock::new();
    RUNS.get_or_init(|| {
        let r = Runs {
            c: uniform(24),
            f: uniform(96),
            u48: uniform(48),
            a: adapted("A", false),
            ainv: adapted("Ainv", true),
        };
        println!(
            "F-102 results: N_A {} N_F {} N_Ainv {}; D(A) {:.5} D(C) {:.5} D(U48) {:.5} D(Ainv) {:.5}; D(A)/D(C) {:.4}; \
             G(A) {:.5} G(F) {:.5} G(C) {:.5} G(U48) {:.5} G(Ainv) {:.5}",
            r.a.run.amr.n_cells(),
            r.f.cells,
            r.ainv.run.amr.n_cells(),
            r.d(&r.a.dense),
            r.d(&r.c.dense),
            r.d(&r.u48.dense),
            r.d(&r.ainv.dense),
            r.d(&r.a.dense) / r.d(&r.c.dense),
            r.a.ghia,
            r.f.ghia,
            r.c.ghia,
            r.u48.ghia,
            r.ainv.ghia
        );
        r
    })
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn c1a_adapted_has_fewer_cells_than_fine() {
    let r = runs();
    assert!(r.a.run.amr.n_cells() < r.f.cells, "N_A {}", r.a.run.amr.n_cells());
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn c1b_adapted_has_at_most_half_the_fine_cells() {
    let r = runs();
    assert!(
        r.a.run.amr.n_cells() * 2 <= r.f.cells,
        "N_A {} > {}",
        r.a.run.amr.n_cells(),
        r.f.cells / 2
    );
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn c2_adapted_matches_fine_on_the_centrelines() {
    let r = runs();
    let d = r.d(&r.a.dense);
    assert!(d <= 0.01, "D(A) = {d}");
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn c3_adapted_halves_the_coarse_difference() {
    let r = runs();
    let (da, dc) = (r.d(&r.a.dense), r.d(&r.c.dense));
    assert!(da <= 0.5 * dc, "D(A)/D(C) = {}", da / dc);
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn c4_adapted_and_fine_against_ghia() {
    let r = runs();
    assert!(r.a.ghia <= 0.02, "G(A) = {}", r.a.ghia);
    assert!(r.f.ghia <= 0.02, "G(F) = {}", r.f.ghia);
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn c5_solves_converge_and_the_adapted_mesh_is_sound() {
    let r = runs();
    assert!(r.c.converged, "C not converged");
    assert!(r.f.converged, "F not converged");
    for (k, rec) in r.a.run.cycles.iter().enumerate() {
        assert!(rec.converged, "A cycle {k} not converged");
    }
    let mesh = r.a.run.amr.to_mesh().unwrap();
    mesh.validate().unwrap();
    assert!(max_face_level_jump(&r.a.run.amr, &mesh) <= 1);
    assert!(level_histogram(&r.a.run.amr.levels()).contains_key(&2));
    assert!(r.a.run.cycles.windows(2).all(|w| w[1].cells > w[0].cells));
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn nc1_coarse_fails_c2() {
    let r = runs();
    let d = r.d(&r.c.dense);
    assert!(d > 0.01, "D(C) = {d}");
}

#[test]
#[ignore = "heavy (ADR 0242 cavity): run in release with --include-ignored"]
fn nc2_inverted_indicator_fails_c2() {
    let r = runs();
    let d = r.d(&r.ainv.dense);
    assert!(d > 0.01, "D(Ainv) = {d}");
}

#[test]
fn h1_zero_cycles_is_the_plain_solver() {
    let run = run_adaptive_simple(base(24, 2), cavity_setup, &adaptive_settings(0, 50)).unwrap();
    let mesh = base(24, 2).to_mesh().unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p, s) = cavity_setup(&geom).unwrap();
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(50, TOL);
    assert_eq!(run.cycles.len(), 1);
    assert_eq!(run.cycles[0].iterations, sum.iterations);
    assert!(run.solver.u.values == solver.u.values);
    assert!(run.solver.p.values == solver.p.values);
    assert!(run.solver.phi == solver.phi);
}

fn lin_u(x: Vec3) -> Vec3 {
    [1.0 + 2.0 * x[0] - 3.0 * x[1], 0.5 - x[0] + 4.0 * x[1], 0.0]
}

fn lin_p(x: Vec3) -> f64 {
    2.0 - x[0] + 0.5 * x[1]
}

#[test]
fn h2_the_transfer_is_applied() {
    let mut calls = 0;
    let setup = |geom: &Arc<FvGeometry>| -> Result<(VectorField, ScalarField, SimpleSettings)> {
        let (mut u, mut p, s) = cavity_setup(geom)?;
        if calls == 0 {
            let cc = geom.mesh.cell_centres();
            u.values = cc.iter().map(|&c| lin_u(c)).collect();
            p.values = cc.iter().map(|&c| lin_p(c)).collect();
        }
        calls += 1;
        Ok((u, p, s))
    };
    let run = run_adaptive_simple(base(12, 2), setup, &adaptive_settings(2, 0)).unwrap();
    let n = run.solver.geom.n_cells();
    assert_eq!(n, run.amr.n_cells());
    assert!(n > 144, "{n} cells");
    let cc = run.solver.geom.mesh.cell_centres();
    let mut worst = [0.0f64; 3];
    let mut range = [(f64::INFINITY, f64::NEG_INFINITY); 3];
    for c in 0..n {
        let (eu, ep) = (lin_u(cc[c]), lin_p(cc[c]));
        for (k, (e, v)) in [
            (eu[0], run.solver.u.values[c][0]),
            (eu[1], run.solver.u.values[c][1]),
            (ep, run.solver.p.values[c]),
        ]
        .into_iter()
        .enumerate()
        {
            worst[k] = worst[k].max((e - v).abs());
            range[k] = (range[k].0.min(e), range[k].1.max(e));
        }
    }
    println!("H2: {n} cells, worst {worst:?}");
    for k in 0..3 {
        assert!(worst[k] <= 1e-12 * (range[k].1 - range[k].0), "field {k}: {}", worst[k]);
    }
}

#[test]
fn h3_fixed_fraction_marks() {
    let ind: Vec<f64> = (0..100).map(|i| ((37 * i) % 100) as f64).collect();
    let m = fixed_fraction_marks(&ind, &[0; 100], 1, 0.3).unwrap();
    for i in 0..100 {
        assert_eq!(m[i] == Mark::Refine, ind[i] >= 70.0, "cell {i}");
        assert!(m[i] == Mark::Refine || m[i] == Mark::Keep);
    }
    let mut lv = [0u8; 100];
    lv[..10].iter_mut().for_each(|l| *l = 1);
    let m = fixed_fraction_marks(&ind, &lv, 1, 0.3).unwrap();
    let mut rest: Vec<f64> = ind[10..].to_vec();
    rest.sort_by(|a, b| b.total_cmp(a));
    let cut = rest[29];
    for i in 0..100 {
        let want = i >= 10 && ind[i] >= cut;
        assert_eq!(m[i] == Mark::Refine, want, "cell {i}");
    }
    assert_eq!(m.iter().filter(|x| **x == Mark::Refine).count(), 30);
    let m = fixed_fraction_marks(&[1.0; 4], &[0; 4], 1, 0.5).unwrap();
    assert_eq!(m, vec![Mark::Refine, Mark::Refine, Mark::Keep, Mark::Keep]);
    let m = fixed_fraction_marks(&ind, &[0; 100], 1, 0.0).unwrap();
    assert!(m.iter().all(|x| *x == Mark::Keep));
    let m = fixed_fraction_marks(&ind, &lv, 1, 1.0).unwrap();
    for i in 0..100 {
        assert_eq!(m[i] == Mark::Refine, i >= 10, "cell {i}");
    }
}

#[test]
fn h4_located_errors() {
    for f in [-0.1, 1.5, f64::NAN] {
        let e = fixed_fraction_marks(&[1.0, 2.0], &[0, 0], 1, f)
            .unwrap_err()
            .to_string();
        assert!(e.contains("fraction"), "{e}");
    }
    let e = fixed_fraction_marks(&[1.0, f64::NAN], &[0, 0], 1, 0.5)
        .unwrap_err()
        .to_string();
    assert!(e.contains("indicator"), "{e}");
    let e = fixed_fraction_marks(&[1.0, 2.0], &[0], 1, 0.5).unwrap_err().to_string();
    assert!(e.contains("levels"), "{e}");
    let mut calls = 0;
    let setup = |geom: &Arc<FvGeometry>| -> Result<(VectorField, ScalarField, SimpleSettings)> {
        calls += 1;
        if calls == 2 {
            return Err(rcfd_core::Error::Other("setup refused".into()));
        }
        cavity_setup(geom)
    };
    let e = match run_adaptive_simple(base(8, 2), setup, &adaptive_settings(2, 5)) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("cycle 1") && e.contains("setup refused"), "{e}");
}
