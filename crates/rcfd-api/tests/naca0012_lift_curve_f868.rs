//! ADR 0760 (F-868): the NACA 0012 lift curve at Re 6e6 (steady Spalart-Allmaras, mesh M1w-far500, ADR 0167's SA arm on
//! current main) against Ladson's measured lift, by the registered band
//! `abs(cl - cl_L(alpha)) <= max(0.05 abs(cl_L(alpha)), 0.018)` at twelve angles from 0 to 17 degrees.
//!
//! Three parts. (1) The band on the heavy record: the committed `docs/investigations/f868-naca0012-lift-curve/results.json`
//! (written by `scripts/f868_naca0012_lift_curve.py collect` from the twelve runs of ADR 0167's driver) is judged here
//! again, independently of the driver, against the committed 80-grit table. No solver runs in that part: the sweep is a
//! heavy record made on demand. (2) The fast test on the same path, which does run the solver: the committed case files
//! of 0 and 4 degrees on the committed coarse O-grid of the example project, three lines replaced. (3) Negative controls:
//! the band must reject the curve with the angle of attack offset by one degree and with the reference area wrong by 10 %.

use std::path::{Path, PathBuf};

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::Value;

/// The scatter of the measurement: the largest spread of Ladson's three grit zones from 0 to 17 degrees, rounded up (S1).
const SCATTER: f64 = 0.018;
/// ADR 0167, LT10 and LT15: cl within 5 %.
const RELATIVE: f64 = 0.05;
/// The angles compared.
const ALPHAS: [f64; 12] = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0];
/// ADR 0167's mesh M1w-far500 (its check P2).
const MESH_SHA: &str = "59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889";
/// FT4: `cl_eff` of the first run of the fast test at 4 degrees on the build of this work (main 13b0937d), a record of
/// the code's own output and not a measured value; the run took its 600 steps without meeting the residual targets.
const R4: f64 = 0.3292877732;
/// Steps of the fast runs (the cap that replaces the case's 8000).
const FT_STEPS: usize = 600;

/// `CLCD_Ladson_expdata.dat`, zone "120 grit": (alpha in degrees, cl). Recorded in `docs/reference-data.d/0760-F-868.part`.
const LADSON_120: [(f64, f64); 18] = [
    (-4.01, -0.4466),
    (-2.12, -0.2425),
    (-0.01, -0.0120),
    (0.01, -0.0122),
    (2.15, 0.2236),
    (4.11, 0.4397),
    (6.01, 0.6487),
    (8.08, 0.8701),
    (10.10, 1.0775),
    (11.23, 1.1849),
    (12.13, 1.2720),
    (13.26, 1.3699),
    (14.30, 1.4571),
    (15.27, 1.5280),
    (16.16, 1.5838),
    (17.24, 1.6347),
    (18.18, 1.1886),
    (19.25, 1.1888),
];
/// The same file, zone "180 grit".
const LADSON_180: [(f64, f64); 18] = [
    (-3.99, -0.4363),
    (-1.98, -0.2213),
    (-0.03, -0.0115),
    (0.04, -0.0013),
    (2.00, 0.2213),
    (4.06, 0.4365),
    (6.09, 0.6558),
    (8.09, 0.8689),
    (10.18, 1.0809),
    (11.13, 1.1731),
    (12.10, 1.2644),
    (13.31, 1.3676),
    (14.08, 1.4316),
    (15.24, 1.5169),
    (16.33, 1.5855),
    (17.13, 1.6219),
    (18.21, 1.0104),
    (19.27, 1.0664),
];

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn folder() -> PathBuf {
    repo().join("docs/investigations/f868-naca0012-lift-curve")
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .replace("\r\n", "\n")
}

fn results() -> Value {
    serde_json::from_str(&read(&folder().join("results.json"))).expect("results.json parses")
}

/// Ladson's 80-grit zone, the reference: (alpha in degrees, cl) from the committed table.
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

/// Linear interpolation in alpha between the rows of a table sorted by alpha.
fn interp(table: &[(f64, f64)], a: f64) -> f64 {
    for w in table.windows(2) {
        let ((a0, c0), (a1, c1)) = (w[0], w[1]);
        if a0 <= a && a <= a1 {
            return c0 + (c1 - c0) * (a - a0) / (a1 - a0);
        }
    }
    panic!("{a} degrees is outside the table");
}

/// The registered band at a reference lift.
fn band(cl_ladson: f64) -> f64 {
    (RELATIVE * cl_ladson.abs()).max(SCATTER)
}

