//! Characteristic compressible boundary conditions through the case schema (F-062, ADR 0006 check E).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const DUCT: &str = r#"
title = "total inlet duct"
[mesh]
generator = "box_2d"
nx = 40
ny = 2
lx = 2.0
ly = 0.2
[solver]
algorithm = "compressible"
end_time = 0.004
[compressible]
flux = "hllc"
[compressible.initial.left]
density = 1.15
velocity = [60.0, 0.0, 0.0]
pressure = 98000.0
[boundaries.xmin]
compressible_bc = "total_inlet"
total_pressure_value = 101325.0
temperature_value = 300.0
flow_direction = [1.0, 0.0, 0.0]
[boundaries.xmax]
compressible_bc = "pressure_outlet"
pressure_value = 98000.0
[boundaries.ymin]
compressible_bc = "slip"
[boundaries.ymax]
compressible_bc = "slip"
[reports.inflow]
type = "mass_flow"
patch = "xmin"
"#;

fn errors(text: &str) -> Vec<(String, Option<usize>, String)> {
    Case::parse_toml(text)
        .unwrap_err()
        .into_iter()
        .map(|d| (d.path, d.line, d.message))
        .collect()
}

fn find(text: &str, path: &str) -> String {
    let d = errors(text);
    d.iter()
        .find(|e| e.0 == path)
        .unwrap_or_else(|| panic!("no diagnostic for {path}: {d:?}"))
        .2
        .clone()
}

#[test]
fn total_inlet_duct_runs_from_a_case_file() {
    let case = Case::parse_toml(DUCT).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(100000);
    assert!(sim.is_done());
    let m = sim.monitors.last("inflow").unwrap();
    assert!(m.is_finite() && m < 0.0, "inflow {m}");
    assert!(sim.monitors.last("res_rho").unwrap().is_finite());
}

#[test]
fn characteristic_bc_validation_errors_name_the_key() {
    let t = DUCT.replace("total_pressure_value = 101325.0\n", "");
    assert!(find(&t, "boundaries.xmin.total_pressure_value").contains("positive total pressure"));
    let t = DUCT.replace("temperature_value = 300.0", "temperature_value = 0.0");
    assert!(find(&t, "boundaries.xmin.temperature_value").contains("total temperature"));
    let t = DUCT.replace(
        "pressure_value = 98000.0\n[boundaries.ymin]",
        "pressure_value = 0.0\n[boundaries.ymin]",
    );
    assert!(find(&t, "boundaries.xmax.pressure_value").contains("static pressure"));
    let t = DUCT.replace("pressure_outlet", "nonreflecting") + "";
    let t = t.replace("[boundaries.ymin]", "reflection_value = 1.5\n[boundaries.ymin]");
    assert!(find(&t, "boundaries.xmax.reflection_value").contains("[0, 1]"));
    let t = DUCT.replace("pressure_outlet", "farfield").replace(
        "[boundaries.ymin]",
        "mach_value = 0.5
temperature_value = 0.0\n[boundaries.ymin]",
    );
    assert!(find(&t, "boundaries.xmax.temperature_value").contains("static temperature"));
    // without the compressible algorithm
    let t = "[boundaries.lid]\ncompressible_bc = \"total_inlet\"\n";
    assert!(find(t, "boundaries.lid.compressible_bc").contains("compressible"));
}

#[test]
fn farfield_and_nonreflecting_run_from_a_case_file() {
    let t = DUCT
        .replace("pressure_outlet", "nonreflecting")
        .replace("[boundaries.ymin]", "reflection_value = 0.1\n[boundaries.ymin]");
    let case = Case::parse_toml(&t).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(100000);
    assert!(sim.monitors.last("inflow").unwrap().is_finite());
    let t = DUCT
        .replace("compressible_bc = \"total_inlet\"", "compressible_bc = \"farfield\"")
        .replace(
            "total_pressure_value = 101325.0\n",
            "pressure_value = 98000.0\nmach_value = 0.17\nflow_angle = 0.0\n",
        );
    let case = Case::parse_toml(&t).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(100000);
    assert!(sim.monitors.last("inflow").unwrap().is_finite());
}
