//! ADR 0762 (F-870): FAN-01 at its design point, pressure rise and shaft power against the measured characteristic. The state of the
//! second-order set-up of ADR 0086 (C-grid `camber_seam_h`, k-omega SST, Barth-Jespersen-limited `linear_upwind`, relative-velocity rotating
//! frame) held as numbers: a heavy record on the finest mesh B1 and fast tests on the same path. The band, the mesh quality gates, the
//! convergence rule and the tolerance of the state were registered in ADR 0762 before any run. The result MISSES the acceptance of F-870;
//! it is recorded as a miss and no agreement with the measurement is claimed.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_mesh::quality::check_mesh;
use std::f64::consts::PI;
use std::sync::OnceLock;
use std::time::Instant;

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RPM: f64 = 1486.0;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075): no source that was read gives the density of the measurement (ADR 0762, open question 2)
const Q_DESIGN: f64 = 1.4032;
/// Measured design point (`docs/validation-data.md`, 1. FAN-01; row 13 of `w34d_fan01_characteristic_n1ug.csv`).
const DP_TS_MEAS: f64 = 126.46;
const POWER_MEAS: f64 = 333.30;
/// Iteration cap of the registered run (ADR 0086, ADR 0762).
const CAP: usize = 8000;
/// Iterations of the fast test on the same path.
const SHORT: usize = 300;

/// The mesh-convergence estimate of ADR 0762, fixed before the run from the recorded triplets of ADR 0170 and ADR 0086:
/// half-widths for the shaft torque [N m] and the pressure rise [Pa].
const U_NUM_TORQUE: f64 = 0.21190;
const U_NUM_DP: f64 = 29.75;
/// The stated uncertainty of the measurement is NOT registered: it is in no source that could be read (ADR 0762). The checks are
/// evaluated with zero, the narrowest band of the registered form.
const U_MEAS_POWER: f64 = 0.0;
const U_MEAS_DP: f64 = 0.0;
/// Registered mesh quality gates of the fan passage mesh (`docs/meshing.md`, set-up A): non-orthogonality [deg], skewness, aspect ratio.
const GATE_NON_ORTHOGONALITY: f64 = 70.0;
const GATE_SKEWNESS: f64 = 2.0;
const GATE_ASPECT: f64 = 300.0;
/// Factor of safety of the mesh-convergence estimate (ADR 0086).
const FS: f64 = 1.25;
/// Tolerance of the state, relative: twice the 0.5 % to which the cap rule of ADR 0075 / ADR 0086 defines a functional (ADR 0762).
const STATE_TOL: f64 = 0.01;

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) of the ADR 0086 family.
type Counts = (usize, usize, usize, usize, usize, usize, usize);
const B1: Counts = (12, 24, 12, 15, 6, 21, 10);
const B2: Counts = (8, 17, 8, 11, 4, 15, 7);
const B3: Counts = (6, 12, 6, 8, 3, 10, 5);
const CELLS: [f64; 3] = [38184.0, 13627.0, 4920.0];

/// Recorded triplets (B1, B2, B3) the band was registered from: effective torque [N m] (ADR 0170) and pressure rise [Pa] (ADR 0086).
const TORQUE_0170: [f64; 3] = [2.098164, 2.028692, 1.918189];
const DP_0086: [f64; 3] = [73.988, 64.233, 46.778];

/// Recorded mesh quality of B1 (ADR 0086, lines 51 to 57): non-orthogonality [deg], skewness, aspect ratio.
const B1_QUALITY_RECORD: [f64; 3] = [83.37, 3.765, 252.4];

/// The recorded state: (torque [N m], pressure rise [Pa]) of the first run on main 13b0937d (ADR 0762, addendum 1).
const B1_RECORD: (f64, f64) = (f64::NAN, f64::NAN);
const B3_SHORT_RECORD: (f64, f64) = (1.918174, 46.5819);
/// The recorded verdicts of the acceptance clauses on B1: A1 pressure rise in the band, A2 shaft power in the band, A3a non-orthogonality,
/// A3b skewness, A3c aspect ratio, A4 converged. `true` = met.
const B1_VERDICTS_RECORD: [bool; 6] = [false, true, false, false, true, false];

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
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

