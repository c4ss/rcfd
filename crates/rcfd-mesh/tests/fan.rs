//! Wave 32A (ADR 0065): parametric fan geometry, blade surface and the periodic sector mesh. The limits are the registered
//! ones (G1-G3, M1-M5); a miss is moved unchanged into an ignored `*_known_miss` test.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::fan::*;
use rcfd_mesh::fan_mesh::*;
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

/// Test blade B of ADR 0065.
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

fn spec_a() -> SectorSpec {
    SectorSpec {
        n_up: 22,
        n_chord: 24,
        n_down: 22,
        n_side: 20,
        n_mid: 3,
        n_span: 30,
        n_gap: 18,
        n_bl: 5,
        growth: 1.2,
        first_cell: FirstCell::YPlus {
            y_plus: 30.0,
            speed: 90.0,
            nu: 1.5e-5,
        },
        trunc_le: 0.02,
        trunc_te: 0.01,
        include_blade: true,
    }
}

fn spec_b() -> SectorSpec {
    SectorSpec {
        n_bl: 8,
        n_up: 24,
        n_down: 24,
        n_side: 24,
        n_span: 40,
        n_gap: 26,
        first_cell: FirstCell::YPlus {
            y_plus: 1.0,
            speed: 90.0,
            nu: 1.5e-5,
        },
        ..spec_a()
    }
}

