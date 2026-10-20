//! Wave 36C (ADR 0082): where the spurious absolute angular momentum of the relative-velocity rotating frame on the FAN-01 H-slit sector is
//! generated (a per-cell, per-term angular-momentum budget of the converged empty passage), and the registered checks of the opt-in
//! absolute-velocity rotor passage. Everything here was registered in ADR 0082 before it was written. No fan performance claim is made.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::diffusion::{laplacian, uniform_gamma};
use rcfd_fv::gradient::least_squares;
use rcfd_fv::pressure::{boundary_values_pbc, pressure_bc};
use std::f64::consts::PI;
use std::time::Instant;

/// FAN-01 sections (as `fan01_diagnosis_w35c.rs`): r [mm], chord [mm], stagger [deg], camber, camber position, thickness, LE x [mm], LE theta [deg].
const SECTIONS: [[f64; 8]; 16] = [
    [124.0, 98.92, 43.63, 0.0364, 0.53, 0.1086, -23.70, -15.93],
    [127.0, 96.38, 44.81, 0.0365, 0.53, 0.1088, -22.40, -16.34],
    [130.0, 93.84, 45.99, 0.0366, 0.53, 0.1090, -21.10, -16.75],
    [140.0, 85.99, 49.94, 0.0369, 0.52, 0.1094, -17.05, -18.05],
    [150.0, 79.23, 53.26, 0.0372, 0.52, 0.1097, -13.80, -19.26],
    [160.0, 73.68, 55.89, 0.0374, 0.52, 0.1100, -11.35, -20.33],
    [170.0, 69.41, 58.20, 0.0375, 0.52, 0.1102, -9.43, -21.21],
    [180.0, 65.89, 60.21, 0.0375, 0.52, 0.1104, -7.87, -21.95],
    [190.0, 62.88, 61.97, 0.0376, 0.52, 0.1106, -6.59, -22.60],
    [200.0, 60.51, 63.57, 0.0376, 0.52, 0.1107, -5.53, -23.14],
    [210.0, 58.57, 65.04, 0.0376, 0.52, 0.1109, -4.62, -23.60],
    [220.0, 57.11, 66.39, 0.0376, 0.52, 0.1110, -3.87, -23.98],
    [230.0, 56.19, 67.67, 0.0376, 0.52, 0.1111, -3.23, -24.29],
    [240.0, 55.49, 68.83, 0.0376, 0.515, 0.1111, -2.68, -24.55],
    [245.0, 55.08, 69.35, 0.0376, 0.515, 0.1111, -2.43, -24.69],
    [247.5, 54.87, 69.60, 0.0376, 0.515, 0.1112, -2.29, -24.76],
];

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_TIP: f64 = 0.2475;
const R_DUCT: f64 = 0.250;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), UNVERIFIED
const NU: f64 = 1.5e-5;
const RPM: f64 = 1486.0;
const Q_DESIGN: f64 = 1.4032;
const WALL_CLOCK_MIN: f64 = 60.0;
/// Hub trailing-edge plane relative to `z_le_root` (ADR 0078 harness value, 0.072 m).
const HUB_CHORD_Z: f64 = 0.072;

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

fn list(f: impl Fn(&[f64; 8]) -> f64) -> String {
    let v: Vec<String> = SECTIONS.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// Velocity formulation of an arm.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Frame {
    /// ADR 0078 X2a / E0: `rotating_frame`, relative velocities, hub and shroud `moving_wall` at `-Omega` with the projection (empty passage).
    Relative,
    /// ADR 0078 X2b: no source, absolute velocities, hub and shroud at rest.
    Inertial,
}

/// Mesh counts of an arm (ADR 0082 M1-M3; M1 = the ADR 0078 X2 / E0 counts).
#[derive(Clone, Copy, Debug)]
struct Counts {
    n_up: usize,
    n_chord: usize,
    n_down: usize,
    n_side: usize,
    n_span: usize,
    n_gap: usize,
    n_mid: usize,
    n_bl: usize,
    h1: f64,
    growth: f64,
}

fn m1() -> Counts {
    Counts {
        n_up: 8,
        n_chord: 12,
        n_down: 8,
        n_side: 10,
        n_span: 16,
        n_gap: 7,
        n_mid: 1,
        n_bl: 2,
        h1: 2.0e-4,
        growth: 1.2,
    }
}

/// Amendment 3: one step coarser than M1 by 1.5 (replaces the defective M3 of the registered family).
fn m0() -> Counts {
    Counts {
        n_up: 5,
        n_chord: 8,
        n_down: 5,
        n_side: 7,
        n_span: 11,
        n_gap: 5,
        n_mid: 1,
        n_bl: 1,
        h1: 2.0e-4 * 1.5,
        growth: 1.2f64.powf(1.5),
    }
}

fn m2() -> Counts {
    Counts {
        n_up: 12,
        n_chord: 18,
        n_down: 12,
        n_side: 15,
        n_span: 24,
        n_gap: 11,
        n_mid: 2,
        n_bl: 3,
        h1: 2.0e-4 / 1.5,
        growth: 1.2f64.powf(1.0 / 1.5),
    }
}

fn m3() -> Counts {
    Counts {
        n_up: 18,
        n_chord: 27,
        n_down: 18,
        n_side: 23,
        n_span: 36,
        n_gap: 16,
        n_mid: 2,
        n_bl: 5,
        h1: 2.0e-4 / 2.25,
        growth: 1.2f64.powf(1.0 / 2.25),
    }
}

#[derive(Clone, Debug)]
struct Arm {
    label: String,
    counts: Counts,
    include_blade: bool,
    frame: Frame,
    convection: &'static str,
    tolerance: f64,
}

