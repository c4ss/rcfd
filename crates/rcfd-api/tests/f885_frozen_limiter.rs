//! ADR 0819 (F-885): the opt-in `outer_frozen_limiter`, which keeps each component's Barth-Jespersen factor of the step's
//! first outer iteration for the step's later ones, and mode B of ADR 0816 with ADR 0811's term off and on. On the fan
//! (20 steps of H(dt), 20 outer iterations): F (A0 of ADR 0816 with the opt-in: the factor kept at c11837, the cycle gone,
//! `C` classed by ADR 0815's rule), B1r (B1 of ADR 0816 reproduced), M1 to M3 (the limiter off, the term off, `alpha_u` 0.9,
//! 0.7, 0.5) against B1 to B3 of ADR 0816 by HC1 to HC3. On the cavity: two routes, read-only without the limiter, the
//! control, and a steady case that gives the same answer with the opt-in on. The case text and the measures of mode A are
//! copied unchanged from `f885_two_outer_modes.rs` (ADR 0816). The fan runs are a heavy record; the gate reads its
//! committed files.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::gradient::{least_squares, limiter, Limiter};
use rcfd_fv::pimple::{OuterProbe, PimpleSettings, PimpleSolver, ProbeIteration};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RPM: f64 = 1486.0;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075), as in ADR 0762
const Q_DESIGN: f64 = 1.4032;
/// ADR 0183 D5: dt = T_c / 100.
const H_DT: f64 = 4.605e-5;
/// ADR 0810 section 5: `C <= 1e-2` CONVERGES.
const C_CONVERGES: f64 = 1e-2;
/// The cavity: steps and time step (ADR 0767 section 1).
const CAV_STEPS: usize = 3;
const CAV_DT: f64 = 0.05;
/// The fan runs: steps, the first classed step, the outer iterations (ADR 0816 section 4).
const FAN_STEPS: usize = 20;
const FAN_FROM: usize = 11;
const N_OUT: usize = 20;
/// The two probe cells (ADR 0815 addendum 1).
const C_A: usize = 11837;
const C_B: usize = 6245;
/// R0, R1, G2: relative tolerance against the printed digits of a record.
const REC_TOL: f64 = 1e-6;
/// T1, FA1: tolerance of two routes separated only by rounding.
const ROUTE_TOL: f64 = 1e-12;
/// ADR 0816 section 2, mode A: the floor of a switching factor, the return ratio, the local factor, the sign changes, the
/// share of the classed steps.
const PSI_FLOOR: f64 = 1e-9;
const RETURN_RATIO: f64 = 0.1;
const LOCAL_FACTOR: f64 = 0.99;
const SIGN_CHANGES: usize = 8;
const STEP_SHARE: f64 = 0.8;
/// ADR 0819 section 2: the cycle is gone if `q >= 0.1` (the return ratio of ADR 0816 applied to the iterate).
const Q_GONE: f64 = 0.1;
/// ADR 0819 section 3 (ADR 0816 section 2): the band of a predicted factor.
const BAND: f64 = 0.015;
/// Mode B: `alpha_u` of the grid, and `rho` with the term on (B1, B2, B3 of ADR 0816, its `FAN_RECORD`).
const ALPHAS: [f64; 3] = [0.9, 0.7, 0.5];
const RHO_ON: [f64; 3] = [0.853789228181186, 0.8729920722691961, 0.8920440841467279];
/// S1 (ADR 0819 section 4): the steady case, its preconditions and the limit.
const S_NU: f64 = 0.1;
const S_DT: f64 = 0.1;
const S_STEPS: usize = 400;
const S_STEADY: f64 = 1e-12;
const S_SAME: f64 = 1e-8;

/// The recorded outcomes of the registered fan run (written after the run): F at c11837 (HA1, HA2, HA3 held; `q`; the
/// largest change of the kept factor, FA1), `C` of F, `rho` of B1r, M1, M2, M3, and result C.
type FanRecord = ([bool; 3], f64, f64, f64, [f64; 4], &'static str);
const FAN_RECORD: Option<FanRecord> = Some((
    [false, false, false],
    2.2505325177996163,
    0.0,
    0.014300805347754355,
    [
        0.853789228181186,
        0.8541098171565654,
        0.8713420211615733,
        0.8926640194754057,
    ],
    "named HC2",
));

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// Relative difference that is 0 when both are NaN (a value not formed on both routes) and infinite when one is.
fn rel_nan(a: f64, b: f64) -> f64 {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => 0.0,
        (false, false) => rel(a, b),
        _ => f64::INFINITY,
    }
}

fn norm(a: Vec3, b: Vec3) -> f64 {
    let (x, y, z) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (x * x + y * y + z * z).sqrt()
}

// ---------------------------------------------------------------- the case of check H (copied from ADR 0815)

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

/// The case text of check H, as `h_text` of `f885_outer_stall.rs` (ADR 0815).
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

// ---------------------------------------------------------------- the measures (ADR 0816 section 2)

/// The upper median `x[len / 2]` of the sorted values; NaN if any is NaN or there are none.
fn upper_median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() || v.iter().any(|x| x.is_nan()) {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// A run's record of the changes: per step `(step, d_k, cells)`.
#[derive(Debug, Clone)]
struct Run {
    name: String,
    steps: Vec<(usize, Vec<f64>, Vec<usize>)>,
    planned: usize,
    failed: Option<String>,
}

impl Run {
    fn complete(&self) -> bool {
        self.failed.is_none() && self.steps.len() == self.planned
    }
}

/// The result of a mode (section 2).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Named(&'static str),
    AllRuledOut,
    NamesNone(String),
    Undecided,
}

