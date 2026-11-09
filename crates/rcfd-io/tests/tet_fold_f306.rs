//! ADR 0228 (F-306): the folded edge removal of `tetmesh` on the STL cylinder-in-box surfaces of the ADR 0107 candidate set-ups
//! (`w42c_tet_default_eval::s0_smoke_finer_candidates`, whose helpers are copied here). R3 is the input that makes a folded
//! removal on main with default options (F14), N1s the fold-free ones, whose fingerprints were recorded on the library of main
//! 9f95fb12 before any library change. Limits are those of
//! docs/adr/0228-f306-registration-tetmesh-neighbour-link-panic-on-a-thin-box.md, registered before this file was written.

use rcfd_core::vec3::{cross, dot, sub, unit};
use rcfd_io::stl::{self, StlSolid};
use rcfd_mesh::predicates::orient3d;
use rcfd_mesh::surface::{box_surface, cylinder, diagnose, DiagOptions, TriSurface};
use rcfd_mesh::surface_remesh::{remesh, RemeshOptions, SizeField};
use rcfd_mesh::tetmesh::{tetrahedralize, TetMesh, TetOptions};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

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
        .join(format!("f306_cylinder_in_box_{}_{n}.stl", std::process::id()));
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

/// The mesher without the pass, as in `w42c_tet_default_eval`.
fn no_pass() -> TetOptions {
    TetOptions {
        quality_sweeps: 0,
        ..Default::default()
    }
}

enum Outcome {
    Mesh(TetMesh),
    Error(String),
    Panic(String),
}

fn run(s: &TriSurface, o: &TetOptions) -> Outcome {
    match catch_unwind(AssertUnwindSafe(|| tetrahedralize(s, o))) {
        Ok(Ok(m)) => Outcome::Mesh(m),
        Ok(Err(e)) => Outcome::Error(e.to_string()),
        Err(p) => Outcome::Panic(
            p.downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_default(),
        ),
    }
}

/// V of ADR 0228 (as in `rcfd-mesh/tests/tetmesh_fold_f306.rs`).
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

/// The fingerprint of ADR 0228 (as in `rcfd-mesh/tests/tetmesh_fold_f306.rs`).
fn fingerprint(m: &TetMesh) -> (usize, usize, u64) {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for p in &m.points {
        for c in p {
            eat(c.to_bits());
        }
    }
    for c in &m.cells {
        for &v in c {
            eat(v as u64);
        }
    }
    for t in &m.surface.tris {
        for &v in t {
            eat(v as u64);
        }
    }
    for &g in &m.surface.tags {
        eat(u64::from(g));
    }
    let r = &m.report;
    for x in [
        r.input_points,
        r.steiner_surface,
        r.steiner_interior,
        r.rounds,
        r.bad_left,
        r.flips,
        r.flat_split,
        r.moves,
    ] {
        eat(x as u64);
    }
    (m.cells.len(), m.points.len(), h)
}

/// R3 (ADR 0228): F14, `(theta, h_min, h_max) = (0.1, 0.02, 0.2)` with default options, returns a valid mesh.
#[test]
fn r3_f14_stl_candidate_returns_a_valid_mesh() {
    let s = curvature_remesh(&from_stl_file(), 0.1, 0.02, 0.2);
    match run(&s, &no_pass()) {
        Outcome::Mesh(m) => {
            let (cells, points, hash) = fingerprint(&m);
            let v = validity(&s, &m);
            eprintln!(
                "F14: Ok, {cells} cells, {points} points, hash {hash:016x}, report {:?}, V {v:?}",
                m.report
            );
            assert!(v.is_ok(), "F14: {}", v.unwrap_err());
        }
        Outcome::Error(e) => panic!("F14: located error instead of a mesh: {e}"),
        Outcome::Panic(p) => panic!("F14: the mesher panicked: {p}"),
    }
}

/// The N1s record: `((theta, h_min, h_max), Ok((cells, points, hash)) or Err(text))` of the fold-free candidates, printed by the
/// library of main 9f95fb12 (Windows) before any library change of F-306.
type Record = ((f64, f64, f64), Result<(usize, usize, u64), &'static str>);
const RECORD: &[Record] = &[
    ((0.5, 0.1, 1.0), Ok((1115, 304, 0x2831fddb0b914bb4))),
    ((0.25, 0.05, 0.5), Ok((3499, 948, 0xc8259dd07c0f0eb8))),
    ((0.2, 0.04, 0.4), Ok((4439, 1270, 0x7a26ef072cf6c0be))),
    ((0.15, 0.03, 0.3), Ok((6866, 2052, 0x8078a799e5432395))),
    (
        (0.12, 0.025, 0.25),
        Err("mesh error: surface has 2 degenerate and 0 duplicate triangle(s)"),
    ),
    ((0.15, 0.03, 0.2), Ok((10710, 3279, 0xe2f71fa14190a4c1))),
    ((0.15, 0.03, 0.15), Ok((16855, 5302, 0x8453ce14e19b1ada))),
];

/// N1s (ADR 0228): every fold-free candidate gives the recorded fingerprint or error text (Windows) and the `Ok` ones pass V
/// (every platform). Prints a record line per input.
#[test]
fn n1s_fold_free_stl_candidates_bit_identical() {
    let inputs = [
        (0.5, 0.1, 1.0),
        (0.25, 0.05, 0.5),
        (0.2, 0.04, 0.4),
        (0.15, 0.03, 0.3),
        (0.12, 0.025, 0.25),
        (0.15, 0.03, 0.2),
        (0.15, 0.03, 0.15),
    ];
    assert_eq!(RECORD.len(), inputs.len(), "one record per input");
    let base = from_stl_file();
    let mut bad = Vec::new();
    for (k, &(theta, h_min, h_max)) in inputs.iter().enumerate() {
        let s = curvature_remesh(&base, theta, h_min, h_max);
        let got: Result<(usize, usize, u64), String> = match run(&s, &no_pass()) {
            Outcome::Mesh(m) => {
                if let Err(e) = validity(&s, &m) {
                    bad.push(format!("{theta} {h_min} {h_max}: {e}"));
                }
                Ok(fingerprint(&m))
            }
            Outcome::Error(e) => Err(e),
            Outcome::Panic(p) => {
                bad.push(format!("{theta} {h_min} {h_max}: panic {p}"));
                continue;
            }
        };
        match &got {
            Ok((c, p, h)) => eprintln!("    (({theta:?}, {h_min:?}, {h_max:?}), Ok(({c}, {p}, 0x{h:016x}))),"),
            Err(e) => eprintln!("    (({theta:?}, {h_min:?}, {h_max:?}), Err({e:?})),"),
        }
        if cfg!(windows) {
            let (key, want) = RECORD[k];
            assert_eq!(key, (theta, h_min, h_max), "record order");
            let same = match (&got, want) {
                (Ok(a), Ok(b)) => *a == b,
                (Err(a), Err(b)) => a == b,
                _ => false,
            };
            if !same {
                bad.push(format!("{theta} {h_min} {h_max}: {got:?} against the record {want:?}"));
            }
        }
    }
    if !cfg!(windows) {
        eprintln!("N1s: no recorded fingerprints for this platform (ADR 0228 Q6); V only");
    }
    assert!(bad.is_empty(), "N1s: {} inputs fail:\n{}", bad.len(), bad.join("\n"));
}
