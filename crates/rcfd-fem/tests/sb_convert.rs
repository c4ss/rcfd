//! F-371 (ADR 0413, the registration of ADR 0272): a solid mesh from the repository's meshes. Checks C1 to C12, the negative
//! controls N1, N2, N3 (record only, amendment 1), N3b and N4 of the registration.

use rcfd_core::vec3::{cross, norm, sub, Vec3};
use rcfd_fem::builders::{hex_box, upgrade_to_tet10, MidEdgeNode};
use rcfd_fem::convert::{from_mesh, from_tet_mesh, onto_cylinder, project_mid_edge_nodes, split_hexahedra};
use rcfd_fem::{ElementKind, FemError, SolidMesh};
use rcfd_mesh::gen::{annulus_3d, annulus_3d_sector, box_mesh, Grading};
use rcfd_mesh::surface::box_surface;
use rcfd_mesh::tetmesh::{tetrahedralize, TetOptions};
use rcfd_mesh::{BoundarySpec, Mesh};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::PI;

const NAMES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn sorted(v: &[usize]) -> Vec<usize> {
    let mut k = v.to_vec();
    k.sort_unstable();
    k
}

fn volume(m: &SolidMesh) -> f64 {
    (0..m.n_elements()).map(|e| m.element_volume(e)).sum()
}

/// The source's patches: name -> (area, outward area vector per sorted face key).
type Patches = BTreeMap<String, (f64, HashMap<Vec<usize>, Vec3>)>;

fn fv_patches(fv: &Mesh) -> Patches {
    let mut out = BTreeMap::new();
    for p in fv.patches() {
        if p.size == 0 {
            continue;
        }
        let mut area = 0.0;
        let mut faces = HashMap::new();
        for f in p.faces() {
            area += fv.face_area(f);
            faces.insert(sorted(fv.face_points(f)), fv.face_area_vectors()[f]);
        }
        out.insert(p.name.clone(), (area, faces));
    }
    out
}

/// C4 and C5 against a source's patches; returns (max relative patch-area error, max relative area-vector error).
fn check_sets(what: &str, m: &SolidMesh, src: &Patches, closed: &[(&str, f64)]) -> (f64, f64) {
    let names: Vec<&str> = m.surface_set_names().collect();
    let want: Vec<&str> = src.keys().map(|s| s.as_str()).collect();
    assert_eq!(names, want, "{what}: surface set names (C4)");
    let (mut da, mut dn) = (0.0f64, 0.0f64);
    for (name, (area, faces)) in src {
        let set = m.surface_set(name).unwrap();
        assert_eq!(set.len(), faces.len(), "{what} {name}: faces per set (C4)");
        let mut seen = BTreeSet::new();
        let mut a = 0.0;
        for &f in set {
            a += m.face_area(f);
            let key = sorted(&m.face_nodes(f));
            let av = faces.get(&key).unwrap_or_else(|| panic!("{what} {name}: face {key:?} not in the patch"));
            assert!(seen.insert(key), "{what} {name}: face twice");
            dn = dn.max(norm(sub(m.face_area_vector(f), *av)) / norm(*av));
        }
        da = da.max(rel(a, *area));
        if let Some((_, c)) = closed.iter().find(|(n, _)| n == name) {
            da = da.max(rel(a, *c));
        }
    }
    println!("{what}: patch area max rel {da:.3e}, area vector max rel {dn:.3e}");
    assert!(da <= 1e-12, "{what}: C4 patch area {da:e}");
    assert!(dn <= 1e-12, "{what}: C5 area vector {dn:e}");
    (da, dn)
}

/// C1 and the per-element part of C2/C3b.
fn check_counts_and_cells(what: &str, m: &SolidMesh, fv: &Mesh, kind: ElementKind) -> f64 {
    assert_eq!(m.nodes().len(), fv.n_points(), "{what}: C1 nodes");
    for (a, b) in m.nodes().iter().zip(fv.points()) {
        assert!(a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()), "{what}: C1 coordinates");
    }
    assert_eq!(m.n_elements(), fv.n_cells(), "{what}: C1 elements");
    assert_eq!(m.blocks().len(), 1, "{what}: one block");
    assert_eq!(m.blocks()[0].kind(), kind, "{what}: kind");
    let mut d = 0.0f64;
    for (e, &vc) in fv.cell_volumes().iter().enumerate() {
        d = d.max(rel(m.element_volume(e), vc));
    }
    println!("{what}: {} nodes, {} elements, element volume vs cell max rel {d:.3e}", m.nodes().len(), m.n_elements());
    assert!(d <= 1e-12, "{what}: element volumes against the cells {d:e}");
    d
}

