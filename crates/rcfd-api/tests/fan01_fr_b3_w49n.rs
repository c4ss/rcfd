//! Job 49N (ADR 0177): the under-relaxed deferred correction of ADR 0111 (F-R) on the bladed FAN-01 passage B3, through the
//! test-only field `SimpleSolver::deferred_correction_relaxation_test_only`. Everything was registered in ADR 0177 before the
//! runs. The mesh and case text are those of `fan01_stall_w43c.rs` (via `fan01_stall_locus_w49l.rs`), copied unchanged.
//!
//! Each arm takes about 2.5 min in release; run one per process:
//! `W49N_OUT=<dir> cargo test --release -p rcfd-api --test fan01_fr_b3_w49n <arm> -- --ignored --exact --nocapture`.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
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

fn e3() -> Simulation {
    Simulation::new(&Case::parse_toml(&case_toml(F3, false)).unwrap()).unwrap()
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

fn bits(s: &Simulation) -> Vec<u64> {
    let v = s.velocity().values.iter().flat_map(|u| u.iter().map(|x| x.to_bits()));
    let p = s.pressure().values.iter().map(|x| x.to_bits());
    let f = s.flux().iter().map(|x| x.to_bits());
    v.chain(p).chain(f).collect()
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

fn union(a: &[bool], b: &[bool]) -> Vec<bool> {
    a.iter().zip(b).map(|(x, y)| *x || *y).collect()
}

// ---------------------------------------------------------------- the F-R arm (ADR 0177 definitions)

const N0: usize = 1000;
const N1: usize = 2000;
const W: usize = 500;
const RECORD_AT: [usize; 5] = [1010, 1100, 1250, 1500, 2000];

/// The LE2 record of one iterate change.
fn le2_record(sim: &Simulation, du: &[f64], le2: &[bool], hub2: &[bool]) -> Value {
    let tot2: f64 = du.iter().map(|x| x * x).sum::<f64>().max(1e-300);
    let max_in = |r: &[bool]| (0..du.len()).filter(|&c| r[c]).map(|c| du[c]).fold(0.0, f64::max);
    let share_in = |r: &[bool]| (0..du.len()).filter(|&c| r[c]).map(|c| du[c] * du[c]).sum::<f64>() / tot2;
    let mut idx: Vec<usize> = (0..du.len()).collect();
    idx.sort_by(|a, b| du[*b].partial_cmp(&du[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<Value> = idx
        .iter()
        .take(5)
        .map(|&c| {
            let (r, z) = rz(sim, c);
            let class = if le2[c] {
                "LE"
            } else if hub2[c] {
                "HUB"
            } else {
                "other"
            };
            json!({"cell": c, "du": du[c], "r": r, "z": z, "class": class})
        })
        .collect();
    json!({"max_du_le2": max_in(le2), "max_du_hub2": max_in(hub2), "share_le2": share_in(le2),
           "share_hub2": share_in(hub2), "du_l2": l2(du), "top5": top})
}

/// Reference band of a functional: [min, max] over iterations 501-1000, widened by 0.5 % of |mean| on each side.
fn band(v: &[f64]) -> (f64, f64) {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |a, x| (a.0.min(*x), a.1.max(*x)));
    (lo - 0.005 * mean.abs(), hi + 0.005 * mean.abs())
}

fn fr_arm(id: &str, beta: f64) -> Value {
    let t0 = Instant::now();
    let mut sim = b3();
    let le2 = ring(&sim, &S_LE, 2);
    let hub2 = ring(&sim, &S_HUB, 2);
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut f_ref: Vec<(f64, f64)> = Vec::new();
    let mut prev = sim.velocity().values.clone();
    let mut prev2 = prev.clone();
    let mut du_switch = f64::NAN;
    let mut records = serde_json::Map::new();
    let mut f_switch = (f64::NAN, f64::NAN);
    let (mut reached, mut stopped, mut conv_first) = (None, None, None);
    for it in 1..=(N0 + N1) {
        if it == N0 + 1 {
            let d = speed_change(&sim.velocity().values, &prev);
            du_switch = l2(&d);
            f_switch = functionals(&sim);
            records.insert("1000".into(), le2_record(&sim, &d, &le2, &hub2));
            sim.simple_solver_mut()
                .unwrap()
                .deferred_correction_relaxation_test_only = Some(beta);
        }
        prev2 = std::mem::replace(&mut prev, sim.velocity().values.clone());
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it > N0 - W && it <= N0 {
            f_ref.push(functionals(&sim));
        }
        if it > N0 && reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if it > N0 && conv_first.is_none() && sim.is_converged() {
            conv_first = Some(it);
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            println!("W49N {id} STOP at {it}");
            break;
        }
        if RECORD_AT.contains(&it) {
            let d = speed_change(&sim.velocity().values, &prev);
            records.insert(it.to_string(), le2_record(&sim, &d, &le2, &hub2));
        }
        if it % 250 == 0 {
            println!(
                "W49N {id} it {it}: res_u {:.3e} res_p {:.3e} |du|_2 {:.3e} ({:.1} s)",
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
    let end_rec = le2_record(&sim, &d_end, &le2, &hub2);
    records.insert("end".into(), end_rec.clone());
    let ratio = l2(&speed_change(&sim.velocity().values, &prev2)) / du_end.max(1e-300);
    let rho_med = (arm_res.0 / ref_res.0).max(arm_res.1 / ref_res.1);
    let rho_du = du_end / du_switch;
    let f_end = functionals(&sim);
    let bt = band(&f_ref.iter().map(|f| f.0).collect::<Vec<_>>());
    let bp = band(&f_ref.iter().map(|f| f.1).collect::<Vec<_>>());
    let umax = sim.velocity().values.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
    let cu_end = d_end.iter().cloned().fold(0.0, f64::max) / umax.max(1e-300);
    let res_nut_end = sim.monitors.last("res_nut").unwrap_or(f64::NAN);
    let converged_end = sim.is_converged();
    // probe: the default Barth-Jespersen map, one iteration
    sim.simple_solver_mut()
        .unwrap()
        .deferred_correction_relaxation_test_only = None;
    let p = sim.step();
    let le_switch = records["1000"]["max_du_le2"].as_f64().unwrap();
    let le_end = end_rec["max_du_le2"].as_f64().unwrap();
    let ok = stopped.is_none();
    let limits = json!({
        "Z2a": ok && rho_med <= 0.01 && rho_du <= 0.01,
        "Z2b": ok && reached.is_some(),
        "Z2c": ok && p.res_u <= 1e-5 && p.res_p <= 1e-5,
        "Z2t": ok && f_end.0 >= bt.0 && f_end.0 <= bt.1,
        "Z2p": ok && f_end.1 >= bp.0 && f_end.1 <= bp.1,
        "Z2le": ok && le_end <= 0.01 * le_switch,
    });
    let rec = json!({
        "adr": "0177", "arm": id, "beta": beta,
        "ref_median_res_u": ref_res.0, "ref_median_res_p": ref_res.1,
        "arm_median_res_u": arm_res.0, "arm_median_res_p": arm_res.1,
        "du_switch": du_switch, "du_end": du_end, "rho_med": rho_med, "rho_du": rho_du,
        "reached": reached, "stopped": stopped, "is_converged_first": conv_first, "is_converged_end": converged_end,
        "cu_end": cu_end, "res_nut_end": res_nut_end, "period_ratio_end": ratio,
        "probe_res_u": p.res_u, "probe_res_p": p.res_p,
        "torque_switch": f_switch.0, "dpts_switch": f_switch.1,
        "torque_end": f_end.0, "dpts_end": f_end.1,
        "torque_band": [bt.0, bt.1], "dpts_band": [bp.0, bp.1],
        "max_du_le2_switch": le_switch, "max_du_le2_end": le_end,
        "limits": limits,
        "le2_record": records,
        "seconds": t0.elapsed().as_secs_f64(),
    });
    println!("W49N {id} RESULT {}", serde_json::to_string(&rec).unwrap());
    println!(
        "W49N {id} rho_med {rho_med:.3e} rho_du {rho_du:.3e} reached {reached:?} probe {:.2e}/{:.2e} torque {:.4} band [{:.4}, {:.4}] dpts {:.3} band [{:.3}, {:.3}] LE2 max du {le_switch:.3e} -> {le_end:.3e}; limits {}",
        p.res_u, p.res_p, f_end.0, bt.0, bt.1, f_end.1, bp.0, bp.1, rec["limits"]
    );
    if let Ok(dir) = std::env::var("W49N_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        let path = std::path::Path::new(&dir).join(format!("{id}.json"));
        std::fs::write(path, serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    }
    // V1 (checked in every arm)
    let (u, pp, t) = (ref_res.0, ref_res.1, f_switch.0);
    assert!(
        format!("{u:.3e}") == "1.089e-3" && format!("{pp:.2e}") == "4.87e-4" && format!("{t:.4}") == "1.9177",
        "V1 failed: median res_u {u:.3e} res_p {pp:.2e} torque {t:.4}"
    );
    rec
}

fn assert_limits(r: &Value) {
    let l = &r["limits"];
    let missed: Vec<&str> = ["Z2a", "Z2b", "Z2c", "Z2t", "Z2p", "Z2le"]
        .into_iter()
        .filter(|k| l[*k] != json!(true))
        .collect();
    assert!(missed.is_empty(), "missed: {missed:?}");
}

// ---------------------------------------------------------------- checks

/// ADR 0177 V2: the field left `None` is bit-identical to a solver that never had it (E3, 30 iterations; u, p, phi).
#[test]
fn w49n_field_none_is_bit_identical() {
    let (mut a, mut b) = (e3(), e3());
    b.simple_solver_mut().unwrap().deferred_correction_relaxation_test_only = None;
    for _ in 0..30 {
        a.step();
        b.step();
    }
    assert_eq!(bits(&a), bits(&b));
}

/// ADR 0177 V1 (cells) and V2 (B3: beta = 1 after 30 default iterations differs from the default iteration by round-off).
#[test]
#[ignore = "ADR 0177 V1 / V2 (seconds in release)"]
fn v1_v2_cells_and_beta_one() {
    let (mut a, mut b) = (b3(), b3());
    assert_eq!(a.geom.mesh.n_cells(), 4920);
    assert_eq!(count(&union(&ring(&a, &S_LE, 2), &ring(&a, &S_HUB, 2))), 129);
    for _ in 0..30 {
        a.step();
        b.step();
    }
    b.simple_solver_mut().unwrap().deferred_correction_relaxation_test_only = Some(1.0);
    a.step();
    b.step();
    let rel = |x: Vec<f64>, y: Vec<f64>| {
        let m = x.iter().map(|v| v.abs()).fold(0.0, f64::max).max(1e-300);
        x.iter().zip(&y).map(|(p, q)| (p - q).abs()).fold(0.0, f64::max) / m
    };
    let flat = |s: &Simulation| -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        (
            s.velocity().values.iter().flat_map(|u| u.iter().copied()).collect(),
            s.pressure().values.clone(),
            s.flux().to_vec(),
        )
    };
    let ((ua, pa, fa), (ub, pb, fb)) = (flat(&a), flat(&b));
    let (du, dp, df) = (rel(ua, ub), rel(pa, pb), rel(fa, fb));
    println!("W49N V2 beta = 1: relative max differences u {du:.2e} p {dp:.2e} phi {df:.2e}");
    assert!(du <= 1e-12 && dp <= 1e-12 && df <= 1e-12);
}

/// ADR 0177 acceptance on B3 (Z2a, Z2b, Z2c, Z2t, Z2p, Z2le) with beta = 0.25.
#[test]
#[ignore = "known miss (ADR 0177 Z2a, Z2b, Z2c): F-R(0.25) rho_med 1.038, rho_du 1.929, tolerance not reached, probe 8.25e-4 / 6.19e-4; Z2t, Z2p, Z2le met (about 2.5 min in release)"]
fn fr_025_b3_known_miss() {
    assert_limits(&fr_arm("FR-0.25", 0.25));
}

/// ADR 0177 conditional arm: beta = 0.1, run only if F-R(0.25) misses Z2a.
#[test]
#[ignore = "known miss (ADR 0177, conditional arm): F-R(0.1) misses every limit (rho_med 28.4, rho_du 5.85, Delta p_ts 42.48 outside its band) (about 2.5 min in release)"]
fn fr_010_b3_known_miss() {
    assert_limits(&fr_arm("FR-0.1", 0.1));
}