fn arm(label: &str, counts: Counts, include_blade: bool, frame: Frame, convection: &'static str) -> Arm {
    Arm {
        label: label.into(),
        counts,
        include_blade,
        frame,
        convection,
        tolerance: 1e-5,
    }
}

fn mesh_toml(a: &Arm) -> String {
    let (le_x0, le_t0) = (SECTIONS[0][6], SECTIONS[0][7]);
    let c = a.counts;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = {R_HUB}\nr_tip = {R_TIP}\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
n_up = {}\nn_chord = {}\nn_down = {}\nn_side = {}\nn_mid = {}\nn_span = {}\nn_gap = {}\nn_bl = {}\ngrowth = {:?}\nfirst_cell_height = {:?}\n{}",
        list(|s| s[0] * 1e-3),
        list(|s| s[1] * 1e-3),
        list(|s| s[2]),
        list(|s| s[3]),
        list(|s| s[4]),
        list(|s| s[5]),
        list(|s| (s[6] - le_x0) * 1e-3),
        list(|s| s[0] * 1e-3 * (s[7] - le_t0).to_radians()),
        c.n_up,
        c.n_chord,
        c.n_down,
        c.n_side,
        c.n_mid,
        c.n_span,
        c.n_gap,
        c.n_bl,
        c.growth,
        c.h1,
        if a.include_blade { "" } else { "include_blade = false\n" },
    )
}

/// The ADR 0078 E0 / X2a / X2b cases (same text for M1 and the relative or inertial frame), and the ADR 0082 F-RP variants.
fn case_toml(a: &Arm) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let projected = format!("velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n", -w);
    let (shroud, hub, rel, frame) = match a.frame {
        Frame::Relative => (
            if a.include_blade {
                format!("velocity = \"fixed\"\nvelocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\nturbulence = \"wall\"\n")
            } else {
                projected.clone()
            },
            if a.include_blade { "velocity = \"fixed\"\n".to_string() } else { projected.clone() },
            format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n"),
            format!("[sources.frame]\ntype = \"rotating_frame\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = {w:?}\ncentrifugal = true\n"),
        ),
        Frame::Inertial => (
            "velocity = \"fixed\"\nturbulence = \"wall\"\n".to_string(),
            "velocity = \"fixed\"\n".to_string(),
            String::new(),
            String::new(),
        ),
    };
    let blade = if !a.include_blade {
        String::new()
    } else {
        "[boundaries.blade]\nvelocity = \"fixed\"\n[reports.m_blade]\ntype = \"moment\"\npatch = \"blade\"\ndirection = [0.0, 0.0, 1.0]\n".to_string()
    };
    format!(
        r#"{}
[physics]
nu = {NU:?}
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
{hub}[boundaries.shroud]
{shroud}{frame}{blade}[solver]
algorithm = "simplec"
convection = "{}"
max_iterations = 8000
tolerance_u = {:?}
tolerance_p = {:?}
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
        mesh_toml(a),
        a.convection,
        a.tolerance,
        a.tolerance
    )
}

// ---------------------------------------------------------------- surface statistics (ADR 0078 definitions)

struct Sample {
    x: Vec3,
    c: Vec3,
    p: f64,
    phi: f64,
}

fn absolute(w: Vec3, x: Vec3, om: f64) -> Vec3 {
    [w[0] - om * x[1], w[1] + om * x[0], w[2]]
}

fn polar(c: Vec3, x: Vec3) -> (f64, f64, f64) {
    let r = x[0].hypot(x[1]);
    ((x[0] * c[0] + x[1] * c[1]) / r, (-x[1] * c[0] + x[0] * c[1]) / r, r)
}

fn patch_samples(sim: &Simulation, name: &str, om: f64) -> Vec<Sample> {
    let g = &sim.geom;
    let ni = g.n_internal;
    let ub = sim.velocity().boundary_values(g);
    let pb = sim.boundary_field("p").unwrap();
    let phi = sim.flux();
    let (cf, sf) = (g.mesh.face_centres(), g.mesh.face_area_vectors());
    g.mesh
        .patch(name)
        .unwrap()
        .faces()
        .map(|f| {
            let s = if sf[f][2] >= 0.0 { 1.0 } else { -1.0 };
            Sample {
                x: cf[f],
                c: absolute(ub[f - ni], cf[f], om),
                p: pb[f - ni],
                phi: s * phi[f],
            }
        })
        .collect()
}

fn cut_samples(sim: &Simulation, z0: f64, om: f64) -> Vec<Sample> {
    let g = &sim.geom;
    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
    let (cf, cc) = (g.mesh.face_centres(), g.mesh.cell_centres());
    let u = &sim.velocity().values;
    let p = sim.field("p").unwrap();
    let phi = sim.flux();
    let mut out = Vec::new();
    for f in 0..g.n_internal {
        let (o, n) = (own[f], nei[f]);
        let s = if cc[o][2] < z0 && cc[n][2] >= z0 {
            1.0
        } else if cc[o][2] >= z0 && cc[n][2] < z0 {
            -1.0
        } else {
            continue;
        };
        let w = vec3::scale(vec3::add(u[o], u[n]), 0.5);
        out.push(Sample {
            x: cf[f],
            c: absolute(w, cf[f], om),
            p: 0.5 * (p[o] + p[n]),
            phi: s * phi[f],
        });
    }
    out
}

#[derive(Debug, Clone, Copy, Default)]
struct Stats {
    q: f64,
    pt_mass: f64,
    kth: f64,
    l_flux: f64,
}

