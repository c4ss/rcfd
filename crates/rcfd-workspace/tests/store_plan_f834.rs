//! F-834: the start plan's engine checks that went with the egui Studio (F-449), on the workspace's plan
//! (`rcfd_workspace::plan`, what `start_run` applies): plan parity over every shipped example (ADR 0186 C-P, F-271),
//! every shipped case planning a store and the cell counts (ADR 0140 SF1, SF3), and a case's own output settings (ADR
//! 0140 amendment 2, SF9 (j) to (l), with ADR 0152's (k)). The Studio's start-dialog lines, size estimates, warnings
//! and fixes (`rcfd_studio::store_plan`) were Studio code and are not ported (F-837).

mod f834;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_workspace::examples::EXAMPLES;
use rcfd_workspace::plan::{parse_case, Every, Plan};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

/// What `new_simulations_f271.rs` compared: the `--set` entries, the snapshot count, the size estimate, "no store", and
/// the note.
type Seen = (Vec<String>, Option<u64>, Option<u64>, bool, Option<String>);

fn workspace(text: &str, dir: &Path, set: &[&str]) -> Seen {
    let case = parse_case(text, dir, None).unwrap_or_else(|e| panic!("{e:?}"));
    let set: Vec<String> = set.iter().map(|s| s.to_string()).collect();
    let p = Plan::with_set(case, &set).unwrap();
    (
        p.entries(),
        p.snapshots(),
        p.estimated_bytes(),
        p.every.is_none(),
        p.note.clone(),
    )
}

fn seen(entries: &[&str], snapshots: Option<u64>, bytes: Option<u64>, note: Option<&str>) -> Seen {
    (
        entries.iter().map(|s| s.to_string()).collect(),
        snapshots,
        bytes,
        entries.is_empty(),
        note.map(String::from),
    )
}

const F32: &str = "output.store_precision=f32";
const OWN: &str = "the case sets its own field store (output.store_*)";

/// The Studio's start-dialog plan (`rcfd_studio::runs::StartPlan::new`) of every shipped example and of the nine
/// variants, as `new_simulations_f271.rs::cp_the_workspace_plans_the_store_as_the_studio_does` printed it (and asserted
/// it equal to the workspace's) when run on an archive copy of 44e63df8, the last commit with the Studio.
fn studio_seen(what: &str) -> Seen {
    match what {
        "channel/kepsilon_channel.toml" => seen(&["output.store_interval=80", F32], Some(100), Some(89_600), None),
        "sod/sod.toml" => seen(
            &["output.store_time_interval=0.002", F32],
            Some(100),
            Some(160_000),
            None,
        ),
        "rotating_channel/rotating_channel.toml" => {
            seen(&["output.store_interval=17", F32], Some(100), Some(256_000), None)
        }
        "channel/sst_channel.toml" => seen(&["output.store_interval=90", F32], Some(100), Some(384_000), None),
        "channel/kepsilon_inlet.toml" => seen(&["output.store_interval=4", F32], Some(100), Some(460_800), None),
        "packed_bed/packed_bed.toml" => seen(&["output.store_interval=17", F32], Some(100), Some(512_000), None),
        "channel/profile_inlet.toml" => seen(&["output.store_interval=20", F32], Some(100), Some(768_000), None),
        "wedge/wedge.toml" => seen(
            &["output.store_time_interval=0.03", F32],
            Some(100),
            Some(1_920_000),
            None,
        ),
        "axial_fan_cgrid/axial_fan_cgrid.toml" | "axial_fan_sector/axial_fan_sector.toml" => {
            seen(&["output.store_interval=60", F32], Some(100), None, None)
        }
        "steady" | "steady as JSON" => seen(&["output.store_interval=6", F32], Some(100), Some(230_400), None),
        "steady 250" => seen(&["output.store_interval=3", F32], Some(84), Some(193_536), None),
        "piso end_time" => seen(
            &["output.store_time_interval=0.003", F32],
            Some(100),
            Some(230_400),
            None,
        ),
        "piso no end_time" => seen(
            &["output.store_time_interval=0.01", F32],
            Some(100),
            Some(230_400),
            None,
        ),
        "dynamic mesh" => seen(
            &[],
            None,
            None,
            Some("no field store: the store does not support dynamic_mesh.enabled"),
        ),
        "own store_interval" | "own store_time_interval" => seen(&[], None, None, Some(OWN)),
        "own precision" => seen(&["output.store_interval=6"], Some(100), Some(230_400), None),
        other => panic!("no Studio plan measured for {other}"),
    }
}

fn cavity(alg: &str, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\n{extra}"
    )
}

/// `piston("", "")` of `crates/rcfd-api/tests/dynamic_mesh.rs` (as the deleted `start_plan_r10.rs`).
const PISTON: &str = r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 0.5

