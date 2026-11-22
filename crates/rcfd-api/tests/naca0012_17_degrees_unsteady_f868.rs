//! ADR 0764 (F-868): the NACA 0012 at 17 degrees by an unsteady arm. Time-accurate Spalart-Allmaras (`pimple`, `bdf2`,
//! three outer iterations, adaptive correctors, dt 0.02 c/U to t = 200) on the mesh M1w-far500 of ADR 0760, at 17 degrees
//! and, as the control, at 15 degrees; the time mean of `cl_eff` over [100, 200] is judged by ADR 0760's band at 17
//! degrees, `abs(M - 1.6057) <= 0.0803`, and at 15 degrees against the steady value of ADR 0760 within 0.018.
//!
//! Three parts. (1) The judgement on the heavy record: the committed
//! `docs/investigations/f868-naca0012-17-degrees-unsteady/results.json` (written by
//! `scripts/f868_naca0012_lift_curve.py ucollect` from the two runs) holds the samples of the window; the means, the half
//! means over whole periods and the verdicts are computed here again from those samples. No solver runs in that part.
//! (2) The fast test on the same path, which does run the solver: the committed case of 17 degrees on the committed coarse
//! O-grid of the example project for 50 time steps. (3) Negative controls of the band, of the control's tolerance and of
//! the settled-mean rule. The known miss of the steady arm stays in `naca0012_lift_curve_f868.rs` (ADR 0760).

use std::path::{Path, PathBuf};

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::Value;

/// The scatter of the measurement (ADR 0760, S1) and the relative term of its band.
const SCATTER: f64 = 0.018;
const RELATIVE: f64 = 0.05;
/// S17, S15: the mean has settled when the half means differ by at most this (the scatter).
const SETTLED: f64 = 0.018;
/// C15: the control's tolerance (the scatter).
const CONTROL: f64 = 0.018;
/// ADR 0167 D5: a range of cl up to this is "steady in time".
const STEADY_RANGE: f64 = 0.005;
/// The window of the mean, c/U.
const WINDOW: (f64, f64) = (100.0, 200.0);
/// The end of the run, c/U.
const END: f64 = 200.0;
/// ADR 0167's mesh M1w-far500 (its check P2).
const MESH_SHA: &str = "59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889";
/// FT17: `cl_eff` after 50 steps of the first run of the fast test on the build of this work (main 2bab1863), a record
/// of the code's own output on the coarse mesh one time unit after an impulsive start, and not a lift value.
const R17: f64 = 1.2079835934;
/// Steps of the fast run (t = 1.0 at dt 0.02).
const FT_STEPS: usize = 50;

/// The tables that replace `[solver]` and `[output]` of ADR 0760's case text; `{dt}` is 0.02, or 0.01 for the fallback.
const TABLES: &str = "[solver]
algorithm = \"pimple\"
time_scheme = \"bdf2\"
n_outer = 3
n_correctors = 2
corrector_control = \"adaptive\"
convection = \"linear_upwind\"
dt = {dt}
end_time = 200.0

[output]
store_time_interval = 200.0
store_precision = \"f32\"
store_fields = [\"ux\", \"uy\", \"p\", \"nut\"]
";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn folder() -> PathBuf {
    repo().join("docs/investigations/f868-naca0012-17-degrees-unsteady")
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn results() -> Value {
    serde_json::from_str(&read(&folder().join("results.json"))).expect("results.json parses")
}

/// The record of ADR 0760 (the steady sweep).
fn steady_results() -> Value {
    serde_json::from_str(&read(
        &repo().join("docs/investigations/f868-naca0012-lift-curve/results.json"),
    ))
    .expect("the results.json of ADR 0760 parses")
}

/// `cl` of the steady curve of ADR 0760 at `alpha`.
fn steady_cl(alpha: f64) -> f64 {
    steady_results()["evaluation"]["curve"]
        .as_array()
        .expect("curve")
        .iter()
        .find(|p| p["alpha"].as_f64() == Some(alpha))
        .unwrap_or_else(|| panic!("no steady value at {alpha} degrees"))["cl"]
        .as_f64()
        .unwrap()
}

/// Ladson's 80-grit lift interpolated linearly to `alpha` (the committed table), as ADR 0760.
fn cl_ladson(alpha: f64) -> f64 {
    let text = read(&repo().join("validation-data/reference/w31c_naca0012_ladson1988_80grit_cl_cd.csv"));
    let rows: Vec<(f64, f64)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut c = l.split(',');
            (
                c.next().unwrap().trim().parse().unwrap(),
                c.next().unwrap().trim().parse().unwrap(),
            )
        })
        .collect();
    assert_eq!(rows.len(), 17, "the 80-grit zone has 17 rows");
    for w in rows.windows(2) {
        let ((a0, c0), (a1, c1)) = (w[0], w[1]);
        if a0 <= alpha && alpha <= a1 {
            return c0 + (c1 - c0) * (alpha - a0) / (a1 - a0);
        }
    }
    panic!("{alpha} degrees is outside the table");
}

