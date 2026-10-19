//! Wave 34A (ADR 0072 part 2): the registered checks Q1-E to Q7-E, Q4a', Q4b/Q5-E and the robustness scan R-E of the C-grid passage mesh
//! built with construction E (Amendment 4). Limits as ADR 0065/0068, registered before this code ran; a miss becomes an ignored
//! `*_known_miss` test with the limit unchanged. Helpers copied from `fan_cgrid.rs` (wave 33A).

/// Construction E of ADR 0072 Amendment 4.
fn construction_e() -> CGridOptions {
    CGridOptions {
        radial_remap: true,
        smoothing: true,
        ..CGridOptions::default()
    }
}
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_mesh::{first_cell_height_from_yplus, FirstCell, QualitySummary};
use rcfd_mesh::mesh::Mesh;
use std::f64::consts::PI;

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

/// Registered set-up A' (y+ = 30).
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

/// Registered set-up B' (y+ = 1).
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

/// Registered small set-up S.
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

fn build(fan: &AxialFan, spec: &CGridSpec) -> CGridMesh {
    build_cgrid_mesh_with(fan, spec, &construction_e()).unwrap_or_else(|e| panic!("{e:?}"))
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

fn gl(n: usize, a: f64, b: f64) -> Vec<(f64, f64)> {
    let (x, w) = gauss_legendre(n);
    x.iter()
        .zip(&w)
        .map(|(x, w)| (0.5 * (a + b) + 0.5 * (b - a) * x, 0.5 * (b - a) * w))
        .collect()
}

fn report(tag: &str, m: &CGridMesh) -> QualitySummary {
    let q = m.quality();
    println!(
        "Q1 {tag}: {} cells; non-orth max {:.2} mean {:.2} p99 {:.2} deg, >50: {}, >60: {}; skewness max {:.3}; aspect max {:.1}; min volume {:.3e}; max volume ratio {:.2}; h1 {:.3e}",
        m.mesh.n_cells(),
        q.max_non_orthogonality,
        q.mean_non_orthogonality,
        q.p99_non_orthogonality,
        q.n_above_50,
        q.n_above_60,
        q.max_skewness,
        q.max_aspect_ratio,
        q.min_volume,
        q.max_volume_ratio,
        m.h1
    );
    q
}

// ---------------------------------------------------------------- Q1 quality

// -------------------------------------------------------- Q2 closure / topology

fn closed_cells_and_topology(m: &Mesh) {
    m.validate().unwrap();
    for f in 0..m.n_internal_faces() {
        assert!(m.owner()[f] < m.neighbour()[f]);
    }
    assert!(m.cell_volumes().iter().all(|&v| v > 0.0));
    let sf = m.face_area_vectors();
    let mut worst: f64 = 0.0;
    for c in 0..m.n_cells() {
        let (mut sum, mut mag) = ([0.0; 3], 0.0);
        for &f in m.cell_faces(c) {
            let sign = if m.owner()[f] == c { 1.0 } else { -1.0 };
            sum = vec3::add(sum, vec3::scale(sf[f], sign));
            mag += vec3::norm(sf[f]);
        }
        worst = worst.max(vec3::norm(sum) / mag);
    }
    println!("Q2 worst closure |sum S|/sum|S| = {worst:.2e}");
    assert!(worst <= 1e-13);
    let mut inc = vec![0usize; m.n_faces()];
    for c in 0..m.n_cells() {
        for &f in m.cell_faces(c) {
            inc[f] += 1;
        }
    }
    for (f, &n) in inc.iter().enumerate() {
        assert_eq!(n, if f < m.n_internal_faces() { 2 } else { 1 }, "face {f}");
    }
}

// ----------------------------------------------------------- Q3 volume conservation

/// Volume of the blade by the divergence theorem; the closure on the hub is `closure(mesh)`: the registered one (a flat polygon
/// through the 2 n_chord root-section nodes) or the quads of the core block (the faces the empty mesh really has there).
fn solid_volume(m: &CGridMesh, quads: bool) -> f64 {
    let mesh = &m.mesh;
    let mut v = 0.0;
    for f in mesh.patch("blade").unwrap().faces() {
        v += -vec3::dot(mesh.face_centres()[f], mesh.face_area_vectors()[f]) / 3.0;
    }
    let mut loops: Vec<Vec<usize>> = Vec::new();
    if quads {
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
                loops.push(l);
            }
        }
    } else {
        loops.push((0..2 * m.n_chord()).map(|ch| m.wall_node(ch, 0)).collect());
    }
    for ids in loops {
        let (c, mut s) = rcfd_mesh::mesh::polygon_geometry(&m.grid, &ids);
        if s[0] * c[0] + s[1] * c[1] > 0.0 {
            s = vec3::scale(s, -1.0);
        }
        v += vec3::dot(s, c) / 3.0;
    }
    v
}

