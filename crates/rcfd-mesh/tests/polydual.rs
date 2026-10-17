//! F-102 (ADR 0054 check A5): polyhedral dual of tetrahedral meshes. Limits registered before the first run.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::polydual::tet_to_polydual;
use rcfd_mesh::surface::{icosphere, TriSurface};
use rcfd_mesh::tetmesh::{tetrahedralize, TetMesh, TetOptions, TetReport};
use rcfd_mesh::Mesh;
use std::collections::{BTreeMap, HashMap};

fn tet_volume(p: &[Vec3], t: [usize; 4]) -> f64 {
    let (a, b, c, d) = (p[t[0]], p[t[1]], p[t[2]], p[t[3]]);
    vec3::dot(vec3::cross(vec3::sub(b, a), vec3::sub(c, a)), vec3::sub(d, a)) / 6.0
}

/// TetMesh from points and tets; free faces become the surface (tag = side of the box 1..6 by the dominant normal component).
fn make_tetmesh(points: Vec<Vec3>, mut tets: Vec<[usize; 4]>) -> TetMesh {
    for t in tets.iter_mut() {
        if tet_volume(&points, *t) < 0.0 {
            t.swap(0, 1);
        }
        assert!(tet_volume(&points, *t) > 0.0);
    }
    let mut faces: HashMap<[usize; 3], Vec<(usize, usize)>> = HashMap::new();
    for (i, t) in tets.iter().enumerate() {
        for a in 0..4 {
            let f: Vec<usize> = (0..4).filter(|&k| k != a).map(|k| t[k]).collect();
            let mut k = [f[0], f[1], f[2]];
            k.sort_unstable();
            faces.entry(k).or_default().push((i, t[a]));
        }
    }
    let mut tris = Vec::new();
    let mut tags = Vec::new();
    let mut keys: Vec<_> = faces.iter().filter(|(_, v)| v.len() == 1).collect();
    keys.sort();
    for (k, v) in keys {
        let (_, opp) = v[0];
        let mut tri = *k;
        let n = vec3::cross(
            vec3::sub(points[tri[1]], points[tri[0]]),
            vec3::sub(points[tri[2]], points[tri[0]]),
        );
        if vec3::dot(n, vec3::sub(points[opp], points[tri[0]])) > 0.0 {
            tri.swap(1, 2);
        }
        let n = vec3::unit(vec3::cross(
            vec3::sub(points[tri[1]], points[tri[0]]),
            vec3::sub(points[tri[2]], points[tri[0]]),
        ));
        let a = (0..3)
            .max_by(|&i, &j| n[i].abs().partial_cmp(&n[j].abs()).unwrap())
            .unwrap();
        tags.push(1 + 2 * a as u32 + (n[a] > 0.0) as u32);
        tris.push(tri);
    }
    TetMesh {
        points: points.clone(),
        cells: tets,
        surface: TriSurface { points, tris, tags },
        report: TetReport::default(),
    }
}

/// Kuhn split of a graded box: six tets per cube around the diagonal 0 -> 7, conforming across cubes.
fn kuhn(nx: usize, ny: usize, nz: usize) -> (TetMesh, f64) {
    let xs: Vec<f64> = (0..=nx).map(|i| (i as f64 / nx as f64).powf(1.3) * 1.2).collect();
    let ys: Vec<f64> = (0..=ny).map(|i| i as f64 / ny as f64 * 0.9).collect();
    let zs: Vec<f64> = (0..=nz).map(|i| (i as f64 / nz as f64).powf(0.8) * 0.7).collect();
    let id = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
    let mut pts = Vec::new();
    for k in 0..=nz {
        for j in 0..=ny {
            for i in 0..=nx {
                pts.push([xs[i], ys[j], zs[k]]);
            }
        }
    }
    let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let mut tets = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                for p in perms {
                    let mut c = [i, j, k];
                    let mut v = [id(c[0], c[1], c[2]); 4];
                    for (s, &ax) in p.iter().enumerate() {
                        c[ax] += 1;
                        v[s + 1] = id(c[0], c[1], c[2]);
                    }
                    tets.push(v);
                }
            }
        }
    }
    (make_tetmesh(pts, tets), 1.2 * 0.9 * 0.7)
}

