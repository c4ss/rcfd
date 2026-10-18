//! ADR 0067: the `rcfd map` command (operating-map automation) on a small cavity.

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("map_w32c").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const CAVITY: &str = r#"title = "cavity"
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
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

#[test]
fn map_runs_a_chain_writes_tables_and_is_repeatable() {
    let d = tmp("chain");
    let case = d.join("cavity.toml");
    std::fs::write(&case, CAVITY).unwrap();
    let mut texts = Vec::new();
    for k in 0..2 {
        let out = d.join(format!("run{k}"));
        let o = rcfd(&[
            "map",
            case.to_str().unwrap(),
            "--axis",
            "physics.nu=0.02,0.015,0.01",
            "--out",
            out.to_str().unwrap(),
            "--quiet",
        ]);
        assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
        let csv = std::fs::read_to_string(out.join("map.csv")).unwrap();
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.lines().next().unwrap().contains("lid_drag"));
        let j: Value = serde_json::from_str(&std::fs::read_to_string(out.join("map.json")).unwrap()).unwrap();
        let pts = j["points"].as_array().unwrap();
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0]["restart"], "cold");
        assert_eq!(pts[1]["restart"], "warm");
        assert!(pts.iter().all(|p| p["status"] == "converged"));
        texts.push((csv, std::fs::read_to_string(out.join("map.json")).unwrap()));
    }
    assert_eq!(texts[0], texts[1], "two runs give byte-identical tables");
    // --cold never restarts from the previous point
    let out = d.join("cold");
    let o = rcfd(&[
        "map",
        case.to_str().unwrap(),
        "--axis",
        "physics.nu=0.02:0.01:3",
        "--cold",
        "--out",
        out.to_str().unwrap(),
        "--quiet",
    ]);
    assert_eq!(o.status.code(), Some(0));
    let j: Value = serde_json::from_str(&std::fs::read_to_string(out.join("map.json")).unwrap()).unwrap();
    assert!(j["points"].as_array().unwrap().iter().all(|p| p["restart"] == "cold"));
}

#[test]
fn map_usage_and_bad_axes_are_reported() {
    let d = tmp("bad");
    let case = d.join("cavity.toml");
    std::fs::write(&case, CAVITY).unwrap();
    let c = case.to_str().unwrap();
    assert_eq!(rcfd(&["map", c]).status.code(), Some(2));
    assert_eq!(rcfd(&["map", c, "--axis", "novalue"]).status.code(), Some(2));
    let o = rcfd(&[
        "map",
        c,
        "--axis",
        "physics.nosuch=1,2",
        "--out",
        d.join("o").to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(3), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stderr).contains("physics.nosuch"));
    assert!(!d.join("o").exists());
}
