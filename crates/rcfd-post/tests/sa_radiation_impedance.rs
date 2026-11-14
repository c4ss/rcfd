//! F-551 (ADR 0488): surface pressure and radiation impedance of a baffled plane, the singular part 1/R of the Rayleigh kernel
//! integrated analytically over each face. Checks A1 to A3, G1, S1, S2, Z1 to Z3, M1, H1, NC1 to NC3, N1 and T1 of the
//! registration (DB1 in `sa_radiation_impedance_default_bits.rs`).

use rcfd_core::vec3::Vec3;
use rcfd_post::solid_acoustic::{polygon_inverse_distance, BaffledPlane, Phasor, RayleighOptions, VibratingSurface};
use std::f64::consts::PI;

const RHO: f64 = 1.2;
const C: f64 = 340.0;
const A: f64 = 0.1;
const V: f64 = 1e-3;

struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, bound: f64) {
        let ok = value > bound;
        println!(
            "{name}: {value:.6e} (must be > {bound:e}) {}",
            if ok { "as registered" } else { "NOT AS REGISTERED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {bound:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn rel(x: f64, r: f64) -> f64 {
    (x / r - 1.0).abs()
}

/// Gauss-Legendre nodes and weights on [-1, 1] (Newton on the recurrence).
fn gauss(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            let mut dp = 1.0;
            for _ in 0..100 {
                let (mut p0, mut p1) = (1.0, x);
                for k in 2..=n {
                    let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                    p0 = p1;
                    p1 = p2;
                }
                dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
                let dx = p1 / dp;
                x -= dx;
                if dx.abs() < 1e-15 {
                    break;
                }
            }
            (x, 2.0 / ((1.0 - x * x) * dp * dp))
        })
        .collect()
}

/// J1 by `(1/pi) int_0^pi cos(t - x sin t) dt`, 400-point midpoint rule (as ADR 0468).
fn j1(x: f64) -> f64 {
    let n = 400;
    (0..n)
        .map(|i| {
            let t = (i as f64 + 0.5) * PI / n as f64;
            (t - x * t.sin()).cos()
        })
        .sum::<f64>()
        / n as f64
}

/// Struve H1 by `(2x/pi) int_0^{pi/2} cos^2 p sin(x sin p) dp`, 200-point Gauss-Legendre.
fn h1(x: f64) -> f64 {
    let s: f64 = gauss(200)
        .iter()
        .map(|&(u, w)| {
            let p = (u + 1.0) * PI / 4.0;
            w * PI / 4.0 * p.cos().powi(2) * (x * p.sin()).sin()
        })
        .sum();
    2.0 * x / PI * s
}

/// The complete elliptic integral of the second kind of modulus `m`, 400-point Gauss-Legendre.
fn ellip_e(m: f64) -> f64 {
    gauss(400)
        .iter()
        .map(|&(u, w)| {
            let p = (u + 1.0) * PI / 4.0;
            w * PI / 4.0 * (1.0 - m * m * p.sin().powi(2)).sqrt()
        })
        .sum()
}

/// The polar disk mesh of ADR 0468.
fn disk(a: f64, n_r: usize, n_t: usize) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let mut nodes = vec![[0.0, 0.0, 0.0]];
    for j in 1..=n_r {
        for m in 0..n_t {
            let t = 2.0 * PI * m as f64 / n_t as f64;
            let r = a * j as f64 / n_r as f64;
            nodes.push([r * t.cos(), r * t.sin(), 0.0]);
        }
    }
    let idx = |j: usize, m: usize| if j == 0 { 0 } else { 1 + (j - 1) * n_t + (m % n_t) };
    let mut faces = Vec::new();
    for m in 0..n_t {
        faces.push(vec![0, idx(1, m), idx(1, m + 1)]);
    }
    for j in 1..n_r {
        for m in 0..n_t {
            faces.push(vec![idx(j, m), idx(j + 1, m), idx(j + 1, m + 1), idx(j, m + 1)]);
        }
    }
    (nodes, faces)
}

fn mesh_of(ka: f64) -> (usize, usize) {
    if ka == 10.0 {
        (16, 100)
    } else {
        (4, 64)
    }
}