fn q3_residual(spec: &CGridSpec, quads: bool) -> (f64, f64, f64, f64) {
    let fan = blade_b();
    let sm = build(&fan, spec);
    let empty = build(
        &fan,
        &CGridSpec {
            include_blade: false,
            ..spec.clone()
        },
    );
    let (vp, ve, vs) = (
        sm.mesh.total_volume(),
        empty.mesh.total_volume(),
        solid_volume(&sm, quads),
    );
    (vp, ve, vs, (vp + vs - ve).abs() / ve)
}

// ------------------------------------------------- Q4 refinement order of the volumes

/// Area of the lateral blade faces: the `blade` patch without the tip cap (faces whose nodes all lie at the tip radius).
fn lateral_blade_area(m: &CGridMesh) -> f64 {
    let mesh = &m.mesh;
    let r_tip = *m.radii.get(m.n_span).unwrap();
    let mut a = 0.0;
    for f in mesh.patch("blade").unwrap().faces() {
        let cap = mesh.face_points(f).iter().all(|&p| {
            let q = mesh.points()[p];
            (q[0].hypot(q[1]) - r_tip).abs() < 1e-9
        });
        if !cap {
            a += mesh.face_area(f);
        }
    }
    a
}

fn lateral_area_quadrature(fan: &AxialFan) -> f64 {
    let h = 1e-6;
    let mut a = 0.0;
    for side in [Side::Suction, Side::Pressure] {
        for (r, wr) in gl(10, fan.r_hub, fan.r_tip) {
            for q in 0..24 {
                for (b, wb) in gl(16, PI * q as f64 / 24.0, PI * (q + 1) as f64 / 24.0) {
                    let x = |b: f64| 0.5 * (1.0 - b.cos());
                    let xr = (
                        fan.surface_point(r + h, x(b), side),
                        fan.surface_point(r - h, x(b), side),
                    );
                    let xb = (
                        fan.surface_point(r, x(b + h), side),
                        fan.surface_point(r, x(b - h), side),
                    );
                    let dr = vec3::scale(vec3::sub(xr.0, xr.1), 0.5 / h);
                    let db = vec3::scale(vec3::sub(xb.0, xb.1), 0.5 / h);
                    a += wr * wb * vec3::norm(vec3::cross(db, dr));
                }
            }
        }
    }
    a
}

// -------------------------------------------------------- Q6 periodicity / rotation

fn rotate(p: Vec3, ang: f64) -> Vec3 {
    let (s, c) = ang.sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}

