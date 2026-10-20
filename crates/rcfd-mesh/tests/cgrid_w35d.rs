//! Wave 35D (ADR 0079): the C-grid mesh of the FAN-01 passage (part 1), the B' quality mechanisms one at a time (part 2), the control
//! range (part 3) and the bit-identity of the existing constructions (part 4). Limits registered in ADR 0079 before this code ran; a miss is
//! an ignored `*_known_miss` test with the limit unchanged. Helpers follow `cgrid_w34a.rs` / `cgrid_e_w34a.rs` (wave 34A).

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the limits

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_mesh::{quality_summary, FirstCell, QualitySummary};
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::compute_metrics;
use std::f64::consts::PI;

// ---------------------------------------------------------------- set-ups

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

/// Set-up S of ADR 0068.
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

/// Set-up A' of ADR 0072 (y+ = 30).
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

/// Set-up B' of ADR 0072 (y+ = 1).
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

/// Construction E of ADR 0072 Amendment 4.
fn construction_e() -> CGridOptions {
    CGridOptions {
        radial_remap: true,
        smoothing: true,
        ..CGridOptions::default()
    }
}

/// FAN-01 (ADR 0075 amendment 1): the 16 registered sections of `w34d_fan01_cad_sections.csv` (the row r = 247.0 mm is not one of them).
fn fan01() -> AxialFan {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("r_mm"))
        .map(|l| {
            l.split(',')
                .take(8)
                .map(|v| v.trim().parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
        })
        .filter(|r| r[0] != 247.0)
        .collect();
    assert_eq!(rows.len(), 16, "the 16 registered FAN-01 sections");
    let (x0, t0) = (rows[0][6], rows[0][7]);
    AxialFan {
        n_blades: 9,
        r_hub: 0.124,
        r_tip: 0.2475,
        tip_clearance: 0.0025,
        axial_extent: 0.4,
        z_le_root: 0.1,
        family: SectionFamily::Naca4,
        stations: rows
            .iter()
            .map(|s| Station {
                radius: s[0] * 1e-3,
                chord: s[1] * 1e-3,
                stagger: s[2].to_radians(),
                camber: s[3],
                camber_pos: s[4],
                thickness: s[5],
                sweep: (s[6] - x0) * 1e-3,
                lean: s[0] * 1e-3 * (s[7] - t0).to_radians(),
            })
            .collect(),
    }
}

/// First cell of the FAN-01 rule (y+ = 30 at the design-point relative speed at r = 0.186 m).
fn fan01_first_cell() -> FirstCell {
    let area = PI * (0.25f64.powi(2) - 0.124f64.powi(2));
    let omega = 1486.0 * 2.0 * PI / 60.0;
    FirstCell::YPlus {
        y_plus: 30.0,
        speed: (1.4032 / area).hypot(omega * 0.186),
        nu: 1.5e-5,
    }
}

/// The registered FAN-01 C-grid counts (ADR 0075 amendment 1).
fn spec_fan01() -> CGridSpec {
    CGridSpec {
        first_cell: fan01_first_cell(),
        ..spec_s()
    }
}

// ---------------------------------------------------------------- hashes (part 4)

fn fnv(h: &mut u64, x: u64) {
    for b in x.to_le_bytes() {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

/// FNV-1a over the node coordinates, face node lists, owner/neighbour and patch names and ranges.
fn mesh_hash(m: &Mesh) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for p in m.points() {
        for v in p {
            fnv(&mut h, v.to_bits());
        }
    }
    for f in 0..m.n_faces() {
        fnv(&mut h, m.face_points(f).len() as u64);
        for &n in m.face_points(f) {
            fnv(&mut h, n as u64);
        }
    }
    for _ in 0..m.n_cells() {
        fnv(&mut h, m.owner().len() as u64);
    }
    for f in 0..m.n_faces() {
        fnv(&mut h, m.owner()[f] as u64);
    }
    for f in 0..m.n_internal_faces() {
        fnv(&mut h, m.neighbour()[f] as u64);
    }
    for p in m.patches() {
        for b in p.name.bytes() {
            fnv(&mut h, b as u64);
        }
        fnv(&mut h, p.start as u64);
        fnv(&mut h, p.size as u64);
    }
    h
}

/// B1 (ADR 0079 part 4): S and A' with the 33A construction and with construction E reproduce the hashes recorded on 0cef8f5, and the
/// default construction still refuses FAN-01 with the same message.
#[test]
fn b1_existing_constructions_are_bit_identical() {
    let want = [
        ("S", "pursuit", 0xeebcf5467f718bf0u64),
        ("S", "E", 0xde01897c36a13987),
        ("A'", "pursuit", 0x3247673c05cd65bf),
        ("A'", "E", 0x35359c0db0e47e3a),
    ];
    for (tag, ct, h) in want {
        let spec = if tag == "S" { spec_s() } else { spec_a() };
        let opts = if ct == "E" {
            construction_e()
        } else {
            CGridOptions::default()
        };
        let m = build_cgrid_mesh_with(&blade_b(), &spec, &opts).unwrap();
        let got = mesh_hash(&m.mesh);
        println!("B1 {tag} {ct}: {got:#018x}");
        assert_eq!(got, h, "{tag} {ct}");
    }
    for opts in [CGridOptions::default(), construction_e()] {
        let e = build_cgrid_mesh_with(&fan01(), &spec_fan01(), &opts).unwrap_err();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].path, "n_down");
        assert_eq!(
            e[0].message,
            "the wake line (s from -0.0683 to -0.0862) leaves the passage (-0.0774, 0.0092) [layer k=0 (r = 0.12400)]"
        );
    }
}

// ---------------------------------------------------------------- attribution (D1)

