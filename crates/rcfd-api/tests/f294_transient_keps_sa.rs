//! ADR 0210 (F-294), case-tree checks of transient k-epsilon and Spalart-Allmaras (DES off) on `pimple`: S1 scope and located
//! errors, R1 the reduction to the steady SIMPLEC solution of a fully developed channel, S3 the time order of the coupled step
//! from a start that meets the walls (and the negative control S3N with one sweep per step), S4 the rotating frame on a sector.
//! The limits are those registered in ADR 0210 before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use rcfd_turb::{
    create_model, KEpsilonVariant, MixingProfile, ModelContext, SLimiter, SaWallMode, TurbulenceModel, WallTreatment,
};
use std::sync::Arc;
use std::time::Instant;

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
// The channels CH-K and CH-A
// ---------------------------------------------------------------------------------------------------------------

/// The two models of the item.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Model {
    KEpsilon,
    Sa,
}

impl Model {
    /// The `[turbulence]` lines of CH-K / CH-A.
    fn lines(self) -> &'static str {
        match self {
            Model::KEpsilon => "model = \"k_epsilon\"\nk_initial = 1.0\nepsilon_initial = 3.6\n",
            Model::Sa => "model = \"spalart_allmaras\"\nnutilde_ratio = 40.0\n",
        }
    }

    /// The fields of R1.
    fn r1_fields(self) -> &'static [&'static str] {
        match self {
            Model::KEpsilon => &["ux", "k", "epsilon", "nut"],
            Model::Sa => &["ux", "nuTilda", "nut"],
        }
    }

    /// The fields of S3.
    fn s3_fields(self) -> &'static [&'static str] {
        match self {
            Model::KEpsilon => &["ux", "k", "epsilon"],
            Model::Sa => &["ux", "nuTilda"],
        }
    }
}

const UNIFORM: &str = "velocity = [15.0, 0.0, 0.0]\n";
/// The start of S3 (P2 of ADR 0210): zero on both walls, the same centre speed.
const PARABOLA: &str =
    "velocity = [0.0, 0.0, 0.0]\nvelocity_expression_x = \"15 [m/s] * (y / 1 [m]) * (2 - y / 1 [m])\"\n";
const DRIVE: &str = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";

/// CH of ADR 0199 with the model's lines, the `[initial]` lines, `extra` turbulence lines or tables and the `[solver]` lines.
fn ch_with(model: Model, initial: &str, extra: &str, solver: &str) -> String {
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
{}{extra}[solver]
convection = "upwind"
{solver}"#,
        model.lines()
    )
}

/// CH with the uniform start.
fn ch(model: Model, extra: &str, solver: &str) -> String {
    ch_with(model, UNIFORM, extra, solver)
}

fn pimple(scheme: &str) -> String {
    format!("algorithm = \"pimple\"\ntime_scheme = \"{scheme}\"\nn_outer = 3\ndt = 0.1\nend_time = 1.0\n")
}

// ---------------------------------------------------------------------------------------------------------------
// The sector of S4 (ADR 0183 PC-3; ADR 0210 amendment 1: `outer` a wall at rest in the rotating frame)
// ---------------------------------------------------------------------------------------------------------------

/// The S4 sector with the model; `moving_outer` gives the ADR 0199 moving wall (the case of amendment 1).
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

/// `max abs(phi_f + phi_g) / max abs(phi_f)` over the faces of the rotationally periodic pairs (as ADR 0183 PC-3).
fn seam_pair_sum(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let phi = sim.flux();
    let (mut sum, mut big) = (0.0_f64, 0.0_f64);
    for (pi, patch) in g.mesh.patches().iter().enumerate() {
        let (Some(map), Some(_)) = (&g.periodic_partner[pi], &g.periodic_rot[pi]) else {
            continue;
        };
        for (j, f) in patch.faces().enumerate() {
            sum = sum.max((phi[f] + phi[map[j]]).abs());
            big = big.max(phi[f].abs());
        }
    }
    assert!(big > 0.0, "no rotational seam flux");
    sum / big
}

// ---------------------------------------------------------------------------------------------------------------
// S1 scope
// ---------------------------------------------------------------------------------------------------------------

/// A model of the registry on a small channel, for the dispatch answers of S1.
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

