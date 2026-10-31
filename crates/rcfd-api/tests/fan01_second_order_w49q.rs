//! Job 49Q (ADR 0180): a face-bounded second-order convection scheme for the FAN-01 bladed stall. From the stalled state at
//! iteration 1000 the momentum convection is switched to an existing TVD scheme (`tvd_minmod`, `tvd_van_albada`, or
//! `tvd_van_leer` as the control), with no relaxation, mask or selector, and run 2000 more iterations, on B3 and then on
//! B2 (held out). No solver change. Registered in ADR 0180 before any run. Mesh and case text as `fan01_auto_fallback_w49p.rs`.
//!
//! `W49Q_OUT=<dir> cargo test --release -p rcfd-api --test fan01_second_order_w49q <arm> -- --ignored --exact --nocapture`.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use serde_json::{json, Value};
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const Q_DESIGN: f64 = 1.4032;
const N_BLADES: f64 = 9.0;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

// ---------------------------------------------------------------- mesh and case (copied from fan01_auto_fallback_w49p.rs; the scheme lines are a parameter)

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

type Counts = (usize, usize, usize, usize, usize, usize, usize);
const F3: Counts = (6, 12, 6, 8, 3, 10, 5);
const F2: Counts = (8, 17, 8, 11, 4, 15, 7);

fn mesh_toml(c: Counts, blade: bool) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n{}",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
        if blade { "" } else { "include_blade = false\n" },
    )
}

fn case_toml(c: Counts, blade: bool, conv: Option<&str>) -> String {
    let outlet = "velocity = \"zero_gradient\"
pressure = \"fixed\"
";
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let wall_t = "turbulence = \"wall\"\n";
    let casing = format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\n{wall_t}",
        -w
    );
    let hub = if blade {
        "velocity = \"fixed\"\n".to_string()
    } else {
        casing.clone()
    };
    let blade_t = if blade {
        "[boundaries.blade]\nvelocity = \"fixed\"\n[reports.m_blade]\ntype = \"moment\"\npatch = \"blade\"\ndirection = [0.0, 0.0, 1.0]\n"
    } else {
        ""
    };
    let turb = "[turbulence]\nmodel = \"k_omega_sst\"\n".to_string();
    let inlet_t =
        format!("turbulence = \"inlet\"\nturbulence_spec = \"values\"\nk_value = {k:?}\nomega_value = {om:?}\n");
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    let solver_extra = "";
    // None: the ADR 0086 scheme lines unchanged; Some(v): `convection = v` and no limiter line (sim.rs refuses the
    // limiter with a TVD scheme)
    let scheme = match conv {
        None => "convection = \"linear_upwind\"\nlinear_upwind_limiter = \"barth_jespersen\"".to_string(),
        Some(v) => format!("convection = \"{v}\""),
    };
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = {RHO:?}
{turb}[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}{inlet_t}[boundaries.outlet]
{outlet}[boundaries.hub]
{hub}[boundaries.shroud]
{casing}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
{blade_t}[solver]
algorithm = "simplec"
{scheme}
max_iterations = 100000
tolerance_u = 1e-5
tolerance_p = 1e-5
{solver_extra}
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
[reports.p_in]
type = "area_average"
patch = "inlet"
field = "p"
[reports.p_out]
type = "area_average"
patch = "outlet"
field = "p"
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
[reports.m_shroud]
type = "moment"
patch = "shroud"
direction = [0.0, 0.0, 1.0]
"#,
        mesh_toml(c, blade)
    )
}

fn sim_for(c: Counts, conv: Option<&str>) -> Simulation {
    Simulation::new(&Case::parse_toml(&case_toml(c, true, conv)).unwrap()).unwrap()
}

/// Bladed (torque [N m], Delta p_ts [Pa]) as ADR 0086.
fn functionals(sim: &Simulation) -> (f64, f64) {
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    (
        -N_BLADES * (mon("m_blade") + mon("m_hub")),
        RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
    )
}