fn summary(tag: &str, q: &QualitySummary) {
    println!(
        "{tag}: non-orth max {:.2} mean {:.2} p99 {:.2} >60: {}; skew max {:.3}; aspect max {:.1}",
        q.max_non_orthogonality,
        q.mean_non_orthogonality,
        q.p99_non_orthogonality,
        q.n_above_60,
        q.max_skewness,
        q.max_aspect_ratio
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

/// Face family: k-face, i/j (C block), j(cut), a/b (F block), interface, or the boundary patch.
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

/// The 8 grid nodes of a C or F cell (hex order: bit 0 first index, bit 1 second index, bit 2 layer); None for core cells.
fn cell_nodes(m: &CGridMesh, tag: (u8, usize, usize, usize)) -> Option<[Vec3; 8]> {
    let (blk, a, b, k) = tag;
    if blk == 2 {
        return None;
    }
    let mut p = [[0.0; 3]; 8];
    for (c, pc) in p.iter_mut().enumerate() {
        let (da, db, dk) = (c & 1, (c >> 1) & 1, c >> 2);
        let id = if blk == 0 {
            m.c_node(a + da, b + db, k + dk)
        } else {
            m.f_node(a + da, b + db, k + dk)
        };
        *pc = m.grid[id];
    }
    Some(p)
}

/// Geometric decomposition of a cell (ADR 0079 D1): (largest tilt of the layer-to-layer node lines [deg], thin direction (0 first index,
/// 1 second index, 2 layer), separation of the thin pair, angle between the thin pair's normals [deg], wedge = angle x face size /
/// separation, shear of the thin pair = in-plane offset of its centres / separation).
fn decompose(p: &[Vec3; 8]) -> (f64, usize, f64, f64, f64, f64) {
    let mut tilt: f64 = 0.0;
    for c in 0..4 {
        let (a, b) = (p[c], p[c + 4]);
        let e = vec3::sub(b, a);
        let mid = vec3::scale(vec3::add(a, b), 0.5);
        let rho = vec3::unit([mid[0], mid[1], 0.0]);
        let er = vec3::dot(e, rho);
        let et = vec3::norm(vec3::sub(e, vec3::scale(rho, er)));
        tilt = tilt.max(et.atan2(er.abs()).to_degrees());
    }
    // opposite face pairs: (low corners, high corners) by direction
    let quad = |ids: [usize; 4]| -> (Vec3, Vec3, f64) {
        let q: Vec<Vec3> = ids.iter().map(|&i| p[i]).collect();
        let c = vec3::scale(vec3::add(vec3::add(q[0], q[1]), vec3::add(q[2], q[3])), 0.25);
        let n = vec3::cross(vec3::sub(q[2], q[0]), vec3::sub(q[3], q[1]));
        let size = (0.5 * vec3::norm(n)).sqrt();
        (c, vec3::unit(n), size)
    };
    let pairs = [
        ([0, 2, 6, 4], [1, 3, 7, 5]),
        ([0, 1, 5, 4], [2, 3, 7, 6]),
        ([0, 1, 3, 2], [4, 5, 7, 6]),
    ];
    let mut best = (0usize, f64::INFINITY, 0.0, 0.0, 0.0);
    for (d, (lo, hi)) in pairs.iter().enumerate() {
        let (c0, n0, s0) = quad(*lo);
        let (c1, n1, s1) = quad(*hi);
        let dc = vec3::sub(c1, c0);
        let nm = vec3::unit(vec3::add(n0, n1));
        let sep = vec3::dot(dc, nm).abs();
        if sep < best.1 {
            let ang = vec3::norm(vec3::cross(n0, n1)).atan2(vec3::dot(n0, n1)).to_degrees();
            let shear = vec3::norm(vec3::sub(dc, vec3::scale(nm, vec3::dot(dc, nm)))) / sep;
            best = (d, sep, ang, ang.to_radians() * 0.5 * (s0 + s1) / sep, shear);
        }
    }
    (tilt, best.0, best.1, best.2, best.3, best.4)
}

/// Relative variation `(max - min) / mean` of the trilinear Jacobian over the 8 corners of a hex cell.
fn jacobian_variation(p: &[Vec3; 8]) -> f64 {
    let mut js = Vec::with_capacity(8);
    for c in 0..8usize {
        let (a, b, k) = (c & 1, (c >> 1) & 1, c >> 2);
        let e = |bit: usize| -> Vec3 {
            let (lo, hi) = (c & !(1 << bit), c | (1 << bit));
            vec3::sub(p[hi], p[lo])
        };
        let _ = (a, b, k);
        js.push(vec3::dot(e(0), vec3::cross(e(1), e(2))));
    }
    let mean = js.iter().sum::<f64>() / 8.0;
    let (lo, hi) = js
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |m, &j| (m.0.min(j), m.1.max(j)));
    (hi - lo) / mean.abs()
}

/// Warp of a face: largest distance of a node from the plane through the face centre with the face normal, over the face size.
fn face_warp(mesh: &Mesh, f: usize) -> (f64, f64) {
    let c = mesh.face_centres()[f];
    let s = mesh.face_area_vectors()[f];
    let n = vec3::unit(s);
    let w = mesh
        .face_points(f)
        .iter()
        .map(|&q| vec3::dot(vec3::sub(mesh.points()[q], c), n).abs())
        .fold(0.0, f64::max);
    (w, w / vec3::norm(s).sqrt())
}

fn attribution(tag: &str, m: &CGridMesh) {
    let mesh = &m.mesh;
    let met = compute_metrics(mesh);
    summary(&format!("D1 {tag}"), &quality_summary(mesh));
    let fc = mesh.face_centres();
    let loc = |p: Vec3| {
        let r = p[0].hypot(p[1]);
        format!("(z {:.4}, s {:.4}, r {:.5})", p[2], r * p[1].atan2(p[0]), r)
    };
    let describe = |f: usize| -> String {
        let o = m.cell_tags[mesh.owner()[f]];
        let (w, wr) = face_warp(mesh, f);
        let dec = cell_nodes(m, o)
            .map(|p| {
                let (tilt, d, sep, ang, wedge, shear) = decompose(&p);
                format!(
                    "tilt {tilt:.1} deg, thin dir {d} sep {sep:.2e}, pair angle {ang:.3} deg, wedge {wedge:.3}, shear {shear:.3}, warp/thin {:.3}, jac var {:.4}",
                    w / sep,
                    jacobian_variation(&p)
                )
            })
            .unwrap_or_else(|| "core".into());
        let nb = if f < mesh.n_internal_faces() {
            format!("{:?}", m.cell_tags[mesh.neighbour()[f]])
        } else {
            "-".into()
        };
        format!(
            "[{}] owner {:?} {} nb {nb} {}; warp {w:.2e} (/size {wr:.2e}); {dec}",
            family(m, f),
            o,
            region(m, o),
            loc(fc[f])
        )
    };
    for (name, vals) in [("non-orth", &met.non_orthogonality), ("skewness", &met.skewness)] {
        let mut idx: Vec<usize> = (0..vals.len()).collect();
        idx.sort_by(|a, b| vals[*b].partial_cmp(&vals[*a]).unwrap());
        println!("D1 {tag} top 20 by {name}:");
        for &f in idx.iter().take(20) {
            println!("  face {f}: {:.3} {}", vals[f], describe(f));
        }
    }
    // the worst faces of each class (ADR 0079 D1)
    let class = |f: usize| -> &'static str {
        let o = m.cell_tags[mesh.owner()[f]];
        let (reg, fam) = (region(m, o), family(m, f));
        let end_wall = o.3 < m.n_bl || o.3 + m.n_bl >= m.n_k;
        if reg == "wake/stack" && fam == "j(cut)" {
            "wake cut"
        } else if reg == "wake/stack" {
            "wake stack"
        } else if reg.ends_with("/stack") {
            "blade stack"
        } else if end_wall && (fam == "k" || fam == "bnd:hub" || fam == "bnd:shroud") {
            "end wall"
        } else if reg == "nose/outer" {
            "nose outer"
        } else {
            "other"
        }
    };
    for (name, vals) in [("non-orth", &met.non_orthogonality), ("skewness", &met.skewness)] {
        for cl in [
            "wake cut",
            "wake stack",
            "blade stack",
            "end wall",
            "nose outer",
            "other",
        ] {
            let mut idx: Vec<usize> = (0..vals.len()).filter(|&f| class(f) == cl).collect();
            idx.sort_by(|a, b| vals[*b].partial_cmp(&vals[*a]).unwrap());
            println!("D1 {tag} class {cl}: top 5 by {name}:");
            for &f in idx.iter().take(5) {
                println!("  face {f}: {:.3} {}", vals[f], describe(f));
            }
        }
    }
    let mut idx: Vec<usize> = (0..mesh.n_cells()).collect();
    idx.sort_by(|a, b| met.aspect_ratio[*b].partial_cmp(&met.aspect_ratio[*a]).unwrap());
    println!("D1 {tag} top 10 by aspect:");
    for &c in idx.iter().take(10) {
        let t = m.cell_tags[c];
        println!(
            "  cell {c}: {:.1} {:?} {} {}",
            met.aspect_ratio[c],
            t,
            region(m, t),
            loc(mesh.cell_centres()[c])
        );
    }
    for (name, vals, lim) in [
        ("non-orth > 70", &met.non_orthogonality, 70.0),
        ("skewness > 2", &met.skewness, 2.0),
    ] {
        let mut counts: std::collections::BTreeMap<(String, String), usize> = std::collections::BTreeMap::new();
        for f in 0..mesh.n_faces() {
            if vals[f] > lim {
                let o = m.cell_tags[mesh.owner()[f]];
                *counts.entry((region(m, o), family(m, f))).or_insert(0) += 1;
            }
        }
        println!("D1 {tag}: faces with {name} by (owner region, family): {counts:?}");
    }
}

/// D1 (ADR 0079 part 2): attribution and geometric decomposition of the worst faces of A' and B' with construction E. Measurements only.
#[test]
#[ignore = "diagnostic D1 of ADR 0079, run by hand: cargo test --release -p rcfd-mesh --test cgrid_w35d d1 -- --ignored --nocapture"]
fn d1_attribution_of_construction_e() {
    for (tag, spec) in [("A'", spec_a()), ("B'", spec_b())] {
        let m = build_cgrid_mesh_with(&blade_b(), &spec, &construction_e()).unwrap();
        attribution(tag, &m);
    }
}

// ---------------------------------------------------------------- part 1: FAN-01

/// The construction of the case value `camber_seam` (ADR 0079 F3 with the checked range, G).
fn construction_f() -> CGridOptions {
    CGridOptions::camber_seam_construction()
}