[dynamic_mesh]
enabled = true
moving_patches = ["xmin"]
deformation = "smoothing"
displacement_expression_x = "1 [m/s^2] * t^2"
fixed_patches = ["xmax"]
"#;

/// Replaces `crates/rcfd-studio/tests/new_simulations_f271.rs::cp_the_workspace_plans_the_store_as_the_studio_does`
/// (ADR 0186 C-P): the workspace plans every shipped example and the nine variants as the Studio's start dialog did
/// (its values measured on 44e63df8, [`studio_seen`]), and plans for a run's `set` override as for the edited text.
#[test]
fn cp_the_workspace_plans_every_shipped_example_as_the_studio_did() {
    let repo = f834::repo().join("cases");
    let mut n = 0;
    assert_eq!(EXAMPLES.len(), 10, "the shipped examples");
    for e in EXAMPLES {
        let file = repo.join(e.source);
        let text = std::fs::read_to_string(&file).unwrap();
        let w = workspace(&text, file.parent().unwrap(), &[]);
        eprintln!("C-P {}: {:?}", e.source, w);
        assert_eq!(w, studio_seen(e.source), "{}", e.source);
        n += 1;
    }
    let steady = cavity("simplec", "max_iterations = 600\n");
    let steady_json = serde_json::to_string(&toml::from_str::<Value>(&steady).unwrap()).unwrap();
    let variants = [
        ("steady", steady.clone()),
        ("steady 250", cavity("simplec", "max_iterations = 250\n")),
        (
            "piso end_time",
            cavity("piso", "max_iterations = 400\nend_time = 0.3\ndt = 0.01\n"),
        ),
        ("piso no end_time", cavity("piso", "max_iterations = 400\ndt = 0.01\n")),
        ("dynamic mesh", PISTON.to_string()),
        (
            "own store_interval",
            cavity("simplec", "max_iterations = 600\n[output]\nstore_interval = 5\n"),
        ),
        (
            "own store_time_interval",
            cavity(
                "piso",
                "end_time = 0.3\ndt = 0.01\n[output]\nstore_time_interval = 0.05\n",
            ),
        ),
        (
            "own precision",
            cavity("simplec", "max_iterations = 600\n[output]\nstore_precision = \"f32\"\n"),
        ),
        ("steady as JSON", steady_json),
    ];
    let here = Path::new(".");
    let mut verdicts = Vec::new();
    for (what, text) in &variants {
        let w = workspace(text, here, &[]);
        eprintln!("C-P {what}: {:?}", w);
        assert_eq!(w, studio_seen(what), "{what}");
        verdicts.push(w.3);
        n += 1;
    }
    // the variants reach both verdicts and both rules
    assert_eq!(verdicts.iter().filter(|x| **x).count(), 3, "{verdicts:?}");
    // an override is planned for as the edited text is (the Studio planned the edited text so)
    let edited = steady.replace("max_iterations = 600", "max_iterations = 40");
    let with = workspace(&steady, here, &["solver.max_iterations=40"]);
    assert_eq!(with, workspace(&edited, here, &[]));
    assert_eq!(with.0, ["output.store_interval=1", F32]);
    // control: without the override the plans differ (6 against 1)
    assert_ne!(workspace(&steady, here, &[]), workspace(&edited, here, &[]));
    eprintln!(
        "C-P: {n} texts as the Studio planned them; override planned as the edited text: {:?}",
        with.0
    );
}

/// The registered `set()` of every shipped case (SF1), before `output.store_precision=f32`.
const EXPECTED: [(&str, &str); 11] = [
    ("cases/axial_fan_cgrid/axial_fan_cgrid.toml", "output.store_interval=60"),
    (
        "cases/axial_fan_sector/axial_fan_sector.toml",
        "output.store_interval=60",
    ),
    ("cases/cavity/adapt_re100.toml", "output.store_interval=20"),
    ("cases/channel/kepsilon_channel.toml", "output.store_interval=80"),
    ("cases/channel/kepsilon_inlet.toml", "output.store_interval=4"),
    ("cases/channel/profile_inlet.toml", "output.store_interval=20"),
    ("cases/channel/sst_channel.toml", "output.store_interval=90"),
    ("cases/packed_bed/packed_bed.toml", "output.store_interval=17"),
    (
        "cases/rotating_channel/rotating_channel.toml",
        "output.store_interval=17",
    ),
    ("cases/sod/sod.toml", "output.store_time_interval=0.002"),
    ("cases/wedge/wedge.toml", "output.store_time_interval=0.03"),
];

fn plan_of(rel: &str, path: &Path) -> Plan {
    let text = std::fs::read_to_string(path).unwrap();
    Plan::of(&parse_case(&text, path.parent().unwrap(), None).unwrap_or_else(|d| panic!("{rel}: {d:?}")))
}