fn stats(s: &[Sample]) -> Stats {
    let q: f64 = s.iter().map(|v| v.phi).sum();
    let m = |f: &dyn Fn(&Sample) -> f64| s.iter().map(|v| f(v) * v.phi).sum::<f64>() / q;
    let kz = m(&|v| 0.5 * v.c[2] * v.c[2]);
    let kth = m(&|v| 0.5 * polar(v.c, v.x).1.powi(2));
    let kr = m(&|v| 0.5 * polar(v.c, v.x).0.powi(2));
    Stats {
        q: N_BLADES * q,
        pt_mass: RHO * (m(&|v| v.p) + kz + kth + kr),
        kth: RHO * kth,
        l_flux: N_BLADES
            * RHO
            * s.iter()
                .map(|v| {
                    let (_, t, r) = polar(v.c, v.x);
                    r * t * v.phi
                })
                .sum::<f64>(),
    }
}

// ---------------------------------------------------------------- one run with the ADR 0078 stop rules

#[derive(Debug, Clone, Default)]
struct Outcome {
    converged: bool,
    iterations: usize,
    l_out: f64,
    l_excess: f64,
    dp_tt: f64,
    kth_out: f64,
    l_cut_u: f64,
}

fn build(a: &Arm) -> Simulation {
    let case = Case::parse_toml(&case_toml(a)).unwrap_or_else(|e| panic!("{}: {e:?}", a.label));
    Simulation::new(&case).unwrap_or_else(|e| panic!("{}: {e:?}", a.label))
}

/// Run to convergence (or a stop rule) and print the ADR 0078 quantities; returns the simulation for the budget.
fn run(a: &Arm, max_iter: usize) -> (Outcome, Simulation) {
    let mut sim = build(a);
    let n_cells = sim.geom.mesh.n_cells();
    let t0 = Instant::now();
    let mut n = 0;
    let mut stop = None;
    while n < max_iter && !sim.is_done() {
        let st = sim.step();
        n += 1;
        if n <= 30 && (n % 10 == 0 || n == 1) {
            println!(
                "W36C {} smoke it {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e}",
                a.label, st.continuity, st.res_u, st.res_p
            );
        }
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) || st.continuity > 1e6 {
            stop = Some(format!("diverged: continuity {:.3e} at iteration {n}", st.continuity));
            break;
        }
        if let Some(d) = sim.steady_divergence() {
            stop = Some(format!("diverged: steady guard {d:?}"));
            break;
        }
        if t0.elapsed().as_secs_f64() > 60.0 * WALL_CLOCK_MIN {
            stop = Some(format!("NOT CONVERGED (time): wall-clock cap at iteration {n}"));
            break;
        }
    }
    let minutes = t0.elapsed().as_secs_f64() / 60.0;
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    let om_f = if a.frame == Frame::Relative { w } else { 0.0 };
    let m_blade = if a.include_blade { mon("m_blade") } else { 0.0 };
    let torque = -N_BLADES * (m_blade + mon("m_hub"));
    let torque_shroud = -N_BLADES * mon("m_shroud");
    let dp_ts_34d = RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in));
    let converged = stop.is_none() && sim.failure().is_none() && sim.is_converged();
    println!(
        "W36C {} RESULT: {n_cells} cells, {n} iterations, {minutes:.1} min, converged {converged} ({})",
        a.label,
        stop.clone().unwrap_or_else(|| format!("{:?}", sim.last()))
    );
    let s_in = stats(&patch_samples(&sim, "inlet", om_f));
    let s_out = stats(&patch_samples(&sim, "outlet", om_f));
    let s_u = stats(&cut_samples(&sim, 0.1 - 0.04, om_f));
    let dp_tt = s_out.pt_mass - s_in.pt_mass;
    let l_excess = (s_out.l_flux - s_in.l_flux) - (torque + torque_shroud);
    println!(
        "W36C {} ADR0075-defs: Delta p_ts {dp_ts_34d:.3} Pa; torque blade+hub {torque:.5} N m, shroud {torque_shroud:.5} N m",
        a.label
    );
    println!(
        "W36C {} outlet L {:.5} N m (inlet {:.5}, cut U {:.5}), L excess {l_excess:.5} N m, outlet rho<cth2/2> {:.3} Pa, Delta p_tt (mass-averaged) {dp_tt:.3} Pa, Q {:.6}",
        a.label, s_out.l_flux, s_in.l_flux, s_u.l_flux, s_out.kth, s_in.q
    );
    (
        Outcome {
            converged,
            iterations: n,
            l_out: s_out.l_flux,
            l_excess,
            dp_tt,
            kth_out: s_out.kth,
            l_cut_u: s_u.l_flux,
        },
        sim,
    )
}

// ---------------------------------------------------------------- the angular-momentum budget (ADR 0082 "Hypotheses")

