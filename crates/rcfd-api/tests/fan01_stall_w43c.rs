//! Wave 43C (ADR 0110): why the FAN-01 rotor-passage steady run does not converge. The ADR 0086 set-up (C-grid `camber_seam_h`, SST,
//! SIMPLEC, Barth-Jespersen-limited `linear_upwind`, `rotating_frame`) on the coarsest family member F3 (bladed B3, empty E3), a per-iteration
//! location record, and one switch per registered mechanism applied from the same stalled state. Everything was registered in ADR 0110 before
//! the registered runs. No agreement with measured fan data is claimed.

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
    let du_end = l2(&speed_change(&sim.velocity().values, &prev[prev.len() - 1]));
    let converged_flag = sim.is_converged();
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
    } else if l[2] < l[1] && l[2] <= 0.053 {
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
    // ADR 0086 ratios of the empty family (fine first)
    let (r21, r32) = (1.4073f64, 1.4017f64);
    let order = {
        let (e21, e32) = (l[1] - l[2], l[0] - l[1]);
        let sgn = (e32 / e21).signum();
        let mut p = 1.0f64;
        for _ in 0..500 {
            let q = ((r21.powf(p) - sgn) / (r32.powf(p) - sgn)).ln();
            p = ((e32 / e21).abs().ln() + q).abs() / r21.ln();
        }
        p
    };
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
