//! Wave 46A (ADR 0118): computer-fan parameter schema, constraint checker and manufacturing perturbations.

use rcfd_mesh::fan::{gauss_legendre, AxialFan, Side};
use rcfd_mesh::fan_cgrid::*;
use rcfd_mesh::fan_design::*;
use rcfd_mesh::fan_mesh::*;

fn print_violations(tag: &str, f: &ComputerFan, lim: &Limits) {
    let v = f.check(lim);
    println!("{tag}: {} violations", v.len());
    for x in &v {
        println!(
            "  {:?} {} blade {:?} r {:?} value {:.4e} limit {:.4e}: {}",
            x.code, x.path, x.blade, x.radius, x.value, x.limit, x.message
        );
    }
}

#[test]
#[ignore = "rule-d smoke of ADR 0118, run once before registration"]
fn smoke_w46a() {
    let f = ComputerFan::default();
    let lim = Limits::default();
    print_violations("default", &f, &lim);
    // margins of the default: rerun with tightened limits to see where each check would fire
    let tight = Limits {
        min_wall: 1.5e-3,
        min_passage_gap: 1e-2,
        min_draft_deg: 30.0,
        min_axial_gap: 4e-3,
        ..lim
    };
    print_violations("default with tight limits (margins)", &f, &tight);
    let af = f.axial_fan(0, 0.03, 0.04);
    println!("axial fan validate: {:?}", af.validate());
    let ss = SectorSpec {
        n_up: 8,
        n_chord: 16,
        n_down: 8,
        n_side: 10,
        n_mid: 2,
        n_span: 10,
        n_gap: 7,
        n_bl: 2,
        growth: 1.2,
        first_cell: FirstCell::Height(5e-5),
        trunc_le: 0.02,
        trunc_te: 0.01,
        include_blade: true,
    };
    match build_sector_mesh(&af, &ss) {
        Ok(m) => println!("h_slit: {} cells, quality {:?}", m.mesh.n_cells(), m.quality()),
        Err(e) => println!("h_slit failed: {e:?}"),
    }
    let cs = CGridSpec {
        n_up: 8,
        n_chord: 24,
        n_down: 8,
        n_side: 10,
        n_nose: 6,
        n_core: 2,
        n_span: 10,
        n_gap: 7,
        n_bl: 2,
        growth: 1.2,
        first_cell: FirstCell::Height(5e-5),
        front_gap: 0.3,
        include_blade: true,
    };
    for (name, o) in [
        ("pursuit", CGridOptions::default()),
        ("camber_seam_h", CGridOptions::camber_seam_h_construction()),
    ] {
        match build_cgrid_mesh_with(&af, &cs, &o) {
            Ok(m) => println!("c_grid {name}: {} cells, quality {:?}", m.mesh.n_cells(), m.quality()),
            Err(e) => println!("c_grid {name} failed: {e:?}"),
        }
    }
    // mass properties
    let (m0, c0) = f.blade_mass(0).unwrap();
    println!("blade mass {m0:.6e} kg, centroid {c0:?}");
    let u = f.static_unbalance().unwrap();
    println!("nominal U {u:?} kg m");
    for (nc, ns) in [(48, 24), (96, 48), (192, 96)] {
        let s = f.axial_fan(0, 0.0, 0.0).blade_surface(nc, ns).unwrap();
        println!("({nc},{ns}) volume {:.10e} centroid {:?}", s.volume(), s.centroid());
    }
    // timing of an ensemble in this build
    let tol = Tolerances {
        stagger_common_deg: Dist::TruncatedNormal {
            mean: 0.0,
            sd: 0.5,
            lo: -1.0,
            hi: 1.0,
        },
        stagger_blade_deg: Dist::Uniform { lo: -0.5, hi: 0.5 },
        mass_rel: Dist::TruncatedNormal {
            mean: 0.0,
            sd: 0.01,
            lo: -0.02,
            hi: 0.03,
        },
        ..Tolerances::default()
    };
    let t0 = std::time::Instant::now();
    let e = f.ensemble(&tol, 2000, 46).unwrap();
    println!("ensemble 2000 x 7: {:?}", t0.elapsed());
    let t0 = std::time::Instant::now();
    let bad = e.iter().take(5).filter(|x| !x.check(&lim).is_empty()).count();
    println!("check of 5 specimens: {:?}, {bad} rejected", t0.elapsed());
    let t0 = std::time::Instant::now();
    let _ = e[0].static_unbalance().unwrap();
    println!("unbalance of one specimen: {:?}", t0.elapsed());
    // candidate bad variants
    let mut v = f.clone();
    v.stations[2].thickness = 0.015;
    print_violations("thin tip", &v, &lim);
    let mut v = f.clone();
    v.tip_gap = 2e-4;
    print_violations("small gap", &v, &lim);
    let mut v = f.clone();
    v.tip_gap = 3.5e-3;
    v.frame.side = 0.13;
    print_violations("large gap", &v, &lim);
    let mut v = f.clone();
    v.deviation = Some(Deviation {
        eccentricity: 1.2e-3,
        ..Deviation::default()
    });
    print_violations("eccentric rub", &v, &lim);
    let mut v = f.clone();
    v.r_hub = 0.042;
    print_violations("hub ratio", &v, &lim);
    let mut v = f.clone();
    v.stations[2].chord = 0.012;
    v.stations[2].thickness = 0.1;
    print_violations("low tip solidity", &v, &lim);
    let mut v = f.clone();
    v.stations[0].chord = 0.032;
    print_violations("hub overlap", &v, &lim);
    print_violations(
        "hub overlap, no mould",
        &v,
        &Limits {
            axial_mould: false,
            ..lim
        },
    );
    let mut v = f.clone();
    v.n_blades = 11;
    v.stations[0].chord = 0.03;
    v.stations[0].stagger_deg = 60.0;
    print_violations(
        "collision",
        &v,
        &Limits {
            axial_mould: false,
            ..lim
        },
    );
    let mut v = f.clone();
    v.stations[0].stagger_deg = 3.0;
    print_violations("low root stagger", &v, &lim);
    let mut v = f.clone();
    v.stations[0].stagger_deg = 0.0;
    v.stations[0].camber = 0.12;
    print_violations("zero stagger high camber root", &v, &lim);
    let mut v = f.clone();
    let mut d = Deviation {
        blades: vec![BladeDeviation::default(); 7],
        ..Deviation::default()
    };
    d.blades[3].thickness_rel = -0.45;
    v.deviation = Some(d);
    print_violations("thin blade 3", &v, &lim);
    let mut v = f.clone();
    v.frame.side = 0.119;
    print_violations("frame wall", &v, &lim);
    let mut v = f.clone();
    v.z_le_root = 0.006;
    print_violations("axial fit", &v, &lim);
    let mut v = f.clone();
    v.hub.z_start = 0.004;
    v.hub.length = 0.02;
    print_violations("root off hub", &v, &lim);
    let mut v = f.clone();
    v.stations[0].stagger_deg = 12.0;
    v.frame.depth = 0.03;
    v.hub.length = 0.028;
    print_violations("draft variant", &v, &lim);
    for g in [8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0] {
        let mut v = f.clone();
        v.stations[0].stagger_deg = g;
        let codes: Vec<_> = v
            .check(&lim)
            .iter()
            .map(|x| (x.code, x.path.clone(), x.value))
            .collect();
        println!("root stagger {g}: {codes:?}");
    }
    for t in [0.25, 0.28, 0.3, 0.35] {
        let mut v = f.clone();
        v.n_blades = 11;
        v.stations[0].chord = 0.022;
        v.stations[0].stagger_deg = 60.0;
        v.stations[0].thickness = t;
        print_violations(
            &format!("11 blades, hub thickness {t}, no mould"),
            &v,
            &Limits {
                axial_mould: false,
                ..lim
            },
        );
    }
    for (c, g) in [(0.026, 50.0), (0.025, 52.0), (0.024, 55.0)] {
        let mut v = f.clone();
        v.stations[0].chord = c;
        v.stations[0].stagger_deg = g;
        print_violations(&format!("hub chord {c} stagger {g}"), &v, &lim);
    }
}