fn oname(o: &Outcome) -> String {
    match o {
        Outcome::Named(h) => format!("named {h}"),
        Outcome::AllRuledOut => "all ruled out".into(),
        Outcome::NamesNone(why) => format!("names none ({why})"),
        Outcome::Undecided => "undecided".into(),
    }
}

/// The value of iteration `k` (1-based).
fn it(p: &OuterProbe, k: usize) -> &ProbeIteration {
    &p.iterations[k - 1]
}

/// HA1 in one step: the entry (cell or neighbour, component) of the largest median change of its factor over `k = 3..19`
/// switches between two values.
fn ha1_step(p: &OuterProbe) -> bool {
    let n_entries = it(p, 1).psi.len();
    let mut best: Option<(f64, usize, usize)> = None;
    for e in 0..n_entries {
        for j in 0..3 {
            let v = |k: usize| it(p, k).psi[e][j];
            if (1..=19).any(|k| v(k).is_nan()) {
                continue;
            }
            let med = upper_median((3..=19).map(|k| (v(k) - v(k - 1)).abs()).collect());
            if best.is_none_or(|b| med > b.0) {
                best = Some((med, e, j));
            }
        }
    }
    let Some((med, e, j)) = best else {
        return false;
    };
    let v = |k: usize| it(p, k).psi[e][j];
    let ret = upper_median((4..=19).map(|k| (v(k) - v(k - 2)).abs()).collect());
    med >= PSI_FLOOR && ret <= RETURN_RATIO * med
}

/// HA2 in one step: the dominance lost for some active component in every relaxed iteration `k = 2..19`, and the cell's own
/// factor `1 - alpha min_j median_k a_c / s` at least 0.99.
fn ha2_step(p: &OuterProbe, alpha: f64) -> bool {
    let active: Vec<usize> = (0..3).filter(|&j| !it(p, 1).diag[j].is_nan()).collect();
    let lost = active
        .iter()
        .any(|&j| (2..=19).all(|k| it(p, k).off_sum[j] > it(p, k).diag[j]));
    let rmin = active
        .iter()
        .map(|&j| upper_median((2..=19).map(|k| it(p, k).diag[j] / it(p, k).off_sum[j]).collect()))
        .fold(f64::INFINITY, f64::min);
    lost && 1.0 - alpha * rmin >= LOCAL_FACTOR
}

/// HA3 in one step: some face of the cell whose outward flux changes sign in at least 8 of the pairs `(k - 1, k)`,
/// `k = 4..19`.
fn ha3_step(p: &OuterProbe) -> bool {
    (0..it(p, 1).flux.len()).any(|f| {
        (4..=19)
            .filter(|&k| it(p, k).flux[f] * it(p, k - 1).flux[f] < 0.0)
            .count()
            >= SIGN_CHANGES
    })
}

/// The shape of the mode in one step: median over `k = 3..19` of `|u_k - u_(k-2)| / |u_k - u_(k-1)|`.
fn shape_step(p: &OuterProbe) -> f64 {
    upper_median(
        (3..=19)
            .map(|k| norm(it(p, k).u, it(p, k - 2).u) / norm(it(p, k).u, it(p, k - 1).u))
            .collect(),
    )
}

/// The result of mode A from the classed steps' probe records at one cell: the three hypotheses held, `q`, the result.
/// `complete` is whether the run made all its steps.
fn mode_a(steps: &[&OuterProbe], alpha: f64, complete: bool) -> ([bool; 3], f64, Outcome) {
    if !complete || steps.is_empty() || steps.iter().any(|p| p.iterations.len() < 20) {
        return ([false; 3], f64::NAN, Outcome::Undecided);
    }
    let n = steps.len() as f64;
    let share = |f: &dyn Fn(&OuterProbe) -> bool| steps.iter().filter(|p| f(p)).count() as f64 >= STEP_SHARE * n;
    let held = [share(&ha1_step), share(&|p| ha2_step(p, alpha)), share(&ha3_step)];
    let q = upper_median(steps.iter().map(|p| shape_step(p)).collect());
    let names = ["HA1", "HA2", "HA3"];
    let s: Vec<&'static str> = (0..3).filter(|&i| held[i]).map(|i| names[i]).collect();
    let out = match s.len() {
        0 => Outcome::AllRuledOut,
        1 => Outcome::Named(s[0]),
        _ => Outcome::NamesNone(format!("{s:?} hold")),
    };
    (held, q, out)
}

/// The factor of a run: median over the classed steps of `(d_19 / d_16)^(1/3)`; NaN if the run is not complete.
fn rho(run: &Run) -> f64 {
    if !run.complete() {
        return f64::NAN;
    }
    upper_median(
        run.steps
            .iter()
            .filter(|s| s.0 >= FAN_FROM)
            .map(|s| (s.1[18] / s.1[15]).powf(1.0 / 3.0))
            .collect(),
    )
}

/// The dominance ratio of a run at a probe cell: median over the classed steps and `k = 16..19` of `min_j a_c / s`.
fn dominance(steps: &[&OuterProbe]) -> f64 {
    let mut v = Vec::new();
    for p in steps {
        if p.iterations.len() < 20 {
            return f64::NAN;
        }
        for k in 16..=19 {
            let i = it(p, k);
            v.push(
                (0..3)
                    .filter(|&j| !i.diag[j].is_nan())
                    .map(|j| i.diag[j] / i.off_sum[j])
                    .fold(f64::INFINITY, f64::min),
            );
        }
    }
    upper_median(v)
}

