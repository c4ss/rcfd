//! Registered checks E1 to E5 of ADR 0018 (F-231): mesh transforms, merge and copy, extrusion,
//! orientation repair and patch operations. Limits are those registered in the ADR.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::edit::{
    copy_by_offset, extrude_patch, merge_meshes, merge_patches, repair_face_orientation, repair_mesh, separate_patch,
    Affine, ExtrudeDir,
};
use rcfd_mesh::gen::{box_2d, box_mesh, cylinder_ogrid, warp, Grading};
use rcfd_mesh::measures::compute_measures;
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::{check_mesh, compute_metrics};

const U: [Grading; 3] = [Grading::Uniform; 3];

fn rel(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol * b.abs().max(1e-300),
        "{what}: {a} vs {b} (rel tol {tol})"
    );
}

fn patch_area(m: &Mesh, name: &str) -> f64 {
    m.patch(name)
        .unwrap_or_else(|| panic!("no patch {name}"))
        .faces()
        .map(|f| vec3::norm(m.face_area_vectors()[f]))
        .sum()
}

fn bases() -> Vec<(&'static str, Mesh)> {
    let graded = box_mesh(
        [4, 3, 2],
        [2.0, 1.5, 1.0],
        [Grading::Expansion(3.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let warped = warp(&graded, |p| {
        [
            p[0] + 0.1 * (2.0 * std::f64::consts::PI * p[1]).sin(),
            p[1],
            p[2] + 0.05 * p[0] * p[1],
        ]
    })
    .unwrap();
    vec![
        ("warped graded box", warped),
        (
            "o-grid",
            cylinder_ogrid(16, 4, 1.0, 2.0, Grading::Uniform, 1.0).unwrap(),
        ),
        ("unit-spaced box", box_mesh([2, 2, 2], [2.0, 2.0, 2.0], U).unwrap()),
    ]
}

fn maps() -> Vec<(&'static str, Affine, bool, f64)> {
    // (name, map, rigid-or-uniform (dimensionless measures invariant), uniform scale factor squared for areas)
    let rot = Affine::rotation([1.0, 2.0, 3.0], 0.7, [0.3, -0.2, 0.5]).unwrap();
    let sc = Affine::scaling([2.5, 2.5, 2.5]);
    let mir_x = Affine::mirror([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]).unwrap();
    let mir_g = Affine::mirror([1.0, -2.0, 0.5], [0.2, 0.1, -0.3]).unwrap();
    let comp = rot.then(&sc).then(&Affine::translation([1.0, -2.0, 3.0]));
    vec![
        ("translation", Affine::translation([3.0, -1.5, 0.25]), true, 1.0),
        ("rotation", rot, true, 1.0),
        ("uniform scale 2.5", sc, true, 6.25),
        ("anisotropic scale", Affine::scaling([2.0, 0.5, 3.0]), false, 0.0),
        ("mirror x", mir_x, true, 1.0),
        ("mirror general plane", mir_g, true, 1.0),
        ("rotation then scale then translation", comp, true, 6.25),
    ]
}

#[test]
fn e1_transforms_keep_volumes_and_areas_exact() {
    let mut worst_v = 0.0f64;
    let mut worst_s = 0.0f64;
    for (bname, base) in bases() {
        for (mname, a, invariant, area_factor) in maps() {
            let m = transform_checked(&base, &a, bname, mname);
            let det = a.det();
            // total and per-cell volume
            rel(m.total_volume(), det.abs() * base.total_volume(), 1e-12, "total volume");
            for c in 0..base.n_cells() {
                let e = (m.cell_volumes()[c] / (det.abs() * base.cell_volumes()[c]) - 1.0).abs();
                worst_v = worst_v.max(e);
                assert!(e <= 1e-12, "{bname} / {mname}: cell {c} volume ratio error {e}");
            }
            // area vectors: S' = sign(det) cof(M) S (the loops of a mirror are reversed)
            let cof = a.cofactor();
            let sgn = det.signum();
            for f in 0..base.n_faces() {
                let s = base.face_area_vectors()[f];
                let want = [
                    sgn * (cof[0][0] * s[0] + cof[0][1] * s[1] + cof[0][2] * s[2]),
                    sgn * (cof[1][0] * s[0] + cof[1][1] * s[1] + cof[1][2] * s[2]),
                    sgn * (cof[2][0] * s[0] + cof[2][1] * s[1] + cof[2][2] * s[2]),
                ];
                let got = m.face_area_vectors()[f];
                let e = vec3::norm(vec3::sub(got, want)) / vec3::norm(want);
                worst_s = worst_s.max(e);
                assert!(e <= 1e-12, "{bname} / {mname}: face {f} area vector error {e}");
            }
            // rigid maps and uniform scaling: areas scale by the known factor; patch areas follow
            if invariant {
                for p in base.patches() {
                    rel(
                        patch_area(&m, &p.name),
                        area_factor * patch_area(&base, &p.name),
                        1e-12,
                        "patch area",
                    );
                }
                // dimensionless quality is invariant
                // (the non-orthogonality part of this registered check is in
                // `e1_non_orthogonality_invariance`)
                let (m0, m1) = (compute_metrics(&base), compute_metrics(&m));
                for (x, y) in m0.aspect_ratio.iter().zip(&m1.aspect_ratio) {
                    assert!(
                        (x - y).abs() <= 1e-9 * x.abs().max(1.0),
                        "{bname} / {mname}: aspect ratio"
                    );
                }
                let (q0, q1) = (compute_measures(&base), compute_measures(&m));
                for (x, y) in q0.cell_inverse_orthogonal.iter().zip(&q1.cell_inverse_orthogonal) {
                    assert!((x - y).abs() <= 1e-9, "{bname} / {mname}: inverse orthogonal");
                }
            }
        }
    }
    println!("E1 worst cell volume ratio error {worst_v:.3e}, worst area vector error {worst_s:.3e}");
}

/// Registered limit (ADR 0018 E1): the existing non-orthogonality is unchanged by rigid and uniform maps to
/// 1e-9 (absolute below 1 degree, relative above). This was first a known miss (1.479e-6 degrees) because
/// `compute_metrics` evaluated `acos(cos)`, conditioning 1/sin(angle); it now uses `atan2(|d x n|, d . n)` and the
/// assertion below is the registered one, unchanged. See docs/mesh-quality.md (post-hoc note).
#[test]
fn e1_non_orthogonality_invariance() {
    for (bname, base) in bases() {
        for (mname, a, invariant, _) in maps() {
            if !invariant {
                continue;
            }
            let m = transform_checked(&base, &a, bname, mname);
            let (m0, m1) = (compute_metrics(&base), compute_metrics(&m));
            for (x, y) in m0.non_orthogonality.iter().zip(&m1.non_orthogonality) {
                assert!(
                    (x - y).abs() <= 1e-9 * x.abs().max(1.0),
                    "{bname} / {mname}: non-orthogonality {x} vs {y}"
                );
            }
        }
    }
}

/// Diagnosis (print only, no pass criterion), kept as a record: before the atan2 form the change was rounding noise of
/// order sqrt(machine epsilon) in radians (1.479e-6 degrees, ADR results); it is now 3.5e-13 degrees.
#[test]
fn e1_non_orthogonality_noise_diagnosis() {
    let mut worst = 0.0f64;
    for (bname, base) in bases() {
        for (mname, a, invariant, _) in maps() {
            if !invariant {
                continue;
            }
            let m = transform_checked(&base, &a, bname, mname);
            let (m0, m1) = (compute_metrics(&base), compute_metrics(&m));
            for (x, y) in m0.non_orthogonality.iter().zip(&m1.non_orthogonality) {
                worst = worst.max((x - y).abs());
            }
        }
    }
    println!("E1 worst non-orthogonality change {worst:.3e} degrees");
}

fn transform_checked(base: &Mesh, a: &Affine, bname: &str, mname: &str) -> Mesh {
    let m = rcfd_mesh::edit::transform_mesh(base, a).unwrap_or_else(|e| panic!("{bname} / {mname}: {e}"));
    m.validate().unwrap();
    assert_eq!(m.owner(), base.owner());
    assert_eq!(m.neighbour(), base.neighbour());
    assert_eq!(m.patches(), base.patches());
    if a.det() < 0.0 {
        for f in 0..base.n_faces() {
            let mut rev: Vec<usize> = base.face_points(f).to_vec();
            rev.reverse();
            assert_eq!(
                m.face_points(f),
                &rev[..],
                "{bname} / {mname}: loop of face {f} not reversed"
            );
        }
    }
    m
}

#[test]
fn e1_closed_form_patch_areas_after_anisotropic_scaling() {
    let b = box_mesh([2, 2, 2], [2.0, 1.5, 1.0], U).unwrap();
    let m = rcfd_mesh::edit::transform_mesh(&b, &Affine::scaling([2.0, 0.5, 3.0])).unwrap();
    rel(patch_area(&m, "xmin"), 1.5 * 1.0 * 0.5 * 3.0, 1e-12, "xmin");
    rel(patch_area(&m, "ymin"), 2.0 * 1.0 * 2.0 * 3.0, 1e-12, "ymin");
    rel(patch_area(&m, "zmin"), 2.0 * 1.5 * 2.0 * 0.5, 1e-12, "zmin");
    assert!(rcfd_mesh::edit::transform_mesh(&b, &Affine::scaling([1.0, 0.0, 1.0])).is_err());
}

fn counts(m: &Mesh) -> (usize, usize, usize) {
    (m.n_cells(), m.n_internal_faces(), m.n_boundary_faces())
}

#[test]
fn e2_merge_and_copy() {
    let unit = box_mesh([2, 2, 2], [1.0, 1.0, 1.0], U).unwrap();
    let shifted = rcfd_mesh::edit::transform_mesh(&unit, &Affine::translation([1.0, 0.0, 0.0])).unwrap();
    // (a) stitched
    let m = merge_meshes(&unit, &shifted, 1e-9).unwrap();
    assert_eq!(counts(&m), (16, 28, 40));
    rel(m.total_volume(), 2.0, 1e-12, "volume");
    for (name, area) in [
        ("xmin", 1.0),
        ("xmax", 1.0),
        ("ymin", 2.0),
        ("ymax", 2.0),
        ("zmin", 2.0),
        ("zmax", 2.0),
    ] {
        rel(patch_area(&m, name), area, 1e-12, name);
    }
    assert_eq!(m.patches().len(), 6);
    assert!(check_mesh(&m).is_ok());
    let reference = box_mesh([4, 2, 2], [2.0, 1.0, 1.0], U).unwrap();
    assert_eq!(counts(&m), counts(&reference));
    // (b) gap
    let far = rcfd_mesh::edit::transform_mesh(&unit, &Affine::translation([1.5, 0.0, 0.0])).unwrap();
    let g = merge_meshes(&unit, &far, 1e-9).unwrap();
    assert_eq!(counts(&g), (16, 24, 48));
    rel(g.total_volume(), 2.0, 1e-12, "volume");
    assert_eq!(g.patch("xmin").unwrap().size, 8);
    assert_eq!(g.patch("xmax").unwrap().size, 8);
    // (c) copy by offset
    let c = copy_by_offset(&unit, 3, [1.0, 0.0, 0.0], 1e-9).unwrap();
    assert_eq!(c.n_cells(), 32);
    rel(c.total_volume(), 4.0, 1e-12, "volume");
    assert!(check_mesh(&c).is_ok());
    assert_eq!(
        c.n_internal_faces(),
        box_mesh([8, 2, 2], [4.0, 1.0, 1.0], U).unwrap().n_internal_faces()
    );
    // (d) cell zones combine with offset ids
    let mut a = unit.clone();
    let mut b = shifted.clone();
    a.add_cell_zone("z", vec![0, 1]).unwrap();
    b.add_cell_zone("z", vec![0]).unwrap();
    let m = merge_meshes(&a, &b, 1e-9).unwrap();
    assert_eq!(m.cell_zone("z").unwrap().ids, vec![0, 1, 8]);
    assert!(merge_meshes(&unit, &shifted, 0.0).is_err());
}

#[test]
fn e3_extrude() {
    let base = box_mesh([3, 2, 1], [3.0, 2.0, 1.0], U).unwrap();
    let m = extrude_patch(&base, "zmax", ExtrudeDir::Vector([0.0, 0.0, 2.0]), 4).unwrap();
    assert_eq!(m.n_cells(), 6 + 6 * 4);
    rel(m.total_volume(), 18.0, 1e-12, "volume");
    rel(patch_area(&m, "zmax"), 6.0, 1e-12, "cap");
    rel(patch_area(&m, "zmax_side"), 20.0, 1e-12, "side");
    assert!(check_mesh(&m).is_ok());
    // Normal extrusion of a planar patch equals the vector extrusion
    let n = extrude_patch(&base, "zmax", ExtrudeDir::Normal(2.0), 4).unwrap();
    assert_eq!(n.points().len(), m.points().len());
    for (p, q) in n.points().iter().zip(m.points()) {
        assert!(vec3::norm(vec3::sub(*p, *q)) <= 1e-14);
    }
    // slanted extrusion
    let s = extrude_patch(&base, "zmax", ExtrudeDir::Vector([1.0, 0.0, 2.0]), 4).unwrap();
    rel(s.total_volume(), 18.0, 1e-12, "slanted volume");
    assert!(check_mesh(&s).is_ok());
    // an inward direction is refused
    assert!(extrude_patch(&base, "zmax", ExtrudeDir::Vector([0.0, 0.0, -1.0]), 2).is_err());
    assert!(extrude_patch(&base, "nope", ExtrudeDir::Normal(1.0), 2).is_err());
}

#[test]
fn e4_orientation_repair() {
    for (name, base) in bases() {
        let hash = base.content_hash();
        let nf = base.n_faces();
        let cases: Vec<(&str, Vec<usize>)> = vec![
            ("every fifth face", (0..nf).filter(|f| f % 5 == 0).collect()),
            ("all faces", (0..nf).collect()),
            ("one boundary face", vec![nf - 1]),
        ];
        for (label, which) in cases {
            let mut topo = base.topology().clone();
            for &f in &which {
                topo.face_points[topo.face_offsets[f]..topo.face_offsets[f + 1]].reverse();
            }
            assert!(
                Mesh::from_topology(topo.clone()).is_err(),
                "{name} / {label}: positive control failed"
            );
            let (m, rep) = repair_mesh(topo).unwrap_or_else(|e| panic!("{name} / {label}: {e}"));
            assert_eq!(rep.flipped_faces, which.len(), "{name} / {label}");
            assert!(check_mesh(&m).is_ok(), "{name} / {label}");
            assert_eq!(
                m.content_hash(),
                hash,
                "{name} / {label}: repaired mesh differs from the original"
            );
        }
        // already correct
        let mut topo = base.topology().clone();
        let rep = repair_face_orientation(&mut topo);
        assert_eq!(rep.flipped_faces, 0);
        assert_eq!(&topo, base.topology());
        // owner > neighbour is an error, not repaired
        let mut topo = base.topology().clone();
        let (o, n) = (topo.owner[0], topo.neighbour[0]);
        topo.owner[0] = n;
        topo.neighbour[0] = o;
        assert!(
            repair_mesh(topo).is_err(),
            "{name}: swapped owner/neighbour must stay an error"
        );
    }
}

#[test]
fn e5_patch_operations() {
    let mut base = box_mesh([4, 3, 2], [4.0, 3.0, 2.0], U).unwrap();
    let ymin = base.patch("ymin").unwrap().faces().next().unwrap();
    let zmax = base.patch("zmax").unwrap().faces().nth(3).unwrap();
    base.add_face_zone("fz", vec![ymin, zmax]).unwrap();
    let merged = merge_patches(&base, &["ymin", "ymax"], "ywalls").unwrap();
    let w = merged.patch("ywalls").unwrap();
    assert_eq!(
        w.size,
        base.patch("ymin").unwrap().size + base.patch("ymax").unwrap().size
    );
    rel(
        patch_area(&merged, "ywalls"),
        patch_area(&base, "ymin") + patch_area(&base, "ymax"),
        1e-12,
        "area",
    );
    assert!(merged.patch("ymin").is_none() && merged.patch("ymax").is_none());
    assert_eq!(merged.cell_volumes(), base.cell_volumes(), "cell volumes");
    assert_eq!(merged.n_internal_faces(), base.n_internal_faces());
    // face zone entries point at faces with the same centres
    let mut before: Vec<Vec3> = base.face_zones()[0]
        .ids
        .iter()
        .map(|&f| base.face_centres()[f])
        .collect();
    let mut after: Vec<Vec3> = merged.face_zones()[0]
        .ids
        .iter()
        .map(|&f| merged.face_centres()[f])
        .collect();
    before.sort_by(|a, b| a.partial_cmp(b).unwrap());
    after.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(before, after);
    // separate it again
    let sep = separate_patch(&merged, "ywalls").unwrap();
    assert_eq!(sep.patches().len(), merged.patches().len() + 1);
    let (a0, a1) = (patch_area(&sep, "ywalls_0"), patch_area(&sep, "ywalls_1"));
    rel(a0, patch_area(&base, "ymin"), 1e-12, "component 0");
    rel(a1, patch_area(&base, "ymax"), 1e-12, "component 1");
    let boundary_area = |m: &Mesh| m.patches().iter().map(|p| patch_area(m, &p.name)).sum::<f64>();
    rel(boundary_area(&sep), boundary_area(&base), 1e-12, "round trip total");
    assert_eq!(sep.n_boundary_faces(), base.n_boundary_faces());
    // gapped two-box merge: zmin has two components
    let unit = box_mesh([2, 2, 2], [1.0, 1.0, 1.0], U).unwrap();
    let far = rcfd_mesh::edit::transform_mesh(&unit, &Affine::translation([1.5, 0.0, 0.0])).unwrap();
    let g = merge_meshes(&unit, &far, 1e-9).unwrap();
    let s = separate_patch(&g, "zmin").unwrap();
    rel(patch_area(&s, "zmin_0"), 1.0, 1e-12, "zmin_0");
    rel(patch_area(&s, "zmin_1"), 1.0, 1e-12, "zmin_1");
    // a connected patch is returned unchanged
    let same = separate_patch(&base, "zmin").unwrap();
    assert_eq!(same.content_hash(), base.content_hash());
    // periodic patches are refused
    let p = box_2d(4, 4, 1.0, 1.0, true, false).unwrap();
    let cyc: Vec<&str> = p
        .patches()
        .iter()
        .filter(|q| q.periodic_with.is_some())
        .map(|q| q.name.as_str())
        .collect();
    assert_eq!(cyc.len(), 2);
    assert!(merge_patches(&p, &cyc, "both").is_err());
    assert!(separate_patch(&p, cyc[0]).is_err());
}
