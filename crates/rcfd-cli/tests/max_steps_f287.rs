//! ADR 0200 R-2: `rcfd run --max-steps n` caps the steps of the launch (state `step_limit`, exit 0) and says so in the
//! `start` event; 0 and a case with `chain.sequence` are usage errors (exit 2).

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f287_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The 12 x 12 cavity of the runner tests: tolerances 1e-14, so only a cap ends it.
const CASE: &str = r#"
title = "max-steps cavity"
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
tolerance_u = 1e-14
tolerance_p = 1e-14
"#;

const CHAIN: &str = r#"
[chain]
sequence = ["steady"]
[chain.stages.steady]
"#;

#[test]
fn r2_max_steps_caps_the_launch() {
    let d = tmp("r2");
    let case = d.join("cavity.toml");
    std::fs::write(&case, CASE).unwrap();
    let case = case.display().to_string();
    let out = d.join("out").display().to_string();
    let o = rcfd(&["run", &case, "--out", &out, "--quiet", "--max-steps", "0"]);
    assert_eq!(o.status.code(), Some(2), "{}", String::from_utf8_lossy(&o.stderr));
    let o = rcfd(&["run", &case, "--out", &out, "--max-steps", "5", "--status-json"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let events: Vec<Value> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let start = events.iter().find(|e| e["event"] == "start").expect("a start event");
    assert_eq!(start["info"]["max_steps"], 5, "{start}");
    let fin = events.iter().find(|e| e["event"] == "finish").expect("a finish event");
    assert_eq!(fin["state"], "step_limit", "{fin}");
    assert_eq!(fin["iterations"], 5, "{fin}");
    assert!(
        d.join("out/restart.rcfd").is_file(),
        "the checkpoint of a step_limit run"
    );
    eprintln!("R-2: --max-steps 5: {fin}");
    let chain = d.join("chain.toml");
    std::fs::write(&chain, format!("{CASE}{CHAIN}")).unwrap();
    let o = rcfd(&[
        "run",
        &chain.display().to_string(),
        "--out",
        &out,
        "--quiet",
        "--max-steps",
        "5",
    ]);
    assert_eq!(o.status.code(), Some(2), "{}", String::from_utf8_lossy(&o.stderr));
    eprintln!(
        "R-2: with chain.sequence: {}",
        String::from_utf8_lossy(&o.stderr).trim()
    );
    let _ = std::fs::remove_dir_all(&d);
}
