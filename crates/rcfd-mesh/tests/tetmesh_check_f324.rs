//! ADR 0231 (F-324): the output validity check of `tetrahedralize` (`TetMesh::check_output`). C3 are the negative and positive
//! controls on a cube mesh, C4a and C4b the folded inputs F1 to F13 of ADR 0228, C6 their cell counts and hashes against ADR 0228's
//! addendum, C5 the cost (ignored, run once), C7 a print of the margin. Limits are those of
//! docs/adr/0231-f324-registration-tet-output-check.md, registered (with its amendment 1) before this file was written. The
//! input helpers, V and the fingerprint are copied from `tetmesh_fold_f306.rs`.

use rcfd_core::vec3::{cross, dot, sub};
use rcfd_mesh::predicates::orient3d;
use rcfd_mesh::surface::{box_surface, cube, cylinder, diagnose, icosphere, torus, DiagOptions, TriSurface};
use rcfd_mesh::tetmesh::{tetrahedralize, TetMesh, TetOptions};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::OnceLock;
use std::time::Instant;

struct Input {
    name: String,
    s: TriSurface,
    o: TetOptions,
}

fn input(name: String, s: TriSurface, size: Option<f64>, quality_sweeps: usize) -> Input {
    let o = TetOptions {
        size,
        quality_sweeps,
        ..TetOptions::default()
    };
    Input { name, s, o }
}

fn box_input(hi: [f64; 3], n: [usize; 3], size: Option<f64>, sweeps: usize) -> Input {
    let name = format!(
        "box {hi:?} {n:?} size {} sweeps {sweeps}",
        size.map_or("none".to_string(), |h| h.to_string())
    );
    input(name, box_surface([0.0; 3], hi, n), size, sweeps)
}

/// F1 to F13 of ADR 0228 with the cells (and points, hash where recorded) of ADR 0228's addendum at the F-306 fix.
type Expect = (usize, Option<usize>, Option<u64>);

fn folded() -> Vec<(&'static str, Input, Expect)> {
    let t = [1.0, 0.1, 0.1];
    let p = [1.0, 1.0, 0.1];
    vec![
        ("F1", box_input(t, [10, 1, 1], Some(0.025), 0), (3206, None, None)),
        ("F2", box_input(t, [10, 1, 1], Some(0.025), 1), (3486, None, None)),
        ("F3", box_input(t, [10, 1, 1], Some(0.025), 2), (3941, None, None)),
        ("F4", box_input(t, [10, 1, 1], Some(0.025), 3), (4485, None, None)),
        ("F5", box_input(t, [10, 1, 1], Some(0.0333), 0), (1156, None, None)),
        (
            "F6",
            box_input(t, [10, 1, 1], Some(0.0333), 1),
            (1204, None, Some(0x2d3ca7189bac0af7)),
        ),
        (
            "F7",
            box_input(t, [10, 1, 1], Some(0.0333), 2),
            (1354, None, Some(0x1fb9fcdc40f95c94)),
        ),
        (
            "F8",
            box_input(t, [10, 1, 1], Some(0.0333), 3),
            (1468, None, Some(0x6e90359a20d929c4)),
        ),
        ("F9", box_input(t, [10, 1, 1], Some(0.035), 1), (1125, None, None)),
        ("F10", box_input(t, [10, 1, 1], Some(0.035), 2), (1333, None, None)),
        ("F11", box_input(t, [10, 1, 1], Some(0.035), 3), (1427, Some(323), None)),
        ("F12", box_input(p, [10, 10, 1], Some(0.035), 0), (11292, None, None)),
        ("F13", box_input(p, [10, 10, 1], Some(0.035), 3), (13383, None, None)),
    ]
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

/// The fingerprint of ADR 0228.
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

/// One folded input's outcome, summarised.
struct Row {
    id: &'static str,
    name: String,
    expect: Expect,
    /// `Ok(fingerprint, V, diff / bound)`, `Err(text)` for a located error, or a panic text.
    kind: Kind,
}

enum Kind {
    Mesh((usize, usize, u64), Result<(), String>, f64),
    Error(String),
    Panic(String),
}

/// The 13 outcomes, computed once per test process (amendment 1).
fn rows() -> &'static Vec<Row> {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| {
        folded()
            .into_iter()
            .map(|(id, x, expect)| {
                let kind = match run(&x.s, &x.o) {
                    Outcome::Mesh(m) => {
                        let v = validity(&x.s, &m);
                        let margin = m
                            .check_output()
                            .map(|c| c.difference.abs() / c.bound)
                            .unwrap_or(f64::NAN);
                        Kind::Mesh(fingerprint(&m), v, margin)
                    }
                    Outcome::Error(e) => Kind::Error(e),
                    Outcome::Panic(p) => Kind::Panic(p),
                };
                let row = Row {
                    id,
                    name: x.name,
                    expect,
                    kind,
                };
                match &row.kind {
                    Kind::Mesh(f, v, r) => eprintln!(
                        "{} {}: Ok, {} cells, {} points, hash {:016x}, V {v:?}, diff/B {r:.3e}",
                        row.id, row.name, f.0, f.1, f.2
                    ),
                    Kind::Error(e) => eprintln!("{} {}: Err {e}", row.id, row.name),
                    Kind::Panic(p) => eprintln!("{} {}: panic {p}", row.id, row.name),
                }
                row
            })
            .collect()
    })
}