fn check_periodic_pair(m: &CGridMesh, tol: f64) {
    let ang = 2.0 * PI / 5.0;
    let mesh = &m.mesh;
    let mut worst: f64 = 0.0;
    for k in 0..=m.n_k {
        let (lo, hi) = m.seam_nodes(k);
        assert_eq!(lo.len(), hi.len());
        for (a, b) in lo.iter().zip(&hi) {
            worst = worst.max(vec3::norm(vec3::sub(rotate(m.grid[*a], ang), m.grid[*b])));
        }
    }
    println!("Q6a node rotation mismatch (construction) {worst:.2e}");
    assert!(worst <= tol);
    // (a) every seam_lo mesh point rotated lands on a seam_hi point (bijection)
    let (plo, phi) = (mesh.patch("seam_lo").unwrap(), mesh.patch("seam_hi").unwrap());
    assert_eq!(plo.size, phi.size);
    assert_eq!(plo.periodic_with.as_deref(), Some("seam_hi"));
    let rot = plo.rotation.unwrap();
    assert!((rot.angle - ang).abs() < 1e-15);
    let pts_of = |p: &rcfd_mesh::mesh::Patch| -> Vec<usize> {
        let mut v: Vec<usize> = p.faces().flat_map(|f| mesh.face_points(f).to_vec()).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let (nlo, nhi) = (pts_of(plo), pts_of(phi));
    assert_eq!(nlo.len(), nhi.len());
    let mut taken = vec![false; nhi.len()];
    let mut wn: f64 = 0.0;
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
        assert!(!taken[best], "seam point matched twice");
        taken[best] = true;
        wn = wn.max(bd);
    }
    println!("Q6a seam point bijection mismatch {wn:.2e}");
    assert!(wn <= tol);
    // (b) face centres and (c) area vectors
    let cf = mesh.face_centres();
    let sf = mesh.face_area_vectors();
    let mat = rot.matrix();
    let apply = |v: Vec3| -> Vec3 {
        [
            mat[0][0] * v[0] + mat[0][1] * v[1] + mat[0][2] * v[2],
            mat[1][0] * v[0] + mat[1][1] * v[1] + mat[1][2] * v[2],
            mat[2][0] * v[0] + mat[2][1] * v[1] + mat[2][2] * v[2],
        ]
    };
    let mut used = vec![false; phi.size];
    let (mut wf, mut ws): (f64, f64) = (0.0, 0.0);
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
        assert!(!used[best - phi.start]);
        used[best - phi.start] = true;
        wf = wf.max(bd);
        let sum = vec3::add(sf[best], apply(sf[f]));
        ws = ws.max(vec3::norm(sum) / vec3::norm(sf[f]));
    }
    println!("Q6b face-centre mismatch {wf:.2e}; Q6c area-vector |S_hi + R S_lo| / |S_lo| {ws:.2e}");
    assert!(wf <= tol);
    assert!(ws <= tol);
}

// -------------------------------------------------------------- Q7 exact stacks

fn plane(m: &CGridMesh, id: usize, r: f64) -> (f64, f64) {
    let p = m.grid[id];
    (p[2], r * p[1].atan2(p[0]))
}

fn stack_checks(spec: &CGridSpec, tol_rel: f64) {
    let fan = blade_b();
    let m = build(&fan, spec);
    let (g, nb) = (spec.growth, spec.n_bl);
    let h1 = m.h1;
    if let FirstCell::YPlus { y_plus, speed, nu } = spec.first_cell {
        let mid = fan.station_at(0.5 * (fan.r_hub + fan.r_tip));
        let expect = first_cell_height_from_yplus(y_plus, speed, nu, mid.chord);
        assert!(rel(h1, expect) <= 1e-14, "h1 {h1} vs formula {expect}");
    }
    let exact = |q: usize| -> f64 { (0..=q).map(|p| h1 * g.powi(p as i32)).sum() };
    // radial stacks at hub, tip cap (below and above), shroud
    let r = &m.radii;
    let nk = m.n_k;
    let mut worst: f64 = 0.0;
    for q in 0..nb {
        let want = h1 * g.powi(q as i32);
        for got in [
            r[q + 1] - r[q],
            r[m.n_span - q] - r[m.n_span - q - 1],
            r[m.n_span + q + 1] - r[m.n_span + q],
            r[nk - q] - r[nk - q - 1],
        ] {
            worst = worst.max(rel(got, want));
        }
    }
    println!("Q7 radial stacks: worst relative deviation {worst:.2e} (tolerance {tol_rel:.2e})");
    assert!(worst <= tol_rel);
    // blade wall stacks along the defined normals, every layer below the tip
    let mut worst_d: f64 = 0.0;
    let mut worst_t: f64 = 0.0;
    let nd = m.n_down();
    let n_i = m.n_i();
    for k in 0..=m.n_span {
        for i in nd..=n_i - nd {
            let w = plane(&m, m.c_node(i, 0, k), r[k]);
            let n = m.normals[k][i];
            assert_eq!(m.stack_cells[k][i], nb);
            for q in 1..=nb {
                let p = plane(&m, m.c_node(i, q, k), r[k]);
                let d = (p.0 - w.0, p.1 - w.1);
                let along = d.0 * n[0] + d.1 * n[1];
                let across = d.0 * n[1] - d.1 * n[0];
                worst_d = worst_d.max(rel(along, exact(q - 1)));
                worst_t = worst_t.max(across.abs() / exact(q - 1));
            }
        }
    }
    println!("Q7 wall stacks: distance deviation {worst_d:.2e}, transverse deviation {worst_t:.2e}");
    assert!(worst_d <= tol_rel && worst_t <= tol_rel);
}

