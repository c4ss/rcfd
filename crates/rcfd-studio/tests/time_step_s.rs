//! ADR 0153 (job 48S): the time step is set when a run starts. Checks of the model on the example cases (S-E: the
//! computed step follows the cell size; S-G: a case without a speed scale; S-H: a JSON case plans as its TOML twin; S-K:
//! the scope, steady and compressible cases) and the solver runs of the decision between the adaptive and the fixed
//! automatic step (D1-D4: `rcfd run` of the example cases with the Studio's own `--set` entries; heavy, ignored, run on
//! purpose). Release builds, `--test-threads 1 --nocapture`; `rcfd` of the same build (`common::binaries`).

mod common;
mod ts;

use rcfd_studio::time_step::{self, Facts, Kind, RunTime, Step, StepMode, TimePlan};
use std::path::Path;

fn facts(text: &str, dir: &Path) -> Facts {
    time_step::facts(text, dir).unwrap_or_else(|e| panic!("facts: {e}"))
}

fn plan(f: &Facts, n: f64, mode: StepMode) -> TimePlan {
    time_step::plan(
        f,
        &RunTime {
            simulate: Some(n),
            mode,
            ..RunTime::default()
        },
    )
    .unwrap_or_else(|e| panic!("plan: {e}"))
}

fn dt0(tp: &TimePlan) -> f64 {
    match tp.step {
        Step::Adaptive { dt0, .. } => dt0,
        Step::Fixed { dt, .. } => dt,
        Step::CaseDt { dt } => dt,
        Step::CaseControl { .. } => f64::NAN,
    }
}

// ---------------------------------------------------------------- S-E: the step follows the cell size

/// S-E's measure: the computed first step of the default plan (`dt0`), its largest step, and the thinnest cell of each
/// case and of the case refined as "Refine x2" refines it (every count doubled), with the run-time setting `mode`.
fn se(mode: StepMode) -> Vec<(String, f64, f64, f64, f64)> {
    let wt = ts::wind_tunnel_cases();
    let qs = ts::quick_start_cases();
    let pairs = [
        (
            "cylinder 64 x 48 -> 128 x 96",
            ts::cylinder(64, 48),
            ts::cylinder(128, 96),
            wt.clone(),
            80.0,
        ),
        (
            "cavity piso 32 -> 64",
            ts::cavity_piso(32, 10.0),
            ts::cavity_piso(64, 10.0),
            qs.clone(),
            10.0,
        ),
    ];
    let mut out = Vec::new();
    for (tag, a, b, dir, n) in pairs {
        let (fa, fb) = (facts(&a, &dir), facts(&b, &dir));
        let (pa, pb) = (plan(&fa, n, mode), plan(&fb, n, mode));
        let max = |p: &TimePlan| match p.step {
            Step::Adaptive { max_dt, .. } => max_dt,
            _ => f64::NAN,
        };
        eprintln!(
            "S-E {tag} ({mode:?}): h_min {:.6} -> {:.6} (ratio {:.4}); dt0 {} -> {} (ratio {:.4}); max_dt {} -> {}",
            fa.h_min,
            fb.h_min,
            fb.h_min / fa.h_min,
            dt0(&pa),
            dt0(&pb),
            dt0(&pb) / dt0(&pa),
            max(&pa),
            max(&pb)
        );
        out.push((
            tag.to_string(),
            dt0(&pb) / dt0(&pa),
            fb.h_min / fa.h_min,
            max(&pa),
            max(&pb),
        ));
    }
    out
}

/// S-E: halving the cell size (every count doubled) halves the computed first step within [0.47, 0.53]; the largest step
/// (the simulated time over 100) does not change.
#[test]
fn se_the_computed_step_halves_with_the_cell_size() {
    for (tag, ratio, _, ma, mb) in se(StepMode::Adaptive) {
        assert!((0.47..=0.53).contains(&ratio), "{tag}: dt0 ratio {ratio}");
        assert_eq!(ma.to_bits(), mb.to_bits(), "{tag}: max_dt");
    }
}

/// S-E control: the case's own step (`StepMode::Case`, the step before ADR 0153) does not follow the cell size.
#[test]
fn se_control_the_case_step_does_not_follow_the_mesh() {
    let r = se(StepMode::Case);
    assert!(
        r.iter().any(|(_, ratio, ..)| !(0.47..=0.53).contains(ratio)),
        "control: {r:?}"
    );
}

