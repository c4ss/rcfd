//! F-664 (ADR 0587): check N4 (record of D5), the eigenvalues and modes of ADR 0580 keep their bits: every evaluation of
//! the Beck bisection by `tangent_modes_full` and the two problems of `buckling_factors_full` (dead load, 2 factors; the
//! follower, 4 factors). Uses only API that existed at 847c5881; the expected digests were printed by this file run on the
//! unchanged base 847c5881 (`f664-scratch/n4-base.log`) and written into `EXPECTED` before any library code of F-664.

mod f664_beck;

use f664_beck::{bisection, p_ref, setup};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::follower::{buckling_factors_full, FollowerPressure};
use rcfd_linalg::complex::C64;

/// Digests of 847c5881 in the order of the run; `None` prints them.
const EXPECTED: Option<[(&str, u64); 3]> = Some([
    ("tangent_modes_full bisection", 0xf1b6_a13c_7edf_fece),
    ("buckling_factors_full dead", 0xa8df_ec8d_66ec_8014),
    ("buckling_factors_full follower", 0xcf98_1164_b8c4_4b28),
]);

fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

fn push(v: &mut Vec<f64>, values: &[C64], modes: &[Vec<C64>], residuals: &[f64], steps: usize) {
    for z in values {
        v.push(z.re);
        v.push(z.im);
    }
    for x in modes {
        for z in x {
            v.push(z.re);
            v.push(z.im);
        }
    }
    v.extend_from_slice(residuals);
    v.push(steps as f64);
}

#[test]
fn n4_adr0580_bits() {
    let b = setup();
    let fp = FollowerPressure::new(&b.model, "xmax", p_ref()).unwrap();
    let mut got: Vec<(&str, u64)> = Vec::new();
    let runs = bisection(&b, &fp);
    let mut v = Vec::new();
    for (p, r) in &runs {
        v.push(*p);
        push(&mut v, &r.eigenvalues, &r.modes, &r.residuals, r.steps);
    }
    println!(
        "bisection: {} evaluations, last p = {:.12}",
        runs.len(),
        runs.last().unwrap().0
    );
    got.push(("tangent_modes_full bisection", digest(&v)));
    let fpd = FollowerPressure::new(&b.dead, "xmax", p_ref()).unwrap();
    let dead = buckling_factors_full(&b.dead, &b.pre, &[], &BucklingOptions::new(2)).unwrap();
    let foll = buckling_factors_full(&b.dead, &b.pre, &[&fpd], &BucklingOptions::new(4)).unwrap();
    for (name, r) in [
        ("buckling_factors_full dead", &dead),
        ("buckling_factors_full follower", &foll),
    ] {
        let mut v = Vec::new();
        push(&mut v, &r.factors, &r.modes, &r.residuals, r.steps);
        println!("{name}: factors {:?}", r.factors);
        got.push((name, digest(&v)));
    }
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    match EXPECTED {
        Some(e) => assert_eq!(got, e.to_vec()),
        None => panic!("no expected digests yet (the base run prints them)"),
    }
}