/// C4a (K1): no folded input returns an `Ok` that fails V.
#[test]
fn c4a_k1_no_folded_input_returns_an_overlapping_ok() {
    let bad: Vec<String> = rows()
        .iter()
        .filter_map(|r| match &r.kind {
            Kind::Mesh(_, Err(e), _) => Some(format!("{} {}: {e}", r.id, r.name)),
            _ => None,
        })
        .collect();
    assert!(bad.is_empty(), "C4a: overlapping Ok:\n{}", bad.join("\n"));
}

/// C4b: every folded input returns `Ok`.
#[test]
fn c4b_every_folded_input_returns_ok() {
    let bad: Vec<String> = rows()
        .iter()
        .filter_map(|r| match &r.kind {
            Kind::Mesh(..) => None,
            Kind::Error(e) => Some(format!("{} {}: Err {e}", r.id, r.name)),
            Kind::Panic(p) => Some(format!("{} {}: panic {p}", r.id, r.name)),
        })
        .collect();
    assert!(bad.is_empty(), "C4b:\n{}", bad.join("\n"));
}

/// C6: the cells (points, hashes where recorded) of the R outputs equal ADR 0228's addendum (Windows).
#[test]
fn c6_folded_outputs_equal_the_f306_record() {
    let mut bad = Vec::new();
    for r in rows() {
        if let Kind::Mesh(f, _, _) = &r.kind {
            let (cells, points, hash) = r.expect;
            let same = f.0 == cells && points.map_or(true, |p| p == f.1) && hash.map_or(true, |h| h == f.2);
            if !same && cfg!(windows) {
                bad.push(format!("{} {}: {f:?} against {:?}", r.id, r.name, r.expect));
            }
        } else {
            bad.push(format!("{} {}: no mesh", r.id, r.name));
        }
    }
    if !cfg!(windows) {
        eprintln!("C6: no record for this platform; printed only");
    }
    assert!(bad.is_empty(), "C6:\n{}", bad.join("\n"));
}

/// C7: the margin `|diff| / B` on the N1 C-group inputs (closed shapes), printed only.
#[test]
fn c7_margin_on_closed_shapes_print_only() {
    type Shape = (&'static str, fn() -> TriSurface);
    let shapes: [Shape; 4] = [
        ("cube(2, 2)", || cube(2.0, 2)),
        ("icosphere(1, 2)", || icosphere(1.0, 2)),
        ("torus(2, 0.5, 24, 12)", || torus(2.0, 0.5, 24, 12)),
        ("cylinder(0.5, 1, 16, 4)", || cylinder(0.5, 1.0, 16, 4)),
    ];
    let mut v = Vec::new();
    for (name, f) in shapes {
        for sweeps in [0, 1, 3] {
            v.push(input(format!("{name} size none sweeps {sweeps}"), f(), None, sweeps));
        }
    }
    v.push(input(
        "icosphere(1, 2) size 0.25 sweeps 0".into(),
        icosphere(1.0, 2),
        Some(0.25),
        0,
    ));
    v.push(input("cube(2, 4) size 0.5 sweeps 0".into(), cube(2.0, 4), Some(0.5), 0));
    let mut worst: f64 = 0.0;
    for x in &v {
        let m = tetrahedralize(&x.s, &x.o).expect("closed shape");
        let c = m.check_output().expect("check");
        let r = c.difference.abs() / c.bound;
        worst = worst.max(r);
        eprintln!(
            "C7 {}: cells {}, diff {:.3e}, B {:.3e} ({:.3e} of the volume), diff/B {r:.3e}",
            x.name,
            m.cells.len(),
            c.difference,
            c.bound,
            c.bound / c.enclosed_volume
        );
    }
    eprintln!("C7: largest diff/B {worst:.3e}");
}

fn cube_mesh() -> TetMesh {
    tetrahedralize(&cube(2.0, 2), &TetOptions::default()).expect("cube")
}

fn signed_vol(m: &TetMesh, c: [usize; 4]) -> f64 {
    let p = c.map(|v| m.points[v]);
    dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), sub(p[3], p[0])) / 6.0
}

