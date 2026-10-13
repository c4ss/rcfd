//! Transient energy equation with buoyancy through the settings tree (F-050 gap closed for PISO/PIMPLE).
//!
//! Check fixed before running: the Ra = 1e3 differentially heated cavity of the steady test (32x32 graded 4:1,
//! nondimensional alpha = 1, H = 1, g = Ra Pr) marched with PISO to `t = 1.5` (dt = 5e-3, steady well before) gives a
//! Nusselt number within 2 percent of de Vahl Davis (1.118), within 1 percent of the steady SIMPLEC value 1.1167 of
//! the same discretisation, and hot and cold wall heat flows balanced to 1e-6.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const CAVITY: &str = r#"
title = "heated cavity, transient"
[mesh]
generator = "box_2d"
nx = 32
ny = 32
lx = 1.0
ly = 1.0
grading = 4.0
[physics]
nu = 0.71
rho = 1.0
cp = 1.0
conductivity = 1.0
reference_temperature = 0.5
thermal_expansion = 1.0
gravity = [0.0, -710.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "piso"
convection = "tvd_van_leer"
dt = 0.005
end_time = 1.5
[reports.q_hot]
type = "heat_flow"
patch = "xmin"
[reports.q_cold]
type = "heat_flow"
patch = "xmax"
"#;

#[test]
fn piso_cavity_with_energy_reaches_the_steady_nusselt_number() {
    let case = Case::parse_toml(CAVITY).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(2000);
    assert!(sim.is_done());
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    let q_hot = sim.monitors.last("q_hot").unwrap();
    let q_cold = sim.monitors.last("q_cold").unwrap();
    let nu = -q_hot / 0.03125;
    println!("{n} steps, Nu = {nu:.4}, q_hot {q_hot:.6}, q_cold {q_cold:.6}");
    assert!((q_hot + q_cold).abs() < 1e-6 * q_hot.abs(), "{q_hot} {q_cold}");
    assert!((nu / 1.118 - 1.0).abs() < 0.02, "Nu {nu}");
    assert!((nu / 1.1167 - 1.0).abs() < 0.01, "Nu vs steady {nu}");
    assert!(sim.monitors.get("res_T").is_some());
}

#[test]
fn transient_energy_with_properties_laws_is_still_diagnosed() {
    let text = CAVITY.replace(
        "[solver]",
        "[properties.viscosity]\nlaw = \"polynomial\"\ncoefficients = [0.71]\n[solver]",
    );
    let err = match Case::parse_toml(&text) {
        Err(d) => d,
        Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
    };
    assert!(
        err.iter().any(|d| d.path.starts_with("properties.viscosity")),
        "{err:?}"
    );
}
