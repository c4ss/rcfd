//! ADR 0815 (F-885): why the fan's outer loop does not contract within a step. Controls of the relaxed outer loop on 20 steps
//! of H(dt) with `alpha_free_face_flux` (ADR 0811) and 20 outer iterations: the baseline G0 (F2s of ADR 0811), the face
//! viscosity of the step's first outer iteration kept (GT, the turbulence sweep), the hook's source kept (GS, the rotating
//! frame), no limiter (GL), tight linear solvers (GP), the implicit rotational seam (GR) and all five (GA); the cavity as a
//! control that contracts; the two opt-in freezes `outer_freeze_source` and `outer_freeze_viscosity` checked by two routes.
//! The case text is copied unchanged from `f885_alpha_free_face_flux.rs` (ADR 0811). The fan controls are a heavy record; the
//! gate reads its committed file.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{OuterHook, OuterUpdate, PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
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
/// ADR 0810 section 5 (ADR 0815 section 4): `C <= 1e-2` CONVERGES.
const C_CONVERGES: f64 = 1e-2;
/// The cavity: steps and time step (ADR 0767 section 1).
const CAV_STEPS: usize = 3;
const CAV_DT: f64 = 0.05;
/// The fan controls: steps, the first classed step, the outer iterations (ADR 0815 section 4).
const FAN_STEPS: usize = 20;
const FAN_FROM: usize = 11;
const N_OUT: usize = 20;
/// R0 and G2: relative tolerance against the 10 printed digits of a record.
const REC_TOL: f64 = 1e-6;
/// The test hook's rotation rate [1/s] (ADR 0815 section 4).
const OMEGA_TEST: f64 = 1.0;

