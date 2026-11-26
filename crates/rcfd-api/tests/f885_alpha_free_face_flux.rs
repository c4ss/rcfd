//! ADR 0811 (F-885): an opt-in face flux whose converged value does not depend on `alpha_u`
//! (`PimpleSettings::alpha_free_face_flux`, N. Kuhl, arXiv:2607.21736, Eq. 25), checked on the cavity of ADR 0767 and ADR 0810
//! (the controls K0 and K4 with the switch on, and two routes with two values of `alpha_u` that must reach the same face flux)
//! and on 20 steps of H(dt) of check H (F0 and F2 of ADR 0810 with the switch on: the jump and the relaxed-loop stall), then check
//! H of ADR 0183 with the switch on, read by the outer-iteration rule of ADR 0183 as written (ADR 0767 E-1). The set-up, the
//! measures, the class and the verdict of check H are copied unchanged from `f885_check_h_outer_record.rs` (ADR 0767), but for
//! the switch. The fan controls and the two check H runs are heavy records; the gate reads their committed files.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RPM: f64 = 1486.0;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), as in ADR 0762
const Q_DESIGN: f64 = 1.4032;
/// ADR 0183 D5: dt = T_c / 100.
const H_DT: f64 = 4.605e-5;
/// ADR 0183 D5: steps of H(dt) and H(dt/2) (0.2532 s, six flow-through times).
const H_STEPS: usize = 5498;
/// ADR 0183 D5: the window is the steps whose time is above this.
const WINDOW_START: f64 = 0.1266;
/// The B2 site cells of ADR 0180 / ADR 0181 (the first wake column behind the hub trailing edge).
const SITE: [usize; 6] = [7, 57, 107, 157, 621, 1235];
/// ADR 0183, check H: the STEADY threshold `0.01 Omega r_hub` as printed there [m/s].
const A_STEADY: f64 = 0.193;
/// ADR 0183, check H: the bands of PHYSICAL and NUMERICAL.
const F_BAND: (f64, f64) = (0.9, 1.1);
const A_BAND: (f64, f64) = (0.75, 1.33);
const MIN_CROSSINGS: usize = 6;
const MIN_STEPS_PER_PERIOD: f64 = 20.0;
const PERIOD_AGREEMENT: f64 = 0.10;
const SHORT_PERIOD_STEPS: f64 = 4.0;
/// ADR 0766, the persistence of a PHYSICAL oscillation: amplitude of the second half of the window over that of the first.
const PERSISTENCE: f64 = 0.75;
/// ADR 0183, check H (ADR 0767 E-1): the velocity change of the 5th outer iteration at most this share of the 1st ...
const OUTER_RATIO: f64 = 1e-2;
/// ... in at least this share of the window's steps.
const OUTER_SHARE: f64 = 0.99;
/// ADR 0766 D-2, reported beside only (ADR 0767 E-1): the pressure ratio and its absolute floor [m^2/s^2].
const D2_FLOOR: f64 = 0.0415;
/// Steps of the fast test on the path of the heavy records.

/// ADR 0810 section 5 (ADR 0811 section 3): the measures' decision numbers.
const J_PRESENT: f64 = 4.0;
const J_ABSENT: f64 = 2.0;
const C_CONVERGES: f64 = 1e-2;
/// K4s: `Q = median d_20 / d_1` at most ADR 0183's outer ratio.
const Q_LIMIT: f64 = 1e-2;
/// TR: "Same discrete problem by two routes"; TRn: the switch off must differ by more than this.
const TR_TOL: f64 = 1e-8;
const TRN_MIN: f64 = 1e-5;
/// A2: ADR 0767 A2, step 1 of the cavity (K0), and the tolerance of its 5 printed digits.
const A2_STEP1: [f64; 5] = [3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3];
const A2_TOL: f64 = 1e-4;
/// The cavity: steps and time step (ADR 0767 section 1).
const CAV_STEPS: usize = 3;
const CAV_DT: f64 = 0.05;
/// The fan controls: steps, and the first step of the steps they are classed over (ADR 0810 section 5).
const FAN_STEPS: usize = 20;
const FAN_FROM: usize = 11;
/// S1 and G2: relative tolerance against the 10 printed digits of a record.
const REC_TOL: f64 = 1e-6;

/// One recorded outcome of the fan controls: the name, its class, its statistic.
type Entry = (&'static str, &'static str, f64);
/// G2: the fan controls' outcomes of the registered run (written after the run).
const FAN_RECORD: Option<&[Entry]> = Some(&[
    ("F0s", "ABSENT", 0.6585016624337551),
    ("F2s:C", "STALLS", 0.02580994273283583),
]);

/// A class with its verdict.
type Outcome = (Class, Verdict);
/// What G1 holds: class, verdict, and per run (H(dt), H(dt/2)) the share of the window's steps that meet the rule of E-1, the
/// amplitude [m/s] and the frequency [Hz] of the site signal.
type Recorded = (Class, Verdict, [(f64, f64, f64); 2]);
/// G1: the recorded values of the two registered runs (written after the runs).
const RECORD: Option<Recorded> = Some((
    Class::NotEvaluable,
    Verdict::DoesNotDecide,
    [(0.0, 1.451348e-3, f64::NAN), (0.0, 2.288205e-3, f64::NAN)],
));

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// Route B: the largest Euclidean norm over the cells of `b - a`, formed in the test.
fn change(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| {
        m.max(((y[0] - x[0]).powi(2) + (y[1] - x[1]).powi(2) + (y[2] - x[2]).powi(2)).sqrt())
    })
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

// ---------------------------------------------------------------- the case of check H (ADR 0766, copied)

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1).
fn sections() -> Vec<Vec<f64>> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
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

/// The case text of check H, as `h_text` of `f879_fan01_check_h.rs` (ADR 0766: the ADR 0086 bladed B2 case with the solver of
/// ADR 0183 D5, the seam diffusivity `pair` of D-1 and the six reports of D-4).
fn h_text(dt: f64) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    // B2 of ADR 0086
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = (8, 17, 8, 11, 4, 15, 7);
    let mesh = format!(
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
    );
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
        r#"{mesh}
[physics]
nu = 1.5e-5
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[turbulence.periodic_seam]
diffusivity = "pair"
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
[solver]
algorithm = "pimple"
n_outer = 5
time_scheme = "bdf2"
dt = {dt:?}
end_time = 0.2532
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
[reports.m_blade]
type = "moment"
patch = "blade"
direction = [0.0, 0.0, 1.0]
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
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
"#
    )
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

