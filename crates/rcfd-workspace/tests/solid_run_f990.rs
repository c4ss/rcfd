//! ADR 0840 (F-990): the solid driver `rcfd-solid-run` run as the runner launches it, on `solid_cases/cantilever_hex8.toml`.
//! D1: its exit, status stream and outputs, the report equal to the case's own entry's (`run_solid_case_file`); D2, D3:
//! its field store; D4: its pre-launch check; N1: the negative controls (an invalid case, amendment 1; a `--set` entry).

use rcfd_api::solid_case::{run_solid_case_file, SolidOutcome, SolidRun};
use rcfd_tsstore::{Location, Reader};
use rcfd_workspace::solid_run::{FIELDS, MONITORS};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("f990-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn case_text() -> String {
    std::fs::read_to_string(repo().join("crates/rcfd-api/tests/solid_cases/cantilever_hex8.toml")).unwrap()
}

/// The report without `outputs` (the paths written differ).
fn bare(mut v: Value) -> Value {
    v.as_object_mut().unwrap().remove("outputs");
    v
}

/// The case's own entry on the same text, in this process.
fn own_run(dir: &Path) -> SolidRun {
    let p = dir.join("own.toml");
    std::fs::write(&p, case_text()).unwrap();
    run_solid_case_file(&p, Some(dir)).unwrap()
}

