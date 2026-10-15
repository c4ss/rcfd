//! ADR 0022 library checks L1 to L6 (limits registered in docs/adr/0022-bc-catalogue.md before this file was written).

use rcfd_fv::bc::{Bc, BcCtx, Directed, FixedValue, Profile, Switched, ZeroGradient};
use rcfd_fv::outlets::{
    area_mean_shift, outflow_speeds, radial_equilibrium, PressureUpdate, PressureUpdater, RadialReference,
};

fn ctx(n: [f64; 3], u: [f64; 3], face: usize) -> BcCtx {
    BcCtx {
        normal: n,
        dn: 0.1,
        local_face: face,
        cell_value: u,
        partner: None,
        is_vector: true,
    }
}

fn face_u(bc: &dyn Bc, c: &BcCtx) -> [f64; 3] {
    [0, 1, 2].map(|k| bc.coeffs(c, k).value(c.cell_value[k]))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[test]
fn l1_directed() {
    let s = 1.0 / 14.0_f64.sqrt();
    let n = [s, 2.0 * s, 3.0 * s];
    let u = [1.0, -0.5, 0.7];
    let c = ctx(n, u, 0);
    // dir = None: u_f = n (n.u_P)
    let uf = face_u(
        &Directed {
            dir: None,
            tangential: Profile::Uniform([0.0; 3]),
        },
        &c,
    );
    let un = dot(n, u);
    let worst = (0..3).map(|k| (uf[k] - n[k] * un).abs()).fold(0.0, f64::max);
    println!("L1 normal backflow max error {worst:.3e}");
    assert!(worst <= 1e-14, "{worst}");
    // direction e: same normal component, parallel to e
    let e = [0.3, -0.4, 0.8];
    let bc = Directed {
        dir: Some(e),
        tangential: Profile::Uniform([0.0; 3]),
    };
    let uf = face_u(&bc, &c);
    let x = cross(uf, e);
    let r = (dot(x, x)).sqrt() / dot(uf, uf).sqrt();
    println!(
        "L1 direction: n.u_f - n.u_P = {:.3e}, |u x e|/|u| = {r:.3e}",
        dot(n, uf) - un
    );
    assert!((dot(n, uf) - un).abs() <= 1e-14 && r <= 1e-14);
    // tangential velocity added: normal component unchanged
    let t = {
        let t0 = [0.5, 0.1, -0.2];
        let tn = dot(t0, n);
        [t0[0] - tn * n[0], t0[1] - tn * n[1], t0[2] - tn * n[2]]
    };
    let uf2 = face_u(
        &Directed {
            dir: Some(e),
            tangential: Profile::Uniform(t),
        },
        &c,
    );
    println!("L1 swirl: n.u_f change {:.3e}", dot(n, uf2) - un);
    assert!((dot(n, uf2) - un).abs() <= 1e-14);
}

#[test]
fn l2_switched_is_bitwise() {
    let on = FixedValue::vector([1.0, 2.0, 3.0]);
    let off = ZeroGradient;
    let sw = Switched {
        mask: vec![true, false],
        on: Box::new(on.clone()),
        off: Box::new(off),
    };
    for k in 0..3 {
        let a = sw.coeffs(&ctx([1.0, 0.0, 0.0], [4.0, 5.0, 6.0], 0), k);
        let b = on.coeffs(&ctx([1.0, 0.0, 0.0], [4.0, 5.0, 6.0], 0), k);
        assert_eq!(a, b);
        let a = sw.coeffs(&ctx([1.0, 0.0, 0.0], [4.0, 5.0, 6.0], 1), k);
        let b = off.coeffs(&ctx([1.0, 0.0, 0.0], [4.0, 5.0, 6.0], 1), k);
        assert_eq!(a, b);
    }
}

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}

#[test]
fn l3_outflow_scaling() {
    let mut seed = 7;
    for prevent in [false, true] {
        let n = 50;
        let area: Vec<f64> = (0..n).map(|_| 0.1 + lcg(&mut seed)).collect();
        // shape with negative entries but a positive integral
        let u_n: Vec<f64> = (0..n).map(|_| lcg(&mut seed) - 0.3).collect();
        let target = 0.37;
        let q = outflow_speeds(&u_n, &area, target, prevent);
        let flux: f64 = q.iter().zip(&area).map(|(q, a)| q * a).sum();
        println!(
            "L3 prevent {prevent}: flux error {:.3e}",
            (flux - target).abs() / target
        );
        assert!((flux - target).abs() <= 1e-13 * target);
        if prevent {
            assert!(q.iter().all(|&v| v >= 0.0));
        }
    }
    // degenerate shape falls back to uniform
    let q = outflow_speeds(&[-1.0, -2.0], &[1.0, 1.0], 3.0, true);
    assert!((q[0] - 1.5).abs() < 1e-15 && (q[1] - 1.5).abs() < 1e-15);
}

