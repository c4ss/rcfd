//! F-246 (ADR 0193): the pressure-interpolation variants of the steady kernel in the case tree (`solver.pressure_interpolation`).
//! Limits are those registered in docs/adr/0193-f246-registration-pressure-interpolation-variants.md before this file was written.
//!
//! T0: the skewed `wedge_2d` mesh W has skewed and non-orthogonal faces. T1: located errors (R1 to R4). T2: the tree run equals the
//! library run (`rcfd_fv` only) with the same variant to 1e-12. T3: every variant moves the tree result away from the default run
//! by more than 1e-9, so T2 cannot pass unwired. T4 (i): the key written at its default changes neither the settings hash nor a bit
//! of the result. The variants are one table (`VARIANTS`), as F-256's option sets are (ADR 0190).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pressure_interp::PressureInterpolation;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use std::sync::Arc;

/// T2 limit (the acceptance's 1e-12).
const TREE_LIBRARY: f64 = 1e-12;
/// T3 limit: the smallest change of `u_x`, `u_y` or `p` a variant must make against the default run.
const TEETH: f64 = 1e-9;
/// Iterations of C1 and C2.
const ITERS: usize = 60;
/// `physics.nu` and `physics.body_force` of C1 and C2.
const NU: f64 = 0.1;
const BODY: [f64; 3] = [0.0, -10.0, 0.0];

/// The variants of ADR 0193: name, the `[solver]` line, the library value.
const VARIANTS: [(&str, &str, PressureInterpolation); 2] = [
    (
        "V1",
        "pressure_interpolation = \"skew_corrected\"\n",
        PressureInterpolation::SkewCorrected,
    ),
    (
        "V2",
        "pressure_interpolation = \"body_force_weighted\"\n",
        PressureInterpolation::BodyForceWeighted,
    ),
];

/// The key written at its default (T4 (i)).
const DEFAULT_KEY: &str = "pressure_interpolation = \"linear\"\n";

/// The lid cavity on mesh W: `correctors` non-orthogonal correctors, the body force when `body`, extra `[solver]` lines,
/// extra tables after the solver section.
fn cavity_text(correctors: usize, body: bool, solver: &str, tables: &str) -> String {
    let body = if body {
        format!("body_force = [{:.1}, {:.1}, {:.1}]\n", BODY[0], BODY[1], BODY[2])
    } else {
        String::new()
    };
    format!(
        r#"
title = "f246 cavity"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
nu = {NU:.1}
{body}[boundaries.xmin]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
convection = "linear_upwind"
non_orthogonal_correctors = {correctors}
{solver}{tables}"#
    )
}

/// The steady run lines of C1 and C2.
fn steady(extra: &str) -> String {
    format!("algorithm = \"simplec\"\nmax_iterations = {ITERS}\ntolerance_u = 1e-14\ntolerance_p = 1e-14\n{extra}")
}

/// A buoyant cavity on mesh W (energy with Boussinesq buoyancy and gravity, no `physics.body_force`) with extra `[solver]` lines.
fn buoyant_text(solver: &str) -> String {
    format!(
        r#"
title = "f246 buoyant"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
nu = 0.1
rho = 1.0
cp = 1.0
conductivity = 0.1
thermal_expansion = 1.0
reference_temperature = 0.5
gravity = [0.0, -10.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.xmax]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = {ITERS}
{solver}"#
    )
}