/// Gauss-Legendre points and weights on `[a, b]`.
fn gl(n: usize, a: f64, b: f64) -> Vec<(f64, f64)> {
    let (x, w) = gauss_legendre(n);
    x.iter()
        .zip(&w)
        .map(|(xi, wi)| (0.5 * (a + b) + 0.5 * (b - a) * xi, 0.5 * (b - a) * wi))
        .collect()
}

/// Independent mass properties of blade 0 (I4): volume and first moment by Green's theorem on a fine section polygon
/// (`int int g(s) dz ds = oint z g(s) ds`, `int int z dz ds = oint z^2/2 ds`, three Gauss points per edge) and Gauss-Legendre
/// in `r` between the control points. In cylindrical coordinates `dV = dr dz ds` with `s = r theta`.
fn quadrature_mass_properties(f: &ComputerFan, n_poly: usize) -> (f64, [f64; 3]) {
    quadrature_mass_properties_k(f, 0, n_poly)
}

/// As [`quadrature_mass_properties`] for blade `k` in its own frame (not rotated to its azimuth); the mass includes
/// `1 + mass_rel`.
fn quadrature_mass_properties_k(f: &ComputerFan, k: usize, n_poly: usize) -> (f64, [f64; 3]) {
    let af = f.axial_fan(k, 0.0, 0.0);
    let xs: Vec<f64> = (0..=n_poly)
        .map(|i| 0.5 * (1.0 - (std::f64::consts::PI * i as f64 / n_poly as f64).cos()))
        .collect();
    let g3 = gl(3, 0.0, 1.0);
    let (mut v, mut m) = (0.0, [0.0; 3]);
    let radii: Vec<f64> = af.stations.iter().map(|s| s.radius).collect();
    for w in radii.windows(2) {
        for (r, wr) in gl(16, w[0], w[1]) {
            let st = af.station_at(r);
            let mut poly: Vec<(f64, f64)> = xs.iter().map(|&x| af.surface_zs(&st, x, Side::Suction)).collect();
            poly.extend(
                xs[1..n_poly]
                    .iter()
                    .rev()
                    .map(|&x| af.surface_zs(&st, x, Side::Pressure)),
            );
            let (mut a, mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0, 0.0);
            for i in 0..poly.len() {
                let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                let ds = q.1 - p.1;
                for &(t, wt) in &g3 {
                    let (z, s) = (p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1));
                    a += wt * z * ds;
                    cx += wt * z * r * (s / r).cos() * ds;
                    cy += wt * z * r * (s / r).sin() * ds;
                    cz += wt * 0.5 * z * z * ds;
                }
            }
            let sg = a.signum();
            v += wr * sg * a;
            m[0] += wr * sg * cx;
            m[1] += wr * sg * cy;
            m[2] += wr * sg * cz;
        }
    }
    let mrel = f
        .deviation
        .as_ref()
        .and_then(|d| d.blades.get(k))
        .map_or(0.0, |b| b.mass_rel);
    (f.density * v * (1.0 + mrel), [m[0] / v, m[1] / v, m[2] / v])
}

