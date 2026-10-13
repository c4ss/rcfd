//! Turbulence and compressible physics through the case schema (settings tree):
//! parity with the library API, validation diagnostics and the settings round trip.

use rcfd_api::case::Case;
use rcfd_api::schema::case_schema;
use rcfd_api::sim::Simulation;
use rcfd_core::settings::validate_json_schema;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, Gas, Prim};
use rcfd_fv::riemann::ExactRiemann;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, box_2d_graded, Grading};
use rcfd_turb::{KEpsilon, KEpsilonVariant};
use serde_json::json;
use std::sync::Arc;

const RE_TAU: f64 = 395.0;

fn channel_toml(variant: &str, iterations: usize) -> String {
    format!(
        r#"
title = "k-epsilon channel"
[mesh]
generator = "box_2d"
nx = 4
ny = 14
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = {nu}
body_force = [1.0, 0.0, 0.0]
[initial]
velocity = [17.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
alpha_u = 0.9
max_iterations = {iterations}
tolerance_u = 1e-14
tolerance_p = 1e-14
[turbulence]
model = "k_epsilon"
variant = "{variant}"
k_initial = 1.0
epsilon_initial = 0.36
[reports.yplus_wall]
type = "y_plus"
patch = "ymin"
statistic = "average"
[reports.yplus_max]
type = "y_plus"
patch = "ymax"
statistic = "max"
"#,
        nu = 1.0 / RE_TAU
    )
}

/// The library-API run of `rcfd-turb/tests/channel_kepsilon.rs`, stopped after `iterations`.
fn library_channel(variant: KEpsilonVariant, iterations: usize) -> (Vec<[f64; 3]>, Vec<f64>, Vec<f64>) {
    let nu = 1.0 / RE_TAU;
    let mesh = box_2d_graded(4, 14, 1.0, 2.0, [Grading::Uniform, Grading::Uniform], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [17.0, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    let mut model = KEpsilon::new(variant, geom.clone(), nu, &["ymin", "ymax"], 1.0, 0.36).unwrap();
    for _ in 0..iterations {
        model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        flow.iterate();
    }
    (flow.u.values.clone(), model.k.values.clone(), model.nut.clone())
}

#[test]
fn kepsilon_channel_from_the_settings_tree_equals_the_library_run() {
    for (name, variant) in [
        ("standard", KEpsilonVariant::Standard),
        ("rng", KEpsilonVariant::Rng),
        ("realizable", KEpsilonVariant::Realizable),
    ] {
        let n = 60;
        let case = Case::parse_toml(&channel_toml(name, n)).unwrap();
        let mut sim = Simulation::new(&case).unwrap();
        assert_eq!(sim.run(1000), n);
        let (u, k, nut) = library_channel(variant, n);
        let max_diff = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        let ux: Vec<f64> = u.iter().map(|v| v[0]).collect();
        assert!(max_diff(&sim.field("ux").unwrap(), &ux) < 1e-12, "{name}: ux");
        assert!(max_diff(&sim.field("k").unwrap(), &k) < 1e-12, "{name}: k");
        assert!(max_diff(&sim.field("nut").unwrap(), &nut) < 1e-12, "{name}: nut");
        assert!(sim.field("epsilon").unwrap().iter().all(|e| *e > 0.0));
        assert!(sim.monitors.get("res_nut").is_some());
    }
}

#[test]
fn kepsilon_channel_converges_and_reports_y_plus() {
    let case = Case::parse_toml(&channel_toml("standard", 8000)).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(8000);
    let nut_change = sim.monitors.last("res_nut").unwrap();
    assert!(nut_change < 1e-6, "eddy viscosity not converged: {nut_change}");
    // wall-function y* of the first cell: Cmu^(1/4) sqrt(k) y / nu with y = h/14 and k ~ u_tau^2 / sqrt(Cmu)
    let yp = sim.monitors.last("yplus_wall").unwrap();
    assert!((20.0..40.0).contains(&yp), "average y+ {yp}");
    let ym = sim.monitors.last("yplus_max").unwrap();
    assert!(ym >= yp * 0.5 && ym < 60.0, "max y+ {ym}");
    // bulk velocity within the 10 % acceptance band of the DNS bulk velocity U_b+ = 17.4 (rcfd-turb channel test)
    let ux = sim.field("ux").unwrap();
    let bulk = ux.iter().sum::<f64>() / ux.len() as f64;
    println!("k-epsilon channel via the tree: bulk {bulk:.4}, y+ {yp:.3}, nut change {nut_change:.2e}");
    assert!((bulk / 17.4 - 1.0).abs() < 0.10, "bulk velocity {bulk}");
    let st = sim.status_json();
    assert_eq!(st["turbulence"]["model"], "k_epsilon");
    let mut wp: Vec<String> = serde_json::from_value(st["turbulence"]["wall_patches"].clone()).unwrap();
    wp.sort();
    assert_eq!(wp, ["ymax", "ymin"]);
    // a y+ field exists on the wall cells only
    let ypc = sim.field("y_plus").unwrap();
    assert_eq!(ypc.iter().filter(|v| **v > 0.0).count(), 8, "two rows of 4 wall cells");
    assert!(sim.field("wall_distance").unwrap().iter().all(|d| *d > 0.0));
}

const INLET_CHANNEL: &str = r#"
title = "channel inlet"
[mesh]
generator = "channel_2d"
nx = 16
ny = 10
lx = 2.0
ly = 0.2
depth = 0.1
[physics]
nu = 1e-4
[boundaries.inlet]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
turbulence_spec = "intensity_length"
turbulence_intensity = 0.05
turbulence_length = 0.01
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 30
[turbulence]
model = "k_epsilon"
[reports.k_in]
type = "area_average"
patch = "inlet"
field = "k"
[reports.eps_in]
type = "area_average"
patch = "inlet"
field = "epsilon"
"#;

#[test]
fn inlet_turbulence_helpers_set_k_and_epsilon() {
    let case = Case::parse_toml(INLET_CHANNEL).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(30);
    let k = 1.5 * (0.05f64).powi(2);
    let eps = 0.09f64.powf(0.75) * k.powf(1.5) / 0.01;
    let got_k = sim.monitors.last("k_in").unwrap();
    let got_e = sim.monitors.last("eps_in").unwrap();
    assert!((got_k / k - 1.0).abs() < 1e-12, "{got_k} vs {k}");
    assert!((got_e / eps - 1.0).abs() < 1e-12, "{got_e} vs {eps}");
    // viscosity-ratio form: nut = Cmu k^2 / eps = ratio * nu at the inlet
    let mut c2 = case.clone();
    c2.set("boundaries.inlet.turbulence_spec", json!("intensity_viscosity_ratio"))
        .unwrap();
    c2.set("boundaries.inlet.turbulence_viscosity_ratio", json!(25.0))
        .unwrap();
    let mut sim2 = Simulation::new(&c2).unwrap();
    sim2.run(5);
    let (k2, e2) = (
        sim2.monitors.last("k_in").unwrap(),
        sim2.monitors.last("eps_in").unwrap(),
    );
    assert!((0.09 * k2 * k2 / e2 / 1e-4 - 25.0).abs() < 1e-9);
    // explicit values
    let mut c3 = case.clone();
    c3.set("boundaries.inlet.turbulence_spec", json!("values")).unwrap();
    c3.set("boundaries.inlet.k_value", json!(0.02)).unwrap();
    c3.set("boundaries.inlet.epsilon_value", json!(0.5)).unwrap();
    let mut sim3 = Simulation::new(&c3).unwrap();
    sim3.run(2);
    assert!((sim3.monitors.last("k_in").unwrap() - 0.02).abs() < 1e-15);
    assert!((sim3.monitors.last("eps_in").unwrap() - 0.5).abs() < 1e-15);
}

fn diag_paths(text: &str) -> Vec<(String, Option<usize>, String)> {
    Case::parse_toml(text)
        .unwrap_err()
        .into_iter()
        .map(|d| (d.path, d.line, d.message))
        .collect()
}

#[test]
fn turbulence_validation_errors_carry_paths_and_lines() {
    // missing length scale on a k-epsilon inlet
    let text = INLET_CHANNEL.replace("turbulence_length = 0.01\n", "");
    let d = diag_paths(&text);
    let e = d.iter().find(|e| e.0 == "boundaries.inlet.turbulence_length").unwrap();
    assert!(e.2.contains("turbulence_length"), "{}", e.2);
    assert!(e.1.is_some(), "line expected: {d:?}");
    // transient algorithm
    let text = INLET_CHANNEL.replace("algorithm = \"simplec\"", "algorithm = \"piso\"");
    let d = diag_paths(&text);
    let e = d.iter().find(|e| e.0 == "turbulence.model").unwrap();
    assert!(e.2.contains("steady"), "{}", e.2);
    let want = text.lines().position(|l| l == "model = \"k_epsilon\"").unwrap() + 1;
    assert_eq!(e.1, Some(want), "{d:?}");
    // no wall
    let text = INLET_CHANNEL.replace("[boundaries.walls]\nvelocity = \"fixed\"\n", "");
    assert!(diag_paths(&text).iter().any(|e| e.2.contains("wall patch")));
    // a turbulence role needs the model
    let text = INLET_CHANNEL.replace("model = \"k_epsilon\"", "model = \"laminar\"") + "\n";
    let d = diag_paths(&(text + "[boundaries.extra]\nturbulence = \"wall\"\n"));
    assert!(d.iter().any(|e| e.0 == "boundaries.extra.turbulence"), "{d:?}");
    // initial k cannot be derived without any velocity
    let text = "[mesh]\ngenerator = \"cavity_2d\"\n[boundaries.walls]\nvelocity = \"fixed\"\n[turbulence]\nmodel = \"k_epsilon\"\n";
    assert!(diag_paths(text).iter().any(|e| e.0 == "turbulence.k_initial"));
    // energy and turbulence
    let text = INLET_CHANNEL.to_string() + "[energy]\nenabled = true\n";
    assert!(diag_paths(&text).iter().any(|e| e.2.contains("energy")));
    // y_plus report on a non-wall patch
    let text = INLET_CHANNEL.to_string() + "[reports.bad]\ntype = \"y_plus\"\npatch = \"outlet\"\n";
    assert!(diag_paths(&text).iter().any(|e| e.0 == "reports.bad.patch"));
}

// ---------------------------------------------------------------- compressible

fn sod_toml(n: usize, extra_compressible: &str) -> String {
    format!(
        r#"
title = "Sod"
[mesh]
generator = "box_2d"
nx = {n}
ny = 1
lx = 1.0
ly = {ly}
[solver]
algorithm = "compressible"
end_time = 0.2
[compressible]
flux = "hllc"
reconstruction = "tvd_van_leer"
rk_order = 3
cfl = 0.4
gamma = 1.4
{extra_compressible}
[compressible.initial]
mode = "riemann"
split_axis = "x"
split_position = 0.5
[compressible.initial.left]
density = 1.0
pressure = 1.0
[compressible.initial.right]
density = 0.125
pressure = 0.1
[boundaries.xmin]
compressible_bc = "fixed"
density_value = 1.0
pressure_value = 1.0
[boundaries.xmax]
compressible_bc = "fixed"
density_value = 0.125
pressure_value = 0.1
[reports.mass_in]
type = "volume_integral"
field = "rho"
"#,
        ly = 1.0 / n as f64
    )
}

#[test]
fn sod_from_the_settings_tree_equals_the_library_run_and_the_exact_solution() {
    let n = 100;
    let case = Case::parse_toml(&sod_toml(n, "")).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(100000);
    assert!(sim.is_done() && !sim.is_steady());
    assert!((sim.time() - 0.2).abs() < 1e-12, "t = {}", sim.time());
    // the same run through the library API
    let geom = FvGeometry::new(Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap())).unwrap();
    let (l, r) = (
        Prim {
            rho: 1.0,
            u: [0.0; 3],
            p: 1.0,
        },
        Prim {
            rho: 0.125,
            u: [0.0; 3],
            p: 0.1,
        },
    );
    let mut lib = CompressibleSolver::new(
        geom,
        Gas::AIR,
        CompSettings::default(),
        |x| if x[0] < 0.5 { l } else { r },
    )
    .unwrap();
    lib.set_bc("xmin", CompBc::Fixed(l)).unwrap();
    lib.set_bc("xmax", CompBc::Fixed(r)).unwrap();
    lib.run_to(0.2).unwrap();
    let rho = sim.field("rho").unwrap();
    let p = sim.field("p").unwrap();
    let prim = lib.primitives();
    for c in 0..n {
        assert!((rho[c] - prim[c].rho).abs() < 1e-13, "rho {c}");
        assert!((p[c] - prim[c].p).abs() < 1e-13, "p {c}");
    }
    assert_eq!(sim.iteration(), lib.steps);
    // density L1 error against the exact Riemann solution
    let ex = ExactRiemann::new(Gas::AIR, [1.0, 0.0, 1.0], [0.125, 0.0, 0.1]).unwrap();
    let cc = sim.geom.mesh.cell_centres().to_vec();
    let l1: f64 = (0..n)
        .map(|c| (rho[c] - ex.sample((cc[c][0] - 0.5) / 0.2)[0]).abs())
        .sum::<f64>()
        / n as f64;
    println!("Sod n={n} via the settings tree: density L1 = {l1:.4e}");
    assert!(l1 < 8e-3, "density L1 {l1}");
    // mach and temperature fields exist; the density integral is the conserved mass
    assert!(sim.field("mach").unwrap().iter().all(|m| *m >= 0.0));
    assert!(sim.field("T").unwrap().iter().all(|t| *t > 0.0));
    let st = sim.status_json();
    assert_eq!(st["algorithm"], "compressible");
}

#[test]
fn compressible_flux_and_reconstruction_options_are_applied() {
    let run = |extra: &str| {
        let mut sim = Simulation::new(&Case::parse_toml(&sod_toml(60, extra)).unwrap()).unwrap();
        sim.run(100000);
        sim.field("rho").unwrap()
    };
    let hllc = run("");
    let rus = run("");
    assert_eq!(hllc, rus, "deterministic");
    // later keys of the TOML table override the earlier defaults: flux is set above, so edit the tree instead
    let mut case = Case::parse_toml(&sod_toml(60, "")).unwrap();
    case.set("compressible.flux", json!("rusanov")).unwrap();
    case.set("compressible.reconstruction", json!("first_order")).unwrap();
    case.set("compressible.rk_order", json!(2)).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(100000);
    let diffusive = sim.field("rho").unwrap();
    let diff: f64 = hllc.iter().zip(&diffusive).map(|(a, b)| (a - b).abs()).sum();
    assert!(diff > 1e-3, "first-order Rusanov must differ from MUSCL HLLC ({diff})");
}

#[test]
fn compressible_validation_rejects_unsupported_combinations() {
    let base = sod_toml(20, "");
    let find = |text: &str, path: &str| -> String {
        let d = diag_paths(text);
        d.iter()
            .find(|e| e.0 == path)
            .unwrap_or_else(|| panic!("no diagnostic for {path}: {d:?}"))
            .2
            .clone()
    };
    // forward Euler with a limited reconstruction
    let t = base.replace("rk_order = 3", "rk_order = 1");
    assert!(find(&t, "compressible.rk_order").contains("forward Euler"));
    // turbulence, energy, pseudo-transient, periodic mesh
    assert!(find(
        &(base.clone() + "[turbulence]\nmodel = \"k_epsilon\"\n"),
        "turbulence.model"
    )
    .contains("compressible"));
    assert!(find(&(base.clone() + "[energy]\nenabled = true\n"), "energy.enabled").contains("Boussinesq"));
    let t = base.replace("end_time = 0.2", "end_time = 0.2\npseudo_mode = \"local\"");
    assert!(find(&t, "solver.pseudo_mode").contains("SIMPLE"));
    let t = base.replace("ly = 0.05", "ly = 0.05\nperiodic_x = true");
    assert!(find(&t, "mesh.periodic_x").contains("periodic"));
    // incompressible reports
    let t = base.clone() + "[reports.f]\ntype = \"force\"\npatch = \"xmin\"\n";
    assert!(find(&t, "reports.f.type").contains("not available"));
    // a fixed state without pressure
    let t = base.replace("pressure_value = 1.0", "pressure_value = 0.0");
    assert!(find(&t, "boundaries.xmin.pressure_value").contains("positive"));
    // non-positive initial state, with a line number
    let t = base.replace(
        "[compressible.initial.right]\ndensity = 0.125",
        "[compressible.initial.right]\ndensity = 0.0",
    );
    let d = diag_paths(&t);
    let e = d.iter().find(|e| e.0 == "compressible.initial.right.density").unwrap();
    assert!(e.1.is_some(), "{d:?}");
    // compressible_bc outside the compressible solver
    let t = "[boundaries.lid]\ncompressible_bc = \"slip\"\n";
    assert!(find(t, "boundaries.lid.compressible_bc").contains("compressible"));
    // unknown flux
    let t = base.replace("flux = \"hllc\"", "flux = \"godunov\"");
    assert!(find(&t, "compressible.flux").contains("not one of"));
}

#[test]
fn wedge_generator_and_fixed_inflow_run_from_the_settings_tree() {
    let text = r#"
title = "wedge"
[mesh]
generator = "wedge_2d"
nx = 30
ny = 10
lx = 1.5
ly = 1.0
ramp_start = 0.2
ramp_angle = 10.0
[solver]
algorithm = "compressible"
end_time = 1.0
[compressible]
flux = "hllc"
[compressible.initial.left]
density = 1.0
velocity = [2.0, 0.0, 0.0]
pressure = 0.7142857142857143
[boundaries.xmin]
compressible_bc = "fixed"
velocity_value = [2.0, 0.0, 0.0]
density_value = 1.0
pressure_value = 0.7142857142857143
[boundaries.xmax]
compressible_bc = "extrapolate"
[boundaries.ymax]
compressible_bc = "extrapolate"
[boundaries.ymin]
compressible_bc = "slip"
[reports.inflow]
type = "mass_flow"
patch = "xmin"
[reports.outflow]
type = "mass_flow"
patch = "xmax"
[reports.pmax]
type = "maximum"
field = "p"
"#;
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.run(100000);
    assert!(sim.is_done());
    // the ramp turns the flow upward, so the ymin patch has points above y = 0
    let cc = sim.geom.mesh.cell_centres();
    assert!(cc.iter().any(|c| c[0] > 1.4 && c[1] > 0.2), "ramp geometry");
    // free-stream inflow carries rho u A = 1 * 2 * (1.0 * depth) of mass inwards (negative outward flux)
    let area = rcfd_post::integrals::patch_area(&sim.geom, "xmin").unwrap();
    let m_in = sim.monitors.last("inflow").unwrap();
    assert!(
        (m_in + 2.0 * area).abs() < 1e-9 * area,
        "inflow {m_in} vs {}",
        2.0 * area
    );
    // the compression raises the pressure above the free stream
    let pmax = sim.monitors.last("pmax").unwrap();
    assert!(pmax > 0.7142857142857143 * 1.05, "pmax {pmax}");
    assert!(sim.monitors.last("res_rho").unwrap().is_finite());
}

// ---------------------------------------------------------------- round trip

#[test]
fn new_settings_round_trip_through_the_tree_toml_and_json_schema() {
    let mut c = Case::new();
    let sets: Vec<(&str, serde_json::Value)> = vec![
        ("mesh.generator", json!("wedge_2d")),
        ("mesh.ramp_start", json!(0.3)),
        ("mesh.ramp_angle", json!(12.0)),
        ("physics.body_force", json!([1.0, 0.0, 0.0])),
        ("solver.h_operator", json!("previous_iterate")),
        ("solver.p_ref_cell", json!(3)),
        ("solver.p_ref_value", json!(0.5)),
        ("solver.bulk_velocity", json!(2.0)),
        ("solver.bulk_direction", json!([0.0, 1.0, 0.0])),
        ("solver.momentum_rel_tol", json!(1e-2)),
        ("solver.momentum_max_iter", json!(50)),
        ("solver.pressure_rel_tol", json!(1e-3)),
        ("solver.pressure_max_iter", json!(500)),
        ("solver.ddt_correction", json!(false)),
        ("turbulence.model", json!("k_epsilon")),
        ("turbulence.variant", json!("realizable")),
        ("turbulence.wall_function", json!("standard")),
        ("turbulence.kappa", json!(0.4)),
        ("turbulence.e_wall", json!(9.0)),
        ("turbulence.convection", json!("tvd_van_leer")),
        ("turbulence.relax", json!(0.6)),
        ("turbulence.relax_nut", json!(0.4)),
        ("turbulence.k_initial", json!(0.1)),
        ("turbulence.epsilon_initial", json!(0.2)),
        ("turbulence.initial_intensity", json!(0.03)),
        ("turbulence.initial_viscosity_ratio", json!(20.0)),
        ("boundaries.in.turbulence", json!("inlet")),
        ("boundaries.in.turbulence_spec", json!("intensity_viscosity_ratio")),
        ("boundaries.in.turbulence_intensity", json!(0.02)),
        ("boundaries.in.turbulence_length", json!(0.01)),
        ("boundaries.in.turbulence_viscosity_ratio", json!(15.0)),
        ("boundaries.in.k_value", json!(0.3)),
        ("boundaries.in.epsilon_value", json!(0.4)),
        ("boundaries.in.compressible_bc", json!("fixed")),
        ("boundaries.in.density_value", json!(1.2)),
        ("compressible.flux", json!("ausm_plus")),
        ("compressible.reconstruction", json!("tvd_koren")),
        ("compressible.rk_order", json!(2)),
        ("compressible.cfl", json!(0.6)),
        ("compressible.gamma", json!(1.3)),
        ("compressible.gas_constant", json!(300.0)),
        ("compressible.mode", json!("steady")),
        ("compressible.steady_tolerance", json!(1e-7)),
        ("compressible.initial.mode", json!("riemann")),
        ("compressible.initial.split_axis", json!("y")),
        ("compressible.initial.split_position", json!(0.25)),
        ("compressible.initial.left.density", json!(2.0)),
        ("compressible.initial.left.velocity", json!([1.0, 2.0, 0.0])),
        ("compressible.initial.left.pressure", json!(3.0)),
        ("compressible.initial.right.density", json!(0.5)),
        ("compressible.initial.right.pressure", json!(0.4)),
        ("reports.yp.type", json!("y_plus")),
        ("reports.yp.statistic", json!("max")),
        ("solver.algorithm", json!("compressible")),
    ];
    for (p, v) in &sets {
        c.set(p, v.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    for (p, v) in &sets {
        assert_eq!(&c.get(p).unwrap(), v, "{p}");
    }
    // TOML and JSON round trips preserve the whole tree
    let again = Case::parse_toml(&c.to_toml());
    // the combination is deliberately inconsistent (compressible + k-epsilon): the round trip must still
    // reproduce the tree through the unvalidated JSON path
    assert!(again.is_err());
    let back = rcfd_core::settings::SettingsTree::from_json(case_schema(), &c.tree.to_json()).unwrap();
    assert_eq!(back.to_json(), c.tree.to_json());
    // a consistent case round trips through TOML and the exported JSON schema validates the tree
    let mut ok = Case::new();
    for (p, v) in sets.iter().filter(|(p, _)| {
        !p.starts_with("turbulence.") && !p.starts_with("reports.") && !p.starts_with("boundaries.in.turbulence")
    }) {
        if *p == "physics.body_force" || *p == "solver.bulk_velocity" {
            continue;
        }
        ok.set(p, v.clone()).unwrap();
    }
    ok.set("boundaries.in.compressible_bc", json!("fixed")).unwrap();
    ok.set("boundaries.in.pressure_value", json!(1.0)).unwrap();
    let parsed = Case::parse_toml(&ok.to_toml()).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(parsed.tree.to_json(), ok.tree.to_json());
    let js = parsed.tree.schema().to_json_schema();
    validate_json_schema(&js, &parsed.tree.to_json()).unwrap();
    for key in ["turbulence", "compressible"] {
        assert!(
            js["properties"][key]["properties"].is_object(),
            "{key} in the JSON schema"
        );
    }
    assert_eq!(
        js["properties"]["solver"]["properties"]["algorithm"]["enum"]
            .as_array()
            .unwrap()
            .last()
            .unwrap(),
        "compressible"
    );
}

#[test]
fn body_force_bulk_velocity_and_solver_controls_work_in_a_laminar_channel() {
    // laminar plane Poiseuille flow driven by a body force; bulk velocity driver reaches its target
    let text = |extra: &str| {
        format!(
            r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 12
lx = 1.0
ly = 1.0
periodic_x = true
[physics]
nu = 0.1
{extra}
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-6
pressure_rel_tol = 1e-6
pressure_max_iter = 1000
max_iterations = 3000
tolerance_u = 1e-10
tolerance_p = 1e-10
"#
        )
    };
    // body force G = 1.2 gives U_bulk = G h^2 / (12 nu) = 1.0 for h = 1
    let mut sim = Simulation::new(&Case::parse_toml(&text("body_force = [1.2, 0.0, 0.0]")).unwrap()).unwrap();
    sim.run(3000);
    let ux = sim.field("ux").unwrap();
    let bulk = ux.iter().sum::<f64>() / ux.len() as f64;
    // 12 cells: wall half-cell discretisation error of the parabolic profile (about 1.4 %)
    assert!((bulk - 1.0).abs() < 0.02, "bulk {bulk}");
    // the driver finds the same flow without being told the force
    let driven = text("").replace("max_iterations = 3000", "bulk_velocity = 1.0\nmax_iterations = 3000")
        + "[initial]\nvelocity = [0.5, 0.0, 0.0]\n";
    let mut sim = Simulation::new(&Case::parse_toml(&driven).unwrap()).unwrap();
    sim.run(3000);
    let ux = sim.field("ux").unwrap();
    let bulk = ux.iter().sum::<f64>() / ux.len() as f64;
    assert!((bulk - 1.0).abs() < 1e-6, "driven bulk {bulk}");
    // transient algorithms reject the steady-only drivers
    let t = text("body_force = [1.0, 0.0, 0.0]").replace("algorithm = \"simplec\"", "algorithm = \"piso\"");
    assert!(diag_paths(&t).iter().any(|e| e.0 == "physics.body_force"));
    // pinned pressure cell outside the mesh
    let t = text("").replace("max_iterations = 3000", "p_ref_cell = 9999\nmax_iterations = 3000");
    assert!(Simulation::new(&Case::parse_toml(&t).unwrap())
        .err()
        .unwrap()
        .iter()
        .any(|d| d.path == "solver.p_ref_cell"));
}

#[test]
fn res_t_is_an_accepted_convergence_monitor() {
    let t = "[energy]\nenabled = true\n[convergence.c]\nmonitor = \"res_T\"\ntype = \"below\"\ntolerance = 1e-8\n";
    Case::parse_toml(t).unwrap();
}
