//! F-131 / F-132 (ADR 0104): opt-in tetrahedral quality pass on the cylinder-in-box pipeline, from the STL file and from the
//! in-code surface. Limits are those registered in docs/adr/0104-wave41d-tet-sliver-removal.md before this file was run (the
//! ignored diagnostic tests print only).

use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, unit, Vec3};
use rcfd_io::stl::{self, StlSolid};
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::surface::{box_surface, cube, cylinder, diagnose, icosphere, torus, DiagOptions, TriSurface};
use rcfd_mesh::surface_remesh::{remesh, RemeshOptions, SizeField};
use rcfd_mesh::tetmesh::{circumsphere, tet_quality, tetrahedralize, TetMesh, TetOptions, TetQuality};
use rcfd_mesh::Mesh;
use std::collections::{BTreeMap, BTreeSet};

/// Box tag 0, inward-facing cylinder tag 1 (the `cyl_in_box` surface of crates/rcfd-mesh/tests/tetmesh.rs).
fn in_code() -> TriSurface {
    let mut s = box_surface([-2.0, -2.0, -1.0], [2.0, 2.0, 1.0], [4, 4, 2]);
    let mut c = cylinder(0.5, 1.0, 16, 4);
    c.flip_all();
    c.set_tag(1);
    s.append(&c);
    s
}

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

/// The ADR 0093 STL path: two ASCII solids in one file, read back, each welded bitwise, the cylinder tagged 1.
fn from_stl_file() -> TriSurface {
    let b = box_surface([-2.0, -2.0, -1.0], [2.0, 2.0, 1.0], [4, 4, 2]);
    let mut c = cylinder(0.5, 1.0, 16, 4);
    c.flip_all();
    let text = stl::write_ascii(&solid("box", &b)).unwrap() + &stl::write_ascii(&solid("cylinder", &c)).unwrap();
    // one file per call: tests run in parallel and a shared name raced (a reader saw the file truncated by another writer)
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("w41d_cylinder_in_box_{n}.stl"));
    std::fs::write(&path, text).unwrap();
    let back = stl::read_file(&path).unwrap();
    let mut s = TriSurface::from_soup(&back[0].triangles, 0.0);
    let mut cyl = TriSurface::from_soup(&back[1].triangles, 0.0);
    cyl.set_tag(1);
    s.append(&cyl);
    s
}

fn curvature_remesh(s: &TriSurface) -> TriSurface {
    let f = SizeField::Curvature {
        theta: 0.5,
        h_min: 0.1,
        h_max: 1.0,
    };
    remesh(s, &f, &RemeshOptions::default()).unwrap().0
}

fn fingerprint(tm: &TetMesh) -> (usize, u64) {
    (tm.cells.len(), tm.to_mesh(&BTreeMap::new()).unwrap().content_hash())
}

/// W0: default options give the fingerprints (cell count, `Mesh::content_hash`) recorded at HEAD a2e0bfd before any library
/// change (the ADR 0104 smoke table), so the default output is bit-identical.
#[test]
fn w0_default_output_bit_identical() {
    let cases = [
        ("cube", cube(2.0, 2), None, (108, 2507394269494118308)),
        ("sphere", icosphere(1.0, 2), None, (320, 15939201240145194707)),
        ("torus", torus(2.0, 0.5, 24, 12), None, (899, 826700040160655771)),
        ("cyl_in_box", in_code(), None, (1137, 9960403326714801105)),
        (
            "p1_in_code",
            curvature_remesh(&in_code()),
            None,
            (1061, 1192961234831520497),
        ),
        (
            "s2_stl",
            curvature_remesh(&from_stl_file()),
            None,
            (1115, 7631245200477700787),
        ),
        ("sphere_h0.3", icosphere(1.0, 2), Some(0.3), (823, 4586941130221034709)),
        (
            "sphere_h0.25",
            icosphere(1.0, 2),
            Some(0.25),
            (1277, 9750081027263524416),
        ),
        ("cube4_h0.5", cube(2.0, 4), Some(0.5), (597, 11005660464813738162u64)),
    ];
    for (n, s, size, want) in cases {
        let o = TetOptions {
            size,
            ..Default::default()
        };
        assert_eq!(fingerprint(&tetrahedralize(&s, &o).unwrap()), want, "{n}");
    }
}