fn surface(n_r: usize, n_t: usize, vel: impl Fn(Vec3) -> f64) -> VibratingSurface {
    let (nodes, faces) = disk(A, n_r, n_t);
    let v = nodes
        .iter()
        .map(|&x| [Phasor::default(), Phasor::default(), Phasor::new(vel(x), 0.0)])
        .collect();
    VibratingSurface::new(nodes, faces, v).unwrap()
}

fn plane(n_r: usize, n_t: usize) -> BaffledPlane {
    BaffledPlane::new(&surface(n_r, n_t, |_| V), &RayleighOptions::new()).unwrap()
}

fn omega_of(ka: f64) -> f64 {
    ka / A * C
}

fn r1(ka: f64) -> f64 {
    1.0 - j1(2.0 * ka) / ka
}

fn x1(ka: f64) -> f64 {
    h1(2.0 * ka) / ka
}

// ------------------------------------------------------------------------------------------------------------ A

fn rect_corner(a: f64, b: f64) -> f64 {
    let d = (a * a + b * b).sqrt();
    a * ((b + d) / a).ln() + b * ((a + d) / b).ln()
}

#[test]
#[ignore = "ADR 0488 amendment 1: NaN-blind f64::max aggregation (MU7 not caught); continued in sa_radiation_impedance_amend1.rs"]
fn a1_a3_analytic_polygon_integrals() {
    let mut c = Checks::new();
    let l = 0.37;
    let sq: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [l, 0.0, 0.0], [l, l, 0.0], [0.0, l, 0.0]];
    let ln = (1.0 + 2f64.sqrt()).ln();
    let tri: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [l, 0.0, 0.0], [0.0, l, 0.0]];
    let cases = [
        ("square centre", polygon_inverse_distance(&sq, [l / 2.0, l / 2.0, 0.0]).unwrap(), 4.0 * l * ln),
        ("square corner", polygon_inverse_distance(&sq, [0.0, 0.0, 0.0]).unwrap(), 2.0 * l * ln),
        (
            "square mid-edge",
            polygon_inverse_distance(&sq, [l / 2.0, 0.0, 0.0]).unwrap(),
            2.0 * rect_corner(l / 2.0, l),
        ),
        (
            "right triangle at the right angle",
            polygon_inverse_distance(&tri, [0.0, 0.0, 0.0]).unwrap(),
            2f64.sqrt() * l * ln,
        ),
    ];
    let mut worst: f64 = 0.0;
    for (name, v, cf) in cases {
        println!("A1 {name}: {v:.16e} closed {cf:.16e} rel {:.3e}", rel(v, cf));
        worst = worst.max(rel(v, cf));
    }
    c.at_most("A1 largest rel vs the closed forms", worst, 1e-13);

    // A2: outside point vs 24 x 24 Gauss; general quadrilateral, inner point, vs Duffy 40 x 40 on four triangles
    let xo = [2.0 * l, 0.5 * l, 0.0];
    let g24 = gauss(24);
    let mut s = 0.0;
    for &(gu, wu) in &g24 {
        for &(gv, wv) in &g24 {
            let y = [(gu + 1.0) * l / 2.0, (gv + 1.0) * l / 2.0, 0.0];
            let r = ((y[0] - xo[0]).powi(2) + (y[1] - xo[1]).powi(2)).sqrt();
            s += wu * wv * (l / 2.0) * (l / 2.0) / r;
        }
    }
    let vo = polygon_inverse_distance(&sq, xo).unwrap();
    println!("A2 outside: {vo:.16e} Gauss {s:.16e} rel {:.3e}", rel(vo, s));
    let quad: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [0.3, -0.05, 0.0], [0.35, 0.25, 0.0], [-0.02, 0.2, 0.0]];
    let xi = [0.1, 0.07, 0.0];
    let g40 = gauss(40);
    let mut sd = 0.0;
    for i in 0..4 {
        let (p1, p2) = (quad[i], quad[(i + 1) % 4]);
        let e1 = [p1[0] - xi[0], p1[1] - xi[1]];
        let e2 = [p2[0] - xi[0], p2[1] - xi[1]];
        let cr = (e1[0] * e2[1] - e1[1] * e2[0]).abs();
        for &(gu, wu) in &g40 {
            let uu = (gu + 1.0) / 2.0;
            for &(gv, wv) in &g40 {
                let vv = (gv + 1.0) / 2.0;
                let y = [
                    uu * ((1.0 - vv) * e1[0] + vv * e2[0]),
                    uu * ((1.0 - vv) * e1[1] + vv * e2[1]),
                ];
                let r = (y[0] * y[0] + y[1] * y[1]).sqrt();
                sd += wu * wv / 4.0 * uu * cr / r;
            }
        }
    }
    let vq = polygon_inverse_distance(&quad, xi).unwrap();
    println!("A2 general quadrilateral: {vq:.16e} Duffy {sd:.16e} rel {:.3e}", rel(vq, sd));
    c.at_most("A2 largest rel vs quadrature", rel(vo, s).max(rel(vq, sd)), 1e-11);

    // A3: orientation and the collapsed triangle
    let mut rev = sq.clone();
    rev.reverse();
    let x = [l / 2.0, l / 2.0, 0.0];
    let vr = polygon_inverse_distance(&rev, x).unwrap();
    let vf = polygon_inverse_distance(&sq, x).unwrap();
    c.at_most("A3 reversed vs forward", rel(vr, vf), 1e-13);
    let mut tq = tri.clone();
    tq.push(tri[2]);
    let a = polygon_inverse_distance(&tri, [0.0, 0.0, 0.0]).unwrap();
    let b = polygon_inverse_distance(&tq, [0.0, 0.0, 0.0]).unwrap();
    c.holds(
        "A3 triangle as a quadrilateral with its last node repeated",
        a.to_bits() == b.to_bits(),
        &format!("{a:e} vs {b:e} (bits)"),
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ G1, S1, S2

#[test]
#[ignore = "ADR 0488 amendment 1: NaN-blind f64::max aggregation (MU7 not caught); continued in sa_radiation_impedance_amend1.rs"]
fn g1_disk_potential() {
    let mut c = Checks::new();
    let mut worst: f64 = 0.0;
    for (n_r, n_t, js) in [(4usize, 64usize, vec![0usize, 1, 2, 3, 4]), (16, 100, vec![0, 4, 8, 12, 16])] {
        let p = plane(n_r, n_t);
        for j in js {
            // the node on phi = 0 at ring j: face 0 corner (-1, -1) for the centre, else a quadrilateral or triangle corner
            let (face, u, v) = if j == 0 {
                (0, -1.0, -1.0)
            } else if j == 1 {
                (0, 1.0, -1.0)
            } else {
                // quadrilateral between rings j - 1 and j at m = 0: corners (j-1, 0), (j, 0), (j, 1), (j-1, 1)
                (n_t * (j - 1), 1.0, -1.0)
            };
            let got = p.inverse_distance_potential(face, u, v).unwrap();
            let cf = 4.0 * A * ellip_e(j as f64 / n_r as f64);
            println!("G1 {n_r} x {n_t} r = {j}/{n_r} a: {got:.12e} 4 a E {cf:.12e} rel {:.3e}", rel(got, cf));
            worst = worst.max(rel(got, cf));
        }
    }
    c.at_most("G1 largest rel vs 4 a E(r/a)", worst, 5e-3);
    c.finish();
}

#[test]
#[ignore = "ADR 0488 amendment 1: NaN-blind f64::max aggregation (MU7 not caught); continued in sa_radiation_impedance_amend1.rs"]
fn s1_s2_surface_pressure() {
    let mut c = Checks::new();
    let mut worst: f64 = 0.0;
    for ka in [0.1, 0.5, 2.0, 10.0] {
        let (n_r, n_t) = mesh_of(ka);
        let p = plane(n_r, n_t).surface_pressure(0, -1.0, -1.0, RHO, C, omega_of(ka)).unwrap();
        let cf = Phasor::new(RHO * C * V * (1.0 - ka.cos()), RHO * C * V * ka.sin());
        let e = Phasor::new(p.re - cf.re, p.im - cf.im).abs() / cf.abs();
        println!("S1 ka={ka}: P ({:.10e}, {:.10e}) closed ({:.10e}, {:.10e}) {e:.3e}", p.re, p.im, cf.re, cf.im);
        worst = worst.max(e);
    }
    c.at_most("S1 centre pressure vs rho c V (1 - e^{-i k a})", worst, 5e-3);

    // S2: non-uniform velocity V (1 + x/a), k a = 2, against the F-418 pressure at height h with the Taylor step
    let ka = 2.0;
    let omega = omega_of(ka);
    let s = surface(4, 64, |x| V * (1.0 + x[0] / A));
    let bp = BaffledPlane::new(&s, &RayleighOptions::new()).unwrap();
    let h = A / 100.0;
    let mut worst = 0.0f64;
    let mut worst_nc = f64::INFINITY;
    for (node, face, u, v) in [(0usize, 0usize, -1.0, -1.0), (65, 64, 1.0, -1.0)] {
        let x = s.nodes()[node];
        let vn = s.velocity()[node][2];
        let ps = bp.surface_pressure(face, u, v, RHO, C, omega).unwrap();
        let ph = bp.pressure([x[0], x[1], x[2] + h], RHO, C, omega).unwrap();
        // P_s - i omega rho V_n h
        let pred = Phasor::new(ps.re + omega * RHO * vn.im * h, ps.im - omega * RHO * vn.re * h);
        let e = Phasor::new(ph.re - pred.re, ph.im - pred.im).abs() / ps.abs();
        let e_nc = Phasor::new(ph.re - ps.re, ph.im - ps.im).abs() / ps.abs();
        println!("S2 node {node} (face {face} at ({u}, {v})): {e:.3e}; without the Taylor step {e_nc:.3e}");
        worst = worst.max(e);
        worst_nc = worst_nc.min(e_nc);
    }
    c.at_most("S2 surface pressure (non-uniform velocity) vs the F-418 pressure at h = a/100", worst, 1e-3);
    c.above("NC3 S2 without the Taylor step (smallest)", worst_nc, 5e-3);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ Z, M, H, T

#[test]
fn z1_z3_m1_t1_piston_impedance() {
    let mut c = Checks::new();
    let s_disk = PI * A * A;
    for ka in [0.1, 0.5, 2.0, 10.0] {
        let (n_r, n_t) = mesh_of(ka);
        let p = plane(n_r, n_t);
        let omega = omega_of(ka);
        let t0 = std::time::Instant::now();
        let z = p.radiation_impedance(RHO, C, omega).unwrap();
        let t = t0.elapsed().as_secs_f64();
        let w = p.power(RHO, C, omega).unwrap();
        let zr = z.impedance.re / (RHO * C * s_disk);
        let zx = z.impedance.im / (RHO * C * s_disk);
        println!(
            "ka={ka} ({n_r} x {n_t}, {} points): Z/(rho c pi a^2) = ({zr:.10}, {zx:.10}), closed ({:.10}, {:.10}), {t:.2} s",
            p.quadrature_points().len(),
            r1(ka),
            x1(ka)
        );
        if ka == 10.0 {
            println!("T1 ka=10 impedance wall time {t:.2} s (printed; heavy record above 120 s)");
        }
        c.at_most(&format!("Z1 ka={ka} resistance"), rel(zr, r1(ka)), 1e-2);
        c.at_most(&format!("Z2 ka={ka} reactance"), rel(zx, x1(ka)), 1e-2);
        c.at_most(
            &format!("Z3 ka={ka} Re Z vs 2 W/V^2 of F-418"),
            rel(z.impedance.re, 2.0 * w.power / (V * V)),
            1e-6,
        );
        if ka == 0.1 {
            c.at_most("M1 ka=0.1 added mass vs 8 rho a^3/3", rel(z.added_mass, 8.0 * RHO * A.powi(3) / 3.0), 2e-2);
        }
    }
    c.finish();
}

#[test]
fn h1_nc1_nc2_order_and_negative_controls() {
    let mut c = Checks::new();
    // H1: faces reversed at k a = 2
    let s = surface(4, 64, |_| V);
    let omega = omega_of(2.0);
    let z = BaffledPlane::new(&s, &RayleighOptions::new())
        .unwrap()
        .radiation_impedance(RHO, C, omega)
        .unwrap()
        .impedance;
    let mut faces = s.faces().to_vec();
    faces.reverse();
    let rev = VibratingSurface::new(s.nodes().to_vec(), faces, s.velocity().to_vec()).unwrap();
    let zr = BaffledPlane::new(&rev, &RayleighOptions::new())
        .unwrap()
        .radiation_impedance(RHO, C, omega)
        .unwrap()
        .impedance;
    c.at_most(
        "H1 ka=2 faces reversed |Z_rev - Z|/|Z|",
        Phasor::new(zr.re - z.re, zr.im - z.im).abs() / z.abs(),
        1e-10,
    );
    // NC1: coarse mesh disk(a, 2, 16) at k a = 0.1
    let z = plane(2, 16).radiation_impedance(RHO, C, omega_of(0.1)).unwrap().impedance;
    c.above(
        "NC1 disk(a, 2, 16) ka=0.1 reactance rel",
        rel(z.im / (RHO * C * PI * A * A), x1(0.1)),
        1e-2,
    );
    // NC2: naive collocation, coincident point dropped, no analytic part
    for ka in [0.1, 2.0] {
        let (n_r, n_t) = mesh_of(ka);
        let p = plane(n_r, n_t);
        let k = ka / A;
        let q = p.quadrature_points();
        let mut sum = 0.0;
        for (i, &(xi, wi, _)) in q.iter().enumerate() {
            let mut row = 0.0;
            for (j, &(xj, wj, _)) in q.iter().enumerate() {
                if i != j {
                    let r = ((xi[0] - xj[0]).powi(2) + (xi[1] - xj[1]).powi(2) + (xi[2] - xj[2]).powi(2)).sqrt();
                    row += wj * (k * r).cos() / r;
                }
            }
            sum += wi * row;
        }
        let xn = k / (2.0 * PI * PI * A * A) * sum;
        c.above(&format!("NC2 ka={ka} naive collocation reactance rel"), rel(xn, x1(ka)), 1e-2);
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ N1

#[test]
fn n1_refusals() {
    let mut c = Checks::new();
    let p = plane(4, 64);
    let rest = BaffledPlane::new(&surface(4, 64, |_| 0.0), &RayleighOptions::new()).unwrap();
    let w = omega_of(2.0);
    let sq: Vec<Vec3> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    let cases: Vec<(&str, Result<(), String>)> = vec![
        ("polygon of 2 vertices", polygon_inverse_distance(&sq[..2], [0.5; 3]).map(|_| ())),
        (
            "polygon NaN vertex",
            polygon_inverse_distance(&[[f64::NAN, 0.0, 0.0], sq[1], sq[2]], [0.5; 3]).map(|_| ()),
        ),
        ("polygon NaN point", polygon_inverse_distance(&sq, [f64::NAN, 0.0, 0.0]).map(|_| ())),
        (
            "polygon collinear",
            polygon_inverse_distance(&[[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], [0.5; 3]).map(|_| ()),
        ),
        ("surface_pressure face out of range", p.surface_pressure(256, 0.0, 0.0, RHO, C, w).map(|_| ())),
        ("surface_pressure u > 1", p.surface_pressure(3, 1.5, 0.0, RHO, C, w).map(|_| ())),
        ("surface_pressure v < -1", p.surface_pressure(3, 0.0, -1.01, RHO, C, w).map(|_| ())),
        ("surface_pressure u NaN", p.surface_pressure(3, f64::NAN, 0.0, RHO, C, w).map(|_| ())),
        ("potential face out of range", p.inverse_distance_potential(999, 0.0, 0.0).map(|_| ())),
        ("potential v NaN", p.inverse_distance_potential(0, 0.0, f64::NAN).map(|_| ())),
        ("surface_pressure rho 0", p.surface_pressure(3, 0.0, 0.0, 0.0, C, w).map(|_| ())),
        ("surface_pressure c negative", p.surface_pressure(3, 0.0, 0.0, RHO, -1.0, w).map(|_| ())),
        ("surface_pressure omega 0", p.surface_pressure(3, 0.0, 0.0, RHO, C, 0.0).map(|_| ())),
        ("surface_pressures rho NaN", p.surface_pressures(f64::NAN, C, w).map(|_| ())),
        ("surface_pressures omega negative", p.surface_pressures(RHO, C, -w).map(|_| ())),
        ("radiation_impedance c 0", p.radiation_impedance(RHO, 0.0, w).map(|_| ())),
        ("radiation_impedance omega NaN", p.radiation_impedance(RHO, C, f64::NAN).map(|_| ())),
        ("radiation_impedance at rest", rest.radiation_impedance(RHO, C, w).map(|_| ())),
    ];
    let n = cases.len();
    let mut refused = 0;
    for (name, r) in cases {
        match r {
            Err(e) => {
                println!("N1 {name}: refused: {e}");
                refused += 1;
            }
            Ok(()) => println!("N1 {name}: ACCEPTED"),
        }
    }
    c.holds("N1 refusals", refused == n, &format!("{refused} of {n}"));
    c.finish();
}
