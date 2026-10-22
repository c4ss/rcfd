//! ADR 0107 (wave 42C): evaluation of the ADR 0104 tetrahedral quality pass as a default. Cost, safety and failure-mode
//! checks on the cylinder-in-box STL pipeline and a finer mesh of about ten times more cells. Limits are those registered in
//! docs/adr/0107-wave42c-tet-quality-pass-default-evaluation.md before this file's registered tests were run.

use rcfd_core::vec3::{cross, sub, unit};
use rcfd_io::stl::{self, StlSolid};
use rcfd_mesh::predicates::orient3d;
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::surface::{box_surface, cylinder, TriSurface};
use rcfd_mesh::surface_remesh::{remesh, RemeshOptions, SizeField};
use rcfd_mesh::tetmesh::{tet_quality, tetrahedralize, TetMesh, TetOptions};
use std::collections::BTreeMap;
use std::time::Instant;

fn solid(name: &str, s: &TriSurface) -> StlSolid {
    let triangles: Vec<_> = (0..s.n_tris()).map(|i| s.tri_points(i)).collect();
    let stored_normals = triangles
        .iter()
        .map(|[a, b, c]| unit(cross(sub(*b, *a), sub(*c, *a))))
        .collect();
    StlSolid {
        name: name.into(),
        triangles,
        stored_normals,
    }
}

/// The ADR 0093 STL path (two ASCII solids, read back, welded bitwise, cylinder tagged 1); one temp file per call.
fn from_stl_file() -> TriSurface {
    let b = box_surface([-2.0, -2.0, -1.0], [2.0, 2.0, 1.0], [4, 4, 2]);
    let mut c = cylinder(0.5, 1.0, 16, 4);
    c.flip_all();
    let text = stl::write_ascii(&solid("box", &b)).unwrap() + &stl::write_ascii(&solid("cylinder", &c)).unwrap();
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("w42c_cylinder_in_box_{n}.stl"));
    std::fs::write(&path, text).unwrap();
    let back = stl::read_file(&path).unwrap();
    let mut s = TriSurface::from_soup(&back[0].triangles, 0.0);
    let mut cyl = TriSurface::from_soup(&back[1].triangles, 0.0);
    cyl.set_tag(1);
    s.append(&cyl);
    s
}

fn curvature_remesh(s: &TriSurface, theta: f64, h_min: f64, h_max: f64) -> TriSurface {
    let f = SizeField::Curvature { theta, h_min, h_max };
    remesh(s, &f, &RemeshOptions::default()).unwrap().0
}

fn m1() -> TriSurface {
    curvature_remesh(&from_stl_file(), 0.5, 0.1, 1.0)
}

/// About 9.6 times the M1 cell count (ADR 0107 smoke and selection rule).
fn m10() -> TriSurface {
    curvature_remesh(&from_stl_file(), 0.15, 0.03, 0.2)
}

fn pass() -> TetOptions {
    TetOptions {
        quality_sweeps: 10,
        ..Default::default()
    }
}

/// The mesher without the pass, stated explicitly so the comparison holds whatever the default is.
fn no_pass() -> TetOptions {
    TetOptions {
        quality_sweeps: 0,
        ..Default::default()
    }
}

fn budget_used(tm: &TetMesh) -> f64 {
    let r = &tm.report;
    let budget = TetOptions::default().max_steiner_factor * r.input_points + 10000;
    (r.steiner_surface + r.steiner_interior) as f64 / budget as f64
}

/// L3: the pass on M1 and M10 is Ok, inverts no cell (exact orient3d), keeps volume, patches, surface and `check_mesh`.
#[test]
fn l3_pass_safety_m1_m10() {
    for (n, s) in [("M1", m1()), ("M10", m10())] {
        let base = tetrahedralize(&s, &no_pass()).unwrap();
        let tm = tetrahedralize(&s, &pass()).unwrap_or_else(|e| panic!("{n}: {e}"));
        let inverted = tm
            .cells
            .iter()
            .filter(|c| orient3d(tm.points[c[0]], tm.points[c[1]], tm.points[c[2]], tm.points[c[3]]) <= 0)
            .count();
        assert_eq!(inverted, 0, "{n}: cells with orient3d <= 0");
        let m = tm.to_mesh(&BTreeMap::new()).unwrap();
        assert!(
            m.cell_volumes().iter().all(|&v| v > 0.0),
            "{n}: non-positive cell volume"
        );
        let rep = check_mesh(&m);
        assert!(rep.is_ok(), "{n}: {:?}", rep.errors);
        let e = (m.total_volume() - s.enclosed_volume()).abs() / s.enclosed_volume();
        assert!(e <= 1e-10, "{n}: volume error {e}");
        assert_eq!(m.patches().len(), 2, "{n}");
        assert_eq!(tm.surface.tris.len(), base.surface.tris.len(), "{n}: surface triangles");
        eprintln!(
            "L3 {n}: {} cells (default {}), volume error {e:e}, Steiner budget used {:.4} (default {:.4}), pass {:?} default {:?}",
            tm.cells.len(),
            base.cells.len(),
            budget_used(&tm),
            budget_used(&base),
            tm.report,
            base.report
        );
    }
}