/// The difference printed in an error text, `difference <x>,`.
fn printed_difference(e: &str) -> f64 {
    let s = e.split("difference ").nth(1).expect("difference in text");
    s.split(',').next().unwrap().trim().parse().expect("number")
}

/// C3a NC-dup: a duplicated cell is rejected, located at a face of more than two cells or two cells on one side.
#[test]
fn c3a_duplicated_cell_is_a_located_error() {
    let mut m = cube_mesh();
    m.cells.push(m.cells[0]);
    let e = m.check_output().expect_err("duplicated cell accepted").to_string();
    eprintln!("C3a: {e}");
    assert!(e.contains("tetrahedralize output check:"), "{e}");
    assert!(
        e.contains("is shared by the cells") || e.contains("on the same side"),
        "C3a: not located: {e}"
    );
    let want = signed_vol(&m, m.cells[0]);
    let got = printed_difference(&e);
    assert!(
        (got - want).abs() <= 1e-9 * want.abs(),
        "C3a: difference {got:e} against cell 0 {want:e}"
    );
}

/// C3b NC-flip: a negative cell is rejected and named.
#[test]
fn c3b_negative_cell_is_named() {
    let mut m = cube_mesh();
    let k = m.cells.len() / 2;
    m.cells[k].swap(2, 3);
    let e = m.check_output().expect_err("negative cell accepted").to_string();
    eprintln!("C3b: {e}");
    assert!(
        e.contains(&format!("cell {k} ")) && e.contains("is not positively oriented"),
        "C3b: {e}"
    );
}

/// C3c NC-gap: a missing cell is rejected, located at a free face that is not a surface triangle.
#[test]
fn c3c_missing_cell_is_a_located_error() {
    let mut m = cube_mesh();
    let last = m.cells.pop().unwrap();
    let e = m.check_output().expect_err("missing cell accepted").to_string();
    eprintln!("C3c: {e}");
    assert!(
        e.contains("is free but is not a surface triangle"),
        "C3c: not located: {e}"
    );
    let want = -signed_vol(&m, last);
    let got = printed_difference(&e);
    assert!(
        (got - want).abs() <= 1e-9 * want.abs(),
        "C3c: difference {got:e} against minus the cell {want:e}"
    );
}

/// C3d NC-surf: a surface triangle with a vertex outside the points is a located error, not a panic.
#[test]
fn c3d_surface_vertex_outside_is_a_located_error() {
    let mut m = cube_mesh();
    let n = m.points.len();
    m.surface.tris[3][1] = n;
    let e = m.check_output().expect_err("bad surface accepted").to_string();
    eprintln!("C3d: {e}");
    assert!(e.contains("surface triangle 3 "), "C3d: {e}");
}

/// C3e PC: the valid cube mesh offset and scaled still passes.
#[test]
fn c3e_offset_and_scaled_valid_mesh_passes() {
    type Map = (&'static str, fn([f64; 3]) -> [f64; 3]);
    let base = cube_mesh();
    let maps: [Map; 3] = [
        ("offset", |p| [p[0] + 1e3, p[1] - 2e3, p[2] + 5e2]),
        ("scale 1e-6", |p| [p[0] * 1e-6, p[1] * 1e-6, p[2] * 1e-6]),
        ("scale 1e6", |p| [p[0] * 1e6, p[1] * 1e6, p[2] * 1e6]),
    ];
    for (name, f) in maps {
        let mut m = base.clone();
        for p in m.points.iter_mut() {
            *p = f(*p);
        }
        m.surface.points = m.points.clone();
        let c = m.check_output().unwrap_or_else(|e| panic!("C3e {name}: {e}"));
        eprintln!(
            "C3e {name}: diff {:.3e}, B {:.3e}, diff/B {:.3e}",
            c.difference,
            c.bound,
            c.difference.abs() / c.bound
        );
    }
}

/// C5 (ADR 0231): the check's time on F12 at most 5 percent of the `tetrahedralize` time. Timing; run once by ADR 0231.
#[test]
#[ignore = "timing, run once by ADR 0231 (C5)"]
fn c5_check_cost_on_f12() {
    let x = box_input([1.0, 1.0, 0.1], [10, 10, 1], Some(0.035), 0);
    let t0 = Instant::now();
    let m = tetrahedralize(&x.s, &x.o).expect("F12");
    let t_mesh = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let c = m.check_output().expect("check");
    let t_check = t1.elapsed().as_secs_f64();
    let frac = t_check / t_mesh;
    eprintln!(
        "C5 F12: {} cells, tetrahedralize {t_mesh:.4} s, check {t_check:.6} s, fraction {frac:.3e}, diff/B {:.3e}",
        m.cells.len(),
        c.difference.abs() / c.bound
    );
    assert!(frac <= 0.05, "C5 F12: fraction {frac:.3e}");
}