/// Development counts FD of ADR 0079 (FAN-01 geometry, y+ = 30 rule).
fn spec_fd() -> CGridSpec {
    CGridSpec {
        n_up: 10,
        n_chord: 20,
        n_down: 10,
        n_side: 12,
        n_nose: 5,
        n_core: 1,
        n_span: 12,
        n_gap: 6,
        n_bl: 2,
        growth: 1.2,
        first_cell: fan01_first_cell(),
        front_gap: 0.3,
        include_blade: true,
    }
}

/// Smallest angle [deg] between the last segment of a C line and the seam where it meets it, over all layers (90 = orthogonal).
fn line_to_seam_angle(m: &CGridMesh) -> (f64, String) {
    let (n_i, ns) = (m.n_i(), m.n_side());
    let h_c = n_i / 2 - m.n_nose();
    let mut worst = (90.0, String::new());
    for k in 0..=m.n_k {
        let r = m.radii[k];
        let pl = |id: usize| -> (f64, f64) {
            let p = m.grid[id];
            (p[2], r * p[1].atan2(p[0]))
        };
        for i in 1..h_c {
            let (a, b) = (pl(m.c_node(i, ns - 1, k)), pl(m.c_node(i, ns, k)));
            let (p, q) = (pl(m.c_node(i - 1, ns, k)), pl(m.c_node(i + 1, ns, k)));
            let (lx, ly) = (b.0 - a.0, b.1 - a.1);
            let (tx, ty) = (q.0 - p.0, q.1 - p.1);
            let c = ((lx * tx + ly * ty) / (lx.hypot(ly) * tx.hypot(ty))).abs().min(1.0);
            let ang = c.acos().to_degrees();
            if ang < worst.0 {
                worst = (ang, format!("line {i} layer {k}"));
            }
        }
    }
    worst
}

/// DH of ADR 0079: end-wall first-layer and k-face maxima.
fn hub_diagnostic(tag: &str, m: &CGridMesh) {
    let mesh = &m.mesh;
    let met = compute_metrics(mesh);
    let (mut no_first, mut sk_first, mut no_k, mut sk_k) =
        ((0.0f64, 0usize), (0.0f64, 0usize), (0.0f64, 0usize), (0.0f64, 0usize));
    for f in 0..mesh.n_faces() {
        let fam = family(m, f);
        let o = m.cell_tags[mesh.owner()[f]];
        let first = (fam == "bnd:hub" && o.3 == 0) || (fam == "k" && o.3 + 1 < m.n_bl);
        let upd = |w: &mut (f64, usize), v: f64| {
            if v > w.0 {
                *w = (v, f);
            }
        };
        if first {
            upd(&mut no_first, met.non_orthogonality[f]);
            upd(&mut sk_first, met.skewness[f]);
        }
        if fam == "k" {
            upd(&mut no_k, met.non_orthogonality[f]);
            upd(&mut sk_k, met.skewness[f]);
        }
    }
    let at = |f: usize| {
        format!(
            "{:?} {} [{}]",
            m.cell_tags[mesh.owner()[f]],
            region(m, m.cell_tags[mesh.owner()[f]]),
            family(m, f)
        )
    };
    println!(
        "DH {tag}: hub first-layer non-orth {:.2} at {}; skewness {:.3} at {}; k-faces non-orth {:.2} at {}; skewness {:.3} at {}",
        no_first.0,
        at(no_first.1),
        sk_first.0,
        at(sk_first.1),
        no_k.0,
        at(no_k.1),
        sk_k.0,
        at(sk_k.1)
    );
}

/// Development of part 1 on FD and on blade B / S (ADR 0079; not the registered FAN-01 counts).
#[test]
#[ignore = "development of ADR 0079 part 1, run by hand: cargo test --release -p rcfd-mesh --test cgrid_w35d part1_development -- --ignored --nocapture"]
fn part1_development() {
    for (tag, fan, spec) in [("FD", fan01(), spec_fd()), ("S blade B", blade_b(), spec_s())] {
        match build_cgrid_mesh_with(&fan, &spec, &construction_f()) {
            Ok(m) => {
                let q = quality_summary(&m.mesh);
                summary(
                    &format!(
                        "{tag} F (step {}, h1 {:.3e}, h_gap {:.3e})",
                        m.construction_step, m.h1, m.h_gap
                    ),
                    &q,
                );
                let (a, at) = line_to_seam_angle(&m);
                println!("{tag}: smallest line-to-seam angle {a:.1} deg at {at}");
                hub_diagnostic(tag, &m);
                attribution(tag, &m);
            }
            Err(e) => println!("{tag}: build failed {e:?}"),
        }
    }
}

// ---------------------------------------------------------------- part 2: the single-factor changes

/// Q2 measure: (worst closure |sum S| / sum |S|, topology and volumes ok).
fn closure(m: &Mesh) -> (f64, bool) {
    let mut ok = m.validate().is_ok() && m.cell_volumes().iter().all(|&v| v > 0.0);
    for f in 0..m.n_internal_faces() {
        ok &= m.owner()[f] < m.neighbour()[f];
    }
    let sf = m.face_area_vectors();
    let mut worst: f64 = 0.0;
    let mut inc = vec![0usize; m.n_faces()];
    for c in 0..m.n_cells() {
        let (mut sum, mut mag) = ([0.0; 3], 0.0);
        for &f in m.cell_faces(c) {
            inc[f] += 1;
            let sign = if m.owner()[f] == c { 1.0 } else { -1.0 };
            sum = vec3::add(sum, vec3::scale(sf[f], sign));
            mag += vec3::norm(sf[f]);
        }
        worst = worst.max(vec3::norm(sum) / mag);
    }
    for (f, &n) in inc.iter().enumerate() {
        ok &= n == if f < m.n_internal_faces() { 2 } else { 1 };
    }
    (worst, ok)
}

fn rotate(p: Vec3, ang: f64) -> Vec3 {
    let (s, c) = ang.sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}

