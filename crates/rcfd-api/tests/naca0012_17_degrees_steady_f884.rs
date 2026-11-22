//! ADR 0770 (F-884): why the steady SIMPLEC iteration of ADR 0760 does not reach the attached NACA 0012 state at 17
//! degrees that ADR 0764's time-accurate run finds (cl 1.5516). Five candidate causes, one run each that changes it alone
//! (KU the velocity under-relaxation, KS the start state, KT the eddy-viscosity relaxation, KC the non-orthogonal
//! corrector, KG a global pseudo time step), the unchanged case K0, and for every change that reaches the target a control
//! at 15 degrees.
//!
//! Three parts. (1) The judgement on the heavy record: the committed `docs/investigations/f884-naca0012-17-steady/
//! results.json` (written by `scripts/f884_naca0012_17_steady.py collect`) is judged here again by ADR 0167's window rule
//! D5 from its window statistics: a cause is named when its run is steady (window range of cl_eff <= 0.005, of cd_eff <=
//! 0.0005), the target is `abs(cl - 1.5516) <= 0.018`, the control `abs(cl - 1.4932034) <= 0.018`. No solver runs in
//! that part. (2) The fast tests on the same path, which run the solver: each committed changed case at 17 degrees on the
//! committed coarse O-grid of the example project for 600 iterations. (3) Negative controls of the rule and of the two
//! tolerances. ADR 0760's known miss at 17 degrees stays in `naca0012_lift_curve_f868.rs`.

use std::path::{Path, PathBuf};

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::Value;

/// ADR 0764, U17: the time mean of cl_eff at 17 degrees, the state to be reached.
const TARGET: f64 = 1.5516;
/// The acceptance of F-884 (target) and ADR 0764's C15 (control).
const TOL: f64 = 0.018;
/// ADR 0760: the steady cl_eff at 15 degrees.
const CONTROL: f64 = 1.4932034;
/// ADR 0167 D5: steady when the window range of cl_eff and of cd_eff are at most these.
const D5_CL: f64 = 0.005;
const D5_CD: f64 = 0.0005;
/// ADR 0167's mesh M1w-far500 (its check P2).
const MESH_SHA: &str = "59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889";
/// Iterations of the fast runs (the cap that replaces the case's 8000).
const FT_STEPS: usize = 600;
/// FT-x: `cl_eff` after 600 iterations of the first run of each fast test on the build of this work (main 21054b68), a
/// record of the code's own output on the coarse mesh and not a lift value. FT-U has no value: that coarse run diverges
/// (ADR 0770, amendment 1), and the test asserts the divergence.
const R_S: f64 = 1.4197453512;
const R_T: f64 = 0.6849084672;
const R_C: f64 = -0.0781516830;
const R_G: f64 = 0.0571143866;

