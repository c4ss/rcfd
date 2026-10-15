//! Command-line surface of partitioning, sampling and inflation (ADR 0021: D9, D10, P9, N1 to N5, W3).

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_wiring_cli_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn write(dir: &std::path::Path, name: &str, text: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p.to_string_lossy().into_owned()
}

const BCS: &str = "[boundaries.xmin]\nvelocity = \"fixed\"\n[boundaries.xmax]\nvelocity = \"fixed\"\n[boundaries.ymin]\nvelocity = \"fixed\"\n[boundaries.ymax]\nvelocity = \"fixed\"\n";

fn linear(extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"box_2d\"\nnx = 12\nny = 10\nlx = 4.0\nly = 1.0\n[physics]\nnu = 0.05\nrho = 1.2\n[initial]\nvelocity_expression_x = \"1 [1/s] * x + 2 [1/s] * y\"\nvelocity_expression_y = \"3 [1/s] * x - 1 [1/s] * y\"\n{BCS}[solver]\nalgorithm = \"simplec\"\nmax_iterations = 5\n{extra}"
    )
}

#[test]
fn partition_json_keys_and_exit_codes() {
    let d = tmp("part");
    let case = write(&d, "c.toml", &linear(""));
    let out = d.join("part.vtu");
    let o = rcfd(&[
        "partition",
        &case,
        "--method",
        "bisection",
        "--parts",
        "4",
        "--json",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    for k in [
        "method",
        "n_parts",
        "n_cells",
        "part_cells",
        "part_weights",
        "imbalance",
        "edge_cut",
        "ghost_cells",
    ] {
        assert!(v.get(k).is_some(), "{k}");
    }
    assert_eq!(v["n_parts"], 4);
    assert_eq!(v["n_cells"], 120);
    let sum: u64 = v["part_cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap())
        .sum();
    assert_eq!(sum, 120);
    let vtu = std::fs::read_to_string(&out).unwrap();
    assert!(vtu.contains("Name=\"partition\""));
    // bandwidth report
    let o = rcfd(&[
        "partition",
        &case,
        "--method",
        "inertial",
        "--parts",
        "3",
        "--reorder",
        "--json",
    ]);
    assert_eq!(code(&o), 0);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(v["bandwidth"]["natural"].is_u64() && v["bandwidth"]["rcm"].is_u64());
    // D9/D10 invalid inputs: located, exit 3
    let o = rcfd(&["partition", &case, "--method", "bisection", "--parts", "0"]);
    assert_eq!(code(&o), 3);
    assert!(String::from_utf8_lossy(&o.stderr).contains("decomposition.parts"));
    let o = rcfd(&["partition", &case, "--method", "bisection", "--parts", "100000"]);
    assert_eq!(code(&o), 3);
    assert!(String::from_utf8_lossy(&o.stderr).contains("decomposition.parts"));
    let o = rcfd(&["partition", &case, "--parts", "4", "--method", "nonsense"]);
    assert_eq!(code(&o), 3);
    assert!(String::from_utf8_lossy(&o.stderr).contains("decomposition.method"));
    let o = rcfd(&["partition"]);
    assert_eq!(code(&o), 2);
}

#[test]
fn case_decomposition_section_is_validated_by_rcfd_validate() {
    let d = tmp("val");
    let bad = write(
        &d,
        "bad.toml",
        &linear("[decomposition]\nmethod = \"bisection\"\nparts = 0\n"),
    );
    let o = rcfd(&["validate", &bad]);
    assert_eq!(code(&o), 3);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(all.contains("decomposition.parts"), "{all}");
    let good = write(
        &d,
        "good.toml",
        &linear("[decomposition]\nmethod = \"bisection\"\nparts = 4\n"),
    );
    assert_eq!(code(&rcfd(&["validate", &good])), 0);
}

#[test]
fn w3_validate_reports_the_ignored_vd_conductivity() {
    let d = tmp("w3");
    let case = write(
        &d,
        "vd.toml",
        r#"
[mesh]
generator = "nozzle_2d"
nx = 20
ny = 4
lx = 2.0
ly = 0.1
throat_height = 0.05
exit_height = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
[energy]
enabled = true
[boundaries.xmin]
inlet = "stagnation"
total_pressure_value = 101325.0
temperature_value = 300.0
velocity = "zero_gradient"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 95000.0
temperature = "zero_gradient"
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
max_iterations = 5
[reports.q]
type = "heat_flow"
patch = "ymin"
conductivity = 2.0
"#,
    );
    let o = rcfd(&["validate", &case]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(code(&o), 3, "{all}");
    assert!(all.contains("reports.q.conductivity"), "{all}");
}

#[test]
fn p9_sample_command_matches_the_run_output() {
    let d = tmp("sample");
    // box_2d spans z in [0, lx/nx] = [0, 1/3]
    let z = "0.1";
    let case = write(&d, "c.toml", &linear(""));
    let l = format!("0.8,0.3,{z}:3.2,0.7,{z}");
    let out = d.join("direct.csv");
    let o = rcfd(&[
        "sample",
        &case,
        "--max-iterations",
        "1",
        "--line",
        &l,
        "--points",
        "20",
        "--fields",
        "ux,uy",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let direct = std::fs::read_to_string(&out).unwrap();
    assert_eq!(direct.lines().next().unwrap(), "s,x,y,z,ux,uy,inside");
    assert_eq!(direct.lines().count(), 21);

    // the same sample defined in the case, written by `rcfd run`
    let with = write(
        &d,
        "w.toml",
        &linear(&format!(
            "[output]\ndirectory = \"res\"\n[postprocessing.samples.s1]\ntype = \"line\"\np0 = [0.8, 0.3, {z}]\np1 = [3.2, 0.7, {z}]\npoints = 20\nfields = [\"ux\", \"uy\"]\n"
        )),
    );
    let o = rcfd(&[
        "run",
        &with,
        "--max-iterations",
        "1",
        "--out",
        d.join("res").to_str().unwrap(),
    ]);
    assert!(
        code(&o) == 0 || code(&o) == 5,
        "{} {}",
        code(&o),
        String::from_utf8_lossy(&o.stderr)
    );
    let ran = std::fs::read_to_string(d.join("res").join("sample_s1.csv")).unwrap();
    assert_eq!(ran, direct);

    // usage and invalid inputs
    assert_eq!(code(&rcfd(&["sample", &case])), 2);
    let bad = write(
        &d,
        "b.toml",
        &linear(
            "[postprocessing.samples.s1]\ntype = \"line\"\np0 = [0.0, 0.0, 0.0]\np1 = [0.0, 0.0, 0.0]\npoints = 5\n",
        ),
    );
    let o = rcfd(&["sample", &bad]);
    assert_eq!(code(&o), 3);
    assert!(String::from_utf8_lossy(&o.stderr).contains("postprocessing.samples.s1.p1"));
}

#[test]
fn inflate_command_reports_and_fails_cleanly() {
    let d = tmp("infl");
    let case = write(
        &d,
        "c.toml",
        "[mesh]\ngenerator = \"box_2d\"\nnx = 8\nny = 4\nlx = 2.0\nly = 1.0\n",
    );
    let out = d.join("i.vtu");
    let o = rcfd(&[
        "inflate",
        &case,
        "--patch",
        "ymax",
        "--layers",
        "6",
        "--first",
        "0.01",
        "--json",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    for k in ["patch", "sizing", "cells_before", "cells_after", "report"] {
        assert!(v.get(k).is_some(), "{k}");
    }
    let (b, a) = (v["cells_before"].as_u64().unwrap(), v["cells_after"].as_u64().unwrap());
    assert_eq!(a - b, 48);
    let vtu = std::fs::read_to_string(&out).unwrap();
    assert!(vtu.contains(&format!("NumberOfCells=\"{a}\"")));
    // y+ target
    let o = rcfd(&[
        "inflate", &case, "--patch", "ymax", "--layers", "3", "--y-plus", "0.8", "--u-tau", "0.05", "--nu", "1e-5",
        "--json",
    ]);
    assert_eq!(code(&o), 0);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let h = v["report"]["first_height_min"].as_f64().unwrap();
    assert!((h - 3.2e-4).abs() / 3.2e-4 <= 1e-12, "{h}");
    // errors
    let o = rcfd(&[
        "inflate", &case, "--patch", "nowhere", "--layers", "3", "--first", "0.01",
    ]);
    assert_eq!(code(&o), 1);
    assert!(String::from_utf8_lossy(&o.stderr).contains("nowhere"));
    assert_eq!(
        code(&rcfd(&[
            "inflate", &case, "--patch", "ymax", "--layers", "0", "--first", "0.01"
        ])),
        2
    );
    assert_eq!(code(&rcfd(&["inflate", &case, "--patch", "ymax", "--layers", "3"])), 2);
    assert_eq!(
        code(&rcfd(&[
            "inflate", &case, "--patch", "ymax", "--layers", "3", "--first", "0.01", "--growth", "0"
        ])),
        1
    );
}
