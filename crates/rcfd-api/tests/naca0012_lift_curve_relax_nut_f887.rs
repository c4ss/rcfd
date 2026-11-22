//! ADR 0772 (F-887): ADR 0760's twelve steady NACA 0012 cases (0 to 17 degrees, Spalart-Allmaras, mesh M1w-far500) with
//! the one setting `turbulence.relax_nut = 1.0` added, the setting that reached the attached state at 17 degrees in ADR
//! 0770. No default is changed.
//!
//! Three parts. (1) The judgement on the heavy record: the committed `docs/investigations/f887-naca0012-lift-curve-relax-nut/
//! results.json` (written by `scripts/f887_naca0012_lift_curve_relax_nut.py collect`) is judged here again from its window
//! statistics by ADR 0167's D5 (steady when the window range of cl_eff <= 0.005 and of cd_eff <= 0.0005) and D7, then by
//! row A (the acceptance: steady and within 0.018 of ADR 0760's steady value, of ADR 0764's 1.5516 at 17 degrees), row V
//! (ADR 0760's band against Ladson's 80-grit lift) and row VS (the scatter 0.018 alone). No solver runs in that part.
//! (2) The fast tests on the same path, which run the solver: the committed cases at 0 and 4 degrees on the committed
//! coarse O-grid of the example project for 600 iterations. (3) Negative controls of the tolerance, of D5 and of the band.

use std::path::{Path, PathBuf};

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::Value;

/// The twelve angles of ADR 0760.
const ALPHAS: [u32; 12] = [0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17];
/// The acceptance of F-887: cl within this of the reference of row A.
const TOL: f64 = 0.018;
/// ADR 0764, U17: the time mean of cl_eff at 17 degrees, the reference of row A at 17 degrees.
const TARGET17: f64 = 1.5516;
/// ADR 0760: the scatter of Ladson's three grit zones (S1) and the relative term of its band.
const SCATTER: f64 = 0.018;
const RELATIVE: f64 = 0.05;
/// ADR 0760, VM: Ladson's 80-grit maximum and the band on the maximum lift.
const CL_MAX_LADSON: f64 = 1.6116;
const CL_MAX_BAND: f64 = 0.05;
/// ADR 0167 D5: steady when the window range of cl_eff and of cd_eff are at most these.
const D5_CL: f64 = 0.005;
const D5_CD: f64 = 0.0005;
/// ADR 0167's mesh M1w-far500 (its check P2).
const MESH_SHA: &str = "59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889";
/// Iterations of the fast runs (the cap that replaces the case's 8000).
const FT_STEPS: usize = 600;
/// FT4n: cl_eff after 600 iterations of the first run of the fast test at 4 degrees on the build of this work (main
/// d6e44b27), a record of the code's own output on the coarse mesh and not a lift value.
const R4N: f64 = 0.3293069785;
/// ADR 0760's R4, the same fast run with the default relax_nut 0.5 (printed beside R4N, no limit).
const R4_DEFAULT: f64 = 0.3292877732;
/// The line the cases add to ADR 0760's.
const ADDED: (&str, &str) = ("relax_nut", "1.0");

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn folder() -> PathBuf {
    repo().join("docs/investigations/f887-naca0012-lift-curve-relax-nut")
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&read(&repo().join(rel))).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn results() -> Value {
    json("docs/investigations/f887-naca0012-lift-curve-relax-nut/results.json")
}

fn results760() -> Value {
    json("docs/investigations/f868-naca0012-lift-curve/results.json")
}

fn base_case(alpha: u32) -> String {
    read(&repo().join(format!(
        "docs/investigations/f868-naca0012-lift-curve/cases/SA-a{alpha:02}.toml"
    )))
}

fn id(alpha: u32) -> String {
    format!("RN-a{alpha:02}")
}

