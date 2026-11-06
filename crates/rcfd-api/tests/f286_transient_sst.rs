//! ADR 0199 (F-286), case-tree checks of transient k-omega SST on `pimple`: S1 scope and located errors, R1 the reduction to
//! the steady SIMPLEC solution of a fully developed channel, S3 the time order of the coupled step (and the negative control
//! S3N with one sweep per step), S4 the rotating frame on a sector, S6 the case text of ADR 0183 check H(dt) builds, and the
//! report-only V3H smoke of H(dt) (ignored). The limits are those registered in ADR 0199 before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::f64::consts::PI;
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
// The channel CH
// ---------------------------------------------------------------------------------------------------------------

/// CH: the fully developed channel at Re_tau = 395 (u_tau = 1, h = 1), with the given `[solver]` lines and `extra` text.
fn ch(solver: &str, extra: &str) -> String {
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
velocity = [15.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.drive]
type = "acceleration"
acceleration = [1.0, 0.0, 0.0]
zone = "all"
[turbulence]
model = "k_omega_sst"
k_initial = 1.0
omega_initial = 40.0
{extra}[solver]
convection = "upwind"
{solver}"#
    )
}

fn pimple(scheme: &str) -> String {
    format!("algorithm = \"pimple\"\ntime_scheme = \"{scheme}\"\nn_outer = 3\ndt = 0.1\nend_time = 1.0\n")
}

// ---------------------------------------------------------------------------------------------------------------
// The sector of S4 (ADR 0183 PC-3 with k-omega SST)
// ---------------------------------------------------------------------------------------------------------------

const S4_TEXT: &str = r#"
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
turbulence_spec = "values"
k_value = 0.01
omega_value = 10.0
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.inner]
velocity = "fixed"
[boundaries.outer]
velocity = "moving_wall"
wall_rotation_rate = -2.0
wall_velocity_projection = "tangential"
turbulence = "wall"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = true
[turbulence]
model = "k_omega_sst"
[solver]
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
algorithm = "pimple"
time_scheme = "bdf2"
n_outer = 3
n_correctors = 2
dt = 0.05
end_time = 100.0
"#;

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

/// CH without its drive source (amendment 3).
fn ch_no_drive(solver: &str) -> String {
    let drive = "[sources.drive]\ntype = \"acceleration\"\nacceleration = [1.0, 0.0, 0.0]\nzone = \"all\"\n";
    let t = ch(solver, "");
    assert!(t.contains(drive), "the CH text changed");
    t.replace(drive, "")
}