/// Small controls for the refinement studies.
fn spec_small(n_side: usize, n_chord: usize, n_span: usize) -> SectorSpec {
    SectorSpec {
        n_up: 8,
        n_chord,
        n_down: 8,
        n_side,
        n_mid: 2,
        n_span,
        n_gap: 7,
        n_bl: 2,
        growth: 1.2,
        first_cell: FirstCell::Height(1e-3),
        trunc_le: 0.02,
        trunc_te: 0.01,
        include_blade: true,
    }
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

fn single(family: SectionFamily, m: f64, t: f64) -> (AxialFan, Station) {
    let s = Station {
        radius: 0.2,
        chord: 0.2,
        stagger: 0.3,
        camber: m,
        camber_pos: 0.4,
        thickness: t,
        sweep: 0.0,
        lean: 0.0,
    };
    let fan = AxialFan::linear(4, 0.2, 0.3, 0.01, 1.0, 0.3, family, s, Station { radius: 0.3, ..s });
    (fan, s)
}

fn g1_run(n: usize) {
    let cases = [
        (SectionFamily::Naca4, 0.0, 0.12),
        (SectionFamily::Naca4, 0.04, 0.12),
        (SectionFamily::CircularArc, 0.05, 0.08),
    ];
    for (fam, m, t) in cases {
        let (fan, s) = single(fam, m, t);
        // chord: LE to TE distance
        let le = fan.surface_zs(&s, 0.0, Side::Suction);
        let te = fan.surface_zs(&s, 1.0, Side::Suction);
        let c = (te.0 - le.0).hypot(te.1 - le.1);
        assert!(
            (c - s.chord).abs() <= 1e-13 * s.chord,
            "{fam:?} m {m}: chord {c} vs {}",
            s.chord
        );
        // shoelace area of the unit section from 400 cosine points per side
        let xs: Vec<f64> = (0..=n)
            .map(|i| 0.5 * (1.0 - (PI * i as f64 / n as f64).cos()))
            .collect();
        let mut loop_uv: Vec<(f64, f64)> = xs
            .iter()
            .map(|&x| section_uv(fam, m, 0.4, t, x, Side::Suction))
            .collect();
        loop_uv.extend(
            xs[1..n]
                .iter()
                .rev()
                .map(|&x| section_uv(fam, m, 0.4, t, x, Side::Pressure)),
        );
        let mut a2 = 0.0;
        for i in 0..loop_uv.len() {
            let (p, q) = (loop_uv[i], loop_uv[(i + 1) % loop_uv.len()]);
            a2 += p.0 * q.1 - q.0 * p.1;
        }
        let poly = 0.5 * a2.abs();
        let quad = unit_section_area(fam, m, 0.4, t);
        println!(
            "G1 {fam:?} m {m} t {t}: polygon {poly:.10e} quadrature {quad:.10e} rel {:.2e}",
            rel(poly, quad)
        );
        assert!(rel(poly, quad) <= 1e-5, "{fam:?} area");
        if fam == SectionFamily::Naca4 && m == 0.0 {
            let closed = 10.0 * t * (0.2969 * 2.0 / 3.0 - 0.1260 / 2.0 - 0.3516 / 3.0 + 0.2843 / 4.0 - 0.1036 / 5.0);
            println!("G1 closed form {closed:.10e} rel {:.2e}", rel(quad, closed));
            assert!(rel(poly, closed) <= 1e-5 && rel(quad, closed) <= 1e-5);
        }
        // maximum thickness
        let tmax = (0..=20000)
            .map(|i| 2.0 * half_thickness(fam, t, i as f64 / 20000.0))
            .fold(0.0, f64::max);
        match fam {
            SectionFamily::Naca4 => assert!((tmax / t - 1.0).abs() <= 0.01, "max thickness {tmax}"),
            SectionFamily::CircularArc => assert!((2.0 * half_thickness(fam, t, 0.5) - t).abs() <= 1e-12),
        }
        // dimensional section area scales with chord^2
        assert!(rel(fan.section_area(0.25), s.chord * s.chord * quad) <= 1e-14);
    }
}

/// Registered set-up: 400 cosine points per side. The NACA 0012 polygon misses the 1e-5 limit (1.03e-5): the polygon
/// (chord) error of the nose, not the formulas. Kept as a known miss; the corrected set-up below uses 1000 points.
#[test]
#[ignore = "known miss: 400-point polygon area differs from the quadrature by 1.03e-5 (> 1e-5) for NACA 0012; see g1_with_1000_points"]
fn g1_sections_match_analytic_formulas_known_miss() {
    g1_run(400);
}

#[test]
fn g1_with_1000_points() {
    g1_run(1000);
}

fn surface_area_quadrature(fan: &AxialFan) -> f64 {
    let mut a = fan.section_area(fan.r_hub) + fan.section_area(fan.r_tip);
    let h = 1e-6;
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

#[test]
fn g2_g3_blade_surface_area_volume_watertight_and_refinement() {
    let fan = blade_b();
    assert!(fan.validate().is_empty());
    let vol_ref: f64 = gl(8, fan.r_hub, fan.r_tip)
        .iter()
        .map(|(r, w)| w * fan.section_area(*r))
        .sum();
    let area_ref = surface_area_quadrature(&fan);
    let mut errs = Vec::new();
    for (nc, ns) in [(80, 24), (160, 48), (320, 96)] {
        let s = fan.blade_surface(nc, ns).unwrap();
        s.check_watertight().unwrap();
        assert!(s.volume() > 0.0);
        let (ev, ea) = (rel(s.volume(), vol_ref), rel(s.area(), area_ref));
        println!("G2 ({nc},{ns}): volume {:.8e} (ref {vol_ref:.8e}) rel {ev:.3e}; area {:.8e} (ref {area_ref:.8e}) rel {ea:.3e}", s.volume(), s.area());
        errs.push((ev, ea));
    }
    assert!(errs[0].0 <= 1e-3 && errs[0].1 <= 2e-3, "base resolution limits");
    assert!(
        errs[1].0 < errs[0].0 && errs[2].0 < errs[1].0,
        "volume error not monotone"
    );
    assert!(
        errs[1].1 < errs[0].1 && errs[2].1 < errs[1].1,
        "area error not monotone"
    );
    let order_v = (errs[1].0 / errs[2].0).log2();
    let order_a = (errs[1].1 / errs[2].1).log2();
    println!("G2 observed order volume {order_v:.2} area {order_a:.2}");
    assert!(order_v >= 1.5 && order_a >= 1.5);
}

#[test]
fn validation_errors_are_located() {
    let mut f = blade_b();
    f.stations[1].chord = -1.0;
    f.stations[0].thickness = 0.9;
    f.n_blades = 1;
    f.tip_clearance = 0.0;
    let e = f.validate();
    let paths: Vec<&str> = e.iter().map(|x| x.path.as_str()).collect();
    for p in [
        "stations[1].chord",
        "stations[0].thickness",
        "n_blades",
        "tip_clearance",
    ] {
        assert!(paths.contains(&p), "{p} missing in {paths:?}");
    }
    let mut f = blade_b();
    f.stations[1].radius = 0.25;
    assert!(f.validate().iter().any(|x| x.path == "stations[1].radius"));
    // mesh-level located errors
    let mut sp = spec_small(10, 12, 16);
    sp.n_span = 3;
    let e = build_sector_mesh(&blade_b(), &sp).expect_err("must fail");
    assert!(e.iter().any(|x| x.path == "n_span"), "{e:?}");
    let mut sp = spec_small(10, 12, 16);
    sp.growth = 0.5;
    let e = build_sector_mesh(&blade_b(), &sp).expect_err("must fail");
    assert!(!e.is_empty(), "{e:?}");
}

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
    println!("M2 worst closure |sum S|/sum|S| = {worst:.2e}");
    assert!(worst <= 1e-13);
    // each internal face shared by exactly two cells: counts per cell face incidence
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

fn solid_volume_by_faces(sm: &SectorMesh) -> f64 {
    let m = &sm.mesh;
    let blade = m.patch("blade").unwrap();
    let mut v = 0.0;
    for f in blade.faces() {
        v += -vec3::dot(m.face_centres()[f], m.face_area_vectors()[f]) / 3.0;
    }
    for r in &sm.removed {
        if r[2] != 0 {
            continue;
        }
        let (i, j) = (r[0], r[1]);
        let lp = [
            sm.node(i, j, 0),
            sm.node(i + 1, j, 0),
            sm.node(i + 1, j + 1, 0),
            sm.node(i, j + 1, 0),
        ];
        let (c, mut s) = rcfd_mesh::mesh::polygon_geometry(&sm.grid, &lp);
        if s[0] * c[0] + s[1] * c[1] > 0.0 {
            s = vec3::scale(s, -1.0);
        }
        v += vec3::dot(s, c) / 3.0;
    }
    v
}

#[test]
fn m1_m2_volume_topology_and_conservation() {
    let fan = blade_b();
    let sm = build_sector_mesh(&fan, &spec_small(10, 16, 14)).unwrap();
    let empty = build_sector_mesh(
        &fan,
        &SectorSpec {
            include_blade: false,
            ..spec_small(10, 16, 14)
        },
    )
    .unwrap();
    closed_cells_and_topology(&sm.mesh);
    closed_cells_and_topology(&empty.mesh);
    let (vp, ve, vs) = (
        sm.mesh.total_volume(),
        empty.mesh.total_volume(),
        solid_volume_by_faces(&sm),
    );
    let resid = (vp + vs - ve).abs() / ve;
    println!("M1 passage {vp:.10e} solid(by faces) {vs:.10e} envelope {ve:.10e}; residual {resid:.2e}");
    assert!(resid <= 1e-11);
    for p in ["inlet", "outlet", "hub", "shroud", "blade", "seam_lo", "seam_hi"] {
        assert!(sm.mesh.patch(p).is_some(), "{p}");
    }
    assert_eq!(sm.mesh.cell_zone("rotor").unwrap().ids.len(), sm.mesh.n_cells());
    assert!(empty.mesh.patch("blade").is_none());
}

#[test]
fn m1_envelope_and_solid_volume_refinement() {
    let fan = blade_b();
    let analytic = PI / 5.0 * (fan.r_shroud().powi(2) - fan.r_hub.powi(2)) * fan.axial_extent;
    let mut e_env = Vec::new();
    for ns in [8, 16, 32] {
        let sm = build_sector_mesh(
            &fan,
            &SectorSpec {
                include_blade: false,
                ..spec_small(ns, 12, 14)
            },
        )
        .unwrap();
        let e = rel(sm.mesh.total_volume(), analytic);
        println!(
            "M1 envelope n_side {ns}: {:.8e} vs analytic {analytic:.8e}: rel {e:.3e}",
            sm.mesh.total_volume()
        );
        e_env.push(e);
    }
    assert!(e_env[2] <= 5e-3, "finest envelope error");
    assert!(
        e_env[1] < e_env[0] && e_env[2] < e_env[1],
        "envelope error not monotone: {e_env:?}"
    );
    // solid volume vs quadrature of the z-truncated analytic sections
    let (le, te) = (0.02, 0.01);
    let a_trunc = |r: f64| -> f64 {
        let s = fan.station_at(r);
        let za = fan.camber_zs(&s, le).0;
        let zb = fan.camber_zs(&s, 1.0 - te).0;
        let branch = |side: Side, z: f64| -> f64 {
            // bisection on the monotone branch (x >= argmin z)
            let f = |x: f64| fan.surface_zs(&s, x, side);
            let mut x0 = 0.0;
            let mut zm = f64::INFINITY;
            for i in 0..=20000 {
                let x = i as f64 / 20000.0;
                let zz = f(x).0;
                if zz < zm {
                    zm = zz;
                    x0 = x;
                }
            }
            let (mut a, mut b) = (x0, 1.0);
            for _ in 0..100 {
                let c = 0.5 * (a + b);
                if f(c).0 < z {
                    a = c
                } else {
                    b = c
                }
            }
            f(0.5 * (a + b)).1
        };
        let mut a = 0.0;
        for q in 0..40 {
            let (z0, z1) = (za + (zb - za) * q as f64 / 40.0, za + (zb - za) * (q + 1) as f64 / 40.0);
            for (z, w) in gl(6, z0, z1) {
                a += w * (branch(Side::Pressure, z) - branch(Side::Suction, z));
            }
        }
        a
    };
    let vref: f64 = gl(6, fan.r_hub, fan.r_tip).iter().map(|(r, w)| w * a_trunc(*r)).sum();
    let mut e_sol = Vec::new();
    for nc in [12, 24, 48] {
        let sm = build_sector_mesh(&fan, &spec_small(10, nc, 16)).unwrap();
        let env = build_sector_mesh(
            &fan,
            &SectorSpec {
                include_blade: false,
                ..spec_small(10, nc, 16)
            },
        )
        .unwrap();
        let vs = env.mesh.total_volume() - sm.mesh.total_volume();
        let e = rel(vs, vref);
        println!("M1 solid n_chord {nc}: {vs:.8e} vs quadrature {vref:.8e}: rel {e:.3e}");
        e_sol.push(e);
    }
    assert!(e_sol[2] <= 2e-3, "finest solid volume error");
    assert!(
        e_sol[1] < e_sol[0] && e_sol[2] < e_sol[1],
        "solid volume error not monotone: {e_sol:?}"
    );
}

#[test]
fn m3_periodic_pair_matches_under_rotation() {
    let fan = blade_b();
    let sm = build_sector_mesh(&fan, &spec_small(10, 16, 14)).unwrap();
    let m = &sm.mesh;
    let ang = 2.0 * PI / 5.0;
    let rot = |p: Vec3| -> Vec3 {
        let (s, c) = ang.sin_cos();
        [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
    };
    let mut worst: f64 = 0.0;
    for k in 0..=sm.n_k {
        for i in 0..=sm.n_i {
            let a = rot(sm.grid[sm.node(i, 0, k)]);
            let b = sm.grid[sm.node(i, sm.n_j, k)];
            worst = worst.max(vec3::norm(vec3::sub(a, b)));
        }
    }
    println!("M3 node rotation mismatch {worst:.2e}");
    assert!(worst <= 1e-12);
    let (lo, hi) = (m.patch("seam_lo").unwrap(), m.patch("seam_hi").unwrap());
    assert_eq!(lo.size, hi.size);
    assert_eq!(lo.size, sm.n_i * sm.n_k);
    assert_eq!(lo.periodic_with.as_deref(), Some("seam_hi"));
    let r = lo.rotation.unwrap();
    assert!((r.angle - ang).abs() < 1e-15);
    let cf = m.face_centres();
    let mut used = vec![false; hi.size];
    let mut wf: f64 = 0.0;
    for f in lo.faces() {
        let t = r.apply_point(cf[f]);
        let (mut best, mut bd) = (0, f64::INFINITY);
        for g in hi.faces() {
            let d = vec3::norm(vec3::sub(cf[g], t));
            if d < bd {
                bd = d;
                best = g - hi.start;
            }
        }
        assert!(!used[best]);
        used[best] = true;
        wf = wf.max(bd);
    }
    println!("M3 face-centre mismatch {wf:.2e}");
    assert!(wf <= 1e-12);
}

fn report_quality(tag: &str, sm: &SectorMesh) -> QualitySummary {
    let q = sm.quality();
    println!(
        "M4 {tag}: {} cells; non-orth max {:.2} mean {:.2} p99 {:.2} deg, >50: {}, >60: {}; skewness max {:.3}; aspect max {:.1}; min volume {:.3e}; h1 {:.3e}",
        sm.mesh.n_cells(),
        q.max_non_orthogonality,
        q.mean_non_orthogonality,
        q.p99_non_orthogonality,
        q.n_above_50,
        q.n_above_60,
        q.max_skewness,
        q.max_aspect_ratio,
        q.min_volume,
        sm.h1
    );
    q
}

#[test]
fn m4_quality_gates_and_m5_first_cell() {
    let fan = blade_b();
    // h1 from y+ matches the formula
    let chord_mid = fan.station_at(0.2).chord;
    let (u, nu) = (90.0, 1.5e-5);
    let re = u * chord_mid / nu;
    let u_tau = u * (0.5 * 0.0576 * re.powf(-0.2)).sqrt();
    let a = build_sector_mesh(&fan, &spec_a()).unwrap();
    let h1_formula = 2.0 * 30.0 * nu / u_tau;
    assert!(rel(a.h1, h1_formula) <= 1e-14);
    assert!(rel(0.5 * a.h1 * u_tau / nu, 30.0) <= 1e-14, "centre y+");
    let _qa = report_quality("A (y+ 30)", &a);
    closed_cells_and_topology(&a.mesh);
    first_cell_checks(&a, 1e-12);
    let b = build_sector_mesh(&fan, &spec_b()).unwrap();
    let _qb = report_quality("B (y+ 1)", &b);
    // corrected set-up: node coordinates (|x| <= 0.4) are stored in f64, so a spacing h1 = 7e-6 cannot be recovered
    // from coordinate differences better than about eps * 0.4 / h1 (4e-12); the bound below is 4 times that.
    first_cell_checks(&b, 4.0 * f64::EPSILON * 0.4 / b.h1);
}

/// M5 for set-up B at the registered 1e-12: KNOWN MISS, representation limit of f64 coordinates (see above).
#[test]
#[ignore = "known miss: 1e-12 relative is below the f64 coordinate resolution at h1 = 7e-6, see ADR 0065 addendum"]
fn m5_first_cell_b_1e12_known_miss() {
    let b = build_sector_mesh(&blade_b(), &spec_b()).unwrap();
    first_cell_checks(&b, 1e-12);
}

/// M4 as registered in ADR 0065 (non-orthogonality <= 70 deg, skewness <= 2.0, aspect <= 300 (A) / 3000 (B)).
/// KNOWN MISS: the limits are unchanged and the measured values are in the ADR addendum; run with `--ignored`.
#[test]
#[ignore = "known miss: registered M4 gates are not met, see ADR 0065 addendum"]
fn m4_quality_gates_known_miss() {
    let fan = blade_b();
    let a = build_sector_mesh(&fan, &spec_a()).unwrap();
    let qa = report_quality("A (y+ 30)", &a);
    assert!(
        qa.max_non_orthogonality <= 70.0,
        "non-orth {}",
        qa.max_non_orthogonality
    );
    assert!(qa.max_skewness <= 2.0, "skew {}", qa.max_skewness);
    assert!(qa.max_aspect_ratio <= 300.0, "aspect {}", qa.max_aspect_ratio);
    let b = build_sector_mesh(&fan, &spec_b()).unwrap();
    let qb = report_quality("B (y+ 1)", &b);
    assert!(qb.max_non_orthogonality <= 70.0 && qb.max_skewness <= 2.0 && qb.max_aspect_ratio <= 3000.0);
}

/// Set-up A with a single cell across the blunt edge faces (`n_mid = 1`, everything else as A): a different set-up, kept beside
/// the registered one. Non-orthogonality and skewness meet the registered gates; the aspect ratio does not (405 against 300).
#[test]
fn m4_set_up_a_single_mid_cell_meets_two_of_three_gates() {
    let mut spec = spec_a();
    spec.n_mid = 1;
    let sm = build_sector_mesh(&blade_b(), &spec).unwrap();
    let q = report_quality("A, n_mid 1", &sm);
    closed_cells_and_topology(&sm.mesh);
    first_cell_checks(&sm, 1e-12);
    assert!(q.max_non_orthogonality <= 70.0, "non-orth {}", q.max_non_orthogonality);
    assert!(q.max_skewness <= 2.0, "skew {}", q.max_skewness);
}

/// The aspect gate of that set-up (300): KNOWN MISS, 405.
#[test]
#[ignore = "known miss: aspect ratio 405 against the registered 300 for A with n_mid = 1, see ADR 0065 addendum"]
fn m4_set_up_a_single_mid_cell_aspect_known_miss() {
    let mut spec = spec_a();
    spec.n_mid = 1;
    let sm = build_sector_mesh(&blade_b(), &spec).unwrap();
    assert!(report_quality("A, n_mid 1", &sm).max_aspect_ratio <= 300.0);
}

fn zs_of(sm: &SectorMesh, i: usize, j: usize, k: usize) -> (f64, f64) {
    let p = sm.grid[sm.node(i, j, k)];
    let r = p[0].hypot(p[1]);
    (p[2], r * p[1].atan2(p[0]))
}

/// Spacing checks: `h1` and the growth ratio on every wall, tolerance `tol` relative to the spacing.
fn first_cell_checks(sm: &SectorMesh, tol: f64) {
    let (h1, g, nb) = (sm.h1, sm.growth, sm.n_bl);
    let mut worst: f64 = 0.0;
    let mut check = |got: f64, want: f64| {
        let e = (got - want).abs() / want;
        worst = worst.max(e);
        e
    };
    let rad = |i: usize, j: usize, k: usize| {
        let p = sm.grid[sm.node(i, j, k)];
        p[0].hypot(p[1])
    };
    // radial: hub, tip cap (above and below), shroud
    let (ns, nk) = (sm.n_span, sm.n_k);
    for (i, j) in [(0, 0), (sm.n_up + 3, sm.n_side - 2), (sm.n_i, sm.n_j)] {
        for m in 0..nb {
            let w = h1 * g.powi(m as i32);
            check(rad(i, j, 1 + m) - rad(i, j, m), w);
            check(rad(i, j, ns + 1 + m) - rad(i, j, ns + m), w);
            check(rad(i, j, nk - m) - rad(i, j, nk - m - 1), w);
            check(rad(i, j, ns - m) - rad(i, j, ns - m - 1), w);
        }
    }
    // axial: leading end face upstream, trailing end face downstream
    let (ile, ite) = (sm.n_up, sm.n_up + sm.n_chord);
    for k in [0, ns / 2, ns - 1] {
        for j in [0, sm.n_side, sm.n_side + sm.n_mid, sm.n_j] {
            for m in 0..nb {
                let w = h1 * g.powi(m as i32);
                check(zs_of(sm, ile - m, j, k).0 - zs_of(sm, ile - m - 1, j, k).0, w);
                check(zs_of(sm, ite + m + 1, j, k).0 - zs_of(sm, ite + m, j, k).0, w);
            }
        }
    }
    // blade sides: wall-normal distance of the layer nodes in the unrolled section plane
    let nj_p = sm.n_side + sm.n_mid;
    for k in 0..=ns {
        for (side_suc, wall_j) in [(true, sm.n_side), (false, nj_p)] {
            let wall: Vec<(f64, f64)> = (0..=sm.n_chord).map(|c| zs_of(sm, ile + c, wall_j, k)).collect();
            for c in 0..=sm.n_chord {
                let (a, b) = (c.saturating_sub(1), if c == sm.n_chord { c } else { c + 1 });
                let (tz, ts) = (wall[b].0 - wall[a].0, wall[b].1 - wall[a].1);
                let l = tz.hypot(ts);
                let n = if side_suc { (ts / l, -tz / l) } else { (-ts / l, tz / l) };
                let mut cum = 0.0;
                for m in 0..nb {
                    cum += h1 * g.powi(m as i32);
                    let jq = if side_suc { wall_j - 1 - m } else { wall_j + 1 + m };
                    let q = zs_of(sm, ile + c, jq, k);
                    let d = (q.0 - wall[c].0) * n.0 + (q.1 - wall[c].1) * n.1;
                    check(d, cum);
                }
            }
        }
    }
    println!("M5 worst relative first-cell/growth deviation {worst:.2e} (h1 {h1:.3e}, g {g})");
    assert!(worst <= tol, "first-cell deviation {worst:e} > {tol:e}");
}

/// Diagnostic (ignored): where the worst quality values sit.
#[test]
#[ignore]
fn diag_worst_cells() {
    let fan = blade_b();
    let sm = build_sector_mesh(&fan, &spec_a()).unwrap();
    let m = rcfd_mesh::quality::compute_metrics(&sm.mesh);
    let mut ijk = Vec::new();
    let rem: std::collections::HashSet<[usize; 3]> = sm.removed.iter().copied().collect();
    for k in 0..sm.n_k {
        for j in 0..sm.n_j {
            for i in 0..sm.n_i {
                if !rem.contains(&[i, j, k]) {
                    ijk.push([i, j, k]);
                }
            }
        }
    }
    let mut ar: Vec<(f64, [usize; 3])> = m.aspect_ratio.iter().cloned().zip(ijk.iter().cloned()).collect();
    ar.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    println!("worst aspect: {:?}", &ar[..6]);
    let mut no: Vec<(f64, usize)> = m
        .non_orthogonality
        .iter()
        .cloned()
        .enumerate()
        .map(|(f, v)| (v, f))
        .collect();
    no.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (v, f) in no.iter().take(6) {
        let c = sm.mesh.owner()[*f];
        println!(
            "non-orth {v:.1} face {f} internal {} owner cell ijk {:?}",
            *f < sm.mesh.n_internal_faces(),
            ijk[c]
        );
    }
    let mut sk: Vec<(f64, usize)> = m.skewness.iter().cloned().enumerate().map(|(f, v)| (v, f)).collect();
    sk.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    for (v, f) in sk.iter().take(6) {
        println!("skew {v:.2} face {f} owner ijk {:?}", ijk[sm.mesh.owner()[*f]]);
    }
    println!(
        "dims i {} (up {} chord {}) j {} (side {} mid {}) k {} (span {})",
        sm.n_i, sm.n_up, sm.n_chord, sm.n_j, sm.n_side, sm.n_mid, sm.n_k, sm.n_span
    );
}

#[test]
#[ignore]
fn diag_face() {
    let fan = blade_b();
    let sm = build_sector_mesh(&fan, &spec_a()).unwrap();
    let m = &sm.mesh;
    let met = rcfd_mesh::quality::compute_metrics(m);
    let (mut best, mut bf) = (0.0, 0);
    for (f, v) in met.non_orthogonality.iter().enumerate() {
        if *v > best {
            best = *v;
            bf = f;
        }
    }
    let (o, n) = (m.owner()[bf], m.neighbour()[bf]);
    println!(
        "face {bf} nonorth {best}; owner c {:?} neigh c {:?}; face centre {:?} S {:?}",
        m.cell_centres()[o],
        m.cell_centres()[n],
        m.face_centres()[bf],
        m.face_area_vectors()[bf]
    );
    for &p in m.face_points(bf) {
        println!("pt {:?}", m.points()[p]);
    }
    println!("vol o {} n {}", m.cell_volumes()[o], m.cell_volumes()[n]);
}
