//! Real-gas equation of state (ADR 0007) through the case schema: wiring checks and validation messages (R3 through
//! the API, R7). The quantitative validation against the independent real-gas isentropic reference is in
//! rcfd-fv/tests/varden_realgas_nozzle.rs.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

/// CO2 nozzle_2d case (100 x 8, subsonic, back pressure 0.97 p0) with the given `[fluid]` block and stagnation state.
fn nozzle(fluid: &str, p0: f64, t0: f64) -> String {
    format!(
        r#"
[mesh]
generator = "nozzle_2d"
nx = 100
ny = 8
lx = 2.0
ly = 0.1
throat_height = 0.05
exit_height = 0.1
[fluid]
{fluid}
[energy]
enabled = true
relax = 0.9
[initial]
velocity = [30.0, 0.0, 0.0]
temperature = {t0}
pressure = {pm}
[boundaries.xmin]
inlet = "stagnation"
total_pressure_value = {p0}
temperature_value = {t0}
velocity = "zero_gradient"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = {pe}
temperature = "zero_gradient"
[boundaries.ymin]
velocity = "symmetry"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 800
[reports.m_in]
type = "mass_flow"
patch = "xmin"
[reports.m_out]
type = "mass_flow"
patch = "xmax"
"#,
        pm = 0.985 * p0,
        pe = 0.97 * p0
    )
}

const REAL: &str = "density = \"real_gas\"\nspecies = \"CO2\"\nspecific_heat = 940.0";

fn run(text: &str) -> (f64, f64) {
    let case = Case::parse_toml(text).unwrap();
    assert!(case.semantic_errors().is_empty(), "{:?}", case.semantic_errors());
    let mut sim = Simulation::new(&case).unwrap();
    while !sim.is_done() {
        sim.step();
    }
    let last = |n: &str| *sim.monitors.get(n).unwrap().y.last().unwrap();
    (last("m_in"), last("m_out"))
}

#[test]
fn real_gas_nozzle_conserves_mass_through_the_case_file() {
    let (m_in, m_out) = run(&nozzle(REAL, 5.0e6, 400.0));
    println!("real gas mass flow in {m_in:.6e} out {m_out:.6e}");
    assert!(m_in < 0.0 && m_out > 0.0);
    assert!(((m_in + m_out) / m_out).abs() < 1e-6, "{m_in} {m_out}");
}

#[test]
fn real_gas_at_low_pressure_equals_the_ideal_gas_case_and_differs_at_high_pressure() {
    // ideal gas with the same R = R_u / M and cp: gas_constant 8.314462618 / 0.0440098, cp = 940 -> gamma = cp/(cp - R)
    let r = 8.314_462_618_153_24 / 0.0440098;
    let gamma = 940.0 / (940.0 - r);
    let ideal = format!("density = \"ideal_gas\"\ngas_constant = {r}\ngamma = {gamma}");
    let (ri, ro) = run(&nozzle(REAL, 2000.0, 400.0));
    let (ii, io) = run(&nozzle(&ideal, 2000.0, 400.0));
    let d_low = (ro / io - 1.0).abs();
    println!("2 kPa: real {ro:.6e} ideal {io:.6e} diff {d_low:.3e} (in: {ri:.4e} {ii:.4e})");
    assert!(d_low <= 1e-3, "{d_low}");
    let (_, rh) = run(&nozzle(REAL, 5.0e6, 400.0));
    let (_, ih) = run(&nozzle(&ideal, 5.0e6, 400.0));
    let d_high = rh / ih - 1.0;
    println!("5 MPa: real {rh:.6e} ideal {ih:.6e} diff {d_high:.3e}");
    assert!(d_high.abs() > 0.01, "the real-gas model is not active: {d_high}");
}

fn first_error(text: &str) -> (String, String) {
    match Case::parse_toml(text) {
        Err(d) => (d[0].path.clone(), d[0].message.clone()),
        Ok(c) => {
            let e = c.semantic_errors();
            assert!(!e.is_empty(), "no diagnostic");
            (e[0].path.clone(), e[0].message.clone())
        }
    }
}

#[test]
fn real_gas_settings_are_validated_with_their_path() {
    let n = |fluid: &str| nozzle(fluid, 5.0e6, 400.0);
    // unknown species
    let (p, m) = first_error(&n("density = \"real_gas\"\nspecies = \"unobtainium\""));
    assert_eq!(p, "fluid.species");
    assert!(m.contains("unobtainium") && m.contains("CO2"), "{m}");
    // no species and no constants
    let (p, _) = first_error(&n("density = \"real_gas\""));
    assert_eq!(p, "fluid.critical_temperature");
    // partial constants
    let (p, _) = first_error(&n(
        "density = \"real_gas\"\ncritical_temperature = 300.0\ncritical_pressure = 5e6",
    ));
    assert_eq!(p, "fluid.molar_mass");
    // unknown eos name is a schema error at fluid.eos
    let (p, _) = first_error(&n(&format!("{REAL}\neos = \"van_der_waals\"")));
    assert_eq!(p, "fluid.eos");
    // low_mach
    let (p, _) = first_error(&n(&format!("{REAL}\nmode = \"low_mach\"")));
    assert!(p == "fluid.mode" || p == "boundaries.xmin.inlet", "{p}");
    // gas_constant is derived
    let (p, _) = first_error(&n(&format!("{REAL}\ngas_constant = 300.0")));
    assert_eq!(p, "fluid.gas_constant");
    // cp0 below R
    let (p, _) = first_error(&n("density = \"real_gas\"\nspecies = \"CO2\"\nspecific_heat = 100.0"));
    assert_eq!(p, "fluid.specific_heat");
    // real-gas keys need real_gas
    let (p, _) = first_error(&n("density = \"ideal_gas\"\nspecies = \"CO2\""));
    assert_eq!(p, "fluid.species");
    let (p, _) = first_error(&n("density = \"ideal_gas\"\neos = \"redlich_kwong\""));
    assert_eq!(p, "fluid.eos");
    let (p, m) = first_error(&n("density = \"constant\"\ncritical_pressure = 5e6"));
    assert_eq!(p, "fluid.critical_pressure");
    assert!(m.contains("real_gas"), "{m}");
    // explicit constants are accepted
    let ok = n("density = \"real_gas\"\ncritical_temperature = 304.1282\ncritical_pressure = 7.3773e6\nacentric_factor = 0.22394\nmolar_mass = 0.0440098\nspecific_heat = 940.0\neos = \"redlich_kwong\"");
    let case = Case::parse_toml(&ok).unwrap();
    assert!(case.semantic_errors().is_empty(), "{:?}", case.semantic_errors());
}
