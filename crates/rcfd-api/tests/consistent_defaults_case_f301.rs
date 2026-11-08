//! F-301 (ADR 0223) N2: fingerprints of default case-path runs, written with the API of main 46a5f029 only. They were printed on
//! that library before any library change of F-301 and again on the final tree; ADR 0223 registers that the printed lines are equal.
//! Each test prints `N2 <id> <hash>` (FNV-1a over the bits of the fields and records named in ADR 0223 N2).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn cylinder_text() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    std::fs::read_to_string(path).expect("example case readable")
}

fn field_words(sim: &Simulation) -> Vec<u64> {
    ["p", "ux", "uy"]
        .iter()
        .flat_map(|n| sim.field(n).unwrap().into_iter().map(|x| x.to_bits()))
        .collect()
}

/// ADR 0156's piston case (box_2d, piso, moving xmin) without the adaptive key, with one corrector so that the case path sets
/// the pressure-lag check.
const PISTON_ONE_CORRECTOR: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5
n_correctors = 1
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

/// N2 (g): the cylinder example through `Simulation`, 16 steps.
#[test]
fn n2g_cylinder_example_sixteen_steps() {
    let case = Case::parse_toml(&cylinder_text()).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..16 {
        sim.step();
    }
    assert!(!sim.diverged());
    println!("N2 g {:016x}", fnv(field_words(&sim)));
}

/// N2 (h): the piston case with one corrector through `Simulation`, 10 steps, and the number of run-time warnings.
#[test]
fn n2h_moving_piston_with_one_corrector() {
    let case = Case::parse_toml(PISTON_ONE_CORRECTOR).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut warnings = 0u64;
    for _ in 0..10 {
        sim.step();
        warnings += sim.take_runtime_warnings().len() as u64;
    }
    assert!(!sim.diverged());
    let mut words = field_words(&sim);
    words.push(warnings);
    println!("N2 h {:016x} (run-time warnings {warnings})", fnv(words));
}

/// N2 (i): the cylinder example's settings content hash and the FNV of its written TOML.
#[test]
fn n2i_cylinder_example_hash_and_text() {
    let case = Case::parse_toml(&cylinder_text()).unwrap_or_else(|e| panic!("{e:?}"));
    let text = case.to_toml();
    println!(
        "N2 i {:016x} {:016x}",
        case.tree.content_hash(),
        rcfd_core::hash::fnv64(text.as_bytes())
    );
}