/// One recorded outcome: the run, its class, its `C`.
type Entry = (&'static str, &'static str, f64);
/// G2: the fan controls' outcomes of the registered run, and the result (written after the run).
const FAN_RECORD: Option<(&[Entry], &str)> = Some((
    &[
        ("G0", "STALLS", 0.02580994273283583),
        ("GT", "STALLS", 0.026023832604940185),
        ("GS", "STALLS", 0.03307324754628127),
        ("GL", "STALLS", 0.017267008366235827),
        ("GP", "STALLS", 0.025809942568396187),
        ("GR", "STALLS", 0.025778076441339885),
        ("GA", "STALLS", 0.016801791555276994),
    ],
    "all ruled out",
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

/// Values in scientific notation, for the logs.
fn sci(v: &[f64]) -> String {
    let x: Vec<String> = v.iter().map(|a| format!("{a:.4e}")).collect();
    format!("[{}]", x.join(", "))
}

// ---------------------------------------------------------------- the case of check H (ADR 0766, copied from ADR 0811)

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

// ---------------------------------------------------------------- the measures (ADR 0810 section 5, ADR 0815 section 4)

/// One control run: per step the changes `d_k` and their cells.
#[derive(Debug, Clone)]
struct Run {
    name: String,
    n_outer: usize,
    steps: Vec<(usize, Vec<f64>, Vec<usize>)>,
    planned: usize,
    failed: Option<String>,
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

/// The class of `C`.
fn class_of(c: f64) -> Conv {
    if c <= C_CONVERGES {
        Conv::Converges
    } else if c > C_CONVERGES {
        Conv::Stalls
    } else {
        Conv::Undecided
    }
}

/// `C` = median of `d_19 / d_2` and its class.
fn conv(run: &Run, from: usize) -> (f64, Conv) {
    assert_eq!(run.n_outer, N_OUT);
    let c = upper_median(classed(run, from).iter().map(|d| d[18] / d[1]).collect());
    (c, class_of(c))
}

fn cname(c: Conv) -> &'static str {
    match c {
        Conv::Converges => "CONVERGES",
        Conv::Stalls => "STALLS",
        Conv::Undecided => "UNDECIDED",
    }
}

/// The result of section 4.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Named(&'static str),
    AllRuledOut,
    NamesNone(String),
}

/// The five single controls and their hypotheses.
const SINGLES: [(&str, &str); 5] = [("GT", "HT"), ("GS", "HS"), ("GL", "HL"), ("GP", "HP"), ("GR", "HR")];

/// The rule of section 4 from the classes of the controls (a missing control is UNDECIDED).
fn result(classes: &[(&str, Conv)]) -> Outcome {
    let get = |n: &str| classes.iter().find(|c| c.0 == n).map_or(Conv::Undecided, |c| c.1);
    if let Some((g, _)) = SINGLES.iter().find(|(g, _)| get(g) == Conv::Undecided) {
        return Outcome::NamesNone(format!("{g} UNDECIDED"));
    }
    let s: Vec<&'static str> = SINGLES
        .iter()
        .filter(|(g, _)| get(g) == Conv::Converges)
        .map(|(_, h)| *h)
        .collect();
    match s.len() {
        1 => Outcome::Named(s[0]),
        0 if get("GA") == Conv::Stalls => Outcome::AllRuledOut,
        0 => Outcome::NamesNone(format!("no single control converges, GA {}", cname(get("GA")))),
        _ => Outcome::NamesNone(format!("{} single controls converge: {s:?}", s.len())),
    }
}

fn oname(o: &Outcome) -> String {
    match o {
        Outcome::Named(h) => format!("named {h}"),
        Outcome::AllRuledOut => "all ruled out".into(),
        Outcome::NamesNone(why) => format!("names none ({why})"),
    }
}

/// T4 (ADR 0815): the class of `C` and the rule of section 4 on built records.
#[test]
fn f885_os_rules_on_built_records() {
    assert_eq!(class_of(1e-3), Conv::Converges);
    assert_eq!(class_of(0.1), Conv::Stalls);
    assert_eq!(class_of(f64::NAN), Conv::Undecided);
    let (c, s, u) = (Conv::Converges, Conv::Stalls, Conv::Undecided);
    let set = |gt, gs, gl, gp, gr, ga| vec![("GT", gt), ("GS", gs), ("GL", gl), ("GP", gp), ("GR", gr), ("GA", ga)];
    assert_eq!(result(&set(c, s, s, s, s, c)), Outcome::Named("HT"));
    assert_eq!(result(&set(s, s, c, s, s, c)), Outcome::Named("HL"));
    assert_eq!(result(&set(s, s, s, s, c, s)), Outcome::Named("HR"));
    assert_eq!(result(&set(s, s, s, s, s, s)), Outcome::AllRuledOut);
    assert!(matches!(result(&set(s, s, s, s, s, c)), Outcome::NamesNone(_)));
    assert!(matches!(result(&set(s, s, s, s, s, u)), Outcome::NamesNone(_)));
    assert!(matches!(result(&set(c, c, s, s, s, c)), Outcome::NamesNone(_)));
    assert!(matches!(result(&set(c, u, s, s, s, c)), Outcome::NamesNone(_)));
    assert!(matches!(result(&set(s, s, s, s, s, s)[..5]), Outcome::NamesNone(_)));
    // a run that did not complete is UNDECIDED; a complete one is classed by d_19 / d_2
    let short = Run {
        name: "x".into(),
        n_outer: N_OUT,
        steps: vec![(1, vec![1.0; N_OUT], vec![0; N_OUT])],
        planned: 2,
        failed: None,
    };
    assert_eq!(conv(&short, 1).1, Conv::Undecided);
    let mut d = vec![1.0; N_OUT];
    d[18] = 5e-3;
    let full = Run {
        steps: vec![(1, d, vec![0; N_OUT])],
        planned: 1,
        ..short
    };
    assert_eq!(conv(&full, 1), (5e-3, Conv::Converges));
}

// ---------------------------------------------------------------- the cavity (ADR 0815 section 4)

/// The 8 x 8 lid-driven cavity of ADR 0767 from rest, the record and `alpha_free_face_flux` on, the freezes as given.
fn cavity(n_outer: usize, freeze_src: bool, freeze_nu: bool) -> PimpleSolver {
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
    s.alpha_free_face_flux = true;
    s.outer_freeze_source = freeze_src;
    s.outer_freeze_viscosity = freeze_nu;
    PimpleSolver::new(geom, u, p, s)
}

/// The test hook's values for the iterate `u`: the source `-2 Omega x u` (`Omega = (0, 0, OMEGA_TEST)`; face values
/// `a_f . S_f`, `a_f` the mean of the two cells, the owner value on boundary faces) and the face viscosity
/// `nu (1 + 0.5 |u_o|)`.
fn hook_value(g: &FvGeometry, u: &VectorField, nu: f64) -> OuterUpdate {
    let mesh = &g.mesh;
    let cell: Vec<Vec3> = u
        .values
        .iter()
        .map(|v| [2.0 * OMEGA_TEST * v[1], -2.0 * OMEGA_TEST * v[0], 0.0])
        .collect();
    let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
    let dot = |a: Vec3, s: Vec3| a[0] * s[0] + a[1] * s[1] + a[2] * s[2];
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            let a = if f < g.n_internal {
                let (x, y) = (cell[own[f]], cell[nei[f]]);
                [0.5 * (x[0] + y[0]), 0.5 * (x[1] + y[1]), 0.5 * (x[2] + y[2])]
            } else {
                cell[own[f]]
            };
            dot(a, sf[f])
        })
        .collect();
    let nu_face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            let v = u.values[own[f]];
            nu * (1.0 + 0.5 * (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        })
        .collect();
    OuterUpdate {
        source_force: Some(BodyForce { cell, face }),
        nu_face: Some(nu_face),
        ..Default::default()
    }
}

