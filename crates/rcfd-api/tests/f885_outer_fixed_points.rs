//! ADR 0810 (F-885): why the relaxed and the unrelaxed outer iterations of a pimple step have different fixed points. The
//! record of ADR 0767 (`PimpleSettings::outer_velocity_record`) now keeps the cell of each outer iteration's largest change; it
//! is shown equal to a second route on the cavity of ADR 0767. The controls of ADR 0810 section 5 (the relaxation factors, the
//! outer count, the transient Rhie-Chow term, the component diagonal) are run on that cavity and on 20 steps of H(dt) of check H
//! (FAN-01, B2), and classed by the measures and the table of hypotheses registered there. The fan controls are a heavy record;
//! the gate reads its committed file.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{OuterHook, OuterUpdate, PimpleSettings, PimpleSolver};
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
/// ADR 0810 section 5: steps of the fan controls, and the first step of the steps they are classed over.
const FAN_STEPS: usize = 20;
const FAN_FROM: usize = 11;
/// The cavity controls: steps and time step (ADR 0767 section 1).
const CAV_STEPS: usize = 3;
const CAV_DT: f64 = 0.05;

/// ADR 0810 section 5: the measures' decision numbers.
const J_PRESENT: f64 = 4.0;
const J_ABSENT: f64 = 2.0;
const C_CONVERGES: f64 = 1e-2;
const L_BAND: (f64, f64) = (0.4, 0.6);

/// C0c: ADR 0767 A2, step 1 of the cavity, and the tolerance of its 5 printed digits.
const A2_STEP1: [f64; 5] = [3.4197e-2, 7.1717e-3, 1.5369e-3, 3.3740e-4, 1.4550e-3];
const A2_TOL: f64 = 1e-4;
/// C0f: ADR 0767 S2, step 1 of H(dt), and the tolerance of its 4 printed digits.
const S2_STEP1: [f64; 5] = [36.09, 26.05, 4.316, 1.340, 0.916];
const S2_TOL: f64 = 1e-3;

/// One recorded outcome: the run, its class, its statistic (`J`, or `C` for a `:C` entry, `L` for `L`).
type Entry = (&'static str, &'static str, f64);

/// The cavity's outcomes of the registered run of ADR 0810, and the rows of the table that hold (written after the run).
const CAVITY_RECORD: Option<(&[Entry], &str)> = Some((
    &[
        ("K0", "PRESENT", 9.825472184802878),
        ("K1", "ABSENT", 0.07409914988148718),
        ("K2", "PRESENT", 9.845220450795065),
        ("K3", "ABSENT", 0.4874011035830805),
        ("K4", "PRESENT", 13289364.866419517),
        ("K5", "PRESENT", 6.86305979852826),
        ("K4:C", "CONVERGES", 4.777392522375897e-8),
        ("L", "IN", 0.4869685505487506),
    ],
    "H1",
));
/// The fan's outcomes of the registered run, the rows that hold, and the result (written after the run).
const FAN_RECORD: Option<(&[Entry], &str, &str)> = Some((
    &[
        ("F0", "PRESENT", 34.784013699672094),
        ("F1", "ABSENT", 0.9338187940214693),
        ("F2", "PRESENT", 105.25470460252978),
        ("F3", "PRESENT", 34.78401369965596),
        ("F4", "PRESENT", 9.79140090285106),
        ("F2:C", "STALLS", 0.03662076573314066),
    ],
    "H1",
    "named H1",
));

// ---------------------------------------------------------------- the measures (ADR 0810 section 5)

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

/// For `N = 20`: `Q` = median of `d_20 / d_1` (reported).
fn q_rule(run: &Run, from: usize) -> f64 {
    upper_median(classed(run, from).iter().map(|d| d[19] / d[0]).collect())
}

