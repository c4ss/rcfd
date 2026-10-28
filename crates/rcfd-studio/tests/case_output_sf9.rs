//! ADR 0140 amendment 2, SF9 (j)-(l): a case's own output settings are respected and the start plan says what it does
//! with them: its own interval (kept), its own precision (f64), its own field list (`p` only, also end to end).

mod common;

use rcfd_api::case::Case;
use rcfd_api::chain::apply_set;
use rcfd_api::runctl::{drive, RunOptions};
use rcfd_api::sim::Simulation;
use rcfd_studio::runs::StartPlan;
use rcfd_studio::store_plan;
use rcfd_studio::ui::StartDialog;
use rcfd_studio::{Studio, StudioApp};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

fn with_output(lines: &str) -> String {
    format!("{}\n[output]\n{lines}\n", common::RUN_CASE)
}

fn plan(text: &str) -> StartPlan {
    StartPlan::new("cases/cavity.toml", text, Path::new(".")).unwrap()
}

#[test]
fn sf9_j_the_case_s_own_interval_is_kept() {
    let p = plan(&with_output("store_interval = 7"));
    eprintln!(
        "SF9 (j): store {:?}, set {:?}, lines {:?}, {:?}, {:?}, estimate {:?}, warnings {:?}",
        p.store,
        p.set(),
        store_plan::case_lines(&p),
        store_plan::store_line(&p),
        store_plan::size_line(&p),
        store_plan::estimate(&p),
        store_plan::warnings(&p)
    );
    assert!(p.store.is_none());
    assert!(p.set().is_empty());
    assert_eq!(
        store_plan::case_lines(&p),
        ["The case sets its own field store (output.store_interval = 7), which is kept."]
    );
    assert_eq!(
        store_plan::store_line(&p).unwrap(),
        "Stores ux, uy, uz, p every 7 iterations: at most 58 snapshots (fewer if the run converges earlier), f64."
    );
    assert_eq!(store_plan::estimate(&p), Some(348_346));
    assert_eq!(
        store_plan::size_line(&p).unwrap(),
        "Estimated size 348 kB (144 cells), in runs/<run id>/fields/ where the Viewport reads it."
    );
    assert!(store_plan::warnings(&p).is_empty());
    // negative control: the 48D estimate shows nothing for a case's own store
    eprintln!("SF9 (j) control: estimated_bytes() {:?}", p.estimated_bytes());
    assert_eq!(p.estimated_bytes(), None);
}

#[test]
fn sf9_k_the_case_s_own_precision_is_kept() {
    let p = plan(&with_output("store_precision = \"f64\""));
    eprintln!(
        "SF9 (k): set {:?}, {:?}, estimate {:?}, lines {:?}, warnings {:?}",
        p.set(),
        store_plan::store_line(&p),
        store_plan::estimate(&p),
        store_plan::case_lines(&p),
        store_plan::warnings(&p)
    );
    assert_eq!(p.set(), ["output.store_interval=4"]);
    assert!(store_plan::store_line(&p).unwrap().ends_with(", f64."));
    assert_eq!(store_plan::estimate(&p), Some(572_416));
    assert!(store_plan::case_lines(&p).is_empty());
    assert!(store_plan::warnings(&p).is_empty());
    // negative control: the 48D estimate assumes 4-byte values for this f64 store
    eprintln!("SF9 (k) control: estimated_bytes() {:?}", p.estimated_bytes());
    assert_eq!(p.estimated_bytes(), Some(230_400));
}

/// The text of every drawn text shape.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

#[test]
fn sf9_l_the_case_s_own_fields_are_stored_and_shown() {
    let text = with_output("store_fields = [\"p\"]");
    let p = plan(&text);
    let e = store_plan::effective(&p);
    eprintln!(
        "SF9 (l): set {:?}, fields {:?}, {:?}, estimate {:?}",
        p.set(),
        e.fields,
        store_plan::store_line(&p),
        store_plan::estimate(&p)
    );
    assert_eq!(p.set(), ["output.store_interval=4", "output.store_precision=f32"]);
    assert_eq!(e.fields, ["p"]);
    assert_eq!(
        store_plan::store_line(&p).unwrap(),
        "Stores p every 4 iterations: at most 100 snapshots (fewer if the run converges earlier), f32."
    );
    assert_eq!(store_plan::estimate(&p), Some(123_316));
    // negative control: the 48D estimate counts four fields
    eprintln!("SF9 (l) control: estimated_bytes() {:?}", p.estimated_bytes());
    assert_eq!(p.estimated_bytes(), Some(230_400));
    // the drawn label
    let root = common::temp("sf9-l").join("proj");
    rcfd_project::Project::create(&root, "sf9 l", "rcfd test").unwrap();
    std::fs::write(root.join("cases/cavity.toml"), &text).unwrap();
    let mut st = Studio::in_memory();
    st.poll = None;
    st.open_project(&root);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    let mut app = StudioApp::new(st, None);
    app.ui_state.start_dialog = Some(StartDialog {
        case: "cases/cavity.toml".into(),
        ..Default::default()
    });
    let ctx = egui::Context::default();
    assert!(common::frames_until(
        &mut app,
        &ctx,
        Duration::from_secs(30),
        |a| matches!(&a.st.runs.plan, Some(Ok(_)))
    ));
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn = texts(&common::frame(&mut app, &ctx).shapes);
    }
    let label = "Store fields for scrubbing (p)".to_string();
    eprintln!(
        "SF9 (l): label drawn {}; 48D label drawn {}",
        drawn.contains(&label),
        drawn.iter().any(|t| t == "Store fields for scrubbing (ux, uy, uz, p)")
    );
    assert!(drawn.contains(&label), "{drawn:?}");
    assert!(!drawn.iter().any(|t| t == "Store fields for scrubbing (ux, uy, uz, p)"));
    // end to end in process: three steps store exactly `p`, in f32
    let dir = common::temp("sf9-l-run");
    let file = dir.join("cavity.toml");
    std::fs::write(&file, &text).unwrap();
    let mut case = Case::load(&file).unwrap();
    for s in p.set() {
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
}
