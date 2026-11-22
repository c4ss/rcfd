//! ADR 0766 (F-879): check H of ADR 0183 on FAN-01, the reversed-flow pocket at the hub trailing edge in time. Two time-accurate
//! k-omega SST runs of the B2 passage in the rotating frame (`pimple`, 5 outer iterations, BDF2), H(dt) and H(dt/2), from the uniform
//! start, class the pocket as steady, physically unsteady or numerical by the rule ADR 0183 registered and ADR 0766 restated before
//! the run, or the run does not decide. The gate holds the measures on known signals, the site cells, a 20-step run on the path of
//! the heavy records and the class of the committed records; the two full runs are heavy records.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
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
/// ADR 0766 D-2: the outer-iteration rule on the recorded pressure ratio, its absolute floor [m^2/s^2] and the share of steps.
const OUTER_RATIO: f64 = 1e-2;
const OUTER_FLOOR: f64 = 0.0415;
const OUTER_SHARE: f64 = 0.99;
/// Steps of the fast test on the path of the heavy records.
const SHORT: usize = 20;
/// Tolerance of a held state, relative (ADR 0762, "The tolerance of the state").
const STATE_TOL: f64 = 0.01;

/// S1 (ADR 0766): the first 20-step run of H(dt) on main 2bab1863: largest site speed [m/s], torque [N m], pressure rise [Pa].
const SHORT_RECORD: (f64, f64, f64) = (13.824327, 2.607352, 110.1842);

/// A class with its verdict.
type Outcome = (Class, Verdict);
/// What G1 holds: class, verdict, and (amplitude, frequency) of H(dt) and H(dt/2).
type Recorded = (Class, Verdict, [(f64, f64); 2]);

/// G1 (ADR 0766): the recorded class by ADR 0183's rule, the verdict of F-879, and per run (H(dt), H(dt/2)) the amplitude [m/s] and the
/// frequency [Hz] (NaN where fewer than two crossings define none) of the site signal, from the registered runs of 2026-11-22.
const RECORD: Option<Recorded> = Some((
    Class::NotEvaluable,
    Verdict::DoesNotDecide,
    [(5.364938e-3, f64::NAN), (6.493334e-3, f64::NAN)],
));

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

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

/// The case text of check H: `h_text` of `f286_transient_sst.rs` (ADR 0199 S6: the ADR 0086 bladed B2 case with the solver of ADR
/// 0183 D5), with the time step `dt`, the seam diffusivity `pair` (ADR 0766 D-1) and six reports (D-4).
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

/// `max abs(phi_f + phi_g) / max abs(phi_f)` over the faces of the rotationally periodic pairs (as ADR 0183 PC-3).
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
    /// the recorded outer ratio `max |p_4 - p_3| / max |p_1 - p^n|` (ADR 0766 D-2)
    ratio: f64,
    /// `max |p_1 - p^n|` of the step [m^2/s^2]
    first: f64,
    res_u: f64,
    continuity: f64,
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