/// The driver with the runner's launch arguments in run folder `dir` (`case.snapshot` written there).
fn launch(dir: &Path, text: &str, extra: &[&str]) -> (i32, Vec<Value>) {
    std::fs::write(dir.join("case.snapshot"), text).unwrap();
    let d = |n: &str| dir.join(n).display().to_string();
    let mut args = vec![
        "run".to_string(),
        d("case.snapshot"),
        "--case-dir".into(),
        dir.display().to_string(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    for (k, v) in [
        ("--out", d("output")),
        ("--status-file", d("solver-status.jsonl")),
        ("--status-every", "1".into()),
        ("--threads", "1".into()),
        ("--stop-file", d("cancel")),
        ("--pause-file", d("pause")),
        ("--store-dir", d("fields")),
    ] {
        args.push(k.into());
        args.push(v);
    }
    args.push("--quiet".into());
    args.push("--status-monitors".into());
    let code = Command::new(env!("CARGO_BIN_EXE_rcfd-solid-run"))
        .args(&args)
        .status()
        .unwrap()
        .code()
        .unwrap();
    let events = std::fs::read_to_string(dir.join("solver-status.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (code, events)
}

fn check_only(dir: &Path, text: &str) -> (i32, Value) {
    std::fs::write(dir.join("case.snapshot"), text).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rcfd-solid-run"))
        .args([
            "run",
            &dir.join("case.snapshot").display().to_string(),
            "--case-dir",
            &dir.display().to_string(),
        ])
        .args(["--check-only", "--status-json"])
        .output()
        .unwrap();
    let first = String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .unwrap_or("null")
        .to_string();
    (o.status.code().unwrap(), serde_json::from_str(&first).unwrap())
}

/// ADR 0840 D1, D2, D3, D4.
#[test]
fn d1_to_d4_the_driver_runs_the_cantilever_as_the_runner_launches_it() {
    let dir = scratch("d");
    let own = own_run(&scratch("own"));
    let want = bare(own.report.clone());
    // D4
    let (code, line) = check_only(&dir, &case_text());
    println!("D4: exit {code}, {line}");
    assert_eq!(
        (code, line.clone()),
        (0, serde_json::json!({"ok": true, "cells": 1280})),
        "D4"
    );
    // D1
    let (code, ev) = launch(&dir, &case_text(), &[]);
    let fin = ev.last().unwrap();
    println!("D1: exit {code}, finish {fin}");
    assert_eq!(code, 0, "D1 exit");
    assert_eq!(
        (fin["event"].as_str(), fin["state"].as_str(), fin["exit_code"].as_i64()),
        (Some("finish"), Some("finished"), Some(0)),
        "D1 finish"
    );
    let it: Vec<&Value> = ev.iter().filter(|e| e["event"] == "iteration").collect();
    assert_eq!(it.len(), 1, "D1 one iteration event");
    for (name, group, key) in MONITORS {
        assert_eq!(it[0]["monitors"][name], want[group][key], "D1 monitor {name}");
    }
    let out = dir.join("output");
    assert!(out.join("cantilever_hex8.vtu").is_file(), "D1 vtu");
    let report: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("cantilever_hex8.json")).unwrap()).unwrap();
    assert_eq!(
        bare(report.clone()),
        want,
        "D1 the report equals the case's own entry's"
    );
    // D2
    let mut r = Reader::open(&dir.join("fields")).unwrap();
    assert_eq!(r.snapshots().len(), 1, "D2 one snapshot");
    assert_eq!(
        (r.snapshots()[0].step, r.snapshots()[0].time),
        (1, 0.0),
        "D2 step and time"
    );
    assert_eq!((r.n_cells(), r.n_points()), (1280, 2025), "D2 mesh");
    let infos = r.fields(0).unwrap();
    let names: Vec<&str> = infos.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, FIELDS, "D2 fields");
    assert!(
        infos.iter().all(|f| f.location == Location::Cell && f.n == 1280),
        "D2 cell fields of 1280 values"
    );
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let vm = r.read_field(0, "von_mises").unwrap();
    let evm = want["stress"]["element_von_mises_max"].as_f64().unwrap();
    println!("D2: max von_mises {:e}, report element_von_mises_max {evm:e}", max(&vm));
    assert_eq!(
        max(&vm).to_bits(),
        evm.to_bits(),
        "D2 max von_mises is the report's element_von_mises_max"
    );
    let um = r.read_field(0, "u_magnitude").unwrap();
    let q = max(&um) / want["displacement"]["max_magnitude"].as_f64().unwrap();
    println!("D2: max u_magnitude / max_magnitude = {q:.6} (registered [0.97, 1.0], estimate 0.991)");
    assert!((0.97..=1.0).contains(&q), "D2 ratio {q}");
    // D3
    let SolidOutcome::Static { result, .. } = &own.outcome else {
        panic!("static")
    };
    let model = &own.setup.model;
    let mesh = model.mesh();
    let f = rcfd_post::solid_output::solid_fields(model, result).unwrap();
    let mut worst = 0.0f64;
    for (c, name) in ["ux", "uy", "uz"].iter().enumerate() {
        let got = r.read_field(0, name).unwrap();
        for (e, g) in got.iter().enumerate() {
            let nodes = &mesh.element_nodes(e)[..mesh.element_kind(e).corners()];
            let mean = nodes.iter().map(|&a| f.displacement[3 * a + c]).sum::<f64>() / nodes.len() as f64;
            worst = worst.max((g - mean).abs());
        }
    }
    for (k, name) in ["sxx", "syy", "szz", "sxy", "syz", "szx"].iter().enumerate() {
        let got = r.read_field(0, name).unwrap();
        for (e, g) in got.iter().enumerate() {
            worst = worst.max((g - f.element_stress[6 * e + k]).abs());
        }
    }
    println!("D3: max abs difference {worst:e} (registered 0)");
    assert_eq!(worst, 0.0, "D3");
}

/// ADR 0840 N1.
#[test]
fn n1_an_invalid_case_and_a_set_entry_are_refused() {
    // amendment 1: the cantilever without its line `youngs_modulus = 2.0e11`
    let bad = case_text().replace("youngs_modulus = 2.0e11", "");
    assert_ne!(bad, case_text());
    let dir = scratch("n1a");
    let (code, line) = check_only(&dir, &bad);
    println!("N1 check-only: exit {code}, {line}");
    assert_eq!(code, 2, "N1 check-only exit");
    assert_eq!(line["ok"], false);
    assert!(
        line["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["path"] == "materials.steel.youngs_modulus"),
        "N1 the diagnostic's key"
    );
    let (code, ev) = launch(&dir, &bad, &[]);
    let fin = ev.last().unwrap();
    println!("N1 run: exit {code}, {fin}");
    assert_ne!(code, 0, "N1 run exit");
    assert_eq!(
        (fin["event"].as_str(), fin["state"].as_str()),
        (Some("finish"), Some("failed"))
    );
    assert!(!dir.join("fields").exists(), "N1 no store");
    let dir = scratch("n1b");
    let (code, ev) = launch(&dir, &case_text(), &["--set", "a=1"]);
    let fin = ev.last().unwrap();
    println!("N1 --set: exit {code}, {fin}");
    assert_ne!(code, 0, "N1 --set exit");
    assert_eq!(fin["state"], "failed");
    assert!(fin["reason"].as_str().unwrap().contains("--set"), "N1 --set named");
}