/// A point of the curve: the angle, `cl` (ADR 0167's D5 and D7), and whether it is a flow value (the run met the residual
/// targets, exit code 0, or is steady by D5).
struct Point {
    alpha: f64,
    cl: f64,
    flow: bool,
}

fn curve(r: &Value) -> Vec<Point> {
    r["evaluation"]["curve"]
        .as_array()
        .expect("curve")
        .iter()
        .map(|p| Point {
            alpha: p["alpha"].as_f64().unwrap(),
            cl: p["cl"].as_f64().unwrap(),
            flow: p["exit"].as_i64().unwrap() == 0 || p["steady"].as_bool().unwrap(),
        })
        .collect()
}

fn at(c: &[Point], a: f64) -> &Point {
    c.iter()
        .find(|p| p.alpha == a)
        .unwrap_or_else(|| panic!("no curve value at {a} degrees"))
}

/// Is `cl`, computed at some angle, outside the band when judged at `alpha`?
fn outside(table: &[(f64, f64)], alpha: f64, cl: f64) -> bool {
    let reference = interp(table, alpha);
    (cl - reference).abs() > band(reference)
}

/// Rows V00 to V17: the lift at `alpha` is a flow value and within the band of Ladson's 80-grit lift.
fn lift_within_the_band(alpha: f64) {
    let r = results();
    let c = curve(&r);
    let p = at(&c, alpha);
    let table = ladson_80();
    let reference = interp(&table, alpha);
    let (d, b) = (p.cl - reference, band(reference));
    println!(
        "V{alpha:02}: cl {:.4}, Ladson {reference:.4}, difference {d:+.4}, band {b:.4}, flow value {}",
        p.cl, p.flow
    );
    assert!(
        p.flow,
        "{alpha} degrees: the run neither met the residual targets nor is steady; {:.4} is an iteration mean",
        p.cl
    );
    assert!(
        d.abs() <= b,
        "{alpha} degrees: cl {:.4} differs from {reference:.4} by {d:+.4}, band {b:.4}",
        p.cl
    );
}

#[test]
fn f868_v00_lift_within_the_band_at_0_degrees() {
    lift_within_the_band(0.0);
}

#[test]
fn f868_v02_lift_within_the_band_at_2_degrees() {
    lift_within_the_band(2.0);
}

#[test]
fn f868_v04_lift_within_the_band_at_4_degrees() {
    lift_within_the_band(4.0);
}

#[test]
fn f868_v06_lift_within_the_band_at_6_degrees() {
    lift_within_the_band(6.0);
}

#[test]
fn f868_v08_lift_within_the_band_at_8_degrees() {
    lift_within_the_band(8.0);
}

#[test]
fn f868_v10_lift_within_the_band_at_10_degrees() {
    lift_within_the_band(10.0);
}

#[test]
fn f868_v12_lift_within_the_band_at_12_degrees() {
    lift_within_the_band(12.0);
}

#[test]
fn f868_v13_lift_within_the_band_at_13_degrees() {
    lift_within_the_band(13.0);
}

#[test]
fn f868_v14_lift_within_the_band_at_14_degrees() {
    lift_within_the_band(14.0);
}

#[test]
fn f868_v15_lift_within_the_band_at_15_degrees() {
    lift_within_the_band(15.0);
}

#[test]
fn f868_v16_lift_within_the_band_at_16_degrees() {
    lift_within_the_band(16.0);
}

/// Known miss (ADR 0760 addendum 1): at 17 degrees the steady iteration does not converge (cl swings between -0.07 and
/// 1.95 over the last 1000 of 8000 iterations); the iteration mean 1.0476 is not a flow value and is 0.558 under Ladson's
/// 1.6057 (band 0.0803). The measured aerofoil is still attached at 17 degrees; steady SA on this mesh has stalled.
#[test]
#[ignore = "known miss, ADR 0760: at 17 degrees no steady value; the iteration mean 1.0476 is 0.558 under Ladson's 1.6057, band 0.0803"]
fn f868_v17_lift_within_the_band_at_17_degrees_known_miss() {
    lift_within_the_band(17.0);
}

/// VM: the largest lift among the flow values is within 5 % of Ladson's 80-grit maximum.
#[test]
fn f868_vm_maximum_lift_within_5_percent() {
    let r = results();
    let c = curve(&r);
    let table = ladson_80();
    let cl_max_ladson = table.iter().map(|t| t.1).fold(f64::MIN, f64::max);
    assert_eq!(cl_max_ladson, 1.6116);
    let top = c
        .iter()
        .filter(|p| p.flow && ALPHAS.contains(&p.alpha))
        .max_by(|a, b| a.cl.total_cmp(&b.cl))
        .expect("a flow value");
    let rel = top.cl / cl_max_ladson - 1.0;
    println!(
        "VM: cl_max {:.4} at {} degrees, {:+.2} % against {cl_max_ladson}",
        top.cl,
        top.alpha,
        100.0 * rel
    );
    assert!(rel.abs() <= 0.05, "cl_max {:.4}: {rel:+.4} outside 5 %", top.cl);
}

