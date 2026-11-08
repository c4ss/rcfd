//! ADR 0228 (F-306): the folded edge removal of `tetmesh` behind the neighbour-link panic on a thin box. R1 is the acceptance
//! input, R2 the other inputs of the registered sweep that make a folded removal on main, N1 the fold-free inputs, whose
//! fingerprints were recorded on the library of main 9f95fb12 before any library change. Limits are those of
//! docs/adr/0228-f306-registration-tetmesh-neighbour-link-panic-on-a-thin-box.md, registered before this file was written.

use rcfd_core::vec3::{cross, dot, sub};
use rcfd_mesh::predicates::orient3d;
use rcfd_mesh::surface::{box_surface, cube, cylinder, diagnose, icosphere, torus, DiagOptions, TriSurface};
use rcfd_mesh::tetmesh::{tetrahedralize, TetMesh, TetOptions};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// One input of the registered sweep.
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

const SIZES_A: [Option<f64>; 5] = [None, Some(0.025), Some(0.0333), Some(0.035), Some(0.05)];
const SIZES_B: [Option<f64>; 3] = [None, Some(0.035), Some(0.05)];

fn size_text(size: Option<f64>) -> String {
    size.map_or("none".to_string(), |h| h.to_string())
}

fn box_input(hi: [f64; 3], n: [usize; 3], size: Option<f64>, sweeps: usize) -> Input {
    let name = format!("box {hi:?} {n:?} size {} sweeps {sweeps}", size_text(size));
    input(name, box_surface([0.0; 3], hi, n), size, sweeps)
}

/// N-A of ADR 0228: the ADR 0215 family, 80 inputs.
fn n_a() -> Vec<Input> {
    let mut v = Vec::new();
    for n in [[10, 1, 1], [20, 2, 2], [30, 3, 3], [40, 4, 4]] {
        for size in SIZES_A {
            for sweeps in 0..4 {
                v.push(box_input([1.0, 0.1, 0.1], n, size, sweeps));
            }
        }
    }
    v
}

/// N-B: other boxes, 30 inputs.
fn n_b() -> Vec<Input> {
    let mut v = Vec::new();
    for (hi, n) in [
        ([0.5, 0.12, 0.08], [10, 3, 2]),
        ([0.8, 0.1, 0.12], [16, 2, 3]),
        ([0.3, 0.1, 0.1], [6, 2, 2]),
        ([1.0, 0.05, 0.05], [20, 1, 1]),
        ([1.0, 1.0, 0.1], [10, 10, 1]),
    ] {
        for size in SIZES_B {
            for sweeps in [0, 3] {
                v.push(box_input(hi, n, size, sweeps));
            }
        }
    }
    v
}

