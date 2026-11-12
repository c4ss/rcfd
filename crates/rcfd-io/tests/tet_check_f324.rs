//! ADR 0231 (F-324): the output validity check of `tetrahedralize` on F14 of ADR 0228, the STL cylinder-in-box candidate
//! `(theta, h_min, h_max) = (0.1, 0.02, 0.2)` with default options: C4a, C4b, C6 (17134 cells, 5050 points at the F-306 fix) and
//! the cost C5 (ignored, run once). Limits are those of docs/adr/0231-f324-registration-tet-output-check.md, registered (with its
//! amendment 1) before this file was written. The STL helpers, V and the fingerprint are copied from `tet_fold_f306.rs`.

use rcfd_core::vec3::{cross, dot, sub, unit};
use rcfd_io::stl::{self, StlSolid};
use rcfd_mesh::predicates::orient3d;
use rcfd_mesh::surface::{box_surface, cylinder, diagnose, DiagOptions, TriSurface};
use rcfd_mesh::surface_remesh::{remesh, RemeshOptions, SizeField};
use rcfd_mesh::tetmesh::{tetrahedralize, TetMesh, TetOptions};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;
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
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("f324_cylinder_in_box_{}_{n}.stl", std::process::id()));
    std::fs::write(&path, text).unwrap();
    let back = stl::read_file(&path).unwrap();
    let mut s = TriSurface::from_soup(&back[0].triangles, 0.0);
    let mut cyl = TriSurface::from_soup(&back[1].triangles, 0.0);
    cyl.set_tag(1);
    s.append(&cyl);
    s
}

fn f14_surface() -> TriSurface {
    let f = SizeField::Curvature {
        theta: 0.1,
        h_min: 0.02,
        h_max: 0.2,
    };
    remesh(&from_stl_file(), &f, &RemeshOptions::default()).unwrap().0
}

fn no_pass() -> TetOptions {
    TetOptions {
        quality_sweeps: 0,
        ..Default::default()
    }
}

/// V of ADR 0228.
fn validity(s: &TriSurface, m: &TetMesh) -> Result<(), String> {
    let mut vol = 0.0;
    for (i, c) in m.cells.iter().enumerate() {
        let p = c.map(|v| m.points[v]);
        if orient3d(p[0], p[1], p[2], p[3]) <= 0 {
            return Err(format!("V(a): cell {i} is not positively oriented"));
        }
        vol += dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), sub(p[3], p[0])) / 6.0;
    }
    let e = diagnose(s, &DiagOptions::default()).enclosed_volume;
    let rel = (vol - e).abs() / e;
    if rel.is_nan() || rel > 1e-12 {
        return Err(format!(
            "V(b): cell volume {vol:.15e} against enclosed {e:.15e}, relative {rel:.3e}"
        ));
    }
    m.to_mesh(&BTreeMap::new()).map_err(|x| format!("V(c): to_mesh: {x}"))?;
    Ok(())
}

/// F14's outcome: `Ok((cells, points), V, diff / B)` or the error or panic text, computed once per test process.
type F14 = Result<((usize, usize), Result<(), String>, f64), String>;

fn f14() -> &'static F14 {
    static R: OnceLock<F14> = OnceLock::new();
    R.get_or_init(|| {
        let s = f14_surface();
        let r = match catch_unwind(AssertUnwindSafe(|| tetrahedralize(&s, &no_pass()))) {
            Ok(Ok(m)) => {
                let v = validity(&s, &m);
                let margin = m
                    .check_output()
                    .map(|c| c.difference.abs() / c.bound)
                    .unwrap_or(f64::NAN);
                Ok(((m.cells.len(), m.points.len()), v, margin))
            }
            Ok(Err(e)) => Err(format!("Err {e}")),
            Err(p) => Err(format!(
                "panic {}",
                p.downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_default()
            )),
        };
        eprintln!("F14: {r:?}");
        r
    })
}

/// C4a (K1): F14 does not return an `Ok` that fails V.
#[test]
fn c4a_k1_f14_no_overlapping_ok() {
    if let Ok((_, Err(e), _)) = f14() {
        panic!("C4a F14: overlapping Ok: {e}");
    }
}

/// C4b: F14 returns `Ok`.
#[test]
fn c4b_f14_returns_ok() {
    if let Err(e) = f14() {
        panic!("C4b F14: {e}");
    }
}

/// C6: F14 has the cells and points of ADR 0228's addendum (Windows).
#[test]
fn c6_f14_equals_the_f306_record() {
    match f14() {
        Ok((cp, _, _)) => {
            if cfg!(windows) {
                assert_eq!(*cp, (17134, 5050), "C6 F14");
            } else {
                eprintln!("C6 F14: no record for this platform; printed only");
            }
        }
        Err(e) => panic!("C6 F14: no mesh: {e}"),
    }
}

/// C5 (ADR 0231): the check's time on F14 at most 5 percent of the `tetrahedralize` time. Timing; run once by ADR 0231.
#[test]
#[ignore = "timing, run once by ADR 0231 (C5)"]
fn c5_check_cost_on_f14() {
    let s = f14_surface();
    let t0 = Instant::now();
    let m = tetrahedralize(&s, &no_pass()).expect("F14");
    let t_mesh = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let c = m.check_output().expect("check");
    let t_check = t1.elapsed().as_secs_f64();
    let frac = t_check / t_mesh;
    eprintln!(
        "C5 F14: {} cells, tetrahedralize {t_mesh:.4} s, check {t_check:.6} s, fraction {frac:.3e}, diff/B {:.3e}",
        m.cells.len(),
        c.difference.abs() / c.bound
    );
    assert!(frac <= 0.05, "C5 F14: fraction {frac:.3e}");
}
