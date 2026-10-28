//! ADR 0154 (job 48T): pressure spokes on the refined cylinder O-grid of `example-projects/wind-tunnel`.
//!
//! * `w48t_e1_harness_replicates_the_case` and `w48t_dg1_pressure_lag_two_correctors` (amendment 1, ignored diagnostics that
//!   print): a `PimpleSolver` seeded from the case's own `Simulation` (geometry and initial fields) with the case's conditions
//!   and settings, so that the ADR 0073 pressure-lag estimate can run with two correctors (the case path enables it with one).
//! * `w48t_k1_..._known_miss` (amendment 2): the default PISO (two correctors) on the 64 x 256 O-grid (first radial cell
//!   0.00262 D) through `Simulation` to t = 3 must not be stopped by the divergence guard and must be clean, M_hf <= 0.0114. It
//!   diverges at step 30: a known miss, ignored with the limit intact. `w48t_k2_...` is the same with `n_correctors = 3`.
//! * `w48t_mhf_control_cross_check` (amendment 2, prints): the Rust M_hf against `scripts/w48t_spoke_metrics.py` on the control.
//!
//! Every test prints what it measured before it asserts.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{LagAction, LagCheck, PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, Freestream, ScalarField, VectorField};
use serde_json::{json, Value};

// ------------------------------------------------------------------------------------------------ set-up

/// The committed example case (`cylinder_ogrid` 64 x 48, PISO, dt 1/16) with `overrides` applied.
fn case_with(overrides: &[(&str, Value)]) -> Case {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../example-projects/wind-tunnel/cases/cylinder-re100.toml"
    );
    let mut case = Case::parse_toml(&std::fs::read_to_string(path).expect("example case readable"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    for (k, v) in overrides {
        case.tree.set(k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    case
}

/// Mesh and step of an arm.
fn mesh_dt(nx: u32, ny: u32, dt: f64) -> Vec<(&'static str, Value)> {
    vec![("mesh.nx", json!(nx)), ("mesh.ny", json!(ny)), ("solver.dt", json!(dt))]
}

/// Algorithm of a harness arm: PISO with `n_correctors`, or PIMPLE `(n_outer, alpha_u, alpha_p)` with `n_correctors`.
#[derive(Clone, Copy, Debug)]
struct Coupling {
    n_correctors: usize,
    pimple: Option<(usize, f64, f64)>,
}

const PISO2: Coupling = Coupling {
    n_correctors: 2,
    pimple: None,
};

/// A `PimpleSolver` with the geometry and initial fields of the case's `Simulation`, the case's conditions (cylinder velocity
/// fixed 0, far field `Freestream` with U_ref (1, 0, 0) for velocity and p = 0 for pressure) and the settings `rcfd-api` builds for
/// it (`sim.rs`, transient branch): nu 0.01, `linear_upwind`, AMG pressure above 2000 cells, alpha_u 0.9, alpha_p 1.0 (case
/// defaults), the coupling of the arm. Returns the solver and the case's step.
fn harness(case: &Case, c: Coupling, lag: Option<LagCheck>) -> (PimpleSolver, f64) {
    let sim = Simulation::new(case).unwrap_or_else(|e| panic!("{e:?}"));
    let geom = sim.geom.clone();
    let (ux, uy, p0) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("p").unwrap(),
    );
    let n = geom.n_cells();
    let mut u = VectorField::from_fn("U", &geom, |_| [0.0; 3]);
    u.values = (0..n).map(|i| [ux[i], uy[i], 0.0]).collect();
    let uinf = [1.0, 0.0, 0.0];
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: uinf,
            fixed_on_inflow: true,
        }),
    )
    .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.values = p0;
    p.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: [0.0; 3],
            fixed_on_inflow: false,
        }),
    )
    .unwrap();
    let mut s = match c.pimple {
        Some((n_outer, _, _)) => PimpleSettings::pimple(0.01, n_outer),
        None => PimpleSettings::piso(0.01),
    };
    s.convection = ConvScheme::LinearUpwind;
    s.n_correctors = c.n_correctors;
    (s.alpha_u, s.alpha_p) = match c.pimple {
        Some((_, au, ap)) => (au, ap),
        None => (0.9, 1.0),
    };
    s.amg_pressure = n > 2000;
    s.pressure_lag_check = lag;
    let dt = case.tree.get("solver.dt").unwrap().as_f64().unwrap();
    (PimpleSolver::new(geom, u, p, s), dt)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    let scale = b.iter().fold(0.0f64, |m, x| m.max(x.abs())).max(1e-300);
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

// ------------------------------------------------------------------------------------------------ E1, DG1 (amendment 1)