#[test]
#[ignore = "rule-d smoke of ADR 0118 (mass properties and statistics set-ups), run once before registration"]
fn smoke_w46a_mass_and_stats() {
    let f = ComputerFan::default();
    for n in [1000, 2000, 4000] {
        let (m, c) = quadrature_mass_properties(&f, n);
        println!("quadrature n_poly {n}: mass {m:.12e} centroid {c:?}");
    }
    let (mq, cq) = quadrature_mass_properties(&f, 4000);
    for (nc, ns) in [(48, 24), (96, 48), (192, 96)] {
        let s = f.axial_fan(0, 0.0, 0.0).blade_surface(nc, ns).unwrap();
        let m = f.density * s.volume();
        let c = s.centroid();
        let dc = ((c[0] - cq[0]).powi(2) + (c[1] - cq[1]).powi(2) + (c[2] - cq[2]).powi(2)).sqrt();
        let rc = (cq[0] * cq[0] + cq[1] * cq[1]).sqrt();
        println!(
            "({nc},{ns}): mass rel err {:.3e}, centroid err / r_c {:.3e}",
            (m - mq).abs() / mq,
            dc / rc
        );
    }
    // negative-control magnitudes of the statistics set-up
    let d = Dist::TruncatedNormal {
        mean: 0.0,
        sd: 0.5,
        lo: -0.5,
        hi: 0.5,
    };
    let mut rng = rcfd_core::rng::Rng::new(1);
    let n = 28000;
    let ks = |xs: &mut Vec<f64>, cdf: &dyn Fn(f64) -> f64| {
        xs.sort_by(f64::total_cmp);
        let n = xs.len() as f64;
        xs.iter().enumerate().fold(0.0f64, |m, (i, &x)| {
            let fx = cdf(x);
            m.max((fx - i as f64 / n).abs()).max((fx - (i + 1) as f64 / n).abs())
        })
    };
    let mut clamp: Vec<f64> = (0..n)
        .map(|_| {
            // Box-Muller from two uniforms, clamped (the wrong truncation)
            let (u1, u2) = (1.0 - rng.uniform(), rng.uniform());
            (0.5 * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()).clamp(-0.5, 0.5)
        })
        .collect();
    println!(
        "NS1 clamped normal D = {:.4e}, DKW limit {:.4e}",
        ks(&mut clamp, &|x| d.cdf(x)),
        ((2.0f64 / 1e-3).ln() / (2.0 * n as f64)).sqrt()
    );
    let mut good: Vec<f64> = (0..n).map(|_| d.sample(&mut rng)).collect();
    println!("truncated sampler D = {:.4e}", ks(&mut good, &|x| d.cdf(x)));
    let c = Dist::TruncatedNormal {
        mean: 0.0,
        sd: 0.01,
        lo: -0.015,
        hi: 0.02,
    };
    let (mu, var) = (c.mean(), c.variance());
    let h = (3.0 * var).sqrt();
    let mut uni: Vec<f64> = (0..n).map(|_| rng.range(mu - h, mu + h)).collect();
    println!(
        "NS2 uniform with equal mean/var D = {:.4e}",
        ks(&mut uni, &|x| c.cdf(x))
    );
    let mrel = Dist::TruncatedNormal {
        mean: 0.005,
        sd: 0.01,
        lo: -0.01,
        hi: 0.04,
    };
    println!(
        "NS3 truncated mean {:.6e} vs parent mean 0.005; sd_t {:.6e}",
        mrel.mean(),
        mrel.variance().sqrt()
    );
    let tail = Dist::TruncatedNormal {
        mean: 0.0,
        sd: 0.01,
        lo: 0.02,
        hi: 0.06,
    };
    let t0 = std::time::Instant::now();
    let mut tx: Vec<f64> = (0..n).map(|_| tail.sample(&mut rng)).collect();
    println!(
        "tail D = {:.4e} ({:?} for {n} draws), mean {:.6e} var {:.6e}",
        ks(&mut tx, &|x| tail.cdf(x)),
        t0.elapsed(),
        tail.mean(),
        tail.variance()
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Registered checks of ADR 0118.

fn find<'a>(v: &'a [Violation], code: Code, path: &str) -> Option<&'a Violation> {
    v.iter().find(|x| x.code == code && x.path == path)
}

/// NC1: the bounds-only checker (every scalar inside its `PARAMS` range).
fn in_bounds(f: &ComputerFan) -> bool {
    f.scalars().iter().all(|(_, key, x)| {
        let s = param_spec(key).unwrap();
        *x >= s.min && *x <= s.max
    })
}

/// Set-up T of ADR 0118 E.
fn tol_t() -> Tolerances {
    let tn = |mean, sd, lo, hi| Dist::TruncatedNormal { mean, sd, lo, hi };
    Tolerances {
        stagger_common_deg: tn(0.0, 0.5, -1.0, 1.0),
        stagger_blade_deg: tn(0.0, 0.5, -0.5, 0.5),
        chord_rel: tn(0.0, 0.01, -0.015, 0.02),
        thickness_rel: Dist::Uniform { lo: -0.05, hi: 0.05 },
        tip_gap_delta: tn(0.0, 1e-4, -2e-4, 2e-4),
        eccentricity: tn(0.0, 1e-4, 0.0, 3e-4),
        mass_rel: tn(0.005, 0.01, -0.01, 0.04),
        roughness_ks: Dist::Uniform { lo: 0.0, hi: 2e-5 },
    }
}

