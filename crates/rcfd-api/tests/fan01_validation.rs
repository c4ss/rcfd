//! Wave 34D (ADR 0075, part B): the F-126 acceptance attempt on a published low-speed axial fan, FAN-01 (FAU / EAA benchmark,
//! Zenger et al. SAE 2016-01-1805; data and CAD: Zenodo 10.5281/zenodo.20037409, CC BY 4.0), and the checks H1-H3 of the optional
//! multi-station blade input of `mesh.fan`. Everything here (geometry, mesh rule, physics, operating points, limits) was registered in
//! ADR 0075 amendment 1 before the mesh was built and before any solver run. A rotor-only sector of a published fan compared with its
//! measured installation characteristic: whatever the outcome, no fan performance claim is made beyond this one comparison.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the limits

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use std::f64::consts::PI;

/// Cylindrical sections of the FAN-01 blade, extracted from the CC BY 4.0 CAD (`cad_model.igs` of the Zenodo record) for ADR 0075:
/// r [mm], chord [mm], stagger from the axis [deg], max camber / c, its position x/c, max thickness / c, leading edge x [mm] and theta [deg]
/// (CAD frame: axis +x, flow +x, theta = atan2(z, y), rotation +theta). r = 124 and 247.5 are linear extrapolations of the two
/// neighbouring sections (the CAD has a root fillet below r = 127 and a tip cap at 247.4-247.5).
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
const RHO: f64 = 1.2; // ASSUMED (the data set gives no density), UNVERIFIED
const NU: f64 = 1.5e-5;
const RPM: f64 = 1486.0;

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// Measured operating points read from `characteristic_n1ug.mat` (Zenodo record): Q [m^3/s], Delta p_ts [Pa], shaft power [W]
/// (the file labels it `P_shaft_in_Nm`; the values are watts: Q Delta p / P reproduces the stated 53 % efficiency).
const POINTS: [(f64, f64, f64); 2] = [(1.4032, 126.46, 333.30), (1.8039, 63.9, 292.9)];

fn list(f: impl Fn(&[f64; 8]) -> f64) -> String {
    let v: Vec<String> = SECTIONS.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// The `mesh.fan` section of FAN-01 with the registered counts of the chosen topology.
/// `first_cell`: None = the amendment 1 y+ = 30 rule, Some(h) = a fixed first-cell height (attempt 2, amendment 2).
fn fan_section(topology: &str, first_cell: Option<f64>) -> String {
    let counts = if topology == "c_grid" {
        "topology = \"c_grid\"\nn_up = 8\nn_chord = 16\nn_down = 8\nn_side = 10\nn_nose = 4\nn_core = 1\nfront_gap = 0.3\nn_span = 14\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\n"
    } else {
        "n_up = 8\nn_chord = 12\nn_down = 8\nn_side = 10\nn_mid = 1\nn_span = 16\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\n"
    };
    let le_x0 = SECTIONS[0][6];
    let le_t0 = SECTIONS[0][7];
    let ref_speed = (POINTS[0].0 / annulus_area()).hypot(omega() * 0.186);
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = {R_HUB}\nr_tip = {R_TIP}\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
{counts}first_cell_height = {:?}\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = {NU:?}\n",
        list(|s| s[0] * 1e-3),
        list(|s| s[1] * 1e-3),
        list(|s| s[2]),
        list(|s| s[3]),
        list(|s| s[4]),
        list(|s| s[5]),
        list(|s| (s[6] - le_x0) * 1e-3),
        list(|s| s[0] * 1e-3 * (s[7] - le_t0).to_radians()),
        first_cell.unwrap_or(0.0),
    )
}