/// The pass frozen in ADR 0104 amendment 1.
fn with_pass() -> TetOptions {
    TetOptions {
        quality_sweeps: 10,
        ..Default::default()
    }
}

struct Run {
    r: TriSurface,
    tm: TetMesh,
    m: Mesh,
    q: TetQuality,
}

fn run_pass(s: &TriSurface) -> Run {
    let tm = tetrahedralize(s, &with_pass()).expect("tetrahedralize");
    let m = tm.to_mesh(&BTreeMap::new()).expect("mesh");
    let q = tet_quality(&tm, 2.0);
    eprintln!("{} cells, {:?}, {q:?}", m.n_cells(), tm.report);
    Run { r: s.clone(), tm, m, q }
}

fn stl_path() -> Run {
    run_pass(&curvature_remesh(&from_stl_file()))
}

fn in_code_path() -> Run {
    run_pass(&curvature_remesh(&in_code()))
}

/// Q1 / Q3: the ADR 0093 S2 checks with the pass.
fn watertight_and_dihedral(x: &Run) {
    assert!(diagnose(&x.r, &DiagOptions::default()).is_closed_manifold());
    let e = (x.m.total_volume() - x.r.enclosed_volume()).abs() / x.r.enclosed_volume();
    assert!(e <= 1e-10, "volume error {e}");
    assert_eq!(x.m.patches().len(), 2);
    assert!(x.m.cell_volumes().iter().all(|&v| v > 0.0) && x.q.min_volume > 0.0);
    let rep = check_mesh(&x.m);
    assert!(rep.is_ok(), "{:?}", rep.errors);
    assert!(x.q.min_dihedral_deg >= 3.0, "min dihedral {}", x.q.min_dihedral_deg);
}

/// Q2 / Q4: the ADR 0093 S3 limit with the pass.
fn radius_edge(x: &Run) {
    assert!(
        x.q.frac_radius_edge_ok >= 0.99,
        "radius-edge fraction {}",
        x.q.frac_radius_edge_ok
    );
}

fn euler(m: &Mesh) -> i64 {
    let mut used = BTreeSet::new();
    let mut edges = BTreeSet::new();
    for f in 0..m.n_faces() {
        let ids = m.face_points(f);
        for k in 0..ids.len() {
            used.insert(ids[k]);
            let (a, b) = (ids[k], ids[(k + 1) % ids.len()]);
            edges.insert((a.min(b), a.max(b)));
        }
    }
    used.len() as i64 - edges.len() as i64 + m.n_faces() as i64 - m.n_cells() as i64
}

/// Q5: per tag, patch area equal to the remeshed surface area within 1e-10 relative; every boundary face centroid on a
/// remeshed triangle of the same tag within 1e-9; V - E + F - C = 2.
fn patches_preserved(x: &Run) {
    let m = &x.m;
    for (k, p) in m.patches().iter().enumerate() {
        let tris: Vec<[Vec3; 3]> = (0..x.r.n_tris())
            .filter(|&i| x.r.tags[i] == k as u32)
            .map(|i| x.r.tri_points(i))
            .collect();
        let a_in: f64 = tris
            .iter()
            .map(|t| 0.5 * norm(cross(sub(t[1], t[0]), sub(t[2], t[0]))))
            .sum();
        let a_out: f64 = (p.start..p.start + p.size).map(|f| m.face_area(f)).sum();
        assert!(
            (a_out - a_in).abs() <= 1e-10 * a_in,
            "patch {}: area {a_out} vs {a_in}",
            p.name
        );
        for f in p.start..p.start + p.size {
            let c = m.face_centres()[f];
            let on = tris.iter().any(|t| {
                let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
                let nn = norm(n);
                dot(sub(c, t[0]), n).abs() / nn <= 1e-9
                    && (0..3).all(|j| dot(cross(sub(t[(j + 1) % 3], t[j]), sub(c, t[j])), n) >= -1e-9 * nn)
            });
            assert!(
                on,
                "patch {}: face {f} centroid {c:?} not on a remeshed triangle of its tag",
                p.name
            );
        }
    }
    assert_eq!(euler(m), 2);
}

