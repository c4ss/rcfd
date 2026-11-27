//! ADR 0821 (F-885): the remedy `PimpleSettings::outer_frozen_convection_gradient` of ADR 0820 (the gradient of linear
//! upwind's deferred correction kept from the step's first outer iteration), checked for BDF2's time order on the
//! Taylor-Green vortex (O) and for the same steady answer on the cavity (S), with the cavity as a control (KC), then check H
//! of ADR 0183 with the opt-ins `alpha_free_face_flux` (ADR 0811), `outer_frozen_limiter` (ADR 0819) and the remedy, read by
//! the outer-iteration rule of ADR 0183 as written (ADR 0767 E-1). The set-up, the measures, the class and the verdict of
//! check H are copied unchanged from `f885_alpha_free_face_flux.rs` (ADR 0811), but for the opt-ins. The two check H runs
//! are heavy records; the gate reads their committed files.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::gradient::Limiter;
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

/// S1 and G1: relative tolerance against the 10 printed digits of a record.
const REC_TOL: f64 = 1e-6;
/// SR: the steps of the short run.
const SHORT: usize = 20;

/// A class with its verdict.
type Outcome = (Class, Verdict);
/// What G1 holds: class, verdict, and per run (H(dt), H(dt/2)) the share of the window's steps that meet the rule of E-1, the
/// amplitude [m/s] and the frequency [Hz] of the site signal.
type Recorded = (Class, Verdict, [(f64, f64, f64); 2]);
/// G1: the recorded values of the two registered runs (written after the runs).
const RECORD: Option<Recorded> = None;

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

