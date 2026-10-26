//! Wave 46B (ADR 0119) D2: the opt-in reduced-order fan evaluator drives the design-study machinery (DOE, Nelder-Mead) and
//! reports located failures per condition.

use rcfd_api::design::fan_rom_eval::{FanRomEvaluator, RomCondition};
use rcfd_api::design::store::Store;
use rcfd_api::design::study::{
    Condition, Doe, Evaluator, Method, ObjectiveDef, ParamDef, Robust, Sense, Study, StudyDef,
};
use rcfd_mesh::fan::SectionFamily;
use rcfd_post::fan_rom::*;
use serde_json::json;

const Q_D: f64 = 0.045;

fn op() -> Operating {
    Operating {
        rpm: 3000.0,
        rho: 1.2,
        nu: 1.5e-5,
    }
}

/// Set-up C of ADR 0119 (free-vortex design, ideal 45 Pa at Q_D) and the R1b system coefficient.
fn set_up() -> (RomFan, f64) {
    let s = |radius: f64| RomSection {
        radius,
        chord: 0.030,
        stagger: 0.0,
        camber: 0.04,
        camber_pos: 0.5,
    };
    let base = RomFan {
        n_blades: 7,
        r_hub: 0.030,
        r_tip: 0.058,
        tip_gap: 0.001,
        family: SectionFamily::Naca4,
        sections: vec![s(0.030), s(0.058)],
    };
    let m = RomModel::default();
    let fan = free_vortex_design(&base, &m, &op(), Q_D, 45.0 / 1.2 / op().omega()).unwrap();
    let k = FanRom::new(fan.clone(), m).unwrap().point(&op(), Q_D).unwrap().dp_s / (Q_D * Q_D);
    (fan, k)
}

fn evaluator(fan: RomFan, k: f64) -> FanRomEvaluator {
    FanRomEvaluator {
        geometry: Box::new(move |x: &[f64]| {
            let mut f = fan.clone();
            for s in &mut f.sections {
                s.stagger += x[0].to_radians();
            }
            Ok(f)
        }),
        model: RomModel::default(),
        base: RomCondition {
            op: op(),
            system: SystemCurve { k, l: 0.0 },
            c0: 343.0,
            observer_r: 1.0,
            observer_theta: std::f64::consts::FRAC_PI_2,
            n_scan: 100,
        },
    }
}

fn def(conditions: Vec<Condition>) -> StudyDef {
    StudyDef {
        name: "w46b".into(),
        params: vec![ParamDef {
            name: "stagger_offset_deg".into(),
            lo: -3.0,
            hi: 3.0,
            init: Some(0.0),
            targets: vec![],
        }],
        conditions,
        objectives: vec![ObjectiveDef {
            metric: "eta_s".into(),
            sense: Sense::Max,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers: 1,
    }
}

fn cond(label: &str, set: Vec<(&str, f64)>) -> Condition {
    Condition {
        label: label.into(),
        set: set.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect(),
    }
}

#[test]
fn d2_design_loop_runs_on_the_reduced_order_fan() {
    let (fan, k) = set_up();
    let ev = evaluator(fan, k);
    let d = def(vec![
        cond("k1", vec![("system_k", k)]),
        cond("k15", vec![("system_k", 1.5 * k)]),
    ]);
    let mut s = Study::new(d.clone(), Store::memory("w46b"), &ev).unwrap();
    s.run(&Method::Doe(Doe::Factorial { levels: vec![5] })).unwrap();
    let centre = s
        .store
        .records()
        .iter()
        .find(|r| r.params[0].1 == 0.0)
        .expect("centre point evaluated");
    for c in &centre.conditions {
        println!("W46B D2 centre {}: ok {} {:?} {}", c.label, c.ok, c.metrics, c.message);
        assert!(
            c.ok && c.metrics.len() == 8 && c.metrics.values().all(|v| v.is_finite()),
            "{c:?}"
        );
    }
    assert!((centre.conditions[0].metrics["q"] / Q_D - 1.0).abs() < 1e-9);
    let mut s = Study::new(d, Store::memory("w46b-nm"), &ev).unwrap();
    let r = s
        .run(&Method::NelderMead {
            budget: 30,
            restarts: 0,
            seed: 1,
        })
        .unwrap();
    println!(
        "W46B D2 Nelder-Mead: x {:?}, fitness {:?}, evaluations {}",
        r.x, r.fitness, r.evaluations
    );

    let bad = def(vec![
        cond("bogus", vec![("bogus", 1.0)]),
        cond("stall", vec![("system_k", 20.0 * k)]),
    ]);
    let out = ev.evaluate(&bad, &[0.0]);
    println!("W46B D2 failures: {} | {}", out[0].message, out[1].message);
    assert!(!out[0].ok && out[0].message.contains("bogus"), "{:?}", out[0]);
    assert!(!out[1].ok && out[1].message.contains("stall"), "{:?}", out[1]);
}
