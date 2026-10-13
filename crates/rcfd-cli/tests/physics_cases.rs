//! Turbulence and compressible example cases through the `rcfd` binary.

use std::path::PathBuf;
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn case(rel: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../cases")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_phys_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run_example(rel: &str, name: &str) -> (Output, PathBuf) {
    let out = tmp(name);
    let o = rcfd(&["run", &case(rel), "--out", out.to_str().unwrap(), "--quiet"]);
    (o, out)
}

#[test]
fn every_physics_example_validates() {
    for rel in [
        "channel/kepsilon_channel.toml",
        "channel/kepsilon_inlet.toml",
        "channel/sst_channel.toml",
        "sod/sod.toml",
        "wedge/wedge.toml",
    ] {
        let o = rcfd(&["validate", &case(rel)]);
        assert_eq!(
            o.status.code(),
            Some(0),
            "{rel}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
}

#[test]
fn sod_runs_and_writes_compressible_fields() {
    let (o, out) = run_example("sod/sod.toml", "sod");
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let vtu = std::fs::read_to_string(out.join("solution.vtu")).unwrap();
    for f in ["\"rho\"", "\"mach\"", "\"T\""] {
        assert!(vtu.contains(f), "vtu lacks {f}");
    }
    assert!(out.join("monitors.csv").is_file());
}

#[test]
fn wedge_runs_to_the_end_time() {
    let (o, out) = run_example("wedge/wedge.toml", "wedge");
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.join("solution.vtu").is_file());
}

#[test]
fn kepsilon_channel_converges_and_writes_turbulence_fields() {
    let (o, out) = run_example("channel/kepsilon_channel.toml", "kechannel");
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let vtu = std::fs::read_to_string(out.join("solution.vtu")).unwrap();
    for f in ["\"k\"", "\"epsilon\"", "\"nut\"", "\"y_plus\""] {
        assert!(vtu.contains(f), "vtu lacks {f}");
    }
    let csv = std::fs::read_to_string(out.join("monitors.csv")).unwrap();
    assert!(csv.lines().next().unwrap().contains("yplus_lower"));
}

#[test]
fn sst_channel_converges_to_unit_wall_y_plus_and_writes_omega() {
    let (o, out) = run_example("channel/sst_channel.toml", "sstchannel");
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let vtu = std::fs::read_to_string(out.join("solution.vtu")).unwrap();
    for f in ["\"k\"", "\"omega\"", "\"nut\"", "\"y_plus\""] {
        assert!(vtu.contains(f), "vtu lacks {f}");
    }
    let csv = std::fs::read_to_string(out.join("monitors.csv")).unwrap();
    let head: Vec<&str> = csv.lines().next().unwrap().split(',').collect();
    let last: Vec<&str> = csv.lines().last().unwrap().split(',').collect();
    let col = |n: &str| last[head.iter().position(|h| *h == n).unwrap()].parse::<f64>().unwrap();
    assert!((col("yplus_lower") - 1.0).abs() < 0.01, "y+ {}", col("yplus_lower"));
    // bulk velocity: 18.17 (DNS 17.4, +4.4 percent, see the F-043 roadmap note)
    assert!((col("u_mean") / 17.4 - 1.0).abs() < 0.10, "bulk {}", col("u_mean"));
}

#[test]
fn kepsilon_inlet_example_runs() {
    let (o, out) = run_example("channel/kepsilon_inlet.toml", "keinlet");
    // converged (0) or iteration cap (5); anything else is a failure
    assert!(
        matches!(o.status.code(), Some(0) | Some(5)),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(out.join("solution.vtu").is_file());
}

#[test]
fn unsupported_combinations_are_reported_with_line_numbers() {
    let d = tmp("diag");
    let text = std::fs::read_to_string(case("sod/sod.toml")).unwrap();
    // rk_order 1 with a limited reconstruction
    let bad = d.join("bad.toml");
    std::fs::write(&bad, text.replace("rk_order = 3", "rk_order = 1")).unwrap();
    let o = rcfd(&["validate", bad.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(3));
    let err = String::from_utf8_lossy(&o.stderr);
    let line = text.lines().position(|l| l.starts_with("rk_order")).unwrap() + 1;
    assert!(
        err.contains("compressible.rk_order") && err.contains(&format!("line {line}")),
        "{err}"
    );
    // k-epsilon with a transient algorithm
    let k = std::fs::read_to_string(case("channel/kepsilon_inlet.toml")).unwrap();
    let bad = d.join("bad2.toml");
    std::fs::write(&bad, k.replace("algorithm = \"simplec\"", "algorithm = \"piso\"")).unwrap();
    let o = rcfd(&["validate", bad.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&o.stderr).contains("turbulence.model"));
    // an error found only when the simulation is built still gets a line (pinned cell outside the mesh)
    let bad = d.join("bad3.toml");
    std::fs::write(&bad, k.replace("[solver]\n", "[solver]\np_ref_cell = 99999\n")).unwrap();
    let o = rcfd(&["validate", bad.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(3));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("solver.p_ref_cell") && err.contains("line "), "{err}");
}