/// The registered FAN-01 case at volume flow `q`: attempt 2 (H-slit, first cell 2e-4 m, `linear_upwind`, diverged) with the attempt 3 change
/// of ADR 0075 amendment 3 (`upwind` momentum convection, the first configuration of the diagnostic round that stayed stable).
fn fan01_case(q: f64) -> String {
    let w = omega();
    let u_in = q / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    format!(
        r#"{}
[physics]
nu = {NU:?}
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[initial]
velocity = [0.0, 0.0, {u_in:?}]
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
turbulence = "inlet"
turbulence_spec = "values"
k_value = {k:?}
omega_value = {om:?}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.blade]
velocity = "fixed"
[boundaries.shroud]
velocity = "fixed"
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
turbulence = "wall"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 8000
tolerance_u = 1e-5
tolerance_p = 1e-5
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
[reports.m_blade]
type = "moment"
patch = "blade"
direction = [0.0, 0.0, 1.0]
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
"#,
        fan_section("h_slit", Some(ATTEMPT2_FIRST_CELL))
    )
}

// ---------------------------------------------------------------- H1-H3: the optional station lists

const EXAMPLE_HUB_TIP: &str = "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 5\nr_hub = 0.10\nr_tip = 0.30\ntip_clearance = 0.01\naxial_extent = 0.7\nz_le_root = 0.3\nfamily = \"naca4\"\nhub_chord = 0.10\ntip_chord = 0.07\nhub_stagger = 35.0\ntip_stagger = 20.0\nhub_camber = 0.04\ntip_camber = 0.04\ncamber_pos = 0.4\nhub_thickness = 0.10\ntip_thickness = 0.06\nhub_sweep = 0.0\ntip_sweep = 0.01\nhub_lean = 0.0\ntip_lean = 0.005\nn_up = 8\nn_chord = 12\nn_down = 8\nn_side = 10\nn_mid = 1\nn_span = 16\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\nfirst_cell_height = 1e-3\n";

const EXAMPLE_STATIONS: &str = "station_radius = [0.10, 0.30]\nstation_chord = [0.10, 0.07]\nstation_stagger = [35.0, 20.0]\nstation_camber = [0.04, 0.04]\nstation_camber_pos = [0.4, 0.4]\nstation_thickness = [0.10, 0.06]\nstation_sweep = [0.0, 0.01]\nstation_lean = [0.0, 0.005]\n";

fn mesh_of(text: &str) -> Result<rcfd_mesh::Mesh, Vec<rcfd_api::case::Diagnostic>> {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    build_mesh(&case)
}

#[test]
fn h1_two_station_lists_reproduce_the_hub_tip_mesh_exactly() {
    let a = mesh_of(EXAMPLE_HUB_TIP).unwrap();
    // the station lists replace the hub/tip values; give the hub/tip keys deliberately different values to prove they are unused
    let text = format!(
        "{}{EXAMPLE_STATIONS}",
        EXAMPLE_HUB_TIP
            .replace("hub_chord = 0.10", "hub_chord = 0.2")
            .replace("tip_stagger = 20.0", "tip_stagger = 10.0")
    );
    let b = mesh_of(&text).unwrap();
    assert_eq!(a.n_cells(), b.n_cells());
    assert_eq!(a.n_faces(), b.n_faces());
    assert_eq!(a.points().len(), b.points().len());
    let worst = a
        .points()
        .iter()
        .zip(b.points())
        .map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f64::max))
        .fold(0.0, f64::max);
    println!("H1: {} cells, largest node difference {worst:e} m", a.n_cells());
    assert_eq!(worst, 0.0, "node coordinates differ by {worst:e}");
}

#[test]
fn h2_inconsistent_station_lists_are_located_errors() {
    let short = format!(
        "{EXAMPLE_HUB_TIP}{}",
        EXAMPLE_STATIONS.replace("station_chord = [0.10, 0.07]", "station_chord = [0.10]")
    );
    let e = mesh_of(&short).unwrap_err();
    println!("H2 short list: {e:?}");
    assert!(e.iter().any(|d| d.path == "mesh.fan.station_chord"), "{e:?}");
    let off = format!(
        "{EXAMPLE_HUB_TIP}{}",
        EXAMPLE_STATIONS.replace("station_radius = [0.10, 0.30]", "station_radius = [0.12, 0.30]")
    );
    let e = mesh_of(&off).unwrap_err();
    println!("H2 first radius off the hub: {e:?}");
    assert!(e.iter().any(|d| d.path == "mesh.fan.station_radius[0]"), "{e:?}");
    let orphan = format!("{EXAMPLE_HUB_TIP}station_chord = [0.1, 0.07]\n");
    let e = mesh_of(&orphan).unwrap_err();
    assert!(e.iter().any(|d| d.path == "mesh.fan.station_chord"), "{e:?}");
}

