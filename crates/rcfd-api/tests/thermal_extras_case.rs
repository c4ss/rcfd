//! Wave 21A (ADR 0035) through the settings tree: layered and open thermal interfaces (checks G1, G2, G4, G7). The limits
//! are those registered in the ADR before this file was written; none was changed afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"))
}

fn build(text: &str) -> Simulation {
    Simulation::new(&case(text)).unwrap_or_else(|e| panic!("{e:?}"))
}

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => match Simulation::new(&c) {
            Err(e) => e,
            Ok(_) => vec![],
        },
    }
}

fn has(errs: &[Diagnostic], path: &str) -> bool {
    errs.iter().any(|d| d.path == path)
}

fn patch_area(s: &Simulation, patch: &str) -> f64 {
    let sf = s.geom.mesh.face_area_vectors();
    s.geom
        .mesh
        .patch(patch)
        .unwrap()
        .faces()
        .map(|f| vec3::norm(sf[f]))
        .sum()
}

/// Two solids `k = 1` (x < 0.05) and `k = 10`, 40 cells over `L = 0.1`, ends 400 K and 300 K, `interface_extra` lines in
/// `[thermal_interfaces.i]`.
fn bar_text(interface_extra: &str, algorithm: &str) -> String {
    format!(
        r#"
title = "two solids"
[mesh]
generator = "box_2d"
nx = 40
ny = 1
lx = 0.1
ly = 1.0
[physics]
nu = 0.01
rho = 1.0
cp = 1.0
conductivity = 1.0
[energy]
enabled = true
relax = 1.0
[initial]
temperature = 350.0
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 400.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 300.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "{algorithm}"
convection = "upwind"
frozen_flow = true
tolerance_u = 1e-14
max_iterations = 4
[solids.a]
zone = "box"
box_min = [-10.0, -10.0, -10.0]
box_max = [0.05, 10.0, 10.0]
conductivity = 1.0
density = 1.0
specific_heat = 1.0
[solids.b]
zone = "box"
box_min = [0.05, -10.0, -10.0]
box_max = [10.0, 10.0, 10.0]
conductivity = 10.0
density = 1.0
specific_heat = 1.0
[thermal_interfaces.i]
solid_a = "a"
solid_b = "b"
{interface_extra}
[reports.q_in]
type = "heat_flow"
patch = "xmin"
[reports.q_out]
type = "heat_flow"
patch = "xmax"
"#
    )
}

/// Exact piecewise-linear solution with total interface resistance `r`.
fn bar_exact(x: f64, r: f64) -> (f64, f64) {
    let q = 100.0 / (0.05 / 1.0 + r + 0.05 / 10.0);
    let t = if x < 0.05 {
        400.0 - q * x
    } else {
        400.0 - q * 0.05 - q * r - q * (x - 0.05) / 10.0
    };
    (t, q)
}

fn check_bar(extra: &str, r: f64, tag: &str) {
    let mut s = build(&bar_text(extra, "simple"));
    s.run(10);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    let t = s.field("T").unwrap();
    let cc = s.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| (t[i] - bar_exact(cc[i][0], r).0).abs())
        .fold(0.0, f64::max)
        / 100.0;
    let q = bar_exact(0.0, r).1;
    // extrapolated jump from the two cells next to the interface on each side (cells 19 | 20)
    let tl = t[19] + 0.5 * (t[19] - t[18]);
    let tr = t[20] - 0.5 * (t[21] - t[20]);
    let jump = (tl - tr - q * r).abs() / (q * r);
    println!("{tag}: error/dT {err:.3e} (limit 1e-10), jump relative {jump:.3e} (limit 1e-10)");
    assert!(err <= 1e-10, "{tag} temperature {err}");
    assert!(jump <= 1e-10, "{tag} jump {jump}");
    let _ = patch_area(&s, "xmin");
}

const LAYERS: &str = "layer_thickness = [0.002, 0.001]\nlayer_conductivity = [0.5, 45.0]\n";