/// The runs `names` from a record of the form `run,step,k,d,cell` (ADR 0815's and this one's).
fn read_d(path: &Path, names: &[&str]) -> Vec<Run> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut runs: Vec<Run> = names
        .iter()
        .map(|n| Run {
            name: (*n).into(),
            steps: Vec::new(),
            planned: FAN_STEPS,
            failed: None,
        })
        .collect();
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix("# ") {
            if let (Some(i), Some(sp)) = (rest.find(" failed Some"), rest.find(' ')) {
                if let Some(r) = runs.iter_mut().find(|r| r.name == rest[..sp]) {
                    r.failed = Some(rest[i + 8..].to_string());
                }
            }
            continue;
        }
        if l.starts_with("run") || l.is_empty() {
            continue;
        }
        let c: Vec<&str> = l.split(',').collect();
        assert_eq!(c.len(), 5, "{l}");
        let Some(r) = runs.iter_mut().find(|r| r.name == c[0]) else {
            continue;
        };
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

/// The probe records of `probe.csv`, per run.
fn read_probe(path: &Path) -> Vec<(String, OuterProbe)> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out: Vec<(String, OuterProbe)> = Vec::new();
    let num = |s: &str| s.parse::<f64>().unwrap();
    for l in text.lines() {
        if l.starts_with('#') || l.starts_with("run") || l.is_empty() {
            continue;
        }
        let c: Vec<&str> = l.split(',').collect();
        assert_eq!(c.len(), 15, "{l}");
        let (step, cell, k): (usize, usize, usize) =
            (c[1].parse().unwrap(), c[2].parse().unwrap(), c[3].parse().unwrap());
        let psi_flat: Vec<f64> = c[13].split(';').map(num).collect();
        let it = ProbeIteration {
            diag: [num(c[4]), num(c[5]), num(c[6])],
            off_sum: [num(c[7]), num(c[8]), num(c[9])],
            u: [num(c[10]), num(c[11]), num(c[12])],
            psi: psi_flat.chunks(3).map(|e| [e[0], e[1], e[2]]).collect(),
            flux: c[14].split(';').map(num).collect(),
        };
        if k == 1 {
            out.push((
                c[0].to_string(),
                OuterProbe {
                    step,
                    cell,
                    faces: Vec::new(),
                    neighbours: Vec::new(),
                    iterations: Vec::new(),
                },
            ));
        }
        let last = out.last_mut().unwrap();
        assert_eq!(
            (last.0.as_str(), last.1.step, last.1.cell, last.1.iterations.len() + 1),
            (c[0], step, cell, k),
            "{l}"
        );
        last.1.iterations.push(it);
    }
    out
}

// ---------------------------------------------------------------- the measures of this registration (sections 2 and 3)

/// `C` of a run: median over the classed steps of `d_19 / d_2`; NaN if the run is not complete (ADR 0815, the measures).
fn c_of(run: &Run) -> f64 {
    if !run.complete() {
        return f64::NAN;
    }
    upper_median(
        run.steps
            .iter()
            .filter(|s| s.0 >= FAN_FROM)
            .map(|s| s.1[18] / s.1[1])
            .collect(),
    )
}

/// The class of `C` (ADR 0815, the measures).
fn class(c: f64) -> &'static str {
    if c.is_nan() {
        "UNDECIDED"
    } else if c <= C_CONVERGES {
        "CONVERGES"
    } else {
        "STALLS"
    }
}

/// FA2: the cycle gone, from the result of the rule of mode A and `q`.
fn cycle_gone(out: &Outcome, q: f64) -> bool {
    *out == Outcome::AllRuledOut && q >= Q_GONE
}

/// FA1: the largest absolute change of the factor used at a probe cell and its neighbours, every component formed, in
/// iterations 2 to 20 against iteration 1; NaN if a step has no factor (no limiter) or fewer than 20 iterations.
fn kept_change(steps: &[&OuterProbe]) -> f64 {
    let mut worst = 0.0f64;
    let mut formed = 0usize;
    for p in steps {
        if p.iterations.len() < N_OUT {
            return f64::NAN;
        }
        let first = &p.iterations[0].psi;
        for i in &p.iterations[1..] {
            for (e, f) in i.psi.iter().zip(first) {
                for j in 0..3 {
                    if f[j].is_nan() && e[j].is_nan() {
                        continue;
                    }
                    formed += 1;
                    worst = worst.max((e[j] - f[j]).abs());
                }
            }
        }
    }
    if formed == 0 || worst.is_nan() {
        f64::NAN
    } else {
        worst
    }
}

/// Result C (section 3) from `rho` without the term (M1 to M3) and with it (B1 to B3 of ADR 0816).
fn mode_c(off: [f64; 3], on: [f64; 3]) -> ([bool; 3], Outcome) {
    if off.iter().chain(&on).any(|x| x.is_nan()) {
        return ([false; 3], Outcome::Undecided);
    }
    let near = |a: f64, b: f64| (a - b).abs() <= BAND;
    let l_off = 1.0 - (1.0 - off[0]) / ALPHAS[0];
    let hc1 = (1..3).all(|i| near(off[i], 1.0 - ALPHAS[i] * (1.0 - l_off)));
    let hc2 = (0..3).all(|i| near(off[i], on[i]));
    let hc3 = (0..3).all(|i| off[i] <= on[i] - BAND);
    let held = [hc1, hc2, hc3];
    let names = ["HC1", "HC2", "HC3"];
    let s: Vec<&'static str> = (0..3).filter(|&i| held[i]).map(|i| names[i]).collect();
    let out = match s.as_slice() {
        ["HC1", "HC3"] => Outcome::Named("HC1"),
        [] => Outcome::AllRuledOut,
        [h] => Outcome::Named(h),
        _ => Outcome::NamesNone(format!("{s:?} hold")),
    };
    (held, out)
}

