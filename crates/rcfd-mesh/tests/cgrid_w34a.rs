//! Wave 34A (ADR 0072, part 2): diagnostics G1-G5 of the C-grid passage mesh quality (measurements; the hypotheses N1-N3 and K1
//! and their falsifiers are in the ADR).
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_mesh::{quality_summary, FirstCell, QualitySummary};
use rcfd_mesh::quality::compute_metrics;

fn st(r: f64, chord: f64, stagger_deg: f64, camber: f64, t: f64, sweep: f64, lean: f64) -> Station {
    Station {
        radius: r,
        chord,
        stagger: stagger_deg.to_radians(),
        camber,
        camber_pos: 0.4,
        thickness: t,
        sweep,
        lean,
    }
}

/// Test blade B of ADR 0065 / 0068.
fn blade_b() -> AxialFan {
    AxialFan {
        n_blades: 5,
        r_hub: 0.10,
        r_tip: 0.30,
        tip_clearance: 0.01,
        axial_extent: 0.7,
        z_le_root: 0.3,
        family: SectionFamily::Naca4,
        stations: vec![
            st(0.10, 0.10, 35.0, 0.04, 0.10, 0.0, 0.0),
            st(0.30, 0.07, 20.0, 0.04, 0.06, 0.01, 0.005),
        ],
    }
}

/// G4: blade B with both stations equal to its mid-span station (no twist, lean, sweep or chord change).
fn blade_untwisted() -> AxialFan {
    let b = blade_b();
    let mid = b.station_at(0.2);
    let mut lo = mid;
    lo.radius = 0.10;
    let mut hi = mid;
    hi.radius = 0.30;
    AxialFan {
        stations: vec![lo, hi],
        ..b
    }
}

fn spec_a() -> CGridSpec {
    CGridSpec {
        n_up: 24,
        n_chord: 32,
        n_down: 28,
        n_side: 20,
        n_nose: 8,
        n_core: 2,
        n_span: 30,
        n_gap: 18,
        n_bl: 5,
        growth: 1.2,
        first_cell: FirstCell::YPlus {
            y_plus: 30.0,
            speed: 90.0,
            nu: 1.5e-5,
        },
        front_gap: 0.3,
        include_blade: true,
    }
}

fn spec_b() -> CGridSpec {
    CGridSpec {
        n_up: 40,
        n_chord: 48,
        n_down: 40,
        n_side: 30,
        n_nose: 16,
        n_core: 2,
        n_span: 60,
        n_gap: 30,
        n_bl: 8,
        growth: 1.2,
        first_cell: FirstCell::YPlus {
            y_plus: 1.0,
            speed: 90.0,
            nu: 1.5e-5,
        },
        front_gap: 0.3,
        include_blade: true,
    }
}

fn spec_s() -> CGridSpec {
    CGridSpec {
        n_up: 8,
        n_chord: 16,
        n_down: 8,
        n_side: 10,
        n_nose: 4,
        n_core: 1,
        n_span: 14,
        n_gap: 7,
        n_bl: 2,
        growth: 1.2,
        first_cell: FirstCell::Height(1e-3),
        front_gap: 0.3,
        include_blade: true,
    }
}

fn summary(tag: &str, q: &QualitySummary) {
    println!(
        "{tag}: non-orth max {:.2} p99 {:.2} >60: {}; skew max {:.3}; aspect max {:.1}",
        q.max_non_orthogonality, q.p99_non_orthogonality, q.n_above_60, q.max_skewness, q.max_aspect_ratio
    );
}

/// Region of a cell tag (ADR 0072 G1).
fn region(m: &CGridMesh, tag: (u8, usize, usize, usize)) -> String {
    let (blk, a, b, _) = tag;
    match blk {
        1 => "F".into(),
        2 => "core".into(),
        _ => {
            let (n_i, nd) = (m.n_i(), m.n_down());
            let h_c = n_i / 2 - m.n_nose();
            let r = if a < nd || a >= n_i - nd {
                "wake"
            } else if a < h_c {
                "suction"
            } else if a < n_i - h_c {
                "nose"
            } else {
                "pressure"
            };
            format!("{r}/{}", if b < m.n_bl { "stack" } else { "outer" })
        }
    }
}

/// Face family: k-face (radial neighbours), i/j (C block), a/b (F block), interface, or the boundary patch.
fn family(m: &CGridMesh, f: usize) -> String {
    let mesh = &m.mesh;
    let o = m.cell_tags[mesh.owner()[f]];
    if f >= mesh.n_internal_faces() {
        let p = mesh.patch_of_face(f).map(|p| p.name.clone()).unwrap_or_default();
        return format!("bnd:{p}");
    }
    let n = m.cell_tags[mesh.neighbour()[f]];
    if o.3 != n.3 {
        return "k".into();
    }
    if o.0 != n.0 {
        return format!("iface{}-{}", o.0, n.0);
    }
    match o.0 {
        0 => {
            if o.1 != n.1 && o.2 == n.2 && o.2 == 0 && o.1 + n.1 == m.n_i() - 1 {
                "j(cut)".into()
            } else if o.1 != n.1 {
                "i".into()
            } else {
                "j".into()
            }
        }
        1 => {
            if o.1 != n.1 {
                "a".into()
            } else {
                "b".into()
            }
        }
        _ => "core".into(),
    }
}