/// Q6 measures (ADR 0068/0072): largest of the seam-node rotation mismatch, the seam point bijection mismatch, the face-centre
/// mismatch and `|S_hi + R S_lo| / |S_lo|`; `None` when the pairing is not a bijection.
fn periodicity(m: &CGridMesh, n_blades: usize) -> Option<f64> {
    let ang = 2.0 * PI / n_blades as f64;
    let mesh = &m.mesh;
    let mut worst: f64 = 0.0;
    for k in 0..=m.n_k {
        let (lo, hi) = m.seam_nodes(k);
        if lo.len() != hi.len() {
            return None;
        }
        for (a, b) in lo.iter().zip(&hi) {
            worst = worst.max(vec3::norm(vec3::sub(rotate(m.grid[*a], ang), m.grid[*b])));
        }
    }
    let (plo, phi) = (mesh.patch("seam_lo")?, mesh.patch("seam_hi")?);
    if plo.size != phi.size || plo.periodic_with.as_deref() != Some("seam_hi") {
        return None;
    }
    let rot = plo.rotation?;
    if (rot.angle - ang).abs() > 1e-15 {
        return None;
    }
    let pts_of = |p: &rcfd_mesh::mesh::Patch| -> Vec<usize> {
        let mut v: Vec<usize> = p.faces().flat_map(|f| mesh.face_points(f).to_vec()).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let (nlo, nhi) = (pts_of(plo), pts_of(phi));
    if nlo.len() != nhi.len() {
        return None;
    }
    let mut taken = vec![false; nhi.len()];
    for &a in &nlo {
        let t = rot.apply_point(mesh.points()[a]);
        let (mut best, mut bd) = (0, f64::INFINITY);
        for (q, &b) in nhi.iter().enumerate() {
            let d = vec3::norm(vec3::sub(mesh.points()[b], t));
            if d < bd {
                bd = d;
                best = q;
            }
        }
        if taken[best] {
            return None;
        }
        taken[best] = true;
        worst = worst.max(bd);
    }
    let (cf, sf, mat) = (mesh.face_centres(), mesh.face_area_vectors(), rot.matrix());
    let apply = |v: Vec3| -> Vec3 {
        [
            mat[0][0] * v[0] + mat[0][1] * v[1] + mat[0][2] * v[2],
            mat[1][0] * v[0] + mat[1][1] * v[1] + mat[1][2] * v[2],
            mat[2][0] * v[0] + mat[2][1] * v[1] + mat[2][2] * v[2],
        ]
    };
    let mut used = vec![false; phi.size];
    for f in plo.faces() {
        let t = rot.apply_point(cf[f]);
        let (mut best, mut bd) = (0, f64::INFINITY);
        for g in phi.faces() {
            let d = vec3::norm(vec3::sub(cf[g], t));
            if d < bd {
                bd = d;
                best = g;
            }
        }
        if used[best - phi.start] {
            return None;
        }
        used[best - phi.start] = true;
        worst = worst
            .max(bd)
            .max(vec3::norm(vec3::add(sf[best], apply(sf[f]))) / vec3::norm(sf[f]));
    }
    Some(worst)
}

/// Chord station of the wall node of line `i` (the generator's cosine-blend stations) and its side.
fn wall_station(m: &CGridMesh, i: usize) -> (f64, Side) {
    let (nc, nd) = (m.n_chord(), m.n_down());
    let h = nd + nc;
    let xs = |q: usize| {
        let u = q as f64 / nc as f64;
        0.85 * 0.5 * (1.0 - (PI * u).cos()) + 0.15 * u
    };
    if i <= h {
        (xs(nc - (i - nd)), Side::Suction)
    } else {
        (xs(i - h), Side::Pressure)
    }
}

/// Q7 measures (ADR 0068/0072): worst relative deviation of the radial stacks (`h1` at the hub and below the tip, `h_gap` in the gap)
/// and of the blade wall stacks (distance and transverse), in the unrolled plane of each layer.
fn stack_deviation(m: &CGridMesh) -> (f64, f64) {
    let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
    let (g, nb, h1, hg) = (m.growth, m.n_bl, m.h1, m.h_gap);
    let r = &m.radii;
    let nk = m.n_k;
    let mut radial: f64 = 0.0;
    for q in 0..nb {
        for (got, h) in [
            (r[q + 1] - r[q], h1),
            (r[m.n_span - q] - r[m.n_span - q - 1], h1),
            (r[m.n_span + q + 1] - r[m.n_span + q], hg),
            (r[nk - q] - r[nk - q - 1], hg),
        ] {
            radial = radial.max(rel(got, h * g.powi(q as i32)));
        }
    }
    let exact = |q: usize| -> f64 { (0..=q).map(|p| h1 * g.powi(p as i32)).sum() };
    let mut wall: f64 = 0.0;
    let (nd, n_i) = (m.n_down(), m.n_i());
    for k in 0..=m.n_span {
        // in the unrolled plane (z, r theta) of the layer, as ADR 0068/0072 Q7
        let plane = |id: usize| -> (f64, f64) {
            let p = m.grid[id];
            (p[2], r[k] * p[1].atan2(p[0]))
        };
        for i in nd..=n_i - nd {
            let w = plane(m.c_node(i, 0, k));
            let n = m.normals[k][i];
            for q in 1..=nb {
                let p = plane(m.c_node(i, q, k));
                let d = (p.0 - w.0, p.1 - w.1);
                let along = d.0 * n[0] + d.1 * n[1];
                let across = d.0 * n[1] - d.1 * n[0];
                let want = exact(q - 1);
                wall = wall.max(rel(along, want)).max(across.abs() / want);
            }
        }
    }
    (radial, wall)
}

/// Target measures of ADR 0079 (B'): (wake max skewness, blade-stack max skewness, blade-stack max non-orth, nose/outer max
/// skewness, nose-stack max non-orth).
fn target_measures(m: &CGridMesh) -> [f64; 5] {
    let mesh = &m.mesh;
    let met = compute_metrics(mesh);
    let mut t = [0.0f64; 5];
    for f in 0..mesh.n_faces() {
        let reg = region(m, m.cell_tags[mesh.owner()[f]]);
        let (no, sk) = (met.non_orthogonality[f], met.skewness[f]);
        if reg == "wake/stack" {
            t[0] = t[0].max(sk);
        } else if reg.ends_with("/stack") {
            t[1] = t[1].max(sk);
            t[2] = t[2].max(no);
        }
        if reg == "nose/outer" {
            t[3] = t[3].max(sk);
        }
        if reg == "nose/stack" {
            t[4] = t[4].max(no);
        }
    }
    t
}

/// The registered R-E lists of ADR 0072.
fn r_e_list() -> Vec<CGridSpec> {
    let s = spec_s();
    let mut list: Vec<CGridSpec> = Vec::new();
    for fg in [0.2, 0.3, 0.5, 1.0, 1.5, 2.0] {
        list.push(CGridSpec {
            front_gap: fg,
            ..s.clone()
        });
    }
    for nn in [2, 3, 4, 6, 8] {
        list.push(CGridSpec {
            n_nose: nn,
            ..s.clone()
        });
    }
    for ns in [6, 8, 10, 16, 32] {
        list.push(CGridSpec {
            n_side: ns,
            ..s.clone()
        });
    }
    for nc in [12, 16, 24, 32, 64] {
        list.push(CGridSpec {
            n_chord: nc,
            ..s.clone()
        });
    }
    for nc in [16, 24, 32] {
        for nn in [2, 4, 6, 8] {
            for ns in [6, 10, 16, 32] {
                for blade in [true, false] {
                    list.push(CGridSpec {
                        n_chord: nc,
                        n_nose: nn,
                        n_side: ns,
                        include_blade: blade,
                        ..s.clone()
                    });
                }
            }
        }
    }
    list
}

/// R-E scan of ADR 0072: number of combinations that build and validate.
fn r_e_count(opts: &CGridOptions) -> (usize, usize) {
    let list = r_e_list();
    let builds = |sp: &CGridSpec| {
        build_cgrid_mesh_with(&blade_b(), sp, opts)
            .map(|m| m.mesh.validate().is_ok() && m.mesh.cell_volumes().iter().all(|&v| v > 0.0))
            .unwrap_or(false)
    };
    // two threads (the wave's resource limit)
    let (even, odd) = std::thread::scope(|sc| {
        let h = sc.spawn(|| list.iter().step_by(2).filter(|sp| builds(sp)).count());
        let o = list.iter().skip(1).step_by(2).filter(|sp| builds(sp)).count();
        (h.join().unwrap(), o)
    });
    (even + odd, list.len())
}

/// The measures of the ADR 0079 part 2 decision rule for one construction, printed: Q1-E of A' and B', the target measures, Q2 and Q6 on S
/// and A', Q7 on A' and B', the R-E build count. Returns (Q2 topology ok, R-E count). The registered sequence C2, C5, C6, C4 ran with this
/// harness in the record commit 5348934 (all four rejected, ADR 0079 amendment 2); the options were removed afterwards.
fn part2_measure(tag: &str, opts: &CGridOptions) -> Result<(bool, usize), String> {
    let fan = blade_b();
    let bld = |sp: &CGridSpec| build_cgrid_mesh_with(&fan, sp, opts).map_err(|e| format!("{tag}: {e:?}"));
    let s = bld(&spec_s())?;
    let a = bld(&spec_a())?;
    let b = bld(&spec_b())?;
    let (qa, qb) = (quality_summary(&a.mesh), quality_summary(&b.mesh));
    let target = target_measures(&b);
    let (c_s, ok_s) = closure(&s.mesh);
    let (c_a, ok_a) = closure(&a.mesh);
    let q6 = match (periodicity(&s, 5), periodicity(&a, 5)) {
        (Some(x), Some(y)) => Some(x.max(y)),
        _ => None,
    };
    let (q7_a, q7_b) = (stack_deviation(&a), stack_deviation(&b));
    let (r_e, n_re) = r_e_count(opts);
    summary(&format!("P2 {tag} A' (step {})", a.construction_step), &qa);
    summary(&format!("P2 {tag} B' (step {})", b.construction_step), &qb);
    println!(
        "P2 {tag}: targets B' wake skew {:.4}, blade-stack skew {:.4}, blade-stack non-orth {:.3}, nose/outer skew {:.4}, nose-stack non-orth {:.3}; A' aspect {:.2}, B' aspect {:.1}",
        target[0], target[1], target[2], target[3], target[4], qa.max_aspect_ratio, qb.max_aspect_ratio
    );
    println!(
        "P2 {tag}: Q2 closure {:.2e} (S {c_s:.2e}, A' {c_a:.2e}) topology {}; Q6 {:?}; Q7 A' radial {:.2e} wall {:.2e}, B' radial {:.2e} wall {:.2e}; R-E {r_e} of {n_re}",
        c_s.max(c_a),
        ok_s && ok_a,
        q6,
        q7_a.0,
        q7_a.1,
        q7_b.0,
        q7_b.1
    );
    Ok((ok_s && ok_a, r_e))
}

/// Smoke of the part 2 harness on the baseline E alone (no change), ADR 0079 rule (d): it must reproduce the ADR 0072 numbers.
#[test]
#[ignore = "harness smoke of ADR 0079 part 2 on construction E: cargo test --release -p rcfd-mesh --test cgrid_w35d part2_harness_smoke -- --ignored --nocapture"]
fn part2_harness_smoke_on_e() {
    let (q2_ok, r_e) = part2_measure("E", &construction_e()).unwrap();
    assert!(q2_ok && r_e == 107);
}

/// The Q7 part of the harness on A' and B' with construction E (ADR 0072: 3.2e-13 / 1.7e-11).
#[test]
#[ignore = "harness smoke of ADR 0079 part 2 (Q7): cargo test --release -p rcfd-mesh --test cgrid_w35d q7_harness_smoke -- --ignored --nocapture"]
fn q7_harness_smoke_on_e() {
    for spec in [spec_a(), spec_b()] {
        let m = build_cgrid_mesh_with(&blade_b(), &spec, &construction_e()).unwrap();
        println!("Q7 harness on E: {:?}", stack_deviation(&m));
    }
}

// ---------------------------------------------------------------- part 1: the registered FAN-01 checks (P1-P8)

fn build_fan01(spec: &CGridSpec) -> CGridMesh {
    build_fan01_with(spec, &CGridOptions::camber_seam_construction())
}

fn build_fan01_with(spec: &CGridSpec, opts: &CGridOptions) -> CGridMesh {
    let m = build_cgrid_mesh_with(&fan01(), spec, opts).unwrap_or_else(|e| panic!("P1 FAN-01 not built: {e:?}"));
    println!(
        "FAN-01-C: {} cells, construction step {}, h1 {:.4e}, h_gap {:.4e}",
        m.mesh.n_cells(),
        m.construction_step,
        m.h1,
        m.h_gap
    );
    m
}

/// Volume of the blade by the divergence theorem, closed on the hub by the quads of the core block (Q3 of ADR 0072).
fn solid_volume(m: &CGridMesh) -> f64 {
    let mesh = &m.mesh;
    let mut v = 0.0;
    for f in mesh.patch("blade").unwrap().faces() {
        v += -vec3::dot(mesh.face_centres()[f], mesh.face_area_vectors()[f]) / 3.0;
    }
    for t in 0..m.n_core() {
        for q in 0..m.n_chord() {
            let mut l = vec![
                m.core_node(q, t, 0),
                m.core_node(q + 1, t, 0),
                m.core_node(q + 1, t + 1, 0),
                m.core_node(q, t + 1, 0),
            ];
            l.dedup();
            if l.first() == l.last() {
                l.pop();
            }
            let (c, mut s) = rcfd_mesh::mesh::polygon_geometry(&m.grid, &l);
            if s[0] * c[0] + s[1] * c[1] > 0.0 {
                s = vec3::scale(s, -1.0);
            }
            v += vec3::dot(s, c) / 3.0;
        }
    }
    v
}

/// P1, P3-P8 of ADR 0079 on FAN-01-C (the P2 quality gates are a separate test).
#[test]
fn p1_p8_fan01_mesh_builds_closes_is_periodic_and_exact() {
    p_checks(&spec_fan01());
}

/// Smoke of the P1-P8 harness on the development counts FD (ADR 0079 rule (d)), not the registered set-up.
#[test]
#[ignore = "harness smoke on FD: cargo test --release -p rcfd-mesh --test cgrid_w35d p_checks_smoke -- --ignored --nocapture"]
fn p_checks_smoke_on_fd() {
    p_checks(&spec_fd());
}

fn p_checks(spec: &CGridSpec) {
    p_checks_with(spec, &CGridOptions::camber_seam_construction());
}

/// P1, P3-P8 with the given construction; with `lean_stack` the wall stacks (P5) are measured along the 3-D surface normal (ADR 0083 H3).
fn p_checks_with(spec: &CGridSpec, opts: &CGridOptions) {
    let fan = fan01();
    let m = build_fan01_with(spec, opts);
    // P1
    assert!(m.mesh.validate().is_ok());
    assert!(m.mesh.cell_volumes().iter().all(|&v| v > 0.0));
    let q = quality_summary(&m.mesh);
    summary("P2 FAN-01-C (reported here; gated in p2)", &q);
    // P3
    let (c, ok) = closure(&m.mesh);
    println!("P3 closure {c:.2e}, topology {ok}");
    assert!(ok && c <= 1e-13);
    // P4
    let per = periodicity(&m, 9);
    println!("P4 periodicity {per:?}");
    assert!(per.is_some_and(|x| x <= 1e-12));
    // P5
    let (radial, wall_ip) = stack_deviation(&m);
    let wall = if opts.lean_stack {
        wall_stack_deviation_3d(&fan, &m)
    } else {
        wall_ip
    };
    println!("P5 stacks: radial {radial:.2e}, wall {wall:.2e} (in-plane form {wall_ip:.2e})");
    assert!(radial <= 1e-12 && wall <= 1e-12);
    // P6
    let empty = build_cgrid_mesh_with(
        &fan,
        &CGridSpec {
            include_blade: false,
            ..spec.clone()
        },
        opts,
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    for k in 0..=m.n_k {
        for i in 0..=m.n_i() {
            for j in 0..=m.n_side() {
                worst = worst.max(vec3::norm(vec3::sub(
                    m.grid[m.c_node(i, j, k)],
                    empty.grid[empty.c_node(i, j, k)],
                )));
            }
        }
    }
    let (vp, ve, vs) = (m.mesh.total_volume(), empty.mesh.total_volume(), solid_volume(&m));
    let res = (vp + vs - ve).abs() / ve;
    println!("P6 C nodes bladed vs empty {worst:.1e}; passage {vp:.10e} solid {vs:.10e} envelope {ve:.10e}; residual {res:.2e}");
    assert!(worst == 0.0 && res <= 1e-11);
    // P7
    let mut off: f64 = 0.0;
    for k in 0..=m.n_span {
        for i in m.n_down()..=m.n_i() - m.n_down() {
            let (x, side) = wall_station(&m, i);
            off = off.max(vec3::norm(vec3::sub(
                m.grid[m.c_node(i, 0, k)],
                fan.surface_point(m.radii[k], x, side),
            )));
        }
    }
    println!("P7 wall nodes off the exact surface by at most {off:.2e} m");
    assert!(off <= 1e-12);
    // P8 (reported)
    let gl = |n: usize, a: f64, b: f64| -> Vec<(f64, f64)> {
        let (x, w) = gauss_legendre(n);
        x.iter()
            .zip(&w)
            .map(|(x, w)| (0.5 * (a + b) + 0.5 * (b - a) * x, 0.5 * (b - a) * w))
            .collect()
    };
    // piecewise-linear stations: integrate each station interval separately
    let mut v_ref = 0.0;
    for w2 in fan.stations.windows(2) {
        v_ref += gl(8, w2[0].radius, w2[1].radius)
            .iter()
            .map(|(r, w)| w * fan.section_area(*r))
            .sum::<f64>();
    }
    println!(
        "P8 solid volume {vs:.6e} against the section-area quadrature {v_ref:.6e}: relative {:.3e}",
        (vs - v_ref).abs() / v_ref
    );
}

/// P2 of ADR 0079: the ADR 0072 A' gates (70 deg, 2.0, 300) as the target on FAN-01-C. KNOWN MISS (a3): 83.26 deg, skewness 3.978 (aspect 227.2
/// met).
#[test]
#[ignore = "known miss: FAN-01-C non-orthogonality 83.26 deg and skewness 3.98 against 70 / 2.0 (aspect 227 met), ADR 0079 amendment 5"]
fn p2_fan01_quality_gates_known_miss() {
    let m = build_fan01(&spec_fan01());
    let q = quality_summary(&m.mesh);
    summary("P2 FAN-01-C", &q);
    assert!(
        q.max_non_orthogonality <= 70.0,
        "non-orthogonality {}",
        q.max_non_orthogonality
    );
    assert!(q.max_skewness <= 2.0, "skewness {}", q.max_skewness);
    assert!(q.max_aspect_ratio <= 300.0, "aspect {}", q.max_aspect_ratio);
}

/// Attribution of the FAN-01-C mesh (diagnostic after a registered build; measurements only).
#[test]
#[ignore = "diagnostic of ADR 0079 part 1: cargo test --release -p rcfd-mesh --test cgrid_w35d fan01_attribution -- --ignored --nocapture"]
fn fan01_attribution() {
    let m = build_fan01(&spec_fan01());
    attribution("FAN-01-C", &m);
    hub_diagnostic("FAN-01-C", &m);
    let (a, at) = line_to_seam_angle(&m);
    println!("FAN-01-C: smallest line-to-seam angle {a:.1} deg at {at}");
}

// ---------------------------------------------------------------- part 3: the control range

/// One scan entry: (label, fan, spec).
type ScanCase = (String, AxialFan, CGridSpec);

/// Builds a scan list with two threads (the wave's limit); prints one line per combination and returns
/// (label, built and valid, construction step, meets the A' gates 70 / 2.0 / 300, non-orthogonality, error path).
fn run_scan_g(tag: &str, list: &[ScanCase], opts: &CGridOptions) -> Vec<(String, bool, usize, bool, f64, String)> {
    let one = |(label, fan, spec): &ScanCase| match build_cgrid_mesh_with(fan, spec, opts) {
        Ok(m) => {
            let valid = m.mesh.validate().is_ok() && m.mesh.cell_volumes().iter().all(|&v| v > 0.0);
            let q = quality_summary(&m.mesh);
            let meets = q.max_non_orthogonality <= 70.0 && q.max_skewness <= 2.0 && q.max_aspect_ratio <= 300.0;
            (
                label.clone(),
                valid,
                m.construction_step,
                meets && valid,
                q.max_non_orthogonality,
                format!("skew {:.3} aspect {:.1}", q.max_skewness, q.max_aspect_ratio),
            )
        }
        Err(e) => (
            label.clone(),
            false,
            99,
            false,
            f64::NAN,
            format!("ERROR {}: {}", e[0].path, e[0].message),
        ),
    };
    let (even, odd) = std::thread::scope(|sc| {
        let h = sc.spawn(|| list.iter().step_by(2).map(one).collect::<Vec<_>>());
        let o: Vec<_> = list.iter().skip(1).step_by(2).map(one).collect();
        (h.join().unwrap(), o)
    });
    // back in list order (entry q of the list is even[q / 2] or odd[q / 2])
    let mut a = Vec::with_capacity(list.len());
    let (mut e, mut o) = (even.into_iter(), odd.into_iter());
    for q in 0..list.len() {
        a.push(if q % 2 == 0 { e.next() } else { o.next() }.expect("one result per entry"));
    }
    for r in &a {
        println!(
            "SCAN {tag} | {} | built {} | step {} | meets {} | non-orth {:.2} | {}",
            r.0, r.1, r.2, r.3, r.4, r.5
        );
    }
    a
}

/// Development scan of ADR 0079 part 3 (combinations different from R-E and R-G).
#[test]
#[ignore = "development scan of ADR 0079 part 3: cargo test --release -p rcfd-mesh --test cgrid_w35d part3_development_scan -- --ignored --nocapture"]
fn part3_development_scan() {
    let mut list: Vec<ScanCase> = Vec::new();
    for (fname, fan, base) in [("B", blade_b(), spec_s()), ("FAN-01", fan01(), spec_fd())] {
        if std::env::var("SCAN_ONLY").is_ok_and(|v| v != fname) {
            continue;
        }
        for nc in [12, 20, 28, 40] {
            for nn in [2, 3, 5, 7, 10] {
                for ns in [5, 8, 12, 20, 28] {
                    if nn + 2 > nc {
                        continue;
                    }
                    list.push((
                        format!("{fname} n_chord {nc} n_nose {nn} n_side {ns}"),
                        fan.clone(),
                        CGridSpec {
                            n_chord: nc,
                            n_nose: nn,
                            n_side: ns,
                            ..base.clone()
                        },
                    ));
                }
            }
        }
        for fg in [0.15, 0.25, 0.4, 0.7, 1.2, 2.5] {
            list.push((
                format!("{fname} front_gap {fg}"),
                fan.clone(),
                CGridSpec {
                    front_gap: fg,
                    ..base.clone()
                },
            ));
        }
    }
    let res = run_scan_g("DEV", &list, &CGridOptions::camber_seam_construction());
    println!(
        "DEV: {} of {} build, {} meet the A' gates",
        res.iter().filter(|r| r.1).count(),
        res.len(),
        res.iter().filter(|r| r.3).count()
    );
}

/// Located errors of the control range (ADR 0079 amendment 5): only with `checked_range`; one error per offending control.
#[test]
fn control_range_is_checked_only_by_the_camber_seam_construction() {
    let g = CGridOptions::camber_seam_construction();
    for (spec, path) in [
        (CGridSpec { n_nose: 2, ..spec_s() }, "n_nose"),
        (CGridSpec { n_nose: 7, ..spec_s() }, "n_nose"),
        (CGridSpec { n_side: 7, ..spec_s() }, "n_side"),
        (CGridSpec { n_side: 33, ..spec_s() }, "n_side"),
        (
            CGridSpec {
                front_gap: 1.2,
                ..spec_s()
            },
            "front_gap",
        ),
    ] {
        let e = build_cgrid_mesh_with(&blade_b(), &spec, &g).unwrap_err();
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].path, path);
        assert!(e[0].message.contains("documented range"), "{}", e[0].message);
        // the other constructions do not check the range
        if let Err(e) = build_cgrid_mesh_with(&blade_b(), &spec, &construction_e()) {
            assert!(e.iter().all(|x| !x.message.contains("documented range")), "{e:?}");
        }
    }
    for spec in [spec_s(), spec_a(), spec_b(), spec_fd(), spec_fan01()] {
        assert!(control_range_problems(&spec).is_empty(), "{spec:?}");
    }
}