/// T3 (ADR 0819): the rules of sections 2 and 3 on built records.
#[test]
fn f885_fl_rules_on_built_records() {
    let on = RHO_ON;
    // HC1 alone: the blend from 0.80 at 0.9 (0.8444, 0.8889), not faster at 0.5
    let blend = |r9: f64| {
        let l = 1.0 - (1.0 - r9) / 0.9;
        [r9, 1.0 - 0.7 * (1.0 - l), 1.0 - 0.5 * (1.0 - l)]
    };
    assert_eq!(mode_c(blend(0.80), on).0, [true, false, false]);
    assert_eq!(mode_c(blend(0.80), on).1, Outcome::Named("HC1"));
    // HC1 with HC3: the blend from 0.70, faster everywhere
    assert_eq!(mode_c(blend(0.70), on).0, [true, false, true]);
    assert_eq!(mode_c(blend(0.70), on).1, Outcome::Named("HC1"));
    // HC2 alone: the factors with the term
    assert_eq!(mode_c(on, on).1, Outcome::Named("HC2"));
    // HC3 alone: 0.05 faster everywhere, not a blend
    assert_eq!(mode_c(on.map(|r| r - 0.05), on).1, Outcome::Named("HC3"));
    // HC1 with HC2: within the band of both
    let both = [0.845, 0.876, 0.903];
    assert_eq!(mode_c(both, on).0, [true, true, false]);
    assert!(matches!(mode_c(both, on).1, Outcome::NamesNone(_)));
    // none
    assert_eq!(mode_c([0.95, 0.80, 0.99], on).1, Outcome::AllRuledOut);
    // a factor NaN
    assert_eq!(mode_c([0.85, f64::NAN, 0.9], on).1, Outcome::Undecided);
    // the class of C
    assert_eq!(
        (class(1e-3), class(0.1), class(f64::NAN)),
        ("CONVERGES", "STALLS", "UNDECIDED")
    );
    // FA2
    assert!(cycle_gone(&Outcome::AllRuledOut, 2.0));
    assert!(!cycle_gone(&Outcome::AllRuledOut, 0.05));
    assert!(!cycle_gone(&Outcome::Named("HA1"), 2.0));
    // FA1 on built records: a kept factor has no change, a switching one does, none formed is NaN
    let rec = |psi: &dyn Fn(usize) -> f64| OuterProbe {
        step: 1,
        cell: 0,
        faces: vec![0],
        neighbours: vec![1],
        iterations: (1..=N_OUT)
            .map(|k| ProbeIteration {
                diag: [2.0, 2.0, f64::NAN],
                off_sum: [1.0, 1.0, f64::NAN],
                psi: vec![[psi(k), 1.0, f64::NAN], [1.0, 1.0, f64::NAN]],
                flux: vec![1.0],
                u: [k as f64 * 1e-3, 0.0, 0.0],
            })
            .collect(),
    };
    let kept = rec(&|_| 0.6);
    let switching = rec(&|k| if k % 2 == 0 { 1.0 } else { 0.6 });
    let none = rec(&|_| f64::NAN);
    assert_eq!(kept_change(&[&kept]), 0.0);
    assert!((kept_change(&[&switching]) - 0.4).abs() < 1e-12);
    let none = OuterProbe {
        iterations: none
            .iterations
            .into_iter()
            .map(|mut i| {
                i.psi = vec![[f64::NAN; 3]; 2];
                i
            })
            .collect(),
        ..kept.clone()
    };
    assert!(kept_change(&[&none]).is_nan());
    // C of a run not complete is NaN
    let short = Run {
        name: "x".into(),
        steps: vec![(11, vec![1.0; N_OUT], vec![0; N_OUT])],
        planned: 2,
        failed: None,
    };
    assert!(c_of(&short).is_nan());
}

// ---------------------------------------------------------------- the cavity (ADR 0819 section 4)

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

/// KC0 of ADR 0815 (20 outer iterations, BDF2, the record and the switch on), with the limiter (KL of ADR 0816) or not,
/// and the opt-in.
fn cavity_settings(with_limiter: bool, frozen: bool) -> PimpleSettings {
    let mut s = PimpleSettings::pimple(0.01, N_OUT);
    s.time_scheme = TimeScheme::Bdf2;
    s.outer_velocity_record = true;
    s.alpha_free_face_flux = true;
    if with_limiter {
        s.convection = ConvScheme::LinearUpwind;
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
    }
    s.outer_frozen_limiter = frozen;
    s
}

fn run_cavity(s: PimpleSettings, steps: usize, dt: f64) -> PimpleSolver {
    let mut sol = cavity(s);
    for _ in 0..steps {
        sol.step(dt);
        assert!(sol.diverged.is_none(), "the cavity stopped");
    }
    sol
}

type Bits = (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>);

