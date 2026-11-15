//! F-664 (ADR 0587): check N5 (record of D5), `nearest_pencil_eigenpairs` keeps its bits on the pencils K1, K2 (of ADR
//! 0580) and K5c: values, vectors, residuals, estimates, steps. Uses only API that existed at 847c5881; the expected digests
//! were printed by this file run on the unchanged base 847c5881 (`f664-scratch/n5-base.log`) and written into `EXPECTED`
//! before any library code of F-664.

mod f664_pencils;

use rcfd_linalg::eigen_pencil::{nearest_pencil_eigenpairs, PencilOptions};

/// Digests of 847c5881 in the order of the run; `None` prints them.
const EXPECTED: Option<[(&str, u64); 3]> = Some([
    ("K1", 0x5869_799e_c6d9_7157),
    ("K2", 0x1bff_32ea_c30c_26c1),
    ("K5c", 0xab53_693d_3af8_2471),
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

#[test]
fn n5_pencil_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    for p in f664_pencils::all() {
        let r = nearest_pencil_eigenpairs(&p.a, &p.b, p.count, &PencilOptions::new().shift(p.shift)).unwrap();
        let mut v = Vec::new();
        for z in &r.values {
            v.push(z.re);
            v.push(z.im);
        }
        for x in &r.vectors {
            for z in x {
                v.push(z.re);
                v.push(z.im);
            }
        }
        v.extend_from_slice(&r.residuals);
        v.extend_from_slice(&r.estimates);
        v.push(r.steps as f64);
        println!(
            "{}: {} values, {} steps, values {:?}",
            p.name,
            r.values.len(),
            r.steps,
            r.values
        );
        got.push((p.name, digest(&v)));
    }
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    match EXPECTED {
        Some(e) => assert_eq!(got, e.to_vec()),
        None => panic!("no expected digests yet (the base run prints them)"),
    }
}