/// Specimen of S3 / I4: blade-3 deviations only, plus a common stagger.
fn specimen_s3() -> ComputerFan {
    let mut d = Deviation {
        stagger_common_deg: 0.7,
        blades: vec![BladeDeviation::default(); 7],
        ..Deviation::default()
    };
    d.blades[3] = BladeDeviation {
        stagger_deg: -0.4,
        chord_rel: 0.03,
        thickness_rel: -0.05,
        mass_rel: 0.0,
        roughness_ks: 0.0,
    };
    ComputerFan {
        deviation: Some(d),
        ..ComputerFan::default()
    }
}

fn scalar_bits(f: &ComputerFan) -> Vec<(String, u64)> {
    f.scalars().into_iter().map(|(p, _, x)| (p, x.to_bits())).collect()
}

#[test]
fn s1_schema_round_trip_and_unknown_keys_rejected() {
    let spec = ComputerFan::default().ensemble(&tol_t(), 1, 46).unwrap().remove(0);
    for f in [ComputerFan::default(), spec] {
        let js = serde_json::to_string(&f).unwrap();
        let back: ComputerFan = serde_json::from_str(&js).unwrap();
        assert_eq!(back, f);
        assert_eq!(scalar_bits(&back), scalar_bits(&f));
        assert_eq!(serde_json::to_string(&back).unwrap(), js);
    }
    // NS-S
    let js = serde_json::to_string(&ComputerFan::default()).unwrap();
    assert_eq!(js.matches("\"tip_gap\"").count(), 1);
    assert!(serde_json::from_str::<ComputerFan>(&js.replacen("\"tip_gap\"", "\"tip_gapp\"", 1)).is_err());
    let mut v: serde_json::Value = serde_json::from_str(&js).unwrap();
    v["stations"][0].as_object_mut().unwrap().remove("chord");
    assert!(serde_json::from_value::<ComputerFan>(v).is_err());
}

#[test]
fn s2_schema_table_units_bounds_defaults() {
    let f0 = ComputerFan::default();
    let with_dev = ComputerFan {
        deviation: Some(Deviation {
            blades: vec![BladeDeviation::default(); 7],
            ..Deviation::default()
        }),
        ..f0.clone()
    };
    let spec = f0.ensemble(&tol_t(), 1, 46).unwrap().remove(0);
    let mut used = std::collections::HashSet::new();
    for f in [&f0, &with_dev, &spec] {
        for (path, key, _) in f.scalars() {
            assert!(param_spec(key).is_some(), "{path} has no spec");
            used.insert(key);
        }
    }
    for s in PARAMS {
        assert!(used.contains(s.path), "spec {} is not used", s.path);
        if let Some(d) = s.default {
            let got = with_dev.scalars().into_iter().find(|(p, _, _)| p == s.path).unwrap().2;
            assert_eq!(got.to_bits(), d.to_bits(), "default of {}", s.path);
        }
    }
    assert!(in_bounds(&f0) && in_bounds(&with_dev));
    let js = serde_json::to_string(PARAMS).unwrap();
    for s in PARAMS {
        assert!(
            js.contains(&format!("\"path\":\"{}\",\"unit\":\"{}\"", s.path, s.unit)),
            "{}",
            s.path
        );
    }
}

#[test]
fn s3_realisation_of_deviations() {
    let f0 = ComputerFan::default();
    let zero = ComputerFan {
        deviation: Some(Deviation {
            blades: vec![BladeDeviation::default(); 7],
            ..Deviation::default()
        }),
        ..f0.clone()
    };
    let nominal = f0.axial_fan(0, 0.03, 0.04);
    for k in 0..7 {
        assert_eq!(zero.axial_fan(k, 0.03, 0.04), nominal);
    }
    let mut s = specimen_s3();
    s.deviation.as_mut().unwrap().tip_gap_delta = -1.3e-4;
    let b = s.axial_fan(3, 0.03, 0.04);
    for (sn, sb) in nominal.stations.iter().zip(&b.stations) {
        assert!((sb.chord - sn.chord * 1.03).abs() <= 1e-15 * sn.chord);
        let (tn, tb) = (sn.chord * sn.thickness, sb.chord * sb.thickness);
        assert!((tb - tn * 0.95).abs() <= 1e-14 * tn, "{tb} {tn}");
        assert!((sb.stagger - (sn.stagger + 0.3f64.to_radians())).abs() <= 1e-15);
    }
    assert_eq!(b.tip_clearance, 0.001 + -1.3e-4);
}

fn smoke_specs() -> (SectorSpec, CGridSpec) {
    (
        SectorSpec {
            n_up: 8,
            n_chord: 16,
            n_down: 8,
            n_side: 10,
            n_mid: 2,
            n_span: 10,
            n_gap: 7,
            n_bl: 2,
            growth: 1.2,
            first_cell: FirstCell::Height(5e-5),
            trunc_le: 0.02,
            trunc_te: 0.01,
            include_blade: true,
        },
        CGridSpec {
            n_up: 8,
            n_chord: 24,
            n_down: 8,
            n_side: 10,
            n_nose: 6,
            n_core: 2,
            n_span: 10,
            n_gap: 7,
            n_bl: 2,
            growth: 1.2,
            first_cell: FirstCell::Height(5e-5),
            front_gap: 0.3,
            include_blade: true,
        },
    )
}