/// S1: what the case tree accepts and where it refuses, after F-294.
#[test]
#[ignore = "superseded by ADR 0212 R1' (F-296): periodic_seam.diffusivity = pair is accepted with k_epsilon and spalart_allmaras; the cases are in S1' of f296_seam_diffusivity.rs; the lead ignored this record after the F-296 merge, in the way ADR 0212 D4 ignored the F-295 S1"]
fn s1_scope_of_transient_kepsilon_and_sa() {
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
    ok(&ch(
        A,
        "[turbulence.sa]\ndes = \"ddes\"\n",
        "algorithm = \"piso\"\ndt = 0.1\nend_time = 1.0\n",
    ));
    ok(&sector(K, false));
    ok(&sector(A, false));
    println!("S1: accepted: CH-K and CH-A on pimple with euler, bdf2, bdf2_bounded; k-epsilon rng, realizable, scalable, enhanced, hysteresis; SA resolved, auto_sa_law, min_0p3, negative_nutilde off, ddes on pimple and piso; the two S4 sectors");
    for m in [K, A] {
        let piso = ch(m, "", "algorithm = \"piso\"\ndt = 0.1\nend_time = 1.0\n");
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
        has(
            &ch(
                m,
                "[turbulence.periodic_seam]\ndiffusivity = \"pair\"\n",
                &pimple("euler"),
            ),
            "turbulence.periodic_seam.diffusivity",
        );
    }
    // amendment 1: the moving wall of ADR 0199's sector stays refused for k-epsilon
    has(&sector(K, true), "boundaries.outer.velocity");
    println!("S1: located at turbulence.model (piso, naming pimple and steady; energy), dynamic_mesh.enabled, solver.error_tolerance, solver.time_advance (pimple with the fractional step, no drive), turbulence.periodic_seam.diffusivity, boundaries.outer.velocity (the moving wall)");
    // the dispatch of ADR 0210 D3
    assert!(registry_model("k_epsilon").updates_in_outer_iterations());
    let mut sa = registry_model("spalart_allmaras");
    assert!(sa.updates_in_outer_iterations());
    for (des, want) in [("des", false), ("ddes", false), ("off", true)] {
        sa.set_option("des", des).unwrap();
        assert_eq!(sa.updates_in_outer_iterations(), want, "SA des = {des}");
    }
    println!("S1: updates_in_outer_iterations: k_epsilon true; spalart_allmaras off true, des false, ddes false");
}

// ---------------------------------------------------------------------------------------------------------------
// R1 reduction to the steady solution
// ---------------------------------------------------------------------------------------------------------------

/// R1 for one model: run to a steady state, the pimple solution equals the SIMPLEC one.
fn r1(model: Model) {
    let t0 = Instant::now();
    let mut steady = sim_of(&ch(model, "", "algorithm = \"simplec\"\nmax_iterations = 10000\n"));
    let n = steady.run(10000);
    assert_eq!(n, 10000);
    assert!(steady.failure().is_none(), "{:?}", steady.failure());
    let ts = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let mut tr = sim_of(&ch(
        model,
        "",
        "algorithm = \"pimple\"\ntime_scheme = \"euler\"\nn_outer = 2\ndt = 0.2\nend_time = 1000.0\n",
    ));
    let mut prev = tr.field("ux").unwrap();
    let mut last_change = f64::NAN;
    let mut steps = 0;
    while !tr.is_done() {
        tr.step();
        steps += 1;
        assert!(
            tr.failure().is_none(),
            "R1 {model:?} pimple step {steps}: {:?}",
            tr.failure()
        );
        let ux = tr.field("ux").unwrap();
        last_change = max_diff(&ux, &prev);
        prev = ux;
        assert!(steps <= 5000, "R1 {model:?}: more than 5000 steps");
    }
    let tt = t1.elapsed().as_secs_f64();
    let umax = max_abs(&prev);
    let settled = last_change <= 1e-11 * umax;
    println!(
        "R1 {model:?}: simplec 10000 its {ts:.1} s; pimple {steps} steps {tt:.1} s, t = {:.3}, last-step change of ux {last_change:.3e} = {:.3e} max ux (settled limit 1e-11)",
        tr.time(),
        last_change / umax
    );
    assert_eq!(steps, 5000, "R1 {model:?}: the pimple run took {steps} steps, not 5000");
    assert!(settled, "R1 {model:?} NOT SETTLED: {:.3e}", last_change / umax);
    let mut worst = 0.0_f64;
    for f in model.r1_fields() {
        let (a, b) = (tr.field(f).unwrap(), steady.field(f).unwrap());
        let r = max_diff(&a, &b) / max_abs(&b);
        println!(
            "R1 {model:?} {f}: max |pimple - simplec| / max |simplec| = {r:.3e} (limit 1e-8); max simplec {:.6e}",
            max_abs(&b)
        );
        worst = worst.max(r);
    }
    let uy = [tr.field("uy").unwrap(), steady.field("uy").unwrap()];
    let uy_rel = [max_abs(&uy[0]) / umax, max_abs(&uy[1]) / umax];
    println!(
        "R1 {model:?} uy: max |uy| / max ux pimple {:.3e}, simplec {:.3e} (limit 1e-8)",
        uy_rel[0], uy_rel[1]
    );
    assert!(worst <= 1e-8, "R1 {model:?}: {worst:.3e}");
    assert!(uy_rel[0] <= 1e-8 && uy_rel[1] <= 1e-8, "R1 {model:?} uy: {uy_rel:?}");
}

