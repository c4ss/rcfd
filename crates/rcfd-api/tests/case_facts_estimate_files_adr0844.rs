//! ADR 0844 (F-989): `case.facts` refuses by `mesh_preview::cell_estimate` for mesh files (Gmsh, Fluent, OpenFOAM) and
//! for the generators `estimate` does not name, before it builds the mesh, with the reply of `FactsError::TooLarge`.
//!
//! a1, a2: the estimate equals the registered count (route B) and the built count (route A). a3: at the built count the
//! case is not refused, one below it is refused with that count. a4: the service's reply. n1: refused before a build
//! (files that cannot be built). n2: no estimate where the reader refuses the file. Heavy, ignored: `p0_` writes the
//! over-limit files; `rN_` the refusal alone and `hN_` the refusal and the build, timed (ADR 0812's limits).

use rcfd_api::case::Case;
use rcfd_api::facts::{self, FactsError, InText};
use rcfd_api::mesh_preview::cell_estimate;
use rcfd_api::service::{Service, ServiceConfig};
use rcfd_api::sim;
use rcfd_mesh::gen::{self, Grading};
use rcfd_mesh::Mesh;
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd-adr0844-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn file_case(path: &Path) -> Case {
    let mut case = Case::new();
    case.set("mesh.source", json!("file")).unwrap();
    case.set("mesh.file", json!(path.display().to_string())).unwrap();
    case
}

fn gen_case(g: &str, extra: &[(&str, Value)]) -> Case {
    let mut case = Case::new();
    case.set("mesh.generator", json!(g)).unwrap();
    case.set("mesh.nx", json!(8)).unwrap();
    case.set("mesh.ny", json!(4)).unwrap();
    for (k, v) in extra {
        case.set(k, v.clone()).unwrap();
    }
    case
}

fn shipped(rel: &str, extra: &[(&str, Value)]) -> Case {
    let mut case = Case::load(repo().join(rel)).unwrap();
    for (k, v) in extra {
        case.set(k, v.clone()).unwrap();
    }
    case
}

fn built(case: &Case) -> u64 {
    sim::build_mesh(case).unwrap().n_cells() as u64
}

// ------------------------------------------------------------------ writers of the test's files