/// z-component of `a x b`.
fn zx(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn rperp2(r: Vec3) -> f64 {
    r[0] * r[0] + r[1] * r[1]
}

/// Face fluxes (outward from the owner) of the three operators of component `k`, rebuilt from the converged state, and the check
/// against the kernel's operators. Returns (convection, Laplacian (LHS sign), largest relative row mismatch).
struct Rebuilt {
    conv: Vec<Vec3>,
    visc: Vec<Vec3>,
    worst: f64,
}

fn rebuild_fluxes(sim: &Simulation) -> Rebuilt {
    let s = sim.simple_solver().expect("steady SIMPLE solver");
    let g = &*s.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let n_int = g.n_internal;
    let nf = mesh.n_faces();
    let phi: Vec<f64> = match &s.conv_flux_offset {
        Some(o) => s.phi.iter().zip(o).map(|(p, o)| p - o).collect(),
        None => s.phi.clone(),
    };
    let gamma = s.nu_face.clone().unwrap_or_else(|| uniform_gamma(g, s.settings.nu));
    let scheme = s.settings.convection;
    let sf = mesh.face_area_vectors();
    let ub = s.u.boundary_values(g);
    let mut conv = vec![[0.0; 3]; nf];
    let mut visc = vec![[0.0; 3]; nf];
    let mut worst: f64 = 0.0;
    for k in 0..3 {
        let cells: Vec<f64> = s.u.values.iter().map(|v| v[k]).collect();
        let bc = s.u.boundary_coeffs(g, k);
        let bvals: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let grad =
            (scheme.needs_gradient() || s.settings.non_orth_correctors > 0).then(|| least_squares(g, &cells, &bvals));
        for f in 0..n_int {
            if g.empty[f] {
                continue;
            }
            let (o, n) = (own[f], nei[f]);
            let fl = phi[f];
            let forward = fl >= 0.0;
            let (up, dn) = if forward { (o, n) } else { (n, o) };
            let wt = g.weights[f];
            let val = match scheme {
                ConvScheme::Upwind => cells[up],
                ConvScheme::Tvd(lim) => {
                    let gr = grad.as_ref().unwrap()[up];
                    let dvec = if forward {
                        g.delta[f]
                    } else {
                        vec3::scale(g.delta[f], -1.0)
                    };
                    let gd = vec3::dot(gr, dvec);
                    let diff = cells[dn] - cells[up];
                    let r = if diff.abs() < 1e-300 {
                        0.0
                    } else {
                        2.0 * gd / diff - 1.0
                    };
                    let lambda = if forward { 1.0 - wt } else { wt };
                    cells[up] + lim.psi(r) * lambda * diff
                }
                other => panic!("budget: scheme {other:?} not rebuilt"),
            };
            conv[f][k] = fl * val;
            let gl = gamma[f] * g.lap_coeff[f];
            let mut fv = gl * (cells[o] - cells[n]);
            if let Some(gr) = &grad {
                let d = g.delta[f];
                let delta = vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d));
                let kv = vec3::sub(sf[f], delta);
                let gf = [0, 1, 2].map(|i| wt * gr[o][i] + (1.0 - wt) * gr[n][i]);
                fv -= gamma[f] * vec3::dot(kv, gf);
            }
            visc[f][k] = fv;
        }
        for f in n_int..nf {
            if g.empty[f] {
                continue;
            }
            let c = own[f];
            let co = &bc[f - n_int];
            let fl = phi[f];
            conv[f][k] = if g.coupled_face[f - n_int] && co.a > 0.0 && co.a < 1.0 {
                let q = co.b / (1.0 - co.a);
                let ud = if fl >= 0.0 { cells[c] } else { q };
                let kf = if matches!(scheme, ConvScheme::Upwind) { 0.0 } else { 1.0 };
                fl * ud + kf * fl * (co.a * cells[c] + co.b - ud)
            } else {
                fl * (co.a * cells[c] + co.b)
            };
            let a = vec3::norm(sf[f]);
            let mut fv = -gamma[f] * a * (co.c * cells[c] + co.d);
            if let (Some(gr), true) = (&grad, co.a == 0.0) {
                let d = g.delta[f];
                let kv = vec3::sub(sf[f], vec3::scale(d, a * a / vec3::dot(sf[f], d)));
                fv -= gamma[f] * vec3::dot(kv, gr[c]);
            }
            visc[f][k] = fv;
        }
        // reconstruction check against the kernel's operators
        let mc = convection(g, &phi, &cells, &bc, scheme, grad.as_deref());
        let ml = laplacian(g, &gamma, &bc, grad.as_deref());
        for (m, flux) in [(&mc, &conv), (&ml, &visc)] {
            let mut ax = vec![0.0; cells.len()];
            m.a.matvec(&cells, &mut ax);
            let mut sum = vec![0.0; cells.len()];
            let mut mag = vec![0.0f64; cells.len()];
            for f in 0..nf {
                if g.empty[f] {
                    continue;
                }
                sum[own[f]] += flux[f][k];
                mag[own[f]] = mag[own[f]].max(flux[f][k].abs());
                if f < n_int {
                    sum[nei[f]] -= flux[f][k];
                    mag[nei[f]] = mag[nei[f]].max(flux[f][k].abs());
                }
            }
            let scale = mag.iter().cloned().fold(0.0, f64::max).max(1e-300);
            for c in 0..cells.len() {
                worst = worst.max(((ax[c] - m.b[c]) - sum[c]).abs() / scale);
            }
        }
    }
    Rebuilt { conv, visc, worst }
}

/// Boundary pressures of the cell gradients (the kernel's trapezoid wall values; the other boundary values from the unmodified pressure
/// conditions, which is the kernel's value except at cells with two wall faces).
fn pressure_boundary(sim: &Simulation) -> Vec<f64> {
    let s = sim.simple_solver().unwrap();
    let g = &*s.geom;
    let mesh = &g.mesh;
    let pbc = pressure_bc(g, &s.u, &s.p);
    let p = &s.p.values;
    let mut out = boundary_values_pbc(g, &pbc, p);
    let (Some(bf), true) = (&s.body_force, s.wall_pressure_trapezoid) else {
        return out;
    };
    let (sf, own, nei, vol) = (
        mesh.face_area_vectors(),
        mesh.owner(),
        mesh.neighbour(),
        mesh.cell_volumes(),
    );
    let base = out.clone();
    for i in 0..out.len() {
        if !(pbc.fixed_u[i] && pbc.coupled[i].is_none()) {
            continue;
        }
        let f = g.n_internal + i;
        let c = own[f];
        let area = vec3::norm(sf[f]).max(1e-300);
        let nhat = vec3::scale(sf[f], 1.0 / area);
        let mut others = 0.0;
        for &fo in mesh.cell_faces(c) {
            if fo == f || g.empty[fo] {
                continue;
            }
            let (pf, sign) = if fo < g.n_internal {
                let w = g.weights[fo];
                (
                    w * p[own[fo]] + (1.0 - w) * p[nei[fo]],
                    if own[fo] == c { 1.0 } else { -1.0 },
                )
            } else {
                (base[fo - g.n_internal], 1.0)
            };
            others += pf * sign * vec3::dot(sf[fo], nhat);
        }
        out[i] = (vol[c] * vec3::dot(bf.cell[c], nhat) - others) / area;
    }
    out
}