/// The registered R-G list (ADR 0079 amendment 5) on blade B (S counts otherwise) and FAN-01 (FAN-01-C counts otherwise).
fn r_g_list() -> Vec<ScanCase> {
    let mut list: Vec<ScanCase> = Vec::new();
    for (fname, fan, base) in [("B", blade_b(), spec_s()), ("FAN-01", fan01(), spec_fan01())] {
        for nc in [14, 18, 24, 36] {
            for nn in [3, 4, 6, 9] {
                for ns in [9, 14, 24, 40] {
                    if nn + 2 > nc {
                        continue;
                    }
                    list.push((
                        format!("{fname} n_chord {nc} n_nose {nn} n_side {ns}"),
                        fan.clone(),
                        CGridSpec {
                            n_chord: nc,
                            n_nose: nn,
                            n_side: ns,
                            ..base.clone()
                        },
                    ));
                }
            }
        }
        for fg in [0.1, 0.18, 0.35, 0.6, 0.9, 1.5] {
            list.push((
                format!("{fname} front_gap {fg}"),
                fan.clone(),
                CGridSpec {
                    front_gap: fg,
                    ..base.clone()
                },
            ));
        }
    }
    list
}

/// R-G gate (b): every out-of-range combination of the R-G list is refused with a located error naming `n_nose`, `n_side` or `front_gap`.
#[test]
fn r_g_out_of_range_combinations_are_located_errors() {
    let out: Vec<ScanCase> = r_g_list()
        .into_iter()
        .filter(|(_, _, sp)| !control_range_problems(sp).is_empty())
        .collect();
    let res = run_scan_g("R-G out", &out, &CGridOptions::camber_seam_construction());
    let bad: Vec<String> = res
        .iter()
        .filter(|r| {
            let ok = r.5.starts_with("ERROR n_nose")
                || r.5.starts_with("ERROR n_side")
                || r.5.starts_with("ERROR front_gap");
            !(ok && r.5.contains("documented range"))
        })
        .map(|r| format!("{}: {}", r.0, r.5))
        .collect();
    println!(
        "R-G out of range: {} combinations, {} located errors",
        out.len(),
        out.len() - bad.len()
    );
    assert!(bad.is_empty(), "{bad:#?}");
}