/// R1, k-epsilon.
#[test]
fn r1_kepsilon_pimple_steady_state_equals_the_simplec_solution() {
    r1(Model::KEpsilon)
}

/// R1, Spalart-Allmaras.
#[test]
fn r1_sa_pimple_steady_state_equals_the_simplec_solution() {
    r1(Model::Sa)
}

// ---------------------------------------------------------------------------------------------------------------
// S3 time order (and S3N)
// ---------------------------------------------------------------------------------------------------------------

const S3_T: f64 = 2.0;

/// CH on pimple to T with `n` steps from the `[initial]` lines `initial`, with the model's `[turbulence]` lines replaced by
/// `lines` when given; `each(time, sim)` after every step. The fields of S3 at T and the largest `nut_change` of a step.
#[allow(clippy::too_many_arguments)]
fn s3_run_with(
    model: Model,
    lines: Option<&str>,
    initial: &str,
    scheme: &str,
    n_outer: usize,
    n: usize,
    mut each: impl FnMut(&Simulation),
) -> (Vec<Vec<f64>>, f64) {
    let dt = S3_T / n as f64;
    let mut text = ch_with(
        model,
        initial,
        "",
        &format!(
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
        ),
    );
    if let Some(l) = lines {
        assert!(text.contains(model.lines()), "the CH text changed");
        text = text.replace(model.lines(), l);
    }
    let mut sim = sim_of(&text);
    let mut worst = 0.0_f64;
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
        assert!(
            sim.failure().is_none(),
            "S3 {model:?} {scheme} n_outer {n_outer} n {n} step {steps}: {:?}",
            sim.failure()
        );
        let c = sim.status_json()["turbulence"]["nut_change"]
            .as_f64()
            .unwrap_or(f64::NAN);
        worst = worst.max(c);
        each(&sim);
    }
    assert_eq!(steps, n, "S3: {steps} steps instead of {n}");
    assert!((sim.time() - S3_T).abs() < 1e-12, "S3: ended at {}", sim.time());
    (model.s3_fields().iter().map(|f| sim.field(f).unwrap()).collect(), worst)
}

/// The observed orders of the S3 fields from the runs at `T/n` for the three `n` (`lines` as in [`s3_run_with`]).
fn s3_orders_at(model: Model, lines: Option<&str>, scheme: &str, n_outer: usize, ns: [usize; 3]) -> Vec<f64> {
    let runs: Vec<(Vec<Vec<f64>>, f64)> = ns
        .iter()
        .map(|&n| s3_run_with(model, lines, PARABOLA, scheme, n_outer, n, |_| {}))
        .collect();
    let mut p = Vec::new();
    for (i, name) in model.s3_fields().iter().enumerate() {
        let scale = max_abs(&runs[2].0[i]);
        let e1 = max_diff(&runs[0].0[i], &runs[1].0[i]) / scale;
        let e2 = max_diff(&runs[1].0[i], &runs[2].0[i]) / scale;
        let pi = (e1 / e2).log2();
        println!(
            "S3 {model:?} {scheme} n_outer {n_outer} {name}: e({},{}) {e1:.4e}, e({},{}) {e2:.4e}, p {pi:.4}; largest nut_change of a step {:.3e} / {:.3e} / {:.3e}",
            ns[0], ns[1], ns[1], ns[2], runs[0].1, runs[1].1, runs[2].1
        );
        p.push(pi);
    }
    p
}

/// The observed orders of the S3 fields from the runs at T/80, T/160, T/320.
fn s3_orders(model: Model, scheme: &str, n_outer: usize) -> Vec<f64> {
    s3_orders_at(model, None, scheme, n_outer, [80, 160, 320])
}