/// The bits of `u`, `p`, the flux and the record of `d_k`.
fn bits(s: &PimpleSolver) -> Bits {
    (
        s.u.values.iter().flat_map(|v| v.map(f64::to_bits)).collect(),
        s.p.values.iter().map(|v| v.to_bits()).collect(),
        s.phi.iter().map(|v| v.to_bits()).collect(),
        s.outer_velocity
            .iter()
            .flat_map(|r| r.change.iter().map(|v| v.to_bits()))
            .collect(),
    )
}

/// The Barth-Jespersen factor of each component of a velocity field, formed with the crate's public functions (NaN for an
/// inactive component).
fn factors(g: &FvGeometry, u: &VectorField) -> [Vec<f64>; 3] {
    let ub = u.boundary_values(g);
    [0, 1, 2].map(|k| {
        if !g.active[k] {
            return vec![f64::NAN; u.values.len()];
        }
        let cells: Vec<f64> = u.values.iter().map(|v| v[k]).collect();
        let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
        let grad = least_squares(g, &cells, &bv);
        limiter(g, &cells, &bv, &grad, Limiter::BarthJespersen)
    })
}

/// T1 (ADR 0819): the factor the probe records at every cell in every outer iteration against the factor of the state
/// before the step (implicit Euler), and the non-vacuity with the opt-in off.
#[test]
fn f885_fl_two_routes() {
    let settings = |frozen: bool| {
        let mut s = PimpleSettings::pimple(0.01, 5);
        s.convection = ConvScheme::LinearUpwind;
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
        s.outer_frozen_limiter = frozen;
        s.outer_cell_probe = (0..64).collect();
        assert_eq!(s.time_scheme, TimeScheme::ImplicitEuler);
        s
    };
    let mut sol = cavity(settings(true));
    let g = sol.geom.clone();
    let (mut worst, mut compared) = (0.0f64, 0usize);
    let mut miss = Vec::new();
    for step in 1..=CAV_STEPS {
        let want = factors(&g, &sol.u);
        sol.step(CAV_DT);
        assert!(sol.diverged.is_none());
        let probes: Vec<&OuterProbe> = sol.outer_probe.iter().filter(|p| p.step == step).collect();
        assert_eq!(probes.len(), 64);
        for p in probes {
            assert_eq!(p.iterations.len(), 5);
            for (n, i) in p.iterations.iter().enumerate() {
                for k in 0..3 {
                    let d = rel_nan(i.psi[0][k], want[k][p.cell]);
                    if !g.active[k] && !i.psi[0][k].is_nan() {
                        miss.push(format!(
                            "step {step} cell {} iteration {}: inactive {k} not NaN",
                            p.cell,
                            n + 1
                        ));
                    }
                    compared += 1;
                    worst = worst.max(d);
                }
            }
        }
    }
    // the non-vacuity: without the opt-in some factor changes within step 2 or 3
    let mut off = cavity(settings(false));
    let mut moved = 0.0f64;
    for _ in 0..CAV_STEPS {
        off.step(CAV_DT);
        assert!(off.diverged.is_none());
    }
    for p in off.outer_probe.iter().filter(|p| p.step >= 2) {
        for k in (0..3).filter(|&k| g.active[k]) {
            moved = moved.max((p.iterations[4].psi[0][k] - p.iterations[0].psi[0][k]).abs());
        }
    }
    println!(
        "F885-FL T1: {compared} factors compared, largest relative difference {worst:.3e} (limit {ROUTE_TOL:e}); misses {miss:?}; without the opt-in the largest change of a factor in steps 2 and 3 {moved:.3e} (floor {PSI_FLOOR:e})"
    );
    assert!(
        moved > PSI_FLOOR,
        "T1 does not decide: no factor changes without the opt-in ({moved})"
    );
    assert!(worst <= ROUTE_TOL && miss.is_empty(), "T1: {worst}, {miss:?}");
}

/// T2 (ADR 0819): the opt-in read only with the limiter: KC0 of ADR 0815 with the opt-in on against off.
#[test]
fn f885_fl_read_only() {
    let on = run_cavity(cavity_settings(false, true), CAV_STEPS, CAV_DT);
    let off = run_cavity(cavity_settings(false, false), CAV_STEPS, CAV_DT);
    let (a, b) = (bits(&on), bits(&off));
    println!(
        "F885-FL T2 KC0: u equal {}, p equal {}, flux equal {}, record equal {}",
        a.0 == b.0,
        a.1 == b.1,
        a.2 == b.2,
        a.3 == b.3
    );
    assert!(a == b, "T2 missed");
}

/// K (ADR 0819): KL of ADR 0816 with the opt-in contracts.
#[test]
fn f885_fl_cavity_control() {
    let sol = run_cavity(cavity_settings(true, true), CAV_STEPS, CAV_DT);
    for r in &sol.outer_velocity {
        let x: Vec<String> = r.change.iter().map(|a| format!("{a:.4e}")).collect();
        println!("F885-FL K step {}: d [{}]", r.step, x.join(", "));
    }
    let c = upper_median(sol.outer_velocity.iter().map(|r| r.change[18] / r.change[1]).collect());
    println!("F885-FL K: C {c:.6e} ({}, limit {C_CONVERGES:e})", class(c));
    assert!(c <= C_CONVERGES, "K: C {c}");
}