/// The band of ADR 0760 at a reference lift.
fn band(cl_ref: f64) -> f64 {
    (RELATIVE * cl_ref.abs()).max(SCATTER)
}

/// The mean over [lo, hi] of the piecewise-linear signal through the samples (the trapezoid rule).
fn tmean(ts: &[f64], ys: &[f64], lo: f64, hi: f64) -> f64 {
    let (mut acc, mut span) = (0.0, 0.0);
    for k in 0..ts.len() - 1 {
        let (ta, tb, a, b) = (ts[k], ts[k + 1], ys[k], ys[k + 1]);
        let (a0, b0) = (ta.max(lo), tb.min(hi));
        if b0 <= a0 {
            continue;
        }
        let fa = a + (b - a) * (a0 - ta) / (tb - ta);
        let fb = a + (b - a) * (b0 - ta) / (tb - ta);
        acc += 0.5 * (fa + fb) * (b0 - a0);
        span += b0 - a0;
    }
    assert!(span > 0.0, "no samples in [{lo}, {hi}]");
    acc / span
}

/// The times in [lo, hi] at which the signal crosses `level` upwards (linear interpolation between samples).
fn upward(ts: &[f64], ys: &[f64], level: f64, lo: f64, hi: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for k in 0..ts.len() - 1 {
        if ts[k] < lo || ts[k + 1] > hi {
            continue;
        }
        let (a, b) = (ys[k] - level, ys[k + 1] - level);
        if a < 0.0 && 0.0 <= b {
            out.push(ts[k] + (ts[k + 1] - ts[k]) * (-a) / (b - a));
        }
    }
    out
}

/// The registered judgement of a window of samples.
#[derive(Debug)]
struct Judgement {
    m: f64,
    md: f64,
    m1: f64,
    m2: f64,
    range: f64,
    covered: bool,
    crossings: usize,
    period: Option<f64>,
}

impl Judgement {
    fn drift(&self) -> f64 {
        self.m2 - self.m1
    }
    fn settled(&self) -> bool {
        self.covered && self.drift().abs() <= SETTLED
    }
    fn oscillating(&self) -> bool {
        self.range > STEADY_RANGE
    }
}

/// ADR 0764: the time mean `M` over the window, the half means `M1`, `M2` (from the first to the last upward crossing of
/// `cl - M` in each half when it has at least two, else over the whole half), the range and the period.
fn judge(ts: &[f64], cl: &[f64], cd: &[f64], whole_periods: bool) -> Judgement {
    let (lo, hi) = (ts[0], ts[ts.len() - 1]);
    let mid = 0.5 * (WINDOW.0 + WINDOW.1);
    let m = tmean(ts, cl, lo, hi);
    let half = |a: f64, b: f64| {
        let u = if whole_periods {
            upward(ts, cl, m, a, b)
        } else {
            Vec::new()
        };
        if u.len() >= 2 {
            tmean(ts, cl, u[0], u[u.len() - 1])
        } else {
            tmean(ts, cl, a, b)
        }
    };
    let range = cl.iter().fold(f64::MIN, |x, y| x.max(*y)) - cl.iter().fold(f64::MAX, |x, y| x.min(*y));
    let u = upward(ts, cl, m, lo, hi);
    let period = (range > STEADY_RANGE && u.len() >= 4).then(|| (u[u.len() - 1] - u[0]) / (u.len() - 1) as f64);
    Judgement {
        m,
        md: tmean(ts, cd, lo, hi),
        m1: half(lo, mid),
        m2: half(mid, hi),
        range,
        covered: lo <= WINDOW.0 + 1e-6 && hi >= WINDOW.1 - 1e-6,
        crossings: u.len(),
        period,
    }
}