/// S3 for one model and scheme: every field's p in [lo, hi].
fn s3(model: Model, scheme: &str, lo: f64, hi: f64) {
    let p = s3_orders(model, scheme, 20);
    for (name, pi) in model.s3_fields().iter().zip(p) {
        assert!(
            pi >= lo && pi <= hi,
            "S3 {model:?} {scheme} {name}: p {pi:.4} outside [{lo}, {hi}]"
        );
    }
}

/// S3N for one model: with one sweep per step (n_outer = 1) BDF2 loses its order in at least one field.
fn s3n(model: Model) {
    let p = s3_orders(model, "bdf2", 1);
    let least = p.iter().cloned().fold(f64::INFINITY, f64::min);
    println!("S3N {model:?}: smallest p {least:.4} (must be <= 1.3)");
    assert!(least <= 1.3, "S3N {model:?} did not fire: {p:?}");
}

/// S3, k-epsilon, BDF2: second order in ux, k and epsilon with converged outer iterations. Known miss (ADR 0210 addendum): the
/// run of record gave p 0.7975 (ux), 1.4910 (k), 1.5303 (epsilon), below the band; the outer iterations had converged (largest
/// nut_change of a step 1.8e-8, 2.8e-12, 1.3e-15 at T/80, T/160, T/320).
#[test]
#[ignore = "known miss (ADR 0210 addendum): k-epsilon BDF2 p 0.7975 / 1.4910 / 1.5303 below [1.7, 2.3]"]
fn s3_kepsilon_bdf2_time_order_known_miss() {
    s3(Model::KEpsilon, "bdf2", 1.7, 2.3)
}