fn seam_pair_sum(sim: &Simulation) -> f64 {
    let g = &sim.geom;
    let phi = sim.flux();
    let (mut sum, mut big) = (0.0_f64, 0.0_f64);
    for (pi, patch) in g.mesh.patches().iter().enumerate() {
        let (Some(map), Some(_)) = (&g.periodic_partner[pi], &g.periodic_rot[pi]) else {
            continue;
        };
        for (j, f) in patch.faces().enumerate() {
            sum = sum.max((phi[f] + phi[map[j]]).abs());
            big = big.max(phi[f].abs());
        }
    }
    sum / big
}

// ---------------------------------------------------------------- the step series and its measures

/// One step of a run of check H.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Row {
    step: usize,
    /// `step * dt` [s]
    t: f64,
    /// relative speed of the six site cells [m/s]
    w: [f64; 6],
    /// smallest axial relative velocity of the six site cells [m/s]
    wz_min: f64,
    /// shaft torque `-9 (m_blade + m_hub)` [N m]
    torque: f64,
    /// total-to-static pressure rise [Pa]
    dp: f64,
    /// the recorded pressure ratio `max |p_4 - p_3| / max |p_1 - p^n|` (ADR 0766 D-2; reported only)
    ratio: f64,
    /// `max |p_1 - p^n|` of the step [m^2/s^2]
    first: f64,
    res_u: f64,
    continuity: f64,
    /// the velocity change of the outer iterations 1 to 5 of the step (ADR 0767) [m/s]
    dv: [f64; 5],
    /// `max_c |u^(n+1) - u^n|` of the step [m/s] (reported only)
    du_step: f64,
}

/// A run: its time step, its rows, and the stop if it did not complete.
#[derive(Debug, Clone)]
struct Series {
    dt: f64,
    steps: usize,
    rows: Vec<Row>,
    failed: Option<String>,
}

/// Mean, half range, upward zero crossings of the mean-removed signal, the frequency from them and the DFT peak (ADR 0183, check H;
/// ADR 0766 D-5).
#[derive(Debug, Clone, Copy)]
struct Meas {
    mean: f64,
    amp: f64,
    crossings: usize,
    /// `(n - 1) / (t_n - t_1)`; NaN with fewer than two crossings
    freq: f64,
    /// frequency of the largest modulus of the discrete Fourier sum, zero frequency left out
    dft: f64,
}

fn measure(t: &[f64], x: &[f64]) -> Meas {
    let n = x.len();
    assert!(n >= 2 && t.len() == n);
    let mean = x.iter().sum::<f64>() / n as f64;
    let (lo, hi) = x
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    let mut cross: Vec<f64> = Vec::new();
    for i in 1..n {
        let (a, b) = (x[i - 1] - mean, x[i] - mean);
        if a < 0.0 && b >= 0.0 {
            cross.push(t[i - 1] + (t[i] - t[i - 1]) * (-a) / (b - a));
        }
    }
    let freq = if cross.len() >= 2 {
        (cross.len() - 1) as f64 / (cross[cross.len() - 1] - cross[0])
    } else {
        f64::NAN
    };
    // the discrete Fourier sum at the multiples of 1 / (window length)
    let length = (t[n - 1] - t[0]) * n as f64 / (n - 1) as f64;
    let (mut best, mut dft) = (0.0f64, f64::NAN);
    for k in 1..=n / 2 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        let w = 2.0 * PI * k as f64 / n as f64;
        for (i, &v) in x.iter().enumerate() {
            let (s, c) = (w * i as f64).sin_cos();
            re += (v - mean) * c;
            im -= (v - mean) * s;
        }
        let m = re.hypot(im);
        if m > best {
            best = m;
            dft = k as f64 / length;
        }
    }
    Meas {
        mean,
        amp: 0.5 * (hi - lo),
        crossings: cross.len(),
        freq,
        dft,
    }
}

/// Smallest, median and largest of the finite values (NaN when none).
fn spread(v: &[f64]) -> (f64, f64, f64) {
    let mut x: Vec<f64> = v.iter().copied().filter(|a| a.is_finite()).collect();
    if x.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (x[0], x[x.len() / 2], x[x.len() - 1])
}

/// Values in scientific notation, for the logs.
fn sci(v: &[f64]) -> String {
    let x: Vec<String> = v.iter().map(|a| format!("{a:.4e}")).collect();
    format!("[{}]", x.join(", "))
}

/// The step meets the outer-iteration rule of ADR 0183 as written (ADR 0767 E-1).
fn meets_rule(r: &Row) -> bool {
    r.dv[4] <= OUTER_RATIO * r.dv[0]
}

/// The measures of one run over its window.
#[derive(Debug, Clone)]
struct Summary {
    dt: f64,
    steps: usize,
    window_steps: usize,
    failed: bool,
    /// share of the window's steps that meet the rule of ADR 0767 E-1
    share_rule: f64,
    /// share that meet the rule of ADR 0766 D-2 (reported only)
    share_d2: f64,
    time_accurate: bool,
    /// index into `SITE` of the cell with the largest amplitude
    cell: usize,
    site: Meas,
    /// amplitude of that cell over the first and the second half of the window
    halves: (f64, f64),
    torque: Meas,
    dp: Meas,
    wz_min: f64,
}

impl Summary {
    /// `1 / (f dt)`; NaN where no frequency is defined.
    fn period_steps(&self) -> f64 {
        1.0 / (self.site.freq * self.dt)
    }
}

fn half_range(x: &[f64]) -> f64 {
    let (lo, hi) = x
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    0.5 * (hi - lo)
}

fn window(s: &Series) -> Vec<&Row> {
    s.rows.iter().filter(|r| r.t > WINDOW_START).collect()
}