// ------------------------------------------------------------- the registered checks

fn q1_gates(tag: &str, spec: CGridSpec, aspect_limit: f64) {
    let t0 = std::time::Instant::now();
    let m = build(&blade_b(), &spec);
    println!("Q1-E {tag}: built in {:.1} s", t0.elapsed().as_secs_f64());
    let q = report(&format!("{tag} (E)"), &m);
    assert!(
        q.max_non_orthogonality <= 70.0,
        "{tag} non-orthogonality {}",
        q.max_non_orthogonality
    );
    assert!(q.max_skewness <= 2.0, "{tag} skewness {}", q.max_skewness);
    assert!(
        q.max_aspect_ratio <= aspect_limit,
        "{tag} aspect {}",
        q.max_aspect_ratio
    );
}

/// Q1-E A' (limits 70 deg, 2.0, 300). KNOWN MISS: aspect 395.1 (non-orthogonality 61.38 and skewness 0.863 are met).
#[test]
#[ignore = "known miss: A' aspect ratio 395.1 > 300 (cells at the irregular seam corner in the end-wall layers), see ADR 0072 addendum"]
fn q1e_quality_gates_a_prime_known_miss() {
    q1_gates("A'", spec_a(), 300.0);
}

/// Q1-E B' (limits 70 deg, 2.0, 3000); a miss was predicted in Amendment 4. KNOWN MISS: 78.17 deg, skewness 7.08, aspect 8532.
#[test]
#[ignore = "known miss (predicted): B' non-orthogonality 78.17, skewness 7.08, aspect 8532, see ADR 0072 addendum"]
fn q1e_quality_gates_b_prime_known_miss() {
    q1_gates("B'", spec_b(), 3000.0);
}

/// The Q1-E gates that are met by A' (non-orthogonality and skewness); the full test is the known miss above.
#[test]
fn q1e_a_prime_gates_that_are_met() {
    let m = build(&blade_b(), &spec_a());
    let q = report("A' (E)", &m);
    assert!(
        q.max_non_orthogonality <= 70.0,
        "non-orthogonality {}",
        q.max_non_orthogonality
    );
    assert!(q.max_skewness <= 2.0, "skewness {}", q.max_skewness);
}

/// Q2-E closure and topology on S and A'.
#[test]
fn q2e_closure_and_topology() {
    for (tag, spec) in [("S", spec_s()), ("A'", spec_a())] {
        let m = build(&blade_b(), &spec);
        println!("Q2-E {tag}: {} cells, {} faces", m.mesh.n_cells(), m.mesh.n_faces());
        closed_cells_and_topology(&m.mesh);
        for p in ["inlet", "outlet", "hub", "shroud", "blade", "seam_lo", "seam_hi"] {
            assert!(m.mesh.patch(p).is_some(), "{p}");
        }
    }
    let empty = build(
        &blade_b(),
        &CGridSpec {
            include_blade: false,
            ..spec_s()
        },
    );
    closed_cells_and_topology(&empty.mesh);
}

