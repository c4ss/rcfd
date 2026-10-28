//! ADR 0152 (job 48R) DV1-DV6: a store key at its schema default is not the case's own (the maintainer's answer to ADR
//! 0149 question 9), so a case saved in the Studio (every key spelled out, `store_interval = 0` among them) stores its
//! fields when the Studio starts it, and the Run side shows a picture. Negative controls: the route before
//! (`PlanRoute::default_store_keys_set`, `QuickRun::switches.default_store_keys_set`,
//! `poll::Switches::default_store_keys_set`). Release builds, `--test-threads 1 --nocapture`; `rcfd-runner` and `rcfd`
//! of the same build (`common::binaries`); `TMP` outside every checkout (ADR 0152, "What I already knew").

mod common;
mod dv;
mod g4;
mod lj;
mod pv;
mod vp;

use common::RunnerProject;
use rcfd_api::case::Case;
use rcfd_project::records::ResidualLine;
use rcfd_project::RunState;
use rcfd_studio::case_edit::CaseEditor;
use rcfd_studio::quick_run;
use rcfd_studio::runs::{case_facts_route, PlanRoute, StartPlan, StorePlan};
use rcfd_studio::store_plan::{self, Count, Effective, Fix, Warning};
use rcfd_studio::ui::StartDialog;
use rcfd_studio::view_toggle::Side;
use rcfd_studio::viewport::{Status, StoreView, ViewGpu, MOUSE_HINT};
use rcfd_studio::{Studio, StudioApp};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);
const RUN_WAIT: Duration = Duration::from_secs(60);
/// W-case-off of ADR 0140 for `output.store_interval`.
const CASE_OFF: &str =
    "The case turns the field store off (output.store_interval = 0), so the Viewport will have nothing to show for this run.";

/// The route before ADR 0152.
fn before() -> PlanRoute {
    PlanRoute {
        default_store_keys_set: true,
        ..PlanRoute::default()
    }
}

/// The cavity (`common::RUN_CASE`) with an `[output]` table of `lines`.
fn cavity_with(lines: &str) -> String {
    format!("{}\n[output]\n{lines}\n", common::RUN_CASE)
}

/// SF4 (b)'s piso cavity.
fn piso() -> String {
    common::RUN_CASE.replace(
        "algorithm = \"simplec\"",
        "algorithm = \"piso\"\ndt = 0.0025\nend_time = 0.5",
    )
}

fn piso_with(lines: &str) -> String {
    format!("{}\n[output]\n{lines}\n", piso())
}

/// The JSON text of a TOML text's tree (only the keys the text sets).
fn json_of(toml_text: &str) -> String {
    serde_json::to_string_pretty(&toml::from_str::<Value>(toml_text).unwrap()).unwrap()
}

fn plan_by(text: &str, route: PlanRoute) -> StartPlan {
    StartPlan::new_route("cases/cavity.toml", text, Path::new("."), route).unwrap()
}

/// The text a save by the case editor writes for `text` with `path` committed as `value` (the field's commit), made
/// in-process (`CaseEditor::edit_text`, `CaseEditor::to_bytes`).
fn editor_save(text: &str, path: &str, value: &str) -> String {
    let st = Studio::in_memory();
    let mut ed =
        CaseEditor::from_bytes_in(st.svc.clone(), "cases/cavity.toml", Path::new("."), text.as_bytes()).unwrap();
    ed.edit_text(&st.schema, path, value).unwrap();
    String::from_utf8(ed.to_bytes().unwrap()).unwrap()
}

// ---------------------------------------------------------------- DV1: the rule

/// (`sets_store`, `sets_precision`, the keys of `case_store`) of a text by a route.
type Read = (bool, bool, Vec<String>);

fn read(text: &str, route: PlanRoute) -> Read {
    let f = case_facts_route(text, Path::new("."), route).unwrap();
    (
        f.sets_store,
        f.sets_precision,
        f.extra.case_store.iter().map(|(k, _)| k.clone()).collect(),
    )
}

fn r(store: bool, precision: bool, keys: &[&str]) -> Read {
    (store, precision, keys.iter().map(|k| k.to_string()).collect())
}

const ALL_DEFAULTS: &str =
    "store_interval = 0\nstore_time_interval = 0.0\nstore_fields = []\nstore_precision = \"f64\"";