/// R-G gate (a): every in-range combination builds and validates, per fan. KNOWN MISS: blade B 35 of 38, FAN-01 20 of 39 (ADR 0079 addendum).
#[test]
#[ignore = "known miss: R-G in-range builds, blade B 35 of 38 and FAN-01 20 of 39 (folds in the leading-edge fan), ADR 0079 addendum; about 9 min"]
fn r_g_in_range_combinations_build_known_miss() {
    let inr: Vec<ScanCase> = r_g_list()
        .into_iter()
        .filter(|(_, _, sp)| control_range_problems(sp).is_empty())
        .collect();
    let res = run_scan_g("R-G in", &inr, &CGridOptions::camber_seam_construction());
    let mut fails = Vec::new();
    for fname in ["B", "FAN-01"] {
        let mine: Vec<_> = res.iter().filter(|r| r.0.starts_with(&format!("{fname} "))).collect();
        let built: Vec<_> = mine.iter().filter(|r| r.1).collect();
        let mut no: Vec<f64> = built.iter().map(|r| r.4).collect();
        no.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut steps = std::collections::BTreeMap::new();
        for r in &built {
            *steps.entry(r.2).or_insert(0) += 1;
        }
        println!(
            "R-G {fname}: in range {}: built {}, built AND meeting 70 / 2.0 / 300 {}, below 85 deg {}, median non-orthogonality {:.2}, steps {steps:?}",
            mine.len(),
            built.len(),
            mine.iter().filter(|r| r.3).count(),
            built.iter().filter(|r| r.4 < 85.0).count(),
            no.get(no.len() / 2).copied().unwrap_or(f64::NAN)
        );
        fails.extend(mine.iter().filter(|r| !r.1).map(|r| format!("{}: {}", r.0, r.5)));
    }
    assert!(fails.is_empty(), "{} in-range failures: {fails:#?}", fails.len());
}

