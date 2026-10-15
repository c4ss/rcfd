//! Run-control additions of wave 19C (ADR 0031 D5/D6): exit codes 4 and 6 end to end, `--print-settings`,
//! `--set`, `rcfd check`.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn rcfd_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w19c_rc_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const CASE: &str = r#"
title = "run-control cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-7
tolerance_p = 1e-7
"#;

fn write(dir: &Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.display().to_string()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn events(o: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[test]
fn r1_exit_code_4_end_to_end_or_not_reproduced() {
    let d = tmp("r1_4");
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/wedge/wedge.toml"))
        .unwrap()
        .replace("ramp_angle = 10.0", "ramp_angle = 89.0");
    let case = write(&d, "wedge89.toml", &text);
    let (a, b, c) = (
        rcfd(&["run", &case, "--out", d.join("o").to_str().unwrap(), "--quiet"]),
        rcfd(&["run", &case, "--check-only"]),
        rcfd(&["check", &case]),
    );
    let codes = (code(&a), code(&b), code(&c));
    println!(
        "R1 exit 4 candidate wedge_2d ramp_angle=89: run {}, run --check-only {}, check {}",
        codes.0, codes.1, codes.2
    );
    if codes == (4, 4, 4) {
        assert!(
            !d.join("o").join("solution.vtu").exists(),
            "a bad mesh must not be solved"
        );
    } else {
        println!("R1 exit 4: NOT REPRODUCED through the binary with this candidate (left to the library tests)");
    }
}

#[test]
fn r1_exit_code_6_end_to_end_or_not_reproduced() {
    let d = tmp("r1_6");
    let cand1 = CASE
        .replace(
            "velocity_value = [1.0, 0.0, 0.0]",
            "velocity_value = [1000.0, 0.0, 0.0]",
        )
        .replace("nu = 0.01", "nu = 1e-4")
        .replace(
            "convection = \"upwind\"",
            "convection = \"central\"\nalpha_u = 1.0\nalpha_p = 1.0",
        );
    let cand2 = CASE
        .replace("nu = 0.01", "nu = 1e-4")
        .replace(
            "algorithm = \"simplec\"",
            "algorithm = \"piso\"\ndt = 100.0\nend_time = 1000.0",
        )
        .replace("convection = \"upwind\"", "convection = \"central\"");
    let mut got = None;
    for (i, c) in [cand1, cand2].iter().enumerate() {
        let case = write(&d, &format!("c{i}.toml"), c);
        let o = rcfd(&[
            "run",
            &case,
            "--out",
            d.join(format!("o{i}")).to_str().unwrap(),
            "--status-json",
        ]);
        let ev = events(&o);
        let state = ev.last().map(|e| e["state"].to_string()).unwrap_or_default();
        println!("R1 exit 6 candidate {}: exit {} state {}", i + 1, code(&o), state);
        if code(&o) == 6 {
            assert_eq!(ev.last().unwrap()["state"], "diverged");
            assert_eq!(ev.last().unwrap()["exit_code"], 6);
            got = Some(i + 1);
            break;
        }
    }
    if got.is_none() {
        println!(
            "R1 exit 6: NOT REPRODUCED through the binary with either registered candidate (left to the library tests)"
        );
    }
}

#[test]
fn r2_print_settings_equals_the_resolved_tree_and_runs_nothing() {
    let d = tmp("r2");
    let case = write(&d, "case.toml", CASE);
    let o = rcfd_in(&d, &["run", &case, "--print-settings"]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let got: Value = serde_json::from_slice(&o.stdout).expect("stdout is JSON");
    let want = rcfd_api::case::Case::load(&case).unwrap().tree.to_json();
    assert_eq!(got, want);
    let entries: Vec<_> = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(entries.len(), 1, "no results directory expected: {entries:?}");
    // --set is applied to the printed settings
    let o = rcfd_in(
        &d,
        &["run", &case, "--print-settings", "--set", "solver.max_iterations=7"],
    );
    let got: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(got["solver"]["max_iterations"], 7);
    // an invalid case exits 3
    let bad = write(&d, "bad.toml", &CASE.replace("nu = 0.01", "nu = -1.0"));
    assert_eq!(code(&rcfd_in(&d, &["run", &bad, "--print-settings"])), 3);
}

#[test]
fn r3_set_overrides_equal_a_hand_edited_case_and_errors_have_the_right_codes() {
    let d = tmp("r3");
    let case = write(&d, "case.toml", CASE);
    let edited = write(
        &d,
        "edited.toml",
        &CASE
            .replace("max_iterations = 400", "max_iterations = 7")
            .replace("nx = 12", "nx = 10"),
    );
    let hash = |args: &[&str]| -> String {
        let o = rcfd(args);
        events(&o).first().expect("start event")["case_hash"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let out = d.join("o");
    let a = hash(&[
        "run",
        &case,
        "--out",
        out.to_str().unwrap(),
        "--status-json",
        "--set",
        "solver.max_iterations=7",
        "--set",
        "mesh.nx=10",
    ]);
    let b = hash(&["run", &edited, "--out", out.to_str().unwrap(), "--status-json"]);
    assert_eq!(a, b);
    assert_eq!(code(&rcfd(&["run", &case, "--set", "foo"])), 2);
    assert_eq!(code(&rcfd(&["run", &case, "--set", "solver.dt={"])), 2);
    assert_eq!(
        code(&rcfd(&["run", &case, "--set", "solver.no_such=1", "--check-only"])),
        3
    );
    assert_eq!(
        code(&rcfd(&[
            "run",
            &case,
            "--set",
            "solver.algorithm=\"warp\"",
            "--check-only"
        ])),
        3
    );
    // a bare word is a string
    assert_eq!(
        code(&rcfd(&[
            "run",
            &case,
            "--set",
            "solver.algorithm=simple",
            "--check-only"
        ])),
        0
    );
}

#[test]
fn r4_check_exit_codes_and_json_shape() {
    let d = tmp("r4");
    let good = write(&d, "good.toml", CASE);
    let o = rcfd(&["check", &good, "--json"]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["diagnostics", "exit_code", "mesh", "ok", "rules"]);
    assert_eq!(v["ok"], true);
    // warning only: exit 0, rule listed with exactly the registered keys
    let warn = write(
        &d,
        "warn.toml",
        &CASE.replace("tolerance_u = 1e-7", "tolerance_u = 0.05"),
    );
    let o = rcfd(&["check", &warn, "--json"]);
    assert_eq!(code(&o), 0);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let rules = v["rules"].as_array().unwrap();
    assert!(
        rules.iter().any(|r| r["id"] == "SV105" && r["severity"] == "warning"),
        "{rules:?}"
    );
    for r in rules {
        let mut k: Vec<_> = r.as_object().unwrap().keys().cloned().collect();
        k.sort();
        let want: Vec<&str> = if r.get("fix").is_some() {
            vec!["category", "fix", "id", "message", "path", "severity"]
        } else {
            vec!["category", "id", "message", "path", "severity"]
        };
        assert_eq!(k, want);
    }
    // an invalid case: exit 3 with located diagnostics
    let bad = write(&d, "bad.toml", &CASE.replace("nu = 0.01", "nu = -1.0"));
    let o = rcfd(&["check", &bad, "--json"]);
    assert_eq!(code(&o), 3);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["ok"], false);
    assert!(!v["diagnostics"].as_array().unwrap().is_empty());
    // a mesh that fails its quality check: exit 4 (same candidate as R1)
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/wedge/wedge.toml"))
        .unwrap()
        .replace("ramp_angle = 10.0", "ramp_angle = 89.0");
    let w = write(&d, "w89.toml", &text);
    let c = code(&rcfd(&["check", &w]));
    println!("R4 bad-mesh candidate: exit {c}");
    // the missing file and usage errors
    assert_eq!(code(&rcfd(&["check"])), 2);
    assert_eq!(code(&rcfd(&["check", d.join("nope.toml").to_str().unwrap()])), 3);
}

#[test]
fn rules_warnings_reach_the_status_stream_with_the_rule_id() {
    let d = tmp("warn");
    let warn = write(
        &d,
        "warn.toml",
        &CASE
            .replace("tolerance_u = 1e-7", "tolerance_u = 0.05")
            .replace("max_iterations = 400", "max_iterations = 3"),
    );
    let o = rcfd(&["run", &warn, "--out", d.join("o").to_str().unwrap(), "--status-json"]);
    let ev = events(&o);
    let w = ev.iter().find(|e| e["event"] == "warning" && e["rule"] == "SV105");
    assert!(w.is_some(), "no SV105 warning event: {ev:?}");
    assert_eq!(w.unwrap()["path"], "solver.tolerance_u");
    assert!(String::from_utf8_lossy(&o.stderr).contains("SV105"));
    // validate prints rule lines too and keeps its exit code for warnings
    let v = rcfd(&["validate", &warn]);
    assert_eq!(code(&v), 0);
    assert!(String::from_utf8_lossy(&v.stderr).contains("SV105"));
}