#[test]
fn s4_realised_default_meshes() {
    let af = ComputerFan::default().axial_fan(0, 0.03, 0.04);
    let (ss, cs) = smoke_specs();
    let h = build_sector_mesh(&af, &ss).expect("h_slit builds");
    println!("S4 h_slit: {} cells, {:?}", h.mesh.n_cells(), h.quality());
    let c = build_cgrid_mesh_with(&af, &cs, &CGridOptions::camber_seam_h_construction()).expect("camber_seam_h builds");
    println!("S4 c_grid camber_seam_h: {} cells, {:?}", c.mesh.n_cells(), c.quality());
    match build_cgrid_mesh_with(&af, &cs, &CGridOptions::default()) {
        Ok(m) => println!("S4 c_grid pursuit (reported): {} cells", m.mesh.n_cells()),
        Err(e) => println!("S4 c_grid pursuit (reported): rejected {e:?}"),
    }
    assert!(h.mesh.n_cells() > 0 && c.mesh.n_cells() > 0);
}

#[test]
fn c1_default_fan_is_accepted() {
    let v = ComputerFan::default().check(&Limits::default());
    assert!(v.is_empty(), "{v:?}");
}

struct Bad {
    name: &'static str,
    fan: ComputerFan,
    lim: Limits,
    code: Code,
    path: &'static str,
    radius: Option<f64>,
}

fn bad_variants() -> Vec<Bad> {
    let f0 = ComputerFan::default();
    let l0 = Limits::default();
    let no_mould = Limits {
        axial_mould: false,
        ..l0
    };
    let edit = |g: &dyn Fn(&mut ComputerFan)| {
        let mut f = f0.clone();
        g(&mut f);
        f
    };
    let b = |name, fan, lim, code, path, radius| Bad {
        name,
        fan,
        lim,
        code,
        path,
        radius,
    };
    let rh = Some(f0.r_hub);
    vec![
        b(
            "thin tip",
            edit(&|f| f.stations[2].thickness = 0.015),
            l0,
            Code::MinThickness,
            "stations[2].thickness",
            None,
        ),
        b(
            "small gap",
            edit(&|f| f.tip_gap = 2e-4),
            l0,
            Code::TipGap,
            "tip_gap",
            None,
        ),
        b(
            "large gap",
            edit(&|f| {
                f.tip_gap = 3.5e-3;
                f.frame.side = 0.13
            }),
            l0,
            Code::TipGap,
            "tip_gap",
            None,
        ),
        b(
            "eccentric rub",
            edit(&|f| {
                f.deviation = Some(Deviation {
                    eccentricity: 1.2e-3,
                    ..Deviation::default()
                })
            }),
            l0,
            Code::TipRub,
            "deviation.eccentricity",
            None,
        ),
        b(
            "hub ratio",
            edit(&|f| f.r_hub = 0.042),
            l0,
            Code::HubTipRatio,
            "r_hub",
            None,
        ),
        b(
            "low tip solidity",
            edit(&|f| {
                f.stations[2].chord = 0.012;
                f.stations[2].thickness = 0.1
            }),
            l0,
            Code::Solidity,
            "stations[2].chord",
            None,
        ),
        b(
            "axial overlap",
            edit(&|f| {
                f.stations[0].chord = 0.026;
                f.stations[0].stagger_deg = 50.0
            }),
            l0,
            Code::AxialOverlap,
            "stations[0].chord",
            rh,
        ),
        b(
            "collision",
            edit(&|f| {
                f.n_blades = 11;
                f.stations[0].chord = 0.022;
                f.stations[0].stagger_deg = 60.0;
                f.stations[0].thickness = 0.35
            }),
            no_mould,
            Code::Collision,
            "stations[0].chord",
            None,
        ),
        b(
            "passage gap",
            edit(&|f| {
                f.n_blades = 11;
                f.stations[0].chord = 0.022;
                f.stations[0].stagger_deg = 60.0;
                f.stations[0].thickness = 0.30
            }),
            no_mould,
            Code::PassageGap,
            "stations[0].chord",
            None,
        ),
        b(
            "draft",
            edit(&|f| {
                f.stations[0].stagger_deg = 12.0;
                f.frame.depth = 0.03;
                f.hub.length = 0.028
            }),
            l0,
            Code::Draft,
            "stations[0].stagger_deg",
            rh,
        ),
        b(
            "undercut",
            edit(&|f| {
                f.stations[0].stagger_deg = 0.0;
                f.stations[0].camber = 0.12
            }),
            l0,
            Code::Undercut,
            "stations[0].stagger_deg",
            rh,
        ),
        b(
            "thin blade 3",
            edit(&|f| {
                let mut d = Deviation {
                    blades: vec![BladeDeviation::default(); 7],
                    ..Deviation::default()
                };
                d.blades[3].thickness_rel = -0.45;
                f.deviation = Some(d)
            }),
            l0,
            Code::MinThickness,
            "stations[0].thickness",
            rh,
        ),
        b(
            "frame wall",
            edit(&|f| f.frame.side = 0.119),
            l0,
            Code::FrameWall,
            "frame.side",
            None,
        ),
        b(
            "axial fit",
            edit(&|f| f.z_le_root = 0.006),
            l0,
            Code::AxialFit,
            "stations[0].sweep",
            None,
        ),
        b(
            "root off hub",
            edit(&|f| {
                f.hub.z_start = 0.004;
                f.hub.length = 0.02
            }),
            l0,
            Code::RootOffHub,
            "z_le_root",
            rh,
        ),
    ]
}

