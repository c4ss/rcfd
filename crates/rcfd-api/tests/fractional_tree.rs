//! ADR 0037 checks F11 (tree equals library), F12 (located errors), F13 (defaults unchanged) of the fractional-step option.
//! Limits as registered before the code existed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

fn tg_text(solver_extra: &str, other: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 24
ny = 24
lx = {l:.17e}
ly = {l:.17e}
periodic_x = true
periodic_y = true
[physics]
nu = 0.05
[initial]
velocity_expression_x = "sin(x/(1 [m]))*cos(y/(1 [m])) * 1 [m/s]"
velocity_expression_y = "-cos(x/(1 [m]))*sin(y/(1 [m])) * 1 [m/s]"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.02
end_time = 0.4
{solver_extra}
{other}
"#,
        l = 2.0 * PI
    )
}

fn sim(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

fn fields(s: &Simulation) -> Vec<Vec<f64>> {
    ["ux", "uy", "p"].iter().map(|n| s.field(n).unwrap()).collect()
}

fn bits_equal(a: &[Vec<f64>], b: &[Vec<f64>]) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| x.iter().zip(y).all(|(p, q)| p.to_bits() == q.to_bits()))
}

#[test]
fn f11_tree_equals_library() {
    let mut s = sim(&tg_text("time_advance = \"fractional_step\"", ""));
    s.run(1000);
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(24, 24, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(0.05);
    st.convection = ConvScheme::Central;
    st.fractional = Some(FractionalSettings::default());
    let mut lib = PimpleSolver::new(geom, u, p, st);
    lib.run_fixed(0.4, 0.02);
    let a = fields(&s);
    let ux: Vec<f64> = lib.u.values.iter().map(|v| v[0]).collect();
    let uy: Vec<f64> = lib.u.values.iter().map(|v| v[1]).collect();
    let b = vec![ux, uy, lib.p.values.clone()];
    let mut worst = 0.0f64;
    for (x, y) in a.iter().zip(&b) {
        let scale = y.iter().fold(1e-300f64, |m, v| m.max(v.abs()));
        for (p, q) in x.iter().zip(y) {
            worst = worst.max((p - q).abs() / scale);
        }
    }
    println!(
        "F11 tree vs library worst relative difference {worst:.3e} (limit 1e-12), bits equal {}",
        bits_equal(&a, &b)
    );
    assert!(worst <= 1e-12);
}

#[test]
fn f12_located_errors() {
    let fs = "time_advance = \"fractional_step\"";
    let has = |e: &[rcfd_api::case::Diagnostic], path: &str| e.iter().any(|d| d.path == path && !d.message.is_empty());
    // pimple
    let t = tg_text(fs, "").replace("\"piso\"", "\"pimple\"");
    let e = Case::parse_toml(&t)
        .map(|c| Simulation::new(&c).err().unwrap_or_default())
        .unwrap_or_else(|e| e);
    assert!(has(&e, "solver.time_advance"), "pimple: {e:?}");
    // steady
    let t = tg_text(fs, "").replace("\"piso\"", "\"simple\"");
    let e = Case::parse_toml(&t)
        .map(|c| Simulation::new(&c).err().unwrap_or_default())
        .unwrap_or_else(|e| e);
    assert!(has(&e, "solver.time_advance"), "simple: {e:?}");
    // energy
    let t = tg_text(fs, "[energy]\nenabled = true");
    let c = Case::parse_toml(&t).unwrap();
    let e2 = Simulation::new(&c).err().unwrap_or_default();
    assert!(has(&e2, "solver.time_advance"), "energy: {e2:?}");
    // turbulence
    let t = tg_text(fs, "[turbulence]\nmodel = \"smagorinsky\"");
    let c = Case::parse_toml(&t).unwrap();
    let e3 = c
        .semantic_errors()
        .into_iter()
        .chain(Simulation::new(&c).err().unwrap_or_default())
        .collect::<Vec<_>>();
    assert!(has(&e3, "solver.time_advance"), "turbulence: {e3:?}");
    // rheology
    let t = tg_text(
        fs,
        "[rheology]\nmodel = \"power_law\"\nconsistency_index = 0.05\npower_index = 0.8",
    );
    let c = Case::parse_toml(&t).unwrap();
    let e4 = Simulation::new(&c).err().unwrap_or_default();
    assert!(has(&e4, "solver.time_advance"), "rheology: {e4:?}");
    // body force: already a located error of the transient algorithms (physics.body_force), so the fractional option never sees it
    let t = tg_text(fs, "").replace(
        "[physics]",
        "[physics]
body_force = [0.0, 1.0, 0.0]",
    );
    let e5 = Case::parse_toml(&t).err().unwrap_or_default();
    assert!(has(&e5, "physics.body_force"), "body force: {e5:?}");
    // bdf2_bounded
    let t = tg_text(&format!("{fs}\ntime_scheme = \"bdf2_bounded\""), "");
    let c = Case::parse_toml(&t).unwrap();
    let e6 = Simulation::new(&c).err().unwrap_or_default();
    assert!(has(&e6, "solver.time_advance"), "bdf2_bounded: {e6:?}");
    // full stress
    let t = tg_text(&format!("{fs}\nviscous_stress = \"full\""), "");
    let c = Case::parse_toml(&t).unwrap();
    let e7 = Simulation::new(&c).err().unwrap_or_default();
    assert!(has(&e7, "solver.time_advance"), "full stress: {e7:?}");
    // unknown enum value is a schema error
    let t = tg_text("time_advance = \"nonsense\"", "");
    assert!(Case::parse_toml(&t).is_err() || !Case::parse_toml(&t).unwrap().semantic_errors().is_empty());
}

#[test]
fn f13_default_is_bit_identical() {
    let mut a = sim(&tg_text("", ""));
    let mut b = sim(&tg_text("time_advance = \"iterative\"", ""));
    a.run(1000);
    b.run(1000);
    assert!(bits_equal(&fields(&a), &fields(&b)));
}
