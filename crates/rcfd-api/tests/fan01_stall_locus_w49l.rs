//! Job 49L (ADR 0175): where the FAN-01 Barth-Jespersen stall is driven in the nonlinear iteration. From the stalled B3 state
//! at iteration 1000 (the ADR 0110 switch protocol) the momentum convection deferred correction (C) and / or the explicit
//! non-orthogonal Laplacian correction (L) is removed in a region through the test-only hook
//! `SimpleSolver::high_order_mask_test_only`: phase 1 over every cell (which term carries the ADR 0110 S10 stabilisation),
//! phase 2 at the blade-root leading edge, the hub mid-chord, both, and a distant control. Everything was registered in ADR
//! 0175 before the runs. The mesh and case text are those of `fan01_stall_w43c.rs`, copied unchanged.
//!
//! Each arm takes about 2.5 min in release; run one per process:
//! `W49L_OUT=<dir> cargo test --release -p rcfd-api --test fan01_stall_locus_w49l <arm> -- --ignored --nocapture`.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::simple::{HighOrderMask, SimpleSolver};
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

/// The five cells nearest to (r 0.22 m, z 0.15 m) in the (r, z) plane.
fn s_d(sim: &Simulation) -> Vec<usize> {
    let n = sim.geom.mesh.n_cells();
    let mut idx: Vec<usize> = (0..n).collect();
    let d = |c: usize| {
        let (r, z) = rz(sim, c);
        (r - 0.22).hypot(z - 0.15)
    };
    idx.sort_by(|a, b| d(*a).partial_cmp(&d(*b)).unwrap());
    idx.truncate(5);
    idx
}

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

struct Regions {
    le1: Vec<bool>,
    le2: Vec<bool>,
    hub1: Vec<bool>,
    hub2: Vec<bool>,
    d2: Vec<bool>,
    d_seeds: Vec<usize>,
}

fn regions(sim: &Simulation) -> Regions {
    let d_seeds = s_d(sim);
    Regions {
        le1: ring(sim, &S_LE, 1),
        le2: ring(sim, &S_LE, 2),
        hub1: ring(sim, &S_HUB, 1),
        hub2: ring(sim, &S_HUB, 2),
        d2: ring(sim, &d_seeds, 2),
        d_seeds,
    }
}

// ---------------------------------------------------------------- the arm (ADR 0110 switch protocol)

const N0: usize = 1000;
const N1: usize = 2000;
const W: usize = 500;

/// The region of one arm: (R_C, R_L); `None` = empty.
type Mask = (Option<Vec<bool>>, Option<Vec<bool>>);

fn set_mask(s: &mut SimpleSolver, m: &Mask) {
    let n = s.u.values.len();
    s.high_order_mask_test_only = Some(HighOrderMask {
        conv: m.0.clone().unwrap_or_else(|| vec![false; n]),
        lap: m.1.clone().unwrap_or_else(|| vec![false; n]),
    });
}

fn classify(c: usize, rg: &Regions) -> &'static str {
    if rg.le2[c] {
        "LE"
    } else if rg.hub2[c] {
        "HUB"
    } else {
        "other"
    }
}