/// ADR 0079 part 2 (registered): the Q1-E gates of A' and B' for construction G itself (blade B).
fn q1_g(spec: CGridSpec, aspect_limit: f64, tag: &str) {
    let m = build_cgrid_mesh_with(&blade_b(), &spec, &CGridOptions::camber_seam_construction()).unwrap();
    let q = quality_summary(&m.mesh);
    summary(&format!("Q1-G {tag} (step {})", m.construction_step), &q);
    assert!(
        q.max_non_orthogonality <= 70.0,
        "non-orthogonality {}",
        q.max_non_orthogonality
    );
    assert!(q.max_skewness <= 2.0, "skewness {}", q.max_skewness);
    assert!(q.max_aspect_ratio <= aspect_limit, "aspect {}", q.max_aspect_ratio);
}

/// KNOWN MISS: A' with G 66.18 deg, skewness 0.889, aspect 394.4 (aspect above 300; E: 395.1).
#[test]
#[ignore = "known miss: A' with construction G aspect 394.4 > 300 (66.18 deg and 0.889 met), ADR 0079 addendum"]
fn q1g_quality_gates_a_prime_known_miss() {
    q1_g(spec_a(), 300.0, "A'");
}

/// KNOWN MISS: B' with G 80.94 deg, skewness 8.897, aspect 8805 (E: 78.17 / 7.08 / 8532).
#[test]
#[ignore = "known miss: B' with construction G 80.94 deg, skewness 8.90, aspect 8805 against 70 / 2.0 / 3000, ADR 0079 addendum"]
fn q1g_quality_gates_b_prime_known_miss() {
    q1_g(spec_b(), 3000.0, "B'");
}

// ---------------------------------------------------------------- wave 36D (ADR 0083): the H changes (the H5 range and its R-H scan were rejected and removed)

/// 3-D unit surface normal at wall line `i` of layer `k` and the in-plane normal as a 3-D vector (ADR 0079 C5 / ADR 0083 H3).
fn normals_3d(fan: &AxialFan, m: &CGridMesh, i: usize, k: usize) -> (Vec3, Vec3) {
    let r = m.radii[k];
    let w = m.grid[m.c_node(i, 0, k)];
    let th = w[1].atan2(w[0]);
    let e_t = [-th.sin(), th.cos(), 0.0];
    let n = m.normals[k][i];
    let n_ip = [n[1] * e_t[0], n[1] * e_t[1], n[0]];
    let t_ip = [n[0] * e_t[0], n[0] * e_t[1], -n[1]];
    let (x, side) = wall_station(m, i);
    let dr = 1e-6 * r;
    let t_r = vec3::sub(fan.surface_point(r + dr, x, side), fan.surface_point(r - dr, x, side));
    let mut nn = vec3::unit(vec3::cross(t_ip, t_r));
    if vec3::dot(nn, n_ip) < 0.0 {
        nn = vec3::scale(nn, -1.0);
    }
    (nn, n_ip)
}

/// Q7 wall-stack deviation measured along the 3-D surface normal (the in-plane offset times cos(lean)), ADR 0083 H3; the transverse
/// deviation against the in-plane normal line.
fn wall_stack_deviation_3d(fan: &AxialFan, m: &CGridMesh) -> f64 {
    let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
    let (g, nb, h1) = (m.growth, m.n_bl, m.h1);
    let exact = |q: usize| -> f64 { (0..=q).map(|p| h1 * g.powi(p as i32)).sum() };
    let mut wall: f64 = 0.0;
    let (nd, n_i) = (m.n_down(), m.n_i());
    for k in 0..=m.n_span {
        let r = m.radii[k];
        let plane = |id: usize| -> (f64, f64) {
            let p = m.grid[id];
            (p[2], r * p[1].atan2(p[0]))
        };
        for i in nd..=n_i - nd {
            let w = plane(m.c_node(i, 0, k));
            let n = m.normals[k][i];
            let (nn, n_ip) = normals_3d(fan, m, i, k);
            let cos_l = vec3::dot(nn, n_ip).abs();
            for q in 1..=nb {
                let p = plane(m.c_node(i, q, k));
                let d = (p.0 - w.0, p.1 - w.1);
                let along = (d.0 * n[0] + d.1 * n[1]) * cos_l;
                let across = d.0 * n[1] - d.1 * n[0];
                let want = exact(q - 1);
                wall = wall.max(rel(along, want)).max(across.abs() / want);
            }
        }
    }
    wall
}

/// Construction G plus the ADR 0083 changes named by letters: U = H1 `post_untangle`, L = H3 `lean_stack` (H4 `nose_blend`, letter N, was
/// rejected and removed: with H1 + H3 + H4 set-up S did not build).
fn g_plus(letters: &str) -> CGridOptions {
    CGridOptions {
        post_untangle: letters.contains('U'),
        lean_stack: letters.contains('L'),
        ..CGridOptions::camber_seam_construction()
    }
}

/// In-range R-G builds per fan (blade B, FAN-01) under the G range, with the given options.
fn r_g_builds(tag: &str, opts: &CGridOptions) -> (usize, usize, usize, usize) {
    let inr: Vec<ScanCase> = r_g_list()
        .into_iter()
        .filter(|(_, _, sp)| control_range_problems(sp).is_empty())
        .collect();
    let res = run_scan_g(tag, &inr, opts);
    let count = |f: &str| {
        let mine: Vec<_> = res.iter().filter(|r| r.0.starts_with(&format!("{f} "))).collect();
        (mine.iter().filter(|r| r.1).count(), mine.len())
    };
    let ((bb, nb), (bf, nf)) = (count("B"), count("FAN-01"));
    (bb, nb, bf, nf)
}

