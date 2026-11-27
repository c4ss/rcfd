//! ADR 0820 (F-885): the slow outer mode of the fan that remains with ADR 0819's kept limiter factor, by keeping the lagged
//! parts of the momentum predictor from the step's first outer iteration one at a time: the convecting flux (GF,
//! `outer_frozen_convecting_flux`), the convection's gradient (GD, `outer_frozen_convection_gradient`), the Laplacian's
//! gradient (GN, `outer_frozen_laplacian_gradient`) and the three with ADR 0815's freezes (GX), against F0 (F of ADR 0819
//! reproduced), on 20 steps of H(dt) with 20 outer iterations; the spectral radius of the outer map and the place of its
//! dominant vector from the field record (`outer_field_record`). On the cavity and its sheared form: what is kept, the frozen
//! map solving one problem, the record read-only, each opt-in read only where it acts, and the control. The case text is
//! copied unchanged from `f885_frozen_limiter.rs` (ADR 0819). The fan runs are a heavy record; the gate reads its committed
//! files.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::gradient::{least_squares, Limiter};
use rcfd_fv::pimple::{OuterField, PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use rcfd_mesh::Mesh;
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
/// ADR 0820 section 2: the factor at which 20 outer iterations meet the rule, `10^(-2/17)`.
const RHO_RULE: f64 = 0.7627;
/// ADR 0820 section 2 (ADR 0816 section 2): the band.
const BAND: f64 = 0.015;
/// ADR 0820 section 4: one dominant mode if `cos >= 0.99`.
const COS_ONE: f64 = 0.99;
/// The cavity: steps and time step (ADR 0767 section 1).
const CAV_STEPS: usize = 3;
const CAV_DT: f64 = 0.05;
/// The fan runs: steps, the first classed step, the outer iterations.
const FAN_STEPS: usize = 20;
const FAN_FROM: usize = 11;
const N_OUT: usize = 20;
/// The recorded outer iterations of the fan (`delta_16` to `delta_19`).
const FIELD_ITERS: [usize; 5] = [15, 16, 17, 18, 19];
/// R1, G2: relative tolerance against the printed digits of a record.
const REC_TOL: f64 = 1e-6;
/// T1: tolerance of two routes separated only by rounding; the floor of a change.
const ROUTE_TOL: f64 = 1e-12;
const MOVE_FLOOR: f64 = 1e-9;
/// L: the limit of `d_k / d_1`, and its non-vacuity floor.
const LIN_TOL: f64 = 1e-8;
const LIN_FLOOR: f64 = 1e-6;
/// The shear of the cavity `Sh`: `x += 0.3 y`.
const SHEAR: f64 = 0.3;
/// The classes of a cell, in their order of precedence (section 4).
const CLASSES: [&str; 7] = ["blade", "shroud", "hub", "coupled", "inlet", "outlet", "interior"];

/// The recorded outcomes of the registered fan run (written after the run): `rho` and `C` of F0, GF, GD, GN, GX; `rho2`
/// and `cos` of F0; the classes of GF, GD, GN, GX; result D.
type FanRecord = ([f64; 5], [f64; 5], f64, f64, [&'static str; 4], &'static str);
const FAN_RECORD: Option<FanRecord> = Some((
    [
        0.8344381950609923,
        0.7818107451098382,
        0.3586621403408897,
        0.833495900116017,
        0.21376833963263475,
    ],
    [
        0.014300805347754355,
        0.005406352141590268,
        1.1187101663942197e-08,
        0.009663068082988417,
        1.4062236549539693e-12,
    ],
    0.8334816951560352,
    0.9997542159884052,
    ["PART", "CARRIES", "NONE", "CARRIES"],
    "named HD",
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

// ---------------------------------------------------------------- the case of check H (copied from ADR 0815, through ADR 0819)

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

// ---------------------------------------------------------------- the measures (ADR 0820 section 4)

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

/// The class of a control against F0 (section 4).
fn control_class(r: f64, r0: f64) -> &'static str {
    if r.is_nan() || r0.is_nan() {
        "UNDECIDED"
    } else if r <= RHO_RULE {
        "CARRIES"
    } else if r - r0 < -BAND {
        "PART"
    } else if (r - r0).abs() <= BAND {
        "NONE"
    } else {
        "SLOWS"
    }
}

/// Result D (section 4) from `rho` of F0 and of GF, GD, GN, GX: the four classes and the result.
fn mode_d(r0: f64, r: [f64; 4]) -> ([&'static str; 4], String) {
    let cl = r.map(|x| control_class(x, r0));
    if cl.contains(&"UNDECIDED") {
        return (cl, "undecided".into());
    }
    let names = ["HF", "HD", "HN"];
    let s: Vec<&str> = (0..3).filter(|&i| cl[i] == "CARRIES").map(|i| names[i]).collect();
    let out = match s.len() {
        1 => format!("named {}", s[0]),
        0 if cl[3] == "CARRIES" => "named HX".into(),
        0 if cl.iter().all(|c| *c == "NONE" || *c == "SLOWS") => "HF to HX ruled out, HL by elimination".into(),
        0 => {
            let p: Vec<&str> = (0..4)
                .filter(|&i| cl[i] == "PART")
                .map(|i| ["HF", "HD", "HN", "HX"][i])
                .collect();
            format!("parts named {p:?}")
        }
        _ => format!("names the set {s:?}"),
    };
    (cl, out)
}

/// The class of each cell of the mesh (section 4): the first of `CLASSES` that one of its boundary faces belongs to.
fn cell_classes(g: &FvGeometry) -> Vec<usize> {
    let mesh = &g.mesh;
    let mut cls = vec![6usize; g.n_cells()];
    for (pi, p) in mesh.patches().iter().enumerate() {
        let c = match p.name.as_str() {
            "blade" => 0,
            "shroud" => 1,
            "hub" => 2,
            "inlet" => 4,
            "outlet" => 5,
            _ if g.periodic_partner[pi].is_some() => 3,
            _ => continue,
        };
        for f in p.faces() {
            let o = mesh.owner()[f];
            cls[o] = cls[o].min(c);
        }
    }
    cls
}

/// The measures of one step from its recorded velocities `u_15` to `u_19` (in that order): `n2(delta_k)` for `k` = 16 to
/// 19, the cosine of `delta_19` and `delta_18`, the share of `n2(delta_19)^2` per class, `n90`, and the cell of the largest
/// change of `delta_19`.
#[derive(Debug, Clone, PartialEq)]
struct StepFields {
    n2: [f64; 4],
    cos: f64,
    share: [f64; 7],
    n90: usize,
    cell: usize,
}

fn step_fields(us: &[&[Vec3]], cls: &[usize], active: [bool; 3]) -> StepFields {
    assert_eq!(us.len(), 5);
    let delta = |k: usize| -> Vec<f64> {
        us[k]
            .iter()
            .zip(us[k - 1])
            .flat_map(|(a, b)| (0..3).map(move |j| if active[j] { a[j] - b[j] } else { 0.0 }))
            .collect()
    };
    let ds: Vec<Vec<f64>> = (1..5).map(delta).collect();
    let n2 = [0, 1, 2, 3].map(|i| ds[i].iter().map(|x| x * x).sum::<f64>().sqrt());
    let dot: f64 = ds[3].iter().zip(&ds[2]).map(|(a, b)| a * b).sum();
    let cos = dot / (n2[3] * n2[2]);
    let per_cell: Vec<f64> = ds[3].chunks(3).map(|c| c.iter().map(|x| x * x).sum()).collect();
    let total: f64 = per_cell.iter().sum();
    let mut share = [0.0; 7];
    for (c, e) in per_cell.iter().enumerate() {
        share[cls[c]] += e / total;
    }
    let mut sorted = per_cell.clone();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let (mut acc, mut n90) = (0.0, 0usize);
    for e in &sorted {
        if acc >= 0.9 * total {
            break;
        }
        acc += e;
        n90 += 1;
    }
    let cell = (0..per_cell.len()).fold(0, |b, c| if per_cell[c] > per_cell[b] { c } else { b });
    StepFields {
        n2,
        cos,
        share,
        n90,
        cell,
    }
}

/// `rho2` and `cos` of a run's classed steps (NaN if there are none).
fn spectral(fs: &[(usize, StepFields)]) -> (f64, f64) {
    let classed: Vec<&StepFields> = fs.iter().filter(|(s, _)| *s >= FAN_FROM).map(|(_, f)| f).collect();
    (
        upper_median(classed.iter().map(|f| (f.n2[3] / f.n2[0]).powf(1.0 / 3.0)).collect()),
        upper_median(classed.iter().map(|f| f.cos).collect()),
    )
}

/// One dominant mode (section 4).
fn one_mode(rho2: f64, cos: f64, rho: f64) -> bool {
    cos >= COS_ONE && (rho2 - rho).abs() <= BAND
}

/// The runs `names` from a record of the form `run,step,k,d,cell`.
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

const F_HEADER: &str =
    "run,step,n16,n17,n18,n19,cos,s_blade,s_shroud,s_hub,s_coupled,s_inlet,s_outlet,s_interior,n90,cell,r,z";

/// The field rows of `fields.csv`, per run: `(step, StepFields)`.
fn read_fields(path: &Path, names: &[&str]) -> Vec<Vec<(usize, StepFields)>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = vec![Vec::new(); names.len()];
    for l in text.lines() {
        if l.starts_with('#') || l.starts_with("run") || l.is_empty() {
            continue;
        }
        let c: Vec<&str> = l.split(',').collect();
        assert_eq!(c.len(), 18, "{l}");
        let f = |i: usize| c[i].parse::<f64>().unwrap();
        let Some(i) = names.iter().position(|n| *n == c[0]) else {
            continue;
        };
        out[i].push((
            c[1].parse().unwrap(),
            StepFields {
                n2: [f(2), f(3), f(4), f(5)],
                cos: f(6),
                share: [f(7), f(8), f(9), f(10), f(11), f(12), f(13)],
                n90: c[14].parse().unwrap(),
                cell: c[15].parse().unwrap(),
            },
        ));
    }
    out
}

/// T3 (ADR 0820): the rules of section 4 on built records.
#[test]
fn f885_so_rules_on_built_records() {
    let r0 = 0.834;
    assert_eq!(
        (
            control_class(0.70, r0),
            control_class(0.80, r0),
            control_class(0.84, r0),
            control_class(0.86, r0),
            control_class(f64::NAN, r0)
        ),
        ("CARRIES", "PART", "NONE", "SLOWS", "UNDECIDED")
    );
    assert_eq!(mode_d(r0, [0.70, 0.83, 0.84, 0.60]).1, "named HF");
    assert_eq!(mode_d(r0, [0.83, 0.70, 0.84, 0.60]).1, "named HD");
    assert_eq!(mode_d(r0, [0.83, 0.84, 0.70, 0.60]).1, "named HN");
    assert_eq!(mode_d(r0, [0.80, 0.80, 0.84, 0.70]).1, "named HX");
    assert_eq!(
        mode_d(r0, [0.83, 0.84, 0.86, 0.835]).1,
        "HF to HX ruled out, HL by elimination"
    );
    assert_eq!(mode_d(r0, [0.80, 0.84, 0.84, 0.80]).1, "parts named [\"HF\", \"HX\"]");
    assert!(mode_d(r0, [0.70, 0.70, 0.84, 0.60]).1.starts_with("names the set"));
    assert_eq!(mode_d(r0, [0.70, f64::NAN, 0.84, 0.60]).1, "undecided");
    assert_eq!(
        (class(1e-3), class(0.1), class(f64::NAN)),
        ("CONVERGES", "STALLS", "UNDECIDED")
    );
    // the field measures on built fields: two cells, class 0 and 6; delta_k = 0.8^k (1, 2) at cell 0 and 0.1 0.8^k at cell 1
    let us: Vec<Vec<Vec3>> = (0..5)
        .map(|i| {
            let s: f64 = (1..=i).map(|k| 0.8f64.powi(k)).sum();
            vec![[s, 2.0 * s, 0.0], [0.1 * s, 0.0, 0.0]]
        })
        .collect();
    let refs: Vec<&[Vec3]> = us.iter().map(|v| v.as_slice()).collect();
    let f = step_fields(&refs, &[0, 6], [true, true, false]);
    assert!(((f.n2[3] / f.n2[0]).powf(1.0 / 3.0) - 0.8).abs() < 1e-12, "{f:?}");
    assert!((f.cos - 1.0).abs() < 1e-12);
    let e0 = 5.0;
    let e1 = 0.01;
    assert!((f.share[0] - e0 / (e0 + e1)).abs() < 1e-12 && (f.share[6] - e1 / (e0 + e1)).abs() < 1e-12);
    assert_eq!((f.n90, f.cell), (1, 0));
    // an alternating mode: cos -1
    let alt: Vec<Vec<Vec3>> = (0..5)
        .map(|i| vec![[if i % 2 == 0 { 0.0 } else { 1.0 }, 0.0, 0.0]])
        .collect();
    let refs: Vec<&[Vec3]> = alt.iter().map(|v| v.as_slice()).collect();
    assert!((step_fields(&refs, &[0], [true, true, false]).cos + 1.0).abs() < 1e-12);
    assert!(one_mode(0.83, 0.995, 0.834) && !one_mode(0.83, 0.9, 0.834) && !one_mode(0.80, 0.995, 0.834));
    // a run not complete has no factor
    let short = Run {
        name: "x".into(),
        steps: vec![(11, vec![1.0; N_OUT], vec![0; N_OUT])],
        planned: 2,
        failed: None,
    };
    assert!(rho(&short).is_nan() && c_of(&short).is_nan());
}

// ---------------------------------------------------------------- the cavity (ADR 0820 section 5)

/// The 8 x 8 lid-driven cavity of ADR 0767 from rest; `sheared`: every point moved by `x += 0.3 y` (`Sh`).
fn cavity(s: PimpleSettings, sheared: bool) -> PimpleSolver {
    let mut mesh = box_2d(8, 8, 1.0, 1.0, false, false).unwrap();
    if sheared {
        let mut topo = mesh.topology().clone();
        for p in topo.points.iter_mut() {
            p[0] += SHEAR * p[1];
        }
        mesh = Mesh::from_topology(topo).unwrap();
    }
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

/// KL of ADR 0816 (KC0 of ADR 0815 with the limiter: 20 outer iterations, BDF2, the record and the switch on), or KC0
/// without the limiter.
fn kl_settings(with_limiter: bool) -> PimpleSettings {
    let mut s = PimpleSettings::pimple(0.01, N_OUT);
    s.time_scheme = TimeScheme::Bdf2;
    s.outer_velocity_record = true;
    s.alpha_free_face_flux = true;
    if with_limiter {
        s.convection = ConvScheme::LinearUpwind;
        s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
    }
    s
}

/// The opt-ins GF, GD, GN set as given.
fn with_freezes(mut s: PimpleSettings, gf: bool, gd: bool, gn: bool) -> PimpleSettings {
    s.outer_frozen_convecting_flux = gf;
    s.outer_frozen_convection_gradient = gd;
    s.outer_frozen_laplacian_gradient = gn;
    s
}

fn run_cavity(s: PimpleSettings, sheared: bool, steps: usize) -> PimpleSolver {
    let mut sol = cavity(s, sheared);
    for _ in 0..steps {
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

/// `max abs(a - b) / max abs(b)` over the entries.
fn vdiff(a: &[Vec3], b: &[Vec3]) -> f64 {
    let num = a
        .iter()
        .zip(b)
        .flat_map(|(x, y)| (0..3).map(move |j| (x[j] - y[j]).abs()))
        .fold(0.0, f64::max);
    let den = b.iter().flat_map(|y| y.iter().map(|v| v.abs())).fold(0.0, f64::max);
    if num == 0.0 {
        0.0
    } else {
        num / den
    }
}

fn sdiff(a: &[f64], b: &[f64]) -> f64 {
    let num = a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    let den = b.iter().map(|v| v.abs()).fold(0.0, f64::max);
    if num == 0.0 {
        0.0
    } else {
        num / den
    }
}

/// The least-squares gradient of each component of the velocity `values` with the boundary values of `u`'s conditions
/// (`None` for an inactive component).
fn gradients(g: &FvGeometry, u: &VectorField, values: &[Vec3]) -> [Option<Vec<Vec3>>; 3] {
    let mut v = u.clone();
    v.values = values.to_vec();
    let ub = v.boundary_values(g);
    [0, 1, 2].map(|k| {
        g.active[k].then(|| {
            let cells: Vec<f64> = values.iter().map(|x| x[k]).collect();
            let bv: Vec<f64> = ub.iter().map(|x| x[k]).collect();
            least_squares(g, &cells, &bv)
        })
    })
}

/// T1 settings: `Sh`, `pimple(0.01, 5)` (implicit Euler), linear upwind with the kept limiter factor, the record at all
/// five iterations.
fn t1_settings(on: bool) -> PimpleSettings {
    let mut s = PimpleSettings::pimple(0.01, 5);
    s.convection = ConvScheme::LinearUpwind;
    s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
    s.outer_frozen_limiter = true;
    s.outer_field_record = (1..=5).collect();
    assert_eq!(s.time_scheme, TimeScheme::ImplicitEuler);
    with_freezes(s, on, on, on)
}

/// T1 (ADR 0820): what is kept, against the state before the step; the record's velocity; the non-vacuity and the record
/// of what was used with the opt-ins off.
#[test]
fn f885_so_two_routes() {
    let mut sol = cavity(t1_settings(true), true);
    let g = sol.geom.clone();
    let (mut worst, mut compared) = (0.0f64, 0usize);
    let mut miss: Vec<String> = Vec::new();
    for step in 1..=CAV_STEPS {
        let want = gradients(&g, &sol.u, &sol.u.values);
        let want_phi = sol.phi.clone();
        sol.step(CAV_DT);
        assert!(sol.diverged.is_none());
        let rec: Vec<&OuterField> = sol.outer_fields.iter().filter(|f| f.step == step).collect();
        assert_eq!(rec.len(), 5);
        for f in &rec {
            for k in 0..3 {
                for (what, got) in [
                    ("convection", &f.grad_convection[k]),
                    ("laplacian", &f.grad_laplacian[k]),
                ] {
                    match (got, &want[k]) {
                        (Some(a), Some(b)) => {
                            worst = worst.max(vdiff(a, b));
                            compared += 1;
                        }
                        (None, None) => {}
                        _ => miss.push(format!(
                            "step {step} iteration {} {what} {k}: formed on one route",
                            f.iteration
                        )),
                    }
                }
            }
            worst = worst.max(sdiff(&f.phi_conv, &want_phi));
            compared += 1;
        }
        let last = rec.iter().find(|f| f.iteration == 5).unwrap();
        let ub: Vec<u64> = last.u.iter().flat_map(|v| v.map(f64::to_bits)).collect();
        let sb: Vec<u64> = sol.u.values.iter().flat_map(|v| v.map(f64::to_bits)).collect();
        if ub != sb {
            miss.push(format!(
                "step {step}: the record's u of iteration 5 is not the velocity after the step"
            ));
        }
    }
    // the non-vacuity, and the record of what was used, with the opt-ins off
    let off = run_cavity(t1_settings(false), true, CAV_STEPS);
    let (mut moved_c, mut moved_l, mut moved_p, mut used) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for step in 1..=CAV_STEPS {
        let rec: Vec<&OuterField> = off.outer_fields.iter().filter(|f| f.step == step).collect();
        assert_eq!(rec.len(), 5);
        for i in 1..5 {
            let want = gradients(&g, &off.u, &rec[i - 1].u);
            for k in (0..3).filter(|&k| g.active[k]) {
                let (c, l) = (
                    rec[i].grad_convection[k].as_ref().unwrap(),
                    rec[i].grad_laplacian[k].as_ref().unwrap(),
                );
                let w = want[k].as_ref().unwrap();
                used = used.max(vdiff(c, w)).max(vdiff(l, w));
            }
        }
        if step >= 2 {
            for k in (0..3).filter(|&k| g.active[k]) {
                let (a, b) = (&rec[4], &rec[0]);
                moved_c = moved_c.max(vdiff(
                    a.grad_convection[k].as_ref().unwrap(),
                    b.grad_convection[k].as_ref().unwrap(),
                ));
                moved_l = moved_l.max(vdiff(
                    a.grad_laplacian[k].as_ref().unwrap(),
                    b.grad_laplacian[k].as_ref().unwrap(),
                ));
            }
            moved_p = moved_p.max(sdiff(&rec[4].phi_conv, &rec[0].phi_conv));
        }
    }
    println!(
        "F885-SO T1: {compared} kept quantities compared, largest relative difference {worst:.3e} (limit {ROUTE_TOL:e}); misses {miss:?}; off: the record against what was used {used:.3e}; changes of iteration 5 against 1 in steps 2 and 3: convection gradient {moved_c:.3e}, Laplacian gradient {moved_l:.3e}, flux {moved_p:.3e} (floor {MOVE_FLOOR:e})"
    );
    assert!(
        moved_c > MOVE_FLOOR && moved_l > MOVE_FLOOR && moved_p > MOVE_FLOOR,
        "T1 does not decide: {moved_c} {moved_l} {moved_p}"
    );
    assert!(
        worst <= ROUTE_TOL && used <= ROUTE_TOL && miss.is_empty(),
        "T1: {worst}, {used}, {miss:?}"
    );
}

/// L settings: `Sh`, implicit Euler, no relaxation, 60 correctors, tight solvers, 5 outer iterations, the record of `d_k`.
fn l_settings(gf: bool, gd: bool, gn: bool) -> PimpleSettings {
    let mut s = PimpleSettings::pimple(0.01, 5);
    s.convection = ConvScheme::LinearUpwind;
    s.linear_upwind_limiter = Some(Limiter::BarthJespersen);
    s.outer_frozen_limiter = true;
    s.alpha_u = 1.0;
    s.alpha_p = 1.0;
    s.n_correctors = 60;
    s.momentum_solver.tol_rel = 1e-12;
    s.pressure_solver.tol_rel = 1e-12;
    s.outer_velocity_record = true;
    with_freezes(s, gf, gd, gn)
}

/// The largest `d_k / d_1`, `k = 2..5`, over the steps; and the largest `d_2 / d_1`.
fn ratios(sol: &PimpleSolver) -> (f64, f64) {
    let mut all = 0.0f64;
    let mut second = 0.0f64;
    for r in &sol.outer_velocity {
        assert_eq!(r.change.len(), 5);
        for k in 1..5 {
            all = all.max(r.change[k] / r.change[0]);
        }
        second = second.max(r.change[1] / r.change[0]);
    }
    (all, second)
}

/// L (ADR 0820): with every lag of the predictor kept and no relaxation, the outer iterations solve one problem.
#[test]
fn f885_so_linear_map() {
    let on = run_cavity(l_settings(true, true, true), true, CAV_STEPS);
    let (worst, _) = ratios(&on);
    for r in &on.outer_velocity {
        let x: Vec<String> = r.change.iter().map(|a| format!("{a:.4e}")).collect();
        println!("F885-SO L step {}: d [{}]", r.step, x.join(", "));
    }
    let floors: Vec<f64> = [(false, true, true), (true, false, true), (true, true, false)]
        .iter()
        .map(|&(a, b, c)| ratios(&run_cavity(l_settings(a, b, c), true, CAV_STEPS)).1)
        .collect();
    println!(
        "F885-SO L: largest d_k / d_1 {worst:.3e} (limit {LIN_TOL:e}); d_2 / d_1 with GF, GD, GN off in turn {floors:?} (floor {LIN_FLOOR:e})"
    );
    assert!(floors.iter().all(|&f| f > LIN_FLOOR), "L does not decide: {floors:?}");
    assert!(worst <= LIN_TOL, "L: {worst}");
}

/// R0 and T2 (ADR 0820): the field record read-only; each opt-in read only where it acts.
#[test]
fn f885_so_read_only() {
    let mut rec = kl_settings(true);
    rec.outer_field_record = (1..=N_OUT).collect();
    let a = run_cavity(rec, false, CAV_STEPS);
    let b = run_cavity(kl_settings(true), false, CAV_STEPS);
    let r0 = bits(&a) == bits(&b);
    let n_rec = a.outer_fields.len();
    // GD and GN with upwind convection and no non-orthogonal correctors: no gradient formed
    let up_on = run_cavity(with_freezes(kl_settings(false), false, true, true), true, CAV_STEPS);
    let up_off = run_cavity(kl_settings(false), true, CAV_STEPS);
    let t2g = bits(&up_on) == bits(&up_off);
    // GF with one outer iteration
    let piso = |on: bool| {
        let mut s = PimpleSettings::piso(0.01);
        s.convection = ConvScheme::LinearUpwind;
        s.outer_velocity_record = true;
        with_freezes(s, on, false, false)
    };
    let t2f = bits(&run_cavity(piso(true), true, CAV_STEPS)) == bits(&run_cavity(piso(false), true, CAV_STEPS));
    println!(
        "F885-SO R0: KL with the field record against without: equal {r0} ({n_rec} iterations recorded); T2: GD and GN with upwind equal {t2g}; GF with one outer iteration equal {t2f}"
    );
    assert_eq!(n_rec, CAV_STEPS * N_OUT);
    assert!(r0, "R0 missed");
    assert!(t2g && t2f, "T2 missed");
}

/// K (ADR 0820): KL of ADR 0816 with the kept limiter factor and each opt-in, and the three, contracts.
#[test]
fn f885_so_cavity_control() {
    let mut miss = Vec::new();
    for (name, gf, gd, gn) in [
        ("GF", true, false, false),
        ("GD", false, true, false),
        ("GN", false, false, true),
        ("all", true, true, true),
    ] {
        let mut s = with_freezes(kl_settings(true), gf, gd, gn);
        s.outer_frozen_limiter = true;
        let sol = run_cavity(s, false, CAV_STEPS);
        let c = upper_median(sol.outer_velocity.iter().map(|r| r.change[18] / r.change[1]).collect());
        println!("F885-SO K {name}: C {c:.6e} ({}, limit {C_CONVERGES:e})", class(c));
        if !(c <= C_CONVERGES) {
            miss.push(format!("{name} {c}"));
        }
    }
    assert!(miss.is_empty(), "K: {miss:?}");
}

// ---------------------------------------------------------------- the fan runs (ADR 0820 section 4)

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-slow-outer-mode")
}

fn record_0819() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-frozen-limiter/fan_d.csv")
}

/// The runs in their order: name, GF, GD, GN, ADR 0815's two freezes.
const RUNS: [(&str, bool, bool, bool, bool); 5] = [
    ("F0", false, false, false, false),
    ("GF", true, false, false, false),
    ("GD", false, true, false, false),
    ("GN", false, false, true, false),
    ("GX", true, true, true, true),
];

/// One fan run: 20 steps of H(dt) as F of ADR 0819, with the run's opt-ins and the field record at iterations 15 to 19.
fn fan_run(name: &str, gf: bool, gd: bool, gn: bool, hook: bool) -> FanRun {
    let mut sim = sim_of(&h_text(H_DT));
    assert_eq!(sim.geom.mesh.n_cells(), 13627);
    let g = sim.geom.clone();
    let cls = cell_classes(&g);
    let centres = g.mesh.cell_centres().to_vec();
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
        s.alpha_free_face_flux = true;
        s.n_outer = N_OUT;
        s.outer_frozen_limiter = true;
        s.outer_frozen_convecting_flux = gf;
        s.outer_frozen_convection_gradient = gd;
        s.outer_frozen_laplacian_gradient = gn;
        s.outer_freeze_viscosity = hook;
        s.outer_freeze_source = hook;
        s.outer_field_record = FIELD_ITERS.to_vec();
    }
    let mut run = Run {
        name: name.into(),
        steps: Vec::new(),
        planned: FAN_STEPS,
        failed: None,
    };
    let mut fields = Vec::new();
    let mut places = Vec::new();
    let t0 = Instant::now();
    for k in 1..=FAN_STEPS {
        let st = sim.step();
        if let Some(f) = sim.failure() {
            run.failed = Some(format!("failure at step {k}: {f}"));
            break;
        }
        let ps = sim.pimple_solver_mut().expect("pimple");
        let rec: Vec<OuterField> = std::mem::take(&mut ps.outer_fields);
        let r = ps.outer_velocity.last().cloned();
        match r {
            Some(r) if r.step == k && r.change.len() == N_OUT && st.res_u.is_finite() => {
                if !r.change.iter().all(|d| d.is_finite()) {
                    run.failed = Some(format!("non-finite change at step {k}"));
                    break;
                }
                let x: Vec<String> = r.change.iter().map(|a| format!("{a:.4e}")).collect();
                println!(
                    "F885-SO {name} step {k}: d [{}] cells {:?}, {:.2} s per step",
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
        if k >= FAN_FROM {
            assert_eq!(rec.len(), FIELD_ITERS.len());
            let us: Vec<&[Vec3]> = rec.iter().map(|f| f.u.as_slice()).collect();
            let sf = step_fields(&us, &cls, g.active);
            let c = centres[sf.cell];
            println!(
                "F885-SO {name} step {k}: n2 {:?}, cos {:.6}, shares {:?}, n90 {}, cell c{} (r {:.4}, z {:.4})",
                sf.n2,
                sf.cos,
                sf.share,
                sf.n90,
                sf.cell,
                c[0].hypot(c[1]),
                c[2]
            );
            places.push((c[0].hypot(c[1]), c[2]));
            fields.push((k, sf));
        }
    }
    println!(
        "F885-SO {name} END: {} of {FAN_STEPS} steps, failed {:?}, {:.1} s",
        run.steps.len(),
        run.failed,
        t0.elapsed().as_secs_f64()
    );
    (run, fields, places)
}

const D_HEADER: &str = "run,step,k,d,cell";

type FanRun = (Run, Vec<(usize, StepFields)>, Vec<(f64, f64)>);

/// The records: `fan_d.csv` (every step) and `fields.csv` (the classed steps, values in round-trip form).
fn write_records(dir: &Path, runs: &[FanRun]) {
    let mut d = format!(
        "# ADR 0820 (F-885): the fan runs F0, GF, GD, GN, GX (20 outer iterations) on 20 steps of H(dt) (dt = {H_DT:?} s); d [m/s] the velocity change of outer iteration k, cell its cell\n{D_HEADER}\n"
    );
    let mut f = format!(
        "# ADR 0820 (F-885): the field record of the classed steps; n16..n19 the Euclidean norm of delta_k = u_k - u_(k-1) over the cells and active components, cos that of delta_19 and delta_18, s_* the share of n19^2 per class of cell, n90 the cells holding 90 % of it, cell the cell of its largest change with its radius r and z [m]\n{F_HEADER}\n"
    );
    for (r, fields, places) in runs {
        writeln!(d, "# {} steps {} failed {:?}", r.name, r.steps.len(), r.failed).unwrap();
        for (n, dk, c) in &r.steps {
            for k in 0..dk.len() {
                writeln!(d, "{},{n},{},{:.9e},{}", r.name, k + 1, dk[k], c[k]).unwrap();
            }
        }
        for ((n, sf), (rr, z)) in fields.iter().zip(places) {
            let sh: Vec<String> = sf.share.iter().map(|x| format!("{x:?}")).collect();
            writeln!(
                f,
                "{},{n},{:?},{:?},{:?},{:?},{:?},{},{},{},{rr:?},{z:?}",
                r.name,
                sf.n2[0],
                sf.n2[1],
                sf.n2[2],
                sf.n2[3],
                sf.cos,
                sh.join(","),
                sf.n90,
                sf.cell
            )
            .unwrap();
        }
    }
    std::fs::write(dir.join("fan_d.csv"), d).unwrap();
    std::fs::write(dir.join("fields.csv"), f).unwrap();
}

/// R1: the largest relative difference of every `d_k` of 20 steps against F of ADR 0819 (NaN if a step is missing).
fn against_0819(run: &Run) -> f64 {
    let theirs = &read_d(&record_0819(), &["F"])[0];
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
    rhos: [f64; 5],
    cs: [f64; 5],
    rho2: f64,
    cos: f64,
    classes: [&'static str; 4],
    result: String,
    r1: f64,
    medians: Vec<[f64; 7]>,
}

fn fan_outcomes(runs: &[(Run, Vec<(usize, StepFields)>)]) -> Outcomes {
    let rhos = [0, 1, 2, 3, 4].map(|i| rho(&runs[i].0));
    let cs = [0, 1, 2, 3, 4].map(|i| c_of(&runs[i].0));
    let (rho2, cos) = if runs[0].1.len() == FAN_STEPS - FAN_FROM + 1 {
        spectral(&runs[0].1)
    } else {
        (f64::NAN, f64::NAN)
    };
    let (classes, result) = mode_d(rhos[0], [rhos[1], rhos[2], rhos[3], rhos[4]]);
    let medians = runs
        .iter()
        .map(|(_, f)| [0, 1, 2, 3, 4, 5, 6].map(|j| upper_median(f.iter().map(|(_, s)| s.share[j]).collect())))
        .collect();
    Outcomes {
        rhos,
        cs,
        rho2,
        cos,
        classes,
        result,
        r1: against_0819(&runs[0].0),
        medians,
    }
}

fn print_outcomes(o: &Outcomes, runs: &[(Run, Vec<(usize, StepFields)>)]) {
    for i in 0..5 {
        let n90 = upper_median(runs[i].1.iter().map(|(_, s)| s.n90 as f64).collect());
        let (r2, cs) = spectral(&runs[i].1);
        println!(
            "F885-SO {}: rho {:.6}, C {:.6e} ({}), rho2 {r2:.6}, cos {cs:.6}, median shares {:?} ({CLASSES:?}), median n90 {n90}",
            RUNS[i].0,
            o.rhos[i],
            o.cs[i],
            class(o.cs[i]),
            o.medians[i]
        );
    }
    println!(
        "F885-SO SM: F0 rho2 {:.6}, cos {:.6}, one dominant mode {}",
        o.rho2,
        o.cos,
        one_mode(o.rho2, o.cos, o.rhos[0])
    );
    println!(
        "F885-SO FD: classes GF {} GD {} GN {} GX {}; result D {}",
        o.classes[0], o.classes[1], o.classes[2], o.classes[3], o.result
    );
    println!("F885-SO R1: F0 against F of ADR 0819 {:.3e} (limit {REC_TOL:e})", o.r1);
}

/// F0, GF, GD, GN, GX (ADR 0820): the heavy record of the fan runs, written to `F885_SO_OUT`.
#[test]
#[ignore = "heavy record, ADR 0820: F0, GF, GD, GN, GX on 20 steps of H(dt) on B2 (13 627 cells, 2 000 outer iterations, minutes): cargo test --release -p rcfd-api --test f885_slow_outer_mode f885_so_fan_record -- --ignored --nocapture"]
fn f885_so_fan_record() {
    let dir = std::env::var("F885_SO_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let mut runs: Vec<FanRun> = Vec::new();
    for (name, gf, gd, gn, hook) in RUNS {
        runs.push(fan_run(name, gf, gd, gn, hook));
        write_records(&dir, &runs);
    }
    let plain: Vec<(Run, Vec<(usize, StepFields)>)> = runs.into_iter().map(|(r, f, _)| (r, f)).collect();
    print_outcomes(&fan_outcomes(&plain), &plain);
    println!("F885-SO fan records written to {}", dir.display());
}

/// G2 (ADR 0820): the outcomes from the committed records, held by `FAN_RECORD`, and R1, FD, FC from them.
#[test]
fn f885_so_fan_from_the_committed_record() {
    let names: Vec<&str> = RUNS.iter().map(|r| r.0).collect();
    let d = read_d(&record_dir().join("fan_d.csv"), &names);
    let fields = read_fields(&record_dir().join("fields.csv"), &names);
    let runs: Vec<(Run, Vec<(usize, StepFields)>)> = d.into_iter().zip(fields).collect();
    for (r, f) in &runs {
        println!(
            "F885-SO {}: {} steps, failed {:?}, {} field rows",
            r.name,
            r.steps.len(),
            r.failed,
            f.len()
        );
    }
    let o = fan_outcomes(&runs);
    print_outcomes(&o, &runs);
    let want = FAN_RECORD.expect("no recorded outcome yet: the registered fan run of ADR 0820 has not been recorded");
    let same = |a: f64, b: f64| (a.is_nan() && b.is_nan()) || rel(a, b) <= REC_TOL;
    for i in 0..5 {
        assert!(same(o.rhos[i], want.0[i]), "{} rho {}", RUNS[i].0, o.rhos[i]);
        assert!(same(o.cs[i], want.1[i]), "{} C {}", RUNS[i].0, o.cs[i]);
    }
    assert!(
        same(o.rho2, want.2) && same(o.cos, want.3),
        "rho2 {} cos {}",
        o.rho2,
        o.cos
    );
    assert_eq!(o.classes, want.4);
    assert_eq!(o.result, want.5);
    let mut miss = Vec::new();
    if !(o.r1 <= REC_TOL) {
        miss.push(format!("R1 {}", o.r1));
    }
    if o.cs.iter().any(|c| class(*c) == "UNDECIDED") {
        miss.push("FC undecided".into());
    }
    let decided = o.result.starts_with("named HF")
        || o.result.starts_with("named HD")
        || o.result.starts_with("named HN")
        || o.result == "named HX"
        || o.result.starts_with("HF to HX ruled out");
    if !(runs[1..].iter().all(|r| r.0.complete()) && decided) {
        miss.push(format!("FD {}", o.result));
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}