/// The mesh section of `fan01_gci_w37c.rs` (ADR 0086), bladed passage, construction `camber_seam_h`; no value changed.
fn mesh_toml(c: Counts) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    format!(
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
    )
}

/// The bladed case of `fan01_gci_w37c.rs` (ADR 0086): design point, `rotating_frame`, k-omega SST, SIMPLEC, `linear_upwind` with the
/// Barth-Jespersen limiter, tolerances 1e-5. `cap` is `solver.max_iterations`.
fn case_toml(c: Counts, cap: usize) -> String {
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
        r#"{}
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
[reports.m_blade]
type = "moment"
patch = "blade"
direction = [0.0, 0.0, 1.0]
[solver]
algorithm = "simplec"
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
max_iterations = {cap}
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
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
"#,
        mesh_toml(c)
    )
}

/// The registered mesh-convergence estimate (ADR 0762): for `f(h) = f0 + C h^p` the error of the fine value is
/// `(f2 - f1) / (r21^p - 1)`; the estimate takes the order capped at 1 and the factor of safety. Returns (observed order, half-width).
fn mesh_convergence_estimate(f: [f64; 3], cells: [f64; 3]) -> (f64, f64) {
    let (r21, r32) = ((cells[0] / cells[1]).cbrt(), (cells[1] / cells[2]).cbrt());
    let (e21, e32) = (f[1] - f[0], f[2] - f[1]);
    let s = (e32 / e21).signum();
    let mut p = 1.0f64;
    for _ in 0..500 {
        let q = ((r21.powf(p) - s) / (r32.powf(p) - s)).ln();
        let pn = ((e32 / e21).abs().ln() + q).abs() / r21.ln();
        if !pn.is_finite() {
            return (f64::NAN, f64::NAN);
        }
        let done = (pn - p).abs() < 1e-13;
        p = pn;
        if done {
            break;
        }
    }
    (p, FS * e21.abs() / (r21.powf(p.min(1.0)) - 1.0))
}

#[derive(Debug, Clone)]
struct Outcome {
    cells: usize,
    iterations: usize,
    minutes: f64,
    /// shaft torque [N m], effective moment (the default since ADR 0170)
    torque: f64,
    /// total-to-static pressure rise [Pa]
    dp_ts: f64,
    /// change of torque and pressure rise over the last 1000 iterations (or the last half of a shorter run)
    change: (f64, f64),
    mass_balance: f64,
    res_u: f64,
    res_p: f64,
    /// `Simulation::is_converged()`: the registered convergence rule of F-870
    converged: bool,
    /// the cap rule of ADR 0075 / ADR 0086, reported only
    cap_rule: bool,
    stop: Option<String>,
    /// largest non-orthogonality [deg], skewness, aspect ratio of the mesh of the run
    quality: [f64; 3],
}

impl Outcome {
    fn power(&self) -> f64 {
        self.torque * omega()
    }

    /// The acceptance clauses of F-870 as registered in ADR 0762: A1, A2, A3a, A3b, A3c, A4 (`true` = met).
    fn verdicts(&self) -> [bool; 6] {
        [
            (self.dp_ts - DP_TS_MEAS).abs() <= U_NUM_DP + U_MEAS_DP,
            (self.power() - POWER_MEAS).abs() <= U_NUM_TORQUE * omega() + U_MEAS_POWER,
            self.quality[0] <= GATE_NON_ORTHOGONALITY,
            self.quality[1] <= GATE_SKEWNESS,
            self.quality[2] <= GATE_ASPECT,
            self.converged,
        ]
    }
}

/// Torque and pressure rise with the ADR 0075 definitions.
fn functionals(sim: &Simulation) -> (f64, f64) {
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    (
        -N_BLADES * (mon("m_blade") + mon("m_hub")),
        RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
    )
}