fn box_closed() -> Vec<(&'static str, f64)> {
    vec![("xmin", 6.0), ("xmax", 6.0), ("ymin", 3.0), ("ymax", 3.0), ("zmin", 2.0), ("zmax", 2.0)]
}

#[test]
fn c1_c2_c4_c5_box_mesh() {
    for (what, g) in [("B", Grading::Uniform), ("BG", Grading::Expansion(3.0))] {
        let fv = box_mesh([3, 4, 5], [1.0, 2.0, 3.0], [g, Grading::Uniform, Grading::Uniform]).unwrap();
        let m = from_mesh(&fv).unwrap();
        check_counts_and_cells(what, &m, &fv, ElementKind::Hex8);
        let v = rel(volume(&m), 6.0);
        println!("{what}: C2 volume rel {v:.3e}");
        assert!(v <= 1e-12, "{what}: C2 {v:e}");
        check_sets(what, &m, &fv_patches(&fv), &box_closed());
        for name in NAMES {
            assert_eq!(m.node_set(name).unwrap(), m.surface_set_nodes(name).unwrap(), "{what}: node set {name}");
        }
        assert_eq!(m.element_set("all").unwrap().len(), 60);
    }
}

fn chord_volume(n: usize, delta: f64, a: f64, b: f64, h: f64) -> f64 {
    0.5 * h * (b * b - a * a) * n as f64 * delta.sin()
}

#[test]
fn c1_c3_c3b_c4_c5_c12_annulus() {
    // AQ: quarter annulus, 24 chords of pi / 48.
    let mut fv = annulus_3d_sector(24, 3, 2, 1.0, 2.0, 1.0, PI / 2.0, Grading::Uniform).unwrap();
    fv.add_cell_zone("half", (0..72).collect()).unwrap();
    let m = from_mesh(&fv).unwrap();
    assert_eq!(fv.n_points(), 300);
    check_counts_and_cells("AQ", &m, &fv, ElementKind::Hex8);
    let v = volume(&m);
    let (e, ec) = (rel(v, 0.75 * PI), rel(v, chord_volume(24, PI / 48.0, 1.0, 2.0, 1.0)));
    println!("AQ: C3 volume vs pi (b^2 - a^2) h / 4 rel {e:.4e} (estimate 7.138e-4), C3b vs chord closed form {ec:.3e}");
    assert!(e <= 1e-3, "AQ: C3 {e:e}");
    assert!(ec <= 1e-12, "AQ: C3b {ec:e}");
    check_sets("AQ", &m, &fv_patches(&fv), &[]);
    assert_eq!(m.element_set("half").unwrap(), (0..72).collect::<Vec<_>>().as_slice(), "C12");
    // AF: full ring, 96 chords.
    let fv = annulus_3d(96, 2, 2, 1.0, 2.0, 1.0, 0.0, Grading::Uniform).unwrap();
    let m = from_mesh(&fv).unwrap();
    assert_eq!(fv.n_points(), 864);
    check_counts_and_cells("AF", &m, &fv, ElementKind::Hex8);
    let v = volume(&m);
    let (e, ec) = (rel(v, 3.0 * PI), rel(v, chord_volume(96, PI / 48.0, 1.0, 2.0, 1.0)));
    println!("AF: C3 volume vs pi (b^2 - a^2) h rel {e:.4e}, C3b vs chord closed form {ec:.3e}");
    assert!(e <= 1e-3, "AF: C3 {e:e}");
    assert!(ec <= 1e-12, "AF: C3b {ec:e}");
    check_sets("AF", &m, &fv_patches(&fv), &[]);
}

