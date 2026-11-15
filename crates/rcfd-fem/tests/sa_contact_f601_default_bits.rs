//! F-601 (ADR 0540 D8): B, the default-bits record of the contact module before the bucket search, the rigid curved targets
//! and the new faces. The digests were printed by this file run on 4d0d0f1b plus ADR 0540 (`f601-scratch/bits-base.log`) and
//! written into `EXPECTED` before any library code of F-601. G1 is ADR 0517's hub at `n = 4` solved; G2 the hub at `n = 8`
//! evaluated at the 20 states of ADR 0540 D5 with both reaches; G3 ADR 0507 T2's punch (total Lagrangian); G4 ADR 0507 B1's
//! blocks with a search distance; G5 ADR 0507 T1's warped pair at the 20 states.

mod f601_common;

use f601_common::*;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactSet, Kinematics, Target};
use rcfd_fem::nonlinear::NewtonOptions;
use rcfd_fem::Load;

/// Digests of 4d0d0f1b plus ADR 0540 (unchanged library) in the order of the run, from `f601-scratch/bits-base.log`.
const EXPECTED: Option<[(&str, u64); 5]> = Some([
    ("G1 hub n = 4", 0x66c6_f6c4_b5aa_10c9),
    ("G2 hub n = 8 states", 0x7b6a_7148_73e6_6ac0),
    ("G3 punch, total Lagrangian", 0x9ce8_7ed9_ab4e_0a30),
    ("G4 blocks, search distance", 0xff38_dddb_945e_d36d),
    ("G5 T1 pair states", 0xc5db_d098_367e_7c4f),
]);

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

fn evaluate_states(m: &rcfd_fem::SolidModel, pair: &ContactPair, amp: f64, reach: Option<f64>) -> Vec<u64> {
    let mut o = ContactOptions::new(1, 0.0);
    if let Some(d) = reach {
        o = o.search_distance(d);
    }
    let set = ContactSet::new(m, std::slice::from_ref(pair), &o).unwrap();
    let n = m.dof_map().n_equations();
    let mut w = Vec::new();
    for seed in 0..20u64 {
        let u = state(n, amp, seed);
        for v in set.evaluate(&u).unwrap() {
            node_words(&v, &mut w);
        }
    }
    w
}

#[test]
fn b_contact_default_bits_f601() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    // G1: the hub at n = 4, ADR 0517's options.
    let m = hub_model(4);
    let h = hub_h(4);
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E_HUB / h);
    let r = solve(
        &m,
        &[],
        &[],
        &[pair],
        &ContactOptions::new(1, 1e-6 * hub_d()).newton(newton()),
    )
    .unwrap();
    got.push(("G1 hub n = 4", digest(&result_words(&r))));
    // G2: the hub at n = 8 at the states, both reaches.
    let m = hub_model(8);
    let h = hub_h(8);
    let pair = ContactPair::new("bore", Target::Surface("shaft_out".into()), 100.0 * E_HUB / h);
    let mut w = evaluate_states(&m, &pair, 0.3 * h, None);
    w.extend(evaluate_states(&m, &pair, 0.3 * h, Some(0.1 * h)));
    got.push(("G2 hub n = 8 states", digest(&w)));
    // G3: the punch, total Lagrangian.
    let m = punch_model(0.01);
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    let o = ContactOptions::new(2, 1e-8)
        .newton(newton())
        .kinematics(Kinematics::TotalLagrangian);
    let r = solve(&m, &[], &[], &[pair], &o).unwrap();
    got.push(("G3 punch, total Lagrangian", digest(&result_words(&r))));
    // G4: the blocks pressed with a search distance.
    let mut m = blocks(0.0);
    m.add_load(Load::pressure("upper.zmax", 1.0e8)).unwrap();
    let pair = ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5);
    let o = ContactOptions::new(2, 1e-12).newton(newton()).search_distance(0.01);
    let r = solve(&m, &[], &[], &[pair], &o).unwrap();
    got.push(("G4 blocks, search distance", digest(&result_words(&r))));
    // G5: T1's warped pair at the states.
    let m = two_hex(T1_CONTACTOR);
    let pair = ContactPair::new("contactor", Target::Surface("target".into()), 10.0);
    let mut w = evaluate_states(&m, &pair, 0.05, None);
    w.extend(evaluate_states(&m, &pair, 0.05, Some(0.02)));
    got.push(("G5 T1 pair states", digest(&w)));
    for (name, d) in &got {
        println!("{name}: 0x{d:016x}");
    }
    let expected = EXPECTED.expect("the digests of the base are written into EXPECTED before the library code");
    for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(*d, *ed, "{name}: 0x{d:016x} against the base 0x{ed:016x}");
    }
}
