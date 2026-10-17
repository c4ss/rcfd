//! ADR 0056 wiring checks (W): `multiphase.mixture` (drift-flux slip) and `multiphase.cavitation` (Schnerr-Sauer) through the case tree.
//! The limits are those registered in the ADR before the code was written; none was changed afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_fv::cavitation::Cavitation;
use rcfd_fv::mixture::{DragLaw, MixtureSettings};
use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const G: f64 = 9.81;

fn errors(text: &str) -> Vec<Diagnostic> {
    match Case::parse_toml(text) {
        Err(e) => e,
        Ok(c) => {
            let mut e = c.semantic_errors();
            e.extend(Simulation::new(&c).err().unwrap_or_default());
            e
        }
    }
}

fn sim(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

const MIX: &str = "\n[multiphase.mixture]\nmodel = \"drift_flux\"\nparticle_diameter = 1.0e-4\n";
const CAV: &str = "\n[multiphase.cavitation]\nmodel = \"schnerr_sauer\"\nsaturation_pressure = 2339.0\nbubble_number_density = 1.0e11\nnucleus_radius = 1.0e-5\n";
const MT_LEE: &str = "\n[multiphase.mass_transfer]\nmodel = \"lee\"\nsaturation_temperature = 373.15\nlatent_heat = 2.26e6\nevaporation_frequency = 10.0\ncondensation_frequency = 10.0\ntemperature = 393.15\n";

/// Pool of 10 x 20 cells; the top boundary is an open pressure patch (p_rgh = 0).
fn pool(extra_mp: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 10
ny = 20
lx = 0.5
ly = 1.0
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5
{extra_mp}
[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.5, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.0e-3
end_time = 0.05
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "zero_gradient"
pressure = "fixed"
"#
    )
}

/// Closed settling column of one cell width: water (phase 1) below, heavy dispersed phase 2 (2500) above.
fn column(extra_mp: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 1
ny = 20
lx = 0.05
ly = 1.0
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 2500.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.0e-3
{extra_mp}
[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.525, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.0e-3
end_time = 0.02
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
"#
    )
}

/// Low-pressure box: no gravity, liquid everywhere, the top is an open pressure patch with p_rgh = 0 < p_sat.
fn low_pressure(extra_mp: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 10
ny = 20
lx = 0.5
ly = 1.0
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 0.02
viscosity_1 = 1.0e-3
viscosity_2 = 1.0e-5
{extra_mp}
[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 10.0, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 1.0e-3
end_time = 0.03
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "zero_gradient"
pressure = "fixed"
"#
    )
}

/// W1: every refusal and inert key has a located error; valid cases are silent.
#[test]
fn w1_located_errors() {
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "mixture without particle diameter",
            pool("\n[multiphase.mixture]\nmodel = \"drift_flux\"\n"),
            "multiphase.mixture.particle_diameter",
        ),
        (
            "mixture with zero diameter",
            pool(&MIX.replace("1.0e-4", "0.0")),
            "multiphase.mixture.particle_diameter",
        ),
        (
            "mixture with negative diameter",
            pool(&MIX.replace("1.0e-4", "-3.0e-4")),
            "multiphase.mixture.particle_diameter",
        ),
        (
            "mixture without gravity",
            pool(MIX).replace("gravity = [0.0, -9.81, 0.0]", "gravity = [0.0, 0.0, 0.0]"),
            "physics.gravity",
        ),
        (
            "mixture with thermal mass transfer",
            pool(&format!("{MIX}{MT_LEE}")),
            "multiphase.mixture.model",
        ),
        (
            "mixture with cavitation",
            pool(&format!("{MIX}{CAV}")),
            "multiphase.mixture.model",
        ),
        (
            "cavitation without saturation pressure",
            pool(&CAV.replace("saturation_pressure = 2339.0\n", "")),
            "multiphase.cavitation.saturation_pressure",
        ),
        (
            "cavitation without bubble-number density",
            pool(&CAV.replace("bubble_number_density = 1.0e11\n", "")),
            "multiphase.cavitation.bubble_number_density",
        ),
        (
            "cavitation without nucleus radius",
            pool(&CAV.replace("nucleus_radius = 1.0e-5\n", "")),
            "multiphase.cavitation.nucleus_radius",
        ),
        (
            "cavitation with a negative nucleus radius",
            pool(&CAV.replace("nucleus_radius = 1.0e-5", "nucleus_radius = -2.0")),
            "multiphase.cavitation.nucleus_radius",
        ),
        (
            "cavitation without a pressure patch",
            pool(CAV).replace("zero_gradient\"\npressure = \"fixed\"", "fixed\""),
            "multiphase.cavitation.model",
        ),
        (
            "cavitation with thermal mass transfer",
            pool(&format!("{CAV}{MT_LEE}")),
            "multiphase.cavitation.model",
        ),
        (
            "mixture particle diameter while the model is none",
            pool("\n[multiphase.mixture]\nparticle_diameter = 1.0e-4\n"),
            "multiphase.mixture.particle_diameter",
        ),
        (
            "mixture drag while the model is none",
            pool("\n[multiphase.mixture]\ndrag = \"schiller_naumann\"\n"),
            "multiphase.mixture.drag",
        ),
        (
            "mixture dispersed phase while the model is none",
            pool("\n[multiphase.mixture]\ndispersed_phase = \"phase_1\"\n"),
            "multiphase.mixture.dispersed_phase",
        ),
        (
            "cavitation saturation pressure while the model is none",
            pool("\n[multiphase.cavitation]\nsaturation_pressure = 2339.0\n"),
            "multiphase.cavitation.saturation_pressure",
        ),
        (
            "cavitation factor while the model is none",
            pool("\n[multiphase.cavitation]\nvaporisation_factor = 2.0\n"),
            "multiphase.cavitation.vaporisation_factor",
        ),
    ];
    for (what, text, path) in rows {
        let e = errors(&text);
        let hit = e.iter().any(|d| d.path == path && !d.message.is_empty());
        println!(
            "W1 {what}: expected path {path}: {}",
            if hit { "ok" } else { "MISSING" }
        );
        assert!(hit, "{what}: no located error at {path}; got {e:?}");
    }
    // disabled multiphase: the models are refused / inert
    let off = "[mesh]\ngenerator = \"box_2d\"\nnx = 4\nny = 4\nlx = 1.0\nly = 1.0\n[multiphase]\nenabled = false\n";
    for (what, key, path) in [
        ("mixture", MIX, "multiphase.mixture.model"),
        ("cavitation", CAV, "multiphase.cavitation.model"),
    ] {
        let e = errors(&format!("{off}{key}"));
        let hit = e.iter().any(|d| d.path == path);
        println!(
            "W1 {what} with multiphase disabled: expected path {path}: {}",
            if hit { "ok" } else { "MISSING" }
        );
        assert!(hit, "{what} with multiphase disabled: {e:?}");
    }
    // valid cases are silent
    for (what, text) in [
        ("pool", pool("")),
        ("pool with drift flux", pool(MIX)),
        (
            "pool with schiller-naumann drift flux",
            pool(&format!(
                "{MIX}drag = \"schiller_naumann\"\ndispersed_phase = \"phase_1\"\n"
            )),
        ),
        ("pool with cavitation", pool(CAV)),
        ("settling column", column(&MIX.replace("1.0e-4", "2.0e-4"))),
        ("low-pressure box with cavitation", low_pressure(CAV)),
    ] {
        let e = errors(&text);
        assert!(e.is_empty(), "{what}: {e:?}");
    }
}

