//! F-617 (ADR 0539): check B2 (D8). The runs with a nodal temperature field and no thermal predictor give the bits they gave at
//! 8ebc67e8, before the predictor: the driver on the free cube (FE1, with its halved first step; FE2; the hot start refused),
//! on the held cube (HB1) and on a nonuniform beam; the integrator at the equilibrium of FEi, the ramp of INT1 and the V3T bar
//! (values and state bytes). Uses only API that existed at 8ebc67e8; the expected digests were printed by this file run on the
//! unchanged base (`f617-scratch/b2-base.log`) and written into `EXPECTED` before any library code of F-617.

#[path = "pred_common/mod.rs"]
mod common;

use common::*;
use rcfd_fem::nl_transient::NlTransientOptions;
use rcfd_fem::nonlinear::{solve_in_time_with, TimeLoading, TimeSteppingOptions};
use rcfd_fem::thermal::TemperatureFieldHistory;
use rcfd_fem::transient::Rayleigh;
use rcfd_fem::SolidModel;

/// Digests of the base in the order of the run.
const EXPECTED: Option<[(&str, u64); 9]> = Some([
    ("driver FE1 free cube step (halved)", 0xb11c_1ebb_4988_d581),
    ("driver FE2 free cube ramp", 0x90b7_0d07_58cd_0002),
    ("driver hot start refusal", 0xa8b5_34a3_acf4_66f7),
    ("driver HB1 held cube", 0x5438_7b05_156a_0e4c),
    ("driver nonuniform beam ramp", 0xdb4d_e8b4_8b37_8ac4),
    ("integrator FEi equilibrium", 0xefb4_b208_efc4_678d),
    ("integrator INT1 ramp", 0x528e_14ef_a4cc_e3ba),
    ("integrator INT1 ramp corrections", 0xd2d5_17a4_d81b_c5a5),
    ("integrator V3T bar values and bytes", 0xaea8_ad3e_560a_be74),
]);

fn driver(model: &SolidModel, field: TemperatureFieldHistory, times: &[f64]) -> rcfd_fem::Result<Vec<f64>> {
    let opts = TimeSteppingOptions::new(f64::INFINITY)
        .newton(tight())
        .temperature_field(field);
    solve_in_time_with(model, &[], &TimeLoading::new(), times, &opts).map(|h| flat_history(&h))
}

#[test]
fn b2_runs_with_a_field_and_no_predictor_keep_their_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();

    let free = free_cube(steel(ALPHA));
    let v = driver(&free, step_field(&free), &[1.0, 2.0, 3.0]).unwrap();
    got.push(("driver FE1 free cube step (halved)", digest(&v)));
    let v = driver(&free, ramp_field(&free), &[1.0, 2.0, 3.0, 4.0]).unwrap();
    got.push(("driver FE2 free cube ramp", digest(&v)));
    let msg = format!("{:?}", driver(&free, hot_field(&free), &[1.0, 2.0]).err());
    println!("B2 hot start refusal: {msg}");
    got.push(("driver hot start refusal", digest_bytes(msg.bytes())));

    let held = held_cube(steel(ALPHA));
    let v = driver(&held, step_field(&held), &[1.0, 2.0, 3.0]).unwrap();
    got.push(("driver HB1 held cube", digest(&v)));

    let bm = beam(1e-5);
    let v = driver(&bm, beam_field(&bm), &[1.0, 2.0, 3.0]).unwrap();
    got.push(("driver nonuniform beam ramp", digest(&v)));

    // FEi: the cube at its thermal equilibrium.
    let a = ALPHA * DT;
    let n = free.dof_map().n_equations();
    let field = TemperatureFieldHistory::constant(REF, uniform(&free, REF + DT), 0.0, 1.0).unwrap();
    let o = NlTransientOptions::new(1e-5)
        .damping(Rayleigh {
            mass: 10.0,
            stiffness: 1e-6,
        })
        .newton(tight())
        .temperature_field(field);
    let run = vibrate(&free, &o, &expanded(&free, a), &vec![0.0; n], 50);
    got.push(("integrator FEi equilibrium", digest(&run.flat)));

    // INT1: the free expansion under a ramp without the predictor (rate a per second).
    let ramp =
        TemperatureFieldHistory::new(REF, vec![(0.0, uniform(&free, REF)), (1.0, uniform(&free, REF + DT))]).unwrap();
    let o = NlTransientOptions::new(0.05).newton(tight()).temperature_field(ramp);
    let run = vibrate(&free, &o, &vec![0.0; n], &expanded(&free, a), 20);
    got.push(("integrator INT1 ramp", digest(&run.flat)));
    let counts: Vec<f64> = run.corrections.iter().map(|&k| k as f64).collect();
    got.push(("integrator INT1 ramp corrections", digest(&counts)));
    println!("B2 INT1 corrections without the predictor: {:?}", run.corrections);

    // V3T: the nonuniform step.
    let b = bar(steel(1e-5));
    let dt = 2e-6;
    let o = NlTransientOptions::new(dt)
        .damping(Rayleigh {
            mass: 20.0,
            stiffness: 2.0e-8,
        })
        .temperature_field(v3t_field(&b, dt));
    let run = vibrate(&b, &o, &vec![0.0; b.dof_map().n_equations()], &v3t_velocity(&b), 40);
    let mut all = run.flat.clone();
    all.extend(run.bytes.iter().map(|&x| f64::from(x)));
    got.push(("integrator V3T bar values and bytes", digest(&all)));

    for (name, d) in &got {
        println!("B2 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B2 {name}: {d:#018x} against {e:#018x}");
    }
}