/// Location record of one iterate change: the five largest cells, their class, the site, and the shares of `sum |du|^2`.
fn location(sim: &Simulation, du: &[f64], rg: &Regions) -> Value {
    let mut idx: Vec<usize> = (0..du.len()).collect();
    idx.sort_by(|a, b| du[*b].partial_cmp(&du[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let tot2: f64 = du.iter().map(|x| x * x).sum::<f64>().max(1e-300);
    let share_in = |r: &[bool]| (0..du.len()).filter(|&c| r[c]).map(|c| du[c] * du[c]).sum::<f64>() / tot2;
    let n1 = (du.len() / 100).max(1);
    let top1 = idx.iter().take(n1).map(|&c| du[c] * du[c]).sum::<f64>() / tot2;
    let top: Vec<Value> = idx
        .iter()
        .take(5)
        .map(|&c| {
            let (r, z) = rz(sim, c);
            json!({"cell": c, "du": du[c], "r": r, "z": z, "class": classify(c, rg)})
        })
        .collect();
    let n_of = |k: &str| idx.iter().take(5).filter(|&&c| classify(c, rg) == k).count();
    let site = if n_of("LE") >= 3 {
        "LE"
    } else if n_of("HUB") >= 3 {
        "HUB"
    } else if n_of("other") >= 3 {
        "other"
    } else {
        "mixed"
    };
    json!({"top5": top, "site": site, "share_le2": share_in(&rg.le2), "share_hub2": share_in(&rg.hub2), "share_top1pct": top1})
}

fn verdict(rho_med: f64, rho_du: f64) -> &'static str {
    if !(rho_med.is_finite() && rho_du.is_finite()) {
        "NOT EVALUABLE"
    } else if rho_med <= 0.01 && rho_du <= 0.01 {
        "CALMS"
    } else if rho_med >= 1.0 / 3.0 {
        "NO EFFECT"
    } else {
        "PARTIAL"
    }
}

/// Run one arm; print and (with `W49L_OUT`) write its record; return it.
fn arm(id: &str, region_name: &str, mask: impl FnOnce(&Simulation, &Regions) -> Mask) -> Value {
    let t0 = Instant::now();
    let mut sim = b3();
    let rg = regions(&sim);
    let m = mask(&sim, &rg);
    let n_mask = (m.0.as_deref().map_or(0, count), m.1.as_deref().map_or(0, count));
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut prev = sim.velocity().values.clone();
    let mut prev2 = prev.clone();
    let (mut du_ref, mut f_ref) = (f64::NAN, (f64::NAN, f64::NAN));
    let mut loc_ref = Value::Null;
    let mut reached = None;
    let mut stopped = None;
    for it in 1..=(N0 + N1) {
        if it == N0 + 1 {
            let d = speed_change(&sim.velocity().values, &prev);
            du_ref = l2(&d);
            f_ref = functionals(&sim);
            loc_ref = location(&sim, &d, &rg);
            set_mask(sim.simple_solver_mut().unwrap(), &m);
        }
        prev2 = std::mem::replace(&mut prev, sim.velocity().values.clone());
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        if it > N0 && reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            stopped = Some(it);
            println!("W49L {id} STOP at {it}");
            break;
        }
        if it % 500 == 0 {
            let d = speed_change(&sim.velocity().values, &prev);
            println!(
                "W49L {id} it {it}: res_u {:.3e} res_p {:.3e} |du|_2 {:.3e} ({:.1} s)",
                st.res_u,
                st.res_p,
                l2(&d),
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
    let ratio = l2(&speed_change(&sim.velocity().values, &prev2)) / du_end.max(1e-300);
    let rho_med = (arm_res.0 / ref_res.0).max(arm_res.1 / ref_res.1);
    let rho_du = du_end / du_ref;
    let v = if stopped.is_some() {
        "NOT EVALUABLE"
    } else {
        verdict(rho_med, rho_du)
    };
    let f_end = functionals(&sim);
    let rec = json!({
        "adr": "0175",
        "arm": id,
        "region": region_name,
        "mask_cells_conv": n_mask.0,
        "mask_cells_lap": n_mask.1,
        "ref_median_res_u": ref_res.0, "ref_median_res_p": ref_res.1,
        "arm_median_res_u": arm_res.0, "arm_median_res_p": arm_res.1,
        "du_switch": du_ref, "du_end": du_end,
        "rho_med": rho_med, "rho_du": rho_du, "verdict": v,
        "reached": reached, "stopped": stopped,
        "is_converged_end": sim.is_converged(),
        "period_ratio_end": ratio,
        "torque_switch": f_ref.0, "dpts_switch": f_ref.1,
        "torque_end": f_end.0, "dpts_end": f_end.1,
        "location_switch": loc_ref,
        "location_end": location(&sim, &d_end, &rg),
        "seconds": t0.elapsed().as_secs_f64(),
    });
    println!("W49L {id} RESULT {}", serde_json::to_string(&rec).unwrap());
    println!("W49L {id} VERDICT {v} (rho_med {rho_med:.3e}, rho_du {rho_du:.3e}, reached {reached:?})");
    if let Ok(dir) = std::env::var("W49L_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        let p = std::path::Path::new(&dir).join(format!("{id}.json"));
        std::fs::write(p, serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    }
    rec
}

fn all(sim: &Simulation) -> Vec<bool> {
    vec![true; sim.geom.mesh.n_cells()]
}

fn verdict_of(r: &Value) -> &str {
    r["verdict"].as_str().unwrap()
}

/// V3 (checked in every arm; ADR 0175 amendment 1): the reference reproduces the ADR 0110 / 0111 residual medians to the
/// printed digits. The torque is reported only: ADR 0170 changed the moment reports after ADR 0110.
fn v3(r: &Value) {
    let (u, p, t) = (
        r["ref_median_res_u"].as_f64().unwrap(),
        r["ref_median_res_p"].as_f64().unwrap(),
        r["torque_switch"].as_f64().unwrap(),
    );
    println!("W49L V3: median res_u {u:.3e} res_p {p:.2e}; torque at 1000 {t:.4} N m (reported)");
    assert!(
        format!("{u:.3e}") == "1.089e-3" && format!("{p:.2e}") == "4.87e-4",
        "V3 failed: median res_u {u:.3e} res_p {p:.2e}"
    );
}

// ---------------------------------------------------------------- default and exactness checks

/// ADR 0175: the hook with empty regions leaves the solver bit-identical (E3, 30 iterations; u, p, phi).
#[test]
fn w49l_empty_mask_is_bit_identical() {
    let (mut a, mut b) = (e3(), e3());
    set_mask(b.simple_solver_mut().unwrap(), &(None, None));
    for _ in 0..30 {
        a.step();
        b.step();
    }
    assert_eq!(bits(&a), bits(&b));
}

/// V1: the cell count and the recorded positions (ADR 0174 V2).
#[test]
#[ignore = "ADR 0175 V1 (seconds in release)"]
fn v1_cells_and_positions() {
    let sim = b3();
    assert_eq!(sim.geom.mesh.n_cells(), 4920);
    for (c, r, z) in [
        (665, 0.1256, 0.1003),
        (17, 0.1243, 0.0998),
        (341, 0.1249, 0.1000),
        (1234, 0.1408, 0.149),
        (365, 0.1249, 0.172),
        (401, 0.1249, 0.172),
    ] {
        let (rc, zc) = rz(&sim, c);
        assert!((rc - r).abs() <= 6e-4 && (zc - z).abs() <= 6e-4, "c{c}: r {rc} z {zc}");
    }
}

/// V2: after 30 default B3 iterations, one iteration with (a) the empty mask equals the default one and (b) the full mask
/// on both terms equals the ADR 0110 `upwind` switch, bit for bit in u, p and phi.
#[test]
#[ignore = "ADR 0175 V2 (seconds in release)"]
fn v2_hook_exactness() {
    let mut s: Vec<Simulation> = (0..4).map(|_| b3()).collect();
    for _ in 0..30 {
        for x in s.iter_mut() {
            x.step();
        }
    }
    let full = all(&s[2]);
    set_mask(s[1].simple_solver_mut().unwrap(), &(None, None));
    set_mask(s[2].simple_solver_mut().unwrap(), &(Some(full.clone()), Some(full)));
    {
        let u = s[3].simple_solver_mut().unwrap();
        u.settings.convection = rcfd_fv::convection::ConvScheme::Upwind;
        u.linear_upwind_limiter = None;
    }
    for x in s.iter_mut() {
        x.step();
    }
    assert_eq!(bits(&s[0]), bits(&s[1]), "V2 (a)");
    assert_eq!(bits(&s[2]), bits(&s[3]), "V2 (b)");
    assert_ne!(bits(&s[0]), bits(&s[2]), "the full mask must change the iteration");
    println!("W49L V2 (a) and (b) bit-identical");
}

/// V4: ring sizes printed; LE2 and HUB2 disjoint; |D2| within a factor 2 of |LE2| and disjoint from both.
#[test]
#[ignore = "ADR 0175 V4 (seconds in release)"]
fn v4_rings() {
    let sim = b3();
    let rg = regions(&sim);
    let cnt = |r: &[bool]| count(r);
    let inter = |a: &[bool], b: &[bool]| a.iter().zip(b).filter(|(x, y)| **x && **y).count();
    let d_pos: Vec<(usize, f64, f64)> = rg
        .d_seeds
        .iter()
        .map(|&c| {
            let (r, z) = rz(&sim, c);
            (c, r, z)
        })
        .collect();
    println!(
        "W49L V4 |LE1| {} |LE2| {} |HUB1| {} |HUB2| {} |U1| {} |U2| {} |D2| {}; LE2&HUB2 {} D2&LE2 {} D2&HUB2 {}; D seeds {d_pos:?}",
        cnt(&rg.le1),
        cnt(&rg.le2),
        cnt(&rg.hub1),
        cnt(&rg.hub2),
        cnt(&union(&rg.le1, &rg.hub1)),
        cnt(&union(&rg.le2, &rg.hub2)),
        cnt(&rg.d2),
        inter(&rg.le2, &rg.hub2),
        inter(&rg.d2, &rg.le2),
        inter(&rg.d2, &rg.hub2)
    );
    assert_eq!(inter(&rg.le2, &rg.hub2), 0);
    assert_eq!(inter(&rg.d2, &rg.le2), 0);
    assert_eq!(inter(&rg.d2, &rg.hub2), 0);
    let ratio = cnt(&rg.d2) as f64 / cnt(&rg.le2) as f64;
    assert!((0.5..=2.0).contains(&ratio), "|D2| / |LE2| = {ratio}");
}

// ---------------------------------------------------------------- phase 1 (region ALL)

/// C0 (P0): the empty mask; the stall continues.
#[test]
#[ignore = "ADR 0175 C0 (about 2.5 min in release)"]
fn c0_empty_mask() {
    let r = arm("C0", "empty", |_, _| (None, None));
    v3(&r);
    assert_eq!(verdict_of(&r), "NO EFFECT", "P0");
}

/// C-CL (V5): both terms removed everywhere = ADR 0110 S10.
#[test]
#[ignore = "ADR 0175 C-CL (about 2.5 min in release)"]
fn c_cl_both_terms_everywhere() {
    let r = arm("C-CL", "ALL (C and L)", |s, _| (Some(all(s)), Some(all(s))));
    v3(&r);
    let (rm, rd) = (r["rho_med"].as_f64().unwrap(), r["rho_du"].as_f64().unwrap());
    assert!(
        r["reached"].as_u64() == Some(1114) && format!("{rm:.1e}") == "4.3e-3" && format!("{rd:.1e}") == "7.4e-3",
        "V5: reached {:?} rho_med {rm:.1e} rho_du {rd:.1e}",
        r["reached"]
    );
}

/// C-C (P1): the convection deferred correction removed everywhere, the Laplacian correction live.
#[test]
#[ignore = "ADR 0175 C-C (about 2.5 min in release)"]
fn c_c_convection_everywhere() {
    let r = arm("C-C", "ALL (C)", |s, _| (Some(all(s)), None));
    v3(&r);
    assert_eq!(verdict_of(&r), "CALMS", "P1");
}

/// C-L (P2): the Laplacian non-orthogonal correction removed everywhere, the convection correction live.
#[test]
#[ignore = "ADR 0175 C-L (about 2.5 min in release)"]
fn c_l_laplacian_everywhere() {
    let r = arm("C-L", "ALL (L)", |s, _| (None, Some(all(s))));
    v3(&r);
    assert_eq!(verdict_of(&r), "NO EFFECT", "P2");
}

// ---------------------------------------------------------------- phase 2 (term set T by the registered rule)

/// The registered rule: T = C if C-C CALMS; else L if C-L CALMS; else C+L if C-CL CALMS; else phase 2 is not run.
fn term_set() -> (bool, bool) {
    let dir = std::env::var("W49L_OUT").expect("phase 2 reads the phase-1 records from W49L_OUT");
    let v = |id: &str| -> String {
        let p = std::path::Path::new(&dir).join(format!("{id}.json"));
        let t = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let j: Value = serde_json::from_str(&t).unwrap();
        j["verdict"].as_str().unwrap().to_string()
    };
    let t = if v("C-C") == "CALMS" {
        (true, false)
    } else if v("C-L") == "CALMS" {
        (false, true)
    } else if v("C-CL") == "CALMS" {
        (true, true)
    } else {
        panic!("C-CL does not calm: phase 2 needs an amendment");
    };
    println!("W49L term set T: C {} L {}", t.0, t.1);
    t
}

fn local(id: &str, name: &str, pick: impl Fn(&Simulation, &Regions) -> Vec<bool>) -> Value {
    let (tc, tl) = term_set();
    let r = arm(id, name, |s, rg| {
        let m = pick(s, rg);
        (tc.then(|| m.clone()), tl.then_some(m))
    });
    v3(&r);
    r
}

/// P4 (single-site arms do not calm).
fn p4(r: &Value) {
    assert_ne!(verdict_of(r), "CALMS", "P4");
}

#[test]
#[ignore = "ADR 0175 A-LE1 (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_le1() {
    p4(&local("A-LE1", "LE1", |_, rg| rg.le1.clone()));
}

/// A-LE2 (P4, P6: the remaining cycle sits at HUB).
#[test]
#[ignore = "known miss (ADR 0175 P6): the A-LE2 cycle sits at 'other' (2 of the top 5 in HUB2), not at HUB; P4 met (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_le2_known_miss() {
    let r = local("A-LE2", "LE2", |_, rg| rg.le2.clone());
    p4(&r);
    assert_eq!(r["location_end"]["site"].as_str(), Some("HUB"), "P6");
}

#[test]
#[ignore = "ADR 0175 A-HUB1 (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_hub1() {
    p4(&local("A-HUB1", "HUB1", |_, rg| rg.hub1.clone()));
}

#[test]
#[ignore = "ADR 0175 A-HUB2 (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_hub2() {
    p4(&local("A-HUB2", "HUB2", |_, rg| rg.hub2.clone()));
}

/// A-U1: reported (predicted PARTIAL, no limit).
#[test]
#[ignore = "ADR 0175 A-U1 (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_u1() {
    let r = local("A-U1", "U1", |_, rg| union(&rg.le1, &rg.hub1));
    assert_ne!(verdict_of(&r), "NOT EVALUABLE");
}

/// A-U2 (P3: calms).
#[test]
#[ignore = "known miss (ADR 0175 P3): A-U2 is NO EFFECT, rho_med 0.651, rho_du 0.416 (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_u2_known_miss() {
    let r = local("A-U2", "U2", |_, rg| union(&rg.le2, &rg.hub2));
    assert_eq!(verdict_of(&r), "CALMS", "P3");
}

/// A-D2 (P5: does not calm).
#[test]
#[ignore = "ADR 0175 A-D2 (about 2.5 min in release; needs the phase-1 records in W49L_OUT)"]
fn a_d2() {
    let r = local("A-D2", "D2", |_, rg| rg.d2.clone());
    assert_ne!(verdict_of(&r), "CALMS", "P5");
}