/// The full rows, in ADR 0766's form with the outer record after them (committed: ADR 0821 section 5, ADR 0767 E-3).
fn write_full(path: &Path, s: &Series) {
    let mut out = format!(
        "# ADR 0821 (F-885), check H of ADR 0183 with alpha_free_face_flux, outer_frozen_limiter and outer_frozen_convection_gradient: dt = {:?} s, {} steps registered; one row per step\n{HEADER},d1,d2,d3,d4,d5,du_step\n",
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
/// has the opt-ins of ADR 0821 section 5 on, and
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
        // ADR 0821 section 5: the opt-ins by which check H departs from ADR 0767
        s.alpha_free_face_flux = true;
        s.outer_frozen_limiter = true;
        s.outer_frozen_convection_gradient = true;
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

// ---------------------------------------------------------------- the order check O (ADR 0821 section 2)

/// The Taylor-Green vortex of ADR 0223's harness: viscosity, cells a side, the steps and the reference step to `t = 1`.
const TG_NU: f64 = 0.1;
const TG_N: usize = 16;
const TG_DTS: [f64; 3] = [0.1, 0.05, 0.025];
const TG_DT_REF: f64 = 1.0 / 640.0;
/// O1, O2: `docs/process/tolerances.md`, "Observed order of accuracy" (BDF2's order 2 less 0.1).
const ORDER_MIN: f64 = 1.9;
/// O3: the setting must act on the case.
const O3_FLOOR: f64 = 1e-9;

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let d = (-2.0 * TG_NU * t).exp();
    [x[0].sin() * x[1].cos() * d, -x[0].cos() * x[1].sin() * d, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    0.25 * ((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * TG_NU * t).exp()
}

/// Tight linear solvers (ADR 0821 sections 2 and 3).
fn tight(s: &mut PimpleSettings, tol_abs: f64) {
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = tol_abs;
        c.tol_rel = 1e-12;
        c.max_iter = 5000;
    }
}

/// One arm of O at one step: the velocity at `t = 1`.
fn tg_run(on: bool, dt: f64) -> Vec<Vec3> {
    tg_run_with(on, dt, &|_| {})
}

/// As `tg_run`, with `set` applied to the settings last (the diagnosis of ADR 0821 amendment 1).
fn tg_run_with(on: bool, dt: f64, set: &dyn Fn(&mut PimpleSettings)) -> Vec<Vec3> {
    let mut s = PimpleSettings::piso(TG_NU);
    s.n_outer = 10;
    s.alpha_u = 1.0;
    s.alpha_p = 1.0;
    s.time_scheme = TimeScheme::Bdf2;
    s.convection = ConvScheme::LinearUpwind;
    s.linear_upwind_limiter = None;
    tight(&mut s, 1e-14);
    s.outer_frozen_convection_gradient = on;
    set(&mut s);
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(TG_N, TG_N, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| tg_u(c, 0.0));
    let p = ScalarField::from_fn("p", &geom, |c| tg_p(c, 0.0));
    let mut sol = PimpleSolver::new(geom, u, p, s);
    for _ in 0..(1.0 / dt).round() as usize {
        sol.step(dt);
        assert!(sol.diverged.is_none(), "the Taylor-Green run stopped at dt {dt}");
    }
    assert!((sol.time - 1.0).abs() < 1e-9);
    sol.u.values.clone()
}

/// The relative L2 norm over the cells of `a - b` (the two in-plane components).
fn rel_l2(a: &[Vec3], b: &[Vec3]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        num += (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
        den += y[0] * y[0] + y[1] * y[1];
    }
    (num / den).sqrt()
}

/// O (ADR 0821): the observed order of BDF2 with the setting off (O2, the control) and on (O1), and the non-vacuity (O3).
/// KNOWN MISS (ADR 0821 amendment 1, limits intact): O2 missed, so O does not decide.
#[test]
#[ignore = "known miss, ADR 0821 amendment 1: p_f(OFF) 1.0151 and p_f(ON) 1.0609 against 1.9 (O2 and O1 missed, O3 4.15e-4 met): O does not decide"]
fn f885_fc_time_order_known_miss() {
    let mut p_f = [f64::NAN; 2];
    let mut at_mid: Vec<Vec<Vec3>> = Vec::new();
    let mut errs: Vec<Vec<f64>> = Vec::new();
    for (i, on) in [false, true].into_iter().enumerate() {
        let r = tg_run(on, TG_DT_REF);
        let us: Vec<Vec<Vec3>> = TG_DTS.iter().map(|&dt| tg_run(on, dt)).collect();
        let e: Vec<f64> = us.iter().map(|u| rel_l2(u, &r)).collect();
        let (pc, pf) = ((e[0] / e[1]).log2(), (e[1] / e[2]).log2());
        println!(
            "F885-FC O {}: e(0.1, 0.05, 0.025) {}, p_c {pc:.4}, p_f {pf:.4} (limit {ORDER_MIN})",
            if on { "ON" } else { "OFF" },
            sci(&e)
        );
        p_f[i] = pf;
        at_mid.push(us[1].clone());
        errs.push(e);
    }
    let o3 = rel_l2(&at_mid[1], &at_mid[0]);
    let ratio: Vec<f64> = (0..3).map(|j| errs[1][j] / errs[0][j]).collect();
    println!(
        "F885-FC O: O3 ON against OFF at dt 0.05 {o3:.4e} (floor {O3_FLOOR:e}); e_ON / e_OFF {}; abs(p_f(ON) - p_f(OFF)) {:.4}",
        sci(&ratio),
        (p_f[1] - p_f[0]).abs()
    );
    assert!(o3 > O3_FLOOR, "O does not decide: O3 {o3}");
    assert!(p_f[0] >= ORDER_MIN, "O does not decide: O2 p_f(OFF) {}", p_f[0]);
    assert!(p_f[1] >= ORDER_MIN, "O1 missed: p_f(ON) {}", p_f[1]);
}

/// One change of the settings of a diagnosis arm.
type SetFn<'a> = &'a dyn Fn(&mut PimpleSettings);

/// The diagnosis of O2 (ADR 0821 amendment 1, a record without limits): the OFF arm of O with one change each.
#[test]
#[ignore = "diagnosis record, ADR 0821 amendment 1 (seconds, prints only): cargo test --release -p rcfd-api --test f885_frozen_convection_check_h f885_fc_order_diagnosis_record -- --ignored --nocapture"]
fn f885_fc_order_diagnosis_record() {
    let cases: [(&str, SetFn); 4] = [
        ("D-a central", &|s| s.convection = ConvScheme::Central),
        ("D-b linear upwind, n_outer 20", &|s| s.n_outer = 20),
        ("D-c linear upwind, periodic_partner_extrapolated", &|s| {
            s.periodic_partner_extrapolated = true
        }),
        ("D-d linear upwind, implicit Euler", &|s| {
            s.time_scheme = TimeScheme::ImplicitEuler
        }),
    ];
    for (name, set) in cases {
        let r = tg_run_with(false, TG_DT_REF, set);
        let e: Vec<f64> = TG_DTS
            .iter()
            .map(|&dt| rel_l2(&tg_run_with(false, dt, set), &r))
            .collect();
        println!(
            "F885-FC {name}: e(0.1, 0.05, 0.025) {}, p_c {:.4}, p_f {:.4}",
            sci(&e),
            (e[0] / e[1]).log2(),
            (e[1] / e[2]).log2()
        );
    }
}

// ---------------------------------------------------------------- the steady check S and the cavity control KC (sections 3, 4)

const S_DT: f64 = 0.2;
const S_STEPS: usize = 5000;
/// S1: the step change of a steady state [m/s].
const S_CRIT: f64 = 1e-12;
/// S2: "Same discrete problem by two routes".
const S_TOL: f64 = 1e-8;
/// S3: the setting must act in the transient.
const S3_FLOOR: f64 = 1e-6;
const KC_DT: f64 = 0.05;
const KC_STEPS: usize = 20;

/// The 8 x 8 lid-driven cavity of ADR 0767 from rest.
fn cavity(s: PimpleSettings) -> PimpleSolver {
    let mesh = box_2d(8, 8, 1.0, 1.0, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["xmin", "xmax", "ymin"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, s)
}

fn steady_settings(on: bool, limited: bool) -> PimpleSettings {
    let mut s = PimpleSettings::pimple(0.01, 5);
    s.time_scheme = TimeScheme::Bdf2;
    s.convection = ConvScheme::LinearUpwind;
    s.alpha_free_face_flux = true;
    if limited {
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
        s.outer_frozen_limiter = true;
    }
    tight(&mut s, 1e-15);
    s.outer_frozen_convection_gradient = on;
    s
}

/// `max_c abs(a - b) / max_c abs(b)`, Euclidean per cell.
fn vmeasure(a: &[Vec3], b: &[Vec3]) -> f64 {
    let den = b
        .iter()
        .map(|y| (y[0] * y[0] + y[1] * y[1] + y[2] * y[2]).sqrt())
        .fold(0.0, f64::max);
    change(b, a) / den
}

fn smeasure(a: &[f64], b: &[f64]) -> f64 {
    let num = a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    num / b.iter().map(|v| v.abs()).fold(0.0, f64::max)
}

/// A steady march: the solver, the step at which the criterion was met (if it was), the last step change, the velocity
/// after 5 steps.
fn march(s: PimpleSettings) -> (PimpleSolver, Option<usize>, f64, Vec<Vec3>) {
    let mut sol = cavity(s);
    let (mut last, mut five) = (f64::NAN, Vec::new());
    for k in 1..=S_STEPS {
        let prev = sol.u.values.clone();
        sol.step(S_DT);
        assert!(sol.diverged.is_none(), "the steady march stopped at step {k}");
        last = change(&prev, &sol.u.values);
        if k == 5 {
            five = sol.u.values.clone();
        }
        if last <= S_CRIT && k > 5 {
            return (sol, Some(k), last, five);
        }
    }
    (sol, None, last, five)
}

/// S (ADR 0821): the steady answer with the setting on against off; SL beside.
#[test]
fn f885_fc_steady() {
    let mut out = Vec::new();
    for limited in [false, true] {
        let (off, n_off, l_off, f_off) = march(steady_settings(false, limited));
        let (on, n_on, l_on, f_on) = march(steady_settings(true, limited));
        let (du, dphi, d5) = (
            vmeasure(&on.u.values, &off.u.values),
            smeasure(&on.phi, &off.phi),
            vmeasure(&f_on, &f_off),
        );
        println!(
            "F885-FC {}: steps to the criterion OFF {n_off:?} (last change {l_off:.3e}), ON {n_on:?} (last change {l_on:.3e}); velocity {du:.4e}, flux {dphi:.4e} (limit {S_TOL:e}); after 5 steps {d5:.4e} (floor {S3_FLOOR:e})",
            if limited { "SL (beside)" } else { "S" }
        );
        out.push((n_off.is_some() && n_on.is_some(), du, dphi, d5));
    }
    let (reached, du, dphi, d5) = out[0];
    assert!(reached, "S does not decide: S1 not met");
    assert!(d5 > S3_FLOOR, "S does not decide: S3 {d5}");
    assert!(du <= S_TOL && dphi <= S_TOL, "S2 missed: {du} {dphi}");
}

/// KC (ADR 0821): the cavity with check H's settings, the setting on and off; classed, not a limit.
#[test]
fn f885_fc_cavity_control() {
    for on in [false, true] {
        let mut s = PimpleSettings::pimple(0.01, 5);
        s.time_scheme = TimeScheme::Bdf2;
        s.convection = ConvScheme::LinearUpwind;
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
        s.outer_frozen_limiter = true;
        s.alpha_free_face_flux = true;
        s.outer_velocity_record = true;
        s.outer_frozen_convection_gradient = on;
        let mut sol = cavity(s);
        for _ in 0..KC_STEPS {
            sol.step(KC_DT);
            assert!(sol.diverged.is_none(), "the cavity stopped");
        }
        assert_eq!(sol.outer_velocity.len(), KC_STEPS);
        let q: Vec<f64> = sol.outer_velocity.iter().map(|r| r.change[4] / r.change[0]).collect();
        let share = q.iter().filter(|&&x| x <= OUTER_RATIO).count() as f64 / KC_STEPS as f64;
        println!(
            "F885-FC KC {}: share of {KC_STEPS} steps meeting E-1 {share:.3}; d5/d1 (min, median, max) {}",
            if on { "ON" } else { "OFF" },
            {
                let (a, b, c) = spread(&q);
                sci(&[a, b, c])
            }
        );
    }
}

// ---------------------------------------------------------------- check H's rule on built records (ADR 0767 C5, copied)

/// A built series of check H (ADR 0767, copied).
fn synthetic(
    dt: f64,
    steps: usize,
    g: impl Fn(f64) -> f64,
    dv: impl Fn(usize) -> [f64; 5],
    pressure: (f64, f64),
) -> Series {
    let rows = (1..=steps)
        .map(|k| {
            let t = k as f64 * dt;
            Row {
                step: k,
                t,
                w: [5.0 + g(t), 5.0, 5.0, 5.0, 5.0, 5.0],
                wz_min: -1.0,
                torque: 2.0,
                dp: 60.0,
                ratio: pressure.0,
                first: pressure.1,
                res_u: 1e-6,
                continuity: 1e-10,
                dv: dv(k),
                du_step: 1e-3,
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

/// A velocity record with `d_5 / d_1 = q`.
fn outer(q: f64) -> [f64; 5] {
    [1.0, 0.3, 0.1, 0.03, q]
}

fn sine(a: f64, f: f64) -> impl Fn(f64) -> f64 {
    move |t| a * (2.0 * PI * f * t).sin()
}

/// C5 (ADR 0821, ADR 0767 C5): the rule of E-1 on built velocity records of H(dt), H(dt/2) good, the site signal steady.
#[test]
fn f885_fc_rules_on_built_records() {
    let good = 1e-3;
    let pgood = (1e-3, 1.0);
    let bad_first = |n: usize, d: [f64; 5]| move |k: usize| if (2750..2750 + n).contains(&k) { d } else { outer(good) };
    let steady = |da: &dyn Fn(usize) -> [f64; 5]| {
        let a = summarise(&synthetic(H_DT, H_STEPS, sine(0.05, 100.0), da, pgood));
        let b = summarise(&synthetic(
            0.5 * H_DT,
            2 * H_STEPS,
            sine(0.05, 100.0),
            |_| outer(good),
            pgood,
        ));
        (a.share_rule, verdict(&a, &b))
    };
    let meets = (Class::Steady, Verdict::Steady);
    let fails = (Class::NotEvaluable, Verdict::DoesNotDecide);
    let c5: Vec<(&str, (f64, Outcome), Outcome)> = vec![
        ("d5/d1 = 1e-3 everywhere", steady(&|_| outer(1e-3)), meets),
        ("d5/d1 = 0.05 everywhere", steady(&|_| outer(0.05)), fails),
        (
            "d5/d1 = 0.05 in 2 % of the window",
            steady(&bad_first(55, outer(0.05))),
            fails,
        ),
        (
            "d5/d1 = 0.05 in 0.5 % of the window",
            steady(&bad_first(14, outer(0.05))),
            meets,
        ),
        (
            "d4/d1 = 0.5 with d5/d1 = 1e-3: the 5th is read",
            steady(&|_| [1.0, 0.8, 0.6, 0.5, 1e-3]),
            meets,
        ),
        ("d1 = d5 = 0", steady(&|_| [0.0; 5]), meets),
        (
            "a NaN d5 in 2 % of the window",
            steady(&bad_first(55, [1.0, 0.3, 0.1, 0.03, f64::NAN])),
            fails,
        ),
    ];
    for (name, (share, got), want) in &c5 {
        println!("C5 {name}: share {share:.5}, {got:?}");
        assert_eq!(got, want, "{name}");
    }
}

// ---------------------------------------------------------------- check H: the records, the heavy runs, the class (section 5)

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-frozen-convection-check-h")
}

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

/// A registered run of check H with the opt-ins, written to the folder `F885_FC_OUT` as the full rows.
fn heavy(name: &str, dt: f64, steps: usize) {
    let s = run_h(name, dt, steps, true);
    let dir = std::env::var("F885_FC_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    write_full(&dir.join(format!("{name}.csv")), &s);
    println!("F885 {name}: records written to {}", dir.display());
    let new = summarise(&s);
    print_summary(name, &new);
    print_outer(name, &s);
    assert!(s.failed.is_none(), "{:?}", s.failed);
    assert_eq!(s.rows.len(), steps);
}

/// H(dt) of ADR 0183 D5 with the opt-ins: 5 498 steps of 4.605e-5 s.
#[test]
#[ignore = "heavy record, ADR 0821: check H(dt) on B2 with the remedy (13 627 cells, 5 498 steps of 5 outer iterations, hours): cargo test --release -p rcfd-api --test f885_frozen_convection_check_h f885_fc_h_dt_record -- --ignored --nocapture"]
fn f885_fc_h_dt_record() {
    heavy("H_dt", H_DT, H_STEPS);
}

/// H(dt/2) of ADR 0183 D5 with the opt-ins: 10 996 steps of 2.3025e-5 s.
#[test]
#[ignore = "heavy record, ADR 0821: check H(dt/2) on B2 with the remedy (13 627 cells, 10 996 steps of 5 outer iterations, hours): cargo test --release -p rcfd-api --test f885_frozen_convection_check_h f885_fc_h_dt2_record -- --ignored --nocapture"]
fn f885_fc_h_dt2_record() {
    heavy("H_dt2", 0.5 * H_DT, 2 * H_STEPS);
}

/// SR (ADR 0821): 20 steps of the check H path with the opt-ins against the first 20 rows of the committed `H_dt.csv`.
#[test]
#[ignore = "not run: check H was not run by ADR 0821's rule (amendment 1)"]
fn f885_fc_short_run_on_the_check_h_path() {
    let a = run_h("short", H_DT, SHORT, true);
    assert!(a.failed.is_none(), "{:?}", a.failed);
    assert_eq!(a.rows.len(), SHORT);
    assert!(a
        .rows
        .iter()
        .all(|r| r.dv.iter().all(|d| d.is_finite()) && r.dv[0] > 0.0));
    let want = read_full(&record_dir().join("H_dt.csv"), H_DT, H_STEPS);
    let mut worst = 0.0f64;
    for (x, y) in a.rows.iter().zip(&want.rows[..SHORT]) {
        assert_eq!(x.step, y.step);
        let fx =
            x.w.iter()
                .chain([&x.wz_min, &x.torque, &x.dp, &x.ratio, &x.first, &x.res_u, &x.continuity])
                .chain(x.dv.iter())
                .chain([&x.du_step]);
        let fy =
            y.w.iter()
                .chain([&y.wz_min, &y.torque, &y.dp, &y.ratio, &y.first, &y.res_u, &y.continuity])
                .chain(y.dv.iter())
                .chain([&y.du_step]);
        for (p, q) in fx.zip(fy) {
            worst = worst.max(rel(*p, *q));
        }
    }
    println!("F885-FC SR: 20 steps against the committed H_dt.csv, largest relative difference {worst:.3e} (limit {REC_TOL:e})");
    assert!(worst <= REC_TOL, "SR: {worst}");
}

/// G1 (ADR 0821): the class of ADR 0183 and the verdict of F-879 from the committed full rows of the two runs, as recorded.
#[test]
#[ignore = "not run: check H was not run by ADR 0821's rule (amendment 1)"]
fn f885_fc_class_from_the_committed_records() {
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
    let (rc, rv, rn) = RECORD.expect("no recorded class yet: the registered runs of ADR 0821 have not been recorded");
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