fn summarise(s: &Series) -> Summary {
    let win = window(s);
    let failed = s.failed.is_some() || s.rows.len() != s.steps || win.len() < 4;
    if win.len() < 4 {
        let nan = Meas {
            mean: f64::NAN,
            amp: f64::NAN,
            crossings: 0,
            freq: f64::NAN,
            dft: f64::NAN,
        };
        return Summary {
            dt: s.dt,
            steps: s.rows.len(),
            window_steps: win.len(),
            failed,
            share_rule: 0.0,
            share_d2: 0.0,
            time_accurate: false,
            cell: 0,
            site: nan,
            halves: (f64::NAN, f64::NAN),
            torque: nan,
            dp: nan,
            wz_min: f64::NAN,
        };
    }
    let t: Vec<f64> = win.iter().map(|r| r.t).collect();
    let col = |c: usize| -> Vec<f64> { win.iter().map(|r| r.w[c]).collect() };
    let cell = (0..6)
        .max_by(|&a, &b| half_range(&col(a)).partial_cmp(&half_range(&col(b))).unwrap())
        .unwrap();
    let x = col(cell);
    let h = x.len() / 2;
    let n = win.len() as f64;
    let by_rule = win.iter().filter(|r| meets_rule(r)).count() as f64 / n;
    let by_d2 = win
        .iter()
        .filter(|r| r.ratio <= OUTER_RATIO || (r.ratio.is_finite() && r.ratio * r.first <= D2_FLOOR))
        .count() as f64
        / n;
    Summary {
        dt: s.dt,
        steps: s.rows.len(),
        window_steps: win.len(),
        failed,
        share_rule: by_rule,
        share_d2: by_d2,
        time_accurate: by_rule >= OUTER_SHARE,
        cell,
        site: measure(&t, &x),
        halves: (half_range(&x[..h]), half_range(&x[h..])),
        torque: measure(&t, &win.iter().map(|r| r.torque).collect::<Vec<f64>>()),
        dp: measure(&t, &win.iter().map(|r| r.dp).collect::<Vec<f64>>()),
        wz_min: win.iter().map(|r| r.wz_min).fold(f64::INFINITY, f64::min),
    }
}

/// The class of ADR 0183, check H.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    NotEvaluable,
    Steady,
    Physical,
    Numerical,
}

/// The verdict of F-879 (ADR 0766).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Steady,
    PhysicallyUnsteady,
    Numerical,
    DoesNotDecide,
}

fn within(x: f64, band: (f64, f64)) -> bool {
    x >= band.0 && x <= band.1
}

/// ADR 0183, check H, "Class (exactly one; tested in this order)", with the readings of ADR 0766 D-5 and the outer rule of ADR
/// 0767 E-1. `a` is H(dt), `b` H(dt/2).
fn class_0183(a: &Summary, b: &Summary) -> Class {
    if a.failed || b.failed || !a.time_accurate || !b.time_accurate {
        return Class::NotEvaluable;
    }
    let (aa, ab) = (a.site.amp, b.site.amp);
    if aa <= A_STEADY && ab <= A_STEADY {
        return Class::Steady;
    }
    if !(aa > A_STEADY && ab > A_STEADY) {
        return Class::NotEvaluable;
    }
    let (pa, pb) = (a.period_steps(), b.period_steps());
    if a.site.crossings >= MIN_CROSSINGS
        && b.site.crossings >= MIN_CROSSINGS
        && pa >= MIN_STEPS_PER_PERIOD
        && within(b.site.freq / a.site.freq, F_BAND)
        && within(ab / aa, A_BAND)
    {
        return Class::Physical;
    }
    if (pb / pa - 1.0).abs() <= PERIOD_AGREEMENT || pa <= SHORT_PERIOD_STEPS || pb <= SHORT_PERIOD_STEPS {
        return Class::Numerical;
    }
    Class::NotEvaluable
}

/// The verdict of F-879: the class of ADR 0183, with PHYSICAL narrowed by persistence (ADR 0766).
fn verdict(a: &Summary, b: &Summary) -> Outcome {
    let c = class_0183(a, b);
    let v = match c {
        Class::Steady => Verdict::Steady,
        Class::Numerical => Verdict::Numerical,
        Class::NotEvaluable => Verdict::DoesNotDecide,
        Class::Physical => {
            if a.halves.1 / a.halves.0 >= PERSISTENCE && b.halves.1 / b.halves.0 >= PERSISTENCE {
                Verdict::PhysicallyUnsteady
            } else {
                Verdict::DoesNotDecide
            }
        }
    };
    (c, v)
}

fn print_summary(label: &str, s: &Summary) {
    let m = |name: &str, unit: &str, m: &Meas| {
        format!(
            "{name}: mean {:.6} {unit}, A {:.6e} {unit}, crossings {}, f {:.4} Hz, DFT peak {:.2} Hz",
            m.mean, m.amp, m.crossings, m.freq, m.dft
        )
    };
    println!(
        "F885 {label} SUMMARY: dt {:e} s, {} steps ({:.3} revolutions), window {} steps; failed {}; outer rule of ADR 0183 (E-1): {:.6} of the window's steps, TIME-ACCURATE {}; ADR 0766 D-2 on the same run {:.6}; \
max-A site cell c{}; {}; halves A1 {:.6e} A2 {:.6e} (A2/A1 {:.4}); period {:.2} steps; smallest axial relative velocity at the site {:.4} m/s; {}; {}; shaft power mean {:.3} W",
        s.dt,
        s.steps,
        s.steps as f64 * s.dt * RPM / 60.0,
        s.window_steps,
        s.failed,
        s.share_rule,
        s.time_accurate,
        s.share_d2,
        SITE[s.cell],
        m("site speed", "m/s", &s.site),
        s.halves.0,
        s.halves.1,
        s.halves.1 / s.halves.0,
        s.period_steps(),
        s.wz_min,
        m("torque", "N m", &s.torque),
        m("pressure rise", "Pa", &s.dp),
        s.torque.mean * omega(),
    );
}

/// The outer record over the window, reported without a limit (ADR 0767, "Reported beside").
fn print_outer(label: &str, s: &Series) {
    let win = window(s);
    let col = |f: &dyn Fn(&Row) -> f64| {
        let (a, b, c) = spread(&win.iter().map(|r| f(r)).collect::<Vec<f64>>());
        sci(&[a, b, c])
    };
    println!(
        "F885 {label} OUTER: d5/d1 (min, median, max) {}; d1 {}; d5 {}; d2/d1 {}; d3/d2 {}; d4/d3 {}; d5/d4 {}; step change {}; step change / d1 {}; pressure ratio {}",
        col(&|r| r.dv[4] / r.dv[0]),
        col(&|r| r.dv[0]),
        col(&|r| r.dv[4]),
        col(&|r| r.dv[1] / r.dv[0]),
        col(&|r| r.dv[2] / r.dv[1]),
        col(&|r| r.dv[3] / r.dv[2]),
        col(&|r| r.dv[4] / r.dv[3]),
        col(&|r| r.du_step),
        col(&|r| r.du_step / r.dv[0]),
        col(&|r| r.ratio),
    );
    // by flow-through time, the median of d5/d1 and the share that meets the rule
    let sixth = s.rows.len() / 6;
    for i in 0..6 {
        let part = &s.rows[i * sixth..((i + 1) * sixth).min(s.rows.len())];
        if part.is_empty() {
            continue;
        }
        let (q0, q1, q2) = spread(&part.iter().map(|r| r.dv[4] / r.dv[0]).collect::<Vec<f64>>());
        let q = sci(&[q0, q1, q2]);
        let share = part.iter().filter(|r| meets_rule(r)).count() as f64 / part.len() as f64;
        println!(
            "F885 {label} OUTER flow-through time {}: d5/d1 {q}, share meeting the rule {share:.4}, d1 median {:.4e}",
            i + 1,
            spread(&part.iter().map(|r| r.dv[0]).collect::<Vec<f64>>()).1
        );
    }
}