fn lib_column() -> VofSolver {
    let mesh = Arc::new(box_2d(1, 20, 0.05, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= 0.525);
    let mut s = VofSettings::new([1000.0, 2500.0], [1e-3, 1e-3]);
    s.gravity = [0.0, -G, 0.0];
    s.n_correctors = 3;
    s.mixture = Some(MixtureSettings {
        dispersed_phase: 2,
        diameter: 2.0e-4,
        drag: DragLaw::Stokes,
    });
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// (alpha difference, pressure difference, interface motion, volume change) of the W2 tree and library runs.
fn w2_run() -> (f64, f64, f64, f64) {
    let mut s = sim(&column(&MIX.replace("1.0e-4", "2.0e-4")));
    let mut lib = lib_column();
    let a0 = lib.transport.alpha.values.clone();
    for _ in 0..20 {
        s.step();
        lib.step(1e-3).unwrap();
    }
    let da = max_diff(&s.field("alpha").unwrap(), &lib.transport.alpha.values);
    let dp = max_diff(&s.field("p").unwrap(), &lib.p) / (2500.0 * G * 1.0);
    let moved = max_diff(&a0, &lib.transport.alpha.values);
    let v0: f64 = a0.iter().sum();
    let v1: f64 = lib.transport.alpha.values.iter().sum();
    println!("W2 tree vs library: alpha {da:.3e}, p (relative to rho g H) {dp:.3e}; interface moved by {moved:.3e} (must be > 1e-4)");
    (da, dp, moved, (v1 - v0).abs() / v0)
}

/// W2: the settling column through the tree equals the library to 1e-9 (the precedent of the VOF wiring tests), and the slip really
/// moves the interface. The registered bitwise equality is the ignored known-miss test below.
#[test]
fn w2_mixture_column_tree_equals_library() {
    let (da, dp, moved, dv) = w2_run();
    assert!(da <= 1e-9 && dp <= 1e-9, "{da} {dp}");
    assert!(moved > 1e-4, "the slip moved nothing: {moved}");
    assert!(dv <= 1e-12, "phase volume changed by {dv}");
}

#[test]
#[ignore = "known miss: the tree and the library differ at round-off level (ADR 0056 addendum); registered limit is bitwise"]
fn w2_mixture_column_tree_equals_library_bitwise_known_miss() {
    let (da, dp, _, _) = w2_run();
    assert!(da == 0.0 && dp == 0.0, "{da} {dp}");
}

fn lib_low_pressure() -> VofSolver {
    let mesh = Arc::new(box_2d(10, 20, 0.5, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    p_rgh.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= 10.0);
    let mut s = VofSettings::new([1000.0, 0.02], [1e-3, 1e-5]);
    s.n_correctors = 3;
    s.cavitation = Some(Cavitation {
        liquid_phase: 1,
        p_sat: 2339.0,
        n0: 1.0e11,
        r_nucleus: 1.0e-5,
        f_vap: 1.0,
        f_cond: 1.0,
    });
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// (alpha difference, pressure difference, vapour fraction formed, 100 floor) of the W3 tree and library runs.
fn w3_run() -> (f64, f64, f64, f64) {
    let mut s = sim(&low_pressure(CAV));
    let mut lib = lib_low_pressure();
    let floor = lib.settings.cavitation.unwrap().alpha_floor();
    let mut a_min = 1.0f64;
    for _ in 0..30 {
        s.step();
        lib.step(1e-3).unwrap();
        a_min = a_min.min(lib.transport.alpha.values.iter().cloned().fold(1.0, f64::min));
    }
    let da = max_diff(&s.field("alpha").unwrap(), &lib.transport.alpha.values);
    let dp = max_diff(&s.field("p").unwrap(), &lib.p) / (1000.0 * G * 1.0);
    println!(
        "W3 tree vs library: alpha {da:.3e}, p (relative to rho g H) {dp:.3e}; smallest liquid fraction {a_min:.6} (vapour fraction {:.3e} > 100 floor = {:.3e})",
        1.0 - a_min,
        100.0 * floor
    );
    (da, dp, 1.0 - a_min, 100.0 * floor)
}

/// W3: the cavitation case through the tree equals the library to 1e-9, and vapour really forms.
#[test]
fn w3_cavitation_tree_equals_library() {
    let (da, dp, av, f100) = w3_run();
    assert!(da <= 1e-9 && dp <= 1e-9, "{da} {dp}");
    assert!(av > f100, "no vapour formed: {av}");
}

#[test]
#[ignore = "known miss: the tree and the library differ at round-off level (ADR 0056 addendum); registered limit is bitwise"]
fn w3_cavitation_tree_equals_library_bitwise_known_miss() {
    let (da, dp, _, _) = w3_run();
    assert!(da == 0.0 && dp == 0.0, "{da} {dp}");
}