/// S1 (ADR 0819): the steady cavity at Re 10 gives the same answer with the opt-in on as off.
#[test]
fn f885_fl_steady_same_answer() {
    let settings = |frozen: bool| {
        let mut s = PimpleSettings::pimple(S_NU, N_OUT);
        s.convection = ConvScheme::LinearUpwind;
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
        s.alpha_free_face_flux = true;
        s.outer_frozen_limiter = frozen;
        assert_eq!(s.time_scheme, TimeScheme::ImplicitEuler);
        s
    };
    let t0 = Instant::now();
    let mut out = Vec::new();
    let mut pre = Vec::new();
    for frozen in [false, true] {
        let mut sol = run_cavity(settings(frozen), S_STEPS - 1, S_DT);
        let before = sol.u.values.clone();
        sol.step(S_DT);
        assert!(sol.diverged.is_none());
        let last = before
            .iter()
            .zip(&sol.u.values)
            .map(|(a, b)| (0..3).map(|j| (a[j] - b[j]).abs()).fold(0.0, f64::max))
            .fold(0.0, f64::max);
        let g = sol.geom.clone();
        let psi = factors(&g, &sol.u);
        let limited = (0..3)
            .filter(|&k| g.active[k])
            .map(|k| psi[k].iter().filter(|&&f| f < 1.0 - PSI_FLOOR).count())
            .sum::<usize>();
        println!(
            "F885-FL S1 opt-in {frozen}: last step change {last:.3e} (precondition {S_STEADY:e}), limited cell components {limited}"
        );
        pre.push(last <= S_STEADY && limited > 0);
        out.push(sol);
    }
    let (off, on) = (&out[0], &out[1]);
    let umax = off
        .u
        .values
        .iter()
        .map(|v| v.iter().fold(0.0f64, |m, x| m.max(x.abs())))
        .fold(0.0, f64::max);
    let du = off
        .u
        .values
        .iter()
        .zip(&on.u.values)
        .map(|(a, b)| (0..3).map(|j| (a[j] - b[j]).abs()).fold(0.0, f64::max))
        .fold(0.0, f64::max)
        / umax;
    let centred = |p: &[f64]| {
        let m = p.iter().sum::<f64>() / p.len() as f64;
        p.iter().map(|x| x - m).collect::<Vec<f64>>()
    };
    let (po, pn) = (centred(&off.p.values), centred(&on.p.values));
    let pmax = po.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let dp = po.iter().zip(&pn).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / pmax;
    println!(
        "F885-FL S1: u relative {du:.3e}, p relative {dp:.3e} (limit {S_SAME:e}); preconditions {pre:?}; {:.1} s",
        t0.elapsed().as_secs_f64()
    );
    assert!(pre.iter().all(|&x| x), "S1 does not decide: preconditions {pre:?}");
    assert!(du <= S_SAME && dp <= S_SAME, "S1: u {du}, p {dp}");
}

// ---------------------------------------------------------------- the fan runs (ADR 0819 section 2)

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-frozen-limiter")
}

fn record_0816() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-two-outer-modes/fan_d.csv")
}

/// The runs in their order: name, the limiter on, the opt-in on, ADR 0811's term on, `alpha_u`.
const RUNS: [(&str, bool, bool, bool, f64); 5] = [
    ("F", true, true, true, 0.9),
    ("B1r", false, false, true, 0.9),
    ("M1", false, false, false, ALPHAS[0]),
    ("M2", false, false, false, ALPHAS[1]),
    ("M3", false, false, false, ALPHAS[2]),
];

