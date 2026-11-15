//! F-623 (ADR 0551 D7): the bit record B2 of the default (Gauss-point, ADR 0540 D3) contactor of Hex20 and Tet10 faces,
//! written and run on the base e906faa8 before any library code of F-623; the digests are those printed by that run.

mod f601_common;
mod f623_common;

use f601_common::*;
use f623_common::*;
use rcfd_fem::contact::{solve, ContactOptions, ContactPair, ContactSet, Target};
use rcfd_fem::nonlinear::{Contribution, NewtonOptions};
use rcfd_fem::SolidModel;

fn newton() -> NewtonOptions {
    NewtonOptions::new(1).max_iterations(30)
}

fn pt(kind: Kind) -> u64 {
    let m = patch_model(kind, 1.0e8);
    let pair = ContactPair::new("upper.zmin", Target::Surface("lower.zmax".into()), 100.0 * E_BL / 0.5);
    let r = solve(&m, &[], &[], &[pair], &ContactOptions::new(2, 1e-13).newton(newton())).unwrap();
    digest(&result_words(&r))
}

fn t1(m: &SolidModel) -> u64 {
    let pair = ContactPair::new("contactor", Target::Surface("target".into()), 10.0);
    let set = ContactSet::new(m, &[pair], &ContactOptions::new(1, 0.0)).unwrap();
    let lam: Vec<f64> = (0..set.nodes(0).len()).map(|k| 0.1 + 0.01 * k as f64).collect();
    set.set_multipliers(0, &lam).unwrap();
    let n = m.dof_map().n_equations();
    let mut words = Vec::new();
    for seed in 1..=20u64 {
        let u = state(n, 0.01, seed);
        for v in set.evaluate(&u).unwrap() {
            node_words(&v, &mut words);
        }
        if seed == 1 {
            let mut f = vec![0.0; n];
            let mut t = Vec::new();
            set.add(&u, 1.0, &mut f, &mut t).unwrap();
            words.extend(f.iter().map(|x| x.to_bits()));
            for (r, c, v) in t {
                words.extend([r as u64, c as u64, v.to_bits()]);
            }
        }
    }
    digest(&words)
}

fn q20() -> u64 {
    let m = q20_model();
    let pair = ContactPair::new("punch.zmin", Target::Surface("plate.zmax".into()), 800.0);
    let r = solve(&m, &[], &[], &[pair], &ContactOptions::new(2, 1e-8).newton(newton())).unwrap();
    digest(&result_words(&r))
}

#[test]
fn b2_default_gauss_point_contactor_bits() {
    let got = [
        ("G1 PT10", pt(Kind::Tet10)),
        ("G2 PT20", pt(Kind::Hex20)),
        ("G3 T1g20", t1(&hex20_contactor_model())),
        ("G3 T1g10", t1(&tet10_contactor_model())),
        ("G4 Q20", q20()),
    ];
    let expected: [u64; 5] = [
        0x85aa14aa58252b26,
        0x7e4db66afd3c0440,
        0xce16e985710f6e81,
        0x29c15a5c8f3fa5e5,
        0x3b8079edf9603135,
    ];
    for ((name, d), e) in got.iter().zip(expected) {
        println!("{name}: {d:#018x} (recorded {e:#018x})");
    }
    for ((name, d), e) in got.iter().zip(expected) {
        assert_eq!(*d, e, "{name}");
    }
}
