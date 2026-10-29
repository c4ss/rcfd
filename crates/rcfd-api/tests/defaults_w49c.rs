//! ADR 0166 (job 49C): the effective wall shear is the default of force reports with a turbulence model (D1), moment
//! reports keep their default (D3), the old values stay spellable as `total_molecular` / `viscous_molecular` (D4), and a
//! Spalart-Allmaras free-stream far field takes the `freestream` turbulence role by default (D2). Limits T1-T6 of ADR 0166,
//! registered before this file was written.

use rcfd_api::case::Case;
use rcfd_api::sim::{ReportDef, Simulation};

/// ADR 0164's plate (Re = 5e6 per m, wall cell for y+ about 40), with every component of the drag and three moments.
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

fn with_reports(text: &str) -> String {
    let mut t = text.to_string();
    for c in COMPONENTS {
        t += &format!(
            "[reports.d_{c}]\ntype = \"force_coefficient\"\npatch = \"ymin\"\ndirection = [1.0, 0.0, 0.0]\na_ref = 0.02\ncomponent = \"{c}\"\n"
        );
    }
    // the default component, unset
    t += "[reports.d_unset]\ntype = \"force_coefficient\"\npatch = \"ymin\"\ndirection = [1.0, 0.0, 0.0]\na_ref = 0.02\n";
    t += "[reports.m_unset]\ntype = \"moment\"\npatch = \"ymin\"\ndirection = [0.0, 0.0, 1.0]\n";
    for c in ["total_molecular", "total_effective"] {
        t += &format!(
            "[reports.m_{c}]\ntype = \"moment\"\npatch = \"ymin\"\ndirection = [0.0, 0.0, 1.0]\ncomponent = \"{c}\"\n"
        );
    }
    t
}

fn run(c: &Case, n: usize) -> Simulation {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    for _ in 0..n {
        s.step();
    }
    s
}