#[test]
fn q1_stl_watertight_and_min_dihedral_with_pass() {
    watertight_and_dihedral(&stl_path());
}

#[test]
fn q2_stl_radius_edge_fraction_with_pass() {
    radius_edge(&stl_path());
}

#[test]
fn q3_in_code_watertight_and_min_dihedral_with_pass() {
    watertight_and_dihedral(&in_code_path());
}

#[test]
fn q4_in_code_radius_edge_fraction_with_pass() {
    radius_edge(&in_code_path());
}

#[test]
fn q5_patches_preserved_with_pass() {
    patches_preserved(&stl_path());
    patches_preserved(&in_code_path());
}

#[test]
fn q6_determinism_with_pass() {
    let (a, b) = (stl_path(), stl_path());
    assert_eq!(a.tm.cells, b.tm.cells);
    assert_eq!(a.tm.points.len(), b.tm.points.len());
    for (p, q) in a.tm.points.iter().zip(&b.tm.points) {
        assert_eq!(key(*p), key(*q));
    }
}

/// Q7: held-out cases with the pass, the ADR 0030 T5 limits plus volume and `check_mesh`.
fn held_out(n: &str, s: &TriSurface) {
    let x = run_pass(s);
    let e = (x.m.total_volume() - s.enclosed_volume()).abs() / s.enclosed_volume();
    eprintln!("Q7 {n}: volume error {e:e}");
    assert!(e <= 1e-10, "{n}: volume error {e}");
    assert!(check_mesh(&x.m).is_ok(), "{n}: check_mesh");
    assert!(
        x.q.frac_radius_edge_ok >= 0.99,
        "{n}: fraction {}",
        x.q.frac_radius_edge_ok
    );
    assert!(
        x.q.min_dihedral_deg >= 3.0,
        "{n}: min dihedral {}",
        x.q.min_dihedral_deg
    );
    assert!(x.q.min_volume > 0.0, "{n}: min volume {}", x.q.min_volume);
}

#[test]
fn q7a_held_out_cube_with_pass() {
    held_out("cube", &cube(2.0, 2));
}

#[test]
fn q7b_held_out_sphere_with_pass() {
    held_out("sphere", &icosphere(1.0, 2));
}

#[test]
fn q7c_held_out_remeshed_torus_with_pass() {
    let f = SizeField::Curvature {
        theta: 0.6,
        h_min: 0.05,
        h_max: 0.8,
    };
    let r = remesh(&torus(2.0, 0.5, 48, 24), &f, &RemeshOptions::default())
        .unwrap()
        .0;
    held_out("torus", &r);
}

type Key = [u64; 3];

fn key(p: Vec3) -> Key {
    [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()]
}

/// Surface as geometry: sorted multiset of (canonically rotated bitwise triangle, tag).
fn surf_geom(s: &TriSurface) -> Vec<([Key; 3], u32)> {
    let mut v: Vec<([Key; 3], u32)> = (0..s.n_tris())
        .map(|i| {
            let t = s.tri_points(i).map(key);
            let k = (0..3).min_by_key(|&k| t[k]).unwrap();
            ([t[k], t[(k + 1) % 3], t[(k + 2) % 3]], s.tags[i])
        })
        .collect();
    v.sort_unstable();
    v
}

/// Volume mesh as geometry: sorted multiset of cells, each the sorted bitwise coordinates of its vertices.
fn cell_geom(tm: &TetMesh) -> Vec<[Key; 4]> {
    let mut v: Vec<[Key; 4]> = tm
        .cells
        .iter()
        .map(|c| {
            let mut k = c.map(|i| key(tm.points[i]));
            k.sort_unstable();
            k
        })
        .collect();
    v.sort_unstable();
    v
}