#[test]
fn g1_layered_interface_with_contact_resistance() {
    let r = 0.01 + 0.002 / 0.5 + 0.001 / 45.0;
    check_bar(&format!("resistance = 0.01\n{LAYERS}"), r, "G1");
}

#[test]
fn g2_layers_only_resistance_unset() {
    let r = 0.002 / 0.5 + 0.001 / 45.0;
    check_bar(LAYERS, r, "G2");
}

#[test]
fn g4_open_connection_is_adiabatic() {
    let mut open = build(&bar_text("connection = \"open\"\n", "simple"));
    open.run(10);
    assert!(open.failure().is_none(), "{:?}", open.failure());
    let t = open.field("T").unwrap();
    let cc = open.geom.mesh.cell_centres();
    let err = (0..t.len())
        .map(|i| {
            let want = if cc[i][0] < 0.05 { 400.0 } else { 300.0 };
            (t[i] - want).abs()
        })
        .fold(0.0, f64::max)
        / 100.0;
    // reference flow with a closed, resistance-free interface
    let mut closed = build(&bar_text("resistance = 0.0\n", "simple"));
    closed.run(10);
    let a = patch_area(&closed, "xmin");
    let q_ref = 100.0 / (0.05 + 0.005) * a;
    // the interface flow equals the flow through the end walls of each side, and the end wall flows are the Dirichlet
    // conduction into a uniform field, so they must vanish too
    let fl = open.eval_all_reports();
    let q_in = fl[0].as_ref().unwrap().abs();
    let q_out = fl[1].as_ref().unwrap().abs();
    println!(
        "G4: error/dT {err:.3e} (limit 1e-10), end-wall flows {:.3e}, {:.3e} of {q_ref:.3e} (limit 1e-12 relative)",
        q_in / q_ref,
        q_out / q_ref
    );
    assert!(err <= 1e-10, "G4 temperature {err}");
    assert!(
        q_in / q_ref <= 1e-12 && q_out / q_ref <= 1e-12,
        "G4 flows {q_in} {q_out}"
    );
}

#[test]
fn g7_located_errors() {
    let cases: [(&str, &str, &str); 9] = [
        (
            "layer_thickness = [0.001]\nlayer_conductivity = [1.0, 2.0]\n",
            "simple",
            "thermal_interfaces.i.layer_conductivity",
        ),
        (
            "layer_thickness = [0.001]\nlayer_conductivity = [-1.0]\n",
            "simple",
            "thermal_interfaces.i.layer_conductivity",
        ),
        (
            "layer_thickness = [0.0]\nlayer_conductivity = [1.0]\n",
            "simple",
            "thermal_interfaces.i.layer_thickness",
        ),
        ("", "simple", "thermal_interfaces.i.resistance"),
        ("resistance = -0.5\n", "simple", "thermal_interfaces.i.resistance"),
        ("connection = \"ajar\"\n", "simple", "thermal_interfaces.i.connection"),
        (
            "connection = \"open\"\nresistance = 0.1\n",
            "simple",
            "thermal_interfaces.i.connection",
        ),
        (
            "connection = \"open\"\nlayer_thickness = [0.001]\nlayer_conductivity = [1.0]\n",
            "simple",
            "thermal_interfaces.i.connection",
        ),
        ("connection = \"open\"\n", "simple", ""),
    ];
    for (i, (extra, alg, path)) in cases.iter().enumerate() {
        let e = errors(&bar_text(extra, alg));
        if path.is_empty() {
            assert!(e.is_empty(), "case {i}: {e:?}");
        } else {
            assert!(has(&e, path), "case {i}: expected {path}, got {e:?}");
        }
    }
    // a steady open interface that leaves a side without a fixed boundary
    let text = bar_text("connection = \"open\"\n", "simple").replace(
        "[boundaries.xmax]\nvelocity = \"fixed\"\ntemperature = \"fixed\"\ntemperature_value = 300.0",
        "[boundaries.xmax]\nvelocity = \"fixed\"",
    );
    let e = errors(&text);
    assert!(has(&e, "thermal_interfaces.i.connection"), "{e:?}");
}