/// Per-cell terms of the budget (full annulus, N m) and the global sums.
struct Budget {
    conv: Vec<f64>,
    press: Vec<f64>,
    visc: Vec<f64>,
    frame: Vec<f64>,
    frame_rc: Vec<f64>,
    res: Vec<f64>,
    /// the closed form `C_up` per cell (half of each face to each of its cells)
    c_up_cell: Vec<f64>,
    e_s: f64,
    s_seam: f64,
    c_up: f64,
    c_visc: f64,
    c_visc_wall: f64,
    worst_rebuild: f64,
}

fn budget(sim: &Simulation, om_frame: f64) -> Budget {
    let rb = rebuild_fluxes(sim);
    let s = sim.simple_solver().unwrap();
    let g = &*s.geom;
    let mesh = &g.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let (cc, cf, sf, vol) = (
        mesh.cell_centres(),
        mesh.face_centres(),
        mesh.face_area_vectors(),
        mesh.cell_volumes(),
    );
    let n_int = g.n_internal;
    let nf = mesh.n_faces();
    let nc = mesh.n_cells();
    let k = N_BLADES * RHO;
    let phi: Vec<f64> = match &s.conv_flux_offset {
        Some(o) => s.phi.iter().zip(o).map(|(p, o)| p - o).collect(),
        None => s.phi.clone(),
    };
    let p = &s.p.values;
    let pb = pressure_boundary(sim);
    let pf: Vec<f64> = (0..nf)
        .map(|f| {
            if f < n_int {
                let w = g.weights[f];
                w * p[own[f]] + (1.0 - w) * p[nei[f]]
            } else {
                pb[f - n_int]
            }
        })
        .collect();
    let u = &s.u.values;
    let wbar: Vec<f64> = {
        let ub = s.u.boundary_values(g);
        (0..nf)
            .map(|f| {
                if f < n_int {
                    let w = g.weights[f];
                    vec3::dot(
                        vec3::add(vec3::scale(u[own[f]], w), vec3::scale(u[nei[f]], 1.0 - w)),
                        sf[f],
                    )
                } else {
                    vec3::dot(ub[f - n_int], sf[f])
                }
            })
            .collect()
    };
    let acc: Vec<Vec3> = s.body_force.as_ref().map_or(vec![[0.0; 3]; nc], |b| b.cell.clone());
    let mut out = Budget {
        conv: vec![0.0; nc],
        press: vec![0.0; nc],
        visc: vec![0.0; nc],
        frame: vec![0.0; nc],
        frame_rc: vec![0.0; nc],
        res: vec![0.0; nc],
        c_up_cell: vec![0.0; nc],
        e_s: 0.0,
        s_seam: 0.0,
        c_up: 0.0,
        c_visc: 0.0,
        c_visc_wall: 0.0,
        worst_rebuild: rb.worst,
    };
    // momentum residual R_C = sum_f (conv + p S + visc) - V a_C; the pressure part of the face sums is the Green-Gauss V grad p of the kernel
    let mut resid = vec![[0.0; 3]; nc];
    for c in 0..nc {
        resid[c] = vec3::scale(acc[c], -vol[c]);
        out.frame[c] += k * zx(cc[c], vec3::scale(acc[c], vol[c]));
    }
    let gamma = s.nu_face.clone().unwrap_or_else(|| uniform_gamma(g, s.settings.nu));
    for f in 0..nf {
        if g.empty[f] {
            continue;
        }
        let fp = vec3::scale(sf[f], pf[f]);
        let total = vec3::add(vec3::add(rb.conv[f], fp), rb.visc[f]);
        let o = own[f];
        let mut sides = vec![(o, 1.0)];
        if f < n_int {
            sides.push((nei[f], -1.0));
        }
        for (c, sg) in sides {
            let lever = vec3::sub(cf[f], cc[c]);
            out.conv[c] += k * sg * zx(lever, rb.conv[f]);
            out.press[c] += k * sg * zx(lever, fp);
            out.visc[c] += k * sg * zx(lever, rb.visc[f]);
            out.frame[c] += k * sg * om_frame * phi[f] * rperp2(cf[f]);
            out.frame_rc[c] += k * sg * om_frame * (phi[f] - wbar[f]) * rperp2(cf[f]);
            resid[c] = vec3::add(resid[c], vec3::scale(total, sg));
        }
        if f >= n_int {
            let gen = k * (zx(cf[f], total) + om_frame * phi[f] * rperp2(cf[f]));
            if g.coupled_face[f - n_int] {
                out.s_seam += gen;
            } else {
                out.e_s += gen;
            }
        }
        // closed forms (H1, H2): rigid part, sign-definite
        if f < n_int {
            let d = vec3::sub(cc[nei[f]], cc[o]);
            let cu = k * 0.5 * om_frame * phi[f].abs() * rperp2(d);
            out.c_up += cu;
            out.c_up_cell[o] += 0.5 * cu;
            out.c_up_cell[nei[f]] += 0.5 * cu;
            out.c_visc += k * om_frame * gamma[f] * g.lap_coeff[f] * rperp2(d);
        } else if !g.coupled_face[f - n_int] {
            let d = vec3::sub(cf[f], cc[o]);
            out.c_visc_wall += k * om_frame * gamma[f] * vec3::norm(sf[f]) / g.dn[f - n_int] * rperp2(d);
        }
    }
    // periodic faces of the closed form: partner centre mapped into the owner's frame, half per face (each pair has two faces)
    for (i, d, _) in rcfd_fv::gradient::periodic_partner_points(g, &s.u, u) {
        let f = n_int + i;
        let cu = k * 0.25 * om_frame * phi[f].abs() * rperp2(d);
        out.c_up += cu;
        out.c_up_cell[s.geom.mesh.owner()[f]] += cu;
    }
    for c in 0..nc {
        out.res[c] = k * zx(cc[c], resid[c]);
    }
    out
}