/// Fluent ASCII (3D) or binary-section file of `mesh` (all faces quads): nodes, cells, the interior faces and one wall
/// zone per patch.
fn fluent_3d(mesh: &Mesh, binary: bool) -> Vec<u8> {
    let (np, nc, nf, ni) = (mesh.n_points(), mesh.n_cells(), mesh.n_faces(), mesh.n_internal_faces());
    let mut b: Vec<u8> = Vec::new();
    write!(b, "(0 \"rcfd ADR 0844 (test)\")\n(2 3)\n(10 (0 1 {np:x} 0 3))\n").unwrap();
    let face = |f: usize| -> [usize; 6] {
        let p = mesh.face_points(f);
        assert_eq!(p.len(), 4);
        let c1 = if f < ni { mesh.neighbour()[f] + 1 } else { 0 };
        [p[0] + 1, p[1] + 1, p[2] + 1, p[3] + 1, mesh.owner()[f] + 1, c1]
    };
    let mut zones: Vec<(usize, usize, usize, usize)> = vec![(3, 0, ni, 2)];
    for (z, p) in mesh.patches().iter().enumerate() {
        zones.push((4 + z, p.start, p.start + p.size, 3));
    }
    if binary {
        write!(b, "(3010 (1 1 {np:x} 1 3)(").unwrap();
        for p in mesh.points() {
            for x in p {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        b.extend_from_slice(b"\nEnd of Binary Section 3010))\n");
        write!(
            b,
            "(12 (0 1 {nc:x} 0))\n(3012 (2 1 {nc:x} 1 4))\n(13 (0 1 {nf:x} 0 0))\n"
        )
        .unwrap();
        for (z, s, e, bc) in zones {
            write!(b, "(3013 ({z:x} {:x} {:x} {bc:x} 4)(", s + 1, e).unwrap();
            for f in s..e {
                for v in face(f) {
                    b.extend_from_slice(&(v as i32).to_le_bytes());
                }
            }
            b.extend_from_slice(b"End of Binary Section 3013))\n");
        }
    } else {
        let mut s = format!("(10 (1 1 {np:x} 1 3)(\n");
        for p in mesh.points() {
            let _ = writeln!(s, "{:e} {:e} {:e}", p[0], p[1], p[2]);
        }
        let _ = write!(
            s,
            "))\n(12 (0 1 {nc:x} 0))\n(12 (2 1 {nc:x} 1 4))\n(13 (0 1 {nf:x} 0 0))\n"
        );
        b.extend_from_slice(s.as_bytes());
        for (z, st, e, bc) in zones {
            let mut s = format!("(13 ({z:x} {:x} {:x} {bc:x} 4)(\n", st + 1, e);
            for f in st..e {
                let v = face(f);
                let _ = writeln!(s, "{:x} {:x} {:x} {:x} {:x} {:x}", v[0], v[1], v[2], v[3], v[4], v[5]);
            }
            s.push_str("))\n");
            b.extend_from_slice(s.as_bytes());
        }
    }
    b
}

/// Fluent ASCII 2D: a 3 x 2 grid of unit quads, two-node faces (W3, 6 cells).
fn fluent_2d() -> String {
    let (nx, ny) = (3usize, 2usize);
    let node = |i: usize, j: usize| 1 + i + (nx + 1) * j;
    let cell = |i: usize, j: usize| 1 + i + nx * j;
    let mut inner = Vec::new();
    let mut wall = Vec::new();
    for j in 0..ny {
        for i in 0..=nx {
            let (a, b) = (node(i, j), node(i, j + 1));
            match (i > 0, i < nx) {
                (true, true) => inner.push([a, b, cell(i - 1, j), cell(i, j)]),
                (false, _) => wall.push([a, b, cell(i, j), 0]),
                (_, false) => wall.push([a, b, cell(i - 1, j), 0]),
            }
        }
    }
    for j in 0..=ny {
        for i in 0..nx {
            let (a, b) = (node(i, j), node(i + 1, j));
            match (j > 0, j < ny) {
                (true, true) => inner.push([a, b, cell(i, j), cell(i, j - 1)]),
                (false, _) => wall.push([a, b, cell(i, j), 0]),
                (_, false) => wall.push([a, b, cell(i, j - 1), 0]),
            }
        }
    }
    let np = (nx + 1) * (ny + 1);
    let mut s = format!("(0 \"rcfd ADR 0844 2D (test)\")\n(2 2)\n(10 (0 1 {np:x} 0 2))\n(10 (1 1 {np:x} 1 2)(\n");
    for j in 0..=ny {
        for i in 0..=nx {
            let _ = writeln!(s, "{} {}", i as f64, j as f64);
        }
    }
    let nc = nx * ny;
    let nf = inner.len() + wall.len();
    let _ = write!(
        s,
        "))\n(12 (0 1 {nc:x} 0))\n(12 (2 1 {nc:x} 1 3))\n(13 (0 1 {nf:x} 0 0))\n"
    );
    let mut first = 1;
    for (z, bc, list) in [(3, 2, &inner), (4, 3, &wall)] {
        let _ = writeln!(s, "(13 ({z:x} {first:x} {:x} {bc:x} 2)(", first + list.len() - 1);
        for f in list.iter() {
            let _ = writeln!(s, "{:x} {:x} {:x} {:x}", f[0], f[1], f[2], f[3]);
        }
        s.push_str("))\n");
        first += list.len();
    }
    s
}

/// A structured `n`^3 grid of unit hexahedra as Gmsh 2.2 ASCII or 4.1 binary, with the six sides as quads.
fn gmsh_grid(n: usize, path: &Path, v41_binary: bool) {
    let np1 = n + 1;
    let id = |i: usize, j: usize, k: usize| 1 + i + np1 * (j + np1 * k);
    let hex = |i: usize, j: usize, k: usize| {
        [
            id(i, j, k),
            id(i + 1, j, k),
            id(i + 1, j + 1, k),
            id(i, j + 1, k),
            id(i, j, k + 1),
            id(i + 1, j, k + 1),
            id(i + 1, j + 1, k + 1),
            id(i, j + 1, k + 1),
        ]
    };
    // the six sides: (entity tag, quads)
    let mut sides: Vec<(i32, Vec<[usize; 4]>)> = Vec::new();
    for (tag, axis, at) in [(1, 0, 0), (2, 0, n), (3, 1, 0), (4, 1, n), (5, 2, 0), (6, 2, n)] {
        let mut q = Vec::with_capacity(n * n);
        for b in 0..n {
            for a in 0..n {
                let p = |da: usize, db: usize| match axis {
                    0 => id(at, a + da, b + db),
                    1 => id(a + da, at, b + db),
                    _ => id(a + da, b + db, at),
                };
                q.push([p(0, 0), p(1, 0), p(1, 1), p(0, 1)]);
            }
        }
        sides.push((tag, q));
    }
    let f = std::fs::File::create(path).unwrap();
    let mut w = std::io::BufWriter::with_capacity(1 << 20, f);
    let n_nodes = np1 * np1 * np1;
    let n_quads = 6 * n * n;
    let n_hex = n * n * n;
    if !v41_binary {
        write!(w, "$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$Nodes\n{n_nodes}\n").unwrap();
        for k in 0..np1 {
            for j in 0..np1 {
                for i in 0..np1 {
                    writeln!(w, "{} {i} {j} {k}", id(i, j, k)).unwrap();
                }
            }
        }
        write!(w, "$EndNodes\n$Elements\n{}\n", n_quads + n_hex).unwrap();
        let mut e = 1;
        for (tag, q) in &sides {
            for p in q {
                writeln!(w, "{e} 3 2 {tag} {tag} {} {} {} {}", p[0], p[1], p[2], p[3]).unwrap();
                e += 1;
            }
        }
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let h = hex(i, j, k);
                    writeln!(
                        w,
                        "{e} 5 2 9 1 {} {} {} {} {} {} {} {}",
                        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
                    )
                    .unwrap();
                    e += 1;
                }
            }
        }
        w.write_all(b"$EndElements\n").unwrap();
    } else {
        let sz = |w: &mut std::io::BufWriter<std::fs::File>, v: usize| w.write_all(&(v as u64).to_le_bytes()).unwrap();
        let i4 = |w: &mut std::io::BufWriter<std::fs::File>, v: i32| w.write_all(&v.to_le_bytes()).unwrap();
        w.write_all(b"$MeshFormat\n4.1 1 8\n").unwrap();
        i4(&mut w, 1);
        w.write_all(b"\n$EndMeshFormat\n$Nodes\n").unwrap();
        for v in [1, n_nodes, 1, n_nodes] {
            sz(&mut w, v);
        }
        i4(&mut w, 3);
        i4(&mut w, 1);
        i4(&mut w, 0);
        sz(&mut w, n_nodes);
        for t in 1..=n_nodes {
            sz(&mut w, t);
        }
        for k in 0..np1 {
            for j in 0..np1 {
                for i in 0..np1 {
                    for x in [i as f64, j as f64, k as f64] {
                        w.write_all(&x.to_le_bytes()).unwrap();
                    }
                }
            }
        }
        w.write_all(b"\n$EndNodes\n$Elements\n").unwrap();
        for v in [7, n_quads + n_hex, 1, n_quads + n_hex] {
            sz(&mut w, v);
        }
        let mut e = 1;
        for (tag, q) in &sides {
            i4(&mut w, 2);
            i4(&mut w, *tag);
            i4(&mut w, 3);
            sz(&mut w, q.len());
            for p in q {
                sz(&mut w, e);
                e += 1;
                for &x in p {
                    sz(&mut w, x);
                }
            }
        }
        i4(&mut w, 3);
        i4(&mut w, 1);
        i4(&mut w, 5);
        sz(&mut w, n_hex);
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    sz(&mut w, e);
                    e += 1;
                    for x in hex(i, j, k) {
                        sz(&mut w, x);
                    }
                }
            }
        }
        w.write_all(b"\n$EndElements\n").unwrap();
    }
    w.flush().unwrap();
}