/// How the cavity is driven: no hook, the hook as it is, or the hook returning its first value of each step for the source
/// (`src`) and/or the viscosity (`nu`).
#[derive(Clone, Copy)]
enum Hook {
    None,
    Live,
    Cached { src: bool, nu: bool },
}

/// A cavity run of `CAV_STEPS` steps.
fn cavity_run(n_outer: usize, freeze_src: bool, freeze_nu: bool, mode: Hook) -> PimpleSolver {
    let mut sol = cavity(n_outer, freeze_src, freeze_nu);
    let g = sol.geom.clone();
    for _ in 0..CAV_STEPS {
        match mode {
            Hook::None => {
                sol.step(CAV_DT);
            }
            Hook::Live => {
                let mut h = |u: &VectorField, _: &[f64]| hook_value(&g, u, 0.01);
                let hp: &mut OuterHook = &mut h;
                sol.step_with(CAV_DT, Some(hp));
            }
            Hook::Cached { src, nu } => {
                let mut first: Option<OuterUpdate> = None;
                let mut h = |u: &VectorField, _: &[f64]| {
                    let now = hook_value(&g, u, 0.01);
                    match &first {
                        None => {
                            first = Some(now.clone());
                            now
                        }
                        Some(f) => OuterUpdate {
                            source_force: if src { f.source_force.clone() } else { now.source_force },
                            nu_face: if nu { f.nu_face.clone() } else { now.nu_face },
                            ..Default::default()
                        },
                    }
                };
                let hp: &mut OuterHook = &mut h;
                sol.step_with(CAV_DT, Some(hp));
            }
        }
        assert!(sol.diverged.is_none(), "the cavity stopped");
    }
    sol
}

type Bits = (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>);

/// The bits of `u`, `p`, the flux and the record.
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