/// Q8: M10 with the pass meets the ADR 0030 T5 limits.
#[test]
fn q8_m10_t5_limits_with_pass() {
    let tm = tetrahedralize(&m10(), &pass()).unwrap();
    let q = tet_quality(&tm, 2.0);
    eprintln!("Q8 M10: {} cells, {q:?}", tm.cells.len());
    assert!(q.min_dihedral_deg >= 3.0, "min dihedral {}", q.min_dihedral_deg);
    assert!(
        q.frac_radius_edge_ok >= 0.99,
        "radius-edge fraction {}",
        q.frac_radius_edge_ok
    );
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// L1 cost factor at most 3.0 on M1 and M10; L2 added time per cell on M10 at most 2.0 times that on M1. Wall-clock, so
/// ignored (load-sensitive); run with `--release -- --ignored --test-threads=1`.
/// Known miss (ADR 0107): L1 on M1 measured 3.051 in the registered run (limit 3.0). Measures both meshes before asserting
/// (amendment 1; the first version stopped at M1).
#[test]
#[ignore = "known miss (ADR 0107 L1): cost factor of the pass on M1 3.051 vs 3.0; wall-clock, run in release with one thread"]
fn l12_known_miss_cost_factor_and_scaling() {
    let (mut per_cell, mut factors) = (Vec::new(), Vec::new());
    for (n, s) in [("M1", m1()), ("M10", m10())] {
        let (mut td, mut tp, mut cells) = (Vec::new(), Vec::new(), 0);
        for _ in 0..5 {
            let t = Instant::now();
            tetrahedralize(&s, &no_pass()).unwrap();
            td.push(t.elapsed().as_secs_f64());
            let t = Instant::now();
            cells = tetrahedralize(&s, &pass()).unwrap().cells.len();
            tp.push(t.elapsed().as_secs_f64());
        }
        let (d, p) = (median(td.clone()), median(tp.clone()));
        let f = p / d;
        per_cell.push((p - d) / cells as f64);
        eprintln!("L1 {n}: default {td:?} s, pass {tp:?} s, medians {d:.4} / {p:.4} s, factor {f:.3}, {cells} cells");
        factors.push((n, f));
    }
    let g = per_cell[1] / per_cell[0];
    eprintln!(
        "L2: added time per cell M1 {:.3e} s, M10 {:.3e} s, ratio {g:.3}",
        per_cell[0], per_cell[1]
    );
    for (n, f) in factors {
        assert!(f <= 3.0, "{n}: cost factor {f}");
    }
    assert!(g <= 2.0, "added time per cell ratio {g}");
}

/// ADR 0107 smoke: default cell counts of the candidate finer set-ups (construction only, the pass is not run).
#[test]
#[ignore = "ADR 0107 set-up smoke: prints only"]
fn s0_smoke_finer_candidates() {
    let s = from_stl_file();
    for (theta, h_min, h_max) in [
        (0.5, 0.1, 1.0),
        (0.25, 0.05, 0.5),
        (0.2, 0.04, 0.4),
        (0.15, 0.03, 0.3),
        (0.12, 0.025, 0.25),
        (0.1, 0.02, 0.2),
        (0.15, 0.03, 0.2),
        (0.15, 0.03, 0.15),
    ] {
        let r = curvature_remesh(&s, theta, h_min, h_max);
        let tm = tetrahedralize(&r, &no_pass()).map(|t| t.cells.len());
        eprintln!(
            "S0 theta {theta} h_min {h_min} h_max {h_max}: {} surface tris, cells {tm:?}",
            r.n_tris()
        );
    }
}