#[test]
fn c4_c5_c6_c11_tetrahedral_mesher() {
    let (lo, hi) = ([0.0; 3], [1.0, 2.0, 3.0]);
    let mut s = box_surface(lo, hi, [2, 3, 4]);
    for (i, t) in s.tris.iter().enumerate() {
        let p: Vec<Vec3> = t.iter().map(|&v| s.points[v]).collect();
        let tag = (0..6)
            .find(|&f| {
                let (d, x) = (f / 2, if f % 2 == 0 { lo[f / 2] } else { hi[f / 2] });
                p.iter().all(|q| q[d] == x)
            })
            .expect("a box face");
        s.tags[i] = tag as u32;
    }
    let tm = tetrahedralize(&s, &TetOptions::default()).unwrap();
    let names: BTreeMap<u32, String> = (0..6).map(|i| (i as u32, NAMES[i].to_string())).collect();
    let m = from_tet_mesh(&tm, &names).unwrap();
    assert_eq!(m.nodes().len(), tm.points.len(), "C6 nodes");
    assert_eq!(m.n_elements(), tm.cells.len(), "C6 elements");
    let v = rel(volume(&m), 6.0);
    println!("T: {} nodes, {} elements, C6 volume rel {v:.3e}", m.nodes().len(), m.n_elements());
    assert!(v <= 1e-12, "C6 {v:e}");
    // The source's outward area vectors: its boundary triangles.
    let mut src: Patches = BTreeMap::new();
    for (t, &tag) in tm.surface.tris.iter().zip(&tm.surface.tags) {
        let p: Vec<Vec3> = t.iter().map(|&v| tm.points[v]).collect();
        let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        let av = [0.5 * c[0], 0.5 * c[1], 0.5 * c[2]];
        let e = src.entry(names[&tag].clone()).or_default();
        e.0 += norm(av);
        e.1.insert(sorted(t), av);
    }
    check_sets("T (from_tet_mesh)", &m, &src, &box_closed());
    // The second route, through the finite-volume mesh of the tetrahedral mesher.
    let fv = tm.to_mesh(&names).unwrap();
    let m2 = from_mesh(&fv).unwrap();
    check_counts_and_cells("T (from_mesh)", &m2, &fv, ElementKind::Tet4);
    assert_eq!(m2.nodes().len(), tm.points.len(), "C6 nodes, route 2");
    let v2 = rel(volume(&m2), 6.0);
    assert!(v2 <= 1e-12, "C6 route 2 {v2:e}");
    check_sets("T (from_mesh)", &m2, &fv_patches(&fv), &box_closed());
    check_sets("T (from_mesh) vs triangles", &m2, &src, &box_closed());
    // C11: the two routes give the same elements.
    let mut d = 0.0f64;
    for e in 0..m.n_elements() {
        assert_eq!(sorted(m.element_nodes(e)), sorted(m2.element_nodes(e)), "C11 element {e}");
        d = d.max(rel(m2.element_volume(e), m.element_volume(e)));
    }
    println!("T: C11 element volumes of the two routes max rel {d:.3e}");
    assert!(d <= 1e-12, "C11 {d:e}");
}

fn single(points: Vec<Vec3>, cells: Vec<Vec<Vec<usize>>>) -> Mesh {
    Mesh::from_cell_faces(points, &cells, &[] as &[BoundarySpec]).unwrap()
}

fn expect_cell(what: &str, r: rcfd_fem::Result<SolidMesh>, cell: usize, shape: &str) {
    let e = r.expect_err(what);
    println!("{what}: {e}");
    match &e {
        FemError::UnsupportedCell { cell: c, shape: s, .. } => assert_eq!((*c, *s), (cell, shape), "{what}"),
        other => panic!("{what}: not UnsupportedCell but {other:?}"),
    }
    let msg = e.to_string();
    assert!(msg.contains(&format!("cell {cell}")) && msg.contains(shape), "{what}: {msg}");
}