/// The settings: (run prefix, the lines ADR 0770 adds, as key and value).
const SETTINGS: [(&str, &[(&str, &str)]); 5] = [
    ("KU", &[("alpha_u", "0.7")]),
    ("KS", &[("method", "\"potential\"")]),
    ("KT", &[("relax_nut", "1.0")]),
    (
        "KC",
        &[
            ("non_orthogonal_correctors", "1"),
            ("simplec_non_orthogonal_compensation", "true"),
        ],
    ),
    ("KG", &[("pseudo_mode", "\"physical\""), ("pseudo_dt", "0.02")]),
];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn folder() -> PathBuf {
    repo().join("docs/investigations/f884-naca0012-17-steady")
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn results() -> Value {
    serde_json::from_str(&read(&folder().join("results.json"))).expect("results.json parses")
}

fn base_case(alpha: u32) -> String {
    read(&repo().join(format!(
        "docs/investigations/f868-naca0012-lift-curve/cases/SA-a{alpha:02}.toml"
    )))
}

fn case_file(id: &str) -> String {
    read(&folder().join("cases").join(format!("{id}.toml")))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// D5 from the window statistics of a run: (range of cl_eff, range of cd_eff, steady, value of cl_eff).
fn d5(run: &Value) -> (f64, f64, bool, f64) {
    let w = &run["window"];
    let rcl = f(&w["cl_eff"]["max"]) - f(&w["cl_eff"]["min"]);
    let rcd = f(&w["cd_eff"]["max"]) - f(&w["cd_eff"]["min"]);
    let exit = run["exit"].as_i64().unwrap();
    let value = if exit == 0 {
        f(&w["cl_eff"]["final"])
    } else {
        f(&w["cl_eff"]["mean"])
    };
    (rcl, rcd, steady(rcl, rcd), value)
}

fn steady(rcl: f64, rcd: f64) -> bool {
    rcl <= D5_CL && rcd <= D5_CD
}

fn within(value: f64, reference: f64) -> bool {
    (value - reference).abs() <= TOL
}

fn run(id: &str) -> Value {
    let r = results();
    let v = r["runs"][id].clone();
    assert!(v.is_object(), "{id} is not in the record");
    v
}

/// N-x: the run of setting x at 17 degrees ends steady by D5, so x is named as a cause.
fn named(prefix: &str) {
    let r = run(&format!("{prefix}-a17"));
    let (rcl, rcd, st, cl) = d5(&r);
    assert!(
        st,
        "{prefix}-a17 not steady by D5: range cl_eff {rcl:.4e} (limit {D5_CL}), cd_eff {rcd:.4e} (limit {D5_CD}); value {cl:.5}"
    );
}

/// F17-x: steady by D5 and within the tolerance of the time-accurate value.
fn target(prefix: &str) {
    let r = run(&format!("{prefix}-a17"));
    let (rcl, rcd, st, cl) = d5(&r);
    assert!(
        st && within(cl, TARGET),
        "{prefix}-a17: steady {st} (ranges {rcl:.4e}, {rcd:.4e}), cl {cl:.5}, {:+.5} from {TARGET} (limit {TOL})",
        cl - TARGET
    );
}

#[test]
fn f884_n_ku_velocity_under_relaxation_names_a_cause() {
    named("KU");
}
#[test]
fn f884_n_ks_start_state_names_a_cause() {
    named("KS");
}
#[test]
fn f884_n_kt_eddy_viscosity_relaxation_names_a_cause() {
    named("KT");
}
#[test]
fn f884_n_kc_non_orthogonal_corrector_names_a_cause() {
    named("KC");
}
#[test]
fn f884_n_kg_global_pseudo_time_step_names_a_cause() {
    named("KG");
}
#[test]
fn f884_f17_ku_reaches_the_attached_state() {
    target("KU");
}
#[test]
fn f884_f17_ks_reaches_the_attached_state() {
    target("KS");
}
#[test]
fn f884_f17_kt_reaches_the_attached_state() {
    target("KT");
}
#[test]
fn f884_f17_kc_reaches_the_attached_state() {
    target("KC");
}
#[test]
fn f884_f17_kg_reaches_the_attached_state() {
    target("KG");
}

/// C15-x: every change that meets the target at 17 degrees has a control run at 15 degrees, steady and within the
/// tolerance of ADR 0760's steady value there.
#[test]
fn f884_c15_controls_hold_for_every_change_that_meets_the_target() {
    let r = results();
    for (p, _) in SETTINGS {
        let r17 = &r["runs"][format!("{p}-a17")];
        if !r17.is_object() {
            continue;
        }
        let (_, _, st, cl) = d5(r17);
        let ctl = &r["runs"][format!("{p}-a15")];
        if st && within(cl, TARGET) {
            assert!(
                ctl.is_object(),
                "{p} meets the target but has no control run at 15 degrees"
            );
        }
        if ctl.is_object() {
            let (rcl, rcd, st15, cl15) = d5(ctl);
            assert!(
                st15 && within(cl15, CONTROL),
                "{p}-a15: steady {st15} (ranges {rcl:.4e}, {rcd:.4e}), cl {cl15:.6}, {:+.5} from {CONTROL}",
                cl15 - CONTROL
            );
        }
    }
}

/// D0: K0 (ADR 0760's committed case unchanged) reproduces ADR 0760's run SA-a17 bit for bit.
#[test]
fn f884_d0_the_unchanged_case_reproduces_adr0760() {
    let k0 = run("K0-a17");
    let old: Value = serde_json::from_str(&read(
        &repo().join("docs/investigations/f868-naca0012-lift-curve/results.json"),
    ))
    .unwrap();
    let old = &old["runs"]["SA-a17"];
    assert_eq!(k0["iterations"], old["iterations"]);
    assert_eq!(k0["exit"], old["exit"]);
    for m in ["cl_eff", "cd_eff"] {
        for q in ["mean", "min", "max", "final"] {
            assert_eq!(
                f(&k0["window"][m][q]).to_bits(),
                f(&old[m][q]).to_bits(),
                "{m} {q}: {} against {}",
                k0["window"][m][q],
                old[m][q]
            );
        }
    }
    assert_eq!(k0["case"].as_str().unwrap(), base_case(17));
}

/// P2: the mesh of the record is ADR 0167's.
#[test]
fn f884_p2_the_mesh_is_adr0167s() {
    assert_eq!(results()["mesh"]["sha256"].as_str(), Some(MESH_SHA));
}

/// The lines of a case text after the title, as (key, value without comment) for `key = value` lines, others as is.
fn body(text: &str) -> Vec<(String, String)> {
    text.split('\n')
        .skip(2)
        .map(|l| match l.split_once(" = ") {
            Some((k, v)) if !l.starts_with('#') => (k.to_string(), v.split('#').next().unwrap().trim().to_string()),
            _ => (l.to_string(), String::new()),
        })
        .collect()
}

/// T1: each committed changed case is ADR 0760's case of its angle with only the registered lines added, equals the
/// text recorded in `results.json` where the run is there, and names its angle.
#[test]
fn f884_t1_the_cases_are_the_registered_setup() {
    let r = results();
    for (p, lines) in SETTINGS {
        for alpha in [17u32, 15] {
            let id = format!("{p}-a{alpha:02}");
            let text = case_file(&id);
            let base = body(&base_case(alpha));
            let new = body(&text);
            let mut added: Vec<(String, String)> = new.clone();
            for b in &base {
                let i = added
                    .iter()
                    .position(|x| x == b)
                    .unwrap_or_else(|| panic!("{id}: line {b:?} of ADR 0760's case is missing or changed"));
                added.remove(i);
            }
            let want: Vec<(String, String)> = lines.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            assert_eq!(added, want, "{id}: the added lines");
            assert!(text.contains(&format!("rotate_z_deg = -{alpha} ")), "{id}: the angle");
            assert!(
                text.starts_with(&format!("# ADR 0770 (F-884) run {id}:")),
                "{id}: the head comment"
            );
            if r["runs"][&id].is_object() {
                assert_eq!(
                    r["runs"][&id]["case"].as_str().unwrap(),
                    text,
                    "{id}: the recorded text"
                );
            }
        }
    }
}

/// T2: the record is consistent: min <= mean <= max, the history in the window lies in [min, max], and the driver's
/// verdicts follow from the statistics by D5.
#[test]
fn f884_t2_the_record_is_consistent() {
    let r = results();
    let runs = r["runs"].as_object().expect("runs");
    assert!(!runs.is_empty());
    for (id, run) in runs {
        let n = run["iterations"].as_u64().unwrap();
        let nw = run["window_iterations"].as_u64().unwrap();
        assert_eq!(nw, 1000.min(n / 2), "{id}");
        for (m, col) in [("cl_eff", 1), ("cd_eff", 2)] {
            let w = &run["window"][m];
            let (lo, mean, hi) = (f(&w["min"]), f(&w["mean"]), f(&w["max"]));
            assert!(lo <= mean && mean <= hi, "{id} {m}");
            for h in run["history"].as_array().unwrap() {
                if h[0].as_u64().unwrap() > n - nw {
                    let v = f(&h[col]);
                    assert!(lo <= v && v <= hi, "{id} {m}: history {v} outside [{lo}, {hi}]");
                }
            }
        }
        let (rcl, rcd, st, cl) = d5(run);
        let v = &run["verdict"];
        assert_eq!(v["steady"].as_bool(), Some(st), "{id}: steady");
        assert_eq!(f(&v["cl"]).to_bits(), cl.to_bits(), "{id}: value");
        assert!((f(&v["range_cl_eff"]) - rcl).abs() <= 1e-12 && (f(&v["range_cd_eff"]) - rcd).abs() <= 1e-12);
        let reference = if id.ends_with("-a17") { TARGET } else { CONTROL };
        assert_eq!(v["met"].as_bool(), Some(st && within(cl, reference)), "{id}: met");
    }
}

/// NC1: D5 says not steady for ADR 0760's committed run at 17 degrees.
#[test]
fn f884_nc1_d5_rejects_the_steady_arm_at_17_degrees() {
    let old: Value = serde_json::from_str(&read(
        &repo().join("docs/investigations/f868-naca0012-lift-curve/results.json"),
    ))
    .unwrap();
    let o = &old["runs"]["SA-a17"];
    let rcl = f(&o["cl_eff"]["max"]) - f(&o["cl_eff"]["min"]);
    let rcd = f(&o["cd_eff"]["max"]) - f(&o["cd_eff"]["min"]);
    assert!(rcl > 2.0 && !steady(rcl, rcd), "range {rcl}, {rcd}");
}

/// NC2: the target rejects ADR 0760's steady value at 15 degrees and its iteration mean at 17 degrees.
#[test]
fn f884_nc2_the_target_rejects_15_degrees_and_the_iteration_mean() {
    assert!(!within(CONTROL, TARGET));
    assert!(!within(1.0476, TARGET));
}

/// NC3: the control's tolerance rejects ADR 0760's steady values at 14 and 16 degrees.
#[test]
fn f884_nc3_the_control_rejects_14_and_16_degrees() {
    let old: Value = serde_json::from_str(&read(
        &repo().join("docs/investigations/f868-naca0012-lift-curve/results.json"),
    ))
    .unwrap();
    for id in ["SA-a14", "SA-a16"] {
        let o = &old["runs"][id];
        let cl = if o["exit"].as_i64() == Some(0) {
            f(&o["cl_eff"]["final"])
        } else {
            f(&o["cl_eff"]["mean"])
        };
        assert!(!within(cl, CONTROL), "{id}: {cl}");
    }
}

fn fast_case(id: &str) -> Case {
    let mesh = repo()
        .join("example-projects/wind-tunnel/meshes/naca0012-ogrid")
        .canonicalize()
        .unwrap();
    let mesh = mesh.to_string_lossy().replace('\\', "/");
    let mesh = mesh.strip_prefix("//?/").unwrap_or(&mesh).to_string();
    let mut text = case_file(id);
    for (from, to) in [
        (
            "file = \"../meshes/M1w-far500\"\n".to_string(),
            format!("file = \"{mesh}\"\n"),
        ),
        (
            "max_iterations = 8000\n".to_string(),
            format!("max_iterations = {FT_STEPS}\n"),
        ),
        (
            "min_iterations = 8000 ".to_string(),
            format!("min_iterations = {FT_STEPS} "),
        ),
    ] {
        assert_eq!(text.matches(&from).count(), 1, "{from:?}");
        text = text.replace(&from, &to);
    }
    let c = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let errs = c.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}");
    c
}

/// The value of cl_eff after 600 iterations of the committed case `id` on the coarse mesh.
fn fast_cl(id: &str) -> f64 {
    let mut s = Simulation::new(&fast_case(id)).unwrap_or_else(|e| panic!("{e:?}"));
    let steps = s.run(FT_STEPS);
    let cl = s
        .step_report_values()
        .into_iter()
        .find(|(n, _)| n == "cl_eff")
        .expect("the report cl_eff")
        .1;
    println!(
        "FT {id}: cl_eff {cl:.10} after {steps} iterations, converged {}",
        s.is_converged()
    );
    assert_eq!(steps, FT_STEPS, "{id}: iterations");
    cl
}

/// FT-x: the committed case of setting x at 17 degrees on the coarse mesh for 600 iterations keeps its recorded lift.
fn fast(id: &str, recorded: f64) {
    let cl = fast_cl(id);
    assert!(cl.is_finite(), "{id}: cl_eff {cl}");
    assert!(
        (cl - recorded).abs() <= TOL,
        "{id}: cl_eff {cl} differs from the recorded {recorded} by more than {TOL}"
    );
}

/// FT-U (amendment 1): the coarse run with alpha_u = 0.7 has diverged after 600 iterations, as its first run did.
#[test]
fn f884_ft_u_fast_run_diverges_on_the_coarse_mesh() {
    let cl = fast_cl("KU-a17");
    assert!(
        !cl.is_finite() || cl.abs() > 100.0,
        "KU-a17 on the coarse mesh: cl_eff {cl}, no longer diverged"
    );
}
#[test]
fn f884_ft_s_fast_run_keeps_its_lift() {
    fast("KS-a17", R_S);
}
#[test]
fn f884_ft_t_fast_run_keeps_its_lift() {
    fast("KT-a17", R_T);
}
#[test]
fn f884_ft_c_fast_run_keeps_its_lift() {
    fast("KC-a17", R_C);
}
#[test]
fn f884_ft_g_fast_run_keeps_its_lift() {
    fast("KG-a17", R_G);
}