/// Q3-E (corrected Q3 of ADR 0068): identical C nodes of the bladed and empty builds, volume identity <= 1e-11.
#[test]
fn q3e_volume_conservation() {
    let fan = blade_b();
    let a = build(&fan, &spec_s());
    let b = build(
        &fan,
        &CGridSpec {
            include_blade: false,
            ..spec_s()
        },
    );
    let mut worst: f64 = 0.0;
    for k in 0..=a.n_k {
        for i in 0..=a.n_i() {
            for j in 0..=a.n_side() {
                worst = worst.max(vec3::norm(vec3::sub(
                    a.grid[a.c_node(i, j, k)],
                    b.grid[b.c_node(i, j, k)],
                )));
            }
        }
    }
    println!("Q3-E C-node coordinates, bladed vs empty build: max difference {worst:.2e}");
    assert!(worst == 0.0);
    let (vp, ve, vs, r) = q3_residual(&spec_s(), true);
    println!("Q3-E: passage {vp:.10e} solid {vs:.10e} envelope {ve:.10e}; residual {r:.2e}");
    assert!(r <= 1e-11);
}

/// Q6-E periodicity on S, A' and a blade of other lean/sweep (1e-12).
#[test]
fn q6e_periodic_pair_matches_under_rotation() {
    for spec in [spec_s(), spec_a()] {
        check_periodic_pair(&build(&blade_b(), &spec), 1e-12);
    }
    let mut fan = blade_b();
    fan.stations[1].lean = 0.02;
    fan.stations[1].sweep = 0.03;
    check_periodic_pair(&build(&fan, &spec_s()), 1e-12);
}

/// Q7-E exact stacks, A' (1e-12).
#[test]
fn q7e_exact_stacks_a() {
    stack_checks(&spec_a(), 1e-12);
}

/// Q7-E exact stacks, B' (4 eps 0.4 / h1).
#[test]
fn q7e_exact_stacks_b() {
    stack_checks(&spec_b(), 4.0 * f64::EPSILON * 0.4 / 7.31e-6);
}

fn q4a_prime(opts: &CGridOptions, tag: &str) -> Vec<f64> {
    let fan = blade_b();
    let analytic = PI / 5.0 * (fan.r_shroud().powi(2) - fan.r_hub.powi(2)) * fan.axial_extent;
    let mut e = Vec::new();
    for (nu, nc, nd, ns, nn) in [(8, 16, 8, 8, 4), (16, 32, 16, 16, 8), (32, 64, 32, 32, 16)] {
        let spec = CGridSpec {
            n_up: nu,
            n_chord: nc,
            n_down: nd,
            n_side: ns,
            n_nose: nn,
            include_blade: false,
            ..spec_s()
        };
        let m = build_cgrid_mesh_with(&fan, &spec, opts)
            .unwrap_or_else(|err| panic!("Q4a' {tag} level ({nu}, {nc}, {nd}, {ns}, {nn}): {err:?}"));
        let er = rel(m.mesh.total_volume(), analytic);
        println!("Q4a' {tag} ({nu}, {nc}, {nd}, {ns}, {nn}): envelope rel error {er:.4e}");
        e.push(er);
    }
    println!(
        "Q4a' {tag}: observed order between the last two {:.3}",
        (e[1] / e[2]).log2()
    );
    e
}

fn q4a_gates(e: &[f64]) {
    assert!(e[1] < e[0] && e[2] < e[1], "not strictly decreasing: {e:?}");
    assert!(e[2] <= 5e-3, "finest {}", e[2]);
    assert!((e[1] / e[2]).log2() >= 1.5, "order {}", (e[1] / e[2]).log2());
}

/// Q4a'-E (corrected Q4a): envelope volume refinement with all in-plane counts scaled, construction E.
#[test]
fn q4a_prime_envelope_refinement_construction_e() {
    q4a_gates(&q4a_prime(&construction_e(), "E"));
}

/// Q4a' with the 33A construction (same levels; a fold is its result). KNOWN MISS: the second level folds.
#[test]
#[ignore = "known miss: the 33A construction folds at the second Q4a' level, see ADR 0072 addendum"]
fn q4a_prime_envelope_refinement_construction_33a_known_miss() {
    q4a_gates(&q4a_prime(&CGridOptions::default(), "33A"));
}