fn box_mesh(n: [usize; 3]) -> Mesh {
    gen::box_mesh(n, [1.0, 1.0, 1.0], [Grading::Uniform; 3]).unwrap()
}

// ------------------------------------------------------------------ the registered set

/// R1 to R14 and W1 to W3 with their registered counts (route B).
fn files() -> (Vec<(String, Case, u64)>, PathBuf) {
    files_in("files")
}

/// As [`files`], with the test's files in a folder of its own (ADR 0844 amendment 1).
fn files_in(folder: &str) -> (Vec<(String, Case, u64)>, PathBuf) {
    let mut out = Vec::new();
    let data = repo().join("crates/rcfd-io/tests/data");
    for (stem, n) in [("hex", 64), ("prism", 336), ("tet", 1139)] {
        for v in ["v2.2_ascii", "v2.2_bin", "v4.1_ascii", "v4.1_bin"] {
            let p = data.join(format!("{stem}_{v}.msh"));
            out.push((format!("{stem}_{v}"), file_case(&p), n));
        }
    }
    for root in ["example-projects", "example-projects-linux"] {
        let p = repo().join(root).join("wind-tunnel/meshes/naca0012-ogrid");
        out.push((format!("{root} naca0012-ogrid"), file_case(&p), 6144));
    }
    let d = tmp(folder);
    let m = box_mesh([3, 4, 5]);
    for (name, bin) in [("W1 fluent ascii", false), ("W2 fluent binary", true)] {
        let p = d.join(format!("w{}.msh", u8::from(bin) + 1));
        std::fs::write(&p, fluent_3d(&m, bin)).unwrap();
        out.push((name.into(), file_case(&p), m.n_cells() as u64));
    }
    let p = d.join("w3.msh");
    std::fs::write(&p, fluent_2d()).unwrap();
    out.push(("W3 fluent 2d".into(), file_case(&p), 6));
    (out, d)
}

