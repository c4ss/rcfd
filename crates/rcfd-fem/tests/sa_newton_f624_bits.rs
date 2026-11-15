//! F-624 (ADR 0566) check B, records G3 to G8: the bits of the Newton driver with the supernodal factor (and, as controls,
//! with the present envelope factor) on ADR 0542's total Lagrangian hub and cantilever, recorded on b29c1383 before any
//! library code of F-624 ("without the reuse") and asserted unchanged after it (ADR 0566 D1, D2).

mod f624_common;

use f624_common::*;
use rcfd_fem::contact::solve;
use rcfd_fem::error::FemError;
use rcfd_fem::nonlinear::{solve as nl_solve, solve_with as nl_solve_with, DriverExtensions, NewtonOptions};
use rcfd_fem::solve::DirectFactor;
use std::cell::RefCell;

/// FNV-1a 64 over bytes.
fn digest_bytes(b: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn hub_flat(n: usize, d: f64, route: Route) -> Result<u64, FemError> {
    let s = Setup { n, d, ..BASE };
    let fit = fit_model(s);
    let (pair, o) = options(&s, route);
    solve(&fit.model, &[], &[], &[pair], &o).map(|r| digest(&flat(&r)))
}

/// ADR 0542 D5's device on the cantilever with `factor`: per increment (step, load factor, halvings, iterations) and the
/// displacement.
fn cantilever_device(factor: DirectFactor) -> u64 {
    let (m, eq) = cantilever();
    let plain = nl_solve(&m, &[], &[], &NewtonOptions::new(2)).unwrap();
    let tau = 0.35 * plain.result.displacement_values()[eq];
    let kick = 10.0 * rcfd_fem::assemble::stiffness(&m).unwrap().diag()[eq];
    let dev = Spoiler {
        eq,
        tau,
        kick,
        start: RefCell::new(None),
    };
    let r = nl_solve_with(
        &m,
        &[],
        &[],
        &NewtonOptions::new(2).factor(factor),
        DriverExtensions {
            contributions: vec![&dev],
            ..Default::default()
        },
    )
    .unwrap();
    let mut v: Vec<f64> = increments(&r)
        .iter()
        .flat_map(|&(s, l, h, i)| [s as f64, f64::from_bits(l), h as f64, i as f64])
        .collect();
    v.extend_from_slice(r.result.displacement_values());
    digest(&v)
}

/// The plain cantilever with the supernodal factor: displacement and reactions.
fn cantilever_plain_supernodal() -> u64 {
    let (m, _) = cantilever();
    let r = nl_solve(
        &m,
        &[],
        &[],
        &NewtonOptions::new(2).factor(DirectFactor::SupernodalNestedDissection),
    )
    .unwrap();
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap_or(&[]));
    digest(&v)
}

/// G3 to G8 of ADR 0566 check B.
#[test]
fn f624_b_g3_to_g8_newton_bits() {
    let g3 = hub_flat(8, D_TL, Route::Contact).unwrap();
    let g4 = cantilever_device(DirectFactor::SupernodalNestedDissection);
    let g5 = cantilever_plain_supernodal();
    let e = hub_flat(4, 1e-3 * A, Route::Contact).unwrap_err().to_string();
    let g6 = digest_bytes(e.as_bytes());
    let g7 = hub_flat(8, D_TL, Route::Present).unwrap();
    let g8 = cantilever_device(DirectFactor::Envelope);
    println!("G6 text: {e}");
    let got = [g3, g4, g5, g6, g7, g8];
    for (k, g) in got.iter().enumerate() {
        println!("G{}: {g:#018x}", k + 3);
    }
    assert_eq!(got, RECORD, "G3 to G8");
}

/// Recorded on b29c1383 (`f624-scratch/bits-base-fem.log`).
const RECORD: [u64; 6] = [
    0x43fa_d375_e5cc_2654,
    0xec34_8445_02a5_f123,
    0x7141_2901_3681_f33a,
    0x88d9_dee6_a05f_54e5,
    0xf5ed_93c6_1c21_db4e,
    0xcbf5_ab0d_7bef_f7aa,
];