/// Replaces `crates/rcfd-studio/tests/store_plan_sf.rs::sf1_every_shipped_case_plans_a_store` (ADR 0140 SF1): every
/// shipped case gets a store with its registered entries and no note; the six benchmark descriptors (`cases/**/*.json`)
/// are refused as `rcfd run` refuses them (the Studio's one-line refusal text was its own). Negative control: the 48D
/// rule (ADR 0133, the time rule only for piso and pimple) gives sod the step rule.
#[test]
fn sf1_every_shipped_case_plans_a_store() {
    let cases = f834::shipped("toml");
    let names: Vec<&str> = cases.iter().map(|(r, _)| r.as_str()).collect();
    assert_eq!(names, EXPECTED.map(|(r, _)| r), "the shipped case files");
    for ((rel, path), (_, want)) in cases.iter().zip(EXPECTED) {
        let plan = plan_of(rel, path);
        eprintln!("SF1 {rel}: set {:?}", plan.entries());
        assert!(plan.every.is_some(), "{rel}: no store");
        assert_eq!(plan.entries(), [want, F32], "{rel}");
        assert!(plan.note.is_none(), "{rel}: {:?}", plan.note);
    }
    let descriptors = f834::shipped("json");
    assert_eq!(descriptors.len(), 6, "{descriptors:?}");
    for (rel, path) in &descriptors {
        let text = std::fs::read_to_string(path).unwrap();
        let err = parse_case(&text, path.parent().unwrap(), None).unwrap_err();
        eprintln!("SF1 {rel}: refused: {err:?}");
        assert!(!err.is_empty(), "{rel}");
    }
    // negative control: the 48D rule gives sod the step rule
    let (rel, path) = &cases[9];
    let case = Case::load(path).unwrap();
    let get = |p: &str| case.get(p).unwrap();
    let (alg, end, m) = (
        get("solver.algorithm"),
        get("solver.end_time").as_f64().unwrap(),
        get("solver.max_iterations").as_u64().unwrap(),
    );
    let old = if matches!(alg.as_str(), Some("piso" | "pimple")) && end > 0.0 {
        format!("output.store_time_interval={}", end / 100.0)
    } else {
        format!("output.store_interval={}", m.div_ceil(100).max(1))
    };
    eprintln!("SF1 control: 48D rule for {rel}: {old}");
    assert_eq!(old, "output.store_interval=20");
    assert_ne!(old, EXPECTED[9].1);
}

/// Replaces `crates/rcfd-studio/tests/store_plan_sf.rs::sf3_cell_counts` (ADR 0140 SF3), its engine part: the plan's
/// cell count from the generator table is the built mesh's for every shipped case that has one, and the two fan cases
/// (no table entry: their `nx * ny` is not the count) have none. The Studio counted the fan cases' cells in its run
/// worker; the workspace plan has no such counter (F-837), so that part is not ported.
#[test]
fn sf3_cell_counts() {
    let mut fan = Vec::new();
    for (rel, path) in f834::shipped("toml") {
        let case = Case::load(&path).unwrap();
        let n = rcfd_api::sim::build_mesh(&case).unwrap().n_cells() as u64;
        match plan_of(&rel, &path).cells {
            Some(c) => {
                eprintln!("SF3 {rel}: table {c}, mesh {n}");
                assert_eq!(c, n, "{rel}");
            }
            None => {
                let g = |k: &str| case.get(k).ok().and_then(|v| v.as_u64()).unwrap_or(0);
                let nxny = g("mesh.nx") * g("mesh.ny");
                eprintln!("SF3 {rel}: no table count, mesh {n}; control nx * ny = {nxny}");
                assert_ne!(nxny, n, "{rel}");
                fan.push(rel);
            }
        }
    }
    assert_eq!(
        fan,
        [
            "cases/axial_fan_cgrid/axial_fan_cgrid.toml",
            "cases/axial_fan_sector/axial_fan_sector.toml"
        ]
    );
}

fn with_output(lines: &str) -> String {
    format!("{}\n[output]\n{lines}\n", f834::RUN_CASE)
}

fn cavity_plan(text: &str) -> Plan {
    Plan::of(&parse_case(text, Path::new("."), None).unwrap())
}

const BIG: &str = r#"[mesh]
generator = "box_2d"
nx = 1000
ny = 1000
[physics]
nu = 0.01
[boundaries.xmin]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 2000
"#;

