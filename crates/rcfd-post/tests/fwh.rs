//! FW-H formulation 1A against exact analytic fields (ADR 0013, checks A1-A8). Limits are those registered in the ADR.

use rcfd_post::fwh::{Element, Surface};
use std::f64::consts::PI;

const RHO0: f64 = 1.2;
const C0: f64 = 340.0;
type V3 = [f64; 3];

fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..n {
        let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            let dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-15 {
                break;
            }
        }
        let (mut p0, mut p1) = (1.0, x);
        for k in 2..=n {
            let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
            p0 = p1;
            p1 = p2;
        }
        let dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
        out.push((x, 2.0 / ((1.0 - x * x) * dp * dp)));
    }
    out
}

/// Unit sphere quadrature: (outward normal, weight summing to 4 pi).
fn sphere_points() -> Vec<(V3, f64)> {
    let gl = gauss_legendre(16);
    let nphi = 32;
    let mut pts = Vec::new();
    for (mu, w) in gl {
        let s = (1.0 - mu * mu).sqrt();
        for j in 0..nphi {
            let phi = 2.0 * PI * (j as f64 + 0.5) / nphi as f64;
            pts.push(([s * phi.cos(), s * phi.sin(), mu], w * 2.0 * PI / nphi as f64));
        }
    }
    pts
}

/// Build a surface of radius `a` around the path `centre(t)` (velocity `vel(t)`), sampled `ns` times.
fn build(
    a: f64,
    t0: f64,
    dt: f64,
    ns: usize,
    centre: &dyn Fn(f64) -> V3,
    vel: &dyn Fn(f64) -> V3,
    field: &dyn Fn(V3, f64) -> (f64, V3),
) -> Surface {
    let mut elements = Vec::new();
    for (nrm, w) in sphere_points() {
        let mut e = Element {
            area: w * a * a,
            x: vec![],
            n: vec![],
            v: vec![],
            p: vec![],
            rho: vec![],
            u: vec![],
        };
        for i in 0..ns {
            let t = t0 + i as f64 * dt;
            let c = centre(t);
            let x = [c[0] + a * nrm[0], c[1] + a * nrm[1], c[2] + a * nrm[2]];
            let (p, u) = field(x, t);
            e.x.push(x);
            e.n.push(nrm);
            e.v.push(vel(t));
            e.p.push(p);
            e.rho.push(p / (C0 * C0));
            e.u.push(u);
        }
        elements.push(e);
    }
    Surface {
        rho0: RHO0,
        c0: C0,
        t0,
        dt,
        elements,
    }
}