#[test]
fn c7_prism_pyramid_polyhedron_and_mixed_refused() {
    let prism = single(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
        vec![vec![vec![0, 1, 2], vec![3, 4, 5], vec![0, 1, 4, 3], vec![1, 2, 5, 4], vec![2, 0, 3, 5]]],
    );
    expect_cell("prism", from_mesh(&prism), 0, "prism");
    let pyramid = single(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.5, 0.5, 1.0]],
        vec![vec![vec![0, 1, 2, 3], vec![0, 1, 4], vec![1, 2, 4], vec![2, 3, 4], vec![3, 0, 4]]],
    );
    expect_cell("pyramid", from_mesh(&pyramid), 0, "pyramid");
    let mut pts = Vec::new();
    for z in [0.0, 1.0] {
        for i in 0..6 {
            let t = PI / 3.0 * i as f64;
            pts.push([t.cos(), t.sin(), z]);
        }
    }
    let mut faces = vec![(0..6).collect::<Vec<_>>(), (6..12).collect()];
    for i in 0..6 {
        let j = (i + 1) % 6;
        faces.push(vec![i, j, j + 6, i + 6]);
    }
    expect_cell("hexagonal prism", from_mesh(&single(pts, vec![faces])), 0, "polyhedron");
    // Mixed: a tetrahedron (cell 0) and a hexahedron (cell 1), not touching.
    let mut pts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for c in 0..8 {
        pts.push([3.0 + (c & 1) as f64, ((c >> 1) & 1) as f64, (c >> 2) as f64]);
    }
    let p = |c: usize| 4 + c;
    let hex = vec![
        vec![p(0), p(2), p(6), p(4)],
        vec![p(1), p(3), p(7), p(5)],
        vec![p(0), p(1), p(5), p(4)],
        vec![p(2), p(3), p(7), p(6)],
        vec![p(0), p(1), p(3), p(2)],
        vec![p(4), p(5), p(7), p(6)],
    ];
    let tet = vec![vec![0, 1, 2], vec![0, 1, 3], vec![1, 2, 3], vec![0, 2, 3]];
    let mixed = single(pts, vec![tet, hex]);
    expect_cell("mixed", from_mesh(&mixed), 1, "hexahedron in a mesh of tetrahedra");
}

/// One level of CQ: (volume, max radius error, projected nodes, elements, the mesh and its mid-edge nodes).
fn curved_level(k: usize, project: bool) -> (f64, f64, usize, usize, SolidMesh, Vec<MidEdgeNode>) {
    let fv = annulus_3d_sector(4 * k, k, k, 1.0, 2.0, 1.0, PI / 2.0, Grading::Uniform).unwrap();
    let tets = split_hexahedra(&from_mesh(&fv).unwrap()).unwrap();
    let (q, mids) = upgrade_to_tet10(&tets).unwrap();
    let inner: BTreeSet<usize> = q.surface_set_nodes("inner").unwrap().into_iter().collect();
    let outer: BTreeSet<usize> = q.surface_set_nodes("outer").unwrap().into_iter().collect();
    let mut moved = Vec::new();
    let m = project_mid_edge_nodes(&q, &mids, |mid, x| {
        if !project {
            return None;
        }
        let r = if inner.contains(&mid.node) {
            1.0
        } else if outer.contains(&mid.node) {
            2.0
        } else {
            return None;
        };
        moved.push((mid.node, r));
        Some(onto_cylinder(x, [0.0; 3], [0.0, 0.0, 1.0], r))
    })
    .unwrap();
    let mut dr = 0.0f64;
    for &(n, r) in &moved {
        let p = m.nodes()[n];
        dr = dr.max((p[0].hypot(p[1]) - r).abs());
    }
    (volume(&m), dr, moved.len(), m.n_elements(), m, mids)
}

fn orders(e: &[f64]) -> Vec<f64> {
    e.windows(2).map(|w| (w[0] / w[1]).log2()).collect()
}