/// T1 and T2 (ADR 0815): each freeze against a hook that returns its first value of the step (equal bits), and against no
/// freeze with the hook as it is (a different flux).
#[test]
fn f885_os_freeze_two_routes() {
    let off = cavity_run(5, false, false, Hook::Live);
    let mut miss = Vec::new();
    for (name, fs, fnu) in [("source", true, false), ("viscosity", false, true)] {
        let a = cavity_run(5, fs, fnu, Hook::Live);
        let b = cavity_run(5, false, false, Hook::Cached { src: fs, nu: fnu });
        let (ba, bb) = (bits(&a), bits(&b));
        let dphi = a.phi.iter().zip(&off.phi).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
        println!(
            "F885-OS T1 {name}: u equal {}, p equal {}, flux equal {}; T2: largest flux difference to no freeze {dphi:.3e}",
            ba.0 == bb.0,
            ba.1 == bb.1,
            ba.2 == bb.2
        );
        if (ba.0, ba.1, ba.2) != (bb.0, bb.1, bb.2) {
            miss.push(format!("T1 {name}"));
        }
        if !(dphi > 0.0) {
            miss.push(format!("T2 {name}: {dphi}"));
        }
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}

/// T3 (ADR 0815): the freezes on without a hook give the bits of the freezes off.
#[test]
fn f885_os_freeze_without_hook() {
    let (on, off) = (cavity_run(5, true, true, Hook::None), cavity_run(5, false, false, Hook::None));
    let (a, b) = (bits(&on), bits(&off));
    println!(
        "F885-OS T3: u equal {}, p equal {}, flux equal {}, record equal {}",
        a.0 == b.0,
        a.1 == b.1,
        a.2 == b.2,
        a.3 == b.3
    );
    assert!(a == b, "T3: the freezes changed a run without a hook");
}

/// The record of a cavity run as a `Run`.
fn as_run(name: &str, s: &PimpleSolver, n_outer: usize) -> Run {
    Run {
        name: name.into(),
        n_outer,
        steps: s
            .outer_velocity
            .iter()
            .map(|r| (r.step, r.change.clone(), r.cell.clone()))
            .collect(),
        planned: CAV_STEPS,
        failed: None,
    }
}

/// K1 (ADR 0815): the cavity contracts with 20 outer iterations: without a hook (KC0), with the test hook (KCh) and with
/// the test hook and both freezes (KCf).
#[test]
fn f885_os_cavity_contracts() {
    let mut miss = Vec::new();
    for (name, fz, mode) in [("KC0", false, Hook::None), ("KCh", false, Hook::Live), ("KCf", true, Hook::Live)] {
        let s = cavity_run(N_OUT, fz, fz, mode);
        let r = as_run(name, &s, N_OUT);
        for (n, d, _) in &r.steps {
            println!("F885-OS {name} step {n}: d {}", sci(d));
        }
        let (c, k) = conv(&r, 1);
        println!("F885-OS {name}: C {c:.6e} {}", cname(k));
        if k != Conv::Converges {
            miss.push(format!("{name}: C {c}"));
        }
    }
    assert!(miss.is_empty(), "missed: {miss:?}");
}

// ---------------------------------------------------------------- the fan controls (ADR 0815 section 4)

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-outer-stall")
}

fn record_0811() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-alpha-free-face-flux/fan_controls.csv")
}

fn g0(_: &mut PimpleSettings) {}
fn gt(s: &mut PimpleSettings) {
    s.outer_freeze_viscosity = true;
}
fn gs(s: &mut PimpleSettings) {
    s.outer_freeze_source = true;
}
fn gl(s: &mut PimpleSettings) {
    s.linear_upwind_limiter = None;
}
fn gp(s: &mut PimpleSettings) {
    s.momentum_solver.tol_rel = 1e-12;
    s.momentum_solver.max_iter = 5000;
    s.pressure_solver.tol_rel = 1e-11;
    s.pressure_solver.max_iter = 20000;
}
fn gr(s: &mut PimpleSettings) {
    s.rotational_periodic_implicit = true;
}
fn ga(s: &mut PimpleSettings) {
    gt(s);
    gs(s);
    gl(s);
    gp(s);
    gr(s);
}