/// E1: the harness replicates `Simulation::step` on the committed case (64 x 48, dt 1/16) for 16 steps, bit for bit (prints).
#[test]
#[ignore = "ADR 0154 amendment 1 diagnostic (prints): cargo test --release -p rcfd-api --test w48t_cylinder_spokes -- --ignored"]
fn w48t_e1_harness_replicates_the_case() {
    let case = case_with(&[]);
    let mut sim = Simulation::new(&case).unwrap();
    let (mut h, dt) = harness(&case, PISO2, None);
    for _ in 0..16 {
        sim.step();
        h.step(dt);
    }
    let names = ["p", "ux", "uy"];
    let mine = [
        h.p.values.clone(),
        h.u.values.iter().map(|v| v[0]).collect::<Vec<_>>(),
        h.u.values.iter().map(|v| v[1]).collect::<Vec<_>>(),
    ];
    for (name, hv) in names.iter().zip(&mine) {
        let sv = sim.field(name).unwrap();
        println!(
            "E1 {name}: bit-identical {}, max relative difference {:.3e} (t = {})",
            bits(hv) == bits(&sv),
            max_rel(hv, &sv),
            h.time
        );
    }
}

/// A DG1 arm: name, case overrides, coupling, steps between estimates.
type Dg1Arm = (&'static str, Vec<(&'static str, Value)>, Coupling, usize);

/// DG1: the ADR 0073 pressure-lag spectral radius of the PISO/PIMPLE step with the arm's correctors, at step 1 and at t = 1
/// (prints; amendment 1 holds the reading rule).
#[test]
#[ignore = "ADR 0154 amendment 1 diagnostic (prints): cargo test --release -p rcfd-api --test w48t_cylinder_spokes -- --ignored"]
fn w48t_dg1_pressure_lag_two_correctors() {
    let only = std::env::var("W48T_DG1").unwrap_or_default();
    let c4 = Coupling {
        n_correctors: 4,
        pimple: None,
    };
    let p2 = Coupling {
        n_correctors: 2,
        pimple: Some((2, 0.9, 0.9)),
    };
    let arms: Vec<Dg1Arm> = vec![
        ("G64", mesh_dt(64, 48, 1.0 / 16.0), PISO2, 16),
        ("G128", mesh_dt(128, 96, 1.0 / 32.0), PISO2, 32),
        ("G256x48", mesh_dt(256, 48, 1.0 / 64.0), PISO2, 64),
        ("G64x256", mesh_dt(64, 256, 1.0 / 64.0), PISO2, 16),
        ("G256", mesh_dt(256, 256, 1.0 / 64.0), PISO2, 64),
        ("G256s", mesh_dt(256, 256, 1.0 / 256.0), PISO2, 256),
        (
            "G256g",
            [mesh_dt(256, 256, 1.0 / 64.0), vec![("mesh.grading", json!(10.88))]].concat(),
            PISO2,
            64,
        ),
        ("G256c4", mesh_dt(256, 256, 1.0 / 64.0), c4, 64),
        ("G256p2", mesh_dt(256, 256, 1.0 / 64.0), p2, 64),
    ];
    for (name, ov, coupling, every) in arms {
        if !only.is_empty() && !only.split(',').any(|x| x == name) {
            continue;
        }
        let case = case_with(&ov);
        let lag = LagCheck {
            every,
            start: 1,
            iterations: 60,
            action: LagAction::Warn,
        };
        let (mut h, dt) = harness(&case, coupling, Some(lag));
        let t0 = std::time::Instant::now();
        let (mut iters, mut steps) = (Vec::new(), 0usize);
        while h.time < 1.0 - 1e-9 && h.diverged.is_none() {
            let info = h.step(dt);
            iters.push(info.p_iters);
            steps += 1;
        }
        let cc = h.geom.mesh.cell_centres();
        let est: Vec<String> = h
            .lag_estimates
            .iter()
            .map(|w| {
                let x = cc[w.cell];
                format!(
                    "step {} rho {:.4} at r {:.4} theta {:.1} deg",
                    w.step,
                    w.rho,
                    x[0].hypot(x[1]),
                    x[1].atan2(x[0]).to_degrees()
                )
            })
            .collect();
        let mean = iters.iter().sum::<usize>() as f64 / iters.len().max(1) as f64;
        println!(
            "DG1 {name} ({coupling:?}, dt {dt}): {steps} steps to t = {:.4}{}; p_iters max {} mean {mean:.1}; estimates: {}; wall {:.1} s",
            h.time,
            h.diverged.as_ref().map_or(String::new(), |d| format!(", STOPPED: {}", d.message(None))),
            iters.iter().max().copied().unwrap_or(0),
            est.join("; "),
            t0.elapsed().as_secs_f64()
        );
    }
}