/// L2 (ADR 0167): the least-squares slope through 0, 2, 4, 6, 8 degrees, as a factor of 2 pi, within 0.90 to 1.05.
#[test]
fn f868_l2_lift_slope_within_the_band() {
    let r = results();
    let c = curve(&r);
    let pts: Vec<(f64, f64)> = [0.0, 2.0, 4.0, 6.0, 8.0]
        .iter()
        .map(|&a| (f64::to_radians(a), at(&c, a).cl))
        .collect();
    let n = pts.len() as f64;
    let (mx, my) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let factor = sxy / sxx / (2.0 * std::f64::consts::PI);
    println!("L2: slope factor {factor:.5}");
    assert!(
        (0.90..=1.05).contains(&factor),
        "slope factor {factor} outside [0.90, 1.05]"
    );
}

/// L4 (ADR 0167): no lift at zero incidence.
#[test]
fn f868_l4_no_lift_at_zero_incidence() {
    let r = results();
    let cl0 = at(&curve(&r), 0.0).cl;
    println!("L4: cl(0) = {cl0:.3e}");
    assert!(cl0.abs() <= 0.005, "cl(0) = {cl0}");
}

/// R1 and P2: twelve runs, each ended with exit code 0 or 5, on ADR 0167's mesh.
#[test]
fn f868_r1_p2_twelve_complete_runs_on_the_registered_mesh() {
    let r = results();
    let runs = r["runs"].as_object().expect("runs");
    assert_eq!(runs.len(), ALPHAS.len());
    for (id, run) in runs {
        let exit = run["exit"].as_i64().unwrap();
        assert!(exit == 0 || exit == 5, "{id}: exit code {exit}");
    }
    let c = curve(&r);
    for a in ALPHAS {
        at(&c, a);
    }
    assert_eq!(r["evaluation"]["P2"]["sha256"].as_str().unwrap(), MESH_SHA);
    assert_eq!(r["mesh"].as_str().unwrap(), "M1w-far500");
}

fn case_id(alpha: f64) -> String {
    format!("SA-a{:02}", alpha as i64)
}

fn case_file(alpha: f64) -> String {
    read(&folder().join("cases").join(case_id(alpha) + ".toml"))
}

/// The value of the one line `key = value   # comment` of a case text.
fn line_value<'a>(text: &'a str, key: &str) -> &'a str {
    let start = format!("{key} = ");
    let mut found = text.lines().filter(|l| l.starts_with(&start));
    let l = found.next().unwrap_or_else(|| panic!("no line {key}"));
    assert!(found.next().is_none(), "more than one line {key}");
    l[start.len()..].split('#').next().unwrap().trim()
}

/// T1: the committed case files are the texts the runs used; each names its angle, the model and the mesh.
#[test]
fn f868_t1_committed_cases_are_those_of_the_record() {
    let r = results();
    assert_eq!(r["model"].as_str().unwrap(), "spalart_allmaras");
    for a in ALPHAS {
        let (id, text) = (case_id(a), case_file(a));
        assert_eq!(
            r["cases"][&id].as_str().unwrap(),
            text,
            "{id}: the committed case differs from the record"
        );
        let rot: f64 = line_value(&text, "rotate_z_deg").parse().unwrap();
        assert_eq!(rot, -a, "{id}: rotate_z_deg");
        assert_eq!(line_value(&text, "model"), "\"spalart_allmaras\"", "{id}");
        assert_eq!(line_value(&text, "file"), "\"../meshes/M1w-far500\"", "{id}");
        assert_eq!(line_value(&text, "turbulence"), "\"freestream\"", "{id}");
        assert_eq!(text.matches("component = \"total_effective\"").count(), 2, "{id}");
        assert_eq!(text.matches("a_ref = 0.1\n").count(), 6, "{id}");
    }
}

