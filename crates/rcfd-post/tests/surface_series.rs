//! ADR 0066 checks S1, S2 and the surface-series part of M5.

use rcfd_post::fwh::SourceMode;
use rcfd_post::surface_series::*;
use rcfd_post::surfaces::Surface as PostSurface;

fn spec() -> SeriesSpec {
    let faces = (0..3)
        .map(|k| {
            let a = 0.7 * k as f64;
            FaceSpec {
                area: 1e-3 * (k + 1) as f64,
                x: [0.2 + 0.05 * k as f64, 0.1 * a.sin(), 0.02 * k as f64],
                n: [0.0, 0.6, 0.8],
                p: (0..32).map(|i| 3.0 * (0.3 * i as f64 + k as f64).sin() + 1.5).collect(),
            }
        })
        .collect();
    SeriesSpec {
        rho0: 1.2,
        c0: 340.0,
        t0: 0.25,
        dt: 1e-4,
        mode: SourceMode::Impermeable,
        flow: [0.0, 0.0, 12.0],
        motion: Motion::Rotation {
            axis: [0.0, 0.0, 2.0],
            origin: [0.0, 0.0, 0.0],
            omega: 150.0,
            angle0: 0.3,
            velocity: [0.0, 0.0, 0.0],
        },
        faces,
    }
}

#[test]
fn s1_round_trips_and_rotation_kinematics() {
    let s = spec();
    // JSON round trip, bit exact
    let text = serde_json::to_string(&s.to_json()).unwrap();
    let back = SeriesSpec::from_json(&serde_json::from_str(&text).unwrap(), None).unwrap();
    assert_eq!(back, s, "JSON round trip is bit exact");
    // CSV pressure table
    let mut doc = s.to_json();
    for f in doc["faces"].as_array_mut().unwrap() {
        f.as_object_mut().unwrap().remove("p");
    }
    doc["pressure_csv"] = serde_json::json!("p.csv");
    assert_eq!(pressure_csv_path(&doc), Some("p.csv"));
    let csv = s.pressure_csv();
    let back = SeriesSpec::from_json(&doc, Some(&csv)).unwrap();
    assert_eq!(back, s, "CSV round trip is bit exact");
    // kinematics of the built surface
    let case = s.build().unwrap();
    case.surface.validate().unwrap();
    let mut worst_r = 0.0_f64;
    let mut worst_v = 0.0_f64;
    let mut worst_n = 0.0_f64;
    for (f, e) in s.faces.iter().zip(&case.surface.elements) {
        let r0 = (f.x[0] * f.x[0] + f.x[1] * f.x[1]).sqrt();
        for i in 0..e.p.len() {
            let r = (e.x[i][0] * e.x[i][0] + e.x[i][1] * e.x[i][1]).sqrt();
            worst_r = worst_r.max((r / r0 - 1.0).abs());
            // v = omega z_hat x r
            let (vx, vy) = (-150.0 * e.x[i][1], 150.0 * e.x[i][0]);
            let vs = (vx * vx + vy * vy).sqrt();
            worst_v = worst_v.max(((e.v[i][0] - vx).powi(2) + (e.v[i][1] - vy).powi(2)).sqrt() / vs);
            let nn = (e.n[i][0].powi(2) + e.n[i][1].powi(2) + e.n[i][2].powi(2)).sqrt();
            worst_n = worst_n.max((nn - 1.0).abs());
            assert!(e.v[i][2].abs() < 1e-12 && (e.x[i][2] - f.x[2]).abs() < 1e-12);
        }
    }
    println!("S1 rotation: radius {worst_r:.3e}, velocity {worst_v:.3e}, normal {worst_n:.3e}");
    assert!(worst_r <= 1e-12 && worst_v <= 1e-12 && worst_n <= 1e-12);
    // angle advance: after one sample the face has turned omega dt
    let e = &case.surface.elements[0];
    let a0 = e.x[0][1].atan2(e.x[0][0]);
    let a1 = e.x[1][1].atan2(e.x[1][0]);
    assert!((a1 - a0 - 150.0 * 1e-4).abs() < 1e-12);
    // medium frame: the stream is removed
    let m = case.medium_frame().unwrap();
    assert!((m.elements[0].v[0][2] + 12.0).abs() < 1e-12);
}