/// Region masks of the location report.
fn regions(sim: &Simulation) -> Vec<(String, Vec<bool>)> {
    let g = &sim.geom;
    let mesh = &g.mesh;
    let cc = mesh.cell_centres();
    let nc = mesh.n_cells();
    let mut patch_of = vec![String::new(); mesh.n_faces()];
    for p in mesh.patches() {
        for f in p.faces() {
            patch_of[f] = p.name.clone();
        }
    }
    let touches =
        |c: usize, pred: &dyn Fn(usize) -> bool| mesh.cell_faces(c).iter().any(|&f| f >= g.n_internal && pred(f));
    let mut out: Vec<(String, Vec<bool>)> = vec![
        ("upstream".into(), (0..nc).map(|c| cc[c][2] < 0.1).collect()),
        (
            "chord band".into(),
            (0..nc)
                .map(|c| cc[c][2] >= 0.1 && cc[c][2] <= 0.1 + HUB_CHORD_Z)
                .collect(),
        ),
        (
            "downstream".into(),
            (0..nc).map(|c| cc[c][2] > 0.1 + HUB_CHORD_Z).collect(),
        ),
        (
            "wall-adjacent".into(),
            (0..nc)
                .map(|c| touches(c, &|f| patch_of[f] == "hub" || patch_of[f] == "shroud"))
                .collect(),
        ),
        (
            "seam-adjacent".into(),
            (0..nc)
                .map(|c| touches(c, &|f| g.coupled_face[f - g.n_internal]))
                .collect(),
        ),
        (
            "inlet/outlet-adjacent".into(),
            (0..nc)
                .map(|c| touches(c, &|f| patch_of[f] == "inlet" || patch_of[f] == "outlet"))
                .collect(),
        ),
    ];
    let dr = (R_DUCT - R_HUB) / 8.0;
    for b in 0..8 {
        let (lo, hi) = (R_HUB + b as f64 * dr, R_HUB + (b + 1) as f64 * dr);
        out.push((
            format!("r {:.4}-{:.4}", lo, hi),
            (0..nc)
                .map(|c| {
                    let r = cc[c][0].hypot(cc[c][1]);
                    r >= lo && (r < hi || b == 7)
                })
                .collect(),
        ));
    }
    out
}

fn report_budget(label: &str, sim: &Simulation, b: &Budget, excess_0078: f64) {
    let tot = |v: &[f64]| v.iter().sum::<f64>();
    let (gc, gp, gv, gf, grc, gr) = (
        tot(&b.conv),
        tot(&b.press),
        tot(&b.visc),
        tot(&b.frame),
        tot(&b.frame_rc),
        tot(&b.res),
    );
    println!(
        "W36C {label} BUDGET: E_s {:.5} N m (ADR 0078 report-based excess {excess_0078:.5}), S_seam {:.3e}; G_conv {gc:.5}, G_p {gp:.5}, G_visc {gv:.5}, G_frame {gf:.5} (Rhie-Chow part {grc:.5}), G_res {gr:.3e}; sum {:.5} = E_s + S_seam {:.5}; reconstruction worst {:.2e}",
        b.e_s,
        b.s_seam,
        gc + gp + gv + gf + gr,
        b.e_s + b.s_seam,
        b.worst_rebuild
    );
    println!(
        "W36C {label} CLOSED FORMS: C_up {:.5} N m (C_up / G_conv {:.3}), C_visc interior {:.5} N m, wall faces {:.5} N m; shares of E_s: conv {:.3}, p {:.3}, visc {:.3}, frame {:.3}, res {:.3}, seam {:.3}",
        b.c_up,
        b.c_up / gc,
        b.c_visc,
        b.c_visc_wall,
        gc / b.e_s,
        gp / b.e_s,
        gv / b.e_s,
        gf / b.e_s,
        gr / b.e_s,
        b.s_seam / b.e_s
    );
    let cc = sim.geom.mesh.cell_centres();
    let vol = sim.geom.mesh.cell_volumes();
    for (name, mask) in regions(sim) {
        let sel = |v: &[f64]| -> (f64, f64) {
            let s: f64 = v.iter().zip(&mask).filter(|(_, m)| **m).map(|(x, _)| *x).sum();
            let a: f64 = v.iter().zip(&mask).filter(|(_, m)| **m).map(|(x, _)| x.abs()).sum();
            (s, a)
        };
        let (zmin, zmax) = mask
            .iter()
            .enumerate()
            .filter(|(_, m)| **m)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (c, _)| {
                (lo.min(cc[c][2]), hi.max(cc[c][2]))
            });
        let v: f64 = vol.iter().zip(&mask).filter(|(_, m)| **m).map(|(x, _)| *x).sum::<f64>() * N_BLADES;
        let (c, ca) = sel(&b.conv);
        let (p, pa) = sel(&b.press);
        let (vi, va) = sel(&b.visc);
        let (fr, fa) = sel(&b.frame);
        let (r, ra) = sel(&b.res);
        let (cu, _) = sel(&b.c_up_cell);
        println!(
            "W36C {label} REGION {name}: cells {}, volume {v:.5} m^3, z {zmin:.3}..{zmax:.3}; conv {c:.5} (|.| {ca:.5}), p {p:.5} ({pa:.5}), visc {vi:.5} ({va:.5}), frame {fr:.5} ({fa:.5}), res {r:.3e} ({ra:.3e}); closed form C_up {cu:.5}; conv per metre {:.4} N m/m",
            mask.iter().filter(|m| **m).count(),
            c / (zmax - zmin).max(1e-9)
        );
    }
}