fn check_dual(tm: &TetMesh, mesh: &Mesh, what: &str) {
    mesh.validate().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(mesh.n_cells(), tm.points.len(), "{what}: one cell per vertex");
    let vt: f64 = tm.cells.iter().map(|&t| tet_volume(&tm.points, t)).sum();
    let vp: f64 = mesh.cell_volumes().iter().sum();
    println!(
        "A5 {what}: {} cells, {} faces, volume {vp:.15} vs {vt:.15}",
        mesh.n_cells(),
        mesh.n_faces()
    );
    assert!((vp - vt).abs() <= 1e-13 * vt, "{what}: volume {vp} vs {vt}");
    let mut count = vec![0usize; mesh.n_faces()];
    for c in 0..mesh.n_cells() {
        let (mut s, mut m) = (vec3::ZERO, 0.0);
        for &f in mesh.cell_faces(c) {
            count[f] += 1;
            let sg = if mesh.owner()[f] == c { 1.0 } else { -1.0 };
            s = vec3::add(s, vec3::scale(mesh.face_area_vectors()[f], sg));
            m += mesh.face_area(f);
        }
        assert!(
            vec3::norm(s) <= 1e-13 * m,
            "{what}: cell {c} closure {}",
            vec3::norm(s) / m
        );
    }
    for f in 0..mesh.n_faces() {
        assert_eq!(
            count[f],
            if f < mesh.n_internal_faces() { 2 } else { 1 },
            "{what}: face {f}"
        );
    }
    // per-tag patch area
    let mut want: BTreeMap<u32, f64> = BTreeMap::new();
    for (t, tag) in tm.surface.tris.iter().zip(&tm.surface.tags) {
        let a = vec3::norm(vec3::cross(
            vec3::sub(tm.points[t[1]], tm.points[t[0]]),
            vec3::sub(tm.points[t[2]], tm.points[t[0]]),
        )) * 0.5;
        *want.entry(*tag).or_default() += a;
    }
    for (tag, a) in want {
        let name = if tag == 0 {
            "wall".to_string()
        } else {
            format!("wall{tag}")
        };
        let p = mesh.patch(&name).unwrap_or_else(|| panic!("{what}: patch {name}"));
        let got: f64 = p.faces().map(|f| mesh.face_area(f)).sum();
        assert!((got - a).abs() <= 1e-13 * a, "{what}: patch {name} area {got} vs {a}");
    }
}

#[test]
fn a5_dual_of_kuhn_box() {
    let (tm, vol) = kuhn(4, 3, 3);
    let mesh = tet_to_polydual(&tm, &BTreeMap::new()).unwrap();
    check_dual(&tm, &mesh, "kuhn box");
    let vp: f64 = mesh.cell_volumes().iter().sum();
    assert!((vp - vol).abs() <= 1e-13 * vol);
    // interior cells are true polyhedra with many faces
    let max_faces = (0..mesh.n_cells()).map(|c| mesh.cell_faces(c).len()).max().unwrap();
    println!("A5 kuhn box: max faces per dual cell {max_faces}");
    assert!(max_faces > 6);
}

#[test]
fn a5_dual_of_tetrahedralised_sphere() {
    let s = icosphere(1.0, 2);
    let tm = tetrahedralize(&s, &TetOptions::default()).unwrap();
    let mesh = tet_to_polydual(&tm, &BTreeMap::new()).unwrap();
    check_dual(&tm, &mesh, "sphere");
}

#[test]
fn a5_bad_inputs() {
    let (mut tm, _) = kuhn(2, 2, 2);
    // vertex used by no tetrahedron
    tm.points.push([5.0, 5.0, 5.0]);
    let e = tet_to_polydual(&tm, &BTreeMap::new()).unwrap_err().to_string();
    assert!(e.contains("used by no tetrahedron"), "{e}");
    // a face shared by three tetrahedra
    let (mut tm, _) = kuhn(2, 2, 2);
    let t0 = tm.cells[0];
    let mut dup = t0;
    dup.swap(0, 1);
    tm.cells.push(t0);
    tm.cells.push(dup);
    let e = tet_to_polydual(&tm, &BTreeMap::new()).unwrap_err().to_string();
    assert!(e.contains("shared by"), "{e}");
    // surface not matching the free faces
    let (mut tm, _) = kuhn(2, 2, 2);
    tm.surface.tris.pop();
    tm.surface.tags.pop();
    let e = tet_to_polydual(&tm, &BTreeMap::new()).unwrap_err().to_string();
    assert!(e.contains("free tetrahedron faces"), "{e}");
}