fn attribution(tag: &str, m: &CGridMesh) {
    let mesh = &m.mesh;
    let met = compute_metrics(mesh);
    let q = quality_summary(mesh);
    summary(&format!("G1 {tag}"), &q);
    let fc = mesh.face_centres();
    let loc = |f: usize| {
        let p = fc[f];
        let r = p[0].hypot(p[1]);
        format!("(z {:.4}, s {:.4}, r {:.4})", p[2], r * p[1].atan2(p[0]), r)
    };
    for (name, vals) in [("non-orth", &met.non_orthogonality), ("skewness", &met.skewness)] {
        let mut idx: Vec<usize> = (0..vals.len()).collect();
        idx.sort_by(|a, b| vals[*b].partial_cmp(&vals[*a]).unwrap());
        println!("G1 {tag} top 20 by {name}:");
        for &f in idx.iter().take(20) {
            let o = m.cell_tags[mesh.owner()[f]];
            let nb = if f < mesh.n_internal_faces() {
                format!("{:?}", m.cell_tags[mesh.neighbour()[f]])
            } else {
                "-".into()
            };
            println!(
                "  face {f}: {:.3} [{}] owner {:?} {} neighbour {nb} {}",
                vals[f],
                family(m, f),
                o,
                region(m, o),
                loc(f)
            );
        }
    }
    let mut counts: std::collections::BTreeMap<(String, String), usize> = std::collections::BTreeMap::new();
    let mut total = 0;
    for f in 0..mesh.n_faces() {
        if met.non_orthogonality[f] > 70.0 {
            total += 1;
            let o = m.cell_tags[mesh.owner()[f]];
            *counts.entry((region(m, o), family(m, f))).or_insert(0) += 1;
        }
    }
    println!("G1 {tag}: {total} faces above 70 deg by (owner region, family):");
    for ((r, fam), n) in &counts {
        println!("  {r:16} {fam:14} {n}");
    }
    let mut by_region: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for ((r, _), n) in &counts {
        *by_region.entry(r.split('/').next().unwrap().to_string()).or_insert(0) += n;
    }
    println!("G1 {tag}: above 70 deg by region {by_region:?}");
    let mut by_layer: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for f in 0..mesh.n_faces() {
        if met.skewness[f] > 2.0 {
            *by_layer.entry(m.cell_tags[mesh.owner()[f]].3).or_insert(0) += 1;
        }
    }
    println!(
        "G1 {tag}: faces with skewness above 2 by owner layer {by_layer:?} (n_k {}, n_span {})",
        m.n_k, m.n_span
    );
    let mut sk: std::collections::BTreeMap<(String, String), usize> = std::collections::BTreeMap::new();
    for f in 0..mesh.n_faces() {
        if met.skewness[f] > 2.0 {
            let o = m.cell_tags[mesh.owner()[f]];
            *sk.entry((region(m, o), family(m, f))).or_insert(0) += 1;
        }
    }
    println!("G1 {tag}: faces with skewness above 2 by (owner region, family): {sk:?}");
}