/// Q4b/Q5-E: solid volume and lateral area refinement at the registered levels (n_chord, n_span) = (16, 8), (32, 16), (64, 32).
#[test]
fn q4b_q5_solid_volume_and_lateral_area_refinement_construction_e() {
    let fan = blade_b();
    let vol_ref: f64 = gl(8, fan.r_hub, fan.r_tip)
        .iter()
        .map(|(r, w)| w * fan.section_area(*r))
        .sum();
    let area_ref = lateral_area_quadrature(&fan);
    let (mut e_vol, mut e_area) = (Vec::new(), Vec::new());
    for (nc, nsp) in [(16, 8), (32, 16), (64, 32)] {
        let spec = CGridSpec {
            n_chord: nc,
            n_span: nsp,
            ..spec_s()
        };
        let sm = build(&fan, &spec);
        let env = build(
            &fan,
            &CGridSpec {
                include_blade: false,
                ..spec
            },
        );
        let vs = env.mesh.total_volume() - sm.mesh.total_volume();
        let area = lateral_blade_area(&sm);
        let (ev, ea) = (rel(vs, vol_ref), rel(area, area_ref));
        println!("Q4b/Q5-E (n_chord {nc}, n_span {nsp}): solid volume {vs:.8e} (ref {vol_ref:.8e}) rel {ev:.4e}; lateral area {area:.8e} (ref {area_ref:.8e}) rel {ea:.4e}");
        e_vol.push(ev);
        e_area.push(ea);
    }
    let (ov, oa) = ((e_vol[1] / e_vol[2]).log2(), (e_area[1] / e_area[2]).log2());
    println!("Q4b/Q5-E observed order: volume {ov:.3}, area {oa:.3}");
    assert!(
        e_vol[1] < e_vol[0] && e_vol[2] < e_vol[1],
        "volume not monotone {e_vol:?}"
    );
    assert!(
        e_area[1] < e_area[0] && e_area[2] < e_area[1],
        "area not monotone {e_area:?}"
    );
    assert!(
        e_vol[2] <= 2e-3 && e_area[2] <= 2e-3,
        "finest {} {}",
        e_vol[2],
        e_area[2]
    );
    assert!(ov >= 1.5 && oa >= 1.5);
}

/// The registered scan lists of R-E: (label, spec).
fn scan_list() -> Vec<(String, CGridSpec)> {
    let mut v = Vec::new();
    for fg in [0.2, 0.3, 0.5, 1.0, 1.5, 2.0] {
        v.push((
            format!("front_gap {fg}"),
            CGridSpec {
                front_gap: fg,
                ..spec_s()
            },
        ));
    }
    for nn in [2, 3, 4, 6, 8] {
        v.push((format!("n_nose {nn}"), CGridSpec { n_nose: nn, ..spec_s() }));
    }
    for ns in [6, 8, 10, 16, 32] {
        v.push((format!("n_side {ns}"), CGridSpec { n_side: ns, ..spec_s() }));
    }
    for nc in [12, 16, 24, 32, 64] {
        v.push((
            format!("n_chord {nc}"),
            CGridSpec {
                n_chord: nc,
                ..spec_s()
            },
        ));
    }
    for nc in [16, 24, 32] {
        for nn in [2, 4, 6, 8] {
            for ns in [6, 10, 16, 32] {
                for blade in [true, false] {
                    v.push((
                        format!("grid n_chord {nc} n_nose {nn} n_side {ns} blade {blade}"),
                        CGridSpec {
                            n_chord: nc,
                            n_nose: nn,
                            n_side: ns,
                            include_blade: blade,
                            ..spec_s()
                        },
                    ));
                }
            }
        }
    }
    v
}