#[test]
fn c2_c3_constructed_violations_are_located() {
    for b in bad_variants() {
        assert!(in_bounds(&b.fan), "NC1: {} must be in bounds", b.name);
        let v = b.fan.check(&b.lim);
        assert!(v.iter().all(|x| x.code != Code::OutOfBounds), "{}: {v:?}", b.name);
        let hit =
            find(&v, b.code, b.path).unwrap_or_else(|| panic!("{}: no {:?} at {}: {v:?}", b.name, b.code, b.path));
        if let Some(r) = b.radius {
            assert_eq!(hit.radius, Some(r), "{}", b.name);
        }
        if b.name == "thin blade 3" {
            assert!(v.iter().all(|x| x.blade == Some(3)), "{v:?}");
        }
        println!(
            "C2 {}: {:?} {} value {:.4e} limit {:.4e} ({} violations)",
            b.name,
            hit.code,
            hit.path,
            hit.value,
            hit.limit,
            v.len()
        );
        if matches!(b.code, Code::AxialOverlap | Code::Draft | Code::Undercut) {
            let off = b.fan.check(&Limits {
                axial_mould: false,
                ..b.lim
            });
            assert!(off.iter().all(|x| x.code != b.code), "C3 {}: {off:?}", b.name);
        }
    }
}

#[test]
fn c4_structural_violations_are_located() {
    let f0 = ComputerFan::default();
    let l0 = Limits::default();
    let mut f = f0.clone();
    f.n_blades = 30;
    let v = f.check(&l0);
    let x = find(&v, Code::OutOfBounds, "n_blades").expect("n_blades");
    assert_eq!((x.value, x.limit), (30.0, 15.0));
    let mut f = f0.clone();
    f.deviation = Some(Deviation {
        blades: vec![BladeDeviation::default(); 3],
        ..Deviation::default()
    });
    assert!(find(&f.check(&l0), Code::Invalid, "deviation.blades").is_some());
    let mut f = f0.clone();
    f.stations[1].span = 0.0;
    assert!(
        find(&f.check(&l0), Code::Invalid, "stations[1].span").is_some(),
        "{:?}",
        f.check(&l0)
    );
    let mut f = f0.clone();
    f.deviation = Some(Deviation {
        tip_gap_delta: -1.1e-3,
        ..Deviation::default()
    });
    let v = f.check(&l0);
    assert!(find(&v, Code::Invalid, "tip_gap").is_some(), "{v:?}");
    assert!(
        v.iter().all(|x| matches!(x.code, Code::Invalid | Code::OutOfBounds)),
        "{v:?}"
    );
}

#[test]
fn c5_violations_serialise() {
    let mut f = ComputerFan::default();
    f.stations[2].thickness = 0.015;
    let v = f.check(&Limits::default());
    let js = serde_json::to_string(&v[0]).unwrap();
    assert!(js.contains("\"code\":\"min_thickness\""), "{js}");
    let back: Violation = serde_json::from_str(&js).unwrap();
    assert_eq!(back, v[0]);
}

#[test]
fn e1_ensembles_are_reproducible_and_zero_tolerance_is_nominal() {
    let f0 = ComputerFan::default();
    let a = serde_json::to_string(&f0.ensemble(&tol_t(), 50, 46).unwrap()).unwrap();
    let b = serde_json::to_string(&f0.ensemble(&tol_t(), 50, 46).unwrap()).unwrap();
    let c = serde_json::to_string(&f0.ensemble(&tol_t(), 50, 47).unwrap()).unwrap();
    assert_eq!(a, b);
    assert_ne!(a, c);
    let u0 = f0.static_unbalance().unwrap();
    let nominal: Vec<AxialFan> = (0..7).map(|k| f0.axial_fan(k, 0.0, 0.0)).collect();
    for s in f0.ensemble(&Tolerances::default(), 3, 46).unwrap() {
        for k in 0..7 {
            assert_eq!(s.axial_fan(k, 0.0, 0.0), nominal[k]);
        }
        let u = s.static_unbalance().unwrap();
        assert_eq!((u[0].to_bits(), u[1].to_bits()), (u0[0].to_bits(), u0[1].to_bits()));
    }
}

fn pdf(d: &Dist, x: f64) -> f64 {
    match *d {
        Dist::Uniform { lo, hi } => 1.0 / (hi - lo),
        Dist::TruncatedNormal { mean, sd, lo, hi } => {
            let z = normal_cdf((hi - mean) / sd) - normal_cdf((lo - mean) / sd);
            let t = (x - mean) / sd;
            (-0.5 * t * t).exp() / (std::f64::consts::TAU.sqrt() * sd * z)
        }
        Dist::Fixed { .. } => unreachable!(),
    }
}

