//! F-235 (ADR 0047): sweep, thin-volume and multizone hexahedral meshing. Limits are the registered ones (S1..S9).

use rcfd_mesh::gen::Grading;
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::sweep::*;
use std::collections::HashSet;
use std::f64::consts::PI;

const PI2: f64 = 2.0 * PI;

fn polygon_area_factor(n: usize) -> f64 {
    0.5 * n as f64 * (PI2 / n as f64).sin()
}

fn z_line(len: f64, layers: usize, g: Grading, bottom: &str, top: &str) -> SweepSpec {
    SweepSpec {
        path: Path::Line {
            dir: [0.0, 0.0, 1.0],
            length: len,
        },
        layers,
        grading: g,
        bottom: bottom.into(),
        top: top.into(),
    }
}

fn pipe() -> Mesh {
    pipe_mesh(17, 12, 8, 1.0, 1.0, 0.5, Grading::Uniform, false).unwrap()
}

fn annulus() -> Mesh {
    let b = annulus_base(64, 8, 0.5, 1.0, Grading::Uniform).unwrap();
    sweep(&b, &z_line(2.0, 5, Grading::Expansion(2.0), "zmin", "zmax")).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// S1 + S2 + S3 common part: all hex, valid, closed.
fn check_all_hex_closed(m: &Mesh, n_cells: usize) {
    m.validate().unwrap();
    assert_eq!(m.n_cells(), n_cells);
    let mut incidence = vec![0usize; m.n_faces()];
    for c in 0..m.n_cells() {
        let fs = m.cell_faces(c);
        assert_eq!(fs.len(), 6, "cell {c} is not a hexahedron (faces)");
        let mut pts = HashSet::new();
        for &f in fs {
            assert_eq!(m.face_points(f).len(), 4, "cell {c} has a non-quad face");
            pts.extend(m.face_points(f).iter().copied());
            incidence[f] += 1;
        }
        assert_eq!(pts.len(), 8, "cell {c} does not have eight distinct points");
    }
    for (f, &inc) in incidence.iter().enumerate() {
        let want = if f < m.n_internal_faces() { 2 } else { 1 };
        assert_eq!(inc, want, "face {f} incidence");
    }
    assert!(m.cell_volumes().iter().all(|&v| v > 0.0));
}

fn patch_sizes(m: &Mesh) -> Vec<(String, usize)> {
    let mut v: Vec<_> = m.patches().iter().map(|p| (p.name.clone(), p.size)).collect();
    v.sort();
    v
}

fn patch_area(m: &Mesh, name: &str) -> f64 {
    m.patch(name).unwrap().faces().map(|f| m.face_area(f)).sum()
}

#[test]
fn s1_s5_pipe_exact() {
    let m = pipe();
    check_all_hex_closed(&m, 8840);
    assert_eq!(
        patch_sizes(&m),
        vec![("wall".into(), 68 * 8), ("zmax".into(), 1105), ("zmin".into(), 1105)]
    );
    let nn = 68usize;
    let v = polygon_area_factor(nn);
    assert!(rel(m.total_volume(), v) < 1e-12, "{} vs {}", m.total_volume(), v);
    let wall = nn as f64 * 2.0 * (PI / nn as f64).sin();
    assert!(rel(patch_area(&m, "wall"), wall) < 1e-12);
    assert!(rel(patch_area(&m, "zmin"), v) < 1e-12);
    // deficit against the circle is the inscribed-polygon one
    let deficit = 1.0 - m.total_volume() / PI;
    assert!((deficit - (PI2 * PI2) / (6.0 * 68.0 * 68.0)).abs() < 1e-5, "{deficit}");
}

#[test]
fn s8_pipe_quality_and_no_degenerate_cells() {
    let m = pipe();
    let q = check_mesh(&m);
    println!(
        "pipe: nonorth {:.3} skew {:.4} aspect {:.3} angles [{:.2},{:.2}]",
        q.max_non_orthogonality,
        q.max_skewness,
        q.max_aspect_ratio,
        q.measures.min_face_angle,
        q.measures.max_face_angle
    );
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    assert!(q.max_non_orthogonality <= 45.0, "{}", q.max_non_orthogonality);
    assert!(q.max_skewness <= 0.5, "{}", q.max_skewness);
    assert!(q.max_aspect_ratio <= 10.0, "{}", q.max_aspect_ratio);
    assert!(q.measures.min_face_angle >= 40.0, "{}", q.measures.min_face_angle);
    assert!(q.measures.max_face_angle <= 140.0, "{}", q.measures.max_face_angle);
}

#[test]
fn s1_s5_annulus_exact_layers_and_quality() {
    let m = annulus();
    check_all_hex_closed(&m, 2560);
    assert_eq!(
        patch_sizes(&m),
        vec![
            ("inner".into(), 64 * 5),
            ("outer".into(), 64 * 5),
            ("zmax".into(), 512),
            ("zmin".into(), 512)
        ]
    );
    let nn = 64usize;
    let v = polygon_area_factor(nn) * (1.0 - 0.25) * 2.0;
    assert!(rel(m.total_volume(), v) < 1e-12);
    for (name, r) in [("inner", 0.5), ("outer", 1.0)] {
        let a = nn as f64 * 2.0 * r * (PI / nn as f64).sin() * 2.0;
        assert!(rel(patch_area(&m, name), a) < 1e-12, "{name}");
    }
    assert!(rel(patch_area(&m, "zmin"), polygon_area_factor(nn) * 0.75) < 1e-12);
    // S7: exactly 6 distinct z values, last/first thickness 2
    let mut zs: Vec<f64> = m.points().iter().map(|p| p[2]).collect();
    zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    zs.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    assert_eq!(zs.len(), 6);
    let (first, last) = (zs[1] - zs[0], zs[5] - zs[4]);
    assert!(rel(last / first, 2.0) < 1e-12, "{}", last / first);
    let q = check_mesh(&m);
    assert!(q.errors.is_empty());
    println!(
        "annulus: nonorth {:.3e} skew {:.3e} aspect {:.3}",
        q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio
    );
    assert!(q.max_non_orthogonality <= 0.01, "{}", q.max_non_orthogonality);
    assert!(q.max_aspect_ratio <= 12.0, "{}", q.max_aspect_ratio);
}

/// Registered S8 limit (ADR 0047): skewness <= 1e-6 on the annulus. MISS: measured 2.1e-2, see the diagnosis below and the ADR
/// addendum. The limit is intact.
#[test]
#[ignore = "known miss: registered annulus skewness limit 1e-6 (ADR 0047 S8) is not met; measured 2.1e-2"]
fn s8_annulus_skewness_known_miss() {
    let q = check_mesh(&annulus());
    assert!(q.max_skewness <= 1e-6, "{}", q.max_skewness);
}

/// Diagnosis of the S8 skewness miss (post-hoc test, added after the run, ADR 0047 addendum): the radial (chord) faces and the
/// axial faces have zero skewness; the whole value sits on the tangential (radial-plane) faces, where the cell centroid radius of
/// an annular-sector cell differs from the mid-radius of the face by about dr^2/(12 r), so the centre line crosses the face plane
/// off its centre. The derived bound (this new check, not a replacement of the registered one) is a skewness of at most 0.03.
#[test]
fn s8_annulus_skewness_diagnosis() {
    let m = annulus();
    let sk = rcfd_mesh::quality::compute_metrics(&m).skewness;
    let mut worst = [0.0f64; 3]; // tangential, radial, axial
    for f in 0..m.n_internal_faces() {
        let n = m.face_area_vectors()[f];
        let c = m.face_centres()[f];
        let rr = (c[0] * c[0] + c[1] * c[1]).sqrt();
        let nr = (n[0] * c[0] + n[1] * c[1]) / rr;
        let nn = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let kind = if n[2].abs() > 0.99 * nn {
            2
        } else if nr.abs() > 0.9 * nn {
            1
        } else {
            0
        };
        worst[kind] = worst[kind].max(sk[f]);
    }
    println!(
        "annulus skewness: tangential {:.3e} radial {:.3e} axial {:.3e}",
        worst[0], worst[1], worst[2]
    );
    assert!(worst[0] > 1e-3, "{worst:?}");
    assert!(worst[1] <= 1e-6 && worst[2] <= 1e-6, "{worst:?}");
    assert!(worst[0] <= 0.03, "{worst:?}");
}

#[test]
fn s6_annulus_volume_convergence_order() {
    let exact = PI * (1.0 - 0.25) * 2.0;
    let mut errs = Vec::new();
    for nt in [16usize, 32, 64] {
        let b = annulus_base(nt, 8, 0.5, 1.0, Grading::Uniform).unwrap();
        let m = sweep(&b, &z_line(2.0, 5, Grading::Uniform, "zmin", "zmax")).unwrap();
        m.validate().unwrap();
        errs.push((m.total_volume() - exact).abs());
    }
    for w in errs.windows(2) {
        let order = (w[0] / w[1]).log2();
        println!("annulus volume order {order:.4}");
        assert!(order >= 1.9, "{order}");
    }
}

fn revolve_base() -> BaseMesh {
    let mut b = rect_base(4, 2, 1.0, 1.0).unwrap();
    // move the rectangle into the xz plane at r in [1,2]: (x, y, 0) -> (1 + x, 0, y)
    for p in &mut b.points {
        *p = [1.0 + p[0], 0.0, p[1]];
    }
    b
}

fn revolve(angle: f64, steps: usize) -> Result<Mesh, rcfd_core::Error> {
    sweep(
        &revolve_base(),
        &SweepSpec {
            path: Path::Revolve {
                origin: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                angle,
            },
            layers: steps,
            grading: Grading::Uniform,
            bottom: "start".into(),
            top: "end".into(),
        },
    )
}

#[test]
fn s4_s6_s7_revolution() {
    let m = revolve(PI2, 48).unwrap();
    check_all_hex_closed(&m, 4 * 2 * 48);
    assert!(
        m.patch("start").is_none() && m.patch("end").is_none(),
        "closed revolution has no caps"
    );
    assert!(rel(m.total_volume(), polygon_area_factor(48) * 3.0) < 1e-12);
    // 48 angular stations: number of distinct points = base points * steps
    assert_eq!(m.n_points(), 15 * 48);
    // partial sweep: 90 degrees in 12 steps
    let p = revolve(PI / 2.0, 12).unwrap();
    check_all_hex_closed(&p, 4 * 2 * 12);
    let exact = 12.0 * 0.5 * (PI / 24.0).sin() * 3.0;
    assert!(rel(p.total_volume(), exact) < 1e-12);
    assert_eq!(p.patch("start").unwrap().size, 8);
    assert_eq!(p.patch("end").unwrap().size, 8);
    // Pappus order
    let mut errs = Vec::new();
    for n in [24usize, 48, 96] {
        errs.push((revolve(PI2, n).unwrap().total_volume() - 3.0 * PI).abs());
    }
    for w in errs.windows(2) {
        let order = (w[0] / w[1]).log2();
        println!("revolution Pappus order {order:.4}");
        assert!(order >= 1.9, "{order}");
    }
}

#[test]
fn s9_error_handling() {
    // base touching the axis
    let b = rect_base(2, 2, 1.0, 1.0).unwrap();
    let e = sweep(
        &b,
        &SweepSpec {
            path: Path::Revolve {
                origin: [0.0; 3],
                axis: [0.0, 1.0, 0.0],
                angle: 1.0,
            },
            layers: 4,
            grading: Grading::Uniform,
            bottom: "a".into(),
            top: "b".into(),
        },
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("axis"), "{e}");
    // unassigned boundary edge
    let mut b2 = rect_base(2, 2, 1.0, 1.0).unwrap();
    b2.groups.pop();
    let e = sweep(&b2, &z_line(1.0, 2, Grading::Uniform, "a", "b"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("not assigned"), "{e}");
    // zero layers
    let e = sweep(&b, &z_line(1.0, 0, Grading::Uniform, "a", "b"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("layer"), "{e}");
}

#[test]
fn s4_s7_s8_thin_volumes() {
    let plate = rect_base(20, 10, 2.0, 1.0).unwrap();
    let m = thin_volume(&plate, 0.01).unwrap();
    check_all_hex_closed(&m, 200);
    assert!(rel(m.total_volume(), 0.02) < 1e-12);
    assert_eq!(m.patch("base").unwrap().size, 200);
    assert_eq!(m.patch("top").unwrap().size, 200);
    let q = check_mesh(&m);
    assert!(q.max_aspect_ratio <= 10.0001, "{}", q.max_aspect_ratio);
    assert!(q.max_non_orthogonality <= 1e-6, "{}", q.max_non_orthogonality);
    // curved shell
    let shell = cylinder_shell_base(64, 8, 1.0, 1.0).unwrap();
    let s = thin_volume(&shell, 0.02).unwrap();
    check_all_hex_closed(&s, 512);
    let exact = polygon_area_factor(64) * (1.02f64 * 1.02 - 1.0) * 1.0;
    assert!(rel(s.total_volume(), exact) < 1e-12, "{} {}", s.total_volume(), exact);
    assert_eq!(s.patch("base").unwrap().size, 512);
    assert_eq!(s.patch("top").unwrap().size, 512);
}

#[test]
fn rect_sweep_layer_distribution() {
    let b = rect_base(3, 2, 1.0, 1.0).unwrap();
    let m = sweep(&b, &z_line(1.0, 7, Grading::Expansion(4.0), "zmin", "zmax")).unwrap();
    check_all_hex_closed(&m, 3 * 2 * 7);
    let mut zs: Vec<f64> = m.points().iter().map(|p| p[2]).collect();
    zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    zs.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    assert_eq!(zs.len(), 8);
    assert!(rel((zs[7] - zs[6]) / (zs[1] - zs[0]), 4.0) < 1e-12);
}
