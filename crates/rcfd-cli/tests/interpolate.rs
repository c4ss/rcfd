//! `rcfd run --interpolate-*` (F-115): start a fine-mesh case from a coarse-mesh checkpoint.
//! Criteria fixed up front: exit 0 and the first iteration event of the interpolated run has a
//! lower momentum residual than the uniform start; incomplete or conflicting flags are exit 2;
//! a checkpoint from another mesh is exit 1.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_interp_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn case_file(dir: &Path, name: &str, n: usize) -> String {
    let text = format!(
        r#"
title = "interp {n}"
[mesh]
generator = "cavity_2d"
nx = {n}
ny = {n}
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
max_iterations = 1000
tolerance_u = 1e-6
tolerance_p = 1e-6
"#
    );
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.display().to_string()
}

fn first_iteration_res_u(o: &Output) -> f64 {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v["event"] == "iteration")
        .map(|v| v["res_u"].as_f64().unwrap())
        .expect("an iteration event")
}

#[test]
fn interpolated_start_has_lower_first_residual_and_bad_usage_is_rejected() {
    let d = tmp("run");
    let coarse = case_file(&d, "coarse.toml", 12);
    let fine = case_file(&d, "fine.toml", 24);
    let ck = d.join("coarse.ckpt").display().to_string();
    let o = rcfd(&[
        "run",
        &coarse,
        "--quiet",
        "--checkpoint",
        &ck,
        "--out",
        &d.join("o1").display().to_string(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));

    let out = |s: &str| d.join(s).display().to_string();
    let uni = rcfd(&["run", &fine, "--quiet", "--status-json", "--out", &out("o2")]);
    assert_eq!(uni.status.code(), Some(0));
    let int = rcfd(&[
        "run",
        &fine,
        "--quiet",
        "--status-json",
        "--out",
        &out("o3"),
        "--interpolate-case",
        &coarse,
        "--interpolate-checkpoint",
        &ck,
        "--interpolate-mode",
        "conservative",
    ]);
    assert_eq!(int.status.code(), Some(0), "{}", String::from_utf8_lossy(&int.stderr));
    let (a, b) = (first_iteration_res_u(&int), first_iteration_res_u(&uni));
    println!("first iteration res_u: interpolated {a:.4e}, uniform {b:.4e}");
    assert!(a < b);

    // only one of the pair, an unknown mode, a combination with --continue-from: usage errors
    let lone = rcfd(&["run", &fine, "--quiet", "--interpolate-case", &coarse]);
    assert_eq!(lone.status.code(), Some(2));
    let mode = rcfd(&[
        "run",
        &fine,
        "--quiet",
        "--interpolate-case",
        &coarse,
        "--interpolate-checkpoint",
        &ck,
        "--interpolate-mode",
        "fancy",
    ]);
    assert_eq!(mode.status.code(), Some(2));
    let both = rcfd(&[
        "run",
        &fine,
        "--quiet",
        "--interpolate-case",
        &coarse,
        "--interpolate-checkpoint",
        &ck,
        "--continue-from",
        &ck,
    ]);
    assert_eq!(both.status.code(), Some(2));

    // checkpoint of the 12x12 mesh with a 16x16 source description
    let other = case_file(&d, "other.toml", 16);
    let wrong = rcfd(&[
        "run",
        &fine,
        "--quiet",
        "--interpolate-case",
        &other,
        "--interpolate-checkpoint",
        &ck,
    ]);
    assert_eq!(wrong.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("not written for the mesh"));
}
