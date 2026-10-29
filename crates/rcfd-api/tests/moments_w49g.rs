//! ADR 0170 (job 49G): `moment` reports follow the force default. With a turbulence model, `total` (default) and
//! `viscous` take the momentum equation's face viscosity (D1); `pressure` and `*_molecular` keep `physics.nu` (D2, D4);
//! laminar runs are unchanged. Limits M1-M3 of ADR 0170, registered before this file was written.

use rcfd_api::case::Case;
use rcfd_api::sim::{ReportDef, Simulation};

/// ADR 0164's plate (Re = 5e6 per m, wall cell for y+ about 40), as in `defaults_w49c.rs`.
const PLATE: &str = r#"
title = "plate, y+ 40"
[mesh]
generator = "box_2d"
nx = 30
ny = 24
lx = 2.0
ly = 1.0
wall_layers = 10
layer_walls = "lower"
layer_growth = 1.2
target_y_plus = 40.0
layer_reference_velocity = 1.0
layer_reference_length = 1.0
[physics]
nu = 2e-7
[initial]
velocity = [1.0, 0.0, 0.0]
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "symmetry"
[solver]
algorithm = "simplec"
max_iterations = 100000
"#;

const COMPONENTS: [&str; 7] = [
    "total",
    "viscous",
    "pressure",
    "total_effective",
    "viscous_effective",
    "total_molecular",
    "viscous_molecular",
];

/// The case with one moment report about z on the plate per component, and one with the component unset.
fn with_moments(text: &str) -> String {
    let mut t = text.to_string();
    for c in COMPONENTS {
        t += &format!(
            "[reports.m_{c}]\ntype = \"moment\"\npatch = \"ymin\"\ndirection = [0.0, 0.0, 1.0]\norigin = [0.5, 0.0, 0.0]\ncomponent = \"{c}\"\n"
        );
    }
    t += "[reports.m_unset]\ntype = \"moment\"\npatch = \"ymin\"\ndirection = [0.0, 0.0, 1.0]\norigin = [0.5, 0.0, 0.0]\n";
    t
}

fn values(text: &str, model: Option<&str>, n: usize) -> Vec<(String, f64)> {
    let mut c = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    if let Some(m) = model {
        c.set("turbulence.model", serde_json::json!(m)).unwrap();
    }
    let mut s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    ReportDef::from_case(&c)
        .iter()
        .map(|r| (r.name.clone(), s.eval_report(r).unwrap()))
        .collect()
}

fn get(v: &[(String, f64)], name: &str) -> f64 {
    v.iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no report {name}"))
        .1
}

fn same(v: &[(String, f64)], a: &str, b: &str) {
    assert_eq!(
        get(v, a).to_bits(),
        get(v, b).to_bits(),
        "{a} = {} and {b} = {} differ",
        get(v, a),
        get(v, b)
    );
}

/// The turbulent checks of M1 / M2: default, `total` (and `viscous` when `with_viscous`) are the effective moment;
/// control: the molecular moment differs.
fn turbulent(v: &[(String, f64)], with_viscous: bool) {
    same(v, "m_unset", "m_total_effective");
    same(v, "m_total", "m_total_effective");
    if with_viscous {
        same(v, "m_viscous", "m_total_effective");
    }
    assert!(
        get(v, "m_total_effective").is_finite() && get(v, "m_total_molecular").is_finite(),
        "{v:?}"
    );
    assert_ne!(
        get(v, "m_unset").to_bits(),
        get(v, "m_total_molecular").to_bits(),
        "control: the effective and molecular moments are equal: {v:?}"
    );
}

/// M1: Spalart-Allmaras with its wall function at y+ about 40 (400 iterations).
#[test]
fn m1_sa_default_moment_is_effective() {
    let v = values(&with_moments(PLATE), Some("spalart_allmaras"), 400);
    println!("M1 {v:?}");
    turbulent(&v, true);
    // D2: a moment does not apply the component; `pressure` keeps the whole moment with physics.nu
    same(&v, "m_pressure", "m_total_molecular");
    same(&v, "m_viscous_molecular", "m_total_molecular");
    same(&v, "m_viscous_effective", "m_total_effective");
}

/// M2: k-omega SST (the FAN-01 model) on the same plate, with the inlet length SST's inlet requires (400 iterations).
#[test]
fn m2_sst_default_moment_is_effective() {
    let text = with_moments(PLATE).replace(
        "velocity_value = [1.0, 0.0, 0.0]\n[boundaries.xmax]",
        "velocity_value = [1.0, 0.0, 0.0]\nturbulence_length = 0.01\n[boundaries.xmax]",
    );
    assert!(text.contains("turbulence_length = 0.01"));
    let v = values(&text, Some("k_omega_sst"), 400);
    println!("M2 {v:?}");
    turbulent(&v, false);
}

/// M3: laminar, constant viscosity and the ADR 0166 T2 power law (40 iterations): every spelling except the
/// `*_effective` ones of the power law is the molecular moment, bit for bit.
#[test]
fn m3_laminar_moment_is_unchanged() {
    let power = with_moments(PLATE)
        + "[rheology]\nmodel = \"power_law\"\nconsistency_index = 2e-7\npower_index = 0.9\nmax_viscosity = 1e-5\n";
    for (name, text) in [("constant", with_moments(PLATE)), ("power law", power)] {
        let v = values(&text, None, 40);
        println!("M3 {name} {v:?}");
        for c in ["m_unset", "m_total", "m_viscous", "m_pressure", "m_viscous_molecular"] {
            same(&v, c, "m_total_molecular");
        }
        assert!(get(&v, "m_total_molecular").is_finite(), "{v:?}");
    }
}