/// `L` = median over the steps of `d_20(b) / d_20(a)`.
fn size_ratio(a: &Run, b: &Run) -> f64 {
    let (sa, sb) = (classed(a, 1), classed(b, 1));
    if sa.len() != sb.len() || sa.is_empty() {
        return f64::NAN;
    }
    upper_median(sa.iter().zip(&sb).map(|(x, y)| y[19] / x[19]).collect())
}

fn in_band(l: f64) -> bool {
    l >= L_BAND.0 && l <= L_BAND.1
}

/// The cavity's outcomes (ADR 0810 section 5).
#[derive(Debug, Clone, Copy, PartialEq)]
struct CavityOut {
    k: [Jump; 6], // K0 to K5
    k4c: Conv,
    l_in: bool,
}

/// The fan's outcomes.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FanOut {
    f: [Jump; 5], // F0 to F4
    f2c: Conv,
}

use Jump::{Absent as A, Present as P};

/// The rows of the table that hold on the cavity, among H1, H2, Hc (H6 is zero there), and whether H4 holds.
fn cavity_rows(o: &CavityOut) -> (Vec<&'static str>, bool) {
    let k = o.k;
    let mut r = Vec::new();
    if k[0] == P && k[1] == A && k[2] == P && k[3] == A && o.k4c == Conv::Converges && k[4] == P && o.l_in {
        r.push("H1");
    }
    if k[0] == P && k[1] == A && k[2] == A && k[3] == P {
        r.push("H2");
    }
    if k[0] == P && k[1] == P {
        r.push("Hc");
    }
    (r, o.k4c == Conv::Stalls)
}

/// The rows that hold on the fan, among H1, H6, Hc (H2 is not tested there), and whether H4 holds.
fn fan_rows(o: &FanOut) -> (Vec<&'static str>, bool) {
    let f = o.f;
    let mut r = Vec::new();
    if f[0] == P && f[1] == A && f[2] == P && f[3] == P {
        r.push("H1");
    }
    if f[0] == P && f[1] == A && f[3] == A {
        r.push("H6");
    }
    if f[0] == P && f[1] == P {
        r.push("Hc");
    }
    (r, o.f2c == Conv::Stalls)
}