/// DV1: the schema defaults, and which store keys a text sets by ADR 0152's rule (TOML and JSON alike); a text whose
/// store keys are at their defaults plans exactly as the text without them.
#[test]
fn dv1_a_store_key_at_its_schema_default_is_not_set() {
    let d = store_plan::store_defaults();
    eprintln!("DV1 defaults: {d:?}");
    let keys: Vec<&str> = d.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        keys,
        [
            "output.store_interval",
            "output.store_time_interval",
            "output.store_fields",
            "output.store_precision"
        ]
    );
    assert!(d[0].1.is_u64() && d[0].1.as_u64() == Some(0), "{:?}", d[0]);
    assert!(d[1].1.is_f64() && d[1].1.as_f64() == Some(0.0), "{:?}", d[1]);
    assert_eq!(d[2].1, json!([]));
    assert_eq!(d[3].1, json!("f64"));
    let none = r(false, false, &[]);
    let table: Vec<(&str, String, Read)> = vec![
        ("(i)", cavity_with("store_interval = 0"), none.clone()),
        ("(ii)", piso_with("store_time_interval = 0.0"), none.clone()),
        ("(iii)", piso_with("store_time_interval = 0"), none.clone()),
        ("(iv)", piso_with("store_time_interval = -0.0"), none.clone()),
        ("(v)", cavity_with("store_precision = \"f64\""), none.clone()),
        ("(vi)", cavity_with("store_fields = []"), none.clone()),
        ("(vii)", cavity_with(ALL_DEFAULTS), none.clone()),
        (
            "(viii)",
            cavity_with("store_interval = 7"),
            r(true, false, &["output.store_interval"]),
        ),
        (
            "(ix)",
            piso_with("store_time_interval = 0.05"),
            r(true, false, &["output.store_time_interval"]),
        ),
        (
            "(x)",
            cavity_with("store_precision = \"f32\""),
            r(false, true, &["output.store_precision"]),
        ),
        (
            "(xi)",
            cavity_with("store_fields = [\"p\"]"),
            r(false, false, &["output.store_fields"]),
        ),
        (
            "(xii)",
            piso_with("store_interval = 0\nstore_time_interval = 0.05"),
            r(true, false, &["output.store_time_interval"]),
        ),
    ];
    let mut controls = Vec::new();
    for (id, text, want) in &table {
        let got = read(text, PlanRoute::default());
        let got_json = read(&json_of(text), PlanRoute::default());
        let got_before = read(text, before());
        eprintln!("DV1 {id}: {got:?}; JSON {got_json:?}; the route before {got_before:?}");
        assert_eq!(&got, want, "{id}");
        assert_eq!(&got_json, want, "{id} (JSON)");
        controls.push((*id, got_before));
    }
    // (i)-(vii): exactly the plan of the same case without the store keys
    for (id, text, _) in &table[..7] {
        let bare = if text.contains("piso") {
            piso()
        } else {
            common::RUN_CASE.to_string()
        };
        assert_eq!(
            plan_by(text, PlanRoute::default()),
            plan_by(&bare, PlanRoute::default()),
            "{id}: not the plan of the case without the store keys"
        );
        assert_eq!(
            plan_by(&json_of(text), PlanRoute::default()),
            plan_by(&bare, PlanRoute::default()),
            "{id} (JSON): not the plan of the case without the store keys"
        );
    }
    // negative control: the route before takes every key in the text as the case's own
    let c = |id: &str| controls.iter().find(|(i, _)| *i == id).unwrap().1.clone();
    for id in ["(i)", "(ii)", "(iii)", "(iv)", "(vii)"] {
        assert!(c(id).0, "control {id}: sets_store {:?}", c(id));
    }
    for id in ["(v)", "(vii)"] {
        assert!(c(id).1, "control {id}: sets_precision {:?}", c(id));
    }
    assert_eq!(
        c("(vii)").2,
        [
            "output.store_interval",
            "output.store_time_interval",
            "output.store_fields",
            "output.store_precision"
        ]
    );
}

// ---------------------------------------------------------------- DV2, DV3: saved, Run, a picture

/// The Studio model on `p` (watcher and run worker on), the Run button searching `p`'s runner.
fn studio(p: &RunnerProject) -> Studio {
    let mut st = Studio::in_memory();
    st.runner_exe = p.runner.clone();
    st.quick.search = rcfd_studio::bins::Search::explicit(&p.runner);
    st.open_project(&p.dir);
    assert!(
        st.drain_until(WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()),
        "the project did not open"
    );
    st
}

fn settle(st: &mut Studio) {
    assert!(
        st.drain_until(WAIT, |s| s.pending_case.is_none() && lj::settled(s)),
        "did not settle: {:?}",
        st.preview.status
    );
}