fn renumber(s: &TriSurface, rev_ids: bool, rev_tris: bool) -> TriSurface {
    let n = s.points.len();
    let mut out = s.clone();
    if rev_ids {
        out.points = s.points.iter().rev().copied().collect();
        for t in out.tris.iter_mut() {
            *t = t.map(|v| n - 1 - v);
        }
    }
    if rev_tris {
        out.tris.reverse();
        out.tags.reverse();
    }
    out
}

fn print_quality(n: &str, tm: &TetMesh) {
    let q = tet_quality(tm, 2.0);
    eprintln!(
        "{n}: {} cells, min dihedral {:.4}, frac {:.4}, {:?}",
        tm.cells.len(),
        q.min_dihedral_deg,
        q.frac_radius_edge_ok,
        tm.report
    );
}

/// ADR 0104 D0, D1, D2.
#[test]
#[ignore = "ADR 0104 diagnostic D0-D2: prints only"]
fn d012_numbering_dependence() {
    let o = TetOptions::default();
    let (a, b) = (in_code(), from_stl_file());
    let (ta, tb) = (tetrahedralize(&a, &o).unwrap(), tetrahedralize(&b, &o).unwrap());
    eprintln!(
        "D0 raw surfaces equal as geometry: {}; tet meshes equal as geometry: {} ({} vs {} cells)",
        surf_geom(&a) == surf_geom(&b),
        cell_geom(&ta) == cell_geom(&tb),
        ta.cells.len(),
        tb.cells.len()
    );
    let (ra, rb) = (curvature_remesh(&a), curvature_remesh(&b));
    let (ga, gb) = (surf_geom(&ra), surf_geom(&rb));
    let common = ga.iter().filter(|t| gb.binary_search(t).is_ok()).count();
    eprintln!(
        "D1 remeshed: {} vs {} tris, {} vs {} points, equal as geometry: {}, triangles in common {common}",
        ra.n_tris(),
        rb.n_tris(),
        ra.points.len(),
        rb.points.len(),
        ga == gb
    );
    let base = tetrahedralize(&ra, &o).unwrap();
    print_quality("D2 base (in-code remeshed)", &base);
    for (n, ri, rt) in [
        ("D2a ids reversed", true, false),
        ("D2b tris reversed", false, true),
        ("D2c both", true, true),
    ] {
        let s = renumber(&ra, ri, rt);
        assert_eq!(surf_geom(&s), ga);
        let tm = tetrahedralize(&s, &o).unwrap();
        print_quality(n, &tm);
        eprintln!("{n}: equal to base as geometry: {}", cell_geom(&tm) == cell_geom(&base));
    }
    print_quality("D2 STL remeshed", &tetrahedralize(&rb, &o).unwrap());
}

/// Dihedral angle (degrees) at each of the six edges (i, j) of a tetrahedron.
fn dihedrals(p: [Vec3; 4]) -> [((usize, usize), f64); 6] {
    let mut out = [((0, 0), 0.0); 6];
    let mut k = 0;
    for i in 0..4 {
        for j in i + 1..4 {
            let o: Vec<usize> = (0..4).filter(|&x| x != i && x != j).collect();
            let e = unit(sub(p[j], p[i]));
            let perp = |q: Vec3| {
                let d = sub(q, p[i]);
                sub(d, scale(e, dot(d, e)))
            };
            let (u, w) = (perp(p[o[0]]), perp(p[o[1]]));
            out[k] = (
                (i, j),
                (dot(u, w) / (norm(u) * norm(w))).clamp(-1.0, 1.0).acos().to_degrees(),
            );
            k += 1;
        }
    }
    out
}