fn values(s: &Simulation, c: &Case) -> Vec<(String, f64)> {
    ReportDef::from_case(c)
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

/// T1: laminar, constant viscosity: every spelling of a part is the same number, bit for bit.
#[test]
fn t1_laminar_constant_viscosity_components_are_equal() {
    let c = Case::parse_toml(&with_reports(PLATE)).unwrap();
    let v = values(&run(&c, 40), &c);
    for b in ["d_total_effective", "d_total_molecular", "d_unset"] {
        same(&v, "d_total", b);
    }
    for b in ["d_viscous_effective", "d_viscous_molecular"] {
        same(&v, "d_viscous", b);
    }
    same(&v, "m_unset", "m_total_molecular");
    same(&v, "m_unset", "m_total_effective");
    assert!(get(&v, "d_viscous") > 0.0, "{v:?}");
}

/// T2: laminar, variable viscosity (power law): the default stays the molecular `physics.nu` (ADR 0166 D1 gates on a
/// turbulence model, not on a face viscosity); control: the face viscosity exists, so `viscous_effective` differs.
#[test]
fn t2_laminar_variable_viscosity_default_is_unchanged() {
    let text = with_reports(PLATE)
        + "[rheology]\nmodel = \"power_law\"\nconsistency_index = 2e-7\npower_index = 0.9\nmax_viscosity = 1e-5\n";
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let v = values(&run(&c, 40), &c);
    same(&v, "d_total", "d_total_molecular");
    same(&v, "d_unset", "d_total_molecular");
    same(&v, "d_viscous", "d_viscous_molecular");
    same(&v, "m_unset", "m_total_molecular");
    assert!(
        get(&v, "d_viscous").is_finite() && get(&v, "d_viscous_effective").is_finite(),
        "{v:?}"
    );
    assert_ne!(
        get(&v, "d_viscous").to_bits(),
        get(&v, "d_viscous_effective").to_bits(),
        "control: a power law gives a face viscosity other than physics.nu: {v:?}"
    );
}

/// T3 / T4: Spalart-Allmaras with its wall function at y+ about 40: the default force components are the effective
/// ones; the molecular ones are ADR 0164 C2/C3's (ratio > 1.5, pressure part the same); a moment keeps the molecular
/// default (D3), control: its effective value differs.
#[test]
fn t3_t4_sa_default_force_is_effective_and_moment_is_unchanged() {
    let mut c = Case::parse_toml(&with_reports(PLATE)).unwrap();
    c.set("turbulence.model", serde_json::json!("spalart_allmaras"))
        .unwrap();
    let v = values(&run(&c, 400), &c);
    same(&v, "d_total", "d_total_effective");
    same(&v, "d_unset", "d_total_effective");
    same(&v, "d_viscous", "d_viscous_effective");
    let (mol, eff) = (get(&v, "d_viscous_molecular"), get(&v, "d_viscous"));
    assert!(mol > 0.0 && eff.is_finite(), "{v:?}");
    assert!(eff / mol > 1.5, "effective {eff} molecular {mol}");
    let p_default = get(&v, "d_total") - eff;
    let p_mol = get(&v, "d_total_molecular") - mol;
    assert!(
        (p_default - p_mol).abs() <= 1e-12 * (get(&v, "d_total").abs() + 1e-30),
        "{v:?}"
    );
    // T4
    same(&v, "m_unset", "m_total_molecular");
    assert_ne!(
        get(&v, "m_unset").to_bits(),
        get(&v, "m_total_effective").to_bits(),
        "control: {v:?}"
    );
}

/// T5 / T6 set-up: the plate with free-stream inflow and outflow patches (ADR 0164 C4) and the given turbulence lines.
fn freestream_plate(model: &str, turbulence_line: &str) -> Case {
    let text = PLATE
        .replace(
            "[boundaries.xmin]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]",
            &format!("[boundaries.xmin]\nvelocity = \"freestream\"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = \"freestream\"\n{turbulence_line}"),
        )
        .replace(
            "[boundaries.xmax]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"",
            &format!("[boundaries.xmax]\nvelocity = \"freestream\"\nvelocity_value = [1.0, 0.0, 0.0]\npressure = \"freestream\"\n{turbulence_line}"),
        )
        + &format!("[turbulence]\nmodel = \"{model}\"\n");
    assert_eq!(text.matches("velocity = \"freestream\"").count(), 2);
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let errs = c.semantic_errors();
    assert!(errs.is_empty(), "{model} {turbulence_line:?}: {errs:?}");
    c
}

/// Bits of the named cell fields and of the face flux after `n` steps.
fn bits(c: &Case, fields: &[&str], n: usize) -> Vec<u64> {
    let s = run(c, n);
    let mut out = Vec::new();
    for f in fields {
        out.extend(
            s.field(f)
                .unwrap_or_else(|e| panic!("{f}: {e}"))
                .iter()
                .map(|x| x.to_bits()),
        );
    }
    out.extend(s.flux().iter().map(|x| x.to_bits()));
    out
}

/// T5: Spalart-Allmaras: the far field without a `turbulence` line is the explicit `freestream` role, bit for bit; the
/// inflow faces hold nuTilda = 3 nu; control: the explicit `zero_gradient` (the old default) differs.
#[test]
fn t5_sa_far_field_default_is_freestream() {
    let fields = ["ux", "uy", "p", "nuTilda"];
    let auto = freestream_plate("spalart_allmaras", "");
    let explicit = freestream_plate("spalart_allmaras", "turbulence = \"freestream\"");
    let old = freestream_plate("spalart_allmaras", "turbulence = \"zero_gradient\"");
    let a = bits(&auto, &fields, 30);
    assert_eq!(
        a,
        bits(&explicit, &fields, 30),
        "auto against the explicit freestream role"
    );
    assert_ne!(a, bits(&old, &fields, 30), "control: the explicit zero_gradient role");
    let s = run(&auto, 30);
    let g = &s.geom;
    let nb = s.boundary_field("nuTilda").unwrap();
    for f in g.mesh.patch("xmin").unwrap().faces() {
        assert_eq!(nb[f - g.n_internal], 3.0 * 2e-7, "inflow face {f}");
    }
}

/// T6: SST and k-epsilon keep the old far-field role under `auto`: no validation error (checked in
/// `freestream_plate`), and the run is the explicit `zero_gradient` one, bit for bit.
#[test]
fn t6_sst_and_k_epsilon_far_field_default_is_unchanged() {
    let fields = ["ux", "uy", "p", "k"];
    for model in ["k_omega_sst", "k_epsilon"] {
        let auto = freestream_plate(model, "");
        let old = freestream_plate(model, "turbulence = \"zero_gradient\"");
        assert_eq!(bits(&auto, &fields, 30), bits(&old, &fields, 30), "{model}");
    }
}
