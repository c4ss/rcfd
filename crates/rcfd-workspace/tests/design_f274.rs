//! ADR 0189 C4-U: the `rcfd run --set` entries of a design run give the settings tree the in-process evaluator builds, and
//! the result of a run is read from its `metrics.json` by the in-process status rule.

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::design::study::{Condition, ObjectiveDef, ParamDef, Robust, Sense, StudyDef, Target};
use rcfd_api::study::apply_value;
use rcfd_workspace::design::{cond_result, entries, set_entry, variant_name};
use rcfd_workspace::templates::LID_DRIVEN_CAVITY;
use serde_json::{json, Value};
use std::path::Path;

/// The template at 16 x 16 cells (its own size) with `max_iterations = 3000`.
fn small_case() -> Case {
    let text = LID_DRIVEN_CAVITY.replace("max_iterations = 600", "max_iterations = 3000");
    assert_ne!(text, LID_DRIVEN_CAVITY);
    Case::parse_toml_with(&text, Path::new("."), None).unwrap()
}

fn param(name: &str, targets: Vec<Target>) -> ParamDef {
    ParamDef {
        name: name.into(),
        lo: -1e30,
        hi: 1e30,
        init: None,
        targets,
    }
}

fn target(path: &str, scale: f64, offset: f64) -> Target {
    Target {
        path: path.into(),
        scale,
        offset,
    }
}

fn def(params: Vec<ParamDef>) -> StudyDef {
    StudyDef {
        name: "t".into(),
        params,
        conditions: vec![Condition {
            label: "base".into(),
            set: vec![],
        }],
        objectives: vec![ObjectiveDef {
            metric: "lid_drag".into(),
            sense: Sense::Min,
            weight: 1.0,
            robust: Robust::Mean,
        }],
        constraints: vec![],
        workers: 1,
    }
}

#[test]
fn c4u_entries_give_the_tree_of_the_in_process_evaluator() {
    // the entry text
    assert_eq!(set_entry("p", &json!(20.0)), "p=20");
    assert_eq!(set_entry("p", &json!(0.015)), "p=0.015");
    let big = set_entry("p", &json!(1e20));
    let text = big.strip_prefix("p=").unwrap();
    assert!(text.parse::<i64>().is_err(), "1e20 written as an integer: {big}");
    assert_eq!(text.parse::<f64>().unwrap(), 1e20, "{big}");
    assert_eq!(set_entry("p", &json!("piso")), "p=\"piso\"");
    assert_eq!(set_entry("p", &json!([2.0, 0.0, 0.0])), "p=[2.0,0.0,0.0]");
    eprintln!("C4-U entries: p=20, p=0.015, {big}, p=\"piso\", p=[2.0,0.0,0.0]");
    // each entry, applied as `rcfd run --set` applies it, gives the tree `apply_value` gives
    let base = small_case();
    for (path, v) in [
        ("mesh.nx", json!(20.0)),
        ("physics.nu", json!(0.015)),
        ("solver.algorithm", json!("piso")),
        ("boundaries.lid.velocity_value", json!([2.0, 0.0, 0.0])),
    ] {
        let mut a = base.clone();
        apply_value(&mut a, path, &v).unwrap();
        let mut b = base.clone();
        apply_set(&mut b, &set_entry(path, &v)).unwrap();
        assert_eq!(a.tree.to_json(), b.tree.to_json(), "{path}");
        assert_ne!(
            a.tree.to_json(),
            base.tree.to_json(),
            "{path}: the value did not change the case"
        );
    }
    // control: the plain text of 20.0 is not what apply_value makes of it
    let mut a = base.clone();
    apply_value(&mut a, "mesh.nx", &json!(20.0)).unwrap();
    let mut c = base.clone();
    match apply_set(&mut c, "mesh.nx=20.0") {
        Err(e) => eprintln!("C4-U control: mesh.nx=20.0 refused: {e}"),
        Ok(()) => {
            assert_ne!(c.tree.to_json(), a.tree.to_json(), "mesh.nx=20.0 gave the integer tree");
            eprintln!("C4-U control: mesh.nx=20.0 gives another tree");
        }
    }
    // the entries of a candidate: targets (scale * x + offset) in order, then the condition's settings
    let mut d = def(vec![
        param("nu", vec![target("physics.nu", 1.0, 0.0)]),
        param("u", vec![target("mesh.nx", 10.0, 0.0), target("mesh.ny", 1.0, 18.0)]),
    ]);
    d.conditions[0].set = vec![("solver.algorithm".into(), json!("simple"))];
    let x = [0.015, 2.0];
    let e = entries(&d, &x, &d.conditions[0]);
    assert_eq!(
        e,
        [
            "physics.nu=0.015",
            "mesh.nx=20",
            "mesh.ny=20",
            "solver.algorithm=\"simple\""
        ]
    );
    assert_eq!(variant_name(&d, &x), "nu=0.015,u=2.0");
    let ev = rcfd_api::design::case_eval::CaseEvaluator::new(base.clone(), Default::default(), true);
    let want = ev.build_case(&d, &x, Some(&d.conditions[0])).unwrap();
    let mut got = base.clone();
    for s in &e {
        apply_set(&mut got, s).unwrap();
    }
    assert_eq!(want.tree.to_json(), got.tree.to_json());
}

fn finish(state: &str, exit_code: u64, iterations: u64, reports: Value) -> Value {
    json!({"state": "finished", "reason": "x", "exit_code": exit_code, "steps": iterations, "time": 0.0,
        "solver_finish": {"event": "finish", "state": state, "exit_code": exit_code, "iterations": iterations,
            "reason": format!("{state} after {iterations}")},
        "last_residuals": null, "reports": reports})
}

#[test]
fn c4u_the_result_follows_the_in_process_status_rule() {
    let r = cond_result("base", &finish("converged", 0, 412, json!({"lid_drag": 0.0123})), "");
    assert!(r.ok, "{r:?}");
    assert_eq!((r.status.as_str(), r.iterations), ("converged", 412));
    assert_eq!(r.metrics.get("lid_drag"), Some(&0.0123));
    // a run the runner calls finished (exit 5) that did not converge
    let r = cond_result("base", &finish("not_converged", 5, 3000, json!({"lid_drag": 0.01})), "");
    assert!(!r.ok);
    assert_eq!(r.status, "not_converged");
    assert!(r.metrics.is_empty(), "{r:?}");
    assert!(r.message.starts_with("not_converged after 3000"), "{}", r.message);
    let r = cond_result("base", &finish("diverged", 6, 40, json!({})), "");
    assert_eq!((r.ok, r.status.as_str()), (false, "diverged"));
    // no solver result: ended before launch
    let pending = json!({"state": "failed", "reason": "the pre-launch check failed", "steps": 0, "launched": false,
        "reports": {}});
    let r = cond_result("base", &pending, "the pre-launch check failed");
    assert_eq!((r.ok, r.status.as_str()), (false, "failed"));
    assert_eq!(r.message, "the pre-launch check failed");
    // a null report is left out and named
    let r = cond_result("slow", &finish("converged", 0, 9, json!({"a": 1.5, "b": null})), "");
    assert!(r.ok);
    assert_eq!(r.label, "slow");
    assert_eq!(r.metrics.keys().collect::<Vec<_>>(), ["a"]);
    assert!(r.message.contains("report b: not finite"), "{}", r.message);
    // only non-finite reports: not usable, as in process
    let r = cond_result("base", &finish("converged", 0, 9, json!({"b": null})), "");
    assert!(!r.ok && r.metrics.is_empty());
}