/// The measures of one run over its window.
#[derive(Debug, Clone)]
struct Summary {
    dt: f64,
    steps: usize,
    window_steps: usize,
    failed: bool,
    /// share of the window's steps with the recorded ratio at most 1e-2
    share_ratio: f64,
    /// share that meet the rule of ADR 0766 D-2 (the ratio, or the absolute floor)
    share_rule: f64,
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

fn summarise(s: &Series) -> Summary {
    let win: Vec<&Row> = s.rows.iter().filter(|r| r.t > WINDOW_START).collect();
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
            share_ratio: 0.0,
            share_rule: 0.0,
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
    let by_ratio = win.iter().filter(|r| r.ratio <= OUTER_RATIO).count() as f64 / n;
    let by_rule = win
        .iter()
        .filter(|r| r.ratio <= OUTER_RATIO || (r.ratio.is_finite() && r.ratio * r.first <= OUTER_FLOOR))
        .count() as f64
        / n;
    Summary {
        dt: s.dt,
        steps: s.rows.len(),
        window_steps: win.len(),
        failed,
        share_ratio: by_ratio,
        share_rule: by_rule,
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

/// ADR 0183, check H, "Class (exactly one; tested in this order)", with the readings of ADR 0766 D-2 and D-5. `a` is H(dt), `b`
/// H(dt/2).
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
fn verdict(a: &Summary, b: &Summary) -> (Class, Verdict) {
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
        "F879 {label} SUMMARY: dt {:e} s, {} steps ({:.3} revolutions), window {} steps; failed {}; outer rule: {:.4} of the window's steps by the ratio alone, {:.4} by the rule, TIME-ACCURATE {}; \
max-A site cell c{}; {}; halves A1 {:.6e} A2 {:.6e} (A2/A1 {:.4}); period {:.2} steps; smallest axial relative velocity at the site {:.4} m/s; {}; {}; shaft power mean {:.3} W",
        s.dt,
        s.steps,
        s.steps as f64 * s.dt * RPM / 60.0,
        s.window_steps,
        s.failed,
        s.share_ratio,
        s.share_rule,
        s.time_accurate,
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

// ---------------------------------------------------------------- the record files

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f879-fan01-check-h")
}

const HEADER: &str =
    "step,t,w_c7,w_c57,w_c107,w_c157,w_c621,w_c1235,wz_min,torque,dp_ts,outer_ratio,outer_first,res_u,continuity";

fn write_csv(path: &Path, s: &Series) {
    let mut out = format!(
        "# ADR 0766 (F-879), check H of ADR 0183: dt = {:?} s, {} steps registered; one row per step\n{HEADER}\n",
        s.dt, s.steps
    );
    for r in &s.rows {
        write!(out, "{},{:.9e}", r.step, r.t).unwrap();
        for v in
            r.w.iter()
                .chain([&r.wz_min, &r.torque, &r.dp, &r.ratio, &r.first, &r.res_u, &r.continuity])
        {
            write!(out, ",{v:.9e}").unwrap();
        }
        out.push('\n');
    }
    std::fs::write(path, out).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn read_csv(path: &Path, dt: f64, steps: usize) -> Series {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Row> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("step"))
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            assert_eq!(c.len(), 15, "{l}");
            let f = |i: usize| c[i].trim().parse::<f64>().unwrap_or_else(|e| panic!("{l}: {e}"));
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

// ---------------------------------------------------------------- the run

/// The one path of every solving test of this file: `steps` steps of check H at the time step `dt`. With `record` the solver keeps
/// its outer record (ADR 0766 D-2: a negative tolerance is never met, so every step runs its 5 outer iterations as without it).
/// No wall-clock stop (ADR 0766 D-3); a failure of the simulation or a non-finite value ends the run.
fn run_h(label: &str, dt: f64, steps: usize, record: bool) -> Series {
    let mut sim = sim_of(&h_text(dt));
    let cells = sim.geom.mesh.n_cells();
    assert_eq!(cells, 13627);
    if record {
        sim.pimple_solver_mut()
            .expect("pimple")
            .settings
            .outer_pressure_tolerance = Some(-1.0);
    }
    let u_in = Q_DESIGN / annulus_area();
    let window = steps == H_STEPS || steps == 2 * H_STEPS;
    let (mut lo, mut hi) = (vec![f64::INFINITY; cells], vec![f64::NEG_INFINITY; cells]);
    let mut seam = 0.0f64;
    let mut rows: Vec<Row> = Vec::with_capacity(steps);
    let mut failed = None;
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
        if window && t > WINDOW_START {
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
        let (ratio, first) = match sim.pimple_solver().and_then(|s| s.outer_record.last()) {
            Some(r) if record => {
                assert_eq!((r.step, r.used, r.met), (k, 5, false), "outer record of step {k}");
                (r.ratio, r.first)
            }
            _ => (f64::NAN, f64::NAN),
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
        });
        if k % 250 == 0 || k <= 2 || k == steps {
            println!(
                "F879 {label} step {k}: t {t:.5e} s, res_u {:.3e}, continuity {:.3e}, outer ratio {ratio:.3e} (first {first:.3e}), site speeds {w:.4?}, torque {torque:.6} N m, dp_ts {dp:.4} Pa, {:.3} s per step",
                st.res_u,
                st.continuity,
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
        "F879 {label} END: {} of {steps} steps, failed {failed:?}, fields finite {fields_finite}, {:.3} s per step ({:.1} min), mass balance {:.3e}, solver time {:.6e} s",
        rows.len(),
        secs / rows.len().max(1) as f64,
        secs / 60.0,
        (m_in + m_out).abs() / m_in.abs(),
        sim.solver_time(),
    );
    if window {
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
        println!("F879 {label} REPORT: five cells of largest amplitude over the window: {top:?}; largest seam-pair flux sum (every 50 steps of the window) {seam:.3e}");
    }
    Series {
        dt,
        steps,
        rows,
        failed,
    }
}

/// `a` within the tolerance of a held state of the recorded value `b`.
fn holds(a: f64, b: f64) -> bool {
    (a - b).abs() <= STATE_TOL * b.abs()
}

// ---------------------------------------------------------------- run by the gate

/// A series of the registered length whose max-amplitude site signal is `g(t)` and whose outer record is `(ratio, first)`.
fn synthetic(dt: f64, steps: usize, g: impl Fn(f64) -> f64, outer: (f64, f64)) -> Series {
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
                ratio: outer.0,
                first: outer.1,
                res_u: 1e-6,
                continuity: 1e-10,
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

/// C3 and C4 (ADR 0766): the measures recover a sampled sine, and built pairs of series get the verdict they were built for.
#[test]
fn f879_measures_and_classes_on_known_signals() {
    // C3: 200 samples a period, 10.5 periods
    let (amp, freq, n) = (0.37, 123.0, 2100usize);
    let t: Vec<f64> = (0..n).map(|i| i as f64 / (200.0 * freq)).collect();
    let x: Vec<f64> = t
        .iter()
        .map(|&t| 3.0 + amp * (2.0 * PI * freq * t + 0.4).sin())
        .collect();
    let m = measure(&t, &x);
    println!("C3: {m:?}");
    assert!((m.amp / amp - 1.0).abs() <= 1e-3, "{m:?}");
    assert!((m.freq / freq - 1.0).abs() <= 1e-3, "{m:?}");
    assert_eq!(m.crossings, 10);
    assert!(
        (m.dft / freq - 1.0).abs() <= 0.06,
        "the DFT peak is within a bin: {m:?}"
    );

    // C4
    let good = (1e-3, 1.0);
    let sine = |a: f64, f: f64| move |t: f64| a * (2.0 * PI * f * t + 0.3).sin();
    let pair = |ga: &dyn Fn(f64) -> f64, gb: &dyn Fn(f64) -> f64, oa: (f64, f64), ob: (f64, f64)| {
        let a = summarise(&synthetic(H_DT, H_STEPS, ga, oa));
        let b = summarise(&synthetic(0.5 * H_DT, 2 * H_STEPS, gb, ob));
        assert_eq!((a.window_steps, b.window_steps), (2749, 5498));
        verdict(&a, &b)
    };
    let cases: Vec<(&str, Outcome, Outcome)> = vec![
        (
            "small oscillation in both",
            pair(&sine(0.05, 100.0), &sine(0.05, 100.0), good, good),
            (Class::Steady, Verdict::Steady),
        ),
        (
            "steady, with the outer ratio large where the step's first change is below the floor",
            pair(&sine(0.05, 100.0), &sine(0.05, 100.0), (0.5, 1e-3), (0.5, 1e-3)),
            (Class::Steady, Verdict::Steady),
        ),
        (
            "the same oscillation at both steps",
            pair(&sine(0.3, 100.0), &sine(0.3, 100.0), good, good),
            (Class::Physical, Verdict::PhysicallyUnsteady),
        ),
        (
            "the frequency doubles with the halved step",
            pair(&sine(0.3, 100.0), &sine(0.3, 200.0), good, good),
            (Class::Numerical, Verdict::Numerical),
        ),
        (
            "a period of four steps of H(dt)",
            pair(&sine(0.3, 0.26 / H_DT), &sine(0.3, 0.26 / H_DT), good, good),
            (Class::Numerical, Verdict::Numerical),
        ),
        (
            "an oscillation that decays across the window",
            pair(
                &|t| 0.6 * (-(t - WINDOW_START) / 0.05).exp() * (2.0 * PI * 100.0 * t).sin(),
                &|t| 0.6 * (-(t - WINDOW_START) / 0.05).exp() * (2.0 * PI * 100.0 * t).sin(),
                good,
                good,
            ),
            (Class::Physical, Verdict::DoesNotDecide),
        ),
        (
            "the outer rule fails in H(dt/2)",
            pair(&sine(0.3, 100.0), &sine(0.3, 100.0), good, (0.05, 100.0)),
            (Class::NotEvaluable, Verdict::DoesNotDecide),
        ),
        (
            "above the threshold in one run only",
            pair(&sine(0.3, 100.0), &sine(0.05, 100.0), good, good),
            (Class::NotEvaluable, Verdict::DoesNotDecide),
        ),
        (
            "a drift with no period",
            pair(&|t| 4.0 * (t - WINDOW_START), &|t| 4.0 * (t - WINDOW_START), good, good),
            (Class::NotEvaluable, Verdict::DoesNotDecide),
        ),
        (
            "the amplitude halves with the halved step, the frequency moves by a third",
            pair(&sine(0.6, 100.0), &sine(0.3, 133.0), good, good),
            (Class::NotEvaluable, Verdict::DoesNotDecide),
        ),
    ];
    for (name, got, want) in &cases {
        println!("C4 {name}: {got:?}");
        assert_eq!(got, want, "{name}");
    }
    // a run that stopped early does not decide
    let mut cut = synthetic(H_DT, H_STEPS, sine(0.05, 100.0), good);
    cut.rows.truncate(4000);
    let (a, b) = (
        summarise(&cut),
        summarise(&synthetic(0.5 * H_DT, 2 * H_STEPS, sine(0.05, 100.0), good)),
    );
    assert_eq!(verdict(&a, &b), (Class::NotEvaluable, Verdict::DoesNotDecide));
}

/// The centres of the six site cells as ADR 0181 recorded them (`docs/investigations/job49r-fan01-hub-mesh/D-B2.json`, `site_cells`):
/// radius and z [m], in the order of `SITE`.
const SITE_CENTRES: [(f64, f64); 6] = [
    (0.124280343942, 0.171963158688),
    (0.124280321800, 0.171588947980),
    (0.124279904561, 0.171448894521),
    (0.124278914451, 0.171622046962),
    (0.124902604538, 0.171556489357),
    (0.125648816746, 0.171069999584),
];

/// C1 (ADR 0766, amendment 1): the B2 mesh of current main has 13 627 cells and the six site cells have the centres ADR 0181
/// recorded for them (the first wake column behind the hub trailing edge, hub layers 0 to 2), within 1e-8 relative.
#[test]
fn f879_site_cells_are_at_the_hub_trailing_edge() {
    let case = Case::parse_toml(&h_text(H_DT)).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = rcfd_api::sim::build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(mesh.n_cells(), 13627);
    let cc = mesh.cell_centres();
    for (&c, &(r0, z0)) in SITE.iter().zip(&SITE_CENTRES) {
        let (r, z) = (cc[c][0].hypot(cc[c][1]), cc[c][2]);
        println!("C1 c{c}: r {r:.12} m, z {z:.12} m");
        assert!((r / r0 - 1.0).abs() <= 1e-8, "c{c}: r {r} against {r0}");
        assert!((z / z0 - 1.0).abs() <= 1e-8, "c{c}: z {z} against {z0}");
    }
}

/// S1 and C2 (ADR 0766): 20 steps of H(dt) on the path of the heavy records. The state is within 1 % of its first run, and the run
/// without the outer record gives the same bits.
#[test]
fn f879_short_run_holds_the_state() {
    let a = run_h("short", H_DT, SHORT, true);
    assert!(a.failed.is_none(), "{:?}", a.failed);
    assert_eq!(a.rows.len(), SHORT);
    let last = a.rows[SHORT - 1];
    assert!(
        (last.t / (SHORT as f64 * 4.605e-5) - 1.0).abs() <= 1e-12,
        "time {}",
        last.t
    );
    assert!(
        a.rows.iter().all(|r| r.ratio.is_finite() && r.first > 0.0),
        "the outer record"
    );
    let w_max = last.w.iter().fold(0.0f64, |m, &v| m.max(v));
    println!(
        "S1: after {SHORT} steps: largest site speed {w_max:.6} m/s, torque {:.6} N m, dp_ts {:.4} Pa, outer ratio {:.3e}",
        last.torque, last.dp, last.ratio
    );
    let b = run_h("short, no outer record", H_DT, SHORT, false);
    assert!(b.failed.is_none(), "{:?}", b.failed);
    for (ra, rb) in a.rows.iter().zip(&b.rows) {
        assert!(
            ra.w.iter().zip(&rb.w).all(|(x, y)| x.to_bits() == y.to_bits())
                && ra.torque.to_bits() == rb.torque.to_bits()
                && ra.dp.to_bits() == rb.dp.to_bits(),
            "C2: the outer record changed the solution at step {}",
            ra.step
        );
    }
    assert!(
        holds(w_max, SHORT_RECORD.0) && holds(last.torque, SHORT_RECORD.1) && holds(last.dp, SHORT_RECORD.2),
        "the state moved: site speed {w_max} against {}, torque {} against {}, dp_ts {} against {}",
        SHORT_RECORD.0,
        last.torque,
        SHORT_RECORD.1,
        last.dp,
        SHORT_RECORD.2
    );
}

/// G1 (ADR 0766): the class of ADR 0183 and the verdict of F-879 from the two committed records, as recorded.
#[test]
fn f879_class_from_the_committed_records() {
    let (a, b) = (
        read_csv(&record_dir().join("H_dt.csv"), H_DT, H_STEPS),
        read_csv(&record_dir().join("H_dt2.csv"), 0.5 * H_DT, 2 * H_STEPS),
    );
    let (sa, sb) = (summarise(&a), summarise(&b));
    print_summary("H(dt)", &sa);
    print_summary("H(dt/2)", &sb);
    let (c, v) = verdict(&sa, &sb);
    println!(
        "F879 CLASS by ADR 0183: {c:?}; VERDICT of F-879: {v:?}; f(dt/2)/f(dt) {:.4}, A(dt/2)/A(dt) {:.4}, period in steps {:.2} and {:.2}; torque A ratio {:.4}, f ratio {:.4}; pressure rise A ratio {:.4}, f ratio {:.4}",
        sb.site.freq / sa.site.freq,
        sb.site.amp / sa.site.amp,
        sa.period_steps(),
        sb.period_steps(),
        sb.torque.amp / sa.torque.amp,
        sb.torque.freq / sa.torque.freq,
        sb.dp.amp / sa.dp.amp,
        sb.dp.freq / sa.dp.freq,
    );
    assert_eq!((a.rows.len(), b.rows.len()), (5498, 10996));
    assert_eq!((sa.window_steps, sb.window_steps), (2749, 5498));
    let (rc, rv, rn) = RECORD.expect("no recorded class yet: the registered runs of ADR 0766 have not been recorded");
    assert_eq!((c, v), (rc, rv));
    let same = |x: f64, y: f64| (x.is_nan() && y.is_nan()) || (x / y - 1.0).abs() <= 1e-6;
    for (s, r) in [(&sa, rn[0]), (&sb, rn[1])] {
        assert!(
            same(s.site.amp, r.0) && same(s.site.freq, r.1),
            "{:?} against {r:?}",
            s.site
        );
    }
}

// ---------------------------------------------------------------- heavy records

/// A registered run of check H: the full run, written as a record to the folder `F879_OUT` (default: the test's temporary folder);
/// where the committed record of the run exists, the new run is compared with it (R1 of ADR 0766).
fn heavy(name: &str, dt: f64, steps: usize) {
    let s = run_h(name, dt, steps, true);
    let dir = std::env::var("F879_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join(format!("{name}.csv"));
    write_csv(&out, &s);
    println!("F879 {name}: record written to {}", out.display());
    let new = summarise(&s);
    print_summary(name, &new);
    // the decay: the amplitude in each of the six flow-through times of the run
    let sixth = s.rows.len() / 6;
    for i in 0..6 {
        let part = &s.rows[i * sixth..((i + 1) * sixth).min(s.rows.len())];
        if part.len() < 2 {
            continue;
        }
        let col = |f: &dyn Fn(&Row) -> f64| half_range(&part.iter().map(f).collect::<Vec<f64>>());
        println!(
            "F879 {name} DECAY: flow-through time {}: A of the site cell c{} {:.4e} m/s, of torque {:.4e} N m, of pressure rise {:.4e} Pa",
            i + 1,
            SITE[new.cell],
            col(&|r| r.w[new.cell]),
            col(&|r| r.torque),
            col(&|r| r.dp)
        );
    }
    assert!(s.failed.is_none(), "{:?}", s.failed);
    assert_eq!(s.rows.len(), steps);
    let committed = record_dir().join(format!("{name}.csv"));
    if !committed.exists() {
        println!("F879 {name}: FIRST RUN, no committed record to compare with");
        return;
    }
    let old = summarise(&read_csv(&committed, dt, steps));
    for (what, x, y) in [
        ("site A", new.site.amp, old.site.amp),
        ("site f", new.site.freq, old.site.freq),
        ("site mean", new.site.mean, old.site.mean),
        ("torque mean", new.torque.mean, old.torque.mean),
        ("pressure rise mean", new.dp.mean, old.dp.mean),
    ] {
        assert!((x.is_nan() && y.is_nan()) || holds(x, y), "R1 {what}: {x} against {y}");
    }
    assert_eq!(
        (
            new.time_accurate,
            new.site.amp <= A_STEADY,
            new.site.crossings >= MIN_CROSSINGS
        ),
        (
            old.time_accurate,
            old.site.amp <= A_STEADY,
            old.site.crossings >= MIN_CROSSINGS
        ),
        "R1: a condition of the class changed"
    );
}

/// H(dt) of ADR 0183 D5: 5 498 steps of 4.605e-5 s.
#[test]
#[ignore = "heavy record, ADR 0766: check H(dt) on B2 (13 627 cells, 5 498 steps of 5 outer iterations, hours): cargo test --release -p rcfd-api --test f879_fan01_check_h f879_h_dt_record -- --ignored --nocapture"]
fn f879_h_dt_record() {
    heavy("H_dt", H_DT, H_STEPS);
}

/// H(dt/2) of ADR 0183 D5: 10 996 steps of 2.3025e-5 s.
#[test]
#[ignore = "heavy record, ADR 0766: check H(dt/2) on B2 (13 627 cells, 10 996 steps of 5 outer iterations, hours): cargo test --release -p rcfd-api --test f879_fan01_check_h f879_h_dt2_record -- --ignored --nocapture"]
fn f879_h_dt2_record() {
    heavy("H_dt2", 0.5 * H_DT, 2 * H_STEPS);
}