/// The one path of every solving test of this file: the design point on the mesh `c` for at most `cap` iterations. No wall-clock stop
/// (ADR 0762); the steady divergence guard, a non-finite residual or continuity above 1e6 end the run.
fn run_design_point(label: &str, c: Counts, cap: usize) -> Outcome {
    let case = Case::parse_toml(&case_toml(c, cap)).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let cells = sim.geom.mesh.n_cells();
    let q = check_mesh(&sim.geom.mesh);
    let quality = [q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio];
    let t0 = Instant::now();
    let mut hist: Vec<(usize, f64, f64)> = Vec::new();
    let (mut n, mut stop) = (0usize, None);
    let (mut res_u, mut res_p) = (f64::NAN, f64::NAN);
    while n < cap && !sim.is_done() {
        let st = sim.step();
        n += 1;
        (res_u, res_p) = (st.res_u, st.res_p);
        if n % 50 == 0 || sim.is_done() {
            let (a, b) = functionals(&sim);
            hist.push((n, a, b));
        }
        if n % 500 == 0 || n <= 3 || n == 10 || n == 30 || n == 100 {
            let (a, b) = functionals(&sim);
            println!(
                "F870 {label} it {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e} torque {a:.6} dp_ts {b:.4} ({:.1} min)",
                st.continuity,
                st.res_u,
                st.res_p,
                t0.elapsed().as_secs_f64() / 60.0
            );
        }
        if let Some(d) = sim.steady_divergence() {
            stop = Some(format!("steady guard trip at {n}: {} {}", d.kind, d.quantity));
            break;
        }
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) || st.continuity > 1e6 {
            stop = Some(format!("diverged: continuity {:.3e} at iteration {n}", st.continuity));
            break;
        }
    }
    if stop.is_none() {
        stop = sim.failure().map(|f| format!("failure after {n} iterations: {f}"));
    }
    let minutes = t0.elapsed().as_secs_f64() / 60.0;
    let (torque, dp_ts) = functionals(&sim);
    if hist.last().map(|h| h.0) != Some(n) {
        hist.push((n, torque, dp_ts));
    }
    let window = 1000.min(n / 2);
    let back = hist
        .iter()
        .rev()
        .find(|h| h.0 + window <= n)
        .copied()
        .unwrap_or(hist[0]);
    let change = ((torque - back.1).abs(), (dp_ts - back.2).abs());
    let (m_in, m_out) = (
        sim.monitors.last("m_in").unwrap_or(f64::NAN),
        sim.monitors.last("m_out").unwrap_or(f64::NAN),
    );
    let mass_balance = (m_in + m_out).abs() / m_in.abs();
    let converged = stop.is_none() && sim.is_converged();
    let cap_rule = stop.is_none()
        && n >= CAP
        && change.0 < 5e-3 * torque.abs()
        && change.1 < 5e-3 * dp_ts.abs()
        && mass_balance <= 1e-3;
    let o = Outcome {
        cells,
        iterations: n,
        minutes,
        torque,
        dp_ts,
        change,
        mass_balance,
        res_u,
        res_p,
        converged,
        cap_rule,
        stop,
        quality,
    };
    println!(
        "F870 {label} RESULT: {cells} cells, {n} iterations, {:.1} min; torque {torque:.6} N m, shaft power {:.3} W, dp_ts {dp_ts:.4} Pa; \
change over {window}: {:.3e} N m, {:.3e} Pa; mass balance {:.2e}; res_u {:.3e} res_p {:.3e}; is_converged {converged}; cap rule {}; stop {:?}; \
mesh non-orthogonality {:.2} deg, skewness {:.3}, aspect {:.1}",
        o.minutes,
        o.power(),
        o.change.0,
        o.change.1,
        o.mass_balance,
        o.res_u,
        o.res_p,
        o.cap_rule,
        o.stop,
        quality[0],
        quality[1],
        quality[2],
    );
    o
}

/// `a` within the registered tolerance of the state of the recorded value `b`.
fn holds(a: f64, b: f64) -> bool {
    (a - b).abs() <= STATE_TOL * b.abs()
}

fn report_against_the_band(o: &Outcome) {
    let (dp, dw) = ((o.dp_ts - DP_TS_MEAS).abs(), (o.power() - POWER_MEAS).abs());
    println!(
        "F870 BAND: dp_ts {:.3} Pa against {DP_TS_MEAS} Pa: distance {dp:.3} Pa ({:+.1} %), U_num {U_NUM_DP} Pa, u_meas needed {:.3} Pa; \
shaft power {:.3} W against {POWER_MEAS} W: distance {dw:.3} W ({:+.1} %), U_num {:.3} W, u_meas needed {:.3} W; verdicts A1 A2 A3a A3b A3c A4 {:?}",
        o.dp_ts,
        100.0 * (o.dp_ts / DP_TS_MEAS - 1.0),
        (dp - U_NUM_DP).max(0.0),
        o.power(),
        100.0 * (o.power() / POWER_MEAS - 1.0),
        U_NUM_TORQUE * omega(),
        (dw - U_NUM_TORQUE * omega()).max(0.0),
        o.verdicts(),
    );
}

