//! Wave 43C (ADR 0110): why the FAN-01 rotor-passage steady run does not converge. The ADR 0086 set-up (C-grid `camber_seam_h`, SST,
//! SIMPLEC, Barth-Jespersen-limited `linear_upwind`, `rotating_frame`) on the coarsest family member F3 (bladed B3, empty E3), a per-iteration
//! location record, and one switch per registered mechanism applied from the same stalled state. Everything was registered in ADR 0110 before
//! the registered runs. No agreement with measured fan data is claimed.
//!
//! Wave 44A (ADR 0111, at the end): the linearisation of the frozen-limiter map on B3 (matrix-free Arnoldi, analysed by
//! `scripts/w44a_arnoldi_analysis.py`), its attribution checks and the stop-rule record (the under-relaxed deferred correction F-R missed its acceptance and was removed
//! under rule (b); its numbers are in the ADR).

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::simple::SimpleSolver;
use std::f64::consts::PI;
use std::time::Instant;

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const Q_DESIGN: f64 = 1.4032;

fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1).
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

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) of the ADR 0086 family.
type Counts = (usize, usize, usize, usize, usize, usize, usize);
const F1: Counts = (12, 24, 12, 15, 6, 21, 10);
const F2: Counts = (8, 17, 8, 11, 4, 15, 7);
const F3: Counts = (6, 12, 6, 8, 3, 10, 5);

/// The ADR 0086 mesh (construction `camber_seam_h`), unchanged.
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

/// The ADR 0086 case exactly (`blade = false`: the empty passage, exact absolute swirl zero), plus `solver_extra` lines in `[solver]` and
/// `turbulence` replaced by laminar flow when `laminar`.
fn case_toml(c: Counts, blade: bool, solver_extra: &str, laminar: bool) -> String {
    case_toml_outlet(
        c,
        blade,
        solver_extra,
        laminar,
        "velocity = \"zero_gradient\"
pressure = \"fixed\"
",
    )
}

/// [`case_toml`] with the body of `[boundaries.outlet]` replaced.
fn case_toml_outlet(c: Counts, blade: bool, solver_extra: &str, laminar: bool, outlet: &str) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let wall_t = if laminar { "" } else { "turbulence = \"wall\"\n" };
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
    let turb = if laminar {
        String::new()
    } else {
        "[turbulence]\nmodel = \"k_omega_sst\"\n".to_string()
    };
    let inlet_t = if laminar {
        String::new()
    } else {
        format!("turbulence = \"inlet\"\nturbulence_spec = \"values\"\nk_value = {k:?}\nomega_value = {om:?}\n")
    };
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
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

// ---------------------------------------------------------------- outlet statistics (ADR 0078 / 0086 definitions)

fn polar(c: Vec3, x: Vec3) -> (f64, f64) {
    let r = x[0].hypot(x[1]);
    ((-x[1] * c[0] + x[0] * c[1]) / r, r)
}

/// Outlet (L [N m], rho <c_theta^2/2>_m [Pa], number of outlet faces with inflow) on the full annulus.
fn outlet_stats(sim: &Simulation) -> (f64, f64, usize) {
    let g = &sim.geom;
    let ni = g.n_internal;
    let om = omega();
    let ub = sim.velocity().boundary_values(g);
    let phi = sim.flux();
    let (cf, sf) = (g.mesh.face_centres(), g.mesh.face_area_vectors());
    let (mut q, mut l, mut k, mut back) = (0.0, 0.0, 0.0, 0usize);
    for f in g.mesh.patch("outlet").unwrap().faces() {
        let s = if sf[f][2] >= 0.0 { 1.0 } else { -1.0 };
        let w = ub[f - ni];
        let x = cf[f];
        let c = [w[0] - om * x[1], w[1] + om * x[0], w[2]];
        let (ct, r) = polar(c, x);
        let fl = s * phi[f];
        q += fl;
        l += r * ct * fl;
        k += 0.5 * ct * ct * fl;
        if phi[f] < 0.0 {
            back += 1;
        }
    }
    (N_BLADES * RHO * l, RHO * k / q, back)
}

/// Bladed: (torque, Delta p_ts) as ADR 0086; empty: (outlet L, outlet swirl energy).
fn functionals(sim: &Simulation, blade: bool) -> (f64, f64) {
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    if blade {
        (
            -N_BLADES * (mon("m_blade") + mon("m_hub")),
            RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
        )
    } else {
        let (l, k, _) = outlet_stats(sim);
        (l, k)
    }
}

// ---------------------------------------------------------------- location record