/// (mass, mean, variance, fourth central moment) of the density by composite Simpson (20 000 intervals).
fn simpson_moments(d: &Dist) -> (f64, f64, f64, f64) {
    let (lo, hi) = match *d {
        Dist::Uniform { lo, hi } | Dist::TruncatedNormal { lo, hi, .. } => (lo, hi),
        Dist::Fixed { .. } => unreachable!(),
    };
    let n = 20000;
    let h = (hi - lo) / n as f64;
    let int = |g: &dyn Fn(f64) -> f64| {
        (0..=n)
            .map(|i| {
                let x = lo + h * i as f64;
                let w = if i == 0 || i == n {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                w * g(x) * pdf(d, x)
            })
            .sum::<f64>()
            * h
            / 3.0
    };
    let mass = int(&|_| 1.0);
    let mean = int(&|x| x) / mass;
    let var = int(&|x| (x - mean).powi(2)) / mass;
    let m4 = int(&|x| (x - mean).powi(4)) / mass;
    (mass, mean, var, m4)
}

fn ks_stat(xs: &[f64], d: &Dist) -> f64 {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len() as f64;
    s.iter().enumerate().fold(0.0f64, |m, (i, &x)| {
        let f = d.cdf(x);
        m.max((f - i as f64 / n).abs()).max((f - (i + 1) as f64 / n).abs())
    })
}

fn ks_limit(n: usize) -> f64 {
    ((2.0f64 / 1e-3).ln() / (2.0 * n as f64)).sqrt()
}

/// E2 limits; returns the (mean, variance, KS) ratios measured / limit.
fn stats_ratios(xs: &[f64], d: &Dist) -> (f64, f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let s2 = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let (mu, var) = (d.mean(), d.variance());
    let (_, _, _, m4) = simpson_moments(d);
    (
        (mean - mu).abs() / (4.5 * var.sqrt() / n.sqrt()),
        (s2 - var).abs() / (4.5 * ((m4 - var * var) / n).sqrt()),
        ks_stat(xs, d) / ks_limit(xs.len()),
    )
}

#[test]
fn e2_e3_ensemble_statistics_match_the_distributions() {
    let t = tol_t();
    let ens = ComputerFan::default().ensemble(&t, 4000, 46).unwrap();
    let dv = |f: &dyn Fn(&Deviation) -> f64| {
        ens.iter()
            .map(|s| f(s.deviation.as_ref().unwrap()))
            .collect::<Vec<f64>>()
    };
    let bv = |f: &dyn Fn(&BladeDeviation) -> f64| {
        ens.iter()
            .flat_map(|s| s.deviation.as_ref().unwrap().blades.iter().map(f).collect::<Vec<f64>>())
            .collect::<Vec<f64>>()
    };
    let tail = Dist::TruncatedNormal {
        mean: 0.0,
        sd: 0.01,
        lo: 0.02,
        hi: 0.06,
    };
    let mut rng = rcfd_core::rng::Rng::new(4601);
    let tail_xs: Vec<f64> = (0..28000).map(|_| tail.sample(&mut rng)).collect();
    let cases: Vec<(&str, Vec<f64>, Dist)> = vec![
        (
            "stagger_common_deg",
            dv(&|d| d.stagger_common_deg),
            t.stagger_common_deg,
        ),
        ("tip_gap_delta", dv(&|d| d.tip_gap_delta), t.tip_gap_delta),
        ("eccentricity", dv(&|d| d.eccentricity), t.eccentricity),
        (
            "eccentricity_angle_deg",
            dv(&|d| d.eccentricity_angle_deg),
            Dist::Uniform { lo: 0.0, hi: 360.0 },
        ),
        ("stagger_blade_deg", bv(&|b| b.stagger_deg), t.stagger_blade_deg),
        ("chord_rel", bv(&|b| b.chord_rel), t.chord_rel),
        ("thickness_rel", bv(&|b| b.thickness_rel), t.thickness_rel),
        ("mass_rel", bv(&|b| b.mass_rel), t.mass_rel),
        ("roughness_ks", bv(&|b| b.roughness_ks), t.roughness_ks),
        ("E2b far tail", tail_xs, tail),
    ];
    let mut worst: f64 = 0.0;
    for (name, xs, d) in &cases {
        let (rm, rv, rk) = stats_ratios(xs, d);
        println!(
            "E2 {name}: n {} mean {rm:.3} var {rv:.3} KS {rk:.3} (measured / limit)",
            xs.len()
        );
        assert!(rm <= 1.0 && rv <= 1.0 && rk <= 1.0, "{name}");
        worst = worst.max(rm).max(rv).max(rk);
        if let Dist::TruncatedNormal { .. } = d {
            let (mass, mean, var, _) = simpson_moments(d);
            let (em, ev) = (
                (mean - d.mean()).abs() / d.mean().abs().max(d.variance().sqrt()),
                (var - d.variance()).abs() / d.variance(),
            );
            println!("E3 {name}: mass {mass:.12} mean rel err {em:.2e} var rel err {ev:.2e}");
            assert!((mass - 1.0).abs() <= 1e-9 && em <= 1e-9 && ev <= 1e-9, "{name}");
        }
    }
    println!("E2 worst ratio {worst:.3}");
}

#[test]
fn e2_negative_controls_discriminate() {
    let n = 28000;
    let mut rng = rcfd_core::rng::Rng::new(4602);
    // NS1: clamped instead of truncated
    let d = Dist::TruncatedNormal {
        mean: 0.0,
        sd: 0.5,
        lo: -0.5,
        hi: 0.5,
    };
    let clamp: Vec<f64> = (0..n)
        .map(|_| {
            let (u1, u2) = (1.0 - rng.uniform(), rng.uniform());
            (0.5 * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()).clamp(-0.5, 0.5)
        })
        .collect();
    let ns1 = ks_stat(&clamp, &d) / ks_limit(n);
    // NS2: uniform with the mean and variance of chord_rel
    let c = tol_t().chord_rel;
    let h = (3.0 * c.variance()).sqrt();
    let uni: Vec<f64> = (0..n).map(|_| rng.range(c.mean() - h, c.mean() + h)).collect();
    let ns2 = ks_stat(&uni, &c) / ks_limit(n);
    // NS3: the parent mean taken as the expected mean of mass_rel
    let m = tol_t().mass_rel;
    let xs: Vec<f64> = (0..n).map(|_| m.sample(&mut rng)).collect();
    let xbar = xs.iter().sum::<f64>() / n as f64;
    let ns3 = (xbar - 0.005).abs() / (4.5 * m.variance().sqrt() / (n as f64).sqrt());
    println!("NS1 {ns1:.2}x, NS2 {ns2:.2}x, NS3 {ns3:.2}x of the limit");
    assert!(ns1 >= 2.0 && ns2 >= 2.0 && ns3 >= 2.0);
}

fn rot(k: usize, n: usize, c: [f64; 3]) -> [f64; 2] {
    let (s, co) = (std::f64::consts::TAU * k as f64 / n as f64).sin_cos();
    [co * c[0] - s * c[1], s * c[0] + co * c[1]]
}

#[test]
fn i1_i2_i3_unbalance_equals_the_sum_of_mass_times_offset() {
    let f0 = ComputerFan::default();
    let n = f0.n_blades;
    let (m0, c0) = f0.blade_mass(0).unwrap();
    let rc = c0[0].hypot(c0[1]);
    // I1 and NI1 (centroids not rotated: |sum m c0| = N m0 r_c)
    let u = f0.static_unbalance().unwrap();
    let lim1 = 1e-12 * n as f64 * m0 * rc;
    let (i1, ni1) = (u[0].hypot(u[1]) / lim1, n as f64 * m0 * rc / lim1);
    println!(
        "I1 |U| = {:.3e} kg m, {i1:.3e} of the limit; NI1 {ni1:.1e} of the limit",
        u[0].hypot(u[1])
    );
    assert!(i1 <= 1.0 && ni1 >= 2.0);
    // I2 and NI2
    let mut f = f0.clone();
    let mut d = Deviation {
        blades: vec![BladeDeviation::default(); n],
        ..Deviation::default()
    };
    d.blades[3].mass_rel = 0.01;
    f.deviation = Some(d);
    let u = f.static_unbalance().unwrap();
    let e = rot(3, n, c0).map(|x| 0.01 * m0 * x);
    let lim2 = 1e-10 * 0.01 * m0 * rc;
    let i2 = (u[0] - e[0]).hypot(u[1] - e[1]) / lim2;
    let uw = f0.static_unbalance().unwrap(); // NI2: mass_rel ignored
    let ni2 = (uw[0] - e[0]).hypot(uw[1] - e[1]) / lim2;
    println!(
        "I2 |U| = {:.6e} kg m (expected {:.6e}), {i2:.3e} of the limit; NI2 {ni2:.2e}",
        u[0].hypot(u[1]),
        e[0].hypot(e[1])
    );
    assert!(i2 <= 1.0 && ni2 >= 2.0);
    // I3
    let tol = Tolerances {
        mass_rel: tol_t().mass_rel,
        ..Tolerances::default()
    };
    let mut worst: f64 = 0.0;
    for s in f0.ensemble(&tol, 20, 46).unwrap() {
        let u = s.static_unbalance().unwrap();
        let mut e = [0.0; 2];
        for (k, b) in s.deviation.as_ref().unwrap().blades.iter().enumerate() {
            let r = rot(k, n, c0);
            e[0] += m0 * b.mass_rel * r[0];
            e[1] += m0 * b.mass_rel * r[1];
        }
        worst = worst.max((u[0] - e[0]).hypot(u[1] - e[1]) / (1e-10 * m0 * rc));
    }
    println!("I3 worst {worst:.3e} of the limit");
    assert!(worst <= 1.0);
}

#[test]
fn i4_blade_mass_and_centroid_against_quadrature() {
    for (tag, f, k) in [
        ("F0 blade 0", ComputerFan::default(), 0usize),
        ("S3 blade 3", specimen_s3(), 3),
    ] {
        let (mq, cq0) = quadrature_mass_properties_k(&f, k, 4000);
        let r2 = rot(k, f.n_blades, cq0);
        let cq = [r2[0], r2[1], cq0[2]];
        let rc = cq[0].hypot(cq[1]);
        let (mb, cb) = f.blade_mass(k).unwrap();
        let mut errs = Vec::new();
        for (nc, ns) in [(96, 48), (192, 96)] {
            let s = f.axial_fan(k, 0.0, 0.0).blade_surface(nc, ns).unwrap();
            let m = f.density * s.volume();
            let c0 = s.centroid();
            let r2 = rot(k, f.n_blades, c0);
            let c = [r2[0], r2[1], c0[2]];
            if (nc, ns) == MASS_RESOLUTION {
                assert_eq!(m.to_bits(), mb.to_bits());
                assert!((0..3).all(|i| (c[i] - cb[i]).abs() <= 1e-15 * rc));
            }
            let dc = ((c[0] - cq[0]).powi(2) + (c[1] - cq[1]).powi(2) + (c[2] - cq[2]).powi(2)).sqrt();
            errs.push(((m - mq).abs() / mq, dc / rc));
        }
        println!(
            "I4 {tag}: mass {mq:.6e} kg; (96,48) mass err {:.3e} centroid err {:.3e}; ratios {:.2} {:.2}",
            errs[0].0,
            errs[0].1,
            errs[0].0 / errs[1].0,
            errs[0].1 / errs[1].1
        );
        assert!(errs[0].0 <= 1e-3 && errs[0].1 <= 2e-4, "{tag}");
        assert!(errs[0].0 / errs[1].0 >= 3.0 && errs[0].1 / errs[1].1 >= 3.0, "{tag}");
    }
}
