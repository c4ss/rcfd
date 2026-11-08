//! ADR 0225 (F-300), case-tree checks of the transient RANS follow-ups of F-294.
//!
//! Part A: T1 `rcfd_turb::supports_transient` against what the case tree accepts on `pimple` (with the precondition T1-0 and the
//! negative control T1N, main's answer), T1p the `piso` statement of its doc, S1c the corrected scope of ADR 0210 S1 (its cases
//! with `periodic_seam.diffusivity = "pair"` accepted, ADR 0212 R1'). Part B (ADR 0199 amendment 4): S3' the time order of
//! transient SST from the start that meets the walls, and its negative control S3'N. Part C: T3 the smooth wall eddy viscosity
//! (`wall_nut_law = spalding`) on the wall faces of a case-tree run, with T3N; S3s the time order of k-epsilon with it, and S3sN.
//! The limits are those registered in ADR 0225 before this code ran. The set-ups are copies of those of
//! `f286_transient_sst.rs`, `f294_transient_keps_sa.rs` and `rsm_tree.rs`.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use rcfd_turb::{
    create_model, KEpsilonVariant, MixingProfile, ModelContext, SLimiter, SaWallMode, TurbulenceModel, WallConstants,
    WallTreatment,
};
use std::sync::Arc;

/// Paths of the diagnostics of a case: parse errors, then static errors, then the errors of `Simulation::new`.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d: Vec<String> = c.semantic_errors().iter().map(|x| x.path.clone()).collect();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e.iter().map(|x| x.path.clone()).collect();
                }
            }
            d
        }
    }
}

fn messages(text: &str) -> Vec<(String, String)> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| (x.path.clone(), x.message.clone())).collect(),
        Ok(c) => c
            .semantic_errors()
            .iter()
            .map(|x| (x.path.clone(), x.message.clone()))
            .collect(),
    }
}

fn has(text: &str, path: &str) {
    let p = paths(text);
    assert!(p.iter().any(|x| x == path), "{path}: got {p:?}\n{text}");
}

fn ok(text: &str) {
    let p = paths(text);
    assert!(p.is_empty(), "expected no diagnostic, got {p:?}\n{text}");
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}\n{text}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"))
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

fn max_diff(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------------------------
// The set-ups
// ---------------------------------------------------------------------------------------------------------------

/// `physics.nu` of CH (1/395).
const NU_CH: f64 = 0.002_531_645_569_620_253;
const SST_LINES: &str = "model = \"k_omega_sst\"\nk_initial = 1.0\nomega_initial = 40.0\n";
const KE_LINES: &str = "model = \"k_epsilon\"\nk_initial = 1.0\nepsilon_initial = 3.6\n";
const SA_LINES: &str = "model = \"spalart_allmaras\"\nnutilde_ratio = 40.0\n";
const UNIFORM: &str = "velocity = [15.0, 0.0, 0.0]\n";
/// The start of ADR 0210 S3 and of S3' and S3s: zero on both walls, the same centre speed.
const PARABOLA: &str =
    "velocity = [0.0, 0.0, 0.0]\nvelocity_expression_x = \"15 [m/s] * (y / 1 [m]) * (2 - y / 1 [m])\"\n";
const DRIVE: &str = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";

/// CH of ADR 0199 with the `[turbulence]` lines `turb`, the `[initial]` lines, `extra` turbulence lines or tables and the
/// `[solver]` lines.
fn ch_with(turb: &str, initial: &str, extra: &str, solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 20
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = 0.0025316455696202532
rho = 1.0
[initial]
{initial}[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
{DRIVE}[turbulence]
{turb}{extra}[solver]
convection = "upwind"
{solver}"#
    )
}

/// The two models of ADR 0210 S1.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Model {
    KEpsilon,
    Sa,
}

impl Model {
    fn lines(self) -> &'static str {
        match self {
            Model::KEpsilon => KE_LINES,
            Model::Sa => SA_LINES,
        }
    }
}

/// CH-K / CH-A with the uniform start.
fn ch(model: Model, extra: &str, solver: &str) -> String {
    ch_with(model.lines(), UNIFORM, extra, solver)
}

fn pimple(scheme: &str) -> String {
    format!("algorithm = \"pimple\"\ntime_scheme = \"{scheme}\"\nn_outer = 3\ndt = 0.1\nend_time = 1.0\n")
}

