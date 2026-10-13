//! Profile and table inputs (F-105): a parabolic inlet read from a CSV profile reproduces the
//! analytic plane Poiseuille flow, tables are callable from expressions and bad data is reported.
//!
//! The acceptance check was fixed before running: the cell-centre velocity in the last cell
//! column must match u(y) = 6 U y (H - y) / H^2 within 2 % of the centreline value, and the
//! inlet-to-outlet kinematic pressure drop must match 12 nu U L / H^2 within 3 %.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::path::PathBuf;

const H: f64 = 1.0;
const U: f64 = 1.0;
const NU: f64 = 0.1;
const L: f64 = 2.0;

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rcfd_tables_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn channel(inlet: &str, tables: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 24
ny = 20
lx = {L}
ly = {H}
depth = 0.1
[physics]
nu = {NU}
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
{inlet}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-6
pressure_rel_tol = 1e-6
pressure_max_iter = 1000
max_iterations = 2000
tolerance_u = 1e-9
tolerance_p = 1e-9
[reports.p_in]
type = "area_average"
patch = "inlet"
field = "p"
[reports.p_out]
type = "area_average"
patch = "outlet"
field = "p"
{tables}
"#
    )
}

fn poiseuille_csv() -> String {
    let mut s = String::from("# parabolic inlet, plane Poiseuille, U = 1 m/s mean, H = 1 m\ny [m], ux [m/s]\n");
    for i in 0..=100 {
        let y = i as f64 * H / 100.0;
        s.push_str(&format!("{y},{}\n", 6.0 * U * y * (H - y) / (H * H)));
    }
    s
}

fn check_poiseuille(sim: &Simulation) {
    let ux = sim.field("ux").unwrap();
    let c = sim.geom.mesh.cell_centres();
    let n = ux.len();
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for i in 0..n {
        if c[i][0] > L - L / 24.0 {
            let y = c[i][1];
            let exact = 6.0 * U * y * (H - y) / (H * H);
            worst = worst.max((ux[i] - exact).abs() / 1.5);
            count += 1;
        }
    }
    assert_eq!(count, 20);
    eprintln!("last-column max deviation {worst:.5} of the centreline value");
    assert!(
        worst < 0.02,
        "last-column velocity deviates {worst} of the centreline value"
    );
    let rv = |name: &str| {
        sim.report_values()
            .into_iter()
            .find(|(k, _)| k == name)
            .unwrap()
            .1
            .unwrap()
            .value
    };
    let dp = rv("p_in") - rv("p_out");
    let exact = 12.0 * NU * U * L / (H * H);
    eprintln!("pressure drop {dp:.5} vs analytic {exact:.5}");
    assert!(((dp - exact) / exact).abs() < 0.03, "dp {dp} vs {exact}");
}