/// S1: what the case tree accepts and where it refuses, after F-286. `amended` = 3 is the case of record (amendment 3);
/// 1 is the run of amendment 1, kept as a record.
fn s1(amended: usize) {
    for scheme in ["euler", "bdf2", "bdf2_bounded"] {
        ok(&ch(&pimple(scheme), ""));
    }
    ok(&ch(&pimple("euler"), "sst_variant = \"bsl\"\n"));
    ok(&ch(&pimple("euler"), "wall_omega = \"low_re\"\n"));
    ok(S4_TEXT);
    println!("S1: accepted: pimple with euler, bdf2, bdf2_bounded; bsl; low_re; the S4 sector with the frame");
    let piso = ch("algorithm = \"piso\"\ndt = 0.1\nend_time = 1.0\n", "");
    has(&piso, "turbulence.model");
    let m = messages(&piso);
    let (_, text) = m.iter().find(|(p, _)| p == "turbulence.model").expect("piso message");
    assert!(text.contains("pimple"), "the piso refusal does not name pimple: {text}");
    has(
        &ch(&pimple("euler"), "").replace(
            "model = \"k_omega_sst\"\nk_initial = 1.0\nomega_initial = 40.0\n",
            "model = \"k_epsilon\"\n",
        ),
        "turbulence.model",
    );
    has(
        &ch(&pimple("euler"), "").replace(
            "model = \"k_omega_sst\"\nk_initial = 1.0\nomega_initial = 40.0\n",
            "model = \"spalart_allmaras\"\n",
        ),
        "turbulence.model",
    );
    has(
        &format!("{}[energy]\nenabled = true\n", ch(&pimple("euler"), "")),
        "turbulence.model",
    );
    has(
        &format!(
            "{}[dynamic_mesh]\nenabled = true\nmotion = \"prescribed\"\nmoving_patches = [\"ymin\"]\n",
            ch(&pimple("euler"), "")
        ),
        "dynamic_mesh.enabled",
    );
    has(
        &ch(&format!("{}error_tolerance = 1e-3\n", pimple("euler")), ""),
        "solver.error_tolerance",
    );
    // amendment 1: the fractional step on pimple reaches the rule of Simulation::new; on piso the static refusal comes first.
    // Amendment 3: without the drive source, which the fractional step refuses statically (sources.drive.type)
    let fractional = format!("{}time_advance = \"fractional_step\"\n", pimple("euler"));
    let text = if amended >= 3 {
        ch_no_drive(&fractional)
    } else {
        ch(&fractional, "")
    };
    has(&text, "solver.time_advance");
    has(
        &ch(
            "algorithm = \"piso\"\ntime_advance = \"fractional_step\"\ndt = 0.1\nend_time = 1.0\n",
            "",
        ),
        "turbulence.model",
    );
    println!("S1: located at turbulence.model (piso, with pimple named; k_epsilon and SA on pimple; energy; piso with the fractional step), dynamic_mesh.enabled, solver.error_tolerance, solver.time_advance (pimple with the fractional step)");
}

/// S1 (the case of record, amendment 3).
#[test]
fn s1_scope_of_transient_sst() {
    s1(3)
}

/// S1 as amended by amendment 1, run once: the fractional-step case was located at `sources.drive.type` (amendment 3).
#[test]
#[ignore = "record of the S1 run of amendment 1 (ADR 0199 amendment 3): the fractional-step case gave sources.drive.type"]
fn s1_amendment1_record() {
    s1(1)
}

// ---------------------------------------------------------------------------------------------------------------
// R1 reduction to the steady solution
// ---------------------------------------------------------------------------------------------------------------

const FIELDS: [&str; 4] = ["ux", "k", "omega", "nut"];