// ------------------------------------------------------------------------------------------------ M_hf (ADR 0154 "Metrics")

/// Kinematic dynamic pressure of the free stream, 0.5 rho U^2 with rho = 1, U = 1.
const Q: f64 = 0.5;

/// Cells grouped into rings by centre radius (equal to 1e-9 relative) and ordered by angle in [0, 2 pi); `(radius, cells)`.
fn rings(centres: &[[f64; 3]]) -> Vec<(f64, Vec<usize>)> {
    let r: Vec<f64> = centres.iter().map(|c| c[0].hypot(c[1])).collect();
    let mut order: Vec<usize> = (0..r.len()).collect();
    order.sort_by(|&a, &b| r[a].total_cmp(&r[b]));
    let mut groups: Vec<Vec<usize>> = vec![vec![order[0]]];
    for w in order.windows(2) {
        if r[w[1]] - r[w[0]] > 1e-9 * r[w[1]] {
            groups.push(Vec::new());
        }
        groups.last_mut().unwrap().push(w[1]);
    }
    let theta = |c: usize| {
        centres[c][1]
            .atan2(centres[c][0])
            .rem_euclid(2.0 * std::f64::consts::PI)
    };
    groups
        .into_iter()
        .map(|mut g| {
            g.sort_by(|&a, &b| theta(a).total_cmp(&theta(b)));
            (g.iter().map(|&c| r[c]).sum::<f64>() / g.len() as f64, g)
        })
        .collect()
}

/// M_hf: RMS over the rings with centre radius in [0.55, 0.70] of the part of p in the circumferential modes n/8 <= k <= n/2,
/// divided by q (by Parseval from the DFT of each ring: (2 sum_{n/8 <= k < n/2} |P_k|^2 + |P_{n/2}|^2) / n per ring).
fn m_hf(p: &[f64], centres: &[[f64; 3]]) -> f64 {
    let (mut sum, mut cnt) = (0.0, 0usize);
    for (rj, g) in rings(centres) {
        if !(0.55..=0.70).contains(&rj) {
            continue;
        }
        let n = g.len();
        assert!(n % 8 == 0, "ring of {n} cells");
        let mut hp = 0.0;
        for k in n / 8..=n / 2 {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, &c) in g.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * i % n) as f64 / n as f64;
                re += p[c] * a.cos();
                im += p[c] * a.sin();
            }
            hp += if k == n / 2 { 1.0 } else { 2.0 } * (re * re + im * im);
        }
        sum += hp / n as f64;
        cnt += n;
    }
    assert!(cnt > 0, "no ring in [0.55, 0.70]");
    (sum / cnt as f64).sqrt() / Q
}

/// The case run through `Simulation` to `t_end`: (stopped by the guard, time reached, M_hf of p at the end, steps).
fn run_case(overrides: &[(&str, Value)], t_end: f64) -> (bool, f64, f64, usize) {
    let mut ov = overrides.to_vec();
    ov.push(("solver.end_time", json!(t_end)));
    let case = case_with(&ov);
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
    }
    let cc = sim.geom.mesh.cell_centres().to_vec();
    let p = sim.field("p").unwrap();
    let finite = p.iter().all(|x| x.is_finite());
    let m = if finite { m_hf(&p, &cc) } else { f64::INFINITY };
    (sim.diverged(), sim.time(), m, steps)
}

/// K1 and K2's registered limit: clean by L3 at t = 3, max(2 x M_hf(R64, t = 3), 0.01) with R64's measured 0.0057.
const K_LIMIT: f64 = 0.0114;

/// The 64 x 256 arm of ADR 0154 (B2b): the committed case with 256 radial cells (first cell 0.00262 D) at dt 1/64.
fn k_overrides(n_correctors: u32) -> Vec<(&'static str, Value)> {
    let mut v = mesh_dt(64, 256, 1.0 / 64.0);
    v.push(("solver.n_correctors", json!(n_correctors)));
    v
}

/// K1 (ADR 0154 amendment 2): known miss. The default PISO (two correctors) on the 64 x 256 O-grid is stopped by the divergence
/// guard at step 30 (t = 0.453), where it must reach t = 3 clean (M_hf <= 0.0114).
#[test]
#[ignore = "known miss (ADR 0154 K1): PISO with 2 correctors on the 64 x 256 cylinder O-grid (first cell 0.00262 D, nu dt / dr1^2 = 23) diverges at step 30; the pressure-lag map amplifies by 4.4 per step; a fix needs the maintainer's decision"]
fn w48t_k1_fine_wall_layer_piso_two_correctors_is_clean_known_miss() {
    let (diverged, t, m, steps) = run_case(&k_overrides(2), 3.0);
    println!("K1: {steps} steps, stopped by the guard {diverged}, t = {t}, M_hf {m:.4e} (limit {K_LIMIT})");
    assert!(
        !diverged && (t - 3.0).abs() < 1e-9,
        "K1: stopped at t = {t} after {steps} steps"
    );
    assert!(m <= K_LIMIT, "K1: M_hf {m:.4e} > {K_LIMIT}");
}

