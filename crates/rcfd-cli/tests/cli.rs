//! Integration tests of the `rcfd` binary (F-081).

use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const CASE: &str = r#"
title = "cli cavity"
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
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
max_iterations = 300
tolerance_u = 1e-5
tolerance_p = 1e-5
"#;

#[test]
fn help_and_version_and_unknown_command() {
    let o = rcfd(&["--help"]);
    assert_eq!(o.status.code(), Some(0));
    let text = String::from_utf8_lossy(&o.stdout);
    for cmd in [
        "validate",
        "schema",
        "run",
        "check-mesh",
        "convert",
        "post",
        "serve",
        "EXIT CODES",
    ] {
        assert!(text.contains(cmd), "help lacks {cmd}");
    }
    assert_eq!(rcfd(&["run", "--help"]).status.code(), Some(0));
    assert!(String::from_utf8_lossy(&rcfd(&["--version"]).stdout).starts_with("rcfd "));
    assert_eq!(rcfd(&["frobnicate"]).status.code(), Some(2));
    assert_eq!(rcfd(&["run"]).status.code(), Some(2));
}

#[test]
fn validate_reports_line_numbers_and_exit_codes() {
    let d = tmp("validate");
    let good = d.join("good.toml");
    std::fs::write(&good, CASE).unwrap();
    let o = rcfd(&["validate", good.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let bad = d.join("bad.toml");
    std::fs::write(&bad, CASE.replace("nx = 8", "nx = \"eight\"")).unwrap();
    let o = rcfd(&["validate", bad.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(3));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("mesh.nx") && err.contains("line 5"), "{err}");
    let sem = d.join("sem.toml");
    std::fs::write(
        &sem,
        format!("{CASE}\n[reports.r]\ntype = \"force\"\npatch = \"nowhere\"\n"),
    )
    .unwrap();
    assert_eq!(rcfd(&["validate", sem.to_str().unwrap()]).status.code(), Some(3));
    assert_eq!(
        rcfd(&["validate", d.join("missing.toml").to_str().unwrap()])
            .status
            .code(),
        Some(3)
    );
}

#[test]
fn schema_is_a_json_schema_document() {
    let d = tmp("schema");
    let f = d.join("case.schema.json");
    assert_eq!(rcfd(&["schema", "--out", f.to_str().unwrap()]).status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
    assert!(v["properties"]["mesh"]["properties"]["nx"].is_object());
    let o = rcfd(&["schema"]);
    let v2: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v, v2);
}

#[test]
fn run_writes_results_and_post_rebuilds_reports() {
    let d = tmp("run");
    let case = d.join("c.toml");
    std::fs::write(&case, CASE).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap(), "--quiet"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    for f in ["solution.vtu", "monitors.json", "report.html", "monitors.csv"] {
        assert!(out.join(f).is_file(), "{f} missing");
    }
    let o = rcfd(&["post", out.to_str().unwrap(), "--format", "csv"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()
        .unwrap()
        .contains("res_u"));
    assert!(String::from_utf8_lossy(&rcfd(&["post", out.to_str().unwrap()]).stdout).contains("<polyline"));
    assert_eq!(rcfd(&["post", d.to_str().unwrap()]).status.code(), Some(1));

    // iteration cap reached without convergence -> exit code 5
    let o = rcfd(&[
        "run",
        case.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--quiet",
        "--max-iterations",
        "3",
    ]);
    assert_eq!(o.status.code(), Some(5));
}

#[test]
fn check_mesh_and_convert_round_trip() {
    let d = tmp("mesh");
    let case = d.join("c.toml");
    std::fs::write(&case, CASE).unwrap();
    let o = rcfd(&["check-mesh", case.to_str().unwrap(), "--json"]);
    assert_eq!(o.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["n_cells"], 64);
    let foam = d.join("foam");
    let o = rcfd(&["convert", case.to_str().unwrap(), foam.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let o = rcfd(&["check-mesh", foam.to_str().unwrap(), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["n_cells"], 64);
    let vtu = d.join("m.vtu");
    assert_eq!(
        rcfd(&["convert", foam.to_str().unwrap(), vtu.to_str().unwrap()])
            .status
            .code(),
        Some(0)
    );
    assert!(std::fs::read_to_string(vtu).unwrap().contains("UnstructuredGrid"));
    assert_eq!(
        rcfd(&["check-mesh", d.join("nope.msh").to_str().unwrap()])
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn check_mesh_reports_extended_measures() {
    let d = tmp("measures");
    let case = d.join("c.toml");
    std::fs::write(&case, CASE).unwrap();
    let o = rcfd(&["check-mesh", case.to_str().unwrap(), "--json"]);
    assert_eq!(o.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    // 2D cavity: all faces are rectangles, no warp, and the old keys are still there
    assert!(v["measures"]["max_warp"].as_f64().unwrap() < 1e-9, "{v}");
    assert!((v["measures"]["min_face_angle"].as_f64().unwrap() - 90.0).abs() < 1e-9);
    assert!(v["poor_list"].as_array().unwrap().is_empty());
    assert!(v["max_non_orthogonality"].is_number() && v["warnings"].is_array());
    let o = rcfd(&["check-mesh", case.to_str().unwrap()]);
    let t = String::from_utf8_lossy(&o.stdout);
    assert!(
        t.contains("warp max") && t.contains("inverse orthogonal quality"),
        "{t}"
    );
}

#[test]
fn serve_over_stdio_answers_requests() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["serve", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"service.info"}}"#).unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["result"]["protocol"], "rcfd-service/1");
}

#[test]
fn call_talks_to_a_running_service() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let mut srv = Command::new(env!("CARGO_BIN_EXE_rcfd"))
        .args(["serve", "--addr", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(srv.stdout.take().unwrap()).read_line(&mut line).unwrap();
    let addr = line.trim().rsplit(' ').next().unwrap().to_string();
    let o = rcfd(&["call", "--addr", &addr, "service.info"]);
    assert_eq!(o.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["protocol"], "rcfd-service/1");
    let o = rcfd(&["call", "--addr", &addr, "run.status", r#"{"session":"zz"}"#]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("1001"));
    srv.kill().unwrap();
    srv.wait().unwrap();
}