/// The measures of the ADR 0083 part B decision rule for G plus the changes in `letters` (env `PART_B`), printed. Run once per step of the
/// registered sequence (about 12 minutes each).
#[test]
#[ignore = "ADR 0083 part B sequence step, run by hand: PART_B=<letters> cargo test --release -p rcfd-mesh --test cgrid_w35d part_b_step -- --ignored --nocapture"]
fn part_b_step() {
    let letters = std::env::var("PART_B").unwrap_or_default();
    let opts = g_plus(&letters);
    let tag = format!("G+{letters}");
    let (fb, ff) = (blade_b(), fan01());
    let s = build_cgrid_mesh_with(&fb, &spec_s(), &opts).unwrap();
    let a = build_cgrid_mesh_with(&fb, &spec_a(), &opts).unwrap();
    let b = build_cgrid_mesh_with(&fb, &spec_b(), &opts).unwrap();
    let c = build_cgrid_mesh_with(&ff, &spec_fan01(), &opts).unwrap();
    for (t, m) in [("S", &s), ("A'", &a), ("B'", &b), ("FAN-01-C", &c)] {
        summary(
            &format!(
                "PB {tag} {t} (step {}, hash {:#018x})",
                m.construction_step,
                mesh_hash(&m.mesh)
            ),
            &quality_summary(&m.mesh),
        );
    }
    let tb = target_measures(&b);
    let tc = target_measures(&c);
    println!(
        "PB {tag} targets: B' blade-stack skewness {:.4} (non-orth {:.3}), B' nose/outer skewness {:.4}, B' wake skewness {:.4}; FAN-01-C wake skewness {:.4}",
        tb[1], tb[2], tb[3], tb[0], tc[0]
    );
    let mut ok = true;
    for (t, m, nbl) in [("S", &s, 5usize), ("A'", &a, 5), ("FAN-01-C", &c, 9)] {
        let (cl, topo) = closure(&m.mesh);
        let per = periodicity(m, nbl);
        println!("PB {tag} {t}: closure {cl:.2e} topology {topo}; periodicity {per:?}");
        ok &= topo && cl <= 1e-13 && per.is_some_and(|x| x <= 1e-12);
    }
    let lean = letters.contains('L');
    for (t, m, fan, lim) in [
        ("A'", &a, &fb, 1e-12),
        ("FAN-01-C", &c, &ff, 1e-12),
        ("B'", &b, &fb, 4.9e-11),
    ] {
        let (radial, wall_ip) = stack_deviation(m);
        let wall = if lean { wall_stack_deviation_3d(fan, m) } else { wall_ip };
        println!(
            "PB {tag} {t}: stacks radial {radial:.2e}, wall {wall:.2e} ({}; in-plane form {wall_ip:.2e})",
            if lean { "along the 3-D normal" } else { "in-plane" }
        );
        ok &= radial <= lim && wall <= lim;
    }
    let (bb, nb, bf, nf) = r_g_builds(&format!("PB {tag}"), &opts);
    println!("PB {tag}: R-G in-range builds blade B {bb} of {nb}, FAN-01 {bf} of {nf}; (iii) {ok}");
}

/// B1 of ADR 0083: construction G (case value `camber_seam`) is bit-identical to the HEAD of the ADR 0083 registration (c89495f).
#[test]
fn b1_construction_g_is_bit_identical() {
    let g = CGridOptions::camber_seam_construction();
    for (tag, fan, spec, want) in [
        ("S", blade_b(), spec_s(), 0x59b26da9febc9b2eu64),
        ("FAN-01-C", fan01(), spec_fan01(), 0x00b3e9673d470e52),
    ] {
        let got = mesh_hash(&build_cgrid_mesh_with(&fan, &spec, &g).unwrap().mesh);
        println!("B1 G {tag}: {got:#018x}");
        assert_eq!(got, want, "{tag}");
    }
}

/// ADR 0083 C1: P1, P3-P8 of ADR 0079 on FAN-01-C with construction H.
#[test]
fn c1_fan01_h_mesh_builds_closes_is_periodic_and_exact() {
    p_checks_with(&spec_fan01(), &CGridOptions::camber_seam_h_construction());
}

/// ADR 0083 C1, P2: the quality target 70 deg / 2.0 / 300 on FAN-01-C with construction H. KNOWN MISS: 83.26 deg, skewness 3.978 (aspect 227.2
/// met), the lower seam corner of the tip layers and the wake line 5 (ADR 0083 amendment 1, DC and DW).
#[test]
#[ignore = "known miss: FAN-01-C with H 83.26 deg and skewness 3.98 against 70 / 2.0 (aspect 227 met), ADR 0083 addendum"]
fn c1_p2_fan01_h_quality_gates_known_miss() {
    let m = build_fan01_with(&spec_fan01(), &CGridOptions::camber_seam_h_construction());
    let q = quality_summary(&m.mesh);
    summary("C1 P2 FAN-01-C H", &q);
    assert!(
        q.max_non_orthogonality <= 70.0,
        "non-orthogonality {}",
        q.max_non_orthogonality
    );
    assert!(q.max_skewness <= 2.0, "skewness {}", q.max_skewness);
    assert!(q.max_aspect_ratio <= 300.0, "aspect {}", q.max_aspect_ratio);
}

/// ADR 0083 C5: the ADR 0072 Q1 gates of A' (70 / 2.0 / 300) and B' (70 / 2.0 / 3000) with construction H. DEFECTIVE SET-UP (ADR 0083
/// amendment 3): A' and B' lay outside the H5 range and were refused; the corrected test is `c5b_q1_gates_with_h_meshes_known_miss` (H5 was
/// rejected afterwards, ADR 0083 addendum, so this set-up now builds the same meshes as c5b).
#[test]
#[ignore = "defective set-up under the rejected H5 range (ADR 0083 amendment 3); corrected in c5b"]
fn c5_q1_gates_with_h() {
    q1_h(&CGridOptions::camber_seam_h_construction());
}

/// ADR 0083 C5b: the Q1 gates of A' and B' with the mesh options of H and the range check off. KNOWN MISS: A' 66.18 / 0.888 / 394.4 (aspect),
/// B' 80.93 / 8.894 / 8805.0 (all three).
#[test]
#[ignore = "known miss: A' with H meshes aspect 394.4 > 300, B' 80.93 deg / 8.89 / 8805 against 70 / 2.0 / 3000, ADR 0083 addendum"]
fn c5b_q1_gates_with_h_meshes_known_miss() {
    q1_h(&CGridOptions {
        checked_range: false,
        ..CGridOptions::camber_seam_h_construction()
    });
}

fn q1_h(h: &CGridOptions) {
    let h = *h;
    let mut miss = Vec::new();
    for (tag, spec, lim) in [("A'", spec_a(), 300.0), ("B'", spec_b(), 3000.0)] {
        let m = build_cgrid_mesh_with(&blade_b(), &spec, &h).unwrap();
        let q = quality_summary(&m.mesh);
        summary(&format!("C5 Q1-H {tag} (step {})", m.construction_step), &q);
        if !(q.max_non_orthogonality <= 70.0 && q.max_skewness <= 2.0 && q.max_aspect_ratio <= lim) {
            miss.push(tag);
        }
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}

// ---------------------------------------------------------------- wave 37D (ADR 0087): construction I (inclined front edge) was rejected by its
// registered keep clause and removed with its tests (the numbers are in the ADR 0087 addendum)

/// B1 of ADR 0087: construction H (case value `camber_seam_h`) is bit-identical to the HEAD of the ADR 0087 registration (6f97dcb).
#[test]
fn b1_construction_h_is_bit_identical() {
    let h = CGridOptions::camber_seam_h_construction();
    for (tag, fan, spec, want) in [
        ("S", blade_b(), spec_s(), 0x6f6342d53409da61u64),
        ("FAN-01-C", fan01(), spec_fan01(), 0x21104bbbdd9b9129),
    ] {
        let got = mesh_hash(&build_cgrid_mesh_with(&fan, &spec, &h).unwrap().mesh);
        println!("B1 H {tag}: {got:#018x}");
        assert_eq!(got, want, "{tag}");
    }
}
