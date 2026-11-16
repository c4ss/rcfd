//! F-303 (ADR 0637) C1 and C2: a case file with a sliding rotor-stator interface (`[mesh.sliding]`) run by the `rcfd`
//! binary, against the in-process run of the same text, and the located error of the table through `rcfd validate`. The
//! case text is written to a temporary folder (no case file is added to the repository).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::path::PathBuf;
use std::process::{Command, Output};

/// C1 of ADR 0637: the binary's report values against an in-process run of the same text (as ADR 0257 X3).
const SAME: f64 = 1e-12;
/// `Omega end_time` of CL.
const ANGLE: f64 = 0.2617993877991494;

/// CL of ADR 0637: NA (the non-axisymmetric strain field, walls at rest, the rotor turning at 1 rad/s) for 48 steps, with
/// two reports.
const CL: &str = r#"title = "F-303 CL: sliding rotor-stator interface"

[mesh]
generator = "annulus_3d_cylindrical_interface"
nx = 144
ny = 8
nz = 1
nx_right = 120
ny_right = 8
nz_right = 1
r_in = 1.0
r_interface = 1.5
r_out = 2.0
lz = 0.1

[mesh.sliding]
angular_velocity = 1.0
rotor_walls = "rest"

[physics]
nu = 0.01

[initial]
velocity_expression_x = "0.5 [1/s] * x"
velocity_expression_y = "-0.5 [1/s] * y"

[boundaries.inner]
velocity = "fixed"

[boundaries.outer]
velocity = "fixed"

[boundaries.zmin]
velocity = "symmetry"

[boundaries.zmax]
velocity = "symmetry"

[solver]
algorithm = "piso"
convection = "central"
non_orthogonal_correctors = 6
momentum_rel_tol = 1e-13
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
momentum_max_iter = 5000
pressure_max_iter = 5000
dt = 0.00545415391248228
end_time = 0.2617993877991494

[reports.ux_mean]
type = "volume_average"
field = "ux"

[reports.uy_mean]
type = "volume_average"
field = "uy"
"#;

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f303_{}_{name}", std::process::id()));
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
fn c1_sliding_case_file_through_the_binary_equals_the_in_process_run() {
    let d = tmp("c1");
    let case = d.join("sliding.toml");
    std::fs::write(&case, CL).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap(), "--quiet"]);
    let code = o.status.code();
    println!(
        "C1: exit {code:?}; stderr {}",
        String::from_utf8_lossy(&o.stderr).trim()
    );
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let names = ["sliding_angle", "ux_mean", "uy_mean"];
    let cli = last_values(&std::fs::read_to_string(out.join("monitors.csv")).unwrap(), &names);
    let mut s = Simulation::new(&Case::parse_toml(CL).unwrap()).unwrap();
    let steps = s.run(1_000_000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let lib: Vec<f64> = names
        .iter()
        .map(|n| *s.monitors.get(n).unwrap().y.last().unwrap())
        .collect();
    let d_angle = (cli[0] - ANGLE).abs();
    let diff: Vec<f64> = cli[1..]
        .iter()
        .zip(&lib[1..])
        .map(|(a, b)| (a - b).abs() / b.abs())
        .collect();
    println!(
        "C1: binary {cli:?}, in process {lib:?} ({steps} steps); angle against {ANGLE} {d_angle:.3e} (1e-12); report relative differences {diff:?} (limit {SAME:e})"
    );
    assert!(d_angle <= 1e-12, "C1 angle {d_angle:e}");
    assert!(diff.iter().all(|x| *x <= SAME), "C1 {diff:?}");
}

#[test]
fn c2_validate_locates_the_error_of_the_table() {
    let d = tmp("c2");
    let bad = CL.replace(
        "generator = \"annulus_3d_cylindrical_interface\"",
        "generator = \"annulus_3d\"",
    );
    assert_ne!(bad, CL);
    let f = d.join("bad.toml");
    std::fs::write(&f, &bad).unwrap();
    let o = rcfd(&["validate", f.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&o.stderr);
    println!("C2: exit {:?}: {}", o.status.code(), err.trim());
    assert_eq!(o.status.code(), Some(3), "{err}");
    assert!(err.contains("mesh.sliding"), "{err}");
}
