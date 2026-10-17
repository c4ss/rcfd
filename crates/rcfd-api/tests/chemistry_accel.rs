//! ADR 0053 D4 case-tree checks W1 to W4 (F-223): `chemistry.acceleration` and `chemistry.accel_tolerance` on the wave 21C K2b closed
//! adiabatic isomerisation box (4 x 4 cells). Limits are those registered in the ADR before any run.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_reactor::mech::R_GAS;
use rcfd_reactor::toy;

const W: f64 = 0.020;

fn box_text(accel: &str, model: &str, algorithm: &str) -> String {
    let (m, t) = toy::isomer_texts(1.0e8, 19872.0, -3000.0, false);
    let rho = 1.5;
    let cv = 1.5 * R_GAS / W;
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 4
lx = 1.0
ly = 1.0
[physics]
nu = 0.01
rho = {rho:.17e}
cp = {cv:.17e}
conductivity = 1.0
[energy]
enabled = true
buoyancy = false
[initial]
temperature = 800.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "{algorithm}"
dt = 1e-5
end_time = 2e-3
[species.A]
diffusivity = 1e-5
initial = 1.0
[species.B]
diffusivity = 1e-5
initial = 0.0
remainder = true
[chemistry]
model = "{model}"
density = {rho:.17e}
heat_release = true
heat_capacity = {cv:.17e}
mechanism_text = '''
{m}'''
thermo_text = '''
{t}'''
{accel}
"#
    )
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

fn error_of(text: &str) -> String {
    match Case::parse_toml(text) {
        Err(e) => format!("{e:?}"),
        Ok(case) => match Simulation::new(&case) {
            Ok(_) => panic!("the case was accepted"),
            Err(e) => format!("{e:?}"),
        },
    }
}

fn run(accel: &str) -> (Vec<Vec<f64>>, Simulation) {
    let mut sim = sim_of(&box_text(accel, "finite_rate", "piso"));
    let mut hist = Vec::new();
    for _ in 0..200 {
        sim.step();
        assert!(sim.failure().is_none(), "{:?}", sim.failure());
        let mut row = sim.field("T").unwrap();
        row.extend(sim.field("A").unwrap());
        hist.push(row);
    }
    (hist, sim)
}

#[test]
fn w1_cluster_with_tolerance_zero_is_bit_identical() {
    let (off, _) = run("");
    let (cl, sim) = run("acceleration = \"cluster\"\naccel_tolerance = 0.0");
    let mismatches = off
        .iter()
        .zip(&cl)
        .flat_map(|(a, b)| a.iter().zip(b))
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    let rep = sim.chemistry_report().unwrap();
    let acc = &rep["acceleration"];
    println!(
        "W1 bit mismatches {mismatches} of {} values; clustering: {} cells, {} integrations over {} steps",
        off.len() * off[0].len(),
        acc["cells"],
        acc["integrations"],
        acc["steps"]
    );
    assert_eq!(mismatches, 0);
    // with tolerance 0 only bit-equal cells share an integration, so never more integrations than cells
    assert!(acc["integrations"].as_u64().unwrap() <= acc["cells"].as_u64().unwrap());
    assert_eq!(acc["cells"].as_u64().unwrap(), 16 * acc["steps"].as_u64().unwrap());
}

#[test]
fn w2_isat_follows_the_direct_chemistry() {
    let (off, _) = run("");
    let (is, sim) = run("acceleration = \"isat\"\naccel_tolerance = 1e-3");
    // scaled max-norm: 100 K and 0.01 in mass fraction
    let mut worst = 0.0_f64;
    for (a, b) in off.iter().zip(&is) {
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            let s = if i < 16 { 100.0 } else { 0.01 };
            worst = worst.max((x - y).abs() / s);
        }
    }
    let rep = sim.chemistry_report().unwrap();
    let acc = &rep["acceleration"];
    println!(
        "W2 worst scaled difference to off {worst:.3e} (limit 1e-3); ISAT queries {} retrieved {} grown {} added {} records {}",
        acc["queries"], acc["retrieved"], acc["grown"], acc["added"], acc["records"]
    );
    assert!(acc["retrieved"].as_u64().unwrap() > 0, "the table was never used");
    assert!(worst <= 1e-3);
}

#[test]
fn w3_located_errors() {
    // closures other than finite_rate: the turbulence closures need k and epsilon, so the first error is the acceleration one
    let e = error_of(&box_text(
        "acceleration = \"isat\"\naccel_tolerance = 1e-3",
        "eddy_dissipation",
        "piso",
    ));
    assert!(e.contains("chemistry.acceleration") && e.contains("finite_rate"), "{e}");
    // unset tolerance
    let e = error_of(&box_text("acceleration = \"cluster\"", "finite_rate", "piso"));
    assert!(e.contains("chemistry.accel_tolerance") && e.contains("unset"), "{e}");
    // negative tolerance
    let e = error_of(&box_text(
        "acceleration = \"isat\"\naccel_tolerance = -0.5",
        "finite_rate",
        "piso",
    ));
    assert!(e.contains("chemistry.accel_tolerance"), "{e}");
    // isat with zero
    let e = error_of(&box_text(
        "acceleration = \"isat\"\naccel_tolerance = 0.0",
        "finite_rate",
        "piso",
    ));
    assert!(e.contains("chemistry.accel_tolerance") && e.contains("> 0"), "{e}");
    // unknown value
    let e = error_of(&box_text(
        "acceleration = \"tabulate\"\naccel_tolerance = 0.1",
        "finite_rate",
        "piso",
    ));
    assert!(e.contains("acceleration"), "{e}");
    // tolerance without acceleration
    let e = error_of(&box_text("accel_tolerance = 0.1", "finite_rate", "piso"));
    assert!(e.contains("chemistry.accel_tolerance"), "{e}");
    // steady algorithm
    let e = error_of(&box_text(
        "acceleration = \"cluster\"\naccel_tolerance = 0.1",
        "finite_rate",
        "simple",
    ));
    assert!(e.contains("chemistry.acceleration") && e.contains("transient"), "{e}");
    println!("W3 all seven refusals name the offending key");
}

#[test]
fn w4_default_has_no_acceleration_report() {
    let (a, sim) = run("");
    let (b, _) = run("acceleration = \"off\"");
    assert_eq!(a, b);
    let rep = sim.chemistry_report().unwrap();
    assert!(
        rep.get("acceleration").is_none(),
        "the default report must not gain a key"
    );
    println!("W4 default and explicit off identical; chemistry.json without an acceleration key");
}