// ---------------------------------------------------------------- S-G: no speed scale

/// S-G (model): (g1) the heated cavity at rest under PISO gets the gravity scale `sqrt(|g| L)` (7100 m/s^2 over 1 m) and
/// a plan from it; (g2) the cavity whose lid is ramped from rest has no speed scale: the adaptive plan starts with the
/// case's own `solver.dt` (the schema default 0.01 s) under the Courant controller and says so; the fixed automatic step
/// is refused with the registered message.
#[test]
fn sg_gravity_scale_and_no_speed_scale() {
    let dir = ts::quick_start_cases();
    // (g1)
    let f = facts(&ts::heated_piso(2.0), &dir);
    eprintln!(
        "S-G (g1) facts: u0 {} from {:?}, h_min {}, dt {}",
        f.u0, f.u0_from, f.h_min, f.dt
    );
    assert_eq!(f.kind, Kind::Transient);
    assert_eq!(f.u0_from.as_deref(), Some(time_step::GRAVITY_SCALE));
    assert!((f.u0 - 7100f64.sqrt()).abs() <= 1e-9 * f.u0, "(g1) u0 {}", f.u0);
    let p = plan(&f, 2.0, StepMode::Adaptive);
    assert!(!p.no_speed);
    assert_eq!(
        p.entries,
        vec![
            "solver.end_time=2",
            "solver.dt=0.000148",
            "solver.max_courant=0.5",
            "solver.max_dt=0.02"
        ]
    );
    // (g2)
    let f = facts(&ts::cavity_ramp_piso(2.0), &dir);
    eprintln!(
        "S-G (g2) facts: u0 {} from {:?}, h_min {}, dt {}",
        f.u0, f.u0_from, f.h_min, f.dt
    );
    assert_eq!(f.kind, Kind::Transient);
    assert_eq!((f.u0, f.u0_from.clone()), (0.0, None));
    let p = plan(&f, 2.0, StepMode::Adaptive);
    assert!(p.no_speed);
    assert_eq!(
        p.entries,
        vec![
            "solver.end_time=2",
            "solver.dt=0.01",
            "solver.max_courant=0.5",
            "solver.max_dt=0.02"
        ]
    );
    assert_eq!(p.courant0, None);
    let h = time_step::hover(&f, &p);
    assert!(
        h.contains(
            "(the case's solver.dt: no inflow, moving wall, initial velocity or gravity gives a speed at the start)"
        ),
        "{h}"
    );
    let fixed = time_step::plan(
        &f,
        &RunTime {
            mode: StepMode::Fixed,
            ..RunTime::default()
        },
    );
    assert_eq!(
        fixed,
        Err(
            "no speed scale for a fixed automatic step: the case has no inflow, moving wall, initial velocity or \
             gravity at its start"
                .to_string()
        )
    );
}

// ---------------------------------------------------------------- S-H: JSON and TOML

/// S-H (model): the cylinder as JSON (`Case::to_json_string`, the writer of O10) has the facts and the plan of its TOML
/// file, apart from the text's hash.
#[test]
fn sh_a_json_case_plans_as_its_toml() {
    let dir = ts::wind_tunnel_cases();
    let toml = ts::cylinder(64, 48);
    let json = rcfd_api::case::Case::parse_toml(&toml).unwrap().to_json_string();
    let (ft, fj) = (facts(&toml, &dir), facts(&json, &dir));
    eprintln!("S-H: toml {ft:?}\n     json {fj:?}");
    assert_ne!(ft.hash, fj.hash);
    // the JSON writer spells out every setting: the text sets solver.dt and solver.end_time either way
    assert_eq!(
        Facts {
            hash: String::new(),
            ..ft.clone()
        },
        Facts {
            hash: String::new(),
            ..fj.clone()
        }
    );
    assert_eq!(plan(&ft, 80.0, StepMode::Adaptive), plan(&fj, 80.0, StepMode::Adaptive));
}

// ---------------------------------------------------------------- S-K: the scope