#[test]
fn h3_the_fan_examples_still_build() {
    for name in [
        "axial_fan_sector/axial_fan_sector.toml",
        "axial_fan_cgrid/axial_fan_cgrid.toml",
    ] {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../cases")
            .join(name);
        let text = std::fs::read_to_string(&p).unwrap();
        let m = mesh_of(&text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        println!("H3 {name}: {} cells", m.n_cells());
    }
}

// ---------------------------------------------------------------- F-126 attempt

/// Attempt 2 (ADR 0075 amendment 2): first-cell height of the H-slit mesh.
const ATTEMPT2_FIRST_CELL: f64 = 2.0e-4;

/// Amendment 1 rule: c_grid unless its generator returns an error, then h_slit, first cell from y+ = 30. Measured outcome: BOTH refuse
/// (NOT RUNNABLE): the c_grid wake line leaves the passage at the hub, the h_slit tip gap cannot hold the boundary layers.
#[test]
fn fan01_attempt1_registered_mesh_rule_is_not_runnable() {
    let c = mesh_of(&fan_section("c_grid", None));
    let h = mesh_of(&fan_section("h_slit", None));
    println!("FAN-01 attempt 1: c_grid {:?}", c.as_ref().map(|m| m.n_cells()));
    println!("FAN-01 attempt 1: h_slit {:?}", h.as_ref().map(|m| m.n_cells()));
    // the c_grid half is informational only: the C-grid construction is being changed by wave 34A (ADR 0072); the attempt 1 outcome
    // (refused, the wake line leaves the passage at the hub) is recorded in ADR 0075
    let _ = c;
    let he = h.expect_err("the h_slit now builds with the y+ rule: the recorded outcome of attempt 1 changed");
    assert!(he.iter().any(|d| d.path == "mesh.fan.n_gap"), "{he:?}");
}

#[test]
fn fan01_attempt2_mesh_builds() {
    let m =
        mesh_of(&fan_section("h_slit", Some(ATTEMPT2_FIRST_CELL))).unwrap_or_else(|e| panic!("NOT RUNNABLE: {e:?}"));
    let q = rcfd_mesh::quality::check_mesh(&m);
    println!(
        "FAN-01 attempt 2 mesh: h_slit, {} cells, max non-orthogonality {:.1} deg (mean {:.1}), max skewness {:.2}, max aspect {:.0}, min volume {:.3e}, errors {:?}",
        m.n_cells(),
        q.max_non_orthogonality,
        q.mean_non_orthogonality,
        q.max_skewness,
        q.max_aspect_ratio,
        q.min_volume,
        q.errors
    );
    assert!(q.is_ok(), "{:?}", q.errors);
}

/// S1b (ADR 0075 amendment 2): the stationary casing (a moving wall in the rotating frame) is accepted with k-omega SST and still refused
/// with k-epsilon.
#[test]
fn s1b_moving_wall_is_accepted_for_sst_only() {
    let text = fan01_case(POINTS[0].0);
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("SST case refused: {e:?}"));
    Simulation::new(&case).unwrap_or_else(|e| panic!("SST case refused: {e:?}"));
    let ke = text.replace("model = \"k_omega_sst\"", "model = \"k_epsilon\"");
    let e = Case::parse_toml(&ke).map(|_| ()).unwrap_err();
    println!("S1b k-epsilon: {e:?}");
    assert!(e.iter().any(|d| d.path == "boundaries.shroud.turbulence"), "{e:?}");
}