/// A counted run of the record: its entry, and the judgement computed here from its committed samples.
struct Run {
    id: String,
    entry: Value,
    j: Judgement,
}

impl Run {
    /// R1 for this run: exit code 0, the end time reached, no `corrector_cap` warning.
    fn complete(&self) -> bool {
        self.entry["exit"].as_i64() == Some(0)
            && self.entry["time_end"].as_f64().unwrap() >= END - 1e-6
            && !self.entry["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w.as_str() == Some("corrector_cap"))
    }
}

/// The counted run of `alpha` (the registered pair, or the fallback pair with dt 0.01 when the record says so).
fn run_of(r: &Value, alpha: u32) -> Run {
    let id = r["counted"]
        .as_array()
        .expect("counted")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .find(|i| i.starts_with(&format!("U-a{alpha:02}")))
        .unwrap_or_else(|| panic!("no counted run at {alpha} degrees"));
    let entry = r["runs"][&id].clone();
    let col = |k: usize| -> Vec<f64> {
        entry["window_samples"]
            .as_array()
            .expect("window_samples")
            .iter()
            .map(|s| s[k].as_f64().unwrap())
            .collect()
    };
    let (ts, cl, cd) = (col(0), col(1), col(2));
    assert!(ts.len() >= 3, "{id}: no samples in the window");
    let j = judge(&ts, &cl, &cd, true);
    Run { id, entry, j }
}

fn describe(run: &Run) {
    println!(
        "{}: M {:.6}, M1 {:.6}, M2 {:.6}, drift {:+.6}, range {:.6} ({}), crossings {}, period {:?}, Md {:.6}, complete {}",
        run.id,
        run.j.m,
        run.j.m1,
        run.j.m2,
        run.j.drift(),
        run.j.range,
        if run.j.oscillating() {
            "oscillating"
        } else {
            "steady in time"
        },
        run.j.crossings,
        run.j.period,
        run.j.md,
        run.complete()
    );
}

/// U17: the time mean of the lift at 17 degrees, of a complete and settled run, is within ADR 0760's band of Ladson's
/// 80-grit lift.
#[test]
fn f868u_u17_time_mean_at_17_degrees_within_the_band() {
    let r = results();
    let run = run_of(&r, 17);
    describe(&run);
    let reference = cl_ladson(17.0);
    let (d, b) = (run.j.m - reference, band(reference));
    println!(
        "U17: M {:.4}, Ladson {reference:.4}, difference {d:+.4}, band {b:.4}",
        run.j.m
    );
    assert!(
        (reference - 1.6057).abs() < 5e-5 && (b - 0.0803).abs() < 5e-5,
        "{reference} {b}"
    );
    assert!(run.complete(), "{}: the run is not complete", run.id);
    assert!(
        run.j.settled(),
        "{}: the mean has not settled, drift {:+.4}",
        run.id,
        run.j.drift()
    );
    assert!(
        d.abs() <= b,
        "17 degrees: the time mean {:.4} differs from {reference:.4} by {d:+.4}, band {b:.4}",
        run.j.m
    );
}

/// S17: the mean at 17 degrees has settled (half means over whole periods within the scatter).
#[test]
fn f868u_s17_mean_at_17_degrees_has_settled() {
    let r = results();
    let run = run_of(&r, 17);
    describe(&run);
    assert!(run.j.covered, "{}: the samples do not cover the window", run.id);
    assert!(
        run.j.drift().abs() <= SETTLED,
        "{}: M1 {:.4}, M2 {:.4}, drift {:+.4} above {SETTLED}",
        run.id,
        run.j.m1,
        run.j.m2,
        run.j.drift()
    );
}

/// C15: the control. The same set-up at 15 degrees returns the steady value of ADR 0760 within the scatter.
#[test]
fn f868u_c15_control_reproduces_the_steady_value() {
    let r = results();
    let run = run_of(&r, 15);
    describe(&run);
    let steady = steady_cl(15.0);
    let d = run.j.m - steady;
    println!(
        "C15: M {:.6}, steady {steady:.6}, difference {d:+.6}, tolerance {CONTROL}",
        run.j.m
    );
    assert!((steady - 1.4932034).abs() < 1e-6, "{steady}");
    assert!(run.complete(), "{}: the run is not complete", run.id);
    assert!(
        d.abs() <= CONTROL,
        "15 degrees: the time mean {:.4} differs from the steady {steady:.4} by {d:+.4}",
        run.j.m
    );
}

/// S15: the mean at 15 degrees has settled.
#[test]
fn f868u_s15_mean_at_15_degrees_has_settled() {
    let r = results();
    let run = run_of(&r, 15);
    assert!(run.j.covered, "{}: the samples do not cover the window", run.id);
    assert!(
        run.j.drift().abs() <= SETTLED,
        "{}: M1 {:.4}, M2 {:.4}, drift {:+.4} above {SETTLED}",
        run.id,
        run.j.m1,
        run.j.m2,
        run.j.drift()
    );
}

/// R1 and P2: both counted runs reach t = 200 with exit code 0 and no corrector-cap warning, on ADR 0167's mesh.
#[test]
fn f868u_r1_p2_two_complete_runs_on_the_registered_mesh() {
    let r = results();
    assert_eq!(r["counted"].as_array().unwrap().len(), 2);
    for alpha in [15, 17] {
        let run = run_of(&r, alpha);
        assert!(
            run.complete(),
            "{}: exit {}, end {}, warnings {}",
            run.id,
            run.entry["exit"],
            run.entry["time_end"],
            run.entry["warnings"]
        );
        let (dt, steps) = (run.entry["dt"].as_f64().unwrap(), run.entry["steps"].as_f64().unwrap());
        assert!((steps * dt - END).abs() < 1e-6, "{}: {steps} steps of {dt}", run.id);
        assert_eq!(run.entry["alpha"].as_u64(), Some(alpha as u64));
    }
    assert_eq!(r["evaluation"]["P2"]["sha256"].as_str().unwrap(), MESH_SHA);
    assert_eq!(r["mesh"].as_str().unwrap(), "M1w-far500");
    assert_eq!(r["model"].as_str().unwrap(), "spalart_allmaras");
}

/// The value of the one line `key = value   # comment` of a case text.
fn line_value<'a>(text: &'a str, key: &str) -> &'a str {
    let start = format!("{key} = ");
    let mut found = text.lines().filter(|l| l.starts_with(&start));
    let l = found.next().unwrap_or_else(|| panic!("no line {key}"));
    assert!(found.next().is_none(), "more than one line {key}");
    l[start.len()..].split('#').next().unwrap().trim()
}