// ---------------------------------------------------------------- run by the gate

/// S2 (ADR 0762, tolerance on the band numbers by amendment 1): the reference is the design row of the transcribed characteristic, the registered band follows from the recorded
/// triplets, and the estimate recovers the exact error of a power law.
#[test]
fn f870_reference_and_band() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_characteristic_n1ug.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("q_m3"))
        .map(|l| l.split(',').map(|v| v.trim().parse::<f64>().unwrap()).collect())
        .collect();
    assert_eq!(rows.len(), 20);
    let d = &rows[12];
    println!("S2 design row: {d:?}");
    assert!((d[0] - Q_DESIGN).abs() <= 5e-5, "{d:?}");
    assert!((d[1] - DP_TS_MEAS).abs() <= 5e-3, "{d:?}");
    assert!((d[2] - POWER_MEAS).abs() <= 5e-3, "{d:?}");
    // no other row is nearer to the design flow
    assert!(rows.iter().all(|r| (r[0] - Q_DESIGN).abs() >= (d[0] - Q_DESIGN).abs()));

    let (p_t, u_t) = mesh_convergence_estimate(TORQUE_0170, CELLS);
    let (p_d, u_d) = mesh_convergence_estimate(DP_0086, CELLS);
    println!(
        "S2 band: torque p {p_t:.4} U_num {u_t:.5} N m = {:.3} W; dp_ts p {p_d:.4} U_num {u_d:.4} Pa",
        u_t * omega()
    );
    assert!((u_t - U_NUM_TORQUE).abs() <= 5e-6, "{u_t}");
    assert!((u_d - U_NUM_DP).abs() <= 5e-3, "{u_d}");
    assert!(
        (p_t - 1.3928).abs() <= 1e-3 && (p_d - 1.7378).abs() <= 1e-3,
        "{p_t} {p_d}"
    );

    // closed form: for f = f0 + C h^p with p <= 1 the estimate is FS times the exact error of the fine value
    for &(p_true, n) in &[(1.0, [8000.0, 2700.0, 1000.0]), (0.7, [38184.0, 13627.0, 4920.0])] {
        let f = |cells: f64| 3.0 + 0.7 * cells.powf(-p_true / 3.0);
        let (p, u) = mesh_convergence_estimate([f(n[0]), f(n[1]), f(n[2])], n);
        let exact = FS * (f(n[0]) - 3.0);
        println!("S2 power law p {p_true}: observed {p:.10}, estimate {u:e}, FS x exact error {exact:e}");
        assert!((p - p_true).abs() <= 1e-9, "{p}");
        assert!((u / exact - 1.0).abs() <= 1e-9, "{u} {exact}");
    }
}

/// S3 (ADR 0762): the B1 mesh has the recorded quality, and each registered gate has its recorded verdict: non-orthogonality MISSED,
/// skewness MISSED, aspect ratio met. A mesh that starts to meet a gate, or moves by more than 0.5 %, fails this test.
#[test]
fn f870_b1_mesh_gates_as_recorded() {
    let case = Case::parse_toml(&case_toml(B1, CAP)).unwrap_or_else(|e| panic!("{e:?}"));
    let m = build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let q = check_mesh(&m);
    let v = [q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio];
    let gates = [GATE_NON_ORTHOGONALITY, GATE_SKEWNESS, GATE_ASPECT];
    println!(
        "S3 B1 mesh: {} cells, non-orthogonality {:.3} deg, skewness {:.4}, aspect {:.2}; gates {gates:?}",
        m.n_cells(),
        v[0],
        v[1],
        v[2]
    );
    assert_eq!(m.n_cells(), 38184);
    for i in 0..3 {
        assert!(
            (v[i] / B1_QUALITY_RECORD[i] - 1.0).abs() <= 5e-3,
            "quality {i}: {} against {}",
            v[i],
            B1_QUALITY_RECORD[i]
        );
        assert_eq!(
            v[i] <= gates[i],
            B1_VERDICTS_RECORD[2 + i],
            "gate {i}: {} against {}",
            v[i],
            gates[i]
        );
    }
}