/// A named surface constructor.
type Shape = (&'static str, fn() -> TriSurface);

/// N-C: the closed shapes of the existing tests, 14 inputs.
fn n_c() -> Vec<Input> {
    let mut v = Vec::new();
    let shapes: [Shape; 4] = [
        ("cube(2, 2)", || cube(2.0, 2)),
        ("icosphere(1, 2)", || icosphere(1.0, 2)),
        ("torus(2, 0.5, 24, 12)", || torus(2.0, 0.5, 24, 12)),
        ("cylinder(0.5, 1, 16, 4)", || cylinder(0.5, 1.0, 16, 4)),
    ];
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
    v
}

/// F1 to F13 of ADR 0228: the inputs of N-A and N-B that make a folded edge removal on main (F14 is in rcfd-io).
fn folded() -> Vec<(&'static str, Input)> {
    let t = [1.0, 0.1, 0.1];
    let p = [1.0, 1.0, 0.1];
    vec![
        ("F1", box_input(t, [10, 1, 1], Some(0.025), 0)),
        ("F2", box_input(t, [10, 1, 1], Some(0.025), 1)),
        ("F3", box_input(t, [10, 1, 1], Some(0.025), 2)),
        ("F4", box_input(t, [10, 1, 1], Some(0.025), 3)),
        ("F5", box_input(t, [10, 1, 1], Some(0.0333), 0)),
        ("F6", box_input(t, [10, 1, 1], Some(0.0333), 1)),
        ("F7", box_input(t, [10, 1, 1], Some(0.0333), 2)),
        ("F8", box_input(t, [10, 1, 1], Some(0.0333), 3)),
        ("F9", box_input(t, [10, 1, 1], Some(0.035), 1)),
        ("F10", box_input(t, [10, 1, 1], Some(0.035), 2)),
        ("F11", box_input(t, [10, 1, 1], Some(0.035), 3)),
        ("F12", box_input(p, [10, 10, 1], Some(0.035), 0)),
        ("F13", box_input(p, [10, 10, 1], Some(0.035), 3)),
    ]
}

enum Outcome {
    Mesh(TetMesh),
    Error(String),
    Panic(String),
}

/// `tetrahedralize` under `catch_unwind`: a panic of the mesher is an outcome of the input, not of the test binary.
fn run(x: &Input) -> Outcome {
    match catch_unwind(AssertUnwindSafe(|| tetrahedralize(&x.s, &x.o))) {
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

/// V of ADR 0228: (a) every cell positive (exact `orient3d`), (b) the cell volumes sum to the enclosed volume of the surface
/// within `1e-12` relative, (c) `to_mesh` succeeds. `Err` names the failed part.
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

/// The fingerprint of ADR 0228: cell and point counts and FNV-1a 64 over the point bits, the cell vertex lists, the surface
/// triangles and tags and the eight `TetReport` counters.
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

/// R1 and R2: the input returns `Ok` and V holds.
fn check_folded(id: &str) {
    let all = folded();
    let (_, x) = all.iter().find(|(k, _)| *k == id).expect("registered id");
    match run(x) {
        Outcome::Mesh(m) => {
            let (cells, points, hash) = fingerprint(&m);
            let v = validity(&x.s, &m);
            eprintln!(
                "{id} {}: Ok, {cells} cells, {points} points, hash {hash:016x}, report {:?}, V {v:?}",
                x.name, m.report
            );
            assert!(v.is_ok(), "{id} {}: {}", x.name, v.unwrap_err());
        }
        Outcome::Error(e) => panic!("{id} {}: located error instead of a mesh: {e}", x.name),
        Outcome::Panic(p) => panic!("{id} {}: the mesher panicked: {p}", x.name),
    }
}

/// R1 (acceptance, ADR 0228): F11, the input of F-306, returns a valid mesh instead of panicking at `tetmesh.rs:386`.
#[test]
fn r1_f11_thin_box_size_0035_sweeps_3_returns_a_valid_mesh() {
    check_folded("F11");
}

#[test]
fn r2_f1_thin_box_size_0025_sweeps_0() {
    check_folded("F1");
}

#[test]
fn r2_f2_thin_box_size_0025_sweeps_1() {
    check_folded("F2");
}

#[test]
fn r2_f3_thin_box_size_0025_sweeps_2() {
    check_folded("F3");
}

#[test]
fn r2_f4_thin_box_size_0025_sweeps_3() {
    check_folded("F4");
}

#[test]
fn r2_f5_thin_box_size_00333_sweeps_0() {
    check_folded("F5");
}

#[test]
fn r2_f6_thin_box_size_00333_sweeps_1() {
    check_folded("F6");
}

#[test]
fn r2_f7_thin_box_size_00333_sweeps_2() {
    check_folded("F7");
}

#[test]
fn r2_f8_thin_box_size_00333_sweeps_3() {
    check_folded("F8");
}

#[test]
fn r2_f9_thin_box_size_0035_sweeps_1() {
    check_folded("F9");
}

#[test]
fn r2_f10_thin_box_size_0035_sweeps_2() {
    check_folded("F10");
}

#[test]
fn r2_f12_plate_size_0035_sweeps_0() {
    check_folded("F12");
}

#[test]
fn r2_f13_plate_size_0035_sweeps_3() {
    check_folded("F13");
}

/// The N1 record: `(name, cells, points, hash)` of every fold-free input of N-A, N-B and N-C in sweep order, printed by the
/// library of main 9f95fb12 (Windows) before any library change of F-306.
const RECORD: &[(&str, usize, usize, u64)] = &[
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size none sweeps 0",
        252,
        102,
        0x4a39c977075663bb,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size none sweeps 1",
        252,
        102,
        0x0483b3b5e445a41c,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size none sweeps 2",
        252,
        102,
        0x0483b3b5e445a41c,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size none sweeps 3",
        252,
        102,
        0x0483b3b5e445a41c,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size 0.035 sweeps 0",
        1055,
        249,
        0x96d84c78f1a7a492,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size 0.05 sweeps 0",
        342,
        117,
        0xb00ea04e8b6866a4,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size 0.05 sweeps 1",
        376,
        124,
        0xcbb07c13d0215c88,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size 0.05 sweeps 2",
        397,
        128,
        0x25f3fdd940e4a460,
    ),
    (
        "box [1.0, 0.1, 0.1] [10, 1, 1] size 0.05 sweeps 3",
        434,
        135,
        0xa9771b600592fec9,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size none sweeps 0",
        1151,
        441,
        0x5b88f2064e5c5503,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size none sweeps 1",
        1118,
        441,
        0xb9aa2b7fd142dfda,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size none sweeps 2",
        1118,
        441,
        0xc56133d27c1e785f,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size none sweeps 3",
        1118,
        441,
        0xc56133d27c1e785f,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.025 sweeps 0",
        3095,
        733,
        0x85ba723997a844b9,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.025 sweeps 1",
        3018,
        742,
        0x0494c153bd1655cc,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.025 sweeps 2",
        3259,
        783,
        0xbff0efc36ee3040a,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.025 sweeps 3",
        3381,
        805,
        0x95bfbecd9b8cdd4b,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.0333 sweeps 0",
        1760,
        524,
        0xe56509647fc34278,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.0333 sweeps 1",
        1734,
        525,
        0xc9fb5072aa1081b9,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.0333 sweeps 2",
        1773,
        532,
        0xe097076571ea9f7b,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.0333 sweeps 3",
        1773,
        532,
        0x61ea4427de85f890,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.035 sweeps 0",
        1667,
        511,
        0x3ba4653b0e8a3234,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.035 sweeps 1",
        1643,
        511,
        0x2ac72d31bac14b25,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.035 sweeps 2",
        1707,
        521,
        0x58c0cfaff10d0f69,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.035 sweeps 3",
        1711,
        522,
        0x6431fe29df465167,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.05 sweeps 0",
        1238,
        450,
        0xf2e180be8cd8b5a7,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.05 sweeps 1",
        1219,
        450,
        0xee1ec90ab42ad73b,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.05 sweeps 2",
        1219,
        450,
        0xa2059030a8dc16ec,
    ),
    (
        "box [1.0, 0.1, 0.1] [20, 2, 2] size 0.05 sweeps 3",
        1219,
        450,
        0xcea79a9263b38d6a,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size none sweeps 0",
        2921,
        1049,
        0xb39539510a77e35b,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size none sweeps 1",
        2864,
        1052,
        0x197257fa1c629439,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size none sweeps 2",
        2869,
        1054,
        0x7f67c8070e334dd0,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size none sweeps 3",
        2884,
        1056,
        0xbe7485ba234b5329,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.025 sweeps 0",
        4377,
        1247,
        0x940dbc953c1e2ebb,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.025 sweeps 1",
        4278,
        1251,
        0x1cfc7bbc9177dbea,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.025 sweeps 2",
        4384,
        1268,
        0x30e6b98cfed2e675,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.025 sweeps 3",
        4446,
        1278,
        0xb4e8a9f22c858afc,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.0333 sweeps 0",
        3266,
        1084,
        0xf97afee4108f4c64,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.0333 sweeps 1",
        3268,
        1094,
        0x829dab863a8aaa6b,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.0333 sweeps 2",
        3448,
        1118,
        0xdec1fc1547d9fdb6,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.0333 sweeps 3",
        3474,
        1124,
        0x182e217dd336d68f,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.035 sweeps 0",
        3127,
        1070,
        0xf57179c4e6c0d497,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.035 sweeps 1",
        3172,
        1085,
        0x4473d1837e1ef106,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.035 sweeps 2",
        3283,
        1099,
        0xc1a8b9da85f30839,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.035 sweeps 3",
        3304,
        1103,
        0x553d9b9cea87cde0,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.05 sweeps 0",
        2944,
        1051,
        0xeaba9bf9b5ac87b5,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.05 sweeps 1",
        2869,
        1052,
        0x033aad8465d9289a,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.05 sweeps 2",
        2878,
        1054,
        0x5402184f6d2f42bc,
    ),
    (
        "box [1.0, 0.1, 0.1] [30, 3, 3] size 0.05 sweeps 3",
        2886,
        1055,
        0x761f98d370469e4d,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size none sweeps 0",
        6399,
        2139,
        0x593577a88adbfb1a,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size none sweeps 1",
        6352,
        2158,
        0x0f367d5247dd9893,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size none sweeps 2",
        6400,
        2167,
        0x72117a9f07846ee1,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size none sweeps 3",
        6414,
        2169,
        0x3b0abef34c2590dc,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.025 sweeps 0",
        6819,
        2189,
        0x3b15e2afbd5580ea,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.025 sweeps 1",
        6729,
        2203,
        0xedbe9028a50f1ca8,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.025 sweeps 2",
        7016,
        2246,
        0x7aa0d5928993272d,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.025 sweeps 3",
        7141,
        2267,
        0x4f1262b1d900ae63,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.0333 sweeps 0",
        6535,
        2155,
        0x9883c460a212b04a,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.0333 sweeps 1",
        6413,
        2165,
        0x13c990ae7c5b26d8,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.0333 sweeps 2",
        6469,
        2176,
        0x074a0c012527f5c4,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.0333 sweeps 3",
        6474,
        2178,
        0xddfaf7bf0f6c63b6,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.035 sweeps 0",
        6429,
        2141,
        0x049ba1bdd395a843,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.035 sweeps 1",
        6340,
        2154,
        0x4299a33fa9071c02,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.035 sweeps 2",
        6393,
        2164,
        0xba5b7fc941238a90,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.035 sweeps 3",
        6416,
        2168,
        0x346643d6038119fc,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.05 sweeps 0",
        6429,
        2141,
        0x049ba1bdd395a843,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.05 sweeps 1",
        6340,
        2154,
        0x4299a33fa9071c02,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.05 sweeps 2",
        6374,
        2160,
        0x8855301e9da9769d,
    ),
    (
        "box [1.0, 0.1, 0.1] [40, 4, 4] size 0.05 sweeps 3",
        6384,
        2162,
        0xcc0f81c5cc5db5c7,
    ),
    (
        "box [0.5, 0.12, 0.08] [10, 3, 2] size none sweeps 0",
        837,
        302,
        0x1319f11701718633,
    ),
    (
        "box [0.5, 0.12, 0.08] [10, 3, 2] size none sweeps 3",
        816,
        302,
        0x84f02010cee88632,
    ),
    (
        "box [0.5, 0.12, 0.08] [10, 3, 2] size 0.035 sweeps 0",
        913,
        315,
        0xb5fa8a7d7f6a7df2,
    ),
    (
        "box [0.5, 0.12, 0.08] [10, 3, 2] size 0.035 sweeps 3",
        924,
        320,
        0xa52d572aeb29c871,
    ),
    (
        "box [0.5, 0.12, 0.08] [10, 3, 2] size 0.05 sweeps 0",
        837,
        302,
        0x1319f11701718633,
    ),
    (
        "box [0.5, 0.12, 0.08] [10, 3, 2] size 0.05 sweeps 3",
        816,
        302,
        0x84f02010cee88632,
    ),
    (
        "box [0.8, 0.1, 0.12] [16, 2, 3] size none sweeps 0",
        1336,
        469,
        0x814a89ac053a5cf5,
    ),
    (
        "box [0.8, 0.1, 0.12] [16, 2, 3] size none sweeps 3",
        1324,
        472,
        0xdfbd7cdf24a8504c,
    ),
    (
        "box [0.8, 0.1, 0.12] [16, 2, 3] size 0.035 sweeps 0",
        1685,
        519,
        0x4d4937865e342bd6,
    ),
    (
        "box [0.8, 0.1, 0.12] [16, 2, 3] size 0.035 sweeps 3",
        1742,
        538,
        0xba8272a0c9b8f3c1,
    ),
    (
        "box [0.8, 0.1, 0.12] [16, 2, 3] size 0.05 sweeps 0",
        1323,
        467,
        0x906f53b20a86c579,
    ),
    (
        "box [0.8, 0.1, 0.12] [16, 2, 3] size 0.05 sweeps 3",
        1315,
        471,
        0xbb10d6be048b1b07,
    ),
    (
        "box [0.3, 0.1, 0.1] [6, 2, 2] size none sweeps 0",
        363,
        147,
        0x5a2e81ecb9d814c6,
    ),
    (
        "box [0.3, 0.1, 0.1] [6, 2, 2] size none sweeps 3",
        360,
        147,
        0x1d46878ac371f5e7,
    ),
    (
        "box [0.3, 0.1, 0.1] [6, 2, 2] size 0.035 sweeps 0",
        511,
        166,
        0xe34eb4a1fdd7476e,
    ),
    (
        "box [0.3, 0.1, 0.1] [6, 2, 2] size 0.035 sweeps 3",
        542,
        172,
        0x467d7d216badf48e,
    ),
    (
        "box [0.3, 0.1, 0.1] [6, 2, 2] size 0.05 sweeps 0",
        404,
        150,
        0x111f7e4dc0119829,
    ),
    (
        "box [0.3, 0.1, 0.1] [6, 2, 2] size 0.05 sweeps 3",
        398,
        150,
        0x5832efe60bf923a4,
    ),
    (
        "box [1.0, 0.05, 0.05] [20, 1, 1] size none sweeps 0",
        500,
        198,
        0xb57e5509a91a165b,
    ),
    (
        "box [1.0, 0.05, 0.05] [20, 1, 1] size none sweeps 3",
        500,
        198,
        0xf4cf0984069a6bf4,
    ),
    (
        "box [1.0, 0.05, 0.05] [20, 1, 1] size 0.035 sweeps 0",
        544,
        209,
        0x0a6cb064015f38da,
    ),
    (
        "box [1.0, 0.05, 0.05] [20, 1, 1] size 0.035 sweeps 3",
        544,
        209,
        0x16c000006b6a286d,
    ),
    (
        "box [1.0, 0.05, 0.05] [20, 1, 1] size 0.05 sweeps 0",
        500,
        198,
        0xb57e5509a91a165b,
    ),
    (
        "box [1.0, 0.05, 0.05] [20, 1, 1] size 0.05 sweeps 3",
        500,
        198,
        0xf4cf0984069a6bf4,
    ),
    (
        "box [1.0, 1.0, 0.1] [10, 10, 1] size none sweeps 0",
        1808,
        626,
        0xfd820f3bd91309c0,
    ),
    (
        "box [1.0, 1.0, 0.1] [10, 10, 1] size none sweeps 3",
        1768,
        626,
        0xffafca827f83c306,
    ),
    (
        "box [1.0, 1.0, 0.1] [10, 10, 1] size 0.05 sweeps 0",
        3915,
        975,
        0x2915bd53402bf241,
    ),
    (
        "box [1.0, 1.0, 0.1] [10, 10, 1] size 0.05 sweeps 3",
        4644,
        1116,
        0xf244ebae45f44cae,
    ),
    ("cube(2, 2) size none sweeps 0", 108, 50, 0x74f754fa29f615a6),
    ("cube(2, 2) size none sweeps 1", 108, 50, 0x74f754fa29f615a6),
    ("cube(2, 2) size none sweeps 3", 108, 50, 0x74f754fa29f615a6),
    ("icosphere(1, 2) size none sweeps 0", 320, 163, 0xded84bdaf9aeb558),
    ("icosphere(1, 2) size none sweeps 1", 320, 163, 0xded84bdaf9aeb558),
    ("icosphere(1, 2) size none sweeps 3", 320, 163, 0xded84bdaf9aeb558),
    ("torus(2, 0.5, 24, 12) size none sweeps 0", 899, 337, 0xf7b20559549fd7cf),
    ("torus(2, 0.5, 24, 12) size none sweeps 1", 945, 348, 0x9ca4df515d62083b),
    ("torus(2, 0.5, 24, 12) size none sweeps 3", 945, 348, 0x3709c4b32a8c4ca6),
    (
        "cylinder(0.5, 1, 16, 4) size none sweeps 0",
        224,
        115,
        0xe612bbae87857587,
    ),
    (
        "cylinder(0.5, 1, 16, 4) size none sweeps 1",
        224,
        115,
        0x667e64a55b345a3e,
    ),
    (
        "cylinder(0.5, 1, 16, 4) size none sweeps 3",
        224,
        115,
        0xf4b3a87dd01a75a9,
    ),
    ("icosphere(1, 2) size 0.25 sweeps 0", 1277, 299, 0x6e38d607b666baa2),
    ("cube(2, 4) size 0.5 sweeps 0", 597, 208, 0x25cdcce0d5813cbf),
];

/// N1 over the inputs of one group (`"A10"`, `"A20"`, `"A30"`, `"A40"`, `"B"`, `"C"`): every fold-free input gives the recorded
/// fingerprint (Windows) and passes V (every platform). Prints a record line per input.
fn n1(group: &str) {
    let folded_names: Vec<String> = folded().into_iter().map(|(_, x)| x.name).collect();
    let all: Vec<(String, Input)> = n_a()
        .into_iter()
        .enumerate()
        .map(|(i, x)| (format!("A{}", [10, 20, 30, 40][i / 20]), x))
        .chain(n_b().into_iter().map(|x| ("B".to_string(), x)))
        .chain(n_c().into_iter().map(|x| ("C".to_string(), x)))
        .collect();
    let fold_free: Vec<(String, Input)> = all
        .into_iter()
        .filter(|(_, x)| !folded_names.contains(&x.name))
        .collect();
    assert_eq!(fold_free.len(), 111, "N1 has 111 fold-free inputs");
    assert_eq!(RECORD.len(), fold_free.len(), "one record per fold-free input");
    let mut bad = Vec::new();
    let mut ran = 0;
    for (k, (g, x)) in fold_free.iter().enumerate() {
        if g != group {
            continue;
        }
        ran += 1;
        let got = match run(x) {
            Outcome::Mesh(m) => {
                if let Err(e) = validity(&x.s, &m) {
                    bad.push(format!("{}: {e}", x.name));
                }
                fingerprint(&m)
            }
            Outcome::Error(e) => {
                bad.push(format!("{}: error {e}", x.name));
                continue;
            }
            Outcome::Panic(p) => {
                bad.push(format!("{}: panic {p}", x.name));
                continue;
            }
        };
        eprintln!("    (\"{}\", {}, {}, 0x{:016x}),", x.name, got.0, got.1, got.2);
        if cfg!(windows) {
            let r = RECORD[k];
            assert_eq!(r.0, x.name, "record order");
            if (r.1, r.2, r.3) != got {
                bad.push(format!(
                    "{}: fingerprint {got:?} against the record {:?}",
                    x.name,
                    (r.1, r.2, r.3)
                ));
            }
        }
    }
    assert!(ran > 0, "group {group} is empty");
    if !cfg!(windows) {
        eprintln!("N1 {group}: no recorded fingerprints for this platform (ADR 0228 Q6); V only");
    }
    assert!(
        bad.is_empty(),
        "N1 {group}: {} of {ran} inputs fail:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn n1_a10_fold_free_thin_boxes_10_1_1_bit_identical() {
    n1("A10");
}

#[test]
fn n1_a20_fold_free_thin_boxes_20_2_2_bit_identical() {
    n1("A20");
}

#[test]
fn n1_a30_fold_free_thin_boxes_30_3_3_bit_identical() {
    n1("A30");
}

#[test]
fn n1_a40_fold_free_thin_boxes_40_4_4_bit_identical() {
    n1("A40");
}

#[test]
fn n1_b_fold_free_other_boxes_bit_identical() {
    n1("B");
}

#[test]
fn n1_c_fold_free_closed_shapes_bit_identical() {
    n1("C");
}