fn speed_change(a: &[Vec3], b: &[Vec3]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| vec3::norm(vec3::sub(*x, *y))).collect()
}

fn l2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        f64::NAN
    } else {
        v[v.len() / 2]
    }
}

fn rz(sim: &Simulation, c: usize) -> (f64, f64) {
    let x = sim.geom.mesh.cell_centres()[c];
    (x[0].hypot(x[1]), x[2])
}

/// The `n` cells with the largest values of `d`, largest first.
fn top(d: &[f64], n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..d.len()).collect();
    idx.sort_by(|a, b| d[*b].partial_cmp(&d[*a]).unwrap_or(std::cmp::Ordering::Equal));
    idx.truncate(n);
    idx
}

// ---------------------------------------------------------------- arms (ADR 0180, fixed before any run)

const N0: usize = 1000;
const N1: usize = 2000;
const W: usize = 500;
const FLOOR_T: f64 = 0.002;
const FLOOR_P: f64 = 0.005;
const T_MEAS: f64 = 2.142;
const P_MEAS: f64 = 126.5;

fn scheme_of(arm: &str) -> (ConvScheme, &'static str) {
    match arm {
        "T-MM" => (ConvScheme::Tvd(TvdLimiter::Minmod), "tvd_minmod"),
        "T-VA" => (ConvScheme::Tvd(TvdLimiter::VanAlbada), "tvd_van_albada"),
        "T-VL" => (ConvScheme::Tvd(TvdLimiter::VanLeer), "tvd_van_leer"),
        _ => panic!("unknown arm {arm}"),
    }
}

type Captured = Arc<Mutex<Vec<(usize, Vec<f64>, Vec<Vec3>)>>>;

/// Record K (report only): kappa_f = psi g_U . (x_f - x_U) / (phi_D - phi_U) of the Barth-Jespersen `linear_upwind` face
/// value, from the limited gradients the kernel used in this iteration (captured by the hook, left unchanged).
fn record_k(sim: &Simulation, flux: &[f64], cap: &[(usize, Vec<f64>, Vec<Vec3>)], hot: &[usize]) -> Value {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let (cc, cf) = (mesh.cell_centres(), mesh.face_centres());
    let mut is_hot = vec![false; mesh.n_cells()];
    for &c in hot {
        is_hot[c] = true;
    }
    // counts: [n, kappa > 1/2, kappa > 1, kappa < 0] over all faces and over the hot cells' faces
    let (mut all, mut hotc) = ([0usize; 4], [0usize; 4]);
    for (_k, cells, grad) in cap {
        for f in 0..g.n_internal {
            let (o, n) = (own[f], nei[f]);
            let (u, d) = if flux[f] >= 0.0 { (o, n) } else { (n, o) };
            let diff = cells[d] - cells[u];
            if diff.abs() <= 1e-3 {
                continue;
            }
            let kappa = vec3::dot(grad[u], vec3::sub(cf[f], cc[u])) / diff;
            let flags = [true, kappa > 0.5, kappa > 1.0, kappa < 0.0];
            for (i, fl) in flags.iter().enumerate() {
                if *fl {
                    all[i] += 1;
                    if is_hot[o] || is_hot[n] {
                        hotc[i] += 1;
                    }
                }
            }
        }
    }
    let share = |c: [usize; 4], i: usize| c[i] as f64 / c[0].max(1) as f64;
    json!({
        "components": cap.len(),
        "all_faces": {"n": all[0], "gt_half": share(all, 1), "gt_one": share(all, 2), "lt_zero": share(all, 3)},
        "top20_faces": {"n": hotc[0], "gt_half": share(hotc, 1), "gt_one": share(hotc, 2), "lt_zero": share(hotc, 3)},
        "QK": share(hotc, 2) > share(all, 2),
    })
}