/// The result of ADR 0810 section 5: "named H..", "all ruled out" or "names none".
fn result(c: &CavityOut, f: &FanOut) -> String {
    if c.k[0] != P || f.f[0] != P {
        return "names none".into();
    }
    let (cr, _) = cavity_rows(c);
    let (fr, _) = fan_rows(f);
    if cr.is_empty() && fr.is_empty() {
        return "all ruled out".into();
    }
    // a hypothesis holds where it is tested: H1 and Hc on both cases, H2 on the cavity, H6 on the fan
    let holds = |h: &str| match h {
        "H2" => cr.contains(&h),
        "H6" => fr.contains(&h),
        _ => cr.contains(&h) && fr.contains(&h),
    };
    let cand: Vec<&str> = ["H1", "H2", "H6", "Hc"].into_iter().filter(|h| holds(h)).collect();
    let mut any: Vec<&str> = cr.iter().chain(&fr).copied().collect();
    any.sort();
    any.dedup();
    if cand.len() == 1 && any == cand {
        format!("named {}", cand[0])
    } else {
        "names none".into()
    }
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

fn sci(v: &[f64]) -> String {
    let x: Vec<String> = v.iter().map(|a| format!("{a:.4e}")).collect();
    format!("[{}]", x.join(", "))
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// Compare outcomes with a record: the same runs in the same order, the same classes, the statistic within 1e-6 relative.
fn check_record(got: &[(String, &'static str, f64)], want: &[Entry]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        let same = (g.2.is_nan() && w.2.is_nan()) || rel(g.2, w.2) <= 1e-6;
        assert!(g.0 == w.0 && g.1 == w.1 && same, "{g:?} against {w:?}");
    }
}

// ---------------------------------------------------------------- the cavity (ADR 0767 section 1)

/// The 8 x 8 lid-driven cavity of ADR 0767 from rest, the record on, with the settings changed by `set`.
fn cavity(n_outer: usize, record: bool, set: impl Fn(&mut PimpleSettings)) -> PimpleSolver {
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
    s.outer_velocity_record = record;
    set(&mut s);
    PimpleSolver::new(geom, u, p, s)
}

/// Route B: the largest Euclidean norm over the cells of `b - a` and its first cell, formed in the test.
fn change_at(a: &[Vec3], b: &[Vec3]) -> (f64, usize) {
    let mut best = (0.0f64, 0usize);
    for (c, (x, y)) in a.iter().zip(b).enumerate() {
        let (dx, dy, dz) = (y[0] - x[0], y[1] - x[1], y[2] - x[2]);
        let v = (dx * dx + dy * dy + dz * dz).sqrt();
        if v > best.0 {
            best = (v, c);
        }
    }
    best
}

/// One step with a hook that copies the iterate it is handed at the start of every outer iteration.
fn step_copying(sol: &mut PimpleSolver, dt: f64) -> Vec<Vec<Vec3>> {
    let mut copies: Vec<Vec<Vec3>> = Vec::new();
    {
        let mut hook = |u: &VectorField, _phi: &[f64]| {
            copies.push(u.values.clone());
            OuterUpdate::default()
        };
        let h: &mut OuterHook = &mut hook;
        sol.step_with(dt, Some(h));
    }
    copies
}

/// The baseline settings of the cavity (K0): those of `PimpleSettings::pimple`, `alpha_u = 0.8`, `alpha_p = 0.5`.
fn k0(_: &mut PimpleSettings) {}

/// P1 and P2 (ADR 0810): the recorded cell is the cell of the largest change by a second route; the record off keeps nothing
/// and gives the same bits.
#[test]
fn f885_fp_cell_two_routes() {
    let mut sol = cavity(5, true, k0);
    for n in 1..=CAV_STEPS {
        let mut v = step_copying(&mut sol, CAV_DT);
        assert_eq!(v.len(), 5);
        v.push(sol.u.values.clone());
        let rec = sol.outer_velocity.last().expect("a record").clone();
        assert_eq!((rec.step, rec.change.len(), rec.cell.len()), (n, 5, 5));
        for k in 0..5 {
            let (d, c) = change_at(&v[k], &v[k + 1]);
            println!(
                "P1 step {n} outer {}: record {:.12e} at c{}, route B {d:.12e} at c{c}",
                k + 1,
                rec.change[k],
                rec.cell[k]
            );
            assert_eq!(rec.cell[k], c, "P1: step {n}, outer {}", k + 1);
            assert!(rel(rec.change[k], d) <= 1e-12, "the value of ADR 0767");
        }
    }
    let (mut on, mut off) = (cavity(5, true, k0), cavity(5, false, k0));
    for _ in 0..CAV_STEPS {
        on.step(CAV_DT);
        off.step(CAV_DT);
    }
    assert!(off.outer_velocity.is_empty(), "P2: nothing is kept with the record off");
    let ub = |s: &PimpleSolver| -> Vec<u64> { s.u.values.iter().flat_map(|v| v.map(f64::to_bits)).collect() };
    let fb = |x: &[f64]| -> Vec<u64> { x.iter().map(|v| v.to_bits()).collect() };
    assert_eq!(ub(&on), ub(&off), "P2: u");
    assert_eq!(fb(&on.p.values), fb(&off.p.values), "P2: p");
    assert_eq!(fb(&on.phi), fb(&off.phi), "P2: flux");
}

/// Run a cavity control: `CAV_STEPS` steps, the record of each.
fn cavity_run(name: &str, n_outer: usize, set: impl Fn(&mut PimpleSettings)) -> (Run, PimpleSolver) {
    let mut sol = cavity(n_outer, true, set);
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
    (run, sol)
}

/// The cell of the largest change of the final and of the first iteration in each classed step, with the centre (reported).
fn print_cells(run: &Run, from: usize, centre: &dyn Fn(usize) -> String) {
    for (n, d, c) in run.steps.iter().filter(|s| s.0 >= from) {
        let last = run.n_outer - 1;
        println!(
            "F885-FP {} step {n}: d {} cells first c{} ({}), final c{} ({}), same {}",
            run.name,
            sci(d),
            c[0],
            centre(c[0]),
            c[last],
            centre(c[last]),
            c[0] == c[last]
        );
    }
}

/// The cavity controls K0 to K6b, classed; with the outcome list for the record.
fn cavity_controls(print: bool) -> (CavityOut, Vec<(String, &'static str, f64)>, Run) {
    let ap1 = |s: &mut PimpleSettings| s.alpha_p = 1.0;
    let runs: Vec<(Run, PimpleSolver)> = vec![
        cavity_run("K0", 5, k0),
        cavity_run("K1", 5, |s| {
            s.alpha_u = 1.0;
            s.alpha_p = 1.0;
        }),
        cavity_run("K2", 5, ap1),
        cavity_run("K3", 5, |s| s.alpha_u = 1.0),
        cavity_run("K4", 20, k0),
        cavity_run("K5", 5, |s| s.ddt_correction = false),
        cavity_run("K6a", 20, |s| {
            s.alpha_p = 1.0;
            s.alpha_u = 0.9;
        }),
        cavity_run("K6b", 20, |s| {
            s.alpha_p = 1.0;
            s.alpha_u = 0.95;
        }),
    ];
    let mut out = Vec::new();
    let mut k = [Jump::Undecided; 6];
    for (i, (run, sol)) in runs.iter().take(6).enumerate() {
        let (j, c) = jump(run, 1);
        k[i] = c;
        out.push((run.name.clone(), jname(c), j));
        if print {
            let cc = sol.geom.mesh.cell_centres().to_vec();
            print_cells(run, 1, &|c| format!("x {:.4}, y {:.4}", cc[c][0], cc[c][1]));
            println!("F885-FP {} J {j:.6e} {} failed {:?}", run.name, jname(c), run.failed);
        }
    }
    let (cv, k4c) = conv(&runs[4].0, 1);
    out.push(("K4:C".into(), cname(k4c), cv));
    let l = size_ratio(&runs[6].0, &runs[7].0);
    let l_in = in_band(l);
    out.push(("L".into(), if l_in { "IN" } else { "OUT" }, l));
    if print {
        println!("F885-FP K4 C {cv:.6e} {}, Q {:.6e}", cname(k4c), q_rule(&runs[4].0, 1));
        for r in [&runs[6].0, &runs[7].0] {
            println!(
                "F885-FP {} d {:?}",
                r.name,
                r.steps.iter().map(|s| sci(&s.1)).collect::<Vec<_>>()
            );
        }
        println!("F885-FP L {l:.6e} in [0.4, 0.6]: {l_in}");
    }
    (CavityOut { k, k4c, l_in }, out, runs[0].0.clone())
}

/// K0 to K6b and C0c (ADR 0810): the cavity controls, classed as registered; the outcomes held by `CAVITY_RECORD`.
#[test]
fn f885_fp_cavity_controls() {
    let (o, out, k0run) = cavity_controls(true);
    let s1 = &k0run.steps[0].1;
    println!("C0c: K0 step 1 {} against ADR 0767 A2 {}", sci(s1), sci(&A2_STEP1));
    for (a, b) in s1.iter().zip(A2_STEP1) {
        assert!(rel(*a, b) <= A2_TOL, "C0c: {a} against {b}");
    }
    let (rows, h4) = cavity_rows(&o);
    println!("F885-FP CAVITY outcomes {out:?}; rows that hold {rows:?}; H4 {h4}");
    if let Some((want, want_rows)) = CAVITY_RECORD {
        check_record(&out, want);
        assert_eq!(rows.join(","), want_rows);
    }
}

// ---------------------------------------------------------------- the rules on built records (P4)

fn built(n_outer: usize, steps: usize, d: impl Fn(usize) -> Vec<f64>) -> Run {
    Run {
        name: "built".into(),
        n_outer,
        steps: (1..=steps).map(|n| (n, d(n), vec![0; n_outer])).collect(),
        planned: steps,
        failed: None,
    }
}

/// P4 (ADR 0810): the measures and the table of section 5 on built records.
#[test]
fn f885_fp_rules_on_built_records() {
    let with = |q: f64| move |_: usize| vec![1.0, 0.2, 0.1, 0.05, 0.05 * q];
    assert_eq!(jump(&built(5, 3, with(6.0)), 1).1, Jump::Present);
    assert_eq!(jump(&built(5, 3, with(1.0)), 1).1, Jump::Absent);
    assert_eq!(jump(&built(5, 3, with(3.0)), 1).1, Jump::Undecided);
    assert_eq!(jump(&built(5, 3, with(f64::NAN)), 1).1, Jump::Undecided);
    // J reads d_N / d_(N-1): a jump at d_4 with d_5 = d_4 is no jump of the final iteration
    assert_eq!(
        jump(&built(5, 3, |_| vec![1.0, 0.2, 0.05, 1.0, 1.0]), 1).1,
        Jump::Absent
    );
    // the median over the classed steps: two of three steps carry the jump
    let mixed = built(5, 3, |n| if n == 2 { with(1.0)(n) } else { with(6.0)(n) });
    assert_eq!(jump(&mixed, 1).1, Jump::Present);
    // the fan's steps 11 to 20: the first ten steps do not count
    let late = built(5, FAN_STEPS, |n| if n < FAN_FROM { with(1.0)(n) } else { with(6.0)(n) });
    assert_eq!(jump(&late, FAN_FROM).1, Jump::Present);
    let mut cut = built(5, 3, with(6.0));
    cut.steps.pop();
    assert_eq!(jump(&cut, 1).1, Jump::Undecided, "a run that did not complete");
    let n20 = |c: f64| {
        move |_: usize| {
            (0..20)
                .map(|k| {
                    if k == 1 {
                        1.0
                    } else if k == 18 {
                        c
                    } else {
                        0.5
                    }
                })
                .collect()
        }
    };
    assert_eq!(conv(&built(20, 3, n20(1e-3)), 1).1, Conv::Converges);
    assert_eq!(conv(&built(20, 3, n20(0.1)), 1).1, Conv::Stalls);
    assert_eq!(conv(&built(20, 3, n20(f64::NAN)), 1).1, Conv::Undecided);
    let last = |x: f64| move |_: usize| (0..20).map(|k| if k == 19 { x } else { 1.0 }).collect::<Vec<f64>>();
    assert!(in_band(size_ratio(&built(20, 3, last(2.0)), &built(20, 3, last(1.0)))));
    assert!(!in_band(size_ratio(&built(20, 3, last(1.0)), &built(20, 3, last(0.7)))));

    // the table
    let h1c = CavityOut {
        k: [P, A, P, A, P, P],
        k4c: Conv::Converges,
        l_in: true,
    };
    let h1f = FanOut {
        f: [P, A, P, P, P],
        f2c: Conv::Converges,
    };
    assert_eq!(cavity_rows(&h1c).0, vec!["H1"]);
    assert_eq!(fan_rows(&h1f).0, vec!["H1"]);
    assert_eq!(result(&h1c, &h1f), "named H1");
    // H4 beside: the fan's relaxed loop stalls, the cause is still named
    assert_eq!(
        result(
            &h1c,
            &FanOut {
                f2c: Conv::Stalls,
                ..h1f
            }
        ),
        "named H1"
    );
    assert!(
        fan_rows(&FanOut {
            f2c: Conv::Stalls,
            ..h1f
        })
        .1
    );
    let h2c = CavityOut {
        k: [P, A, A, P, P, P],
        k4c: Conv::Converges,
        l_in: false,
    };
    let none_f = FanOut {
        f: [P, A, P, A, P],
        f2c: Conv::Converges,
    };
    assert_eq!(cavity_rows(&h2c).0, vec!["H2"]);
    assert_eq!(fan_rows(&none_f).0, vec!["H6"]);
    assert_eq!(result(&h2c, &none_f), "names none", "two hypotheses each on one case");
    let no_c = CavityOut {
        k: [P, A, A, A, P, P],
        k4c: Conv::Converges,
        l_in: false,
    };
    let no_f = FanOut {
        f: [P, A, A, P, P],
        f2c: Conv::Converges,
    };
    assert_eq!(result(&no_c, &no_f), "all ruled out");
    assert_eq!(result(&no_c, &none_f), "named H6");
    assert_eq!(result(&h1c, &no_f), "names none", "the cases disagree");
    let hc = CavityOut {
        k: [P, P, P, P, P, P],
        k4c: Conv::Converges,
        l_in: true,
    };
    assert_eq!(cavity_rows(&hc).0, vec!["Hc"]);
    assert_eq!(
        result(
            &hc,
            &FanOut {
                f: [P, P, P, P, P],
                ..h1f
            }
        ),
        "named Hc"
    );
    assert_eq!(
        result(
            &h1c,
            &FanOut {
                f: [Jump::Undecided, A, P, P, P],
                ..h1f
            }
        ),
        "names none",
        "a baseline UNDECIDED"
    );
    assert!(
        cavity_rows(&CavityOut {
            k4c: Conv::Stalls,
            ..h1c
        })
        .1
    );
}

// ---------------------------------------------------------------- the fan (ADR 0766, ADR 0767: the case of check H, copied)

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

/// The case text of check H, as `h_text` of `f885_check_h_outer_record.rs` (ADR 0767), unchanged.
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

/// The fan controls of ADR 0810 section 5: name, outer count, the change of the settings.
fn fan_controls() -> Vec<(&'static str, usize, fn(&mut PimpleSettings))> {
    vec![
        ("F0", 5, |_| {}),
        ("F1", 5, |s| s.alpha_u = 1.0),
        ("F2", 20, |_| {}),
        ("F3", 5, |s| s.component_diagonal = true),
        ("F4", 5, |s| s.ddt_correction = false),
    ]
}

/// One fan control: 20 steps of H(dt) with both records of ADR 0767 E-2; a failure or a non-finite value ends the run.
fn fan_run(name: &str, n_outer: usize, set: fn(&mut PimpleSettings)) -> (Run, Vec<[f64; 3]>) {
    let mut sim = sim_of(&h_text(H_DT));
    assert_eq!(sim.geom.mesh.n_cells(), 13627);
    {
        let s = &mut sim.pimple_solver_mut().expect("pimple").settings;
        assert_eq!((s.alpha_u, s.alpha_p, s.n_outer), (0.9, 1.0, 5), "the case's settings");
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        s.n_outer = n_outer;
        set(s);
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
                    "F885-FP {name} step {k}: d {} cells {:?}, {:.2} s per step",
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
        "F885-FP {name} END: {} of {FAN_STEPS} steps, failed {:?}, {:.1} s",
        run.steps.len(),
        run.failed,
        t0.elapsed().as_secs_f64()
    );
    (run, cc)
}

fn record_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f885-outer-fixed-points")
}

const FAN_HEADER: &str = "run,step,k,d,cell";

/// The fan record: one row per run, step and outer iteration; a comment line per run with its stop (if any).
fn write_fan(path: &Path, runs: &[Run]) {
    let mut out = format!(
        "# ADR 0810 (F-885): the fan controls F0 to F4 on 20 steps of H(dt) (dt = {H_DT:?} s); d [m/s] the velocity change of outer iteration k, cell its cell\n{FAN_HEADER}\n"
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
    let mut runs: Vec<Run> = fan_controls()
        .iter()
        .map(|(name, n, _)| Run {
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

/// The fan's outcomes from its runs.
fn fan_outcomes(runs: &[Run]) -> (FanOut, Vec<(String, &'static str, f64)>) {
    let mut out = Vec::new();
    let mut f = [Jump::Undecided; 5];
    for (i, r) in runs.iter().enumerate() {
        let (j, c) = jump(r, FAN_FROM);
        f[i] = c;
        out.push((r.name.clone(), jname(c), j));
    }
    let (cv, f2c) = conv(&runs[2], FAN_FROM);
    out.push(("F2:C".into(), cname(f2c), cv));
    (FanOut { f, f2c }, out)
}

/// F0 to F4 and C0f (ADR 0810): the heavy record of the fan controls, written to `F885_FP_OUT`.
#[test]
#[ignore = "heavy record, ADR 0810: the five fan controls on 20 steps of H(dt) on B2 (13 627 cells, 800 outer iterations, minutes): cargo test --release -p rcfd-api --test f885_outer_fixed_points f885_fp_fan_controls_record -- --ignored --nocapture"]
fn f885_fp_fan_controls_record() {
    let mut runs = Vec::new();
    let mut centres = Vec::new();
    for (name, n, set) in fan_controls() {
        let (r, cc) = fan_run(name, n, set);
        runs.push(r);
        centres = cc;
    }
    let dir = std::env::var("F885_FP_OUT").map_or_else(|_| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    std::fs::create_dir_all(&dir).unwrap();
    write_fan(&dir.join("fan_controls.csv"), &runs);
    println!("F885-FP fan record written to {}", dir.display());
    for r in &runs {
        print_cells(r, FAN_FROM, &|c| {
            let x = centres[c];
            format!("r {:.4}, z {:.4}", x[0].hypot(x[1]), x[2])
        });
    }
    let (o, out) = fan_outcomes(&runs);
    println!(
        "F885-FP FAN outcomes {out:?}; rows {:?}; F2 Q {:.6e}",
        fan_rows(&o),
        q_rule(&runs[2], FAN_FROM)
    );
    let s1 = &runs[0].steps[0].1;
    println!("C0f: F0 step 1 {} against ADR 0767 S2 {}", sci(s1), sci(&S2_STEP1));
    for (a, b) in s1.iter().zip(S2_STEP1) {
        assert!(rel(*a, b) <= S2_TOL, "C0f: {a} against {b}");
    }
}

/// P5 (ADR 0810): the fan's outcomes from the committed record and the result with the cavity's; held by `FAN_RECORD`.
#[test]
fn f885_fp_fan_from_the_committed_record() {
    let runs = read_fan(&record_dir().join("fan_controls.csv"));
    for r in &runs {
        println!("F885-FP {}: {} steps, failed {:?}", r.name, r.steps.len(), r.failed);
    }
    let (o, out) = fan_outcomes(&runs);
    let (rows, h4) = fan_rows(&o);
    let (c, _, _) = cavity_controls(false);
    let res = result(&c, &o);
    println!(
        "F885-FP FAN outcomes {out:?}; rows that hold {rows:?}; H4 {h4}; Q of F2 {:.6e}; RESULT {res}",
        q_rule(&runs[2], FAN_FROM)
    );
    let (want, want_rows, want_res) =
        FAN_RECORD.expect("no recorded outcome yet: the registered fan run of ADR 0810 has not been recorded");
    assert!(
        runs.iter().all(|r| r.failed.is_some() || r.steps.len() == FAN_STEPS),
        "every run complete or stopped"
    );
    check_record(&out, want);
    assert_eq!(rows.join(","), want_rows);
    assert_eq!(res, want_res);
}
