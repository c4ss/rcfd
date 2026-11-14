//! F-536 (ADR 0458): the opt-in outer-iteration criterion `PimpleSettings::outer_pressure_tolerance` and its record
//! `PimpleSolver::outer_record` on a small driven channel. E1: a criterion that is never met runs the fixed count bit for bit;
//! E2: no record without the criterion; E3: a fluid at rest meets it at once and runs one final iteration; E4: a driven flow
//! meets it before the cap. The limits were registered in ADR 0458 before this file existed.

use rcfd_core::vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 1e-2;
const DT: f64 = 0.05;
const STEPS: usize = 5;

/// A channel of 12 x 6 x 1 cells over `2 x 1 x 0.2`: no-slip `ymin`, `ymax`, `p = 0` at `xmax`, the inflow at `xmin`.
fn solver(s: PimpleSettings) -> PimpleSolver {
    let mesh = box_mesh(
        [12, 6, 1],
        [2.0, 1.0, 0.2],
        [Grading::Uniform, Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax", "xmin"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, p, s)
}

/// `steps` steps; the inflow `sin(pi t)` when `driven`, none otherwise.
fn run(mut fl: PimpleSolver, driven: bool) -> PimpleSolver {
    for k in 1..=STEPS {
        if driven {
            let g = fl.geom.clone();
            let v = (PI * k as f64 * DT).sin();
            fl.u.set_bc(&g, "xmin", Box::new(FixedValue::vector([v, 0.0, 0.0])))
                .unwrap();
        }
        fl.step(DT);
        assert!(fl.diverged.is_none(), "step {k} stopped");
    }
    fl
}

fn settings(n_outer: usize, tol: Option<f64>, consistent: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.n_outer = n_outer;
    s.outer_pressure_tolerance = tol;
    s.consistent_correctors = consistent;
    s
}

fn bits(fl: &PimpleSolver) -> Vec<u64> {
    fl.u.values
        .iter()
        .flatten()
        .chain(&fl.p.values)
        .chain(&fl.phi)
        .map(|x| x.to_bits())
        .collect()
}

#[test]
fn e1_never_met_runs_the_fixed_count_bit_for_bit_e2_no_record_without() {
    let fixed = run(solver(settings(6, None, false)), true);
    let never = run(solver(settings(6, Some(0.0), false)), true);
    assert!(bits(&fixed) == bits(&never), "E1: the fields differ");
    assert_eq!(never.outer_record.len(), STEPS, "E1: one record per step");
    for (i, r) in never.outer_record.iter().enumerate() {
        println!("E1 {r:?}");
        assert_eq!(r.step, i + 1);
        assert_eq!(r.used, 6, "E1: used");
        assert!(!r.met, "E1: met");
    }
    assert!(fixed.outer_record.is_empty(), "E2: a record without the criterion");
}

#[test]
fn e3_rest_meets_at_once_with_one_final_iteration() {
    let fl = run(solver(settings(6, Some(1e-3), false)), false);
    assert_eq!(fl.outer_record.len(), STEPS, "E3: one record per step");
    for (i, r) in fl.outer_record.iter().enumerate() {
        println!("E3 {r:?}");
        assert_eq!(r.step, i + 1);
        assert_eq!(r.used, 2, "E3: used");
        assert!(r.met, "E3: met");
    }
}

#[test]
fn e4_driven_flow_meets_before_the_cap() {
    let fl = run(solver(settings(30, Some(1e-2), false)), true);
    assert_eq!(fl.outer_record.len(), STEPS, "E4: one record per step");
    for (i, r) in fl.outer_record.iter().enumerate() {
        println!("E4 {r:?}");
        assert_eq!(r.step, i + 1);
        assert!(r.met, "E4: met");
        assert!(r.used < 30, "E4: used {}", r.used);
        assert!(r.ratio <= 1e-2, "E4: ratio {}", r.ratio);
    }
}