/// Nearest feature of the cylinder-in-box geometry to a point: (class, distance).
fn geometry_class(c: Vec3) -> (&'static str, f64) {
    let rho = (c[0] * c[0] + c[1] * c[1]).sqrt();
    let rim = ((rho - 0.5).powi(2) + (c[2].abs() - 0.5).powi(2)).sqrt();
    let side = if c[2].abs() <= 0.5 { (rho - 0.5).abs() } else { rim };
    let cap = if rho <= 0.5 { (c[2].abs() - 0.5).abs() } else { rim };
    let mut f = [2.0 - c[0].abs(), 2.0 - c[1].abs(), 1.0 - c[2].abs()];
    f.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let bedge = (f[0] * f[0] + f[1] * f[1]).sqrt();
    for (n, d, lim) in [
        ("cyl rim", rim, 0.15),
        ("box edge", bedge, 0.15),
        ("cyl side", side, 0.3),
        ("cyl cap", cap, 0.3),
        ("box face", f[0], 0.3),
    ] {
        if d < lim {
            return (n, d);
        }
    }
    ("interior", side.min(cap).min(f[0]))
}

/// ADR 0104 D3 table for one mesh; `input` is the surface it was built from.
fn bad_cells(path: &str, s: &TriSurface, tm: &TetMesh) {
    let input: BTreeSet<Key> = s.points.iter().map(|&p| key(p)).collect();
    let mut on_surf = vec![false; tm.points.len()];
    let mut faces = BTreeSet::new();
    let mut sedges = BTreeSet::new();
    for t in &tm.surface.tris {
        let mut k = *t;
        k.sort_unstable();
        faces.insert(k);
        for i in 0..3 {
            on_surf[t[i]] = true;
            let (a, b) = (t[i], t[(i + 1) % 3]);
            sedges.insert((a.min(b), a.max(b)));
        }
    }
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows = Vec::new();
    for c in &tm.cells {
        let p = c.map(|i| tm.points[i]);
        let (_, r) = circumsphere(p);
        let mut lmin = f64::INFINITY;
        for i in 0..4 {
            for j in i + 1..4 {
                lmin = lmin.min(norm(sub(p[i], p[j])));
            }
        }
        let re = r / lmin;
        let d = dihedrals(p);
        let &((i, j), dmin) = d.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
        if dmin >= 3.0 && re <= 2.0 {
            continue;
        }
        let nsf = (0..4)
            .filter(|&o| {
                let mut k = [0usize; 3];
                let mut m = 0;
                for x in 0..4 {
                    if x != o {
                        k[m] = c[x];
                        m += 1;
                    }
                }
                k.sort_unstable();
                faces.contains(&k)
            })
            .count();
        let nsv = c.iter().filter(|&&v| on_surf[v]).count();
        let int_st = c.iter().filter(|&&v| !on_surf[v]).count();
        let surf_st = c
            .iter()
            .filter(|&&v| on_surf[v] && !input.contains(&key(tm.points[v])))
            .count();
        let e = (c[i].min(c[j]), c[i].max(c[j]));
        let cen = scale(add(add(p[0], p[1]), add(p[2], p[3])), 0.25);
        let (cls, dist) = geometry_class(cen);
        let kind = if dmin < 3.0 { "sliver" } else { "re>2" };
        *hist
            .entry(format!(
                "{kind} near {cls}, surface faces {nsf}, surface vertices {nsv}"
            ))
            .or_default() += 1;
        rows.push((dmin, re, nsf, nsv, int_st, surf_st, sedges.contains(&e), cls, dist));
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    eprintln!("D3 {path}: {} failing cells of {}", rows.len(), tm.cells.len());
    for (k, n) in &hist {
        eprintln!("D3 {path}   {n:4} x {k}");
    }
    for r in rows.iter().take(12) {
        eprintln!(
            "D3 {path} worst: dih {:.3} re {:.2} sfaces {} svert {} interior_steiner {} surface_steiner {} min_edge_on_surface {} {} d={:.3}",
            r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8
        );
    }
}

/// ADR 0104 D3: where the cells failing a T5 limit sit (default options, both paths).
#[test]
#[ignore = "ADR 0104 diagnostic D3: prints only"]
fn d3_bad_cells() {
    for (path, s) in [
        ("in-code", curvature_remesh(&in_code())),
        ("STL", curvature_remesh(&from_stl_file())),
    ] {
        let tm = tetrahedralize(&s, &TetOptions::default()).unwrap();
        bad_cells(path, &s, &tm);
    }
}