/// G3: maximum in-plane (2-D) non-orthogonality of the C and F blocks over all node layers, with its location.
fn in_plane_max(m: &CGridMesh) -> (f64, String) {
    let (n_i, ns, nf, nu) = (m.n_i(), m.n_side(), 2 * m.n_nose(), m.n_up());
    let h_c = n_i / 2 - m.n_nose();
    let mut worst = (0.0, String::new());
    for k in 0..=m.n_k {
        let r = m.radii[k];
        let pl = |id: usize| -> (f64, f64) {
            let p = m.grid[id];
            (p[2], r * p[1].atan2(p[0]))
        };
        let quad_c = |q: [usize; 4]| -> (f64, f64) {
            let pts: Vec<(f64, f64)> = q.iter().map(|&id| pl(id)).collect();
            let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
            for t in 0..4 {
                let (p0, p1) = (pts[t], pts[(t + 1) % 4]);
                let cr = p0.0 * p1.1 - p1.0 * p0.1;
                a += cr;
                cx += (p0.0 + p1.0) * cr;
                cy += (p0.1 + p1.1) * cr;
            }
            (cx / (3.0 * a), cy / (3.0 * a))
        };
        let cq = |i: usize, j: usize| {
            quad_c([
                m.c_node(i, j, k),
                m.c_node(i + 1, j, k),
                m.c_node(i + 1, j + 1, k),
                m.c_node(i, j + 1, k),
            ])
        };
        let fq = |a: usize, b: usize| {
            quad_c([
                m.f_node(a, b, k),
                m.f_node(a + 1, b, k),
                m.f_node(a + 1, b + 1, k),
                m.f_node(a, b + 1, k),
            ])
        };
        let mut test = |c1: (f64, f64), c2: Option<(f64, f64)>, e0: usize, e1: usize, what: String| {
            let (p, q) = (pl(e0), pl(e1));
            let mid = (0.5 * (p.0 + q.0), 0.5 * (p.1 + q.1));
            let d = match c2 {
                Some(c) => (c.0 - c1.0, c.1 - c1.1),
                None => (mid.0 - c1.0, mid.1 - c1.1),
            };
            let (tx, ty) = (q.0 - p.0, q.1 - p.1);
            let mut n = (ty, -tx);
            if n.0 * (mid.0 - c1.0) + n.1 * (mid.1 - c1.1) < 0.0 {
                n = (-n.0, -n.1);
            }
            let ang = (d.0 * n.1 - d.1 * n.0).abs().atan2(d.0 * n.0 + d.1 * n.1).to_degrees();
            if ang > worst.0 {
                worst = (ang, format!("{what} layer {k}"));
            }
        };
        // C block
        for j in 0..ns {
            for i in 0..=n_i {
                let (e0, e1) = (m.c_node(i, j, k), m.c_node(i, j + 1, k));
                if i == 0 {
                    test(cq(0, j), None, e0, e1, format!("C outlet i=0 j={j}"));
                } else if i == n_i {
                    test(cq(n_i - 1, j), None, e0, e1, format!("C outlet i={n_i} j={j}"));
                } else {
                    test(cq(i - 1, j), Some(cq(i, j)), e0, e1, format!("C i-edge i={i} j={j}"));
                }
            }
        }
        for i in 0..n_i {
            for j in 0..=ns {
                let (e0, e1) = (m.c_node(i, j, k), m.c_node(i + 1, j, k));
                if j == 0 {
                    let nd = m.n_down();
                    if i < nd {
                        test(cq(i, 0), Some(cq(n_i - 1 - i, 0)), e0, e1, format!("C wake cut i={i}"));
                    } else if i < n_i - nd {
                        test(cq(i, 0), None, e0, e1, format!("C wall i={i}"));
                    }
                } else if j == ns {
                    if i >= h_c && i < n_i - h_c {
                        test(
                            cq(i, ns - 1),
                            Some(fq(nu - 1, i - h_c)),
                            e0,
                            e1,
                            format!("C/F front i={i}"),
                        );
                    } else {
                        test(cq(i, ns - 1), None, e0, e1, format!("C seam i={i}"));
                    }
                } else {
                    test(cq(i, j - 1), Some(cq(i, j)), e0, e1, format!("C j-edge i={i} j={j}"));
                }
            }
        }
        // F block
        for b in 0..nf {
            for a in 0..nu {
                let (e0, e1) = (m.f_node(a, b, k), m.f_node(a, b + 1, k));
                if a == 0 {
                    test(fq(0, b), None, e0, e1, format!("F inlet b={b}"));
                } else {
                    test(fq(a - 1, b), Some(fq(a, b)), e0, e1, format!("F a-edge a={a} b={b}"));
                }
            }
        }
        for a in 0..nu {
            for b in 0..=nf {
                let (e0, e1) = (m.f_node(a, b, k), m.f_node(a + 1, b, k));
                if b == 0 || b == nf {
                    test(
                        fq(a, if b == 0 { 0 } else { nf - 1 }),
                        None,
                        e0,
                        e1,
                        format!("F seam a={a} b={b}"),
                    );
                } else {
                    test(fq(a, b - 1), Some(fq(a, b)), e0, e1, format!("F b-edge a={a} b={b}"));
                }
            }
        }
    }
    worst
}

fn build(fan: &AxialFan, spec: &CGridSpec) -> Result<CGridMesh, String> {
    build_cgrid_mesh(fan, spec).map_err(|e| format!("{e:?}"))
}