// ---------------------------------------------------------------- the record files (ADR 0767, copied)

const HEADER: &str =
    "step,t,w_c7,w_c57,w_c107,w_c157,w_c621,w_c1235,wz_min,torque,dp_ts,outer_ratio,outer_first,res_u,continuity";
const OUTER_HEADER: &str = "step,d1,d2,d3,d4,d5,du_step";

/// The full rows, in ADR 0766's form with the outer record after them (committed: ADR 0811 section 4, ADR 0767 E-3).
fn write_full(path: &Path, s: &Series) {
    let mut out = format!(
        "# ADR 0811 (F-885), check H of ADR 0183 with alpha_free_face_flux: dt = {:?} s, {} steps registered; one row per step\n{HEADER},d1,d2,d3,d4,d5,du_step\n",
        s.dt, s.steps
    );
    for r in &s.rows {
        write!(out, "{},{:.9e}", r.step, r.t).unwrap();
        for v in
            r.w.iter()
                .chain([&r.wz_min, &r.torque, &r.dp, &r.ratio, &r.first, &r.res_u, &r.continuity])
                .chain(r.dv.iter())
                .chain([&r.du_step])
        {
            write!(out, ",{v:.9e}").unwrap();
        }
        out.push('\n');
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// The outer record of a run: the step, `d_1` to `d_5` and the step change (ADR 0767 E-3).
fn write_outer(path: &Path, s: &Series) {
    let mut out = format!(
        "# ADR 0811 (F-885), the velocity change of each outer iteration in check H with alpha_free_face_flux: dt = {:?} s; one row per step [m/s]\n{OUTER_HEADER}\n",
        s.dt
    );
    for r in &s.rows {
        write!(out, "{}", r.step).unwrap();
        for v in r.dv.iter().chain([&r.du_step]) {
            write!(out, ",{v:.9e}").unwrap();
        }
        out.push('\n');
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn data_lines(path: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("step"))
        .map(|l| l.split(',').map(|c| c.trim().to_string()).collect())
        .collect()
}

fn num(c: &[String], i: usize) -> f64 {
    c[i].parse::<f64>().unwrap_or_else(|e| panic!("{c:?}: {e}"))
}

// ---------------------------------------------------------------- the run (ADR 0767, copied; the switch added)

/// The one path of every solving test of check H in this file: `steps` steps at the time step `dt`. With `record` the solver
/// has the switch of ADR 0811 on, and
/// keeps both outer records (ADR 0767 E-2: the pressure record of ADR 0766 D-2 with a tolerance that is never met, and the velocity
/// record). No wall-clock stop (ADR 0766 D-3); a failure of the simulation or a non-finite value ends the run.
fn run_h(label: &str, dt: f64, steps: usize, record: bool) -> Series {
    let mut sim = sim_of(&h_text(dt));
    let cells = sim.geom.mesh.n_cells();
    assert_eq!(cells, 13627);
    if record {
        let s = &mut sim.pimple_solver_mut().expect("pimple").settings;
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        // ADR 0811 section 4: the one line by which check H departs from ADR 0767
        s.alpha_free_face_flux = true;
    }
    let u_in = Q_DESIGN / annulus_area();
    let full = steps == H_STEPS || steps == 2 * H_STEPS;
    let (mut lo, mut hi) = (vec![f64::INFINITY; cells], vec![f64::NEG_INFINITY; cells]);
    let mut seam = 0.0f64;
    let mut rows: Vec<Row> = Vec::with_capacity(steps);
    let mut failed = None;
    let mut prev: Vec<Vec3> = sim.velocity().values.clone();
    let t0 = Instant::now();
    for k in 1..=steps {
        let st = sim.step();
        let t = k as f64 * dt;
        let u = &sim.velocity().values;
        let speed = |c: usize| (u[c][0] * u[c][0] + u[c][1] * u[c][1] + u[c][2] * u[c][2]).sqrt();
        let mut w = [0.0; 6];
        for (i, &c) in SITE.iter().enumerate() {
            w[i] = speed(c);
        }
        let wz_min = SITE.iter().map(|&c| u[c][2]).fold(f64::INFINITY, f64::min);
        let du_step = change(&prev, u);
        prev.clone_from(u);
        if full && t > WINDOW_START {
            for c in 0..cells {
                let v = speed(c);
                lo[c] = lo[c].min(v);
                hi[c] = hi[c].max(v);
            }
            if k % 50 == 0 {
                seam = seam.max(seam_pair_sum(&sim));
            }
        }
        let mon = |name: &str| sim.monitors.last(name).unwrap_or(f64::NAN);
        let torque = -N_BLADES * (mon("m_blade") + mon("m_hub"));
        let dp = RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in));
        let ps = sim.pimple_solver();
        let (ratio, first) = match ps.and_then(|s| s.outer_record.last()) {
            Some(r) if record => {
                assert_eq!((r.step, r.used, r.met), (k, 5, false), "outer record of step {k}");
                (r.ratio, r.first)
            }
            _ => (f64::NAN, f64::NAN),
        };
        let dv = match ps.and_then(|s| s.outer_velocity.last()) {
            Some(r) if record => {
                assert_eq!((r.step, r.change.len()), (k, 5), "velocity record of step {k}");
                [r.change[0], r.change[1], r.change[2], r.change[3], r.change[4]]
            }
            _ => [f64::NAN; 5],
        };
        rows.push(Row {
            step: k,
            t,
            w,
            wz_min,
            torque,
            dp,
            ratio,
            first,
            res_u: st.res_u,
            continuity: st.continuity,
            dv,
            du_step,
        });
        if k % 250 == 0 || k <= 2 || k == steps {
            println!(
                "F885 {label} step {k}: t {t:.5e} s, res_u {:.3e}, continuity {:.3e}, d1..d5 {} (d5/d1 {:.3e}), step change {du_step:.3e}, pressure ratio {ratio:.3e}, site speeds {w:.4?}, torque {torque:.6} N m, dp_ts {dp:.4} Pa, {:.3} s per step",
                st.res_u,
                st.continuity,
                sci(&dv),
                dv[4] / dv[0],
                t0.elapsed().as_secs_f64() / k as f64
            );
        }
        let finite = w.iter().all(|v| v.is_finite()) && torque.is_finite() && dp.is_finite() && st.res_u.is_finite();
        if let Some(f) = sim.failure() {
            failed = Some(format!("failure at step {k}: {f}"));
            break;
        }
        if !finite {
            failed = Some(format!("non-finite value at step {k}"));
            break;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let fields_finite = ["ux", "uy", "uz", "p", "k", "omega"]
        .iter()
        .all(|f| sim.field(f).unwrap().iter().all(|x| x.is_finite()));
    if failed.is_none() && !fields_finite {
        failed = Some("a field is not finite at the end".into());
    }
    let (m_in, m_out) = (
        sim.monitors.last("m_in").unwrap_or(f64::NAN),
        sim.monitors.last("m_out").unwrap_or(f64::NAN),
    );
    println!(
        "F885 {label} END: {} of {steps} steps, failed {failed:?}, fields finite {fields_finite}, {:.3} s per step ({:.1} min), mass balance {:.3e}, solver time {:.6e} s",
        rows.len(),
        secs / rows.len().max(1) as f64,
        secs / 60.0,
        (m_in + m_out).abs() / m_in.abs(),
        sim.solver_time(),
    );
    if full {
        let mut amp: Vec<(usize, f64)> = (0..cells).map(|c| (c, 0.5 * (hi[c] - lo[c]))).collect();
        amp.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let cc = sim.geom.mesh.cell_centres();
        let top: Vec<String> = amp
            .iter()
            .take(5)
            .map(|&(c, a)| {
                format!(
                    "c{c} A {a:.4} m/s (r {:.4}, z {:.4})",
                    cc[c][0].hypot(cc[c][1]),
                    cc[c][2]
                )
            })
            .collect();
        println!("F885 {label} REPORT: five cells of largest amplitude over the window: {top:?}; largest seam-pair flux sum (every 50 steps of the window) {seam:.3e}");
    }
    Series {
        dt,
        steps,
        rows,
        failed,
    }
}

// ---------------------------------------------------------------- the measures of ADR 0810 section 5 (copied)

/// The steps of one run: per step, `d_k` and the cell of each outer iteration; `failed` if it stopped.
#[derive(Debug, Clone)]
struct Run {
    name: String,
    n_outer: usize,
    steps: Vec<(usize, Vec<f64>, Vec<usize>)>,
    planned: usize,
    failed: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Jump {
    Present,
    Absent,
    Undecided,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conv {
    Converges,
    Stalls,
    Undecided,
}

/// The upper median `x[len / 2]` of the sorted values; NaN if any is NaN or there are none.
fn upper_median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() || v.iter().any(|x| x.is_nan()) {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// The steps a run is classed over (from step `from` on); empty if the run did not complete.
fn classed(run: &Run, from: usize) -> Vec<&Vec<f64>> {
    if run.failed.is_some() || run.steps.len() != run.planned {
        return Vec::new();
    }
    run.steps.iter().filter(|s| s.0 >= from).map(|s| &s.1).collect()
}

/// `J` = median of `d_N / d_(N-1)` and its class.
fn jump(run: &Run, from: usize) -> (f64, Jump) {
    let n = run.n_outer;
    let s = classed(run, from);
    let j = upper_median(s.iter().map(|d| d[n - 1] / d[n - 2]).collect());
    let c = if j >= J_PRESENT {
        Jump::Present
    } else if j <= J_ABSENT {
        Jump::Absent
    } else {
        Jump::Undecided
    };
    (j, c)
}

/// For `N = 20`: `C` = median of `d_19 / d_2` and its class.
fn conv(run: &Run, from: usize) -> (f64, Conv) {
    assert_eq!(run.n_outer, 20);
    let c = upper_median(classed(run, from).iter().map(|d| d[18] / d[1]).collect());
    let k = if c <= C_CONVERGES {
        Conv::Converges
    } else if c > C_CONVERGES {
        Conv::Stalls
    } else {
        Conv::Undecided
    };
    (c, k)
}

/// For `N = 20`: `Q` = median of `d_20 / d_1`.
fn q_rule(run: &Run, from: usize) -> f64 {
    upper_median(classed(run, from).iter().map(|d| d[19] / d[0]).collect())
}

fn jname(j: Jump) -> &'static str {
    match j {
        Jump::Present => "PRESENT",
        Jump::Absent => "ABSENT",
        Jump::Undecided => "UNDECIDED",
    }
}

fn cname(c: Conv) -> &'static str {
    match c {
        Conv::Converges => "CONVERGES",
        Conv::Stalls => "STALLS",
        Conv::Undecided => "UNDECIDED",
    }
}

// ---------------------------------------------------------------- the cavity (ADR 0767 section 1, ADR 0811 section 3)

/// The 8 x 8 lid-driven cavity of ADR 0767 from rest, the record on, the switch `af`, the settings changed by `set`.
fn cavity(n_outer: usize, af: bool, set: impl Fn(&mut PimpleSettings)) -> PimpleSolver {
    let mesh = box_2d(8, 8, 1.0, 1.0, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSettings::pimple(0.01, n_outer);
    s.time_scheme = TimeScheme::Bdf2;
    s.outer_velocity_record = true;
    s.alpha_free_face_flux = af;
    set(&mut s);
    PimpleSolver::new(geom, u, p, s)
}

/// The baseline settings of the cavity (K0 of ADR 0810): `alpha_u = 0.8`, `alpha_p = 0.5`.
fn k0(_: &mut PimpleSettings) {}

/// A cavity control: `CAV_STEPS` steps and the record of each.
fn cavity_run(name: &str, n_outer: usize, af: bool, set: impl Fn(&mut PimpleSettings)) -> Run {
    let mut sol = cavity(n_outer, af, set);
    let mut run = Run {
        name: name.into(),
        n_outer,
        steps: Vec::new(),
        planned: CAV_STEPS,
        failed: None,
    };
    for n in 1..=CAV_STEPS {
        sol.step(CAV_DT);
        match sol.outer_velocity.last() {
            Some(r) if r.step == n && r.change.len() == n_outer => {
                run.steps.push((n, r.change.clone(), r.cell.clone()));
            }
            _ => {
                run.failed = Some(format!("no complete record at step {n}"));
                break;
            }
        }
    }
    for (n, d, c) in &run.steps {
        println!("F885-AF {name} step {n}: d {} cells {c:?}", sci(d));
    }
    run
}

/// A2 (ADR 0811): the switch on with `alpha_u = alpha_p = 1` gives the bits of the switch off; K0 with the switch off is ADR
/// 0767 A2 at its printed digits.
#[test]
fn f885_af_defaults_and_bits() {
    let one = |s: &mut PimpleSettings| {
        s.alpha_u = 1.0;
        s.alpha_p = 1.0;
    };
    let (mut on, mut off) = (cavity(5, true, one), cavity(5, false, one));
    for _ in 0..CAV_STEPS {
        on.step(CAV_DT);
        off.step(CAV_DT);
    }
    let ub = |s: &PimpleSolver| -> Vec<u64> { s.u.values.iter().flat_map(|v| v.map(f64::to_bits)).collect() };
    let fb = |x: &[f64]| -> Vec<u64> { x.iter().map(|v| v.to_bits()).collect() };
    let rb = |s: &PimpleSolver| -> Vec<u64> {
        s.outer_velocity
            .iter()
            .flat_map(|r| r.change.iter().map(|v| v.to_bits()))
            .collect()
    };
    println!(
        "A2: alpha 1, switch on and off: u equal {}, p equal {}, flux equal {}, record equal {}",
        ub(&on) == ub(&off),
        fb(&on.p.values) == fb(&off.p.values),
        fb(&on.phi) == fb(&off.phi),
        rb(&on) == rb(&off)
    );
    let k0off = cavity_run("K0 switch off", 5, false, k0);
    let s1 = &k0off.steps[0].1;
    println!(
        "A2: K0 switch off, step 1 {} against ADR 0767 A2 {}",
        sci(s1),
        sci(&A2_STEP1)
    );
    assert_eq!(ub(&on), ub(&off), "A2: u");
    assert_eq!(fb(&on.p.values), fb(&off.p.values), "A2: p");
    assert_eq!(fb(&on.phi), fb(&off.phi), "A2: flux");
    assert_eq!(rb(&on), rb(&off), "A2: record");
    for (a, b) in s1.iter().zip(A2_STEP1) {
        assert!(rel(*a, b) <= A2_TOL, "A2: {a} against {b}");
    }
}

/// The largest relative difference over the steps between two routes: `N = 30`, `alpha_p = 1`, `alpha_u` 0.8 (a) and 0.9 (b),
/// the switch `af`; per step `max_f |phi_a - phi_b| / max_f |phi_a|` and the same of the cell velocity.
fn two_routes(af: bool) -> f64 {
    let set = |au: f64| {
        move |s: &mut PimpleSettings| {
            s.alpha_p = 1.0;
            s.alpha_u = au;
        }
    };
    let (mut a, mut b) = (cavity(30, af, set(0.8)), cavity(30, af, set(0.9)));
    let mut worst = 0.0f64;
    for n in 1..=CAV_STEPS {
        a.step(CAV_DT);
        b.step(CAV_DT);
        let big = |x: &[f64]| x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let dphi = a.phi.iter().zip(&b.phi).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / big(&a.phi);
        let zero = vec![[0.0; 3]; a.u.values.len()];
        let du = change(&a.u.values, &b.u.values) / change(&zero, &a.u.values);
        let last = |s: &PimpleSolver| s.outer_velocity.last().map_or(f64::NAN, |r| r.change[28]);
        println!(
            "F885-AF TR switch {af} step {n}: flux {dphi:.6e}, velocity {du:.6e} relative; d_29 {:.3e} (a), {:.3e} (b)",
            last(&a),
            last(&b)
        );
        if !(dphi.is_finite() && du.is_finite()) {
            return f64::NAN;
        }
        worst = worst.max(dphi).max(du);
    }
    worst
}

/// K0s, K4s, TR, TRn (ADR 0811 section 3): the switch on the cavity.
#[test]
fn f885_af_cavity() {
    let mut miss = Vec::new();
    let k0s = cavity_run("K0s", 5, true, k0);
    let (j0, c0) = jump(&k0s, 1);
    println!("F885-AF K0s J {j0:.6e} {}", jname(c0));
    if c0 != Jump::Absent {
        miss.push(format!("K0s: J {j0}"));
    }
    let k4s = cavity_run("K4s", 20, true, k0);
    let (j4, c4) = jump(&k4s, 1);
    let (cv, cc) = conv(&k4s, 1);
    let q = q_rule(&k4s, 1);
    println!(
        "F885-AF K4s J {j4:.6e} {}, C {cv:.6e} {}, Q {q:.6e} (limit {Q_LIMIT:e})",
        jname(c4),
        cname(cc)
    );
    if c4 != Jump::Absent || cc != Conv::Converges || !(q <= Q_LIMIT) {
        miss.push(format!("K4s: J {j4}, C {cv}, Q {q}"));
    }
    let tr = two_routes(true);
    let trn = two_routes(false);
    println!("F885-AF TR {tr:.6e} (limit {TR_TOL:e}); TRn {trn:.6e} (must exceed {TRN_MIN:e})");
    if !(tr <= TR_TOL) {
        miss.push(format!("TR: {tr}"));
    }
    if !(trn > TRN_MIN) {
        miss.push(format!("TRn: {trn}"));
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}

// ---------------------------------------------------------------- the fan controls (ADR 0811 sections 2 and 3)

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-alpha-free-face-flux")
}

/// The fan controls with the switch on: name, outer count.
const FAN_CONTROLS: [(&str, usize); 2] = [("F0s", 5), ("F2s", 20)];

/// One fan control: 20 steps of H(dt) with both records of ADR 0767 E-2 and the switch on; a failure or a non-finite value ends
/// the run.
fn fan_run(name: &str, n_outer: usize) -> (Run, Vec<[f64; 3]>) {
    let mut sim = sim_of(&h_text(H_DT));
    assert_eq!(sim.geom.mesh.n_cells(), 13627);
    {
        let s = &mut sim.pimple_solver_mut().expect("pimple").settings;
        assert_eq!((s.alpha_u, s.alpha_p, s.n_outer), (0.9, 1.0, 5), "the case's settings");
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        s.alpha_free_face_flux = true;
        s.n_outer = n_outer;
    }
    let cc: Vec<[f64; 3]> = sim.geom.mesh.cell_centres().to_vec();
    let mut run = Run {
        name: name.into(),
        n_outer,
        steps: Vec::new(),
        planned: FAN_STEPS,
        failed: None,
    };
    let t0 = Instant::now();
    for k in 1..=FAN_STEPS {
        let st = sim.step();
        if let Some(f) = sim.failure() {
            run.failed = Some(format!("failure at step {k}: {f}"));
            break;
        }
        let rec = sim.pimple_solver().and_then(|s| s.outer_velocity.last()).cloned();
        match rec {
            Some(r) if r.step == k && r.change.len() == n_outer && st.res_u.is_finite() => {
                if !r.change.iter().all(|d| d.is_finite()) {
                    run.failed = Some(format!("non-finite change at step {k}"));
                    break;
                }
                println!(
                    "F885-AF {name} step {k}: d {} cells {:?}, {:.2} s per step",
                    sci(&r.change),
                    r.cell,
                    t0.elapsed().as_secs_f64() / k as f64
                );
                run.steps.push((k, r.change, r.cell));
            }
            _ => {
                run.failed = Some(format!("no complete record or a non-finite residual at step {k}"));
                break;
            }
        }
    }
    println!(
        "F885-AF {name} END: {} of {FAN_STEPS} steps, failed {:?}, {:.1} s",
        run.steps.len(),
        run.failed,
        t0.elapsed().as_secs_f64()
    );
    (run, cc)
}

const FAN_HEADER: &str = "run,step,k,d,cell";

/// The fan record: one row per run, step and outer iteration; a comment line per run with its stop (if any).
fn write_fan(path: &Path, runs: &[Run]) {
    let mut out = format!(
        "# ADR 0811 (F-885): the fan controls F0s and F2s (alpha_free_face_flux on) on 20 steps of H(dt) (dt = {H_DT:?} s); d [m/s] the velocity change of outer iteration k, cell its cell\n{FAN_HEADER}\n"
    );
    for r in runs {
        writeln!(
            out,
            "# {} n_outer {} steps {} failed {:?}",
            r.name,
            r.n_outer,
            r.steps.len(),
            r.failed
        )
        .unwrap();
        for (n, d, c) in &r.steps {
            for k in 0..r.n_outer {
                writeln!(out, "{},{n},{},{:.9e},{}", r.name, k + 1, d[k], c[k]).unwrap();
            }
        }
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

/// The fan runs from the record.
fn read_fan(path: &Path) -> Vec<Run> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut runs: Vec<Run> = FAN_CONTROLS
        .iter()
        .map(|(name, n)| Run {
            name: (*name).into(),
            n_outer: *n,
            steps: Vec::new(),
            planned: FAN_STEPS,
            failed: None,
        })
        .collect();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix("# F") {
            if let Some(i) = rest.find(" failed Some") {
                let name = format!("F{}", &rest[..rest.find(' ').unwrap()]);
                let r = runs.iter_mut().find(|r| r.name == name).unwrap();
                r.failed = Some(rest[i + 8..].to_string());
            }
            continue;
        }
        if l.starts_with('#') || l.starts_with("run") || l.is_empty() {
            continue;
        }
        let c: Vec<&str> = l.split(',').collect();
        assert_eq!(c.len(), 5, "{l}");
        let r = runs.iter_mut().find(|r| r.name == c[0]).expect("a registered run");
        let (n, k): (usize, usize) = (c[1].parse().unwrap(), c[2].parse().unwrap());
        if k == 1 {
            r.steps.push((n, Vec::new(), Vec::new()));
        }
        let s = r.steps.last_mut().unwrap();
        assert_eq!((s.0, s.1.len() + 1), (n, k), "{l}");
        s.1.push(c[3].parse().unwrap());
        s.2.push(c[4].parse().unwrap());
    }
    runs
}

/// The fan's outcomes: `J` of F0s, `C` of F2s; beside, the median `d_19` of F2s and `P20`, the share of F0s's classed steps with
/// `d_5 <= 1e-2 d_1` (ADR 0811 section 2).
fn fan_outcomes(runs: &[Run]) -> (Vec<(String, &'static str, f64)>, f64, f64) {
    let (j, c) = jump(&runs[0], FAN_FROM);
    let (cv, k) = conv(&runs[1], FAN_FROM);
    let d19 = upper_median(classed(&runs[1], FAN_FROM).iter().map(|d| d[18]).collect());
    let s = classed(&runs[0], FAN_FROM);
    let p20 = if s.is_empty() {
        f64::NAN
    } else {
        s.iter().filter(|d| d[4] <= OUTER_RATIO * d[0]).count() as f64 / s.len() as f64
    };
    (
        vec![("F0s".into(), jname(c), j), ("F2s:C".into(), cname(k), cv)],
        d19,
        p20,
    )
}

fn check_record(got: &[(String, &'static str, f64)], want: &[Entry]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        let same = (g.2.is_nan() && w.2.is_nan()) || rel(g.2, w.2) <= REC_TOL;
        assert!(g.0 == w.0 && g.1 == w.1 && same, "{g:?} against {w:?}");
    }
}

/// F0s and F2s (ADR 0811): the heavy record of the fan controls, written to `F885_AF_OUT`.
#[test]
#[ignore = "heavy record, ADR 0811: F0s and F2s on 20 steps of H(dt) on B2 (13 627 cells, 500 outer iterations, minutes): cargo test --release -p rcfd-api --test f885_alpha_free_face_flux f885_af_fan_controls_record -- --ignored --nocapture"]
fn f885_af_fan_controls_record() {
    let mut runs = Vec::new();
    let mut centres = Vec::new();
    for (name, n) in FAN_CONTROLS {
        let (r, cc) = fan_run(name, n);
        runs.push(r);
        centres = cc;
    }
    let dir = std::env::var("F885_AF_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    write_fan(&dir.join("fan_controls.csv"), &runs);
    println!("F885-AF fan record written to {}", dir.display());
    for r in &runs {
        for (n, d, c) in r.steps.iter().filter(|s| s.0 >= FAN_FROM) {
            let at = |i: usize| {
                let x = centres[c[i]];
                format!("c{} (r {:.4}, z {:.4})", c[i], x[0].hypot(x[1]), x[2])
            };
            let k = if r.n_outer == 20 { 18 } else { r.n_outer - 1 };
            println!(
                "F885-AF {} step {n}: d_{} {:.4e} at {}, d_1 at {}",
                r.name,
                k + 1,
                d[k],
                at(k),
                at(0)
            );
        }
    }
    let (out, d19, p20) = fan_outcomes(&runs);
    println!(
        "F885-AF FAN outcomes {out:?}; F2s median d_19 {d19:.6e} m/s, Q {:.6e}; P20 {p20:.4}",
        q_rule(&runs[1], FAN_FROM)
    );
}

/// G2 (ADR 0811): the fan controls' outcomes from the committed record; held by `FAN_RECORD`.
#[test]
fn f885_af_fan_from_the_committed_record() {
    let runs = read_fan(&record_dir().join("fan_controls.csv"));
    for r in &runs {
        println!("F885-AF {}: {} steps, failed {:?}", r.name, r.steps.len(), r.failed);
    }
    let (out, d19, p20) = fan_outcomes(&runs);
    println!("F885-AF FAN outcomes {out:?}; F2s median d_19 {d19:.6e} m/s; P20 {p20:.4}");
    let want = FAN_RECORD.expect("no recorded outcome yet: the registered fan run of ADR 0811 has not been recorded");
    assert!(
        runs.iter().all(|r| r.failed.is_some() || r.steps.len() == FAN_STEPS),
        "every run complete or stopped"
    );
    check_record(&out, want);
}

/// S1 (ADR 0811): 20 steps of H(dt) on the path of the check H runs, with the switch on; `d_1` to `d_5` of every step against
/// F0s of the committed fan record.
#[test]
fn f885_af_short_run_on_the_check_h_path() {
    let a = run_h("short", H_DT, FAN_STEPS, true);
    assert!(a.failed.is_none(), "{:?}", a.failed);
    let runs = read_fan(&record_dir().join("fan_controls.csv"));
    let f0 = &runs[0];
    assert_eq!((a.rows.len(), f0.steps.len()), (FAN_STEPS, FAN_STEPS));
    let mut worst = 0.0f64;
    for (r, (n, d, _)) in a.rows.iter().zip(&f0.steps) {
        assert_eq!(r.step, *n);
        for k in 0..5 {
            worst = worst.max(rel(r.dv[k], d[k]));
        }
        println!("S1 step {n}: d1..d5 {} against {}", sci(&r.dv), sci(d));
    }
    println!("S1: largest relative difference {worst:.3e} (limit {REC_TOL:e})");
    assert!(worst <= REC_TOL, "S1: {worst}");
}

// ---------------------------------------------------------------- check H with the switch on (ADR 0811 section 4)

/// The full rows of a run as `write_full` writes them (21 columns).
fn read_full(path: &Path, dt: f64, steps: usize) -> Series {
    let rows: Vec<Row> = data_lines(path)
        .iter()
        .map(|c| {
            assert_eq!(c.len(), 21, "{c:?}");
            let f = |i: usize| num(c, i);
            Row {
                step: c[0].parse().unwrap(),
                t: f(1),
                w: [f(2), f(3), f(4), f(5), f(6), f(7)],
                wz_min: f(8),
                torque: f(9),
                dp: f(10),
                ratio: f(11),
                first: f(12),
                res_u: f(13),
                continuity: f(14),
                dv: [f(15), f(16), f(17), f(18), f(19)],
                du_step: f(20),
            }
        })
        .collect();
    Series {
        dt,
        steps,
        rows,
        failed: None,
    }
}

/// A registered run of check H with the switch on, written to the folder `F885_AF_OUT` as the full rows and the outer record.
fn heavy(name: &str, dt: f64, steps: usize) {
    let s = run_h(name, dt, steps, true);
    let dir = std::env::var("F885_AF_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    write_full(&dir.join(format!("{name}.csv")), &s);
    write_outer(&dir.join(format!("{name}_outer.csv")), &s);
    println!("F885 {name}: records written to {}", dir.display());
    let new = summarise(&s);
    print_summary(name, &new);
    print_outer(name, &s);
    assert!(s.failed.is_none(), "{:?}", s.failed);
    assert_eq!(s.rows.len(), steps);
}

/// H(dt) of ADR 0183 D5 with the switch on: 5 498 steps of 4.605e-5 s.
#[test]
#[ignore = "heavy record, ADR 0811: check H(dt) on B2 with alpha_free_face_flux (13 627 cells, 5 498 steps of 5 outer iterations, hours): cargo test --release -p rcfd-api --test f885_alpha_free_face_flux f885_af_h_dt_record -- --ignored --nocapture"]
fn f885_af_h_dt_record() {
    heavy("H_dt", H_DT, H_STEPS);
}

/// H(dt/2) of ADR 0183 D5 with the switch on: 10 996 steps of 2.3025e-5 s.
#[test]
#[ignore = "heavy record, ADR 0811: check H(dt/2) on B2 with alpha_free_face_flux (13 627 cells, 10 996 steps of 5 outer iterations, hours): cargo test --release -p rcfd-api --test f885_alpha_free_face_flux f885_af_h_dt2_record -- --ignored --nocapture"]
fn f885_af_h_dt2_record() {
    heavy("H_dt2", 0.5 * H_DT, 2 * H_STEPS);
}

/// G1 (ADR 0811): the class of ADR 0183 and the verdict of F-879 from the committed full rows of the two runs, as recorded.
#[test]
fn f885_af_class_from_the_committed_records() {
    let a = read_full(&record_dir().join("H_dt.csv"), H_DT, H_STEPS);
    let b = read_full(&record_dir().join("H_dt2.csv"), 0.5 * H_DT, 2 * H_STEPS);
    let (sa, sb) = (summarise(&a), summarise(&b));
    print_summary("H(dt)", &sa);
    print_outer("H(dt)", &a);
    print_summary("H(dt/2)", &sb);
    print_outer("H(dt/2)", &b);
    let (c, v): Outcome = verdict(&sa, &sb);
    println!(
        "F885 CLASS by ADR 0183: {c:?}; VERDICT of F-879: {v:?}; f(dt/2)/f(dt) {:.4}, A(dt/2)/A(dt) {:.4}",
        sb.site.freq / sa.site.freq,
        sb.site.amp / sa.site.amp,
    );
    assert_eq!((a.rows.len(), b.rows.len()), (H_STEPS, 2 * H_STEPS));
    assert_eq!((sa.window_steps, sb.window_steps), (2749, 5498));
    let (rc, rv, rn) = RECORD.expect("no recorded class yet: the registered runs of ADR 0811 have not been recorded");
    assert_eq!((c, v), (rc, rv));
    let same = |x: f64, y: f64| (x.is_nan() && y.is_nan()) || (x == y) || (x / y - 1.0).abs() <= REC_TOL;
    for (s, r) in [(&sa, rn[0]), (&sb, rn[1])] {
        assert!(
            same(s.share_rule, r.0) && same(s.site.amp, r.1) && same(s.site.freq, r.2),
            "share {}, {:?} against {r:?}",
            s.share_rule,
            s.site
        );
    }
}