/// S1: the scatter 0.018 is the largest spread of the three zones of the measured file at the twelve angles, rounded up.
#[test]
fn f868_s1_scatter_of_the_three_grit_zones() {
    let l80 = ladson_80();
    let mut worst = 0.0f64;
    for a in ALPHAS {
        let v = [interp(&l80, a), interp(&LADSON_120, a), interp(&LADSON_180, a)];
        let spread = v.iter().fold(f64::MIN, |m, x| m.max(*x)) - v.iter().fold(f64::MAX, |m, x| m.min(*x));
        println!(
            "S1: {a:4} degrees: {:.4} {:.4} {:.4}, spread {spread:.4}",
            v[0], v[1], v[2]
        );
        worst = worst.max(spread);
    }
    println!("S1: largest spread {worst:.5}");
    assert!(worst <= SCATTER, "largest spread {worst} above {SCATTER}");
    assert!(
        worst > SCATTER - 0.001,
        "{SCATTER} is not the largest spread {worst} rounded up"
    );
}

/// B1: the folder of the record stays small.
#[test]
fn f868_b1_storage_budget() {
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
    assert!(total <= 200_000, "{total} bytes");
}

/// NC1: with the angle of attack offset by one degree (the value computed at alpha judged at alpha - 1) the band rejects
/// the curve at 8 or more of the 11 angles from 0 to 16 degrees.
#[test]
fn f868_nc1_an_angle_offset_of_one_degree_is_rejected() {
    let r = results();
    let c = curve(&r);
    let table = ladson_80();
    let out: Vec<f64> = ALPHAS
        .iter()
        .copied()
        .filter(|&a| a <= 16.0 && outside(&table, a - 1.0, at(&c, a).cl))
        .collect();
    println!("NC1: outside the band at {out:?}");
    assert!(
        out.len() >= 8,
        "an offset of one degree is rejected at only {} angles: {out:?}",
        out.len()
    );
}

/// NC2: with the reference area wrong by 10 % (cl multiplied by 1.1) the band rejects the curve at 8 or more of the 11
/// angles from 0 to 16 degrees.
#[test]
fn f868_nc2_a_reference_area_wrong_by_10_percent_is_rejected() {
    let r = results();
    let c = curve(&r);
    let table = ladson_80();
    let out: Vec<f64> = ALPHAS
        .iter()
        .copied()
        .filter(|&a| a <= 16.0 && outside(&table, a, 1.1 * at(&c, a).cl))
        .collect();
    println!("NC2: outside the band at {out:?}");
    assert!(
        out.len() >= 8,
        "a 10 % error is rejected at only {} angles: {out:?}",
        out.len()
    );
}

/// The committed case file of `alpha` on the committed coarse O-grid of the example project (6144 cells, far field at 20
/// chords), with exactly three lines replaced: the mesh file and the two iteration counts.
fn fast_case(alpha: f64) -> Case {
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

/// `cl_eff` of the last step of the fast run at `alpha`, and the steps taken.
fn fast_lift(alpha: f64) -> (f64, usize) {
    let mut s = Simulation::new(&fast_case(alpha)).unwrap_or_else(|e| panic!("{e:?}"));
    // the same steps as one `run(FT_STEPS)`; the value 150 steps before the end is printed to show how settled the run is
    let before = s.run(FT_STEPS - 150);
    let earlier = s
        .step_report_values()
        .into_iter()
        .find(|(n, _)| n == "cl_eff")
        .map(|v| v.1);
    let steps = before + s.run(150);
    let cl = s
        .step_report_values()
        .into_iter()
        .find(|(n, _)| n == "cl_eff")
        .expect("the report cl_eff")
        .1;
    println!(
        "FT: alpha {alpha}: cl_eff {cl:.10} after {steps} steps, converged {}; after {before} steps {earlier:?}",
        s.is_converged()
    );
    (cl, steps)
}

/// FT0: the same path on the coarse mesh at 0 degrees gives no lift (ADR 0167's L4).
#[test]
fn f868_ft0_fast_run_at_0_degrees_has_no_lift() {
    let (cl, _) = fast_lift(0.0);
    assert!(cl.abs() <= 0.005, "cl_eff(0) = {cl}");
}

/// FT4 and FT4u: the same path on the coarse mesh at 4 degrees gives the recorded lift within the scatter 0.018, and a
/// positive lift not above the upper edge of ADR 0167's slope band.
#[test]
fn f868_ft4_fast_run_at_4_degrees_keeps_its_lift() {
    let (cl, _) = fast_lift(4.0);
    let upper = 1.05 * 2.0 * std::f64::consts::PI * f64::to_radians(4.0);
    assert!(cl > 0.0 && cl <= upper, "cl_eff(4) = {cl} outside (0, {upper:.4}]");
    assert!(
        (cl - R4).abs() <= SCATTER,
        "cl_eff(4) = {cl} differs from the recorded {R4} by more than {SCATTER}"
    );
}