const PISO: &str = "algorithm = \"piso\"\ndt = 0.1\nend_time = 1.0\n";
const SIMPLEC: &str = "algorithm = \"simplec\"\nmax_iterations = 10\n";

/// The S4 sector of ADR 0210 (amendment 1: `outer` a wall at rest in the rotating frame); `moving_outer` gives the ADR 0199
/// moving wall.
fn sector(model: Model, moving_outer: bool) -> String {
    let (inlet, turb) = match model {
        Model::KEpsilon => (
            "turbulence_spec = \"values\"\nk_value = 0.01\nepsilon_value = 0.009\n",
            "model = \"k_epsilon\"\n",
        ),
        Model::Sa => ("", "model = \"spalart_allmaras\"\n"),
    };
    let outer = if moving_outer {
        "velocity = \"moving_wall\"\nwall_rotation_rate = -2.0\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n"
    } else {
        "velocity = \"fixed\"\nturbulence = \"wall\"\n"
    };
    format!(
        r#"
[mesh]
generator = "annulus_3d_sector"
nx = 8
ny = 6
nz = 8
r_in = 0.5
r_out = 1.0
lz = 0.5
sector_angle = 40.0
[physics]
nu = 0.01
rho = 1.0
[initial]
velocity = [0.0, 0.0, 1.0]
[boundaries.zmin]
velocity = "fixed"
velocity_expression_x = "2 [1/s] * y + 0.2 [1/s] * x"
velocity_expression_y = "-2 [1/s] * x + 0.2 [1/s] * y"
velocity_expression_z = "1 [m/s] + 1 [1/s] * y"
{inlet}[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
{outer}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
[turbulence]
{turb}[solver]
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
algorithm = "pimple"
time_scheme = "bdf2"
n_outer = 3
n_correctors = 2
dt = 0.05
end_time = 100.0
"#
    )
}

/// RSMBOX: the doubly periodic decay box of `rsm_tree.rs` with the given `[solver]` lines.
fn rsm_box(solver: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 4
lx = 1.0
ly = 1.0
periodic_x = true
periodic_y = true
[physics]
nu = 0.001
[initial]
velocity = [0.0, 0.0, 0.0]
[solver]
convection = "upwind"
{solver}[turbulence]
model = "rsm"
k_initial = 1.0
epsilon_initial = 1.0
"#
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Part A: T1, T1N, T1p
// ---------------------------------------------------------------------------------------------------------------

const PLUGIN: &str = "f300_plugin";

fn register_plugin() {
    rcfd_turb::register_model(
        PLUGIN,
        "ADR 0225 T1: a test plug-in (the k-epsilon model behind a plug-in key)",
        Arc::new(|c| Ok(Box::new(rcfd_turb::model::KEpsilonModel::new(c)?) as Box<dyn TurbulenceModel>)),
    );
}

/// The base text of T1 for a registry key with the given `[solver]` lines.
fn base(key: &str, solver: &str) -> String {
    match key {
        "k_epsilon" => ch_with(KE_LINES, UNIFORM, "", solver),
        "k_omega_sst" => ch_with(SST_LINES, UNIFORM, "", solver),
        "spalart_allmaras" => ch_with(SA_LINES, UNIFORM, "", solver),
        "smagorinsky" => ch_with("model = \"smagorinsky\"\n", UNIFORM, "", solver),
        "wale" => ch_with("model = \"wale\"\n", UNIFORM, "", solver),
        "mixing_length" => ch_with("model = \"mixing_length\"\nlength_scale = 1.0\n", UNIFORM, "", solver),
        "rsm" => rsm_box(solver),
        other => ch_with(
            &format!("model = \"plugin\"\nplugin = \"{other}\"\n"),
            UNIFORM,
            "",
            solver,
        ),
    }
}

/// The reference lines of T1-0: a steady algorithm where the model has one, else `piso`.
fn reference(key: &str) -> &'static str {
    match key {
        "smagorinsky" | "wale" | "rsm" => PISO,
        _ => SIMPLEC,
    }
}

const DES: [&str; 2] = ["[turbulence.sa]\ndes = \"des\"\n", "[turbulence.sa]\ndes = \"ddes\"\n"];

/// The answer of `supports_transient` on main c201a770 (T1N).
fn main_answer(key: &str) -> bool {
    matches!(key, "smagorinsky" | "wale" | "mixing_length" | "rsm")
}

/// T1: `supports_transient` agrees with what the case tree accepts on pimple, for every key of the registry.
#[test]
fn t1_supports_transient_matches_the_case_tree_on_pimple() {
    register_plugin();
    let keys: Vec<String> = rcfd_turb::model_keys().into_iter().map(|k| k.0).collect();
    println!("T1: registry keys {keys:?}");
    assert!(
        keys.iter().any(|k| k == PLUGIN),
        "T1: the test plug-in is not registered"
    );
    // T1-0
    for key in &keys {
        let p = paths(&base(key, reference(key)));
        assert!(
            p.is_empty(),
            "T1-0 {key}: the base text with its reference lines gave {p:?}"
        );
    }
    println!("T1-0: every base text accepted with its reference lines");
    let pimple_euler = pimple("euler");
    let mut disagree_main = Vec::new();
    for key in &keys {
        let p = paths(&base(key, &pimple_euler));
        let accepted = p.is_empty();
        let answer = rcfd_turb::supports_transient(key);
        println!("T1 {key}: supports_transient {answer}; pimple diagnostics {p:?}");
        if answer {
            assert!(accepted, "T1 {key}: supports_transient is true but pimple gives {p:?}");
        } else {
            assert!(
                p.iter().any(|x| x == "turbulence.model"),
                "T1 {key}: supports_transient is false but pimple gives {p:?}"
            );
        }
        if main_answer(key) != accepted {
            disagree_main.push(key.clone());
        }
    }
    for des in DES {
        let t = ch_with(SA_LINES, UNIFORM, des, &pimple_euler);
        ok(&t);
    }
    assert!(rcfd_turb::supports_transient("spalart_allmaras"));
    println!(
        "T1: SA-DES and SA-DDES accepted on pimple (supports_transient(spalart_allmaras) true for every DES mode)"
    );
    // T1N: main's answer disagrees with the case tree for exactly the three models of F-286 and F-294
    disagree_main.sort();
    println!("T1N: main's answer disagrees with the case tree for {disagree_main:?}");
    assert_eq!(
        disagree_main,
        vec![
            "k_epsilon".to_string(),
            "k_omega_sst".to_string(),
            "spalart_allmaras".to_string()
        ],
        "T1N"
    );
}

/// T1p: the `piso` statement of the doc of `supports_transient`.
#[test]
fn t1p_the_piso_statement_of_the_doc() {
    register_plugin();
    for key in ["k_epsilon", "k_omega_sst", "spalart_allmaras", PLUGIN] {
        let p = paths(&base(key, PISO));
        println!("T1p {key} piso: {p:?}");
        assert!(p.iter().any(|x| x == "turbulence.model"), "T1p {key}: {p:?}");
    }
    for key in ["smagorinsky", "wale", "mixing_length", "rsm"] {
        let p = paths(&base(key, PISO));
        println!("T1p {key} piso: {p:?}");
        assert!(p.is_empty(), "T1p {key}: {p:?}");
    }
    for des in DES {
        let p = paths(&ch_with(SA_LINES, UNIFORM, des, PISO));
        println!("T1p SA {} piso: {p:?}", des.trim().replace('\n', " "));
        assert!(p.is_empty(), "T1p SA {des}: {p:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Part A: S1c
// ---------------------------------------------------------------------------------------------------------------

/// A model of the registry on a small channel, for the dispatch answers of S1c.
fn registry_model(key: &str) -> Box<dyn TurbulenceModel> {
    let mesh = box_2d(4, 4, 1.0, 1.0, true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let ctx = ModelContext {
        geom,
        nu: 0.01,
        walls: vec!["ymin".into(), "ymax".into()],
        k0: 1.0,
        eps0: 1.0,
        omega0: 10.0,
        nutilde0: 0.03,
        variant: KEpsilonVariant::Standard,
        wall_treatment: WallTreatment::Smooth,
        sa_wall_mode: SaWallMode::Auto,
        sa_limiter: SLimiter::Allmaras,
        sa_negative: true,
        kappa: 0.41,
        e_wall: 9.8,
        convection: rcfd_fv::convection::ConvScheme::Upwind,
        relax_turb: 0.7,
        relax_nut: 0.5,
        wall_damping: false,
        length_scale: 0.0,
        mixing_profile: MixingProfile::Linear,
        time_accurate: true,
    };
    create_model(key, &ctx).unwrap()
}

/// S1c: the scope of ADR 0210 S1 (with its amendment 1) after F-296: `pair` is accepted with both models on pimple.
#[test]
fn s1c_scope_of_transient_kepsilon_and_sa_after_f296() {
    use Model::{KEpsilon as K, Sa as A};
    for m in [K, A] {
        for scheme in ["euler", "bdf2", "bdf2_bounded"] {
            ok(&ch(m, "", &pimple(scheme)));
        }
    }
    for extra in [
        "variant = \"rng\"\n",
        "variant = \"realizable\"\n",
        "wall_function = \"scalable\"\n",
        "wall_function = \"enhanced\"\n",
        "wall_treatment = \"hysteresis\"\n",
    ] {
        ok(&ch(K, extra, &pimple("euler")));
    }
    for extra in [
        "[turbulence.sa]\nwall_mode = \"resolved\"\n",
        "[turbulence.sa]\nwall_mode = \"auto_sa_law\"\n",
        "[turbulence.sa]\nlimiter = \"min_0p3\"\n",
        "[turbulence.sa]\nnegative_nutilde = false\n",
        "[turbulence.sa]\ndes = \"ddes\"\n",
    ] {
        ok(&ch(A, extra, &pimple("euler")));
    }
    ok(&ch(A, "[turbulence.sa]\ndes = \"ddes\"\n", PISO));
    ok(&sector(K, false));
    ok(&sector(A, false));
    // the correction (ADR 0212 R1'): pair is accepted with both models on pimple
    for m in [K, A] {
        ok(&ch(
            m,
            "[turbulence.periodic_seam]\ndiffusivity = \"pair\"\n",
            &pimple("euler"),
        ));
    }
    println!("S1c: accepted: CH-K and CH-A on pimple with euler, bdf2, bdf2_bounded; k-epsilon rng, realizable, scalable, enhanced, hysteresis; SA resolved, auto_sa_law, min_0p3, negative_nutilde off, ddes on pimple and piso; the two S4 sectors; CH-K and CH-A with pair on pimple (ADR 0212 R1')");
    for m in [K, A] {
        let piso = ch(m, "", PISO);
        has(&piso, "turbulence.model");
        let msgs = messages(&piso);
        let (_, text) = msgs
            .iter()
            .find(|(p, _)| p == "turbulence.model")
            .expect("piso message");
        assert!(
            text.contains("pimple") && text.contains("steady"),
            "{m:?}: the piso refusal does not name pimple and steady: {text}"
        );
        has(
            &format!("{}[energy]\nenabled = true\n", ch(m, "", &pimple("euler"))),
            "turbulence.model",
        );
        has(
            &format!(
                "{}[dynamic_mesh]\nenabled = true\nmotion = \"prescribed\"\nmoving_patches = [\"ymin\"]\n",
                ch(m, "", &pimple("euler"))
            ),
            "dynamic_mesh.enabled",
        );
        has(
            &ch(m, "", &format!("{}error_tolerance = 1e-3\n", pimple("euler"))),
            "solver.error_tolerance",
        );
        let fractional = ch(
            m,
            "",
            &format!("{}time_advance = \"fractional_step\"\n", pimple("euler")),
        );
        assert!(fractional.contains(DRIVE), "the CH text changed");
        has(&fractional.replace(DRIVE, ""), "solver.time_advance");
    }
    // ADR 0210 amendment 1: the moving wall of ADR 0199's sector stays refused for k-epsilon
    has(&sector(K, true), "boundaries.outer.velocity");
    println!("S1c: located at turbulence.model (piso, naming pimple and steady; energy), dynamic_mesh.enabled, solver.error_tolerance, solver.time_advance (pimple with the fractional step, no drive), boundaries.outer.velocity (the moving wall)");
    // the dispatch of ADR 0210 D3
    assert!(registry_model("k_epsilon").updates_in_outer_iterations());
    let mut sa = registry_model("spalart_allmaras");
    assert!(sa.updates_in_outer_iterations());
    for (des, want) in [("des", false), ("ddes", false), ("off", true)] {
        sa.set_option("des", des).unwrap();
        assert_eq!(sa.updates_in_outer_iterations(), want, "SA des = {des}");
    }
    println!("S1c: updates_in_outer_iterations: k_epsilon true; spalart_allmaras off true, des false, ddes false");
}

// ---------------------------------------------------------------------------------------------------------------
// The time-order harness (S3', S3s)
// ---------------------------------------------------------------------------------------------------------------

const S3_T: f64 = 2.0;

/// The models of the time-order checks.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Order {
    /// SST (S3', ADR 0199 amendment 4).
    Sst,
    /// k-epsilon with `wall_nut_law = spalding` (S3s).
    KeSpalding,
}

impl Order {
    fn lines(self) -> &'static str {
        match self {
            Order::Sst => SST_LINES,
            Order::KeSpalding => KE_LINES,
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Order::Sst => &["ux", "k", "omega"],
            Order::KeSpalding => &["ux", "k", "epsilon"],
        }
    }
}

/// The S3 solver lines of ADR 0199 with `n` steps to T.
fn s3_solver(scheme: &str, n_outer: usize, n: usize) -> String {
    let dt = S3_T / n as f64;
    format!(
        "algorithm = \"pimple\"
time_scheme = \"{scheme}\"
n_outer = {n_outer}
n_correctors = 2
momentum_rel_tol = 1e-12
pressure_rel_tol = 1e-12
pressure_max_iter = 1000
dt = {dt:?}
end_time = {S3_T:?}
"
    )
}

/// Set `wall_nut_law = spalding` on the model of a k-epsilon run (ADR 0225 D5), before its first step.
fn set_spalding(sim: &mut Simulation) {
    sim.turbulence_model_mut()
        .expect("a turbulence model")
        .set_option("wall_nut_law", "spalding")
        .expect("wall_nut_law = spalding");
}

/// One run of CH from PARABOLA to T with `n` steps: the fields at T, the largest `nut_change` of a step and the range of the
/// wall-cell y* over the steps (k-epsilon; `(inf, 0)` for SST).
fn s3_run(m: Order, scheme: &str, n_outer: usize, n: usize) -> (Vec<Vec<f64>>, f64, (f64, f64)) {
    let mut sim = sim_of(&ch_with(m.lines(), PARABOLA, "", &s3_solver(scheme, n_outer, n)));
    if m == Order::KeSpalding {
        set_spalding(&mut sim);
    }
    let mut worst = 0.0_f64;
    let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
        assert!(
            sim.failure().is_none(),
            "{m:?} {scheme} n_outer {n_outer} n {n} step {steps}: {:?}",
            sim.failure()
        );
        let c = sim.status_json()["turbulence"]["nut_change"]
            .as_f64()
            .unwrap_or(f64::NAN);
        worst = worst.max(c);
        if m == Order::KeSpalding {
            for v in sim.field("y_plus").unwrap().into_iter().filter(|&v| v > 0.0) {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    assert_eq!(steps, n, "{m:?}: {steps} steps instead of {n}");
    assert!((sim.time() - S3_T).abs() < 1e-12, "{m:?}: ended at {}", sim.time());
    (
        m.fields().iter().map(|f| sim.field(f).unwrap()).collect(),
        worst,
        (lo, hi),
    )
}

/// The observed orders of the fields from the runs at T/80, T/160, T/320.
fn s3_orders(m: Order, scheme: &str, n_outer: usize) -> Vec<f64> {
    let ns = [80, 160, 320];
    let runs: Vec<_> = ns.iter().map(|&n| s3_run(m, scheme, n_outer, n)).collect();
    let mut p = Vec::new();
    for (i, name) in m.fields().iter().enumerate() {
        let scale = max_abs(&runs[2].0[i]);
        let e1 = max_diff(&runs[0].0[i], &runs[1].0[i]) / scale;
        let e2 = max_diff(&runs[1].0[i], &runs[2].0[i]) / scale;
        let pi = (e1 / e2).log2();
        println!(
            "{m:?} {scheme} n_outer {n_outer} {name}: e(80,160) {e1:.4e}, e(160,320) {e2:.4e}, p {pi:.4}; largest nut_change of a step {:.3e} / {:.3e} / {:.3e}",
            runs[0].1, runs[1].1, runs[2].1
        );
        p.push(pi);
    }
    if m == Order::KeSpalding {
        let (lo, hi) = runs[2].2;
        println!(
            "{m:?} {scheme} n_outer {n_outer}: wall-cell y* of the T/320 run in [{lo:.4}, {hi:.4}] (y*_lam 11.5301)"
        );
    }
    p
}

/// A time-order check: every field's p in [lo, hi].
fn order_check(m: Order, scheme: &str, lo: f64, hi: f64) {
    let p = s3_orders(m, scheme, 20);
    for (name, pi) in m.fields().iter().zip(p) {
        assert!(
            pi >= lo && pi <= hi,
            "{m:?} {scheme} {name}: p {pi:.4} outside [{lo}, {hi}]"
        );
    }
}

/// A negative control: with one sweep per step BDF2 loses its order in at least one field.
fn negative_control(m: Order) {
    let p = s3_orders(m, "bdf2", 1);
    let least = p.iter().cloned().fold(f64::INFINITY, f64::min);
    println!("{m:?} negative control: smallest p {least:.4} (must be <= 1.3)");
    assert!(least <= 1.3, "{m:?}: the negative control did not fire: {p:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Part B: S3', S3'N (ADR 0199 amendment 4)
// ---------------------------------------------------------------------------------------------------------------

/// S3', BDF2: transient SST from the start that meets the walls is second order in ux, k and omega.
#[test]
fn s3p_sst_bdf2_time_order_from_the_parabola() {
    order_check(Order::Sst, "bdf2", 1.7, 2.3)
}

/// S3', Euler: first order.
#[test]
fn s3p_sst_euler_time_order_from_the_parabola() {
    order_check(Order::Sst, "euler", 0.8, 1.2)
}

/// S3'N: with one sweep per step BDF2 loses its order (S3' can fail).
#[test]
fn s3pn_sst_one_sweep_per_step_from_the_parabola() {
    negative_control(Order::Sst)
}

// ---------------------------------------------------------------------------------------------------------------
// Part C: T3, T3N, S3s, S3sN
// ---------------------------------------------------------------------------------------------------------------

/// The wall faces (`ymin`, `ymax`) of the T3 run after 5 steps: (solver face viscosity, the expected value of the law).
fn t3_run(spalding: bool) -> (Vec<f64>, Vec<f64>) {
    let mut sim = sim_of(&ch_with(KE_LINES, PARABOLA, "", &s3_solver("bdf2", 20, 80)));
    if spalding {
        set_spalding(&mut sim);
    }
    for step in 1..=5 {
        sim.step();
        assert!(sim.failure().is_none(), "T3 step {step}: {:?}", sim.failure());
    }
    let nuf = sim.momentum_face_viscosity().expect("the solver's face viscosity");
    let k = sim.field("k").unwrap();
    let wc = WallConstants::default();
    let g = &sim.geom;
    let own = g.mesh.owner();
    let (mut got, mut want) = (Vec::new(), Vec::new());
    for patch in ["ymin", "ymax"] {
        for f in g.mesh.patch(patch).unwrap().faces() {
            let y = g.dn[f - g.n_internal];
            let kc = k[own[f]];
            let nut = if spalding {
                wc.nut_wall_from_k_spalding(NU_CH, kc, y)
            } else {
                wc.nut_wall_from_k(NU_CH, kc, y)
            };
            got.push(nuf[f]);
            want.push(NU_CH + nut);
        }
    }
    (got, want)
}

/// T3 and T3N: the option reaches the wall faces of a case-tree run, and the refusals of the option.
#[test]
fn t3_spalding_wall_nut_reaches_the_wall_faces() {
    let (got_s, want_s) = t3_run(true);
    let off_s = got_s
        .iter()
        .zip(&want_s)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    println!(
        "T3: {} wall faces, {off_s} off the bits of nu + nut_wall_from_k_spalding(k); face viscosity / nu in [{:.6}, {:.6}]",
        got_s.len(),
        got_s.iter().cloned().fold(f64::INFINITY, f64::min) / NU_CH,
        got_s.iter().cloned().fold(0.0, f64::max) / NU_CH
    );
    assert!(!got_s.is_empty());
    assert_eq!(off_s, 0, "T3: {got_s:?} against {want_s:?}");
    let (got_n, want_n) = t3_run(false);
    let off_n = got_n
        .iter()
        .zip(&want_n)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    let diff = got_s
        .iter()
        .zip(&got_n)
        .map(|(a, b)| (a - b).abs() / b)
        .fold(0.0, f64::max);
    println!(
        "T3N: {off_n} of {} wall faces off the bits of nu + nut_wall_from_k(k); largest relative difference of the two runs {diff:.3e} (must be >= 1e-3)",
        got_n.len()
    );
    assert_eq!(off_n, 0, "T3N: {got_n:?} against {want_n:?}");
    assert!(diff >= 1e-3, "T3N: {diff:e}");
    // refusals
    let mut sim = sim_of(&ch_with(KE_LINES, PARABOLA, "", &s3_solver("bdf2", 20, 80)));
    let e = sim
        .turbulence_model_mut()
        .unwrap()
        .set_option("wall_nut_law", "kader")
        .unwrap_err();
    println!("T3: wall_nut_law = kader: {e}");
    let mut sim = sim_of(&ch_with(
        KE_LINES,
        PARABOLA,
        "wall_function = \"scalable\"\n",
        &s3_solver("bdf2", 20, 80),
    ));
    let e = sim
        .turbulence_model_mut()
        .unwrap()
        .set_option("wall_nut_law", "spalding")
        .unwrap_err();
    println!("T3: wall_nut_law = spalding with the scalable wall function: {e}");
}

/// S3s, BDF2: k-epsilon with the smooth wall eddy viscosity (the test of the suspected cause of the ADR 0210 S3 known miss).
/// Known miss (ADR 0225 addendum): the run of record gave p 0.8031 (ux), 1.4814 (k), 1.4287 (epsilon), below the band and
/// within 0.1 of the standard law's 0.7975 / 1.4910 / 1.5303; the outer iterations had converged (largest nut_change of a step
/// 1.8e-8, 2.9e-12, 5.9e-16 at T/80, T/160, T/320). So the kink of the standard wall eddy viscosity is not shown to be the cause.
#[test]
#[ignore = "known miss (ADR 0225 addendum): k-epsilon with the Spalding wall eddy viscosity, BDF2 p 0.8031 / 1.4814 / 1.4287 below [1.7, 2.3]"]
fn s3s_kepsilon_spalding_bdf2_time_order_known_miss() {
    order_check(Order::KeSpalding, "bdf2", 1.7, 2.3)
}

/// The observed orders of S3s BDF2 at finer levels (report only, the diagnosis of the S3s known miss, ADR 0225 addendum): the
/// same runs as the ADR 0210 diagnosis of the standard law (T/160, T/320, T/640 and T/320, T/640, T/1280).
#[test]
#[ignore = "diagnosis of the S3s known miss, report only (ADR 0225 addendum)"]
fn s3s_kepsilon_spalding_bdf2_finer_levels_diagnosis() {
    for ns in [[160, 320, 640], [320, 640, 1280]] {
        let runs: Vec<_> = ns.iter().map(|&n| s3_run(Order::KeSpalding, "bdf2", 20, n)).collect();
        for (i, name) in Order::KeSpalding.fields().iter().enumerate() {
            let scale = max_abs(&runs[2].0[i]);
            let e1 = max_diff(&runs[0].0[i], &runs[1].0[i]) / scale;
            let e2 = max_diff(&runs[1].0[i], &runs[2].0[i]) / scale;
            println!(
                "S3s diagnosis {name}: e({},{}) {e1:.4e}, e({},{}) {e2:.4e}, p {:.4}; largest nut_change of a step {:.3e} / {:.3e} / {:.3e}",
                ns[0],
                ns[1],
                ns[1],
                ns[2],
                (e1 / e2).log2(),
                runs[0].1,
                runs[1].1,
                runs[2].1
            );
        }
    }
}

/// S3s, Euler: first order.
#[test]
fn s3s_kepsilon_spalding_euler_time_order() {
    order_check(Order::KeSpalding, "euler", 0.8, 1.2)
}

/// S3sN: with one sweep per step BDF2 loses its order (S3s can fail).
#[test]
fn s3sn_kepsilon_spalding_one_sweep_per_step() {
    negative_control(Order::KeSpalding)
}