/// One fan run: 20 steps of H(dt) as ADR 0816's harness, the probe at c11837 and c6245, then the run's settings.
fn fan_run(name: &str, lim: bool, frozen: bool, term: bool, alpha: f64) -> (Run, Vec<OuterProbe>) {
    let mut sim = sim_of(&h_text(H_DT));
    assert_eq!(sim.geom.mesh.n_cells(), 13627);
    {
        let s = &mut sim.pimple_solver_mut().expect("pimple").settings;
        assert_eq!(
            (s.alpha_u, s.alpha_p, s.n_outer, s.n_correctors),
            (0.9, 1.0, 5, 2),
            "the case's settings"
        );
        assert!(s.linear_upwind_limiter.is_some() && !s.rotational_periodic_implicit);
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        s.alpha_free_face_flux = term;
        s.n_outer = N_OUT;
        s.outer_cell_probe = vec![C_A, C_B];
        if !lim {
            s.linear_upwind_limiter = None;
        }
        s.outer_frozen_limiter = frozen;
        s.alpha_u = alpha;
    }
    let mut run = Run {
        name: name.into(),
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
            Some(r) if r.step == k && r.change.len() == N_OUT && st.res_u.is_finite() => {
                if !r.change.iter().all(|d| d.is_finite()) {
                    run.failed = Some(format!("non-finite change at step {k}"));
                    break;
                }
                let x: Vec<String> = r.change.iter().map(|a| format!("{a:.4e}")).collect();
                println!(
                    "F885-FL {name} step {k}: d [{}] cells {:?}, {:.2} s per step",
                    x.join(", "),
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
        "F885-FL {name} END: {} of {FAN_STEPS} steps, failed {:?}, {:.1} s",
        run.steps.len(),
        run.failed,
        t0.elapsed().as_secs_f64()
    );
    let probes = sim.pimple_solver().map_or_else(Vec::new, |s| {
        s.outer_probe.iter().filter(|p| p.step >= FAN_FROM).cloned().collect()
    });
    (run, probes)
}

const D_HEADER: &str = "run,step,k,d,cell";
const P_HEADER: &str = "run,step,cell,k,diag0,diag1,diag2,off0,off1,off2,u0,u1,u2,psi,flux";

fn join(v: impl Iterator<Item = f64>) -> String {
    v.map(|x| format!("{x:?}")).collect::<Vec<_>>().join(";")
}

/// The records: `fan_d.csv` (every step) and `probe.csv` (the classed steps, values in round-trip form).
fn write_records(dir: &Path, runs: &[(Run, Vec<OuterProbe>)]) {
    let mut d = format!(
        "# ADR 0819 (F-885): the fan runs F, B1r, M1, M2, M3 (20 outer iterations) on 20 steps of H(dt) (dt = {H_DT:?} s); d [m/s] the velocity change of outer iteration k, cell its cell\n{D_HEADER}\n"
    );
    let mut p = format!(
        "# ADR 0819 (F-885): the probe at c{C_A} and c{C_B} in the classed steps; diag the momentum diagonal before the relaxation, off the sum the relaxation takes, u the velocity after the correctors, psi the factors used (cell then neighbours, three components each), flux the outward convecting flux of each face\n{P_HEADER}\n"
    );
    for (r, probes) in runs {
        writeln!(d, "# {} steps {} failed {:?}", r.name, r.steps.len(), r.failed).unwrap();
        for (n, dk, c) in &r.steps {
            for k in 0..dk.len() {
                writeln!(d, "{},{n},{},{:.9e},{}", r.name, k + 1, dk[k], c[k]).unwrap();
            }
        }
        for pr in probes {
            for (k, i) in pr.iterations.iter().enumerate() {
                writeln!(
                    p,
                    "{},{},{},{},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{},{}",
                    r.name,
                    pr.step,
                    pr.cell,
                    k + 1,
                    i.diag[0],
                    i.diag[1],
                    i.diag[2],
                    i.off_sum[0],
                    i.off_sum[1],
                    i.off_sum[2],
                    i.u[0],
                    i.u[1],
                    i.u[2],
                    join(i.psi.iter().flat_map(|e| e.iter().copied())),
                    join(i.flux.iter().copied())
                )
                .unwrap();
            }
        }
    }
    std::fs::write(dir.join("fan_d.csv"), d).unwrap();
    std::fs::write(dir.join("probe.csv"), p).unwrap();
}

/// R0 and R1: the largest relative difference of `d_k` against a run of ADR 0816, over every `k` of 20 steps or (`first`)
/// `d_1` of step 1 only (NaN if a step is missing).
fn against_0816(run: &Run, of: &str, first: bool) -> f64 {
    let theirs = &read_d(&record_0816(), &[of])[0];
    if first {
        return match (run.steps.first(), theirs.steps.first()) {
            (Some(a), Some(b)) if a.0 == 1 && b.0 == 1 => rel(a.1[0], b.1[0]),
            _ => f64::NAN,
        };
    }
    if run.steps.len() != FAN_STEPS || theirs.steps.len() != FAN_STEPS {
        return f64::NAN;
    }
    let mut worst = 0.0f64;
    for ((na, da, _), (nb, db, _)) in run.steps.iter().zip(&theirs.steps) {
        assert_eq!(na, nb);
        for (a, b) in da.iter().zip(db) {
            worst = worst.max(rel(*a, *b));
        }
    }
    worst
}

/// The outcomes of the fan runs.
struct Outcomes {
    held_a: [bool; 3],
    q: f64,
    out_a: Outcome,
    fa1: f64,
    c_f: f64,
    rhos: [f64; 4],
    cs: [f64; 4],
    r_b: [f64; 4],
    held_c: [bool; 3],
    out_c: Outcome,
    r0: f64,
    r1: f64,
}

fn fan_outcomes(runs: &[(Run, Vec<OuterProbe>)]) -> Outcomes {
    let at = |i: usize, cell: usize| -> Vec<&OuterProbe> { runs[i].1.iter().filter(|p| p.cell == cell).collect() };
    let n_classed = FAN_STEPS - FAN_FROM + 1;
    let fa = at(0, C_A);
    let (held_a, q, out_a) = mode_a(&fa, 0.9, runs[0].0.complete() && fa.len() == n_classed);
    let fa1 = if fa.len() == n_classed {
        kept_change(&fa)
    } else {
        f64::NAN
    };
    let rhos = [1, 2, 3, 4].map(|i| rho(&runs[i].0));
    let cs = [1, 2, 3, 4].map(|i| c_of(&runs[i].0));
    let r_b = [1, 2, 3, 4].map(|i| {
        let v = at(i, C_B);
        if v.len() == n_classed {
            dominance(&v)
        } else {
            f64::NAN
        }
    });
    let (held_c, out_c) = mode_c([rhos[1], rhos[2], rhos[3]], RHO_ON);
    Outcomes {
        held_a,
        q,
        out_a,
        fa1,
        c_f: c_of(&runs[0].0),
        rhos,
        cs,
        r_b,
        held_c,
        out_c,
        r0: against_0816(&runs[1].0, "B1", false),
        r1: against_0816(&runs[0].0, "A0", true),
    }
}

fn print_outcomes(o: &Outcomes) {
    println!(
        "F885-FL FA1: largest change of the kept factor at c{C_A} and its neighbours {:.3e} (limit {ROUTE_TOL:e})",
        o.fa1
    );
    println!(
        "F885-FL FA2: mode A at c{C_A}: HA1 {} HA2 {} HA3 {}, q {:.6} (gone if >= {Q_GONE}), result {}; cycle gone {}",
        o.held_a[0],
        o.held_a[1],
        o.held_a[2],
        o.q,
        oname(&o.out_a),
        cycle_gone(&o.out_a, o.q)
    );
    println!("F885-FL FC: F C {:.6e} ({})", o.c_f, class(o.c_f));
    println!(
        "F885-FL R0: B1r against B1 of ADR 0816 {:.3e}; R1: F d_1 of step 1 against A0 {:.3e} (limit {REC_TOL:e})",
        o.r0, o.r1
    );
    for i in 0..4 {
        println!(
            "F885-FL {}: rho {:.6}, C {:.6e} ({}), r at c{C_B} {:.6}",
            RUNS[i + 1].0,
            o.rhos[i],
            o.cs[i],
            class(o.cs[i]),
            o.r_b[i]
        );
    }
    let l_off = 1.0 - (1.0 - o.rhos[1]) / ALPHAS[0];
    println!(
        "F885-FL mode B: rho off {:.6} {:.6} {:.6}, on {:?}; lambda_off {l_off:.6}, HC1 predicts {:.6} (0.7), {:.6} (0.5); HC1 {} HC2 {} HC3 {}; result C {}",
        o.rhos[1],
        o.rhos[2],
        o.rhos[3],
        RHO_ON,
        1.0 - ALPHAS[1] * (1.0 - l_off),
        1.0 - ALPHAS[2] * (1.0 - l_off),
        o.held_c[0],
        o.held_c[1],
        o.held_c[2],
        oname(&o.out_c)
    );
}

/// F, B1r, M1 to M3 (ADR 0819): the heavy record of the fan runs, written to `F885_FL_OUT`.
#[test]
#[ignore = "heavy record, ADR 0819: F, B1r, M1, M2, M3 on 20 steps of H(dt) on B2 (13 627 cells, 2 000 outer iterations, minutes): cargo test --release -p rcfd-api --test f885_frozen_limiter f885_fl_fan_record -- --ignored --nocapture"]
fn f885_fl_fan_record() {
    let dir = std::env::var("F885_FL_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let mut runs: Vec<(Run, Vec<OuterProbe>)> = Vec::new();
    for (name, lim, frozen, term, alpha) in RUNS {
        let (r, probes) = fan_run(name, lim, frozen, term, alpha);
        for p in &probes {
            let i = &p.iterations;
            println!(
                "F885-FL {name} step {} c{}: factors of the cell (k 1, 18, 19, 20) {:?}, u (k 18, 19, 20) {:?}",
                p.step,
                p.cell,
                [i[0].psi[0], i[17].psi[0], i[18].psi[0], i[19].psi[0]],
                [i[17].u, i[18].u, i[19].u]
            );
        }
        runs.push((r, probes));
        write_records(&dir, &runs);
    }
    print_outcomes(&fan_outcomes(&runs));
    println!("F885-FL fan records written to {}", dir.display());
}

/// G2 (ADR 0819): the outcomes from the committed records, held by `FAN_RECORD`, and FA1, FA2, FC, R0, R1, BC from them.
#[test]
fn f885_fl_fan_from_the_committed_record() {
    let names: Vec<&str> = RUNS.iter().map(|r| r.0).collect();
    let d = read_d(&record_dir().join("fan_d.csv"), &names);
    let probes = read_probe(&record_dir().join("probe.csv"));
    let runs: Vec<(Run, Vec<OuterProbe>)> = d
        .into_iter()
        .map(|r| {
            let p = probes
                .iter()
                .filter(|(n, _)| *n == r.name)
                .map(|(_, p)| p.clone())
                .collect();
            (r, p)
        })
        .collect();
    for (r, p) in &runs {
        println!(
            "F885-FL {}: {} steps, failed {:?}, {} probe records",
            r.name,
            r.steps.len(),
            r.failed,
            p.len()
        );
    }
    let o = fan_outcomes(&runs);
    print_outcomes(&o);
    let want = FAN_RECORD.expect("no recorded outcome yet: the registered fan run of ADR 0819 has not been recorded");
    let same = |a: f64, b: f64| (a.is_nan() && b.is_nan()) || rel(a, b) <= REC_TOL;
    assert_eq!(o.held_a, want.0, "mode A held at c{C_A}");
    assert!(same(o.q, want.1), "q {} against {}", o.q, want.1);
    assert!(same(o.fa1, want.2), "FA1 {} against {}", o.fa1, want.2);
    assert!(same(o.c_f, want.3), "C {} against {}", o.c_f, want.3);
    for i in 0..4 {
        assert!(same(o.rhos[i], want.4[i]), "{}: {}", RUNS[i + 1].0, o.rhos[i]);
    }
    assert_eq!(oname(&o.out_c), want.5);
    let mut miss = Vec::new();
    if !(o.fa1 <= ROUTE_TOL) {
        miss.push(format!("FA1 {}", o.fa1));
    }
    if !cycle_gone(&o.out_a, o.q) {
        miss.push(format!("FA2 {} q {}", oname(&o.out_a), o.q));
    }
    if class(o.c_f) == "UNDECIDED" {
        miss.push("FC undecided".into());
    }
    if !(o.r0 <= REC_TOL && o.r1 <= REC_TOL) {
        miss.push(format!("R0 {} R1 {}", o.r0, o.r1));
    }
    if !(runs[2..].iter().all(|r| r.0.complete()) && matches!(o.out_c, Outcome::Named(_) | Outcome::AllRuledOut)) {
        miss.push(format!("BC {}", oname(&o.out_c)));
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}