/// S-K (model): steady examples and the compressible cases of the repository get no time plan and the registered line;
/// the cylinder's facts are those registered (its own `solver.dt` and `solver.end_time` in the text, no step control).
#[test]
fn sk_scope_steady_compressible_and_the_cylinder() {
    let qs = ts::quick_start_cases();
    let cav = std::fs::read_to_string(qs.join("cavity-re100.toml")).unwrap();
    let f = facts(&cav, &qs);
    assert_eq!(f.kind, Kind::Steady { max_iterations: 2000 });
    assert_eq!(
        time_step::other_label(&f).as_deref(),
        Some("steady (simplec): until converged, at most 2,000 iterations")
    );
    assert_eq!(
        time_step::plan(&f, &RunTime::default()),
        Err("a steady run has no time step".to_string())
    );
    for c in ["sod/sod.toml", "wedge/wedge.toml"] {
        let p = ts::repo_root().join("cases").join(c);
        let text = std::fs::read_to_string(&p).unwrap();
        let f = facts(&text, p.parent().unwrap());
        eprintln!("S-K {c}: {:?}, label {:?}", f.kind, time_step::other_label(&f));
        assert_eq!(
            f.kind,
            Kind::Other("the compressible solver is explicit and sets its own step from compressible.cfl".into()),
            "{c}"
        );
        assert!(time_step::plan(&f, &RunTime::default()).is_err(), "{c}");
        assert!(
            time_step::other_label(&f)
                .unwrap()
                .starts_with("compressible: the case's own time step, to its end time"),
            "{c}"
        );
    }
    let wt = ts::wind_tunnel_cases();
    let f = facts(&ts::cylinder(64, 48), &wt);
    eprintln!("S-K cylinder: {f:?}");
    assert_eq!(f.kind, Kind::Transient);
    assert!(f.dt_in_text && f.end_in_text && f.own_control.is_empty());
    assert_eq!((f.dt, f.end_time, f.cells), (0.0625, 80.0, 3072));
    assert_eq!(f.u0_from.as_deref(), Some("the initial velocity field"));
}

// ---------------------------------------------------------------- D1-D4: the decision

/// What one D run measured (printed whole).
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DRun {
    mode: StepMode,
    entries: Vec<String>,
    exit: i32,
    wall_s: f64,
    steps: usize,
    t_end_bits: u64,
    t_end: f64,
    co_first: f64,
    co_max_after_10: f64,
    dt_first: f64,
    dt_min: f64,
    dt_max: f64,
    last_step_fraction: f64,
    metric: f64,
}

/// The monitor columns of a run by name.
type Columns = std::collections::BTreeMap<String, Vec<f64>>;

/// One D run: `text` with the Studio's plan for `mode` (simulated time `n`), run by `rcfd run`; `metric` reads the
/// comparison quantity from the monitor columns.
fn d_run(tag: &str, text: &str, dir: &Path, n: f64, mode: StepMode, metric: &dyn Fn(&Columns) -> f64) -> DRun {
    let (_, rcfd) = common::binaries();
    let f = facts(text, dir);
    let p = plan(&f, n, mode);
    let out = common::temp(&format!("d-{tag}-{mode:?}")).join("out");
    let (exit, wall_s) = ts::run_cli(&rcfd, text, dir, &out, &p.entries);
    let m = ts::monitors(&out.join("monitors.csv"));
    let (t, dt, co) = (&m["time"], &m["dt"], &m["courant"]);
    let k = dt.len();
    let r = DRun {
        mode,
        entries: p.entries.clone(),
        exit,
        wall_s,
        steps: t.len(),
        t_end_bits: t.last().copied().unwrap_or(f64::NAN).to_bits(),
        t_end: t.last().copied().unwrap_or(f64::NAN),
        co_first: co.first().copied().unwrap_or(f64::NAN),
        co_max_after_10: ts::max_from(co, 10),
        dt_first: dt.first().copied().unwrap_or(f64::NAN),
        dt_min: dt.iter().copied().fold(f64::INFINITY, f64::min),
        dt_max: dt.iter().copied().fold(0.0, f64::max),
        last_step_fraction: if k >= 2 { dt[k - 1] / dt[k - 2] } else { f64::NAN },
        metric: metric(&m),
    };
    eprintln!("D {tag} {mode:?}: {r:?}");
    r
}

