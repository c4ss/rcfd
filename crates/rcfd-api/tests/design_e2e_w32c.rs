//! ADR 0067 E1: end-to-end design loop on a real rcfd case (contraction channel, pressure-drop minimisation).

use rcfd_api::case::Case;
use rcfd_api::design::case_eval::{CaseEvaluator, RunLimits};
use rcfd_api::design::store::{key_of, Store};
use rcfd_api::design::study::{
    Cmp, Condition, ConstraintDef, Method, ObjectiveDef, ParamDef, Robust, Sense, Study, StudyDef, Target,
};

const BASE: &str = r#"
[mesh]
generator = "wedge_2d"
nx = 40
ny = 12
lx = 5.0
ly = 1.0
ramp_start = 0.5
ramp_angle = 6.0
[physics]
nu = 0.05
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.p_in]
type = "area_average"
patch = "xmin"
field = "p"
[reports.p_out]
type = "area_average"
patch = "xmax"
field = "p"
[reports.ux_out]
type = "area_average"
patch = "xmax"
field = "ux"
[reports.dp]
type = "expression"
expression = "p_in - p_out"
"#;

fn def() -> StudyDef {
    StudyDef {
        name: "contraction".into(),
        params: vec![
            ParamDef {
                name: "angle".into(),
                lo: 0.0,
                hi: 12.0,
                init: Some(6.0),
                targets: vec![Target {
                    path: "mesh.ramp_angle".into(),
                    scale: 1.0,
                    offset: 0.0,
                }],
            },
            ParamDef {
                name: "start".into(),
                lo: 0.5,
                hi: 3.0,
                init: Some(0.5),
                targets: vec![Target {
                    path: "mesh.ramp_start".into(),
                    scale: 1.0,
                    offset: 0.0,
                }],
            },
        ],
        conditions: vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: vec![ObjectiveDef {
            metric: "dp".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![ConstraintDef {
            metric: "ux_out".into(),
            cmp: Cmp::Ge,
            limit: 1.3,
            scale: 1.3,
        }],
        workers: 1,
    }
}

#[test]
fn e1_contraction_pressure_drop_design_loop() {
    let base = Case::parse_toml(BASE).unwrap();
    let ev = CaseEvaluator::new(base, RunLimits::default(), true);
    let dir = std::env::temp_dir().join(format!("rcfd_e1_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("evaluations.jsonl");
    let method = Method::NelderMead {
        budget: 80,
        restarts: 1,
        seed: 1,
    };
    let mut study = Study::new(def(), Store::open(&path, "e1").unwrap(), &ev).unwrap();
    let res = study.run(&method).unwrap();
    let recs = study.store.records();
    let first = &recs[0];
    let f0 = study.fitness_of(first);
    let start_x = study.x_of(first);
    assert_eq!(start_x, vec![6.0, 0.5], "the first evaluation is the start point");
    assert!(
        f0.feasible(),
        "the start point must be feasible: violation {}",
        f0.violation
    );
    let best = study.best().unwrap();
    let fb = study.fitness_of(&recs[best]);
    let xb = study.x_of(&recs[best]);
    println!(
        "E1: {} evaluations; start dp {:.6} at {:?}; best dp {:.6} at {:?} (ux_out {:.5}); reduction {:.1} %; optimiser result f {:.6}",
        recs.len(),
        f0.f,
        start_x,
        fb.f,
        xb,
        recs[best].conditions[0].metrics["ux_out"],
        100.0 * (1.0 - fb.f / f0.f),
        res.fitness.f
    );
    for r in recs {
        assert!(r.ok(), "record {} not converged: {}", r.index, r.message);
        assert!(r.conditions[0].status == "converged");
    }
    assert!(fb.feasible() && fb.violation <= 1e-3);
    assert!(fb.f <= 0.75 * f0.f, "best {} vs start {}", fb.f, f0.f);
    assert!(recs.len() <= 80 + 2);
    // re-run three stored evaluations from their stored parameters: bit-identical metrics
    let n = recs.len();
    for &i in &[0, n / 2, n - 1] {
        let x = study.x_of(&recs[i]);
        let again = rcfd_api::design::study::Evaluator::evaluate(&ev, &study.def, &x);
        assert_eq!(again[0].metrics.len(), recs[i].conditions[0].metrics.len());
        for (k, v) in &recs[i].conditions[0].metrics {
            assert_eq!(again[0].metrics[k].to_bits(), v.to_bits(), "record {i} metric {k}");
        }
        assert_eq!(key_of(&x), recs[i].key);
    }
    // resumed from the store: no new evaluation
    let before = ev.runs.load(std::sync::atomic::Ordering::SeqCst);
    let mut again = Study::new(def(), Store::open(&path, "e1").unwrap(), &ev).unwrap();
    again.run(&method).unwrap();
    assert_eq!(again.evaluated, 0);
    assert_eq!(ev.runs.load(std::sync::atomic::Ordering::SeqCst), before);
    assert_eq!(again.store.records().len(), n);
}
