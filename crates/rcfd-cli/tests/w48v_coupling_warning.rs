//! ADR 0156 (job 48V) through the `rcfd` binary: B3 (`rcfd check --json` gives the computed rule SV301 on the refined
//! cylinder wall layer, exit code unchanged), B4 (`rcfd run` emits it as a `warning` event after `start`, before the first
//! iteration) and B5 (the pre-run estimate changes no result: `pressure_lag_check` warn against off, every iteration event and
//! the stored fields equal).

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w48v_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The committed example case (`cylinder_ogrid` 64 x 48, PISO, dt 1/16).
fn example() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../example-projects/wind-tunnel/cases/cylinder-re100.toml")
        .display()
        .to_string()
}

/// The 64 x 256 wall layer of ADR 0154 K1 (first radial cell 0.00262 D) at dt 1/64.
const FINE: [&str; 6] = [
    "--set",
    "mesh.nx=64",
    "--set",
    "mesh.ny=256",
    "--set",
    "solver.dt=0.015625",
];

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn events(text: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(text)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[test]
fn w48v_b3_rcfd_check_json_gives_sv301_with_its_fix() {
    let case = example();
    let mut args = vec!["check", case.as_str(), "--json"];
    args.extend(FINE);
    let o = rcfd(&args);
    let v: Value = serde_json::from_slice(&o.stdout).expect("rcfd check --json prints JSON");
    let sv: Vec<&Value> = v["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"] == "SV301")
        .collect();
    println!("B3: exit {}, SV301 {sv:?}", code(&o));
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(sv.len(), 1);
    assert_eq!(sv[0]["severity"], "warning");
    assert_eq!(sv[0]["category"], "solver");
    assert_eq!(sv[0]["path"], "solver.corrector_control");
    assert_eq!(sv[0]["fix"]["value"], "adaptive");
    // the human form names the rule and the remedy on stdout
    let mut args = vec!["check", case.as_str()];
    args.extend(FINE);
    let o = rcfd(&args);
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(code(&o), 0);
    assert!(
        out.contains("SV301 [warning/solver] solver.corrector_control")
            && out.contains("corrector_control = \"adaptive\""),
        "{out}"
    );
    // the committed case has none
    let o = rcfd(&["check", case.as_str(), "--json"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(v["rules"].as_array().unwrap().iter().all(|r| r["id"] != "SV301"), "{v}");
}

#[test]
fn w48v_b4_rcfd_run_emits_sv301_as_a_warning_event_before_the_first_iteration() {
    let d = tmp("b4");
    let case = example();
    let out = d.join("out").display().to_string();
    let mut args = vec!["run", case.as_str(), "--status-json", "--out", out.as_str()];
    args.extend(FINE);
    args.extend(["--set", "solver.end_time=0.03125"]);
    let o = rcfd(&args);
    let ev = events(&o.stdout);
    let kinds: Vec<String> = ev
        .iter()
        .map(|e| {
            format!(
                "{}{}",
                e["event"].as_str().unwrap_or(""),
                e["rule"].as_str().map_or(String::new(), |r| format!(":{r}"))
            )
        })
        .collect();
    println!("B4: exit {}, events {kinds:?}", code(&o));
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let pos = |k: &str| kinds.iter().position(|x| x == k);
    let (start, warn, first) = (pos("start"), pos("warning:SV301"), pos("iteration"));
    assert!(
        start.is_some() && warn.is_some() && first.is_some() && start < warn && warn < first,
        "B4: order {kinds:?}"
    );
    assert!(String::from_utf8_lossy(&o.stderr).contains("solver.corrector_control"));
    let _ = std::fs::remove_dir_all(&d);
}

/// B5: the committed case, 20 steps, with the pre-run estimate (`pressure_lag_check = "warn"`, the default) and without it
/// (`"off"`): every iteration event equal in index, time, residuals, continuity and monitors; the stores byte-identical.
#[test]
fn w48v_b5_the_preflight_changes_no_result() {
    let d = tmp("b5");
    let case = example();
    let run = |lag: &str| -> (Vec<Value>, Vec<u8>) {
        let dir = d.join(lag);
        let (out, store) = (dir.join("out"), dir.join("fields"));
        let set = format!("solver.pressure_lag_check=\"{lag}\"");
        let o = rcfd(&[
            "run",
            case.as_str(),
            "--status-json",
            "--status-every",
            "1",
            "--status-monitors",
            "--out",
            out.to_str().unwrap(),
            "--store-dir",
            store.to_str().unwrap(),
            "--set",
            "solver.end_time=1.25",
            "--set",
            set.as_str(),
        ]);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        let its: Vec<Value> = events(&o.stdout)
            .into_iter()
            .filter(|e| e["event"] == "iteration")
            .map(|e| {
                serde_json::json!([
                    e["index"],
                    e["time"],
                    e["res_u"],
                    e["res_p"],
                    e["continuity"],
                    e["monitors"]
                ])
            })
            .collect();
        (its, std::fs::read(store.join("store.rts")).expect("store written"))
    };
    let (warn, store_warn) = run("warn");
    let (off, store_off) = run("off");
    println!(
        "B5: {} and {} iteration events, stores {} and {} bytes",
        warn.len(),
        off.len(),
        store_warn.len(),
        store_off.len()
    );
    assert_eq!(warn.len(), 20);
    assert!(warn == off, "B5: iteration events differ");
    assert!(store_warn == store_off, "B5: stores differ");
    let _ = std::fs::remove_dir_all(&d);
}