/// Largest face non-orthogonality angle [deg] of every cell, and the patch names each cell touches.
fn cell_attributes(sim: &Simulation) -> (Vec<f64>, Vec<String>) {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let (cc, cf, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let nc = mesh.n_cells();
    let mut ang = vec![0.0f64; nc];
    let mut tags = vec![String::new(); nc];
    let mut patch_of = vec![String::new(); mesh.n_faces()];
    for p in mesh.patches() {
        for f in p.faces() {
            patch_of[f] = p.name.clone();
        }
    }
    for f in 0..mesh.n_faces() {
        if g.empty[f] {
            continue;
        }
        let o = own[f];
        let d = if f < g.n_internal {
            vec3::sub(cc[nei[f]], cc[o])
        } else {
            vec3::sub(cf[f], cc[o])
        };
        let cosv = vec3::dot(d, sf[f]) / (vec3::norm(d) * vec3::norm(sf[f])).max(1e-300);
        let a = cosv.clamp(-1.0, 1.0).acos().to_degrees();
        ang[o] = ang[o].max(a);
        if f < g.n_internal {
            ang[nei[f]] = ang[nei[f]].max(a);
        } else {
            let name = if g.coupled_face[f - g.n_internal] {
                "seam".to_string()
            } else {
                patch_of[f].clone()
            };
            if !tags[o].split('+').any(|t| t == name) {
                if !tags[o].is_empty() {
                    tags[o].push('+');
                }
                tags[o].push_str(&name);
            }
        }
    }
    (ang, tags)
}

/// One record of the state change between two iterates: concentration and location of `|u^n - u^(n-1)|`.
fn location(sim: &Simulation, du: &[f64], ang: &[f64], tags: &[String], top: usize) -> String {
    let cc = sim.geom.mesh.cell_centres();
    let mut idx: Vec<usize> = (0..du.len()).collect();
    idx.sort_by(|a, b| du[*b].partial_cmp(&du[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let tot2: f64 = du.iter().map(|x| x * x).sum::<f64>().max(1e-300);
    let share = |k: usize| idx.iter().take(k).map(|&c| du[c] * du[c]).sum::<f64>() / tot2;
    let n1 = (du.len() / 100).max(1);
    let mut s = format!(
        "share of sum|du|^2 in top 10 cells {:.3}, top 1 % ({n1}) {:.3}; top cells:",
        share(10),
        share(n1)
    );
    for &c in idx.iter().take(top) {
        s += &format!(
            " [c{c} du {:.2e} r {:.4} z {:.4} nonorth {:.1} {}]",
            du[c],
            cc[c][0].hypot(cc[c][1]),
            cc[c][2],
            ang[c],
            if tags[c].is_empty() { "interior" } else { &tags[c] }
        );
    }
    s
}

/// Barth-Jespersen factor of every velocity component of the current state (the kernel's limiter on the unpartnered least-squares gradient).
fn bj_factors(sim: &Simulation) -> [Vec<f64>; 3] {
    let g = &sim.geom;
    let ub = sim.velocity().boundary_values(g);
    let u = &sim.velocity().values;
    [0, 1, 2].map(|k| {
        let cells: Vec<f64> = u.iter().map(|v| v[k]).collect();
        let b: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let gr = rcfd_fv::gradient::least_squares(g, &cells, &b);
        rcfd_fv::gradient::limiter(g, &cells, &b, &gr, rcfd_fv::gradient::Limiter::BarthJespersen)
    })
}

/// For the `top` cells of `du`: the limiter factors now and one iteration earlier, and the faces whose flux changed sign.
fn switch_record(
    sim: &Simulation,
    du: &[f64],
    psi: &[Vec<f64>; 3],
    psi_prev: &[Vec<f64>; 3],
    phi_prev: &[f64],
    top: usize,
) -> String {
    let mesh = &sim.geom.mesh;
    let phi = sim.flux();
    let mut idx: Vec<usize> = (0..du.len()).collect();
    idx.sort_by(|a, b| du[*b].partial_cmp(&du[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let mut s = String::from("switches:");
    for &c in idx.iter().take(top) {
        let flips = mesh
            .cell_faces(c)
            .iter()
            .filter(|&&f| !sim.geom.empty[f] && phi[f] * phi_prev[f] < 0.0)
            .count();
        let fmax = mesh.cell_faces(c).iter().map(|&f| phi[f].abs()).fold(0.0, f64::max);
        let fmin = mesh
            .cell_faces(c)
            .iter()
            .filter(|&&f| !sim.geom.empty[f])
            .map(|&f| phi[f].abs())
            .fold(f64::INFINITY, f64::min);
        s += &format!(
            " [c{c} psi {:.3}/{:.3}/{:.3} (prev {:.3}/{:.3}/{:.3}) flux sign flips {flips}, min/max |phi| {:.1e}]",
            psi[0][c],
            psi[1][c],
            psi[2][c],
            psi_prev[0][c],
            psi_prev[1][c],
            psi_prev[2][c],
            fmin / fmax.max(1e-300)
        );
    }
    let changed = (0..du.len())
        .filter(|&c| (0..3).any(|k| (psi[k][c] - psi_prev[k][c]).abs() > 1e-3))
        .count();
    let flipped = (0..phi.len())
        .filter(|&f| !sim.geom.empty[f] && phi[f] * phi_prev[f] < 0.0)
        .count();
    s + &format!("; cells with a limiter change > 1e-3: {changed}; faces with a flux sign flip: {flipped}")
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

/// Outcome of one arm.
#[derive(Debug, Clone)]
#[allow(dead_code)] // printed through Debug
struct Arm {
    /// median res_u / res_p over the last 500 iterations of the stalled reference window (iterations n0-499..=n0) and of the switched run
    ref_res: (f64, f64),
    arm_res: (f64, f64),
    /// residual of the final state with the reference settings restored (one iteration), when `probe`
    probe: Option<(f64, f64)>,
    /// first iteration (absolute; after the switch, or anywhere without a switch) at which res_u and res_p were both below 1e-5
    reached: Option<usize>,
    /// largest change of each functional over the last 100 iterations
    last100: (f64, f64),
    /// |du|_2 at the switch and at the end
    du_ref: f64,
    du_end: f64,
    /// functionals at the switch and at the end
    f_ref: (f64, f64),
    f_end: (f64, f64),
    converged_flag: bool,
    /// ADR 0111: largest |functional| over the 100-iteration samples of the reference window (the stalled cycle's worst value)
    f_ref_max: (f64, f64),
    /// ADR 0111 rule G: max over cells |u^n - u^(n-1)| / max over cells |u^n| at the end, and the `res_nut` monitor at the end
    cu_end: f64,
    res_nut_end: f64,
    /// ADR 0111 rule G: res_u and res_p below 1e-5 in each of the last 500 iterations
    tail_below: bool,
    /// ADR 0115: first iteration (absolute) at which `is_converged()` was true
    conv_first: Option<usize>,
}

/// Run `n0` iterations of the reference set-up, apply `switch` to the solver, run `n1` more; print the location record. With `probe`, the
/// final state is iterated once more with the reference settings restored and that residual is reported.
#[allow(clippy::too_many_arguments)]
fn arm(
    label: &str,
    c: Counts,
    blade: bool,
    solver_extra: &str,
    laminar: bool,
    outlet: Option<&str>,
    n0: usize,
    switch: &dyn Fn(&mut SimpleSolver),
    n1: usize,
    probe: bool,
) -> (Arm, Simulation) {
    let t0 = Instant::now();
    let text = match outlet {
        Some(o) => case_toml_outlet(c, blade, solver_extra, laminar, o),
        None => case_toml(c, blade, solver_extra, laminar),
    };
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let (ang, tags) = cell_attributes(&sim);
    let nc = sim.geom.mesh.n_cells();
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut prev: Vec<Vec<Vec3>> = Vec::new();
    let mut f_ref = (f64::NAN, f64::NAN);
    let mut du_ref = f64::NAN;
    let mut saved: Option<(
        rcfd_fv::simple::SimpleSettings,
        Option<rcfd_fv::gradient::Limiter>,
        bool,
        bool,
        bool,
    )> = None;
    let mut reached = None;
    let mut conv_first = None;
    let mut psi_prev: Option<[Vec<f64>; 3]> = None;
    let mut phi_prev: Vec<f64> = Vec::new();
    let mut f_hist: Vec<(usize, f64, f64)> = Vec::new();
    for it in 1..=(n0 + n1) {
        if it == n0 + 1 {
            f_ref = functionals(&sim, blade);
            if let Some(p) = prev.last() {
                du_ref = l2(&speed_change(&sim.velocity().values, p));
            }
            let s = sim.simple_solver_mut().unwrap();
            saved = Some((
                s.settings.clone(),
                s.linear_upwind_limiter,
                s.reconstructed_correction,
                s.simplec_boundary_compensation,
                s.periodic_gradient_partner,
            ));
            let had_pseudo = s.pseudo.is_some();
            switch(s);
            assert!(!had_pseudo, "the reference has no pseudo time");
            println!("W43C {label} SWITCH at iteration {n0}");
        }
        let before = sim.velocity().values.clone();
        let near = it % 250 == 0 || it == n0 + n1;
        if near {
            psi_prev = Some(bj_factors(&sim));
            phi_prev = sim.flux().to_vec();
        }
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
        prev.push(before);
        if prev.len() > 3 {
            prev.remove(0);
        }
        if (it > n0 || n1 == 0) && reached.is_none() && st.res_u < 1e-5 && st.res_p < 1e-5 {
            reached = Some(it);
        }
        if conv_first.is_none() && sim.is_converged() {
            conv_first = Some(it);
        }
        if it % 100 == 0 {
            let (fa, fb) = functionals(&sim, blade);
            f_hist.push((it, fa, fb));
        }
        if !(st.res_u.is_finite() && st.continuity.is_finite()) || sim.steady_divergence().is_some() {
            println!(
                "W43C {label} STOP at {it}: res_u {:.3e} cont {:.3e} guard {:?}",
                st.res_u,
                st.continuity,
                sim.steady_divergence().map(|d| d.kind)
            );
            break;
        }
        let report = it % 250 == 0 || it == n0 || it == n0 + n1 || it == 1 || it == 10 || it == 100;
        if report {
            let s = sim.simple_solver().unwrap();
            let h = s.history.last().unwrap();
            let (fa, fb) = functionals(&sim, blade);
            let (_, _, back) = outlet_stats(&sim);
            let u = &sim.velocity().values;
            let d1 = speed_change(u, &prev[prev.len() - 1]);
            // period test: |u^n - u^(n-2)| against |u^n - u^(n-1)| (0 for a period-two cycle, about 2 for a monotone drift)
            let ratio = if prev.len() >= 2 {
                l2(&speed_change(u, &prev[prev.len() - 2])) / l2(&d1).max(1e-300)
            } else {
                f64::NAN
            };
            println!(
                "W43C {label} it {it}: res_u {:.3e} res_p {:.3e} cont {:.3e} max_du {:.3e} p_iters {} backflow faces {back}; f_a {fa:.6} f_b {fb:.5}; |du|_2 {:.3e} period ratio {ratio:.3}; {} ({:.1} s)",
                st.res_u,
                st.res_p,
                st.continuity,
                h.max_du,
                h.p_iters,
                l2(&d1),
                location(&sim, &d1, &ang, &tags, 5),
                t0.elapsed().as_secs_f64()
            );
            if let (Some(pp), false) = (&psi_prev, phi_prev.is_empty()) {
                if it % 250 == 0 || it == n0 + n1 {
                    println!(
                        "W43C {label} it {it} {}",
                        switch_record(&sim, &d1, &bj_factors(&sim), pp, &phi_prev, 5)
                    );
                }
            }
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
    // ADR 0110 amendment 1: both windows have length min(500, n1 / 2) (500 for the arms without a switch)
    let w = if n1 == 0 { 500 } else { 500.min(n1 / 2) };
    let ref_res = win(n0.saturating_sub(w), n0);
    let arm_res = win(n.saturating_sub(w), n);
    let f_end = functionals(&sim, blade);
    let last100 = f_hist
        .iter()
        .rev()
        .find(|h| h.0 + 100 <= n)
        .map_or((f64::NAN, f64::NAN), |h| ((f_end.0 - h.1).abs(), (f_end.1 - h.2).abs()));
    let d_end = speed_change(&sim.velocity().values, &prev[prev.len() - 1]);
    let du_end = l2(&d_end);
    let umax = sim.velocity().values.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
    let cu_end = d_end.iter().cloned().fold(0.0, f64::max) / umax.max(1e-300);
    let res_nut_end = sim.monitors.last("res_nut").unwrap_or(f64::NAN);
    let f_ref_max = f_hist
        .iter()
        .filter(|h| h.0 + w > n0 && h.0 <= n0)
        .fold((0.0f64, 0.0f64), |a, h| (a.0.max(h.1.abs()), a.1.max(h.2.abs())));
    let converged_flag = sim.is_converged();
    let tail_below = n >= 500 && hist[n - 500..].iter().all(|h| h.0 < 1e-5 && h.1 < 1e-5);
    let probe = if probe {
        if let Some((st, lim, rc, sbc, pgp)) = saved {
            let s = sim.simple_solver_mut().unwrap();
            s.settings = st;
            s.linear_upwind_limiter = lim;
            s.reconstructed_correction = rc;
            s.simplec_boundary_compensation = sbc;
            s.periodic_gradient_partner = pgp;
            s.pseudo = None;
            s.frozen_limiter = None;
            let p = sim.step();
            Some((p.res_u, p.res_p))
        } else {
            None
        }
    } else {
        None
    };
    let a = Arm {
        ref_res,
        arm_res,
        probe,
        reached,
        last100,
        du_ref,
        du_end,
        f_ref,
        f_end,
        converged_flag,
        f_ref_max,
        cu_end,
        res_nut_end,
        tail_below,
        conv_first,
    };
    println!(
        "W43C {label} RESULT ({nc} cells, {:.1} s): {a:?}",
        t0.elapsed().as_secs_f64()
    );
    (a, sim)
}

fn none(_: &mut SimpleSolver) {}

// ---------------------------------------------------------------- switches of the registered arms (ADR 0110)

fn venkatakrishnan(s: &mut SimpleSolver) {
    s.linear_upwind_limiter = Some(rcfd_fv::gradient::Limiter::Venkatakrishnan(5.0));
}

fn upwind(s: &mut SimpleSolver) {
    s.settings.convection = rcfd_fv::convection::ConvScheme::Upwind;
    s.linear_upwind_limiter = None;
}

fn unlimited(s: &mut SimpleSolver) {
    s.linear_upwind_limiter = None;
}

fn face_flux(s: &mut SimpleSolver) {
    s.reconstructed_correction = true;
}

fn tight(s: &mut SimpleSolver) {
    s.settings.momentum_solver.tol_rel = 1e-10;
    s.settings.momentum_solver.tol_abs = 1e-16;
    s.settings.momentum_solver.max_iter = 2000;
    s.settings.pressure_solver.tol_rel = 1e-10;
    s.settings.pressure_solver.tol_abs = 1e-18;
    s.settings.pressure_solver.max_iter = 20000;
}

fn partner(s: &mut SimpleSolver) {
    s.periodic_gradient_partner = true;
}

/// The outlet of arm X7: catalogue pressure outlet at 0 Pa with closed reverse-flow faces.
const OUTLET_X7: &str = "bc_kind = \"pressure_outlet\"
bc_pressure = 0.0
prevent_reverse_flow = true
";

/// Registered reading of an arm (ADR 0110 part S): `rho_med` = the larger ratio of the arm-window medians of res_u and res_p to the
/// reference-window medians, `rho_du` = |du|_2 at the end over |du|_2 at the switch (or at the end of the reference run).
fn verdict(a: &Arm) -> (&'static str, f64) {
    let r = (a.arm_res.0 / a.ref_res.0).max(a.arm_res.1 / a.ref_res.1);
    let rdu = a.du_end / a.du_ref;
    let v = if !(r.is_finite() && rdu.is_finite()) {
        "NOT EVALUABLE"
    } else if r <= 0.01 && rdu <= 0.01 {
        "STANDS"
    } else if r >= 1.0 / 3.0 {
        "FALSIFIED"
    } else {
        "CONTRIBUTES"
    };
    println!("W43C verdict: rho_med {r:.3e}, rho_du {rdu:.3e}: {v}");
    (v, r)
}

fn switch_arm(id: &str, switch: &dyn Fn(&mut SimpleSolver), probe: bool) -> (&'static str, Arm) {
    let (a, _) = arm(
        &format!("{id} B3"),
        F3,
        true,
        "",
        false,
        None,
        1000,
        switch,
        2000,
        probe,
    );
    let (v, r) = verdict(&a);
    println!("W43C {id} VERDICT {v} (largest median ratio {r:.3e})");
    (v, a)
}

/// Arms that change the set-up from the first iteration: the reference (1000 iterations, window 500-1000) and the variant (3000, window
/// 2500-3000).
fn start_arm(id: &str, laminar: bool, outlet: Option<&str>) -> &'static str {
    let (r, _) = arm(
        &format!("{id} reference B3"),
        F3,
        true,
        "",
        false,
        None,
        1000,
        &none,
        0,
        false,
    );
    let (mut a, _) = arm(
        &format!("{id} B3"),
        F3,
        true,
        "",
        laminar,
        outlet,
        3000,
        &none,
        0,
        false,
    );
    a.ref_res = r.arm_res;
    a.du_ref = r.du_end;
    let (v, x) = verdict(&a);
    println!("W43C {id} VERDICT {v} (largest median ratio {x:.3e})");
    v
}

/// Observed order of |L| over E3, E2, E1 (`l` coarse first), ADR 0086 fixed-point formula with the empty-family ratios (fine first).
fn family_order(l: &[f64]) -> f64 {
    let (r21, r32) = (1.4073f64, 1.4017f64);
    let (e21, e32) = (l[1] - l[2], l[0] - l[1]);
    let sgn = (e32 / e21).signum();
    let mut p = 1.0f64;
    for _ in 0..500 {
        let q = ((r21.powf(p) - sgn) / (r32.powf(p) - sgn)).ln();
        p = ((e32 / e21).abs().ln() + q).abs() / r21.ln();
    }
    p
}

/// Empty-passage family with one change from the first iteration (600 iterations each): outlet L and swirl energy per member, their change
/// over the last 100 iterations, and the registered reading of the L trend.
fn family(id: &str, switch: &dyn Fn(&mut SimpleSolver), laminar: bool) -> &'static str {
    let mut out = Vec::new();
    for (name, c) in [("E3", F3), ("E2", F2), ("E1", F1)] {
        let (a, _) = arm(
            &format!("{id} {name}"),
            c,
            false,
            "",
            laminar,
            None,
            0,
            switch,
            600,
            false,
        );
        out.push(a);
    }
    let l: Vec<f64> = out.iter().map(|a| a.f_end.0.abs()).collect();
    let settled = out.iter().all(|a| a.last100.0 <= 1e-3);
    let v = if !settled || l.iter().any(|x| !x.is_finite()) {
        "NOT EVALUABLE"
    } else if l[2] < l[1] && l[1] < l[0] && family_order(&l) >= 0.5 {
        "STANDS"
    } else if l[2] >= l[1] {
        "FALSIFIED"
    } else {
        "CONTRIBUTES"
    };
    println!(
        "W43C {id} FAMILY |L| E3 {:.5} E2 {:.5} E1 {:.5} N m; swirl energy {:.4} / {:.4} / {:.4} Pa; last-100 change of L {:.2e} / {:.2e} / {:.2e}; VERDICT {v}",
        l[0], l[1], l[2], out[0].f_end.1, out[1].f_end.1, out[2].f_end.1, out[0].last100.0, out[1].last100.0, out[2].last100.0
    );
    v
}

/// Rule (d) smoke of every arm set-up: 20 + 20 iterations (switch arms) or 40 iterations (set-up changes), finite and no guard trip.
#[test]
#[ignore = "ADR 0110 set-up smoke (about 1 min in release)"]
fn smoke_setups() {
    for (id, f) in [
        ("venkatakrishnan", &venkatakrishnan as &dyn Fn(&mut SimpleSolver)),
        ("upwind", &upwind),
        ("face_flux", &face_flux),
        ("tight", &tight),
        ("partner", &partner),
        ("unlimited", &unlimited),
    ] {
        arm(&format!("smoke {id} B3"), F3, true, "", false, None, 20, f, 20, true);
        arm(&format!("smoke {id} E3"), F3, false, "", false, None, 0, f, 40, false);
    }
    arm("smoke laminar B3", F3, true, "", true, None, 40, &none, 0, false);
    arm("smoke laminar E3", F3, false, "", true, None, 40, &none, 0, false);
    arm(
        "smoke outlet B3",
        F3,
        true,
        "",
        false,
        Some(OUTLET_X7),
        40,
        &none,
        0,
        false,
    );
    for (name, c) in [("E2", F2), ("E1", F1)] {
        arm(
            &format!("smoke unlimited {name}"),
            c,
            false,
            "",
            false,
            None,
            0,
            &unlimited,
            20,
            false,
        );
    }
}

/// S1 switch: freeze the Barth-Jespersen factors of the current iterate (the kernel's own call, so the first frozen iteration is identical
/// to an unfrozen one).
fn freeze(s: &mut SimpleSolver) {
    let g = s.geom.clone();
    let ub = s.u.boundary_values(&g);
    let u = &s.u.values;
    let psi = [0, 1, 2].map(|k| {
        let cells: Vec<f64> = u.iter().map(|v| v[k]).collect();
        let b: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let gr = rcfd_fv::gradient::least_squares(&g, &cells, &b);
        rcfd_fv::gradient::limiter(&g, &cells, &b, &gr, rcfd_fv::gradient::Limiter::BarthJespersen)
    });
    s.frozen_limiter = Some(psi);
}

/// ADR 0110 S1 on E3 (amendment 1): psi frozen at iteration 500 of the empty passage; registered prediction STANDS and both residuals below
/// 1e-5 within the 250 iterations after the switch.
#[test]
fn s1_e3_frozen_limiter_converges() {
    let (a, _) = arm("S1 E3", F3, false, "", false, None, 500, &freeze, 250, true);
    let (v, _) = verdict(&a);
    assert_eq!(v, "STANDS", "{a:?}");
    assert!(a.reached.is_some(), "tolerance 1e-5 not reached: {a:?}");
}

/// ADR 0110 S1 on B3: psi frozen at iteration 1000, 2000 more iterations.
#[test]
#[ignore = "ADR 0110 S1 B3 (about 2.5 min in release)"]
fn s1_b3_frozen_limiter() {
    let (v, a) = switch_arm("S1", &freeze, true);
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S1v (reported, not a discriminator): Venkatakrishnan(5) from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S1v B3 (about 2.5 min in release)"]
fn s1v_b3_venkatakrishnan() {
    switch_arm("S1v", &venkatakrishnan, false);
}

/// ADR 0110 S3: `velocity_correction = "face_flux"` from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S3 B3 (about 2.5 min in release)"]
fn s3_face_flux() {
    let (v, a) = switch_arm("S3", &face_flux, true);
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S4: laminar B3 from the start against the SST reference.
#[test]
#[ignore = "ADR 0110 S4 B3 (about 3.5 min in release)"]
fn s4_laminar() {
    assert_ne!(start_arm("S4", true, None), "NOT EVALUABLE");
}

/// ADR 0110 S5: `periodic_gradient = "partner_cell"` from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S5 B3 (about 2.5 min in release)"]
fn s5_partner() {
    let (v, a) = switch_arm("S5", &partner, true);
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S6: tight linear solves from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S6 B3 (about 3 min in release)"]
fn s6_tight() {
    let (v, a) = switch_arm("S6", &tight, true);
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S7: catalogue pressure outlet with closed reverse-flow faces, B3 from the start.
#[test]
#[ignore = "ADR 0110 S7 B3 (about 3.5 min in release)"]
fn s7_outlet() {
    assert_ne!(start_arm("S7", false, Some(OUTLET_X7)), "NOT EVALUABLE");
}

/// psi-weighted closed form of the ADR 0082 rigid-rotation torque (ADR 0110 L1), full annulus [N m]: internal faces only; the upwind cell's
/// factors `psi` (x, y) weight the first-order reversion `(1 - psi) Omega (z x (x_f - x_up))` of the rigid part. Also returns the unweighted
/// `C_up` (psi = 0) and the share of cells with psi_x or psi_y below 0.99.
fn c_psi(sim: &Simulation, psi: &[Vec<f64>; 3]) -> (f64, f64, f64) {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let (cc, cf) = (mesh.cell_centres(), mesh.face_centres());
    let phi = sim.flux();
    let om = omega();
    let k = N_BLADES * RHO;
    let (mut cpsi, mut cup) = (0.0, 0.0);
    for f in 0..g.n_internal {
        if g.empty[f] {
            continue;
        }
        let (p, n) = (own[f], nei[f]);
        let up = if phi[f] >= 0.0 { p } else { n };
        let s = vec3::sub(cf[f], cc[up]);
        let zs = [-s[1], s[0]];
        let d = vec3::sub(cc[p], cc[n]);
        let zx = |dx: f64, dy: f64| d[0] * dy - d[1] * dx;
        let du_psi = [(1.0 - psi[0][up]) * om * zs[0], (1.0 - psi[1][up]) * om * zs[1]];
        cpsi -= k * zx(phi[f] * du_psi[0], phi[f] * du_psi[1]);
        cup -= k * zx(phi[f] * om * zs[0], phi[f] * om * zs[1]);
    }
    let clipped = (0..mesh.n_cells())
        .filter(|&c| psi[0][c] < 0.99 || psi[1][c] < 0.99)
        .count() as f64
        / mesh.n_cells() as f64;
    (cpsi, cup, clipped)
}

/// ADR 0110 L1: the psi-weighted rigid-rotation torque on the settled default empty passage, E3 / E2 / E1 (600 iterations each).
#[test]
#[ignore = "ADR 0110 L1 (about 5 min in release)"]
fn l1_limited_rigid_torque_trend() {
    let mut rows = Vec::new();
    for (name, c) in [("E3", F3), ("E2", F2), ("E1", F1)] {
        let (a, mut sim) = arm(&format!("L1 {name}"), c, false, "", false, None, 599, &none, 0, false);
        let p0 = bj_factors(&sim);
        sim.step();
        let p1 = bj_factors(&sim);
        let pm = [0, 1, 2].map(|k| {
            p0[k]
                .iter()
                .zip(&p1[k])
                .map(|(a, b)| 0.5 * (a + b))
                .collect::<Vec<f64>>()
        });
        let (cp, cu, cl) = c_psi(&sim, &pm);
        let (l, e) = functionals(&sim, false);
        println!(
            "W43C L1 {name}: L {l:.6} N m, swirl energy {e:.4} Pa, last-100 change {:.2e}; C_psi {cp:.6} N m, C_up {cu:.5} N m, clipped share {cl:.4}",
            a.last100.0
        );
        rows.push((l, cp, a.last100.0));
    }
    let v = if rows.iter().any(|r| r.2 > 1e-3) {
        "NOT EVALUABLE"
    } else if rows[0].1 < rows[1].1 && rows[1].1 < rows[2].1 {
        "STANDS"
    } else if rows[2].1 < rows[1].1 {
        "FALSIFIED"
    } else {
        "CONTRIBUTES"
    };
    println!(
        "W43C L1 VERDICT {v}; (C_psi(E1) - C_psi(E3)) / (L(E1) - L(E3)) = {:.3}",
        (rows[2].1 - rows[0].1) / (rows[2].0 - rows[0].0)
    );
}

/// ADR 0110 L2: the empty-passage family with `periodic_gradient = "partner_cell"`.
#[test]
#[ignore = "ADR 0110 L2 (about 5 min in release)"]
fn l2_partner_family() {
    family("L2", &partner, false);
}

/// ADR 0110 L3: the laminar empty-passage family.
#[test]
#[ignore = "ADR 0110 L3 (about 5 min in release)"]
fn l3_laminar_family() {
    family("L3", &none, true);
}

/// `x_c = 2 Omega V_c / a_c` of every cell (ADR 0110 amendment 2, R-x): `a_c` the diagonal of the x-momentum convection (limited
/// `linear_upwind`, the kernel's operator) plus Laplacian (current `nu_face`) before relaxation, on the current state.
fn coriolis_gain(sim: &Simulation) -> Vec<f64> {
    let s = sim.simple_solver().unwrap();
    let g = &*s.geom;
    let ub = s.u.boundary_values(g);
    let cells: Vec<f64> = s.u.values.iter().map(|v| v[0]).collect();
    let b: Vec<f64> = ub.iter().map(|v| v[0]).collect();
    let bc = s.u.boundary_coeffs(g, 0);
    let gr = rcfd_fv::gradient::least_squares(g, &cells, &b);
    let psi = rcfd_fv::gradient::limiter(g, &cells, &b, &gr, rcfd_fv::gradient::Limiter::BarthJespersen);
    let lim: Vec<Vec3> = gr.iter().zip(&psi).map(|(v, k)| vec3::scale(*v, *k)).collect();
    let mc = rcfd_fv::convection::convection(g, &s.phi, &cells, &bc, s.settings.convection, Some(&lim));
    let gamma = s
        .nu_face
        .clone()
        .unwrap_or_else(|| rcfd_fv::diffusion::uniform_gamma(g, s.settings.nu));
    let ml = rcfd_fv::diffusion::laplacian(g, &gamma, &bc, Some(&gr));
    let vol = g.mesh.cell_volumes();
    (0..g.n_cells())
        .map(|c| 2.0 * omega() * vol[c] / (mc.a.diag[c] + ml.a.diag[c]).max(1e-300))
        .collect()
}

/// ADR 0110 R-x: the single-cell Coriolis gain on the B3 state at iteration 1000 and at the five largest-`du` cells of the S1 B3 record.
#[test]
#[ignore = "ADR 0110 R-x (about 1 min in release)"]
fn rx_coriolis_gain_record() {
    let (_, sim) = arm("R-x B3", F3, true, "", false, None, 1000, &none, 0, false);
    let x = coriolis_gain(&sim);
    let cc = sim.geom.mesh.cell_centres();
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|a, b| x[*b].partial_cmp(&x[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let over = x.iter().filter(|v| **v > 1.1).count();
    let mut line = String::new();
    for &c in idx.iter().take(10) {
        line += &format!(
            " [c{c} x {:.3} r {:.4} z {:.4}]",
            x[c],
            cc[c][0].hypot(cc[c][1]),
            cc[c][2]
        );
    }
    println!("W43C R-x: cells with x > 1.1: {over} of {}; largest:{line}", x.len());
    let top5 = [17usize, 665, 701, 341, 53];
    let hits = top5.iter().filter(|&&c| x[c] > 1.1).count();
    let listed: Vec<String> = top5.iter().map(|&c| format!("c{c} {:.3}", x[c])).collect();
    let v = if hits >= 3 {
        "SUPPORTED"
    } else if hits == 0 {
        "CONTRADICTED"
    } else {
        "NEITHER"
    };
    println!(
        "W43C R-x: S1 B3 top cells {}; {hits} of 5 above 1.1: {v}",
        listed.join(", ")
    );
}

fn pseudo(s: &mut SimpleSolver, dt: f64) {
    s.pseudo = Some(rcfd_fv::simple::PseudoTransient {
        mode: rcfd_fv::simple::PseudoMode::Physical { dt },
        factor: 1.0,
    });
}

/// ADR 0110 S8: pseudo time 1/Omega from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S8 B3 (about 2.5 min in release)"]
fn s8_pseudo_time() {
    let (v, a) = switch_arm("S8", &|s: &mut SimpleSolver| pseudo(s, 1.0 / omega()), true);
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S9: pseudo time 1/Omega and psi frozen from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S9 B3 (about 2.5 min in release)"]
fn s9_pseudo_time_and_frozen_limiter() {
    let (v, a) = switch_arm(
        "S9",
        &|s: &mut SimpleSolver| {
            pseudo(s, 1.0 / omega());
            freeze(s);
        },
        true,
    );
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S9' (amendment 3): pseudo time 0.1/Omega and psi frozen from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S9' B3 (about 2.5 min in release)"]
fn s9b_small_pseudo_time_and_frozen_limiter() {
    let (v, a) = switch_arm(
        "S9'",
        &|s: &mut SimpleSolver| {
            pseudo(s, 0.1 / omega());
            freeze(s);
        },
        true,
    );
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 L5 (amendment 2): the empty-passage family with psi frozen at iteration 300, 300 more iterations.
#[test]
#[ignore = "ADR 0110 L5 (about 5 min in release)"]
fn l5_frozen_limiter_family() {
    let mut out = Vec::new();
    for (name, c) in [("E3", F3), ("E2", F2), ("E1", F1)] {
        let (a, _) = arm(
            &format!("L5 {name}"),
            c,
            false,
            "",
            false,
            None,
            300,
            &freeze,
            300,
            false,
        );
        out.push(a);
    }
    let l: Vec<f64> = out.iter().map(|a| a.f_end.0.abs()).collect();
    let conv = out.iter().all(|a| a.reached.is_some());
    let order = family_order(&l);
    let v = if !conv {
        "NOT EVALUABLE"
    } else if l[2] < l[1] && l[1] < l[0] && order >= 0.5 {
        "STANDS"
    } else if l[2] >= l[1] {
        "FALSIFIED"
    } else {
        "CONTRIBUTES"
    };
    println!(
        "W43C L5 FAMILY |L| E3 {:.6} E2 {:.6} E1 {:.6} N m; swirl energy {:.4} / {:.4} / {:.4} Pa; reached {:?} / {:?} / {:?}; order {order:.3}; VERDICT {v}",
        l[0], l[1], l[2], out[0].f_end.1, out[1].f_end.1, out[2].f_end.1, out[0].reached, out[1].reached, out[2].reached
    );
}

/// ADR 0110 S10 (amendment 4): first-order `upwind` from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S10 B3 (about 2.5 min in release)"]
fn s10_upwind() {
    let (v, a) = switch_arm("S10", &upwind, false);
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 S11 (amendment 4): psi frozen plus `face_flux` from iteration 1000 on B3.
#[test]
#[ignore = "ADR 0110 S11 B3 (about 2.5 min in release)"]
fn s11_frozen_limiter_and_face_flux() {
    let (v, a) = switch_arm(
        "S11",
        &|s: &mut SimpleSolver| {
            freeze(s);
            face_flux(s);
        },
        true,
    );
    assert_ne!(v, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110 R-U (amendment 4): where unlimited `linear_upwind` fails on E3 (iterations 1-6, before the guard trip).
#[test]
#[ignore = "ADR 0110 R-U record (seconds)"]
fn ru_unlimited_location_record() {
    let case = Case::parse_toml(&case_toml(F3, false, "", false)).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    sim.simple_solver_mut().unwrap().linear_upwind_limiter = None;
    let (ang, tags) = cell_attributes(&sim);
    for it in 1..=6 {
        let before = sim.velocity().values.clone();
        let st = sim.step();
        let d = speed_change(&sim.velocity().values, &before);
        println!(
            "W43C R-U it {it}: res_u {:.3e} cont {:.3e} |du|_2 {:.3e}; {}",
            st.res_u,
            st.continuity,
            l2(&d),
            location(&sim, &d, &ang, &tags, 5)
        );
    }
}

/// ADR 0110 S12 (amendment 5): psi frozen at its per-cell, per-component minimum over iterations 1001-1010 on B3 (the kernel's own
/// factors of those iterations, collected here; frozen from iteration 1011 on), then to iteration 3000.
#[test]
#[ignore = "ADR 0110 S12 B3 (about 2.5 min in release)"]
fn s12_frozen_limiter_window_minimum() {
    let (r, mut sim) = arm("S12 reference B3", F3, true, "", false, None, 1000, &none, 0, false);
    let mut m = bj_factors(&sim);
    sim.step();
    for _ in 0..9 {
        let p = bj_factors(&sim);
        for k in 0..3 {
            for (a, b) in m[k].iter_mut().zip(&p[k]) {
                *a = a.min(*b);
            }
        }
        sim.step();
    }
    sim.simple_solver_mut().unwrap().frozen_limiter = Some(m);
    let mut hist = Vec::new();
    let mut prev = sim.velocity().values.clone();
    for _ in 0..1990 {
        prev = sim.velocity().values.clone();
        let st = sim.step();
        hist.push((st.res_u, st.res_p));
    }
    let tail = &hist[hist.len() - 500..];
    let a = Arm {
        ref_res: r.arm_res,
        arm_res: (
            median(tail.iter().map(|h| h.0).collect()),
            median(tail.iter().map(|h| h.1).collect()),
        ),
        probe: None,
        reached: hist.iter().position(|h| h.0 < 1e-5 && h.1 < 1e-5).map(|i| 1011 + i),
        last100: (f64::NAN, f64::NAN),
        du_ref: r.du_end,
        du_end: l2(&speed_change(&sim.velocity().values, &prev)),
        f_ref: r.f_end,
        f_end: functionals(&sim, true),
        converged_flag: sim.is_converged(),
        f_ref_max: (f64::NAN, f64::NAN),
        cu_end: f64::NAN,
        res_nut_end: f64::NAN,
        tail_below: false,
        conv_first: None,
    };
    println!("W43C S12 B3 RESULT: {a:?}");
    assert_ne!(verdict(&a).0, "NOT EVALUABLE", "{a:?}");
}

/// ADR 0110: the frozen-limiter hook is exact at the freeze: one iteration with `frozen_limiter` set to the factors of the current iterate
/// is bit-identical to one unfrozen iteration (E3 after 30 iterations; u, p and phi compared bit for bit).
#[test]
fn frozen_limiter_hook_is_bit_identical_at_the_freeze() {
    let case = Case::parse_toml(&case_toml(F3, false, "", false)).unwrap();
    let (mut a, mut b) = (Simulation::new(&case).unwrap(), Simulation::new(&case).unwrap());
    for _ in 0..30 {
        a.step();
        b.step();
    }
    freeze(b.simple_solver_mut().unwrap());
    a.step();
    b.step();
    let bits = |s: &Simulation| -> Vec<u64> {
        let v = s.velocity().values.iter().flat_map(|u| u.iter().map(|x| x.to_bits()));
        let p = s.pressure().values.iter().map(|x| x.to_bits());
        let f = s.flux().iter().map(|x| x.to_bits());
        v.chain(p).chain(f).collect()
    };
    assert_eq!(bits(&a), bits(&b));
}

/// Rule (d) smoke of the record on B3 and E3 (no switch): 500 iterations each (the first smoke ran 1000).
#[test]
#[ignore = "ADR 0110 smoke (about 2 min in release): cargo test --release -p rcfd-api --test fan01_stall_w43c smoke_record -- --ignored --nocapture"]
fn smoke_record() {
    arm("smoke B3", F3, true, "", false, None, 500, &none, 0, false);
    arm("smoke E3", F3, false, "", false, None, 500, &none, 0, false);
}

// ================================================================ ADR 0111 (wave 44A)

/// The five leading-edge root cells of the S1 B3 record (ADR 0110 amendment 2); c17 touches the hub and the blade (two wall faces).
const LE5: [usize; 5] = [17, 665, 701, 341, 53];

/// One outer iteration of the B3 frozen-limiter map as a function of the state alone (ADR 0111): the eddy viscosity, the lagged wall-pressure
/// quantities and the limiter factors are those of the reference state; the rotating-frame force is evaluated from the state's velocity as
/// `Simulation::step` does. State: u_x, u_y, u_z, p (cells), phi (non-empty faces).
struct Lin {
    sim: Simulation,
    src: rcfd_api::sources::Sources,
    nu_face: Vec<f64>,
    lagged: (Vec<f64>, Vec<f64>),
    faces: Vec<usize>,
}

impl Lin {
    fn nc(&self) -> usize {
        self.sim.geom.n_cells()
    }

    fn get(&self) -> Vec<f64> {
        let s = self.sim.simple_solver().unwrap();
        let mut x: Vec<f64> = Vec::new();
        for k in 0..3 {
            x.extend(s.u.values.iter().map(|u| u[k]));
        }
        x.extend_from_slice(&s.p.values);
        x.extend(self.faces.iter().map(|&f| s.phi[f]));
        x
    }

    fn map(&mut self, x: &[f64]) -> Vec<f64> {
        let n = self.nc();
        let s = self.sim.simple_solver_mut().unwrap();
        for c in 0..n {
            for k in 0..3 {
                s.u.values[c][k] = x[k * n + c];
            }
        }
        s.p.values.copy_from_slice(&x[3 * n..4 * n]);
        for (i, &f) in self.faces.iter().enumerate() {
            s.phi[f] = x[4 * n + i];
        }
        s.nu_face = Some(self.nu_face.clone());
        s.set_lagged_state(self.lagged.0.clone(), self.lagged.1.clone());
        s.body_force = self.src.evaluate(&s.geom, &s.u).0;
        s.iterate();
        s.history.clear();
        self.get()
    }

    /// Scale of every state entry (ADR 0084: velocity U = max(1, max|u|), pressure max(max|p|, U^2), flux U times the largest face area).
    fn scales(&self, x: &[f64]) -> Vec<f64> {
        let n = self.nc();
        let nf = self.faces.len();
        let amax = |a: &[f64]| a.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let u = amax(&x[..3 * n]).max(1.0);
        let p = amax(&x[3 * n..4 * n]).max(u * u);
        let sf = self.sim.geom.mesh.face_area_vectors();
        let area = self.faces.iter().map(|&f| vec3::norm(sf[f])).fold(0.0, f64::max);
        let mut d = vec![u; 3 * n];
        d.extend(std::iter::repeat(p).take(n));
        d.extend(std::iter::repeat(u * area).take(nf));
        d
    }
}

/// The B3 reference of ADR 0111: 1000 default iterations, the factors psi of that state, one real iteration (1001) for the eddy viscosity it
/// uses and for check M0, then the map with psi frozen and the tight linear solves of ADR 0110 S6. Returns (map, x_ref, psi).
fn lin_reference() -> (Lin, Vec<f64>, [Vec<f64>; 3]) {
    let (_, mut sim) = arm("W44A reference B3", F3, true, "", false, None, 1000, &none, 0, false);
    let case = Case::parse_toml(&case_toml(F3, true, "", false)).unwrap();
    let src = rcfd_api::sources::Sources::from_case(&case, &sim.geom, None)
        .unwrap()
        .unwrap();
    let s = sim.simple_solver_mut().unwrap();
    let lagged = s.lagged_state();
    freeze(s);
    let psi = s.frozen_limiter.take().unwrap();
    let faces: Vec<usize> = (0..sim.geom.mesh.n_faces()).filter(|&f| !sim.geom.empty[f]).collect();
    let mut lin = Lin {
        sim,
        src,
        nu_face: Vec::new(),
        lagged,
        faces,
    };
    let x_ref = lin.get();
    lin.sim.step();
    let x1 = lin.get();
    lin.nu_face = lin.sim.simple_solver().unwrap().nu_face.clone().unwrap();
    lin.sim.simple_solver_mut().unwrap().frozen_limiter = Some(psi.clone());
    let t = Instant::now();
    let f = lin.map(&x_ref);
    let dt_default = t.elapsed().as_secs_f64();
    let same = f.iter().zip(&x1).filter(|(a, b)| a.to_bits() == b.to_bits()).count();
    let dmax = f.iter().zip(&x1).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!(
        "W44A M0: F(x_ref) against the real iteration 1001: {same} of {} entries bit-identical, max abs difference {dmax:.3e} ({dt_default:.3} s)",
        f.len()
    );
    tight(lin.sim.simple_solver_mut().unwrap());
    let t = Instant::now();
    let f1 = lin.map(&x_ref);
    let dt = t.elapsed().as_secs_f64();
    let f2 = lin.map(&x_ref);
    let m2 = f1.iter().zip(&f2).all(|(a, b)| a.to_bits() == b.to_bits());
    println!(
        "W44A M2: tight map evaluated twice bit-identical: {m2} ({dt:.3} s per evaluation, {} state entries)",
        f1.len()
    );
    (lin, x_ref, psi)
}

/// Matrix-free Arnoldi (m steps, modified Gram-Schmidt with one reorthogonalisation) on `D^-1 J D`, J by central differences of the map
/// at `x` with step `h` times the entry scale. Writes `<tag>.h`, `<tag>.v` and `<tag>.meta` for `scripts/w44a_arnoldi_analysis.py`.
#[allow(clippy::approx_constant)] // the registered start vector is sin(0.7071 i), not sin(i / sqrt 2)
fn arnoldi(lin: &mut Lin, x: &[f64], m: usize, h: f64, tag: &str) {
    let t0 = Instant::now();
    let d = lin.scales(x);
    let n = x.len();
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let mut v0: Vec<f64> = (0..n).map(|i| (0.7071 * i as f64).sin()).collect();
    let n0 = dot(&v0, &v0).sqrt();
    v0.iter_mut().for_each(|a| *a /= n0);
    let mut basis = vec![v0];
    let mut hm = vec![0.0; (m + 1) * m];
    let mut steps = m;
    for j in 0..m {
        let v = &basis[j];
        let xp: Vec<f64> = (0..n).map(|i| x[i] + h * d[i] * v[i]).collect();
        let xm: Vec<f64> = (0..n).map(|i| x[i] - h * d[i] * v[i]).collect();
        let (fp, fm) = (lin.map(&xp), lin.map(&xm));
        let mut w: Vec<f64> = (0..n).map(|i| (fp[i] - fm[i]) / (2.0 * h * d[i])).collect();
        for _ in 0..2 {
            for (i, b) in basis.iter().enumerate() {
                let c = dot(&w, b);
                hm[i * m + j] += c;
                w.iter_mut().zip(b).for_each(|(a, y)| *a -= c * y);
            }
        }
        let nw = dot(&w, &w).sqrt();
        hm[(j + 1) * m + j] = nw;
        if !(nw > 1e-14) {
            steps = j + 1;
            break;
        }
        w.iter_mut().for_each(|a| *a /= nw);
        basis.push(w);
    }
    let dir =
        std::env::var("W44A_OUT").map_or_else(|_| std::env::temp_dir().join("w44a_jac"), std::path::PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let bytes = |v: &mut dyn Iterator<Item = f64>| v.flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    std::fs::write(dir.join(format!("{tag}.h")), bytes(&mut hm.iter().cloned())).unwrap();
    std::fs::write(
        dir.join(format!("{tag}.v")),
        bytes(&mut basis.iter().flatten().cloned()),
    )
    .unwrap();
    let nc = lin.nc();
    let mesh = &lin.sim.geom.mesh;
    let mut n5: Vec<usize> = LE5.to_vec();
    for &c in &LE5 {
        for &f in mesh.cell_faces(c) {
            if f < lin.sim.geom.n_internal {
                for o in [mesh.owner()[f], mesh.neighbour()[f]] {
                    if !n5.contains(&o) {
                        n5.push(o);
                    }
                }
            }
        }
    }
    let list = |v: &[usize]| v.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
    std::fs::write(
        dir.join(format!("{tag}.meta")),
        format!(
            "n {n}\nm {m}\nsteps {steps}\nn_cells {nc}\nn_faces {}\nh {h:e}\nle5 {}\nn5 {}\n",
            lin.faces.len(),
            list(&LE5),
            list(&n5)
        ),
    )
    .unwrap();
    println!(
        "W44A Arnoldi {tag}: {steps} steps, n {n}, h {h:e}, {:.1} s -> {}",
        t0.elapsed().as_secs_f64(),
        dir.display()
    );
}

/// The limited convection gradient of every cell at the reference state (psi times the kernel's least-squares gradient).
fn held_gradient(sim: &Simulation, psi: &[Vec<f64>; 3]) -> [Vec<Vec3>; 3] {
    let g = &sim.geom;
    let ub = sim.velocity().boundary_values(g);
    let u = &sim.velocity().values;
    [0, 1, 2].map(|k| {
        let cells: Vec<f64> = u.iter().map(|v| v[k]).collect();
        let b: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let gr = rcfd_fv::gradient::least_squares(g, &cells, &b);
        gr.iter().zip(&psi[k]).map(|(v, p)| vec3::scale(*v, *p)).collect()
    })
}

fn hold(cells: Vec<usize>, held: [Vec<Vec3>; 3]) -> rcfd_fv::simple::ConvGradientHook {
    Box::new(move |k, _: &[f64], _: &[f64], g: &mut [Vec3]| {
        for &c in &cells {
            g[c] = held[k][c];
        }
    })
}

/// Rule (d) smoke of the ADR 0111 linearisation: the reference, checks M0 and M2, the cost of one evaluation; no Arnoldi.
#[test]
#[ignore = "ADR 0111 smoke (about 1 min in release)"]
fn w44a_lin_smoke() {
    lin_reference();
}

/// ADR 0111 J0 and its attribution variants (H17, H5, GG17, Hall) and check M1 (J0 with h = 1e-7); m = 200 Arnoldi steps each.
#[test]
#[ignore = "ADR 0111 linearisation (several minutes in release); then python scripts/w44a_arnoldi_analysis.py"]
fn w44a_lin_base_and_attribution() {
    let (mut lin, x, psi) = lin_reference();
    let m = 200;
    // M1 companion record: the smallest |phi_f| / (U A_max) on the internal faces of the n5 cells
    {
        let d = lin.scales(&x);
        let (u, ua) = (d[0], d[4 * lin.nc()]);
        let mesh = &lin.sim.geom.mesh;
        let phi = lin.sim.flux();
        let mut fmin = f64::INFINITY;
        for &c in &LE5 {
            for &f in mesh.cell_faces(c) {
                if f < lin.sim.geom.n_internal {
                    let o = if mesh.owner()[f] == c {
                        mesh.neighbour()[f]
                    } else {
                        mesh.owner()[f]
                    };
                    for &g in mesh.cell_faces(o).iter().chain(mesh.cell_faces(c)) {
                        if g < lin.sim.geom.n_internal {
                            fmin = fmin.min(phi[g].abs());
                        }
                    }
                }
            }
        }
        println!("W44A M1 record: smallest |phi_f| / (U A_max) on the internal faces of the n5 cells {:.3e} (U {u:.3}); largest relative step h 1e-6",
            fmin / ua);
    }
    let f0 = lin.map(&x);
    arnoldi(&mut lin, &x, m, 1e-6, "J0");
    arnoldi(&mut lin, &x, m, 1e-7, "J0_h7");
    // the held gradients are those of x_ref: restore its velocity before computing them
    let n = lin.nc();
    {
        let s = lin.sim.simple_solver_mut().unwrap();
        for c in 0..n {
            for k in 0..3 {
                s.u.values[c][k] = x[k * n + c];
            }
        }
    }
    let held = held_gradient(&lin.sim, &psi);
    let variants: Vec<(&str, rcfd_fv::simple::ConvGradientHook)> = vec![
        ("H17", hold(vec![17], held.clone())),
        ("H5", hold(LE5.to_vec(), held.clone())),
        ("Hall", hold((0..n).collect(), held.clone())),
        ("GG17", {
            let geom = lin.sim.geom.clone();
            let p17 = [psi[0][17], psi[1][17], psi[2][17]];
            Box::new(move |k, c: &[f64], b: &[f64], g: &mut [Vec3]| {
                let gg = rcfd_fv::gradient::green_gauss(&geom, c, b, 0);
                g[17] = vec3::scale(gg[17], p17[k]);
            })
        }),
    ];
    for (tag, hook) in variants {
        lin.sim.simple_solver_mut().unwrap().conv_gradient_hook = Some(hook);
        let fv = lin.map(&x);
        let same = fv.iter().zip(&f0).all(|(a, b)| a.to_bits() == b.to_bits());
        println!("W44A M0' {tag}: F(x_ref) bit-identical to the unhooked map: {same}");
        arnoldi(&mut lin, &x, m, 1e-6, tag);
    }
}

/// ADR 0111 M3: two frozen-limiter trajectories (base map, 20 iterations) from x_ref and from x_ref + 1e-6 D v, v the leading Ritz vector of
/// J0 (`J0.lead` written by the analysis script); the geometric-mean growth of their scaled difference over k = 6..20 and its sign changes.
#[test]
#[ignore = "ADR 0111 M3 (about 1 min in release; needs J0.lead)"]
fn w44a_lin_m3() {
    let (mut lin, x, _) = lin_reference();
    let dir =
        std::env::var("W44A_OUT").map_or_else(|_| std::env::temp_dir().join("w44a_jac"), std::path::PathBuf::from);
    let raw = std::fs::read(dir.join("J0.lead")).unwrap();
    let v: Vec<f64> = raw
        .chunks(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(v.len(), x.len());
    {
        // where the leading mode lives (velocity energy per cell)
        let nc = lin.nc();
        let e: Vec<f64> = (0..nc)
            .map(|c| (0..3).map(|k| v[k * nc + c] * v[k * nc + c]).sum())
            .collect();
        let (ang, tags) = cell_attributes(&lin.sim);
        println!(
            "W44A M3 leading mode: {}",
            location(
                &lin.sim,
                &e.iter().map(|x| x.sqrt()).collect::<Vec<_>>(),
                &ang,
                &tags,
                8
            )
        );
    }
    let d = lin.scales(&x);
    let mut a = x.clone();
    let mut b: Vec<f64> = (0..x.len()).map(|i| x[i] + 1e-6 * d[i] * v[i]).collect();
    let mut prev: Option<Vec<f64>> = None;
    let (mut logs, mut neg, mut cnt) = (0.0, 0, 0);
    for k in 1..=20 {
        a = lin.map(&a);
        b = lin.map(&b);
        let dk: Vec<f64> = (0..x.len()).map(|i| (b[i] - a[i]) / d[i]).collect();
        let nk = l2(&dk);
        if let Some(p) = &prev {
            let r = nk / l2(p);
            let dot: f64 = dk.iter().zip(p).map(|(x, y)| x * y).sum();
            if k >= 6 {
                logs += r.ln();
                cnt += 1;
                if dot < 0.0 {
                    neg += 1;
                }
            }
            println!("W44A M3 k {k}: ||d_k|| {nk:.4e}, ratio {r:.5}, d_k . d_(k-1) {dot:+.3e}");
        }
        prev = Some(dk);
    }
    println!(
        "W44A M3: geometric-mean growth over k = 6..20: {:.5}; negative dot products {neg} of {cnt}",
        (logs / cnt as f64).exp()
    );
}

/// Rule G record of an arm (ADR 0111 part G).
fn rule_g(id: &str, a: &Arm) {
    let v = if !a.tail_below {
        "not evaluable"
    } else if a.cu_end < 1e-5 && a.res_nut_end >= 1e-5 {
        "DEFECT"
    } else if a.cu_end >= 1e-5 && !a.converged_flag {
        "NOT A DEFECT (the state is not stationary pointwise)"
    } else {
        "consistent"
    };
    println!(
        "W44A rule G {id}: residuals below 1e-5 over the last 500 iterations {}, c_u {:.3e}, res_nut {:.3e}, is_converged {}: {v}",
        a.tail_below, a.cu_end, a.res_nut_end, a.converged_flag
    );
}

/// ADR 0111 G1: the ADR 0110 S10 arm (B3, `upwind` from iteration 1000) with the rule G record.
#[test]
#[ignore = "ADR 0111 G1 (about 2.5 min in release)"]
fn w44a_g1_upwind() {
    let (a, _) = arm("G1 B3", F3, true, "", false, None, 1000, &upwind, 2000, false);
    rule_g("G1 B3", &a);
}

/// ADR 0111: a no-op convection-gradient hook leaves the solver bit-identical (E3, 30 iterations; u, p, phi).
#[test]
fn w44a_hooks_at_defaults_are_bit_identical() {
    let case = Case::parse_toml(&case_toml(F3, false, "", false)).unwrap();
    let (mut a, mut b) = (Simulation::new(&case).unwrap(), Simulation::new(&case).unwrap());
    b.simple_solver_mut().unwrap().conv_gradient_hook = Some(Box::new(|_, _: &[f64], _: &[f64], _: &mut [Vec3]| {}));
    for _ in 0..30 {
        a.step();
        b.step();
    }
    let bits = |s: &Simulation| -> Vec<u64> {
        let v = s.velocity().values.iter().flat_map(|u| u.iter().map(|x| x.to_bits()));
        let p = s.pressure().values.iter().map(|x| x.to_bits());
        let f = s.flux().iter().map(|x| x.to_bits());
        v.chain(p).chain(f).collect()
    };
    assert_eq!(bits(&a), bits(&b));
}

// ==================================================================================================================================
// ADR 0117 (wave 45C) block, appended: the ADR 0113 F1 (`coupled_offdiag_in_h1`) on the `rotating_frame` FAN-01 path. Nothing above
// this line is changed by ADR 0117. `step()` sets the flag every step, so a plain switch would be undone; the case key
// `solver.cyclic_partner_in_h1` missed F-a and was removed (rule b), and these tests keep F1 on through the test-only route
// `SimpleSolver::cyclic_partner_in_h1_test_only` (ADR 0117 amendment 2), set by the switch before iteration 1.
// ==================================================================================================================================

/// The ADR 0117 test-only route: F1 kept on by `step()` from the next iteration on.
fn w45c_route(s: &mut SimpleSolver) {
    s.cyclic_partner_in_h1_test_only = true;
}

/// The solver flag after two steps of the E3 case (the value `step()` gave it), with or without the route.
fn w45c_flag_after_two_steps(route: bool) -> bool {
    let case = Case::parse_toml(&case_toml(F3, false, "", false)).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    sim.simple_solver_mut().unwrap().cyclic_partner_in_h1_test_only = route;
    sim.step();
    sim.step();
    sim.simple_solver().unwrap().coupled_offdiag_in_h1
}

/// ADR 0117 rule (d) smoke (run on the key code before registration; now through the route): the flag inside `step()` on E3, and 40
/// iterations of E3 and B3 with F1 (finite, no guard trip).
#[test]
#[ignore = "ADR 0117 smoke (about 1 min in release)"]
fn w45c_smoke_key_on_the_fan_path() {
    println!(
        "W45C smoke: flag after step 2 with the route {}, without {}",
        w45c_flag_after_two_steps(true),
        w45c_flag_after_two_steps(false)
    );
    arm(
        "W45C smoke key E3",
        F3,
        false,
        "",
        false,
        None,
        0,
        &w45c_route,
        40,
        false,
    );
    arm(
        "W45C smoke key B3",
        F3,
        true,
        "",
        false,
        None,
        0,
        &w45c_route,
        40,
        false,
    );
}

/// ADR 0117 D-a after the removal (amendment 1): on E3 the solver flag after step 2 is false without the route and true with it (F1
/// reaches the kernel through `step()` on a mesh with cyclic faces). Bit-identity of the default rests on `x || false == x` and on
/// `baseline_hashes.rs`.
#[test]
fn w45c_da_route_reaches_step() {
    assert!(!w45c_flag_after_two_steps(false));
    assert!(w45c_flag_after_two_steps(true));
}

/// Observed order by the ADR 0086 fixed-point form (copied from `fan01_gci_w37c.rs`; Celik et al. 2008, recalled, UNVERIFIED);
/// f1 fine, f3 coarse. Returns (sign of eps32 / eps21, p).
fn w45c_observed_order(f1: f64, f2: f64, f3: f64, r21: f64, r32: f64) -> (f64, f64) {
    let (e21, e32) = (f2 - f1, f3 - f2);
    let s = (e32 / e21).signum();
    let mut p = 1.0f64;
    for _ in 0..500 {
        let q = ((r21.powf(p) - s) / (r32.powf(p) - s)).ln();
        let pn = ((e32 / e21).abs().ln() + q).abs() / r21.ln();
        if !pn.is_finite() {
            return (s, f64::NAN);
        }
        if (pn - p).abs() < 1e-12 {
            return (s, pn);
        }
        p = pn;
    }
    (s, f64::NAN)
}

/// ADR 0117 F family: E3, E2, E1 (600 iterations from the start) without and with F1 (registered run: the case key; now the test-only
/// route, which reproduced it, amendment 2); the harness check (default L
/// equals the ADR 0110 L1 values to the printed digits), F-a (registered limit), F-b (classification), and the record (rho_med against
/// the default arm, top-10 cells of |u_key - u_def|).
#[test]
#[ignore = "known miss (ADR 0117 F-a): |L| E3 0.010108 > 0.009618 and E2 0.060824 > 0.059910 N m; key removed, runs through the test-only route (about 8 min in release)"]
fn w45c_f_family_key_against_default_known_miss() {
    let l1_ref = [-0.009518, 0.059612, 0.105977];
    let mut fail: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    for (i, (name, c)) in [("E3", F3), ("E2", F2), ("E1", F1)].into_iter().enumerate() {
        let (d, sd) = arm(
            &format!("W45C default {name}"),
            c,
            false,
            "",
            false,
            None,
            0,
            &none,
            600,
            false,
        );
        let (k, sk) = arm(
            &format!("W45C key {name}"),
            c,
            false,
            "",
            false,
            None,
            0,
            &w45c_route,
            600,
            false,
        );
        let (ang, tags) = cell_attributes(&sk);
        let du = speed_change(&sk.velocity().values, &sd.velocity().values);
        let rho_med = (k.arm_res.0 / d.arm_res.0).max(k.arm_res.1 / d.arm_res.1);
        println!(
            "W45C {name}: default L {:.6} N m E_s {:.5} Pa (last-100 {:.2e}); key L {:.6} N m E_s {:.5} Pa (last-100 {:.2e}); rho_med {rho_med:.3e}; |u_key - u_def| {}",
            d.f_end.0, d.f_end.1, d.last100.0, k.f_end.0, k.f_end.1, k.last100.0, location(&sk, &du, &ang, &tags, 10)
        );
        if !((d.f_end.0 - l1_ref[i]).abs() <= 5e-7) {
            fail.push(format!(
                "harness: default {name} L {:.6} is not the ADR 0110 L1 value {}",
                d.f_end.0, l1_ref[i]
            ));
        }
        rows.push((d, k));
    }
    // F-a: a member whose default arm is not evaluable is dropped; default evaluable and key arm not evaluable is a miss
    let band_l = |l: f64| (0.005 * l.abs()).max(1e-4);
    let band_e = |e: f64| (0.005 * e).max(0.01);
    let mut evaluated = 0;
    for ((d, k), name) in rows.iter().zip(["E3", "E2", "E1"]) {
        if !(d.last100.0 <= 1e-3) {
            println!("W45C F-a {name}: default arm NOT EVALUABLE, dropped");
            continue;
        }
        evaluated += 1;
        let ok_l = k.f_end.0.abs() <= d.f_end.0.abs() + band_l(d.f_end.0);
        let ok_e = k.f_end.1 <= d.f_end.1 + band_e(d.f_end.1);
        let ok_k = k.last100.0 <= 1e-3;
        println!("W45C F-a {name}: key arm evaluable {ok_k}, |L| not worse {ok_l}, E_s not worse {ok_e}");
        if !(ok_k && ok_l && ok_e) {
            fail.push(format!("F-a {name}"));
        }
    }
    if evaluated == 0 {
        fail.push("F-a: no evaluable member".into());
    }
    // F-b (classification, not a limit)
    let settled = rows.iter().all(|(d, k)| d.last100.0 <= 1e-3 && k.last100.0 <= 1e-3);
    let effect = rows
        .iter()
        .any(|(d, k)| (k.f_end.0 - d.f_end.0).abs() > band_l(d.f_end.0));
    let l: Vec<f64> = rows.iter().map(|(_, k)| k.f_end.0.abs()).collect();
    let (s, p) = w45c_observed_order(l[2], l[1], l[0], 1.4073, 1.4017);
    let growth = settled && l[0] > l[1] && l[1] > l[2] && p >= 0.5;
    let fb = if !settled {
        "NOT EVALUABLE"
    } else if growth {
        "EXPLAINS THE GROWTH"
    } else if effect {
        "CONTRIBUTES"
    } else {
        "NO EFFECT"
    };
    println!(
        "W45C F-b: key |L| E3 {:.6} E2 {:.6} E1 {:.6} N m, observed order {p:.3} (sign {s}); {fb}; F-a and harness {}",
        l[0],
        l[1],
        l[2],
        if fail.is_empty() { "pass" } else { "MISS" }
    );
    assert!(fail.is_empty(), "{fail:?}");
}

/// ADR 0117 B3 (reported, no limit): the ADR 0110 set-up arm, the reference (1000 iterations, window 500-1000) against the key from the
/// start (3000, window 2500-3000), with the ADR 0110 verdict wording, torque and Delta p_ts (default stalled 1.849 N m / 46.6 Pa).
#[test]
#[ignore = "ADR 0117 B3, reported (about 3 min in release); through the test-only route since the key was removed"]
fn w45c_b3_key_from_the_start() {
    let (r, _) = arm("W45C reference B3", F3, true, "", false, None, 1000, &none, 0, false);
    let (mut a, _) = arm("W45C key B3", F3, true, "", false, None, 0, &w45c_route, 3000, false);
    a.ref_res = r.arm_res;
    a.du_ref = r.du_end;
    let (v, x) = verdict(&a);
    println!(
        "W45C B3 VERDICT {v} (largest median ratio {x:.3e}); torque {:.4} N m, Delta p_ts {:.2} Pa (reference {:.4} / {:.2})",
        a.f_end.0, a.f_end.1, r.f_end.0, r.f_end.1
    );
}
// ================================================================================================================ end of ADR 0117 block

// ================================================================ ADR 0115 (wave 45A): the range-scaled Venkatakrishnan limiter
// Everything below this line belongs to ADR 0115 (registered there before the registered runs).

/// FNV-1a (64 bit) over the little-endian bytes of the u, p and phi bit patterns (the `baseline_hashes.rs` hash).
fn w45a_hash(sim: &Simulation) -> u64 {
    let v = sim.velocity().values.iter().flat_map(|u| u.iter().map(|x| x.to_bits()));
    let p = sim.pressure().values.iter().map(|x| x.to_bits());
    let f = sim.flux().iter().map(|x| x.to_bits());
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in v.chain(p).chain(f).flat_map(u64::to_le_bytes) {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// ADR 0115 P5: the key absent, `none` and `barth_jespersen` give B3 after 30 iterations bit-identical to main 511bed4 (hashes recorded there).
#[test]
fn w45a_p5_defaults_are_bit_identical_to_main() {
    let mut miss = Vec::new();
    for (lim, want) in [
        ("", 0xec72_1116_c096_3182u64),
        ("linear_upwind_limiter = \"none\"\n", 0xec72_1116_c096_3182),
        ("linear_upwind_limiter = \"barth_jespersen\"\n", 0x26e5_a0c5_2495_7698),
    ] {
        let text = case_toml(F3, true, "", false).replace("linear_upwind_limiter = \"barth_jespersen\"\n", lim);
        let mut sim = Simulation::new(&Case::parse_toml(&text).unwrap()).unwrap();
        for _ in 0..30 {
            sim.step();
        }
        let h = w45a_hash(&sim);
        println!(
            "W45A P5 [{}]: {h:#018x} (guard {:?})",
            lim.trim(),
            sim.steady_divergence().map(|d| d.kind)
        );
        if h != want {
            miss.push(lim.trim());
        }
    }
    assert!(miss.is_empty(), "P5: {miss:?}");
}

/// The ADR 0115 switch: the range-scaled Venkatakrishnan limiter with constant `k`.
fn w45a_range(k: f64) -> impl Fn(&mut SimpleSolver) {
    move |s: &mut SimpleSolver| s.linear_upwind_limiter = Some(rcfd_fv::gradient::Limiter::VenkatakrishnanRange(k))
}

/// The range R = max - min over the cells of each velocity component of the solver's current iterate.
fn w45a_ranges(s: &SimpleSolver) -> [f64; 3] {
    [0, 1, 2].map(|k| {
        let v = s.u.values.iter().map(|u| u[k]);
        v.clone().fold(f64::NEG_INFINITY, f64::max) - v.fold(f64::INFINITY, f64::min)
    })
}

/// ADR 0115 rule-d smoke: R at iteration 1000 of the default (Barth-Jespersen) run on E3, E2, E1, B3, and 20 + 20 iterations at each
/// registered k on each member (finite, no guard trip).
#[test]
#[ignore = "ADR 0115 smoke (about 15 min in release)"]
fn w45a_smoke() {
    for (name, c, blade) in [
        ("E3", F3, false),
        ("E2", F2, false),
        ("E1", F1, false),
        ("B3", F3, true),
    ] {
        let show = |s: &mut SimpleSolver| {
            let r = w45a_ranges(s);
            println!(
                "W45A smoke {name}: R at the switch (u_x, u_y, u_z) {:.4} / {:.4} / {:.4} m/s",
                r[0], r[1], r[2]
            );
            w45a_range(0.01)(s);
        };
        arm(
            &format!("w45a smoke R {name}"),
            c,
            blade,
            "",
            false,
            None,
            1000,
            &show,
            20,
            false,
        );
        for k in [0.002, 0.01, 0.05] {
            let (_, sim) = arm(
                &format!("w45a smoke k {k} {name}"),
                c,
                blade,
                "",
                false,
                None,
                20,
                &w45a_range(k),
                20,
                false,
            );
            println!(
                "W45A smoke k {k} {name}: iterations {}, guard {:?}",
                sim.iteration(),
                sim.steady_divergence_message()
            );
        }
    }
}

/// One registered Z arm of ADR 0115 (1000 default iterations, the limiter with `k`, 2000 more): the list of missed parts of (a) `rho_med`,
/// `rho_du` <= 0.01, (b) `is_converged()` at the end, (c) the probe (one more iteration, same limiter) `res_u`, `res_p` <= 1e-5, (d) on
/// the empty passage the one-sided exact-swirl check against `lc_ec` (the stalled cycle's worst |L| and swirl energy, ADR 0111).
fn w45a_z(name: &str, c: Counts, blade: bool, k: f64, lc_ec: Option<(f64, f64)>) -> Vec<String> {
    let label = format!("W45A Z k {k} {name}");
    let (a, mut sim) = arm(&label, c, blade, "", false, None, 1000, &w45a_range(k), 2000, false);
    let (_, rho_med) = verdict(&a);
    let rho_du = a.du_end / a.du_ref;
    let r = w45a_ranges(sim.simple_solver().unwrap());
    let mut miss = Vec::new();
    if let Some(m) = sim.steady_divergence_message() {
        miss.push(format!("guard trip: {m}"));
    }
    let pr = sim.step();
    if !(rho_med <= 0.01 && rho_du <= 0.01) {
        miss.push(format!("(a) rho_med {rho_med:.3e} rho_du {rho_du:.3e}"));
    }
    if !a.converged_flag {
        miss.push("(b) is_converged() false at the end".into());
    }
    if !(pr.res_u <= 1e-5 && pr.res_p <= 1e-5) {
        miss.push(format!("(c) probe {:.3e} / {:.3e}", pr.res_u, pr.res_p));
    }
    if let Some((lc, ec)) = lc_ec {
        let (l, e) = a.f_end;
        if !(l.abs() <= lc + (0.005 * lc).max(1e-4) && e <= ec + (0.005 * ec).max(0.01)) {
            miss.push(format!("(d) |L| {:.6} (L_c {lc}), E_s {e:.5} (E_c {ec})", l.abs()));
        }
    }
    println!(
        "{label}: rho_med {rho_med:.3e}, rho_du {rho_du:.3e}, tolerance reached {:?}, is_converged first {:?} / end {}, probe {:.3e} / {:.3e}, functionals at the switch {:?} at the end {:?} (window worst {:?}), R at the end {:.3} / {:.3} / {:.3}, c_u {:.3e}, res_nut {:.3e}; MISSES {miss:?}",
        a.reached, a.conv_first, a.converged_flag, pr.res_u, pr.res_p, a.f_ref, a.f_end, a.f_ref_max, r[0], r[1], r[2], a.cu_end, a.res_nut_end
    );
    miss
}

/// ADR 0115 Z1 at one k: E3, E2, E1 (each member run whatever the previous one gave), with the ADR 0111 reference values.
fn w45a_z1(k: f64) {
    let mut miss = Vec::new();
    for (name, c, lc_ec) in [
        ("E3", F3, (0.009518, 6.26632)),
        ("E2", F2, (0.059612, 9.23040)),
        ("E1", F1, (0.105977, 5.48377)),
    ] {
        for m in w45a_z(name, c, false, k, Some(lc_ec)) {
            miss.push(format!("{name} {m}"));
        }
    }
    println!("W45A Z1 k {k}: {}", if miss.is_empty() { "PASS" } else { "MISS" });
    assert!(miss.is_empty(), "Z1 k {k}: {miss:?}");
}

/// ADR 0115 Z2 at one k: B3; the torque and Delta p_ts are printed by the arm (no limit).
fn w45a_z2(k: f64) {
    let miss = w45a_z("B3", F3, true, k, None);
    println!("W45A Z2 k {k}: {}", if miss.is_empty() { "PASS" } else { "MISS" });
    assert!(miss.is_empty(), "Z2 k {k}: {miss:?}");
}

#[test]
#[ignore = "ADR 0115 Z1 k 0.002: registered known miss (about 20 min in release)"]
fn w45a_z1_k0002_known_miss() {
    w45a_z1(0.002);
}

#[test]
#[ignore = "ADR 0115 Z1 k 0.01: registered known miss (about 20 min in release)"]
fn w45a_z1_k001_known_miss() {
    w45a_z1(0.01);
}

#[test]
#[ignore = "ADR 0115 Z1 k 0.05: registered known miss (about 20 min in release)"]
fn w45a_z1_k005_known_miss() {
    w45a_z1(0.05);
}

#[test]
#[ignore = "ADR 0115 Z2 k 0.002: not run (registered only where Z1 passes; Z1 missed at every k)"]
fn w45a_z2_k0002() {
    w45a_z2(0.002);
}

#[test]
#[ignore = "ADR 0115 Z2 k 0.01: not run (registered only where Z1 passes; Z1 missed at every k)"]
fn w45a_z2_k001() {
    w45a_z2(0.01);
}

#[test]
#[ignore = "ADR 0115 Z2 k 0.05: not run (registered only where Z1 passes; Z1 missed at every k)"]
fn w45a_z2_k005() {
    w45a_z2(0.05);
}