/// G1-G3 on S, A', B'; G4 on S and A' counts; G5 on S. Measurements only (ADR 0072).
#[test]
#[ignore = "diagnostic of ADR 0072 (G1-G5), run by hand: cargo test --release -p rcfd-mesh --test cgrid_w34a -- --ignored --nocapture"]
fn g_diagnostics_of_the_33a_construction() {
    for (tag, spec) in [("S", spec_s()), ("A'", spec_a()), ("B'", spec_b())] {
        let m = build(&blade_b(), &spec).unwrap();
        attribution(tag, &m);
        let (g3, at) = in_plane_max(&m);
        println!("G3 {tag}: in-plane max non-orthogonality {g3:.2} deg at {at}");
    }
    // G2a / G2b (Amendment 1): the first cell height as the single factor
    let (ya, yb) = (spec_a().first_cell, spec_b().first_cell);
    for (tag, spec) in [
        (
            "G2a B' counts, A' first cell",
            CGridSpec {
                first_cell: ya,
                ..spec_b()
            },
        ),
        (
            "G2b A' counts, B' first cell",
            CGridSpec {
                first_cell: yb,
                ..spec_a()
            },
        ),
    ] {
        match build(&blade_b(), &spec) {
            Ok(m) => {
                summary(tag, &m.quality());
                attribution(tag, &m);
            }
            Err(e) => println!("{tag}: build failed: {e}"),
        }
    }
    for (tag, spec) in [("S", spec_s()), ("A'", spec_a())] {
        match build(&blade_untwisted(), &spec) {
            Ok(m) => {
                summary(&format!("G4 {tag} untwisted blade"), &m.quality());
                let (g3, at) = in_plane_max(&m);
                println!("G4 {tag}: in-plane max {g3:.2} deg at {at}");
            }
            Err(e) => println!("G4 {tag} untwisted blade: build failed: {e}"),
        }
    }
    for gap in [0.2, 0.4] {
        match build(
            &blade_b(),
            &CGridSpec {
                front_gap: gap,
                ..spec_s()
            },
        ) {
            Ok(m) => {
                summary(&format!("G5 S front_gap {gap}"), &m.quality());
                let (g3, at) = in_plane_max(&m);
                println!("G5 S front_gap {gap}: in-plane max {g3:.2} deg at {at}");
            }
            Err(e) => println!("G5 S front_gap {gap}: build failed: {e}"),
        }
    }
}

/// (max non-orthogonality of k-faces and hub/shroud boundary faces, max k-face skewness, max skewness of wake-stack j-faces and
/// cut faces, overall summary).
fn w_metrics(m: &CGridMesh) -> (f64, f64, f64, QualitySummary) {
    let mesh = &m.mesh;
    let met = compute_metrics(mesh);
    let (mut no_k, mut sk_k, mut sk_w): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for f in 0..mesh.n_faces() {
        let fam = family(m, f);
        let reg = region(m, m.cell_tags[mesh.owner()[f]]);
        if fam == "k" || fam == "bnd:hub" || fam == "bnd:shroud" {
            no_k = no_k.max(met.non_orthogonality[f]);
        }
        if fam == "k" {
            sk_k = sk_k.max(met.skewness[f]);
        }
        if reg == "wake/stack" && (fam == "j" || fam == "j(cut)") {
            sk_w = sk_w.max(met.skewness[f]);
        }
    }
    (no_k, sk_k, sk_w, quality_summary(mesh))
}

/// W1 / W2 falsifiers of ADR 0072 Amendment 2 on the development set-ups DS and DA: each mechanism as a single change of the 33A
/// construction (and both together, reported).
#[test]
#[ignore = "falsifiers of ADR 0072 Amendment 2 (W1, W2), run by hand: cargo test --release -p rcfd-mesh --test cgrid_w34a w_falsifiers -- --ignored --nocapture"]
fn w_falsifiers() {
    let ds = CGridSpec {
        first_cell: FirstCell::Height(7.31e-6),
        n_bl: 3,
        ..spec_s()
    };
    let da = CGridSpec {
        first_cell: spec_b().first_cell,
        ..spec_a()
    };
    for (tag, spec) in [("DS", ds), ("DA", da)] {
        for (label, opts) in [
            ("33A", CGridOptions::default()),
            (
                "W2 only",
                CGridOptions {
                    uniform_wake_stack: true,
                    ..Default::default()
                },
            ),
            (
                "W1 only",
                CGridOptions {
                    radial_remap: true,
                    ..Default::default()
                },
            ),
            (
                "W1 + W2",
                CGridOptions {
                    uniform_wake_stack: true,
                    radial_remap: true,
                    ..Default::default()
                },
            ),
        ] {
            match build_cgrid_mesh_with(&blade_b(), &spec, &opts) {
                Ok(m) => {
                    let (no_k, sk_k, sk_w, q) = w_metrics(&m);
                    println!(
                        "{tag} {label}: k-face/end-wall non-orth max {no_k:.2} deg; k-face skew max {sk_k:.3}; wake-stack skew max {sk_w:.3}; overall non-orth {:.2} skew {:.3} aspect {:.1}",
                        q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio
                    );
                }
                Err(e) => println!("{tag} {label}: build failed: {e:?}"),
            }
        }
    }
}

#[test]
fn cell_tags_cover_every_cell() {
    let m = build(&blade_b(), &spec_s()).unwrap();
    assert_eq!(m.cell_tags.len(), m.mesh.n_cells());
}