/// T1: the committed case files are the texts the runs used; each is ADR 0760's case of its angle with the head, the
/// title and the tables `[solver]` and `[output]` replaced by the registered ones, and nothing else.
#[test]
fn f868u_t1_committed_cases_are_the_registered_set_up() {
    let r = results();
    for alpha in [15u32, 17] {
        let run = run_of(&r, alpha);
        let id = &run.id;
        let text = read(&folder().join("cases").join(format!("{id}.toml")));
        assert_eq!(
            r["cases"][id].as_str().unwrap(),
            text,
            "{id}: the committed case differs from the record"
        );
        let dt = if id.ends_with("-dt1") { "0.01" } else { "0.02" };
        assert_eq!(
            run.entry["dt"].as_f64().unwrap().to_string(),
            dt,
            "{id}: dt of the record"
        );
        let steady = read(
            &repo()
                .join("docs/investigations/f868-naca0012-lift-curve/cases")
                .join(format!("SA-a{alpha:02}.toml")),
        );
        let mark = "\n# ADR 0167: the effective force";
        let (a, b) = (
            text.find("[solver]\n").expect("[solver]"),
            text.find(mark).expect("the reports of ADR 0167"),
        );
        let (sa, sb) = (steady.find("[solver]\n").unwrap(), steady.find(mark).unwrap());
        // the two tables are the registered ones
        assert_eq!(&text[a..b], TABLES.replace("{dt}", dt), "{id}: [solver] and [output]");
        // everything after them, and everything before them but the head comment and the title, is the steady case
        assert_eq!(&text[b..], &steady[sb..], "{id}: the reports");
        let body = |t: &str, end: usize| t[..end].splitn(3, '\n').nth(2).unwrap().to_string();
        assert_eq!(
            body(&text, a),
            body(&steady, sa),
            "{id}: mesh, physics, boundaries, turbulence"
        );
        let rot: f64 = line_value(&text, "rotate_z_deg").parse().unwrap();
        assert_eq!(rot, -(alpha as f64), "{id}: rotate_z_deg");
        assert_eq!(line_value(&text, "model"), "\"spalart_allmaras\"", "{id}");
        assert_eq!(line_value(&text, "file"), "\"../meshes/M1w-far500\"", "{id}");
        assert_eq!(line_value(&text, "turbulence"), "\"freestream\"", "{id}");
        assert_eq!(line_value(&text, "algorithm"), "\"pimple\"", "{id}");
        assert_eq!(line_value(&text, "time_scheme"), "\"bdf2\"", "{id}");
        assert!(!text.contains("des ="), "{id}: no DES");
    }
}

