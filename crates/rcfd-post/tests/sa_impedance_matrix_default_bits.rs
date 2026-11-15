//! ADR 0538 (F-579) DB1: digests of the F-551 results of `rcfd_post::solid_acoustic` (surface pressures, a surface pressure
//! inside a face, the inverse-distance potential, the radiation impedance) and the F-418 power on the piston of ADR 0468 with
//! a complex velocity, printed at dfd2720d before any library code of F-579 and kept as constants.

use rcfd_core::vec3::Vec3;
use rcfd_post::solid_acoustic::{BaffledPlane, Phasor, RayleighOptions, VibratingSurface};
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
        .map(|x| {
            [
                Phasor::default(),
                Phasor::default(),
                Phasor::new(V * (1.0 + x[0] / A), 0.3 * V),
            ]
        })
        .collect();
    VibratingSurface::new(nodes, faces, vel).unwrap()
}

fn digests() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    let plane = BaffledPlane::new(&piston(4, 64), &RayleighOptions::new()).unwrap();
    for (name, ka) in [("ka 0.5", 0.5), ("ka 2", 2.0)] {
        let omega = ka / A * C;
        let mut v = Vec::new();
        for p in plane.surface_pressures(RHO, C, omega).unwrap() {
            v.extend([p.re, p.im]);
        }
        for (f, u, w) in [(0, -1.0, -1.0), (70, 0.3, -0.6), (200, 1.0, 0.25)] {
            let p = plane.surface_pressure(f, u, w, RHO, C, omega).unwrap();
            v.extend([p.re, p.im, plane.inverse_distance_potential(f, u, w).unwrap()]);
        }
        let z = plane.radiation_impedance(RHO, C, omega).unwrap();
        v.extend([
            z.complex_power.re,
            z.complex_power.im,
            z.area,
            z.mean_square_velocity,
            z.impedance.re,
            z.impedance.im,
            z.normalised.re,
            z.normalised.im,
            z.added_mass,
        ]);
        let w = plane.power(RHO, C, omega).unwrap();
        v.extend([w.power, w.erp, w.efficiency]);
        out.push((name, digest(&v)));
    }
    out
}

/// Printed at dfd2720d by this file before any library code of F-579.
const BASE: [(&str, u64); 2] = [("ka 0.5", 0x93b9a61426972624), ("ka 2", 0x68c7ce3c2b6648cc)];

#[test]
fn db1_f551_results_unchanged() {
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