struct PointResult {
    q: f64,
    dp_ts: f64,
    torque: f64,
    torque_pressure_part: f64,
    eta: f64,
    dp_tt: f64,
    converged: bool,
    how: String,
}

fn rel_change(s: &rcfd_post::monitors::Series, window: usize) -> f64 {
    let n = s.y.len();
    if n <= window {
        return f64::INFINITY;
    }
    let (a, b) = (s.y[n - 1 - window], s.y[n - 1]);
    (b - a).abs() / b.abs().max(1e-300)
}

fn run_point(q: f64) -> PointResult {
    let text = fan01_case(q);
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    // harness stop of amendment 3 (no physics change): a non-finite or exploding continuity ends the run at once
    let mut n = 0;
    let mut exploded = None;
    while n < 8000 && !sim.is_done() {
        let st = sim.step();
        n += 1;
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) || st.continuity > 1e6 {
            exploded = Some((n, st.continuity));
            break;
        }
    }
    let failure = sim.failure();
    let w = omega();
    let u_in = q / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    // moments of the fluid on the rotor about +z (N m per passage); the shaft torque is the opposite
    let torque = -N_BLADES * (mon("m_blade") + mon("m_hub"));
    let dp_ts = RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in));
    let eta = q * dp_ts / (torque * w);
    // pressure part of the blade + hub moment from the owner-cell pressure (information)
    let g = &sim.geom;
    let (cf, sf, own) = (g.mesh.face_centres(), g.mesh.face_area_vectors(), g.mesh.owner());
    let p = sim.field("p").unwrap();
    let mut mp = 0.0;
    for name in ["blade", "hub"] {
        for f in g.mesh.patch(name).unwrap().faces() {
            let fx = RHO * p[own[f]] * sf[f][0];
            let fy = RHO * p[own[f]] * sf[f][1];
            mp += cf[f][0] * fy - cf[f][1] * fx;
        }
    }
    let torque_pressure_part = -N_BLADES * mp;
    // mass-averaged absolute total pressure at the outlet (owner-cell values, information)
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let cc = g.mesh.cell_centres();
    let phi = sim.flux();
    let (mut num, mut den) = (0.0, 0.0);
    for f in g.mesh.patch("outlet").unwrap().faces() {
        let c = own[f];
        let ua = [ux[c] - w * cc[c][1], uy[c] + w * cc[c][0], uz[c]];
        let pt = p[c] + 0.5 * (ua[0] * ua[0] + ua[1] * ua[1] + ua[2] * ua[2]);
        num += pt * phi[f];
        den += phi[f];
    }
    let dp_tt = RHO * (num / den - (mon("p_in") + 0.5 * u_in * u_in));
    let (m_in, m_out) = (mon("m_in"), mon("m_out"));
    let series = |k: &str| sim.monitors.get(k).cloned();
    let how;
    let mut converged = false;
    if failure.is_none() && sim.is_converged() {
        converged = true;
        how = format!("residual tolerance reached in {n} iterations");
    } else if let Some((it, c)) = exploded {
        how = format!("diverged: continuity {c:.3e} at iteration {it}");
    } else if failure.is_none() {
        // registered steady-monitor criterion at the cap: torque and Delta p_ts change < 0.5 % over the last 1000 iterations, mass balance 1e-3
        let ct = match (series("m_blade"), series("m_hub")) {
            (Some(b), Some(h)) => {
                let t: Vec<f64> = b.y.iter().zip(&h.y).map(|(x, y)| x + y).collect();
                let s = rcfd_post::monitors::Series {
                    unit: String::new(),
                    x: b.x.clone(),
                    y: t,
                };
                rel_change(&s, 1000)
            }
            _ => f64::INFINITY,
        };
        let cp = match (series("p_in"), series("p_out")) {
            (Some(a), Some(b)) => {
                let d: Vec<f64> = a.y.iter().zip(&b.y).map(|(i, o)| o - (i + 0.5 * u_in * u_in)).collect();
                let s = rcfd_post::monitors::Series {
                    unit: String::new(),
                    x: a.x.clone(),
                    y: d,
                };
                rel_change(&s, 1000)
            }
            _ => f64::INFINITY,
        };
        let mb = (m_in + m_out).abs() / m_in.abs();
        converged = ct < 5e-3 && cp < 5e-3 && mb <= 1e-3;
        how = format!(
            "at the cap ({n} iterations): torque change {ct:.3e}, Delta p_ts change {cp:.3e} over 1000 iterations, mass balance {mb:.3e}; last {:?}",
            sim.last()
        );
    } else {
        how = format!("failure after {n} iterations: {failure:?}");
    }
    let r = PointResult {
        q,
        dp_ts,
        torque,
        torque_pressure_part,
        eta,
        dp_tt,
        converged,
        how,
    };
    println!(
        "FAN-01 h_slit Q {q} m^3/s: converged {} ({}); Delta p_ts {:.2} Pa, Delta p_tt {:.2} Pa, torque {:.4} N m (pressure part {:.4}), eta_ts {:.4}; m_in {m_in:.6e} m_out {m_out:.6e}",
        r.converged, r.how, r.dp_ts, r.dp_tt, r.torque, r.torque_pressure_part, r.eta
    );
    r
}