/// `rel` saved by the editor: selected and settled, `path` committed as `value` (the field's commit), Case > Save,
/// drained until the editor is clean and not saving. The bytes on disk.
fn save_by_editor(st: &mut Studio, root: &Path, rel: &str, path: &str, value: &str) -> Vec<u8> {
    st.open_case(rel);
    settle(st);
    let schema = st.schema.clone();
    st.editor
        .as_mut()
        .expect("the case is open")
        .edit_text(&schema, path, value)
        .unwrap();
    st.save_case().unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.editor.as_ref().is_some_and(|e| !e.saving && !e.is_dirty())),
        "the save did not end"
    );
    std::fs::read(root.join(rel)).unwrap()
}

/// Run as F5 asks for it; the id of the run it queued.
fn run_now(st: &mut Studio) -> String {
    let before = st.quick.run.as_ref().map(|r| r.0.clone());
    quick_run::run_selected_case(st).unwrap();
    assert!(
        st.drain_until(WAIT, |s| s.quick.starting.is_none()),
        "the start did not end"
    );
    match st.quick.run.as_ref().map(|r| r.0.clone()) {
        Some(id) if Some(&id) != before.as_ref() => id,
        _ => panic!("not started: {:?}", st.quick.error),
    }
}

fn wait_finished(st: &mut Studio, p: &RunnerProject, id: &str) -> bool {
    st.drain_until(RUN_WAIT, |_| p.status(id).is_some_and(|s| s.state.is_terminal()))
        && p.status(id).is_some_and(|s| s.state == RunState::Finished)
}

