//! ADR 0816 (F-885): the two modes of the fan's outer loop that ADR 0815's record shows. Mode A, the constant change at
//! the interior cell c11837 with the Barth-Jespersen limiter (A0 = G0 of ADR 0815 with the probe): a limiter factor that
//! switches (HA1), a diagonal dominance lost at the cell (HA2), a face flux that changes sign (HA3). Mode B, the slow
//! contraction at c6245 without the limiter (B1 = GL of ADR 0815; B2, B3 with `alpha_u` 0.7 and 0.5; B4 with four
//! correctors): HB1 to HB4 by the factor each predicts. The opt-in probe `outer_cell_probe` is checked by two routes and as
//! read-only on the cavity; the cavity with the limiter is the control. The case text is copied unchanged from
//! `f885_outer_stall.rs` (ADR 0815). The fan runs are a heavy record; the gate reads its committed files.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::{convection, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::{laplacian_opt, uniform_gamma};
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
/// T1: relative tolerance of two routes separated only by rounding.
const ROUTE_TOL: f64 = 1e-12;
/// ADR 0816 section 2, mode A: the floor of a switching factor, the return ratio, the local factor, the sign changes, the
/// share of the classed steps.
const PSI_FLOOR: f64 = 1e-9;
const RETURN_RATIO: f64 = 0.1;
const LOCAL_FACTOR: f64 = 0.99;
const SIGN_CHANGES: usize = 8;
const STEP_SHARE: f64 = 0.8;
/// ADR 0816 section 2, mode B: the band of a predicted factor.
const BAND: f64 = 0.015;
/// Mode B: `alpha_u` and the corrector count of B1 to B4.
const B_ALPHA: [f64; 4] = [0.9, 0.7, 0.5, 0.9];
const B_CORR: [usize; 4] = [2, 2, 2, 4];
/// The cavity probe cells (interior of the 8 x 8 mesh).
const CAV_PROBE: [usize; 2] = [27, 36];

