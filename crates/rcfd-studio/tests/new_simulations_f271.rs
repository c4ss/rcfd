//! ADR 0186 (F-271) checks on the Studio's side of the project workspace:
//! * C-P: the start plan of the workspace (`rcfd_workspace::plan`, what `start_run` applies) gives the same store as the
//!   Studio's start dialog (`rcfd_studio::runs::StartPlan::new`) for every shipped example and nine variants, and plans
//!   for a run's `set` overrides as the Studio plans for the edited text.
//! * C-S (ADR 0173 C7, the Studio part): a run that the workspace's `start_run` makes from a new case is listed by the
//!   Studio's run poller at its first tick, shown finished once it ended, and its stored fields are read by the Studio's
//!   store reader. Release builds; `rcfd-runner` and `rcfd` next to `rcfd-studio`.

mod common;

use common::RunnerProject;
use rcfd_project::RunState;
use rcfd_studio::case_starter::EXAMPLES;
use rcfd_studio::poll::RunPoller;
use rcfd_studio::runs::StartPlan;
use rcfd_studio::viewport::StoreView;
use rcfd_workspace::plan::{parse_case, Plan};
use rcfd_workspace::{Answer, Workspace};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// What both plans give: the `--set` entries, the snapshot count, the size estimate, "no store", and the note.
type Seen = (Vec<String>, Option<u64>, Option<u64>, bool, Option<String>);

fn studio(text: &str, dir: &Path) -> Seen {
    let p = StartPlan::new("cases/x.toml", text, dir).unwrap_or_else(|e| panic!("the Studio's plan: {e}"));
    (
        p.set(),
        p.snapshots(),
        p.estimated_bytes(),
        p.store.is_none(),
        p.note.clone(),
    )
}

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

fn cavity(alg: &str, extra: &str) -> String {
    format!(
        "[mesh]\ngenerator = \"cavity_2d\"\nnx = 12\nny = 12\n[physics]\nnu = 0.01\n[boundaries.lid]\nvelocity = \"fixed\"\nvelocity_value = [1.0, 0.0, 0.0]\n[boundaries.walls]\nvelocity = \"fixed\"\n[solver]\nalgorithm = \"{alg}\"\n{extra}"
    )
}

/// `piston("", "")` of `crates/rcfd-api/tests/dynamic_mesh.rs` (as `start_plan_r10.rs`).
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

#[test]
fn cp_the_workspace_plans_the_store_as_the_studio_does() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases");
    let mut n = 0;
    for e in EXAMPLES {
        let file = repo.join(e.source);
        let text = std::fs::read_to_string(&file).unwrap();
        let dir = file.parent().unwrap();
        let (s, w) = (studio(&text, dir), workspace(&text, dir, &[]));
        eprintln!("C-P {}: {:?}", e.source, w);
        assert_eq!(w, s, "{}", e.source);
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
        let (s, w) = (studio(text, here), workspace(text, here, &[]));
        eprintln!("C-P {what}: {:?}", w);
        assert_eq!(w, s, "{what}");
        verdicts.push(w.3);
        n += 1;
    }
    // the variants reach both verdicts and both rules
    assert_eq!(verdicts.iter().filter(|x| **x).count(), 3, "{verdicts:?}");
    // an override is planned for as the Studio plans for the edited text
    let edited = steady.replace("max_iterations = 600", "max_iterations = 40");
    let with = workspace(&steady, here, &["solver.max_iterations=40"]);
    assert_eq!(with, studio(&edited, here));
    // control: without the override the plans differ (6 against 1)
    assert_ne!(workspace(&steady, here, &[]), studio(&edited, here));
    eprintln!(
        "C-P: {n} texts equal; override planned as the edited text: {:?}",
        with.0
    );
}

#[test]
fn cs_the_studio_lists_and_reads_a_run_that_new_case_made() {
    let mut p = RunnerProject::new("f271-cs", 2);
    p.serve();
    let ws = Workspace::open(&p.dir).unwrap().with_runner_exe(&p.runner);
    let mut poller = RunPoller::new(ws.root(), &p.runner);
    let (first, _) = poller.tick();
    assert!(first.rows.as_ref().is_none_or(|r| r.is_empty()), "no run yet");
    let v = match ws.call(
        "start_run",
        &json!({"new_case": {"name": "sod", "example": "sod/sod.toml"}}),
    ) {
        Some(Ok(Answer::Value(v))) => v,
        other => panic!("start_run: {other:?}"),
    };
    let id = v["run"].as_str().unwrap().to_string();
    // the first tick after the call lists the run
    let (info, _) = poller.tick();
    let rows = info.rows.expect("the rows changed");
    let row = rows.iter().find(|r| r.id == id).expect("listed at the first tick");
    eprintln!(
        "C-S: tick {} lists {id}: case {} state {:?}",
        info.n, row.case, row.state
    );
    assert_eq!(row.case, "cases/sod.toml");
    let st = p.client.wait(&id, Duration::from_secs(300)).unwrap();
    assert_eq!(st.state, RunState::Finished, "{st:?}");
    let (info, _) = poller.tick();
    let rows = info.rows.expect("the state changed");
    assert_eq!(
        rows.iter().find(|r| r.id == id).map(|r| r.state),
        Some(RunState::Finished)
    );
    // the Studio's store reader: snapshots, and the last one's pressure as finite face values
    let mut sv = StoreView::open(&p.run_dir(&id).join("fields")).unwrap();
    let snaps = sv.snaps();
    let last = snaps.last().expect("snapshots");
    let f = sv.sample("p", last.time, false).unwrap();
    eprintln!(
        "C-S: {} snapshots, the last at t = {} (index {}), p over {} faces in {:?}",
        snaps.len(),
        last.time,
        f.index,
        f.faces.len(),
        f.range
    );
    assert_eq!(f.index + 1, snaps.len());
    assert!(!f.faces.is_empty() && f.faces.iter().all(|x| x.is_finite()));
}
