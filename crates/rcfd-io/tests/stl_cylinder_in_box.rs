//! F-131 (ADR 0093 S1 to S3): a closed STL file of a cylinder in a box, read back, remeshed with curvature sizing and
//! tetrahedralized. Limits are those registered in docs/adr/0093-wave39b-partials-closeout-mesh-and-schemes.md before
//! this file was run (the smoke test excepted).

use rcfd_core::vec3::{cross, sub, unit};
use rcfd_io::stl::{self, StlSolid};
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::surface::{box_surface, cylinder, diagnose, DiagOptions, TriSurface};
use rcfd_mesh::surface_remesh::{remesh, RemeshOptions, SizeField};
use rcfd_mesh::tetmesh::{tet_quality, tetrahedralize, TetOptions, TetQuality};
use std::collections::BTreeMap;

/// The same geometry as `cyl_in_box` in crates/rcfd-mesh/tests/tetmesh.rs: box tag 0, inward-facing cylinder tag 1.
fn parts() -> (TriSurface, TriSurface) {
    let b = box_surface([-2.0, -2.0, -1.0], [2.0, 2.0, 1.0], [4, 4, 2]);
    let mut c = cylinder(0.5, 1.0, 16, 4);
    c.flip_all();
    (b, c)
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

/// Write both solids to one ASCII STL file, read the file back and weld each solid (bitwise, tol 0).
fn from_stl_file() -> TriSurface {
    let (b, c) = parts();
    let text = stl::write_ascii(&solid("box", &b)).unwrap() + &stl::write_ascii(&solid("cylinder", &c)).unwrap();
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("w39b_cylinder_in_box.stl");
    std::fs::write(&path, text).unwrap();
    let back = stl::read_file(&path).unwrap();
    assert_eq!(back.len(), 2, "two solids in the file");
    let mut s = TriSurface::from_soup(&back[0].triangles, 0.0);
    let mut cyl = TriSurface::from_soup(&back[1].triangles, 0.0);
    cyl.set_tag(1);
    s.append(&cyl);
    s
}

fn pipeline() -> (TriSurface, TriSurface, rcfd_mesh::Mesh, TetQuality) {
    let s = from_stl_file();
    let f = SizeField::Curvature {
        theta: 0.5,
        h_min: 0.1,
        h_max: 1.0,
    };
    let (r, rep) = remesh(&s, &f, &RemeshOptions::default()).unwrap();
    let tm = tetrahedralize(&r, &TetOptions::default()).expect("tetrahedralize");
    let m = tm.to_mesh(&BTreeMap::new()).expect("mesh");
    let q = tet_quality(&tm, 2.0);
    eprintln!(
        "S2: remesh {rep:?}; {} surface tris, {} cells, {:?}, {q:?}",
        r.n_tris(),
        m.n_cells(),
        tm.report
    );
    (s, r, m, q)
}

/// Set-up smoke (run before ADR 0093 registered its limits): construction and read-back only, prints.
#[test]
#[ignore = "set-up smoke for ADR 0093 S1; prints only"]
fn s0_smoke() {
    let s = from_stl_file();
    let (b, c) = parts();
    eprintln!(
        "S0: {} points {} tris, {:?}, in-code volume {}",
        s.points.len(),
        s.n_tris(),
        diagnose(&s, &DiagOptions::default()),
        b.enclosed_volume() + c.enclosed_volume()
    );
}

/// S1: the STL file is closed: two solids, closed manifold, Euler characteristic 4 in 2 components, area and volume equal
/// to the in-code surface to 1e-12 relative (the ASCII round trip is bit exact).
#[test]
fn s1_stl_file_is_closed() {
    let s = from_stl_file();
    let (b, c) = parts();
    let d = diagnose(&s, &DiagOptions::default());
    assert!(d.is_closed_manifold(), "{d:?}");
    assert_eq!((d.euler, d.components), (4, 2));
    let (a0, v0) = (b.area() + c.area(), b.enclosed_volume() + c.enclosed_volume());
    assert!((s.area() - a0).abs() <= 1e-12 * a0, "area {} vs {a0}", s.area());
    assert!(
        (s.enclosed_volume() - v0).abs() <= 1e-12 * v0,
        "volume {} vs {v0}",
        s.enclosed_volume()
    );
    assert_eq!(s.tags.iter().filter(|&&t| t == 1).count(), c.n_tris());
}

/// S2: curvature-sized remesh of the STL surface, then tetrahedralization: closed manifold remeshed surface, volume
/// mesh volume equal to the remeshed surface volume to 1e-10, two patches, `check_mesh` without errors, every cell volume
/// positive and the minimum dihedral angle at least 3.0 degrees (the ADR 0030 T5 limit).
/// Known miss: every check passed except the last one: minimum dihedral 1.84 degrees (the in-code surface of `p1` gives
/// 3.34; the STL surface has the same coordinates but another vertex numbering; that the remesh depends on it is likely
/// but not isolated). ADR 0104 D1 isolated it: the remesh depends on the numbering, the tetrahedral mesher does not. This
/// record keeps the default mesher; the same limits with the opt-in quality pass pass in
/// crates/rcfd-io/tests/w41d_tet_quality.rs (`q1_stl_watertight_and_min_dihedral_with_pass`).
#[test]
#[ignore = "known miss (ADR 0093 S2): minimum dihedral 1.84 deg vs registered 3.0 deg"]
fn s2_known_miss_pipeline_watertight_and_quality() {
    let (_, r, m, q) = pipeline();
    assert!(diagnose(&r, &DiagOptions::default()).is_closed_manifold());
    let e = (m.total_volume() - r.enclosed_volume()).abs() / r.enclosed_volume();
    assert!(e <= 1e-10, "volume error {e}");
    assert_eq!(m.patches().len(), 2);
    assert!(m.cell_volumes().iter().all(|&v| v > 0.0) && q.min_volume > 0.0);
    let rep = check_mesh(&m);
    assert!(rep.is_ok(), "{:?}", rep.errors);
    assert!(q.min_dihedral_deg >= 3.0, "min dihedral {}", q.min_dihedral_deg);
}

/// S3: the ADR 0030 T5 radius-edge limit: at least 99 percent of the cells with circumradius over shortest edge at most 2.
/// Wave 19B measured 0.927 on the in-code surface, so a miss was expected; no quality refinement is added here.
/// Known miss: measured 0.936 (default mesher). With the ADR 0104 quality pass the same limit passes:
/// `q2_stl_radius_edge_fraction_with_pass` in crates/rcfd-io/tests/w41d_tet_quality.rs.
#[test]
#[ignore = "known miss (ADR 0093 S3): radius-edge fraction 0.936 vs registered 0.99"]
fn s3_known_miss_radius_edge_fraction() {
    let (_, _, _, q) = pipeline();
    assert!(
        q.frac_radius_edge_ok >= 0.99,
        "radius-edge fraction {}",
        q.frac_radius_edge_ok
    );
}

/// ADR 0093 amendment, added after the S2 miss (not a registered test of its own): the S2 checks that passed in the
/// registered run, i.e. S2 without the dihedral limit, kept as a running watertightness check.
#[test]
fn s2b_pipeline_watertight_and_check_mesh() {
    let (_, r, m, q) = pipeline();
    assert!(diagnose(&r, &DiagOptions::default()).is_closed_manifold());
    let e = (m.total_volume() - r.enclosed_volume()).abs() / r.enclosed_volume();
    assert!(e <= 1e-10, "volume error {e}");
    assert_eq!(m.patches().len(), 2);
    assert!(m.cell_volumes().iter().all(|&v| v > 0.0) && q.min_volume > 0.0);
    let rep = check_mesh(&m);
    assert!(rep.is_ok(), "{:?}", rep.errors);
}
