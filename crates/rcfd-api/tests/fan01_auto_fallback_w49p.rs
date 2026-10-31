//! Job 49P (ADR 0179): an adaptive first-order fallback for the cells that sustain the FAN-01 stall. From the stalled state
//! at iteration 1000, F-R(0.25) (ADR 0177) is switched on and, every 50 iterations, the convection deferred correction is
//! removed (ADR 0175 hook) in the cells whose iterate keeps oscillating (largest speed change over the round at least
//! theta times the largest, and at least 1 % of the stall's), grown by one face ring, capped at 10 % of the cells. The
//! selector uses the solution only (no cell ids, no coordinates). Registered in ADR 0179 before any run; B2 is held out.
//!
//! `W49P_OUT=<dir> cargo test --release -p rcfd-api --test fan01_auto_fallback_w49p <arm> -- --ignored --exact --nocapture`.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::simple::HighOrderMask;
use serde_json::{json, Value};
use std::f64::consts::PI;
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

// ---------------------------------------------------------------- mesh and case (copied from fan01_stall_w43c.rs unchanged)

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

fn case_toml(c: Counts, blade: bool) -> String {
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
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
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

fn b3() -> Simulation {
    Simulation::new(&Case::parse_toml(&case_toml(F3, true)).unwrap()).unwrap()
}

fn b2() -> Simulation {
    Simulation::new(&Case::parse_toml(&case_toml(F2, true)).unwrap()).unwrap()
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

// ---------------------------------------------------------------- seeds and rings (ADR 0175 definitions)

const S_HUB: [usize; 5] = [1234, 365, 401, 1250, 1233];

/// Face adjacency: internal faces, and periodic seam faces linking their owner to the owner of the partner face.
fn adjacency(sim: &Simulation) -> Vec<Vec<usize>> {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut adj = vec![Vec::new(); mesh.n_cells()];
    for f in 0..g.n_internal {
        adj[own[f]].push(nei[f]);
        adj[nei[f]].push(own[f]);
    }
    for (pi, patch) in mesh.patches().iter().enumerate() {
        if let Some(map) = &g.periodic_partner[pi] {
            for (j, f) in patch.faces().enumerate() {
                adj[own[f]].push(own[map[j]]);
            }
        }
    }
    adj
}

/// X_k: the seeds grown k times by face neighbours.
fn ring(sim: &Simulation, seeds: &[usize], k: usize) -> Vec<bool> {
    let adj = adjacency(sim);
    let mut r = vec![false; sim.geom.mesh.n_cells()];
    for &c in seeds {
        r[c] = true;
    }
    for _ in 0..k {
        let prev = r.clone();
        for (c, &inside) in prev.iter().enumerate() {
            if inside {
                for &d in &adj[c] {
                    r[d] = true;
                }
            }
        }
    }
    r
}

fn count(r: &[bool]) -> usize {
    r.iter().filter(|&&b| b).count()
}

// ---------------------------------------------------------------- the adaptive selector (ADR 0179, fixed before any run)

const N0: usize = 1000;
const N1: usize = 2000;
const W: usize = 500;
const ROUND: usize = 50;
const GROW: usize = 1;
const FLOOR: f64 = 0.01;
const CAP: f64 = 0.10;

/// ADR 0178's hand-picked hub seeds, used only to report the overlap after the runs (never by the selector).
const S_H_EXTRA: [usize; 14] = [907, 906, 581, 905, 582, 149, 437, 150, 438, 402, 113, 114, 77, 257];

/// The ADR 0174 wall-cell metric of every cell (candidate 1; reported only).
fn metric(sim: &Simulation) -> Vec<f64> {
    let mesh = &sim.geom.mesh;
    let n = mesh.n_cells();
    let q = rcfd_mesh::quality::compute_metrics(mesh);
    (0..n)
        .map(|c| {
            let th = mesh
                .cell_faces(c)
                .iter()
                .map(|&f| q.non_orthogonality[f])
                .fold(0.0_f64, f64::max)
                .min(89.9);
            let v = q.aspect_ratio[c] * th.to_radians().tan();
            if v.is_nan() {
                0.0
            } else {
                v
            }
        })
        .collect()
}

/// Median percentile rank (in all cells) of the metric over the cells of `set`.
fn median_rank(m: &[f64], set: &[bool]) -> f64 {
    let mut sorted = m.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = m.len() as f64;
    let ranks: Vec<f64> = (0..m.len())
        .filter(|&c| set[c])
        .map(|c| sorted.partition_point(|x| *x <= m[c]) as f64 / n)
        .collect();
    median(ranks)
}

fn selector_arm(id: &str, mesh_id: &str, theta: f64, budget_s: f64) -> Value {
    let t0 = Instant::now();
    let mut sim = if mesh_id == "B3" { b3() } else { b2() };
    let n = sim.geom.mesh.n_cells();
    let adj = adjacency(&sim);
    let mut mask = vec![false; n];
    let mut cap_hit = false;
    let mut rounds: Vec<Value> = Vec::new();
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut t_ref: Vec<(f64, f64)> = Vec::new();
    let mut prev = sim.velocity().values.clone();
    let (mut du_switch, mut a0, mut f_switch) = (f64::NAN, f64::NAN, (f64::NAN, f64::NAN));
    let mut a_round = vec![0.0f64; n];
    let (mut reached, mut stopped, mut budget_hit) = (None, None, false);
    for it in 1..=(N0 + N1) {
        if it == N0 + 1 {
            let d = speed_change(&sim.velocity().values, &prev);
            du_switch = l2(&d);
            a0 = d.iter().cloned().fold(0.0, f64::max);
            f_switch = functionals(&sim);
            sim.simple_solver_mut()
                .unwrap()
                .deferred_correction_relaxation_test_only = Some(0.25);
        }
        prev = sim.velocity().values.clone();
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it > N0 - W && it <= N0 {
            t_ref.push(functionals(&sim));
        }
        if it > N0 {
            let d = speed_change(&sim.velocity().values, &prev);
            for (a, x) in a_round.iter_mut().zip(&d) {
                *a = a.max(*x);
            }
            if reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
                reached = Some(it);
            }
            if (it - N0) % ROUND == 0 {
                let big_a = a_round.iter().cloned().fold(0.0, f64::max);
                let thr = (theta * big_a).max(FLOOR * a0);
                let mut add = vec![false; n];
                for c in 0..n {
                    if a_round[c] >= thr && !mask[c] {
                        add[c] = true;
                    }
                }
                for _ in 0..GROW {
                    let base = add.clone();
                    for c in 0..n {
                        if base[c] {
                            for &d2 in &adj[c] {
                                add[d2] = true;
                            }
                        }
                    }
                }
                let new: usize = (0..n).filter(|&c| add[c] && !mask[c]).count();
                let size = count(&mask);
                let mut added = 0;
                if !cap_hit && new > 0 {
                    if (size + new) as f64 > CAP * n as f64 {
                        cap_hit = true;
                    } else {
                        for c in 0..n {
                            if add[c] {
                                mask[c] = true;
                            }
                        }
                        added = new;
                        sim.simple_solver_mut().unwrap().high_order_mask_test_only = Some(HighOrderMask {
                            conv: mask.clone(),
                            lap: vec![false; n],
                        });
                    }
                }
                rounds.push(json!({"iteration": it, "A": big_a, "threshold": thr, "added": added,
                                   "mask_cells": count(&mask), "cap_hit": cap_hit, "res_u": st.res_u, "res_p": st.res_p}));
                a_round = vec![0.0; n];
            }
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            println!("W49P {id} STOP at {it}");
            break;
        }
        if t0.elapsed().as_secs_f64() > budget_s {
            budget_hit = true;
            stopped = Some(it);
            println!("W49P {id} BUDGET HIT at {it}");
            break;
        }
        if it % 250 == 0 {
            println!(
                "W49P {id} it {it}: res_u {:.3e} res_p {:.3e} mask {} ({:.1} s)",
                st.res_u,
                st.res_p,
                count(&mask),
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
    let arm_res = win(nh.saturating_sub(W), nh);
    let d_end = speed_change(&sim.velocity().values, &prev);
    let du_end = l2(&d_end);
    let rho_du = du_end / du_switch;
    let rho_med = (arm_res.0 / ref_res.0).max(arm_res.1 / ref_res.1);
    let f_end = functionals(&sim);
    let t_mean = t_ref.iter().map(|f| f.0).sum::<f64>() / t_ref.len().max(1) as f64;
    let p_mean = t_ref.iter().map(|f| f.1).sum::<f64>() / t_ref.len().max(1) as f64;
    let delta = (f_end.0 - t_mean).abs() / t_mean.abs();
    let converged = sim.is_converged();
    let share = count(&mask) as f64 / n as f64;
    let tot2 = d_end.iter().map(|x| x * x).sum::<f64>().max(1e-300);
    let share_du_mask = (0..n).filter(|&c| mask[c]).map(|c| d_end[c] * d_end[c]).sum::<f64>() / tot2;
    let ok = stopped.is_none();
    let mut rec = json!({
        "adr": "0179", "arm": id, "mesh": mesh_id, "n_cells": n, "theta": theta,
        "ref_median_res_u": ref_res.0, "ref_median_res_p": ref_res.1,
        "arm_median_res_u": arm_res.0, "arm_median_res_p": arm_res.1,
        "rho_med": rho_med, "du_switch": du_switch, "du_end": du_end, "rho_du": rho_du, "a0": a0,
        "reached": reached, "stopped": stopped, "budget_hit": budget_hit, "is_converged_end": converged,
        "mask_cells": count(&mask), "mask_share": share, "cap_hit": cap_hit, "share_du_in_mask_end": share_du_mask,
        "torque_switch": f_switch.0, "dpts_switch": f_switch.1, "torque_end": f_end.0, "dpts_end": f_end.1,
        "torque_ref_mean": t_mean, "dpts_ref_mean": p_mean, "delta_torque": delta,
        "dpts_rel_diff": (f_end.1 - p_mean).abs() / p_mean.abs(),
        "metric_median_rank_mask": median_rank(&metric(&sim), &mask),
        "limits": {"C1": ok && reached.is_some(), "C2": ok && converged, "C3": ok && rho_du <= 0.01,
                   "S": ok && share <= CAP && !cap_hit},
        "rounds": rounds,
        "seconds": t0.elapsed().as_secs_f64(),
    });
    if mesh_id == "B3" {
        let mut seeds: Vec<usize> = S_HUB.iter().chain(S_H_EXTRA.iter()).copied().collect();
        seeds.sort_unstable();
        seeds.dedup();
        let (h3, h4) = (ring(&sim, &seeds, 3), ring(&sim, &seeds, 4));
        let inter = |r: &[bool]| (0..n).filter(|&c| mask[c] && r[c]).count();
        rec["overlap_h3"] = json!(inter(&h3));
        rec["overlap_h4"] = json!(inter(&h4));
        rec["h3_cells"] = json!(count(&h3));
        let h = (f_end.0 - 1.9144).abs() <= 0.01 * 1.9144 && (f_end.1 - 45.753).abs() <= 0.02 * 45.753;
        rec["limits"]["H"] = json!(ok && h);
    }
    println!("W49P {id} RESULT {}", serde_json::to_string(&rec).unwrap());
    println!(
        "W49P {id} reached {reached:?} conv {converged} rho_du {rho_du:.3e} mask {} ({:.2} %) cap {cap_hit} torque {:.4} (ref mean {t_mean:.4}, delta {delta:.4}) dpts {:.3} (ref mean {p_mean:.3}); limits {}",
        count(&mask),
        100.0 * share,
        f_end.0,
        f_end.1,
        rec["limits"]
    );
    if let Ok(dir) = std::env::var("W49P_OUT") {
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
        assert!(
            n == 13627 && ref_res.0 >= 1e-4,
            "V2 failed: n {n} median res_u {:.3e}",
            ref_res.0
        );
    }
    rec
}

fn assert_limits(r: &Value, keys: &[&str]) {
    let missed: Vec<&&str> = keys.iter().filter(|k| r["limits"][**k] != json!(true)).collect();
    assert!(missed.is_empty(), "missed: {missed:?}");
}

fn read(id: &str) -> Value {
    let dir = std::env::var("W49P_OUT").expect("W49P_OUT");
    let p = std::path::Path::new(&dir).join(format!("{id}.json"));
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))).unwrap()
}

fn b3_ok(r: &Value) -> bool {
    ["C1", "C2", "C3", "S"].iter().all(|k| r["limits"][*k] == json!(true))
}

// ---------------------------------------------------------------- arms

/// S-B3(0.1), the a-priori arm (Q1: C1, C2, C3, S, H).
#[test]
#[ignore = "ADR 0179 S-B3(0.1) (about 2.5 min in release; budget 10 min)"]
fn s_b3_010() {
    let r = selector_arm("S-B3-0.1", "B3", 0.1, 600.0);
    assert_limits(&r, &["C1", "C2", "C3", "S", "H"]);
}

/// Conditional calibration arm S-B3(0.3) (run only if S-B3(0.1) misses C1, C2, C3 or S).
#[test]
#[ignore = "ADR 0179 S-B3(0.3), conditional (about 2.5 min in release; budget 10 min)"]
fn s_b3_030() {
    assert!(!b3_ok(&read("S-B3-0.1")), "not registered: S-B3(0.1) met C1-C3 and S");
    let r = selector_arm("S-B3-0.3", "B3", 0.3, 600.0);
    assert_limits(&r, &["C1", "C2", "C3", "S"]);
}

/// Conditional calibration arm S-B3(0.03).
#[test]
#[ignore = "ADR 0179 S-B3(0.03), conditional (about 2.5 min in release; budget 10 min)"]
fn s_b3_003() {
    assert!(!b3_ok(&read("S-B3-0.1")), "not registered: S-B3(0.1) met C1-C3 and S");
    let r = selector_arm("S-B3-0.03", "B3", 0.03, 600.0);
    assert_limits(&r, &["C1", "C2", "C3", "S"]);
}

/// theta for B2 by the registered rule: 0.1 without calibration; otherwise the passing theta with the smallest share, or 0.1.
fn theta_b2() -> f64 {
    let a = read("S-B3-0.1");
    if b3_ok(&a) {
        return 0.1;
    }
    let mut best: Option<(f64, f64)> = None;
    for (id, th) in [("S-B3-0.3", 0.3), ("S-B3-0.1", 0.1), ("S-B3-0.03", 0.03)] {
        let r = read(id);
        if b3_ok(&r) {
            let s = r["mask_share"].as_f64().unwrap();
            if best.is_none_or(|b| s < b.1) {
                best = Some((th, s));
            }
        }
    }
    best.map_or(0.1, |b| b.0)
}

/// S-B2 (held out; Q2: C1, C2, C3, S; Q3: R1 against the B3 arm of the same theta).
#[test]
#[ignore = "ADR 0179 S-B2 (about 8 min in release; budget 30 min; needs the B3 records in W49P_OUT)"]
fn s_b2() {
    let th = theta_b2();
    println!("W49P theta for B2: {th}");
    let r = selector_arm("S-B2", "B2", th, 1800.0);
    let b3 = read(&format!("S-B3-{th}"));
    let (d3, d2) = (
        b3["delta_torque"].as_f64().unwrap(),
        r["delta_torque"].as_f64().unwrap(),
    );
    let r1 = d2 < d3 || d2 <= 0.002;
    println!(
        "W49P R1: Delta_B3 {d3:.5} Delta_B2 {d2:.5}: {}",
        if r1 { "met" } else { "MISSED" }
    );
    assert!(r1, "R1");
    assert_limits(&r, &["C1", "C2", "C3", "S"]);
}