/// T2: the driver's values in the record follow from the committed samples (relative difference at most 1e-8).
#[test]
fn f868u_t2_the_record_follows_from_its_samples() {
    let r = results();
    for alpha in [15, 17] {
        let run = run_of(&r, alpha);
        let rec = &run.entry["judgement"];
        for (name, mine) in [
            ("M", run.j.m),
            ("Md", run.j.md),
            ("M1", run.j.m1),
            ("M2", run.j.m2),
            ("range", run.j.range),
        ] {
            let theirs = rec[name].as_f64().unwrap_or_else(|| panic!("{}: no {name}", run.id));
            let scale = theirs.abs().max(1e-3);
            println!("T2 {} {name}: record {theirs:.12}, here {mine:.12}", run.id);
            assert!(
                (mine - theirs).abs() <= 1e-8 * scale,
                "{} {name}: the record has {theirs}, the samples give {mine}",
                run.id
            );
        }
        assert_eq!(rec["settled"].as_bool(), Some(run.j.settled()), "{}", run.id);
        assert_eq!(rec["oscillating"].as_bool(), Some(run.j.oscillating()), "{}", run.id);
        assert_eq!(run.entry["complete"].as_bool(), Some(run.complete()), "{}", run.id);
    }
    // the evaluation of the record agrees with the judgement made here
    let (u, c) = (run_of(&r, 17), run_of(&r, 15));
    let reference = cl_ladson(17.0);
    let met = u.complete() && u.j.settled() && (u.j.m - reference).abs() <= band(reference);
    assert_eq!(r["evaluation"]["U17"]["met"].as_bool(), Some(met));
    let holds = c.complete() && c.j.settled() && (c.j.m - steady_cl(15.0)).abs() <= CONTROL;
    assert_eq!(r["evaluation"]["C15"]["holds"].as_bool(), Some(holds));
}

/// B1: the folder of the record stays small.
#[test]
fn f868u_b1_storage_budget() {
    fn size(p: &Path) -> u64 {
        std::fs::read_dir(p)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                if e.path().is_dir() {
                    size(&e.path())
                } else {
                    e.metadata().unwrap().len()
                }
            })
            .sum()
    }
    let total = size(&folder());
    println!("B1: {total} bytes");
    assert!(total <= 300_000, "{total} bytes");
}

/// NC1: the band at 17 degrees rejects the steady iteration mean of ADR 0760 (1.0476).
#[test]
fn f868u_nc1_the_band_rejects_the_steady_iteration_mean() {
    let cl = steady_cl(17.0);
    let reference = cl_ladson(17.0);
    println!(
        "NC1: the steady iteration mean {cl:.4} against {reference:.4}, band {:.4}",
        band(reference)
    );
    assert!((cl - 1.0476).abs() < 5e-5, "{cl}");
    assert!((cl - reference).abs() > band(reference));
}