#[test]
fn csv_profile_file_reproduces_the_analytic_poiseuille_solution() {
    let dir = tmpdir("poiseuille");
    std::fs::write(dir.join("inlet.csv"), poiseuille_csv()).unwrap();
    let text = channel(
        "profile = \"inlet_u\"",
        "[tables.inlet_u]\nfile = \"inlet.csv\"\ninputs = [\"y\"]\n",
    );
    std::fs::write(dir.join("case.toml"), &text).unwrap();
    let case = Case::load(dir.join("case.toml")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(2000);
    assert!(sim.is_converged(), "not converged");
    check_poiseuille(&sim);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn inline_table_called_from_an_expression_gives_the_same_flow() {
    // the table is used through an expression instead of the profile setting
    let csv = poiseuille_csv().replace('\n', "\\n");
    let text = channel(
        "velocity_expression_x = \"inlet_u(y)\"",
        &format!("[tables.inlet_u]\ndata = \"{csv}\"\ninputs = [\"y\"]\n"),
    );
    let mut sim = Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap();
    sim.run(2000);
    check_poiseuille(&sim);
}

#[test]
fn missing_and_out_of_range_data_raise_errors() {
    // missing file
    let text = channel(
        "profile = \"p\"",
        "[tables.p]\nfile = \"does_not_exist.csv\"\ninputs = [\"y\"]\n",
    );
    let errs = Case::parse_toml(&text).unwrap_err();
    assert!(
        errs.len() == 1 && errs[0].path == "tables.p.file" && errs[0].message.contains("cannot read"),
        "{errs:?}"
    );
    // the table covers only y in [0, 0.5] of a patch that spans [0, 1]
    let half = "y [m],ux [m/s]\\n0,0\\n0.5,1.5\\n";
    let text = channel(
        "profile = \"p\"",
        &format!("[tables.p]\ndata = \"{half}\"\ninputs = [\"y\"]\n"),
    );
    let errs = Simulation::new(&Case::parse_toml(&text).unwrap()).err().unwrap();
    assert!(
        errs.iter().any(|d| d.message.contains("does not cover the patch")),
        "{errs:?}"
    );
    // with clamp the same data builds
    let text = text.replace("inputs = [\"y\"]", "inputs = [\"y\"]\nextrapolation = \"clamp\"");
    Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap();
    // unknown profile table, unknown input column, unit mismatch
    let text = channel("profile = \"nope\"", "");
    let errs = Case::parse_toml(&text).unwrap_err();
    assert!(errs.iter().any(|d| d.path == "boundaries.inlet.profile"), "{errs:?}");
    let text = channel(
        "profile = \"p\"",
        "[tables.p]\ndata = \"T,ux\\n1,2\\n3,4\\n\"\ninputs = [\"T\"]\n",
    );
    let errs = Case::parse_toml(&text).unwrap_err();
    assert!(
        errs.iter()
            .any(|d| d.message.contains("can only depend on x, y, z and t")),
        "{errs:?}"
    );
    let text = channel(
        "profile = \"p\"",
        "[tables.p]\ndata = \"y [m],ux [kg]\\n0,0\\n1,4\\n\"\ninputs = [\"y\"]\n",
    );
    let errs = Case::parse_toml(&text).unwrap_err();
    assert!(errs.iter().any(|d| d.message.contains("dimension")), "{errs:?}");
}

#[test]
fn temperature_dependent_viscosity_table_in_a_report_expression() {
    // nu(T) table in Kelvin read from degC data; the expression evaluates it at 30 degC
    let text = r#"
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
[boundaries.walls]
velocity = "fixed"
[tables.nu_water]
data = "T [degC], nu [m^2/s]\n10, 1.306e-6\n20, 1.004e-6\n30, 0.801e-6\n40, 0.658e-6\n"
inputs = ["T"]
[reports.dummy]
type = "volume_average"
field = "speed"
[reports.nu30]
type = "expression"
expression = "nu_water(30 [degC]) / 1 [m^2/s]"
[reports.nu25]
type = "expression"
expression = "nu_water(298.15 [K]) / 1 [m^2/s]"
"#;
    let sim = Simulation::new(&Case::parse_toml(text).unwrap()).unwrap();
    let get = |n: &str| {
        sim.report_values()
            .into_iter()
            .find(|(k, _)| k == n)
            .unwrap()
            .1
            .unwrap()
            .value
    };
    assert!((get("nu30") - 0.801e-6).abs() < 1e-18);
    assert!((get("nu25") - 0.9025e-6).abs() < 1e-12 * 1e-6, "{}", get("nu25"));
    // an argument with the wrong dimension is rejected with the column named
    let bad = text.replace("nu_water(298.15 [K])", "nu_water(298.15 [m])");
    let errs = Case::parse_toml(&bad).unwrap_err();
    assert!(
        errs.iter().any(|d| d.message.contains("must have dimension")),
        "{errs:?}"
    );
}

#[test]
fn service_sandbox_confines_table_files() {
    use rcfd_api::service::{Service, ServiceConfig};
    let root = tmpdir("sandbox");
    std::fs::write(root.join("inlet.csv"), poiseuille_csv()).unwrap();
    let outside = root
        .parent()
        .unwrap()
        .join(format!("rcfd_secret_{}.csv", std::process::id()));
    std::fs::write(&outside, "y,ux\n0,0\n1,1\n").unwrap();
    let svc = Service::new(ServiceConfig {
        root: Some(root.clone()),
        ..Default::default()
    });
    let toml = |file: &str| {
        channel(
            "profile = \"p\"",
            &format!("[tables.p]\nfile = \"{}\"\ninputs = [\"y\"]\n", file.replace('\\', "/")),
        )
    };
    let ok = svc.call("session.create", &serde_json::json!({"toml": toml("inlet.csv")}));
    assert!(ok.is_ok(), "{ok:?}");
    for bad in [
        "../".to_string() + outside.file_name().unwrap().to_str().unwrap(),
        outside.display().to_string(),
    ] {
        let e = svc
            .call("session.create", &serde_json::json!({"toml": toml(&bad)}))
            .unwrap_err();
        assert!(e.message.contains("outside the sandbox"), "{}: {e:?}", bad);
    }
    std::fs::remove_file(&outside).ok();
    std::fs::remove_dir_all(&root).ok();
}
