//! Job J4 (ADR 0184) registered checks R1 to R10 and R12: the OBJ and glTF surface readers (F-280).
//! Every fixture is written by these tests, in memory (sidecar files through the resolver).

use rcfd_core::units::Unit;
use rcfd_core::vec3::{cross, dot, norm, sub, Vec3};
use rcfd_io::common::SurfaceImport;
use rcfd_io::gltf;
use rcfd_io::guard::fuzz_no_panic;
use rcfd_io::obj::{self, ObjOptions, ObjPatches};
use rcfd_io::registry::{self, Status};
use rcfd_io::stl::{self, StlSolid};
use rcfd_mesh::surface::{box_surface, diagnose, icosphere, DiagOptions, TriSurface};
use serde_json::{json, Value};
use std::fmt::Write as _;

// ---- fixtures --------------------------------------------------------------------------------------------------------

const CUBE: [[f64; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
];
/// Outward counter-clockwise quads, 1-based.
const QUADS: [[usize; 4]; 6] = [
    [1, 4, 3, 2],
    [5, 6, 7, 8],
    [1, 2, 6, 5],
    [4, 8, 7, 3],
    [1, 5, 8, 4],
    [2, 3, 7, 6],
];

fn m() -> ObjOptions {
    ObjOptions::default()
}

fn cube_obj(scale: f64) -> String {
    let mut s = String::from("# unit cube\no cube\n");
    for p in CUBE {
        let _ = writeln!(s, "v {:?} {:?} {:?}", p[0] * scale, p[1] * scale, p[2] * scale);
    }
    for q in QUADS {
        let _ = writeln!(s, "f {} {} {} {}", q[0], q[1], q[2], q[3]);
    }
    s
}

/// Cube triangle indices (0-based), each quad split as (0, 1, 2), (0, 2, 3) like the reader's ear clipping does.
fn cube_tri_indices() -> Vec<usize> {
    QUADS
        .iter()
        .flat_map(|q| [0, 1, 2, 0, 2, 3].map(|k| q[k] - 1))
        .collect()
}

fn key(p: Vec3) -> [u64; 3] {
    [(p[0] + 0.0).to_bits(), (p[1] + 0.0).to_bits(), (p[2] + 0.0).to_bits()]
}

/// Triangles as coordinate triples rotated to start at the smallest key (cyclic order kept), with the patch name; sorted.
fn canon(s: &SurfaceImport) -> Vec<([[u64; 3]; 3], String)> {
    let mut v: Vec<_> = (0..s.surface.n_tris())
        .map(|i| {
            let t = s.surface.tri_points(i).map(key);
            let r = (0..3).min_by_key(|&k| t[k]).unwrap();
            (
                [t[r], t[(r + 1) % 3], t[(r + 2) % 3]],
                s.patches[s.surface.tags[i] as usize].clone(),
            )
        })
        .collect();
    v.sort();
    v
}

fn closed(s: &TriSurface) -> rcfd_mesh::surface::Diagnostics {
    let d = diagnose(s, &DiagOptions::default());
    assert!(d.is_closed_manifold(), "{d:?}");
    assert_eq!(d.euler, 2, "{d:?}");
    d
}

fn tri_normal(t: [Vec3; 3]) -> Vec3 {
    cross(sub(t[1], t[0]), sub(t[2], t[0]))
}

fn tri_area(t: [Vec3; 3]) -> f64 {
    0.5 * norm(tri_normal(t))
}

fn b64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16) | ((*c.get(1).unwrap_or(&0) as u32) << 8) | (*c.get(2).unwrap_or(&0) as u32);
        for k in 0..4 {
            if k <= c.len() {
                out.push(A[(n >> (18 - 6 * k)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Binary data and the JSON arrays that describe it.
#[derive(Default)]
struct Gb {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Gb {
    fn view(&mut self, bytes: &[u8], stride: Option<usize>) -> usize {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let mut v = json!({"buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len()});
        if let Some(s) = stride {
            v["byteStride"] = json!(s);
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(v);
        self.views.len() - 1
    }

    fn acc(&mut self, view: usize, offset: usize, comp: u32, count: usize, ty: &str) -> usize {
        self.accessors
            .push(json!({"bufferView": view, "byteOffset": offset, "componentType": comp, "count": count, "type": ty}));
        self.accessors.len() - 1
    }

    fn positions(&mut self, pts: &[Vec3]) -> usize {
        let b: Vec<u8> = pts.iter().flatten().flat_map(|c| (*c as f32).to_le_bytes()).collect();
        let v = self.view(&b, None);
        self.acc(v, 0, 5126, pts.len(), "VEC3")
    }

    fn indices(&mut self, idx: &[usize], comp: u32) -> usize {
        let b: Vec<u8> = idx
            .iter()
            .flat_map(|&i| match comp {
                5121 => vec![i as u8],
                5123 => (i as u16).to_le_bytes().to_vec(),
                _ => (i as u32).to_le_bytes().to_vec(),
            })
            .collect();
        let v = self.view(&b, None);
        self.acc(v, 0, comp, idx.len(), "SCALAR")
    }

    /// Document with one buffer (no uri yet).
    fn doc(&self, meshes: Value, nodes: Value) -> Value {
        json!({
            "asset": {"version": "2.0", "generator": "rcfd j4 test"},
            "scene": 0,
            "scenes": [{"nodes": (0..nodes.as_array().unwrap().len()).filter(|&i| {
                // roots: nodes that are nobody's child
                !nodes.as_array().unwrap().iter().any(|n| n["children"].as_array().is_some_and(|c| c.contains(&json!(i))))
            }).collect::<Vec<_>>()}],
            "nodes": nodes,
            "meshes": meshes,
            "buffers": [{"byteLength": self.bin.len()}],
            "bufferViews": self.views,
            "accessors": self.accessors,
        })
    }
}

fn glb(doc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(doc).unwrap();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((28 + json.len() + bin.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

fn with_data_uri(mut doc: Value, bin: &[u8]) -> Vec<u8> {
    doc["buffers"][0]["uri"] = json!(format!("data:application/octet-stream;base64,{}", b64(bin)));
    serde_json::to_vec_pretty(&doc).unwrap()
}

fn one_mesh(pos: usize, idx: Option<usize>) -> Value {
    let mut prim = json!({"attributes": {"POSITION": pos}});
    if let Some(i) = idx {
        prim["indices"] = json!(i);
    }
    json!([{"name": "cube-data", "primitives": [prim]}])
}

fn cube_node() -> Value {
    json!([{"mesh": 0, "name": "cube"}])
}

fn cube_glb_u8() -> Vec<u8> {
    let mut g = Gb::default();
    let p = g.positions(&CUBE);
    let i = g.indices(&cube_tri_indices(), 5121);
    glb(&g.doc(one_mesh(p, Some(i)), cube_node()), &g.bin)
}

fn cube_gltf_data_uri() -> Vec<u8> {
    let mut g = Gb::default();
    let p = g.positions(&CUBE);
    let i = g.indices(&cube_tri_indices(), 5123);
    with_data_uri(g.doc(one_mesh(p, Some(i)), cube_node()), &g.bin)
}

fn read_g(bytes: &[u8]) -> rcfd_core::Result<SurfaceImport> {
    gltf::read_bytes(bytes, "m", gltf::no_external)
}

// ---- R1 to R5: OBJ ---------------------------------------------------------------------------------------------------

#[test]
fn r1_obj_cube() {
    let s = obj::read_bytes(cube_obj(1.0).as_bytes(), "m", &m()).unwrap();
    eprintln!("J4 R1 notes {:?}", s.notes);
    assert_eq!(s.patches, ["cube"]);
    assert_eq!(s.surface.points.len(), 8);
    assert_eq!(s.surface.n_tris(), 12);
    let d = closed(&s.surface);
    assert_eq!((d.free_edges, d.nonmanifold_edges, d.misoriented_edges), (0, 0, 0));
    let (a, v) = (s.surface.area(), s.surface.enclosed_volume());
    eprintln!("J4 R1 area {a:e} volume {v:e}");
    assert!((a - 6.0).abs() <= 1e-14 && (v - 1.0).abs() <= 1e-14, "{a} {v}");
    assert_eq!(
        s.surface.points,
        CUBE.to_vec(),
        "unit m keeps coordinates bit-identical"
    );
}

#[test]
fn r1b_obj_units_mm() {
    let s = obj::read_bytes(cube_obj(1000.0).as_bytes(), "mm", &m()).unwrap();
    let f = Unit::parse("mm").unwrap().factor;
    for (p, q) in s.surface.points.iter().zip(CUBE) {
        for k in 0..3 {
            assert_eq!(p[k].to_bits(), (q[k] * 1000.0 * f).to_bits());
        }
    }
    let v = s.surface.enclosed_volume();
    eprintln!("J4 R1b factor {f:e} volume {v:e}");
    assert!((v - 1.0).abs() <= 1e-12, "{v}");
    for bad in ["", "s", "degC", "1", "m/s", "kg"] {
        let e = obj::read_bytes(cube_obj(1.0).as_bytes(), bad, &m())
            .unwrap_err()
            .to_string();
        assert!(e.contains("unit"), "{bad}: {e}");
    }
}

#[test]
fn r2_polygons() {
    // convex octagon in z = 0
    let mut t = String::new();
    for k in 0..8 {
        let a = std::f64::consts::PI * k as f64 / 4.0;
        let _ = writeln!(t, "v {:?} {:?} 0", a.cos(), a.sin());
    }
    t += "f 1 2 3 4 5 6 7 8\n";
    let s = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    assert_eq!(s.surface.n_tris(), 6);
    assert!((0..6).all(|i| tri_normal(s.surface.tri_points(i))[2] > 0.0));

    // non-convex L of area 3, started at (2, 0) so that a fan from the first vertex would leave the polygon,
    // in the tilted plane spanned by u = (0.6, 0, 0.8) and v = (0, 1, 0)
    let l2 = [[2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0], [0.0, 0.0]];
    let (o, u, v) = ([0.5, -1.0, 2.0], [0.6, 0.0, 0.8], [0.0, 1.0, 0.0]);
    let to3 = |x: f64, y: f64| {
        [
            o[0] + x * u[0] + y * v[0],
            o[1] + x * u[1] + y * v[1],
            o[2] + x * u[2] + y * v[2],
        ]
    };
    let mut t = String::from("o L\n");
    for [x, y] in l2 {
        let p = to3(x, y);
        let _ = writeln!(t, "v {:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    t += "f 1 2 3 4 5 6\n";
    let s = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    assert_eq!(s.surface.n_tris(), 4);
    let nl = cross(u, v);
    let mut area = 0.0;
    for i in 0..4 {
        let tp = s.surface.tri_points(i);
        assert!(dot(tri_normal(tp), nl) > 0.0, "triangle {i} is inverted");
        area += tri_area(tp);
        let c = [0, 1, 2].map(|k| (tp[0][k] + tp[1][k] + tp[2][k]) / 3.0);
        let (x, y) = (dot(sub(c, o), u), dot(sub(c, o), v));
        let inside = (x > 0.0 && x < 2.0 && y > 0.0 && y < 1.0) || (x > 0.0 && x < 1.0 && y > 0.0 && y < 2.0);
        assert!(inside, "triangle {i} centroid ({x}, {y}) is outside the L");
    }
    eprintln!("J4 R2 L area {area:e}");
    assert!((area - 3.0).abs() <= 1e-12 * 3.0, "{area}");
    assert!(!s.notes.iter().any(|n| n.contains("fan")), "{:?}", s.notes);

    // self-intersecting quad (bow tie): fan with a note, no error
    let t = "v 0 0 0\nv 1 1 0\nv 1 0 0\nv 0 1 0\nf 1 2 3 4\n";
    let s = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    eprintln!("J4 R2 bow tie notes {:?}", s.notes);
    assert_eq!(s.surface.n_tris(), 2);
    assert!(s.notes.iter().any(|n| n.contains("fan")), "{:?}", s.notes);
}

#[test]
fn r3_negative_indices() {
    let mut t = String::from("o cube\n");
    for p in CUBE {
        let _ = writeln!(t, "v {:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    for q in QUADS {
        let r: Vec<String> = q.iter().map(|&i| format!("{}", i as i64 - 9)).collect();
        let _ = writeln!(t, "f {}", r.join(" "));
    }
    let a = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    let b = obj::read_bytes(cube_obj(1.0).as_bytes(), "m", &m()).unwrap();
    assert_eq!(a, b);
    // relative to the vertices defined so far: a triangle right after its three vertices
    let t = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\nv 0 0 1\nf -4 -1 -3\n";
    let s = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    assert_eq!(s.surface.tris, vec![[0, 1, 2], [0, 3, 1]]);
}

#[test]
fn r4_normals_and_texture_coordinates_are_ignored() {
    let mut t = String::from("mtllib cube.mtl\no cube\n");
    for p in CUBE {
        let _ = writeln!(t, "v {:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    t += "vt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 -1\nvn 0 0 1\nusemtl red\ns 1\n";
    let refs = ["{}/1/1", "{}//2", "{}/3", "{}/4/1", "{}", "{}/2/2"];
    for (f, q) in QUADS.iter().enumerate() {
        let r: Vec<String> = q.iter().map(|i| refs[f].replace("{}", &i.to_string())).collect();
        let _ = writeln!(t, "f {}", r.join(" "));
    }
    let a = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    let b = obj::read_bytes(cube_obj(1.0).as_bytes(), "m", &m()).unwrap();
    eprintln!("J4 R4 notes {:?}", a.notes);
    assert_eq!(a.surface, b.surface);
    assert_eq!(a.patches, b.patches);
    let n = a.notes.join(" | ");
    for k in ["4 vt", "2 vn", "1 s", "1 mtllib", "1 usemtl"] {
        assert!(n.contains(k), "{k} missing from {n}");
    }
}

#[test]
fn r5_several_objects_and_groups() {
    let mut t = String::new();
    for p in CUBE {
        let _ = writeln!(t, "v {:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    t += "f 1 2 3\no a\nf 1 3 4\no b\nf 5 6 7\no a\nf 5 7 8\n";
    let s = obj::read_bytes(t.as_bytes(), "m", &m()).unwrap();
    assert_eq!(s.patches, ["default", "a", "b"]);
    assert_eq!(s.surface.tags, vec![0, 1, 2, 1]);
    assert_eq!(s.patch_names().get(&2).map(|x| x.as_str()), Some("b"));

    let mut g = String::new();
    for p in CUBE {
        let _ = writeln!(g, "v {:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    g += "g left extra\nf 1 2 3\ng right\nf 5 6 7\ng left\nf 1 3 4\n";
    let by_group = obj::read_bytes(
        g.as_bytes(),
        "m",
        &ObjOptions {
            patches: ObjPatches::Groups,
        },
    )
    .unwrap();
    assert_eq!(by_group.patches, ["left", "right"]);
    assert_eq!(by_group.surface.tags, vec![0, 1, 0]);
    let default = obj::read_bytes(g.as_bytes(), "m", &m()).unwrap();
    eprintln!("J4 R5 group file read by objects: notes {:?}", default.notes);
    assert_eq!(default.patches, ["default"]);
    assert!(default.notes.iter().any(|n| n.contains("3 g")), "{:?}", default.notes);
}

// ---- R6, R7: glTF ----------------------------------------------------------------------------------------------------

#[test]
fn r6_gltf_cube_variants() {
    let reference = obj::read_bytes(cube_obj(1.0).as_bytes(), "m", &m()).unwrap();
    let want = canon(&reference);
    let mut variants: Vec<(&str, SurfaceImport)> = Vec::new();

    // (i) data URI, u16 indices
    variants.push(("i data uri u16", read_g(&cube_gltf_data_uri()).unwrap()));

    // (ii) sidecar file through the resolver, u32 indices; the file name needs percent-decoding
    let mut g = Gb::default();
    let p = g.positions(&CUBE);
    let i = g.indices(&cube_tri_indices(), 5125);
    let mut doc = g.doc(one_mesh(p, Some(i)), cube_node());
    doc["buffers"][0]["uri"] = json!("cube%20data.bin");
    let bin = g.bin.clone();
    let mut asked = Vec::new();
    let s = gltf::read_bytes(&serde_json::to_vec(&doc).unwrap(), "m", |rel: &str| {
        asked.push(rel.to_string());
        if rel == "cube data.bin" {
            Ok(bin.clone())
        } else {
            Err(rcfd_core::Error::Io(format!("no file {rel}")))
        }
    })
    .unwrap();
    assert_eq!(asked, ["cube data.bin"]);
    variants.push(("ii sidecar u32", s));

    // (iii) GLB, u8 indices
    variants.push(("iii glb u8", read_g(&cube_glb_u8()).unwrap()));

    // (iv) positions interleaved with normals, stride 24
    let mut g = Gb::default();
    let inter: Vec<u8> = CUBE
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 0.0, 0.0, 1.0])
        .flat_map(|c| (c as f32).to_le_bytes())
        .collect();
    let v = g.view(&inter, Some(24));
    let p = g.acc(v, 0, 5126, 8, "VEC3");
    let n = g.acc(v, 12, 5126, 8, "VEC3");
    let i = g.indices(&cube_tri_indices(), 5123);
    let meshes = json!([{"primitives": [{"attributes": {"POSITION": p, "NORMAL": n}, "indices": i}]}]);
    let s = read_g(&glb(&g.doc(meshes, cube_node()), &g.bin)).unwrap();
    assert!(
        s.notes.iter().any(|x| x.contains("other than POSITION")),
        "{:?}",
        s.notes
    );
    variants.push(("iv interleaved stride 24", s));

    // (v) 24 vertices, split at the face seams, indexed
    let mut g = Gb::default();
    let split: Vec<Vec3> = QUADS.iter().flat_map(|q| q.map(|i| CUBE[i - 1])).collect();
    let idx: Vec<usize> = (0..6).flat_map(|f| [0, 1, 2, 0, 2, 3].map(|k| 4 * f + k)).collect();
    let p = g.positions(&split);
    let i = g.indices(&idx, 5123);
    let s = read_g(&glb(&g.doc(one_mesh(p, Some(i)), cube_node()), &g.bin)).unwrap();
    assert!(
        s.notes.iter().any(|x| x.contains("16 vertices with bitwise-equal")),
        "{:?}",
        s.notes
    );
    variants.push(("v split 24", s));

    // (vi) not indexed, 36 vertices
    let mut g = Gb::default();
    let soup: Vec<Vec3> = cube_tri_indices().iter().map(|&i| CUBE[i]).collect();
    let p = g.positions(&soup);
    variants.push((
        "vi soup 36",
        read_g(&glb(&g.doc(one_mesh(p, None), cube_node()), &g.bin)).unwrap(),
    ));

    for (what, s) in &variants {
        eprintln!(
            "J4 R6 {what}: {} vertices, {} triangles, notes {:?}",
            s.surface.points.len(),
            s.surface.n_tris(),
            s.notes
        );
        assert_eq!(canon(s), want, "{what}");
        assert_eq!(s.patches, ["cube"], "{what}");
        closed(&s.surface);
    }

    // (vii) a unit square as a strip and as a fan, against mode 4
    let sq_strip = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
    let sq_fan = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    for (mode, pts) in [(4u32, sq_fan), (5, sq_strip), (6, sq_fan)] {
        let mut g = Gb::default();
        let p = g.positions(&pts);
        let mut prim = json!({"attributes": {"POSITION": p}, "mode": mode});
        if mode == 4 {
            let i = g.indices(&[0, 1, 2, 0, 2, 3], 5123);
            prim["indices"] = json!(i);
        }
        let s = read_g(&glb(
            &g.doc(json!([{"primitives": [prim]}]), json!([{"mesh": 0}])),
            &g.bin,
        ))
        .unwrap();
        assert_eq!(s.surface.n_tris(), 2, "mode {mode}");
        assert_eq!(s.patches, ["mesh0"]);
        let area: f64 = (0..2).map(|i| tri_area(s.surface.tri_points(i))).sum();
        assert!((area - 1.0).abs() <= 1e-15, "mode {mode}: area {area}");
        for i in 0..2 {
            assert!(tri_normal(s.surface.tri_points(i))[2] > 0.0, "mode {mode} triangle {i}");
        }
    }
}

fn gltf_with_nodes(nodes: Value) -> SurfaceImport {
    let mut g = Gb::default();
    let p = g.positions(&CUBE);
    let i = g.indices(&cube_tri_indices(), 5123);
    read_g(&glb(&g.doc(one_mesh(p, Some(i)), nodes), &g.bin)).unwrap()
}

#[test]
fn r7_gltf_node_transforms() {
    let h = 0.5f64.sqrt();
    let trs = gltf_with_nodes(json!([{"mesh": 0, "name": "c", "translation": [1.0, 2.0, 3.0],
        "rotation": [0.0, 0.0, h, h], "scale": [2.0, 2.0, 2.0]}]));
    let expect = |p: Vec3| [1.0 - 2.0 * p[1], 2.0 + 2.0 * p[0], 3.0 + 2.0 * p[2]];
    let mut worst = 0.0f64;
    for (q, p) in trs.surface.points.iter().zip(CUBE) {
        let x = expect(p);
        for k in 0..3 {
            worst = worst.max((q[k] - x[k]).abs());
        }
    }
    let v = trs.surface.enclosed_volume();
    eprintln!("J4 R7 TRS worst vertex error {worst:e}, volume {v:e}");
    assert!(worst <= 1e-12 && (v - 8.0).abs() <= 1e-12, "{worst} {v}");

    let mat = gltf_with_nodes(json!([{"mesh": 0, "name": "c",
        "matrix": [0.0, 2.0, 0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 1.0, 2.0, 3.0, 1.0]}]));
    for (a, b) in mat.surface.points.iter().zip(&trs.surface.points) {
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() <= 1e-12, "{a:?} {b:?}");
        }
    }

    let child = gltf_with_nodes(
        json!([{"name": "parent", "translation": [10.0, 0.0, 0.0], "children": [1]},
        {"mesh": 0, "name": "c", "scale": [2.0, 2.0, 2.0]}]),
    );
    for (q, p) in child.surface.points.iter().zip(CUBE) {
        assert_eq!(*q, [10.0 + 2.0 * p[0], 2.0 * p[1], 2.0 * p[2]]);
    }

    let mirror = gltf_with_nodes(json!([{"mesh": 0, "name": "c", "scale": [-1.0, 1.0, 1.0]}]));
    let d = closed(&mirror.surface);
    eprintln!(
        "J4 R7 mirrored volume {:e}, notes {:?}",
        d.enclosed_volume, mirror.notes
    );
    assert_eq!(d.misoriented_edges, 0);
    assert!((mirror.surface.enclosed_volume() - 1.0).abs() <= 1e-14);

    let two =
        gltf_with_nodes(json!([{"mesh": 0, "name": "a"}, {"mesh": 0, "name": "b", "translation": [3.0, 0.0, 0.0]}]));
    assert_eq!(two.patches, ["a", "b"]);
    assert_eq!(two.surface.n_tris(), 24);
}

// ---- R8: malformed input -------------------------------------------------------------------------------------------

fn obj_err(text: &str) -> String {
    obj::read_bytes(text.as_bytes(), "m", &m()).unwrap_err().to_string()
}

#[test]
fn r8_obj_errors() {
    let v3 = "v 0 0 0\nv 1 0 0\nv 0 1 0\n";
    let cases: Vec<(String, &str)> = vec![
        (format!("{v3}f 0 1 2\n"), "line 4"),
        (format!("{v3}f 1 2 4\n"), "line 4"),
        (format!("{v3}f -4 1 2\n"), "line 4"),
        (format!("{v3}f 1 2\n"), "line 4"),
        ("v 1 2\n".to_string(), "line 1"),
        ("v 1 nan 2\n".to_string(), "line 1"),
        ("v 1 inf 2\n".to_string(), "line 1"),
        (format!("{v3}f 1 a 3\n"), "line 4"),
        (format!("{v3}f 1/2/3/4 2 3\n"), "line 4"),
        (format!("{v3}surf 0 1 0 1 1 2 3\n"), "line 4"),
        (v3.to_string(), "no triangles"),
        (String::new(), "no triangles"),
    ];
    for (text, place) in cases {
        let e = obj_err(&text);
        eprintln!("J4 R8 OBJ {:?}: {e}", text.lines().last().unwrap_or(""));
        assert!(e.contains(place), "{e}");
    }
}

#[test]
fn r8_gltf_errors() {
    let good = || {
        let mut g = Gb::default();
        let p = g.positions(&CUBE);
        let i = g.indices(&cube_tri_indices(), 5123);
        (g.doc(one_mesh(p, Some(i)), cube_node()), g.bin)
    };
    let (doc0, bin0) = good();
    assert!(read_g(&glb(&doc0, &bin0)).is_ok());
    type Edit = Box<dyn Fn(&mut Value, &mut Vec<u8>)>;
    let cases: Vec<(&str, Edit, &str)> = vec![
        (
            "version 1.0",
            Box::new(|d, _| d["asset"]["version"] = json!("1.0")),
            "asset.version",
        ),
        (
            "required extension",
            Box::new(|d, _| d["extensionsRequired"] = json!(["KHR_draco_mesh_compression"])),
            "KHR_draco_mesh_compression",
        ),
        (
            "accessor past its view",
            Box::new(|d, _| d["accessors"][0]["count"] = json!(9)),
            "accessors[0]",
        ),
        (
            "stride below element",
            Box::new(|d, _| d["bufferViews"][0]["byteStride"] = json!(8)),
            "bufferViews[0].byteStride",
        ),
        (
            "index not below the vertex count",
            Box::new(|_, b| b[96..98].copy_from_slice(&8u16.to_le_bytes())),
            "accessors[1]",
        ),
        (
            "restart value",
            Box::new(|_, b| b[96..98].copy_from_slice(&u16::MAX.to_le_bytes())),
            "primitive-restart",
        ),
        (
            "POSITION as VEC2",
            Box::new(|d, _| d["accessors"][0]["type"] = json!("VEC2")),
            "accessors[0]",
        ),
        (
            "POSITION as u16",
            Box::new(|d, _| d["accessors"][0]["componentType"] = json!(5123)),
            "accessors[0]",
        ),
        (
            "sparse accessor",
            Box::new(|d, _| d["accessors"][0]["sparse"] = json!({"count": 1, "indices": {}, "values": {}})),
            "sparse",
        ),
        (
            "node cycle",
            Box::new(|d, _| {
                d["nodes"] = json!([{"children": [1]}, {"mesh": 0, "children": [0]}]);
                d["scenes"] = json!([{"nodes": [0]}]);
            }),
            "nodes[0]",
        ),
        (
            "35 triangle indices",
            Box::new(|d, _| d["accessors"][1]["count"] = json!(35)),
            "meshes[0].primitives[0]",
        ),
        (
            "unknown mode",
            Box::new(|d, _| d["meshes"][0]["primitives"][0]["mode"] = json!(9)),
            "mode",
        ),
        (
            "missing POSITION",
            Box::new(|d, _| d["meshes"][0]["primitives"][0]["attributes"] = json!({})),
            "POSITION",
        ),
        (
            "bad node index",
            Box::new(|d, _| d["scenes"] = json!([{"nodes": [7]}])),
            "nodes[7]",
        ),
        (
            "bad mesh index",
            Box::new(|d, _| d["nodes"][0]["mesh"] = json!(3)),
            "nodes[0].mesh",
        ),
        (
            "negative count",
            Box::new(|d, _| d["accessors"][0]["count"] = json!(-1)),
            "accessors[0].count",
        ),
        (
            "view past buffer",
            Box::new(|d, _| d["bufferViews"][1]["byteLength"] = json!(1000)),
            "bufferViews[1]",
        ),
        (
            "zero quaternion",
            Box::new(|d, _| d["nodes"][0]["rotation"] = json!([0.0, 0.0, 0.0, 0.0])),
            "rotation",
        ),
    ];
    for (what, edit, place) in cases {
        let (mut d, mut b) = good();
        edit(&mut d, &mut b);
        let e = read_g(&glb(&d, &b)).unwrap_err().to_string();
        eprintln!("J4 R8 glTF {what}: {e}");
        assert!(e.contains(place), "{what}: {e}");
    }

    // buffers and URIs (JSON documents)
    let (doc, bin) = good();
    let mut d = doc.clone();
    d["buffers"][0]["uri"] = json!(format!("data:application/octet-stream;base64,{}*", &b64(&bin)[..8]));
    let e = read_g(&serde_json::to_vec(&d).unwrap()).unwrap_err().to_string();
    eprintln!("J4 R8 glTF bad base64: {e}");
    assert!(e.contains("buffers[0].uri") && e.contains("base64"), "{e}");
    for (uri, place) in [
        ("missing.bin", "buffers[0].uri"),
        ("../cube.bin", "buffers[0].uri"),
        ("sub/../../cube.bin", "buffers[0].uri"),
        ("/abs/cube.bin", "buffers[0].uri"),
        ("C:/cube.bin", "buffers[0].uri"),
        ("https://example.org/cube.bin", "buffers[0].uri"),
    ] {
        let mut d = doc.clone();
        d["buffers"][0]["uri"] = json!(uri);
        let e = gltf::read_bytes(&serde_json::to_vec(&d).unwrap(), "m", |rel: &str| {
            Err(rcfd_core::Error::Io(format!("no file {rel}")))
        })
        .unwrap_err()
        .to_string();
        eprintln!("J4 R8 glTF uri {uri}: {e}");
        assert!(e.contains(place), "{uri}: {e}");
    }
    let mut d = doc.clone();
    d.as_object_mut().unwrap().remove("buffers");
    assert!(read_g(&serde_json::to_vec(&d).unwrap()).is_err());
    let e = read_g(b"[1, 2]").unwrap_err().to_string();
    assert!(e.contains("JSON root"), "{e}");
    let e = read_g(b"{\"asset\": ").unwrap_err().to_string();
    assert!(e.contains("JSON"), "{e}");

    // GLB container
    let ok = glb(&doc, &bin);
    let mut cases: Vec<(&str, Vec<u8>, &str)> = Vec::new();
    let mut b = ok.clone();
    b[3] = b'X';
    cases.push(("bad magic", b, "byte offset 0"));
    let mut b = ok.clone();
    b[4..8].copy_from_slice(&1u32.to_le_bytes());
    cases.push(("version 1", b, "byte offset 4"));
    let mut b = ok.clone();
    b.extend_from_slice(&[0; 4]);
    cases.push(("length mismatch", b, "byte offset 8"));
    let mut b = ok.clone();
    b[16..20].copy_from_slice(b"BIN\0");
    cases.push(("first chunk not JSON", b, "byte offset 16"));
    let mut b = ok.clone();
    let jl = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
    b[12..16].copy_from_slice(&(jl + 4096).to_le_bytes());
    cases.push(("chunk past the end", b, "byte offset 12"));
    let mut b = ok.clone();
    b.truncate(ok.len() - 4);
    let n = b.len() as u32;
    b[8..12].copy_from_slice(&n.to_le_bytes());
    cases.push(("BIN chunk cut", b, "byte offset"));
    for (what, bytes, place) in cases {
        let e = gltf::read_glb(&bytes, "m", gltf::no_external).unwrap_err().to_string();
        eprintln!("J4 R8 GLB {what}: {e}");
        assert!(e.contains(place), "{what}: {e}");
    }
}

// ---- R9: no panic ----------------------------------------------------------------------------------------------------

#[test]
fn r9_no_panic_on_prefixes_and_mutations() {
    let o = cube_obj(1.0);
    let mut n_ok = 0usize;
    fuzz_no_panic(o.as_bytes(), 3000, |b| {
        n_ok += obj::read_bytes(b, "m", &m()).is_ok() as usize
    });
    let j = cube_gltf_data_uri();
    let mut j_ok = 0usize;
    fuzz_no_panic(&j, 3000, |b| j_ok += read_g(b).is_ok() as usize);
    let g = cube_glb_u8();
    let mut g_ok = 0usize;
    fuzz_no_panic(&g, 3000, |b| g_ok += read_g(b).is_ok() as usize);
    eprintln!(
        "J4 R9 accepted inputs: OBJ {n_ok} of {}, glTF {j_ok} of {}, GLB {g_ok} of {}",
        o.len() + 3001,
        j.len() + 3001,
        g.len() + 3001
    );
}

// ---- R10: round trips ------------------------------------------------------------------------------------------------

fn write_obj(s: &TriSurface, names: &[&str]) -> String {
    let mut o = String::new();
    for p in &s.points {
        let _ = writeln!(o, "v {:?} {:?} {:?}", p[0], p[1], p[2]);
    }
    for (k, name) in names.iter().enumerate() {
        let _ = writeln!(o, "o {name}");
        for (t, &tag) in s.tris.iter().zip(&s.tags) {
            if tag as usize == k {
                let _ = writeln!(o, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1);
            }
        }
    }
    o
}

fn two_patch_surface() -> TriSurface {
    let mut s = box_surface([0.0, 0.0, 0.0], [2.0, 1.0, 1.0], [2, 1, 1]);
    let mut b = icosphere(0.3, 1);
    for p in &mut b.points {
        p[0] += 5.0;
    }
    b.set_tag(1);
    s.append(&b);
    s
}

#[test]
fn r10_round_trips() {
    // (a) OBJ, bit exact
    let s = two_patch_surface();
    let back = obj::read_bytes(write_obj(&s, &["box", "sphere"]).as_bytes(), "m", &m()).unwrap();
    assert_eq!(back.patches, ["box", "sphere"]);
    assert_eq!(back.surface, s, "OBJ round trip");
    eprintln!(
        "J4 R10a {} points, {} triangles, notes {:?}",
        s.points.len(),
        s.n_tris(),
        back.notes
    );

    // (b) GLB of the box part (dyadic coordinates are exact in f32)
    let bx = box_surface([0.0, 0.0, 0.0], [2.0, 1.0, 1.0], [2, 1, 1]);
    let mut g = Gb::default();
    let p = g.positions(&bx.points);
    let idx: Vec<usize> = bx.tris.iter().flatten().copied().collect();
    let i = g.indices(&idx, 5125);
    let r = read_g(&glb(
        &g.doc(one_mesh(p, Some(i)), json!([{"mesh": 0, "name": "box"}])),
        &g.bin,
    ))
    .unwrap();
    assert_eq!(r.patches, ["box"]);
    assert_eq!(r.surface, bx, "GLB round trip");

    // (c) OBJ import -> one StlSolid per patch -> ASCII STL -> STL reader
    let mut text = String::new();
    for (k, name) in back.patches.iter().enumerate() {
        let triangles = back.patch_triangles(k as u32);
        let stored_normals = triangles
            .iter()
            .map(|t| rcfd_core::vec3::unit(tri_normal(*t)))
            .collect();
        let solid = StlSolid {
            name: name.clone(),
            triangles,
            stored_normals,
        };
        text += &stl::write_ascii(&solid).unwrap();
    }
    let solids = stl::read_bytes(text.as_bytes()).unwrap();
    assert_eq!(solids.len(), 2);
    for (k, sol) in solids.iter().enumerate() {
        assert_eq!(sol.name, back.patches[k]);
        assert_eq!(sol.triangles, back.patch_triangles(k as u32), "STL solid {k}");
    }
}

// ---- R12: registry ---------------------------------------------------------------------------------------------------

#[test]
fn r12_registry_cases() {
    let all = registry::all();
    for n in ["obj", "gltf"] {
        let i = all.iter().find(|i| i.name == n).expect(n);
        assert_eq!(i.status, Status::Supported);
        registry::conformance_for(n).unwrap();
        let e = registry::conformance_with(n, true).unwrap_err();
        eprintln!("J4 R12 tampered {n}: {e}");
        assert!(e.contains(n), "{e}");
    }
    assert!(all.iter().any(|i| i.extensions.contains(&".glb")));
}

#[test]
fn read_file_resolves_sidecar_in_the_folder() {
    let dir = std::env::temp_dir().join(format!("rcfd_j4_gltf_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let mut g = Gb::default();
    let p = g.positions(&CUBE);
    let i = g.indices(&cube_tri_indices(), 5123);
    let mut doc = g.doc(one_mesh(p, Some(i)), cube_node());
    doc["buffers"][0]["uri"] = json!("data/cube.bin");
    std::fs::write(dir.join("data").join("cube.bin"), &g.bin).unwrap();
    std::fs::write(dir.join("cube.gltf"), serde_json::to_vec(&doc).unwrap()).unwrap();
    std::fs::write(dir.join("cube.obj"), cube_obj(100.0)).unwrap();
    let a = gltf::read_file(dir.join("cube.gltf"), "m");
    let b = obj::read_file(dir.join("cube.obj"), "cm", &m());
    let _ = std::fs::remove_dir_all(&dir);
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(
        canon(&a),
        canon(&obj::read_bytes(cube_obj(1.0).as_bytes(), "m", &m()).unwrap())
    );
    assert!((b.surface.enclosed_volume() - 1.0).abs() <= 1e-12);
}