#[test]
fn s2_adapter_from_post_surface() {
    // two triangles of a unit square tilted about x
    let c = (0.3_f64).cos();
    let sn = (0.3_f64).sin();
    let surf = PostSurface {
        points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, c, sn], [0.0, c, sn]],
        tris: vec![[0, 1, 2], [0, 2, 3]],
        tri_cell: vec![None, None],
        vertex_cell: vec![None; 4],
    };
    let g = faces_from_post_surface(&surf, false).unwrap();
    let total: f64 = g.iter().map(|f| f.0).sum();
    assert!((total / surf.area() - 1.0).abs() <= 1e-12);
    let cen = surf.centroids();
    let av = surf.area_vectors();
    for (i, f) in g.iter().enumerate() {
        assert_eq!(f.1, cen[i]);
        let a = (av[i][0].powi(2) + av[i][1].powi(2) + av[i][2].powi(2)).sqrt();
        for k in 0..3 {
            assert!((f.2[k] - av[i][k] / a).abs() <= 1e-12);
        }
        assert!(((f.2[0].powi(2) + f.2[1].powi(2) + f.2[2].powi(2)) - 1.0).abs() <= 1e-12);
    }
    let gf = faces_from_post_surface(&surf, true).unwrap();
    assert!((gf[0].2[1] + g[0].2[1]).abs() <= 1e-15);
    let specs = face_specs(&g, &[vec![1.0; 8], vec![2.0; 8]]).unwrap();
    assert_eq!(specs.len(), 2);
    assert!(face_specs(&g, &[vec![1.0; 8]]).unwrap_err().to_string().contains("p:"));
    // degenerate facet is located
    let bad = PostSurface {
        points: vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
        tris: vec![[0, 1, 2]],
        tri_cell: vec![None],
        vertex_cell: vec![None; 3],
    };
    assert!(faces_from_post_surface(&bad, false)
        .unwrap_err()
        .to_string()
        .contains("tris[0]"));
}

#[test]
fn m5_series_located_errors() {
    let s = spec();
    let mut doc = s.to_json();
    doc.as_object_mut().unwrap().remove("faces");
    let e = SeriesSpec::from_json(&doc, None).unwrap_err().to_string();
    assert!(e.contains("$.faces"), "{e}");
    let mut doc = s.to_json();
    doc["faces"][2]["p"].as_array_mut().unwrap().pop();
    let e = SeriesSpec::from_json(&doc, None).unwrap_err().to_string();
    assert!(e.contains("faces[2].p"), "{e}");
    let mut doc = s.to_json();
    doc["faces"][1]["n"] = serde_json::json!([0.0, 0.0, 2.0]);
    let e = SeriesSpec::from_json(&doc, None).unwrap_err().to_string();
    assert!(e.contains("faces[1].n"), "{e}");
    let mut doc = s.to_json();
    doc["faces"][0]["p"][4] = serde_json::json!("x");
    let e = SeriesSpec::from_json(&doc, None).unwrap_err().to_string();
    assert!(e.contains("faces[0].p[4]"), "{e}");
    let mut doc = s.to_json();
    doc["format"] = serde_json::json!("nope");
    assert!(SeriesSpec::from_json(&doc, None)
        .unwrap_err()
        .to_string()
        .contains("$.format"));
    // CSV: non-uniform time column
    let csv = "t,f0\n0,1\n1,2\n2,3\n3.5,4\n4.5,5\n";
    let e = parse_pressure_csv(csv).unwrap_err().to_string();
    assert!(e.contains("row 5"), "{e}");
    let e = parse_pressure_csv("t,f0\n0,1\n1,x\n").unwrap_err().to_string();
    assert!(e.contains("row 3") && e.contains("column 2"), "{e}");
    // non-subsonic rotation: validation of the built surface names "subsonic"
    let mut fast = s.clone();
    if let Motion::Rotation { omega, .. } = &mut fast.motion {
        *omega = 5000.0;
    }
    let e = fast.build().unwrap().surface.validate().unwrap_err().to_string();
    assert!(e.contains("subsonic"), "{e}");
}