fn diagnosis(label: &str, convection: &'static str) {
    let a = arm(label, m1(), false, Frame::Relative, convection);
    let (o, sim) = run(&a, 8000);
    let b = budget(&sim, omega());
    report_budget(label, &sim, &b, o.l_excess);
}

#[test]
#[ignore = "ADR 0082 X2a-36: reproduction of ADR 0078 X2a and its budget (about 1 min)"]
fn w36c_x2a_budget() {
    diagnosis("X2a-36", "upwind");
}

#[test]
#[ignore = "ADR 0082 X2d-36: ADR 0078 X2d and its budget (about 16 min)"]
fn w36c_x2d_budget() {
    diagnosis("X2d-36", "tvd_minmod");
}

/// The budget's bookkeeping on a short (unconverged) X2a run: the rebuilt fluxes equal the kernel's operators, and the per-cell terms add up
/// to the boundary sums exactly (an identity of the definitions, whatever the convergence state).
#[test]
fn budget_identity_and_reconstruction_on_a_short_run() {
    let a = arm("X2a-short", m1(), false, Frame::Relative, "upwind");
    let (o, sim) = run(&a, 5);
    let b = budget(&sim, omega());
    let tot = |v: &[f64]| v.iter().sum::<f64>();
    let sum = tot(&b.conv) + tot(&b.press) + tot(&b.visc) + tot(&b.frame) + tot(&b.res);
    println!(
        "W36C short: reconstruction worst {:.2e}; sum {sum:.6e} vs E_s + S_seam {:.6e}; outlet L {:.4}",
        b.worst_rebuild,
        b.e_s + b.s_seam,
        o.l_out
    );
    assert!(b.worst_rebuild <= 1e-10, "reconstruction {:.3e}", b.worst_rebuild);
    let scale = tot(&b.conv.iter().map(|x| x.abs()).collect::<Vec<_>>()).max(1e-30);
    assert!(
        (sum - (b.e_s + b.s_seam)).abs() <= 1e-9 * scale,
        "identity {sum} vs {}",
        b.e_s + b.s_seam
    );
}

// ---------------------------------------------------------------- amendment 1: construction measurements (geometry only)