/// (built, total, failures) of the scan with the given construction; quality of every built mesh printed.
fn run_scan(opts: &CGridOptions, tag: &str) -> (usize, usize, Vec<String>) {
    let list = scan_list();
    let (mut ok, mut fails) = (0, Vec::new());
    for (label, spec) in &list {
        match build_cgrid_mesh_with(&blade_b(), spec, opts) {
            Ok(m) => {
                let valid = m.mesh.validate().is_ok() && m.mesh.cell_volumes().iter().all(|&v| v > 0.0);
                let q = m.quality();
                println!(
                    "R {tag} {label}: built, valid {valid}; non-orth {:.2} skew {:.3} aspect {:.1}",
                    q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio
                );
                if valid {
                    ok += 1;
                } else {
                    fails.push(label.clone());
                }
            }
            Err(e) => {
                println!(
                    "R {tag} {label}: FAILED {}",
                    e.first().map(|x| x.message.as_str()).unwrap_or("")
                );
                fails.push(label.clone());
            }
        }
    }
    println!("R {tag}: {ok} of {} build and validate", list.len());
    (ok, list.len(), fails)
}

/// R-E robustness scan (gated: every combination builds and validates with construction E). KNOWN MISS: 107 of 117.
#[test]
#[ignore = "known miss: 107 of 117 combinations build with construction E (33A: 38), see ADR 0072 addendum"]
fn r_e_robustness_scan_construction_e_known_miss() {
    let (ok, n, fails) = run_scan(&construction_e(), "E");
    assert!(ok == n, "{} of {n} failed: {fails:?}", n - ok);
}

/// The same scan with the 33A construction (reported, not gated).
#[test]
fn r_e_robustness_scan_construction_33a_reported() {
    let (ok, n, _) = run_scan(&CGridOptions::default(), "33A");
    println!("R 33A reported: {ok} of {n}");
}

/// Not registered (ADR 0072 addendum): where the largest aspect ratio of A' with construction E sits.
#[test]
#[ignore = "diagnostic, not registered: cargo test --release -p rcfd-mesh --test cgrid_e_w34a aspect_location -- --ignored --nocapture"]
fn a_prime_aspect_location() {
    for (tag, spec) in [("A'", spec_a()), ("B'", spec_b())] {
        let m = build(&blade_b(), &spec);
        let met = rcfd_mesh::quality::compute_metrics(&m.mesh);
        let mut idx: Vec<usize> = (0..m.mesh.n_cells()).collect();
        idx.sort_by(|a, b| met.aspect_ratio[*b].partial_cmp(&met.aspect_ratio[*a]).unwrap());
        for &c in idx.iter().take(5) {
            let p = m.mesh.cell_centres()[c];
            let r = p[0].hypot(p[1]);
            println!(
                "{tag} aspect {:.1} at cell {c} tag {:?} (z {:.4}, s {:.4}, r {:.4})",
                met.aspect_ratio[c],
                m.cell_tags[c],
                p[2],
                r * p[1].atan2(p[0]),
                r
            );
        }
    }
}

/// Not registered (ADR 0072 addendum): the step of the construction chain that built each registered mesh.
#[test]
#[ignore = "record, not registered: cargo test --release -p rcfd-mesh --test cgrid_e_w34a construction_steps -- --ignored --nocapture"]
fn construction_steps_of_the_registered_meshes() {
    let fan = blade_b();
    let mut list: Vec<(String, CGridSpec)> =
        vec![("S".into(), spec_s()), ("A'".into(), spec_a()), ("B'".into(), spec_b())];
    for (nu, nc, nd, ns, nn) in [(8, 16, 8, 8, 4), (16, 32, 16, 16, 8), (32, 64, 32, 32, 16)] {
        list.push((
            format!("Q4a' ({nu}, {nc}, {nd}, {ns}, {nn})"),
            CGridSpec {
                n_up: nu,
                n_chord: nc,
                n_down: nd,
                n_side: ns,
                n_nose: nn,
                include_blade: false,
                ..spec_s()
            },
        ));
    }
    for (nc, nsp) in [(16, 8), (32, 16), (64, 32)] {
        list.push((
            format!("Q4b ({nc}, {nsp})"),
            CGridSpec {
                n_chord: nc,
                n_span: nsp,
                ..spec_s()
            },
        ));
    }
    for (tag, spec) in list {
        let m = build(&fan, &spec);
        let q = m.quality();
        println!(
            "step {} for {tag}: non-orth {:.4} skew {:.4} aspect {:.2}",
            m.construction_step, q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio
        );
    }
}