/// A prescribed-flow energy case on mesh W (ADR 0190's E1) with extra `[solver]` lines.
fn frozen_text(solver: &str) -> String {
    format!(
        r#"
title = "f246 frozen flow"
[mesh]
generator = "wedge_2d"
nx = 16
ny = 16
lx = 1.0
ly = 1.0
ramp_start = 0.0
ramp_angle = 30.0
[physics]
rho = 1.0
cp = 1.0
conductivity = 0.1
body_force = [0.0, -10.0, 0.0]
[energy]
enabled = true
[initial]
temperature = 0.5
[boundaries.xmin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[boundaries.ymin]
velocity = "fixed"
temperature = "fixed"
temperature_value = 0.0
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
frozen_flow = true
max_iterations = {ITERS}
{solver}"#
    )
}

fn case(text: &str) -> Case {
    Case::parse_toml(text).unwrap_or_else(|d| panic!("case does not parse: {d:?}"))
}

/// The fields `u_x`, `u_y` and `p` after a steady run of the case to its end.
fn tree(text: &str) -> (Simulation, [Vec<f64>; 3]) {
    let mut s = Simulation::new(&case(text)).unwrap_or_else(|d| panic!("not built: {d:?}"));
    let n = s.run(10 * ITERS);
    assert!(s.failure().is_none(), "{:?}", s.failure());
    assert_eq!(n, ITERS, "iterations of the tree run");
    let f = ["ux", "uy", "p"].map(|name| s.field(name).unwrap());
    (s, f)
}

fn max_diff(a: &[Vec<f64>; 3], b: &[Vec<f64>; 3]) -> f64 {
    (0..3)
        .flat_map(|k| {
            assert_eq!(a[k].len(), b[k].len());
            a[k].iter().zip(&b[k]).map(|(x, y)| (x - y).abs())
        })
        .fold(0.0, f64::max)
}

/// The library run of C1 or C2: a `SimpleSolver` on the tree's geometry, set up as the tree sets it, the variant on its field.
fn library(geom: &Arc<FvGeometry>, correctors: usize, v: PressureInterpolation) -> [Vec<f64>; 3] {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    for (patch, value) in [
        ("xmin", [0.0; 3]),
        ("ymin", [0.0; 3]),
        ("xmax", [0.0; 3]),
        ("ymax", [1.0, 0.0, 0.0]),
    ] {
        u.set_bc(geom, patch, Box::new(FixedValue::vector(value))).unwrap();
    }
    let p = ScalarField::uniform("p", geom, 0.0);
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.alpha_u = 0.9;
    s.alpha_p = 1.0;
    s.non_orth_correctors = correctors;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    sol.body_force = Some(BodyForce::uniform(geom, BODY));
    sol.pressure_interpolation = v;
    for _ in 0..ITERS {
        sol.iterate();
    }
    [
        sol.u.values.iter().map(|x| x[0]).collect(),
        sol.u.values.iter().map(|x| x[1]).collect(),
        sol.p.values.clone(),
    ]
}

/// Largest `|x_f - x_f'| / |d|` over the internal faces, `x_f' = w x_P + (1 - w) x_N`.
fn skewness(geom: &FvGeometry) -> f64 {
    let mesh = &geom.mesh;
    let (own, nei, cc, cf) = (mesh.owner(), mesh.neighbour(), mesh.cell_centres(), mesh.face_centres());
    (0..geom.n_internal)
        .map(|f| {
            let w = geom.weights[f];
            let xp = vec3::add(vec3::scale(cc[own[f]], w), vec3::scale(cc[nei[f]], 1.0 - w));
            vec3::norm(vec3::sub(cf[f], xp)) / vec3::norm(geom.delta[f])
        })
        .fold(0.0, f64::max)
}

/// T0: the precondition on mesh W.
fn precondition(s: &Simulation) {
    let q = rcfd_mesh::quality::check_mesh(&s.geom.mesh);
    let sk = skewness(&s.geom);
    println!(
        "T0: mesh W max non-orthogonality {:.3} degrees (at least 20), largest |x_f - x_f'| / |d| {sk:.3e} (at least 1e-6)",
        q.max_non_orthogonality
    );
    assert!(
        q.max_non_orthogonality >= 20.0,
        "T0 set-up: {}",
        q.max_non_orthogonality
    );
    assert!(sk >= 1e-6, "T0 set-up: skewness {sk}");
}

/// T2 and T3 for C1 (0 correctors) or C2 (1).
fn t2_t3(correctors: usize) {
    let label = if correctors == 0 { "C1" } else { "C2" };
    let text = |extra: &str| cavity_text(correctors, true, &steady(extra), "");
    let (s0, default) = tree(&text(""));
    precondition(&s0);
    let lib0 = library(&s0.geom, correctors, PressureInterpolation::Linear);
    let d0 = max_diff(&default, &lib0);
    println!("T2 {label} control: default tree vs library Linear {d0:.3e} (limit {TREE_LIBRARY:e})");
    assert!(d0 <= TREE_LIBRARY, "T2 {label} control {d0}");
    for (name, keys, v) in VARIANTS {
        let (s, f) = tree(&text(keys));
        let lib = library(&s.geom, correctors, v);
        let (d, teeth) = (max_diff(&f, &lib), max_diff(&f, &default));
        println!(
            "T2 {label} {name}: tree vs library {d:.3e} (limit {TREE_LIBRARY:e}); T3 change against the default {teeth:.3e} (limit > {TEETH:e})"
        );
        assert!(d <= TREE_LIBRARY, "T2 {label} {name}: {d}");
        assert!(teeth > TEETH, "T3 {label} {name}: {teeth}");
    }
}

#[test]
fn t2_t3_c1_without_correctors() {
    t2_t3(0);
}

#[test]
fn t2_t3_c2_with_a_corrector() {
    t2_t3(1);
}

/// T4 (i): the key written at its default leaves the settings hash and every bit of the result unchanged.
#[test]
fn t4_i_key_at_its_default_changes_nothing() {
    for correctors in [0, 1] {
        let (a, b) = (
            cavity_text(correctors, true, &steady(""), ""),
            cavity_text(correctors, true, &steady(DEFAULT_KEY), ""),
        );
        let (ha, hb) = (case(&a).tree.content_hash(), case(&b).tree.content_hash());
        assert_eq!(ha, hb, "T4 (i) correctors {correctors}: settings hash");
        let (_, fa) = tree(&a);
        let (_, fb) = tree(&b);
        let differ: usize = (0..3)
            .map(|k| {
                fa[k]
                    .iter()
                    .zip(&fb[k])
                    .filter(|(x, y)| x.to_bits() != y.to_bits())
                    .count()
            })
            .sum();
        println!(
            "T4 (i) correctors {correctors}: settings hash {ha:016x} both; {differ} of {} values differ in a bit (limit 0)",
            3 * fa[0].len()
        );
        assert_eq!(differ, 0, "T4 (i) correctors {correctors}");
    }
}

/// The path of the first diagnostic of a refused case (parse or build).
fn refused_at(text: &str) -> String {
    match Case::parse_toml(text) {
        Err(d) => d.first().map(|x| x.path.clone()).unwrap_or_default(),
        Ok(c) => match Simulation::new(&c) {
            Err(d) => {
                println!("  {}: {}", d[0].path, d[0].message);
                d[0].path.clone()
            }
            Ok(_) => "accepted".into(),
        },
    }
}

#[test]
fn t1_located_errors() {
    const KEY: &str = "solver.pressure_interpolation";
    let (v1, v2) = (VARIANTS[0].1, VARIANTS[1].1);
    let porous =
        "[sources.bed]\ntype = \"porous\"\nmodel = \"darcy_forchheimer\"\npermeability = 1.0\nzone = \"all\"\n";
    let accel = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";
    let pimple = format!("algorithm = \"pimple\"\ndt = 0.01\nend_time = 0.05\n{v1}");
    let coupled = format!("algorithm = \"coupled\"\nmax_iterations = {ITERS}\n{v1}");
    let checks: Vec<(&str, String, &str)> = vec![
        // R1: a value the schema refuses
        (
            "R1 unknown variant",
            cavity_text(0, true, &steady("pressure_interpolation = \"momentum_weighted\"\n"), ""),
            KEY,
        ),
        // R2: no kernel takes it (no physics.body_force here: piso and pimple refuse it first)
        ("R2 pimple", cavity_text(0, false, &pimple, ""), KEY),
        ("R2 coupled", cavity_text(0, false, &coupled, ""), KEY),
        ("R2 frozen flow", frozen_text(v1), KEY),
        // R3: nothing to weight
        ("R3 no body force", cavity_text(0, false, &steady(v2), ""), KEY),
        ("R3 porous source only", cavity_text(0, false, &steady(v2), porous), KEY),
        // R4: the trapezoid wall treatment of an acceleration source
        ("R4 acceleration source", cavity_text(0, true, &steady(v2), accel), KEY),
        // controls: accepted
        (
            "control V1 without a body force",
            cavity_text(0, false, &steady(v1), ""),
            "accepted",
        ),
        (
            "control V2 with physics.body_force",
            cavity_text(0, true, &steady(v2), ""),
            "accepted",
        ),
        ("control V2 buoyant", buoyant_text(v2), "accepted"),
        (
            "control pimple without the key",
            cavity_text(0, false, "algorithm = \"pimple\"\ndt = 0.01\nend_time = 0.05\n", ""),
            "accepted",
        ),
    ];
    for (what, text, want) in checks {
        let got = refused_at(&text);
        println!("T1 {what}: {got} (want {want})");
        assert_eq!(got, want, "T1 {what}");
    }
}