#[test]
fn c8_c9_c10_n1_n2_curved_quarter_annulus() {
    let exact = 0.75 * PI;
    let (mut e, mut e0) = (Vec::new(), Vec::new());
    for k in [1usize, 2, 4, 8] {
        let (v, dr, n, ne, _, _) = curved_level(k, true);
        assert_eq!(ne, 24 * k * k * k, "C10 elements");
        let (v0, _, n0, _, _, _) = curved_level(k, false);
        assert_eq!(n0, 0);
        let delta = PI / (8 * k) as f64;
        let ec = rel(v0, chord_volume(4 * k, delta, 1.0, 2.0, 1.0));
        println!(
            "CQ k={k}: {ne} Tet10, {n} nodes projected, radius err max {dr:.3e}; volume err projected {:.4e}, \
             unprojected {:.4e} (unprojected vs chord closed form {ec:.3e})",
            rel(v, exact),
            rel(v0, exact)
        );
        assert!(dr <= 1e-14, "C8 k={k}: {dr:e}");
        assert!(ec <= 1e-12, "N2 k={k}: {ec:e}");
        e.push(rel(v, exact));
        e0.push(rel(v0, exact));
    }
    let (p, p0) = (orders(&e), orders(&e0));
    println!("CQ: C9 observed orders projected {p:?}; N1 unprojected {p0:?}");
    for (i, o) in p.iter().enumerate() {
        assert!(*o >= 3.0, "C9 pair {i}: order {o}");
    }
    for (i, o) in p0.iter().enumerate() {
        assert!((1.9..=2.1).contains(o), "N1 pair {i}: order {o}");
    }
}

#[test]
fn n3_record_radial_inward_shift() {
    // Registered N3, a record only (amendment 1): every inner-wall mid-edge node of level 1 moved by -0.6 radially.
    let (_, _, _, _, m, mids) = curved_level(1, true);
    let inner: BTreeSet<usize> = m.surface_set_nodes("inner").unwrap().into_iter().collect();
    let r = project_mid_edge_nodes(&m, &mids, |mid, x| {
        inner
            .contains(&mid.node)
            .then(|| onto_cylinder(x, [0.0; 3], [0.0, 0.0, 1.0], x[0].hypot(x[1]) - 0.6))
    });
    match r {
        Ok(_) => println!("N3 (record): accepted, no element inverted"),
        Err(e) => println!("N3 (record): refused: {e}"),
    }
}

#[test]
fn n3b_projection_that_inverts_is_refused() {
    let (_, _, _, _, m, mids) = curved_level(1, true);
    let inner: BTreeSet<usize> = m.surface_set_nodes("inner").unwrap().into_iter().collect();
    let first = *mids.iter().find(|mid| inner.contains(&mid.node)).unwrap();
    let (xa, xb) = (m.nodes()[first.ends[0]], m.nodes()[first.ends[1]]);
    let r = project_mid_edge_nodes(&m, &mids, |mid, x| {
        (mid.node == first.node).then(|| {
            let d = sub(xb, xa);
            [x[0] + 0.4 * d[0], x[1] + 0.4 * d[1], x[2] + 0.4 * d[2]]
        })
    });
    let e = r.expect_err("N3b");
    println!("N3b: {e}");
    match e {
        FemError::InvertedElement {
            element,
            local_node,
            jacobian,
        } => {
            assert!(jacobian < 0.0, "N3b jacobian {jacobian}");
            assert_eq!(m.element_nodes(element)[local_node], first.ends[1], "N3b at the end b");
        }
        other => panic!("N3b: not InvertedElement but {other:?}"),
    }
}

#[test]
fn n4_non_conforming_split_is_refused() {
    let m = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let ok = split_hexahedra(&m).unwrap();
    assert_eq!(ok.n_elements(), 12);
    // Labels 0 and 5 exchanged: the shared face x = 1 is cut 1-10 by the first cell and 4-7 by the second.
    let pi = |v: usize| match v {
        0 => 5,
        5 => 0,
        v => v,
    };
    let mut nodes = m.nodes().to_vec();
    for (o, p) in m.nodes().iter().enumerate() {
        nodes[pi(o)] = *p;
    }
    let mut r = SolidMesh::new(nodes).unwrap();
    r.add_block(ElementKind::Hex8, m.blocks()[0].connectivity().iter().map(|&v| pi(v)).collect())
        .unwrap();
    let e = split_hexahedra(&r).expect_err("N4");
    println!("N4: {e}");
    match e {
        FemError::InvalidValue { key, reason } => {
            assert_eq!(key, "split_hexahedra");
            assert!(reason.starts_with("element 0") || reason.starts_with("element 1"), "{reason}");
        }
        other => panic!("N4: {other:?}"),
    }
}