/// A fan control: its name and its settings.
type Control = (&'static str, fn(&mut PimpleSettings));
/// The fan controls in their order (ADR 0815 section 4).
const CONTROLS: [Control; 7] = [
    ("G0", g0),
    ("GT", gt),
    ("GS", gs),
    ("GL", gl),
    ("GP", gp),
    ("GR", gr),
    ("GA", ga),
];

/// One fan control: 20 steps of H(dt), both records of ADR 0767 E-2, the switch of ADR 0811 on, 20 outer iterations, then
/// the control's settings; a failure or a non-finite value ends the run.
fn fan_run(name: &str, set: fn(&mut PimpleSettings)) -> (Run, Simulation) {
    let mut sim = sim_of(&h_text(H_DT));
    assert_eq!(sim.geom.mesh.n_cells(), 13627);
    {
        let s = &mut sim.pimple_solver_mut().expect("pimple").settings;
        assert_eq!((s.alpha_u, s.alpha_p, s.n_outer), (0.9, 1.0, 5), "the case's settings");
        assert!(s.linear_upwind_limiter.is_some() && !s.rotational_periodic_implicit);
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        s.alpha_free_face_flux = true;
        s.n_outer = N_OUT;
        set(s);
    }
    let mut run = Run {
        name: name.into(),
        n_outer: N_OUT,
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
        let ps = sim.pimple_solver();
        let rec = ps.and_then(|s| s.outer_velocity.last()).cloned();
        let p_iters = ps.and_then(|s| s.history.last()).map_or(0, |h| h.p_iters);
        match rec {
            Some(r) if r.step == k && r.change.len() == N_OUT && st.res_u.is_finite() => {
                if !r.change.iter().all(|d| d.is_finite()) {
                    run.failed = Some(format!("non-finite change at step {k}"));
                    break;
                }
                println!(
                    "F885-OS {name} step {k}: d {} cells {:?}, pressure iterations {p_iters}, {:.2} s per step",
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
        "F885-OS {name} END: {} of {FAN_STEPS} steps, failed {:?}, {:.1} s",
        run.steps.len(),
        run.failed,
        t0.elapsed().as_secs_f64()
    );
    (run, sim)
}

const FAN_HEADER: &str = "run,step,k,d,cell";

/// The fan record: one row per run, step and outer iteration; a comment line per run with its stop (if any).
fn write_fan(path: &Path, runs: &[Run]) {
    let mut out = format!(
        "# ADR 0815 (F-885): the fan controls G0 to GA (alpha_free_face_flux on, 20 outer iterations) on 20 steps of H(dt) (dt = {H_DT:?} s); d [m/s] the velocity change of outer iteration k, cell its cell\n{FAN_HEADER}\n"
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

/// The runs `names` (name, outer iterations) from a record of this form (ADR 0811's and this one's).
fn read_fan(path: &Path, names: &[(&str, usize)]) -> Vec<Run> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut runs: Vec<Run> = names
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

/// The outcomes of the fan controls: per run its class and `C`, and the result of section 4.
fn fan_outcomes(runs: &[Run]) -> (Vec<(String, &'static str, f64)>, Outcome) {
    let mut out = Vec::new();
    let mut classes = Vec::new();
    for (r, (name, _)) in runs.iter().zip(CONTROLS) {
        assert_eq!(r.name, name);
        let (c, k) = conv(r, FAN_FROM);
        out.push((r.name.clone(), cname(k), c));
        classes.push((name, k));
    }
    (out, result(&classes))
}

/// R0: the largest relative difference of G0 against F2s of ADR 0811, every `d_k` of the 20 steps (NaN if a step is missing).
fn r0(g0: &Run) -> f64 {
    let f2s = &read_fan(&record_0811(), &[("F0s", 5), ("F2s", 20)])[1];
    if g0.steps.len() != FAN_STEPS || f2s.steps.len() != FAN_STEPS {
        return f64::NAN;
    }
    let mut worst = 0.0f64;
    for ((na, da, _), (nb, db, _)) in g0.steps.iter().zip(&f2s.steps) {
        assert_eq!(na, nb);
        for (a, b) in da.iter().zip(db) {
            worst = worst.max(rel(*a, *b));
        }
    }
    worst
}

/// G0 to GA (ADR 0815): the heavy record of the fan controls, written to `F885_OS_OUT`.
#[test]
#[ignore = "heavy record, ADR 0815: G0 to GA on 20 steps of H(dt) on B2 (13 627 cells, 2 800 outer iterations, minutes): cargo test --release -p rcfd-api --test f885_outer_stall f885_os_fan_controls_record -- --ignored --nocapture"]
fn f885_os_fan_controls_record() {
    let dir = std::env::var("F885_OS_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    let mut runs = Vec::new();
    for (name, set) in CONTROLS {
        let (r, sim) = fan_run(name, set);
        let mesh = &sim.geom.mesh;
        let cc = mesh.cell_centres();
        for (n, d, c) in r.steps.iter().filter(|s| s.0 >= FAN_FROM) {
            let cell = c[18];
            let x = cc[cell];
            let patches: Vec<&str> = mesh
                .patches()
                .iter()
                .filter(|p| p.faces().any(|f| mesh.owner()[f] == cell))
                .map(|p| p.name.as_str())
                .collect();
            println!(
                "F885-OS {name} step {n}: d_19 {:.4e} at c{cell} (r {:.4}, z {:.4}), patches {patches:?}; d_20 {:.4e} at c{}",
                d[18],
                x[0].hypot(x[1]),
                x[2],
                d[19],
                c[19]
            );
        }
        let (cv, k) = conv(&r, FAN_FROM);
        let d19 = upper_median(classed(&r, FAN_FROM).iter().map(|d| d[18]).collect());
        println!("F885-OS {name}: C {cv:.6e} {}, median d_19 {d19:.6e} m/s", cname(k));
        if name == "G0" {
            println!(
                "F885-OS R0: G0 against F2s of ADR 0811, largest relative difference {:.3e} (limit {REC_TOL:e})",
                r0(&r)
            );
        }
        runs.push(r);
        // the record so far, so that a stop of a later run leaves the earlier ones
        write_fan(&dir.join("fan_controls.csv"), &runs);
    }
    let (out, res) = fan_outcomes(&runs);
    println!("F885-OS FAN outcomes {out:?}; result: {}", oname(&res));
    println!("F885-OS fan record written to {}", dir.display());
}

/// G2 and R0 (ADR 0815): the fan controls' outcomes from the committed record, held by `FAN_RECORD`; G0 against F2s of ADR
/// 0811.
#[test]
fn f885_os_fan_from_the_committed_record() {
    let names: Vec<(&str, usize)> = CONTROLS.iter().map(|(n, _)| (*n, N_OUT)).collect();
    let runs = read_fan(&record_dir().join("fan_controls.csv"), &names);
    for r in &runs {
        println!("F885-OS {}: {} steps, failed {:?}", r.name, r.steps.len(), r.failed);
    }
    let (out, res) = fan_outcomes(&runs);
    let d = r0(&runs[0]);
    println!("F885-OS FAN outcomes {out:?}; result: {}; R0 {d:.3e}", oname(&res));
    let (want, want_res) =
        FAN_RECORD.expect("no recorded outcome yet: the registered fan run of ADR 0815 has not been recorded");
    assert!(d <= REC_TOL, "R0: {d}");
    assert!(
        runs.iter().all(|r| r.failed.is_some() || r.steps.len() == FAN_STEPS),
        "every run complete or stopped"
    );
    assert_eq!(out.len(), want.len(), "{out:?} against {want:?}");
    for (g, w) in out.iter().zip(want) {
        let same = (g.2.is_nan() && w.2.is_nan()) || rel(g.2, w.2) <= REC_TOL;
        assert!(g.0 == w.0 && g.1 == w.1 && same, "{g:?} against {w:?}");
    }
    assert_eq!(oname(&res), want_res);
}
