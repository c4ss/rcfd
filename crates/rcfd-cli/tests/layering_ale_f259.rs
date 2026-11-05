//! F-259 (ADR 0191) X1 and X2: a layered compressible piston through the `rcfd` binary. The case text is written to a
//! temporary folder (no case file is added to the repository).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::path::PathBuf;
use std::process::{Command, Output};

/// X1 of ADR 0191: the binary's report values against an in-process run of the same text.
const SAME: f64 = 1e-12;

/// P-X of ADR 0191: a 20-layer piston, one compression stroke of period 4 (peak piston Mach 0.2), with three reports.
const P_X: &str = r#"title = "F-259 P-X: layered compressible piston"

[mesh]
generator = "box_3d"
nx = 20
ny = 1
nz = 1
lx = 1.0
ly = 0.1
lz = 0.1

[solver]
algorithm = "compressible"
end_time = 4.0

[compressible]
flux = "hllc"
reconstruction = "tvd_van_leer"
rk_order = 3
cfl = 0.4
gamma = 1.4

[compressible.initial.left]
density = 1.0
pressure = 1.0

[dynamic_mesh]
enabled = true
deformation = "layering"
moving_patches = ["xmax"]
displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / 4 [s]))"

[reports.rho_total]
type = "volume_integral"
field = "rho"

[reports.p_mean]
type = "volume_average"
field = "p"

[reports.p_max]
type = "maximum"
field = "p"
"#;

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f259_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The last value of each named column of a `monitors.csv` (the report rows are those of the transient times).
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
fn x1_layered_piston_through_the_binary_equals_the_in_process_run() {
    let d = tmp("x1");
    let case = d.join("piston.toml");
    std::fs::write(&case, P_X).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap(), "--quiet"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let names = ["rho_total", "p_mean", "p_max"];
    let cli = last_values(&std::fs::read_to_string(out.join("monitors.csv")).unwrap(), &names);
    let mut s = Simulation::new(&Case::parse_toml(P_X).unwrap()).unwrap();
    let steps = s.run(1_000_000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let lib: Vec<f64> = names
        .iter()
        .map(|n| *s.monitors.get(n).unwrap().y.last().unwrap())
        .collect();
    let vtu = std::fs::read_to_string(out.join("solution.vtu")).unwrap();
    let cells: usize = vtu
        .split("NumberOfCells=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .and_then(|n| n.parse().ok())
        .expect("NumberOfCells in solution.vtu");
    let diff: Vec<f64> = cli.iter().zip(&lib).map(|(a, b)| (a - b).abs() / b.abs()).collect();
    println!(
        "X1: binary {cli:?}, in process {lib:?} ({steps} steps), relative differences {diff:?} (limit {SAME:e}); \
         solution.vtu {cells} cells, in process {}",
        s.geom.n_cells()
    );
    assert!(diff.iter().all(|x| *x <= SAME), "X1 {diff:?}");
    assert_eq!(cells, s.geom.n_cells(), "X1 cells");
    assert!(cells < 20, "X1: layers must have been removed ({cells} cells)");
}

#[test]
fn x2_validate_locates_layering_errors_with_line_numbers() {
    let d = tmp("x2");
    for (from, to, path) in [
        (
            "moving_patches = [\"xmax\"]",
            "moving_patches = [\"xmin\"]",
            "dynamic_mesh.moving_patches",
        ),
        (
            "deformation = \"layering\"",
            "deformation = \"layering\"\nmotion = \"six_dof\"",
            "dynamic_mesh.motion",
        ),
    ] {
        let text = P_X.replace(from, to);
        assert_ne!(text, P_X, "set-up {path}");
        let f = d.join("bad.toml");
        std::fs::write(&f, &text).unwrap();
        let o = rcfd(&["validate", f.to_str().unwrap()]);
        let err = String::from_utf8_lossy(&o.stderr);
        println!("X2 {path}: exit {:?}: {}", o.status.code(), err.trim());
        assert_eq!(o.status.code(), Some(3), "{err}");
        assert!(err.contains(path), "{err}");
        if path == "dynamic_mesh.moving_patches" {
            let line = text.lines().position(|l| l.starts_with("moving_patches")).unwrap() + 1;
            assert!(err.contains(&format!("line {line}")), "{err}");
        }
    }
}