/// The registered F-126 comparison: (failed limits, results).
fn f126_outcome() -> (Vec<String>, Vec<PointResult>) {
    let mut bad = Vec::new();
    let mut out = Vec::new();
    for (q, dp_m, power) in POINTS {
        let r = run_point(q);
        let m_meas = power / omega();
        let eta_meas = q * dp_m / power;
        println!(
            "FAN-01 Q {q}: Delta p_ts {:.2} vs {dp_m} Pa (limit {:.2}), torque {:.4} vs {m_meas:.4} N m ({:+.1} %, limit 5 %), eta {:.4} vs {eta_meas:.4} (limit 0.03)",
            r.dp_ts,
            3.0 + 0.03 * dp_m,
            r.torque,
            100.0 * (r.torque / m_meas - 1.0),
            r.eta
        );
        if !r.converged {
            bad.push(format!("NOT CONVERGED at Q {q}: {}", r.how));
        }
        if !((r.dp_ts - dp_m).abs() <= 3.0 + 0.03 * dp_m) {
            bad.push(format!("Q {q}: Delta p_ts {:.2} vs {dp_m}", r.dp_ts));
        }
        if !((r.torque / m_meas - 1.0).abs() <= 0.05) {
            bad.push(format!("Q {q}: torque {:.4} vs {m_meas:.4}", r.torque));
        }
        if !((r.eta - eta_meas).abs() <= 0.03) {
            bad.push(format!("Q {q}: eta {:.4} vs {eta_meas:.4}", r.eta));
        }
        out.push(r);
    }
    (bad, out)
}

/// F-126 attempt 3 (ADR 0075 amendment 3), measured once per operating point (about 16 min each): both runs converge by the steady-monitor
/// criterion; design point Delta p_ts -3.8 Pa (measured 126.5), torque 1.320 N m (2.142, -38 %), eta_ts -0.03 (0.532); off-design
/// -84.4 Pa (63.9), 1.010 N m (1.882, -46 %), eta_ts -0.97 (0.394). MISSED; the limits are unchanged.
#[test]
#[ignore = "known miss: F-126 on FAN-01 (attempt 3) misses every limit: torque -38 % / -46 %, Delta p_ts -3.8 / -84.4 Pa against 126.5 / 63.9 Pa; two 16-minute SST sector runs"]
fn f126_fan01_rotor_against_the_measured_characteristic_known_miss() {
    let (bad, res) = f126_outcome();
    for r in &res {
        println!(
            "FAN-01 summary Q {}: dp_ts {:.2}, torque {:.4}, eta {:.4}, converged {}",
            r.q, r.dp_ts, r.torque, r.eta, r.converged
        );
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