const FAN_H: &str = "cases/axial_fan_sector/axial_fan_sector.toml";
const FAN_C: &str = "cases/axial_fan_cgrid/axial_fan_cgrid.toml";

/// G1 to G11 with their registered counts (route B, the formulas worked by hand in the ADR).
fn generators() -> Vec<(&'static str, Case, u64)> {
    let sector = [
        ("mesh.r_in", json!(1)),
        ("mesh.r_out", json!(3)),
        ("mesh.r_interface", json!(2)),
        ("mesh.nz", json!(2)),
        ("mesh.sector_angle", json!(30)),
    ];
    let mut sector_right = sector.to_vec();
    sector_right.push(("mesh.ny_right", json!(3)));
    let no_blade = [("mesh.fan.include_blade", json!(false))];
    vec![
        (
            "G1 cylinder_ogrid",
            gen_case("cylinder_ogrid", &[("mesh.r_in", json!(1)), ("mesh.r_out", json!(4))]),
            32,
        ),
        (
            "G2 pipe_sweep",
            gen_case("pipe_sweep", &[("mesh.r_out", json!(1)), ("mesh.nz", json!(2))]),
            384,
        ),
        ("G3 box_3d", gen_case("box_3d", &[("mesh.nz", json!(2))]), 64),
        (
            "G4 box_3d rectangles",
            gen_case(
                "box_3d",
                &[("mesh.nz", json!(2)), ("mesh.box_3d.section", json!("rectangles"))],
            ),
            64,
        ),
        (
            "G5 box_3d triangles",
            gen_case(
                "box_3d",
                &[("mesh.nz", json!(2)), ("mesh.box_3d.section", json!("triangles"))],
            ),
            128,
        ),
        (
            "G6 annulus_3d_sector_cylindrical_interface",
            gen_case("annulus_3d_sector_cylindrical_interface", &sector),
            128,
        ),
        (
            "G7 annulus_3d_sector_cylindrical_interface ny_right 3",
            gen_case("annulus_3d_sector_cylindrical_interface", &sector_right),
            112,
        ),
        ("G8 axial_fan_sector h-grid", shipped(FAN_H, &[]), 13_332),
        ("G9 axial_fan_sector h-grid no blade", shipped(FAN_H, &no_blade), 13_524),
        ("G10 axial_fan_sector c-grid", shipped(FAN_C, &[]), 11_536),
        (
            "G11 axial_fan_sector c-grid no blade",
            shipped(FAN_C, &no_blade),
            11_760,
        ),
    ]
}

