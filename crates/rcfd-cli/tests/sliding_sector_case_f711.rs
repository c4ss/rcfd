//! F-711 (ADR 0646) C1 and C2: a case file with a rotor sector sliding against a stator sector of the same pitch beyond one
//! pitch (`[mesh.sliding]` on `mesh.generator = "annulus_3d_sector_cylindrical_interface"`) run by the `rcfd` binary,
//! against the in-process run of the same text, and the located error of the table through `rcfd validate`. The case text
//! is written to a temporary folder (no case file is added to the repository).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::path::PathBuf;
use std::process::{Command, Output};

/// C1 of ADR 0646: the binary's values against an in-process run of the same text (as ADR 0637 C1).
const SAME: f64 = 1e-12;

/// The initial field of FS (ADR 0646): the stream function `psi = (r^2 - 1)(4 - r^2) sin(6 theta) / 12`.
fn initial() -> String {
    let r = "sqrt((x / 1 [m])^2 + (y / 1 [m])^2)";
    let t = "atan2(y / 1 [m], x / 1 [m])";
    let ux = "0.08333333333333333 [m/s] * (6 * (R^2 - 1) * (4 - R^2) / R * cos(6 * T) * cos(T) + 2 * R * (5 - 2 * R^2) * sin(6 * T) * sin(T))";
    let uy = "0.08333333333333333 [m/s] * (6 * (R^2 - 1) * (4 - R^2) / R * cos(6 * T) * sin(T) - 2 * R * (5 - 2 * R^2) * sin(6 * T) * cos(T))";
    let sub = |e: &str| e.replace('R', r).replace('T', t);
    format!(
        "velocity_expression_x = \"{}\"\nvelocity_expression_y = \"{}\"",
        sub(ux),
        sub(uy)
    )
}

/// CL of ADR 0646: FS (the m = 6 field, walls at rest, the rotor sector turning at 1 rad/s) for 1.1 s (220 steps, one
/// re-imaging step), with two reports.
fn cl() -> String {
    format!(
        r#"title = "F-711 CL: rotor sector sliding against a stator sector"

[mesh]
generator = "annulus_3d_sector_cylindrical_interface"
nx = 24
ny = 8
nz = 1
nx_right = 20
ny_right = 8
nz_right = 1
r_in = 1.0
r_interface = 1.5
r_out = 2.0
lz = 0.1
sector_angle = 60.0

[mesh.sliding]
angular_velocity = 1.0
rotor_walls = "rest"

[physics]
nu = 0.01

[initial]
{}

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
dt = 0.005
end_time = 1.1

[reports.ux_mean]
type = "volume_average"
field = "ux"

[reports.uy_mean]
type = "volume_average"
field = "uy"
"#,
        initial()
    )
}

fn rcfd(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcfd")).args(args).output().unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_f711_{}_{name}", std::process::id()));
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
fn c1_sector_sliding_case_file_through_the_binary_equals_the_in_process_run() {
    let text = cl();
    let d = tmp("c1");
    let case = d.join("sector.toml");
    std::fs::write(&case, &text).unwrap();
    let out = d.join("out");
    let o = rcfd(&["run", case.to_str().unwrap(), "--out", out.to_str().unwrap(), "--quiet"]);
    let code = o.status.code();
    println!(
        "C1: exit {code:?}; stderr {}",
        String::from_utf8_lossy(&o.stderr).trim()
    );
    assert_eq!(code, Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let names = ["sliding_angle", "sliding_image", "ux_mean", "uy_mean"];
    let cli = last_values(&std::fs::read_to_string(out.join("monitors.csv")).unwrap(), &names);
    let mut s = Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap();
    let steps = s.run(1_000_000);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let lib: Vec<f64> = names
        .iter()
        .map(|n| *s.monitors.get(n).unwrap().y.last().unwrap())
        .collect();
    let d_angle = (cli[0] - lib[0]).abs();
    let d_image = (cli[1] - lib[1]).abs();
    let diff: Vec<f64> = cli[2..]
        .iter()
        .zip(&lib[2..])
        .map(|(a, b)| (a - b).abs() / b.abs())
        .collect();
    let pitch = std::f64::consts::PI / 3.0;
    println!(
        "C1: binary {cli:?}, in process {lib:?} ({steps} steps); angle difference {d_angle:.3e}, image difference {d_image:.3e} (1e-12); angle {:.6} against the pitch {pitch:.6}; report relative differences {diff:?} (limit {SAME:e})",
        cli[0]
    );
    assert!(d_angle <= 1e-12 && d_image <= 1e-12, "C1 angle / image");
    assert!(cli[0] >= pitch && cli[1] == 1.0, "C1 beyond one pitch");
    assert!(diff.iter().all(|x| *x <= SAME), "C1 {diff:?}");
}

#[test]
fn c2_validate_locates_the_negative_angular_velocity() {
    let text = cl();
    let d = tmp("c2");
    let bad = text.replace("angular_velocity = 1.0", "angular_velocity = -1.0");
    assert_ne!(bad, text);
    let f = d.join("bad.toml");
    std::fs::write(&f, &bad).unwrap();
    let o = rcfd(&["validate", f.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&o.stderr);
    println!("C2: exit {:?}: {}", o.status.code(), err.trim());
    assert_eq!(o.status.code(), Some(3), "{err}");
    assert!(err.contains("mesh.sliding.angular_velocity"), "{err}");
}
