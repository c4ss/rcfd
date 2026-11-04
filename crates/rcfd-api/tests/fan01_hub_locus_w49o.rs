//! Job 49O (ADR 0178): the FAN-01 B3 hub mid-chord cycle with the leading-edge driver held off by F-R(0.25) (ADR 0177):
//! necessity and complement arms of the convection deferred correction (ADR 0175 hook) at rings 3 and 4 of the extended hub
//! seeds, with distant and control arms. Registered in ADR 0178 before the runs. Mesh and case text as `fan01_stall_w43c.rs`.
//!
//! Each arm takes about 2 min in release; run one per process:
//! `W49O_OUT=<dir> cargo test --release -p rcfd-api --test fan01_hub_locus_w49o <arm> -- --ignored --exact --nocapture`.

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

// ---------------------------------------------------------------- seeds and rings (ADR 0175 definitions)

const S_LE: [usize; 5] = [17, 665, 701, 341, 53];
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

// ---------------------------------------------------------------- seeds, rings and the arm (ADR 0178 definitions)

/// The extended hub seeds S_H: the ADR 0111 cells plus the ADR 0177 F-R(0.25) record's top cells from iteration 1010 on.
const S_H_EXTRA: [usize; 14] = [907, 906, 581, 905, 582, 149, 437, 150, 438, 402, 113, 114, 77, 257];

