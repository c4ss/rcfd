//! F-443 (ADR 0352) T1 to T4: the case key `[solver.rhie_chow] step_consistent` maps to the library option
//! `PimpleSettings::rhie_chow_step_consistent` of ADR 0245, its located errors, its default (hash and written text) and a
//! dynamic mesh with it. The limits were registered in ADR 0352 before this file existed. The case texts are copied from
//! `consistent_correctors_case_f301.rs` (not edited).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};

const KEY: &str = "solver.rhie_chow.step_consistent";

fn repo_file(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn parse(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn with(mut case: Case, overrides: &[(&str, Value)]) -> Case {
    for (k, v) in overrides {
        case.tree.set(k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    case
}

/// The committed cylinder example (64 x 48, PISO, dt 1/16) with `overrides`.
fn cylinder(overrides: &[(&str, Value)]) -> Case {
    with(
        parse(&repo_file("example-projects/wind-tunnel/cases/cylinder-re100.toml")),
        overrides,
    )
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// ADR 0352 T1: the key sets the library option and nothing else, and it acts.
#[test]
fn t1_the_key_is_the_library_option() {
    let mut keyed = Simulation::new(&cylinder(&[(KEY, json!(true))])).unwrap_or_else(|e| panic!("{e:?}"));
    let mut plain = Simulation::new(&cylinder(&[])).unwrap();
    let (flag, same_rest) = {
        let a = keyed.pimple_solver().expect("transient").settings.clone();
        let b = plain.pimple_solver().expect("transient").settings.clone();
        let mut a_reset = a.clone();
        a_reset.rhie_chow_step_consistent = false;
        (
            a.rhie_chow_step_consistent && !b.rhie_chow_step_consistent,
            format!("{a_reset:?}") == format!("{b:?}"),
        )
    };
    for _ in 0..16 {
        keyed.step();
        plain.step();
    }
    let mut differs = false;
    for name in ["p", "ux", "uy"] {
        let d = bits(&keyed.field(name).unwrap()) != bits(&plain.field(name).unwrap());
        println!("T1 {name}: after 16 steps different from the run without the key {d}");
        differs |= d;
    }
    println!("T1: flag set by the key {flag}; the other settings equal {same_rest}");
    assert!(!keyed.diverged() && !plain.diverged(), "T1 stopped");
    assert!(flag, "T1: the key does not set the option");
    assert!(same_rest, "T1: the key changes another setting");
    assert!(differs, "T1: the key does not act");
}

/// A transient channel with a lumped porous jump (`consistent_correctors_case_f301.rs` `LUMPED`).
const LUMPED: &str = r#"
[mesh]
generator = "channel_2d"
nx = 20
ny = 2
lx = 2.0
ly = 0.2
depth = 0.1
[physics]
nu = 0.000015
rho = 1.2
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
[solver]
algorithm = "pimple"
convection = "upwind"
n_outer = 3
dt = 0.01
end_time = 0.1
[components.jump]
type = "porous_jump"
zone = "box"
box_min = [1.2, -1.0, -1.0]
box_max = [1.3, 1.0, 1.0]
direction = [1.0, 0.0, 0.0]
thickness = 0.02
permeability = 1e-8
inertial_resistance = 1000.0
"#;

/// ADR 0156's low-Mach closed box (`consistent_correctors_case_f301.rs` `LOW_MACH`).
const LOW_MACH: &str = r#"
[mesh]
generator = "box_2d"
nx = 6
ny = 6
lx = 0.1
ly = 0.1
[fluid]
density = "ideal_gas"
gas_constant = 287.0
gamma = 1.4
mode = "low_mach"
reference_pressure = 101325.0
inner_iterations = 60
inner_tolerance = 1e-12
[energy]
enabled = true
relax = 1.0
heat_source = 84437.5
[initial]
temperature = 300.0
pressure = 101325.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "pimple"
time_scheme = "bdf2"
dt = 0.01
end_time = 0.1
alpha_u = 0.9
alpha_p = 1.0
momentum_rel_tol = 1e-10
momentum_max_iter = 2000
pressure_rel_tol = 1e-10
pressure_max_iter = 20000
"#;

/// ADR 0156's piston case (`consistent_correctors_case_f301.rs` `PISTON`).
const PISTON: &str = r#"
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
[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

/// ADR 0352 T2: the located errors of the key; each case builds without it.
#[test]
fn t2_located_errors_of_the_key() {
    let cavity = parse(&repo_file("example-projects/quick-start/cases/cavity-re100.toml"));
    let arms: Vec<(&str, Case)> = vec![
        ("steady simplec (the quick-start cavity)", cavity),
        (
            "fractional step",
            cylinder(&[("solver.time_advance", json!("fractional_step"))]),
        ),
        ("variable density (low Mach)", parse(LOW_MACH)),
        ("lumped components", parse(LUMPED)),
    ];
    for (name, case) in arms {
        assert!(
            Simulation::new(&case).is_ok(),
            "T2 {name}: does not build without the key"
        );
        let keyed = with(case, &[(KEY, json!(true))]);
        let r = Simulation::new(&keyed);
        let msg = match &r {
            Ok(_) => "built".to_string(),
            Err(d) => d.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        };
        println!("T2 {name}: {msg}");
        let Err(d) = r else {
            panic!("T2 {name}: built with the key")
        };
        assert!(d.iter().any(|x| x.path == KEY), "T2 {name}: {msg}");
    }
}

/// ADR 0352 T3: the key's default leaves the case hash and the written text unchanged; `true` changes the hash.
#[test]
fn t3_the_default_of_the_key() {
    let plain = cylinder(&[]);
    let off = cylinder(&[(KEY, json!(false))]);
    let on = cylinder(&[(KEY, json!(true))]);
    let (h0, h_off, h_on) = (
        plain.tree.content_hash(),
        off.tree.content_hash(),
        on.tree.content_hash(),
    );
    let (toml, js) = (plain.to_toml(), plain.to_json_string());
    println!(
        "T3: hash without the table {h0:016x}, with step_consistent = false {h_off:016x}, with true {h_on:016x}; the written \
         TOML names the key {}, the JSON {}",
        toml.contains("rhie_chow"),
        js.contains("rhie_chow")
    );
    assert_eq!(h0, h_off, "T3: step_consistent = false changes the hash");
    assert_ne!(h0, h_on, "T3: step_consistent = true leaves the hash");
    assert!(
        !toml.contains("rhie_chow") && !js.contains("rhie_chow"),
        "T3: the written text names the key"
    );
}

/// ADR 0352 T4: the tree's dynamic mesh with the key builds and runs; the key acts.
#[test]
fn t4_a_dynamic_mesh_with_the_key() {
    let run = |case: &Case| {
        let mut sim = Simulation::new(case).unwrap_or_else(|e| panic!("{e:?}"));
        for _ in 0..10 {
            sim.step();
        }
        assert!(!sim.diverged(), "T4 stopped");
        ["p", "ux", "uy"]
            .iter()
            .flat_map(|n| bits(&sim.field(n).unwrap()))
            .collect::<Vec<_>>()
    };
    let plain = run(&parse(PISTON));
    let keyed = run(&with(parse(PISTON), &[(KEY, json!(true))]));
    println!(
        "T4: 10 steps with the key, fields different from the run without it {}",
        plain != keyed
    );
    assert!(plain != keyed, "T4: the key does not act on the dynamic mesh");
}