/// K2 (ADR 0154 amendment 2): known miss. The K1 set-up with a third corrector reaches t = 3 but is not clean (M_hf 282 at t = 3
/// against the limit 0.0114): on this mesh (in-plane wall aspect 18.8) three correctors are not enough.
#[test]
#[ignore = "known miss (ADR 0154 K2, about 15 s in release): the K1 set-up with solver.n_correctors = 3 reaches t = 3 with M_hf 282 (limit 0.0114); a fix needs the maintainer's decision"]
fn w48t_k2_fine_wall_layer_piso_three_correctors_is_clean_known_miss() {
    let (diverged, t, m, steps) = run_case(&k_overrides(3), 3.0);
    println!("K2: {steps} steps, stopped by the guard {diverged}, t = {t}, M_hf {m:.4e} (limit {K_LIMIT})");
    assert!(
        !diverged && (t - 3.0).abs() < 1e-9,
        "K2: stopped at t = {t} after {steps} steps"
    );
    assert!(m <= K_LIMIT, "K2: M_hf {m:.4e} > {K_LIMIT}");
}

/// Amendment 2 cross-check (prints): the Rust M_hf on the control (64 x 48, dt 1/16, t = 3, in-process f64) against the script's
/// value on R64's stored f32 field (0.00570..., `W48T_R64_MHF_T3` overrides it); registered agreement <= 1e-3 relative.
#[test]
#[ignore = "ADR 0154 amendment 2 cross-check (prints)"]
fn w48t_mhf_control_cross_check() {
    let script: f64 = std::env::var("W48T_R64_MHF_T3")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0057);
    let (diverged, t, m, steps) = run_case(&[], 3.0);
    println!(
        "control: {steps} steps, t = {t}, guard {diverged}, Rust M_hf {m:.6e}, script {script:.6e}, relative difference {:.3e}",
        (m - script).abs() / script
    );
}

/// The user's mesh of ADR 0154 (256 x 256, grading 100, first radial cell 0.00262 D) at dt 1/64 with `extra` settings.
fn user_mesh(extra: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> {
    let mut v = mesh_dt(256, 256, 1.0 / 64.0);
    v.extend_from_slice(extra);
    v
}

/// W3, the recommended workaround of ADR 0154 (the accepted candidate with the least measured wall time per simulated second):
/// `solver.time_advance = "fractional_step"` on the user's mesh reaches t = 3 clean (M_hf <= 0.0114; `rcfd run` twin W3: 7.9e-8).
#[test]
#[ignore = "heavy (ADR 0154 W3, 65,536 cells to t = 3): the recommended workaround, fractional step on the user's 256 x 256 mesh"]
fn w48t_w3_fractional_step_on_the_user_mesh_is_clean() {
    let (diverged, t, m, steps) = run_case(&user_mesh(&[("solver.time_advance", json!("fractional_step"))]), 3.0);
    println!("W3: {steps} steps, stopped by the guard {diverged}, t = {t}, M_hf {m:.4e} (limit {K_LIMIT})");
    assert!(
        !diverged && (t - 3.0).abs() < 1e-9,
        "W3: stopped at t = {t} after {steps} steps"
    );
    assert!(m <= K_LIMIT, "W3: M_hf {m:.4e} > {K_LIMIT}");
}

/// W5, the other accepted workaround of ADR 0154: PISO with `solver.n_correctors = 3` on the user's mesh reaches t = 3 clean
/// (`rcfd run` twin W5: M_hf 9.2e-8 at t = 3).
#[test]
#[ignore = "heavy (ADR 0154 W5, 65,536 cells to t = 3): PISO with a third corrector on the user's 256 x 256 mesh"]
fn w48t_w5_three_correctors_on_the_user_mesh_is_clean() {
    let (diverged, t, m, steps) = run_case(&user_mesh(&[("solver.n_correctors", json!(3))]), 3.0);
    println!("W5: {steps} steps, stopped by the guard {diverged}, t = {t}, M_hf {m:.4e} (limit {K_LIMIT})");
    assert!(
        !diverged && (t - 3.0).abs() < 1e-9,
        "W5: stopped at t = {t} after {steps} steps"
    );
    assert!(m <= K_LIMIT, "W5: M_hf {m:.4e} > {K_LIMIT}");
}
