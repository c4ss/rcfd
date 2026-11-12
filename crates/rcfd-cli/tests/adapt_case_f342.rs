//! F-342 (ADR 0262) X1 and X2: an adapted cavity through the `rcfd` binary, and located errors of `[mesh.adapt]` through
//! `rcfd validate`. The case text is written to a temporary folder.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::path::PathBuf;
use std::process::{Command, Output};

/// X1 of ADR 0262: the binary's monitor values against an in-process run of the same text.
const SAME: f64 = 1e-12;

/// T-S of ADR 0262: the 8 x 8 cavity with an empty `[mesh.adapt]` table (ADR 0242's loop).
const T_S: &str = r#"title = "F-342 T-S: adapted Re 100 cavity on an 8 x 8 base"

[mesh]
generator = "cavity_2d"
nx = 8
ny = 8
lx = 1.0
ly = 1.0

[mesh.adapt]

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
simplec_non_orthogonal_compensation = true
"#;

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f342_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The last value of each named column of a `monitors.csv`.
fn last_values(csv: &str, names: &[&str]) -> Vec<f64> {
    let mut lines = csv.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let cols: Vec<usize> = names
        .iter()
        .map(|n| {
            head.iter()
                .position(|h| h == n)
                .unwrap_or_else(|| panic!("no column {n}: {head:?}"))
        })
        .collect();
    let mut out = vec![f64::NAN; names.len()];
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        for (k, &c) in cols.iter().enumerate() {
            if let Some(v) = f.get(c).filter(|v| !v.is_empty()) {
                out[k] = v.parse().unwrap();
            }
        }
    }
    out
}

#[test]
fn x1_an_adapted_cavity_through_the_binary_equals_the_in_process_run() {
    let d = tmp("x1");
    let case = d.join("adapt.toml");
    std::fs::write(&case, T_S).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap(), "--quiet"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.join("solution.vtu").is_file(), "solution.vtu missing");
    let names = ["res_u", "res_p", "continuity"];
    let cli = last_values(&std::fs::read_to_string(out.join("monitors.csv")).unwrap(), &names);
    let mut s = Simulation::new(&Case::parse_toml(T_S).unwrap()).unwrap();
    s.run(10);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let lib: Vec<f64> = names
        .iter()
        .map(|n| *s.monitors.get(n).unwrap().y.last().unwrap())
        .collect();
    let diff: Vec<f64> = cli.iter().zip(&lib).map(|(a, b)| (a - b).abs() / b.abs()).collect();
    println!(
        "X1: binary {cli:?}, in process {lib:?} ({} cells, cycles {:?}), relative differences {diff:?} (limit {SAME:e})",
        s.geom.n_cells(),
        s.adaptive_cycles()
    );
    assert!(diff.iter().all(|x| *x <= SAME), "X1 {diff:?}");
}

#[test]
fn x2_validate_locates_the_errors_of_the_table() {
    let d = tmp("x2");
    for (from, to, path) in [
        ("algorithm = \"simplec\"", "algorithm = \"piso\"", "solver.algorithm"),
        (
            "[mesh.adapt]\n",
            "[mesh.adapt]\nrefine_fraction = 1.5\n",
            "mesh.adapt.refine_fraction",
        ),
    ] {
        let bad = T_S.replacen(from, to, 1);
        assert_ne!(bad, T_S, "set-up {path}");
        let f = d.join("bad.toml");
        std::fs::write(&f, &bad).unwrap();
        let o = rcfd(&["validate", f.to_str().unwrap()]);
        let err = String::from_utf8_lossy(&o.stderr);
        println!("X2 {path}: exit {:?}: {}", o.status.code(), err.trim());
        assert_eq!(o.status.code(), Some(3), "{err}");
        assert!(err.contains(path), "{err}");
    }
}