/// D's limits on the adaptive run `a` against the fixed run `f` (registered): R1 both end at `n` bit for bit with exit 0
/// (the adaptive run: the ADR 0050 controller ran, its largest step above its first; the earlier controller never steps
/// above `solver.dt`); R2 the adaptive run's first Courant number at most the target and its largest after step 10 at
/// most 1.1 times the target; R3 its metric within `tol` (relative) of the fixed run's.
fn d_limits(tag: &str, n: f64, a: &DRun, f: &DRun, tol: f64) {
    let c = time_step::COURANT_TARGET;
    eprintln!(
        "D {tag} summary: adaptive {} steps {:.1} s, fixed {} steps {:.1} s; metric {} vs {} (rel {:.4})",
        a.steps,
        a.wall_s,
        f.steps,
        f.wall_s,
        a.metric,
        f.metric,
        (a.metric - f.metric).abs() / f.metric.abs()
    );
    for r in [a, f] {
        assert_eq!(r.exit, 0, "{tag} R1 {:?}: exit", r.mode);
        assert_eq!(r.t_end_bits, n.to_bits(), "{tag} R1 {:?}: end {}", r.mode, r.t_end);
    }
    assert!(
        a.dt_max > a.dt_first,
        "{tag} R1: the adaptive step did not grow above its first ({} .. {})",
        a.dt_first,
        a.dt_max
    );
    assert!(a.co_first <= c, "{tag} R2: first Courant number {}", a.co_first);
    assert!(
        a.co_max_after_10 <= 1.1 * c,
        "{tag} R2: Courant {} after step 10",
        a.co_max_after_10
    );
    let rel = (a.metric - f.metric).abs() / f.metric.abs();
    assert!(rel <= tol, "{tag} R3: {} against {} ({rel})", a.metric, f.metric);
}

/// The metric "the last value of column `name`".
fn last(name: &'static str) -> impl Fn(&Columns) -> f64 {
    move |m| m[name].last().copied().unwrap_or(f64::NAN)
}

/// D1: the wind tunnel's cylinder (64 x 48) to 80 s; metric: the Strouhal number over the second half.
#[test]
#[ignore = "heavy: ADR 0153 D1 (about 5 minutes)"]
fn d1_cylinder_64x48() {
    let (text, dir) = (ts::cylinder(64, 48), ts::wind_tunnel_cases());
    let st = |m: &Columns| {
        let s = ts::shedding(&m["time"], &m["cl"], 40.0, 80.0);
        eprintln!("D1 shedding {s:?}");
        s.strouhal.unwrap_or(f64::NAN)
    };
    let a = d_run("d1", &text, &dir, 80.0, StepMode::Adaptive, &st);
    let f = d_run("d1", &text, &dir, 80.0, StepMode::Fixed, &st);
    d_limits("D1", 80.0, &a, &f, 0.03);
}

/// D2: the cylinder at 256 x 256 (the maintainer's refinement) to 3 s; metric: `cd` at the end.
#[test]
#[ignore = "heavy: ADR 0153 D2 (about 20 minutes)"]
fn d2_cylinder_256x256() {
    let (text, dir) = (ts::cylinder(256, 256), ts::wind_tunnel_cases());
    let a = d_run("d2", &text, &dir, 3.0, StepMode::Adaptive, &last("cd"));
    let f = d_run("d2", &text, &dir, 3.0, StepMode::Fixed, &last("cd"));
    d_limits("D2", 3.0, &a, &f, 0.02);
}

/// D3: the quick start's cavity as PISO to 10 s; metric: `lid_drag` at the end.
#[test]
#[ignore = "heavy: ADR 0153 D3"]
fn d3_cavity_piso() {
    let (text, dir) = (ts::cavity_piso(32, 10.0), ts::quick_start_cases());
    let a = d_run("d3", &text, &dir, 10.0, StepMode::Adaptive, &last("lid_drag"));
    let f = d_run("d3", &text, &dir, 10.0, StepMode::Fixed, &last("lid_drag"));
    d_limits("D3", 10.0, &a, &f, 0.01);
}

/// D4: the wind tunnel's step (Re 400) as PISO to 30 s; metric: `p_inlet` at the end.
#[test]
#[ignore = "heavy: ADR 0153 D4 (about 4 minutes)"]
fn d4_step_piso() {
    let (text, dir) = (ts::step_piso(30.0), ts::wind_tunnel_cases());
    let a = d_run("d4", &text, &dir, 30.0, StepMode::Adaptive, &last("p_inlet"));
    let f = d_run("d4", &text, &dir, 30.0, StepMode::Fixed, &last("p_inlet"));
    d_limits("D4", 30.0, &a, &f, 0.02);
}