/// S1 (ADR 0762): the fast test on the path of the heavy record. B3 for 300 iterations; torque and pressure rise at iteration 300
/// within 1 % of the values of the first run.
#[test]
fn f870_b3_short_run_holds_the_state() {
    let o = run_design_point("B3 short", B3, SHORT);
    assert_eq!(o.cells, 4920);
    assert_eq!(o.iterations, SHORT, "{:?}", o.stop);
    assert!(o.stop.is_none(), "{:?}", o.stop);
    assert!(
        holds(o.torque, B3_SHORT_RECORD.0) && holds(o.dp_ts, B3_SHORT_RECORD.1),
        "the state moved: torque {} against {}, dp_ts {} against {}",
        o.torque,
        B3_SHORT_RECORD.0,
        o.dp_ts,
        B3_SHORT_RECORD.1
    );
}

// ---------------------------------------------------------------- heavy records

static B1_OUTCOME: OnceLock<Outcome> = OnceLock::new();

/// The one B1 solve that the two heavy tests share when they run in one process.
fn b1_outcome() -> &'static Outcome {
    B1_OUTCOME.get_or_init(|| {
        let o = run_design_point("B1", B1, CAP);
        report_against_the_band(&o);
        o
    })
}

/// The heavy record of the state (ADR 0762, task 5): B1 to 8000 iterations. Fails if torque or pressure rise move by more than 1 % either
/// way, and if the verdict of any acceptance clause changes (a missed clause that starts to be met is seen here).
#[test]
#[ignore = "heavy record, ADR 0762: the B1 design-point solve (38 184 cells, 8000 iterations, about 1 h alone): cargo test --release -p rcfd-api --test f870_fan01_design_point f870_b1 -- --ignored --nocapture --test-threads 1"]
fn f870_b1_design_point_state_record() {
    let o = b1_outcome();
    assert_eq!(o.cells, 38184);
    assert!(
        holds(o.torque, B1_RECORD.0) && holds(o.dp_ts, B1_RECORD.1),
        "the state moved: torque {} against {}, dp_ts {} against {}",
        o.torque,
        B1_RECORD.0,
        o.dp_ts,
        B1_RECORD.1
    );
    assert_eq!(
        o.verdicts(),
        B1_VERDICTS_RECORD,
        "a verdict of the acceptance changed (A1 A2 A3a A3b A3c A4)"
    );
}

/// The acceptance of F-870 with the limits of ADR 0762 intact: pressure rise and shaft power within the band, the mesh within the
/// registered gates, the solve converged.
#[test]
#[ignore = "heavy, ADR 0762: the acceptance of F-870 on B1 (shares the solve of the state record)"]
fn f870_b1_design_point_acceptance() {
    let o = b1_outcome();
    let v = o.verdicts();
    let names = [
        "A1 pressure rise in the band",
        "A2 shaft power in the band",
        "A3a non-orthogonality",
        "A3b skewness",
        "A3c aspect ratio",
        "A4 converged",
    ];
    let missed: Vec<&str> = names.iter().zip(v).filter(|(_, ok)| !ok).map(|(n, _)| *n).collect();
    assert!(missed.is_empty(), "F-870 MISSED: {missed:?}; {o:?}");
}

/// One registered run of the family, selected by `F870_RUN` = `B1`, `B2` or `B3` (ADR 0762, checks R1 to R3).
#[test]
#[ignore = "heavy, ADR 0762: F870_RUN=B2 cargo test --release -p rcfd-api --test f870_fan01_design_point f870_family_run -- --ignored --nocapture"]
fn f870_family_run() {
    let sel = std::env::var("F870_RUN").expect("set F870_RUN");
    let c = match sel.as_str() {
        "B1" => B1,
        "B2" => B2,
        "B3" => B3,
        other => panic!("F870_RUN = {other}"),
    };
    let o = run_design_point(&sel, c, CAP);
    report_against_the_band(&o);
    println!("F870 {sel} OUTCOME {o:?}");
}