fn arm_run(arm: &str, mesh_id: &str, budget_s: f64) -> Value {
    let t0 = Instant::now();
    let id = format!("{arm}-{mesh_id}");
    let (scheme, case_value) = scheme_of(arm);
    let mut sim = sim_for(if mesh_id == "B3" { F3 } else { F2 }, None);
    let n = sim.geom.mesh.n_cells();
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut f_ref: Vec<(f64, f64)> = Vec::new();
    let mut prev = sim.velocity().values.clone();
    let (mut du_switch, mut f_switch) = (f64::NAN, (f64::NAN, f64::NAN));
    let mut k_rec = Value::Null;
    let mut d_prev_switch: Vec<f64> = Vec::new();
    let (mut reached, mut stopped, mut budget_hit) = (None, None, false);
    for it in 1..=(N0 + N1) {
        if it == N0 + 1 {
            let d = speed_change(&sim.velocity().values, &prev);
            du_switch = l2(&d);
            d_prev_switch = d;
            f_switch = functionals(&sim);
            sim.simple_solver_mut().unwrap().settings.convection = scheme;
        }
        // record K: hook on in iteration N0 only (B3), reading the limited gradient without changing it
        let cap: Captured = Arc::new(Mutex::new(Vec::new()));
        let mut flux_k: Vec<f64> = Vec::new();
        if it == N0 && mesh_id == "B3" {
            let s = sim.simple_solver_mut().unwrap();
            flux_k = match &s.conv_flux_offset {
                Some(o) => s.phi.iter().zip(o).map(|(p, o)| p - o).collect(),
                None => s.phi.clone(),
            };
            let c2 = cap.clone();
            s.conv_gradient_hook = Some(Box::new(move |k, cells, _b, v| {
                c2.lock().unwrap().push((k, cells.to_vec(), v.to_vec()));
            }));
        }
        prev = sim.velocity().values.clone();
        let st = sim.step();
        if it == N0 && mesh_id == "B3" {
            sim.simple_solver_mut().unwrap().conv_gradient_hook = None;
            let d = speed_change(&sim.velocity().values, &prev);
            let hot = top(&d, 20);
            k_rec = record_k(&sim, &flux_k, &cap.lock().unwrap(), &hot);
            println!("W49Q {id} record K {k_rec}");
        }
        hist.push((st.res_u, st.res_p));
        if it > N0 - W && it <= N0 {
            f_ref.push(functionals(&sim));
        }
        if it > N0 && reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            println!("W49Q {id} STOP at {it}");
            break;
        }
        if t0.elapsed().as_secs_f64() > budget_s {
            budget_hit = true;
            stopped = Some(it);
            println!("W49Q {id} BUDGET HIT at {it}");
            break;
        }
        if it % 250 == 0 {
            let f = functionals(&sim);
            println!(
                "W49Q {id} it {it}: res_u {:.3e} res_p {:.3e} torque {:.4} dpts {:.3} ({:.1} s)",
                st.res_u,
                st.res_p,
                f.0,
                f.1,
                t0.elapsed().as_secs_f64()
            );
        }
    }
    let nh = hist.len();
    let win = |a: usize, b: usize| -> (f64, f64) {
        let (a, b) = (a.min(nh), b.min(nh));
        (
            median(hist[a..b].iter().map(|h| h.0).collect()),
            median(hist[a..b].iter().map(|h| h.1).collect()),
        )
    };
    let ref_res = win(N0 - W, N0);
    let arm_res = win(nh.saturating_sub(W).max(N0), nh);
    let d_end = speed_change(&sim.velocity().values, &prev);
    let du_end = l2(&d_end);
    let rho_du = du_end / du_switch;
    let rho_med = (arm_res.0 / ref_res.0).max(arm_res.1 / ref_res.1);
    let f_end = functionals(&sim);
    let t_mean = f_ref.iter().map(|f| f.0).sum::<f64>() / f_ref.len().max(1) as f64;
    let p_mean = f_ref.iter().map(|f| f.1).sum::<f64>() / f_ref.len().max(1) as f64;
    let d_t = (f_end.0 - t_mean).abs() / t_mean.abs();
    let d_p = (f_end.1 - p_mean).abs() / p_mean.abs();
    let converged = sim.is_converged();
    let ok = stopped.is_none();
    let top_end: Vec<Value> = top(&d_end, 5)
        .into_iter()
        .map(|c| {
            let (r, z) = rz(&sim, c);
            json!({"cell": c, "du": d_end[c], "r": r, "z": z})
        })
        .collect();
    let top_switch: Vec<usize> = if d_prev_switch.is_empty() {
        Vec::new()
    } else {
        top(&d_prev_switch, 5)
    };
    let last_res = hist.last().copied().unwrap_or((f64::NAN, f64::NAN));
    let rec = json!({
        "adr": "0180", "arm": arm, "id": id, "mesh": mesh_id, "n_cells": n, "case_value": case_value,
        "ref_median_res_u": ref_res.0, "ref_median_res_p": ref_res.1,
        "arm_median_res_u": arm_res.0, "arm_median_res_p": arm_res.1,
        "end_res_u": last_res.0, "end_res_p": last_res.1,
        "res_nut_end": sim.monitors.last("res_nut").unwrap_or(f64::NAN),
        "rho_med": rho_med, "du_switch": du_switch, "du_end": du_end, "rho_du": rho_du,
        "reached": reached, "stopped": stopped, "budget_hit": budget_hit, "is_converged_end": converged,
        "torque_switch": f_switch.0, "dpts_switch": f_switch.1, "torque_end": f_end.0, "dpts_end": f_end.1,
        "torque_ref_mean": t_mean, "dpts_ref_mean": p_mean, "d_torque": d_t, "d_dpts": d_p,
        "torque_vs_nh3": (f_end.0 - 1.9144) / 1.9144, "dpts_vs_nh3": (f_end.1 - 45.753) / 45.753,
        "torque_gap_measured": (f_end.0 - T_MEAS) / T_MEAS, "dpts_gap_measured": (f_end.1 - P_MEAS) / P_MEAS,
        "top5_end": top_end, "top5_switch": top_switch, "record_k": k_rec,
        "limits": {"C1": ok && reached.is_some(), "C2": ok && converged, "C3": ok && rho_du <= 0.01},
        "seconds": t0.elapsed().as_secs_f64(),
    });
    println!("W49Q {id} RESULT {}", serde_json::to_string(&rec).unwrap());
    println!(
        "W49Q {id} reached {reached:?} conv {converged} rho_med {rho_med:.3e} rho_du {rho_du:.3e} torque {:.4} (ref mean {t_mean:.4}, D {d_t:.4}) dpts {:.3} (ref mean {p_mean:.3}, D {d_p:.4}); limits {}",
        f_end.0,
        f_end.1,
        rec["limits"]
    );
    if let Ok(dir) = std::env::var("W49Q_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        let p = std::path::Path::new(&dir).join(format!("{id}.json"));
        std::fs::write(p, serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    }
    if mesh_id == "B3" {
        let (u, p, t) = (ref_res.0, ref_res.1, f_switch.0);
        assert!(
            n == 4920
                && format!("{u:.3e}") == "1.089e-3"
                && format!("{p:.2e}") == "4.87e-4"
                && format!("{t:.4}") == "1.9177",
            "V1 failed: n {n} median res_u {u:.3e} res_p {p:.2e} torque {t:.4}"
        );
    } else {
        let (u, p) = (ref_res.0, ref_res.1);
        assert!(
            n == 13627
                && format!("{u:.3e}") == "1.004e-3"
                && format!("{p:.2e}") == "6.23e-4"
                && format!("{t_mean:.4}") == "2.0308"
                && format!("{p_mean:.3}") == "63.860",
            "V2 failed: n {n} median res_u {u:.3e} res_p {p:.2e} torque mean {t_mean:.4} dpts mean {p_mean:.3}"
        );
    }
    rec
}

