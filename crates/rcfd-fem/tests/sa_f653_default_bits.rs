//! F-653 (ADR 0580): check N1 (record of D6), the symmetric paths next to the new full-load-stiffness entry points give the
//! bits they gave before F-653: on Beck's column of D5 (a Hex20 cantilever with a follower pressure on its end face),
//! `follower::tangent_modes` with the follower contribution, `follower::buckling_factors` with the follower and
//! `buckling::factors`. Uses only API that existed at e348eb34; the expected digests were printed by this file run on the
//! unchanged base e348eb34 (`f653-scratch/n1-base.log`) and written into `EXPECTED` before any library code of F-653.

use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::hex_box;
use rcfd_fem::follower::{buckling_factors, tangent_modes, FollowerPressure};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::nonlinear::{initial_states, Contribution};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of e348eb34 in the order of the run (`f653-scratch/n1-base.log`); `None` prints them.
const EXPECTED: Option<[(&str, u64); 3]> = Some([
    ("tangent_modes follower", 0xfd29_c02b_bea8_2a65),
    ("buckling_factors follower", 0xc44e_5b7a_f4ed_7fe6),
    ("buckling factors", 0x8ca7_18a2_4d34_63ba),
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

const E: f64 = 2.0e11;
const L: f64 = 1.0;
const B: f64 = 0.04;
const H: f64 = 0.02;

fn beck() -> SolidModel {
    let base = hex_box([0.0; 3], [L, B, H], [50, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn p_ref() -> f64 {
    20.0 * (E * B * H * H * H / 12.0) / (L * L * B * H)
}

#[test]
fn n1_default_bits() {
    let m = beck();
    let mut md = beck();
    md.add_load(Load::pressure("xmax", p_ref())).unwrap();
    let s = solve_static(&md, &StaticOptions::default()).unwrap();
    let ulin = s.displacement_values().to_vec();
    let fp = FollowerPressure::new(&m, "xmax", p_ref()).unwrap();
    let mut got: Vec<(&str, u64)> = Vec::new();

    // (1) tangent_modes at p = 1 with the follower (symmetric part), 4 modes.
    let lf = 1.0 / 20.0;
    let u: Vec<f64> = ulin.iter().map(|x| lf * x).collect();
    let list: Vec<&dyn Contribution> = vec![&fp];
    let t = tangent_modes(
        &m,
        &[],
        &u,
        &initial_states(&m).unwrap(),
        lf,
        &list,
        &ModalOptions::new(4).residual_test(ResidualTest::Backward),
    )
    .unwrap();
    let mut v = t.eigenvalues.clone();
    for x in &t.modes {
        v.extend_from_slice(x);
    }
    v.extend_from_slice(&t.residuals);
    v.push(t.iterations as f64);
    got.push(("tangent_modes follower", digest(&v)));

    // (2) follower::buckling_factors, 1 factor; (3) buckling::factors, 2 factors.
    let pre = Prestress::from_static(&md, &s).unwrap();
    for (name, r) in [
        (
            "buckling_factors follower",
            buckling_factors(&md, &pre, &[&fp], &BucklingOptions::new(1)).unwrap(),
        ),
        (
            "buckling factors",
            factors(&md, &pre, &BucklingOptions::new(2)).unwrap(),
        ),
    ] {
        let mut v = r.factors.clone();
        for x in &r.modes {
            v.extend_from_slice(x);
        }
        v.extend_from_slice(&r.residuals);
        v.push(r.iterations as f64);
        v.push(r.sturm.counted as f64);
        println!("{name}: factors {:?}", r.factors);
        got.push((name, digest(&v)));
    }
    println!("tangent_modes eigenvalues {:?}", t.eigenvalues);
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    match EXPECTED {
        Some(e) => assert_eq!(got, e.to_vec()),
        None => panic!("no expected digests yet (the base run prints them)"),
    }
}
