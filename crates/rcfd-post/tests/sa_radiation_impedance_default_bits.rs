//! ADR 0488 (F-551) DB1: digests of the existing `rcfd_post::solid_acoustic` results (ERP, Rayleigh pressure, far field,
//! directivity, radiated power, far-field power) on the piston of ADR 0468, printed at fb7c5d94 before any library code of
//! F-551 and kept as constants (every existing result keeps its bits).

use rcfd_core::vec3::Vec3;
use rcfd_post::solid_acoustic::{equivalent_radiated_power, BaffledPlane, Phasor, RayleighOptions, VibratingSurface};
use std::f64::consts::PI;

const RHO: f64 = 1.2;
const C: f64 = 340.0;
const A: f64 = 0.1;
const V: f64 = 1e-3;

fn fnv(acc: &mut u64, bits: u64) {
    for b in bits.to_le_bytes() {
        *acc ^= b as u64;
        *acc = acc.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn digest(values: &[f64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, values.len() as u64);
    for v in values {
        fnv(&mut h, v.to_bits());
    }
    h
}

/// The polar disk mesh of ADR 0468 with a velocity `V (1 + x/a)` along z (uniform plus rocking).
fn piston(n_r: usize, n_t: usize) -> VibratingSurface {
    let mut nodes: Vec<Vec3> = vec![[0.0, 0.0, 0.0]];
    for j in 1..=n_r {
        for m in 0..n_t {
            let t = 2.0 * PI * m as f64 / n_t as f64;
            let r = A * j as f64 / n_r as f64;
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
    let vel = nodes
        .iter()
        .map(|x| [Phasor::default(), Phasor::default(), Phasor::new(V * (1.0 + x[0] / A), 0.3 * V)])
        .collect();
    VibratingSurface::new(nodes, faces, vel).unwrap()
}

fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    let s = piston(4, 64);
    let omega = 2.0 / A * C;
    let e = equivalent_radiated_power(&s, RHO, C).unwrap();
    out.push(("erp", digest(&[e.power, e.area, e.level_db.unwrap()])));
    let plane = BaffledPlane::new(&s, &RayleighOptions::new()).unwrap();
    let mut v = vec![plane.deviation(), plane.diameter()];
    v.extend(plane.normal());
    v.extend(plane.origin());
    for z in [0.003, 0.05, 0.1, 0.4, 2.0] {
        let p = plane.pressure([0.02, -0.01, z], RHO, C, omega).unwrap();
        v.extend([p.re, p.im]);
    }
    out.push(("pressure", digest(&v)));
    let mut v = Vec::new();
    for deg in [0.0, 10.0, 35.0, 60.0, 89.0] {
        let t: f64 = deg * PI / 180.0;
        let d = [t.sin() * 0.6, t.sin() * 0.8, t.cos()];
        let f = plane.far_field(d, RHO, C, omega).unwrap();
        v.extend([f.re, f.im, plane.directivity(d, C, omega).unwrap()]);
    }
    out.push(("far_field_directivity", digest(&v)));
    let mut v = Vec::new();
    for ka in [0.1, 0.5, 2.0] {
        let w = plane.power(RHO, C, ka / A * C).unwrap();
        v.extend([w.power, w.erp, w.efficiency, w.level_db.unwrap()]);
    }
    v.push(plane.power_from_far_field(RHO, C, omega, 16, 32).unwrap());
    out.push(("power", digest(&v)));
    out
}

/// Printed at fb7c5d94 by this file before any library code of F-551.
const BASE: [(&str, u64); 4] = [
    ("erp", 0x8040632849bc163a),
    ("pressure", 0x89a6d1c8fa5e75d5),
    ("far_field_directivity", 0x7c221730c38021d5),
    ("power", 0x72f0e68d889ff101),
];

#[test]
fn db1_solid_acoustic_results_unchanged() {
    let got = digests();
    let mut equal = 0;
    for ((name, d), (bname, bd)) in got.iter().zip(BASE.iter()) {
        println!("DB1 {name} {d:#018x}");
        assert_eq!(name, bname);
        if d == bd {
            equal += 1;
        }
    }
    println!("DB1 {equal} of {} digests equal to the base", BASE.len());
    assert_eq!(equal, BASE.len());
}