fn read(id: &str) -> Value {
    let dir = std::env::var("W49Q_OUT").expect("W49Q_OUT");
    let p = std::path::Path::new(&dir).join(format!("{id}.json"));
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn assert_limits(r: &Value, keys: &[&str]) {
    let missed: Vec<&&str> = keys.iter().filter(|k| r["limits"][**k] != json!(true)).collect();
    assert!(missed.is_empty(), "{}: missed {missed:?}", r["id"]);
}

/// RN and RS of an arm from its B3 and B2 records (ADR 0180).
fn refinement(arm: &str) -> (bool, Option<bool>, Value) {
    let (b3, b2) = (read(&format!("{arm}-B3")), read(&format!("{arm}-B2")));
    let g = |r: &Value, k: &str| r[k].as_f64().unwrap_or(f64::NAN);
    let (t3, t2, p3, p2) = (
        g(&b3, "d_torque"),
        g(&b2, "d_torque"),
        g(&b3, "d_dpts"),
        g(&b2, "d_dpts"),
    );
    let rn = t2 <= t3.max(FLOOR_T) && p2 <= p3.max(FLOOR_P);
    // RS: NOT EVALUABLE (None) if a B3 difference is at or below its floor
    let rs = if t3 <= FLOOR_T || p3 <= FLOOR_P {
        None
    } else {
        Some(t2 < t3 && p2 < p3)
    };
    let r32: f64 = 1.4017;
    let rec = json!({"arm": arm, "D_T_B3": t3, "D_T_B2": t2, "D_P_B3": p3, "D_P_B2": p2,
        "ratio_T": t2 / t3, "ratio_P": p2 / p3, "order_T": (t3 / t2).ln() / r32.ln(), "order_P": (p3 / p2).ln() / r32.ln(),
        "RN": rn, "RS": rs});
    println!("W49Q refinement {rec}");
    if let Ok(dir) = std::env::var("W49Q_OUT") {
        let p = std::path::Path::new(&dir).join(format!("{arm}-refinement.json"));
        std::fs::write(p, serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    }
    (rn, rs, rec)
}

/// Acceptance of an arm (C1-C3 on both meshes and RN), from the records.
fn accepted(arm: &str) -> bool {
    let ok = |id: String| {
        let r = read(&id);
        ["C1", "C2", "C3"].iter().all(|k| r["limits"][*k] == json!(true))
    };
    ok(format!("{arm}-B3")) && ok(format!("{arm}-B2")) && refinement(arm).0
}

// B3 arms (Q1, Q3 B3 part, Q4); budget 10 min each.

#[test]
#[ignore = "known miss (ADR 0180 Q1: C1, C2, C3): T-MM on B3 keeps a cycle at the hub cells r 0.125, z 0.13-0.17 (rho_med 2.20, rho_du 0.85, res_u 2.2e-3) (about 2 min in release; budget 10 min)"]
fn t_mm_b3_known_miss() {
    let r = arm_run("T-MM", "B3", 600.0);
    assert_limits(&r, &["C1", "C2", "C3"]);
}

#[test]
#[ignore = "known miss (ADR 0180 Q3: C1, C2, C3): T-VA on B3 falls to res_u 4.5e-4 and keeps a cycle at the shroud tip-layer cells z 0.12 (rho_med 0.52, rho_du 0.31) (about 2 min in release; budget 10 min)"]
fn t_va_b3_known_miss() {
    let r = arm_run("T-VA", "B3", 600.0);
    assert_limits(&r, &["C1", "C2", "C3"]);
}

/// Q4 predicts that T-VL misses C3 on B3; the test asserts the acceptance limits of the arm, as for the others.
#[test]
#[ignore = "known miss (ADR 0180 C1, C2, C3; Q4 held): T-VL on B3 keeps a 3-5 m/s cycle at the hub cells r 0.126, z 0.10-0.12 (rho_med 4.88, rho_du 3.39) (about 2 min in release; budget 10 min)"]
fn t_vl_b3_known_miss() {
    let r = arm_run("T-VL", "B3", 600.0);
    assert_limits(&r, &["C1", "C2", "C3"]);
}

// B2 arms (held out: run only after the three B3 records are committed); budget 30 min each. Each also checks RN.

fn b2_arm(arm: &str) {
    let r = arm_run(arm, "B2", 1800.0);
    let (rn, rs, _) = refinement(arm);
    println!("W49Q {arm} RN {rn} RS {rs:?}");
    assert_limits(&r, &["C1", "C2", "C3"]);
    assert!(rn, "{arm}: RN missed");
}

#[test]
#[ignore = "known miss (ADR 0180 Q2: C1, C2, C3, RN): T-MM on B2 cycles at the hub cells z 0.17 (rho_du 1.41); D_T 0.0073 -> 0.0139 grows (about 5 min in release; budget 30 min; needs T-MM-B3 in W49Q_OUT)"]
fn t_mm_b2_known_miss() {
    b2_arm("T-MM");
}

#[test]
#[ignore = "known miss (ADR 0180 Q3: C1, C2, C3, RN): T-VA on B2 cycles at the hub cells z 0.17 (rho_du 1.48); D_T 0.0035 -> 0.0117 grows (about 5 min in release; budget 30 min; needs T-VA-B3 in W49Q_OUT)"]
fn t_va_b2_known_miss() {
    b2_arm("T-VA");
}

#[test]
#[ignore = "known miss (ADR 0180 C1, C2, C3; RN met on cycle samples): T-VL on B2 cycles at the hub cells z 0.17 (rho_du 1.39) (about 5 min in release; budget 30 min; needs T-VL-B3 in W49Q_OUT)"]
fn t_vl_b2_known_miss() {
    b2_arm("T-VL");
}

/// The choice rule: the least dissipative accepted arm, in the order T-VL, T-VA, T-MM.
fn recommended() -> Option<&'static str> {
    ["T-VL", "T-VA", "T-MM"].into_iter().find(|a| accepted(a))
}

/// Report-only run from the start through the case value of the recommended arm (only if reading rule 1 applies).
#[test]
#[ignore = "ADR 0180 report-only B3 run from the start with the recommended case value; NOT RUN: no arm was accepted, so reading rule 1 does not apply (about 2.5 min; budget 10 min)"]
fn from_start_b3() {
    let Some(arm) = recommended() else {
        panic!("not registered: no arm accepted (reading rule 1 does not apply)");
    };
    let (_, case_value) = scheme_of(arm);
    let t0 = Instant::now();
    let mut sim = sim_for(F3, Some(case_value));
    let mut reached = None;
    let mut stopped = None;
    for it in 1..=(N0 + N1) {
        let st = sim.step();
        if reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            break;
        }
        if t0.elapsed().as_secs_f64() > 600.0 {
            stopped = Some(it);
            println!("W49Q from-start BUDGET HIT at {it}");
            break;
        }
        if it % 250 == 0 {
            println!("W49Q from-start it {it}: res_u {:.3e} res_p {:.3e}", st.res_u, st.res_p);
        }
    }
    let f = functionals(&sim);
    let sw = read(&format!("{arm}-B3"));
    let rec = json!({"adr": "0180", "id": "from-start-B3", "arm": arm, "case_value": case_value,
        "reached": reached, "stopped": stopped, "is_converged_end": sim.is_converged(),
        "torque_end": f.0, "dpts_end": f.1,
        "torque_switch_arm_end": sw["torque_end"], "dpts_switch_arm_end": sw["dpts_end"],
        "seconds": t0.elapsed().as_secs_f64()});
    println!("W49Q from-start RESULT {rec}");
    if let Ok(dir) = std::env::var("W49Q_OUT") {
        let p = std::path::Path::new(&dir).join("from-start-B3.json");
        std::fs::write(p, serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    }
}

/// The TVD case values build on B3 (fast smoke of the case text; no iteration).
#[test]
fn w49q_case_values_build() {
    for arm in ["T-MM", "T-VA", "T-VL"] {
        let (scheme, v) = scheme_of(arm);
        let sim = sim_for(F3, Some(v));
        assert_eq!(sim.simple_solver().unwrap().settings.convection, scheme);
    }
}