fn rms(v: &[f64]) -> f64 {
    (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt()
}

/// FW-H vs exact over one period starting inside the window; returns the normalised RMS error.
fn compare(s: &Surface, obs: V3, period: f64, exact: &dyn Fn(f64) -> f64) -> f64 {
    let (lo, hi) = s.valid_window(obs).unwrap();
    assert!(
        hi - lo > 2.0 * period,
        "window too short: {} vs period {period}",
        hi - lo
    );
    let n = 256;
    let ts: Vec<f64> = (0..n)
        .map(|i| lo + 0.5 * period + period * i as f64 / n as f64)
        .collect();
    let sig = s.signal(obs, &ts).unwrap();
    let diff: Vec<f64> = sig.iter().zip(&ts).map(|(g, &t)| g.total() - exact(t)).collect();
    let ex: Vec<f64> = ts.iter().map(|&t| exact(t)).collect();
    rms(&diff) / rms(&ex)
}

// ---------- monopole at rest ----------
const KM: f64 = PI / 2.0; // wavelength 4 m
fn omega_m() -> f64 {
    KM * C0
}
fn q0() -> f64 {
    4.0 * PI * 0.01 / (RHO0 * omega_m())
}
fn mono_field(x: V3, t: f64) -> (f64, V3) {
    let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
    let w = omega_m();
    let tau = t - r / C0;
    let q = q0() * (w * tau).sin();
    let qd = q0() * w * (w * tau).cos();
    let p = RHO0 * qd / (4.0 * PI * r);
    let ur = (q / (r * r) + qd / (C0 * r)) / (4.0 * PI);
    (p, [ur * x[0] / r, ur * x[1] / r, ur * x[2] / r])
}
fn mono_surface() -> Surface {
    let period = 2.0 * PI / omega_m();
    let dt = period / 64.0;
    build(1.0, 0.0, dt, 64 * 14, &|_| [0.0; 3], &|_| [0.0; 3], &mono_field)
}
fn mono_exact_p(r: f64, t: f64) -> f64 {
    mono_field([r, 0.0, 0.0], t).0
}

fn fundamental(s: &Surface, obs: V3, w: f64) -> (f64, f64) {
    let period = 2.0 * PI / w;
    let (lo, _) = s.valid_window(obs).unwrap();
    let n = 256;
    let ts: Vec<f64> = (0..n)
        .map(|i| lo + 0.5 * period + period * i as f64 / n as f64)
        .collect();
    let sig = s.signal(obs, &ts).unwrap();
    let (mut re, mut im) = (0.0, 0.0);
    for (g, &t) in sig.iter().zip(&ts) {
        re += g.total() * (w * t).cos();
        im -= g.total() * (w * t).sin();
    }
    let (re, im) = (2.0 * re / n as f64, 2.0 * im / n as f64);
    ((re * re + im * im).sqrt(), im.atan2(re))
}

#[test]
fn a1_monopole_matches_exact() {
    let s = mono_surface();
    let period = 2.0 * PI / omega_m();
    for r in [3.0, 10.0, 50.0] {
        let e = compare(&s, [r, 0.0, 0.0], period, &|t| mono_exact_p(r, t));
        println!("A1 r = {r}: e = {e:.3e}");
        assert!(e <= 1e-3, "A1 r = {r}: e = {e:.3e} > 1e-3");
    }
}

#[test]
fn a2_inverse_distance_decay() {
    let s = mono_surface();
    let amps: Vec<f64> = [50.0, 100.0, 200.0, 400.0]
        .iter()
        .map(|&r| fundamental(&s, [0.0, r, 0.0], omega_m()).0 * r)
        .collect();
    let (mx, mn) = (
        amps.iter().cloned().fold(f64::MIN, f64::max),
        amps.iter().cloned().fold(f64::MAX, f64::min),
    );
    println!("A2 amp*r = {amps:?}, spread {:.3e}", mx / mn - 1.0);
    assert!(mx / mn - 1.0 <= 1e-3);
}

#[test]
fn a3_retarded_time_phase() {
    let s = mono_surface();
    let r = 50.0;
    let w = omega_m();
    let (_, ph) = fundamental(&s, [r, 0.0, 0.0], w);
    let expect = -w * r / C0;
    let mut d = ph - expect;
    d -= 2.0 * PI * (d / (2.0 * PI)).round();
    println!("A3 phase error = {d:.3e} rad");
    assert!(d.abs() <= 1e-3);
}

// ---------- dipole at rest ----------
fn dip_f0() -> f64 {
    8.0
}
fn dip_field(x: V3, t: f64) -> (f64, V3) {
    let r = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
    let w = omega_m();
    let tau = t - r / C0;
    let f = dip_f0() * (w * tau).sin();
    let fd = dip_f0() * w * (w * tau).cos();
    let big_f = -dip_f0() * (w * tau).cos() / w;
    let p = x[0] / (4.0 * PI * r * r) * (f / r + fd / C0);
    let h = -f / (C0 * r * r) - big_f / (r * r * r);
    let hr = fd / (C0 * C0 * r * r) + 3.0 * f / (C0 * r * r * r) + 3.0 * big_f / r.powi(4);
    let k = 1.0 / (4.0 * PI * RHO0);
    let u = [
        k * (h + x[0] * x[0] / r * hr),
        k * (x[0] * x[1] / r * hr),
        k * (x[0] * x[2] / r * hr),
    ];
    (p, u)
}

#[test]
fn a4_dipole_directivity_and_series() {
    let period = 2.0 * PI / omega_m();
    let dt = period / 64.0;
    let s = build(1.0, 0.0, dt, 64 * 14, &|_| [0.0; 3], &|_| [0.0; 3], &dip_field);
    let r = 100.0;
    let (a0, _) = fundamental(&s, [r, 0.0, 0.0], omega_m());
    for deg in [0.0_f64, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0] {
        let th = deg.to_radians();
        let (a, _) = fundamental(&s, [r * th.cos(), r * th.sin(), 0.0], omega_m());
        let err = (a / a0 - th.cos().abs()).abs();
        println!(
            "A4 theta {deg}: ratio {:.6} vs {:.6}, err {err:.2e}",
            a / a0,
            th.cos().abs()
        );
        assert!(err <= 1e-3, "A4 theta {deg}: {err:.3e}");
    }
    let th = 45.0_f64.to_radians();
    let obs = [r * th.cos(), r * th.sin(), 0.0];
    let e = compare(&s, obs, period, &|t| dip_field(obs, t).0);
    println!("A4 series e = {e:.3e}");
    assert!(e <= 1e-3);
}

// ---------- orbiting sources ----------
const R_ORB: f64 = 1.0;
fn omega_rot() -> f64 {
    0.3 * C0 / R_ORB
}
fn omega_src() -> f64 {
    4.0 * omega_rot()
}
fn orbit(tau: f64, phase: f64) -> V3 {
    [
        R_ORB * (omega_rot() * tau + phase).cos(),
        R_ORB * (omega_rot() * tau + phase).sin(),
        0.0,
    ]
}
fn orbit_vel(tau: f64, phase: f64) -> V3 {
    [
        -R_ORB * omega_rot() * (omega_rot() * tau + phase).sin(),
        R_ORB * omega_rot() * (omega_rot() * tau + phase).cos(),
        0.0,
    ]
}

/// Velocity potential of a moving monopole q0 sin(omega_src tau) at orbit phase offset `phase`.
fn phi_moving(x: V3, t: f64, phase: f64, sign: f64) -> f64 {
    let mut tau = t - (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt() / C0;
    for _ in 0..100 {
        let y = orbit(tau, phase);
        let v = orbit_vel(tau, phase);
        let r = [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
        let rn = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
        let g = tau + rn / C0 - t;
        let gp = 1.0 - (v[0] * r[0] + v[1] * r[1] + v[2] * r[2]) / (C0 * rn);
        tau -= g / gp;
        if (g / gp).abs() < 1e-15 {
            break;
        }
    }
    let y = orbit(tau, phase);
    let v = orbit_vel(tau, phase);
    let r = [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let rn = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let mr = (v[0] * r[0] + v[1] * r[1] + v[2] * r[2]) / (C0 * rn);
    let q = sign * q0_rot() * (omega_src() * tau).sin();
    -q / (4.0 * PI * rn * (1.0 - mr))
}
fn q0_rot() -> f64 {
    4.0 * PI * 0.01 / (RHO0 * omega_src())
}

const FD8: [f64; 9] = [
    1.0 / 280.0,
    -4.0 / 105.0,
    1.0 / 5.0,
    -4.0 / 5.0,
    0.0,
    4.0 / 5.0,
    -1.0 / 5.0,
    4.0 / 105.0,
    -1.0 / 280.0,
];

fn deriv8(h: f64, f: &dyn Fn(f64) -> f64) -> f64 {
    let mut s = 0.0;
    for (k, c) in FD8.iter().enumerate() {
        if *c != 0.0 {
            s += c * f((k as f64 - 4.0) * h);
        }
    }
    s / h
}

/// Field of a set of (phase, sign) orbiting monopoles: p' = -rho0 phi_t, u = grad phi.
fn orbit_field(sources: &[(f64, f64)], x: V3, t: f64) -> (f64, V3) {
    let phi = |x: V3, t: f64| sources.iter().map(|&(ph, sg)| phi_moving(x, t, ph, sg)).sum::<f64>();
    let ht = 1e-4 * 2.0 * PI / omega_src() * 4.0; // small relative to the source period
    let p = -RHO0 * deriv8(ht, &|d| phi(x, t + d));
    let hs = 0.01;
    let mut u = [0.0; 3];
    for c in 0..3 {
        u[c] = deriv8(hs, &|d| {
            let mut y = x;
            y[c] += d;
            phi(y, t)
        });
    }
    (p, u)
}

fn orbit_case(sources: &'static [(f64, f64)], obs: V3) -> f64 {
    let trot = 2.0 * PI / omega_rot();
    let tsrc = 2.0 * PI / omega_src();
    let dt = tsrc / 64.0;
    let ns = (3.5 * trot / dt) as usize;
    let centre = |t: f64| orbit(t, 0.0);
    let vel = |t: f64| orbit_vel(t, 0.0);
    let s = build(0.3, 0.0, dt, ns, &centre, &vel, &|x, t| orbit_field(sources, x, t));
    compare(&s, obs, trot, &|t| orbit_field(sources, obs, t).0)
}

#[test]
fn a5_rotating_monopole() {
    static SRC: [(f64, f64); 1] = [(0.0, 1.0)];
    for (name, obs) in [
        ("in-plane x", [30.0, 0.0, 0.0]),
        ("in-plane y", [0.0, 30.0, 0.0]),
        (
            "30 deg out of plane",
            [30.0 * 30f64.to_radians().cos(), 0.0, 30.0 * 30f64.to_radians().sin()],
        ),
    ] {
        let e = orbit_case(&SRC, obs);
        println!("A5 {name}: e = {e:.3e}");
        assert!(e <= 1e-3, "A5 {name}: e = {e:.3e} > 1e-3");
    }
}

#[test]
fn a6_rotating_dipole_pair() {
    static SRC: [(f64, f64); 2] = [(0.05, 1.0), (-0.05, -1.0)];
    for (name, obs) in [("in-plane x", [30.0, 0.0, 0.0]), ("in-plane y", [0.0, 30.0, 0.0])] {
        let e = orbit_case(&SRC, obs);
        println!("A6 {name}: e = {e:.3e}");
        assert!(e <= 1e-3, "A6 {name}: e = {e:.3e} > 1e-3");
    }
}

// ---------- files and guards ----------
#[test]
fn a7_json_round_trip_and_located_errors() {
    let period = 2.0 * PI / omega_m();
    let mut s = build(
        1.0,
        0.0,
        period / 64.0,
        64 * 4,
        &|_| [0.0; 3],
        &|_| [0.0; 3],
        &mono_field,
    );
    s.elements.truncate(40);
    let text = serde_json::to_string(&s.to_json()).unwrap();
    let back = Surface::from_json(&serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(back, s, "round trip is bit exact");
    let mut doc = s.to_json();
    doc["elements"][3]["p"].as_array_mut().unwrap().pop();
    let err = Surface::from_json(&doc).unwrap_err().to_string();
    assert!(err.contains("elements[3].p"), "{err}");
    let mut doc = s.to_json();
    doc["elements"][1]["n"][2] = serde_json::json!("x");
    let err = Surface::from_json(&doc).unwrap_err().to_string();
    assert!(err.contains("elements[1].n[2]"), "{err}");
    let mut doc = s.to_json();
    doc["format"] = serde_json::json!("other");
    assert!(Surface::from_json(&doc).unwrap_err().to_string().contains("$.format"));
}

#[test]
fn a8_guards() {
    let period = 2.0 * PI / omega_m();
    let mut s = build(
        1.0,
        0.0,
        period / 64.0,
        64 * 4,
        &|_| [0.0; 3],
        &|_| [0.0; 3],
        &mono_field,
    );
    s.elements.truncate(20);
    let obs = [10.0, 0.0, 0.0];
    let (lo, hi) = s.valid_window(obs).unwrap();
    let e = s.signal(obs, &[lo - 1e-3]).unwrap_err().to_string();
    assert!(e.contains("valid window"), "{e}");
    assert!(s.signal(obs, &[hi + 1e-3]).is_err());
    let mut fast = s.clone();
    fast.elements[2].v[5] = [C0 * 1.1, 0.0, 0.0];
    let e = fast.validate().unwrap_err().to_string();
    assert!(e.contains("elements[2].v[5]") && e.contains("subsonic"), "{e}");
    let mut bad = s.clone();
    bad.dt = -1.0;
    assert!(bad.validate().is_err());
}