fn s_h() -> Vec<usize> {
    let mut v: Vec<usize> = S_HUB.iter().chain(S_H_EXTRA.iter()).copied().collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The 19 cells nearest to (r 0.20 m, z 0.30 m) in the (r, z) plane.
fn s_d(sim: &Simulation) -> Vec<usize> {
    let n = sim.geom.mesh.n_cells();
    let mut idx: Vec<usize> = (0..n).collect();
    let d = |c: usize| {
        let (r, z) = rz(sim, c);
        (r - 0.20).hypot(z - 0.30)
    };
    idx.sort_by(|a, b| d(*a).partial_cmp(&d(*b)).unwrap());
    idx.truncate(19);
    idx
}

struct Regions {
    le2: Vec<bool>,
    h3: Vec<bool>,
    h4: Vec<bool>,
    d3: Vec<bool>,
}

fn regions(sim: &Simulation) -> Regions {
    let sh = s_h();
    Regions {
        le2: ring(sim, &S_LE, 2),
        h3: ring(sim, &sh, 3),
        h4: ring(sim, &sh, 4),
        // ADR 0178 amendment 1: D3' = D3 minus H4
        d3: ring(sim, &s_d(sim), 3)
            .iter()
            .zip(ring(sim, &sh, 4))
            .map(|(d, h)| *d && !h)
            .collect(),
    }
}

const N0: usize = 1000;
const N1: usize = 2000;
const W: usize = 500;

fn location(sim: &Simulation, du: &[f64], rg: &Regions) -> Value {
    let tot2: f64 = du.iter().map(|x| x * x).sum::<f64>().max(1e-300);
    let share_in = |r: &[bool]| (0..du.len()).filter(|&c| r[c]).map(|c| du[c] * du[c]).sum::<f64>() / tot2;
    let max_in = |r: &[bool]| (0..du.len()).filter(|&c| r[c]).map(|c| du[c]).fold(0.0, f64::max);
    let mut idx: Vec<usize> = (0..du.len()).collect();
    idx.sort_by(|a, b| du[*b].partial_cmp(&du[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<Value> = idx
        .iter()
        .take(5)
        .map(|&c| {
            let (r, z) = rz(sim, c);
            let class = if rg.le2[c] {
                "LE"
            } else if rg.h4[c] {
                "HUB"
            } else if rg.d3[c] {
                "D"
            } else {
                "other"
            };
            json!({"cell": c, "du": du[c], "r": r, "z": z, "class": class})
        })
        .collect();
    json!({"top5": top, "share_le2": share_in(&rg.le2), "share_h4": share_in(&rg.h4), "share_d3": share_in(&rg.d3),
           "max_du_le2": max_in(&rg.le2)})
}

/// One arm: F-R(0.25) and the C-mask `removed` (None = nothing removed) set at the switch.
fn arm(id: &str, removed: impl FnOnce(&Regions) -> Option<Vec<bool>>) -> (Value, Regions) {
    let t0 = Instant::now();
    let mut sim = b3();
    let rg = regions(&sim);
    let mask = removed(&rg);
    let n_removed = mask.as_deref().map_or(0, count);
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut prev = sim.velocity().values.clone();
    let (mut du_switch, mut f_switch) = (f64::NAN, (f64::NAN, f64::NAN));
    let (mut reached, mut stopped) = (None, None);
    for it in 1..=(N0 + N1) {
        if it == N0 + 1 {
            du_switch = l2(&speed_change(&sim.velocity().values, &prev));
            f_switch = functionals(&sim);
            let n = sim.geom.mesh.n_cells();
            let s = sim.simple_solver_mut().unwrap();
            s.deferred_correction_relaxation_test_only = Some(0.25);
            if let Some(m) = &mask {
                s.high_order_mask_test_only = Some(HighOrderMask {
                    conv: m.clone(),
                    lap: vec![false; n],
                });
            }
        }
        prev = sim.velocity().values.clone();
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it > N0 && reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            println!("W49O {id} STOP at {it}");
            break;
        }
        if it % 500 == 0 {
            println!(
                "W49O {id} it {it}: res_u {:.3e} res_p {:.3e} |du|_2 {:.3e} ({:.1} s)",
                st.res_u,
                st.res_p,
                l2(&speed_change(&sim.velocity().values, &prev)),
                t0.elapsed().as_secs_f64()
            );
        }
    }
    let n = hist.len();
    let win = |a: usize, b: usize| -> (f64, f64) {
        let (a, b) = (a.min(n), b.min(n));
        (
            median(hist[a..b].iter().map(|h| h.0).collect()),
            median(hist[a..b].iter().map(|h| h.1).collect()),
        )
    };
    let ref_res = win(N0 - W, N0);
    let arm_res = win(n.saturating_sub(W), n);
    let d_end = speed_change(&sim.velocity().values, &prev);
    let du_end = l2(&d_end);
    let rho_med = (arm_res.0 / ref_res.0).max(arm_res.1 / ref_res.1);
    let rho_du = du_end / du_switch;
    let v = if stopped.is_some() || !(rho_med.is_finite() && rho_du.is_finite()) {
        "NOT EVALUABLE"
    } else if rho_med <= 0.01 && rho_du <= 0.01 {
        "CALMS"
    } else if rho_med >= 1.0 / 3.0 {
        "NO EFFECT"
    } else {
        "PARTIAL"
    };
    let f_end = functionals(&sim);
    let rec = json!({
        "adr": "0178", "arm": id, "c_removed_cells": n_removed,
        "ref_median_res_u": ref_res.0, "ref_median_res_p": ref_res.1,
        "arm_median_res_u": arm_res.0, "arm_median_res_p": arm_res.1,
        "du_switch": du_switch, "du_end": du_end, "rho_med": rho_med, "rho_du": rho_du, "verdict": v,
        "reached": reached, "stopped": stopped, "is_converged_end": sim.is_converged(),
        "torque_switch": f_switch.0, "dpts_switch": f_switch.1, "torque_end": f_end.0, "dpts_end": f_end.1,
        "location_end": location(&sim, &d_end, &rg),
        "seconds": t0.elapsed().as_secs_f64(),
    });
    println!("W49O {id} RESULT {}", serde_json::to_string(&rec).unwrap());
    println!("W49O {id} VERDICT {v} (rho_med {rho_med:.3e}, rho_du {rho_du:.3e}, reached {reached:?})");
    write(&rec);
    let (u, p, t) = (ref_res.0, ref_res.1, f_switch.0);
    assert!(
        format!("{u:.3e}") == "1.089e-3" && format!("{p:.2e}") == "4.87e-4" && format!("{t:.4}") == "1.9177",
        "V1 failed: median res_u {u:.3e} res_p {p:.2e} torque {t:.4}"
    );
    (rec, rg)
}

fn write(rec: &Value) {
    if let Ok(dir) = std::env::var("W49O_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        let p = std::path::Path::new(&dir).join(format!("{}.json", rec["arm"].as_str().unwrap()));
        std::fs::write(p, serde_json::to_string_pretty(rec).unwrap()).unwrap();
    }
}

fn verdict_of(r: &Value) -> &str {
    r["verdict"].as_str().unwrap()
}

/// Complement verdict (ADR 0175 amendment 2 rule) with the arm's own site; added to the record as `verdict_k`.
fn complement(id: &str, kept: impl Fn(&Regions) -> Vec<bool>, site: impl Fn(&Regions) -> Vec<bool>) -> Value {
    let (mut r, rg) = arm(id, |rg| Some(kept(rg).iter().map(|b| !b).collect()));
    let s = site(&rg);
    let in_site = r["location_end"]["top5"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| s[t["cell"].as_u64().unwrap() as usize])
        .count();
    let rho_du = r["rho_du"].as_f64().unwrap();
    let vk = match verdict_of(&r) {
        "CALMS" => "CALMS",
        "NOT EVALUABLE" => "NOT EVALUABLE",
        _ if rho_du >= 0.1 && in_site >= 3 => "SUSTAINS",
        _ => "WEAK",
    };
    r["verdict_k"] = json!(vk);
    r["top5_in_own_site"] = json!(in_site);
    println!("W49O {id} VERDICT_K {vk} ({in_site} of the top 5 in the site)");
    write(&r);
    r
}

fn vk(r: &Value) -> &str {
    r["verdict_k"].as_str().unwrap()
}

// ---------------------------------------------------------------- checks and arms

/// V4: seed and ring sizes, overlaps; D3 disjoint from H4 and LE2 and within a factor 2 of H3.
#[test]
#[ignore = "ADR 0178 V1 / V4 (seconds in release)"]
fn v1_v4_rings() {
    let sim = b3();
    assert_eq!(sim.geom.mesh.n_cells(), 4920);
    assert_eq!(s_h().len(), 19);
    let rg = regions(&sim);
    let inter = |a: &[bool], b: &[bool]| a.iter().zip(b).filter(|(x, y)| **x && **y).count();
    println!(
        "W49O V4 |S_H| 19 |H3| {} |H4| {} |D3| {} |LE2| {}; H3&LE2 {} H4&LE2 {} D3&H4 {} D3&LE2 {}; D seeds {:?}",
        count(&rg.h3),
        count(&rg.h4),
        count(&rg.d3),
        count(&rg.le2),
        inter(&rg.h3, &rg.le2),
        inter(&rg.h4, &rg.le2),
        inter(&rg.d3, &rg.h4),
        inter(&rg.d3, &rg.le2),
        s_d(&sim)
    );
    assert_eq!(inter(&rg.d3, &rg.h4), 0);
    assert_eq!(inter(&rg.d3, &rg.le2), 0);
    let ratio = count(&rg.d3) as f64 / count(&rg.h3) as f64;
    assert!((0.5..=2.0).contains(&ratio), "|D3| / |H3| = {ratio}");
}

/// F0 (V2): F-R(0.25) alone reproduces the ADR 0177 record.
#[test]
#[ignore = "ADR 0178 F0 (about 2 min in release)"]
fn f0_control() {
    let (r, _) = arm("F0", |_| None);
    let (rm, rd, le) = (
        r["rho_med"].as_f64().unwrap(),
        r["rho_du"].as_f64().unwrap(),
        r["location_end"]["max_du_le2"].as_f64().unwrap(),
    );
    assert!(
        format!("{rm:.3e}") == "1.038e0" && format!("{rd:.3e}") == "1.929e0" && format!("{le:.3e}") == "1.032e-2",
        "V2: rho_med {rm:.3e} rho_du {rd:.3e} LE2 {le:.3e}"
    );
}

/// FC (V3): C removed everywhere with F-R on reproduces ADR 0175 C-C.
#[test]
#[ignore = "ADR 0178 FC (about 2 min in release)"]
fn fc_control() {
    let (r, _) = arm("FC", |rg| Some(vec![true; rg.h3.len()]));
    let (rm, rd) = (r["rho_med"].as_f64().unwrap(), r["rho_du"].as_f64().unwrap());
    assert!(
        r["reached"].as_u64() == Some(1115) && format!("{rm:.3e}") == "3.957e-3" && format!("{rd:.3e}") == "6.897e-3",
        "V3: reached {:?} rho_med {rm:.3e} rho_du {rd:.3e}",
        r["reached"]
    );
}

/// N-H3 (P1: does not calm).
#[test]
#[ignore = "known miss (ADR 0178 P1): N-H3 CALMS (rho_med 9.085e-9, rho_du 2.758e-9, reached 1298) (about 2 min in release)"]
fn n_h3_known_miss() {
    let (r, _) = arm("N-H3", |rg| Some(rg.h3.clone()));
    assert_ne!(verdict_of(&r), "CALMS", "P1");
}

/// N-H4 (P2: calms).
#[test]
#[ignore = "ADR 0178 N-H4 (about 2 min in release)"]
fn n_h4() {
    let (r, _) = arm("N-H4", |rg| Some(rg.h4.clone()));
    assert_eq!(verdict_of(&r), "CALMS", "P2");
}

/// K-H3 (P3: sustains).
#[test]
#[ignore = "ADR 0178 K-H3 (about 2 min in release)"]
fn k_h3() {
    let r = complement("K-H3", |rg| rg.h3.clone(), |rg| rg.h4.clone());
    assert_eq!(vk(&r), "SUSTAINS", "P3");
}

/// K-H4 (P4: sustains).
#[test]
#[ignore = "ADR 0178 K-H4 (about 2 min in release)"]
fn k_h4() {
    let r = complement("K-H4", |rg| rg.h4.clone(), |rg| rg.h4.clone());
    assert_eq!(vk(&r), "SUSTAINS", "P4");
}

/// N-D3 (P5: does not calm).
#[test]
#[ignore = "ADR 0178 N-D3 (about 2 min in release)"]
fn n_d3() {
    let (r, _) = arm("N-D3", |rg| Some(rg.d3.clone()));
    assert_ne!(verdict_of(&r), "CALMS", "P5");
}

/// K-D3 (P6: calms).
#[test]
#[ignore = "ADR 0178 K-D3 (about 2 min in release)"]
fn k_d3() {
    let r = complement("K-D3", |rg| rg.d3.clone(), |rg| rg.d3.clone());
    assert_eq!(vk(&r), "CALMS", "P6");
}
