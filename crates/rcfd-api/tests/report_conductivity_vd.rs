//! Item W of ADR 0021: `reports.<n>.conductivity` is not read by the variable-density solver, so a positive value
//! is a located validation error there (checks W1 and W2).

use rcfd_api::case::Case;

const VD: &str = r#"
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
relax = 0.9
[initial]
velocity = [30.0, 0.0, 0.0]
temperature = 300.0
pressure = 100000.0
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
"#;

fn with_report(extra: &str) -> String {
    format!("{VD}\n[reports.q]\ntype = \"heat_flow\"\npatch = \"ymin\"\n{extra}")
}

#[test]
fn w1_positive_conductivity_with_variable_density_is_a_located_error() {
    let text = with_report("conductivity = 2.0\n");
    let errs = Case::parse_toml(&text).unwrap_err();
    let e = errs
        .iter()
        .find(|e| e.path == "reports.q.conductivity")
        .unwrap_or_else(|| panic!("{errs:?}"));
    let expected = text.lines().position(|l| l.starts_with("conductivity = 2.0")).unwrap() + 1;
    assert_eq!(e.line, Some(expected), "{e:?}");
    assert!(e.message.contains("fluid"), "{}", e.message);
}

#[test]
fn w2_positive_controls_validate() {
    // key absent and key = 0 (unset) are fine
    Case::parse_toml(&with_report("")).unwrap_or_else(|e| panic!("{e:?}"));
    Case::parse_toml(&with_report("conductivity = 0.0\n")).unwrap_or_else(|e| panic!("{e:?}"));
    // constant density keeps the key: the same report on a constant-density heat case
    let cd = r#"
[mesh]
generator = "box_2d"
nx = 8
ny = 2
lx = 0.1
ly = 0.02
[physics]
nu = 0.01
rho = 1.0
cp = 1000.0
conductivity = 2.0
[energy]
enabled = true
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 310.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[reports.q]
type = "heat_flow"
patch = "xmin"
conductivity = 2.0
"#;
    Case::parse_toml(cd).unwrap_or_else(|e| panic!("{e:?}"));
}