/// The recorded outcomes of the registered fan run (written after the run): mode A (HA1, HA2, HA3 held; `q`; the result),
/// mode B (`rho` and `r` of B1 to B4; the result).
type FanRecord = (([bool; 3], f64, &'static str), ([f64; 4], [f64; 4], &'static str));
const FAN_RECORD: Option<FanRecord> = Some((
    ([true, false, false], 0.005036605059273898, "named HA1"),
    (
        [0.853789228181186, 0.8729920722691961, 0.8920440841467279, 0.8537402360728994],
        [1.3805424501415016, 1.3805235301438004, 1.380475306072651, 1.3805439272334756],
        "all ruled out",
    ),
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

/// The result of mode B from the factors and the dominance ratios of B1 to B4: the hypotheses held and the result.
fn mode_b(rho: [f64; 4], r: [f64; 4]) -> ([bool; 4], Outcome) {
    if rho.iter().chain(&r[..3]).any(|x| x.is_nan()) {
        return ([false; 4], Outcome::Undecided);
    }
    let near = |a: f64, b: f64| (a - b).abs() <= BAND;
    let hb1 = r[0] < 1.0 && (0..3).all(|i| near(rho[i], 1.0 - B_ALPHA[i] * r[i]));
    let hb2 = rho[3] <= rho[0] - BAND;
    let l0 = 1.0 - (1.0 - rho[0]) / B_ALPHA[0];
    let hb3 = near(rho[1], 1.0 - B_ALPHA[1] * (1.0 - l0))
        && near(rho[2], 1.0 - B_ALPHA[2] * (1.0 - l0))
        && near(rho[3], rho[0]);
    let hb4 = near(rho[1], rho[0]) && near(rho[2], rho[0]);
    let held = [hb1, hb2, hb3, hb4];
    let names = ["HB1", "HB2", "HB3", "HB4"];
    let s: Vec<&'static str> = (0..4).filter(|&i| held[i]).map(|i| names[i]).collect();
    let out = match s.as_slice() {
        ["HB1", "HB3"] => Outcome::Named("HB1"),
        [] => Outcome::AllRuledOut,
        [h] => Outcome::Named(h),
        _ => Outcome::NamesNone(format!("{s:?} hold")),
    };
    (held, out)
}

/// A built probe record of 20 iterations: the factor, the diagonal and sum, the flux of one face, per `k`.
fn built(psi: impl Fn(usize) -> f64, dom: (f64, f64), flux: impl Fn(usize) -> f64) -> OuterProbe {
    OuterProbe {
        step: 1,
        cell: 0,
        faces: vec![0],
        neighbours: vec![1],
        iterations: (1..=20)
            .map(|k| ProbeIteration {
                diag: [dom.0, dom.0, f64::NAN],
                off_sum: [dom.1, dom.1, f64::NAN],
                psi: vec![[psi(k), 1.0, f64::NAN], [1.0, 1.0, f64::NAN]],
                flux: vec![flux(k)],
                u: [k as f64 * 1e-3, 0.0, 0.0],
            })
            .collect(),
    }
}

/// T3 (ADR 0816): the rules of section 2 on built records.
#[test]
fn f885_tm_rules_on_built_records() {
    let alt = |k: usize| 0.5 + 0.1 * if k % 2 == 0 { 1.0 } else { -1.0 };
    let drift = |k: usize| 0.5 + 1e-3 * k as f64;
    let pinned = |_: usize| 1.0;
    let pos = |_: usize| 1.0;
    let flip = |k: usize| if k % 2 == 0 { 1.0 } else { -1.0 };
    // HA1
    assert!(ha1_step(&built(alt, (2.0, 1.0), pos)));
    assert!(!ha1_step(&built(drift, (2.0, 1.0), pos)));
    assert!(!ha1_step(&built(pinned, (2.0, 1.0), pos)));
    // HA2: lost with a local factor 1 - 0.9 * 0.005 >= 0.99; lost with 1 - 0.9 * 0.5 < 0.99; kept
    assert!(ha2_step(&built(pinned, (0.005, 1.0), pos), 0.9));
    assert!(!ha2_step(&built(pinned, (1.0, 2.0), pos), 0.9));
    assert!(!ha2_step(&built(pinned, (2.0, 1.0), pos), 0.9));
    // HA3
    assert!(ha3_step(&built(pinned, (2.0, 1.0), flip)));
    assert!(!ha3_step(&built(pinned, (2.0, 1.0), pos)));
    // the shape: a drift has `q` 2
    assert!((shape_step(&built(pinned, (2.0, 1.0), pos)) - 2.0).abs() < 1e-9);
    // the result of mode A over ten steps
    let ten = |p: OuterProbe| vec![p; 10];
    let only1 = ten(built(alt, (2.0, 1.0), pos));
    fn refs(v: &[OuterProbe]) -> Vec<&OuterProbe> {
        v.iter().collect()
    }
    assert_eq!(mode_a(&refs(&only1), 0.9, true).2, Outcome::Named("HA1"));
    let none = ten(built(drift, (2.0, 1.0), pos));
    assert_eq!(mode_a(&refs(&none), 0.9, true).2, Outcome::AllRuledOut);
    let three = ten(built(alt, (0.005, 1.0), flip));
    assert!(matches!(mode_a(&refs(&three), 0.9, true).2, Outcome::NamesNone(_)));
    assert_eq!(mode_a(&refs(&only1), 0.9, false).2, Outcome::Undecided);
    // seven of ten steps switching is not 80 %
    let mut seven = ten(built(alt, (2.0, 1.0), pos));
    for p in seven.iter_mut().take(3) {
        *p = built(drift, (2.0, 1.0), pos);
    }
    assert_eq!(mode_a(&refs(&seven), 0.9, true).2, Outcome::AllRuledOut);
    let mut eight = ten(built(alt, (2.0, 1.0), pos));
    for p in eight.iter_mut().take(2) {
        *p = built(drift, (2.0, 1.0), pos);
    }
    assert_eq!(mode_a(&refs(&eight), 0.9, true).2, Outcome::Named("HA1"));
    // mode B: the blend (HB3) with the dominance kept
    let blend = [0.85, 1.0 - 0.7 * 0.15 / 0.9, 1.0 - 0.5 * 0.15 / 0.9, 0.85];
    assert_eq!(mode_b(blend, [2.0; 4]).1, Outcome::Named("HB3"));
    // the blend with the dominance lost that predicts it: HB1 and HB3, named HB1
    let r: [f64; 4] = [0, 1, 2, 3].map(|i| (1.0 - blend[i]) / B_ALPHA[i]);
    assert_eq!(mode_b(blend, r).0, [true, false, true, false]);
    assert_eq!(mode_b(blend, r).1, Outcome::Named("HB1"));
    // a factor that does not depend on alpha (HB4); with four correctors faster as well (HB2 and HB4)
    assert_eq!(mode_b([0.85; 4], [2.0; 4]).1, Outcome::Named("HB4"));
    assert!(matches!(
        mode_b([0.85, 0.85, 0.85, 0.80], [2.0; 4]).1,
        Outcome::NamesNone(_)
    ));
    assert_eq!(mode_b([0.85, 0.95, 0.6, 0.85], [2.0; 4]).1, Outcome::AllRuledOut);
    assert_eq!(mode_b([0.85, 0.95, 0.6, 0.80], [2.0; 4]).1, Outcome::Named("HB2"));
    assert_eq!(mode_b([0.85, f64::NAN, 0.6, 0.85], [2.0; 4]).1, Outcome::Undecided);
    // a run not complete has no factor
    let short = Run {
        name: "x".into(),
        steps: vec![(11, vec![1.0; N_OUT], vec![0; N_OUT])],
        planned: 2,
        failed: None,
    };
    assert!(rho(&short).is_nan());
    let mut d = vec![1.0; N_OUT];
    d[18] = 0.125;
    let full = Run {
        steps: vec![(11, d, vec![0; N_OUT])],
        planned: 1,
        ..short
    };
    assert!((rho(&full) - 0.5).abs() < 1e-12);
}

// ---------------------------------------------------------------- the cavity (ADR 0816 sections 2 and 4)

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

/// KC0 of ADR 0815 (20 outer iterations, BDF2, the record and the switch on), with the limiter (KL) or not.
fn cavity_settings(with_limiter: bool, probe: &[usize]) -> PimpleSettings {
    let mut s = PimpleSettings::pimple(0.01, N_OUT);
    s.time_scheme = TimeScheme::Bdf2;
    s.outer_velocity_record = true;
    s.alpha_free_face_flux = true;
    if with_limiter {
        s.convection = ConvScheme::LinearUpwind;
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
    }
    s.outer_cell_probe = probe.to_vec();
    s
}

fn run_cavity(s: PimpleSettings) -> PimpleSolver {
    let mut sol = cavity(s);
    for _ in 0..CAV_STEPS {
        sol.step(CAV_DT);
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

/// T1 (ADR 0816): the probe against the quantities assembled in the test from the state before each step (first outer
/// iteration, implicit Euler), and its velocity of the last iteration against the solver's.
#[test]
fn f885_tm_probe_two_routes() {
    let mut s = PimpleSettings::pimple(0.01, 5);
    s.convection = ConvScheme::LinearUpwind;
    s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
    s.outer_cell_probe = vec![CAV_PROBE[0]];
    assert_eq!(s.time_scheme, TimeScheme::ImplicitEuler);
    let mut sol = cavity(s);
    let g = sol.geom.clone();
    let c = CAV_PROBE[0];
    let vol = g.mesh.cell_volumes().to_vec();
    let gamma = uniform_gamma(&g, 0.01);
    let mut worst = 0.0f64;
    let mut miss = Vec::new();
    for step in 1..=CAV_STEPS {
        let (u0, phi0) = (sol.u.clone(), sol.phi.clone());
        sol.step(CAV_DT);
        assert!(sol.diverged.is_none());
        let p = sol.outer_probe.last().unwrap().clone();
        assert_eq!((p.step, p.cell, p.iterations.len()), (step, c, 5));
        let i1 = &p.iterations[0];
        let ub = u0.boundary_values(&g);
        for k in 0..3 {
            if !g.active[k] {
                let nan = i1.diag[k].is_nan() && i1.off_sum[k].is_nan() && i1.psi.iter().all(|e| e[k].is_nan());
                if !nan {
                    miss.push(format!("step {step}: inactive component {k} not NaN"));
                }
                continue;
            }
            let cells: Vec<f64> = u0.values.iter().map(|v| v[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            let bc = u0.boundary_coeffs(&g, k);
            let grad = least_squares(&g, &cells, &bv);
            let psi = limiter(&g, &cells, &bv, &grad, Limiter::BarthJespersen);
            let lim: Vec<Vec3> = grad
                .iter()
                .zip(&psi)
                .map(|(v, f)| [v[0] * f, v[1] * f, v[2] * f])
                .collect();
            let mut m = convection(&g, &phi0, &cells, &bc, ConvScheme::LinearUpwind, Some(&lim));
            m.add(&laplacian_opt(&g, &gamma, &bc, Some(&grad), &cells, &bv, None));
            let off = m.a.off_diag_abs_sum()[c];
            let diag = m.a.diag[c] + vol[c] / CAV_DT;
            let mut d = rel_nan(i1.diag[k], diag).max(rel_nan(i1.off_sum[k], off));
            for (e, &cell) in std::iter::once(&c).chain(&p.neighbours).enumerate() {
                d = d.max(rel_nan(i1.psi[e][k], psi[cell]));
            }
            println!(
                "F885-TM T1 step {step} component {k}: a_c {:.6e} (test {diag:.6e}), s {:.6e} (test {off:.6e}), factor {:.4} (test {:.4}), largest relative difference {d:.3e}",
                i1.diag[k], i1.off_sum[k], i1.psi[0][k], psi[c]
            );
            worst = worst.max(d);
        }
        let own = g.mesh.owner();
        for (fi, &f) in p.faces.iter().enumerate() {
            let want = if own[f] == c { phi0[f] } else { -phi0[f] };
            worst = worst.max(rel(i1.flux[fi], want));
        }
        let ul = p.iterations[4].u;
        worst = worst.max((0..3).map(|j| rel(ul[j], sol.u.values[c][j])).fold(0.0, f64::max));
    }
    println!("F885-TM T1: largest relative difference {worst:.3e} (limit {ROUTE_TOL:e}); misses {miss:?}");
    assert!(worst <= ROUTE_TOL && miss.is_empty(), "T1: {worst}, {miss:?}");
}

/// T2 (ADR 0816): the probe read-only: KC0 of ADR 0815 and KL with the probe at 27 and 36 against without.
#[test]
fn f885_tm_probe_read_only() {
    let mut miss = Vec::new();
    for (name, lim) in [("KC0", false), ("KL", true)] {
        let on = run_cavity(cavity_settings(lim, &CAV_PROBE));
        let off = run_cavity(cavity_settings(lim, &[]));
        let (a, b) = (bits(&on), bits(&off));
        println!(
            "F885-TM T2 {name}: u equal {}, p equal {}, flux equal {}, record equal {}; probe records {} (with), {} (without)",
            a.0 == b.0,
            a.1 == b.1,
            a.2 == b.2,
            a.3 == b.3,
            on.outer_probe.len(),
            off.outer_probe.len()
        );
        if a != b || on.outer_probe.len() != CAV_STEPS * 2 || !off.outer_probe.is_empty() {
            miss.push(name);
        }
    }
    assert!(miss.is_empty(), "T2 missed: {miss:?}");
}

/// K2 and K3 (ADR 0816): the cavity with the limiter (KL) contracts and the rule of mode A at cell 27 rules all out.
#[test]
fn f885_tm_cavity_control() {
    let sol = run_cavity(cavity_settings(true, &CAV_PROBE));
    for r in &sol.outer_velocity {
        let x: Vec<String> = r.change.iter().map(|a| format!("{a:.4e}")).collect();
        println!("F885-TM KL step {}: d [{}]", r.step, x.join(", "));
    }
    let c = upper_median(sol.outer_velocity.iter().map(|r| r.change[18] / r.change[1]).collect());
    let at: Vec<&OuterProbe> = sol.outer_probe.iter().filter(|p| p.cell == CAV_PROBE[0]).collect();
    let (held, q, out) = mode_a(&at, 0.8, sol.outer_velocity.len() == CAV_STEPS);
    println!(
        "F885-TM KL: C {c:.6e} ({}), mode A at cell {}: HA1 {} HA2 {} HA3 {}, q {q:.4}, result {}",
        if c <= C_CONVERGES { "CONVERGES" } else { "not CONVERGES" },
        CAV_PROBE[0],
        held[0],
        held[1],
        held[2],
        oname(&out)
    );
    let mut miss = Vec::new();
    if !(c <= C_CONVERGES) {
        miss.push(format!("K2: C {c}"));
    }
    if out != Outcome::AllRuledOut {
        miss.push(format!("K3: {}", oname(&out)));
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}

// ---------------------------------------------------------------- the fan runs (ADR 0816 section 4)

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-two-outer-modes")
}

fn record_0815() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-outer-stall/fan_controls.csv")
}

/// The runs in their order: name, the limiter on, `alpha_u`, the correctors, the run of ADR 0815 it reproduces.
const RUNS: [(&str, bool, f64, usize, Option<&str>); 5] = [
    ("A0", true, 0.9, 2, Some("G0")),
    ("B1", false, B_ALPHA[0], B_CORR[0], Some("GL")),
    ("B2", false, B_ALPHA[1], B_CORR[1], None),
    ("B3", false, B_ALPHA[2], B_CORR[2], None),
    ("B4", false, B_ALPHA[3], B_CORR[3], None),
];

/// One fan run: 20 steps of H(dt) as ADR 0815's harness, the probe at c11837 and c6245, then the run's settings.
fn fan_run(name: &str, lim: bool, alpha: f64, corr: usize) -> (Run, Vec<OuterProbe>) {
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
        println!(
            "F885-TM {name}: corrector control {}",
            if s.corrector_control.is_some() { "on" } else { "off" }
        );
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        s.alpha_free_face_flux = true;
        s.n_outer = N_OUT;
        s.outer_cell_probe = vec![C_A, C_B];
        if !lim {
            s.linear_upwind_limiter = None;
        }
        s.alpha_u = alpha;
        s.n_correctors = corr;
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
                    "F885-TM {name} step {k}: d [{}] cells {:?}, {:.2} s per step",
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
        "F885-TM {name} END: {} of {FAN_STEPS} steps, failed {:?}, {:.1} s",
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
        "# ADR 0816 (F-885): the fan runs A0 and B1 to B4 (alpha_free_face_flux on, 20 outer iterations) on 20 steps of H(dt) (dt = {H_DT:?} s); d [m/s] the velocity change of outer iteration k, cell its cell\n{D_HEADER}\n"
    );
    let mut p = format!(
        "# ADR 0816 (F-885): the probe at c{C_A} and c{C_B} in the classed steps; diag the momentum diagonal before the relaxation, off the sum the relaxation takes, u the velocity after the correctors, psi the factors (cell then neighbours, three components each), flux the outward convecting flux of each face\n{P_HEADER}\n"
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

/// R0: the largest relative difference of a run's `d_k` against a run of ADR 0815 (NaN if a step is missing).
fn r0(run: &Run, of: &str) -> f64 {
    let theirs = &read_d(&record_0815(), &[of])[0];
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

/// R1: in A0's classed steps, where the record's cell of `d_k` is c11837 (`k >= 2`), `|u_k - u_(k-1)|` from the probe against
/// `d_k`: the largest relative difference and the number of iterations compared.
fn r1(run: &Run, probes: &[&OuterProbe]) -> (f64, usize) {
    let (mut worst, mut n) = (0.0f64, 0usize);
    for p in probes.iter().filter(|p| p.cell == C_A) {
        let Some((_, d, cells)) = run.steps.iter().find(|s| s.0 == p.step) else {
            return (f64::NAN, 0);
        };
        for k in 2..=N_OUT {
            if cells[k - 1] == C_A {
                worst = worst.max(rel(norm(it(p, k).u, it(p, k - 1).u), d[k - 1]));
                n += 1;
            }
        }
    }
    (worst, n)
}

/// The outcomes of the fan runs: mode A at c11837 from A0, mode B from B1 to B4 (`rho` and `r` at c6245).
type Outcomes = (([bool; 3], f64, Outcome), ([f64; 4], [f64; 4], [bool; 4], Outcome));

fn fan_outcomes(runs: &[(Run, Vec<OuterProbe>)]) -> Outcomes {
    let at = |i: usize, cell: usize| -> Vec<&OuterProbe> { runs[i].1.iter().filter(|p| p.cell == cell).collect() };
    let a = mode_a(
        &at(0, C_A),
        0.9,
        runs[0].0.complete() && at(0, C_A).len() == FAN_STEPS - FAN_FROM + 1,
    );
    let rhos = [1, 2, 3, 4].map(|i| rho(&runs[i].0));
    let rs = [1, 2, 3, 4].map(|i| {
        let v = at(i, C_B);
        if v.len() == FAN_STEPS - FAN_FROM + 1 {
            dominance(&v)
        } else {
            f64::NAN
        }
    });
    let (held, out) = mode_b(rhos, rs);
    (a, (rhos, rs, held, out))
}

fn print_outcomes(o: &Outcomes) {
    let ((ha, q, oa), (rhos, rs, hb, ob)) = o;
    println!(
        "F885-TM mode A at c{C_A}: HA1 {} HA2 {} HA3 {}, q {q:.6}, result {}",
        ha[0],
        ha[1],
        ha[2],
        oname(oa)
    );
    for i in 0..4 {
        println!(
            "F885-TM mode B {}: alpha_u {} correctors {}: rho {:.6}, r {:.6}",
            RUNS[i + 1].0,
            B_ALPHA[i],
            B_CORR[i],
            rhos[i],
            rs[i]
        );
    }
    let l0 = 1.0 - (1.0 - rhos[0]) / B_ALPHA[0];
    println!(
        "F885-TM mode B: lambda_0 {l0:.6}; predicted by HB3 {:.6} (B2), {:.6} (B3); by HB1 {:.6}, {:.6}, {:.6}; HB1 {} HB2 {} HB3 {} HB4 {}; result {}",
        1.0 - B_ALPHA[1] * (1.0 - l0),
        1.0 - B_ALPHA[2] * (1.0 - l0),
        1.0 - B_ALPHA[0] * rs[0],
        1.0 - B_ALPHA[1] * rs[1],
        1.0 - B_ALPHA[2] * rs[2],
        hb[0],
        hb[1],
        hb[2],
        hb[3],
        oname(ob)
    );
}

/// A0 and B1 to B4 (ADR 0816): the heavy record of the fan runs, written to `F885_TM_OUT`.
#[test]
#[ignore = "heavy record, ADR 0816: A0 and B1 to B4 on 20 steps of H(dt) on B2 (13 627 cells, 2 000 outer iterations, minutes): cargo test --release -p rcfd-api --test f885_two_outer_modes f885_tm_fan_record -- --ignored --nocapture"]
fn f885_tm_fan_record() {
    let dir = std::env::var("F885_TM_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let mut runs: Vec<(Run, Vec<OuterProbe>)> = Vec::new();
    for (name, lim, alpha, corr, of) in RUNS {
        let (r, probes) = fan_run(name, lim, alpha, corr);
        for p in &probes {
            let i = &p.iterations;
            println!(
                "F885-TM {name} step {} c{}: a_c {:?} s {:?} (k 19), factors of the cell {:?} (k 17, 18, 19), fluxes k 19 {:?}",
                p.step,
                p.cell,
                i[18].diag,
                i[18].off_sum,
                [i[16].psi[0], i[17].psi[0], i[18].psi[0]],
                i[18].flux
            );
        }
        if let Some(of) = of {
            println!(
                "F885-TM R0: {name} against {of} of ADR 0815, largest relative difference {:.3e} (limit {REC_TOL:e})",
                r0(&r, of)
            );
        }
        if name == "A0" {
            let refs: Vec<&OuterProbe> = probes.iter().collect();
            let (w, n) = r1(&r, &refs);
            println!("F885-TM R1: probe against record at c{C_A}, {n} iterations, largest relative difference {w:.3e}");
        }
        runs.push((r, probes));
        write_records(&dir, &runs);
    }
    print_outcomes(&fan_outcomes(&runs));
    println!("F885-TM fan records written to {}", dir.display());
}

/// G2, R0 and R1 (ADR 0816): the outcomes from the committed records, held by `FAN_RECORD`.
#[test]
fn f885_tm_fan_from_the_committed_record() {
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
            "F885-TM {}: {} steps, failed {:?}, {} probe records",
            r.name,
            r.steps.len(),
            r.failed,
            p.len()
        );
    }
    let o = fan_outcomes(&runs);
    print_outcomes(&o);
    let (ra, rb) = (r0(&runs[0].0, "G0"), r0(&runs[1].0, "GL"));
    let refs: Vec<&OuterProbe> = runs[0].1.iter().collect();
    let (w1, n1) = r1(&runs[0].0, &refs);
    println!("F885-TM R0 {ra:.3e} (A0), {rb:.3e} (B1); R1 {w1:.3e} over {n1} iterations");
    let (want_a, want_b) =
        FAN_RECORD.expect("no recorded outcome yet: the registered fan run of ADR 0816 has not been recorded");
    assert!(ra <= REC_TOL && rb <= REC_TOL, "R0: {ra}, {rb}");
    assert!(w1 <= REC_TOL, "R1: {w1} over {n1}");
    let ((ha, q, oa), (rhos, rs, _, ob)) = o;
    let same = |a: f64, b: f64| (a.is_nan() && b.is_nan()) || rel(a, b) <= REC_TOL;
    assert_eq!(ha, want_a.0, "mode A held");
    assert!(same(q, want_a.1), "q {q} against {}", want_a.1);
    assert_eq!(oname(&oa), want_a.2);
    for i in 0..4 {
        assert!(
            same(rhos[i], want_b.0[i]) && same(rs[i], want_b.1[i]),
            "B{}: {} {}",
            i + 1,
            rhos[i],
            rs[i]
        );
    }
    assert_eq!(oname(&ob), want_b.2);
}