/// The residual lines of a run.
fn residuals(p: &RunnerProject, id: &str) -> Vec<ResidualLine> {
    std::fs::read_to_string(p.run_dir(id).join("residuals.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// A residual line as bits: step, time, residuals and monitors by name.
type Bits = (u64, u64, Vec<(String, u64)>, Vec<(String, u64)>);

fn line_bits(l: &ResidualLine) -> Bits {
    (
        l.step,
        l.time.to_bits(),
        l.residuals.iter().map(|(k, v)| (k.clone(), v.to_bits())).collect(),
        l.monitors.iter().map(|(k, v)| (k.clone(), v.to_bits())).collect(),
    )
}

/// The saved case's text as a tree (JSON when it starts with `{`, else TOML).
fn tree_of(bytes: &[u8]) -> Value {
    let text = std::str::from_utf8(bytes).unwrap();
    if text.trim_start().starts_with('{') {
        serde_json::from_str(text).unwrap()
    } else {
        toml::from_str(text).unwrap()
    }
}

/// The precondition of DV2 (a) and DV3: `physics.nu` 0.02 and the four store keys spelled out at their defaults
/// (`store_interval` an integer 0, `store_time_interval` a float 0.0).
fn saved_as_found(bytes: &[u8]) -> bool {
    let t = tree_of(bytes);
    let o = &t["output"];
    t["physics"]["nu"].as_f64() == Some(0.02)
        && o["store_interval"].is_u64()
        && o["store_interval"].as_u64() == Some(0)
        && o["store_time_interval"].is_f64()
        && o["store_time_interval"].as_f64() == Some(0.0)
        && o["store_precision"] == json!("f64")
        && o["store_fields"] == json!([])
}

/// `run.json` without `id`, `created` and `settings.set`.
fn record_rest(p: &RunnerProject, id: &str) -> Value {
    let mut v: Value = serde_json::from_slice(&std::fs::read(p.run_dir(id).join("run.json")).unwrap()).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("id");
    o.remove("created");
    if let Some(s) = o.get_mut("settings").and_then(Value::as_object_mut) {
        s.remove("set");
    }
    v
}

fn set_of(p: &RunnerProject, id: &str) -> Vec<String> {
    let v: Value = serde_json::from_slice(&std::fs::read(p.run_dir(id).join("run.json")).unwrap()).unwrap();
    serde_json::from_value(v["settings"]["set"].clone()).unwrap_or_default()
}

/// What DV2 or DV3 saw.
#[derive(Debug, Default)]
struct Saved {
    as_found: bool,
    a: String,
    a_finished: bool,
    a_set: Vec<String>,
    a_snapshots: usize,
    /// The field names and precisions of every snapshot of A's store.
    a_fields_ok: bool,
    a_fields: Vec<String>,
    a_side: Option<Side>,
    a_shown: bool,
    a_status: String,
    a_field: Option<String>,
    a_last_p_equal: bool,
    a_note: Option<String>,
    a_drawn_caption: bool,
    a_drawn_picture: bool,
    gpu: Option<String>,
    a_renders: Option<(u64, u64)>,
    a_gpu_error: Option<String>,
    b: String,
    b_finished: bool,
    b_set: Vec<String>,
    b_store: bool,
    b_shown: bool,
    b_status: String,
    b_explained: Option<String>,
    b_drawn_picture: bool,
    snapshots_equal: bool,
    hash_equal: bool,
    rest_equal: bool,
    residual_lines: usize,
    residuals_equal: bool,
}

/// DV2 and DV3's procedure: `rel` (holding `text`) in a served project, saved by the editor (`physics.nu` 0.02); Run:
/// A; the route before, Run: B; the model drawn with A, then B; A and B compared.
fn saved_runs(tag: &str, rel: &str, text: &str) -> Saved {
    let mut p = RunnerProject::new(tag, 3);
    std::fs::write(p.dir.join(rel), text).unwrap();
    p.serve();
    let mut st = studio(&p);
    let mut s = Saved::default();
    // (a)
    let saved = save_by_editor(&mut st, &p.dir, rel, "physics.nu", "0.02");
    s.as_found = saved_as_found(&saved);
    eprintln!(
        "{tag} (a): saved {} bytes; the store keys spelled out at their defaults: {}",
        saved.len(),
        s.as_found
    );
    // (b) Run: A
    s.a = run_now(&mut st);
    let a = s.a.clone();
    s.a_finished = wait_finished(&mut st, &p, &a);
    s.a_set = set_of(&p, &a);
    let dir = p.run_dir(&a).join("fields");
    s.a_snapshots = g4::store_snapshots(&dir);
    if s.a_snapshots > 0 {
        let mut rd = rcfd_tsstore::Reader::open(&dir).unwrap();
        s.a_fields_ok = (0..s.a_snapshots).all(|k| {
            rd.fields(k).is_ok_and(|f| {
                f.iter().map(|x| x.name.as_str()).collect::<Vec<_>>() == ["ux", "uy", "uz", "p"]
                    && f.iter().all(|x| x.precision == rcfd_tsstore::Precision::F32)
            })
        });
        s.a_fields = rd
            .fields(s.a_snapshots - 1)
            .map(|f| f.iter().map(|x| format!("{} {:?}", x.name, x.precision)).collect())
            .unwrap_or_default();
    }
    // (c) A on the Run side
    let n = s.a_snapshots;
    s.a_shown = n > 0
        && st.drain_until(WAIT, |x| {
            x.view.side == Side::Run
                && x.viewport.run.as_deref() == Some(a.as_str())
                && x.viewport.status == Status::Ready
                && x.viewport.snaps.len() == n
                && x.viewport.frame.as_ref().is_some_and(|f| f.index + 1 == n)
                && !x.viewport.pending()
        });
    s.a_side = Some(st.view.side);
    s.a_status = format!("{:?}", st.viewport.status);
    s.a_field = st.viewport.field.clone();
    s.a_note = st.viewport.store_note.clone();
    if s.a_shown {
        let sv = StoreView::open(&dir).unwrap();
        let mut rd = rcfd_tsstore::Reader::open(&dir).unwrap();
        let want = sv
            .surface
            .face_values_from_cells(&rd.read_field(n - 1, "p").unwrap())
            .unwrap();
        let f = st.viewport.frame.as_ref().unwrap();
        s.a_last_p_equal =
            f.faces.len() == want.len() && f.faces.iter().zip(&want).all(|(x, y)| x.to_bits() == y.to_bits());
    }
    eprintln!(
        "{tag} (b), (c): run A {a}: finished {}, set {:?}, {n} snapshots, fields {:?}; side {:?}, viewport {}, field {:?}, \
         last p bit-equal {}, note {:?}",
        s.a_finished, s.a_set, s.a_fields, s.a_side, s.a_status, s.a_field, s.a_last_p_equal, s.a_note
    );
    // (d) the route before, Run: B
    st.quick.switches.default_store_keys_set = true;
    s.b = run_now(&mut st);
    st.quick.switches.default_store_keys_set = false;
    let b = s.b.clone();
    s.b_finished = wait_finished(&mut st, &p, &b);
    s.b_set = set_of(&p, &b);
    s.b_store = p.run_dir(&b).join("fields").join(rcfd_tsstore::STORE_FILE).exists();
    s.b_shown = st.drain_until(WAIT, |x| {
        x.view.side == Side::Run
            && x.viewport.run.as_deref() == Some(b.as_str())
            && matches!(x.viewport.status, Status::NoStore(_))
    });
    s.b_status = format!("{:?}", st.viewport.status);
    s.b_explained = st.viewport.empty.as_ref().map(|e| format!("{:?}: {}", e.kind, e.text));
    eprintln!(
        "{tag} (d): run B {b} (the route before): finished {}, set {:?}, store {}, viewport {} (explanation {:?})",
        s.b_finished, s.b_set, s.b_store, s.b_status, s.b_explained
    );
    // (e) drawn
    let mut app = StudioApp::new(st, None);
    let ctx = egui::Context::default();
    let gpu = vp::adapter_list().first();
    match gpu {
        Some(ad) => {
            s.gpu = Some(vp::adapter_name(ad));
            app.st.viewport.set_gpu(ViewGpu::from_adapter(ad));
        }
        None => eprintln!(
            "SKIP {}:{}: DV2 (e) GPU part: no wgpu adapter (DX12 or Vulkan)",
            file!(),
            line!()
        ),
    }
    let r0 = app.st.viewport.gpu().map_or(0, |g| g.renders);
    app.st.select_run(&a);
    let shown_a = common::frames_until(&mut app, &ctx, WAIT, |x| {
        x.st.viewport.run.as_deref() == Some(a.as_str())
            && x.st.viewport.status == Status::Ready
            && x.st.viewport.frame.as_ref().is_some_and(|f| f.index + 1 == n.max(1))
            && !x.st.viewport.pending()
    });
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn = g4::texts(&common::frame(&mut app, &ctx));
    }
    let hover = format!("hover over the surface for its value; {MOUSE_HINT}");
    s.a_drawn_caption = drawn.contains(&format!("stored fields of run {a}"));
    s.a_drawn_picture = shown_a && drawn.contains(&hover);
    s.a_renders = app.st.viewport.gpu().map(|g| (r0, g.renders));
    s.a_gpu_error = app.st.viewport.gpu_error().map(String::from);
    app.st.select_run(&b);
    let _ = common::frames_until(&mut app, &ctx, WAIT, |x| {
        x.st.viewport.run.as_deref() == Some(b.as_str()) && matches!(x.st.viewport.status, Status::NoStore(_))
    });
    for _ in 0..3 {
        drawn = g4::texts(&common::frame(&mut app, &ctx));
    }
    s.b_drawn_picture = drawn.contains(&hover);
    eprintln!(
        "{tag} (e): A drawn: caption {}, picture {}, GPU {:?} renders {:?} error {:?}; B drawn: picture {}",
        s.a_drawn_caption, s.a_drawn_picture, s.gpu, s.a_renders, s.a_gpu_error, s.b_drawn_picture
    );
    // (f) A and B compared
    let snap = |id: &str| std::fs::read(p.run_dir(id).join("case.snapshot")).unwrap_or_default();
    s.snapshots_equal = snap(&a) == snap(&b) && snap(&a) == saved;
    let hash = |id: &str| {
        let v: Value = serde_json::from_slice(&std::fs::read(p.run_dir(id).join("run.json")).unwrap()).unwrap();
        v["case_snapshot_hash"].clone()
    };
    s.hash_equal = hash(&a) == hash(&b) && hash(&a).is_string();
    s.rest_equal = record_rest(&p, &a) == record_rest(&p, &b);
    let (ra, rb) = (residuals(&p, &a), residuals(&p, &b));
    s.residual_lines = ra.len();
    s.residuals_equal =
        !ra.is_empty() && ra.iter().map(line_bits).collect::<Vec<_>>() == rb.iter().map(line_bits).collect::<Vec<_>>();
    eprintln!(
        "{tag} (f): snapshots equal {}, hash equal {}, the rest of run.json equal {} ({}), residual lines {} equal {}",
        s.snapshots_equal,
        s.hash_equal,
        s.rest_equal,
        record_rest(&p, &a),
        s.residual_lines,
        s.residuals_equal
    );
    s
}

const PROPOSAL: [&str; 2] = ["output.store_interval=1", "output.store_precision=f32"];

/// DV2 and DV3's limits and controls.
fn check_saved(tag: &str, s: &Saved) {
    // (a)
    assert!(
        s.as_found,
        "{tag} (a): the saved file does not spell the store keys out: the check is void"
    );
    // (b)
    assert!(s.a_finished, "{tag} (b): run A did not finish");
    assert_eq!(s.a_set, PROPOSAL, "{tag} (b): A's set");
    assert_eq!(s.a_snapshots, 30, "{tag} (b): A's snapshots");
    assert!(s.a_fields_ok, "{tag} (b): A's fields {:?}", s.a_fields);
    // (c)
    assert_eq!(s.a_side, Some(Side::Run), "{tag} (c): side");
    assert!(s.a_shown, "{tag} (c): A not shown: {}", s.a_status);
    assert!(s.a_last_p_equal, "{tag} (c): the last p differs from the store");
    assert_eq!(s.a_note, None, "{tag} (c): store note");
    // (e)
    assert!(s.a_drawn_caption, "{tag} (e): the caption not drawn");
    assert!(s.a_drawn_picture, "{tag} (e): the picture not drawn");
    if s.gpu.is_some() {
        let (r0, r1) = s.a_renders.unwrap();
        assert!(r1 > r0, "{tag} (e): no render ({r0} -> {r1})");
        assert_eq!(s.a_gpu_error, None, "{tag} (e): GPU error");
    }
    // (f) check 5
    assert!(s.snapshots_equal, "{tag} (f): case.snapshot");
    assert!(s.hash_equal, "{tag} (f): case_snapshot_hash");
    assert!(s.rest_equal, "{tag} (f): run.json beyond id, created and set");
    let mut a_from_b: Vec<String> = PROPOSAL.iter().map(|x| x.to_string()).collect();
    a_from_b.extend(s.b_set.iter().cloned());
    assert_eq!(
        s.a_set, a_from_b,
        "{tag} (f): A's set is not the store entries followed by B's"
    );
    assert_eq!(s.residual_lines, 30, "{tag} (f): residual lines");
    assert!(s.residuals_equal, "{tag} (f): residuals differ");
    // negative control: B, the route before (the defect as found)
    assert!(s.b_finished, "{tag} control: run B did not finish");
    assert!(s.b_set.is_empty(), "{tag} control: B's set {:?}", s.b_set);
    assert!(!s.b_store, "{tag} control: B has a store");
    assert!(s.b_shown, "{tag} control: B not NoStore: {}", s.b_status);
    assert!(!s.b_drawn_picture, "{tag} control: B drawn with a picture");
}

/// DV2: a TOML case saved by the editor stores its fields when Run starts it, and the Run side shows them (checks 1 and
/// 5); the route before stores none (the control, run B).
#[test]
fn dv2_a_toml_case_saved_in_the_editor_stores_fields_and_shows_a_picture() {
    let s = saved_runs("dv2", "cases/s.toml", &g4::cavity(30));
    eprintln!("DV2: {s:#?}");
    check_saved("DV2", &s);
}

/// DV3: the same for a `.json` case written by `rcfd-api`'s writer and saved by the editor (check 4).
#[test]
fn dv3_a_json_case_saved_in_the_editor_stores_fields_and_shows_a_picture() {
    let json = Case::parse_toml(&g4::cavity(30)).unwrap().to_json_string();
    let s = saved_runs("dv3", "cases/j.json", &json);
    eprintln!("DV3: {s:#?}");
    check_saved("DV3", &s);
}

// ---------------------------------------------------------------- DV4: the Start run dialog

/// The Start run dialog's plan of `rel` (the run worker reads the file).
fn dialog_plan(st: &mut Studio, rel: &str) -> Result<StartPlan, String> {
    st.plan_start(rel).unwrap();
    assert!(
        st.drain_until(WAIT, |s| matches!(&s.runs.plan, Some(Ok(p)) if p.case == rel)
            || matches!(&s.runs.plan, Some(Err(_)))),
        "{rel}: no plan"
    );
    st.runs.plan.clone().unwrap()
}

/// The dialog drawn on `rel`: frames until the plan arrived, then 3 frames; the drawn texts.
fn dialog_drawn(st: Studio, rel: &str) -> Vec<String> {
    let mut app = StudioApp::new(st, None);
    app.ui_state.start_dialog = Some(StartDialog {
        case: rel.into(),
        ..Default::default()
    });
    let ctx = egui::Context::default();
    assert!(common::frames_until(&mut app, &ctx, WAIT, |a| {
        matches!(&a.st.runs.plan, Some(Ok(p)) if p.case == rel)
    }));
    let mut drawn = Vec::new();
    for _ in 0..3 {
        drawn = g4::texts(&common::frame(&mut app, &ctx));
    }
    drawn
}

/// A Studio on `root` (the watcher off, the run worker on), the route before for the dialog's plan when `route_before`.
fn dialog_studio(root: &Path, route_before: bool) -> Studio {
    let mut st = Studio::in_memory();
    st.poll = None;
    st.run_switches.default_store_keys_set = route_before;
    st.open_project(root);
    assert!(st.drain_until(WAIT, |s| s.browser().is_some() && s.runs.last_tick.is_some()));
    st
}

/// DV4: the Start run dialog of a case saved by the editor proposes the store (check 1, the dialog's route); the route
/// before shows W-case-off.
#[test]
fn dv4_the_start_dialog_of_a_saved_case_proposes_the_store() {
    let root = common::temp("dv4").join("proj");
    rcfd_project::Project::create(&root, "dv4", "rcfd test").unwrap();
    std::fs::write(root.join("cases/s.toml"), g4::cavity(30)).unwrap();
    let root = rcfd_project::paths::canonicalize(&root).unwrap();
    let rel = "cases/s.toml";
    let mut st = dialog_studio(&root, false);
    let saved = save_by_editor(&mut st, &root, rel, "physics.nu", "0.02");
    assert!(
        saved_as_found(&saved),
        "set-up: the saved file does not spell the store keys out"
    );
    // (a)
    let p = dialog_plan(&mut st, rel).unwrap();
    let lines = store_plan::case_lines(&p);
    eprintln!(
        "DV4 (a): store {:?}, note {:?}, set {:?}, warnings {:?}, sets_precision {}, {:?}; case lines (reported) {lines:?}",
        p.store,
        p.note,
        p.set(),
        store_plan::warnings(&p),
        p.facts.sets_precision,
        store_plan::store_line(&p)
    );
    assert!(p.store.as_ref().is_some_and(|s| s.on), "(a) store not on");
    assert_eq!(p.note, None, "(a) note");
    assert_eq!(p.set(), PROPOSAL, "(a) set");
    assert!(store_plan::warnings(&p).is_empty(), "(a) warnings");
    assert!(!p.facts.sets_precision, "(a) sets_precision");
    // (b)
    let drawn = dialog_drawn(st, rel);
    let store_line =
        "Stores ux, uy, uz, p every iteration: at most 30 snapshots (fewer if the run converges earlier), f32.";
    for want in ["Store fields for scrubbing (ux, uy, uz, p)", store_line] {
        assert!(drawn.iter().any(|t| t == want), "(b) not drawn: {want}\n{drawn:?}");
    }
    for not in ["(set by the case)", CASE_OFF, "Store fields for this run"] {
        assert!(!drawn.iter().any(|t| t == not), "(b) drawn: {not}");
    }
    eprintln!("DV4 (b): the label and the store line drawn; no (set by the case), no W-case-off");
    // negative control: the route before
    let mut st2 = dialog_studio(&root, true);
    let c = dialog_plan(&mut st2, rel).unwrap();
    let w = store_plan::warnings(&c);
    eprintln!(
        "DV4 control: store {:?}, note {:?}, set {:?}, warnings {w:?}",
        c.store,
        c.note,
        c.set()
    );
    assert_eq!(c.store, None);
    assert_eq!(
        c.note.as_deref(),
        Some("the case sets its own field store (output.store_*)")
    );
    assert!(c.set().is_empty());
    assert_eq!(
        w,
        [Warning {
            text: CASE_OFF.into(),
            fix: Some(("Store fields for this run".into(), Fix::StoreForCase))
        }]
    );
    let drawn = dialog_drawn(st2, rel);
    for want in [CASE_OFF, "Store fields for this run"] {
        assert!(drawn.iter().any(|t| t == want), "control (b) not drawn: {want}");
    }
}

// ---------------------------------------------------------------- DV5: explicit non-default keys as before

/// What the dialog and the run see of a plan.
#[derive(Debug, PartialEq)]
struct Seen {
    store: Option<StorePlan>,
    note: Option<String>,
    set: Vec<String>,
    settings: rcfd_runner::records::RunSettings,
    effective: Effective,
    count: Option<Count>,
    estimate: Option<u64>,
    store_line: Option<String>,
    size_line: Option<String>,
    case_lines: Vec<String>,
    warnings: Vec<Warning>,
}

fn seen(p: &StartPlan) -> Seen {
    let e = store_plan::effective(p);
    Seen {
        store: p.store.clone(),
        note: p.note.clone(),
        set: p.set(),
        settings: p.settings(),
        count: store_plan::count(&e),
        effective: e,
        estimate: store_plan::estimate(p),
        store_line: store_plan::store_line(p),
        size_line: store_plan::size_line(p),
        case_lines: store_plan::case_lines(p),
        warnings: store_plan::warnings(p),
    }
}

/// DV5: a case with explicit non-default store keys plans exactly as before (check 2).
#[test]
fn dv5_explicit_non_default_store_keys_behave_as_before() {
    let mut texts: Vec<(String, String, PathBuf)> = vec![
        ("(a)".into(), cavity_with("store_interval = 7"), ".".into()),
        ("(b)".into(), cavity_with("store_fields = [\"p\"]"), ".".into()),
        ("(c)".into(), cavity_with("store_precision = \"f32\""), ".".into()),
        (
            "(d)".into(),
            cavity_with("store_interval = 5\nstore_time_interval = 0.0"),
            ".".into(),
        ),
        ("(e)".into(), piso_with("store_time_interval = 0.05"), ".".into()),
        (
            "(f)".into(),
            piso_with("store_interval = 0\nstore_time_interval = 0.05"),
            ".".into(),
        ),
        (
            "(g)".into(),
            editor_save(common::RUN_CASE, "output.store_interval", "7"),
            ".".into(),
        ),
    ];
    let g = &texts[6].1;
    let gt = tree_of(g.as_bytes());
    assert!(
        gt["output"]["store_interval"].as_u64() == Some(7) && gt["output"]["store_precision"] == json!("f64"),
        "set-up (g): {}",
        gt["output"]
    );
    for e in dv::unedited()
        .into_iter()
        .filter(|e| e.label.starts_with("example-projects/"))
    {
        texts.push((format!("(h) {}", e.label), e.text, e.base));
    }
    assert_eq!(texts.len(), 17, "7 texts and the ten cases of example-projects/");
    for (id, text, base) in &texts {
        let a = StartPlan::new_route("cases/cavity.toml", text, base, PlanRoute::default()).unwrap();
        let b = StartPlan::new_route("cases/cavity.toml", text, base, before()).unwrap();
        let (sa, sb) = (seen(&a), seen(&b));
        eprintln!("DV5 {id}: {sa:?}");
        assert_eq!(sa, sb, "{id}: differs from the route before");
        if id.starts_with("(h)") {
            assert_eq!(a, b, "{id}: the whole plan differs from the route before");
        }
    }
    // negative control: the saved cavity's set() differs between the routes
    let saved = editor_save(&g4::cavity(30), "physics.nu", "0.02");
    let (a, b) = (plan_by(&saved, PlanRoute::default()), plan_by(&saved, before()));
    eprintln!(
        "DV5 control: the saved cavity: set {:?} against the route before {:?}",
        a.set(),
        b.set()
    );
    assert_ne!(a.set(), b.set());
    assert!(b.set().is_empty());
}

// ---------------------------------------------------------------- DV6: unedited cases, bit-identical

/// DV6: the plans of the 22 unedited cases equal the record made on 6867e40 byte for byte (a) and the route before (b);
/// the saved control differs from the record, and its route-before plan equals it.
#[test]
fn dv6_unedited_plans_are_bit_identical() {
    let entries = dv::unedited();
    assert_eq!(entries.len(), 22, "the unedited cases");
    let record = dv::blocks(&dv::recorded());
    let control = dv::saved_control();
    let labels: Vec<&str> = record.iter().map(|(l, _)| l.as_str()).collect();
    let mut want: Vec<&str> = entries.iter().map(|e| e.label.as_str()).collect();
    want.push(control.label.as_str());
    assert_eq!(labels, want, "the record's cases");
    let recorded = |label: &str| record.iter().find(|(l, _)| l == label).map(|(_, b)| b.clone()).unwrap();
    for e in &entries {
        // (a)
        let now = dv::plan_text(e, &StartPlan::new(&e.rel, &e.text, &e.base));
        assert_eq!(now, recorded(&e.label), "(a) {}: differs from the record", e.label);
        // (b)
        let a = StartPlan::new_route(&e.rel, &e.text, &e.base, PlanRoute::default()).unwrap();
        let b = StartPlan::new_route(&e.rel, &e.text, &e.base, before()).unwrap();
        assert_eq!(a, b, "(b) {}", e.label);
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "(b) {}", e.label);
        assert_eq!(a.set(), b.set(), "(b) {}", e.label);
        assert_eq!(a.settings(), b.settings(), "(b) {}", e.label);
        eprintln!(
            "DV6 {}: equal to the record and to the route before; set {:?}",
            e.label,
            a.set()
        );
    }
    // negative control: the saved control
    let app = dv::plan_text(
        &control,
        &StartPlan::new_route(&control.rel, &control.text, &control.base, PlanRoute::default()),
    );
    let old = dv::plan_text(
        &control,
        &StartPlan::new_route(&control.rel, &control.text, &control.base, before()),
    );
    eprintln!(
        "DV6 control: the app's route:\n{app}the record:\n{}",
        recorded(&control.label)
    );
    assert_ne!(
        app,
        recorded(&control.label),
        "control: the app's route equals the record"
    );
    assert_eq!(
        old,
        recorded(&control.label),
        "control: the route before differs from the record"
    );
}