fn case_file(alpha: u32) -> String {
    read(&folder().join("cases").join(format!("{}.toml", id(alpha))))
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// Ladson's 80-grit zone: (alpha in degrees, cl) from the committed table.
fn ladson_80() -> Vec<(f64, f64)> {
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
    rows
}

fn interp(table: &[(f64, f64)], a: f64) -> f64 {
    for w in table.windows(2) {
        let ((a0, c0), (a1, c1)) = (w[0], w[1]);
        if a0 <= a && a <= a1 {
            return c0 + (c1 - c0) * (a - a0) / (a1 - a0);
        }
    }
    panic!("{a} degrees is outside the table");
}

fn band(cl_ladson: f64) -> f64 {
    (RELATIVE * cl_ladson.abs()).max(SCATTER)
}

fn steady(rcl: f64, rcd: f64) -> bool {
    rcl <= D5_CL && rcd <= D5_CD
}

/// D5 and D7 of a run from its window statistics.
struct Judged {
    rcl: f64,
    rcd: f64,
    steady: bool,
    /// D5: the final iterate when exit 0, the window mean otherwise.
    d5: f64,
    /// D7: the D5 value when steady, the window mean otherwise.
    d7: f64,
    /// ADR 0760: exit 0 or steady.
    flow: bool,
}

fn judge(run: &Value) -> Judged {
    let w = &run["window"];
    let rcl = f(&w["cl_eff"]["max"]) - f(&w["cl_eff"]["min"]);
    let rcd = f(&w["cd_eff"]["max"]) - f(&w["cd_eff"]["min"]);
    let exit = run["exit"].as_i64().expect("exit");
    let st = steady(rcl, rcd);
    let mean = f(&w["cl_eff"]["mean"]);
    let d5 = if exit == 0 { f(&w["cl_eff"]["final"]) } else { mean };
    Judged {
        rcl,
        rcd,
        steady: st,
        d5,
        d7: if st { d5 } else { mean },
        flow: exit == 0 || st,
    }
}

fn run(alpha: u32) -> Value {
    let v = results()["runs"][id(alpha)].clone();
    assert!(v.is_object(), "{} is not in the record", id(alpha));
    v
}

/// The reference of row A: ADR 0760's steady value (its `evaluation.angles` cl) at 0 to 16 degrees, 1.5516 at 17.
fn reference_a(alpha: u32) -> f64 {
    if alpha == 17 {
        return TARGET17;
    }
    let r = results760();
    let row = r["evaluation"]["angles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["alpha"].as_u64() == Some(alpha as u64))
        .unwrap_or_else(|| panic!("ADR 0760 has no angle {alpha}"))
        .clone();
    f(&row["cl"])
}

fn row_a(alpha: u32) {
    let j = judge(&run(alpha));
    let reference = reference_a(alpha);
    assert!(
        j.steady && (j.d5 - reference).abs() <= TOL,
        "A-{alpha}: steady by D5 {} (ranges cl_eff {:.4e}, cd_eff {:.4e}), cl {:.6}, {:+.6} from {reference:.6} (limit {TOL})",
        j.steady,
        j.rcl,
        j.rcd,
        j.d5,
        j.d5 - reference
    );
}

fn row_v(alpha: u32, limit: Option<f64>) {
    let j = judge(&run(alpha));
    let cl_l = interp(&ladson_80(), alpha as f64);
    let b = limit.unwrap_or_else(|| band(cl_l));
    assert!(
        j.flow && (j.d7 - cl_l).abs() <= b,
        "alpha {alpha}: flow value {}, cl {:.5}, {:+.5} from Ladson's {cl_l:.4} (limit {b:.4})",
        j.flow,
        j.d7,
        j.d7 - cl_l
    );
}

macro_rules! rows {
    ($($a:ident, $v:ident, $vs:ident, $alpha:expr;)*) => {$(
        #[test]
        fn $a() {
            row_a($alpha);
        }
        #[test]
        fn $v() {
            row_v($alpha, None);
        }
        #[test]
        fn $vs() {
            row_v($alpha, Some(SCATTER));
        }
    )*};
}

rows! {
    f887_a00_steady_and_within_tol_of_adr0760, f887_v00_within_adr0760_band, f887_vs00_within_the_scatter, 0;
    f887_a02_steady_and_within_tol_of_adr0760, f887_v02_within_adr0760_band, f887_vs02_within_the_scatter, 2;
    f887_a04_steady_and_within_tol_of_adr0760, f887_v04_within_adr0760_band, f887_vs04_within_the_scatter, 4;
    f887_a06_steady_and_within_tol_of_adr0760, f887_v06_within_adr0760_band, f887_vs06_within_the_scatter, 6;
    f887_a08_steady_and_within_tol_of_adr0760, f887_v08_within_adr0760_band, f887_vs08_within_the_scatter, 8;
    f887_a10_steady_and_within_tol_of_adr0760, f887_v10_within_adr0760_band, f887_vs10_within_the_scatter, 10;
    f887_a12_steady_and_within_tol_of_adr0760, f887_v12_within_adr0760_band, f887_vs12_within_the_scatter, 12;
    f887_a13_steady_and_within_tol_of_adr0760, f887_v13_within_adr0760_band, f887_vs13_within_the_scatter, 13;
    f887_a14_steady_and_within_tol_of_adr0760, f887_v14_within_adr0760_band, f887_vs14_within_the_scatter, 14;
    f887_a15_steady_and_within_tol_of_adr0760, f887_v15_within_adr0760_band, f887_vs15_within_the_scatter, 15;
    f887_a16_steady_and_within_tol_of_adr0760, f887_v16_within_adr0760_band, f887_vs16_within_the_scatter, 16;
    f887_a17_steady_and_within_tol_of_adr0764, f887_v17_within_adr0760_band, f887_vs17_within_the_scatter, 17;
}

/// VM: the maximum lift among the flow values within 5 % of Ladson's 80-grit maximum.
#[test]
fn f887_vm_maximum_lift() {
    let (mut a, mut top) = (0, f64::MIN);
    for alpha in ALPHAS {
        let j = judge(&run(alpha));
        if j.flow && j.d7 > top {
            (a, top) = (alpha, j.d7);
        }
    }
    let rel = top / CL_MAX_LADSON - 1.0;
    assert!(
        rel.abs() <= CL_MAX_BAND,
        "VM: cl_max {top:.4} at {a} degrees, {:+.2} %",
        100.0 * rel
    );
}

/// L2: the lift slope over 0 to 8 degrees (least squares with intercept) within 0.90 to 1.05 of 2 pi.
#[test]
fn f887_l2_lift_slope() {
    let pts: Vec<(f64, f64)> = [0u32, 2, 4, 6, 8]
        .iter()
        .map(|&a| ((a as f64).to_radians(), judge(&run(a)).d7))
        .collect();
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let k =
        pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / pts.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
    let factor = k / (2.0 * std::f64::consts::PI);
    assert!((0.90..=1.05).contains(&factor), "L2: {factor}");
}

/// L4: symmetry at 0 degrees.
#[test]
fn f887_l4_symmetry_at_0_degrees() {
    let j = judge(&run(0));
    assert!(j.d7.abs() <= 0.005, "L4: cl {}", j.d7);
}

/// R1: the twelve runs are in the record, each with a finish record.
#[test]
fn f887_r1_the_twelve_runs_are_made() {
    for alpha in ALPHAS {
        let r = run(alpha);
        assert_eq!(r["finished"].as_bool(), Some(true), "{}: no finish record", id(alpha));
        assert!(r["exit"].is_i64(), "{}: no exit code", id(alpha));
    }
}

/// D15, D17: RN-a15 and RN-a17 reproduce ADR 0770's KT-a15 and KT-a17 bit for bit.
fn reproduces(alpha: u32, kt: &str) {
    let r = run(alpha);
    let old = json("docs/investigations/f884-naca0012-17-steady/results.json");
    let old = &old["runs"][kt];
    assert_eq!(r["iterations"], old["iterations"], "iterations");
    assert_eq!(r["exit"], old["exit"], "exit");
    for m in ["cl_eff", "cd_eff"] {
        for q in ["mean", "min", "max", "final"] {
            assert_eq!(
                f(&r["window"][m][q]).to_bits(),
                f(&old["window"][m][q]).to_bits(),
                "{m} {q}: {} against {}",
                r["window"][m][q],
                old["window"][m][q]
            );
        }
    }
}

#[test]
fn f887_d15_reproduces_adr0770_kt_a15() {
    reproduces(15, "KT-a15");
}

#[test]
fn f887_d17_reproduces_adr0770_kt_a17() {
    reproduces(17, "KT-a17");
}

/// P2: the mesh of the record is ADR 0167's.
#[test]
fn f887_p2_the_mesh_is_adr0167s() {
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

/// T1: each committed case is ADR 0760's case of its angle with only `relax_nut = 1.0` added, names its angle, and equals
/// the text recorded in `results.json`.
#[test]
fn f887_t1_the_cases_are_the_registered_setup() {
    let r = results();
    for alpha in ALPHAS {
        let text = case_file(alpha);
        let mut added = body(&text);
        for b in body(&base_case(alpha)) {
            let i = added
                .iter()
                .position(|x| *x == b)
                .unwrap_or_else(|| panic!("{}: line {b:?} of ADR 0760's case is missing or changed", id(alpha)));
            added.remove(i);
        }
        assert_eq!(
            added,
            vec![(ADDED.0.to_string(), ADDED.1.to_string())],
            "{}: the added lines",
            id(alpha)
        );
        let angle = if alpha == 0 {
            "rotate_z_deg = 0.0 ".to_string()
        } else {
            format!("rotate_z_deg = -{alpha} ")
        };
        assert!(text.contains(&angle), "{}: the angle", id(alpha));
        assert!(text.starts_with(&format!("# ADR 0772 (F-887) run {}:", id(alpha))));
        assert_eq!(
            r["runs"][id(alpha)]["case"].as_str(),
            Some(text.as_str()),
            "{}: the recorded text",
            id(alpha)
        );
    }
}

/// T2: the record is consistent, and the driver's verdicts follow from the statistics by D5 and D7.
#[test]
fn f887_t2_the_record_is_consistent() {
    let table = ladson_80();
    for alpha in ALPHAS {
        let r = run(alpha);
        let n = r["iterations"].as_u64().unwrap();
        let nw = r["window_iterations"].as_u64().unwrap();
        assert_eq!(nw, 1000.min(n / 2), "{alpha}");
        for (m, col) in [("cl_eff", 1), ("cd_eff", 2)] {
            let w = &r["window"][m];
            let (lo, mean, hi) = (f(&w["min"]), f(&w["mean"]), f(&w["max"]));
            assert!(lo <= mean && mean <= hi, "{alpha} {m}");
            for h in r["history"].as_array().unwrap() {
                if h[0].as_u64().unwrap() > n - nw {
                    let v = f(&h[col]);
                    assert!(lo <= v && v <= hi, "{alpha} {m}: history {v} outside [{lo}, {hi}]");
                }
            }
        }
        let j = judge(&r);
        let v = &r["verdict"];
        let cl_l = interp(&table, alpha as f64);
        let reference = reference_a(alpha);
        assert_eq!(v["steady"].as_bool(), Some(j.steady), "{alpha}: steady");
        assert_eq!(v["flow_value"].as_bool(), Some(j.flow), "{alpha}: flow value");
        assert_eq!(f(&v["d5_value"]).to_bits(), j.d5.to_bits(), "{alpha}: D5 value");
        assert_eq!(f(&v["cl"]).to_bits(), j.d7.to_bits(), "{alpha}: D7 value");
        assert!((f(&v["reference_A"]) - reference).abs() <= 1e-12, "{alpha}: reference");
        assert!((f(&v["cl_ladson"]) - cl_l).abs() <= 1e-12, "{alpha}: Ladson");
        assert_eq!(
            v["A_met"].as_bool(),
            Some(j.steady && (j.d5 - reference).abs() <= TOL),
            "{alpha}: A"
        );
        assert_eq!(
            v["V_met"].as_bool(),
            Some(j.flow && (j.d7 - cl_l).abs() <= band(cl_l)),
            "{alpha}: V"
        );
        assert_eq!(
            v["VS_met"].as_bool(),
            Some(j.flow && (j.d7 - cl_l).abs() <= SCATTER),
            "{alpha}: VS"
        );
    }
}

/// NC1: the tolerance of row A rejects the reference of each angle judged against that of the next angle (at least 10
/// of the 11 pairs; 16 and 17 degrees differ by 0.0096, arithmetic 4 of ADR 0772).
#[test]
fn f887_nc1_the_tolerance_tells_neighbouring_angles_apart() {
    let refs: Vec<f64> = ALPHAS.iter().map(|&a| reference_a(a)).collect();
    let rejected = refs.windows(2).filter(|w| (w[0] - w[1]).abs() > TOL).count();
    assert!(rejected >= 10, "NC1: {rejected} of 11 rejected");
}

/// NC2: each limit of D5 alone rejects a window (the cl_eff range of ADR 0760's SA-a17 with the cd_eff range of its
/// SA-a10, and the reverse).
#[test]
fn f887_nc2_each_limit_of_d5_rejects_alone() {
    let r = results760();
    let range = |id: &str, m: &str| f(&r["runs"][id][m]["max"]) - f(&r["runs"][id][m]["min"]);
    let (cl17, cd17) = (range("SA-a17", "cl_eff"), range("SA-a17", "cd_eff"));
    let (cl10, cd10) = (range("SA-a10", "cl_eff"), range("SA-a10", "cd_eff"));
    assert!(steady(cl10, cd10), "SA-a10 is the steady window of this control");
    assert!(!steady(cl17, cd10), "cl_eff range {cl17} with cd_eff range {cd10}");
    assert!(!steady(cl10, cd17), "cl_eff range {cl10} with cd_eff range {cd17}");
}

/// NC3: ADR 0760's band rejects this work's curve at 0 to 16 degrees with cl multiplied by 1.1 at least 8 of 11 times.
#[test]
fn f887_nc3_the_band_rejects_a_reference_area_wrong_by_ten_percent() {
    let table = ladson_80();
    let mut rejected = 0;
    for alpha in ALPHAS.iter().copied().filter(|&a| a <= 16) {
        let j = judge(&run(alpha));
        let cl_l = interp(&table, alpha as f64);
        if !(j.flow && (1.1 * j.d7 - cl_l).abs() <= band(cl_l)) {
            rejected += 1;
        }
    }
    assert!(rejected >= 8, "NC3: {rejected} of 11 rejected");
}

fn fast_case(alpha: u32) -> Case {
    let mesh = repo()
        .join("example-projects/wind-tunnel/meshes/naca0012-ogrid")
        .canonicalize()
        .unwrap();
    let mesh = mesh.to_string_lossy().replace('\\', "/");
    let mesh = mesh.strip_prefix("//?/").unwrap_or(&mesh).to_string();
    let mut text = case_file(alpha);
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

/// cl_eff after at most 600 iterations of the committed case of `alpha` on the coarse mesh, and the iterations taken.
fn fast_cl(alpha: u32) -> (f64, usize) {
    let mut s = Simulation::new(&fast_case(alpha)).unwrap_or_else(|e| panic!("{e:?}"));
    let steps = s.run(FT_STEPS);
    let cl = s
        .step_report_values()
        .into_iter()
        .find(|(n, _)| n == "cl_eff")
        .expect("the report cl_eff")
        .1;
    println!(
        "FT {}: cl_eff {cl:.10} after {steps} iterations, converged {}",
        id(alpha),
        s.is_converged()
    );
    (cl, steps)
}

/// FT0n: the coarse run at 0 degrees with relax_nut = 1.0 is symmetric.
#[test]
fn f887_ft0n_fast_run_is_symmetric() {
    let (cl, _) = fast_cl(0);
    assert!(cl.abs() <= 0.005, "FT0n: cl_eff {cl}");
}

/// FT4n and FT4nu: the coarse run at 4 degrees with relax_nut = 1.0 keeps its recorded lift and lies in the slope band.
#[test]
fn f887_ft4n_fast_run_keeps_its_lift() {
    let (cl, steps) = fast_cl(4);
    println!(
        "FT4n: R4n {R4N}; ADR 0760's R4 with the default setting {R4_DEFAULT}, difference {:+.6}",
        cl - R4_DEFAULT
    );
    let upper = 1.05 * 2.0 * std::f64::consts::PI * 4f64.to_radians();
    assert!(cl > 0.0 && cl <= upper, "FT4nu: cl_eff {cl} outside (0, {upper:.4}]");
    assert!(
        (cl - R4N).abs() <= TOL,
        "FT4n: cl_eff {cl} after {steps} iterations differs from the recorded {R4N} by more than {TOL}"
    );
}