/// Replaces the engine part of `crates/rcfd-studio/tests/store_plan_sf.rs::sf9_start_dialog_lines_warnings_and_fixes`
/// (ADR 0140 SF9 (a), (c) as ADR 0152 changed it, (d)): the entries of the default cavity, of the cavity with
/// `store_interval = 0` (the schema default, so not the case's own) and of the 1000 x 1000 box. The dialog's lines,
/// warnings, fixes and estimates of (a) to (h) were Studio code (`rcfd_studio::store_plan`), not ported.
#[test]
fn sf9_a_c_d_the_plans_behind_the_start_dialog() {
    let a = cavity_plan(f834::RUN_CASE);
    assert_eq!(a.entries(), ["output.store_interval=4", F32]);
    assert_eq!(a.cells, Some(144));
    let c = cavity_plan(&with_output("store_interval = 0"));
    assert_eq!(c, a, "(c): store_interval = 0 is the schema default (ADR 0152)");
    let d = cavity_plan(BIG);
    assert_eq!(d.cells, Some(1_000_000));
    assert_eq!(d.entries(), ["output.store_interval=20", F32]);
    eprintln!(
        "SF9 (a) {:?}, (c) {:?}, (d) {:?}",
        a.entries(),
        c.entries(),
        d.entries()
    );
}

/// Replaces `crates/rcfd-studio/tests/case_output_sf9.rs::sf9_j_the_case_s_own_interval_is_kept` (engine part): the
/// case's own interval is kept, so no entries and no estimate. The dialog's lines and its estimate (348 346 bytes) were
/// Studio code.
#[test]
fn sf9_j_the_case_s_own_interval_is_kept() {
    let p = cavity_plan(&with_output("store_interval = 7"));
    eprintln!("SF9 (j): {p:?}");
    assert_eq!(p.every, None);
    assert!(p.entries().is_empty());
    assert_eq!(p.note.as_deref(), Some(OWN));
    assert_eq!((p.snapshots(), p.estimated_bytes()), (None, None));
}

/// Replaces `crates/rcfd-studio/tests/case_output_sf9.rs::sf9_k_adr0152_f64_at_its_default_is_not_the_case_s_own`
/// (engine part): `"f64"` is the schema default, so the precision is the proposal's f32 and the plan is SF9 (a)'s; the
/// data-only estimate is 230 400 bytes. The Studio's own estimate (342 016) and lines were Studio code; the registered
/// (k) on the route before ADR 0152 (`sf9_k_the_case_s_own_precision_is_kept`) was a Studio switch, not ported.
#[test]
fn sf9_k_f64_at_its_default_is_not_the_case_s_own() {
    let p = cavity_plan(&with_output("store_precision = \"f64\""));
    eprintln!("SF9 (k): {p:?}");
    assert_eq!(p.entries(), ["output.store_interval=4", F32]);
    assert!(!p.sets_precision);
    assert_eq!(p.estimated_bytes(), Some(230_400));
    assert_eq!(p, cavity_plan(f834::RUN_CASE));
}

/// Replaces `crates/rcfd-studio/tests/case_output_sf9.rs::sf9_l_the_case_s_own_fields_are_stored_and_shown` (engine
/// part): the case's own field list leaves the plan as SF9 (a)'s, and three steps in process store exactly `p`, in f32,
/// with no warning. The dialog's label and lines were Studio code.
#[test]
fn sf9_l_the_case_s_own_fields_are_stored() {
    let text = with_output("store_fields = [\"p\"]");
    let p = cavity_plan(&text);
    assert_eq!(p.entries(), ["output.store_interval=4", F32]);
    // the data-only estimate counts four fields
    assert_eq!(p.estimated_bytes(), Some(230_400));
    let dir = f834::temp("sf9-l-run");
    let file = dir.join("cavity.toml");
    std::fs::write(&file, &text).unwrap();
    let mut case = Case::load(&file).unwrap();
    for s in p.entries() {
        apply_set(&mut case, &s).unwrap();
    }
    let opts = RunOptions {
        max_steps: Some(3),
        out_dir: Some(dir.join("output")),
        store_dir: Some(dir.join("fields")),
        status_monitors: true,
        ..Default::default()
    };
    let mut warnings = Vec::new();
    drive(
        &Mutex::new(Simulation::new(&case).unwrap()),
        &opts,
        &|| false,
        &mut |ev| {
            if ev["event"] == "warning" {
                warnings.push(ev["message"].to_string());
            }
        },
    );
    let mut r = rcfd_tsstore::Reader::open(&dir.join("fields")).unwrap();
    let n = r.snapshots().len();
    let f = r.fields(n - 1).unwrap();
    let names: Vec<&str> = f.iter().map(|x| x.name.as_str()).collect();
    eprintln!(
        "SF9 (l): {n} snapshot(s), fields {names:?}, precision {:?}, warnings {warnings:?}",
        f.first().map(|x| x.precision)
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(names, ["p"]);
    assert_eq!(f[0].precision, rcfd_tsstore::Precision::F32);
    assert_eq!(p.every, Some(Every::Steps(4)));
    let _ = std::fs::remove_dir_all(&dir);
}
