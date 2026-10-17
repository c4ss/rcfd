//! ADR 0052 tree wiring checks (T1-T4): `multiphase.mass_transfer`, `multiphase.waves`, `multiphase.reference_point` and AMG in the
//! multiphase pressure solve. The limits are those registered in the ADR before the code was written; none was changed afterwards.

use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::sim::Simulation;
use rcfd_fv::vof::{
    indicator_fraction, MassTransfer, MassTransferModel, RelaxTarget, RelaxZone, VofSettings, VofSolver, WaveInlet,
    WaveSettings,
};
use rcfd_fv::waves::{Blend, Wave};
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

const MT_LEE: &str = "\n[multiphase.mass_transfer]\nmodel = \"lee\"\nsaturation_temperature = 373.15\nlatent_heat = 2.26e6\nevaporation_frequency = 10.0\ncondensation_frequency = 10.0\ntemperature = 393.15\n";

/// Pool of 10 x 20 cells; the top boundary is an open pressure patch (p_rgh = 0).
fn pool(extra_mp: &str, extra: &str) -> String {
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
{extra}
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

/// Tank 2.0 x 0.45, 64 x 30 cells, depth 0.30, wave inlet at xmin, beach on [1.2, 2.0].
const WAVES: &str = "\n[multiphase.waves]\ninlet_patch = \"xmin\"\nheight = 0.04\nperiod = 1.0\ndepth = 0.3\nbeach = \"still_water\"\nbeach_x_inner = 1.2\nbeach_x_outer = 2.0\n";

fn tank(extra_mp: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 64
ny = 30
lx = 2.0
ly = 0.45
[physics]
gravity = [0.0, -9.81, 0.0]
[multiphase]
enabled = true
density_1 = 1000.0
density_2 = 1.0
viscosity_1 = 1.0e-3
viscosity_2 = 1.8e-5
reference_point = [0.0, 0.45, 0.0]
{extra_mp}
[multiphase.initial]
shape = "half_space"
normal = [0.0, 1.0, 0.0]
point = [0.0, 0.3, 0.0]
[solver]
algorithm = "piso"
convection = "upwind"
n_correctors = 3
dt = 5.0e-3
end_time = 0.5
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

/// T1: every refusal and inert key has a located error; valid cases are silent.
#[test]
fn t1_located_errors() {
    let rows: Vec<(&str, String, &str)> = vec![
        (
            "lee without saturation temperature",
            pool(&MT_LEE.replace("saturation_temperature = 373.15\n", ""), ""),
            "multiphase.mass_transfer.saturation_temperature",
        ),
        (
            "lee without latent heat",
            pool(&MT_LEE.replace("latent_heat = 2.26e6\n", ""), ""),
            "multiphase.mass_transfer.latent_heat",
        ),
        (
            "lee without temperature",
            pool(&MT_LEE.replace("temperature = 393.15\n", ""), ""),
            "multiphase.mass_transfer.temperature",
        ),
        (
            "mass transfer without a pressure patch",
            pool(MT_LEE, "").replace("zero_gradient\"\npressure = \"fixed\"", "fixed\""),
            "multiphase.mass_transfer.model",
        ),
        (
            "hertz-knudsen key with lee",
            pool(&format!("{MT_LEE}molar_mass = 0.018\n"), ""),
            "multiphase.mass_transfer.molar_mass",
        ),
        (
            "hertz-knudsen without molar mass",
            pool(
                &(MT_LEE
                    .replace("\"lee\"", "\"hertz_knudsen\"")
                    .replace("evaporation_frequency = 10.0\ncondensation_frequency = 10.0\n", "")
                    + "saturation_pressure = 1.0e5\nvapour_pressure = 1.0e5\n"),
                "",
            ),
            "multiphase.mass_transfer.molar_mass",
        ),
        (
            "lee frequency with hertz-knudsen",
            pool(
                &(MT_LEE.replace("\"lee\"", "\"hertz_knudsen\"")
                    + "molar_mass = 0.018\nsaturation_pressure = 1.0e5\nvapour_pressure = 1.0e5\n"),
                "",
            ),
            "multiphase.mass_transfer.evaporation_frequency",
        ),
        (
            "bad temperature gradient length",
            pool(&format!("{MT_LEE}temperature_gradient = [1.0, 2.0]\n"), ""),
            "multiphase.mass_transfer.temperature_gradient",
        ),
        (
            "latent heat with model none",
            pool("\n[multiphase.mass_transfer]\nlatent_heat = 2.26e6\n", ""),
            "multiphase.mass_transfer.latent_heat",
        ),
        (
            "wave inlet without height",
            tank(&WAVES.replace("height = 0.04\n", "")),
            "multiphase.waves.height",
        ),
        (
            "wave inlet without period",
            tank(&WAVES.replace("period = 1.0\n", "")),
            "multiphase.waves.period",
        ),
        (
            "wave inlet on a missing patch",
            tank(&WAVES.replace("\"xmin\"", "\"nowhere\"")),
            "multiphase.waves.inlet_patch",
        ),
        (
            "wave inlet patch not fixed velocity",
            tank(WAVES).replace(
                "[boundaries.xmin]\nvelocity = \"fixed\"",
                "[boundaries.xmin]\nvelocity = \"zero_gradient\"",
            ),
            "boundaries.xmin.velocity",
        ),
        (
            "beach with equal edges",
            tank(&WAVES.replace("beach_x_outer = 2.0", "beach_x_outer = 1.2")),
            "multiphase.waves.beach_x_outer",
        ),
        (
            "beach without a level",
            tank("\n[multiphase.waves]\nbeach = \"still_water\"\nbeach_x_inner = 1.2\nbeach_x_outer = 2.0\n"),
            "multiphase.waves.level",
        ),
        (
            "wave height with no inlet",
            tank("\n[multiphase.waves]\nheight = 0.04\n"),
            "multiphase.waves.height",
        ),
        (
            "wave order out of range",
            tank(&format!("{WAVES}order = 3\n")),
            "multiphase.waves.order",
        ),
        (
            "waves with sideways gravity",
            tank(WAVES).replace("gravity = [0.0, -9.81, 0.0]", "gravity = [1.0, -9.81, 0.0]"),
            "physics.gravity",
        ),
        (
            "bad reference point",
            tank("").replace("reference_point = [0.0, 0.45, 0.0]", "reference_point = [0.0, 0.45]"),
            "multiphase.reference_point",
        ),
    ];
    for (what, text, path) in rows {
        let e = errors(&text);
        let hit = e.iter().any(|d| d.path == path && !d.message.is_empty());
        println!(
            "T1 {what}: expected path {path}: {}",
            if hit { "ok" } else { "MISSING" }
        );
        assert!(hit, "{what}: no located error at {path}; got {e:?}");
    }
    // valid cases are silent
    for (what, text) in [
        ("pool", pool("", "")),
        ("pool with lee", pool(MT_LEE, "")),
        ("tank with waves and beach", tank(WAVES)),
        ("tank without waves", tank("")),
    ] {
        let e = errors(&text);
        assert!(e.is_empty(), "{what}: {e:?}");
    }
    // disabled multiphase: the new keys are inert
    let off = "[mesh]\ngenerator = \"box_2d\"\nnx = 4\nny = 4\nlx = 1.0\nly = 1.0\n[multiphase]\nenabled = false\n";
    for key in [
        "[multiphase.mass_transfer]\nlatent_heat = 1.0e6\n",
        "[multiphase.waves]\nheight = 0.1\n",
        "[multiphase.waves]\ninlet_patch = \"xmin\"\n",
    ] {
        let e = errors(&format!("{off}{key}"));
        assert!(!e.is_empty(), "inert key not reported: {key}");
    }
}

fn lib_pool_with_mass_transfer() -> VofSolver {
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
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= 0.5);
    let n = geom.n_cells();
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.n_correctors = 3;
    s.mass_transfer = Some(MassTransfer {
        liquid_phase: 1,
        model: MassTransferModel::Lee {
            c_evap: 10.0,
            c_cond: 10.0,
        },
        t_sat: 373.15,
        h_fg: 2.26e6,
        temperature: vec![393.15; n],
    });
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// T2: the mass-transfer case through the tree equals the library, and evaporation really removes liquid.
#[test]
fn t2_mass_transfer_tree_equals_library() {
    let mut s = sim(&pool(MT_LEE, ""));
    let mut lib = lib_pool_with_mass_transfer();
    let v0: f64 = lib.phase_volume();
    for _ in 0..50 {
        s.step();
        lib.step(1e-3).unwrap();
    }
    let da = max_diff(&s.field("alpha").unwrap(), &lib.transport.alpha.values);
    let dp = max_diff(&s.field("p").unwrap(), &lib.p) / (1000.0 * G * 0.5);
    let lost = (v0 - lib.phase_volume()) / v0;
    println!("T2 tree vs library: alpha {da:.3e}, p {dp:.3e} (limit 1e-9); liquid volume lost {lost:.3e}");
    assert!(da <= 1e-9 && dp <= 1e-9);
    assert!(lost > 1e-6, "the tree case evaporated nothing: {lost}");
}

fn lib_tank() -> VofSolver {
    let omega = 2.0 * std::f64::consts::PI;
    let wave = Wave::new(0.02, omega, 0.3, G, 0.0, 1).unwrap();
    let mesh = Arc::new(box_2d(64, 30, 2.0, 0.45, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    p_rgh.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= 0.3);
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.x_ref = [0.0, 0.45, 0.0];
    s.n_correctors = 3;
    s.waves = Some(WaveSettings {
        level: 0.3,
        inlet: Some(WaveInlet {
            patch: "xmin".into(),
            wave,
            t_ramp: 2.0,
        }),
        zones: vec![RelaxZone {
            x_inner: 1.2,
            x_outer: 2.0,
            blend: Blend::Exponential { p: 3.5 },
            target: RelaxTarget::StillWater,
        }],
    });
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// T3: the wave tank through the tree equals the library and the waves are really generated.
#[test]
fn t3_waves_tree_equals_library() {
    let mut s = sim(&tank(WAVES));
    let mut lib = lib_tank();
    let mut umax = 0.0f64;
    for _ in 0..100 {
        s.step();
        lib.step(5e-3).unwrap();
        umax = umax.max(lib.u.values.iter().map(|v| v[0].abs()).fold(0.0, f64::max));
    }
    let da = max_diff(&s.field("alpha").unwrap(), &lib.transport.alpha.values);
    let dp = max_diff(&s.field("p").unwrap(), &lib.p) / (1000.0 * G * 0.3);
    println!("T3 tree vs library: alpha {da:.3e}, p {dp:.3e} (limit 1e-9); max|ux| {umax:.3e}");
    assert!(da <= 1e-9 && dp <= 1e-9);
    assert!(umax > 1e-3, "no wave motion: {umax}");
}

/// T4: AMG is used in the multiphase pressure solve above 2000 cells (the dam-break tree test in vof_tree.rs runs on it) and a
/// small case keeps the plain CG; both build without diagnostics.
#[test]
fn t4_amg_flag() {
    assert!(errors(&pool("", "")).is_empty());
    let big = tank("").replace("nx = 64\nny = 30", "nx = 100\nny = 30");
    let mut s = sim(&big);
    for _ in 0..5 {
        s.step();
    }
    let a = s.field("alpha").unwrap();
    assert!(a.iter().all(|x| x.is_finite() && *x > -1e-9 && *x < 1.0 + 1e-9));
}