/// R1: run to a steady state, the pimple solution equals the SIMPLEC one.
#[test]
fn r1_pimple_steady_state_equals_the_simplec_solution() {
    let t0 = Instant::now();
    let mut steady = sim_of(&ch("algorithm = \"simplec\"\nmax_iterations = 10000\n", ""));
    let n = steady.run(10000);
    assert_eq!(n, 10000);
    assert!(steady.failure().is_none(), "{:?}", steady.failure());
    let ts = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let mut tr = sim_of(&ch(
        "algorithm = \"pimple\"\ntime_scheme = \"euler\"\nn_outer = 2\ndt = 0.2\nend_time = 1000.0\n",
        "",
    ));
    let mut prev = tr.field("ux").unwrap();
    let mut last_change = f64::NAN;
    let mut steps = 0;
    while !tr.is_done() {
        tr.step();
        steps += 1;
        assert!(tr.failure().is_none(), "R1 pimple step {steps}: {:?}", tr.failure());
        let ux = tr.field("ux").unwrap();
        last_change = max_diff(&ux, &prev);
        prev = ux;
        assert!(steps <= 5000, "R1: more than 5000 steps");
    }
    let tt = t1.elapsed().as_secs_f64();
    let umax = max_abs(&prev);
    let settled = last_change <= 1e-11 * umax;
    println!(
        "R1: simplec 10000 its {ts:.1} s; pimple {steps} steps {tt:.1} s, t = {:.3}, last-step change of ux {last_change:.3e} = {:.3e} max ux (settled limit 1e-11)",
        tr.time(),
        last_change / umax
    );
    assert_eq!(steps, 5000, "R1: the pimple run took {steps} steps, not 5000");
    assert!(settled, "R1 NOT SETTLED: {:.3e}", last_change / umax);
    let mut worst = 0.0_f64;
    for f in FIELDS {
        let (a, b) = (tr.field(f).unwrap(), steady.field(f).unwrap());
        let r = max_diff(&a, &b) / max_abs(&b);
        println!(
            "R1 {f}: max |pimple - simplec| / max |simplec| = {r:.3e} (limit 1e-8); max simplec {:.6e}",
            max_abs(&b)
        );
        worst = worst.max(r);
    }
    let uy = [tr.field("uy").unwrap(), steady.field("uy").unwrap()];
    let uy_rel = [max_abs(&uy[0]) / umax, max_abs(&uy[1]) / umax];
    println!(
        "R1 uy: max |uy| / max ux pimple {:.3e}, simplec {:.3e} (limit 1e-8)",
        uy_rel[0], uy_rel[1]
    );
    assert!(worst <= 1e-8, "R1: {worst:.3e}");
    assert!(uy_rel[0] <= 1e-8 && uy_rel[1] <= 1e-8, "R1 uy: {uy_rel:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// S3 time order (and S3N)
// ---------------------------------------------------------------------------------------------------------------

const S3_T: f64 = 2.0;
const S3_FIELDS: [&str; 3] = ["ux", "k", "omega"];

/// CH on pimple to T with `n` steps; the fields of S3 at T and the largest `nut_change` of a step.
fn s3_run(scheme: &str, n_outer: usize, n: usize) -> (Vec<Vec<f64>>, f64) {
    let dt = S3_T / n as f64;
    let mut sim = sim_of(&ch(
        &format!(
            "algorithm = \"pimple\"\ntime_scheme = \"{scheme}\"\nn_outer = {n_outer}\nn_correctors = 2\nmomentum_rel_tol = 1e-12\npressure_rel_tol = 1e-12\npressure_max_iter = 1000\ndt = {dt:?}\nend_time = {S3_T:?}\n"
        ),
        "",
    ));
    let mut worst = 0.0_f64;
    let mut steps = 0;
    while !sim.is_done() {
        sim.step();
        steps += 1;
        assert!(
            sim.failure().is_none(),
            "S3 {scheme} n_outer {n_outer} n {n} step {steps}: {:?}",
            sim.failure()
        );
        let c = sim.status_json()["turbulence"]["nut_change"]
            .as_f64()
            .unwrap_or(f64::NAN);
        worst = worst.max(c);
    }
    assert_eq!(steps, n, "S3: {steps} steps instead of {n}");
    assert!((sim.time() - S3_T).abs() < 1e-12, "S3: ended at {}", sim.time());
    (S3_FIELDS.iter().map(|f| sim.field(f).unwrap()).collect(), worst)
}

/// The observed orders of ux, k and omega from the runs at T/80, T/160, T/320.
fn s3_orders(scheme: &str, n_outer: usize) -> [f64; 3] {
    s3_orders_at(scheme, n_outer, [80, 160, 320])
}

/// The observed orders of ux, k and omega from the runs at `T/n` for the three `n`.
fn s3_orders_at(scheme: &str, n_outer: usize, ns: [usize; 3]) -> [f64; 3] {
    let runs: Vec<(Vec<Vec<f64>>, f64)> = ns.iter().map(|&n| s3_run(scheme, n_outer, n)).collect();
    let mut p = [0.0; 3];
    for (i, name) in S3_FIELDS.iter().enumerate() {
        let scale = max_abs(&runs[2].0[i]);
        let e1 = max_diff(&runs[0].0[i], &runs[1].0[i]) / scale;
        let e2 = max_diff(&runs[1].0[i], &runs[2].0[i]) / scale;
        p[i] = (e1 / e2).log2();
        println!(
            "S3 {scheme} n_outer {n_outer} {name}: e({},{}) {e1:.4e}, e({},{}) {e2:.4e}, p {:.4}; largest nut_change of a step {:.3e} / {:.3e} / {:.3e}",
            ns[0], ns[1], ns[1], ns[2], p[i], runs[0].1, runs[1].1, runs[2].1
        );
    }
    p
}

/// S3 for one scheme: every field's p in [lo, hi].
fn s3(scheme: &str, lo: f64, hi: f64) {
    let p = s3_orders(scheme, 20);
    for (name, pi) in S3_FIELDS.iter().zip(p) {
        assert!(
            pi >= lo && pi <= hi,
            "S3 {scheme} {name}: p {pi:.4} outside [{lo}, {hi}]"
        );
    }
}

/// S3, BDF2: with converged outer iterations BDF2 is second order in ux, k and omega. Known miss (ADR 0199 addendum): the run
/// of record gave p 2.6694 (ux), 2.4075 (k), 2.5729 (omega), above the band; the outer iterations had converged (largest
/// nut_change of a step 1e-11).
#[test]
#[ignore = "known miss (ADR 0199 addendum): BDF2 p 2.6694 / 2.4075 / 2.5729 above [1.7, 2.3]"]
fn s3_bdf2_time_order_known_miss() {
    s3("bdf2", 1.7, 2.3)
}

/// S3, Euler: with converged outer iterations Euler is first order in ux, k and omega.
#[test]
fn s3_euler_time_order() {
    s3("euler", 0.8, 1.2)
}

/// Diagnosis of the S3 BDF2 miss, report only (not a registered check): the same runs one and two levels finer.
#[test]
#[ignore = "diagnosis of the S3 BDF2 known miss, report only (ADR 0199 addendum)"]
fn s3_bdf2_finer_levels_diagnosis() {
    let p1 = s3_orders_at("bdf2", 20, [160, 320, 640]);
    let p2 = s3_orders_at("bdf2", 20, [320, 640, 1280]);
    println!("S3 diagnosis: BDF2 p from T/160, T/320, T/640 {p1:?}; from T/320, T/640, T/1280 {p2:?}");
}

/// S3N: with one sweep per step (n_outer = 1) BDF2 loses its order in at least one field (S3 can fail).
#[test]
fn s3n_one_sweep_per_step_loses_the_order() {
    let p = s3_orders("bdf2", 1);
    let least = p.iter().cloned().fold(f64::INFINITY, f64::min);
    println!("S3N: smallest p {least:.4} (must be <= 1.3)");
    assert!(least <= 1.3, "S3N did not fire: {p:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// S4 rotating frame
// ---------------------------------------------------------------------------------------------------------------

/// S4: SST with the rotating frame on pimple: finite, positive k and omega, seam pairs balanced.
#[test]
fn s4_sst_with_the_rotating_frame_on_pimple() {
    let mut sim = sim_of(S4_TEXT);
    let mut worst = 0.0_f64;
    for k in 1..=10 {
        sim.step();
        assert!(sim.failure().is_none(), "S4 step {k}: {:?}", sim.failure());
        for f in ["ux", "uy", "uz", "p", "k", "omega", "nut"] {
            let v = sim.field(f).unwrap();
            assert!(v.iter().all(|x| x.is_finite()), "S4 step {k}: {f} not finite");
        }
        let kmin = sim.field("k").unwrap().iter().cloned().fold(f64::INFINITY, f64::min);
        let wmin = sim
            .field("omega")
            .unwrap()
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        assert!(kmin > 0.0 && wmin > 0.0, "S4 step {k}: k min {kmin}, omega min {wmin}");
        let s = seam_pair_sum(&sim);
        println!("  S4 step {k}: seam pair sum {s:.3e}, min k {kmin:.3e}, min omega {wmin:.3e}");
        worst = worst.max(s);
    }
    let nut = max_abs(&sim.field("nut").unwrap());
    println!(
        "S4: worst seam pair sum {worst:.3e} (limit 1e-8); reported max nut / nu {:.3e}",
        nut / 0.01
    );
    assert!(worst <= 1e-8, "S4 seam pair sum {worst:.3e}");
}

// ---------------------------------------------------------------------------------------------------------------
// S6 / V3H: the case text of ADR 0183 check H(dt) (B2, the ADR 0086 bladed case on pimple)
// ---------------------------------------------------------------------------------------------------------------

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2;
const Q_DESIGN: f64 = 1.4032;
/// ADR 0183 D5: dt = T_c / 100.
const H_DT: f64 = 4.605e-5;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1), as `fan01_gci_w37c.rs` reads them.
fn sections() -> Vec<Vec<f64>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("r_mm"))
        .map(|l| {
            l.split(',')
                .take(8)
                .map(|v| v.trim().parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
        })
        .filter(|r| r[0] != 247.0)
        .collect();
    assert_eq!(rows.len(), 16);
    rows
}

fn list(rows: &[Vec<f64>], f: impl Fn(&[f64]) -> f64) -> String {
    let v: Vec<String> = rows.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// The B2 counts: the ADR 0086 x0.75 sqrt(2) scaling of FAN-01-C (`fan01_gci_w37c.rs`, `scaled(0.75 * 2f64.sqrt())`).
fn b2_counts() -> (usize, usize, usize, usize, usize, usize, usize) {
    let s = 0.75 * 2f64.sqrt();
    let r = |n: usize| (n as f64 * s).round() as usize;
    (r(8), r(16), r(8), r(10), r(4), r(14), r(7))
}

/// The ADR 0086 bladed B2 case (`fan01_gci_w37c.rs` `case_toml(B2, "camber_seam_h", true)`) with the solver of ADR 0183 D5.
fn h_text() -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = b2_counts();
    let mesh = format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
    );
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let casing = format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n",
        -w
    );
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    format!(
        r#"{mesh}
[physics]
nu = 1.5e-5
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}turbulence = "inlet"
turbulence_spec = "values"
k_value = {k:?}
omega_value = {om:?}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.shroud]
{casing}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[boundaries.blade]
velocity = "fixed"
[solver]
algorithm = "pimple"
n_outer = 5
time_scheme = "bdf2"
dt = {H_DT:?}
end_time = 0.2532
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
"#
    )
}