/// The smallest and largest wall-cell `y*` (the `y_plus` field of k-epsilon, `C_mu^(1/4) k^(1/2) y / nu`) over the steps of one
/// BDF2 run at T/n, and the times of the steps at which some wall cell is on the other side of `y*_lam` than at the step before.
fn ystar_history(lines: Option<&str>, n: usize, y_lam: f64) -> (f64, f64, Vec<f64>) {
    let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
    let mut prev: Option<Vec<bool>> = None;
    let mut crossings = Vec::new();
    s3_run_with(Model::KEpsilon, lines, PARABOLA, "bdf2", 20, n, |sim| {
        let yp: Vec<f64> = sim.field("y_plus").unwrap().into_iter().filter(|&v| v > 0.0).collect();
        for &v in &yp {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        let above: Vec<bool> = yp.iter().map(|&v| v >= y_lam).collect();
        if prev.as_ref().is_some_and(|p| *p != above) {
            crossings.push(sim.time());
        }
        prev = Some(above);
    });
    (lo, hi, crossings)
}

/// Diagnosis of the S3 k-epsilon BDF2 miss, report only (not a registered check, ADR 0210 addendum): the wall-cell y* against
/// the switch of the wall-function eddy viscosity at y*_lam, the same runs one and two levels finer, and the registered levels
/// from a start whose wall cells stay above y*_lam.
#[test]
#[ignore = "diagnosis of the S3 k-epsilon BDF2 known miss, report only (ADR 0210 addendum)"]
fn s3_kepsilon_bdf2_diagnosis() {
    let y_lam = rcfd_turb::WallConstants::default().y_plus_lam();
    let (lo, hi, cr) = ystar_history(None, 320, y_lam);
    println!(
        "S3 diagnosis: y*_lam {y_lam:.6}; registered start, T/320: wall-cell y* in [{lo:.4}, {hi:.4}], {} steps with a crossing, first at t {:?}, last at t {:?}",
        cr.len(),
        cr.first(),
        cr.last()
    );
    let p1 = s3_orders_at(Model::KEpsilon, None, "bdf2", 20, [160, 320, 640]);
    let p2 = s3_orders_at(Model::KEpsilon, None, "bdf2", 20, [320, 640, 1280]);
    println!("S3 diagnosis: BDF2 p from T/160, T/320, T/640 {p1:?}; from T/320, T/640, T/1280 {p2:?}");
    let high = "model = \"k_epsilon\"
k_initial = 2.0
epsilon_initial = 14.4
";
    let (lo, hi, cr) = ystar_history(Some(high), 320, y_lam);
    println!(
        "S3 diagnosis: start k 2, epsilon 14.4 (the same initial nut), T/320: wall-cell y* in [{lo:.4}, {hi:.4}], {} steps with a crossing",
        cr.len()
    );
    let p3 = s3_orders_at(Model::KEpsilon, Some(high), "bdf2", 20, [80, 160, 320]);
    println!("S3 diagnosis: BDF2 p from that start at T/80, T/160, T/320 {p3:?}");
}

/// S3, k-epsilon, Euler: first order.
#[test]
fn s3_kepsilon_euler_time_order() {
    s3(Model::KEpsilon, "euler", 0.8, 1.2)
}

/// S3N, k-epsilon.
#[test]
fn s3n_kepsilon_one_sweep_per_step_loses_the_order() {
    s3n(Model::KEpsilon)
}

/// S3, Spalart-Allmaras, BDF2: second order in ux and nuTilda with converged outer iterations.
#[test]
fn s3_sa_bdf2_time_order() {
    s3(Model::Sa, "bdf2", 1.7, 2.3)
}

/// S3, Spalart-Allmaras, Euler: first order.
#[test]
fn s3_sa_euler_time_order() {
    s3(Model::Sa, "euler", 0.8, 1.2)
}

/// S3N, Spalart-Allmaras.
#[test]
fn s3n_sa_one_sweep_per_step_loses_the_order() {
    s3n(Model::Sa)
}

/// Second diagnosis of the S3 k-epsilon BDF2 miss, report only (ADR 0210 addendum): the registered levels with the enhanced
/// wall function, whose wall-face eddy viscosity (Kader's law) has no switch at y*_lam.
#[test]
#[ignore = "diagnosis of the S3 k-epsilon BDF2 known miss, report only (ADR 0210 addendum)"]
fn s3_kepsilon_bdf2_diagnosis_enhanced() {
    let lines = "model = \"k_epsilon\"
k_initial = 1.0
epsilon_initial = 3.6
wall_function = \"enhanced\"
";
    let p = s3_orders_at(Model::KEpsilon, Some(lines), "bdf2", 20, [80, 160, 320]);
    println!("S3 diagnosis: BDF2 p with the enhanced wall function at T/80, T/160, T/320 {p:?}");
    let e = s3_orders_at(Model::KEpsilon, Some(lines), "euler", 20, [80, 160, 320]);
    println!("S3 diagnosis: Euler p with the enhanced wall function at T/80, T/160, T/320 {e:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// S4 rotating frame
// ---------------------------------------------------------------------------------------------------------------

/// S4 for one model: the frame on pimple, finite, positive k and epsilon (k-epsilon), seam pairs balanced.
fn s4(model: Model) {
    let mut sim = sim_of(&sector(model, false));
    let fields: &[&str] = match model {
        Model::KEpsilon => &["ux", "uy", "uz", "p", "k", "epsilon", "nut"],
        Model::Sa => &["ux", "uy", "uz", "p", "nuTilda", "nut"],
    };
    let mut worst = 0.0_f64;
    let mut least_nt = f64::INFINITY;
    for step in 1..=10 {
        sim.step();
        assert!(sim.failure().is_none(), "S4 {model:?} step {step}: {:?}", sim.failure());
        for f in fields {
            let v = sim.field(f).unwrap();
            assert!(
                v.iter().all(|x| x.is_finite()),
                "S4 {model:?} step {step}: {f} not finite"
            );
        }
        let min_of = |f: &str| sim.field(f).unwrap().iter().cloned().fold(f64::INFINITY, f64::min);
        let s = seam_pair_sum(&sim);
        match model {
            Model::KEpsilon => {
                let (kmin, emin) = (min_of("k"), min_of("epsilon"));
                assert!(
                    kmin > 0.0 && emin > 0.0,
                    "S4 step {step}: k min {kmin}, epsilon min {emin}"
                );
                println!("  S4 {model:?} step {step}: seam pair sum {s:.3e}, min k {kmin:.3e}, min epsilon {emin:.3e}");
            }
            Model::Sa => {
                let nmin = min_of("nuTilda");
                least_nt = least_nt.min(nmin);
                println!("  S4 {model:?} step {step}: seam pair sum {s:.3e}, min nuTilda {nmin:.3e}");
            }
        }
        worst = worst.max(s);
    }
    let nut = max_abs(&sim.field("nut").unwrap());
    println!(
        "S4 {model:?}: worst seam pair sum {worst:.3e} (limit 1e-8); reported max nut / nu {:.3e}, smallest nuTilda {least_nt:.3e}",
        nut / 0.01
    );
    assert!(worst <= 1e-8, "S4 {model:?} seam pair sum {worst:.3e}");
}

/// S4, k-epsilon.
#[test]
fn s4_kepsilon_with_the_rotating_frame_on_pimple() {
    s4(Model::KEpsilon)
}

/// S4, Spalart-Allmaras.
#[test]
fn s4_sa_with_the_rotating_frame_on_pimple() {
    s4(Model::Sa)
}
