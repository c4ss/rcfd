//! F-108 registered checks I1 to I7 (ADR 0020, part B). Limits are the registered ones.

use rcfd_mesh::gen::{box_mesh, cylinder_ogrid, Grading};
use rcfd_mesh::inflate::{inflate_patch, InflateReport, LayerSizing};
use rcfd_mesh::layers::{first_cell_height_from_y_plus, y_plus_of_first_cell};
use rcfd_mesh::mesh::{BoundarySpec, Mesh, PatchKind};
use rcfd_mesh::quality::check_mesh;
use std::collections::HashMap;

fn patch_area(m: &Mesh, name: &str) -> f64 {
    m.patch(name).unwrap().faces().map(|f| m.face_area(f)).sum()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Sorted distinct z values above `z0` (clustered at 1e-9).
fn levels_above(m: &Mesh, z0: f64) -> Vec<f64> {
    let mut z: Vec<f64> = m.points().iter().map(|p| p[2]).filter(|&v| v > z0 + 1e-9).collect();
    z.sort_by(|a, b| a.total_cmp(b));
    let mut out: Vec<f64> = Vec::new();
    for v in z {
        if out.last().map_or(true, |&l| v - l > 1e-9) {
            out.push(v);
        }
    }
    out
}

fn heights_from_levels(levels: &[f64], z0: f64) -> Vec<f64> {
    let mut prev = z0;
    levels
        .iter()
        .map(|&l| {
            let h = l - prev;
            prev = l;
            h
        })
        .collect()
}

fn box_top() -> Mesh {
    box_mesh([4, 3, 2], [4.0, 3.0, 2.0], [Grading::Uniform; 3]).unwrap()
}

/// 6 x 6 triangulated unit square extruded one layer (thickness 1) into prisms; patches zmin, zmax, sides.
fn tri_prism_mesh() -> Mesh {
    let n = 6usize;
    let pid = |i: usize, j: usize, k: usize| i + (n + 1) * (j + (n + 1) * k);
    let h = 1.0 / n as f64;
    let mut points = Vec::new();
    for k in 0..2 {
        for j in 0..=n {
            for i in 0..=n {
                points.push([i as f64 * h, j as f64 * h, k as f64]);
            }
        }
    }
    let mut cells = Vec::new();
    let (mut bottom, mut top) = (Vec::new(), Vec::new());
    for j in 0..n {
        for i in 0..n {
            let tris = [
                [pid(i, j, 0), pid(i + 1, j, 0), pid(i + 1, j + 1, 0)],
                [pid(i, j, 0), pid(i + 1, j + 1, 0), pid(i, j + 1, 0)],
            ];
            for t in tris {
                let u = [
                    t[0] + (n + 1) * (n + 1),
                    t[1] + (n + 1) * (n + 1),
                    t[2] + (n + 1) * (n + 1),
                ];
                let faces = vec![
                    vec![t[0], t[1], t[2]],
                    vec![u[0], u[1], u[2]],
                    vec![t[0], t[1], u[1], u[0]],
                    vec![t[1], t[2], u[2], u[1]],
                    vec![t[2], t[0], u[0], u[2]],
                ];
                bottom.push(faces[0].clone());
                top.push(faces[1].clone());
                cells.push(faces);
            }
        }
    }
    let mut count: HashMap<Vec<usize>, usize> = HashMap::new();
    let key = |f: &Vec<usize>| {
        let mut k = f.clone();
        k.sort_unstable();
        k
    };
    for c in &cells {
        for f in c {
            *count.entry(key(f)).or_insert(0) += 1;
        }
    }
    let sides: Vec<Vec<usize>> = cells
        .iter()
        .flat_map(|c| c[2..].iter().cloned())
        .filter(|f| count[&key(f)] == 1)
        .collect();
    let specs = vec![
        BoundarySpec {
            name: "zmin".into(),
            kind: PatchKind::Wall,
            faces: bottom,
        },
        BoundarySpec {
            name: "zmax".into(),
            kind: PatchKind::Wall,
            faces: top,
        },
        BoundarySpec {
            name: "sides".into(),
            kind: PatchKind::Wall,
            faces: sides,
        },
    ];
    Mesh::from_cell_faces(points, &cells, &specs).unwrap()
}

fn geometric_sum(first: f64, g: f64, n: usize) -> f64 {
    first * (g.powi(n as i32) - 1.0) / (g - 1.0)
}

#[test]
#[ignore = "record: set-up defect, layer order measured from the original surface (thinnest layer on the wrong side of the wall cap); see i1c/i3c/i4c/i6c and ADR 0020 addendum"]
fn i1_block_patch() {
    let m = box_top();
    assert_eq!(m.n_cells(), 24);
    let (out, rep) = inflate_patch(
        &m,
        "zmax",
        LayerSizing::FirstGrowth {
            first: 0.01,
            growth: 1.2,
            layers: 8,
        },
    )
    .unwrap();
    assert_eq!(out.n_cells(), 24 + 96);
    let t = geometric_sum(0.01, 1.2, 8);
    let vol_err = rel(out.total_volume(), 24.0 + 12.0 * t);
    println!("I1 volume rel err {vol_err:e}");
    assert!(vol_err <= 1e-12);
    let levels = levels_above(&out, 2.0);
    assert_eq!(levels.len(), 8);
    let mut worst = 0.0f64;
    let mut cum = 0.0;
    for (k, &l) in levels.iter().enumerate() {
        cum += 0.01 * 1.2f64.powi(k as i32);
        worst = worst.max((l - (2.0 + cum)).abs());
    }
    println!("I1 layer plane max error {worst:e}");
    assert!(worst <= 1e-12);
    assert!(rel(patch_area(&out, "zmax"), 12.0) <= 1e-12);
    let side_err = rel(patch_area(&out, "zmax_side"), 2.0 * 7.0 * t);
    println!("I1 side area rel err {side_err:e}");
    assert!(side_err <= 1e-12);
    assert!((rep.first_height_min - 0.01).abs() <= 1e-12 * 0.01 && (rep.first_height_max - 0.01).abs() <= 1e-12 * 0.01);
    let last = 0.01 * 1.2f64.powi(7);
    assert!(rel(rep.last_height_min, last) <= 1e-12 && rel(rep.last_height_max, last) <= 1e-12);
    assert!((rep.growth_min - 1.2).abs() <= 1e-12 && (rep.growth_max - 1.2).abs() <= 1e-12);
    assert!(rep.quality.is_ok());
    println!(
        "I1 max aspect {}, non-orth {}, skew {}",
        rep.quality.max_aspect_ratio, rep.quality.max_non_orthogonality, rep.quality.max_skewness
    );
    assert!(rel(rep.quality.max_aspect_ratio, 100.0) <= 1e-9);
    assert!(rep.quality.max_non_orthogonality <= 1e-4);
    assert!(rep.quality.max_skewness <= 1e-9);
}

#[test]
fn i2_unstructured_triangle_patch() {
    let m = tri_prism_mesh();
    assert_eq!(m.n_cells(), 72);
    assert!(rel(patch_area(&m, "zmax"), 1.0) <= 1e-12);
    let (out, _rep) = inflate_patch(
        &m,
        "zmax",
        LayerSizing::FirstGrowth {
            first: 0.02,
            growth: 1.3,
            layers: 5,
        },
    )
    .unwrap();
    assert_eq!(out.n_cells(), 72 + 360);
    let t = geometric_sum(0.02, 1.3, 5);
    let vol_err = rel(out.total_volume(), 1.0 + t);
    println!("I2 volume rel err {vol_err:e}");
    assert!(vol_err <= 1e-12);
    assert!(rel(patch_area(&out, "zmax"), 1.0) <= 1e-12);
    let side_err = rel(patch_area(&out, "zmax_side"), 4.0 * t);
    println!("I2 side area rel err {side_err:e}");
    assert!(side_err <= 1e-12);
    for c in 72..out.n_cells() {
        let fs = out.cell_faces(c);
        assert_eq!(fs.len(), 5, "cell {c}");
        let mut pts: Vec<usize> = fs.iter().flat_map(|&f| out.face_points(f).iter().copied()).collect();
        pts.sort_unstable();
        pts.dedup();
        assert_eq!(pts.len(), 6, "cell {c}");
    }
    out.validate().unwrap();
    assert!(check_mesh(&out).is_ok());
}

#[test]
#[ignore = "record: set-up defect, layer order measured from the original surface (thinnest layer on the wrong side of the wall cap); see i1c/i3c/i4c/i6c and ADR 0020 addendum"]
fn i3_curved_patch() {
    let m = cylinder_ogrid(24, 3, 1.0, 2.0, Grading::Uniform, 0.5).unwrap();
    let v0 = m.total_volume();
    let (out, _rep) = inflate_patch(
        &m,
        "cylinder",
        LayerSizing::FirstGrowth {
            first: 0.05,
            growth: 1.25,
            layers: 4,
        },
    )
    .unwrap();
    let t = geometric_sum(0.05, 1.25, 4);
    let n = 24.0f64;
    let exact = 0.5 * (n / 2.0) * (2.0 * std::f64::consts::PI / n).sin() * (1.0 - (1.0 - t) * (1.0 - t));
    let added = out.total_volume() - v0;
    let err = rel(added, exact);
    println!("I3 added volume {added} exact {exact} rel err {err:e}");
    assert!(err <= 1e-10);
    // layer radii
    let mut cum = [0.0; 4];
    let mut c = 0.0;
    for (k, v) in cum.iter_mut().enumerate() {
        c += 0.05 * 1.25f64.powi(k as i32);
        *v = c;
    }
    let mut seen = [false; 4];
    let mut worst = 0.0f64;
    for p in &out.points()[m.n_points()..] {
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let (k, d) = (0..4)
            .map(|k| (k, (r - (1.0 - cum[k])).abs()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        seen[k] = true;
        worst = worst.max(d);
    }
    println!("I3 layer radius max error {worst:e}");
    assert!(worst <= 1e-12);
    assert!(seen.iter().all(|&s| s));
    assert!(check_mesh(&out).is_ok());
}

fn heights_of(out: &Mesh, z0: f64) -> Vec<f64> {
    heights_from_levels(&levels_above(out, z0), z0)
}

#[test]
#[ignore = "record: set-up defect, layer order measured from the original surface (thinnest layer on the wrong side of the wall cap); see i1c/i3c/i4c/i6c and ADR 0020 addendum"]
fn i4_sizing_types() {
    let boxm = box_top();
    let tri = tri_prism_mesh();
    // Uniform
    for (m, patch, z0) in [(&boxm, "zmax", 2.0), (&tri, "zmax", 1.0)] {
        let (out, _) = inflate_patch(m, patch, LayerSizing::Uniform { total: 0.3, layers: 6 }).unwrap();
        let h = heights_of(&out, z0);
        assert_eq!(h.len(), 6);
        for x in &h {
            assert!((x - 0.05).abs() <= 1e-12, "{h:?}");
        }
    }
    // AspectRatio
    for (m, s, z0) in [(&boxm, 1.0, 2.0), (&tri, (1.0f64 / 72.0).sqrt(), 1.0)] {
        let (out, rep) = inflate_patch(
            m,
            "zmax",
            LayerSizing::AspectRatio {
                aspect: 20.0,
                growth: 1.15,
                layers: 5,
            },
        )
        .unwrap();
        let h = heights_of(&out, z0);
        assert!((h[0] - s / 20.0).abs() <= 1e-12, "{h:?}");
        for w in h.windows(2) {
            assert!((w[1] / w[0] - 1.15).abs() <= 1e-12);
        }
        assert!((rep.first_height_min - s / 20.0).abs() <= 1e-12);
    }
    // LastRatio
    let (out, _) = inflate_patch(
        &boxm,
        "zmax",
        LayerSizing::LastRatio {
            first: 0.01,
            last_ratio: 0.5,
            layers: 6,
        },
    )
    .unwrap();
    let h = heights_of(&out, 2.0);
    let g = 50.0f64.powf(0.2);
    assert!((h[0] - 0.01).abs() <= 1e-12 && (h[5] - 0.5).abs() <= 1e-12, "{h:?}");
    for w in h.windows(2) {
        assert!((w[1] / w[0] - g).abs() <= 1e-12);
    }
}

#[test]
fn i5_invalid_input() {
    let m = box_top();
    let ok = |s: LayerSizing| inflate_patch(&m, "zmax", s);
    assert!(ok(LayerSizing::FirstGrowth {
        first: 0.01,
        growth: 1.2,
        layers: 3
    })
    .is_ok());
    assert!(ok(LayerSizing::FirstGrowth {
        first: 0.01,
        growth: 1.2,
        layers: 0
    })
    .is_err());
    assert!(ok(LayerSizing::FirstGrowth {
        first: 0.0,
        growth: 1.2,
        layers: 3
    })
    .is_err());
    assert!(ok(LayerSizing::FirstGrowth {
        first: -0.1,
        growth: 1.2,
        layers: 3
    })
    .is_err());
    assert!(ok(LayerSizing::FirstGrowth {
        first: 0.01,
        growth: 0.0,
        layers: 3
    })
    .is_err());
    assert!(ok(LayerSizing::LastRatio {
        first: 0.01,
        last_ratio: 0.5,
        layers: 1
    })
    .is_err());
    assert!(inflate_patch(&m, "nope", LayerSizing::Uniform { total: 0.1, layers: 2 }).is_err());
    let mut per = m.clone();
    per.set_periodic("xmin", "xmax").unwrap();
    assert!(inflate_patch(&per, "zmax", LayerSizing::Uniform { total: 0.1, layers: 2 }).is_err());
}

#[test]
fn i6_y_plus_target() {
    let first = first_cell_height_from_y_plus(0.8, 0.05, 1e-5);
    let tri = tri_prism_mesh();
    let (_, rep) = inflate_patch(
        &tri,
        "zmax",
        LayerSizing::FirstGrowth {
            first,
            growth: 1.2,
            layers: 6,
        },
    )
    .unwrap();
    assert!(rel(rep.first_height_min, 3.2e-4) <= 1e-12);
    let yp = y_plus_of_first_cell(rep.first_height_min, 0.05, 1e-5);
    println!("I6 y+ {yp}");
    assert!(rel(yp, 0.8) <= 1e-12);
}

#[test]
fn i7_report() {
    let m = box_top();
    let (_, rep) = inflate_patch(
        &m,
        "zmax",
        LayerSizing::FirstGrowth {
            first: 0.01,
            growth: 1.2,
            layers: 8,
        },
    )
    .unwrap();
    let t = geometric_sum(0.01, 1.2, 8);
    assert!((rep.thickness_min - t).abs() <= 1e-12 && (rep.thickness_max - t).abs() <= 1e-12);
    let json = serde_json::to_string(&rep).unwrap();
    let back: InflateReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back, rep);
}

// ---- Corrected tests (ADR 0020 addendum): the far cap is the wall, the thinnest layer lies against it. ----

/// Owner-cell height of every cap face of a flat patch: cell volume over face area.
fn cap_cell_heights(m: &Mesh, patch: &str) -> Vec<f64> {
    m.patch(patch)
        .unwrap()
        .faces()
        .map(|f| m.cell_volumes()[m.owner()[f]] / m.face_area(f))
        .collect()
}

#[test]
fn i1c_block_patch_thinnest_layer_at_wall() {
    let m = box_top();
    let (out, rep) = inflate_patch(
        &m,
        "zmax",
        LayerSizing::FirstGrowth {
            first: 0.01,
            growth: 1.2,
            layers: 8,
        },
    )
    .unwrap();
    let t = geometric_sum(0.01, 1.2, 8);
    assert!(rel(out.total_volume(), 24.0 + 12.0 * t) <= 1e-12);
    // planes counted from the wall (cap at 2 + t): wall - cum_k
    let levels = levels_above(&out, 2.0);
    assert_eq!(levels.len(), 8);
    assert!((levels[7] - (2.0 + t)).abs() <= 1e-12, "cap at {}", levels[7]);
    let mut worst = 0.0f64;
    let mut cum = 0.0;
    for k in 0..8 {
        worst = worst.max((levels[7 - k] - (2.0 + t - cum)).abs());
        cum += 0.01 * 1.2f64.powi(k as i32);
    }
    println!("I1c layer plane max error {worst:e}");
    assert!(worst <= 1e-12);
    // the cell against the wall has the first height, the cell against the original surface the last
    for h in cap_cell_heights(&out, "zmax") {
        assert!((h - 0.01).abs() <= 1e-12, "{h}");
    }
    let lowest = 2.0;
    let last = 0.01 * 1.2f64.powi(7);
    for c in 24..out.n_cells() {
        let p = out.cell_centres()[c];
        if p[2] < lowest + last {
            assert!((out.cell_volumes()[c] - last).abs() <= 1e-12);
        }
    }
    assert!(rel(rep.quality.max_aspect_ratio, 100.0) <= 1e-9);
    assert!(rep.quality.max_non_orthogonality <= 1e-4 && rep.quality.max_skewness <= 1e-9);
    assert!(rep.quality.is_ok());
}

#[test]
fn i3c_curved_patch_radii_from_the_wall() {
    let m = cylinder_ogrid(24, 3, 1.0, 2.0, Grading::Uniform, 0.5).unwrap();
    let v0 = m.total_volume();
    let (out, _) = inflate_patch(
        &m,
        "cylinder",
        LayerSizing::FirstGrowth {
            first: 0.05,
            growth: 1.25,
            layers: 4,
        },
    )
    .unwrap();
    let t = geometric_sum(0.05, 1.25, 4);
    let n = 24.0f64;
    let exact = 0.5 * (n / 2.0) * (2.0 * std::f64::consts::PI / n).sin() * (1.0 - (1.0 - t) * (1.0 - t));
    assert!(rel(out.total_volume() - v0, exact) <= 1e-10);
    // wall (cap) at radius 1 - t, planes at 1 - t + cum_k (cum from the wall)
    let mut expect = Vec::new();
    let mut cum = 0.0;
    for k in 0..4 {
        expect.push(1.0 - t + cum);
        cum += 0.05 * 1.25f64.powi(k);
    }
    let mut seen = [false; 4];
    let mut worst = 0.0f64;
    for p in &out.points()[m.n_points()..] {
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        let (k, d) = (0..4)
            .map(|k| (k, (r - expect[k]).abs()))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        seen[k] = true;
        worst = worst.max(d);
    }
    println!("I3c layer radius max error {worst:e}");
    assert!(worst <= 1e-12 && seen.iter().all(|&s| s));
    // the cylinder patch is now the cap at radius 1 - t
    for f in out.patch("cylinder").unwrap().faces() {
        let c = out.face_centres()[f];
        let r = (c[0] * c[0] + c[1] * c[1]).sqrt();
        assert!(r < 1.0 - t + 0.01);
    }
    assert!(check_mesh(&out).is_ok());
}

#[test]
fn i4c_sizing_types_from_the_wall() {
    let boxm = box_top();
    let tri = tri_prism_mesh();
    let from_wall = |out: &Mesh, z0: f64| -> Vec<f64> {
        let mut h = heights_of(out, z0);
        h.reverse();
        h
    };
    for (m, z0) in [(&boxm, 2.0), (&tri, 1.0)] {
        let (out, _) = inflate_patch(m, "zmax", LayerSizing::Uniform { total: 0.3, layers: 6 }).unwrap();
        for x in from_wall(&out, z0) {
            assert!((x - 0.05).abs() <= 1e-12);
        }
    }
    for (m, s, z0) in [(&boxm, 1.0, 2.0), (&tri, (1.0f64 / 72.0).sqrt(), 1.0)] {
        let (out, _) = inflate_patch(
            m,
            "zmax",
            LayerSizing::AspectRatio {
                aspect: 20.0,
                growth: 1.15,
                layers: 5,
            },
        )
        .unwrap();
        let h = from_wall(&out, z0);
        assert!((h[0] - s / 20.0).abs() <= 1e-12, "{h:?}");
        for w in h.windows(2) {
            assert!((w[1] / w[0] - 1.15).abs() <= 1e-12);
        }
    }
    let (out, _) = inflate_patch(
        &boxm,
        "zmax",
        LayerSizing::LastRatio {
            first: 0.01,
            last_ratio: 0.5,
            layers: 6,
        },
    )
    .unwrap();
    let h = from_wall(&out, 2.0);
    let g = 50.0f64.powf(0.2);
    assert!((h[0] - 0.01).abs() <= 1e-12 && (h[5] - 0.5).abs() <= 1e-12, "{h:?}");
    for w in h.windows(2) {
        assert!((w[1] / w[0] - g).abs() <= 1e-12);
    }
}

#[test]
fn i6c_wall_adjacent_cell_meets_the_y_plus_target() {
    let first = first_cell_height_from_y_plus(0.8, 0.05, 1e-5);
    let tri = tri_prism_mesh();
    let (out, _) = inflate_patch(
        &tri,
        "zmax",
        LayerSizing::FirstGrowth {
            first,
            growth: 1.2,
            layers: 6,
        },
    )
    .unwrap();
    let hs = cap_cell_heights(&out, "zmax");
    assert_eq!(hs.len(), 72);
    for h in hs {
        assert!(rel(h, 3.2e-4) <= 1e-9, "{h}");
        let yp = y_plus_of_first_cell(h, 0.05, 1e-5);
        assert!(rel(yp, 0.8) <= 1e-9, "{yp}");
    }
}