/// Per-cell frame-flux closure `|sum_f (Omega x r_f).S_f| / (|Omega| sum_f r_perp,f |S_f|)` over all faces of every cell (Omega cancels).
fn frame_closure(mesh: &rcfd_mesh::Mesh) -> Vec<f64> {
    let (cf, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    (0..mesh.n_cells())
        .map(|c| {
            let (mut num, mut den) = (0.0, 0.0);
            for &f in mesh.cell_faces(c) {
                let sign = if mesh.owner()[f] == c { 1.0 } else { -1.0 };
                // (z x r) . S = x S_y - y S_x
                num += sign * (cf[f][0] * sf[f][1] - cf[f][1] * sf[f][0]);
                den += rperp2(cf[f]).sqrt() * vec3::norm(sf[f]);
            }
            num.abs() / den.max(1e-300)
        })
        .collect()
}

/// Frame flux `(z x r).dA` integrated exactly over the triangle fan of `polygon_geometry` (the mesh's own face decomposition), per face.
fn frame_flux_tri(mesh: &rcfd_mesh::Mesh, f: usize) -> f64 {
    let pts = mesh.points();
    let ids = mesh.face_points(f);
    let n = ids.len();
    let p = |i: usize| pts[ids[i % n]];
    let tri = |a: Vec3, b: Vec3, e: Vec3| {
        let c = vec3::scale(vec3::add(vec3::add(a, b), e), 1.0 / 3.0);
        let s = vec3::scale(vec3::cross(vec3::sub(b, a), vec3::sub(e, a)), 0.5);
        c[0] * s[1] - c[1] * s[0]
    };
    if n == 3 {
        return tri(p(0), p(1), p(2));
    }
    let est = vec3::scale((0..n).fold(vec3::ZERO, |acc, i| vec3::add(acc, p(i))), 1.0 / n as f64);
    (0..n).map(|i| tri(p(i), p(i + 1), est)).sum()
}

/// The closure of `frame_flux_tri` per cell, normalised as `frame_closure`.
fn frame_closure_tri(mesh: &rcfd_mesh::Mesh) -> Vec<f64> {
    let (cf, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let ft: Vec<f64> = (0..mesh.n_faces()).map(|f| frame_flux_tri(mesh, f)).collect();
    (0..mesh.n_cells())
        .map(|c| {
            let (mut num, mut den) = (0.0, 0.0);
            for &f in mesh.cell_faces(c) {
                num += if mesh.owner()[f] == c { ft[f] } else { -ft[f] };
                den += rperp2(cf[f]).sqrt() * vec3::norm(sf[f]);
            }
            num.abs() / den.max(1e-300)
        })
        .collect()
}

/// Largest and median cell extents (axial, radial, pitchwise r dtheta) from the face centres of each cell.
fn extents(mesh: &rcfd_mesh::Mesh) -> [(f64, f64); 3] {
    let cf = mesh.face_centres();
    let mut e: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for c in 0..mesh.n_cells() {
        let pts: Vec<Vec3> = mesh.cell_faces(c).iter().map(|&f| cf[f]).collect();
        let r: Vec<f64> = pts.iter().map(|p| p[0].hypot(p[1])).collect();
        let rm = r.iter().sum::<f64>() / r.len() as f64;
        let th: Vec<f64> = pts.iter().map(|p| p[1].atan2(p[0])).collect();
        let span = |v: &[f64]| {
            v.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - v.iter().cloned().fold(f64::INFINITY, f64::min)
        };
        let z: Vec<f64> = pts.iter().map(|p| p[2]).collect();
        e[0].push(span(&z));
        e[1].push(span(&r));
        e[2].push(rm * span(&th));
    }
    e.map(|mut v| {
        v.sort_by(|a, b| a.total_cmp(b));
        (v[v.len() - 1], v[v.len() / 2])
    })
}

#[test]
#[ignore = "ADR 0082 amendment 1: frame-flux closure and cell extents of M1-M3 and the E0 mesh (construction only, about 1 min)"]
fn w36c_construction_closure_and_extents() {
    let meshes = [
        ("M0", m0(), false),
        ("M1", m1(), false),
        ("M2", m2(), false),
        ("M3", m3(), false),
        ("E0", m1(), true),
    ];
    for (name, counts, blade) in meshes {
        let a = arm(name, counts, blade, Frame::Inertial, "upwind");
        let case = Case::parse_toml(&case_toml(&a)).unwrap();
        let mesh = rcfd_api::sim::build_mesh(&case).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let cl = frame_closure(&mesh);
        let (worst_c, worst) = cl
            .iter()
            .enumerate()
            .fold((0, 0.0f64), |b, (c, &v)| if v > b.1 { (c, v) } else { b });
        let mut s = cl.clone();
        s.sort_by(|a, b| a.total_cmp(b));
        let q = |p: f64| s[((s.len() - 1) as f64 * p) as usize];
        let above = cl.iter().filter(|&&v| v > 1e-10).count();
        let cc = mesh.cell_centres()[worst_c];
        let ex = extents(&mesh);
        let tri_worst = frame_closure_tri(&mesh).into_iter().fold(0.0f64, f64::max);
        let warp_worst = (0..mesh.n_faces())
            .map(|f| {
                let (cf, sf) = (mesh.face_centres()[f], mesh.face_area_vectors()[f]);
                (frame_flux_tri(&mesh, f) - (cf[0] * sf[1] - cf[1] * sf[0])).abs()
                    / (rperp2(cf).sqrt() * vec3::norm(sf)).max(1e-300)
            })
            .fold(0.0f64, f64::max);
        println!("W36C CONSTRUCTION {name}: triangle-exact frame-flux closure worst {tri_worst:.3e}; largest face warp term |F_tri - (z x r_f).S_f| / (r |S|) {warp_worst:.3e}");
        println!(
            "W36C CONSTRUCTION {name}: {} cells; frame-flux closure worst {worst:.3e} at cell {worst_c} (r {:.4}, z {:.4}), median {:.3e}, 99th percentile {:.3e}, cells above 1e-10: {above}; extents largest/median axial {:.4e}/{:.4e}, radial {:.4e}/{:.4e}, pitchwise {:.4e}/{:.4e} m",
            mesh.n_cells(),
            cc[0].hypot(cc[1]),
            cc[2],
            q(0.5),
            q(0.99),
            ex[0].0,
            ex[0].1,
            ex[1].0,
            ex[1].1,
            ex[2].0,
            ex[2].1
        );
    }
}

// ---------------------------------------------------------------- the inertial control B and the relative family R (ADR 0082 amendment 3)
// The F-RP arms (Y, F1) and their checks were removed with F-RP (ADR 0082 addendum: A2, A3 missed on M0); their measured values are in the ADR.

fn b_arm(name: &str, counts: Counts, tol: f64) -> Arm {
    Arm {
        tolerance: tol,
        ..arm(name, counts, false, Frame::Inertial, "upwind")
    }
}

fn print_outcome(label: &str, o: &Outcome) {
    println!(
        "W36C {label} OUTCOME: converged {}, {} iterations, outlet L {:.6e} N m, L excess {:.6e}, outlet rho<cth2/2> {:.4} Pa, Delta p_tt {:.3} Pa, cut U L {:.5}",
        o.converged, o.iterations, o.l_out, o.l_excess, o.kth_out, o.dp_tt, o.l_cut_u
    );
}

/// Iteration-error check on M1 (B at 1e-5 and 1e-7; the Y arms were removed with F-RP).
#[test]
#[ignore = "ADR 0082 iteration check on M1 (a few minutes)"]
fn w36c_iteration_check_m1() {
    for a in [b_arm("B(M1) 1e-5", m1(), 1e-5), b_arm("B(M1) 1e-7", m1(), 1e-7)] {
        let (o, _) = run(&a, 8000);
        print_outcome(&a.label, &o);
    }
}

/// The 3-mesh study (amendment 3: M0, M1, M2): the inertial control B on M0 and M2 at the tolerance chosen by the iteration check
/// (environment `W36C_TOL`, default 1e-5; the Y arms were removed with F-RP).
#[test]
#[ignore = "ADR 0082 B on M0, M2 (up to an hour)"]
fn w36c_mesh_study_b() {
    let tol: f64 = std::env::var("W36C_TOL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1e-5);
    for a in [b_arm("B(M0)", m0(), tol), b_arm("B(M2)", m2(), tol)] {
        let (o, _) = run(&a, 8000);
        print_outcome(&a.label, &o);
    }
}

/// The relative-frame family R on M0, M2 (amendment 3; diagnosis of the H1 order; ADR 0078 X2c did not converge).
#[test]
#[ignore = "ADR 0082 R(M0), R(M2) (up to an hour)"]
fn w36c_mesh_study_r() {
    for a in [
        arm("R(M0)", m0(), false, Frame::Relative, "upwind"),
        arm("R(M2)", m2(), false, Frame::Relative, "upwind"),
    ] {
        let (o, sim) = run(&a, 8000);
        print_outcome(&a.label, &o);
        if o.converged {
            let b = budget(&sim, omega());
            report_budget(&a.label, &sim, &b, o.l_excess);
        }
    }
}