#[test]
fn l4_area_mean_shift() {
    let mut seed = 99;
    let area: Vec<f64> = (0..80).map(|_| 0.01 + lcg(&mut seed)).collect();
    let p: Vec<f64> = (0..80).map(|_| 5.0 * (lcg(&mut seed) - 0.5)).collect();
    let out = area_mean_shift(&p, &area, 1.7);
    let tot: f64 = area.iter().sum();
    let mean: f64 = out.iter().zip(&area).map(|(p, a)| p * a).sum::<f64>() / tot;
    let pmax = p.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    println!("L4 mean error {:.3e}", (mean - 1.7).abs() / pmax);
    assert!((mean - 1.7).abs() <= 1e-12 * pmax);
}

#[test]
fn l5_radial_integration() {
    // solid body on non-uniform radii in [0.5, 1], deliberately unsorted
    let omega = 2.0;
    let mut seed = 5;
    let mut r: Vec<f64> = (0..40).map(|_| 0.5 + 0.5 * lcg(&mut seed)).collect();
    r[0] = 0.5;
    r[39] = 1.0;
    let w: Vec<f64> = r.iter().map(|r| omega * r).collect();
    let area = vec![1.0; r.len()];
    let p = radial_equilibrium(&r, &w, &area, RadialReference::Hub(0.0));
    let range = omega * omega * (1.0 - 0.25) / 2.0;
    let worst = r
        .iter()
        .zip(&p)
        .map(|(r, p)| (p - omega * omega * (r * r - 0.25) / 2.0).abs())
        .fold(0.0, f64::max);
    println!("L5 solid body error/range {:.3e}", worst / range);
    assert!(worst <= 1e-12 * range);
    // free vortex w = Gamma/r on 200 uniform radii
    let gamma = 0.7;
    let r: Vec<f64> = (0..200).map(|i| 0.5 + 0.5 * i as f64 / 199.0).collect();
    let w: Vec<f64> = r.iter().map(|r| gamma / r).collect();
    let area = vec![1.0; r.len()];
    let p = radial_equilibrium(&r, &w, &area, RadialReference::Hub(-gamma * gamma / (2.0 * 0.25)));
    let range = gamma * gamma / 2.0 * (1.0 / 0.25 - 1.0);
    let worst = r
        .iter()
        .zip(&p)
        .map(|(r, p)| (p + gamma * gamma / (2.0 * r * r)).abs())
        .fold(0.0, f64::max);
    println!("L5 free vortex error/range {:.3e}", worst / range);
    assert!(worst <= 1e-4 * range);
    // area-mean reference
    let area: Vec<f64> = r.iter().map(|r| 2.0 * std::f64::consts::PI * r).collect();
    let p = radial_equilibrium(&r, &w, &area, RadialReference::AreaMean(1.25));
    let tot: f64 = area.iter().sum();
    let mean: f64 = p.iter().zip(&area).map(|(p, a)| p * a).sum::<f64>() / tot;
    println!("L5 area mean error {:.3e}", (mean - 1.25).abs());
    assert!((mean - 1.25).abs() <= 1e-12 * range);
}

#[test]
fn l6_update_rules() {
    // Bernoulli: p + r e U_ref/(rho A)
    let mut b = PressureUpdater::new(PressureUpdate::Bernoulli, 0.5);
    let (p, m, t, rho, a) = (3.0, 0.12, 0.1, 1.2, 0.05);
    let u_ref = 0.12 / (rho * a);
    let expect = p + 0.5 * (m - t) * u_ref / (rho * a);
    let got = b.step(p, m, t, rho, a);
    assert!((got - expect).abs() <= 1e-14 * expect.abs(), "{got} {expect}");
    // secant on a linear m(p) = m0 - s_inv p: the root in one further step with r = 1
    let mut sec = PressureUpdater::new(PressureUpdate::Secant, 1.0);
    let (rho, a, t) = (1.0, 0.1, 0.1);
    let m_of = |p: f64| 0.5 - 0.04 * p;
    let p_root = (0.5 - t) / 0.04;
    let p0 = 1.0;
    let p1 = sec.step(p0, m_of(p0), t, rho, a);
    let p2 = sec.step(p1, m_of(p1), t, rho, a);
    println!("L6 secant: p2 = {p2}, root {p_root}");
    assert!((p2 - p_root).abs() <= 1e-12 * p_root, "{p2} {p_root}");
}