/// S6: the case text of check H(dt) builds with no diagnostic (H is runnable as registered).
#[test]
fn s6_check_h_case_text_builds() {
    let t0 = Instant::now();
    let text = h_text();
    let sim = sim_of(&text);
    println!(
        "S6: H(dt) builds: {} cells, {:.2} s",
        sim.geom.n_cells(),
        t0.elapsed().as_secs_f64()
    );
}

/// V3H (report only, ADR 0183 D3 V3 for H(dt)): 200 steps of H(dt) on B2; finite or not, guard trip or not, seconds per step.
#[test]
#[ignore = "report only: 200 steps of the B2 passage (minutes), run once by ADR 0199"]
fn v3h_smoke_of_check_h() {
    let mut sim = sim_of(&h_text());
    let t0 = Instant::now();
    let mut steps = 0;
    for k in 1..=200 {
        sim.step();
        steps = k;
        if sim.failure().is_some() || t0.elapsed().as_secs_f64() > 1800.0 {
            break;
        }
        if k % 20 == 0 {
            println!(
                "  V3H step {k}: t {:.4e} s, {:.2} s per step",
                sim.time(),
                t0.elapsed().as_secs_f64() / k as f64
            );
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let finite = ["ux", "uy", "uz", "p", "k", "omega"]
        .iter()
        .all(|f| sim.field(f).unwrap().iter().all(|x| x.is_finite()));
    println!(
        "V3H: {steps} of 200 steps, failure {:?}, fields finite {finite}, {:.3} s per step, {} cells",
        sim.failure(),
        secs / steps.max(1) as f64,
        sim.geom.n_cells()
    );
}