fn check_two_routes(what: &str, case: &Case, registered: u64) {
    let est = cell_estimate(case);
    let n = built(case);
    eprintln!("{what}: estimate {est:?}, registered {registered}, built {n}");
    assert_eq!(est, Some(registered), "{what}: route B (registered count)");
    assert_eq!(est, Some(n), "{what}: route A (built mesh)");
}

/// K1.
#[test]
fn a1_files_by_two_routes() {
    let (set, d) = files();
    for (what, case, n) in &set {
        check_two_routes(what, case, *n);
    }
    let _ = std::fs::remove_dir_all(d);
}

/// K2.
#[test]
fn a2_generators_by_two_routes() {
    for (what, case, n) in &generators() {
        check_two_routes(what, case, *n);
    }
}

/// K3: not refused at the built count, refused one below it with the built count.
#[test]
#[ignore = "ADR 0844 amendment 1: a defective set-up (its folder is shared with a1_ and deleted by it); replaced by a3b_"]
fn a3_at_and_below_the_built_count() {
    let (mut set, d) = files();
    set.extend(generators().into_iter().map(|(w, c, n)| (w.to_string(), c, n)));
    for (what, case, _) in &set {
        let n = built(case);
        match facts::case_facts(case, InText::NotDefault, Some(n)) {
            Ok(f) => assert_eq!(f.cells, n, "{what}"),
            Err(FactsError::TooLarge { cells, limit }) => panic!("{what}: refused at the limit ({cells} > {limit})"),
            Err(e) => eprintln!("{what}: at the limit, not refused; the build after it: {e:?}"),
        }
        match facts::case_facts(case, InText::NotDefault, Some(n - 1)) {
            Err(FactsError::TooLarge { cells, limit }) => {
                assert_eq!((cells, limit), (n, n - 1), "{what}")
            }
            other => panic!("{what}: below the limit: {other:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

/// K3 (ADR 0844 amendment 1): not refused at the built count, refused one below it with the built count.
#[test]
fn a3b_at_and_below_the_built_count() {
    let (mut set, d) = files_in("a3b");
    set.extend(generators().into_iter().map(|(w, c, n)| (w.to_string(), c, n)));
    for (what, case, _) in &set {
        let n = built(case);
        match facts::case_facts(case, InText::NotDefault, Some(n)) {
            Ok(f) => assert_eq!(f.cells, n, "{what}"),
            Err(FactsError::TooLarge { cells, limit }) => panic!("{what}: refused at the limit ({cells} > {limit})"),
            Err(e) => eprintln!("{what}: at the limit, not refused; the build after it: {e:?}"),
        }
        match facts::case_facts(case, InText::NotDefault, Some(n - 1)) {
            Err(FactsError::TooLarge { cells, limit }) => {
                assert_eq!((cells, limit), (n, n - 1), "{what}")
            }
            other => panic!("{what}: below the limit: {other:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

/// K3: the service's reply.
#[test]
fn a4_the_service_reply() {
    let cfg = ServiceConfig {
        max_cells: 20,
        ..ServiceConfig::default()
    };
    let svc = Service::new(cfg);
    svc.enable_ui();
    let call = |method: &str, params: Value| {
        svc.handle_request(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
            .unwrap()
    };
    for (what, case, _) in &generators() {
        let settings: Value = serde_json::from_str(&case.to_json_string()).unwrap();
        let r = call("session.create", json!({"settings": settings}));
        let id = r["result"]["session"]
            .as_str()
            .unwrap_or_else(|| panic!("{what}: {r}"))
            .to_string();
        let reply = call("case.facts", json!({"session": id}));
        assert_eq!(reply["error"]["code"], 1004, "{what}: {reply}");
        assert_eq!(
            reply["error"]["message"].as_str(),
            Some(format!("mesh has {} cells, limit is 20", built(case)).as_str()),
            "{what}"
        );
    }
}

/// Negative control: files that cannot be built are refused by their estimate, so the refusal came before any build.
#[test]
fn n1_refused_before_a_build() {
    let d = tmp("n1");
    let mut s = String::from("$MeshFormat\n2.2 0 8\n$EndMeshFormat\n$Elements\n60\n");
    for e in 1..=60 {
        let _ = writeln!(s, "{e} 5 2 1 1 1 2 3 4 5 6 7 8");
    }
    s.push_str("$EndElements\n");
    let gm = d.join("no_nodes.msh");
    std::fs::write(&gm, s).unwrap();
    let foam = d.join("labels_only");
    std::fs::create_dir_all(&foam).unwrap();
    let naca = repo().join("example-projects/wind-tunnel/meshes/naca0012-ogrid");
    for f in ["owner", "neighbour"] {
        std::fs::copy(naca.join(f), foam.join(f)).unwrap();
    }
    for (path, n) in [(gm, 60u64), (foam, 6144)] {
        let case = file_case(&path);
        match facts::case_facts(&case, InText::NotDefault, Some(10)) {
            Err(FactsError::TooLarge { cells, limit }) => assert_eq!((cells, limit), (n, 10), "{}", path.display()),
            other => panic!("{}: {other:?}", path.display()),
        }
        assert!(
            matches!(
                facts::case_facts(&case, InText::NotDefault, None),
                Err(FactsError::Invalid(_))
            ),
            "{}: the build must fail",
            path.display()
        );
    }
    let _ = std::fs::remove_dir_all(d);
}

/// Negative control: no estimate where the reader refuses the file; the reply is the build's.
#[test]
fn n2_none_where_the_reader_refuses() {
    let d = tmp("n2");
    let foam = d.join("binary");
    std::fs::create_dir_all(&foam).unwrap();
    for f in ["owner", "neighbour"] {
        std::fs::write(
            foam.join(f),
            "FoamFile\n{\n    version 2.0;\n    format binary;\n    class labelList;\n}\n2\n(0 1)\n",
        )
        .unwrap();
    }
    let gz = d.join("gz.msh");
    std::fs::write(&gz, [0x1f, 0x8b, 8, 0]).unwrap();
    let v40 = d.join("v40.msh");
    std::fs::write(
        &v40,
        "$MeshFormat\n4.0 0 8\n$EndMeshFormat\n$Elements\n1 1 1 1\n3 1 5 1\n1 1 2 3 4 5 6 7 8\n$EndElements\n",
    )
    .unwrap();
    let missing = d.join("missing.msh");
    for path in [foam, gz, v40, missing] {
        let case = file_case(&path);
        assert_eq!(cell_estimate(&case), None, "{}", path.display());
        let a = format!("{:?}", facts::case_facts(&case, InText::NotDefault, Some(10)));
        let b = format!("{:?}", facts::case_facts(&case, InText::NotDefault, None));
        assert!(a.starts_with("Err(Invalid"), "{}: {a}", path.display());
        assert_eq!(a, b, "{}", path.display());
    }
    let _ = std::fs::remove_dir_all(d);
}

// ------------------------------------------------------------------ heavy (ignored; run once each, ADR 0844)

fn heavy_dir() -> PathBuf {
    std::env::var_os("RCFD_F989_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("rcfd-f989"))
}

/// H1 to H4: the over-limit files.
#[test]
#[ignore = "ADR 0844 heavy: writes the over-limit files H1 to H4 into $RCFD_F989_DIR"]
fn p0_write_heavy_files() {
    let d = heavy_dir();
    std::fs::create_dir_all(&d).unwrap();
    gmsh_grid(128, &d.join("h1_v22_ascii.msh"), false);
    gmsh_grid(128, &d.join("h2_v41_bin.msh"), true);
    let m = box_mesh([128, 128, 128]);
    std::fs::write(d.join("h3_fluent.msh"), fluent_3d(&m, false)).unwrap();
    rcfd_io::openfoam::write_dir(&m, d.join("h4_polyMesh")).unwrap();
}

fn heavy_case(k: usize) -> (Case, u64) {
    let d = heavy_dir();
    let sector = |nx: i64, ny: i64, nz: i64| {
        gen_case(
            "annulus_3d_sector_cylindrical_interface",
            &[
                ("mesh.nx", json!(nx)),
                ("mesh.ny", json!(ny)),
                ("mesh.r_in", json!(1)),
                ("mesh.r_out", json!(3)),
                ("mesh.r_interface", json!(2)),
                ("mesh.nz", json!(nz)),
                ("mesh.sector_angle", json!(30)),
            ],
        )
    };
    let fan = |rel: &str, v: &[(&str, i64)]| {
        let e: Vec<(String, Value)> = v.iter().map(|(k, x)| (format!("mesh.fan.{k}"), json!(x))).collect();
        let e: Vec<(&str, Value)> = e.iter().map(|(k, x)| (k.as_str(), x.clone())).collect();
        shipped(rel, &e)
    };
    match k {
        1 => (file_case(&d.join("h1_v22_ascii.msh")), 2_097_152),
        2 => (file_case(&d.join("h2_v41_bin.msh")), 2_097_152),
        3 => (file_case(&d.join("h3_fluent.msh")), 2_097_152),
        4 => (file_case(&d.join("h4_polyMesh")), 2_097_152),
        5 => (
            gen_case(
                "cylinder_ogrid",
                &[
                    ("mesh.nx", json!(2000)),
                    ("mesh.ny", json!(1010)),
                    ("mesh.r_in", json!(1)),
                    ("mesh.r_out", json!(4)),
                ],
            ),
            2_020_000,
        ),
        6 => (
            gen_case(
                "pipe_sweep",
                &[
                    ("mesh.nx", json!(40)),
                    ("mesh.ny", json!(20)),
                    ("mesh.nz", json!(420)),
                    ("mesh.r_out", json!(1)),
                ],
            ),
            2_016_000,
        ),
        7 => (
            gen_case(
                "box_3d",
                &[
                    ("mesh.nx", json!(130)),
                    ("mesh.ny", json!(130)),
                    ("mesh.nz", json!(120)),
                ],
            ),
            2_028_000,
        ),
        8 => (
            gen_case(
                "box_3d",
                &[
                    ("mesh.nx", json!(200)),
                    ("mesh.ny", json!(100)),
                    ("mesh.nz", json!(101)),
                    ("mesh.box_3d.section", json!("rectangles")),
                ],
            ),
            2_020_000,
        ),
        9 => (sector(100, 50, 202), 2_020_000),
        10 => (
            fan(
                FAN_H,
                &[
                    ("n_up", 40),
                    ("n_chord", 60),
                    ("n_down", 40),
                    ("n_side", 50),
                    ("n_mid", 5),
                    ("n_span", 120),
                    ("n_gap", 20),
                ],
            ),
            2_022_000,
        ),
        11 => (
            fan(
                FAN_C,
                &[
                    ("n_up", 40),
                    ("n_chord", 60),
                    ("n_down", 40),
                    ("n_side", 70),
                    ("n_nose", 10),
                    ("n_core", 4),
                    ("n_span", 120),
                    ("n_gap", 20),
                ],
            ),
            2_076_800,
        ),
        // H11b (ADR 0844 amendment 2): H11 with the shipped case's n_nose 4
        12 => (
            fan(
                FAN_C,
                &[
                    ("n_up", 40),
                    ("n_chord", 60),
                    ("n_down", 40),
                    ("n_side", 70),
                    ("n_nose", 4),
                    ("n_core", 4),
                    ("n_span", 120),
                    ("n_gap", 20),
                ],
            ),
            2_009_600,
        ),
        _ => unreachable!(),
    }
}

const LIMIT: u64 = 2_000_000;

/// The refusal, timed: T_r in seconds.
fn refuse(k: usize, case: &Case, n: u64) -> f64 {
    let t = Instant::now();
    let r = facts::case_facts(case, InText::NotDefault, Some(LIMIT));
    let tr = t.elapsed().as_secs_f64();
    match r {
        Err(FactsError::TooLarge { cells, limit }) => assert_eq!((cells, limit), (n, LIMIT), "H{k}"),
        other => panic!("H{k}: {other:?}"),
    }
    tr
}

fn heavy_refusal(k: usize) {
    let (case, n) = heavy_case(k);
    let tr = refuse(k, &case, n);
    eprintln!("H{k} refusal alone: T_r {tr:.6} s");
}

fn heavy_timed(k: usize) {
    let (case, n) = heavy_case(k);
    let tr = refuse(k, &case, n);
    let t = Instant::now();
    let mesh = sim::build_mesh(&case);
    let tb = t.elapsed().as_secs_f64();
    let mesh = mesh.unwrap_or_else(|e| panic!("H{k}: the reference build failed (not deciding): {e:?}"));
    eprintln!(
        "H{k}: T_r {tr:.6} s, T_b {tb:.3} s, ratio {:.7}, estimate {n}, built {}",
        tr / tb,
        mesh.n_cells()
    );
    assert_eq!(mesh.n_cells() as u64, n, "H{k}: built count");
    assert!(tr <= 1.0, "H{k}: T_r {tr} s > 1 s");
    assert!(tr / tb <= 0.5, "H{k}: T_r / T_b {} > 0.5", tr / tb);
}

macro_rules! heavy {
    ($($k:literal $r:ident $h:ident),* $(,)?) => {$(
        #[test]
        #[ignore = "ADR 0844 heavy: the refusal alone (run once, under the sampler)"]
        fn $r() {
            heavy_refusal($k);
        }
        #[test]
        #[ignore = "ADR 0844 heavy: the refusal and the build, timed (run once, under the sampler)"]
        fn $h() {
            heavy_timed($k);
        }
    )*};
}

heavy!(
    1 r1_ h1_, 2 r2_ h2_, 3 r3_ h3_, 4 r4_ h4_, 5 r5_ h5_, 6 r6_ h6_, 7 r7_ h7_, 8 r8_ h8_, 9 r9_ h9_, 10 r10_ h10_,
    11 r11_ h11_,
);

// H11b (ADR 0844 amendment 2; printed as H12)
heavy!(12 r11b_ h11b_);