/// NC2: the control's tolerance tells one degree: the time mean at 15 degrees is rejected against the steady values of
/// 14 and of 16 degrees.
#[test]
fn f868u_nc2_the_control_rejects_the_neighbouring_angles() {
    let r = results();
    let m = run_of(&r, 15).j.m;
    for alpha in [14.0, 16.0] {
        let d = m - steady_cl(alpha);
        println!("NC2: M(15) {m:.4} against the steady value of {alpha} degrees: {d:+.4}");
        assert!(
            d.abs() > CONTROL,
            "the control accepts the value of {alpha} degrees ({d:+.4})"
        );
    }
}

/// NC3: the settled-mean rule calls a ramp of 0.001 per time unit not settled and a sine of amplitude 0.76 and period
/// 4.2 (ADR 0159's swing at 20 degrees) settled; without whole periods the half means of the sine differ by more.
#[test]
fn f868u_nc3_the_settled_rule_rejects_a_ramp_and_accepts_a_sine() {
    let ts: Vec<f64> = (0..=1000).map(|k| 100.0 + 0.1 * k as f64).collect();
    let cd = vec![0.03; ts.len()];
    let ramp: Vec<f64> = ts.iter().map(|t| 1.0 + 0.001 * (t - 100.0)).collect();
    let j = judge(&ts, &ramp, &cd, true);
    println!("NC3 ramp: drift {:+.6}", j.drift());
    assert!((j.drift() - 0.05).abs() < 1e-9 && !j.settled(), "{j:?}");
    let sine: Vec<f64> = ts
        .iter()
        .map(|t| 1.0 + 0.76 * (2.0 * std::f64::consts::PI * t / 4.2).sin())
        .collect();
    let (j, plain) = (judge(&ts, &sine, &cd, true), judge(&ts, &sine, &cd, false));
    println!(
        "NC3 sine: drift {:+.3e} with whole periods, {:+.3e} without; period {:?}",
        j.drift(),
        plain.drift(),
        j.period
    );
    assert!(j.settled() && j.drift().abs() < 1e-3, "{j:?}");
    assert!(j.drift().abs() < plain.drift().abs(), "{j:?} {plain:?}");
    assert!((j.period.expect("a period") - 4.2).abs() < 1e-3);
    // a window that stops short of t = 200 is not settled
    let short = judge(&ts[..900], &sine[..900], &cd[..900], true);
    assert!(!short.settled());
}

/// The committed case file of 17 degrees on the committed coarse O-grid of the example project (6144 cells, far field at
/// 20 chords), with exactly one line replaced: the mesh file.
fn fast_case() -> Case {
    let mesh = repo()
        .join("example-projects/wind-tunnel/meshes/naca0012-ogrid")
        .canonicalize()
        .unwrap();
    let mesh = mesh.to_string_lossy().replace('\\', "/");
    let mesh = mesh.strip_prefix("//?/").unwrap_or(&mesh).to_string();
    let mut text = read(&folder().join("cases").join("U-a17.toml"));
    let from = "file = \"../meshes/M1w-far500\"\n";
    assert_eq!(text.matches(from).count(), 1, "{from:?}");
    text = text.replace(from, &format!("file = \"{mesh}\"\n"));
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let errs = c.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}");
    c
}

/// FT17: the same path on the coarse mesh for 50 time steps gives the recorded lift within 0.018.
#[test]
fn f868u_ft17_fast_run_of_the_same_case_keeps_its_lift() {
    let mut s = Simulation::new(&fast_case()).unwrap_or_else(|e| panic!("{e:?}"));
    let steps = s.run(FT_STEPS);
    let cl = s
        .step_report_values()
        .into_iter()
        .find(|(n, _)| n == "cl_eff")
        .expect("the report cl_eff")
        .1;
    println!("FT17: cl_eff {cl:.10} after {steps} steps, time {}", s.time());
    assert_eq!(steps, FT_STEPS);
    assert!((s.time() - 1.0).abs() < 1e-9, "time {}", s.time());
    assert!(cl.is_finite(), "cl_eff = {cl}");
    assert!(
        (cl - R17).abs() <= SCATTER,
        "cl_eff = {cl} differs from the recorded {R17} by more than {SCATTER}"
    );
}
