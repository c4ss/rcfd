//! ADR 0172 (job 49J) through the `rcfd` binary: LP3 (the same SV302 text from `rcfd check --json`, `rcfd run` as a `warning`
//! event after `start`, and a chain run), LW7 (exit 0: a warning, not an error), LW8 (every shipped example keeps its
//! `rcfd check` exit status; the SV302 carriers are printed), LS-CLI (SV301 at the settled step through the binary).

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_w49j_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

fn events(text: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(text)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// A 16 x 20 x 1 channel, geometric wall clustering, sheared by 70 degrees, as polyMesh.
fn sheared_mesh(dir: &Path) -> PathBuf {
    use rcfd_mesh::gen::{channel, warp, Grading};
    let m = channel(
        [16, 20, 1],
        [2.0, 1.0, 1.0],
        [Grading::Uniform, Grading::Symmetric(1.0e4), Grading::Uniform],
        true,
    )
    .unwrap();
    let t = 70f64.to_radians().tan();
    let m = warp(&m, |p| [p[0] + t * p[1], p[1], p[2]]).unwrap();
    let d = dir.join("mesh");
    rcfd_io::openfoam::write_dir(&m, &d).unwrap();
    d
}

fn case_text(mesh: &Path, extra_solver: &str, tail: &str) -> String {
    let file = mesh.display().to_string().replace('\\', "/");
    format!(
        r#"
title = "w49j sheared channel"
[mesh]
source = "file"
file = "{file}"
[physics]
nu = 0.01
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 2
{extra_solver}
{tail}
"#
    )
}

fn sv302(v: &Value) -> Vec<Value> {
    v.as_array()
        .map(|a| a.iter().filter(|r| r["id"] == "SV302").cloned().collect())
        .unwrap_or_default()
}

#[test]
fn lp3_the_same_sv302_text_from_check_run_and_chain() {
    let dir = tmp("lp3");
    let mesh = sheared_mesh(&dir);
    let case = dir.join("case.toml");
    std::fs::write(&case, case_text(&mesh, "", "")).unwrap();
    let c = case.display().to_string();
    // check
    let o = rcfd(&["check", &c, "--json"]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let from_check = sv302(&v["rules"]);
    assert_eq!(from_check.len(), 1, "{v}");
    assert_eq!(from_check[0]["severity"], "warning");
    let text = from_check[0]["message"].as_str().unwrap().to_string();
    // run
    let out = dir.join("out");
    let o = rcfd(&["run", &c, "--status-json", "--out", &out.display().to_string()]);
    // exit 5: stopped after 2 iterations without meeting the tolerances; a refusal would be 3
    assert!(matches!(code(&o), 0 | 5), "{}", String::from_utf8_lossy(&o.stderr));
    let ev = events(&o.stdout);
    let start = ev.iter().position(|e| e["event"] == "start").expect("start");
    let w = ev
        .iter()
        .position(|e| e["event"] == "warning" && e["rule"] == "SV302")
        .expect("a warning event SV302");
    let first_iter = ev.iter().position(|e| e["event"] == "iteration").expect("iteration");
    assert!(start < w && w < first_iter, "order {start} {w} {first_iter}");
    assert_eq!(ev[w]["message"].as_str().unwrap(), text);
    let info = ev[start]["info"]["warnings"].as_array().expect("start.info.warnings");
    assert_eq!(info.iter().filter(|w| w["rule"] == "SV302").count(), 1, "{info:?}");
    // chain
    let chain = dir.join("chain.toml");
    std::fs::write(
        &chain,
        case_text(&mesh, "", "[chain]\nsequence = [\"a\"]\n[chain.stages.a]\n"),
    )
    .unwrap();
    let outc = dir.join("outc");
    let o = rcfd(&[
        "run",
        &chain.display().to_string(),
        "--status-json",
        "--out",
        &outc.display().to_string(),
    ]);
    // exit 5: stopped after 2 iterations without meeting the tolerances; a refusal would be 3
    assert!(matches!(code(&o), 0 | 5), "{}", String::from_utf8_lossy(&o.stderr));
    let ev = events(&o.stdout);
    let w = ev
        .iter()
        .find(|e| e["event"] == "warning" && e["rule"] == "SV302")
        .unwrap_or_else(|| panic!("chain: no SV302 in {ev:?}"));
    assert_eq!(w["message"].as_str().unwrap(), text);
}

#[test]
fn lw8_every_shipped_example_keeps_its_check_exit_and_the_sv302_carriers_are_listed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example-projects");
    let mut carriers = Vec::new();
    let mut n = 0;
    for project in ["quick-start", "wind-tunnel"] {
        let dir = root.join(project).join("cases");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        files.sort();
        for f in files {
            let o = rcfd(&["check", &f.display().to_string(), "--json"]);
            n += 1;
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            println!("LW8 {project}/{name}: exit {}", code(&o));
            assert_eq!(code(&o), 0, "{name}: {}", String::from_utf8_lossy(&o.stderr));
            let v: Value = serde_json::from_slice(&o.stdout).unwrap();
            if !sv302(&v["rules"]).is_empty() {
                carriers.push(format!("{project}/{name}"));
            }
        }
    }
    println!("LW8: {n} examples checked; SV302 carriers: {carriers:?}");
    assert!(n >= 12);
}

#[test]
fn ls_cli_sv301_comes_from_the_settled_step() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../example-projects/wind-tunnel/cases/cylinder-re100.toml")
        .display()
        .to_string();
    let fine = [
        "--set",
        "mesh.nx=64",
        "--set",
        "mesh.ny=256",
        "--set",
        "solver.dt=0.015625",
    ];
    let sv301 = |extra: &[&str]| {
        let mut args = vec!["check", example.as_str(), "--json"];
        args.extend(fine);
        args.extend(extra);
        let o = rcfd(&args);
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        v["rules"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["id"] == "SV301")
            .count()
    };
    assert_eq!(sv301(&[]), 1, "the fixed K1 step is unchanged");
    assert_eq!(
        sv301(&["--set", "solver.max_courant=0.01"]),
        0,
        "the Courant controller cuts the step"
    );
}
